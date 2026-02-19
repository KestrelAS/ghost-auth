use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;

use crate::sync::SyncPayload;

type HmacSha256 = Hmac<Sha256>;

/// Timeout for waiting for a peer connection.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(120);
/// Timeout for read/write operations.
const IO_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout for protocol auto-detection (peek for HTTP upgrade).
const DETECT_TIMEOUT: Duration = Duration::from_millis(500);
/// Maximum payload size (10 MB — generous for TOTP data).
const MAX_PAYLOAD_SIZE: u32 = 10 * 1024 * 1024;
/// Handshake nonce size.
const NONCE_SIZE: usize = 32;
/// HMAC output size (SHA-256).
const HMAC_SIZE: usize = 32;

// ── Unified connection type ──────────────────────────────────────

/// Wraps either a TCP or WebSocket sync connection for uniform handling.
pub enum SyncConnKind {
    Tcp(SyncConnection),
    Ws(crate::sync_ws::WsSyncConnection),
}

impl SyncConnKind {
    pub fn recv_payload(&mut self) -> Result<SyncPayload, String> {
        match self {
            SyncConnKind::Tcp(c) => c.recv_payload(),
            SyncConnKind::Ws(c) => c.recv_payload(),
        }
    }

    pub fn send_payload(&mut self, payload: &SyncPayload) -> Result<(), String> {
        match self {
            SyncConnKind::Tcp(c) => c.send_payload(payload),
            SyncConnKind::Ws(c) => c.send_payload(payload),
        }
    }

    pub fn close(self) {
        match self {
            SyncConnKind::Tcp(c) => c.close(),
            SyncConnKind::Ws(c) => c.close(),
        }
    }
}

// ── Listener (Initiator) ─────────────────────────────────────────

/// A sync listener that waits for a peer to connect.
/// Supports both raw TCP (native apps) and WebSocket (browser extensions)
/// on the same port via protocol auto-detection.
pub struct SyncListener {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl SyncListener {
    /// Bind to a random port on a private network interface only.
    /// Refuses to bind if no RFC 1918 address is available.
    pub fn bind() -> Result<Self, String> {
        let private_ips = local_ips();
        let bind_ip = private_ips.first().ok_or_else(|| {
            tracing::warn!(event = "sync_no_private_ip", "No private IP found for sync listener");
            "Cannot start sync — no local network found. Connect to a Wi-Fi or LAN network and try again.".to_string()
        })?;

        let bind_addr = format!("{}:0", bind_ip);
        let listener = TcpListener::bind(&bind_addr).map_err(|e| {
            tracing::error!(error = %e, addr = %bind_addr, "Failed to bind sync listener");
            "Failed to start sync — could not bind to a port".to_string()
        })?;

        let local_addr = listener.local_addr().map_err(|e| {
            tracing::error!(error = %e, "Failed to get local address");
            "Failed to start sync".to_string()
        })?;

        listener.set_nonblocking(false).ok();

        tracing::info!(
            event = "sync_listener_started",
            addr = %local_addr,
            "Sync listener started (private IP only)"
        );

        Ok(Self {
            listener,
            local_addr,
        })
    }

    /// The port we're listening on.
    pub fn port(&self) -> u16 {
        self.local_addr.port()
    }

    /// The IP address we're listening on.
    pub fn ip(&self) -> String {
        self.local_addr.ip().to_string()
    }

    /// Wait for a peer connection, auto-detect protocol (TCP or WebSocket),
    /// perform authentication handshake, and return the connection.
    ///
    /// Loops on failed handshakes so that a bad connection (e.g. port scanner)
    /// doesn't kill the listener for the real client.
    pub fn accept_any(&self, key: &[u8; 32]) -> Result<SyncConnKind, String> {
        self.listener.set_nonblocking(true).ok();
        let deadline = std::time::Instant::now() + ACCEPT_TIMEOUT;

        loop {
            // Accept a TCP connection (poll with timeout)
            let (stream, peer_addr) = loop {
                match self.listener.accept() {
                    Ok(result) => break result,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return Err(
                                "Sync timed out — no device connected".to_string()
                            );
                        }
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to accept connection");
                        return Err("Failed to accept sync connection".to_string());
                    }
                }
            };

            tracing::info!(
                event = "sync_peer_connected",
                peer = %peer_addr,
                "Peer connected for sync"
            );

            // Switch to blocking for protocol detection
            stream.set_nonblocking(false).ok();
            stream.set_read_timeout(Some(DETECT_TIMEOUT)).ok();
            stream.set_write_timeout(Some(IO_TIMEOUT)).ok();

