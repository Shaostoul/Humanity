//! The owner claim, the handover and the owner's protection (2026-11-20, handlers/owner.rs). Each
//! test's red evidence is in its comment.

use super::*;
use crate::relay::handlers::held::export_part_path;
use crate::relay::handlers::held::tests::{act, block, decide, drain, make_owner, one_held, person, reported_post, signed, told, Dirs, Person, TS};
use crate::relay::storage::held::restore::open_plan;
use crate::relay::storage::held::{held_dir_beside, part_file};
use crate::relay::storage::owner::handover::{ChangeKind, OwnerChange};
use crate::relay::storage::owner::is_owner_protected;
use crate::relay::storage::Storage;
use humanity_protocol::held_seal::{self, acceptance_digest, check_rewrapped_package, header_hash, header_len, open_bytes, part_context, rewrap_header, seal_bytes, Manifest};
use humanity_protocol::key_card;
use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};

fn give_card(st: &RelayState, who: &Person) {
    let c = key_card::sign_card(&who.seed, TS).unwrap();
    assert!(st.db.store_key_card(&c).unwrap().stored());
}

/// A fresh server with its claim code minted into a test folder of its own: the state, the code,
/// the folder (kept alive) and the file.
fn minted(tag: &str) -> (Arc<RelayState>, String, humanity_core::test_temp::TempPath, std::path::PathBuf) {
    let mut st = RelayState::new(Storage::open_temp_dir(&format!("owner_{tag}")));
    let root = humanity_core::test_temp::dir(&format!("owner_{tag}"));
    let file = root.path().join("owner-claim-code.txt");
    let code = mint_claim_code_at(&mut st, &file).expect("a code while no key holds the role");
    (Arc::new(st), code, root, file)
}

