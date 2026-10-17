//! The quiz editor.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    library::{LibraryDialog, use_library},
    widgets::{
        AnswerBadge, ClientOnly, Dialog, ErrorBox, Icon, IconKind, MapPin, PinMap, STAR, Spinner,
        media_url, message,
    },
};
use crate::{
    api::{
        add_library_question, edit_library_question, get_quiz, library_question_uses,
        library_subjects, revert_library_question, save_quiz, start_test, upload_limits,
        upload_question_audio, upload_question_image, upload_question_media,
    },
    library::{
        LibraryCategory, LibraryEdit, LibraryEntry, LibrarySync, answer_line, differences,
        fingerprint, pulled,
    },
    model::{
        AdvanceMode, Answer, BONUS_PERCENTS, Bonus, ImageReveal, LibraryLink, MAX_ANSWERS,
        MAX_MULTIPLIER, MAX_NETWORK_NAME_BYTES, MAX_NETWORK_PASSWORD_CHARS, MAX_PARTS,
        MAX_TIME_LIMIT_SECS, MIN_ANSWERS, MIN_PARTS, MIN_TIME_LIMIT_SECS, MultiScoring,
        NetworkSlide, NumberRange, Pair, PairItem, Part, PhoneDisplay, PinTolerance, Point,
        PointsMode, Question, QuestionKind, Quiz, QuizSettings, RevealMode, ValidationIssue,
        is_video,
    },
    protocol::UploadLimits,
};

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

/// The server's upload limits, once known.
#[derive(Clone, Copy)]
struct Limits(Resource<Option<UploadLimits>>);

impl Limits {
    fn get(self) -> Option<UploadLimits> {
        (*self.0.read()).flatten()
    }
}

/// An error when `file` is larger than `max` bytes, before spending time uploading it.
fn too_large(file: &dioxus::html::FileData, max: Option<u64>, what: &str) -> Option<String> {
    let max = max?;
    (file.size() > max).then(|| {
        format!(
            "This {what} is too large (max {} MB).",
            UploadLimits::megabytes(max)
        )
    })
}

/// An error when a picture or video is larger than the server takes.
fn media_too_large(file: &dioxus::html::FileData, limits: Limits) -> Option<String> {
    let limits = limits.get();
    if file.content_type().is_some_and(|t| t.starts_with("video/")) {
        too_large(file, limits.map(|l| l.video), "video")
    } else {
        too_large(file, limits.map(|l| l.picture), "picture")
    }
}

/// Upload button title: what pictures and videos the server takes.
fn media_limits_title(limits: Limits) -> String {
    match limits.get() {
        Some(l) => format!(
            "Pictures up to {} MB; videos (MP4, WebM, MOV) up to {} MB",
            UploadLimits::megabytes(l.picture),
            UploadLimits::megabytes(l.video),
        ),
        None => "A picture, or a video (MP4, WebM, MOV)".to_owned(),
    }
}

/// Accepted by picture-or-video uploads.
const MEDIA_ACCEPT: &str = "image/*,video/mp4,video/webm,video/quicktime";

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
                QuizNavigator { items: nav_items }
            }
        }
    }
}

/// Edits a library question in a pop-up, with the editor's question card. Saving keeps the change
/// on this server; an edited question can go back to how the library has it.
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
                "Changes are kept on this server only; the library that comes with Quaesito stays as it is."
            }
            div { class: "row between",
                if entry.edited {
                    button { class: "button ghost", onclick: revert, "↶ Back to the original" }
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
            label { class: "field inline beside",
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
                    label: "Several correct answers",
                    options: vec![
                        ("All or nothing", "Points only for picking exactly the correct answers."),
                        ("Partial credit", "Credit for each correct pick, minus wrong picks."),
                    ],
                    selected: usize::from(settings.multi_scoring == MultiScoring::Partial),
                    on_select: move |i| draft.edit_settings(|s| s.multi_scoring = if i == 0 { MultiScoring::AllOrNothing } else { MultiScoring::Partial }),
                }
                Choice {
                    label: "Points",
                    options: vec![
                        ("Speed", "Faster correct answers earn more: 1000 points when instant, down to 500 at the buzzer."),
                        ("Correct answers only", "Every correct answer earns 1000 points, however long it took. Ties go to the faster team."),
                    ],
                    selected: usize::from(settings.points == PointsMode::Flat),
                    on_select: move |i| draft.edit_settings(|s| s.points = if i == 0 { PointsMode::Speed } else { PointsMode::Flat }),
                }
                Choice {
                    label: "Jokers",
                    options: vec![
                        ("Off", "Every question counts the same."),
                        ("On", "Each team may play a joker once, while answering, to double that question's points."),
                    ],
                    selected: usize::from(settings.joker),
                    on_select: move |i| draft.edit_settings(|s| s.joker = i == 1),
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
                Choice {
                    label: "Wi-Fi slide",
                    options: vec![
                        ("Off", "The game opens on the join code."),
                        ("On", "The game opens on the Wi-Fi's name, password and a QR code that connects phones to it. Fill them in on the first card below."),
                    ],
                    selected: usize::from(settings.network.enabled),
                    on_select: move |i| draft.edit_settings(|s| s.network.enabled = i == 1),
                }
            }
        }
    }
}

