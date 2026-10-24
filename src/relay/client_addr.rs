//! Who a request comes from: the one place the relay decides a client's address (2026-10-22).
//!
//! THE PROBLEM. The relay normally sits behind nginx, so every connection it sees comes from
//! nginx on the same machine (127.0.0.1). The visitor's own address arrives in a header nginx
//! writes: `X-Real-IP` (`proxy_set_header X-Real-IP $remote_addr;`, the address nginx saw) or
//! `X-Forwarded-For`. But a header is only text. A visitor who reaches the relay DIRECTLY (a
//! relay listening on every address with its port open, a dev node, a home node on a LAN) can
//! write any `X-Real-IP` it likes, and nginx itself passes a client's own `X-Forwarded-For`
//! through unless a location replaces it, and `$proxy_add_x_forwarded_for` only APPENDS to what
//! the client sent. Before this file the WebSocket's per-address limits (identify attempts, new
//! accounts an hour) read the FIRST `X-Forwarded-For` entry, the one the client writes, so a
//! script could give itself a new address on every connection.
//!
//! THE RULE. A header naming the client is believed only when the connection itself comes from
//! a TRUSTED PROXY ([`TrustedProxies`], loopback by default, `TRUSTED_PROXIES` to change). From
//! a trusted proxy: `X-Real-IP` when it holds an address that is not itself a proxy; otherwise
//! the right-most `X-Forwarded-For` entry that is not a proxy (each proxy appends the address it
//! saw, so everything to the left of the last one is the client's own writing); otherwise the
//! proxy's address itself. From anyone else: the connection's own address, and every header is
//! ignored.
//!
//! ONE CHOKE POINT. [`crate::relay::ip_limit::edge`] runs this for every request before any
//! route sees it, then REWRITES the headers: `X-Forwarded-For` is removed and `X-Real-IP` set to
//! the address decided here (or removed when there is none). So every reader downstream (the
//! WebSocket's limits in mod.rs `ws_handler`, the Files page's counts in files_counts.rs
//! `address_of`, the copyright form's daily limit) reads one header that can be believed, the
//! way nginx's own realip module rewrites `$remote_addr`.

use std::net::IpAddr;

use axum::http::HeaderMap;

/// The environment variable that names the trusted proxies: addresses or networks separated by
/// commas (`127.0.0.1, ::1, 10.0.0.0/8`), or `none` to trust no proxy at all. Unset: loopback.
pub const TRUSTED_PROXIES_ENV: &str = "TRUSTED_PROXIES";

/// The header nginx sets from the connection itself (`proxy_set_header X-Real-IP $remote_addr;`).
pub const REAL_IP: &str = "x-real-ip";
/// The header each proxy appends to (`$proxy_add_x_forwarded_for`); the client may write its start.
pub const FORWARDED_FOR: &str = "x-forwarded-for";

/// One address or network a proxy may connect from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Net {
    addr: IpAddr,
    /// How many leading bits must match (32 or 128 for one address).
    prefix: u8,
}

impl Net {
    fn parse(text: &str) -> Option<Net> {
        let text = text.trim();
        let (addr, prefix) = match text.split_once('/') {
            Some((a, p)) => (a.trim().parse::<IpAddr>().ok()?, Some(p.trim().parse::<u8>().ok()?)),
            None => (text.parse::<IpAddr>().ok()?, None),
        };
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = prefix.unwrap_or(max);
        (prefix <= max).then_some(Net { addr, prefix })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, plain(ip)) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = if self.prefix == 0 { 0 } else { u32::MAX << (32 - self.prefix as u32) };
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = if self.prefix == 0 { 0 } else { u128::MAX << (128 - self.prefix as u32) };
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
}

/// An IPv4 address written as IPv6 (`::ffff:203.0.113.7`) as the IPv4 address it is; any other
/// address as it is. A dual-stack listener reports IPv4 peers in the IPv6 form.
pub fn plain(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

/// The proxies whose headers about a client are believed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedProxies {
    nets: Vec<Net>,
}

impl Default for TrustedProxies {
    /// From `TRUSTED_PROXIES`, or loopback when it is unset.
    fn default() -> Self {
        TrustedProxies::from_setting(std::env::var(TRUSTED_PROXIES_ENV).ok().as_deref())
    }
}

impl TrustedProxies {
    /// Loopback only: nginx on the same machine, the setup every guide describes.
    pub fn loopback() -> TrustedProxies {
        TrustedProxies { nets: vec![Net::parse("127.0.0.0/8").unwrap(), Net::parse("::1").unwrap()] }
    }

