//! Saving and restoring games.

use super::*;

#[test]
fn a_restored_game_resumes_where_it_was_paused() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let cats = join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), false, t0 + secs(2)).unwrap();
    let json = serde_json::to_string(&g.snapshot(t0 + secs(4))).unwrap();

    let later = t0 + secs(600);
    let mut g = Game::restore(serde_json::from_str(&json).unwrap(), later);
    assert_eq!(g.code(), "123456");
    assert_eq!(g.team_for_token("token-Owls"), Some(owls));
    assert!(g.is_paused());
    let HostPhase::Question {
        remaining_ms,
        answered,
        ..
    } = g.host_view(later + secs(30)).phase
    else {
        panic!("expected the open question");
    };
    assert_eq!((remaining_ms, answered), (6000, 1));
    // Phones reconnect; nobody counts as connected until then.
    assert!(g.host_view(later).teams.iter().all(|t| !t.connected));
    g.set_connected(cats, true);
    g.resume(later);
    g.answer(cats, pick(&[0]), false, later + secs(1)).unwrap();
    // Cats answered 5 s into the question, pauses excluded.
    assert!(matches!(g.host_view(later).phase, HostPhase::Closed { .. }));
    let rows = g.standings();
    assert_eq!(rows[0].score, 900);
    assert_eq!(rows[1].score, 750);
}

/// Saved games are read back after an upgrade: the snapshot format must not drift.
#[test]
fn snapshots_keep_their_saved_format() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings {
        joker: true,
        ..QuizSettings::default()
    });
    let owls = join(&mut g, "Owls");
    join(&mut g, "Cats");
    g.next(t0).unwrap();
    g.answer(owls, pick(&[0]), true, t0 + secs(2)).unwrap();
    g.pause(t0 + secs(3)).unwrap();
    let saved = serde_json::to_value(g.snapshot(t0 + secs(4))).unwrap();
    if std::env::var_os("QUAESITO_PRINT_SNAPSHOT").is_some() {
        println!("{}", serde_json::to_string_pretty(&saved).unwrap());
    }
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("snapshot-golden.json")).unwrap();
    assert_eq!(saved, golden);
    let restored = Game::restore(serde_json::from_value(golden.clone()).unwrap(), t0);
    assert_eq!(serde_json::to_value(restored.snapshot(t0)).unwrap(), golden);
}
