//! The panels the host opens over the game: the teams and the results.

use dioxus::prelude::*;

use crate::ui::{
    admin::AdminToken,
    live::Live,
    widgets::{ErrorBox, Spinner, TeamAvatar, message},
};
use crate::{
    api::game_results,
    protocol::{HostCommand, HostView, MAX_NAME_CHARS, TeamInfo},
};

/// Teams listed on a results slide.
pub(super) const RESULTS_SHOWN: usize = 10;

/// Every team's points on every question, with a CSV download.
#[component]
pub(super) fn ResultsPanel(code: String, on_close: EventHandler<()>) -> Element {
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
pub(super) fn TeamsPanel(
    teams: Vec<TeamInfo>,
    pictures: bool,
    joining: bool,
    on_close: EventHandler<()>,
) -> Element {
    let live: Live<HostCommand, HostView> = use_context();
    let set_joining = move |e: FormEvent| {
        live.send(HostCommand::SetJoining(e.checked()));
    };
    let set_pictures = move |e: FormEvent| {
        live.send(HostCommand::SetPictures(e.checked()));
    };
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
                label { class: "check-field",
                    input {
                        r#type: "checkbox",
                        checked: joining,
                        onchange: set_joining,
                    }
                    span { "New teams can join" }
                }
                p { class: "muted small",
                    if joining {
                        "Untick once every team is in, so nobody else can join with the code."
                    } else {
                        "Nobody else can join. Teams already in can still reconnect."
                    }
                }
                label { class: "check-field",
                    input {
                        r#type: "checkbox",
                        checked: pictures,
                        onchange: set_pictures,
                    }
                    span { "Teams can add a picture" }
                }
                p { class: "muted small",
                    if pictures {
                        "Untick to stop uploads and remove the pictures teams uploaded. Ready-made pictures and backgrounds stay."
                    } else {
                        "Teams pick a ready-made picture or show their initial, on the background they choose."
                    }
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
        if name.is_empty() {
            error.set(Some("A team needs a name."));
        } else if taken.contains(&name.to_lowercase()) {
            error.set(Some("Another team already has that name."));
        } else {
            error.set(None);
            live.send(HostCommand::RenameTeam { team: id, name });
        }
    };
    let remove = move |_| {
        live.send(HostCommand::RemoveTeam { team: id });
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
