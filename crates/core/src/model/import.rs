//! Summaries of saved quizzes, and the progress and outcome of importing one.

use serde::{Deserialize, Serialize};

#[cfg(doc)]
use super::QUIZ_FORMAT;

/// A quiz as shown in listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizSummary {
    pub id: String,
    pub title: String,
    /// Questions, not counting slides.
    pub question_count: usize,
    /// What still keeps the quiz from being playable; empty when it is ready.
    pub issues: Vec<String>,
    /// When it was created and last changed, in seconds since 1970, when known.
    pub created: Option<u64>,
    pub edited: Option<u64>,
}

/// What came of importing a quiz file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportOutcome {
    /// The quiz was saved under this id.
    Imported(String),
    /// Nothing was saved: the file comes from a newer version, so importing it would lose
    /// something. Importing again with the user's consent saves what this version understands.
    NeedsConsent(ImportWarning),
}

/// One line of the import route's answer: progress while the bundled media are processed, then
/// how it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportProgress {
    /// Pictures, videos and soundtracks processed so far, of all the bundle brings.
    Media {
        done: usize,
        total: usize,
    },
    Done(ImportOutcome),
    Failed(String),
}

/// Why an imported quiz would not come through whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportWarning {
    pub title: String,
    /// The file's format, when newer than [`QUIZ_FORMAT`].
    pub newer_format: Option<u32>,
    /// Questions of kinds this version does not know, which would be left out.
    pub unknown: Vec<UnknownQuestion>,
}

/// A question of a kind this version does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownQuestion {
    /// Where it is in the file, e.g. "Question 4".
    pub place: String,
    /// The kind's name, e.g. "Hotspot".
    pub kind: String,
    pub text: String,
}
