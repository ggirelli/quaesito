//! Landing page.

use dioxus::prelude::*;

use super::{
    Route,
    player::JoinCodeForm,
    widgets::{Brand, Icon, IconKind},
};

#[component]
pub fn Home() -> Element {
    rsx! {
        main { class: "page center narrow stack",
            Brand { large: true }
            p { class: "tagline", "Quizzes among friends." }
            JoinCodeForm {}
        }
        Link {
            class: "corner-link",
            to: Route::Admin {},
            title: "Admin",
            aria_label: "Admin",
            Icon { kind: IconKind::Settings }
        }
    }
}
