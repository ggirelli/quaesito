//! The pad of order questions.

use dioxus::prelude::*;

use super::Clock;
use super::controls::{BonusField, JokerToggle};
use crate::protocol::{JokerState, PlayerCommand, PlayerView, Response};
use crate::ui::live::{Live, use_clock};

/// An order question: tap the items from first to last, then submit.
#[component]
pub(super) fn OrderPad(
    index: usize,
    total: usize,
    items: Vec<String>,
    question: Option<String>,
    clock: Clock,
    submitted: Option<Vec<usize>>,
    joker: JokerState,
    bonus: Option<(String, u32)>,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut picked = use_signal(Vec::<usize>::new);
    let bonus_text = use_signal(String::new);
    let has_bonus = bonus.is_some();
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;
    let order = submitted.clone().unwrap_or_else(|| picked.cloned());
    let complete = picked.read().len() == items.len();
    let send = move |_| {
        live_socket.send(PlayerCommand::Answer {
            response: Response::Order(picked()),
            joker: armed(),
            bonus: has_bonus.then(|| bonus_text.cloned()),
        });
    };
    rsx! {
        main { class: "player-main top",
            div { class: "phone-timer",
                div { style: "width: {width}%" }
            }
            div { class: "row between",
                span { class: "muted", "Question {index + 1} of {total}" }
                span { class: "muted",
                    if clock.paused { "⏸ Paused" } else { "{(left / 1000.0).ceil()}s" }
                }
            }
            if let Some(question) = question {
                p { class: "phone-question", "{question}" }
            }
            if locked {
                p { class: "strong",
                    if submitted.is_some() { "🔒 Answer locked in" } else { "⌛ Time's up" }
                }
            } else {
                p { class: "muted", "Tap the items from first to last. Tap again to take one back." }
            }
            JokerToggle { state: joker, armed, locked }
            if let Some(bonus) = bonus.clone() {
                BonusField { bonus, value: bonus_text, locked }
            }
            ul { class: "order-pad",
                for (i, item) in items.iter().enumerate() {
                    {
                        let position = order.iter().position(|&p| p == i);
                        rsx! {
                            li { key: "{i}",
                                button {
                                    class: if position.is_some() { "order-choice picked" } else { "order-choice" },
                                    disabled: locked,
                                    onclick: move |_| picked.with_mut(|p| {
                                        if let Some(at) = p.iter().position(|&x| x == i) {
                                            p.remove(at);
                                        } else {
                                            p.push(i);
                                        }
                                    }),
                                    span { class: "order-rank",
                                        if let Some(position) = position { "{position + 1}" } else { "·" }
                                    }
                                    span { "{item}" }
                                }
                            }
                        }
                    }
                }
            }
            if !locked {
                div { class: "row",
                    button {
                        class: "button ghost",
                        disabled: picked.read().is_empty(),
                        onclick: move |_| picked.set(Vec::new()),
                        "Start over"
                    }
                    button {
                        class: "button primary big grow",
                        disabled: !complete,
                        onclick: send,
                        "Submit"
                    }
                }
            }
        }
    }
}
