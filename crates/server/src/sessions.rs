//! Registry of live sessions, each wrapping a [`Game`] with change notification and timers.
//!
//! Every game is saved to `<data>/games/<code>.json` shortly after each change, and the saved
//! games are brought back (paused) when the server starts.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Context;
use quaesito_core::{
    engine::{Game, Refusal, Snapshot},
    model::Quiz,
    protocol::SessionSummary,
};
use tokio::{
    sync::{oneshot, watch},
    time::Instant,
};

/// How long changes settle before a game is saved.
const SAVE_DELAY: Duration = Duration::from_millis(300);

/// Most connections a team's phones keep at once; a new one closes the oldest. A phone has one,
/// plus perhaps a dead one the server has not noticed yet after it changed networks.
const MAX_PHONE_CONNECTIONS: usize = 3;

/// Least time between two actions of a team (answers, renames, pictures): each one sends every
/// screen a new view, so a phone sending them in a loop would keep the server busy for everyone.
/// A team acts a few times per question at most.
const TEAM_ACTION_GAP: Duration = Duration::from_millis(100);

/// A phone connection: its number, and what closes it when dropped.
type Phone = (u64, oneshot::Sender<()>);

/// A live session: the game plus a version counter bumped on every change.
pub struct Session {
    game: Mutex<Game>,
    version: watch::Sender<u64>,
    /// Set when the session is ended, so it is no longer saved.
    ended: AtomicBool,
    /// An admin's test game: never saved.
    test: bool,
    /// Per team, its open phone connections, oldest first: dropping one closes it.
    phones: Mutex<HashMap<u32, Vec<Phone>>>,
    /// Numbers phone connections.
    next_phone: AtomicU64,
    /// Per team, when its latest action may run.
    actions: Mutex<HashMap<u32, Instant>>,
}

impl Session {
    fn new(game: Game, test: bool) -> Self {
        Self {
            game: Mutex::new(game),
            version: watch::channel(0).0,
            ended: AtomicBool::new(false),
            test,
            phones: Mutex::default(),
            next_phone: AtomicU64::new(0),
            actions: Mutex::default(),
        }
    }

    /// Registers a phone connection of `team`, closing the team's oldest if it has too many.
    /// Returns the connection's number, for [`Session::disconnect_phone`], and what tells it to
    /// close.
    pub fn connect_phone(&self, team: u32) -> (u64, oneshot::Receiver<()>) {
        let id = self.next_phone.fetch_add(1, Ordering::Relaxed);
        let (close, closed) = oneshot::channel();
        let mut phones = self.phones.lock().unwrap_or_else(PoisonError::into_inner);
        let open = phones.entry(team).or_default();
        open.push((id, close));
        let excess = open.len().saturating_sub(MAX_PHONE_CONNECTIONS);
        open.drain(..excess);
        (id, closed)
    }

    /// Forgets a phone connection registered with [`Session::connect_phone`].
    pub fn disconnect_phone(&self, team: u32, id: u64) {
        let mut phones = self.phones.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(open) = phones.get_mut(&team) {
            open.retain(|(other, _)| *other != id);
        }
    }

    /// Waits until `team` may act again, so its actions run at most every `TEAM_ACTION_GAP`.
    pub async fn pace(&self, team: u32) {
        let slot = {
            let mut actions = self.actions.lock().unwrap_or_else(PoisonError::into_inner);
            let now = Instant::now();
            let slot = actions
                .get(&team)
                .map_or(now, |last| (*last + TEAM_ACTION_GAP).max(now));
            actions.insert(team, slot);
            slot
        };
        tokio::time::sleep_until(slot).await;
    }

