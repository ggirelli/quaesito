//! Quiz export and import as a zip bundle: `quiz.json` plus the pictures and soundtracks it uses
//! under `media/`. Plain JSON exports from older versions can still be imported.

use std::io::{Cursor, Read, Write};

use anyhow::Context;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::media::{self, ImageKind, MediaStore};
use crate::model::Quiz;

/// Largest accepted import, in bytes.
pub const MAX_BUNDLE_BYTES: usize = 300 * 1024 * 1024;
/// The quiz document inside a bundle.
const QUIZ_FILE: &str = "quiz.json";
/// Largest quiz document accepted inside a bundle.
const MAX_QUIZ_BYTES: u64 = 10 * 1024 * 1024;

/// Packs a quiz and the media it uses. Media missing from the store are left out.
///
/// # Errors
/// Fails if the zip cannot be written.
pub async fn export(quiz: &Quiz, store: &MediaStore) -> anyhow::Result<Vec<u8>> {
    let mut files = Vec::new();
    for name in quiz.media() {
        if let Some(bytes) = store.read_bytes(&name).await {
            files.push((name, bytes));
        }
    }
    let json = serde_json::to_vec_pretty(quiz)?;
    tokio::task::spawn_blocking(move || {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(QUIZ_FILE, SimpleFileOptions::default())?;
        zip.write_all(&json)?;
        // Pictures and soundtracks are compressed already.
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, bytes) in files {
            zip.start_file(format!("media/{name}"), stored)?;
            zip.write_all(&bytes)?;
        }
        Ok(zip.finish()?.into_inner())
    })
    .await?
}

/// Reads an exported quiz (zip bundle or plain JSON). Bundled media are checked and processed
/// like fresh uploads and stored under new names; references to media that are missing or
/// rejected are dropped.
///
/// # Errors
/// Fails if the data is neither a quiz bundle nor quiz JSON.
pub async fn import(bytes: Vec<u8>, store: &MediaStore) -> anyhow::Result<Quiz> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return serde_json::from_slice(&bytes).context("Not a quiz file");
    }
    let (mut quiz, files) = tokio::task::spawn_blocking(move || unpack(bytes)).await??;
    let mut renamed = std::collections::HashMap::new();
    for (name, bytes) in files {
        let stored = if media::audio_extension(&bytes).is_some() {
            store.store_audio(bytes).await
        } else {
            store.store(bytes, ImageKind::Question).await
        };
        match stored {
            Ok(new_name) => {
                renamed.insert(name, new_name);
            }
            Err(err) => dioxus::logger::tracing::warn!("skipping bundled {name}: {err:#}"),
        }
    }
    quiz.rename_media(|name| renamed.get(name).cloned());
    Ok(quiz)
}

/// Bundled files by their name in the quiz.
type MediaFiles = Vec<(String, Vec<u8>)>;

/// The quiz and the bundled media it refers to.
fn unpack(bytes: Vec<u8>) -> anyhow::Result<(Quiz, MediaFiles)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).context("Not a quiz bundle")?;
    let json = read_entry(&mut archive, QUIZ_FILE, MAX_QUIZ_BYTES)?
        .context("The bundle has no quiz.json")?;
    let quiz: Quiz = serde_json::from_slice(&json).context("Not a valid quiz")?;
    let limit = u64::try_from(media::MAX_AUDIO_BYTES).unwrap_or(u64::MAX);
    let mut files = Vec::new();
    for name in quiz.media() {
        if let Some(bytes) = read_entry(&mut archive, &format!("media/{name}"), limit)? {
            files.push((name, bytes));
        }
    }
    Ok((quiz, files))
}

/// One file of the archive, or `None` if it is absent. Refuses files larger than `limit` bytes
/// once decompressed.
fn read_entry(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    name: &str,
    limit: u64,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Ok(file) = archive.by_name(name) else {
        return Ok(None);
    };
    if file.size() > limit {
        anyhow::bail!("{name} is too large");
    }
    // The declared size can lie: never read past the limit.
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        anyhow::bail!("{name} is too large");
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Question};

    fn png() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(4, 3, image::Rgb([255, 183, 3]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[tokio::test]
    async fn bundles_round_trip_with_their_media() {
        let dir = std::env::temp_dir().join(format!("quaesito-bundle-{}", uuid::Uuid::new_v4()));
        let store = MediaStore::open(&dir).await.unwrap();
        let picture = store.store(png(), ImageKind::Question).await.unwrap();
        let mut sound = b"ID3".to_vec();
        sound.extend([0; 64]);
        let soundtrack = store.store_audio(sound).await.unwrap();
        let quiz = Quiz {
            questions: vec![Question {
                text: "Q".to_owned(),
                image: Some(picture.clone()),
                audio: Some(soundtrack.clone()),
                answers: vec![
                    Answer {
                        text: String::new(),
                        correct: true,
                        image: Some(picture.clone()),
                    },
                    Answer {
                        text: "B".to_owned(),
                        correct: false,
                        image: Some("gone.jpg".to_owned()),
                    },
                ],
                ..Question::default()
            }],
            ..Quiz::new("q".to_owned(), "Bundled".to_owned())
        };

        let bytes = export(&quiz, &store).await.unwrap();
        let imported = import(bytes, &store).await.unwrap();
        let question = &imported.questions[0];
        let new_picture = question.image.clone().unwrap();
        assert_ne!(new_picture, picture);
        assert_eq!(question.answers[0].image.as_ref(), Some(&new_picture));
        assert!(store.read_bytes(&new_picture).await.is_some());
        assert!(
            question
                .audio
                .as_ref()
                .is_some_and(|a| std::path::Path::new(a).extension() == Some("mp3".as_ref()))
        );
        // The missing picture is dropped rather than failing the import.
        assert_eq!(question.answers[1].image, None);

        // Older plain JSON exports still import.
        let json = serde_json::to_vec(&quiz).unwrap();
        assert_eq!(import(json, &store).await.unwrap().title, "Bundled");
        assert!(import(b"PK\x03\x04garbage".to_vec(), &store).await.is_err());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
