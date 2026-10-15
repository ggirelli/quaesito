//! The bundled question library (see [`crate::library`]).

use std::sync::OnceLock;

use crate::library::LibraryEntry;

/// Every library question (see `library/README.md`).
const LIBRARY: &str = include_str!("../../library/library.json");

/// The library's questions, read on first use.
///
/// # Panics
/// If the bundled file does not parse, which the tests rule out.
pub fn entries() -> &'static [LibraryEntry] {
    static ENTRIES: OnceLock<Vec<LibraryEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| serde_json::from_str(LIBRARY).expect("the bundled library is valid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{QuestionKind, Quiz};

    #[test]
    fn every_library_question_is_playable() {
        let entries = entries();
        assert!(entries.len() > 1000);
        let mut quiz = Quiz::new("library".into(), "Library".into());
        quiz.questions = entries.iter().map(|e| e.question.clone()).collect();
        assert_eq!(quiz.validate(), []);
        for entry in entries {
            let q = &entry.question;
            assert!((1..=3).contains(&entry.difficulty), "{}", q.text);
            assert!(
                !matches!(
                    q.kind,
                    QuestionKind::Pin { .. } | QuestionKind::Slide { .. }
                ),
                "{}",
                q.text
            );
            assert!(q.image.is_none() && q.audio.is_none(), "{}", q.text);
        }
    }
}
