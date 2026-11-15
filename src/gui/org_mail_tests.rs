// Child of gui/org_mail.rs (#[path]): organisations rung 2's delivery rules, no socket. Each test
// was seen failing once on purpose, recorded at the test.

use super::*;
use humanity_protocol::org::{build_entry, build_founding, fold, role_book_from_json, Directory, Founding, Listing, ADMIN, MEMBER};
use humanity_protocol::pq_crypto::DilithiumKeypair;

const T0: u64 = 1_800_000_000_000;

fn key(n: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&[n; 32])
}

/// An organisation's objects, signed the way the apps sign (each entry following the heads).
struct Log {
    org: String,
    objects: Vec<Object>,
    clock: u64,
}

impl Log {
    fn found(owner: &DilithiumKeypair) -> Log {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/orgs/roles.json");
        let roles = role_book_from_json(&std::fs::read_to_string(path).unwrap()).unwrap().roles;
        let f = Founding { name: "Cart crew".into(), about: String::new(), directory: Directory::Members, roles };
        let o = build_founding(owner, &f, T0).unwrap();
        Log { org: o.object_id().unwrap().to_hex(), objects: vec![o], clock: T0 }
    }
    fn state(&self) -> OrgState {
        fold(&self.org, &self.objects, self.clock).unwrap()
    }
    fn sign(&mut self, kp: &DilithiumKeypair, act: Act) -> String {
        let prev = self.state().heads;
        self.clock += 1000;
        let o = build_entry(kp, &self.org, &act, &prev, self.clock).unwrap();
        let id = o.object_id().unwrap().to_hex();
        self.objects.push(o);
        id
    }
    fn join(&mut self, by: &DilithiumKeypair, who: &DilithiumKeypair, role: &str) -> String {
        let inv = self.sign(by, Act::Invite { who: who.public_key(), role: role.into(), until: self.clock + 86_400_000 });
        self.sign(who, Act::Accept { invite: inv })
    }
}

/// A change goes in whole notes no longer than the limit (one object alone may be), and reads back
/// as the same objects in any order.
///
/// Red first, 2026-11-15: with the limit in `note_texts` made a hundred times larger, every object
/// went in one note and "seven objects of about 7 KB do not fit one note" failed.
#[test]
fn a_change_goes_in_whole_notes_and_reads_back() {
    let (owner, a) = (key(1), key(2));
    let mut log = Log::found(&owner);
    log.join(&owner, &a, ADMIN);
    for n in 0..4 {
        log.sign(&a, Act::Listing { name: format!("Al {n}"), level: Listing::Members });
    }
    let notes = note_texts(&log.org, &log.objects);
    assert!(notes.len() >= 3, "seven objects of about 7 KB do not fit one note");
    let mut back = Vec::new();
    for n in &notes {
        assert!(is_note(n));
        let (org, objs) = parse_note(n).expect("a note reads");
        assert_eq!(org, log.org);
        assert!(!objs.is_empty(), "every note carries an object");
        assert!(n.len() <= NOTE_TEXT_MAX || objs.len() == 1, "a note is within the limit unless one object is alone");
        back.extend(objs);
    }
    let ids = |v: &[Object]| v.iter().map(|o| o.object_id().unwrap().to_hex()).collect::<Vec<_>>();
    assert_eq!(ids(&back), ids(&log.objects));
}

