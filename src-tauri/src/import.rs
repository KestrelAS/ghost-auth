use crate::google_auth_proto;
use crate::storage::Account;
use prost::Message;
use serde::Deserialize;

#[derive(Debug)]
pub struct ImportResult {
    pub format: String,
    pub accounts: Vec<Account>,
    pub skipped: usize,
}

/// Auto-detect the import format and parse accounts from the file data.
pub fn parse_import(data: &[u8]) -> Result<ImportResult, String> {
    let text = std::str::from_utf8(data)
        .map_err(|_| "File is not valid UTF-8 text")?;
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Err("File is empty".to_string());
    }

    // Google Auth migration URI
    if trimmed.starts_with("otpauth-migration://") {
        return parse_google_auth_migration(trimmed);
    }

    // Plain otpauth:// URI list
    if trimmed.starts_with("otpauth://") {
        return parse_otpauth_uri_list(trimmed);
    }

    // JSON formats
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return parse_json_import(trimmed);
    }

    Err("Unrecognized file format. Supported: Aegis, 2FAS, andOTP, Google Authenticator, otpauth:// URI list".to_string())
}

fn parse_json_import(text: &str) -> Result<ImportResult, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("Invalid JSON: {e}"))?;

    if let Some(obj) = value.as_object() {
        if obj.contains_key("db") {
            return parse_aegis(text);
        }
        if obj.contains_key("services") {
            return parse_twofas(text);
        }
        return Err(
            "Unrecognized JSON format. Expected Aegis (\"db\" key) or 2FAS (\"services\" key)."
                .to_string(),
        );
    }

    if value.is_array() {
        return parse_andotp(text);
    }

    Err("Unrecognized JSON structure".to_string())
}

// --- Aegis ---

#[derive(Deserialize)]
struct AegisExport {
    db: AegisDb,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(untagged)]
enum AegisDb {
    Plaintext(AegisDbPlaintext),
    Encrypted(String),
}

#[derive(Deserialize)]
struct AegisDbPlaintext {
    entries: Vec<AegisEntry>,
}

#[derive(Deserialize)]
struct AegisEntry {
    #[serde(rename = "type")]
    entry_type: String,
    name: Option<String>,
    issuer: Option<String>,
    info: AegisInfo,
}

#[derive(Deserialize)]
struct AegisInfo {
    secret: String,
    algo: Option<String>,
    digits: Option<u32>,
    period: Option<u32>,
}

fn parse_aegis(text: &str) -> Result<ImportResult, String> {
    let export: AegisExport =
        serde_json::from_str(text).map_err(|e| format!("Failed to parse Aegis JSON: {e}"))?;

    let entries = match export.db {
        AegisDb::Plaintext(db) => db.entries,
        AegisDb::Encrypted(_) => {
            return Err(
                "This Aegis backup is encrypted. Please export an unencrypted backup from Aegis."
                    .to_string(),
            );
        }
    };

    let mut accounts = Vec::new();
    let mut skipped = 0;

    for entry in entries {
        if entry.entry_type.to_lowercase() != "totp" {
            skipped += 1;
            continue;
        }

        let secret = normalize_secret(&entry.info.secret);
        if secret.is_empty() {
            skipped += 1;
            continue;
        }

        let algorithm = normalize_algorithm(entry.info.algo.as_deref().unwrap_or("SHA1"));
        let digits = entry.info.digits.unwrap_or(6);
        let period = entry.info.period.unwrap_or(30);

        if !is_valid_account(&algorithm, digits, period) {
            skipped += 1;
            continue;
        }

        accounts.push(Account {
            id: uuid::Uuid::new_v4().to_string(),
            issuer: entry.issuer.unwrap_or_default(),
            label: entry.name.unwrap_or_default(),
            secret,
            algorithm,
            digits,
            period,
            icon: None,
            last_modified: 0,
        });
    }

    Ok(ImportResult {
        format: "Aegis".to_string(),
        accounts,
        skipped,
    })
}

// --- 2FAS ---

#[derive(Deserialize)]
struct TwoFASExport {
    services: Vec<TwoFASService>,
}

#[derive(Deserialize)]
struct TwoFASService {
    name: Option<String>,
    secret: Option<String>,
    otp: Option<TwoFASOtp>,
}

#[derive(Deserialize)]
struct TwoFASOtp {
    issuer: Option<String>,
    account: Option<String>,
    algorithm: Option<String>,
    period: Option<u32>,
    digits: Option<u32>,
    #[serde(rename = "tokenType")]
    token_type: Option<String>,
}

