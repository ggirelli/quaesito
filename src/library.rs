//! The question library: ready-made questions to add to a quiz, or to build a random quiz from.
//!
//! The server bundles the entries (`library/library.json`, built from `library/checked`); hosts
//! browse and pick them in the browser.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::{
    model::{Question, QuestionKind},
    search,
};

/// One question of the library, with what it is about and how hard it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Set by the server from the bundled question, to name it in edits.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Whether this server's admin edited the question.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub edited: bool,
    pub category: String,
    pub topic: String,
    /// 1: most adults know it, 2: keen quizzers, 3: experts.
    pub difficulty: u8,
    pub question: Question,
}

/// What an admin may change in a library question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryEdit {
    pub difficulty: u8,
    pub question: Question,
}

/// Each difficulty and its name.
pub const DIFFICULTIES: [(u8, &str); 3] = [(1, "Easy"), (2, "Medium"), (3, "Hard")];

/// The answer types library questions use, named as in the editor.
pub const KINDS: [&str; 7] = [
    "Choices",
    "True/false",
    "Typed answer",
    "Number",
    "Order",
    "Several answers",
    "Pairs",
];

impl LibraryEntry {
    /// "Easy", "Medium" or "Hard".
    #[must_use]
    pub fn difficulty_name(&self) -> &'static str {
        DIFFICULTIES
            .iter()
            .find(|(level, _)| *level == self.difficulty)
            .map_or("", |(_, name)| name)
    }

    /// The answer type, as named in [`KINDS`].
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.question.kind {
            QuestionKind::TrueFalse { .. } => KINDS[1],
            QuestionKind::Text { .. } => KINDS[2],
            QuestionKind::Number { .. } => KINDS[3],
            QuestionKind::Order { .. } => KINDS[4],
            QuestionKind::Parts { .. } => KINDS[5],
            QuestionKind::Pairs { .. } => KINDS[6],
            QuestionKind::Choice | QuestionKind::Pin { .. } | QuestionKind::Slide { .. } => {
                KINDS[0]
            }
        }
    }

    /// The question as added to a quiz: it keeps the category and topic it comes from.
    #[must_use]
    pub fn to_question(&self) -> Question {
        Question {
            category: self.category.clone(),
            topic: self.topic.clone(),
            ..self.question.clone()
        }
    }

    /// The right answer in one line, to judge a question at a glance.
    #[must_use]
    pub fn answer(&self) -> String {
        let question = &self.question;
        let first = |accepted: &[String]| accepted.first().cloned().unwrap_or_default();
        match &question.kind {
            QuestionKind::TrueFalse { answer } => (if *answer { "True" } else { "False" }).into(),
            QuestionKind::Text { accepted } => first(accepted),
            QuestionKind::Number { answer, .. } => {
                answer.map(|n| n.to_string()).unwrap_or_default()
            }
            QuestionKind::Order { items } => items.join(" → "),
            QuestionKind::Parts { parts } => parts
                .iter()
                .map(|p| format!("{}: {}", p.label, first(&p.accepted)))
                .collect::<Vec<_>>()
                .join(" · "),
            QuestionKind::Pairs { pairs } => pairs
                .iter()
                .map(|p| format!("{} – {}", p.left.text, p.right.text))
                .collect::<Vec<_>>()
                .join(" · "),
            QuestionKind::Choice | QuestionKind::Pin { .. } | QuestionKind::Slide { .. } => {
                question
                    .answers
                    .iter()
                    .filter(|a| a.correct)
                    .map(|a| a.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            }
        }
    }

    /// A new GitHub issue reporting this question as wrong, filled in with the question as shown.
    #[must_use]
    pub fn report_url(&self) -> String {
        let question = &self.question;
        let mut details = format!(
            "{}\n\n{} · {} · {} · {}",
            question.text,
            self.category,
            self.topic,
            self.difficulty_name(),
            self.kind_name()
        );
        if !question.passage.is_empty() {
            let _ = write!(details, "\n\nPassage: {}", question.passage);
        }
        if matches!(question.kind, QuestionKind::Choice) {
            for answer in &question.answers {
                let mark = if answer.correct { '✓' } else { '✗' };
                let _ = write!(details, "\n- {mark} {}", answer.text);
            }
        } else {
            let _ = write!(details, "\n\nAnswer: {}", self.answer());
        }
        if self.edited {
            details.push_str("\n\n(Edited on this server.)");
        }
        if !self.id.is_empty() {
            let _ = write!(details, "\n\nId: {}", self.id);
        }
        format!(
            "{}/issues/new?template=wrong_question.yml&title={}&question={}",
            env!("CARGO_PKG_REPOSITORY"),
            encode_query(&format!("Wrong question: {}", question.text)),
            encode_query(&details)
        )
    }
}

