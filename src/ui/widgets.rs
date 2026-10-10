//! Small building blocks shared by pages.

use dioxus::prelude::*;

use super::ICON;

/// Renders its children only in the browser, after hydration.
///
/// Pages that depend on `localStorage` or live sockets use this so the server-rendered HTML
/// matches what the browser renders first.
#[component]
pub fn ClientOnly(children: Element) -> Element {
    let mut mounted = use_signal(|| false);
    use_effect(move || mounted.set(true));
    if mounted() {
        children
    } else {
        rsx! {
            div { class: "page center",
                Spinner {}
            }
        }
    }
}

/// A loading indicator.
#[component]
pub fn Spinner() -> Element {
    rsx! {
        div { class: "spinner", role: "status", aria_label: "Loading" }
    }
}

/// The logo mark and word mark.
#[component]
pub fn Brand(#[props(default)] large: bool) -> Element {
    rsx! {
        span { class: if large { "brand large" } else { "brand" },
            img { src: ICON, alt: "" }
            span { class: "wordmark", "quaesito" }
        }
    }
}

/// An error message box, hidden when there is no error.
#[component]
pub fn ErrorBox(error: Option<String>) -> Element {
    match error {
        Some(message) => rsx! {
            p { class: "error", role: "alert", "{message}" }
        },
        None => rsx! {},
    }
}

/// Human-readable message for a failed server call.
#[must_use]
pub fn message(err: &ServerFnError) -> String {
    match err {
        ServerFnError::ServerError { message, .. } => message.clone(),
        ServerFnError::Request(_) => "Could not reach the server.".to_owned(),
        other => other.to_string(),
    }
}

/// Shapes paired with answer colours, so answers can be told apart without colour.
pub const ANSWER_SHAPES: [&str; 6] = ["▲", "◆", "●", "■", "★", "⬟"];

/// The coloured shape identifying answer `index`.
#[component]
pub fn AnswerBadge(index: usize) -> Element {
    let shape = ANSWER_SHAPES[index % ANSWER_SHAPES.len()];
    let class = format!("badge a{}", index % ANSWER_SHAPES.len());
    rsx! {
        span { class, aria_hidden: "true", "{shape}" }
    }
}

/// URL of an uploaded picture.
#[must_use]
pub fn media_url(name: &str) -> String {
    format!("/media/{name}")
}

/// A team's avatar, or its initial when it has none.
#[component]
pub fn TeamAvatar(team: crate::protocol::TeamInfo, #[props(default)] class: String) -> Element {
    let class = format!("avatar {class}");
    if let Some(avatar) = &team.avatar {
        return rsx! {
            img { class, src: media_url(avatar), alt: "" }
        };
    }
    let initial: String = team
        .name
        .chars()
        .next()
        .into_iter()
        .flat_map(char::to_uppercase)
        .collect();
    rsx! {
        span { class, aria_hidden: "true", "{initial}" }
    }
}
