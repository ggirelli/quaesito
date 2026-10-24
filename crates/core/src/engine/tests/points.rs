//! How answers of each kind are scored.

use super::*;

#[test]
fn reveals_show_the_fastest_team_and_rank_changes() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(cats, pick(&[1]), false, t0 + secs(1)).unwrap();
    g.answer(owls, pick(&[0]), false, t0 + secs(4)).unwrap();
    let HostPhase::Closed {
        fastest, standings, ..
    } = g.host_view(t0).phase
    else {
        panic!("expected results");
    };
    assert_eq!(
        fastest,
        Some(Fastest {
            team: "Owls".to_owned(),
            answer_ms: 4000
        })
    );
    let rows = standings.unwrap();
    assert_eq!(
        rows[0].round,
        Some(RoundResult {
            joker: false,
            points: 800,
            answer_ms: Some(4000),
            previous_rank: None
        })
    );

    // Cats overtake Owls on question two.
    g.next(t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap();
    g.answer(owls, pick(&[1]), false, t0).unwrap();
    let HostPhase::Closed {
        standings: Some(rows),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected results");
    };
    assert_eq!(rows[0].team.id, cats);
    assert_eq!(rows[0].round.unwrap().previous_rank, Some(2));
    assert_eq!(rows[1].round.unwrap().previous_rank, Some(1));
}

#[test]
fn a_joker_doubles_one_question_once() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    // Jokers are off by default.
    assert!(g.answer(owls, pick(&[0]), true, t0).is_err());
    assert_eq!(
        g.player_view(owls, t0).unwrap().joker,
        JokerState::Unavailable
    );

    let mut g = game(QuizSettings {
        joker: true,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().joker, JokerState::Ready);
    g.answer(owls, pick(&[0]), true, t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap();
    assert_eq!(
        g.player_view(owls, t0).unwrap().joker,
        JokerState::PlayedHere
    );
    let rows = g.standings();
    assert_eq!((rows[0].team.id, rows[0].score), (owls, 2000));
    assert_eq!(rows[1].score, 1000);

    g.next(t0).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().joker, JokerState::Spent);
    assert!(g.answer(owls, pick(&[0]), true, t0).is_err());
    // Withdrawing a question gives the joker back.
    g.answer(cats, pick(&[0]), true, t0).unwrap();
    g.back(t0).unwrap();
    assert_eq!(g.player_view(cats, t0).unwrap().joker, JokerState::Ready);
}

#[test]
fn order_answers_score_each_item_in_place() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings::default());
    quiz.questions[0].kind = QuestionKind::Order {
        items: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
    };
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    assert_ne!(g.shown[0], vec![0, 1, 2]);
    // Shown as C, A, B.
    g.shown[0] = vec![2, 0, 1];
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let pigs = join(&mut g, "Pigs");
    g.next(t0).unwrap();
    let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
        panic!("expected the question");
    };
    assert_eq!(question.answers, ["C", "A", "B"]);
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Question { ref items, .. } if items == &["C", "A", "B"]
    ));
    assert!(
        g.answer(owls, Response::Order(vec![0, 0, 1]), false, t0)
            .is_err()
    );
    g.answer(owls, Response::Order(vec![1, 2, 0]), false, t0)
        .unwrap();
    g.answer(cats, Response::Order(vec![0, 1, 2]), false, t0)
        .unwrap();
    g.answer(pigs, Response::Order(vec![1, 0, 2]), false, t0)
        .unwrap();
    let HostPhase::Closed {
        reveal: Some(reveal),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected results");
    };
    assert_eq!(
        reveal,
        Reveal::Order {
            items: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            placed: vec![2, 1, 1],
        }
    );
    let score = |team| {
        g.standings()
            .into_iter()
            .find(|r| r.team.id == team)
            .unwrap()
            .score
    };
    assert_eq!((score(owls), score(cats), score(pigs)), (1000, 0, 333));
}

