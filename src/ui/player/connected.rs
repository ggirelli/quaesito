//! A team leader's phone in a game on the server: its socket, the lobby where the team picks its
//! name and picture, and the way home, around the [`PlayerBoard`].

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{PlayerBoard, lobby::LobbyView};
use crate::ui::{Route, live::use_live};
use crate::{
    api::{check_ticket, player_socket},
    protocol::PlayerPhase,
};

#[component]
pub(super) fn PlayerGame(code: String, token: String, on_gone: EventHandler<()>) -> Element {
    let code = use_signal(|| code);
    let token = use_signal(|| token);
    let live = use_live(move || player_socket(code(), token(), WebSocketOptions::new()));
    use_context_provider(|| live);
    // A dropped connection may mean the host removed the team, or ended the game.
    use_effect(move || {
        if !(live.connected)() {
            spawn(async move {
                if matches!(check_ticket(code(), token()).await, Ok(false)) {
                    on_gone.call(());
                }
            });
        }
    });

    let view = live.view.read();
    let lobby = match view.as_ref() {
        Some(view) => match &view.phase {
            PlayerPhase::Lobby { teams, pictures } => rsx! {
                LobbyView { code: code(), token: token(), team: view.team.clone(), teams: *teams, pictures: *pictures }
            },
            _ => rsx! {},
        },
        None => rsx! {},
    };
    rsx! {
        PlayerBoard {
            lobby,
            done: rsx! {
                Link { class: "button", to: Route::Home {}, "Done" }
            },
        }
    }
}
