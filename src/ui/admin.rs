//! Admin login and dashboard.

use dioxus::prelude::*;

use super::{
    Route, client,
    widgets::{Brand, ClientOnly, ErrorBox, Icon, IconKind, Spinner, message},
};
use crate::api::{
    admin_check, admin_login, create_quiz, delete_quiz, duplicate_quiz, end_session, import_quiz,
    list_quizzes, list_sessions, start_session, start_test,
};

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
        _ => rsx! { LoginForm {} },
    }
}

#[component]
fn LoginForm() -> Element {
    let AdminToken(mut token) = use_context();
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    rsx! {
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
                Link { class: "button ghost", to: Route::Home {}, title: "Home page",
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
    let mut new_title = use_signal(String::new);
    let nav = navigator();

    let create = move |evt: FormEvent| async move {
        evt.prevent_default();
        let title = new_title();
        if title.trim().is_empty() {
            error.set(Some("Give the new quiz a title.".to_owned()));
            return;
        }
        match create_quiz(token.get(), title).await {
            Ok(id) => {
                nav.push(Route::Editor { id });
            }
            Err(err) => error.set(Some(message(&err))),
        }
    };

    let import = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        let result = import_quiz(token.get(), file.into())
            .await
            .map_err(|e| message(&e));
        match result {
            Ok(_) => quizzes.restart(),
            Err(err) => error.set(Some(err)),
        }
    };

    rsx! {
        AdminHeader {}
        main { class: "page wide stack",
            ErrorBox { error: error() }
            LiveGames { on_error: move |e| error.set(Some(e)) }
            section { class: "card stack",
                div { class: "row between",
                    h2 { "Quizzes" }
                    label { class: "button ghost",
                        "Import…"
                        input {
                            r#type: "file",
                            accept: ".zip,.json,application/zip,application/json",
                            class: "visually-hidden",
                            onchange: import,
                        }
                    }
                }
                form { class: "row", onsubmit: create,
                    input {
                        class: "grow",
                        placeholder: "Title of a new quiz",
                        value: new_title,
                        oninput: move |e| new_title.set(e.value()),
                    }
                    button { r#type: "submit", class: "button primary", "Create quiz" }
                }
                match quizzes() {
                    None => rsx! { Spinner {} },
                    Some(Err(err)) => rsx! { ErrorBox { error: Some(message(&err)) } },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        p { class: "muted", "No quizzes yet. Create your first one above." }
                    },
                    Some(Ok(list)) => rsx! {
                        ul { class: "list",
                            for quiz in list {
                                li { key: "{quiz.id}", class: "list-row",
                                    div { class: "grow",
                                        Link {
                                            class: "strong",
                                            to: Route::Editor { id: quiz.id.clone() },
                                            "{quiz.title}"
                                        }
                                        div { class: "muted small",
                                            if quiz.question_count == 1 {
                                                "1 question"
                                            } else {
                                                "{quiz.question_count} questions"
                                            }
                                        }
                                        NotPlayable { issues: quiz.issues.clone() }
                                    }
                                    QuizActions {
                                        id: quiz.id.clone(),
                                        playable: quiz.issues.is_empty(),
                                        on_change: move |()| quizzes.restart(),
                                        on_error: move |e| error.set(Some(e)),
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn QuizActions(
    id: String,
    playable: bool,
    on_change: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let token: AdminToken = use_context();
    let export_href = format!("/export/{id}?token={}", token.get());
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
    let start = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            async move {
                match start_session(token.get(), id).await {
                    Ok(code) => {
                        navigator().push(Route::Host { code });
                    }
                    Err(err) => on_error.call(message(&err)),
                }
            }
        }
    };
    let test = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            async move {
                match start_test(token.get(), id).await {
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
                onclick: start,
                "▶ Start"
            }
            button {
                class: "button",
                disabled: !playable,
                title: if playable { "Try the quiz with the big screen and two phones side by side" } else { "Fix the quiz before testing it" },
                onclick: test,
                "🧪 Test"
            }
            Link { class: "button", to: Route::Editor { id: id.clone() }, "Edit" }
            button { class: "button ghost", onclick: duplicate, "Duplicate" }
            a { class: "button ghost", href: export_href, download: true, "Export" }
            if confirming() {
                button { class: "button danger", onclick: delete, "Really delete?" }
                button { class: "button ghost", onclick: move |_| confirming.set(false), "Keep" }
            } else {
                button { class: "button ghost", onclick: move |_| confirming.set(true), "Delete" }
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
                                if session.test { "🧪 test · " }
                                "{session.teams} teams"
                                if session.finished { " · finished" }
                            }
                        }
                        div { class: "row actions",
                            if session.test {
                                Link {
                                    class: "button primary",
                                    to: Route::Test { code: session.code.clone() },
                                    "Open test"
                                }
                            } else {
                                Link {
                                    class: "button primary",
                                    to: Route::Host { code: session.code.clone() },
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
