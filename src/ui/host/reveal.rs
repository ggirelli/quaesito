//! Revealing a question's answer and how the teams answered, for each kind of question.

use dioxus::prelude::*;

use super::question::{AnswerGrid, QuestionHeader};
use super::standings::{STANDINGS_SHOWN, Standings};
use super::tiles::FullPicture;
use super::{seconds, use_precise_times};
use crate::ui::{
    live::Live,
    widgets::{
        AnswerBadge, MapPin, PairItemContent, PinMap, QuestionMedia, STAR, media_url, pair_letter,
    },
};
use crate::{
    model::{Pair, Point},
    protocol::{
        Explanation, Fastest, Grade, Guess, HostCommand, HostView, Judgement, PairColumns,
        PartReveal, PinGuess, QuestionInfo, Reveal, StandingRow, TextGroup,
    },
    scoring::MAX_BONUS,
};

/// How many of the closest pins the reveal lists.
const PINS_LISTED: usize = 6;

/// The right answer and what teams answered, per question kind.
#[component]
fn RevealBody(question: QuestionInfo, reveal: Reveal) -> Element {
    match reveal {
        Reveal::Choice { correct, picks } => rsx! {
            AnswerGrid {
                answers: question.answers.clone(),
                images: question.answer_images.clone(),
                reveal: Some((correct, picks)),
            }
        },
        Reveal::Text { accepted, groups } => rsx! {
            TextReveal { question: question.index, accepted, groups }
        },
        Reveal::Pin {
            answer,
            aspect,
            pins,
        } => rsx! {
            PinReveal { image: question.image.clone().unwrap_or_default(), answer, aspect, pins }
        },
        Reveal::Parts { parts } => rsx! {
            div { class: "parts-reveal",
                for (i, part) in parts.into_iter().enumerate() {
                    section { key: "{i}", class: "part-reveal",
                        h3 {
                            span { class: "order-rank", "{i + 1}" }
                            "{part.label}"
                        }
                        TextReveal { question: question.index, accepted: part.accepted, groups: part.groups }
                    }
                }
            }
        },
        Reveal::Order { items, placed } => rsx! {
            div { class: "open-reveal",
                ol { class: "order-items",
                    for (i, item) in items.into_iter().enumerate() {
                        li { key: "{i}", class: "order-item revealed",
                            span { class: "order-rank", "{i + 1}" }
                            span { class: "strong", "{item}" }
                            span { class: "muted small",
                                {
                                    let n = placed.get(i).copied().unwrap_or(0);
                                    if n == 1 { "1 team got it".to_owned() } else { format!("{n} teams got it") }
                                }
                            }
                        }
                    }
                }
            }
        },
        Reveal::Number { answer, guesses } => rsx! {
            NumberReveal { question: question.index, answer, guesses }
        },
        Reveal::Pairs { pairs, matched } => rsx! {
            PairsReveal { columns: question.pairs.clone().unwrap_or_default(), pairs, matched }
        },
    }
}

/// The two columns of a pairs question: shapes on the left, letters on the right.
#[component]
pub(super) fn PairBoard(columns: PairColumns) -> Element {
    rsx! {
        div { class: "pair-board",
            ul { class: "pair-column",
                for (l, item) in columns.left.into_iter().enumerate() {
                    li { key: "{l}", class: "pair-card",
                        AnswerBadge { index: l }
                        span { class: "pair-body",
                            PairItemContent { item }
                        }
                    }
                }
            }
            ul { class: "pair-column",
                for (r, item) in columns.right.into_iter().enumerate() {
                    li { key: "{r}", class: "pair-card",
                        span { class: "order-rank", "{pair_letter(r)}" }
                        span { class: "pair-body",
                            PairItemContent { item }
                        }
                    }
                }
            }
        }
    }
}

/// The right pairs, each with the letter it had on the right and how many teams matched it.
#[component]
fn PairsReveal(columns: PairColumns, pairs: Vec<Pair>, matched: Vec<u32>) -> Element {
    rsx! {
        ul { class: "pairs-reveal",
            for (l, pair) in pairs.into_iter().enumerate() {
                li { key: "{l}", class: "pair-row",
                    span { class: "pair-card",
                        AnswerBadge { index: l }
                        span { class: "pair-body",
                            PairItemContent { item: pair.left }
                        }
                    }
                    span { class: "pair-link", "🔗" }
                    span { class: "pair-card",
                        if let Some(r) = columns.right.iter().position(|item| *item == pair.right) {
                            span { class: "order-rank", "{pair_letter(r)}" }
                        }
                        span { class: "pair-body",
                            PairItemContent { item: pair.right }
                        }
                    }
                    span { class: "muted small",
                        {
                            let n = matched.get(l).copied().unwrap_or(0);
                            if n == 1 { "1 team got it".to_owned() } else { format!("{n} teams got it") }
                        }
                    }
                }
            }
        }
    }
}

