//! The bundled question library (see [`crate::library`]).

use std::sync::OnceLock;

use crate::library::LibraryEntry;

/// Every checked library question, combined by `tools/library/clean.py`.
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
    use std::path::Path;

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

    /// `library/library.json` holds exactly the entries of `library/checked`, as
    /// `tools/library/clean.py` writes them.
    #[test]
    fn the_bundled_library_matches_the_checked_files() {
        fn files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    files(&path, found);
                } else if path.extension().is_some_and(|e| e == "json") {
                    found.push(path);
                }
            }
        }
        let checked = Path::new(env!("CARGO_MANIFEST_DIR")).join("library/checked");
        let mut paths = Vec::new();
        files(&checked, &mut paths);
        paths.sort();
        let mut expected: Vec<serde_json::Value> = Vec::new();
        for path in paths {
            let batch: Vec<serde_json::Value> =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            expected.extend(batch);
        }
        let bundled: Vec<serde_json::Value> = serde_json::from_str(LIBRARY).unwrap();
        assert!(
            bundled == expected,
            "library/library.json is stale: run `uv run tools/library/clean.py`"
        );
    }
}
