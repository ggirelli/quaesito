//! Server-only state and HTTP routes.

pub mod media;
pub mod store;

use std::{path::PathBuf, sync::OnceLock};

use dioxus::fullstack::{axum::Router, routing::get};

use self::{media::MediaStore, store::QuizStore};

/// Everything the server keeps for its lifetime.
pub struct AppState {
    pub quizzes: QuizStore,
    pub media: MediaStore,
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
        quizzes: QuizStore::open(data_dir.join("quizzes")).await?,
        media: MediaStore::open(data_dir.join("media")).await?,
    };
    dioxus::logger::tracing::info!("data directory: {}", data_dir.display());
    // A concurrent initialisation may have won the race; either state is equivalent.
    let _ = STATE.set(state);
    Ok(())
}

/// Routes served next to the Dioxus application.
pub fn routes() -> Router {
    Router::new().route("/media/{name}", get(media::serve))
}
