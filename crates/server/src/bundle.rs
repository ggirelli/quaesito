//! Quiz export and import as a zip bundle: `quiz.json` plus the pictures and soundtracks it uses
//! under `media/`. Plain JSON exports from older versions can still be imported.

use std::io::{Cursor, Read, Write};

use anyhow::Context;
use futures_util::StreamExt;
use quaesito_core::{
    model::{ImportWarning, QUIZ_FORMAT, Question, QuestionKind, Quiz, UnknownQuestion, is_video},
    protocol::UploadLimits,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::media::{ImageKind, MediaStore};

/// The quiz document inside a bundle.
const QUIZ_FILE: &str = "quiz.json";
/// Largest quiz document accepted inside a bundle.
const MAX_QUIZ_BYTES: u64 = 10 * 1024 * 1024;

/// Where the format of [`QUIZ_FORMAT`] is described: the guide publishes `docs/schema`.
fn schema_url() -> String {
    format!("https://ggirelli.github.io/quaesito/schema/quiz-{QUIZ_FORMAT}.json")
}

/// The quiz document of an export: the quiz, with the version of the format it is written in.
#[derive(serde::Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
struct QuizFile<'a> {
    /// Where this format is described, for editors that check JSON files.
    #[serde(rename = "$schema")]
    schema: String,
    /// Version of the file format. Files without it are version 1.
    format: u32,
    #[serde(flatten)]
    quiz: &'a Quiz,
}

/// What an import brings: the quiz, or why it would not come through whole.
#[derive(Debug)]
pub enum Imported {
    Quiz(Quiz),
    /// The file comes from a newer version; nothing was stored.
    NeedsConsent(ImportWarning),
}

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
    let json = serde_json::to_vec_pretty(&QuizFile {
        schema: schema_url(),
        format: QUIZ_FORMAT,
        quiz,
    })?;
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
/// A file from a newer version is only imported with `consent`, leaving out the questions of
/// kinds this version does not know; without it, nothing is stored and the warning says what
/// would be lost. `on_media(done, total)` follows the processing of the bundled media, several
/// at a time.
///
/// # Errors
/// Fails if the data is neither a quiz bundle nor quiz JSON.
pub async fn import(
    bytes: Vec<u8>,
    store: &MediaStore,
    limits: &UploadLimits,
    consent: bool,
    mut on_media: impl FnMut(usize, usize),
) -> anyhow::Result<Imported> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(match parse(&bytes)? {
            Parsed {
                warning: Some(warning),
                ..
            } if !consent => Imported::NeedsConsent(warning),
            Parsed { quiz, .. } => Imported::Quiz(quiz),
        });
    }
    let largest = limits.picture.max(limits.audio).max(limits.video);
    let (parsed, files) = tokio::task::spawn_blocking(move || unpack(bytes, largest)).await??;
    let mut quiz = match parsed {
        Parsed {
            warning: Some(warning),
            ..
        } if !consent => return Ok(Imported::NeedsConsent(warning)),
        Parsed { quiz, .. } => quiz,
    };
    // What a file is depends on where the quiz uses it: a WebM file can be a soundtrack or a
    // video, and an MP4 video starts like an M4A soundtrack.
    let soundtracks: std::collections::HashSet<String> = quiz
        .questions
        .iter()
        .flat_map(|q| [q.audio.clone(), q.explanation_audio.clone()])
        .flatten()
        .collect();
    for (name, bytes) in &files {
        let limit = if soundtracks.contains(name) {
            limits.audio
        } else if is_video(name) {
            limits.video
        } else {
            limits.picture
        };
        check_size(bytes.len() as u64, limit, name)?;
    }
    let total = files.len();
    on_media(0, total);
    // Pictures are decoded and resized on as many cores as there are.
    let parallel = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    let mut processed = futures_util::stream::iter(files)
        .map(|(name, bytes)| async {
            let stored = if soundtracks.contains(&name) {
                store.store_audio(bytes).await
            } else if is_video(&name) {
                store.store_video(bytes).await
            } else {
                store.store(bytes, ImageKind::Question).await
            };
            (name, stored)
        })
        .buffer_unordered(parallel);
    let mut renamed = std::collections::HashMap::new();
    let mut done = 0;
    while let Some((name, stored)) = processed.next().await {
        done += 1;
        on_media(done, total);
        match stored {
            Ok(new_name) => {
                renamed.insert(name, new_name);
            }
            Err(err) => dioxus::logger::tracing::warn!("skipping bundled {name}: {err:#}"),
        }
    }
    quiz.rename_media(|name| renamed.get(name).cloned());
    Ok(Imported::Quiz(quiz))
}

