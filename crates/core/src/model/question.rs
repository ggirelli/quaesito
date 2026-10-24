//! A question, its kind and its answers, and how it is checked.

use serde::{Deserialize, Serialize};

use super::is_video;
use super::kinds::{
    Bonus, ImageReveal, MAX_PARTS, MIN_PARTS, NumberRange, NumberScoring, Pair, Part, PinTolerance,
    Point, deserialize_image_reveal, number_problems, pair_problems,
};
use super::settings::QuizSettings;

#[cfg(doc)]
use super::ImportOutcome;

/// Fewest answers a question may offer.
pub const MIN_ANSWERS: usize = 2;

/// Most answers a question may offer (one per answer colour/shape).
pub const MAX_ANSWERS: usize = 6;

/// Shortest allowed time limit, in seconds.
pub const MIN_TIME_LIMIT_SECS: u32 = 5;

/// Longest allowed time limit, in seconds.
pub const MAX_TIME_LIMIT_SECS: u32 = 300;

/// Most a question's points can be multiplied by.
pub const MAX_MULTIPLIER: u32 = 5;

/// Longest typed answer, in characters.
pub const MAX_TEXT_ANSWER_CHARS: usize = 60;

/// A single question, or a slide between questions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Question {
    /// The question, or the body text of a slide.
    pub text: String,
    /// Text shown with the question, line breaks kept: lyrics, a quote, a riddle…
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub passage: String,
    /// How teams answer. Older quizzes without it are multiple choice.
    #[serde(default)]
    pub kind: QuestionKind,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
    /// How the picture is revealed while the clock runs. Reads the former `blur_image` flag
    /// as [`ImageReveal::Zoom`].
    #[serde(
        default,
        alias = "blur_image",
        deserialize_with = "deserialize_image_reveal"
    )]
    pub image_reveal: ImageReveal,
    /// With a gradual reveal, how many seconds before the buzzer the picture is fully shown: the
    /// reveal takes the rest of the time.
    #[serde(default)]
    pub image_hold_secs: u32,
    /// Name of an uploaded soundtrack, if any. The host starts it, and the clock with it.
    #[serde(default)]
    pub audio: Option<String>,
    /// Overrides the quiz-wide time limit when set.
    #[serde(default)]
    #[cfg_attr(feature = "schema", schemars(range(min = MIN_TIME_LIMIT_SECS, max = MAX_TIME_LIMIT_SECS)))]
    pub time_limit_secs: Option<u32>,
    /// The question is worth this many times a usual one (`None` counts once).
    #[serde(default)]
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = MAX_MULTIPLIER)))]
    pub multiplier: Option<u32>,
    /// The options of a multiple-choice question (kept, but unused, for other kinds).
    #[serde(default)]
    pub answers: Vec<Answer>,
    /// A typed bonus question asked together with this one (see [`Question::takes_bonus`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "schema",
        schemars(description = "A typed bonus question, answered together with this one.")
    )]
    pub bonus: Option<Bonus>,
    /// Shown on the big screen only once the answer is revealed: an explanation, a fun fact…
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub explanation: String,
    /// Name of an uploaded picture or video shown with the [`Question::explanation`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation_image: Option<String>,
    /// Name of an uploaded soundtrack the big screen plays once the answer is revealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation_audio: Option<String>,
    /// What the question is about, e.g. "Geography", shown on the big screen. Library questions
    /// keep the category they come from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    /// The subject within the category, e.g. "Capitals", shown with it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub topic: String,
    /// The library question this one was added from or saved as, to keep the two in step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<LibraryLink>,
}

/// Ties a quiz question to a library question (see [`crate::library::sync`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LibraryLink {
    /// The library question's id.
    pub id: String,
    /// The [`crate::library::fingerprint`] of the question when the quiz and the library last
    /// had the same one: which of the two changed since.
    pub synced: String,
}

/// How a question is answered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum QuestionKind {
    /// Pick one or more of the listed answers.
    #[default]
    Choice,
    /// Type a short answer, right when it matches an accepted one (the judge may amend).
    Text { accepted: Vec<String> },
    /// Enter a number; by default the closest guess wins.
    Number {
        answer: Option<f64>,
        /// The numbers teams can give, if limited: phones then also show a slider.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<NumberRange>,
        #[serde(default, skip_serializing_if = "NumberScoring::is_closest")]
        scoring: NumberScoring,
    },
    /// Put items in order; `items` are listed in the right order and shown shuffled.
    Order { items: Vec<String> },
    /// Is the statement true or false? Played like a two-answer multiple choice.
    TrueFalse { answer: bool },
    /// Several typed answers at once (title and artist, five posters…), each judged on its own;
    /// the question's points are split evenly between the parts.
    Parts { parts: Vec<Part> },
    /// Drop a pin on the question's picture (a map, a floor plan…); closer pins earn more.
    Pin {
        /// The right spot, as fractions of the picture's width and height.
        answer: Option<Point>,
        /// The picture's height divided by its width, so distances are measured evenly.
        aspect: f64,
        #[serde(default)]
        tolerance: PinTolerance,
    },
    /// Match each item on the left with its partner on the right; `pairs` are listed matched,
    /// and the right column is shown shuffled. Each pair rightly matched earns its share.
    Pairs { pairs: Vec<Pair> },
    /// Not a question: a title, text and/or picture shown on the big screen, with nothing to
    /// answer.
    Slide {
        title: String,
        /// A results slide: when answers are revealed at the end, it walks through the answers
        /// to the questions since the previous results slide, then shows the standings (an
        /// intermission, say). Its title heads the standings; it has no text or picture.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        results: bool,
    },
}

