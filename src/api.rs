//! Server functions callable from the browser.
//!
//! Bodies run on the server only; the client side gets generated HTTP calls.

#![allow(
    clippy::unused_async,
    reason = "server functions must be async even when the server side does not await"
)]

use dioxus::{fullstack::FileStream, prelude::*};

use crate::model::{Quiz, QuizSummary, ValidationIssue};

/// Exchanges the admin password for a session token.
#[post("/api/admin/login")]
pub async fn admin_login(password: String) -> ServerFnResult<String> {
    crate::server::state()
        .auth
        .login(&password)
        .ok_or_else(|| ServerFnError::new("Wrong password."))
}

/// Whether an admin token is still valid (tokens do not survive restarts).
#[post("/api/admin/check")]
pub async fn admin_check(token: String) -> ServerFnResult<bool> {
    Ok(crate::server::state().auth.is_valid(&token))
}

/// All quizzes, sorted by title.
#[post("/api/admin/quizzes")]
pub async fn list_quizzes(token: String) -> ServerFnResult<Vec<QuizSummary>> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    Ok(state.quizzes.list().await?)
}

/// One quiz, for editing.
#[post("/api/admin/quiz/get")]
pub async fn get_quiz(token: String, id: String) -> ServerFnResult<Quiz> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    state
        .quizzes
        .get(&id)
        .await?
        .ok_or_else(|| ServerFnError::new("This quiz does not exist."))
}

/// Creates an empty quiz and returns its id.
#[post("/api/admin/quiz/create")]
pub async fn create_quiz(token: String, title: String) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let quiz = Quiz::new(crate::server::store::new_id(), title.trim().to_owned());
    state.quizzes.save(&quiz).await?;
    Ok(quiz.id)
}

/// Saves a quiz (drafts may be incomplete) and returns what still keeps it from being playable.
#[post("/api/admin/quiz/save")]
pub async fn save_quiz(token: String, quiz: Quiz) -> ServerFnResult<Vec<ValidationIssue>> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    if state.quizzes.get(&quiz.id).await?.is_none() {
        return Err(ServerFnError::new("This quiz does not exist anymore."));
    }
    state.quizzes.save(&quiz).await?;
    Ok(quiz.validate())
}

/// Deletes a quiz.
#[post("/api/admin/quiz/delete")]
pub async fn delete_quiz(token: String, id: String) -> ServerFnResult<()> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    state.quizzes.delete(&id).await?;
    Ok(())
}

/// Copies a quiz under a new id and returns that id.
#[post("/api/admin/quiz/duplicate")]
pub async fn duplicate_quiz(token: String, id: String) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let mut quiz = state
        .quizzes
        .get(&id)
        .await?
        .ok_or_else(|| ServerFnError::new("This quiz does not exist."))?;
    quiz.id = crate::server::store::new_id();
    quiz.title = format!("{} (copy)", quiz.title);
    state.quizzes.save(&quiz).await?;
    Ok(quiz.id)
}

/// Imports a quiz exported as JSON, under a new id, and returns that id.
#[post("/api/admin/quiz/import")]
pub async fn import_quiz(token: String, json: String) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let mut quiz: Quiz = serde_json::from_str(&json)
        .map_err(|err| ServerFnError::new(format!("Not a valid quiz file: {err}")))?;
    quiz.id = crate::server::store::new_id();
    state.quizzes.save(&quiz).await?;
    Ok(quiz.id)
}

/// Uploads a question picture and returns its media name.
#[post("/api/admin/media?token")]
pub async fn upload_question_image(token: String, file: FileStream) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let bytes = crate::server::media::read_upload(file).await?;
    Ok(state
        .media
        .store(bytes, crate::server::media::ImageKind::Question)
        .await?)
}
