//! Quaesito: quizzes among friends.

pub mod api;
pub mod model;
pub mod protocol;
pub mod scoring;
#[cfg(feature = "server")]
pub mod server;
pub mod ui;
