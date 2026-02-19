import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BackupExport from "./BackupExport.svelte";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("BackupExport", () => {
  it("renders password and confirm fields", () => {
    render(BackupExport, { props: { onclose: vi.fn() } });

    expect(screen.getByLabelText(/^password$/i)).toBeTruthy();
    expect(screen.getByLabelText(/confirm password/i)).toBeTruthy();
  });

  it("rejects short password", async () => {
    render(BackupExport, { props: { onclose: vi.fn() } });

    const pwd = screen.getByLabelText(/^password$/i);
    const confirm = screen.getByLabelText(/confirm password/i);

    await fireEvent.input(pwd, { target: { value: "abc" } });
    await fireEvent.input(confirm, { target: { value: "abc" } });
    await fireEvent.click(screen.getByText("Export"));

    await waitFor(() => {
      expect(
        screen.getByText(/password must be at least 8 characters/i),
      ).toBeTruthy();
    });
  });

  it("rejects mismatched passwords", async () => {
    render(BackupExport, { props: { onclose: vi.fn() } });

    const pwd = screen.getByLabelText(/^password$/i);
    const confirm = screen.getByLabelText(/confirm password/i);

    await fireEvent.input(pwd, { target: { value: "password1234" } });
    await fireEvent.input(confirm, { target: { value: "different1234" } });
    await fireEvent.click(screen.getByText("Export"));

    await waitFor(() => {
      expect(screen.getByText(/passwords don't match/i)).toBeTruthy();
    });
  });

  it("calls export_backup on valid submit", async () => {
    mockInvoke.mockResolvedValueOnce([1, 2, 3]);

    // Mock URL.createObjectURL and URL.revokeObjectURL
    const mockCreateObjectURL = vi.fn().mockReturnValue("blob:test");
    const mockRevokeObjectURL = vi.fn();
    globalThis.URL.createObjectURL = mockCreateObjectURL;
    globalThis.URL.revokeObjectURL = mockRevokeObjectURL;

    const onclose = vi.fn();
    render(BackupExport, { props: { onclose } });

    const pwd = screen.getByLabelText(/^password$/i);
    const confirm = screen.getByLabelText(/confirm password/i);

    await fireEvent.input(pwd, { target: { value: "strongpassword" } });
    await fireEvent.input(confirm, { target: { value: "strongpassword" } });
    await fireEvent.click(screen.getByText("Export"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("export_backup", {
        password: "strongpassword",
      });
    });
  });

  it("shows backend error", async () => {
    mockInvoke.mockRejectedValueOnce("Export failed");

    render(BackupExport, { props: { onclose: vi.fn() } });

    const pwd = screen.getByLabelText(/^password$/i);
    const confirm = screen.getByLabelText(/confirm password/i);

    await fireEvent.input(pwd, { target: { value: "password1234" } });
    await fireEvent.input(confirm, { target: { value: "password1234" } });
    await fireEvent.click(screen.getByText("Export"));

    await waitFor(() => {
      expect(screen.getByText(/Export failed/)).toBeTruthy();
    });
  });

  it("fires onclose on cancel", async () => {
    const onclose = vi.fn();
    render(BackupExport, { props: { onclose } });

    await fireEvent.click(screen.getByText("Cancel"));

    await waitFor(() => expect(onclose).toHaveBeenCalled(), { timeout: 500 });
  });
});
