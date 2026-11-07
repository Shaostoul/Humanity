//! Mail to my own mailbox waits a random while before it goes (2026-11-06, docs/BUGS.md BUG-259).
//! The website's twin is `web/shared/self-mail.js`, held to these limits by
//! `scripts/tests/self-mail-web.test.js`.
//!
//! WHY. A private message is two sealed envelopes: the recipient's copy, and my own copy in my own
//! mailbox so my other devices can show what I sent (the same for a follow, a friendship pass, a
//! permit, a file offer, a choice note). Both apps used to send the two one right after the other,
//! and the relay numbered mailbox rows in arrival order, so a copy of its database (a court order,
//! a breach, a backup with its key) showed two neighbouring envelopes of the same size, one for
//! the recipient and one for me, and paired who wrote to whom. The relay now keeps rows by mailbox
//! with nothing finer than the day (`src/relay/storage/dms.rs`). This is the other half: my own
//! copy leaves a random time between [`SELF_MAIL_DELAY_MIN_MS`] and [`SELF_MAIL_DELAY_MAX_MS`]
//! later, so it does not land beside the recipient's copy in the relay's write log or its
//! continuous backup either, wherever other mail lands between.
//!
//! WHY THE APP AND NOT THE RELAY. The relay already learns, at the moment of sending, who is
//! sending to whom (the put arrives on the sender's signed-in connection); nothing here changes
//! that. What it changes is what is kept: the relay never sees the two copies together, so even
//! its write order and a network watcher's view of two same-size frames in a row stop pairing
//! them. Keeping my copy off the relay entirely (my devices syncing among themselves) would leave
//! it knowing less still, but there is no such route that works with a device asleep (the
//! website's device-to-device sync needs both online): the follow-up is written in BUG-259.
//!
//! THE RULE, held by [`crate::net::ws_client::WsClient::send`]: every `dm_put` addressed to my own
//! key waits, in order (each leaves no earlier than the one before it). A note that pairs with
//! nothing and keeps its own "not sent yet" mark until it is sent (a block note, a task note) uses
//! `WsClient::send_now`, which sends at once: holding it would only add a window in which a crash
//! loses it.
//!
//! Kept until it goes: a socket not connected when a copy is due keeps it until it is; a client
//! dropped while holding copies sends them if its socket is still up (the app closing, a server
//! left), and otherwise leaves them for the next client made for the same server and identity
//! (a reconnect), which sends them when they are due. Only a crash in the minute or two before a
//! copy leaves loses it, and then only my OTHER devices miss it: this one keeps its own copy.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The shortest wait, in milliseconds (the website's `SELF_MAIL_DELAY_MIN_MS`).
pub const SELF_MAIL_DELAY_MIN_MS: u64 = 20_000;
/// The longest wait, in milliseconds (the website's `SELF_MAIL_DELAY_MAX_MS`).
pub const SELF_MAIL_DELAY_MAX_MS: u64 = 120_000;

/// True when `frame` is a `dm_put` addressed to `me`.
pub fn is_self_put(frame: &str, me: &str) -> bool {
    if me.is_empty() || !frame.contains("\"dm_put\"") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(frame) else { return false };
    v["type"] == "dm_put" && v["to"].as_str() == Some(me)
}

/// A random wait between the two limits, from the system's randomness (the longest wait when
/// there is none: waiting too long only delays my other devices).
pub fn random_delay() -> Duration {
    let mut b = [0u8; 8];
    let r = if getrandom::getrandom(&mut b).is_ok() { u64::from_le_bytes(b) } else { u64::MAX };
    delay_for(r)
}

/// The wait for the random number `r`: spread evenly over the two limits.
pub fn delay_for(r: u64) -> Duration {
    let span = SELF_MAIL_DELAY_MAX_MS - SELF_MAIL_DELAY_MIN_MS + 1;
    Duration::from_millis(SELF_MAIL_DELAY_MIN_MS + r % span)
}

/// One connection's mail to myself, waiting to go: oldest first, each due no earlier than the one
/// before it.
#[derive(Debug, Default)]
pub struct SelfMail {
    held: VecDeque<(Instant, String)>,
}

impl SelfMail {
    /// Hold `frame` until `now + delay`, after every copy already waiting.
    pub fn hold(&mut self, frame: String, now: Instant, delay: Duration) {
        let mut due = now + delay;
        if let Some((last, _)) = self.held.back() {
            due = due.max(*last);
        }
        self.held.push_back((due, frame));
    }

    /// Take every copy due at `now`, oldest first.
    pub fn due(&mut self, now: Instant) -> Vec<String> {
        let mut out = Vec::new();
        while self.held.front().is_some_and(|(at, _)| *at <= now) {
            if let Some((_, f)) = self.held.pop_front() {
                out.push(f);
            }
        }
        out
    }

