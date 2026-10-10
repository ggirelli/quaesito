//! Server-only state and HTTP routes.

pub mod auth;
pub mod engine;
pub mod live;
pub mod matching;
pub mod media;
pub mod net;
pub mod sessions;
pub mod store;

use std::{path::PathBuf, sync::OnceLock};

use dioxus::fullstack::{
    axum::{
        Router,
        extract::{Path, Query},
        http::{StatusCode, header},
        response::{IntoResponse, Response},
    },
    routing::get,
};

use self::{auth::Auth, media::MediaStore, sessions::Sessions, store::QuizStore};

/// Everything the server keeps for its lifetime.
pub struct AppState {
    pub auth: Auth,
    pub quizzes: QuizStore,
    pub media: MediaStore,
    pub sessions: Sessions,
}

static STATE: OnceLock<AppState> = OnceLock::new();

/// The global server state.
///
/// # Panics
/// Panics if called before [`init`].
pub fn state() -> &'static AppState {
    STATE.get().expect("server state not initialised")
}

/// Initialises the global server state from the environment. Safe to call repeatedly.
///
/// # Errors
/// Fails if the data directories cannot be created.
pub async fn init() -> anyhow::Result<()> {
    if STATE.get().is_some() {
        return Ok(());
    }
    let data_dir =
        std::env::var_os("QUAESITO_DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    let state = AppState {
        auth: Auth::from_env(data_dir.join("admin-tokens.json")),
        quizzes: QuizStore::open(data_dir.join("quizzes")).await?,
        media: MediaStore::open(data_dir.join("media")).await?,
        sessions: Sessions::open(data_dir.join("games")).await?,
    };
    dioxus::logger::tracing::info!("data directory: {}", data_dir.display());
    // A concurrent initialisation may have won the race; either state is equivalent.
    let _ = STATE.set(state);
    Ok(())
}

/// Routes served next to the Dioxus application.
pub fn routes() -> Router {
    Router::new()
        .route("/media/{name}", get(media::serve))
        .route("/export/{id}", get(export))
}

#[derive(serde::Deserialize)]
struct ExportQuery {
    token: String,
}

/// `GET /export/{id}?token=…`: downloads a quiz as JSON.
async fn export(Path(id): Path<String>, Query(query): Query<ExportQuery>) -> Response {
    let state = state();
    if !state.auth.is_valid(&query.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(quiz)) = state.quizzes.get(&id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(json) = serde_json::to_string_pretty(&quiz) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let file_name: String = quiz
        .title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let file_name = file_name.trim_matches('-');
    let file_name = if file_name.is_empty() {
        "quiz"
    } else {
        file_name
    };
    (
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{file_name}.json\""),
            ),
        ],
        json,
    )
        .into_response()
}
