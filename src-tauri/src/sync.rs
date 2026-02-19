use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::{rngs::OsRng, Rng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use zeroize::Zeroizing;

use crate::storage::{Account, Tombstone};

/// Unambiguous character set (excludes 0/O, 1/I/L) — matches pin.rs recovery codes.
const CODE_CHARS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Number of characters per group in the sync code.
const CODE_GROUP_LEN: usize = 4;
/// Number of groups in the sync code.
const CODE_GROUPS: usize = 6;
/// Sync code validity in seconds.
const CODE_EXPIRY_SECS: u64 = 60;

// ── Sync Session ──────────────────────────────────────────────────

/// An active sync session holding the ephemeral key and metadata.
pub struct SyncSession {
    pub id: String,
    key: Zeroizing<[u8; 32]>,
    pub code: String,
    created_at: Instant,
}

impl SyncSession {
    /// Create a new sync session with a random key and human-readable code.
    pub fn new() -> Self {
        let code = generate_sync_code();
        let key = Zeroizing::new(
            Self::key_from_code(&code).expect("Generated code is always valid"),
        );

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            key,
            code,
            created_at: Instant::now(),
        }
    }

    /// Check if this session has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= CODE_EXPIRY_SECS
    }

    /// Seconds remaining until expiry.
    pub fn remaining_secs(&self) -> u64 {
        CODE_EXPIRY_SECS.saturating_sub(self.created_at.elapsed().as_secs())
    }

    /// Regenerate the key and code (rotation).
    pub fn rotate(&mut self) {
        self.code = generate_sync_code();
        *self.key = Self::key_from_code(&self.code).expect("Generated code is always valid");
        self.created_at = Instant::now();
    }

    /// Get a reference to the session key.
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }

    /// Parse a sync code string and extract the key.
    /// For direct LAN sync, the code IS the key (encoded).
    /// Returns the decoded 32-byte key if valid.
    pub fn key_from_code(code: &str) -> Result<[u8; 32], String> {
        // Strip hyphens and whitespace
        let clean: String = code
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect::<String>()
            .to_uppercase();

        if clean.len() != CODE_GROUP_LEN * CODE_GROUPS {
            return Err("Invalid sync code length".to_string());
        }

        // Validate characters
        for c in clean.bytes() {
            if !CODE_CHARS.contains(&c) {
                return Err(format!("Invalid character in sync code: {}", c as char));
            }
        }

        // Derive a 256-bit key from the code using HMAC-SHA256.
        // Both sides (initiator and joiner) derive the same key from the same code.
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(b"ghost-auth-sync-key-v1")
                .expect("HMAC accepts any key size");
        mac.update(clean.as_bytes());
        let result = mac.finalize().into_bytes();

        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        Ok(key)
    }
}

/// Generate a formatted sync code: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
fn generate_sync_code() -> String {
    let mut rng = OsRng;
    let groups: Vec<String> = (0..CODE_GROUPS)
        .map(|_| {
            (0..CODE_GROUP_LEN)
                .map(|_| CODE_CHARS[rng.gen_range(0..CODE_CHARS.len())] as char)
                .collect()
        })
        .collect();
    groups.join("-")
}

// ── Per-Account Encryption ────────────────────────────────────────

/// An individually encrypted account for sync transport.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedAccount {
    pub id: String,
    pub last_modified: u64,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Encrypt a single account with the sync session key.
pub fn encrypt_account(account: &Account, key: &[u8; 32]) -> Result<EncryptedAccount, String> {
    let plaintext = serde_json::to_vec(account).map_err(|e| {
        tracing::error!(error = %e, "Failed to serialize account for sync");
        "Sync encryption failed".to_string()
    })?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| {
        tracing::error!(error = %e, "Cipher init failed");
        "Sync encryption failed".to_string()
    })?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).map_err(|e| {
        tracing::error!(error = %e, "Account encryption failed");
        "Sync encryption failed".to_string()
    })?;

    Ok(EncryptedAccount {
        id: account.id.clone(),
        last_modified: account.last_modified,
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypt a single account with the sync session key.
pub fn decrypt_account(enc: &EncryptedAccount, key: &[u8; 32]) -> Result<Account, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| {
        tracing::error!(error = %e, "Cipher init failed");
        "Sync decryption failed".to_string()
    })?;

    if enc.nonce.len() != 12 {
        return Err("Invalid nonce length in sync data".to_string());
    }
    let nonce = Nonce::from_slice(&enc.nonce);

    let plaintext = cipher.decrypt(nonce, enc.ciphertext.as_ref()).map_err(|_| {
        "Sync decryption failed — wrong key or corrupted data".to_string()
    })?;

    serde_json::from_slice(&plaintext).map_err(|e| {
        tracing::error!(error = %e, "Failed to deserialize synced account");
        "Sync decryption failed".to_string()
    })
}

