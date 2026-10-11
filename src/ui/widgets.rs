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
    Help,
    Home,
    Back,
    Music,
    Edit,
    Copy,
    Trash,
    Export,
    Import,
}

/// Opens the user guide in a new tab.
#[component]
pub fn HelpLink(#[props(default)] class: String, #[props(default)] label: bool) -> Element {
    rsx! {
        a {
            class,
            href: "/docs/",
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
