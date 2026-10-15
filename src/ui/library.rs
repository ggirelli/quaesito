//! The question library in the admin pages: browsing it from the editor, and picking the
//! categories of a random quiz.

use std::collections::HashSet;

use dioxus::prelude::*;

use super::{
    admin::AdminToken,
    client,
    widgets::{Dialog, ErrorBox, Icon, IconKind, Spinner, message},
};
use crate::{
    api::question_library,
    library::{DIFFICULTIES, KINDS, LibraryEntry, LibraryFilter},
    model::Question,
};

/// Questions shown at first, and added by each "Show more".
const PAGE: usize = 40;
/// Questions a category gets when ticked for a random quiz.
pub const DEFAULT_PER_CATEGORY: usize = 5;

/// Fetches the library once `wanted` is true.
pub fn use_library(
    wanted: impl Fn() -> bool + 'static,
) -> Resource<Option<Result<Vec<LibraryEntry>, ServerFnError>>> {
    let token: AdminToken = use_context();
    use_resource(move || {
        let wanted = wanted();
        async move {
            if wanted {
                Some(question_library(token.get()).await)
            } else {
                None
            }
        }
    })
}

/// The difficulty select's value for `level`.
fn difficulty_key(level: Option<u8>) -> String {
    level.map(|l| l.to_string()).unwrap_or_default()
}

/// A select of the difficulties, with "Any difficulty" first.
#[component]
fn DifficultySelect(value: Option<u8>, on_change: EventHandler<Option<u8>>) -> Element {
    rsx! {
        select {
            aria_label: "Difficulty",
            value: difficulty_key(value),
            onchange: move |e| on_change.call(e.value().parse().ok()),
            option { value: "", "Any difficulty" }
            for (level, name) in DIFFICULTIES {
                option { value: difficulty_key(Some(level)), "{name}" }
            }
        }
    }
}

/// Browses the library from the editor and adds questions to the quiz: picked one by one, or a
/// number of them at random among those shown.
#[component]
pub fn LibraryDialog(
    /// The texts of the questions already in the quiz.
    present: Vec<String>,
    on_add: EventHandler<Vec<Question>>,
    on_close: EventHandler<()>,
) -> Element {
    let library = use_library(|| true);
    let mut search = use_signal(String::new);
    let mut category = use_signal(String::new);
    let mut difficulty = use_signal(|| None::<u8>);
    let mut kind = use_signal(String::new);
    let mut timeless = use_signal(|| false);
    let mut shown_count = use_signal(|| PAGE);
    let mut random_count = use_signal(|| 10usize);

    let loaded = library.read();
    let entries = match &*loaded {
        Some(Some(Ok(entries))) => entries,
        Some(Some(Err(err))) => {
            return rsx! {
                Dialog { title: "Question library", on_close,
                    ErrorBox { error: Some(message(err)) }
                }
            };
        }
        _ => {
            return rsx! {
                Dialog { title: "Question library", on_close, Spinner {} }
            };
        }
    };
    let present: HashSet<&str> = present.iter().map(String::as_str).collect();
    let filter = LibraryFilter {
        search: search(),
        category: Some(category()).filter(|c| !c.is_empty()),
        difficulty: difficulty(),
        kind: KINDS.into_iter().find(|k| *k == kind()),
        timeless: timeless(),
    };
    let matching = filter.apply(entries);
    let new: Vec<&LibraryEntry> = matching
        .iter()
        .copied()
        .filter(|e| !present.contains(e.question.text.as_str()))
        .collect();
    let picks = random_count().min(new.len());
    let add_random = {
        let questions: Vec<Question> = new.iter().map(|e| e.question.clone()).collect();
        move |_| {
            let picked = crate::library::sample(&questions, picks, client::random_below);
            on_add.call(picked);
        }
    };
    let categories = crate::library::categories(entries);

    rsx! {
        Dialog { title: "Question library", wide: true, on_close,
            div { class: "row quiz-filters",
                label { class: "search grow",
                    Icon { kind: IconKind::Search }
                    input {
                        r#type: "search",
                        placeholder: "Search questions and answers",
                        aria_label: "Search questions and answers",
                        value: search,
                        oninput: move |e| {
                            search.set(e.value());
                            shown_count.set(PAGE);
                        },
                    }
                }
                select {
                    aria_label: "Category",
                    value: category,
                    onchange: move |e| {
                        category.set(e.value());
                        shown_count.set(PAGE);
                    },
                    option { value: "", "All categories" }
                    for (name , count) in categories {
                        option { value: "{name}", "{name} ({count})" }
                    }
                }
                DifficultySelect {
                    value: difficulty(),
                    on_change: move |level| difficulty.set(level),
                }
                select {
                    aria_label: "Answer type",
                    value: kind,
                    onchange: move |e| kind.set(e.value()),
                    option { value: "", "Any answer type" }
                    for name in KINDS {
                        option { value: name, "{name}" }
                    }
                }
                label {
                    class: "check-field",
                    title: "Records, title holders… questions that say “As of <year>”",
                    input {
                        r#type: "checkbox",
                        checked: timeless(),
                        onchange: move |e| timeless.set(e.checked()),
                    }
                    span { "Leave out answers that may date" }
                }
            }
            div { class: "row between",
                span { class: "muted small",
                    if matching.len() == 1 {
                        "1 question matches"
                    } else {
                        "{matching.len()} questions match"
                    }
                    if matching.len() > new.len() {
                        ", {matching.len() - new.len()} already in the quiz"
                    }
                }
                div { class: "row",
                    input {
                        class: "short-number",
                        r#type: "number",
                        min: "1",
                        max: "{new.len().max(1)}",
                        aria_label: "How many questions to add at random",
                        value: "{random_count}",
                        oninput: move |e| random_count.set(e.value().parse().unwrap_or(0)),
                    }
                    button {
                        class: "button",
                        disabled: picks == 0,
                        title: "Add this many of the questions shown, picked at random",
                        onclick: add_random,
                        "🎲 Add at random"
                    }
                }
            }
            ul { class: "list library-list",
                for (index , entry) in matching.iter().take(shown_count()).enumerate() {
                    LibraryRow {
                        key: "{index}",
                        entry: (*entry).clone(),
                        in_quiz: present.contains(entry.question.text.as_str()),
                        on_add,
                    }
                }
            }
            if matching.len() > shown_count() {
                button {
                    class: "button ghost",
                    onclick: move |_| shown_count += PAGE,
                    "Show more"
                }
            }
        }
    }
}