/// The Wi-Fi slide that opens the game, while the setting is on: the network's name and password.
#[component]
fn WifiSlideCard(network: NetworkSlide) -> Element {
    let draft: Draft = use_context();
    rsx! {
        section { id: "wifi-slide", class: "card stack question slide-card",
            div { class: "row between",
                h3 { "Wi-Fi slide" }
                button {
                    class: "button ghost icon danger-icon",
                    title: "Turn off the Wi-Fi slide (the name and password are kept)",
                    aria_label: "Turn off the Wi-Fi slide",
                    onclick: move |_| draft.edit_settings(|s| s.network.enabled = false),
                    Icon { kind: IconKind::Trash }
                }
            }
            p { class: "muted",
                "📶 Opens the game with the network's name, password and a QR code that connects phones to it. The join code comes next."
            }
            div { class: "row",
                label { class: "field inline",
                    span { "Network name" }
                    input {
                        r#type: "text",
                        maxlength: MAX_NETWORK_NAME_BYTES,
                        value: "{network.name}",
                        oninput: move |e| draft.edit_settings(|s| s.network.name = e.value()),
                    }
                }
                label { class: "field inline",
                    title: "Leave empty for an open network",
                    span { "Password" }
                    input {
                        r#type: "text",
                        maxlength: MAX_NETWORK_PASSWORD_CHARS,
                        value: "{network.password}",
                        oninput: move |e| draft.edit_settings(|s| s.network.password = e.value()),
                    }
                }
            }
            if let Some(problem) = network.problem() {
                p { class: "field-error", "{problem}" }
            }
        }
    }
}

/// Starts a test at the question or slide at the given position, saving the quiz first.
#[derive(Clone, Copy)]
struct TestFrom(EventHandler<usize>);

/// The `part` of [`AcceptedAnswers`] that stands for a question's bonus question.
const BONUS_PART: usize = usize::MAX;

/// A question's optional bonus question: its text, what it is worth and its accepted answers.
#[component]
fn BonusEditor(question: usize, bonus: Option<Bonus>) -> Element {
    let draft: Draft = use_context();
    let Some(bonus) = bonus else {
        return rsx! {
            button {
                class: "button ghost add-bonus",
                title: "A typed question asked together with this one, worth a share of its points",
                onclick: move |_| draft.edit_question(question, |q| {
                    q.bonus = Some(Bonus {
                        accepted: vec![String::new()],
                        ..Bonus::default()
                    });
                }),
                "＋ Add a bonus question"
            }
        };
    };
    let selected = BONUS_PERCENTS
        .iter()
        .position(|&p| p == bonus.percent)
        .unwrap_or(1);
    rsx! {
        fieldset { class: "bonus-edit stack",
            legend { "Bonus question" }
            div { class: "row",
                input {
                    class: "grow",
                    placeholder: "e.g. Who sings it?",
                    value: "{bonus.text}",
                    oninput: move |e| {
                        let text = e.value();
                        draft.edit_question(question, |q| {
                            if let Some(bonus) = &mut q.bonus {
                                bonus.text = text;
                            }
                        });
                    },
                }
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_question(question, |q| q.bonus = None),
                    "Remove bonus"
                }
            }
            Choice {
                label: "Worth",
                options: vec![
                    ("+25%", "A right bonus answer adds a quarter of the points the question can earn."),
                    ("+50%", "A right bonus answer adds half the points the question can earn."),
                    ("+100%", "A right bonus answer doubles the points the question can earn."),
                ],
                selected,
                on_select: move |i: usize| draft.edit_question(question, |q| {
                    if let Some(bonus) = &mut q.bonus {
                        bonus.percent = BONUS_PERCENTS[i];
                    }
                }),
            }
            AcceptedAnswers { question, part: BONUS_PART, accepted: bonus.accepted }
        }
    }
}

