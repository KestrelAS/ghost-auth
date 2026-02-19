import { vi } from "vitest";

// Mock @tauri-apps/api/core so invoke() works in tests
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock clipboard plugin
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(),
}));

// Mock barcode scanner plugin
vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  scan: vi.fn(),
  Format: { QRCode: "QR_CODE" },
}));

// Mock biometric plugin
vi.mock("@tauri-apps/plugin-biometric", () => ({
  checkStatus: vi.fn().mockResolvedValue({ isAvailable: false }),
  authenticate: vi.fn(),
}));

// Mock SVG asset imports
vi.mock("$lib/assets/ghost.svg", () => ({ default: "ghost.svg" }));
