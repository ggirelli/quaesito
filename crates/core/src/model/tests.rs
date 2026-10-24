use super::*;

fn answer(text: &str, correct: bool) -> Answer {
    Answer {
        text: text.to_owned(),
        correct,
        image: None,
    }
}

fn question() -> Question {
    Question {
        text: "Which planet has the most moons?".to_owned(),
        kind: QuestionKind::Choice,
        image: None,
        image_reveal: ImageReveal::Shown,
        image_hold_secs: 0,
        audio: None,
        time_limit_secs: None,
        multiplier: None,
        answers: vec![answer("Jupiter", false), answer("Saturn", true)],
        bonus: None,
        passage: String::new(),
        explanation: String::new(),
        explanation_image: None,
        explanation_audio: None,
        category: String::new(),
        topic: String::new(),
        library: None,
    }
}

fn quiz() -> Quiz {
    Quiz {
        questions: vec![question()],
        ..Quiz::new("q1".to_owned(), "Friday".to_owned())
    }
}

#[test]
fn valid_quiz_has_no_issues() {
    assert_eq!(quiz().validate(), Vec::new());
}

#[test]
fn empty_quiz_is_invalid() {
    let issues = Quiz::new("q1".to_owned(), " ".to_owned()).validate();
    assert_eq!(issues.len(), 2);
    assert!(issues.iter().all(|i| i.question.is_none()));
}

#[test]
fn question_problems_point_at_the_question() {
    let mut quiz = quiz();
    quiz.questions.push(Question {
        answers: vec![answer("", false)],
        time_limit_secs: Some(1),
        ..Question::default()
    });
    let issues = quiz.validate();
    assert_eq!(issues.len(), 5);
    assert!(issues.iter().all(|i| i.question == Some(1)));
    assert!(issues[0].to_string().starts_with("Question 2: "));
}

#[test]
fn bonus_questions_need_text_and_an_answer_on_closed_questions_only() {
    let mut question = Question {
        text: "Q".to_owned(),
        kind: QuestionKind::TrueFalse { answer: true },
        bonus: Some(Bonus::default()),
        ..Question::default()
    };
    assert_eq!(question.problems().len(), 2);
    question.bonus = Some(Bonus {
        text: "Why?".to_owned(),
        accepted: vec!["Because".to_owned()],
        percent: 50,
    });
    assert_eq!(question.problems(), Vec::<String>::new());
    // Typed questions cannot carry one: it is ignored there.
    question.kind = QuestionKind::Text {
        accepted: vec!["A".to_owned()],
    };
    assert!(!question.takes_bonus());
    // Saved quizzes without a bonus stay as they were.
    question.bonus = None;
    assert!(!serde_json::to_string(&question).unwrap().contains("bonus"));
}

#[test]
fn results_slides_need_answers_revealed_at_the_end() {
    let mut quiz = Quiz::new("id".to_owned(), "Quiz".to_owned());
    quiz.questions.push(Question {
        text: "Q".to_owned(),
        answers: vec![
            Answer {
                text: "a".to_owned(),
                correct: true,
                image: None,
            },
            Answer {
                text: "b".to_owned(),
                correct: false,
                image: None,
            },
        ],
        ..Question::default()
    });
    quiz.questions.push(Question::results_slide());
    quiz.settings.reveal = RevealMode::AfterEachQuestion;
    let issues = quiz.validate();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].question, Some(1));
    quiz.settings.reveal = RevealMode::AtEnd;
    assert_eq!(quiz.validate(), Vec::new());
    // Plain slides are saved without the flag.
    let plain = serde_json::to_string(&Question::slide()).unwrap();
    assert!(!plain.contains("results"));
    assert!(Question::results_slide().is_slide());
}

