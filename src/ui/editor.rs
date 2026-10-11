//! The quiz editor.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    widgets::{
        AnswerBadge, ClientOnly, ErrorBox, Icon, IconKind, MapPin, PinMap, STAR, Spinner,
        media_url, message,
    },
};
use crate::{
    api::{
        get_quiz, save_quiz, start_test, upload_limits, upload_question_audio,
        upload_question_image, upload_question_media,
    },
    model::{
        AdvanceMode, Answer, BONUS_PERCENTS, Bonus, ImageReveal, MAX_ANSWERS, MAX_MULTIPLIER,
        MAX_PARTS, MAX_TIME_LIMIT_SECS, MIN_ANSWERS, MIN_PARTS, MIN_TIME_LIMIT_SECS, MultiScoring,
        Part, PhoneDisplay, PinTolerance, Point, PointsMode, Question, QuestionKind, Quiz,
        QuizSettings, RevealMode, ValidationIssue, is_video,
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
    let save_state = use_autosave(token, draft, issues);
    // The order before the last shuffle, and the revision the shuffle produced: undo is offered
    // until anything else changes.
    let mut before_shuffle = use_signal(|| None::<(u64, Vec<Question>)>);
    let shuffle = move |_| {
        let previous = draft.quiz.peek().questions.clone();
        draft.edit(|q| q.shuffle_questions(client::random_below));
        before_shuffle.set(Some((*draft.revision.peek(), previous)));
    };
    let undo_shuffle = move |_| {
        if let Some((_, previous)) = before_shuffle.take() {
            draft.edit(|q| q.questions = previous);
        }
    };
    let can_undo = before_shuffle
        .read()
        .as_ref()
        .is_some_and(|(revision, _)| *revision == (draft.revision)());

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
    rsx! {
        AdminHeader { back: true,
            span { class: "save-state",
                match save_state() {
                    SaveState::Saved => "All changes saved",
                    SaveState::Pending => "Saving…",
                    SaveState::Failed => "Not saved, retrying…",
                }
            }
            button {
                class: "button",
                disabled: !issues().is_empty(),
                title: if issues().is_empty() { "Try the quiz with the big screen and two phones side by side" } else { "Fix the quiz before testing it" },
                onclick: test,
                "🧪 Test"
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
            ErrorBox { error: test_error() }
            SettingsCard { settings: quiz.settings }
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
            div { class: "row",
                button {
                    class: "button primary add",
                    onclick: move |_| draft.edit(|q| q.questions.push(Question::blank())),
                    "＋ Add question"
                }
                button {
                    class: "button add-slide",
                    title: "A title, text or picture shown between questions",
                    onclick: move |_| draft.edit(|q| q.questions.push(Question::slide())),
                    "＋ Add slide"
                }
                if quiz.settings.reveal == RevealMode::AtEnd {
                    button {
                        class: "button add-slide",
                        title: "Reveal the answers so far and show the standings, e.g. at an intermission",
                        onclick: move |_| draft.edit(|q| q.questions.push(Question::results_slide())),
                        "＋ Add results slide"
                    }
                }
                span { class: "grow" }
                if can_undo {
                    button { class: "button ghost", onclick: undo_shuffle, "↶ Undo shuffle" }
                }
                button {
                    class: "button ghost shuffle",
                    disabled: !can_shuffle,
                    title: "Shuffle the order of the questions. Slides stay in place and questions never move past a slide.",
                    onclick: shuffle,
                    "🔀 Shuffle questions"
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
        let video = file.content_type().is_some_and(|t| t.starts_with("video/"));
        let error = if video {
            too_large(&file, limits.get().map(|l| l.video), "video")
        } else {
            too_large(&file, limits.get().map(|l| l.picture), "picture")
        };
        if let Some(error) = error {
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
            if !results_slide {
                textarea {
                    class: "question-text",
                    placeholder: if slide { "Text (optional)…" } else { "Type the question…" },
                    rows: 2,
                    value: "{question.text}",
                    oninput: move |e| draft.edit_question(index, |q| q.text = e.value()),
                }
                div { class: "question-media row",
                    if let Some(image) = &question.image {
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
                    } else {
                        label {
                            class: "button ghost",
                            title: match limits.get() {
                                Some(l) => format!(
                                    "Pictures up to {} MB; videos (MP4, WebM, MOV) up to {} MB",
                                    UploadLimits::megabytes(l.picture),
                                    UploadLimits::megabytes(l.video),
                                ),
                                None => "A picture, or a video (MP4, WebM, MOV)".to_owned(),
                            },
                            if uploading() { "Uploading…" } else { "＋ Add picture or video" }
                            input {
                                r#type: "file",
                                accept: "image/*,video/mp4,video/webm,video/quicktime",
                                class: "visually-hidden",
                                onchange: upload,
                            }
                        }
                    }
                    if !slide {
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
            }
            ErrorBox { error: upload_error() }
            if !slide {
                div { class: "question-media row",
                    if let Some(audio) = &question.audio {
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
                    } else {
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
                    ],
                    selected: match question.kind {
                        QuestionKind::Choice | QuestionKind::Slide { .. } => 0,
                        QuestionKind::TrueFalse { .. } => 1,
                        QuestionKind::Text { .. } => 2,
                        QuestionKind::Number { .. } => 3,
                        QuestionKind::Order { .. } => 4,
                        QuestionKind::Parts { .. } => 5,
                        QuestionKind::Pin { .. } => 6,
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
                    QuestionKind::Number { answer } => rsx! {
                        NumberAnswer { question: index, answer: *answer }
                    },
                    QuestionKind::Order { items } => rsx! {
                        OrderItems { question: index, items: items.clone() }
                    },
                    QuestionKind::Parts { parts } => rsx! {
                        PartsEditor { question: index, parts: parts.clone() }
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
            }
        }
    }
}

/// Switches a question to the answer type picked in the editor (0 choices, 1 true/false, 2 text,
/// 3 number, 4 order).
fn set_kind(question: &mut Question, choice: usize) {
    question.kind = match (choice, &question.kind) {
        (1, QuestionKind::TrueFalse { .. })
        | (2, QuestionKind::Text { .. })
        | (3, QuestionKind::Number { .. })
        | (4, QuestionKind::Order { .. })
        | (5, QuestionKind::Parts { .. })
        | (6, QuestionKind::Pin { .. }) => return,
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
        (3, _) => QuestionKind::Number { answer: None },
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
fn NumberAnswer(question: usize, answer: Option<f64>) -> Element {
    let draft: Draft = use_context();
    // Keep what is typed, so "3." or "-" survive until the number is complete.
    let mut raw = use_signal(|| answer.map(|a| a.to_string()).unwrap_or_default());
    rsx! {
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
                    let parsed = value
                        .trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite());
                    raw.set(value);
                    draft.edit_question(question, |q| q.kind = QuestionKind::Number { answer: parsed });
                },
            }
        }
    }
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
