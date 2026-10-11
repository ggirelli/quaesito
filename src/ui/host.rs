//! The big screen: lobby with QR code, questions, reveals and podium.

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    LOBBY_ART, Route,
    admin::{AdminGate, AdminToken},
    client,
    live::{ConnectionBanner, Live, countdown_ms, elapsed_now, remaining_ms, use_clock, use_live},
    sfx::{self, Cue, SFX_JS, Sfx},
    widgets::{
        ANSWER_STYLES, AnswerShape, Brand, ClientOnly, ErrorBox, Icon, IconKind, MapPin, PinMap,
        QuestionMedia, STAR, Spinner, TeamAvatar, media_url, message,
    },
};
use crate::{
    api::{game_results, host_socket, join_origin},
    model::{ImageReveal, Point},
    protocol::{
        Fastest, Grade, Guess, HostCommand, HostPhase, HostView, InputKind, Judgement,
        MAX_NAME_CHARS, PartReveal, PinGuess, QuestionInfo, Reveal, SlideInfo, StandingRow,
        TeamInfo, TextGroup, Upcoming,
    },
    scoring::MAX_BONUS,
};

/// The lobby illustration is shown while fewer teams than this have joined.
const LOBBY_ART_MAX_TEAMS: usize = 5;
/// How close the zoomed picture starts: this many times its size.
const MAX_ZOOM: f64 = 5.0;
/// Columns and rows of tiles covering a picture uncovered tile by tile.
const TILE_COLUMNS: usize = 6;
const TILE_ROWS: usize = 4;
const TILES: usize = TILE_COLUMNS * TILE_ROWS;
/// The clock ticks audibly through an open question's last seconds.
const TICK_FROM_SECS: u64 = 5;
/// How many of the closest pins the reveal lists.
const PINS_LISTED: usize = 6;
/// Seconds the host's "+" button adds to an open question.
const EXTRA_SECS: u32 = 10;
/// How many teams the between-questions leaderboard shows.
const STANDINGS_SHOWN: usize = 6;
/// Teams listed on a results slide.
const RESULTS_SHOWN: usize = 10;

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
    let now = use_clock(200);
    let mut show_teams = use_signal(|| false);
    let mut show_results = use_signal(|| false);
    let mut muted = use_signal(sfx::is_muted);
    use_sound_effects(live.view, live.received_at, now);

    let send = move |command: HostCommand| async move {
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
        let _ = live.socket.send(command).await;
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
    let paused = view.paused;
    let soundtrack = soundtrack(&view.phase, received_at, paused);
    let can_pause = !matches!(view.phase, HostPhase::Lobby | HostPhase::Ready { .. });
    let question_open = matches!(view.phase, HostPhase::Question { .. });

    rsx! {
        div {
            class: "host",
            tabindex: 0,
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            // Presentation clickers send → / PageDown and ← / PageUp. Enter and Space are left
            // alone because they also activate a focused button, which would act twice.
            onkeydown: move |e: KeyboardEvent| async move {
                let command = match e.key() {
                    Key::ArrowRight | Key::PageDown => HostCommand::Next,
                    Key::ArrowLeft | Key::PageUp => HostCommand::Back,
                    Key::Character(c) if c == "n" => HostCommand::Next,
                    Key::Character(c) if c == "b" => HostCommand::Back,
                    Key::Character(c) if c == "p" => HostCommand::TogglePause,
                    _ => return,
                };
                e.prevent_default();
                send(command).await;
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
                }
            }
            if show_teams() {
                TeamsPanel {
                    teams: view.teams.clone(),
                    on_close: move |()| show_teams.set(false),
                }
            }
            if paused {
                div { class: "paused-banner", role: "status", "⏸ Paused" }
            }
            main { class: "host-main",
                match &view.phase {
                    HostPhase::Lobby => rsx! { Lobby { code: view.code.clone(), teams: view.teams.clone() } },
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
                    HostPhase::Closed { question, answered, reveal, fastest, standings, bonus, .. } => rsx! {
                        ClosedSlide {
                            question: question.clone(),
                            answered: *answered,
                            teams: view.teams.len(),
                            reveal: reveal.clone(),
                            fastest: fastest.clone(),
                            standings: standings.clone(),
                            bonus: bonus.clone(),
                        }
                    },
                    HostPhase::Slide { slide, .. } => rsx! {
                        SlideView { slide: slide.clone() }
                    },
                    HostPhase::Standings { title, standings, .. } => rsx! {
                        div { class: "slide results-slide",
                            if !title.is_empty() {
                                h1 { class: "slide-title", "{title}" }
                            }
                            Standings { rows: standings.iter().take(RESULTS_SHOWN).cloned().collect::<Vec<_>>() }
                        }
                    },
                    HostPhase::Review { question, reveal, fastest, bonus, .. } => rsx! {
                        ReviewSlide {
                            question: question.clone(),
                            reveal: reveal.clone(),
                            fastest: fastest.clone(),
                            bonus: bonus.clone(),
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
                        disabled: matches!(view.phase, HostPhase::Lobby),
                        onclick: move |_| send(HostCommand::Back),
                        "◀ Back"
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
                if let Some((index, name, started_at)) = soundtrack {
                    Soundtrack { key: "{index}", name, started_at, paused }
                }
                if let Some(secs) = auto_left {
                    span { class: "auto-note", "Continuing in {secs}s" }
                }
                if let Some(label) = next_label(view) {
                    button {
                        class: "button primary big",
                        disabled: matches!(view.phase, HostPhase::Lobby) && view.teams.is_empty(),
                        onclick: move |_| send(HostCommand::Next),
                        "{label}"
                    }
                } else {
                    button {
                        class: "button primary big",
                        onclick: move |_| show_results.set(true),
                        "📊 Results"
                    }
                    Link { class: "button big", to: Route::Admin {}, "Back to admin" }
                }
            }
            if show_results() {
                ResultsPanel { code: code(), on_close: move |()| show_results.set(false) }
            }
            ConnectionBanner { connected: (live.connected)() }
        }
    }
}

/// Plays sound effects as the quiz moves on, and ticks through an open question's last seconds
/// (unless its soundtrack is playing).
fn use_sound_effects(view: Signal<Option<HostView>>, received_at: Signal<f64>, now: Signal<f64>) {
    let mut last_cue = use_signal(|| None::<Cue>);
    use_effect(move || {
        let view = view.read();
        let Some(view) = view.as_ref() else {
            return;
        };
        let cue = Cue::of(view);
        let previous = last_cue.peek().clone();
        if previous.as_ref() == Some(&cue) {
            return;
        }
        if let Some(sound) = previous.and_then(|p| cue.sound_after(&p)) {
            sfx::play(sound);
        }
        last_cue.set(Some(cue));
    });
    let mut last_tick = use_signal(|| None::<(usize, u64)>);
    use_effect(move || {
        let now = now();
        let view = view.read();
        let Some(HostView {
            phase:
                HostPhase::Question {
                    question,
                    remaining_ms: left,
                    ..
                },
            paused: false,
            ..
        }) = view.as_ref()
        else {
            return;
        };
        if question.audio.is_some() {
            return;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few seconds, never negative"
        )]
        let secs = (countdown_ms(*left, *received_at.peek(), now, false) / 1000.0).ceil() as u64;
        let tick = (question.index, secs);
        if (1..=TICK_FROM_SECS).contains(&secs) && *last_tick.peek() != Some(tick) {
            last_tick.set(Some(tick));
            sfx::play(if secs == 1 { Sfx::LastTick } else { Sfx::Tick });
        }
    });
}

/// Every team's points on every question, with a CSV download.
#[component]
fn ResultsPanel(code: String, on_close: EventHandler<()>) -> Element {
    let token: AdminToken = use_context();
    let code = use_signal(|| code);
    let results = use_resource(move || async move { game_results(token.get(), code()).await });
    let body = match results() {
        None => rsx! { Spinner {} },
        Some(Err(err)) => rsx! { ErrorBox { error: Some(message(&err)) } },
        Some(Ok(results)) => {
            let csv = format!(
                "data:text/csv;charset=utf-8,{}",
                percent_encode(&results.to_csv())
            );
            let file = format!("{}.csv", file_stem(&results.title));
            rsx! {
                div { class: "results-scroll",
                    table { class: "results",
                        thead {
                            tr {
                                th { "#" }
                                th { "Team" }
                                th { "Total" }
                                for (i, question) in results.questions.iter().enumerate() {
                                    th { key: "{i}", title: "{question}", "Q{i + 1}" }
                                }
                            }
                        }
                        tbody {
                            for row in results.rows.iter() {
                                tr { key: "{row.rank}-{row.team}",
                                    td { "{row.rank}" }
                                    td { class: "strong", "{row.team}" }
                                    td { class: "total", "{row.score}" }
                                    for (i, points) in row.points.iter().enumerate() {
                                        td { key: "{i}",
                                            match points {
                                                Some(p) => p.to_string(),
                                                None => "–".to_owned(),
                                            }
                                            if row.jokers.get(i).copied().unwrap_or(false) {
                                                " 🃏"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "row",
                    a { class: "button primary", href: csv, download: file, "Download CSV" }
                    button {
                        class: "button ghost",
                        onclick: move |_| {
                            document::eval("window.print()");
                        },
                        "Print"
                    }
                }
            }
        }
    };
    rsx! {
        div {
            class: "overlay results-overlay",
            onclick: move |_| on_close.call(()),
            onkeydown: move |e: KeyboardEvent| {
                e.stop_propagation();
                if e.key() == Key::Escape {
                    on_close.call(());
                }
            },
            section {
                class: "card stack panel wide-panel",
                role: "dialog",
                aria_label: "Results",
                onclick: move |e| e.stop_propagation(),
                div { class: "row between",
                    h2 { "Results" }
                    button { class: "button ghost no-print", onclick: move |_| on_close.call(()), "Close" }
                }
                {body}
            }
        }
    }
}

/// Encodes text for a `data:` URL.
fn percent_encode(text: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// A safe file name from a quiz title.
fn file_stem(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "results".to_owned()
    } else {
        format!("{stem}-results")
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

/// The soundtrack to keep on screen: question index, media name, and the browser time its
/// clock started (pauses excluded), once it has.
fn soundtrack(
    phase: &HostPhase,
    received_at: f64,
    paused: bool,
) -> Option<(usize, String, Option<f64>)> {
    let (question, started_at) = match phase {
        HostPhase::Question {
            question,
            elapsed_ms,
            ..
        } => {
            #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
            let elapsed = *elapsed_ms as f64;
            (question, (!paused).then_some(received_at - elapsed))
        }
        // Ready preloads the music; Closed keeps it going while the answers are shown.
        HostPhase::Ready { question } | HostPhase::Closed { question, .. } => (question, None),
        _ => return None,
    };
    let name = question.audio.clone()?;
    Some((question.index, name, started_at))
}

/// Plays a question's soundtrack once its clock has started, from the matching position, and
/// pauses it with the clock.
#[component]
fn Soundtrack(name: String, started_at: Option<f64>, paused: bool) -> Element {
    let mut played = use_signal(|| false);
    use_effect(use_reactive!(|started_at, paused| {
        if paused {
            if *played.peek() {
                played.set(false);
                document::eval(
                    "const a = document.getElementById('soundtrack'); if (a) { a.pause(); }",
                );
            }
            return;
        }
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
            span { class: "muted",
                "Question {question.number} of {question.total}"
                if question.multiplier > 1 {
                    span { class: "multiplier", "×{question.multiplier} points" }
                }
            }
            {children}
        }
        h1 { class: "question-title", "{question.text}" }
        if let Some((text, percent)) = &question.bonus {
            p { class: "bonus-line",
                strong { "Bonus +{percent}%: " }
                "{text}"
            }
        }
        if question.multi {
            p { class: "muted", style: "text-align:center;margin:0", "Several answers are correct" }
        }
    }
}

#[component]
fn QuestionSlide(
    question: QuestionInfo,
    left_ms: f64,
    elapsed_ms: f64,
    answered: usize,
    teams: usize,
    paused: bool,
) -> Element {
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let limit = question.time_limit_ms as f64;
    let fraction = if limit == 0.0 { 0.0 } else { left_ms / limit };
    // The picture's reveal follows the time spent, not the time left, so extra time from the
    // host never runs it backwards. It completes `image_hold_ms` before the original limit.
    #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
    let reveal_span = limit - question.image_hold_ms as f64;
    let reveal_left = if reveal_span <= 0.0 {
        0.0
    } else {
        1.0 - elapsed_ms / reveal_span
    };
    let seconds = (left_ms / 1000.0).ceil();
    rsx! {
        QuestionHeader { question: question.clone(),
            Timer { fraction, seconds }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                QuestionImage { image: image.clone(), reveal: question.image_reveal, left: reveal_left, seed: question.index, paused }
            }
            div { class: "answered",
                strong { "{answered}/{teams}" }
                span { class: "muted", "answered" }
            }
        }
        AnswersOrPrompt { question: question.clone() }
    }
}

/// A question's picture, revealed as the clock runs: `left` is the share of the clock left (1
/// when it starts, 0 at the buzzer) and `seed` varies the order tiles drop away.
#[component]
fn QuestionImage(
    image: String,
    reveal: ImageReveal,
    left: f64,
    seed: usize,
    #[props(default)] paused: bool,
) -> Element {
    let left = left.clamp(0.0, 1.0);
    match reveal {
        ImageReveal::Shown => rsx! {
            QuestionMedia { name: image, class: "question-image", autoplay: true, paused }
        },
        ImageReveal::Zoom => {
            let scale = 1.0 + (MAX_ZOOM - 1.0) * left;
            rsx! {
                div { class: "reveal-frame",
                    QuestionMedia {
                        name: image,
                        class: "question-image zoomed",
                        style: "transform: scale({scale:.3})",
                        autoplay: true,
                        paused,
                    }
                }
            }
        }
        ImageReveal::Tiles => {
            let gone = tiles_gone(left);
            let order = tile_order(seed);
            rsx! {
                div { class: "reveal-frame",
                    QuestionMedia { name: image, class: "question-image", autoplay: true, paused }
                    div { class: "tiles", aria_hidden: "true",
                        for (i, rank) in order.into_iter().enumerate() {
                            div { key: "{i}", class: if rank < gone { "tile gone" } else { "tile" } }
                        }
                    }
                }
            }
        }
    }
}

/// How many tiles have dropped away with `left` of the clock to go: an even pace, all gone at
/// the buzzer.
fn tiles_gone(left: f64) -> usize {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a handful of tiles, `left` within [0, 1]"
    )]
    let gone = ((1.0 - left) * TILES as f64).floor() as usize;
    gone.min(TILES)
}

/// A picture that was being revealed, shown whole once the question closes (it may close before
/// the reveal completes, when every team has answered).
#[component]
fn FullPicture(question: QuestionInfo) -> Element {
    match (&question.image, question.image_reveal) {
        (Some(image), ImageReveal::Zoom | ImageReveal::Tiles) => rsx! {
            QuestionMedia { name: image.clone(), class: "question-image revealed-picture" }
        },
        _ => rsx! {},
    }
}

/// For each tile, its turn to drop away: a shuffle of `0..TILES` that depends on `seed` only.
fn tile_order(seed: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..TILES).collect();
    // A small linear congruential generator is plenty to scatter a few tiles.
    let mut state = (seed as u64).wrapping_mul(6_364_136_223_846_793_005) ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..TILES).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = usize::try_from((state >> 33) % (i as u64 + 1)).unwrap_or(0);
        order.swap(i, j);
    }
    order
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
                QuestionMedia { name: image.clone(), class: "slide-image", autoplay: true }
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
                QuestionImage { image: image.clone(), reveal: question.image_reveal, left: 1.0, seed: question.index }
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
fn AnswerGrid(
    answers: Vec<String>,
    images: Vec<Option<String>>,
    reveal: Option<(Vec<usize>, Vec<u32>)>,
) -> Element {
    rsx! {
        div { class: "answer-grid",
            for (i, text) in answers.iter().enumerate() {
                {
                    let mut class = format!("answer-tile a{}", i % ANSWER_STYLES);
                    if let Some((correct, _)) = &reveal {
                        class.push_str(if correct.contains(&i) { " right" } else { " dim" });
                    }
                    let picks = reveal.as_ref().and_then(|(_, picks)| picks.get(i).copied());
                    let image = images.get(i).cloned().flatten();
                    if image.is_some() {
                        class.push_str(" has-image");
                    }
                    rsx! {
                        div { key: "{i}", class,
                            AnswerShape { index: i }
                            if let Some(image) = image {
                                img { class: "tile-image", src: media_url(&image), alt: "{text}" }
                            }
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
                AnswerGrid { answers: question.answers.clone(), images: question.answer_images.clone(), reveal: None }
            };
        }
        InputKind::Order => {
            return rsx! {
                div { class: "open-reveal",
                    p { class: "open-prompt", "🔀 Put them in order on your phone" }
                    ul { class: "order-items",
                        for (i, item) in question.answers.iter().enumerate() {
                            li { key: "{i}", class: "order-item", "{item}" }
                        }
                    }
                }
            };
        }
        InputKind::Parts => {
            return rsx! {
                div { class: "open-reveal",
                    p { class: "open-prompt", "✍️ Type the answers on your phone" }
                    PartCards { labels: question.answers.clone(), images: question.answer_images.clone() }
                }
            };
        }
        InputKind::Pin => "📍 Drop a pin on your phone",
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
            AnswerGrid {
                answers: question.answers.clone(),
                images: question.answer_images.clone(),
                reveal: Some((correct, picks)),
            }
        },
        Reveal::Text { accepted, groups } => rsx! {
            TextReveal { question: question.index, accepted, groups }
        },
        Reveal::Pin {
            answer,
            aspect,
            pins,
        } => rsx! {
            PinReveal { image: question.image.clone().unwrap_or_default(), answer, aspect, pins }
        },
        Reveal::Parts { parts } => rsx! {
            div { class: "parts-reveal",
                for (i, part) in parts.into_iter().enumerate() {
                    section { key: "{i}", class: "part-reveal",
                        h3 {
                            span { class: "order-rank", "{i + 1}" }
                            "{part.label}"
                        }
                        TextReveal { question: question.index, accepted: part.accepted, groups: part.groups }
                    }
                }
            }
        },
        Reveal::Order { items, placed } => rsx! {
            div { class: "open-reveal",
                ol { class: "order-items",
                    for (i, item) in items.into_iter().enumerate() {
                        li { key: "{i}", class: "order-item revealed",
                            span { class: "order-rank", "{i + 1}" }
                            span { class: "strong", "{item}" }
                            span { class: "muted small",
                                {
                                    let n = placed.get(i).copied().unwrap_or(0);
                                    if n == 1 { "1 team got it".to_owned() } else { format!("{n} teams got it") }
                                }
                            }
                        }
                    }
                }
            }
        },
        Reveal::Number { answer, guesses } => rsx! {
            NumberReveal { answer, guesses }
        },
    }
}

/// Every team's pin and the right spot, with the closest pins listed.
#[component]
fn PinReveal(image: String, answer: Point, aspect: f64, pins: Vec<PinGuess>) -> Element {
    let mut marks: Vec<MapPin> = pins
        .iter()
        .map(|pin| MapPin {
            at: pin.at,
            label: pin
                .team
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default(),
            title: pin.team.clone(),
            class: "",
        })
        .collect();
    marks.push(MapPin {
        at: answer,
        label: STAR.to_owned(),
        title: "The right spot".to_owned(),
        class: "answer",
    });
    rsx! {
        div { class: "pin-reveal",
            PinMap { image, aspect, pins: marks }
            if pins.is_empty() {
                p { class: "muted", "Nobody dropped a pin." }
            }
            ol { class: "guesses",
                for (i, pin) in pins.into_iter().take(PINS_LISTED).enumerate() {
                    // The trophy goes to the closest pin only.
                    li { key: "{i}", class: if i == 0 && pin.points > 0 { "guess closest" } else { "guess" },
                        span { class: "strong", "{pin.team}" }
                        span { class: "muted small", title: "Distance, as a share of the picture's width", "{pin.distance * 100.0:.0}% away" }
                        span { "+{pin.points}" }
                    }
                }
            }
        }
    }
}

/// The parts of a several-answers question while teams answer: numbered labels, with each
/// part's picture when it has one.
#[component]
fn PartCards(labels: Vec<String>, images: Vec<Option<String>>) -> Element {
    rsx! {
        ol { class: "part-cards",
            for (i, label) in labels.into_iter().enumerate() {
                li { key: "{i}", class: "part-card",
                    if let Some(Some(image)) = images.get(i) {
                        img { src: media_url(image), alt: "" }
                    }
                    span {
                        span { class: "order-rank", "{i + 1}" }
                        "{label}"
                    }
                }
            }
        }
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
    fastest: Option<Fastest>,
    standings: Option<Vec<StandingRow>>,
    bonus: Option<PartReveal>,
) -> Element {
    match (reveal, standings) {
        (Some(reveal), Some(standings)) => rsx! {
            QuestionHeader { question: question.clone(),
                span { class: "muted", "{answered}/{teams} answered" }
            }
            div { class: "closed-layout",
                div { class: "stack",
                    FullPicture { question: question.clone() }
                    RevealBody { question: question.clone(), reveal }
                    BonusReveal { question: question.index, bonus }
                    FastestNote { fastest }
                }
                Standings { rows: standings.into_iter().take(STANDINGS_SHOWN).collect::<Vec<_>>() }
            }
        },
        _ => rsx! {
            QuestionHeader { question: question.clone() }
            FullPicture { question: question.clone() }
            p { class: "waiting-note",
                "Time's up! {answered} of {teams} teams answered."
                br {}
                "Answers are revealed at the end."
            }
        },
    }
}

#[component]
fn ReviewSlide(
    question: QuestionInfo,
    reveal: Reveal,
    fastest: Option<Fastest>,
    bonus: Option<PartReveal>,
) -> Element {
    rsx! {
        QuestionHeader { question: question.clone(),
            span { class: "muted", "The answers" }
        }
        div { class: "question-body",
            if let Some(image) = &question.image {
                QuestionMedia { name: image.clone(), class: "question-image" }
            }
        }
        RevealBody { question: question.clone(), reveal }
        BonusReveal { question: question.index, bonus }
        FastestNote { fastest }
    }
}

/// The bonus question's right answer and the teams' typed answers, to judge like typed answers.
#[component]
fn BonusReveal(question: usize, bonus: Option<PartReveal>) -> Element {
    let Some(bonus) = bonus else {
        return rsx! {};
    };
    rsx! {
        section { class: "part-reveal bonus-reveal",
            h3 { "{bonus.label}" }
            TextReveal { question, accepted: bonus.accepted, groups: bonus.groups }
        }
    }
}

/// "⚡ Fastest: Owls in 2.3 s".
#[component]
fn FastestNote(fastest: Option<Fastest>) -> Element {
    let Some(fastest) = fastest else {
        return rsx! {};
    };
    rsx! {
        p { class: "fastest",
            "⚡ Fastest: "
            strong { "{fastest.team}" }
            " in {seconds(fastest.answer_ms)}"
        }
    }
}

/// Milliseconds as "2.3 s".
fn seconds(ms: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "answer times are small")]
    let secs = ms as f64 / 1000.0;
    format!("{secs:.1} s")
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
                    if let Some(round) = row.round {
                        RankChange { rank: row.rank, previous: round.previous_rank }
                    }
                    TeamAvatar { team: row.team.clone() }
                    span { class: "name", "{row.team.name}" }
                    if let Some(round) = row.round {
                        span { class: "round",
                            if round.joker {
                                span { title: "Joker: points doubled", "🃏 " }
                            }
                            match round.points {
                                0 => String::new(),
                                p if p > 0 => format!("+{p}"),
                                p => p.to_string(),
                            }
                            if let Some(ms) = round.answer_ms {
                                span { class: "muted", " {seconds(ms)}" }
                            }
                        }
                    }
                    span { class: "score", "{row.score}" }
                }
            }
        }
    }
}

/// An arrow showing how many places a team moved with the last question.
#[component]
fn RankChange(rank: usize, previous: Option<usize>) -> Element {
    let Some(previous) = previous else {
        return rsx! {
            span { class: "rank-change" }
        };
    };
    rsx! {
        match previous.cmp(&rank) {
            std::cmp::Ordering::Greater => rsx! {
                span { class: "rank-change up", title: "Up {previous - rank}", "▲{previous - rank}" }
            },
            std::cmp::Ordering::Less => rsx! {
                span { class: "rank-change down", title: "Down {rank - previous}", "▼{rank - previous}" }
            },
            std::cmp::Ordering::Equal => rsx! {
                span { class: "rank-change same", "–" }
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_drop_away_at_an_even_pace() {
        assert_eq!(tiles_gone(1.0), 0);
        assert_eq!(tiles_gone(0.5), TILES / 2);
        assert_eq!(tiles_gone(0.0), TILES);
        assert_eq!(tiles_gone(-0.5), TILES);
    }

    #[test]
    fn tile_order_is_a_stable_shuffle_per_question() {
        let order = tile_order(3);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..TILES).collect::<Vec<_>>());
        assert_eq!(order, tile_order(3));
        assert_ne!(order, tile_order(4));
        assert_ne!(order, (0..TILES).collect::<Vec<_>>());
    }
}
