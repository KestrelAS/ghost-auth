use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

const RECOVERY_CODE_COUNT: usize = 8;
// Excludes ambiguous characters: 0/O, 1/I/L
const RECOVERY_CODE_CHARS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

#[derive(Serialize, Deserialize)]
struct RateLimitState {
    failed_attempts: u32,
    /// Unix timestamp of the last failure (seconds since epoch).
    last_failure_epoch: Option<u64>,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            failed_attempts: 0,
            last_failure_epoch: None,
        }
    }

    fn lockout_duration(&self) -> Option<Duration> {
        let lockout_secs = match self.failed_attempts {
            0..=4 => return None,
            5..=7 => 30u64,
            8..=9 => 300,
            _ => 900,
        };

        if let Some(last) = self.last_failure_epoch {
            let elapsed = now_secs().saturating_sub(last);
            if elapsed < lockout_secs {
                Some(Duration::from_secs(lockout_secs - elapsed))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn record_failure(&mut self) {
        self.failed_attempts += 1;
        self.last_failure_epoch = Some(now_secs());
    }

    fn reset(&mut self) {
        self.failed_attempts = 0;
        self.last_failure_epoch = None;
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct RecoveryCodeEntry {
    hash: String,
    used: bool,
}

#[derive(Serialize, Deserialize)]
struct RecoveryStore {
    codes: Vec<RecoveryCodeEntry>,
}

pub struct PinManager {
    hash_path: PathBuf,
    rate_limit_path: PathBuf,
    recovery_path: PathBuf,
    rate_limit: Mutex<RateLimitState>,
}

impl PinManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let rate_limit_path = data_dir.join("pin.ratelimit");
        let state = Self::load_rate_limit(&rate_limit_path);
        Self {
            hash_path: data_dir.join("pin.hash"),
            rate_limit_path,
            recovery_path: data_dir.join("pin.recovery"),
            rate_limit: Mutex::new(state),
        }
    }

    fn load_rate_limit(path: &PathBuf) -> RateLimitState {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(RateLimitState::new)
    }

    fn persist_rate_limit(&self, state: &RateLimitState) {
        if let Ok(json) = serde_json::to_string(state) {
            let _ = write_restricted(&self.rate_limit_path, &json);
        }
    }

    pub fn has_pin(&self) -> bool {
        self.hash_path.exists()
    }

    pub fn set_pin(&self, pin: &str) -> Result<Vec<String>, String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let hash = argon2.hash_password(pin.as_bytes(), &salt).map_err(|e| {
            tracing::error!(error = %e, "Failed to hash PIN");
            "Failed to set PIN".to_string()
        })?;

        write_restricted(&self.hash_path, &hash.to_string())?;

        let codes = Self::generate_recovery_codes();
        self.store_recovery_hashes(&codes)?;

        Ok(codes)
    }

    pub fn verify_pin(&self, pin: &str) -> Result<bool, String> {
        // Check rate limit
        let mut rl = self.rate_limit.lock().map_err(|_| {
            tracing::error!("Rate limit mutex poisoned");
            "PIN verification unavailable — please restart the app".to_string()
        })?;

        if let Some(remaining) = rl.lockout_duration() {
            tracing::warn!(
                event = "pin_lockout",
                remaining_secs = remaining.as_secs(),
                attempts = rl.failed_attempts,
                "PIN entry locked out"
            );
            return Err(format!(
                "Too many attempts. Try again in {} seconds.",
                remaining.as_secs()
            ));
        }

        let hash_str = fs::read_to_string(&self.hash_path).map_err(|e| {
            tracing::error!(error = %e, "Failed to read PIN file");
            "PIN not configured".to_string()
        })?;

        let hash = PasswordHash::new(&hash_str).map_err(|e| {
            tracing::error!(error = %e, "Invalid PIN hash");
            "PIN data corrupted".to_string()
        })?;

        let valid = Argon2::default()
            .verify_password(pin.as_bytes(), &hash)
            .is_ok();

        if valid {
            tracing::info!(event = "pin_verified", "PIN verified successfully");
            rl.reset();
        } else {
            rl.record_failure();
            tracing::warn!(
                event = "pin_failed",
                attempts = rl.failed_attempts,
                "Failed PIN verification attempt"
            );
        }

        self.persist_rate_limit(&rl);
        Ok(valid)
    }

    /// Internal: removes PIN hash file without touching the rate limit lock.
    fn remove_pin_files(&self) -> Result<(), String> {
        if self.hash_path.exists() {
            fs::remove_file(&self.hash_path).map_err(|e| {
                tracing::error!(error = %e, "Failed to remove PIN file");
                "Failed to remove PIN".to_string()
            })?;
        }
        Ok(())
    }

    pub fn remove_pin(&self) -> Result<(), String> {
        self.remove_pin_files()?;
        // Reset rate limit on removal
        if let Ok(mut rl) = self.rate_limit.lock() {
            rl.reset();
            self.persist_rate_limit(&rl);
        }
        // Clean up rate limit and recovery files
        let _ = fs::remove_file(&self.rate_limit_path);
        let _ = fs::remove_file(&self.recovery_path);
        Ok(())
    }

    fn generate_recovery_codes() -> Vec<String> {
        let mut rng = OsRng;
        (0..RECOVERY_CODE_COUNT)
            .map(|_| {
                let part1: String = (0..4)
                    .map(|_| RECOVERY_CODE_CHARS[rng.gen_range(0..RECOVERY_CODE_CHARS.len())] as char)
                    .collect();
                let part2: String = (0..4)
                    .map(|_| RECOVERY_CODE_CHARS[rng.gen_range(0..RECOVERY_CODE_CHARS.len())] as char)
                    .collect();
                format!("{}-{}", part1, part2)
            })
            .collect()
    }

    fn store_recovery_hashes(&self, codes: &[String]) -> Result<(), String> {
        let argon2 = Argon2::default();
        let entries: Result<Vec<RecoveryCodeEntry>, String> = codes
            .iter()
            .map(|code| {
                let normalized = code.replace('-', "").to_uppercase();
                let salt = SaltString::generate(&mut OsRng);
                let hash = argon2
                    .hash_password(normalized.as_bytes(), &salt)
                    .map_err(|e| {
                        tracing::error!(error = %e, "Failed to hash recovery code");
                        "Failed to generate recovery codes".to_string()
                    })?;
                Ok(RecoveryCodeEntry {
                    hash: hash.to_string(),
                    used: false,
                })
            })
            .collect();

        let store = RecoveryStore { codes: entries? };
        let json = serde_json::to_string(&store).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize recovery codes");
            "Failed to save recovery codes".to_string()
        })?;

        write_restricted(&self.recovery_path, &json)
    }

    pub fn verify_recovery_code(&self, code: &str) -> Result<bool, String> {
        // Shared rate limiting with PIN
        let mut rl = self.rate_limit.lock().map_err(|_| {
            tracing::error!("Rate limit mutex poisoned");
            "Recovery code verification unavailable — please restart the app".to_string()
        })?;

        if let Some(remaining) = rl.lockout_duration() {
            tracing::warn!(
                event = "recovery_lockout",
                remaining_secs = remaining.as_secs(),
                attempts = rl.failed_attempts,
                "Recovery code entry locked out"
            );
            return Err(format!(
                "Too many attempts. Try again in {} seconds.",
                remaining.as_secs()
            ));
        }

        let json = fs::read_to_string(&self.recovery_path).map_err(|e| {
            tracing::error!(error = %e, "Failed to read recovery codes file");
            "No recovery codes available".to_string()
        })?;

        let mut store: RecoveryStore = serde_json::from_str(&json).map_err(|e| {
            tracing::error!(error = %e, "Failed to parse recovery codes");
            "Recovery codes data corrupted".to_string()
        })?;

        let normalized = code.replace('-', "").to_uppercase();
        let argon2 = Argon2::default();

        let mut matched_idx: Option<usize> = None;
        for (i, entry) in store.codes.iter().enumerate() {
            if entry.used {
                continue;
            }
            if let Ok(hash) = PasswordHash::new(&entry.hash) {
                if argon2.verify_password(normalized.as_bytes(), &hash).is_ok() {
                    matched_idx = Some(i);
                    break;
                }
            }
        }

        if let Some(idx) = matched_idx {
            // Mark code as used
            store.codes[idx].used = true;
            let json = serde_json::to_string(&store).map_err(|e| {
                tracing::error!(error = %e, "Failed to serialize recovery codes");
                "Failed to update recovery codes".to_string()
            })?;
            write_restricted(&self.recovery_path, &json)?;

            // Reset rate limit and remove PIN
            rl.reset();
            self.persist_rate_limit(&rl);

            tracing::info!(event = "recovery_code_used", "Recovery code used successfully, removing PIN");

            // Drop lock before calling remove_pin_files (which doesn't need the lock)
            drop(rl);
            self.remove_pin_files()?;
            // Clean up rate limit file
            let _ = fs::remove_file(&self.rate_limit_path);

            Ok(true)
        } else {
            rl.record_failure();
            tracing::warn!(
                event = "recovery_code_failed",
                attempts = rl.failed_attempts,
                "Failed recovery code attempt"
            );
            self.persist_rate_limit(&rl);
            Ok(false)
        }
    }

    pub fn has_recovery_codes(&self) -> bool {
        if let Ok(json) = fs::read_to_string(&self.recovery_path) {
            if let Ok(store) = serde_json::from_str::<RecoveryStore>(&json) {
                return store.codes.iter().any(|entry| !entry.used);
            }
        }
        false
    }
}

