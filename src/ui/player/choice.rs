//! The pad of choice questions.

use dioxus::prelude::*;

use super::Clock;
use super::controls::{BonusField, JokerToggle};
use crate::ui::{
    live::{Live, use_clock},
    widgets::{ANSWER_STYLES, AnswerShape, media_url},
};
use crate::{
    model::PhoneDisplay,
    protocol::{JokerState, PlayerCommand, PlayerView, Response},
};

#[component]
pub(super) fn AnswerPad(
    index: usize,
    total: usize,
    answer_count: usize,
    /// The answers from first to last, by index; empty keeps their own order.
    order: Vec<usize>,
    text: Option<(String, Vec<String>)>,
    /// Answer labels shown whatever the phone display (True/False).
    labels: Vec<String>,
    images: Vec<Option<String>>,
    multi: bool,
    clock: Clock,
    submitted: Option<Vec<usize>>,
    display: PhoneDisplay,
    joker: JokerState,
    bonus: Option<(String, u32)>,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut selected = use_signal(Vec::<usize>::new);
    let bonus_text = use_signal(String::new);
    let has_bonus = bonus.is_some();
    // With a bonus to type, a single answer is picked and then submitted, as several are.
    let pick_then_submit = multi || has_bonus;
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;
    let chosen = submitted.clone().unwrap_or_else(|| selected.cloned());

    let send = move |answer: Vec<usize>| {
        live_socket.send(PlayerCommand::Answer {
            response: Response::Choice(answer),
            joker: armed(),
            bonus: has_bonus.then(|| bonus_text.cloned()),
        });
    };
    let (question, labels) = match text {
        Some((question, texts)) => (Some(question), Some(texts)),
        None => (None, (!labels.is_empty()).then_some(labels)),
    };
    let with_text = labels.is_some() && (display == PhoneDisplay::FullText || question.is_none());
    // Each answer keeps its index (shape, colour and what is sent), wherever it is listed.
    let order = if order.len() == answer_count {
        order
    } else {
        (0..answer_count).collect()
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
            } else if multi {
                p { class: "muted", "Pick every correct answer, then submit." }
            } else if has_bonus {
                p { class: "muted", "Pick an answer, type the bonus, then submit." }
            }
            JokerToggle { state: joker, armed, locked }
            if let Some(bonus) = bonus.clone() {
                BonusField { bonus, value: bonus_text, locked }
            }
            div { class: if with_text { "phone-answers with-text" } else { "phone-answers" },
                for i in order {
                    {
                        let mut class = format!("phone-answer a{}", i % ANSWER_STYLES);
                        if chosen.contains(&i) {
                            class.push_str(" selected");
                        } else if locked {
                            class.push_str(" faded");
                        }
                        let label = labels
                            .as_ref()
                            .and_then(|l| l.get(i).cloned())
                            .filter(|l| !l.is_empty());
                        let aria_label = label.clone().unwrap_or_else(|| format!("Answer {}", i + 1));
                        let image = images.get(i).cloned().flatten();
                        rsx! {
                            button {
                                key: "{i}",
                                class,
                                disabled: locked,
                                aria_label,
                                onclick: move |_| {
                                    if multi {
                                        selected.with_mut(|s| {
                                            if let Some(pos) = s.iter().position(|&x| x == i) {
                                                s.remove(pos);
                                            } else {
                                                s.push(i);
                                            }
                                        });
                                    } else if has_bonus {
                                        selected.set(vec![i]);
                                    } else {
                                        selected.set(vec![i]);
                                        send(vec![i]);
                                    }
                                },
                                AnswerShape { index: i }
                                if let Some(image) = image {
                                    img { class: "phone-answer-image", src: media_url(&image), alt: "" }
                                }
                                if let Some(label) = label {
                                    span { "{label}" }
                                }
                            }
                        }
                    }
                }
            }
            if pick_then_submit && !locked {
                button {
                    class: "button primary big",
                    disabled: selected().is_empty(),
                    onclick: move |_| send(selected()),
                    "Submit"
                }
            }
        }
    }
}
