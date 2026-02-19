import { describe, it, expect } from "vitest";
import {
  encryptAccount,
  decryptAccount,
  buildPayload,
  merge,
} from "./sync-protocol";
import type { Account, Tombstone } from "./types";

function randomKey(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(32));
}

function makeAccount(overrides: Partial<Account> = {}): Account {
  return {
    id: crypto.randomUUID(),
    issuer: "Test",
    label: "user@test.com",
    secret: "JBSWY3DPEHPK3PXP",
    algorithm: "SHA1",
    digits: 6,
    period: 30,
    icon: null,
    last_modified: 1000,
    ...overrides,
  };
}

describe("encryptAccount / decryptAccount", () => {
  it("roundtrips all account fields correctly", async () => {
    const key = randomKey();
    const account = makeAccount({
      issuer: "GitHub",
      label: "alice@github.com",
      algorithm: "SHA256",
      digits: 8,
      period: 60,
      icon: "github",
      last_modified: 1700000000,
    });

    const encrypted = await encryptAccount(account, key);
    expect(encrypted.id).toBe(account.id);
    expect(encrypted.last_modified).toBe(account.last_modified);
    expect(encrypted.nonce).toHaveLength(12);
    expect(encrypted.ciphertext.length).toBeGreaterThan(0);

    const decrypted = await decryptAccount(encrypted, key);
    expect(decrypted).toEqual(account);
  });

  it("fails to decrypt with wrong key", async () => {
    const key1 = randomKey();
    const key2 = randomKey();
    const encrypted = await encryptAccount(makeAccount(), key1);
    await expect(decryptAccount(encrypted, key2)).rejects.toThrow();
  });
});

describe("buildPayload", () => {
  it("encrypts all accounts and includes metadata", async () => {
    const key = randomKey();
    const accounts = [makeAccount(), makeAccount()];
    const tombstones: Tombstone[] = [{ id: "dead", deleted_at: 999 }];

    const payload = await buildPayload("device-1", accounts, tombstones, key);

    expect(payload.device_id).toBe("device-1");
    expect(payload.timestamp).toBeGreaterThan(0);
    expect(payload.accounts).toHaveLength(2);
    expect(payload.tombstones).toEqual(tombstones);

    // Verify encrypted accounts can be decrypted back
    for (let i = 0; i < accounts.length; i++) {
      const decrypted = await decryptAccount(payload.accounts[i], key);
      expect(decrypted).toEqual(accounts[i]);
    }
  });
});

