//! Quaesito: quizzes among friends.

use dioxus::prelude::*;

const FAVICON: Asset = asset!("/assets/brand/icon.svg");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Title { "Quaesito" }
        document::Link { rel: "icon", href: FAVICON, r#type: "image/svg+xml" }
        document::Stylesheet { href: MAIN_CSS }
        main { class: "splash",
            h1 { "Quaesito" }
            p { "Quizzes among friends." }
        }
    }
}
