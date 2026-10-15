//! The question library: ready-made questions to add to a quiz, or to build a random quiz from.
//!
//! The server bundles the entries (`library/library.json`, built from `library/checked`); hosts
//! browse and pick them in the browser.

use serde::{Deserialize, Serialize};

use crate::{
    model::{Question, QuestionKind},
    search,
};

/// One question of the library, with what it is about and how hard it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub category: String,
    pub topic: String,
    /// 1: most adults know it, 2: keen quizzers, 3: experts.
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

/// The topics of `category` with their number of questions, by name.
#[must_use]
pub fn topics(entries: &[LibraryEntry], category: &str) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = Vec::new();
    for entry in entries.iter().filter(|e| e.category == category) {
        match found.iter_mut().find(|(name, _)| *name == entry.topic) {
            Some((_, count)) => *count += 1,
            None => found.push((entry.topic.clone(), 1)),
        }
    }
    found.sort();
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
                .map(|e| e.question.clone()),
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
    fn answers_read_in_one_line() {
        let all = library();
        assert_eq!(all[0].answer(), "Paris");
        assert_eq!(all[1].answer(), "True");
        assert_eq!(all[2].answer(), "1969");
        assert_eq!(all[2].kind_name(), "Number");
        assert_eq!(all[2].difficulty_name(), "Medium");
    }

    #[test]
    fn categories_and_their_topics_are_counted_by_name() {
        let mut all = library();
        all[1].topic = "Europe".into();
        assert_eq!(
            categories(&all),
            [("Geography".to_owned(), 2), ("History".to_owned(), 1)]
        );
        assert_eq!(
            topics(&all, "Geography"),
            [("Europe".to_owned(), 1), ("Topic".to_owned(), 1)]
        );
        let europe = LibraryFilter {
            category: Some("Geography".into()),
            topic: Some("Europe".into()),
            ..LibraryFilter::default()
        };
        assert_eq!(europe.apply(&all), [&all[1]]);
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
    }
}