#[test]
fn slides_are_numbered_apart_and_need_content() {
    let mut quiz = quiz();
    quiz.questions.insert(0, Question::slide());
    quiz.questions.push(question());
    assert_eq!(quiz.question_count(), 2);
    assert_eq!(quiz.label(0), "Slide 1");
    assert_eq!(quiz.label(2), "Question 2");
    let issues = quiz.validate();
    assert_eq!(issues.len(), 1);
    assert!(issues[0].to_string().starts_with("Slide 1: "));
    quiz.questions[0].kind = QuestionKind::Slide {
        title: "Round 2".to_owned(),
        results: false,
    };
    assert_eq!(quiz.validate(), Vec::new());
    // Slides alone do not make a quiz.
    quiz.questions.retain(Question::is_slide);
    assert_eq!(quiz.validate().len(), 1);
}

#[test]
fn shuffling_keeps_questions_between_their_slides() {
    let named = |text: &str| Question {
        text: text.to_owned(),
        ..question()
    };
    let slide = |title: &str| Question {
        kind: QuestionKind::Slide {
            title: title.to_owned(),
            results: false,
        },
        ..Question::default()
    };
    let mut quiz = quiz();
    quiz.questions = vec![
        named("a"),
        named("b"),
        named("c"),
        slide("Round 2"),
        named("d"),
        slide("Round 3"),
        named("e"),
        named("f"),
    ];
    assert!(quiz.can_shuffle());
    // Always picking 0 rotates each run.
    quiz.shuffle_questions(|_| 0);
    let order: Vec<&str> = quiz
        .questions
        .iter()
        .map(|q| match &q.kind {
            QuestionKind::Slide { title, .. } => title.as_str(),
            _ => q.text.as_str(),
        })
        .collect();
    assert_eq!(order, ["b", "c", "a", "Round 2", "d", "Round 3", "f", "e"]);

    quiz.questions = vec![named("a"), slide("Round 2"), named("b")];
    assert!(!quiz.can_shuffle());
}

#[test]
fn shuffles_multiple_choice_answers_only() {
    let mut choice = question();
    choice.answers.push(answer("Mars", false));
    let mut text = question();
    text.kind = QuestionKind::Text {
        accepted: vec!["Saturn".to_owned()],
    };
    let mut quiz = quiz();
    quiz.questions = vec![choice, text.clone()];
    assert!(quiz.can_shuffle_answers());
    // Always picking 0 rotates the answers.
    quiz.shuffle_answers(|_| 0);
    let answers: Vec<(&str, bool)> = quiz.questions[0]
        .answers
        .iter()
        .map(|a| (a.text.as_str(), a.correct))
        .collect();
    assert_eq!(
        answers,
        [("Saturn", true), ("Mars", false), ("Jupiter", false)]
    );
    assert_eq!(quiz.questions[1], text);

    quiz.questions = vec![text];
    assert!(!quiz.can_shuffle_answers());
}

#[test]
fn answers_need_a_text_or_a_picture() {
    let mut q = question();
    q.answers[0].text = " ".to_owned();
    assert_eq!(
        q.problems(),
        vec!["Every answer needs a text or a picture."]
    );
    q.answers[0].image = Some("flag.jpg".to_owned());
    assert_eq!(q.problems(), Vec::<String>::new());
}

#[test]
fn order_questions_need_two_to_six_items() {
    let mut q = Question {
        text: "Oldest first".to_owned(),
        kind: QuestionKind::Order {
            items: vec!["Rome".to_owned()],
        },
        ..Question::default()
    };
    assert_eq!(q.problems().len(), 1);
    q.kind = QuestionKind::Order {
        items: vec!["Rome".to_owned(), String::new()],
    };
    assert_eq!(q.problems(), vec!["Every item needs a text."]);
    q.kind = QuestionKind::Order {
        items: vec!["Rome".to_owned(), "Paris".to_owned()],
    };
    assert_eq!(q.problems(), Vec::<String>::new());
}

fn pair(left: &str, right: &str) -> Pair {
    Pair {
        left: PairItem {
            text: left.to_owned(),
            image: None,
        },
        right: PairItem {
            text: right.to_owned(),
            image: None,
        },
    }
}