describe("merge", () => {
  it("adds new remote accounts to local", () => {
    const remote = makeAccount({ id: "r1", last_modified: 100 });
    const result = merge([], [], [remote], [], null);

    expect(result.to_add).toEqual([remote]);
    expect(result.conflicts).toHaveLength(0);
    expect(result.remote_deletions).toHaveLength(0);
    expect(result.auto_updated).toHaveLength(0);
    expect(result.unchanged).toBe(0);
  });

  it("marks unchanged when same last_modified", () => {
    const account = makeAccount({ id: "a1", last_modified: 100 });
    const result = merge([account], [], [{ ...account }], [], null);

    expect(result.to_add).toHaveLength(0);
    expect(result.unchanged).toBe(1);
  });

  it("auto-updates when remote is newer", () => {
    const local = makeAccount({ id: "a1", last_modified: 100 });
    const remote = makeAccount({
      id: "a1",
      last_modified: 200,
      label: "updated",
    });
    const result = merge([local], [], [remote], [], null);

    expect(result.auto_updated).toEqual([remote]);
    expect(result.unchanged).toBe(0);
  });

  it("keeps local when local is newer", () => {
    const local = makeAccount({ id: "a1", last_modified: 200 });
    const remote = makeAccount({ id: "a1", last_modified: 100 });
    const result = merge([local], [], [remote], [], null);

    expect(result.auto_updated).toHaveLength(0);
    expect(result.unchanged).toBe(1);
  });

  it("detects conflict when both modified since last sync", () => {
    const local = makeAccount({ id: "a1", last_modified: 150 });
    const remote = makeAccount({ id: "a1", last_modified: 160 });
    const lastSync = 100;
    const result = merge([local], [], [remote], [], lastSync);

    expect(result.conflicts).toHaveLength(1);
    expect(result.conflicts[0].local).toEqual(local);
    expect(result.conflicts[0].remote).toEqual(remote);
  });

  it("does not conflict on first sync (lastSyncWithPeer = 0)", () => {
    const local = makeAccount({ id: "a1", last_modified: 100 });
    const remote = makeAccount({
      id: "a1",
      last_modified: 200,
      label: "remote-wins",
    });
    const result = merge([local], [], [remote], [], 0);

    // lastSync = 0 means the conflict condition (lastSync > 0) is false,
    // so it falls through to timestamp comparison: remote wins
    expect(result.conflicts).toHaveLength(0);
    expect(result.auto_updated).toEqual([remote]);
  });

  it("does not conflict on first sync (lastSyncWithPeer = null)", () => {
    const local = makeAccount({ id: "a1", last_modified: 100 });
    const remote = makeAccount({
      id: "a1",
      last_modified: 200,
      label: "remote-wins",
    });
    const result = merge([local], [], [remote], [], null);

    expect(result.conflicts).toHaveLength(0);
    expect(result.auto_updated).toEqual([remote]);
  });

  it("handles remote tombstone deleting local account", () => {
    const local = makeAccount({ id: "a1", last_modified: 100 });
    const tombstone: Tombstone = { id: "a1", deleted_at: 200 };
    const result = merge([local], [], [], [tombstone], null);

    expect(result.remote_deletions).toEqual([local]);
  });

  it("ignores remote tombstone when local was modified after deletion", () => {
    const local = makeAccount({ id: "a1", last_modified: 300 });
    const tombstone: Tombstone = { id: "a1", deleted_at: 200 };
    const result = merge([local], [], [], [tombstone], null);

    expect(result.remote_deletions).toHaveLength(0);
  });

  it("skips remote account if locally tombstoned after remote modification", () => {
    const remote = makeAccount({ id: "a1", last_modified: 100 });
    const localTombstone: Tombstone = { id: "a1", deleted_at: 200 };
    const result = merge([], [localTombstone], [remote], [], null);

    expect(result.to_add).toHaveLength(0);
    expect(result.unchanged).toBe(1);
  });

  it("adds remote account if locally tombstoned before remote modification", () => {
    const remote = makeAccount({ id: "a1", last_modified: 300 });
    const localTombstone: Tombstone = { id: "a1", deleted_at: 200 };
    const result = merge([], [localTombstone], [remote], [], null);

    expect(result.to_add).toEqual([remote]);
  });

  it("handles empty remote (no changes)", () => {
    const local = makeAccount({ id: "a1" });
    const result = merge([local], [], [], [], null);

    expect(result.to_add).toHaveLength(0);
    expect(result.conflicts).toHaveLength(0);
    expect(result.remote_deletions).toHaveLength(0);
    expect(result.auto_updated).toHaveLength(0);
    expect(result.unchanged).toBe(0);
  });

  it("handles empty local (all remote added)", () => {
    const r1 = makeAccount({ id: "r1" });
    const r2 = makeAccount({ id: "r2" });
    const result = merge([], [], [r1, r2], [], null);

    expect(result.to_add).toEqual([r1, r2]);
  });

  it("handles mixed scenario: add, update, conflict, deletion", () => {
    const localA = makeAccount({ id: "a", last_modified: 100 });
    const localB = makeAccount({ id: "b", last_modified: 150 });
    const localC = makeAccount({ id: "c", last_modified: 100 });

    const remoteA = makeAccount({
      id: "a",
      last_modified: 200,
      label: "updated-a",
    });
    const remoteB = makeAccount({
      id: "b",
      last_modified: 160,
      label: "updated-b",
    });
    const remoteD = makeAccount({ id: "d", last_modified: 100 });
    const remoteTombstoneC: Tombstone = { id: "c", deleted_at: 200 };

    const lastSync = 120;
    const result = merge(
      [localA, localB, localC],
      [],
      [remoteA, remoteB, remoteD],
      [remoteTombstoneC],
      lastSync,
    );

    // a: local modified at 100 (before lastSync), remote at 200 → auto-update
    expect(result.auto_updated).toEqual([remoteA]);
    // b: both modified after lastSync (150 > 120 and 160 > 120) → conflict
    expect(result.conflicts).toHaveLength(1);
    expect(result.conflicts[0].local.id).toBe("b");
    // c: remote tombstone at 200 > local modified at 100 → deletion
    expect(result.remote_deletions).toEqual([localC]);
    // d: new remote account → add
    expect(result.to_add).toEqual([remoteD]);
  });
});
