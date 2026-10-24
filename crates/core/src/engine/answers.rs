//! Taking the teams' answers and the judge's amendments to typed ones.

use std::time::Instant;

use super::{Game, Phase, Refusal, Submission};
use crate::matching::normalize;
use crate::{
    model::{MAX_TEXT_ANSWER_CHARS, Question, QuestionKind, RevealMode},
    protocol::{Grade, Judgement, Response},
    scoring::{self},
};

impl Game {
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
        let kind = &self.quiz.questions[question].kind;
        let typed = matches!(kind, QuestionKind::Text { .. } | QuestionKind::Parts { .. });
        let bonus = key.starts_with(BONUS_KEY_PREFIX) && self.active_bonus(question).is_some();
        let number =
            matches!(kind, QuestionKind::Number { .. }) && key.starts_with(NUMBER_KEY_PREFIX);
        if !typed && !bonus && !number {
            return Err(Refusal("Only typed answers and numbers can be judged."));
        }
        let judgement = Judgement {
            // Numbers are scored by distance: the judge only adds a bonus or malus.
            grade: if number { Grade::Full } else { judgement.grade },
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
        (QuestionKind::Number { range, .. }, Response::Number(value)) if value.is_finite() => {
            match range {
                Some(range) if !range.contains(value) => {
                    Err(Refusal("Your guess is outside the range."))
                }
                _ => Ok(Response::Number(value)),
            }
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
        (QuestionKind::Pairs { pairs }, Response::Pairs(links)) => {
            let mut used: Vec<usize> = links.iter().flatten().copied().collect();
            used.sort_unstable();
            used.dedup();
            if links.len() != pairs.len()
                || used.len() != links.iter().flatten().count()
                || used.last().is_some_and(|&r| r >= pairs.len())
            {
                return Err(Refusal("Match each item with one partner at most."));
            }
            if used.is_empty() {
                return Err(Refusal("Match at least one pair."));
            }
            Ok(Response::Pairs(links))
        }
        _ => Err(Refusal("That answer does not fit this question.")),
    }
}

/// Separates a part's index from the normalised text in judgement keys of several-answers
/// questions (normalised text has letters, digits and spaces only).
pub(super) const PART_KEY_SEPARATOR: char = '|';

/// Prefix of the judgement keys of bonus answers (parts use their index instead).
pub(super) const BONUS_KEY_PREFIX: &str = "bonus|";

/// Prefix of the judgement keys of number guesses, followed by the team's id.
pub(super) const NUMBER_KEY_PREFIX: &str = "number|";

/// The judgement key of `team`'s guess on a number question, which only takes a bonus or malus.
pub(super) fn number_key(team: u32) -> String {
    format!("{NUMBER_KEY_PREFIX}{team}")
}

/// The judgement key of a typed bonus answer.
pub(super) fn bonus_key(text: &str) -> String {
    format!("{BONUS_KEY_PREFIX}{}", normalize(text))
}

/// The judgement key of a typed answer: its normalised text, prefixed by the part's index for a
/// several-answers question.
pub(super) fn text_key(part: Option<usize>, text: &str) -> String {
    match part {
        Some(p) => format!("{p}{PART_KEY_SEPARATOR}{}", normalize(text)),
        None => normalize(text),
    }
}
