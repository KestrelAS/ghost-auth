import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import LockScreen from "./LockScreen.svelte";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("LockScreen", () => {
  it("renders no PIN dots initially", () => {
    const onunlock = vi.fn();
    const { container } = render(LockScreen, { props: { onunlock } });
    const dots = container.querySelectorAll(".rounded-full");
    expect(dots.length).toBe(0);
  });

  it("fills dots when numpad digits are pressed", async () => {
    const onunlock = vi.fn();
    const { container } = render(LockScreen, { props: { onunlock } });

    await fireEvent.click(screen.getByText("1"));
    await fireEvent.click(screen.getByText("2"));
    await fireEvent.click(screen.getByText("3"));

    const filled = container.querySelectorAll(".bg-fg");
    expect(filled.length).toBe(3);
  });

  it("fills dots on keyboard digit input", async () => {
    const onunlock = vi.fn();
    const { container } = render(LockScreen, { props: { onunlock } });

    await fireEvent.keyDown(window, { key: "5" });
    await fireEvent.keyDown(window, { key: "6" });

    const filled = container.querySelectorAll(".bg-fg");
    expect(filled.length).toBe(2);
  });

  it("removes last digit on Backspace key", async () => {
    const onunlock = vi.fn();
    const { container } = render(LockScreen, { props: { onunlock } });

    await fireEvent.keyDown(window, { key: "1" });
    await fireEvent.keyDown(window, { key: "2" });
    await fireEvent.keyDown(window, { key: "3" });
    await fireEvent.keyDown(window, { key: "Backspace" });

    const filled = container.querySelectorAll(".bg-fg");
    expect(filled.length).toBe(2);
  });

  it("calls onunlock after correct PIN", async () => {
    const onunlock = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "has_recovery_codes") return false;
      if (cmd === "verify_pin") return true;
      return undefined;
    });

    render(LockScreen, { props: { onunlock } });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    // After 4+ digits, the bottom-right button shows "ok"
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => expect(onunlock).toHaveBeenCalled());
  });

  it("shows error on incorrect PIN", async () => {
    const onunlock = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "has_recovery_codes") return false;
      if (cmd === "verify_pin") return false;
      return undefined;
    });

    render(LockScreen, { props: { onunlock } });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("incorrect pin")).toBeTruthy();
    });
    expect(onunlock).not.toHaveBeenCalled();
  });

  it("shows rate limiting error", async () => {
    const onunlock = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "has_recovery_codes") return false;
      if (cmd === "verify_pin") throw "Too many attempts. Try again in 30 seconds.";
      return undefined;
    });

    render(LockScreen, { props: { onunlock } });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText(/too many attempts/i)).toBeTruthy();
    });
  });

  it("shows del button when biometricEnabled is false", () => {
    const onunlock = vi.fn();
    render(LockScreen, { props: { onunlock, biometricEnabled: false } });
    expect(screen.getByText("del")).toBeTruthy();
  });

  it("submits on Enter key after 4 digits", async () => {
    const onunlock = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "has_recovery_codes") return false;
      if (cmd === "verify_pin") return true;
      return undefined;
    });

    render(LockScreen, { props: { onunlock } });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.keyDown(window, { key: d });
    }
    await fireEvent.keyDown(window, { key: "Enter" });

    await waitFor(() => expect(onunlock).toHaveBeenCalled());
  });
});