    /// No proxy at all: every connection's own address is the client's, and every header is
    /// ignored. For a relay that faces the internet directly.
    pub fn none() -> TrustedProxies {
        TrustedProxies { nets: Vec::new() }
    }

    /// The setting's text (see [`TRUSTED_PROXIES_ENV`]): None or blank is loopback, `none` is no
    /// proxy, anything else a list. An entry that is not an address or a network is left out and
    /// said in the log, so a typo never trusts more than was written.
    pub fn from_setting(text: Option<&str>) -> TrustedProxies {
        let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
            return TrustedProxies::loopback();
        };
        if text.eq_ignore_ascii_case("none") {
            return TrustedProxies::none();
        }
        let mut nets = Vec::new();
        for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            match Net::parse(entry) {
                Some(net) => nets.push(net),
                None => tracing::warn!("{TRUSTED_PROXIES_ENV}: '{entry}' is not an address or a network, so it is not trusted"),
            }
        }
        TrustedProxies { nets }
    }

    /// Whether `ip` is one of the trusted proxies.
    pub fn trusts(&self, ip: IpAddr) -> bool {
        self.nets.iter().any(|n| n.contains(ip))
    }
}

/// The address a request comes from, by the rule at the top of this file. `peer` is the
/// connection's own address (None when the server was not started with it, which only tests
/// do: then no header is believed and the answer is None, the shared "unknown").
pub fn client_ip(peer: Option<IpAddr>, headers: &HeaderMap, trusted: &TrustedProxies) -> Option<IpAddr> {
    let peer = plain(peer?);
    if !trusted.trusts(peer) {
        return Some(peer);
    }
    let real = headers
        .get(REAL_IP)
        .and_then(|v| v.to_str().ok())
        .and_then(|t| t.trim().parse::<IpAddr>().ok())
        .map(plain);
    if let Some(ip) = real.filter(|ip| !trusted.trusts(*ip)) {
        return Some(ip);
    }
    // Right to left: the last entry was written by the proxy that connected to us, and each
    // proxy before it wrote the one to its left. The first one that is not a proxy is the client;
    // everything further left is the client's own writing and is never read.
    let forwarded: Vec<IpAddr> = headers
        .get_all(FORWARDED_FOR)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|t| t.trim().parse::<IpAddr>().ok())
        .map(plain)
        .collect();
    if let Some(ip) = forwarded.iter().rev().find(|ip| !trusted.trusts(**ip)) {
        return Some(*ip);
    }
    // Every hop a proxy, or no header at all: the request is the proxy's own (or a local
    // process's). Its address is the honest answer.
    Some(real.or(forwarded.first().copied()).unwrap_or(peer))
}

/// What a request with no address counts as: one shared bucket, which can only be stricter.
pub const UNKNOWN: &str = "unknown";

/// The bucket an address counts in: an IPv4 address as itself, an IPv6 address as its /64 (one
/// home or one phone is usually given a whole /64, and can pick any address in it), no address as
/// [`UNKNOWN`]. The same reading as files_counts.rs `address_of`.
pub fn bucket(ip: Option<IpAddr>) -> String {
    match ip.map(plain) {
        Some(IpAddr::V4(v4)) => v4.to_string(),
        Some(IpAddr::V6(v6)) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        None => UNKNOWN.to_string(),
    }
}

