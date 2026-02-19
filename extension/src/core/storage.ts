import { argon2id } from "hash-wasm";
import { aesGcmEncrypt, aesGcmDecrypt } from "./crypto";
import { STORAGE_KEYS, SESSION_KEYS, STORAGE_VERSION, TOMBSTONE_RETENTION_DAYS } from "./constants";

async function derivePinKek(pin: string, salt: Uint8Array): Promise<Uint8Array> {
  const hash = await argon2id({
    password: pin,
    salt,
    parallelism: 1,
    iterations: 3,
    memorySize: 16384, // 16 MB — same as PIN hash in pin.ts
    hashLength: 32,
    outputType: "binary",
  });
  return new Uint8Array(hash);
}
import type { Account, Tombstone, StoragePayload } from "./types";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function getBrowserStorage(): typeof chrome.storage {
  return (globalThis as any).browser?.storage ?? chrome.storage;
}

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64ToUint8(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function deriveKek(password: string, salt: Uint8Array): Promise<Uint8Array> {
  const hash = await argon2id({
    password,
    salt,
    parallelism: 1,
    iterations: 3,
    memorySize: 65536,
    hashLength: 32,
    outputType: "binary",
  });
  return new Uint8Array(hash);
}

export class ExtensionStorage {
  private dek: Uint8Array | null = null;

  async isInitialized(): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.wrappedKey);
    return !!result[STORAGE_KEYS.wrappedKey];
  }

  isUnlocked(): boolean {
    return this.dek !== null;
  }

  /** First-time setup: create DEK, wrap with password, store. */
  async initialize(password: string): Promise<void> {
    if (password.length < 8) {
      throw new Error("Master password must be at least 8 characters");
    }

    // Generate random DEK
    const dek = crypto.getRandomValues(new Uint8Array(32));
    const salt = crypto.getRandomValues(new Uint8Array(16));
    const kek = await deriveKek(password, salt);

    // Wrap DEK with KEK
    const { nonce, ciphertext } = await aesGcmEncrypt(kek, dek);
    kek.fill(0);

    // Format: salt(16) + nonce(12) + ciphertext
    const wrapped = new Uint8Array(16 + 12 + ciphertext.length);
    wrapped.set(salt, 0);
    wrapped.set(nonce, 16);
    wrapped.set(ciphertext, 28);

    // Generate device ID
    const deviceId = crypto.randomUUID();

    // Create empty vault
    const payload: StoragePayload = {
      version: STORAGE_VERSION,
      device_id: deviceId,
      accounts: [],
      tombstones: [],
    };
    const plaintext = encoder.encode(JSON.stringify(payload));
    const vault = await aesGcmEncrypt(dek, plaintext);
    const vaultData = new Uint8Array(12 + vault.ciphertext.length);
    vaultData.set(vault.nonce, 0);
    vaultData.set(vault.ciphertext, 12);

    const storage = getBrowserStorage();
    await storage.local.set({
      [STORAGE_KEYS.wrappedKey]: uint8ToBase64(wrapped),
      [STORAGE_KEYS.deviceId]: deviceId,
      [STORAGE_KEYS.vault]: uint8ToBase64(vaultData),
    });

    // Cache DEK in session storage (in-memory only)
    await this.cacheDek(dek);
    this.dek = dek;
  }

  /** Unlock vault by deriving KEK from password and unwrapping DEK. */
  async unlock(password: string): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.wrappedKey);
    const wrappedB64 = result[STORAGE_KEYS.wrappedKey];
    if (!wrappedB64) throw new Error("Vault not initialized");

    const wrapped = base64ToUint8(wrappedB64);
    const salt = wrapped.slice(0, 16);
    const nonce = wrapped.slice(16, 28);
    const ciphertext = wrapped.slice(28);

    const kek = await deriveKek(password, salt);
    try {
      const dek = await aesGcmDecrypt(kek, nonce, ciphertext);
      kek.fill(0);
      this.dek = dek;
      await this.cacheDek(dek);
      return true;
    } catch {
      kek.fill(0);
      return false;
    }
  }

  /** Try to restore DEK from session cache. */
  async tryRestoreSession(): Promise<boolean> {
    try {
      const storage = getBrowserStorage();
      if (!storage.session) return false;
      const result = await storage.session.get(SESSION_KEYS.dek);
      const dekB64 = result[SESSION_KEYS.dek];
      if (!dekB64) return false;
      this.dek = base64ToUint8(dekB64);
      return true;
    } catch {
      return false;
    }
  }

  lock(): void {
    if (this.dek) {
      this.dek.fill(0);
      this.dek = null;
    }
    const storage = getBrowserStorage();
    storage.session?.remove(SESSION_KEYS.dek).catch(() => {});
  }

  async getAccounts(): Promise<Account[]> {
    const payload = await this.loadPayload();
    return payload.accounts;
  }

  async getTombstones(): Promise<Tombstone[]> {
    const payload = await this.loadPayload();
    return payload.tombstones;
  }

  async getDeviceId(): Promise<string> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.deviceId);
    return result[STORAGE_KEYS.deviceId] || crypto.randomUUID();
  }

  async saveAccounts(accounts: Account[], tombstones: Tombstone[]): Promise<void> {
    const deviceId = await this.getDeviceId();
    const now = Math.floor(Date.now() / 1000);
    const cutoff = now - TOMBSTONE_RETENTION_DAYS * 86400;
    const prunedTombstones = tombstones.filter((t) => t.deleted_at > cutoff);

    const payload: StoragePayload = {
      version: STORAGE_VERSION,
      device_id: deviceId,
      accounts,
      tombstones: prunedTombstones,
    };

    await this.savePayload(payload);
  }

  async addAccount(account: Account): Promise<void> {
    const payload = await this.loadPayload();
    payload.accounts.push(account);
    await this.savePayload(payload);
  }

  async updateAccount(id: string, updates: Partial<Pick<Account, "issuer" | "label">>): Promise<void> {
    const payload = await this.loadPayload();
    const account = payload.accounts.find((a) => a.id === id);
    if (!account) throw new Error("Account not found");
    if (updates.issuer !== undefined) account.issuer = updates.issuer;
    if (updates.label !== undefined) account.label = updates.label;
    account.last_modified = Math.floor(Date.now() / 1000);
    await this.savePayload(payload);
  }

  async deleteAccount(id: string): Promise<void> {
    const payload = await this.loadPayload();
    const index = payload.accounts.findIndex((a) => a.id === id);
    if (index === -1) return;
    payload.accounts.splice(index, 1);
    payload.tombstones.push({ id, deleted_at: Math.floor(Date.now() / 1000) });
    await this.savePayload(payload);
  }

  async reorderAccounts(ids: string[]): Promise<void> {
    const payload = await this.loadPayload();
    const accountMap = new Map(payload.accounts.map((a) => [a.id, a]));
    const reordered: Account[] = [];
    for (const id of ids) {
      const account = accountMap.get(id);
      if (account) reordered.push(account);
    }
    // Append any accounts not in the ids list
    for (const account of payload.accounts) {
      if (!ids.includes(account.id)) reordered.push(account);
    }
    payload.accounts = reordered;
    await this.savePayload(payload);
  }

  async getSyncHistory(): Promise<Record<string, number>> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.syncHistory);
    const data = result[STORAGE_KEYS.syncHistory];
    if (!data) return {};
    try {
      return JSON.parse(data).peers || {};
    } catch {
      return {};
    }
  }

  async saveSyncHistory(peers: Record<string, number>): Promise<void> {
    const storage = getBrowserStorage();
    await storage.local.set({
      [STORAGE_KEYS.syncHistory]: JSON.stringify({ peers }),
    });
  }

  // ── PIN-wrapped DEK ──

  /** Wrap current in-memory DEK with a key derived from the PIN. */
  async wrapDekWithPin(pin: string): Promise<void> {
    if (!this.dek) throw new Error("Vault is locked");
    const salt = crypto.getRandomValues(new Uint8Array(16));
    const kek = await derivePinKek(pin, salt);
    const { nonce, ciphertext } = await aesGcmEncrypt(kek, this.dek);
    kek.fill(0);

    // Format: salt(16) + nonce(12) + ciphertext
    const wrapped = new Uint8Array(16 + 12 + ciphertext.length);
    wrapped.set(salt, 0);
    wrapped.set(nonce, 16);
    wrapped.set(ciphertext, 28);

    const storage = getBrowserStorage();
    await storage.local.set({
      [STORAGE_KEYS.pinWrappedDek]: uint8ToBase64(wrapped),
    });
  }

  /** Unwrap DEK using PIN. Returns true on success (acts as implicit PIN verification). */
  async unwrapDekWithPin(pin: string): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.pinWrappedDek);
    const wrappedB64 = result[STORAGE_KEYS.pinWrappedDek];
    if (!wrappedB64) return false;

    const wrapped = base64ToUint8(wrappedB64);
    const salt = wrapped.slice(0, 16);
    const nonce = wrapped.slice(16, 28);
    const ciphertext = wrapped.slice(28);

    const kek = await derivePinKek(pin, salt);
    try {
      const dek = await aesGcmDecrypt(kek, nonce, ciphertext);
      kek.fill(0);
      this.dek = dek;
      await this.cacheDek(dek);
      return true;
    } catch {
      kek.fill(0);
      return false;
    }
  }

  async hasPinWrappedDek(): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.pinWrappedDek);
    return !!result[STORAGE_KEYS.pinWrappedDek];
  }

  async clearPinWrappedDek(): Promise<void> {
    const storage = getBrowserStorage();
    await storage.local.remove(STORAGE_KEYS.pinWrappedDek);
  }

  // ── Passwordless ──

  /** Enable or disable passwordless mode. */
  async setPasswordless(enabled: boolean): Promise<void> {
    const storage = getBrowserStorage();
    if (enabled) {
      if (!this.dek) throw new Error("Vault is locked");
      await storage.local.set({
        [STORAGE_KEYS.passwordless]: "true",
        [STORAGE_KEYS.passwordlessDek]: uint8ToBase64(this.dek),
      });
    } else {
      await storage.local.remove([STORAGE_KEYS.passwordless, STORAGE_KEYS.passwordlessDek]);
    }
  }

  async isPasswordless(): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.passwordless);
    return result[STORAGE_KEYS.passwordless] === "true";
  }

  /** Restore DEK from passwordless storage. */
  async tryPasswordlessRestore(): Promise<boolean> {
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.passwordlessDek);
    const dekB64 = result[STORAGE_KEYS.passwordlessDek];
    if (!dekB64) return false;
    this.dek = base64ToUint8(dekB64);
    await this.cacheDek(this.dek);
    return true;
  }

  // ── Private helpers ──

  private async loadPayload(): Promise<StoragePayload> {
    if (!this.dek) throw new Error("Vault is locked");
    const storage = getBrowserStorage();
    const result = await storage.local.get(STORAGE_KEYS.vault);
    const vaultB64 = result[STORAGE_KEYS.vault];
    if (!vaultB64) {
      return {
        version: STORAGE_VERSION,
        device_id: await this.getDeviceId(),
        accounts: [],
        tombstones: [],
      };
    }

    const vaultData = base64ToUint8(vaultB64);
    const nonce = vaultData.slice(0, 12);
    const ciphertext = vaultData.slice(12);
    const plaintext = await aesGcmDecrypt(this.dek, nonce, ciphertext);
    return JSON.parse(decoder.decode(plaintext));
  }

  private async savePayload(payload: StoragePayload): Promise<void> {
    if (!this.dek) throw new Error("Vault is locked");
    const plaintext = encoder.encode(JSON.stringify(payload));
    const { nonce, ciphertext } = await aesGcmEncrypt(this.dek, plaintext);
    const vaultData = new Uint8Array(12 + ciphertext.length);
    vaultData.set(nonce, 0);
    vaultData.set(ciphertext, 12);

    const storage = getBrowserStorage();
    await storage.local.set({
      [STORAGE_KEYS.vault]: uint8ToBase64(vaultData),
    });
  }

  private async cacheDek(dek: Uint8Array): Promise<void> {
    try {
      const storage = getBrowserStorage();
      if (storage.session) {
        await storage.session.set({ [SESSION_KEYS.dek]: uint8ToBase64(dek) });
      }
    } catch {
      // Session storage may not be available
    }
  }
}
