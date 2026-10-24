//! The user guide at `/docs/`: the documentation site shipped next to the server, or the hosted one
//! when there is no local copy (as when developing).

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use dioxus::fullstack::{
    axum::{
        Router,
        body::Body,
        extract::Request,
        http::{HeaderValue, StatusCode, Uri, header},
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
    if dir.as_deref().is_some_and(is_stale) {
        dioxus::logger::tracing::warn!(
            "the user guide in {shown} is older than its sources: run `just docs-build`"
        );
    }
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

/// Whether a guide built from this repository (`site`, next to the `docs` sources and
/// `mkdocs.yml`) is older than the newest of its sources. A shipped guide has no sources.
fn is_stale(dir: &Path) -> bool {
    let sources = [PathBuf::from("docs"), PathBuf::from("mkdocs.yml")];
    if dir != Path::new(LOCAL[1]) || !sources.iter().all(|s| s.exists()) {
        return false;
    }
    let built = modified(&dir.join("index.html"));
    sources.iter().filter_map(|s| newest(s)).max() > built
}

/// When `path` was last changed, if it can be told.
fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The latest change to `path` or, for a folder, to anything inside it.
fn newest(path: &Path) -> Option<SystemTime> {
    let own = modified(path);
    let Ok(entries) = std::fs::read_dir(path) else {
        return own;
    };
    entries
        .flatten()
        .filter_map(|entry| newest(&entry.path()))
        .chain(own)
        .max()
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
        Ok(response) => {
            let mut response = response.map(Body::new);
            // Browsers check back every time (a cheap 304 when nothing changed), so a rebuilt
            // guide shows at once instead of a copy they guessed was still fresh.
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            response
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guide_is_stale_once_its_sources_change() {
        let root = std::env::temp_dir().join(format!("quaesito-docs-{}", uuid::Uuid::new_v4()));
        let sources = root.join("docs");
        std::fs::create_dir_all(sources.join("deep")).unwrap();
        std::fs::write(root.join("mkdocs.yml"), "site_name: x").unwrap();
        std::fs::write(sources.join("deep/page.md"), "old").unwrap();
        let site = root.join("site");
        std::fs::create_dir_all(&site).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(site.join("index.html"), "built").unwrap();
        let newest_source = newest(&root.join("docs")).unwrap();
        assert!(newest_source <= modified(&site.join("index.html")).unwrap());
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(sources.join("deep/page.md"), "new").unwrap();
        assert!(newest(&root.join("docs")).unwrap() > modified(&site.join("index.html")).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
