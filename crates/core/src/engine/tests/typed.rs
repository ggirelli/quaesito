//! Typed answers and the judge.

use super::*;

#[test]
fn typed_answers_match_strictly_and_the_judge_amends() {
    let t0 = Instant::now();
    let kind = QuestionKind::Text {
        accepted: vec!["Leonardo da Vinci".to_owned(), "Leonardo".to_owned()],
    };
    let mut g = open_game(kind, QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    let bats = join(&mut g, "Bats");
    let pigs = join(&mut g, "Pigs");
    g.next(t0).unwrap();
    assert!(g.answer(owls, pick(&[0]), false, t0).is_err());
    assert!(g.answer(owls, text(" ?! "), false, t0).is_err());
    assert!(g.answer(owls, text(&"x".repeat(61)), false, t0).is_err());
    g.answer(owls, text("  léonardo DA vinci! "), false, t0)
        .unwrap();
    g.answer(cats, text("Leonardo da Vinchi"), false, t0)
        .unwrap();
    g.answer(bats, text("leonardo da vinchi"), false, t0)
        .unwrap();
    g.answer(pigs, text("Raphael"), false, t0).unwrap();

    let full = Judgement {
        grade: Grade::Full,
        bonus: 0,
    };
    let wrong = Judgement {
        grade: Grade::Wrong,
        bonus: 0,
    };
    let groups = text_groups(&g);
    assert_eq!(
        groups,
        vec![
            (
                "Leonardo da Vinchi".to_owned(),
                vec!["Cats".to_owned(), "Bats".to_owned()],
                wrong,
                false
            ),
            (
                "léonardo DA vinci!".to_owned(),
                vec!["Owls".to_owned()],
                full,
                false
            ),
            ("Raphael".to_owned(), vec!["Pigs".to_owned()], wrong, false),
        ]
    );
    assert_eq!(g.player_view(owls, t0).unwrap().score, Some(1000));
    assert_eq!(g.player_view(cats, t0).unwrap().score, Some(0));

    // Half points plus a bonus for the typo, a malus for Raphael.
    let half_bonus = Judgement {
        grade: Grade::Half,
        bonus: 100,
    };
    g.judge(0, "leonardo da vinchi", half_bonus, t0).unwrap();
    g.judge(
        0,
        "raphael",
        Judgement {
            grade: Grade::Wrong,
            bonus: -200,
        },
        t0,
    )
    .unwrap();
    assert_eq!(g.player_view(cats, t0).unwrap().score, Some(600));
    assert_eq!(g.player_view(bats, t0).unwrap().score, Some(600));
    assert_eq!(g.player_view(pigs, t0).unwrap().score, Some(-200));
    assert_eq!(text_groups(&g)[0].2, half_bonus);
    assert!(text_groups(&g)[0].3);

    // Setting the automatic verdict again clears the amendment.
    g.judge(0, "leonardo da vinchi", wrong, t0).unwrap();
    assert!(!text_groups(&g)[0].3);
    // Bonuses are capped; other questions cannot be judged.
    g.judge(
        0,
        "raphael",
        Judgement {
            grade: Grade::Wrong,
            bonus: -9000,
        },
        t0,
    )
    .unwrap();
    assert_eq!(g.player_view(pigs, t0).unwrap().score, Some(-1000));
    assert!(g.judge(1, "raphael", wrong, t0).is_err());
}

#[test]
fn judging_waits_for_the_review_when_answers_are_hidden() {
    let t0 = Instant::now();
    let kind = QuestionKind::Text {
        accepted: vec!["yes".to_owned()],
    };
    let mut g = open_game(
        kind,
        QuizSettings {
            reveal: RevealMode::AtEnd,
            ..QuizSettings::default()
        },
    );
    let owls = join(&mut g, "Owls");
    g.next(t0).unwrap();
    g.answer(owls, text("no"), false, t0).unwrap();
    let full = Judgement {
        grade: Grade::Full,
        bonus: 0,
    };
    assert!(g.judge(0, "no", full, t0).is_err());
    g.next(t0).unwrap(); // review
    g.judge(0, "no", full, t0).unwrap();
    assert_eq!(text_groups(&g)[0].2, full);
}
