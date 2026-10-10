//! Points for answers and the final ranking.

use std::time::Duration;

use crate::model::{MultiScoring, Question};

/// Points for an instant, fully correct answer. Answering at the buzzer earns half.
pub const MAX_POINTS: u32 = 1000;

/// Speed component: [`MAX_POINTS`] when instant, half of it at the time limit.
#[must_use]
pub fn speed_points(elapsed: Duration, limit: Duration) -> u32 {
    if limit.is_zero() {
        return MAX_POINTS / 2;
    }
    let ratio = (elapsed.as_secs_f64() / limit.as_secs_f64()).clamp(0.0, 1.0);
    to_points(f64::from(MAX_POINTS) * (1.0 - 0.5 * ratio))
}

/// Correctness of a selection, from 0 (wrong) to 1 (fully right).
#[must_use]
pub fn correctness(question: &Question, selected: &[usize], mode: MultiScoring) -> f64 {
    let correct = question.correct_indexes();
    if correct.is_empty() || selected.is_empty() {
        return 0.0;
    }
    let mut picked: Vec<usize> = selected.to_vec();
    picked.sort_unstable();
    picked.dedup();
    let right = picked.iter().filter(|i| correct.contains(i)).count();
    let wrong = picked.len() - right;
    match mode {
        MultiScoring::AllOrNothing => {
            if wrong == 0 && right == correct.len() {
                1.0
            } else {
                0.0
            }
        }
        MultiScoring::Partial => {
            let net = f64::from(u32::try_from(right).unwrap_or(u32::MAX))
                - f64::from(u32::try_from(wrong).unwrap_or(u32::MAX));
            (net / f64::from(u32::try_from(correct.len()).unwrap_or(u32::MAX))).max(0.0)
        }
    }
}

/// Points earned by `selected`, answered `elapsed` into a question lasting `limit`.
#[must_use]
pub fn score(
    question: &Question,
    selected: &[usize],
    elapsed: Duration,
    limit: Duration,
    mode: MultiScoring,
) -> u32 {
    let fraction = correctness(question, selected, mode);
    if fraction <= 0.0 {
        return 0;
    }
    to_points(f64::from(speed_points(elapsed, limit)) * fraction)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to [0, MAX_POINTS] first"
)]
fn to_points(value: f64) -> u32 {
    value.round().clamp(0.0, f64::from(MAX_POINTS)) as u32
}

/// A team's tally used for ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub team: u32,
    pub score: u32,
    /// Total time spent on answers that earned points; lower wins ties.
    pub scoring_time: Duration,
    /// Join order; earlier wins exact ties.
    pub joined: usize,
}

/// Sorts tallies best first: higher score, then faster, then earlier join.
pub fn rank(tallies: &mut [Tally]) {
    tallies.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.scoring_time.cmp(&b.scoring_time))
            .then(a.joined.cmp(&b.joined))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Answer;

    fn question(correct: &[bool]) -> Question {
        Question {
            text: "Q".to_owned(),
            answers: correct
                .iter()
                .map(|&correct| Answer {
                    text: "A".to_owned(),
                    correct,
                })
                .collect(),
            ..Question::default()
        }
    }

    const LIMIT: Duration = Duration::from_secs(20);

    #[test]
    fn speed_points_scale_linearly() {
        assert_eq!(speed_points(Duration::ZERO, LIMIT), 1000);
        assert_eq!(speed_points(Duration::from_secs(10), LIMIT), 750);
        assert_eq!(speed_points(LIMIT, LIMIT), 500);
        assert_eq!(speed_points(Duration::from_secs(60), LIMIT), 500);
    }

    #[test]
    fn single_answer_scoring() {
        let q = question(&[false, true, false]);
        let s = |sel: &[usize]| score(&q, sel, Duration::ZERO, LIMIT, MultiScoring::Partial);
        assert_eq!(s(&[1]), 1000);
        assert_eq!(s(&[0]), 0);
        assert_eq!(s(&[]), 0);
    }

    #[test]
    fn all_or_nothing_requires_exact_set() {
        let q = question(&[true, true, false, false]);
        let c = |sel: &[usize]| correctness(&q, sel, MultiScoring::AllOrNothing);
        assert!((c(&[0, 1]) - 1.0).abs() < f64::EPSILON);
        assert!((c(&[1, 0, 1]) - 1.0).abs() < f64::EPSILON);
        assert!(c(&[0]).abs() < f64::EPSILON);
        assert!(c(&[0, 1, 2]).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_subtracts_wrong_picks() {
        let q = question(&[true, true, false, false]);
        let c = |sel: &[usize]| correctness(&q, sel, MultiScoring::Partial);
        assert!((c(&[0, 1]) - 1.0).abs() < f64::EPSILON);
        assert!((c(&[0]) - 0.5).abs() < f64::EPSILON);
        assert!(c(&[0, 2]).abs() < f64::EPSILON);
        assert!(c(&[2, 3]).abs() < f64::EPSILON);
        assert_eq!(
            score(
                &q,
                &[0],
                Duration::from_secs(10),
                LIMIT,
                MultiScoring::Partial
            ),
            375
        );
    }

    #[test]
    fn ranking_breaks_ties_by_time_then_join_order() {
        let t = |team, score, secs, joined| Tally {
            team,
            score,
            scoring_time: Duration::from_secs(secs),
            joined,
        };
        let mut tallies = [
            t(1, 500, 3, 0),
            t(2, 900, 9, 1),
            t(3, 500, 2, 2),
            t(4, 500, 2, 3),
        ];
        rank(&mut tallies);
        let order: Vec<u32> = tallies.iter().map(|t| t.team).collect();
        assert_eq!(order, vec![2, 3, 4, 1]);
    }
}
