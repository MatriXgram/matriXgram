use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use log::{info, warn};
use percent_encoding::percent_decode_str;

static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static CACHED_MEDIA_URLS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
const DEFAULT_MEDIA_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
static MEDIA_CACHE_MAX_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_MEDIA_CACHE_MAX_BYTES);

/// A normalized, disk-ready media payload. Carries the raw bytes plus the
/// metadata needed to derive a stable file name and extension. Shared by the
/// image, video, and file cache paths.
#[derive(Clone, Debug)]
pub struct NormalizedMediaLoad {
    pub bytes: Vec<u8>,
    pub file_stem: String,
    pub extension: String,
    pub mime_type: String,
}

impl NormalizedMediaLoad {
    pub fn builder() -> NormalizedMediaLoadBuilder {
        NormalizedMediaLoadBuilder::default()
    }
}

#[derive(Default)]
pub struct NormalizedMediaLoadBuilder {
    bytes: Option<Vec<u8>>,
    file_stem: Option<String>,
    extension: Option<String>,
    mime_type: Option<String>,
}

impl NormalizedMediaLoadBuilder {
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        self.bytes = Some(bytes.to_vec());
        self
    }

    pub fn file_stem<T>(mut self, file_stem: T) -> Self
    where
        T: Into<String>,
    {
        self.file_stem = Some(file_stem.into());
        self
    }

    pub fn extension<T>(mut self, extension: T) -> Self
    where
        T: Into<String>,
    {
        self.extension = Some(extension.into());
        self
    }

    pub fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn build(self) -> Option<NormalizedMediaLoad> {
        Some(NormalizedMediaLoad {
            bytes: self.bytes?,
            file_stem: self.file_stem?,
            extension: self.extension?,
            mime_type: self.mime_type?,
        })
    }
}

pub fn initialize_media_cache_dir(cache_dir: &Path) {
    if MEDIA_CACHE_DIR.get().is_some() {
        return;
    }

    let mut resolved = cache_dir.to_path_buf();
    resolved.push("media-cache");
    let _ = MEDIA_CACHE_DIR.set(resolved);
}

/// Remove media while preserving avatar files and legacy images whose purpose
/// cannot be distinguished from avatars. The normal size limit still trims
/// both kinds of files. In-progress atomic writes are never removed here.
pub fn clear_media_cache() -> Result<u64, String> {
    let cache_dir = media_cache_dir();
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Failed to recreate media cache directory: {error}"))?;
    let bytes_cleared = clear_non_avatar_files(&cache_dir)?;
    if let Ok(mut urls) = cached_media_urls().lock() {
        urls.retain(|_, path| media_url_is_available(path));
    }
    Ok(bytes_cleared)
}

fn clear_non_avatar_files(cache_dir: &Path) -> Result<u64, String> {
    let entries = fs::read_dir(cache_dir)
        .map_err(|error| format!("Failed to read media cache directory: {error}"))?;
    let mut bytes_cleared = 0_u64;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read cached media entry: {error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("avatar-") || name.starts_with("img-") || name.ends_with(".tmp") {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to inspect cached media: {error}"))?;
        if !metadata.is_file() {
            continue;
        }
        fs::remove_file(entry.path())
            .map_err(|error| format!("Failed to remove cached media: {error}"))?;
        bytes_cleared = bytes_cleared.saturating_add(metadata.len());
    }
    Ok(bytes_cleared)
}

pub fn media_cache_max_bytes() -> u64 {
    MEDIA_CACHE_MAX_BYTES.load(Ordering::Relaxed)
}

pub fn set_media_cache_max_bytes(bytes: u64) {
    MEDIA_CACHE_MAX_BYTES.store(bytes, Ordering::Relaxed);
    trim_media_cache();
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaCacheUsage {
    pub image_bytes: u64,
    pub video_bytes: u64,
    pub other_bytes: u64,
}

impl MediaCacheUsage {
    pub fn total_bytes(self) -> u64 {
        self.image_bytes
            .saturating_add(self.video_bytes)
            .saturating_add(self.other_bytes)
    }
}

pub fn media_cache_usage() -> MediaCacheUsage {
    let Ok(entries) = fs::read_dir(media_cache_dir()) else {
        return MediaCacheUsage::default();
    };

    let mut usage = MediaCacheUsage::default();
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let extension = entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bytes = metadata.len();
        match extension.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "avif" | "heic" | "tif" | "tiff"
            | "svg" => {
                usage.image_bytes = usage.image_bytes.saturating_add(bytes);
            }
            "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "ogv" | "3gp" | "mpeg" | "mpg" => {
                usage.video_bytes = usage.video_bytes.saturating_add(bytes);
            }
            _ => usage.other_bytes = usage.other_bytes.saturating_add(bytes),
        }
    }
    usage
}

