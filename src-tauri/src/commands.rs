use crate::pin::PinManager;
use crate::storage::{Account, Storage};
use crate::totp;
#[cfg(target_os = "ios")]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tauri::State;
use zeroize::Zeroize;

fn validate_account_fields(
    issuer: &str,
    label: &str,
    algorithm: &str,
    digits: u32,
    period: u32,
) -> Result<(), String> {
    if issuer.len() > 255 {
        return Err("Issuer name is too long (max 255 characters)".to_string());
    }
    if label.len() > 255 {
        return Err("Label is too long (max 255 characters)".to_string());
    }
    if !matches!(algorithm, "SHA1" | "SHA256" | "SHA512") {
        return Err("Algorithm must be SHA1, SHA256, or SHA512".to_string());
    }
    if digits != 6 && digits != 8 {
        return Err("Digits must be 6 or 8".to_string());
    }
    if !(15..=120).contains(&period) {
        return Err("Period must be between 15 and 120 seconds".to_string());
    }
    Ok(())
}

fn lock_storage(storage: &Mutex<Storage>) -> Result<MutexGuard<'_, Storage>, String> {
    storage.lock().map_err(|_| {
        tracing::error!("Storage mutex poisoned");
        "Storage unavailable — please restart the app".to_string()
    })
}

fn lock_auth(auth: &AuthManager) -> Result<MutexGuard<'_, AuthState>, String> {
    auth.inner.lock().map_err(|_| {
        tracing::error!("Auth state mutex poisoned");
        "Auth state unavailable - please restart the app".to_string()
    })
}

fn ensure_unlocked(
    auth_manager: &State<AuthManager>,
    pin_manager: &State<PinManager>,
) -> Result<(), String> {
    if !pin_manager.has_pin() {
        return Ok(());
    }
    let auth = lock_auth(auth_manager)?;
    if auth.unlocked {
        Ok(())
    } else {
        Err("Vault is locked".to_string())
    }
}

#[derive(Default)]
struct AuthState {
    unlocked: bool,
    last_unlock_epoch: Option<u64>,
}

pub struct AuthManager {
    inner: Arc<Mutex<AuthState>>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AuthState::default())),
        }
    }
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub pin_enabled: bool,
    pub unlocked: bool,
    pub last_unlock_epoch: Option<u64>,
    /// True once if account data was unreadable and backed up at startup.
    pub data_recovered: bool,
}

#[derive(Serialize, Clone)]
pub struct AccountDisplay {
    pub id: String,
    pub issuer: String,
    pub label: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u32,
    pub icon: Option<String>,
}

impl From<Account> for AccountDisplay {
    fn from(a: Account) -> Self {
        Self {
            id: a.id,
            issuer: a.issuer,
            label: a.label,
            algorithm: a.algorithm,
            digits: a.digits,
            period: a.period,
            icon: a.icon,
        }
    }
}

#[tauri::command]
pub fn get_accounts(
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<AccountDisplay>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage = lock_storage(&storage)?;
    Ok(storage
        .list()
        .iter()
        .cloned()
        .map(AccountDisplay::from)
        .collect())
}

#[tauri::command]
pub fn add_account(
    uri: String,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<AccountDisplay, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let account = totp::parse_otpauth_uri(&uri)?;
    validate_account_fields(
        &account.issuer,
        &account.label,
        &account.algorithm,
        account.digits,
        account.period,
    )?;
    let mut storage = lock_storage(&storage)?;
    if storage.has_duplicate(&account.issuer, &account.label, &account.secret) {
        return Err("This account already exists".to_string());
    }
    let display = AccountDisplay::from(account.clone());
    storage.add(account)?;
    icloud_push_if_enabled(&app_handle, &storage);
    Ok(display)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn add_account_manual(
    issuer: String,
    label: String,
    secret: String,
    algorithm: String,
    digits: u32,
    period: u32,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<AccountDisplay, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    validate_account_fields(&issuer, &label, &algorithm, digits, period)?;
    let clean_secret = secret.to_uppercase().replace(' ', "");
    if clean_secret.is_empty() {
        return Err("Secret key is required".to_string());
    }
    if data_encoding::BASE32_NOPAD
        .decode(clean_secret.as_bytes())
        .is_err()
    {
        return Err("Secret key is not valid Base32".to_string());
    }
    let account = Account {
        id: uuid::Uuid::new_v4().to_string(),
        issuer,
        label,
        secret: clean_secret,
        algorithm,
        digits,
        period,
        icon: None,
        last_modified: 0,
    };

    // Validate by trying to generate a code
    totp::generate_code(&account)?;

    let mut storage = lock_storage(&storage)?;
    if storage.has_duplicate(&account.issuer, &account.label, &account.secret) {
        return Err("This account already exists".to_string());
    }
    let display = AccountDisplay::from(account.clone());
    storage.add(account)?;
    icloud_push_if_enabled(&app_handle, &storage);
    Ok(display)
}

#[tauri::command]
pub fn delete_account(
    id: String,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<(), String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let mut storage = lock_storage(&storage)?;
    storage.delete(&id)?;
    icloud_push_if_enabled(&app_handle, &storage);
    Ok(())
}

#[tauri::command]
pub fn generate_code(
    id: String,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<totp::CodeResponse, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage = lock_storage(&storage)?;
    let account = storage
        .get(&id)
        .ok_or_else(|| "Account not found".to_string())?;
    totp::generate_code(account)
}

#[tauri::command]
pub fn generate_all_codes(
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<totp::CodeResponse>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage = lock_storage(&storage)?;
    storage.list().iter().map(totp::generate_code).collect()
}

// --- Shared helpers ---

fn deduplicate_and_import(
    accounts: Vec<Account>,
    storage: &mut Storage,
) -> Result<Vec<AccountDisplay>, String> {
    let mut existing: std::collections::HashSet<(String, String, String)> = storage
        .list()
        .iter()
        .map(|e| (e.issuer.clone(), e.label.clone(), e.secret.clone()))
        .collect();
    let mut added = Vec::new();

    for account in accounts {
        let dedupe_key = (
            account.issuer.clone(),
            account.label.clone(),
            account.secret.clone(),
        );
        if existing.insert(dedupe_key) {
            let display = AccountDisplay::from(account.clone());
            let mut new_account = account;
            new_account.id = uuid::Uuid::new_v4().to_string();
            storage.add(new_account)?;
            added.push(display);
        }
    }
    Ok(added)
}

// --- Backup commands ---

fn backup_filename() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("ghost-auth-backup-{}.ghostauth", timestamp)
}

fn save_backup_file_inner(
    data: &[u8],
    filename: &str,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    #[cfg(target_os = "ios")]
    {
        // iOS: write to temp dir, then present the native share sheet.
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(filename);

        std::fs::write(&path, data).map_err(|e| {
            tracing::error!(error = %e, "Failed to write backup file to temp dir");
            "Failed to save backup".to_string()
        })?;

        let path_str = path.to_string_lossy().to_string();

        use tauri_plugin_share_file::ShareFileExt;
        app_handle
            .share_file_plugin()
            .share_file(&path_str, "application/octet-stream")
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to present share sheet");
                "Failed to share backup file".to_string()
            })?;

        tracing::info!(event = "backup_shared", path = %path_str, "Backup file shared via iOS share sheet");
        Ok(path_str)
    }

    #[cfg(target_os = "android")]
    {
        // Android 10+ (API 29): scoped storage forbids raw filesystem writes to
        // shared storage, so save into the public Downloads collection through
        // MediaStore. This needs no runtime permission and makes the file
        // visible in the user's Downloads — unlike the app-scoped external
        // directory that `download_dir()` resolves to on Android.
        match crate::android_downloads::save_to_downloads(
            filename,
            "application/octet-stream",
            data,
        ) {
            Ok(location) => {
                tracing::info!(event = "backup_saved", location = %location, "Backup saved to public Downloads via MediaStore");
                Ok(location)
            }
            Err(e) => {
                // Pre-29 devices (or an unexpected MediaStore failure) fall back
                // to the app-scoped external directory so the export still
                // completes rather than failing outright.
                tracing::warn!(error = %e, "MediaStore save unavailable; falling back to app-private storage");
                save_to_download_dir(data, filename, app_handle)
            }
        }
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop: download_dir() is the user's real Downloads folder.
        save_to_download_dir(data, filename, app_handle)
    }
}

