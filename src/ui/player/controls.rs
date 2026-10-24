//! What every answer pad shares: the joker and the bonus question's field.

use dioxus::prelude::*;

use crate::{model::MAX_TEXT_ANSWER_CHARS, protocol::JokerState};

#[component]
/// The bonus question's field, sent together with the answer.
#[component]
pub(super) fn BonusField(bonus: (String, u32), value: Signal<String>, locked: bool) -> Element {
    let (text, percent) = bonus;
    rsx! {
        label { class: "field bonus-field",
            span {
                strong { "Bonus +{percent}%" }
                " {text}"
            }
            input {
                r#type: "text",
                autocomplete: "off",
                autocapitalize: "off",
                spellcheck: false,
                maxlength: MAX_TEXT_ANSWER_CHARS,
                disabled: locked,
                placeholder: if locked { "" } else { "Optional" },
                value: "{value}",
                oninput: move |e| value.set(e.value()),
            }
        }
    }
}

/// Arms the team's joker before answering, or shows that it was played on this question.
#[component]
pub(super) fn JokerToggle(state: JokerState, armed: Signal<bool>, locked: bool) -> Element {
    match state {
        JokerState::Ready if !locked => rsx! {
            button {
                r#type: "button",
                class: if armed() { "joker armed" } else { "joker" },
                aria_pressed: armed(),
                onclick: move |_| armed.toggle(),
                if armed() { "🃏 Joker on: this answer counts double" } else { "🃏 Play my joker (double points)" }
            }
        },
        JokerState::PlayedHere => rsx! {
            p { class: "joker armed", "🃏 Joker played: points count double" }
        },
        _ => rsx! {},
    }
}
