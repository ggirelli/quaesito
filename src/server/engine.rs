//! The game state machine for one live session.
//!
//! Pure logic: every method takes the current time, so it can be tested without a runtime.
//!
//! ```text
//! Lobby → Question(0) → Closed(0) → … → Closed(n-1) → [Review(0) → … → Review(n-1)] → Podium
//! ```
//!
//! The host can pause (freezing the clock and auto-advance), add time to an open question, and
//! go back: to the previous slide or reveal, or out of a question opened by mistake.
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

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use super::matching::normalize;
use crate::{
    model::{AdvanceMode, Bonus, MAX_TEXT_ANSWER_CHARS, Question, QuestionKind, Quiz, RevealMode},
    protocol::{
        AnswerOutcome, DEFAULT_MAX_TEAMS, Fastest, GameResults, Grade, Guess, HostPhase, HostView,
        InputKind, JokerState, Judgement, MAX_NAME_CHARS, MAX_TEAMS_CEILING, PartReveal, PinGuess,
        PlayerPhase, PlayerView, QuestionInfo, Response, ResultRow, Reveal, RoundResult, SlideInfo,
        StandingRow, TeamInfo, TextGroup, Upcoming,
    },
    scoring::{self, Tally},
};

fn default_max_teams() -> usize {
    DEFAULT_MAX_TEAMS
}

/// Auto-advance pause after a question when its answers are shown.
const AUTO_AFTER_REVEAL: Duration = Duration::from_secs(8);
/// Auto-advance pause after a question when answers stay hidden until the end.
const AUTO_AFTER_HIDDEN: Duration = Duration::from_secs(3);
/// Auto-advance pause on a slide.
const AUTO_SLIDE: Duration = Duration::from_secs(10);
/// Auto-advance pause between podium spots.
const AUTO_PODIUM_STEP: Duration = Duration::from_secs(4);

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

/// One running quiz.
pub struct Game {
    code: String,
    quiz: Quiz,
    teams: Vec<Team>,
    next_team_id: u32,
    /// The most teams that can join.
    max_teams: usize,
    phase: Phase,
    /// Per question: the judge's amendments to typed answers, by normalised text.
    judgements: Vec<HashMap<String, Judgement>>,
    /// Per question: whether it has been closed, so going forward again after going back shows
    /// its results instead of asking it again.
    played: Vec<bool>,
    /// When the host paused the game, if it is paused.
    paused_at: Option<Instant>,
    /// Per order question: the items as shown, by index in the right order (empty otherwise).
    shown: Vec<Vec<usize>>,
}

