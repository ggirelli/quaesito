//! Moving through the quiz, and what the screens show along the way.

use super::*;
use crate::engine::flow::AUTO_AFTER_REVEAL;

#[test]
fn the_wifi_slide_comes_before_the_lobby() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        network: crate::model::NetworkSlide {
            enabled: true,
            name: "Pub".to_owned(),
            password: String::new(),
        },
        ..QuizSettings::default()
    });
    let view = g.host_view(t0);
    assert!(matches!(view.phase, HostPhase::Network { .. }));
    assert!(view.phase.is_start());
    assert!(g.back(t0).is_err());
    assert!(g.pause(t0).is_err());
    // Teams may join while the slide is up, and the host may move on without them.
    assert!(g.in_lobby());
    join(&mut g, "Owls");
    g.next(t0).unwrap();
    let view = g.host_view(t0);
    assert_eq!(view.phase, HostPhase::Lobby { wifi_slide: true });
    assert!(!view.phase.is_start());
    g.back(t0).unwrap();
    assert!(matches!(g.host_view(t0).phase, HostPhase::Network { .. }));
    // A test can start at a chosen question straight from the slide.
    g.start_from(1, t0).unwrap();
    assert_eq!(at(&g, t0), "Q1");
    // Without the slide, the lobby is the start.
    assert!(game(QuizSettings::default()).host_view(t0).phase.is_start());
}

#[test]
fn each_phone_lists_the_answers_in_its_own_lasting_order() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings::default());
    quiz.questions[0].answers = (0..6)
        .map(|i| Answer {
            text: format!("answer {i}"),
            correct: i == 0,
            image: None,
        })
        .collect();
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let teams: Vec<u32> = (0..8).map(|i| join(&mut g, &format!("Team {i}"))).collect();
    g.next(t0).unwrap();
    let order = |g: &Game, team| match g.player_view(team, t0).unwrap().phase {
        PlayerPhase::Question { order, .. } => order,
        other => panic!("expected the question, got {other:?}"),
    };
    let orders: Vec<Vec<usize>> = teams.iter().map(|&t| order(&g, t)).collect();
    for list in &orders {
        let mut sorted = list.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..6).collect::<Vec<_>>());
    }
    // Eight phones, 720 orders: they are not all the same.
    assert!(orders.iter().any(|o| o != &orders[0]));
    // Asking again gives the same order.
    assert_eq!(order(&g, teams[3]), orders[3]);
}

#[test]
fn a_test_can_start_from_any_question_or_slide() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    assert!(g.start_from(1, t0).is_err()); // no teams yet
    let owls = join(&mut g, "Owls");
    assert!(g.start_from(2, t0).is_err());
    g.start_from(1, t0).unwrap();
    let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
        panic!("expected question two");
    };
    assert_eq!(question.index, 1);
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert!(g.start_from(0, t0).is_err());
    // The skipped question counts as not played.
    g.back(t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            question: QuestionInfo { index: 0, .. },
            ..
        }
    ));
}

#[test]
fn pausing_freezes_the_clock_and_extra_time_extends_it() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let _cats = join(&mut g, "Cats");
    assert!(g.pause(t0).is_err());
    g.next(t0).unwrap();
    g.pause(t0 + secs(2)).unwrap();
    assert!(g.pause(t0 + secs(2)).is_err());
    assert_eq!(g.next_deadline(), None);
    let HostPhase::Question {
        remaining_ms,
        elapsed_ms,
        ..
    } = g.host_view(t0 + secs(30)).phase
    else {
        panic!("expected the question");
    };
    assert_eq!((remaining_ms, elapsed_ms), (8000, 2000));
    assert!(g.host_view(t0).paused);

    // Answers still count while paused, timed at the pause.
    g.answer(owls, pick(&[0]), false, t0 + secs(30)).unwrap();
    g.resume(t0 + secs(30));
    assert_eq!(g.next_deadline(), Some(t0 + secs(38)));
    g.add_time(secs(10)).unwrap();
    assert_eq!(g.next_deadline(), Some(t0 + secs(48)));
    assert!(!g.tick(t0 + secs(40)));
    assert!(g.tick(t0 + secs(48)));
    assert!(g.add_time(secs(10)).is_err());
    assert_eq!(g.standings()[0].score, 900);
}

