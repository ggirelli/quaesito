//! Landing page.

use dioxus::prelude::*;

use super::{Route, player::JoinCodeForm, widgets::Brand};

#[component]
pub fn Home() -> Element {
    rsx! {
        main { class: "page center narrow stack",
            Brand { large: true }
            p { class: "tagline", "Quizzes among friends." }
            JoinCodeForm {}
            Link { class: "muted small", to: Route::Admin {}, "Admin" }
        }
    }
}
