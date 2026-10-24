//! Quaesito: quizzes among friends.

#[cfg(feature = "fullstack")]
pub mod api;
pub mod ui;

// The domain modules, under the paths the app has always used.
pub use quaesito_core::{engine, library, model, protocol, scoring, search};
