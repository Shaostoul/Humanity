//! Scheduled re-votes: the part that OPENS a re-vote when its review date arrives.
//!
//! Step 3 of the design written up in `docs/design/scheduled-revotes.md`. Steps 1
//! and 2 already shipped: a proposal can carry a review date or a cadence
//! (v0.1302.0), and a chain of superseding proposals resolves to the decision
//! that currently stands (v0.1303.0). What was missing is the thing that runs on
//! a timer, notices "this decision was due for review last Tuesday", and puts the
//! same question back in front of people.
//!
//! ## Who signs an automatically-opened re-vote
//!
//! **The server does, with its own identity.** The operator's call, and his
//! words: "Makes sense for the server to sign. We want to avoid forged
//! signatures."
//!
//! The reason that is the right answer is worth stating plainly, because a
//! signature is easy to misread as agreement. Here a signature means AUTHORSHIP
//! and INTEGRITY: it says who produced these exact bytes and proves nobody
//! altered them afterwards. It does not mean the signer endorses the proposal.
//!
//! Signing as the original proposer would be forging their authorship. That
//! person may have changed their mind since, may have left, may have died. Their
//! original vote and their original proposal stay immutable and stay theirs. The
//! server already has its own Dilithium3 identity for exactly this class of
//! machine-authored record: it signs federation hellos and server announcements
//! with it (`src/relay/storage/misc.rs::get_or_create_server_keypair`), so the
//! re-vote is signed by the same key, and its record says in plain words that
//! the server opened it because the review date arrived.
//!
//! ## The safeguard: the server may not write the question
//!
//! A server that can open a vote could, if buggy or taken over, quietly reword
//! what people are voting on. So it is not allowed to write the question at all.
//!
//! The successor carries the question of the proposal it supersedes, copied
//! VERBATIM, byte for byte. Everything the server authors (the new voting
//! window, the note saying this was opened automatically, when, and which
//! decision it follows) lives in separate, clearly-named fields, so a reader can
//! always tell machine-added metadata from the human question.
//!
//! This is not a promise, it is enforced twice:
//!
//! 1. [`build_scheduled_revote`] copies the question fields out of the
//!    predecessor's stored payload. It never composes text.
//! 2. `index_proposal` (in `storage/governance.rs`) REFUSES to link a
//!    server-signed successor whose question bytes differ from its
//!    predecessor's. A reworded re-vote is stored as an ordinary unlinked
//!    proposal and never takes over anyone's chain.
//!
//! And a client can check it without trusting this server's word: fetch both
//! objects from `GET /api/v2/objects/{id}`, verify their signatures, strip the
//! scheduling and machine fields listed in [`SCHEDULE_AND_MACHINE_FIELDS`], and
//! compare the remaining canonical CBOR bytes. [`question_digest`] is the same
//! computation, and the chain endpoint publishes it per link as a convenience,
//! never as the authority.

use ciborium::Value;

use crate::relay::core::encoding::{from_canonical_bytes, to_canonical_bytes};
use crate::relay::core::hash::Hash;
use crate::relay::core::object::{Object, ObjectBuilder};
use crate::relay::storage::Storage;

/// Payload fields that are SCHEDULING or MACHINE metadata, not the question.
///
/// Everything else in a proposal payload counts as the question, including any
/// field added later (options, choices, a budget figure). That direction is
/// deliberate: an unknown key is treated as part of the question, so a server
/// that invents a new field is caught by the byte comparison rather than
/// slipping past an allowlist somebody forgot to update.
///
/// - `opens_at` / `closes_at`: each round of voting has its own window.
/// - `review_after` / `review_cadence_ms`: when to ask again. The cadence IS
///   copied forward (so "revisit this every year" keeps meaning that), it is
///   just not part of what people are voting on.
/// - `supersedes`: the link to the predecessor, different by definition.
/// - `auto_opened_*`: written by the server, described below.
pub const SCHEDULE_AND_MACHINE_FIELDS: &[&str] = &[
    "opens_at",
    "closes_at",
    "review_after",
    "review_cadence_ms",
    "supersedes",
    "auto_opened_by",
    "auto_opened_at",
    "auto_opened_reason",
    "auto_opened_note",
];

/// The value the server writes into `auto_opened_reason`. One machine-readable
/// token so a client can branch on it without parsing English.
pub const REASON_SCHEDULED_REVIEW: &str = "scheduled_review";

