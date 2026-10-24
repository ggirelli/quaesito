//! The big screen: lobby with QR code, questions, reveals and podium.

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    Route,
    admin::{AdminGate, AdminToken},
    library::Categories,
    live::{ConnectionBanner, Live, countdown_ms, elapsed_now, remaining_ms, use_clock, use_live},
    sfx::{self, SFX_JS},
    widgets::{Brand, ClientOnly, Icon, IconKind, Spinner, ThemeToggle},
};
use crate::{
    api::{end_session, host_socket, list_sessions},
    protocol::{HostCommand, HostPhase, HostView, Upcoming},
};

mod lobby;
mod panels;
mod question;
mod reveal;
mod sound;
mod standings;
mod tiles;

use lobby::{Lobby, NetworkSlideView};
use panels::{RESULTS_SHOWN, ResultsPanel, TeamsPanel};
use question::{QuestionSlide, ReadySlide, SlideView};
use reveal::{ClosedSlide, ReviewSlide};
use sound::{Playing, Soundtrack, soundtrack, use_sound_effects};
use standings::{Podium, Standings};

/// Seconds the host's "+" button adds to an open question.
const EXTRA_SECS: u32 = 10;

#[component]
pub fn Host(code: String) -> Element {
    rsx! {
        document::Script { src: SFX_JS }
        ClientOnly {
            AdminGate {
                HostScreen { code }
            }
        }
    }
}

