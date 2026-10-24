//! Pairs questions: the items to match.

use dioxus::prelude::*;

use super::Draft;
use crate::ui::{
    admin::AdminToken,
    widgets::{Icon, IconKind, media_url, message},
};
use crate::{
    api::upload_question_image,
    model::{MAX_ANSWERS, MIN_ANSWERS, Pair, PairItem},
};

/// The pairs of a pairs question, each matched as it should be.
#[component]
pub(super) fn PairsEditor(question: usize, pairs: Vec<Pair>) -> Element {
    let draft: Draft = use_context();
    let count = pairs.len();
    rsx! {
        ul { class: "answers-edit",
            for (i, pair) in pairs.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit pair-edit",
                    span { class: "order-rank", "{i + 1}" }
                    input {
                        class: "grow",
                        placeholder: "Left, e.g. France",
                        aria_label: "Left item {i + 1}",
                        value: "{pair.left.text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_pairs(question, |pairs| pairs[i].left.text = value);
                        },
                    }
                    PairImage { question, pair: i, right: false, image: pair.left.image.clone() }
                    span { class: "muted", aria_hidden: "true", "↔" }
                    input {
                        class: "grow",
                        placeholder: "Right, e.g. Paris",
                        aria_label: "Right item {i + 1}",
                        value: "{pair.right.text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_pairs(question, |pairs| pairs[i].right.text = value);
                        },
                    }
                    PairImage { question, pair: i, right: true, image: pair.right.image.clone() }
                    button {
                        class: "button ghost icon danger-icon",
                        title: "Remove this pair",
                        aria_label: "Remove this pair",
                        disabled: count <= MIN_ANSWERS,
                        onclick: move |_| draft.edit_pairs(question, |pairs| {
                            pairs.remove(i);
                        }),
                        Icon { kind: IconKind::Trash }
                    }
                }
            }
        }
        div { class: "row between",
            if count < MAX_ANSWERS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_pairs(question, |pairs| pairs.push(Pair::default())),
                    "＋ Add pair"
                }
            } else {
                span {}
            }
            span { class: "muted small", "Write each pair matched; phones shuffle the right column. Each side takes a text, a picture or both." }
        }
    }
}

/// One side of pair `pair`, if it still exists.
fn pair_side(pairs: &mut [Pair], pair: usize, right: bool) -> Option<&mut PairItem> {
    pairs
        .get_mut(pair)
        .map(|p| if right { &mut p.right } else { &mut p.left })
}

/// The picture of one side of a pair: upload, thumbnail and removal.
#[component]
fn PairImage(question: usize, pair: usize, right: bool, image: Option<String>) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        upload_error.set(None);
        match upload_question_image(token.get(), file.into()).await {
            // Pairs may have been removed while uploading: only set a pair that still exists.
            Ok(name) => draft.edit_pairs(question, |pairs| {
                if let Some(item) = pair_side(pairs, pair, right) {
                    item.image = Some(name);
                }
            }),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    match image {
        Some(image) => rsx! {
            img { class: "answer-thumb", src: media_url(&image), alt: "Picture for this item" }
            button {
                class: "button ghost icon",
                title: "Remove picture",
                onclick: move |_| draft.edit_pairs(question, |pairs| {
                    if let Some(item) = pair_side(pairs, pair, right) {
                        item.image = None;
                    }
                }),
                "✕🖼"
            }
        },
        None => rsx! {
            label {
                class: "button ghost icon",
                title: upload_error().unwrap_or_else(|| "Add a picture for this item".to_owned()),
                if uploading() { "…" } else if upload_error().is_some() { "⚠" } else { "🖼" }
                input {
                    r#type: "file",
                    accept: "image/*",
                    class: "visually-hidden",
                    onchange: upload,
                }
            }
        },
    }
}
