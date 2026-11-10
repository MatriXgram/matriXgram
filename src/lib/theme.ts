/**
 * Theme engine.
 *
 * The app ships two built-in palettes (light and dark) that live in
 * `app.css`, and can load Telegram Desktop themes (`.tdesktop-theme`), whose
 * colours are mapped onto the same CSS variables. On top of that the user can
 * override the accent colour, the background, the text colour and the font
 * size, the way Telegram Desktop does.
 */

import { get, writable } from "svelte/store";

import { THEME_PRESETS } from "./themes/presets";

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

export interface ThemeSettings {
  /** Which built-in palette to use when no Telegram theme is loaded. */
  mode: ThemeMode;
  /** Telegram Desktop colour names (`dialogsBg`, `msgInBg`, ...) -> `#rrggbbaa`. */
  palette: Record<string, string> | null;
  /** Name of the loaded theme, shown in the settings. */
  paletteName: string | null;
  /** Overrides applied on top of the palette. */
  accent: string | null;
  background: string | null;
  text: string | null;
  /** Base font size in pixels. */
  fontSize: number | null;
  /** Wallpaper shown behind the conversation. */
  wallpaper: string | null;
}

const STORAGE_KEY = "matrix-telegram.theme.settings";
const DARK_QUERY = "(prefers-color-scheme: dark)";

/** The theme the app starts with; it can be changed in the settings. */
const DEFAULT_PRESET = THEME_PRESETS[0];

export const DEFAULT_THEME_SETTINGS: ThemeSettings = {
  mode: "dark",
  palette: DEFAULT_PRESET?.colors ?? null,
  paletteName: DEFAULT_PRESET?.name ?? null,
  accent: null,
  background: null,
  text: null,
  fontSize: null,
  wallpaper: DEFAULT_PRESET?.wallpaper ?? null,
};

export const FONT_SIZE_MIN = 12;
export const FONT_SIZE_MAX = 22;
export const FONT_SIZE_DEFAULT = 15;

// ---------------------------------------------------------------------------
// Colour helpers
// ---------------------------------------------------------------------------

export interface Rgb {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** Parse `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` or `rgb()`/`rgba()`. */
export function parseColor(value: string): Rgb | null {
  const trimmed = value.trim();

  if (trimmed.startsWith("#")) {
    const hex = trimmed.slice(1);

    if (hex.length === 3 || hex.length === 4) {
      const [r, g, b, a] = hex.split("");
      return {
        r: Number.parseInt(r + r, 16),
        g: Number.parseInt(g + g, 16),
        b: Number.parseInt(b + b, 16),
        a: a === undefined ? 1 : Number.parseInt(a + a, 16) / 255,
      };
    }

    if (hex.length === 6 || hex.length === 8) {
      return {
        r: Number.parseInt(hex.slice(0, 2), 16),
        g: Number.parseInt(hex.slice(2, 4), 16),
        b: Number.parseInt(hex.slice(4, 6), 16),
        a: hex.length === 8 ? Number.parseInt(hex.slice(6, 8), 16) / 255 : 1,
      };
    }

    return null;
  }

  const match = trimmed.match(
    /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([0-9.]+)\s*)?\)$/i,
  );
  if (!match) {
    return null;
  }

  return {
    r: Number(match[1]),
    g: Number(match[2]),
    b: Number(match[3]),
    a: match[4] === undefined ? 1 : Number(match[4]),
  };
}

function channel(value: number): string {
  return Math.max(0, Math.min(255, Math.round(value)))
    .toString(16)
    .padStart(2, "0");
}

/** Render a colour as `#rrggbb`, or `#rrggbbaa` when it is translucent. */
export function toHex(color: Rgb): string {
  const base = `#${channel(color.r)}${channel(color.g)}${channel(color.b)}`;
  return color.a >= 1 ? base : `${base}${channel(color.a * 255)}`;
}

/** The same colour with a different alpha. */
export function withAlpha(color: Rgb, alpha: number): string {
  return toHex({ ...color, a: alpha });
}

/** Mix two colours; `amount` 0 keeps `from`, 1 returns `to`. */
export function mix(from: Rgb, to: Rgb, amount: number): Rgb {
  return {
    r: from.r + (to.r - from.r) * amount,
    g: from.g + (to.g - from.g) * amount,
    b: from.b + (to.b - from.b) * amount,
    a: from.a + (to.a - from.a) * amount,
  };
}

