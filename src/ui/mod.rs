//! The web interface.

mod admin;
pub mod client;
mod editor;
mod home;
mod host;
mod live;
mod player;
mod widgets;

use dioxus::prelude::*;

use self::{
    admin::Admin,
    editor::Editor,
    home::Home,
    host::Host,
    player::{Join, JoinBlank},
};

const FAVICON: Asset = asset!("/assets/brand/icon-192.png");
const MAIN_CSS: Asset = asset!("/assets/main.css");
/// The logo mark, used in page headers.
pub const ICON: Asset = asset!("/assets/brand/icon-192.png");
/// Illustration on the big screen while teams join.
pub const LOBBY_ART: Asset = asset!("/assets/brand/lobby.webp");

/// All pages.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/join")]
    JoinBlank {},
    #[route("/join/:code")]
    Join { code: String },
    #[route("/admin")]
    Admin {},
    #[route("/admin/quiz/:id")]
    Editor { id: String },
    #[route("/host/:code")]
    Host { code: String },
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Root component.
#[component]
pub fn App() -> Element {
    rsx! {
        document::Title { "Quaesito" }
        document::Link { rel: "icon", href: FAVICON, r#type: "image/png" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Meta { name: "theme-color", content: "#14121f" }
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

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
