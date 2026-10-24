use super::*;

use std::time::{Duration, Instant};

use crate::model::{Answer, QuizSettings};
use crate::{
    model::{
        AdvanceMode, Bonus, NumberScoring, PointsMode, Question, QuestionKind, Quiz, RevealMode,
    },
    protocol::{
        AnswerOutcome, BACKDROP_COUNT, DEFAULT_MAX_TEAMS, Explanation, Fastest, Grade, HostPhase,
        InputKind, JokerState, Judgement, MAX_TEAMS_CEILING, PairColumns, PlayerPhase,
        QuestionInfo, Response, Reveal, RoundResult, Upcoming,
    },
};

mod flow;
mod numbers;
mod points;
mod snapshot;
mod teams;
mod typed;

fn quiz(settings: QuizSettings) -> Quiz {
    let question = |text: &str| Question {
        text: text.to_owned(),
        kind: crate::model::QuestionKind::Choice,
        image: None,
        image_reveal: crate::model::ImageReveal::Shown,
        image_hold_secs: 0,
        audio: None,
        time_limit_secs: Some(10),
        multiplier: None,
        answers: vec![
            Answer {
                text: "right".to_owned(),
                correct: true,
                image: None,
            },
            Answer {
                text: "wrong".to_owned(),
                correct: false,
                image: None,
            },
        ],
        bonus: None,
        passage: String::new(),
        explanation: String::new(),
        explanation_image: None,
        explanation_audio: None,
        category: String::new(),
        topic: String::new(),
        library: None,
    };
    Quiz {
        settings,
        questions: vec![question("one"), question("two")],
        ..Quiz::new("id".to_owned(), "Test".to_owned())
    }
}

/// Not random, so that tests repeat: always the last choice.
fn random_below(n: usize) -> usize {
    n - 1
}

fn game(settings: QuizSettings) -> Game {
    Game::new("123456".to_owned(), quiz(settings), random_below).unwrap()
}

fn join(game: &mut Game, name: &str) -> u32 {
    let id = game.join(name, format!("token-{name}")).unwrap();
    game.set_connected(id, true);
    id
}

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

fn pick(selected: &[usize]) -> Response {
    Response::Choice(selected.to_vec())
}

/// Where the big screen is, in short: "Q1" (open), "C1" (closed), "S1", "R1" (review),
/// "T1" (standings at a results slide), "P" (podium).
fn at(g: &Game, t: Instant) -> String {
    match g.host_view(t).phase {
        HostPhase::Question { question, .. } => format!("Q{}", question.index),
        HostPhase::Closed { question, .. } => format!("C{}", question.index),
        HostPhase::Review { question, .. } => format!("R{}", question.index),
        HostPhase::Slide { .. } => "S".to_owned(),
        HostPhase::Standings { .. } => match g.phase {
            Phase::Standings { index, .. } => format!("T{index}"),
            _ => unreachable!(),
        },
        HostPhase::Podium { .. } => "P".to_owned(),
        HostPhase::Network { .. } => "N".to_owned(),
        HostPhase::Lobby { .. } => "L".to_owned(),
        HostPhase::Ready { .. } => "W".to_owned(),
    }
}

fn open_game(kind: QuestionKind, settings: QuizSettings) -> Game {
    let mut quiz = quiz(settings);
    quiz.questions.truncate(1);
    quiz.questions[0].kind = kind;
    Game::new("123456".to_owned(), quiz, random_below).unwrap()
}

fn text(answer: &str) -> Response {
    Response::Text(answer.to_owned())
}

fn text_groups(g: &Game) -> Vec<(String, Vec<String>, Judgement, bool)> {
    let (HostPhase::Closed {
        reveal: Some(Reveal::Text { groups, .. }),
        ..
    }
    | HostPhase::Review {
        reveal: Reveal::Text { groups, .. },
        ..
    }) = g.host_view(Instant::now()).phase
    else {
        panic!("expected typed answers on screen");
    };
    groups
        .into_iter()
        .map(|g| (g.text, g.teams, g.judgement, g.amended))
        .collect()
}
