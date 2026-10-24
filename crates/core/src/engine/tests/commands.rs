//! The screens' commands, as the server and the browser game apply them.

use super::*;
use crate::protocol::{HostCommand, PlayerCommand};

/// Milliseconds left on the open question.
fn left_ms(g: &Game, t: Instant) -> u64 {
    match g.host_view(t).phase {
        HostPhase::Question { remaining_ms, .. } => remaining_ms,
        _ => panic!("expected an open question"),
    }
}

#[test]
fn toggling_pause_pauses_then_resumes_the_clock() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    join(&mut g, "Owls");
    // Nothing to pause in the lobby.
    assert!(g.apply_host(HostCommand::TogglePause, t0).is_err());
    assert!(!g.is_paused());

    g.apply_host(HostCommand::Next, t0).unwrap();
    g.apply_host(HostCommand::TogglePause, t0 + secs(2))
        .unwrap();
    assert!(g.is_paused());
    // The clock stands still while paused…
    assert_eq!(left_ms(&g, t0 + secs(30)), 8000);
    g.apply_host(HostCommand::TogglePause, t0 + secs(30))
        .unwrap();
    assert!(!g.is_paused());
    // …and runs again from where it stopped.
    assert_eq!(left_ms(&g, t0 + secs(31)), 7000);
}

#[test]
fn extra_time_is_capped() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    join(&mut g, "Owls");
    // No question is open in the lobby.
    assert!(g.apply_host(HostCommand::AddTime(10), t0).is_err());

    g.apply_host(HostCommand::Next, t0).unwrap();
    g.apply_host(HostCommand::AddTime(10), t0).unwrap();
    assert_eq!(left_ms(&g, t0), 20_000);
    g.apply_host(HostCommand::AddTime(MAX_EXTRA_SECS + 1000), t0)
        .unwrap();
    assert_eq!(left_ms(&g, t0), 20_000 + u64::from(MAX_EXTRA_SECS) * 1000);
}

#[test]
fn host_commands_move_through_the_quiz() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    // "Next" waits for a team.
    assert!(g.apply_host(HostCommand::Next, t0).is_err());
    assert_eq!(at(&g, t0), "L");
    let owls = join(&mut g, "Owls");
    g.apply_host(HostCommand::Next, t0).unwrap();
    assert_eq!(at(&g, t0), "Q0");
    g.apply_host(HostCommand::Next, t0).unwrap();
    assert_eq!(at(&g, t0), "C0");
    // Reopen, close again, then back out of the next question.
    g.apply_host(HostCommand::Reopen, t0).unwrap();
    assert_eq!(at(&g, t0), "Q0");
    g.apply_host(HostCommand::Next, t0).unwrap();
    g.apply_host(HostCommand::Next, t0).unwrap();
    assert_eq!(at(&g, t0), "Q1");
    g.apply_host(HostCommand::Back, t0).unwrap();
    assert_eq!(at(&g, t0), "C0");

    g.apply_host(
        HostCommand::RenameTeam {
            team: owls,
            name: "Barn owls".to_owned(),
        },
        t0,
    )
    .unwrap();
    assert_eq!(g.host_view(t0).teams[0].name, "Barn owls");
    g.apply_host(HostCommand::SetJoining(false), t0).unwrap();
    g.apply_host(HostCommand::SetPictures(false), t0).unwrap();
    let view = g.host_view(t0);
    assert!(!view.joining && !view.pictures);
    g.apply_host(HostCommand::RemoveTeam { team: owls }, t0)
        .unwrap();
    assert_eq!(g.team_count(), 0);
}

#[test]
fn a_phone_answers_through_its_command() {
    let t0 = Instant::now();
    let mut g = game(QuizSettings::default());
    let owls = join(&mut g, "Owls");
    let answer = |selected: &[usize]| PlayerCommand::Answer {
        response: pick(selected),
        joker: false,
        bonus: None,
    };
    // Too early.
    assert!(g.apply_player(owls, answer(&[0]), t0).is_err());
    g.apply_host(HostCommand::Next, t0).unwrap();
    g.apply_player(owls, answer(&[0]), t0 + secs(1)).unwrap();
    // The only team answered, so the question closed, and a second answer is refused.
    assert_eq!(at(&g, t0 + secs(1)), "C0");
    assert!(g.apply_player(owls, answer(&[1]), t0 + secs(1)).is_err());
    let score = g.player_view(owls, t0 + secs(1)).unwrap().score;
    assert!(score.is_some_and(|s| s > 0));
}
