import { describe, it, expect } from "vitest";
import {
  cleanSyncCode,
  keyFromSyncCode,
  formatSyncCode,
  parseSyncUri,
} from "./sync-code";

describe("cleanSyncCode", () => {
  it("strips dashes and whitespace, uppercases", () => {
    expect(cleanSyncCode("abcd-efgh-jkmn-pqrs-tuvw-xy23")).toBe(
      "ABCDEFGHJKMNPQRSTUVWXY23",
    );
  });

  it("accepts already-clean uppercase code", () => {
    const code = "ABCDEFGHJKMNPQRSTUVWXY23";
    expect(cleanSyncCode(code)).toBe(code);
  });

  it("strips spaces and tabs", () => {
    expect(cleanSyncCode("ABCD EFGH JKMN PQRS TUVW XY23")).toBe(
      "ABCDEFGHJKMNPQRSTUVWXY23",
    );
  });

  it("rejects invalid length (too short)", () => {
    expect(() => cleanSyncCode("ABCDEFGH")).toThrow("Invalid sync code length");
  });

  it("rejects invalid length (too long)", () => {
    expect(() => cleanSyncCode("ABCDEFGHJKMNPQRSTUVWXY234")).toThrow(
      "Invalid sync code length",
    );
  });

  it("rejects invalid character 0 (zero)", () => {
    expect(() => cleanSyncCode("0BCDEFGHJKMNPQRSTUVWXY23")).toThrow(
      "Invalid character",
    );
  });

  it("rejects invalid character O", () => {
    expect(() => cleanSyncCode("OBCDEFGHJKMNPQRSTUVWXY23")).toThrow(
      "Invalid character",
    );
  });

  it("rejects invalid character 1", () => {
    expect(() => cleanSyncCode("1BCDEFGHJKMNPQRSTUVWXY23")).toThrow(
      "Invalid character",
    );
  });

  it("rejects invalid character I", () => {
    expect(() => cleanSyncCode("IBCDEFGHJKMNPQRSTUVWXY23")).toThrow(
      "Invalid character",
    );
  });

  it("rejects invalid character L", () => {
    expect(() => cleanSyncCode("LBCDEFGHJKMNPQRSTUVWXY23")).toThrow(
      "Invalid character",
    );
  });
});

describe("formatSyncCode", () => {
  it("formats as XXXX-XXXX-XXXX-XXXX-XXXX-XXXX", () => {
    const result = formatSyncCode("ABCDEFGHJKMNPQRSTUVWXY23");
    expect(result).toBe("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
  });

  it("handles already-dashed input", () => {
    const result = formatSyncCode("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
    expect(result).toBe("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
  });
});

describe("parseSyncUri", () => {
  it("parses a valid sync URI with hosts and port", () => {
    const result = parseSyncUri(
      "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&hosts=192.168.1.1&port=8080",
    );
    expect(result.code).toBe("ABCDEFGHJKMNPQRSTUVWXY23");
    expect(result.hosts).toEqual(["192.168.1.1"]);
    expect(result.port).toBe(8080);
  });

  it("parses multiple comma-separated hosts", () => {
    const result = parseSyncUri(
      "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&hosts=192.168.1.1,10.0.0.1&port=9090",
    );
    expect(result.hosts).toEqual(["192.168.1.1", "10.0.0.1"]);
  });

  it("falls back to single host param", () => {
    const result = parseSyncUri(
      "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1&port=8080",
    );
    expect(result.hosts).toEqual(["192.168.1.1"]);
  });

  it("falls back to ws param for port", () => {
    const result = parseSyncUri(
      "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1&ws=9090",
    );
    expect(result.port).toBe(9090);
  });

  it("rejects invalid scheme", () => {
    expect(() =>
      parseSyncUri(
        "https://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1&port=8080",
      ),
    ).toThrow("Invalid sync URI");
  });

  it("rejects missing code", () => {
    expect(() =>
      parseSyncUri("ghost-auth://sync?host=192.168.1.1&port=8080"),
    ).toThrow("Missing code or host");
  });

  it("rejects missing host", () => {
    expect(() =>
      parseSyncUri(
        "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&port=8080",
      ),
    ).toThrow("Missing code or host");
  });

  it("rejects port 0", () => {
    expect(() =>
      parseSyncUri(
        "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1&port=0",
      ),
    ).toThrow("Invalid or missing port");
  });

  it("rejects port > 65535", () => {
    expect(() =>
      parseSyncUri(
        "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1&port=70000",
      ),
    ).toThrow("Invalid or missing port");
  });

  it("rejects missing port entirely", () => {
    expect(() =>
      parseSyncUri(
        "ghost-auth://sync?code=ABCDEFGHJKMNPQRSTUVWXY23&host=192.168.1.1",
      ),
    ).toThrow("Invalid or missing port");
  });
});

describe("keyFromSyncCode", () => {
  it("produces a 32-byte key", async () => {
    const key = await keyFromSyncCode("ABCDEFGHJKMNPQRSTUVWXY23");
    expect(key.length).toBe(32);
  });

  it("is deterministic for the same code", async () => {
    const code = "ABCDEFGHJKMNPQRSTUVWXY23";
    const k1 = await keyFromSyncCode(code);
    const k2 = await keyFromSyncCode(code);
    expect(k1).toEqual(k2);
  });

  it("produces different keys for different codes", async () => {
    const k1 = await keyFromSyncCode("ABCDEFGHJKMNPQRSTUVWXY23");
    const k2 = await keyFromSyncCode("ABCDEFGHJKMNPQRSTUVWXY24");
    expect(k1).not.toEqual(k2);
  });

  it("normalizes case before deriving key", async () => {
    const k1 = await keyFromSyncCode("abcdefghjkmnpqrstuvwxy23");
    const k2 = await keyFromSyncCode("ABCDEFGHJKMNPQRSTUVWXY23");
    expect(k1).toEqual(k2);
  });

  it("strips dashes before deriving key", async () => {
    const k1 = await keyFromSyncCode("ABCD-EFGH-JKMN-PQRS-TUVW-XY23");
    const k2 = await keyFromSyncCode("ABCDEFGHJKMNPQRSTUVWXY23");
    expect(k1).toEqual(k2);
  });
});
