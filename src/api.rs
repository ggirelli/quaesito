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
    library::{LibraryEdit, LibraryEntry},
    model::{Quiz, QuizSummary, ValidationIssue},
    protocol::{
        GameResults, HostCommand, HostView, JoinTicket, PlayerCommand, PlayerView, SessionSummary,
        TestInfo, UploadLimits,
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
    // A copy is new: it is created now.
    quiz.created = None;
    quiz.title = format!("{} (copy)", quiz.title);
    state.quizzes.save(&quiz).await?;
    Ok(quiz.id)
}

/// Every question of the library, with this server's edits, to browse and pick from.
#[post("/api/admin/library")]
pub async fn question_library(token: String) -> ServerFnResult<Vec<LibraryEntry>> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    Ok(state.library_edits.entries().await)
}

/// Saves this server's edit of library question `id`, if it is playable, and returns it.
#[post("/api/admin/library/edit")]
pub async fn edit_library_question(
    token: String,
    id: String,
    edit: LibraryEdit,
) -> ServerFnResult<LibraryEntry> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let mut quiz = Quiz::new(String::new(), "Library".to_owned());
    quiz.questions = vec![edit.question.clone()];
    if let Some(issue) = quiz.validate().into_iter().next() {
        return Err(ServerFnError::new(issue.message));
    }
    if !(1..=3).contains(&edit.difficulty) {
        return Err(ServerFnError::new("Pick a difficulty."));
    }
    Ok(state.library_edits.save(&id, edit).await?)
}

/// Drops this server's edit of library question `id` and returns the question as it came.
#[post("/api/admin/library/revert")]
pub async fn revert_library_question(token: String, id: String) -> ServerFnResult<LibraryEntry> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    Ok(state.library_edits.revert(&id).await?)
}

/// Uploads a picture (for an answer or a part) and returns its media name.
#[post("/api/admin/media?token")]
pub async fn upload_question_image(token: String, file: FileStream) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let bytes = crate::server::media::read_upload(file, state.limits.picture).await?;
    Ok(state
        .media
        .store(bytes, crate::server::media::ImageKind::Question)
        .await?)
}

/// Uploads a question's or slide's picture or video and returns its media name.
#[post("/api/admin/question-media?token")]
pub async fn upload_question_media(token: String, file: FileStream) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let limits = state.limits;
    let bytes = crate::server::media::read_upload(file, limits.picture.max(limits.video)).await?;
    let mb = crate::protocol::UploadLimits::megabytes;
    if crate::server::media::video_extension(&bytes).is_some() {
        if bytes.len() as u64 > limits.video {
            return Err(ServerFnError::new(format!(
                "The video is too large (max {} MB).",
                mb(limits.video)
            )));
        }
        return Ok(state.media.store_video(bytes).await?);
    }
    if bytes.len() as u64 > limits.picture {
        return Err(ServerFnError::new(format!(
            "The picture is too large (max {} MB).",
            mb(limits.picture)
        )));
    }
    Ok(state
        .media
        .store(bytes, crate::server::media::ImageKind::Question)
        .await?)
}

/// The largest uploads the server accepts, for the editor's hints and checks.
#[post("/api/admin/upload-limits")]
pub async fn upload_limits(token: String) -> ServerFnResult<UploadLimits> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    Ok(state.limits)
}

/// Uploads a question soundtrack and returns its media name.
#[post("/api/admin/audio?token")]
pub async fn upload_question_audio(token: String, file: FileStream) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let bytes = crate::server::media::read_upload(file, state.limits.audio).await?;
    Ok(state.media.store_audio(bytes).await?)
}

/// Starts a live session for a quiz and returns its join code. `max_teams` is the most teams
/// that can join ([`crate::protocol::DEFAULT_MAX_TEAMS`] when left out). With
/// `shuffle_answers`, the answers of each multiple-choice question are shuffled for this game
/// only: the saved quiz keeps its order.
#[post("/api/admin/session/start")]
pub async fn start_session(
    token: String,
    quiz_id: String,
    max_teams: Option<usize>,
    shuffle_answers: Option<bool>,
) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let mut quiz = state
        .quizzes
        .get(&quiz_id)
        .await?
        .ok_or_else(|| ServerFnError::new("This quiz does not exist."))?;
    if shuffle_answers == Some(true) {
        quiz.shuffle_answers(crate::server::engine::random_below);
    }
    state
        .sessions
        .create(
            quiz,
            max_teams.unwrap_or(crate::protocol::DEFAULT_MAX_TEAMS),
        )
        .map_err(|refusal| ServerFnError::new(refusal.0))
}

