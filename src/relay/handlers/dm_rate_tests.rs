//! Tests for the private-message send limiter (handlers/dm_rate.rs). A `#[path]` child of
//! dm_rate.rs, so `use super::*` reaches its private bucket.

use super::*;
use crate::relay::handlers::msg_handlers::handle_dm_put;
use crate::relay::handlers::reach::DmAsk;
use crate::relay::relay::RateLimitState;
use crate::relay::storage::Storage;

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A syntactically valid v2 envelope (the relay can't check more).
fn envelope() -> String {
    serde_json::json!({ "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "R0hJ" }).to_string()
}

/// Register `key` with an account `age_secs` old (registered_at is in milliseconds).
fn member(st: &Arc<RelayState>, name: &str, key: &str, age_secs: i64) {
    st.db.register_name(name, key).unwrap();
    st.db
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE registered_names SET registered_at = registered_at - ?1 WHERE public_key = ?2",
            rusqlite::params![age_secs * 1000, key],
        )
        .unwrap();
}

/// A sender an hour old and a recipient who takes anyone's mail (so each send spends a knock,
/// 20 a day, more than any test here sends).
fn pair(st: &Arc<RelayState>) -> (&'static str, &'static str) {
    member(st, "Sender", "rate_sender", 3_600);
    member(st, "Inbox", "rate_inbox", 3_600);
    st.db.set_reach_settings("rate_inbox", &[("message", "anyone")]).unwrap();
    ("rate_sender", "rate_inbox")
}

/// One `dm_put` from `from` to `to`: whether it was stored, and what `from` was told.
fn put(st: &Arc<RelayState>, from: &str, to: &str) -> (bool, Vec<String>) {
    let mut rx = st.broadcast_tx.subscribe();
    let before = st.db.mailbox_fetch(to, 0, 1_000).unwrap().len();
    block(handle_dm_put(st, from, to.to_string(), envelope(), None, DmAsk::Ordinary));
    let stored = st.db.mailbox_fetch(to, 0, 1_000).unwrap().len() > before;
    let mut told = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let RelayMessage::Private { to: who, message } = msg {
            if who == from {
                told.push(message);
            }
        }
    }
    (stored, told)
}

fn knocks_spent(st: &Arc<RelayState>, key: &str) -> u32 {
    block(st.dm_knocks.read()).get(key).map_or(0, |e| e.1)
}

// ── The bucket itself, on a clock the test controls ──

#[test]
fn a_bucket_holds_eight_sends_and_refills_one_a_second() {
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_secs(s);
    let mut b = Bucket::new(t0, 3_600);
    for i in 0..DM_BURST {
        assert_eq!(b.take(t0, false), Ok(()), "send {} of the burst", i + 1);
    }
    assert_eq!(b.take(t0, false), Err(1), "the ninth in the same instant waits a second");
    assert_eq!(b.take(at(1), false), Ok(()), "a second later one send is back");
    assert_eq!(b.take(at(1), false), Err(1), "and only one");
    assert_eq!(b.take(at(4), false), Ok(()));
    assert_eq!(b.take(at(4), false), Ok(()));
    assert_eq!(b.take(at(4), false), Ok(()), "three seconds give three sends");
    assert_eq!(b.take(at(4), false), Err(1));
    // A long quiet spell fills the bucket, never past eight.
    for _ in 0..DM_BURST {
        assert_eq!(b.take(at(1_000), false), Ok(()));
    }
    assert_eq!(b.take(at(1_000), false), Err(1), "a full bucket is eight, however long it waited");
}

#[test]
fn a_new_account_refills_one_per_five_seconds_unless_trusted() {
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_secs(s);
    let mut new = Bucket::new(t0, 0);
    for _ in 0..DM_BURST {
        assert_eq!(new.take(t0, false), Ok(()), "a new account has the same burst");
    }
    assert_eq!(new.take(t0, false), Err(NEW_ACCOUNT_DELAY_SECS), "then waits the new-account delay");
    assert_eq!(new.take(at(1), false), Err(NEW_ACCOUNT_DELAY_SECS - 1), "a second is not enough");
    assert_eq!(new.take(at(NEW_ACCOUNT_DELAY_SECS), false), Ok(()), "the delay is");
    // The same bucket for a trusted role (verified, donor, moderator) refills at one a second.
    let mut trusted = Bucket::new(t0, 0);
    for _ in 0..DM_BURST {
        trusted.take(t0, true).unwrap();
    }
    assert_eq!(trusted.take(at(1), true), Ok(()), "a trusted new account is not slowed");
    // Past the new-account window the account refills at one a second like anyone.
    let mut aging = Bucket::new(t0, NEW_ACCOUNT_WINDOW_SECS - 10);
    for _ in 0..DM_BURST {
        aging.take(t0, false).unwrap();
    }
    assert_eq!(aging.take(at(1), false), Err(NEW_ACCOUNT_DELAY_SECS - 1), "still new for ten more seconds");
    let later = at(10);
    for _ in 0..DM_BURST {
        aging.take(later, false).unwrap();
    }
    assert_eq!(aging.take(later + Duration::from_secs(1), false), Ok(()), "established now: one a second");
}

// ── Through handle_dm_put ──

/// The break this limiter fixes: the apps send quick pairs (a follow then a friendship pass; a
/// pass re-issued, then the next), and the shared one-a-second limiter dropped the second of
/// each pair with "Slow down!". Two, then five, then one more in the same instant all land; the
/// ninth is refused with the same notice as before, stored nowhere and costing no knock.
#[test]
fn quick_dm_puts_land_until_the_bucket_is_empty() {
    let st = fresh_state("dmrate_burst");
    let (sender, inbox) = pair(&st);
    for i in 0..2 {
        assert_eq!(put(&st, sender, inbox), (true, vec![]), "quick pair, send {}", i + 1);
    }
    for i in 0..5 {
        assert_eq!(put(&st, sender, inbox), (true, vec![]), "quick run of five, send {}", i + 1);
    }
    assert_eq!(put(&st, sender, inbox), (true, vec![]), "the eighth fills the burst");
    let (stored, told) = put(&st, sender, inbox);
    assert!(!stored, "the ninth in the same instant is beyond the bucket");
    assert!(told.iter().any(|m| m.contains("Slow down")), "and the sender is told: {told:?}");
    assert_eq!(st.db.mailbox_fetch(inbox, 0, 100).unwrap().len(), 8);
    assert_eq!(knocks_spent(&st, sender), 8, "a send the limiter refused spends no knock");
}

#[test]
fn the_bucket_refills_at_one_a_second() {
    let st = fresh_state("dmrate_refill");
    let (sender, inbox) = pair(&st);
    st.dm_rate.drain(sender);
    assert!(!put(&st, sender, inbox).0, "precondition: an empty bucket refuses");
    st.dm_rate.rewind(sender, 1);
    assert!(put(&st, sender, inbox).0, "a second later one send is back");
    assert!(!put(&st, sender, inbox).0, "only one");
    st.dm_rate.rewind(sender, 3);
    for i in 0..3 {
        assert!(put(&st, sender, inbox).0, "three seconds give three sends ({})", i + 1);
    }
    assert!(!put(&st, sender, inbox).0);
}

/// An account under ten minutes old with no trusted role refills one per five seconds: a second
/// after its bucket empties, an established account can send again and it cannot.
#[test]
fn a_new_accounts_bucket_refills_more_slowly() {
    let st = fresh_state("dmrate_new");
    let (old, inbox) = pair(&st);
    member(&st, "Newcomer", "rate_newcomer", 0);
    for key in [old, "rate_newcomer"] {
        // Make each bucket (reading the account's age), then empty it.
        assert!(put(&st, key, inbox).0, "precondition: {key} can send");
        st.dm_rate.drain(key);
        st.dm_rate.rewind(key, 1);
    }
    assert!(put(&st, old, inbox).0, "an established account has a send back after a second");
    let (stored, told) = put(&st, "rate_newcomer", inbox);
    assert!(!stored, "a new account does not");
    assert!(
        told.iter().any(|m| m.contains(&format!("{} more seconds", NEW_ACCOUNT_DELAY_SECS - 1))),
        "it is told how long is left: {told:?}"
    );
    st.dm_rate.rewind("rate_newcomer", NEW_ACCOUNT_DELAY_SECS - 1);
    assert!(put(&st, "rate_newcomer", inbox).0, "after the new-account delay it has one");
    // A trusted role skips the new-account rule.
    st.db.set_role("rate_newcomer", "verified").unwrap();
    st.dm_rate.drain("rate_newcomer");
    st.dm_rate.rewind("rate_newcomer", 1);
    assert!(put(&st, "rate_newcomer", inbox).0, "a verified new account refills at one a second");
}

/// DMs and channel chat no longer share a limiter: a burst of DMs leaves channel chat's entry
/// untouched (so the person's next channel message is not slowed), and a channel limiter at its
/// slowest does not slow a DM.
#[test]
fn channel_chat_limiter_is_unaffected() {
    let st = fresh_state("dmrate_channel");
    let (sender, inbox) = pair(&st);
    for _ in 0..DM_BURST {
        assert!(put(&st, sender, inbox).0);
    }
    assert!(
        block(st.rate_limits.read()).get(sender).is_none(),
        "DMs leave no mark on channel chat's limiter"
    );
    st.db.set_reach_settings(sender, &[("message", "anyone")]).unwrap(); // so the reply is let in
    let now = Instant::now();
    block(st.rate_limits.write()).insert(
        "rate_inbox".to_string(),
        RateLimitState { first_seen: now, last_message_time: now, fib_index: crate::relay::relay::FIB_DELAYS.len() - 1 },
    );
    let back = put(&st, "rate_inbox", sender);
    assert_eq!(back, (true, vec![]), "a person channel chat has just slowed can still DM");
}

/// The self-copy (sent history for the sender's other devices) and admins are not limited.
#[test]
fn self_copies_and_admins_are_not_limited() {
    let st = fresh_state("dmrate_exempt");
    let (sender, inbox) = pair(&st);
    for _ in 0..(DM_BURST * 3) {
        assert!(put(&st, sender, sender).0, "self-copies are never limited");
    }
    for _ in 0..DM_BURST {
        assert!(put(&st, sender, inbox).0, "and spend nothing from the bucket");
    }
    member(&st, "Admin", "rate_admin", 3_600);
    member(&st, "Owner", "rate_owner", 3_600);
    st.db.set_role("rate_admin", "admin").unwrap();
    st.db.set_role("rate_owner", "owner").unwrap();
    for key in ["rate_admin", "rate_owner"] {
        for i in 0..(DM_BURST + 4) {
            assert!(put(&st, key, inbox).0, "{key} send {} is not limited", i + 1);
        }
    }
}

/// Dropping full buckets loses nothing: a full bucket is what a sender with none gets.
#[test]
fn full_buckets_are_dropped_past_the_cap_and_others_kept() {
    let limits = DmRateLimits::default();
    let t0 = Instant::now();
    for i in 0..=PRUNE_AT {
        limits.take_at(&format!("k{i}"), t0, false, || 3_600).unwrap();
    }
    // A busy sender empties their bucket while the others are still refilling (none dropped yet).
    let busy_at = t0 + Duration::from_secs(5);
    for _ in 0..DM_BURST {
        limits.take_at("busy", busy_at, false, || 3_600).unwrap();
    }
    assert_eq!(limits.buckets.lock().unwrap().len(), PRUNE_AT + 2, "nothing is dropped while it still refills");
    // By the time the slowest bucket would be full again, the first ones are dropped on the next send.
    let later = t0 + Duration::from_secs(DM_BURST as u64 * NEW_ACCOUNT_DELAY_SECS);
    limits.take_at("someone", later, false, || 3_600).unwrap();
    let left: Vec<String> = {
        let mut keys: Vec<String> = limits.buckets.lock().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(left, ["busy", "someone"], "every full bucket was dropped, the refilling one kept");
}

/// `account_age` reads `registered_at` as milliseconds, so an hour-old account is an hour old;
/// channel chat's limiter now uses it too (relay.rs), where an inline copy that took the value as
/// seconds made every account "new" for ten minutes after each restart (2026-10-10). An unknown
/// key counts as brand new. Seen red 2026-10-10 with the `/ 1000` taken out: "an hour-old
/// account is about an hour old" (it read some 3.6 million seconds).
#[test]
fn account_age_reads_milliseconds() {
    let st = fresh_state("dm_rate_age");
    member(&st, "Hour", "age_hour", 3_600);
    let age = account_age(&st, "age_hour");
    assert!((3_590..=3_700).contains(&age), "an hour-old account is about an hour old: {age}");
    assert_eq!(account_age(&st, "nobody_here"), 0, "an unknown key counts as new");
}
