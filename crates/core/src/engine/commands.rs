//! The screens' commands, applied to the game: what the server runs for each command it
//! receives, and what a game running in the browser runs too.

use std::time::Duration;

use web_time::Instant;

use super::{Game, Refusal};
use crate::protocol::{HostCommand, PlayerCommand};

/// Most time the host can add in one go.
pub const MAX_EXTRA_SECS: u32 = 60;

impl Game {
    /// Applies a command of the host screen.
    ///
    /// # Errors
    /// As the command's own method: refusals leave the game unchanged (e.g. "next" without
    /// teams).
    pub fn apply_host(&mut self, command: HostCommand, now: Instant) -> Result<(), Refusal> {
        match command {
            HostCommand::Next => self.next(now),
            HostCommand::Judge {
                question,
                key,
                judgement,
            } => self.judge(question, &key, judgement, now),
            HostCommand::Back => self.back(now),
            HostCommand::Reopen => self.reopen(now),
            HostCommand::TogglePause => {
                if self.is_paused() {
                    self.resume(now);
                    Ok(())
                } else {
                    self.pause(now)
                }
            }
            HostCommand::AddTime(secs) => {
                self.add_time(Duration::from_secs(u64::from(secs.min(MAX_EXTRA_SECS))))
            }
            HostCommand::RenameTeam { team, name } => self.rename_team(team, &name),
            HostCommand::RemoveTeam { team } => self.remove_team(team, now),
            HostCommand::SetPictures(pictures) => {
                self.set_pictures(pictures);
                Ok(())
            }
            HostCommand::SetJoining(joining) => {
                self.set_joining(joining);
                Ok(())
            }
        }
    }

    /// Applies a command of `team`'s phone.
    ///
    /// # Errors
    /// As [`Game::answer_with_bonus`]: late or duplicate answers, invalid answers, an
    /// unavailable joker.
    pub fn apply_player(
        &mut self,
        team: u32,
        command: PlayerCommand,
        now: Instant,
    ) -> Result<(), Refusal> {
        match command {
            PlayerCommand::Answer {
                response,
                joker,
                bonus,
            } => self.answer_with_bonus(team, response, joker, bonus, now),
        }
    }
}