/// Write a file into the Downloads directory reported by Tauri's path resolver.
/// On desktop this is the user's real Downloads folder; on Android it is the
/// app-scoped external directory, used only as a pre-29 fallback for
/// [`save_backup_file_inner`].
#[cfg(not(target_os = "ios"))]
fn save_to_download_dir(
    data: &[u8],
    filename: &str,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    use tauri::Manager;

    let backup_dir = app_handle.path().download_dir().map_err(|e| {
        tracing::error!(error = %e, "Failed to resolve downloads directory");
        "Failed to save backup".to_string()
    })?;
    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        tracing::error!(error = %e, "Failed to create backup directory");
        "Failed to save backup".to_string()
    })?;

    let path = backup_dir.join(filename);

    std::fs::write(&path, data).map_err(|e| {
        tracing::error!(error = %e, "Failed to write backup file");
        "Failed to save backup".to_string()
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    let path_str = path.to_string_lossy().to_string();
    tracing::info!(event = "backup_saved", path = %path_str, "Backup file saved");
    Ok(path_str)
}

#[tauri::command]
pub fn export_backup(
    password: String,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<u8>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage = lock_storage(&storage)?;
    let accounts = storage.list();
    let result = crate::backup::export_accounts(accounts, &password)?;
    tracing::info!(
        event = "backup_exported",
        count = accounts.len(),
        "Backup exported"
    );
    Ok(result)
}

#[tauri::command]
pub async fn export_backup_file(
    password: String,
    storage: State<'_, Mutex<Storage>>,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage = lock_storage(&storage)?;
    let accounts = storage.list();
    let count = accounts.len();
    let data = crate::backup::export_accounts(accounts, &password)?;
    drop(storage);

    let filename = backup_filename();
    let path_str = save_backup_file_inner(&data, &filename, &app_handle)?;
    tracing::info!(
        event = "backup_exported_to_file",
        count,
        path = %path_str,
        "Backup exported to file"
    );
    Ok(path_str)
}

#[derive(Serialize)]
pub struct BackupPreview {
    pub accounts: Vec<AccountDisplay>,
    pub duplicates: usize,
}

#[tauri::command]
pub fn import_backup(
    data: Vec<u8>,
    password: String,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<BackupPreview, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let accounts = crate::backup::import_accounts(&data, &password)?;
    let storage = lock_storage(&storage)?;
    let existing: Vec<(&str, &str, &str)> = storage
        .list()
        .iter()
        .map(|e| (e.issuer.as_str(), e.label.as_str(), e.secret.as_str()))
        .collect();

    let mut new_accounts = Vec::new();
    let mut duplicates = 0usize;
    for account in accounts {
        let is_dup = existing
            .iter()
            .any(|e| e.0 == account.issuer && e.1 == account.label && e.2 == account.secret);
        if is_dup {
            duplicates += 1;
        } else {
            new_accounts.push(AccountDisplay::from(account));
        }
    }

    Ok(BackupPreview {
        accounts: new_accounts,
        duplicates,
    })
}

#[tauri::command]
pub fn import_backup_confirm(
    data: Vec<u8>,
    password: String,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<AccountDisplay>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let accounts = crate::backup::import_accounts(&data, &password)?;
    let mut storage = lock_storage(&storage)?;
    let added = deduplicate_and_import(accounts, &mut storage)?;
    icloud_push_if_enabled(&app_handle, &storage);
    tracing::info!(
        event = "backup_imported",
        count = added.len(),
        "Backup imported"
    );
    Ok(added)
}

// --- Backup file save (mobile-compatible) ---

#[tauri::command]
pub async fn save_backup_file(
    data: Vec<u8>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let filename = backup_filename();
    save_backup_file_inner(&data, &filename, &app_handle)
}

// --- Export QR commands ---

const EXPORT_BATCH_SIZE: usize = 8;

#[derive(Serialize, Clone)]
pub struct ExportAccountInfo {
    pub issuer: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct ExportBatch {
    pub migration_uri: String,
    pub accounts: Vec<ExportAccountInfo>,
    pub batch_index: usize,
    pub batch_count: usize,
}

fn account_to_otp_params(
    account: &Account,
) -> Result<crate::google_auth_proto::OtpParameters, String> {
    let secret_bytes = data_encoding::BASE32_NOPAD
        .decode(account.secret.as_bytes())
        .map_err(|e| format!("Failed to decode secret for {}: {e}", account.issuer))?;

    let algorithm = match account.algorithm.as_str() {
        "SHA1" => crate::google_auth_proto::Algorithm::Sha1 as i32,
        "SHA256" => crate::google_auth_proto::Algorithm::Sha256 as i32,
        "SHA512" => crate::google_auth_proto::Algorithm::Sha512 as i32,
        _ => crate::google_auth_proto::Algorithm::Sha1 as i32,
    };

    let digits = match account.digits {
        8 => crate::google_auth_proto::DigitCount::Eight as i32,
        _ => crate::google_auth_proto::DigitCount::Six as i32,
    };

    let name = if !account.label.is_empty() {
        account.label.clone()
    } else {
        account.issuer.clone()
    };

    Ok(crate::google_auth_proto::OtpParameters {
        secret: secret_bytes,
        name,
        issuer: account.issuer.clone(),
        algorithm,
        digits,
        otp_type: crate::google_auth_proto::OtpType::Totp as i32,
        counter: 0,
    })
}

fn build_migration_uri(
    params: Vec<crate::google_auth_proto::OtpParameters>,
    batch_size: i32,
    batch_index: i32,
) -> Result<String, String> {
    use prost::Message;

    let payload = crate::google_auth_proto::MigrationPayload {
        otp_parameters: params,
        version: 1,
        batch_size,
        batch_index,
        batch_id: 0,
    };

    let mut buf = Vec::new();
    payload
        .encode(&mut buf)
        .map_err(|e| format!("Failed to encode migration payload: {e}"))?;

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);
    let b64_encoded = b64
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D");
    Ok(format!("otpauth-migration://offline?data={b64_encoded}"))
}

#[tauri::command]
pub fn get_export_accounts(
    acknowledge_secret_export: bool,
    current_pin: Option<String>,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<ExportBatch>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    if !acknowledge_secret_export {
        return Err("Explicit secret export acknowledgment is required".to_string());
    }
    verify_export_reauth(&pin_manager, current_pin)?;
    let storage = lock_storage(&storage)?;
    let accounts = storage.list();
    let total = accounts.len();
    let batch_count = if total == 0 {
        0
    } else {
        total.div_ceil(EXPORT_BATCH_SIZE)
    };

    let mut batches = Vec::new();

    for (batch_index, chunk) in accounts.chunks(EXPORT_BATCH_SIZE).enumerate() {
        let mut otp_params = Vec::new();
        let mut account_infos = Vec::new();

        for account in chunk {
            otp_params.push(account_to_otp_params(account)?);
            account_infos.push(ExportAccountInfo {
                issuer: account.issuer.clone(),
                label: account.label.clone(),
            });
        }

        let migration_uri =
            build_migration_uri(otp_params, batch_count as i32, batch_index as i32)?;

        batches.push(ExportBatch {
            migration_uri,
            accounts: account_infos,
            batch_index,
            batch_count,
        });
    }

    tracing::info!(
        event = "export_qr_generated",
        accounts = total,
        batches = batch_count,
        "Export QR migration URIs generated"
    );
    Ok(batches)
}

fn verify_export_reauth(
    pin_manager: &PinManager,
    current_pin: Option<String>,
) -> Result<(), String> {
    if !pin_manager.has_pin() {
        return Ok(());
    }
    let mut pin = current_pin.ok_or_else(|| "Current PIN is required for export".to_string())?;
    let verified = pin_manager.verify_pin(&pin);
    pin.zeroize();
    let verified = verified?;
    if verified {
        Ok(())
    } else {
        Err("Incorrect current PIN".to_string())
    }
}

// --- External import commands ---

#[derive(Serialize)]
pub struct ImportPreview {
    pub format: String,
    pub accounts: Vec<AccountDisplay>,
    pub skipped: usize,
    pub duplicates: usize,
}

#[tauri::command]
pub fn decode_qr_from_image(data: Vec<u8>) -> Result<Vec<String>, String> {
    let img = image::load_from_memory(&data).map_err(|e| format!("Failed to decode image: {e}"))?;
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    // Try full image
    let r = try_rqrr(&gray);
    if !r.is_empty() {
        return Ok(r);
    }

    // Try center crops (helps when QR is a small part of a screenshot)
    for frac in [0.7f64, 0.5, 0.35] {
        let cw = (w as f64 * frac) as u32;
        let ch = (h as f64 * frac) as u32;
        if cw == 0 || ch == 0 {
            continue;
        }
        let cx = (w - cw) / 2;
        let cy = (h - ch) / 2;
        let cropped = image::imageops::crop_imm(&gray, cx, cy, cw, ch).to_image();
        let r = try_rqrr(&cropped);
        if !r.is_empty() {
            return Ok(r);
        }
    }

    // Try downscaled versions of full image
    for max_dim in [1024u32, 1536, 2048] {
        let scale = (max_dim as f64) / (w.max(h) as f64);
        if scale >= 1.0 {
            continue;
        }
        let sw = (w as f64 * scale).round() as u32;
        let sh = (h as f64 * scale).round() as u32;
        if sw == 0 || sh == 0 {
            continue;
        }
        let resized = image::imageops::resize(&gray, sw, sh, image::imageops::FilterType::Lanczos3);
        let r = try_rqrr(&resized);
        if !r.is_empty() {
            return Ok(r);
        }
    }

    Ok(vec![])
}

fn try_rqrr(gray: &image::GrayImage) -> Vec<String> {
    let mut prepared = rqrr::PreparedImage::prepare(gray.clone());
    let grids = prepared.detect_grids();
    let mut results = Vec::new();
    for grid in grids {
        if let Ok((_, content)) = grid.decode() {
            results.push(content);
        }
    }
    results
}

#[cfg(test)]
mod qr_tests {
    use super::decode_qr_from_image;

    #[test]
    fn decodes_real_ios_screenshot() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test-fixtures/qr-images");
        // Find the fixture regardless of extension (.png, .jpeg, .jpg)
        let fixture = ["png", "jpeg", "jpg"]
            .iter()
            .map(|ext| dir.join(format!("qr-code-test-ios.{ext}")))
            .find(|p| p.exists());
        let Some(fixture) = fixture else {
            eprintln!(
                "Skipping: no qr-code-test-ios fixture found in {}",
                dir.display()
            );
            return;
        };
        let data = std::fs::read(&fixture).unwrap();
        let stem = fixture.file_stem().unwrap().to_str().unwrap();
        let expected = {
            let p = dir.join(format!("{stem}.expected.txt"));
            std::fs::read_to_string(p).unwrap().trim().to_string()
        };
        let results = decode_qr_from_image(data).unwrap();
        assert!(!results.is_empty(), "No QR code detected in fixture image");
        assert_eq!(results[0], expected);
    }
}

