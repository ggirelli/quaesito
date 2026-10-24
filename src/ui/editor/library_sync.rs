//! Questions linked to the question library: editing them there, and syncing a quiz's copy.

use dioxus::prelude::*;

use super::answers::Choice;
use super::question::QuestionCard;
use super::uploads::Limits;
use super::{Draft, Issues, TestFrom};
use crate::ui::{
    admin::AdminToken,
    widgets::{Dialog, ErrorBox, Icon, IconKind, Spinner, message},
};
use crate::{
    api::{
        add_library_question, delete_library_question, edit_library_question,
        library_question_uses, revert_library_question, upload_limits,
    },
    library::{
        LibraryEdit, LibraryEntry, LibrarySync, answer_line, differences, fingerprint, pulled,
    },
    model::{LibraryLink, Question, Quiz, QuizSettings},
};

/// Edits a library question in a pop-up, with the editor's question card. Saving keeps the change
/// on this server; an edited question can go back to how the library has it, and one added here
/// can be deleted.
#[component]
pub fn LibraryQuestionEditor(
    entry: LibraryEntry,
    on_saved: EventHandler<LibraryEntry>,
    on_close: EventHandler<()>,
) -> Element {
    let token: AdminToken = use_context();
    let draft = Draft {
        quiz: use_signal(|| {
            let mut quiz = Quiz::new(String::new(), "Library".to_owned());
            quiz.questions = vec![entry.question.clone()];
            quiz
        }),
        revision: use_signal(|| 0),
    };
    use_context_provider(|| draft);
    let limits = use_resource(move || async move { upload_limits(token.get()).await.ok() });
    use_context_provider(|| Limits(limits));
    use_context_provider(|| TestFrom(EventHandler::new(|_| {})));
    let mut difficulty = use_signal(|| entry.difficulty);
    let mut error = use_signal(|| None::<String>);

    let id = entry.id.clone();
    let save = move |_| {
        let id = id.clone();
        async move {
            let edit = LibraryEdit {
                difficulty: difficulty(),
                question: draft.quiz.peek().questions[0].clone(),
            };
            match edit_library_question(token.get(), id, edit).await {
                Ok(saved) => on_saved.call(saved),
                Err(err) => error.set(Some(message(&err))),
            }
        }
    };
    let id = entry.id.clone();
    let revert = move |_| {
        let id = id.clone();
        async move {
            match revert_library_question(token.get(), id).await {
                Ok(original) => on_saved.call(original),
                Err(err) => error.set(Some(message(&err))),
            }
        }
    };

    let mut deleting = use_signal(|| false);
    let deleted = entry.clone();
    let delete = move |_| {
        let entry = deleted.clone();
        async move {
            if !deleting() {
                deleting.set(true);
                return;
            }
            match delete_library_question(token.get(), entry.id.clone()).await {
                Ok(()) => on_saved.call(entry),
                Err(err) => error.set(Some(message(&err))),
            }
        }
    };

    let quiz = draft.quiz.read();
    let issues = quiz.validate();
    let defaults = QuizSettings::default();
    rsx! {
        Dialog { title: "Edit library question", class: "edit-library-dialog", on_close,
            Choice {
                label: "Difficulty",
                options: vec![
                    ("Easy", "Most adults know the answer."),
                    ("Medium", "Keen quizzers know it."),
                    ("Hard", "For experts."),
                ],
                selected: usize::from(difficulty().clamp(1, 3) - 1),
                on_select: move |i: usize| difficulty.set(u8::try_from(i + 1).unwrap_or(1)),
            }
            QuestionCard {
                index: 0,
                label: "{entry.category} · {entry.topic}",
                count: 1,
                question: quiz.questions[0].clone(),
                default_limit: defaults.default_time_limit_secs,
                standalone: true,
            }
            Issues { issues: issues.clone() }
            ErrorBox { error: error() }
            p { class: "muted small",
                if entry.added {
                    "Saved to the library on this server, from a quiz. Quizzes using it are offered the changes."
                } else {
                    "Changes are kept on this server only; the library that comes with Quaesito stays as it is."
                }
            }
            div { class: "row between",
                if entry.edited {
                    button { class: "button ghost", onclick: revert, "↶ Back to the original" }
                } else if entry.added {
                    button {
                        class: "button ghost danger",
                        title: "Quizzes keep their copy of the question",
                        onclick: delete,
                        if deleting() { "Delete for good?" } else { "Delete from the library" }
                    }
                } else {
                    span {}
                }
                button {
                    class: "button primary",
                    disabled: !issues.is_empty(),
                    onclick: save,
                    "Save"
                }
            }
        }
    }
}