// ── Sync Payload ──────────────────────────────────────────────────

/// The complete sync payload exchanged between devices.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncPayload {
    pub device_id: String,
    pub timestamp: u64,
    pub accounts: Vec<EncryptedAccount>,
    pub tombstones: Vec<Tombstone>,
}

/// Build a sync payload from the current storage state.
pub fn build_payload(
    device_id: &str,
    accounts: &[Account],
    tombstones: &[Tombstone],
    key: &[u8; 32],
) -> Result<SyncPayload, String> {
    let encrypted: Result<Vec<_>, _> = accounts
        .iter()
        .map(|a| encrypt_account(a, key))
        .collect();

    Ok(SyncPayload {
        device_id: device_id.to_string(),
        timestamp: crate::storage::now_secs(),
        accounts: encrypted?,
        tombstones: tombstones.to_vec(),
    })
}

// ── Merge Logic ───────────────────────────────────────────────────

/// Result of merging a remote payload with local state.
#[derive(Serialize, Debug)]
pub struct MergeResult {
    /// Accounts from the remote that don't exist locally — auto-add.
    pub to_add: Vec<Account>,
    /// Accounts that were changed on both sides since last sync.
    pub conflicts: Vec<MergeConflict>,
    /// Accounts deleted on the remote that still exist locally.
    pub remote_deletions: Vec<Account>,
    /// Accounts auto-updated (remote was newer, no conflict).
    pub auto_updated: Vec<Account>,
    /// Count of accounts that were identical.
    pub unchanged: usize,
}

/// A merge conflict where both devices changed the same account.
#[derive(Serialize, Clone, Debug)]
pub struct MergeConflict {
    pub local: Account,
    pub remote: Account,
}

/// Perform the merge between local state and a decrypted remote payload.
pub fn merge(
    local_accounts: &[Account],
    local_tombstones: &[Tombstone],
    remote_accounts: Vec<Account>,
    remote_tombstones: &[Tombstone],
    last_sync_with_peer: Option<u64>,
) -> MergeResult {
    let local_map: HashMap<&str, &Account> = local_accounts
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();

    let local_tombstone_set: HashMap<&str, u64> = local_tombstones
        .iter()
        .map(|t| (t.id.as_str(), t.deleted_at))
        .collect();

    let remote_tombstone_set: HashMap<&str, u64> = remote_tombstones
        .iter()
        .map(|t| (t.id.as_str(), t.deleted_at))
        .collect();

    let mut to_add = Vec::new();
    let mut conflicts = Vec::new();
    let mut auto_updated = Vec::new();
    let mut unchanged: usize = 0;

    let last_sync = last_sync_with_peer.unwrap_or(0);

    for remote in remote_accounts {
        // Skip if we locally deleted this account after the remote's last_modified
        if let Some(&deleted_at) = local_tombstone_set.get(remote.id.as_str()) {
            if deleted_at >= remote.last_modified {
                unchanged += 1;
                continue;
            }
        }

        if let Some(&local) = local_map.get(remote.id.as_str()) {
            // Account exists on both sides
            if local.last_modified == remote.last_modified {
                // Identical timestamp — no change needed
                unchanged += 1;
            } else if local.last_modified > last_sync && remote.last_modified > last_sync && last_sync > 0 {
                // Both modified since last sync — conflict
                conflicts.push(MergeConflict {
                    local: local.clone(),
                    remote,
                });
            } else if remote.last_modified > local.last_modified {
                // Remote is newer — auto-update
                auto_updated.push(remote);
            } else {
                // Local is newer — skip (we keep ours)
                unchanged += 1;
            }
        } else {
            // Account doesn't exist locally — add it
            to_add.push(remote);
        }
    }

    // Check remote tombstones against our local accounts
    let mut remote_deletions = Vec::new();
    for (id, &deleted_at) in &remote_tombstone_set {
        if let Some(&local) = local_map.get(id) {
            if deleted_at > local.last_modified {
                remote_deletions.push(local.clone());
            }
        }
    }

    MergeResult {
        to_add,
        conflicts,
        remote_deletions,
        auto_updated,
        unchanged,
    }
}

