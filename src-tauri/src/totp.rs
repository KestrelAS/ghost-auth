use crate::storage::Account;
use serde::Serialize;
use totp_rs::{Algorithm, Secret, TOTP};

#[derive(Serialize, Clone)]
pub struct CodeResponse {
    pub id: String,
    pub code: String,
    pub remaining: u32,
}

fn to_algorithm(name: &str) -> Result<Algorithm, String> {
    match name.to_uppercase().as_str() {
        "SHA1" => Ok(Algorithm::SHA1),
        "SHA256" => Ok(Algorithm::SHA256),
        "SHA512" => Ok(Algorithm::SHA512),
        _ => Err("Unsupported algorithm".to_string()),
    }
}

pub fn generate_code(account: &Account) -> Result<CodeResponse, String> {
    let algorithm = to_algorithm(&account.algorithm)?;

    let secret_bytes = Secret::Encoded(account.secret.clone())
        .to_bytes()
        .map_err(|e| {
            tracing::warn!(account_id = %account.id, error = %e, "Invalid TOTP secret");
            "Invalid account secret".to_string()
        })?;

    // Use new_unchecked to support real-world secrets that may be < 128 bits
    let totp = TOTP::new_unchecked(
        algorithm,
        account.digits as usize,
        1,
        account.period as u64,
        secret_bytes,
        Some(account.issuer.clone()),
        account.label.clone(),
    );

    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "System clock error".to_string())?
        .as_secs();

    let code = totp.generate(time);
    let remaining = (account.period as u64 - (time % account.period as u64)) as u32;

    Ok(CodeResponse {
        id: account.id.clone(),
        code,
        remaining,
    })
}

pub fn parse_otpauth_uri(uri: &str) -> Result<Account, String> {
    let totp = TOTP::from_url_unchecked(uri).map_err(|e| {
        tracing::warn!(error = ?e, "Invalid otpauth URI");
        "Invalid QR code or URI format".to_string()
    })?;

    let algorithm = match totp.algorithm {
        Algorithm::SHA1 => "SHA1",
        Algorithm::SHA256 => "SHA256",
        Algorithm::SHA512 => "SHA512",
    }
    .to_string();

    let secret = data_encoding::BASE32_NOPAD.encode(&totp.secret);

    Ok(Account {
        id: uuid::Uuid::new_v4().to_string(),
        issuer: totp.issuer.unwrap_or_default(),
        label: totp.account_name,
        secret,
        algorithm,
        digits: totp.digits as u32,
        period: totp.step as u32,
        icon: None,
        last_modified: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account() -> Account {
        Account {
            id: "test".to_string(),
            issuer: "TestService".to_string(),
            label: "testuser@example.com".to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            algorithm: "SHA1".to_string(),
            digits: 6,
            period: 30,
            icon: None,
            last_modified: 0,
        }
    }

    #[test]
    fn test_generate_code_length() {
        let account = test_account();
        let result = generate_code(&account).unwrap();
        assert_eq!(result.code.len(), 6);
        assert!(result.remaining > 0 && result.remaining <= 30);
    }

    #[test]
    fn test_generate_code_8_digits() {
        let mut account = test_account();
        account.digits = 8;
        let result = generate_code(&account).unwrap();
        assert_eq!(result.code.len(), 8);
    }

    #[test]
    fn test_parse_otpauth_uri() {
        let uri = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA1&digits=6&period=30";
        let account = parse_otpauth_uri(uri).unwrap();
        assert_eq!(account.issuer, "GitHub");
        assert_eq!(account.label, "user@example.com");
        assert_eq!(account.algorithm, "SHA1");
        assert_eq!(account.digits, 6);
        assert_eq!(account.period, 30);
    }

    #[test]
    fn test_parse_otpauth_uri_defaults() {
        let uri = "otpauth://totp/Service:user?secret=JBSWY3DPEHPK3PXP&issuer=Service";
        let account = parse_otpauth_uri(uri).unwrap();
        assert_eq!(account.issuer, "Service");
        assert_eq!(account.digits, 6);
        assert_eq!(account.period, 30);
    }

    #[test]
    fn test_roundtrip_parse_then_generate() {
        let uri = "otpauth://totp/TestService:testuser@example.com?secret=JBSWY3DPEHPK3PXP&issuer=TestService&algorithm=SHA1&digits=6&period=30";
        let account = parse_otpauth_uri(uri).unwrap();
        let result = generate_code(&account).unwrap();
        assert_eq!(result.code.len(), 6);
    }

    #[test]
    fn test_rfc6238_known_secret() {
        // RFC 6238 test secret: "12345678901234567890" -> base32
        let account = Account {
            id: "rfc".to_string(),
            issuer: "RFC".to_string(),
            label: "test".to_string(),
            secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".to_string(),
            algorithm: "SHA1".to_string(),
            digits: 8,
            period: 30,
            icon: None,
            last_modified: 0,
        };
        let result = generate_code(&account).unwrap();
        assert_eq!(result.code.len(), 8);
    }
}