/// The words each `owner_reply` in `all` told `who`.
fn replied(all: &[RelayMessage], who: &str) -> String {
    all.iter()
        .filter_map(|m| match m {
            RelayMessage::OwnerReply { to, message, .. } if to == who => Some(message.clone()),
            RelayMessage::Private { to, message } if to == who => Some(message.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn try_claim(st: &Arc<RelayState>, who: &Person, code: &str, misses: &mut u32) -> (Vec<RelayMessage>, String) {
    let mut rx = st.broadcast_tx.subscribe();
    block(handle(st, &who.key, "owner_claim", &json!({ "type": "owner_claim", "code": code }), misses));
    let all = drain(&mut rx);
    let words = replied(&all, &who.key);
    (all, words)
}

/// The code is minted only while no key holds the owner role, written to its file, and in no
/// message the claim sends to any client; the right code (in any letter case) makes its bearer
/// the owner, burns the code and deletes the file; everyone is told the server has an owner.
///
/// Seen red 2026-11-20 with the success words carrying the code ("the code reached a client").
#[test]
fn the_code_is_minted_only_without_an_owner_and_reaches_no_client() {
    let (st, code, root, file) = minted("mint");
    assert_eq!(code.len(), 16, "16 hex digits");
    assert!(std::fs::read_to_string(&file).unwrap().contains(&code), "the file holds the code");

    let mut owned = RelayState::new(Storage::open_temp_dir("owner_mint_owned"));
    owned.db.claim_owner(&person(31).key, 0).unwrap();
    let stale = root.path().join("stale.txt");
    std::fs::write(&stale, "an old code").unwrap();
    assert!(mint_claim_code_at(&mut owned, &stale).is_none(), "no code on a server with an owner");
    assert!(!stale.exists(), "and a stale file goes");

    let ann = person(32);
    give_card(&st, &ann);
    let mut misses = 0;
    let (mut all, _) = try_claim(&st, &ann, "0000000000000000", &mut misses);
    let (more, words) = try_claim(&st, &ann, &code.to_lowercase(), &mut misses);
    all.extend(more);
    assert!(words.contains("You are now this server's owner"), "{words}");
    for m in &all {
        let j = serde_json::to_string(m).unwrap();
        assert!(!j.contains(&code) && !j.contains(&code.to_lowercase()), "the code reached a client: {j}");
    }
    assert!(all.iter().any(|m| matches!(m, RelayMessage::OwnerState { has_owner: true })), "everyone is told the server has an owner");
    assert!(st.db.is_owner(&ann.key) && st.db.owner_keys().unwrap() == [ann.key.clone()]);
    assert_eq!(st.db.owner_log(5).unwrap()[0].kind, "claim", "the claim is in the owner log");
    assert!(!file.exists(), "the file is deleted on success");
}

/// A wrong code is refused, counted per connection AND per key (a reconnect does not reset the
/// key's count), and logged; but NO count, by anyone, ever stops the right code from claiming (the
/// owner review's finding 4: the server-wide count was checked before the code, so one key's thirty
/// wrong guesses locked the real owner out). A second use of the right code is refused; a claimer
/// whose key card has not reached the server is refused with words that say how to get one, and
/// the code still works for them once it has.
///
/// Seen red 2026-11-20 with the branch's order put back (the per-connection and server-wide
/// refusals before the comparison): the stranger's flood stopped being counted after five on its
/// connection (`overall >= MISSES_OVERALL` failed), the refusal answering instead of the compare.
#[test]
fn wrong_codes_are_counted_but_never_stop_the_right_code() {
    let (st, code, _root, _file) = minted("misses");
    let (ann, bob, cy, mal) = (person(33), person(34), person(35), person(36));
    give_card(&st, &ann);

    let mut ann_misses = 0;
    for _ in 0..MISSES_PER_CONNECTION {
        let (_, words) = try_claim(&st, &ann, "FFFFFFFFFFFFFFFF", &mut ann_misses);
        assert!(words.contains("Wrong claim code"), "{words}");
    }
    assert_eq!(ann_misses, MISSES_PER_CONNECTION, "each wrong code is counted on the connection");
    let mut reconnected = 0;
    try_claim(&st, &ann, "FFFFFFFFFFFFFFFE", &mut reconnected);
    assert_eq!(st.owner_claim_misses.lock().unwrap().per_key.get(&ann.key).map(|e| e.0), Some(MISSES_PER_CONNECTION + 1), "and on the key, across a reconnect");

    // A stranger's flood: thirty and more wrong codes from one key, past every count.
    let mut mal_misses = 0;
    for i in 0..(MISSES_OVERALL + 2) {
        try_claim(&st, &mal, &format!("{:016X}", i), &mut mal_misses);
    }
    assert!(st.owner_claim_misses.lock().unwrap().overall >= MISSES_OVERALL);

    let (_, words) = try_claim(&st, &cy, &code, &mut 0);
    assert!(words.contains("your key card has not reached this server") && words.contains("Sign in again"), "no key card, no claim: {words}");
    assert!(!st.db.has_owner(), "nothing changed");
    give_card(&st, &cy);
    let (_, words) = try_claim(&st, &cy, &code, &mut 0);
    assert!(st.db.is_owner(&cy.key), "the right code claims after a stranger's thirty wrong ones: {words}");
    assert!(block(st.claim_code.read()).is_none(), "the code is burned");

    give_card(&st, &bob);
    let (_, words) = try_claim(&st, &bob, &code, &mut 0);
    assert!(words.contains("already has an owner") && !st.db.is_owner(&bob.key), "a second use is refused: {words}");
    assert_eq!(st.db.owner_keys().unwrap(), [cy.key.clone()]);
}

/// The right code claims even from a key and a connection that have tried many wrong ones (an
/// owner who mistyped). Seen red 2026-11-20 on the branch as reviewed: "the right code after five
/// wrong ones on this connection" failed ("takes no more").
#[test]
fn the_right_code_claims_after_ones_own_wrong_tries() {
    let (st, code, _root, _file) = minted("own_misses");
    let ann = person(37);
    give_card(&st, &ann);
    let mut misses = 0;
    for _ in 0..(MISSES_PER_KEY + 1) {
        try_claim(&st, &ann, "0000000000000001", &mut misses);
    }
    let (_, words) = try_claim(&st, &ann, &code, &mut misses);
    assert!(st.db.is_owner(&ann.key), "the right code after five wrong ones on this connection: {words}");
}

/// THE WRONG CODES REMEMBERED PER KEY ARE BOUNDED (the second review, R7: the map was never
/// pruned): at most MISS_KEYS_MAX keys, the one whose latest wrong code is oldest forgotten first,
/// and every entry forgotten once its window has passed.
///
/// Seen red 2026-11-20 with `ClaimMisses::count` neither pruning nor capping (as the branch had
/// it): "at most MISS_KEYS_MAX keys" failed.
#[test]
fn the_wrong_codes_remembered_per_key_are_bounded() {
    let mut m = ClaimMisses::default();
    for i in 0..(MISS_KEYS_MAX + 50) {
        m.count(&format!("k{i}"), 1_000 + i as i64);
    }
    assert_eq!(m.per_key.len(), MISS_KEYS_MAX, "at most MISS_KEYS_MAX keys");
    assert!(!m.per_key.contains_key("k0") && m.per_key.contains_key(&format!("k{}", MISS_KEYS_MAX + 49)), "the oldest forgotten first");
    assert_eq!(m.count("k60", 1_100), 2, "a key still remembered counts on");
    assert_eq!(m.count("late", 1_000 + 3 * MISS_WINDOW_MS), 1);
    assert_eq!(m.per_key.len(), 1, "every entry past its window forgotten");
}

// ── The handover ──

fn signed_handover(st: &RelayState, who: &Person, target: &str) -> Value {
    signed_owner(st, who, "owner_handover", target)
}

fn signed_owner(st: &RelayState, who: &Person, action: &str, target: &str) -> Value {
    signed_mod(st, who, action, target, "identity", "handing the server on")
}

/// `who`'s own signed `mod_action_v1` for this server, as the JSON a client sends under `action`.
fn signed_mod(st: &RelayState, who: &Person, action: &str, target: &str, target_kind: &str, reason: &str) -> Value {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use humanity_protocol::moderation::{build_mod_action_v1, ModAction};
    let a = ModAction { action, target, target_kind, reason, rule: "", expires_at: 0, role: "" };
    let o = build_mod_action_v1(&who.seed, &st.db.server_did().unwrap(), &a, crate::relay::storage::now_millis(), &[]).unwrap();
    json!({
        "protocol_version": o.protocol_version, "object_type": o.object_type, "space_id": o.space_id, "channel_id": o.channel_id,
        "author_public_key_b64": B64.encode(&o.author_public_key), "created_at": o.created_at, "references": o.references,
        "payload_schema_version": o.payload_schema_version, "payload_encoding": o.payload_encoding,
        "payload_b64": B64.encode(&o.payload), "signature_b64": B64.encode(&o.signature),
    })
}

/// Start a handover from `own` to `to` with `action`: the grant and what the owner was told.
fn begin_with(st: &RelayState, d: &Dirs, own: &Person, action: Value) -> (Option<String>, String) {
    let mut rx = st.broadcast_tx.subscribe();
    begin_in(st, &own.key, &json!({ "type": "owner_change_begin", "action": action }), d.folders(), now_ms());
    let all = drain(&mut rx);
    let grant = all.iter().find_map(|m| match m {
        RelayMessage::OwnerChangeReady { to, grant, .. } if *to == own.key => Some(grant.clone()),
        _ => None,
    });
    (grant, replied(&all, &own.key))
}

fn kyber_of(who: &Person) -> KyberKeypair {
    KyberKeypair::from_seed(&derive_kyber_seed(&who.seed)).unwrap()
}

/// `p`'s Kyber768 key from the card the relay holds.
fn card_of(st: &RelayState, p: &Person) -> Vec<u8> {
    key_card::kyber_bytes(&st.db.get_key_card(&p.key).unwrap().unwrap()).unwrap()
}

/// What the owner's device does to each new header before it sends it (a test's sabotage).
type Tamper<'a> = &'a dyn Fn(usize, &mut Vec<u8>);

/// The owner's device, as both apps run it (format v4): fetch every part's CURRENT header under
/// `grant` (as the owner, the caller its signed requests prove), re-wrap it for `to`'s cards and send
/// the new header, leaving out part `skip` of every item and passing each through `tamper`.
fn rewrap_all_with(st: &RelayState, d: &Dirs, own: &Person, to: &[&Person], grant: &str, skip: Option<usize>, tamper: Option<Tamper>) {
    let kp = kyber_of(own);
    let kybers: Vec<Vec<u8>> = to.iter().map(|p| card_of(st, p)).collect();
    let kyber: Vec<&[u8]> = kybers.iter().map(|k| k.as_slice()).collect();
    let now = now_ms();
    for row in st.db.held_items().unwrap() {
        for p in 0..row.parts as usize {
            let header = current_header_in(st, row.id, p as i64, grant, &own.key, d.folders(), now).expect("the owner is served the header");
            assert_eq!(header.len(), header_len(row.recipients()), "a header alone, never a body");
            let mut new = rewrap_header(&kp, &part_context(&row.package, p), &header, &kyber).unwrap();
            if Some(p) == skip {
                continue;
            }
            if let Some(t) = tamper {
                t(p, &mut new);
            }
            let r = new_header_in(st, row.id, p as i64, grant, &own.key, &new, d.folders(), now);
            assert_eq!(r.status().as_u16(), 200, "part {p}'s new header is taken");
        }
    }
}

fn rewrap_all(st: &RelayState, d: &Dirs, own: &Person, to: &[&Person], grant: &str, skip: Option<usize>) {
    rewrap_all_with(st, d, own, to, grant, skip, None)
}

/// The owner's `owner_change_finish` (no keys travel any more).
fn finish(st: &RelayState, d: &Dirs, own: &Person, grant: &str) -> Result<(), (&'static str, String)> {
    finish_in(st, &own.key, &json!({ "grant": grant }), d.folders(), now_ms())
}

/// The digest `who`'s device signs for the change now waiting under `grant`: every new header
/// fetched as `who`, its own slot checked (core `check_rewrapped_package`, at its place in the slot
/// order), and the round the relay sent. Its device's refusal in words.
fn digest_as(st: &RelayState, d: &Dirs, who: &Person, grant: &str) -> Result<String, String> {
    let kp = kyber_of(who);
    let now = now_ms();
    let p = pending_for(st, grant, now).ok_or("nothing is pending")?;
    let slot = p.change.recipients.iter().position(|(k, _)| *k == who.key).ok_or("not a recipient")?;
    let mut items = Vec::new();
    for id in &p.ids {
        let Some(row) = st.db.held_item(*id).unwrap() else { continue };
        let hashes = check_rewrapped_package(&kp, &row.package, row.parts as usize, p.change.recipients.len(), slot, |part| {
            accept::rewrapped_header_in(st, row.id, part as i64, grant, &who.key, d.folders(), now).ok_or_else(|| "no new header for this caller".to_string())
        })?;
        items.push((row.package.clone(), hashes));
    }
    Ok(acceptance_digest(&p.round, &items))
}

/// A recipient's device, as both apps run it (handlers/owner_accept.rs): check, then sign an
/// acceptance. Its device's refusal, or the relay's answer.
fn accept_as(st: &RelayState, d: &Dirs, who: &Person, grant: &str) -> Result<Option<OwnerChange>, String> {
    let digest = digest_as(st, d, who, grant)?;
    send_accept(st, d, who, grant, &digest)
}

/// `who`'s signed acceptance of `grant` with `digest`, as the relay gets it (no restore keys).
fn send_accept(st: &RelayState, d: &Dirs, who: &Person, grant: &str, digest: &str) -> Result<Option<OwnerChange>, String> {
    let action = signed_mod(st, who, humanity_protocol::moderation::OWNER_ACCEPT, grant, "object", &format!("checked {digest}"));
    accept::accept_in(st, &who.key, &json!({ "action": action }), d.folders(), now_ms())
}

/// Every sealed part of every held item, by package and part.
fn parts_on_disk(st: &RelayState, d: &Dirs) -> Vec<(String, usize, Vec<u8>)> {
    let mut out = Vec::new();
    for row in st.db.held_items().unwrap() {
        for p in 0..row.parts as usize {
            out.push((row.package.clone(), p, std::fs::read(held_dir_beside(d.folders()).join(&row.package).join(part_file(p))).unwrap()));
        }
    }
    out
}

/// Every body (a part less its header), by package and part: what no change may alter.
fn bodies(st: &RelayState, d: &Dirs) -> Vec<(String, usize, Vec<u8>)> {
    parts_on_disk(st, d).into_iter().map(|(pkg, p, sealed)| (pkg, p, sealed[held_seal::parse_header(&sealed).map(|h| header_len(h.recipients)).unwrap()..].to_vec())).collect()
}

fn bo_member(st: &RelayState) -> Person {
    let bo = person(40);
    st.db.register_name("Bo", &bo.key).unwrap();
    give_card(st, &bo);
    bo
}

/// A handover while the owner is alive: refused to a member without a key card and to anyone but
/// the owner; the owner's device re-wraps every part's header for the new owner's card, fetching
/// and sending headers alone; the relay changes NOTHING until the new owner's own device has checked
/// its slot of every new header and accepted (the owner review's finding 5); only the new owner may
/// fetch the new headers, not the owner, an admin or anyone else. Then every part opens with the new
/// owner's test seed, as the same bytes, and no longer with the old owner's; EVERY BODY BYTE IS AS IT
/// WAS (format v4: a change re-wraps, it never re-encrypts); part 0 still carries the restore key that
/// opens the unchanged restore plan; the old owner is an admin; the log has the handover and the
/// acceptance; the same signed handover is refused a second time; nothing of it is left behind.
///
/// Seen red 2026-11-20 with `owner_handover_in` leaving `held_items.sealed_to` as it was ("every
/// held item is sealed to the new owner"); and on the branch as first reviewed, where finish swapped
/// at once: "nothing changes before the new owner accepts".
#[test]
fn a_handover_rewraps_every_header_and_keeps_every_body_byte() {
    let d = crate::relay::handlers::held::tests::dirs("owner_handover");
    let (st, _id, ada, own, row) = one_held("owner_handover", &d);
    let before = parts_on_disk(&st, &d);
    let bodies_before = bodies(&st, &d);
    assert!(before.len() >= 2, "a package with files");

    let nocard = person(41);
    st.db.register_name("Nocard", &nocard.key).unwrap();
    let (grant, words) = begin_with(&st, &d, &own, signed_handover(&st, &own, &nocard.key));
    assert!(grant.is_none() && words.contains("no key card here that checks out"), "{words}");
    let bo = bo_member(&st);
    let (grant, words) = begin_with(&st, &d, &ada, signed_handover(&st, &ada, &bo.key));
    assert!(grant.is_none() && words.contains("only this server's owner"), "an admin cannot hand the role over: {words}");

    let action = signed_handover(&st, &own, &bo.key);
    let (grant, words) = begin_with(&st, &d, &own, action.clone());
    let grant = grant.unwrap_or_else(|| panic!("no ready frame: {words}"));
    assert!(current_header_in(&st, row.id, 0, &grant, &ada.key, d.folders(), now_ms()).is_none(), "an admin is not served a header");
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    let mut rx = st.broadcast_tx.subscribe();
    finish(&st, &d, &own, &grant).unwrap();
    let check = drain(&mut rx).into_iter().find_map(|m| match m {
        RelayMessage::OwnerChangeCheck { ref to, ref object, ref round, .. } if *to == bo.key => Some((object.clone(), round.clone())),
        _ => None,
    });
    let (object, round) = check.expect("the new owner is asked to check");
    assert_eq!(object, action, "the check carries the owner's signed action, for the device to check");
    assert_eq!(round.len(), 32, "and the round");
    assert!(st.db.is_owner(&own.key) && !st.db.is_owner(&bo.key), "nothing changes before the new owner accepts");
    assert_eq!(parts_on_disk(&st, &d), before, "every original is in place until then");
    for who in [&own, &ada] {
        assert!(accept::rewrapped_header_in(&st, row.id, 0, &grant, &who.key, d.folders(), now_ms()).is_none(), "only a recipient fetches a new header");
    }
    let change = accept_as(&st, &d, &bo, &grant).unwrap().expect("the last acceptance carries it out");
    assert_eq!(change.target, bo.key);

    assert!(st.db.is_owner(&bo.key) && st.db.owner_keys().unwrap() == [bo.key.clone()], "the new owner holds the role alone");
    assert_eq!(st.db.get_role(&own.key).unwrap(), "admin", "the old owner is an admin");
    let rows = st.db.held_items().unwrap();
    assert!(rows.iter().all(|r| r.sealed_to == bo.key), "every held item is sealed to the new owner");
    assert_eq!(bodies(&st, &d), bodies_before, "every body byte is as it was");
    let (bo_kp, own_kp) = (kyber_of(&bo), kyber_of(&own));
    for (package, p, sealed) in parts_on_disk(&st, &d) {
        let (_, _, old) = before.iter().find(|(pk, pp, _)| *pk == package && *pp == p).unwrap();
        let was = open_bytes(&own_kp, &part_context(&package, p), old).unwrap();
        assert_eq!(open_bytes(&bo_kp, &part_context(&package, p), &sealed).unwrap(), was, "part {p} opens for the new owner as the same bytes");
        assert!(open_bytes(&own_kp, &part_context(&package, p), &sealed).is_err(), "part {p} no longer opens for the old owner");
        assert_eq!(sealed.len(), old.len(), "part {p} is as long as before");
    }
    let m: Manifest = serde_json::from_slice(&open_bytes(&bo_kp, &part_context(&row.package, 0), &std::fs::read(held_dir_beside(d.folders()).join(&row.package).join(part_file(0))).unwrap()).unwrap()).unwrap();
    assert!(open_plan(&rows[0], d.folders(), &m.restore_key).is_ok(), "the restore plan still opens with the key inside part 0");
    let log = st.db.owner_log(5).unwrap();
    let handover = log.iter().find(|l| l.kind == "owner_handover").expect("the log has the handover");
    assert!(handover.old_key == own.key && handover.new_key == bo.key && !handover.object.is_empty());
    assert!(!handover.reason.contains("owner alone"), "checked by the new owner, not the owner alone");
    assert!(log.iter().any(|l| l.kind == humanity_protocol::moderation::OWNER_ACCEPT && l.new_key == bo.key && l.reason.starts_with("checked ")), "and the new owner's signed acceptance");
    let leftovers: Vec<String> = std::fs::read_dir(held_dir_beside(d.folders())).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with('.')).collect();
    assert!(leftovers.is_empty(), "nothing of the handover is left: {leftovers:?}");
    let (grant, words) = begin_with(&st, &d, &own, action);
    assert!(grant.is_none() && words.contains("only this server's owner"), "the old owner cannot use it again: {words}");
}

/// A CHANGE NEVER TAKES A BODY BYTE, AND A HEADER IS TAKEN ONLY FOR THE BODY IT HEADS (format v4;
/// the second review, R1): only the owner sends new headers; a header followed by body bytes, a
/// whole part, and a header for another number of people are refused; a header made under another
/// key (its own commitment) is refused, so no device can swap what a copy holds.
///
/// Seen red 2026-11-20 with the commitment comparison taken out of `take_new_header`: "a header for
/// another key is refused" got 200.
#[test]
fn body_bytes_and_a_header_for_another_key_are_refused() {
    let d = crate::relay::handlers::held::tests::dirs("owner_nobody");
    let (st, _id, _ada, own, row) = one_held("owner_nobody", &d);
    let bo = bo_member(&st);
    let (grant, _) = begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key));
    let grant = grant.expect("a ready frame");
    let now = now_ms();
    let header = current_header_in(&st, row.id, 1, &grant, &own.key, d.folders(), now).unwrap();
    let new = rewrap_header(&kyber_of(&own), &part_context(&row.package, 1), &header, &[&card_of(&st, &bo)]).unwrap();
    let put = |who: &Person, bytes: &[u8]| new_header_in(&st, row.id, 1, &grant, &who.key, bytes, d.folders(), now).status().as_u16();
    assert_eq!(put(&bo, &new), 404, "only the owner sends new headers");
    let whole = std::fs::read(held_dir_beside(d.folders()).join(&row.package).join(part_file(1))).unwrap();
    let with_body = [&new[..], &whole[header.len()..]].concat();
    assert_eq!(put(&own, &with_body), 413, "a header followed by body bytes is refused");
    assert_eq!(put(&own, &whole), 413, "a whole part is refused");
    let two = rewrap_header(&kyber_of(&own), &part_context(&row.package, 1), &header, &[&card_of(&st, &bo), &card_of(&st, &own)]).unwrap();
    assert_eq!(put(&own, &two), 413, "a header for another number of people is refused");
    let other = seal_bytes(&[&card_of(&st, &bo)], &part_context(&row.package, 1), b"other bytes").unwrap();
    assert_eq!(put(&own, &other[..header_len(1)]), 400, "a header for another key is refused");
    assert_eq!(put(&own, &new), 200, "the honest header is taken");
    assert_eq!(new_header_in(&st, row.id, 1, "nope", &own.key, &new, d.folders(), now).status().as_u16(), 404, "no grant, nothing taken");
}

/// A change whose new headers are wrong changes NOTHING, the role and every original byte included:
/// a header that has not arrived; a GARBAGE SLOT for the new owner (the right commitment, so the
/// relay takes it) caught by the new owner's own check; a header wrapped for a third key in the new
/// owner's place, caught the same way; an acceptance whose digest is not the relay's; a failure
/// inside the swap. A stop part-way through the swap is undone when the relay starts.
///
/// Seen red 2026-11-20 with the digest comparison taken out of `accept_in`: the acceptance of
/// something else was taken (its `unwrap_err` found Ok, at "an acceptance of something else").
#[test]
fn a_change_with_one_bad_header_changes_nothing() {
    let d = crate::relay::handlers::held::tests::dirs("owner_badpart");
    let (st, _id, _ada, own, row) = one_held("owner_badpart", &d);
    let (bo, cal) = (bo_member(&st), person(47));
    st.db.register_name("Cal", &cal.key).unwrap();
    give_card(&st, &cal);
    let before = parts_on_disk(&st, &d);
    let held = held_dir_beside(d.folders());
    let unchanged = |why: &str| {
        assert!(st.db.is_owner(&own.key) && !st.db.is_owner(&bo.key), "{why}: the roles are as they were");
        assert!(st.db.held_items().unwrap().iter().all(|r| r.sealed_to == own.key), "{why}: the held items are as they were");
        assert_eq!(parts_on_disk(&st, &d), before, "{why}: every sealed byte is as it was");
    };
    let start = || begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key)).0.expect("a ready frame");

    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, Some(1));
    let why = finish(&st, &d, &own, &grant).unwrap_err().1;
    assert!(why.contains("part 1 has no new header") && why.contains("Nothing was changed"), "{why}");
    unchanged("a header missing");

    // A garbage slot for the new owner: the relay cannot open a slot, so it takes the header (its
    // commitment is right); the new owner's device does not.
    let garble: Tamper = &|p, h| {
        if p == 1 {
            h[held_seal::MAGIC.len() + 1 + held_seal::COMMITMENT + 40] ^= 1;
        }
    };
    rewrap_all_with(&st, &d, &own, &[&bo], &grant, None, Some(garble));
    finish(&st, &d, &own, &grant).unwrap();
    let why = digest_as(&st, &d, &bo, &grant).unwrap_err();
    assert!(why.contains("part 1"), "the new owner's device catches it: {why}");
    accept::decline(&st, &bo.key, &json!({ "grant": grant, "why": why }), d.folders());
    unchanged("a garbage slot");
    assert!(pending_for(&st, &grant, now_ms()).is_none() && !held.join(format!(".handover-{}", row.package)).exists(), "rolled back, its new parts removed");

    // Wrapped for a third key (Cal) in the new owner's place: the right count and commitment.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&cal], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    assert!(digest_as(&st, &d, &bo, &grant).unwrap_err().contains("does not open for your key"), "a header wrapped for a third key");
    accept::decline(&st, &bo.key, &json!({ "grant": grant, "why": "part 0" }), d.folders());
    unchanged("a header for a third key");

    // An acceptance whose digest is not the relay's (a device that did not really check): refused,
    // and the change rolled back.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    let why = send_accept(&st, &d, &bo, &grant, &"ab".repeat(32)).unwrap_err();
    assert!(why.contains("did not check out"), "{why}");
    unchanged("an acceptance of something else");
    assert!(pending_for(&st, &grant, now_ms()).is_none(), "rolled back");

    // A failure inside the swap: the owner role moved (by hand) after every check passed.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    st.db.with_conn(|c| c.execute("UPDATE user_roles SET role = 'mod' WHERE public_key = ?1", rusqlite::params![own.key])).unwrap();
    let why = accept_as(&st, &d, &bo, &grant).unwrap_err();
    assert!(why.contains("owner role changed"), "{why}");
    st.db.with_conn(|c| c.execute("UPDATE user_roles SET role = 'owner' WHERE public_key = ?1", rusqlite::params![own.key])).unwrap();
    unchanged("a failure inside the swap");
    assert!(!held.join(format!(".handover-{}", row.package)).exists(), "and its new parts removed");

    // A stop part-way through the swap, before the transaction: undone at the next start.
    let pkg = row.package.clone();
    std::fs::write(held.join(crate::relay::storage::owner::handover::MARKER), json!({ "object_id": "never-recorded", "packages": [pkg] }).to_string()).unwrap();
    std::fs::create_dir_all(held.join(format!(".handover-{pkg}"))).unwrap();
    std::fs::copy(held.join(&pkg).join(held_seal::RESTORE_PLAN_FILE), held.join(format!(".handover-{pkg}")).join(held_seal::RESTORE_PLAN_FILE)).unwrap();
    std::fs::rename(held.join(&pkg), held.join(format!(".pre-handover-{pkg}"))).unwrap();
    std::fs::rename(held.join(format!(".handover-{pkg}")), held.join(&pkg)).unwrap();
    assert!(st.db.owner_handover_recover_in(d.folders()), "the interrupted handover is found");
    unchanged("a stop before the transaction");
    assert!(!held.join(format!(".pre-handover-{pkg}")).exists() && !held.join(crate::relay::storage::owner::handover::MARKER).exists());

    // And the same handover carried through afterwards works.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    accept_as(&st, &d, &bo, &grant).unwrap().expect("carried out");
    assert!(st.db.is_owner(&bo.key));
}

