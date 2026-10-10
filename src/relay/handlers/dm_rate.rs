//! The private-message send limiter (2026-10-10).
//!
//! Why `dm_put` has a limiter of its own: until 2026-10-10 every recipient-addressed `dm_put`
//! went through channel chat's Fibonacci limiter (`RelayState::rate_limits`, relay.rs), which
//! measures whole seconds, so a second `dm_put` inside the same second was dropped with "Slow
//! down!". Channel chat is a person typing; a DM's control traffic is a program sending. The apps'
//! own protocol sends quick pairs (a follow and then a friendship pass; a pass re-issued and then
//! the next one), and they count a pass as delivered once it is sent, so one dropped pass could
//! leave a friend holding only a withdrawn pass for good. The two kinds of sending need different
//! shapes, and sharing one entry also meant a burst of DMs slowed the same person's channel chat.
//!
//! The shape: a bucket per sender holding [`DM_BURST`] sends, refilled at one a second (one per
//! `NEW_ACCOUNT_DELAY_SECS` while an account that is not trusted is under `NEW_ACCOUNT_WINDOW_SECS`
//! old, the rule channel chat has). A burst of quick sends lands; a sustained flood is held to the
//! refill rate, the same rate as before. Strangers stay capped by the daily knock and
//! contact-request budgets (handlers/reach.rs), which this does not touch.
//!
//! In memory only, like every other limiter here: a restart refills every bucket, which costs at
//! most one burst per sender.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::relay::relay::{RelayMessage, RelayState, NEW_ACCOUNT_DELAY_SECS, NEW_ACCOUNT_WINDOW_SECS};

/// Sends a sender may make at once before the refill rate holds them. The apps' quick pairs (a
/// follow then a friendship pass; a pass re-issued, then the next) and the re-issue of a few
/// friends' passes after a change in Settings > Safety fit in it, and it is small next to the 20
/// knocks a day a stranger has, so it widens nobody's reach. One case it does not cover: the
/// apps' pass sweep (engine/dm.rs `sweep_friend_passes`, web chat-social.js `sweepFriendPasses`)
/// sends a pass to every friend owed one back to back, so a sweep owing more than eight meets
/// the refill rate from the ninth on; pacing that sweep is the apps' side.
pub const DM_BURST: u32 = 8;

/// Seconds per refilled send for an established or trusted account: one a second, the sustained
/// rate channel chat's limiter allowed `dm_put` before.
const DM_REFILL_SECS: u64 = 1;

/// Above this many buckets, the full ones are dropped on the next send. A full bucket is exactly
/// what a sender with no bucket gets, so dropping one loses nothing; this only bounds memory.
const PRUNE_AT: usize = 4_096;

/// One sender's bucket.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bucket {
    /// Sends available now, fractional while refilling.
    tokens: f64,
    /// When `tokens` was last brought up to date.
    last: Instant,
    /// Until when this sender is a new account (None: already past the window when first seen).
    /// Kept as an end time rather than a registration time so nothing is subtracted from an
    /// `Instant`, which can underflow on a machine that booted recently.
    new_until: Option<Instant>,
}

impl Bucket {
    /// A full bucket for a sender whose account is `account_age_secs` old at `now`.
    fn new(now: Instant, account_age_secs: u64) -> Bucket {
        let new_until = (account_age_secs < NEW_ACCOUNT_WINDOW_SECS)
            .then(|| now + Duration::from_secs(NEW_ACCOUNT_WINDOW_SECS - account_age_secs));
        Bucket { tokens: DM_BURST as f64, last: now, new_until }
    }

    /// Seconds per refilled send at `now`.
    fn refill_secs(&self, now: Instant, trusted: bool) -> u64 {
        let new_account = !trusted && self.new_until.map_or(false, |end| now < end);
        if new_account {
            NEW_ACCOUNT_DELAY_SECS
        } else {
            DM_REFILL_SECS
        }
    }

