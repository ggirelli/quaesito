//! Comparing typed answers.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Folds a typed answer for comparison: lower case, no accents, punctuation and runs of spaces
/// turned into single spaces, trimmed. "  Léonard   de Vinci! " becomes "leonard de vinci".
#[must_use]
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for c in text
        .nfkd()
        .filter(|&c| !is_combining_mark(c))
        .flat_map(char::to_lowercase)
    {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn ignores_case_accents_punctuation_and_spacing() {
        assert_eq!(normalize("  Léonard   de Vinci! "), "leonard de vinci");
        assert_eq!(normalize("Leonard-de-Vinci"), "leonard de vinci");
        assert_eq!(normalize("ÉCOLE"), "ecole");
        assert_eq!(normalize("Straße"), "straße");
        assert_eq!(normalize("?!"), "");
    }

    #[test]
    fn spelling_still_matters() {
        assert_ne!(
            normalize("Leonardo da Vinchi"),
            normalize("Leonardo da Vinci")
        );
    }
}
