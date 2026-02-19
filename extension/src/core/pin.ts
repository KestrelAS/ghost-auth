import { argon2id, argon2Verify } from "hash-wasm";
import { STORAGE_KEYS } from "./constants";

/**
 * PIN management for the browser extension.
 * Uses Argon2id for hashing/verification, with escalating rate limiting.
 * All state stored in browser.storage.local.
 */

// Recovery code character set: no 0/O, 1/I/L
const RECOVERY_CHARS = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

interface RateLimitState {
  failed_attempts: number;
  last_failure_epoch: number | null;
}

interface RecoveryEntry {
  hash: string;
  used: boolean;
}

function getBrowserStorage(): typeof chrome.storage {
  return (globalThis as any).browser?.storage ?? chrome.storage;
}

function lockoutSeconds(attempts: number): number {
  if (attempts < 5) return 0;
  if (attempts < 8) return 30;
  if (attempts < 10) return 300;
  return 900;
}

function nowEpoch(): number {
  return Math.floor(Date.now() / 1000);
}

function generateRecoveryCode(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  let code = "";
  for (let i = 0; i < 8; i++) {
    code += RECOVERY_CHARS[bytes[i] % RECOVERY_CHARS.length];
    if (i === 3) code += "-";
  }
  return code;
}

function normalizeRecoveryCode(code: string): string {
  return code.replace(/-/g, "").toUpperCase();
}

async function hashPin(pin: string): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  return await argon2id({
    password: pin,
    salt,
    parallelism: 1,
    iterations: 3,
    memorySize: 16384, // 16 MB — lighter than vault key derivation for quick PIN checks
    hashLength: 32,
    outputType: "encoded",
  });
}

async function verifyHash(pin: string, encoded: string): Promise<boolean> {
  return await argon2Verify({ password: pin, hash: encoded });
}

// ── Rate limit helpers ──

async function loadRateLimit(): Promise<RateLimitState> {
  const storage = getBrowserStorage();
  const result = await storage.local.get(STORAGE_KEYS.pinRateLimit);
  const data = result[STORAGE_KEYS.pinRateLimit];
  if (!data) return { failed_attempts: 0, last_failure_epoch: null };
  try {
    return JSON.parse(data);
  } catch {
    return { failed_attempts: 0, last_failure_epoch: null };
  }
}

async function saveRateLimit(state: RateLimitState): Promise<void> {
  const storage = getBrowserStorage();
  await storage.local.set({
    [STORAGE_KEYS.pinRateLimit]: JSON.stringify(state),
  });
}

function checkRateLimit(state: RateLimitState): { blocked: boolean; remainingSeconds: number } {
  const lockout = lockoutSeconds(state.failed_attempts);
  if (lockout === 0 || state.last_failure_epoch === null) {
    return { blocked: false, remainingSeconds: 0 };
  }

  const elapsed = nowEpoch() - state.last_failure_epoch;
  const remaining = lockout - elapsed;
  if (remaining <= 0) {
    return { blocked: false, remainingSeconds: 0 };
  }
  return { blocked: true, remainingSeconds: remaining };
}

// ── Recovery code helpers ──

async function loadRecoveryCodes(): Promise<RecoveryEntry[]> {
  const storage = getBrowserStorage();
  const result = await storage.local.get("ghost_pin_recovery");
  const data = result["ghost_pin_recovery"];
  if (!data) return [];
  try {
    return JSON.parse(data);
  } catch {
    return [];
  }
}

async function saveRecoveryCodes(codes: RecoveryEntry[]): Promise<void> {
  const storage = getBrowserStorage();
  await storage.local.set({
    ghost_pin_recovery: JSON.stringify(codes),
  });
}

// ── Public API ──

export async function hasPin(): Promise<boolean> {
  const storage = getBrowserStorage();
  const result = await storage.local.get(STORAGE_KEYS.pinHash);
  return !!result[STORAGE_KEYS.pinHash];
}

export async function setPin(pin: string, currentPin?: string): Promise<string[]> {
  if (pin.length < 4 || pin.length > 8 || !/^\d+$/.test(pin)) {
    throw new Error("PIN must be 4-8 digits");
  }

  // If a PIN already exists, verify current one first
  const existing = await hasPin();
  if (existing) {
    if (!currentPin) throw new Error("Current PIN required");
    const ok = await verifyPin(currentPin);
    if (!ok) throw new Error("Incorrect current PIN");
  }

  // Hash and store PIN
  const encoded = await hashPin(pin);
  const storage = getBrowserStorage();
  await storage.local.set({ [STORAGE_KEYS.pinHash]: encoded });

  // Generate 8 recovery codes
  const codes: string[] = [];
  const entries: RecoveryEntry[] = [];
  for (let i = 0; i < 8; i++) {
    const code = generateRecoveryCode();
    codes.push(code);
    const hash = await hashPin(normalizeRecoveryCode(code));
    entries.push({ hash, used: false });
  }
  await saveRecoveryCodes(entries);

  // Reset rate limiting
  await saveRateLimit({ failed_attempts: 0, last_failure_epoch: null });

  return codes;
}

export async function verifyPin(pin: string): Promise<boolean> {
  const rl = await loadRateLimit();
  const { blocked, remainingSeconds } = checkRateLimit(rl);
  if (blocked) {
    throw new Error(`Too many attempts. Try again in ${remainingSeconds}s`);
  }

  const storage = getBrowserStorage();
  const result = await storage.local.get(STORAGE_KEYS.pinHash);
  const encoded = result[STORAGE_KEYS.pinHash];
  if (!encoded) throw new Error("No PIN set");

  const ok = await verifyHash(pin, encoded);
  if (ok) {
    await saveRateLimit({ failed_attempts: 0, last_failure_epoch: null });
    return true;
  }

  // Record failure
  rl.failed_attempts++;
  rl.last_failure_epoch = nowEpoch();
  await saveRateLimit(rl);
  return false;
}

export async function removePin(pin: string): Promise<void> {
  const ok = await verifyPin(pin);
  if (!ok) throw new Error("Incorrect PIN");

  const storage = getBrowserStorage();
  await storage.local.remove([
    STORAGE_KEYS.pinHash,
    STORAGE_KEYS.pinRateLimit,
    "ghost_pin_recovery",
  ]);
}

export async function hasRecoveryCodes(): Promise<boolean> {
  const codes = await loadRecoveryCodes();
  return codes.some((c) => !c.used);
}

export async function verifyRecoveryCode(code: string): Promise<boolean> {
  const rl = await loadRateLimit();
  const { blocked, remainingSeconds } = checkRateLimit(rl);
  if (blocked) {
    throw new Error(`Too many attempts. Try again in ${remainingSeconds}s`);
  }

  const normalized = normalizeRecoveryCode(code);
  const entries = await loadRecoveryCodes();

  for (const entry of entries) {
    if (entry.used) continue;
    const ok = await verifyHash(normalized, entry.hash);
    if (ok) {
      // Mark used
      entry.used = true;
      await saveRecoveryCodes(entries);

      // Remove PIN entirely (recovery code usage removes PIN)
      const storage = getBrowserStorage();
      await storage.local.remove([
        STORAGE_KEYS.pinHash,
        STORAGE_KEYS.pinRateLimit,
        STORAGE_KEYS.pinWrappedDek,
      ]);

      return true;
    }
  }

  // No match — record failure
  rl.failed_attempts++;
  rl.last_failure_epoch = nowEpoch();
  await saveRateLimit(rl);
  return false;
}
