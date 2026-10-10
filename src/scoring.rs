//! Points for answers and the final ranking.

use std::time::Duration;

use crate::{
    model::{MultiScoring, PointsMode, Question},
    protocol::{Grade, Judgement},
};

/// Points for an instant, fully correct answer. Answering at the buzzer earns half.
pub const MAX_POINTS: u32 = 1000;
/// Largest bonus or malus the judge can give a typed answer.
pub const MAX_BONUS: i32 = 1000;
/// Points for the closest guess on a number question, however long it took.
pub const CLOSEST_POINTS: i32 = 1000;

/// Speed component: [`MAX_POINTS`] when instant, half of it at the time limit.
#[must_use]
pub fn speed_points(elapsed: Duration, limit: Duration) -> u32 {
    if limit.is_zero() {
        return MAX_POINTS / 2;
    }
    let ratio = (elapsed.as_secs_f64() / limit.as_secs_f64()).clamp(0.0, 1.0);
    to_points(f64::from(MAX_POINTS) * (1.0 - 0.5 * ratio))
}

/// What a fully correct answer is worth, answered `elapsed` into a question lasting `limit`.
#[must_use]
pub fn full_points(mode: PointsMode, elapsed: Duration, limit: Duration) -> u32 {
    match mode {
        PointsMode::Speed => speed_points(elapsed, limit),
        PointsMode::Flat => MAX_POINTS,
    }
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

/// Points earned by `selected`, where a fully correct answer is worth `full` (see
/// [`full_points`]).
#[must_use]
pub fn score(question: &Question, selected: &[usize], full: u32, mode: MultiScoring) -> u32 {
    let fraction = correctness(question, selected, mode);
    if fraction <= 0.0 {
        return 0;
    }
    to_points(f64::from(full) * fraction)
}

/// Points for an order question: `full` (see [`full_points`]) times the share of items put in
/// their right place.
#[must_use]
pub fn order_points(full: u32, in_place: usize, items: usize) -> u32 {
    if items == 0 {
        return 0;
    }
    let share = f64::from(u32::try_from(in_place).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(items).unwrap_or(u32::MAX));
    to_points(f64::from(full) * share)
}

/// Points for a pin `distance` (in picture widths) from the right spot: `full` (see
/// [`full_points`]) up to the tolerance's first distance, nothing beyond its second, and a share
/// falling evenly in between.
#[must_use]
pub fn pin_points(full: u32, distance: f64, tolerance: crate::model::PinTolerance) -> u32 {
    let (near, far) = tolerance.distances();
    let share = if distance <= near {
        1.0
    } else if distance >= far {
        0.0
    } else {
        (far - distance) / (far - near)
    };
    to_points(f64::from(full) * share)
}

/// Points for a typed answer: `full` (see [`full_points`]) scaled by the judge's grade, plus the
/// bonus (or minus the malus).
#[must_use]
pub fn text_points(judgement: Judgement, full: u32) -> i32 {
    let fraction = match judgement.grade {
        Grade::Full => 1.0,
        Grade::Half => 0.5,
        Grade::Wrong => 0.0,
    };
    let base = to_points(f64::from(full) * fraction);
    i32::try_from(base).unwrap_or(i32::MAX) + judgement.bonus.clamp(-MAX_BONUS, MAX_BONUS)
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
    pub score: i32,
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
                    image: None,
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
    fn flat_points_ignore_speed() {
        assert_eq!(full_points(PointsMode::Speed, LIMIT, LIMIT), 500);
        assert_eq!(full_points(PointsMode::Flat, LIMIT, LIMIT), 1000);
        assert_eq!(full_points(PointsMode::Flat, Duration::ZERO, LIMIT), 1000);
    }

    #[test]
    fn single_answer_scoring() {
        let q = question(&[false, true, false]);
        let s = |sel: &[usize]| score(&q, sel, 1000, MultiScoring::Partial);
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
                full_points(PointsMode::Speed, Duration::from_secs(10), LIMIT),
                MultiScoring::Partial
            ),
            375
        );
    }

    #[test]
    fn typed_answers_follow_the_judgement() {
        let j = |grade, bonus| Judgement { grade, bonus };
        assert_eq!(text_points(j(Grade::Full, 0), 750), 750);
        assert_eq!(text_points(j(Grade::Half, 0), 750), 375);
        assert_eq!(text_points(j(Grade::Wrong, 0), 750), 0);
        assert_eq!(text_points(j(Grade::Half, 200), 750), 575);
        assert_eq!(text_points(j(Grade::Wrong, -300), 750), -300);
        assert_eq!(text_points(j(Grade::Wrong, -5000), 750), -1000);
        assert_eq!(text_points(j(Grade::Half, 0), 1000), 500);
    }

    #[test]
    fn order_points_count_items_in_place() {
        assert_eq!(order_points(1000, 4, 4), 1000);
        assert_eq!(order_points(1000, 2, 4), 500);
        assert_eq!(order_points(800, 1, 3), 267);
        assert_eq!(order_points(1000, 0, 4), 0);
    }

    #[test]
    fn pin_points_fall_with_distance() {
        use crate::model::PinTolerance::Normal;
        assert_eq!(pin_points(1000, 0.0, Normal), 1000);
        assert_eq!(pin_points(1000, 0.04, Normal), 1000);
        assert_eq!(pin_points(1000, 0.17, Normal), 500);
        assert_eq!(pin_points(1000, 0.3, Normal), 0);
        assert_eq!(pin_points(1000, 2.0, Normal), 0);
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