/// Percent-encodes `text` for a URL's query string.
fn encode_query(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// What the host is looking for; empty fields match everything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibraryFilter {
    /// Words to find in the question, its answer, topic or category.
    pub search: String,
    pub category: Option<String>,
    /// A topic of the category.
    pub topic: Option<String>,
    pub difficulty: Option<u8>,
    /// One of [`KINDS`].
    pub kind: Option<&'static str>,
}

impl LibraryFilter {
    /// The entries that match, in library order.
    #[must_use]
    pub fn apply<'a>(&self, entries: &'a [LibraryEntry]) -> Vec<&'a LibraryEntry> {
        let words = search::words(&self.search);
        entries
            .iter()
            .filter(|e| self.category.as_ref().is_none_or(|c| *c == e.category))
            .filter(|e| self.topic.as_ref().is_none_or(|t| *t == e.topic))
            .filter(|e| self.difficulty.is_none_or(|d| d == e.difficulty))
            .filter(|e| self.kind.is_none_or(|k| k == e.kind_name()))
            .filter(|e| {
                words.is_empty()
                    || search::contains_all(
                        &format!(
                            "{} {} {} {}",
                            e.question.text,
                            e.answer(),
                            e.topic,
                            e.category
                        ),
                        &words,
                    )
            })
            .collect()
    }
}

/// How the questions shown are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibrarySort {
    /// The library's own order: by category, then topic.
    #[default]
    Category,
    /// Easy first.
    Difficulty,
    /// By answer type, in the order of [`KINDS`].
    Kind,
    /// By question text, ignoring case and accents.
    Text,
}

impl LibrarySort {
    pub const ALL: [Self; 4] = [Self::Category, Self::Difficulty, Self::Kind, Self::Text];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Category => "Category",
            Self::Difficulty => "Difficulty",
            Self::Kind => "Question type",
            Self::Text => "A–Z",
        }
    }

    /// Orders `entries`; ties keep the library's order.
    pub fn apply(self, entries: &mut [&LibraryEntry]) {
        match self {
            Self::Category => {}
            Self::Difficulty => entries.sort_by_key(|e| e.difficulty),
            Self::Kind => {
                entries.sort_by_key(|e| KINDS.iter().position(|k| *k == e.kind_name()));
            }
            Self::Text => entries.sort_by_cached_key(|e| search::fold(&e.question.text)),
        }
    }
}

/// Every category with its number of questions, by name.
#[must_use]
pub fn categories(entries: &[LibraryEntry]) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = Vec::new();
    for entry in entries {
        match found.iter_mut().find(|(name, _)| *name == entry.category) {
            Some((_, count)) => *count += 1,
            None => found.push((entry.category.clone(), 1)),
        }
    }
    found.sort();
    found
}

/// How a category shows: an icon (one of [`CATEGORY_ICONS`]) in a colour (a CSS hex colour).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryLook {
    pub icon: String,
    pub color: String,
}

impl CategoryLook {
    fn new(icon: &str, color: &str) -> Self {
        Self {
            icon: icon.to_owned(),
            color: color.to_owned(),
        }
    }

    /// The look of a category with none: a question mark in grey.
    #[must_use]
    pub fn unknown() -> Self {
        Self::new("help", "#94a3b8")
    }

    /// The look of one of the library's own categories.
    #[must_use]
    pub fn of_library(category: &str) -> Option<Self> {
        LIBRARY_LOOKS
            .iter()
            .find(|(name, ..)| *name == category)
            .map(|(_, icon, color)| Self::new(icon, color))
    }
}

/// The library's categories, each with its icon and colour.
const LIBRARY_LOOKS: [(&str, &str, &str); 14] = [
    ("Art & Design", "pen-tool", "#ef4444"),
    ("Film & TV", "film", "#a855f7"),
    ("Food & Drink", "coffee", "#f97316"),
    ("Geography", "globe", "#0d9488"),
    ("History", "clock", "#a16207"),
    ("Language & Words", "type", "#84cc16"),
    ("Literature", "book", "#6366f1"),
    ("Music", "music", "#ec4899"),
    ("Nature", "leaf", "#22c55e"),
    ("Pop Culture", "star", "#eab308"),
    ("Science", "flask", "#06b6d4"),
    ("Society & Culture", "users", "#d946ef"),
    ("Sport", "trophy", "#2563eb"),
    ("Technology", "cpu", "#64748b"),
];

