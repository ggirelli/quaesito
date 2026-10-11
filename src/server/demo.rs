//! The demo quiz, added the first time the server starts so there is an example to play and to
//! open in the editor. It is offered once: deleting it does not bring it back.

use std::path::Path;

use super::{
    media::{ImageKind, MediaStore},
    store::{QuizStore, new_id},
};
use crate::model::Quiz;

const QUIZ: &str = include_str!("demo-quiz.json");
/// The quiz's one picture, named `lobby.jpg` in the quiz.
const PICTURE: &[u8] = include_bytes!("demo-quiz.jpg");
/// Written to the data directory once the demo quiz has been offered.
const MARKER: &str = "demo-quiz-added";

/// Adds the demo quiz if it has never been offered and there are no quizzes yet.
///
/// # Errors
/// Fails if the quiz or its picture cannot be stored.
pub async fn add_once(
    data_dir: &Path,
    quizzes: &QuizStore,
    media: &MediaStore,
) -> anyhow::Result<()> {
    let marker = data_dir.join(MARKER);
    if tokio::fs::try_exists(&marker).await? {
        return Ok(());
    }
    if quizzes.list().await?.is_empty() {
        quizzes.save(&demo(media).await?).await?;
    }
    tokio::fs::write(
        &marker,
        "The demo quiz has been added once. Delete this file to have it added again.\n",
    )
    .await?;
    Ok(())
}

async fn demo(media: &MediaStore) -> anyhow::Result<Quiz> {
    let mut quiz: Quiz = serde_json::from_str(QUIZ)?;
    quiz.id = new_id();
    let picture = media.store(PICTURE.to_vec(), ImageKind::Question).await?;
    quiz.rename_media(|_| Some(picture.clone()));
    Ok(quiz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_demo_quiz_is_playable_and_added_once() {
        let dir = std::env::temp_dir().join(format!("quaesito-demo-{}", uuid::Uuid::new_v4()));
        let quizzes = QuizStore::open(dir.join("quizzes")).await.unwrap();
        let media = MediaStore::open(dir.join("media")).await.unwrap();

        add_once(&dir, &quizzes, &media).await.unwrap();
        let all = quizzes.all().await.unwrap();
        assert_eq!(all.len(), 1);
        let quiz = &all[0];
        assert_eq!(quiz.validate(), Vec::new());
        for name in quiz.media() {
            assert!(media.read_bytes(&name).await.is_some(), "{name} is missing");
        }

        // Deleted, it stays deleted.
        quizzes.delete(&quiz.id).await.unwrap();
        add_once(&dir, &quizzes, &media).await.unwrap();
        assert_eq!(quizzes.list().await.unwrap(), Vec::new());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn existing_quizzes_get_no_demo() {
        let dir = std::env::temp_dir().join(format!("quaesito-demo-{}", uuid::Uuid::new_v4()));
        let quizzes = QuizStore::open(dir.join("quizzes")).await.unwrap();
        let media = MediaStore::open(dir.join("media")).await.unwrap();
        quizzes
            .save(&Quiz::new(new_id(), "Mine".to_owned()))
            .await
            .unwrap();
        add_once(&dir, &quizzes, &media).await.unwrap();
        assert_eq!(quizzes.list().await.unwrap().len(), 1);
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}