#[tauri::command]
pub fn import_external_preview(
    data: Vec<u8>,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<ImportPreview, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let result = crate::import::parse_import(&data)?;
    let storage = lock_storage(&storage)?;
    let existing: Vec<(&str, &str, &str)> = storage
        .list()
        .iter()
        .map(|e| (e.issuer.as_str(), e.label.as_str(), e.secret.as_str()))
        .collect();

    let mut new_accounts = Vec::new();
    let mut duplicates = 0usize;
    for account in result.accounts {
        let is_dup = existing
            .iter()
            .any(|e| e.0 == account.issuer && e.1 == account.label && e.2 == account.secret);
        if is_dup {
            duplicates += 1;
        } else {
            new_accounts.push(AccountDisplay::from(account));
        }
    }

    Ok(ImportPreview {
        format: result.format,
        accounts: new_accounts,
        skipped: result.skipped,
        duplicates,
    })
}

#[tauri::command]
pub fn import_external_confirm(
    data: Vec<u8>,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<AccountDisplay>, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let result = crate::import::parse_import(&data)?;
    let mut storage = lock_storage(&storage)?;
    let added = deduplicate_and_import(result.accounts, &mut storage)?;
    icloud_push_if_enabled(&app_handle, &storage);
    tracing::info!(
        event = "external_import",
        format = %result.format,
        count = added.len(),
        "External import completed"
    );
    Ok(added)
}

// --- Account editing commands ---

#[tauri::command]
pub fn edit_account(
    id: String,
    issuer: String,
    label: String,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<AccountDisplay, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    if issuer.len() > 255 {
        return Err("Issuer name is too long (max 255 characters)".to_string());
    }
    if label.len() > 255 {
        return Err("Label is too long (max 255 characters)".to_string());
    }
    let mut storage = lock_storage(&storage)?;
    storage.update(&id, issuer, label)?;
    let account = storage
        .get(&id)
        .ok_or_else(|| "Account not found".to_string())?;
    let display = AccountDisplay::from(account.clone());
    icloud_push_if_enabled(&app_handle, &storage);
    Ok(display)
}

#[tauri::command]
pub fn reorder_accounts(
    ids: Vec<String>,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<(), String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let mut storage = lock_storage(&storage)?;
    storage.reorder(&ids)?;
    icloud_push_if_enabled(&app_handle, &storage);
    Ok(())
}

// --- PIN / auth commands ---

