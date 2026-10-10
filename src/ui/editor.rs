//! The quiz editor.

use dioxus::prelude::*;

use super::{
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    widgets::{AnswerBadge, ClientOnly, ErrorBox, Spinner, media_url, message},
};
use crate::{
    api::{get_quiz, save_quiz, upload_question_image},
    model::{
        AdvanceMode, Answer, MAX_ANSWERS, MAX_TIME_LIMIT_SECS, MIN_ANSWERS, MIN_TIME_LIMIT_SECS,
        MultiScoring, PhoneDisplay, Question, Quiz, QuizSettings, RevealMode, ValidationIssue,
    },
};

/// How long the editor waits after the last change before saving.
const AUTOSAVE_DELAY_MS: u32 = 800;

#[component]
pub fn Editor(id: String) -> Element {
    rsx! {
        ClientOnly {
            AdminGate {
                EditorLoader { id }
            }
        }
    }
}

#[component]
fn EditorLoader(id: String) -> Element {
    let token: AdminToken = use_context();
    let quiz = use_resource(use_reactive!(|id| async move {
        get_quiz(token.get(), id).await
    }));
    let loaded = quiz.read().clone();
    match loaded {
        None => rsx! {
            AdminHeader { back: true }
            div { class: "page center", Spinner {} }
        },
        Some(Err(err)) => rsx! {
            AdminHeader { back: true }
            main { class: "page narrow",
                ErrorBox { error: Some(message(&err)) }
            }
        },
        Some(Ok(quiz)) => rsx! {
            EditorForm { key: "{quiz.id}", quiz }
        },
    }
}

/// Shared editor state: the draft and a revision counter bumped on every edit.
#[derive(Clone, Copy)]
struct Draft {
    quiz: Signal<Quiz>,
    revision: Signal<u64>,
}

impl Draft {
    fn edit(mut self, f: impl FnOnce(&mut Quiz)) {
        f(&mut self.quiz.write());
        self.revision += 1;
    }

    fn edit_question(self, index: usize, f: impl FnOnce(&mut Question)) {
        self.edit(|quiz| {
            if let Some(question) = quiz.questions.get_mut(index) {
                f(question);
            }
        });
    }

