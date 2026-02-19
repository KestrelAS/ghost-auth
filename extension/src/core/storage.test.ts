import { describe, it, expect, beforeEach } from "vitest";
import { ExtensionStorage } from "./storage";
import type { Account } from "./types";

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

const PASSWORD = "test-password-123";

describe("ExtensionStorage", () => {
  let storage: ExtensionStorage;

  beforeEach(() => {
    storage = new ExtensionStorage();
  });

  describe("initialization", () => {
    it("is not initialized by default", async () => {
      expect(await storage.isInitialized()).toBe(false);
      expect(storage.isUnlocked()).toBe(false);
    });

    it("initializes with password and becomes unlocked", async () => {
      await storage.initialize(PASSWORD);
      expect(await storage.isInitialized()).toBe(true);
      expect(storage.isUnlocked()).toBe(true);
    });

    it("rejects short password", async () => {
      await expect(storage.initialize("short")).rejects.toThrow(
        "at least 8 characters",
      );
    });

    it("creates empty accounts after initialization", async () => {
      await storage.initialize(PASSWORD);
      const accounts = await storage.getAccounts();
      expect(accounts).toEqual([]);
    });
  });

  describe("unlock / lock", () => {
    beforeEach(async () => {
      await storage.initialize(PASSWORD);
      storage.lock();
    });

    it("unlocks with correct password", async () => {
      expect(storage.isUnlocked()).toBe(false);
      const ok = await storage.unlock(PASSWORD);
      expect(ok).toBe(true);
      expect(storage.isUnlocked()).toBe(true);
    });

    it("returns false for wrong password", async () => {
      const ok = await storage.unlock("wrong-password!!");
      expect(ok).toBe(false);
      expect(storage.isUnlocked()).toBe(false);
    });

    it("lock clears DEK and marks as locked", async () => {
      await storage.unlock(PASSWORD);
      expect(storage.isUnlocked()).toBe(true);
      storage.lock();
      expect(storage.isUnlocked()).toBe(false);
    });

    it("throws when vault not initialized", async () => {
      const fresh = new ExtensionStorage();
      // Clear the storage so there's no wrapped key
      await chrome.storage.local.clear();
      await expect(fresh.unlock(PASSWORD)).rejects.toThrow("not initialized");
    });
  });

  describe("account CRUD", () => {
    beforeEach(async () => {
      await storage.initialize(PASSWORD);
    });

    it("adds and retrieves accounts", async () => {
      const account = makeAccount({ issuer: "GitHub", label: "alice" });
      await storage.addAccount(account);
      const accounts = await storage.getAccounts();
      expect(accounts).toHaveLength(1);
      expect(accounts[0].issuer).toBe("GitHub");
      expect(accounts[0].label).toBe("alice");
    });

    it("adds multiple accounts", async () => {
      await storage.addAccount(makeAccount({ id: "a1" }));
      await storage.addAccount(makeAccount({ id: "a2" }));
      await storage.addAccount(makeAccount({ id: "a3" }));
      const accounts = await storage.getAccounts();
      expect(accounts).toHaveLength(3);
    });

    it("updates account issuer and label", async () => {
      const account = makeAccount({ id: "a1", issuer: "Old", label: "old" });
      await storage.addAccount(account);

      await storage.updateAccount("a1", {
        issuer: "New",
        label: "new",
      });

      const accounts = await storage.getAccounts();
      expect(accounts[0].issuer).toBe("New");
      expect(accounts[0].label).toBe("new");
      expect(accounts[0].last_modified).toBeGreaterThan(account.last_modified);
    });

    it("throws when updating non-existent account", async () => {
      await expect(
        storage.updateAccount("nonexistent", { issuer: "X" }),
      ).rejects.toThrow("Account not found");
    });

    it("deletes an account and creates a tombstone", async () => {
      const account = makeAccount({ id: "a1" });
      await storage.addAccount(account);

      await storage.deleteAccount("a1");

      const accounts = await storage.getAccounts();
      expect(accounts).toHaveLength(0);

      const tombstones = await storage.getTombstones();
      expect(tombstones).toHaveLength(1);
      expect(tombstones[0].id).toBe("a1");
      expect(tombstones[0].deleted_at).toBeGreaterThan(0);
    });

    it("delete is idempotent for unknown id", async () => {
      await storage.deleteAccount("nonexistent");
      const accounts = await storage.getAccounts();
      expect(accounts).toEqual([]);
    });
  });

  describe("reorderAccounts", () => {
    beforeEach(async () => {
      await storage.initialize(PASSWORD);
    });

    it("reorders accounts per provided IDs", async () => {
      await storage.addAccount(makeAccount({ id: "a" }));
      await storage.addAccount(makeAccount({ id: "b" }));
      await storage.addAccount(makeAccount({ id: "c" }));

      await storage.reorderAccounts(["c", "a", "b"]);

      const accounts = await storage.getAccounts();
      expect(accounts.map((a) => a.id)).toEqual(["c", "a", "b"]);
    });

    it("appends accounts not in the id list", async () => {
      await storage.addAccount(makeAccount({ id: "a" }));
      await storage.addAccount(makeAccount({ id: "b" }));
      await storage.addAccount(makeAccount({ id: "c" }));

      await storage.reorderAccounts(["b"]);

      const accounts = await storage.getAccounts();
      expect(accounts[0].id).toBe("b");
      // a and c are appended in original order
      expect(accounts.map((a) => a.id)).toEqual(["b", "a", "c"]);
    });
  });

  describe("passwordless mode", () => {
    beforeEach(async () => {
      await storage.initialize(PASSWORD);
    });

    it("is not passwordless by default", async () => {
      expect(await storage.isPasswordless()).toBe(false);
    });

    it("enables passwordless and stores DEK", async () => {
      await storage.setPasswordless(true);
      expect(await storage.isPasswordless()).toBe(true);
    });

    it("tryPasswordlessRestore restores DEK after lock", async () => {
      await storage.setPasswordless(true);
      storage.lock();
      expect(storage.isUnlocked()).toBe(false);

      const ok = await storage.tryPasswordlessRestore();
      expect(ok).toBe(true);
      expect(storage.isUnlocked()).toBe(true);

      // Verify we can still access data
      const accounts = await storage.getAccounts();
      expect(accounts).toEqual([]);
    });

    it("disabling passwordless removes stored DEK", async () => {
      await storage.setPasswordless(true);
      expect(await storage.isPasswordless()).toBe(true);

      await storage.setPasswordless(false);
      expect(await storage.isPasswordless()).toBe(false);

      storage.lock();
      const ok = await storage.tryPasswordlessRestore();
      expect(ok).toBe(false);
    });

    it("throws when enabling passwordless while locked", async () => {
      storage.lock();
      await expect(storage.setPasswordless(true)).rejects.toThrow(
        "Vault is locked",
      );
    });
  });

  describe("session restore", () => {
    it("restores from session storage after popup close", async () => {
      await storage.initialize(PASSWORD);
      // DEK was cached in session storage during initialize

      // New storage instance (simulates popup reopen)
      const storage2 = new ExtensionStorage();
      const ok = await storage2.tryRestoreSession();
      expect(ok).toBe(true);
      expect(storage2.isUnlocked()).toBe(true);

      // Should be able to read data
      const accounts = await storage2.getAccounts();
      expect(accounts).toEqual([]);
    });

    it("returns false when no session cache exists", async () => {
      const ok = await storage.tryRestoreSession();
      expect(ok).toBe(false);
    });
  });

  describe("PIN-wrapped DEK", () => {
    const pin = "1234";

    beforeEach(async () => {
      await storage.initialize(PASSWORD);
    });

    it("has no PIN-wrapped DEK initially", async () => {
      expect(await storage.hasPinWrappedDek()).toBe(false);
    });

    it("wraps and unwraps DEK with PIN", async () => {
      await storage.wrapDekWithPin(pin);
      expect(await storage.hasPinWrappedDek()).toBe(true);

      storage.lock();
      expect(storage.isUnlocked()).toBe(false);

      const ok = await storage.unwrapDekWithPin(pin);
      expect(ok).toBe(true);
      expect(storage.isUnlocked()).toBe(true);

      // Verify data is still accessible
      const accounts = await storage.getAccounts();
      expect(accounts).toEqual([]);
    });

    it("unwrapDekWithPin returns false for wrong PIN", async () => {
      await storage.wrapDekWithPin(pin);
      storage.lock();

      const ok = await storage.unwrapDekWithPin("0000");
      expect(ok).toBe(false);
      expect(storage.isUnlocked()).toBe(false);
    });

    it("clearPinWrappedDek removes the wrapped key", async () => {
      await storage.wrapDekWithPin(pin);
      expect(await storage.hasPinWrappedDek()).toBe(true);

      await storage.clearPinWrappedDek();
      expect(await storage.hasPinWrappedDek()).toBe(false);
    });

    it("throws when wrapping while locked", async () => {
      storage.lock();
      await expect(storage.wrapDekWithPin(pin)).rejects.toThrow(
        "Vault is locked",
      );
    });
  });

  describe("sync history", () => {
    beforeEach(async () => {
      await storage.initialize(PASSWORD);
    });

    it("returns empty history by default", async () => {
      const history = await storage.getSyncHistory();
      expect(history).toEqual({});
    });

    it("saves and retrieves sync history", async () => {
      const peers = { "device-a": 1000, "device-b": 2000 };
      await storage.saveSyncHistory(peers);
      const history = await storage.getSyncHistory();
      expect(history).toEqual(peers);
    });
  });

  describe("getDeviceId", () => {
    it("returns the device id created during initialization", async () => {
      await storage.initialize(PASSWORD);
      const id = await storage.getDeviceId();
      expect(id).toBeTruthy();
      expect(typeof id).toBe("string");
    });

    it("returns consistent id across calls", async () => {
      await storage.initialize(PASSWORD);
      const id1 = await storage.getDeviceId();
      const id2 = await storage.getDeviceId();
      expect(id1).toBe(id2);
    });
  });
});
