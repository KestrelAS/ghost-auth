import { aesGcmEncrypt, aesGcmDecrypt } from "./crypto";
import type {
  Account,
  Tombstone,
  EncryptedAccount,
  SyncPayload,
  MergeResult,
  MergeConflict,
} from "./types";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * Encrypt a single account with the sync session key.
 * Matches src-tauri/src/sync.rs encrypt_account().
 */
export async function encryptAccount(
  account: Account,
  key: Uint8Array,
): Promise<EncryptedAccount> {
  const plaintext = encoder.encode(JSON.stringify(account));
  const { nonce, ciphertext } = await aesGcmEncrypt(key, plaintext);
  return {
    id: account.id,
    last_modified: account.last_modified,
    nonce: Array.from(nonce),
    ciphertext: Array.from(ciphertext),
  };
}

/**
 * Decrypt a single account with the sync session key.
 * Matches src-tauri/src/sync.rs decrypt_account().
 */
export async function decryptAccount(
  enc: EncryptedAccount,
  key: Uint8Array,
): Promise<Account> {
  const nonce = new Uint8Array(enc.nonce);
  const ciphertext = new Uint8Array(enc.ciphertext);
  const plaintext = await aesGcmDecrypt(key, nonce, ciphertext);
  return JSON.parse(decoder.decode(plaintext));
}

/**
 * Build a sync payload from the current storage state.
 * Matches src-tauri/src/sync.rs build_payload().
 */
export async function buildPayload(
  deviceId: string,
  accounts: Account[],
  tombstones: Tombstone[],
  key: Uint8Array,
): Promise<SyncPayload> {
  const encrypted = await Promise.all(
    accounts.map((a) => encryptAccount(a, key)),
  );
  return {
    device_id: deviceId,
    timestamp: Math.floor(Date.now() / 1000),
    accounts: encrypted,
    tombstones: [...tombstones],
  };
}

/**
 * Perform the merge between local state and decrypted remote accounts.
 * Matches src-tauri/src/sync.rs merge() exactly.
 */
export function merge(
  localAccounts: Account[],
  localTombstones: Tombstone[],
  remoteAccounts: Account[],
  remoteTombstones: Tombstone[],
  lastSyncWithPeer: number | null,
): MergeResult {
  const localMap = new Map(localAccounts.map((a) => [a.id, a]));

  const localTombstoneMap = new Map(
    localTombstones.map((t) => [t.id, t.deleted_at]),
  );

  const remoteTombstoneMap = new Map(
    remoteTombstones.map((t) => [t.id, t.deleted_at]),
  );

  const toAdd: Account[] = [];
  const conflicts: MergeConflict[] = [];
  const autoUpdated: Account[] = [];
  let unchanged = 0;

  const lastSync = lastSyncWithPeer ?? 0;

  for (const remote of remoteAccounts) {
    // Skip if we locally deleted this account after the remote's last_modified
    const localDeletedAt = localTombstoneMap.get(remote.id);
    if (localDeletedAt !== undefined && localDeletedAt >= remote.last_modified) {
      unchanged++;
      continue;
    }

    const local = localMap.get(remote.id);
    if (local) {
      // Account exists on both sides
      if (local.last_modified === remote.last_modified) {
        unchanged++;
      } else if (
        local.last_modified > lastSync &&
        remote.last_modified > lastSync &&
        lastSync > 0
      ) {
        // Both modified since last sync — conflict
        conflicts.push({ local, remote });
      } else if (remote.last_modified > local.last_modified) {
        // Remote is newer — auto-update
        autoUpdated.push(remote);
      } else {
        // Local is newer — keep ours
        unchanged++;
      }
    } else {
      // Account doesn't exist locally — add it
      toAdd.push(remote);
    }
  }

  // Check remote tombstones against our local accounts
  const remoteDeletions: Account[] = [];
  for (const [id, deletedAt] of remoteTombstoneMap) {
    const local = localMap.get(id);
    if (local && deletedAt > local.last_modified) {
      remoteDeletions.push(local);
    }
  }

  return {
    to_add: toAdd,
    conflicts,
    remote_deletions: remoteDeletions,
    auto_updated: autoUpdated,
    unchanged,
  };
}
