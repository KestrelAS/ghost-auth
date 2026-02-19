import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PinRemove from "./PinRemove.svelte";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("PinRemove", () => {
  it("renders enter pin to remove prompt", () => {
    render(PinRemove, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });
    expect(screen.getByText("Enter PIN to remove")).toBeTruthy();
  });

  it("calls ondone on correct PIN", async () => {
    const ondone = vi.fn();
    mockInvoke.mockResolvedValueOnce(undefined);

    render(PinRemove, {
      props: { onclose: vi.fn(), ondone },
    });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => expect(ondone).toHaveBeenCalled());
  });

  it("shows incorrect pin error on wrong PIN", async () => {
    mockInvoke.mockRejectedValueOnce("Incorrect PIN");

    render(PinRemove, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    for (const d of ["9", "9", "9", "9"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("incorrect pin")).toBeTruthy();
    });
  });

  it("shows rate limiting error", async () => {
    mockInvoke.mockRejectedValueOnce(
      "Too many attempts. Try again in 30 seconds.",
    );

    render(PinRemove, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText(/too many attempts/i)).toBeTruthy();
    });
  });

  it("fires onclose on Escape", async () => {
    const onclose = vi.fn();
    render(PinRemove, { props: { onclose, ondone: vi.fn() } });

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });

  it("fires onclose on cancel button click", async () => {
    const onclose = vi.fn();
    render(PinRemove, { props: { onclose, ondone: vi.fn() } });

    const cancelBtn = screen.getByLabelText("Cancel");
    await fireEvent.click(cancelBtn);
    expect(onclose).toHaveBeenCalled();
  });
});
