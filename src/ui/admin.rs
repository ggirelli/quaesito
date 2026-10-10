//! Admin login and dashboard.

use dioxus::prelude::*;

use super::{
    Route, client,
    widgets::{Brand, ClientOnly, ErrorBox, Spinner, message},
};
use crate::api::{
    admin_check, admin_login, create_quiz, delete_quiz, duplicate_quiz, import_quiz, list_quizzes,
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

/// Top bar of admin pages.
#[component]
pub fn AdminHeader(children: Element) -> Element {
    let token: AdminToken = use_context();
    rsx! {
        header { class: "topbar",
            Link { to: Route::Admin {}, Brand {} }
            div { class: "row", {children} }
            button { class: "button ghost", onclick: move |_| token.clear(), "Log out" }
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
        let result = match file.read_string().await {
            Ok(json) => import_quiz(token.get(), json)
                .await
                .map_err(|e| message(&e)),
            Err(err) => Err(err.to_string()),
        };
        match result {
            Ok(_) => quizzes.restart(),
            Err(err) => error.set(Some(err)),
        }
    };

    rsx! {
        AdminHeader {}
        main { class: "page wide stack",
            ErrorBox { error: error() }
            section { class: "card stack",
                div { class: "row between",
                    h2 { "Quizzes" }
                    label { class: "button ghost",
                        "Import JSON…"
                        input {
                            r#type: "file",
                            accept: "application/json,.json",
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
                                    }
                                    QuizActions {
                                        id: quiz.id.clone(),
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
fn QuizActions(id: String, on_change: EventHandler<()>, on_error: EventHandler<String>) -> Element {
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
    let mut confirming = use_signal(|| false);
    rsx! {
        div { class: "row actions",
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
