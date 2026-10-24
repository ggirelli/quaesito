//! The lobby on the phone: the team's name, picture and background.

use dioxus::prelude::*;

use crate::ui::widgets::{
    ErrorBox, MASCOT_PICTURES, MascotArt, TeamAvatar, backdrop_class, message,
};
use crate::{
    api::{pictures_from_here, rename_my_team, set_team_backdrop, set_team_mascot, upload_avatar},
    protocol::{BACKDROP_COUNT, MASCOTS, MAX_NAME_CHARS, TeamInfo},
};

#[component]
pub(super) fn LobbyView(
    code: String,
    token: String,
    team: TeamInfo,
    teams: usize,
    /// Whether the game takes team pictures.
    pictures: bool,
) -> Element {
    // Only phones on the quiz's own network may upload one; others still pick a ready-made one.
    let from_here = use_resource(|| async { pictures_from_here().await.unwrap_or(false) });
    let pictures = pictures && from_here().unwrap_or(false);
    let code = use_signal(|| code);
    let token = use_signal(|| token);
    let mut uploading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let upload = move |evt: FormEvent| async move {
        let Some(file) = evt.files().into_iter().next() else {
            return;
        };
        uploading.set(true);
        error.set(None);
        if let Err(err) = upload_avatar(code(), token(), file.into()).await {
            error.set(Some(message(&err)));
        }
        uploading.set(false);
    };
    let pick = move |backdrop: Option<u8>| async move {
        error.set(None);
        if let Err(err) = set_team_backdrop(code(), token(), backdrop).await {
            error.set(Some(message(&err)));
        }
    };
    let pick_mascot = move |mascot: Option<String>| async move {
        error.set(None);
        if let Err(err) = set_team_mascot(code(), token(), mascot).await {
            error.set(Some(message(&err)));
        }
    };
    // Bumped when a new name is refused, so the field shows the team's name again.
    let mut refusals = use_signal(|| 0u32);
    let rename = move |e: FormEvent| async move {
        error.set(None);
        if let Err(err) = rename_my_team(code(), token(), e.value()).await {
            error.set(Some(message(&err)));
            refusals += 1;
        }
    };
    let backdrop = backdrop_class(team.backdrop);
    rsx! {
        main { class: "player-main",
            h1 { class: "status-title", "You're in!" }
            p { class: "muted",
                if teams == 1 { "1 team has joined." } else { "{teams} teams have joined." }
                br {}
                "Watch the big screen, the quiz starts soon."
            }
            // The team's look, folded away until opened.
            details { class: "team-look",
                summary {
                    TeamAvatar { team: team.clone(), class: "large" }
                    span { class: "button ghost", "Change your name or picture" }
                }
                label { class: "field team-name-field",
                    span { "Team name" }
                    input {
                        key: "{team.name}-{refusals}",
                        value: "{team.name}",
                        maxlength: MAX_NAME_CHARS,
                        onchange: rename,
                    }
                }
                if pictures {
                    label { class: "avatar-picker",
                        span { class: "button ghost",
                            if uploading() {
                                "Uploading…"
                            } else {
                                "Upload your own picture"
                            }
                        }
                        input {
                            r#type: "file",
                            accept: "image/*",
                            class: "visually-hidden",
                            onchange: upload,
                        }
                    }
                }
                div {
                    class: "mascot-picker",
                    role: "group",
                    aria_label: if pictures { "Or pick one of ours" } else { "Pick a team picture" },
                    for (name , picture) in MASCOTS.iter().zip(MASCOT_PICTURES) {
                        {
                            let chosen = team.mascot.as_deref() == Some(*name);
                            rsx! {
                                button {
                                    class: "mascot-choice",
                                    class: if chosen { "selected" },
                                    aria_pressed: chosen,
                                    title: "{name}",
                                    onclick: move |_| pick_mascot((!chosen).then(|| (*name).to_owned())),
                                    MascotArt { class: "avatar {backdrop}", picture, alt: "{name}" }
                                }
                            }
                        }
                    }
                }
                div {
                    class: "backdrop-picker",
                    role: "group",
                    aria_label: "Background of your team picture",
                    button {
                        class: "backdrop-swatch default",
                        class: if team.backdrop.is_none() { "selected" },
                        aria_pressed: team.backdrop.is_none(),
                        title: "No background",
                        onclick: move |_| pick(None),
                    }
                    for index in 0..BACKDROP_COUNT {
                        {
                            rsx! {
                                button {
                                    class: "backdrop-swatch {backdrop_class(Some(index))}",
                                    class: if team.backdrop == Some(index) { "selected" },
                                    aria_pressed: team.backdrop == Some(index),
                                    title: "Background {index + 1}",
                                    onclick: move |_| pick(Some(index)),
                                }
                            }
                        }
                    }
                }
                p { class: "muted small backdrop-hint",
                    if pictures {
                        "Upload a picture or pick one of ours, then a background: it shows behind your initial, and through the see-through parts of a picture."
                    } else {
                        "Pick a picture and a background for your team."
                    }
                }
                button {
                    class: "button primary",
                    // Fold the team picture away again.
                    onclick: move |_| {
                        document::eval(
                            "document.querySelector('details.team-look')?.removeAttribute('open')",
                        );
                    },
                    "✓ Done"
                }
            }
            ErrorBox { error: error() }
        }
    }
}
