//! WebSocket loops that push views to screens and forward their commands.

use std::sync::Arc;

use dioxus::fullstack::TypedWebsocket;
use quaesito_core::protocol::{HostCommand, HostView, PlayerCommand, PlayerView};

use super::sessions::Session;

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
                // Refusals (e.g. "next" without teams) leave the view unchanged.
                Ok(command) => {
                    let _ = session.update(|g, now| g.apply_host(command, now));
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
    // Connecting also sends every screen a new view.
    session.pace(team).await;
    let (phone, mut closed) = session.connect_phone(team);
    session.update(|g, _| g.set_connected(team, true));
    let mut updates = session.subscribe();
    while let Some(view) = session.read(|g| g.player_view(team, std::time::Instant::now())) {
        if socket.send(view).await.is_err() {
            break;
        }
        tokio::select! {
            changed = updates.changed() => if changed.is_err() { break },
            // The team opened too many connections: this, its oldest, closes.
            _ = &mut closed => break,
            command = socket.recv() => match command {
                Ok(command) => {
                    session.pace(team).await;
                    // Refusals (late or duplicate answers) are reflected by the next view.
                    let _ = session.update(|g, now| g.apply_player(team, command, now));
                }
                Err(_) => break,
            },
        }
    }
    session.disconnect_phone(team, phone);
    session.update(|g, _| g.set_connected(team, false));
}
