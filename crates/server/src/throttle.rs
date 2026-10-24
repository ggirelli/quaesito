//! Limits on guessing from one client: wrong admin passwords and unknown join codes.
//!
//! Clients are told apart by [`super::peer::Peer::client`]. In development builds every client
//! counts as one.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

/// Most clients remembered; past it, the ones whose window has ended are forgotten.
const MAX_CLIENTS: usize = 10_000;

/// Who is trying, `None` when it cannot be told.
pub type Client = Option<IpAddr>;

/// Allows `limit` failures per client in a window starting at its first failure.
pub struct Throttle {
    limit: u32,
    window: Duration,
    /// When each client's window started, and its failures since.
    failures: Mutex<HashMap<Client, (Instant, u32)>>,
}

impl Throttle {
    #[must_use]
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            failures: Mutex::default(),
        }
    }

    /// Whether `client` may try again.
    pub fn allows(&self, client: Client, now: Instant) -> bool {
        self.lock()
            .get(&client)
            .is_none_or(|&(start, count)| count < self.limit || now >= start + self.window)
    }

    /// Counts a failure for `client`.
    pub fn fail(&self, client: Client, now: Instant) {
        let mut failures = self.lock();
        if failures.len() >= MAX_CLIENTS {
            failures.retain(|_, (start, _)| now < *start + self.window);
        }
        let entry = failures.entry(client).or_insert((now, 0));
        if now >= entry.0 + self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Client, (Instant, u32)>> {
        self.failures.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_lock_a_client_out_for_the_window() {
        let throttle = Throttle::new(2, Duration::from_secs(60));
        let (me, other) = (Some("203.0.113.1".parse().unwrap()), None);
        let start = Instant::now();
        throttle.fail(me, start);
        assert!(throttle.allows(me, start));
        throttle.fail(me, start + Duration::from_secs(1));
        assert!(!throttle.allows(me, start + Duration::from_secs(59)));
        assert!(throttle.allows(other, start + Duration::from_secs(59)));
        assert!(throttle.allows(me, start + Duration::from_secs(60)));
        throttle.fail(me, start + Duration::from_secs(60));
        assert!(throttle.allows(me, start + Duration::from_secs(61)));
    }
}