    /// Take everything still waiting, oldest first.
    pub fn take_all(&mut self) -> Vec<(Instant, String)> {
        self.held.drain(..).collect()
    }

    /// Adopt copies another client left waiting (a reconnect), keeping their times and order.
    pub fn adopt(&mut self, left: Vec<(Instant, String)>) {
        if left.is_empty() {
            return;
        }
        let mut all: Vec<(Instant, String)> = left;
        all.extend(self.held.drain(..));
        // Their times already rise within each list; the earlier list goes first.
        let mut last: Option<Instant> = None;
        for (at, f) in all {
            let at = last.map_or(at, |l| at.max(l));
            last = Some(at);
            self.held.push_back((at, f));
        }
    }

    /// How many copies are waiting.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

/// Copies a client left waiting when its socket was already down, by (server URL, identity), for
/// the next client made for the same pair. Held only in memory, for this run of the app.
static LEFT_WAITING: Mutex<Vec<(String, String, Vec<(Instant, String)>)>> = Mutex::new(Vec::new());

/// Leave `copies` for the next client of `url` signed in as `me`.
pub fn leave_for_next(url: &str, me: &str, copies: Vec<(Instant, String)>) {
    if copies.is_empty() {
        return;
    }
    if let Ok(mut left) = LEFT_WAITING.lock() {
        match left.iter_mut().find(|(u, m, _)| u == url && m == me) {
            Some((_, _, list)) => list.extend(copies),
            None => left.push((url.to_string(), me.to_string(), copies)),
        }
    }
}

/// Take what an earlier client of `url` signed in as `me` left waiting.
pub fn take_left(url: &str, me: &str) -> Vec<(Instant, String)> {
    let Ok(mut left) = LEFT_WAITING.lock() else { return Vec::new() };
    match left.iter().position(|(u, m, _)| u == url && m == me) {
        Some(i) => left.remove(i).2,
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "me_key";

    fn put(to: &str, content: &str) -> String {
        serde_json::json!({ "type": "dm_put", "to": to, "content": content }).to_string()
    }

    #[test]
    fn only_a_put_to_my_own_key_is_mine() {
        assert!(is_self_put(&put(ME, "x"), ME));
        assert!(!is_self_put(&put("bob", "x"), ME), "to someone else");
        assert!(!is_self_put(&serde_json::json!({ "type": "chat", "to": ME }).to_string(), ME), "not a dm_put");
        assert!(!is_self_put(&put(ME, "x"), ""), "no identity, nothing held");
    }

    #[test]
    fn the_wait_stays_between_the_limits() {
        assert_eq!(delay_for(0), Duration::from_millis(SELF_MAIL_DELAY_MIN_MS));
        assert_eq!(delay_for(SELF_MAIL_DELAY_MAX_MS - SELF_MAIL_DELAY_MIN_MS), Duration::from_millis(SELF_MAIL_DELAY_MAX_MS));
        for _ in 0..200 {
            let d = random_delay().as_millis() as u64;
            assert!((SELF_MAIL_DELAY_MIN_MS..=SELF_MAIL_DELAY_MAX_MS).contains(&d), "{d}");
        }
    }

    #[test]
    fn copies_leave_in_order_and_none_before_its_time() {
        let t0 = Instant::now();
        let mut q = SelfMail::default();
        q.hold("a".into(), t0, Duration::from_secs(100));
        q.hold("b".into(), t0, Duration::from_secs(30)); // shorter, but never before a
        q.hold("c".into(), t0, Duration::from_secs(110));
        assert!(q.due(t0 + Duration::from_secs(99)).is_empty());
        assert_eq!(q.due(t0 + Duration::from_secs(100)), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(q.due(t0 + Duration::from_secs(200)), vec!["c".to_string()]);
        assert!(q.is_empty());
    }

    #[test]
    fn what_one_client_left_the_next_one_for_the_same_server_takes() {
        let t0 = Instant::now();
        let url = format!("wss://left-{}.invalid/ws", t0.elapsed().as_nanos());
        leave_for_next(&url, ME, vec![(t0, "left".into())]);
        assert!(take_left(&url, "someone else").is_empty(), "only the same identity");
        let mut q = SelfMail::default();
        q.hold("new".into(), t0, Duration::from_secs(1));
        q.adopt(take_left(&url, ME));
        assert_eq!(q.due(t0 + Duration::from_secs(5)), vec!["left".to_string(), "new".to_string()]);
        assert!(take_left(&url, ME).is_empty(), "taken once");
    }
}