fn parse_twofas(text: &str) -> Result<ImportResult, String> {
    let export: TwoFASExport =
        serde_json::from_str(text).map_err(|e| format!("Failed to parse 2FAS JSON: {e}"))?;

    let mut accounts = Vec::new();
    let mut skipped = 0;

    for service in export.services {
        let otp = match &service.otp {
            Some(otp) => otp,
            None => {
                skipped += 1;
                continue;
            }
        };

        // Skip non-TOTP
        if let Some(ref token_type) = otp.token_type {
            if token_type.to_uppercase() != "TOTP" {
                skipped += 1;
                continue;
            }
        }

        let raw_secret = service.secret.as_deref().unwrap_or("");
        let secret = normalize_secret(raw_secret);
        if secret.is_empty() {
            skipped += 1;
            continue;
        }

        let algorithm = normalize_algorithm(otp.algorithm.as_deref().unwrap_or("SHA1"));
        let digits = otp.digits.unwrap_or(6);
        let period = otp.period.unwrap_or(30);

        if !is_valid_account(&algorithm, digits, period) {
            skipped += 1;
            continue;
        }

        let issuer = otp
            .issuer
            .clone()
            .or(service.name.clone())
            .unwrap_or_default();
        let label = otp.account.clone().unwrap_or_default();

        accounts.push(Account {
            id: uuid::Uuid::new_v4().to_string(),
            issuer,
            label,
            secret,
            algorithm,
            digits,
            period,
            icon: None,
            last_modified: 0,
        });
    }

    Ok(ImportResult {
        format: "2FAS".to_string(),
        accounts,
        skipped,
    })
}

// --- andOTP ---

#[derive(Deserialize)]
struct AndOTPEntry {
    secret: String,
    label: Option<String>,
    issuer: Option<String>,
    period: Option<u32>,
    digits: Option<u32>,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    algorithm: Option<String>,
}

fn parse_andotp(text: &str) -> Result<ImportResult, String> {
    let entries: Vec<AndOTPEntry> =
        serde_json::from_str(text).map_err(|e| format!("Failed to parse andOTP JSON: {e}"))?;

    let mut accounts = Vec::new();
    let mut skipped = 0;

    for entry in entries {
        let entry_type = entry.entry_type.as_deref().unwrap_or("TOTP");
        if entry_type.to_uppercase() != "TOTP" {
            skipped += 1;
            continue;
        }

        let secret = normalize_secret(&entry.secret);
        if secret.is_empty() {
            skipped += 1;
            continue;
        }

        let algorithm = normalize_algorithm(entry.algorithm.as_deref().unwrap_or("SHA1"));
        let digits = entry.digits.unwrap_or(6);
        let period = entry.period.unwrap_or(30);

        if !is_valid_account(&algorithm, digits, period) {
            skipped += 1;
            continue;
        }

        // andOTP uses "label" which may contain "issuer:label" format
        let (issuer, label) = if let Some(ref issuer) = entry.issuer {
            (issuer.clone(), entry.label.unwrap_or_default())
        } else if let Some(ref raw_label) = entry.label {
            split_issuer_label(raw_label)
        } else {
            (String::new(), String::new())
        };

        accounts.push(Account {
            id: uuid::Uuid::new_v4().to_string(),
            issuer,
            label,
            secret,
            algorithm,
            digits,
            period,
            icon: None,
            last_modified: 0,
        });
    }

    Ok(ImportResult {
        format: "andOTP".to_string(),
        accounts,
        skipped,
    })
}

// --- Google Authenticator migration ---

