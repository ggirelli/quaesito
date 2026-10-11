//! Quaesito: quizzes among friends.

use quaesito::ui::App;

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        quaesito::server::init().await?;
        Ok(dioxus::server::router(App)
            .merge(quaesito::server::routes())
            .layer(quaesito::server::offline_policy()))
    });
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}
