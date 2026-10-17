//! The question library in the admin pages: browsing it on its own tab of the admin page or from
//! the editor, and picking the categories of a random quiz.

use std::collections::HashSet;

use dioxus::prelude::*;

use super::{
    admin::AdminToken,
    client,
    editor::LibraryQuestionEditor,
    widgets::{Dialog, ErrorBox, Icon, IconKind, Spinner, message},
};
use crate::{
    api::{add_library_topic, library_subjects, question_library, remove_library_topic},
    library::{
        CATEGORY_COLORS, CATEGORY_ICONS, CategoryLook, DIFFICULTIES, KINDS, LibraryCategory,
        LibraryEntry, LibraryFilter, LibrarySort, ORIGINS, look_of,
    },
    model::{Question, QuestionKind},
};

/// The library's categories with their looks, those added on this server included, once loaded:
/// what the big screen draws a question's category with.
#[derive(Clone, Copy)]
pub struct Categories(pub Resource<Vec<LibraryCategory>>);

impl Categories {
    /// Loads the categories.
    pub fn use_loaded() -> Self {
        let token: AdminToken = use_context();
        Self(use_resource(move || async move {
            library_subjects(token.get()).await.unwrap_or_default()
        }))
    }

    /// The icon and colour of `category`; the library's own until the categories load.
    pub fn look(self, category: &str) -> CategoryLook {
        look_of(category, self.0.read().as_deref().unwrap_or_default())
    }
}

/// A category's icon, in its colour.
#[component]
pub fn CategoryIcon(look: CategoryLook) -> Element {
    rsx! {
        span { class: "category-icon", style: "color: {look.color}",
            Icon { kind: IconKind::of_category(&look.icon) }
        }
    }
}

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

/// The editor's window onto the library, to add questions to the quiz.
#[component]
pub fn LibraryDialog(
    /// The texts of the questions already in the quiz.
    present: Vec<String>,
    on_add: EventHandler<Vec<Question>>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog { title: "Question library", class: "library-dialog", on_close,
            LibraryBrowser { present, on_add }
        }
    }
}

