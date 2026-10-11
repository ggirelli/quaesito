//! Messages exchanged between the server and the host/player screens.
//!
//! The server pushes a complete view whenever something changes; clients render it as-is.

use serde::{Deserialize, Serialize};

use crate::model::{ImageReveal, PhoneDisplay, Point};

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
    /// Items to put in order.
    Order,
    /// Several short typed answers, one per part.
    Parts,
    /// A pin dropped on the question's picture.
    Pin,
}

/// A team's answer to a question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    /// Indexes of the picked answers.
    Choice(Vec<usize>),
    Text(String),
    Number(f64),
    /// Items as shown (by index in the shuffled list), from first to last.
    Order(Vec<usize>),
    /// One typed answer per part (empty for parts left blank).
    Parts(Vec<String>),
    /// Where the team dropped its pin on the picture.
    Pin(Point),
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
    /// How the picture is revealed while the clock runs.
    pub image_reveal: ImageReveal,
    /// With a gradual reveal, how long before the buzzer the picture is fully shown.
    pub image_hold_ms: u64,
    /// The question is worth this many times the usual points.
    pub multiplier: u32,
    /// Media name of the soundtrack, if any.
    pub audio: Option<String>,
    pub input: InputKind,
    /// Answer texts of a multiple-choice question, or the shuffled items of an order question
    /// (empty for other kinds).
    pub answers: Vec<String>,
    /// Answer pictures, parallel to `answers`.
    pub answer_images: Vec<Option<String>>,
    /// More than one answer is correct.
    pub multi: bool,
    pub time_limit_ms: u64,
    /// The bonus question and what it is worth, in percent of the question's points.
    pub bonus: Option<(String, u32)>,
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
    /// A results slide: the answers since the previous one, then the standings.
    Results,
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
    /// The items in the right order, and how many teams put each one in its place.
    Order {
        items: Vec<String>,
        placed: Vec<u32>,
    },
    /// For each part, its accepted answers and the typed ones, grouped and judged as for a
    /// typed-answer question. Group keys start with the part's index.
    Parts { parts: Vec<PartReveal> },
    /// The right spot and every pin, closest first.
    Pin {
        answer: Point,
        /// The picture's height divided by its width.
        aspect: f64,
        pins: Vec<PinGuess>,
    },
}

/// One team's pin on a picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinGuess {
    pub team: String,
    pub at: Point,
    /// Distance from the right spot, in picture widths.
    pub distance: f64,
    pub points: i32,
}

/// One part of a several-answers reveal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartReveal {
    pub label: String,
    pub accepted: Vec<String>,
    pub groups: Vec<TextGroup>,
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
    /// The team played its joker on this question.
    pub joker: bool,
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
        /// The bonus question's answers, when revealing and the question has one.
        bonus: Option<PartReveal>,
        then: Upcoming,
    },
    /// A slide between questions.
    Slide { slide: SlideInfo, then: Upcoming },
    /// The standings at a results slide, with the points each team gained and the places it
    /// moved since the previous results slide.
    Standings {
        title: String,
        standings: Vec<StandingRow>,
        then: Upcoming,
    },
    /// Walk through the answers (reveal at end), at a results slide or at the end of the quiz.
    Review {
        question: QuestionInfo,
        reveal: Reveal,
        fastest: Option<Fastest>,
        /// The bonus question's answers, if the question has one.
        bonus: Option<PartReveal>,
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
    /// Negative when the judge gave a malus. Doubled by a joker.
    pub points: i32,
    pub fully_correct: bool,
    /// The team played its joker on this question.
    pub joker: bool,
}

/// Everything a team leader's phone shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub title: String,
    pub team: TeamInfo,
    pub display: PhoneDisplay,
    pub joker: JokerState,
    /// Total score, when the quiz settings allow showing it.
    pub score: Option<i32>,
    pub phase: PlayerPhase,
}

/// A team's joker, which doubles one question's points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JokerState {
    /// The quiz has no jokers.
    Unavailable,
    /// Not played yet.
    Ready,
    /// Played on the current question.
    PlayedHere,
    /// Played on another question.
    Spent,
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
        /// The shuffled items of an order question, or True and False (always sent: they cannot
        /// be told apart by colour alone).
        items: Vec<String>,
        /// Question and answer texts, when the phone display shows full text.
        text: Option<(String, Vec<String>)>,
        /// Answer pictures, parallel to the answer texts, when the phone display shows full text.
        answer_images: Vec<Option<String>>,
        /// The picture to drop a pin on and its height-to-width ratio (pin questions only).
        map: Option<(String, f64)>,
        multi: bool,
        remaining_ms: u64,
        /// The host paused the clock.
        paused: bool,
        time_limit_ms: u64,
        submitted: Option<Response>,
        /// The bonus question and its worth in percent, to answer together with the question.
        bonus: Option<(String, u32)>,
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
    /// The big screen shows the standings at a results slide.
    Standings {
        rank: usize,
        teams: usize,
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
    /// Submit an answer to the open question, optionally playing the team's joker on it, with
    /// the typed answer to its bonus question if it has one.
    Answer {
        response: Response,
        joker: bool,
        #[serde(default)]
        bonus: Option<String>,
    },
}

/// Credentials a team gets when joining; kept on the phone to reconnect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinTicket {
    pub team_id: u32,
    pub token: String,
}

/// The full results of a game: every team's points on every question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameResults {
    pub title: String,
    /// Question texts, in playing order (slides left out).
    pub questions: Vec<String>,
    /// Best first.
    pub rows: Vec<ResultRow>,
}

/// One team's line in [`GameResults`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultRow {
    pub rank: usize,
    pub team: String,
    pub score: i32,
    /// Points per question, `None` where the team did not answer.
    pub points: Vec<Option<i32>>,
    /// Whether the team's joker was played on each question.
    pub jokers: Vec<bool>,
}

impl GameResults {
    /// The results as CSV: rank, team, total, then one column per question.
    #[must_use]
    pub fn to_csv(&self) -> String {
        let field = |text: &str| {
            if text.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_owned()
            }
        };
        let mut lines = vec![
            ["Rank", "Team", "Total"]
                .into_iter()
                .map(str::to_owned)
                .chain(self.questions.iter().map(|q| field(q)))
                .collect::<Vec<_>>()
                .join(","),
        ];
        for row in &self.rows {
            let mut cells = vec![
                row.rank.to_string(),
                field(&row.team),
                row.score.to_string(),
            ];
            cells.extend(
                row.points
                    .iter()
                    .map(|p| p.map(|p| p.to_string()).unwrap_or_default()),
            );
            lines.push(cells.join(","));
        }
        lines.join("\r\n") + "\r\n"
    }
}

/// The largest uploads the server accepts, in bytes (set with `QUAESITO_MAX_*_MB`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadLimits {
    pub picture: u64,
    pub audio: u64,
    pub video: u64,
    /// A quiz bundle to import, media included.
    pub import: u64,
}

impl UploadLimits {
    /// Megabytes, for messages.
    #[must_use]
    pub fn megabytes(bytes: u64) -> u64 {
        bytes / (1024 * 1024)
    }
}

/// Short description of a live session for the admin page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub code: String,
    pub title: String,
    pub teams: usize,
    pub finished: bool,
    /// An admin's test game (not saved, ended from the test page).
    pub test: bool,
}

/// What the admin's test page needs about a test game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestInfo {
    pub quiz_id: String,
    pub title: String,
    /// The test phones, in joining order.
    pub phones: Vec<TestPhone>,
}

/// One test phone: its team name and the ticket its frame plays with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestPhone {
    pub name: String,
    pub ticket: String,
}
