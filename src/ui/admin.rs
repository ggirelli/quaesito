//! Admin login and dashboard.

use dioxus::prelude::*;

use super::{
    Route, client,
    library::{RandomQuiz, RandomQuizFields, use_library},
    transfer::{self, IMPORT_INPUT, TransferStatus, Transfers},
    widgets::{
        Brand, ClientOnly, Dialog, ErrorBox, HelpLink, Icon, IconKind, Spinner, ThemeToggle,
        message,
    },
};
use crate::api::{
    admin_check, admin_login, create_quiz, delete_quiz, duplicate_quiz, end_session, get_quiz,
    list_quizzes, list_sessions, save_quiz, start_session, start_test,
};
use crate::library::LibraryEntry;
use crate::model::{ImportOutcome, ImportWarning, Question, QuizSummary};
use crate::protocol::{DEFAULT_MAX_TEAMS, MAX_TEAMS_CEILING};
use crate::search;

const TOKEN_KEY: &str = "quaesito.admin";

/// The admin token, provided by [`AdminGate`]. Always `Some` below the gate.
#[derive(Clone, Copy)]
pub struct AdminToken(pub Signal<Option<String>>);

impl AdminToken {
    /// The current token (empty if logged out).
    #[must_use]
    pub fn get(self) -> String {
        self.0().unwrap_or_default()
    }

    /// Forgets the token, returning to the login form.
    pub fn clear(mut self) {
        client::storage_remove(TOKEN_KEY);
        self.0.set(None);
    }
}

#[component]
pub fn Admin() -> Element {
    rsx! {
        ClientOnly {
            AdminGate { Dashboard {} }
        }
    }
}

/// Shows the login form unless a valid admin token is stored.
#[component]
pub fn AdminGate(children: Element) -> Element {
    let token = use_signal(|| client::storage_get(TOKEN_KEY));
    use_context_provider(|| AdminToken(token));
    // Tokens do not survive server restarts, so check before trusting a stored one.
    let valid = use_resource(move || async move {
        match token() {
            Some(t) => admin_check(t).await.unwrap_or(false),
            None => false,
        }
    });
    match (token().is_some(), valid()) {
        (true, None) => rsx! {
            div { class: "page center", Spinner {} }
        },
        (true, Some(true)) => children,
        _ => rsx! {
            LoginForm {}
        },
    }
}

#[component]
fn LoginForm() -> Element {
    let AdminToken(mut token) = use_context();
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        nav { class: "corner-links",
            ThemeToggle { class: "corner-link" }
            HelpLink { class: "corner-link" }
            Link {
                class: "corner-link",
                to: Route::Home {},
                title: "Home page",
                aria_label: "Home page",
                Icon { kind: IconKind::Home }
            }
        }
        main { class: "page center narrow stack",
            Brand { large: true }
            form {
                class: "card stack",
                onsubmit: move |evt: FormEvent| async move {
                    evt.prevent_default();
                    match admin_login(password()).await {
                        Ok(t) => {
                            client::storage_set(TOKEN_KEY, &t);
                            token.set(Some(t));
                        }
                        Err(err) => error.set(Some(message(&err))),
                    }
                },
                h1 { "Admin" }
                label { class: "field",
                    span { "Password" }
                    input {
                        r#type: "password",
                        autofocus: true,
                        value: password,
                        oninput: move |e| password.set(e.value()),
                    }
                }
                ErrorBox { error: error() }
                button { r#type: "submit", class: "button primary", "Log in" }
            }
            p { class: "muted small",
                "The password is set with QUAESITO_ADMIN_PASSWORD, or printed in the server log at startup."
            }
        }
    }
}

/// Top bar of admin pages. With `back`, it links back to the quiz list.
#[component]
pub fn AdminHeader(#[props(default)] back: bool, children: Element) -> Element {
    let token: AdminToken = use_context();
    rsx! {
        header { class: "topbar",
            div { class: "row",
                if back {
                    Link { class: "button ghost", to: Route::Admin {},
                        Icon { kind: IconKind::Back }
                        "Quizzes"
                    }
                }
                Link { to: Route::Admin {}, Brand {} }
            }
            div { class: "row", {children} }
            div { class: "row",
                ThemeToggle { class: "button ghost icon" }
                HelpLink { class: "button ghost", label: true }
                Link {
                    class: "button ghost",
                    to: Route::Home {},
                    title: "Home page",
                    Icon { kind: IconKind::Home }
                    "Home"
                }
                button { class: "button ghost", onclick: move |_| token.clear(), "Log out" }
            }
        }
    }
}

