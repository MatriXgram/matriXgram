//! Telegram Desktop theme files (`.tdesktop-theme`).
//!
//! The file is a ZIP archive with two entries:
//!
//! - `colors.tdesktop-theme`: a plain-text list of `key: #rrggbbaa;` lines,
//!   using Telegram Desktop's own colour names (`dialogsBg`, `msgInBg`, ...).
//! - `background.jpg`: the wallpaper, optional.
//!
//! Only the colours are interpreted here. The names are kept as they are so
//! the frontend can map them onto the app's own CSS variables, and so a theme
//! that sets a key the app does not know about is still usable.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use log::info;

/// A parsed Telegram Desktop theme.
#[derive(Clone, Debug, Default)]
pub struct DesktopTheme {
    /// Telegram colour name -> `#rrggbbaa`.
    pub colors: BTreeMap<String, String>,
    /// Path of the extracted wallpaper, when the theme ships one.
    pub background_path: Option<String>,
}

const COLORS_ENTRY: &str = "colors.tdesktop-theme";
const BACKGROUND_ENTRY: &str = "background.jpg";

/// File name of the extracted wallpaper inside the media cache.
const WALLPAPER_FILE_NAME: &str = "desktop-theme-wallpaper.jpg";

/// Load a `.tdesktop-theme` file.
///
/// The wallpaper is written next to the media cache so the frontend can load
/// it through Tauri's asset protocol.
pub fn load_desktop_theme(theme_path: &Path) -> Result<DesktopTheme, String> {
    let file =
        File::open(theme_path).map_err(|error| format!("Failed to open theme file: {error}"))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("Failed to read theme archive: {error}"))?;

    let colors = read_colors(&mut archive)?;
    let background_path = extract_background(&mut archive)?;

    Ok(DesktopTheme {
        colors,
        background_path,
    })
}

fn read_colors<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<BTreeMap<String, String>, String> {
    let mut contents = String::new();
    {
        let mut entry = archive
            .by_name(COLORS_ENTRY)
            .map_err(|error| format!("Theme archive has no {COLORS_ENTRY}: {error}"))?;
        entry
            .read_to_string(&mut contents)
            .map_err(|error| format!("Failed to read theme colours: {error}"))?;
    }

    Ok(parse_colors(&contents))
}

/// Parse the `key: #rrggbbaa;` colour list.
fn parse_colors(contents: &str) -> BTreeMap<String, String> {
    let mut colors = BTreeMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().trim_end_matches(';').trim();

        if key.is_empty() || !value.starts_with('#') {
            continue;
        }

        // A colour is `#rgb`, `#rrggbb` or `#rrggbbaa`. Telegram writes the
        // eight digit form; the shorter ones are accepted for hand-written
        // themes.
        let digits = &value[1..];
        let is_valid = matches!(digits.len(), 3 | 6 | 8)
            && digits
                .chars()
                .all(|character| character.is_ascii_hexdigit());

        if !is_valid {
            continue;
        }

        colors.insert(key.to_string(), value.to_ascii_lowercase());
    }

    colors
}

fn extract_background<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Option<String>, String> {
    let mut entry = match archive.by_name(BACKGROUND_ENTRY) {
        Ok(entry) => entry,
        // A theme without a wallpaper is valid.
        Err(_) => return Ok(None),
    };

    let target = wallpaper_path()?;
    let mut output = File::create(&target)
        .map_err(|error| format!("Failed to create theme wallpaper file: {error}"))?;

    std::io::copy(&mut entry, &mut output)
        .map_err(|error| format!("Failed to write theme wallpaper: {error}"))?;

    info!("Extracted theme wallpaper to {:?}", target);

    Ok(Some(target.to_string_lossy().to_string()))
}

/// Where the extracted wallpaper is written.
fn wallpaper_path() -> Result<PathBuf, String> {
    let mut path = crate::image::media_cache_dir_path();
    std::fs::create_dir_all(&path)
        .map_err(|error| format!("Failed to create media cache directory: {error}"))?;
    path.push(WALLPAPER_FILE_NAME);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::{load_desktop_theme, parse_colors};

    /// Build a theme archive in memory and write it to a temporary file.
    fn write_theme_archive(contents: &str) -> std::path::PathBuf {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file("colors.tdesktop-theme", options)
                .expect("start colour entry");
            writer
                .write_all(contents.as_bytes())
                .expect("write colour entry");
            writer.finish().expect("finish archive");
        }

        let path = std::env::temp_dir().join("matrix-telegram-test-theme.tdesktop-theme");
        std::fs::write(&path, buffer.into_inner()).expect("write theme file");

        path
    }

    #[test]
    fn loads_a_theme_archive() {
        let path = write_theme_archive("dialogsBg: #000000ff;\nmsgInBg: #1c183dcc;\n");

        let theme = load_desktop_theme(&path).expect("theme loads");

        assert_eq!(theme.colors.len(), 2);
        assert_eq!(
            theme.colors.get("dialogsBg").map(String::as_str),
            Some("#000000ff")
        );
        assert_eq!(
            theme.colors.get("msgInBg").map(String::as_str),
            Some("#1c183dcc")
        );
        // No wallpaper in the archive.
        assert!(theme.background_path.is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reports_a_missing_colour_entry() {
        let path = std::env::temp_dir().join("matrix-telegram-test-theme-broken.tdesktop-theme");
        std::fs::write(&path, b"not a zip file").expect("write theme file");

        assert!(load_desktop_theme(&path).is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parses_colour_entries() {
        let contents = "// a comment\ndialogsBg: #000000ff;\nmsgInBg: #1c183dcc;\n";
        let colors = parse_colors(contents);

        assert_eq!(
            colors.get("dialogsBg").map(String::as_str),
            Some("#000000ff")
        );
        assert_eq!(colors.get("msgInBg").map(String::as_str), Some("#1c183dcc"));
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn ignores_malformed_entries() {
        let contents = "broken\nname: not-a-colour;\nok: #fff;\nname2: #12345;\n";
        let colors = parse_colors(contents);

        assert_eq!(colors.len(), 1);
        assert_eq!(colors.get("ok").map(String::as_str), Some("#fff"));
    }

    #[test]
    fn accepts_the_short_colour_forms() {
        let colors = parse_colors("a: #fff;\nb: #ffffff;\nc: #ffffffff;\n");

        assert_eq!(colors.len(), 3);
    }
}