/// AN ACCEPTANCE OF ONE ROUND NEVER CARRIES OUT ANOTHER (the second review, R4): a handover begun,
/// re-wrapped and checked, its acceptance signed but held back; cancelled; the SAME signed action
/// begun again, and the very same new headers sent again (so only the round tells the two apart).
/// The first round's acceptance is refused in the second round, and nothing changes; a round's own
/// acceptance carries it out.
///
/// Seen red 2026-11-20 with the round left out of core `acceptance_digest`'s text: the round-1
/// acceptance carried round 2 out (its `unwrap_err` found Ok).
#[test]
fn a_round_one_acceptance_is_refused_in_round_two() {
    let d = crate::relay::handlers::held::tests::dirs("owner_round");
    let (st, _id, _ada, own, _row) = one_held("owner_round", &d);
    let bo = bo_member(&st);
    let action = signed_handover(&st, &own, &bo.key);
    let grant = begin_with(&st, &d, &own, action.clone()).0.expect("round 1");
    let sent: std::cell::RefCell<std::collections::HashMap<usize, Vec<u8>>> = Default::default();
    let keep: Tamper = &|p, h| {
        sent.borrow_mut().insert(p, h.clone());
    };
    rewrap_all_with(&st, &d, &own, &[&bo], &grant, None, Some(keep));
    finish(&st, &d, &own, &grant).unwrap();
    let first = digest_as(&st, &d, &bo, &grant).unwrap();
    cancel_in(&st, &own.key, d.folders());
    assert!(pending_for(&st, &grant, now_ms()).is_none(), "cancelled");
    let again = begin_with(&st, &d, &own, action).0.expect("round 2, the same signed action");
    assert_eq!(again, grant, "the same grant");
    let same: Tamper = &|p, h| *h = sent.borrow()[&p].clone();
    rewrap_all_with(&st, &d, &own, &[&bo], &grant, None, Some(same));
    finish(&st, &d, &own, &grant).unwrap();
    let why = send_accept(&st, &d, &bo, &grant, &first).unwrap_err();
    assert!(why.contains("did not check out"), "a round-1 acceptance is refused in round 2: {why}");
    assert!(st.db.is_owner(&own.key) && !st.db.is_owner(&bo.key), "nothing changed");
    let grant = begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key)).0.expect("round 3");
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    accept_as(&st, &d, &bo, &grant).unwrap().expect("its own round's acceptance carries it out");
}

/// NOBODY READS A BODY DURING A CHANGE (the second review, R2 and R3, the critic's demos made
/// tests). The owner begins a change and fetches under its grant: no part is served, only headers
/// (begin, read, cancel used to read every copy unlogged). A successor-to-be, asked to check, is
/// served each new header alone (no body, no part under the change's grant), and declining leaves
/// them with nothing: never named, nothing logged, nothing exported.
///
/// Seen red 2026-11-20 with `granted_item`'s change branch put back as the branch had it: "the owner
/// is served no part under a change's grant" was served part 0. Seen red 2026-11-21 (scratchpad
/// ownfix4-mutate.js) with `rewrapped_header_in` serving 64 bytes past the header: "a header alone,
/// no body" (owner_tests.rs:568).
#[test]
fn nobody_reads_a_body_during_a_change() {
    let d = crate::relay::handlers::held::tests::dirs("owner_noread");
    let (st, _id, _ada, own, row) = one_held("owner_noread", &d);
    let bo = bo_member(&st);
    let (grant, _) = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key));
    let grant = grant.expect("a ready frame");
    for p in 0..row.parts {
        assert!(export_part_path(&st, row.id, p, &grant, &own.key, d.folders(), now_ms()).is_none(), "the owner is served no part under a change's grant");
        let h = current_header_in(&st, row.id, p, &grant, &own.key, d.folders(), now_ms()).unwrap();
        assert_eq!(h.len(), header_len(1), "a header alone");
    }
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    for p in 0..row.parts {
        let h = accept::rewrapped_header_in(&st, row.id, p, &grant, &bo.key, d.folders(), now_ms()).expect("the successor-to-be is served the new header");
        assert_eq!(h.len(), header_len(2), "a header alone, no body");
        assert!(export_part_path(&st, row.id, p, &grant, &bo.key, d.folders(), now_ms()).is_none(), "and no part");
    }
    accept::decline(&st, &bo.key, &json!({ "grant": grant, "why": "" }), d.folders());
    assert!(st.db.successor().is_none(), "never named");
    assert!(st.db.owner_log(20).unwrap().iter().all(|l| l.kind == "claim"), "nothing in the owner log");
    assert_eq!(st.db.held_counts().unwrap().exported, 0, "no export counted, and none was possible");
    assert_eq!(st.db.held_item(row.id).unwrap().unwrap().exported_count, 0);
}