fn parse_google_auth_migration(uri: &str) -> Result<ImportResult, String> {
    // Extract the data parameter from the URI
    let data_start = uri
        .find("data=")
        .ok_or("Missing 'data' parameter in migration URI")?
        + 5;

    let data_param = &uri[data_start..];
    // Handle case where there might be other params after data
    let data_param = data_param.split('&').next().unwrap_or(data_param);

    // URL-decode then base64-decode
    let url_decoded = percent_decode(data_param)?;
    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        url_decoded.as_bytes(),
    )
    .map_err(|e| format!("Failed to decode base64 migration data: {e}"))?;

    let payload = google_auth_proto::MigrationPayload::decode(bytes.as_slice())
        .map_err(|e| format!("Failed to decode protobuf migration data: {e}"))?;

    let mut accounts = Vec::new();
    let mut skipped = 0;

    for param in payload.otp_parameters {
        // Only import TOTP (type == 2)
        if param.otp_type != google_auth_proto::OtpType::Totp as i32 {
            skipped += 1;
            continue;
        }

        // Encode raw secret bytes to Base32 no-pad
        let secret = data_encoding::BASE32_NOPAD.encode(&param.secret);
        if secret.is_empty() {
            skipped += 1;
            continue;
        }

        let algorithm = match param.algorithm {
            x if x == google_auth_proto::Algorithm::Sha1 as i32 => "SHA1",
            x if x == google_auth_proto::Algorithm::Sha256 as i32 => "SHA256",
            x if x == google_auth_proto::Algorithm::Sha512 as i32 => "SHA512",
            0 => "SHA1", // Unspecified defaults to SHA1
            _ => {
                skipped += 1;
                continue;
            }
        }
        .to_string();

        let digits: u32 = match param.digits {
            x if x == google_auth_proto::DigitCount::Six as i32 => 6,
            x if x == google_auth_proto::DigitCount::Eight as i32 => 8,
            0 => 6, // Unspecified defaults to 6
            _ => {
                skipped += 1;
                continue;
            }
        };

        // Parse issuer from name if needed (format: "issuer:label")
        let (issuer, label) = if !param.issuer.is_empty() {
            // If the name starts with "issuer:", strip that prefix for the label
            let label = param
                .name
                .strip_prefix(&format!("{}:", param.issuer))
                .unwrap_or(&param.name)
                .trim()
                .to_string();
            (param.issuer, label)
        } else {
            split_issuer_label(&param.name)
        };

        accounts.push(Account {
            id: uuid::Uuid::new_v4().to_string(),
            issuer,
            label,
            secret,
            algorithm,
            digits,
            period: 30,
            icon: None,
            last_modified: 0,
        });
    }

    Ok(ImportResult {
        format: "Google Authenticator".to_string(),
        accounts,
        skipped,
    })
}

// --- otpauth:// URI list ---

fn parse_otpauth_uri_list(text: &str) -> Result<ImportResult, String> {
    let mut accounts = Vec::new();
    let mut skipped = 0;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with("otpauth://totp/") {
            if line.starts_with("otpauth://") {
                // Non-TOTP otpauth URI (e.g., hotp)
                skipped += 1;
            }
            continue;
        }

        match crate::totp::parse_otpauth_uri(line) {
            Ok(account) => accounts.push(account),
            Err(e) => {
                tracing::warn!(error = %e, line = %line, "Skipping invalid otpauth URI");
                skipped += 1;
            }
        }
    }

    if accounts.is_empty() && skipped == 0 {
        return Err("No otpauth:// URIs found in file".to_string());
    }

    Ok(ImportResult {
        format: "otpauth:// URI list".to_string(),
        accounts,
        skipped,
    })
}

// --- Helpers ---

/// Normalize a Base32 secret: remove spaces, uppercase, strip padding.
fn normalize_secret(secret: &str) -> String {
    secret
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .collect::<String>()
        .to_uppercase()
}

/// Normalize algorithm string to canonical form.
fn normalize_algorithm(algo: &str) -> String {
    match algo.to_uppercase().as_str() {
        "SHA1" | "SHA-1" | "HMACSHA1" => "SHA1".to_string(),
        "SHA256" | "SHA-256" | "HMACSHA256" => "SHA256".to_string(),
        "SHA512" | "SHA-512" | "HMACSHA512" => "SHA512".to_string(),
        _ => algo.to_uppercase(),
    }
}

/// Validate that account fields are within acceptable ranges.
fn is_valid_account(algorithm: &str, digits: u32, period: u32) -> bool {
    matches!(algorithm, "SHA1" | "SHA256" | "SHA512")
        && (digits == 6 || digits == 8)
        && (15..=120).contains(&period)
}

/// Split "issuer:label" format into (issuer, label).
fn split_issuer_label(combined: &str) -> (String, String) {
    if let Some((issuer, label)) = combined.split_once(':') {
        (issuer.trim().to_string(), label.trim().to_string())
    } else {
        (combined.trim().to_string(), String::new())
    }
}

