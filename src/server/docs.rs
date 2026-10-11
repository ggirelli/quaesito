//! The user guide at `/docs/`: the documentation site shipped next to the server, or the hosted one
//! when there is no local copy (as when developing).

use std::path::PathBuf;

use dioxus::fullstack::{
    axum::{
        Router,
        body::Body,
        extract::Request,
        http::{StatusCode, Uri},
        response::{IntoResponse, Redirect, Response},
    },
    routing::get,
};
use tower_http::services::ServeDir;

/// The hosted documentation.
const HOSTED: &str = "https://ggirelli.github.io/quaesito/";

/// Where the documentation site is looked for, unless `QUAESITO_DOCS_DIR` says: `docs` in the
/// downloads, `site` after `just docs-build` in the repository (whose `docs` holds the sources).
const LOCAL: [&str; 2] = ["docs", "site"];

/// Routes for `/docs` and everything under it.
pub fn routes() -> Router {
    let dir = local_dir();
    let shown = dir
        .as_ref()
        .map_or_else(|| HOSTED.to_owned(), |dir| dir.display().to_string());
    dioxus::logger::tracing::info!("user guide: {shown}");
    let serve = move |request: Request| serve(dir.clone(), request);
    Router::new()
        .route("/docs", get(|| async { Redirect::permanent("/docs/") }))
        .route("/docs/", get(serve.clone()))
        .route("/docs/{*path}", get(serve))
}

/// The built documentation site, if there is one.
fn local_dir() -> Option<PathBuf> {
    let candidates = match std::env::var_os("QUAESITO_DOCS_DIR") {
        Some(dir) => vec![PathBuf::from(dir)],
        None => LOCAL.iter().map(PathBuf::from).collect(),
    };
    candidates
        .into_iter()
        .find(|dir| dir.join("index.html").is_file())
}

async fn serve(dir: Option<PathBuf>, request: Request) -> Response {
    let path = request
        .uri()
        .path()
        .strip_prefix("/docs")
        .unwrap_or_default()
        .to_owned();
    let Some(dir) = dir else {
        let page = path.trim_start_matches('/');
        return Redirect::temporary(&format!("{HOSTED}{page}")).into_response();
    };
    let (mut parts, body) = request.into_parts();
    parts.uri = match parts.uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
    .parse()
    .unwrap_or_else(|_| Uri::from_static("/"));
    match ServeDir::new(dir)
        .try_call(Request::from_parts(parts, body))
        .await
    {
        Ok(response) => response.map(Body::new),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
