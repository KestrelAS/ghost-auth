import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getAccounts,
  addAccount,
  addAccountManual,
  deleteAccount,
  generateAllCodes,
  hasPin,
  setPin,
  verifyPin,
  removePin,
} from "./accounts";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe("accounts store", () => {
  it("getAccounts invokes get_accounts", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    const result = await getAccounts();
    expect(mockInvoke).toHaveBeenCalledWith("get_accounts");
    expect(result).toEqual([]);
  });

  it("addAccount passes uri to backend", async () => {
    const mockAccount = {
      id: "1",
      issuer: "Test",
      label: "u",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
      icon: null,
    };
    mockInvoke.mockResolvedValueOnce(mockAccount);
    const result = await addAccount("otpauth://totp/Test:u?secret=AAA");
    expect(mockInvoke).toHaveBeenCalledWith("add_account", {
      uri: "otpauth://totp/Test:u?secret=AAA",
    });
    expect(result.issuer).toBe("Test");
  });

  it("addAccountManual passes all fields", async () => {
    const mockAccount = {
      id: "2",
      issuer: "GitHub",
      label: "me",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
      icon: null,
    };
    mockInvoke.mockResolvedValueOnce(mockAccount);
    await addAccountManual("GitHub", "me", "SECRET", "SHA1", 6, 30);
    expect(mockInvoke).toHaveBeenCalledWith("add_account_manual", {
      issuer: "GitHub",
      label: "me",
      secret: "SECRET",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
    });
  });

  it("deleteAccount passes id", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await deleteAccount("abc");
    expect(mockInvoke).toHaveBeenCalledWith("delete_account", { id: "abc" });
  });

  it("generateAllCodes returns code responses", async () => {
    const codes = [{ id: "1", code: "123456", remaining: 15 }];
    mockInvoke.mockResolvedValueOnce(codes);
    const result = await generateAllCodes();
    expect(result).toEqual(codes);
  });

  it("hasPin invokes has_pin", async () => {
    mockInvoke.mockResolvedValueOnce(true);
    expect(await hasPin()).toBe(true);
  });

  it("setPin passes pin string", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await setPin("1234");
    expect(mockInvoke).toHaveBeenCalledWith("set_pin", { pin: "1234", current_pin: null });
  });

  it("verifyPin passes pin string", async () => {
    mockInvoke.mockResolvedValueOnce(true);
    expect(await verifyPin("1234")).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith("verify_pin", { pin: "1234" });
  });

  it("removePin passes pin string", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await removePin("1234");
    expect(mockInvoke).toHaveBeenCalledWith("remove_pin", { pin: "1234" });
  });
});