impl QuestionKind {
    /// Every kind's name as stored in quiz files. A file from a newer version can name others;
    /// see [`ImportOutcome`].
    pub const NAMES: [&str; 9] = [
        "Choice",
        "Text",
        "Number",
        "Order",
        "TrueFalse",
        "Parts",
        "Pin",
        "Pairs",
        "Slide",
    ];
}

impl Question {
    /// A fresh question with two empty answers, as the editor creates it.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            answers: vec![Answer::default(), Answer::default()],
            ..Self::default()
        }
    }

    /// A fresh, empty slide.
    #[must_use]
    pub fn slide() -> Self {
        Self {
            kind: QuestionKind::Slide {
                title: String::new(),
                results: false,
            },
            ..Self::default()
        }
    }

    /// A fresh results slide.
    #[must_use]
    pub fn results_slide() -> Self {
        Self {
            kind: QuestionKind::Slide {
                title: "Results so far".to_owned(),
                results: true,
            },
            ..Self::default()
        }
    }

    /// Replaces the question's picture. A pin question's spot belonged to the old picture, so it
    /// is cleared and must be clicked again.
    pub fn set_image(&mut self, image: Option<String>) {
        if image != self.image
            && let QuestionKind::Pin { answer, aspect, .. } = &mut self.kind
        {
            *answer = None;
            *aspect = 0.0;
        }
        self.image = image;
    }

    /// How many times a usual question's points this one is worth.
    #[must_use]
    pub fn multiplier(&self) -> u32 {
        self.multiplier.unwrap_or(1)
    }

    /// Whether the question's picture is revealed gradually while the clock runs (never for a
    /// pin question: the picture is the map).
    #[must_use]
    pub fn reveals_image(&self) -> bool {
        self.image.is_some()
            && self.image_reveal != ImageReveal::Shown
            && !matches!(self.kind, QuestionKind::Pin { .. })
    }

    /// Whether the question can carry a [`Bonus`]: those answered without typing words
    /// (multiple choice, true/false, closest number, put in order, pin on a picture, pairs).
    #[must_use]
    pub fn takes_bonus(&self) -> bool {
        matches!(
            self.kind,
            QuestionKind::Choice
                | QuestionKind::TrueFalse { .. }
                | QuestionKind::Number { .. }
                | QuestionKind::Order { .. }
                | QuestionKind::Pin { .. }
                | QuestionKind::Pairs { .. }
        )
    }

    /// Whether this is a slide rather than a question.
    #[must_use]
    pub fn is_slide(&self) -> bool {
        matches!(self.kind, QuestionKind::Slide { .. })
    }

    /// Whether this is a results slide (see [`QuestionKind::Slide`]).
    #[must_use]
    pub fn is_results(&self) -> bool {
        matches!(self.kind, QuestionKind::Slide { results: true, .. })
    }

    /// Effective time limit given the quiz settings.
    #[must_use]
    pub fn time_limit_secs(&self, settings: &QuizSettings) -> u32 {
        self.time_limit_secs
            .unwrap_or(settings.default_time_limit_secs)
    }

    /// Indexes of the correct answers of a multiple-choice (or true/false) question.
    #[must_use]
    pub fn correct_indexes(&self) -> Vec<usize> {
        match self.kind {
            QuestionKind::Choice => self
                .answers
                .iter()
                .enumerate()
                .filter_map(|(i, a)| a.correct.then_some(i))
                .collect(),
            QuestionKind::TrueFalse { answer } => vec![usize::from(!answer)],
            _ => Vec::new(),
        }
    }

    /// The answers teams pick from: a multiple-choice question's options, or True and False.
    #[must_use]
    pub fn choices(&self) -> Vec<String> {
        match self.kind {
            QuestionKind::Choice => self.answers.iter().map(|a| a.text.clone()).collect(),
            QuestionKind::TrueFalse { .. } => vec!["True".to_owned(), "False".to_owned()],
            _ => Vec::new(),
        }
    }

    /// Whether this is a multiple-choice question with more than one correct answer.
    #[must_use]
    pub fn is_multi(&self) -> bool {
        self.correct_indexes().len() > 1
    }

    /// What is missing from the question's bonus question, if it has one it can carry.
    fn bonus_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if let Some(bonus) = &self.bonus
            && self.takes_bonus()
        {
            if bonus.text.trim().is_empty() {
                problems.push("The bonus question has no text.".to_owned());
            }
            if bonus.accepted.iter().all(|a| a.trim().is_empty()) {
                problems.push("The bonus question needs at least one accepted answer.".to_owned());
            }
            if !(1..=100).contains(&bonus.percent) {
                problems.push("A bonus is worth 1 to 100% of the question's points.".to_owned());
            }
        }
        problems
    }

    pub(super) fn problems(&self) -> Vec<String> {
        if let QuestionKind::Slide { title, results } = &self.kind {
            if *results {
                return Vec::new();
            }
            let empty =
                title.trim().is_empty() && self.text.trim().is_empty() && self.image.is_none();
            return if empty {
                vec!["Give the slide a title, some text or a picture.".to_owned()]
            } else {
                Vec::new()
            };
        }
        let mut problems = Vec::new();
        if self.text.trim().is_empty() {
            problems.push("The question has no text.".to_owned());
        }
        match &self.kind {
            QuestionKind::Choice => {
                if !(MIN_ANSWERS..=MAX_ANSWERS).contains(&self.answers.len()) {
                    problems.push(format!(
                        "A question needs between {MIN_ANSWERS} and {MAX_ANSWERS} answers."
                    ));
                }
                if self
                    .answers
                    .iter()
                    .any(|a| a.text.trim().is_empty() && a.image.is_none())
                {
                    problems.push("Every answer needs a text or a picture.".to_owned());
                }
                if !self.answers.iter().any(|a| a.correct) {
                    problems.push("At least one answer must be marked correct.".to_owned());
                }
            }
            QuestionKind::Text { accepted } => {
                if accepted.iter().all(|a| a.trim().is_empty()) {
                    problems.push("Add at least one accepted answer.".to_owned());
                }
            }
            QuestionKind::Number {
                answer,
                range,
                scoring,
            } => problems.extend(number_problems(*answer, *range, *scoring)),
            QuestionKind::Order { items } => {
                if !(MIN_ANSWERS..=MAX_ANSWERS).contains(&items.len()) {
                    problems.push(format!(
                        "Put between {MIN_ANSWERS} and {MAX_ANSWERS} items in order."
                    ));
                }
                if items.iter().any(|item| item.trim().is_empty()) {
                    problems.push("Every item needs a text.".to_owned());
                }
            }
            QuestionKind::Parts { parts } => {
                if !(MIN_PARTS..=MAX_PARTS).contains(&parts.len()) {
                    problems.push(format!(
                        "Ask for between {MIN_PARTS} and {MAX_PARTS} answers."
                    ));
                }
                if parts
                    .iter()
                    .any(|p| p.accepted.iter().all(|a| a.trim().is_empty()))
                {
                    problems.push("Every answer needs at least one accepted spelling.".to_owned());
                }
            }
            QuestionKind::Pairs { pairs } => problems.extend(pair_problems(pairs)),
            QuestionKind::Pin { answer, aspect, .. } => {
                match &self.image {
                    None => problems.push("Add the picture to drop pins on.".to_owned()),
                    Some(image) if is_video(image) => {
                        problems.push("Pins are dropped on a picture, not a video.".to_owned());
                    }
                    Some(_) => {}
                }
                let placed =
                    answer.is_some_and(Point::is_on_picture) && aspect.is_finite() && *aspect > 0.0;
                if !placed {
                    problems.push("Click the right spot on the picture.".to_owned());
                }
            }
            QuestionKind::Slide { .. } | QuestionKind::TrueFalse { .. } => {}
        }
        problems.extend(self.bonus_problems());
        if !(1..=MAX_MULTIPLIER).contains(&self.multiplier()) {
            problems.push(format!(
                "A question can be worth 1 to {MAX_MULTIPLIER} times the usual points."
            ));
        }
        if let Some(limit) = self.time_limit_secs
            && !(MIN_TIME_LIMIT_SECS..=MAX_TIME_LIMIT_SECS).contains(&limit)
        {
            problems.push(format!(
                "The time limit must be between {MIN_TIME_LIMIT_SECS} and {MAX_TIME_LIMIT_SECS} seconds."
            ));
        }
        problems
    }
}

/// One answer option.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Answer {
    /// May be empty when the answer has a picture.
    pub text: String,
    #[serde(default)]
    pub correct: bool,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
}