/// The library as the editor compares linked questions with it, with this server's edits and
/// added questions. Setting `wanted` loads it even with no question linked.
#[derive(Clone, Copy)]
pub(super) struct Library {
    pub(super) entries: Resource<Option<Result<Vec<LibraryEntry>, ServerFnError>>>,
    pub(super) wanted: Signal<bool>,
}

impl Library {
    /// The entries, once loaded.
    fn loaded(self) -> Option<Vec<LibraryEntry>> {
        match &*self.entries.read() {
            Some(Some(Ok(entries))) => Some(entries.clone()),
            _ => None,
        }
    }
}

/// Where a question stands against the library, on a button that opens [`LibrarySyncDialog`].
#[component]
pub(super) fn LibrarySyncButton(index: usize, question: Question) -> Element {
    let library: Library = use_context();
    let mut open = use_signal(|| false);
    let entries = library.loaded();
    let state = entries
        .as_deref()
        .and_then(|entries| crate::library::sync(&question, entries))
        .map(|(state, _)| state);
    let (label, title, class) = match (&question.library, state) {
        (None, _) if question.category.is_empty() || question.topic.is_empty() => (
            "Save to library",
            "Pick a category and a topic to save the question to the library",
            "button ghost icon library-sync",
        ),
        (None, _) => (
            "Save to library",
            "Keeps it on this server, for other quizzes",
            "button ghost icon library-sync",
        ),
        (Some(_), None) => (
            "Library…",
            "Loading the library",
            "button ghost icon library-sync",
        ),
        (Some(_), Some(LibrarySync::InStep)) => (
            "In the library",
            "Same as the library's question",
            "button ghost icon library-sync in-step",
        ),
        (Some(_), Some(LibrarySync::ChangedHere)) => (
            "Update library",
            "Changed here since it came from the library",
            "button ghost icon library-sync changed",
        ),
        (Some(_), Some(LibrarySync::ChangedInLibrary)) => (
            "Update from library",
            "The library's question changed since",
            "button ghost icon library-sync changed",
        ),
        (Some(_), Some(LibrarySync::ChangedInBoth)) => (
            "Changed in both",
            "Changed here and in the library: pick a version",
            "button ghost icon library-sync changed",
        ),
        (Some(_), Some(LibrarySync::Gone)) => (
            "Not in the library",
            "The library no longer has this question",
            "button ghost icon library-sync",
        ),
    };
    let unfiled =
        question.library.is_none() && (question.category.is_empty() || question.topic.is_empty());
    let loading = question.library.is_some() && state.is_none();
    rsx! {
        button {
            r#type: "button",
            class,
            title: "{label}: {title}",
            aria_label: "{label}",
            disabled: unfiled || loading,
            onclick: move |_| {
                let mut wanted = library.wanted;
                wanted.set(true);
                open.set(true);
            },
            Icon { kind: IconKind::Save }
        }
        if open() {
            LibrarySyncDialog { index, question, on_close: move |()| open.set(false) }
        }
    }
}

