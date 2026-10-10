//! Tests for channel chat's send limiter (handlers/chat_rate.rs). A `#[path]` child of
//! chat_rate.rs, so `use super::*` reaches everything it has.

use super::*;

/// The earliest moment this machine's monotonic clock can name: the furthest step back from now
/// that `Instant::checked_sub` allows, found by halving. On Windows an `Instant` counts from the
/// machine's boot, so this is the clock of a machine that has only just started, the case that
/// panicked; on other systems it is further back, and the same test holds there.
fn earliest_instant() -> Instant {
    let now = Instant::now();
    let mut back = Duration::ZERO;
    let mut step = Duration::from_secs(u64::MAX / 2);
    while !step.is_zero() {
        if let Some(further) = back.checked_add(step) {
            if now.checked_sub(further).is_some() {
                back = further;
            }
        }
        step /= 2;
    }
    now.checked_sub(back).expect("found by checked_sub")
}

/// A limiter first made on a machine that has only just booted works like any other. The old
/// limiter set its sender up by subtracting from now (the new-account window plus a second, the
/// account's age, a minute "so the first message goes"), and on Windows an `Instant` cannot go
/// below the machine's boot, so in a machine's first ten minutes the first chat message of every
/// sender panicked and killed their socket's task. Every case below is one step from the clock's
/// earliest instant: a second before it cannot be named at all.
///
/// Seen red 2026-10-10 against the old `first` (its three subtractions, moved out of relay.rs
/// unchanged): the first case panicked with "overflow when subtracting duration from instant".
#[test]
fn a_limiter_first_made_on_a_machine_that_just_booted_works() {
    let t0 = earliest_instant();
    assert!(t0.checked_sub(Duration::from_secs(1)).is_none(), "precondition: nothing a second before t0 exists");
    let at = |s: u64| t0 + Duration::from_secs(s);

    // A brand-new account: the first message goes, then the new-account slow mode holds.
    let mut new = RateLimitState::first(t0, 0);
    assert_eq!(new.take(t0, false), Ok(()), "a new account's first message goes");
    assert_eq!(new.take(at(1), false), Err(NEW_ACCOUNT_DELAY_SECS - 1), "then waits the new-account delay");
    assert_eq!(new.take(at(NEW_ACCOUNT_DELAY_SECS), false), Ok(()));

    // An established account: the first message goes and only the Fibonacci rule applies.
    let mut old = RateLimitState::first(t0, 10_000);
    assert_eq!(old.take(t0, false), Ok(()), "an established account's first message goes");
    assert_eq!(old.take(at(1), false), Ok(()), "and a second later is not slowed by the new-account rule");

    // An account 300 s old at first sight is new for 300 s more, counted from t0.
    let mut aging = RateLimitState::first(t0, NEW_ACCOUNT_WINDOW_SECS - 300);
    assert_eq!(aging.take(t0, false), Ok(()));
    assert_eq!(aging.take(at(298), false), Ok(()));
    assert_eq!(aging.take(at(299), false), Err(NEW_ACCOUNT_DELAY_SECS - 1), "still new at 299 s");
    assert_eq!(aging.take(at(305), false), Ok(()));
    assert_eq!(aging.take(at(306), false), Ok(()), "established from 300 s: one second is enough");

    // A trusted role skips the slow mode from the start.
    let mut trusted = RateLimitState::first(t0, 0);
    assert_eq!(trusted.take(t0, true), Ok(()));
    assert_eq!(trusted.take(at(1), true), Ok(()), "a trusted new account is not slowed");
}

/// The rules themselves, on the ordinary clock: a message sent exactly when the wait is over
/// moves one step along the Fibonacci delays, a message sent later starts them again, and an
/// early one is refused with the seconds still to wait.
#[test]
fn the_fibonacci_backoff_steps_on_at_the_boundary_and_resets_after_a_pause() {
    let t = Instant::now();
    let at = |s: u64| t + Duration::from_secs(s);
    let mut rl = RateLimitState::first(t, 10_000);
    assert_eq!(rl.take(t, false), Ok(()));
    assert_eq!(rl.take(at(1), false), Ok(()), "1 s after: the first delay is 1 s");
    assert_eq!(rl.take(at(2), false), Ok(()), "1 s after again: the second delay is 1 s");
    assert_eq!(rl.take(at(3), false), Err(1), "the third delay is 2 s");
    assert_eq!(rl.take(at(4), false), Ok(()), "2 s after the last: goes, and the next delay is 3 s");
    assert_eq!(rl.take(at(5), false), Err(2));
    assert_eq!(rl.take(at(100), false), Ok(()), "a pause starts the delays again");
    assert_eq!(rl.take(at(101), false), Ok(()), "so 1 s is enough once more");
}