#[test]
fn going_back_revisits_without_replaying() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    assert!(g.back(t0).is_err());
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap(); // closes question one
    g.next(t0).unwrap();
    g.answer(owls, pick(&[1]), false, t0).unwrap(); // closes question two
    g.back(t0).unwrap(); // back to question one's results
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed { question, .. } if question.index == 0
    ));
    // Going forward shows question two's results again instead of asking it anew.
    g.next(t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed { question, .. } if question.index == 1
    ));

    // Leaving an open question withdraws it.
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let _cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    g.back(t0).unwrap();
    assert!(matches!(g.host_view(t0).phase, HostPhase::Lobby { .. }));
    g.next(t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Question { answered: 0, .. }
    ));

    // Podium steps go back one at a time, then to the last results.
    g.next(t0).unwrap();
    g.next(t0).unwrap();
    g.next(t0).unwrap();
    g.next(t0).unwrap(); // podium, nothing revealed
    g.next(t0).unwrap(); // 2nd place
    g.back(t0).unwrap();
    assert!(
        matches!(g.host_view(t0).phase, HostPhase::Podium { ref standings, .. } if standings.is_empty())
    );
    g.back(t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed { question, .. } if question.index == 1
    ));
}

#[test]
fn a_question_closed_by_mistake_can_be_reopened() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.set_connected(owls, true);
    g.set_connected(cats, true);
    assert!(g.reopen(t0).is_err());
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0 + Duration::from_millis(500))
        .unwrap();
    // A double click closes question one and opens question two.
    g.next(t0 + secs(3)).unwrap();
    g.next(t0 + secs(3)).unwrap();
    assert!(g.reopen(t0 + secs(3)).is_err());
    g.back(t0 + secs(4)).unwrap();
    g.reopen(t0 + secs(5)).unwrap();
    let HostPhase::Question {
        question,
        remaining_ms,
        answered,
        ..
    } = g.host_view(t0 + secs(5)).phase
    else {
        panic!("expected the question open again");
    };
    assert_eq!((question.index, answered), (0, 1));
    assert_eq!(remaining_ms, 10_000);
    assert!(g.answer(owls, pick(&[1]), false, t0 + secs(6)).is_err());
    g.answer(cats, pick(&[0]), false, t0 + secs(6)).unwrap();
    // Everyone is in: closed again, the Owls keeping their answer time.
    let HostPhase::Closed {
        fastest, answered, ..
    } = g.host_view(t0 + secs(6)).phase
    else {
        panic!("expected results");
    };
    assert_eq!(answered, 2);
    assert_eq!(
        fastest.map(|f| (f.team, f.answer_ms)),
        Some(("Owls".to_owned(), 500))
    );
    assert!(g.reopen(t0 + secs(7)).is_err());
    // Going forward asks question two afresh, as it was withdrawn.
    g.next(t0 + secs(7)).unwrap();
    assert!(matches!(
        g.host_view(t0 + secs(7)).phase,
        HostPhase::Question { answered: 0, .. }
    ));
}

#[test]
fn full_game_with_reveal_after_each_question() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    assert_eq!(g.next_deadline(), Some(t0 + secs(10)));

    // Owls answer right instantly, Cats wrong; everyone answered closes the question.
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Question { answered: 1, .. }
    ));
    g.answer(cats, pick(&[1]), false, t0 + secs(1)).unwrap();
    let HostPhase::Closed {
        reveal: Some(reveal),
        standings: Some(standings),
        then: Upcoming::Question,
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected a revealed closed question");
    };
    assert_eq!(
        reveal,
        Reveal::Choice {
            correct: vec![0],
            picks: vec![1, 1]
        }
    );
    assert_eq!(standings[0].team.id, owls);
    assert_eq!(standings[0].score, 1000);
    let cats_view = g.player_view(cats, t0).unwrap();
    assert_eq!(cats_view.score, Some(0));
    assert!(matches!(
        cats_view.phase,
        PlayerPhase::Closed {
            result: Some((
                AnswerOutcome {
                    points: 0,
                    fully_correct: false,
                    joker: false
                },
                _
            )),
            ..
        }
    ));

    // Manual advance: no deadline while closed.
    assert_eq!(g.next_deadline(), None);
    g.next(t0 + secs(2)).unwrap();

    // Nobody answers question two; the timer closes it.
    assert!(!g.tick(t0 + secs(11)));
    assert!(g.tick(t0 + secs(12)));
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            then: Upcoming::Podium,
            ..
        }
    ));

    // Podium: 2nd then 1st.
    g.next(t0 + secs(13)).unwrap();
    let podium = |g: &Game| match g.host_view(t0).phase {
        HostPhase::Podium {
            standings,
            finished,
        } => (
            standings.iter().map(|r| r.rank).collect::<Vec<_>>(),
            finished,
        ),
        other => panic!("expected podium, got {other:?}"),
    };
    assert_eq!(podium(&g), (vec![], false));
    assert_eq!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Podium {
            rank: None,
            teams: 2,
            finished: false
        }
    );
    g.next(t0).unwrap();
    assert_eq!(podium(&g), (vec![2], false));
    g.next(t0).unwrap();
    assert_eq!(podium(&g), (vec![1, 2], true));
    assert!(g.is_finished());
    assert_eq!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Podium {
            rank: Some(1),
            teams: 2,
            finished: true
        }
    );
    assert!(g.next(t0).is_err());
}

