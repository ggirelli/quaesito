//! Quaesito's Playground: one quiz played in the browser alone, with no server.

use dioxus::prelude::*;
use quaesito::{model::Quiz, ui::playground::Playground};

/// The quiz the Playground plays.
const QUIZ: &str = include_str!("demo-quiz.json");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    match serde_json::from_str::<Quiz>(QUIZ) {
        Ok(quiz) => rsx! {
            Playground { quiz }
        },
        Err(err) => rsx! {
            p { "The Playground's quiz is unreadable: {err}" }
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use quaesito::{
        engine::{Game, Instant},
        model::{QuestionKind, Quiz},
        protocol::{PlayerPhase, Response},
    };

    use super::QUIZ;

    fn quiz() -> Quiz {
        serde_json::from_str(QUIZ).unwrap()
    }

    #[test]
    fn the_demo_quiz_is_valid_and_needs_no_server() {
        let quiz = quiz();
        let issues = quiz.validate();
        assert!(issues.is_empty(), "{issues:?}");
        // Pictures and sounds come from a server's uploads.
        assert_eq!(quiz.media(), Vec::<String>::new());
        let kinds: BTreeSet<_> = quiz
            .questions
            .iter()
            .map(|q| serde_json::to_value(&q.kind).unwrap())
            .map(|kind| match kind {
                serde_json::Value::String(name) => name,
                kind => kind.as_object().unwrap().keys().next().unwrap().clone(),
            })
            .collect();
        // Every kind but pins, which need a picture.
        let expected: BTreeSet<_> = QuestionKind::NAMES
            .iter()
            .filter(|&&name| name != "Pin")
            .map(|&name| name.to_owned())
            .collect();
        assert_eq!(kinds, expected);
        assert!(
            quiz.questions
                .iter()
                .filter(|q| !matches!(q.kind, QuestionKind::Slide { .. }))
                .all(|q| !q.explanation.is_empty()),
            "every question explains its answer"
        );
    }

    /// Plays the questions that take a fixed answer with their right answers (the order and
    /// pairs questions are shown shuffled; the browser check plays them).
    #[test]
    fn the_demo_quizs_answers_are_right() {
        let t0 = Instant::now();
        let mut game = Game::new(String::new(), quiz(), |_| 0).unwrap();
        let team = game.join("You", String::new()).unwrap();
        game.set_connected(team, true);
        let right = [
            None,
            Some(Response::Choice(vec![1])),
            Some(Response::Choice(vec![0, 1])),
            Some(Response::Choice(vec![0])),
            Some(Response::Text("canberra".to_owned())),
            Some(Response::Number(88.0)),
            None,
            Some(Response::Parts(vec![
                "Leonardo".to_owned(),
                "Paris".to_owned(),
            ])),
            None,
            None,
        ];
        assert_eq!(right.len(), quiz().questions.len());
        game.next(t0).unwrap();
        for (index, response) in right.into_iter().enumerate() {
            if let Some(response) = response {
                game.answer(team, response, false, t0).unwrap();
                let view = game.player_view(team, t0).unwrap();
                let PlayerPhase::Closed {
                    result: Some((outcome, _)),
                    ..
                } = view.phase
                else {
                    panic!("question {index} did not close with a result");
                };
                assert!(outcome.fully_correct, "question {index}");
            } else if !matches!(quiz().questions[index].kind, QuestionKind::Slide { .. }) {
                game.next(t0).unwrap();
            }
            game.next(t0).unwrap();
        }
    }
}
