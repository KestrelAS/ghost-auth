import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PinSetup from "./PinSetup.svelte";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("PinSetup", () => {
  it("renders choose a pin prompt initially", () => {
    const onclose = vi.fn();
    const ondone = vi.fn();
    render(PinSetup, { props: { onclose, ondone } });
    expect(screen.getByText("Choose a PIN")).toBeTruthy();
  });

  it("fills PIN dots as digits are entered", async () => {
    const { container } = render(PinSetup, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }

    const filled = container.querySelectorAll(".bg-white");
    expect(filled.length).toBe(4);
  });

  it("advances to confirm step after first PIN entry", async () => {
    render(PinSetup, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("Confirm PIN")).toBeTruthy();
    });
  });

  it("calls ondone when PINs match", async () => {
    const ondone = vi.fn();
    mockInvoke.mockResolvedValueOnce(["ABCD-1234", "EFGH-5678"]);

    render(PinSetup, {
      props: { onclose: vi.fn(), ondone },
    });

    // Enter first PIN
    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("Confirm PIN")).toBeTruthy();
    });

    // Enter matching PIN
    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    // Recovery codes are shown first
    await waitFor(() => {
      expect(screen.getByText("Recovery codes")).toBeTruthy();
    });

    // Confirm codes saved
    await fireEvent.click(screen.getByText("I've saved these"));

    await waitFor(() => expect(ondone).toHaveBeenCalled());
  });

  it("shows error when PINs don't match", async () => {
    render(PinSetup, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    // Enter first PIN: 1234
    for (const d of ["1", "2", "3", "4"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("Confirm PIN")).toBeTruthy();
    });

    // Enter different PIN: 5678
    for (const d of ["5", "6", "7", "8"]) {
      await fireEvent.click(screen.getByText(d));
    }
    await fireEvent.click(screen.getByText("ok"));

    await waitFor(() => {
      expect(screen.getByText("pins don't match")).toBeTruthy();
    });
  });

  it("supports keyboard input", async () => {
    const { container } = render(PinSetup, {
      props: { onclose: vi.fn(), ondone: vi.fn() },
    });

    for (const d of ["7", "8", "9", "0"]) {
      await fireEvent.keyDown(window, { key: d });
    }

    const filled = container.querySelectorAll(".bg-white");
    expect(filled.length).toBe(4);
  });

  it("fires onclose on Escape", async () => {
    const onclose = vi.fn();
    render(PinSetup, { props: { onclose, ondone: vi.fn() } });

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });

  it("fires onclose on cancel button click", async () => {
    const onclose = vi.fn();
    render(PinSetup, { props: { onclose, ondone: vi.fn() } });

    const cancelBtn = screen.getByLabelText("Cancel");
    await fireEvent.click(cancelBtn);
    expect(onclose).toHaveBeenCalled();
  });
});
