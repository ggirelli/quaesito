//! Quiz definitions as authored in the admin editor.

use serde::{Deserialize, Serialize};

/// Whether an uploaded file is a video, from the name the server gave it. Only question and
/// slide pictures can be videos, so a `WebM` file there is a video.
#[must_use]
pub fn is_video(name: &str) -> bool {
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    matches!(ext, "mp4" | "mov" | "webm")
}

/// Version of the exported quiz file format. Raise it whenever an older version would misread
/// or lose something in a newer file: a new question kind, setting value or field.
pub const QUIZ_FORMAT: u32 = 2;

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
/// Longest Wi-Fi network name, in bytes (the limit of the standard).
pub const MAX_NETWORK_NAME_BYTES: usize = 32;
/// Longest Wi-Fi password, in characters (the limit of WPA).
pub const MAX_NETWORK_PASSWORD_CHARS: usize = 63;

/// A complete quiz: settings plus an ordered list of questions and slides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
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

/// Per-quiz options chosen by the admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default)]
pub struct QuizSettings {
    /// Time limit for questions that do not set their own.
    #[cfg_attr(test, schemars(range(min = MIN_TIME_LIMIT_SECS, max = MAX_TIME_LIMIT_SECS)))]
    pub default_time_limit_secs: u32,
    pub reveal: RevealMode,
    pub multi_scoring: MultiScoring,
    pub points: PointsMode,
    /// Each team may double one question's points.
    pub joker: bool,
    pub advance: AdvanceMode,
    pub phone_display: PhoneDisplay,
    /// A first slide telling the room how to get on the Wi-Fi.
    pub network: NetworkSlide,
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
            network: NetworkSlide::default(),
        }
    }
}

/// The Wi-Fi network teams join before the game, shown on a first slide with a QR code that
/// connects phones to it. The name and password are kept while the slide is off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default)]
pub struct NetworkSlide {
    /// Whether the game opens on the slide, before the join code.
    pub enabled: bool,
    /// The network name (SSID).
    pub name: String,
    /// Empty for an open network.
    pub password: String,
}

impl NetworkSlide {
    /// What keeps the slide from being shown, if anything.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        if !self.enabled {
            None
        } else if self.name.trim().is_empty() {
            Some("The Wi-Fi slide needs the network name.".to_owned())
        } else if self.name.len() > MAX_NETWORK_NAME_BYTES {
            Some(format!(
                "A Wi-Fi network name has at most {MAX_NETWORK_NAME_BYTES} characters."
            ))
        } else if self.password.chars().count() > MAX_NETWORK_PASSWORD_CHARS {
            Some(format!(
                "A Wi-Fi password has at most {MAX_NETWORK_PASSWORD_CHARS} characters."
            ))
        } else {
            None
        }
    }

    /// The text of a QR code that joins the network, as phone cameras read it.
    #[must_use]
    pub fn qr_text(&self) -> String {
        fn escape(text: &str) -> String {
            text.chars()
                .flat_map(|c| {
                    let escaped = matches!(c, '\\' | ';' | ',' | ':' | '"');
                    escaped.then_some('\\').into_iter().chain([c])
                })
                .collect()
        }
        let name = escape(&self.name);
        if self.password.is_empty() {
            format!("WIFI:T:nopass;S:{name};;")
        } else {
            format!("WIFI:T:WPA;S:{name};P:{};;", escape(&self.password))
        }
    }
}

/// When correct answers and scores are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum RevealMode {
    /// After every question, with the current standings.
    #[default]
    AfterEachQuestion,
    /// Only once all questions have been played, keeping the suspense.
    AtEnd,
}

/// How questions with several correct answers are scored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum MultiScoring {
    /// Points only for selecting exactly the correct set.
    #[default]
    AllOrNothing,
    /// Credit proportional to correct picks minus wrong picks.
    Partial,
}

/// What a correct answer is worth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum PointsMode {
    /// Faster answers earn more: from 1000 points when instant down to 500 at the buzzer.
    #[default]
    Speed,
    /// Every correct answer earns 1000 points, however long it took.
    Flat,
}

/// Who moves the quiz forward between questions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum AdvanceMode {
    /// The host clicks to continue.
    #[default]
    Manual,
    /// The quiz continues by itself after a short pause.
    Auto,
}

/// What team leaders see on their phones while answering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum PhoneDisplay {
    /// Only coloured shapes; the question is read on the big screen.
    #[default]
    ButtonsOnly,
    /// The question and answer texts as well.
    FullText,
}

