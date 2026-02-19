export const STORAGE_VERSION = 2;
export const TOMBSTONE_RETENTION_DAYS = 90;

export const STORAGE_KEYS = {
  vault: "ghost_vault",
  wrappedKey: "ghost_wrapped_key",
  deviceId: "ghost_device_id",
  syncHistory: "ghost_sync_history",
  pinHash: "ghost_pin_hash",
  pinRateLimit: "ghost_pin_ratelimit",
  pinWrappedDek: "ghost_pin_wrapped_dek",
  passwordless: "ghost_passwordless",
  passwordlessDek: "ghost_passwordless_dek",
} as const;

export const SESSION_KEYS = {
  dek: "ghost_dek",
} as const;
