//! The teams of a game: joining, names, pictures and connections.

use std::time::Instant;

use super::{Game, Phase, Refusal, Team};
use crate::protocol::{BACKDROP_COUNT, MASCOTS, MAX_NAME_CHARS};

impl Game {
    /// Registers a team and returns its id and reconnect token.
    ///
    /// # Errors
    /// Refuses empty, overlong or duplicate names, and joining a full or finished game.
    pub fn join(&mut self, name: &str, token: String) -> Result<u32, Refusal> {
        let name = self.check_name(name, None)?;
        if !self.joining {
            return Err(Refusal("This game is not taking new teams: ask the host."));
        }
        if self.teams.len() >= self.max_teams {
            return Err(Refusal("This game is full."));
        }
        if matches!(self.phase, Phase::Podium { .. })
            || matches!(self.phase, Phase::Review { index, .. } if self.part_end(index) == self.quiz.questions.len())
        {
            return Err(Refusal("This game is already over."));
        }
        let id = self.next_team_id;
        self.next_team_id += 1;
        self.teams.push(Team {
            id,
            name,
            avatar: None,
            mascot: None,
            backdrop: None,
            token,
            connections: 0,
            submissions: (0..self.quiz.questions.len()).map(|_| None).collect(),
            joker: None,
        });
        Ok(id)
    }

    /// Renames a team (the host fixing a typo or an unwelcome name).
    ///
    /// # Errors
    /// Refuses unknown teams and names [`Game::join`] would refuse.
    pub fn rename_team(&mut self, team: u32, name: &str) -> Result<(), Refusal> {
        let name = self.check_name(name, Some(team))?;
        self.team_mut(team).ok_or(Refusal("Unknown team."))?.name = name;
        Ok(())
    }

    /// Removes a team and its answers; its phone is sent back to the join form.
    ///
    /// # Errors
    /// Refuses unknown teams, and removals once the podium is being revealed.
    pub fn remove_team(&mut self, team: u32, now: Instant) -> Result<(), Refusal> {
        if matches!(self.phase, Phase::Podium { .. }) {
            return Err(Refusal("The podium is already being revealed."));
        }
        let before = self.teams.len();
        self.teams.retain(|t| t.id != team);
        if self.teams.len() == before {
            return Err(Refusal("Unknown team."));
        }
        // The removed team may have been the last one the question was waiting for.
        if let Phase::Question { index, .. } = self.phase
            && !self.teams.is_empty()
            && self.everyone_answered(index)
        {
            self.set_phase(Phase::Closed { index, since: now });
        }
        Ok(())
    }

    /// A trimmed, valid team name not used by another team than `except`.
    fn check_name(&self, name: &str, except: Option<u32>) -> Result<String, Refusal> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Refusal("Please pick a team name."));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(Refusal("That team name is too long."));
        }
        if self
            .teams
            .iter()
            .any(|t| Some(t.id) != except && t.name.to_lowercase() == name.to_lowercase())
        {
            return Err(Refusal("Another team already has that name."));
        }
        Ok(name.to_owned())
    }

    /// The team owning a reconnect token.
    #[must_use]
    pub fn team_for_token(&self, token: &str) -> Option<u32> {
        self.teams
            .iter()
            .find(|t| crate::constant_time_eq(t.token.as_bytes(), token.as_bytes()))
            .map(|t| t.id)
    }

    /// Lets teams upload a picture, or not (they still pick a background for their initial).
    /// Turning pictures off also drops those already uploaded.
    pub fn set_pictures(&mut self, pictures: bool) {
        self.pictures = pictures;
        if !pictures {
            for team in &mut self.teams {
                team.avatar = None;
            }
        }
    }

    /// Opens or closes the game to new teams. Teams already in stay, and their phones can still
    /// reconnect.
    pub fn set_joining(&mut self, joining: bool) {
        self.joining = joining;
    }

    /// Whether the game is still waiting in the lobby (or on the Wi-Fi slide before it).
    #[must_use]
    pub fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Network | Phase::Lobby)
    }

    /// Whether teams may upload a picture.
    #[must_use]
    pub fn pictures(&self) -> bool {
        self.pictures
    }

    /// Sets a team's avatar.
    ///
    /// # Errors
    /// Refuses when the game takes no pictures.
    pub fn set_avatar(&mut self, team: u32, avatar: String) -> Result<(), Refusal> {
        if !self.pictures {
            return Err(Refusal("This game does not take team pictures."));
        }
        if let Some(t) = self.team_mut(team) {
            t.avatar = Some(avatar);
            t.mascot = None;
        }
        Ok(())
    }

    /// Gives a team a ready-made picture (`None` for none), in place of an uploaded one. Allowed
    /// even when the game takes no uploads.
    ///
    /// # Errors
    /// Refuses a picture that does not exist.
    pub fn set_mascot(&mut self, team: u32, mascot: Option<String>) -> Result<(), Refusal> {
        if mascot.as_deref().is_some_and(|m| !MASCOTS.contains(&m)) {
            return Err(Refusal("There is no such picture."));
        }
        if let Some(t) = self.team_mut(team) {
            if mascot.is_some() {
                t.avatar = None;
            }
            t.mascot = mascot;
        }
        Ok(())
    }

    /// Sets the background of a team's picture (`None` for the default).
    ///
    /// # Errors
    /// Refuses a background that does not exist.
    pub fn set_backdrop(&mut self, team: u32, backdrop: Option<u8>) -> Result<(), Refusal> {
        if backdrop.is_some_and(|b| b >= BACKDROP_COUNT) {
            return Err(Refusal("There is no such background."));
        }
        if let Some(t) = self.team_mut(team) {
            t.backdrop = backdrop;
        }
        Ok(())
    }

    /// Tracks a team leader's phone connecting (`true`) or disconnecting.
    pub fn set_connected(&mut self, team: u32, connected: bool) {
        if let Some(t) = self.team_mut(team) {
            if connected {
                t.connections += 1;
            } else {
                t.connections = t.connections.saturating_sub(1);
            }
        }
    }
}