/// A single question, or a slide between questions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
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
    #[cfg_attr(test, schemars(range(min = MIN_TIME_LIMIT_SECS, max = MAX_TIME_LIMIT_SECS)))]
    pub time_limit_secs: Option<u32>,
    /// The question is worth this many times a usual one (`None` counts once).
    #[serde(default)]
    #[cfg_attr(test, schemars(range(min = 1, max = MAX_MULTIPLIER)))]
    pub multiplier: Option<u32>,
    /// The options of a multiple-choice question (kept, but unused, for other kinds).
    #[serde(default)]
    pub answers: Vec<Answer>,
    /// A typed bonus question asked together with this one (see [`Question::takes_bonus`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        test,
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct LibraryLink {
    /// The library question's id.
    pub id: String,
    /// The [`crate::library::fingerprint`] of the question when the quiz and the library last
    /// had the same one: which of the two changed since.
    pub synced: String,
}

/// A bonus question: teams type its answer together with the main answer, and the judge can
/// amend the verdict as for typed answers. A right answer adds a share of the question's points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Bonus {
    /// The bonus question, e.g. "Who sings it?".
    #[serde(default)]
    pub text: String,
    /// Accepted spellings, matched like a typed answer.
    #[serde(default)]
    pub accepted: Vec<String>,
    /// What a right answer adds, in percent of the points the main answer can earn.
    #[serde(default = "default_bonus_percent")]
    pub percent: u32,
}

/// What a bonus is worth unless the quiz says otherwise, in percent.
pub const DEFAULT_BONUS_PERCENT: u32 = 50;
/// Bonus values the editor offers, in percent.
pub const BONUS_PERCENTS: [u32; 3] = [25, 50, 100];

fn default_bonus_percent() -> u32 {
    DEFAULT_BONUS_PERCENT
}

impl Default for Bonus {
    fn default() -> Self {
        Self {
            text: String::new(),
            accepted: Vec::new(),
            percent: DEFAULT_BONUS_PERCENT,
        }
    }
}

/// How a question's picture is revealed on the big screen while the clock runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Point {
    #[cfg_attr(test, schemars(range(min = 0.0, max = 1.0)))]
    pub x: f64,
    #[cfg_attr(test, schemars(range(min = 0.0, max = 1.0)))]
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
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

/// One side of a [`QuestionKind::Pairs`] pair: a text, a picture, or both.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct PairItem {
    #[serde(default)]
    pub text: String,
    /// Name of an uploaded picture, if any.
    #[serde(default)]
    pub image: Option<String>,
}

impl PairItem {
    /// Whether the item has neither text nor picture.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.image.is_none()
    }

    /// What tells the item apart from the others in its column.
    fn identity(&self) -> (String, Option<&str>) {
        (self.text.trim().to_lowercase(), self.image.as_deref())
    }
}

/// Two items that go together in a [`QuestionKind::Pairs`] question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Pair {
    #[serde(default)]
    pub left: PairItem,
    #[serde(default)]
    pub right: PairItem,
}

/// Whether two of the items are alike, so teams could not tell which goes where.
fn has_twins<'a>(items: impl Iterator<Item = &'a PairItem>) -> bool {
    let mut seen = Vec::new();
    for item in items.filter(|i| !i.is_empty()) {
        let identity = item.identity();
        if seen.contains(&identity) {
            return true;
        }
        seen.push(identity);
    }
    false
}

/// What is wrong with a pairs question's pairs: too few or too many, empty sides, or items a
/// team could not tell apart.
fn pair_problems(pairs: &[Pair]) -> Vec<String> {
    let mut problems = Vec::new();
    if !(MIN_ANSWERS..=MAX_ANSWERS).contains(&pairs.len()) {
        problems.push(format!(
            "Make between {MIN_ANSWERS} and {MAX_ANSWERS} pairs."
        ));
    }
    if pairs
        .iter()
        .any(|p| p.left.is_empty() || p.right.is_empty())
    {
        problems.push("Both sides of every pair need a text or a picture.".to_owned());
    }
    if has_twins(pairs.iter().map(|p| &p.left)) || has_twins(pairs.iter().map(|p| &p.right)) {
        problems.push(
            "Two items in a column are the same: teams could not tell them apart.".to_owned(),
        );
    }
    problems
}

/// What is missing from a closest-number question: its answer, and a sensible range around it.
fn number_problems(answer: Option<f64>, range: Option<NumberRange>) -> Vec<String> {
    let mut problems = Vec::new();
    if !answer.is_some_and(f64::is_finite) {
        problems.push("Enter the correct number.".to_owned());
    }
    if let Some(range) = range {
        if !range.is_valid() {
            problems.push("The range must go from a smaller to a larger number.".to_owned());
        } else if answer.is_some_and(|a| !range.contains(a)) {
            problems.push("The correct number must be within the range.".to_owned());
        }
    }
    problems
}