    fn edit_settings(self, f: impl FnOnce(&mut QuizSettings)) {
        self.edit(|quiz| f(&mut quiz.settings));
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SaveState {
    Saved,
    Pending,
    Failed,
}

#[component]
fn EditorForm(quiz: Quiz) -> Element {
    let token: AdminToken = use_context();
    let issues = use_signal(|| quiz.validate());
    let draft = Draft {
        quiz: use_signal(|| quiz),
        revision: use_signal(|| 0),
    };
    use_context_provider(|| draft);
    let save_state = use_autosave(token, draft, issues);

    let quiz = draft.quiz.read();
    let question_count = quiz.questions.len();
    rsx! {
        AdminHeader { back: true,
            span { class: "save-state",
                match save_state() {
                    SaveState::Saved => "All changes saved",
                    SaveState::Pending => "Saving…",
                    SaveState::Failed => "Not saved, retrying…",
                }
            }
        }
        main { class: "page wide stack editor",
            input {
                class: "title-input",
                placeholder: "Quiz title",
                value: "{quiz.title}",
                oninput: move |e| draft.edit(|q| q.title = e.value()),
            }
            Issues { issues: issues() }
            SettingsCard { settings: quiz.settings }
            for index in 0..question_count {
                QuestionCard {
                    key: "{index}",
                    index,
                    count: question_count,
                    question: quiz.questions[index].clone(),
                    default_limit: quiz.settings.default_time_limit_secs,
                }
            }
            button {
                class: "button primary add",
                onclick: move |_| draft.edit(|q| q.questions.push(Question::blank())),
                "＋ Add question"
            }
        }
    }
}

/// Saves the draft a moment after each change; returns the save status.
fn use_autosave(
    token: AdminToken,
    draft: Draft,
    mut issues: Signal<Vec<ValidationIssue>>,
) -> Signal<SaveState> {
    let mut state = use_signal(|| SaveState::Saved);
    let mut saved_revision = use_signal(|| 0u64);
    use_future(move || async move {
        loop {
            client::sleep_ms(AUTOSAVE_DELAY_MS).await;
            let revision = *draft.revision.peek();
            if revision == *saved_revision.peek() {
                continue;
            }
            state.set(SaveState::Pending);
            let quiz = draft.quiz.peek().clone();
            match save_quiz(token.get(), quiz).await {
                Ok(found) => {
                    issues.set(found);
                    saved_revision.set(revision);
                    if *draft.revision.peek() == revision {
                        state.set(SaveState::Saved);
                    }
                }
                Err(_) => state.set(SaveState::Failed),
            }
        }
    });
    use_effect(move || {
        if (draft.revision)() != *saved_revision.peek() {
            state.set(SaveState::Pending);
        }
    });
    state
}

#[component]
fn Issues(issues: Vec<ValidationIssue>) -> Element {
    if issues.is_empty() {
        return rsx! {
            p { class: "ready", "✓ Ready to play" }
        };
    }
    rsx! {
        div { class: "issues",
            strong { "Not ready to play yet:" }
            ul {
                for issue in issues {
                    li { "{issue}" }
                }
            }
        }
    }
}

#[component]
fn SettingsCard(settings: QuizSettings) -> Element {
    let draft: Draft = use_context();
    rsx! {
        section { class: "card stack",
            h2 { "Settings" }
            label { class: "field inline",
                span { "Default time per question (seconds)" }
                input {
                    r#type: "number",
                    class: "short",
                    min: MIN_TIME_LIMIT_SECS,
                    max: MAX_TIME_LIMIT_SECS,
                    value: settings.default_time_limit_secs,
                    oninput: move |e| {
                        if let Ok(secs) = e.value().parse() {
                            draft.edit_settings(|s| s.default_time_limit_secs = secs);
                        }
                    },
                }
            }
            div { class: "settings-grid",
                Choice {
                    label: "Reveal answers",
                    options: vec![
                        ("After each question", "Show the correct answer and standings after every question."),
                        ("At the end", "Keep everyone guessing; walk through the answers before the podium."),
                    ],
                    selected: usize::from(settings.reveal == RevealMode::AtEnd),
                    on_select: move |i| draft.edit_settings(|s| s.reveal = if i == 0 { RevealMode::AfterEachQuestion } else { RevealMode::AtEnd }),
                }
                Choice {
                    label: "Questions with several correct answers",
                    options: vec![
                        ("All or nothing", "Points only for picking exactly the correct answers."),
                        ("Partial credit", "Credit for each correct pick, minus wrong picks."),
                    ],
                    selected: usize::from(settings.multi_scoring == MultiScoring::Partial),
                    on_select: move |i| draft.edit_settings(|s| s.multi_scoring = if i == 0 { MultiScoring::AllOrNothing } else { MultiScoring::Partial }),
                }
                Choice {
                    label: "Moving on",
                    options: vec![
                        ("Host clicks", "The host decides when to show the next question."),
                        ("Automatic", "Continue by itself after a short pause."),
                    ],
                    selected: usize::from(settings.advance == AdvanceMode::Auto),
                    on_select: move |i| draft.edit_settings(|s| s.advance = if i == 0 { AdvanceMode::Manual } else { AdvanceMode::Auto }),
                }
                Choice {
                    label: "On team phones",
                    options: vec![
                        ("Buttons only", "Read the question on the big screen; phones show coloured shapes."),
                        ("Full text", "Phones show the question and answers too."),
                    ],
                    selected: usize::from(settings.phone_display == PhoneDisplay::FullText),
                    on_select: move |i| draft.edit_settings(|s| s.phone_display = if i == 0 { PhoneDisplay::ButtonsOnly } else { PhoneDisplay::FullText }),
                }
            }
        }
    }
}

/// A segmented choice between a few options, each with an explanation.
#[component]
fn Choice(
    label: &'static str,
    options: Vec<(&'static str, &'static str)>,
    selected: usize,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        fieldset { class: "choice",
            legend { "{label}" }
            div { class: "segmented",
                for (i, (name, _)) in options.iter().enumerate() {
                    button {
                        r#type: "button",
                        class: if i == selected { "selected" },
                        aria_pressed: i == selected,
                        onclick: move |_| on_select.call(i),
                        "{name}"
                    }
                }
            }
            p { class: "muted small", "{options[selected.min(options.len() - 1)].1}" }
        }
    }
}

#[component]
fn QuestionCard(index: usize, count: usize, question: Question, default_limit: u32) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let mut upload_error = use_signal(|| None::<String>);
    let mut uploading = use_signal(|| false);
    let answer_count = question.answers.len();
    let multi = question.is_multi();
    let limit = question
        .time_limit_secs
        .map(|s| s.to_string())
        .unwrap_or_default();

    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        upload_error.set(None);
        match upload_question_image(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(index, |q| q.image = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };

    rsx! {
        section { class: "card stack question",
            div { class: "row between",
                h3 { "Question {index + 1}" }
                div { class: "row actions",
                    button {
                        class: "button ghost icon",
                        title: "Move up",
                        disabled: index == 0,
                        onclick: move |_| draft.edit(|q| q.questions.swap(index, index - 1)),
                        "↑"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move down",
                        disabled: index + 1 == count,
                        onclick: move |_| draft.edit(|q| q.questions.swap(index, index + 1)),
                        "↓"
                    }
                    button {
                        class: "button ghost",
                        onclick: move |_| draft.edit(|q| {
                            let copy = q.questions[index].clone();
                            q.questions.insert(index + 1, copy);
                        }),
                        "Duplicate"
                    }
                    button {
                        class: "button ghost",
                        onclick: move |_| draft.edit(|q| {
                            q.questions.remove(index);
                        }),
                        "Delete"
                    }
                }
            }
            textarea {
                class: "question-text",
                placeholder: "Type the question…",
                rows: 2,
                value: "{question.text}",
                oninput: move |e| draft.edit_question(index, |q| q.text = e.value()),
            }
            div { class: "question-media row",
                if let Some(image) = &question.image {
                    img { class: "thumb", src: media_url(image), alt: "Question picture" }
                    button {
                        class: "button ghost",
                        onclick: move |_| draft.edit_question(index, |q| q.image = None),
                        "Remove picture"
                    }
                } else {
                    label { class: "button ghost",
                        if uploading() { "Uploading…" } else { "＋ Add picture" }
                        input {
                            r#type: "file",
                            accept: "image/*",
                            class: "visually-hidden",
                            onchange: upload,
                        }
                    }
                }
                label { class: "field inline",
                    span { "Seconds" }
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
            }
            ErrorBox { error: upload_error() }
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
        }
    }
}

#[component]
fn AnswerRow(question: usize, index: usize, answer: Answer, removable: bool) -> Element {
    let draft: Draft = use_context();
    rsx! {
        li { class: if answer.correct { "answer-edit correct" } else { "answer-edit" },
            AnswerBadge { index }
            input {
                class: "grow",
                placeholder: "Answer {index + 1}",
                value: "{answer.text}",
                oninput: move |e| draft.edit_question(question, |q| q.answers[index].text = e.value()),
            }
            label { class: "correct-toggle",
                input {
                    r#type: "checkbox",
                    checked: answer.correct,
                    onchange: move |e| draft.edit_question(question, |q| q.answers[index].correct = e.checked()),
                }
                span { "Correct" }
            }
            button {
                class: "button ghost icon",
                title: "Remove answer",
                disabled: !removable,
                onclick: move |_| draft.edit_question(question, |q| {
                    q.answers.remove(index);
                }),
                "✕"
            }
        }
    }
}
