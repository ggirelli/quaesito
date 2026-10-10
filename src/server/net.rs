//! Working out the address phones should use to reach this machine.

use std::net::{IpAddr, UdpSocket};

/// Base URL for join links: `QUAESITO_PUBLIC_URL` if set, otherwise this machine's LAN address
/// with the port the host screen was opened on.
#[must_use]
pub fn join_origin(page_port: &str) -> String {
    if let Ok(url) = std::env::var("QUAESITO_PUBLIC_URL")
        && !url.is_empty()
    {
        return url.trim_end_matches('/').to_owned();
    }
    let ip = lan_ip().map_or_else(|| "localhost".to_owned(), |ip| ip.to_string());
    let port = page_port.trim();
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        format!("http://{ip}")
    } else {
        format!("http://{ip}:{port}")
    }
}

/// The address of the interface used for outbound traffic. Connecting a UDP socket sends nothing.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}