#[test]
fn pairs_answers_score_each_pair_matched() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings::default());
    let item = |text: &str| crate::model::PairItem {
        text: text.to_owned(),
        image: None,
    };
    let pairs: Vec<crate::model::Pair> =
        [("France", "Paris"), ("Italy", "Rome"), ("Spain", "Madrid")]
            .into_iter()
            .map(|(left, right)| crate::model::Pair {
                left: item(left),
                right: item(right),
            })
            .collect();
    quiz.questions[0].kind = QuestionKind::Pairs {
        pairs: pairs.clone(),
    };
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    assert_ne!(g.shown[0], vec![0, 1, 2]);
    // The right column shows Madrid, Paris, Rome.
    g.shown[0] = vec![2, 0, 1];
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let pigs = join(&mut g, "Pigs");
    g.next(t0).unwrap();
    let columns = PairColumns {
        left: vec![item("France"), item("Italy"), item("Spain")],
        right: vec![item("Madrid"), item("Paris"), item("Rome")],
    };
    let HostPhase::Question { question, .. } = g.host_view(t0).phase else {
        panic!("expected the question");
    };
    assert_eq!(question.input, InputKind::Pairs);
    assert_eq!(question.pairs.as_ref(), Some(&columns));
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Question { pairs: Some(ref p), .. } if p == &columns
    ));
    for wrong in [
        vec![Some(0), Some(0), None],
        vec![Some(3), None, None],
        vec![Some(1), Some(2)],
        vec![None, None, None],
    ] {
        assert!(g.answer(owls, Response::Pairs(wrong), false, t0).is_err());
    }
    g.answer(
        owls,
        Response::Pairs(vec![Some(1), Some(2), Some(0)]),
        false,
        t0,
    )
    .unwrap();
    g.answer(
        cats,
        Response::Pairs(vec![Some(0), Some(1), Some(2)]),
        false,
        t0,
    )
    .unwrap();
    // Partly matched answers count too.
    g.answer(pigs, Response::Pairs(vec![Some(1), None, None]), false, t0)
        .unwrap();
    let HostPhase::Closed {
        reveal: Some(reveal),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected results");
    };
    assert_eq!(
        reveal,
        Reveal::Pairs {
            pairs,
            matched: vec![2, 1, 1],
        }
    );
    let score = |team| {
        g.standings()
            .into_iter()
            .find(|r| r.team.id == team)
            .unwrap()
            .score
    };
    assert_eq!((score(owls), score(cats), score(pigs)), (1000, 0, 333));
}

#[test]
fn results_list_every_teams_points_per_question() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        joker: true,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats, \"the\"");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), true, t0).unwrap();
    g.answer(cats, pick(&[1]), false, t0).unwrap();
    g.next(t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap();
    g.next(t0).unwrap(); // close question two; Owls did not answer
    let results = g.results();
    assert_eq!(results.questions, ["one", "two"]);
    assert_eq!(results.rows[0].team, "Owls");
    assert_eq!(results.rows[0].points, [Some(2000), None]);
    assert_eq!(results.rows[0].jokers, [true, false]);
    assert_eq!(results.rows[1].points, [Some(0), Some(1000)]);
    assert_eq!(
        results.to_csv(),
        "Rank,Team,Total,one,two\r\n1,Owls,2000,2000,\r\n2,\"Cats, \"\"the\"\"\",1000,0,1000\r\n"
    );
}

#[test]
fn a_multiplier_scales_points_before_the_joker() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        joker: true,
        ..QuizSettings::default()
    });
    quiz.questions[0].multiplier = Some(3);
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), true, t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap();
    let scores: Vec<i32> = g.standings().iter().map(|r| r.score).collect();
    assert_eq!(scores, [6000, 3000]);
    let HostPhase::Closed { question, .. } = g.host_view(t0).phase else {
        panic!("expected results");
    };
    assert_eq!(question.multiplier, 3);
}

