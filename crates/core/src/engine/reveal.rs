//! Revealing the answers of a question, and how the teams answered it.

use super::Game;
use super::answers::{
    BONUS_KEY_PREFIX, NUMBER_KEY_PREFIX, PART_KEY_SEPARATOR, bonus_key, number_key, text_key,
};
use crate::matching::normalize;
use crate::{
    model::QuestionKind,
    protocol::{Grade, Guess, Judgement, PartReveal, PinGuess, Response, Reveal, TextGroup},
};

impl Game {
    /// The bonus question's typed answers to question `index`, grouped, if it has one.
    pub(super) fn bonus_reveal(&self, index: usize) -> Option<PartReveal> {
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

    pub(super) fn answered(&self, index: usize) -> usize {
        self.teams
            .iter()
            .filter(|t| t.submissions[index].is_some())
            .count()
    }

    pub(super) fn reveal(&self, index: usize) -> Reveal {
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
            QuestionKind::Pairs { pairs } => {
                let mut matched = vec![0; pairs.len()];
                for (_, response) in responses() {
                    if let Response::Pairs(links) = response {
                        for (left, count) in matched.iter_mut().enumerate() {
                            if self.is_matched(index, links, left) {
                                *count += 1;
                            }
                        }
                    }
                }
                Reveal::Pairs {
                    pairs: pairs.clone(),
                    matched,
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
            QuestionKind::Number { answer, .. } => {
                self.number_reveal(index, answer.unwrap_or_default())
            }
        }
    }

    /// Every guess on number question `index`, closest to `answer` first.
    fn number_reveal(&self, index: usize, answer: f64) -> Reveal {
        let mut guesses: Vec<Guess> = self
            .teams
            .iter()
            .filter_map(|team| match team.submissions[index].as_ref()?.response {
                Response::Number(value) => {
                    let outcome = self.outcome(index, team);
                    let key = number_key(team.id);
                    Some(Guess {
                        team: team.name.clone(),
                        value,
                        full: outcome.fully_correct,
                        points: outcome.points,
                        bonus: self.judgement(index, &key).0.bonus,
                        key,
                    })
                }
                _ => None,
            })
            .collect();
        guesses.sort_by(|a, b| {
            (a.value - answer)
                .abs()
                .total_cmp(&(b.value - answer).abs())
        });
        Reveal::Number { answer, guesses }
    }

    /// Answer texts as shown for question `index`: the options of a multiple-choice question,
    /// the shuffled items of an order question, else none.
    pub(super) fn shown_answers(&self, index: usize) -> Vec<String> {
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
    pub(super) fn in_place(&self, index: usize, order: &[usize]) -> usize {
        order
            .iter()
            .enumerate()
            .filter(|&(position, &shown)| self.shown[index].get(shown) == Some(&position))
            .count()
    }

    /// The verdict on typed answers normalising to `key`, and whether the judge amended it.
    pub(super) fn judgement(&self, index: usize, key: &str) -> (Judgement, bool) {
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

    /// Full points when `key` matches an accepted answer (or is a number guess's), none
    /// otherwise.
    pub(super) fn automatic_judgement(&self, index: usize, key: &str) -> Judgement {
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
            QuestionKind::Number { .. } => key.starts_with(NUMBER_KEY_PREFIX),
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

/// Accepted answers as shown: trimmed, blanks left out.
fn tidy(accepted: &[String]) -> Vec<String> {
    accepted
        .iter()
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect()
}
