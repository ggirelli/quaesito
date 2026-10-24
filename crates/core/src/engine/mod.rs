//! The game state machine for one live session.
//!
//! Pure logic: every method takes the current time, so it can be tested without a runtime.
//!
//! ```text
//! [Network] → Lobby → Question(0) → Closed(0) → … → Closed(n-1) → [Review(0) → … → Review(n-1)] → Podium
//! ```
//!
//! The host can pause (freezing the clock and auto-advance), add time to an open question, and
//! go back: to the previous slide or reveal, or out of a question opened by mistake. A closed
//! question can be reopened for the teams that have not answered yet, when it was closed by
//! mistake.
//!
//! When the quiz has a Wi-Fi slide, the game opens on it, before the join code; teams may already
//! join there.
//!
//! A question with a soundtrack first waits in `Ready` until the host starts the music, which
//! also starts its clock. Slides sit between questions as `Slide(i)` and are skipped by the
//! review.
//!
//! When answers are revealed at the end, results slides split the quiz into parts: reaching one
//! walks through the answers of its part, then shows the standings, before the quiz goes on.
//!
//! ```text
//! … → Closed(r-1) → [Review(first of part) → … → Review(last of part)] → Standings(r) → …
//! ```
//!
//! The review at the end then only covers the questions after the last results slide.

use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    model::{QuestionKind, Quiz},
    protocol::{DEFAULT_MAX_TEAMS, Judgement, MAX_TEAMS_CEILING, Response, TeamInfo},
};

mod answers;
mod commands;
mod flow;
mod outcomes;
mod reveal;
mod snapshot;
mod standings;
mod teams;
mod views;

pub use commands::MAX_EXTRA_SECS;
pub use snapshot::Snapshot;
/// The clock the engine takes: std's `Instant` natively, the browser's clock in wasm (where std's
/// cannot tell the time).
pub use web_time::Instant;

/// One running quiz.
pub struct Game {
    code: String,
    quiz: Quiz,
    teams: Vec<Team>,
    next_team_id: u32,
    /// The most teams that can join.
    max_teams: usize,
    /// Whether teams may upload a picture.
    pictures: bool,
    /// Whether new teams may join: the host closes joining once every team is in.
    joining: bool,
    phase: Phase,
    /// Per question: the judge's amendments to typed answers, by normalised text.
    judgements: Vec<HashMap<String, Judgement>>,
    /// Per question: whether it has been closed, so going forward again after going back shows
    /// its results instead of asking it again.
    played: Vec<bool>,
    /// When the host paused the game, if it is paused.
    paused_at: Option<Instant>,
    /// Per order question: the items as shown, by index in the right order. Per pairs question:
    /// the right column as shown, by index of the pair. Empty otherwise.
    shown: Vec<Vec<usize>>,
}