            // Auto-detect: peek at incoming data.
            // WebSocket clients send an HTTP upgrade request immediately ("GET ...").
            // Raw TCP joiners wait for the initiator's nonce (no data from client).
            let mut peek_buf = [0u8; 4];
            let is_ws = match stream.peek(&mut peek_buf) {
                Ok(n) if n >= 3 && &peek_buf[..3] == b"GET" => true,
                _ => false, // Timeout, WouldBlock, or non-HTTP data → raw TCP
            };

            // Set proper I/O timeout for handshake
            stream.set_read_timeout(Some(IO_TIMEOUT)).ok();

            if is_ws {
                tracing::info!(event = "sync_protocol_detected", protocol = "websocket");
                match crate::sync_ws::upgrade_and_handshake(stream, key) {
                    Ok(conn) => return Ok(SyncConnKind::Ws(conn)),
                    Err(e) => {
                        tracing::warn!(error = %e, "WS handshake failed, continuing to accept");
                        if std::time::Instant::now() >= deadline {
                            return Err(e);
                        }
                        continue;
                    }
                }
            } else {
                tracing::info!(event = "sync_protocol_detected", protocol = "tcp");
                match handshake_initiator(stream, key) {
                    Ok(conn) => return Ok(SyncConnKind::Tcp(conn)),
                    Err(e) => {
                        tracing::warn!(error = %e, "TCP handshake failed, continuing to accept");
                        if std::time::Instant::now() >= deadline {
                            return Err(e);
                        }
                        continue;
                    }
                }
            }
        }
    }
}

// ── Connector (Joiner) ───────────────────────────────────────────

/// Connect to a sync peer and authenticate.
pub fn connect(host: &str, port: u16, key: &[u8; 32]) -> Result<SyncConnection, String> {
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect_timeout(
        &addr.parse().map_err(|_| "Invalid sync address".to_string())?,
        IO_TIMEOUT,
    )
    .map_err(|e| {
        tracing::error!(error = %e, addr = %addr, "Failed to connect to sync peer");
        "Failed to connect — ensure both devices are on the same network".to_string()
    })?;

    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();

    tracing::info!(
        event = "sync_connected",
        peer = %addr,
        "Connected to sync peer"
    );

    // Perform handshake as joiner (prover)
    let conn = handshake_joiner(stream, key)?;
    Ok(conn)
}

// ── Handshake ─────────────────────────────────────────────────────

/// Authenticated connection after successful handshake.
pub struct SyncConnection {
    stream: TcpStream,
    session_key: [u8; 32],
}

/// Initiator (shows QR) handshake:
/// 1. Send random nonce
/// 2. Receive HMAC(nonce, key) from joiner
/// 3. Verify HMAC
/// 4. Send HMAC(joiner_hmac, key) as mutual auth
fn handshake_initiator(mut stream: TcpStream, key: &[u8; 32]) -> Result<SyncConnection, String> {
    // 1. Generate and send nonce
    let mut nonce = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    write_exact(&mut stream, &nonce)?;

    // 2. Receive joiner's HMAC
    let joiner_hmac = read_exact_bytes(&mut stream, HMAC_SIZE)?;

    // 3. Verify
    let expected = compute_hmac(key, &nonce);
    if !constant_time_eq(&joiner_hmac, &expected) {
        tracing::warn!(event = "sync_auth_failed", "Handshake failed: invalid HMAC from joiner");
        let _ = stream.shutdown(Shutdown::Both);
        return Err("Authentication failed — the sync code may be incorrect".to_string());
    }

    // 4. Send mutual auth: HMAC(joiner_hmac, key)
    let ack = compute_hmac(key, &joiner_hmac);
    write_exact(&mut stream, &ack)?;

    // Derive session encryption key from handshake nonce
    let session_key = crate::sync::derive_session_key(key, &nonce);

    tracing::info!(event = "sync_handshake_ok", "Handshake completed (initiator)");
    Ok(SyncConnection { stream, session_key })
}