/// THE OWNER ALONE CANNOT REPLACE A BODY (the second review, R1, the critic's demo made a test): on
/// a successor removal the owner is the only person who checks the change. Every way to put other
/// bytes in a copy is refused (body bytes, a header for another key), and when the owner's device
/// sends a header whose own slot is garbage and accepts it anyway (the residual hide-and-keep.md
/// names: the same as destroying the copies), every BODY survives byte for byte and the owner log
/// says the change was "checked by the owner alone".
///
/// Seen red 2026-11-20 with `owner_alone` answering false: the log line lacked "checked by the owner
/// alone". Seen red 2026-11-21 (scratchpad ownfix4-mutate.js) with `take_new_header` taking more
/// than a header (its length check `<` for `!=`): "right-sized garbage is refused"; and with its
/// commitment comparison taken out: "a header for other bytes is refused".
#[test]
fn the_owner_alone_cannot_replace_a_body() {
    let d = crate::relay::handlers::held::tests::dirs("owner_alone");
    let (st, _id, _ada, own, row) = one_held("owner_alone", &d);
    let bo = bo_member(&st);
    change(&st, &d, &own, "owner_successor", &bo, &[&own, &bo]).unwrap();
    let bodies_before = bodies(&st, &d);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor_remove", &bo.key)).0.expect("a ready frame");
    let now = now_ms();
    let header = current_header_in(&st, row.id, 1, &grant, &own.key, d.folders(), now).unwrap();
    let whole = std::fs::read(held_dir_beside(d.folders()).join(&row.package).join(part_file(1))).unwrap();
    let garbage_body = [&header[..header_len(1)], &vec![0x5a; whole.len() - header.len()][..]].concat();
    assert_eq!(new_header_in(&st, row.id, 1, &grant, &own.key, &garbage_body, d.folders(), now).status().as_u16(), 413, "right-sized garbage is refused");
    let fresh = seal_bytes(&[&card_of(&st, &own)], &part_context(&row.package, 1), &vec![0x5a; 64]).unwrap();
    assert_eq!(new_header_in(&st, row.id, 1, &grant, &own.key, &fresh[..header_len(1)], d.folders(), now).status().as_u16(), 400, "a header for other bytes is refused");
    let garble: Tamper = &|_, h| h[held_seal::MAGIC.len() + 1 + held_seal::COMMITMENT + 40] ^= 1;
    rewrap_all_with(&st, &d, &own, &[&own], &grant, None, Some(garble));
    finish(&st, &d, &own, &grant).unwrap();
    let p = pending_for(&st, &grant, now_ms()).unwrap();
    let hashes: Vec<(String, Vec<String>)> = st
        .db
        .held_items()
        .unwrap()
        .iter()
        .map(|r| (r.package.clone(), (0..r.parts as usize).map(|part| header_hash(&accept::rewrapped_header_in(&st, r.id, part as i64, &grant, &own.key, d.folders(), now_ms()).unwrap())).collect()))
        .collect();
    send_accept(&st, &d, &own, &grant, &acceptance_digest(&p.round, &hashes)).unwrap().expect("the owner's own acceptance carries it out");
    assert_eq!(bodies(&st, &d), bodies_before, "every body survives byte for byte");
    let log = st.db.owner_log(5).unwrap();
    assert!(log.iter().any(|l| l.kind == "owner_successor_remove" && l.reason.contains("checked by the owner alone")), "the log says who checked it: {log:?}");
}

/// A post held while a change waits for its acceptance fails it (its copy was never re-wrapped), the
/// set of held items being compared, not their number (the owner review's finding 9); a change
/// nobody accepts in time rolls back on the minute's pass; a stop after the change was recorded is
/// FINISHED at the next start (the set-aside original removed, the new copy kept).
///
/// Seen red 2026-11-20 with `expire_pending` doing nothing: "rolled back once its hour is up".
#[test]
fn a_change_rolls_back_in_time_and_a_recorded_one_is_finished() {
    let d = crate::relay::handlers::held::tests::dirs("owner_timing");
    let (st, _id, _ada, own, row) = one_held("owner_timing", &d);
    let bo = bo_member(&st);
    let held = held_dir_beside(d.folders());
    let start = || begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key)).0.expect("a ready frame");

    // Out of time.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    expire_pending(&st, d.folders());
    assert!(pending_for(&st, &grant, now_ms()).is_some(), "kept while within its hour");
    slot(&st).pending.as_mut().unwrap().checked_at = Some(now_ms() - ACCEPT_WINDOW_MS - 1);
    expire_pending(&st, d.folders());
    assert!(slot(&st).pending.is_none(), "rolled back once its hour is up");
    assert!(!held.join(format!(".handover-{}", row.package)).exists() && st.db.is_owner(&own.key));
    assert!(crate::relay::storage::owner::handover::STAGING_LIFETIME_MS > HANDOVER_WINDOW_MS + ACCEPT_WINDOW_MS, "the sweep never takes a live change's copies");

    // A post held while it waits: compared as a set.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    slot(&st).pending.as_mut().unwrap().ids = vec![row.id + 1000];
    let why = accept_as(&st, &d, &bo, &grant).unwrap_err();
    assert!(why.contains("no new header") || why.contains("held while it ran"), "{why}");
    slot(&st).pending.take();
    let p = PendingHandover { change: OwnerChange { kind: ChangeKind::Handover, object_id: "x".into(), owner: own.key.clone(), target: bo.key.clone(), recipients: vec![(bo.key.clone(), 1)], reason: "r".into(), signed_at: 0 }, round: "r".into(), object_json: String::new(), started_at: 0, ids: vec![row.id + 1000], checked_at: None, building: false, accepted: Vec::new() };
    assert!(covered_rows(&st, &p).unwrap_err().contains("held while it ran"), "an item the change does not cover fails it");
    let rows = st.db.held_items().unwrap();
    assert!(st.db.owner_change_in(&p.change, &p.round, &rows, &p.ids, "", &[], now_ms(), d.folders()).unwrap_err().contains("held while it ran"), "the swap compares the set too");
    st.db.owner_handover_clear_staging_in(d.folders());

    // Carried out, then a stop is simulated after the transaction: the marker and a set-aside
    // original remain. The change is in the log, so the start FINISHES it.
    let grant = start();
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    accept_as(&st, &d, &bo, &grant).unwrap().expect("carried out");
    let pkg = row.package.clone();
    let now_parts = parts_on_disk(&st, &d);
    std::fs::create_dir_all(held.join(format!(".pre-handover-{pkg}"))).unwrap();
    std::fs::write(held.join(format!(".pre-handover-{pkg}")).join(part_file(0)), b"the old original").unwrap();
    std::fs::write(held.join(crate::relay::storage::owner::handover::MARKER), json!({ "object_id": grant, "packages": [pkg] }).to_string()).unwrap();
    assert!(st.db.owner_handover_recover_in(d.folders()), "the interrupted change is found");
    assert!(!held.join(format!(".pre-handover-{pkg}")).exists(), "finished: the set-aside original is removed");
    assert_eq!(parts_on_disk(&st, &d), now_parts, "and the new copy kept");
    assert!(!held.join(crate::relay::storage::owner::handover::MARKER).exists());
    assert!(st.db.is_owner(&bo.key));
}

/// ROLLBACK AND CARRY-OUT EXCLUDE EACH OTHER (the second review, R4): with a hook stepping into
/// `accept_in`, a cancel, a decline, the expiry tick and a new begin each arrive while the last
/// acceptance is being carried out, and each is refused with words while the change commits:
/// exactly one outcome, nothing lost. A cancel that arrives after the acceptance was checked but
/// before it was recorded wins instead: the acceptance is refused and nothing changes. And the held
/// folder's sweep, run from another thread while the swap's folders are renamed, waits for it.
///
/// Seen red 2026-11-20 with the pending change left in its slot during the carry-out (as the branch
/// had it): the cancel arriving then rolled it back and cleared its new parts under the carry-out,
/// which then failed ("the change is carried out" panicked); and with the held folder's lock left
/// out of the sweep: "the sweep waited for the swap" failed.
#[test]
fn rollback_and_carry_out_exclude_each_other() {
    use accept::{Step, HOOK};
    let d = crate::relay::handlers::held::tests::dirs("owner_race");
    let (st, _id, _ada, own, _row) = one_held("owner_race", &d);
    let bo = bo_member(&st);
    let ready = |st: &RelayState| {
        let grant = begin_with(st, &d, &own, signed_handover(st, &own, &bo.key)).0.expect("a ready frame");
        rewrap_all(st, &d, &own, &[&bo], &grant, None);
        finish(st, &d, &own, &grant).unwrap();
        grant
    };
    let set_hook = |at: Step, f: Box<dyn FnMut()>| {
        let mut f = f;
        HOOK.with(|h| *h.borrow_mut() = Some(Box::new(move |s| if s == at { f() })));
    };
    let clear_hook = || HOOK.with(|h| *h.borrow_mut() = None);

    // A cancel during the carry-out: refused, and the change commits.
    let grant = ready(&st);
    let digest = digest_as(&st, &d, &bo, &grant).unwrap();
    let st2 = st.clone();
    let (own_key, bo_key, grant2) = (own.key.clone(), bo.key.clone(), grant.clone());
    let told: std::rc::Rc<std::cell::RefCell<Vec<RelayMessage>>> = Default::default();
    let told2 = told.clone();
    let folders_owned = crate::relay::storage::files::OwnedFolders::of(d.folders());
    set_hook(
        Step::Carrying,
        Box::new(move || {
            let mut rx = st2.broadcast_tx.subscribe();
            let f = folders_owned.get();
            cancel_in(&st2, &own_key, f);
            accept::decline(&st2, &bo_key, &json!({ "grant": grant2, "why": "" }), f);
            expire_pending_at(&st2, f, now_ms() + 10 * ACCEPT_WINDOW_MS);
            told2.borrow_mut().extend(drain(&mut rx));
        }),
    );
    let done = send_accept(&st, &d, &bo, &grant, &digest);
    clear_hook();
    assert!(done.unwrap().is_some(), "the change is carried out");
    let all = told.borrow().clone();
    assert!(replied(&all, &own.key).contains("Too late to cancel"), "the owner is told it is too late: {}", replied(&all, &own.key));
    assert!(replied(&all, &bo.key).contains("Too late to decline"), "the decliner too: {}", replied(&all, &bo.key));
    assert!(!replied(&all, &own.key).contains("Nothing was changed"), "nobody is told it rolled back");
    assert!(st.db.is_owner(&bo.key), "exactly one outcome: carried out");
    assert!(slot(&st).carrying.is_none() && slot(&st).pending.is_none());
    let bo_kp = kyber_of(&bo);
    assert!(parts_on_disk(&st, &d).iter().all(|(pkg, p, s)| open_bytes(&bo_kp, &part_context(pkg, *p), s).is_ok()), "nothing lost");

    // Hand it back for the next case (the new owner hands the role to the old one).
    let d2 = crate::relay::handlers::held::tests::dirs("owner_race2");
    let (st, _id, _ada, own, row) = one_held("owner_race2", &d2);
    let bo = bo_member(&st);
    let d = d2;
    let ready = |st: &RelayState| {
        let grant = begin_with(st, &d, &own, signed_handover(st, &own, &bo.key)).0.expect("a ready frame");
        rewrap_all(st, &d, &own, &[&bo], &grant, None);
        finish(st, &d, &own, &grant).unwrap();
        grant
    };
    // A cancel after the check but before the acceptance is recorded: the cancel wins.
    let grant = ready(&st);
    let digest = digest_as(&st, &d, &bo, &grant).unwrap();
    let st2 = st.clone();
    let own_key = own.key.clone();
    let folders_owned = crate::relay::storage::files::OwnedFolders::of(d.folders());
    set_hook(Step::Checked, Box::new(move || cancel_in(&st2, &own_key, folders_owned.get())));
    let why = send_accept(&st, &d, &bo, &grant, &digest).unwrap_err();
    clear_hook();
    assert!(why.contains("cancelled or begun again"), "{why}");
    assert!(st.db.is_owner(&own.key) && !st.db.is_owner(&bo.key), "exactly one outcome: rolled back");
    assert!(!held_dir_beside(d.folders()).join(format!(".handover-{}", row.package)).exists(), "its new parts removed");

    // A begin during the carry-out is refused; the sweep from another thread waits for the swap.
    let grant = ready(&st);
    let digest = digest_as(&st, &d, &bo, &grant).unwrap();
    let st2 = st.clone();
    let (own2, act2) = (Person { seed: own.seed.clone(), key: own.key.clone() }, signed_handover(&st, &own, &bo.key));
    let folders_owned = crate::relay::storage::files::OwnedFolders::of(d.folders());
    let begun: std::rc::Rc<std::cell::RefCell<String>> = Default::default();
    let begun2 = begun.clone();
    set_hook(
        Step::Carrying,
        Box::new(move || {
            let mut rx = st2.broadcast_tx.subscribe();
            begin_in(&st2, &own2.key, &json!({ "type": "owner_change_begin", "action": act2.clone() }), folders_owned.get(), now_ms());
            *begun2.borrow_mut() = replied(&drain(&mut rx), &own2.key);
        }),
    );
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let waited = std::thread::scope(|s| {
        let st3 = st.clone();
        let f = d.folders();
        let sweep = s.spawn(move || {
            go_rx.recv().unwrap();
            let r = st3.db.held_sweep_strays_in(f, std::time::SystemTime::now() + std::time::Duration::from_secs(3600));
            let _ = done_tx.send(()); // the hook may be gone by then: the sweep waited past it
            r
        });
        let finished_early: std::rc::Rc<std::cell::Cell<bool>> = Default::default();
        let early = finished_early.clone();
        crate::relay::storage::owner::handover::SWAP_HOOK.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || {
                go_tx.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(300));
                early.set(done_rx.try_recv().is_ok());
            }))
        });
        let done = send_accept(&st, &d, &bo, &grant, &digest);
        crate::relay::storage::owner::handover::SWAP_HOOK.with(|h| *h.borrow_mut() = None);
        assert!(done.unwrap().is_some(), "carried out");
        sweep.join().unwrap().unwrap();
        !finished_early.get()
    });
    clear_hook();
    assert!(begun.borrow().contains("being carried out right now"), "a begin during the carry-out is refused: {}", begun.borrow());
    assert!(waited, "the sweep waited for the swap");
    assert!(st.db.is_owner(&bo.key));
    let bo_kp = kyber_of(&bo);
    assert!(parts_on_disk(&st, &d).iter().all(|(pkg, p, s)| open_bytes(&bo_kp, &part_context(pkg, *p), s).is_ok()), "nothing lost");
}

