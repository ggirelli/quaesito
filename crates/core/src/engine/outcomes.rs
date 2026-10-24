//! How each team did on a question: its points, grade and bonus.

use std::time::Duration;

use super::answers::{bonus_key, number_key, text_key};
use super::{Game, Team};
use crate::matching::normalize;
use crate::{
    model::{NumberScoring, QuestionKind},
    protocol::{AnswerOutcome, Grade, Judgement, Response},
    scoring::{self},
};

impl Game {
    /// How `team` did with a guess `distance` from the correct number of question `index`, where a
    /// fully correct answer is worth `full`, the judge's bonus or malus included.
    fn number_outcome(
        &self,
        index: usize,
        team: &Team,
        distance: f64,
        number_scoring: NumberScoring,
        full: u32,
    ) -> AnswerOutcome {
        let (points, fully_correct) = if number_scoring == NumberScoring::Closest {
            let closest = self.closest_distance(index).is_some_and(|c| distance <= c);
            (if closest { scoring::CLOSEST_POINTS } else { 0 }, closest)
        } else {
            let points = scoring::number_points(full, distance, number_scoring);
            let all = scoring::number_points(full, 0.0, number_scoring);
            (i32::try_from(points).unwrap_or(i32::MAX), points == all)
        };
        let bonus = self.judgement(index, &number_key(team.id)).0.bonus;
        AnswerOutcome {
            points: points.saturating_add(bonus.clamp(-scoring::MAX_BONUS, scoring::MAX_BONUS)),
            fully_correct,
            joker: false,
        }
    }

    /// Distance of the closest guess on number question `index`, if anyone guessed.
    fn closest_distance(&self, index: usize) -> Option<f64> {
        let QuestionKind::Number {
            answer: Some(answer),
            ..
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
    pub(super) fn outcome(&self, index: usize, team: &Team) -> AnswerOutcome {
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
            // Speed means nothing when the closest guesses win: they earn a set amount.
            let full = match question.kind {
                QuestionKind::Number {
                    scoring: NumberScoring::Closest,
                    ..
                } => u32::try_from(scoring::CLOSEST_POINTS).unwrap_or(scoring::MAX_POINTS),
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
                    scoring: number_scoring,
                    ..
                },
                Response::Number(value),
            ) => self.number_outcome(index, team, (value - answer).abs(), *number_scoring, full),
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
            (QuestionKind::Pairs { pairs }, Response::Pairs(links)) => {
                let matched = self.matched(index, links);
                AnswerOutcome {
                    points: i32::try_from(scoring::order_points(full, matched, pairs.len()))
                        .unwrap_or(i32::MAX),
                    fully_correct: matched == pairs.len(),
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
}