/// Shortest voting window an automatically-opened re-vote gets.
///
/// A person who opens a proposal by hand chooses the moment and tells people.
/// Nobody chooses the moment a scheduled review opens, so it gets a week
/// minimum for the community to notice it, even if the original vote ran for an
/// hour. This is a server-authored number, which is why it lives in `closes_at`
/// and not in the question.
pub const MIN_REVOTE_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Longest voting window an automatically-opened re-vote gets. Bounds a silly
/// or hostile original window (a hundred-year vote) from becoming permanent.
pub const MAX_REVOTE_WINDOW_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// How often the scheduler looks for decisions that have come due. A review
/// date is a date, not a deadline to the second, so hourly is plenty and costs
/// one indexed query per hour.
pub const SWEEP_INTERVAL_SECS: u64 = 60 * 60;

/// Wait after boot before the first sweep, so startup is not competing with a
/// database write nobody is waiting for.
pub const SWEEP_STARTUP_DELAY_SECS: u64 = 30;

/// Most re-votes opened in one sweep. A relay that was offline for a year could
/// otherwise open hundreds in one tick; the rest come due again next hour.
pub const MAX_OPENED_PER_SWEEP: usize = 25;

/// How long to wait before retrying a proposal whose re-vote failed to open.
///
/// Without this, a bug that stops the successor linking would have the
/// scheduler open a fresh unlinked proposal every single hour, forever, because
/// an unlinked successor does not stop the predecessor coming due.
pub const RETRY_COOLDOWN_MS: i64 = 24 * 60 * 60 * 1000;

/// The `server_state` key holding "do not retry this one until".
fn cooldown_key(parent_id: &str) -> String {
    format!("revote_retry_after:{parent_id}")
}

/// The question fields of a proposal payload, with scheduling and machine
/// metadata stripped out.
///
/// Returns the raw CBOR entries so callers can re-encode them into a successor
/// unchanged. Order does not matter: `to_canonical_bytes` sorts map keys, so
/// two payloads carrying the same question always encode to the same bytes.
pub fn question_entries(payload: &[u8]) -> Result<Vec<(Value, Value)>, String> {
    let value = from_canonical_bytes(payload).map_err(|e| format!("payload is not CBOR: {e:?}"))?;
    let entries = match value {
        Value::Map(entries) => entries,
        _ => return Err("proposal payload is not a CBOR map".to_string()),
    };
    Ok(entries
        .into_iter()
        .filter(|(k, _)| match k {
            Value::Text(name) => !SCHEDULE_AND_MACHINE_FIELDS.contains(&name.as_str()),
            // A non-text key is not one of ours; keep it, so it counts as part
            // of the question and any difference is visible.
            _ => true,
        })
        .collect())
}

/// Canonical bytes of a proposal's question. Two proposals asking exactly the
/// same thing produce identical bytes; one character of difference anywhere
/// produces different bytes.
pub fn question_bytes(payload: &[u8]) -> Result<Vec<u8>, String> {
    let entries = question_entries(payload)?;
    to_canonical_bytes(&Value::Map(entries)).map_err(|e| format!("re-encode failed: {e:?}"))
}

/// BLAKE3 of [`question_bytes`], hex. The short form a client (or a person
/// reading a chain) can eyeball to see that every link asks the same thing.
pub fn question_digest(payload: &[u8]) -> Option<String> {
    question_bytes(payload).ok().map(|b| Hash::digest(&b).to_hex())
}

/// Read a text field straight out of a payload. Local helper so this module
/// does not depend on the storage module's private readers.
fn payload_text(payload: &[u8], field: &str) -> Option<String> {
    let value = from_canonical_bytes(payload).ok()?;
    if let Value::Map(entries) = value {
        for (k, v) in entries {
            if let (Value::Text(name), Value::Text(s)) = (&k, &v) {
                if name == field {
                    return Some(s.clone());
                }
            }
        }
    }
    None
}

/// True when this payload claims to be an automatically-opened review.
/// Presentation only: a client uses it to label the proposal. It carries no
/// authority, because the server writes it.
pub fn is_auto_opened(payload: &[u8]) -> bool {
    payload_text(payload, "auto_opened_reason").as_deref() == Some(REASON_SCHEDULED_REVIEW)
}

/// The note the server wrote, if any. Machine text: show it as such.
pub fn auto_opened_note(payload: &[u8]) -> Option<String> {
    payload_text(payload, "auto_opened_note")
}

