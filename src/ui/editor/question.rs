//! The card that edits one question or slide.

use dioxus::prelude::*;

use super::answers::{AcceptedAnswers, AnswerRow, Choice};
use super::bonus::BonusEditor;
use super::explanation::ExplanationEditor;
use super::library_sync::LibrarySyncButton;
use super::navigator::{anchor, follow_move};
use super::numbers::{NumberAnswer, NumberScoringChoice};
use super::order::OrderItems;
use super::pairs::PairsEditor;
use super::parts::{PartsEditor, blank_part};
use super::pins::PinEditor;
use super::uploads::{Limits, MEDIA_ACCEPT, media_limits_title, media_too_large, too_large};
use super::{DEFAULT_HOLD_SECS, Draft, TestFrom};
use crate::ui::{
    admin::AdminToken,
    widgets::{ErrorBox, Icon, IconKind, media_url, message},
};
use crate::{
    api::{upload_question_audio, upload_question_media},
    library::LibraryCategory,
    model::{
        Answer, ImageReveal, MAX_ANSWERS, MAX_MULTIPLIER, MAX_TIME_LIMIT_SECS, MIN_ANSWERS,
        MIN_TIME_LIMIT_SECS, NumberScoring, Pair, PinTolerance, Question, QuestionKind, is_video,
    },
};

