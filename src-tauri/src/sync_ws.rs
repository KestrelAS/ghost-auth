use std::net::TcpStream;

use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use tungstenite::{accept, Message, WebSocket};

use crate::sync::SyncPayload;

type HmacSha256 = Hmac<Sha256>;

/// Maximum payload size (10 MB).
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;
/// Handshake nonce size.
const NONCE_SIZE: usize = 32;
/// HMAC output size (SHA-256).
const HMAC_SIZE: usize = 32;

// ── Public entry point ───────────────────────────────────────────

/// Upgrade a raw TCP stream to a WebSocket, then perform HMAC handshake as initiator.
/// Called by sync_transport when auto-detection identifies a WebSocket client.
pub fn upgrade_and_handshake(stream: TcpStream, key: &[u8; 32]) -> Result<WsSyncConnection, String> {
    let ws = accept(stream).map_err(|e| {
        tracing::error!(error = %e, "WebSocket upgrade failed");
        "WebSocket handshake failed".to_string()
    })?;

    handshake_initiator(ws, key)
}

// ── Handshake ─────────────────────────────────────────────────────

/// Initiator (shows QR) WebSocket handshake:
/// 1. Send random nonce as binary message
/// 2. Receive HMAC(nonce, key) from joiner as binary message
/// 3. Verify HMAC
/// 4. Send HMAC(joiner_hmac, key) as mutual auth
fn handshake_initiator(
    mut ws: WebSocket<TcpStream>,
    key: &[u8; 32],
) -> Result<WsSyncConnection, String> {
    // 1. Generate and send nonce
    let mut nonce = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    ws.send(Message::Binary(nonce.to_vec().into()))
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to send WS nonce");
            "WebSocket sync error".to_string()
        })?;

    // 2. Receive joiner's HMAC
    let joiner_hmac = recv_binary(&mut ws, HMAC_SIZE)?;

    // 3. Verify
    let expected = compute_hmac(key, &nonce);
    if !constant_time_eq(&joiner_hmac, &expected) {
        tracing::warn!(
            event = "ws_sync_auth_failed",
            "WS handshake failed: invalid HMAC from joiner"
        );
        let _ = ws.close(None);
        return Err("Authentication failed — the sync code may be incorrect".to_string());
    }

    // 4. Send mutual auth: HMAC(joiner_hmac, key)
    let ack = compute_hmac(key, &joiner_hmac);
    ws.send(Message::Binary(ack.into())).map_err(|e| {
        tracing::error!(error = %e, "Failed to send WS mutual auth");
        "WebSocket sync error".to_string()
    })?;

    // Derive session encryption key from handshake nonce
    let session_key = crate::sync::derive_session_key(key, &nonce);

    tracing::info!(
        event = "ws_sync_handshake_ok",
        "WebSocket handshake completed (initiator)"
    );
    Ok(WsSyncConnection { ws, session_key })
}

// ── Connection ────────────────────────────────────────────────────

/// Authenticated WebSocket connection after successful handshake.
pub struct WsSyncConnection {
    ws: WebSocket<TcpStream>,
    session_key: [u8; 32],
}

