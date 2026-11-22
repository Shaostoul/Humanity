//! The addresses this relay answers to, which every signature a person makes for it must name
//! (2026-11-21, D1 of the signed-requests round 2; round 3 the same day; the bytes are
//! crates/humanity-protocol/src/signed_request.rs).
//!
//! A request is signed over `hum/request/v2`, the AUDIENCE, the purpose and the time; a sign-in over
//! `hum/identify/v2`, the audience, the relay's nonce and the key. The audience is the address the
//! person's app connected to (host name, and port when not the scheme's default). This relay accepts
//! a signature only when [`OwnNames::admit`] does, so a signature a person made for another server
//! never works here (the review of 2026-11-21: a server the desktop sent a channel read to replayed it
//! at another server; a server passed another server's sign-in challenge through to the person's app
//! and became the person there).
//!
//! WHAT [`OwnNames::admit`] ASKS, in order (round 3, the second review of 2026-11-21):
//! 1. The audience is the address THIS request or socket was sent to: an HTTP request's `Host`, a
//!    socket's upgrade `Host` ([`Arrival`]). Round 2 let a sign-in name any of the relay's names,
//!    whatever the socket was opened to.
//! 2. It is one of this relay's own names ([`OwnNames`]).
//! 3. A name that means "this computer" or "this local network" is taken only from a client that is
//!    there ([`Reach`]): a loopback name (`127.0.0.1`, `[::1]`, `localhost`) only from a loopback
//!    client, a local-network name (a private, link-local or shared address; a `.local`, `.lan`,
//!    `.home.arpa` or `.internal` name, or a one-word computer name) only from a client on that
//!    network (the address's /24, or /64 for IPv6; for a `.local` or one-word name, the /24 of one
//!    of the relay's own LAN addresses), never from a loopback client. A shared carrier address
//!    (100.64.0.0/10) is its own network, unless the owner says the relay is on a Tailscale
//!    network ([`TAILNET_ENV`], L3 of the review of round 5): then any address in the range is on
//!    it. Every relay on a loopback port and every home node at a common LAN address shares such a
//!    name with thousands of others, so the name alone says nothing about WHICH server: round 2's
//!    relays accepted each other's sign-ins through an `ssh -L` tunnel or a malicious home node at
//!    `192.168.1.5:3210`. The client's address here is the CONNECTION'S OWN, never a header's, and
//!    a connection through a proxy is never local (client_addr.rs `direct_peer`; H1 of the review of
//!    rounds 3 and 4, 2026-11-21: through Tor or nginx on the same computer every visitor was
//!    127.0.0.1, or whatever `X-Real-IP` they wrote, and a loopback client could use every LAN name).
//!    A forwarder that passes no header hands its visitors over from ITS address, so a loopback
//!    client is "this computer" and a client on the network is "the network" whoever they really
//!    are: that is why a home node with a public name lists no loopback name and, unless its owner
//!    asks, no LAN name either (host_node.rs `node_public_hosts`; M1 of the review of round 5: Tor
//!    on a router or a Pi, a tunnel agent on a NAS, made every visitor local).
//!
//! THE OWNER'S DOOR (M2 of the review of round 5, 2026-11-21). A relay given [`OWNER_PORT_ENV`] also
//! listens on `127.0.0.1` at that port, the way its owner's own app reaches it (the Host a node
//! page always opens one: the main port of a node with a public name answers to no loopback name).
//! Its names (`127.0.0.1`, `localhost` and `[::1]` at that port) are taken only through that door,
//! from a connection with no forwarding header, and only for the owner's key
//! ([`OWNER_KEY_ENV`], [`OwnNames::admit_signer`]); through the door no other name is taken. So a
//! forwarder pointed at the main port (as the docs say) hands over visitors who can use only the
//! public names, and one pointed at the door by mistake hands over visitors who can use only
//! signatures the owner's own app made for his own door.
//!
//! WHERE THE NAMES COME FROM (read once at start, [`OwnNames::from_settings`]):
//! - `PUBLIC_HOSTS`: comma-separated `host` or `host:port`, the names people's apps reach this server
//!   by that no other setting gives (an onion address, an IP address and port, a second domain). The
//!   desktop's Host a node section writes it from its Public address field (host_node.rs);
//! - every `https://` origin in `ALLOWED_ORIGINS` (the server's own websites, the white-label seam);
//!   an `http://` one only behind the dev switch below (L6 of the review of round 5: a name that
//!   only plain http reaches is one the network path can answer for);
//! - `TURN_PUBLIC_HOST`, else `TURN_SERVER_HOST`, when it is a domain name (origins.rs `own_site`);
//! - `127.0.0.1`, `localhost` and `[::1]` with the listening port ONLY behind the dev and test switch
//!   [`DEV_LOOPBACK_NAMES_ENV`] (the rigs set it, docs/ai/game-boots.md), never from the bind address
//!   alone: a public relay behind nginx or Tor listens on loopback too;
//! - the owner's door's names, through the door only ([`OWNER_PORT_ENV`]).
//!
//! THE WORDS. Every refusal of an audience, a port, a reach, a door or a signature is one sentence,
//! [`REFUSED_AS_REACHED`] (L1, 2026-11-21: round 3's words differed by case, so a prober could tell
//! "one of my names, wrong reach" from "not my name" and tie an onion address to its clearnet name
//! or find a home node's LAN address; L4 of the review of round 5: a made-up signature for one of
//! the relay's names still got the signature's own words, so the signature is now verified whatever
//! the audience and refused in the same words). The detail and this server's names go to its log.
//!
//! WHAT IT DOES NOT STOP: this relay, being the audience, can use what it receives here, once. And
//! over plain http, whoever can answer for the name (DNS, the network path) is that name; https (a
//! certificate binding the name) is what makes the audience mean the server, as for passkeys. On a
//! local network without TLS, anyone on that network who can answer for the address is that address:
//! docs/reference/cryptography.md says what a LAN name can and cannot promise. And a prober who
//! signs with a key of its own for a name it guesses is taken when the guess is right, as anyone
//! is for a public name: the words hide nothing from that, nor from `server_did` or `/health`
//! (docs/reference/cryptography.md, the onion-and-clearnet note).