/// Phones a test game starts with.
#[cfg(feature = "server")]
const TEST_PHONES: usize = 2;
/// Most phones a test game can have.
#[cfg(feature = "server")]
const MAX_TEST_PHONES: usize = 6;

/// Starts an admin's test game, with phones already joined, and returns its join code. With
/// `from`, the test skips the lobby and starts at that question or slide (by position).
#[post("/api/admin/test/start")]
pub async fn start_test(
    token: String,
    quiz_id: String,
    from: Option<usize>,
) -> ServerFnResult<String> {
    let state = crate::server::state();
    state.auth.require(&token)?;
    let quiz = state
        .quizzes
        .get(&quiz_id)
        .await?
        .ok_or_else(|| ServerFnError::new("This quiz does not exist."))?;
    let code = state
        .sessions
        .create_test(quiz)
        .map_err(|refusal| ServerFnError::new(refusal.0))?;
    for _ in 0..TEST_PHONES {
        if let Err(err) = add_test_phone(token.clone(), code.clone()).await {
            // Do not leave a half-started test game behind.
            state.sessions.remove(&code);
            return Err(err);
        }
    }
    if let Some(from) = from {
        let started = state
            .sessions
            .get(&code)
            .map(|session| session.update(|g, now| g.start_from(from, now)));
        if let Some(Err(refusal)) = started {
            state.sessions.remove(&code);
            return Err(ServerFnError::new(refusal.0));
        }
    }
    Ok(code)
}

/// The test page's view of a test game: its quiz and the phones' tickets.
#[post("/api/admin/test/info")]
pub async fn test_info(token: String, code: String) -> ServerFnResult<TestInfo> {
    let session = test_session(&token, &code)?;
    Ok(session.read(|g| TestInfo {
        quiz_id: g.quiz_id().to_owned(),
        title: g.title().to_owned(),
        phones: g
            .tickets()
            .into_iter()
            .map(|(name, ticket)| crate::protocol::TestPhone { name, ticket })
            .collect(),
    }))
}

/// Adds a phone to a test game, named after the first free "Phone N".
#[post("/api/admin/test/add-phone")]
pub async fn add_test_phone(token: String, code: String) -> ServerFnResult<()> {
    let session = test_session(&token, &code)?;
    session
        .update(|g, _| {
            if g.team_count() >= MAX_TEST_PHONES {
                return Err(crate::server::engine::Refusal(
                    "That is enough phones for a test.",
                ));
            }
            let ticket = uuid::Uuid::new_v4().simple().to_string();
            // A free name always exists among MAX_TEST_PHONES + 1 candidates.
            (1..=MAX_TEST_PHONES + 1)
                .find_map(|n| g.join(&format!("Phone {n}"), ticket.clone()).ok())
                .map(|_| ())
                .ok_or(crate::server::engine::Refusal("Could not add a phone."))
        })
        .map_err(|refusal| ServerFnError::new(refusal.0))
}

/// Removes a phone from a test game.
#[post("/api/admin/test/remove-phone")]
pub async fn remove_test_phone(token: String, code: String, ticket: String) -> ServerFnResult<()> {
    let session = test_session(&token, &code)?;
    session
        .update(|g, now| match g.team_for_token(&ticket) {
            Some(team) => g.remove_team(team, now),
            None => Ok(()),
        })
        .map_err(|refusal| ServerFnError::new(refusal.0))
}

/// The test game behind `code`, for an admin.
#[cfg(feature = "server")]
fn test_session(
    token: &str,
    code: &str,
) -> Result<std::sync::Arc<crate::server::sessions::Session>, ServerFnError> {
    let state = crate::server::state();
    state.auth.require(token)?;
    state
        .sessions
        .get(code)
        .filter(|s| s.is_test())
        .ok_or_else(|| ServerFnError::new("This test is not running anymore."))
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
    let bytes = crate::server::media::read_upload(file, state.limits.picture).await?;
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