/// A removed member stops receiving: the removal itself goes to them (so they learn it), nothing
/// after it does, and a note from them is no longer taken by a member's app.
///
/// Red first, 2026-11-15: with `recipients` also listing everyone ever invited, the change after the
/// removal still went to C ("nothing after the removal goes to C" failed); with `judge` answering
/// Merge for every sender, the owner's app took C's note after the removal.
#[test]
fn a_removed_member_stops_receiving() {
    let (owner, a, c) = (key(1), key(2), key(4));
    let mut log = Log::found(&owner);
    log.join(&owner, &a, ADMIN);
    log.join(&a, &c, MEMBER);
    let removal = Act::Remove { who: c.public_key() };
    log.sign(&a, removal.clone());
    let st = log.state();
    assert!(recipients(&st, &a.public_key(), &removal).contains(&c.public_key()), "the removal itself goes to C");
    let after = Act::Settings { name: Some("Cans".into()), about: None, directory: None };
    log.sign(&owner, after.clone());
    assert!(!recipients(&log.state(), &owner.public_key(), &after).contains(&c.public_key()), "nothing after the removal goes to C");
    // C, removed, signs a change; the owner's app does not merge C's note (parked, never merged).
    let late = build_entry(&c, &log.org, &Act::Listing { name: "C".into(), level: Listing::Members }, &log.state().heads, log.clock + 1000).unwrap();
    assert_eq!(judge(&log.org, &log.objects, &[late], &owner.public_key(), &c.public_key(), log.clock + 2000), Take::Park);
    // And C's own app, holding the removal, takes nothing more from the members.
    let next = build_entry(&owner, &log.org, &Act::Settings { name: Some("Cans 2".into()), about: None, directory: None }, &log.state().heads, log.clock + 3000).unwrap();
    assert_eq!(judge(&log.org, &log.objects, &[next], &c.public_key(), &owner.public_key(), log.clock + 4000), Take::Drop("not_mine"));
}

/// Only my own devices, fellow members and invitations are taken; a stranger's note is not; a
/// note from someone not yet known as a member waits parked until the log that proves them comes.
///
/// Red first, 2026-11-15: with `judge` answering Merge for every sender, the stranger's note to an
/// organisation I hold read Merge where Park is right.
#[test]
fn only_my_devices_members_and_invitations_are_taken() {
    let (owner, a, b, stranger) = (key(1), key(2), key(3), key(9));
    let mut log = Log::found(&owner);
    log.join(&owner, &a, ADMIN);
    let entry = log.objects.last().unwrap().clone();
    let now = log.clock + 1000;
    // My own key: another device of mine.
    assert_eq!(judge(&log.org, &log.objects, &[entry.clone()], &a.public_key(), &a.public_key(), now), Take::Merge);
    // A fellow member.
    assert_eq!(judge(&log.org, &log.objects, &[entry.clone()], &a.public_key(), &owner.public_key(), now), Take::Merge);
    // A stranger, to an organisation I hold: parked until a log proves them, never merged.
    assert_eq!(judge(&log.org, &log.objects, &[entry.clone()], &a.public_key(), &stranger.public_key(), now), Take::Park);
    // A stranger's note about an organisation I do not hold: dropped.
    assert_eq!(judge(&log.org, &[], &[entry], &b.public_key(), &stranger.public_key(), now), Take::Drop("not_invited"));
    // An invitation for me from its inviter: taken, though I hold nothing of it.
    let inv = log.sign(&a, Act::Invite { who: b.public_key(), role: MEMBER.into(), until: log.clock + 86_400_000 });
    let st = log.state();
    let chain: Vec<String> = std::iter::once(log.org.clone()).chain(st.invites[&inv].support.iter().cloned()).chain(std::iter::once(inv.clone())).collect();
    let carried: Vec<Object> = log.objects.iter().filter(|o| chain.contains(&o.object_id().unwrap().to_hex())).cloned().collect();
    assert_eq!(judge(&log.org, &[], &carried, &b.public_key(), &a.public_key(), log.clock), Take::Merge);
    // The same invitation relayed by someone who did not make it: dropped.
    assert_eq!(judge(&log.org, &[], &carried, &b.public_key(), &owner.public_key(), log.clock), Take::Drop("not_invited"));
}