/// What the big screen shows only once the answer is revealed: a text, a picture or a video.
#[component]
fn ExplanationEditor(
    question: usize,
    text: String,
    image: Option<String>,
    audio: Option<String>,
) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let limits: Limits = use_context();
    let mut opened = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
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
            Ok(name) => draft.edit_question(question, |q| q.explanation_image = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    let mut uploading_audio = use_signal(|| false);
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
            Ok(name) => draft.edit_question(question, |q| q.explanation_audio = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading_audio.set(false);
    };
    if text.is_empty() && image.is_none() && audio.is_none() && !opened() {
        return rsx! {
            button {
                class: "button ghost add-bonus",
                title: "An explanation, a fun fact, a picture or a soundtrack, shown on the big screen with the answer",
                onclick: move |_| opened.set(true),
                "＋ Add something to show with the answer"
            }
        };
    }
    rsx! {
        fieldset { class: "bonus-edit stack",
            legend { "Shown with the answer" }
            textarea {
                class: "question-passage-input",
                placeholder: "An explanation, a fun fact… (optional)",
                rows: 2,
                value: "{text}",
                oninput: move |e| draft.edit_question(question, |q| q.explanation = e.value()),
            }
            div { class: "question-media stack",
                if let Some(image) = &image {
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
                                img { class: "thumb", src: media_url(image), alt: "Picture shown with the answer" }
                            }
                            button {
                                class: "button ghost",
                                onclick: move |_| draft.edit_question(question, |q| q.explanation_image = None),
                                if is_video(image) { "Remove video" } else { "Remove picture" }
                            }
                        }
                    }
                }
                if let Some(audio) = &audio {
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
                                onclick: move |_| draft.edit_question(question, |q| q.explanation_audio = None),
                                "Remove soundtrack"
                            }
                        }
                    }
                }
                div { class: "row",
                    if image.is_none() {
                        label { class: "button ghost",
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
                    if audio.is_none() {
                        label { class: "button ghost",
                            title: "Music or a sound the big screen plays as it shows the answer",
                            if uploading_audio() { "Uploading…" } else { "＋ Add soundtrack" }
                            input {
                                r#type: "file",
                                accept: "audio/*",
                                class: "visually-hidden",
                                onchange: upload_audio,
                            }
                        }
                    }
                    span { class: "grow" }
                    button {
                        class: "button ghost",
                        onclick: move |_| {
                            opened.set(false);
                            draft.edit_question(question, |q| {
                                q.explanation.clear();
                                q.explanation_image = None;
                                q.explanation_audio = None;
                            });
                        },
                        "Remove"
                    }
                }
            }
            ErrorBox { error: upload_error() }
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

/// The URL fragment of the card labelled `label`: "Question 3" is at `#question-3`.
fn anchor(label: &str) -> String {
    label.to_lowercase().replace(' ', "-")
}

/// A question or slide as the navigator shows it.
#[derive(Clone, PartialEq)]
struct NavItem {
    anchor: String,
    /// The question's number, or the slide's: "12", "S2".
    number: String,
    /// The start of its text, shown beside the number where there is room.
    text: String,
    /// The label and the start of the text, on hover.
    title: String,
    slide: bool,
    /// Something keeps it from being played.
    flagged: bool,
}

impl NavItem {
    fn new(quiz: &Quiz, index: usize, flagged: bool) -> Self {
        const TEXT_CHARS: usize = 80;
        let question = &quiz.questions[index];
        let label = quiz.label(index);
        let number = quiz.ordinal(index) + 1;
        let full = question.text.trim();
        let text = match full.char_indices().nth(TEXT_CHARS) {
            Some((cut, _)) => format!("{}…", &full[..cut]),
            None => full.to_owned(),
        };
        Self {
            anchor: anchor(&label),
            number: if question.is_slide() {
                format!("S{number}")
            } else {
                number.to_string()
            },
            title: if text.is_empty() {
                label
            } else {
                format!("{label}: {text}")
            },
            text,
            slide: question.is_slide(),
            flagged,
        }
    }
}

/// Every question and slide as a link, in a strip that always fits: thin bars, widened around
/// the cards on screen and around the pointer. A click (or a finger lifted after sliding along
/// it) scrolls there.
#[component]
fn QuizNavigator(items: Vec<NavItem>) -> Element {
    rsx! {
        nav { class: "editor-nav", aria_label: "Questions and slides",
            for item in items {
                a {
                    key: "{item.anchor}",
                    href: "#{item.anchor}",
                    class: match (item.slide, item.flagged) {
                        (true, true) => "slide flagged",
                        (true, false) => "slide",
                        (false, true) => "flagged",
                        (false, false) => "",
                    },
                    title: "{item.title}",
                    onclick: move |e| {
                        e.prevent_default();
                        document::eval(&format!("window.qnav?.jump({:?})", item.anchor));
                    },
                    span { class: "nav-number", "{item.number}" }
                    span { class: "nav-text", "{item.text}" }
                }
            }
        }
    }
}

/// A question or slide as the table shows it.
#[derive(Clone, PartialEq)]
struct TableRow {
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
    fn new(quiz: &Quiz, index: usize, issues: &[ValidationIssue]) -> Self {
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

/// The whole quiz in a table, one row per question or slide; a row opens its card.
#[component]
fn QuizTable(rows: Vec<TableRow>, on_open: EventHandler<String>) -> Element {
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

/// Sizes the navigator's links (open around the cards on screen, or around the pointer or a
/// finger sliding along it, thin elsewhere), lights the ones on screen, keeps the card at the top
/// of the list in the URL fragment (without adding history entries), and on load scrolls back to
/// the card the fragment names, following it while pictures above it load, until the user scrolls.
const NAVIGATOR_JS: &str = r#"
window.qnav?.stop();
const list = document.querySelector(".editor-list");
const nav = document.querySelector(".editor-nav");
if (list && nav) {
    const root = document.documentElement;
    const topbar = document.querySelector(".topbar");
    // Below this width the navigator lies across the top of the list instead of down the side.
    const narrow = matchMedia("(max-width: 1300px)");
    const listen = new AbortController();
    const on = (target, name, handler, options = {}) =>
        target.addEventListener(name, handler, { ...options, signal: listen.signal });
    // Where cards come into view: under the header, and on narrow screens under the navigator
    // and the buttons held below it.
    const held = [".topbar", ".editor-nav", ".editor-add", ".editor-tools"];
    // Measured where they stick, not where they are: near the top of the page they are lower.
    const offset = () => Math.max(0, ...(narrow.matches ? held : held.slice(0, 1)).map((selector) => {
        const element = document.querySelector(selector);
        return element ? (parseFloat(getComputedStyle(element).top) || 0) + element.offsetHeight : 0;
    })) + 8;
    // The cards, or the table's rows in their place.
    const cards = () => [...list.querySelectorAll(":scope > [id], tbody > tr[id]")];
    const chips = () => [...nav.children];
    let following = null;
    let shown = null;
    // The cards on screen, as the first and last index; and where the pointer points, if anywhere.
    let onScreen = [0, -1];
    let pointer = null;
    let scrubbing = false;
    let scrubbedAt = 0;
    const align = (card, behavior) => window.scrollTo({
        top: card.getBoundingClientRect().top + window.scrollY - offset(),
        behavior,
    });
    const layout = () => {
        const all = chips();
        const count = all.length;
        if (!count) return;
        const across = narrow.matches;
        const length = across ? nav.clientWidth : nav.clientHeight;
        // Tighter where many links share little room, as on a phone.
        const gap = count * 4 > length ? 1 : 2;
        nav.style.gap = `${gap}px`;
        const room = length - gap * (count - 1);
        const thinnest = 2;
        let open = across ? 40 : 30;
        // Fully open within `reach` of the focus, then narrowing over three more.
        const [first, last, reach] = pointer === null
            ? [...onScreen, 2]
            : [Math.floor(pointer), Math.floor(pointer), 3];
        const sizes = all.map((_, i) => {
            if (count * open <= room) return 1;
            const away = i < first ? first - i : i > last ? i - last : 0;
            return Math.max(0, Math.min(1, 1 - (away - reach) / 3));
        });
        const opened = sizes.reduce((sum, size) => sum + size, 0);
        open = Math.min(open, (room - thinnest * (count - opened)) / opened);
        const thin = opened >= count ? 0 : Math.max(thinnest, (room - open * opened) / (count - opened));
        all.forEach((chip, i) => {
            chip.style.flexBasis = `${thin + (open - thin) * sizes[i]}px`;
            chip.toggleAttribute("data-open", sizes[i] === 1);
        });
    };
    const update = () => {
        const top = offset();
        const all = chips();
        let current = -1;
        onScreen = [0, -1];
        cards().forEach((card, i) => {
            const at = card.getBoundingClientRect();
            const visible = at.bottom > top && at.top < window.innerHeight;
            all[i]?.toggleAttribute("data-visible", visible);
            all[i]?.removeAttribute("aria-current");
            if (visible) onScreen = [onScreen[1] < 0 ? i : onScreen[0], i];
            if (at.top <= top + 1) current = i;
        });
        all[current]?.setAttribute("aria-current", "location");
        layout();
        const id = cards()[current]?.id ?? null;
        if (id === shown) return;
        shown = id;
        const url = shown ? `#${shown}` : location.pathname + location.search;
        history.replaceState(history.state, "", url);
    };
    let frame = 0;
    const schedule = () => {
        if (!frame) frame = requestAnimationFrame(() => {
            frame = 0;
            if (following) align(following, "instant");
            update();
        });
    };
    const letGo = () => { following = null; };
    // The pointer's place along the strip, in links, as if they were all the same size: the open
    // part then stays put under it instead of sliding away as the links resize.
    const along = (e) => {
        const box = nav.getBoundingClientRect();
        const share = narrow.matches
            ? (e.clientX - box.left) / box.width
            : (e.clientY - box.top) / box.height;
        return Math.max(0, Math.min(0.999, share)) * nav.children.length;
    };
    on(nav, "pointermove", (e) => { pointer = along(e); schedule(); });
    on(nav, "pointerleave", () => { if (!scrubbing) { pointer = null; schedule(); } });
    // A finger slides along the strip, opening it where it goes, and jumps where it lifts.
    on(nav, "pointerdown", (e) => {
        if (e.pointerType === "mouse") return;
        scrubbing = true;
        nav.setPointerCapture(e.pointerId);
        pointer = along(e);
        schedule();
    });
    on(nav, "pointerup", (e) => {
        if (!scrubbing) return;
        scrubbing = false;
        scrubbedAt = Date.now();
        const chip = document.elementFromPoint(e.clientX, e.clientY)?.closest(".editor-nav a");
        const card = cards()[chip ? chips().indexOf(chip) : Math.floor(along(e))];
        pointer = null;
        letGo();
        if (card) align(card, "smooth");
        schedule();
    });
    on(nav, "pointercancel", () => { scrubbing = false; pointer = null; schedule(); });
    // Tabbing through the links opens the strip around the focused one. A click focuses a link
    // too, but there the pointer already steers, and moving the link would lose the click.
    on(nav, "focusin", (e) => {
        if (!e.target.matches(":focus-visible")) return;
        pointer = chips().indexOf(e.target.closest("a"));
        schedule();
    });
    on(nav, "focusout", () => { if (!nav.matches(":hover")) { pointer = null; schedule(); } });
    // The heights the held bars stack on, kept for the CSS.
    const add = document.querySelector(".editor-add");
    const sizes = new ResizeObserver(() => {
        root.style.setProperty("--topbar-height", `${topbar?.offsetHeight ?? 0}px`);
        root.style.setProperty("--editor-add-height", `${add?.offsetHeight ?? 0}px`);
        schedule();
    });
    if (topbar) sizes.observe(topbar);
    if (add) sizes.observe(add);
    sizes.observe(list);
    sizes.observe(nav);
    const changes = new MutationObserver(schedule);
    changes.observe(list, { childList: true, subtree: true, attributeFilter: ["id"] });
    changes.observe(nav, { childList: true });
    for (const name of ["wheel", "touchstart", "keydown", "pointerdown"]) {
        on(window, name, letGo, { passive: true });
    }
    on(window, "scroll", schedule, { passive: true });
    on(window, "resize", schedule);
    const timer = setTimeout(letGo, 3000);
    const id = decodeURIComponent(location.hash.slice(1));
    const target = id && document.getElementById(id);
    if (target && list.contains(target)) following = target;
    requestAnimationFrame(() => requestAnimationFrame(schedule));
    window.qnav = {
        jump(id, behavior = "smooth") {
            // The click that follows a finger lifted from the strip.
            if (Date.now() - scrubbedAt < 500) return;
            const card = document.getElementById(id);
            following = null;
            if (card) align(card, behavior);
        },
        stop() {
            listen.abort();
            sizes.disconnect();
            changes.disconnect();
            clearTimeout(timer);
            cancelAnimationFrame(frame);
            delete window.qnav;
        },
    };
}
"#;

/// Keeps a moved question or slide in view: once the cards are redrawn, scrolls so the card at `to`
/// sits where the one at `from` was on screen, and focuses the same move button on it (or its
/// first enabled one, when that button is now disabled at the top or bottom).
fn follow_move(from: usize, to: usize, button: &str) {
    document::eval(&format!(
        r#"const cards = () => document.querySelectorAll(".editor-list > .question");
        const top = cards()[{from}]?.getBoundingClientRect().top ?? 0;
        requestAnimationFrame(() => requestAnimationFrame(() => {{
            const card = cards()[{to}];
            if (!card) return;
            window.scrollBy({{ top: card.getBoundingClientRect().top - top, behavior: "instant" }});
            const buttons = [...card.querySelectorAll(".actions button")].filter((b) => !b.disabled);
            (buttons.find((b) => b.title === "{button}") ?? buttons[0])?.focus({{ preventScroll: true }});
        }}));"#
    ));
}

#[component]
fn QuestionCard(
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
                        ("Number", "Teams guess a number; the closest guess wins 1000 points."),
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
                    QuestionKind::Number { answer, range } => rsx! {
                        NumberAnswer { question: index, answer: *answer, range: *range }
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

/// The library as the editor compares linked questions with it, with this server's edits and
/// added questions. Setting `wanted` loads it even with no question linked.
#[derive(Clone, Copy)]
struct Library {
    entries: Resource<Option<Result<Vec<LibraryEntry>, ServerFnError>>>,
    wanted: Signal<bool>,
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
fn LibrarySyncButton(index: usize, question: Question) -> Element {
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

/// The library's categories with their topics, those added on this server included: the ones
/// questions can be filed under. `None` until loaded.
#[derive(Clone, Copy)]
struct Subjects(Resource<Option<Vec<LibraryCategory>>>);

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
        },
        _ => {
            if question.answers.len() < MIN_ANSWERS {
                question.answers.resize_with(MIN_ANSWERS, Answer::default);
            }
            QuestionKind::Choice
        }
    };
}

/// The accepted answers of a typed-text question.
#[component]
fn AcceptedAnswers(
    question: usize,
    #[props(default)] part: Option<usize>,
    accepted: Vec<String>,
) -> Element {
    let draft: Draft = use_context();
    let count = accepted.len();
    rsx! {
        ul { class: "answers-edit",
            for (i, text) in accepted.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit",
                    input {
                        class: "grow",
                        placeholder: if i == 0 { "Accepted answer" } else { "Another accepted spelling" },
                        value: "{text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_accepted(question, part, |a| a[i] = value);
                        },
                    }
                    button {
                        class: "button ghost icon",
                        title: "Remove",
                        disabled: count <= 1,
                        onclick: move |_| draft.edit_accepted(question, part, |a| {
                            a.remove(i);
                        }),
                        "✕"
                    }
                }
            }
        }
        div { class: "row between",
            button {
                class: "button ghost",
                onclick: move |_| draft.edit_accepted(question, part, |a| a.push(String::new())),
                "＋ Add accepted answer"
            }
            span { class: "muted small",
                "Case, accents, punctuation and spacing are ignored; spelling must match."
            }
        }
    }
}

/// The right spot of a pin question, clicked on its picture, and how close pins must be.
#[component]
fn PinEditor(
    question: usize,
    image: Option<String>,
    answer: Option<Point>,
    aspect: f64,
    tolerance: PinTolerance,
) -> Element {
    let draft: Draft = use_context();
    let Some(image) = image else {
        return rsx! {
            p { class: "muted", "Add the picture above (a map, a floor plan…), then click the right spot on it." }
        };
    };
    let set_tolerance = move |tolerance: PinTolerance| {
        draft.edit_question(question, |q| {
            if let QuestionKind::Pin { tolerance: t, .. } = &mut q.kind {
                *t = tolerance;
            }
        });
    };
    rsx! {
        p { class: "muted small",
            if answer.is_some() { "Click to move the right spot." } else { "Click the right spot on the picture." }
        }
        div { class: "pin-edit",
            PinMap {
                aspect,
                image,
                pins: answer
                    .map(|at| MapPin {
                        at,
                        label: STAR.to_owned(),
                        title: "The right spot".to_owned(),
                        class: "answer",
                    })
                    .into_iter()
                    .collect::<Vec<_>>(),
                on_pick: EventHandler::new(move |(at, aspect): (Point, f64)| {
                    draft.edit_question(question, |q| {
                        if let QuestionKind::Pin { answer, aspect: a, .. } = &mut q.kind {
                            *answer = Some(at);
                            *a = aspect;
                        }
                    });
                }),
            }
        }
        Choice {
            label: "How close",
            options: vec![
                ("Tight", "Full points within 2% of the picture's width, none beyond 15%."),
                ("Normal", "Full points within 4% of the picture's width, none beyond 30%."),
                ("Loose", "Full points within 8% of the picture's width, none beyond 50%."),
            ],
            selected: match tolerance {
                PinTolerance::Tight => 0,
                PinTolerance::Normal => 1,
                PinTolerance::Loose => 2,
            },
            on_select: move |i| set_tolerance(match i {
                0 => PinTolerance::Tight,
                2 => PinTolerance::Loose,
                _ => PinTolerance::Normal,
            }),
        }
    }
}

/// A new, empty part of a several-answers question.
fn blank_part() -> Part {
    Part {
        accepted: vec![String::new()],
        ..Part::default()
    }
}

/// The parts of a several-answers question: a label, an optional picture and accepted answers
/// each.
#[component]
fn PartsEditor(question: usize, parts: Vec<Part>) -> Element {
    let draft: Draft = use_context();
    let count = parts.len();
    rsx! {
        div { class: "parts-edit",
            for (i, part) in parts.into_iter().enumerate() {
                div { key: "{i}", class: "part-edit stack",
                    div { class: "row",
                        span { class: "order-rank", "{i + 1}" }
                        input {
                            class: "grow",
                            placeholder: "What to type, e.g. Title, Artist, Poster {i + 1}",
                            value: "{part.label}",
                            oninput: move |e| {
                                let value = e.value();
                                draft.edit_parts(question, |parts| parts[i].label = value);
                            },
                        }
                        PartImage { question, part: i, image: part.image.clone() }
                        button {
                            class: "button ghost icon danger-icon",
                            title: "Remove this answer",
                            aria_label: "Remove this answer",
                            disabled: count <= MIN_PARTS,
                            onclick: move |_| draft.edit_parts(question, |parts| {
                                parts.remove(i);
                            }),
                            Icon { kind: IconKind::Trash }
                        }
                    }
                    AcceptedAnswers { question, part: Some(i), accepted: part.accepted.clone() }
                }
            }
        }
        div { class: "row between",
            if count < MAX_PARTS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_parts(question, |parts| parts.push(blank_part())),
                    "＋ Add answer"
                }
            } else {
                span {}
            }
            span { class: "muted small", "The question's points are shared evenly between its answers." }
        }
    }
}