#[test]
fn reveal_at_end_hides_results_then_reviews() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        reveal: RevealMode::AtEnd,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0 + secs(5)).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            reveal: None,
            standings: None,
            ..
        }
    ));
    let view = g.player_view(owls, t0).unwrap();
    assert_eq!(view.score, None);
    assert!(matches!(
        view.phase,
        PlayerPhase::Closed { result: None, .. }
    ));

    g.next(t0).unwrap(); // question two
    g.next(t0).unwrap(); // close early
    g.next(t0).unwrap(); // review one
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Review {
            index: 0,
            outcome: AnswerOutcome {
                points: 750,
                fully_correct: true,
                joker: false
            },
            ..
        }
    ));
    g.next(t0).unwrap(); // review two, which nobody answered
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Review {
            answered: 0,
            then: Upcoming::Podium,
            ..
        }
    ));
    g.next(t0).unwrap(); // podium
    assert!(matches!(g.host_view(t0).phase, HostPhase::Podium { .. }));
}

#[test]
fn explanations_wait_for_the_answer() {
    let t0 = Instant::now();
    let explained = |reveal| {
        let mut quiz = quiz(QuizSettings {
            reveal,
            ..QuizSettings::default()
        });
        quiz.questions[0].explanation = " Saturn has 274.\n".to_owned();
        quiz.questions[0].explanation_image = Some("rings.jpg".to_owned());
        quiz.questions[0].explanation_audio = Some("fanfare.mp3".to_owned());
        quiz.questions[1].explanation = "  ".to_owned();
        let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
        join(&mut g, "Owls");
        g
    };
    let shown = Some(Explanation {
        text: "Saturn has 274.".to_owned(),
        image: Some("rings.jpg".to_owned()),
        audio: Some("fanfare.mp3".to_owned()),
    });

    let mut g = explained(RevealMode::AfterEachQuestion);
    g.next(t0).unwrap(); // question one
    g.next(t0).unwrap(); // close early
    let HostPhase::Closed { explanation, .. } = g.host_view(t0).phase else {
        panic!("not closed");
    };
    assert_eq!(explanation, shown);
    g.next(t0).unwrap(); // question two
    g.next(t0).unwrap(); // close early
    let HostPhase::Closed { explanation, .. } = g.host_view(t0).phase else {
        panic!("not closed");
    };
    assert_eq!(explanation, None, "blank text and no media");

    let mut g = explained(RevealMode::AtEnd);
    g.next(t0).unwrap(); // question one
    g.next(t0).unwrap(); // close early
    let HostPhase::Closed { explanation, .. } = g.host_view(t0).phase else {
        panic!("not closed");
    };
    assert_eq!(explanation, None, "the answer is not revealed yet");
    g.next(t0).unwrap(); // question two
    g.next(t0).unwrap(); // close early
    g.next(t0).unwrap(); // review one
    let HostPhase::Review { explanation, .. } = g.host_view(t0).phase else {
        panic!("not reviewing");
    };
    assert_eq!(explanation, shown);
}