use std::net::IpAddr;

use humanity_protocol::signed_request::canonical_audience;

/// The setting that adds names to the list.
pub const PUBLIC_HOSTS_ENV: &str = "PUBLIC_HOSTS";

/// The dev and test switch (round 3, C2): `1` gives the relay its loopback names on its listening
/// port (`127.0.0.1:{port}`, `localhost:{port}`, `[::1]:{port}`). The rigs' throwaway relays set it
/// (scripts/lib/throwaway-relay.js); no product setting does.
pub const DEV_LOOPBACK_NAMES_ENV: &str = "HUMANITY_DEV_LOOPBACK_NAMES";

/// The setting that opens the owner's door (M2, 2026-11-21): a port number. The relay then also
/// listens on `127.0.0.1` at that port (relay/mod.rs `run_relay`), and takes that door's names
/// through it alone, for the owner's key alone (the top of this file). The Host a node page sets it
/// to the node's port and one ([`owner_port_for`]).
pub const OWNER_PORT_ENV: &str = "OWNER_PORT";

/// The setting naming the owner's key (the Host a node page sets it to the app's identity): the
/// key granted the admin role at start (relay/mod.rs), and the only key the owner's door takes.
pub const OWNER_KEY_ENV: &str = "HUMANITY_OWNER_ADMIN_KEY";

/// The setting that says the relay is on a Tailscale network (L3, 2026-11-21): `1` lets a client
/// with any address in 100.64.0.0/10 use the relay's names in that range and its one-word names
/// (the tailnet's MagicDNS names). Off, an address in the range is its own network: a carrier's
/// customers share it, so it says nothing about who is near. The Host a node page writes it from
/// its "This computer is on a Tailscale network" box.
pub const TAILNET_ENV: &str = "ON_TAILNET";

/// The owner's door's port for a node listening on `port`: the next port, or the one before it for
/// the last port there is.
pub fn owner_port_for(port: u16) -> u16 {
    port.checked_add(1).unwrap_or(port - 1)
}

/// Which listener a connection came in by: the relay's own port, or the owner's door (M2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Door {
    /// The relay's port (`PORT`), where everyone comes in.
    #[default]
    Main,
    /// The owner's door ([`OWNER_PORT_ENV`]): `127.0.0.1` only, the owner's own app. Set as a request
    /// extension by that listener's outermost layer (relay/mod.rs), which no client can write.
    Owner,
}

/// The key a per-address count (the edge's allowances, ip_limit.rs; the sign-in attempts and new
/// accounts an hour, relay.rs) is kept under for a client at `address` that came in by `door`
/// (round 7 of the signed requests, 2026-11-21). Through the owner's door the owner is 127.0.0.1,
/// and so is every visitor a headerless forwarder on this computer hands over (what the docs
/// advise), so one shared count let a Tor visitor sending a sign-in every two seconds lock the
/// owner out of his own door. The door's counts are its own.
pub fn door_key(address: String, door: Door) -> String {
    match door {
        Door::Main => address,
        Door::Owner => format!("{OWNER_DOOR_PREFIX}{address}"),
    }
}

/// What every key [`door_key`] makes for the owner's door starts with. Only the door's own listener
/// can produce it (a main-port key is always a parsed address or "unknown").
pub const OWNER_DOOR_PREFIX: &str = "owner's door, ";

/// Whether `key` is one of the owner's door's own keys ([`door_key`]): the edge's table never moves
/// such a key into its shared overflow bucket (ip_limit.rs `take`, the review of round 7: a flood
/// filling the table with 50,000 IPv6 /64s kept that shared bucket empty, so the door answered 429).
pub fn is_owner_door_key(key: &str) -> bool {
    key.starts_with(OWNER_DOOR_PREFIX)
}

/// Where a signature arrived: the address the request or socket was sent to (its `Host`), the
/// connection's own address when it came straight from the client, the only address reach is
/// decided from (client_addr.rs `direct_peer`, H1 of 2026-11-21), and the listener it came in by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arrival {
    /// The `Host` the request or the socket's upgrade named.
    pub host: Option<String>,
    /// The connection's own address when no proxy stood between (no forwarding header, not a
    /// trusted proxy elsewhere); `None` through a proxy, which is never on this computer or this
    /// local network. Never an address a header names.
    pub peer: Option<IpAddr>,
    /// The listener the connection came in by.
    pub door: Door,
}

impl Arrival {
    /// The arrival of a request: its `Host`, the edge's decision `decided` about where the
    /// connection came from (ip_limit.rs `edge`, as a request extension), and the listener `door` it
    /// came in by (a request extension, [`Door`]). A router built without the edge (some tests build
    /// one) has no decision: the same rule is applied here to the connection's own address and the
    /// headers as they came.
    pub fn from_parts(
        headers: &axum::http::HeaderMap,
        connection: Option<std::net::SocketAddr>,
        decided: Option<crate::relay::client_addr::DirectPeer>,
        door: Option<Door>,
    ) -> Self {
        use crate::relay::client_addr::{direct_peer, TrustedProxies};
        let host = headers.get(axum::http::header::HOST).and_then(|v| v.to_str().ok()).map(str::to_string);
        let peer = match decided {
            Some(d) => d.0,
            None => direct_peer(connection.map(|c| c.ip()), headers, &TrustedProxies::loopback()),
        };
        Self { host, peer: peer.map(|ip| ip.to_canonical()), door: door.unwrap_or_default() }
    }
}

