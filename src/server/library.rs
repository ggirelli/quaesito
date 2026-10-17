//! The bundled question library (see [`crate::library`]), and this server's edits to it: changed
//! questions, and questions saved from quizzes.

use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    library::{
        Added, AddedCategory, AddedTopic, CATEGORY_COLORS, CATEGORY_ICONS, CategoryLook,
        LibraryCategory, LibraryEdit, LibraryEntry,
    },
    model::{Question, Quiz},
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
    crate::library::fnv(&serde_json::to_string(entry).expect("entries serialise"))
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

/// This server's edits to the library: changes to bundled questions, by id, and the questions
/// added here, each kept in a JSON file.
pub struct LibraryEdits {
    path: PathBuf,
    edits: Mutex<BTreeMap<String, StoredEdit>>,
    added_path: PathBuf,
    added: Mutex<Vec<LibraryEntry>>,
}

impl LibraryEdits {
    /// Reads the edits saved at `path` and the questions added at `added_path`, if any.
    ///
    /// # Errors
    /// Fails if a file exists but cannot be read or parsed.
    pub async fn open(path: PathBuf, added_path: PathBuf) -> anyhow::Result<Self> {
        let edits = read_or_default(&path).await?;
        let added = read_or_default(&added_path).await?;
        Ok(Self {
            path,
            edits: Mutex::new(edits),
            added_path,
            added: Mutex::new(added),
        })
    }

    /// The library with this server's edits applied, then the questions added here.
    pub async fn entries(&self) -> Vec<LibraryEntry> {
        let edits = self.edits.lock().await;
        let added = self.added.lock().await;
        entries()
            .iter()
            .map(|entry| apply(entry, edits.get(&entry.id)))
            .chain(added.iter().map(|entry| LibraryEntry {
                added: true,
                ..entry.clone()
            }))
            .collect()
    }

    /// Adds a question to the library, filed under the category and topic it has, and returns
    /// it with its new id.
    ///
    /// # Errors
    /// Fails if the question has no category or topic, or it cannot be written.
    pub async fn add(&self, difficulty: u8, question: Question) -> anyhow::Result<LibraryEntry> {
        anyhow::ensure!(
            !question.category.is_empty() && !question.topic.is_empty(),
            "Pick a category and a topic first."
        );
        let mut entry = LibraryEntry {
            id: crate::server::store::new_id(),
            edited: false,
            added: false,
            category: String::new(),
            topic: String::new(),
            difficulty,
            question: Question::default(),
        };
        file(&mut entry, question);
        let mut added = self.added.lock().await;
        added.push(entry.clone());
        write(&self.added_path, &*added).await?;
        Ok(LibraryEntry {
            added: true,
            ..entry
        })
    }

    /// Saves an edit of question `id` and returns the edited entry. A category and topic in
    /// the question move it there.
    ///
    /// # Errors
    /// Fails if there is no such question or the edits cannot be written.
    pub async fn save(&self, id: &str, edit: LibraryEdit) -> anyhow::Result<LibraryEntry> {
        let mut added = self.added.lock().await;
        if let Some(entry) = added.iter_mut().find(|e| e.id == id) {
            entry.difficulty = edit.difficulty;
            file(entry, edit.question);
            let entry = LibraryEntry {
                added: true,
                ..entry.clone()
            };
            write(&self.added_path, &*added).await?;
            return Ok(entry);
        }
        drop(added);
        let entry = find(id)?;
        let mut edits = self.edits.lock().await;
        edits.insert(
            id.to_owned(),
            StoredEdit {
                category: entry.category.clone(),
                topic: entry.topic.clone(),
                text: entry.question.text.clone(),
                edit: LibraryEdit {
                    difficulty: edit.difficulty,
                    question: crate::library::shared(&edit.question),
                },
            },
        );
        write(&self.path, &*edits).await?;
        Ok(apply(entry, edits.get(id)))
    }

    /// Drops the edit of bundled question `id`, if any, and returns the bundled question.
    ///
    /// # Errors
    /// Fails if there is no such question or the edits cannot be written.
    pub async fn revert(&self, id: &str) -> anyhow::Result<LibraryEntry> {
        let entry = find(id)?;
        let mut edits = self.edits.lock().await;
        if edits.remove(id).is_some() {
            write(&self.path, &*edits).await?;
        }
        Ok(entry.clone())
    }

