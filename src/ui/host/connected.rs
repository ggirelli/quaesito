//! The big screen of a game on the server: its socket, the lobby's join code and the admin's
//! panels around the [`HostBoard`].

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    HostBoard,
    lobby::{Lobby, NetworkSlideView},
    panels::{ResultsPanel, TeamsPanel},
};
use crate::ui::{
    Route,
    admin::{AdminGate, AdminToken},
    categories::Categories,
    live::use_live,
    sfx::SFX_JS,
    widgets::{ClientOnly, Icon, IconKind, Spinner},
};
use crate::{
    api::{end_session, host_socket, list_sessions},
    protocol::HostPhase,
};

#[component]
pub fn Host(code: String) -> Element {
    rsx! {
        document::Script { src: SFX_JS }
        ClientOnly {
            AdminGate {
                HostScreen { code }
            }
        }
    }
}

#[component]
fn HostScreen(code: String) -> Element {
    let token: AdminToken = use_context();
    let code = use_signal(|| code);
    let live = use_live(move || host_socket(code(), token.get(), WebSocketOptions::new()));
    use_context_provider(|| live);
    // The browser can't tell a refused socket from a dropped one, so on every drop ask whether
    // the game is still running.
    let mut gone = use_signal(|| false);
    use_effect(move || {
        if !(live.connected)() {
            spawn(async move {
                if let Ok(sessions) = list_sessions(token.get()).await {
                    gone.set(!sessions.iter().any(|s| s.code == *code.peek()));
                }
            });
        }
    });
    let categories = Categories::use_loaded();
    use_context_provider(|| categories);
    let mut show_teams = use_signal(|| false);
    let mut show_results = use_signal(|| false);

    if gone() {
        return rsx! {
            div { class: "page center",
                h1 { "This game is not running" }
                BackToAdmin { class: "button", code: code() }
            }
        };
    }
    let view = live.view.read();
    let Some(view) = view.as_ref() else {
        return rsx! {
            div { class: "page center", Spinner {} }
        };
    };

    rsx! {
        HostBoard {
            lobby: match &view.phase {
                HostPhase::Network { network } => rsx! {
                    NetworkSlideView { network: network.clone() }
                },
                HostPhase::Lobby { .. } => rsx! {
                    Lobby { code: view.code.clone(), teams: view.teams.clone(), max_teams: view.max_teams }
                },
                _ => rsx! {},
            },
            tools: rsx! {
                button {
                    class: "button ghost",
                    title: "Rename or remove teams",
                    onclick: move |_| show_teams.set(true),
                    "Teams ({view.teams.len()})"
                }
                Link {
                    class: "button ghost icon",
                    to: Route::Admin {},
                    title: "Admin",
                    aria_label: "Admin",
                    Icon { kind: IconKind::Settings }
                }
            },
            finish: rsx! {
                button {
                    class: "button primary big",
                    onclick: move |_| show_results.set(true),
                    "📊 Results"
                }
                BackToAdmin { class: "button big", code: code() }
            },
            if show_teams() {
                TeamsPanel {
                    teams: view.teams.clone(),
                    pictures: view.pictures,
                    joining: view.joining,
                    on_close: move |()| show_teams.set(false),
                }
            }
            if show_results() {
                ResultsPanel { code: code(), on_close: move |()| show_results.set(false) }
            }
        }
    }
}

/// Back to the quiz list. In the test page's frame it ends the test and leaves the test page,
/// instead of opening the admin pages inside the frame.
#[component]
fn BackToAdmin(class: &'static str, code: String) -> Element {
    let token: AdminToken = use_context();
    let leave = move |_| {
        let code = code.clone();
        async move {
            let framed = document::eval("return window.self !== window.top;")
                .join::<bool>()
                .await
                .unwrap_or(false);
            if framed {
                let _ = end_session(token.get(), code).await;
                document::eval("window.top.location.assign('/admin');");
            } else {
                navigator().push(Route::Admin {});
            }
        }
    };
    rsx! {
        button { class, onclick: leave, "Back to admin" }
    }
}