/// Who may present a signature for a name: anyone, a program on this computer, or a client on the
/// name's local network (its address when the name is one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    Anywhere,
    ThisComputer,
    LocalNetwork(Option<IpAddr>),
}

/// The host part of an audience (`host` or `host:port`), brackets kept off an IPv6 address.
fn host_part(audience: &str) -> &str {
    if let Some(rest) = audience.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    audience.rsplit_once(':').map_or(audience, |(h, _)| h)
}

/// Whether `ip` is an address only a local network routes: private (RFC 1918), link-local, the
/// shared carrier range 100.64.0.0/10, or for IPv6 a unique-local (fc00::/7) or link-local address.
fn is_local_network(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private() || v4.is_link_local() || (o[0] == 100 && (o[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments()[0];
            (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

/// Who may present a signature for `audience` (a canonical audience).
pub fn reach_of(audience: &str) -> Reach {
    let host = host_part(audience);
    if let Ok(ip) = host.parse::<IpAddr>() {
        let ip = ip.to_canonical();
        return if ip.is_loopback() || ip.is_unspecified() {
            Reach::ThisComputer
        } else if is_local_network(ip) {
            Reach::LocalNetwork(Some(ip))
        } else {
            Reach::Anywhere
        };
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return Reach::ThisComputer;
    }
    let local_suffix = [".local", ".lan", ".home.arpa", ".internal"].iter().any(|s| host.ends_with(s));
    if local_suffix || !host.contains('.') {
        return Reach::LocalNetwork(None);
    }
    Reach::Anywhere
}

/// Whether `ip` is in the shared carrier range 100.64.0.0/10 (a tailnet's addresses, a carrier's NAT).
fn is_shared_range(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
}

/// Whether two addresses share a local network: the same /24 (IPv4) or the same /64 (IPv6). An
/// address in the shared carrier range 100.64.0.0/10 is its own network, shared with nobody else in
/// the range (L4 of 2026-11-21: the range was one network, so any carrier customer counted as
/// local), unless `tailnet` (the owner said this relay is on a Tailscale network, [`TAILNET_ENV`]):
/// then the whole range is that tailnet (L3 of the review of round 5: round 5's rule refused every
/// tailnet friend, whose addresses share no /24 with the node's).
fn same_network(a: IpAddr, b: IpAddr, tailnet: bool) -> bool {
    match (a, b) {
        (IpAddr::V4(x), IpAddr::V4(y)) => {
            if is_shared_range(a) || is_shared_range(b) {
                return a == b || (tailnet && is_shared_range(a) && is_shared_range(b));
            }
            x.octets()[..3] == y.octets()[..3]
        }
        (IpAddr::V6(x), IpAddr::V6(y)) => x.segments()[..4] == y.segments()[..4],
        _ => false,
    }
}

/// Whether a client at `peer` (the connection's own address, [`Arrival::peer`]) may present a
/// signature for a name of `reach`, on a relay whose own local-network addresses are `own_lan` and
/// which is on a Tailscale network when `tailnet`. A loopback client is this computer only, never
/// the local network (H1, 2026-11-21: a Tor-style forwarder on this computer hands every visitor
/// over from 127.0.0.1); a local-network name needs a client on that network: the name's own /24
/// (or /64) for an address, one of the relay's own LAN addresses' for a `.local` or one-word name
/// (L4: any private address took those).
pub fn peer_may_use(reach: Reach, peer: Option<IpAddr>, own_lan: &[IpAddr], tailnet: bool) -> bool {
    // An IPv4 client written as an IPv4-mapped IPv6 address is the same client.
    let peer = peer.map(|p| p.to_canonical());
    match reach {
        Reach::Anywhere => true,
        Reach::ThisComputer => peer.is_some_and(|p| p.is_loopback()),
        Reach::LocalNetwork(name_ip) => peer.is_some_and(|p| {
            !p.is_loopback()
                && is_local_network(p)
                && match name_ip {
                    Some(ip) => same_network(ip, p, tailnet),
                    None => own_lan.iter().any(|ip| same_network(*ip, p, tailnet)),
                }
        }),
    }
}

/// The one sentence for every refusal of an audience, a port, a reach, the owner's door or a
/// signature (L1, 2026-11-21; L4 of the review of round 5): one per case let a prober learn which
/// names a relay answers to (membership was asked before reach and the signature, each with its own
/// words), tying an onion address to its clearnet name or showing a home node's LAN address. The
/// detail goes to the server's log.
pub const REFUSED_AS_REACHED: &str = "This server cannot take this signature: it was not made for this server as you reached it, or it \
     does not match what was sent. Connect again by the address this server's owner gives; if you run this server, its log \
     says why.";

/// The owner's door (M2): its port, and the owner's key, the only key it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerDoor {
    port: u16,
    /// Lowercase; None when no owner key was given (then the door takes no signature at all).
    key: Option<String>,
}

/// The names this relay answers to, lowercase, each `host` or `host:port`; the owner's door when it
/// has one; whether it is on a Tailscale network.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnNames {
    names: Vec<String>,
    door: Option<OwnerDoor>,
    tailnet: bool,
}

impl OwnNames {
    /// The names from the settings `var` reads (the environment, or a test's map), and the loopback
    /// names on `bound`'s port when the dev switch [`DEV_LOOPBACK_NAMES_ENV`] is `1`.
    pub fn from_settings(var: impl Fn(&str) -> Option<String>, bound: std::net::SocketAddr) -> Self {
        let dev = var(DEV_LOOPBACK_NAMES_ENV).is_some_and(|v| v.trim() == "1");
        let mut names = Vec::new();
        if let Some(list) = var(PUBLIC_HOSTS_ENV) {
            names.extend(list.split(',').filter_map(canonical_audience));
        }
        if let Some(list) = var("ALLOWED_ORIGINS") {
            // https only, but behind the dev switch (L6): docs/reference/cryptography.md promises a
            // name from the origins is one a certificate binds.
            names.extend(
                list.split(',')
                    .map(str::trim)
                    .filter(|o| o.starts_with("https://") || (dev && o.starts_with("http://")))
                    .filter_map(humanity_protocol::signed_request::audience_of),
            );
        }
        if let Some(site) = crate::relay::origins::own_site(&var) {
            names.extend(humanity_protocol::signed_request::audience_of(&site));
        }
        // The owner's door: a port that is not the relay's own (that would put the door's names on
        // the main port, where a forwarder's visitors arrive).
        let door = var(OWNER_PORT_ENV).and_then(|p| p.trim().parse::<u16>().ok()).filter(|p| *p != 0 && *p != bound.port()).map(|port| {
            let key = var(OWNER_KEY_ENV).map(|k| k.trim().to_ascii_lowercase()).filter(|k| !k.is_empty());
            OwnerDoor { port, key }
        });
        let tailnet = var(TAILNET_ENV).is_some_and(|v| v.trim() == "1");
        let mut own = Self { names, door, tailnet };
        if dev {
            own.add_loopback(bound.port());
        }
        own.dedupe();
        own
    }

    /// The owner's door's port, when the relay has one (relay/mod.rs listens there).
    pub fn door_port(&self) -> Option<u16> {
        self.door.as_ref().map(|d| d.port)
    }

    /// The owner's door's names: `127.0.0.1`, `localhost` and `[::1]` at its port.
    pub fn door_names(&self) -> Vec<String> {
        self.door.as_ref().map_or_else(Vec::new, |d| loopback_names(d.port).to_vec())
    }

    /// Whether the owner said this relay is on a Tailscale network ([`TAILNET_ENV`]).
    pub fn on_tailnet(&self) -> bool {
        self.tailnet
    }

    /// The loopback names on `bound`'s port alone: the test seam (`serve_relay` and the tests that
    /// build their own router, no settings read), the same as the dev switch.
    pub fn loopback(bound: std::net::SocketAddr) -> Self {
        let mut own = Self::default();
        own.add_loopback(bound.port());
        // In a test build, also the test audience: a name anyone may use, for the tests that stand
        // in for clients behind a proxy (an X-Forwarded-For sent from 127.0.0.1), whose loopback
        // name would rightly be refused from a public address.
        #[cfg(test)]
        own.names.push(TEST_AUDIENCE.to_string());
        own
    }

    fn add_loopback(&mut self, p: u16) {
        self.names.extend(loopback_names(p));
        self.dedupe();
    }

    fn dedupe(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.names.retain(|n| seen.insert(n.clone()));
    }

    /// Whether `audience` is one of this relay's names (it is compared lowercase). Membership only:
    /// a signature is checked by [`Self::admit`], which also asks where it came from.
    pub fn accepts(&self, audience: &str) -> bool {
        canonical_audience(audience).is_some_and(|a| self.names.iter().any(|n| *n == a))
    }

    /// The names, for the log and tests (never for a refusal's words).
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether a signature made for `signed_for` may be checked here, arriving as `arrival`: the
    /// canonical audience to verify it against, or the words to send back (the three questions at the
    /// top of this file). Every refusal of an audience, a port or a reach is the one sentence
    /// [`REFUSED_AS_REACHED`] (L1, 2026-11-21), so the words never tell a prober which names this
    /// relay answers to; the detail, and this relay's names, go to its log.
    pub fn admit(&self, signed_for: &str, arrival: &Arrival) -> Result<String, String> {
        let shown: String = signed_for.chars().filter(|c| !c.is_control()).take(80).collect();
        let Some(aud) = canonical_audience(signed_for) else {
            let sent = if shown.is_empty() { "no address".to_string() } else { shown };
            return Err(format!("This was signed for {sent}, which is not an address, so this server cannot check it."));
        };
        if self.names.is_empty() && self.door.is_none() {
            return Err(
                "This server has not been told its own address, so it accepts no signed request or sign-in. Its owner sets \
                 PUBLIC_HOSTS (or ALLOWED_ORIGINS) to the address people connect to."
                    .to_string(),
            );
        }
        let refuse = |why: String| refused(&aud, arrival, &why, self);
        let host = arrival.host.as_deref().and_then(canonical_audience);
        if host.as_deref() != Some(aud.as_str()) {
            let shown_host = host.as_deref().unwrap_or("no Host");
            if host.as_deref().is_some_and(|h| host_part(h) == host_part(&aud)) {
                return refuse(format!("the connection was opened to {shown_host}, the same name with a different port: {PORT_HINT}"));
            }
            return refuse(format!("the connection was opened to {shown_host}, not that address"));
        }
        // The owner's door takes its own names alone, from a connection on this computer with no
        // forwarding header (M2); its names are taken nowhere else (they are not in `names`).
        if arrival.door == Door::Owner {
            if !self.door_names().contains(&aud) {
                return refuse("through the owner's door only the door's own names are taken".to_string());
            }
            if !arrival.peer.is_some_and(|p| p.to_canonical().is_loopback()) {
                return refuse("the owner's door takes only this computer's own connections, never one through a proxy".to_string());
            }
            return Ok(aud);
        }
        if !self.names.contains(&aud) {
            if self.door_names().contains(&aud) {
                return refuse("that is the owner's door's name, taken only through the door".to_string());
            }
            if self.names.iter().any(|n| host_part(n) == host_part(&aud)) {
                return refuse(format!("one of this relay's names with a different port, or none: {PORT_HINT}"));
            }
            return refuse("not one of this relay's names".to_string());
        }
        let reach = reach_of(&aud);
        if !peer_may_use(reach, arrival.peer, &self.own_lan(), self.tailnet) {
            return refuse(format!(
                "that name is good only from {reach:?}, and the connection's own address (never a header's) is not there"
            ));
        }
        Ok(aud)
    }

    /// [`Self::admit`] for a signature made by `key`: through the owner's door, also the owner's key
    /// alone (M2: a forwarder pointed at the door by mistake hands over visitors who can then use
    /// only what the owner's own app signed for his own door). The same one sentence otherwise.
    pub fn admit_signer(&self, signed_for: &str, arrival: &Arrival, key: &str) -> Result<String, String> {
        let aud = self.admit(signed_for, arrival)?;
        if arrival.door == Door::Owner {
            let owner = self.door.as_ref().and_then(|d| d.key.as_deref());
            if owner != Some(key.trim().to_ascii_lowercase().as_str()) {
                let why = if owner.is_some() { "the owner's door takes the owner's key alone" } else { "the owner's door was given no owner key, so it takes none" };
                return refused(&aud, arrival, why, self);
            }
        }
        Ok(aud)
    }

    /// This relay's own local-network addresses (its names that are a private, link-local or shared
    /// address): the networks a `.local` or one-word name of it may be used from (L4).
    pub fn own_lan(&self) -> Vec<IpAddr> {
        self.names
            .iter()
            .filter_map(|n| match reach_of(n) {
                Reach::LocalNetwork(Some(ip)) => Some(ip),
                _ => None,
            })
            .collect()
    }

    /// Whether a pass (friendship pass, file pass, household permit: pq_crypto.rs) made out for
    /// `aud` is for THIS relay, shown by a client at `peer` (round 3, C8): `aud` is one of this
    /// relay's names, and the client may use that name ([`peer_may_use`]: a loopback name only from
    /// this computer, a LAN name only from that network, by the connection's own address).
    /// The canonical audience when it is.
    ///
    /// Unlike [`Self::admit`], `aud` need not be the address this connection was opened to: a pass
    /// is long-lived and names the address its ISSUER's app opened, and the person holding it may
    /// reach the same relay by another of its names (its onion address, its domain: decided
    /// 2026-11-21, docs/reference/cryptography.md). The relay stays the judge of its own names: a
    /// pass made out at another server, even one claiming this relay's did:hum, names that server's
    /// address and is refused here.
    pub fn admit_pass(&self, aud: &str, peer: Option<IpAddr>) -> Option<String> {
        let aud = canonical_audience(aud)?;
        // The owner's door's names are never a pass's: they are the owner's alone, and a pass is
        // for a friend (host_node.rs makes none naming them).
        if !self.names.contains(&aud) {
            return None;
        }
        peer_may_use(reach_of(&aud), peer, &self.own_lan(), self.tailnet).then_some(aud)
    }
}

/// `127.0.0.1`, `localhost` and `[::1]` at port `p`.
fn loopback_names(p: u16) -> [String; 3] {
    [format!("127.0.0.1:{p}"), format!("localhost:{p}"), format!("[::1]:{p}")]
}

/// Log why a signature made for `aud` was refused, and give the one sentence ([`REFUSED_AS_REACHED`]).
fn refused<T>(aud: &str, arrival: &Arrival, why: &str, own: &OwnNames) -> Result<T, String> {
    let door = own.door_port().map_or(String::new(), |p| format!("; the owner's door is port {p}"));
    tracing::info!(
        "Refused a signature made for {aud} from {:?} by the {:?} door: {why} (this relay answers to {}{door})",
        arrival.peer,
        arrival.door,
        own.names.join(", ")
    );
    Err(REFUSED_AS_REACHED.to_string())
}

/// For the log, when only the port differs (C5): usually a proxy in front of the server that drops
/// the port (nginx's `$host`).
const PORT_HINT: &str = "a proxy in front of the server must pass the port on (nginx: proxy_set_header Host $http_host; \
     docs/admin/SELF-HOSTING.md)";

/// The audience a test's direct call of a handler (no HTTP request, so no `Host`) is checked
/// against, and the one name a relay built without a listener answers to in a test build. A test
/// that calls a route's function directly signs for this; the HTTP-level tests (signed_request_tests,
/// audience_tests) go through the real `Host` path. No product build has it.
#[cfg(test)]
pub const TEST_AUDIENCE: &str = "relay.test";

/// The names `state` answers to: those set at start, else none (in a test build, [`TEST_AUDIENCE`]
/// for a relay built without a listener).
pub fn of(state: &crate::relay::relay::RelayState) -> OwnNames {
    if let Some(names) = state.own_names.get() {
        return names.clone();
    }
    #[cfg(test)]
    return OwnNames { names: vec![TEST_AUDIENCE.to_string()], ..OwnNames::default() };
    #[cfg(not(test))]
    OwnNames::default()
}

/// The relay's own names, read from the environment once at start (relay/mod.rs `run_relay`).
pub fn from_env(bound: std::net::SocketAddr) -> OwnNames {
    let own = OwnNames::from_settings(|k| std::env::var(k).ok(), bound);
    if own.names().is_empty() && own.door.is_none() {
        tracing::warn!(
            "This relay knows no address of its own, so it will refuse every signed request and sign-in. Set PUBLIC_HOSTS \
             (or ALLOWED_ORIGINS) to the address people connect to (docs/admin/SELF-HOSTING.md, Configuration)."
        );
    } else {
        tracing::info!("Signatures are accepted for: {}", own.names().join(", "));
    }
    // A public name beside a local-network one (M1): a forwarder on that network that adds no
    // header (Tor on a router or a Pi, a tunnel's agent on a NAS) hands its visitors over from a
    // local address, so they may use the local names too. Said, not refused: the owner chose it.
    let reaches: Vec<Reach> = own.names().iter().map(|n| reach_of(n)).collect();
    if reaches.contains(&Reach::Anywhere) && reaches.iter().any(|r| matches!(r, Reach::LocalNetwork(_))) {
        tracing::warn!(
            "This relay answers to a public name and to a local-network name. Anything on the local network that forwards \
             the public name's visitors without a header (Tor on a router, Pi or NAS; a tunnel's agent) makes those visitors \
             local, so they may use the local-network names. Point every forwarder at 127.0.0.1 on this computer, or list no \
             local-network name (docs/admin/SELF-HOSTING.md, Configuration)."
        );
    }
    match (&own.door, std::env::var(OWNER_PORT_ENV).ok()) {
        (Some(d), _) => tracing::info!(
            "The owner's door is 127.0.0.1:{} (the owner's own app; it takes {})",
            d.port,
            if d.key.is_some() { "the owner's key alone" } else { "no signature: no owner key was given" }
        ),
        (None, Some(p)) => tracing::warn!("{OWNER_PORT_ENV}={p:?} is not a port other than this relay's own, so no owner's door is opened"),
        (None, None) => {}
    }
    if own.tailnet {
        tracing::info!("On a Tailscale network: clients in 100.64.0.0/10 may use this relay's tailnet names");
    }
    own
}

/// The sign-in check (relay.rs, at `identify_response`): `public_key` in its one canonical form
/// (lowercase hex, D3: the same key in another case would sign in as a second, unbanned account),
/// `audience` admitted for this socket's `arrival` (the address its upgrade was sent to, one of this
/// relay's own names, from a client the name's reach allows, through the owner's door the owner's
/// key alone: [`OwnNames::admit_signer`]), and `sig_b64` the
/// key's Dilithium3 signature over `hum/identify/v2\n{audience}\n{nonce}\n{key}` (verified off the
/// async executor; a panic fails closed). The words to send back otherwise. A server cannot pass this
/// relay's challenge through to a person's app and sign in as them: the app signs only for the
/// address IT opened, and this relay takes only the address ITS socket was opened to.
///
/// Nothing to do after a restart (D6): the nonce is this socket's own, made fresh when it asked to
/// sign in, so a proof made before a restart answers no challenge after it.
pub async fn verify_identify(
    state: &crate::relay::relay::RelayState,
    arrival: &Arrival,
    audience: &str,
    nonce: &str,
    public_key: &str,
    sig_b64: &str,
) -> Result<(), String> {
    if !humanity_protocol::signed_request::is_canonical_key(public_key) {
        return Err("Your key must be a full key written in lowercase hex. Update your app and connect again.".to_string());
    }
    // The audience and the signature are both decided before either answer is given, and a
    // refusal of either is the one sentence (L4 of the review of round 5, 2026-11-21: a failed
    // signature had its own words, so a made-up proof told which names this relay answers to).
    let admitted = of(state).admit_signer(audience, arrival, public_key);
    let aud = match &admitted {
        Ok(a) => a.clone(),
        Err(_) => canonical_audience(audience).unwrap_or_default(),
    };
    let preimage = humanity_protocol::signed_request::identify_preimage(&aud, nonce, public_key);
    let (pk, sig) = (public_key.to_string(), sig_b64.to_string());
    let ok = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        crate::relay::signed_request::note_verified(&pk);
        crate::relay::handlers::broadcast::verify_dilithium_b64(&pk, preimage.as_bytes(), &sig)
    })
    .await
    .unwrap_or(false);
    admitted?;
    if ok {
        Ok(())
    } else {
        tracing::info!("Refused a sign-in made for {aud} from {:?}: the signature does not match", arrival.peer);
        Err(REFUSED_AS_REACHED.to_string())
    }
}

tokio::task_local! {
    /// Where an HTTP request arrived, for `signed_request::check` (set by [`scope_host`]).
    static REQUEST_ARRIVAL: Arrival;
}

/// Middleware over the whole router (inside the edge, so the edge's decision about the connection
/// is there to read): remembers the request's [`Arrival`] (its `Host`, or the authority of its URI,
/// and the connection's own address when no proxy stood between) for the request's own task, what
/// a signed request is checked against.
pub async fn scope_host(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let connection = req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0);
    let decided = req.extensions().get::<crate::relay::client_addr::DirectPeer>().copied();
    let door = req.extensions().get::<Door>().copied();
    let mut arrival = Arrival::from_parts(req.headers(), connection, decided, door);
    if arrival.host.is_none() {
        arrival.host = req.uri().authority().map(|a| a.to_string());
    }
    REQUEST_ARRIVAL.scope(arrival, next.run(req)).await
}

