//! The quiz editor.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    library::LibraryDialog,
    widgets::{
        AnswerBadge, ClientOnly, Dialog, ErrorBox, Icon, IconKind, MapPin, PinMap, STAR, Spinner,
        media_url, message,
    },
};
use crate::{
    api::{
        edit_library_question, get_quiz, library_subjects, revert_library_question, save_quiz,
        start_test, upload_limits, upload_question_audio, upload_question_image,
        upload_question_media,
    },
    library::{LibraryCategory, LibraryEdit, LibraryEntry},
    model::{
        AdvanceMode, Answer, BONUS_PERCENTS, Bonus, ImageReveal, MAX_ANSWERS, MAX_MULTIPLIER,
        MAX_PARTS, MAX_TIME_LIMIT_SECS, MIN_ANSWERS, MIN_PARTS, MIN_TIME_LIMIT_SECS, MultiScoring,
        NumberRange, Pair, PairItem, Part, PhoneDisplay, PinTolerance, Point, PointsMode, Question,
        QuestionKind, Quiz, QuizSettings, RevealMode, ValidationIssue, is_video,
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
                SettingsCard { settings: quiz.settings }
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
            if browsing() {
                LibraryDialog {
                    present: quiz.questions.iter().map(|q| q.text.clone()).collect::<Vec<_>>(),
                    on_add: move |questions: Vec<Question>| draft.edit(|q| q.questions.extend(questions)),
                    on_close: move |()| browsing.set(false),
                }
            }
            aside { class: "editor-side editor-shuffle",
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

/// Keeps a moved question or slide in view: once the cards are redrawn, scrolls so the card at `to`
/// sits where the one at `from` was on screen, and focuses the same move button on it (or its
/// first enabled one, when that button is now disabled at the top or bottom).
fn follow_move(from: usize, to: usize, button: &str) {
    document::eval(&format!(
        r#"const top = document.getElementById("question-{from}")?.getBoundingClientRect().top ?? 0;
        requestAnimationFrame(() => requestAnimationFrame(() => {{
            const card = document.getElementById("question-{to}");
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
            id: "question-{index}",
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
