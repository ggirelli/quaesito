//! Quizzes persisted as one JSON file each.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::model::{Quiz, QuizSummary};

/// Directory of `<id>.json` quiz documents.
pub struct QuizStore {
    dir: PathBuf,
}

impl QuizStore {
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

    /// Summaries of all stored quizzes, sorted by title. Unreadable files are skipped.
    ///
    /// # Errors
    /// Fails if the directory cannot be listed.
    pub async fn list(&self) -> anyhow::Result<Vec<QuizSummary>> {
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        let mut summaries = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match read(&path).await {
                Ok(quiz) => summaries.push(quiz.summary()),
                Err(err) => dioxus::logger::tracing::warn!("skipping {}: {err:#}", path.display()),
            }
        }
        summaries.sort_by_key(|s| s.title.to_lowercase());
        Ok(summaries)
    }

    /// Every stored quiz. Unreadable files are skipped.
    ///
    /// # Errors
    /// Fails if the directory cannot be listed.
    pub async fn all(&self) -> anyhow::Result<Vec<Quiz>> {
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        let mut quizzes = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(quiz) = read(&path).await
            {
                quizzes.push(quiz);
            }
        }
        Ok(quizzes)
    }

    /// Loads one quiz, or `None` if it does not exist.
    ///
    /// # Errors
    /// Fails if the file exists but cannot be read or parsed.
    pub async fn get(&self, id: &str) -> anyhow::Result<Option<Quiz>> {
        let Some(path) = self.path(id) else {
            return Ok(None);
        };
        if !tokio::fs::try_exists(&path).await? {
            return Ok(None);
        }
        read(&path).await.map(Some)
    }

    /// Writes a quiz atomically, replacing any previous version.
    ///
    /// # Errors
    /// Fails on an invalid id or an I/O error.
    pub async fn save(&self, quiz: &Quiz) -> anyhow::Result<()> {
        let path = self.path(&quiz.id).context("invalid quiz id")?;
        let tmp = path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(quiz)?).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    /// Deletes a quiz. Returns whether it existed.
    ///
    /// # Errors
    /// Fails on an I/O error other than the file being absent.
    pub async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let Some(path) = self.path(id) else {
            return Ok(false);
        };
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        is_valid_id(id).then(|| self.dir.join(format!("{id}.json")))
    }
}

/// A fresh random quiz id.
#[must_use]
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Ids are 32 lowercase hex digits, which keeps them safe to use as file names.
fn is_valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

async fn read(path: &Path) -> anyhow::Result<Quiz> {
    let bytes = tokio::fs::read(path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("quaesito-test-{}", new_id()))
    }

    #[tokio::test]
    async fn roundtrip() {
        let dir = temp_dir();
        let store = QuizStore::open(&dir).await.unwrap();
        let quiz = Quiz::new(new_id(), "Zebra".to_owned());
        let other = Quiz::new(new_id(), "apple".to_owned());
        store.save(&quiz).await.unwrap();
        store.save(&other).await.unwrap();

        assert_eq!(store.get(&quiz.id).await.unwrap(), Some(quiz.clone()));
        let titles: Vec<_> = store
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(titles, vec!["apple", "Zebra"]);

        assert!(store.delete(&quiz.id).await.unwrap());
        assert!(!store.delete(&quiz.id).await.unwrap());
        assert_eq!(store.get(&quiz.id).await.unwrap(), None);
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_path_like_ids() {
        let store = QuizStore::open(temp_dir()).await.unwrap();
        assert_eq!(store.get("../etc/passwd").await.unwrap(), None);
        let quiz = Quiz::new("../x".to_owned(), "T".to_owned());
        assert!(store.save(&quiz).await.is_err());
        tokio::fs::remove_dir_all(store.dir).await.unwrap();
    }
}
