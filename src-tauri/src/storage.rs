use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use crate::keystore;

const STORAGE_VERSION: u8 = 2;
const TOMBSTONE_RETENTION_DAYS: u64 = 90;

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Serialize, Deserialize)]
struct StoragePayload {
    version: u8,
    #[serde(default = "generate_device_id")]
    device_id: String,
    accounts: Vec<Account>,
    #[serde(default)]
    tombstones: Vec<Tombstone>,
}

fn generate_device_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tombstone {
    pub id: String,
    pub deleted_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub id: String,
    pub issuer: String,
    pub label: String,
    pub secret: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u32,
    pub icon: Option<String>,
    #[serde(default = "now_secs")]
    pub last_modified: u64,
}

pub struct Storage {
    data_dir: PathBuf,
    device_id: String,
    accounts: Vec<Account>,
    tombstones: Vec<Tombstone>,
    key: Zeroizing<[u8; 32]>,
}

impl Storage {
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|e| {
            tracing::error!(error = %e, path = %data_dir.display(), "Failed to create data directory");
            "Failed to initialize storage".to_string()
        })?;

        let key = Self::load_or_create_key(&data_dir)?;
        let (device_id, accounts, tombstones) = Self::load_payload(&data_dir, &key[..])?;

        Ok(Self {
            data_dir,
            device_id,
            accounts,
            tombstones,
            key,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn tombstones(&self) -> &[Tombstone] {
        &self.tombstones
    }

    fn legacy_key_path(data_dir: &Path) -> PathBuf {
        data_dir.join("ghost.key")
    }

    fn data_path(data_dir: &Path) -> PathBuf {
        data_dir.join("accounts.enc")
    }

    /// Load the encryption key with the following priority:
    /// 1. OS keychain (desktop: Credential Manager / Keychain / Secret Service)
    /// 2. Legacy plaintext file (ghost.key) — migrated to keychain then deleted
    /// 3. Generate new key — stored in keychain, file fallback if keychain unavailable
    fn load_or_create_key(data_dir: &Path) -> Result<Zeroizing<[u8; 32]>, String> {
        let legacy_path = Self::legacy_key_path(data_dir);

        // 1. Try OS keychain
        if let Some(key) = keystore::load_key() {
            // Clean up legacy file if it still exists
            if legacy_path.exists() {
                let _ = fs::remove_file(&legacy_path);
            }
            return Ok(Zeroizing::new(key));
        }

        // 2. Migrate from legacy plaintext file → keychain
        if legacy_path.exists() {
            let key = Self::read_key_file(&legacy_path)?;
            if keystore::store_key(&key) {
                // Migrated successfully — delete the plaintext file
                let _ = fs::remove_file(&legacy_path);
            }
            // Return key regardless of whether keychain store succeeded
            // (on mobile, store_key returns false but the key is still valid)
            return Ok(key);
        }

        // 3. Generate new key
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut *key);

        if keystore::store_key(&key) {
            // Stored securely in OS keychain — no file on disk
            return Ok(key);
        }

        // Fallback: write to file (mobile platforms until keychain integration)
        fs::write(&legacy_path, *key).map_err(|e| {
            tracing::error!(error = %e, "Failed to write key file");
            "Failed to initialize encryption".to_string()
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&legacy_path, perms);
        }

        Ok(key)
    }

    fn read_key_file(path: &Path) -> Result<Zeroizing<[u8; 32]>, String> {
        let bytes = fs::read(path).map_err(|e| {
            tracing::error!(error = %e, "Failed to read key file");
            "Failed to load encryption key".to_string()
        })?;
        if bytes.len() != 32 {
            return Err("Invalid encryption key".to_string());
        }
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    fn load_payload(
        data_dir: &Path,
        key: &[u8],
    ) -> Result<(String, Vec<Account>, Vec<Tombstone>), String> {
        let path = Self::data_path(data_dir);
        if !path.exists() {
            return Ok((generate_device_id(), Vec::new(), Vec::new()));
        }

        let data = fs::read(&path).map_err(|e| {
            tracing::error!(error = %e, "Failed to read accounts file");
            "Failed to load accounts".to_string()
        })?;
        if data.len() < 12 {
            return Ok((generate_device_id(), Vec::new(), Vec::new()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| {
            tracing::error!(error = %e, "Cipher initialization failed");
            "Failed to load accounts".to_string()
        })?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = match cipher.decrypt(nonce, ciphertext) {
            Ok(pt) => pt,
            Err(e) => {
                tracing::warn!(error = %e, "Decryption failed — encryption key does not match stored data. \
                    Backing up unreadable file and starting fresh.");
                let backup_path = path.with_extension("enc.bak");
                if let Err(rename_err) = fs::rename(&path, &backup_path) {
                    tracing::error!(error = %rename_err, "Failed to back up unreadable accounts file");
                }
                return Ok((generate_device_id(), Vec::new(), Vec::new()));
            }
        };

        // Try versioned format first, fall back to legacy Vec<Account>
        if let Ok(payload) = serde_json::from_slice::<StoragePayload>(&plaintext) {
            return Ok((payload.device_id, payload.accounts, payload.tombstones));
        }

        let accounts: Vec<Account> = serde_json::from_slice(&plaintext).map_err(|e| {
            tracing::error!(error = %e, "Failed to deserialize accounts");
            "Failed to load accounts".to_string()
        })?;
        Ok((generate_device_id(), accounts, Vec::new()))
    }

    fn save(&mut self) -> Result<(), String> {
        // Prune tombstones older than retention period
        let cutoff = now_secs().saturating_sub(TOMBSTONE_RETENTION_DAYS * 24 * 60 * 60);
        self.tombstones.retain(|t| t.deleted_at >= cutoff);

        let payload = StoragePayload {
            version: STORAGE_VERSION,
            device_id: self.device_id.clone(),
            accounts: self.accounts.clone(),
            tombstones: self.tombstones.clone(),
        };
        let plaintext = serde_json::to_vec(&payload).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize accounts");
            "Failed to save accounts".to_string()
        })?;

        let cipher = Aes256Gcm::new_from_slice(&self.key[..]).map_err(|e| {
            tracing::error!(error = %e, "Cipher initialization failed");
            "Failed to save accounts".to_string()
        })?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).map_err(|e| {
            tracing::error!(error = %e, "Encryption failed");
            "Failed to save accounts".to_string()
        })?;

        let mut data = Vec::with_capacity(12 + ciphertext.len());
        data.extend_from_slice(&nonce_bytes);
        data.extend(ciphertext);

        let path = Self::data_path(&self.data_dir);
        let tmp_path = path.with_extension("enc.tmp");

        fs::write(&tmp_path, &data).map_err(|e| {
            tracing::error!(error = %e, "Failed to write temporary accounts file");
            "Failed to save accounts".to_string()
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
        }

        fs::rename(&tmp_path, &path).map_err(|e| {
            tracing::error!(error = %e, "Failed to rename temporary accounts file");
            let _ = fs::remove_file(&tmp_path);
            "Failed to save accounts".to_string()
        })?;

        Ok(())
    }

    pub fn list(&self) -> &[Account] {
        &self.accounts
    }

    pub fn has_duplicate(&self, issuer: &str, label: &str, secret: &str) -> bool {
        self.accounts
            .iter()
            .any(|a| a.issuer == issuer && a.label == label && a.secret == secret)
    }

    pub fn add(&mut self, mut account: Account) -> Result<(), String> {
        account.last_modified = now_secs();
        self.accounts.push(account);
        self.save()
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.tombstones.push(Tombstone {
            id: id.to_string(),
            deleted_at: now_secs(),
        });
        self.accounts.retain(|a| a.id != id);
        self.save()
    }

    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn update(&mut self, id: &str, issuer: String, label: String) -> Result<(), String> {
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| "Account not found".to_string())?;
        account.issuer = issuer;
        account.label = label;
        account.last_modified = now_secs();
        self.save()
    }

    pub fn reorder(&mut self, ids: &[String]) -> Result<(), String> {
        // Build new order from the provided IDs
        let mut reordered = Vec::with_capacity(self.accounts.len());
        for id in ids {
            if let Some(pos) = self.accounts.iter().position(|a| a.id == *id) {
                reordered.push(self.accounts[pos].clone());
            }
        }
        // Append any accounts not in the provided list (safety net)
        for account in &self.accounts {
            if !ids.contains(&account.id) {
                reordered.push(account.clone());
            }
        }
        self.accounts = reordered;
        self.save()
    }

    /// Add a synced account, preserving its original last_modified timestamp.
    pub fn add_synced(&mut self, account: Account) -> Result<(), String> {
        self.accounts.push(account);
        self.save()
    }

    /// Replace an existing account in-place (preserving list order).
    pub fn replace_account(&mut self, account: Account) -> Result<(), String> {
        let pos = self
            .accounts
            .iter()
            .position(|a| a.id == account.id)
            .ok_or_else(|| "Account not found".to_string())?;
        self.accounts[pos] = account;
        self.save()
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

#[cfg(test)]
impl Storage {
    /// Test-only constructor that accepts a pre-generated key,
    /// bypassing OS keychain access.
    pub fn new_with_key(data_dir: PathBuf, key: [u8; 32]) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|e| {
            tracing::error!(error = %e, path = %data_dir.display(), "Failed to create data directory");
            "Failed to initialize storage".to_string()
        })?;
        let (device_id, accounts, tombstones) = Self::load_payload(&data_dir, &key[..])?;
        Ok(Self {
            data_dir,
            device_id,
            accounts,
            tombstones,
            key: Zeroizing::new(key),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0xAA; 32]
    }

    fn make_account(id: &str) -> Account {
        Account {
            id: id.to_string(),
            issuer: "TestIssuer".to_string(),
            label: "test@example.com".to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            algorithm: "SHA1".to_string(),
            digits: 6,
            period: 30,
            icon: None,
            last_modified: 0,
        }
    }

    #[test]
    fn test_add_and_list_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        assert_eq!(storage.list().len(), 0);

        storage.add(make_account("a1")).unwrap();
        assert_eq!(storage.list().len(), 1);
        assert_eq!(storage.list()[0].issuer, "TestIssuer");
    }

    #[test]
    fn test_encrypt_decrypt_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let key = test_key();

        {
            let mut s = Storage::new_with_key(dir.path().to_path_buf(), key).unwrap();
            s.add(make_account("a1")).unwrap();
            s.add(make_account("a2")).unwrap();
        }

        {
            let s = Storage::new_with_key(dir.path().to_path_buf(), key).unwrap();
            assert_eq!(s.list().len(), 2);
        }
    }

    #[test]
    fn test_wrong_key_recovers_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut s = Storage::new_with_key(dir.path().to_path_buf(), [0xAA; 32]).unwrap();
            s.add(make_account("a1")).unwrap();
        }
        // Wrong key triggers graceful recovery: backs up the file and starts fresh
        let s = Storage::new_with_key(dir.path().to_path_buf(), [0xBB; 32]).unwrap();
        assert_eq!(s.list().len(), 0);
        // Original file backed up as .enc.bak
        assert!(dir.path().join("accounts.enc.bak").exists());
    }

    #[test]
    fn test_corrupted_file_recovers_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.enc");
        fs::write(&path, vec![0u8; 64]).unwrap();
        // Corrupted data triggers graceful recovery: backs up and starts fresh
        let s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        assert_eq!(s.list().len(), 0);
        assert!(dir.path().join("accounts.enc.bak").exists());
    }

    #[test]
    fn test_short_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.enc");
        fs::write(&path, vec![0u8; 5]).unwrap();
        let s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        assert_eq!(s.list().len(), 0);
    }

    #[test]
    fn test_no_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        assert_eq!(s.list().len(), 0);
    }

    #[test]
    fn test_delete_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        s.add(make_account("a2")).unwrap();
        s.delete("a1").unwrap();
        assert_eq!(s.list().len(), 1);
        assert_eq!(s.list()[0].id, "a2");
    }

    #[test]
    fn test_get_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        assert!(s.get("a1").is_some());
        assert!(s.get("nonexistent").is_none());
    }

    #[test]
    fn test_has_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        assert!(s.has_duplicate("TestIssuer", "test@example.com", "JBSWY3DPEHPK3PXP"));
        assert!(!s.has_duplicate("Other", "test@example.com", "JBSWY3DPEHPK3PXP"));
        assert!(!s.has_duplicate("TestIssuer", "other@example.com", "JBSWY3DPEHPK3PXP"));
        assert!(!s.has_duplicate("TestIssuer", "test@example.com", "OTHERSECRET"));
    }

    #[test]
    fn test_update_account() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        s.update("a1", "NewIssuer".into(), "new@example.com".into())
            .unwrap();
        let acc = s.get("a1").unwrap();
        assert_eq!(acc.issuer, "NewIssuer");
        assert_eq!(acc.label, "new@example.com");
    }

    #[test]
    fn test_update_nonexistent_account_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        let result = s.update("nope", "X".into(), "Y".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_reorder_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        s.add(make_account("a2")).unwrap();
        s.add(make_account("a3")).unwrap();
        s.reorder(&["a3".into(), "a1".into(), "a2".into()]).unwrap();
        let ids: Vec<&str> = s.list().iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a3", "a1", "a2"]);
    }

    #[test]
    fn test_reorder_preserves_unlisted_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Storage::new_with_key(dir.path().to_path_buf(), test_key()).unwrap();
        s.add(make_account("a1")).unwrap();
        s.add(make_account("a2")).unwrap();
        s.add(make_account("a3")).unwrap();
        // Only reorder a2 and a3, a1 should be appended
        s.reorder(&["a3".into(), "a2".into()]).unwrap();
        let ids: Vec<&str> = s.list().iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a3", "a2", "a1"]);
    }
}