/// Build and sign the successor proposal for `parent_id`, as the server.
///
/// The question is copied out of the parent's stored payload, verbatim. The
/// server authors only the new voting window and the four `auto_opened_*`
/// fields that say who opened this, when, and why.
///
/// Fails (rather than guessing) when the parent proposal or its stored object
/// is not on this server, because a re-vote whose question had to be invented
/// is exactly the thing this design refuses to produce.
pub fn build_scheduled_revote(
    db: &Storage,
    parent_id: &str,
    now_ms: i64,
) -> Result<Object, String> {
    use crate::relay::core::encoding::{cbor_int, cbor_text};
    use crate::relay::core::pq_crypto::{DilithiumKeypair, derive_dilithium_seed};

    // The index row gives us the original voting window; the stored object gives
    // us the question and the space it belongs to.
    let row = db
        .get_proposal(parent_id)
        .map_err(|e| format!("read proposal: {e}"))?
        .ok_or_else(|| format!("no proposal {parent_id} on this server"))?;
    let stored = db
        .get_signed_object(parent_id)
        .map_err(|e| format!("read proposal object: {e}"))?
        .ok_or_else(|| format!("proposal {parent_id} has no stored object here"))?;

    // ── COPIED, never composed ────────────────────────────────────────────
    // Every question field of the predecessor, exactly as its author signed
    // them: proposal_type, scope, title, body, and anything else they carried.
    let mut fields = question_entries(&stored.payload)?;
    if fields.is_empty() {
        return Err(format!("proposal {parent_id} carries no question to copy"));
    }

    // ── AUTHORED BY THE SERVER ────────────────────────────────────────────
    // The new voting window. The original window length is honoured where it is
    // sensible, clamped at both ends (see the constants).
    let original_window = row.closes_at.saturating_sub(row.opens_at);
    let window = original_window.clamp(MIN_REVOTE_WINDOW_MS, MAX_REVOTE_WINDOW_MS);
    let opens_at = now_ms.max(0);
    let closes_at = opens_at.saturating_add(window);
    fields.push((Value::Text("opens_at".into()), cbor_int(opens_at as u64)));
    fields.push((Value::Text("closes_at".into()), cbor_int(closes_at as u64)));

    // Carry the cadence forward so "revisit this every year" keeps meaning
    // that. A proposal that named a one-off `review_after` and no cadence gets
    // no cadence here, so its chain stops after this single review, which is
    // what its author asked for.
    if let Some(cadence) = row.review_cadence_ms.filter(|ms| *ms > 0) {
        fields.push((
            Value::Text("review_cadence_ms".into()),
            cbor_int(cadence as u64),
        ));
    }

    // The link. `index_proposal` re-checks that the server was entitled to make
    // it, so writing it here is a claim, not a grant.
    fields.push((Value::Text("supersedes".into()), cbor_text(parent_id)));

    // The plain-language record of what happened, in fields named so nobody can
    // mistake them for part of the question.
    let server_did = db.server_did().map_err(|e| format!("server did: {e}"))?;
    fields.push((Value::Text("auto_opened_by".into()), cbor_text(&server_did)));
    fields.push((
        Value::Text("auto_opened_at".into()),
        cbor_int(opens_at as u64),
    ));
    fields.push((
        Value::Text("auto_opened_reason".into()),
        cbor_text(REASON_SCHEDULED_REVIEW),
    ));
    fields.push((
        Value::Text("auto_opened_note".into()),
        cbor_text(
            "This re-vote was opened automatically by the server because the review date \
             set on the previous decision arrived. The question is copied word for word \
             from that decision; the server did not write it. Signing it records who \
             produced this record, not agreement with it.",
        ),
    ));

    // Sign with the server's own Dilithium3 identity, re-derived from the
    // stored 32-byte seed the same way `sign_with_server_key` does, so the
    // expanded secret never sits in the database.
    let (_, seed_hex) = db
        .get_or_create_server_keypair()
        .map_err(|e| format!("server keypair: {e}"))?;
    let seed = hex::decode(&seed_hex).map_err(|e| format!("server seed is not hex: {e}"))?;
    if seed.len() != 32 {
        return Err("server seed is not 32 bytes".to_string());
    }
    let keypair = DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed));

    let mut builder = ObjectBuilder::new("proposal_v1")
        .created_at(opens_at as u64)
        // The predecessor is also named in `references`, so the link is visible
        // in the raw object graph without decoding CBOR.
        .reference(parent_id);
    if let Some(space) = stored.space_id.as_deref() {
        builder = builder.space_id(space);
    }
    builder
        .payload_cbor(&Value::Map(fields))
        .map_err(|e| format!("encode successor payload: {e:?}"))?
        .sign(&keypair)
        .map_err(|e| format!("sign successor: {e:?}"))
}

/// What one sweep did. Returned so tests and the log can be specific.
#[derive(Debug, Default)]
pub struct RevoteSweep {
    /// `(predecessor_id, successor_id)` for each re-vote opened.
    pub opened: Vec<(String, String)>,
    /// `(predecessor_id, why)` for each one that could not be opened.
    pub failed: Vec<(String, String)>,
    /// Proposals skipped because a recent failure put them on cooldown.
    pub skipped: usize,
}

