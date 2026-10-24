//! The answers of choice and typed questions.

use dioxus::prelude::*;

use super::Draft;
use crate::ui::{
    admin::AdminToken,
    widgets::{AnswerBadge, media_url, message},
};
use crate::{api::upload_question_image, model::Answer};

/// A segmented choice between a few options, each with an explanation.
#[component]
pub(super) fn Choice(
    label: &'static str,
    options: Vec<(&'static str, &'static str)>,
    selected: usize,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        fieldset { class: "choice",
            legend { "{label}" }
            div { class: "segmented",
                for (i, (name, _)) in options.iter().enumerate() {
                    button {
                        r#type: "button",
                        class: if i == selected { "selected" },
                        aria_pressed: i == selected,
                        onclick: move |_| on_select.call(i),
                        "{name}"
                    }
                }
            }
            p { class: "muted small", "{options[selected.min(options.len() - 1)].1}" }
        }
    }
}

/// The accepted answers of a typed-text question.
#[component]
pub(super) fn AcceptedAnswers(
    question: usize,
    #[props(default)] part: Option<usize>,
    accepted: Vec<String>,
) -> Element {
    let draft: Draft = use_context();
    let count = accepted.len();
    rsx! {
        ul { class: "answers-edit",
            for (i, text) in accepted.into_iter().enumerate() {
                li { key: "{i}", class: "answer-edit",
                    input {
                        class: "grow",
                        placeholder: if i == 0 { "Accepted answer" } else { "Another accepted spelling" },
                        value: "{text}",
                        oninput: move |e| {
                            let value = e.value();
                            draft.edit_accepted(question, part, |a| a[i] = value);
                        },
                    }
                    button {
                        class: "button ghost icon",
                        title: "Remove",
                        disabled: count <= 1,
                        onclick: move |_| draft.edit_accepted(question, part, |a| {
                            a.remove(i);
                        }),
                        "✕"
                    }
                }
            }
        }
        div { class: "row between",
            button {
                class: "button ghost",
                onclick: move |_| draft.edit_accepted(question, part, |a| a.push(String::new())),
                "＋ Add accepted answer"
            }
            span { class: "muted small",
                "Case, accents, punctuation and spacing are ignored; spelling must match."
            }
        }
    }
}

#[component]
pub(super) fn AnswerRow(question: usize, index: usize, answer: Answer, removable: bool) -> Element {
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
            Ok(name) => draft.edit_question(question, |q| q.answers[index].image = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    rsx! {
        li { class: if answer.correct { "answer-edit correct" } else { "answer-edit" },
            AnswerBadge { index }
            if let Some(image) = &answer.image {
                img { class: "answer-thumb", src: media_url(image), alt: "Answer picture" }
                button {
                    class: "button ghost icon",
                    title: "Remove picture",
                    onclick: move |_| draft.edit_question(question, |q| q.answers[index].image = None),
                    "✕🖼"
                }
            } else {
                label {
                    class: "button ghost icon",
                    title: upload_error().unwrap_or_else(|| "Add a picture to this answer".to_owned()),
                    if uploading() { "…" } else if upload_error().is_some() { "⚠" } else { "🖼" }
                    input {
                        r#type: "file",
                        accept: "image/*",
                        class: "visually-hidden",
                        onchange: upload,
                    }
                }
            }
            input {
                class: "grow",
                placeholder: if answer.image.is_some() { "Caption (optional)" } else { "Answer {index + 1}" },
                value: "{answer.text}",
                oninput: move |e| draft.edit_question(question, |q| q.answers[index].text = e.value()),
            }
            label { class: "correct-toggle",
                input {
                    r#type: "checkbox",
                    checked: answer.correct,
                    onchange: move |e| draft.edit_question(question, |q| q.answers[index].correct = e.checked()),
                }
                span { "Correct" }
            }
            button {
                class: "button ghost icon",
                title: "Remove answer",
                disabled: !removable,
                onclick: move |_| draft.edit_question(question, |q| {
                    q.answers.remove(index);
                }),
                "✕"
            }
        }
    }
}
