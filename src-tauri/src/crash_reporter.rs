//! Privacy-preserving crash reporter for Ghost Auth.
//!
//! - Disabled by default; user must opt in via Settings.
//! - DSN injected at compile time: `GHOST_AUTH_DSN_DEV` for debug builds,
//!   `GHOST_AUTH_DSN` for release builds. If absent the module compiles
//!   in but is permanently inert.
//! - All event data passes through an allowlist/denylist sanitizer
//!   before touching disk or network.
//! - Events are AES-256-GCM encrypted on disk and POSTed as Sentry
//!   envelopes to a self-hosted GlitchTip instance.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::keystore;

// ── Constants ───────────────────────────────────────────────────────

const MAX_BREADCRUMBS: usize = 20;
const MAX_QUEUE_SIZE: usize = 10;
const MAX_STACK_FRAMES: usize = 50;
const DRAIN_TIMEOUT_SECS: u64 = 10;

/// Keywords whose associated values must be redacted in all string fields.
const DENYLIST: &[&str] = &[
    "secret",
    "otp",
    "otpauth",
    "issuer",
    "label",
    "pin",
    "recovery",
    "clipboard",
    "account_id",
    "nonce",
    "peer_addr",
    "sync_code",
    "password",
    "key",
    "token",
];

const VALID_FEATURE_AREAS: &[&str] = &[
    "sync", "import", "storage", "keystore", "backup", "pin", "totp", "settings",
];

// ── Static singleton ────────────────────────────────────────────────

static REPORTER: OnceLock<CrashReporter> = OnceLock::new();
/// Consent-file path — set unconditionally at startup so the user preference
/// persists even when no DSN is compiled in (e.g. local dev builds).
static CONSENT_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

// ── Types ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum FeatureArea {
    Sync,
    Import,
    Storage,
    Keystore,
    Backup,
    Pin,
    Totp,
    Settings,
}

impl FeatureArea {
    fn as_str(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Import => "import",
            Self::Storage => "storage",
            Self::Keystore => "keystore",
            Self::Backup => "backup",
            Self::Pin => "pin",
            Self::Totp => "totp",
            Self::Settings => "settings",
        }
    }
}

struct ParsedDsn {
    public_key: String,
    host: String,
    project_id: String,
}

fn parse_dsn(dsn: &str) -> Option<ParsedDsn> {
    let parsed = url::Url::parse(dsn).ok()?;
    let public_key = parsed.username().to_string();
    if public_key.is_empty() {
        return None;
    }
    let host_str = parsed.host_str()?;
    let host = match parsed.port() {
        Some(p) => format!("{}://{}:{}", parsed.scheme(), host_str, p),
        None => format!("{}://{}", parsed.scheme(), host_str),
    };
    let project_id = parsed
        .path()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if project_id.is_empty() {
        return None;
    }
    Some(ParsedDsn {
        public_key,
        host,
        project_id,
    })
}