/// A PANIC INSIDE THE CARRY-OUT, OR IN ONE PASS OF THE OWNER'S MINUTE, LEAVES NOTHING STUCK (the
/// fifth review's L1). The swap hook panics after every folder is swapped and before the
/// transaction. The round is no longer named as carried out; the carry-out answers like any failure;
/// the held folder is put right at once (undone: the change was never recorded), no marker left for
/// the next start; so a hide seals at once instead of waiting for a change that will never end, and
/// a begin goes ahead. Then a forced panic in one pass of the owner's minute ends that pass alone,
/// and the next pass runs (the loop was one task, so one panic ended it for good).
///
/// Seen red 2026-11-21 (scratchpad ownfix8-red-m1.log) with round 5's carry-out put back (no
/// `catch_unwind`, `CarryingNow` clearing nothing): "the round is no longer named as carried out"
/// failed; and (scratchpad ownfix8-red-m2.log) with `minute_pass` calling `minute` inline: "the
/// panicking pass is caught and ends alone" failed, `Err(Any { .. })` (the panic escaped the pass).
#[test]
fn a_panic_in_the_carry_out_or_the_minute_leaves_nothing_stuck() {
    let d = crate::relay::handlers::held::tests::dirs("owner_panic");
    let (st, _first, ada, own, _row) = one_held("owner_panic", &d);
    let bo = bo_member(&st);
    let grant = begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key)).0.expect("a ready frame");
    rewrap_all(&st, &d, &own, &[&bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap();
    let before = parts_on_disk(&st, &d);
    let digest = digest_as(&st, &d, &bo, &grant).unwrap();
    crate::relay::storage::owner::handover::SWAP_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(|| panic!("a forced panic inside the carry-out"))));
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| send_accept(&st, &d, &bo, &grant, &digest)));
    crate::relay::storage::owner::handover::SWAP_HOOK.with(|h| *h.borrow_mut() = None);
    assert!(slot(&st).carrying.is_none(), "the round is no longer named as carried out");
    let done = done.expect("the carry-out answers, it does not panic out");
    assert!(done.as_ref().is_err_and(|w| w.contains("stopped part-way")), "answered like any failure: {done:?}");
    assert!(st.db.is_owner(&own.key) && !st.db.is_owner(&bo.key), "nothing changed");
    assert_eq!(parts_on_disk(&st, &d), before, "every copy put back as it was");
    assert!(!held_dir_beside(d.folders()).join(crate::relay::storage::owner::handover::MARKER).exists(), "no marker left for the next start");
    let (report, _) = second_report(&st, &d, 95);
    let all = decide(&st, &d, &ada, report, "hide_and_keep");
    assert!(!told(&all, &ada.key).contains("WAITS"), "a hide seals at once: {}", told(&all, &ada.key));
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 0, "nothing waits");
    assert!(begin_with(&st, &d, &own, signed_handover(&st, &own, &bo.key)).0.is_some(), "a begin goes ahead");

    PANIC_NEXT_MINUTE.store(true, std::sync::atomic::Ordering::SeqCst);
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| block(minute_pass(st.clone(), crate::relay::storage::files::OwnedFolders::of(d.folders())))));
    assert!(first.as_ref().is_ok_and(|ran| !ran), "the panicking pass is caught and ends alone: {first:?}");
    assert!(block(minute_pass(st.clone(), crate::relay::storage::files::OwnedFolders::of(d.folders()))), "the next pass runs");
}

// ── The successor (the operator, 2026-11-20: "Might as well, just in case.") ──

/// Carry a signed owner action through, as the owner's device and each recipient's do: begin,
/// re-wrap every header for `to`, finish, then each of `to` checks and accepts. What the owner was
/// told when it did not go through.
fn change(st: &RelayState, d: &Dirs, own: &Person, action: &str, target: &Person, to: &[&Person]) -> Result<(), String> {
    let (grant, words) = begin_with(st, d, own, signed_owner(st, own, action, &target.key));
    let grant = grant.ok_or(words)?;
    rewrap_all(st, d, own, to, &grant, None);
    finish(st, d, own, &grant).map_err(|(_, why)| why)?;
    let mut done = None;
    for who in to {
        done = accept_as(st, d, who, &grant)?;
    }
    done.map(|_| ()).ok_or_else(|| "the last acceptance did not carry it out".to_string())
}

/// Whether every held part opens for each of `yes` and for none of `no`, as the bytes it held.
fn opens_for(st: &RelayState, d: &Dirs, yes: &[&Person], no: &[&Person]) -> bool {
    parts_on_disk(st, d).iter().all(|(pkg, p, sealed)| {
        let ctx = part_context(pkg, *p);
        let first = open_bytes(&kyber_of(yes[0]), &ctx, sealed).ok();
        first.is_some()
            && yes.iter().all(|w| open_bytes(&kyber_of(w), &ctx, sealed).ok() == first)
            && no.iter().all(|w| open_bytes(&kyber_of(w), &ctx, sealed).is_err())
    })
}

/// The owner names a successor with a checked key card: every held copy is re-wrapped for both
/// cards (each opens with the owner's test seed and the successor's, and no other), and a copy held
/// after is sealed to both from the start; removing the successor re-wraps for the owner alone. The
/// successor has no power while the owner holds the role (refused export); every admin sees who
/// the successor is and the log of it. A successor without a key card, or the owner, cannot be
/// named; only the successor named can be removed.
///
/// Seen red 2026-11-20 with `owner_card` leaving the successor out ("a copy held after the naming
/// opens for both": the new package opened for the owner alone); and with `granted_item`'s caller
/// check taken out: "the successor with the owner's grant gets nothing".
#[test]
fn a_successor_is_sealed_to_shown_to_admins_and_has_no_powers() {
    let d = crate::relay::handlers::held::tests::dirs("owner_successor");
    let (st, _id, ada, own, row) = one_held("owner_successor", &d);
    let (bo, stranger) = (bo_member(&st), person(46));
    st.db.register_name("Stranger", &stranger.key).unwrap();
    assert!(opens_for(&st, &d, &[&own], &[&bo]), "before: the owner alone");

    let why = change(&st, &d, &own, "owner_successor", &stranger, &[&own, &stranger]).unwrap_err();
    assert!(why.contains("no key card here that checks out"), "{why}");
    let why = change(&st, &d, &own, "owner_successor", &own, &[&own]).unwrap_err();
    assert!(why.contains("cannot be their own successor"), "{why}");
    let why = change(&st, &d, &own, "owner_successor_remove", &bo, &[&own]).unwrap_err();
    assert!(why.contains("not the successor"), "{why}");

    change(&st, &d, &own, "owner_successor", &bo, &[&own, &bo]).unwrap();
    assert_eq!(st.db.successor().as_deref(), Some(bo.key.as_str()));
    assert!(opens_for(&st, &d, &[&own, &bo], &[&stranger]), "re-wrapped on naming: the owner and the successor, nobody else");
    assert!(st.db.held_items().unwrap().iter().all(|r| r.sealed_to == own.key && r.also_sealed_to == bo.key));

    // No powers while the owner holds the role.
    let all = act(&st, &d, &Person { seed: bo.seed.clone(), key: bo.key.clone() }, signed(&st, &bo, "held_export", "held", row.id, 0));
    assert!(told(&all, &bo.key).contains("only this server's owner"), "the successor is refused export: {}", told(&all, &bo.key));
    // Nor can they fetch a part under the owner's own export grant, though every part is sealed to
    // them too (the owner review's finding 1): the grant is the owner's, proven by the owner's key.
    let export = signed(&st, &own, "held_export", "held", row.id, 0);
    act(&st, &d, &own, export.clone());
    let grant = {
        let sub: crate::relay::api_v2_objects::SignedObjectSubmission = serde_json::from_value(export).unwrap();
        sub.to_object().unwrap().object_id().unwrap().to_hex()
    };
    assert!(export_part_path(&st, row.id, 0, &grant, &bo.key, d.folders(), now_ms()).is_none(), "the successor with the owner's grant gets nothing");
    assert!(export_part_path(&st, row.id, 0, &grant, &own.key, d.folders(), now_ms()).is_some(), "the owner does");

    // Shown to every admin.
    let mut rx = st.broadcast_tx.subscribe();
    crate::relay::handlers::held::handle_list(&st, &ada.key);
    let list = drain(&mut rx).into_iter().find_map(|m| match m {
        RelayMessage::HeldItems { ref to, .. } if *to == ada.key => Some(serde_json::to_value(&m).unwrap()),
        _ => None,
    }).expect("the admin's list");
    assert_eq!(list["successor"]["name"], "Bo", "an admin sees who the successor is: {list}");
    assert!(list["owner_log"].as_array().unwrap().iter().any(|l| l["kind"] == "owner_successor" && l["new_name"] == "Bo"), "and the log of it");
    assert_eq!(list["items"][0]["sealed_to_successor"], true);

    change(&st, &d, &own, "owner_successor_remove", &bo, &[&own]).unwrap();
    assert!(st.db.successor().is_none());
    assert!(opens_for(&st, &d, &[&own], &[&bo, &stranger]), "re-wrapped on removal: the owner alone");

    // A copy held while a successor is named is sealed to both from the start.
    change(&st, &d, &own, "owner_successor", &bo, &[&own, &bo]).unwrap();
    let d2 = crate::relay::handlers::held::tests::dirs("owner_successor_new");
    let (st2, id2, _rae, _tom, ada2) = reported_post("owner_successor_new", &d2);
    make_owner(&st2, &own, true);
    let bo2 = bo_member(&st2);
    change(&st2, &d2, &own, "owner_successor", &bo2, &[&own, &bo2]).unwrap();
    decide(&st2, &d2, &ada2, id2, "hide_and_keep");
    assert_eq!(st2.db.held_items().unwrap().len(), 1);
    assert!(opens_for(&st2, &d2, &[&own, &bo2], &[&stranger]), "a copy held after the naming opens for both");

    // The owner role gone (by hand) and claimed by the successor: the copies stay as they are and open
    // for them; a handover clears the successor.
    st2.db.with_conn(|c| c.execute("DELETE FROM user_roles WHERE role = 'owner'", [])).unwrap();
    assert!(st2.db.claim_owner(&bo2.key, 1).unwrap());
    assert!(st2.db.successor().is_none(), "a claim clears the successor");
    let all = act(&st2, &d2, &bo2, signed(&st2, &bo2, "held_export", "held", st2.db.held_items().unwrap()[0].id, 0));
    assert!(told(&all, &bo2.key).contains("Export of held item"), "the successor, now the owner, may export: {}", told(&all, &bo2.key));
}