pub fn media_cache_size_bytes() -> u64 {
    media_cache_usage().total_bytes()
}

/// Trim the media cache to the configured maximum, dropping the oldest files
/// first.
///
/// Called on startup: the cache is disposable, so a full disk is worse than a
/// re-download, but a cache that is wiped on every launch makes the app feel
/// broken on a slow connection.
pub fn trim_media_cache() {
    trim_media_cache_preserving(None);
}

fn trim_media_cache_preserving(preserve: Option<&Path>) {
    let cache_dir = media_cache_dir();
    if !cache_dir.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(&cache_dir) else {
        return;
    };

    let mut files = Vec::new();
    let mut total_bytes = 0_u64;

    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if !metadata.is_file() {
            continue;
        }

        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        total_bytes = total_bytes.saturating_add(metadata.len());
        files.push((modified, metadata.len(), entry.path()));
    }

    let max_bytes = media_cache_max_bytes();
    if total_bytes <= max_bytes {
        return;
    }

    files.sort_by_key(|(modified, _, _)| *modified);

    let mut remaining = total_bytes;
    for (_, size, path) in files {
        if remaining <= max_bytes {
            break;
        }

        if preserve.is_some_and(|preserve_path| preserve_path == path.as_path()) {
            continue;
        }

        if let Err(error) = fs::remove_file(&path) {
            warn!("Failed to remove cached media file: {error}");
            continue;
        }

        remaining = remaining.saturating_sub(size);
    }

    info!(
        "Trimmed media cache from {} to {} bytes",
        total_bytes, remaining
    );
}

/// Persist a normalized media payload to the media cache directory and return
/// the resolved `asset://` URL. Writes are atomic (temp file + rename) and
/// idempotent (existing files are returned as-is).
pub fn persist_normalized_media(request: &NormalizedMediaLoad) -> Option<String> {
    let cache_dir = media_cache_dir();
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        warn!("Failed to initialize media cache directory: {error}");
        return None;
    }

    let file_name = format!("{}.{}", request.file_stem, request.extension);
    let final_path = cache_dir.join(file_name);

    if final_path.exists() {
        return Some(to_asset_storage_url(&final_path));
    }

    let temp_path = cache_dir.join(format!("{}.tmp", request.file_stem));
    if let Err(error) = fs::write(&temp_path, &request.bytes) {
        warn!("Failed to write cached media file: {error}");
        return None;
    }

    if let Err(error) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        if final_path.exists() {
            return Some(to_asset_storage_url(&final_path));
        }
        warn!("Failed to finalize cached media file: {error}");
        return None;
    }

    let asset_url = to_asset_storage_url(&final_path);
    trim_media_cache_preserving(Some(&final_path));
    Some(asset_url)
}

pub fn to_asset_storage_url(path: &Path) -> String {
    let absolute = path.to_string_lossy();
    let encoded = percent_encode_asset_path(absolute.as_ref());
    format!("asset://localhost/{encoded}")
}

fn percent_encode_asset_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');

        if is_unreserved {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }

    encoded
}

/// Load the raw bytes for a resolved media URL (`asset://`, `file://`, or an
/// absolute path) from disk.
pub fn load_media_bytes_from_resolved_url(raw_url: &str) -> Option<Vec<u8>> {
    let file_path = resolved_media_file_path(raw_url)?;
    fs::read(file_path).ok()
}

/// Whether a resolved media URL (`asset://`, `file://`, or absolute path) still
/// points to an existing file on disk. Used to detect stale cached media URLs
/// so the caller can re-fetch from the server.
pub fn media_url_is_available(raw_url: &str) -> bool {
    resolved_media_file_path(raw_url).is_some_and(|path| path.exists())
}

/// Resolve a media URL (`asset://`, `file://`, or absolute path) to its
/// filesystem path. Returns `None` for empty or unrecognized URLs.
pub fn resolved_media_file_path(raw_url: &str) -> Option<PathBuf> {
    if raw_url.starts_with("asset://") {
        let parsed = url::Url::parse(raw_url).ok()?;
        let path = percent_decode_str(parsed.path()).decode_utf8().ok()?;
        if path.is_empty() || path == "/" {
            return None;
        }

        return Some(asset_url_path_to_file_path(&path));
    }

    if raw_url.starts_with("file://") {
        let parsed = url::Url::parse(raw_url).ok()?;
        return parsed.to_file_path().ok();
    }

    if raw_url.starts_with('/') {
        return Some(PathBuf::from(raw_url));
    }

    None
}

