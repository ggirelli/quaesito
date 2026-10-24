//! Moving through the quiz: next and back, pausing, timers and auto-advance.

use std::time::{Duration, Instant};

use super::{Game, Phase, Refusal};
use crate::{
    model::{AdvanceMode, RevealMode},
    protocol::Upcoming,
};

/// Auto-advance pause after a question when its answers are shown.
pub(super) const AUTO_AFTER_REVEAL: Duration = Duration::from_secs(8);

/// Auto-advance pause after a question when answers stay hidden until the end.
const AUTO_AFTER_HIDDEN: Duration = Duration::from_secs(3);

/// Auto-advance pause on a slide.
const AUTO_SLIDE: Duration = Duration::from_secs(10);

/// Auto-advance pause between podium spots.
const AUTO_PODIUM_STEP: Duration = Duration::from_secs(4);

impl Game {
    /// Whether every connected team has answered question `index`.
    pub(super) fn everyone_answered(&self, index: usize) -> bool {
        self.teams
            .iter()
            .filter(|t| t.connections > 0)
            .all(|t| t.submissions[index].is_some())
    }

    /// The host's "next" button.
    ///
    /// # Errors
    /// Refuses to start without teams, or to go past the end.
    pub fn next(&mut self, now: Instant) -> Result<(), Refusal> {
        self.resume(now);
        let phase = match self.phase {
            Phase::Network => Phase::Lobby,
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

    /// Leaves the lobby straight for item `index` (a test from a chosen question or slide):
    /// the items before it count as not played.
    ///
    /// # Errors
    /// Refuses outside the lobby, without teams, or past the end of the quiz.
    pub fn start_from(&mut self, index: usize, now: Instant) -> Result<(), Refusal> {
        if !self.in_lobby() {
            return Err(Refusal("The quiz has already started."));
        }
        if self.teams.is_empty() {
            return Err(Refusal("Wait for at least one team to join."));
        }
        if index >= self.quiz.questions.len() {
            return Err(Refusal("The quiz has no such question or slide."));
        }
        let phase = self.item_phase(index, now);
        self.set_phase(phase);
        Ok(())
    }

    /// The host's "back" button: the previous slide, reveal or podium step. Leaving a question
    /// that is still open withdraws it, so it is asked afresh later.
    ///
    /// # Errors
    /// Refuses at the start: the lobby, or the Wi-Fi slide before it.
    pub fn back(&mut self, now: Instant) -> Result<(), Refusal> {
        self.resume(now);
        let last = self.quiz.questions.len() - 1;
        self.phase = match self.phase {
            Phase::Lobby if self.quiz.settings.network.enabled => Phase::Network,
            Phase::Network | Phase::Lobby => return Err(Refusal("This is the start.")),
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

    /// Reopens the closed question on screen for the teams that have not answered, with a fresh
    /// clock; answers already in stay. For a question closed early by mistake.
    ///
    /// # Errors
    /// Refuses when no closed question is on screen, or when every team has answered.
    pub fn reopen(&mut self, now: Instant) -> Result<(), Refusal> {
        let Phase::Closed { index, .. } = self.phase else {
            return Err(Refusal("No closed question is on screen."));
        };
        if self.teams.iter().all(|t| t.submissions[index].is_some()) {
            return Err(Refusal("Every team has answered."));
        }
        self.resume(now);
        self.phase = self.open_question(index, now);
        Ok(())
    }

    /// Freezes the clock and the auto-advance countdown.
    ///
    /// # Errors
    /// Refuses when there is nothing to pause.
    pub fn pause(&mut self, now: Instant) -> Result<(), Refusal> {
        if matches!(
            self.phase,
            Phase::Network | Phase::Lobby | Phase::Ready { .. }
        ) || self.paused_at.is_some()
        {
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
            Phase::Network | Phase::Lobby | Phase::Ready { .. } => {}
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
    pub(super) fn first_question_in(&self, start: usize, end: usize) -> Option<usize> {
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
    pub(super) fn part_start(&self, index: usize) -> usize {
        (0..index)
            .rev()
            .find(|&i| self.quiz.questions[i].is_results())
            .map_or(0, |i| i + 1)
    }

    /// Where the part of the quiz holding item `index` ends: at the results slide at or after it,
    /// or at the end of the quiz.
    pub(super) fn part_end(&self, index: usize) -> usize {
        let len = self.quiz.questions.len();
        (index..len)
            .find(|&i| self.quiz.questions[i].is_results())
            .unwrap_or(len)
    }

    /// Where "next" leads after question or slide `index` is done.
    pub(super) fn upcoming_after(&self, index: usize) -> Upcoming {
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
}
