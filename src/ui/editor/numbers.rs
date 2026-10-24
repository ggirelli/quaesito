//! Number questions: the answer, its range and how guesses are scored.

use dioxus::prelude::*;

use super::Draft;
use super::answers::Choice;
use crate::model::{NumberRange, NumberScoring, QuestionKind};

/// The correct number of a number question, and the range teams answer in.
#[component]
pub(super) fn NumberAnswer(
    question: usize,
    answer: Option<f64>,
    range: Option<NumberRange>,
) -> Element {
    let draft: Draft = use_context();
    // Keep what is typed, so "3." or "-" survive until the number is complete.
    let shown = |n: Option<f64>| n.map(|n| n.to_string()).unwrap_or_default();
    let mut raw = use_signal(|| shown(answer));
    let mut raw_min = use_signal(|| shown(range.map(|r| r.min)));
    let mut raw_max = use_signal(|| shown(range.map(|r| r.max)));
    // The range is set once both ends are numbers, and cleared when both are emptied.
    let set_range = move || {
        let range = match (parse_number(&raw_min()), parse_number(&raw_max())) {
            (Some(min), Some(max)) => Some(NumberRange { min, max }),
            _ => None,
        };
        draft.edit_question(question, |q| {
            if let QuestionKind::Number { range: old, .. } = &mut q.kind {
                *old = range;
            }
        });
    };
    rsx! {
        div { class: "row number-edit",
            label { class: "field inline",
                span { "Correct number" }
                input {
                    r#type: "text",
                    inputmode: "decimal",
                    class: "short",
                    placeholder: "330",
                    value: raw,
                    oninput: move |e| {
                        let value = e.value();
                        let parsed = parse_number(&value);
                        raw.set(value);
                        draft.edit_question(question, |q| {
                            if let QuestionKind::Number { answer, .. } = &mut q.kind {
                                *answer = parsed;
                            }
                        });
                    },
                }
            }
            label { class: "field inline",
                title: "Optional: teams answer within this range, with a slider on their phones",
                span { "Range" }
                input {
                    r#type: "text",
                    inputmode: "decimal",
                    class: "short",
                    placeholder: "from",
                    aria_label: "Range from",
                    value: raw_min,
                    oninput: move |e| {
                        raw_min.set(e.value());
                        set_range();
                    },
                }
                span { "–" }
                input {
                    r#type: "text",
                    inputmode: "decimal",
                    class: "short",
                    placeholder: "to",
                    aria_label: "Range to",
                    value: raw_max,
                    oninput: move |e| {
                        raw_max.set(e.value());
                        set_range();
                    },
                }
            }
        }
        if let Some(problem) = range_problem(answer, range) {
            p { class: "field-error", "{problem}" }
        }
    }
}

/// How a number question is scored, with the distances its scoring needs.
#[component]
pub(super) fn NumberScoringChoice(
    question: usize,
    answer: Option<f64>,
    range: Option<NumberRange>,
    scoring: NumberScoring,
) -> Element {
    let draft: Draft = use_context();
    let set_scoring = move |scoring: NumberScoring| {
        draft.edit_question(question, |q| {
            if let QuestionKind::Number { scoring: s, .. } = &mut q.kind {
                *s = scoring;
            }
        });
    };
    rsx! {
        Choice {
            label: "Scoring",
            options: vec![
                ("Closest wins", "The closest guesses win 1000 points, however long they took; the others none."),
                ("Closer earns more", "Full points up to the first distance from the correct number, none beyond the second, and fewer the farther in between."),
                ("Close enough", "Full points up to a margin from the correct number (0: only the exact number), none beyond."),
            ],
            selected: match scoring {
                NumberScoring::Closest => 0,
                NumberScoring::Distance { .. } => 1,
                NumberScoring::Margin { .. } => 2,
            },
            on_select: move |i| set_scoring(match (i, scoring) {
                (1, NumberScoring::Distance { .. }) | (2, NumberScoring::Margin { .. }) => scoring,
                (1, _) => NumberScoring::Distance {
                    full_within: 0.0,
                    none_beyond: default_spread(answer, range),
                },
                (2, _) => NumberScoring::Margin { margin: 0.0 },
                _ => NumberScoring::Closest,
            }),
        }
        match scoring {
            NumberScoring::Closest => rsx! {},
            NumberScoring::Distance { full_within, none_beyond } => rsx! {
                div { class: "row number-edit",
                    DistanceField {
                        key: "full-within",
                        label: "Full points within",
                        value: full_within,
                        on_change: move |full_within| set_scoring(NumberScoring::Distance { full_within, none_beyond }),
                    }
                    DistanceField {
                        key: "none-beyond",
                        label: "No points beyond",
                        value: none_beyond,
                        on_change: move |none_beyond| set_scoring(NumberScoring::Distance { full_within, none_beyond }),
                    }
                }
            },
            NumberScoring::Margin { margin } => rsx! {
                div { class: "row number-edit",
                    DistanceField {
                        key: "margin",
                        label: "Right within ±",
                        value: margin,
                        on_change: move |margin| set_scoring(NumberScoring::Margin { margin }),
                    }
                }
            },
        }
        if let Some(problem) = scoring.problem() {
            p { class: "field-error", "{problem}" }
        }
    }
}

/// A starting "no points beyond" distance: a quarter of the range, or a tenth of the answer.
fn default_spread(answer: Option<f64>, range: Option<NumberRange>) -> f64 {
    let spread = match (range, answer) {
        (Some(range), _) if range.is_valid() => (range.max - range.min) / 4.0,
        (_, Some(answer)) => answer.abs() / 10.0,
        _ => 10.0,
    };
    spread.round().max(1.0)
}

/// A distance from the correct number, kept as typed until it is a number.
#[component]
fn DistanceField(label: &'static str, value: f64, on_change: EventHandler<f64>) -> Element {
    let mut raw = use_signal(|| value.to_string());
    rsx! {
        label { class: "field inline",
            span { "{label}" }
            input {
                r#type: "text",
                inputmode: "decimal",
                class: "short",
                value: raw,
                oninput: move |e| {
                    let value = e.value();
                    if let Some(distance) = parse_number(&value) {
                        on_change.call(distance);
                    }
                    raw.set(value);
                },
            }
        }
    }
}

/// What is wrong with a number question's range, to show right under it.
fn range_problem(answer: Option<f64>, range: Option<NumberRange>) -> Option<&'static str> {
    let range = range?;
    if !range.is_valid() {
        Some("The range must go from a smaller to a larger number.")
    } else if answer.is_some_and(|a| !range.contains(a)) {
        Some("The correct number must be within the range.")
    } else {
        None
    }
}

/// A typed number, with a comma or a point as decimal separator.
fn parse_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}
