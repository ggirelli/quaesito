//! Small building blocks shared by pages.

use dioxus::prelude::*;

use super::ICON;

/// A pop-up box over the page, closed with its Close button, Escape or a click outside it.
#[component]
pub fn Dialog(
    title: String,
    on_close: EventHandler<()>,
    /// Extra classes for the box, e.g. to size it.
    #[props(default)]
    class: &'static str,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "overlay",
            onclick: move |_| on_close.call(()),
            onkeydown: move |e: KeyboardEvent| {
                e.stop_propagation();
                if e.key() == Key::Escape {
                    on_close.call(());
                }
            },
            section {
                class: "card stack panel dialog",
                class: "{class}",
                role: "dialog",
                aria_label: "{title}",
                onclick: move |e| e.stop_propagation(),
                div { class: "row between",
                    h2 { "{title}" }
                    button { class: "button ghost", onclick: move |_| on_close.call(()), "Close" }
                }
                {children}
            }
        }
    }
}

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

/// Shapes paired with the answer colours (`.a0`…`.a5`), so answers can be told apart without
/// colour: a sparkle, a moon, a heart, a bolt, a leaf and a drop, drawn on a 24×24 grid. Drawn
/// rather than typed, since phones render symbol characters differently (some as emoji).
const ANSWER_SHAPES: [&str; 6] = [
    "M12 1C13 8 16 11 23 12C16 13 13 16 12 23C11 16 8 13 1 12C8 11 11 8 12 1Z",
    "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z",
    "M12 21.4l-1.5-1.3C5.4 15.4 2 12.3 2 8.5 2 5.4 4.4 3 7.5 3c1.7 0 3.4.8 4.5 2.1C13.1 3.8 14.8 3 16.5 3 19.6 3 22 5.4 22 8.5c0 3.8-3.4 6.9-8.6 11.5z",
    "M13.5 1 3 14h8l-1.5 9L21 10h-8z",
    "M3 21C3 10 10 3 21 3 21 14 14 21 3 21Z",
    "M12 1.5C12 1.5 4.5 10 4.5 15a7.5 7.5 0 0 0 15 0C19.5 10 12 1.5 12 1.5Z",
];

/// How many answer colours and shapes there are; later answers reuse them.
pub const ANSWER_STYLES: usize = ANSWER_SHAPES.len();

/// The shape of answer `index`, as big as the surrounding text.
#[component]
pub fn AnswerShape(index: usize) -> Element {
    rsx! {
        svg {
            class: "shape",
            view_box: "0 0 24 24",
            width: "1em",
            height: "1em",
            fill: "currentColor",
            "aria-hidden": "true",
            path { d: ANSWER_SHAPES[index % ANSWER_STYLES] }
        }
    }
}

/// The letter naming item `index` of a pairs question's right column: A, B…
#[must_use]
pub fn pair_letter(index: usize) -> char {
    char::from(b'A' + u8::try_from(index % 26).unwrap_or(0))
}

/// An item of a pairs question: its picture, if any, then its text.
#[component]
pub fn PairItemContent(item: crate::model::PairItem) -> Element {
    let text = item.text.trim().to_owned();
    rsx! {
        if let Some(image) = &item.image {
            img { class: "pair-image", src: media_url(image), alt: "{text}" }
        }
        if !text.is_empty() {
            span { class: "pair-text", "{text}" }
        }
    }
}

/// The coloured shape identifying answer `index`.
#[component]
pub fn AnswerBadge(index: usize) -> Element {
    let class = format!("badge a{}", index % ANSWER_STYLES);
    rsx! {
        span { class, aria_hidden: "true",
            AnswerShape { index }
        }
    }
}

/// URL of an uploaded picture.
#[must_use]
pub fn media_url(name: &str) -> String {
    format!("/media/{name}")
}

