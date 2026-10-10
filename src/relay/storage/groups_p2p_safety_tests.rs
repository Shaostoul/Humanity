//! A `#[path]` child of groups_p2p.rs: the three group fixes of 2026-10-10 (a removed member's
//! old ticket, an epoch key overwritten, the newest messages served). Its own small builders,
//! each taking the signed `created_at`, because these tests turn on which object is older.

use super::*;
use crate::relay::core::object::ObjectBuilder;
use crate::relay::core::pq_crypto::DilithiumKeypair;
use ciborium::Value;

fn db(tag: &str) -> Storage {
    Storage::open_temp(&format!("p2pgroups_safety_{tag}"))
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

/// A signed object of `kind` referencing `refs`, with `fields` as its payload, stamped
/// `created_at` (None: no stamp at all).
fn signed(by: &DilithiumKeypair, kind: &str, refs: &[&str], created_at: Option<u64>, fields: Vec<(&str, Value)>) -> Object {
    let mut b = ObjectBuilder::new(kind);
    for r in refs {
        b = b.reference(r);
    }
    if let Some(t) = created_at {
        b = b.created_at(t);
    }
    let payload = Value::Map(fields.into_iter().map(|(k, v)| (text(k), v)).collect());
    b.payload_cbor(&payload).unwrap().sign(by).unwrap()
}

fn id(o: &Object) -> String {
    o.object_id().unwrap().to_hex()
}

/// Put `o` and return whether it was stored (a refusal is an Err, counted as not stored).
fn put(db: &Storage, o: &Object) -> bool {
    db.put_signed_object(o, None).unwrap_or(false)
}

/// A group `creator` made; its id.
fn group(db: &Storage, creator: &DilithiumKeypair) -> String {
    group_named(db, creator, "Workshop")
}

fn group_named(db: &Storage, creator: &DilithiumKeypair, name: &str) -> String {
    let g = signed(creator, "group_v1", &[], Some(1_000), vec![("name", text(name))]);
    assert!(put(db, &g));
    id(&g)
}

/// A ticket's invite object, stamped `created_at`, valid for a week from now; its id.
fn invite(db: &Storage, creator: &DilithiumKeypair, gid: &str, secret: &[u8], created_at: Option<u64>) -> String {
    let week = now_millis() + 7 * 86_400_000;
    let i = signed(
        creator,
        "group_invite_v1",
        &[gid],
        created_at,
        vec![("expires_at", Value::Integer(week.into())), ("secret_hash", Value::Bytes(blake3::hash(secret).as_bytes().to_vec()))],
    );
    assert!(put(db, &i));
    id(&i)
}

/// `who` joins on the ticket (`invite_id`, `secret`), stamped `created_at` (each join a distinct
/// object, or the store would take a repeat as already seen); whether they are a member after.
fn join(db: &Storage, who: &DilithiumKeypair, gid: &str, invite_id: &str, secret: &[u8], created_at: u64) -> bool {
    let j = signed(who, "group_join_v1", &[gid, invite_id], Some(created_at), vec![("secret", Value::Bytes(secret.to_vec()))]);
    assert!(put(db, &j), "the join object itself is always stored");
    db.p2p_group_has_member(gid, &who.public_key()).unwrap()
}

/// `by` removes `subject` (a creator's removal, or a leave when they are the same).
fn remove(db: &Storage, by: &DilithiumKeypair, gid: &str, subject: &DilithiumKeypair, created_at: Option<u64>) {
    let r = signed(by, "group_member_v1", &[gid], created_at, vec![("action", text("remove")), ("subject", Value::Bytes(subject.public_key()))]);
    assert!(put(db, &r));
    assert!(!db.p2p_group_has_member(gid, &subject.public_key()).unwrap(), "precondition: removed");
}

fn fp(who: &DilithiumKeypair) -> String {
    author_fingerprint(&who.public_key())
}

/// A key object for `epoch` sealed to `to` (only the fingerprints matter to the relay).
fn epoch_key(by: &DilithiumKeypair, gid: &str, epoch: u64, to: &[&DilithiumKeypair], created_at: u64) -> Object {
    let recipients = to
        .iter()
        .map(|who| {
            Value::Map(vec![
                (text("fp"), text(&fp(who))),
                (text("ek_ct"), Value::Bytes(vec![1; 8])),
                (text("nonce"), Value::Bytes(vec![2; 12])),
                (text("ct"), Value::Bytes(vec![3; 16])),
            ])
        })
        .collect();
    signed(by, "group_epoch_key_v1", &[gid], Some(created_at), vec![("epoch", Value::Integer(epoch.into())), ("recipients", Value::Array(recipients))])
}

// ── A removed member and their old ticket ──

/// Someone the creator removed cannot come back on a ticket issued before the removal, and can
/// on one issued after it (until they are removed again); another person joins on the old ticket
/// as before; someone who left on their own comes back on a ticket they hold.
///
/// Seen red 2026-10-10 with the `invite_postdates_removal` call taken out of `index_group_join`:
/// "a removed member's rejoin on the old ticket is refused" failed.
#[test]
fn a_removed_member_comes_back_only_on_a_ticket_issued_after_the_removal() {
    let db = db("rejoin");
    let (creator, alice, bob) = (DilithiumKeypair::generate().unwrap(), DilithiumKeypair::generate().unwrap(), DilithiumKeypair::generate().unwrap());
    let gid = group(&db, &creator);
    let old = invite(&db, &creator, &gid, b"old ticket", Some(1_100));
    assert!(join(&db, &alice, &gid, &old, b"old ticket", 1_200), "precondition: the ticket works");
    remove(&db, &creator, &gid, &alice, Some(1_300));

    assert!(!join(&db, &alice, &gid, &old, b"old ticket", 1_400), "a removed member's rejoin on the old ticket is refused");
    assert!(join(&db, &bob, &gid, &old, b"old ticket", 1_450), "another person's join on the old ticket still works");

    let new = invite(&db, &creator, &gid, b"new ticket", Some(1_500));
    assert!(join(&db, &alice, &gid, &new, b"new ticket", 1_600), "a ticket issued after the removal lets them back");
    remove(&db, &creator, &gid, &alice, Some(1_700));
    assert!(!join(&db, &alice, &gid, &new, b"new ticket", 1_800), "removed again, that ticket is old too");

    // A leave is not a removal: the person may come back on a ticket they hold.
    remove(&db, &bob, &gid, &bob, Some(1_900));
    assert!(join(&db, &bob, &gid, &old, b"old ticket", 2_000), "someone who left on their own comes back on their ticket");
}

/// The two stamps compared are the creator's own, so the order objects reach this relay in
/// does not matter: an old invite that arrives here only after the removal is still old. With no
/// stamp on the invite, arrival here decides: an invite that was here before the removal is old,
/// one that arrived after it is new.
#[test]
fn issued_after_is_the_creators_stamp_or_else_arrival_here() {
    let db = db("rejoin_stamps");
    let (creator, alice) = (DilithiumKeypair::generate().unwrap(), DilithiumKeypair::generate().unwrap());
    let gid = group(&db, &creator);
    let first = invite(&db, &creator, &gid, b"first", Some(1_100));
    assert!(join(&db, &alice, &gid, &first, b"first", 1_200));
    remove(&db, &creator, &gid, &alice, Some(5_000));
    let late_old = invite(&db, &creator, &gid, b"stamped before", Some(4_000)); // arrives after the removal
    assert!(!join(&db, &alice, &gid, &late_old, b"stamped before", 5_100), "stamped before the removal: old, whenever it arrived");
    let stamped_after = invite(&db, &creator, &gid, b"stamped after", Some(5_001));
    assert!(join(&db, &alice, &gid, &stamped_after, b"stamped after", 5_200), "stamped after it: new");

    // No stamp on the invite: arrival at this relay decides.
    let gid = group_named(&db, &creator, "Unstamped");
    let first = invite(&db, &creator, &gid, b"first", Some(1_100));
    assert!(join(&db, &alice, &gid, &first, b"first", 1_200));
    let unstamped_before = invite(&db, &creator, &gid, b"unstamped before", None);
    std::thread::sleep(std::time::Duration::from_millis(5)); // distinct arrival times
    remove(&db, &creator, &gid, &alice, Some(5_000));
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(!join(&db, &alice, &gid, &unstamped_before, b"unstamped before", 5_100), "unstamped and here before the removal: old");
    let unstamped_after = invite(&db, &creator, &gid, b"unstamped after", None);
    assert!(join(&db, &alice, &gid, &unstamped_after, b"unstamped after", 5_200), "unstamped and here after it: new");
}

// ── An epoch's key cannot be overwritten ──

/// A key for an epoch that has one is accepted only as a re-seal of the latest epoch to everyone
/// still in the group: a re-seal to a bigger roster is accepted; one that leaves a member out is
/// refused (and not stored), the stored key unchanged; someone who left may be dropped; a new
/// higher epoch is accepted as before; an older epoch cannot be replaced at all.
///
/// Seen red 2026-10-10 with the `epoch_key_refusal` checks taken out of `index_group_epoch_key`
/// and `put_signed_object`: "a key that leaves a member out is refused" failed (the object was
/// stored and became epoch 1's key).
#[test]
fn an_epochs_key_is_replaced_only_by_a_reseal_that_drops_no_member() {
    let db = db("epochs");
    let people: Vec<DilithiumKeypair> = (0..4).map(|_| DilithiumKeypair::generate().unwrap()).collect();
    let (creator, alice, bob, carol) = (&people[0], &people[1], &people[2], &people[3]);
    let gid = group(&db, creator);
    let ticket = invite(&db, creator, &gid, b"t", Some(1_100));
    assert!(join(&db, alice, &gid, &ticket, b"t", 1_200));
    let latest = |db: &Storage| db.p2p_group_latest_epoch_object(&gid).unwrap();

    let e1 = epoch_key(creator, &gid, 1, &[creator, alice], 2_000);
    assert!(put(&db, &e1));
    assert!(join(&db, bob, &gid, &ticket, b"t", 2_100));
    let reseal = epoch_key(creator, &gid, 1, &[creator, alice, bob], 2_200);
    assert!(put(&db, &reseal), "a re-seal to a bigger roster is accepted");
    assert_eq!(latest(&db), Some(id(&reseal)));

    let drops_alice = epoch_key(creator, &gid, 1, &[creator, bob], 2_300);
    let refused = db.put_signed_object(&drops_alice, None).unwrap_err().to_string();
    assert!(refused.contains("group key refused"), "a key that leaves a member out is refused: {refused}");
    assert_eq!(latest(&db), Some(id(&reseal)), "the stored key is unchanged");
    assert!(db.get_signed_object(&id(&drops_alice)).unwrap().is_none(), "and the refused object is not kept");
    assert_eq!(db.index_group_epoch_key(&drops_alice).unwrap(), false, "the projection refuses it too");
    assert_eq!(latest(&db), Some(id(&reseal)));

    // Alice leaves; Carol joins. A re-seal without Alice is accepted: she is not in the group.
    remove(&db, alice, &gid, alice, Some(2_400));
    assert!(join(&db, carol, &gid, &ticket, b"t", 2_500));
    let after_leave = epoch_key(creator, &gid, 1, &[creator, bob, carol], 2_600);
    assert!(put(&db, &after_leave), "someone who left may be dropped from a re-seal");

    let e2 = epoch_key(creator, &gid, 2, &[creator, carol], 3_000);
    assert!(put(&db, &e2), "a new higher epoch is accepted as before, sealed to whoever the creator chooses");
    assert_eq!(latest(&db), Some(id(&e2)));

    let old_epoch = epoch_key(creator, &gid, 1, &[creator, bob, carol], 3_100);
    let refused = db.put_signed_object(&old_epoch, None).unwrap_err().to_string();
    assert!(refused.contains("not this group's latest epoch"), "an older epoch cannot be replaced: {refused}");
    assert_eq!(db.p2p_group_all_epoch_objects(&gid).unwrap(), [id(&after_leave), id(&e2)]);

    // The same object again is not a replacement and is not refused.
    assert!(!put(&db, &e2), "a repeat is simply already stored");
    assert!(db.put_signed_object(&e2, None).is_ok());
}

// ── The newest messages ──

/// With 250 messages, the 200 newest come back, in time order (both apps draw in that order).
///
/// Seen red 2026-10-10 against the old query (`ORDER BY created_at ASC ... LIMIT`): the oldest
/// 200 came back.
#[test]
fn a_groups_message_list_is_its_newest_in_time_order() {
    let db = db("messages");
    let creator = DilithiumKeypair::generate().unwrap();
    let gid = group(&db, &creator);
    // Put in a scrambled order, so the order out comes from the stamps, not from arrival.
    let mut ids: Vec<(u64, String)> = Vec::new();
    for i in (0..250u64).map(|i| (i * 7) % 250) {
        let at = 10_000 + i;
        let m = signed(&creator, "group_msg_v1", &[gid.as_str()], Some(at), vec![("epoch", Value::Integer(1.into())), ("ct", Value::Bytes(vec![i as u8]))]);
        assert!(put(&db, &m));
        ids.push((at, id(&m)));
    }
    ids.sort();
    let newest: Vec<String> = ids[50..].iter().map(|(_, id)| id.clone()).collect();
    assert_eq!(db.p2p_group_message_ids(&gid, 200).unwrap(), newest, "the newest 200, oldest of them first");
    let few = db.p2p_group_message_ids(&gid, 3).unwrap();
    assert_eq!(few, newest[197..].to_vec(), "a smaller cap is the newest few, still in time order");
}
