import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BackupImport from "./BackupImport.svelte";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("BackupImport", () => {
  it("renders file input and password field", () => {
    render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess: vi.fn() },
    });

    expect(screen.getByText(/choose .ghostauth file/i)).toBeTruthy();
    expect(screen.getByLabelText(/password/i)).toBeTruthy();
  });

  it("decrypt button is disabled without file", () => {
    render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess: vi.fn() },
    });

    const decryptBtn = screen.getByText("Decrypt");
    expect(decryptBtn.hasAttribute("disabled")).toBe(true);
  });

  it("shows wrong password error on preview", async () => {
    mockInvoke.mockRejectedValueOnce(
      "Decryption failed — wrong password or corrupted file",
    );

    const { container } = render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess: vi.fn() },
    });

    // Simulate file selection
    const fileInput = container.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "test.ghostauth");
    await fireEvent.change(fileInput, { target: { files: [file] } });

    // Enter password
    const pwdInput = screen.getByLabelText(/password/i);
    await fireEvent.input(pwdInput, { target: { value: "wrongpassword" } });

    // Wait for file to be read, then submit
    await waitFor(() => {
      const btn = screen.getByText("Decrypt");
      expect(btn.hasAttribute("disabled")).toBe(false);
    });

    await fireEvent.click(screen.getByText("Decrypt"));

    await waitFor(() => {
      expect(screen.getByText(/wrong password/i)).toBeTruthy();
    });
  });

  it("shows preview after successful decrypt", async () => {
    mockInvoke.mockResolvedValueOnce({
      accounts: [
        {
          id: "1",
          issuer: "GitHub",
          label: "user@test.com",
          algorithm: "SHA1",
          digits: 6,
          period: 30,
          icon: null,
        },
        {
          id: "2",
          issuer: "Google",
          label: "me@gmail.com",
          algorithm: "SHA256",
          digits: 8,
          period: 30,
          icon: null,
        },
      ],
      duplicates: 0,
    });

    const { container } = render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess: vi.fn() },
    });

    // Simulate file selection
    const fileInput = container.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "test.ghostauth");
    await fireEvent.change(fileInput, { target: { files: [file] } });

    const pwdInput = screen.getByLabelText(/password/i);
    await fireEvent.input(pwdInput, { target: { value: "correctpassword" } });

    await waitFor(() => {
      const btn = screen.getByText("Decrypt");
      expect(btn.hasAttribute("disabled")).toBe(false);
    });

    await fireEvent.click(screen.getByText("Decrypt"));

    await waitFor(() => {
      expect(screen.getByText("2 accounts found.")).toBeTruthy();
      expect(screen.getByText("GitHub")).toBeTruthy();
      expect(screen.getByText("Google")).toBeTruthy();
    });
  });

  it("calls onsuccess after confirming import", async () => {
    // First call: preview
    mockInvoke.mockResolvedValueOnce({
      accounts: [
        {
          id: "1",
          issuer: "GitHub",
          label: "user@test.com",
          algorithm: "SHA1",
          digits: 6,
          period: 30,
          icon: null,
        },
      ],
      duplicates: 0,
    });
    // Second call: confirm
    mockInvoke.mockResolvedValueOnce([
      {
        id: "1",
        issuer: "GitHub",
        label: "user@test.com",
        algorithm: "SHA1",
        digits: 6,
        period: 30,
        icon: null,
      },
    ]);

    const onsuccess = vi.fn();
    const { container } = render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess },
    });

    // Simulate file + password + decrypt
    const fileInput = container.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "test.ghostauth");
    await fireEvent.change(fileInput, { target: { files: [file] } });

    const pwdInput = screen.getByLabelText(/password/i);
    await fireEvent.input(pwdInput, { target: { value: "correctpassword" } });

    await waitFor(() => {
      const btn = screen.getByText("Decrypt");
      expect(btn.hasAttribute("disabled")).toBe(false);
    });

    await fireEvent.click(screen.getByText("Decrypt"));

    await waitFor(() => {
      expect(screen.getByText("Import")).toBeTruthy();
    });

    await fireEvent.click(screen.getByText("Import"));

    await waitFor(() => expect(onsuccess).toHaveBeenCalled());
  });

  it("back button returns to file selection from preview", async () => {
    mockInvoke.mockResolvedValueOnce({
      accounts: [
        {
          id: "1",
          issuer: "GitHub",
          label: "user@test.com",
          algorithm: "SHA1",
          digits: 6,
          period: 30,
          icon: null,
        },
      ],
      duplicates: 0,
    });

    const { container } = render(BackupImport, {
      props: { onclose: vi.fn(), onsuccess: vi.fn() },
    });

    // File + password + decrypt
    const fileInput = container.querySelector(
      'input[type="file"]',
    ) as HTMLInputElement;
    const file = new File([new Uint8Array([1, 2, 3])], "test.ghostauth");
    await fireEvent.change(fileInput, { target: { files: [file] } });

    const pwdInput = screen.getByLabelText(/password/i);
    await fireEvent.input(pwdInput, { target: { value: "correctpassword" } });

    await waitFor(() => {
      const btn = screen.getByText("Decrypt");
      expect(btn.hasAttribute("disabled")).toBe(false);
    });

    await fireEvent.click(screen.getByText("Decrypt"));

    await waitFor(() => {
      expect(screen.getByText("GitHub")).toBeTruthy();
    });

    await fireEvent.click(screen.getByText("Back"));

    await waitFor(() => {
      // After going back, the file input form reappears with the previously selected filename
      expect(screen.getByText("test.ghostauth")).toBeTruthy();
      expect(screen.getByLabelText(/password/i)).toBeTruthy();
    });
  });
});