/// Browses the library, laid out like the editor: the categories on the left, each opening onto
/// its topics; the questions in the middle, under the search; the other filters on the right. With `on_add` (in the editor),
/// questions can be added to the quiz, picked one by one or a number of them at random among
/// those shown.
#[component]
pub fn LibraryBrowser(
    /// The texts of the questions already in the quiz.
    #[props(default)]
    present: Vec<String>,
    on_add: Option<EventHandler<Vec<Question>>>,
) -> Element {
    let token: AdminToken = use_context();
    let mut library = use_library(|| true);
    // The categories and topics, with those added on this server.
    let mut subjects = use_resource(move || async move { library_subjects(token.get()).await });
    let mut search = use_signal(String::new);
    let mut category = use_signal(|| None::<String>);
    let mut topic = use_signal(|| None::<String>);
    let mut difficulty = use_signal(|| None::<u8>);
    let mut kind = use_signal(String::new);
    let mut added = use_signal(|| None::<bool>);
    let mut sort = use_signal(LibrarySort::default);
    // Whether the pop-up adding a topic is open.
    let mut adding = use_signal(|| false);
    // The question being edited in a pop-up.
    let mut editing = use_signal(|| None::<LibraryEntry>);
    let mut shown_count = use_signal(|| PAGE);
    let mut random_count = use_signal(|| 10usize);

    let loaded = library.read();
    let entries = match &*loaded {
        Some(Some(Ok(entries))) => entries,
        Some(Some(Err(err))) => {
            return rsx! {
                ErrorBox { error: Some(message(err)) }
            };
        }
        _ => {
            return rsx! {
                Spinner {}
            };
        }
    };
    let present: HashSet<&str> = present.iter().map(String::as_str).collect();
    let filter = LibraryFilter {
        search: search(),
        category: category(),
        topic: topic(),
        difficulty: difficulty(),
        kind: KINDS.into_iter().find(|k| *k == kind()),
        added: added(),
    };
    let mut matching = filter.apply(entries);
    sort().apply(&mut matching);
    // Each list's counts follow the other filters: how many questions picking that option shows.
    let difficulties: Vec<FilterOption> = DIFFICULTIES
        .into_iter()
        .map(|(level, name)| FilterOption {
            key: level.to_string(),
            label: name.to_owned(),
            count: LibraryFilter {
                difficulty: Some(level),
                ..filter.clone()
            }
            .apply(entries)
            .len(),
        })
        .collect();
    let kinds: Vec<FilterOption> = KINDS
        .into_iter()
        .map(|name| FilterOption {
            key: name.to_owned(),
            label: name.to_owned(),
            count: LibraryFilter {
                kind: Some(name),
                ..filter.clone()
            }
            .apply(entries)
            .len(),
        })
        .collect();
    let origins: Vec<FilterOption> = ORIGINS
        .into_iter()
        .map(|(origin, name)| FilterOption {
            key: origin.to_string(),
            label: name.to_owned(),
            count: LibraryFilter {
                added: Some(origin),
                ..filter.clone()
            }
            .apply(entries)
            .len(),
        })
        .collect();
    let new: Vec<&LibraryEntry> = matching
        .iter()
        .copied()
        .filter(|e| !present.contains(e.question.text.as_str()))
        .collect();
    let picks = random_count().min(new.len());
    let add_random = {
        let questions: Vec<Question> = new.iter().map(|e| e.to_question()).collect();
        move |_| {
            if let Some(on_add) = on_add {
                on_add.call(crate::library::sample(
                    &questions,
                    picks,
                    client::random_below,
                ));
            }
        }
    };
    // Picking a category opens its topics; picking it again closes them and shows everything.
    let mut pick_category = move |name: Option<String>| {
        let name = if name == *category.peek() { None } else { name };
        category.set(name);
        topic.set(None);
        shown_count.set(PAGE);
    };
    let mut pick_topic = move |name: String| {
        let name = Some(name).filter(|n| Some(n) != topic.peek().as_ref());
        topic.set(name);
        shown_count.set(PAGE);
    };
    let categories = match &*subjects.read() {
        Some(Ok(found)) => found.clone(),
        _ => crate::library::subjects(entries, &crate::library::Added::default()),
    };
    let topics = categories
        .iter()
        .find(|c| Some(&c.name) == category().as_ref())
        .map(|c| c.topics.clone())
        .unwrap_or_default();
    // On the Questions tab, a topic added on this server can be removed while it has no
    // questions.
    let remove_topic = move |category: String, topic: String| async move {
        if remove_library_topic(token.get(), category, topic)
            .await
            .is_ok()
        {
            subjects.restart();
        }
    };
    rsx! {
        if adding() {
            AddTopicDialog {
                categories: categories.clone(),
                on_added: move |()| {
                    adding.set(false);
                    subjects.restart();
                },
                on_close: move |()| adding.set(false),
            }
        }
        if let Some(entry) = editing() {
            LibraryQuestionEditor {
                key: "{entry.id}",
                entry,
                // Reload, so the list and its counts show the change.
                on_saved: move |_| {
                    editing.set(None);
                    library.restart();
                    subjects.restart();
                },
                on_close: move |()| editing.set(None),
            }
        }
        div { class: "library-layout",
            label { class: "search library-search",
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
            nav { class: "library-nav", aria_label: "Categories",
                button {
                    class: "library-category",
                    class: if category().is_none() { "selected" },
                    aria_pressed: category().is_none(),
                    onclick: move |_| pick_category(None),
                    span { class: "grow", "All categories" }
                    span { class: "count", "{entries.len()}" }
                }
                for LibraryCategory { name, count, look, .. } in categories.clone() {
                    div { key: "{name}", class: "library-group",
                        button {
                            class: "library-category",
                            class: if category().as_deref() == Some(name.as_str()) { "selected" },
                            aria_expanded: category().as_deref() == Some(name.as_str()),
                            onclick: {
                                let name = name.clone();
                                move |_| pick_category(Some(name.clone()))
                            },
                            CategoryIcon { look }
                            span { class: "grow", "{name}" }
                            span { class: "count", "{count}" }
                        }
                        if category().as_deref() == Some(name.as_str()) {
                            div { class: "library-topics",
                                for item in topics.clone() {
                                    div { key: "{item.name}", class: "library-topic-row",
                                        button {
                                            class: "library-topic",
                                            class: if topic().as_deref() == Some(item.name.as_str()) { "selected" },
                                            aria_pressed: topic().as_deref() == Some(item.name.as_str()),
                                            onclick: {
                                                let name = item.name.clone();
                                                move |_| pick_topic(name.clone())
                                            },
                                            span { class: "grow", "{item.name}" }
                                            span { class: "count", "{item.count}" }
                                        }
                                        if on_add.is_none() && item.count == 0 {
                                            button {
                                                class: "button ghost icon remove-topic",
                                                title: "Remove this topic",
                                                aria_label: "Remove {item.name}",
                                                onclick: {
                                                    let (category, topic) = (name.clone(), item.name.clone());
                                                    move |_| remove_topic(category.clone(), topic.clone())
                                                },
                                                "×"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if on_add.is_none() {
                    button {
                        class: "button ghost add-topic",
                        title: "Add a topic to a category, or a new category, to file questions under",
                        onclick: move |_| adding.set(true),
                        "＋ Add a topic"
                    }
                }
            }
            aside { class: "library-filters", aria_label: "Filters",
                FilterList {
                    title: "Difficulty",
                    options: difficulties,
                    selected: difficulty().map(|level| level.to_string()),
                    on_pick: move |key: Option<String>| {
                        difficulty.set(key.and_then(|k| k.parse().ok()));
                        shown_count.set(PAGE);
                    },
                }
                FilterList {
                    title: "Question type",
                    options: kinds,
                    selected: Some(kind()).filter(|k| !k.is_empty()),
                    on_pick: move |key: Option<String>| {
                        kind.set(key.unwrap_or_default());
                        shown_count.set(PAGE);
                    },
                }
                FilterList {
                    title: "Source",
                    options: origins,
                    selected: added().map(|origin| origin.to_string()),
                    on_pick: move |key: Option<String>| {
                        added.set(key.and_then(|k| k.parse().ok()));
                        shown_count.set(PAGE);
                    },
                }
                div {
                    class: "library-filter-list",
                    role: "group",
                    aria_label: "Sort by",
                    span { class: "library-filter-title", "Sort by" }
                    for order in LibrarySort::ALL {
                        button {
                            class: "library-option",
                            class: if sort() == order { "selected" },
                            aria_pressed: sort() == order,
                            onclick: move |_| {
                                sort.set(order);
                                shown_count.set(PAGE);
                            },
                            span { class: "grow", "{order.label()}" }
                        }
                    }
                }
            }
            section { class: "library-main card stack",
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
                    if on_add.is_some() {
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
                }
                ul { class: "list library-list",
                    for (index , entry) in matching.iter().take(shown_count()).enumerate() {
                        LibraryRow {
                            key: "{index}",
                            entry: (*entry).clone(),
                            look: look_of(&entry.category, &categories),
                            in_quiz: present.contains(entry.question.text.as_str()),
                            on_add,
                            on_edit: move |entry| editing.set(Some(entry)),
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
}

/// A pop-up adding a topic to a category of the library, or to a new one with an icon and a
/// colour no other category has, so quizzes can file their questions under it.
#[component]
fn AddTopicDialog(
    /// The categories so far, with their looks.
    categories: Vec<LibraryCategory>,
    on_added: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let token: AdminToken = use_context();
    let taken_icons: Vec<String> = categories.iter().map(|c| c.look.icon.clone()).collect();
    let taken_colors: Vec<String> = categories.iter().map(|c| c.look.color.clone()).collect();
    let mut category = use_signal(String::new);
    let mut topic = use_signal(String::new);
    // A new category starts with the first icon and colour still free.
    let mut icon = use_signal(|| {
        let free = CATEGORY_ICONS
            .iter()
            .find(|i| !taken_icons.iter().any(|t| t == *i));
        free.map(|i| (*i).to_owned()).unwrap_or_default()
    });
    let mut color = use_signal(|| {
        let free = CATEGORY_COLORS
            .iter()
            .find(|c| !taken_colors.iter().any(|t| t == *c));
        free.map(|c| (*c).to_owned()).unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);

    let typed = category().trim().to_lowercase();
    let existing = categories
        .iter()
        .find(|c| c.name.to_lowercase() == typed)
        .cloned();
    let new_category = existing.is_none() && !typed.is_empty();
    let blank = typed.is_empty()
        || topic().trim().is_empty()
        || (new_category && (icon().is_empty() || color().is_empty()));
    let add = move |evt: FormEvent| async move {
        evt.prevent_default();
        let look = new_category.then(|| CategoryLook {
            icon: icon(),
            color: color(),
        });
        match add_library_topic(token.get(), category(), topic(), look).await {
            Ok(_) => on_added.call(()),
            Err(err) => error.set(Some(message(&err))),
        }
    };
    rsx! {
        Dialog { title: "Add a topic", class: "add-topic-dialog", on_close,
            form { class: "stack", onsubmit: add,
                label { class: "field",
                    span { "Category" }
                    input {
                        list: "library-category-names",
                        placeholder: "One of the library's, or a new one",
                        autofocus: true,
                        value: category,
                        oninput: move |e| category.set(e.value()),
                    }
                    datalist { id: "library-category-names",
                        for c in categories.iter() {
                            option { value: "{c.name}" }
                        }
                    }
                }
                if let Some(found) = existing {
                    p { class: "category-preview",
                        CategoryIcon { look: found.look.clone() }
                        span { style: "color: {found.look.color}", "{found.name}" }
                        span { class: "muted small", "gets a new topic." }
                    }
                } else if new_category {
                    p { class: "category-preview",
                        CategoryIcon {
                            look: CategoryLook {
                                icon: icon(),
                                color: color(),
                            },
                        }
                        span { style: "color: {color}", "{category().trim()}" }
                        span { class: "muted small", "is a new category: pick its icon and colour." }
                    }
                    div { class: "look-choices", role: "group", aria_label: "Icon",
                        for key in CATEGORY_ICONS {
                            button {
                                r#type: "button",
                                class: "look-choice",
                                class: if icon() == key { "selected" },
                                style: "color: {color}",
                                disabled: taken_icons.iter().any(|t| t == key),
                                title: if taken_icons.iter().any(|t| t == key) { "Another category has this icon" } else { key },
                                aria_label: "Icon {key}",
                                aria_pressed: icon() == key,
                                onclick: move |_| icon.set(key.to_owned()),
                                Icon { kind: IconKind::of_category(key) }
                            }
                        }
                    }
                    div { class: "look-choices", role: "group", aria_label: "Colour",
                        for value in CATEGORY_COLORS {
                            button {
                                r#type: "button",
                                class: "look-choice swatch",
                                class: if color() == value { "selected" },
                                style: "--swatch: {value}",
                                disabled: taken_colors.iter().any(|t| t == value),
                                title: if taken_colors.iter().any(|t| t == value) { "Another category has this colour" } else { value },
                                aria_label: "Colour {value}",
                                aria_pressed: color() == value,
                                onclick: move |_| color.set(value.to_owned()),
                            }
                        }
                    }
                    p { class: "muted small",
                        "Icons and colours other categories have are greyed out."
                    }
                }
                label { class: "field",
                    span { "Topic" }
                    input {
                        placeholder: "e.g. Chess",
                        value: topic,
                        oninput: move |e| topic.set(e.value()),
                    }
                }
                ErrorBox { error: error() }
                div { class: "row between",
                    span { class: "muted small", "Added topics are kept on this server." }
                    button {
                        class: "button primary",
                        r#type: "submit",
                        disabled: blank,
                        "＋ Add topic"
                    }
                }
            }
        }
    }
}

/// An option of a [`FilterList`], with how many questions picking it shows.
#[derive(Clone, PartialEq)]
struct FilterOption {
    key: String,
    label: String,
    count: usize,
}

/// A titled list of options to filter by, like the categories: picking the selected option
/// again clears it.
#[component]
fn FilterList(
    title: &'static str,
    options: Vec<FilterOption>,
    selected: Option<String>,
    on_pick: EventHandler<Option<String>>,
) -> Element {
    rsx! {
        div { class: "library-filter-list", role: "group", aria_label: title,
            span { class: "library-filter-title", "{title}" }
            for option in options {
                button {
                    key: "{option.key}",
                    class: "library-option",
                    class: if selected.as_ref() == Some(&option.key) { "selected" },
                    aria_pressed: selected.as_ref() == Some(&option.key),
                    disabled: option.count == 0 && selected.as_ref() != Some(&option.key),
                    onclick: {
                        let key = option.key.clone();
                        let picked = selected.as_ref() == Some(&key);
                        move |_| on_pick.call((!picked).then(|| key.clone()))
                    },
                    span { class: "grow", "{option.label}" }
                    span { class: "count", "{option.count}" }
                }
            }
        }
    }
}

/// One library question with its answer, and a button to add it (with `on_add`).
#[component]
fn LibraryRow(
    entry: LibraryEntry,
    look: CategoryLook,
    in_quiz: bool,
    on_add: Option<EventHandler<Vec<Question>>>,
    on_edit: EventHandler<LibraryEntry>,
) -> Element {
    let question = entry.to_question();
    let edited = entry.clone();
    rsx! {
        li { class: "list-row library-row", style: "--category: {look.color}",
            div { class: "grow stack-tight",
                div { class: "muted small library-meta",
                    span {
                        span { class: "category-icon",
                            Icon { kind: IconKind::of_category(&look.icon) }
                        }
                        "{entry.category} · {entry.topic}"
                        if entry.edited {
                            span { class: "library-edited", " · edited" }
                        }
                        if entry.added {
                            span { class: "library-edited", " · added" }
                        }
                    }
                    span { class: "library-meta-kind",
                        "{entry.difficulty_name()} · {entry.kind_name()} · "
                        button {
                            class: "link-button",
                            title: "Edit this question",
                            aria_label: "Edit this question",
                            onclick: move |_| on_edit.call(edited.clone()),
                            "edit"
                        }
                        // Questions added here are not Quaesito's to fix.
                        if !entry.added {
                            " · "
                            a {
                                class: "link-button",
                                href: entry.report_url(),
                                target: "_blank",
                                rel: "noopener",
                                title: "Report this question as wrong, in a new GitHub issue",
                                "report"
                            }
                        }
                    }
                }
                span { class: "strong", "{entry.question.text}" }
                if !entry.question.passage.is_empty() {
                    span { class: "small library-passage", "{entry.question.passage}" }
                }
                match &entry.question.kind {
                    // Every option in its place: the right ones ticked, the others crossed out.
                    QuestionKind::Choice => rsx! {
                        span { class: "small library-options",
                            for answer in entry.question.answers.iter() {
                                if answer.correct {
                                    span { class: "library-answer", "✓ {answer.text}" }
                                } else {
                                    span { class: "library-wrong", "✗ {answer.text}" }
                                }
                            }
                        }
                    },
                    QuestionKind::Number { range: Some(range), .. } => rsx! {
                        span { class: "small",
                            span { class: "library-answer", "✓ {entry.answer()}" }
                            span { class: "muted", " · range {range.min} to {range.max}" }
                        }
                    },
                    _ => rsx! {
                        span { class: "small library-answer", "✓ {entry.answer()}" }
                    },
                }
            }
            div { class: "library-row-actions",
                if in_quiz {
                    span { class: "muted small", "✓ In the quiz" }
                } else if let Some(on_add) = on_add {
                    button {
                        class: "button",
                        onclick: move |_| on_add.call(vec![question.clone()]),
                        "＋ Add"
                    }
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
                            CategoryIcon { look: look_of(&name, &[]) }
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