#[derive(Serialize, Deserialize, Clone)]
struct Breadcrumb {
    timestamp: String,
    level: String,
    category: String,
    message: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct StackFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lineno: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ExceptionMechanism {
    #[serde(rename = "type")]
    mechanism_type: String,
    handled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct ExceptionValue {
    #[serde(rename = "type")]
    exception_type: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mechanism: Option<ExceptionMechanism>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stacktrace: Option<Stacktrace>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Stacktrace {
    frames: Vec<StackFrame>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ExceptionData {
    values: Vec<ExceptionValue>,
}

#[derive(Serialize, Deserialize, Clone)]
struct OsContext {
    name: String,
    version: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct DeviceContext {
    arch: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct AppContext {
    app_version: String,
    build_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct EventContexts {
    os: OsContext,
    device: DeviceContext,
    app: AppContext,
}

#[derive(Serialize, Deserialize, Clone)]
struct BreadcrumbsData {
    values: Vec<Breadcrumb>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CrashEvent {
    event_id: String,
    timestamp: String,
    platform: String,
    level: String,
    release: String,
    environment: String,
    tags: HashMap<String, String>,
    contexts: EventContexts,
    #[serde(skip_serializing_if = "Option::is_none")]
    exception: Option<ExceptionData>,
    breadcrumbs: BreadcrumbsData,
}

// ── CrashReporter ───────────────────────────────────────────────────

struct CrashReporter {
    enabled: AtomicBool,
    dsn: ParsedDsn,
    dsn_raw: String,
    queue_path: PathBuf,
    /// Queue encryption key, lazily derived from the keystore on first use.
    /// `None` until the keystore becomes accessible (after storage init / PIN unlock).
    key: Mutex<Option<[u8; 32]>>,
    breadcrumbs: Mutex<VecDeque<Breadcrumb>>,
}

// ── Sanitizer ───────────────────────────────────────────────────────

fn sanitize_string(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Redact otpauth:// URIs
    loop {
        let lower = result.to_lowercase();
        let Some(pos) = lower.find("otpauth://") else {
            break;
        };
        let end = result[pos..]
            .find(char::is_whitespace)
            .map(|i| pos + i)
            .unwrap_or(result.len());
        result.replace_range(pos..end, "[redacted_uri]");
    }

    // 2. Redact base32 sequences (32+ chars of A-Z2-7 with optional = padding)
    let chars: Vec<char> = result.chars().collect();
    let mut sanitized = String::with_capacity(result.len());
    let mut i = 0;
    while i < chars.len() {
        let is_b32 =
            chars[i].is_ascii_uppercase() || ('2'..='7').contains(&chars[i]) || chars[i] == '=';
        if is_b32 {
            let start = i;
            while i < chars.len() {
                let c = chars[i];
                if c.is_ascii_uppercase() || ('2'..='7').contains(&c) || c == '=' {
                    i += 1;
                } else {
                    break;
                }
            }
            if i - start >= 32 {
                sanitized.push_str("[redacted_b32]");
            } else {
                for &c in &chars[start..i] {
                    sanitized.push(c);
                }
            }
        } else {
            sanitized.push(chars[i]);
            i += 1;
        }
    }
    result = sanitized;

    // 3. Redact home-directory paths → basename only
    let home_patterns = ["/Users/", "/home/", "C:\\Users\\", "C:/Users/"];
    for pattern in &home_patterns {
        while let Some(pos) = result.find(pattern) {
            let end = result[pos..]
                .find(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == ')' || c == ']')
                .map(|i| pos + i)
                .unwrap_or(result.len());
            let path_str = result[pos..end].to_string();
            let basename = path_str
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&path_str)
                .to_string();
            if basename.is_empty() {
                result.replace_range(pos..end, "<home>");
            } else {
                result.replace_range(pos..end, &basename);
            }
        }
    }

    // 4. Redact denylist keywords in key=value / key: value patterns
    for keyword in DENYLIST {
        let kw = *keyword;
        let mut search_from = 0;
        loop {
            let lower = result.to_lowercase();
            let remaining = match lower.get(search_from..) {
                Some(r) => r,
                None => break,
            };
            let Some(rel_pos) = remaining.find(kw) else {
                break;
            };
            let abs_pos = search_from + rel_pos;

            // Word-boundary check
            let before_ok = abs_pos == 0 || !result.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
            let after_pos = abs_pos + kw.len();
            let after_ok =
                after_pos >= result.len() || !result.as_bytes()[after_pos].is_ascii_alphanumeric();

            if before_ok && after_ok && after_pos < result.len() {
                let sep = result.as_bytes()[after_pos] as char;
                if sep == '=' || sep == ':' {
                    // Skip whitespace after separator
                    let mut value_start = after_pos + 1;
                    while value_start < result.len()
                        && result.as_bytes()[value_start].is_ascii_whitespace()
                    {
                        value_start += 1;
                    }
                    // Find end of value
                    let value_end = result[value_start..]
                        .find(|c: char| {
                            c.is_whitespace() || c == ',' || c == '}' || c == ']' || c == ')'
                        })
                        .map(|i| value_start + i)
                        .unwrap_or(result.len());
                    if value_start < value_end {
                        result.replace_range(value_start..value_end, "[redacted]");
                        search_from = value_start + "[redacted]".len();
                        continue;
                    }
                }
            }
            search_from = abs_pos + kw.len();
        }
    }

    result
}

fn sanitize_path(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn sanitize_stack_frame(frame: &StackFrame) -> StackFrame {
    StackFrame {
        function: frame.function.clone(),
        module: frame.module.clone(),
        lineno: frame.lineno,
        filename: frame.filename.as_ref().map(|f| sanitize_path(f)),
    }
}

fn sanitize_event(mut event: CrashEvent) -> CrashEvent {
    // Sanitize exception
    if let Some(ref mut exc) = event.exception {
        for val in &mut exc.values {
            val.value = sanitize_string(&val.value);
            val.exception_type = sanitize_string(&val.exception_type);
            if let Some(ref mut st) = val.stacktrace {
                // Limit frames: keep bottom + top, drop middle
                if st.frames.len() > MAX_STACK_FRAMES {
                    let keep_bottom = MAX_STACK_FRAMES / 2;
                    let keep_top = MAX_STACK_FRAMES - keep_bottom;
                    let len = st.frames.len();
                    let mut trimmed = Vec::with_capacity(MAX_STACK_FRAMES);
                    trimmed.extend_from_slice(&st.frames[..keep_bottom]);
                    trimmed.extend_from_slice(&st.frames[len - keep_top..]);
                    st.frames = trimmed;
                }
                st.frames = st.frames.iter().map(sanitize_stack_frame).collect();
            }
        }
    }

    // Sanitize breadcrumbs
    for crumb in &mut event.breadcrumbs.values {
        crumb.message = sanitize_string(&crumb.message);
    }

    // Validate feature area tag
    if let Some(area) = event.tags.get("feature_area")
        && !VALID_FEATURE_AREAS.contains(&area.as_str())
    {
        event.tags.remove("feature_area");
    }

    // Sanitize all tag values
    event.tags = event
        .tags
        .iter()
        .map(|(k, v)| (k.clone(), sanitize_string(v)))
        .collect();

    event
}

// ── Timestamps ──────────────────────────────────────────────────────

fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days = secs / 86400;
    let day_secs = secs % 86400;
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    let (y, mut rem) = days_to_ymd(days);
    let leap = is_leap(y);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u64;
    for &d in &mdays {
        if rem < d {
            break;
        }
        rem -= d;
        mo += 1;
    }
    let day = rem + 1;
    format!("{y:04}-{mo:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64) {
    let mut y = 1970u64;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if days < dy {
            return (y, days);
        }
        days -= dy;
        y += 1;
    }
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

// ── OS context ──────────────────────────────────────────────────────

fn collect_contexts() -> EventContexts {
    EventContexts {
        os: OsContext {
            name: std::env::consts::OS.to_string(),
            version: os_version(),
        },
        device: DeviceContext {
            arch: std::env::consts::ARCH.to_string(),
        },
        app: AppContext {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            build_type: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
            .to_string(),
        },
    }
}

#[cfg(target_os = "windows")]
fn os_version() -> String {
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(target_os = "macos")]
fn os_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(target_os = "linux")]
fn os_version() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("VERSION_ID="))
                .map(|l| {
                    l.trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string()
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn os_version() -> String {
    "unknown".to_string()
}

// ── Encrypted disk queue ────────────────────────────────────────────

fn read_queue(path: &Path, key: &[u8; 32]) -> Vec<CrashEvent> {
    let data = match std::fs::read(path) {
        Ok(d) if d.len() > 12 => d,
        _ => return Vec::new(),
    };
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let Ok(cipher) = Aes256Gcm::new_from_slice(key) else {
        return Vec::new();
    };
    let nonce = Nonce::from_slice(nonce_bytes);
    let Ok(plaintext) = cipher.decrypt(nonce, ciphertext) else {
        return Vec::new();
    };
    serde_json::from_slice(&plaintext).unwrap_or_default()
}

fn write_queue(path: &Path, key: &[u8; 32], events: &[CrashEvent]) -> Result<(), String> {
    let plaintext =
        serde_json::to_vec(events).map_err(|e| format!("Failed to serialize events: {e}"))?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init failed: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut data = Vec::with_capacity(12 + ciphertext.len());
    data.extend_from_slice(&nonce_bytes);
    data.extend(ciphertext);

    let tmp_path = path.with_extension("enc.tmp");
    std::fs::write(&tmp_path, &data).map_err(|e| format!("Write failed: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("Rename failed: {e}")
    })
}

fn append_to_queue(reporter: &CrashReporter, event: CrashEvent) {
    let Some(key) = get_queue_key(reporter) else {
        return;
    };
    let mut events = read_queue(&reporter.queue_path, &key);
    events.push(event);
    while events.len() > MAX_QUEUE_SIZE {
        events.remove(0);
    }
    let _ = write_queue(&reporter.queue_path, &key, &events);
}

// ── Sentry envelope ─────────────────────────────────────────────────

fn build_envelope(event: &CrashEvent, _dsn: &ParsedDsn, dsn_raw: &str) -> Vec<u8> {
    let payload = serde_json::to_string(event).unwrap_or_default();
    let envelope_header = serde_json::json!({
        "event_id": event.event_id,
        "dsn": dsn_raw,
        "sent_at": now_rfc3339(),
        "sdk": { "name": "ghost-auth", "version": env!("CARGO_PKG_VERSION") }
    });
    let item_header = serde_json::json!({
        "type": "event",
        "length": payload.len()
    });

    let mut buf = Vec::new();
    buf.extend_from_slice(envelope_header.to_string().as_bytes());
    buf.push(b'\n');
    buf.extend_from_slice(item_header.to_string().as_bytes());
    buf.push(b'\n');
    buf.extend_from_slice(payload.as_bytes());
    buf
}

// ── Tracing breadcrumb layer ────────────────────────────────────────

pub struct BreadcrumbLayer;

struct BreadcrumbVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl BreadcrumbVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: Vec::new(),
        }
    }

    fn into_message(self) -> String {
        let mut msg = self.message;
        for (k, v) in self.fields {
            if !msg.is_empty() {
                msg.push_str(", ");
            }
            msg.push_str(&k);
            msg.push('=');
            msg.push_str(&v);
        }
        msg
    }
}

impl tracing::field::Visit for BreadcrumbVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let val = format!("{value:?}");
        if field.name() == "message" {
            self.message = val;
        } else {
            self.fields.push((field.name().to_string(), val));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for BreadcrumbLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing::Level;
        let level = *event.metadata().level();
        if level != Level::WARN && level != Level::ERROR {
            return;
        }
        let Some(reporter) = REPORTER.get() else {
            return;
        };
        if !reporter.enabled.load(Ordering::Relaxed) {
            return;
        }

        let mut visitor = BreadcrumbVisitor::new();
        event.record(&mut visitor);

        let breadcrumb = Breadcrumb {
            timestamp: now_rfc3339(),
            level: if level == Level::WARN {
                "warning"
            } else {
                "error"
            }
            .to_string(),
            category: "tracing".to_string(),
            message: sanitize_string(&visitor.into_message()),
        };

        if let Ok(mut crumbs) = reporter.breadcrumbs.lock() {
            if crumbs.len() >= MAX_BREADCRUMBS {
                crumbs.pop_front();
            }
            crumbs.push_back(breadcrumb);
        }
    }
}

// ── Event builder ───────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_event(
    level: &str,
    exception_type: &str,
    message: &str,
    area: Option<FeatureArea>,
    mechanism_type: &str,
    handled: bool,
    location_file: Option<&str>,
    location_line: Option<u32>,
    breadcrumbs: Vec<Breadcrumb>,
) -> CrashEvent {
    let event_id = uuid::Uuid::new_v4().as_simple().to_string();

    let mut frames = Vec::new();
    if let Some(file) = location_file {
        frames.push(StackFrame {
            function: None,
            module: None,
            lineno: location_line,
            filename: Some(file.to_string()),
        });
    }

    let mut tags = HashMap::new();
    if let Some(area) = area {
        tags.insert("feature_area".to_string(), area.as_str().to_string());
    }

    CrashEvent {
        event_id,
        timestamp: now_rfc3339(),
        platform: "other".to_string(),
        level: level.to_string(),
        release: format!("ghost-auth@{}", env!("CARGO_PKG_VERSION")),
        environment: if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        }
        .to_string(),
        tags,
        contexts: collect_contexts(),
        exception: Some(ExceptionData {
            values: vec![ExceptionValue {
                exception_type: exception_type.to_string(),
                value: message.to_string(),
                mechanism: Some(ExceptionMechanism {
                    mechanism_type: mechanism_type.to_string(),
                    handled,
                }),
                stacktrace: if frames.is_empty() {
                    None
                } else {
                    Some(Stacktrace { frames })
                },
            }],
        }),
        breadcrumbs: BreadcrumbsData {
            values: breadcrumbs,
        },
    }
}

// ── Panic hook ──────────────────────────────────────────────────────

fn capture_panic(info: &std::panic::PanicHookInfo) {
    let Some(reporter) = REPORTER.get() else {
        return;
    };
    if !reporter.enabled.load(Ordering::Relaxed) {
        return;
    }

    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    };

    let (file, line) = info
        .location()
        .map(|l| (Some(l.file()), Some(l.line())))
        .unwrap_or((None, None));

    // try_lock: if the mutex is poisoned/held we skip breadcrumbs
    let breadcrumbs = reporter
        .breadcrumbs
        .try_lock()
        .map(|crumbs| crumbs.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let event = build_event(
        "fatal",
        "panic",
        &message,
        None,
        "panic_hook",
        false,
        file,
        line,
        breadcrumbs,
    );
    let sanitized = sanitize_event(event);
    append_to_queue(reporter, sanitized);
}

// ── Public API ──────────────────────────────────────────────────────

/// Initialize the crash reporter. Returns a breadcrumb tracing layer
/// if the compile-time DSN is present and the keystore is accessible.
/// Call **before** `tracing_subscriber::registry().init()`.
/// Try to derive the queue encryption sub-key from the OS keystore.
/// Returns `None` if the keystore is not yet accessible.
fn derive_queue_key() -> Option<[u8; 32]> {
    let main_key = keystore::load_key()?;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(&main_key).expect("HMAC-SHA256 accepts any key length");
    mac.update(b"ghost-auth-crash-queue");
    let derived = mac.finalize().into_bytes();
    let mut sub_key = [0u8; 32];
    sub_key.copy_from_slice(&derived);
    Some(sub_key)
}

/// Get or lazily initialize the queue encryption key.
fn get_queue_key(reporter: &CrashReporter) -> Option<[u8; 32]> {
    let mut guard = reporter.key.lock().ok()?;
    if let Some(k) = *guard {
        return Some(k);
    }
    let key = derive_queue_key()?;
    *guard = Some(key);
    Some(key)
}

pub fn init(data_dir: &Path) -> Option<BreadcrumbLayer> {
    // Always store the consent path so the user preference can be
    // persisted / read even when no DSN is compiled in.
    let consent_path = data_dir.join("crash_reporting_enabled");
    let _ = CONSENT_PATH.set(consent_path.clone());

    #[cfg(debug_assertions)]
    let dsn_str = option_env!("GHOST_AUTH_DSN_DEV")?;
    #[cfg(not(debug_assertions))]
    let dsn_str = option_env!("GHOST_AUTH_DSN")?;
    let dsn = parse_dsn(dsn_str)?;

    // Try to derive queue key now; if keystore isn't ready yet it will
    // be loaded lazily on first queue access (after storage init / PIN unlock).
    let key = derive_queue_key();

    let enabled = std::fs::read_to_string(&consent_path)
        .map(|v| v.trim() == "true")
        .unwrap_or(false);

    let reporter = CrashReporter {
        enabled: AtomicBool::new(enabled),
        dsn,
        dsn_raw: dsn_str.to_string(),
        queue_path: data_dir.join("crash_queue.enc"),
        key: Mutex::new(key),
        breadcrumbs: Mutex::new(VecDeque::with_capacity(MAX_BREADCRUMBS)),
    };
    let _ = REPORTER.set(reporter);

    // Chain panic hook (preserves previous hook)
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        capture_panic(info);
        prev_hook(info);
    }));

    Some(BreadcrumbLayer)
}

