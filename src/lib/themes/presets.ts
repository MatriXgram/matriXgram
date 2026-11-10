/**
 * Built-in Telegram Desktop themes.
 *
 * `Warp` is the theme by @ThemesM8, reduced to the colours the app uses. The
 * full palette can be imported at any time from a `.tdesktop-theme` file.
 */

export interface ThemePreset {
  id: string;
  name: string;
  description: string;
  colors: Record<string, string>;
  /** Wallpaper shipped with the theme. */
  wallpaper?: string;
}

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: "warp",
    name: "Warp",
    description: "Deep violet bubbles on black, by @ThemesM8",
    wallpaper: "/warp-wallpaper.jpg",
    colors: {
      dialogsBg: "#000000ff",
      dialogsTextFg: "#cccafcaf",
      dialogsChatIconFg: "#cccafcff",
      dialogsBgActive: "#121224ff",
      dialogsUnreadBg: "#c839c8ff",
      dialogsUnreadFg: "#ffffffff",
      dialogsUnreadBgMuted: "#282258ff",
      sideBarTextFg: "#cccafcff",
      sideBarIconFg: "#cccafcff",
      sideBarTextFgActive: "#ff49ffff",
      sideBarIconFgActive: "#ff49ffff",
      sideBarBadgeBg: "#ff49ffff",
      sideBarBadgeFg: "#000000",
      windowFg: "#cccafcff",
      windowBoldFg: "#cccafcff",
      historyComposeAreaBg: "#000000ff",
      historyTextInFg: "#dbd9fcff",
      historyTextOutFg: "#dbd9fcff",
      historyLinkInFg: "#ff49ffff",
      historyLinkOutFg: "#ff49ffff",
      historySendIconFg: "#ff49ffff",
      historySendIconFgOver: "#ff91ffff",
      historyPinnedBg: "#14122cff",
      historyUnreadBarBg: "#00000096",
      historyReplyBg: "#000000ff",
      historyReplyIconFg: "#ff49ffff",
      historyReplyCancelFg: "#ff49ffff",
      historyFileThumbIconFg: "#ff49ffff",
      msgInBg: "#1c183dcc",
      msgOutBg: "#320046c8",
      msgServiceBg: "#0f0c20b4",
      msgServiceFg: "#cccafcff",
      msgInDateFg: "#cccafc96",
      msgOutDateFg: "#cccafc96",
      msgInReplyBarColor: "#ff49ffff",
      msgOutReplyBarColor: "#ff49ffff",
      msgDateImgBg: "#000000c8",
      placeholderFg: "#cccafc96",
      scrollBg: "#00000021",
      scrollBgOver: "#0000006b",
      toastBg: "#cccafcb2",
      activeLineFg: "#ff49ffff",
      lightButtonBg: "#000000ff",
      lightButtonFg: "#ff49ffff",
      activeButtonBg: "#ff49ffff",
      activeButtonFg: "#ffffff",
      menuBg: "#0f0d21ff",
      boxTitleCloseFg: "#ff49ffff",
      photoEditorBg: "#000000aa",
      inputBorderFg: "#cccafcff",
    },
  },
];

export function findPreset(id: string): ThemePreset | undefined {
  return THEME_PRESETS.find((preset) => preset.id === id);
}
