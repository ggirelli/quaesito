//! What some kinds of question add to a question: a bonus question, picture reveals and pins,
//! parts, pairs and number answers.

use serde::{Deserialize, Serialize};

use super::question::{MAX_ANSWERS, MIN_ANSWERS};

#[cfg(doc)]
use super::QuestionKind;

/// A bonus question: teams type its answer together with the main answer, and the judge can
/// amend the verdict as for typed answers. A right answer adds a share of the question's points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
pub(super) fn deserialize_image_reveal<'de, D: serde::Deserializer<'de>>(
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Point {
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0, max = 1.0)))]
    pub x: f64,
    #[cfg_attr(feature = "schema", schemars(range(min = 0.0, max = 1.0)))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
pub(super) fn pair_problems(pairs: &[Pair]) -> Vec<String> {
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

/// What is missing from a number question: its answer, a sensible range around it, and sensible
/// distances to score by.
pub(super) fn number_problems(
    answer: Option<f64>,
    range: Option<NumberRange>,
    scoring: NumberScoring,
) -> Vec<String> {
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
    problems.extend(scoring.problem().map(str::to_owned));
    problems
}

/// The numbers a closest-number question accepts, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// How a number question is scored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum NumberScoring {
    /// The closest guesses win a set amount, however long they took; the others nothing.
    #[default]
    Closest,
    /// Full points up to `full_within` from the correct number, nothing beyond `none_beyond`,
    /// and a share falling evenly in between.
    Distance { full_within: f64, none_beyond: f64 },
    /// Full points up to `margin` from the correct number (0: only the exact number), nothing
    /// beyond.
    Margin { margin: f64 },
}

impl NumberScoring {
    #[must_use]
    pub fn is_closest(&self) -> bool {
        *self == Self::Closest
    }

    /// What is wrong with the scoring's distances, if anything.
    #[must_use]
    pub fn problem(self) -> Option<&'static str> {
        match self {
            Self::Closest => None,
            Self::Distance {
                full_within,
                none_beyond,
            } => (!(full_within.is_finite()
                && none_beyond.is_finite()
                && 0.0 <= full_within
                && full_within < none_beyond))
                .then_some(
                    "The distance for full points must be smaller than the one for no points.",
                ),
            Self::Margin { margin } => (!(margin.is_finite() && margin >= 0.0))
                .then_some("The margin must be zero or a positive number."),
        }
    }
}

/// Fewest parts a several-answers question has.
pub const MIN_PARTS: usize = 2;

/// Most parts a several-answers question has.
pub const MAX_PARTS: usize = 8;