/// Simple percent-decoding for URL query parameters.
fn percent_decode(input: &str) -> Result<String, String> {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &input[i + 1..i + 3];
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| format!("Invalid percent-encoding: %{hex}"))?;
            result.push(byte);
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(result).map_err(|_| "Invalid UTF-8 after percent-decoding".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_secret() {
        assert_eq!(normalize_secret("JBSW Y3DP"), "JBSWY3DP");
        assert_eq!(normalize_secret("jbswy3dp"), "JBSWY3DP");
        assert_eq!(normalize_secret("JBSWY3DP===="), "JBSWY3DP");
        assert_eq!(normalize_secret("  jbsw y3dp == "), "JBSWY3DP");
    }

    #[test]
    fn test_normalize_algorithm() {
        assert_eq!(normalize_algorithm("sha1"), "SHA1");
        assert_eq!(normalize_algorithm("SHA-256"), "SHA256");
        assert_eq!(normalize_algorithm("HmacSHA512"), "SHA512");
    }

    #[test]
    fn test_split_issuer_label() {
        let (i, l) = split_issuer_label("GitHub:user@example.com");
        assert_eq!(i, "GitHub");
        assert_eq!(l, "user@example.com");

        let (i, l) = split_issuer_label("JustIssuer");
        assert_eq!(i, "JustIssuer");
        assert_eq!(l, "");
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world").unwrap(), "hello world");
        assert_eq!(percent_decode("a%2Fb%3Dc").unwrap(), "a/b=c");
        assert_eq!(percent_decode("keeps+literal").unwrap(), "keeps+literal");
        assert_eq!(percent_decode("plain").unwrap(), "plain");
    }

    #[test]
    fn test_is_valid_account() {
        assert!(is_valid_account("SHA1", 6, 30));
        assert!(is_valid_account("SHA256", 8, 60));
        assert!(!is_valid_account("MD5", 6, 30));
        assert!(!is_valid_account("SHA1", 7, 30));
        assert!(!is_valid_account("SHA1", 6, 10));
    }

    #[test]
    fn test_parse_aegis() {
        let json = r#"{
            "version": 1,
            "header": {"slots": null, "params": null},
            "db": {
                "version": 3,
                "entries": [
                    {
                        "type": "totp",
                        "name": "user@example.com",
                        "issuer": "GitHub",
                        "info": {
                            "secret": "JBSWY3DPEHPK3PXP",
                            "algo": "SHA1",
                            "digits": 6,
                            "period": 30
                        }
                    },
                    {
                        "type": "totp",
                        "name": "alice",
                        "issuer": "Google",
                        "info": {
                            "secret": "GEZDGNBVGY3TQOJQ",
                            "algo": "SHA256",
                            "digits": 8,
                            "period": 60
                        }
                    },
                    {
                        "type": "hotp",
                        "name": "counter-based",
                        "issuer": "Other",
                        "info": {
                            "secret": "JBSWY3DPEHPK3PXP",
                            "algo": "SHA1",
                            "digits": 6,
                            "counter": 0
                        }
                    }
                ]
            }
        }"#;

        let result = parse_aegis(json).unwrap();
        assert_eq!(result.format, "Aegis");
        assert_eq!(result.accounts.len(), 2);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.accounts[0].issuer, "GitHub");
        assert_eq!(result.accounts[0].label, "user@example.com");
        assert_eq!(result.accounts[0].algorithm, "SHA1");
        assert_eq!(result.accounts[0].digits, 6);
        assert_eq!(result.accounts[1].issuer, "Google");
        assert_eq!(result.accounts[1].digits, 8);
        assert_eq!(result.accounts[1].period, 60);
    }

    #[test]
    fn test_parse_aegis_encrypted_rejected() {
        let json = r#"{"version":1,"header":{"slots":[],"params":{}},"db":"base64ciphertext"}"#;
        let result = parse_aegis(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("encrypted"));
    }

    #[test]
    fn test_parse_twofas() {
        let json = r#"{
            "services": [
                {
                    "name": "GitLab",
                    "secret": "JBSWY3DPEHPK3PXP",
                    "otp": {
                        "account": "user@gitlab.com",
                        "period": 30,
                        "algorithm": "SHA1",
                        "issuer": "GitLab",
                        "tokenType": "TOTP",
                        "digits": 6
                    }
                },
                {
                    "name": "Steam",
                    "secret": "ABCDEFGHIJ234567",
                    "otp": {
                        "tokenType": "STEAM",
                        "digits": 5
                    }
                }
            ]
        }"#;

        let result = parse_twofas(json).unwrap();
        assert_eq!(result.format, "2FAS");
        assert_eq!(result.accounts.len(), 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.accounts[0].issuer, "GitLab");
        assert_eq!(result.accounts[0].label, "user@gitlab.com");
    }

    #[test]
    fn test_parse_andotp() {
        let json = r#"[
            {
                "secret": "JBSWY3DPEHPK3PXP",
                "issuer": "TestService",
                "label": "testuser",
                "period": 30,
                "digits": 6,
                "type": "TOTP",
                "algorithm": "SHA1"
            },
            {
                "secret": "GEZDGNBVGY3TQOJQ",
                "label": "GitHub:user@example.com",
                "period": 30,
                "digits": 6,
                "type": "TOTP",
                "algorithm": "SHA1"
            },
            {
                "secret": "AAAABBBBCCCCDDDD",
                "label": "counter",
                "type": "HOTP",
                "algorithm": "SHA1",
                "digits": 6
            }
        ]"#;

        let result = parse_andotp(json).unwrap();
        assert_eq!(result.format, "andOTP");
        assert_eq!(result.accounts.len(), 2);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.accounts[0].issuer, "TestService");
        assert_eq!(result.accounts[0].label, "testuser");
        // Second entry: issuer parsed from "label" field
        assert_eq!(result.accounts[1].issuer, "GitHub");
        assert_eq!(result.accounts[1].label, "user@example.com");
    }

    #[test]
    fn test_parse_otpauth_uri_list() {
        let text = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub\n\
                    # comment line\n\
                    \n\
                    otpauth://totp/Google:alice?secret=GEZDGNBVGY3TQOJQ&issuer=Google\n\
                    otpauth://hotp/Counter:test?secret=AAAABBBB&counter=0\n";

        let result = parse_otpauth_uri_list(text).unwrap();
        assert_eq!(result.format, "otpauth:// URI list");
        assert_eq!(result.accounts.len(), 2);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.accounts[0].issuer, "GitHub");
        assert_eq!(result.accounts[1].issuer, "Google");
    }

    #[test]
    fn test_parse_google_auth_migration() {
        // Build a real protobuf payload for testing
        let payload = google_auth_proto::MigrationPayload {
            otp_parameters: vec![
                google_auth_proto::OtpParameters {
                    secret: b"Hello!".to_vec(), // "JBSWY3DPBI" in Base32
                    name: "GitHub:user@example.com".to_string(),
                    issuer: "GitHub".to_string(),
                    algorithm: google_auth_proto::Algorithm::Sha1 as i32,
                    digits: google_auth_proto::DigitCount::Six as i32,
                    otp_type: google_auth_proto::OtpType::Totp as i32,
                    counter: 0,
                },
                google_auth_proto::OtpParameters {
                    secret: b"World!".to_vec(),
                    name: "HOTP:counter".to_string(),
                    issuer: "HOTP".to_string(),
                    algorithm: google_auth_proto::Algorithm::Sha1 as i32,
                    digits: google_auth_proto::DigitCount::Six as i32,
                    otp_type: google_auth_proto::OtpType::Hotp as i32,
                    counter: 0,
                },
            ],
            version: 1,
            batch_size: 1,
            batch_index: 0,
            batch_id: 0,
        };

        let mut buf = Vec::new();
        payload.encode(&mut buf).unwrap();

        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);
        let uri = format!("otpauth-migration://offline?data={b64}");

        let result = parse_google_auth_migration(&uri).unwrap();
        assert_eq!(result.format, "Google Authenticator");
        assert_eq!(result.accounts.len(), 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.accounts[0].issuer, "GitHub");
        assert_eq!(result.accounts[0].label, "user@example.com");
        assert_eq!(result.accounts[0].algorithm, "SHA1");
        assert_eq!(result.accounts[0].digits, 6);
        assert_eq!(result.accounts[0].period, 30);
    }

    #[test]
    fn test_auto_detect_aegis() {
        let json = r#"{"db":{"entries":[]}}"#;
        let result = parse_import(json.as_bytes()).unwrap();
        assert_eq!(result.format, "Aegis");
    }

    #[test]
    fn test_auto_detect_twofas() {
        let json = r#"{"services":[]}"#;
        let result = parse_import(json.as_bytes()).unwrap();
        assert_eq!(result.format, "2FAS");
    }

    #[test]
    fn test_auto_detect_andotp() {
        let json = r#"[]"#;
        let result = parse_import(json.as_bytes()).unwrap();
        assert_eq!(result.format, "andOTP");
    }

    #[test]
    fn test_empty_file_error() {
        let result = parse_import(b"");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_unrecognized_format_error() {
        let result = parse_import(b"this is not a valid format");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unrecognized"));
    }
}