    /// Whether this is an admin's test game.
    #[must_use]
    pub fn is_test(&self) -> bool {
        self.test
    }

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
pub struct Sessions {
    map: Mutex<HashMap<String, Arc<Session>>>,
    /// Where games are saved.
    dir: PathBuf,
}

impl Sessions {
    /// Sessions saved in `dir`, bringing back the games saved there. Unreadable files are
    /// skipped.
    ///
    /// # Errors
    /// Fails if the directory cannot be created or listed.
    pub async fn open(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into();
        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("creating {}", dir.display()))?;
        let sessions = Self {
            map: Mutex::default(),
            dir: dir.clone(),
        };
        let mut entries = tokio::fs::read_dir(&dir).await?;
        let now = std::time::Instant::now();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match read_snapshot(&path).await {
                Ok(snapshot) => {
                    let code = snapshot.code().to_owned();
                    sessions.start(
                        &mut sessions.lock(),
                        code,
                        Game::restore(snapshot, now),
                        false,
                    );
                }
                Err(err) => dioxus::logger::tracing::warn!("skipping {}: {err:#}", path.display()),
            }
        }
        Ok(sessions)
    }

    /// Starts a session for a snapshot of `quiz` and returns its join code.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn create(&self, quiz: Quiz, max_teams: usize, pictures: bool) -> Result<String, Refusal> {
        self.create_with(quiz, false, max_teams, pictures)
    }

    /// Starts an admin's test game: like [`Sessions::create`], but never saved.
    ///
    /// # Errors
    /// Refuses quizzes that are not playable.
    pub fn create_test(&self, quiz: Quiz) -> Result<String, Refusal> {
        self.create_with(quiz, true, quaesito_core::protocol::DEFAULT_MAX_TEAMS, true)
    }

    fn create_with(
        &self,
        quiz: Quiz,
        test: bool,
        max_teams: usize,
        pictures: bool,
    ) -> Result<String, Refusal> {
        let mut map = self.lock();
        let code = loop {
            let candidate = format!("{:06}", uuid::Uuid::new_v4().as_u128() % 1_000_000);
            if !map.contains_key(&candidate) {
                break candidate;
            }
        };
        let mut game = Game::new(code.clone(), quiz, super::random_below)?;
        game.set_max_teams(max_teams)?;
        game.set_pictures(pictures);
        self.start(&mut map, code.clone(), game, test);
        Ok(code)
    }

    #[must_use]
    pub fn get(&self, code: &str) -> Option<Arc<Session>> {
        self.lock().get(code.trim()).cloned()
    }

    /// Ends a session and forgets its saved game. Connected screens keep their last view.
    pub fn remove(&self, code: &str) {
        if let Some(session) = self.lock().remove(code) {
            session.ended.store(true, Ordering::SeqCst);
        }
        if let Some(path) = self.path(code) {
            // Absent when the game was never saved; nothing else to do either way.
            let _ = std::fs::remove_file(path);
        }
    }

    /// Uploaded files shown by any live game.
    #[must_use]
    pub fn media(&self) -> Vec<String> {
        self.lock()
            .values()
            .flat_map(|s| s.read(Game::media))
            .collect()
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
                    test: s.test,
                })
            })
            .collect();
        list.sort_by(|a, b| a.code.cmp(&b.code));
        list
    }

    /// Registers a game and starts its timer task, and its saving task unless it is a test.
    fn start(&self, map: &mut HashMap<String, Arc<Session>>, code: String, game: Game, test: bool) {
        let session = Arc::new(Session::new(game, test));
        tokio::spawn(run_timers(Arc::downgrade(&session), session.subscribe()));
        if let Some(path) = self.path(&code).filter(|_| !test) {
            tokio::spawn(run_saver(
                Arc::downgrade(&session),
                session.subscribe(),
                path,
            ));
            // Save the new (or restored) game right away.
            session.version.send_modify(|v| *v += 1);
        }
        map.insert(code, session);
    }

    /// The saved game's file, for valid codes only.
    fn path(&self, code: &str) -> Option<PathBuf> {
        let valid = !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit());
        valid.then(|| self.dir.join(format!("{code}.json")))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<Session>>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

