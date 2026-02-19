import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { scan } from "@tauri-apps/plugin-barcode-scanner";
import AddAccount from "./AddAccount.svelte";

const mockInvoke = vi.mocked(invoke);
const mockScan = vi.mocked(scan);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("AddAccount", () => {
  it("renders choose mode with three options", () => {
    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });
    expect(screen.getByText("scan qr code")).toBeTruthy();
    expect(screen.getByText("manual entry")).toBeTruthy();
    expect(screen.getByText("paste uri")).toBeTruthy();
  });

  it("navigates to manual entry mode", async () => {
    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("manual entry"));

    expect(screen.getByLabelText(/service/i)).toBeTruthy();
    expect(screen.getByLabelText(/^account$/i)).toBeTruthy();
    expect(screen.getByLabelText(/secret key/i)).toBeTruthy();
  });

  it("rejects empty secret in manual entry", async () => {
    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("manual entry"));
    await fireEvent.click(screen.getByText("Add"));

    await waitFor(() => {
      expect(screen.getByText(/secret key is required/i)).toBeTruthy();
    });
  });

  it("calls onsuccess on successful manual entry", async () => {
    const onsuccess = vi.fn();
    mockInvoke.mockResolvedValueOnce({
      id: "1",
      issuer: "GitHub",
      label: "user",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
      icon: null,
    });

    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess, onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("manual entry"));

    const secretInput = screen.getByLabelText(/secret key/i);
    await fireEvent.input(secretInput, {
      target: { value: "JBSWY3DPEHPK3PXP" },
    });
    await fireEvent.click(screen.getByText("Add"));

    await waitFor(() => expect(onsuccess).toHaveBeenCalled());
  });

  it("shows backend error on manual entry failure", async () => {
    mockInvoke.mockRejectedValueOnce("Invalid account secret");

    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("manual entry"));

    const secretInput = screen.getByLabelText(/secret key/i);
    await fireEvent.input(secretInput, { target: { value: "BADSECRET" } });
    await fireEvent.click(screen.getByText("Add"));

    await waitFor(() => {
      expect(screen.getByText(/Invalid account secret/)).toBeTruthy();
    });
  });

  it("navigates to URI mode and rejects empty URI", async () => {
    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("paste uri"));
    await fireEvent.click(screen.getByText("Add"));

    await waitFor(() => {
      expect(screen.getByText(/uri is required/i)).toBeTruthy();
    });
  });

  it("submits URI successfully", async () => {
    const onsuccess = vi.fn();
    mockInvoke.mockResolvedValueOnce({
      id: "1",
      issuer: "GitHub",
      label: "user",
      algorithm: "SHA1",
      digits: 6,
      period: 30,
      icon: null,
    });

    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess, onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("paste uri"));

    const textarea = screen.getByPlaceholderText(/otpauth:\/\//);
    await fireEvent.input(textarea, {
      target: { value: "otpauth://totp/GitHub:user?secret=JBSWY3DPEHPK3PXP" },
    });
    await fireEvent.click(screen.getByText("Add"));

    await waitFor(() => expect(onsuccess).toHaveBeenCalled());
  });

  it("shows web scanner fallback on desktop", async () => {
    mockScan.mockRejectedValueOnce(new Error("not implemented"));

    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("scan qr code"));

    await waitFor(() => {
      expect(screen.getByLabelText("Close camera")).toBeTruthy();
    });
  });

  it("shows camera permission error", async () => {
    mockScan.mockRejectedValueOnce(new Error("camera permission denied"));

    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("scan qr code"));

    await waitFor(() => {
      expect(screen.getByText(/camera permission denied/i)).toBeTruthy();
    });
  });

  it("fires onclose on close button click", async () => {
    const onclose = vi.fn();
    render(AddAccount, {
      props: { onclose, onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    const closeBtn = screen.getByLabelText("Close");
    await fireEvent.click(closeBtn);

    // Close uses setTimeout(onclose, 250)
    await waitFor(() => expect(onclose).toHaveBeenCalled(), { timeout: 500 });
  });

  it("back button in manual mode returns to choose mode", async () => {
    render(AddAccount, {
      props: { onclose: vi.fn(), onsuccess: vi.fn(), onmigration: vi.fn(), onimportexternal: vi.fn() },
    });

    await fireEvent.click(screen.getByText("manual entry"));
    expect(screen.getByLabelText(/secret key/i)).toBeTruthy();

    await fireEvent.click(screen.getByText("Back"));

    await waitFor(() => {
      expect(screen.getByText("scan qr code")).toBeTruthy();
    });
  });
});
