//! The side list of questions and slides, which follows the scroll and moves items.

use dioxus::prelude::*;

use crate::model::Quiz;

/// The URL fragment of the card labelled `label`: "Question 3" is at `#question-3`.
pub(super) fn anchor(label: &str) -> String {
    label.to_lowercase().replace(' ', "-")
}

/// A question or slide as the navigator shows it.
#[derive(Clone, PartialEq)]
pub(super) struct NavItem {
    anchor: String,
    /// The question's number, or the slide's: "12", "S2".
    number: String,
    /// The start of its text, shown beside the number where there is room.
    text: String,
    /// The label and the start of the text, on hover.
    title: String,
    slide: bool,
    /// Something keeps it from being played.
    flagged: bool,
}

impl NavItem {
    pub(super) fn new(quiz: &Quiz, index: usize, flagged: bool) -> Self {
        const TEXT_CHARS: usize = 80;
        let question = &quiz.questions[index];
        let label = quiz.label(index);
        let number = quiz.ordinal(index) + 1;
        let full = question.text.trim();
        let text = match full.char_indices().nth(TEXT_CHARS) {
            Some((cut, _)) => format!("{}…", &full[..cut]),
            None => full.to_owned(),
        };
        Self {
            anchor: anchor(&label),
            number: if question.is_slide() {
                format!("S{number}")
            } else {
                number.to_string()
            },
            title: if text.is_empty() {
                label
            } else {
                format!("{label}: {text}")
            },
            text,
            slide: question.is_slide(),
            flagged,
        }
    }
}

/// Every question and slide as a link, in a strip that always fits: thin bars, widened around
/// the cards on screen and around the pointer. A click (or a finger lifted after sliding along
/// it) scrolls there. Kept but `hidden` in the table view, where the rows fit on a few screens.
#[component]
pub(super) fn QuizNavigator(items: Vec<NavItem>, hidden: bool) -> Element {
    rsx! {
        nav { class: "editor-nav", aria_label: "Questions and slides", hidden,
            for item in items {
                a {
                    key: "{item.anchor}",
                    href: "#{item.anchor}",
                    class: match (item.slide, item.flagged) {
                        (true, true) => "nav-slide flagged",
                        (true, false) => "nav-slide",
                        (false, true) => "flagged",
                        (false, false) => "",
                    },
                    title: "{item.title}",
                    onclick: move |e| {
                        e.prevent_default();
                        document::eval(&format!("window.qnav?.jump({:?})", item.anchor));
                    },
                    span { class: "nav-number", "{item.number}" }
                    span { class: "nav-text", "{item.text}" }
                }
            }
        }
    }
}

/// Sizes the navigator's links (open around the cards on screen, or around the pointer or a
/// finger sliding along it, thin elsewhere), lights the ones on screen, keeps the card at the top
/// of the list in the URL fragment (without adding history entries), and on load scrolls back to
/// the card the fragment names, following it while pictures above it load, until the user scrolls.
pub(super) const NAVIGATOR_JS: &str = include_str!("navigator.js");

/// Keeps a moved question or slide in view: once the cards are redrawn, scrolls so the card at `to`
/// sits where the one at `from` was on screen, and focuses the same move button on it (or its
/// first enabled one, when that button is now disabled at the top or bottom).
pub(super) fn follow_move(from: usize, to: usize, button: &str) {
    document::eval(&format!(
        r#"const cards = () => document.querySelectorAll(".editor-list > .question");
        const top = cards()[{from}]?.getBoundingClientRect().top ?? 0;
        requestAnimationFrame(() => requestAnimationFrame(() => {{
            const card = cards()[{to}];
            if (!card) return;
            window.scrollBy({{ top: card.getBoundingClientRect().top - top, behavior: "instant" }});
            const buttons = [...card.querySelectorAll(".actions button")].filter((b) => !b.disabled);
            (buttons.find((b) => b.title === "{button}") ?? buttons[0])?.focus({{ preventScroll: true }});
        }}));"#
    ));
}
