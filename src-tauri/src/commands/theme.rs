//! Theme command adapters: loading a Telegram Desktop theme file.
//!
//! Delegates to the `assets` crate, which knows the `.tdesktop-theme` format.

use std::path::PathBuf;

use types::theme::{MatrixDesktopTheme, MatrixLoadDesktopThemeRequest};

#[tauri::command]
pub async fn matrix_load_desktop_theme(
    request: MatrixLoadDesktopThemeRequest,
) -> Result<MatrixDesktopTheme, String> {
    let path = PathBuf::from(request.path);

    let theme = assets::load_desktop_theme(&path)?;

    Ok(MatrixDesktopTheme {
        colors: theme.colors,
        background_path: theme.background_path,
    })
}
