//! Scores over the quiz: standings, the fastest team, the podium and the results.

use super::views::millis;
use super::{Game, Phase, Team};
use crate::{
    model::QuestionKind,
    protocol::{Fastest, GameResults, ResultRow, RoundResult, StandingRow},
    scoring::{self, Tally},
};

impl Game {
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

    /// Points from the first `closed` questions; open questions do not count yet.
    pub(super) fn team_score(&self, team: &Team, closed: usize) -> i32 {
        (0..closed).map(|i| self.outcome(i, team).points).sum()
    }

    /// How many questions, from the start, have closed and therefore count towards scores.
    pub(super) fn closed_count(&self) -> usize {
        match self.phase {
            Phase::Network | Phase::Lobby => 0,
            Phase::Ready { index } | Phase::Question { index, .. } => index,
            Phase::Closed { index, .. }
            | Phase::Slide { index, .. }
            | Phase::Standings { index, .. } => index + 1,
            Phase::Review { index, .. } => self.part_end(index),
            Phase::Podium { .. } => self.quiz.questions.len(),
        }
    }

    pub(super) fn standings(&self) -> Vec<StandingRow> {
        self.standings_at(self.closed_count())
    }

    /// Standings once question `index` closed, with each team's result on it and rank change.
    pub(super) fn standings_after(&self, index: usize) -> Vec<StandingRow> {
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
    pub(super) fn standings_of_part(&self, index: usize) -> Vec<StandingRow> {
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
    pub(super) fn fastest(&self, index: usize) -> Option<Fastest> {
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
    pub(super) fn podium_steps(&self) -> usize {
        self.teams.len().min(3)
    }

    /// Ranks strictly above this are visible after `revealed` podium steps.
    pub(super) fn podium_threshold(&self, revealed: usize) -> usize {
        self.podium_steps() - revealed
    }
}
