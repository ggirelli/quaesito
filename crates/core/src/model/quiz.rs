//! A quiz: its title, items and settings, and how it is checked before it is played.

use serde::{Deserialize, Serialize};

use super::ValidationIssue;
use super::import::QuizSummary;
use super::question::{MAX_TIME_LIMIT_SECS, MIN_TIME_LIMIT_SECS, Question, QuestionKind};
use super::settings::{QuizSettings, RevealMode};

/// Version of the exported quiz file format. Raise it whenever an older version would misread
/// or lose something in a newer file: a new question kind, setting value or field.
pub const QUIZ_FORMAT: u32 = 3;

/// A complete quiz: settings plus an ordered list of questions and slides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Quiz {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub settings: QuizSettings,
    /// Questions and slides, in playing order.
    #[serde(default)]
    pub questions: Vec<Question>,
    /// When the quiz was created, in seconds since 1970; set when it is first saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
}

impl Quiz {
    /// Creates an empty quiz with default settings.
    #[must_use]
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            settings: QuizSettings::default(),
            questions: Vec::new(),
            created: None,
        }
    }

    /// Lists every problem that prevents this quiz from being played.
    #[must_use]
    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        if self.title.trim().is_empty() {
            issues.push(ValidationIssue::quiz("The quiz needs a title."));
        }
        if !(MIN_TIME_LIMIT_SECS..=MAX_TIME_LIMIT_SECS)
            .contains(&self.settings.default_time_limit_secs)
        {
            issues.push(ValidationIssue::quiz(format!(
                "The default time limit must be between {MIN_TIME_LIMIT_SECS} and {MAX_TIME_LIMIT_SECS} seconds."
            )));
        }
        if let Some(problem) = self.settings.network.problem() {
            issues.push(ValidationIssue::quiz(problem));
        }
        if self.questions.iter().all(Question::is_slide) {
            issues.push(ValidationIssue::quiz(
                "The quiz needs at least one question.",
            ));
        }
        for (index, question) in self.questions.iter().enumerate() {
            let mut problems = question.problems();
            if question.is_results() && self.settings.reveal != RevealMode::AtEnd {
                problems.push(
                    "Results slides only work when the answers are revealed at the end (see the settings)."
                        .to_owned(),
                );
            }
            if question.reveals_image()
                && question.image_hold_secs >= question.time_limit_secs(&self.settings)
            {
                problems.push(
                    "The picture must stay fully visible for less time than the question lasts."
                        .to_owned(),
                );
            }
            issues.extend(problems.into_iter().map(|message| ValidationIssue {
                question: Some(index),
                place: Some(self.label(index)),
                message,
            }));
        }
        issues
    }

    /// Number of real questions, leaving slides out.
    #[must_use]
    pub fn question_count(&self) -> usize {
        self.questions.iter().filter(|q| !q.is_slide()).count()
    }

    /// Position of item `index` among the items of its sort (questions or slides), from 0.
    #[must_use]
    pub fn ordinal(&self, index: usize) -> usize {
        let slide = self.questions[index].is_slide();
        self.questions[..index]
            .iter()
            .filter(|q| q.is_slide() == slide)
            .count()
    }

    /// How item `index` is called in the editor: "Question 3" or "Slide 1".
    #[must_use]
    pub fn label(&self, index: usize) -> String {
        let sort = if self.questions[index].is_slide() {
            "Slide"
        } else {
            "Question"
        };
        format!("{sort} {}", self.ordinal(index) + 1)
    }

    /// Whether some questions could change places with [`Quiz::shuffle_questions`].
    #[must_use]
    pub fn can_shuffle(&self) -> bool {
        self.questions
            .windows(2)
            .any(|pair| !pair[0].is_slide() && !pair[1].is_slide())
    }

    /// Shuffles each run of consecutive questions in place; slides stay where they are and
    /// questions never move past a slide. `random_below(n)` must return a number in `0..n`.
    pub fn shuffle_questions(&mut self, mut random_below: impl FnMut(usize) -> usize) {
        for run in self.questions.split_mut(Question::is_slide) {
            // Fisher–Yates.
            for i in (1..run.len()).rev() {
                run.swap(i, random_below(i + 1).min(i));
            }
        }
    }

    /// Whether some answers could change places with [`Quiz::shuffle_answers`].
    #[must_use]
    pub fn can_shuffle_answers(&self) -> bool {
        self.questions
            .iter()
            .any(|q| q.kind == QuestionKind::Choice && q.answers.len() > 1)
    }

    /// Shuffles the answers of each multiple-choice question; the right ones stay right.
    /// `random_below(n)` must return a number in `0..n`.
    pub fn shuffle_answers(&mut self, mut random_below: impl FnMut(usize) -> usize) {
        for question in &mut self.questions {
            if question.kind != QuestionKind::Choice {
                continue;
            }
            // Fisher–Yates.
            for i in (1..question.answers.len()).rev() {
                question.answers.swap(i, random_below(i + 1).min(i));
            }
        }
    }

    /// Names of every uploaded file the quiz uses: pictures (questions, slides, answers) and
    /// soundtracks.
    #[must_use]
    pub fn media(&self) -> Vec<String> {
        let mut names = Vec::new();
        for question in &self.questions {
            names.extend(question.image.iter().cloned());
            names.extend(question.explanation_image.iter().cloned());
            names.extend(question.explanation_audio.iter().cloned());
            names.extend(question.audio.iter().cloned());
            names.extend(question.answers.iter().filter_map(|a| a.image.clone()));
            match &question.kind {
                QuestionKind::Parts { parts } => {
                    names.extend(parts.iter().filter_map(|p| p.image.clone()));
                }
                QuestionKind::Pairs { pairs } => names.extend(
                    pairs
                        .iter()
                        .flat_map(|p| [&p.left.image, &p.right.image])
                        .flatten()
                        .cloned(),
                ),
                _ => {}
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Replaces every media name with `rename(name)`; `None` drops the reference.
    pub fn rename_media(&mut self, mut rename: impl FnMut(&str) -> Option<String>) {
        let mut apply = |slot: &mut Option<String>| {
            *slot = slot.as_deref().and_then(&mut rename);
        };
        for question in &mut self.questions {
            apply(&mut question.image);
            apply(&mut question.explanation_image);
            apply(&mut question.explanation_audio);
            apply(&mut question.audio);
            for answer in &mut question.answers {
                apply(&mut answer.image);
            }
            match &mut question.kind {
                QuestionKind::Parts { parts } => {
                    for part in parts {
                        apply(&mut part.image);
                    }
                }
                QuestionKind::Pairs { pairs } => {
                    for pair in pairs {
                        apply(&mut pair.left.image);
                        apply(&mut pair.right.image);
                    }
                }
                _ => {}
            }
        }
    }

    /// Lightweight description used in quiz listings.
    #[must_use]
    pub fn summary(&self) -> QuizSummary {
        QuizSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            question_count: self.question_count(),
            issues: self.validate().iter().map(ToString::to_string).collect(),
            created: self.created,
            edited: None,
        }
    }
}
