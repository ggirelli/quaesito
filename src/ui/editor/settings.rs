//! The quiz settings card and the Wi-Fi slide card.

use dioxus::prelude::*;

use super::Draft;
use super::answers::Choice;
use crate::model::{
    AdvanceMode, MAX_NETWORK_NAME_BYTES, MAX_NETWORK_PASSWORD_CHARS, MAX_TIME_LIMIT_SECS,
    MIN_TIME_LIMIT_SECS, MultiScoring, NetworkSlide, PhoneDisplay, PointsMode, QuizSettings,
    RevealMode,
};
use crate::ui::widgets::{Icon, IconKind};

#[component]
pub(super) fn SettingsCard(settings: QuizSettings) -> Element {
    let draft: Draft = use_context();
    rsx! {
        section { class: "card stack",
            h2 { "Settings" }
            label { class: "field inline beside",
                span { "Default time per question (seconds)" }
                input {
                    r#type: "number",
                    class: "short",
                    min: MIN_TIME_LIMIT_SECS,
                    max: MAX_TIME_LIMIT_SECS,
                    value: settings.default_time_limit_secs,
                    oninput: move |e| {
                        if let Ok(secs) = e.value().parse() {
                            draft.edit_settings(|s| s.default_time_limit_secs = secs);
                        }
                    },
                }
            }
            div { class: "settings-grid",
                Choice {
                    label: "Reveal answers",
                    options: vec![
                        ("After each question", "Show the correct answer and standings after every question."),
                        ("At the end", "Keep everyone guessing; walk through the answers before the podium."),
                    ],
                    selected: usize::from(settings.reveal == RevealMode::AtEnd),
                    on_select: move |i| draft.edit_settings(|s| s.reveal = if i == 0 { RevealMode::AfterEachQuestion } else { RevealMode::AtEnd }),
                }
                Choice {
                    label: "Several correct answers",
                    options: vec![
                        ("All or nothing", "Points only for picking exactly the correct answers."),
                        ("Partial credit", "Credit for each correct pick, minus wrong picks."),
                    ],
                    selected: usize::from(settings.multi_scoring == MultiScoring::Partial),
                    on_select: move |i| draft.edit_settings(|s| s.multi_scoring = if i == 0 { MultiScoring::AllOrNothing } else { MultiScoring::Partial }),
                }
                Choice {
                    label: "Points",
                    options: vec![
                        ("Speed", "Faster correct answers earn more: 1000 points when instant, down to 500 at the buzzer."),
                        ("Correct answers only", "Every correct answer earns 1000 points, however long it took. Ties go to the faster team."),
                    ],
                    selected: usize::from(settings.points == PointsMode::Flat),
                    on_select: move |i| draft.edit_settings(|s| s.points = if i == 0 { PointsMode::Speed } else { PointsMode::Flat }),
                }
                Choice {
                    label: "Jokers",
                    options: vec![
                        ("Off", "Every question counts the same."),
                        ("On", "Each team may play a joker once, while answering, to double that question's points."),
                    ],
                    selected: usize::from(settings.joker),
                    on_select: move |i| draft.edit_settings(|s| s.joker = i == 1),
                }
                Choice {
                    label: "Moving on",
                    options: vec![
                        ("Host clicks", "The host decides when to show the next question."),
                        ("Automatic", "Continue by itself after a short pause."),
                    ],
                    selected: usize::from(settings.advance == AdvanceMode::Auto),
                    on_select: move |i| draft.edit_settings(|s| s.advance = if i == 0 { AdvanceMode::Manual } else { AdvanceMode::Auto }),
                }
                Choice {
                    label: "On team phones",
                    options: vec![
                        ("Buttons only", "Read the question on the big screen; phones show coloured shapes."),
                        ("Full text", "Phones show the question and answers too."),
                    ],
                    selected: usize::from(settings.phone_display == PhoneDisplay::FullText),
                    on_select: move |i| draft.edit_settings(|s| s.phone_display = if i == 0 { PhoneDisplay::ButtonsOnly } else { PhoneDisplay::FullText }),
                }
                Choice {
                    label: "Wi-Fi slide",
                    options: vec![
                        ("Off", "The game opens on the join code."),
                        ("On", "The game opens on the Wi-Fi's name, password and a QR code that connects phones to it. Fill them in on the first card below."),
                    ],
                    selected: usize::from(settings.network.enabled),
                    on_select: move |i| draft.edit_settings(|s| s.network.enabled = i == 1),
                }
            }
        }
    }
}

/// The Wi-Fi slide that opens the game, while the setting is on: the network's name and password.
#[component]
pub(super) fn WifiSlideCard(network: NetworkSlide) -> Element {
    let draft: Draft = use_context();
    rsx! {
        section { id: "wifi-slide", class: "card stack question slide-card",
            div { class: "row between",
                h3 { "Wi-Fi slide" }
                button {
                    class: "button ghost icon danger-icon",
                    title: "Turn off the Wi-Fi slide (the name and password are kept)",
                    aria_label: "Turn off the Wi-Fi slide",
                    onclick: move |_| draft.edit_settings(|s| s.network.enabled = false),
                    Icon { kind: IconKind::Trash }
                }
            }
            p { class: "muted",
                "📶 Opens the game with the network's name, password and a QR code that connects phones to it. The join code comes next."
            }
            div { class: "row",
                label { class: "field inline",
                    span { "Network name" }
                    input {
                        r#type: "text",
                        maxlength: MAX_NETWORK_NAME_BYTES,
                        value: "{network.name}",
                        oninput: move |e| draft.edit_settings(|s| s.network.name = e.value()),
                    }
                }
                label { class: "field inline",
                    title: "Leave empty for an open network",
                    span { "Password" }
                    input {
                        r#type: "text",
                        maxlength: MAX_NETWORK_PASSWORD_CHARS,
                        value: "{network.password}",
                        oninput: move |e| draft.edit_settings(|s| s.network.password = e.value()),
                    }
                }
            }
            if let Some(problem) = network.problem() {
                p { class: "field-error", "{problem}" }
            }
        }
    }
}