/** Make a colour lighter (positive) or darker (negative). */
export function shade(color: Rgb, amount: number): Rgb {
  const target = amount >= 0 ? { r: 255, g: 255, b: 255, a: 1 } : { r: 0, g: 0, b: 0, a: 1 };
  return mix(color, target, Math.abs(amount));
}

// ---------------------------------------------------------------------------
// Telegram Desktop palette -> CSS variables
// ---------------------------------------------------------------------------

/**
 * Telegram colour names mapped onto the app's CSS variables.
 *
 * The names are Telegram Desktop's own, so a theme written for the desktop
 * client lands on the right parts of the UI without any per-theme code.
 */
export const TELEGRAM_COLOR_MAP: Record<string, string> = {
  // Chat list (sidebar)
  dialogsBg: "--tg-panel",
  dialogsTextFg: "--tg-text",
  dialogsChatIconFg: "--tg-text-secondary",
  dialogsBgActive: "--tg-active",
  dialogsUnreadBg: "--tg-badge-bg",
  dialogsUnreadFg: "--tg-badge-fg",
  dialogsUnreadBgMuted: "--tg-badge-bg-muted",
  sideBarTextFg: "--tg-text",
  sideBarIconFg: "--tg-text-secondary",
  sideBarTextFgActive: "--tg-accent",
  sideBarIconFgActive: "--tg-accent",
  sideBarBadgeBg: "--tg-accent",
  sideBarBadgeFg: "--tg-on-accent",

  // Window and conversation
  windowBg: "--tg-app-bg",
  windowFg: "--tg-text",
  windowBoldFg: "--tg-text",
  historyComposeAreaBg: "--tg-panel-alt",
  historyTextInFg: "--tg-bubble-in-text",
  historyTextOutFg: "--tg-bubble-out-text",
  historyLinkInFg: "--tg-accent",
  historyLinkOutFg: "--tg-accent",
  historySendIconFg: "--tg-accent",
  historySendIconFgOver: "--tg-accent-hover",
  historyPinnedBg: "--tg-pinned",
  historyUnreadBarBg: "--tg-unread-bar",
  historyReplyIconFg: "--tg-accent",
  historyReplyCancelFg: "--tg-accent",
  historyFileThumbIconFg: "--tg-accent",
  historyTitleFg: "--tg-text",

  // Bubbles
  msgInBg: "--tg-bubble-in",
  msgOutBg: "--tg-bubble-out",
  msgServiceBg: "--tg-service-bg",
  msgServiceFg: "--tg-text-secondary",
  msgInDateFg: "--tg-text-muted",
  msgOutDateFg: "--tg-text-muted",
  msgInReplyBarColor: "--tg-accent",
  msgOutReplyBarColor: "--tg-accent",
  msgDateImgBg: "--tg-hover",

  // Controls and menus
  placeholderFg: "--tg-text-muted",
  scrollBg: "--tg-scrollbar",
  scrollBgOver: "--tg-scrollbar-hover",
  toastBg: "--tg-elevated",
  activeLineFg: "--tg-accent",
  lightButtonBg: "--tg-panel-alt",
  lightButtonFg: "--tg-accent",
  activeButtonBg: "--tg-accent",
  activeButtonFg: "--tg-on-accent",
  menuBg: "--tg-elevated",
  boxTitleCloseFg: "--tg-accent",
  photoEditorBg: "--tg-app-bg",
  msgServiceFgSelected: "--tg-text",
};

/** Every CSS variable the palette can touch. */
const PALETTE_VARIABLES = [...new Set(Object.values(TELEGRAM_COLOR_MAP))];

// ---------------------------------------------------------------------------
// Settings store
// ---------------------------------------------------------------------------

function loadSettings(): ThemeSettings {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) {
      return { ...DEFAULT_THEME_SETTINGS };
    }

    const parsed = JSON.parse(stored) as Partial<ThemeSettings>;
    return {
      ...DEFAULT_THEME_SETTINGS,
      ...parsed,
      mode:
        parsed.mode === "light" || parsed.mode === "dark" || parsed.mode === "system"
          ? parsed.mode
          : DEFAULT_THEME_SETTINGS.mode,
    };
  } catch {
    return { ...DEFAULT_THEME_SETTINGS };
  }
}

export const themeSettings = writable<ThemeSettings>(loadSettings());

function persist(settings: ThemeSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // A webview with a blocked storage partition keeps the theme in memory.
  }
}

