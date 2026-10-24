//! Quaesito: quizzes among friends.

pub mod api;
pub mod ui;

// The domain modules, under the paths the app has always used.
pub use quaesito_core::{library, model, protocol, scoring, search};
