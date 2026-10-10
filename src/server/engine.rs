//! The game state machine for one live session.
//!
//! Pure logic: every method takes the current time, so it can be tested without a runtime.
//!
//! ```text
//! Lobby → Question(0) → Closed(0) → … → Closed(n-1) → [Review(0) → … → Review(n-1)] → Podium
//! ```
//!
//! A question with a soundtrack first waits in `Ready` until the host starts the music, which
//! also starts its clock.

use std::time::{Duration, Instant};

use crate::{
    model::{AdvanceMode, Question, Quiz, RevealMode},
    protocol::{
        AnswerOutcome, HostPhase, HostView, MAX_NAME_CHARS, PlayerPhase, PlayerView, QuestionInfo,
        Reveal, StandingRow, TeamInfo,
    },
    scoring::{self, Tally},
};

/// Most teams in one session.
pub const MAX_TEAMS: usize = 50;

/// Auto-advance pause after a question when its answers are shown.
const AUTO_AFTER_REVEAL: Duration = Duration::from_secs(8);
/// Auto-advance pause after a question when answers stay hidden until the end.
const AUTO_AFTER_HIDDEN: Duration = Duration::from_secs(3);
/// Auto-advance pause between podium spots.
const AUTO_PODIUM_STEP: Duration = Duration::from_secs(4);