/// The class giving a team's picture the background it picked (`.backdrop-0` to `.backdrop-10`
/// in the stylesheet: the answer colours, flat, then in pairs), if any. A class rather than an
/// inline style: switching an inline style straight to another value left it empty on every other
/// change.
#[must_use]
pub fn backdrop_class(backdrop: Option<u8>) -> String {
    backdrop
        .filter(|b| *b < crate::protocol::BACKDROP_COUNT)
        .map(|b| format!("backdrop-{b}"))
        .unwrap_or_default()
}

/// A team's avatar, or its initial when it has none, on the background it picked.
#[component]
pub fn TeamAvatar(team: crate::protocol::TeamInfo, #[props(default)] class: String) -> Element {
    let class = format!("avatar {class} {}", backdrop_class(team.backdrop));
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
    Help,
    Home,
    Back,
    Music,
    Edit,
    Copy,
    Trash,
    Export,
    Import,
    NewFile,
    Search,
    Sun,
    Moon,
    // The question library's categories (Music is above).
    PenTool,
    Film,
    Coffee,
    Globe,
    Clock,
    Type,
    Book,
    Leaf,
    Star,
    Flask,
    Users,
    Trophy,
    Cpu,
}

/// Where this device remembers the chosen colour theme ("light"; dark when unset).
pub const THEME_KEY: &str = "quaesito.theme";

/// Applies the remembered theme before the page is drawn, so a light page never flashes dark,
/// and follows a change made in another tab or in the test page's frames.
pub const THEME_SCRIPT: &str = "(function(){var k='quaesito.theme';\
function apply(){var t=null;try{t=localStorage.getItem(k)}catch(e){}\
if(t==='light'){document.documentElement.dataset.theme='light'}\
else{delete document.documentElement.dataset.theme}}\
apply();window.addEventListener('storage',function(e){if(e.key===k)apply()})})();";

/// Switches between the dark and light themes, like the user guide's toggle. The choice is kept
/// on this device and reaches its other open pages.
#[component]
pub fn ThemeToggle(#[props(default)] class: String) -> Element {
    let mut light = use_signal(|| false);
    use_effect(move || {
        light.set(super::client::storage_get(THEME_KEY).as_deref() == Some("light"));
    });
    let label = if light() { "Dark mode" } else { "Light mode" };
    rsx! {
        button {
            class,
            title: label,
            aria_label: label,
            onclick: move |_| {
                let now_light = !light();
                light.set(now_light);
                if now_light {
                    super::client::storage_set(THEME_KEY, "light");
                } else {
                    super::client::storage_remove(THEME_KEY);
                }
                document::eval(&format!(
                    "if ({now_light}) {{ document.documentElement.dataset.theme = 'light'; }} \
                     else {{ delete document.documentElement.dataset.theme; }}"
                ));
            },
            Icon { kind: if light() { IconKind::Moon } else { IconKind::Sun } }
        }
    }
}

/// Opens the user guide, at its getting-started page, in a new tab.
#[component]
pub fn HelpLink(#[props(default)] class: String, #[props(default)] label: bool) -> Element {
    rsx! {
        a {
            class,
            href: "/docs/getting-started/",
            target: "_blank",
            title: "User guide",
            aria_label: "User guide",
            Icon { kind: IconKind::Help }
            if label {
                "Help"
            }
        }
    }
}

