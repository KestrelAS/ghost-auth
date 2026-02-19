import { vi, beforeEach } from "vitest";
import { webcrypto } from "node:crypto";

// Polyfill crypto.subtle for jsdom (which doesn't expose it)
if (!globalThis.crypto?.subtle) {
  Object.defineProperty(globalThis, "crypto", {
    value: webcrypto,
    writable: true,
  });
}

// In-memory chrome.storage mock
function createStorageArea() {
  let store: Record<string, unknown> = {};

  return {
    get: vi.fn(
      async (
        keys?: string | string[] | Record<string, unknown> | null,
      ): Promise<Record<string, unknown>> => {
        if (keys === null || keys === undefined) {
          return { ...store };
        }
        if (typeof keys === "string") {
          return { [keys]: store[keys] };
        }
        if (Array.isArray(keys)) {
          const result: Record<string, unknown> = {};
          for (const k of keys) {
            if (k in store) result[k] = store[k];
          }
          return result;
        }
        // Object form: keys are requested, values are defaults
        const result: Record<string, unknown> = {};
        for (const [k, defaultVal] of Object.entries(keys)) {
          result[k] = k in store ? store[k] : defaultVal;
        }
        return result;
      },
    ),
    set: vi.fn(async (items: Record<string, unknown>): Promise<void> => {
      Object.assign(store, items);
    }),
    remove: vi.fn(async (keys: string | string[]): Promise<void> => {
      const keyList = typeof keys === "string" ? [keys] : keys;
      for (const k of keyList) {
        delete store[k];
      }
    }),
    clear: vi.fn(async (): Promise<void> => {
      store = {};
    }),
    // Test helpers (not part of the Chrome API)
    _getStore: () => store,
    _reset: () => {
      store = {};
    },
  };
}

const localStorage = createStorageArea();
const sessionStorage = createStorageArea();

Object.defineProperty(globalThis, "chrome", {
  value: {
    storage: {
      local: localStorage,
      session: sessionStorage,
    },
  },
  writable: true,
});

// Reset storage between tests
beforeEach(() => {
  localStorage._reset();
  sessionStorage._reset();
  vi.clearAllMocks();
});