#[component]
fn HostScreen(code: String) -> Element {
    let token: AdminToken = use_context();
    let code = use_signal(|| code);
    let live = use_live(move || host_socket(code(), token.get(), WebSocketOptions::new()));
    use_context_provider(|| live);
    // The browser can't tell a refused socket from a dropped one, so on every drop ask whether
    // the game is still running.
    let mut gone = use_signal(|| false);
    use_effect(move || {
        if !(live.connected)() {
            spawn(async move {
                if let Ok(sessions) = list_sessions(token.get()).await {
                    gone.set(!sessions.iter().any(|s| s.code == *code.peek()));
                }
            });
        }
    });
    let categories = Categories::use_loaded();
    use_context_provider(|| categories);
    let now = use_clock(200);
    let mut show_teams = use_signal(|| false);
    let mut show_results = use_signal(|| false);
    let mut muted = use_signal(sfx::is_muted);
    use_sound_effects(live.view, live.received_at, now);

    let send = move |command: HostCommand| {
        // Stopping the clock by hand stops the soundtrack too; when time runs out, the music
        // plays on through the reveal.
        let stopping = command == HostCommand::Next
            && matches!(
                live.view.peek().as_ref().map(|v| &v.phase),
                Some(HostPhase::Question { .. })
            );
        if stopping {
            document::eval(
                "const a = document.getElementById('soundtrack'); if (a) { a.pause(); }",
            );
        }
        live.send(command);
    };

    if gone() {
        return rsx! {
            div { class: "page center",
                h1 { "This game is not running" }
                BackToAdmin { class: "button", code: code() }
            }
        };
    }
    let view = live.view.read();
    let Some(view) = view.as_ref() else {
        return rsx! {
            div { class: "page center", Spinner {} }
        };
    };
    let received_at = (live.received_at)();
    let auto_left = view
        .auto_advance_ms
        .map(|ms| (remaining_ms(ms, received_at, now()) / 1000.0).ceil());
    let paused = view.paused;
    let soundtrack = soundtrack(&view.phase, received_at, paused);
    let can_pause = !matches!(
        view.phase,
        HostPhase::Network { .. } | HostPhase::Lobby { .. } | HostPhase::Ready { .. }
    );
    let question_open = matches!(view.phase, HostPhase::Question { .. });
    let can_reopen =
        matches!(view.phase, HostPhase::Closed { answered, .. } if answered < view.teams.len());

    rsx! {
        div {
            class: "host",
            tabindex: 0,
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            // Presentation clickers send → / PageDown and ← / PageUp. Enter and Space are left
            // alone because they also activate a focused button, which would act twice.
            onkeydown: move |e: KeyboardEvent| {
                let command = match e.key() {
                    Key::ArrowRight | Key::PageDown => HostCommand::Next,
                    Key::ArrowLeft | Key::PageUp => HostCommand::Back,
                    Key::Character(c) if c == "n" => HostCommand::Next,
                    Key::Character(c) if c == "b" => HostCommand::Back,
                    Key::Character(c) if c == "p" => HostCommand::TogglePause,
                    _ => return,
                };
                e.prevent_default();
                send(command);
            },
            header { class: "host-bar",
                Brand {}
                span { class: "strong", "{view.title}" }
                div { class: "row",
                    span {
                        "Code "
                        span { class: "code", "{view.code}" }
                    }
                    button {
                        class: "button ghost",
                        title: if muted() { "Turn sound effects on" } else { "Mute sound effects" },
                        aria_pressed: muted(),
                        onclick: move |_| {
                            let mute = !muted();
                            sfx::set_muted(mute);
                            muted.set(mute);
                        },
                        if muted() { "🔇" } else { "🔊" }
                    }
                    button {
                        class: "button ghost",
                        title: "Rename or remove teams",
                        onclick: move |_| show_teams.set(true),
                        "Teams ({view.teams.len()})"
                    }
                    Link {
                        class: "button ghost icon",
                        to: Route::Admin {},
                        title: "Admin",
                        aria_label: "Admin",
                        Icon { kind: IconKind::Settings }
                    }
                    ThemeToggle { class: "button ghost icon" }
                }
            }
            if show_teams() {
                TeamsPanel {
                    teams: view.teams.clone(),
                    pictures: view.pictures,
                    joining: view.joining,
                    on_close: move |()| show_teams.set(false),
                }
            }
            if paused {
                div { class: "paused-banner", role: "status", "⏸ Paused" }
            }
            main { class: "host-main",
                match &view.phase {
                    HostPhase::Network { network } => rsx! {
                        NetworkSlideView { network: network.clone() }
                    },
                    HostPhase::Lobby { .. } => rsx! {
                        Lobby { code: view.code.clone(), teams: view.teams.clone(), max_teams: view.max_teams }
                    },
                    HostPhase::Ready { question } => rsx! {
                        ReadySlide { question: question.clone(), teams: view.teams.len() }
                    },
                    HostPhase::Question { question, remaining_ms: left, elapsed_ms, answered } => rsx! {
                        QuestionSlide {
                            question: question.clone(),
                            left_ms: countdown_ms(*left, received_at, now(), paused),
                            elapsed_ms: elapsed_now(*elapsed_ms, received_at, now(), paused),
                            answered: *answered,
                            teams: view.teams.len(),
                            paused,
                        }
                    },
                    HostPhase::Closed { question, answered, reveal, fastest, standings, bonus, explanation, .. } => rsx! {
                        ClosedSlide {
                            question: question.clone(),
                            answered: *answered,
                            teams: view.teams.len(),
                            reveal: reveal.clone(),
                            fastest: fastest.clone(),
                            standings: standings.clone(),
                            bonus: bonus.clone(),
                            explanation: explanation.clone(),
                        }
                    },
                    HostPhase::Slide { slide, .. } => rsx! {
                        SlideView { slide: slide.clone() }
                    },
                    HostPhase::Standings { title, standings, .. } => rsx! {
                        div {
                            class: "slide results-slide",
                            style: "--rows: {standings.len().min(RESULTS_SHOWN)}",
                            if !title.is_empty() {
                                h1 { class: "slide-title", "{title}" }
                            }
                            Standings { rows: standings.iter().take(RESULTS_SHOWN).cloned().collect::<Vec<_>>() }
                        }
                    },
                    HostPhase::Review { question, answered, reveal, fastest, bonus, explanation, .. } => rsx! {
                        ReviewSlide {
                            question: question.clone(),
                            answered: *answered,
                            teams: view.teams.len(),
                            reveal: reveal.clone(),
                            fastest: fastest.clone(),
                            bonus: bonus.clone(),
                            explanation: explanation.clone(),
                        }
                    },
                    HostPhase::Podium { standings, finished } => rsx! {
                        Podium { standings: standings.clone(), finished: *finished, teams: view.teams.len() }
                    },
                }
            }
            footer { class: "host-controls",
                div { class: "row host-tools",
                    button {
                        class: "button ghost",
                        title: "Back (← or B)",
                        disabled: view.phase.is_start(),
                        onclick: move |_| send(HostCommand::Back),
                        "◀ Back"
                    }
                    if can_reopen {
                        button {
                            class: "button ghost",
                            title: "Open this question again for the teams that have not answered",
                            onclick: move |_| send(HostCommand::Reopen),
                            "↺ Reopen"
                        }
                    }
                    if can_pause {
                        button {
                            class: if paused { "button primary" } else { "button ghost" },
                            title: "Pause or resume the clock (P)",
                            onclick: move |_| send(HostCommand::TogglePause),
                            if paused { "▶ Resume" } else { "⏸ Pause" }
                        }
                    }
                    if question_open {
                        button {
                            class: "button ghost",
                            title: "Give everyone more time",
                            onclick: move |_| send(HostCommand::AddTime(EXTRA_SECS)),
                            "+{EXTRA_SECS} s"
                        }
                    }
                }
                span { class: "grow" }
                if let Some(Playing { key, name, started_at, seek }) = soundtrack {
                    Soundtrack { key: "{key}", name, started_at, seek, paused }
                }
                if let Some(secs) = auto_left {
                    span { class: "auto-note", "Continuing in {secs}s" }
                }
                if let Some(label) = next_label(view) {
                    button {
                        class: "button primary big",
                        disabled: matches!(view.phase, HostPhase::Lobby { .. }) && view.teams.is_empty(),
                        onclick: move |_| send(HostCommand::Next),
                        "{label}"
                    }
                } else {
                    button {
                        class: "button primary big",
                        onclick: move |_| show_results.set(true),
                        "📊 Results"
                    }
                    BackToAdmin { class: "button big", code: code() }
                }
            }
            if show_results() {
                ResultsPanel { code: code(), on_close: move |()| show_results.set(false) }
            }
            ConnectionBanner { connected: (live.connected)() }
        }
    }
}

