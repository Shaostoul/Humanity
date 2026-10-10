//! Passes on their way to the server (section 10l of docs/design/blocking-and-safe-mode.md,
//! "A pass counts as given only once the server took it", 2026-10-10), without a socket.
//!
//! WHY: the relay can refuse a `dm_put` (its burst limit, a new account's slower refill, the
//! recipient's "who can reach me" gate) and, before 10l, never said which send it refused. The app
//! recorded a friendship pass as given the moment it was written to the socket and withdrew the
//! pass it replaced, so a refused send left a friend with no pass while the app showed one
//! standing, and a pass recorded as given is never sent again. Now every `dm_put` that changes
//! friendship state (a new pass, a re-issued one, the pass a contact request carries) carries a
//! `ref`, the relay answers `dm_put_ok` or `dm_put_refused` for it, and engine/dm.rs records the
//! pass only on `dm_put_ok`.
//!
//! What is held here, in memory only: each such send, its `ref`, the pass it carries, and our
//! self-copy of it, which goes out only once the server took theirs (so our other devices never
//! learn of a pass the friend never got). The DM store keeps the pass on its own persisted list of
//! passes not yet answered (net/dm_store.rs `passes_unanswered`), which outlives this run: a send
//! whose answer never came may have reached the friend, so it is withdrawn whenever the passes to
//! that friend are, even though it never counts as given.

use std::time::{Duration, Instant};

use super::dm_store::SentPass;

/// How long a send waits for its answer (10l: "no answer within 30 seconds"). After that it
/// counts as not taken: nothing is recorded and the friend is still owed a pass.
pub const ANSWER_WAIT: Duration = Duration::from_secs(30);

/// A fresh `ref` for one send: 32 lower-case hex characters (10l allows 1 to 64 of
/// `[A-Za-z0-9_-]`). Random, never the pass's own serial: the server sees refs, and a serial it
/// could match to a later withdrawal would tell it which put carried which pass.
pub fn new_ref() -> Option<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// What a held send was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// A friendship pass. `reissue`: it replaces the passes the friend holds from us (a tick
    /// changed, 10c-ii), which are withdrawn once the server took it.
    Pass { reissue: bool },
    /// The pass inside a contact request (10c).
    Request,
}

/// One send waiting for the server's answer.
#[derive(Debug, Clone)]
pub struct PendingPut {
    /// The `ref` the `dm_put` carried, which the answer names.
    pub reference: String,
    /// Who it went to.
    pub peer: String,
    /// The pass it carries: recorded as given on `dm_put_ok`, never before.
    pub pass: SentPass,
    /// Our self-copy, sent on `dm_put_ok` and dropped otherwise. None when it already went out
    /// beside theirs: a re-issue that takes something away (10m R2, engine/dm.rs `reissue_pass`)
    /// tells my other devices the new choice at once, and it is never sent a second time.
    pub self_copy: Option<serde_json::Value>,
    pub held: Held,
    /// When it was written to the socket; ANSWER_WAIT after this it counts as not taken.
    pub at: Instant,
}

/// The server's answer to one of our sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// `dm_put_ok {ref}`: stored in the recipient's mailbox.
    Taken(String),
    /// `dm_put_refused {ref, reason}`: not stored; the reason is `rate`, `reach`, `size` or
    /// `other`.
    Refused(String, String),
}

impl Answer {
    /// Read a frame from the relay: Some when it is one of the two answers and names a ref.
    pub fn read(frame: &serde_json::Value) -> Option<Self> {
        let reference = frame.get("ref").and_then(|v| v.as_str()).filter(|r| !r.is_empty())?.to_string();
        match frame.get("type").and_then(|t| t.as_str())? {
            "dm_put_ok" => Some(Answer::Taken(reference)),
            "dm_put_refused" => {
                let reason = frame.get("reason").and_then(|v| v.as_str()).unwrap_or("other").to_string();
                Some(Answer::Refused(reference, reason))
            }
            _ => None,
        }
    }

    pub fn reference(&self) -> &str {
        match self {
            Answer::Taken(r) | Answer::Refused(r, _) => r,
        }
    }
}

/// The sends waiting for their answers.
#[derive(Debug, Default)]
pub struct PendingPuts {
    list: Vec<PendingPut>,
}

