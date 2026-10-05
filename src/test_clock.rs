//! A clock that a test moves by hand (BUG-152). Test builds only: the product
//! cannot reach it.
//!
//! Some code decides things by how much time has passed: the media player's
//! playback clock picks the frame that is due, and the relay's perception rate
//! limit turns away a second request inside 200 ms. In the product both read
//! the wall clock (`Instant::now()`), which is what they should do. A TEST that
//! drives them through the wall clock, though, lets the machine's speed decide
//! the result: with ten other builds running, the first frame after a seek
//! arrives after the clock has moved on, and two messages sent together reach
//! the rate limit more than 200 ms apart. Such a test fails on a busy machine
//! and passes on a rerun, which teaches people to rerun until it is green.
//!
//! Handed this clock instead, the code sees time pass only when the test calls
//! `advance`, so the result is the same on an idle machine and a loaded one.
//! The production path is unchanged: each user holds an `Option` of this clock
//! behind `#[cfg(test)]` and reads `Instant::now()` whenever it is absent.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Time that passes only when the test says so. `now()` is an `Instant`, so it
/// slots into code that does `Instant` arithmetic without any other change.
pub(crate) struct ManualClock {
    /// A fixed point taken at creation; `now()` is this plus `elapsed_ns`.
    epoch: Instant,
    /// How far the test has moved the clock, in nanoseconds.
    elapsed_ns: AtomicU64,
}

impl ManualClock {
    /// A clock standing still at an arbitrary moment. Shared, because the test
    /// keeps one handle to move it and hands the other to the code under test.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self { epoch: Instant::now(), elapsed_ns: AtomicU64::new(0) })
    }

    /// The current time on this clock. Two reads with no `advance` between
    /// them are equal, however long the machine took in between.
    pub(crate) fn now(&self) -> Instant {
        self.epoch + Duration::from_nanos(self.elapsed_ns.load(Ordering::SeqCst))
    }

    /// Move the clock forward by `by`.
    pub(crate) fn advance(&self, by: Duration) {
        self.elapsed_ns.fetch_add(by.as_nanos() as u64, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_moves_only_when_told() {
        let c = ManualClock::new();
        let a = c.now();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(c.now(), a, "wall time passing does not move it");
        c.advance(Duration::from_millis(250));
        assert_eq!(c.now() - a, Duration::from_millis(250), "it moves by exactly what it was told");
    }
}
