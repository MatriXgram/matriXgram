use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A Telegram Desktop theme, as loaded from a `.tdesktop-theme` file.
///
/// The colour names are Telegram's own (`dialogsBg`, `msgInBg`, ...), so a
/// theme can carry keys the app does not use yet without losing anything.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDesktopTheme {
    /// Telegram colour name -> `#rrggbbaa`.
    pub colors: BTreeMap<String, String>,
    /// Path of the extracted wallpaper, when the theme ships one.
    #[serde(default)]
    pub background_path: Option<String>,
}

/// Request to load a `.tdesktop-theme` file from disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixLoadDesktopThemeRequest {
    /// Absolute path of the theme file.
    pub path: String,
}
