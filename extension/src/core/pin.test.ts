import { describe, it, expect } from "vitest";
import {
  hasPin,
  setPin,
  verifyPin,
  removePin,
  hasRecoveryCodes,
  verifyRecoveryCode,
} from "./pin";

describe("setPin", () => {
  it("stores a PIN and returns 8 recovery codes", async () => {
    const codes = await setPin("1234");
    expect(codes).toHaveLength(8);
    for (const code of codes) {
      expect(code).toMatch(/^[A-Z2-9]{4}-[A-Z2-9]{4}$/);
    }
  });

  it("makes hasPin return true", async () => {
    expect(await hasPin()).toBe(false);
    await setPin("1234");
    expect(await hasPin()).toBe(true);
  });

  it("rejects non-digit PIN", async () => {
    await expect(setPin("abcd")).rejects.toThrow("4-8 digits");
  });

  it("rejects PIN shorter than 4 digits", async () => {
    await expect(setPin("123")).rejects.toThrow("4-8 digits");
  });

  it("rejects PIN longer than 8 digits", async () => {
    await expect(setPin("123456789")).rejects.toThrow("4-8 digits");
  });

  it("accepts 4-digit PIN", async () => {
    const codes = await setPin("1234");
    expect(codes).toHaveLength(8);
  });

  it("accepts 8-digit PIN", async () => {
    const codes = await setPin("12345678");
    expect(codes).toHaveLength(8);
  });

  it("requires current PIN when changing", async () => {
    await setPin("1234");
    await expect(setPin("5678")).rejects.toThrow("Current PIN required");
  });

  it("rejects wrong current PIN when changing", async () => {
    await setPin("1234");
    await expect(setPin("5678", "0000")).rejects.toThrow("Incorrect current PIN");
  });

  it("allows changing PIN with correct current PIN", async () => {
    await setPin("1234");
    const codes = await setPin("5678", "1234");
    expect(codes).toHaveLength(8);
  });
});

describe("verifyPin", () => {
  it("returns true for correct PIN", async () => {
    await setPin("1234");
    expect(await verifyPin("1234")).toBe(true);
  });

  it("returns false for wrong PIN", async () => {
    await setPin("1234");
    expect(await verifyPin("0000")).toBe(false);
  });

  it("throws when no PIN is set", async () => {
    await expect(verifyPin("1234")).rejects.toThrow("No PIN set");
  });

  it("throws after 5 consecutive failures (rate limiting)", async () => {
    await setPin("1234");

    // 4 failures should be fine
    for (let i = 0; i < 4; i++) {
      expect(await verifyPin("0000")).toBe(false);
    }

    // 5th failure triggers rate limit
    expect(await verifyPin("0000")).toBe(false);

    // 6th attempt should be blocked
    await expect(verifyPin("1234")).rejects.toThrow("Too many attempts");
  });

  it("resets failure counter on successful verification", async () => {
    await setPin("1234");

    // Fail 3 times
    for (let i = 0; i < 3; i++) {
      await verifyPin("0000");
    }

    // Succeed — resets counter
    expect(await verifyPin("1234")).toBe(true);

    // Fail 4 more times — should still be under limit
    for (let i = 0; i < 4; i++) {
      expect(await verifyPin("0000")).toBe(false);
    }
  });
});

describe("removePin", () => {
  it("removes PIN with correct current PIN", async () => {
    await setPin("1234");
    expect(await hasPin()).toBe(true);
    await removePin("1234");
    expect(await hasPin()).toBe(false);
  });

  it("rejects wrong PIN", async () => {
    await setPin("1234");
    await expect(removePin("0000")).rejects.toThrow("Incorrect PIN");
  });
});

describe("recovery codes", () => {
  it("has recovery codes after setPin", async () => {
    await setPin("1234");
    expect(await hasRecoveryCodes()).toBe(true);
  });

  it("has no recovery codes initially", async () => {
    expect(await hasRecoveryCodes()).toBe(false);
  });

  it("valid recovery code removes PIN and returns true", async () => {
    const codes = await setPin("1234");
    const result = await verifyRecoveryCode(codes[0]);
    expect(result).toBe(true);
    expect(await hasPin()).toBe(false);
  });

  it("invalid recovery code returns false", async () => {
    await setPin("1234");
    const result = await verifyRecoveryCode("ZZZZ-ZZZZ");
    expect(result).toBe(false);
    // PIN should still be set
    expect(await hasPin()).toBe(true);
  });

  it("used recovery code returns false", async () => {
    const codes = await setPin("1234");
    // Use the first code
    expect(await verifyRecoveryCode(codes[0])).toBe(true);

    // Re-set PIN so we can test code reuse
    await setPin("5678");

    // The first code from the original set is no longer valid
    // (setPin generates new codes)
    expect(await verifyRecoveryCode(codes[0])).toBe(false);
  });

  it("recovery code works case-insensitively", async () => {
    const codes = await setPin("1234");
    const lowered = codes[0].toLowerCase();
    const result = await verifyRecoveryCode(lowered);
    expect(result).toBe(true);
  });

  it("recovery code works without dash", async () => {
    const codes = await setPin("1234");
    const noDash = codes[0].replace("-", "");
    const result = await verifyRecoveryCode(noDash);
    expect(result).toBe(true);
  });
});