#[test]
fn results_slides_reveal_each_part_then_the_standings() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        reveal: RevealMode::AtEnd,
        ..QuizSettings::default()
    });
    // one, two, [results], three, four
    let mut more = quiz.questions.clone();
    more[0].text = "three".to_owned();
    more[1].text = "four".to_owned();
    quiz.questions.push(Question::results_slide());
    quiz.questions.extend(more);
    let mut g = Game::new("123456".to_owned(), quiz.clone(), random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");

    g.next(t0).unwrap();
    g.answer(owls, pick(&[1]), false, t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap(); // Cats lead
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    g.answer(cats, pick(&[1]), false, t0).unwrap();
    assert_eq!(at(&g, t0), "C1");
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            then: Upcoming::Results,
            ..
        }
    ));
    let mut path = Vec::new();
    for _ in 0..3 {
        g.next(t0).unwrap();
        path.push(at(&g, t0));
    }
    assert_eq!(path, ["R0", "R1", "T2"]);
    let HostPhase::Standings {
        title, standings, ..
    } = g.host_view(t0).phase
    else {
        panic!("expected the standings");
    };
    assert_eq!(title, "Results so far");
    assert_eq!(standings.len(), 2);
    let round = standings[0].round.unwrap();
    assert_eq!(
        (round.points, round.previous_rank),
        (standings[0].score, None)
    );
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Standings { teams: 2, .. }
    ));
    assert!(g.player_view(owls, t0).unwrap().score.is_some());
    // Teams can still join at an intermission.
    join(&mut g, "Bats");

    // Back walks the same way.
    let mut path = Vec::new();
    for _ in 0..3 {
        g.back(t0).unwrap();
        path.push(at(&g, t0));
    }
    assert_eq!(path, ["R1", "R0", "C1"]);
    for _ in 0..3 {
        g.next(t0).unwrap();
    }
    assert_eq!(at(&g, t0), "T2");

    // The second part: only its questions are reviewed at the end.
    g.next(t0).unwrap();
    assert_eq!(at(&g, t0), "Q3");
    g.next(t0).unwrap();
    g.next(t0).unwrap();
    g.next(t0).unwrap();
    assert_eq!(at(&g, t0), "C4");
    let mut path = Vec::new();
    for _ in 0..3 {
        g.next(t0).unwrap();
        path.push(at(&g, t0));
    }
    assert_eq!(path, ["R3", "R4", "P"]);
    g.back(t0).unwrap();
    assert_eq!(at(&g, t0), "R4");
    g.back(t0).unwrap();
    g.back(t0).unwrap();
    assert_eq!(at(&g, t0), "C4");
}

#[test]
fn ending_on_a_results_slide_skips_the_end_review() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        reveal: RevealMode::AtEnd,
        ..QuizSettings::default()
    });
    quiz.questions.push(Question::results_slide());
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    join(&mut g, "Owls");
    let mut path = Vec::new();
    for _ in 0..6 {
        g.next(t0).unwrap();
        path.push(at(&g, t0));
    }
    assert_eq!(path, ["Q0", "C0", "Q1", "C1", "R0", "R1"]);
    g.next(t0).unwrap();
    assert_eq!(at(&g, t0), "T2");
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Standings {
            then: Upcoming::Podium,
            ..
        }
    ));
    g.next(t0).unwrap();
    assert_eq!(at(&g, t0), "P");
    g.back(t0).unwrap();
    assert_eq!(at(&g, t0), "T2");
}

#[test]
fn slides_show_between_questions_and_are_skipped_in_the_review() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        reveal: RevealMode::AtEnd,
        ..QuizSettings::default()
    });
    let slide = |title: &str| Question {
        kind: QuestionKind::Slide {
            title: title.to_owned(),
            results: false,
        },
        text: " Get ready ".to_owned(),
        ..Question::default()
    };
    quiz.questions.insert(0, slide("Round 1"));
    quiz.questions.insert(2, slide("Round 2"));
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");

    g.next(t0).unwrap();
    let HostPhase::Slide { slide, then } = g.host_view(t0).phase else {
        panic!("expected a slide");
    };
    assert_eq!(slide.title, "Round 1");
    assert_eq!(slide.text, "Get ready");
    assert_eq!(then, Upcoming::Question);
    assert_eq!(g.player_view(owls, t0).unwrap().phase, PlayerPhase::Slide);
    assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
    // Manual advance waits on slides.
    assert_eq!(g.next_deadline(), None);

    g.next(t0).unwrap();
    let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
        panic!("expected a question");
    };
    assert_eq!((question.index, question.number, question.total), (1, 1, 2));
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Question {
            index: 0,
            total: 2,
            ..
        }
    ));
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            then: Upcoming::Slide,
            ..
        }
    ));
    g.next(t0).unwrap(); // Round 2
    g.next(t0).unwrap(); // question two
    g.answer(owls, pick(&[1]), false, t0).unwrap();
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Closed {
            then: Upcoming::Answers,
            ..
        }
    ));

    // The review only walks through questions.
    g.next(t0).unwrap();
    let HostPhase::Review { question, then, .. } = g.host_view(t0).phase else {
        panic!("expected the review");
    };
    assert_eq!((question.index, then), (1, Upcoming::Question));
    g.next(t0).unwrap();
    let HostPhase::Review { question, then, .. } = g.host_view(t0).phase else {
        panic!("expected the review");
    };
    assert_eq!((question.index, then), (3, Upcoming::Podium));
    g.next(t0).unwrap();
    assert!(matches!(g.host_view(t0).phase, HostPhase::Podium { .. }));
    assert_eq!(g.standings()[0].score, 1000);
}

