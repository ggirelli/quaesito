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

/// Line icons drawn inline so they follow the text colour (shapes from Feather, MIT).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Settings,
    Home,
    Back,
    Music,
    Edit,
    Copy,
    Trash,
    Export,
    Import,
}

/// A small inline icon.
#[component]
pub fn Icon(kind: IconKind) -> Element {
    let body = match kind {
        IconKind::Settings => rsx! {
            circle { cx: 12, cy: 12, r: 3 }
            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" }
        },
        IconKind::Home => rsx! {
            path { d: "M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" }
            polyline { points: "9 22 9 12 15 12 15 22" }
        },
        IconKind::Back => rsx! {
            line { x1: 19, y1: 12, x2: 5, y2: 12 }
            polyline { points: "12 19 5 12 12 5" }
        },
        IconKind::Music => rsx! {
            path { d: "M9 18V5l12-2v13" }
            circle { cx: 6, cy: 18, r: 3 }
            circle { cx: 18, cy: 16, r: 3 }
        },
        IconKind::Edit => rsx! {
            path { d: "M12 20h9" }
            path { d: "M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" }
        },
        IconKind::Copy => rsx! {
            rect { x: 9, y: 9, width: 13, height: 13, rx: 2, ry: 2 }
            path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
        },
        IconKind::Trash => rsx! {
            polyline { points: "3 6 5 6 21 6" }
            path { d: "M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" }
            path { d: "M10 11v6" }
            path { d: "M14 11v6" }
            path { d: "M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2" }
        },
        IconKind::Export => rsx! {
            path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
            polyline { points: "7 10 12 15 17 10" }
            line { x1: 12, y1: 15, x2: 12, y2: 3 }
        },
        IconKind::Import => rsx! {
            path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
            polyline { points: "17 8 12 3 7 8" }
            line { x1: 12, y1: 3, x2: 12, y2: 15 }
        },
    };
    rsx! {
        svg {
            class: "icon-svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: 2,
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            {body}
        }
    }
}
