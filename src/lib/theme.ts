/**
 * Theme handling.
 *
 * The palette lives in `app.css` and is switched by the `data-theme`
 * attribute on `<html>`. This module keeps that attribute, `localStorage` and
 * the operating system preference in sync.
 */

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "matrix-telegram.theme";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function readStoredMode(): ThemeMode | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored === "light" || stored === "dark" || stored === "system" ? stored : null;
  } catch {
    return null;
  }
}

function systemMode(): ResolvedTheme {
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

/// The preference the user picked: "system" follows the OS.
export function preferredMode(): ThemeMode {
  return readStoredMode() ?? "system";
}

/// The theme that is actually applied right now.
export function currentMode(): ResolvedTheme {
  const attribute = document.documentElement.getAttribute("data-theme");
  if (attribute === "light" || attribute === "dark") {
    return attribute;
  }

  const preferred = preferredMode();
  return preferred === "system" ? systemMode() : preferred;
}

export function applyMode(mode: ThemeMode): void {
  const resolved: ResolvedTheme = mode === "system" ? systemMode() : mode;

  document.documentElement.setAttribute("data-theme", resolved);
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // A webview with a blocked storage partition still gets the in-memory theme.
  }
}

export function setMode(mode: ThemeMode): void {
  applyMode(mode);
}

export function toggleMode(): ResolvedTheme {
  const next: ResolvedTheme = currentMode() === "dark" ? "light" : "dark";
  applyMode(next);
  return next;
}

/// Follow the OS preference while the user has not chosen one explicitly.
export function watchSystemTheme(): () => void {
  const media = window.matchMedia(DARK_QUERY);
  const listener = () => {
    if (preferredMode() === "system") {
      applyMode("system");
    }
  };

  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}