/// Report an error explicitly at a critical code path.
#[allow(dead_code)]
pub fn report_error(area: FeatureArea, message: &str) {
    let Some(reporter) = REPORTER.get() else {
        return;
    };
    if !reporter.enabled.load(Ordering::Relaxed) {
        return;
    }

    let breadcrumbs = reporter
        .breadcrumbs
        .try_lock()
        .map(|crumbs| crumbs.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let event = build_event(
        "error",
        "error",
        message,
        Some(area),
        "explicit",
        true,
        None,
        None,
        breadcrumbs,
    );
    let sanitized = sanitize_event(event);
    append_to_queue(reporter, sanitized);
}

/// Drain queued crash events to GlitchTip. Call on startup in a background thread.
pub fn drain_queue() {
    let Some(reporter) = REPORTER.get() else {
        return;
    };
    if !reporter.enabled.load(Ordering::Relaxed) {
        return;
    }

    let Some(key) = get_queue_key(reporter) else {
        return;
    };
    let events = read_queue(&reporter.queue_path, &key);
    if events.is_empty() {
        return;
    }

    let Ok(client) = reqwest::blocking::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(DRAIN_TIMEOUT_SECS))
        .build()
    else {
        return;
    };

    let url = format!(
        "{}/api/{}/envelope/",
        reporter.dsn.host, reporter.dsn.project_id
    );
    let auth_header = format!(
        "Sentry sentry_version=7, sentry_client=ghost-auth/{}, sentry_key={}",
        env!("CARGO_PKG_VERSION"),
        reporter.dsn.public_key,
    );

    let mut keep = Vec::new();
    for event in events {
        let envelope = build_envelope(&event, &reporter.dsn, &reporter.dsn_raw);
        match client
            .post(&url)
            .header("Content-Type", "application/x-sentry-envelope")
            .header("X-Sentry-Auth", &auth_header)
            .body(envelope)
            .send()
        {
            Ok(resp) => match resp.status().as_u16() {
                200..=299 => { /* accepted */ }
                429 | 500..=599 => keep.push(event),
                _ => { /* 4xx: malformed, discard */ }
            },
            Err(_) => keep.push(event),
        }
    }

    if keep.is_empty() {
        let _ = std::fs::remove_file(&reporter.queue_path);
    } else {
        let _ = write_queue(&reporter.queue_path, &key, &keep);
    }
}