/// Where the current HTTP request arrived, when there is one.
pub fn request_arrival() -> Option<Arrival> {
    REQUEST_ARRIVAL.try_with(|a| a.clone()).ok()
}

tokio::task_local! {
    /// Where the signed-in socket whose frame is being handled arrived (set over the socket's
    /// receive task, relay.rs `handle_connection`): what a pass's audience is checked against
    /// ([`presenter_peer`]).
    static SOCKET_ARRIVAL: Arrival;
}

/// Run a socket's receive loop `work` knowing where the socket arrived, so the handlers it calls
/// can ask [`presenter_peer`].
pub async fn scope_socket<F: std::future::Future>(arrival: Arrival, work: F) -> F::Output {
    SOCKET_ARRIVAL.scope(arrival, work).await
}

/// The connection's own address of whoever is presenting a pass now ([`Arrival::peer`]): the
/// socket's whose frame is being handled, else the HTTP request's. `None` outside both (a test
/// calling a handler directly, a task the handler spawned) and through a proxy, which
/// [`peer_may_use`] reads as "from nowhere near": a pass naming a loopback or LAN name is then
/// refused, one naming a public name taken.
pub fn presenter_peer() -> Option<IpAddr> {
    SOCKET_ARRIVAL.try_with(|a| a.peer).ok().flatten().or_else(|| request_arrival().and_then(|a| a.peer))
}