#[test]
fn auto_advance_moves_on_by_itself() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        advance: AdvanceMode::Auto,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert_eq!(g.next_deadline(), Some(t0 + AUTO_AFTER_REVEAL));
    assert_eq!(g.host_view(t0).auto_advance_ms, Some(8000));
    assert!(g.tick(t0 + AUTO_AFTER_REVEAL));
    assert!(matches!(
        g.host_view(t0).phase,
        HostPhase::Question {
            question: QuestionInfo { index: 1, .. },
            ..
        }
    ));
}

#[test]
fn scores_do_not_move_while_a_question_is_open() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().score, Some(0));
    g.next(t0).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().score, Some(1000));
}

#[test]
fn soundtrack_questions_wait_for_the_host() {
    let t0 = Instant::now();
    let mut settings = quiz(QuizSettings {
        advance: AdvanceMode::Auto,
        ..QuizSettings::default()
    });
    settings.questions[1].audio = Some("song.mp3".to_owned());
    let mut g = Game::new("123456".to_owned(), settings, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert!(g.tick(t0 + AUTO_AFTER_REVEAL));

    // Question two waits, even with auto-advance, and takes no answers yet.
    let HostPhase::Ready { question } = g.host_view(t0).phase else {
        panic!("expected the soundtrack to wait for the host");
    };
    assert_eq!(question.audio.as_deref(), Some("song.mp3"));
    assert_eq!(g.next_deadline(), None);
    assert_eq!(g.host_view(t0).auto_advance_ms, None);
    assert!(g.answer(owls, pick(&[0]), false, t0 + secs(20)).is_err());
    assert_eq!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Ready { index: 1, total: 2 }
    );

    // The clock starts with the music.
    let t1 = t0 + secs(30);
    g.next(t1).unwrap();
    assert_eq!(g.next_deadline(), Some(t1 + secs(10)));
    g.answer(owls, pick(&[0]), false, t1).unwrap();
    assert_eq!(g.player_view(owls, t1).unwrap().score, Some(2000));
}

#[test]
fn a_passage_shows_with_its_question() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        phone_display: crate::model::PhoneDisplay::FullText,
        ..QuizSettings::default()
    });
    quiz.questions[0].passage = " Is this the real life?\nIs this just fantasy? ".to_owned();
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
        panic!("expected a question");
    };
    assert_eq!(
        question.passage,
        "Is this the real life?\nIs this just fantasy?"
    );
    let PlayerPhase::Question {
        text: Some((text, _)),
        ..
    } = g.player_view(owls, t0).unwrap().phase
    else {
        panic!("expected the full text");
    };
    assert_eq!(text, "one\n\nIs this the real life?\nIs this just fantasy?");
}

#[test]
fn phone_shows_texts_only_in_full_text_mode() {
    let t0 = Instant::now();
    for (display, expect_text) in [
        (crate::model::PhoneDisplay::ButtonsOnly, false),
        (crate::model::PhoneDisplay::FullText, true),
    ] {
        let mut g = game(QuizSettings {
            phone_display: display,
            ..QuizSettings::default()
        });
        let owls = join(&mut g, "Owls");
        g.next(t0).unwrap();
        let PlayerPhase::Question { text, .. } = g.player_view(owls, t0).unwrap().phase else {
            panic!("expected a question");
        };
        assert_eq!(text.is_some(), expect_text);
    }
}
