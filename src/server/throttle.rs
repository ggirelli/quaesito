//! Limits on guessing from one client: wrong admin passwords and unknown join codes.
//!
//! A client is the address a reverse proxy puts last in `X-Forwarded-For` (Caddy sets it to the
//! connecting address), so a server reachable from the internet must sit behind such a proxy.
//! Without the header, as on a LAN, every client counts as one.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use dioxus::fullstack::HeaderMap;

/// Most clients remembered; past it, the ones whose window has ended are forgotten.
const MAX_CLIENTS: usize = 10_000;

/// Who is trying: an address (IPv6 by its /64, which one subscriber usually holds whole), or
/// `None` for requests that did not come through a proxy.
pub type Client = Option<IpAddr>;

/// The client a request comes from.
#[must_use]
pub fn client(headers: &HeaderMap) -> Client {
    let ip: IpAddr = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .next_back()?
        .trim()
        .parse()
        .ok()?;
    Some(match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6((u128::from(v6) & !u128::from(u64::MAX)).into()),
        },
        IpAddr::V4(_) => ip,
    })
}

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

    fn headers(forwarded: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in forwarded {
            headers.append("x-forwarded-for", value.parse().unwrap());
        }
        headers
    }

    #[test]
    fn the_proxy_names_the_client() {
        assert_eq!(client(&headers(&[])), None);
        assert_eq!(client(&headers(&["junk"])), None);
        assert_eq!(
            client(&headers(&["10.0.0.1, 203.0.113.7"])),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            client(&headers(&["10.0.0.1", "203.0.113.8"])),
            Some("203.0.113.8".parse().unwrap())
        );
        assert_eq!(
            client(&headers(&["::ffff:203.0.113.9"])),
            Some("203.0.113.9".parse().unwrap())
        );
        assert_eq!(
            client(&headers(&["2001:db8:1:2:3:4:5:6"])),
            client(&headers(&["2001:db8:1:2:ffff::1"]))
        );
        assert_ne!(
            client(&headers(&["2001:db8:1:2::1"])),
            client(&headers(&["2001:db8:1:3::1"]))
        );
    }

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