/// A small inline icon.
#[component]
pub fn Icon(kind: IconKind) -> Element {
    let body = match kind {
        IconKind::Settings => rsx! {
            circle { cx: 12, cy: 12, r: 3 }
            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" }
        },
        IconKind::Help => rsx! {
            circle { cx: 12, cy: 12, r: 10 }
            path { d: "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" }
            line { x1: 12, y1: 17, x2: 12.01, y2: 17 }
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
        IconKind::NewFile => rsx! {
            path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
            polyline { points: "14 2 14 8 20 8" }
            line { x1: 12, y1: 18, x2: 12, y2: 12 }
            line { x1: 9, y1: 15, x2: 15, y2: 15 }
        },
        IconKind::Search => rsx! {
            circle { cx: 11, cy: 11, r: 8 }
            line { x1: 21, y1: 21, x2: 16.65, y2: 16.65 }
        },
        IconKind::Sun => rsx! {
            circle { cx: 12, cy: 12, r: 5 }
            line { x1: 12, y1: 1, x2: 12, y2: 3 }
            line { x1: 12, y1: 21, x2: 12, y2: 23 }
            line { x1: 4.22, y1: 4.22, x2: 5.64, y2: 5.64 }
            line { x1: 18.36, y1: 18.36, x2: 19.78, y2: 19.78 }
            line { x1: 1, y1: 12, x2: 3, y2: 12 }
            line { x1: 21, y1: 12, x2: 23, y2: 12 }
            line { x1: 4.22, y1: 19.78, x2: 5.64, y2: 18.36 }
            line { x1: 18.36, y1: 5.64, x2: 19.78, y2: 4.22 }
        },
        IconKind::Moon => rsx! {
            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
        },
        IconKind::PenTool => rsx! {
            path { d: "M12 19l7-7 3 3-7 7-3-3z" }
            path { d: "M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z" }
            path { d: "M2 2l7.586 7.586" }
            circle { cx: 11, cy: 11, r: 2 }
        },
        IconKind::Film => rsx! {
            rect { x: 2, y: 2, width: 20, height: 20, rx: 2.18, ry: 2.18 }
            line { x1: 7, y1: 2, x2: 7, y2: 22 }
            line { x1: 17, y1: 2, x2: 17, y2: 22 }
            line { x1: 2, y1: 12, x2: 22, y2: 12 }
            line { x1: 2, y1: 7, x2: 7, y2: 7 }
            line { x1: 2, y1: 17, x2: 7, y2: 17 }
            line { x1: 17, y1: 17, x2: 22, y2: 17 }
            line { x1: 17, y1: 7, x2: 22, y2: 7 }
        },
        IconKind::Coffee => rsx! {
            path { d: "M18 8h1a4 4 0 0 1 0 8h-1" }
            path { d: "M2 8h16v9a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V8z" }
            line { x1: 6, y1: 1, x2: 6, y2: 4 }
            line { x1: 10, y1: 1, x2: 10, y2: 4 }
            line { x1: 14, y1: 1, x2: 14, y2: 4 }
        },
        IconKind::Globe => rsx! {
            circle { cx: 12, cy: 12, r: 10 }
            line { x1: 2, y1: 12, x2: 22, y2: 12 }
            path { d: "M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" }
        },
        IconKind::Clock => rsx! {
            circle { cx: 12, cy: 12, r: 10 }
            polyline { points: "12 6 12 12 16 14" }
        },
        IconKind::Type => rsx! {
            polyline { points: "4 7 4 4 20 4 20 7" }
            line { x1: 9, y1: 20, x2: 15, y2: 20 }
            line { x1: 12, y1: 4, x2: 12, y2: 20 }
        },
        IconKind::Book => rsx! {
            path { d: "M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" }
            path { d: "M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" }
        },
        IconKind::Leaf => rsx! {
            path { d: "M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z" }
            path { d: "M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12" }
        },
        IconKind::Star => rsx! {
            polygon { points: "12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" }
        },
        IconKind::Flask => rsx! {
            path { d: "M10 2v7.527a2 2 0 0 1-.211.896L4.72 20.55a1 1 0 0 0 .9 1.45h12.76a1 1 0 0 0 .9-1.45l-5.069-10.127A2 2 0 0 1 14 9.527V2" }
            path { d: "M8.5 2h7" }
            path { d: "M7 16h10" }
        },
        IconKind::Users => rsx! {
            path { d: "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" }
            circle { cx: 9, cy: 7, r: 4 }
            path { d: "M23 21v-2a4 4 0 0 0-3-3.87" }
            path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
        },
        IconKind::Trophy => rsx! {
            path { d: "M6 9H4.5a2.5 2.5 0 0 1 0-5H6" }
            path { d: "M18 9h1.5a2.5 2.5 0 0 0 0-5H18" }
            path { d: "M4 22h16" }
            path { d: "M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22" }
            path { d: "M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22" }
            path { d: "M18 2H6v7a6 6 0 0 0 12 0V2Z" }
        },
        IconKind::Cpu => rsx! {
            rect { x: 4, y: 4, width: 16, height: 16, rx: 2, ry: 2 }
            rect { x: 9, y: 9, width: 6, height: 6 }
            line { x1: 9, y1: 1, x2: 9, y2: 4 }
            line { x1: 15, y1: 1, x2: 15, y2: 4 }
            line { x1: 9, y1: 20, x2: 9, y2: 23 }
            line { x1: 15, y1: 20, x2: 15, y2: 23 }
            line { x1: 20, y1: 9, x2: 23, y2: 9 }
            line { x1: 20, y1: 14, x2: 23, y2: 14 }
            line { x1: 1, y1: 9, x2: 4, y2: 9 }
            line { x1: 1, y1: 14, x2: 4, y2: 14 }
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

/// A pin drawn on a [`PinMap`].
#[derive(Debug, Clone, PartialEq)]
pub struct MapPin {
    pub at: crate::model::Point,
    /// A few characters drawn in the pin (a team's initial, ★ for the right spot).
    pub label: String,
    /// Shown on hover.
    pub title: String,
    /// Extra CSS class: `answer`, `own`, or empty for other teams.
    pub class: &'static str,
}

/// A picture with pins on it, drawn as one SVG so pins stay on their spot at any size.
///
/// `aspect` is the picture's height divided by its width; while it is unknown (0, before the
/// editor's first click) the picture is shown alone. With `on_pick`, tapping it reports the spot
/// and the picture's height-to-width ratio.
#[component]
pub fn PinMap(
    image: String,
    aspect: f64,
    pins: Vec<MapPin>,
    #[props(default)] on_pick: Option<EventHandler<(crate::model::Point, f64)>>,
    /// Pins are sized relative to the picture; small screens draw them larger.
    #[props(default = 1.0)]
    pin_scale: f64,
) -> Element {
    let mut mounted = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let pick = move |e: MouseEvent| async move {
        let (Some(handler), Some(element)) = (on_pick, mounted()) else {
            return;
        };
        let Ok(rect) = element.get_client_rect().await else {
            return;
        };
        let (width, height) = (rect.width(), rect.height());
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let click = e.client_coordinates();
        let (x, y) = (click.x - rect.min_x(), click.y - rect.min_y());
        // The drawing keeps the picture's shape; when the box is another shape the picture is
        // centred in it ("meet"), so measure from the picture itself.
        let (point, picture_aspect) = if aspect > 0.0 {
            let scale = (width / MAP_WIDTH).min(height / (MAP_WIDTH * aspect));
            let (drawn_w, drawn_h) = (MAP_WIDTH * scale, MAP_WIDTH * aspect * scale);
            let (left, top) = ((width - drawn_w) / 2.0, (height - drawn_h) / 2.0);
            ((x - left) / drawn_w, (y - top) / drawn_h, aspect)
        } else {
            (x / width, y / height, height / width)
        }
        .into_point();
        handler.call((point, picture_aspect));
    };
    if aspect <= 0.0 {
        return rsx! {
            div { class: if on_pick.is_some() { "pin-map pickable" } else { "pin-map" },
                img {
                    src: media_url(&image),
                    alt: "",
                    draggable: false,
                    onmounted: move |e| mounted.set(Some(e.data())),
                    onclick: pick,
                }
            }
        };
    }
    let height = MAP_WIDTH * aspect;
    rsx! {
        svg {
            class: if on_pick.is_some() { "pin-map pickable" } else { "pin-map" },
            view_box: "0 0 {MAP_WIDTH} {height}",
            preserve_aspect_ratio: "xMidYMid meet",
            onmounted: move |e| mounted.set(Some(e.data())),
            onclick: pick,
            image {
                "href": media_url(&image),
                x: 0,
                y: 0,
                width: MAP_WIDTH,
                height,
                preserve_aspect_ratio: "none",
            }
            for (i, pin) in pins.into_iter().enumerate() {
                g {
                    key: "{i}",
                    class: "map-pin {pin.class}",
                    transform: "translate({pin.at.x * MAP_WIDTH:.2} {pin.at.y * height:.2}) scale({pin_scale})",
                    "aria-label": "{pin.title}",
                    circle { r: PIN_RADIUS }
                    // Symbols are drawn as shapes: font glyphs are not centred reliably.
                    match pin.label.as_str() {
                        STAR => rsx! {
                            polygon { points: star_points(PIN_RADIUS * 0.68) }
                        },
                        DOT => rsx! {
                            circle { class: "dot", r: PIN_RADIUS * 0.35 }
                        },
                        label => rsx! {
                            text { text_anchor: "middle", dominant_baseline: "central", "{label}" }
                        },
                    }
                }
            }
        }
    }
}

/// A [`MapPin`] label drawn as a star (the right spot).
pub const STAR: &str = "★";
/// A [`MapPin`] label drawn as a dot (your own pin).
pub const DOT: &str = "•";

/// Points of a five-pointed star of radius `outer`, centred on the origin, pointing up.
fn star_points(outer: f64) -> String {
    let inner = outer * 0.42;
    (0..10)
        .map(|i| {
            let radius = if i % 2 == 0 { outer } else { inner };
            let angle = std::f64::consts::PI * (f64::from(i) / 5.0 - 0.5);
            format!("{:.2},{:.2}", radius * angle.cos(), radius * angle.sin())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Width of a [`PinMap`]'s drawing, in its own units.
const MAP_WIDTH: f64 = 1000.0;
/// Radius of a pin, in drawing units (a share of the picture's width).
const PIN_RADIUS: f64 = 30.0;

/// Turns `(x, y, aspect)` with `x` and `y` as shares of the picture into a clamped point.
trait IntoPoint {
    fn into_point(self) -> (crate::model::Point, f64);
}

impl IntoPoint for (f64, f64, f64) {
    fn into_point(self) -> (crate::model::Point, f64) {
        let (x, y, aspect) = self;
        (
            crate::model::Point {
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            },
            aspect,
        )
    }
}

/// A question's or slide's picture or video. With `autoplay`, a video plays once when shown;
/// clicking it plays it again from the start.
#[component]
pub fn QuestionMedia(
    name: String,
    class: String,
    #[props(default)] style: String,
    #[props(default)] autoplay: bool,
    /// Pauses a playing video while the host has paused the game, and resumes it after.
    #[props(default)]
    paused: bool,
) -> Element {
    let video = crate::model::is_video(&name);
    let id = format!("video-{}", name.split('.').next().unwrap_or_default());
    let mut was_paused = use_signal(|| false);
    use_effect(use_reactive!(|paused, id| {
        if !video || paused == *was_paused.peek() {
            return;
        }
        was_paused.set(paused);
        let action = if paused {
            "v.pause()"
        } else {
            "v.play().catch(() => {})"
        };
        document::eval(&format!(
            "const v = document.getElementById('{id}'); if (v && !v.ended) {{ {action}; }}"
        ));
    }));
    if !video {
        return rsx! {
            img { class, style, src: media_url(&name), alt: "" }
        };
    }
    let replay = {
        let id = id.clone();
        move |_| {
            document::eval(&format!(
                "const v = document.getElementById('{id}'); \
                 if (v) {{ v.currentTime = 0; v.play().catch(() => {{}}); }}"
            ));
        }
    };
    rsx! {
        video {
            id,
            class: "{class} question-video",
            style,
            src: media_url(&name),
            autoplay,
            playsinline: true,
            preload: "auto",
            title: "Click to play again",
            onclick: replay,
        }
    }
}
