//! Quaesito: quizzes among friends.

pub mod api;
pub mod library;
pub mod model;
pub mod protocol;
pub mod scoring;
pub mod search;
#[cfg(feature = "server")]
pub mod server;
pub mod ui;