// ── The owner's protection ──

/// No admin can remove, demote, ban, mute, kick, warn or game-ban the owner, by any path (the
/// profile buttons, the chat commands and Server Settings' member actions, the signed actions,
/// the game ban, the account erase), and nobody grants `owner` but a claim or a handover: the
/// storage refuses it underneath every path, and an owner's linked device is an admin (BUG-345).
///
/// Seen red 2026-11-20 with `refuse_if_owner` left out of `mod_action_refusal`: "kick: None" (an
/// admin's kick of the owner was allowed).
#[test]
fn nobody_can_remove_demote_ban_mute_kick_or_game_ban_the_owner() {
    let st = Arc::new(RelayState::new(Storage::open_temp_dir("owner_protect")));
    let (own, adm) = (person(43), person(44));
    st.db.register_name("Owen", &own.key).unwrap();
    st.db.register_name("Adam", &adm.key).unwrap();
    st.db.claim_owner(&own.key, 0).unwrap();
    st.db.set_role(&adm.key, "admin").unwrap();

    for action in ["kick", "ban", "mute", "warn", "mod", "unmod", "verify", "unverify"] {
        for (key, name) in [(own.key.as_str(), ""), ("", "Owen")] {
            let why = crate::relay::handlers::msg_handlers::mod_action_refusal(&st, &adm.key, action, key, name);
            assert!(why.as_deref().is_some_and(|w| w.starts_with("Nobody can")), "{action}: {why:?}");
        }
    }
    for cmd in ["/kick", "/ban", "/mute", "/mod", "/unmod"] {
        let said = block(crate::relay::handlers::broadcast::handle_mod_command(&st, cmd, "admin", "Owen", &adm.key));
        assert!(said.starts_with("Nobody can") && crate::relay::admin::members::is_refusal(&said), "{cmd}: {said}");
    }
    for action in ["ban", "mute", "grant_role", "revoke_role"] {
        use humanity_protocol::moderation::{build_mod_action_v1, ModAction};
        let role = if action.ends_with("_role") { "member" } else { "" };
        let a = ModAction { action, target: &own.key, target_kind: "identity", reason: "trying", rule: "", expires_at: 0, role };
        let o = build_mod_action_v1(&adm.seed, &st.db.server_did().unwrap(), &a, 1, &[]).unwrap();
        let out = st.db.apply_mod_action(&o).unwrap();
        assert!(format!("{out:?}").contains("owner"), "signed {action}: {out:?}");
    }
    assert!(is_owner_protected(&st.db.game_ban(&own.key, None, "trying", &adm.key).unwrap_err()), "game ban");
    assert!(is_owner_protected(&st.db.ban_user(&own.key, "Owen").unwrap_err()), "ban_user");
    assert!(is_owner_protected(&st.db.mute_user(&own.key, "Owen").unwrap_err()), "mute_user");
    assert!(st.db.set_role(&own.key, "admin").is_err_and(|e| is_owner_protected(&e)), "set_role demotes the owner");
    assert!(st.db.set_role(&adm.key, "owner").is_err_and(|e| is_owner_protected(&e)), "set_role grants the owner role");
    assert!(!st.db.claim_owner(&adm.key, 0).unwrap(), "a second claim");
    use crate::relay::handlers::account_erase::{admin_erase_check, Refused};
    assert_eq!(admin_erase_check(&st, &adm.key, &own.key, "Owen"), Err(Refused::TheOwner), "an admin's erase");
    assert!(Refused::TheOwner.words().contains("only the owner can hand it"));

    let code = st.db.create_link_code("Owen", &own.key).unwrap();
    st.db.redeem_link_code(&code, "owen_laptop_key").unwrap();
    assert_eq!(st.db.get_role("owen_laptop_key").unwrap(), "admin", "the owner's linked device is an admin");
    assert_eq!(st.db.owner_keys().unwrap(), [own.key.clone()], "and the owner is still one key");
    assert!(st.db.is_owner(&own.key) && !st.db.is_banned(&own.key).unwrap() && st.db.get_role(&own.key).unwrap() == "owner");
}

/// AN EARLIER ACCEPTANCE IS CHECKED AGAIN AT THE CARRY-OUT (the third review's S1, its sketch
/// scratchpad critic3-relay/finding1_late_build_test.rs made real). A late write lands after Bo
/// checked and accepted: a new header for part 0 whose successor slot is garbage (the same
/// commitment and count, so it is taken), and the new part rebuilt from it. In production that write
/// came from a second `owner_change_finish` on another socket of the owner's key, still building
/// when phase 2 began (now refused: `a_second_finish_and_headers_are_refused_while_one_builds`); here
/// the storage functions stand in for a write by any path. The owner then accepts LAST, over what is
/// staged now. The carry-out works out the digest of what it would carry out, under the held-folder
/// lock, and Bo's recorded digest is not it: refused, nothing changed.
///
/// Seen red 2026-11-21 (before `owner_change_in` compared the recorded acceptances): "carried out
/// over headers Bo never checked: Ok(Some(..))" (Bo became the successor with part 0 no longer
/// opening for him).
#[test]
fn an_earlier_acceptance_is_checked_again_at_the_carry_out() {
    use crate::relay::storage::owner::handover::{build_new_parts, new_header, take_new_header};
    let d = crate::relay::handlers::held::tests::dirs("owner_latebuild");
    let (st, _id, _ada, own, row) = one_held("owner_latebuild", &d);
    let bo = bo_member(&st);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    // Still phase 1: the owner keeps back a header for part 0 whose SUCCESSOR slot is garbage.
    let now = now_ms();
    let cur = current_header_in(&st, row.id, 0, &grant, &own.key, d.folders(), now).unwrap();
    let (own_k, bo_k) = (card_of(&st, &own), card_of(&st, &bo));
    let mut bad = rewrap_header(&kyber_of(&own), &part_context(&row.package, 0), &cur, &[own_k.as_slice(), bo_k.as_slice()]).unwrap();
    bad[held_seal::MAGIC.len() + 1 + held_seal::COMMITMENT + held_seal::SLOT + 40] ^= 1; // slot 1 = Bo
    finish(&st, &d, &own, &grant).unwrap(); // phase 2 begins
    assert!(accept_as(&st, &d, &bo, &grant).unwrap().is_none(), "Bo accepted, the owner still to accept");
    let before = parts_on_disk(&st, &d);
    // The late write lands, and the new part 0 is rebuilt from it (part 1 then fails: its header was
    // used; that error changes nothing else).
    take_new_header(&row, d.folders(), 0, 2, &bad).unwrap();
    let _ = build_new_parts(&st.db.held_items().unwrap(), 2, d.folders());
    assert_eq!(new_header(&row, d.folders(), 0).unwrap(), bad, "the staged part 0 now heads with the bad header");
    // The owner accepts LAST, over what is staged now.
    let p = pending_for(&st, &grant, now_ms()).unwrap();
    let hashes: Vec<(String, Vec<String>)> = st
        .db
        .held_items()
        .unwrap()
        .iter()
        .map(|r| (r.package.clone(), (0..r.parts as usize).map(|part| header_hash(&accept::rewrapped_header_in(&st, r.id, part as i64, &grant, &own.key, d.folders(), now_ms()).unwrap())).collect()))
        .collect();
    let carried = send_accept(&st, &d, &own, &grant, &acceptance_digest(&p.round, &hashes));
    assert!(carried.is_err(), "carried out over headers Bo never checked: {carried:?}");
    let why = carried.unwrap_err();
    assert!(why.contains("not exactly what everyone who accepted checked") && why.contains("Nothing was changed"), "{why}");
    assert!(st.db.successor().is_none(), "Bo is not the successor");
    assert_eq!(parts_on_disk(&st, &d), before, "every held copy is as it was");
}

/// A SECOND FINISH AND EVERY HEADER ROUTE ARE REFUSED WHILE A FINISH BUILDS (the third review's S1):
/// the slot is marked building under its lock before the finish checks anything, so a second finish
/// on another socket, a header sent, and a header read are each refused until it ends; once it ends
/// (here: put back as it was), headers are taken again, and a finish that fails clears the mark.
///
/// Seen red 2026-11-21 with `finish_in` not setting `building` (the round-3 code): "a second finish
/// is refused while one builds" got Ok(()). CORRECTED 2026-11-21 (the fourth review's finding 7):
/// this test sets `building` BEFORE the header is sent, so `rewrapping`'s outer check refuses it
/// (404) and the re-check under the lock is never reached here; the round-4 note that this test saw
/// that re-check red was wrong. `a_header_is_refused_when_a_finish_starts_between_its_check_and_the_lock`
/// reaches it.
#[test]
fn a_second_finish_and_headers_are_refused_while_one_builds() {
    let d = crate::relay::handlers::held::tests::dirs("owner_building");
    let (st, _id, _ada, own, row) = one_held("owner_building", &d);
    let bo = bo_member(&st);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    let now = now_ms();
    let cur = current_header_in(&st, row.id, 0, &grant, &own.key, d.folders(), now).unwrap();
    let (own_k, bo_k) = (card_of(&st, &own), card_of(&st, &bo));
    let again = rewrap_header(&kyber_of(&own), &part_context(&row.package, 0), &cur, &[own_k.as_slice(), bo_k.as_slice()]).unwrap();
    // A finish is building (the mark as finish_in sets it under the lock).
    slot(&st).pending.as_mut().unwrap().building = true;
    let second = finish(&st, &d, &own, &grant);
    assert!(second.as_ref().is_err_and(|(_, why)| why.contains("being checked right now")), "a second finish is refused while one builds: {second:?}");
    assert!(slot(&st).pending.as_ref().unwrap().building, "and the first one's mark stays");
    assert_eq!(new_header_in(&st, row.id, 0, &grant, &own.key, &again, d.folders(), now).status().as_u16(), 404, "a header sent while it builds is refused");
    assert!(current_header_in(&st, row.id, 0, &grant, &own.key, d.folders(), now).is_none(), "a header read while it builds is refused");
    slot(&st).pending.as_mut().unwrap().building = false;
    assert_eq!(new_header_in(&st, row.id, 0, &grant, &own.key, &again, d.folders(), now).status().as_u16(), 200, "taken again once it ended");
    // A finish that fails (a part missing) clears its own mark.
    std::fs::remove_file(crate::relay::storage::owner::handover::staging_dir(d.folders(), &row.package).join(crate::relay::storage::owner::handover::header_file(1))).unwrap();
    assert!(finish(&st, &d, &own, &grant).is_err());
    assert!(!slot(&st).pending.as_ref().unwrap().building, "a failed finish clears the mark");
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    finish(&st, &d, &own, &grant).expect("the finish goes through once every header is in");
    assert!(accept_as(&st, &d, &bo, &grant).unwrap().is_none());
    assert!(accept_as(&st, &d, &own, &grant).unwrap().is_some(), "carried out: every acceptance is of exactly what was built");
}