/// Open the successor proposal for every decision whose review date has
/// arrived. This is the whole scheduler; the timer just calls it.
///
/// Safe to call repeatedly, which matters because it runs on a timer and a
/// relay restarts. `proposals_due_for_review` excludes anything already
/// superseded, so once a successor exists its predecessor stops coming back and
/// a second run opens nothing.
pub fn open_due_revotes(db: &Storage, now_ms: i64) -> RevoteSweep {
    let mut sweep = RevoteSweep::default();

    let due = match db.proposals_due_for_review(now_ms) {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!("scheduled re-votes: due query failed: {e}");
            return sweep;
        }
    };

    for parent_id in due.into_iter().take(MAX_OPENED_PER_SWEEP) {
        // A previous attempt failed; do not hammer it every hour.
        if let Ok(Some(until)) = db.get_state(&cooldown_key(&parent_id)) {
            if until.parse::<i64>().map(|t| now_ms < t).unwrap_or(false) {
                sweep.skipped += 1;
                continue;
            }
        }

        match open_one(db, &parent_id, now_ms) {
            Ok(successor_id) => {
                // No need to clear the cooldown: the predecessor is superseded
                // now, so `proposals_due_for_review` will never return it again
                // and the key is never read again either.
                tracing::info!(
                    "scheduled re-votes: opened {successor_id} to review {parent_id}"
                );
                sweep.opened.push((parent_id, successor_id));
            }
            Err(why) => {
                // Cooldown so a persistent fault does not fill the database with
                // near-duplicate unlinked proposals, one per hour.
                let _ = db.set_state(
                    &cooldown_key(&parent_id),
                    &now_ms.saturating_add(RETRY_COOLDOWN_MS).to_string(),
                );
                tracing::error!("scheduled re-votes: could not review {parent_id}: {why}");
                sweep.failed.push((parent_id, why));
            }
        }
    }

    sweep
}

/// Build, store and verify one successor. Returns its object id.
fn open_one(db: &Storage, parent_id: &str, now_ms: i64) -> Result<String, String> {
    let successor = build_scheduled_revote(db, parent_id, now_ms)?;
    let successor_id = successor
        .object_id()
        .map_err(|e| format!("successor object_id: {e:?}"))?
        .to_hex();

    // Goes in by the ordinary front door: the same signature check, the same
    // indexing, the same list and chain reads every other proposal gets.
    db.put_signed_object(&successor, None)
        .map_err(|e| format!("store successor: {e}"))?;

    // Prove the link actually took. `index_proposal` drops a `supersedes`
    // pointer it does not consider authorized, and a successor that failed to
    // link would leave the predecessor due forever. Better to hear about it.
    match db.get_proposal(&successor_id) {
        Ok(Some(row)) if row.supersedes.as_deref() == Some(parent_id) => Ok(successor_id),
        Ok(Some(_)) => Err(format!(
            "successor {successor_id} was stored but did not link to {parent_id}; \
             the relay refused the supersedes pointer"
        )),
        Ok(None) => Err(format!("successor {successor_id} was not indexed as a proposal")),
        Err(e) => Err(format!("read back successor: {e}")),
    }
}