/// Write a file with restricted permissions (owner-only on Unix, current user + SYSTEM on Windows).
fn write_restricted(path: &std::path::Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| {
        tracing::error!(error = %e, "Failed to write file");
        "Failed to save PIN".to_string()
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms).map_err(|e| {
            tracing::error!(error = %e, "Failed to set file permissions");
            "Failed to save PIN".to_string()
        })?;
    }

    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy();
        let username = std::env::var("USERNAME").unwrap_or_default();
        if !username.is_empty() {
            // Remove inherited permissions, grant only current user + SYSTEM
            let output = std::process::Command::new("icacls")
                .args([
                    &*path_str,
                    "/inheritance:r",
                    "/grant:r",
                    &format!("{}:F", username),
                    "/grant:r",
                    "SYSTEM:F",
                ])
                .output();
            if let Err(e) = output {
                tracing::warn!(error = %e, "Failed to restrict file permissions via icacls");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_pin_initially() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        assert!(!pm.has_pin());
    }

    #[test]
    fn test_set_and_verify_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1234").unwrap();
        assert!(pm.has_pin());
        assert!(pm.verify_pin("1234").unwrap());
        assert!(!pm.verify_pin("9999").unwrap());
    }

    #[test]
    fn test_remove_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("5678").unwrap();
        assert!(pm.has_pin());
        pm.remove_pin().unwrap();
        assert!(!pm.has_pin());
    }

    #[test]
    fn test_remove_nonexistent_pin_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        assert!(pm.remove_pin().is_ok());
    }

    #[test]
    fn test_verify_without_pin_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        assert!(pm.verify_pin("1234").is_err());
    }

    #[test]
    fn test_overwrite_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1111").unwrap();
        pm.set_pin("2222").unwrap();
        assert!(!pm.verify_pin("1111").unwrap());
        assert!(pm.verify_pin("2222").unwrap());
    }

    #[test]
    fn test_rate_limiting_lockout() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1234").unwrap();

        // 5 failed attempts should trigger lockout
        for _ in 0..5 {
            let _ = pm.verify_pin("wrong");
        }

        let result = pm.verify_pin("1234");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Too many attempts"));
    }

    #[test]
    fn test_rate_limiting_resets_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1234").unwrap();

        // 4 failures (below threshold)
        for _ in 0..4 {
            let _ = pm.verify_pin("wrong");
        }

        // Successful verification resets counter
        assert!(pm.verify_pin("1234").unwrap());

        // 4 more failures should still be fine
        for _ in 0..4 {
            let _ = pm.verify_pin("wrong");
        }

        // Should still work (counter was reset)
        assert!(pm.verify_pin("1234").unwrap());
    }

    #[test]
    fn test_rate_limit_persists_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();

        // First instance: set PIN and accumulate failures
        {
            let pm = PinManager::new(path.clone());
            pm.set_pin("1234").unwrap();
            for _ in 0..5 {
                let _ = pm.verify_pin("wrong");
            }
        }

        // Second instance: should still be locked out
        {
            let pm = PinManager::new(path);
            let result = pm.verify_pin("1234");
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("Too many attempts"));
        }
    }

    #[test]
    fn test_remove_pin_cleans_rate_limit_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let pm = PinManager::new(path.clone());
        pm.set_pin("1234").unwrap();

        for _ in 0..3 {
            let _ = pm.verify_pin("wrong");
        }

        pm.remove_pin().unwrap();
        assert!(!path.join("pin.ratelimit").exists());
    }

    // --- Recovery code tests ---

    #[test]
    fn test_set_pin_returns_recovery_codes() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        let codes = pm.set_pin("1234").unwrap();
        assert_eq!(codes.len(), 8);
        for code in &codes {
            assert_eq!(code.len(), 9); // XXXX-XXXX
            assert_eq!(code.chars().nth(4), Some('-'));
        }
    }

    #[test]
    fn test_recovery_code_verifies_and_removes_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        let codes = pm.set_pin("1234").unwrap();
        assert!(pm.has_pin());
        assert!(pm.has_recovery_codes());

        let result = pm.verify_recovery_code(&codes[0]).unwrap();
        assert!(result);
        assert!(!pm.has_pin());
    }

    #[test]
    fn test_recovery_code_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        let codes = pm.set_pin("1234").unwrap();

        let lower = codes[0].to_lowercase();
        let result = pm.verify_recovery_code(&lower).unwrap();
        assert!(result);
    }

    #[test]
    fn test_recovery_code_without_hyphen() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        let codes = pm.set_pin("1234").unwrap();

        let no_hyphen = codes[0].replace('-', "");
        let result = pm.verify_recovery_code(&no_hyphen).unwrap();
        assert!(result);
    }

    #[test]
    fn test_invalid_recovery_code() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1234").unwrap();

        let result = pm.verify_recovery_code("ZZZZ-ZZZZ").unwrap();
        assert!(!result);
        assert!(pm.has_pin());
    }

    #[test]
    fn test_recovery_code_rate_limited() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        pm.set_pin("1234").unwrap();

        for _ in 0..5 {
            let _ = pm.verify_recovery_code("ZZZZ-ZZZZ");
        }

        let result = pm.verify_recovery_code("ZZZZ-ZZZZ");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many attempts"));
    }

    #[test]
    fn test_has_recovery_codes_false_without_pin() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PinManager::new(dir.path().to_path_buf());
        assert!(!pm.has_recovery_codes());
    }

    #[test]
    fn test_remove_pin_cleans_recovery_codes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let pm = PinManager::new(path.clone());
        pm.set_pin("1234").unwrap();
        assert!(path.join("pin.recovery").exists());
        pm.remove_pin().unwrap();
        assert!(!path.join("pin.recovery").exists());
    }
}