#[test]
fn true_false_plays_like_two_choices() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings::default());
    quiz.questions[0].kind = QuestionKind::TrueFalse { answer: false };
    quiz.questions[0].answers.clear();
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Question { answer_count: 2, ref items, .. } if items == &["True", "False"]
    ));
    assert!(g.answer(owls, pick(&[2]), false, t0).is_err());
    g.answer(owls, pick(&[1]), false, t0).unwrap();
    g.answer(cats, pick(&[0]), false, t0).unwrap();
    let HostPhase::Closed {
        question,
        reveal: Some(reveal),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected results");
    };
    assert_eq!(question.answers, ["True", "False"]);
    assert_eq!(
        reveal,
        Reveal::Choice {
            correct: vec![1],
            picks: vec![1, 1]
        }
    );
    assert_eq!(g.standings()[0].team.id, owls);
}

#[test]
fn parts_are_judged_one_by_one_and_share_the_points() {
    use crate::model::Part;
    let t0 = Instant::now();
    let part = |label: &str, accepted: &[&str]| Part {
        label: label.to_owned(),
        accepted: accepted.iter().map(|a| (*a).to_owned()).collect(),
        image: None,
    };
    let mut quiz = quiz(QuizSettings {
        points: crate::model::PointsMode::Flat,
        ..QuizSettings::default()
    });
    quiz.questions[0].kind = QuestionKind::Parts {
        parts: vec![
            part("Title", &["Yesterday"]),
            part(" ", &["The Beatles", "Beatles"]),
        ],
    };
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let pigs = join(&mut g, "Pigs");
    g.next(t0).unwrap();
    let texts = |a: &str, b: &str| Response::Parts(vec![a.to_owned(), b.to_owned()]);
    assert!(
        g.answer(owls, Response::Parts(vec!["x".to_owned()]), false, t0)
            .is_err()
    );
    assert!(g.answer(owls, texts(" ", ""), false, t0).is_err());
    g.answer(owls, texts("yesterday", "beatles"), false, t0)
        .unwrap();
    g.answer(cats, texts("Yesterday", "Paul McCartney"), false, t0)
        .unwrap();
    g.answer(pigs, texts("Let it be", ""), false, t0).unwrap();

    let HostPhase::Closed {
        reveal: Some(Reveal::Parts { parts }),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected a parts reveal");
    };
    assert_eq!(parts[1].label, "Answer 2");
    assert_eq!(parts[0].groups.len(), 2);
    // The blank artist is left out of the groups.
    assert_eq!(parts[1].groups.len(), 2);
    let score = |g: &Game, team| {
        g.standings()
            .into_iter()
            .find(|r| r.team.id == team)
            .unwrap()
            .score
    };
    assert_eq!(
        (score(&g, owls), score(&g, cats), score(&g, pigs)),
        (1000, 500, 0)
    );

    // The judge gives Paul McCartney half the artist's points.
    let key = parts[1]
        .groups
        .iter()
        .find(|g| g.text == "Paul McCartney")
        .unwrap()
        .key
        .clone();
    let half = Judgement {
        grade: Grade::Half,
        bonus: 0,
    };
    g.judge(0, &key, half, t0).unwrap();
    assert_eq!(score(&g, cats), 750);
}

#[test]
fn pins_score_by_distance() {
    use crate::model::{PinTolerance, Point};
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        points: crate::model::PointsMode::Flat,
        ..QuizSettings::default()
    });
    quiz.questions[0].image = Some("map.jpg".to_owned());
    quiz.questions[0].kind = QuestionKind::Pin {
        answer: Some(Point { x: 0.5, y: 0.5 }),
        aspect: 0.5,
        tolerance: PinTolerance::Normal,
    };
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let pigs = join(&mut g, "Pigs");
    g.next(t0).unwrap();
    assert!(matches!(
        g.player_view(owls, t0).unwrap().phase,
        PlayerPhase::Question { map: Some((ref m, aspect)), input: InputKind::Pin, .. } if m == "map.jpg" && (aspect - 0.5).abs() < f64::EPSILON
    ));
    let pin = |x, y| Response::Pin(Point { x, y });
    assert!(g.answer(owls, pin(1.5, 0.5), false, t0).is_err());
    g.answer(owls, pin(0.52, 0.5), false, t0).unwrap();
    // 0.34 of the height of a picture half as high as wide is 0.17 of its width.
    g.answer(cats, pin(0.5, 0.84), false, t0).unwrap();
    g.answer(pigs, pin(0.0, 0.0), false, t0).unwrap();
    let HostPhase::Closed {
        reveal: Some(Reveal::Pin { pins, .. }),
        fastest,
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected a pin reveal");
    };
    let points: Vec<(&str, i32)> = pins.iter().map(|p| (p.team.as_str(), p.points)).collect();
    assert_eq!(points, [("Owls", 1000), ("Cats", 500), ("Pigs", 0)]);
    assert_eq!(fastest.map(|f| f.team), Some("Owls".to_owned()));
}