/// The timer. Follows the same shape as the relay's other periodic work (the
/// backup + retention sweep, the game world save): spawned once at boot, sleeps
/// between passes, logs what it did.
pub async fn scheduled_revote_loop(state: std::sync::Arc<crate::relay::relay::RelayState>) {
    tokio::time::sleep(tokio::time::Duration::from_secs(SWEEP_STARTUP_DELAY_SECS)).await;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(SWEEP_INTERVAL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let now = crate::relay::storage::now_millis() as i64;
        let sweep = open_due_revotes(&state.db, now);
        if !sweep.opened.is_empty() || !sweep.failed.is_empty() {
            tracing::info!(
                "scheduled re-votes: {} opened, {} failed, {} on cooldown",
                sweep.opened.len(),
                sweep.failed.len(),
                sweep.skipped
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::core::encoding::{cbor_int, cbor_map, cbor_text};
    use crate::relay::core::pq_crypto::DilithiumKeypair;
    use rusqlite::params;

    fn test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_revote_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// A human-authored proposal that asks to be reviewed, with its vote already
    /// in the past so it is genuinely due.
    fn seed_decision(db: &Storage, author: &DilithiumKeypair, title: &str, body: &str) -> String {
        let payload = cbor_map(vec![
            ("proposal_type", cbor_text("parameter_change")),
            ("scope", cbor_text("local")),
            ("title", cbor_text(title)),
            ("body", cbor_text(body)),
            ("opens_at", cbor_int(1_000)),
            ("closes_at", cbor_int(2_000)),
            ("review_after", cbor_int(3_000)),
            ("review_cadence_ms", cbor_int(365 * 86_400_000)),
        ]);
        let object = ObjectBuilder::new("proposal_v1")
            .space_id("test_server")
            .created_at(1_000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(author)
            .unwrap();
        db.put_signed_object(&object, None).unwrap();
        object.object_id().unwrap().to_hex()
    }

    /// Record a decisive closed result on a proposal, so it counts as a
    /// decision that stands.
    fn ratify(db: &Storage, id: &str, yes: f64, no: f64) {
        db.with_conn(|conn| {
            for (i, (choice, w)) in [("yes", yes), ("no", no)].iter().enumerate() {
                if *w > 0.0 {
                    conn.execute(
                        "INSERT OR REPLACE INTO votes
                            (vote_object_id, proposal_object_id, voter_did, choice,
                             weight_at_vote, cast_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                        params![format!("{id}_v{i}"), id, format!("did:test:{i}"), choice, w],
                    )?;
                }
            }
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();
    }

    fn payload_of(db: &Storage, id: &str) -> Vec<u8> {
        db.get_signed_object(id).unwrap().unwrap().payload
    }

    /// THE SAFEGUARD. The server may open a re-vote; it may not write the
    /// question. Byte-for-byte, the successor asks exactly what the decision it
    /// reviews asked.
    ///
    /// To see this test EARN its keep, change one character of the copied text
    /// in `build_scheduled_revote` (for example append a space to the title) and
    /// re-run: it fails with "the successor must ask the SAME question".
    #[test]
    fn the_successor_asks_the_same_question_byte_for_byte() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(
            &db,
            &author,
            "Quiet hours in the workshop",
            "No power tools between 22:00 and 07:00.",
        );

        let successor = build_scheduled_revote(&db, &parent, 10_000).unwrap();

        let parent_question = question_bytes(&payload_of(&db, &parent)).unwrap();
        let successor_question = question_bytes(&successor.payload).unwrap();
        assert_eq!(
            successor_question, parent_question,
            "the successor must ask the SAME question, byte for byte"
        );
        assert_eq!(
            question_digest(&successor.payload),
            question_digest(&payload_of(&db, &parent)),
            "and therefore carry the same question digest"
        );

        // Not vacuous: there IS a question in there, and the fields a reader
        // cares about survived the copy.
        assert!(!parent_question.is_empty());
        assert!(
            String::from_utf8_lossy(&successor.payload).contains("Quiet hours in the workshop"),
            "the human title must be present in the successor"
        );
    }

    /// The other half of the safeguard: the fields the SERVER writes are
    /// present, separate, and honest about being machine-written.
    #[test]
    fn the_server_authored_fields_are_separate_from_the_question() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "A title", "A body");

        let successor = build_scheduled_revote(&db, &parent, 10_000).unwrap();
        let text = String::from_utf8_lossy(&successor.payload).to_string();

        assert!(is_auto_opened(&successor.payload), "labelled as machine-opened");
        assert!(
            auto_opened_note(&successor.payload)
                .unwrap()
                .contains("opened automatically by the server"),
            "the note says plainly what happened"
        );
        assert!(text.contains("auto_opened_by"), "names the identity that opened it");
        assert!(text.contains(&db.server_did().unwrap()), "and that identity is this server");
        assert_eq!(
            successor.author_public_key,
            {
                let (pk_hex, _) = db.get_or_create_server_keypair().unwrap();
                hex::decode(pk_hex).unwrap()
            },
            "signed by the server's own key, not the original proposer's"
        );
        successor
            .verify_signature()
            .expect("and the signature verifies");

        // None of the machine fields count as the question, so stripping them
        // leaves exactly what the human wrote.
        for field in SCHEDULE_AND_MACHINE_FIELDS {
            let stripped = question_bytes(&successor.payload).unwrap();
            assert!(
                !String::from_utf8_lossy(&stripped).contains(field),
                "{field} must not survive into the question bytes"
            );
        }
    }

    /// A re-vote gets a window people can actually notice, and a sane one even
    /// when the original vote was open for a single second.
    #[test]
    fn the_new_voting_window_is_at_least_a_week() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "T", "B"); // original window: 1 second

        let now = 10_000i64;
        let successor = build_scheduled_revote(&db, &parent, now).unwrap();
        db.put_signed_object(&successor, None).unwrap();
        let row = db
            .get_proposal(&successor.object_id().unwrap().to_hex())
            .unwrap()
            .unwrap();

        assert_eq!(row.opens_at, now);
        assert_eq!(row.closes_at, now + MIN_REVOTE_WINDOW_MS);
        assert_eq!(
            row.review_cadence_ms,
            Some(365 * 86_400_000),
            "the cadence carries forward, so a yearly review stays yearly"
        );
        assert_eq!(
            row.review_after,
            Some(row.closes_at + 365 * 86_400_000),
            "and the NEXT review is already scheduled one cadence after this one closes"
        );
    }

    /// IDEMPOTENCE. The scheduler runs on a timer and a relay restarts, so it
    /// will run again over the same data. Once is once.
    #[test]
    fn running_the_scheduler_twice_opens_exactly_one_revote() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "Shared tool budget", "Cap it at 200 a year.");
        ratify(&db, &parent, 4.0, 1.0);

        let now = 10_000i64;
        assert_eq!(
            db.proposals_due_for_review(now).unwrap(),
            vec![parent.clone()],
            "precondition: the decision is due for review"
        );

        let first = open_due_revotes(&db, now);
        assert_eq!(first.opened.len(), 1, "failures: {:?}", first.failed);
        assert!(first.failed.is_empty());

        let second = open_due_revotes(&db, now + 1);
        assert!(
            second.opened.is_empty() && second.failed.is_empty(),
            "a second sweep must open nothing: {second:?}"
        );

        assert_eq!(
            db.decision_chain(&parent).unwrap().len(),
            2,
            "exactly one successor exists"
        );
    }

    /// THE PROPERTY THE OPERATOR ASKED ABOUT. Opening a review does not change
    /// the answer. The standing decision stays the last CLOSED, carried one
    /// until the new vote closes and carries in its turn.
    #[test]
    fn opening_a_revote_does_not_change_the_standing_answer() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "Quiet hours", "22:00 to 07:00.");
        ratify(&db, &parent, 5.0, 1.0);
        assert_eq!(
            db.standing_decision(&parent).unwrap().as_deref(),
            Some(parent.as_str()),
            "precondition: the original decision stands"
        );

        let sweep = open_due_revotes(&db, 10_000);
        let (_, successor) = sweep.opened.first().cloned().expect("a re-vote was opened");

        assert_eq!(
            db.standing_decision(&parent).unwrap().as_deref(),
            Some(parent.as_str()),
            "the ORIGINAL decision still stands while its review is open"
        );
        // And the same answer through the route the chain endpoint takes:
        // resolve from the ROOT of the chain, which is how `standing_decision`
        // is meant to be entered (it walks forward from the link it is given,
        // so handing it a mid-chain link asks a narrower question).
        let chain = db.decision_chain(&successor).unwrap();
        assert_eq!(
            db.standing_decision(&chain[0].proposal_object_id).unwrap().as_deref(),
            Some(parent.as_str()),
            "resolving the whole chain still gives the original decision"
        );

        // Only when the re-vote closes and carries does the answer move.
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE proposals SET closes_at = 1 WHERE proposal_object_id = ?1",
                params![&successor],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();
        ratify(&db, &successor, 6.0, 2.0);
        assert_eq!(
            db.standing_decision(&parent).unwrap().as_deref(),
            Some(successor.as_str()),
            "a closed, carried review becomes the standing answer"
        );
    }

    /// The successor is an ordinary proposal in every other respect: it lists,
    /// it can be voted on, and the chain endpoint sees it.
    #[test]
    fn the_successor_is_an_ordinary_listed_proposal() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "T", "B");
        ratify(&db, &parent, 2.0, 0.0);

        let sweep = open_due_revotes(&db, 10_000);
        let (_, successor) = sweep.opened.first().cloned().unwrap();

        let listed = db.list_proposals(Some("local"), None, None, None, None).unwrap();
        assert!(
            listed.iter().any(|p| p.proposal_object_id == successor),
            "the re-vote appears in the normal proposals list"
        );
        let row = db.get_proposal(&successor).unwrap().unwrap();
        assert_eq!(row.proposal_type, "parameter_change", "type copied from the predecessor");
        assert_eq!(row.scope, "local", "scope copied from the predecessor");
        assert_eq!(row.supersedes.as_deref(), Some(parent.as_str()));
        assert_eq!(row.space_id.as_deref(), Some("test_server"));

        let chain = db.decision_chain(&successor).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].proposal_object_id, parent);
        assert_eq!(chain[1].proposal_object_id, successor);
    }

    /// A decision with no review date is never touched by any of this.
    #[test]
    fn a_decision_with_no_review_date_is_left_alone() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let payload = cbor_map(vec![
            ("proposal_type", cbor_text("parameter_change")),
            ("scope", cbor_text("local")),
            ("title", cbor_text("No review asked for")),
            ("body", cbor_text("B")),
            ("opens_at", cbor_int(1_000)),
            ("closes_at", cbor_int(2_000)),
        ]);
        let object = ObjectBuilder::new("proposal_v1")
            .created_at(1_000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(&author)
            .unwrap();
        db.put_signed_object(&object, None).unwrap();

        let sweep = open_due_revotes(&db, i64::MAX / 2);
        assert!(sweep.opened.is_empty() && sweep.failed.is_empty());
    }

    /// The question comparison has to be sensitive to a single character, or it
    /// is not a safeguard. This proves it by hand-building a successor whose
    /// title differs by one space, exactly as a buggy or hostile server would.
    #[test]
    fn one_changed_character_makes_the_question_bytes_differ() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "Quiet hours", "22:00 to 07:00.");
        let parent_question = question_bytes(&payload_of(&db, &parent)).unwrap();

        let reworded = cbor_map(vec![
            ("proposal_type", cbor_text("parameter_change")),
            ("scope", cbor_text("local")),
            ("title", cbor_text("Quiet hours ")), // one trailing space
            ("body", cbor_text("22:00 to 07:00.")),
            ("opens_at", cbor_int(10_000)),
            ("closes_at", cbor_int(20_000)),
        ]);
        let reworded_bytes = to_canonical_bytes(&reworded).unwrap();

        assert_ne!(
            question_bytes(&reworded_bytes).unwrap(),
            parent_question,
            "one character of difference must be visible in the question bytes"
        );
    }

    /// And the comparison is not fooled by field ORDER, which would make it
    /// fail on honest input and be quietly disabled by the next person.
    #[test]
    fn field_order_does_not_change_the_question_bytes() {
        let a = to_canonical_bytes(&cbor_map(vec![
            ("title", cbor_text("T")),
            ("body", cbor_text("B")),
            ("scope", cbor_text("local")),
            ("opens_at", cbor_int(1)),
        ]))
        .unwrap();
        let b = to_canonical_bytes(&cbor_map(vec![
            ("scope", cbor_text("local")),
            ("opens_at", cbor_int(999)),
            ("body", cbor_text("B")),
            ("title", cbor_text("T")),
        ]))
        .unwrap();
        assert_eq!(question_bytes(&a).unwrap(), question_bytes(&b).unwrap());
    }

    /// An unknown field counts as part of the question, so a server that
    /// invents one is caught rather than waved through.
    #[test]
    fn an_invented_field_counts_as_part_of_the_question() {
        let plain = to_canonical_bytes(&cbor_map(vec![
            ("title", cbor_text("T")),
            ("body", cbor_text("B")),
        ]))
        .unwrap();
        let with_extra = to_canonical_bytes(&cbor_map(vec![
            ("title", cbor_text("T")),
            ("body", cbor_text("B")),
            ("options", cbor_text("yes/no/maybe")),
        ]))
        .unwrap();
        assert_ne!(question_bytes(&plain).unwrap(), question_bytes(&with_extra).unwrap());
    }

    /// Sign a proposal payload with THIS SERVER's own key, the way the
    /// scheduler does. Used to hand-build the successors a compromised or buggy
    /// server would produce, and check the relay refuses them.
    fn server_signed_proposal(db: &Storage, fields: Vec<(&str, Value)>) -> Object {
        use crate::relay::core::pq_crypto::{DilithiumKeypair, derive_dilithium_seed};
        let (_, seed_hex) = db.get_or_create_server_keypair().unwrap();
        let seed = hex::decode(seed_hex).unwrap();
        let keypair = DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed));
        ObjectBuilder::new("proposal_v1")
            .space_id("test_server")
            .created_at(10_000)
            .payload_cbor(&cbor_map(fields))
            .unwrap()
            .sign(&keypair)
            .unwrap()
    }

    /// THE ENFORCEMENT, not just the intention. Even signed with the server's
    /// own key, a successor that reworded the question does not get to join the
    /// chain. This is the permanent version of a deliberate red proof: with the
    /// question check removed from `server_may_open_review`, the reworded
    /// successor links and this test fails.
    #[test]
    fn a_reworded_server_signed_review_is_refused_the_link() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "Quiet hours", "22:00 to 07:00.");
        ratify(&db, &parent, 3.0, 0.0);

        // Identical to a legitimate successor except for one character of title.
        let reworded = server_signed_proposal(
            &db,
            vec![
                ("proposal_type", cbor_text("parameter_change")),
                ("scope", cbor_text("local")),
                ("title", cbor_text("Quiet hours ")), // one trailing space
                ("body", cbor_text("22:00 to 07:00.")),
                ("opens_at", cbor_int(10_000)),
                ("closes_at", cbor_int(10_000 + MIN_REVOTE_WINDOW_MS as u64)),
                ("supersedes", cbor_text(&parent)),
                ("auto_opened_reason", cbor_text(REASON_SCHEDULED_REVIEW)),
            ],
        );
        db.put_signed_object(&reworded, None).unwrap();
        let reworded_id = reworded.object_id().unwrap().to_hex();

        // Stored, because it is a validly signed statement. Not linked, because
        // it is not the question the decision it claims to review asked.
        assert!(db.get_proposal(&reworded_id).unwrap().is_some());
        assert_eq!(
            db.get_proposal(&reworded_id).unwrap().unwrap().supersedes,
            None,
            "a reworded question must not be allowed to take over the chain"
        );
        assert_eq!(db.decision_chain(&parent).unwrap().len(), 1);

        // And the identical object with the question copied faithfully DOES link.
        let faithful = server_signed_proposal(
            &db,
            vec![
                ("proposal_type", cbor_text("parameter_change")),
                ("scope", cbor_text("local")),
                ("title", cbor_text("Quiet hours")),
                ("body", cbor_text("22:00 to 07:00.")),
                ("opens_at", cbor_int(10_000)),
                ("closes_at", cbor_int(10_000 + MIN_REVOTE_WINDOW_MS as u64)),
                ("supersedes", cbor_text(&parent)),
                ("auto_opened_reason", cbor_text(REASON_SCHEDULED_REVIEW)),
            ],
        );
        db.put_signed_object(&faithful, None).unwrap();
        assert_eq!(
            db.get_proposal(&faithful.object_id().unwrap().to_hex())
                .unwrap()
                .unwrap()
                .supersedes
                .as_deref(),
            Some(parent.as_str()),
            "the same object with the question copied faithfully must link"
        );
    }

    /// The server may only open a review somebody scheduled. It cannot decide
    /// on its own that a settled question should be reopened.
    #[test]
    fn the_server_cannot_review_a_decision_that_asked_for_no_review() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();

        // No review_after, no cadence: this decision asked to be left alone.
        let payload = cbor_map(vec![
            ("proposal_type", cbor_text("parameter_change")),
            ("scope", cbor_text("local")),
            ("title", cbor_text("Settled")),
            ("body", cbor_text("B")),
            ("opens_at", cbor_int(1_000)),
            ("closes_at", cbor_int(2_000)),
        ]);
        let original = ObjectBuilder::new("proposal_v1")
            .space_id("test_server")
            .created_at(1_000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(&author)
            .unwrap();
        db.put_signed_object(&original, None).unwrap();
        let parent = original.object_id().unwrap().to_hex();

        // The server tries anyway, copying the question faithfully.
        let uninvited = server_signed_proposal(
            &db,
            vec![
                ("proposal_type", cbor_text("parameter_change")),
                ("scope", cbor_text("local")),
                ("title", cbor_text("Settled")),
                ("body", cbor_text("B")),
                ("opens_at", cbor_int(10_000)),
                ("closes_at", cbor_int(10_000 + MIN_REVOTE_WINDOW_MS as u64)),
                ("supersedes", cbor_text(&parent)),
            ],
        );
        db.put_signed_object(&uninvited, None).unwrap();
        assert_eq!(
            db.get_proposal(&uninvited.object_id().unwrap().to_hex())
                .unwrap()
                .unwrap()
                .supersedes,
            None,
            "no review was scheduled, so the server may not open one"
        );
        assert_eq!(db.decision_chain(&parent).unwrap().len(), 1);
    }

    /// A failed attempt must not turn into an hourly stream of near-duplicate
    /// proposals. Here the parent's object is deleted, so the question cannot be
    /// copied and the attempt fails on purpose.
    #[test]
    fn a_failed_attempt_goes_on_cooldown_instead_of_retrying_every_hour() {
        let db = test_storage();
        let author = DilithiumKeypair::generate().unwrap();
        let parent = seed_decision(&db, &author, "T", "B");
        db.with_conn(|conn| {
            conn.execute(
                "DELETE FROM signed_objects WHERE object_id = ?1",
                params![&parent],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .unwrap();

        let now = 10_000i64;
        let first = open_due_revotes(&db, now);
        assert_eq!(first.failed.len(), 1, "the attempt fails, loudly");
        assert!(first.opened.is_empty());

        let second = open_due_revotes(&db, now + 60_000);
        assert_eq!(second.skipped, 1, "and the next sweep leaves it alone");
        assert!(second.failed.is_empty() && second.opened.is_empty());

        // After the cooldown it is tried again, because the fault may be fixed.
        let later = open_due_revotes(&db, now + RETRY_COOLDOWN_MS + 1);
        assert_eq!(later.failed.len(), 1);
    }
}
