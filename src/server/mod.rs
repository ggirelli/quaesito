//! Server-only state and HTTP routes.

pub mod auth;
pub mod bundle;
pub mod engine;
pub mod live;
pub mod matching;
pub mod media;
pub mod net;
pub mod sessions;
pub mod store;

use std::{collections::HashSet, path::PathBuf, sync::OnceLock, time::Duration};

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
use crate::model::Quiz;

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
    if STATE.set(state).is_ok() {
        tokio::spawn(clean_up_media());
        #[cfg(not(debug_assertions))]
        print_banner(&self::state().auth);
    }
    Ok(())
}

/// Tells whoever started a release build where to go and how to log in, above the log lines.
#[cfg(not(debug_assertions))]
fn print_banner(auth: &Auth) {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_owned());
    let ip = std::env::var("IP").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let phones = if ip.starts_with("127.") || ip == "localhost" {
        format!("not reachable: listening on {ip} only (set IP=0.0.0.0)")
    } else {
        net::join_origin(&port)
    };
    let password = auth.generated_password().map_or_else(
        || "the one in QUAESITO_ADMIN_PASSWORD".to_owned(),
        |p| format!("{p}   (set QUAESITO_ADMIN_PASSWORD to choose your own)"),
    );
    let line = "─".repeat(72);
    println!(
        "\n{line}\n  quaesito is running\n\n  On this computer:  http://localhost:{port}\n  \
         Phones join at:    {phones}\n  Admin password:    {password}\n{line}\n"
    );
}

/// How long an upload may stay unused before it is deleted (it may belong to an editor that has
/// not saved yet).
const MEDIA_GRACE: Duration = Duration::from_hours(24);
/// How often unused uploads are looked for.
const MEDIA_CLEANUP_EVERY: Duration = Duration::from_hours(24);

/// Deletes uploads no quiz or live game uses, now and then daily.
async fn clean_up_media() {
    loop {
        let state = state();
        match state.quizzes.all().await {
            Ok(quizzes) => {
                let used: HashSet<String> = quizzes
                    .iter()
                    .flat_map(Quiz::media)
                    .chain(state.sessions.media())
                    .collect();
                match state.media.collect_garbage(&used, MEDIA_GRACE).await {
                    Ok(0) => {}
                    Ok(n) => dioxus::logger::tracing::info!("deleted {n} unused uploads"),
                    Err(err) => dioxus::logger::tracing::warn!("cleaning up uploads: {err:#}"),
                }
            }
            Err(err) => dioxus::logger::tracing::warn!("cleaning up uploads: {err:#}"),
        }
        tokio::time::sleep(MEDIA_CLEANUP_EVERY).await;
    }
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

/// `GET /export/{id}?token=…`: downloads a quiz with its media as a zip bundle.
async fn export(Path(id): Path<String>, Query(query): Query<ExportQuery>) -> Response {
    let state = state();
    if !state.auth.is_valid(&query.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(quiz)) = state.quizzes.get(&id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bundle) = bundle::export(&quiz, &state.media).await else {
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
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{file_name}.zip\""),
            ),
        ],
        bundle,
    )
        .into_response()
}
