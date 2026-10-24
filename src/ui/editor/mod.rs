//! The quiz editor.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    library::{LibraryDialog, use_library},
    widgets::{ClientOnly, ErrorBox, Spinner, message},
};
use crate::{
    api::{get_quiz, library_subjects, save_quiz, start_test, upload_limits},
    model::{Pair, Part, Question, QuestionKind, Quiz, QuizSettings, RevealMode, ValidationIssue},
};

mod answers;
mod bonus;
mod explanation;
mod library_sync;
mod navigator;
mod numbers;
mod order;
mod pairs;
mod parts;
mod pins;
mod question;
mod settings;
mod table;
mod uploads;

use bonus::BONUS_PART;
use library_sync::Library;
pub use library_sync::LibraryQuestionEditor;
use navigator::{NAVIGATOR_JS, NavItem, QuizNavigator};
use question::{QuestionCard, Subjects};
use settings::{SettingsCard, WifiSlideCard};
use table::{QuizTable, TableRow};
use uploads::Limits;

/// Seconds a gradually revealed picture stays fully visible, suggested when picking a reveal.
const DEFAULT_HOLD_SECS: u32 = 5;
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

    /// Edits the accepted answers of a typed-answer question, or of one `part` of a
    /// several-answers question.
    fn edit_accepted(self, index: usize, part: Option<usize>, f: impl FnOnce(&mut Vec<String>)) {
        if part == Some(BONUS_PART) {
            self.edit_question(index, |q| {
                if let Some(bonus) = &mut q.bonus {
                    f(&mut bonus.accepted);
                }
            });
            return;
        }
        self.edit_question(index, |q| match (&mut q.kind, part) {
            (QuestionKind::Text { accepted }, None) => f(accepted),
            (QuestionKind::Parts { parts }, Some(p)) => {
                if let Some(part) = parts.get_mut(p) {
                    f(&mut part.accepted);
                }
            }
            _ => {}
        });
    }

    fn edit_parts(self, index: usize, f: impl FnOnce(&mut Vec<Part>)) {
        self.edit_question(index, |q| {
            if let QuestionKind::Parts { parts } = &mut q.kind {
                f(parts);
            }
        });
    }

    fn edit_pairs(self, index: usize, f: impl FnOnce(&mut Vec<Pair>)) {
        self.edit_question(index, |q| {
            if let QuestionKind::Pairs { pairs } = &mut q.kind {
                f(pairs);
            }
        });
    }

    fn edit_items(self, index: usize, f: impl FnOnce(&mut Vec<String>)) {
        self.edit_question(index, |q| {
            if let QuestionKind::Order { items } = &mut q.kind {
                f(items);
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
    let limits = use_resource(move || async move { upload_limits(token.get()).await.ok() });
    use_context_provider(|| Limits(limits));
    let subjects = use_resource(move || async move { library_subjects(token.get()).await.ok() });
    use_context_provider(|| Subjects(subjects));
    // The library, to compare linked questions with: loaded once a question is linked, or one
    // is about to be saved to it.
    let linked = use_memo(move || {
        draft
            .quiz
            .read()
            .questions
            .iter()
            .any(|q| q.library.is_some())
    });
    let wanted = use_signal(|| false);
    let library = use_library(move || linked() || wanted());
    use_context_provider(|| Library {
        entries: library,
        wanted,
    });
    let save_state = use_autosave(token, draft, issues);
    // The order before the last shuffle, and the revision the shuffle produced: undo is offered
    // until anything else changes.
    let mut before_shuffle = use_signal(|| None::<(u64, Vec<Question>)>);
    let mut shuffle_with = move |shuffle: fn(&mut Quiz)| {
        let previous = draft.quiz.peek().questions.clone();
        draft.edit(shuffle);
        before_shuffle.set(Some((*draft.revision.peek(), previous)));
    };
    let shuffle = move |_| shuffle_with(|q| q.shuffle_questions(client::random_below));
    let shuffle_answers = move |_| shuffle_with(|q| q.shuffle_answers(client::random_below));
    let undo_shuffle = move |_| {
        if let Some((_, previous)) = before_shuffle.take() {
            draft.edit(|q| q.questions = previous);
        }
    };
    let can_undo = before_shuffle
        .read()
        .as_ref()
        .is_some_and(|(revision, _)| *revision == (draft.revision)());

    let mut browsing = use_signal(|| false);
    // The quiz as a table, one row per question or slide, instead of its cards.
    let mut table = use_signal(|| false);
    // Shows the cards or the table, at the item named (or the one at the top of the list).
    let mut switch_view = move |to_table: bool, at: Option<String>| {
        table.set(to_table);
        document::eval(&format!(
            "const id = {:?} || decodeURIComponent(location.hash.slice(1));
            requestAnimationFrame(() => requestAnimationFrame(() => window.qnav?.jump(id, 'instant')));",
            at.unwrap_or_default()
        ));
    };
    let mut test_error = use_signal(|| None::<String>);
    let nav = navigator();
    // Save the latest edits first, so the test plays what is on screen. `from` starts the test
    // at that question or slide instead of the lobby.
    let run_test = move |from: Option<usize>| async move {
        let quiz = draft.quiz.peek().clone();
        let id = quiz.id.clone();
        let result = match save_quiz(token.get(), quiz).await {
            Ok(found) if found.is_empty() => start_test(token.get(), id, from).await,
            Ok(_) => Err(ServerFnError::new("Fix the quiz before testing it.")),
            Err(err) => Err(err),
        };
        match result {
            Ok(code) => {
                nav.push(Route::Test { code });
            }
            Err(err) => {
                test_error.set(Some(message(&err)));
                document::eval("window.scrollTo({ top: 0, behavior: 'smooth' })");
            }
        }
    };
    let test = move |_| run_test(None);
    use_context_provider(|| {
        TestFrom(EventHandler::new(move |index| {
            spawn(run_test(Some(index)));
        }))
    });

    let quiz = draft.quiz.read();
    let question_count = quiz.questions.len();
    let can_shuffle = quiz.can_shuffle();
    let can_shuffle_answers = quiz.can_shuffle_answers();
    let flagged: Vec<usize> = issues.read().iter().filter_map(|i| i.question).collect();
    let nav_items: Vec<NavItem> = (0..question_count)
        .map(|index| NavItem::new(&quiz, index, flagged.contains(&index)))
        .collect();
    use_effect(|| {
        document::eval(NAVIGATOR_JS);
    });
    use_drop(|| {
        document::eval("window.qnav?.stop()");
    });
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
        main { class: "page editor",
            div { class: "editor-top stack",
                input {
                    class: "title-input",
                    placeholder: "Quiz title",
                    value: "{quiz.title}",
                    oninput: move |e| draft.edit(|q| q.title = e.value()),
                }
                Issues { issues: issues() }
                ErrorBox { error: test_error() }
                SettingsCard { settings: quiz.settings.clone() }
            }
            aside { class: "editor-side editor-add",
                button {
                    class: "button primary",
                    onclick: move |_| draft.edit(|q| q.questions.push(Question::blank())),
                    "＋ Add question"
                }
                button {
                    class: "button add-slide",
                    title: "A title, text or picture shown between questions",
                    onclick: move |_| draft.edit(|q| q.questions.push(Question::slide())),
                    "＋ Add slide"
                }
                button {
                    class: "button add-library",
                    title: "Pick ready-made questions from the question library",
                    onclick: move |_| browsing.set(true),
                    "＋ From the library"
                }
                if quiz.settings.reveal == RevealMode::AtEnd {
                    button {
                        class: "button add-slide",
                        title: "Reveal the answers so far and show the standings, e.g. at an intermission",
                        onclick: move |_| draft.edit(|q| q.questions.push(Question::results_slide())),
                        "＋ Add results slide"
                    }
                }
            }
            div { class: "editor-list stack",
                if table() {
                    QuizTable {
                        rows: (0..question_count).map(|index| TableRow::new(&quiz, index, &issues.read())).collect::<Vec<_>>(),
                        on_open: move |anchor| switch_view(false, Some(anchor)),
                        on_move: move |(from, to): (usize, usize)| draft.edit(|q| {
                            if from < q.questions.len() && to < q.questions.len() {
                                let item = q.questions.remove(from);
                                q.questions.insert(to, item);
                            }
                        }),
                    }
                } else {
                    if quiz.settings.network.enabled {
                        WifiSlideCard { network: quiz.settings.network.clone() }
                    }
                    for index in 0..question_count {
                        QuestionCard {
                            key: "{index}",
                            index,
                            label: quiz.label(index),
                            count: question_count,
                            question: quiz.questions[index].clone(),
                            default_limit: quiz.settings.default_time_limit_secs,
                        }
                    }
                }
            }
            if browsing() {
                LibraryDialog {
                    present: quiz.questions.iter().map(|q| q.text.clone()).collect::<Vec<_>>(),
                    on_add: move |questions: Vec<Question>| draft.edit(|q| q.questions.extend(questions)),
                    on_close: move |()| browsing.set(false),
                }
            }
            aside { class: "editor-side editor-shuffle",
                div { class: "editor-tools",
                    button {
                        class: "button",
                        disabled: !issues().is_empty(),
                        title: if issues().is_empty() { "Try the quiz with the big screen and two phones side by side" } else { "Fix the quiz before testing it" },
                        onclick: test,
                        "🧪 Test quiz"
                    }
                    button {
                        class: "button ghost shuffle",
                        disabled: !can_shuffle,
                        title: "Shuffle the order of the questions. Slides stay in place and questions never move past a slide.",
                        onclick: shuffle,
                        "🔀 Shuffle questions"
                    }
                    button {
                        class: "button ghost shuffle",
                        disabled: !can_shuffle_answers,
                        title: "Shuffle the answers of every multiple-choice question. The right answers stay right.",
                        onclick: shuffle_answers,
                        "🔀 Shuffle answers"
                    }
                    if can_undo {
                        button { class: "button ghost", onclick: undo_shuffle, "↶ Undo shuffle" }
                    }
                    button {
                        class: "button ghost",
                        title: if table() { "Edit the questions on their cards" } else { "See the whole quiz at a glance, one row per question or slide" },
                        onclick: move |_| switch_view(!table(), None),
                        if table() { "▦ Card view" } else { "☰ Table view" }
                    }
                }
                QuizNavigator { items: nav_items, hidden: table() }
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

/// Starts a test at the question or slide at the given position, saving the quiz first.
#[derive(Clone, Copy)]
struct TestFrom(EventHandler<usize>);
