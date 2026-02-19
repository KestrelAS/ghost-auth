import { describe, it, expect } from "vitest";
import {
  aesGcmEncrypt,
  aesGcmDecrypt,
  hmacSha256,
  constantTimeEqual,
} from "./crypto";

function randomKey(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(32));
}

describe("aesGcmEncrypt / aesGcmDecrypt", () => {
  it("roundtrips plaintext correctly", async () => {
    const key = randomKey();
    const plaintext = new TextEncoder().encode("hello world");
    const { nonce, ciphertext } = await aesGcmEncrypt(key, plaintext);
    const decrypted = await aesGcmDecrypt(key, nonce, ciphertext);
    expect(Array.from(decrypted)).toEqual(Array.from(plaintext));
  });

  it("produces 12-byte nonces", async () => {
    const key = randomKey();
    const { nonce } = await aesGcmEncrypt(key, new Uint8Array(1));
    expect(nonce.length).toBe(12);
  });

  it("produces different ciphertexts for the same plaintext (random nonce)", async () => {
    const key = randomKey();
    const plaintext = new TextEncoder().encode("determinism check");
    const r1 = await aesGcmEncrypt(key, plaintext);
    const r2 = await aesGcmEncrypt(key, plaintext);
    expect(r1.nonce).not.toEqual(r2.nonce);
  });

  it("fails to decrypt with wrong key", async () => {
    const key1 = randomKey();
    const key2 = randomKey();
    const { nonce, ciphertext } = await aesGcmEncrypt(
      key1,
      new TextEncoder().encode("secret"),
    );
    await expect(aesGcmDecrypt(key2, nonce, ciphertext)).rejects.toThrow();
  });

  it("fails to decrypt with wrong nonce", async () => {
    const key = randomKey();
    const { ciphertext } = await aesGcmEncrypt(
      key,
      new TextEncoder().encode("secret"),
    );
    const wrongNonce = crypto.getRandomValues(new Uint8Array(12));
    await expect(aesGcmDecrypt(key, wrongNonce, ciphertext)).rejects.toThrow();
  });

  it("fails to decrypt tampered ciphertext (GCM authentication)", async () => {
    const key = randomKey();
    const { nonce, ciphertext } = await aesGcmEncrypt(
      key,
      new TextEncoder().encode("secret"),
    );
    ciphertext[0] ^= 0xff;
    await expect(aesGcmDecrypt(key, nonce, ciphertext)).rejects.toThrow();
  });

  it("handles empty plaintext", async () => {
    const key = randomKey();
    const { nonce, ciphertext } = await aesGcmEncrypt(key, new Uint8Array(0));
    const decrypted = await aesGcmDecrypt(key, nonce, ciphertext);
    expect(decrypted.length).toBe(0);
  });
});

describe("hmacSha256", () => {
  it("produces a 32-byte result", async () => {
    const key = randomKey();
    const data = new TextEncoder().encode("test");
    const result = await hmacSha256(key, data);
    expect(result.length).toBe(32);
  });

  it("is deterministic for the same key and data", async () => {
    const key = randomKey();
    const data = new TextEncoder().encode("determinism");
    const r1 = await hmacSha256(key, data);
    const r2 = await hmacSha256(key, data);
    expect(r1).toEqual(r2);
  });

  it("produces different output for different keys", async () => {
    const key1 = randomKey();
    const key2 = randomKey();
    const data = new TextEncoder().encode("same data");
    const r1 = await hmacSha256(key1, data);
    const r2 = await hmacSha256(key2, data);
    expect(r1).not.toEqual(r2);
  });

  it("produces different output for different data", async () => {
    const key = randomKey();
    const r1 = await hmacSha256(key, new TextEncoder().encode("data1"));
    const r2 = await hmacSha256(key, new TextEncoder().encode("data2"));
    expect(r1).not.toEqual(r2);
  });
});

describe("constantTimeEqual", () => {
  it("returns true for equal arrays", () => {
    const a = new Uint8Array([1, 2, 3, 4]);
    expect(constantTimeEqual(a, new Uint8Array([1, 2, 3, 4]))).toBe(true);
  });

  it("returns false for different arrays", () => {
    const a = new Uint8Array([1, 2, 3, 4]);
    expect(constantTimeEqual(a, new Uint8Array([1, 2, 3, 5]))).toBe(false);
  });

  it("returns false for different lengths", () => {
    const a = new Uint8Array([1, 2, 3]);
    expect(constantTimeEqual(a, new Uint8Array([1, 2, 3, 4]))).toBe(false);
  });

  it("returns true for empty arrays", () => {
    expect(constantTimeEqual(new Uint8Array(0), new Uint8Array(0))).toBe(true);
  });
});