// ── Sync History ──────────────────────────────────────────────────

/// Tracks the last sync timestamp with each peer device.
#[derive(Serialize, Deserialize, Default)]
pub struct SyncHistory {
    pub peers: HashMap<String, u64>,
}

impl SyncHistory {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("sync_history.json");
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        let path = data_dir.join("sync_history.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize sync history");
            "Failed to save sync history".to_string()
        })?;
        fs::write(&path, json).map_err(|e| {
            tracing::error!(error = %e, "Failed to write sync history");
            "Failed to save sync history".to_string()
        })
    }

    pub fn last_sync_with(&self, device_id: &str) -> Option<u64> {
        self.peers.get(device_id).copied()
    }

    pub fn record_sync(&mut self, device_id: &str, timestamp: u64) {
        self.peers.insert(device_id.to_string(), timestamp);
    }
}

// ── Session Encryption (transport-layer envelope) ─────────────────

/// Derive a session encryption key using HKDF-SHA256 (RFC 5869).
/// IKM = shared sync key, Salt = handshake nonce, Info = "ghost-auth-session-v1".
/// The result is cryptographically distinct from both the shared key and handshake HMACs.
pub(crate) fn derive_session_key(key: &[u8; 32], nonce: &[u8; 32]) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // HKDF-Extract (RFC 5869 §2.2): PRK = HMAC-SHA256(salt=nonce, IKM=key)
    let mut extract = <HmacSha256 as Mac>::new_from_slice(nonce)
        .expect("HMAC accepts any key size");
    extract.update(key);
    let prk = extract.finalize().into_bytes();

    // HKDF-Expand (RFC 5869 §2.3): T(1) = HMAC-SHA256(PRK, info || 0x01)
    let mut expand = <HmacSha256 as Mac>::new_from_slice(&prk)
        .expect("HMAC accepts any key size");
    expand.update(b"ghost-auth-session-v1");
    expand.update(&[0x01u8]);
    let okm = expand.finalize().into_bytes();

    let mut result = [0u8; 32];
    result.copy_from_slice(&okm);
    result
}

/// Encrypt data with AES-256-GCM using a fresh random nonce.
pub(crate) fn session_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<([u8; 12], Vec<u8>), String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "Session cipher init failed".to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|_| "Session encryption failed".to_string())?;
    Ok((nonce_bytes, ciphertext))
}

