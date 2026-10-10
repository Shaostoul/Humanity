//! Channel chat's send limiter: Fibonacci backoff per sender, with a slow mode for accounts
//! under `NEW_ACCOUNT_WINDOW_SECS` old. Moved out of relay.rs's socket loop on 2026-10-10, when
//! it was found to panic on a machine that had booted recently.
//!
//! The panic: the limiter set a sender up by subtracting from now (the new-account window and a
//! second; the account's age; a minute "so the first message goes"), and on Windows an `Instant`
//! counts from the machine's boot, so `now - 601 s` panics while the machine has been up for
//! less than ten minutes. That killed the sender's socket task on their first channel message.
//! Now it keeps what it needs as an end time and an optional last send, the way the DM limiter
//! (dm_rate.rs) keeps its new-account window, so nothing is ever subtracted from an `Instant`.
//! No fallback instant would have been right: there is no instant before the boot to fall back
//! to, and `now` would have made the first message wait.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::relay::handlers::dm_rate;
use crate::relay::relay::{RelayMessage, RelayState, FIB_DELAYS, NEW_ACCOUNT_DELAY_SECS, NEW_ACCOUNT_WINDOW_SECS};

/// One sender's place in channel chat's limiter (`RelayState::rate_limits`).
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// Until when this sender counts as a new account and is held to the slow mode, or None when
    /// they were past the window at first sight. An end time, so nothing is subtracted.
    pub new_until: Option<Instant>,
    /// When their last message went, or None before their first (which never waits).
    pub last_message_time: Option<Instant>,
    /// Current position in the Fibonacci delay sequence.
    pub fib_index: usize,
}

impl RateLimitState {
    /// The state for a sender first seen at `now`, whose account is `account_age` seconds old.
    pub(crate) fn first(now: Instant, account_age: u64) -> RateLimitState {
        let new_until = (account_age < NEW_ACCOUNT_WINDOW_SECS)
            .then(|| now + Duration::from_secs(NEW_ACCOUNT_WINDOW_SECS - account_age));
        RateLimitState { new_until, last_message_time: None, fib_index: 0 }
    }

    /// One message at `now`: Ok, or Err(whole seconds still to wait). `trusted` roles skip the
    /// new-account slow mode. A message sent exactly when its wait is over moves one step along
    /// the Fibonacci delays; one sent later starts them again.
    pub(crate) fn take(&mut self, now: Instant, trusted: bool) -> Result<(), u64> {
        let elapsed = self.last_message_time.map_or(u64::MAX, |last| now.saturating_duration_since(last).as_secs());
        let fib_delay = FIB_DELAYS[self.fib_index];
        let new_account = !trusted && self.new_until.is_some_and(|end| now < end);
        let new_account_delay = if new_account { NEW_ACCOUNT_DELAY_SECS } else { 0 };
        let required_delay = fib_delay.max(new_account_delay);
        if elapsed < required_delay {
            return Err(required_delay - elapsed);
        }
        if elapsed > required_delay {
            self.fib_index = 0;
        } else {
            self.fib_index = (self.fib_index + 1).min(FIB_DELAYS.len() - 1);
        }
        self.last_message_time = Some(now);
        Ok(())
    }
}

/// May `sender` (whose role is `role`) post to channel chat now? If not they are told how long
/// to wait. Bots and admins are not limited; verified, donor, moderator and admin accounts skip
/// the new-account slow mode.
pub async fn allow(state: &Arc<RelayState>, sender: &str, role: &str) -> bool {
    if sender.starts_with("bot_") || role == "admin" {
        return true;
    }
    let trusted = matches!(role, "verified" | "donor" | "mod" | "admin");
    let now = Instant::now();
    let mut rate_limits = state.rate_limits.write().await;
    // The account's age comes from the database (dm_rate's `account_age`, registered_at in
    // milliseconds), so a relay restart does not put established accounts back in the slow mode.
    let rl = rate_limits
        .entry(sender.to_string())
        .or_insert_with(|| RateLimitState::first(now, dm_rate::account_age(state, sender)));
    match rl.take(now, trusted) {
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
#[path = "chat_rate_tests.rs"]
mod tests;