/// Toggle crash reporting on or off from Settings.
pub fn set_enabled(enabled: bool) {
    // Persist to the consent file regardless of whether the full reporter
    // is initialized — the preference should survive restarts even when
    // no DSN is compiled in.
    if let Some(path) = CONSENT_PATH.get() {
        if enabled {
            if let Err(e) = std::fs::write(path, "true") {
                tracing::warn!(error = %e, "Failed to write crash reporting consent file");
            }
        } else {
            let _ = std::fs::remove_file(path);
        }
    }

    // Update in-memory state and clear queue if reporter is active.
    if let Some(reporter) = REPORTER.get() {
        reporter.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            let _ = std::fs::remove_file(&reporter.queue_path);
            if let Ok(mut crumbs) = reporter.breadcrumbs.lock() {
                crumbs.clear();
            }
        }
    }
}

/// Check whether crash reporting is currently enabled.
pub fn is_enabled() -> bool {
    // Prefer the in-memory flag from the full reporter when available.
    if let Some(reporter) = REPORTER.get() {
        return reporter.enabled.load(Ordering::Relaxed);
    }
    // Fall back to reading the consent file directly (reporter not
    // initialized — no DSN compiled in, but preference should still work).
    CONSENT_PATH
        .get()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|v| v.trim() == "true")
        .unwrap_or(false)
}

