//! What the big screen and each phone show.

use std::{
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};

use super::{Game, Phase, Team};
use crate::{
    model::{Bonus, NumberScoring, PointsMode, Question, QuestionKind, RevealMode},
    protocol::{
        Explanation, HostPhase, HostView, InputKind, JokerState, PairColumns, PlayerPhase,
        PlayerView, QuestionInfo, SlideInfo, Upcoming,
    },
};

impl Game {
    /// What the big screen shows.
    #[must_use]
    pub fn host_view(&self, now: Instant) -> HostView {
        let clock = self.clock(now);
        let phase = match self.phase {
            Phase::Network => HostPhase::Network {
                network: self.quiz.settings.network.clone(),
            },
            Phase::Lobby => HostPhase::Lobby {
                wifi_slide: self.quiz.settings.network.enabled,
            },
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
                    explanation: revealing.then(|| self.explanation(index)).flatten(),
                    then: self.upcoming_after(index),
                }
            }
            Phase::Slide { index, .. } => HostPhase::Slide {
                slide: self.slide_info(index),
                then: self.upcoming_after(index),
            },
            Phase::Review { index, .. } => HostPhase::Review {
                question: self.question_info(index),
                answered: self.answered(index),
                reveal: self.reveal(index),
                fastest: self.fastest(index),
                bonus: self.bonus_reveal(index),
                explanation: self.explanation(index),
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
            pictures: self.pictures,
            joining: self.joining,
            paused: self.paused_at.is_some(),
            speed_points: self.quiz.settings.points == PointsMode::Speed,
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
            Phase::Network | Phase::Lobby => PlayerPhase::Lobby {
                teams: self.teams.len(),
                pictures: self.pictures,
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

    /// The order a team's phone lists the answers of question `index` in: its own, but the same
    /// every time it is asked (after reconnecting or a restart too).
    fn phone_order(&self, index: usize, team: &Team) -> Vec<usize> {
        let mut hasher = std::hash::DefaultHasher::new();
        (&self.code, team.id, index).hash(&mut hasher);
        let mut state = hasher.finish();
        let mut order: Vec<usize> = (0..self.quiz.questions[index].answers.len()).collect();
        for i in (1..order.len()).rev() {
            // SplitMix64 steps.
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let j = usize::try_from(z % (i as u64 + 1)).unwrap_or(0);
            order.swap(i, j);
        }
        order
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
                InputKind::Text | InputKind::Number | InputKind::Pin | InputKind::Pairs => 0,
            },
            order: match question.kind {
                QuestionKind::Choice => self.phone_order(index, team),
                _ => Vec::new(),
            },
            // Order items, True/False and part labels cannot be told apart by colour alone:
            // always sent.
            items: match question.kind {
                QuestionKind::Order { .. }
                | QuestionKind::TrueFalse { .. }
                | QuestionKind::Parts { .. } => self.shown_answers(index),
                _ => Vec::new(),
            },
            // Phones show the passage (lyrics, a quote…) under the question, line breaks kept.
            text: full_text.then(|| {
                let passage = question.passage.trim();
                let text = if passage.is_empty() {
                    question.text.clone()
                } else {
                    format!("{}\n\n{passage}", question.text)
                };
                (text, self.shown_answers(index))
            }),
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
            range: match question.kind {
                QuestionKind::Number { range, .. } => range,
                _ => None,
            },
            bonus: self
                .active_bonus(index)
                .map(|b| (b.text.trim().to_owned(), b.percent)),
            pairs: self.pair_columns(index),
        }
    }

    /// The columns of pairs question `index` as shown, or none for other kinds.
    fn pair_columns(&self, index: usize) -> Option<PairColumns> {
        let QuestionKind::Pairs { pairs } = &self.quiz.questions[index].kind else {
            return None;
        };
        Some(PairColumns {
            left: pairs.iter().map(|p| p.left.clone()).collect(),
            right: self.shown[index]
                .iter()
                .map(|&i| pairs[i].right.clone())
                .collect(),
        })
    }

    /// Whether a pairs answer (right items by index in the shown column) matches left item
    /// `left` with its partner.
    pub(super) fn is_matched(&self, index: usize, links: &[Option<usize>], left: usize) -> bool {
        links
            .get(left)
            .copied()
            .flatten()
            .and_then(|right| self.shown[index].get(right))
            == Some(&left)
    }

    /// How many pairs a pairs answer matches rightly.
    pub(super) fn matched(&self, index: usize, links: &[Option<usize>]) -> usize {
        (0..links.len())
            .filter(|&left| self.is_matched(index, links, left))
            .count()
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
            closest_wins: matches!(
                question.kind,
                QuestionKind::Number {
                    scoring: NumberScoring::Closest,
                    ..
                }
            ),
            answers: self.shown_answers(index),
            answer_images: answer_images(question),
            multi: question.is_multi(),
            time_limit_ms: u64::from(question.time_limit_secs(&self.quiz.settings)) * 1000,
            bonus: self
                .active_bonus(index)
                .map(|b| (b.text.trim().to_owned(), b.percent)),
            passage: question.passage.trim().to_owned(),
            pairs: self.pair_columns(index),
            category: question.category.trim().to_owned(),
            topic: question.topic.trim().to_owned(),
        }
    }

    /// What question `index` shows with its answer, if anything.
    pub(super) fn explanation(&self, index: usize) -> Option<Explanation> {
        let question = &self.quiz.questions[index];
        let text = question.explanation.trim();
        let media = question.explanation_image.is_some() || question.explanation_audio.is_some();
        (!text.is_empty() || media).then(|| Explanation {
            text: text.to_owned(),
            image: question.explanation_image.clone(),
            audio: question.explanation_audio.clone(),
        })
    }

    /// Question `index`'s bonus question, if it has one it can carry.
    pub(super) fn active_bonus(&self, index: usize) -> Option<&Bonus> {
        let question = &self.quiz.questions[index];
        question.bonus.as_ref().filter(|_| question.takes_bonus())
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
        QuestionKind::Pairs { .. } => InputKind::Pairs,
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

/// How many parts a several-answers question has (0 for other kinds).
fn question_parts(question: &Question) -> usize {
    match &question.kind {
        QuestionKind::Parts { parts } => parts.len(),
        _ => 0,
    }
}

pub(super) fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
