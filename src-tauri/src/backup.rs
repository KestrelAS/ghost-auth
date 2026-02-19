use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::storage::Account;

const MAGIC: &[u8; 4] = b"GHST";
const FORMAT_VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct BackupPayload {
    version: u8,
    exported_at: u64,
    accounts: Vec<Account>,
}

/// Derive a 32-byte key from a password and salt using Argon2id.
/// The returned key is wrapped in `Zeroizing` to ensure it is zeroed on drop.
fn derive_key(password: &str, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>, String> {
    let params = Params::new(65536, 3, 1, Some(32)).map_err(|e| {
        tracing::error!(error = %e, "Argon2 parameter construction failed");
        "Key derivation failed".to_string()
    })?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|e| {
            tracing::error!(error = %e, "Key derivation failed");
            "Key derivation failed".to_string()
        })?;
    Ok(key)
}

/// Create an encrypted backup of the given accounts.
/// Returns raw bytes in the Ghost Auth backup format:
/// MAGIC(4) + VERSION(1) + SALT(16) + NONCE(12) + CIPHERTEXT
pub fn export_accounts(accounts: &[Account], password: &str) -> Result<Vec<u8>, String> {
    if password.len() < 8 {
        return Err("Backup password must be at least 8 characters".to_string());
    }

    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt)?;

    let exported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let payload = BackupPayload {
        version: FORMAT_VERSION,
        exported_at,
        accounts: accounts.to_vec(),
    };
    let plaintext = serde_json::to_vec(&payload).map_err(|e| {
        tracing::error!(error = %e, "Backup serialization failed");
        "Failed to create backup".to_string()
    })?;

    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|e| {
        tracing::error!(error = %e, "Cipher initialization failed");
        "Failed to create backup".to_string()
    })?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).map_err(|e| {
        tracing::error!(error = %e, "Backup encryption failed");
        "Failed to create backup".to_string()
    })?;

    let mut output = Vec::with_capacity(4 + 1 + 16 + 12 + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.push(FORMAT_VERSION);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend(ciphertext);

    Ok(output)
}

