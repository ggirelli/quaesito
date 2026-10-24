//! Order questions: the items, in the right order.

use dioxus::prelude::*;

use super::Draft;
use crate::model::{MAX_ANSWERS, MIN_ANSWERS};

/// The items of an order question, in the right order.
#[component]
pub(super) fn OrderItems(question: usize, items: Vec<String>) -> Element {
    let draft: Draft = use_context();
    let count = items.len();
    rsx! {
        ol { class: "answers-edit",
            for (i, text) in items.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit",
                    span { class: "order-rank", "{i + 1}" }
                    input {
                        class: "grow",
                        placeholder: if i == 0 { "First item" } else { "Next item" },
                        value: "{text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_items(question, |items| items[i] = value);
                        },
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move up",
                        disabled: i == 0,
                        onclick: move |_| draft.edit_items(question, |items| items.swap(i, i - 1)),
                        "↑"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Move down",
                        disabled: i + 1 == count,
                        onclick: move |_| draft.edit_items(question, |items| items.swap(i, i + 1)),
                        "↓"
                    }
                    button {
                        class: "button ghost icon",
                        title: "Remove",
                        disabled: count <= MIN_ANSWERS,
                        onclick: move |_| draft.edit_items(question, |items| {
                            items.remove(i);
                        }),
                        "✕"
                    }
                }
            }
        }
        div { class: "row between",
            if count < MAX_ANSWERS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_items(question, |items| items.push(String::new())),
                    "＋ Add item"
                }
            } else {
                span {}
            }
            span { class: "muted small", "List the items in the right order; phones show them shuffled." }
        }
    }
}
