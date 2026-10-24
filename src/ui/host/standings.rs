//! The standings between questions and the podium at the end.

use dioxus::prelude::*;

use super::{seconds, use_precise_times};
use crate::protocol::StandingRow;
use crate::ui::widgets::TeamAvatar;

/// Teams listed on a results slide.
pub(super) const RESULTS_SHOWN: usize = 10;

/// How many teams the between-questions leaderboard shows.
pub(super) const STANDINGS_SHOWN: usize = 6;

#[component]
pub(super) fn Standings(rows: Vec<StandingRow>) -> Element {
    let precise = use_precise_times();
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
                                span { class: "muted", " {seconds(ms, precise)}" }
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
pub(super) fn Podium(standings: Vec<StandingRow>, finished: bool, teams: usize) -> Element {
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
