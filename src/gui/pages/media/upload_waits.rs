//! How Add to Media's upload waits (round 3 of docs/design/media-showcase.md, from the
//! 2026-10-11 review of round 2). Every wait the worker in resumable.rs makes is here, because
//! the review's three loop findings were all a wait that did not happen:
//!
//! - FINDING 2, Pause before the upload had started looped with no wait: the start's loop never
//!   looked at Pause, and every wait returned at once while paused (1,008,232 start requests in
//!   half a second, measured). Now [`hold`] runs before EVERY request: while paused, nothing is
//!   sent, in any phase.
//! - FINDING 3, a 429 whose `resets_at` was not ahead of this PC's clock was retried at once (a
//!   clock a little fast, or a reset just passed). Now an allowance waits the server's own number,
//!   `resets_in_s` (R3-6), falling back to `resets_at` only when the server sent no number, and
//!   never less than [`ALLOWANCE_WAIT_MIN`].
//! - FINDING 5, a server error that never clears (a full disk's 500) was resent forever and shown
//!   as a dropped connection. Now each server error waits a little longer than the last
//!   ([`Backoff`]), says the server had a problem, and after [`STUCK_AFTER`] of the same one in a
//!   row ([`Streak`]) the upload stops and shows the server's own words with Retry.
//!
//! A 5xx, or a 429 WITHOUT the allowance's JSON (nginx's rate limit, or the relay's "You have 3
//! uploads in progress"), is never the end of an upload: it waits and tries again.

use std::sync::mpsc::Sender;
use std::time::Duration;

use serde_json::Value;

use super::resumable::{Control, Event, Flags, Reply};

/// The least an allowance wait lasts, whatever the numbers say (finding 3).
pub const ALLOWANCE_WAIT_MIN: Duration = Duration::from_secs(5);
/// The most an allowance wait lasts before asking again (a day's allowance resets within a day).
pub const ALLOWANCE_WAIT_MAX: Duration = Duration::from_secs(25 * 3600);
/// How many of the same server error in a row before it is shown, with Retry (finding 5).
pub const STUCK_AFTER: u32 = 10;
/// The largest piece this app sends, whatever the server's `chunk_max` says (finding 9: a
/// number from the server was used as an allocation's size unchecked).
pub const CHUNK_CEILING: u64 = 64 * 1024 * 1024;
/// The piece size until the server states its own (`chunk_max`, 8 MiB on the relay).
pub const DEFAULT_CHUNK: u64 = 8 * 1024 * 1024;

/// What is said while a server error is waited out.
pub const SERVER_TROUBLE: &str = "The server had a problem with the upload. It tries again in a moment, carrying on from where the server got.";
/// What is said while a 429 that is not an allowance is waited out and its words say nothing.
pub const SERVER_BUSY: &str = "The server asked this app to wait a moment. The upload carries on by itself.";

/// The piece size to use for what the server says its `chunk_max` is: its number, held to
/// [`CHUNK_CEILING`]; the default when it says none (or 0).
pub fn piece_size(chunk_max: Option<u64>) -> u64 {
    chunk_max.filter(|c| *c > 0).unwrap_or(DEFAULT_CHUNK).min(CHUNK_CEILING)
}

/// A day's or an hour's allowance used up (a 429 with the allowance's JSON, the relay's B3 and
/// R3-6): its words, how long to wait, and what is left of the window, when the server says.
#[derive(Debug, Clone, PartialEq)]
pub struct Allowance {
    pub words: String,
    pub wait: Duration,
    /// `left_bytes`: what the window still allows. A piece no larger than this is taken now.
    pub left: Option<u64>,
    /// Where the server is in the file, when a refused piece's answer says (`offset`).
    pub offset: Option<u64>,
}