/// One running quiz.
pub struct Game {
    code: String,
    quiz: Quiz,
    teams: Vec<Team>,
    next_team_id: u32,
    phase: Phase,
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
    selected: Vec<usize>,
    elapsed: Duration,
    points: u32,
    fully_correct: bool,
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
        Ok(Self {
            code,
            quiz,
            teams: Vec::new(),
            next_team_id: 1,
            phase: Phase::Lobby,
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
            .any(|t| t.name.to_lowercase() == name.to_lowercase())
        {
            return Err(Refusal("Another team already has that name."));
        }
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
            name: name.to_owned(),
            avatar: None,
            token,
            connections: 0,
            submissions: (0..self.quiz.questions.len()).map(|_| None).collect(),
        });
        Ok(id)
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
                self.question_phase(0, now)
            }
            Phase::Ready { index } => self.open_question(index, now),
            Phase::Question { index, .. } => Phase::Closed { index, since: now },
            Phase::Closed { index, .. } if index + 1 < self.quiz.questions.len() => {
                self.question_phase(index + 1, now)
            }
            Phase::Closed { .. } => match self.quiz.settings.reveal {
                RevealMode::AtEnd => Phase::Review {
                    index: 0,
                    since: now,
                },
                RevealMode::AfterEachQuestion => Phase::Podium {
                    revealed: 0,
                    since: now,
                },
            },
            Phase::Review { index, .. } if index + 1 < self.quiz.questions.len() => Phase::Review {
                index: index + 1,
                since: now,
            },
            Phase::Review { .. } => Phase::Podium {
                revealed: 0,
                since: now,
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
    /// Refuses when no question is open, the team already answered, or the selection is invalid.
    pub fn answer(&mut self, team: u32, selected: &[usize], now: Instant) -> Result<(), Refusal> {
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
        let question = &self.quiz.questions[index];
        let mut selected = selected.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if selected.is_empty() || selected.iter().any(|&i| i >= question.answers.len()) {
            return Err(Refusal("Please pick an answer."));
        }
        if !question.is_multi() && selected.len() > 1 {
            return Err(Refusal("Only one answer can be picked."));
        }
        let limit = Duration::from_secs(u64::from(question.time_limit_secs(&self.quiz.settings)));
        let elapsed = now.duration_since(started);
        let mode = self.quiz.settings.multi_scoring;
        let points = scoring::score(question, &selected, elapsed, limit, mode);
        let fully_correct = scoring::correctness(question, &selected, mode) >= 1.0;

        let team = self
            .teams
            .iter_mut()
            .find(|t| t.id == team)
            .ok_or(Refusal("Unknown team."))?;
        if team.submissions[index].is_some() {
            return Err(Refusal("Your answer is already in."));
        }
        team.submissions[index] = Some(Submission {
            selected,
            elapsed,
            points,
            fully_correct,
        });

        let everyone_answered = self
            .teams
            .iter()
            .filter(|t| t.connections > 0)
            .all(|t| t.submissions[index].is_some());
        if everyone_answered {
            self.phase = Phase::Closed { index, since: now };
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
                    last: index + 1 == self.quiz.questions.len(),
                }
            }
            Phase::Review { index, .. } => HostPhase::Review {
                question: self.question_info(index),
                reveal: self.reveal(index),
                last: index + 1 == self.quiz.questions.len(),
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
        let total = self.quiz.questions.len();
        let submitted = |index: usize| team.submissions[index].as_ref().map(|s| s.selected.clone());
        let outcome = |index: usize| {
            team.submissions[index].as_ref().map_or(
                AnswerOutcome {
                    points: 0,
                    fully_correct: false,
                },
                |s| AnswerOutcome {
                    points: s.points,
                    fully_correct: s.fully_correct,
                },
            )
        };
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
            Phase::Ready { index } => PlayerPhase::Ready { index, total },
            Phase::Question {
                index, deadline, ..
            } => {
                let question = &self.quiz.questions[index];
                PlayerPhase::Question {
                    index,
                    total,
                    answer_count: question.answers.len(),
                    text: (self.quiz.settings.phone_display
                        == crate::model::PhoneDisplay::FullText)
                        .then(|| {
                            (
                                question.text.clone(),
                                question.answers.iter().map(|a| a.text.clone()).collect(),
                            )
                        }),
                    multi: question.is_multi(),
                    remaining_ms: millis(deadline.saturating_duration_since(now)),
                    submitted: submitted(index),
                }
            }
            Phase::Closed { index, .. } => PlayerPhase::Closed {
                index,
                total,
                submitted: submitted(index),
                result: revealing
                    .then(|| (outcome(index), self.quiz.questions[index].correct_indexes())),
            },
            Phase::Review { index, .. } => PlayerPhase::Review {
                index,
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
            score: score_visible.then(|| team.score(self.closed_count())),
            phase,
        })
    }

    fn team_mut(&mut self, id: u32) -> Option<&mut Team> {
        self.teams.iter_mut().find(|t| t.id == id)
    }

    /// The phase that shows question `index`: waiting for its soundtrack, or open right away.
    fn question_phase(&self, index: usize, now: Instant) -> Phase {
        if self.quiz.questions[index].audio.is_some() {
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

    fn question_info(&self, index: usize) -> QuestionInfo {
        let question: &Question = &self.quiz.questions[index];
        QuestionInfo {
            index,
            total: self.quiz.questions.len(),
            text: question.text.clone(),
            image: question.image.clone(),
            audio: question.audio.clone(),
            answers: question.answers.iter().map(|a| a.text.clone()).collect(),
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
        let mut picks = vec![0; question.answers.len()];
        for submission in self
            .teams
            .iter()
            .filter_map(|t| t.submissions[index].as_ref())
        {
            for &i in &submission.selected {
                picks[i] += 1;
            }
        }
        Reveal {
            correct: question.correct_indexes(),
            picks,
        }
    }

    /// How many questions, from the start, have closed and therefore count towards scores.
    fn closed_count(&self) -> usize {
        match self.phase {
            Phase::Lobby => 0,
            Phase::Ready { index } | Phase::Question { index, .. } => index,
            Phase::Closed { index, .. } => index + 1,
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
                score: t.score(closed),
                scoring_time: t.submissions[..closed]
                    .iter()
                    .flatten()
                    .filter(|s| s.points > 0)
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

    /// Points from the first `closed` questions; open questions do not count yet.
    fn score(&self, closed: usize) -> u32 {
        self.submissions[..closed]
            .iter()
            .flatten()
            .map(|s| s.points)
            .sum()
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
        g.answer(owls, &[0], t0).unwrap();
        assert!(g.answer(owls, &[0], t0).is_err());
        assert!(matches!(
            g.host_view(t0).phase,
            HostPhase::Question { answered: 1, .. }
        ));
        g.answer(cats, &[1], t0 + secs(1)).unwrap();
        let HostPhase::Closed {
            reveal: Some(reveal),
            standings: Some(standings),
            last: false,
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected a revealed closed question");
        };
        assert_eq!(reveal.picks, vec![1, 1]);
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
            HostPhase::Closed { last: true, .. }
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
        g.answer(owls, &[0], t0 + secs(5)).unwrap();
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
            HostPhase::Review { last: true, .. }
        ));
        g.next(t0).unwrap(); // podium
        assert!(matches!(g.host_view(t0).phase, HostPhase::Podium { .. }));
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
        g.answer(owls, &[0], t0).unwrap();
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
        g.answer(owls, &[0], t0).unwrap();
        assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));
    }

    #[test]
    fn answers_are_validated() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        assert!(g.answer(owls, &[0], t0).is_err());
        g.next(t0).unwrap();
        assert!(g.answer(owls, &[], t0).is_err());
        assert!(g.answer(owls, &[7], t0).is_err());
        assert!(g.answer(owls, &[0, 1], t0).is_err());
        assert!(g.answer(owls, &[0], t0 + secs(10)).is_err());
        assert!(g.answer(99, &[0], t0).is_err());
    }

    #[test]
    fn scores_do_not_move_while_a_question_is_open() {
        let t0 = Instant::now();
        let mut g = game(QuizSettings::default());
        let owls = join(&mut g, "Owls");
        join(&mut g, "Cats");
        g.next(t0).unwrap();
        g.answer(owls, &[0], t0).unwrap();
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
        g.answer(owls, &[0], t0).unwrap();
        assert!(g.tick(t0 + AUTO_AFTER_REVEAL));

        // Question two waits, even with auto-advance, and takes no answers yet.
        let HostPhase::Ready { question } = g.host_view(t0).phase else {
            panic!("expected the soundtrack to wait for the host");
        };
        assert_eq!(question.audio.as_deref(), Some("song.mp3"));
        assert_eq!(g.next_deadline(), None);
        assert_eq!(g.host_view(t0).auto_advance_ms, None);
        assert!(g.answer(owls, &[0], t0 + secs(20)).is_err());
        assert_eq!(
            g.player_view(owls, t0).unwrap().phase,
            PlayerPhase::Ready { index: 1, total: 2 }
        );

        // The clock starts with the music.
        let t1 = t0 + secs(30);
        g.next(t1).unwrap();
        assert_eq!(g.next_deadline(), Some(t1 + secs(10)));
        g.answer(owls, &[0], t1).unwrap();
        assert_eq!(g.player_view(owls, t1).unwrap().score, Some(2000));
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