/// Joiner (scans QR) handshake:
/// 1. Receive nonce from initiator
/// 2. Send HMAC(nonce, key)
/// 3. Receive and verify mutual auth HMAC
fn handshake_joiner(mut stream: TcpStream, key: &[u8; 32]) -> Result<SyncConnection, String> {
    // 1. Receive nonce
    let nonce = read_exact_bytes(&mut stream, NONCE_SIZE)?;

    // 2. Send HMAC(nonce, key)
    let our_hmac = compute_hmac(key, &nonce);
    write_exact(&mut stream, &our_hmac)?;

    // 3. Receive mutual auth
    let ack = read_exact_bytes(&mut stream, HMAC_SIZE)?;

    // 4. Verify mutual auth: HMAC(our_hmac, key)
    let expected_ack = compute_hmac(key, &our_hmac);
    if !constant_time_eq(&ack, &expected_ack) {
        tracing::warn!(event = "sync_mutual_auth_failed", "Mutual auth failed");
        let _ = stream.shutdown(Shutdown::Both);
        return Err("Authentication failed — the sync code may be incorrect".to_string());
    }

    // Derive session encryption key from handshake nonce
    let nonce_arr: [u8; 32] = nonce
        .try_into()
        .map_err(|_| "Internal error: nonce wrong size".to_string())?;
    let session_key = crate::sync::derive_session_key(key, &nonce_arr);

    tracing::info!(event = "sync_handshake_ok", "Handshake completed (joiner)");
    Ok(SyncConnection { stream, session_key })
}

impl SyncConnection {
    /// Send a sync payload over the connection.
    /// Format: [4-byte length (big-endian)] [12-byte AES-GCM nonce] [ciphertext]
    pub fn send_payload(&mut self, payload: &SyncPayload) -> Result<(), String> {
        let json = serde_json::to_vec(payload).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize sync payload");
            "Failed to send sync data".to_string()
        })?;

        if json.len() > MAX_PAYLOAD_SIZE as usize {
            return Err("Sync payload too large".to_string());
        }

        let (gcm_nonce, ciphertext) = crate::sync::session_encrypt(&self.session_key, &json)?;

        let body_len = (12 + ciphertext.len()) as u32;
        let len_bytes = body_len.to_be_bytes();
        write_exact(&mut self.stream, &len_bytes)?;
        write_exact(&mut self.stream, &gcm_nonce)?;
        write_exact(&mut self.stream, &ciphertext)?;

        tracing::info!(
            event = "sync_payload_sent",
            size = json.len(),
            "Sync payload sent (encrypted)"
        );
        Ok(())
    }

    /// Receive a sync payload from the connection.
    pub fn recv_payload(&mut self) -> Result<SyncPayload, String> {
        let len_bytes = read_exact_bytes(&mut self.stream, 4)?;
        let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;

        // Body must contain at least 12-byte nonce + 16-byte GCM auth tag
        if len < 28 {
            return Err("Sync payload too short to be valid".to_string());
        }
        if len > MAX_PAYLOAD_SIZE as usize + 28 {
            return Err(format!(
                "Sync payload too large ({} bytes, max {})",
                len, MAX_PAYLOAD_SIZE
            ));
        }

        let gcm_nonce = read_exact_bytes(&mut self.stream, 12)?;
        let ciphertext = read_exact_bytes(&mut self.stream, len - 12)?;

        let json = crate::sync::session_decrypt(&self.session_key, &gcm_nonce, &ciphertext)?;

        let payload: SyncPayload = serde_json::from_slice(&json).map_err(|e| {
            tracing::error!(error = %e, "Failed to deserialize sync payload");
            "Failed to read sync data".to_string()
        })?;

        tracing::info!(
            event = "sync_payload_received",
            size = json.len(),
            accounts = payload.accounts.len(),
            "Sync payload received (decrypted)"
        );
        Ok(payload)
    }

    /// Close the connection.
    pub fn close(self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

// ── Helpers ───────────────────────────────────────────────────────

fn compute_hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can accept any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn write_exact(stream: &mut TcpStream, data: &[u8]) -> Result<(), String> {
    stream.write_all(data).map_err(|e| {
        tracing::error!(error = %e, "Sync write failed");
        "Sync connection error".to_string()
    })
}

fn read_exact_bytes(stream: &mut TcpStream, len: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).map_err(|e| {
        tracing::error!(error = %e, len = len, "Sync read failed");
        "Sync connection error".to_string()
    })?;
    Ok(buf)
}

/// Enumerate all private IPv4 addresses on this machine.
/// Filters to RFC 1918 ranges (10.x, 172.16-31.x, 192.168.x).
pub fn local_ips() -> Vec<String> {
    let Ok(interfaces) = local_ip_address::list_afinet_netifas() else {
        // Fallback to single IP if enumeration fails
        return local_ip_address::local_ip()
            .ok()
            .map(|ip| vec![ip.to_string()])
            .unwrap_or_default();
    };

    interfaces
        .into_iter()
        .filter_map(|(_name, addr)| {
            if let std::net::IpAddr::V4(v4) = addr {
                let octets = v4.octets();
                let is_private = octets[0] == 10
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168);
                if is_private {
                    return Some(v4.to_string());
                }
            }
            None
        })
        .collect()
}


// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::EncryptedAccount;

    fn make_test_payload() -> SyncPayload {
        SyncPayload {
            device_id: "test-device".to_string(),
            timestamp: 1000,
            accounts: vec![EncryptedAccount {
                id: "a1".to_string(),
                last_modified: 1000,
                nonce: vec![0u8; 12],
                ciphertext: vec![1, 2, 3, 4],
            }],
            tombstones: vec![],
        }
    }

    #[test]
    fn test_hmac_computation() {
        let key = [0xAA; 32];
        let data = b"test data";
        let mac1 = compute_hmac(&key, data);
        let mac2 = compute_hmac(&key, data);
        assert_eq!(mac1, mac2);
        assert_eq!(mac1.len(), 32);

        // Different data produces different HMAC
        let mac3 = compute_hmac(&key, b"other data");
        assert_ne!(mac1, mac3);

        // Different key produces different HMAC
        let mac4 = compute_hmac(&[0xBB; 32], data);
        assert_ne!(mac1, mac4);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }

    #[test]
    fn test_listener_binds() {
        if local_ips().is_empty() {
            // CI environments without private IPs — skip
            return;
        }
        let listener = SyncListener::bind().unwrap();
        assert!(listener.port() > 0);
        // Verify we bound to a private IP, not 0.0.0.0
        let ip = listener.ip();
        assert!(
            ip.starts_with("10.")
                || ip.starts_with("172.")
                || ip.starts_with("192.168."),
            "Expected private IP, got {}",
            ip
        );
    }

    #[test]
    fn test_full_sync_flow_loopback() {
        if local_ips().is_empty() {
            return;
        }
        let key = [0xCC; 32];

        // Start listener
        let listener = SyncListener::bind().unwrap();
        let port = listener.port();

        // Spawn a joiner thread — connect to the listener's private IP
        let listener_ip = listener.ip();
        let joiner = std::thread::spawn(move || {
            let mut conn = connect(&listener_ip, port, &key).unwrap();
            // Joiner sends first
            conn.send_payload(&make_test_payload()).unwrap();
            // Then receives
            let received = conn.recv_payload().unwrap();
            conn.close();
            received
        });

        // Initiator accepts (auto-detect will detect raw TCP since joiner waits for nonce)
        let mut conn = match listener.accept_any(&key).unwrap() {
            SyncConnKind::Tcp(c) => c,
            SyncConnKind::Ws(_) => panic!("Expected TCP connection"),
        };
        // Initiator receives first (matches joiner send order)
        let received = conn.recv_payload().unwrap();
        // Then sends
        let initiator_payload = SyncPayload {
            device_id: "initiator".to_string(),
            timestamp: 2000,
            accounts: vec![],
            tombstones: vec![],
        };
        conn.send_payload(&initiator_payload).unwrap();
        conn.close();

        // Verify joiner received initiator's payload
        let joiner_received = joiner.join().unwrap();
        assert_eq!(joiner_received.device_id, "initiator");
        assert_eq!(joiner_received.timestamp, 2000);

        // Verify initiator received joiner's payload
        assert_eq!(received.device_id, "test-device");
        assert_eq!(received.accounts.len(), 1);
    }

    #[test]
    fn test_wrong_key_rejected() {
        if local_ips().is_empty() {
            return;
        }
        let key_a = [0xAA; 32];
        let key_b = [0xBB; 32];

        let listener = SyncListener::bind().unwrap();
        let port = listener.port();
        let listener_ip = listener.ip();

        // accept_any loops on failed handshakes, so run it in a background thread
        // and let it get dropped when the test ends.
        let accept_handle = std::thread::spawn(move || {
            listener.accept_any(&key_a)
        });

        // Joiner with wrong key should be rejected
        let joiner_result = connect(&listener_ip, port, &key_b);
        assert!(joiner_result.is_err());

        // Drop the accept thread (it's still looping but that's fine)
        drop(accept_handle);
    }

    #[test]
    fn test_local_ip_discovery() {
        // This may or may not return IPs depending on the test environment
        let ips = local_ips();
        for addr in &ips {
            assert!(!addr.is_empty());
            // Should be private IPs, not localhost
            assert!(!addr.starts_with("127."));
        }
    }
}