    /// Deletes question `id`, one added here.
    ///
    /// # Errors
    /// Fails if the question was not added here, or the file cannot be written.
    pub async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let mut added = self.added.lock().await;
        let index = added
            .iter()
            .position(|e| e.id == id)
            .context("Only questions added on this server can be deleted.")?;
        added.remove(index);
        write(&self.added_path, &*added).await
    }

    /// Names of the uploaded files edited and added questions use, so they are not cleaned up.
    pub async fn media(&self) -> Vec<String> {
        let edits = self.edits.lock().await;
        let added = self.added.lock().await;
        let mut quiz = Quiz::new(String::new(), String::new());
        quiz.questions = edits
            .values()
            .map(|e| e.edit.question.clone())
            .chain(added.iter().map(|e| e.question.clone()))
            .collect();
        quiz.media()
    }
}

/// Files `question` in `entry`: under its category and topic, if it has them, without what only
/// a quiz has.
fn file(entry: &mut LibraryEntry, mut question: Question) {
    if !question.category.is_empty() {
        entry.category = std::mem::take(&mut question.category);
        entry.topic = std::mem::take(&mut question.topic);
    }
    entry.question = crate::library::shared(&question);
}

/// The JSON at `path`, or the default if there is no file.
async fn read_or_default<T: Default + serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> anyhow::Result<T> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("reading {}", path.display()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes `value` to `path` as JSON, whole or not at all.
async fn write(path: &std::path::Path, value: &impl Serialize) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(value)?).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

/// The categories and topics this server added to the library, kept in one JSON file. Quizzes
/// file their questions under these and the library's own.
pub struct AddedTopics {
    path: PathBuf,
    added: Mutex<Added>,
}

