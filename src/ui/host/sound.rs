//! Sound on the big screen: the effects of the game and the questions' soundtracks.

use dioxus::prelude::*;

use crate::protocol::{HostPhase, HostView};
use crate::ui::{
    client,
    live::countdown_ms,
    sfx::{self, Cue, Sfx},
    widgets::{Icon, IconKind, media_url},
};

/// The clock ticks audibly through an open question's last seconds.
const TICK_FROM_SECS: u64 = 5;

/// Plays sound effects as the quiz moves on, and ticks through an open question's last seconds
/// (unless its soundtrack is playing).
pub(super) fn use_sound_effects(
    view: Signal<Option<HostView>>,
    received_at: Signal<f64>,
    now: Signal<f64>,
) {
    let mut last_cue = use_signal(|| None::<Cue>);
    use_effect(move || {
        let view = view.read();
        let Some(view) = view.as_ref() else {
            return;
        };
        let cue = Cue::of(view);
        let previous = last_cue.peek().clone();
        if previous.as_ref() == Some(&cue) {
            return;
        }
        if let Some(sound) = previous.and_then(|p| cue.sound_after(&p)) {
            sfx::play(sound);
        }
        last_cue.set(Some(cue));
    });
    let mut last_tick = use_signal(|| None::<(usize, u64)>);
    use_effect(move || {
        let now = now();
        let view = view.read();
        let Some(HostView {
            phase:
                HostPhase::Question {
                    question,
                    remaining_ms: left,
                    ..
                },
            paused: false,
            ..
        }) = view.as_ref()
        else {
            return;
        };
        if question.audio.is_some() {
            return;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few seconds, never negative"
        )]
        let secs = (countdown_ms(*left, *received_at.peek(), now, false) / 1000.0).ceil() as u64;
        let tick = (question.index, secs);
        if (1..=TICK_FROM_SECS).contains(&secs) && *last_tick.peek() != Some(tick) {
            last_tick.set(Some(tick));
            sfx::play(if secs == 1 { Sfx::LastTick } else { Sfx::Tick });
        }
    });
}

/// A soundtrack kept on screen.
pub(super) struct Playing {
    /// Tells soundtracks apart, so a new one starts from its beginning.
    pub(super) key: String,
    pub(super) name: String,
    /// The browser time the music started (pauses excluded), once it has.
    pub(super) started_at: Option<f64>,
    /// Whether to play from `started_at`, keeping the music in time with a clock.
    pub(super) seek: bool,
}

/// The soundtrack to keep on screen: the one revealed with an answer, or else the question's.
pub(super) fn soundtrack(phase: &HostPhase, received_at: f64, paused: bool) -> Option<Playing> {
    if let HostPhase::Closed {
        question,
        explanation: Some(explanation),
        ..
    }
    | HostPhase::Review {
        question,
        explanation: Some(explanation),
        ..
    } = phase
        && let Some(name) = explanation.audio.clone()
    {
        // Plays from where it is, and on from a pause.
        return Some(Playing {
            key: format!("answer-{}", question.index),
            name,
            started_at: Some(received_at),
            seek: false,
        });
    }
    let (question, started_at) = match phase {
        HostPhase::Question {
            question,
            elapsed_ms,
            ..
        } => {
            #[allow(clippy::cast_precision_loss, reason = "time limits are small")]
            let elapsed = *elapsed_ms as f64;
            (question, (!paused).then_some(received_at - elapsed))
        }
        // Ready preloads the music; Closed keeps it going while the answers are shown.
        HostPhase::Ready { question } | HostPhase::Closed { question, .. } => (question, None),
        _ => return None,
    };
    let name = question.audio.clone()?;
    Some(Playing {
        key: question.index.to_string(),
        name,
        started_at,
        seek: true,
    })
}

/// Plays a soundtrack once it has started (a question's with its clock, from the matching
/// position), and pauses it with the game.
#[component]
pub(super) fn Soundtrack(
    name: String,
    started_at: Option<f64>,
    seek: bool,
    paused: bool,
) -> Element {
    let mut played = use_signal(|| false);
    use_effect(use_reactive!(|started_at, seek, paused| {
        if paused {
            if *played.peek() {
                played.set(false);
                document::eval(
                    "const a = document.getElementById('soundtrack'); if (a) { a.pause(); }",
                );
            }
            return;
        }
        let Some(started_at) = started_at else {
            return;
        };
        if *played.peek() {
            return;
        }
        played.set(true);
        let offset = ((client::now_ms() - started_at) / 1000.0).max(0.0);
        let seek = if seek {
            format!("a.currentTime = {offset:.3}; ")
        } else {
            String::new()
        };
        document::eval(&format!(
            "const a = document.getElementById('soundtrack');\n\
             if (a) {{ {seek}a.play().catch(() => {{}}); }}"
        ));
    }));
    // No player controls: the host starts the music with the "next" step only, which also
    // starts the clock.
    let mut playing = use_signal(|| false);
    rsx! {
        div {
            class: if playing() { "soundtrack playing" } else { "soundtrack" },
            title: if playing() { "Music playing" } else { "Soundtrack ready" },
            Icon { kind: IconKind::Music }
            span { class: "equalizer", aria_hidden: "true",
                span {}
                span {}
                span {}
            }
            audio {
                id: "soundtrack",
                src: media_url(&name),
                preload: "auto",
                onplay: move |_| playing.set(true),
                onpause: move |_| playing.set(false),
                onended: move |_| playing.set(false),
            }
        }
    }
}