#[component]
fn Dashboard() -> Element {
    let token: AdminToken = use_context();
    let mut error = use_signal(|| None::<String>);
    let mut quizzes = use_resource(move || async move { list_quizzes(token.get()).await });
    let mut creating = use_signal(|| false);
    let mut search = use_signal(String::new);
    let mut sort = use_signal(|| QuizSort::Edited);

    let transfers = use_context_provider(|| Transfers(Signal::new(None)));
    let busy = move || (transfers.0)().is_some();
    // The name of a file from a newer version, waiting for the admin to import it anyway; the
    // file stays chosen in the import input meanwhile.
    let mut unsure = use_signal(|| None::<(String, ImportWarning)>);

    let send = move |name: String, consent: bool| async move {
        error.set(None);
        let result = transfer::import(token.get(), consent, &name, transfers).await;
        match result {
            Ok(ImportOutcome::NeedsConsent(warning)) => {
                unsure.set(Some((name, warning)));
                return;
            }
            Ok(ImportOutcome::Imported(_)) => quizzes.restart(),
            Err(err) => error.set(Some(err)),
        }
        transfer::clear_import_input();
    };
    let import = move |evt: FormEvent| async move {
        if let Some(file) = evt.files().into_iter().next() {
            send(file.name(), false).await;
        }
    };
    let import_anyway = move |()| async move {
        if let Some((name, _)) = unsure.take() {
            send(name, true).await;
        }
    };
    let cancel_import = move |()| {
        unsure.set(None);
        transfer::clear_import_input();
    };

    rsx! {
        AdminHeader {}
        main { class: "page wide stack",
            ErrorBox { error: error() }
            LiveGames { on_error: move |e| error.set(Some(e)) }
            section { class: "card stack",
                div { class: "row between",
                    h2 { "Quizzes" }
                    div { class: "row",
                        button {
                            class: "button primary",
                            title: "Create a quiz",
                            onclick: move |_| creating.set(true),
                            Icon { kind: IconKind::NewFile }
                            "New quiz"
                        }
                        label {
                            class: "button ghost icon",
                            class: if busy() { "disabled" },
                            title: "Import a quiz (.zip or .json)",
                            aria_label: "Import a quiz",
                            Icon { kind: IconKind::Import }
                            input {
                                id: IMPORT_INPUT,
                                r#type: "file",
                                accept: ".zip,.json,application/zip,application/json",
                                class: "visually-hidden",
                                disabled: busy(),
                                onchange: import,
                            }
                        }
                    }
                }
                if creating() {
                    CreateDialog { on_close: move |()| creating.set(false) }
                }
                if let Some((_, warning)) = unsure() {
                    ImportConsentDialog { warning, on_import: import_anyway, on_close: cancel_import }
                }
                TransferStatus {}
                div { class: "row quiz-filters",
                    label { class: "search grow",
                        Icon { kind: IconKind::Search }
                        input {
                            r#type: "search",
                            placeholder: "Search quizzes",
                            aria_label: "Search quizzes",
                            value: search,
                            oninput: move |e| search.set(e.value()),
                        }
                    }
                    label { class: "field inline",
                        span { "Sort by" }
                        select {
                            value: sort().key(),
                            onchange: move |e| sort.set(QuizSort::from_key(&e.value())),
                            for option in QuizSort::ALL {
                                option { value: option.key(), "{option.label()}" }
                            }
                        }
                    }
                }
                match quizzes() {
                    None => rsx! {
                        Spinner {}
                    },
                    Some(Err(err)) => rsx! {
                        ErrorBox { error: Some(message(&err)) }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        p { class: "muted", "No quizzes yet. Create your first one with New quiz." }
                    },
                    Some(Ok(list)) => {
                        let shown = sorted_matches(&list, &search(), sort());
                        let now = client::now_ms() / 1000.0;
                        rsx! {
                            if shown.is_empty() {
                                p { class: "muted", "No quiz matches “{search}”." }
                            }
                            ul { class: "list",
                                for quiz in shown {
                                    li { key: "{quiz.id}", class: "list-row",
                                        div { class: "grow",
                                            Link {
                                                class: "strong",
                                                to: Route::Editor {
                                                    id: quiz.id.clone(),
                                                },
                                                "{quiz.title}"
                                            }
                                            div { class: "muted small",
                                                if quiz.question_count == 1 {
                                                    "1 question"
                                                } else {
                                                    "{quiz.question_count} questions"
                                                }
                                                if let Some(edited) = quiz.edited {
                                                    " · edited {ago(now, edited)}"
                                                }
                                            }
                                            NotPlayable { issues: quiz.issues.clone() }
                                        }
                                        QuizActions {
                                            id: quiz.id.clone(),
                                            title: quiz.title.clone(),
                                            playable: quiz.issues.is_empty(),
                                            on_change: move |()| quizzes.restart(),
                                            on_error: move |e| error.set(Some(e)),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// How the quiz list is ordered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum QuizSort {
    Name,
    Edited,
    Created,
    Questions,
}

impl QuizSort {
    const ALL: [Self; 4] = [Self::Edited, Self::Created, Self::Name, Self::Questions];

    fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Edited => "edited",
            Self::Created => "created",
            Self::Questions => "questions",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Edited => "Last edited",
            Self::Created => "Newest",
            Self::Questions => "Most questions",
        }
    }

    fn from_key(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.key() == key)
            .unwrap_or(Self::Edited)
    }
}

/// The quizzes whose title contains every word of `search` (ignoring case and accents), in
/// `sort` order; ties go by name.
fn sorted_matches(list: &[QuizSummary], search: &str, sort: QuizSort) -> Vec<QuizSummary> {
    let words = search::words(search);
    let mut shown: Vec<QuizSummary> = list
        .iter()
        .filter(|q| search::contains_all(&q.title, &words))
        .cloned()
        .collect();
    shown.sort_by(|a, b| {
        let by_name = search::fold(&a.title).cmp(&search::fold(&b.title));
        match sort {
            QuizSort::Name => by_name,
            QuizSort::Edited => b.edited.cmp(&a.edited).then(by_name),
            QuizSort::Created => b.created.cmp(&a.created).then(by_name),
            QuizSort::Questions => b.question_count.cmp(&a.question_count).then(by_name),
        }
    });
    shown
}

/// How long ago `then` was, from `now` (both in seconds since 1970): "just now", "5 min ago"…
fn ago(now: f64, then: u64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "times are far below 2^52 seconds"
    )]
    let secs = (now - then as f64).max(0.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded, non-negative"
    )]
    let (minutes, hours, days) = (
        (secs / 60.0) as u64,
        (secs / 3600.0) as u64,
        (secs / 86400.0) as u64,
    );
    match (minutes, hours, days) {
        (0, ..) => "just now".to_owned(),
        (m, 0, _) => format!("{m} min ago"),
        (_, 1, 0) => "an hour ago".to_owned(),
        (_, h, 0) => format!("{h} hours ago"),
        (.., 1) => "yesterday".to_owned(),
        (.., d) if d < 60 => format!("{d} days ago"),
        (.., d) => format!("{} months ago", d / 30),
    }
}

/// Asks for the new quiz's title, creates it and opens it in the editor. The quiz starts empty,
/// or with questions picked at random from the library.
#[component]
fn CreateDialog(on_close: EventHandler<()>) -> Element {
    let token: AdminToken = use_context();
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut from_library = use_signal(|| false);
    let library = use_library(move || *from_library.read());
    let mut entries = use_signal(Vec::<LibraryEntry>::new);
    use_effect(move || {
        if let Some(Some(Ok(loaded))) = &*library.read() {
            entries.set(loaded.clone());
        }
    });
    let choice = use_signal(RandomQuiz::default);
    let create = move |evt: FormEvent| async move {
        evt.prevent_default();
        if title().trim().is_empty() {
            error.set(Some("Give the new quiz a title.".to_owned()));
            return;
        }
        let questions = if from_library() {
            let picked = choice.peek().pick(&entries.peek());
            if picked.iter().all(Question::is_slide) {
                error.set(Some("Pick at least one category.".to_owned()));
                return;
            }
            picked
        } else {
            Vec::new()
        };
        let result = async {
            let id = create_quiz(token.get(), title()).await?;
            if !questions.is_empty() {
                let mut quiz = get_quiz(token.get(), id.clone()).await?;
                quiz.questions = questions;
                save_quiz(token.get(), quiz).await?;
            }
            Ok::<_, ServerFnError>(id)
        };
        match result.await {
            Ok(id) => {
                navigator().push(Route::Editor { id });
            }
            Err(err) => error.set(Some(message(&err))),
        }
    };
    rsx! {
        Dialog { title: "New quiz", on_close,
            form { class: "stack", onsubmit: create,
                label { class: "field",
                    span { "Title" }
                    input {
                        placeholder: "e.g. Movie night",
                        autofocus: true,
                        value: title,
                        oninput: move |e| title.set(e.value()),
                        onmounted: move |e| async move {
                            let _ = e.set_focus(true).await;
                        },
                    }
                }
                label { class: "check-field",
                    input {
                        r#type: "checkbox",
                        checked: from_library(),
                        onchange: move |e| from_library.set(e.checked()),
                    }
                    span { "Fill it with random questions from the library" }
                }
                if from_library() {
                    match &*library.read() {
                        Some(Some(Err(err))) => rsx! {
                            ErrorBox { error: Some(message(err)) }
                        },
                        Some(Some(Ok(_))) => rsx! {
                            RandomQuizFields { entries, choice }
                        },
                        _ => rsx! {
                            Spinner {}
                        },
                    }
                }
                ErrorBox { error: error() }
                button { r#type: "submit", class: "button primary big", "Create quiz" }
            }
        }
    }
}

/// Asks before importing a quiz from a newer version, saying what would be left out.
#[component]
fn ImportConsentDialog(
    warning: ImportWarning,
    on_import: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let unknown = warning.unknown;
    rsx! {
        Dialog { title: "Import “{warning.title}”?", on_close,
            // Plain flow, so the paragraphs' margins collapse.
            div {
                p {
                    "This quiz was made with a newer version of Quaesito, and this one cannot read all of it."
                }
                if unknown.is_empty() {
                    p { "Some of its content may be lost." }
                } else {
                    p {
                        "These questions are of kinds this version does not know, and would be left out:"
                    }
                    ul {
                        for question in unknown.iter() {
                            li {
                                strong { "{question.place}" }
                                if !question.text.is_empty() {
                                    " “{question.text}”"
                                }
                                span { class: "muted", " ({question.kind})" }
                            }
                        }
                    }
                }
                p { class: "muted small",
                    "Update Quaesito to import everything, or import what this version can read."
                }
            }
            div { class: "row",
                button {
                    class: "button primary",
                    onclick: move |_| on_import.call(()),
                    if unknown.is_empty() {
                        "Import anyway"
                    } else {
                        "Import without them"
                    }
                }
                button {
                    class: "button ghost",
                    onclick: move |_| on_close.call(()),
                    "Cancel"
                }
            }
        }
    }
}

#[component]
fn StartDialog(id: String, on_close: EventHandler<()>) -> Element {
    let token: AdminToken = use_context();
    let mut max_teams = use_signal(|| DEFAULT_MAX_TEAMS.to_string());
    let mut shuffle_answers = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let start = move |evt: FormEvent| {
        evt.prevent_default();
        let id = id.clone();
        async move {
            let Ok(limit) = max_teams().trim().parse::<usize>() else {
                error.set(Some(format!(
                    "Enter a number from 1 to {MAX_TEAMS_CEILING}."
                )));
                return;
            };
            match start_session(token.get(), id, Some(limit), Some(shuffle_answers())).await {
                Ok(code) => {
                    navigator().push(Route::Host { code });
                }
                Err(err) => error.set(Some(message(&err))),
            }
        }
    };
    rsx! {
        Dialog { title: "Start a live game", on_close,
            form { class: "stack", onsubmit: start,
                label { class: "field",
                    span { "Most teams" }
                    input {
                        r#type: "number",
                        min: 1,
                        max: MAX_TEAMS_CEILING,
                        value: max_teams,
                        oninput: move |e| max_teams.set(e.value()),
                    }
                }
                p { class: "muted small",
                    "Each team plays from one phone, with as many players around it as you like. "
                    "{DEFAULT_MAX_TEAMS} suits most game nights; raise it for a big event, up to 10,000. "
                    "Your Wi-Fi may hold fewer phones than that."
                }
                label { class: "check-field",
                    input {
                        r#type: "checkbox",
                        checked: shuffle_answers(),
                        onchange: move |e| shuffle_answers.set(e.checked()),
                    }
                    span { "Shuffle the answers" }
                }
                p { class: "muted small",
                    "The big screen shows each multiple-choice question's answers in a new order, "
                    "for this game only: the saved quiz keeps its order. Phones always list the "
                    "answers in their own order."
                }
                ErrorBox { error: error() }
                button { r#type: "submit", class: "button primary big", "▶ Start" }
            }
        }
    }
}

#[component]
fn QuizActions(
    id: String,
    title: String,
    playable: bool,
    on_change: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let token: AdminToken = use_context();
    let transfers: Transfers = use_context();
    let export = {
        let id = id.clone();
        move |_| {
            let (id, title) = (id.clone(), title.clone());
            async move {
                if let Err(err) = transfer::export(token.get(), id, &title, transfers).await {
                    on_error.call(err);
                }
            }
        }
    };
    let duplicate = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            async move {
                match duplicate_quiz(token.get(), id).await {
                    Ok(_) => on_change.call(()),
                    Err(err) => on_error.call(message(&err)),
                }
            }
        }
    };
    let delete = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            async move {
                match delete_quiz(token.get(), id).await {
                    Ok(()) => on_change.call(()),
                    Err(err) => on_error.call(message(&err)),
                }
            }
        }
    };
    let mut starting = use_signal(|| false);
    let test = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            async move {
                match start_test(token.get(), id, None).await {
                    Ok(code) => {
                        navigator().push(Route::Test { code });
                    }
                    Err(err) => on_error.call(message(&err)),
                }
            }
        }
    };
    let mut confirming = use_signal(|| false);
    rsx! {
        div { class: "row actions",
            button {
                class: "button primary",
                disabled: !playable,
                title: if playable { "Start a live game" } else { "Fix the quiz before playing it" },
                onclick: move |_| starting.set(true),
                "▶ Start"
            }
            if starting() {
                StartDialog { id: id.clone(), on_close: move |()| starting.set(false) }
            }
            button {
                class: "button",
                disabled: !playable,
                title: if playable { "Try the quiz with the big screen and two phones side by side" } else { "Fix the quiz before testing it" },
                onclick: test,
                "🧪 Test"
            }
            Link {
                class: "button icon",
                to: Route::Editor { id: id.clone() },
                title: "Edit",
                aria_label: "Edit",
                Icon { kind: IconKind::Edit }
            }
            button {
                class: "button ghost icon",
                title: "Duplicate",
                aria_label: "Duplicate",
                onclick: duplicate,
                Icon { kind: IconKind::Copy }
            }
            button {
                class: "button ghost icon",
                disabled: (transfers.0)().is_some(),
                onclick: export,
                title: "Export (.zip with pictures and soundtracks)",
                aria_label: "Export",
                Icon { kind: IconKind::Export }
            }
            if confirming() {
                button { class: "button danger", onclick: delete, "Really delete?" }
                button {
                    class: "button ghost",
                    onclick: move |_| confirming.set(false),
                    "Keep"
                }
            } else {
                button {
                    class: "button ghost icon danger-icon",
                    title: "Delete",
                    aria_label: "Delete",
                    onclick: move |_| confirming.set(true),
                    Icon { kind: IconKind::Trash }
                }
            }
        }
    }
}

