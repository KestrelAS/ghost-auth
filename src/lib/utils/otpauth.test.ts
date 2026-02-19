import { describe, it, expect } from "vitest";
import { parseOtpAuthUri, buildOtpAuthUri } from "./otpauth";

describe("parseOtpAuthUri", () => {
  it("parses a standard URI with all parameters", () => {
    const uri =
      "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA256&digits=8&period=60";
    const result = parseOtpAuthUri(uri);
    expect(result.issuer).toBe("GitHub");
    expect(result.label).toBe("user@example.com");
    expect(result.secret).toBe("JBSWY3DPEHPK3PXP");
    expect(result.algorithm).toBe("SHA256");
    expect(result.digits).toBe(8);
    expect(result.period).toBe(60);
  });

  it("uses defaults for missing optional params", () => {
    const uri =
      "otpauth://totp/Service:user?secret=JBSWY3DPEHPK3PXP&issuer=Service";
    const result = parseOtpAuthUri(uri);
    expect(result.algorithm).toBe("SHA1");
    expect(result.digits).toBe(6);
    expect(result.period).toBe(30);
  });

  it("extracts issuer from path when param is missing", () => {
    const uri = "otpauth://totp/MyApp:me@test.com?secret=JBSWY3DPEHPK3PXP";
    const result = parseOtpAuthUri(uri);
    expect(result.issuer).toBe("MyApp");
    expect(result.label).toBe("me@test.com");
  });

  it("throws on non-otpauth protocol", () => {
    expect(() => parseOtpAuthUri("https://example.com")).toThrow();
  });

  it("throws on HOTP (non-TOTP)", () => {
    expect(() =>
      parseOtpAuthUri("otpauth://hotp/Test?secret=JBSWY3DPEHPK3PXP"),
    ).toThrow();
  });

  it("throws when secret is missing", () => {
    expect(() => parseOtpAuthUri("otpauth://totp/Test?issuer=X")).toThrow();
  });

  it("uppercases the secret", () => {
    const uri = "otpauth://totp/X:y?secret=jbswy3dpehpk3pxp";
    expect(parseOtpAuthUri(uri).secret).toBe("JBSWY3DPEHPK3PXP");
  });
});

describe("buildOtpAuthUri", () => {
  it("produces a valid roundtrip", () => {
    const params = {
      issuer: "GitHub",
      label: "user@example.com",
      secret: "JBSWY3DPEHPK3PXP",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
    };
    const uri = buildOtpAuthUri(params);
    expect(uri).toContain("otpauth://totp/");
    expect(uri).toContain("secret=JBSWY3DPEHPK3PXP");
    const reparsed = parseOtpAuthUri(uri);
    expect(reparsed.issuer).toBe("GitHub");
    expect(reparsed.label).toBe("user@example.com");
  });
});
