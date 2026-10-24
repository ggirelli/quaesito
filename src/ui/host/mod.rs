//! The big screen: questions, reveals and podium, driven by a game on the server (`connected`) or
//! in the browser (the Playground).

use dioxus::prelude::*;

use super::{
    live::{ConnectionBanner, Live, countdown_ms, elapsed_now, remaining_ms, use_clock},
    sfx,
    widgets::{Brand, Spinner, ThemeToggle},
};
use crate::protocol::{HostCommand, HostPhase, HostView, Upcoming};

#[cfg(feature = "fullstack")]
mod connected;
#[cfg(feature = "fullstack")]
mod lobby;
#[cfg(feature = "fullstack")]
mod panels;
mod question;
mod reveal;
mod sound;
mod standings;
mod tiles;

#[cfg(feature = "fullstack")]
pub use connected::Host;
use question::{QuestionSlide, ReadySlide, SlideView};
use reveal::{ClosedSlide, ReviewSlide};
use sound::{Playing, Soundtrack, soundtrack, use_sound_effects};
use standings::{Podium, RESULTS_SHOWN, Standings};

/// Seconds the host's "+" button adds to an open question.
const EXTRA_SECS: u32 = 10;

/// The big screen of a live game, whatever drives it: plays the [`Live`] game in the context,
/// and leaves the lobby, the extra tools and the end to its caller.
#[component]
pub fn HostBoard(
    /// What the Wi-Fi slide and the lobby show.
    lobby: Element,
    /// More buttons in the header, after the sound toggle.
    tools: Element,
    /// The buttons once everything is revealed.
    finish: Element,
    /// Whether the header shows the join code.
    #[props(default = true)]
    show_code: bool,
    /// Panels opened over the game.
    children: Element,
) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let now = use_clock(200);
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
                    if show_code {
                        span {
                            "Code "
                            span { class: "code", "{view.code}" }
                        }
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
                    {tools}
                    ThemeToggle { class: "button ghost icon" }
                }
            }
            if paused {
                div { class: "paused-banner", role: "status", "⏸ Paused" }
            }
            main { class: "host-main",
                match &view.phase {
                    HostPhase::Network { .. } | HostPhase::Lobby { .. } => lobby,
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
                    {finish}
                }
            }
            {children}
            ConnectionBanner { connected: (live.connected)() }
        }
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
