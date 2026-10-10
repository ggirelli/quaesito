//! Server functions callable from the browser.
//!
//! Bodies run on the server only; the client side gets generated HTTP calls.

#![allow(
    clippy::unused_async,
    reason = "server functions must be async even when the server side does not await"
)]

use dioxus::{
    fullstack::{FileStream, WebSocketOptions, Websocket},
    prelude::*,
};

use crate::{
    model::{Quiz, QuizSummary, ValidationIssue},
    protocol::{
        GameResults, HostCommand, HostView, JoinTicket, PlayerCommand, PlayerView, SessionSummary,
    },
};

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
    let bytes =
        crate::server::media::read_upload(file, crate::server::media::MAX_IMAGE_BYTES).await?;
    Ok(state
        .media
        .store(bytes, crate::server::media::ImageKind::Question)
        .await?)
}

/// Uploads a question soundtrack and returns its media name.
#[post("/api/admin/audio?token")]
pub async fn upload_question_audio(token: String, file: FileStream) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let bytes =
        crate::server::media::read_upload(file, crate::server::media::MAX_AUDIO_BYTES).await?;
    Ok(state.media.store_audio(bytes).await?)
}

/// Starts a live session for a quiz and returns its join code.
#[post("/api/admin/session/start")]
pub async fn start_session(token: String, quiz_id: String) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let quiz = state
        .quizzes
        .get(&quiz_id)
        .await?
        .ok_or_else(|| ServerFnError::new("This quiz does not exist."))?;
    state
        .sessions
        .create(quiz)
        .map_err(|refusal| ServerFnError::new(refusal.0))
}

/// Live sessions.
#[post("/api/admin/sessions")]
pub async fn list_sessions(token: String) -> ServerFnResult<Vec<SessionSummary>> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    Ok(state.sessions.list())
}

/// Every team's points on every question of a live session.
#[post("/api/admin/session/results")]
pub async fn game_results(token: String, code: String) -> ServerFnResult<GameResults> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let session = state
        .sessions
        .get(&code)
        .ok_or_else(|| ServerFnError::new("This game is not running anymore."))?;
    Ok(session.read(crate::server::engine::Game::results))
}

/// Ends a live session.
#[post("/api/admin/session/end")]
pub async fn end_session(token: String, code: String) -> ServerFnResult<()> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    state.sessions.remove(&code);
    Ok(())
}

/// Base URL phones should use to join, given the port the host page was opened on.
#[post("/api/admin/join-origin")]
pub async fn join_origin(token: String, page_port: String) -> ServerFnResult<String> {
    crate::server::state().auth.require(&token)?;
    Ok(crate::server::net::join_origin(&page_port))
}

/// Title of the quiz behind a join code, or `None` if no such session is live.
#[post("/api/game/info")]
pub async fn game_info(code: String) -> ServerFnResult<Option<String>> {
    Ok(crate::server::state()
        .sessions
        .get(&code)
        .map(|s| s.read(|g| g.title().to_owned())))
}

/// Registers a team in a session.
#[post("/api/game/join")]
pub async fn join_game(code: String, name: String) -> ServerFnResult<JoinTicket> {
    let session = crate::server::state()
        .sessions
        .get(&code)
        .ok_or_else(|| ServerFnError::new("No game with this code is running."))?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let team_id = session
        .update(|g, _| g.join(&name, token.clone()))
        .map_err(|refusal| ServerFnError::new(refusal.0))?;
    Ok(JoinTicket { team_id, token })
}

/// Whether a stored ticket still belongs to a team in the session.
#[post("/api/game/check-ticket")]
pub async fn check_ticket(code: String, token: String) -> ServerFnResult<bool> {
    Ok(crate::server::state()
        .sessions
        .get(&code)
        .is_some_and(|s| s.read(|g| g.team_for_token(&token).is_some())))
}

/// Uploads a team avatar and returns its media name.
#[post("/api/game/avatar?code&token")]
pub async fn upload_avatar(
    code: String,
    token: String,
    file: FileStream,
) -> ServerFnResult<String> {
    let state = crate::server::state();
    let session = state
        .sessions
        .get(&code)
        .ok_or_else(|| ServerFnError::new("No game with this code is running."))?;
    let team = session
        .read(|g| g.team_for_token(&token))
        .ok_or_else(|| ServerFnError::new("Unknown team."))?;
    let bytes =
        crate::server::media::read_upload(file, crate::server::media::MAX_IMAGE_BYTES).await?;
    let name = state
        .media
        .store(bytes, crate::server::media::ImageKind::Avatar)
        .await?;
    session.update(|g, _| g.set_avatar(team, name.clone()));
    Ok(name)
}

/// Live updates for the host screen.
#[get("/api/ws/host?code&token")]
pub async fn host_socket(
    code: String,
    token: String,
    options: WebSocketOptions,
) -> ServerFnResult<Websocket<HostCommand, HostView>> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let session = state
        .sessions
        .get(&code)
        .ok_or_else(|| ServerFnError::new("This game is not running anymore."))?;
    Ok(options.on_upgrade(move |socket| crate::server::live::host(socket, session)))
}

/// Live updates for a team leader's phone.
#[get("/api/ws/player?code&token")]
pub async fn player_socket(
    code: String,
    token: String,
    options: WebSocketOptions,
) -> ServerFnResult<Websocket<PlayerCommand, PlayerView>> {
    let session = crate::server::state()
        .sessions
        .get(&code)
        .ok_or_else(|| ServerFnError::new("This game is not running anymore."))?;
    let team = session
        .read(|g| g.team_for_token(&token))
        .ok_or_else(|| ServerFnError::new("Unknown team."))?;
    Ok(options.on_upgrade(move |socket| crate::server::live::player(socket, session, team)))
}
