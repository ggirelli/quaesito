//! The big screen: lobby with QR code, questions, reveals and podium.

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    LOBBY_ART, Route,
    admin::{AdminGate, AdminToken},
    client,
    live::{ConnectionBanner, Live, remaining_ms, use_clock, use_live},
    widgets::{ANSWER_SHAPES, Brand, ClientOnly, Icon, IconKind, Spinner, TeamAvatar, media_url},
};
use crate::{
    api::{host_socket, join_origin},
    protocol::{
        Grade, Guess, HostCommand, HostPhase, HostView, InputKind, Judgement, MAX_NAME_CHARS,
        QuestionInfo, Reveal, SlideInfo, StandingRow, TeamInfo, TextGroup, Upcoming,
    },
    scoring::MAX_BONUS,
};

/// The lobby illustration is shown while fewer teams than this have joined.
const LOBBY_ART_MAX_TEAMS: usize = 5;
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
    use_context_provider(|| live);
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
    let mut show_teams = use_signal(|| false);

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
                div { class: "row",
                    span {
                        "Code "
                        span { class: "code", "{view.code}" }
                    }
                    button {
                        class: "button ghost",
                        title: "Rename or remove teams",
                        onclick: move |_| show_teams.set(true),
                        "Teams ({view.teams.len()})"
                    }
                }
            }
            if show_teams() {
                TeamsPanel {
                    teams: view.teams.clone(),
                    on_close: move |()| show_teams.set(false),
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
                    HostPhase::Slide { slide, .. } => rsx! {
                        SlideView { slide: slide.clone() }
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

/// The host's team manager: rename or remove teams.
#[component]
fn TeamsPanel(teams: Vec<TeamInfo>, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "overlay",
            onclick: move |_| on_close.call(()),
            // Typing a name must not advance the quiz.
            onkeydown: move |e: KeyboardEvent| {
                e.stop_propagation();
                if e.key() == Key::Escape {
                    on_close.call(());
                }
            },
            section {
                class: "card stack panel",
                role: "dialog",
                aria_label: "Teams",
                onclick: move |e| e.stop_propagation(),
                div { class: "row between",
                    h2 { "Teams" }
                    button { class: "button ghost", onclick: move |_| on_close.call(()), "Close" }
                }
                if teams.is_empty() {
                    p { class: "muted", "No team has joined yet." }
                }
                ul { class: "list",
                    for team in teams.iter() {
                        TeamRow {
                            key: "{team.id}",
                            team: team.clone(),
                            taken: teams
                                .iter()
                                .filter(|t| t.id != team.id)
                                .map(|t| t.name.to_lowercase())
                                .collect::<Vec<_>>(),
                        }
                    }
                }
                p { class: "muted small",
                    "A removed team loses its points and its phone goes back to the join page."
                }
            }
        }
    }
}

#[component]
fn TeamRow(team: TeamInfo, taken: Vec<String>) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let mut error = use_signal(|| None::<&'static str>);
    let mut confirming = use_signal(|| false);
    let id = team.id;
    let rename = move |e: FormEvent| {
        let name = e.value().trim().to_owned();
        let taken = taken.clone();
        async move {
            if name.is_empty() {
                error.set(Some("A team needs a name."));
            } else if taken.contains(&name.to_lowercase()) {
                error.set(Some("Another team already has that name."));
            } else {
                error.set(None);
                let _ = live
                    .socket
                    .send(HostCommand::RenameTeam { team: id, name })
                    .await;
            }
        }
    };
    let remove = move |_| async move {
        let _ = live.socket.send(HostCommand::RemoveTeam { team: id }).await;
    };
    rsx! {
        li { class: "list-row team-row",
            TeamAvatar { team: team.clone() }
            div { class: "grow stack-tight",
                input {
                    value: "{team.name}",
                    maxlength: MAX_NAME_CHARS,
                    aria_label: "Team name",
                    onchange: rename,
                }
                if let Some(error) = error() {
                    span { class: "error small", "{error}" }
                }
            }
            span { class: "muted small",
                if team.connected { "online" } else { "offline" }
            }
            if confirming() {
                button { class: "button danger", onclick: remove, "Really remove?" }
                button { class: "button ghost", onclick: move |_| confirming.set(false), "Keep" }
            } else {
                button { class: "button ghost", onclick: move |_| confirming.set(true), "Remove" }
            }
        }
    }
}

/// Label of the host's "next" button, or `None` once everything is revealed.
fn next_label(view: &HostView) -> Option<String> {
    Some(match &view.phase {
        HostPhase::Lobby => "Start the quiz".to_owned(),
        HostPhase::Ready { .. } => "▶ Start the music".to_owned(),
        HostPhase::Question { .. } => "Stop the clock".to_owned(),
        HostPhase::Closed { then, .. } | HostPhase::Slide { then, .. } => match then {
            Upcoming::Question => "Next question",
            Upcoming::Slide => "Continue",
            Upcoming::Answers => "Reveal the answers",
            Upcoming::Podium => "On to the podium",
        }
        .to_owned(),
        HostPhase::Review {
            then: Upcoming::Podium,
            ..
        } => "On to the podium".to_owned(),
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
    // No player controls: the host starts the music with the "next" step only, which also
    // starts the clock.
    let mut playing = use_signal(|| false);
    rsx! {
        div {
            class: if playing() { "soundtrack playing" } else { "soundtrack" },
            title: if playing() { "Music playing" } else { "Soundtrack ready" },
            Icon { kind: IconKind::Music }
            span { class: "equalizer", aria_hidden: "true",
                span {}
                span {}
                span {}
            }
            audio {
                id: "soundtrack",
                src: media_url(&name),
                preload: "auto",
                onplay: move |_| playing.set(true),
                onpause: move |_| playing.set(false),
                onended: move |_| playing.set(false),
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
                // The illustration fills the room until enough teams have joined to take its place.
                if teams.len() < LOBBY_ART_MAX_TEAMS {
                    img { class: "lobby-art", src: LOBBY_ART, alt: "" }
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
            span { class: "muted", "Question {question.number} of {question.total}" }
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
        AnswersOrPrompt { question: question.clone() }
    }
}

/// A slide between questions: title, picture and text, each optional.
#[component]
fn SlideView(slide: SlideInfo) -> Element {
    rsx! {
        div { class: "slide",
            if !slide.title.is_empty() {
                h1 { class: "slide-title", "{slide.title}" }
            }
            if let Some(image) = &slide.image {
                img { class: "slide-image", src: media_url(image), alt: "" }
            }
            if !slide.text.is_empty() {
                p { class: "slide-text", "{slide.text}" }
            }
        }
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
        AnswersOrPrompt { question: question.clone() }
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
fn AnswerGrid(answers: Vec<String>, reveal: Option<(Vec<usize>, Vec<u32>)>) -> Element {
    rsx! {
        div { class: "answer-grid",
            for (i, text) in answers.iter().enumerate() {
                {
                    let mut class = format!("answer-tile a{}", i % ANSWER_SHAPES.len());
                    if let Some((correct, _)) = &reveal {
                        class.push_str(if correct.contains(&i) { " right" } else { " dim" });
                    }
                    let picks = reveal.as_ref().and_then(|(_, picks)| picks.get(i).copied());
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

/// The answer buttons of a multiple-choice question, or what to do for other kinds.
#[component]
fn AnswersOrPrompt(question: QuestionInfo) -> Element {
    let prompt = match question.input {
        InputKind::Choice => {
            return rsx! {
                AnswerGrid { answers: question.answers.clone(), reveal: None }
            };
        }
        InputKind::Text => "✍️ Type your answer on your phone",
        InputKind::Number => "🔢 Enter a number on your phone: closest wins",
    };
    rsx! {
        p { class: "open-prompt", "{prompt}" }
    }
}

/// The right answer and what teams answered, per question kind.
#[component]
fn RevealBody(question: QuestionInfo, reveal: Reveal) -> Element {
    match reveal {
        Reveal::Choice { correct, picks } => rsx! {
            AnswerGrid { answers: question.answers.clone(), reveal: Some((correct, picks)) }
        },
        Reveal::Text { accepted, groups } => rsx! {
            TextReveal { question: question.index, accepted, groups }
        },
        Reveal::Number { answer, guesses } => rsx! {
            NumberReveal { answer, guesses }
        },
    }
}

/// Typed answers grouped by normalised text, with the judge's controls.
#[component]
fn TextReveal(question: usize, accepted: Vec<String>, groups: Vec<TextGroup>) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let judge = move |key: String, judgement: Judgement| async move {
        let _ = live
            .socket
            .send(HostCommand::Judge {
                question,
                key,
                judgement,
            })
            .await;
    };
    rsx! {
        div { class: "open-reveal",
            p { class: "accepted",
                span { class: "muted", "Answer: " }
                strong { {accepted.join(" · ")} }
            }
            if groups.is_empty() {
                p { class: "muted", "Nobody answered." }
            }
            ul { class: "text-groups",
                for group in groups {
                    {
                        let j = group.judgement;
                        let class = match j.grade {
                            Grade::Full => "text-group full",
                            Grade::Half => "text-group half",
                            Grade::Wrong => "text-group wrong",
                        };
                        let bonus = match j.bonus {
                            0 => String::new(),
                            b if b > 0 => format!("+{b}"),
                            b => b.to_string(),
                        };
                        let grade_button = |grade: Grade, label: &'static str, title: &'static str| {
                            let key = group.key.clone();
                            rsx! {
                                button {
                                    class: if j.grade == grade { "judge-button selected" } else { "judge-button" },
                                    title,
                                    onclick: move |_| judge(key.clone(), Judgement { grade, ..j }),
                                    "{label}"
                                }
                            }
                        };
                        let bonus_button = |delta: i32, label: &'static str, title: &'static str| {
                            let key = group.key.clone();
                            let bonus = j.bonus + delta;
                            rsx! {
                                button {
                                    class: "judge-button",
                                    title,
                                    disabled: bonus.abs() > MAX_BONUS,
                                    onclick: move |_| judge(key.clone(), Judgement { bonus, ..j }),
                                    "{label}"
                                }
                            }
                        };
                        rsx! {
                            li { key: "{group.key}", class,
                                div { class: "group-text",
                                    span { class: "strong", "{group.text}" }
                                    span { class: "muted small", {group.teams.join(", ")} }
                                }
                                div { class: "judge",
                                    {grade_button(Grade::Full, "✓", "Full points")}
                                    {grade_button(Grade::Half, "½", "Half points")}
                                    {grade_button(Grade::Wrong, "✗", "No points")}
                                    {bonus_button(-BONUS_STEP, "−", "Malus")}
                                    span { class: "bonus", "{bonus}" }
                                    {bonus_button(BONUS_STEP, "+", "Bonus")}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Points added or removed per click on the bonus buttons.
const BONUS_STEP: i32 = 100;

/// The right number and the guesses, closest first.
#[component]
fn NumberReveal(answer: f64, guesses: Vec<Guess>) -> Element {
    rsx! {
        div { class: "open-reveal",
            p { class: "number-answer", {format_number(answer)} }
            if guesses.is_empty() {
                p { class: "muted", "Nobody answered." }
            }
            ol { class: "guesses",
                for (i, guess) in guesses.into_iter().enumerate() {
                    li { key: "{i}", class: if guess.closest { "guess closest" } else { "guess" },
                        span { class: "strong", "{guess.team}" }
                        span { {format_number(guess.value)} }
                        span { class: "muted small", "off by {format_number((guess.value - answer).abs())}" }
                    }
                }
            }
        }
    }
}

/// A number without float noise: at most three decimals, no trailing zeros.
fn format_number(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
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
                RevealBody { question: question.clone(), reveal }
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
        RevealBody { question: question.clone(), reveal }
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
