//! The bundled question library (see [`crate::library`]), and this server's edits to it.

use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    library::{AddedTopic, LibraryCategory, LibraryEdit, LibraryEntry},
    model::Quiz,
};

/// Longest name a category or topic added on this server may have, in characters.
const MAX_NAME_CHARS: usize = 60;

/// Every library question: the topic files under `library/`, combined by `build.rs`.
const LIBRARY: &str = include_str!(concat!(env!("OUT_DIR"), "/library.json"));

/// The library's questions as bundled, each with its id, read on first use.
///
/// # Panics
/// If the bundled file does not parse, which the tests rule out.
pub fn entries() -> &'static [LibraryEntry] {
    static ENTRIES: OnceLock<Vec<LibraryEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut entries: Vec<LibraryEntry> =
            serde_json::from_str(LIBRARY).expect("the bundled library is valid");
        for entry in &mut entries {
            entry.id = id_of(entry);
        }
        entries
    })
}

/// A stable id for a bundled entry, from its content: edits stay with their question across
/// restarts and new versions, and are left behind when the bundled question itself changes.
fn id_of(entry: &LibraryEntry) -> String {
    let json = serde_json::to_string(entry).expect("entries serialise");
    // FNV-1a: tiny, and the same in every Rust version (unlike the standard hasher).
    let hash = json.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// An edit as kept in `library-edits.json`: with where the question came from, so
/// `tools/library/apply-edits.py` can find it in the library's topic files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct StoredEdit {
    #[serde(default)]
    category: String,
    #[serde(default)]
    topic: String,
    /// The question's text as bundled.
    #[serde(default)]
    text: String,
    #[serde(flatten)]
    edit: LibraryEdit,
}

/// This server's edits to library questions, by id, kept in one JSON file.
pub struct LibraryEdits {
    path: PathBuf,
    edits: Mutex<BTreeMap<String, StoredEdit>>,
}

impl LibraryEdits {
    /// Reads the edits saved at `path`, if any.
    ///
    /// # Errors
    /// Fails if the file exists but cannot be read or parsed.
    pub async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let edits = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("reading {}", path.display()))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self {
            path,
            edits: Mutex::new(edits),
        })
    }

    /// The library with this server's edits applied.
    pub async fn entries(&self) -> Vec<LibraryEntry> {
        let edits = self.edits.lock().await;
        entries()
            .iter()
            .map(|entry| apply(entry, edits.get(&entry.id)))
            .collect()
    }

    /// Saves an edit of the bundled question `id` and returns the edited entry.
    ///
    /// # Errors
    /// Fails if there is no such question or the edits cannot be written.
    pub async fn save(&self, id: &str, edit: LibraryEdit) -> anyhow::Result<LibraryEntry> {
        let entry = find(id)?;
        let mut edits = self.edits.lock().await;
        edits.insert(
            id.to_owned(),
            StoredEdit {
                category: entry.category.clone(),
                topic: entry.topic.clone(),
                text: entry.question.text.clone(),
                edit,
            },
        );
        self.write(&edits).await?;
        Ok(apply(entry, edits.get(id)))
    }

    /// Drops the edit of question `id`, if any, and returns the bundled question.
    ///
    /// # Errors
    /// Fails if there is no such question or the edits cannot be written.
    pub async fn revert(&self, id: &str) -> anyhow::Result<LibraryEntry> {
        let entry = find(id)?;
        let mut edits = self.edits.lock().await;
        if edits.remove(id).is_some() {
            self.write(&edits).await?;
        }
        Ok(entry.clone())
    }

    /// Names of the uploaded files edited questions use, so they are not cleaned up.
    pub async fn media(&self) -> Vec<String> {
        let edits = self.edits.lock().await;
        let mut quiz = Quiz::new(String::new(), String::new());
        quiz.questions = edits.values().map(|e| e.edit.question.clone()).collect();
        quiz.media()
    }

    async fn write(&self, edits: &BTreeMap<String, StoredEdit>) -> anyhow::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(edits)?).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }
}

/// The categories and topics this server added to the library, kept in one JSON file. Quizzes
/// file their questions under these and the library's own.
pub struct AddedTopics {
    path: PathBuf,
    topics: Mutex<Vec<AddedTopic>>,
}