/// The inviter's app sees which members joined through its own invitations, with their acceptance.
///
/// Red first, 2026-11-15: with the `i.by == me` test removed, the owner's app also claimed A's
/// invitee ("only the inviter forwards" failed).
#[test]
fn the_inviter_forwards_an_acceptance_of_its_invitation() {
    let (owner, a, b) = (key(1), key(2), key(3));
    let mut log = Log::found(&owner);
    log.join(&owner, &a, ADMIN);
    let before = log.state();
    let accept = log.join(&a, &b, MEMBER);
    let after = log.state();
    assert_eq!(joined_by_my_invitation(&before, &after, &a.public_key()), vec![(b.public_key(), accept)]);
    assert!(joined_by_my_invitation(&before, &after, &owner.public_key()).is_empty(), "only the inviter forwards");
    // What a signed invitation queues: my devices, every other member, and the invitation to B.
    let inv = log.sign(&a, Act::Invite { who: key(5).public_key(), role: MEMBER.into(), until: log.clock + 86_400_000 });
    let st = log.state();
    let act = Act::Invite { who: key(5).public_key(), role: MEMBER.into(), until: 0 };
    let items = deliveries(&st, &a.public_key(), &inv, &act, Some(&[log.org.clone(), inv.clone()]));
    let to: BTreeSet<String> = items.iter().map(|i| i.to.clone()).collect();
    let want: BTreeSet<String> = [&a, &owner, &b, &key(5)].iter().map(|k| hex::encode(k.public_key())).collect();
    assert_eq!(to, want);
}

/// The very note texts of one delivery, written and signed by the WEBSITE
/// (scripts/tests/fixtures/org-note-v1.json, from scripts/gen-org-note-fixture.mjs), taken through
/// this app's arrival rule and folded: the state written by hand there, as
/// scripts/tests/org-mail-web.test.js reaches with the website's rules. One set of bytes, one answer.
///
/// Red first, 2026-11-15: with the invitation path of `judge` removed (an organisation this device
/// does not hold always dropped), "every note is taken" failed on the invitation.
#[test]
fn the_notes_of_one_delivery_apply_as_recorded() {
    use base64::Engine;
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/org-note-v1.json");
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("the note fixture")).unwrap();
    let org_id = fx["org"].as_str().unwrap();
    let now = fx["now"].as_u64().unwrap();
    let key = |who: &str| base64::engine::general_purpose::STANDARD.decode(fx["keys"][who].as_str().unwrap()).unwrap();
    let notes = fx["notes"].as_array().unwrap();
    // The inviter's own log starts with what it signed itself: the founding and the invitation.
    let mut logs: std::collections::BTreeMap<&str, Vec<Object>> = [("a", parse_note(notes[0]["text"].as_str().unwrap()).unwrap().1), ("b", Vec::new())].into_iter().collect();
    for n in notes {
        let (to, from) = (n["to"].as_str().unwrap(), n["from"].as_str().unwrap());
        let (org, objects) = parse_note(n["text"].as_str().unwrap()).expect("a note reads");
        assert_eq!(org, org_id);
        let mine = logs[to].clone();
        assert_eq!(judge(org_id, &mine, &objects, &key(to), &key(from), now), Take::Merge, "every note is taken: {}", n["why"]);
        org::merge(org_id, logs.get_mut(to).unwrap(), objects);
    }
    let st = fold(org_id, &logs["b"], now).unwrap();
    let e = &fx["expect"];
    assert_eq!(st.name, e["name"].as_str().unwrap());
    let members: serde_json::Map<String, serde_json::Value> = st.members.values().map(|m| (m.did.clone(), serde_json::json!(m.role))).collect();
    assert_eq!(serde_json::Value::Object(members), e["members"]);
    assert_eq!(st.held as u64, e["held"].as_u64().unwrap());
    let applied: Vec<serde_json::Value> = st.log.iter().skip(1).filter(|l| l.refused.is_none()).map(|l| serde_json::json!(l.entry_id)).collect();
    assert_eq!(serde_json::Value::from(applied), e["entries"]);
}
