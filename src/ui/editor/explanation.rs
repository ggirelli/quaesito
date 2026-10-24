//! The explanation shown with a question's answer.

use dioxus::prelude::*;

use super::Draft;
use super::uploads::{Limits, MEDIA_ACCEPT, media_limits_title, media_too_large, too_large};
use crate::ui::{
    admin::AdminToken,
    widgets::{ErrorBox, Icon, IconKind, media_url, message},
};
use crate::{
    api::{upload_question_audio, upload_question_media},
    model::is_video,
};

/// What the big screen shows only once the answer is revealed: a text, a picture or a video.
#[component]
pub(super) fn ExplanationEditor(
    question: usize,
    text: String,
    image: Option<String>,
    audio: Option<String>,
) -> Element {
    let draft: Draft = use_context();
    let token: AdminToken = use_context();
    let limits: Limits = use_context();
    let mut opened = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        if let Some(error) = media_too_large(&file, limits) {
            upload_error.set(Some(error));
            return;
        }
        uploading.set(true);
        upload_error.set(None);
        match upload_question_media(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(question, |q| q.explanation_image = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading.set(false);
    };
    let mut uploading_audio = use_signal(|| false);
    let upload_audio = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        if let Some(error) = too_large(&file, limits.get().map(|l| l.audio), "soundtrack") {
            upload_error.set(Some(error));
            return;
        }
        uploading_audio.set(true);
        upload_error.set(None);
        match upload_question_audio(token.get(), file.into()).await {
            Ok(name) => draft.edit_question(question, |q| q.explanation_audio = Some(name)),
            Err(err) => upload_error.set(Some(message(&err))),
        }
        uploading_audio.set(false);
    };
    if text.is_empty() && image.is_none() && audio.is_none() && !opened() {
        return rsx! {
            button {
                class: "button ghost add-bonus",
                title: "An explanation, a fun fact, a picture or a soundtrack, shown on the big screen with the answer",
                onclick: move |_| opened.set(true),
                "＋ Add something to show with the answer"
            }
        };
    }
    rsx! {
        fieldset { class: "bonus-edit stack",
            legend { "Shown with the answer" }
            textarea {
                class: "question-passage-input",
                placeholder: "An explanation, a fun fact… (optional)",
                rows: 2,
                value: "{text}",
                oninput: move |e| draft.edit_question(question, |q| q.explanation = e.value()),
            }
            div { class: "question-media stack",
                if let Some(image) = &image {
                    fieldset { class: "media-box",
                        legend { if is_video(image) { "Video" } else { "Picture" } }
                        div { class: "row",
                            if is_video(image) {
                                video {
                                    class: "thumb",
                                    src: media_url(image),
                                    preload: "metadata",
                                    muted: true,
                                    controls: true,
                                }
                            } else {
                                img { class: "thumb", src: media_url(image), alt: "Picture shown with the answer" }
                            }
                            button {
                                class: "button ghost",
                                onclick: move |_| draft.edit_question(question, |q| q.explanation_image = None),
                                if is_video(image) { "Remove video" } else { "Remove picture" }
                            }
                        }
                    }
                }
                if let Some(audio) = &audio {
                    fieldset { class: "media-box",
                        legend { "Soundtrack" }
                        div { class: "row",
                            Icon { kind: IconKind::Music }
                            audio {
                                class: "audio-preview",
                                src: media_url(audio),
                                preload: "metadata",
                                controls: true,
                            }
                            button {
                                class: "button ghost",
                                onclick: move |_| draft.edit_question(question, |q| q.explanation_audio = None),
                                "Remove soundtrack"
                            }
                        }
                    }
                }
                div { class: "row",
                    if image.is_none() {
                        label { class: "button ghost",
                            title: media_limits_title(limits),
                            if uploading() { "Uploading…" } else { "＋ Add picture or video" }
                            input {
                                r#type: "file",
                                accept: MEDIA_ACCEPT,
                                class: "visually-hidden",
                                onchange: upload,
                            }
                        }
                    }
                    if audio.is_none() {
                        label { class: "button ghost",
                            title: "Music or a sound the big screen plays as it shows the answer",
                            if uploading_audio() { "Uploading…" } else { "＋ Add soundtrack" }
                            input {
                                r#type: "file",
                                accept: "audio/*",
                                class: "visually-hidden",
                                onchange: upload_audio,
                            }
                        }
                    }
                    span { class: "grow" }
                    button {
                        class: "button ghost",
                        onclick: move |_| {
                            opened.set(false);
                            draft.edit_question(question, |q| {
                                q.explanation.clear();
                                q.explanation_image = None;
                                q.explanation_audio = None;
                            });
                        },
                        "Remove"
                    }
                }
            }
            ErrorBox { error: upload_error() }
        }
    }
}
