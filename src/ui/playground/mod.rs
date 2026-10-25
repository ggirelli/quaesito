//! The Playground: a quiz played in the browser alone, its big screen and a pretend phone side by
//! side, as on the admin's test page. The game runs here, with the same engine as the server's
//! games; nothing is sent anywhere.

use dioxus::prelude::*;

use super::{
    ICON, MAIN_CSS,
    categories::Categories,
    client,
    host::HostBoard,
    player::PlayerBoard,
    sfx::SFX_JS,
    widgets::{ErrorBox, Icon, IconKind, THEME_SCRIPT},
};
use crate::model::Quiz;

mod driver;

use driver::{LocalGame, use_local_live};

/// The Playground's page, playing `quiz`.
#[component]
pub fn Playground(quiz: Quiz) -> Element {
    // Restarting mounts a new game.
    let mut rounds = use_signal(|| 0u32);
    rsx! {
        document::Title { "Quaesito Playground" }
        document::Link { rel: "icon", href: ICON, r#type: "image/png" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Meta { name: "theme-color", content: "#14121f" }
        document::Stylesheet { href: MAIN_CSS }
        document::Script { {THEME_SCRIPT} }
        document::Script { src: SFX_JS }
        main { class: "test-page playground",
            div { class: "row test-tools",
                span { class: "strong", "Quaesito Playground" }
                button {
                    class: "button",
                    title: "Start the quiz over",
                    onclick: move |_| rounds += 1,
                    "↺ Restart"
                }
                span { class: "muted small grow",
                    "Play on the phone and follow along on the big screen. Click the big screen to use its keys and hear its sounds."
                }
                // The Playground is published next to the user guide.
                a { class: "button ghost", href: "../", "What is Quaesito?" }
                a {
                    class: "button ghost icon",
                    href: env!("CARGO_PKG_REPOSITORY"),
                    title: "Quaesito on GitHub",
                    aria_label: "Quaesito on GitHub",
                    Icon { kind: IconKind::GitHub }
                }
            }
            // Keys only tell apart the items of a list: a list of one game, so that a new key
            // replaces it.
            for round in [rounds()] {
                PlaygroundGame { key: "{round}", quiz: quiz.clone(), on_restart: move |()| rounds += 1 }
            }
        }
    }
}

/// One game of the Playground, from the lobby to the podium.
#[component]
fn PlaygroundGame(quiz: Quiz, on_restart: EventHandler<()>) -> Element {
    let started =
        use_hook(|| LocalGame::start(quiz.clone(), client::random_below).map(Signal::new));
    match started {
        Ok(game) => rsx! {
            PlaygroundScreens { game, on_restart }
        },
        Err(refusal) => rsx! {
            ErrorBox { error: Some(format!("This quiz cannot be played: {refusal}")) }
        },
    }
}

/// The big screen and the phone of a running game.
#[component]
fn PlaygroundScreens(game: Signal<LocalGame>, on_restart: EventHandler<()>) -> Element {
    let (host, player) = use_local_live(game);
    use_context_provider(|| host);
    use_context_provider(|| player);
    let categories = Categories::use_builtin();
    use_context_provider(|| categories);
    let title = host
        .view
        .read()
        .as_ref()
        .map(|v| v.title.clone())
        .unwrap_or_default();
    let again = move |_| on_restart.call(());
    rsx! {
        div { class: "test-layout",
            section { class: "test-screen playground-screen",
                HostBoard {
                    show_code: false,
                    lobby: rsx! {
                        div { class: "slide playground-intro",
                            h1 { class: "slide-title", "{title}" }
                            p { "You play on the phone; this is the big screen everyone watches." }
                            p { class: "muted", "Press “Start the quiz” when you are ready." }
                        }
                    },
                    tools: rsx! {},
                    finish: rsx! {
                        button { class: "button primary big", onclick: again, "↺ Play again" }
                    },
                }
            }
            section { class: "test-phones",
                div { class: "test-phone",
                    span { class: "strong small", "Your phone" }
                    div { class: "playground-phone",
                        PlayerBoard {
                            lobby: rsx! {
                                main { class: "player-main",
                                    div { class: "big-icon", "👋" }
                                    h1 { class: "status-title", "You're in!" }
                                    p { class: "muted", "The quiz starts from the big screen." }
                                }
                            },
                            done: rsx! {
                                button { class: "button", onclick: again, "Play again" }
                            },
                        }
                    }
                }
            }
        }
    }
}