/// A part's own picture: upload, thumbnail and removal.
#[component]
fn PartImage(question: usize, part: usize, image: Option<String>) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        upload_error.set(None);
        match upload_question_image(token.get(), file.into()).await {
            // Parts may have been removed while uploading: only set a part that still exists.
            Ok(name) => draft.edit_parts(question, |parts| {
                if let Some(p) = parts.get_mut(part) {
                    p.image = Some(name);
                }
            }),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    match image {
        Some(image) => rsx! {
            img { class: "answer-thumb", src: media_url(&image), alt: "Picture for this answer" }
            button {
                class: "button ghost icon",
                title: "Remove picture",
                onclick: move |_| draft.edit_parts(question, |parts| {
                    if let Some(p) = parts.get_mut(part) {
                        p.image = None;
                    }
                }),
                "✕🖼"
            }
        },
        None => rsx! {
            label {
                class: "button ghost icon",
                title: upload_error().unwrap_or_else(|| "Add a picture for this answer".to_owned()),
                if uploading() { "…" } else if upload_error().is_some() { "⚠" } else { "🖼" }
                input {
                    r#type: "file",
                    accept: "image/*",
                    class: "visually-hidden",
                    onchange: upload,
                }
            }
        },
    }
}

/// The pairs of a pairs question, each matched as it should be.
#[component]
fn PairsEditor(question: usize, pairs: Vec<Pair>) -> Element {
    let draft: Draft = use_context();
    let count = pairs.len();
    rsx! {
        ul { class: "answers-edit",
            for (i, pair) in pairs.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit pair-edit",
                    span { class: "order-rank", "{i + 1}" }
                    input {
                        class: "grow",
                        placeholder: "Left, e.g. France",
                        aria_label: "Left item {i + 1}",
                        value: "{pair.left.text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_pairs(question, |pairs| pairs[i].left.text = value);
                        },
                    }
                    PairImage { question, pair: i, right: false, image: pair.left.image.clone() }
                    span { class: "muted", aria_hidden: "true", "↔" }
                    input {
                        class: "grow",
                        placeholder: "Right, e.g. Paris",
                        aria_label: "Right item {i + 1}",
                        value: "{pair.right.text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_pairs(question, |pairs| pairs[i].right.text = value);
                        },
                    }
                    PairImage { question, pair: i, right: true, image: pair.right.image.clone() }
                    button {
                        class: "button ghost icon danger-icon",
                        title: "Remove this pair",
                        aria_label: "Remove this pair",
                        disabled: count <= MIN_ANSWERS,
                        onclick: move |_| draft.edit_pairs(question, |pairs| {
                            pairs.remove(i);
                        }),
                        Icon { kind: IconKind::Trash }
                    }
                }
            }
        }
        div { class: "row between",
            if count < MAX_ANSWERS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_pairs(question, |pairs| pairs.push(Pair::default())),
                    "＋ Add pair"
                }
            } else {
                span {}
            }
            span { class: "muted small", "Write each pair matched; phones shuffle the right column. Each side takes a text, a picture or both." }
        }
    }
}

