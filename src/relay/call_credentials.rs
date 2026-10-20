//! Credentials for the call forwarder (call_forwarder.rs), given out over the signed-in socket
//! only (docs/design/blocking-and-safe-mode.md 10f; files: docs/design/files-showcase.md,
//! "Device-to-device big files: the shapes, exact", part 2).
//!
//! The request, from a signed-in socket:
//!
//! ```text
//! {"type":"call_credentials","room":"<voice room id>"}
//! {"type":"call_credentials","call":"<the other person's key>"}
//! {"type":"call_credentials","file":"<transfer id>","peer":"<the other person's key>","file_pass":{...}}
//! ```
//!
//! answered only when the asker is in that voice room's live roster (`RelayState::voice_rooms`),
//! in an open call with that person (`handlers/reach.rs`, a ring that was let through), or, for a
//! file, holds a file pass for it (pq_crypto `verify_file_pass`) that checks with (issuer,
//! grantee) = (peer, asker) or (asker, peer) and whose `id` is the `file` asked for (32 lowercase
//! hex; never with the asker as their own peer), with
//!
//! ```text
//! {"type":"call_credentials","room"|"call"|"file":...,
//!  "urls":["turn:<host>:<port>?transport=udp","stun:<host>:<port>"],
//!  "username":"<expiry>:<scope>-<nonce>[-<person tag>-<MB a day>]","credential":"<base64 HMAC-SHA1>","ttl":3600}
//! ```
//!
//! to that person's own sockets. Anything else is answered at once with
//! `{"type":"call_credentials","room"|"call"|"file":<as asked>,"refused":true}` (no urls, username
//! or credential), so the client stops waiting, and a Private notice saying why.
//!
//! # What the credential is
//!
//! `username` is `"{expiry}:{scope}-{nonce}"`: the Unix time it stops working, a hash of the room
//! (or of the two call keys, sorted; or of the transfer id and the two keys, sorted) and 8 random
//! bytes so that no two requests get the same one. A FILE credential carries two more parts,
//! `"-{tag}-{limit}"`: a keyed tag of the person it was issued to (32 hex, [`CallKeys::person_tag`])
//! and their role's `max_forwarded_mb_per_day` (0 = no limit). `credential` is the base64
//! HMAC-SHA1 of `username` under a 32-byte secret made when the relay starts and never written
//! anywhere ([`CallKeys`]). So a credential is good for one room, call or transfer on this run of
//! this relay: the forwarder recomputes the HMAC to check it, reads the scope (and a file's tag
//! and limit) from the username it covers, and every credential dies with the process. The scope
//! hash and the tag are keyed by the same secret, so a username says nothing about which room or
//! whom it is for to anyone else.
//!
//! An allocation made with a credential belongs to its scope, and the forwarder only ever passes
//! packets between allocations of the same scope. The forwarder counts what each person tag's
//! allocations send through it in a UTC day and stops relaying for that tag past its limit; a
//! call's or a room's credentials carry no tag and are never counted.

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::relay::relay::{RelayMessage, RelayState};

/// How long a credential can start an allocation, in seconds. It is the `ttl` the reply carries.
pub const CREDENTIAL_TTL_SECS: u64 = 3600;

/// How long the forwarder accepts a nonce it handed out, in seconds (a client given 438 Stale
/// Nonce retries with the new one).
pub const NONCE_LIFETIME_SECS: u64 = 600;

/// Credentials one person may be given a minute. A browser asks once per room or call it joins;
/// this is only there so a loop in a client cannot spin the server.
const ISSUES_PER_MINUTE: usize = 20;

/// The longest room id or key a request may name. Room ids are channel ids; a key is a
/// Dilithium3 public key in hex (3,904 characters).
const MAX_ID_CHARS: usize = 8192;

/// How many people's issue times are remembered at once before the idle ones are dropped.
const ISSUE_TRACK_MAX: usize = 10_000;

/// The notice for a request from someone not in that room or call.
pub const NOT_IN_IT: &str = "No call credentials: you are not in that voice room or call.";
/// The notice for a file request whose pass is missing, does not check out, is not for this
/// transfer or does not name the asker (device-to-device big files).
pub const NO_FILE_PASS: &str = "No credentials for that transfer: its file pass is missing, run out, or not between you and that person.";
/// The notice when this relay is not running the forwarder (voice is off, or its UDP port could
/// not be opened). The same sentence the clients show when a call cannot be put through.
pub const NOT_SET_UP: &str =
    "Calls go through the server to keep your address private; this server is not set up for that yet.";