/// One library question with its answer, and a button to add it.
#[component]
fn LibraryRow(entry: LibraryEntry, in_quiz: bool, on_add: EventHandler<Vec<Question>>) -> Element {
    let question = entry.question.clone();
    rsx! {
        li { class: "list-row",
            div { class: "grow stack-tight",
                span { class: "strong", "{entry.question.text}" }
                if !entry.question.passage.is_empty() {
                    span { class: "small library-passage", "{entry.question.passage}" }
                }
                span { class: "small library-answer", "✓ {entry.answer()}" }
                span { class: "muted small",
                    "{entry.category} · {entry.topic} · {entry.difficulty_name()} · {entry.kind_name()}"
                    if entry.time_sensitive {
                        " · may date"
                    }
                }
            }
            if in_quiz {
                span { class: "muted small", "✓ In the quiz" }
            } else {
                button {
                    class: "button",
                    onclick: move |_| on_add.call(vec![question.clone()]),
                    "＋ Add"
                }
            }
        }
    }
}

/// How many questions of each category a random quiz takes, and which ones.
#[derive(Clone, Default, PartialEq)]
pub struct RandomQuiz {
    /// Every category with its number of questions to pick (0 leaves it out).
    pub counts: Vec<(String, usize)>,
    pub difficulty: Option<u8>,
    pub timeless: bool,
    pub round_slides: bool,
}

impl RandomQuiz {
    /// The questions picked from `entries`.
    pub fn pick(&self, entries: &[LibraryEntry]) -> Vec<Question> {
        crate::library::random_quiz(
            entries,
            &self.counts,
            &LibraryFilter {
                difficulty: self.difficulty,
                timeless: self.timeless,
                ..LibraryFilter::default()
            },
            self.round_slides,
            client::random_below,
        )
    }
}

/// Ticks the categories of a random quiz and sets how many questions each gets.
#[component]
pub fn RandomQuizFields(
    entries: ReadSignal<Vec<LibraryEntry>>,
    choice: Signal<RandomQuiz>,
) -> Element {
    let mut choice = choice;
    let current = choice();
    let filter = LibraryFilter {
        difficulty: current.difficulty,
        timeless: current.timeless,
        ..LibraryFilter::default()
    };
    let entries = entries.read();
    let available: Vec<(String, usize)> = crate::library::categories(entries.as_slice())
        .into_iter()
        .map(|(name, _)| {
            let count = LibraryFilter {
                category: Some(name.clone()),
                ..filter.clone()
            }
            .apply(entries.as_slice())
            .len();
            (name, count)
        })
        .collect();
    let wanted = |name: &str| {
        current
            .counts
            .iter()
            .find(|(n, _)| n == name)
            .map_or(0, |(_, count)| *count)
    };
    let total: usize = available
        .iter()
        .map(|(name, count)| wanted(name).min(*count))
        .sum();
    let mut set_count = move |name: String, count: usize| {
        choice.with_mut(|c| match c.counts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, n)) => *n = count,
            None => c.counts.push((name, count)),
        });
    };
    rsx! {
        div { class: "stack",
            div { class: "row",
                DifficultySelect {
                    value: current.difficulty,
                    on_change: move |level| choice.with_mut(|c| c.difficulty = level),
                }
                label { class: "check-field",
                    input {
                        r#type: "checkbox",
                        checked: current.timeless,
                        onchange: move |e| choice.with_mut(|c| c.timeless = e.checked()),
                    }
                    span { "Leave out answers that may date" }
                }
            }
            ul { class: "list random-categories",
                for (name , count) in available {
                    li { key: "{name}", class: "list-row",
                        label { class: "check-field grow",
                            input {
                                r#type: "checkbox",
                                checked: wanted(&name) > 0,
                                disabled: count == 0,
                                onchange: {
                                    let name = name.clone();
                                    move |e: FormEvent| {
                                        set_count(
                                            name.clone(),
                                            if e.checked() { DEFAULT_PER_CATEGORY.min(count) } else { 0 },
                                        );
                                    }
                                },
                            }
                            span { "{name}" }
                            span { class: "muted small", "({count})" }
                        }
                        input {
                            class: "short-number",
                            r#type: "number",
                            min: "0",
                            max: "{count}",
                            aria_label: "Questions from {name}",
                            value: "{wanted(&name).min(count)}",
                            oninput: {
                                let name = name.clone();
                                move |e: FormEvent| {
                                    set_count(name.clone(), e.value().parse::<usize>().unwrap_or(0).min(count));
                                }
                            },
                        }
                    }
                }
            }
            label { class: "check-field",
                input {
                    r#type: "checkbox",
                    checked: current.round_slides,
                    onchange: move |e| choice.with_mut(|c| c.round_slides = e.checked()),
                }
                span { "Open each category with a title slide" }
            }
            p { class: "muted small",
                if total == 1 {
                    "1 question, picked at random. Review them in the editor before playing."
                } else {
                    "{total} questions, picked at random. Review them in the editor before playing."
                }
            }
        }
    }
}
