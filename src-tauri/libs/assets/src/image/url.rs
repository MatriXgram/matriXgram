use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;

use super::cache_key::ImageCacheKeyParts;
use super::mime::{
    image_extension_from_mime, image_extension_from_raw_url, mime_type_from_extension,
};
use super::persistence::{
    cached_media_path_for_source_url, load_media_bytes_from_resolved_url, persist_normalized_media,
    register_cached_media_path, NormalizedMediaLoad,
};

pub async fn cache_mxc_media_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    cache_mxc_image(client, raw_url, false).await
}

/// Avatars are stored separately so clearing photos and videos leaves them intact.
pub async fn cache_avatar_mxc_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    cache_mxc_image(client, raw_url, true).await
}

async fn cache_mxc_image(
    client: &matrix_sdk::Client,
    raw_url: &str,
    avatar: bool,
) -> Option<String> {
    if !raw_url.starts_with("mxc://") {
        return None;
    }

    if let Some(cached_path) = cached_media_path_for_source_url(raw_url) {
        // Older versions saved SVG/AVIF/ICO images as application/octet-stream.
        // Repair those entries without clearing the user's cache or downloading again.
        if cached_path.ends_with(".bin") || (avatar && !cached_path.contains("avatar-")) {
            if let Some(bytes) = load_media_bytes_from_resolved_url(&cached_path) {
                if sniffed_image_extension(&bytes).is_some() {
                    return persist_mxc_image(raw_url, &bytes, avatar);
                }
            }
        }
        return Some(cached_path);
    }

    let mxc_uri = matrix_sdk::ruma::OwnedMxcUri::from(raw_url);
    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc_uri),
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch MXC image {raw_url}: {error}");
            return None;
        }
    };

    persist_mxc_image(raw_url, &bytes, avatar)
}

fn persist_mxc_image(raw_url: &str, bytes: &[u8], avatar: bool) -> Option<String> {
    let file_stem = format!(
        "{}{}",
        if avatar { "avatar-" } else { "media-" },
        mxc_image_cache_key(raw_url, bytes)
    );
    // Avatar mxc URLs have no file-name extension, so derive the type from the
    // bytes; a served `application/octet-stream` never renders in an `<img>`.
    let extension = sniffed_image_extension(bytes)
        .or_else(|| sniffed_extension_from_raw_url(raw_url))
        .unwrap_or("bin");
    let mime_type = mime_type_from_extension(extension);

    let request = NormalizedMediaLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(mime_type)
        .build()?;

    let resolved_path = persist_normalized_media(&request)?;
    register_cached_media_path(raw_url, &resolved_path);
    Some(resolved_path)
}

pub async fn resolve_pack_media_url(client: &matrix_sdk::Client, raw_url: &str) -> Option<String> {
    if raw_url.starts_with("mxc://") {
        return cache_mxc_media_to_local_path(client, raw_url).await;
    }

    if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
            return cache_mxc_media_to_local_path(client, &mxc_url).await;
        }

        warn!(
            "Ignoring non-Matrix HTTP media URL because image fetching is Matrix SDK-only: {}",
            raw_url
        );
        return None;
    }

    None
}

pub fn canonical_pack_source_url(raw_url: &str) -> String {
    if raw_url.starts_with("mxc://") {
        return raw_url.to_owned();
    }

    if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
        return mxc_url;
    }

    raw_url.to_owned()
}

pub fn cache_event_image(bytes: &[u8], key_parts: ImageCacheKeyParts) -> Option<String> {
    // Events may omit `content.info.mimetype`, which maps to the `bin`
    // fallback; sniff the actual bytes before giving up on a renderable type.
    let extension = match image_extension_from_mime(&key_parts.mime_type) {
        "bin" => sniffed_image_extension(bytes).unwrap_or("bin"),
        extension => extension,
    };
    let file_stem = format!("media-{}", image_cache_key(&key_parts));

    let request = NormalizedMediaLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(mime_type_from_extension(extension))
        .build()?;

    persist_normalized_media(&request)
}

