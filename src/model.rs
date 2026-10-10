//! Quiz definitions as authored in the admin editor.

use serde::{Deserialize, Serialize};

/// Fewest answers a question may offer.
pub const MIN_ANSWERS: usize = 2;
/// Most answers a question may offer (one per answer colour/shape).
pub const MAX_ANSWERS: usize = 6;
/// Shortest allowed time limit, in seconds.
pub const MIN_TIME_LIMIT_SECS: u32 = 5;
/// Longest allowed time limit, in seconds.
pub const MAX_TIME_LIMIT_SECS: u32 = 300;

/// A complete quiz: settings plus an ordered list of questions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quiz {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub settings: QuizSettings,
    #[serde(default)]
    pub questions: Vec<Question>,
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
        if self.questions.is_empty() {
            issues.push(ValidationIssue::quiz(
                "The quiz needs at least one question.",
            ));
        }
        for (index, question) in self.questions.iter().enumerate() {
            issues.extend(
                question
                    .problems()
                    .into_iter()
                    .map(|message| ValidationIssue {
                        question: Some(index),
                        message,
                    }),
            );
        }
        issues
    }

    /// Lightweight description used in quiz listings.
    #[must_use]
    pub fn summary(&self) -> QuizSummary {
        QuizSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            question_count: self.questions.len(),
        }
    }
}

/// Per-quiz options chosen by the admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuizSettings {
    /// Time limit for questions that do not set their own.
    pub default_time_limit_secs: u32,
    pub reveal: RevealMode,
    pub multi_scoring: MultiScoring,
    pub advance: AdvanceMode,
    pub phone_display: PhoneDisplay,
}

impl Default for QuizSettings {
    fn default() -> Self {
        Self {
            default_time_limit_secs: 20,
            reveal: RevealMode::default(),
            multi_scoring: MultiScoring::default(),
            advance: AdvanceMode::default(),
            phone_display: PhoneDisplay::default(),
        }
    }
}

/// When correct answers and scores are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevealMode {
    /// After every question, with the current standings.
    #[default]
    AfterEachQuestion,
    /// Only once all questions have been played, keeping the suspense.
    AtEnd,
}

/// How questions with several correct answers are scored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiScoring {
    /// Points only for selecting exactly the correct set.
    #[default]
    AllOrNothing,
    /// Credit proportional to correct picks minus wrong picks.
    Partial,
}

/// Who moves the quiz forward between questions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdvanceMode {
    /// The host clicks to continue.
    #[default]
    Manual,
    /// The quiz continues by itself after a short pause.
    Auto,
}

/// What team leaders see on their phones while answering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhoneDisplay {
    /// Only coloured shapes; the question is read on the big screen.
    #[default]
    ButtonsOnly,
    /// The question and answer texts as well.
    FullText,
}

/// A single question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub text: String,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
    /// Name of an uploaded soundtrack, if any. The host starts it, and the clock with it.
    #[serde(default)]
    pub audio: Option<String>,
    /// Overrides the quiz-wide time limit when set.
    #[serde(default)]
    pub time_limit_secs: Option<u32>,
    pub answers: Vec<Answer>,
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

    /// Effective time limit given the quiz settings.
    #[must_use]
    pub fn time_limit_secs(&self, settings: &QuizSettings) -> u32 {
        self.time_limit_secs
            .unwrap_or(settings.default_time_limit_secs)
    }

    /// Indexes of the correct answers.
    #[must_use]
    pub fn correct_indexes(&self) -> Vec<usize> {
        self.answers
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.correct.then_some(i))
            .collect()
    }

    /// Whether more than one answer is correct.
    #[must_use]
    pub fn is_multi(&self) -> bool {
        self.answers.iter().filter(|a| a.correct).count() > 1
    }

    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.text.trim().is_empty() {
            problems.push("The question has no text.".to_owned());
        }
        if !(MIN_ANSWERS..=MAX_ANSWERS).contains(&self.answers.len()) {
            problems.push(format!(
                "A question needs between {MIN_ANSWERS} and {MAX_ANSWERS} answers."
            ));
        }
        if self.answers.iter().any(|a| a.text.trim().is_empty()) {
            problems.push("Every answer needs a text.".to_owned());
        }
        if !self.answers.iter().any(|a| a.correct) {
            problems.push("At least one answer must be marked correct.".to_owned());
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
pub struct Answer {
    pub text: String,
    #[serde(default)]
    pub correct: bool,
}

/// A problem found by [`Quiz::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Index of the offending question, or `None` for quiz-wide problems.
    pub question: Option<usize>,
    pub message: String,
}

impl ValidationIssue {
    fn quiz(message: impl Into<String>) -> Self {
        Self {
            question: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.question {
            Some(i) => write!(f, "Question {}: {}", i + 1, self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// A quiz as shown in listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizSummary {
    pub id: String,
    pub title: String,
    pub question_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(text: &str, correct: bool) -> Answer {
        Answer {
            text: text.to_owned(),
            correct,
        }
    }

    fn question() -> Question {
        Question {
            text: "Which planet has the most moons?".to_owned(),
            image: None,
            audio: None,
            time_limit_secs: None,
            answers: vec![answer("Jupiter", false), answer("Saturn", true)],
        }
    }

    fn quiz() -> Quiz {
        Quiz {
            questions: vec![question()],
            ..Quiz::new("q1".to_owned(), "Friday".to_owned())
        }
    }

    #[test]
    fn valid_quiz_has_no_issues() {
        assert_eq!(quiz().validate(), Vec::new());
    }

    #[test]
    fn empty_quiz_is_invalid() {
        let issues = Quiz::new("q1".to_owned(), " ".to_owned()).validate();
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.question.is_none()));
    }

    #[test]
    fn question_problems_point_at_the_question() {
        let mut quiz = quiz();
        quiz.questions.push(Question {
            answers: vec![answer("", false)],
            time_limit_secs: Some(1),
            ..Question::default()
        });
        let issues = quiz.validate();
        assert_eq!(issues.len(), 5);
        assert!(issues.iter().all(|i| i.question == Some(1)));
        assert!(issues[0].to_string().starts_with("Question 2: "));
    }

    #[test]
    fn time_limit_falls_back_to_quiz_default() {
        let settings = QuizSettings::default();
        let mut q = question();
        assert_eq!(q.time_limit_secs(&settings), 20);
        q.time_limit_secs = Some(45);
        assert_eq!(q.time_limit_secs(&settings), 45);
    }

    #[test]
    fn correct_indexes_and_multi() {
        let mut q = question();
        assert_eq!(q.correct_indexes(), vec![1]);
        assert!(!q.is_multi());
        q.answers.push(answer("Uranus", true));
        assert_eq!(q.correct_indexes(), vec![1, 2]);
        assert!(q.is_multi());
    }

    #[test]
    fn missing_fields_deserialize_to_defaults() {
        let quiz: Quiz = serde_json::from_str(
            r#"{"id":"x","title":"T","questions":[{"text":"Q","answers":[{"text":"A"}]}]}"#,
        )
        .unwrap();
        assert_eq!(quiz.settings, QuizSettings::default());
        assert!(!quiz.questions[0].answers[0].correct);
    }
}
