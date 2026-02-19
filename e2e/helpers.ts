import type { Page } from "@playwright/test";

/**
 * Mock the Tauri invoke API in the browser context.
 * Mocks are keyed by command name and return the specified value.
 */
export async function mockTauriInvoke(
  page: Page,
  mocks: Record<string, unknown>,
) {
  await page.addInitScript((mocksArg) => {
    (window as Record<string, unknown>).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args?: unknown) => {
        if (cmd in mocksArg) {
          const val = (mocksArg as Record<string, unknown>)[cmd];
          if (
            val !== null &&
            typeof val === "object" &&
            "error" in (val as Record<string, unknown>)
          ) {
            throw (val as Record<string, unknown>).error;
          }
          return val;
        }
        console.warn(`Unmocked Tauri command: ${cmd}`, args);
        return null;
      },
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
    };
  }, mocks);
}
