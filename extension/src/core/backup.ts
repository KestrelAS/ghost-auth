import { argon2id } from "hash-wasm";
import { aesGcmEncrypt, aesGcmDecrypt } from "./crypto";
import type { Account } from "./types";

const MAGIC = new Uint8Array([0x47, 0x48, 0x53, 0x54]); // "GHST"
const FORMAT_VERSION = 1;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface BackupPayload {
  version: number;
  exported_at: number;
  accounts: Account[];
}

/**
 * Derive a 32-byte key from a password and salt using Argon2id.
 * Parameters match src-tauri/src/backup.rs exactly:
 *   m=65536 KiB (64 MB), t=3 iterations, p=1 parallelism, 32-byte output
 */
async function deriveKey(password: string, salt: Uint8Array): Promise<Uint8Array> {
  const hash = await argon2id({
    password,
    salt,
    parallelism: 1,
    iterations: 3,
    memorySize: 65536, // KiB = 64 MB
    hashLength: 32,
    outputType: "binary",
  });
  return new Uint8Array(hash);
}

/**
 * Create an encrypted backup of accounts in the Ghost Auth format.
 * Format: MAGIC(4) + VERSION(1) + SALT(16) + NONCE(12) + AES-256-GCM ciphertext
 *
 * Byte-compatible with src-tauri/src/backup.rs export_accounts().
 */
export async function exportBackup(
  accounts: Account[],
  password: string,
): Promise<Uint8Array> {
  if (password.length < 8) {
    throw new Error("Backup password must be at least 8 characters");
  }

  const salt = crypto.getRandomValues(new Uint8Array(16));
  const key = await deriveKey(password, salt);

  const payload: BackupPayload = {
    version: FORMAT_VERSION,
    exported_at: Math.floor(Date.now() / 1000),
    accounts,
  };
  const plaintext = encoder.encode(JSON.stringify(payload));
  const { nonce, ciphertext } = await aesGcmEncrypt(key, plaintext);

  // Zero key material
  key.fill(0);

  // Assemble: MAGIC + VERSION + SALT + NONCE + CIPHERTEXT
  const output = new Uint8Array(4 + 1 + 16 + 12 + ciphertext.length);
  output.set(MAGIC, 0);
  output[4] = FORMAT_VERSION;
  output.set(salt, 5);
  output.set(nonce, 21);
  output.set(ciphertext, 33);

  return output;
}

/**
 * Decrypt a .ghostauth backup file and return the accounts.
 * Format must match src-tauri/src/backup.rs import_accounts().
 */
export async function importBackup(
  data: Uint8Array,
  password: string,
): Promise<Account[]> {
  // Minimum: 4 (magic) + 1 (version) + 16 (salt) + 12 (nonce) + 16 (min AES-GCM tag)
  if (data.length < 49) {
    throw new Error("File is too small to be a valid backup");
  }

  if (
    data[0] !== 0x47 ||
    data[1] !== 0x48 ||
    data[2] !== 0x53 ||
    data[3] !== 0x54
  ) {
    throw new Error("Not a Ghost Auth backup file");
  }

  const version = data[4];
  if (version !== FORMAT_VERSION) {
    throw new Error(`Unsupported backup version: ${version}`);
  }

  const salt = data.slice(5, 21);
  const nonce = data.slice(21, 33);
  const ciphertext = data.slice(33);

  const key = await deriveKey(password, salt);
  try {
    const plaintext = await aesGcmDecrypt(key, nonce, ciphertext);
    const payload: BackupPayload = JSON.parse(decoder.decode(plaintext));
    return payload.accounts;
  } catch {
    throw new Error("Decryption failed \u2014 wrong password or corrupted file");
  } finally {
    key.fill(0);
  }
}