/// Replace the client headers with the one address decided here, so every reader after the edge
/// reads what can be believed: `X-Forwarded-For` goes, `X-Real-IP` is the address (or goes).
pub fn rewrite(headers: &mut HeaderMap, ip: Option<IpAddr>) {
    headers.remove(FORWARDED_FOR);
    headers.remove(REAL_IP);
    if let Some(ip) = ip {
        if let Ok(v) = axum::http::HeaderValue::from_str(&ip.to_string()) {
            headers.insert(REAL_IP, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        h
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// A HEADER FROM A CONNECTION THAT IS NOT A PROXY IS NEVER BELIEVED: someone reaching the
    /// relay directly from 198.51.100.9 who writes `X-Real-IP: 203.0.113.7` (or any
    /// `X-Forwarded-For`) is 198.51.100.9. From nginx on loopback the same headers are believed.
    ///
    /// Seen red 2026-10-22 with the `trusts(peer)` check taken out of `client_ip` (headers read
    /// from anyone): "a direct client's X-Real-IP was believed / left: 203.0.113.7".
    #[test]
    fn a_header_from_a_connection_that_is_not_a_proxy_is_never_believed() {
        let t = TrustedProxies::loopback();
        let spoof = headers(&[("x-real-ip", "203.0.113.7"), ("x-forwarded-for", "192.0.2.1")]);
        assert_eq!(client_ip(Some(ip("198.51.100.9")), &spoof, &t), Some(ip("198.51.100.9")), "a direct client's X-Real-IP was believed");
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &spoof, &t), Some(ip("203.0.113.7")), "nginx on loopback is believed");
        assert_eq!(client_ip(Some(ip("::ffff:127.0.0.1")), &spoof, &t), Some(ip("203.0.113.7")), "loopback written as IPv6");
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &spoof, &TrustedProxies::none()), Some(ip("127.0.0.1")), "no trusted proxy: nothing is believed");
        assert_eq!(client_ip(None, &spoof, &t), None, "no connection address: no header is believed");
    }

    /// X-FORWARDED-FOR IS READ FROM THE RIGHT: the client writes the left of it, each proxy
    /// appends what it saw, so the right-most entry that is not a proxy is the client. X-Real-IP
    /// wins when it names a client; one naming a proxy falls through to the walk.
    ///
    /// Seen red 2026-10-22 with the walk taking the FIRST entry (the old ws_handler reading):
    /// "the client's own first entry was believed / left: 6.6.6.6".
    #[test]
    fn x_forwarded_for_is_read_from_the_right() {
        let t = TrustedProxies::from_setting(Some("127.0.0.1, 10.0.0.0/8"));
        let chain = headers(&[("x-forwarded-for", "6.6.6.6, 203.0.113.7, 10.1.2.3")]);
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &chain, &t), Some(ip("203.0.113.7")), "the client's own first entry was believed");
        let both = headers(&[("x-real-ip", "198.51.100.4"), ("x-forwarded-for", "6.6.6.6")]);
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &both, &t), Some(ip("198.51.100.4")), "X-Real-IP first");
        let real_is_proxy = headers(&[("x-real-ip", "10.0.0.2"), ("x-forwarded-for", "203.0.113.7, 10.0.0.2")]);
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &real_is_proxy, &t), Some(ip("203.0.113.7")));
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &headers(&[]), &t), Some(ip("127.0.0.1")), "no header: the proxy's own request");
        let two = headers(&[("x-forwarded-for", "6.6.6.6"), ("x-forwarded-for", "203.0.113.8")]);
        assert_eq!(client_ip(Some(ip("127.0.0.1")), &two, &t), Some(ip("203.0.113.8")), "two header lines read as one list");
    }

    #[test]
    fn the_setting_reads_addresses_networks_and_none() {
        assert_eq!(TrustedProxies::from_setting(None), TrustedProxies::loopback());
        assert_eq!(TrustedProxies::from_setting(Some("  ")), TrustedProxies::loopback());
        assert_eq!(TrustedProxies::from_setting(Some("NONE")), TrustedProxies::none());
        let t = TrustedProxies::from_setting(Some("10.0.0.0/8, 2001:db8::/32, bogus, 192.0.2.5"));
        assert!(t.trusts(ip("10.200.1.1")) && t.trusts(ip("2001:db8:5::1")) && t.trusts(ip("192.0.2.5")));
        assert!(!t.trusts(ip("192.0.2.6")) && !t.trusts(ip("127.0.0.1")), "only what was written");
        assert!(Net::parse("10.0.0.0/33").is_none() && Net::parse("::1/129").is_none());
    }

    #[test]
    fn a_bucket_is_an_ipv4_address_or_an_ipv6_slash_64() {
        assert_eq!(bucket(Some(ip("203.0.113.7"))), "203.0.113.7");
        assert_eq!(bucket(Some(ip("::ffff:203.0.113.7"))), "203.0.113.7");
        assert_eq!(bucket(Some(ip("2001:db8:1:2:ffff::9"))), "2001:db8:1:2::/64");
        assert_eq!(bucket(None), UNKNOWN);
    }

    #[test]
    fn the_rewrite_leaves_one_header_that_can_be_believed() {
        let mut h = headers(&[("x-real-ip", "6.6.6.6"), ("x-forwarded-for", "6.6.6.6"), ("x-forwarded-for", "7.7.7.7")]);
        rewrite(&mut h, Some(ip("198.51.100.9")));
        assert_eq!(h.get(REAL_IP).unwrap(), "198.51.100.9");
        assert!(h.get(FORWARDED_FOR).is_none());
        rewrite(&mut h, None);
        assert!(h.get(REAL_IP).is_none());
    }
}
