//! The team leader's phone: join a game, answer questions, see results.

use dioxus::{fullstack::WebSocketOptions, prelude::*};

use super::{
    Route, client,
    live::{ConnectionBanner, Live, countdown_ms, use_clock, use_live},
    widgets::{ANSWER_SHAPES, Brand, ClientOnly, ErrorBox, Spinner, TeamAvatar, message},
};
use crate::{
    api::{check_ticket, game_info, join_game, player_socket, upload_avatar},
    model::{MAX_TEXT_ANSWER_CHARS, PhoneDisplay},
    protocol::{
        AnswerOutcome, InputKind, JokerState, MAX_NAME_CHARS, PlayerCommand, PlayerPhase,
        PlayerView, Response, TeamInfo,
    },
};

fn ticket_key(code: &str) -> String {
    format!("quaesito.ticket.{code}")
}

/// `/join`: asks for a game code.
#[component]
pub fn JoinBlank() -> Element {
    rsx! {
        main { class: "page center narrow stack",
            Brand { large: true }
            JoinCodeForm {}
        }
    }
}

/// A form to enter a game code.
#[component]
pub fn JoinCodeForm() -> Element {
    let mut code = use_signal(String::new);
    let nav = navigator();
    rsx! {
        form {
            class: "card stack",
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let code = code().trim().to_owned();
                if !code.is_empty() {
                    nav.push(Route::Join { code });
                }
            },
            label { class: "field",
                span { "Game code" }
                input {
                    class: "code-input",
                    inputmode: "numeric",
                    autocomplete: "off",
                    maxlength: 6,
                    placeholder: "000000",
                    value: code,
                    oninput: move |e| code.set(e.value().chars().filter(char::is_ascii_digit).collect()),
                }
            }
            button { r#type: "submit", class: "button primary big", "Join" }
        }
    }
}

/// `/join/:code`: join (or rejoin) a game and play it.
#[component]
pub fn Join(code: String) -> Element {
    rsx! {
        ClientOnly {
            PlayerEntry { key: "{code}", code }
        }
    }
}

