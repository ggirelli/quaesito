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

/// How a question is answered, as screens need to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    /// Coloured answer buttons.
    Choice,
    /// A short typed answer.
    Text,
    /// A number; the closest guess wins.
    Number,
}

/// A team's answer to a question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    /// Indexes of the picked answers.
    Choice(Vec<usize>),
    Text(String),
    Number(f64),
}

/// The judge's verdict on a typed answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    Full,
    Half,
    Wrong,
}

/// How a group of identical typed answers is scored: a grade plus a bonus (or malus) in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgement {
    pub grade: Grade,
    pub bonus: i32,
}

/// A question as shown on the big screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionInfo {
    /// Position in the quiz, slides included; identifies the question in commands.
    pub index: usize,
    /// Position among the questions, from 1, as shown to people.
    pub number: usize,
    /// Number of questions, slides left out.
    pub total: usize,
    pub text: String,
    pub image: Option<String>,
    /// Media name of the soundtrack, if any.
    pub audio: Option<String>,
    pub input: InputKind,
    /// Answer texts of a multiple-choice question (empty for other kinds).
    pub answers: Vec<String>,
    /// More than one answer is correct.
    pub multi: bool,
    pub time_limit_ms: u64,
}

/// A slide as shown on the big screen; any part may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideInfo {
    pub title: String,
    pub text: String,
    pub image: Option<String>,
}

/// What the host's "next" button leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Upcoming {
    Question,
    Slide,
    /// The end-of-quiz walk through the answers.
    Answers,
    Podium,
}

/// The right answer to a question and what the teams answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Reveal {
    /// Correct answers and how many teams picked each answer.
    Choice {
        correct: Vec<usize>,
        picks: Vec<u32>,
    },
    /// Accepted answers and the typed answers, grouped by normalised text.
    Text {
        accepted: Vec<String>,
        groups: Vec<TextGroup>,
    },
    /// The right number and every guess, closest first.
    Number { answer: f64, guesses: Vec<Guess> },
}

/// Typed answers that are the same once normalised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGroup {
    /// Normalised text, used to judge the group.
    pub key: String,
    /// The answer as the first team typed it.
    pub text: String,
    /// Names of the teams that gave this answer.
    pub teams: Vec<String>,
    pub judgement: Judgement,
    /// Whether the judge changed the automatic verdict.
    pub amended: bool,
}

/// One team's guess on a number question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guess {
    pub team: String,
    pub value: f64,
    /// Whether this is (one of) the closest guesses.
    pub closest: bool,
}

/// One line of the leaderboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandingRow {
    /// 1-based position.
    pub rank: usize,
    pub team: TeamInfo,
    pub score: i32,
    /// How the team did on the question just revealed, in the standings shown after it.
    pub round: Option<RoundResult>,
}

/// A team's result on the question just revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundResult {
    pub points: i32,
    /// How long the team took to answer, if it did.
    pub answer_ms: Option<u64>,
    /// Position before this question, if an earlier question was played.
    pub previous_rank: Option<usize>,
}

/// The quickest fully correct answer to a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fastest {
    pub team: String,
    pub answer_ms: u64,
}

/// Everything the host screen shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostView {
    pub title: String,
    pub code: String,
    pub teams: Vec<TeamInfo>,
    /// Time until the quiz moves on by itself, when auto-advance is on (and not paused).
    pub auto_advance_ms: Option<u64>,
    /// The host paused the clock.
    pub paused: bool,
    pub phase: HostPhase,
}

/// The stage of the quiz, as seen by the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HostPhase {
    /// Waiting for teams to join.
    Lobby,
    /// A question with a soundtrack, waiting for the host to start the music and the clock.
    Ready { question: QuestionInfo },
    /// A question is open for answers.
    Question {
        question: QuestionInfo,
        remaining_ms: u64,
        /// Clock time spent so far, pauses excluded (to seek the soundtrack).
        elapsed_ms: u64,
        answered: usize,
    },
    /// Time is up for a question. Reveal and standings are present when revealing after each question.
    Closed {
        question: QuestionInfo,
        answered: usize,
        reveal: Option<Reveal>,
        /// The quickest right answer, when revealing (not for number questions).
        fastest: Option<Fastest>,
        standings: Option<Vec<StandingRow>>,
        then: Upcoming,
    },
    /// A slide between questions.
    Slide { slide: SlideInfo, then: Upcoming },
    /// End-of-quiz walk through the answers (reveal at end).
    Review {
        question: QuestionInfo,
        reveal: Reveal,
        fastest: Option<Fastest>,
        then: Upcoming,
    },
    /// Final ranking, revealed from 3rd to 1st. `standings` only holds rows revealed so far.
    Podium {
        standings: Vec<StandingRow>,
        finished: bool,
    },
}

/// Commands the host screen can send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommand {
    /// Move to the next step: start, play a soundtrack, close the question early, next
    /// question or slide, next podium spot.
    Next,
    /// Amend the verdict on a group of typed answers (identified by its normalised `key`)
    /// while that question's answers are shown.
    Judge {
        question: usize,
        key: String,
        judgement: Judgement,
    },
    /// Go back to the previous slide, reveal or podium step.
    Back,
    /// Pause or resume the clock and auto-advance.
    TogglePause,
    /// Give the open question this many more seconds.
    AddTime(u32),
    /// Rename a team.
    RenameTeam { team: u32, name: String },
    /// Remove a team from the game.
    RemoveTeam { team: u32 },
}

/// How a team did on one question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerOutcome {
    /// Negative when the judge gave a malus.
    pub points: i32,
    pub fully_correct: bool,
}

/// Everything a team leader's phone shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub title: String,
    pub team: TeamInfo,
    pub display: PhoneDisplay,
    /// Total score, when the quiz settings allow showing it.
    pub score: Option<i32>,
    pub phase: PlayerPhase,
}

/// The stage of the quiz, as seen by a team. Question indexes count questions only, from 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerPhase {
    Lobby {
        teams: usize,
    },
    /// The big screen shows a slide.
    Slide,
    /// The next question has a soundtrack and opens when the host starts it.
    Ready {
        index: usize,
        total: usize,
    },
    Question {
        index: usize,
        total: usize,
        input: InputKind,
        /// Number of answer buttons (multiple choice only).
        answer_count: usize,
        /// Question and answer texts, when the phone display shows full text.
        text: Option<(String, Vec<String>)>,
        multi: bool,
        remaining_ms: u64,
        /// The host paused the clock.
        paused: bool,
        time_limit_ms: u64,
        submitted: Option<Response>,
    },
    Closed {
        index: usize,
        total: usize,
        submitted: Option<Response>,
        /// Present when revealing after each question.
        result: Option<(AnswerOutcome, Vec<usize>)>,
    },
    Review {
        index: usize,
        total: usize,
        submitted: Option<Response>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerCommand {
    /// Submit an answer to the open question.
    Answer(Response),
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
