use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::{info, warn};
use percent_encoding::percent_decode_str;

static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static CACHED_MEDIA_URLS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

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

/// Remove all cached media files from the media cache directory.
///
/// Kept for a "clear cache" action; the app does not call it on startup any
/// more, because that threw away every image the user had already seen.
pub fn clear_media_cache() {
    let cache_dir = media_cache_dir();
    if !cache_dir.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(&cache_dir) {
        warn!("Failed to clear media cache directory: {error}");
        return;
    }

    let _ = fs::create_dir_all(&cache_dir);
}

/// Maximum size of the media cache before the oldest files are dropped.
const MEDIA_CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Trim the media cache to `MEDIA_CACHE_MAX_BYTES`, dropping the oldest files
/// first.
///
/// Called on startup: the cache is disposable, so a full disk is worse than a
/// re-download, but a cache that is wiped on every launch makes the app feel
/// broken on a slow connection.
pub fn trim_media_cache() {
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

    if total_bytes <= MEDIA_CACHE_MAX_BYTES {
        return;
    }

    files.sort_by_key(|(modified, _, _)| *modified);

    let mut remaining = total_bytes;
    for (_, size, path) in files {
        if remaining <= MEDIA_CACHE_MAX_BYTES {
            break;
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

    Some(to_asset_storage_url(&final_path))
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

        return Some(PathBuf::from(path.as_ref()));
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
            let mut dir = std::env::temp_dir();
            dir.push("matrix-telegram");
            dir.push("media-cache");
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
        load_media_bytes_from_resolved_url, media_url_is_available, percent_encode_asset_path,
        to_asset_storage_url, NormalizedMediaLoad, NormalizedMediaLoadBuilder,
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
