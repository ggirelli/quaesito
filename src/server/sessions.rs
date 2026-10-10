//! Registry of live sessions, each wrapping a [`Game`] with change notification and timers.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

use tokio::{sync::watch, time::Instant};

use super::engine::{Game, Refusal};
use crate::{model::Quiz, protocol::SessionSummary};

/// A live session: the game plus a version counter bumped on every change.
pub struct Session {
    game: Mutex<Game>,
    version: watch::Sender<u64>,
}

impl Session {
    /// Reads the game.
    pub fn read<R>(&self, f: impl FnOnce(&Game) -> R) -> R {
        f(&self.lock())
    }

    /// Mutates the game and notifies every connected screen.
    pub fn update<R>(&self, f: impl FnOnce(&mut Game, std::time::Instant) -> R) -> R {
        let result = f(&mut self.lock(), std::time::Instant::now());
        self.version.send_modify(|v| *v += 1);
        result
    }

    /// Change notifications for this session.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.version.subscribe()
    }

    fn lock(&self) -> MutexGuard<'_, Game> {
        self.game.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// All live sessions by join code.
#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<String, Arc<Session>>>,
}

impl Sessions {
    /// Starts a session for a snapshot of `quiz` and returns its join code.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn create(&self, quiz: Quiz) -> Result<String, Refusal> {
        let mut map = self.lock();
        let code = loop {
            let candidate = format!("{:06}", uuid::Uuid::new_v4().as_u128() % 1_000_000);
            if !map.contains_key(&candidate) {
                break candidate;
            }
        };
        let session = Arc::new(Session {
            game: Mutex::new(Game::new(code.clone(), quiz)?),
            version: watch::channel(0).0,
        });
        tokio::spawn(run_timers(Arc::downgrade(&session), session.subscribe()));
        map.insert(code.clone(), session);
        Ok(code)
    }

    #[must_use]
    pub fn get(&self, code: &str) -> Option<Arc<Session>> {
        self.lock().get(code.trim()).cloned()
    }

    /// Ends a session. Connected screens keep their last view.
    pub fn remove(&self, code: &str) {
        self.lock().remove(code);
    }

    #[must_use]
    pub fn list(&self) -> Vec<SessionSummary> {
        let mut list: Vec<_> = self
            .lock()
            .values()
            .map(|s| {
                s.read(|g| SessionSummary {
                    code: g.code().to_owned(),
                    title: g.title().to_owned(),
                    teams: g.team_count(),
                    finished: g.is_finished(),
                })
            })
            .collect();
        list.sort_by(|a, b| a.code.cmp(&b.code));
        list
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<Session>>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Fires time-based transitions (question deadlines, auto-advance) until the session is dropped.
async fn run_timers(session: Weak<Session>, mut updates: watch::Receiver<u64>) {
    loop {
        let Some(deadline) = session.upgrade().map(|s| s.read(Game::next_deadline)) else {
            return;
        };
        let changed = match deadline {
            Some(deadline) => tokio::select! {
                () = tokio::time::sleep_until(Instant::from_std(deadline)) => {
                    let Some(s) = session.upgrade() else { return };
                    s.update(Game::tick);
                    continue;
                }
                changed = updates.changed() => changed,
            },
            None => updates.changed().await,
        };
        if changed.is_err() {
            return;
        }
    }
}