#[derive(Serialize, Deserialize)]
struct Team {
    id: u32,
    name: String,
    avatar: Option<String>,
    /// The ready-made picture picked instead of uploading one (see [`MASCOTS`]).
    #[serde(default)]
    mascot: Option<String>,
    /// The background picked for the picture, by index (see [`BACKDROP_COUNT`]).
    #[serde(default)]
    backdrop: Option<u8>,
    token: String,
    /// Live phone connections; phones reconnect after a restart.
    #[serde(skip)]
    connections: u32,
    submissions: Vec<Option<Submission>>,
    /// The question the team played its joker on, if it has.
    joker: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct Submission {
    response: Response,
    elapsed: Duration,
    /// The typed answer to the question's bonus question, if any.
    #[serde(default)]
    bonus: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The Wi-Fi slide, before the lobby.
    Network,
    Lobby,
    Ready {
        index: usize,
    },
    Question {
        index: usize,
        started: Instant,
        deadline: Instant,
    },
    Closed {
        index: usize,
        since: Instant,
    },
    Slide {
        index: usize,
        since: Instant,
    },
    Review {
        index: usize,
        since: Instant,
    },
    /// The standings at results slide `index`.
    Standings {
        index: usize,
        since: Instant,
    },
    Podium {
        revealed: usize,
        since: Instant,
    },
}

/// Why a request was refused; the message is shown to users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub &'static str);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Refusal {}

impl Game {
    /// Starts a session in the lobby. `random_below(n)` must return a number in `0..n`; it shows
    /// the items of order and pairs questions in a random order.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn new(
        code: String,
        quiz: Quiz,
        mut random_below: impl FnMut(usize) -> usize,
    ) -> Result<Self, Refusal> {
        if !quiz.validate().is_empty() {
            return Err(Refusal("This quiz is not ready to be played yet."));
        }
        let quiz_len = quiz.questions.len();
        let shown = quiz
            .questions
            .iter()
            .map(|q| match &q.kind {
                QuestionKind::Order { items } => shuffled(items.len(), &mut random_below),
                QuestionKind::Pairs { pairs } => shuffled(pairs.len(), &mut random_below),
                _ => Vec::new(),
            })
            .collect();
        let phase = if quiz.settings.network.enabled {
            Phase::Network
        } else {
            Phase::Lobby
        };
        Ok(Self {
            code,
            quiz,
            teams: Vec::new(),
            next_team_id: 1,
            max_teams: DEFAULT_MAX_TEAMS,
            pictures: true,
            joining: true,
            phase,
            judgements: vec![HashMap::new(); quiz_len],
            played: vec![false; quiz_len],
            paused_at: None,
            shown,
        })
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Sets the most teams that can join, from 1 to [`MAX_TEAMS_CEILING`].
    ///
    /// # Errors
    /// Refuses a limit out of that range.
    pub fn set_max_teams(&mut self, max_teams: usize) -> Result<(), Refusal> {
        if !(1..=MAX_TEAMS_CEILING).contains(&max_teams) {
            return Err(Refusal("A game takes 1 to 10,000 teams."));
        }
        self.max_teams = max_teams;
        Ok(())
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.quiz.title
    }

    /// Id of the quiz being played.
    #[must_use]
    pub fn quiz_id(&self) -> &str {
        &self.quiz.id
    }

    /// Every team's name and reconnect token, in joining order (for the admin's test page).
    #[must_use]
    pub fn tickets(&self) -> Vec<(String, String)> {
        self.teams
            .iter()
            .map(|t| (t.name.clone(), t.token.clone()))
            .collect()
    }

    /// Uploaded files the game shows: its quiz's media and the team pictures.
    #[must_use]
    pub fn media(&self) -> Vec<String> {
        let mut names = self.quiz.media();
        names.extend(self.teams.iter().filter_map(|t| t.avatar.clone()));
        names
    }

    #[must_use]
    pub fn team_count(&self) -> usize {
        self.teams.len()
    }

    /// Whether the game has reached the end of the podium.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Podium { revealed, .. } if revealed == self.podium_steps())
    }

    /// The game's clock: frozen while paused.
    fn clock(&self, now: Instant) -> Instant {
        self.paused_at.unwrap_or(now)
    }

    /// Moves to `phase`, remembering which questions have been closed.
    fn set_phase(&mut self, phase: Phase) {
        if let Phase::Closed { index, .. } = phase {
            self.played[index] = true;
        }
        self.phase = phase;
    }

    fn team_mut(&mut self, id: u32) -> Option<&mut Team> {
        self.teams.iter_mut().find(|t| t.id == id)
    }
}

impl Team {
    fn info(&self) -> TeamInfo {
        TeamInfo {
            id: self.id,
            name: self.name.clone(),
            avatar: self.avatar.clone(),
            mascot: self.mascot.clone(),
            backdrop: self.backdrop,
            connected: self.connections > 0,
        }
    }
}

/// A random order of `0..n` that differs from the original one when it can. `random_below(n)`
/// must return a number in `0..n`.
fn shuffled(n: usize, random_below: &mut impl FnMut(usize) -> usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, random_below(i + 1).min(i));
    }
    if n > 1 && order.iter().enumerate().all(|(i, &v)| i == v) {
        order.rotate_left(1);
    }
    order
}

#[cfg(test)]
mod tests;