function systemMode(): ResolvedTheme {
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

/** The theme that is applied right now. */
export function resolvedMode(settings: ThemeSettings): ResolvedTheme {
  if (settings.palette) {
    // A Telegram theme carries its own colours; they win over the built-in
    // light and dark palettes.
    return "dark";
  }

  return settings.mode === "system" ? systemMode() : settings.mode;
}

/** The theme that is applied right now, read from the store. */
export function currentMode(): ResolvedTheme {
  return resolvedMode(get(themeSettings));
}

/** Switch between the light and the dark palette. */
export function toggleMode(): ResolvedTheme {
  const next: ResolvedTheme = currentMode() === "dark" ? "light" : "dark";
  updateThemeSettings({ mode: next, palette: null, paletteName: null });
  return next;
}

/**
 * Write the theme onto the document.
 *
 * The built-in palettes live in `app.css` and are selected by the
 * `data-theme` attribute; everything else is written as an inline custom
 * property, which wins over the stylesheet.
 */
export function applySettings(settings: ThemeSettings): void {
  const root = document.documentElement;

  root.setAttribute("data-theme", resolvedMode(settings));

  for (const variable of PALETTE_VARIABLES) {
    root.style.removeProperty(variable);
  }

  if (settings.palette) {
    for (const [key, cssVariable] of Object.entries(TELEGRAM_COLOR_MAP)) {
      const value = settings.palette[key];
      if (value) {
        root.style.setProperty(cssVariable, value);
      }
    }
  }

  applyOverrides(settings, root);

  if (settings.fontSize) {
    root.style.setProperty("--tg-font-size", `${settings.fontSize}px`);
  } else {
    root.style.removeProperty("--tg-font-size");
  }

  if (settings.wallpaper) {
    root.style.setProperty("--tg-wallpaper", `url("${settings.wallpaper}")`);
  } else {
    root.style.removeProperty("--tg-wallpaper");
  }
}

/** Apply the user's manual overrides on top of the palette. */
function applyOverrides(settings: ThemeSettings, root: HTMLElement): void {
  if (settings.accent) {
    const accent = parseColor(settings.accent);
    if (accent) {
      root.style.setProperty("--tg-accent", toHex(accent));
      root.style.setProperty("--tg-accent-hover", toHex(shade(accent, -0.12)));
      root.style.setProperty("--tg-accent-soft", withAlpha(accent, 0.18));
      root.style.setProperty("--tg-on-accent", readableTextOn(accent));
      root.style.setProperty("--tg-badge-bg", toHex(accent));
    }
  }

  if (settings.background) {
    const background = parseColor(settings.background);
    if (background) {
      root.style.setProperty("--tg-panel", toHex(background));
      root.style.setProperty("--tg-panel-alt", toHex(shade(background, -0.04)));
      root.style.setProperty("--tg-app-bg", toHex(shade(background, -0.08)));
    }
  }

  if (settings.text) {
    const text = parseColor(settings.text);
    if (text) {
      root.style.setProperty("--tg-text", toHex(text));
      root.style.setProperty("--tg-text-secondary", toHex(mix(text, { r: 128, g: 128, b: 128, a: 1 }, 0.45)));
      root.style.setProperty("--tg-text-muted", toHex(mix(text, { r: 128, g: 128, b: 128, a: 1 }, 0.6)));
      root.style.setProperty("--tg-bubble-in-text", toHex(text));
    }
  }
}

/** Black or white, whichever reads better on the given colour. */
function readableTextOn(color: Rgb): string {
  const luminance = (0.299 * color.r + 0.587 * color.g + 0.114 * color.b) / 255;
  return luminance > 0.6 ? "#000000" : "#ffffff";
}

/** Merge a patch into the settings, persist it and apply it. */
export function updateThemeSettings(patch: Partial<ThemeSettings>): void {
  const next = { ...get(themeSettings), ...patch };
  persist(next);
  applySettings(next);
  themeSettings.set(next);
}

/** Drop the Telegram theme and the manual overrides. */
export function resetThemeSettings(): void {
  updateThemeSettings({ ...DEFAULT_THEME_SETTINGS });
}

/** Follow the OS preference while the user has not chosen one explicitly. */
export function watchSystemTheme(): () => void {
  const media = window.matchMedia(DARK_QUERY);
  const listener = () => {
    const current = get(themeSettings);
    if (current.mode !== "system" || current.palette) {
      return;
    }

    applySettings(current);
  };

  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}

/** Apply the stored settings. Called once, before the first paint. */
export function initialiseTheme(): void {
  applySettings(loadSettings());
  themeSettings.set(loadSettings());
}
