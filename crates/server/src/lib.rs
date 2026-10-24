//! Quaesito's server: its state (quizzes, media, the question library, live sessions), admin
//! sign-in, WebSocket loops and HTTP routes. The game rules come from `quaesito-core`.

pub mod auth;
pub mod bundle;
pub mod demo;
pub mod docs;
pub mod library;
pub mod live;
pub mod media;
pub mod net;
pub mod peer;
pub mod sessions;
pub mod store;
pub mod throttle;

use std::{collections::HashSet, path::PathBuf, sync::OnceLock, time::Duration};

use dioxus::fullstack::{
    axum::{
        Router,
        body::Body,
        extract::{Path, Query},
        http::{Extensions, HeaderMap, HeaderValue, StatusCode, header},
        response::{IntoResponse, Response},
    },
    routing::{get, post},
};

use quaesito_core::model::{ImportOutcome, ImportProgress, Quiz};
use tower_http::set_header::SetResponseHeaderLayer;

use self::{
    auth::Auth, media::MediaStore, sessions::Sessions, store::QuizStore, throttle::Throttle,
};

/// A random number in `0..n` (`n` must not be 0).
#[must_use]
pub fn random_below(n: usize) -> usize {
    let random = uuid::Uuid::new_v4().as_u128();
    usize::try_from(random % n as u128).unwrap_or(0)
}

/// Everything the server keeps for its lifetime.
pub struct AppState {
    pub auth: Auth,
    pub quizzes: QuizStore,
    pub media: MediaStore,
    pub sessions: Sessions,
    /// This server's edits to the question library.
    pub library_edits: library::LibraryEdits,
    /// The categories and topics this server added to the question library.
    pub library_topics: library::AddedTopics,
    /// The largest uploads accepted.
    pub limits: quaesito_core::protocol::UploadLimits,
    /// Wrong admin passwords.
    pub logins: Throttle,
    /// Join codes asked for that no game has.
    pub join_codes: Throttle,
}

/// Wrong admin passwords allowed per client and [`GUESS_WINDOW`].
const MAX_WRONG_PASSWORDS: u32 = 10;
/// Unknown join codes allowed per client and [`GUESS_WINDOW`]: a mistyped code is rare, while
/// finding a live game among the million codes takes hundreds of thousands of tries.
const MAX_UNKNOWN_CODES: u32 = 30;
const GUESS_WINDOW: Duration = Duration::from_mins(5);

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
        library_edits: library::LibraryEdits::open(
            data_dir.join("library-edits.json"),
            data_dir.join("library-added.json"),
        )
        .await?,
        library_topics: library::AddedTopics::open(data_dir.join("library-topics.json")).await?,
        limits: media::limits_from_env(),
        logins: Throttle::new(MAX_WRONG_PASSWORDS, GUESS_WINDOW),
        join_codes: Throttle::new(MAX_UNKNOWN_CODES, GUESS_WINDOW),
    };
    if let Err(err) = demo::add_once(&data_dir, &state.quizzes, &state.media).await {
        dioxus::logger::tracing::warn!("adding the demo quiz: {err:#}");
    }
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
        net::join_origin("", &port)
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