/// Send a test crash report immediately (bypasses queue). Returns Ok on success.
pub fn send_test_report() -> Result<(), String> {
    let reporter = REPORTER
        .get()
        .ok_or("Crash reporter not initialized (no DSN?)")?;

    let breadcrumbs = reporter
        .breadcrumbs
        .try_lock()
        .map(|crumbs| crumbs.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let event = build_event(
        "info",
        "test",
        "Test crash report from Ghost Auth dev mode",
        None,
        "manual",
        true,
        Some(file!()),
        Some(line!()),
        breadcrumbs,
    );
    let sanitized = sanitize_event(event);
    let envelope = build_envelope(&sanitized, &reporter.dsn, &reporter.dsn_raw);

    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(DRAIN_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = format!(
        "{}/api/{}/envelope/",
        reporter.dsn.host, reporter.dsn.project_id
    );
    let auth_header = format!(
        "Sentry sentry_version=7, sentry_client=ghost-auth/{}, sentry_key={}",
        env!("CARGO_PKG_VERSION"),
        reporter.dsn.public_key,
    );

    let resp = client
        .post(&url)
        .header("Content-Type", "application/x-sentry-envelope")
        .header("X-Sentry-Auth", &auth_header)
        .body(envelope)
        .send()
        .map_err(|e| format!("Network error: {e}"))?;

    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("GlitchTip returned HTTP {status}"))
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── DSN parsing ─────────────────────────────────────────────────

    #[test]
    fn dsn_valid() {
        let d = parse_dsn("https://abc123@glitchtip.example.com/1").unwrap();
        assert_eq!(d.public_key, "abc123");
        assert_eq!(d.host, "https://glitchtip.example.com");
        assert_eq!(d.project_id, "1");
    }

    #[test]
    fn dsn_with_port() {
        let d = parse_dsn("https://key@localhost:8000/42").unwrap();
        assert_eq!(d.host, "https://localhost:8000");
        assert_eq!(d.project_id, "42");
    }

    #[test]
    fn dsn_missing_key() {
        assert!(parse_dsn("https://glitchtip.example.com/1").is_none());
    }

    #[test]
    fn dsn_missing_project() {
        assert!(parse_dsn("https://key@glitchtip.example.com/").is_none());
    }

    // ── Sanitizer ───────────────────────────────────────────────────

    #[test]
    fn redacts_otpauth_uri() {
        let input =
            "failed to parse otpauth://totp/GitHub:user?secret=JBSWY3DPEHPK3PXP&issuer=GitHub";
        let out = sanitize_string(input);
        assert!(!out.contains("otpauth://"), "URI must be redacted");
        assert!(out.contains("[redacted_uri]"));
    }

    #[test]
    fn redacts_base32_secret() {
        let input = "loaded secret JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP from storage";
        let out = sanitize_string(input);
        assert!(!out.contains("JBSWY3DPEHPK3PXP"));
        assert!(out.contains("[redacted_b32]"));
    }

    #[test]
    fn short_base32_passes() {
        let input = "error code ABCDEF";
        assert_eq!(sanitize_string(input), input);
    }

    #[test]
    fn redacts_unix_home_path() {
        let input = "failed to read /Users/eirik/Documents/ghost-auth/storage.rs";
        let out = sanitize_string(input);
        assert!(!out.contains("/Users/eirik"));
        assert!(out.contains("storage.rs"));
    }

    #[test]
    fn redacts_windows_home_path() {
        let input = "failed to read C:\\Users\\Eirik\\Documents\\ghost-auth\\storage.rs";
        let out = sanitize_string(input);
        assert!(!out.contains("C:\\Users\\Eirik"));
        assert!(out.contains("storage.rs"));
    }

    #[test]
    fn redacts_denylist_key_value() {
        let input = "loaded secret=MYSECRETVALUE from config";
        let out = sanitize_string(input);
        assert!(!out.contains("MYSECRETVALUE"));
        assert!(out.contains("[redacted]"));
    }

    #[test]
    fn clean_message_unchanged() {
        let input = "Failed to initialize storage at line 42";
        assert_eq!(sanitize_string(input), input);
    }

    #[test]
    fn path_basename() {
        assert_eq!(sanitize_path("/foo/bar/baz.rs"), "baz.rs");
        assert_eq!(sanitize_path("C:\\Users\\test\\file.txt"), "file.txt");
        assert_eq!(sanitize_path("simple.rs"), "simple.rs");
    }

    #[test]
    fn feature_area_strings() {
        assert_eq!(FeatureArea::Sync.as_str(), "sync");
        assert_eq!(FeatureArea::Storage.as_str(), "storage");
        assert_eq!(FeatureArea::Keystore.as_str(), "keystore");
    }

    #[test]
    fn feature_area_validation() {
        let mut event = build_event(
            "error",
            "test",
            "msg",
            None,
            "explicit",
            true,
            None,
            None,
            Vec::new(),
        );
        event
            .tags
            .insert("feature_area".to_string(), "invalid_area".to_string());
        let sanitized = sanitize_event(event);
        assert!(!sanitized.tags.contains_key("feature_area"));
    }

    #[test]
    fn valid_feature_area_kept() {
        let event = build_event(
            "error",
            "test",
            "msg",
            Some(FeatureArea::Storage),
            "explicit",
            true,
            None,
            None,
            Vec::new(),
        );
        let sanitized = sanitize_event(event);
        assert_eq!(sanitized.tags.get("feature_area").unwrap(), "storage");
    }

    // ── Queue ───────────────────────────────────────────────────────

    #[test]
    fn queue_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_queue.enc");
        let key = [0xBB; 32];

        let event = build_event(
            "error",
            "test_error",
            "test message",
            Some(FeatureArea::Storage),
            "explicit",
            true,
            Some("test.rs"),
            Some(42),
            Vec::new(),
        );
        write_queue(&path, &key, std::slice::from_ref(&event)).unwrap();

        let loaded = read_queue(&path, &key);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].event_id, event.event_id);
        assert_eq!(loaded[0].level, "error");
    }

    #[test]
    fn queue_cap_enforcement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_queue.enc");
        let key = [0xCC; 32];

        let mut events: Vec<CrashEvent> = (0..11)
            .map(|i| {
                build_event(
                    "error",
                    "test",
                    &format!("event {i}"),
                    None,
                    "explicit",
                    true,
                    None,
                    None,
                    Vec::new(),
                )
            })
            .collect();

        // FIFO eviction
        while events.len() > MAX_QUEUE_SIZE {
            events.remove(0);
        }
        write_queue(&path, &key, &events).unwrap();

        let loaded = read_queue(&path, &key);
        assert_eq!(loaded.len(), MAX_QUEUE_SIZE);
        assert!(
            loaded[0].exception.as_ref().unwrap().values[0]
                .value
                .contains("event 1")
        );
    }

    #[test]
    fn queue_wrong_key_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_queue.enc");
        let key = [0xDD; 32];

        let event = build_event(
            "error",
            "test",
            "msg",
            None,
            "explicit",
            true,
            None,
            None,
            Vec::new(),
        );
        write_queue(&path, &key, &[event]).unwrap();
        assert!(read_queue(&path, &[0xEE; 32]).is_empty());
    }

    // ── Envelope ────────────────────────────────────────────────────

    #[test]
    fn envelope_format() {
        let event = build_event(
            "error",
            "test_error",
            "test",
            Some(FeatureArea::Sync),
            "explicit",
            true,
            None,
            None,
            Vec::new(),
        );
        let dsn = ParsedDsn {
            public_key: "testkey".to_string(),
            host: "https://glitchtip.example.com".to_string(),
            project_id: "1".to_string(),
        };
        let raw = "https://testkey@glitchtip.example.com/1";
        let envelope = build_envelope(&event, &dsn, raw);
        let text = String::from_utf8(envelope).unwrap();

        let lines: Vec<&str> = text.split('\n').collect();
        assert_eq!(lines.len(), 3, "Envelope must have exactly 3 lines");

        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["event_id"].as_str().unwrap(), event.event_id);

        let item: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(item["type"].as_str().unwrap(), "event");
        assert_eq!(item["length"].as_u64().unwrap(), lines[2].len() as u64);

        let payload: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(payload["event_id"].as_str().unwrap(), event.event_id);
    }

    // ── Stack frame limit ───────────────────────────────────────────

    #[test]
    fn stack_frame_limit() {
        let mut event = build_event(
            "fatal",
            "panic",
            "test",
            None,
            "panic_hook",
            false,
            None,
            None,
            Vec::new(),
        );
        let frames: Vec<StackFrame> = (0..60)
            .map(|i| StackFrame {
                function: Some(format!("fn_{i}")),
                module: None,
                lineno: Some(i),
                filename: Some(format!("/path/to/file_{i}.rs")),
            })
            .collect();
        event.exception.as_mut().unwrap().values[0].stacktrace = Some(Stacktrace { frames });

        let sanitized = sanitize_event(event);
        let exc = sanitized.exception.unwrap();
        let st = exc.values[0].stacktrace.as_ref().unwrap();
        assert_eq!(st.frames.len(), MAX_STACK_FRAMES);
        for f in &st.frames {
            if let Some(ref name) = f.filename {
                assert!(!name.contains('/'), "paths must be basenames");
            }
        }
    }

    // ── Timestamp ───────────────────────────────────────────────────

    #[test]
    fn rfc3339_format() {
        let ts = now_rfc3339();
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
