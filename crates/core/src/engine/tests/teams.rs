//! Joining, naming, pictures and connections of teams.

use super::*;

#[test]
fn the_host_can_close_joining() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    g.set_joining(false);
    assert!(!g.host_view(t0).joining);
    assert!(g.join("Cats", "token-cats".to_owned()).is_err());
    assert_eq!(g.team_for_token("token-Owls"), Some(owls));
    let mut restored = Game::restore(g.snapshot(t0), t0);
    assert!(restored.join("Cats", "token-cats".to_owned()).is_err());
    g.set_joining(true);
    join(&mut g, "Cats");
}

#[test]
fn team_pictures_can_be_turned_off_and_backdrops_picked() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    g.set_avatar(owls, "owls.png".to_owned()).unwrap();
    g.set_backdrop(owls, Some(3)).unwrap();
    assert!(g.set_backdrop(owls, Some(BACKDROP_COUNT)).is_err());
    let team = g.player_view(owls, t0).unwrap().team;
    assert_eq!(
        (team.avatar.as_deref(), team.backdrop),
        (Some("owls.png"), Some(3))
    );

    // Both survive a restart.
    let restored = Game::restore(g.snapshot(t0), t0)
        .player_view(owls, t0)
        .unwrap()
        .team;
    assert_eq!(
        (restored.avatar, restored.backdrop),
        (team.avatar, team.backdrop)
    );

    // A ready-made picture replaces the upload, and an upload replaces it.
    assert!(g.set_mascot(owls, Some("kraken".to_owned())).is_err());
    g.set_mascot(owls, Some("owl".to_owned())).unwrap();
    let team = g.player_view(owls, t0).unwrap().team;
    assert_eq!((team.avatar, team.mascot.as_deref()), (None, Some("owl")));
    g.set_avatar(owls, "owls.png".to_owned()).unwrap();
    assert_eq!(g.player_view(owls, t0).unwrap().team.mascot, None);

    // Without pictures, uploads are refused and pictures dropped; backdrops stay.
    g.set_pictures(false);
    assert!(g.set_avatar(owls, "again.png".to_owned()).is_err());
    let view = g.player_view(owls, t0).unwrap();
    assert_eq!((view.team.avatar, view.team.backdrop), (None, Some(3)));
    assert!(matches!(
        view.phase,
        PlayerPhase::Lobby {
            pictures: false,
            ..
        }
    ));
    let restored = Game::restore(g.snapshot(t0), t0);
    assert!(!restored.pictures());
    // Ready-made pictures stay allowed, and survive a restart.
    g.set_mascot(owls, Some("fox".to_owned())).unwrap();
    let restored = Game::restore(g.snapshot(t0), t0)
        .player_view(owls, t0)
        .unwrap()
        .team;
    assert_eq!(restored.mascot.as_deref(), Some("fox"));
}

#[test]
fn invalid_quizzes_cannot_start_a_game() {
    let empty = Quiz::new("id".to_owned(), "Empty".to_owned());
    assert!(Game::new("1".to_owned(), empty, random_below).is_err());
}

#[test]
fn the_host_sets_how_many_teams_can_join() {
    let mut g = game(QuizSettings::default());
    assert_eq!(g.host_view(Instant::now()).max_teams, DEFAULT_MAX_TEAMS);
    assert!(g.set_max_teams(0).is_err());
    assert!(g.set_max_teams(MAX_TEAMS_CEILING + 1).is_err());
    g.set_max_teams(2).unwrap();
    join(&mut g, "Owls");
    join(&mut g, "Cats");
    assert_eq!(
        g.join("Bats", "token-bats".to_owned()),
        Err(Refusal("This game is full."))
    );
    g.set_max_teams(MAX_TEAMS_CEILING).unwrap();
    join(&mut g, "Bats");
    // The limit survives a restart.
    let t0 = Instant::now();
    let restored = Game::restore(g.snapshot(t0), t0);
    assert_eq!(restored.host_view(t0).max_teams, MAX_TEAMS_CEILING);
}

#[test]
fn join_validates_names() {
    let mut g = game(QuizSettings::default());
    assert!(g.join(" ", "t".to_owned()).is_err());
    assert!(g.join(&"x".repeat(25), "t".to_owned()).is_err());
    let id = g.join("  Owls ", "t1".to_owned()).unwrap();
    assert!(g.join("owls", "t2".to_owned()).is_err());
    assert_eq!(g.team_for_token("t1"), Some(id));
    assert_eq!(g.team_for_token("nope"), None);
}

#[test]
fn the_host_renames_and_removes_teams() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    assert!(g.rename_team(owls, "cats").is_err());
    assert!(g.rename_team(owls, " ").is_err());
    g.rename_team(owls, " Night Owls ").unwrap();
    g.rename_team(cats, "CATS").unwrap();
    assert_eq!(g.host_view(t0).teams[0].name, "Night Owls");

    // Removing the only team still to answer closes the question.
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    g.remove_team(cats, t0).unwrap();
    assert!(g.remove_team(cats, t0).is_err());
    assert!(g.player_view(cats, t0).is_none());
    assert_eq!(g.team_for_token("token-Cats"), None);
    assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));

    g.next(t0).unwrap();
    g.next(t0).unwrap();
    g.next(t0).unwrap(); // podium
    assert!(g.remove_team(owls, t0).is_err());
}

#[test]
fn team_names_cannot_be_spreadsheet_formulas() {
    let mut g = game(QuizSettings::default());
    join(&mut g, "=HYPERLINK(\"x\")");
    join(&mut g, "-1+2");
    let csv = g.results().to_csv();
    assert!(csv.contains(",\"'=HYPERLINK(\"\"x\"\")\","), "{csv}");
    assert!(csv.contains(",'-1+2,"), "{csv}");
}

#[test]
fn cannot_start_without_teams() {
    let mut g = game(QuizSettings::default());
    assert!(g.next(Instant::now()).is_err());
}

#[test]
fn disconnected_teams_do_not_hold_up_a_question() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.set_connected(cats, false);
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0).unwrap();
    assert!(matches!(g.host_view(t0).phase, HostPhase::Closed { .. }));
}