/// One side of pair `pair`, if it still exists.
fn pair_side(pairs: &mut [Pair], pair: usize, right: bool) -> Option<&mut PairItem> {
    pairs
        .get_mut(pair)
        .map(|p| if right { &mut p.right } else { &mut p.left })
}

/// The picture of one side of a pair: upload, thumbnail and removal.
#[component]
fn PairImage(question: usize, pair: usize, right: bool, image: Option<String>) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        upload_error.set(None);
        match upload_question_image(token.get(), file.into()).await {
            // Pairs may have been removed while uploading: only set a pair that still exists.
            Ok(name) => draft.edit_pairs(question, |pairs| {
                if let Some(item) = pair_side(pairs, pair, right) {
                    item.image = Some(name);
                }
            }),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    match image {
        Some(image) => rsx! {
            img { class: "answer-thumb", src: media_url(&image), alt: "Picture for this item" }
            button {
                class: "button ghost icon",
                title: "Remove picture",
                onclick: move |_| draft.edit_pairs(question, |pairs| {
                    if let Some(item) = pair_side(pairs, pair, right) {
                        item.image = None;
                    }
                }),
                "✕🖼"
            }
        },
        None => rsx! {
            label {
                class: "button ghost icon",
                title: upload_error().unwrap_or_else(|| "Add a picture for this item".to_owned()),
                if uploading() { "…" } else if upload_error().is_some() { "⚠" } else { "🖼" }
                input {
                    r#type: "file",
                    accept: "image/*",
                    class: "visually-hidden",
                    onchange: upload,
                }
            }
        },
    }
}

