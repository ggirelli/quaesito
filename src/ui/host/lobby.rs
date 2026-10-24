//! The lobby, where teams join with the code or the QR code, and the Wi-Fi slide before it.

use dioxus::prelude::*;

use crate::ui::{LOBBY_ART, admin::AdminToken, client, widgets::TeamAvatar};
use crate::{api::join_origin, model::NetworkSlide, protocol::TeamInfo};

/// The lobby illustration is shown while fewer teams than this have joined.
const LOBBY_ART_MAX_TEAMS: usize = 5;

#[component]
pub(super) fn Lobby(code: String, teams: Vec<TeamInfo>, max_teams: usize) -> Element {
    let token: AdminToken = use_context();
    let origin = use_resource(move || async move {
        join_origin(token.get(), client::page_origin(), client::page_port())
            .await
            .unwrap_or_else(|_| client::page_origin())
    });
    let url = origin().map(|o| format!("{o}/join/{code}"));
    let qr = url.as_deref().and_then(qr_svg);
    rsx! {
        div { class: "lobby",
            div { class: "join-panel",
                span { "Join on your phone" }
                div { class: "qr",
                    if let Some(svg) = qr {
                        div { dangerous_inner_html: svg }
                    }
                }
                if let Some(url) = url {
                    span { class: "join-url", "{url}" }
                }
                span { "or enter the code" }
                span { class: "join-code", "{code}" }
            }
            div { class: "stack",
                h1 { class: "question-title",
                    if teams.is_empty() {
                        "Waiting for teams…"
                    } else if teams.len() == 1 {
                        "1 team is in"
                    } else {
                        "{teams.len()} teams are in"
                    }
                }
                p { class: "muted lobby-limit",
                    if teams.len() >= max_teams {
                        "The game is full ({max_teams} teams)."
                    } else {
                        "Up to {max_teams} teams"
                    }
                }
                // The illustration fills the room until enough teams have joined to take its place.
                if teams.len() < LOBBY_ART_MAX_TEAMS {
                    img { class: "lobby-art", src: LOBBY_ART, alt: "" }
                }
                div { class: "teams-grid",
                    for team in teams {
                        div {
                            key: "{team.id}",
                            class: if team.connected { "team-chip" } else { "team-chip offline" },
                            TeamAvatar { team: team.clone() }
                            span { "{team.name}" }
                        }
                    }
                }
            }
        }
    }
}

/// The first slide: how to get on the Wi-Fi, before scanning the join code.
#[component]
pub(super) fn NetworkSlideView(network: NetworkSlide) -> Element {
    let qr = qr_svg(&network.qr_text());
    rsx! {
        div { class: "lobby",
            div { class: "join-panel",
                span { "Scan to join the Wi-Fi" }
                div { class: "qr",
                    if let Some(svg) = qr {
                        div { dangerous_inner_html: svg }
                    }
                }
            }
            div { class: "stack wifi-details",
                h1 { class: "question-title", "Get on the Wi-Fi" }
                div { class: "wifi-field",
                    span { class: "muted", "Network" }
                    span { class: "wifi-value", "{network.name}" }
                }
                div { class: "wifi-field",
                    span { class: "muted", "Password" }
                    if network.password.is_empty() {
                        span { class: "wifi-value", "None needed" }
                    } else {
                        span { class: "wifi-value", "{network.password}" }
                    }
                }
            }
        }
    }
}

/// Text, such as the join link, as an SVG QR code.
fn qr_svg(text: &str) -> Option<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .quiet_zone(false)
            .dark_color(qrcode::render::svg::Color("#14121f"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}