/// Warning under a quiz that cannot be played yet, listing what to fix.
#[component]
fn NotPlayable(issues: Vec<String>) -> Element {
    let Some(first) = issues.first() else {
        return rsx! {};
    };
    let more = match issues.len() - 1 {
        0 => String::new(),
        1 => " (and 1 more)".to_owned(),
        n => format!(" (and {n} more)"),
    };
    rsx! {
        div { class: "not-playable small", title: issues.join("\n"),
            "⚠ Not ready to play: {first}{more}"
        }
    }
}

/// Sessions currently running, with links to their host screens.
#[component]
fn LiveGames(on_error: EventHandler<String>) -> Element {
    let token: AdminToken = use_context();
    let mut sessions = use_resource(move || async move { list_sessions(token.get()).await });
    let list = match sessions() {
        Some(Ok(list)) if !list.is_empty() => list,
        _ => return rsx! {},
    };
    rsx! {
        section { class: "card stack",
            h2 { "Live games" }
            ul { class: "list",
                for session in list {
                    li { key: "{session.code}", class: "list-row",
                        span { class: "code", "{session.code}" }
                        div { class: "grow",
                            div { class: "strong", "{session.title}" }
                            div { class: "muted small",
                                if session.test {
                                    "🧪 test · "
                                }
                                "{session.teams} teams"
                                if session.finished {
                                    " · finished"
                                }
                            }
                        }
                        div { class: "row actions",
                            if session.test {
                                Link {
                                    class: "button primary",
                                    to: Route::Test {
                                        code: session.code.clone(),
                                    },
                                    "Open test"
                                }
                            } else {
                                Link {
                                    class: "button primary",
                                    to: Route::Host {
                                        code: session.code.clone(),
                                    },
                                    "Open big screen"
                                }
                            }
                            button {
                                class: "button ghost",
                                onclick: {
                                    let code = session.code.clone();
                                    move |_| {
                                        let code = code.clone();
                                        async move {
                                            match end_session(token.get(), code).await {
                                                Ok(()) => sessions.restart(),
                                                Err(err) => on_error.call(message(&err)),
                                            }
                                        }
                                    }
                                },
                                "End"
                            }
                        }
                    }
                }
            }
        }
    }
}
