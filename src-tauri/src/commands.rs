use crate::pin::PinManager;
use crate::storage::{Account, Storage};
use crate::totp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
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
pub fn get_accounts(storage: State<Mutex<Storage>>) -> Result<Vec<AccountDisplay>, String> {
    let storage = lock_storage(&storage)?;
    Ok(storage
        .list()
        .iter()
        .cloned()
        .map(AccountDisplay::from)
        .collect())
}

#[tauri::command]
pub fn add_account(uri: String, storage: State<Mutex<Storage>>) -> Result<AccountDisplay, String> {
    let account = totp::parse_otpauth_uri(&uri)?;
    let mut storage = lock_storage(&storage)?;
    if storage.has_duplicate(&account.issuer, &account.label, &account.secret) {
        return Err("This account already exists".to_string());
    }
    let display = AccountDisplay::from(account.clone());
    storage.add(account)?;
    Ok(display)
}

#[tauri::command]
pub fn add_account_manual(
    issuer: String,
    label: String,
    secret: String,
    algorithm: String,
    digits: u32,
    period: u32,
    storage: State<Mutex<Storage>>,
) -> Result<AccountDisplay, String> {
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
    Ok(display)
}

#[tauri::command]
pub fn delete_account(id: String, storage: State<Mutex<Storage>>) -> Result<(), String> {
    let mut storage = lock_storage(&storage)?;
    storage.delete(&id)
}

#[tauri::command]
pub fn generate_code(
    id: String,
    storage: State<Mutex<Storage>>,
) -> Result<totp::CodeResponse, String> {
    let storage = lock_storage(&storage)?;
    let account = storage
        .get(&id)
        .ok_or_else(|| "Account not found".to_string())?;
    totp::generate_code(account)
}

#[tauri::command]
pub fn generate_all_codes(
    storage: State<Mutex<Storage>>,
) -> Result<Vec<totp::CodeResponse>, String> {
    let storage = lock_storage(&storage)?;
    storage.list().iter().map(totp::generate_code).collect()
}

// --- Shared helpers ---

