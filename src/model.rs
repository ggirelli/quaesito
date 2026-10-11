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
/// Most a question's points can be multiplied by.
pub const MAX_MULTIPLIER: u32 = 5;
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
            let mut problems = question.problems();
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

    /// Names of every uploaded file the quiz uses: pictures (questions, slides, answers) and
    /// soundtracks.
    #[must_use]
    pub fn media(&self) -> Vec<String> {
        let mut names = Vec::new();
        for question in &self.questions {
            names.extend(question.image.iter().cloned());
            names.extend(question.audio.iter().cloned());
            names.extend(question.answers.iter().filter_map(|a| a.image.clone()));
            if let QuestionKind::Parts { parts } = &question.kind {
                names.extend(parts.iter().filter_map(|p| p.image.clone()));
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
            apply(&mut question.audio);
            for answer in &mut question.answers {
                apply(&mut answer.image);
            }
            if let QuestionKind::Parts { parts } = &mut question.kind {
                for part in parts {
                    apply(&mut part.image);
                }
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
    pub time_limit_secs: Option<u32>,
    /// The question is worth this many times a usual one (`None` counts once).
    #[serde(default)]
    pub multiplier: Option<u32>,
    /// The options of a multiple-choice question (kept, but unused, for other kinds).
    #[serde(default)]
    pub answers: Vec<Answer>,
}

/// How a question's picture is revealed on the big screen while the clock runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageReveal {
    /// Shown as is.
    #[default]
    Shown,
    /// Starts as a close-up of its centre and zooms out to the whole picture.
    Zoom,
    /// Starts covered by tiles that drop away one by one.
    Tiles,
}

/// Accepts an [`ImageReveal`], or the former `blur_image` flag (`true` becomes a zoom).
fn deserialize_image_reveal<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ImageReveal, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Reveal(ImageReveal),
        Blur(bool),
    }
    Ok(match Stored::deserialize(deserializer)? {
        Stored::Reveal(reveal) => reveal,
        Stored::Blur(true) => ImageReveal::Zoom,
        Stored::Blur(false) => ImageReveal::Shown,
    })
}

/// A spot on a picture, as fractions of its width (`x`) and height (`y`) from the top left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Whether both coordinates are within the picture.
    #[must_use]
    pub fn is_on_picture(self) -> bool {
        (0.0..=1.0).contains(&self.x) && (0.0..=1.0).contains(&self.y)
    }

    /// Distance to `other` on a picture `aspect` times as high as wide, in picture widths.
    #[must_use]
    pub fn distance(self, other: Self, aspect: f64) -> f64 {
        (self.x - other.x).hypot((self.y - other.y) * aspect)
    }
}

/// How close a pin must be: within the first distance it earns full points, beyond the second
/// none, and in between a share falling evenly. Distances are in picture widths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinTolerance {
    Tight,
    #[default]
    Normal,
    Loose,
}

impl PinTolerance {
    /// Distances (in picture widths) up to which a pin earns all, then some, of the points.
    #[must_use]
    pub fn distances(self) -> (f64, f64) {
        match self {
            Self::Tight => (0.02, 0.15),
            Self::Normal => (0.04, 0.3),
            Self::Loose => (0.08, 0.5),
        }
    }
}

/// One answer of a [`QuestionKind::Parts`] question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    /// What to type, e.g. "Title" or "Poster 3".
    #[serde(default)]
    pub label: String,
    /// Accepted spellings, matched like a typed answer.
    #[serde(default)]
    pub accepted: Vec<String>,
    /// A picture just for this part (a poster, a face…).
    #[serde(default)]
    pub image: Option<String>,
}