/// Every team's pin and the right spot, with the closest pins listed.
#[component]
fn PinReveal(image: String, answer: Point, aspect: f64, pins: Vec<PinGuess>) -> Element {
    let mut marks: Vec<MapPin> = pins
        .iter()
        .map(|pin| MapPin {
            at: pin.at,
            label: pin
                .team
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default(),
            title: pin.team.clone(),
            class: "",
        })
        .collect();
    marks.push(MapPin {
        at: answer,
        label: STAR.to_owned(),
        title: "The right spot".to_owned(),
        class: "answer",
    });
    rsx! {
        div { class: "pin-reveal",
            PinMap { image, aspect, pins: marks }
            if pins.is_empty() {
                p { class: "muted", "Nobody dropped a pin." }
            }
            ol { class: "guesses",
                for (i, pin) in pins.into_iter().take(PINS_LISTED).enumerate() {
                    // The trophy goes to the closest pin only.
                    li { key: "{i}", class: if i == 0 && pin.points > 0 { "guess closest" } else { "guess" },
                        span { class: "strong", "{pin.team}" }
                        span { class: "muted small", title: "Distance, as a share of the picture's width", "{pin.distance * 100.0:.0}% away" }
                        span { "+{pin.points}" }
                    }
                }
            }
        }
    }
}

/// The parts of a several-answers question while teams answer: numbered labels, with each
/// part's picture when it has one.
#[component]
pub(super) fn PartCards(labels: Vec<String>, images: Vec<Option<String>>) -> Element {
    rsx! {
        ol { class: "part-cards",
            for (i, label) in labels.into_iter().enumerate() {
                li { key: "{i}", class: "part-card",
                    if let Some(Some(image)) = images.get(i) {
                        img { src: media_url(image), alt: "" }
                    }
                    span {
                        span { class: "order-rank", "{i + 1}" }
                        "{label}"
                    }
                }
            }
        }
    }
}