#[test]
fn pairs_questions_need_two_to_six_full_and_distinct_pairs() {
    let mut q = Question {
        text: "Match the capitals".to_owned(),
        kind: QuestionKind::Pairs {
            pairs: vec![pair("France", "Paris")],
        },
        ..Question::default()
    };
    assert_eq!(q.problems(), vec!["Make between 2 and 6 pairs."]);
    q.kind = QuestionKind::Pairs {
        pairs: vec![pair("France", "Paris"), pair("Italy", " ")],
    };
    assert_eq!(
        q.problems(),
        vec!["Both sides of every pair need a text or a picture."]
    );
    q.kind = QuestionKind::Pairs {
        pairs: vec![pair("France", "Paris"), pair("Italy", "paris ")],
    };
    assert_eq!(
        q.problems(),
        vec!["Two items in a column are the same: teams could not tell them apart."]
    );
    let mut pairs = vec![pair("France", "Paris"), pair("Italy", "")];
    pairs[1].right.image = Some("rome.jpg".to_owned());
    q.kind = QuestionKind::Pairs { pairs };
    assert_eq!(q.problems(), Vec::<String>::new());
    assert!(q.takes_bonus());
}

#[test]
fn media_lists_and_renames_every_upload() {
    let mut quiz = quiz();
    quiz.questions[0].image = Some("q.jpg".to_owned());
    quiz.questions[0].audio = Some("s.mp3".to_owned());
    quiz.questions[0].answers[1].image = Some("a.jpg".to_owned());
    quiz.questions[0].explanation_image = Some("e.mp4".to_owned());
    quiz.questions[0].explanation_audio = Some("f.mp3".to_owned());
    quiz.questions.push(Question {
        image: Some("q.jpg".to_owned()),
        ..Question::slide()
    });
    let mut pairs = vec![pair("Paris", ""), pair("", "Rome")];
    pairs[0].right.image = Some("r.jpg".to_owned());
    pairs[1].left.image = Some("l.jpg".to_owned());
    quiz.questions.push(Question {
        kind: QuestionKind::Pairs { pairs },
        ..Question::default()
    });
    assert_eq!(
        quiz.media(),
        [
            "a.jpg", "e.mp4", "f.mp3", "l.jpg", "q.jpg", "r.jpg", "s.mp3"
        ]
    );
    quiz.rename_media(|name| (name != "s.mp3").then(|| format!("new-{name}")));
    assert_eq!(
        quiz.media(),
        [
            "new-a.jpg",
            "new-e.mp4",
            "new-f.mp3",
            "new-l.jpg",
            "new-q.jpg",
            "new-r.jpg"
        ]
    );
    assert_eq!(quiz.questions[0].audio, None);
}

#[test]
fn the_picture_hold_must_leave_time_for_the_reveal() {
    let mut quiz = quiz();
    quiz.questions[0].image = Some("p.jpg".to_owned());
    quiz.questions[0].image_reveal = ImageReveal::Tiles;
    quiz.questions[0].image_hold_secs = 20; // the default limit
    assert_eq!(quiz.validate().len(), 1);
    quiz.questions[0].image_hold_secs = 5;
    assert_eq!(quiz.validate(), Vec::new());
    // Without a gradual reveal the hold is ignored.
    quiz.questions[0].image_hold_secs = 60;
    quiz.questions[0].image_reveal = ImageReveal::Shown;
    assert_eq!(quiz.validate(), Vec::new());
}