/// Whether the pass audience `aud` is this relay's and may be used by whoever is presenting now
/// ([`OwnNames::admit_pass`] with [`presenter_peer`]): the one question every pass check asks
/// first (handlers/friend_passes.rs, handlers/reach.rs file passes, handlers/shared_build.rs).
pub fn pass_is_for_here(state: &crate::relay::relay::RelayState, aud: &str) -> Option<String> {
    of(state).admit_pass(aud, presenter_peer())
}

/// For tests: the name a test mints passes for at `state`, one any client may use (the test
/// audience, or a public name), else the first name.
#[cfg(test)]
pub fn test_pass_audience(state: &crate::relay::relay::RelayState) -> String {
    let own = of(state);
    own.names().iter().find(|n| reach_of(n) == Reach::Anywhere).or(own.names().first()).cloned().expect("a test relay has a name")
}

/// For tests: the `identify_response` a real app sends to the relay it opened at `opened` (a URL or
/// `host:port`; a test relay's is `127.0.0.1:{port}`), from the keypair `dil` whose key hex is `key`.
#[cfg(test)]
pub fn test_identify_response(dil: &humanity_protocol::pq_crypto::DilithiumKeypair, opened: &str, nonce: &str, key: &str) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let aud = humanity_protocol::signed_request::audience_of(opened).expect("a test relay's address");
    let words = humanity_protocol::signed_request::identify_preimage(&aud, nonce, key);
    serde_json::json!({ "type": "identify_response", "sig_b64": B64.encode(dil.sign(words.as_bytes())), "audience": aud })
}

