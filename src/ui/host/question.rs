//! A question or slide on screen, with its timer and its answers.

use dioxus::prelude::*;

use super::reveal::{PairBoard, PartCards};
use super::tiles::QuestionImage;
use crate::protocol::{InputKind, QuestionInfo, SlideInfo};
use crate::ui::{
    library::{Categories, CategoryIcon},
    widgets::{ANSWER_STYLES, AnswerShape, Icon, IconKind, QuestionMedia, media_url},
};

/// A question's count, category and `children` (such as the clock), then its text.
#[component]
pub(super) fn QuestionHeader(question: QuestionInfo, children: Element) -> Element {
    rsx! {
        QuestionHead { question: question.clone(), {children} }
        QuestionText { question }
    }
}

/// The line above a question: its count and category, with `children` (such as the clock).
#[component]
fn QuestionHead(question: QuestionInfo, children: Element) -> Element {
    let categories: Categories = use_context();
    let look = categories.look(&question.category);
    rsx! {
        div { class: "question-head",
            span { class: "muted question-count",
                span { "Question {question.number} of {question.total}" }
                if !question.category.is_empty() {
                    span { "—" }
                    span {
                        class: "question-subject",
                        style: "--category: {look.color}",
                        CategoryIcon { look: look.clone() }
                        "{question.category}"
                        if !question.topic.is_empty() {
                            span { class: "question-topic", "· {question.topic}" }
                        }
                    }
                }
                if question.multiplier > 1 {
                    span { class: "multiplier", "×{question.multiplier} points" }
                }
            }
            {children}
        }
    }
}

/// A question's text, with its passage (unless shown elsewhere), bonus question and a note when
/// several answers are right.
#[component]
fn QuestionText(question: QuestionInfo, #[props(default = true)] passage: bool) -> Element {
    rsx! {
        h1 { class: "question-title", "{question.text}" }
        if passage && !question.passage.is_empty() {
            p { class: "question-passage", "{question.passage}" }
        }
        if let Some((text, percent)) = &question.bonus {
            p { class: "bonus-line",
                strong { "Bonus +{percent}%: " }
                "{text}"
            }
        }
        if question.multi {
            p { class: "muted", style: "text-align:center;margin:0", "Several answers are correct" }
        }
    }
}

#[component]
pub(super) fn QuestionSlide(
    question: QuestionInfo,
    left_ms: f64,
    elapsed_ms: f64,
    answered: usize,
    teams: usize,
    paused: bool,
) -> Element {
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let limit = question.time_limit_ms as f64;
    let fraction = if limit == 0.0 { 0.0 } else { left_ms / limit };
    // The picture's reveal follows the time spent, not the time left, so extra time from the
    // host never runs it backwards. It completes `image_hold_ms` before the original limit.
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let reveal_span = limit - question.image_hold_ms as f64;
    let reveal_left = if reveal_span <= 0.0 {
        0.0
    } else {
        1.0 - elapsed_ms / reveal_span
    };
    let seconds = (left_ms / 1000.0).ceil();
    rsx! {
        QuestionHead { question: question.clone(),
            Timer { fraction, seconds }
        }
        QuestionBody {
            question: question.clone(),
            picture: rsx! {
                if let Some(image) = &question.image {
                    QuestionImage { image: image.clone(), reveal: question.image_reveal, left: reveal_left, seed: question.index, paused }
                }
            },
            div { class: "answered",
                strong { "{answered}/{teams}" }
                span { class: "muted", "answered" }
            }
        }
    }
}

/// A slide between questions: title, picture and text, each optional.
/// With both a picture and text, the picture goes left of the text to use the screen's width.
#[component]
pub(super) fn SlideView(slide: SlideInfo) -> Element {
    let media = slide.image.as_ref().map(|image| {
        rsx! {
            QuestionMedia { name: image.clone(), class: "slide-image", autoplay: true }
        }
    });
    let text = (!slide.text.is_empty()).then(|| {
        rsx! {
            p { class: "slide-text", "{slide.text}" }
        }
    });
    rsx! {
        div { class: "slide",
            if !slide.title.is_empty() {
                h1 { class: "slide-title", "{slide.title}" }
            }
            if media.is_some() && text.is_some() {
                div { class: "slide-split", {media} {text} }
            } else {
                {media}
                {text}
            }
        }
    }
}

/// A soundtrack question before the music starts: everything is shown, the clock is full.
#[component]
pub(super) fn ReadySlide(question: QuestionInfo, teams: usize) -> Element {
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let seconds = (question.time_limit_ms as f64 / 1000.0).ceil();
    rsx! {
        QuestionHead { question: question.clone(),
            Timer { fraction: 1.0, seconds }
        }
        QuestionBody {
            question: question.clone(),
            picture: rsx! {
                if let Some(image) = &question.image {
                    QuestionImage { image: image.clone(), reveal: question.image_reveal, left: 1.0, seed: question.index }
                }
            },
            div { class: "answered",
                strong { Icon { kind: IconKind::Music } }
                span { class: "muted", "The clock starts with the music" }
            }
        }
    }
}

