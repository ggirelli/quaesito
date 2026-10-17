//! Sound effects on the big screen, played by `assets/js/sfx.js` (synthesised, no audio files).

use dioxus::prelude::*;

use super::client;
use crate::protocol::{HostPhase, HostView};

/// The synthesiser script; include it once on pages that play effects.
pub const SFX_JS: Asset = asset!("/assets/js/sfx.js");

/// Where the host's mute choice is remembered.
const MUTED_KEY: &str = "quaesito.muted";

/// A sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Tick,
    LastTick,
    TimeUp,
    Reveal,
    Join,
    Drumroll,
    Fanfare,
}

impl Sfx {
    fn name(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::LastTick => "lastTick",
            Self::TimeUp => "timeUp",
            Self::Reveal => "reveal",
            Self::Join => "join",
            Self::Drumroll => "drumroll",
            Self::Fanfare => "fanfare",
        }
    }
}

/// Plays an effect unless the host muted them.
pub fn play(sfx: Sfx) {
    document::eval(&format!("window.qsfx && window.qsfx('{}')", sfx.name()));
}

/// Whether the host muted sound effects.
#[must_use]
pub fn is_muted() -> bool {
    client::storage_get(MUTED_KEY).as_deref() == Some("1")
}

/// Mutes or unmutes sound effects.
pub fn set_muted(muted: bool) {
    if muted {
        client::storage_set(MUTED_KEY, "1");
    } else {
        client::storage_remove(MUTED_KEY);
    }
}

/// What the big screen shows, reduced to what decides sound effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cue {
    Lobby { teams: usize },
    Question(usize),
    Closed { index: usize, revealed: bool },
    Review(usize),
    Podium { shown: usize, finished: bool },
    Other,
}

impl Cue {
    #[must_use]
    pub fn of(view: &HostView) -> Self {
        match &view.phase {
            HostPhase::Lobby { .. } => Self::Lobby {
                teams: view.teams.len(),
            },
            HostPhase::Question { question, .. } => Self::Question(question.index),
            HostPhase::Closed {
                question, reveal, ..
            } => Self::Closed {
                index: question.index,
                revealed: reveal.is_some(),
            },
            HostPhase::Review { question, .. } => Self::Review(question.index),
            HostPhase::Podium {
                standings,
                finished,
            } => Self::Podium {
                shown: standings.len(),
                finished: *finished,
            },
            HostPhase::Network { .. }
            | HostPhase::Ready { .. }
            | HostPhase::Slide { .. }
            | HostPhase::Standings { .. } => Self::Other,
        }
    }

    /// The effect for moving from `previous` to this cue, if any. Going back plays nothing.
    #[must_use]
    pub fn sound_after(&self, previous: &Self) -> Option<Sfx> {
        match (previous, self) {
            (Self::Lobby { teams: before }, Self::Lobby { teams }) if teams > before => {
                Some(Sfx::Join)
            }
            (Self::Question(before), Self::Closed { index, revealed }) if before == index => {
                Some(if *revealed { Sfx::Reveal } else { Sfx::TimeUp })
            }
            (Self::Review(before), Self::Review(index)) if index > before => Some(Sfx::Reveal),
            (Self::Closed { .. }, Self::Review(_)) => Some(Sfx::Reveal),
            (Self::Podium { shown: before, .. }, Self::Podium { shown, finished })
                if shown > before =>
            {
                Some(if *finished {
                    Sfx::Fanfare
                } else {
                    Sfx::Drumroll
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_pick_effects_only_going_forward() {
        let closed = Cue::Closed {
            index: 2,
            revealed: true,
        };
        assert_eq!(closed.sound_after(&Cue::Question(2)), Some(Sfx::Reveal));
        let hidden = Cue::Closed {
            index: 2,
            revealed: false,
        };
        assert_eq!(hidden.sound_after(&Cue::Question(2)), Some(Sfx::TimeUp));
        // Going back from the next question's results to these ones is silent.
        assert_eq!(closed.sound_after(&Cue::Question(3)), None);
        assert_eq!(
            Cue::Lobby { teams: 3 }.sound_after(&Cue::Lobby { teams: 2 }),
            Some(Sfx::Join)
        );
        assert_eq!(
            Cue::Lobby { teams: 1 }.sound_after(&Cue::Lobby { teams: 2 }),
            None
        );
        let podium = |shown, finished| Cue::Podium { shown, finished };
        assert_eq!(
            podium(1, false).sound_after(&podium(0, false)),
            Some(Sfx::Drumroll)
        );
        assert_eq!(
            podium(3, true).sound_after(&podium(2, false)),
            Some(Sfx::Fanfare)
        );
        assert_eq!(podium(1, false).sound_after(&podium(2, false)), None);
    }
}