impl AddedTopics {
    /// Reads the topics saved at `path`, if any.
    ///
    /// # Errors
    /// Fails if the file exists but cannot be read or parsed.
    pub async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let topics = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("reading {}", path.display()))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self {
            path,
            topics: Mutex::new(topics),
        })
    }

    /// Every category with its topics: those of `entries`, and the ones added here.
    pub async fn subjects(&self, entries: &[LibraryEntry]) -> Vec<LibraryCategory> {
        crate::library::subjects(entries, &self.topics.lock().await)
    }

    /// Adds `topic` to `category`, a new one or one of `entries` (whatever the case it is typed
    /// in), and returns every category.
    ///
    /// # Errors
    /// Fails if a name is empty or too long, the topic already exists, or the file cannot be
    /// written.
    pub async fn add(
        &self,
        entries: &[LibraryEntry],
        category: &str,
        topic: &str,
    ) -> anyhow::Result<Vec<LibraryCategory>> {
        let (category, topic) = (category.trim(), topic.trim());
        anyhow::ensure!(
            !category.is_empty() && !topic.is_empty(),
            "Type a category and a topic."
        );
        anyhow::ensure!(
            category.chars().count() <= MAX_NAME_CHARS && topic.chars().count() <= MAX_NAME_CHARS,
            "Keep names to {MAX_NAME_CHARS} characters."
        );
        let mut topics = self.topics.lock().await;
        let same = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
        let existing = crate::library::subjects(entries, &topics);
        let found = existing.iter().find(|c| same(&c.name, category));
        if let Some(known) = found.and_then(|c| c.topics.iter().find(|t| same(&t.name, topic))) {
            anyhow::bail!(
                "{} already has the topic {}.",
                found.map_or(category, |c| &c.name),
                known.name
            );
        }
        topics.push(AddedTopic {
            category: found.map_or(category, |c| &c.name).to_owned(),
            topic: topic.to_owned(),
        });
        self.write(&topics).await?;
        Ok(crate::library::subjects(entries, &topics))
    }

    /// Removes a topic added here, and its category if it has no other, and returns every
    /// category.
    ///
    /// # Errors
    /// Fails if the topic was not added here, or the file cannot be written.
    pub async fn remove(
        &self,
        entries: &[LibraryEntry],
        category: &str,
        topic: &str,
    ) -> anyhow::Result<Vec<LibraryCategory>> {
        let mut topics = self.topics.lock().await;
        let index = topics
            .iter()
            .position(|t| t.category == category && t.topic == topic)
            .context("Only topics added on this server can be removed.")?;
        topics.remove(index);
        self.write(&topics).await?;
        Ok(crate::library::subjects(entries, &topics))
    }

    async fn write(&self, topics: &[AddedTopic]) -> anyhow::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(topics)?).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }
}

fn find(id: &str) -> anyhow::Result<&'static LibraryEntry> {
    entries()
        .iter()
        .find(|e| e.id == id)
        .context("This library question does not exist.")
}

fn apply(entry: &LibraryEntry, edit: Option<&StoredEdit>) -> LibraryEntry {
    let mut entry = entry.clone();
    if let Some(StoredEdit { edit, .. }) = edit {
        entry.difficulty = edit.difficulty;
        entry.question = edit.question.clone();
        entry.edited = true;
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::QuestionKind;

    #[test]
    fn every_library_question_is_playable() {
        let entries = entries();
        assert!(entries.len() > 1000);
        let mut quiz = Quiz::new("library".into(), "Library".into());
        quiz.questions = entries.iter().map(|e| e.question.clone()).collect();
        assert_eq!(quiz.validate(), []);
        for entry in entries {
            let q = &entry.question;
            assert!((1..=3).contains(&entry.difficulty), "{}", q.text);
            assert!(
                !matches!(
                    q.kind,
                    QuestionKind::Pin { .. } | QuestionKind::Slide { .. }
                ),
                "{}",
                q.text
            );
            assert!(q.image.is_none() && q.audio.is_none(), "{}", q.text);
        }
    }

    #[test]
    fn every_library_question_has_its_own_id() {
        let mut ids: Vec<&str> = entries().iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), entries().len());
    }

    #[tokio::test]
    async fn edits_are_kept_applied_and_reverted() {
        let dir = std::env::temp_dir().join(format!("quaesito-library-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("library-edits.json");
        let original = entries()[0].clone();
        let mut question = original.question.clone();
        question.text = "Edited?".into();
        question.image = Some("picture.webp".into());
        let edit = LibraryEdit {
            difficulty: 3,
            question,
        };

        let edits = LibraryEdits::open(path.clone()).await.unwrap();
        let edited = edits.save(&original.id, edit.clone()).await.unwrap();
        assert!(edited.edited && edited.difficulty == 3 && edited.id == original.id);
        assert!(edits.save("nope", edit).await.is_err());

        // A restart reads the edits back; the file says where each question came from.
        let saved = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(saved.contains(&format!(
            "\"text\": {}",
            serde_json::to_string(&original.question.text).unwrap()
        )));
        let edits = LibraryEdits::open(path).await.unwrap();
        assert_eq!(edits.entries().await[0].question.text, "Edited?");
        assert_eq!(edits.media().await, ["picture.webp"]);

        assert_eq!(edits.revert(&original.id).await.unwrap(), original);
        assert_eq!(edits.entries().await[0], original);
        assert_eq!(edits.media().await, Vec::<String>::new());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn topics_are_added_kept_and_removed() {
        let dir = std::env::temp_dir().join(format!("quaesito-topics-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("library-topics.json");
        let entries = entries();
        let known = &entries[0];
        let topics = AddedTopics::open(path.clone()).await.unwrap();

        // A known category keeps its spelling; a known topic is refused.
        let lower = known.category.to_lowercase();
        let found = topics
            .add(entries, &lower, " Odds and ends ")
            .await
            .unwrap();
        let category = found.iter().find(|c| c.name == known.category).unwrap();
        assert!(
            category
                .topics
                .iter()
                .any(|t| t.name == "Odds and ends" && t.count == 0)
        );
        let upper = known.topic.to_uppercase();
        assert!(topics.add(entries, &known.category, &upper).await.is_err());
        assert!(topics.add(entries, "Board games", " ").await.is_err());
        let found = topics.add(entries, "Board games", "Chess").await.unwrap();
        assert!(
            found
                .iter()
                .any(|c| c.name == "Board games" && c.count == 0)
        );

        // A restart reads them back; library topics cannot be removed.
        let topics = AddedTopics::open(path).await.unwrap();
        assert!(
            topics
                .remove(entries, &known.category, &known.topic)
                .await
                .is_err()
        );
        let found = topics
            .remove(entries, "Board games", "Chess")
            .await
            .unwrap();
        assert!(!found.iter().any(|c| c.name == "Board games"));
        assert_eq!(found.len(), topics.subjects(entries).await.len());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