/// The items of an order question, in the right order.
#[component]
fn OrderItems(question: usize, items: Vec<String>) -> Element {
    let draft: Draft = use_context();
    let count = items.len();
    rsx! {
        ol { class: "answers-edit",
            for (i, text) in items.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit",
                    span { class: "order-rank", "{i + 1}" }
                    input {
                        class: "grow",
                        placeholder: if i == 0 { "First item" } else { "Next item" },
                        value: "{text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_items(question, |items| items[i] = value);
                        },
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move up",
                        disabled: i == 0,
                        onclick: move |_| draft.edit_items(question, |items| items.swap(i, i - 1)),
                        "↑"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move down",
                        disabled: i + 1 == count,
                        onclick: move |_| draft.edit_items(question, |items| items.swap(i, i + 1)),
                        "↓"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Remove",
                        disabled: count <= MIN_ANSWERS,
                        onclick: move |_| draft.edit_items(question, |items| {
                            items.remove(i);
                        }),
                        "✕"
                    }
                }
            }
        }
        div { class: "row between",
            if count < MAX_ANSWERS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_items(question, |items| items.push(String::new())),
                    "＋ Add item"
                }
            } else {
                span {}
            }
            span { class: "muted small", "List the items in the right order; phones show them shuffled." }
        }
    }
}