/// Lines longer than this, in characters, give a passage beside a picture the wider column.
const WIDE_PASSAGE: usize = 40;

/// A question's text, picture, `children` (such as how many teams answered) and answers. With a
/// picture and a passage, the picture goes left of the passage to use the screen's width, as on
/// slides, and `children` go below them. A passage with long lines takes the wider share.
#[component]
fn QuestionBody(question: QuestionInfo, picture: Element, children: Element) -> Element {
    let split = question.image.is_some() && !question.passage.is_empty();
    // The passage's longest line sizes its text to fit without wrapping, within limits.
    let longest = question
        .passage
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    rsx! {
        QuestionText { question: question.clone(), passage: !split }
        if split {
            div { class: if longest > WIDE_PASSAGE { "question-split wide-text" } else { "question-split" },
                {picture}
                div { class: "passage-box",
                    p { class: "question-passage", style: "--longest: {longest.max(1)}", "{question.passage}" }
                }
            }
            {children}
        } else {
            div { class: "question-body",
                {picture}
                {children}
            }
        }
        AnswersOrPrompt { question }
    }
}

#[component]
fn Timer(fraction: f64, seconds: f64) -> Element {
    const CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * 45.0;
    let offset = CIRCUMFERENCE * (1.0 - fraction.clamp(0.0, 1.0));
    rsx! {
        div { class: if seconds <= 5.0 { "timer urgent" } else { "timer" },
            svg { view_box: "0 0 100 100",
                circle { class: "track", cx: 50, cy: 50, r: 45 }
                circle {
                    class: "progress",
                    cx: 50,
                    cy: 50,
                    r: 45,
                    stroke_dasharray: "{CIRCUMFERENCE}",
                    stroke_dashoffset: "{offset}",
                }
            }
            span { "{seconds}" }
        }
    }
}

#[component]
pub(super) fn AnswerGrid(
    answers: Vec<String>,
    images: Vec<Option<String>>,
    reveal: Option<(Vec<usize>, Vec<u32>)>,
) -> Element {
    rsx! {
        div { class: "answer-grid",
            for (i, text) in answers.iter().enumerate() {
                {
                    let mut class = format!("answer-tile a{}", i % ANSWER_STYLES);
                    if let Some((correct, _)) = &reveal {
                        class.push_str(if correct.contains(&i) { " right" } else { " dim" });
                    }
                    let picks = reveal.as_ref().and_then(|(_, picks)| picks.get(i).copied());
                    let image = images.get(i).cloned().flatten();
                    if image.is_some() {
                        class.push_str(" has-image");
                    }
                    rsx! {
                        div { key: "{i}", class,
                            AnswerShape { index: i }
                            if let Some(image) = image {
                                img { class: "tile-image", src: media_url(&image), alt: "{text}" }
                            }
                            span { class: "text", "{text}" }
                            if let Some(picks) = picks {
                                span { class: "picks", "{picks} 👥" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The answer buttons of a multiple-choice question, or what to do for other kinds.
#[component]
fn AnswersOrPrompt(question: QuestionInfo) -> Element {
    let prompt = match question.input {
        InputKind::Choice => {
            return rsx! {
                AnswerGrid { answers: question.answers.clone(), images: question.answer_images.clone(), reveal: None }
            };
        }
        InputKind::Order => {
            return rsx! {
                div { class: "open-reveal",
                    p { class: "open-prompt", "🔀 Put them in order on your phone" }
                    ul { class: "order-items",
                        for (i, item) in question.answers.iter().enumerate() {
                            li { key: "{i}", class: "order-item", "{item}" }
                        }
                    }
                }
            };
        }
        InputKind::Parts => {
            return rsx! {
                div { class: "open-reveal",
                    p { class: "open-prompt", "✍️ Type the answers on your phone" }
                    PartCards { labels: question.answers.clone(), images: question.answer_images.clone() }
                }
            };
        }
        InputKind::Pairs => {
            return rsx! {
                div { class: "open-reveal",
                    p { class: "open-prompt", "🔗 Match the pairs on your phone" }
                    PairBoard { columns: question.pairs.clone().unwrap_or_default() }
                }
            };
        }
        InputKind::Pin => "📍 Drop a pin on your phone",
        InputKind::Text => "✍️ Type your answer on your phone",
        InputKind::Number if question.closest_wins => {
            "🔢 Enter a number on your phone: closest wins"
        }
        InputKind::Number => "🔢 Enter a number on your phone",
    };
    rsx! {
        p { class: "open-prompt", "{prompt}" }
    }
}
