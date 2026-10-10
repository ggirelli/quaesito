//! Uploaded pictures: validation, re-encoding and serving.

use std::{io::Cursor, path::PathBuf};

use anyhow::Context;
use dioxus::fullstack::{
    axum::{
        extract::Path,
        http::{StatusCode, header},
        response::{IntoResponse, Response},
    },
    body::Body,
};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, imageops::FilterType};

/// Largest accepted upload, in bytes.
pub const MAX_UPLOAD_BYTES: usize = 15 * 1024 * 1024;

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

/// Directory of processed pictures.
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

    /// Reads a stored picture by file name.
    async fn read(&self, name: &str) -> Option<(Vec<u8>, &'static str)> {
        let content_type = content_type(name)?;
        let data = tokio::fs::read(self.dir.join(name)).await.ok()?;
        Some((data, content_type))
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
        _ => None,
    }
}

/// Axum handler for `GET /media/{name}`.
pub async fn serve(Path(name): Path<String>) -> Response {
    match super::state().media.read(&name).await {
        Some((data, content_type)) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            Body::from(data),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
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
    fn only_generated_names_are_served() {
        assert_eq!(
            content_type("0123456789abcdef0123456789abcdef.jpg"),
            Some("image/jpeg")
        );
        assert_eq!(content_type("../secret.jpg"), None);
        assert_eq!(content_type("0123456789abcdef0123456789abcdef.svg"), None);
    }
}
