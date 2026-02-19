import { describe, it, expect, vi } from "vitest";
import { generateCode, generateAllCodes } from "./totp";
import type { Account } from "./types";

function makeAccount(overrides: Partial<Account> = {}): Account {
  return {
    id: "test-id",
    issuer: "Test",
    label: "user@test.com",
    secret: "JBSWY3DPEHPK3PXP", // Base32-encoded "Hello!"
    algorithm: "SHA1",
    digits: 6,
    period: 30,
    icon: null,
    last_modified: 1000,
    ...overrides,
  };
}

describe("generateCode", () => {
  it("returns a 6-digit code for SHA1/6-digit account", () => {
    const result = generateCode(makeAccount({ digits: 6 }));
    expect(result.code).toMatch(/^\d{6}$/);
  });

  it("returns an 8-digit code for 8-digit account", () => {
    const result = generateCode(makeAccount({ digits: 8 }));
    expect(result.code).toMatch(/^\d{8}$/);
  });

  it("returns remaining seconds between 1 and period", () => {
    const period = 30;
    const result = generateCode(makeAccount({ period }));
    expect(result.remaining).toBeGreaterThanOrEqual(1);
    expect(result.remaining).toBeLessThanOrEqual(period);
  });

  it("returns the correct account id", () => {
    const result = generateCode(makeAccount({ id: "my-account" }));
    expect(result.id).toBe("my-account");
  });

  it("generates valid codes for SHA256", () => {
    const result = generateCode(
      makeAccount({ algorithm: "SHA256", secret: "JBSWY3DPEHPK3PXP" }),
    );
    expect(result.code).toMatch(/^\d{6}$/);
  });

  it("generates valid codes for SHA512", () => {
    const result = generateCode(
      makeAccount({ algorithm: "SHA512", secret: "JBSWY3DPEHPK3PXP" }),
    );
    expect(result.code).toMatch(/^\d{6}$/);
  });

  it("generates deterministic codes for the same timestamp", () => {
    const now = Date.now();
    vi.setSystemTime(now);
    const r1 = generateCode(makeAccount());
    const r2 = generateCode(makeAccount());
    expect(r1.code).toBe(r2.code);
    vi.useRealTimers();
  });
});

describe("generateAllCodes", () => {
  it("returns one CodeResponse per account", () => {
    const accounts = [
      makeAccount({ id: "a1" }),
      makeAccount({ id: "a2" }),
      makeAccount({ id: "a3" }),
    ];
    const results = generateAllCodes(accounts);
    expect(results).toHaveLength(3);
    expect(results.map((r) => r.id)).toEqual(["a1", "a2", "a3"]);
  });

  it("returns empty array for empty input", () => {
    expect(generateAllCodes([])).toEqual([]);
  });
});