/// Back to the quiz list. In the test page's frame it ends the test and leaves the test page,
/// instead of opening the admin pages inside the frame.
#[component]
fn BackToAdmin(class: &'static str, code: String) -> Element {
    let token: AdminToken = use_context();
    let leave = move |_| {
        let code = code.clone();
        async move {
            let framed = document::eval("return window.self !== window.top;")
                .join::<bool>()
                .await
                .unwrap_or(false);
            if framed {
                let _ = end_session(token.get(), code).await;
                document::eval("window.top.location.assign('/admin');");
            } else {
                navigator().push(Route::Admin {});
            }
        }
    };
    rsx! {
        button { class, onclick: leave, "Back to admin" }
    }
}

/// Label of the host's "next" button, or `None` once everything is revealed.
fn next_label(view: &HostView) -> Option<String> {
    Some(match &view.phase {
        HostPhase::Network { .. } => "Show the join code".to_owned(),
        HostPhase::Lobby { .. } => "Start the quiz".to_owned(),
        HostPhase::Ready { .. } => "▶ Start the music".to_owned(),
        HostPhase::Question { .. } => "Stop the clock".to_owned(),
        HostPhase::Closed { then, .. }
        | HostPhase::Slide { then, .. }
        | HostPhase::Standings { then, .. } => match then {
            Upcoming::Question => "Next question",
            Upcoming::Slide => "Continue",
            Upcoming::Results => "Reveal the answers so far",
            Upcoming::Answers => "Reveal the answers",
            Upcoming::Podium => "On to the podium",
        }
        .to_owned(),
        HostPhase::Review {
            then: Upcoming::Podium,
            ..
        } => "On to the podium".to_owned(),
        HostPhase::Review {
            then: Upcoming::Results,
            ..
        } => "Show the standings".to_owned(),
        HostPhase::Review { .. } => "Next answer".to_owned(),
        HostPhase::Podium { finished: true, .. } => return None,
        HostPhase::Podium { standings, .. } => {
            let podium = view.teams.len().min(3);
            let shown = standings.iter().filter(|r| r.rank <= podium).count();
            match podium - shown {
                3 => "Reveal 3rd place",
                2 => "Reveal 2nd place",
                _ => "Reveal the winner",
            }
            .to_owned()
        }
    })
}

/// Whether answer times are shown to the millisecond: when faster answers earn more points.
fn use_precise_times() -> bool {
    let live: Live<HostCommand, HostView> = use_context();
    live.view
        .read()
        .as_ref()
        .is_some_and(|view| view.speed_points)
}

/// Milliseconds as "2.3 s", or "2.347 s" when `precise`.
fn seconds(ms: u64, precise: bool) -> String {
    #[allow(clippy::cast_precision_loss, reason = "answer times are small")]
    let secs = ms as f64 / 1000.0;
    if precise {
        format!("{secs:.3} s")
    } else {
        format!("{secs:.1} s")
    }
}
