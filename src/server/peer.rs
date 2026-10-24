//! Where a request comes from: this machine, the local network, or elsewhere.
//!
//! The address is the connection's, or, when that is this machine and the request carries
//! `X-Forwarded-For`, the one a reverse proxy here put last in it (Caddy sets it to the address
//! that connected to it). Elsewhere, the header is ignored: anyone can send it. A request from
//! this machine must also be addressed to it (`localhost` or a loopback address in `Host`), so a
//! proxy that passes on the public hostname but no `X-Forwarded-For` does not make every visitor
//! look local. This machine opened at one of its network addresses (`http://192.168.…`) counts
//! as another device, but is told to use `localhost` instead.
//!
//! Development builds run behind dx's proxy, which does not pass the address on: there, every
//! request counts as coming from this machine.

use std::net::{IpAddr, SocketAddr, UdpSocket};

use dioxus::fullstack::{
    FullstackContext, HeaderMap,
    axum::extract::ConnectInfo,
    http::{Extensions, StatusCode},
};
use dioxus::prelude::{HttpError, ServerFnError};

/// Where a request comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer {
    /// The address, `None` if it cannot be told (development builds).
    ip: Option<IpAddr>,
    /// The request was addressed to this machine by a loopback name.
    to_loopback: bool,
    /// The port in `Host`, for the `localhost` address in [`Self::refusal`].
    port: Option<u16>,
}

impl Peer {
    /// The peer of a request, from its extensions (the connection's address) and headers.
    #[must_use]
    pub fn of(extensions: &Extensions, headers: &HeaderMap) -> Self {
        let connection = extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_canonical());
        let host = host(headers);
        Self {
            ip: connection.map(|ip| {
                if ip.is_loopback() {
                    forwarded_for(headers).unwrap_or(ip)
                } else {
                    ip
                }
            }),
            to_loopback: host.is_some_and(|(name, _)| is_loopback_name(name)),
            port: host.and_then(|(_, port)| port),
        }
    }

    /// The peer of the server function being run.
    #[must_use]
    pub fn current() -> Self {
        let unknown = Self {
            ip: None,
            to_loopback: false,
            port: None,
        };
        FullstackContext::current().map_or(unknown, |context| {
            let parts = context.parts_mut();
            Self::of(&parts.extensions, &parts.headers)
        })
    }

    /// Who is guessing, for [`super::throttle`]: the address, IPv6 by its /64 (which one
    /// subscriber usually holds whole).
    #[must_use]
    pub fn client(self) -> Option<IpAddr> {
        self.ip.map(|ip| match ip {
            IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) & !u128::from(u64::MAX)).into()),
            IpAddr::V4(_) => ip,
        })
    }

    /// Whether the request comes from the machine quaesito runs on, addressed to it as
    /// `localhost` (an SSH tunnel to it counts).
    #[must_use]
    pub fn is_this_machine(self) -> bool {
        self.ip.map_or(cfg!(debug_assertions), |ip| {
            ip.is_loopback() && self.to_loopback
        })
    }

    /// Whether the request comes from this machine, addressed to it by one of its network
    /// addresses: only this machine's own addresses can be bound.
    fn is_this_machine_by_network_address(self) -> bool {
        self.ip
            .is_some_and(|ip| !ip.is_loopback() && UdpSocket::bind((ip, 0)).is_ok())
    }

    /// Whether the request comes from this machine or the local network (a private address),
    /// not from the internet.
    #[must_use]
    pub fn is_local_network(self) -> bool {
        self.ip.map_or(cfg!(debug_assertions), |ip| match ip {
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
            Err(HttpError::new(StatusCode::FORBIDDEN, self.refusal()).into())
        }
    }

    /// Why an upload from elsewhere was refused. This machine opened at its network address is
    /// given its `localhost` address, which the admin page shows as a link.
    #[must_use]
    pub fn refusal(self) -> String {
        if self.is_this_machine_by_network_address() {
            let port = self.port.map(|port| format!(":{port}")).unwrap_or_default();
            format!(
                "Uploads only work on the computer quaesito runs on, opened at localhost: this \
                 page is open at the computer's network address, which counts as another \
                 device. Open it at http://localhost{port} instead."
            )
        } else {
            "Uploads only work on the computer quaesito runs on: open it there, at \
             http://localhost and its port (on a server, through an SSH tunnel)."
                .to_owned()
        }
    }
}

