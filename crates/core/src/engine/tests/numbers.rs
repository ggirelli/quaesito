//! Number questions: ranges, closeness and the judge's bonus.

use super::*;

#[test]
fn a_bonus_on_a_number_question_is_a_share_of_the_closest_points() {
    let t0 = Instant::now();
    let mut quiz = quiz(QuizSettings::default());
    quiz.questions.truncate(1);
    quiz.questions[0].kind = QuestionKind::Number {
        answer: Some(88.0),
        range: None,
        scoring: NumberScoring::Closest,
    };
    quiz.questions[0].bonus = Some(Bonus {
        text: "Who invented it?".to_owned(),
        accepted: vec!["Cristofori".to_owned()],
        percent: 25,
    });
    let mut g = Game::new("123456".to_owned(), quiz, random_below).unwrap();
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    let bonus = Some("cristofori".to_owned());
    g.answer_with_bonus(
        owls,
        Response::Number(85.0),
        false,
        bonus.clone(),
        t0 + secs(9),
    )
    .unwrap();
    g.answer_with_bonus(cats, Response::Number(50.0), false, bonus, t0)
        .unwrap();
    let scores: Vec<(u32, i32)> = g.standings().iter().map(|r| (r.team.id, r.score)).collect();
    assert_eq!(scores, [(owls, 1250), (cats, 250)]);
}

#[test]
fn a_number_range_reaches_phones_and_bounds_guesses() {
    let t0 = Instant::now();
    let range = crate::model::NumberRange {
        min: 0.0,
        max: 100.0,
    };
    let kind = QuestionKind::Number {
        answer: Some(42.0),
        range: Some(range),
        scoring: NumberScoring::Closest,
    };
    let mut g = open_game(kind, QuizSettings::default());
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    let PlayerPhase::Question { range: shown, .. } = g.player_view(owls, t0).unwrap().phase else {
        panic!("expected a question");
    };
    assert_eq!(shown, Some(range));
    assert!(g.answer(owls, Response::Number(101.0), false, t0).is_err());
    g.answer(owls, Response::Number(100.0), false, t0).unwrap();
}

#[test]
fn the_closest_number_wins_and_ties_share() {
    let t0 = Instant::now();
    let kind = QuestionKind::Number {
        answer: Some(330.0),
        range: None,
        scoring: NumberScoring::Closest,
    };
    let mut g = open_game(kind, QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let bats = join(&mut g, "Bats");
    g.next(t0).unwrap();
    assert!(g.answer(owls, text("330"), false, t0).is_err());
    assert!(
        g.answer(owls, Response::Number(f64::NAN), false, t0)
            .is_err()
    );
    g.answer(owls, Response::Number(300.0), false, t0).unwrap();
    g.answer(cats, Response::Number(360.0), false, t0 + secs(9))
        .unwrap();
    g.answer(bats, Response::Number(1000.0), false, t0).unwrap();
    let HostPhase::Closed {
        reveal: Some(Reveal::Number { answer, guesses }),
        ..
    } = g.host_view(t0).phase
    else {
        panic!("expected guesses on screen");
    };
    assert!((answer - 330.0).abs() < f64::EPSILON);
    let summary: Vec<(&str, bool)> = guesses.iter().map(|g| (g.team.as_str(), g.full)).collect();
    assert_eq!(
        summary,
        vec![("Owls", true), ("Cats", true), ("Bats", false)]
    );
    assert_eq!(g.player_view(cats, t0).unwrap().score, Some(1000));
    assert_eq!(g.player_view(bats, t0).unwrap().score, Some(0));
}

#[test]
fn numbers_can_score_by_distance_or_margin() {
    let t0 = Instant::now();
    let flat = QuizSettings {
        points: PointsMode::Flat,
        ..QuizSettings::default()
    };
    let number = |scoring| QuestionKind::Number {
        answer: Some(100.0),
        range: None,
        scoring,
    };
    let distance = NumberScoring::Distance {
        full_within: 10.0,
        none_beyond: 50.0,
    };
    let mut g = open_game(number(distance), flat.clone());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let bats = join(&mut g, "Bats");
    g.next(t0).unwrap();
    g.answer(owls, Response::Number(95.0), false, t0).unwrap();
    g.answer(cats, Response::Number(130.0), false, t0).unwrap();
    g.answer(bats, Response::Number(160.0), false, t0).unwrap();
    let score = |g: &Game, team| g.player_view(team, t0).unwrap().score;
    assert_eq!(score(&g, owls), Some(1000));
    assert_eq!(score(&g, cats), Some(500));
    assert_eq!(score(&g, bats), Some(0));

    let mut g = open_game(number(NumberScoring::Margin { margin: 0.0 }), flat);
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, Response::Number(100.0), false, t0).unwrap();
    g.answer(cats, Response::Number(101.0), false, t0).unwrap();
    assert_eq!(score(&g, owls), Some(1000));
    assert_eq!(score(&g, cats), Some(0));
}

#[test]
fn the_judge_gives_number_guesses_a_bonus_or_malus() {
    let t0 = Instant::now();
    let kind = QuestionKind::Number {
        answer: Some(330.0),
        range: None,
        scoring: NumberScoring::Closest,
    };
    let mut g = open_game(kind, QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, Response::Number(300.0), false, t0).unwrap();
    g.answer(cats, Response::Number(1000.0), false, t0).unwrap();
    let guesses = |g: &Game| {
        let HostPhase::Closed {
            reveal: Some(Reveal::Number { guesses, .. }),
            ..
        } = g.host_view(t0).phase
        else {
            panic!("expected guesses on screen");
        };
        guesses
    };
    let key = guesses(&g)[1].key.clone();
    // The grade is ignored: numbers are scored by distance.
    let malus = Judgement {
        grade: Grade::Wrong,
        bonus: -200,
    };
    g.judge(0, &key, malus, t0).unwrap();
    let cats_guess = &guesses(&g)[1];
    assert_eq!((cats_guess.bonus, cats_guess.points), (-200, -200));
    assert_eq!(g.player_view(cats, t0).unwrap().score, Some(-200));
    let bonus = Judgement {
        grade: Grade::Full,
        bonus: 100,
    };
    let owls_key = guesses(&g)[0].key.clone();
    g.judge(0, &owls_key, bonus, t0).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().score, Some(1100));
    // Back to no bonus, the verdict is the automatic one again.
    let none = Judgement {
        grade: Grade::Full,
        bonus: 0,
    };
    g.judge(0, &key, none, t0).unwrap();
    assert!(!g.judgements[0].contains_key(&key));
    assert!(g.judge(0, "330", bonus, t0).is_err());
}
