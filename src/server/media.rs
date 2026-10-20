//! Uploaded pictures, soundtracks and videos: validation, processing and serving.

use std::{
    io::{Cursor, SeekFrom},
    path::PathBuf,
};

use anyhow::Context;
use dioxus::fullstack::{
    axum::{
        extract::Path,
        http::{HeaderMap, StatusCode, header},
        response::{IntoResponse, Response},
    },
    body::Body,
};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, imageops::FilterType};

use crate::protocol::UploadLimits;

/// Upload limits from `QUAESITO_MAX_PICTURE_MB`, `QUAESITO_MAX_AUDIO_MB`,
/// `QUAESITO_MAX_VIDEO_MB` and `QUAESITO_MAX_IMPORT_MB`, with defaults of 15, 40, 200 and 500 MB.
#[must_use]
pub fn limits_from_env() -> UploadLimits {
    let megabytes = |var: &str, default: u64| {
        let mb = std::env::var(var)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&mb| mb > 0)
            .unwrap_or(default);
        mb.saturating_mul(1024 * 1024)
    };
    UploadLimits {
        picture: megabytes("QUAESITO_MAX_PICTURE_MB", 15),
        audio: megabytes("QUAESITO_MAX_AUDIO_MB", 40),
        video: megabytes("QUAESITO_MAX_VIDEO_MB", 200),
        import: megabytes("QUAESITO_MAX_IMPORT_MB", 500),
    }
}

/// Largest slice of a file sent for an open-ended range request (videos stream in slices).
const MAX_SLICE_BYTES: u64 = 8 * 1024 * 1024;

const QUESTION_MAX_WIDTH: u32 = 1600;
const QUESTION_MAX_HEIGHT: u32 = 1200;
const AVATAR_SIZE: u32 = 256;
const JPEG_QUALITY: u8 = 85;

/// What a picture is used for, which decides how it is processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    /// Shown on the big screen: downscaled to fit, stored as JPEG.
    Question,
    /// A team avatar: cropped square and stored as a small PNG.
    Avatar,
}

/// Directory of processed pictures and soundtracks.
pub struct MediaStore {
    dir: PathBuf,
}

impl MediaStore {
    /// Opens (creating if needed) the store rooted at `dir`.
    ///
    /// # Errors
    /// Fails if the directory cannot be created.
    pub async fn open(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into();
        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self { dir })
    }

    /// Processes an uploaded picture and stores it, returning its file name.
    ///
    /// # Errors
    /// Fails if the data is not a supported image or cannot be written.
    pub async fn store(&self, bytes: Vec<u8>, kind: ImageKind) -> anyhow::Result<String> {
        let (data, ext) = tokio::task::spawn_blocking(move || process(&bytes, kind)).await??;
        let name = format!("{}.{ext}", uuid::Uuid::new_v4().simple());
        tokio::fs::write(self.dir.join(&name), data).await?;
        Ok(name)
    }

    /// Stores an uploaded soundtrack as-is, returning its file name.
    ///
    /// # Errors
    /// Fails if the data is not a supported audio format or cannot be written.
    pub async fn store_audio(&self, bytes: Vec<u8>) -> anyhow::Result<String> {
        let ext = audio_extension(&bytes)
            .context("Unsupported soundtrack. Use MP3, M4A/AAC, Ogg, Opus, WebM, FLAC or WAV.")?;
        let name = format!("{}.{ext}", uuid::Uuid::new_v4().simple());
        tokio::fs::write(self.dir.join(&name), bytes).await?;
        Ok(name)
    }

    /// Stores an uploaded video as-is, returning its file name.
    ///
    /// # Errors
    /// Fails if the data is not a supported video format or cannot be written.
    pub async fn store_video(&self, bytes: Vec<u8>) -> anyhow::Result<String> {
        let ext =
            video_extension(&bytes).context("Unsupported video. Use MP4 (H.264), WebM or MOV.")?;
        let name = format!("{}.{ext}", uuid::Uuid::new_v4().simple());
        tokio::fs::write(self.dir.join(&name), bytes).await?;
        Ok(name)
    }

    /// A stored file's bytes, or `None` for unknown or invalid names.
    pub async fn read_bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.read(name).await.map(|(data, _)| data)
    }

    /// Deletes stored files that are not in `used` and were last written more than `grace`
    /// ago (so uploads not saved into a quiz yet survive). Returns how many were deleted.
    ///
    /// # Errors
    /// Fails if the directory cannot be listed.
    pub async fn collect_garbage(
        &self,
        used: &std::collections::HashSet<String>,
        grace: std::time::Duration,
    ) -> anyhow::Result<usize> {
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        let mut deleted = 0;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if content_type(&name).is_none() || used.contains(&name) {
                continue;
            }
            let old_enough = entry
                .metadata()
                .await
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > grace);
            if old_enough && tokio::fs::remove_file(entry.path()).await.is_ok() {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Reads a stored file by name.
    async fn read(&self, name: &str) -> Option<(Vec<u8>, &'static str)> {
        let content_type = content_type(name)?;
        let data = tokio::fs::read(self.dir.join(name)).await.ok()?;
        Some((data, content_type))
    }

    /// Opens a stored file by name, with its length and content type.
    async fn open_file(&self, name: &str) -> Option<(tokio::fs::File, u64, &'static str)> {
        let content_type = content_type(name)?;
        let file = tokio::fs::File::open(self.dir.join(name)).await.ok()?;
        let len = file.metadata().await.ok()?.len();
        Some((file, len, content_type))
    }
}