/// For tests: a request signature (hex) from `dil` for the relay at `opened` (a URL or `host:port`).
#[cfg(test)]
pub fn test_request_sig(dil: &humanity_protocol::pq_crypto::DilithiumKeypair, opened: &str, purpose: &str, ts: u64) -> String {
    let aud = humanity_protocol::signed_request::audience_of(opened).expect("a test relay's address");
    hex::encode(dil.sign(humanity_protocol::signed_request::request_preimage(&aud, purpose, ts).as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn settings(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: std::collections::HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| map.get(k).cloned()
    }

    fn arrival(host: &str, peer: &str) -> Arrival {
        Arrival { host: Some(host.to_string()), peer: peer.parse().ok(), ..Default::default() }
    }

    #[test]
    fn the_live_servers_settings_name_its_addresses() {
        let bound: SocketAddr = "0.0.0.0:3210".parse().unwrap();
        let own = OwnNames::from_settings(
            settings(&[("ALLOWED_ORIGINS", "https://united-humanity.us,https://chat.united-humanity.us"), ("TURN_PUBLIC_HOST", "united-humanity.us")]),
            bound,
        );
        assert_eq!(own.names(), ["united-humanity.us", "chat.united-humanity.us"]);
        assert!(own.accepts("united-humanity.us"));
        assert!(own.accepts("Chat.United-Humanity.US"), "compared lowercase");
        assert!(!own.accepts("127.0.0.1:3210"), "a public relay never takes a loopback name on its own");
        assert!(!own.accepts("evil.example"));
        assert_eq!(own.admit("united-humanity.us", &arrival("united-humanity.us", "203.0.113.9")).as_deref(), Ok("united-humanity.us"));
    }

    /// C2: no automatic shared names. A relay listening on loopback (a public relay behind nginx or
    /// Tor is one) has no loopback name unless the dev switch says so. Red evidence (2026-11-21, with
    /// round 2's loopback names from the bind address put back): "a loopback-bound relay without
    /// the dev switch has no loopback name" failed, left ["relay.example.org", "127.0.0.1:3210",
    /// "localhost:3210", "[::1]:3210"].
    #[test]
    fn a_loopback_bound_relay_has_no_loopback_name_without_the_dev_switch() {
        let bound: SocketAddr = "127.0.0.1:3210".parse().unwrap();
        let public = OwnNames::from_settings(settings(&[(PUBLIC_HOSTS_ENV, "relay.example.org")]), bound);
        assert_eq!(public.names(), ["relay.example.org"], "a loopback-bound relay without the dev switch has no loopback name");
        assert!(public.admit("127.0.0.1:3210", &arrival("127.0.0.1:3210", "127.0.0.1")).is_err());
        let dev = OwnNames::from_settings(settings(&[(DEV_LOOPBACK_NAMES_ENV, "1")]), bound);
        for n in ["127.0.0.1:3210", "localhost:3210", "[::1]:3210"] {
            assert!(dev.accepts(n), "{n}");
        }
        assert!(!dev.accepts("127.0.0.1:3211"), "another port is another server");
    }

    /// C2: a name that means "this computer" or "this network" is taken only from a client there.
    /// Red evidence (2026-11-21, `peer_may_use` made to answer true for every reach): the first
    /// refusal, "a loopback name from the LAN", failed.
    #[test]
    fn local_names_are_taken_only_from_local_clients() {
        let own = OwnNames::from_settings(
            settings(&[(PUBLIC_HOSTS_ENV, "127.0.0.1:3210,192.168.1.5:3210,mypc.local:3210,node.example.org:3210")]),
            "0.0.0.0:3210".parse().unwrap(),
        );
        // This computer's name: only from this computer.
        assert!(own.admit("127.0.0.1:3210", &arrival("127.0.0.1:3210", "127.0.0.1")).is_ok());
        assert!(own.admit("127.0.0.1:3210", &arrival("127.0.0.1:3210", "192.168.1.7")).is_err(), "a loopback name from the LAN");
        assert!(own.admit("127.0.0.1:3210", &arrival("127.0.0.1:3210", "203.0.113.9")).is_err());
        assert!(own.admit("127.0.0.1:3210", &Arrival { host: Some("127.0.0.1:3210".into()), peer: None, ..Default::default() }).is_err(), "no address is not here");
        // A LAN name: from the same network, never from this computer (H1, 2026-11-21: a Tor-style
        // forwarder hands every visitor over from 127.0.0.1).
        assert!(own.admit("192.168.1.5:3210", &arrival("192.168.1.5:3210", "192.168.1.7")).is_ok());
        assert!(own.admit("192.168.1.5:3210", &arrival("192.168.1.5:3210", "::ffff:192.168.1.7")).is_ok(), "a mapped address is the same client");
        assert!(own.admit("192.168.1.5:3210", &arrival("192.168.1.5:3210", "127.0.0.1")).is_err(), "a LAN name from this computer");
        let far = own.admit("192.168.1.5:3210", &arrival("192.168.1.5:3210", "203.0.113.9"));
        assert_eq!(far.unwrap_err(), REFUSED_AS_REACHED, "a LAN name from a public client is refused");
        assert!(own.admit("192.168.1.5:3210", &arrival("192.168.1.5:3210", "192.168.2.7")).is_err(), "another household's network");
        // A .local name: from the network of one of the node's LAN addresses (L4).
        assert!(own.admit("mypc.local:3210", &arrival("mypc.local:3210", "192.168.1.9")).is_ok());
        assert!(own.admit("mypc.local:3210", &arrival("mypc.local:3210", "10.0.0.4")).is_err(), "another private network");
        assert!(own.admit("mypc.local:3210", &arrival("mypc.local:3210", "198.51.100.1")).is_err());
        // A public name: from anyone.
        assert!(own.admit("node.example.org:3210", &arrival("node.example.org:3210", "198.51.100.1")).is_ok());
    }

    /// C1 at the word level: the audience must be the address the connection was opened to, and the
    /// refusal names nothing (C4; since L1 one sentence for every case). Red evidence (2026-11-21,
    /// the Host comparison taken out of `admit`): "signed for a listed name but arriving under another
    /// Host" passed admit.
    #[test]
    fn the_audience_must_be_the_address_the_connection_was_opened_to() {
        let own = OwnNames::from_settings(
            settings(&[(PUBLIC_HOSTS_ENV, "abcdefghijklmnop.onion"), ("ALLOWED_ORIGINS", "https://relay.example.org")]),
            "127.0.0.1:3210".parse().unwrap(),
        );
        let words = own
            .admit("relay.example.org", &arrival("abcdefghijklmnop.onion", "127.0.0.1"))
            .expect_err("signed for a listed name but arriving under another Host");
        assert_eq!(words, REFUSED_AS_REACHED);
        let words = own.admit("evil.example", &arrival("evil.example", "203.0.113.9")).unwrap_err();
        assert_eq!(words, REFUSED_AS_REACHED, "refusal words do not list the server's names");
        assert!(OwnNames::default().admit("a.example", &arrival("a.example", "203.0.113.9")).unwrap_err().contains("PUBLIC_HOSTS"));
    }

    /// C5: a proxy that drops the port (nginx's `$host`) is refused, and since L1 (2026-11-21) in
    /// the one sentence every audience refusal has, the port and the nginx line going to the log
    /// (`PORT_HINT`), where the server's owner reads them; the words told a prober the name was one
    /// of this server's. Red evidence (2026-11-21, before `port_words`): the words said "not this
    /// server's address"; since L1, `every_audience_port_or_reach_refusal_has_the_same_words`.
    #[test]
    fn a_port_mismatch_is_refused_in_the_one_sentence() {
        let own = OwnNames::from_settings(settings(&[(PUBLIC_HOSTS_ENV, "example.org:8443")]), "0.0.0.0:3210".parse().unwrap());
        // An HTTP request: the proxy forwarded the Host without the port.
        assert_eq!(own.admit("example.org", &arrival("example.org", "203.0.113.9")).unwrap_err(), REFUSED_AS_REACHED);
        // A sign-in signed for the port, over a socket whose Host lost it.
        assert_eq!(own.admit("example.org:8443", &arrival("example.org", "203.0.113.9")).unwrap_err(), REFUSED_AS_REACHED);
        assert!(PORT_HINT.contains("$http_host"), "the log names the fix");
    }

    #[test]
    fn public_hosts_add_names_and_junk_is_ignored() {
        let own = OwnNames::from_settings(
            settings(&[(PUBLIC_HOSTS_ENV, " Example.org:8443 , abc.onion,bad host, ,http://x/y"), ("ALLOWED_ORIGINS", "tauri://localhost")]),
            "0.0.0.0:3210".parse().unwrap(),
        );
        assert_eq!(own.names(), ["example.org:8443", "abc.onion"]);
    }

    #[test]
    fn reaches_of_names() {
        assert_eq!(reach_of("localhost:1"), Reach::ThisComputer);
        assert_eq!(reach_of("[::1]:1"), Reach::ThisComputer);
        assert_eq!(reach_of("0.0.0.0:1"), Reach::ThisComputer);
        assert_eq!(reach_of("10.1.2.3"), Reach::LocalNetwork(Some("10.1.2.3".parse().unwrap())));
        assert_eq!(reach_of("[fd00::1]:3210"), Reach::LocalNetwork(Some("fd00::1".parse().unwrap())));
        assert_eq!(reach_of("100.101.1.2:3210"), Reach::LocalNetwork(Some("100.101.1.2".parse().unwrap())));
        assert_eq!(reach_of("router.home.arpa"), Reach::LocalNetwork(None));
        assert_eq!(reach_of("mypc:3210"), Reach::LocalNetwork(None));
        assert_eq!(reach_of("united-humanity.us"), Reach::Anywhere);
        assert_eq!(reach_of("203.0.113.9:3210"), Reach::Anywhere);
        assert_eq!(reach_of("abc.onion"), Reach::Anywhere);
    }
}
