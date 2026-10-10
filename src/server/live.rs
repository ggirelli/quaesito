//! WebSocket loops that push views to screens and forward their commands.

use std::sync::Arc;

use dioxus::fullstack::TypedWebsocket;

use super::{engine::Game, sessions::Session};
use crate::protocol::{HostCommand, HostView, PlayerCommand, PlayerView};

/// Serves the host screen until it disconnects.
pub async fn host(mut socket: TypedWebsocket<HostCommand, HostView>, session: Arc<Session>) {
    let mut updates = session.subscribe();
    loop {
        let view = session.read(|g| g.host_view(std::time::Instant::now()));
        if socket.send(view).await.is_err() {
            return;
        }
        tokio::select! {
            changed = updates.changed() => if changed.is_err() { return },
            command = socket.recv() => match command {
                Ok(HostCommand::Next) => {
                    // A refused "next" (e.g. no teams yet) leaves the view unchanged.
                    let _ = session.update(Game::next);
                }
                Ok(HostCommand::Judge { question, key, judgement }) => {
                    let _ = session.update(|g, now| g.judge(question, &key, judgement, now));
                }
                Ok(HostCommand::RenameTeam { team, name }) => {
                    let _ = session.update(|g, _| g.rename_team(team, &name));
                }
                Ok(HostCommand::RemoveTeam { team }) => {
                    let _ = session.update(|g, now| g.remove_team(team, now));
                }
                Err(_) => return,
            },
        }
    }
}

/// Serves a team leader's phone until it disconnects.
pub async fn player(
    mut socket: TypedWebsocket<PlayerCommand, PlayerView>,
    session: Arc<Session>,
    team: u32,
) {
    session.update(|g, _| g.set_connected(team, true));
    let mut updates = session.subscribe();
    while let Some(view) = session.read(|g| g.player_view(team, std::time::Instant::now())) {
        if socket.send(view).await.is_err() {
            break;
        }
        tokio::select! {
            changed = updates.changed() => if changed.is_err() { break },
            command = socket.recv() => match command {
                Ok(PlayerCommand::Answer(response)) => {
                    // Refusals (late or duplicate answers) are reflected by the next view.
                    let _ = session.update(|g, now| g.answer(team, response, now));
                }
                Err(_) => break,
            },
        }
    }
    session.update(|g, _| g.set_connected(team, false));
}
