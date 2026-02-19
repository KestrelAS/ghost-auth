import { describe, it, expect } from "vitest";
import { exportBackup, importBackup } from "./backup";
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

describe("exportBackup / importBackup", () => {
  const password = "strong-password-123";

  it("roundtrips accounts correctly with all fields preserved", async () => {
    const accounts = [
      makeAccount({ issuer: "GitHub", label: "alice@github.com" }),
      makeAccount({
        issuer: "Google",
        label: "bob@gmail.com",
        algorithm: "SHA256",
        digits: 8,
        icon: "google",
        last_modified: 1700000001,
      }),
    ];

    const data = await exportBackup(accounts, password);
    const imported = await importBackup(data, password);

    expect(imported).toHaveLength(2);
    for (let i = 0; i < accounts.length; i++) {
      expect(imported[i].id).toBe(accounts[i].id);
      expect(imported[i].issuer).toBe(accounts[i].issuer);
      expect(imported[i].label).toBe(accounts[i].label);
      expect(imported[i].secret).toBe(accounts[i].secret);
      expect(imported[i].algorithm).toBe(accounts[i].algorithm);
      expect(imported[i].digits).toBe(accounts[i].digits);
      expect(imported[i].period).toBe(accounts[i].period);
      expect(imported[i].icon).toBe(accounts[i].icon);
      expect(imported[i].last_modified).toBe(accounts[i].last_modified);
    }
  });

  it("exported data starts with GHST magic bytes", async () => {
    const data = await exportBackup([], password);
    expect(data[0]).toBe(0x47); // G
    expect(data[1]).toBe(0x48); // H
    expect(data[2]).toBe(0x53); // S
    expect(data[3]).toBe(0x54); // T
  });

  it("exported data has version byte after magic", async () => {
    const data = await exportBackup([], password);
    expect(data[4]).toBe(1); // FORMAT_VERSION
  });

  it("rejects password shorter than 8 characters", async () => {
    await expect(exportBackup([], "short")).rejects.toThrow(
      "at least 8 characters",
    );
  });

  it("fails to import with wrong password", async () => {
    const data = await exportBackup([makeAccount()], password);
    await expect(importBackup(data, "wrong-password-123")).rejects.toThrow(
      "wrong password",
    );
  });

  it("rejects truncated data (< 49 bytes)", async () => {
    const tooSmall = new Uint8Array(48);
    await expect(importBackup(tooSmall, password)).rejects.toThrow(
      "too small",
    );
  });

  it("rejects invalid magic bytes", async () => {
    const data = await exportBackup([], password);
    data[0] = 0x00; // corrupt magic
    await expect(importBackup(data, password)).rejects.toThrow(
      "Not a Ghost Auth backup",
    );
  });

  it("rejects unsupported version", async () => {
    const data = await exportBackup([], password);
    data[4] = 99; // unsupported version
    await expect(importBackup(data, password)).rejects.toThrow(
      "Unsupported backup version",
    );
  });

  it("handles empty accounts array", async () => {
    const data = await exportBackup([], password);
    const imported = await importBackup(data, password);
    expect(imported).toEqual([]);
  });

  // Golden file test: this blob was created by the Rust backend's export_accounts().
  // If this test breaks, the binary format has drifted and existing .ghostauth files
  // will be unreadable, or Rust/Extension implementations are no longer byte-compatible.
  it("decrypts golden file from Rust backend", async () => {
    const hex =
      "4748535401290854456e705936cbfa217211dc9b30ae9e180348279b2e1ab6cea883018bf5b14a3b9a116a44b3ec13f0e0ebd68e170b5cd1453ce7d0aacf6cf5427e42a8f0b24f51c08a04fb9c1c1d532d1bac62d995280f1498737d1827d3100d22accda4848a04eb7cf50abc552e83607f255dd0309eef2030f6c4d6ee8e3fd9ae21553509e0085c1774acd3bb25e9e3a9a981f2d3d133f86be882770c8c2274ac04b486ab789c03505d11708c9c6356ece813efc5ffa832d00240ea17d0b17f2bbeb6798d9487d67f5c1a5cdceb25197ac6a35bba1d335512a8a67e5e832e47c3c5dbf45e9be70937837c7068f8c7ba0eac6807fb43d7e43f38407119c7661dcbbbfe8d7803c81997209a93bd068189fb379635301646dd65416e76dc95591d3e1c149bfec235c42abe4ae9915ba2accbab6a95204712744659ea3a20e43824033e6581659826e7f040cddb9f31c64a12770e26d044c468eaff066188017890a95d158a4c352f9c8a59873548a69deca7e64ed5a93e29d37fdcdea88faa6a99e27e2cfb21181762b71f637bc7c2fedc7da250a8bdb0e0890c5f9930f59b67ae43217c88c2e973ca86fe8fdeebf886597277823f10f00478d03b1fc08a2ae140";
    const data = new Uint8Array(
      hex.match(/.{2}/g)!.map((byte) => parseInt(byte, 16)),
    );

    const goldenPassword = "ghost-test-password-1234";
    const accounts = await importBackup(data, goldenPassword);

    expect(accounts).toHaveLength(2);

    // Account 1: GitHub (SHA1, 6 digits, no icon)
    expect(accounts[0].id).toBe("a1b2c3d4");
    expect(accounts[0].issuer).toBe("GitHub");
    expect(accounts[0].label).toBe("user@example.com");
    expect(accounts[0].secret).toBe("JBSWY3DPEHPK3PXP");
    expect(accounts[0].algorithm).toBe("SHA1");
    expect(accounts[0].digits).toBe(6);
    expect(accounts[0].period).toBe(30);
    expect(accounts[0].icon).toBeNull();
    expect(accounts[0].last_modified).toBe(1700000000);

    // Account 2: Google (SHA256, 8 digits, with icon)
    expect(accounts[1].id).toBe("e5f6g7h8");
    expect(accounts[1].issuer).toBe("Google");
    expect(accounts[1].label).toBe("alice@gmail.com");
    expect(accounts[1].secret).toBe("GEZDGNBVGY3TQOJQ");
    expect(accounts[1].algorithm).toBe("SHA256");
    expect(accounts[1].digits).toBe(8);
    expect(accounts[1].period).toBe(30);
    expect(accounts[1].icon).toBe("google");
    expect(accounts[1].last_modified).toBe(1700000001);
  });
});