#[tauri::command]
pub fn auth_status(
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<AuthStatus, String> {
    let pin_enabled = pin_manager.has_pin();
    let auth = lock_auth(&auth_manager)?;
    let unlocked = if pin_enabled { auth.unlocked } else { true };
    let data_recovered = lock_storage(&storage)?.take_data_recovered();
    Ok(AuthStatus {
        pin_enabled,
        unlocked,
        last_unlock_epoch: auth.last_unlock_epoch,
        data_recovered,
    })
}

#[tauri::command]
pub fn unlock_with_pin(
    mut pin: String,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<bool, String> {
    let verified = pin_manager.verify_pin(&pin);
    pin.zeroize();
    let verified = verified?;
    if verified {
        let mut auth = lock_auth(&auth_manager)?;
        auth.unlocked = true;
        auth.last_unlock_epoch = Some(crate::storage::now_secs());
    }
    Ok(verified)
}

#[tauri::command]
pub fn unlock_with_recovery_code(
    mut code: String,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<bool, String> {
    let verified = pin_manager.verify_recovery_code(&code);
    code.zeroize();
    let verified = verified?;
    if verified {
        let mut auth = lock_auth(&auth_manager)?;
        auth.unlocked = true;
        auth.last_unlock_epoch = Some(crate::storage::now_secs());
    }
    Ok(verified)
}

#[tauri::command]
pub fn unlock_with_biometric(
    app_handle: tauri::AppHandle,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<bool, String> {
    if !pin_manager.has_pin() {
        return Ok(true);
    }
    verify_biometric_unlock(&app_handle)?;
    let mut auth = lock_auth(&auth_manager)?;
    auth.unlocked = true;
    auth.last_unlock_epoch = Some(crate::storage::now_secs());
    Ok(true)
}

#[cfg(mobile)]
fn verify_biometric_unlock(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_biometric::{AuthOptions, BiometricExt};

    let biometric = app_handle.biometric();
    let status = biometric.status().map_err(|e| e.to_string())?;
    if !status.is_available {
        return Err(status
            .error
            .unwrap_or_else(|| "Biometric authentication is unavailable".to_string()));
    }

    biometric
        .authenticate(
            "Unlock Ghost Auth".to_string(),
            AuthOptions {
                allow_device_credential: false,
                confirmation_required: Some(false),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())
}

#[cfg(not(mobile))]
fn verify_biometric_unlock(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Err("Biometric unlock is unavailable on this platform".to_string())
}

#[tauri::command]
pub fn lock_vault(auth_manager: State<AuthManager>) -> Result<(), String> {
    let mut auth = lock_auth(&auth_manager)?;
    auth.unlocked = false;
    Ok(())
}

#[tauri::command]
pub fn has_pin(pin_manager: State<PinManager>) -> bool {
    pin_manager.has_pin()
}

#[tauri::command]
pub fn set_pin(
    mut pin: String,
    current_pin: Option<String>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<String>, String> {
    // Defense-in-depth: verify current PIN at the backend level if one exists
    if pin_manager.has_pin() {
        let current = current_pin
            .as_deref()
            .ok_or_else(|| "Current PIN is required to change PIN".to_string())?;
        if !pin_manager.verify_pin(current)? {
            pin.zeroize();
            return Err("Incorrect current PIN".to_string());
        }
    }
    let result = pin_manager.set_pin(&pin);
    pin.zeroize();
    let codes = result?;
    {
        let mut auth = lock_auth(&auth_manager)?;
        auth.unlocked = true;
        auth.last_unlock_epoch = Some(crate::storage::now_secs());
    }
    tracing::info!(event = "pin_set", "PIN was set or updated");
    Ok(codes)
}

#[tauri::command]
pub fn verify_pin(mut pin: String, pin_manager: State<PinManager>) -> Result<bool, String> {
    let result = pin_manager.verify_pin(&pin);
    pin.zeroize();
    result
}

#[tauri::command]
pub fn remove_pin(
    mut pin: String,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<(), String> {
    // Verify current PIN before allowing removal
    let valid = pin_manager.verify_pin(&pin)?;
    pin.zeroize();
    if !valid {
        tracing::warn!(
            event = "pin_remove_failed",
            "PIN removal attempted with incorrect PIN"
        );
        return Err("Incorrect PIN".to_string());
    }
    pin_manager.remove_pin()?;
    {
        let mut auth = lock_auth(&auth_manager)?;
        auth.unlocked = false;
    }
    tracing::info!(event = "pin_removed", "PIN was removed");
    Ok(())
}

#[tauri::command]
pub fn verify_recovery_code(
    mut code: String,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<bool, String> {
    let result = pin_manager.verify_recovery_code(&code);
    code.zeroize();
    let verified = result?;
    if verified {
        let mut auth = lock_auth(&auth_manager)?;
        auth.unlocked = true;
        auth.last_unlock_epoch = Some(crate::storage::now_secs());
    }
    Ok(verified)
}

#[tauri::command]
pub fn has_recovery_codes(pin_manager: State<PinManager>) -> bool {
    pin_manager.has_recovery_codes()
}

// --- Sync commands ---

pub struct SyncManager {
    inner: Arc<Mutex<Option<ActiveSync>>>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

struct ActiveSync {
    session_id: String,
    created_at: Instant,
    phase: SyncPhase,
    pending: Option<PendingMerge>,
    cancel_signal: Arc<AtomicBool>,
}

#[derive(Clone)]
enum SyncPhase {
    Connecting,
    WaitingForPeer,
    Exchanging,
    MergeReady,
    #[allow(dead_code)]
    Completed,
    Failed(String),
}

struct PendingMerge {
    remote_device_id: String,
    merge_result: crate::sync::MergeResult,
    data_dir: PathBuf,
}

fn cancel_active_sync(state: &mut Option<ActiveSync>) {
    if let Some(active) = state.take() {
        active.cancel_signal.store(true, Ordering::SeqCst);
    }
}

#[derive(Serialize, Clone)]
pub struct SyncSessionInfo {
    pub session_id: String,
    pub qr_data: String,
    pub text_code: String,
    pub host: Option<String>,
    pub all_hosts: Vec<String>,
    pub port: u16,
    pub expires_in: u64,
}

#[derive(Serialize, Clone)]
pub struct SyncPollResult {
    pub status: String,
    pub merge_preview: Option<MergePreview>,
    pub error: Option<String>,
    pub expires_in: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct MergePreview {
    pub to_add: Vec<AccountDisplay>,
    pub conflicts: Vec<ConflictDisplay>,
    pub to_delete: Vec<AccountDisplay>,
    pub auto_updated: Vec<AccountDisplay>,
    pub unchanged: usize,
}

#[derive(Serialize, Clone)]
pub struct ConflictDisplay {
    pub account_id: String,
    pub local: AccountDisplay,
    pub remote: AccountDisplay,
}

#[derive(Deserialize)]
pub struct MergeDecision {
    pub account_id: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct SyncConfirmResult {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

#[derive(Serialize)]
pub struct SyncPeerInfo {
    pub device_id: String,
    pub last_synced: u64,
}

fn merge_result_to_preview(result: &crate::sync::MergeResult) -> MergePreview {
    MergePreview {
        to_add: result
            .to_add
            .iter()
            .cloned()
            .map(AccountDisplay::from)
            .collect(),
        conflicts: result
            .conflicts
            .iter()
            .map(|c| ConflictDisplay {
                account_id: c.local.id.clone(),
                local: AccountDisplay::from(c.local.clone()),
                remote: AccountDisplay::from(c.remote.clone()),
            })
            .collect(),
        to_delete: result
            .remote_deletions
            .iter()
            .cloned()
            .map(AccountDisplay::from)
            .collect(),
        auto_updated: result
            .auto_updated
            .iter()
            .cloned()
            .map(AccountDisplay::from)
            .collect(),
        unchanged: result.unchanged,
    }
}

#[tauri::command]
pub fn sync_start(
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
    app_handle: tauri::AppHandle,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<SyncSessionInfo, String> {
    use tauri::Manager;
    ensure_unlocked(&auth_manager, &pin_manager)?;

    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    if state.is_some() {
        return Err("A sync session is already active — cancel it first".to_string());
    }

    let session = crate::sync::SyncSession::new();
    let listener = crate::sync_transport::SyncListener::bind()?;
    let port = listener.port();
    let primary_host = listener.ip();
    let all_hosts = crate::sync_transport::local_ips();
    let host = Some(primary_host);

    // Snapshot storage state (release lock before spawning thread)
    let storage_guard = lock_storage(&storage)?;
    let device_id = storage_guard.device_id().to_string();
    let accounts = storage_guard.list().to_vec();
    let tombstones = storage_guard.tombstones().to_vec();
    let storage_key: [u8; 32] = *storage_guard.encryption_key();
    drop(storage_guard);

    let key = *session.key();
    let local_payload = crate::sync::build_payload(&device_id, &accounts, &tombstones, &key)?;

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let hosts_csv = all_hosts.join(",");
    let qr_data = format!(
        "ghost-auth://sync?code={}&host={}&hosts={}&port={}",
        session.code.replace('-', ""),
        host.as_deref().unwrap_or(""),
        hosts_csv,
        port
    );

    let info = SyncSessionInfo {
        session_id: session.id.clone(),
        qr_data,
        text_code: session.code.clone(),
        host,
        all_hosts,
        port,
        expires_in: session.remaining_secs(),
    };

    let session_id = session.id.clone();
    let shared = sync_state.inner.clone();
    let created_at = Instant::now();
    let session_deadline = created_at + Duration::from_secs(crate::sync::CODE_EXPIRY_SECS);
    let cancel_signal = Arc::new(AtomicBool::new(false));

    *state = Some(ActiveSync {
        session_id: session_id.clone(),
        created_at,
        phase: SyncPhase::WaitingForPeer,
        pending: None,
        cancel_signal: cancel_signal.clone(),
    });
    drop(state);

    tracing::info!(
        event = "sync_started",
        port = port,
        "Sync session started (initiator)"
    );

    // Background thread: accept connection (auto-detects TCP vs WebSocket),
    // exchange payloads, compute merge
    std::thread::spawn(move || {
        let mut conn = match listener.accept_any_cancellable(&key, || {
            cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline
        }) {
            Ok(c) => c,
            Err(e) => {
                if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
                    return;
                }
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.phase = SyncPhase::Exchanging;
        }

        // Joiner sends first, so initiator receives first
        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }
        let remote_payload = match conn.recv_payload() {
            Ok(p) => p,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        if let Err(e) = conn.send_payload(&local_payload) {
            if let Ok(mut s) = shared.lock()
                && let Some(ref mut a) = *s
                && a.session_id == session_id
            {
                a.phase = SyncPhase::Failed(e);
            }
            return;
        }

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        conn.close();

        // Decrypt remote accounts
        let remote_accounts: Result<Vec<_>, _> = remote_payload
            .accounts
            .iter()
            .map(|enc| crate::sync::decrypt_account(enc, &key))
            .collect();
        let remote_accounts = match remote_accounts {
            Ok(a) => a,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            return;
        }

        let history = crate::sync::SyncHistory::load(&data_dir, &storage_key);
        let last_sync = history.last_sync_with(&remote_payload.device_id);

        let merge_result = crate::sync::merge(
            &accounts,
            &tombstones,
            remote_accounts,
            &remote_payload.tombstones,
            last_sync,
        );

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            return;
        }

        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.pending = Some(PendingMerge {
                remote_device_id: remote_payload.device_id,
                merge_result,
                data_dir,
            });
            a.phase = SyncPhase::MergeReady;
        }

        tracing::info!(
            event = "sync_exchange_complete",
            "Sync exchange completed (initiator)"
        );
    });

    Ok(info)
}

#[tauri::command]
pub fn sync_start_with_key(
    key: Vec<u8>,
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
    app_handle: tauri::AppHandle,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<SyncSessionInfo, String> {
    use tauri::Manager;
    ensure_unlocked(&auth_manager, &pin_manager)?;

    if key.len() != 32 {
        return Err("Key must be 32 bytes".to_string());
    }
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);

    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    if state.is_some() {
        return Err("A sync session is already active — cancel it first".to_string());
    }

    let listener = crate::sync_transport::SyncListener::bind()?;
    let port = listener.port();
    let primary_host = listener.ip();
    let all_hosts = crate::sync_transport::local_ips();
    let host = Some(primary_host);

    // Snapshot storage state (release lock before spawning thread)
    let storage_guard = lock_storage(&storage)?;
    let device_id = storage_guard.device_id().to_string();
    let accounts = storage_guard.list().to_vec();
    let tombstones = storage_guard.tombstones().to_vec();
    let storage_key: [u8; 32] = *storage_guard.encryption_key();
    drop(storage_guard);

    let local_payload = crate::sync::build_payload(&device_id, &accounts, &tombstones, &key_arr)?;

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let hex_key: String = key.iter().map(|b| format!("{:02x}", b)).collect();
    let hosts_csv = all_hosts.join(",");
    let qr_data = format!(
        "ghost-auth://qr-sync?key={}&hosts={}&port={}",
        hex_key, hosts_csv, port
    );

    let session_id = uuid::Uuid::new_v4().to_string();

    let info = SyncSessionInfo {
        session_id: session_id.clone(),
        qr_data,
        text_code: String::new(),
        host,
        all_hosts,
        port,
        expires_in: crate::sync::CODE_EXPIRY_SECS,
    };

    let shared = sync_state.inner.clone();
    let created_at = Instant::now();
    let session_deadline = created_at + Duration::from_secs(crate::sync::CODE_EXPIRY_SECS);
    let cancel_signal = Arc::new(AtomicBool::new(false));

    *state = Some(ActiveSync {
        session_id: session_id.clone(),
        created_at,
        phase: SyncPhase::WaitingForPeer,
        pending: None,
        cancel_signal: cancel_signal.clone(),
    });
    drop(state);

    tracing::info!(
        event = "sync_started_with_key",
        port = port,
        "Sync session started with pre-shared key (initiator)"
    );

    // Background thread: accept connection (auto-detects TCP vs WebSocket),
    // exchange payloads, compute merge
    std::thread::spawn(move || {
        let mut conn = match listener.accept_any_cancellable(&key_arr, || {
            cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline
        }) {
            Ok(c) => c,
            Err(e) => {
                if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
                    return;
                }
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.phase = SyncPhase::Exchanging;
        }

        // Joiner sends first, so initiator receives first
        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }
        let remote_payload = match conn.recv_payload() {
            Ok(p) => p,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        if let Err(e) = conn.send_payload(&local_payload) {
            if let Ok(mut s) = shared.lock()
                && let Some(ref mut a) = *s
                && a.session_id == session_id
            {
                a.phase = SyncPhase::Failed(e);
            }
            return;
        }

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            conn.close();
            return;
        }

        conn.close();

        // Decrypt remote accounts
        let remote_accounts: Result<Vec<_>, _> = remote_payload
            .accounts
            .iter()
            .map(|enc| crate::sync::decrypt_account(enc, &key_arr))
            .collect();
        let remote_accounts = match remote_accounts {
            Ok(a) => a,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            return;
        }

        let history = crate::sync::SyncHistory::load(&data_dir, &storage_key);
        let last_sync = history.last_sync_with(&remote_payload.device_id);

        let merge_result = crate::sync::merge(
            &accounts,
            &tombstones,
            remote_accounts,
            &remote_payload.tombstones,
            last_sync,
        );

        if cancel_signal.load(Ordering::SeqCst) || Instant::now() >= session_deadline {
            return;
        }

        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.pending = Some(PendingMerge {
                remote_device_id: remote_payload.device_id,
                merge_result,
                data_dir,
            });
            a.phase = SyncPhase::MergeReady;
        }

        tracing::info!(
            event = "sync_exchange_complete",
            "Sync exchange completed (initiator, pre-shared key)"
        );
    });

    Ok(info)
}

#[tauri::command]
pub fn sync_poll(sync_state: State<SyncManager>) -> Result<SyncPollResult, String> {
    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;

    let (phase, merge_preview, expires_in) = {
        let active = state
            .as_ref()
            .ok_or_else(|| "No active sync session".to_string())?;
        let elapsed = active.created_at.elapsed().as_secs();
        let expires_in = crate::sync::CODE_EXPIRY_SECS.saturating_sub(elapsed);
        let merge_preview = active
            .pending
            .as_ref()
            .map(|p| merge_result_to_preview(&p.merge_result));
        (active.phase.clone(), merge_preview, expires_in)
    };

    if matches!(
        phase,
        SyncPhase::Connecting | SyncPhase::WaitingForPeer | SyncPhase::Exchanging
    ) && expires_in == 0
    {
        cancel_active_sync(&mut state);
        return Ok(SyncPollResult {
            status: "error".into(),
            merge_preview: None,
            error: Some("Sync session expired".to_string()),
            expires_in: Some(0),
        });
    }

    match phase {
        SyncPhase::Connecting => Ok(SyncPollResult {
            status: "connecting".into(),
            merge_preview: None,
            error: None,
            expires_in: Some(expires_in),
        }),
        SyncPhase::WaitingForPeer => Ok(SyncPollResult {
            status: "waiting".into(),
            merge_preview: None,
            error: None,
            expires_in: Some(expires_in),
        }),
        SyncPhase::Exchanging => Ok(SyncPollResult {
            status: "exchanging".into(),
            merge_preview: None,
            error: None,
            expires_in: Some(expires_in),
        }),
        SyncPhase::MergeReady => Ok(SyncPollResult {
            status: "merge_ready".into(),
            merge_preview,
            error: None,
            expires_in: Some(expires_in),
        }),
        SyncPhase::Completed => Ok(SyncPollResult {
            status: "completed".into(),
            merge_preview: None,
            error: None,
            expires_in: Some(expires_in),
        }),
        SyncPhase::Failed(e) => Ok(SyncPollResult {
            status: "error".into(),
            merge_preview: None,
            error: Some(e),
            expires_in: Some(expires_in),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn sync_join(
    code: String,
    hosts: Vec<String>,
    port: u16,
    allow_public_host: bool,
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
    app_handle: tauri::AppHandle,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<(), String> {
    use tauri::Manager;
    ensure_unlocked(&auth_manager, &pin_manager)?;

    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    if state.is_some() {
        return Err("A sync session is already active — cancel it first".to_string());
    }

    if hosts.is_empty() {
        return Err("At least one host address is required".to_string());
    }

    let key = crate::sync::SyncSession::key_from_code(&code)?;

    // Snapshot storage (release lock before spawning thread)
    let storage_guard = lock_storage(&storage)?;
    let device_id = storage_guard.device_id().to_string();
    let accounts = storage_guard.list().to_vec();
    let tombstones = storage_guard.tombstones().to_vec();
    let storage_key: [u8; 32] = *storage_guard.encryption_key();
    drop(storage_guard);

    let local_payload = crate::sync::build_payload(&device_id, &accounts, &tombstones, &key)?;

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let shared = sync_state.inner.clone();
    let created_at = Instant::now();
    let cancel_signal = Arc::new(AtomicBool::new(false));

    *state = Some(ActiveSync {
        session_id: session_id.clone(),
        created_at,
        phase: SyncPhase::Connecting,
        pending: None,
        cancel_signal: cancel_signal.clone(),
    });
    drop(state);

    tracing::info!(
        event = "sync_join_started",
        hosts = ?hosts,
        port = port,
        "Sync join started (joiner, non-blocking)"
    );

    // Background thread: connect to one of the hosts, exchange payloads, compute merge
    std::thread::spawn(move || {
        // --- Phase: Connecting — try each host ---
        let mut conn = None;
        let mut last_error = String::new();

        for host in &hosts {
            if cancel_signal.load(Ordering::SeqCst) {
                return;
            }
            match crate::sync_transport::connect(host, port, &key, allow_public_host) {
                Ok(c) => {
                    conn = Some(c);
                    break;
                }
                Err(e) => {
                    last_error = e;
                }
            }
        }

        let mut conn = match conn {
            Some(c) => c,
            None => {
                if cancel_signal.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(last_error);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) {
            conn.close();
            return;
        }

        // --- Phase: Exchanging ---
        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.phase = SyncPhase::Exchanging;
        }

        // Joiner sends first
        if let Err(e) = conn.send_payload(&local_payload) {
            if let Ok(mut s) = shared.lock()
                && let Some(ref mut a) = *s
                && a.session_id == session_id
            {
                a.phase = SyncPhase::Failed(e);
            }
            return;
        }

        if cancel_signal.load(Ordering::SeqCst) {
            conn.close();
            return;
        }

        let remote_payload = match conn.recv_payload() {
            Ok(p) => p,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        conn.close();

        if cancel_signal.load(Ordering::SeqCst) {
            return;
        }

        // Decrypt remote accounts
        let remote_accounts: Result<Vec<_>, _> = remote_payload
            .accounts
            .iter()
            .map(|enc| crate::sync::decrypt_account(enc, &key))
            .collect();
        let remote_accounts = match remote_accounts {
            Ok(a) => a,
            Err(e) => {
                if let Ok(mut s) = shared.lock()
                    && let Some(ref mut a) = *s
                    && a.session_id == session_id
                {
                    a.phase = SyncPhase::Failed(e);
                }
                return;
            }
        };

        if cancel_signal.load(Ordering::SeqCst) {
            return;
        }

        let history = crate::sync::SyncHistory::load(&data_dir, &storage_key);
        let last_sync = history.last_sync_with(&remote_payload.device_id);

        let merge_result = crate::sync::merge(
            &accounts,
            &tombstones,
            remote_accounts,
            &remote_payload.tombstones,
            last_sync,
        );

        if cancel_signal.load(Ordering::SeqCst) {
            return;
        }

        // --- Phase: MergeReady ---
        if let Ok(mut s) = shared.lock()
            && let Some(ref mut a) = *s
            && a.session_id == session_id
        {
            a.pending = Some(PendingMerge {
                remote_device_id: remote_payload.device_id,
                merge_result,
                data_dir,
            });
            a.phase = SyncPhase::MergeReady;
        }

        tracing::info!(
            event = "sync_exchange_complete",
            "Sync exchange completed (joiner)"
        );
    });

    Ok(())
}

#[tauri::command]
pub fn sync_confirm(
    decisions: Vec<MergeDecision>,
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<SyncConfirmResult, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let pending = {
        let mut state = sync_state
            .inner
            .lock()
            .map_err(|_| "Sync state unavailable".to_string())?;
        let active = state
            .as_mut()
            .ok_or_else(|| "No active sync session".to_string())?;
        active
            .pending
            .take()
            .ok_or_else(|| "No pending merge to confirm".to_string())?
    };

    let PendingMerge {
        remote_device_id,
        merge_result,
        data_dir,
    } = pending;

    let crate::sync::MergeResult {
        to_add,
        conflicts,
        remote_deletions,
        auto_updated,
        unchanged: _,
    } = merge_result;

    let mut storage = lock_storage(&storage)?;

    let mut added = 0usize;
    let mut updated = 0usize;
    let mut deleted = 0usize;

    // Auto-add new accounts from remote
    for account in to_add {
        storage.add_synced(account)?;
        added += 1;
    }

    // Auto-update accounts where remote is newer
    for account in auto_updated {
        storage.replace_account(account)?;
        updated += 1;
    }

    // Apply user decisions for conflicts and deletions
    let decision_map: std::collections::HashMap<&str, &str> = decisions
        .iter()
        .map(|d| (d.account_id.as_str(), d.action.as_str()))
        .collect();

    for conflict in &conflicts {
        match decision_map.get(conflict.local.id.as_str()) {
            Some(&"keep_remote") => {
                storage.replace_account(conflict.remote.clone())?;
                updated += 1;
            }
            Some(&"delete") => {
                storage.delete(&conflict.local.id)?;
                deleted += 1;
            }
            _ => {} // keep_local or unspecified — keep local version
        }
    }

    for account in &remote_deletions {
        if let Some(&"delete") = decision_map.get(account.id.as_str()) {
            storage.delete(&account.id)?;
            deleted += 1;
        }
    }

    icloud_push_if_enabled(&app_handle, &storage);

    let storage_key: [u8; 32] = *storage.encryption_key();
    drop(storage);

    // Record sync in history
    let mut history = crate::sync::SyncHistory::load(&data_dir, &storage_key);
    history.record_sync(&remote_device_id, crate::storage::now_secs());
    if let Err(e) = history.save(&data_dir, &storage_key) {
        tracing::warn!(error = %e, "Failed to save sync history");
    }

    // Clear sync state
    if let Ok(mut state) = sync_state.inner.lock() {
        cancel_active_sync(&mut state);
    }

    tracing::info!(
        event = "sync_confirmed",
        added = added,
        updated = updated,
        deleted = deleted,
        "Sync merge applied"
    );

    Ok(SyncConfirmResult {
        added,
        updated,
        deleted,
    })
}

#[tauri::command]
pub fn probe_local_network() -> Result<String, String> {
    let ips = crate::sync_transport::local_ips();
    let Some(ip) = ips.first() else {
        return Err("no_network".into());
    };

    if cfg!(target_os = "ios") {
        // On iOS, we need a real LAN operation to trigger (and later verify)
        // the Local Network permission dialog.  TcpListener::bind only
        // touches the device's own interface and never prompts, and UDP
        // send_to returns Ok immediately (fire-and-forget) so it can't
        // detect the result.
        //
        // TCP-connect to the likely gateway (last-octet .1) on port 80:
        //   • TimedOut / ConnectionRefused / ConnectionReset
        //       → traffic reached the network → permission granted
        //   • Fast error (ENETDOWN, EPERM, etc.)
        //       → blocked by iOS → permission denied (or dialog still pending)
        //
        // The frontend polls this command, so each call is a single check.
        let gateway = match ip.rfind('.') {
            Some(pos) => format!("{}.1", &ip[..pos]),
            None => return Err("no_network".into()),
        };
        let target: std::net::SocketAddr = format!("{}:80", gateway)
            .parse()
            .map_err(|_| "no_network".to_string())?;

        match std::net::TcpStream::connect_timeout(&target, Duration::from_millis(500)) {
            Ok(_) => {
                tracing::info!(event = "probe_local_network_ok", ip = %ip, gateway = %gateway);
                Ok("ok".into())
            }
            Err(e) => {
                let kind = e.kind();
                if matches!(
                    kind,
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionReset
                ) {
                    tracing::info!(event = "probe_local_network_ok", ip = %ip, gateway = %gateway, error_kind = ?kind);
                    Ok("ok".into())
                } else {
                    tracing::warn!(event = "probe_local_network_denied", ip = %ip, gateway = %gateway, error = %e, kind = ?kind);
                    Err("permission_denied".into())
                }
            }
        }
    } else {
        // Non-iOS: a successful bind is enough to confirm network presence.
        let addr = format!("{}:0", ip);
        if std::net::TcpListener::bind(&addr).is_ok() {
            tracing::info!(event = "probe_local_network_ok", ip = %ip);
            Ok("ok".into())
        } else {
            tracing::warn!(event = "probe_local_network_denied", ip = %ip);
            Err("permission_denied".into())
        }
    }
}

#[tauri::command]
pub fn sync_cancel(
    sync_state: State<SyncManager>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<(), String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    cancel_active_sync(&mut state);
    tracing::info!(event = "sync_cancelled", "Sync session cancelled");
    Ok(())
}

#[tauri::command]
pub fn sync_history(
    app_handle: tauri::AppHandle,
    storage: State<Mutex<Storage>>,
    pin_manager: State<PinManager>,
    auth_manager: State<AuthManager>,
) -> Result<Vec<SyncPeerInfo>, String> {
    use tauri::Manager;
    ensure_unlocked(&auth_manager, &pin_manager)?;
    let storage_guard = lock_storage(&storage)?;
    let storage_key: [u8; 32] = *storage_guard.encryption_key();
    drop(storage_guard);
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let history = crate::sync::SyncHistory::load(&data_dir, &storage_key);
    Ok(history
        .peers
        .into_iter()
        .map(|(device_id, last_synced)| SyncPeerInfo {
            device_id,
            last_synced,
        })
        .collect())
}

#[cfg(target_os = "android")]
fn apply_android_system_theme(theme: &str) -> Result<(), String> {
    use jni::JavaVM;
    use jni::objects::{JObject, JValue};

    fn jni_err(e: impl std::fmt::Display) -> String {
        format!("JNI error: {e}")
    }

    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread_as_daemon()
        .map_err(|e| format!("Failed to attach JNI thread: {e}"))?;

    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    let normalized = if theme.trim().eq_ignore_ascii_case("dark") {
        "dark"
    } else {
        "light"
    };
    let theme_obj: JObject = env.new_string(normalized).map_err(jni_err)?.into();

    env.call_method(
        &activity,
        "applyThemeFromJs",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&theme_obj)],
    )
    .map_err(jni_err)?;

    Ok(())
}

#[cfg(target_os = "android")]
fn read_android_font_scale() -> Result<f64, String> {
    use jni::JavaVM;
    use jni::objects::JObject;

    fn jni_err(e: impl std::fmt::Display) -> String {
        format!("JNI error: {e}")
    }

    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread_as_daemon()
        .map_err(|e| format!("Failed to attach JNI thread: {e}"))?;

    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    // activity.getResources().getConfiguration().fontScale
    let resources = env
        .call_method(
            &activity,
            "getResources",
            "()Landroid/content/res/Resources;",
            &[],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;

    let configuration = env
        .call_method(
            &resources,
            "getConfiguration",
            "()Landroid/content/res/Configuration;",
            &[],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;

    let font_scale = env
        .get_field(&configuration, "fontScale", "F")
        .map_err(jni_err)?
        .f()
        .map_err(jni_err)?;

    Ok(font_scale as f64)
}

#[tauri::command]
pub fn get_font_scale() -> f64 {
    #[cfg(target_os = "android")]
    {
        read_android_font_scale().unwrap_or(1.0)
    }
    #[cfg(not(target_os = "android"))]
    {
        1.0
    }
}

#[tauri::command]
pub fn get_biometric_preference(app_handle: tauri::AppHandle) -> Result<bool, String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    let path = data_dir.join("biometric_enabled");
    match std::fs::read_to_string(&path) {
        Ok(val) => Ok(val.trim() == "true"),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub fn set_biometric_preference(app_handle: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    let path = data_dir.join("biometric_enabled");
    if enabled {
        std::fs::write(&path, "true")
            .map_err(|e| format!("Failed to save biometric preference: {e}"))?;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

#[tauri::command]
pub fn save_theme(app_handle: tauri::AppHandle, theme: String) -> Result<(), String> {
    use tauri::Manager;
    let normalized_theme = if theme.trim().eq_ignore_ascii_case("dark") {
        "dark"
    } else {
        "light"
    };
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    std::fs::write(data_dir.join("theme"), normalized_theme.as_bytes())
        .map_err(|e| format!("Failed to save theme: {e}"))?;

    #[cfg(target_os = "android")]
    if let Err(e) = apply_android_system_theme(normalized_theme) {
        tracing::warn!(error = %e, "Failed to apply Android system bar theme override");
    }

    Ok(())
}

#[tauri::command]
pub fn get_crash_reporting_preference() -> bool {
    crate::crash_reporter::is_enabled()
}

#[tauri::command]
pub fn set_crash_reporting_preference(enabled: bool) {
    crate::crash_reporter::set_enabled(enabled);
}

#[tauri::command]
pub fn send_test_crash_report() -> Result<(), String> {
    crate::crash_reporter::send_test_report()
}

// ── iCloud Sync ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ICloudSyncStatus {
    available: bool,
    enabled: bool,
    /// Unix timestamp (seconds) of last successful sync, or 0 if never.
    last_synced_at: u64,
}

#[derive(Serialize)]
pub struct ICloudEnableResult {
    added: usize,
    updated: usize,
    deleted: usize,
}

#[derive(Serialize)]
pub struct ICloudMergeStatus {
    added: usize,
    updated: usize,
    deleted: usize,
}

#[tauri::command]
pub async fn icloud_sync_status(app_handle: tauri::AppHandle) -> Result<ICloudSyncStatus, String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    let enabled = crate::icloud::is_enabled(&data_dir);

    // Check iCloud availability via the Swift plugin (iOS only).
    #[cfg(target_os = "ios")]
    let available = {
        use tauri_plugin_icloud_sync::ICloudSyncExt;
        app_handle.icloud_sync().check_available().unwrap_or(false)
    };
    #[cfg(not(target_os = "ios"))]
    let available = false;

    let last_synced_at = crate::icloud::last_synced_at(&data_dir);
    Ok(ICloudSyncStatus {
        available,
        enabled,
        last_synced_at,
    })
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn icloud_sync_enable(
    app_handle: tauri::AppHandle,
    storage: State<'_, Mutex<Storage>>,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
) -> Result<ICloudEnableResult, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;

    #[cfg(not(target_os = "ios"))]
    return Err("iCloud sync is only available on iOS".to_string());

    #[cfg(target_os = "ios")]
    {
        use tauri::Manager;
        use tauri_plugin_icloud_sync::ICloudSyncExt;

        let plugin = app_handle.icloud_sync();

        // 1. Ensure iCloud is available
        if !plugin.check_available().unwrap_or(false) {
            return Err(
                "iCloud is not available. Sign into iCloud in your device settings.".to_string(),
            );
        }

        // 2. Obtain or create the cloud sync key
        let cloud_key = match crate::icloud::load_cloud_key() {
            Some(key) => key,
            None => {
                // Check iCloud Keychain for a key from another device
                match plugin.load_cloud_key()? {
                    Some(mut bytes) => {
                        let key = crate::icloud::store_cloud_key_locally(&bytes)?;
                        bytes.fill(0); // zeroize plugin transport buffer
                        key
                    }
                    None => {
                        // First device: generate and store everywhere
                        let key = crate::icloud::generate_and_store_cloud_key()?;
                        plugin.store_cloud_key(key.as_ref())?;
                        key
                    }
                }
            }
        };

        // 3. Pull existing vault from iCloud and merge (if another device already pushed).
        //    read_blob/decrypt failures are non-fatal (first-time enable, or key not
        //    yet synced), but a successful decrypt that fails to merge must abort.
        let mut added = 0;
        let mut updated = 0;
        let mut deleted = 0;
        if let Ok(Some(remote_blob)) = plugin.read_blob() {
            if let Ok((remote_device_id, remote_accounts, remote_tombstones)) =
                crate::icloud::decrypt_from_cloud(&remote_blob, &cloud_key)
            {
                let mut storage = lock_storage(&storage)?;
                if remote_device_id != storage.device_id() {
                    let result = crate::icloud::merge_cloud_payload(
                        &mut storage,
                        &remote_device_id,
                        remote_accounts,
                        &remote_tombstones,
                    )?;
                    added = result.added;
                    updated = result.updated;
                    deleted = result.deleted;
                }
            }
        }

        // 4. Encrypt merged vault (hold lock only during read + encrypt)
        let blob = {
            let storage = lock_storage(&storage)?;
            crate::icloud::encrypt_for_cloud(
                storage.list(),
                storage.tombstones(),
                storage.device_id(),
                &cloud_key,
            )?
            // storage lock released here
        };

        // 5. Push merged vault to iCloud (no lock held — this may be slow)
        plugin.write_blob(&blob)?;

        // 6. Start watching for remote changes
        plugin.start_watching()?;

        // 7. Persist preference and record sync timestamp
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|_| "Failed to resolve data directory".to_string())?;
        crate::icloud::set_enabled(&data_dir, true)?;
        crate::icloud::touch_last_synced(&data_dir);

        tracing::info!(event = "icloud_sync_enabled", "iCloud sync enabled");
        Ok(ICloudEnableResult {
            added,
            updated,
            deleted,
        })
    }
}

#[tauri::command]
pub async fn icloud_sync_disable(
    app_handle: tauri::AppHandle,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
) -> Result<(), String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;

    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_icloud_sync::ICloudSyncExt;
        let _ = app_handle.icloud_sync().stop_watching();
    }

    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    crate::icloud::set_enabled(&data_dir, false)?;

    tracing::info!(event = "icloud_sync_disabled", "iCloud sync disabled");
    Ok(())
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn icloud_sync_merge(
    blob_b64: String,
    app_handle: tauri::AppHandle,
    storage: State<'_, Mutex<Storage>>,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
) -> Result<ICloudMergeStatus, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;

    #[cfg(not(target_os = "ios"))]
    return Err("iCloud sync is only available on iOS".to_string());

    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_icloud_sync::ICloudSyncExt;

        let cloud_key = crate::icloud::load_cloud_key()
            .ok_or_else(|| "Cloud sync key not found — re-enable iCloud sync".to_string())?;

        let blob = base64::engine::general_purpose::STANDARD
            .decode(&blob_b64)
            .map_err(|e| format!("Invalid cloud blob data: {e}"))?;

        let (remote_device_id, remote_accounts, remote_tombstones) =
            crate::icloud::decrypt_from_cloud(&blob, &cloud_key)?;

        let mut storage = lock_storage(&storage)?;

        // Skip merge if the blob came from this device (echo)
        if remote_device_id == storage.device_id() {
            return Ok(ICloudMergeStatus {
                added: 0,
                updated: 0,
                deleted: 0,
            });
        }

        let result = crate::icloud::merge_cloud_payload(
            &mut storage,
            &remote_device_id,
            remote_accounts,
            &remote_tombstones,
        )?;

        // Encrypt re-push blob while lock is held (fast), then release lock before I/O.
        let repush_blob = if result.added > 0 || result.updated > 0 || result.deleted > 0 {
            Some(crate::icloud::encrypt_for_cloud(
                storage.list(),
                storage.tombstones(),
                storage.device_id(),
                &cloud_key,
            )?)
        } else {
            None
        };
        drop(storage);

        if let Some(blob) = repush_blob {
            let _ = app_handle.icloud_sync().write_blob(&blob);
        }

        tracing::info!(
            event = "icloud_merge",
            added = result.added,
            updated = result.updated,
            deleted = result.deleted,
            "iCloud merge applied"
        );

        Ok(ICloudMergeStatus {
            added: result.added,
            updated: result.updated,
            deleted: result.deleted,
        })
    }
}

/// Pull and merge the latest vault from iCloud.
/// Called when the frontend receives an icloud-change event.
#[tauri::command]
#[allow(unused_variables)]
pub async fn icloud_sync_pull(
    app_handle: tauri::AppHandle,
    storage: State<'_, Mutex<Storage>>,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
) -> Result<ICloudMergeStatus, String> {
    ensure_unlocked(&auth_manager, &pin_manager)?;

    #[cfg(not(target_os = "ios"))]
    return Ok(ICloudMergeStatus {
        added: 0,
        updated: 0,
        deleted: 0,
    });

    #[cfg(target_os = "ios")]
    {
        use tauri::Manager;
        use tauri_plugin_icloud_sync::ICloudSyncExt;

        let cloud_key = crate::icloud::load_cloud_key()
            .ok_or_else(|| "Cloud sync key not found — re-enable iCloud sync".to_string())?;

        let blob = app_handle
            .icloud_sync()
            .read_blob()?
            .ok_or_else(|| "No cloud vault found".to_string())?;

        let (remote_device_id, remote_accounts, remote_tombstones) =
            crate::icloud::decrypt_from_cloud(&blob, &cloud_key)?;

        let mut storage = lock_storage(&storage)?;

        if remote_device_id == storage.device_id() {
            return Ok(ICloudMergeStatus {
                added: 0,
                updated: 0,
                deleted: 0,
            });
        }

        let result = crate::icloud::merge_cloud_payload(
            &mut storage,
            &remote_device_id,
            remote_accounts,
            &remote_tombstones,
        )?;

        // Encrypt re-push blob while lock is held (fast), then release lock before I/O.
        let repush_blob = if result.added > 0 || result.updated > 0 || result.deleted > 0 {
            Some(crate::icloud::encrypt_for_cloud(
                storage.list(),
                storage.tombstones(),
                storage.device_id(),
                &cloud_key,
            )?)
        } else {
            None
        };
        drop(storage);

        if let Some(blob) = repush_blob {
            let _ = app_handle.icloud_sync().write_blob(&blob);
        }

        if let Ok(data_dir) = app_handle.path().app_data_dir() {
            crate::icloud::touch_last_synced(&data_dir);
        }

        tracing::info!(
            event = "icloud_pull",
            added = result.added,
            updated = result.updated,
            deleted = result.deleted,
            "iCloud pull-and-merge completed"
        );

        Ok(ICloudMergeStatus {
            added: result.added,
            updated: result.updated,
            deleted: result.deleted,
        })
    }
}

/// Resume iCloud sync after app (re)launch: restart the watcher and pull
/// any remote changes. Safe to call even when locked — skips the pull but
/// still restarts the watcher so events arrive once the user unlocks.
#[tauri::command]
#[allow(unused_variables)]
pub async fn icloud_sync_resume(
    app_handle: tauri::AppHandle,
    storage: State<'_, Mutex<Storage>>,
    pin_manager: State<'_, PinManager>,
    auth_manager: State<'_, AuthManager>,
) -> Result<ICloudMergeStatus, String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    if !crate::icloud::is_enabled(&data_dir) {
        return Ok(ICloudMergeStatus {
            added: 0,
            updated: 0,
            deleted: 0,
        });
    }

    // Restart the watcher unconditionally (even if locked — so events queue up).
    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_icloud_sync::ICloudSyncExt;
        let _ = app_handle.icloud_sync().start_watching();
    }

    // Pull only if unlocked. If locked, the pull will happen after unlock via
    // the pending-pull mechanism in the frontend.
    let unlocked = if pin_manager.has_pin() {
        lock_auth(&auth_manager)?.unlocked
    } else {
        true
    };

    if !unlocked {
        return Ok(ICloudMergeStatus {
            added: 0,
            updated: 0,
            deleted: 0,
        });
    }

    #[cfg(not(target_os = "ios"))]
    return Ok(ICloudMergeStatus {
        added: 0,
        updated: 0,
        deleted: 0,
    });

    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_icloud_sync::ICloudSyncExt;

        let cloud_key = match crate::icloud::load_cloud_key() {
            Some(k) => k,
            None => {
                return Ok(ICloudMergeStatus {
                    added: 0,
                    updated: 0,
                    deleted: 0,
                });
            }
        };

        let blob = match app_handle.icloud_sync().read_blob() {
            Ok(Some(b)) => b,
            _ => {
                return Ok(ICloudMergeStatus {
                    added: 0,
                    updated: 0,
                    deleted: 0,
                });
            }
        };

        let (remote_device_id, remote_accounts, remote_tombstones) =
            match crate::icloud::decrypt_from_cloud(&blob, &cloud_key) {
                Ok(v) => v,
                Err(_) => {
                    return Ok(ICloudMergeStatus {
                        added: 0,
                        updated: 0,
                        deleted: 0,
                    });
                }
            };

        let mut storage = lock_storage(&storage)?;

        if remote_device_id == storage.device_id() {
            return Ok(ICloudMergeStatus {
                added: 0,
                updated: 0,
                deleted: 0,
            });
        }

        let result = crate::icloud::merge_cloud_payload(
            &mut storage,
            &remote_device_id,
            remote_accounts,
            &remote_tombstones,
        )?;

        let repush_blob = if result.added > 0 || result.updated > 0 || result.deleted > 0 {
            Some(crate::icloud::encrypt_for_cloud(
                storage.list(),
                storage.tombstones(),
                storage.device_id(),
                &cloud_key,
            )?)
        } else {
            None
        };
        drop(storage);

        if let Some(blob) = repush_blob {
            let _ = app_handle.icloud_sync().write_blob(&blob);
        }

        crate::icloud::touch_last_synced(&data_dir);

        tracing::info!(
            event = "icloud_resume",
            added = result.added,
            updated = result.updated,
            deleted = result.deleted,
            "iCloud resume pull completed"
        );

        Ok(ICloudMergeStatus {
            added: result.added,
            updated: result.updated,
            deleted: result.deleted,
        })
    }
}

/// Push the current vault to iCloud if sync is enabled.
/// Encrypts the blob while the caller holds the storage lock (fast, CPU-only),
/// then sends it to a serialized background writer so the storage lock can be
/// released immediately by the caller and writes arrive in order.
#[cfg(target_os = "ios")]
pub fn icloud_push_if_enabled(app_handle: &tauri::AppHandle, storage: &Storage) {
    use std::sync::mpsc;
    use tauri::Manager;

    // Lazily-initialized single writer thread + channel.
    static WRITER: std::sync::OnceLock<mpsc::Sender<(tauri::AppHandle<tauri::Wry>, Vec<u8>)>> =
        std::sync::OnceLock::new();

    let data_dir = match app_handle.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return,
    };
    if !crate::icloud::is_enabled(&data_dir) {
        return;
    }
    let cloud_key = match crate::icloud::load_cloud_key() {
        Some(k) => k,
        None => return,
    };
    // Encrypt while we still have access to storage (fast, no I/O).
    let blob = match crate::icloud::encrypt_for_cloud(
        storage.list(),
        storage.tombstones(),
        storage.device_id(),
        &cloud_key,
    ) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to encrypt vault for iCloud push");
            return;
        }
    };
    // Send to the writer thread (serialized, in-order).
    let tx = WRITER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(tauri::AppHandle<tauri::Wry>, Vec<u8>)>();
        std::thread::spawn(move || {
            while let Ok((handle, data)) = rx.recv() {
                use tauri_plugin_icloud_sync::ICloudSyncExt;
                if let Err(e) = handle.icloud_sync().write_blob(&data) {
                    tracing::warn!(error = %e, "Failed to push vault to iCloud");
                } else if let Ok(d) = handle.path().app_data_dir() {
                    crate::icloud::touch_last_synced(&d);
                }
            }
        });
        tx
    });
    let _ = tx.send((app_handle.clone(), blob));
}

#[cfg(not(target_os = "ios"))]
pub fn icloud_push_if_enabled(_app_handle: &tauri::AppHandle, _storage: &Storage) {}

#[cfg(test)]
mod tests {
    use crate::pin::PinManager;
    use crate::storage::{Account, Storage};
    use crate::totp;

    fn test_key() -> [u8; 32] {
        [0xAA; 32]
    }

    #[test]
    fn test_validate_account_fields_valid() {
        assert!(super::validate_account_fields("GitHub", "user@test.com", "SHA1", 6, 30).is_ok());
        assert!(super::validate_account_fields("GitHub", "user@test.com", "SHA256", 8, 60).is_ok());
        assert!(super::validate_account_fields("", "", "SHA512", 6, 15).is_ok());
    }

    #[test]
    fn test_validate_digits_must_be_6_or_8() {
        assert!(super::validate_account_fields("X", "Y", "SHA1", 0, 30).is_err());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 5, 30).is_err());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 7, 30).is_err());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 10, 30).is_err());
    }

    #[test]
    fn test_validate_period_range() {
        assert!(super::validate_account_fields("X", "Y", "SHA1", 6, 14).is_err());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 6, 121).is_err());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 6, 15).is_ok());
        assert!(super::validate_account_fields("X", "Y", "SHA1", 6, 120).is_ok());
    }

    #[test]
    fn test_validate_issuer_label_length() {
        let long = "a".repeat(256);
        assert!(super::validate_account_fields(&long, "Y", "SHA1", 6, 30).is_err());
        assert!(super::validate_account_fields("X", &long, "SHA1", 6, 30).is_err());
        let max = "a".repeat(255);
        assert!(super::validate_account_fields(&max, &max, "SHA1", 6, 30).is_ok());
    }

    #[test]
    fn test_validate_algorithm() {
        assert!(super::validate_account_fields("X", "Y", "SHA1", 6, 30).is_ok());
        assert!(super::validate_account_fields("X", "Y", "SHA256", 6, 30).is_ok());
        assert!(super::validate_account_fields("X", "Y", "SHA512", 6, 30).is_ok());
        assert!(super::validate_account_fields("X", "Y", "MD5", 6, 30).is_err());
        assert!(super::validate_account_fields("X", "Y", "sha1", 6, 30).is_err());
        assert!(super::validate_account_fields("X", "Y", "", 6, 30).is_err());
    }

    #[test]
    fn test_account_display_strips_secret() {
        let account = Account {
            id: "id1".into(),
            issuer: "GitHub".into(),
            label: "user@test.com".into(),
            secret: "SUPERSECRET".into(),
            algorithm: "SHA1".into(),
            digits: 6,
            period: 30,
            icon: None,
            last_modified: 0,
        };
        let display = super::AccountDisplay::from(account);
        assert_eq!(display.id, "id1");
        assert_eq!(display.issuer, "GitHub");
    }

    #[test]
    fn test_deduplicate_and_import_skips_duplicates_within_same_batch() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();

        let base = Account {
            id: "import-1".into(),
            issuer: "GitHub".into(),
            label: "user@example.com".into(),
            secret: "JBSWY3DPEHPK3PXP".into(),
            algorithm: "SHA1".into(),
            digits: 6,
            period: 30,
            icon: None,
            last_modified: 0,
        };
        let mut duplicate = base.clone();
        duplicate.id = "import-2".into();

        let imported = super::deduplicate_and_import(vec![base, duplicate], &mut storage).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(storage.list().len(), 1);
    }

    #[test]
    fn test_full_add_generate_delete_flow() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();

        let uri = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub";
        let account = totp::parse_otpauth_uri(uri).unwrap();
        let id = account.id.clone();
        storage.add(account).unwrap();

        let acc = storage.get(&id).unwrap();
        let code = totp::generate_code(acc).unwrap();
        assert_eq!(code.code.len(), 6);

        storage.delete(&id).unwrap();
        assert!(storage.get(&id).is_none());
    }

    #[test]
    fn test_verify_export_reauth_without_pin_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let pin_manager = PinManager::new(dir.path().to_path_buf());
        assert!(super::verify_export_reauth(&pin_manager, None).is_ok());
    }

    #[test]
    fn test_verify_export_reauth_requires_current_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pin_manager = PinManager::new(dir.path().to_path_buf());
        pin_manager.set_pin("1234").unwrap();
        assert!(super::verify_export_reauth(&pin_manager, None).is_err());
    }

    #[test]
    fn test_verify_export_reauth_rejects_wrong_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pin_manager = PinManager::new(dir.path().to_path_buf());
        pin_manager.set_pin("1234").unwrap();
        let err = super::verify_export_reauth(&pin_manager, Some("9999".to_string())).unwrap_err();
        assert!(err.contains("Incorrect current PIN"));
    }

    #[test]
    fn test_verify_export_reauth_accepts_valid_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pin_manager = PinManager::new(dir.path().to_path_buf());
        pin_manager.set_pin("1234").unwrap();
        assert!(super::verify_export_reauth(&pin_manager, Some("1234".to_string())).is_ok());
    }
}
