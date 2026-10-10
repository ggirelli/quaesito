//! The game state machine for one live session.
//!
//! Pure logic: every method takes the current time, so it can be tested without a runtime.
//!
//! ```text
//! Lobby → Question(0) → Closed(0) → … → Closed(n-1) → [Review(0) → … → Review(n-1)] → Podium
//! ```
//!
//! A question with a soundtrack first waits in `Ready` until the host starts the music, which
//! also starts its clock. Slides sit between questions as `Slide(i)` and are skipped by the
//! review.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use super::matching::normalize;
use crate::{
    model::{AdvanceMode, MAX_TEXT_ANSWER_CHARS, Question, QuestionKind, Quiz, RevealMode},
    protocol::{
        AnswerOutcome, Grade, Guess, HostPhase, HostView, InputKind, Judgement, MAX_NAME_CHARS,
        PlayerPhase, PlayerView, QuestionInfo, Response, Reveal, SlideInfo, StandingRow, TeamInfo,
        TextGroup, Upcoming,
    },
    scoring::{self, Tally},
};

/// Most teams in one session.
pub const MAX_TEAMS: usize = 50;

/// Auto-advance pause after a question when its answers are shown.
const AUTO_AFTER_REVEAL: Duration = Duration::from_secs(8);
/// Auto-advance pause after a question when answers stay hidden until the end.
const AUTO_AFTER_HIDDEN: Duration = Duration::from_secs(3);
/// Auto-advance pause on a slide.
const AUTO_SLIDE: Duration = Duration::from_secs(10);
/// Auto-advance pause between podium spots.
const AUTO_PODIUM_STEP: Duration = Duration::from_secs(4);

/// One running quiz.
pub struct Game {
    code: String,
    quiz: Quiz,
    teams: Vec<Team>,
    next_team_id: u32,
    phase: Phase,
    /// Per question: the judge's amendments to typed answers, by normalised text.
    judgements: Vec<HashMap<String, Judgement>>,
}

struct Team {
    id: u32,
    name: String,
    avatar: Option<String>,
    token: String,
    connections: u32,
    submissions: Vec<Option<Submission>>,
}

struct Submission {
    response: Response,
    elapsed: Duration,
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
        Ok(Self {
            code,
            quiz,
            teams: Vec::new(),
            next_team_id: 1,
            phase: Phase::Lobby,
            judgements: vec![HashMap::new(); quiz_len],
        })
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.quiz.title
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
        if self.teams.len() >= MAX_TEAMS {
            return Err(Refusal("This game is full."));
        }
        if matches!(self.phase, Phase::Review { .. } | Phase::Podium { .. }) {
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
            self.phase = Phase::Closed { index, since: now };
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
        self.phase = match self.phase {
            Phase::Lobby => {
                if self.teams.is_empty() {
                    return Err(Refusal("Wait for at least one team to join."));
                }
                self.item_phase(0, now)
            }
            Phase::Ready { index } => self.open_question(index, now),
            Phase::Question { index, .. } => Phase::Closed { index, since: now },
            Phase::Closed { index, .. } | Phase::Slide { index, .. } => {
                match self.upcoming_after(index) {
                    Upcoming::Question | Upcoming::Slide => self.item_phase(index + 1, now),
                    Upcoming::Answers => Phase::Review {
                        index: self.question_from(0).unwrap_or_default(),
                        since: now,
                    },
                    Upcoming::Podium => Phase::Podium {
                        revealed: 0,
                        since: now,
                    },
                }
            }
            Phase::Review { index, .. } => match self.question_from(index + 1) {
                Some(index) => Phase::Review { index, since: now },
                None => Phase::Podium {
                    revealed: 0,
                    since: now,
                },
            },
            Phase::Podium { revealed, .. } if revealed < self.podium_steps() => Phase::Podium {
                revealed: revealed + 1,
                since: now,
            },
            Phase::Podium { .. } => return Err(Refusal("The quiz is over.")),
        };
        Ok(())
    }

