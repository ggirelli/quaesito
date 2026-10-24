//! The pad of pairs questions.

use dioxus::prelude::*;

use super::Clock;
use super::controls::{BonusField, JokerToggle};
use crate::protocol::{JokerState, PairColumns, PlayerCommand, PlayerView, Response};
use crate::ui::{
    live::{Live, use_clock},
    widgets::{AnswerBadge, PairItemContent, pair_letter},
};

/// An item tapped first on a pairs question, waiting for its partner.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PairPick {
    Left(usize),
    Right(usize),
}

/// Links left item `left` with right item `right`, unlinking whatever either was linked to.
fn link_pair(links: &mut [Option<usize>], left: usize, right: usize) {
    for link in links.iter_mut() {
        if *link == Some(right) {
            *link = None;
        }
    }
    links[left] = Some(right);
}

/// A pairs question: tap an item, then its partner in the other column; tap a matched item to
/// change its partner. Any number of pairs can be submitted.
#[component]
pub(super) fn PairsPad(
    index: usize,
    total: usize,
    columns: PairColumns,
    question: Option<String>,
    clock: Clock,
    submitted: Option<Vec<Option<usize>>>,
    joker: JokerState,
    bonus: Option<(String, u32)>,
) -> Element {
    let live_socket: Live<PlayerCommand, PlayerView> = use_context();
    let count = columns.left.len();
    let mut links = use_signal(|| vec![None::<usize>; count]);
    let mut pick = use_signal(|| None::<PairPick>);
    let bonus_text = use_signal(String::new);
    let has_bonus = bonus.is_some();
    let armed = use_signal(|| false);
    let now = use_clock(200);
    let (left_ms, width) = clock.at(now());
    let locked = submitted.is_some() || left_ms <= 0.0;
    let shown = submitted.clone().unwrap_or_else(|| links.cloned());
    let partner_of = |right: usize| shown.iter().position(|&l| l == Some(right));
    let anything = links.read().iter().any(Option::is_some);
    let mut tap = move |tapped: PairPick| {
        let current = links.cloned();
        // A matched item comes loose and waits for a new partner.
        let matched = match tapped {
            PairPick::Left(l) => current[l].is_some(),
            PairPick::Right(r) => current.contains(&Some(r)),
        };
        match (pick(), tapped) {
            (Some(PairPick::Left(l)), PairPick::Right(r))
            | (Some(PairPick::Right(r)), PairPick::Left(l)) => {
                links.with_mut(|links| link_pair(links, l, r));
                pick.set(None);
            }
            (Some(picked), _) if picked == tapped => pick.set(None),
            _ => {
                if matched {
                    links.with_mut(|links| match tapped {
                        PairPick::Left(l) => links[l] = None,
                        PairPick::Right(r) => {
                            for link in links.iter_mut() {
                                if *link == Some(r) {
                                    *link = None;
                                }
                            }
                        }
                    });
                }
                pick.set(Some(tapped));
            }
        }
    };
    let send = move |_| {
        live_socket.send(PlayerCommand::Answer {
            response: Response::Pairs(links()),
            joker: armed(),
            bonus: has_bonus.then(|| bonus_text.cloned()),
        });
    };
    rsx! {
        main { class: "player-main top",
            div { class: "phone-timer",
                div { style: "width: {width}%" }
            }
            div { class: "row between",
                span { class: "muted", "Question {index + 1} of {total}" }
                span { class: "muted",
                    if clock.paused { "⏸ Paused" } else { "{(left_ms / 1000.0).ceil()}s" }
                }
            }
            if let Some(question) = question {
                p { class: "phone-question", "{question}" }
            }
            if locked {
                p { class: "strong",
                    if submitted.is_some() { "🔒 Pairs locked in" } else { "⌛ Time's up" }
                }
            } else {
                p { class: "muted", "Tap an item, then its partner. Tap a matched item to change it." }
            }
            JokerToggle { state: joker, armed, locked }
            if let Some(bonus) = bonus.clone() {
                BonusField { bonus, value: bonus_text, locked }
            }
            div { class: "pairs-pad",
                ul { class: "pair-column",
                    for (l, item) in columns.left.iter().enumerate() {
                        li { key: "l{l}",
                            button {
                                class: if pick() == Some(PairPick::Left(l)) { "pair-choice picked" } else { "pair-choice" },
                                disabled: locked,
                                aria_pressed: pick() == Some(PairPick::Left(l)),
                                onclick: move |_| tap(PairPick::Left(l)),
                                AnswerBadge { index: l }
                                span { class: "pair-body",
                                    PairItemContent { item: item.clone() }
                                }
                                span { class: "pair-mark",
                                    match shown.get(l).copied().flatten() {
                                        Some(r) => format!("→ {}", pair_letter(r)),
                                        None => String::new(),
                                    }
                                }
                            }
                        }
                    }
                }
                ul { class: "pair-column",
                    for (r, item) in columns.right.iter().enumerate() {
                        li { key: "r{r}",
                            button {
                                class: if pick() == Some(PairPick::Right(r)) { "pair-choice picked" } else { "pair-choice" },
                                disabled: locked,
                                aria_pressed: pick() == Some(PairPick::Right(r)),
                                onclick: move |_| tap(PairPick::Right(r)),
                                span { class: "order-rank", "{pair_letter(r)}" }
                                span { class: "pair-body",
                                    PairItemContent { item: item.clone() }
                                }
                                if let Some(l) = partner_of(r) {
                                    AnswerBadge { index: l }
                                }
                            }
                        }
                    }
                }
            }
            if !locked {
                div { class: "row",
                    button {
                        class: "button ghost",
                        disabled: !anything,
                        onclick: move |_| {
                            links.set(vec![None; count]);
                            pick.set(None);
                        },
                        "Start over"
                    }
                    button {
                        class: "button primary big grow",
                        disabled: !anything,
                        onclick: send,
                        "Submit"
                    }
                }
            }
        }
    }
}
