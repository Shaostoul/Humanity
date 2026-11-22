//! The relay's memory of signed requests it has already accepted, so each is good once
//! (2026-11-21, D4 of the signed-requests round 2; it was `RelayState::auth_nonce_fresh`).
//!
//! WHAT WAS WRONG. The memory was a map from text to time, keyed by `{hash of the key's text}:
//! {tag}:{timestamp}`, and several routes passed request text as the tag (a channel read passed its
//! purpose, with a channel name the request chose; the Files page likewise; a file edit built a tag
//! from an unchecked role). Entries were recorded before the route's rank check, and the map was
//! pruned only once it passed 100,000 entries, so 3,000 requests naming a 1 MB channel held about
//! 3 GB. And the key's TEXT was hashed while verifying ignores hex letter case, so the same
//! signature with the key upper-cased got a fresh slot (D3).
//!
//! NOW:
//! - an entry is a fixed 16 bytes (the first half of a SHA-256 over the key in lowercase, the
//!   route's tag, the time and the request's own details, each length-prefixed), whatever the request
//!   holds; the tag is a `&'static str`, so only a name written in the code can be one;
//! - `signed_request::check` records an entry only after the cheap checks (a key of the canonical
//!   form, a purpose of at most 2,048 bytes, a fresh time, this server's own name) and the
//!   signature pass;
//! - the memory holds at most [`CAPACITY`] entries and forgets the oldest first, and forgets any
//!   entry older than [`WINDOW`] (wider than the five minutes a request's time may be old, so an
//!   entry outlives the request it guards).
//!
//! WHAT A FLOOD CAN AND CANNOT DO. The memory never holds more than [`CAPACITY`] entries of fixed
//! size: about 13 MB at most however many requests arrive, so it cannot run the relay out of memory.
//! Each entry costs the sender a valid Dilithium3 signature under some key over a request naming
//! this server, and the relay one verify. A sender who makes more than [`CAPACITY`] such requests
//! inside [`WINDOW`] pushes the oldest entries out early, after which a request first used inside the
//! window but pushed out would be accepted once more by whoever holds a copy of it. Since requests
//! name their server (round 2, D1), only this server itself (and, over plain http, someone on the
//! network path) ever holds such a copy. Nothing kept across runs: after a restart a request whose
//! time is older than the start is refused instead (D6, `signed_request::check`).

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The most entries the memory holds. At 16 bytes in the set and 32 in the order queue, plus the
/// set's own overhead, about 13 MB when full.
pub const CAPACITY: usize = 200_000;

/// How long an entry is kept: six minutes, wider than the five minutes a request's time may be old
/// (`signed_request::SIGNED_WINDOW_MS`) plus the 30 seconds it may be ahead.
pub const WINDOW: Duration = Duration::from_secs(6 * 60);

/// One accepted request: 16 bytes of a SHA-256, never its text.
type Id = [u8; 16];

#[derive(Default)]
struct Inner {
    seen: HashSet<Id>,
    /// Oldest first, for pruning by age and by count.
    order: VecDeque<(Id, Instant)>,
}

/// The memory itself, one per relay (`RelayState::replay`).
#[derive(Default)]
pub struct ReplayMemory {
    inner: Mutex<Inner>,
}

impl ReplayMemory {
    /// Whether this is the first time (`key`, `tag`, `timestamp`, `detail`) is seen inside the
    /// window; records it when so. Call only after the signature over it has verified. `key` is
    /// read in lowercase (one key, one entry, however it was spelled); `tag` is the route's fixed
    /// name; `detail` is whatever else the signature covered (a purpose, an object's name), hashed
    /// in so two different requests a key signs in the same millisecond are not refused as one.
    pub fn first_use(&self, key: &str, tag: &'static str, timestamp: u64, detail: &[u8]) -> bool {
        self.first_use_at(key, tag, timestamp, detail, Instant::now())
    }

    fn first_use_at(&self, key: &str, tag: &'static str, timestamp: u64, detail: &[u8], now: Instant) -> bool {
        let id = entry_id(key, tag, timestamp, detail);
        let mut m = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(&(old, at)) = m.order.front() {
            if now.saturating_duration_since(at) < WINDOW && m.order.len() < CAPACITY {
                break;
            }
            m.order.pop_front();
            m.seen.remove(&old);
        }
        if !m.seen.insert(id) {
            return false;
        }
        m.order.push_back((id, now));
        true
    }

    /// How many entries are held now.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).order.len()
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The fixed-size id of one request.
fn entry_id(key: &str, tag: &str, timestamp: u64, detail: &[u8]) -> Id {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    let key = key.to_ascii_lowercase();
    for part in [key.as_bytes(), tag.as_bytes(), &timestamp.to_be_bytes(), detail] {
        h.update((part.len() as u64).to_be_bytes());
        h.update(part);
    }
    let mut id = [0u8; 16];
    id.copy_from_slice(&h.finalize()[..16]);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D3 in the memory itself: the critic's model (scratchpad critic-sigreq/nonce-model.js) as a
    /// test. Red evidence (2026-11-21, `entry_id` hashing the key's text as given, the old
    /// `auth_nonce_fresh`): "the upper-cased key gets no fresh slot" failed.
    #[test]
    fn a_key_in_another_case_gets_no_fresh_slot() {
        let m = ReplayMemory::default();
        let key = "ab".repeat(1952);
        assert!(m.first_use(&key, "group_read", 7, b""));
        assert!(!m.first_use(&key, "group_read", 7, b""), "the same request again is refused");
        assert!(!m.first_use(&key.to_ascii_uppercase(), "group_read", 7, b""), "the upper-cased key gets no fresh slot");
        assert!(m.first_use(&key, "group_read", 8, b""), "another time is another request");
        assert!(m.first_use(&key, "bug_vote", 7, b""), "another route is another request");
        assert!(m.first_use(&key, "group_read", 7, b"other purpose"), "another purpose is another request");
    }

    /// D4: a flood is bounded. Red evidence (2026-11-21, the count check taken out of the prune
    /// loop): "the memory never holds more than CAPACITY" failed at 200,500.
    #[test]
    fn a_flood_is_bounded_and_forgets_the_oldest_first() {
        let m = ReplayMemory::default();
        let now = Instant::now();
        let big = vec![b'x'; 1 << 20]; // a megabyte of request text costs the memory 16 bytes
        for i in 0..(CAPACITY as u64 + 500) {
            assert!(m.first_use_at("k", "chat_read", i, &big[..(i as usize % 7)], now));
        }
        assert_eq!(m.len(), CAPACITY, "the memory never holds more than CAPACITY");
        // The oldest 500 were forgotten; the newest is still remembered.
        assert!(!m.first_use_at("k", "chat_read", CAPACITY as u64 + 499, &big[..((CAPACITY + 499) % 7)], now));
        // Entries older than the window are forgotten too.
        let later = now + WINDOW + Duration::from_secs(1);
        assert!(m.first_use_at("k", "chat_read", 1, b"fresh", later));
        assert_eq!(m.len(), 1, "everything older than the window went");
        // The id is a fixed 16 bytes, so the megabyte never lands in the memory.
        assert_eq!(std::mem::size_of::<Id>(), 16);
        let _ = big;
    }
}
