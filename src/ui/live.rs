//! Live connection to a session: the latest view pushed by the server, with reconnection.
//!
//! Screens only see a [`Live`]: the latest view, and [`Live::send`] for their commands. They do
//! not know where views come from, so another source (such as a game running in the browser)
//! could drive the same screens.

#[cfg(feature = "fullstack")]
use std::future::Future;

#[cfg(feature = "fullstack")]
use dioxus::fullstack::{Websocket, use_websocket};
use dioxus::prelude::*;
#[cfg(feature = "fullstack")]
use serde::{Serialize, de::DeserializeOwned};

use super::client;

/// Pause before reconnecting a dropped socket.
#[cfg(feature = "fullstack")]
const RECONNECT_DELAY_MS: u32 = 1500;

/// A live game as a screen sees it: what it last received, and where its commands go.
pub struct Live<In: 'static, Out: 'static> {
    /// The latest view, `None` until the first one arrives.
    pub view: Signal<Option<Out>>,
    /// Browser time (ms) when `view` arrived, to run countdowns locally.
    pub received_at: Signal<f64>,
    /// Whether the socket is currently up.
    pub connected: Signal<bool>,
    commands: Callback<In>,
}

impl<In, Out> Clone for Live<In, Out> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<In, Out> Copy for Live<In, Out> {}

impl<In, Out> Live<In, Out> {
    /// A live game fed by its owner, such as a game running in the browser: it sets `view` and
    /// `received_at`, and `commands` gets what the screen sends.
    pub fn new(
        view: Signal<Option<Out>>,
        received_at: Signal<f64>,
        connected: Signal<bool>,
        commands: Callback<In>,
    ) -> Self {
        Self {
            view,
            received_at,
            connected,
            commands,
        }
    }

    /// Sends a command; the screen learns its effect from the next view.
    pub fn send(&self, command: In) {
        self.commands.call(command);
    }
}

/// Connects with `connect` and keeps the connection alive.
#[cfg(feature = "fullstack")]
pub fn use_live<In, Out, Fut>(connect: impl FnMut() -> Fut + Copy + 'static) -> Live<In, Out>
where
    In: Serialize + 'static,
    Out: DeserializeOwned + 'static,
    Fut: Future<Output = Result<Websocket<In, Out>, ServerFnError>> + 'static,
{
    let mut socket = use_websocket(connect);
    let mut view = use_signal(|| None);
    let mut received_at = use_signal(|| 0.0);
    let mut connected = use_signal(|| true);
    let commands = use_callback(move |command: In| {
        spawn(async move {
            let _ = socket.send(command).await;
        });
    });
    use_future(move || {
        let mut connect = connect;
        async move {
            loop {
                if let Ok(next) = socket.recv().await {
                    view.set(Some(next));
                    received_at.set(client::now_ms());
                    connected.set(true);
                } else {
                    connected.set(false);
                    client::sleep_ms(RECONNECT_DELAY_MS).await;
                    socket.set(connect().await);
                }
            }
        }
    });
    Live::new(view, received_at, connected, commands)
}

/// The browser clock, refreshed every `period_ms`.
pub fn use_clock(period_ms: u32) -> Signal<f64> {
    let mut now = use_signal(client::now_ms);
    use_future(move || async move {
        loop {
            client::sleep_ms(period_ms).await;
            now.set(client::now_ms());
        }
    });
    now
}

/// Milliseconds left on a server countdown, adjusted for time elapsed since it was received.
#[must_use]
pub fn remaining_ms(server_ms: u64, received_at: f64, now: f64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "countdowns are far below 2^52 ms"
    )]
    let server_ms = server_ms as f64;
    (server_ms - (now - received_at)).max(0.0)
}

/// Like [`remaining_ms`], but frozen while the host has paused the game.
#[must_use]
pub fn countdown_ms(server_ms: u64, received_at: f64, now: f64, paused: bool) -> f64 {
    if paused {
        remaining_ms(server_ms, received_at, received_at)
    } else {
        remaining_ms(server_ms, received_at, now)
    }
}

/// Clock time spent on a question at browser time `now`, from the server's count when the view
/// arrived; frozen while paused.
#[must_use]
pub fn elapsed_now(server_ms: u64, received_at: f64, now: f64, paused: bool) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "countdowns are far below 2^52 ms"
    )]
    let server_ms = server_ms as f64;
    if paused {
        server_ms
    } else {
        server_ms + (now - received_at).max(0.0)
    }
}

/// A small "reconnecting" banner shown while the socket is down.
#[component]
pub fn ConnectionBanner(connected: bool) -> Element {
    if connected {
        return rsx! {};
    }
    rsx! {
        div { class: "connection", role: "status", "Reconnecting…" }
    }
}
