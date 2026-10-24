//! The bonus question of a question.

use dioxus::prelude::*;

use super::Draft;
use super::answers::{AcceptedAnswers, Choice};
use crate::model::{BONUS_PERCENTS, Bonus};

/// The `part` of [`AcceptedAnswers`] that stands for a question's bonus question.
pub(super) const BONUS_PART: usize = usize::MAX;

/// A question's optional bonus question: its text, what it is worth and its accepted answers.
#[component]
pub(super) fn BonusEditor(question: usize, bonus: Option<Bonus>) -> Element {
    let draft: Draft = use_context();
    let Some(bonus) = bonus else {
        return rsx! {
            button {
                class: "button ghost add-bonus",
                title: "A typed question asked together with this one, worth a share of its points",
                onclick: move |_| draft.edit_question(question, |q| {
                    q.bonus = Some(Bonus {
                        accepted: vec![String::new()],
                        ..Bonus::default()
                    });
                }),
                "＋ Add a bonus question"
            }
        };
    };
    let selected = BONUS_PERCENTS
        .iter()
        .position(|&p| p == bonus.percent)
        .unwrap_or(1);
    rsx! {
        fieldset { class: "bonus-edit stack",
            legend { "Bonus question" }
            div { class: "row",
                input {
                    class: "grow",
                    placeholder: "e.g. Who sings it?",
                    value: "{bonus.text}",
                    oninput: move |e| {
                        let text = e.value();
                        draft.edit_question(question, |q| {
                            if let Some(bonus) = &mut q.bonus {
                                bonus.text = text;
                            }
                        });
                    },
                }
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_question(question, |q| q.bonus = None),
                    "Remove bonus"
                }
            }
            Choice {
                label: "Worth",
                options: vec![
                    ("+25%", "A right bonus answer adds a quarter of the points the question can earn."),
                    ("+50%", "A right bonus answer adds half the points the question can earn."),
                    ("+100%", "A right bonus answer doubles the points the question can earn."),
                ],
                selected,
                on_select: move |i: usize| draft.edit_question(question, |q| {
                    if let Some(bonus) = &mut q.bonus {
                        bonus.percent = BONUS_PERCENTS[i];
                    }
                }),
            }
            AcceptedAnswers { question, part: BONUS_PART, accepted: bonus.accepted }
        }
    }
}
