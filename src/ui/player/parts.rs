//! The pad of questions in parts.

use dioxus::prelude::*;

use super::Clock;
use super::controls::JokerToggle;
use crate::ui::live::{Live, use_clock};
use crate::{
    model::MAX_TEXT_ANSWER_CHARS,
    protocol::{JokerState, PlayerCommand, PlayerView, Response},
};

/// A several-answers question: one field per part, submitted together.
#[component]
pub(super) fn PartsPad(
    index: usize,
    total: usize,
    labels: Vec<String>,
    question: Option<String>,
    clock: Clock,
    submitted: Option<Vec<String>>,
    joker: JokerState,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let count = labels.len();
    let mut drafts = use_signal(|| vec![String::new(); count]);
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;
    let anything = drafts.read().iter().any(|d| !d.trim().is_empty());
    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        live_socket.send(PlayerCommand::Answer {
            response: Response::Parts(drafts()),
            joker: armed(),
            bonus: None,
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
                (Some(answers), _) => rsx! {
                    p { class: "strong", "🔒 Answers locked in" }
                    JokerToggle { state: joker, armed, locked }
                    ul { class: "locked-parts",
                        for (i, label) in labels.iter().enumerate() {
                            li { key: "{i}",
                                span { class: "muted small", "{label}" }
                                span { class: "strong",
                                    match answers.get(i).map(|a| a.trim()).filter(|a| !a.is_empty()) {
                                        Some(answer) => answer.to_owned(),
                                        None => "–".to_owned(),
                                    }
                                }
                            }
                        }
                    }
                },
                (None, true) => rsx! {
                    p { class: "strong", "⌛ Time's up" }
                },
                (None, false) => rsx! {
                    form { class: "stack open-form", onsubmit: submit,
                        for (i, label) in labels.iter().enumerate() {
                            label { key: "{i}", class: "field",
                                span { "{i + 1}. {label}" }
                                input {
                                    r#type: "text",
                                    autocomplete: "off",
                                    autocapitalize: "off",
                                    spellcheck: false,
                                    maxlength: MAX_TEXT_ANSWER_CHARS,
                                    value: "{drafts.read().get(i).cloned().unwrap_or_default()}",
                                    oninput: move |e| drafts.with_mut(|d| d[i] = e.value()),
                                }
                            }
                        }
                        JokerToggle { state: joker, armed, locked }
                        button {
                            r#type: "submit",
                            class: "button primary big",
                            disabled: !anything,
                            "Submit"
                        }
                    }
                },
            }
        }
    }
}
