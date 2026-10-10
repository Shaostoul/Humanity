//! The desktop app's own pace for `dm_put`s it sends in the background (the 2026-10-10 server
//! review of docs/design/blocking-and-safe-mode.md).
//!
//! WHY: the relay gives each sender a burst of 8 `dm_put`s that refills at one a second (one
//! every five seconds for a brand-new account that is not yet trusted), and a put over that is
//! refused with "Slow down" and NOT delivered. The friendship-pass sweep (engine/dm.rs
//! `sweep_friend_passes`) used to send one pass to every friend owed one, back to back, two puts
//! each (the friend's copy and our own), and record each as delivered: a sweep owing more than
//! four passes lost the rest for good, since a pass recorded as given is never sent again.
//!
//! So background sends draw on this budget: at most `BURST` puts at once, then no faster than
//! `REFILL_PER_SEC`, which keeps them under the server's limit with room left for a message the
//! person sends themselves. What does not fit waits (`held`) and goes out as the budget refills.
//! The pace is a politeness, not the guarantee: since 10l a pass is recorded as given only once
//! the server answers `dm_put_ok` for it (net/put_answers.rs), so one it refuses is sent again.

use std::time::Instant;

/// Puts the background may send at once: under the server's 8, so the person's own message right
/// after a sweep still goes through.
pub const BURST: f64 = 6.0;

/// Puts the background may send per second once the burst is spent: the server's own refill.
pub const REFILL_PER_SEC: f64 = 1.0;

/// The puts one control message takes: the friend's sealed copy and our self-copy.
pub const PUTS_PER_CONTROL: u32 = 2;

/// The budget: a token bucket, refilled by the time passed.
#[derive(Debug, Clone)]
pub struct PutPacer {
    tokens: f64,
    at: Option<Instant>,
    /// Something was held back for want of budget; engine/dm.rs `pace_owed_passes` sends it as
    /// the budget refills.
    pub held: bool,
}

impl Default for PutPacer {
    fn default() -> Self {
        Self { tokens: BURST, at: None, held: false }
    }
}

impl PutPacer {
    fn refill(&mut self, now: Instant) {
        if let Some(at) = self.at {
            let secs = now.saturating_duration_since(at).as_secs_f64();
            self.tokens = (self.tokens + secs * REFILL_PER_SEC).min(BURST);
        }
        self.at = Some(now);
    }

    /// May `puts` more go out at `now`?
    pub fn has(&mut self, puts: u32, now: Instant) -> bool {
        self.refill(now);
        self.tokens + 1e-9 >= f64::from(puts)
    }

    /// `puts` went out at `now`.
    pub fn spend(&mut self, puts: u32, now: Instant) {
        self.refill(now);
        self.tokens = (self.tokens - f64::from(puts)).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The bucket itself: six at once, then one a second, never more than six banked.
    /// Seen red 2026-10-10 with `refill` adding a whole BURST each call: "six at once, no more"
    /// failed (the budget never ran out).
    #[test]
    fn six_at_once_then_one_a_second() {
        let t0 = Instant::now();
        let mut p = PutPacer::default();
        for _ in 0..3 {
            assert!(p.has(2, t0));
            p.spend(2, t0);
        }
        assert!(!p.has(1, t0), "six at once, no more");
        assert!(!p.has(2, t0 + Duration::from_millis(1500)), "then one a second: two need two seconds");
        assert!(p.has(2, t0 + Duration::from_secs(2)));
        assert!(p.has(6, t0 + Duration::from_secs(60)) && !p.has(7, t0 + Duration::from_secs(60)), "never more than six banked");
    }
}