#[component]
pub(super) fn QuestionCard(
    index: usize,
    label: String,
    count: usize,
    question: Question,
    default_limit: u32,
    /// Edited on its own (a library question): no moving, testing, copying or deleting.
    #[props(default)]
    standalone: bool,
) -> Element {
    let draft: Draft = use_context();
    let test_from: TestFrom = use_context();
    let token: AdminToken = use_context();
    let mut upload_error = use_signal(|| None::<String>);
    let mut uploading = use_signal(|| false);
    let mut uploading_audio = use_signal(|| false);
    let answer_count = question.answers.len();
    let multi = question.is_multi();
    let slide = question.is_slide();
    let results_slide = question.is_results();
    let limit = question
        .time_limit_secs
        .map(|s| s.to_string())
        .unwrap_or_default();

    let limits: Limits = use_context();
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        if let Some(error) = media_too_large(&file, limits) {
            upload_error.set(Some(error));
            return;
        }
        uploading.set(true);
        upload_error.set(None);
        match upload_question_media(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(index, |q| q.set_image(Some(name))),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };

    let upload_audio = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        if let Some(error) = too_large(&file, limits.get().map(|l| l.audio), "soundtrack") {
            upload_error.set(Some(error));
            return;
        }
        uploading_audio.set(true);
        upload_error.set(None);
        match upload_question_audio(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(index, |q| q.audio = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading_audio.set(false);
    };

    rsx! {
        section {
            id: (!standalone).then(|| anchor(&label)),
            class: if slide { "card stack question slide-card" } else { "card stack question" },
            div { class: "row between",
                h3 { "{label}" }
                if !standalone {
                div { class: "row actions",
                    button {
                        class: "button ghost icon",
                        title: "Move to the top",
                        disabled: index == 0,
                        onclick: move |_| {
                            follow_move(index, 0, "Move to the top");
                            draft.edit(|q| {
                                let item = q.questions.remove(index);
                                q.questions.insert(0, item);
                            });
                        },
                        "⤒"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move up",
                        disabled: index == 0,
                        onclick: move |_| {
                            follow_move(index, index - 1, "Move up");
                            draft.edit(|q| q.questions.swap(index, index - 1));
                        },
                        "↑"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move down",
                        disabled: index + 1 == count,
                        onclick: move |_| {
                            follow_move(index, index + 1, "Move down");
                            draft.edit(|q| q.questions.swap(index, index + 1));
                        },
                        "↓"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move to the bottom",
                        disabled: index + 1 == count,
                        onclick: move |_| {
                            follow_move(index, count - 1, "Move to the bottom");
                            draft.edit(|q| {
                                let item = q.questions.remove(index);
                                q.questions.push(item);
                            });
                        },
                        "⤓"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Test from here",
                        aria_label: "Test from here",
                        onclick: move |_| test_from.0.call(index),
                        "🧪"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Duplicate",
                        aria_label: "Duplicate",
                        onclick: move |_| draft.edit(|q| {
                            let copy = q.questions[index].clone();
                            q.questions.insert(index + 1, copy);
                        }),
                        Icon { kind: IconKind::Copy }
                    }
                    if !slide {
                        LibrarySyncButton { index, question: question.clone() }
                    }
                    button {
                        class: "button ghost icon danger-icon",
                        title: "Delete",
                        aria_label: "Delete",
                        onclick: move |_| draft.edit(|q| {
                            q.questions.remove(index);
                        }),
                        Icon { kind: IconKind::Trash }
                    }
                }
                }
            }
            if let QuestionKind::Slide { title, results } = &question.kind {
                input {
                    class: "slide-title-input",
                    placeholder: if *results { "Title above the standings, e.g. Half time!" } else { "Slide title, e.g. Round 2: Music" },
                    value: "{title}",
                    oninput: move |e| {
                        let title = e.value();
                        draft.edit_question(index, |q| {
                            if let QuestionKind::Slide { title: old, .. } = &mut q.kind {
                                *old = title;
                            }
                        });
                    },
                }
                if *results {
                    p { class: "muted",
                        "📊 Reveals the answers to the questions since the previous results slide, then shows the standings."
                    }
                }
            }
            if !slide {
                div { class: "question-settings row",
                    if !standalone {
                        SubjectEditor {
                            question: index,
                            category: question.category.clone(),
                            topic: question.topic.clone(),
                        }
                    }
                    label { class: "field inline",
                        span { "Time (s)" }
                        input {
                            r#type: "number",
                            class: "short",
                            min: MIN_TIME_LIMIT_SECS,
                            max: MAX_TIME_LIMIT_SECS,
                            placeholder: "{default_limit}",
                            value: "{limit}",
                            oninput: move |e| {
                                let value = e.value();
                                draft.edit_question(index, |q| q.time_limit_secs = value.trim().parse().ok());
                            },
                        }
                    }
                    label { class: "field inline",
                        title: "How many times a usual question's points this one is worth",
                        span { "Points" }
                        select {
                            value: "{question.multiplier()}",
                            onchange: move |e| {
                                let multiplier = e.value().parse().ok().filter(|&m| m > 1);
                                draft.edit_question(index, |q| q.multiplier = multiplier);
                            },
                            for m in 1..=MAX_MULTIPLIER {
                                option { value: "{m}", "×{m}" }
                            }
                        }
                    }
                }
            }
            if !results_slide {
                textarea {
                    class: "question-text",
                    placeholder: if slide { "Text (optional)…" } else { "Type the question…" },
                    rows: 2,
                    value: "{question.text}",
                    oninput: move |e| draft.edit_question(index, |q| q.text = e.value()),
                }
                if !slide {
                    textarea {
                        class: "question-passage-input",
                        placeholder: "Text shown with the question (optional): lyrics, a quote, a riddle…",
                        rows: if question.passage.is_empty() { 1 } else { 4 },
                        value: "{question.passage}",
                        oninput: move |e| draft.edit_question(index, |q| q.passage = e.value()),
                    }
                }
                div { class: "question-media stack",
                    if let Some(image) = &question.image {
                        fieldset { class: "media-box",
                            legend { if is_video(image) { "Video" } else { "Picture" } }
                            div { class: "row",
                                if is_video(image) {
                                    video {
                                        class: "thumb",
                                        src: media_url(image),
                                        preload: "metadata",
                                        muted: true,
                                        controls: true,
                                    }
                                } else {
                                    img { class: "thumb", src: media_url(image), alt: "Question picture" }
                                }
                                button {
                                    class: "button ghost",
                                    onclick: move |_| draft.edit_question(index, |q| q.set_image(None)),
                                    if is_video(image) { "Remove video" } else { "Remove picture" }
                                }
                                if !slide {
                                    label { class: "field inline",
                                        title: "How the picture appears while the clock runs",
                                        span { "Reveal" }
                                        select {
                                            value: match question.image_reveal {
                                                ImageReveal::Shown => "shown",
                                                ImageReveal::Zoom => "zoom",
                                                ImageReveal::Tiles => "tiles",
                                            },
                                            onchange: move |e| {
                                                let reveal = match e.value().as_str() {
                                                    "zoom" => ImageReveal::Zoom,
                                                    "tiles" => ImageReveal::Tiles,
                                                    _ => ImageReveal::Shown,
                                                };
                                                draft.edit_question(index, |q| {
                                                    // Suggest a few seconds to look at the whole picture.
                                                    if q.image_reveal == ImageReveal::Shown && q.image_hold_secs == 0 {
                                                        q.image_hold_secs = DEFAULT_HOLD_SECS;
                                                    }
                                                    q.image_reveal = reveal;
                                                });
                                            },
                                            option { value: "shown", "Show it all" }
                                            option { value: "zoom", "Zoom out from a close-up" }
                                            option { value: "tiles", "Uncover tile by tile" }
                                        }
                                    }
                                    if question.image_reveal != ImageReveal::Shown {
                                        label { class: "field inline",
                                            title: "The reveal takes the rest of the question's time",
                                            span { "then fully visible for" }
                                            input {
                                                r#type: "number",
                                                class: "short",
                                                min: 0,
                                                max: MAX_TIME_LIMIT_SECS,
                                                value: "{question.image_hold_secs}",
                                                oninput: move |e| {
                                                    if let Ok(secs) = e.value().trim().parse() {
                                                        draft.edit_question(index, |q| q.image_hold_secs = secs);
                                                    }
                                                },
                                            }
                                            span { "s" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let (false, Some(audio)) = (slide, &question.audio) {
                        fieldset { class: "media-box",
                            legend { "Soundtrack" }
                            div { class: "row",
                                Icon { kind: IconKind::Music }
                                audio {
                                    class: "audio-preview",
                                    src: media_url(audio),
                                    preload: "metadata",
                                    controls: true,
                                }
                                button {
                                    class: "button ghost",
                                    onclick: move |_| draft.edit_question(index, |q| q.audio = None),
                                    "Remove soundtrack"
                                }
                                span { class: "muted small",
                                    "The clock starts when you start the music on the big screen."
                                }
                            }
                        }
                    }
                    if question.image.is_none() || (!slide && question.audio.is_none()) {
                        div { class: "row",
                            if question.image.is_none() {
                                label {
                                    class: "button ghost",
                                    title: media_limits_title(limits),
                                    if uploading() { "Uploading…" } else { "＋ Add picture or video" }
                                    input {
                                        r#type: "file",
                                        accept: MEDIA_ACCEPT,
                                        class: "visually-hidden",
                                        onchange: upload,
                                    }
                                }
                            }
                            if !slide && question.audio.is_none() {
                                label { class: "button ghost",
                                    if uploading_audio() { "Uploading…" } else { "＋ Add soundtrack" }
                                    input {
                                        r#type: "file",
                                        accept: "audio/*",
                                        class: "visually-hidden",
                                        onchange: upload_audio,
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ErrorBox { error: upload_error() }
            if !slide {
                Choice {
                    label: "Answer type",
                    options: vec![
                        ("Choices", "Teams pick from coloured answers."),
                        ("True/false", "Teams say whether the statement is true or false."),
                        ("Typed answer", "Teams type a short answer. You can amend the verdict on each answer when they are revealed."),
                        ("Number", "Teams guess a number: the closest guess wins, or the closer the more points, or close enough is right."),
                        ("Order", "Teams put the items in order; each item in its right place earns its share of the points."),
                        ("Several answers", "Teams type a few answers at once (title and artist, five posters…); each is judged on its own and the points are shared between them."),
                        ("Pin", "Teams drop a pin on the picture (a map, a floor plan…); the closer, the more points."),
                        ("Pairs", "Teams match each item on the left with its partner on the right; each pair matched earns its share of the points."),
                    ],
                    selected: match question.kind {
                        QuestionKind::Choice | QuestionKind::Slide { .. } => 0,
                        QuestionKind::TrueFalse { .. } => 1,
                        QuestionKind::Text { .. } => 2,
                        QuestionKind::Number { .. } => 3,
                        QuestionKind::Order { .. } => 4,
                        QuestionKind::Parts { .. } => 5,
                        QuestionKind::Pin { .. } => 6,
                        QuestionKind::Pairs { .. } => 7,
                    },
                    on_select: move |i| draft.edit_question(index, |q| set_kind(q, i)),
                }
                match &question.kind {
                    QuestionKind::Choice => rsx! {
                        ul { class: "answers-edit",
                            for (i, answer) in question.answers.iter().enumerate() {
                                AnswerRow {
                                    key: "{i}",
                                    question: index,
                                    index: i,
                                    answer: answer.clone(),
                                    removable: answer_count > MIN_ANSWERS,
                                }
                            }
                        }
                        div { class: "row between",
                            if answer_count < MAX_ANSWERS {
                                button {
                                    class: "button ghost",
                                    onclick: move |_| draft.edit_question(index, |q| q.answers.push(Answer::default())),
                                    "＋ Add answer"
                                }
                            } else {
                                span {}
                            }
                            span { class: "muted small",
                                if multi {
                                    "Several correct answers: teams may pick more than one."
                                } else {
                                    "Tick the correct answer. Tick more than one to allow several."
                                }
                            }
                        }
                    },
                    QuestionKind::Text { accepted } => rsx! {
                        AcceptedAnswers { question: index, accepted: accepted.clone() }
                    },
                    QuestionKind::Number { answer, range, scoring } => rsx! {
                        NumberAnswer { question: index, answer: *answer, range: *range }
                        NumberScoringChoice { question: index, answer: *answer, range: *range, scoring: *scoring }
                    },
                    QuestionKind::Order { items } => rsx! {
                        OrderItems { question: index, items: items.clone() }
                    },
                    QuestionKind::Parts { parts } => rsx! {
                        PartsEditor { question: index, parts: parts.clone() }
                    },
                    QuestionKind::Pairs { pairs } => rsx! {
                        PairsEditor { question: index, pairs: pairs.clone() }
                    },
                    QuestionKind::Pin { answer, aspect, tolerance } => rsx! {
                        PinEditor { question: index, image: question.image.clone(), answer: *answer, aspect: *aspect, tolerance: *tolerance }
                    },
                    QuestionKind::TrueFalse { answer } => rsx! {
                        Choice {
                            label: "The statement is",
                            options: vec![
                                ("True", "Teams answering True are right."),
                                ("False", "Teams answering False are right."),
                            ],
                            selected: usize::from(!answer),
                            on_select: move |i| draft.edit_question(index, |q| q.kind = QuestionKind::TrueFalse { answer: i == 0 }),
                        }
                    },
                    QuestionKind::Slide { .. } => rsx! {},
                }
                if question.takes_bonus() {
                    BonusEditor { question: index, bonus: question.bonus.clone() }
                }
                ExplanationEditor {
                    question: index,
                    text: question.explanation.clone(),
                    image: question.explanation_image.clone(),
                    audio: question.explanation_audio.clone(),
                }
            }
        }
    }
}

/// The library's categories with their topics, those added on this server included: the ones
/// questions can be filed under. `None` until loaded.
#[derive(Clone, Copy)]
pub(super) struct Subjects(pub(super) Resource<Option<Vec<LibraryCategory>>>);

/// The category and topic the big screen shows with a question, picked among the library's.
/// A name the library doesn't have (from an imported quiz, say) stays until another is picked.
#[component]
fn SubjectEditor(question: usize, category: String, topic: String) -> Element {
    let draft: Draft = use_context();
    let subjects: Subjects = use_context();
    let loaded = subjects.0.read().clone().flatten();
    let categories = loaded.clone().unwrap_or_default();
    let topics: Vec<String> = categories
        .iter()
        .find(|c| c.name == category)
        .map(|c| c.topics.iter().map(|t| t.name.clone()).collect())
        .unwrap_or_default();
    // Hidden copies of every name widen each menu to its longest, whichever category is picked.
    let category_names: Vec<String> = categories.iter().map(|c| c.name.clone()).collect();
    let topic_names: Vec<String> = categories
        .iter()
        .flat_map(|c| c.topics.iter().map(|t| t.name.clone()))
        .collect();
    // Until the library loads, the names are shown as they are.
    let unknown_category = !category.is_empty() && !categories.iter().any(|c| c.name == category);
    let unknown_topic = !topic.is_empty() && !topics.contains(&topic);
    let suffix = if loaded.is_some() {
        " (not in the library)"
    } else {
        ""
    };
    // Options say which is selected: the select's value would be set before its options exist.
    rsx! {
        label { class: "field inline",
            title: "Shown on the big screen with the question. Add categories and topics on the admin page's Questions tab.",
            span { "Category" }
            select {
                class: "subject-select",
                // A topic belongs to its category: picking another clears it.
                onchange: move |e| draft.edit_question(question, |q| {
                    q.category = e.value();
                    q.topic.clear();
                }),
                option { value: "", selected: category.is_empty(), "None" }
                if unknown_category {
                    option { value: "{category}", selected: true, "{category}{suffix}" }
                }
                for c in categories {
                    option { value: "{c.name}", selected: c.name == category, "{c.name}" }
                }
            }
            for name in category_names {
                div { class: "select-sizer", aria_hidden: "true", "{name}" }
            }
        }
        label { class: "field inline",
            title: "Shown on the big screen with the question. Add categories and topics on the admin page's Questions tab.",
            span { "Topic" }
            select {
                class: "subject-select",
                disabled: category.is_empty(),
                onchange: move |e| draft.edit_question(question, |q| q.topic = e.value()),
                option { value: "", selected: topic.is_empty(), "None" }
                if unknown_topic {
                    option { value: "{topic}", selected: true, "{topic}{suffix}" }
                }
                for name in topics {
                    option { value: "{name}", selected: name == topic, "{name}" }
                }
            }
            for name in topic_names {
                div { class: "select-sizer", aria_hidden: "true", "{name}" }
            }
        }
    }
}

/// Switches a question to the answer type picked in the editor (0 choices, 1 true/false, 2 text,
/// 3 number, 4 order, 5 several answers, 6 pin, 7 pairs).
fn set_kind(question: &mut Question, choice: usize) {
    question.kind = match (choice, &question.kind) {
        (1, QuestionKind::TrueFalse { .. })
        | (2, QuestionKind::Text { .. })
        | (3, QuestionKind::Number { .. })
        | (4, QuestionKind::Order { .. })
        | (5, QuestionKind::Parts { .. })
        | (6, QuestionKind::Pin { .. })
        | (7, QuestionKind::Pairs { .. }) => return,
        (7, _) => QuestionKind::Pairs {
            pairs: vec![Pair::default(); 3],
        },
        (6, _) => QuestionKind::Pin {
            answer: None,
            aspect: 0.0,
            tolerance: PinTolerance::default(),
        },
        (5, _) => QuestionKind::Parts {
            parts: vec![blank_part(), blank_part()],
        },
        (1, _) => QuestionKind::TrueFalse { answer: true },
        // Start from the choices' texts, if any were typed.
        (4, _) => {
            let mut items: Vec<String> = question
                .answers
                .iter()
                .map(|a| a.text.trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect();
            if items.len() < MIN_ANSWERS {
                items.resize(MIN_ANSWERS.max(3), String::new());
            }
            QuestionKind::Order { items }
        }
        (2, _) => QuestionKind::Text {
            accepted: vec![String::new()],
        },
        (3, _) => QuestionKind::Number {
            answer: None,
            range: None,
            scoring: NumberScoring::Closest,
        },
        _ => {
            if question.answers.len() < MIN_ANSWERS {
                question.answers.resize_with(MIN_ANSWERS, Answer::default);
            }
            QuestionKind::Choice
        }
    };
}