/// Typed answers grouped by normalised text, with the judge's controls.
#[component]
fn TextReveal(question: usize, accepted: Vec<String>, groups: Vec<TextGroup>) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let judge = move |key: String, judgement: Judgement| {
        live.send(HostCommand::Judge {
            question,
            key,
            judgement,
        });
    };
    rsx! {
        div { class: "open-reveal",
            p { class: "accepted",
                span { class: "muted", "Answer: " }
                strong { {accepted.join(" · ")} }
            }
            if groups.is_empty() {
                p { class: "muted", "Nobody answered." }
            }
            ul { class: "text-groups",
                for group in groups {
                    {
                        let j = group.judgement;
                        let class = match j.grade {
                            Grade::Full => "text-group full",
                            Grade::Half => "text-group half",
                            Grade::Wrong => "text-group wrong",
                        };
                        let bonus = bonus_label(j.bonus);
                        let grade_button = |grade: Grade, label: &'static str, title: &'static str| {
                            let key = group.key.clone();
                            rsx! {
                                button {
                                    class: if j.grade == grade { "judge-button selected" } else { "judge-button" },
                                    title,
                                    onclick: move |_| judge(key.clone(), Judgement { grade, ..j }),
                                    "{label}"
                                }
                            }
                        };
                        let bonus_button = |delta: i32, label: &'static str, title: &'static str| {
                            let key = group.key.clone();
                            let bonus = j.bonus + delta;
                            rsx! {
                                button {
                                    class: "judge-button",
                                    title,
                                    disabled: bonus.abs() > MAX_BONUS,
                                    onclick: move |_| judge(key.clone(), Judgement { bonus, ..j }),
                                    "{label}"
                                }
                            }
                        };
                        rsx! {
                            li { key: "{group.key}", class,
                                div { class: "group-text",
                                    span { class: "strong", "{group.text}" }
                                    span { class: "muted small", {group.teams.join(", ")} }
                                }
                                div { class: "judge",
                                    {grade_button(Grade::Full, "✓", "Full points")}
                                    {grade_button(Grade::Half, "½", "Half points")}
                                    {grade_button(Grade::Wrong, "✗", "No points")}
                                    {bonus_button(-BONUS_STEP, "−", "Malus")}
                                    span { class: "bonus", "{bonus}" }
                                    {bonus_button(BONUS_STEP, "+", "Bonus")}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A bonus as the judge's controls show it: signed, and blank when there is none.
fn bonus_label(bonus: i32) -> String {
    match bonus {
        0 => String::new(),
        b if b > 0 => format!("+{b}"),
        b => b.to_string(),
    }
}

/// Points added or removed per click on the bonus buttons.
const BONUS_STEP: i32 = 100;

/// The right number and the guesses, closest first, each with the judge's bonus and malus.
#[component]
fn NumberReveal(question: usize, answer: f64, guesses: Vec<Guess>) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let judge = move |key: String, bonus: i32| {
        let judgement = Judgement {
            grade: Grade::Full,
            bonus,
        };
        live.send(HostCommand::Judge {
            question,
            key,
            judgement,
        });
    };
    rsx! {
        div { class: "open-reveal",
            p { class: "number-answer", {format_number(answer)} }
            if guesses.is_empty() {
                p { class: "muted", "Nobody answered." }
            }
            ol { class: "guesses",
                for guess in guesses {
                    {
                        let bonus_button = |delta: i32, label: &'static str, title: &'static str| {
                            let key = guess.key.clone();
                            let bonus = guess.bonus + delta;
                            rsx! {
                                button {
                                    class: "judge-button",
                                    title,
                                    disabled: bonus.abs() > MAX_BONUS,
                                    onclick: move |_| judge(key.clone(), bonus),
                                    "{label}"
                                }
                            }
                        };
                        let bonus = bonus_label(guess.bonus);
                        rsx! {
                            li { key: "{guess.key}", class: if guess.full { "guess closest" } else { "guess" },
                                span { class: "strong", "{guess.team}" }
                                span { {format_number(guess.value)} }
                                span { class: "muted small", "off by {format_number((guess.value - answer).abs())}" }
                                span { if guess.points > 0 { "+{guess.points}" } else { "{guess.points}" } }
                                div { class: "judge",
                                    {bonus_button(-BONUS_STEP, "−", "Malus")}
                                    span { class: "bonus", "{bonus}" }
                                    {bonus_button(BONUS_STEP, "+", "Bonus")}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A number without float noise: at most three decimals, no trailing zeros.
fn format_number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

#[component]
pub(super) fn ClosedSlide(
    question: QuestionInfo,
    answered: usize,
    teams: usize,
    reveal: Option<Reveal>,
    fastest: Option<Fastest>,
    standings: Option<Vec<StandingRow>>,
    bonus: Option<PartReveal>,
    explanation: Option<Explanation>,
) -> Element {
    match (reveal, standings) {
        (Some(reveal), Some(standings)) => rsx! {
            QuestionHeader { question: question.clone(),
                span { class: "muted", "{answered}/{teams} answered" }
            }
            if answered == 0 {
                p { class: "waiting-note", "Nobody answered this one." }
            }
            div { class: "closed-layout",
                div { class: "stack",
                    FullPicture { question: question.clone() }
                    RevealBody { question: question.clone(), reveal }
                    BonusReveal { question: question.index, bonus }
                    FastestNote { fastest }
                    ExplanationNote { explanation }
                }
                Standings { rows: standings.into_iter().take(STANDINGS_SHOWN).collect::<Vec<_>>() }
            }
        },
        _ => rsx! {
            QuestionHeader { question: question.clone() }
            FullPicture { question: question.clone() }
            p { class: "waiting-note",
                "Time's up! {answered} of {teams} teams answered."
                br {}
                "Answers are revealed at the end."
            }
        },
    }
}

#[component]
pub(super) fn ReviewSlide(
    question: QuestionInfo,
    answered: usize,
    teams: usize,
    reveal: Reveal,
    fastest: Option<Fastest>,
    bonus: Option<PartReveal>,
    explanation: Option<Explanation>,
) -> Element {
    rsx! {
        QuestionHeader { question: question.clone(),
            span { class: "muted", "The answers · {answered}/{teams} answered" }
        }
        if answered == 0 {
            p { class: "waiting-note", "Nobody answered this one." }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                QuestionMedia { name: image.clone(), class: "question-image" }
            }
        }
        RevealBody { question: question.clone(), reveal }
        BonusReveal { question: question.index, bonus }
        FastestNote { fastest }
        ExplanationNote { explanation }
    }
}

/// What the question shows with its answer: an explanation, a fun fact, a picture…
#[component]
fn ExplanationNote(explanation: Option<Explanation>) -> Element {
    // A soundtrack alone plays from the toolbar.
    let Some(explanation) = explanation.filter(|e| !e.text.is_empty() || e.image.is_some()) else {
        return rsx! {};
    };
    rsx! {
        aside { class: "explanation",
            if let Some(image) = &explanation.image {
                QuestionMedia { name: image.clone(), class: "explanation-image", autoplay: true }
            }
            if !explanation.text.is_empty() {
                p { class: "explanation-text", "{explanation.text}" }
            }
        }
    }
}

/// The bonus question's right answer and the teams' typed answers, to judge like typed answers.
#[component]
fn BonusReveal(question: usize, bonus: Option<PartReveal>) -> Element {
    let Some(bonus) = bonus else {
        return rsx! {};
    };
    rsx! {
        section { class: "part-reveal bonus-reveal",
            h3 { "{bonus.label}" }
            TextReveal { question, accepted: bonus.accepted, groups: bonus.groups }
        }
    }
}

/// "⚡ Fastest: Owls in 2.3 s".
#[component]
fn FastestNote(fastest: Option<Fastest>) -> Element {
    let precise = use_precise_times();
    let Some(fastest) = fastest else {
        return rsx! {};
    };
    rsx! {
        p { class: "fastest",
            "⚡ Fastest: "
            strong { "{fastest.team}" }
            " in {seconds(fastest.answer_ms, precise)}"
        }
    }
}