async fn read_snapshot(path: &Path) -> anyhow::Result<Snapshot> {
    let bytes = tokio::fs::read(path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Saves the game shortly after each change until the session is ended or dropped.
async fn run_saver(session: Weak<Session>, mut updates: watch::Receiver<u64>, path: PathBuf) {
    while updates.changed().await.is_ok() {
        tokio::time::sleep(SAVE_DELAY).await;
        let Some(snapshot) = session.upgrade().and_then(|s| {
            (!s.ended.load(Ordering::SeqCst))
                .then(|| s.read(|g| g.snapshot(std::time::Instant::now())))
        }) else {
            return;
        };
        if let Err(err) = write_snapshot(&path, &snapshot).await {
            dioxus::logger::tracing::warn!("saving {}: {err:#}", path.display());
        }
    }
}

/// Writes a snapshot atomically.
async fn write_snapshot(path: &Path, snapshot: &Snapshot) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(snapshot)?).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use quaesito_core::model::{Answer, Question};

    fn quiz() -> Quiz {
        Quiz {
            questions: vec![Question {
                text: "Q".to_owned(),
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
            ..Quiz::new("q".to_owned(), "Saved".to_owned())
        }
    }

    fn session() -> Session {
        Session::new(
            Game::new("123456".to_owned(), quiz(), crate::random_below).unwrap(),
            true,
        )
    }

    #[test]
    fn a_new_phone_connection_closes_the_teams_oldest() {
        let session = session();
        let mut open: Vec<_> = (0..MAX_PHONE_CONNECTIONS)
            .map(|_| session.connect_phone(1))
            .collect();
        // Another team's connections do not count.
        let (_, mut other) = session.connect_phone(2);
        for (_, closed) in &mut open {
            assert_eq!(closed.try_recv(), Err(oneshot::error::TryRecvError::Empty));
        }
        let (newest, _newest_closed) = session.connect_phone(1);
        assert_eq!(
            open[0].1.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        );
        assert_eq!(
            open[1].1.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        );
        assert_eq!(other.try_recv(), Err(oneshot::error::TryRecvError::Empty));
        // A connection that ends makes room.
        session.disconnect_phone(1, newest);
        session.connect_phone(1);
        assert_eq!(
            open[1].1.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_teams_actions_are_paced() {
        let session = session();
        let start = Instant::now();
        session.pace(1).await;
        session.pace(2).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        session.pace(1).await;
        session.pace(1).await;
        assert_eq!(start.elapsed(), TEAM_ACTION_GAP * 2);
    }

    #[tokio::test]
    async fn games_are_saved_and_brought_back() {
        let dir = std::env::temp_dir().join(format!("quaesito-games-{}", uuid::Uuid::new_v4()));
        let sessions = Sessions::open(&dir).await.unwrap();
        let code = sessions
            .create(quiz(), quaesito_core::protocol::DEFAULT_MAX_TEAMS, true)
            .unwrap();
        let session = sessions.get(&code).unwrap();
        let team = session.update(|g, _| g.join("Owls", "ticket".to_owned()).unwrap());

        // Wait for the saver to write the game with its team.
        let mut restored = None;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let reopened = Sessions::open(&dir).await.unwrap();
            if reopened
                .get(&code)
                .is_some_and(|s| s.read(|g| g.team_for_token("ticket")) == Some(team))
            {
                restored = Some(reopened);
                break;
            }
        }
        let restored = restored.expect("the saved game was not brought back");

        sessions.remove(&code);
        restored.remove(&code);
        assert!(!dir.join(format!("{code}.json")).exists());

        // Test games are never saved.
        let test = sessions.create_test(quiz()).unwrap();
        assert!(sessions.get(&test).unwrap().is_test());
        assert!(sessions.list().iter().any(|s| s.code == test && s.test));
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!dir.join(format!("{test}.json")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
