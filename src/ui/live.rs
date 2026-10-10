//! Live connection to a session: the latest view pushed by the server, with reconnection.

use std::future::Future;

use dioxus::{
    fullstack::{UseWebsocket, Websocket, use_websocket},
    prelude::*,
};
use serde::{Serialize, de::DeserializeOwned};

use super::client;

/// Pause before reconnecting a dropped socket.
const RECONNECT_DELAY_MS: u32 = 1500;

/// A live socket and what it last received.
pub struct Live<In: 'static, Out: 'static> {
    pub socket: UseWebsocket<In, Out>,
    /// The latest view, `None` until the first one arrives.
    pub view: Signal<Option<Out>>,
    /// Browser time (ms) when `view` arrived, to run countdowns locally.
    pub received_at: Signal<f64>,
    /// Whether the socket is currently up.
    pub connected: Signal<bool>,
}

impl<In, Out> Clone for Live<In, Out> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<In, Out> Copy for Live<In, Out> {}

/// Connects with `connect` and keeps the connection alive.
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
    Live {
        socket,
        view,
        received_at,
        connected,
    }
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
