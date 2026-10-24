//! Saving a game and restoring it, for example after the server restarts.

use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};
use web_time::Instant;

use super::{Game, Phase, Submission, Team};
use crate::{
    model::Quiz,
    protocol::{DEFAULT_MAX_TEAMS, Judgement},
};

fn default_max_teams() -> usize {
    DEFAULT_MAX_TEAMS
}

fn yes() -> bool {
    true
}

/// A saved [`Game`], with times relative to when it was saved (`Instant`s do not survive a
/// restart).
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    code: String,
    quiz: Quiz,
    teams: Vec<Team>,
    next_team_id: u32,
    /// The most teams that can join.
    #[serde(default = "default_max_teams")]
    max_teams: usize,
    /// Whether teams may upload a picture.
    #[serde(default = "yes")]
    pictures: bool,
    /// Whether new teams may join.
    #[serde(default = "yes")]
    joining: bool,
    phase: SavedPhase,
    judgements: Vec<HashMap<String, Judgement>>,
    played: Vec<bool>,
    shown: Vec<Vec<usize>>,
}

impl Snapshot {
    /// The join code of the saved game.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
}

#[derive(Serialize, Deserialize)]
enum SavedPhase {
    Network,
    Lobby,
    Ready {
        index: usize,
    },
    Question {
        index: usize,
        elapsed: Duration,
        left: Duration,
    },
    Closed {
        index: usize,
    },
    Slide {
        index: usize,
    },
    Review {
        index: usize,
    },
    Standings {
        index: usize,
    },
    Podium {
        revealed: usize,
    },
}

impl Game {
    /// Everything needed to bring the game back after a restart.
    #[must_use]
    pub fn snapshot(&self, now: Instant) -> Snapshot {
        let clock = self.clock(now);
        Snapshot {
            code: self.code.clone(),
            quiz: self.quiz.clone(),
            teams: self
                .teams
                .iter()
                .map(|t| Team {
                    id: t.id,
                    name: t.name.clone(),
                    avatar: t.avatar.clone(),
                    mascot: t.mascot.clone(),
                    backdrop: t.backdrop,
                    token: t.token.clone(),
                    connections: 0,
                    submissions: t
                        .submissions
                        .iter()
                        .map(|s| {
                            s.as_ref().map(|s| Submission {
                                response: s.response.clone(),
                                elapsed: s.elapsed,
                                bonus: s.bonus.clone(),
                            })
                        })
                        .collect(),
                    joker: t.joker,
                })
                .collect(),
            next_team_id: self.next_team_id,
            max_teams: self.max_teams,
            pictures: self.pictures,
            joining: self.joining,
            phase: match self.phase {
                Phase::Network => SavedPhase::Network,
                Phase::Lobby => SavedPhase::Lobby,
                Phase::Ready { index } => SavedPhase::Ready { index },
                Phase::Question {
                    index,
                    started,
                    deadline,
                } => SavedPhase::Question {
                    index,
                    elapsed: clock.saturating_duration_since(started),
                    left: deadline.saturating_duration_since(clock),
                },
                Phase::Closed { index, .. } => SavedPhase::Closed { index },
                Phase::Slide { index, .. } => SavedPhase::Slide { index },
                Phase::Review { index, .. } => SavedPhase::Review { index },
                Phase::Standings { index, .. } => SavedPhase::Standings { index },
                Phase::Podium { revealed, .. } => SavedPhase::Podium { revealed },
            },
            judgements: self.judgements.clone(),
            played: self.played.clone(),
            shown: self.shown.clone(),
        }
    }

    /// Brings a game back from a [`Snapshot`], paused so the host decides when to go on.
    #[must_use]
    pub fn restore(snapshot: Snapshot, now: Instant) -> Self {
        let phase = match snapshot.phase {
            SavedPhase::Network => Phase::Network,
            SavedPhase::Lobby => Phase::Lobby,
            SavedPhase::Ready { index } => Phase::Ready { index },
            SavedPhase::Question {
                index,
                elapsed,
                left,
            } => Phase::Question {
                index,
                started: now.checked_sub(elapsed).unwrap_or(now),
                deadline: now + left,
            },
            SavedPhase::Closed { index } => Phase::Closed { index, since: now },
            SavedPhase::Slide { index } => Phase::Slide { index, since: now },
            SavedPhase::Review { index } => Phase::Review { index, since: now },
            SavedPhase::Standings { index } => Phase::Standings { index, since: now },
            SavedPhase::Podium { revealed } => Phase::Podium {
                revealed,
                since: now,
            },
        };
        let mut game = Self {
            code: snapshot.code,
            quiz: snapshot.quiz,
            teams: snapshot.teams,
            next_team_id: snapshot.next_team_id,
            max_teams: snapshot.max_teams,
            pictures: snapshot.pictures,
            joining: snapshot.joining,
            phase,
            judgements: snapshot.judgements,
            played: snapshot.played,
            paused_at: None,
            shown: snapshot.shown,
        };
        let _ = game.pause(now);
        game
    }
}
