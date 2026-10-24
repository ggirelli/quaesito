//! Quaesito's domain logic, free of any server, runtime, filesystem or network: quizzes and
//! their validation, the messages of a live game, scoring and the game engine.

pub mod engine;
pub mod library;
pub mod matching;
pub mod model;
pub mod protocol;
pub mod scoring;
pub mod search;

/// Compares secrets in a time that does not tell how much of them matched.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}