/// A second post by Tom (`person(2)`), at `stamp`, linking a picture of its own, reported by Rae
/// (`person(1)`) as one_held's was: a REAL post and report for a hide during a change. The report id.
fn second_report(st: &Arc<RelayState>, d: &Dirs, stamp: i64) -> (i64, String) {
    use crate::relay::handlers::held::tests::TS;
    // A reporter of its own each time, so no rule about one person reporting twice gets in the way.
    let (rae, tom) = (person(60 + (stamp % 50) as u8), person(2));
    st.db.register_name(&format!("R{stamp}"), &rae.key).unwrap();
    let name = format!("late_{stamp}.png");
    std::fs::write(d.uploads.join(&name), format!("HELD-LATE {stamp}")).unwrap();
    st.db.record_upload(&tom.key, &name, 20, 0, &name, 20).unwrap();
    let content = format!("HELD-LATE look /uploads/{name}");
    st.db
        .with_conn(|c| {
            c.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', ?1, 'Tom', ?2, ?3, '{}', 'global')",
                rusqlite::params![tom.key, content, stamp],
            )
        })
        .unwrap();
    let evidence = json!([{ "kind": "post", "from": tom.key, "timestamp": stamp }]);
    let sig = humanity_protocol::pq_crypto::build_report_sig(&rae.seed, &rae.key, &tom.key, "child_danger", &evidence.to_string(), TS + stamp as u64);
    let v = json!({ "type": "report_v2", "target": tom.key, "context": "post", "reason": "child_danger", "note": "", "evidence": evidence, "ts": TS + stamp as u64, "sig": sig });
    let mut rx = st.broadcast_tx.subscribe();
    block(crate::relay::handlers::reports::handle(st, &rae.key, "report_v2", &v, &v.to_string()));
    let id = drain(&mut rx)
        .into_iter()
        .find_map(|m| match m {
            RelayMessage::ReportReceived { to, id } if to == rae.key => Some(id),
            _ => None,
        })
        .expect("the second report is received");
    (id, name)
}

/// A HIDE DURING AN OWNER CHANGE WAITS FOR ITS SEALED COPY, AND NEVER ROLLS THE CHANGE BACK (the
/// third review's S3). With the owner naming Bo successor in phase 2 (every new header in, Bo still
/// to check), an admin hides a REAL reported post: it leaves every view at once (the Delete goes out,
/// its picture is in no served folder, its row is gone), no held row is added (so the change's set
/// of held items is untouched), the moderator is told in words that the sealed copy waits, and the
/// copy waits in the held folder. The change is then carried out, and the waiting copy is sealed
/// right after to the recipients THEN: the owner and Bo, opening for both. Rolled back instead (a
/// cancel), it is sealed to the owner alone.
///
/// Seen red 2026-11-21 (run by mutation, scratchpad ownfix5-rust-mutants.js) with `SealingNow::start`
/// ignoring a change in flight, so the hide sealed at once as in round 3: "the moderator is told the
/// copy waits" failed (the relay said the post was kept sealed to the owner's key card).
#[test]
fn a_hide_during_a_change_waits_and_is_sealed_to_the_recipients_then() {
    let d = crate::relay::handlers::held::tests::dirs("owner_waithide");
    let (st, _first, ada, own, _row) = one_held("owner_waithide", &d);
    let bo = bo_member(&st);
    let (report, pic) = second_report(&st, &d, 77);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    finish(&st, &d, &own, &grant).unwrap(); // phase 2: Bo still to check

    let all = decide(&st, &d, &ada, report, "hide_and_keep");
    let words = told(&all, &ada.key);
    assert!(words.contains("sealed copy WAITS for the owner change"), "the moderator is told the copy waits: {words}");
    assert!(all.iter().any(|m| matches!(m, RelayMessage::Delete { timestamp: 77, .. })), "the post leaves every view at once");
    assert!(d.folders().find(&pic).is_none(), "its picture is in no served folder");
    assert!(st.db.post_for_report(&person(2).key, 77).unwrap().is_none(), "its row is gone");
    assert_eq!(st.db.held_items().unwrap().len(), 1, "no held row is added while the change runs");
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 1, "the copy waits");

    assert!(accept_as(&st, &d, &bo, &grant).unwrap().is_none(), "Bo checks and accepts: the hide rolled nothing back");
    assert!(accept_as(&st, &d, &own, &grant).unwrap().is_some(), "the change is carried out");
    assert_eq!(st.db.successor().as_deref(), Some(bo.key.as_str()));
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 0, "sealed right after the change");
    let rows = st.db.held_items().unwrap();
    let late = rows.iter().find(|r| r.report_id == report).expect("the waiting copy is a held item now");
    assert_eq!((late.sealed_to.as_str(), late.also_sealed_to.as_str()), (own.key.as_str(), bo.key.as_str()), "sealed to the recipients then");
    assert!(opens_for(&st, &d, &[&own, &bo], &[]), "every copy, the late one included, opens for the owner and Bo");

    // Rolled back instead: the waiting copy is sealed to the owner alone (Bo removed first, so the
    // recipients then are the owner alone).
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor_remove", &bo.key)).0.expect("ready");
    let (report2, _) = second_report(&st, &d, 88);
    let all = decide(&st, &d, &ada, report2, "hide_and_keep");
    assert!(told(&all, &ada.key).contains("WAITS"), "it waits during phase 1 too");
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 1);
    cancel_in(&st, &own.key, d.folders());
    assert!(pending_for(&st, &grant, now_ms()).is_none(), "the cancel rolled the change back");
    let rows = st.db.held_items().unwrap();
    let late2 = rows.iter().find(|r| r.report_id == report2).expect("sealed once the change was rolled back");
    assert_eq!((late2.sealed_to.as_str(), late2.also_sealed_to.as_str()), (own.key.as_str(), bo.key.as_str()), "to the recipients as they still are: the removal never happened");
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 0);
}

/// A HIDE SEALING NOW HOLDS OFF A BEGIN (the third review's S3): while a hide seals outside any
/// change, a begin is refused with words (a copy is never sealed to recipients a change is
/// replacing); once it is done, the begin goes ahead.
///
/// Seen red 2026-11-21 with begin_in not reading `sealing`: the begin went ahead ("refused while a
/// hide seals").
#[test]
fn a_begin_waits_for_a_hide_sealing_now() {
    let d = crate::relay::handlers::held::tests::dirs("owner_sealnow");
    let (st, _first, _ada, own, _row) = one_held("owner_sealnow", &d);
    let bo = bo_member(&st);
    let sealing = SealingNow::start(&st).expect("no change in flight");
    let (grant, words) = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key));
    assert!(grant.is_none() && words.contains("being sealed"), "refused while a hide seals: {words}");
    drop(sealing);
    assert!(begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.is_some(), "and goes ahead after");
    assert!(SealingNow::start(&st).is_none(), "a hide arriving now waits instead");
}

/// THE CLAIM CODE FILE IS THE RELAY'S USER'S ALONE (the third review's S5): written 0600 on Unix, a
/// file left 0644 by an older relay set to 0600 too, and the code in it. (Windows: the data folder's
/// permissions; only the words are checked there.) Not seen red here: this machine is Windows, so
/// the mode assertions compile out locally and run on Linux CI; the round-3 code used std::fs::write
/// (0644 under the usual umask), which these assertions refuse.
#[test]
fn the_claim_code_file_is_written_for_the_relay_alone() {
    let dir = humanity_core::test_temp::dir("owner_claim_mode");
    let file = dir.path().join("owner-claim-code.txt");
    std::fs::write(&file, "old").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    write_claim_code_file_at(&file, "ABCD-EF01").unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("/claim ABCD-EF01") && !text.contains("old"), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600, "an older file is set to 0600");
        std::fs::remove_file(&file).unwrap();
        write_claim_code_file_at(&file, "1234").unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600, "a new file is made 0600");
    }
}

// ── The fourth review's findings 5 and 7 (2026-11-21) ──