/// The numbers a closest-number question accepts, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct NumberRange {
    pub min: f64,
    pub max: f64,
}

impl NumberRange {
    /// Whether the range goes from a smaller to a larger finite number.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min < self.max
    }

    #[must_use]
    pub fn contains(self, value: f64) -> bool {
        (self.min..=self.max).contains(&value)
    }

    /// The slider's step: whole numbers when both ends are, any number otherwise.
    #[must_use]
    pub fn whole(self) -> bool {
        self.min.fract() == 0.0 && self.max.fract() == 0.0
    }
}

/// Fewest parts a several-answers question has.
pub const MIN_PARTS: usize = 2;
/// Most parts a several-answers question has.
pub const MAX_PARTS: usize = 8;

/// How a question is answered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum QuestionKind {
    /// Pick one or more of the listed answers.
    #[default]
    Choice,
    /// Type a short answer, right when it matches an accepted one (the judge may amend).
    Text { accepted: Vec<String> },
    /// Enter a number; the closest guess wins.
    Number {
        answer: Option<f64>,
        /// The numbers teams can give, if limited: phones then also show a slider.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<NumberRange>,
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

    fn problems(&self) -> Vec<String> {
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
            QuestionKind::Number { answer, range } => {
                problems.extend(number_problems(*answer, *range));
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
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
            bonus: None,
            passage: String::new(),
            explanation: String::new(),
            explanation_image: None,
            explanation_audio: None,
            category: String::new(),
            topic: String::new(),
            library: None,
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
    fn bonus_questions_need_text_and_an_answer_on_closed_questions_only() {
        let mut question = Question {
            text: "Q".to_owned(),
            kind: QuestionKind::TrueFalse { answer: true },
            bonus: Some(Bonus::default()),
            ..Question::default()
        };
        assert_eq!(question.problems().len(), 2);
        question.bonus = Some(Bonus {
            text: "Why?".to_owned(),
            accepted: vec!["Because".to_owned()],
            percent: 50,
        });
        assert_eq!(question.problems(), Vec::<String>::new());
        // Typed questions cannot carry one: it is ignored there.
        question.kind = QuestionKind::Text {
            accepted: vec!["A".to_owned()],
        };
        assert!(!question.takes_bonus());
        // Saved quizzes without a bonus stay as they were.
        question.bonus = None;
        assert!(!serde_json::to_string(&question).unwrap().contains("bonus"));
    }

    #[test]
    fn results_slides_need_answers_revealed_at_the_end() {
        let mut quiz = Quiz::new("id".to_owned(), "Quiz".to_owned());
        quiz.questions.push(Question {
            text: "Q".to_owned(),
            answers: vec![
                Answer {
                    text: "a".to_owned(),
                    correct: true,
                    image: None,
                },
                Answer {
                    text: "b".to_owned(),
                    correct: false,
                    image: None,
                },
            ],
            ..Question::default()
        });
        quiz.questions.push(Question::results_slide());
        quiz.settings.reveal = RevealMode::AfterEachQuestion;
        let issues = quiz.validate();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].question, Some(1));
        quiz.settings.reveal = RevealMode::AtEnd;
        assert_eq!(quiz.validate(), Vec::new());
        // Plain slides are saved without the flag.
        let plain = serde_json::to_string(&Question::slide()).unwrap();
        assert!(!plain.contains("results"));
        assert!(Question::results_slide().is_slide());
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
            results: false,
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
                results: false,
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
                QuestionKind::Slide { title, .. } => title.as_str(),
                _ => q.text.as_str(),
            })
            .collect();
        assert_eq!(order, ["b", "c", "a", "Round 2", "d", "Round 3", "f", "e"]);

        quiz.questions = vec![named("a"), slide("Round 2"), named("b")];
        assert!(!quiz.can_shuffle());
    }

    #[test]
    fn shuffles_multiple_choice_answers_only() {
        let mut choice = question();
        choice.answers.push(answer("Mars", false));
        let mut text = question();
        text.kind = QuestionKind::Text {
            accepted: vec!["Saturn".to_owned()],
        };
        let mut quiz = quiz();
        quiz.questions = vec![choice, text.clone()];
        assert!(quiz.can_shuffle_answers());
        // Always picking 0 rotates the answers.
        quiz.shuffle_answers(|_| 0);
        let answers: Vec<(&str, bool)> = quiz.questions[0]
            .answers
            .iter()
            .map(|a| (a.text.as_str(), a.correct))
            .collect();
        assert_eq!(
            answers,
            [("Saturn", true), ("Mars", false), ("Jupiter", false)]
        );
        assert_eq!(quiz.questions[1], text);

        quiz.questions = vec![text];
        assert!(!quiz.can_shuffle_answers());
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

    fn pair(left: &str, right: &str) -> Pair {
        Pair {
            left: PairItem {
                text: left.to_owned(),
                image: None,
            },
            right: PairItem {
                text: right.to_owned(),
                image: None,
            },
        }
    }

    #[test]
    fn pairs_questions_need_two_to_six_full_and_distinct_pairs() {
        let mut q = Question {
            text: "Match the capitals".to_owned(),
            kind: QuestionKind::Pairs {
                pairs: vec![pair("France", "Paris")],
            },
            ..Question::default()
        };
        assert_eq!(q.problems(), vec!["Make between 2 and 6 pairs."]);
        q.kind = QuestionKind::Pairs {
            pairs: vec![pair("France", "Paris"), pair("Italy", " ")],
        };
        assert_eq!(
            q.problems(),
            vec!["Both sides of every pair need a text or a picture."]
        );
        q.kind = QuestionKind::Pairs {
            pairs: vec![pair("France", "Paris"), pair("Italy", "paris ")],
        };
        assert_eq!(
            q.problems(),
            vec!["Two items in a column are the same: teams could not tell them apart."]
        );
        let mut pairs = vec![pair("France", "Paris"), pair("Italy", "")];
        pairs[1].right.image = Some("rome.jpg".to_owned());
        q.kind = QuestionKind::Pairs { pairs };
        assert_eq!(q.problems(), Vec::<String>::new());
        assert!(q.takes_bonus());
    }

    #[test]
    fn media_lists_and_renames_every_upload() {
        let mut quiz = quiz();
        quiz.questions[0].image = Some("q.jpg".to_owned());
        quiz.questions[0].audio = Some("s.mp3".to_owned());
        quiz.questions[0].answers[1].image = Some("a.jpg".to_owned());
        quiz.questions[0].explanation_image = Some("e.mp4".to_owned());
        quiz.questions[0].explanation_audio = Some("f.mp3".to_owned());
        quiz.questions.push(Question {
            image: Some("q.jpg".to_owned()),
            ..Question::slide()
        });
        let mut pairs = vec![pair("Paris", ""), pair("", "Rome")];
        pairs[0].right.image = Some("r.jpg".to_owned());
        pairs[1].left.image = Some("l.jpg".to_owned());
        quiz.questions.push(Question {
            kind: QuestionKind::Pairs { pairs },
            ..Question::default()
        });
        assert_eq!(
            quiz.media(),
            [
                "a.jpg", "e.mp4", "f.mp3", "l.jpg", "q.jpg", "r.jpg", "s.mp3"
            ]
        );
        quiz.rename_media(|name| (name != "s.mp3").then(|| format!("new-{name}")));
        assert_eq!(
            quiz.media(),
            [
                "new-a.jpg",
                "new-e.mp4",
                "new-f.mp3",
                "new-l.jpg",
                "new-q.jpg",
                "new-r.jpg"
            ]
        );
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
    fn the_network_slide_needs_a_name_and_escapes_its_qr_code() {
        let mut network = NetworkSlide {
            name: "Pub; \"Guest\"".to_owned(),
            ..NetworkSlide::default()
        };
        assert_eq!(network.problem(), None);
        network.enabled = true;
        assert_eq!(network.qr_text(), r#"WIFI:T:nopass;S:Pub\; \"Guest\";;"#);
        network.password = r"a:b,c\d".to_owned();
        assert_eq!(
            network.qr_text(),
            r#"WIFI:T:WPA;S:Pub\; \"Guest\";P:a\:b\,c\\d;;"#
        );
        assert_eq!(network.problem(), None);
        network.name = " ".to_owned();
        assert!(network.problem().is_some());
        network.name = "x".repeat(MAX_NETWORK_NAME_BYTES + 1);
        assert!(network.problem().is_some());
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
        q.kind = QuestionKind::Number {
            answer: None,
            range: None,
        };
        assert_eq!(q.problems(), vec!["Enter the correct number."]);
        q.kind = QuestionKind::Number {
            answer: Some(330.0),
            range: None,
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        let range = |min, max| Some(NumberRange { min, max });
        q.kind = QuestionKind::Number {
            answer: Some(330.0),
            range: range(0.0, 1000.0),
        };
        assert_eq!(q.problems(), Vec::<String>::new());
        q.kind = QuestionKind::Number {
            answer: Some(330.0),
            range: range(0.0, 100.0),
        };
        assert_eq!(
            q.problems(),
            vec!["The correct number must be within the range."]
        );
        q.kind = QuestionKind::Number {
            answer: Some(330.0),
            range: range(10.0, 10.0),
        };
        assert_eq!(
            q.problems(),
            vec!["The range must go from a smaller to a larger number."]
        );
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
