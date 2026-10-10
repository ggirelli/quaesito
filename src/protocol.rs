//! Messages exchanged between the server and the host/player screens.
//!
//! The server pushes a complete view whenever something changes; clients render it as-is.

use serde::{Deserialize, Serialize};

use crate::model::PhoneDisplay;

/// Longest team name, in characters.
pub const MAX_NAME_CHARS: usize = 24;

/// A team as shown on screens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamInfo {
    pub id: u32,
    pub name: String,
    /// Media name of the uploaded avatar, if any.
    pub avatar: Option<String>,
    /// Whether the team leader's phone is currently connected.
    pub connected: bool,
}

/// A question as shown on the big screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionInfo {
    pub index: usize,
    pub total: usize,
    pub text: String,
    pub image: Option<String>,
    pub answers: Vec<String>,
    /// More than one answer is correct.
    pub multi: bool,
    pub time_limit_ms: u64,
}

/// Correct answers and how many teams picked each answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reveal {
    pub correct: Vec<usize>,
    pub picks: Vec<u32>,
}

/// One line of the leaderboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandingRow {
    /// 1-based position.
    pub rank: usize,
    pub team: TeamInfo,
    pub score: u32,
}

/// Everything the host screen shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostView {
    pub title: String,
    pub code: String,
    pub teams: Vec<TeamInfo>,
    /// Time until the quiz moves on by itself, when auto-advance is on.
    pub auto_advance_ms: Option<u64>,
    pub phase: HostPhase,
}

/// The stage of the quiz, as seen by the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostPhase {
    /// Waiting for teams to join.
    Lobby,
    /// A question is open for answers.
    Question {
        question: QuestionInfo,
        remaining_ms: u64,
        answered: usize,
    },
    /// Time is up for a question. Reveal and standings are present when revealing after each question.
    Closed {
        question: QuestionInfo,
        answered: usize,
        reveal: Option<Reveal>,
        standings: Option<Vec<StandingRow>>,
        last: bool,
    },
    /// End-of-quiz walk through the answers (reveal at end).
    Review {
        question: QuestionInfo,
        reveal: Reveal,
        last: bool,
    },
    /// Final ranking, revealed from 3rd to 1st. `standings` only holds rows revealed so far.
    Podium {
        standings: Vec<StandingRow>,
        finished: bool,
    },
}

/// Commands the host screen can send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommand {
    /// Move to the next step: start, close the question early, next question, next podium spot.
    Next,
}

/// How a team did on one question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerOutcome {
    pub points: u32,
    pub fully_correct: bool,
}

/// Everything a team leader's phone shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerView {
    pub title: String,
    pub team: TeamInfo,
    pub display: PhoneDisplay,
    /// Total score, when the quiz settings allow showing it.
    pub score: Option<u32>,
    pub phase: PlayerPhase,
}

/// The stage of the quiz, as seen by a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerPhase {
    Lobby {
        teams: usize,
    },
    Question {
        index: usize,
        total: usize,
        answer_count: usize,
        /// Question and answer texts, when the phone display shows full text.
        text: Option<(String, Vec<String>)>,
        multi: bool,
        remaining_ms: u64,
        submitted: Option<Vec<usize>>,
    },
    Closed {
        index: usize,
        total: usize,
        submitted: Option<Vec<usize>>,
        /// Present when revealing after each question.
        result: Option<(AnswerOutcome, Vec<usize>)>,
    },
    Review {
        index: usize,
        total: usize,
        submitted: Option<Vec<usize>>,
        outcome: AnswerOutcome,
        correct: Vec<usize>,
    },
    Podium {
        /// Own position, once revealed.
        rank: Option<usize>,
        teams: usize,
        finished: bool,
    },
}

/// Commands a team leader's phone can send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerCommand {
    /// Submit the selected answer indexes for the open question.
    Answer(Vec<usize>),
}

/// Credentials a team gets when joining; kept on the phone to reconnect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinTicket {
    pub team_id: u32,
    pub token: String,
}

/// Short description of a live session for the admin page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub code: String,
    pub title: String,
    pub teams: usize,
    pub finished: bool,
}