/// The waiting folders in the held folder beside `d`'s uploads.
fn waiting_dirs(d: &Dirs) -> Vec<std::path::PathBuf> {
    let held = held_dir_beside(d.folders());
    let mut out: Vec<_> = std::fs::read_dir(&held)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(crate::relay::storage::held::WAITING_PREFIX))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// `src` copied to `dst`, every file at any depth.
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().filter_map(|e| e.ok()) {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

/// A HEADER IS REFUSED WHEN A FINISH STARTS BETWEEN ITS CHECK AND THE LOCK (the fourth review's
/// finding 7): the outer check (`rewrapping`) passes, then, before `new_header_in` takes the slot's
/// lock, a finish marks the change building (the test's step in that gap); the re-check under the
/// lock refuses the header (409) and the header already staged stays byte for byte.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant R-c of ownfix6-rust-mutants.json) with
/// the re-check under the lock removed: "refused by the re-check under the lock" failed, left 200,
/// right 409.
#[test]
fn a_header_is_refused_when_a_finish_starts_between_its_check_and_the_lock() {
    let d = crate::relay::handlers::held::tests::dirs("owner_gap");
    let (st, _id, _ada, own, row) = one_held("owner_gap", &d);
    let bo = bo_member(&st);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    rewrap_all(&st, &d, &own, &[&own, &bo], &grant, None);
    let now = now_ms();
    let cur = current_header_in(&st, row.id, 0, &grant, &own.key, d.folders(), now).unwrap();
    let (own_k, bo_k) = (card_of(&st, &own), card_of(&st, &bo));
    let again = rewrap_header(&kyber_of(&own), &part_context(&row.package, 0), &cur, &[own_k.as_slice(), bo_k.as_slice()]).unwrap();
    let staged = crate::relay::storage::owner::handover::staging_dir(d.folders(), &row.package).join(crate::relay::storage::owner::handover::header_file(0));
    let before = std::fs::read(&staged).unwrap();
    let gap = st.clone();
    BEFORE_HEADER_LOCK.with(|h| *h.borrow_mut() = Some(Box::new(move || slot(&gap).pending.as_mut().unwrap().building = true)));
    let status = new_header_in(&st, row.id, 0, &grant, &own.key, &again, d.folders(), now).status().as_u16();
    BEFORE_HEADER_LOCK.with(|h| *h.borrow_mut() = None);
    assert_eq!(status, 409, "refused by the re-check under the lock");
    assert_eq!(std::fs::read(&staged).unwrap(), before, "the header staged stays byte for byte");
}

/// A WAITING HIDE STOPPED MID-WAY IS COMPLETE, AND THE SEAL PASS FINISHES IT (the fourth review's
/// finding 5a). The relay stops at the hide's first file move (a panic in the test's step there):
/// the waiting folder is in place WITH its note, nothing moved, the post still in view. The next seal
/// pass, with no change in flight, finishes the hide from the note (the picture out of view, the row
/// gone) and seals it: one held item for the report, no waiting folder left.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log and ownfix7-batch2.log, mutants of
/// ownfix6-rust-mutants.json) with the round-4 order put back, the note written after the files moved
/// (R-d with R-d2): "with its note, written before any file moved" failed; with the seal pass not
/// finishing a half-done hide (R-e): "its picture is out of view" failed.
#[test]
fn a_waiting_hide_stopped_mid_way_is_complete_and_finished_by_the_seal_pass() {
    let d = crate::relay::handlers::held::tests::dirs("owner_waitstop");
    let (st, _first, ada, _own, _row) = one_held("owner_waitstop", &d);
    let (report, pic) = second_report(&st, &d, 91);
    let tom = person(2);
    crate::relay::storage::held::BEFORE_WAITING_MOVE.with(|h| *h.borrow_mut() = Some(Box::new(|| panic!("the relay stops here"))));
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| st.db.hide_post_waiting_in(report, &tom.key, 91, &ada.key, now_ms(), d.folders())));
    crate::relay::storage::held::BEFORE_WAITING_MOVE.with(|h| *h.borrow_mut() = None);
    assert!(stopped.is_err(), "the relay stopped mid-hide");
    let waiting = waiting_dirs(&d);
    assert_eq!(waiting.len(), 1, "the waiting folder is in place");
    assert!(waiting[0].join("post.json").is_file(), "with its note, written before any file moved");
    assert!(d.folders().find(&pic).is_some() && st.db.post_for_report(&tom.key, 91).unwrap().is_some(), "nothing had moved: the post is still in view");
    seal_waiting(&st, d.folders());
    assert!(d.folders().find(&pic).is_none(), "its picture is out of view");
    assert!(st.db.post_for_report(&tom.key, 91).unwrap().is_none(), "its row is gone");
    assert_eq!(st.db.held_items().unwrap().iter().filter(|r| r.report_id == report).count(), 1, "sealed: one held item for the report");
    assert!(waiting_dirs(&d).is_empty(), "no waiting folder left");
}

/// A WAITING FOLDER WITH NO NOTE IS NEVER MATCHED BY TEXT AND NEVER DELETED; IT IS COUNTED AND TOLD
/// (the fourth review's finding 5a; the fifth review's L2, 2026-11-21: round 5 paired such a folder
/// with a post by searching the messages' words for a file name, `instr(content, name)`, so a stray
/// file could be paired with the wrong post, sealed as its copy and its plaintext deleted). Three
/// folders an older build's stop could leave: one with nothing in it at all (removed: no file, so
/// nothing to pair or lose), one holding a picture whose post and report are still on the server,
/// and one holding a file nothing names. Both with a file are kept exactly as they were (the picture
/// still in the folder, no held item made for the report, nothing guessed) and counted for every
/// admin in Held copies, in words.
///
/// Seen red 2026-11-21 (scratchpad ownfix8-red-m1.log) against round 5's seal pass (identify_unnoted
/// still called): "the folder whose post a text search would guess is kept as it was" failed.
#[test]
fn a_waiting_folder_with_no_note_is_counted_never_matched_or_deleted() {
    let d = crate::relay::handlers::held::tests::dirs("owner_unnoted");
    let (st, _first, ada, _own, _row) = one_held("owner_unnoted", &d);
    let held = held_dir_beside(d.folders());
    let named = |c: &str| held.join(format!("{}{}", crate::relay::storage::held::WAITING_PREFIX, c.repeat(32)));
    let (empty, known, unknown) = (named("e"), named("a"), named("b"));
    std::fs::create_dir_all(empty.join("uploads")).unwrap();
    let (report, pic) = second_report(&st, &d, 93);
    std::fs::create_dir_all(known.join("uploads")).unwrap();
    std::fs::rename(d.uploads.join(&pic), known.join("uploads").join(&pic)).unwrap();
    std::fs::create_dir_all(unknown.join("uploads")).unwrap();
    std::fs::write(unknown.join("uploads").join("stray_1.png"), "NOBODY'S").unwrap();
    seal_waiting(&st, d.folders());
    assert!(known.join("uploads").join(&pic).is_file(), "the folder whose post a text search would guess is kept as it was");
    assert!(!known.join("post.json").exists(), "no note is made up for it");
    assert_eq!(st.db.held_items().unwrap().iter().filter(|r| r.report_id == report).count(), 0, "no held item is made for the report it might belong to");
    assert!(!empty.exists(), "the one with nothing in it is removed");
    assert!(unknown.join("uploads").join("stray_1.png").is_file(), "the unknown one is kept");
    seal_waiting(&st, d.folders());
    assert!(known.join("uploads").join(&pic).is_file() && unknown.join("uploads").join("stray_1.png").is_file(), "and kept again on the next pass");
    let mut rx = st.broadcast_tx.subscribe();
    crate::relay::handlers::held::handle_list_in(&st, &ada.key, d.folders());
    let counts = drain(&mut rx)
        .into_iter()
        .find_map(|m| match m {
            RelayMessage::HeldItems { to, counts, .. } if to == ada.key => Some(counts),
            _ => None,
        })
        .expect("the list");
    assert_eq!((counts["waiting"].as_i64(), counts["unidentified"].as_i64()), (Some(0), Some(2)), "told to the admins: {counts}");
}

/// A NEW BEGIN SEALS THE HIDES THAT WAITED FOR THE CHANGE IT REPLACES (the fourth review's finding
/// 5b): a hide waits during the owner's first change; the owner begins again (the replace path), and
/// the waiting copy is sealed then, to the recipients as they still are, and the new change covers it.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant R-g of ownfix6-rust-mutants.json) with
/// the round-4 replace path (the old change taken without sealing): "sealed when the change it waited
/// for was replaced" failed, left 1, right 0.
#[test]
fn a_new_begin_seals_the_hides_that_waited_for_the_change_it_replaces() {
    let d = crate::relay::handlers::held::tests::dirs("owner_rebegin");
    let (st, _first, ada, own, _row) = one_held("owner_rebegin", &d);
    let bo = bo_member(&st);
    let (report, _) = second_report(&st, &d, 95);
    begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    assert!(told(&decide(&st, &d, &ada, report, "hide_and_keep"), &ada.key).contains("WAITS"));
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 1);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_handover", &bo.key)).0.expect("the new change begins");
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 0, "sealed when the change it waited for was replaced");
    let rows = st.db.held_items().unwrap();
    let late = rows.iter().find(|r| r.report_id == report).expect("a held item now");
    assert_eq!((late.sealed_to.as_str(), late.also_sealed_to.as_str()), (own.key.as_str(), ""), "to the recipients as they still are");
    assert!(pending_for(&st, &grant, now_ms()).unwrap().ids.contains(&late.id), "and the new change covers it");
}

/// A HIDE NEVER WAITS LONGER THAN A CHANGE MAY LAST (the fourth review's finding 5b): a waiting copy
/// older than the change windows and a margin is sealed at once to the current recipients, and the
/// change it waits for is rolled back; one within the bound waits on.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant R-h of ownfix6-rust-mutants.json) with
/// `cap_waiting_at` finding no waiting hide: past the bound nothing happened, and "the change is
/// rolled back" failed (the assertion before "sealed past the bound").
#[test]
fn a_hide_never_waits_longer_than_a_change_may_last() {
    let d = crate::relay::handlers::held::tests::dirs("owner_cap");
    let (st, _first, ada, own, _row) = one_held("owner_cap", &d);
    let bo = bo_member(&st);
    let (report, _) = second_report(&st, &d, 97);
    let grant = begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    let now = now_ms();
    st.db.hide_post_waiting_in(report, &person(2).key, 97, &ada.key, now - WAITING_CAP_MS - 1, d.folders()).unwrap().expect("hidden");
    cap_waiting_at(&st, d.folders(), now - 2);
    assert!(pending_for(&st, &grant, now).is_some() && st.db.held_waiting_count_in(d.folders()) == 1, "within the bound it waits on");
    cap_waiting_at(&st, d.folders(), now);
    assert!(pending_for(&st, &grant, now).is_none(), "the change is rolled back");
    assert_eq!(st.db.held_waiting_count_in(d.folders()), 0, "sealed past the bound");
    assert!(st.db.held_items().unwrap().iter().any(|r| r.report_id == report));
}

/// A REPORT WHOSE HIDE WAITS IS NOT CLEARED (the fourth review's finding 5c): `/reports-clear`
/// keeps it, so the held item it becomes still names a report; once sealed, the held rule keeps it.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant R-i of ownfix6-rust-mutants.json) with
/// `held_waiting_reports_in` naming nothing: the first assertion, the waiting report named, failed,
/// left [], right [2].
#[test]
fn a_report_whose_hide_waits_is_not_cleared() {
    let d = crate::relay::handlers::held::tests::dirs("owner_keeprep");
    let (st, _first, ada, own, _row) = one_held("owner_keeprep", &d);
    let bo = bo_member(&st);
    let (report, _) = second_report(&st, &d, 99);
    begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    decide(&st, &d, &ada, report, "hide_and_keep");
    assert_eq!(st.db.held_waiting_reports_in(d.folders()), vec![report]);
    st.db.reports_clear_decided(&st.db.held_waiting_reports_in(d.folders())).unwrap();
    assert!(st.db.report_by_id(report).unwrap().is_some(), "kept while its hide waits");
    cancel_in(&st, &own.key, d.folders());
    st.db.reports_clear_decided(&st.db.held_waiting_reports_in(d.folders())).unwrap();
    assert!(st.db.report_by_id(report).unwrap().is_some(), "and kept by the held rule once sealed");
}

/// SEALING A WAITING HIDE TWICE MAKES ONE HELD ITEM (the fourth review's finding 5e): a stop between
/// recording the held row and removing the waiting folder (here: the folder put back afterwards) and
/// the next pass finds the row by the hide's id, removes the plaintext, and seals nothing again.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant R-j of ownfix6-rust-mutants.json) with
/// a random package per seal (the round-4 code): "one held item for the report" failed, left 2,
/// right 1.
#[test]
fn sealing_a_waiting_hide_twice_makes_one_held_item() {
    let d = crate::relay::handlers::held::tests::dirs("owner_twice");
    let (st, _first, ada, own, _row) = one_held("owner_twice", &d);
    let bo = bo_member(&st);
    let (report, _) = second_report(&st, &d, 101);
    begin_with(&st, &d, &own, signed_owner(&st, &own, "owner_successor", &bo.key)).0.expect("ready");
    decide(&st, &d, &ada, report, "hide_and_keep");
    let waiting = waiting_dirs(&d);
    assert_eq!(waiting.len(), 1);
    let aside = d.uploads.parent().unwrap().join("aside");
    copy_dir(&waiting[0], &aside);
    cancel_in(&st, &own.key, d.folders());
    assert!(waiting_dirs(&d).is_empty(), "sealed and its folder removed");
    copy_dir(&aside, &waiting[0]);
    seal_waiting(&st, d.folders());
    assert_eq!(st.db.held_items().unwrap().iter().filter(|r| r.report_id == report).count(), 1, "one held item for the report");
    assert!(waiting_dirs(&d).is_empty(), "the plaintext put back is removed");
}
