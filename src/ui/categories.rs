//! The question categories' looks, as the big screen draws them.

use dioxus::prelude::*;

use super::widgets::{Icon, IconKind};
use crate::library::{CategoryLook, LibraryCategory, look_of};

/// The library's categories with their looks, those added on this server included, once loaded:
/// what the big screen draws a question's category with.
#[derive(Clone, Copy)]
pub struct Categories(pub Resource<Vec<LibraryCategory>>);

impl Categories {
    /// Only the library's own categories, for a game without a server.
    pub fn use_builtin() -> Self {
        Self(use_resource(|| async { Vec::new() }))
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
