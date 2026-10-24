//! The team leader's phone: join a game, answer questions, see results.

use dioxus::prelude::*;

use super::{
    live::{ConnectionBanner, Live, countdown_ms},
    widgets::{Spinner, TeamAvatar},
};
use crate::protocol::{InputKind, PlayerCommand, PlayerPhase, PlayerView, Response};

mod choice;
#[cfg(feature = "fullstack")]
mod connected;
mod controls;
#[cfg(feature = "fullstack")]
mod join;
#[cfg(feature = "fullstack")]
mod lobby;
mod order;
mod outcome;
mod pairs;
mod parts;
mod pins;
mod typed;

#[cfg(feature = "fullstack")]
use connected::PlayerGame;
#[cfg(feature = "fullstack")]
pub use join::{Join, JoinBlank, JoinCodeForm};

use choice::AnswerPad;
use order::OrderPad;
use outcome::OutcomeView;
use pairs::PairsPad;
use parts::PartsPad;
use pins::PinPad;
use typed::OpenPad;

/// A team leader's phone in a live game, whatever drives it: plays the [`Live`] game in the
/// context, and leaves the lobby and the way out to its caller.
#[component]
pub fn PlayerBoard(
    /// What the phone shows while teams join.
    lobby: Element,
    /// Shown under the final rank, once the quiz is over.
    done: Element,
) -> Element {
    let live: Live<PlayerCommand, PlayerView> = use_context();
    let view = live.view.read();
    let Some(view) = view.as_ref() else {
        return rsx! {
            div { class: "page center", Spinner {} }
        };
    };
    rsx! {
        div { class: "player",
            header { class: "player-bar",
                div { class: "team",
                    TeamAvatar { team: view.team.clone() }
                    span { "{view.team.name}" }
                }
                if let Some(score) = view.score {
                    span { class: "player-score", "{score} pts" }
                }
            }
            PhaseView { view: view.clone(), live_received: (live.received_at)(), lobby, done }
            ConnectionBanner { connected: (live.connected)() }
        }
    }
}

#[component]
fn PhaseView(view: PlayerView, live_received: f64, lobby: Element, done: Element) -> Element {
    match view.phase {
        PlayerPhase::Lobby { .. } => lobby,
        PlayerPhase::Slide => rsx! {
            main { class: "player-main",
                div { class: "big-icon", "👀" }
                h1 { class: "status-title", "Eyes on the big screen" }
                p { class: "muted", "Nothing to answer right now." }
            }
        },
        PlayerPhase::Ready { index, total } => rsx! {
            main { class: "player-main",
                span { class: "muted", "Question {index + 1} of {total}" }
                div { class: "big-icon", "🎵" }
                h1 { class: "status-title", "Listen up!" }
                p { class: "muted", "Answers open when the music starts." }
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: input @ (InputKind::Text | InputKind::Number),
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            bonus,
            range,
            ..
        } => rsx! {
            OpenPad {
                range,
                bonus,
                key: "{index}",
                joker: view.joker,
                index,
                total,
                number: input == InputKind::Number,
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted,
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Pin,
            map,
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            bonus,
            ..
        } => rsx! {
            PinPad {
                bonus,
                key: "{index}",
                joker: view.joker,
                index,
                total,
                map: map.unwrap_or_default(),
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Pin(at)) => Some(at),
                    _ => None,
                },
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Parts,
            items,
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            ..
        } => rsx! {
            PartsPad {
                key: "{index}",
                joker: view.joker,
                index,
                total,
                labels: items,
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Parts(texts)) => Some(texts),
                    _ => None,
                },
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Pairs,
            pairs,
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            bonus,
            ..
        } => rsx! {
            PairsPad {
                bonus,
                key: "{index}",
                joker: view.joker,
                index,
                total,
                columns: pairs.unwrap_or_default(),
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Pairs(links)) => Some(links),
                    _ => None,
                },
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Order,
            items,
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            bonus,
            ..
        } => rsx! {
            OrderPad {
                bonus,
                key: "{index}",
                joker: view.joker,
                index,
                total,
                items,
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Order(order)) => Some(order),
                    _ => None,
                },
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Choice,
            answer_count,
            order,
            items,
            text,
            answer_images,
            map: _,
            multi,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            bonus,
            range: _,
            pairs: _,
        } => rsx! {
            AnswerPad {
                bonus,
                key: "{index}",
                joker: view.joker,
                index,
                total,
                answer_count,
                order,
                text,
                labels: items,
                images: answer_images,
                multi,
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Choice(selected)) => Some(selected),
                    _ => None,
                },
                display: view.display,
            }
        },
        PlayerPhase::Closed {
            index,
            total,
            submitted,
            result,
        } => match result {
            Some((outcome, _)) => rsx! {
                OutcomeView { index, total, outcome, answered: submitted.is_some() }
            },
            None => rsx! {
                main { class: "player-main",
                    span { class: "muted", "Question {index + 1} of {total}" }
                    if submitted.is_some() {
                        div { class: "big-icon", "🔒" }
                        h1 { class: "status-title", "Answer locked in" }
                        p { class: "muted", "Results are revealed at the end. Fingers crossed!" }
                    } else {
                        div { class: "big-icon", "⌛" }
                        h1 { class: "status-title", "Time's up" }
                        p { class: "muted", "No answer this time." }
                    }
                }
            },
        },
        PlayerPhase::Review {
            index,
            total,
            submitted,
            outcome,
            ..
        } => rsx! {
            OutcomeView { index, total, outcome, answered: submitted.is_some() }
        },
        PlayerPhase::Standings { rank, teams } => rsx! {
            main { class: "player-main",
                div { class: "big-icon", "📊" }
                h1 { class: "status-title", "You're #{rank} of {teams} so far" }
                p { class: "muted", "The standings are on the big screen." }
            }
        },
        PlayerPhase::Podium {
            rank,
            teams,
            finished,
        } => rsx! {
            main { class: "player-main",
                match rank {
                    Some(rank) => rsx! {
                        div { class: "big-icon",
                            match rank {
                                1 => "🏆",
                                2 => "🥈",
                                3 => "🥉",
                                _ => "🎉",
                            }
                        }
                        h1 { class: "status-title", "You finished #{rank} of {teams}" }
                    },
                    None => rsx! {
                        div { class: "big-icon", "🥁" }
                        h1 { class: "status-title", "Drumroll…" }
                        p { class: "muted", "Watch the big screen!" }
                    },
                }
                if finished {
                    p { class: "muted", "Thanks for playing!" }
                    {done}
                }
            }
        },
    }
}

/// A question's countdown as last received from the server.
#[derive(Clone, Copy, PartialEq)]
struct Clock {
    left_ms: u64,
    limit_ms: u64,
    paused: bool,
    /// Browser time the view arrived.
    received_at: f64,
}

impl Clock {
    /// Milliseconds left at browser time `now`, and the timer bar's width in percent.
    fn at(self, now: f64) -> (f64, f64) {
        let left = countdown_ms(self.left_ms, self.received_at, now, self.paused);
        #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
        let limit = self.limit_ms.max(1) as f64;
        (left, (left / limit * 100.0).clamp(0.0, 100.0))
    }
}