/// The notice past [`ISSUES_PER_MINUTE`].
pub const TOO_MANY: &str = "Too many requests for call credentials; try again in a minute.";

/// The secret every credential and nonce of this run is made with, and the two keys derived
/// from it. Made at start ([`CallKeys::generate`]) and never stored, so nothing on disk can
/// mint a credential and a restart ends every one.
#[derive(Clone)]
pub struct CallKeys {
    secret: [u8; 32],
    scope_key: [u8; 32],
    nonce_key: [u8; 32],
    /// Keys the person tag a file credential carries ([`CallKeys::person_tag`]).
    tag_key: [u8; 32],
}

impl std::fmt::Debug for CallKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CallKeys(secret not shown)")
    }
}

impl CallKeys {
    /// Fresh keys from the operating system's random numbers (rand's thread RNG, a CSPRNG seeded
    /// from the OS).
    pub fn generate() -> CallKeys {
        use rand::RngCore;
        let mut secret = [0u8; 32];
        rand::rng().fill_bytes(&mut secret);
        CallKeys {
            secret,
            scope_key: blake3::derive_key("hum/call-scope/v1", &secret),
            nonce_key: blake3::derive_key("hum/call-nonce/v1", &secret),
            tag_key: blake3::derive_key("hum/call-person-tag/v1", &secret),
        }
    }