/// The name and port in the `Host` header: `localhost:8080`, `[::1]`.
fn host(headers: &HeaderMap) -> Option<(&str, Option<u16>)> {
    let host = headers
        .get(dioxus::fullstack::http::header::HOST)?
        .to_str()
        .ok()?;
    let (name, port) = match host.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']').unwrap_or((bracketed, "")),
        None => host.split_once(':').unwrap_or((host, "")),
    };
    Some((name, port.trim_start_matches(':').parse().ok()))
}

/// Whether a host name names this machine by a loopback name: `localhost`, `127.0.0.1`, `::1`.
fn is_loopback_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.to_canonical().is_loopback())
}

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
        peer_to("localhost:8080", connection, forwarded)
    }

    fn peer_to(host: &str, connection: Option<&str>, forwarded: &[&str]) -> Peer {
        let mut extensions = Extensions::new();
        if let Some(addr) = connection {
            extensions.insert(ConnectInfo(addr.parse::<SocketAddr>().unwrap()));
        }
        let mut headers = HeaderMap::new();
        headers.insert("host", host.parse().unwrap());
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
        assert_eq!(peer(Some("127.0.0.1:5000"), &[]).ip, Some(ip("127.0.0.1")));
        let proxied = peer(Some("127.0.0.1:5000"), &["10.0.0.1, 203.0.113.7"]);
        assert_eq!(proxied.ip, Some(ip("203.0.113.7")));
        assert_eq!(
            peer(Some("[::1]:5000"), &["10.0.0.1", "203.0.113.8"]).ip,
            Some(ip("203.0.113.8"))
        );
        assert_eq!(
            peer(Some("127.0.0.1:5000"), &["::ffff:203.0.113.9"]).ip,
            Some(ip("203.0.113.9"))
        );
        // Junk leaves the connection's address.
        assert_eq!(
            peer(Some("127.0.0.1:5000"), &["junk"]).ip,
            Some(ip("127.0.0.1"))
        );
    }

    #[test]
    fn others_cannot_claim_an_address() {
        let spoofed = peer(Some("192.168.1.20:5000"), &["127.0.0.1"]);
        assert_eq!(spoofed.ip, Some(ip("192.168.1.20")));
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
    fn this_machine_is_addressed_as_localhost() {
        for host in [
            "localhost",
            "LOCALHOST:8080",
            "127.0.0.1:8080",
            "[::1]:8080",
            "[::1]",
        ] {
            assert!(
                peer_to(host, Some("127.0.0.1:5000"), &[]).is_this_machine(),
                "{host}"
            );
        }
        // A proxy that sends the public name but no X-Forwarded-For.
        let proxied = peer_to("quiz.example.com", Some("127.0.0.1:5000"), &[]);
        assert!(!proxied.is_this_machine());
        let lookalike = peer_to("localhost.example.com", Some("127.0.0.1:5000"), &[]);
        assert!(!lookalike.is_this_machine());
        // No Host header at all.
        let mut extensions = Extensions::new();
        extensions.insert(ConnectInfo("127.0.0.1:5000".parse::<SocketAddr>().unwrap()));
        assert!(!Peer::of(&extensions, &HeaderMap::new()).is_this_machine());
    }

    #[test]
    fn this_machine_at_its_network_address_is_sent_to_localhost() {
        // The unspecified address stands in for a network address: every machine can bind it,
        // and it is not loopback.
        let own = peer_to("192.168.1.20:8080", Some("0.0.0.0:5000"), &[]);
        assert!(!own.is_this_machine());
        assert!(
            own.refusal()
                .ends_with("Open it at http://localhost:8080 instead.")
        );
        let no_port = peer_to("192.168.1.20", Some("0.0.0.0:5000"), &[]);
        assert!(no_port.refusal().ends_with("at http://localhost instead."));
        let other = peer_to("192.168.1.20:8080", Some("192.0.2.1:5000"), &[]);
        assert!(other.refusal().contains("SSH tunnel"));
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
