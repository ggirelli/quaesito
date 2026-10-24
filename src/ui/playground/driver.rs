//! A game with one team, run in the browser: the engine, its timers, and the [`Live`] games its
//! big screen and phone play.

use std::time::Duration;

use dioxus::prelude::*;

use crate::{
    engine::{Game, Instant, Refusal},
    model::Quiz,
    protocol::{HostCommand, HostView, PlayerCommand, PlayerView},
    ui::{client, live::Live},
};

/// The only team's name.
const TEAM: &str = "You";

/// Longest wait between two looks at the game's timers: commands such as pausing or adding time
/// move the next deadline, and the timer sees it at its next look.
const TIMER_CHECK: Duration = Duration::from_millis(250);

/// The game, and its only team, already in and connected as if its phone had joined.
pub struct LocalGame {
    game: Game,
    team: u32,
}

impl LocalGame {
    /// Starts `quiz` in the lobby. `random_below(n)` must return a number in `0..n`.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn start(quiz: Quiz, random_below: impl FnMut(usize) -> usize) -> Result<Self, Refusal> {
        let mut game = Game::new(String::new(), quiz, random_below)?;
        let team = game.join(TEAM, String::new())?;
        game.set_connected(team, true);
        Ok(Self { game, team })
    }

    #[must_use]
    pub fn host_view(&self, now: Instant) -> HostView {
        self.game.host_view(now)
    }

    #[must_use]
    pub fn player_view(&self, now: Instant) -> Option<PlayerView> {
        self.game.player_view(self.team, now)
    }

    /// Applies a command of the big screen; refusals leave the game unchanged.
    pub fn host(&mut self, command: HostCommand, now: Instant) {
        let _ = self.game.apply_host(command, now);
    }

    /// Applies a command of the phone; refusals (such as late answers) leave the game unchanged.
    pub fn player(&mut self, command: PlayerCommand, now: Instant) {
        let _ = self.game.apply_player(self.team, command, now);
    }

    /// How long to wait before looking at the timers again.
    #[must_use]
    pub fn wait(&self, now: Instant) -> Duration {
        self.game.next_deadline().map_or(TIMER_CHECK, |deadline| {
            deadline.saturating_duration_since(now).min(TIMER_CHECK)
        })
    }

    /// Whether a timer is due at `now`.
    #[must_use]
    pub fn due(&self, now: Instant) -> bool {
        self.game
            .next_deadline()
            .is_some_and(|deadline| deadline <= now)
    }

    /// Applies time-based transitions. Returns whether anything changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        self.game.tick(now)
    }
}

/// The big screen's and the phone's [`Live`] games, fed by `game`, whose timers run while the
/// calling component is mounted.
pub fn use_local_live(
    mut game: Signal<LocalGame>,
) -> (Live<HostCommand, HostView>, Live<PlayerCommand, PlayerView>) {
    let mut host_view = use_signal(|| Some(game.peek().host_view(Instant::now())));
    let mut player_view = use_signal(|| game.peek().player_view(Instant::now()));
    let mut received_at = use_signal(client::now_ms);
    let connected = use_signal(|| true);
    // Shows the game as it is now on both screens.
    let publish = use_callback(move |()| {
        let now = Instant::now();
        let game = game.peek();
        host_view.set(Some(game.host_view(now)));
        player_view.set(game.player_view(now));
        received_at.set(client::now_ms());
    });
    let host_commands = use_callback(move |command: HostCommand| {
        game.write().host(command, Instant::now());
        publish.call(());
    });
    let player_commands = use_callback(move |command: PlayerCommand| {
        game.write().player(command, Instant::now());
        publish.call(());
    });
    use_future(move || async move {
        loop {
            let wait = game.peek().wait(Instant::now());
            #[allow(
                clippy::cast_possible_truncation,
                reason = "the wait is at most TIMER_CHECK"
            )]
            client::sleep_ms(wait.as_millis() as u32).await;
            let now = Instant::now();
            if game.peek().due(now) && game.write().tick(now) {
                publish.call(());
            }
        }
    });
    (
        Live::new(host_view, received_at, connected, host_commands),
        Live::new(player_view, received_at, connected, player_commands),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Answer, Question},
        protocol::{HostPhase, PlayerPhase, Response},
    };

    fn quiz() -> Quiz {
        Quiz {
            questions: vec![Question {
                text: "Q".to_owned(),
                time_limit_secs: Some(5),
                answers: vec![
                    Answer {
                        text: "A".to_owned(),
                        correct: true,
                        image: None,
                    },
                    Answer {
                        text: "B".to_owned(),
                        correct: false,
                        image: None,
                    },
                ],
                ..Question::default()
            }],
            ..Quiz::new("q".to_owned(), "Local".to_owned())
        }
    }

    #[test]
    fn the_team_is_in_and_can_start() {
        let t0 = Instant::now();
        let mut game = LocalGame::start(quiz(), |_| 0).unwrap();
        let view = game.host_view(t0);
        assert_eq!(view.teams.len(), 1);
        assert!(view.teams[0].connected);
        game.host(HostCommand::Next, t0);
        assert!(matches!(
            game.host_view(t0).phase,
            HostPhase::Question { .. }
        ));
    }

    #[test]
    fn an_answer_closes_the_question_and_scores() {
        let t0 = Instant::now();
        let mut game = LocalGame::start(quiz(), |_| 0).unwrap();
        game.host(HostCommand::Next, t0);
        game.player(
            PlayerCommand::Answer {
                response: Response::Choice(vec![0]),
                joker: false,
                bonus: None,
            },
            t0,
        );
        assert!(matches!(game.host_view(t0).phase, HostPhase::Closed { .. }));
        let view = game.player_view(t0).unwrap();
        assert!(matches!(view.phase, PlayerPhase::Closed { .. }));
        assert!(view.score.is_some_and(|s| s > 0));
    }

    #[test]
    fn the_timer_closes_an_unanswered_question() {
        let t0 = Instant::now();
        let mut game = LocalGame::start(quiz(), |_| 0).unwrap();
        // No deadline in the lobby: the timer only looks again later.
        assert_eq!(game.wait(t0), TIMER_CHECK);
        game.host(HostCommand::Next, t0);
        assert!(!game.due(t0 + Duration::from_secs(4)));
        let late = t0 + Duration::from_secs(5);
        assert!(game.due(late));
        assert_eq!(game.wait(late), Duration::ZERO);
        assert!(game.tick(late));
        assert!(matches!(
            game.host_view(late).phase,
            HostPhase::Closed { .. }
        ));
    }

    #[test]
    fn unplayable_quizzes_are_refused() {
        let empty = Quiz::new("q".to_owned(), "Empty".to_owned());
        assert!(LocalGame::start(empty, |_| 0).is_err());
    }
}