fn deduplicate_and_import(
    accounts: Vec<Account>,
    storage: &mut Storage,
) -> Result<Vec<AccountDisplay>, String> {
    let existing: Vec<(String, String, String)> = storage
        .list()
        .iter()
        .map(|e| (e.issuer.clone(), e.label.clone(), e.secret.clone()))
        .collect();
    let mut added = Vec::new();

    for account in accounts {
        let is_duplicate = existing
            .iter()
            .any(|e| e.0 == account.issuer && e.1 == account.label && e.2 == account.secret);
        if !is_duplicate {
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

#[tauri::command]
pub fn export_backup(password: String, storage: State<Mutex<Storage>>) -> Result<Vec<u8>, String> {
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
) -> Result<BackupPreview, String> {
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
    storage: State<Mutex<Storage>>,
) -> Result<Vec<AccountDisplay>, String> {
    let accounts = crate::backup::import_accounts(&data, &password)?;
    let mut storage = lock_storage(&storage)?;
    let added = deduplicate_and_import(accounts, &mut storage)?;
    tracing::info!(
        event = "backup_imported",
        count = added.len(),
        "Backup imported"
    );
    Ok(added)
}

// --- Backup file save (mobile-compatible) ---

#[tauri::command]
pub fn save_backup_file(data: Vec<u8>, app_handle: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("ghost-auth-backup-{}.ghostauth", timestamp);

    #[cfg(target_os = "ios")]
    {
        // iOS: write to temp dir, then present the native share sheet
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(&filename);

        std::fs::write(&path, &data).map_err(|e| {
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

    #[cfg(not(target_os = "ios"))]
    {
        // Android / other: save directly to Downloads directory
        let backup_dir = app_handle.path().download_dir().map_err(|e| {
            tracing::error!(error = %e, "Failed to resolve downloads directory");
            "Failed to save backup".to_string()
        })?;
        std::fs::create_dir_all(&backup_dir).map_err(|e| {
            tracing::error!(error = %e, "Failed to create backup directory");
            "Failed to save backup".to_string()
        })?;

        let path = backup_dir.join(&filename);

        std::fs::write(&path, &data).map_err(|e| {
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

fn account_to_otp_params(account: &Account) -> Result<crate::google_auth_proto::OtpParameters, String> {
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

fn build_migration_uri(params: Vec<crate::google_auth_proto::OtpParameters>, batch_size: i32, batch_index: i32) -> Result<String, String> {
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
    let b64_encoded = b64.replace('+', "%2B").replace('/', "%2F").replace('=', "%3D");
    Ok(format!("otpauth-migration://offline?data={b64_encoded}"))
}

#[tauri::command]
pub fn get_export_accounts(storage: State<Mutex<Storage>>) -> Result<Vec<ExportBatch>, String> {
    let storage = lock_storage(&storage)?;
    let accounts = storage.list();
    let total = accounts.len();
    let batch_count = if total == 0 { 0 } else { (total + EXPORT_BATCH_SIZE - 1) / EXPORT_BATCH_SIZE };

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

        let migration_uri = build_migration_uri(otp_params, batch_count as i32, batch_index as i32)?;

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

// --- External import commands ---

#[derive(Serialize)]
pub struct ImportPreview {
    pub format: String,
    pub accounts: Vec<AccountDisplay>,
    pub skipped: usize,
    pub duplicates: usize,
}

#[tauri::command]
pub fn import_external_preview(
    data: Vec<u8>,
    storage: State<Mutex<Storage>>,
) -> Result<ImportPreview, String> {
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
    storage: State<Mutex<Storage>>,
) -> Result<Vec<AccountDisplay>, String> {
    let result = crate::import::parse_import(&data)?;
    let mut storage = lock_storage(&storage)?;
    let added = deduplicate_and_import(result.accounts, &mut storage)?;
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
    storage: State<Mutex<Storage>>,
) -> Result<AccountDisplay, String> {
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
    Ok(AccountDisplay::from(account.clone()))
}

#[tauri::command]
pub fn reorder_accounts(ids: Vec<String>, storage: State<Mutex<Storage>>) -> Result<(), String> {
    let mut storage = lock_storage(&storage)?;
    storage.reorder(&ids)
}

// --- PIN commands ---

#[tauri::command]
pub fn has_pin(pin_manager: State<PinManager>) -> bool {
    pin_manager.has_pin()
}

#[tauri::command]
pub fn set_pin(
    mut pin: String,
    current_pin: Option<String>,
    pin_manager: State<PinManager>,
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
pub fn remove_pin(mut pin: String, pin_manager: State<PinManager>) -> Result<(), String> {
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
    tracing::info!(event = "pin_removed", "PIN was removed");
    Ok(())
}

#[tauri::command]
pub fn verify_recovery_code(
    mut code: String,
    pin_manager: State<PinManager>,
) -> Result<bool, String> {
    let result = pin_manager.verify_recovery_code(&code);
    code.zeroize();
    result
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
    phase: SyncPhase,
    pending: Option<PendingMerge>,
}

#[derive(Clone)]
enum SyncPhase {
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
) -> Result<SyncSessionInfo, String> {
    use tauri::Manager;

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
    drop(storage_guard);

    let key = *session.key();
    let local_payload =
        crate::sync::build_payload(&device_id, &accounts, &tombstones, &key)?;

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

    *state = Some(ActiveSync {
        session_id: session_id.clone(),
        phase: SyncPhase::WaitingForPeer,
        pending: None,
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
        let mut conn = match listener.accept_any(&key) {
            Ok(c) => c,
            Err(e) => {
                if let Ok(mut s) = shared.lock() {
                    if let Some(ref mut a) = *s {
                        if a.session_id == session_id {
                            a.phase = SyncPhase::Failed(e);
                        }
                    }
                }
                return;
            }
        };

        if let Ok(mut s) = shared.lock() {
            if let Some(ref mut a) = *s {
                if a.session_id == session_id {
                    a.phase = SyncPhase::Exchanging;
                }
            }
        }

        // Joiner sends first, so initiator receives first
        let remote_payload = match conn.recv_payload() {
            Ok(p) => p,
            Err(e) => {
                if let Ok(mut s) = shared.lock() {
                    if let Some(ref mut a) = *s {
                        if a.session_id == session_id {
                            a.phase = SyncPhase::Failed(e);
                        }
                    }
                }
                return;
            }
        };

        if let Err(e) = conn.send_payload(&local_payload) {
            if let Ok(mut s) = shared.lock() {
                if let Some(ref mut a) = *s {
                    if a.session_id == session_id {
                        a.phase = SyncPhase::Failed(e);
                    }
                }
            }
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
                if let Ok(mut s) = shared.lock() {
                    if let Some(ref mut a) = *s {
                        if a.session_id == session_id {
                            a.phase = SyncPhase::Failed(e);
                        }
                    }
                }
                return;
            }
        };

        let history = crate::sync::SyncHistory::load(&data_dir);
        let last_sync = history.last_sync_with(&remote_payload.device_id);

        let merge_result = crate::sync::merge(
            &accounts,
            &tombstones,
            remote_accounts,
            &remote_payload.tombstones,
            last_sync,
        );

        if let Ok(mut s) = shared.lock() {
            if let Some(ref mut a) = *s {
                if a.session_id == session_id {
                    a.pending = Some(PendingMerge {
                        remote_device_id: remote_payload.device_id,
                        merge_result,
                        data_dir,
                    });
                    a.phase = SyncPhase::MergeReady;
                }
            }
        }

        tracing::info!(
            event = "sync_exchange_complete",
            "Sync exchange completed (initiator)"
        );
    });

    Ok(info)
}

#[tauri::command]
pub fn sync_poll(sync_state: State<SyncManager>) -> Result<SyncPollResult, String> {
    let state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;

    let active = state
        .as_ref()
        .ok_or_else(|| "No active sync session".to_string())?;

    match &active.phase {
        SyncPhase::WaitingForPeer => Ok(SyncPollResult {
            status: "waiting".into(),
            merge_preview: None,
            error: None,
        }),
        SyncPhase::Exchanging => Ok(SyncPollResult {
            status: "exchanging".into(),
            merge_preview: None,
            error: None,
        }),
        SyncPhase::MergeReady => {
            let preview = active
                .pending
                .as_ref()
                .map(|p| merge_result_to_preview(&p.merge_result));
            Ok(SyncPollResult {
                status: "merge_ready".into(),
                merge_preview: preview,
                error: None,
            })
        }
        SyncPhase::Completed => Ok(SyncPollResult {
            status: "completed".into(),
            merge_preview: None,
            error: None,
        }),
        SyncPhase::Failed(e) => Ok(SyncPollResult {
            status: "error".into(),
            merge_preview: None,
            error: Some(e.clone()),
        }),
    }
}

#[tauri::command]
pub fn sync_join(
    code: String,
    host: String,
    port: u16,
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
    app_handle: tauri::AppHandle,
) -> Result<MergePreview, String> {
    use tauri::Manager;

    {
        let state = sync_state
            .inner
            .lock()
            .map_err(|_| "Sync state unavailable".to_string())?;
        if state.is_some() {
            return Err("A sync session is already active".to_string());
        }
    }

    let key = crate::sync::SyncSession::key_from_code(&code)?;
    let mut conn = crate::sync_transport::connect(&host, port, &key)?;

    // Snapshot storage
    let storage_guard = lock_storage(&storage)?;
    let device_id = storage_guard.device_id().to_string();
    let accounts = storage_guard.list().to_vec();
    let tombstones = storage_guard.tombstones().to_vec();
    drop(storage_guard);

    // Joiner sends first
    let local_payload =
        crate::sync::build_payload(&device_id, &accounts, &tombstones, &key)?;
    conn.send_payload(&local_payload)?;

    let remote_payload = conn.recv_payload()?;
    conn.close();

    // Decrypt and merge
    let remote_accounts: Result<Vec<_>, _> = remote_payload
        .accounts
        .iter()
        .map(|enc| crate::sync::decrypt_account(enc, &key))
        .collect();
    let remote_accounts = remote_accounts?;

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let history = crate::sync::SyncHistory::load(&data_dir);
    let last_sync = history.last_sync_with(&remote_payload.device_id);

    let merge_result = crate::sync::merge(
        &accounts,
        &tombstones,
        remote_accounts,
        &remote_payload.tombstones,
        last_sync,
    );

    let preview = merge_result_to_preview(&merge_result);

    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    *state = Some(ActiveSync {
        session_id: uuid::Uuid::new_v4().to_string(),
        phase: SyncPhase::MergeReady,
        pending: Some(PendingMerge {
            remote_device_id: remote_payload.device_id,
            merge_result,
            data_dir,
        }),
    });

    tracing::info!(
        event = "sync_exchange_complete",
        "Sync exchange completed (joiner)"
    );
    Ok(preview)
}

#[tauri::command]
pub fn sync_confirm(
    decisions: Vec<MergeDecision>,
    storage: State<Mutex<Storage>>,
    sync_state: State<SyncManager>,
) -> Result<SyncConfirmResult, String> {
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

    drop(storage);

    // Record sync in history
    let mut history = crate::sync::SyncHistory::load(&data_dir);
    history.record_sync(&remote_device_id, crate::storage::now_secs());
    if let Err(e) = history.save(&data_dir) {
        tracing::warn!(error = %e, "Failed to save sync history");
    }

    // Clear sync state
    if let Ok(mut state) = sync_state.inner.lock() {
        *state = None;
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
pub fn sync_cancel(sync_state: State<SyncManager>) -> Result<(), String> {
    let mut state = sync_state
        .inner
        .lock()
        .map_err(|_| "Sync state unavailable".to_string())?;
    *state = None;
    tracing::info!(event = "sync_cancelled", "Sync session cancelled");
    Ok(())
}

#[tauri::command]
pub fn sync_history(app_handle: tauri::AppHandle) -> Result<Vec<SyncPeerInfo>, String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;

    let history = crate::sync::SyncHistory::load(&data_dir);
    Ok(history
        .peers
        .into_iter()
        .map(|(device_id, last_synced)| SyncPeerInfo {
            device_id,
            last_synced,
        })
        .collect())
}

#[tauri::command]
pub fn save_theme(app_handle: tauri::AppHandle, theme: String) -> Result<(), String> {
    use tauri::Manager;
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Failed to resolve data directory".to_string())?;
    std::fs::write(data_dir.join("theme"), theme.as_bytes())
        .map_err(|e| format!("Failed to save theme: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