/// A quiz document as read, with what it holds that this version cannot.
struct Parsed {
    /// The quiz, without the questions of unknown kinds.
    quiz: Quiz,
    /// Set when the file is newer than this version.
    warning: Option<ImportWarning>,
}

/// Reads a quiz document, setting aside the questions of kinds this version does not know.
fn parse(json: &[u8]) -> anyhow::Result<Parsed> {
    let mut document: serde_json::Value =
        serde_json::from_slice(json).context("Not a quiz file")?;
    let format = document
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .map_or(QUIZ_FORMAT, |f| u32::try_from(f).unwrap_or(u32::MAX));
    let newer_format = (format > QUIZ_FORMAT).then_some(format);
    // From a newer version, whatever does not read is most likely new.
    let invalid = |err: serde_json::Error, what: String| match newer_format {
        Some(_) => anyhow::anyhow!(
            "This quiz was made with a newer version of Quaesito: update Quaesito to import it ({what}: {err})."
        ),
        None => anyhow::anyhow!("Not a valid quiz ({what}: {err})"),
    };
    let items = match document
        .as_object_mut()
        .and_then(|fields| fields.remove("questions"))
    {
        Some(serde_json::Value::Array(items)) => items,
        Some(_) => anyhow::bail!("Not a valid quiz (its questions are not a list)"),
        None => Vec::new(),
    };
    let mut quiz: Quiz =
        serde_json::from_value(document).map_err(|err| invalid(err, "settings".to_owned()))?;
    let mut unknown = Vec::new();
    // Questions so far, slides left out, to name them as the editor does.
    let mut number = 0;
    for (index, item) in items.into_iter().enumerate() {
        if let Some(kind) = kind_name(&item).filter(|k| !QuestionKind::NAMES.contains(k)) {
            number += 1;
            unknown.push(UnknownQuestion {
                place: format!("Question {number}"),
                kind: kind.to_owned(),
                text: item
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
            continue;
        }
        let question: Question = serde_json::from_value(item)
            .map_err(|err| invalid(err, format!("item {} of the list", index + 1)))?;
        if !question.is_slide() {
            number += 1;
        }
        quiz.questions.push(question);
    }
    let warning = (newer_format.is_some() || !unknown.is_empty()).then(|| ImportWarning {
        title: quiz.title.clone(),
        newer_format,
        unknown,
    });
    Ok(Parsed { quiz, warning })
}

/// The kind a stored question names: `"Choice"`, or the one key of `{"Text": {…}}`.
fn kind_name(question: &serde_json::Value) -> Option<&str> {
    match question.get("kind")? {
        serde_json::Value::String(name) => Some(name),
        serde_json::Value::Object(fields) if fields.len() == 1 => {
            fields.keys().next().map(String::as_str)
        }
        _ => None,
    }
}

/// Bundled files by their name in the quiz.
type MediaFiles = Vec<(String, Vec<u8>)>;

/// Refuses a bundled file larger than this server accepts for its kind.
fn check_size(size: u64, limit: u64, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        size <= limit,
        "{name} is larger than this server accepts ({} MB)",
        UploadLimits::megabytes(limit)
    );
    Ok(())
}

/// The quiz and the bundled media it refers to, each file at most `limit` bytes.
fn unpack(bytes: Vec<u8>, limit: u64) -> anyhow::Result<(Parsed, MediaFiles)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).context("Not a quiz bundle")?;
    let json = read_entry(&mut archive, QUIZ_FILE, MAX_QUIZ_BYTES)?
        .context("The bundle has no quiz.json")?;
    let parsed = parse(&json)?;
    let mut files = Vec::new();
    for name in parsed.quiz.media() {
        if let Some(bytes) = read_entry(&mut archive, &format!("media/{name}"), limit)? {
            files.push((name, bytes));
        }
    }
    Ok((parsed, files))
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
    use quaesito_core::model::Answer;

    /// The quiz an import brought, failing if it asked for consent.
    fn quiz_of(imported: Imported) -> Quiz {
        match imported {
            Imported::Quiz(quiz) => quiz,
            Imported::NeedsConsent(warning) => panic!("unexpected warning: {warning:?}"),
        }
    }

    /// The warning of an import that asked for consent.
    fn warning_of(imported: Imported) -> ImportWarning {
        match imported {
            Imported::NeedsConsent(warning) => warning,
            Imported::Quiz(quiz) => panic!("imported without asking: {quiz:?}"),
        }
    }

    fn png() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(4, 3, image::Rgb([255, 183, 3]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn limits() -> UploadLimits {
        crate::media::limits_from_env()
    }

    #[tokio::test]
    async fn bundles_round_trip_with_their_media() {
        let dir = std::env::temp_dir().join(format!("quaesito-bundle-{}", uuid::Uuid::new_v4()));
        let store = MediaStore::open(&dir).await.unwrap();
        let picture = store.store(png(), ImageKind::Question).await.unwrap();
        let mut sound = b"ID3".to_vec();
        sound.extend([0; 64]);
        let soundtrack = store.store_audio(sound.clone()).await.unwrap();
        let recap = store.store_audio(sound).await.unwrap();
        let quiz = Quiz {
            questions: vec![Question {
                text: "Q".to_owned(),
                image: Some(picture.clone()),
                audio: Some(soundtrack.clone()),
                explanation_audio: Some(recap.clone()),
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
        let mut progress = Vec::new();
        let imported = quiz_of(
            import(bytes, &store, &limits(), false, |done, total| {
                progress.push((done, total));
            })
            .await
            .unwrap(),
        );
        // Every bundled file is counted, from none.
        assert_eq!(progress, [(0, 3), (1, 3), (2, 3), (3, 3)]);
        let question = &imported.questions[0];
        let new_picture = question.image.clone().unwrap();
        assert_ne!(new_picture, picture);
        assert_eq!(question.answers[0].image.as_ref(), Some(&new_picture));
        assert!(store.read_bytes(&new_picture).await.is_some());
        let is_mp3 = |a: &Option<String>| {
            a.as_ref()
                .is_some_and(|a| std::path::Path::new(a).extension() == Some("mp3".as_ref()))
        };
        assert!(is_mp3(&question.audio));
        // The recap's soundtrack is a soundtrack too, not a picture that fails to decode.
        assert!(is_mp3(&question.explanation_audio));
        // The missing picture is dropped rather than failing the import.
        assert_eq!(question.answers[1].image, None);

        // Older plain JSON exports still import.
        let json = serde_json::to_vec(&quiz).unwrap();
        assert_eq!(
            quiz_of(
                import(json, &store, &limits(), false, |_, _| {})
                    .await
                    .unwrap()
            )
            .title,
            "Bundled"
        );
        assert!(
            import(
                b"PK\x03\x04garbage".to_vec(),
                &store,
                &limits(),
                false,
                |_, _| {}
            )
            .await
            .is_err()
        );
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    /// A quiz document with these questions, in this format (`None` leaves the field out).
    fn document(format: Option<u32>, questions: &str) -> Vec<u8> {
        let format = format.map_or(String::new(), |f| format!(r#""format":{f},"#));
        format!(r#"{{{format}"id":"q","title":"Future","questions":[{questions}]}}"#).into_bytes()
    }

    #[test]
    fn exports_name_their_format_and_schema() {
        let quiz = Quiz::new("q".to_owned(), "Now".to_owned());
        let json = serde_json::to_value(QuizFile {
            schema: schema_url(),
            format: QUIZ_FORMAT,
            quiz: &quiz,
        })
        .unwrap();
        assert_eq!(json["format"], QUIZ_FORMAT);
        assert_eq!(json["$schema"], schema_url());
        assert_eq!(json["title"], "Now");
        let parsed = parse(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(parsed.quiz, quiz);
        assert_eq!(parsed.warning, None);
    }

    #[test]
    fn unknown_question_kinds_are_set_aside_and_named() {
        let questions = r#"
            {"text":"Known","kind":{"Text":{"accepted":["a"]}}},
            {"text":"","kind":{"Slide":{"title":"Break"}}},
            {"text":"Point at it","kind":{"Hotspot":{"spots":[1,2]}}},
            {"text":"Plain"},
            {"text":"Shout","kind":"Buzzer"}
        "#;
        let parsed = parse(&document(Some(QUIZ_FORMAT + 1), questions)).unwrap();
        let texts: Vec<_> = parsed
            .quiz
            .questions
            .iter()
            .map(|q| q.text.as_str())
            .collect();
        assert_eq!(texts, ["Known", "", "Plain"]);
        let warning = parsed.warning.unwrap();
        assert_eq!(warning.title, "Future");
        assert_eq!(warning.newer_format, Some(QUIZ_FORMAT + 1));
        // Named as the editor would: slides are not counted.
        assert_eq!(
            warning.unknown,
            [
                UnknownQuestion {
                    place: "Question 2".to_owned(),
                    kind: "Hotspot".to_owned(),
                    text: "Point at it".to_owned(),
                },
                UnknownQuestion {
                    place: "Question 4".to_owned(),
                    kind: "Buzzer".to_owned(),
                    text: "Shout".to_owned(),
                },
            ]
        );

        // Even a file that does not say it is newer is flagged.
        let parsed = parse(&document(None, questions)).unwrap();
        assert_eq!(parsed.warning.unwrap().unknown.len(), 2);
    }

    #[test]
    fn newer_files_are_flagged_even_when_every_question_reads() {
        let parsed = parse(&document(Some(QUIZ_FORMAT + 1), r#"{"text":"Q"}"#)).unwrap();
        let warning = parsed.warning.unwrap();
        assert_eq!(warning.newer_format, Some(QUIZ_FORMAT + 1));
        assert_eq!(warning.unknown, []);
        // Files of this format, or older ones without it, import without asking.
        assert_eq!(
            parse(&document(Some(QUIZ_FORMAT), r#"{"text":"Q"}"#))
                .unwrap()
                .warning,
            None
        );
        assert_eq!(
            parse(&document(None, r#"{"text":"Q"}"#)).unwrap().warning,
            None
        );
    }

    #[test]
    fn broken_questions_of_known_kinds_still_fail() {
        let err = parse(&document(
            None,
            r#"{"text":"Q"},{"text":"N","kind":{"Number":7}}"#,
        ))
        .err()
        .unwrap();
        assert!(
            err.to_string()
                .starts_with("Not a valid quiz (item 2 of the list"),
            "{err}"
        );
        let err = parse(br#"{"id":"q","title":"T","questions":{}}"#)
            .err()
            .unwrap();
        assert!(err.to_string().starts_with("Not a valid quiz"), "{err}");
    }

    #[test]
    fn what_a_newer_file_cannot_say_otherwise_asks_for_an_update() {
        let newer = format!(
            r#"{{"format":{},"id":"q","title":"T","settings":{{"reveal":"Never"}}}}"#,
            QUIZ_FORMAT + 1
        );
        let err = parse(newer.as_bytes()).err().unwrap();
        assert!(err.to_string().contains("update Quaesito"), "{err}");
    }

    #[tokio::test]
    async fn imports_from_newer_versions_wait_for_consent() {
        let dir = std::env::temp_dir().join(format!("quaesito-bundle-{}", uuid::Uuid::new_v4()));
        let store = MediaStore::open(&dir).await.unwrap();
        let json = document(
            Some(QUIZ_FORMAT + 1),
            r#"{"text":"Q"},{"text":"New","kind":{"Hotspot":{}}}"#,
        );
        let unsure = import(json.clone(), &store, &limits(), false, |_, _| {})
            .await
            .unwrap();
        assert_eq!(warning_of(unsure).unknown.len(), 1);
        let quiz = quiz_of(
            import(json, &store, &limits(), true, |_, _| {})
                .await
                .unwrap(),
        );
        assert_eq!(quiz.questions.len(), 1);
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    /// The published schema of the file format, in `docs/schema`.
    fn schema_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../docs/schema/quiz-{QUIZ_FORMAT}.json"))
    }

    /// The schema the quiz types describe, as published.
    fn schema() -> serde_json::Value {
        let mut schema = schemars::schema_for!(QuizFile<'static>);
        schema.insert("$id".to_owned(), schema_url().into());
        schema.insert("title".to_owned(), "Quaesito quiz".into());
        schema.insert(
            "description".to_owned(),
            format!("A quiz exported by Quaesito, file format {QUIZ_FORMAT}.").into(),
        );
        let mut schema = schema.to_value();
        // Exports write both, but files without them read as well.
        schema["required"]
            .as_array_mut()
            .unwrap()
            .retain(|field| field != "$schema" && field != "format");
        plain_descriptions(&mut schema);
        schema
    }

    /// Turns the Rust links of descriptions, from doc comments, into plain names:
    /// "[`QuestionKind::Pairs`]" becomes "`Pairs`".
    fn plain_descriptions(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, field) in fields.iter_mut() {
                    if key == "description"
                        && let serde_json::Value::String(text) = field
                    {
                        let mut plain = String::new();
                        let mut rest = text.as_str();
                        while let Some((before, link)) = rest.split_once("[`") {
                            let (path, after) = link.split_once("`]").unwrap();
                            let name = path.rsplit("::").next().unwrap();
                            plain.push_str(before);
                            plain.push('`');
                            plain.push_str(name);
                            plain.push('`');
                            rest = after;
                        }
                        plain.push_str(rest);
                        *text = plain;
                    } else {
                        plain_descriptions(field);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(plain_descriptions),
            _ => {}
        }
    }

    /// Fails when the quiz format changes without its published schema: run `just schema` and
    /// review the difference. If an older version would misread or lose what changed, raise
    /// `QUIZ_FORMAT` first, so the old schema stays published next to the new one.
    #[test]
    fn the_published_schema_is_current() {
        let current = serde_json::to_string_pretty(&schema()).unwrap() + "\n";
        let path = schema_path();
        if std::env::var_os("QUAESITO_WRITE_SCHEMA").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &current).unwrap();
        }
        let published = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            published == current,
            "The quiz file format changed: run `just schema`, then raise QUIZ_FORMAT if older \
             versions would misread or lose what changed."
        );
    }

    /// The question kinds this version knows are the ones in the schema.
    #[test]
    fn question_kind_names_match_the_schema() {
        let schema = schema();
        let mut names: Vec<String> = schema["$defs"]["QuestionKind"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|variant| {
                let mut units = variant["enum"].as_array().cloned().unwrap_or_default();
                units.extend(variant.get("const").cloned());
                let tagged = variant["required"].as_array().cloned().unwrap_or_default();
                units.into_iter().chain(tagged)
            })
            .map(|name| name.as_str().unwrap().to_owned())
            .collect();
        names.sort();
        let mut known = QuestionKind::NAMES.map(str::to_owned).to_vec();
        known.sort();
        assert_eq!(names, known, "update QuestionKind::NAMES");
    }
}