impl WsSyncConnection {
    /// Send a sync payload over the WebSocket connection.
    /// Format: 4-byte length (big-endian) + 12-byte AES-GCM nonce + ciphertext,
    /// sent as a single binary message.
    pub fn send_payload(&mut self, payload: &SyncPayload) -> Result<(), String> {
        let json = serde_json::to_vec(payload).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize WS sync payload");
            "Failed to send sync data".to_string()
        })?;

        if json.len() > MAX_PAYLOAD_SIZE {
            return Err("Sync payload too large".to_string());
        }

        let (gcm_nonce, ciphertext) = crate::sync::session_encrypt(&self.session_key, &json)?;

        // Frame: 4-byte BE length + 12-byte nonce + ciphertext
        let body_len = 12 + ciphertext.len();
        let len_bytes = (body_len as u32).to_be_bytes();
        let mut frame = Vec::with_capacity(4 + body_len);
        frame.extend_from_slice(&len_bytes);
        frame.extend_from_slice(&gcm_nonce);
        frame.extend_from_slice(&ciphertext);

        self.ws
            .send(Message::Binary(frame.into()))
            .map_err(|e| {
                tracing::error!(error = %e, "WS sync write failed");
                "Sync connection error".to_string()
            })?;

        tracing::info!(
            event = "ws_sync_payload_sent",
            size = json.len(),
            "WS sync payload sent (encrypted)"
        );
        Ok(())
    }

    /// Receive a sync payload from the WebSocket connection.
    pub fn recv_payload(&mut self) -> Result<SyncPayload, String> {
        let frame = recv_binary_any(&mut self.ws)?;

        if frame.len() < 4 + 28 {
            return Err("Invalid sync frame: too short".to_string());
        }

        let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        if len < 28 {
            return Err("Sync payload too short to be valid".to_string());
        }
        if len > MAX_PAYLOAD_SIZE + 28 {
            return Err(format!(
                "Sync payload too large ({} bytes, max {})",
                len, MAX_PAYLOAD_SIZE
            ));
        }

        if frame.len() < 4 + len {
            return Err("Incomplete sync frame".to_string());
        }

        let gcm_nonce = &frame[4..16];
        let ciphertext = &frame[16..4 + len];

        let json = crate::sync::session_decrypt(&self.session_key, gcm_nonce, ciphertext)?;

        let payload: SyncPayload = serde_json::from_slice(&json).map_err(|e| {
            tracing::error!(error = %e, "Failed to deserialize WS sync payload");
            "Failed to read sync data".to_string()
        })?;

        tracing::info!(
            event = "ws_sync_payload_received",
            size = json.len(),
            accounts = payload.accounts.len(),
            "WS sync payload received (decrypted)"
        );
        Ok(payload)
    }

    /// Close the WebSocket connection.
    /// We flush the TCP stream and drop without sending a WebSocket Close frame.
    /// Sending a Close frame immediately after send_payload can race with the
    /// browser processing the Binary payload, causing "WebSocket closed during receive".
    pub fn close(mut self) {
        let _ = std::io::Write::flush(self.ws.get_mut());
        // drop(self) closes the TCP stream, sending a FIN
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

/// Receive a binary WebSocket message of exactly `expected_len` bytes.
fn recv_binary(ws: &mut WebSocket<TcpStream>, expected_len: usize) -> Result<Vec<u8>, String> {
    loop {
        let msg = ws.read().map_err(|e| {
            tracing::error!(error = %e, "WS sync read failed");
            "WebSocket sync read error".to_string()
        })?;

        match msg {
            Message::Binary(data) => {
                if data.len() != expected_len {
                    return Err(format!(
                        "Expected {} bytes, got {}",
                        expected_len,
                        data.len()
                    ));
                }
                return Ok(data.to_vec());
            }
            Message::Ping(data) => {
                let _ = ws.send(Message::Pong(data));
            }
            Message::Close(_) => {
                return Err("WebSocket connection closed during handshake".to_string());
            }
            _ => {
                // Skip text messages, pongs, etc.
                continue;
            }
        }
    }
}

/// Receive any binary WebSocket message (variable length).
fn recv_binary_any(ws: &mut WebSocket<TcpStream>) -> Result<Vec<u8>, String> {
    loop {
        let msg = ws.read().map_err(|e| {
            tracing::error!(error = %e, "WS sync read failed");
            "WebSocket sync read error".to_string()
        })?;

        match msg {
            Message::Binary(data) => {
                return Ok(data.to_vec());
            }
            Message::Ping(data) => {
                let _ = ws.send(Message::Pong(data));
            }
            Message::Close(_) => {
                return Err("WebSocket connection closed".to_string());
            }
            _ => continue,
        }
    }
}