/// Saves a question to the library, or brings it and its library question back in step, each
/// way said before it is done.
#[component]
fn LibrarySyncDialog(index: usize, question: Question, on_close: EventHandler<()>) -> Element {
    let draft: Draft = use_context();
    let library: Library = use_context();
    let token: AdminToken = use_context();
    let mut difficulty = use_signal(|| 1u8);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut as_new = use_signal(|| false);
    let link_id = question.library.as_ref().map(|l| l.id.clone());
    // How many other quizzes an update of the library reaches.
    let uses = use_resource(move || {
        let id = link_id.clone();
        let quiz = draft.quiz.peek().id.clone();
        async move {
            match id {
                Some(id) => library_question_uses(token.get(), id, quiz).await.ok(),
                None => None,
            }
        }
    });
    let Some(entries) = library.loaded() else {
        return rsx! {
            Dialog { title: "Question library", on_close, Spinner {} }
        };
    };
    let found = crate::library::sync(&question, &entries);
    // Saving as new shows what saving an unlinked question does.
    let state = found.map(|(state, _)| state).filter(|_| !as_new());
    let entry = found.and_then(|(_, entry)| entry).cloned();

    // Links the question to `entry`, as it now is in both.
    let relink = move |entry: &LibraryEntry| {
        let id = entry.id.clone();
        draft.edit_question(index, |q| {
            q.library = Some(LibraryLink {
                synced: fingerprint(q),
                id,
            });
        });
    };
    let save_new = {
        let question = question.clone();
        move |_| {
            let question = question.clone();
            async move {
                busy.set(true);
                let edit = LibraryEdit {
                    difficulty: difficulty(),
                    question,
                };
                match add_library_question(token.get(), edit).await {
                    Ok(entry) => {
                        relink(&entry);
                        library.entries.clone().restart();
                        on_close.call(());
                    }
                    Err(err) => error.set(Some(message(&err))),
                }
                busy.set(false);
            }
        }
    };
    let push = {
        let (question, entry) = (question.clone(), entry.clone());
        move |_| {
            let (question, entry) = (question.clone(), entry.clone());
            async move {
                let Some(entry) = entry else { return };
                busy.set(true);
                let edit = LibraryEdit {
                    difficulty: entry.difficulty,
                    question,
                };
                match edit_library_question(token.get(), entry.id.clone(), edit).await {
                    Ok(saved) => {
                        relink(&saved);
                        library.entries.clone().restart();
                        on_close.call(());
                    }
                    Err(err) => error.set(Some(message(&err))),
                }
                busy.set(false);
            }
        }
    };
    let pull = {
        let entry = entry.clone();
        move |_| {
            if let Some(entry) = &entry {
                draft.edit_question(index, |q| *q = pulled(q, entry));
            }
            on_close.call(());
        }
    };
    let unlink = move |_| {
        draft.edit_question(index, |q| q.library = None);
        on_close.call(());
    };

    let others = match uses() {
        None | Some(None | Some(0)) => "No other quiz uses it.".to_owned(),
        Some(Some(1)) => "1 other quiz uses it and will be offered the update.".to_owned(),
        Some(Some(n)) => format!("{n} other quizzes use it and will be offered the update."),
    };
    let duplicate = state.is_none()
        && entries
            .iter()
            .any(|e| crate::search::fold(&e.question.text) == crate::search::fold(&question.text));
    let changed = entry
        .as_ref()
        .map(|e| differences(&question, &e.to_question()))
        .unwrap_or_default();
    let title = match state {
        None | Some(LibrarySync::Gone) => "Save to library",
        Some(LibrarySync::InStep) => "In the library",
        Some(LibrarySync::ChangedHere) => "Update the library?",
        Some(LibrarySync::ChangedInLibrary) => "Update from the library?",
        Some(LibrarySync::ChangedInBoth) => "Changed in both",
    };
    rsx! {
        Dialog { title, class: "library-sync-dialog", on_close,
            match (state, &entry) {
                (None | Some(LibrarySync::Gone), _) => rsx! {
                    if state == Some(LibrarySync::Gone) {
                        p { "The library no longer has the question this one came from. Save it again, or keep it in this quiz only." }
                    }
                    p { "Saves the question to this server's library, under {question.category} · {question.topic}. Its time limit and points stay with the quiz." }
                    if duplicate {
                        p { class: "library-duplicate", "⚠ The library already has a question with this text." }
                    }
                    Choice {
                        label: "Difficulty",
                        options: vec![
                            ("Easy", "Most adults know the answer."),
                            ("Medium", "Keen quizzers know it."),
                            ("Hard", "For experts."),
                        ],
                        selected: usize::from(difficulty().clamp(1, 3) - 1),
                        on_select: move |i: usize| difficulty.set(u8::try_from(i + 1).unwrap_or(1)),
                    }
                },
                (Some(LibrarySync::InStep), _) => rsx! {
                    p { "This question is the same as the library's. Its time limit and points are the quiz's own." }
                },
                (Some(_), Some(entry)) => rsx! {
                    p { class: "muted small", "Differs in: {changed.join(\", \")}" }
                    div { class: "library-versions",
                        QuestionVersion { heading: "In this quiz", question: question.clone() }
                        QuestionVersion { heading: "In the library", question: entry.to_question() }
                    }
                    if matches!(state, Some(LibrarySync::ChangedHere | LibrarySync::ChangedInBoth)) {
                        p {
                            strong { "Update library" }
                            " replaces the library's question with this one: future imports get this version. "
                            "{others}"
                        }
                    }
                    if matches!(state, Some(LibrarySync::ChangedInLibrary | LibrarySync::ChangedInBoth)) {
                        p {
                            strong { "Update from library" }
                            " replaces this question with the library's version: your changes here are lost. Its time limit and points stay."
                        }
                    }
                },
                (Some(_), None) => rsx! {},
            }
            ErrorBox { error: error() }
            div { class: "row between",
                div { class: "row",
                    if question.library.is_some() {
                        button {
                            class: "button ghost",
                            title: "Keep the question in this quiz only",
                            onclick: unlink,
                            "Unlink"
                        }
                    }
                    if matches!(state, Some(LibrarySync::ChangedHere | LibrarySync::ChangedInBoth)) {
                        button {
                            class: "button ghost",
                            title: "Keep the library's question, and save this one next to it",
                            disabled: busy(),
                            onclick: move |_| {
                                if let Some(entry) = &entry {
                                    difficulty.set(entry.difficulty);
                                }
                                as_new.set(true);
                            },
                            "Save as new…"
                        }
                    }
                }
                div { class: "row",
                    button { class: "button ghost", onclick: move |_| on_close.call(()), "Cancel" }
                    match state {
                        None | Some(LibrarySync::Gone) => rsx! {
                            button { class: "button primary", disabled: busy(), onclick: save_new, "⇪ Save to library" }
                        },
                        Some(LibrarySync::InStep) => rsx! {},
                        _ => rsx! {
                            if matches!(state, Some(LibrarySync::ChangedInLibrary | LibrarySync::ChangedInBoth)) {
                                button { class: "button", disabled: busy(), onclick: pull, "⇩ Update from library" }
                            }
                            if matches!(state, Some(LibrarySync::ChangedHere | LibrarySync::ChangedInBoth)) {
                                button { class: "button primary", disabled: busy(), onclick: push, "⇪ Update library" }
                            }
                        },
                    }
                }
            }
        }
    }
}

/// One version of a question, to compare it with another.
#[component]
fn QuestionVersion(heading: &'static str, question: Question) -> Element {
    rsx! {
        div { class: "library-version stack-tight",
            h3 { "{heading}" }
            span { class: "muted small", "{question.category} · {question.topic}" }
            span { class: "strong", "{question.text}" }
            if !question.passage.is_empty() {
                span { class: "small library-passage", "{question.passage}" }
            }
            span { class: "small library-answer", "✓ {answer_line(&question)}" }
            if !question.explanation.is_empty() {
                span { class: "small muted", "{question.explanation}" }
            }
        }
    }
}
