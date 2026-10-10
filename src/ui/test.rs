//! The admin's test page: the big screen and a few phones side by side, to try a quiz quickly.
//!
//! Each panel is the real page in a frame (`/host/<code>`, `/join/<code>?ticket=…`), so the test
//! shows exactly what players see, each at its own screen size.

use dioxus::prelude::*;

use super::{
    Route,
    admin::{AdminGate, AdminHeader, AdminToken},
    client,
    widgets::{ClientOnly, ErrorBox, Spinner, message},
};
use crate::api::{add_test_phone, end_session, remove_test_phone, start_test, test_info};

/// `/admin/test/:code`.
#[component]
pub fn Test(code: String) -> Element {
    rsx! {
        ClientOnly {
            AdminGate {
                TestPage { code }
            }
        }
    }
}

/// The page follows `code`: Restart navigates to a new test game, and the router keeps this
/// component, so everything below reads the current code rather than the first one.
#[component]
fn TestPage(code: String) -> Element {
    let token: AdminToken = use_context();
    let mut current = use_signal(|| code.clone());
    let mut error = use_signal(|| None::<String>);
    let mut info = use_resource(move || async move { test_info(token.get(), current()).await });
    let nav = navigator();
    let ready = use_ready_frames();
    use_effect(use_reactive!(|code| {
        if *current.peek() != code {
            current.set(code);
            error.set(None);
        }
    }));
    let code = current;

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
                    for (i, phone) in info_now.phones.iter().enumerate() {
                        TestPhoneFrame {
                            key: "{phone.ticket}",
                            // Frame i + 1 waits for the frames before it (the big screen first).
                            load: ready() > i,
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

/// Re-checks the frames this often.
const FRAME_CHECK_MS: u32 = 500;

/// Finds how many frames, in page order, have their app running, and reloads a frame still
/// loading after 15 s.
///
/// Frames are loaded one after another because browsers can stall (Firefox never finishes one of
/// them) when several frames fetch the same large app at once; the reload is a safety net.
const FRAME_CHECK_JS: &str = r"
const frames = [...document.querySelectorAll('.test-layout iframe')];
let ready = 0;
let counting = true;
for (const frame of frames) {
  let running = false;
  try {
    const doc = frame.contentDocument;
    running = !!doc && !!doc.querySelector('.host, .player, form.card');
  } catch (e) {}
  if (!frame.dataset.since) frame.dataset.since = String(Date.now());
  if (running) {
    frame.dataset.since = '';
  } else if (Date.now() - Number(frame.dataset.since) > 15000) {
    frame.dataset.since = String(Date.now());
    frame.src = frame.src;
  }
  if (counting && running) ready += 1; else counting = false;
}
return ready;
";

/// How many frames in a row, from the big screen, have their app running.
fn use_ready_frames() -> Signal<usize> {
    let mut ready = use_signal(|| 0usize);
    use_future(move || async move {
        loop {
            client::sleep_ms(FRAME_CHECK_MS).await;
            if let Ok(count) = document::eval(FRAME_CHECK_JS).join::<usize>().await
                && count != *ready.peek()
            {
                ready.set(count);
            }
        }
    });
    ready
}

/// One test phone: a phone-sized frame playing as its team.
#[component]
fn TestPhoneFrame(
    load: bool,
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
            if load {
                iframe {
                    src: "/join/{code}?ticket={ticket}",
                    title: "{name}",
                }
            } else {
                div { class: "test-phone-waiting",
                    Spinner {}
                }
            }
        }
    }
}
