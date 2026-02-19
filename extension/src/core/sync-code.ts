import { hmacSha256 } from "./crypto";

/**
 * Sync code key derivation — must match src-tauri/src/sync.rs SyncSession::key_from_code().
 *
 * Sync code format: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX (24 chars from CODE_CHARS)
 * Key = HMAC-SHA256(key="ghost-auth-sync-key-v1", data=clean_uppercase_code_bytes)
 */

const CODE_CHARS = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_GROUP_LEN = 4;
const CODE_GROUPS = 6;
const TOTAL_CHARS = CODE_GROUP_LEN * CODE_GROUPS; // 24

const encoder = new TextEncoder();

/**
 * Clean and validate a sync code string.
 * Returns the cleaned uppercase code (24 chars) or throws.
 */
export function cleanSyncCode(code: string): string {
  const clean = code
    .replace(/[\s-]/g, "")
    .toUpperCase();

  if (clean.length !== TOTAL_CHARS) {
    throw new Error(`Invalid sync code length: expected ${TOTAL_CHARS} characters, got ${clean.length}`);
  }

  for (const c of clean) {
    if (!CODE_CHARS.includes(c)) {
      throw new Error(`Invalid character in sync code: ${c}`);
    }
  }

  return clean;
}

/**
 * Derive a 32-byte key from a sync code.
 * Both sides (initiator and joiner) derive the same key from the same code.
 */
export async function keyFromSyncCode(code: string): Promise<Uint8Array> {
  const clean = cleanSyncCode(code);
  const keyMaterial = encoder.encode("ghost-auth-sync-key-v1");
  const data = encoder.encode(clean);
  return hmacSha256(keyMaterial, data);
}

/**
 * Format a raw code string with dashes: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
 */
export function formatSyncCode(raw: string): string {
  const clean = raw.replace(/[^a-zA-Z0-9]/g, "").slice(0, TOTAL_CHARS);
  const segments: string[] = [];
  for (let i = 0; i < clean.length; i += CODE_GROUP_LEN) {
    segments.push(clean.slice(i, i + CODE_GROUP_LEN));
  }
  return segments.join("-");
}

/**
 * Parse a ghost-auth://sync URI and extract code, hosts, and port(s).
 * Supports both single "host" and comma-separated "hosts" params.
 */
export function parseSyncUri(uri: string): {
  code: string;
  hosts: string[];
  port: number;
} {
  if (!uri.startsWith("ghost-auth://sync")) {
    throw new Error("Invalid sync URI");
  }

  const url = new URL(uri);
  const code = url.searchParams.get("code");
  const portStr = url.searchParams.get("port");

  // Backward compat: old URIs had separate "ws" param; new ones use single "port"
  const wsPortStr = url.searchParams.get("ws");

  // Parse hosts: prefer comma-separated "hosts" param, fall back to single "host"
  const hostsParam = url.searchParams.get("hosts");
  const hostParam = url.searchParams.get("host");
  const hosts = hostsParam
    ? hostsParam.split(",").filter(Boolean)
    : hostParam ? [hostParam] : [];

  if (!code || hosts.length === 0) {
    throw new Error("Missing code or host in sync URI");
  }

  // Use "port" (single-port mode), fall back to "ws" (old dual-port mode)
  const port = portStr ? parseInt(portStr, 10) : (wsPortStr ? parseInt(wsPortStr, 10) : 0);

  if (port < 1 || port > 65535) {
    throw new Error("Invalid or missing port in sync URI");
  }

  return { code, hosts, port };
}