#[test]
fn a_bonus_question_adds_its_share_and_is_judged() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings {
        points: crate::model::PointsMode::Flat,
        ..QuizSettings::default()
    });
    quiz.questions[0].bonus = Some(Bonus {
        text: "Who sings it?".to_owned(),
        accepted: vec!["Freddie Mercury".to_owned()],
        percent: 50,
    });
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let bats = join(&mut g, "Bats");
    let rats = join(&mut g, "Rats");
    g.next(t0).unwrap();
    let PlayerPhase::Question { bonus, .. } = g.player_view(owls, t0).unwrap().phase else {
        panic!("expected a question");
    };
    assert_eq!(bonus, Some(("Who sings it?".to_owned(), 50)));
    let bonus = |text: &str| Some(text.to_owned());
    g.answer_with_bonus(owls, pick(&[0]), false, bonus("freddie mercury"), t0)
        .unwrap();
    g.answer_with_bonus(cats, pick(&[1]), false, bonus("Freddie Mercury!"), t0)
        .unwrap();
    g.answer_with_bonus(bats, pick(&[0]), false, bonus("Freddy"), t0)
        .unwrap();
    g.answer(rats, pick(&[0]), false, t0).unwrap();
    let score = |g: &Game, team: u32| {
        g.standings()
            .into_iter()
            .find(|r| r.team.id == team)
            .unwrap()
            .score
    };
    assert_eq!(
        [owls, cats, bats, rats].map(|t| score(&g, t)),
        [1500, 500, 1000, 1000]
    );
    let HostPhase::Closed {
        bonus: Some(reveal),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected the bonus answers");
    };
    assert_eq!(reveal.label, "Bonus answers");
    assert_eq!(reveal.groups.len(), 2);
    // The judge gives Freddy half the bonus; the main answers cannot be judged.
    let half = Judgement {
        grade: Grade::Half,
        bonus: 0,
    };
    g.judge(0, "bonus|freddy", half, t0).unwrap();
    assert_eq!(score(&g, bats), 1250);
    assert!(g.judge(0, "freddy", half, t0).is_err());
}

#[test]
fn flat_points_ignore_speed() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        points: crate::model::PointsMode::Flat,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0 + secs(9)).unwrap();
    g.answer(cats, pick(&[0]), false, t0 + secs(1)).unwrap();
    let standings = g.standings();
    assert_eq!(
        standings.iter().map(|r| r.score).collect::<Vec<_>>(),
        [1000, 1000]
    );
    // Same score: the faster team ranks first.
    assert_eq!(standings[0].team.id, cats);
    // Answer times only matter to the millisecond when points depend on speed.
    assert!(!g.host_view(t0).speed_points);
    assert!(game(QuizSettings::default()).host_view(t0).speed_points);
}

#[test]
fn answers_are_validated() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
    g.next(t0).unwrap();
    assert!(g.answer(owls, pick(&[]), false, t0).is_err());
    assert!(g.answer(owls, pick(&[7]), false, t0).is_err());
    assert!(g.answer(owls, pick(&[0, 1]), false, t0).is_err());
    assert!(g.answer(owls, pick(&[0]), false, t0 + secs(10)).is_err());
    assert!(g.answer(99, pick(&[0]), false, t0).is_err());
}
