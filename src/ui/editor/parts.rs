//! Questions in parts, each with its own answer.

use dioxus::prelude::*;

use super::Draft;
use super::answers::AcceptedAnswers;
use crate::ui::{
    admin::AdminToken,
    widgets::{Icon, IconKind, media_url, message},
};
use crate::{
    api::upload_question_image,
    model::{MAX_PARTS, MIN_PARTS, Part},
};

/// A new, empty part of a several-answers question.
pub(super) fn blank_part() -> Part {
    Part {
        accepted: vec![String::new()],
        ..Part::default()
    }
}

/// The parts of a several-answers question: a label, an optional picture and accepted answers
/// each.
#[component]
pub(super) fn PartsEditor(question: usize, parts: Vec<Part>) -> Element {
    let draft: Draft = use_context();
    let count = parts.len();
    rsx! {
        div { class: "parts-edit",
            for (i, part) in parts.into_iter().enumerate() {
                div { key: "{i}", class: "part-edit stack",
                    div { class: "row",
                        span { class: "order-rank", "{i + 1}" }
                        input {
                            class: "grow",
                            placeholder: "What to type, e.g. Title, Artist, Poster {i + 1}",
                            value: "{part.label}",
                            oninput: move |e| {
                                let value = e.value();
                                draft.edit_parts(question, |parts| parts[i].label = value);
                            },
                        }
                        PartImage { question, part: i, image: part.image.clone() }
                        button {
                            class: "button ghost icon danger-icon",
                            title: "Remove this answer",
                            aria_label: "Remove this answer",
                            disabled: count <= MIN_PARTS,
                            onclick: move |_| draft.edit_parts(question, |parts| {
                                parts.remove(i);
                            }),
                            Icon { kind: IconKind::Trash }
                        }
                    }
                    AcceptedAnswers { question, part: Some(i), accepted: part.accepted.clone() }
                }
            }
        }
        div { class: "row between",
            if count < MAX_PARTS {
                button {
                    class: "button ghost",
                    onclick: move |_| draft.edit_parts(question, |parts| parts.push(blank_part())),
                    "＋ Add answer"
                }
            } else {
                span {}
            }
            span { class: "muted small", "The question's points are shared evenly between its answers." }
        }
    }
}

/// A part's own picture: upload, thumbnail and removal.
#[component]
fn PartImage(question: usize, part: usize, image: Option<String>) -> Element {
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
            // Parts may have been removed while uploading: only set a part that still exists.
            Ok(name) => draft.edit_parts(question, |parts| {
                if let Some(p) = parts.get_mut(part) {
                    p.image = Some(name);
                }
            }),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    match image {
        Some(image) => rsx! {
            img { class: "answer-thumb", src: media_url(&image), alt: "Picture for this answer" }
            button {
                class: "button ghost icon",
                title: "Remove picture",
                onclick: move |_| draft.edit_parts(question, |parts| {
                    if let Some(p) = parts.get_mut(part) {
                        p.image = None;
                    }
                }),
                "✕🖼"
            }
        },
        None => rsx! {
            label {
                class: "button ghost icon",
                title: upload_error().unwrap_or_else(|| "Add a picture for this answer".to_owned()),
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
