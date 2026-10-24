//! The web interface.
//!
//! Without the `fullstack` feature, only the parts that need no server are built: the big
//! screen's and the phones' boards, driven by a game in the browser (see [`playground`]).

#[cfg(feature = "fullstack")]
mod admin;
mod categories;
pub mod client;
#[cfg(feature = "fullstack")]
mod editor;
#[cfg(feature = "fullstack")]
mod home;
mod host;
#[cfg(feature = "fullstack")]
mod library;
mod live;
mod player;
pub mod playground;
mod sfx;
#[cfg(feature = "fullstack")]
mod test;
#[cfg(feature = "fullstack")]
mod transfer;
mod widgets;

use dioxus::prelude::*;

#[cfg(feature = "fullstack")]
use self::{
    admin::{Admin, AdminQuestions},
    editor::Editor,
    home::Home,
    host::Host,
    player::{Join, JoinBlank},
    test::Test,
};

#[cfg(feature = "fullstack")]
const FAVICON: Asset = asset!("/assets/brand/icon-192.png");
/// The app's stylesheet.
pub const MAIN_CSS: Asset = asset!("/assets/main.css");
/// The logo mark, used in page headers.
pub const ICON: Asset = asset!("/assets/brand/icon-192.png");
/// Illustration on the big screen while teams join.
pub const LOBBY_ART: Asset = asset!("/assets/brand/lobby.webp");

/// All pages.
#[cfg(feature = "fullstack")]
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},
    // `?code=` is where the join form lands when submitted before the app has loaded.
    #[route("/join?:code")]
    JoinBlank { code: String },
    // `?ticket=` plays as an existing team without remembering it (the admin's test phones).
    #[route("/join/:code?:ticket")]
    Join { code: String, ticket: String },
    #[route("/admin")]
    Admin {},
    #[route("/admin/questions")]
    AdminQuestions {},
    #[route("/admin/quiz/:id")]
    Editor { id: String },
    #[route("/admin/test/:code")]
    Test { code: String },
    #[route("/host/:code")]
    Host { code: String },
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Root component.
#[cfg(feature = "fullstack")]
#[component]
pub fn App() -> Element {
    rsx! {
        document::Title { "Quaesito" }
        document::Link { rel: "icon", href: FAVICON, r#type: "image/png" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Meta { name: "theme-color", content: "#14121f" }
        document::Stylesheet { href: MAIN_CSS }
        document::Script { {widgets::THEME_SCRIPT} }
        Router::<Route> {}
    }
}

#[cfg(feature = "fullstack")]
#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");
    rsx! {
        main { class: "page center",
            h1 { "Nothing here" }
            p { class: "muted", "There is no page at /{path}." }
            Link { class: "button", to: Route::Home {}, "Go home" }
        }
    }
}