/// The icons a category can have: the library's, then more for new categories.
pub const CATEGORY_ICONS: [&str; 34] = [
    "pen-tool",
    "film",
    "coffee",
    "globe",
    "clock",
    "type",
    "book",
    "music",
    "leaf",
    "star",
    "flask",
    "users",
    "trophy",
    "cpu",
    "heart",
    "camera",
    "compass",
    "anchor",
    "gift",
    "map",
    "zap",
    "smile",
    "award",
    "briefcase",
    "headphones",
    "feather",
    "tv",
    "truck",
    "shopping-bag",
    "umbrella",
    "dollar-sign",
    "activity",
    "grid",
    "hash",
];

/// The colours a category can have: the library's, then more for new categories.
pub const CATEGORY_COLORS: [&str; 26] = [
    "#ef4444", "#a855f7", "#f97316", "#0d9488", "#a16207", "#84cc16", "#6366f1", "#ec4899",
    "#22c55e", "#eab308", "#06b6d4", "#d946ef", "#2563eb", "#64748b", "#f43f5e", "#0ea5e9",
    "#7c3aed", "#10b981", "#f59e0b", "#be185d", "#0f766e", "#65a30d", "#c2410c", "#4f46e5",
    "#b45309", "#78716c",
];

/// A category questions can be filed under: one of the library's, or one this server added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryCategory {
    pub name: String,
    /// Its number of library questions.
    pub count: usize,
    pub topics: Vec<LibraryTopic>,
    pub look: CategoryLook,
}

/// The look of `category` among `categories`, else of the library's, else the unknown one.
#[must_use]
pub fn look_of(category: &str, categories: &[LibraryCategory]) -> CategoryLook {
    categories
        .iter()
        .find(|c| c.name == category)
        .map(|c| c.look.clone())
        .or_else(|| CategoryLook::of_library(category))
        .unwrap_or_else(CategoryLook::unknown)
}

/// A topic of a [`LibraryCategory`]; one this server added has no questions yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryTopic {
    pub name: String,
    /// Its number of library questions.
    pub count: usize,
}

/// A topic this server added to the library, possibly in a new category.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedTopic {
    pub category: String,
    pub topic: String,
}

/// A category this server added, with its look.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedCategory {
    pub name: String,
    #[serde(flatten)]
    pub look: CategoryLook,
}

/// The categories and topics this server added to the library.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Added {
    #[serde(default)]
    pub categories: Vec<AddedCategory>,
    #[serde(default)]
    pub topics: Vec<AddedTopic>,
}