/// The correct number of a closest-number question.
#[component]
fn NumberAnswer(question: usize, answer: Option<f64>, range: Option<NumberRange>) -> Element {
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

#[component]
fn AnswerRow(question: usize, index: usize, answer: Answer, removable: bool) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        upload_error.set(None);
        match upload_question_image(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(question, |q| q.answers[index].image = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    rsx! {
        li { class: if answer.correct { "answer-edit correct" } else { "answer-edit" },
            AnswerBadge { index }
            if let Some(image) = &answer.image {
                img { class: "answer-thumb", src: media_url(image), alt: "Answer picture" }
                button {
                    class: "button ghost icon",
                    title: "Remove picture",
                    onclick: move |_| draft.edit_question(question, |q| q.answers[index].image = None),
                    "✕🖼"
                }
            } else {
                label {
                    class: "button ghost icon",
                    title: upload_error().unwrap_or_else(|| "Add a picture to this answer".to_owned()),
                    if uploading() { "…" } else if upload_error().is_some() { "⚠" } else { "🖼" }
                    input {
                        r#type: "file",
                        accept: "image/*",
                        class: "visually-hidden",
                        onchange: upload,
                    }
                }
            }
            input {
                class: "grow",
                placeholder: if answer.image.is_some() { "Caption (optional)" } else { "Answer {index + 1}" },
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
