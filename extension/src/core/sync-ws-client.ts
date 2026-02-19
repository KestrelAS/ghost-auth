import { hmacSha256, constantTimeEqual, aesGcmEncrypt, aesGcmDecrypt } from "./crypto";
import type { SyncPayload } from "./types";

/**
 * WebSocket sync client — joiner role.
 *
 * Protocol (matches src-tauri/src/sync_ws.rs handshake_initiator):
 * 1. Receive 32-byte nonce from initiator
 * 2. Send HMAC(nonce, key) to initiator
 * 3. Receive + verify HMAC(our_hmac, key) from initiator (mutual auth)
 * 4. Derive session encryption key via HKDF-SHA256
 * 5. Joiner sends encrypted payload first, then receives initiator's encrypted payload
 *
 * Payload framing: 4-byte big-endian length + 12-byte AES-GCM nonce + ciphertext
 */

const NONCE_SIZE = 32;
const HMAC_SIZE = 32;
const MAX_PAYLOAD_SIZE = 10 * 1024 * 1024; // 10 MB

const encoder = new TextEncoder();
const SESSION_INFO = encoder.encode("ghost-auth-session-v1");

/**
 * Derive a session encryption key using HKDF-SHA256 (RFC 5869).
 * Must produce identical output to the Rust `derive_session_key` in sync.rs.
 */
async function deriveSessionKey(key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
  const ikm = await crypto.subtle.importKey("raw", key, "HKDF", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: nonce,
      info: SESSION_INFO,
    },
    ikm,
    256,
  );
  return new Uint8Array(bits);
}

/**
 * Connect to the sync initiator via WebSocket, perform HMAC handshake,
 * exchange payloads, and return the remote payload.
 */
export async function syncViaWebSocket(
  host: string,
  wsPort: number,
  key: Uint8Array,
  localPayload: SyncPayload,
): Promise<SyncPayload> {
  const ws = new WebSocket(`ws://${host}:${wsPort}`);
  ws.binaryType = "arraybuffer";

  try {
    // Wait for connection
    await waitForOpen(ws);

    // 1. Receive nonce from initiator
    const nonce = await recvBinary(ws, NONCE_SIZE, "nonce");

    // 2. Send HMAC(nonce, key)
    const ourHmac = await hmacSha256(key, nonce);
    ws.send(ourHmac.buffer);

    // 3. Receive mutual auth: HMAC(our_hmac, key)
    const ack = await recvBinary(ws, HMAC_SIZE, "ack");
    const expectedAck = await hmacSha256(key, ourHmac);
    if (!constantTimeEqual(ack, expectedAck)) {
      throw new Error("Authentication failed — the sync code may be incorrect");
    }

    // 4. Derive session encryption key
    const sessionKey = await deriveSessionKey(key, nonce);

    // 5. Joiner sends encrypted payload first
    await sendPayload(ws, sessionKey, localPayload);

    // 6. Receive initiator's encrypted payload
    const remotePayload = await recvPayload(ws, sessionKey);

    return remotePayload;
  } finally {
    try {
      ws.close();
    } catch {
      // Ignore close errors
    }
  }
}

/**
 * Try multiple hosts sequentially, returning the result from the first
 * that connects successfully.
 */
export async function syncViaWebSocketAnyHost(
  hosts: string[],
  wsPort: number,
  key: Uint8Array,
  localPayload: SyncPayload,
): Promise<SyncPayload> {
  if (hosts.length === 0) {
    throw new Error("No hosts to connect to");
  }

  // Single host — use normal timeout
  if (hosts.length === 1) {
    return syncViaWebSocket(hosts[0], wsPort, key, localPayload);
  }

  // Multiple hosts — try each with a shorter per-host timeout
  let lastError = "";
  for (const host of hosts) {
    try {
      return await syncViaWebSocketWithTimeout(host, wsPort, key, localPayload, 5000);
    } catch (e) {
      lastError = String(e);
    }
  }
  throw new Error(lastError || "Failed to connect to any address — ensure both devices are on the same network");
}

/**
 * Same as syncViaWebSocket but with a configurable connection timeout.
 */
async function syncViaWebSocketWithTimeout(
  host: string,
  wsPort: number,
  key: Uint8Array,
  localPayload: SyncPayload,
  connectTimeoutMs: number,
): Promise<SyncPayload> {
  const ws = new WebSocket(`ws://${host}:${wsPort}`);
  ws.binaryType = "arraybuffer";

  try {
    await waitForOpen(ws, connectTimeoutMs);
    const nonce = await recvBinary(ws, NONCE_SIZE, "nonce");
    const ourHmac = await hmacSha256(key, nonce);
    ws.send(ourHmac.buffer);
    const ack = await recvBinary(ws, HMAC_SIZE, "ack");
    const expectedAck = await hmacSha256(key, ourHmac);
    if (!constantTimeEqual(ack, expectedAck)) {
      throw new Error("Authentication failed — the sync code may be incorrect");
    }
    const sessionKey = await deriveSessionKey(key, nonce);
    await sendPayload(ws, sessionKey, localPayload);
    const remotePayload = await recvPayload(ws, sessionKey);
    return remotePayload;
  } finally {
    try { ws.close(); } catch { /* ignore */ }
  }
}