#[test]
fn image_reveal_reads_the_former_blur_flag() {
    let reveal = |json: &str| {
        serde_json::from_str::<Question>(&format!(r#"{{"text":"Q",{json}}}"#))
            .unwrap()
            .image_reveal
    };
    assert_eq!(reveal(r#""blur_image":true"#), ImageReveal::Zoom);
    assert_eq!(reveal(r#""blur_image":false"#), ImageReveal::Shown);
    assert_eq!(reveal(r#""image_reveal":"Tiles""#), ImageReveal::Tiles);
    assert_eq!(reveal(r#""answers":[]"#), ImageReveal::Shown);
}

#[test]
fn true_false_picks_one_of_two() {
    let mut q = Question {
        text: "The Moon is a planet.".to_owned(),
        kind: QuestionKind::TrueFalse { answer: false },
        ..Question::default()
    };
    assert_eq!(q.problems(), Vec::<String>::new());
    assert_eq!(q.choices(), ["True", "False"]);
    assert_eq!(q.correct_indexes(), [1]);
    q.kind = QuestionKind::TrueFalse { answer: true };
    assert_eq!(q.correct_indexes(), [0]);
    assert!(!q.is_multi());
}

#[test]
fn parts_need_accepted_answers_and_count_their_pictures() {
    let part = |label: &str, accepted: &str| Part {
        label: label.to_owned(),
        accepted: vec![accepted.to_owned()],
        image: None,
    };
    let mut quiz = quiz();
    quiz.questions[0].kind = QuestionKind::Parts {
        parts: vec![part("Title", "Yesterday")],
    };
    assert_eq!(quiz.validate().len(), 1);
    quiz.questions[0].kind = QuestionKind::Parts {
        parts: vec![
            part("Title", "Yesterday"),
            Part {
                image: Some("poster.jpg".to_owned()),
                ..part("Artist", " ")
            },
        ],
    };
    assert_eq!(quiz.validate().len(), 1);
    assert_eq!(quiz.media(), ["poster.jpg"]);
    quiz.rename_media(|_| Some("new.jpg".to_owned()));
    assert_eq!(quiz.media(), ["new.jpg"]);
}

#[test]
fn a_new_picture_clears_the_pin_spot() {
    let mut q = Question {
        image: Some("old.jpg".to_owned()),
        kind: QuestionKind::Pin {
            answer: Some(Point { x: 0.5, y: 0.5 }),
            aspect: 0.75,
            tolerance: PinTolerance::Normal,
        },
        ..Question::default()
    };
    q.set_image(Some("old.jpg".to_owned()));
    assert!(matches!(
        q.kind,
        QuestionKind::Pin {
            answer: Some(_),
            ..
        }
    ));
    q.set_image(Some("new.jpg".to_owned()));
    assert!(matches!(
        q.kind,
        QuestionKind::Pin { answer: None, aspect, .. } if aspect.abs() < f64::EPSILON
    ));
}

#[test]
fn pins_need_a_picture_and_a_spot() {
    let mut q = Question {
        text: "Where is Rome?".to_owned(),
        kind: QuestionKind::Pin {
            answer: None,
            aspect: 0.75,
            tolerance: PinTolerance::Normal,
        },
        ..Question::default()
    };
    assert_eq!(q.problems().len(), 2);
    q.image = Some("italy.jpg".to_owned());
    q.kind = QuestionKind::Pin {
        answer: Some(Point { x: 0.45, y: 0.5 }),
        aspect: 0.75,
        tolerance: PinTolerance::Normal,
    };
    assert_eq!(q.problems(), Vec::<String>::new());
    q.image_reveal = ImageReveal::Tiles;
    assert!(!q.reveals_image());
    // Distances count the picture's shape: a quarter of the height of a 4:3 picture is
    // 0.1875 of its width.
    let a = Point { x: 0.5, y: 0.25 };
    let b = Point { x: 0.5, y: 0.5 };
    assert!((a.distance(b, 0.75) - 0.1875).abs() < 1e-9);
}

#[test]
fn time_limit_falls_back_to_quiz_default() {
    let settings = QuizSettings::default();
    let mut q = question();
    assert_eq!(q.time_limit_secs(&settings), 20);
    q.time_limit_secs = Some(45);
    assert_eq!(q.time_limit_secs(&settings), 45);
}

#[test]
fn the_network_slide_needs_a_name_and_escapes_its_qr_code() {
    let mut network = NetworkSlide {
        name: "Pub; \"Guest\"".to_owned(),
        ..NetworkSlide::default()
    };
    assert_eq!(network.problem(), None);
    network.enabled = true;
    assert_eq!(network.qr_text(), r#"WIFI:T:nopass;S:Pub\; \"Guest\";;"#);
    network.password = r"a:b,c\d".to_owned();
    assert_eq!(
        network.qr_text(),
        r#"WIFI:T:WPA;S:Pub\; \"Guest\";P:a\:b\,c\\d;;"#
    );
    assert_eq!(network.problem(), None);
    network.name = " ".to_owned();
    assert!(network.problem().is_some());
    network.name = "x".repeat(MAX_NETWORK_NAME_BYTES + 1);
    assert!(network.problem().is_some());
}

#[test]
fn correct_indexes_and_multi() {
    let mut q = question();
    assert_eq!(q.correct_indexes(), vec![1]);
    assert!(!q.is_multi());
    q.answers.push(answer("Uranus", true));
    assert_eq!(q.correct_indexes(), vec![1, 2]);
    assert!(q.is_multi());
}

#[test]
fn text_and_number_questions_need_their_answer() {
    let mut q = Question {
        text: "Who painted the Mona Lisa?".to_owned(),
        kind: QuestionKind::Text {
            accepted: vec![" ".to_owned()],
        },
        ..Question::default()
    };
    assert_eq!(q.problems(), vec!["Add at least one accepted answer."]);
    q.kind = QuestionKind::Text {
        accepted: vec!["Leonardo".to_owned()],
    };
    assert_eq!(q.problems(), Vec::<String>::new());
    q.kind = QuestionKind::Number {
        answer: None,
        range: None,
        scoring: NumberScoring::Closest,
    };
    assert_eq!(q.problems(), vec!["Enter the correct number."]);
    q.kind = QuestionKind::Number {
        answer: Some(330.0),
        range: None,
        scoring: NumberScoring::Closest,
    };
    assert_eq!(q.problems(), Vec::<String>::new());
    let range = |min, max| Some(NumberRange { min, max });
    q.kind = QuestionKind::Number {
        answer: Some(330.0),
        range: range(0.0, 1000.0),
        scoring: NumberScoring::Closest,
    };
    assert_eq!(q.problems(), Vec::<String>::new());
    q.kind = QuestionKind::Number {
        answer: Some(330.0),
        range: range(0.0, 100.0),
        scoring: NumberScoring::Closest,
    };
    assert_eq!(
        q.problems(),
        vec!["The correct number must be within the range."]
    );
    q.kind = QuestionKind::Number {
        answer: Some(330.0),
        range: range(10.0, 10.0),
        scoring: NumberScoring::Closest,
    };
    assert_eq!(
        q.problems(),
        vec!["The range must go from a smaller to a larger number."]
    );
    let scored = |scoring| QuestionKind::Number {
        answer: Some(330.0),
        range: None,
        scoring,
    };
    q.kind = scored(NumberScoring::Margin { margin: 0.0 });
    assert_eq!(q.problems(), Vec::<String>::new());
    q.kind = scored(NumberScoring::Margin { margin: -1.0 });
    assert_eq!(
        q.problems(),
        vec!["The margin must be zero or a positive number."]
    );
    q.kind = scored(NumberScoring::Distance {
        full_within: 10.0,
        none_beyond: 50.0,
    });
    assert_eq!(q.problems(), Vec::<String>::new());
    q.kind = scored(NumberScoring::Distance {
        full_within: 50.0,
        none_beyond: 50.0,
    });
    assert_eq!(
        q.problems(),
        vec!["The distance for full points must be smaller than the one for no points."]
    );
    // Leftover choices of a converted question do not count.
    q.answers = vec![answer("a", true), answer("b", true)];
    assert!(!q.is_multi());
    assert_eq!(q.correct_indexes(), Vec::<usize>::new());
}

#[test]
fn missing_fields_deserialize_to_defaults() {
    let quiz: Quiz = serde_json::from_str(
        r#"{"id":"x","title":"T","questions":[{"text":"Q","answers":[{"text":"A"}]}]}"#,
    )
    .unwrap();
    assert_eq!(quiz.settings, QuizSettings::default());
    assert_eq!(quiz.questions[0].kind, QuestionKind::Choice);
    assert!(!quiz.questions[0].answers[0].correct);
}
