//! Where a request comes from: this machine, the local network, or elsewhere.
//!
//! The address is the connection's, or, when that is this machine and the request carries
//! `X-Forwarded-For`, the one a reverse proxy here put last in it (Caddy sets it to the address
//! that connected to it). Elsewhere, the header is ignored: anyone can send it.
//!
//! Development builds run behind dx's proxy, which does not pass the address on: there, every
//! request counts as coming from this machine.

use std::net::{IpAddr, SocketAddr};

use dioxus::fullstack::{
    FullstackContext, HeaderMap,
    axum::extract::ConnectInfo,
    http::{Extensions, StatusCode},
};
use dioxus::prelude::{HttpError, ServerFnError};

/// The address a request comes from, `None` if it cannot be told (development builds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer(Option<IpAddr>);

impl Peer {
    /// The peer of a request, from its extensions (the connection's address) and headers.
    #[must_use]
    pub fn of(extensions: &Extensions, headers: &HeaderMap) -> Self {
        let connection = extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_canonical());
        Self(connection.map(|ip| {
            if ip.is_loopback() {
                forwarded_for(headers).unwrap_or(ip)
            } else {
                ip
            }
        }))
    }

    /// The peer of the server function being run.
    #[must_use]
    pub fn current() -> Self {
        FullstackContext::current().map_or(Self(None), |context| {
            let parts = context.parts_mut();
            Self::of(&parts.extensions, &parts.headers)
        })
    }

    /// Who is guessing, for [`super::throttle`]: the address, IPv6 by its /64 (which one
    /// subscriber usually holds whole).
    #[must_use]
    pub fn client(self) -> Option<IpAddr> {
        self.0.map(|ip| match ip {
            IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) & !u128::from(u64::MAX)).into()),
            IpAddr::V4(_) => ip,
        })
    }

    /// Whether the request comes from the machine quaesito runs on (an SSH tunnel to it counts).
    #[must_use]
    pub fn is_this_machine(self) -> bool {
        self.0.map_or(cfg!(debug_assertions), |ip| ip.is_loopback())
    }

    /// Whether the request comes from this machine or the local network (a private address),
    /// not from the internet.
    #[must_use]
    pub fn is_local_network(self) -> bool {
        self.0.map_or(cfg!(debug_assertions), |ip| match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
            }
        })
    }

    /// Errors with 403 unless the request comes from this machine.
    ///
    /// # Errors
    /// Refuses requests from elsewhere.
    pub fn require_this_machine(self) -> Result<(), ServerFnError> {
        if self.is_this_machine() {
            Ok(())
        } else {
            Err(HttpError::new(StatusCode::FORBIDDEN, FROM_THIS_MACHINE).into())
        }
    }
}

/// Why an upload from another device was refused.
pub const FROM_THIS_MACHINE: &str = "Uploads only work on the computer quaesito runs on: open \
    it there, at http://localhost and its port (on a server, through an SSH tunnel).";

/// The address a proxy on this machine put last in `X-Forwarded-For`.
fn forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .next_back()?
        .trim()
        .parse::<IpAddr>()
        .ok()
        .map(|ip| ip.to_canonical())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(connection: Option<&str>, forwarded: &[&str]) -> Peer {
        let mut extensions = Extensions::new();
        if let Some(addr) = connection {
            extensions.insert(ConnectInfo(addr.parse::<SocketAddr>().unwrap()));
        }
        let mut headers = HeaderMap::new();
        for value in forwarded {
            headers.append("x-forwarded-for", value.parse().unwrap());
        }
        Peer::of(&extensions, &headers)
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn a_local_proxy_names_the_client() {
        assert_eq!(peer(Some("127.0.0.1:5000"), &[]).0, Some(ip("127.0.0.1")));
        let proxied = peer(Some("127.0.0.1:5000"), &["10.0.0.1, 203.0.113.7"]);
        assert_eq!(proxied.0, Some(ip("203.0.113.7")));
        assert_eq!(
            peer(Some("[::1]:5000"), &["10.0.0.1", "203.0.113.8"]).0,
            Some(ip("203.0.113.8"))
        );
        assert_eq!(
            peer(Some("127.0.0.1:5000"), &["::ffff:203.0.113.9"]).0,
            Some(ip("203.0.113.9"))
        );
        // Junk leaves the connection's address.
        assert_eq!(
            peer(Some("127.0.0.1:5000"), &["junk"]).0,
            Some(ip("127.0.0.1"))
        );
    }

    #[test]
    fn others_cannot_claim_an_address() {
        let spoofed = peer(Some("192.168.1.20:5000"), &["127.0.0.1"]);
        assert_eq!(spoofed.0, Some(ip("192.168.1.20")));
        assert!(!spoofed.is_this_machine());
        let spoofed = peer(Some("203.0.113.5:5000"), &["192.168.1.20"]);
        assert!(!spoofed.is_local_network());
    }

    #[test]
    fn this_machine_the_network_and_the_internet() {
        let here = peer(Some("127.0.0.1:5000"), &[]);
        assert!(here.is_this_machine() && here.is_local_network());
        assert!(peer(Some("[::ffff:127.0.0.1]:5000"), &[]).is_this_machine());
        for lan in [
            "192.168.1.20:5000",
            "10.1.2.3:5000",
            "[fd00::5]:5000",
            "[fe80::1]:5000",
        ] {
            let lan = peer(Some(lan), &[]);
            assert!(!lan.is_this_machine() && lan.is_local_network(), "{lan:?}");
        }
        let lan_through_proxy = peer(Some("127.0.0.1:5000"), &["192.168.1.20"]);
        assert!(!lan_through_proxy.is_this_machine() && lan_through_proxy.is_local_network());
        for internet in ["203.0.113.5:5000", "[2001:db8::1]:5000"] {
            let internet = peer(Some(internet), &[]);
            assert!(!internet.is_this_machine() && !internet.is_local_network());
        }
        let through_proxy = peer(Some("127.0.0.1:5000"), &["203.0.113.5"]);
        assert!(!through_proxy.is_this_machine() && !through_proxy.is_local_network());
        assert!(through_proxy.require_this_machine().is_err());
    }

    #[test]
    fn ipv6_clients_are_their_64() {
        assert_eq!(
            peer(Some("[2001:db8:1:2:3:4:5:6]:1"), &[]).client(),
            peer(Some("[2001:db8:1:2:ffff::1]:1"), &[]).client()
        );
        assert_ne!(
            peer(Some("[2001:db8:1:2::1]:1"), &[]).client(),
            peer(Some("[2001:db8:1:3::1]:1"), &[]).client()
        );
        assert_eq!(peer(None, &["203.0.113.1"]).client(), None);
    }
}