impl PendingPuts {
    pub fn hold(&mut self, put: PendingPut) {
        self.list.push(put);
    }

    /// Is a send to `peer` still waiting? The pass sweep leaves them alone until it is answered,
    /// so one friend never has two passes on their way at once.
    pub fn has_peer(&self, peer: &str) -> bool {
        self.list.iter().any(|p| p.peer == peer)
    }

    /// Forget every send to `peer` still waiting (10m R6: Unfollow and Block). Its answer then
    /// finds nothing and records nothing, and a new pass or request to them need not wait for it.
    /// The passes they carried stay among the unanswered in the DM store, which Unfollow and Block
    /// withdraw. Returns how many were waiting.
    pub fn drop_peer(&mut self, peer: &str) -> usize {
        let before = self.list.len();
        self.list.retain(|p| p.peer != peer);
        before - self.list.len()
    }

    /// The serials of the passes on their way to `peer`.
    pub fn serials_to(&self, peer: &str) -> Vec<String> {
        self.list.iter().filter(|p| p.peer == peer).map(|p| p.pass.serial.clone()).collect()
    }

    /// The send `reference` names, taken off the list. None for a ref that is not ours or whose
    /// answer came after ANSWER_WAIT (that send was already settled as not taken).
    pub fn take(&mut self, reference: &str) -> Option<PendingPut> {
        let at = self.list.iter().position(|p| p.reference == reference)?;
        Some(self.list.remove(at))
    }

    /// The sends that have waited ANSWER_WAIT or longer at `now`, taken off the list.
    pub fn expired(&mut self, now: Instant) -> Vec<PendingPut> {
        if self.list.is_empty() {
            return Vec::new(); // every frame, so the common case does nothing
        }
        let (gone, kept): (Vec<PendingPut>, Vec<PendingPut>) =
            std::mem::take(&mut self.list).into_iter().partition(|p| now.saturating_duration_since(p.at) >= ANSWER_WAIT);
        self.list = kept;
        gone
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answers are read by type and ref, a ref is the shape 10l allows, and a send waits
    /// exactly ANSWER_WAIT. Seen red 2026-10-10 with `expired` comparing with `>` (strictly after
    /// 30 s): "thirty seconds is the limit" failed (nothing expired at 30 s).
    #[test]
    fn answers_refs_and_the_thirty_second_wait() {
        let r = new_ref().unwrap();
        assert!(r.len() <= 64 && !r.is_empty() && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), "a ref 10l allows: {r}");
        assert_ne!(new_ref(), Some(r.clone()), "fresh each time");
        let ok = serde_json::json!({ "type": "dm_put_ok", "ref": "a1" });
        assert_eq!(Answer::read(&ok), Some(Answer::Taken("a1".into())));
        let no = serde_json::json!({ "type": "dm_put_refused", "ref": "a2", "reason": "rate" });
        assert_eq!(Answer::read(&no), Some(Answer::Refused("a2".into(), "rate".into())));
        assert_eq!(Answer::read(&serde_json::json!({ "type": "dm_put_ok" })), None, "an answer names a ref");
        assert_eq!(Answer::read(&serde_json::json!({ "type": "cert_revoked", "ref": "a3" })), None, "another frame is not an answer");

        let t0 = Instant::now();
        let mut held = PendingPuts::default();
        let put = |reference: &str, at: Instant| PendingPut {
            reference: reference.into(),
            peer: "ben".into(),
            pass: SentPass { serial: "00".repeat(16), may: "message".into() },
            self_copy: None,
            held: Held::Pass { reissue: false },
            at,
        };
        held.hold(put("a1", t0));
        held.hold(put("a2", t0 + Duration::from_secs(5)));
        assert!(held.has_peer("ben") && !held.has_peer("cy"));
        assert!(held.expired(t0 + Duration::from_secs(29)).is_empty(), "29 seconds: still waiting");
        let gone = held.expired(t0 + ANSWER_WAIT);
        assert_eq!(gone.iter().map(|p| p.reference.as_str()).collect::<Vec<_>>(), ["a1"], "thirty seconds is the limit");
        assert!(held.take("a1").is_none(), "an expired send's late answer finds nothing");
        assert_eq!(held.take("a2").map(|p| p.reference), Some("a2".to_string()));
        assert!(held.is_empty());
    }
}