    /// The password for `username`: base64(HMAC-SHA1(secret, username)), the scheme coturn calls
    /// `use-auth-secret`, so any TURN client that speaks it can use ours.
    pub fn credential_for(&self, username: &str) -> String {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<sha1::Sha1>::new_from_slice(&self.secret).expect("HMAC accepts any key length");
        mac.update(username.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    fn scope(&self, words: &str) -> String {
        hex::encode(&blake3::keyed_hash(&self.scope_key, words.as_bytes()).as_bytes()[..16])
    }

    /// The scope of voice room `room`: 32 hex characters.
    pub fn room_scope(&self, room: &str) -> String {
        self.scope(&format!("room\n{room}"))
    }

    /// The scope of the call between `a` and `b`, the same whichever of them asks.
    pub fn call_scope(&self, a: &str, b: &str) -> String {
        let (x, y) = if a <= b { (a, b) } else { (b, a) };
        self.scope(&format!("call\n{x}\n{y}"))
    }

    /// The scope of transfer `id` between `a` and `b` (device-to-device big files), the same
    /// whichever of them asks: a keyed hash of `file\n{id}\n{the two keys, sorted}`, so the
    /// forwarder passes packets only between the two people's allocations for this one transfer.
    pub fn file_scope(&self, id: &str, a: &str, b: &str) -> String {
        let (x, y) = if a <= b { (a, b) } else { (b, a) };
        self.scope(&format!("file\n{id}\n{x}\n{y}"))
    }

    /// The tag a file credential issued to `key` carries: 32 hex characters, a keyed hash of the
    /// key, the same for every credential the person is given on this run (so the forwarder counts
    /// all their transfers together) and meaningless to anyone without this run's secret.
    pub fn person_tag(&self, key: &str) -> String {
        hex::encode(&blake3::keyed_hash(&self.tag_key, format!("person\n{key}").as_bytes()).as_bytes()[..16])
    }

    /// A username and credential for `scope`, good for [`CREDENTIAL_TTL_SECS`] from `unix_now`.
    pub fn issue(&self, scope: &str, unix_now: u64) -> (String, String) {
        let username = format!("{}:{scope}-{}", unix_now + CREDENTIAL_TTL_SECS, random_nonce());
        let credential = self.credential_for(&username);
        (username, credential)
    }

    /// A FILE credential for `scope`, issued to `key` whose role may send `limit_mb` MB a day
    /// through the forwarder (0 = no limit): the username carries [`person_tag`](Self::person_tag)
    /// of `key` and the limit, inside the part the HMAC covers, so neither can be changed.
    pub fn issue_metered(&self, scope: &str, key: &str, limit_mb: u64, unix_now: u64) -> (String, String) {
        let username = format!("{}:{scope}-{}-{}-{limit_mb}", unix_now + CREDENTIAL_TTL_SECS, random_nonce(), self.person_tag(key));
        let credential = self.credential_for(&username);
        (username, credential)
    }

    fn nonce_mac(&self, ts: u32, src: SocketAddr) -> String {
        let words = format!("{ts}\n{src}");
        hex::encode(&blake3::keyed_hash(&self.nonce_key, words.as_bytes()).as_bytes()[..8])
    }

    /// The NONCE the forwarder hands `src` in a 401 or 438: the time in hex and a MAC over that
    /// time and `src`, so the forwarder keeps nothing per stranger and a nonce sent to one
    /// address is no use from another. Only a client that receives packets at `src` learns it,
    /// which is what makes an allocation's address one that asked for it.
    pub fn nonce_for(&self, src: SocketAddr, unix_now: u64) -> String {
        let ts = unix_now as u32;
        format!("{ts:08x}{}", self.nonce_mac(ts, src))
    }

    /// Whether `nonce` is one [`nonce_for`](Self::nonce_for) gave `src` within
    /// [`NONCE_LIFETIME_SECS`]. Constant time in the MAC.
    pub fn nonce_ok(&self, nonce: &[u8], src: SocketAddr, unix_now: u64) -> bool {
        let Ok(nonce) = std::str::from_utf8(nonce) else { return false };
        if nonce.len() != 24 {
            return false;
        }
        let Ok(ts) = u32::from_str_radix(&nonce[..8], 16) else { return false };
        let now = unix_now as u32;
        // Fresh: made within the lifetime, or a few seconds "ahead" if the clock stepped back.
        let fresh = if ts <= now { now - ts < NONCE_LIFETIME_SECS as u32 } else { ts - now <= 5 };
        if !fresh {
            return false;
        }
        let want = self.nonce_mac(ts, src);
        want.len() == nonce[8..].len()
            && want.bytes().zip(nonce[8..].bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

/// 8 random bytes as 16 hex characters: what keeps two usernames for one scope apart.
fn random_nonce() -> String {
    use rand::RngCore;
    let mut nonce = [0u8; 8];
    rand::rng().fill_bytes(&mut nonce);
    hex::encode(nonce)
}

/// Who a file credential's allocations are counted against, and how much they may send through
/// the forwarder in a UTC day: read from the username, where the HMAC covers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meter {
    /// [`CallKeys::person_tag`] of the person it was issued to.
    pub tag: String,
    /// Their role's `max_forwarded_mb_per_day` when it was issued; 0 means no limit.
    pub limit_mb: u64,
}

impl Meter {
    /// The day's limit in bytes, or None for no limit.
    pub fn limit_bytes(&self) -> Option<u64> {
        (self.limit_mb > 0).then(|| self.limit_mb.saturating_mul(1024 * 1024))
    }
}

/// The parts of a username in this relay's shape: `"{expiry}:{32 hex}-{16 hex}"`, or a file
/// credential's `"{expiry}:{32 hex}-{16 hex}-{32 hex}-{digits}"`.
fn username_parts(username: &str) -> Option<(u64, &str, Option<Meter>)> {
    let (expiry, rest) = username.split_once(':')?;
    let digits = |s: &str| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(expiry) {
        return None;
    }
    let lower_hex = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let mut parts = rest.split('-');
    let (scope, nonce) = (parts.next()?, parts.next()?);
    if !(lower_hex(scope, 32) && lower_hex(nonce, 16)) {
        return None;
    }
    let meter = match (parts.next(), parts.next(), parts.next()) {
        (None, _, _) => None,
        (Some(tag), Some(limit), None) if lower_hex(tag, 32) && digits(limit) => {
            Some(Meter { tag: tag.to_string(), limit_mb: limit.parse().ok()? })
        }
        _ => return None,
    };
    Some((expiry.parse().ok()?, scope, meter))
}

/// The expiry and scope a username carries, when it has this relay's shape (a call's, a room's or
/// a file's).
pub fn parse_username(username: &str) -> Option<(u64, &str)> {
    username_parts(username).map(|(expiry, scope, _)| (expiry, scope))
}

/// The person tag and day limit a FILE credential's username carries; None for a call's or a
/// room's (never counted) and for anything not in this relay's shape.
pub fn username_meter(username: &str) -> Option<Meter> {
    username_parts(username).and_then(|(_, _, meter)| meter)
}

/// The forwarder's side of the relay state: its keys, where it listens once it does, and how many
/// credentials each person was given in the last minute.
pub struct CallService {
    keys: CallKeys,
    listening: Mutex<Option<(String, u16)>>,
    issued: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Default for CallService {
    fn default() -> Self {
        CallService { keys: CallKeys::generate(), listening: Mutex::new(None), issued: Mutex::new(HashMap::new()) }
    }
}

/// A host as it goes in a `turn:` or `stun:` URL: an IPv6 address in brackets.
fn url_host(host: &str) -> String {
    if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

impl CallService {
    pub fn keys(&self) -> &CallKeys {
        &self.keys
    }

    /// The forwarder is listening on `port`, and clients reach it at `host`.
    pub fn set_listening(&self, host: &str, port: u16) {
        *self.listening.lock().unwrap_or_else(|p| p.into_inner()) = Some((host.to_string(), port));
    }

    fn listening(&self) -> Option<(String, u16)> {
        self.listening.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// The `urls` a credential reply carries, or `None` when the forwarder is not listening.
    pub fn urls(&self) -> Option<Vec<String>> {
        let (host, port) = self.listening()?;
        let h = url_host(&host);
        Some(vec![format!("turn:{h}:{port}?transport=udp"), format!("stun:{h}:{port}")])
    }

    /// Our own STUN entry, the only thing `/api/turn-credentials` offers, or `None` when the
    /// forwarder is not listening.
    pub fn stun_url(&self) -> Option<String> {
        let (host, port) = self.listening()?;
        Some(format!("stun:{}:{port}", url_host(&host)))
    }

    /// May `key` be given another credential now? Counts it when it may.
    fn may_issue(&self, key: &str, now: Instant) -> bool {
        let window = Duration::from_secs(60);
        let mut issued = self.issued.lock().unwrap_or_else(|p| p.into_inner());
        if issued.len() >= ISSUE_TRACK_MAX && !issued.contains_key(key) {
            issued.retain(|_, times| times.back().is_some_and(|t| now.duration_since(*t) < window));
            if issued.len() >= ISSUE_TRACK_MAX {
                return false;
            }
        }
        let times = issued.entry(key.to_string()).or_default();
        while times.front().is_some_and(|t| now.duration_since(*t) >= window) {
            times.pop_front();
        }
        if times.len() >= ISSUES_PER_MINUTE {
            return false;
        }
        times.push_back(now);
        true
    }
}

/// The reply's fields after `"type":"call_credentials"`: `room`, `call` or `file` as asked, then the
/// servers, the username, the credential and how many seconds it is good for. A refusal is the
/// same reply with `"refused":true` and none of the other four, sent at once so a client stops
/// waiting (the web client reads a reply without them as "no credentials").
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CallCredentials {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    /// The transfer id a file request named (device-to-device big files), echoed as asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub credential: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ttl: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refused: bool,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The `room`, `call`, `file` or `peer` a request names (non-empty, not absurdly long).
fn named<'a>(raw: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    raw.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty() && s.len() <= MAX_ID_CHARS)
}

/// What a request asked for, echoed in its reply: a voice room, a call or a file transfer.
#[derive(Clone, Copy)]
struct Asked<'a> {
    room: Option<&'a str>,
    call: Option<&'a str>,
    file: Option<&'a str>,
}

impl<'a> Asked<'a> {
    fn of(raw: &'a serde_json::Value) -> Asked<'a> {
        Asked { room: named(raw, "room"), call: named(raw, "call"), file: named(raw, "file") }
    }

    /// A reply carrying what was asked and nothing else yet.
    fn reply(self) -> CallCredentials {
        CallCredentials {
            room: self.room.map(str::to_string),
            call: self.call.map(str::to_string),
            file: self.file.map(str::to_string),
            ..Default::default()
        }
    }
}

/// Refuse `who`: the refusal reply (echoing `room`, `call` and `file` as asked) so the client
/// stops waiting, and a notice saying why.
fn refuse(state: &RelayState, who: &str, asked: Asked<'_>, message: &str) {
    let creds = CallCredentials { refused: true, ..asked.reply() };
    let _ = state.broadcast_tx.send(RelayMessage::CallCredentials { to: who.to_string(), creds });
    let _ = state.broadcast_tx.send(RelayMessage::Private { to: who.to_string(), message: message.to_string() });
}

/// The refusal reply alone, for a request the WS feature gate turned away because the owner
/// switched voice off (relay.rs; the gate sends its own notice).
/// The notice for a file's credentials on a server whose owner turned voice off: the forwarder
/// that carries calls carries these files too, and it does not run without voice.
pub const FILES_NEED_VOICE: &str = "This server does not carry files sent device to device: its owner turned off voice, and the same call relay carries both.";

pub fn refused_reply(state: &RelayState, who: &str, raw: &serde_json::Value) {
    let creds = CallCredentials { refused: true, ..Asked::of(raw).reply() };
    let _ = state.broadcast_tx.send(RelayMessage::CallCredentials { to: who.to_string(), creds });
}

/// Is `key` in voice room `room` right now?
async fn in_voice_room(state: &RelayState, key: &str, room: &str) -> bool {
    state.voice_rooms.read().await.get(room).is_some_and(|r| r.participants.iter().any(|(k, _)| k == key))
}

/// Does `file_pass` let `me` and `peer` have forwarder credentials for transfer `file`? The pass
/// must check out on this relay with (issuer, grantee) = (peer, me) or (me, peer), its `id` must
/// be `file`, and `peer` must be someone else (the shapes, part 2). Either of the two may ask: the
/// sender (the pass's issuer) and the receiver (its grantee) each need an allocation.
fn file_pass_fits(state: &RelayState, me: &str, peer: &str, peer_known: bool, file: &str, file_pass: Option<&serde_json::Value>) -> bool {
    use crate::relay::handlers::reach::file_pass_of;
    // The other person must be known here (a name on this server, or signed in now): a pass the
    // asker signs to a key nobody holds, or to text that is not a key, gets nothing (the review of
    // 2026-10-12). This is not the other person's consent (the asker can sign a pass naming any
    // known key); what bounds such use is the forwarded day amount and the per-person allocations.
    me != peer
        && peer_known
        && [(peer, me), (me, peer)]
            .into_iter()
            .any(|(issuer, grantee)| file_pass_of(state, issuer, grantee, file_pass).is_some_and(|p| p.id == file))
}

/// What `key`'s role may send through the forwarder a day, in MB (0 = no limit): their role's
/// `max_forwarded_mb_per_day`, as it stands when the credential is issued.
fn forwarded_limit_mb(state: &RelayState, key: &str) -> u64 {
    state.db.role_def(&state.db.get_role(key).unwrap_or_default()).max_forwarded_mb_per_day.max(0) as u64
}

/// `call_credentials` from the signed-in `my_key`. Exactly one of `room`, `call` and `file`; the
/// asker must be in that room, in an open call with that person, or hold a file pass for that
/// transfer between them and `peer` ([`file_pass_fits`]); the forwarder must be listening; and no
/// more than [`ISSUES_PER_MINUTE`] a minute. Otherwise a refusal and a notice. A file credential
/// carries the asker's person tag and day limit ([`CallKeys::issue_metered`]); the others none.
pub async fn handle(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    let asked = Asked::of(raw);
    // Every request counts toward the limit, refused or not, and before any signature is checked:
    // a stream of bad file passes costs the server two signature checks each otherwise.
    if !state.calls.may_issue(my_key, Instant::now()) {
        return refuse(state, my_key, asked, TOO_MANY);
    }
    let (scope, metered) = match (asked.room, asked.call, asked.file) {
        (Some(r), None, None) if in_voice_room(state, my_key, r).await => (state.calls.keys().room_scope(r), false),
        (None, Some(c), None) if crate::relay::handlers::reach::call_is_open(state, my_key, c) => {
            (state.calls.keys().call_scope(my_key, c), false)
        }
        (None, None, Some(f)) => {
            let peer = named(raw, "peer");
            let peer_known = match peer {
                Some(p) => state.db.current_name_for_key(p).ok().flatten().is_some() || state.peers.read().await.contains_key(p),
                None => false,
            };
            match peer.filter(|p| file_pass_fits(state, my_key, p, peer_known, f, raw.get("file_pass"))) {
                Some(p) => (state.calls.keys().file_scope(f, my_key, p), true),
                None => return refuse(state, my_key, asked, NO_FILE_PASS),
            }
        }
        _ => return refuse(state, my_key, asked, NOT_IN_IT),
    };
    let Some(urls) = state.calls.urls() else { return refuse(state, my_key, asked, NOT_SET_UP) };
    let (username, credential) = if metered {
        state.calls.keys().issue_metered(&scope, my_key, forwarded_limit_mb(state, my_key), unix_now())
    } else {
        state.calls.keys().issue(&scope, unix_now())
    };
    let creds = CallCredentials { urls, username, credential, ttl: CREDENTIAL_TTL_SECS, refused: false, ..asked.reply() };
    let _ = state.broadcast_tx.send(RelayMessage::CallCredentials { to: my_key.to_string(), creds });
}

#[cfg(test)]
#[path = "call_credentials_tests.rs"]
mod tests;
