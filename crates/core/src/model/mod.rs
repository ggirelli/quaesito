//! Quiz definitions as authored in the admin editor.

use serde::{Deserialize, Serialize};

mod import;
mod kinds;
mod question;
mod quiz;
mod settings;

pub use import::{ImportOutcome, ImportProgress, ImportWarning, QuizSummary, UnknownQuestion};
pub use kinds::{
    BONUS_PERCENTS, Bonus, DEFAULT_BONUS_PERCENT, ImageReveal, MAX_PARTS, MIN_PARTS, NumberRange,
    NumberScoring, Pair, PairItem, Part, PinTolerance, Point,
};
pub use question::{
    Answer, LibraryLink, MAX_ANSWERS, MAX_MULTIPLIER, MAX_TEXT_ANSWER_CHARS, MAX_TIME_LIMIT_SECS,
    MIN_ANSWERS, MIN_TIME_LIMIT_SECS, Question, QuestionKind,
};
pub use quiz::{QUIZ_FORMAT, Quiz};
pub use settings::{
    AdvanceMode, MAX_NETWORK_NAME_BYTES, MAX_NETWORK_PASSWORD_CHARS, MultiScoring, NetworkSlide,
    PhoneDisplay, PointsMode, QuizSettings, RevealMode,
};

/// Whether an uploaded file is a video, from the name the server gave it. Only question and
/// slide pictures can be videos, so a `WebM` file there is a video.
#[must_use]
pub fn is_video(name: &str) -> bool {
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    matches!(ext, "mp4" | "mov" | "webm")
}

/// A problem found by [`Quiz::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Index of the offending question or slide, or `None` for quiz-wide problems.
    pub question: Option<usize>,
    /// What the offending item is called, e.g. "Slide 2".
    pub place: Option<String>,
    pub message: String,
}

impl ValidationIssue {
    fn quiz(message: impl Into<String>) -> Self {
        Self {
            question: None,
            place: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.place {
            Some(place) => write!(f, "{place}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

#[cfg(test)]
mod tests;
