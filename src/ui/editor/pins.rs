//! Pin questions: where the answer is on the picture.

use dioxus::prelude::*;

use super::Draft;
use super::answers::Choice;
use crate::model::{PinTolerance, Point, QuestionKind};
use crate::ui::widgets::{MapPin, PinMap, STAR};

/// The right spot of a pin question, clicked on its picture, and how close pins must be.
#[component]
pub(super) fn PinEditor(
    question: usize,
    image: Option<String>,
    answer: Option<Point>,
    aspect: f64,
    tolerance: PinTolerance,
) -> Element {
    let draft: Draft = use_context();
    let Some(image) = image else {
        return rsx! {
            p { class: "muted", "Add the picture above (a map, a floor plan…), then click the right spot on it." }
        };
    };
    let set_tolerance = move |tolerance: PinTolerance| {
        draft.edit_question(question, |q| {
            if let QuestionKind::Pin { tolerance: t, .. } = &mut q.kind {
                *t = tolerance;
            }
        });
    };
    rsx! {
        p { class: "muted small",
            if answer.is_some() { "Click to move the right spot." } else { "Click the right spot on the picture." }
        }
        div { class: "pin-edit",
            PinMap {
                aspect,
                image,
                pins: answer
                    .map(|at| MapPin {
                        at,
                        label: STAR.to_owned(),
                        title: "The right spot".to_owned(),
                        class: "answer",
                    })
                    .into_iter()
                    .collect::<Vec<_>>(),
                on_pick: EventHandler::new(move |(at, aspect): (Point, f64)| {
                    draft.edit_question(question, |q| {
                        if let QuestionKind::Pin { answer, aspect: a, .. } = &mut q.kind {
                            *answer = Some(at);
                            *a = aspect;
                        }
                    });
                }),
            }
        }
        Choice {
            label: "How close",
            options: vec![
                ("Tight", "Full points within 2% of the picture's width, none beyond 15%."),
                ("Normal", "Full points within 4% of the picture's width, none beyond 30%."),
                ("Loose", "Full points within 8% of the picture's width, none beyond 50%."),
            ],
            selected: match tolerance {
                PinTolerance::Tight => 0,
                PinTolerance::Normal => 1,
                PinTolerance::Loose => 2,
            },
            on_select: move |i| set_tolerance(match i {
                0 => PinTolerance::Tight,
                2 => PinTolerance::Loose,
                _ => PinTolerance::Normal,
            }),
        }
    }
}
