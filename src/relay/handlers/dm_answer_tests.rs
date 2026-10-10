//! Tests for the answer to a `dm_put` (handlers/dm_answer.rs, spec 10l of
//! docs/design/blocking-and-safe-mode.md), driven through `handle_dm_put` against a real
//! database as the other handler tests are. A `#[path]` child of dm_answer.rs. The socket half
//! (the answer reaches the sender's socket and no one else's, and the feature gate's answer) is
//! in features.rs, the wire shapes in relay_wire_tests.rs.
//!
//! Seen red 2026-10-10 with the `answer_dm_put` call taken out of `handle_dm_put`: the ok, rate,
//! reach, size-and-other and bad-ref tests failed with no answer at all (left `[]`; the bad-ref
//! test at its 64-character ref), while the tests that expect no answer passed, as they should
//! with nothing sent. And with `refuse_gated_dm_put` answering nothing: the gate test failed.

use super::*;
use std::sync::Arc;

use crate::relay::handlers::msg_handlers::handle_dm_put;
use crate::relay::handlers::reach::{DmAsk, DM_KNOCKS_PER_DAY};
use crate::relay::storage::Storage;

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("dm_answer_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A syntactically valid v2 envelope (the relay cannot check more).
fn envelope() -> String {
    serde_json::json!({ "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "R0hJ" }).to_string()
}

/// A sender an hour old and a recipient who takes anyone's mail, so a put spends a stranger's
/// knock (20 a day, more than any test here sends) and is otherwise let in.
fn pair(st: &Arc<RelayState>) -> (&'static str, &'static str) {
    for (name, key) in [("Sender", "ans_sender"), ("Inbox", "ans_inbox")] {
        st.db.register_name(name, key).unwrap();
        st.db
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE registered_names SET registered_at = registered_at - 3600000 WHERE public_key = ?1", rusqlite::params![key])
            .unwrap();
    }
    st.db.set_reach_settings("ans_inbox", &[("message", "anyone")]).unwrap();
    ("ans_sender", "ans_inbox")
}

/// What one put did: whether it reached `to`'s mailbox, the answers its sender got (ref, and
/// None for ok or Some(reason) for a refusal), the notices they got, and how many
/// `reach_refused` they got.
#[derive(Debug)]
struct Heard {
    stored: bool,
    answers: Vec<(String, Option<String>)>,
    notices: Vec<String>,
    reach_refused: usize,
}

fn put_as(st: &Arc<RelayState>, from: &str, to: &str, content: String, put_ref: Option<&str>) -> Heard {
    let mut rx = st.broadcast_tx.subscribe();
    let before = st.db.mailbox_fetch(to, 0, 1_000).unwrap().len();
    block(handle_dm_put(st, from, to.to_string(), content, None, DmAsk::Ordinary, put_ref.map(str::to_string)));
    let stored = st.db.mailbox_fetch(to, 0, 1_000).unwrap().len() > before;
    let mut heard = Heard { stored, answers: Vec::new(), notices: Vec::new(), reach_refused: 0 };
    while let Ok(msg) = rx.try_recv() {
        match msg {
            RelayMessage::DmPutOk { sender, put_ref } => {
                assert_eq!(sender, from, "an answer is routed to the put's own sender");
                heard.answers.push((put_ref, None));
            }
            RelayMessage::DmPutRefused { sender, put_ref, reason } => {
                assert_eq!(sender, from, "an answer is routed to the put's own sender");
                heard.answers.push((put_ref, Some(reason)));
            }
            RelayMessage::Private { to, message } if to == from => heard.notices.push(message),
            RelayMessage::ReachRefused { sender, .. } if sender == from => heard.reach_refused += 1,
            _ => {}
        }
    }
    heard
}

fn put(st: &Arc<RelayState>, from: &str, to: &str, put_ref: Option<&str>) -> Heard {
    put_as(st, from, to, envelope(), put_ref)
}

fn refused(r: &str, reason: &str) -> Vec<(String, Option<String>)> {
    vec![(r.to_string(), Some(reason.to_string()))]
}

#[test]
fn a_stored_put_with_a_ref_is_answered_ok() {
    let st = fresh_state("ok");
    let (sender, inbox) = pair(&st);
    let heard = put(&st, sender, inbox, Some("pass-7_A"));
    assert!(heard.stored, "{heard:?}");
    assert_eq!(heard.answers, vec![("pass-7_A".to_string(), None)], "stored: dm_put_ok with the same ref");
    assert!(heard.notices.is_empty(), "and nothing else: {heard:?}");
}

/// The send limiter's refusal and a spent daily budget are both "rate": the same send can land
/// later. The sender's notice still goes as before.
#[test]
fn a_rate_refusal_is_answered_rate() {
    let st = fresh_state("rate");
    let (sender, inbox) = pair(&st);
    st.dm_rate.drain(sender);
    let heard = put(&st, sender, inbox, Some("r1"));
    assert!(!heard.stored);
    assert_eq!(heard.answers, refused("r1", "rate"), "the burst limiter: {heard:?}");
    assert!(heard.notices.iter().any(|m| m.contains("Slow down")), "the notice still goes: {heard:?}");

    st.dm_rate.forget(sender);
    let today = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() / 86_400) as i64;
    block(st.dm_knocks.write()).insert(sender.to_string(), (today, DM_KNOCKS_PER_DAY));
    let heard = put(&st, sender, inbox, Some("r2"));
    assert!(!heard.stored);
    assert_eq!(heard.answers, refused("r2", "rate"), "a spent day's knocks: {heard:?}");
    assert!(heard.notices.iter().any(|m| m.contains("Daily limit")), "{heard:?}");
}

#[test]
fn a_reach_refusal_is_answered_reach() {
    let st = fresh_state("reach");
    let (sender, inbox) = pair(&st);
    st.db.set_reach_settings(inbox, &[("message", "friends")]).unwrap();
    let heard = put(&st, sender, inbox, Some("r3"));
    assert!(!heard.stored);
    assert_eq!(heard.answers, refused("r3", "reach"), "{heard:?}");
    assert_eq!(heard.reach_refused, 1, "reach_refused still goes: {heard:?}");
}

#[test]
fn an_oversized_envelope_is_answered_size_and_anything_else_other() {
    let st = fresh_state("size");
    let (sender, inbox) = pair(&st);
    let big = serde_json::json!({ "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "A".repeat(140_000) }).to_string();
    let heard = put_as(&st, sender, inbox, big, Some("r4"));
    assert!(!heard.stored);
    assert_eq!(heard.answers, refused("r4", "size"), "{heard:?}");
    assert!(heard.notices.iter().any(|m| m.contains("envelope exceeds")), "{heard:?}");

    let heard = put_as(&st, sender, inbox, "{\"v\":1}".to_string(), Some("r5"));
    assert!(!heard.stored);
    assert_eq!(heard.answers, refused("r5", "other"), "not a sealed v2 envelope: {heard:?}");
}

/// A put without a ref is handled exactly as before spec 10l: no answer either way, and the old
/// notices still go.
#[test]
fn a_put_without_a_ref_gets_no_answer() {
    let st = fresh_state("no_ref");
    let (sender, inbox) = pair(&st);
    let heard = put(&st, sender, inbox, None);
    assert!(heard.stored);
    assert!(heard.answers.is_empty(), "stored, no ref: {heard:?}");
    st.db.set_reach_settings(inbox, &[("message", "friends")]).unwrap();
    let heard = put(&st, sender, inbox, None);
    assert!(!heard.stored);
    assert!(heard.answers.is_empty(), "refused, no ref: {heard:?}");
    assert_eq!(heard.reach_refused, 1);
}

/// The self-copy (the sender's sent history, addressed to their own key) gets no answer, stored
/// or refused: the recipient copy of the same send carries the answer.
#[test]
fn the_self_copy_gets_no_answer() {
    let st = fresh_state("self_copy");
    let (sender, _) = pair(&st);
    let heard = put(&st, sender, sender, Some("self-1"));
    assert!(heard.stored);
    assert!(heard.answers.is_empty(), "a stored self-copy: {heard:?}");
    let big = serde_json::json!({ "v": 2, "ek_ct_b64": "QUJD", "nonce_b64": "REVG", "ct_b64": "A".repeat(140_000) }).to_string();
    let heard = put_as(&st, sender, sender, big, Some("self-2"));
    assert!(!heard.stored);
    assert!(heard.answers.is_empty(), "a refused self-copy: {heard:?}");
    assert!(!heard.notices.is_empty(), "though its notice still goes");
}

/// A ref outside the spec (1 to 64 characters of [A-Za-z0-9_-]) is treated as none: the put is
/// handled as usual and nothing is answered. Sixty-four characters is still a ref.
#[test]
fn a_bad_ref_is_treated_as_no_ref() {
    let st = fresh_state("bad_ref");
    let (sender, inbox) = pair(&st);
    let long = "a".repeat(65);
    for bad in ["", long.as_str(), "has space", "dot.ted", "sl/ash", "ünï", "quote\"d", "new\nline"] {
        st.dm_rate.forget(sender);
        let heard = put(&st, sender, inbox, Some(bad));
        assert!(heard.stored, "{bad:?}: the put itself is handled as usual");
        assert!(heard.answers.is_empty(), "{bad:?} is no ref: {heard:?}");
    }
    st.dm_rate.forget(sender);
    let longest = "Az09_-".repeat(11)[..64].to_string();
    let heard = put(&st, sender, inbox, Some(&longest));
    assert_eq!(heard.answers, vec![(longest.clone(), None)], "64 characters of the allowed set is a ref");
}

/// A `dm_put` the feature gate stops (the owner switched chat off, relay.rs) is answered
/// "other", by the same rules: not for the self-copy, not for a bad ref, not without one.
#[test]
fn a_put_the_feature_gate_stops_is_answered_other() {
    let st = fresh_state("gate");
    let answers = |raw: serde_json::Value| -> Vec<(String, Option<String>)> {
        let mut rx = st.broadcast_tx.subscribe();
        refuse_gated_dm_put(&st, "gated_sender", &raw);
        let mut out = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            if let RelayMessage::DmPutRefused { sender, put_ref, reason } = msg {
                assert_eq!(sender, "gated_sender");
                out.push((put_ref, Some(reason)));
            }
        }
        out
    };
    let frame = |to: &str, r: Option<&str>| {
        let mut v = serde_json::json!({ "type": "dm_put", "to": to, "content": "{}" });
        if let Some(r) = r {
            v["ref"] = serde_json::json!(r);
        }
        v
    };
    assert_eq!(answers(frame("someone", Some("g1"))), refused("g1", "other"));
    assert!(answers(frame("gated_sender", Some("g2"))).is_empty(), "the self-copy");
    assert!(answers(frame("someone", Some("bad ref"))).is_empty(), "a bad ref");
    assert!(answers(frame("someone", None)).is_empty(), "no ref");
}