impl AddedTopics {
    /// Reads the categories and topics saved at `path`, if any.
    ///
    /// # Errors
    /// Fails if the file exists but cannot be read or parsed.
    pub async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let added = read_or_default(&path).await?;
        Ok(Self {
            path,
            added: Mutex::new(added),
        })
    }

    /// Every category with its topics: those of `entries`, and the ones added here.
    pub async fn subjects(&self, entries: &[LibraryEntry]) -> Vec<LibraryCategory> {
        crate::library::subjects(entries, &*self.added.lock().await)
    }

    /// Adds `topic` to `category`, one of the existing ones (whatever the case it is typed in) or
    /// a new one with `look`, and returns every category.
    ///
    /// # Errors
    /// Fails if a name is empty or too long, the topic already exists, a new category has no
    /// look or one another category has, or the file cannot be written.
    pub async fn add(
        &self,
        entries: &[LibraryEntry],
        category: &str,
        topic: &str,
        look: Option<CategoryLook>,
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
        let mut added = self.added.lock().await;
        let same = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
        let existing = crate::library::subjects(entries, &added);
        let name = if let Some(found) = existing.iter().find(|c| same(&c.name, category)) {
            if let Some(known) = found.topics.iter().find(|t| same(&t.name, topic)) {
                anyhow::bail!("{} already has the topic {}.", found.name, known.name);
            }
            found.name.clone()
        } else {
            let look = look.context("Pick an icon and a colour for the new category.")?;
            anyhow::ensure!(
                CATEGORY_ICONS.contains(&look.icon.as_str())
                    && CATEGORY_COLORS.contains(&look.color.as_str()),
                "Pick one of the icons and colours offered."
            );
            if let Some(other) = existing.iter().find(|c| c.look.icon == look.icon) {
                anyhow::bail!("{} already has this icon.", other.name);
            }
            if let Some(other) = existing.iter().find(|c| c.look.color == look.color) {
                anyhow::bail!("{} already has this colour.", other.name);
            }
            added.categories.push(AddedCategory {
                name: category.to_owned(),
                look,
            });
            category.to_owned()
        };
        added.topics.push(AddedTopic {
            category: name,
            topic: topic.to_owned(),
        });
        write(&self.path, &*added).await?;
        Ok(crate::library::subjects(entries, &added))
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
        let mut added = self.added.lock().await;
        let index = added
            .topics
            .iter()
            .position(|t| t.category == category && t.topic == topic)
            .context("Only topics added on this server can be removed.")?;
        added.topics.remove(index);
        if !added.topics.iter().any(|t| t.category == category) {
            added.categories.retain(|c| c.name != category);
        }
        write(&self.path, &*added).await?;
        Ok(crate::library::subjects(entries, &added))
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
        file(&mut entry, edit.question.clone());
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
        let added = dir.join("library-added.json");
        let original = entries()[0].clone();
        let mut question = original.question.clone();
        question.text = "Edited?".into();
        question.image = Some("picture.webp".into());
        let edit = LibraryEdit {
            difficulty: 3,
            question,
        };

        let edits = LibraryEdits::open(path.clone(), added.clone())
            .await
            .unwrap();
        let edited = edits.save(&original.id, edit.clone()).await.unwrap();
        assert!(edited.edited && edited.difficulty == 3 && edited.id == original.id);
        assert!(edits.save("nope", edit).await.is_err());

        // A restart reads the edits back; the file says where each question came from.
        let saved = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(saved.contains(&format!(
            "\"text\": {}",
            serde_json::to_string(&original.question.text).unwrap()
        )));
        let edits = LibraryEdits::open(path, added).await.unwrap();
        assert_eq!(edits.entries().await[0].question.text, "Edited?");
        assert_eq!(edits.media().await, ["picture.webp"]);

        assert_eq!(edits.revert(&original.id).await.unwrap(), original);
        assert_eq!(edits.entries().await[0], original);
        assert_eq!(edits.media().await, Vec::<String>::new());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn questions_are_added_moved_and_deleted() {
        let dir = std::env::temp_dir().join(format!("quaesito-library-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let (path, added) = (
            dir.join("library-edits.json"),
            dir.join("library-added.json"),
        );
        let edits = LibraryEdits::open(path.clone(), added.clone())
            .await
            .unwrap();
        let mut question = Question::blank();
        question.text = "Mine?".into();
        question.time_limit_secs = Some(5);
        question.audio = Some("tune.ogg".into());
        assert!(edits.add(2, question.clone()).await.is_err());

        question.category = "Music".into();
        question.topic = "Mine".into();
        let entry = edits.add(2, question.clone()).await.unwrap();
        assert!(entry.added && entry.category == "Music" && entry.topic == "Mine");
        // Kept without what only a quiz has.
        assert!(entry.question.category.is_empty() && entry.question.time_limit_secs.is_none());
        assert_eq!(edits.entries().await.last(), Some(&entry));
        assert_eq!(edits.media().await, ["tune.ogg"]);
        assert!(edits.revert(&entry.id).await.is_err());

        // Saving moves it, and a restart reads it back.
        question.topic = "Ours".into();
        let edit = LibraryEdit {
            difficulty: 3,
            question,
        };
        let moved = edits.save(&entry.id, edit).await.unwrap();
        assert_eq!(
            (moved.id.as_str(), moved.topic.as_str()),
            (entry.id.as_str(), "Ours")
        );
        let edits = LibraryEdits::open(path.clone(), added.clone())
            .await
            .unwrap();
        assert_eq!(edits.entries().await.last(), Some(&moved));

        // A bundled question moves too.
        let original = entries()[0].clone();
        let mut question = original.to_question();
        question.topic = "Elsewhere".into();
        let edit = LibraryEdit {
            difficulty: 1,
            question,
        };
        let edited = edits.save(&original.id, edit).await.unwrap();
        assert!(edited.edited && edited.topic == "Elsewhere" && edited.question.topic.is_empty());

        assert!(edits.delete(&original.id).await.is_err());
        edits.delete(&entry.id).await.unwrap();
        assert_eq!(edits.entries().await.len(), entries().len());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[test]
    fn every_library_category_has_a_look() {
        for entry in entries() {
            assert!(
                CategoryLook::of_library(&entry.category).is_some(),
                "{}",
                entry.category
            );
        }
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
            .add(entries, &lower, " Odds and ends ", None)
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
        assert!(
            topics
                .add(entries, &known.category, &upper, None)
                .await
                .is_err()
        );
        assert!(topics.add(entries, "Board games", " ", None).await.is_err());

        // A new category needs a look of its own, among those offered.
        let look = |icon: &str, color: &str| {
            Some(CategoryLook {
                icon: icon.into(),
                color: color.into(),
            })
        };
        let taken = CategoryLook::of_library(&known.category).unwrap();
        for refused in [
            None,
            look("grid", &taken.color),
            look(&taken.icon, "#f43f5e"),
            look("grid", "red"),
        ] {
            assert!(
                topics
                    .add(entries, "Board games", "Chess", refused)
                    .await
                    .is_err()
            );
        }
        let found = topics
            .add(entries, "Board games", "Chess", look("grid", "#f43f5e"))
            .await
            .unwrap();
        let board = found.iter().find(|c| c.name == "Board games").unwrap();
        assert_eq!((board.count, board.look.icon.as_str()), (0, "grid"));
        assert!(
            topics
                .add(entries, "Cards", "Poker", look("hash", "#f43f5e"))
                .await
                .is_err()
        );
        topics
            .add(entries, "board games", "Go", None)
            .await
            .unwrap();

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
        assert!(
            found
                .iter()
                .any(|c| c.name == "Board games" && c.look.icon == "grid")
        );
        let found = topics.remove(entries, "Board games", "Go").await.unwrap();
        assert!(!found.iter().any(|c| c.name == "Board games"));
        assert_eq!(found.len(), topics.subjects(entries).await.len());
        // Its look is free again.
        topics
            .add(entries, "Cards", "Poker", look("grid", "#f43f5e"))
            .await
            .unwrap();
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
