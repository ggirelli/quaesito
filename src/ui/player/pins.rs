//! The pad of pin questions.

use dioxus::prelude::*;

use super::Clock;
use super::controls::{BonusField, JokerToggle};
use crate::ui::{
    live::{Live, use_clock},
    widgets::{DOT, MapPin, PinMap},
};
use crate::{
    model::Point,
    protocol::{JokerState, PlayerCommand, PlayerView, Response},
};

/// A pin question: tap the picture to drop (or move) the pin, then submit.
#[component]
pub(super) fn PinPad(
    index: usize,
    total: usize,
    map: (String, f64),
    question: Option<String>,
    clock: Clock,
    submitted: Option<Point>,
    joker: JokerState,
    bonus: Option<(String, u32)>,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut pin = use_signal(|| None::<Point>);
    let bonus_text = use_signal(String::new);
    let has_bonus = bonus.is_some();
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;
    let shown = submitted.or(pin());
    let send = move |_| {
        if let Some(at) = pin() {
            live_socket.send(PlayerCommand::Answer {
                response: Response::Pin(at),
                joker: armed(),
                bonus: has_bonus.then(|| bonus_text.cloned()),
            });
        }
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
                    if submitted.is_some() { "🔒 Pin locked in" } else { "⌛ Time's up" }
                }
            } else {
                p { class: "muted",
                    if pin().is_some() { "Tap again to move your pin, then submit." } else { "Tap the picture to drop your pin." }
                }
            }
            JokerToggle { state: joker, armed, locked }
            if let Some(bonus) = bonus.clone() {
                BonusField { bonus, value: bonus_text, locked }
            }
            PinMap {
                image: map.0,
                aspect: map.1,
                pin_scale: 2.0,
                pins: shown
                    .map(|at| MapPin {
                        at,
                        label: DOT.to_owned(),
                        title: "Your pin".to_owned(),
                        class: "own",
                    })
                    .into_iter()
                    .collect::<Vec<_>>(),
                on_pick: (!locked).then_some(EventHandler::new(move |(at, _): (Point, f64)| pin.set(Some(at)))),
            }
            if !locked {
                button {
                    class: "button primary big",
                    disabled: pin().is_none(),
                    onclick: send,
                    "Submit"
                }
            }
        }
    }
}