/// Turn the decoded path of an `asset://` URL back into a filesystem path.
///
/// A URL path always starts with a slash, but on Windows that slash is not
/// part of the drive letter: the URL of `C:\cache\img.png` decodes to
/// `/C:\cache\img.png`, and the extra slash would turn it into a path
/// relative to the current drive. The separators are normalised for the
/// platform as well, because the encoder leaves the path separators of the
/// source platform untouched.
fn asset_url_path_to_file_path(decoded_path: &str) -> PathBuf {
    let trimmed = decoded_path.strip_prefix('/').unwrap_or(decoded_path);

    let bytes = trimmed.as_bytes();
    let is_windows_drive_path =
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';

    if is_windows_drive_path || cfg!(windows) {
        return PathBuf::from(trimmed.replace('/', "\\"));
    }

    PathBuf::from(trimmed)
}

pub fn cached_media_path_for_source_url(source_url: &str) -> Option<String> {
    let cached_path = {
        let cache = cached_media_urls();
        let lock = cache.lock().ok()?;
        lock.get(source_url).cloned()
    }?;

    if resolved_media_file_path(&cached_path).is_some_and(|path| path.exists()) {
        return Some(cached_path);
    }

    if let Ok(mut cache) = cached_media_urls().lock() {
        cache.remove(source_url);
    }

    None
}

pub fn register_cached_media_path(source_url: &str, resolved_path: &str) {
    if let Ok(mut cache) = cached_media_urls().lock() {
        cache.insert(source_url.to_owned(), resolved_path.to_owned());
    }
}

fn cached_media_urls() -> &'static Mutex<HashMap<String, String>> {
    CACHED_MEDIA_URLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn media_cache_dir() -> PathBuf {
    MEDIA_CACHE_DIR
        .get_or_init(|| {
            let root = std::env::temp_dir();
            let dir = root.join("matrixgram").join("media-cache");
            let legacy = root.join("matrix-telegram").join("media-cache");

            if !dir.exists() && legacy.exists() {
                if let Some(parent) = dir.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::rename(&legacy, &dir).is_err() {
                    return legacy;
                }
            }

            dir
        })
        .clone()
}

pub fn media_cache_dir_path() -> PathBuf {
    media_cache_dir()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        clear_non_avatar_files, load_media_bytes_from_resolved_url, media_url_is_available,
        percent_encode_asset_path, to_asset_storage_url, NormalizedMediaLoad,
        NormalizedMediaLoadBuilder,
    };

    #[test]
    fn normalized_media_load_builder_requires_fields() {
        let missing = NormalizedMediaLoadBuilder::default().file_stem("x").build();
        assert!(missing.is_none());

        let complete = NormalizedMediaLoad::builder()
            .bytes(&[1, 2, 3])
            .file_stem("img-1")
            .extension("png")
            .mime_type("image/png")
            .build();
        assert!(complete.is_some());
    }

    #[test]
    fn clear_media_preserves_avatars_legacy_images_and_inflight_writes() {
        let dir =
            std::env::temp_dir().join(format!("matrixgram-clear-media-{}", rand::random::<u64>()));
        fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in [
            ("avatar-img-a.png", &[1, 2][..]),
            ("img-old.png", &[3][..]),
            ("media-img-b.png", &[4, 5, 6][..]),
            ("video-x.mp4", &[7, 8][..]),
            ("media-inflight.tmp", &[9][..]),
        ] {
            fs::write(dir.join(name), bytes).unwrap();
        }
        assert_eq!(clear_non_avatar_files(&dir).unwrap(), 5);
        assert!(dir.join("avatar-img-a.png").exists());
        assert!(dir.join("img-old.png").exists());
        assert!(dir.join("media-inflight.tmp").exists());
        assert!(!dir.join("media-img-b.png").exists());
        assert!(!dir.join("video-x.mp4").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn percent_encodes_absolute_asset_path() {
        let encoded = percent_encode_asset_path("/home/lux/.cache/media-cache/img-123.bin");
        assert_eq!(
            encoded,
            "%2Fhome%2Flux%2F.cache%2Fmedia-cache%2Fimg-123.bin"
        );
    }

    #[test]
    fn asset_url_round_trips_to_original_path() {
        let dir = std::env::temp_dir().join("singularity-test-assets");
        fs::create_dir_all(&dir).expect("create temp media dir");
        let file = dir.join("img-123.png");
        fs::write(&file, [1, 2, 3]).expect("write temp media file");

        let url = to_asset_storage_url(&file);
        assert!(url.starts_with("asset://localhost/"));

        assert!(media_url_is_available(&url));
        assert_eq!(
            load_media_bytes_from_resolved_url(&url),
            Some(vec![1, 2, 3]),
        );

        fs::remove_file(&file).expect("clean up temp media file");
        fs::remove_dir(&dir).expect("clean up temp media dir");
    }
}
