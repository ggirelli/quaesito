//! WebSocket loops that push views to screens and forward their commands.

use std::sync::Arc;

use dioxus::fullstack::TypedWebsocket;

use super::{engine::Game, sessions::Session};
use crate::protocol::{HostCommand, HostView, PlayerCommand, PlayerView};

/// Most time the host can add in one go.
const MAX_EXTRA_SECS: u32 = 60;

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
                Ok(HostCommand::Back) => {
                    let _ = session.update(Game::back);
                }
                Ok(HostCommand::TogglePause) => {
                    let _ = session.update(|g, now| {
                        if g.is_paused() {
                            g.resume(now);
                            Ok(())
                        } else {
                            g.pause(now)
                        }
                    });
                }
                Ok(HostCommand::AddTime(secs)) => {
                    let extra = std::time::Duration::from_secs(u64::from(secs.min(MAX_EXTRA_SECS)));
                    let _ = session.update(|g, _| g.add_time(extra));
                }
                Ok(HostCommand::RenameTeam { team, name }) => {
                    let _ = session.update(|g, _| g.rename_team(team, &name));
                }
                Ok(HostCommand::RemoveTeam { team }) => {
                    let _ = session.update(|g, now| g.remove_team(team, now));
                }
                Ok(HostCommand::SetPictures(pictures)) => {
                    session.update(|g, _| g.set_pictures(pictures));
                }
                Ok(HostCommand::SetJoining(joining)) => {
                    session.update(|g, _| g.set_joining(joining));
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
                Ok(PlayerCommand::Answer { response, joker, bonus }) => {
                    session.pace(team).await;
                    // Refusals (late or duplicate answers) are reflected by the next view.
                    let _ = session.update(|g, now| {
                        g.answer_with_bonus(team, response, joker, bonus, now)
                    });
                }
                Err(_) => break,
            },
        }
    }
    session.disconnect_phone(team, phone);
    session.update(|g, _| g.set_connected(team, false));
}
