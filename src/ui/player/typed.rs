//! The pad of questions answered by typing: text and numbers.

use dioxus::prelude::*;

use super::Clock;
use super::controls::{BonusField, JokerToggle};
use crate::ui::{
    live::{Live, use_clock},
    widgets::ErrorBox,
};
use crate::{
    model::{MAX_TEXT_ANSWER_CHARS, NumberRange},
    protocol::{JokerState, PlayerCommand, PlayerView, Response},
};

/// A typed or numeric answer: a field and a submit button.
#[component]
pub(super) fn OpenPad(
    index: usize,
    total: usize,
    number: bool,
    question: Option<String>,
    clock: Clock,
    submitted: Option<Response>,
    joker: JokerState,
    bonus: Option<(String, u32)>,
    range: Option<NumberRange>,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut draft = use_signal(String::new);
    let bonus_text = use_signal(String::new);
    let has_bonus = bonus.is_some();
    let armed = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        let text = draft().trim().to_owned();
        let response = if number {
            // Phones with a comma as decimal separator may still send one.
            match text.replace(',', ".").parse::<f64>() {
                Ok(value) if value.is_finite() => Response::Number(value),
                _ => {
                    error.set(Some("Please enter a number.".to_owned()));
                    return;
                }
            }
        } else if text.is_empty() {
            error.set(Some("Please type an answer.".to_owned()));
            return;
        } else {
            Response::Text(text)
        };
        error.set(None);
        live_socket.send(PlayerCommand::Answer {
            response,
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
            match (&submitted, locked) {
                (Some(answer), _) => rsx! {
                    p { class: "strong", "🔒 Answer locked in" }
                    JokerToggle { state: joker, armed, locked }
                    p { class: "locked-answer",
                        match answer {
                            Response::Text(text) => text.clone(),
                            Response::Number(value) => value.to_string(),
                            Response::Choice(_)
                            | Response::Order(_)
                            | Response::Parts(_)
                            | Response::Pin(_)
                            | Response::Pairs(_) => String::new(),
                        }
                    }
                },
                (None, true) => rsx! {
                    p { class: "strong", "⌛ Time's up" }
                },
                (None, false) => rsx! {
                    form { class: "stack open-form", onsubmit: submit,
                        if number {
                            if let Some(range) = range {
                                // The slider for a quick guess, the field for an exact one.
                                div { class: "number-slider",
                                    input {
                                        r#type: "range",
                                        min: "{range.min}",
                                        max: "{range.max}",
                                        step: if range.whole() { "1" } else { "any" },
                                        aria_label: "Your guess",
                                        value: draft().trim().replace(',', ".").parse::<f64>().unwrap_or(f64::midpoint(range.min, range.max)),
                                        oninput: move |e| draft.set(e.value()),
                                    }
                                    div { class: "row between muted small",
                                        span { "{range.min}" }
                                        span { "{range.max}" }
                                    }
                                }
                            }
                            input {
                                r#type: "text",
                                inputmode: "decimal",
                                autocomplete: "off",
                                placeholder: match range {
                                    Some(range) => format!("{} to {}", range.min, range.max),
                                    None => "Your guess".to_owned(),
                                },
                                aria_label: "Your guess",
                                value: draft,
                                oninput: move |e| draft.set(e.value()),
                            }
                        } else {
                            input {
                                r#type: "text",
                                autocomplete: "off",
                                autocapitalize: "off",
                                spellcheck: false,
                                maxlength: MAX_TEXT_ANSWER_CHARS,
                                placeholder: "Your answer",
                                aria_label: "Your answer",
                                value: draft,
                                oninput: move |e| draft.set(e.value()),
                            }
                        }
                        if let Some(bonus) = bonus.clone() {
                            BonusField { bonus, value: bonus_text, locked }
                        }
                        ErrorBox { error: error() }
                        JokerToggle { state: joker, armed, locked }
                        button {
                            r#type: "submit",
                            class: "button primary big",
                            disabled: draft().trim().is_empty(),
                            "Submit"
                        }
                    }
                },
            }
        }
    }
}