    /// Bring the bucket up to `now` and take one send: Ok, or Err(whole seconds until one is back,
    /// at least 1). A refused send takes nothing.
    fn take(&mut self, now: Instant, trusted: bool) -> Result<(), u64> {
        let period = self.refill_secs(now, trusted) as f64;
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / period).min(DM_BURST as f64);
        self.last = self.last.max(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err((((1.0 - self.tokens) * period).ceil() as u64).max(1))
        }
    }

    /// Whether the bucket would be full at `now` whatever the account's age (the slowest refill).
    fn full_at(&self, now: Instant) -> bool {
        let slowest = (DM_BURST as u64) * NEW_ACCOUNT_DELAY_SECS.max(DM_REFILL_SECS);
        now.saturating_duration_since(self.last) >= Duration::from_secs(slowest)
    }
}

/// Every sender's bucket. Lives in `RelayState::dm_rate`.
#[derive(Default)]
pub struct DmRateLimits {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl DmRateLimits {
    /// Take one send for `sender` at `now`. `account_age` is asked (once, for a sender with no
    /// bucket yet) how many seconds old their account is. Err carries the whole seconds to wait.
    pub(crate) fn take_at(&self, sender: &str, now: Instant, trusted: bool, account_age: impl FnOnce() -> u64) -> Result<(), u64> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        if buckets.len() > PRUNE_AT {
            buckets.retain(|_, b| !b.full_at(now));
        }
        buckets
            .entry(sender.to_string())
            .or_insert_with(|| Bucket::new(now, account_age()))
            .take(now, trusted)
    }

    /// Tests: forget `sender`'s bucket, so their next send starts full (the way tests of the daily
    /// budgets keep this limiter out of their counts).
    #[cfg(test)]
    pub(crate) fn forget(&self, sender: &str) {
        self.buckets.lock().unwrap().remove(sender);
    }

    /// Tests: empty `sender`'s bucket as of now (making one if needed), so their next send is
    /// refused until it refills.
    #[cfg(test)]
    pub(crate) fn drain(&self, sender: &str) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        let b = buckets.entry(sender.to_string()).or_insert_with(|| Bucket::new(now, u64::MAX));
        b.tokens = 0.0;
        b.last = now;
    }

    /// Tests: act as if `secs` more seconds had passed for `sender`'s refill (their bucket's last
    /// update moves back; how old their account is does not change).
    #[cfg(test)]
    pub(crate) fn rewind(&self, sender: &str, secs: u64) {
        let mut buckets = self.buckets.lock().unwrap();
        if let Some(b) = buckets.get_mut(sender) {
            b.last = b.last.checked_sub(Duration::from_secs(secs)).expect("test clock rewind");
        }
    }
}

/// Seconds since `key` first registered a name here (0 for a key with none: an unknown sender
/// counts as new). Read from the database so a relay restart does not make established accounts
/// new again. `registered_at` is in MILLISECONDS (storage/messages.rs `register_name_on`); the
/// inline limiter this replaced subtracted it from a count of seconds, which always gave 0, so
/// every account that was not trusted was treated as new for its first ten minutes after each
/// restart.
fn account_age(state: &RelayState, key: &str) -> u64 {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    match state.db.first_registered_at_for_key(key) {
        Ok(Some(registered_ms)) => (now_ms.saturating_sub(registered_ms).max(0) / 1000) as u64,
        _ => 0,
    }
}

/// The limiter's step in `handle_dm_put`, for a recipient-addressed copy (the caller skips the
/// self-copy). True when `sender` may send now; otherwise they are told how long to wait and the
/// send is dropped. `role` is the sender's. Admins and the owner are not limited (the exemption
/// `dm_put` always had; the owner is an admin everywhere else in the relay). Verified, donor,
/// moderator, admin and owner accounts skip the new-account rule.
pub fn allow(state: &Arc<RelayState>, sender: &str, role: &str) -> bool {
    if matches!(role, "admin" | "owner") {
        return true;
    }
    let trusted = matches!(role, "verified" | "donor" | "mod" | "admin" | "owner");
    match state.dm_rate.take_at(sender, Instant::now(), trusted, || account_age(state, sender)) {
        Ok(()) => true,
        Err(wait) => {
            let _ = state.broadcast_tx.send(RelayMessage::Private {
                to: sender.to_string(),
                message: format!("⏳ Slow down! Please wait {} more second{}.", wait, if wait == 1 { "" } else { "s" }),
            });
            false
        }
    }
}

#[cfg(test)]
#[path = "dm_rate_tests.rs"]
mod tests;
