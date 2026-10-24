//! Quaesito: quizzes among friends.

use quaesito::ui::App;

#[cfg(feature = "server")]
async fn app() -> anyhow::Result<dioxus::fullstack::axum::Router> {
    quaesito_server::init().await?;
    Ok(quaesito_server::protect(
        dioxus::server::router(App).merge(quaesito_server::routes()),
    ))
}

fn main() {
    // Development: dx's server, with hot reload, behind dx's proxy.
    #[cfg(all(feature = "server", debug_assertions))]
    dioxus::serve(app);
    // Release: served as dx would, but telling each request its peer's address, which decides
    // what may be uploaded (see `quaesito_server::peer`).
    #[cfg(all(feature = "server", not(debug_assertions)))]
    quaesito_server::serve(app);
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}