#[derive(Serialize, Deserialize)]
struct Team {
    id: u32,
    name: String,
    avatar: Option<String>,
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
    /// Starts a session in the lobby.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn new(code: String, quiz: Quiz) -> Result<Self, Refusal> {
        if !quiz.validate().is_empty() {
            return Err(Refusal("This quiz is not ready to be played yet."));
        }
        let quiz_len = quiz.questions.len();
        let shown = quiz
            .questions
            .iter()
            .map(|q| match &q.kind {
                QuestionKind::Order { items } => shuffled(items.len()),
                _ => Vec::new(),
            })
            .collect();
        Ok(Self {
            code,
            quiz,
            teams: Vec::new(),
            next_team_id: 1,
            max_teams: DEFAULT_MAX_TEAMS,
            phase: Phase::Lobby,
            judgements: vec![HashMap::new(); quiz_len],
            played: vec![false; quiz_len],
            paused_at: None,
            shown,
        })
    }

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
            phase: match self.phase {
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
            phase,
            judgements: snapshot.judgements,
            played: snapshot.played,
            paused_at: None,
            shown: snapshot.shown,
        };
        let _ = game.pause(now);
        game
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

    /// Registers a team and returns its id and reconnect token.
    ///
    /// # Errors
    /// Refuses empty, overlong or duplicate names, and joining a full or finished game.
    pub fn join(&mut self, name: &str, token: String) -> Result<u32, Refusal> {
        let name = self.check_name(name, None)?;
        if self.teams.len() >= self.max_teams {
            return Err(Refusal("This game is full."));
        }
        if matches!(self.phase, Phase::Podium { .. })
            || matches!(self.phase, Phase::Review { index, .. } if self.part_end(index) == self.quiz.questions.len())
        {
            return Err(Refusal("This game is already over."));
        }
        let id = self.next_team_id;
        self.next_team_id += 1;
        self.teams.push(Team {
            id,
            name,
            avatar: None,
            token,
            connections: 0,
            submissions: (0..self.quiz.questions.len()).map(|_| None).collect(),
            joker: None,
        });
        Ok(id)
    }

    /// Renames a team (the host fixing a typo or an unwelcome name).
    ///
    /// # Errors
    /// Refuses unknown teams and names [`Game::join`] would refuse.
    pub fn rename_team(&mut self, team: u32, name: &str) -> Result<(), Refusal> {
        let name = self.check_name(name, Some(team))?;
        self.team_mut(team).ok_or(Refusal("Unknown team."))?.name = name;
        Ok(())
    }

    /// Removes a team and its answers; its phone is sent back to the join form.
    ///
    /// # Errors
    /// Refuses unknown teams, and removals once the podium is being revealed.
    pub fn remove_team(&mut self, team: u32, now: Instant) -> Result<(), Refusal> {
        if matches!(self.phase, Phase::Podium { .. }) {
            return Err(Refusal("The podium is already being revealed."));
        }
        let before = self.teams.len();
        self.teams.retain(|t| t.id != team);
        if self.teams.len() == before {
            return Err(Refusal("Unknown team."));
        }
        // The removed team may have been the last one the question was waiting for.
        if let Phase::Question { index, .. } = self.phase
            && !self.teams.is_empty()
            && self.everyone_answered(index)
        {
            self.set_phase(Phase::Closed { index, since: now });
        }
        Ok(())
    }

    /// A trimmed, valid team name not used by another team than `except`.
    fn check_name(&self, name: &str, except: Option<u32>) -> Result<String, Refusal> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Refusal("Please pick a team name."));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(Refusal("That team name is too long."));
        }
        if self
            .teams
            .iter()
            .any(|t| Some(t.id) != except && t.name.to_lowercase() == name.to_lowercase())
        {
            return Err(Refusal("Another team already has that name."));
        }
        Ok(name.to_owned())
    }

    /// Whether every connected team has answered question `index`.
    fn everyone_answered(&self, index: usize) -> bool {
        self.teams
            .iter()
            .filter(|t| t.connections > 0)
            .all(|t| t.submissions[index].is_some())
    }

    /// The team owning a reconnect token.
    #[must_use]
    pub fn team_for_token(&self, token: &str) -> Option<u32> {
        self.teams.iter().find(|t| t.token == token).map(|t| t.id)
    }

    /// Sets a team's avatar.
    pub fn set_avatar(&mut self, team: u32, avatar: String) {
        if let Some(t) = self.team_mut(team) {
            t.avatar = Some(avatar);
        }
    }

    /// Tracks a team leader's phone connecting (`true`) or disconnecting.
    pub fn set_connected(&mut self, team: u32, connected: bool) {
        if let Some(t) = self.team_mut(team) {
            if connected {
                t.connections += 1;
            } else {
                t.connections = t.connections.saturating_sub(1);
            }
        }
    }

    /// The host's "next" button.
    ///
    /// # Errors
    /// Refuses to start without teams, or to go past the end.
    pub fn next(&mut self, now: Instant) -> Result<(), Refusal> {
        self.resume(now);
        let phase = match self.phase {
            Phase::Lobby => {
                if self.teams.is_empty() {
                    return Err(Refusal("Wait for at least one team to join."));
                }
                self.item_phase(0, now)
            }
            Phase::Ready { index } => self.open_question(index, now),
            Phase::Question { index, .. } => Phase::Closed { index, since: now },
            Phase::Closed { index, .. }
            | Phase::Slide { index, .. }
            | Phase::Standings { index, .. } => match self.upcoming_after(index) {
                Upcoming::Question | Upcoming::Slide | Upcoming::Results => {
                    self.item_phase(index + 1, now)
                }
                Upcoming::Answers => Phase::Review {
                    index: self
                        .first_question_in(self.part_start(self.quiz.questions.len()), usize::MAX)
                        .unwrap_or_default(),
                    since: now,
                },
                Upcoming::Podium => Phase::Podium {
                    revealed: 0,
                    since: now,
                },
            },
            Phase::Review { index, .. } => {
                let end = self.part_end(index);
                match self.first_question_in(index + 1, end) {
                    Some(index) => Phase::Review { index, since: now },
                    None if end < self.quiz.questions.len() => Phase::Standings {
                        index: end,
                        since: now,
                    },
                    None => Phase::Podium {
                        revealed: 0,
                        since: now,
                    },
                }
            }
            Phase::Podium { revealed, .. } if revealed < self.podium_steps() => Phase::Podium {
                revealed: revealed + 1,
                since: now,
            },
            Phase::Podium { .. } => return Err(Refusal("The quiz is over.")),
        };
        self.set_phase(phase);
        Ok(())
    }

    /// The host's "back" button: the previous slide, reveal or podium step. Leaving a question
    /// that is still open withdraws it, so it is asked afresh later.
    ///
    /// # Errors
    /// Refuses in the lobby.
    pub fn back(&mut self, now: Instant) -> Result<(), Refusal> {
        self.resume(now);
        let last = self.quiz.questions.len() - 1;
        self.phase = match self.phase {
            Phase::Lobby => return Err(Refusal("This is the start.")),
            Phase::Question { index, .. } => {
                for team in &mut self.teams {
                    team.submissions[index] = None;
                    if team.joker == Some(index) {
                        team.joker = None;
                    }
                }
                self.judgements[index].clear();
                self.revisit_before(index, now)
            }
            Phase::Ready { index } | Phase::Slide { index, .. } | Phase::Closed { index, .. } => {
                self.revisit_before(index, now)
            }
            Phase::Review { index, .. } => {
                match self.last_question_in(self.part_start(index), index) {
                    Some(index) => Phase::Review { index, since: now },
                    None => self.revisit(self.part_end(index) - 1, now),
                }
            }
            Phase::Standings { index, .. } => {
                match self.last_question_in(self.part_start(index), index) {
                    Some(index) => Phase::Review { index, since: now },
                    None => self.revisit_before(index, now),
                }
            }
            Phase::Podium { revealed, .. } if revealed > 0 => Phase::Podium {
                revealed: revealed - 1,
                since: now,
            },
            Phase::Podium { .. } => match self.quiz.settings.reveal {
                RevealMode::AtEnd => {
                    match self.last_question_in(self.part_start(last + 1), last + 1) {
                        Some(index) => Phase::Review { index, since: now },
                        None => self.revisit(last, now),
                    }
                }
                RevealMode::AfterEachQuestion => self.revisit(last, now),
            },
        };
        Ok(())
    }

    /// Freezes the clock and the auto-advance countdown.
    ///
    /// # Errors
    /// Refuses when there is nothing to pause.
    pub fn pause(&mut self, now: Instant) -> Result<(), Refusal> {
        if matches!(self.phase, Phase::Lobby | Phase::Ready { .. }) || self.paused_at.is_some() {
            return Err(Refusal("Nothing to pause."));
        }
        self.paused_at = Some(now);
        Ok(())
    }

    /// Restarts the clock where it was paused; does nothing when not paused.
    pub fn resume(&mut self, now: Instant) {
        let Some(paused_at) = self.paused_at.take() else {
            return;
        };
        let pause = now.saturating_duration_since(paused_at);
        match &mut self.phase {
            Phase::Question {
                started, deadline, ..
            } => {
                *started += pause;
                *deadline += pause;
            }
            Phase::Closed { since, .. }
            | Phase::Slide { since, .. }
            | Phase::Review { since, .. }
            | Phase::Standings { since, .. }
            | Phase::Podium { since, .. } => *since += pause,
            Phase::Lobby | Phase::Ready { .. } => {}
        }
    }

    /// Gives an open question more time.
    ///
    /// # Errors
    /// Refuses when no question is open.
    pub fn add_time(&mut self, extra: Duration) -> Result<(), Refusal> {
        let Phase::Question { deadline, .. } = &mut self.phase else {
            return Err(Refusal("No question is open right now."));
        };
        *deadline += extra;
        Ok(())
    }

    /// Whether the host has paused the game.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
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

    /// The phase showing item `index` again: a slide, a results slide's standings, or a played
    /// question's results.
    fn revisit(&self, index: usize, now: Instant) -> Phase {
        if self.quiz.questions[index].is_results() {
            Phase::Standings { index, since: now }
        } else if self.quiz.questions[index].is_slide() {
            Phase::Slide { index, since: now }
        } else {
            Phase::Closed { index, since: now }
        }
    }

    /// The phase showing whatever came before item `index`.
    fn revisit_before(&self, index: usize, now: Instant) -> Phase {
        match index.checked_sub(1) {
            Some(previous) => self.revisit(previous, now),
            None => Phase::Lobby,
        }
    }

    /// Submits a team's answer to the open question, optionally playing its joker on it.
    ///
    /// # Errors
    /// Refuses when no question is open, the team already answered, the answer is invalid, or
    /// the joker is not available.
    pub fn answer(
        &mut self,
        team: u32,
        response: Response,
        joker: bool,
        now: Instant,
    ) -> Result<(), Refusal> {
        self.answer_with_bonus(team, response, joker, None, now)
    }

    /// [`Game::answer`], with the typed answer to the question's bonus question. A bonus answer
    /// to a question without one is ignored.
    ///
    /// # Errors
    /// As [`Game::answer`], and refuses an overlong bonus answer.
    pub fn answer_with_bonus(
        &mut self,
        team: u32,
        response: Response,
        joker: bool,
        bonus: Option<String>,
        now: Instant,
    ) -> Result<(), Refusal> {
        let jokers = self.quiz.settings.joker;
        let Phase::Question {
            index,
            started,
            deadline,
        } = self.phase
        else {
            return Err(Refusal("No question is open right now."));
        };
        let now = self.clock(now);
        if now >= deadline {
            return Err(Refusal("Time is up."));
        }
        let response = check_response(&self.quiz.questions[index], response)?;
        let bonus =
            bonus.filter(|text| self.active_bonus(index).is_some() && !normalize(text).is_empty());
        if bonus
            .as_ref()
            .is_some_and(|text| text.chars().count() > MAX_TEXT_ANSWER_CHARS)
        {
            return Err(Refusal("That bonus answer is too long."));
        }
        let team = self
            .teams
            .iter_mut()
            .find(|t| t.id == team)
            .ok_or(Refusal("Unknown team."))?;
        if team.submissions[index].is_some() {
            return Err(Refusal("Your answer is already in."));
        }
        if joker {
            if !jokers {
                return Err(Refusal("This quiz has no jokers."));
            }
            if team.joker.is_some() {
                return Err(Refusal("Your joker is already used."));
            }
            team.joker = Some(index);
        }
        team.submissions[index] = Some(Submission {
            response,
            elapsed: now.duration_since(started),
            bonus,
        });

        if self.everyone_answered(index) {
            self.resume(now);
            self.set_phase(Phase::Closed { index, since: now });
        }
        Ok(())
    }

    /// The judge amends the verdict on the typed answers that normalise to `key`.
    ///
    /// Possible while that question's answers are shown; restarts the auto-advance pause.
    ///
    /// # Errors
    /// Refuses when the question is not a typed-text question whose answers are on screen.
    pub fn judge(
        &mut self,
        question: usize,
        key: &str,
        judgement: Judgement,
        now: Instant,
    ) -> Result<(), Refusal> {
        let revealing = self.quiz.settings.reveal == RevealMode::AfterEachQuestion;
        let shown = match self.phase {
            Phase::Closed { index, .. } => revealing && index == question,
            Phase::Review { index, .. } => index == question,
            _ => false,
        };
        if !shown {
            return Err(Refusal("These answers are not on screen."));
        }
        let typed = matches!(
            self.quiz.questions[question].kind,
            QuestionKind::Text { .. } | QuestionKind::Parts { .. }
        );
        let bonus = key.starts_with(BONUS_KEY_PREFIX) && self.active_bonus(question).is_some();
        if !typed && !bonus {
            return Err(Refusal("Only typed answers can be judged."));
        }
        let judgement = Judgement {
            grade: judgement.grade,
            bonus: judgement
                .bonus
                .clamp(-scoring::MAX_BONUS, scoring::MAX_BONUS),
        };
        if judgement == self.automatic_judgement(question, key) {
            self.judgements[question].remove(key);
        } else {
            self.judgements[question].insert(key.to_owned(), judgement);
        }
        match &mut self.phase {
            Phase::Closed { since, .. } | Phase::Review { since, .. } => *since = now,
            _ => {}
        }
        Ok(())
    }

    /// When [`Game::tick`] next needs to run, if ever.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        if self.paused_at.is_some() {
            return None;
        }
        if let Phase::Question { deadline, .. } = self.phase {
            return Some(deadline);
        }
        if self.quiz.settings.advance != AdvanceMode::Auto {
            return None;
        }
        match self.phase {
            Phase::Closed { since, .. } => Some(
                since
                    + match self.quiz.settings.reveal {
                        RevealMode::AfterEachQuestion => AUTO_AFTER_REVEAL,
                        RevealMode::AtEnd => AUTO_AFTER_HIDDEN,
                    },
            ),
            Phase::Slide { since, .. } | Phase::Standings { since, .. } => Some(since + AUTO_SLIDE),
            Phase::Review { since, .. } => Some(since + AUTO_AFTER_REVEAL),
            Phase::Podium { revealed, since } if revealed < self.podium_steps() => {
                Some(since + AUTO_PODIUM_STEP)
            }
            _ => None,
        }
    }

    /// Applies time-based transitions. Returns whether anything changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        match self.next_deadline() {
            Some(deadline) if now >= deadline => self.next(now).is_ok(),
            _ => false,
        }
    }

    /// What the big screen shows.
    #[must_use]
    pub fn host_view(&self, now: Instant) -> HostView {
        let clock = self.clock(now);
        let phase = match self.phase {
            Phase::Lobby => HostPhase::Lobby,
            Phase::Ready { index } => HostPhase::Ready {
                question: self.question_info(index),
            },
            Phase::Question {
                index,
                started,
                deadline,
            } => HostPhase::Question {
                question: self.question_info(index),
                remaining_ms: millis(deadline.saturating_duration_since(clock)),
                elapsed_ms: millis(clock.saturating_duration_since(started)),
                answered: self.answered(index),
            },
            Phase::Closed { index, .. } => {
                let revealing = self.quiz.settings.reveal == RevealMode::AfterEachQuestion;
                HostPhase::Closed {
                    question: self.question_info(index),
                    answered: self.answered(index),
                    reveal: revealing.then(|| self.reveal(index)),
                    fastest: revealing.then(|| self.fastest(index)).flatten(),
                    standings: revealing.then(|| self.standings_after(index)),
                    bonus: revealing.then(|| self.bonus_reveal(index)).flatten(),
                    then: self.upcoming_after(index),
                }
            }
            Phase::Slide { index, .. } => HostPhase::Slide {
                slide: self.slide_info(index),
                then: self.upcoming_after(index),
            },
            Phase::Review { index, .. } => HostPhase::Review {
                question: self.question_info(index),
                reveal: self.reveal(index),
                fastest: self.fastest(index),
                bonus: self.bonus_reveal(index),
                then: if self
                    .first_question_in(index + 1, self.part_end(index))
                    .is_some()
                {
                    Upcoming::Question
                } else if self.part_end(index) < self.quiz.questions.len() {
                    Upcoming::Results
                } else {
                    Upcoming::Podium
                },
            },
            Phase::Standings { index, .. } => HostPhase::Standings {
                title: self.slide_info(index).title,
                standings: self.standings_of_part(index),
                then: self.upcoming_after(index),
            },
            Phase::Podium { revealed, .. } => {
                let threshold = self.podium_threshold(revealed);
                HostPhase::Podium {
                    standings: self
                        .standings()
                        .into_iter()
                        .filter(|row| row.rank > threshold)
                        .collect(),
                    finished: revealed == self.podium_steps(),
                }
            }
        };
        HostView {
            title: self.quiz.title.clone(),
            code: self.code.clone(),
            teams: self.teams.iter().map(Team::info).collect(),
            max_teams: self.max_teams,
            paused: self.paused_at.is_some(),
            auto_advance_ms: match self.phase {
                Phase::Question { .. } => None,
                _ => self
                    .next_deadline()
                    .map(|d| millis(d.saturating_duration_since(now))),
            },
            phase,
        }
    }

    /// Every team's points on every question played so far.
    #[must_use]
    pub fn results(&self) -> GameResults {
        let questions: Vec<usize> = (0..self.quiz.questions.len())
            .filter(|&i| !self.quiz.questions[i].is_slide())
            .collect();
        let closed = self.closed_count();
        GameResults {
            title: self.quiz.title.clone(),
            questions: questions
                .iter()
                .map(|&i| self.quiz.questions[i].text.trim().to_owned())
                .collect(),
            rows: self
                .standings_at(closed)
                .into_iter()
                .filter_map(|row| {
                    let team = self.teams.iter().find(|t| t.id == row.team.id)?;
                    Some(ResultRow {
                        rank: row.rank,
                        team: team.name.clone(),
                        score: row.score,
                        points: questions
                            .iter()
                            .map(|&i| {
                                (i < closed && team.submissions[i].is_some())
                                    .then(|| self.outcome(i, team).points)
                            })
                            .collect(),
                        jokers: questions.iter().map(|&i| team.joker == Some(i)).collect(),
                    })
                })
                .collect(),
        }
    }

    /// What a team leader's phone shows, or `None` for an unknown team.
    #[must_use]
    pub fn player_view(&self, team_id: u32, now: Instant) -> Option<PlayerView> {
        let team = self.teams.iter().find(|t| t.id == team_id)?;
        let total = self.quiz.question_count();
        // Phones count questions only.
        let number = |index: usize| self.quiz.ordinal(index);
        let submitted = |index: usize| team.submissions[index].as_ref().map(|s| s.response.clone());
        let outcome = |index: usize| self.outcome(index, team);
        let revealing = self.quiz.settings.reveal == RevealMode::AfterEachQuestion;
        let rank = self
            .standings()
            .into_iter()
            .find(|row| row.team.id == team_id)
            .map_or(0, |row| row.rank);
        let mut score_visible = revealing;
        let phase = match self.phase {
            Phase::Lobby => PlayerPhase::Lobby {
                teams: self.teams.len(),
            },
            Phase::Ready { index } => PlayerPhase::Ready {
                index: number(index),
                total,
            },
            Phase::Slide { .. } => PlayerPhase::Slide,
            Phase::Question {
                index, deadline, ..
            } => self.open_question_view(index, deadline, team, now),
            Phase::Closed { index, .. } => PlayerPhase::Closed {
                index: number(index),
                total,
                submitted: submitted(index),
                result: revealing
                    .then(|| (outcome(index), self.quiz.questions[index].correct_indexes())),
            },
            Phase::Review { index, .. } => PlayerPhase::Review {
                index: number(index),
                total,
                submitted: submitted(index),
                outcome: outcome(index),
                correct: self.quiz.questions[index].correct_indexes(),
            },
            Phase::Standings { .. } => {
                score_visible = true;
                PlayerPhase::Standings {
                    rank,
                    teams: self.teams.len(),
                }
            }
            Phase::Podium { revealed, .. } => {
                let visible = rank > self.podium_threshold(revealed);
                score_visible |= visible;
                PlayerPhase::Podium {
                    rank: visible.then_some(rank),
                    teams: self.teams.len(),
                    finished: revealed == self.podium_steps(),
                }
            }
        };
        let current = match self.phase {
            Phase::Question { index, .. }
            | Phase::Closed { index, .. }
            | Phase::Review { index, .. } => Some(index),
            _ => None,
        };
        let joker = match team.joker {
            _ if !self.quiz.settings.joker => JokerState::Unavailable,
            None => JokerState::Ready,
            Some(index) if Some(index) == current => JokerState::PlayedHere,
            Some(_) => JokerState::Spent,
        };
        Some(PlayerView {
            title: self.quiz.title.clone(),
            team: team.info(),
            joker,
            display: self.quiz.settings.phone_display,
            score: score_visible.then(|| self.team_score(team, self.closed_count())),
            phase,
        })
    }

    /// What a phone shows while question `index` is open.
    fn open_question_view(
        &self,
        index: usize,
        deadline: Instant,
        team: &Team,
        now: Instant,
    ) -> PlayerPhase {
        let question = &self.quiz.questions[index];
        let input = input_kind(question);
        let full_text = self.quiz.settings.phone_display == crate::model::PhoneDisplay::FullText;
        PlayerPhase::Question {
            index: self.quiz.ordinal(index),
            total: self.quiz.question_count(),
            input,
            answer_count: match input {
                InputKind::Choice => question.choices().len(),
                InputKind::Order => self.shown[index].len(),
                InputKind::Parts => question_parts(question),
                InputKind::Text | InputKind::Number | InputKind::Pin => 0,
            },
            // Order items, True/False and part labels cannot be told apart by colour alone:
            // always sent.
            items: match question.kind {
                QuestionKind::Order { .. }
                | QuestionKind::TrueFalse { .. }
                | QuestionKind::Parts { .. } => self.shown_answers(index),
                _ => Vec::new(),
            },
            text: full_text.then(|| (question.text.clone(), self.shown_answers(index))),
            answer_images: if full_text {
                answer_images(question)
            } else {
                Vec::new()
            },
            map: match question.kind {
                QuestionKind::Pin { aspect, .. } => {
                    question.image.clone().map(|image| (image, aspect))
                }
                _ => None,
            },
            multi: question.is_multi(),
            remaining_ms: millis(deadline.saturating_duration_since(self.clock(now))),
            paused: self.paused_at.is_some(),
            time_limit_ms: u64::from(question.time_limit_secs(&self.quiz.settings)) * 1000,
            submitted: team.submissions[index].as_ref().map(|s| s.response.clone()),
            bonus: self
                .active_bonus(index)
                .map(|b| (b.text.trim().to_owned(), b.percent)),
        }
    }

    fn team_mut(&mut self, id: u32) -> Option<&mut Team> {
        self.teams.iter_mut().find(|t| t.id == id)
    }

    /// The phase that shows item `index`: a slide, a question waiting for its soundtrack, or a
    /// question open right away.
    fn item_phase(&self, index: usize, now: Instant) -> Phase {
        let item = &self.quiz.questions[index];
        if item.is_results() {
            match self.first_question_in(self.part_start(index), index) {
                Some(index) => Phase::Review { index, since: now },
                None => Phase::Standings { index, since: now },
            }
        } else if item.is_slide() {
            Phase::Slide { index, since: now }
        } else if self.played[index] {
            Phase::Closed { index, since: now }
        } else if item.audio.is_some() {
            Phase::Ready { index }
        } else {
            self.open_question(index, now)
        }
    }

    /// Opens question `index` for answers, starting its clock.
    fn open_question(&self, index: usize, now: Instant) -> Phase {
        let secs = self.quiz.questions[index].time_limit_secs(&self.quiz.settings);
        Phase::Question {
            index,
            started: now,
            deadline: now + Duration::from_secs(u64::from(secs)),
        }
    }

    /// The first question among items `start..end`, skipping slides.
    fn first_question_in(&self, start: usize, end: usize) -> Option<usize> {
        (start..end.min(self.quiz.questions.len())).find(|&i| !self.quiz.questions[i].is_slide())
    }

    /// The last question among items `start..end`, skipping slides.
    fn last_question_in(&self, start: usize, end: usize) -> Option<usize> {
        (start..end.min(self.quiz.questions.len()))
            .rev()
            .find(|&i| !self.quiz.questions[i].is_slide())
    }

    /// Where the part of the quiz holding item `index` starts: just after the results slide
    /// before it, or at the beginning.
    fn part_start(&self, index: usize) -> usize {
        (0..index)
            .rev()
            .find(|&i| self.quiz.questions[i].is_results())
            .map_or(0, |i| i + 1)
    }

    /// Where the part of the quiz holding item `index` ends: at the results slide at or after it,
    /// or at the end of the quiz.
    fn part_end(&self, index: usize) -> usize {
        let len = self.quiz.questions.len();
        (index..len)
            .find(|&i| self.quiz.questions[i].is_results())
            .unwrap_or(len)
    }

    /// Where "next" leads after question or slide `index` is done.
    fn upcoming_after(&self, index: usize) -> Upcoming {
        let len = self.quiz.questions.len();
        match self.quiz.questions.get(index + 1) {
            Some(item) if item.is_results() => Upcoming::Results,
            Some(item) if item.is_slide() => Upcoming::Slide,
            Some(_) => Upcoming::Question,
            None if self.quiz.settings.reveal == RevealMode::AtEnd
                && self.first_question_in(self.part_start(len), len).is_some() =>
            {
                Upcoming::Answers
            }
            None => Upcoming::Podium,
        }
    }

    fn slide_info(&self, index: usize) -> SlideInfo {
        let item = &self.quiz.questions[index];
        SlideInfo {
            title: match &item.kind {
                QuestionKind::Slide { title, .. } => title.trim().to_owned(),
                _ => String::new(),
            },
            text: item.text.trim().to_owned(),
            image: item.image.clone(),
        }
    }

    fn question_info(&self, index: usize) -> QuestionInfo {
        let question: &Question = &self.quiz.questions[index];
        QuestionInfo {
            index,
            number: self.quiz.ordinal(index) + 1,
            total: self.quiz.question_count(),
            text: question.text.clone(),
            image: question.image.clone(),
            image_reveal: if question.reveals_image() {
                question.image_reveal
            } else {
                crate::model::ImageReveal::Shown
            },
            image_hold_ms: u64::from(question.image_hold_secs) * 1000,
            multiplier: question.multiplier(),
            audio: question.audio.clone(),
            input: input_kind(question),
            answers: self.shown_answers(index),
            answer_images: answer_images(question),
            multi: question.is_multi(),
            time_limit_ms: u64::from(question.time_limit_secs(&self.quiz.settings)) * 1000,
            bonus: self
                .active_bonus(index)
                .map(|b| (b.text.trim().to_owned(), b.percent)),
        }
    }

    /// Question `index`'s bonus question, if it has one it can carry.
    fn active_bonus(&self, index: usize) -> Option<&Bonus> {
        let question = &self.quiz.questions[index];
        question.bonus.as_ref().filter(|_| question.takes_bonus())
    }

    /// The bonus question's typed answers to question `index`, grouped, if it has one.
    fn bonus_reveal(&self, index: usize) -> Option<PartReveal> {
        let bonus = self.active_bonus(index)?;
        let mut groups: Vec<TextGroup> = Vec::new();
        for team in &self.teams {
            let Some(text) = team.submissions[index]
                .as_ref()
                .and_then(|s| s.bonus.as_ref())
            else {
                continue;
            };
            let key = bonus_key(text);
            if let Some(group) = groups.iter_mut().find(|g| g.key == key) {
                group.teams.push(team.name.clone());
            } else {
                let (judgement, amended) = self.judgement(index, &key);
                groups.push(TextGroup {
                    key,
                    text: text.clone(),
                    teams: vec![team.name.clone()],
                    judgement,
                    amended,
                });
            }
        }
        groups.sort_by_key(|g| std::cmp::Reverse(g.teams.len()));
        Some(PartReveal {
            label: "Bonus answers".to_owned(),
            accepted: bonus.accepted.clone(),
            groups,
        })
    }

    fn answered(&self, index: usize) -> usize {
        self.teams
            .iter()
            .filter(|t| t.submissions[index].is_some())
            .count()
    }

    fn reveal(&self, index: usize) -> Reveal {
        let question = &self.quiz.questions[index];
        let responses = || {
            self.teams.iter().filter_map(move |t| {
                t.submissions[index]
                    .as_ref()
                    .map(|s| (t.name.as_str(), &s.response))
            })
        };
        match &question.kind {
            QuestionKind::Slide { .. } => Reveal::Choice {
                correct: Vec::new(),
                picks: Vec::new(),
            },
            QuestionKind::Order { items } => {
                let mut placed = vec![0; items.len()];
                for (_, response) in responses() {
                    if let Response::Order(order) = response {
                        for (position, &shown) in order.iter().enumerate() {
                            if self.shown[index].get(shown) == Some(&position) {
                                placed[position] += 1;
                            }
                        }
                    }
                }
                Reveal::Order {
                    items: items.clone(),
                    placed,
                }
            }
            QuestionKind::Choice | QuestionKind::TrueFalse { .. } => {
                let mut picks = vec![0; question.choices().len()];
                for (_, response) in responses() {
                    if let Response::Choice(selected) = response {
                        for &i in selected {
                            picks[i] += 1;
                        }
                    }
                }
                Reveal::Choice {
                    correct: question.correct_indexes(),
                    picks,
                }
            }
            QuestionKind::Text { accepted } => Reveal::Text {
                accepted: tidy(accepted),
                groups: self.text_groups(index, None),
            },
            QuestionKind::Pin { answer, aspect, .. } => {
                self.pin_reveal(index, answer.unwrap_or_default(), *aspect)
            }
            QuestionKind::Parts { parts } => Reveal::Parts {
                parts: parts
                    .iter()
                    .enumerate()
                    .map(|(p, part)| PartReveal {
                        label: part_label(part, p),
                        accepted: tidy(&part.accepted),
                        groups: self.text_groups(index, Some(p)),
                    })
                    .collect(),
            },
            QuestionKind::Number { answer } => {
                let answer = answer.unwrap_or_default();
                let closest = self.closest_distance(index);
                let mut guesses: Vec<(f64, Guess)> = responses()
                    .filter_map(|(team, response)| match response {
                        Response::Number(value) => Some((
                            (value - answer).abs(),
                            Guess {
                                team: team.to_owned(),
                                value: *value,
                                closest: closest.is_some_and(|c| (value - answer).abs() <= c),
                            },
                        )),
                        _ => None,
                    })
                    .collect();
                guesses.sort_by(|a, b| a.0.total_cmp(&b.0));
                Reveal::Number {
                    answer,
                    guesses: guesses.into_iter().map(|(_, g)| g).collect(),
                }
            }
        }
    }

    /// Answer texts as shown for question `index`: the options of a multiple-choice question,
    /// the shuffled items of an order question, else none.
    fn shown_answers(&self, index: usize) -> Vec<String> {
        let question = &self.quiz.questions[index];
        match &question.kind {
            QuestionKind::Choice | QuestionKind::TrueFalse { .. } => question.choices(),
            QuestionKind::Order { items } => self.shown[index]
                .iter()
                .map(|&i| items[i].clone())
                .collect(),
            QuestionKind::Parts { parts } => parts
                .iter()
                .enumerate()
                .map(|(p, part)| part_label(part, p))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// How many items of an order answer (indexes into the shown list) are in their place.
    fn in_place(&self, index: usize, order: &[usize]) -> usize {
        order
            .iter()
            .enumerate()
            .filter(|&(position, &shown)| self.shown[index].get(shown) == Some(&position))
            .count()
    }

    /// The verdict on typed answers normalising to `key`, and whether the judge amended it.
    fn judgement(&self, index: usize, key: &str) -> (Judgement, bool) {
        match self.judgements[index].get(key) {
            Some(&judgement) => (judgement, true),
            None => (self.automatic_judgement(index, key), false),
        }
    }

    /// Every pin on question `index`, closest to `answer` first.
    fn pin_reveal(&self, index: usize, answer: crate::model::Point, aspect: f64) -> Reveal {
        let mut pins: Vec<PinGuess> = self
            .teams
            .iter()
            .filter_map(|team| match team.submissions[index].as_ref()?.response {
                Response::Pin(at) => Some(PinGuess {
                    team: team.name.clone(),
                    at,
                    distance: at.distance(answer, aspect),
                    points: self.outcome(index, team).points,
                }),
                _ => None,
            })
            .collect();
        pins.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        Reveal::Pin {
            answer,
            aspect,
            pins,
        }
    }

    /// Typed answers to question `index` grouped by normalised text, most common first; `part`
    /// picks one part of a several-answers question (blank answers are left out).
    fn text_groups(&self, index: usize, part: Option<usize>) -> Vec<TextGroup> {
        let mut groups: Vec<TextGroup> = Vec::new();
        for team in &self.teams {
            let Some(submission) = &team.submissions[index] else {
                continue;
            };
            let text = match (&submission.response, part) {
                (Response::Text(text), None) => text,
                (Response::Parts(texts), Some(p)) => match texts.get(p) {
                    Some(text) if !normalize(text).is_empty() => text,
                    _ => continue,
                },
                _ => continue,
            };
            let key = text_key(part, text);
            if let Some(group) = groups.iter_mut().find(|g| g.key == key) {
                group.teams.push(team.name.clone());
            } else {
                let (judgement, amended) = self.judgement(index, &key);
                groups.push(TextGroup {
                    key,
                    text: text.clone(),
                    teams: vec![team.name.clone()],
                    judgement,
                    amended,
                });
            }
        }
        // The sort is stable, so ties keep answer order.
        groups.sort_by_key(|g| std::cmp::Reverse(g.teams.len()));
        groups
    }

    /// Full points when `key` matches an accepted answer, none otherwise.
    fn automatic_judgement(&self, index: usize, key: &str) -> Judgement {
        let accepts = |accepted: &[String], norm: &str| {
            !norm.is_empty() && accepted.iter().any(|a| normalize(a) == norm)
        };
        let bonus = key
            .strip_prefix(BONUS_KEY_PREFIX)
            .zip(self.active_bonus(index));
        let matches = match &self.quiz.questions[index].kind {
            _ if key.starts_with(BONUS_KEY_PREFIX) => {
                bonus.is_some_and(|(norm, bonus)| accepts(&bonus.accepted, norm))
            }
            QuestionKind::Text { accepted } => accepts(accepted, key),
            QuestionKind::Parts { parts } => key
                .split_once(PART_KEY_SEPARATOR)
                .and_then(|(p, norm)| Some((parts.get(p.parse::<usize>().ok()?)?, norm)))
                .is_some_and(|(part, norm)| accepts(&part.accepted, norm)),
            _ => false,
        };
        Judgement {
            grade: if matches { Grade::Full } else { Grade::Wrong },
            bonus: 0,
        }
    }

    /// Distance of the closest guess on number question `index`, if anyone guessed.
    fn closest_distance(&self, index: usize) -> Option<f64> {
        let QuestionKind::Number {
            answer: Some(answer),
        } = self.quiz.questions[index].kind
        else {
            return None;
        };
        self.teams
            .iter()
            .filter_map(|t| match t.submissions[index].as_ref()?.response {
                Response::Number(value) => Some((value - answer).abs()),
                _ => None,
            })
            .min_by(f64::total_cmp)
    }

    /// How `team` did on question `index`, its joker included.
    fn outcome(&self, index: usize, team: &Team) -> AnswerOutcome {
        let mut outcome = self.base_outcome(index, team);
        let multiplier = i32::try_from(self.quiz.questions[index].multiplier()).unwrap_or(1);
        outcome.points = outcome.points.saturating_mul(multiplier);
        if team.joker == Some(index) {
            AnswerOutcome {
                points: outcome.points.saturating_mul(2),
                joker: true,
                ..outcome
            }
        } else {
            outcome
        }
    }

    /// How `team` did on question `index`, its bonus question included, before the question's
    /// multiplier and any joker.
    fn base_outcome(&self, index: usize, team: &Team) -> AnswerOutcome {
        let mut outcome = self.main_outcome(index, team);
        if let (Some(bonus), Some(submission)) =
            (self.active_bonus(index), team.submissions[index].as_ref())
            && let Some(text) = &submission.bonus
        {
            let question = &self.quiz.questions[index];
            let limit =
                Duration::from_secs(u64::from(question.time_limit_secs(&self.quiz.settings)));
            // Speed means nothing for numbers: the closest guesses earn a set amount.
            let full = match question.kind {
                QuestionKind::Number { .. } => {
                    u32::try_from(scoring::CLOSEST_POINTS).unwrap_or(scoring::MAX_POINTS)
                }
                _ => scoring::full_points(self.quiz.settings.points, submission.elapsed, limit),
            };
            let (judgement, _) = self.judgement(index, &bonus_key(text));
            outcome.points = outcome
                .points
                .saturating_add(scoring::text_points(judgement, full * bonus.percent / 100));
        }
        outcome
    }

    /// How `team` did on question `index` itself, before the question's multiplier and any joker.
    fn main_outcome(&self, index: usize, team: &Team) -> AnswerOutcome {
        let Some(submission) = &team.submissions[index] else {
            return AnswerOutcome {
                points: 0,
                fully_correct: false,
                joker: false,
            };
        };
        let question = &self.quiz.questions[index];
        let limit = Duration::from_secs(u64::from(question.time_limit_secs(&self.quiz.settings)));
        let full = scoring::full_points(self.quiz.settings.points, submission.elapsed, limit);
        match (&question.kind, &submission.response) {
            (QuestionKind::Choice | QuestionKind::TrueFalse { .. }, Response::Choice(selected)) => {
                let mode = self.quiz.settings.multi_scoring;
                let points = scoring::score(question, selected, full, mode);
                AnswerOutcome {
                    points: i32::try_from(points).unwrap_or(i32::MAX),
                    fully_correct: scoring::correctness(question, selected, mode) >= 1.0,
                    joker: false,
                }
            }
            (QuestionKind::Text { .. }, Response::Text(text)) => {
                let (judgement, _) = self.judgement(index, &text_key(None, text));
                AnswerOutcome {
                    points: scoring::text_points(judgement, full),
                    fully_correct: judgement.grade == Grade::Full,
                    joker: false,
                }
            }
            (
                QuestionKind::Number {
                    answer: Some(answer),
                },
                Response::Number(value),
            ) => {
                let closest = self
                    .closest_distance(index)
                    .is_some_and(|c| (value - answer).abs() <= c);
                AnswerOutcome {
                    points: if closest { scoring::CLOSEST_POINTS } else { 0 },
                    fully_correct: closest,
                    joker: false,
                }
            }
            (QuestionKind::Parts { parts }, Response::Parts(texts)) => {
                // The question's points are split evenly between its parts.
                let judgements: Vec<Judgement> = texts
                    .iter()
                    .enumerate()
                    .filter(|(_, text)| !normalize(text).is_empty())
                    .map(|(p, text)| self.judgement(index, &text_key(Some(p), text)).0)
                    .collect();
                AnswerOutcome {
                    points: scoring::parts_points(&judgements, parts.len(), full),
                    fully_correct: judgements.len() == parts.len()
                        && judgements.iter().all(|j| j.grade == Grade::Full),
                    joker: false,
                }
            }
            (
                QuestionKind::Pin {
                    answer: Some(answer),
                    aspect,
                    tolerance,
                },
                Response::Pin(at),
            ) => {
                let distance = at.distance(*answer, *aspect);
                AnswerOutcome {
                    points: i32::try_from(scoring::pin_points(full, distance, *tolerance))
                        .unwrap_or(i32::MAX),
                    fully_correct: distance <= tolerance.distances().0,
                    joker: false,
                }
            }
            (QuestionKind::Order { items }, Response::Order(order)) => {
                let in_place = self.in_place(index, order);
                AnswerOutcome {
                    points: i32::try_from(scoring::order_points(full, in_place, items.len()))
                        .unwrap_or(i32::MAX),
                    fully_correct: in_place == items.len(),
                    joker: false,
                }
            }
            _ => AnswerOutcome {
                points: 0,
                fully_correct: false,
                joker: false,
            },
        }
    }

    /// Points from the first `closed` questions; open questions do not count yet.
    fn team_score(&self, team: &Team, closed: usize) -> i32 {
        (0..closed).map(|i| self.outcome(i, team).points).sum()
    }

    /// How many questions, from the start, have closed and therefore count towards scores.
    fn closed_count(&self) -> usize {
        match self.phase {
            Phase::Lobby => 0,
            Phase::Ready { index } | Phase::Question { index, .. } => index,
            Phase::Closed { index, .. }
            | Phase::Slide { index, .. }
            | Phase::Standings { index, .. } => index + 1,
            Phase::Review { index, .. } => self.part_end(index),
            Phase::Podium { .. } => self.quiz.questions.len(),
        }
    }

    fn standings(&self) -> Vec<StandingRow> {
        self.standings_at(self.closed_count())
    }

    /// Standings once question `index` closed, with each team's result on it and rank change.
    fn standings_after(&self, index: usize) -> Vec<StandingRow> {
        let before = (self.quiz.ordinal(index) > 0).then(|| self.standings_at(index));
        let mut rows = self.standings_at(index + 1);
        for row in &mut rows {
            let Some(team) = self.teams.iter().find(|t| t.id == row.team.id) else {
                continue;
            };
            let outcome = self.outcome(index, team);
            row.round = Some(RoundResult {
                points: outcome.points,
                joker: outcome.joker,
                answer_ms: team.submissions[index].as_ref().map(|s| millis(s.elapsed)),
                previous_rank: before
                    .as_ref()
                    .and_then(|rows| rows.iter().find(|r| r.team.id == row.team.id))
                    .map(|r| r.rank),
            });
        }
        rows
    }

    /// Standings at results slide `index`, with the points each team gained since the previous
    /// results slide and its rank then.
    fn standings_of_part(&self, index: usize) -> Vec<StandingRow> {
        let start = self.part_start(index);
        let before = (start > 0).then(|| self.standings_at(start));
        let mut rows = self.standings_at(index + 1);
        for row in &mut rows {
            let previous = before
                .as_ref()
                .and_then(|rows| rows.iter().find(|r| r.team.id == row.team.id));
            row.round = Some(RoundResult {
                points: row.score - previous.map_or(0, |r| r.score),
                joker: false,
                answer_ms: None,
                previous_rank: previous.map(|r| r.rank),
            });
        }
        rows
    }

    /// The quickest fully correct answer to question `index`; speed means nothing for numbers.
    fn fastest(&self, index: usize) -> Option<Fastest> {
        if matches!(self.quiz.questions[index].kind, QuestionKind::Number { .. }) {
            return None;
        }
        self.teams
            .iter()
            .filter(|t| self.outcome(index, t).fully_correct)
            .filter_map(|t| Some((t, t.submissions[index].as_ref()?.elapsed)))
            .min_by_key(|(_, elapsed)| *elapsed)
            .map(|(t, elapsed)| Fastest {
                team: t.name.clone(),
                answer_ms: millis(elapsed),
            })
    }

    /// Standings counting the first `closed` items.
    fn standings_at(&self, closed: usize) -> Vec<StandingRow> {
        let mut tallies: Vec<Tally> = self
            .teams
            .iter()
            .enumerate()
            .map(|(joined, t)| Tally {
                team: t.id,
                score: self.team_score(t, closed),
                scoring_time: (0..closed)
                    .filter(|&i| self.outcome(i, t).points > 0)
                    .filter_map(|i| t.submissions[i].as_ref())
                    .map(|s| s.elapsed)
                    .sum(),
                joined,
            })
            .collect();
        scoring::rank(&mut tallies);
        tallies
            .iter()
            .enumerate()
            .filter_map(|(i, tally)| {
                let team = self.teams.iter().find(|t| t.id == tally.team)?;
                Some(StandingRow {
                    rank: i + 1,
                    team: team.info(),
                    score: tally.score,
                    round: None,
                })
            })
            .collect()
    }

    /// Number of podium spots revealed one by one.
    fn podium_steps(&self) -> usize {
        self.teams.len().min(3)
    }

    /// Ranks strictly above this are visible after `revealed` podium steps.
    fn podium_threshold(&self, revealed: usize) -> usize {
        self.podium_steps() - revealed
    }
}

impl Team {
    fn info(&self) -> TeamInfo {
        TeamInfo {
            id: self.id,
            name: self.name.clone(),
            avatar: self.avatar.clone(),
            connected: self.connections > 0,
        }
    }
}

/// Checks an answer against the question and tidies it up.
fn check_response(question: &Question, response: Response) -> Result<Response, Refusal> {
    match (&question.kind, response) {
        (QuestionKind::Choice | QuestionKind::TrueFalse { .. }, Response::Choice(mut selected)) => {
            selected.sort_unstable();
            selected.dedup();
            if selected.is_empty() || selected.iter().any(|&i| i >= question.choices().len()) {
                return Err(Refusal("Please pick an answer."));
            }
            if !question.is_multi() && selected.len() > 1 {
                return Err(Refusal("Only one answer can be picked."));
            }
            Ok(Response::Choice(selected))
        }
        (QuestionKind::Text { .. }, Response::Text(text)) => {
            let text = text.trim();
            if normalize(text).is_empty() {
                return Err(Refusal("Please type an answer."));
            }
            if text.chars().count() > MAX_TEXT_ANSWER_CHARS {
                return Err(Refusal("That answer is too long."));
            }
            Ok(Response::Text(text.to_owned()))
        }
        (QuestionKind::Number { .. }, Response::Number(value)) if value.is_finite() => {
            Ok(Response::Number(value))
        }
        (QuestionKind::Number { .. }, Response::Number(_)) => {
            Err(Refusal("Please enter a number."))
        }
        (QuestionKind::Parts { parts }, Response::Parts(texts)) => {
            if texts.len() != parts.len() {
                return Err(Refusal("That answer does not fit this question."));
            }
            // A part with nothing but punctuation counts as blank, as for a typed answer.
            let texts: Vec<String> = texts
                .iter()
                .map(|t| {
                    if normalize(t).is_empty() {
                        String::new()
                    } else {
                        t.trim().to_owned()
                    }
                })
                .collect();
            if texts.iter().all(|t| normalize(t).is_empty()) {
                return Err(Refusal("Please type at least one answer."));
            }
            if texts
                .iter()
                .any(|t| t.chars().count() > MAX_TEXT_ANSWER_CHARS)
            {
                return Err(Refusal("One of the answers is too long."));
            }
            Ok(Response::Parts(texts))
        }
        (QuestionKind::Pin { .. }, Response::Pin(at)) if at.is_on_picture() => {
            Ok(Response::Pin(at))
        }
        (QuestionKind::Pin { .. }, Response::Pin(_)) => {
            Err(Refusal("Drop your pin on the picture."))
        }
        (QuestionKind::Order { items }, Response::Order(order)) => {
            let mut sorted = order.clone();
            sorted.sort_unstable();
            if sorted != (0..items.len()).collect::<Vec<_>>() {
                return Err(Refusal("Put every item in order."));
            }
            Ok(Response::Order(order))
        }
        _ => Err(Refusal("That answer does not fit this question.")),
    }
}

fn input_kind(question: &Question) -> InputKind {
    match question.kind {
        QuestionKind::Choice | QuestionKind::TrueFalse { .. } | QuestionKind::Slide { .. } => {
            InputKind::Choice
        }
        QuestionKind::Text { .. } => InputKind::Text,
        QuestionKind::Number { .. } => InputKind::Number,
        QuestionKind::Order { .. } => InputKind::Order,
        QuestionKind::Parts { .. } => InputKind::Parts,
        QuestionKind::Pin { .. } => InputKind::Pin,
    }
}

/// Answer pictures of a multiple-choice question, parallel to its answer texts.
fn answer_images(question: &Question) -> Vec<Option<String>> {
    match &question.kind {
        QuestionKind::Choice => question.answers.iter().map(|a| a.image.clone()).collect(),
        QuestionKind::TrueFalse { .. } => vec![None, None],
        QuestionKind::Parts { parts } => parts.iter().map(|p| p.image.clone()).collect(),
        _ => Vec::new(),
    }
}

/// Separates a part's index from the normalised text in judgement keys of several-answers
/// questions (normalised text has letters, digits and spaces only).
const PART_KEY_SEPARATOR: char = '|';
/// Prefix of the judgement keys of bonus answers (parts use their index instead).
const BONUS_KEY_PREFIX: &str = "bonus|";

/// The judgement key of a typed bonus answer.
fn bonus_key(text: &str) -> String {
    format!("{BONUS_KEY_PREFIX}{}", normalize(text))
}

/// The judgement key of a typed answer: its normalised text, prefixed by the part's index for a
/// several-answers question.
fn text_key(part: Option<usize>, text: &str) -> String {
    match part {
        Some(p) => format!("{p}{PART_KEY_SEPARATOR}{}", normalize(text)),
        None => normalize(text),
    }
}

/// A part's label, or "Answer N" when it has none.
fn part_label(part: &crate::model::Part, index: usize) -> String {
    let label = part.label.trim();
    if label.is_empty() {
        format!("Answer {}", index + 1)
    } else {
        label.to_owned()
    }
}

/// How many parts a several-answers question has (0 for other kinds).
fn question_parts(question: &Question) -> usize {
    match &question.kind {
        QuestionKind::Parts { parts } => parts.len(),
        _ => 0,
    }
}

/// Accepted answers as shown: trimmed, blanks left out.
fn tidy(accepted: &[String]) -> Vec<String> {
    accepted
        .iter()
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect()
}

/// A random order of `0..n` that differs from the original one when it can.
fn shuffled(n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let random = uuid::Uuid::new_v4().as_u128();
        let j = usize::try_from(random % (i as u128 + 1)).unwrap_or(0);
        order.swap(i, j);
    }
    if n > 1 && order.iter().enumerate().all(|(i, &v)| i == v) {
        order.rotate_left(1);
    }
    order
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, QuizSettings};

    fn quiz(settings: QuizSettings) -> Quiz {
        let question = |text: &str| Question {
            text: text.to_owned(),
            kind: crate::model::QuestionKind::Choice,
            image: None,
            image_reveal: crate::model::ImageReveal::Shown,
            image_hold_secs: 0,
            audio: None,
            time_limit_secs: Some(10),
            multiplier: None,
            answers: vec![
                Answer {
                    text: "right".to_owned(),
                    correct: true,
                    image: None,
                },
                Answer {
                    text: "wrong".to_owned(),
                    correct: false,
                    image: None,
                },
            ],
            bonus: None,
        };
        Quiz {
            settings,
            questions: vec![question("one"), question("two")],
            ..Quiz::new("id".to_owned(), "Test".to_owned())
        }
    }

    fn game(settings: QuizSettings) -> Game {
        Game::new("123456".to_owned(), quiz(settings)).unwrap()
    }

    fn join(game: &mut Game, name: &str) -> u32 {
        let id = game.join(name, format!("token-{name}")).unwrap();
        game.set_connected(id, true);
        id
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    fn pick(selected: &[usize]) -> Response {
        Response::Choice(selected.to_vec())
    }

    #[test]
    fn invalid_quizzes_cannot_start_a_game() {
        let empty = Quiz::new("id".to_owned(), "Empty".to_owned());
        assert!(Game::new("1".to_owned(), empty).is_err());
    }

    #[test]
    fn the_host_sets_how_many_teams_can_join() {
        let mut g = game(QuizSettings::default());
        assert_eq!(g.host_view(Instant::now()).max_teams, DEFAULT_MAX_TEAMS);
        assert!(g.set_max_teams(0).is_err());
        assert!(g.set_max_teams(MAX_TEAMS_CEILING + 1).is_err());
        g.set_max_teams(2).unwrap();
        join(&mut g, "Owls");
        join(&mut g, "Cats");
        assert_eq!(
            g.join("Bats", "token-bats".to_owned()),
            Err(Refusal("This game is full."))
        );
        g.set_max_teams(MAX_TEAMS_CEILING).unwrap();
        join(&mut g, "Bats");
        // The limit survives a restart.
        let t0 = Instant::now();
        let restored = Game::restore(g.snapshot(t0), t0);
        assert_eq!(restored.host_view(t0).max_teams, MAX_TEAMS_CEILING);
    }

    #[test]
    fn join_validates_names() {
        let mut g = game(QuizSettings::default());
        assert!(g.join(" ", "t".to_owned()).is_err());
        assert!(g.join(&"x".repeat(25), "t".to_owned()).is_err());
        let id = g.join("  Owls ", "t1".to_owned()).unwrap();
        assert!(g.join("owls", "t2".to_owned()).is_err());
        assert_eq!(g.team_for_token("t1"), Some(id));
        assert_eq!(g.team_for_token("nope"), None);
    }

    #[test]
    fn the_host_renames_and_removes_teams() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        assert!(g.rename_team(owls, "cats").is_err());
        assert!(g.rename_team(owls, " ").is_err());
        g.rename_team(owls, " Night Owls ").unwrap();
        g.rename_team(cats, "CATS").unwrap();
        assert_eq!(g.host_view(t0).teams[0].name, "Night Owls");

        // Removing the only team still to answer closes the question.
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        g.remove_team(cats, t0).unwrap();
        assert!(g.remove_team(cats, t0).is_err());
        assert!(g.player_view(cats, t0).is_none());
        assert_eq!(g.team_for_token("token-Cats"), None);
        assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));

        g.next(t0).unwrap();
        g.next(t0).unwrap();
        g.next(t0).unwrap(); // podium
        assert!(g.remove_team(owls, t0).is_err());
    }

    #[test]
    fn pausing_freezes_the_clock_and_extra_time_extends_it() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let _cats = join(&mut g, "Cats");
        assert!(g.pause(t0).is_err());
        g.next(t0).unwrap();
        g.pause(t0 + secs(2)).unwrap();
        assert!(g.pause(t0 + secs(2)).is_err());
        assert_eq!(g.next_deadline(), None);
        let HostPhase::Question {
            remaining_ms,
            elapsed_ms,
            ..
        } = g.host_view(t0 + secs(30)).phase
        else {
            panic!("expected the question");
        };
        assert_eq!((remaining_ms, elapsed_ms), (8000, 2000));
        assert!(g.host_view(t0).paused);

        // Answers still count while paused, timed at the pause.
        g.answer(owls, pick(&[0]), false, t0 + secs(30)).unwrap();
        g.resume(t0 + secs(30));
        assert_eq!(g.next_deadline(), Some(t0 + secs(38)));
        g.add_time(secs(10)).unwrap();
        assert_eq!(g.next_deadline(), Some(t0 + secs(48)));
        assert!(!g.tick(t0 + secs(40)));
        assert!(g.tick(t0 + secs(48)));
        assert!(g.add_time(secs(10)).is_err());
        assert_eq!(g.standings()[0].score, 900);
    }

    #[test]
    fn going_back_revisits_without_replaying() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        assert!(g.back(t0).is_err());
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap(); // closes question one
        g.next(t0).unwrap();
        g.answer(owls, pick(&[1]), false, t0).unwrap(); // closes question two
        g.back(t0).unwrap(); // back to question one's results
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed { question, .. } if question.index == 0
        ));
        // Going forward shows question two's results again instead of asking it anew.
        g.next(t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed { question, .. } if question.index == 1
        ));

        // Leaving an open question withdraws it.
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let _cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        g.back(t0).unwrap();
        assert!(matches!(g.host_view(t0).phase, HostPhase::Lobby));
        g.next(t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Question { answered: 0, .. }
        ));

        // Podium steps go back one at a time, then to the last results.
        g.next(t0).unwrap();
        g.next(t0).unwrap();
        g.next(t0).unwrap();
        g.next(t0).unwrap(); // podium, nothing revealed
        g.next(t0).unwrap(); // 2nd place
        g.back(t0).unwrap();
        assert!(
            matches!(g.host_view(t0).phase, HostPhase::Podium { ref standings, .. } if standings.is_empty())
        );
        g.back(t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed { question, .. } if question.index == 1
        ));
    }

    #[test]
    fn reveals_show_the_fastest_team_and_rank_changes() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(cats, pick(&[1]), false, t0 + secs(1)).unwrap();
        g.answer(owls, pick(&[0]), false, t0 + secs(4)).unwrap();
        let HostPhase::Closed {
            fastest, standings, ..
        } = g.host_view(t0).phase
        else {
            panic!("expected results");
        };
        assert_eq!(
            fastest,
            Some(Fastest {
                team: "Owls".to_owned(),
                answer_ms: 4000
            })
        );
        let rows = standings.unwrap();
        assert_eq!(
            rows[0].round,
            Some(RoundResult {
                joker: false,
                points: 800,
                answer_ms: Some(4000),
                previous_rank: None
            })
        );

        // Cats overtake Owls on question two.
        g.next(t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap();
        g.answer(owls, pick(&[1]), false, t0).unwrap();
        let HostPhase::Closed {
            standings: Some(rows),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected results");
        };
        assert_eq!(rows[0].team.id, cats);
        assert_eq!(rows[0].round.unwrap().previous_rank, Some(2));
        assert_eq!(rows[1].round.unwrap().previous_rank, Some(1));
    }

    #[test]
    fn a_joker_doubles_one_question_once() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        // Jokers are off by default.
        assert!(g.answer(owls, pick(&[0]), true, t0).is_err());
        assert_eq!(
            g.player_view(owls, t0).unwrap().joker,
            JokerState::Unavailable
        );

        let mut g = game(QuizSettings {
            joker: true,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        assert_eq!(g.player_view(owls, t0).unwrap().joker, JokerState::Ready);
        g.answer(owls, pick(&[0]), true, t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap();
        assert_eq!(
            g.player_view(owls, t0).unwrap().joker,
            JokerState::PlayedHere
        );
        let rows = g.standings();
        assert_eq!((rows[0].team.id, rows[0].score), (owls, 2000));
        assert_eq!(rows[1].score, 1000);

        g.next(t0).unwrap();
        assert_eq!(g.player_view(owls, t0).unwrap().joker, JokerState::Spent);
        assert!(g.answer(owls, pick(&[0]), true, t0).is_err());
        // Withdrawing a question gives the joker back.
        g.answer(cats, pick(&[0]), true, t0).unwrap();
        g.back(t0).unwrap();
        assert_eq!(g.player_view(cats, t0).unwrap().joker, JokerState::Ready);
    }

    #[test]
    fn order_answers_score_each_item_in_place() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings::default());
        quiz.questions[0].kind = QuestionKind::Order {
            items: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
        };
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        assert_ne!(g.shown[0], vec![0, 1, 2]);
        // Shown as C, A, B.
        g.shown[0] = vec![2, 0, 1];
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let pigs = join(&mut g, "Pigs");
        g.next(t0).unwrap();
        let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
            panic!("expected the question");
        };
        assert_eq!(question.answers, ["C", "A", "B"]);
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Question { ref items, .. } if items == &["C", "A", "B"]
        ));
        assert!(
            g.answer(owls, Response::Order(vec![0, 0, 1]), false, t0)
                .is_err()
        );
        g.answer(owls, Response::Order(vec![1, 2, 0]), false, t0)
            .unwrap();
        g.answer(cats, Response::Order(vec![0, 1, 2]), false, t0)
            .unwrap();
        g.answer(pigs, Response::Order(vec![1, 0, 2]), false, t0)
            .unwrap();
        let HostPhase::Closed {
            reveal: Some(reveal),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected results");
        };
        assert_eq!(
            reveal,
            Reveal::Order {
                items: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
                placed: vec![2, 1, 1],
            }
        );
        let score = |team| {
            g.standings()
                .into_iter()
                .find(|r| r.team.id == team)
                .unwrap()
                .score
        };
        assert_eq!((score(owls), score(cats), score(pigs)), (1000, 0, 333));
    }

    #[test]
    fn results_list_every_teams_points_per_question() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings {
            joker: true,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats, \"the\"");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), true, t0).unwrap();
        g.answer(cats, pick(&[1]), false, t0).unwrap();
        g.next(t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap();
        g.next(t0).unwrap(); // close question two; Owls did not answer
        let results = g.results();
        assert_eq!(results.questions, ["one", "two"]);
        assert_eq!(results.rows[0].team, "Owls");
        assert_eq!(results.rows[0].points, [Some(2000), None]);
        assert_eq!(results.rows[0].jokers, [true, false]);
        assert_eq!(results.rows[1].points, [Some(0), Some(1000)]);
        assert_eq!(
            results.to_csv(),
            "Rank,Team,Total,one,two\r\n1,Owls,2000,2000,\r\n2,\"Cats, \"\"the\"\"\",1000,0,1000\r\n"
        );
    }

    #[test]
    fn a_restored_game_resumes_where_it_was_paused() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0 + secs(2)).unwrap();
        let json = serde_json::to_string(&g.snapshot(t0 + secs(4))).unwrap();

        let later = t0 + secs(600);
        let mut g = Game::restore(serde_json::from_str(&json).unwrap(), later);
        assert_eq!(g.code(), "123456");
        assert_eq!(g.team_for_token("token-Owls"), Some(owls));
        assert!(g.is_paused());
        let HostPhase::Question {
            remaining_ms,
            answered,
            ..
        } = g.host_view(later + secs(30)).phase
        else {
            panic!("expected the open question");
        };
        assert_eq!((remaining_ms, answered), (6000, 1));
        // Phones reconnect; nobody counts as connected until then.
        assert!(g.host_view(later).teams.iter().all(|t| !t.connected));
        g.set_connected(cats, true);
        g.resume(later);
        g.answer(cats, pick(&[0]), false, later + secs(1)).unwrap();
        // Cats answered 5 s into the question, pauses excluded.
        assert!(matches!(g.host_view(later).phase, HostPhase::Closed { .. }));
        let rows = g.standings();
        assert_eq!(rows[0].score, 900);
        assert_eq!(rows[1].score, 750);
    }

    #[test]
    fn a_multiplier_scales_points_before_the_joker() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            joker: true,
            ..QuizSettings::default()
        });
        quiz.questions[0].multiplier = Some(3);
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), true, t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap();
        let scores: Vec<i32> = g.standings().iter().map(|r| r.score).collect();
        assert_eq!(scores, [6000, 3000]);
        let HostPhase::Closed { question, .. } = g.host_view(t0).phase else {
            panic!("expected results");
        };
        assert_eq!(question.multiplier, 3);
    }

    #[test]
    fn true_false_plays_like_two_choices() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings::default());
        quiz.questions[0].kind = QuestionKind::TrueFalse { answer: false };
        quiz.questions[0].answers.clear();
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Question { answer_count: 2, ref items, .. } if items == &["True", "False"]
        ));
        assert!(g.answer(owls, pick(&[2]), false, t0).is_err());
        g.answer(owls, pick(&[1]), false, t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap();
        let HostPhase::Closed {
            question,
            reveal: Some(reveal),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected results");
        };
        assert_eq!(question.answers, ["True", "False"]);
        assert_eq!(
            reveal,
            Reveal::Choice {
                correct: vec![1],
                picks: vec![1, 1]
            }
        );
        assert_eq!(g.standings()[0].team.id, owls);
    }

    #[test]
    fn parts_are_judged_one_by_one_and_share_the_points() {
        use crate::model::Part;
        let t0 = Instant::now();
        let part = |label: &str, accepted: &[&str]| Part {
            label: label.to_owned(),
            accepted: accepted.iter().map(|a| (*a).to_owned()).collect(),
            image: None,
        };
        let mut quiz = quiz(QuizSettings {
            points: crate::model::PointsMode::Flat,
            ..QuizSettings::default()
        });
        quiz.questions[0].kind = QuestionKind::Parts {
            parts: vec![
                part("Title", &["Yesterday"]),
                part(" ", &["The Beatles", "Beatles"]),
            ],
        };
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let pigs = join(&mut g, "Pigs");
        g.next(t0).unwrap();
        let texts = |a: &str, b: &str| Response::Parts(vec![a.to_owned(), b.to_owned()]);
        assert!(
            g.answer(owls, Response::Parts(vec!["x".to_owned()]), false, t0)
                .is_err()
        );
        assert!(g.answer(owls, texts(" ", ""), false, t0).is_err());
        g.answer(owls, texts("yesterday", "beatles"), false, t0)
            .unwrap();
        g.answer(cats, texts("Yesterday", "Paul McCartney"), false, t0)
            .unwrap();
        g.answer(pigs, texts("Let it be", ""), false, t0).unwrap();

        let HostPhase::Closed {
            reveal: Some(Reveal::Parts { parts }),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected a parts reveal");
        };
        assert_eq!(parts[1].label, "Answer 2");
        assert_eq!(parts[0].groups.len(), 2);
        // The blank artist is left out of the groups.
        assert_eq!(parts[1].groups.len(), 2);
        let score = |g: &Game, team| {
            g.standings()
                .into_iter()
                .find(|r| r.team.id == team)
                .unwrap()
                .score
        };
        assert_eq!(
            (score(&g, owls), score(&g, cats), score(&g, pigs)),
            (1000, 500, 0)
        );

        // The judge gives Paul McCartney half the artist's points.
        let key = parts[1]
            .groups
            .iter()
            .find(|g| g.text == "Paul McCartney")
            .unwrap()
            .key
            .clone();
        let half = Judgement {
            grade: Grade::Half,
            bonus: 0,
        };
        g.judge(0, &key, half, t0).unwrap();
        assert_eq!(score(&g, cats), 750);
    }

    #[test]
    fn pins_score_by_distance() {
        use crate::model::{PinTolerance, Point};
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            points: crate::model::PointsMode::Flat,
            ..QuizSettings::default()
        });
        quiz.questions[0].image = Some("map.jpg".to_owned());
        quiz.questions[0].kind = QuestionKind::Pin {
            answer: Some(Point { x: 0.5, y: 0.5 }),
            aspect: 0.5,
            tolerance: PinTolerance::Normal,
        };
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let pigs = join(&mut g, "Pigs");
        g.next(t0).unwrap();
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Question { map: Some((ref m, aspect)), input: InputKind::Pin, .. } if m == "map.jpg" && (aspect - 0.5).abs() < f64::EPSILON
        ));
        let pin = |x, y| Response::Pin(Point { x, y });
        assert!(g.answer(owls, pin(1.5, 0.5), false, t0).is_err());
        g.answer(owls, pin(0.52, 0.5), false, t0).unwrap();
        // 0.34 of the height of a picture half as high as wide is 0.17 of its width.
        g.answer(cats, pin(0.5, 0.84), false, t0).unwrap();
        g.answer(pigs, pin(0.0, 0.0), false, t0).unwrap();
        let HostPhase::Closed {
            reveal: Some(Reveal::Pin { pins, .. }),
            fastest,
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected a pin reveal");
        };
        let points: Vec<(&str, i32)> = pins.iter().map(|p| (p.team.as_str(), p.points)).collect();
        assert_eq!(points, [("Owls", 1000), ("Cats", 500), ("Pigs", 0)]);
        assert_eq!(fastest.map(|f| f.team), Some("Owls".to_owned()));
    }

    #[test]
    fn cannot_start_without_teams() {
        let mut g = game(QuizSettings::default());
        assert!(g.next(Instant::now()).is_err());
    }

    #[test]
    fn full_game_with_reveal_after_each_question() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        assert_eq!(g.next_deadline(), Some(t0 + secs(10)));

        // Owls answer right instantly, Cats wrong; everyone answered closes the question.
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Question { answered: 1, .. }
        ));
        g.answer(cats, pick(&[1]), false, t0 + secs(1)).unwrap();
        let HostPhase::Closed {
            reveal: Some(reveal),
            standings: Some(standings),
            then: Upcoming::Question,
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected a revealed closed question");
        };
        assert_eq!(
            reveal,
            Reveal::Choice {
                correct: vec![0],
                picks: vec![1, 1]
            }
        );
        assert_eq!(standings[0].team.id, owls);
        assert_eq!(standings[0].score, 1000);
        let cats_view = g.player_view(cats, t0).unwrap();
        assert_eq!(cats_view.score, Some(0));
        assert!(matches!(
            cats_view.phase,
            PlayerPhase::Closed {
                result: Some((
                    AnswerOutcome {
                        points: 0,
                        fully_correct: false,
                        joker: false
                    },
                    _
                )),
                ..
            }
        ));

        // Manual advance: no deadline while closed.
        assert_eq!(g.next_deadline(), None);
        g.next(t0 + secs(2)).unwrap();

        // Nobody answers question two; the timer closes it.
        assert!(!g.tick(t0 + secs(11)));
        assert!(g.tick(t0 + secs(12)));
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                then: Upcoming::Podium,
                ..
            }
        ));

        // Podium: 2nd then 1st.
        g.next(t0 + secs(13)).unwrap();
        let podium = |g: &Game| match g.host_view(t0).phase {
            HostPhase::Podium {
                standings,
                finished,
            } => (
                standings.iter().map(|r| r.rank).collect::<Vec<_>>(),
                finished,
            ),
            other => panic!("expected podium, got {other:?}"),
        };
        assert_eq!(podium(&g), (vec![], false));
        assert_eq!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Podium {
                rank: None,
                teams: 2,
                finished: false
            }
        );
        g.next(t0).unwrap();
        assert_eq!(podium(&g), (vec![2], false));
        g.next(t0).unwrap();
        assert_eq!(podium(&g), (vec![1, 2], true));
        assert!(g.is_finished());
        assert_eq!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Podium {
                rank: Some(1),
                teams: 2,
                finished: true
            }
        );
        assert!(g.next(t0).is_err());
    }

    #[test]
    fn reveal_at_end_hides_results_then_reviews() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings {
            reveal: RevealMode::AtEnd,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0 + secs(5)).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                reveal: None,
                standings: None,
                ..
            }
        ));
        let view = g.player_view(owls, t0).unwrap();
        assert_eq!(view.score, None);
        assert!(matches!(
            view.phase,
            PlayerPhase::Closed { result: None, .. }
        ));

        g.next(t0).unwrap(); // question two
        g.next(t0).unwrap(); // close early
        g.next(t0).unwrap(); // review one
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Review {
                index: 0,
                outcome: AnswerOutcome {
                    points: 750,
                    fully_correct: true,
                    joker: false
                },
                ..
            }
        ));
        g.next(t0).unwrap(); // review two
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Review {
                then: Upcoming::Podium,
                ..
            }
        ));
        g.next(t0).unwrap(); // podium
        assert!(matches!(g.host_view(t0).phase, HostPhase::Podium { .. }));
    }

    /// Where the big screen is, in short: "Q1" (open), "C1" (closed), "S1", "R1" (review),
    /// "T1" (standings at a results slide), "P" (podium).
    fn at(g: &Game, t: Instant) -> String {
        match g.host_view(t).phase {
            HostPhase::Question { question, .. } => format!("Q{}", question.index),
            HostPhase::Closed { question, .. } => format!("C{}", question.index),
            HostPhase::Review { question, .. } => format!("R{}", question.index),
            HostPhase::Slide { .. } => "S".to_owned(),
            HostPhase::Standings { .. } => match g.phase {
                Phase::Standings { index, .. } => format!("T{index}"),
                _ => unreachable!(),
            },
            HostPhase::Podium { .. } => "P".to_owned(),
            HostPhase::Lobby => "L".to_owned(),
            HostPhase::Ready { .. } => "W".to_owned(),
        }
    }

    #[test]
    fn results_slides_reveal_each_part_then_the_standings() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            reveal: RevealMode::AtEnd,
            ..QuizSettings::default()
        });
        // one, two, [results], three, four
        let mut more = quiz.questions.clone();
        more[0].text = "three".to_owned();
        more[1].text = "four".to_owned();
        quiz.questions.push(Question::results_slide());
        quiz.questions.extend(more);
        let mut g = Game::new("123456".to_owned(), quiz.clone()).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");

        g.next(t0).unwrap();
        g.answer(owls, pick(&[1]), false, t0).unwrap();
        g.answer(cats, pick(&[0]), false, t0).unwrap(); // Cats lead
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        g.answer(cats, pick(&[1]), false, t0).unwrap();
        assert_eq!(at(&g, t0), "C1");
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                then: Upcoming::Results,
                ..
            }
        ));
        let mut path = Vec::new();
        for _ in 0..3 {
            g.next(t0).unwrap();
            path.push(at(&g, t0));
        }
        assert_eq!(path, ["R0", "R1", "T2"]);
        let HostPhase::Standings {
            title, standings, ..
        } = g.host_view(t0).phase
        else {
            panic!("expected the standings");
        };
        assert_eq!(title, "Results so far");
        assert_eq!(standings.len(), 2);
        let round = standings[0].round.unwrap();
        assert_eq!(
            (round.points, round.previous_rank),
            (standings[0].score, None)
        );
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Standings { teams: 2, .. }
        ));
        assert!(g.player_view(owls, t0).unwrap().score.is_some());
        // Teams can still join at an intermission.
        join(&mut g, "Bats");

        // Back walks the same way.
        let mut path = Vec::new();
        for _ in 0..3 {
            g.back(t0).unwrap();
            path.push(at(&g, t0));
        }
        assert_eq!(path, ["R1", "R0", "C1"]);
        for _ in 0..3 {
            g.next(t0).unwrap();
        }
        assert_eq!(at(&g, t0), "T2");

        // The second part: only its questions are reviewed at the end.
        g.next(t0).unwrap();
        assert_eq!(at(&g, t0), "Q3");
        g.next(t0).unwrap();
        g.next(t0).unwrap();
        g.next(t0).unwrap();
        assert_eq!(at(&g, t0), "C4");
        let mut path = Vec::new();
        for _ in 0..3 {
            g.next(t0).unwrap();
            path.push(at(&g, t0));
        }
        assert_eq!(path, ["R3", "R4", "P"]);
        g.back(t0).unwrap();
        assert_eq!(at(&g, t0), "R4");
        g.back(t0).unwrap();
        g.back(t0).unwrap();
        assert_eq!(at(&g, t0), "C4");
    }

    #[test]
    fn ending_on_a_results_slide_skips_the_end_review() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            reveal: RevealMode::AtEnd,
            ..QuizSettings::default()
        });
        quiz.questions.push(Question::results_slide());
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        join(&mut g, "Owls");
        let mut path = Vec::new();
        for _ in 0..6 {
            g.next(t0).unwrap();
            path.push(at(&g, t0));
        }
        assert_eq!(path, ["Q0", "C0", "Q1", "C1", "R0", "R1"]);
        g.next(t0).unwrap();
        assert_eq!(at(&g, t0), "T2");
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Standings {
                then: Upcoming::Podium,
                ..
            }
        ));
        g.next(t0).unwrap();
        assert_eq!(at(&g, t0), "P");
        g.back(t0).unwrap();
        assert_eq!(at(&g, t0), "T2");
    }

    #[test]
    fn slides_show_between_questions_and_are_skipped_in_the_review() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            reveal: RevealMode::AtEnd,
            ..QuizSettings::default()
        });
        let slide = |title: &str| Question {
            kind: QuestionKind::Slide {
                title: title.to_owned(),
                results: false,
            },
            text: " Get ready ".to_owned(),
            ..Question::default()
        };
        quiz.questions.insert(0, slide("Round 1"));
        quiz.questions.insert(2, slide("Round 2"));
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");

        g.next(t0).unwrap();
        let HostPhase::Slide { slide, then } = g.host_view(t0).phase else {
            panic!("expected a slide");
        };
        assert_eq!(slide.title, "Round 1");
        assert_eq!(slide.text, "Get ready");
        assert_eq!(then, Upcoming::Question);
        assert_eq!(g.player_view(owls, t0).unwrap().phase, PlayerPhase::Slide);
        assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
        // Manual advance waits on slides.
        assert_eq!(g.next_deadline(), None);

        g.next(t0).unwrap();
        let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
            panic!("expected a question");
        };
        assert_eq!((question.index, question.number, question.total), (1, 1, 2));
        assert!(matches!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Question {
                index: 0,
                total: 2,
                ..
            }
        ));
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                then: Upcoming::Slide,
                ..
            }
        ));
        g.next(t0).unwrap(); // Round 2
        g.next(t0).unwrap(); // question two
        g.answer(owls, pick(&[1]), false, t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                then: Upcoming::Answers,
                ..
            }
        ));

        // The review only walks through questions.
        g.next(t0).unwrap();
        let HostPhase::Review { question, then, .. } = g.host_view(t0).phase else {
            panic!("expected the review");
        };
        assert_eq!((question.index, then), (1, Upcoming::Question));
        g.next(t0).unwrap();
        let HostPhase::Review { question, then, .. } = g.host_view(t0).phase else {
            panic!("expected the review");
        };
        assert_eq!((question.index, then), (3, Upcoming::Podium));
        g.next(t0).unwrap();
        assert!(matches!(g.host_view(t0).phase, HostPhase::Podium { .. }));
        assert_eq!(g.standings()[0].score, 1000);
    }

    #[test]
    fn a_bonus_question_adds_its_share_and_is_judged() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings {
            points: crate::model::PointsMode::Flat,
            ..QuizSettings::default()
        });
        quiz.questions[0].bonus = Some(Bonus {
            text: "Who sings it?".to_owned(),
            accepted: vec!["Freddie Mercury".to_owned()],
            percent: 50,
        });
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let bats = join(&mut g, "Bats");
        let rats = join(&mut g, "Rats");
        g.next(t0).unwrap();
        let PlayerPhase::Question { bonus, .. } = g.player_view(owls, t0).unwrap().phase else {
            panic!("expected a question");
        };
        assert_eq!(bonus, Some(("Who sings it?".to_owned(), 50)));
        let bonus = |text: &str| Some(text.to_owned());
        g.answer_with_bonus(owls, pick(&[0]), false, bonus("freddie mercury"), t0)
            .unwrap();
        g.answer_with_bonus(cats, pick(&[1]), false, bonus("Freddie Mercury!"), t0)
            .unwrap();
        g.answer_with_bonus(bats, pick(&[0]), false, bonus("Freddy"), t0)
            .unwrap();
        g.answer(rats, pick(&[0]), false, t0).unwrap();
        let score = |g: &Game, team: u32| {
            g.standings()
                .into_iter()
                .find(|r| r.team.id == team)
                .unwrap()
                .score
        };
        assert_eq!(
            [owls, cats, bats, rats].map(|t| score(&g, t)),
            [1500, 500, 1000, 1000]
        );
        let HostPhase::Closed {
            bonus: Some(reveal),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected the bonus answers");
        };
        assert_eq!(reveal.label, "Bonus answers");
        assert_eq!(reveal.groups.len(), 2);
        // The judge gives Freddy half the bonus; the main answers cannot be judged.
        let half = Judgement {
            grade: Grade::Half,
            bonus: 0,
        };
        g.judge(0, "bonus|freddy", half, t0).unwrap();
        assert_eq!(score(&g, bats), 1250);
        assert!(g.judge(0, "freddy", half, t0).is_err());
    }

    #[test]
    fn flat_points_ignore_speed() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings {
            points: crate::model::PointsMode::Flat,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0 + secs(9)).unwrap();
        g.answer(cats, pick(&[0]), false, t0 + secs(1)).unwrap();
        let standings = g.standings();
        assert_eq!(
            standings.iter().map(|r| r.score).collect::<Vec<_>>(),
            [1000, 1000]
        );
        // Same score: the faster team ranks first.
        assert_eq!(standings[0].team.id, cats);
    }

    #[test]
    fn auto_advance_moves_on_by_itself() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings {
            advance: AdvanceMode::Auto,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert_eq!(g.next_deadline(), Some(t0 + AUTO_AFTER_REVEAL));
        assert_eq!(g.host_view(t0).auto_advance_ms, Some(8000));
        assert!(g.tick(t0 + AUTO_AFTER_REVEAL));
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Question {
                question: QuestionInfo { index: 1, .. },
                ..
            }
        ));
    }

    #[test]
    fn disconnected_teams_do_not_hold_up_a_question() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.set_connected(cats, false);
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));
    }

    #[test]
    fn answers_are_validated() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
        g.next(t0).unwrap();
        assert!(g.answer(owls, pick(&[]), false, t0).is_err());
        assert!(g.answer(owls, pick(&[7]), false, t0).is_err());
        assert!(g.answer(owls, pick(&[0, 1]), false, t0).is_err());
        assert!(g.answer(owls, pick(&[0]), false, t0 + secs(10)).is_err());
        assert!(g.answer(99, pick(&[0]), false, t0).is_err());
    }

    #[test]
    fn scores_do_not_move_while_a_question_is_open() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert_eq!(g.player_view(owls, t0).unwrap().score, Some(0));
        g.next(t0).unwrap();
        assert_eq!(g.player_view(owls, t0).unwrap().score, Some(1000));
    }

    #[test]
    fn soundtrack_questions_wait_for_the_host() {
        let t0 = Instant::now();
        let mut settings = quiz(QuizSettings {
            advance: AdvanceMode::Auto,
            ..QuizSettings::default()
        });
        settings.questions[1].audio = Some("song.mp3".to_owned());
        let mut g = Game::new("123456".to_owned(), settings).unwrap();
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), false, t0).unwrap();
        assert!(g.tick(t0 + AUTO_AFTER_REVEAL));

        // Question two waits, even with auto-advance, and takes no answers yet.
        let HostPhase::Ready { question } = g.host_view(t0).phase else {
            panic!("expected the soundtrack to wait for the host");
        };
        assert_eq!(question.audio.as_deref(), Some("song.mp3"));
        assert_eq!(g.next_deadline(), None);
        assert_eq!(g.host_view(t0).auto_advance_ms, None);
        assert!(g.answer(owls, pick(&[0]), false, t0 + secs(20)).is_err());
        assert_eq!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Ready { index: 1, total: 2 }
        );

        // The clock starts with the music.
        let t1 = t0 + secs(30);
        g.next(t1).unwrap();
        assert_eq!(g.next_deadline(), Some(t1 + secs(10)));
        g.answer(owls, pick(&[0]), false, t1).unwrap();
        assert_eq!(g.player_view(owls, t1).unwrap().score, Some(2000));
    }

    fn open_game(kind: QuestionKind, settings: QuizSettings) -> Game {
        let mut quiz = quiz(settings);
        quiz.questions.truncate(1);
        quiz.questions[0].kind = kind;
        Game::new("123456".to_owned(), quiz).unwrap()
    }

    fn text(answer: &str) -> Response {
        Response::Text(answer.to_owned())
    }

    fn text_groups(g: &Game) -> Vec<(String, Vec<String>, Judgement, bool)> {
        let (HostPhase::Closed {
            reveal: Some(Reveal::Text { groups, .. }),
            ..
        }
        | HostPhase::Review {
            reveal: Reveal::Text { groups, .. },
            ..
        }) = g.host_view(Instant::now()).phase
        else {
            panic!("expected typed answers on screen");
        };
        groups
            .into_iter()
            .map(|g| (g.text, g.teams, g.judgement, g.amended))
            .collect()
    }

    #[test]
    fn typed_answers_match_strictly_and_the_judge_amends() {
        let t0 = Instant::now();
        let kind = QuestionKind::Text {
            accepted: vec!["Leonardo da Vinci".to_owned(), "Leonardo".to_owned()],
        };
        let mut g = open_game(kind, QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let bats = join(&mut g, "Bats");
        let pigs = join(&mut g, "Pigs");
        g.next(t0).unwrap();
        assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
        assert!(g.answer(owls, text(" ?! "), false, t0).is_err());
        assert!(g.answer(owls, text(&"x".repeat(61)), false, t0).is_err());
        g.answer(owls, text("  léonardo DA vinci! "), false, t0)
            .unwrap();
        g.answer(cats, text("Leonardo da Vinchi"), false, t0)
            .unwrap();
        g.answer(bats, text("leonardo da vinchi"), false, t0)
            .unwrap();
        g.answer(pigs, text("Raphael"), false, t0).unwrap();

        let full = Judgement {
            grade: Grade::Full,
            bonus: 0,
        };
        let wrong = Judgement {
            grade: Grade::Wrong,
            bonus: 0,
        };
        let groups = text_groups(&g);
        assert_eq!(
            groups,
            vec![
                (
                    "Leonardo da Vinchi".to_owned(),
                    vec!["Cats".to_owned(), "Bats".to_owned()],
                    wrong,
                    false
                ),
                (
                    "léonardo DA vinci!".to_owned(),
                    vec!["Owls".to_owned()],
                    full,
                    false
                ),
                ("Raphael".to_owned(), vec!["Pigs".to_owned()], wrong, false),
            ]
        );
        assert_eq!(g.player_view(owls, t0).unwrap().score, Some(1000));
        assert_eq!(g.player_view(cats, t0).unwrap().score, Some(0));

        // Half points plus a bonus for the typo, a malus for Raphael.
        let half_bonus = Judgement {
            grade: Grade::Half,
            bonus: 100,
        };
        g.judge(0, "leonardo da vinchi", half_bonus, t0).unwrap();
        g.judge(
            0,
            "raphael",
            Judgement {
                grade: Grade::Wrong,
                bonus: -200,
            },
            t0,
        )
        .unwrap();
        assert_eq!(g.player_view(cats, t0).unwrap().score, Some(600));
        assert_eq!(g.player_view(bats, t0).unwrap().score, Some(600));
        assert_eq!(g.player_view(pigs, t0).unwrap().score, Some(-200));
        assert_eq!(text_groups(&g)[0].2, half_bonus);
        assert!(text_groups(&g)[0].3);

        // Setting the automatic verdict again clears the amendment.
        g.judge(0, "leonardo da vinchi", wrong, t0).unwrap();
        assert!(!text_groups(&g)[0].3);
        // Bonuses are capped; other questions cannot be judged.
        g.judge(
            0,
            "raphael",
            Judgement {
                grade: Grade::Wrong,
                bonus: -9000,
            },
            t0,
        )
        .unwrap();
        assert_eq!(g.player_view(pigs, t0).unwrap().score, Some(-1000));
        assert!(g.judge(1, "raphael", wrong, t0).is_err());
    }

    #[test]
    fn judging_waits_for_the_review_when_answers_are_hidden() {
        let t0 = Instant::now();
        let kind = QuestionKind::Text {
            accepted: vec!["yes".to_owned()],
        };
        let mut g = open_game(
            kind,
            QuizSettings {
                reveal: RevealMode::AtEnd,
                ..QuizSettings::default()
            },
        );
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        g.answer(owls, text("no"), false, t0).unwrap();
        let full = Judgement {
            grade: Grade::Full,
            bonus: 0,
        };
        assert!(g.judge(0, "no", full, t0).is_err());
        g.next(t0).unwrap(); // review
        g.judge(0, "no", full, t0).unwrap();
        assert_eq!(text_groups(&g)[0].2, full);
    }

    #[test]
    fn a_bonus_on_a_number_question_is_a_share_of_the_closest_points() {
        let t0 = Instant::now();
        let mut quiz = quiz(QuizSettings::default());
        quiz.questions.truncate(1);
        quiz.questions[0].kind = QuestionKind::Number { answer: Some(88.0) };
        quiz.questions[0].bonus = Some(Bonus {
            text: "Who invented it?".to_owned(),
            accepted: vec!["Cristofori".to_owned()],
            percent: 25,
        });
        let mut g = Game::new("123456".to_owned(), quiz).unwrap();
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        let bonus = Some("cristofori".to_owned());
        g.answer_with_bonus(
            owls,
            Response::Number(85.0),
            false,
            bonus.clone(),
            t0 + secs(9),
        )
        .unwrap();
        g.answer_with_bonus(cats, Response::Number(50.0), false, bonus, t0)
            .unwrap();
        let scores: Vec<(u32, i32)> = g.standings().iter().map(|r| (r.team.id, r.score)).collect();
        assert_eq!(scores, [(owls, 1250), (cats, 250)]);
    }

    #[test]
    fn the_closest_number_wins_and_ties_share() {
        let t0 = Instant::now();
        let kind = QuestionKind::Number {
            answer: Some(330.0),
        };
        let mut g = open_game(kind, QuizSettings::default());
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        let bats = join(&mut g, "Bats");
        g.next(t0).unwrap();
        assert!(g.answer(owls, text("330"), false, t0).is_err());
        assert!(
            g.answer(owls, Response::Number(f64::NAN), false, t0)
                .is_err()
        );
        g.answer(owls, Response::Number(300.0), false, t0).unwrap();
        g.answer(cats, Response::Number(360.0), false, t0 + secs(9))
            .unwrap();
        g.answer(bats, Response::Number(1000.0), false, t0).unwrap();
        let HostPhase::Closed {
            reveal: Some(Reveal::Number { answer, guesses }),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected guesses on screen");
        };
        assert!((answer - 330.0).abs() < f64::EPSILON);
        let summary: Vec<(&str, bool)> = guesses
            .iter()
            .map(|g| (g.team.as_str(), g.closest))
            .collect();
        assert_eq!(
            summary,
            vec![("Owls", true), ("Cats", true), ("Bats", false)]
        );
        assert_eq!(g.player_view(cats, t0).unwrap().score, Some(1000));
        assert_eq!(g.player_view(bats, t0).unwrap().score, Some(0));
    }

    #[test]
    fn phone_shows_texts_only_in_full_text_mode() {
        let t0 = Instant::now();
        for (display, expect_text) in [
            (crate::model::PhoneDisplay::ButtonsOnly, false),
            (crate::model::PhoneDisplay::FullText, true),
        ] {
            let mut g = game(QuizSettings {
                phone_display: display,
                ..QuizSettings::default()
            });
            let owls = join(&mut g, "Owls");
            g.next(t0).unwrap();
            let PlayerPhase::Question { text, .. } = g.player_view(owls, t0).unwrap().phase else {
                panic!("expected a question");
            };
            assert_eq!(text.is_some(), expect_text);
        }
    }
}