#[component]
fn PlayerEntry(code: String) -> Element {
    let code = use_signal(|| code);
    let mut token = use_signal(|| client::storage_get(&ticket_key(&code())));
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
            client::storage_remove(&ticket_key(&code.peek()));
            token.set(None);
        }
    });
    match (token(), checked()) {
        (Some(t), Some(true)) => rsx! {
            PlayerGame {
                code: code(),
                token: t,
                on_gone: move |()| {
                    client::storage_remove(&ticket_key(&code.peek()));
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
                client::storage_set(&ticket_key(&code()), &t);
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

#[component]
fn PlayerGame(code: String, token: String, on_gone: EventHandler<()>) -> Element {
    let code = use_signal(|| code);
    let token = use_signal(|| token);
    let live = use_live(move || player_socket(code(), token(), WebSocketOptions::new()));
    use_context_provider(|| live);
    // A dropped connection may mean the host removed the team, or ended the game.
    use_effect(move || {
        if !(live.connected)() {
            spawn(async move {
                if matches!(check_ticket(code(), token()).await, Ok(false)) {
                    on_gone.call(());
                }
            });
        }
    });

    let view = live.view.read();
    let Some(view) = view.as_ref() else {
        return rsx! {
            div { class: "page center", Spinner {} }
        };
    };
    rsx! {
        div { class: "player",
            header { class: "player-bar",
                div { class: "team",
                    TeamAvatar { team: view.team.clone() }
                    span { "{view.team.name}" }
                }
                if let Some(score) = view.score {
                    span { class: "player-score", "{score} pts" }
                }
            }
            PhaseView { code: code(), token: token(), view: view.clone(), live_received: (live.received_at)() }
            ConnectionBanner { connected: (live.connected)() }
        }
    }
}

#[component]
fn PhaseView(code: String, token: String, view: PlayerView, live_received: f64) -> Element {
    match view.phase {
        PlayerPhase::Lobby { teams } => rsx! {
            LobbyView { code, token, team: view.team.clone(), teams }
        },
        PlayerPhase::Slide => rsx! {
            main { class: "player-main",
                div { class: "big-icon", "👀" }
                h1 { class: "status-title", "Eyes on the big screen" }
                p { class: "muted", "Nothing to answer right now." }
            }
        },
        PlayerPhase::Ready { index, total } => rsx! {
            main { class: "player-main",
                span { class: "muted", "Question {index + 1} of {total}" }
                div { class: "big-icon", "🎵" }
                h1 { class: "status-title", "Listen up!" }
                p { class: "muted", "Answers open when the music starts." }
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: input @ (InputKind::Text | InputKind::Number),
            text,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
            ..
        } => rsx! {
            OpenPad {
                key: "{index}",
                joker: view.joker,
                index,
                total,
                number: input == InputKind::Number,
                question: text.map(|(question, _)| question),
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted,
            }
        },
        PlayerPhase::Question {
            index,
            total,
            input: InputKind::Choice,
            answer_count,
            text,
            multi,
            remaining_ms: left,
            paused,
            time_limit_ms,
            submitted,
        } => rsx! {
            AnswerPad {
                key: "{index}",
                joker: view.joker,
                index,
                total,
                answer_count,
                text,
                multi,
                clock: Clock { left_ms: left, limit_ms: time_limit_ms, paused, received_at: live_received },
                submitted: match submitted {
                    Some(Response::Choice(selected)) => Some(selected),
                    _ => None,
                },
                display: view.display,
            }
        },
        PlayerPhase::Closed {
            index,
            total,
            submitted,
            result,
        } => match result {
            Some((outcome, _)) => rsx! {
                OutcomeView { index, total, outcome, answered: submitted.is_some() }
            },
            None => rsx! {
                main { class: "player-main",
                    span { class: "muted", "Question {index + 1} of {total}" }
                    if submitted.is_some() {
                        div { class: "big-icon", "🔒" }
                        h1 { class: "status-title", "Answer locked in" }
                        p { class: "muted", "Results are revealed at the end. Fingers crossed!" }
                    } else {
                        div { class: "big-icon", "⌛" }
                        h1 { class: "status-title", "Time's up" }
                        p { class: "muted", "No answer this time." }
                    }
                }
            },
        },
        PlayerPhase::Review {
            index,
            total,
            submitted,
            outcome,
            ..
        } => rsx! {
            OutcomeView { index, total, outcome, answered: submitted.is_some() }
        },
        PlayerPhase::Podium {
            rank,
            teams,
            finished,
        } => rsx! {
            main { class: "player-main",
                match rank {
                    Some(rank) => rsx! {
                        div { class: "big-icon",
                            match rank {
                                1 => "🏆",
                                2 => "🥈",
                                3 => "🥉",
                                _ => "🎉",
                            }
                        }
                        h1 { class: "status-title", "You finished #{rank} of {teams}" }
                    },
                    None => rsx! {
                        div { class: "big-icon", "🥁" }
                        h1 { class: "status-title", "Drumroll…" }
                        p { class: "muted", "Watch the big screen!" }
                    },
                }
                if finished {
                    p { class: "muted", "Thanks for playing!" }
                    Link { class: "button", to: Route::Home {}, "Done" }
                }
            }
        },
    }
}

#[component]
fn LobbyView(code: String, token: String, team: TeamInfo, teams: usize) -> Element {
    let code = use_signal(|| code);
    let token = use_signal(|| token);
    let mut uploading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        error.set(None);
        if let Err(err) = upload_avatar(code(), token(), file.into()).await {
            error.set(Some(message(&err)));
        }
        uploading.set(false);
    };
    rsx! {
        main { class: "player-main",
            label { class: "avatar-picker",
                TeamAvatar { team: team.clone(), class: "large" }
                span { class: "button ghost",
                    if uploading() {
                        "Uploading…"
                    } else if team.avatar.is_some() {
                        "Change team picture"
                    } else {
                        "Add a team picture"
                    }
                }
                input {
                    r#type: "file",
                    accept: "image/*",
                    class: "visually-hidden",
                    onchange: upload,
                }
            }
            ErrorBox { error: error() }
            h1 { class: "status-title", "You're in!" }
            p { class: "muted",
                if teams == 1 { "1 team has joined." } else { "{teams} teams have joined." }
                br {}
                "Watch the big screen, the quiz starts soon."
            }
        }
    }
}

/// A question's countdown as last received from the server.
#[derive(Clone, Copy, PartialEq)]
struct Clock {
    left_ms: u64,
    limit_ms: u64,
    paused: bool,
    /// Browser time the view arrived.
    received_at: f64,
}

impl Clock {
    /// Milliseconds left at browser time `now`, and the timer bar's width in percent.
    fn at(self, now: f64) -> (f64, f64) {
        let left = countdown_ms(self.left_ms, self.received_at, now, self.paused);
        #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
        let limit = self.limit_ms.max(1) as f64;
        (left, (left / limit * 100.0).clamp(0.0, 100.0))
    }
}

#[component]
fn AnswerPad(
    index: usize,
    total: usize,
    answer_count: usize,
    text: Option<(String, Vec<String>)>,
    multi: bool,
    clock: Clock,
    submitted: Option<Vec<usize>>,
    display: PhoneDisplay,
    joker: JokerState,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut selected = use_signal(Vec::<usize>::new);
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;
    let chosen = submitted.clone().unwrap_or_else(|| selected.cloned());

    let send = move |answer: Vec<usize>| async move {
        let _ = live_socket
            .socket
            .send(PlayerCommand::Answer {
                response: Response::Choice(answer),
                joker: armed(),
            })
            .await;
    };
    let (question, labels) = match text {
        Some((question, labels)) => (Some(question), Some(labels)),
        None => (None, None),
    };
    let with_text = display == PhoneDisplay::FullText && labels.is_some();

    rsx! {
        main { class: "player-main top",
            div { class: "phone-timer",
                div { style: "width: {width}%" }
            }
            div { class: "row between",
                span { class: "muted", "Question {index + 1} of {total}" }
                span { class: "muted",
                    if clock.paused { "⏸ Paused" } else { "{(left / 1000.0).ceil()}s" }
                }
            }
            if let Some(question) = question {
                p { class: "phone-question", "{question}" }
            }
            if locked {
                p { class: "strong",
                    if submitted.is_some() { "🔒 Answer locked in" } else { "⌛ Time's up" }
                }
            } else if multi {
                p { class: "muted", "Pick every correct answer, then submit." }
            }
            JokerToggle { state: joker, armed, locked }
            div { class: if with_text { "phone-answers with-text" } else { "phone-answers" },
                for i in 0..answer_count {
                    {
                        let shape = ANSWER_SHAPES[i % ANSWER_SHAPES.len()];
                        let mut class = format!("phone-answer a{}", i % ANSWER_SHAPES.len());
                        if chosen.contains(&i) {
                            class.push_str(" selected");
                        } else if locked {
                            class.push_str(" faded");
                        }
                        let label = labels.as_ref().and_then(|l| l.get(i).cloned());
                        let aria_label = label.clone().unwrap_or_else(|| format!("Answer {}", i + 1));
                        rsx! {
                            button {
                                key: "{i}",
                                class,
                                disabled: locked,
                                aria_label,
                                onclick: move |_| async move {
                                    if multi {
                                        selected.with_mut(|s| {
                                            if let Some(pos) = s.iter().position(|&x| x == i) {
                                                s.remove(pos);
                                            } else {
                                                s.push(i);
                                            }
                                        });
                                    } else {
                                        selected.set(vec![i]);
                                        send(vec![i]).await;
                                    }
                                },
                                span { class: "shape", "{shape}" }
                                if let Some(label) = label {
                                    span { "{label}" }
                                }
                            }
                        }
                    }
                }
            }
            if multi && !locked {
                button {
                    class: "button primary big",
                    disabled: selected().is_empty(),
                    onclick: move |_| async move { send(selected()).await },
                    "Submit"
                }
            }
        }
    }
}

/// A typed or numeric answer: a field and a submit button.
#[component]
fn OpenPad(
    index: usize,
    total: usize,
    number: bool,
    question: Option<String>,
    clock: Clock,
    submitted: Option<Response>,
    joker: JokerState,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let mut draft = use_signal(String::new);
    let armed = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let now = use_clock(200);
    let (left, width) = clock.at(now());
    let locked = submitted.is_some() || left <= 0.0;

    let submit = move |evt: FormEvent| async move {
        evt.prevent_default();
        let text = draft().trim().to_owned();
        let response = if number {
            // Phones with a comma as decimal separator may still send one.
            match text.replace(',', ".").parse::<f64>() {
                Ok(value) if value.is_finite() => Response::Number(value),
                _ => {
                    error.set(Some("Please enter a number.".to_owned()));
                    return;
                }
            }
        } else if text.is_empty() {
            error.set(Some("Please type an answer.".to_owned()));
            return;
        } else {
            Response::Text(text)
        };
        error.set(None);
        let _ = live_socket
            .socket
            .send(PlayerCommand::Answer {
                response,
                joker: armed(),
            })
            .await;
    };

    rsx! {
        main { class: "player-main top",
            div { class: "phone-timer",
                div { style: "width: {width}%" }
            }
            div { class: "row between",
                span { class: "muted", "Question {index + 1} of {total}" }
                span { class: "muted",
                    if clock.paused { "⏸ Paused" } else { "{(left / 1000.0).ceil()}s" }
                }
            }
            if let Some(question) = question {
                p { class: "phone-question", "{question}" }
            }
            match (&submitted, locked) {
                (Some(answer), _) => rsx! {
                    p { class: "strong", "🔒 Answer locked in" }
                    JokerToggle { state: joker, armed, locked }
                    p { class: "locked-answer",
                        match answer {
                            Response::Text(text) => text.clone(),
                            Response::Number(value) => value.to_string(),
                            Response::Choice(_) => String::new(),
                        }
                    }
                },
                (None, true) => rsx! {
                    p { class: "strong", "⌛ Time's up" }
                },
                (None, false) => rsx! {
                    form { class: "stack open-form", onsubmit: submit,
                        if number {
                            input {
                                r#type: "text",
                                inputmode: "decimal",
                                autocomplete: "off",
                                placeholder: "Your guess",
                                aria_label: "Your guess",
                                value: draft,
                                oninput: move |e| draft.set(e.value()),
                            }
                        } else {
                            input {
                                r#type: "text",
                                autocomplete: "off",
                                autocapitalize: "off",
                                spellcheck: false,
                                maxlength: MAX_TEXT_ANSWER_CHARS,
                                placeholder: "Your answer",
                                aria_label: "Your answer",
                                value: draft,
                                oninput: move |e| draft.set(e.value()),
                            }
                        }
                        ErrorBox { error: error() }
                        JokerToggle { state: joker, armed, locked }
                        button {
                            r#type: "submit",
                            class: "button primary big",
                            disabled: draft().trim().is_empty(),
                            "Submit"
                        }
                    }
                },
            }
        }
    }
}

#[component]
/// Arms the team's joker before answering, or shows that it was played on this question.
#[component]
fn JokerToggle(state: JokerState, armed: Signal<bool>, locked: bool) -> Element {
    match state {
        JokerState::Ready if !locked => rsx! {
            button {
                r#type: "button",
                class: if armed() { "joker armed" } else { "joker" },
                aria_pressed: armed(),
                onclick: move |_| armed.toggle(),
                if armed() { "🃏 Joker on: this answer counts double" } else { "🃏 Play my joker (double points)" }
            }
        },
        JokerState::PlayedHere => rsx! {
            p { class: "joker armed", "🃏 Joker played: points count double" }
        },
        _ => rsx! {},
    }
}

#[component]
fn OutcomeView(index: usize, total: usize, outcome: AnswerOutcome, answered: bool) -> Element {
    let (class, icon, title) = if outcome.fully_correct {
        ("result good", "🎉", "Correct!")
    } else if outcome.points > 0 {
        ("result good", "👍", "Partly right")
    } else if outcome.points < 0 {
        ("result bad", "😬", "Malus!")
    } else if answered {
        ("result bad", "😬", "Not this time")
    } else {
        ("result bad", "⌛", "No answer")
    };
    rsx! {
        main { class: "player-main",
            span { class: "muted", "Question {index + 1} of {total}" }
            div { class,
                div { class: "big-icon", "{icon}" }
                h1 { class: "status-title", "{title}" }
                div { class: "points",
                    if outcome.points < 0 { "{outcome.points}" } else { "+{outcome.points}" }
                }
                if outcome.joker {
                    p { class: "strong", "🃏 Joker: points doubled" }
                }
            }
        }
    }
}