/// Decrypt data with AES-256-GCM.
pub(crate) fn session_decrypt(key: &[u8; 32], nonce_bytes: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if nonce_bytes.len() != 12 {
        return Err("Invalid session nonce length".to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "Session cipher init failed".to_string())?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Session decryption failed — data may be tampered or from a different session".to_string())
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_account(id: &str, issuer: &str, modified: u64) -> Account {
        Account {
            id: id.to_string(),
            issuer: issuer.to_string(),
            label: "test@example.com".to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            algorithm: "SHA1".to_string(),
            digits: 6,
            period: 30,
            icon: None,
            last_modified: modified,
        }
    }

    #[test]
    fn test_sync_session_creation() {
        let session = SyncSession::new();
        assert!(!session.is_expired());
        assert!(session.remaining_secs() > 0);
        assert!(session.remaining_secs() <= CODE_EXPIRY_SECS);
        // Code format: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
        assert_eq!(session.code.matches('-').count(), CODE_GROUPS - 1);
    }

    #[test]
    fn test_sync_code_format() {
        let code = generate_sync_code();
        let parts: Vec<&str> = code.split('-').collect();
        assert_eq!(parts.len(), CODE_GROUPS);
        for part in &parts {
            assert_eq!(part.len(), CODE_GROUP_LEN);
            for c in part.bytes() {
                assert!(CODE_CHARS.contains(&c), "Invalid char: {}", c as char);
            }
        }
    }

    #[test]
    fn test_sync_session_rotation() {
        let mut session = SyncSession::new();
        let old_code = session.code.clone();
        let old_key = *session.key();
        session.rotate();
        // After rotation, code and key should change (extremely unlikely to be same)
        assert_ne!(session.code, old_code);
        assert_ne!(*session.key(), old_key);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let account = make_account("a1", "GitHub", 1000);
        let key = [0xAA; 32];

        let encrypted = encrypt_account(&account, &key).unwrap();
        assert_eq!(encrypted.id, "a1");
        assert_eq!(encrypted.last_modified, 1000);
        assert!(!encrypted.ciphertext.is_empty());

        let decrypted = decrypt_account(&encrypted, &key).unwrap();
        assert_eq!(decrypted.id, "a1");
        assert_eq!(decrypted.issuer, "GitHub");
        assert_eq!(decrypted.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(decrypted.last_modified, 1000);
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let account = make_account("a1", "GitHub", 1000);
        let encrypted = encrypt_account(&account, &[0xAA; 32]).unwrap();
        let result = decrypt_account(&encrypted, &[0xBB; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_payload() {
        let accounts = vec![
            make_account("a1", "GitHub", 1000),
            make_account("a2", "Google", 2000),
        ];
        let tombstones = vec![Tombstone {
            id: "a3".to_string(),
            deleted_at: 500,
        }];
        let key = [0xCC; 32];

        let payload = build_payload("device-1", &accounts, &tombstones, &key).unwrap();
        assert_eq!(payload.device_id, "device-1");
        assert_eq!(payload.accounts.len(), 2);
        assert_eq!(payload.tombstones.len(), 1);

        // Verify we can decrypt the accounts
        let dec1 = decrypt_account(&payload.accounts[0], &key).unwrap();
        assert_eq!(dec1.issuer, "GitHub");
        let dec2 = decrypt_account(&payload.accounts[1], &key).unwrap();
        assert_eq!(dec2.issuer, "Google");
    }

    #[test]
    fn test_merge_new_accounts() {
        let local = vec![make_account("a1", "GitHub", 1000)];
        let remote = vec![
            make_account("a1", "GitHub", 1000),
            make_account("a2", "Google", 2000),
        ];

        let result = merge(&local, &[], remote, &[], None);
        assert_eq!(result.to_add.len(), 1);
        assert_eq!(result.to_add[0].id, "a2");
        assert_eq!(result.unchanged, 1);
        assert!(result.conflicts.is_empty());
        assert!(result.auto_updated.is_empty());
    }

    #[test]
    fn test_merge_remote_newer() {
        let local = vec![make_account("a1", "GitHub", 1000)];
        let remote = vec![make_account("a1", "GitHub Updated", 2000)];

        let result = merge(&local, &[], remote, &[], None);
        assert_eq!(result.auto_updated.len(), 1);
        assert_eq!(result.auto_updated[0].issuer, "GitHub Updated");
        assert!(result.to_add.is_empty());
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_merge_local_newer() {
        let local = vec![make_account("a1", "GitHub Updated", 2000)];
        let remote = vec![make_account("a1", "GitHub", 1000)];

        let result = merge(&local, &[], remote, &[], None);
        assert_eq!(result.unchanged, 1);
        assert!(result.auto_updated.is_empty());
        assert!(result.to_add.is_empty());
    }

    #[test]
    fn test_merge_conflict() {
        let local = vec![make_account("a1", "GitHub Local", 2000)];
        let remote = vec![make_account("a1", "GitHub Remote", 2500)];

        // Both modified since last sync at 1500
        let result = merge(&local, &[], remote, &[], Some(1500));
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].local.issuer, "GitHub Local");
        assert_eq!(result.conflicts[0].remote.issuer, "GitHub Remote");
    }

    #[test]
    fn test_merge_no_conflict_without_prior_sync() {
        // Without a last_sync timestamp, we can't detect conflicts —
        // the newer account wins.
        let local = vec![make_account("a1", "GitHub Local", 2000)];
        let remote = vec![make_account("a1", "GitHub Remote", 2500)];

        let result = merge(&local, &[], remote, &[], None);
        assert!(result.conflicts.is_empty());
        assert_eq!(result.auto_updated.len(), 1);
    }

    #[test]
    fn test_merge_remote_deletion() {
        let local = vec![make_account("a1", "GitHub", 1000)];
        let remote_tombstones = vec![Tombstone {
            id: "a1".to_string(),
            deleted_at: 2000,
        }];

        let result = merge(&local, &[], vec![], &remote_tombstones, None);
        assert_eq!(result.remote_deletions.len(), 1);
        assert_eq!(result.remote_deletions[0].id, "a1");
    }

    #[test]
    fn test_merge_remote_deletion_skipped_if_local_newer() {
        let local = vec![make_account("a1", "GitHub", 3000)];
        let remote_tombstones = vec![Tombstone {
            id: "a1".to_string(),
            deleted_at: 2000,
        }];

        let result = merge(&local, &[], vec![], &remote_tombstones, None);
        assert!(result.remote_deletions.is_empty());
    }

    #[test]
    fn test_merge_local_tombstone_blocks_add() {
        let local: Vec<Account> = vec![];
        let local_tombstones = vec![Tombstone {
            id: "a1".to_string(),
            deleted_at: 2000,
        }];
        let remote = vec![make_account("a1", "GitHub", 1000)];

        let result = merge(&local, &local_tombstones, remote, &[], None);
        // Should not re-add because local tombstone is newer
        assert!(result.to_add.is_empty());
    }

    #[test]
    fn test_merge_local_tombstone_allows_add_if_remote_newer() {
        let local: Vec<Account> = vec![];
        let local_tombstones = vec![Tombstone {
            id: "a1".to_string(),
            deleted_at: 1000,
        }];
        let remote = vec![make_account("a1", "GitHub", 2000)];

        let result = merge(&local, &local_tombstones, remote, &[], None);
        // Remote is newer than tombstone — should add
        assert_eq!(result.to_add.len(), 1);
    }

    #[test]
    fn test_key_from_code_consistency() {
        let session = SyncSession::new();
        let derived = SyncSession::key_from_code(&session.code).unwrap();
        assert_eq!(*session.key(), derived);

        // Same code always produces same key
        let derived2 = SyncSession::key_from_code(&session.code).unwrap();
        assert_eq!(derived, derived2);

        // Different code produces different key
        let session2 = SyncSession::new();
        let derived3 = SyncSession::key_from_code(&session2.code).unwrap();
        assert_ne!(derived, derived3);
    }

    #[test]
    fn test_key_from_code_handles_formatting() {
        let session = SyncSession::new();
        let key1 = SyncSession::key_from_code(&session.code).unwrap();

        // Without hyphens
        let clean = session.code.replace('-', "");
        let key2 = SyncSession::key_from_code(&clean).unwrap();
        assert_eq!(key1, key2);

        // With spaces
        let spaced = session.code.replace('-', " ");
        let key3 = SyncSession::key_from_code(&spaced).unwrap();
        assert_eq!(key1, key3);

        // Lowercase
        let lower = session.code.to_lowercase();
        let key4 = SyncSession::key_from_code(&lower).unwrap();
        assert_eq!(key1, key4);
    }

    #[test]
    fn test_sync_history_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut history = SyncHistory::default();

        assert!(history.last_sync_with("device-2").is_none());

        history.record_sync("device-2", 1000);
        history.save(dir.path()).unwrap();

        let loaded = SyncHistory::load(dir.path());
        assert_eq!(loaded.last_sync_with("device-2"), Some(1000));
    }

    #[test]
    fn test_sync_history_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let history = SyncHistory::load(dir.path());
        assert!(history.peers.is_empty());
    }

    #[test]
    fn test_derive_session_key_deterministic() {
        let key = [0xAA; 32];
        let nonce = [0xBB; 32];
        let sek1 = derive_session_key(&key, &nonce);
        let sek2 = derive_session_key(&key, &nonce);
        assert_eq!(sek1, sek2);
    }

    #[test]
    fn test_derive_session_key_different_nonces() {
        let key = [0xAA; 32];
        let nonce_a = [0xBB; 32];
        let nonce_b = [0xCC; 32];
        assert_ne!(derive_session_key(&key, &nonce_a), derive_session_key(&key, &nonce_b));
    }

    #[test]
    fn test_derive_session_key_differs_from_shared_key() {
        let key = [0xAA; 32];
        let nonce = [0xBB; 32];
        let sek = derive_session_key(&key, &nonce);
        // SEK must not equal the shared key
        assert_ne!(sek, key);
    }

    #[test]
    fn test_session_encrypt_decrypt_roundtrip() {
        let key = [0xDD; 32];
        let plaintext = b"hello, world! this is sync payload data";
        let (nonce, ciphertext) = session_encrypt(&key, plaintext).unwrap();
        let decrypted = session_decrypt(&key, &nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_session_decrypt_wrong_key_fails() {
        let key_a = [0xDD; 32];
        let key_b = [0xEE; 32];
        let (nonce, ciphertext) = session_encrypt(&key_a, b"secret").unwrap();
        assert!(session_decrypt(&key_b, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn test_session_decrypt_tampered_ciphertext_fails() {
        let key = [0xDD; 32];
        let (nonce, mut ciphertext) = session_encrypt(&key, b"secret").unwrap();
        ciphertext[0] ^= 0xFF; // flip a byte
        assert!(session_decrypt(&key, &nonce, &ciphertext).is_err());
    }
}
