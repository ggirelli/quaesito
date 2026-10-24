//! How the team did on a question.

use dioxus::prelude::*;

use crate::protocol::AnswerOutcome;

#[component]
pub(super) fn OutcomeView(
    index: usize,
    total: usize,
    outcome: AnswerOutcome,
    answered: bool,
) -> Element {
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
