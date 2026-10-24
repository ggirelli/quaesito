//! The table view of a quiz, whose rows can be dragged to reorder it.

use dioxus::prelude::*;

use super::navigator::anchor;
use crate::model::{Question, QuestionKind, Quiz, ValidationIssue};

/// A question or slide as the table shows it.
#[derive(Clone, PartialEq)]
pub(super) struct TableRow {
    anchor: String,
    /// "12", or "S2" for a slide.
    number: String,
    kind: &'static str,
    /// The question, or the slide's title (its text when untitled).
    text: String,
    /// The time limit, and whether it is the quiz's default.
    time: Option<(u32, bool)>,
    multiplier: Option<u32>,
    category: String,
    topic: String,
    /// What keeps it from being played, one per line.
    problems: String,
}

impl TableRow {
    pub(super) fn new(quiz: &Quiz, index: usize, issues: &[ValidationIssue]) -> Self {
        let question = &quiz.questions[index];
        let number = quiz.ordinal(index) + 1;
        let slide = question.is_slide();
        let text = match &question.kind {
            QuestionKind::Slide { title, .. } if !title.trim().is_empty() => title.trim(),
            _ => question.text.trim(),
        };
        Self {
            anchor: anchor(&quiz.label(index)),
            number: if slide {
                format!("S{number}")
            } else {
                number.to_string()
            },
            kind: kind_name(question),
            text: text.to_owned(),
            time: (!slide).then(|| {
                (
                    question.time_limit_secs(&quiz.settings),
                    question.time_limit_secs.is_none(),
                )
            }),
            multiplier: question.multiplier.filter(|&m| m > 1),
            category: question.category.trim().to_owned(),
            topic: question.topic.trim().to_owned(),
            problems: issues
                .iter()
                .filter(|issue| issue.question == Some(index))
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// The answer type, named as in the editor.
fn kind_name(question: &Question) -> &'static str {
    match question.kind {
        QuestionKind::Choice => "Choices",
        QuestionKind::TrueFalse { .. } => "True/false",
        QuestionKind::Text { .. } => "Typed answer",
        QuestionKind::Number { .. } => "Number",
        QuestionKind::Order { .. } => "Order",
        QuestionKind::Parts { .. } => "Several answers",
        QuestionKind::Pin { .. } => "Pin",
        QuestionKind::Pairs { .. } => "Pairs",
        QuestionKind::Slide { results: true, .. } => "Results slide",
        QuestionKind::Slide { .. } => "Slide",
    }
}

/// The whole quiz in a table, one row per question or slide; a row opens its card, and held and
/// dragged moves it: `on_move` gets where it was and where it goes.
#[component]
pub(super) fn QuizTable(
    rows: Vec<TableRow>,
    on_open: EventHandler<String>,
    on_move: EventHandler<(usize, usize)>,
) -> Element {
    use_effect(move || {
        spawn(async move {
            let mut drag = document::eval(TABLE_DRAG_JS);
            while let Ok(moved) = drag.recv::<(usize, usize)>().await {
                on_move.call(moved);
            }
        });
    });
    use_drop(|| {
        document::eval("window.qtable?.stop()");
    });
    rsx! {
        div { class: "card quiz-table-card",
            table { class: "quiz-table",
                thead {
                    tr {
                        th { "#" }
                        th { class: "col-kind", "Type" }
                        th { class: "col-subject", "Category" }
                        th { class: "col-subject", "Topic" }
                        th { class: "col-number", "Time" }
                        th { class: "col-number col-points", "Points" }
                        th { "Question" }
                    }
                }
                tbody {
                    for row in rows {
                        tr {
                            key: "{row.anchor}",
                            id: "{row.anchor}",
                            class: if row.time.is_none() { "slide-row" },
                            onclick: {
                                let anchor = row.anchor.clone();
                                move |_| on_open.call(anchor.clone())
                            },
                            td { class: "col-index",
                                a {
                                    href: "#{row.anchor}",
                                    title: "Open its card",
                                    draggable: "false",
                                    onclick: {
                                        let anchor = row.anchor.clone();
                                        move |e: MouseEvent| {
                                            e.prevent_default();
                                            e.stop_propagation();
                                            on_open.call(anchor.clone());
                                        }
                                    },
                                    "{row.number}"
                                }
                                if !row.problems.is_empty() {
                                    span {
                                        class: "table-problem",
                                        title: "{row.problems}",
                                        aria_label: "Not ready to play",
                                        "●"
                                    }
                                }
                            }
                            td { class: "col-kind", "{row.kind}" }
                            td { class: "col-subject", "{row.category}" }
                            td { class: "col-subject", "{row.topic}" }
                            td { class: "col-number",
                                if let Some((secs, inherited)) = row.time {
                                    span {
                                        class: if inherited { "muted" },
                                        title: if inherited { "The quiz's default" },
                                        "{secs}s"
                                    }
                                }
                            }
                            td { class: "col-number col-points",
                                if let Some(times) = row.multiplier { "×{times}" }
                            }
                            td { class: "col-text", title: "{row.text}",
                                div { class: "single-line", "{row.text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Moves a row of the quiz table where it is dragged: a mouse drags once it moves with the button
/// held, a finger once it has held still for a moment (sliding at once scrolls the page). The other
/// rows make room as it goes; Escape puts it back. Sends `[from, to]` when it is dropped elsewhere.
const TABLE_DRAG_JS: &str = include_str!("table-drag.js");
