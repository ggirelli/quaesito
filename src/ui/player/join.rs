//! Joining a game: the code, the team name and the ticket that lets a phone rejoin.

use dioxus::prelude::*;

use super::PlayerGame;
use crate::ui::{
    Route, client,
    widgets::{Brand, ClientOnly, ErrorBox, Spinner, message},
};
use crate::{
    api::{check_ticket, game_info, join_game},
    protocol::MAX_NAME_CHARS,
};

/// The digits of a typed game code (spaces and stray characters are dropped).
fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

fn ticket_key(code: &str) -> String {
    format!("quaesito.ticket.{code}")
}

/// `/join`: asks for a game code, or joins the game in `?code=`.
#[component]
pub fn JoinBlank(code: String) -> Element {
    let code = digits(&code);
    if !code.is_empty() {
        return rsx! {
            Join { code }
        };
    }
    rsx! {
        main { class: "page center narrow stack",
            Brand { large: true }
            JoinCodeForm {}
        }
    }
}

/// A form to enter a game code.
///
/// It also works as a plain HTML form (`GET /join?code=…`), because phones on a slow network can
/// show it, and get it submitted, before the app has loaded. For the same reason the field is not
/// bound to app state: loading the app would otherwise clear what was already typed.
#[component]
pub fn JoinCodeForm() -> Element {
    let nav = navigator();
    rsx! {
        form {
            class: "card stack",
            action: "/join",
            method: "get",
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let code = match evt.get_first("code") {
                    Some(FormValue::Text(text)) => digits(&text),
                    _ => String::new(),
                };
                if !code.is_empty() {
                    nav.push(Route::Join {
                        code,
                        ticket: String::new(),
                    });
                }
            },
            label { class: "field",
                span { "Game code" }
                input {
                    name: "code",
                    class: "code-input",
                    inputmode: "numeric",
                    pattern: "[0-9 ]*",
                    autocomplete: "off",
                    maxlength: 7,
                    placeholder: "000000",
                }
            }
            button { r#type: "submit", class: "button primary big", "Join" }
        }
    }
}

/// `/join/:code`: join (or rejoin) a game and play it. With a `ticket`, play as that team
/// without storing it, so several test phones can share one browser.
#[component]
pub fn Join(code: String, #[props(default)] ticket: String) -> Element {
    rsx! {
        ClientOnly {
            PlayerEntry { key: "{code}-{ticket}", code, ticket }
        }
    }
}

#[component]
fn PlayerEntry(code: String, ticket: String) -> Element {
    // A given ticket is used as is and never stored.
    let remember = ticket.is_empty();
    let code = use_signal(|| code);
    let mut token = use_signal(|| {
        if remember {
            client::storage_get(&ticket_key(&code()))
        } else {
            Some(ticket)
        }
    });
    let mut notice = use_signal(|| None::<&'static str>);
    // A stored ticket is only good while its game is running.
    let checked = use_resource(move || async move {
        match token() {
            Some(t) => check_ticket(code(), t).await.unwrap_or(false),
            None => false,
        }
    });
    use_effect(move || {
        if checked() == Some(false) && token.peek().is_some() {
            if remember {
                client::storage_remove(&ticket_key(&code.peek()));
            }
            token.set(None);
        }
    });
    match (token(), checked()) {
        (Some(t), Some(true)) => rsx! {
            PlayerGame {
                code: code(),
                token: t,
                on_gone: move |()| {
                    if remember {
                        client::storage_remove(&ticket_key(&code.peek()));
                    }
                    notice.set(Some("Your team was removed from the game. You can join again."));
                    token.set(None);
                },
            }
        },
        (Some(_), _) => rsx! {
            div { class: "page center", Spinner {} }
        },
        (None, _) => rsx! {
            JoinForm { code: code(), notice: notice(), on_joined: move |t: String| {
                if remember {
                    client::storage_set(&ticket_key(&code()), &t);
                }
                notice.set(None);
                token.set(Some(t));
            } }
        },
    }
}

#[component]
fn JoinForm(
    code: String,
    notice: Option<&'static str>,
    on_joined: EventHandler<String>,
) -> Element {
    let code = use_signal(|| code);
    let info = use_resource(move || async move { game_info(code()).await });
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let title = match info() {
        None => {
            return rsx! {
                div { class: "page center", Spinner {} }
            };
        }
        Some(Ok(Some(title))) => title,
        Some(Ok(None)) => {
            return rsx! {
                main { class: "page center narrow stack",
                    Brand { large: true }
                    p { class: "error", "No game with code {code} is running. Check the code on the big screen." }
                    JoinCodeForm {}
                }
            };
        }
        Some(Err(err)) => {
            return rsx! {
                main { class: "page center narrow stack",
                    Brand { large: true }
                    ErrorBox { error: Some(message(&err)) }
                }
            };
        }
    };

    rsx! {
        main { class: "page center narrow stack",
            Brand { large: true }
            if let Some(notice) = notice {
                p { class: "issues", "{notice}" }
            }
            p { class: "tagline", "Joining “{title}”" }
            form {
                class: "card stack",
                onsubmit: move |evt: FormEvent| async move {
                    evt.prevent_default();
                    busy.set(true);
                    match join_game(code(), name()).await {
                        Ok(ticket) => on_joined.call(ticket.token),
                        Err(err) => error.set(Some(message(&err))),
                    }
                    busy.set(false);
                },
                label { class: "field",
                    span { "Team name" }
                    input {
                        autofocus: true,
                        autocomplete: "off",
                        maxlength: MAX_NAME_CHARS,
                        placeholder: "The Quizzly Bears",
                        value: name,
                        oninput: move |e| name.set(e.value()),
                    }
                }
                ErrorBox { error: error() }
                button {
                    r#type: "submit",
                    class: "button primary big",
                    disabled: busy() || name().trim().is_empty(),
                    "Let's go"
                }
            }
        }
    }
}