fn mxc_image_cache_key(raw_url: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    raw_url.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn image_cache_key(parts: &ImageCacheKeyParts) -> String {
    let mut hasher = DefaultHasher::new();
    parts.event_id.hash(&mut hasher);
    parts.origin_server_ts.hash(&mut hasher);
    parts.room_id.hash(&mut hasher);
    parts.source_key.hash(&mut hasher);
    parts.mime_type.hash(&mut hasher);
    parts.bytes_len.hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
    let parsed = url::Url::parse(raw_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let segments: Vec<_> = parsed.path_segments()?.collect();
    let download_index = segments.windows(4).position(|window| {
        window.first() == Some(&"_matrix")
            && window.get(1) == Some(&"media")
            && window.get(3) == Some(&"download")
    })?;

    let server_name = segments.get(download_index + 4)?;
    let media_id = segments.get(download_index + 5)?;

    if server_name.is_empty() || media_id.is_empty() {
        return None;
    }

    Some(format!("mxc://{server_name}/{media_id}"))
}

#[cfg(test)]
mod tests {
    use super::{canonical_pack_source_url, resolve_pack_media_url};

    #[test]
    fn canonical_pack_source_url_converts_matrix_download_http_url() {
        let canonical = canonical_pack_source_url(
            "https://matrix.example.org/_matrix/media/v3/download/media.example.org/abc123",
        );
        assert_eq!(canonical, "mxc://media.example.org/abc123");
    }

    #[tokio::test]
    async fn resolve_pack_media_url_rejects_non_matrix_http_urls() {
        let homeserver_url = url::Url::parse("https://example.org")
            .expect("homeserver URL should parse for test setup");
        let client = matrix_sdk::Client::new(homeserver_url)
            .await
            .expect("client should construct for URL-only validation");

        let resolved = resolve_pack_media_url(&client, "https://example.org/image.png").await;
        assert!(resolved.is_none());
    }
}

/// Image file extension sniffed from magic bytes, for media whose URL carries
/// no usable extension (avatar mxc URLs never do).
fn sniffed_image_extension(bytes: &[u8]) -> Option<&'static str> {
    if is_svg(bytes) {
        return Some("svg");
    }
    let format = image::guess_format(bytes).ok()?;
    let extension = match format {
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::Png => "png",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Bmp => "bmp",
        image::ImageFormat::Avif => "avif",
        image::ImageFormat::Ico => "ico",
        _ => return None,
    };
    Some(extension)
}

/// SVG has no binary magic number. Only accept an SVG root, optionally preceded
/// by a UTF-8 BOM, XML declarations, comments or a doctype; not an embedded tag
/// inside an HTML/error response. The browser still loads it as an inert <img>.
fn is_svg(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut text = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        let end = if text.starts_with("<?") {
            text.find("?>").map(|index| index + 2)
        } else if text.starts_with("<!--") {
            text.find("-->").map(|index| index + 3)
        } else if text.starts_with("<!DOCTYPE") {
            text.find('>').map(|index| index + 1)
        } else {
            return text.strip_prefix("<svg").is_some_and(|rest| {
                rest.starts_with(|ch: char| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
            });
        };
        let Some(end) = end else { return false };
        text = text[end..].trim_start();
    }
}

fn sniffed_extension_from_raw_url(raw_url: &str) -> Option<&'static str> {
    match image_extension_from_raw_url(raw_url) {
        "bin" => None,
        extension => Some(extension),
    }
}

#[cfg(test)]
mod sniff_tests {
    use super::{
        cache_mxc_media_to_local_path, image_extension_from_raw_url, mxc_image_cache_key,
        sniffed_extension_from_raw_url, sniffed_image_extension,
    };
    use crate::image::persistence::{
        load_media_bytes_from_resolved_url, persist_normalized_media, register_cached_media_path,
        NormalizedMediaLoad,
    };

    fn png_bytes() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3]
    }

    fn jpeg_bytes() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]
    }

    #[test]
    fn sniffed_image_extension_detects_magic_bytes() {
        assert_eq!(sniffed_image_extension(&png_bytes()), Some("png"));
        assert_eq!(sniffed_image_extension(&jpeg_bytes()), Some("jpg"));
        assert_eq!(sniffed_image_extension(b"plain text"), None);
        assert_eq!(sniffed_image_extension(&[]), None);
    }

    #[test]
    fn detects_custom_emoji_formats_without_file_extensions() {
        for svg in [
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            "\u{feff}  <?xml version=\"1.0\"?>\n<!-- emoji -->\n<svg />",
            "<?xml version=\"1.0\"?><!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\"><svg></svg>",
        ] {
            assert_eq!(sniffed_image_extension(svg.as_bytes()), Some("svg"));
        }
        assert_eq!(
            sniffed_image_extension(b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00mif1avifmiaf"),
            Some("avif")
        );
        assert_eq!(sniffed_image_extension(&[0, 0, 1, 0, 1, 0]), Some("ico"));
        for invalid in [
            "<html><svg /></html>",
            "<svgfake />",
            "<!-- <svg />",
            "<?xml",
        ] {
            assert_eq!(sniffed_image_extension(invalid.as_bytes()), None);
        }
    }

    #[tokio::test]
    async fn repairs_legacy_binary_svg_cache_without_network() {
        let source = "mxc://example.org/legacy-svg-cache-test";
        let bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        let legacy = persist_normalized_media(
            &NormalizedMediaLoad::builder()
                .bytes(bytes)
                .file_stem(mxc_image_cache_key(source, bytes))
                .extension("bin")
                .mime_type("application/octet-stream")
                .build()
                .unwrap(),
        )
        .unwrap();
        register_cached_media_path(source, &legacy);
        let client = matrix_sdk::Client::new(url::Url::parse("https://example.org").unwrap())
            .await
            .unwrap();
        let repaired = cache_mxc_media_to_local_path(&client, source)
            .await
            .unwrap();
        assert!(repaired.ends_with(".svg"));
        assert_eq!(
            load_media_bytes_from_resolved_url(&repaired).as_deref(),
            Some(bytes.as_slice())
        );
    }

    #[test]
    fn sniffed_extension_from_raw_url_rejects_bin() {
        assert_eq!(sniffed_extension_from_raw_url("mxc://server/AbC123"), None);
        assert_eq!(
            sniffed_extension_from_raw_url("mxc://server/photo.png"),
            Some("png")
        );
        assert_eq!(
            image_extension_from_raw_url("mxc://server/photo.png"),
            "png"
        );
    }
}