/// Deletes uploads no quiz, live game or edited or added library question uses, now and then daily.
async fn clean_up_media() {
    loop {
        let state = state();
        match state.quizzes.all().await {
            Ok(quizzes) => {
                let used: HashSet<String> = quizzes
                    .iter()
                    .flat_map(Quiz::media)
                    .chain(state.sessions.media())
                    .chain(state.library_edits.media().await)
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

/// Only quaesito itself may serve what its pages load (scripts, styles, fonts, pictures, sounds,
/// connections): the app works without internet and sends nothing to third parties. Inline code
/// and `eval` stay allowed for Dioxus and the user guide. Only quaesito's own pages may show its
/// pages in a frame (as the test game does), so another site cannot trick an admin into clicks.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self' 'unsafe-inline' 'unsafe-eval' \
    'wasm-unsafe-eval' data: blob:; connect-src 'self' ws: wss:; frame-ancestors 'self'";

/// Adds the content security policy to every response, so a browser refuses third-party requests
/// even from code quaesito does not control (such as the font dx's rebuild notice loads in debug
/// builds).
#[must_use]
pub fn offline_policy() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    )
}

/// Browser protections for every response: the [content security policy](offline_policy),
/// files used only as the type they are served as, and other sites not told which quaesito page
/// linked to them (a phone's join address holds its team's ticket).
pub fn protect(router: Router) -> Router {
    router
        .layer(offline_policy())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
}

/// Serves the application from `app` on `IP`:`PORT` (default `127.0.0.1:8080`), as
/// `dioxus::serve` does in release builds, but with each connection's address for
/// [`peer::Peer`].
#[cfg(not(debug_assertions))]
pub fn serve<F>(app: impl FnOnce() -> F) -> !
where
    F: Future<Output = anyhow::Result<Router>>,
{
    dioxus::logger::initialize_default();
    let served = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(anyhow::Error::from)
        .and_then(|runtime| {
            runtime.block_on(async {
                let router = app().await?;
                let addr = dioxus::cli_config::fullstack_address_or_localhost();
                let listener = tokio::net::TcpListener::bind(addr)
                    .await
                    .map_err(|err| listen_error(addr, &err))?;
                dioxus::fullstack::axum::serve(
                    listener,
                    router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await?;
                Ok(())
            })
        });
    if let Err(err) = served {
        dioxus::logger::tracing::error!("{err:#}");
    }
    std::process::exit(1)
}

/// Why quaesito cannot listen on `addr`. A busy port is common (often an earlier quaesito still
/// running) and each system words it differently, so it also says what to do.
#[cfg(any(test, not(debug_assertions)))]
fn listen_error(addr: std::net::SocketAddr, err: &std::io::Error) -> anyhow::Error {
    if err.kind() != std::io::ErrorKind::AddrInUse {
        return anyhow::anyhow!("listening on {addr}: {err}");
    }
    let port = addr.port();
    let other = port.checked_add(1).unwrap_or(8081);
    anyhow::anyhow!(
        "listening on {addr}: {err}. Another program, perhaps another quaesito window, already \
         uses port {port}: close it, or start quaesito on another port with PORT={other} (in a \
         Windows Command Prompt: set PORT={other}, then start.bat)"
    )
}

/// Routes served next to the Dioxus application.
pub fn routes() -> Router {
    Router::new()
        .route("/media/{name}", get(media::serve))
        .route("/export/{id}", get(export))
        .route("/import", post(import))
        .merge(docs::routes())
}

#[derive(serde::Deserialize)]
struct ImportQuery {
    token: String,
    /// Import a quiz from a newer version anyway, without what this version cannot read.
    #[serde(default)]
    consent: bool,
}

/// `POST /import?token=…&consent=…`, with an exported quiz (zip bundle with media, or plain
/// JSON) as the body: imports it under a new id. The answer streams one JSON
/// [`ImportProgress`] per line, so the admin page can follow the processing of the media.
/// Like other uploads, only from this machine.
async fn import(
    extensions: Extensions,
    headers: HeaderMap,
    Query(query): Query<ImportQuery>,
    body: Body,
) -> Response {
    let state = state();
    if !state.auth.is_valid(&query.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let peer = peer::Peer::of(&extensions, &headers);
    if !peer.is_this_machine() {
        return (StatusCode::FORBIDDEN, peer.refusal()).into_response();
    }
    let bytes = match media::read_upload(body.into_data_stream(), state.limits.import).await {
        Ok(bytes) => bytes,
        Err(err) => return (StatusCode::BAD_REQUEST, format!("{err:#}")).into_response(),
    };
    let (progress, lines) = tokio::sync::mpsc::unbounded_channel();
    // The import finishes even if the page stops listening.
    tokio::spawn(async move {
        let media_progress = progress.clone();
        let imported = bundle::import(bytes, &state.media, &state.limits, query.consent, {
            move |done, total| {
                let _ = media_progress.send(ImportProgress::Media { done, total });
            }
        })
        .await;
        let end = match imported {
            Ok(bundle::Imported::Quiz(mut quiz)) => {
                quiz.id = store::new_id();
                // A copy is new: it is created now.
                quiz.created = None;
                match state.quizzes.save(&quiz).await {
                    Ok(()) => ImportProgress::Done(ImportOutcome::Imported(quiz.id)),
                    Err(err) => ImportProgress::Failed(format!("{err:#}")),
                }
            }
            Ok(bundle::Imported::NeedsConsent(warning)) => {
                ImportProgress::Done(ImportOutcome::NeedsConsent(warning))
            }
            Err(err) => ImportProgress::Failed(format!("{err:#}")),
        };
        let _ = progress.send(end);
    });
    let lines = futures_util::stream::unfold(lines, |mut lines| async move {
        let line = lines.recv().await?;
        let mut json = serde_json::to_vec(&line).unwrap_or_default();
        json.push(b'\n');
        Some((Ok::<_, std::convert::Infallible>(json), lines))
    });
    (
        [(header::CONTENT_TYPE, "application/x-ndjson")],
        Body::from_stream(lines),
    )
        .into_response()
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

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::listen_error;

    #[test]
    fn a_busy_port_says_what_to_do() {
        let addr = "0.0.0.0:8080".parse().unwrap();
        let busy = listen_error(addr, &Error::from(ErrorKind::AddrInUse)).to_string();
        assert!(busy.starts_with("listening on 0.0.0.0:8080: "), "{busy}");
        assert!(busy.contains("uses port 8080"), "{busy}");
        assert!(busy.contains("PORT=8081"), "{busy}");

        let denied = listen_error(addr, &Error::from(ErrorKind::PermissionDenied)).to_string();
        assert!(!denied.contains("PORT="), "{denied}");
    }
}
