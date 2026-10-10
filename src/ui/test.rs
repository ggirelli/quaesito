//! The admin's test page: the big screen and a few phones side by side, to try a quiz quickly.
//!
//! Each panel is the real page in a frame (`/host/<code>`, `/join/<code>?ticket=…`), so the test
//! shows exactly what players see, each at its own screen size.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    widgets::{ClientOnly, ErrorBox, Spinner, message},
};
use crate::api::{add_test_phone, end_session, remove_test_phone, start_test, test_info};

/// `/admin/test/:code`.
#[component]
pub fn Test(code: String) -> Element {
    rsx! {
        ClientOnly {
            AdminGate {
                TestPage { key: "{code}", code }
            }
        }
    }
}

#[component]
fn TestPage(code: String) -> Element {
    let token: AdminToken = use_context();
    let code = use_signal(|| code);
    let mut error = use_signal(|| None::<String>);
    let mut info = use_resource(move || async move { test_info(token.get(), code()).await });
    let nav = navigator();

    let loaded = info.read().clone();
    let info_now = match loaded {
        None => {
            return rsx! {
                AdminHeader { back: true }
                div { class: "page center", Spinner {} }
            };
        }
        Some(Err(err)) => {
            return rsx! {
                AdminHeader { back: true }
                main { class: "page narrow stack",
                    ErrorBox { error: Some(message(&err)) }
                    Link { class: "button", to: Route::Admin {}, "Back to quizzes" }
                }
            };
        }
        Some(Ok(info)) => info,
    };

    let quiz_id = info_now.quiz_id.clone();
    let restart = move |_| {
        let quiz_id = quiz_id.clone();
        async move {
            let _ = end_session(token.get(), code()).await;
            match start_test(token.get(), quiz_id).await {
                Ok(code) => {
                    nav.replace(Route::Test { code });
                }
                Err(err) => error.set(Some(message(&err))),
            }
        }
    };
    let end = move |_| async move {
        let _ = end_session(token.get(), code()).await;
        if nav.can_go_back() {
            nav.go_back();
        } else {
            nav.push(Route::Admin {});
        }
    };
    let add_phone = move |_| async move {
        match add_test_phone(token.get(), code()).await {
            Ok(()) => info.restart(),
            Err(err) => error.set(Some(message(&err))),
        }
    };

    rsx! {
        AdminHeader { back: true,
            span { class: "strong", "Testing “{info_now.title}”" }
        }
        main { class: "test-page",
            div { class: "row test-tools",
                button { class: "button primary", onclick: add_phone, "＋ Phone" }
                button { class: "button", onclick: restart, title: "Start the test over", "↺ Restart" }
                Link {
                    class: "button",
                    to: Route::Editor { id: info_now.quiz_id.clone() },
                    "Edit quiz"
                }
                button { class: "button ghost", onclick: end, "End test" }
                span { class: "muted small grow",
                    "Click the big screen to use its keys and hear its sounds. You can also join from a real phone with code {code}."
                }
            }
            ErrorBox { error: error() }
            div { class: "test-layout",
                section { class: "test-screen",
                    iframe {
                        src: "/host/{code}",
                        title: "Big screen",
                        allow: "autoplay",
                    }
                }
                section { class: "test-phones",
                    for phone in info_now.phones.iter() {
                        TestPhoneFrame {
                            key: "{phone.ticket}",
                            code: code(),
                            name: phone.name.clone(),
                            ticket: phone.ticket.clone(),
                            on_removed: move |()| info.restart(),
                            on_error: move |e| error.set(Some(e)),
                        }
                    }
                    if info_now.phones.is_empty() {
                        p { class: "muted", "No phones. Add one to play." }
                    }
                }
            }
        }
    }
}

/// One test phone: a phone-sized frame playing as its team.
#[component]
fn TestPhoneFrame(
    code: String,
    name: String,
    ticket: String,
    on_removed: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let token: AdminToken = use_context();
    let remove = {
        let code = code.clone();
        let ticket = ticket.clone();
        move |_| {
            let code = code.clone();
            let ticket = ticket.clone();
            async move {
                match remove_test_phone(token.get(), code, ticket).await {
                    Ok(()) => on_removed.call(()),
                    Err(err) => on_error.call(message(&err)),
                }
            }
        }
    };
    rsx! {
        div { class: "test-phone",
            div { class: "row between",
                span { class: "strong small", "{name}" }
                button {
                    class: "button ghost icon",
                    title: "Remove this phone",
                    onclick: remove,
                    "✕"
                }
            }
            iframe {
                src: "/join/{code}?ticket={ticket}",
                title: "{name}",
            }
        }
    }
}
