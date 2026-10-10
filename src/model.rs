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
/// Longest typed answer, in characters.
pub const MAX_TEXT_ANSWER_CHARS: usize = 60;

/// A complete quiz: settings plus an ordered list of questions and slides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quiz {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub settings: QuizSettings,
    /// Questions and slides, in playing order.
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
        if self.questions.iter().all(Question::is_slide) {
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
                        place: Some(self.label(index)),
                        message,
                    }),
            );
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

    /// Lightweight description used in quiz listings.
    #[must_use]
    pub fn summary(&self) -> QuizSummary {
        QuizSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            question_count: self.question_count(),
            issues: self.validate().iter().map(ToString::to_string).collect(),
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
    pub points: PointsMode,
    /// Each team may double one question's points.
    pub joker: bool,
    pub advance: AdvanceMode,
    pub phone_display: PhoneDisplay,
}

impl Default for QuizSettings {
    fn default() -> Self {
        Self {
            default_time_limit_secs: 20,
            reveal: RevealMode::default(),
            multi_scoring: MultiScoring::default(),
            points: PointsMode::default(),
            joker: false,
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

/// What a correct answer is worth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointsMode {
    /// Faster answers earn more: from 1000 points when instant down to 500 at the buzzer.
    #[default]
    Speed,
    /// Every correct answer earns 1000 points, however long it took.
    Flat,
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

/// A single question, or a slide between questions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// The question, or the body text of a slide.
    pub text: String,
    /// How teams answer. Older quizzes without it are multiple choice.
    #[serde(default)]
    pub kind: QuestionKind,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
    /// Show the picture blurred at first, sharpening as the clock runs out.
    #[serde(default)]
    pub blur_image: bool,
    /// Name of an uploaded soundtrack, if any. The host starts it, and the clock with it.
    #[serde(default)]
    pub audio: Option<String>,
    /// Overrides the quiz-wide time limit when set.
    #[serde(default)]
    pub time_limit_secs: Option<u32>,
    /// The options of a multiple-choice question (kept, but unused, for other kinds).
    #[serde(default)]
    pub answers: Vec<Answer>,
}

/// How a question is answered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum QuestionKind {
    /// Pick one or more of the listed answers.
    #[default]
    Choice,
    /// Type a short answer, right when it matches an accepted one (the judge may amend).
    Text { accepted: Vec<String> },
    /// Enter a number; the closest guess wins.
    Number { answer: Option<f64> },
    /// Put items in order; `items` are listed in the right order and shown shuffled.
    Order { items: Vec<String> },
    /// Not a question: a title, text and/or picture shown on the big screen, with nothing to
    /// answer.
    Slide { title: String },
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
            },
            ..Self::default()
        }
    }

    /// Whether this is a slide rather than a question.
    #[must_use]
    pub fn is_slide(&self) -> bool {
        matches!(self.kind, QuestionKind::Slide { .. })
    }

    /// Effective time limit given the quiz settings.
    #[must_use]
    pub fn time_limit_secs(&self, settings: &QuizSettings) -> u32 {
        self.time_limit_secs
            .unwrap_or(settings.default_time_limit_secs)
    }

    /// Indexes of the correct answers of a multiple-choice question.
    #[must_use]
    pub fn correct_indexes(&self) -> Vec<usize> {
        if self.kind != QuestionKind::Choice {
            return Vec::new();
        }
        self.answers
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.correct.then_some(i))
            .collect()
    }

    /// Whether this is a multiple-choice question with more than one correct answer.
    #[must_use]
    pub fn is_multi(&self) -> bool {
        self.correct_indexes().len() > 1
    }

    fn problems(&self) -> Vec<String> {
        if let QuestionKind::Slide { title } = &self.kind {
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
            QuestionKind::Number { answer } => {
                if !answer.is_some_and(f64::is_finite) {
                    problems.push("Enter the correct number.".to_owned());
                }
            }
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
            QuestionKind::Slide { .. } => {}
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
    /// May be empty when the answer has a picture.
    pub text: String,
    #[serde(default)]
    pub correct: bool,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
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

/// A quiz as shown in listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizSummary {
    pub id: String,
    pub title: String,
    /// Questions, not counting slides.
    pub question_count: usize,
    /// What still keeps the quiz from being playable; empty when it is ready.
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(text: &str, correct: bool) -> Answer {
        Answer {
            text: text.to_owned(),
            correct,
            image: None,
        }
    }

    fn question() -> Question {
        Question {
            text: "Which planet has the most moons?".to_owned(),
            kind: QuestionKind::Choice,
            image: None,
            blur_image: false,
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
    fn slides_are_numbered_apart_and_need_content() {
        let mut quiz = quiz();
        quiz.questions.insert(0, Question::slide());
        quiz.questions.push(question());
        assert_eq!(quiz.question_count(), 2);
        assert_eq!(quiz.label(0), "Slide 1");
        assert_eq!(quiz.label(2), "Question 2");
        let issues = quiz.validate();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].to_string().starts_with("Slide 1: "));
        quiz.questions[0].kind = QuestionKind::Slide {
            title: "Round 2".to_owned(),
        };
        assert_eq!(quiz.validate(), Vec::new());
        // Slides alone do not make a quiz.
        quiz.questions.retain(Question::is_slide);
        assert_eq!(quiz.validate().len(), 1);
    }

    #[test]
    fn shuffling_keeps_questions_between_their_slides() {
        let named = |text: &str| Question {
            text: text.to_owned(),
            ..question()
        };
        let slide = |title: &str| Question {
            kind: QuestionKind::Slide {
                title: title.to_owned(),
            },
            ..Question::default()
        };
        let mut quiz = quiz();
        quiz.questions = vec![
            named("a"),
            named("b"),
            named("c"),
            slide("Round 2"),
            named("d"),
            slide("Round 3"),
            named("e"),
            named("f"),
        ];
        assert!(quiz.can_shuffle());
        // Always picking 0 rotates each run.
        quiz.shuffle_questions(|_| 0);
        let order: Vec<&str> = quiz
            .questions
            .iter()
            .map(|q| match &q.kind {
                QuestionKind::Slide { title } => title.as_str(),
                _ => q.text.as_str(),
            })
            .collect();
        assert_eq!(order, ["b", "c", "a", "Round 2", "d", "Round 3", "f", "e"]);

        quiz.questions = vec![named("a"), slide("Round 2"), named("b")];
        assert!(!quiz.can_shuffle());
    }

    #[test]
    fn answers_need_a_text_or_a_picture() {
        let mut q = question();
        q.answers[0].text = " ".to_owned();
        assert_eq!(
            q.problems(),
            vec!["Every answer needs a text or a picture."]
        );
        q.answers[0].image = Some("flag.jpg".to_owned());
        assert_eq!(q.problems(), Vec::<String>::new());
    }

    #[test]
    fn order_questions_need_two_to_six_items() {
        let mut q = Question {
            text: "Oldest first".to_owned(),
            kind: QuestionKind::Order {
                items: vec!["Rome".to_owned()],
            },
            ..Question::default()
        };
        assert_eq!(q.problems().len(), 1);
        q.kind = QuestionKind::Order {
            items: vec!["Rome".to_owned(), String::new()],
        };
        assert_eq!(q.problems(), vec!["Every item needs a text."]);
        q.kind = QuestionKind::Order {
            items: vec!["Rome".to_owned(), "Paris".to_owned()],
        };
        assert_eq!(q.problems(), Vec::<String>::new());
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
    fn text_and_number_questions_need_their_answer() {
        let mut q = Question {
            text: "Who painted the Mona Lisa?".to_owned(),
            kind: QuestionKind::Text {
                accepted: vec![" ".to_owned()],
            },
            ..Question::default()
        };
        assert_eq!(q.problems(), vec!["Add at least one accepted answer."]);
        q.kind = QuestionKind::Text {
            accepted: vec!["Leonardo".to_owned()],
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        q.kind = QuestionKind::Number { answer: None };
        assert_eq!(q.problems(), vec!["Enter the correct number."]);
        q.kind = QuestionKind::Number {
            answer: Some(330.0),
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        // Leftover choices of a converted question do not count.
        q.answers = vec![answer("a", true), answer("b", true)];
        assert!(!q.is_multi());
        assert_eq!(q.correct_indexes(), Vec::<usize>::new());
    }

    #[test]
    fn missing_fields_deserialize_to_defaults() {
        let quiz: Quiz = serde_json::from_str(
            r#"{"id":"x","title":"T","questions":[{"text":"Q","answers":[{"text":"A"}]}]}"#,
        )
        .unwrap();
        assert_eq!(quiz.settings, QuizSettings::default());
        assert_eq!(quiz.questions[0].kind, QuestionKind::Choice);
        assert!(!quiz.questions[0].answers[0].correct);
    }
}