/// Every category with its topics and look, by name: those of `entries`, and the `added` ones.
#[must_use]
pub fn subjects(entries: &[LibraryEntry], added: &Added) -> Vec<LibraryCategory> {
    let mut found: Vec<LibraryCategory> = Vec::new();
    let pairs = entries
        .iter()
        .map(|e| (&e.category, &e.topic, 1))
        .chain(added.topics.iter().map(|a| (&a.category, &a.topic, 0)));
    for (category, topic, count) in pairs {
        let index = found
            .iter()
            .position(|c| c.name == *category)
            .unwrap_or_else(|| {
                let look = CategoryLook::of_library(category)
                    .or_else(|| {
                        let added = added.categories.iter().find(|c| c.name == *category);
                        added.map(|c| c.look.clone())
                    })
                    .unwrap_or_else(CategoryLook::unknown);
                found.push(LibraryCategory {
                    name: category.clone(),
                    count: 0,
                    topics: Vec::new(),
                    look,
                });
                found.len() - 1
            });
        let category = &mut found[index];
        category.count += count;
        match category.topics.iter_mut().find(|t| t.name == *topic) {
            Some(found) => found.count += count,
            None => category.topics.push(LibraryTopic {
                name: topic.clone(),
                count,
            }),
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    for category in &mut found {
        category.topics.sort_by(|a, b| a.name.cmp(&b.name));
    }
    found
}

/// Up to `count` different items of `pool`, picked at random and in random order.
/// `random(n)` returns a number below `n`.
pub fn sample<T: Clone>(
    pool: &[T],
    count: usize,
    mut random: impl FnMut(usize) -> usize,
) -> Vec<T> {
    let mut indices: Vec<usize> = (0..pool.len()).collect();
    let count = count.min(pool.len());
    // The first `count` steps of a Fisher–Yates shuffle.
    for i in 0..count {
        let j = i + random(indices.len() - i);
        indices.swap(i, j);
    }
    indices[..count].iter().map(|&i| pool[i].clone()).collect()
}

/// The questions of a random quiz: for each `(category, count)`, that many questions of the
/// category that match `filter` (its category is ignored), grouped by category in the order
/// given. With `round_slides`, a slide titled with its name opens each category.
pub fn random_quiz(
    entries: &[LibraryEntry],
    counts: &[(String, usize)],
    filter: &LibraryFilter,
    round_slides: bool,
    mut random: impl FnMut(usize) -> usize,
) -> Vec<Question> {
    let mut questions = Vec::new();
    for (category, count) in counts.iter().filter(|(_, count)| *count > 0) {
        let pool = LibraryFilter {
            category: Some(category.clone()),
            ..filter.clone()
        }
        .apply(entries);
        if round_slides && !pool.is_empty() {
            questions.push(Question {
                kind: QuestionKind::Slide {
                    title: category.clone(),
                    results: false,
                },
                ..Question::slide()
            });
        }
        questions.extend(
            sample(&pool, *count, &mut random)
                .into_iter()
                .map(LibraryEntry::to_question),
        );
    }
    questions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Answer;

    fn entry(category: &str, text: &str, difficulty: u8, kind: QuestionKind) -> LibraryEntry {
        LibraryEntry {
            id: String::new(),
            edited: false,
            category: category.into(),
            topic: "Topic".into(),
            difficulty,
            question: Question {
                text: text.into(),
                kind,
                answers: vec![
                    Answer {
                        text: "Paris".into(),
                        correct: true,
                        ..Answer::default()
                    },
                    Answer {
                        text: "Lyon".into(),
                        ..Answer::default()
                    },
                ],
                ..Question::default()
            },
        }
    }

    fn library() -> Vec<LibraryEntry> {
        vec![
            entry("Geography", "Capital of France?", 1, QuestionKind::Choice),
            entry(
                "Geography",
                "Rome is in Italy.",
                2,
                QuestionKind::TrueFalse { answer: true },
            ),
            entry(
                "History",
                "Year of the Moon landing?",
                2,
                QuestionKind::Number {
                    answer: Some(1969.0),
                    range: None,
                },
            ),
        ]
    }

    #[test]
    fn filters_combine_and_search_the_answer_too() {
        let all = library();
        let texts = |filter: LibraryFilter| -> Vec<String> {
            filter
                .apply(&all)
                .iter()
                .map(|e| e.question.text.clone())
                .collect()
        };
        assert_eq!(texts(LibraryFilter::default()).len(), 3);
        assert_eq!(
            texts(LibraryFilter {
                category: Some("Geography".into()),
                difficulty: Some(2),
                ..LibraryFilter::default()
            }),
            ["Rome is in Italy."]
        );
        assert_eq!(
            texts(LibraryFilter {
                kind: Some("Number"),
                ..LibraryFilter::default()
            }),
            ["Year of the Moon landing?"]
        );
        assert_eq!(
            texts(LibraryFilter {
                search: "paris".into(),
                ..LibraryFilter::default()
            }),
            ["Capital of France?"]
        );
    }

    #[test]
    fn sorting_orders_what_is_shown_and_keeps_ties_in_place() {
        let all = library();
        // Shown in reverse: "Year…" (2, Number), "Rome…" (2, True/false), "Capital…" (1, Choices).
        let texts = |sort: LibrarySort| -> Vec<char> {
            let mut shown: Vec<&LibraryEntry> = all.iter().rev().collect();
            sort.apply(&mut shown);
            shown
                .iter()
                .map(|e| e.question.text.chars().next().unwrap())
                .collect()
        };
        assert_eq!(texts(LibrarySort::Category), ['Y', 'R', 'C']);
        assert_eq!(texts(LibrarySort::Difficulty), ['C', 'Y', 'R']);
        assert_eq!(texts(LibrarySort::Kind), ['C', 'R', 'Y']);
        assert_eq!(texts(LibrarySort::Text), ['C', 'R', 'Y']);
    }

    #[test]
    fn reports_open_a_filled_in_issue() {
        let all = library();
        let url = all[0].report_url();
        assert!(url.starts_with(
            "https://github.com/ggirelli/quaesito/issues/new?template=wrong_question.yml&title=Wrong%20question%3A%20"
        ));
        assert!(url.contains("&question="));
        assert!(url.contains("%E2%9C%93%20Paris"));
        assert!(!url[url.find('?').unwrap()..].contains([' ', '\n', '#']));
        assert_eq!(encode_query("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }

    #[test]
    fn answers_read_in_one_line() {
        let all = library();
        assert_eq!(all[0].answer(), "Paris");
        assert_eq!(all[1].answer(), "True");
        assert_eq!(all[2].answer(), "1969");
        assert_eq!(all[2].kind_name(), "Number");
        assert_eq!(all[2].difficulty_name(), "Medium");
    }

    #[test]
    fn categories_are_counted_by_name_and_topics_filter() {
        let mut all = library();
        all[1].topic = "Europe".into();
        assert_eq!(
            categories(&all),
            [("Geography".to_owned(), 2), ("History".to_owned(), 1)]
        );
        let europe = LibraryFilter {
            category: Some("Geography".into()),
            topic: Some("Europe".into()),
            ..LibraryFilter::default()
        };
        assert_eq!(europe.apply(&all), [&all[1]]);
    }

    #[test]
    fn subjects_add_the_added_topics_without_questions() {
        let mut all = library();
        all[1].topic = "Europe".into();
        let added = |category: &str, topic: &str| AddedTopic {
            category: category.into(),
            topic: topic.into(),
        };
        let found = subjects(
            &all,
            &Added {
                categories: vec![AddedCategory {
                    name: "Board games".into(),
                    look: CategoryLook::new("grid", "#f43f5e"),
                }],
                topics: vec![added("Geography", "Rivers"), added("Board games", "Chess")],
            },
        );
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Board games", "Geography", "History"]);
        assert_eq!((found[0].count, found[0].topics[0].count), (0, 0));
        let geography: Vec<(&str, usize)> = found[1]
            .topics
            .iter()
            .map(|t| (t.name.as_str(), t.count))
            .collect();
        assert_eq!(geography, [("Europe", 1), ("Rivers", 0), ("Topic", 1)]);
        assert_eq!(found[1].count, 2);
        assert_eq!(found[0].look, CategoryLook::new("grid", "#f43f5e"));
        assert_eq!(found[1].look, CategoryLook::new("globe", "#0d9488"));
        assert_eq!(look_of("Board games", &found).icon, "grid");
        assert_eq!(look_of("Sport", &[]).icon, "trophy");
        assert_eq!(look_of("Nope", &found), CategoryLook::unknown());
    }

    #[test]
    fn the_library_looks_are_offered_and_all_different() {
        for (name, icon, color) in LIBRARY_LOOKS {
            assert!(CATEGORY_ICONS.contains(&icon), "{name}");
            assert!(CATEGORY_COLORS.contains(&color), "{name}");
        }
        let mut icons = CATEGORY_ICONS.to_vec();
        icons.sort_unstable();
        icons.dedup();
        let mut colors = CATEGORY_COLORS.to_vec();
        colors.sort_unstable();
        colors.dedup();
        assert_eq!(
            (icons.len(), colors.len()),
            (CATEGORY_ICONS.len(), CATEGORY_COLORS.len())
        );
    }

    #[test]
    fn samples_never_repeat_and_stop_at_the_pool() {
        let pool: Vec<u32> = (0..10).collect();
        let mut seed = 7usize;
        let mut random = |n: usize| {
            seed = seed.wrapping_mul(31).wrapping_add(17);
            seed % n
        };
        let mut picked = sample(&pool, 6, &mut random);
        assert_eq!(picked.len(), 6);
        picked.sort_unstable();
        picked.dedup();
        assert_eq!(picked.len(), 6);
        assert_eq!(sample(&pool, 50, &mut random).len(), 10);
    }

    #[test]
    fn random_quizzes_take_each_category_in_turn() {
        let questions = random_quiz(
            &library(),
            &[("History".into(), 5), ("Geography".into(), 1)],
            &LibraryFilter::default(),
            true,
            |_| 0,
        );
        let texts: Vec<&str> = questions.iter().map(|q| q.text.as_str()).collect();
        assert_eq!(texts[1], "Year of the Moon landing?");
        assert_eq!(texts[3], "Capital of France?");
        assert!(
            matches!(&questions[0].kind, QuestionKind::Slide { title, .. } if title == "History")
        );
        assert!(
            matches!(&questions[2].kind, QuestionKind::Slide { title, .. } if title == "Geography")
        );
        assert_eq!(questions.len(), 4);
        assert_eq!(
            (questions[3].category.as_str(), questions[3].topic.as_str()),
            ("Geography", "Topic")
        );
    }
}