/// Decodes, resizes and re-encodes a picture. Returns the bytes and file extension.
///
/// # Errors
/// Fails if the data is not a supported image or is unreasonably large.
pub fn process(bytes: &[u8], kind: ImageKind) -> anyhow::Result<(Vec<u8>, &'static str)> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(10_000);
    limits.max_image_height = Some(10_000);
    reader.limits(limits);
    let image = reader.decode().context("unsupported or corrupt image")?;

    let mut out = Cursor::new(Vec::new());
    match kind {
        ImageKind::Question => {
            let image =
                if image.width() > QUESTION_MAX_WIDTH || image.height() > QUESTION_MAX_HEIGHT {
                    image.resize(
                        QUESTION_MAX_WIDTH,
                        QUESTION_MAX_HEIGHT,
                        FilterType::Lanczos3,
                    )
                } else {
                    image
                };
            let rgb = DynamicImage::ImageRgb8(image.to_rgb8());
            let encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
            rgb.write_with_encoder(encoder)?;
            Ok((out.into_inner(), "jpg"))
        }
        ImageKind::Avatar => {
            let image = image.resize_to_fill(AVATAR_SIZE, AVATAR_SIZE, FilterType::Lanczos3);
            DynamicImage::ImageRgba8(image.to_rgba8()).write_to(&mut out, ImageFormat::Png)?;
            Ok((out.into_inner(), "png"))
        }
    }
}

/// Recognises a video file by its first bytes and returns the extension to store it under.
#[must_use]
pub fn video_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        // ISO media: the brand tells QuickTime and audio-only files from MP4 video.
        [_, _, _, _, b'f', b't', b'y', b'p', brand @ ..] if brand.len() >= 4 => match &brand[..4] {
            b"qt  " => Some("mov"),
            b"M4A " | b"M4B " | b"M4P " => None,
            _ => Some("mp4"),
        },
        [0x1A, 0x45, 0xDF, 0xA3, ..] => Some("webm"),
        _ => None,
    }
}

/// Recognises an audio file by its first bytes and returns the extension to store it under.
#[must_use]
pub fn audio_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [b'I', b'D', b'3', ..] => Some("mp3"),
        [0xFF, second, ..] if second & 0xE0 == 0xE0 && second & 0x06 != 0 => Some("mp3"),
        [0xFF, second, ..] if second & 0xF6 == 0xF0 => Some("aac"),
        [b'O', b'g', b'g', b'S', ..] => Some("ogg"),
        [b'f', b'L', b'a', b'C', ..] => Some("flac"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'A',
            b'V',
            b'E',
            ..,
        ] => Some("wav"),
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => Some("m4a"),
        [0x1A, 0x45, 0xDF, 0xA3, ..] => Some("webm"),
        _ => None,
    }
}