    /// Submits a team's answer to the open question.
    ///
    /// # Errors
    /// Refuses when no question is open, the team already answered, or the answer is invalid.
    pub fn answer(&mut self, team: u32, response: Response, now: Instant) -> Result<(), Refusal> {
        let Phase::Question {
            index,
            started,
            deadline,
        } = self.phase
        else {
            return Err(Refusal("No question is open right now."));
        };
        if now >= deadline {
            return Err(Refusal("Time is up."));
        }
        let response = check_response(&self.quiz.questions[index], response)?;
        let team = self
            .teams
            .iter_mut()
            .find(|t| t.id == team)
            .ok_or(Refusal("Unknown team."))?;
        if team.submissions[index].is_some() {
            return Err(Refusal("Your answer is already in."));
        }
        team.submissions[index] = Some(Submission {
            response,
            elapsed: now.duration_since(started),
        });

        if self.everyone_answered(index) {
            self.phase = Phase::Closed { index, since: now };
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
        if !matches!(
            self.quiz.questions[question].kind,
            QuestionKind::Text { .. }
        ) {
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
            Phase::Slide { since, .. } => Some(since + AUTO_SLIDE),
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
        let phase = match self.phase {
            Phase::Lobby => HostPhase::Lobby,
            Phase::Ready { index } => HostPhase::Ready {
                question: self.question_info(index),
            },
            Phase::Question {
                index, deadline, ..
            } => HostPhase::Question {
                question: self.question_info(index),
                remaining_ms: millis(deadline.saturating_duration_since(now)),
                answered: self.answered(index),
            },
            Phase::Closed { index, .. } => {
                let revealing = self.quiz.settings.reveal == RevealMode::AfterEachQuestion;
                HostPhase::Closed {
                    question: self.question_info(index),
                    answered: self.answered(index),
                    reveal: revealing.then(|| self.reveal(index)),
                    standings: revealing.then(|| self.standings()),
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
                then: if self.question_from(index + 1).is_some() {
                    Upcoming::Question
                } else {
                    Upcoming::Podium
                },
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
            auto_advance_ms: match self.phase {
                Phase::Question { .. } => None,
                _ => self
                    .next_deadline()
                    .map(|d| millis(d.saturating_duration_since(now))),
            },
            phase,
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
            } => {
                let question = &self.quiz.questions[index];
                let input = input_kind(question);
                PlayerPhase::Question {
                    index: number(index),
                    total,
                    input,
                    answer_count: if input == InputKind::Choice {
                        question.answers.len()
                    } else {
                        0
                    },
                    text: (self.quiz.settings.phone_display
                        == crate::model::PhoneDisplay::FullText)
                        .then(|| (question.text.clone(), question_answers(question))),
                    multi: question.is_multi(),
                    remaining_ms: millis(deadline.saturating_duration_since(now)),
                    submitted: submitted(index),
                }
            }
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
        Some(PlayerView {
            title: self.quiz.title.clone(),
            team: team.info(),
            display: self.quiz.settings.phone_display,
            score: score_visible.then(|| self.team_score(team, self.closed_count())),
            phase,
        })
    }

    fn team_mut(&mut self, id: u32) -> Option<&mut Team> {
        self.teams.iter_mut().find(|t| t.id == id)
    }

    /// The phase that shows item `index`: a slide, a question waiting for its soundtrack, or a
    /// question open right away.
    fn item_phase(&self, index: usize, now: Instant) -> Phase {
        let item = &self.quiz.questions[index];
        if item.is_slide() {
            Phase::Slide { index, since: now }
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

    /// The first question at or after item `start`, skipping slides.
    fn question_from(&self, start: usize) -> Option<usize> {
        (start..self.quiz.questions.len()).find(|&i| !self.quiz.questions[i].is_slide())
    }

    /// Where "next" leads after question or slide `index` is done.
    fn upcoming_after(&self, index: usize) -> Upcoming {
        match self.quiz.questions.get(index + 1) {
            Some(item) if item.is_slide() => Upcoming::Slide,
            Some(_) => Upcoming::Question,
            None if self.quiz.settings.reveal == RevealMode::AtEnd => Upcoming::Answers,
            None => Upcoming::Podium,
        }
    }

    fn slide_info(&self, index: usize) -> SlideInfo {
        let item = &self.quiz.questions[index];
        SlideInfo {
            title: match &item.kind {
                QuestionKind::Slide { title } => title.trim().to_owned(),
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
            audio: question.audio.clone(),
            input: input_kind(question),
            answers: question_answers(question),
            multi: question.is_multi(),
            time_limit_ms: u64::from(question.time_limit_secs(&self.quiz.settings)) * 1000,
        }
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
            QuestionKind::Choice => {
                let mut picks = vec![0; question.answers.len()];
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
            QuestionKind::Text { accepted } => {
                let mut groups: Vec<TextGroup> = Vec::new();
                for (team, response) in responses() {
                    let Response::Text(text) = response else {
                        continue;
                    };
                    let key = normalize(text);
                    if let Some(group) = groups.iter_mut().find(|g| g.key == key) {
                        group.teams.push(team.to_owned());
                    } else {
                        let (judgement, amended) = self.judgement(index, &key);
                        groups.push(TextGroup {
                            key,
                            text: text.clone(),
                            teams: vec![team.to_owned()],
                            judgement,
                            amended,
                        });
                    }
                }
                // Most common answers first; the sort is stable, so ties keep answer order.
                groups.sort_by_key(|g| std::cmp::Reverse(g.teams.len()));
                Reveal::Text {
                    accepted: accepted
                        .iter()
                        .map(|a| a.trim().to_owned())
                        .filter(|a| !a.is_empty())
                        .collect(),
                    groups,
                }
            }
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

    /// The verdict on typed answers normalising to `key`, and whether the judge amended it.
    fn judgement(&self, index: usize, key: &str) -> (Judgement, bool) {
        match self.judgements[index].get(key) {
            Some(&judgement) => (judgement, true),
            None => (self.automatic_judgement(index, key), false),
        }
    }

    /// Full points when `key` matches an accepted answer, none otherwise.
    fn automatic_judgement(&self, index: usize, key: &str) -> Judgement {
        let matches = match &self.quiz.questions[index].kind {
            QuestionKind::Text { accepted } => {
                !key.is_empty() && accepted.iter().any(|a| normalize(a) == key)
            }
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

    /// How `team` did on question `index`.
    fn outcome(&self, index: usize, team: &Team) -> AnswerOutcome {
        let Some(submission) = &team.submissions[index] else {
            return AnswerOutcome {
                points: 0,
                fully_correct: false,
            };
        };
        let question = &self.quiz.questions[index];
        let limit = Duration::from_secs(u64::from(question.time_limit_secs(&self.quiz.settings)));
        let full = scoring::full_points(self.quiz.settings.points, submission.elapsed, limit);
        match (&question.kind, &submission.response) {
            (QuestionKind::Choice, Response::Choice(selected)) => {
                let mode = self.quiz.settings.multi_scoring;
                let points = scoring::score(question, selected, full, mode);
                AnswerOutcome {
                    points: i32::try_from(points).unwrap_or(i32::MAX),
                    fully_correct: scoring::correctness(question, selected, mode) >= 1.0,
                }
            }
            (QuestionKind::Text { .. }, Response::Text(text)) => {
                let (judgement, _) = self.judgement(index, &normalize(text));
                AnswerOutcome {
                    points: scoring::text_points(judgement, full),
                    fully_correct: judgement.grade == Grade::Full,
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
                }
            }
            _ => AnswerOutcome {
                points: 0,
                fully_correct: false,
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
            Phase::Closed { index, .. } | Phase::Slide { index, .. } => index + 1,
            Phase::Review { .. } | Phase::Podium { .. } => self.quiz.questions.len(),
        }
    }

    fn standings(&self) -> Vec<StandingRow> {
        let closed = self.closed_count();
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
        (QuestionKind::Choice, Response::Choice(mut selected)) => {
            selected.sort_unstable();
            selected.dedup();
            if selected.is_empty() || selected.iter().any(|&i| i >= question.answers.len()) {
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
        _ => Err(Refusal("That answer does not fit this question.")),
    }
}

fn input_kind(question: &Question) -> InputKind {
    match question.kind {
        QuestionKind::Choice | QuestionKind::Slide { .. } => InputKind::Choice,
        QuestionKind::Text { .. } => InputKind::Text,
        QuestionKind::Number { .. } => InputKind::Number,
    }
}

/// Answer texts shown for a question: the options of a multiple-choice question, else none.
fn question_answers(question: &Question) -> Vec<String> {
    match question.kind {
        QuestionKind::Choice => question.answers.iter().map(|a| a.text.clone()).collect(),
        _ => Vec::new(),
    }
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
            audio: None,
            time_limit_secs: Some(10),
            answers: vec![
                Answer {
                    text: "right".to_owned(),
                    correct: true,
                },
                Answer {
                    text: "wrong".to_owned(),
                    correct: false,
                },
            ],
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
        g.answer(owls, pick(&[0]), t0).unwrap();
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
        g.answer(owls, pick(&[0]), t0).unwrap();
        assert!(g.answer(owls, pick(&[0]), t0).is_err());
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Question { answered: 1, .. }
        ));
        g.answer(cats, pick(&[1]), t0 + secs(1)).unwrap();
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
                        fully_correct: false
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
        g.answer(owls, pick(&[0]), t0 + secs(5)).unwrap();
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
                    fully_correct: true
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
        assert!(g.answer(owls, pick(&[0]), t0).is_err());
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
        g.answer(owls, pick(&[0]), t0).unwrap();
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Closed {
                then: Upcoming::Slide,
                ..
            }
        ));
        g.next(t0).unwrap(); // Round 2
        g.next(t0).unwrap(); // question two
        g.answer(owls, pick(&[1]), t0).unwrap();
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
    fn flat_points_ignore_speed() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings {
            points: crate::model::PointsMode::Flat,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        let cats = join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), t0 + secs(9)).unwrap();
        g.answer(cats, pick(&[0]), t0 + secs(1)).unwrap();
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
        g.answer(owls, pick(&[0]), t0).unwrap();
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
        g.answer(owls, pick(&[0]), t0).unwrap();
        assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));
    }

    #[test]
    fn answers_are_validated() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        assert!(g.answer(owls, pick(&[0]), t0).is_err());
        g.next(t0).unwrap();
        assert!(g.answer(owls, pick(&[]), t0).is_err());
        assert!(g.answer(owls, pick(&[7]), t0).is_err());
        assert!(g.answer(owls, pick(&[0, 1]), t0).is_err());
        assert!(g.answer(owls, pick(&[0]), t0 + secs(10)).is_err());
        assert!(g.answer(99, pick(&[0]), t0).is_err());
    }

    #[test]
    fn scores_do_not_move_while_a_question_is_open() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, pick(&[0]), t0).unwrap();
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
        g.answer(owls, pick(&[0]), t0).unwrap();
        assert!(g.tick(t0 + AUTO_AFTER_REVEAL));

        // Question two waits, even with auto-advance, and takes no answers yet.
        let HostPhase::Ready { question } = g.host_view(t0).phase else {
            panic!("expected the soundtrack to wait for the host");
        };
        assert_eq!(question.audio.as_deref(), Some("song.mp3"));
        assert_eq!(g.next_deadline(), None);
        assert_eq!(g.host_view(t0).auto_advance_ms, None);
        assert!(g.answer(owls, pick(&[0]), t0 + secs(20)).is_err());
        assert_eq!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Ready { index: 1, total: 2 }
        );

        // The clock starts with the music.
        let t1 = t0 + secs(30);
        g.next(t1).unwrap();
        assert_eq!(g.next_deadline(), Some(t1 + secs(10)));
        g.answer(owls, pick(&[0]), t1).unwrap();
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
        assert!(g.answer(owls, pick(&[0]), t0).is_err());
        assert!(g.answer(owls, text(" ?! "), t0).is_err());
        assert!(g.answer(owls, text(&"x".repeat(61)), t0).is_err());
        g.answer(owls, text("  léonardo DA vinci! "), t0).unwrap();
        g.answer(cats, text("Leonardo da Vinchi"), t0).unwrap();
        g.answer(bats, text("leonardo da vinchi"), t0).unwrap();
        g.answer(pigs, text("Raphael"), t0).unwrap();

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
        g.answer(owls, text("no"), t0).unwrap();
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
        assert!(g.answer(owls, text("330"), t0).is_err());
        assert!(g.answer(owls, Response::Number(f64::NAN), t0).is_err());
        g.answer(owls, Response::Number(300.0), t0).unwrap();
        g.answer(cats, Response::Number(360.0), t0 + secs(9))
            .unwrap();
        g.answer(bats, Response::Number(1000.0), t0).unwrap();
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
