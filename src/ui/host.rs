//! The big screen: lobby with QR code, questions, reveals and podium.

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    Route,
    admin::{AdminGate, AdminToken},
    client,
    live::{ConnectionBanner, remaining_ms, use_clock, use_live},
    widgets::{ANSWER_SHAPES, Brand, ClientOnly, Icon, IconKind, Spinner, TeamAvatar, media_url},
};
use crate::{
    api::{host_socket, join_origin},
    protocol::{HostCommand, HostPhase, HostView, QuestionInfo, Reveal, StandingRow, TeamInfo},
};

/// How many teams the between-questions leaderboard shows.
const STANDINGS_SHOWN: usize = 6;

#[component]
pub fn Host(code: String) -> Element {
    rsx! {
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
    let now = use_clock(200);

    let next = move || async move {
        let _ = live.socket.send(HostCommand::Next).await;
    };

    let view = live.view.read();
    let Some(view) = view.as_ref() else {
        return rsx! {
            div { class: "page center",
                if live.socket.is_err() {
                    h1 { "This game is not running" }
                    Link { class: "button", to: Route::Admin {}, "Back to admin" }
                } else {
                    Spinner {}
                }
            }
        };
    };
    let received_at = (live.received_at)();
    let auto_left = view
        .auto_advance_ms
        .map(|ms| (remaining_ms(ms, received_at, now()) / 1000.0).ceil());
    let soundtrack = soundtrack(&view.phase, received_at);

    rsx! {
        div {
            class: "host",
            tabindex: 0,
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            // Presentation clickers send → or PageDown. Enter and Space are left alone because
            // they also activate a focused button, which would advance twice.
            onkeydown: move |e: KeyboardEvent| async move {
                let key = e.key();
                if matches!(key, Key::ArrowRight | Key::PageDown)
                    || key == Key::Character("n".to_owned())
                {
                    e.prevent_default();
                    next().await;
                }
            },
            header { class: "host-bar",
                Brand {}
                span { class: "strong", "{view.title}" }
                span {
                    "Code "
                    span { class: "code", "{view.code}" }
                }
            }
            main { class: "host-main",
                match &view.phase {
                    HostPhase::Lobby => rsx! { Lobby { code: view.code.clone(), teams: view.teams.clone() } },
                    HostPhase::Ready { question } => rsx! {
                        ReadySlide { question: question.clone(), teams: view.teams.len() }
                    },
                    HostPhase::Question { question, remaining_ms: left, answered } => rsx! {
                        QuestionSlide {
                            question: question.clone(),
                            left_ms: remaining_ms(*left, received_at, now()),
                            answered: *answered,
                            teams: view.teams.len(),
                        }
                    },
                    HostPhase::Closed { question, answered, reveal, standings, .. } => rsx! {
                        ClosedSlide {
                            question: question.clone(),
                            answered: *answered,
                            teams: view.teams.len(),
                            reveal: reveal.clone(),
                            standings: standings.clone(),
                        }
                    },
                    HostPhase::Review { question, reveal, .. } => rsx! {
                        ReviewSlide { question: question.clone(), reveal: reveal.clone() }
                    },
                    HostPhase::Podium { standings, finished } => rsx! {
                        Podium { standings: standings.clone(), finished: *finished, teams: view.teams.len() }
                    },
                }
            }
            footer { class: "host-controls",
                if let Some((index, name, started_at)) = soundtrack {
                    Soundtrack { key: "{index}", name, started_at }
                }
                if let Some(secs) = auto_left {
                    span { class: "auto-note", "Continuing in {secs}s" }
                }
                if let Some(label) = next_label(view) {
                    button {
                        class: "button primary big",
                        disabled: matches!(view.phase, HostPhase::Lobby) && view.teams.is_empty(),
                        onclick: move |_| next(),
                        "{label}"
                    }
                } else {
                    Link { class: "button big", to: Route::Admin {}, "Back to admin" }
                }
            }
            ConnectionBanner { connected: (live.connected)() }
        }
    }
}

/// Label of the host's "next" button, or `None` once everything is revealed.
fn next_label(view: &HostView) -> Option<String> {
    Some(match &view.phase {
        HostPhase::Lobby => "Start the quiz".to_owned(),
        HostPhase::Ready { .. } => "▶ Start the music".to_owned(),
        HostPhase::Question { .. } => "Stop the clock".to_owned(),
        HostPhase::Closed { last: false, .. } => "Next question".to_owned(),
        HostPhase::Closed {
            last: true,
            reveal: None,
            ..
        } => "Reveal the answers".to_owned(),
        HostPhase::Closed { last: true, .. } | HostPhase::Review { last: true, .. } => {
            "On to the podium".to_owned()
        }
        HostPhase::Review { last: false, .. } => "Next answer".to_owned(),
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

/// The soundtrack to keep on screen: question index, media name, and the browser time its
/// clock started, once it has.
fn soundtrack(phase: &HostPhase, received_at: f64) -> Option<(usize, String, Option<f64>)> {
    let (question, started_at) = match phase {
        HostPhase::Question {
            question,
            remaining_ms: left,
            ..
        } => {
            #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
            let elapsed = question.time_limit_ms.saturating_sub(*left) as f64;
            (question, Some(received_at - elapsed))
        }
        // Ready preloads the music; Closed keeps it going while the answers are shown.
        HostPhase::Ready { question } | HostPhase::Closed { question, .. } => (question, None),
        _ => return None,
    };
    let name = question.audio.clone()?;
    Some((question.index, name, started_at))
}

/// Plays a question's soundtrack once its clock has started, from the matching position.
#[component]
fn Soundtrack(name: String, started_at: Option<f64>) -> Element {
    let mut played = use_signal(|| false);
    use_effect(use_reactive!(|started_at| {
        let Some(started_at) = started_at else {
            return;
        };
        if *played.peek() {
            return;
        }
        played.set(true);
        let offset = ((client::now_ms() - started_at) / 1000.0).max(0.0);
        document::eval(&format!(
            "const a = document.getElementById('soundtrack');\n\
             if (a) {{ a.currentTime = {offset:.3}; a.play().catch(() => {{}}); }}"
        ));
    }));
    rsx! {
        div { class: "soundtrack",
            Icon { kind: IconKind::Music }
            audio {
                id: "soundtrack",
                src: media_url(&name),
                preload: "auto",
                controls: true,
            }
        }
    }
}

#[component]
fn Lobby(code: String, teams: Vec<TeamInfo>) -> Element {
    let token: AdminToken = use_context();
    let origin = use_resource(move || async move {
        join_origin(token.get(), client::page_port())
            .await
            .unwrap_or_else(|_| client::page_origin())
    });
    let url = origin().map(|o| format!("{o}/join/{code}"));
    let qr = url.as_deref().and_then(qr_svg);
    rsx! {
        div { class: "lobby",
            div { class: "join-panel",
                span { "Join on your phone" }
                div { class: "qr",
                    if let Some(svg) = qr {
                        div { dangerous_inner_html: svg }
                    }
                }
                if let Some(url) = url {
                    span { class: "join-url", "{url}" }
                }
                span { "or enter the code" }
                span { class: "join-code", "{code}" }
            }
            div { class: "stack",
                h1 { class: "question-title",
                    if teams.is_empty() {
                        "Waiting for teams…"
                    } else if teams.len() == 1 {
                        "1 team is in"
                    } else {
                        "{teams.len()} teams are in"
                    }
                }
                div { class: "teams-grid",
                    for team in teams {
                        div {
                            key: "{team.id}",
                            class: if team.connected { "team-chip" } else { "team-chip offline" },
                            TeamAvatar { team: team.clone() }
                            span { "{team.name}" }
                        }
                    }
                }
            }
        }
    }
}

/// The join link as an SVG QR code.
fn qr_svg(url: &str) -> Option<String> {
    let code = qrcode::QrCode::new(url.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .quiet_zone(false)
            .dark_color(qrcode::render::svg::Color("#14121f"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

#[component]
fn QuestionHeader(question: QuestionInfo, children: Element) -> Element {
    rsx! {
        div { class: "question-head",
            span { class: "muted", "Question {question.index + 1} of {question.total}" }
            {children}
        }
        h1 { class: "question-title", "{question.text}" }
        if question.multi {
            p { class: "muted", style: "text-align:center;margin:0", "Several answers are correct" }
        }
    }
}

#[component]
fn QuestionSlide(question: QuestionInfo, left_ms: f64, answered: usize, teams: usize) -> Element {
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let fraction = if question.time_limit_ms == 0 {
        0.0
    } else {
        left_ms / question.time_limit_ms as f64
    };
    let seconds = (left_ms / 1000.0).ceil();
    rsx! {
        QuestionHeader { question: question.clone(),
            Timer { fraction, seconds }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                img { class: "question-image", src: media_url(image), alt: "" }
            }
            div { class: "answered",
                strong { "{answered}/{teams}" }
                span { class: "muted", "answered" }
            }
        }
        AnswerGrid { answers: question.answers.clone(), reveal: None }
    }
}

/// A soundtrack question before the music starts: everything is shown, the clock is full.
#[component]
fn ReadySlide(question: QuestionInfo, teams: usize) -> Element {
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let seconds = (question.time_limit_ms as f64 / 1000.0).ceil();
    rsx! {
        QuestionHeader { question: question.clone(),
            Timer { fraction: 1.0, seconds }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                img { class: "question-image", src: media_url(image), alt: "" }
            }
            div { class: "answered",
                strong { Icon { kind: IconKind::Music } }
                span { class: "muted", "The clock starts with the music" }
            }
        }
        AnswerGrid { answers: question.answers.clone(), reveal: None }
    }
}

#[component]
fn Timer(fraction: f64, seconds: f64) -> Element {
    const CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * 45.0;
    let offset = CIRCUMFERENCE * (1.0 - fraction.clamp(0.0, 1.0));
    rsx! {
        div { class: if seconds <= 5.0 { "timer urgent" } else { "timer" },
            svg { view_box: "0 0 100 100",
                circle { class: "track", cx: 50, cy: 50, r: 45 }
                circle {
                    class: "progress",
                    cx: 50,
                    cy: 50,
                    r: 45,
                    stroke_dasharray: "{CIRCUMFERENCE}",
                    stroke_dashoffset: "{offset}",
                }
            }
            span { "{seconds}" }
        }
    }
}

#[component]
fn AnswerGrid(answers: Vec<String>, reveal: Option<Reveal>) -> Element {
    rsx! {
        div { class: "answer-grid",
            for (i, text) in answers.iter().enumerate() {
                {
                    let mut class = format!("answer-tile a{}", i % ANSWER_SHAPES.len());
                    if let Some(reveal) = &reveal {
                        class.push_str(if reveal.correct.contains(&i) { " right" } else { " dim" });
                    }
                    let picks = reveal.as_ref().and_then(|r| r.picks.get(i).copied());
                    rsx! {
                        div { key: "{i}", class,
                            span { class: "shape", "{ANSWER_SHAPES[i % ANSWER_SHAPES.len()]}" }
                            span { class: "text", "{text}" }
                            if let Some(picks) = picks {
                                span { class: "picks", "{picks} 👥" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ClosedSlide(
    question: QuestionInfo,
    answered: usize,
    teams: usize,
    reveal: Option<Reveal>,
    standings: Option<Vec<StandingRow>>,
) -> Element {
    match (reveal, standings) {
        (Some(reveal), Some(standings)) => rsx! {
            QuestionHeader { question: question.clone(),
                span { class: "muted", "{answered}/{teams} answered" }
            }
            div { class: "closed-layout",
                AnswerGrid { answers: question.answers.clone(), reveal: Some(reveal) }
                Standings { rows: standings.into_iter().take(STANDINGS_SHOWN).collect::<Vec<_>>() }
            }
        },
        _ => rsx! {
            QuestionHeader { question: question.clone() }
            p { class: "waiting-note",
                "Time's up! {answered} of {teams} teams answered."
                br {}
                "Answers are revealed at the end."
            }
        },
    }
}

#[component]
fn ReviewSlide(question: QuestionInfo, reveal: Reveal) -> Element {
    rsx! {
        QuestionHeader { question: question.clone(),
            span { class: "muted", "The answers" }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                img { class: "question-image", src: media_url(image), alt: "" }
            }
        }
        AnswerGrid { answers: question.answers.clone(), reveal: Some(reveal) }
    }
}

#[component]
fn Standings(rows: Vec<StandingRow>) -> Element {
    rsx! {
        ol { class: "standings",
            for (i, row) in rows.into_iter().enumerate() {
                li {
                    key: "{row.team.id}",
                    class: "standing",
                    style: "animation-delay: {i * 80}ms",
                    span { class: "rank", "{row.rank}" }
                    TeamAvatar { team: row.team.clone() }
                    span { class: "name", "{row.team.name}" }
                    span { class: "score", "{row.score}" }
                }
            }
        }
    }
}

#[component]
fn Podium(standings: Vec<StandingRow>, finished: bool, teams: usize) -> Element {
    let spot = |rank: usize| standings.iter().find(|r| r.rank == rank).cloned();
    let runners_up: Vec<StandingRow> = standings.iter().filter(|r| r.rank > 3).cloned().collect();
    rsx! {
        h1 { class: "question-title",
            if finished { "🏆 Congratulations!" } else { "And the winners are…" }
        }
        div { class: "podium",
            for rank in 1..=teams.min(3) {
                PodiumSpot { key: "{rank}", rank, row: spot(rank) }
            }
        }
        if !runners_up.is_empty() {
            div { class: "runners-up",
                for row in runners_up {
                    span { key: "{row.team.id}", class: "team-chip",
                        "{row.rank}. {row.team.name} · {row.score}"
                    }
                }
            }
        }
    }
}

#[component]
fn PodiumSpot(rank: usize, row: Option<StandingRow>) -> Element {
    let medal = match rank {
        1 => "1",
        2 => "2",
        _ => "3",
    };
    match row {
        Some(row) => rsx! {
            div { class: "podium-spot p{rank}",
                TeamAvatar { team: row.team.clone() }
                span { class: "team-name", "{row.team.name}" }
                span { class: "points", "{row.score} pts" }
                div { class: "podium-block", "{medal}" }
            }
        },
        None => rsx! {
            div { class: "podium-spot p{rank} hidden",
                div { class: "podium-block" }
            }
        },
    }
}
