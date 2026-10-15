//! Searching by words, ignoring case and accents.

/// Lower case without accents, for searching and sorting by name.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// The words of `search`, folded, to look for with [`contains_all`].
#[must_use]
pub fn words(search: &str) -> Vec<String> {
    search.split_whitespace().map(fold).collect()
}

/// Whether `text` contains every one of `words` (from [`words`]).
#[must_use]
pub fn contains_all(text: &str, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let text = fold(text);
    words.iter().all(|w| text.contains(w.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ignores_case_accents_and_word_order() {
        let found = words("cafe PARIS");
        assert!(contains_all("Le Café de Paris", &found));
        assert!(!contains_all("Café de Flore", &found));
        assert!(contains_all("anything", &words("  ")));
    }
}