/// Decrypt a backup file and return the accounts.
pub fn import_accounts(data: &[u8], password: &str) -> Result<Vec<Account>, String> {
    // Minimum: 4 (magic) + 1 (version) + 16 (salt) + 12 (nonce) + 16 (min AES-GCM tag)
    if data.len() < 49 {
        return Err("File is too small to be a valid backup".to_string());
    }

    if &data[0..4] != MAGIC {
        return Err("Not a Ghost Auth backup file".to_string());
    }

    let version = data[4];
    if version != FORMAT_VERSION {
        return Err(format!("Unsupported backup version: {}", version));
    }

    let salt: [u8; 16] = data[5..21]
        .try_into()
        .map_err(|_| "Invalid backup file".to_string())?;
    let nonce_bytes: [u8; 12] = data[21..33]
        .try_into()
        .map_err(|_| "Invalid backup file".to_string())?;
    let ciphertext = &data[33..];

    let key = derive_key(password, &salt)?;

    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|e| {
        tracing::error!(error = %e, "Cipher initialization failed");
        "Failed to decrypt backup".to_string()
    })?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed — wrong password or corrupted file".to_string())?;

    let payload: BackupPayload = serde_json::from_slice(&plaintext).map_err(|e| {
        tracing::error!(error = %e, "Backup deserialization failed");
        "Invalid backup data".to_string()
    })?;

    Ok(payload.accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_accounts() -> Vec<Account> {
        vec![
            Account {
                id: "1".into(),
                issuer: "GitHub".into(),
                label: "user@test.com".into(),
                secret: "JBSWY3DPEHPK3PXP".into(),
                algorithm: "SHA1".into(),
                digits: 6,
                period: 30,
                icon: None,
                last_modified: 0,
            },
            Account {
                id: "2".into(),
                issuer: "Google".into(),
                label: "me@gmail.com".into(),
                secret: "GEZDGNBVGY3TQOJQ".into(),
                algorithm: "SHA256".into(),
                digits: 8,
                period: 30,
                icon: Some("google".into()),
                last_modified: 0,
            },
        ]
    }

    #[test]
    fn test_export_import_roundtrip() {
        let accounts = sample_accounts();
        let password = "strongpassword123";

        let exported = export_accounts(&accounts, password).unwrap();
        let imported = import_accounts(&exported, password).unwrap();

        assert_eq!(imported.len(), accounts.len());
        for (got, want) in imported.iter().zip(accounts.iter()) {
            assert_eq!(got.id, want.id);
            assert_eq!(got.issuer, want.issuer);
            assert_eq!(got.label, want.label);
            assert_eq!(got.secret, want.secret);
            assert_eq!(got.algorithm, want.algorithm);
            assert_eq!(got.digits, want.digits);
            assert_eq!(got.period, want.period);
            assert_eq!(got.icon, want.icon);
            assert_eq!(got.last_modified, want.last_modified);
        }
    }

    #[test]
    fn test_wrong_password_fails() {
        let accounts = sample_accounts();
        let exported = export_accounts(&accounts, "correctpassword").unwrap();
        let result = import_accounts(&exported, "wrongpassword1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("wrong password"));
    }

    #[test]
    fn test_corrupted_data_fails() {
        let accounts = sample_accounts();
        let mut exported = export_accounts(&accounts, "password1234").unwrap();
        let last = exported.len() - 1;
        exported[last] ^= 0xFF;
        assert!(import_accounts(&exported, "password1234").is_err());
    }

    #[test]
    fn test_too_short_data_fails() {
        assert!(import_accounts(&[0u8; 10], "password").is_err());
    }

    #[test]
    fn test_wrong_magic_fails() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(b"XXXX");
        let err = import_accounts(&data, "password").unwrap_err();
        assert!(err.contains("Not a Ghost Auth backup"));
    }

    #[test]
    fn test_short_password_rejected() {
        let result = export_accounts(&sample_accounts(), "short1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 8"));
    }

    #[test]
    fn test_empty_accounts_roundtrip() {
        let exported = export_accounts(&[], "password1234").unwrap();
        let imported = import_accounts(&exported, "password1234").unwrap();
        assert!(imported.is_empty());
    }

    /// Golden file test: a hardcoded backup blob created by export_accounts() must
    /// always decrypt to the exact same accounts. If this test breaks, the binary
    /// format has drifted and existing .ghostauth files in the wild will be unreadable.
    #[test]
    fn test_golden_file_import() {
        let hex = "4748535401290854456e705936cbfa217211dc9b30ae9e180348279b2e1ab6cea883018bf5b14a3b9a116a44b3ec13f0e0ebd68e170b5cd1453ce7d0aacf6cf5427e42a8f0b24f51c08a04fb9c1c1d532d1bac62d995280f1498737d1827d3100d22accda4848a04eb7cf50abc552e83607f255dd0309eef2030f6c4d6ee8e3fd9ae21553509e0085c1774acd3bb25e9e3a9a981f2d3d133f86be882770c8c2274ac04b486ab789c03505d11708c9c6356ece813efc5ffa832d00240ea17d0b17f2bbeb6798d9487d67f5c1a5cdceb25197ac6a35bba1d335512a8a67e5e832e47c3c5dbf45e9be70937837c7068f8c7ba0eac6807fb43d7e43f38407119c7661dcbbbfe8d7803c81997209a93bd068189fb379635301646dd65416e76dc95591d3e1c149bfec235c42abe4ae9915ba2accbab6a95204712744659ea3a20e43824033e6581659826e7f040cddb9f31c64a12770e26d044c468eaff066188017890a95d158a4c352f9c8a59873548a69deca7e64ed5a93e29d37fdcdea88faa6a99e27e2cfb21181762b71f637bc7c2fedc7da250a8bdb0e0890c5f9930f59b67ae43217c88c2e973ca86fe8fdeebf886597277823f10f00478d03b1fc08a2ae140";
        let data: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();

        let password = "ghost-test-password-1234";
        let accounts = import_accounts(&data, password).unwrap();

        assert_eq!(accounts.len(), 2);

        // Account 1: GitHub (SHA1, 6 digits, no icon)
        assert_eq!(accounts[0].id, "a1b2c3d4");
        assert_eq!(accounts[0].issuer, "GitHub");
        assert_eq!(accounts[0].label, "user@example.com");
        assert_eq!(accounts[0].secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(accounts[0].algorithm, "SHA1");
        assert_eq!(accounts[0].digits, 6);
        assert_eq!(accounts[0].period, 30);
        assert_eq!(accounts[0].icon, None);
        assert_eq!(accounts[0].last_modified, 1700000000);

        // Account 2: Google (SHA256, 8 digits, with icon)
        assert_eq!(accounts[1].id, "e5f6g7h8");
        assert_eq!(accounts[1].issuer, "Google");
        assert_eq!(accounts[1].label, "alice@gmail.com");
        assert_eq!(accounts[1].secret, "GEZDGNBVGY3TQOJQ");
        assert_eq!(accounts[1].algorithm, "SHA256");
        assert_eq!(accounts[1].digits, 8);
        assert_eq!(accounts[1].period, 30);
        assert_eq!(accounts[1].icon, Some("google".into()));
        assert_eq!(accounts[1].last_modified, 1700000001);
    }
}