/// Reads an upload (a file sent to a server function, or a request body) into memory, refusing
/// more than `max_bytes`.
///
/// # Errors
/// Fails if the upload is too large or the stream breaks.
pub async fn read_upload<E: std::fmt::Display>(
    mut file: impl futures_util::Stream<Item = Result<dioxus::fullstack::axum::body::Bytes, E>> + Unpin,
    max_bytes: u64,
) -> anyhow::Result<Vec<u8>> {
    use futures_util::StreamExt;

    let mut bytes = Vec::new();
    while let Some(chunk) = file.next().await {
        let chunk = chunk.map_err(|err| anyhow::anyhow!("upload interrupted: {err}"))?;
        if (bytes.len() + chunk.len()) as u64 > max_bytes {
            anyhow::bail!(
                "The file is too large (max {} MB).",
                UploadLimits::megabytes(max_bytes)
            );
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Content type for a stored file name, or `None` if the name is not one we generate.
fn content_type(name: &str) -> Option<&'static str> {
    let (stem, ext) = name.split_once('.')?;
    let valid_stem = stem.len() == 32 && stem.bytes().all(|b| b.is_ascii_hexdigit());
    if !valid_stem {
        return None;
    }
    match ext {
        "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "mp3" => Some("audio/mpeg"),
        "aac" => Some("audio/aac"),
        "m4a" => Some("audio/mp4"),
        "ogg" => Some("audio/ogg"),
        "flac" => Some("audio/flac"),
        "wav" => Some("audio/wav"),
        // WebM soundtracks play fine when served as video.
        "webm" => Some("video/webm"),
        "mp4" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        _ => None,
    }
}

/// Axum handler for `GET /media/{name}`.
///
/// Honours single byte ranges so browsers can seek in soundtracks and videos, reading only the
/// slice asked for; open-ended ranges get at most [`MAX_SLICE_BYTES`] at a time. Files are
/// streamed, so big videos asked for many times at once do not fill the memory.
pub async fn serve(Path(name): Path<String>, headers: HeaderMap) -> Response {
    use tokio::io::AsyncSeekExt;

    let Some((mut file, len, content_type)) = super::state().media.open_file(&name).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let common = [
        (header::CONTENT_TYPE, content_type.to_owned()),
        (
            header::CACHE_CONTROL,
            "public, max-age=31536000, immutable".to_owned(),
        ),
        (header::ACCEPT_RANGES, "bytes".to_owned()),
    ];
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let Some(range) = range else {
        return (
            common,
            [(header::CONTENT_LENGTH, len.to_string())],
            stream(file, len),
        )
            .into_response();
    };
    let Some((start, end)) = byte_range(range, len) else {
        return (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [(header::CONTENT_RANGE, format!("bytes */{len}"))],
        )
            .into_response();
    };
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let slice = end - start + 1;
    (
        StatusCode::PARTIAL_CONTENT,
        common,
        [
            (header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}")),
            (header::CONTENT_LENGTH, slice.to_string()),
        ],
        stream(file, slice),
    )
        .into_response()
}

/// The next `len` bytes of `file`, read as they are sent.
fn stream(file: tokio::fs::File, len: u64) -> Body {
    use tokio::io::AsyncReadExt;

    const CHUNK_BYTES: usize = 64 * 1024;
    let chunks = futures_util::stream::unfold(Some(file.take(len)), |file| async move {
        let mut file = file?;
        let mut chunk = vec![0; CHUNK_BYTES];
        match file.read(&mut chunk).await {
            Ok(0) => None,
            Ok(n) => {
                chunk.truncate(n);
                Some((Ok(chunk), Some(file)))
            }
            // Ends the body after the error.
            Err(err) => Some((Err(err), None)),
        }
    });
    Body::from_stream(chunks)
}

/// Parses a single `bytes=` range into inclusive bounds within a body of `len` bytes. An
/// open-ended range (`bytes=N-`) is cut to [`MAX_SLICE_BYTES`].
fn byte_range(header: &str, len: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    let (start, end) = spec.split_once('-')?;
    let last = len.checked_sub(1)?;
    let (start, end) = if start.is_empty() {
        // `bytes=-N`: the final N bytes.
        let suffix: u64 = end.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        (len.saturating_sub(suffix), last)
    } else {
        let start: u64 = start.parse().ok()?;
        let end = if end.is_empty() {
            last.min(start.saturating_add(MAX_SLICE_BYTES - 1))
        } else {
            end.parse::<u64>().ok()?.min(last)
        };
        (start, end)
    };
    (start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(width, height)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[tokio::test]
    async fn unused_uploads_are_collected_after_the_grace_period() {
        let dir = std::env::temp_dir().join(format!("quaesito-media-{}", uuid::Uuid::new_v4()));
        let store = MediaStore::open(&dir).await.unwrap();
        let kept = store.store(png(8, 8), ImageKind::Question).await.unwrap();
        let unused = store.store(png(8, 8), ImageKind::Question).await.unwrap();
        tokio::fs::write(dir.join("notes.txt"), b"not ours")
            .await
            .unwrap();
        let used = std::iter::once(kept.clone()).collect();

        // Fresh uploads may still be on their way into a quiz.
        let hour = std::time::Duration::from_hours(1);
        assert_eq!(store.collect_garbage(&used, hour).await.unwrap(), 0);
        let none = std::time::Duration::ZERO;
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(store.collect_garbage(&used, none).await.unwrap(), 1);
        assert!(store.read_bytes(&kept).await.is_some());
        assert!(store.read_bytes(&unused).await.is_none());
        assert!(dir.join("notes.txt").exists());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[test]
    fn videos_are_recognised_by_their_first_bytes() {
        let iso = |brand: &[u8]| {
            let mut bytes = vec![0, 0, 0, 0x20];
            bytes.extend(b"ftyp");
            bytes.extend(brand);
            bytes
        };
        assert_eq!(video_extension(&iso(b"isom")), Some("mp4"));
        assert_eq!(video_extension(&iso(b"mp42")), Some("mp4"));
        assert_eq!(video_extension(&iso(b"qt  ")), Some("mov"));
        assert_eq!(video_extension(&iso(b"M4A ")), None);
        assert_eq!(video_extension(&[0x1A, 0x45, 0xDF, 0xA3, 1]), Some("webm"));
        assert_eq!(video_extension(b"ID3\x04"), None);
        assert_eq!(
            content_type(&format!("{}.mp4", "a".repeat(32))),
            Some("video/mp4")
        );
    }

    #[test]
    fn question_pictures_are_downscaled_jpegs() {
        let (data, ext) = process(&png(3200, 1200), ImageKind::Question).unwrap();
        assert_eq!(ext, "jpg");
        let image = image::load_from_memory(&data).unwrap();
        assert_eq!((image.width(), image.height()), (1600, 600));
    }

    #[test]
    fn small_question_pictures_keep_their_size() {
        let (data, _) = process(&png(400, 300), ImageKind::Question).unwrap();
        let image = image::load_from_memory(&data).unwrap();
        assert_eq!((image.width(), image.height()), (400, 300));
    }

    #[test]
    fn avatars_are_square_pngs() {
        let (data, ext) = process(&png(800, 300), ImageKind::Avatar).unwrap();
        assert_eq!(ext, "png");
        let image = image::load_from_memory(&data).unwrap();
        assert_eq!((image.width(), image.height()), (256, 256));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(process(b"not an image", ImageKind::Avatar).is_err());
    }

    #[test]
    fn audio_is_recognised_by_its_magic_bytes() {
        assert_eq!(audio_extension(b"ID3\x04rest"), Some("mp3"));
        assert_eq!(audio_extension(&[0xFF, 0xFB, 0x90, 0x00]), Some("mp3"));
        assert_eq!(audio_extension(&[0xFF, 0xF1, 0x50, 0x80]), Some("aac"));
        assert_eq!(audio_extension(b"OggS\0\x02"), Some("ogg"));
        assert_eq!(audio_extension(b"fLaC\0\0"), Some("flac"));
        assert_eq!(audio_extension(b"RIFF\x24\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(audio_extension(b"\0\0\0\x20ftypM4A "), Some("m4a"));
        assert_eq!(
            audio_extension(&[0x1A, 0x45, 0xDF, 0xA3, 0x01]),
            Some("webm")
        );
        assert_eq!(audio_extension(b"<html>"), None);
        assert_eq!(audio_extension(b""), None);
    }

    #[test]
    fn byte_ranges_are_clamped_to_the_body() {
        assert_eq!(byte_range("bytes=0-", 10), Some((0, 9)));
        assert_eq!(byte_range("bytes=2-4", 10), Some((2, 4)));
        assert_eq!(byte_range("bytes=5-100", 10), Some((5, 9)));
        assert_eq!(byte_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(byte_range("bytes=10-", 10), None);
        assert_eq!(byte_range("bytes=4-2", 10), None);
        assert_eq!(byte_range("bytes=0-", 0), None);
        assert_eq!(byte_range("items=0-1", 10), None);
        // Open-ended ranges of big files come in slices.
        let big = MAX_SLICE_BYTES * 3;
        assert_eq!(byte_range("bytes=0-", big), Some((0, MAX_SLICE_BYTES - 1)));
        assert_eq!(
            byte_range("bytes=0-", MAX_SLICE_BYTES / 2),
            Some((0, MAX_SLICE_BYTES / 2 - 1))
        );
        assert_eq!(byte_range("bytes=5-", big), Some((5, MAX_SLICE_BYTES + 4)));
    }

    #[test]
    fn only_generated_names_are_served() {
        assert_eq!(
            content_type("0123456789abcdef0123456789abcdef.jpg"),
            Some("image/jpeg")
        );
        assert_eq!(
            content_type("0123456789abcdef0123456789abcdef.mp3"),
            Some("audio/mpeg")
        );
        assert_eq!(content_type("../secret.jpg"), None);
        assert_eq!(content_type("0123456789abcdef0123456789abcdef.svg"), None);
    }
}