// ── Helpers ──

function waitForOpen(ws: WebSocket, timeoutMs = 15000): Promise<void> {
  return new Promise((resolve, reject) => {
    if (ws.readyState === WebSocket.OPEN) {
      resolve();
      return;
    }
    const timeout = setTimeout(() => {
      reject(new Error("WebSocket connection timed out"));
    }, timeoutMs);
    ws.onopen = () => {
      clearTimeout(timeout);
      resolve();
    };
    ws.onerror = () => {
      clearTimeout(timeout);
      reject(new Error("Failed to connect — ensure both devices are on the same network"));
    };
  });
}

function recvBinary(ws: WebSocket, expectedLen: number, step = ""): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const label = step ? ` (${step})` : "";
    const timeout = setTimeout(() => {
      reject(new Error(`WebSocket receive timed out${label}`));
    }, 30000);

    const handler = (e: MessageEvent) => {
      if (e.data instanceof ArrayBuffer) {
        clearTimeout(timeout);
        ws.removeEventListener("message", handler);
        ws.removeEventListener("close", closeHandler);
        const data = new Uint8Array(e.data);
        if (data.length !== expectedLen) {
          reject(new Error(`Expected ${expectedLen} bytes, got ${data.length}${label}`));
          return;
        }
        resolve(data);
      }
      // Ignore non-binary messages
    };

    const closeHandler = () => {
      clearTimeout(timeout);
      ws.removeEventListener("message", handler);
      reject(new Error(`WebSocket closed during receive${label}`));
    };

    ws.addEventListener("message", handler);
    ws.addEventListener("close", closeHandler);
  });
}

function recvBinaryAny(ws: WebSocket, step = ""): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const label = step ? ` (${step})` : "";
    const timeout = setTimeout(() => {
      reject(new Error(`WebSocket receive timed out${label}`));
    }, 30000);

    const handler = (e: MessageEvent) => {
      if (e.data instanceof ArrayBuffer) {
        clearTimeout(timeout);
        ws.removeEventListener("message", handler);
        ws.removeEventListener("close", closeHandler);
        resolve(new Uint8Array(e.data));
      }
    };

    const closeHandler = () => {
      clearTimeout(timeout);
      ws.removeEventListener("message", handler);
      reject(new Error(`WebSocket closed during receive${label}`));
    };

    ws.addEventListener("message", handler);
    ws.addEventListener("close", closeHandler);
  });
}

async function sendPayload(ws: WebSocket, sessionKey: Uint8Array, payload: SyncPayload): Promise<void> {
  const json = encoder.encode(JSON.stringify(payload));
  if (json.length > MAX_PAYLOAD_SIZE) {
    throw new Error("Sync payload too large");
  }

  const { nonce, ciphertext } = await aesGcmEncrypt(sessionKey, json);

  // Frame: 4-byte BE length + 12-byte nonce + ciphertext
  const bodyLen = 12 + ciphertext.length;
  const frame = new Uint8Array(4 + bodyLen);
  const view = new DataView(frame.buffer);
  view.setUint32(0, bodyLen, false); // big-endian
  frame.set(nonce, 4);
  frame.set(ciphertext, 16);

  ws.send(frame.buffer);
}

async function recvPayload(ws: WebSocket, sessionKey: Uint8Array): Promise<SyncPayload> {
  const frame = await recvBinaryAny(ws, "payload");

  if (frame.length < 4 + 28) {
    throw new Error("Invalid sync frame: too short");
  }

  const view = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
  const len = view.getUint32(0, false); // big-endian

  if (len < 28) {
    throw new Error("Sync payload too short to be valid");
  }
  if (len > MAX_PAYLOAD_SIZE + 28) {
    throw new Error(`Sync payload too large (${len} bytes)`);
  }

  if (frame.length < 4 + len) {
    throw new Error("Incomplete sync frame");
  }

  const gcmNonce = frame.slice(4, 16);
  const ciphertext = frame.slice(16, 4 + len);

  const jsonBytes = await aesGcmDecrypt(sessionKey, gcmNonce, ciphertext);
  const decoder = new TextDecoder();
  return JSON.parse(decoder.decode(jsonBytes));
}