/// Fewest parts a several-answers question has.
pub const MIN_PARTS: usize = 2;
/// Most parts a several-answers question has.
pub const MAX_PARTS: usize = 8;

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
            QuestionKind::Pin { answer, aspect, .. } => {
                if self.image.is_none() {
                    problems.push("Add the picture to drop pins on.".to_owned());
                }
                let placed =
                    answer.is_some_and(Point::is_on_picture) && aspect.is_finite() && *aspect > 0.0;
                if !placed {
                    problems.push("Click the right spot on the picture.".to_owned());
                }
            }
            QuestionKind::Slide { .. } | QuestionKind::TrueFalse { .. } => {}
        }
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
            image_reveal: ImageReveal::Shown,
            image_hold_secs: 0,
            audio: None,
            time_limit_secs: None,
            multiplier: None,
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
    fn media_lists_and_renames_every_upload() {
        let mut quiz = quiz();
        quiz.questions[0].image = Some("q.jpg".to_owned());
        quiz.questions[0].audio = Some("s.mp3".to_owned());
        quiz.questions[0].answers[1].image = Some("a.jpg".to_owned());
        quiz.questions.push(Question {
            image: Some("q.jpg".to_owned()),
            ..Question::slide()
        });
        assert_eq!(quiz.media(), ["a.jpg", "q.jpg", "s.mp3"]);
        quiz.rename_media(|name| (name != "s.mp3").then(|| format!("new-{name}")));
        assert_eq!(quiz.media(), ["new-a.jpg", "new-q.jpg"]);
        assert_eq!(quiz.questions[0].audio, None);
    }

    #[test]
    fn the_picture_hold_must_leave_time_for_the_reveal() {
        let mut quiz = quiz();
        quiz.questions[0].image = Some("p.jpg".to_owned());
        quiz.questions[0].image_reveal = ImageReveal::Tiles;
        quiz.questions[0].image_hold_secs = 20; // the default limit
        assert_eq!(quiz.validate().len(), 1);
        quiz.questions[0].image_hold_secs = 5;
        assert_eq!(quiz.validate(), Vec::new());
        // Without a gradual reveal the hold is ignored.
        quiz.questions[0].image_hold_secs = 60;
        quiz.questions[0].image_reveal = ImageReveal::Shown;
        assert_eq!(quiz.validate(), Vec::new());
    }

    #[test]
    fn image_reveal_reads_the_former_blur_flag() {
        let reveal = |json: &str| {
            serde_json::from_str::<Question>(&format!(r#"{{"text":"Q",{json}}}"#))
                .unwrap()
                .image_reveal
        };
        assert_eq!(reveal(r#""blur_image":true"#), ImageReveal::Zoom);
        assert_eq!(reveal(r#""blur_image":false"#), ImageReveal::Shown);
        assert_eq!(reveal(r#""image_reveal":"Tiles""#), ImageReveal::Tiles);
        assert_eq!(reveal(r#""answers":[]"#), ImageReveal::Shown);
    }

    #[test]
    fn true_false_picks_one_of_two() {
        let mut q = Question {
            text: "The Moon is a planet.".to_owned(),
            kind: QuestionKind::TrueFalse { answer: false },
            ..Question::default()
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        assert_eq!(q.choices(), ["True", "False"]);
        assert_eq!(q.correct_indexes(), [1]);
        q.kind = QuestionKind::TrueFalse { answer: true };
        assert_eq!(q.correct_indexes(), [0]);
        assert!(!q.is_multi());
    }

    #[test]
    fn parts_need_accepted_answers_and_count_their_pictures() {
        let part = |label: &str, accepted: &str| Part {
            label: label.to_owned(),
            accepted: vec![accepted.to_owned()],
            image: None,
        };
        let mut quiz = quiz();
        quiz.questions[0].kind = QuestionKind::Parts {
            parts: vec![part("Title", "Yesterday")],
        };
        assert_eq!(quiz.validate().len(), 1);
        quiz.questions[0].kind = QuestionKind::Parts {
            parts: vec![
                part("Title", "Yesterday"),
                Part {
                    image: Some("poster.jpg".to_owned()),
                    ..part("Artist", " ")
                },
            ],
        };
        assert_eq!(quiz.validate().len(), 1);
        assert_eq!(quiz.media(), ["poster.jpg"]);
        quiz.rename_media(|_| Some("new.jpg".to_owned()));
        assert_eq!(quiz.media(), ["new.jpg"]);
    }

    #[test]
    fn a_new_picture_clears_the_pin_spot() {
        let mut q = Question {
            image: Some("old.jpg".to_owned()),
            kind: QuestionKind::Pin {
                answer: Some(Point { x: 0.5, y: 0.5 }),
                aspect: 0.75,
                tolerance: PinTolerance::Normal,
            },
            ..Question::default()
        };
        q.set_image(Some("old.jpg".to_owned()));
        assert!(matches!(
            q.kind,
            QuestionKind::Pin {
                answer: Some(_),
                ..
            }
        ));
        q.set_image(Some("new.jpg".to_owned()));
        assert!(matches!(
            q.kind,
            QuestionKind::Pin { answer: None, aspect, .. } if aspect.abs() < f64::EPSILON
        ));
    }

    #[test]
    fn pins_need_a_picture_and_a_spot() {
        let mut q = Question {
            text: "Where is Rome?".to_owned(),
            kind: QuestionKind::Pin {
                answer: None,
                aspect: 0.75,
                tolerance: PinTolerance::Normal,
            },
            ..Question::default()
        };
        assert_eq!(q.problems().len(), 2);
        q.image = Some("italy.jpg".to_owned());
        q.kind = QuestionKind::Pin {
            answer: Some(Point { x: 0.45, y: 0.5 }),
            aspect: 0.75,
            tolerance: PinTolerance::Normal,
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        q.image_reveal = ImageReveal::Tiles;
        assert!(!q.reveals_image());
        // Distances count the picture's shape: a quarter of the height of a 4:3 picture is
        // 0.1875 of its width.
        let a = Point { x: 0.5, y: 0.25 };
        let b = Point { x: 0.5, y: 0.5 };
        assert!((a.distance(b, 0.75) - 0.1875).abs() < 1e-9);
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