impl Allowance {
    /// When it resets, for the words on the page (Unix ms, by this PC's clock plus the wait).
    pub fn resets_at(&self) -> i64 {
        now_ms() + self.wait.as_millis() as i64
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The allowance in a 429, if it is one: it carries `allowance`, `resets_in_s` or `resets_at`.
/// The wait is the server's own `resets_in_s`; without it, `resets_at` less this PC's clock; never
/// less than `min`, never more than [`ALLOWANCE_WAIT_MAX`].
pub fn allowance_of(r: &Reply, min: Duration) -> Option<Allowance> {
    if r.status != 429 {
        return None;
    }
    let v = r.json();
    let in_s = v.get("resets_in_s").and_then(Value::as_f64);
    let at = v.get("resets_at").and_then(Value::as_i64);
    if in_s.is_none() && at.is_none() && v.get("allowance").is_none() {
        return None;
    }
    let wait_ms = match (in_s, at) {
        (Some(s), _) => (s.max(0.0) * 1000.0) as u64,
        (None, Some(at)) => (at - now_ms()).max(0) as u64,
        (None, None) => 0,
    };
    let wait = Duration::from_millis(wait_ms).max(min).min(ALLOWANCE_WAIT_MAX);
    Some(Allowance { words: r.words(), wait, left: v.get("left_bytes").and_then(Value::as_u64), offset: v.get("offset").and_then(Value::as_u64) })
}

/// The words for a 429 that is not an allowance: the server's own, unless it sent none.
pub fn busy_words(r: &Reply) -> String {
    let body = r.body.trim();
    if body.is_empty() || body.starts_with('<') {
        SERVER_BUSY.to_string()
    } else {
        r.words()
    }
}

/// Waits after a failure: `pace`, then twice that, up to thirty times it.
pub struct Backoff {
    pace: Duration,
    next: Duration,
}

impl Backoff {
    pub fn new(pace: Duration) -> Self {
        Backoff { pace, next: pace }
    }
    pub fn reset(&mut self) {
        self.next = self.pace;
    }
    /// Wait out the next step (woken early by Pause, Cancel or a new token).
    pub fn wait(&mut self, ctl: &Control) {
        let token = ctl.flags().token;
        ctl.wait(Some(self.next), |f| !f.cancelled && !f.paused && f.token == token);
        self.next = (self.next * 2).min(self.pace * 30);
    }
}

/// The same server error, counted while it repeats (finding 5).
#[derive(Debug, Default)]
pub struct Streak {
    last: Option<(u16, String)>,
    count: u32,
}

impl Streak {
    /// Count `r` (a server error) and return how many of it there have been in a row.
    pub fn note(&mut self, r: &Reply) -> u32 {
        let this = (r.status, r.body.clone());
        if self.last.as_ref() == Some(&this) {
            self.count += 1;
        } else {
            self.last = Some(this);
            self.count = 1;
        }
        self.count
    }
    /// Anything but that error: the count starts again.
    pub fn clear(&mut self) {
        self.last = None;
        self.count = 0;
    }
}

/// Before every request (finding 2): while paused, wait here, saying so once, and send nothing.
/// False when the upload was cancelled meanwhile.
pub fn hold(ctl: &Control, tx: &Sender<Event>) -> bool {
    let flags = ctl.flags();
    if flags.cancelled {
        return false;
    }
    if !flags.paused {
        return true;
    }
    let _ = tx.send(Event::Paused);
    let flags = ctl.wait(None, |f| f.paused && !f.cancelled);
    ctl.take_retry();
    if flags.cancelled {
        return false;
    }
    let _ = tx.send(Event::Sending);
    true
}

/// Wait while an allowance is used up: its whole wait, or until Resume, Pause or Cancel.
pub fn wait_for_allowance(ctl: &Control, a: &Allowance) -> Flags {
    let flags = ctl.wait(Some(a.wait), |f| !f.cancelled && !f.retry && !f.paused);
    ctl.take_retry();
    flags
}

/// Wait for Retry (or Cancel): no room at the finish, or a server error that will not clear.
pub fn wait_for_retry(ctl: &Control) -> Flags {
    let flags = ctl.wait(None, |f| !f.cancelled && !f.retry);
    ctl.take_retry();
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(status: u16, body: Value) -> Reply {
        Reply { status, body: body.to_string() }
    }

    /// THE SERVER'S OWN WAIT, AT LEAST FIVE SECONDS (finding 3): `resets_in_s` is used as sent,
    /// even when `resets_at` says the reset has passed by this PC's clock; with no `resets_in_s`
    /// the reset time is used; and a wait is never shorter than the minimum. A 429 with no
    /// allowance's JSON is not an allowance.
    ///
    /// Seen red 2026-10-11 with the wait taken from `resets_at` alone and no minimum (as in round
    /// 2): "the server's own number" (left: 0ns / right: 120s: a `resets_at` in the past, no wait).
    #[test]
    fn an_allowance_waits_the_servers_own_number() {
        let past = now_ms() - 10_000;
        let r = reply(429, serde_json::json!({ "error": "Day used.", "allowance": "day", "resets_at": past, "resets_in_s": 120, "left_bytes": 0 }));
        let a = allowance_of(&r, ALLOWANCE_WAIT_MIN).expect("an allowance");
        assert_eq!(a.wait, Duration::from_secs(120), "the server's own number");
        assert_eq!((a.words.as_str(), a.left), ("Day used.", Some(0)));
        let old = reply(429, serde_json::json!({ "error": "Day used.", "allowance": "day", "resets_at": past }));
        assert_eq!(allowance_of(&old, ALLOWANCE_WAIT_MIN).unwrap().wait, ALLOWANCE_WAIT_MIN, "never less than the minimum");
        let soon = reply(429, serde_json::json!({ "allowance": "hour", "resets_in_s": 0.2 }));
        assert_eq!(allowance_of(&soon, ALLOWANCE_WAIT_MIN).unwrap().wait, ALLOWANCE_WAIT_MIN);
        assert_eq!(allowance_of(&Reply { status: 429, body: "<html>429</html>".into() }, ALLOWANCE_WAIT_MIN), None, "nginx's 429 is not an allowance");
        assert_eq!(allowance_of(&reply(429, serde_json::json!({ "error": "You have 3 uploads in progress" })), ALLOWANCE_WAIT_MIN), None);
    }

    /// A PIECE IS NEVER LARGER THAN 64 MiB, whatever the server says (finding 9).
    ///
    /// Seen red 2026-10-11 with the server's number used as it came: "held to the ceiling" (left:
    /// 18446744073709551615 / right: 67108864).
    #[test]
    fn a_piece_is_never_larger_than_the_ceiling() {
        assert_eq!(piece_size(Some(u64::MAX)), CHUNK_CEILING, "held to the ceiling");
        assert_eq!(piece_size(Some(8 << 20)), 8 << 20);
        assert_eq!(piece_size(Some(0)), DEFAULT_CHUNK);
        assert_eq!(piece_size(None), DEFAULT_CHUNK);
    }
}
