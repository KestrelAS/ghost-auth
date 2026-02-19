export type Theme = "light" | "dark";

const STORAGE_KEY = "ghost-auth-theme";

function getSystemPreference(): Theme {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

function getInitialTheme(): Theme {
  if (typeof window === "undefined") return "light";
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "light" || stored === "dark") return stored;
  return "light";
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
  // Update the native window theme so the macOS title bar matches
  import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => {
      getCurrentWindow().setTheme(theme).catch(() => {});
    })
    .catch(() => {});
  // Persist for Rust to read on next launch (sets title bar before JS loads)
  import("@tauri-apps/api/core")
    .then(({ invoke }) => {
      invoke("save_theme", { theme }).catch(() => {});
    })
    .catch(() => {});
}

let current: Theme = $state(getInitialTheme());

export function getTheme(): Theme {
  return current;
}

export function setTheme(theme: Theme) {
  current = theme;
  localStorage.setItem(STORAGE_KEY, theme);
  applyTheme(theme);
}

export function toggleTheme() {
  setTheme(current === "dark" ? "light" : "dark");
}

// Apply on load
if (typeof window !== "undefined") {
  applyTheme(getTheme());

  // Follow system preference when no explicit preference stored
  window
    .matchMedia("(prefers-color-scheme: light)")
    .addEventListener("change", (e) => {
      if (!localStorage.getItem(STORAGE_KEY)) {
        const next = e.matches ? "light" : "dark";
        current = next;
        applyTheme(next);
      }
    });
}
