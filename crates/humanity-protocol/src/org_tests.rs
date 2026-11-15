//! Organisations, rung 1 (docs/design/organisations.md): the fold's promises, each seen red first.

use super::*;
use crate::pq_crypto::DilithiumKeypair;

/// A test organisation: its objects in the order they were made, and a clock that ticks a second
/// per change.
struct World {
    org: String,
    objects: Vec<Object>,
    clock: u64,
}

fn key(n: u8) -> DilithiumKeypair {
    DilithiumKeypair::from_seed(&[n; 32])
}

/// The roles a new organisation starts with: the shipped file.
fn book() -> RoleBook {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/orgs/roles.json");
    role_book_from_json(&std::fs::read_to_string(path).expect("data/orgs/roles.json")).expect("the shipped role book reads")
}

const T0: u64 = 1_800_000_000_000;

impl World {
    fn found(owner: &DilithiumKeypair, directory: Directory) -> World {
        let f = Founding { name: "Cart crew".into(), about: "Saturday cans".into(), directory, roles: book().roles };
        let o = build_founding(owner, &f, T0).expect("founds");
        World { org: o.object_id().unwrap().to_hex(), objects: vec![o], clock: T0 }
    }
    /// Sign `act` by `kp` one second after the last change, following the heads (as an app does);
    /// its entry id.
    fn sign(&mut self, kp: &DilithiumKeypair, act: Act) -> String {
        let prev = self.state().heads;
        self.clock += 1000;
        let at = self.clock;
        self.sign_as(kp, act, &prev, at)
    }
    /// Sign `act` following `prev`, dated `at`, and keep it; its entry id.
    fn sign_as(&mut self, kp: &DilithiumKeypair, act: Act, prev: &[String], at: u64) -> String {
        let o = build_entry(kp, &self.org, &act, prev, at).expect("signs");
        let id = o.object_id().unwrap().to_hex();
        self.objects.push(o);
        id
    }
    fn state(&self) -> OrgState {
        fold(&self.org, &self.objects, self.clock).expect("folds")
    }
    fn invite(&mut self, by: &DilithiumKeypair, who: &DilithiumKeypair, role: &str) -> String {
        self.sign(by, Act::Invite { who: who.public_key(), role: role.into(), until: self.clock + 86_400_000 })
    }
    /// Invite `who` as `role` (by `by`) and have them accept.
    fn join(&mut self, by: &DilithiumKeypair, who: &DilithiumKeypair, role: &str) {
        let inv = self.invite(by, who, role);
        self.sign(who, Act::Accept { invite: inv });
        assert_eq!(self.state().member(&who.public_key()).map(|m| m.role.clone()).as_deref(), Some(role), "joined as {role}");
    }
    fn refusal_of(&self, entry: &str) -> Option<Refusal> {
        self.state().log.iter().find(|l| l.entry_id == entry).expect("the entry is in the log").refused
    }
}

/// A role grants exactly its permissions: for the shipped admin and member roles and a custom role
/// per enforced permission, every rung 1 action is tried; it is applied exactly when the role lists
/// the permission it needs. Measured on what the fold DID (the entry applied or refused), not on
/// what the role says.
///
/// Red first, 2026-11-14: with `need(st, perm::REMOVE)?` taken out of `Act::Remove`, "member may
/// members.remove: false, but the fold applied it" failed for the member role.
#[test]
fn a_role_grants_exactly_its_permissions() {
    let owner = key(1);
    let mut roles_to_try: Vec<Role> = book().roles.into_iter().filter(|r| r.id != OWNER).collect();
    for p in perm::ENFORCED {
        roles_to_try.push(Role { id: format!("only_{}", p.replace('.', "_")), name: format!("Only {p}"), rank: 30, perms: vec![p.to_string()] });
    }
    for role in roles_to_try {
        let mut w = World::found(&owner, Directory::Organisers);
        // A role ranked below every tested one, for targets and grants.
        w.sign(&owner, Act::RoleSet { role: Role { id: "low".into(), name: "Low".into(), rank: 0, perms: vec![] } });
        if !role.is_built_in() {
            w.sign(&owner, Act::RoleSet { role: role.clone() });
        }
        let actor = key(2);
        w.join(&owner, &actor, &role.id);
        let target = key(3);
        w.join(&owner, &target, "low");
        let st = w.state();
        let a = actor.public_key();
        let tried: Vec<(&str, String)> = vec![
            (perm::INVITE, w.invite(&actor, &key(4), "low")),
            (perm::ASSIGN, w.sign(&actor, Act::Assign { who: target.public_key(), role: MEMBER.into() })),
            (perm::DEFINE, w.sign(&actor, Act::RoleSet { role: Role { id: "new_one".into(), name: "New".into(), rank: 0, perms: vec![] } })),
            (perm::SETTINGS, w.sign(&actor, Act::Settings { name: Some("Renamed".into()), about: None, directory: None })),
            (perm::REMOVE, w.sign(&actor, Act::Remove { who: target.public_key() })),
        ];
        for (p, entry) in tried {
            let applied = w.refusal_of(&entry).is_none();
            assert_eq!(applied, role.grants(p), "{} may {p}: {}, but the fold {}", role.id, role.grants(p), if applied { "applied it" } else { "refused it" });
        }
        // directory.see_all: reading an organisers-only directory.
        assert_eq!(st.directory_for(&a).is_ok(), role.grants(perm::SEE_ALL), "{} reads an organisers-only directory", role.id);
        assert_eq!(st.can(&a, perm::SEE_ALL), role.grants(perm::SEE_ALL));
    }
}

/// Rank: nobody acts on an equal or better, so an admin never removes or demotes the owner or
/// another admin, and the owner never leaves.
#[test]
fn nobody_acts_on_their_equal_or_better() {
    let (owner, a1, a2) = (key(1), key(2), key(3));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a1, ADMIN);
    w.join(&owner, &a2, ADMIN);
    let r1 = w.sign(&a1, Act::Remove { who: owner.public_key() });
    let r2 = w.sign(&a1, Act::Remove { who: a2.public_key() });
    let r3 = w.sign(&a1, Act::Assign { who: a2.public_key(), role: MEMBER.into() });
    let r4 = w.invite(&a1, &key(9), ADMIN);
    let r5 = w.sign(&owner, Act::Leave);
    assert_eq!(w.refusal_of(&r1), Some(Refusal::OwnerStays));
    assert_eq!(w.refusal_of(&r2), Some(Refusal::RankTooLow));
    assert_eq!(w.refusal_of(&r3), Some(Refusal::RankTooLow));
    assert_eq!(w.refusal_of(&r4), Some(Refusal::RankTooLow));
    assert_eq!(w.refusal_of(&r5), Some(Refusal::OwnerStays));
    assert_eq!(w.state().members.len(), 3);
}

/// A non-member cannot read a private directory, nor the audit log; an organisers-only directory
/// is for organisers; a public one names only those who chose to be named; and an invitation
/// carries the chain that proves it, never the member list.
///
/// Red first, 2026-11-14: with the `Directory::Members if !member` arm of `directory_for` removed,
/// "a stranger reads a members-only directory" failed (it returned the list).
#[test]
fn a_non_member_cannot_read_a_private_directory() {
    let (owner, admin, m1, shy, stranger) = (key(1), key(2), key(3), key(4), key(5));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &admin, ADMIN);
    w.join(&admin, &m1, MEMBER);
    w.join(&admin, &shy, MEMBER);
    w.sign(&shy, Act::Listing { name: "Shy".into(), level: Listing::Organisers });
    let st = w.state();
    assert_eq!(st.directory_for(&stranger.public_key()), Err(ReadRefusal::NotAMember), "a stranger reads a members-only directory");
    assert_eq!(st.audit_log_for(&stranger.public_key()).err(), Some(ReadRefusal::NotAMember), "a stranger reads the audit log");
    let as_member = st.directory_for(&m1.public_key()).expect("a member reads it");
    assert_eq!((as_member.named.len(), as_member.private), (3, 1), "the shy member is counted, not named, to a member");
    let as_admin = st.directory_for(&admin.public_key()).expect("an admin reads it");
    assert_eq!((as_admin.named.len(), as_admin.private), (4, 0), "organisers see everyone");
    // The shy member's lines in the audit log are not named to an ordinary member.
    let lines = st.audit_log_for(&m1.public_key()).unwrap();
    assert!(lines.iter().any(|(_, _, s)| s == &Some(Shown::Private)), "the shy member's acceptance shows as private");
    assert!(lines.iter().all(|(_, a, s)| !matches!(a, Shown::Named { name: Some(n), .. } if n == "Shy") && !matches!(s, Some(Shown::Named { name: Some(n), .. }) if n == "Shy")));

    w.sign(&owner, Act::Settings { name: None, about: None, directory: Some(Directory::Organisers) });
    let st = w.state();
    assert_eq!(st.directory_for(&m1.public_key()), Err(ReadRefusal::OrganisersOnly));
    assert!(st.directory_for(&admin.public_key()).is_ok());

    w.sign(&owner, Act::Settings { name: None, about: None, directory: Some(Directory::Public) });
    let st = w.state();
    let public = st.directory_for(&stranger.public_key()).expect("anyone reads a public directory");
    assert_eq!((public.named.len(), public.private), (3, 1), "a public directory still keeps the shy member's name");

    // The invitation a stranger holds names the founder, the inviter and the stranger, nobody else.
    w.sign(&owner, Act::Settings { name: None, about: None, directory: Some(Directory::Members) });
    let inv = w.invite(&admin, &stranger, MEMBER);
    let text = w.state().invite_bundle(&w.objects, &inv).expect("the bundle");
    let (org, objs) = parse_bundle(&text).expect("parses");
    // A member's fold holds what follows entries the invitation leaves out (rung 2); the invitee's
    // fold places them by what is there.
    assert!(!fold(&org, &objs, w.clock).expect("folds").invites.contains_key(&inv));
    let theirs = fold_invitation(&org, &objs, w.clock).expect("the invitee folds it");
    let known: BTreeSet<String> = theirs.log.iter().flat_map(|l| [Some(l.actor.clone()), l.subject.clone()]).flatten().map(|k| did_for_pubkey(&k)).collect();
    let expect: BTreeSet<String> = [&owner, &admin, &stranger].iter().map(|k| did_for_pubkey(&k.public_key())).collect();
    assert_eq!(known, expect, "the invitation names only its proof chain");
    assert!(theirs.invites.contains_key(&inv), "and it proves the invitation");
}

/// An invitation works only when the invitee accepts: an invite alone adds nobody; an acceptance
/// signed by anyone else, after expiry, after withdrawal, twice, or for an invite made before the
/// invitee was removed, is refused.
///
/// Red first, 2026-11-14: with the `inv.who != actor` check removed from `Act::Accept`, "someone
/// else accepts for the invitee" failed (the forger became a member).
#[test]
fn an_invitation_works_only_when_the_invitee_accepts() {
    let (owner, x, y) = (key(1), key(2), key(3));
    let mut w = World::found(&owner, Directory::Members);
    let inv = w.invite(&owner, &x, MEMBER);
    assert!(w.state().member(&x.public_key()).is_none(), "an invite alone adds nobody");
    let forged = w.sign(&y, Act::Accept { invite: inv.clone() });
    assert_eq!(w.refusal_of(&forged), Some(Refusal::NotTheInvitee), "someone else accepts for the invitee");
    assert!(w.state().member(&y.public_key()).is_none());
    let ok = w.sign(&x, Act::Accept { invite: inv.clone() });
    assert_eq!(w.refusal_of(&ok), None);
    assert!(w.state().member(&x.public_key()).is_some());
    w.sign(&x, Act::Leave);
    let again = w.sign(&x, Act::Accept { invite: inv });
    assert_eq!(w.refusal_of(&again), Some(Refusal::InviteUsed), "an invite works once");

    // Expired.
    let late = key(4);
    let inv = w.sign(&owner, Act::Invite { who: late.public_key(), role: MEMBER.into(), until: w.clock + 500 });
    let acc = w.sign(&late, Act::Accept { invite: inv });
    assert_eq!(w.refusal_of(&acc), Some(Refusal::InviteExpired));

    // Withdrawn.
    let z = key(5);
    let inv = w.invite(&owner, &z, MEMBER);
    w.sign(&owner, Act::Withdraw { invite: inv.clone() });
    let acc = w.sign(&z, Act::Accept { invite: inv });
    assert_eq!(w.refusal_of(&acc), Some(Refusal::InviteWithdrawn));

    // Made before a removal: the removed person needs a new invitation.
    let r = key(6);
    w.join(&owner, &r, MEMBER);
    w.sign(&owner, Act::Remove { who: r.public_key() });
    let fresh = w.invite(&owner, &r, MEMBER);
    w.sign(&r, Act::Accept { invite: fresh });
    assert!(w.state().member(&r.public_key()).is_some(), "a fresh invitation after the removal works");
}

/// Every membership and role change appears in the audit log, signed by its actor: folding the log
/// one entry at a time, each change to who is in it or which role they hold is matched by that
/// entry's line, applied, whose actor is the object's author and whose signature verifies; a
/// forged entry is not in the log at all and changes nothing.
///
/// Red first, 2026-11-14: with `st.log.push(...)` skipped for applied entries in `fold`, "a change
/// with no applied line" failed on the first acceptance.
#[test]
fn every_membership_change_appears_in_the_audit_log_signed_by_its_actor() {
    let (owner, a, b, c) = (key(1), key(2), key(3), key(4));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a, ADMIN);
    w.join(&a, &b, MEMBER);
    w.join(&a, &c, MEMBER);
    w.sign(&owner, Act::RoleSet { role: Role { id: "driver".into(), name: "Driver".into(), rank: 20, perms: vec![] } });
    w.sign(&a, Act::Assign { who: b.public_key(), role: "driver".into() });
    w.sign(&a, Act::Remove { who: c.public_key() });
    w.sign(&b, Act::Leave);
    w.sign(&owner, Act::RoleDrop { role: "driver".into() });

    // A forged entry: a real one with its signature broken.
    let mut forged = build_entry(&a, &w.org, &Act::Remove { who: owner.public_key() }, &w.state().heads, w.clock + 1).unwrap();
    forged.signature[10] ^= 0xff;
    let forged_id = forged.object_id().unwrap().to_hex();
    w.objects.push(forged);

    let roster = |st: &OrgState| st.members.values().map(|m| (m.did.clone(), m.role.clone())).collect::<BTreeMap<_, _>>();
    let mut before = roster(&fold(&w.org, &w.objects[..1], w.clock).unwrap());
    let mut changes = 0;
    for n in 2..=w.objects.len() {
        let st = fold(&w.org, &w.objects[..n], w.clock).unwrap();
        let after = roster(&st);
        if after != before {
            changes += 1;
            let o = &w.objects[n - 1];
            let id = o.object_id().unwrap().to_hex();
            let line = st.log.iter().find(|l| l.entry_id == id).unwrap_or_else(|| panic!("a change with no applied line: {id}"));
            assert_eq!(line.refused, None, "a change with no applied line: {id}");
            assert_eq!(line.actor, o.author_public_key, "the line names the signer");
            assert!(o.verify_signature().is_ok(), "the line's entry verifies");
        }
        before = after;
    }
    assert_eq!(changes, 6, "three joins, a role change, a removal and a leave");
    let st = w.state();
    assert!(st.log.iter().all(|l| l.entry_id != forged_id), "a forged entry is not in the log");
    assert!(st.member(&owner.public_key()).is_some());
    assert_eq!(st.dropped, 1);
}

/// The fold is the same whatever order the objects arrive in, and opening the same text twice
/// changes nothing (the merge).
#[test]
fn any_order_and_any_repeat_fold_the_same() {
    let (owner, a, b) = (key(1), key(2), key(3));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a, ADMIN);
    w.join(&a, &b, MEMBER);
    w.sign(&b, Act::Listing { name: "Bea".into(), level: Listing::Members });
    let want = w.state();
    let mut reversed: Vec<Object> = w.objects.iter().rev().cloned().collect();
    reversed.extend(w.objects.iter().cloned());
    let got = fold(&w.org, &reversed, w.clock).unwrap();
    assert_eq!(got.members, want.members);
    assert_eq!(got.log, want.log);

    let mut log = Vec::new();
    let text = bundle_text(&w.org, &w.objects);
    let (org, objs) = parse_bundle(&format!("Here you go: {text} thanks")).unwrap();
    assert_eq!(merge(&org, &mut log, objs.clone()), w.objects.len());
    assert_eq!(merge(&org, &mut log, objs), 0, "opening it twice adds nothing");
}

/// The shipped role book holds together: the owner holds every permission, nothing outranks it,
/// and the founding it makes is acceptable.
#[test]
fn the_shipped_roles_found_an_organisation() {
    let b = book();
    let owner = b.roles.iter().find(|r| r.id == OWNER).expect("an owner role");
    assert_eq!(owner.rank, TOP_RANK);
    assert_eq!(owner.perms, b.permissions, "the owner holds every permission, in the file's order");
    for p in perm::ENFORCED {
        assert!(b.permissions.iter().any(|x| x == p), "{p} is in the catalogue");
    }
    let f = Founding { name: "x".into(), about: String::new(), directory: Directory::Members, roles: b.roles };
    assert_eq!(founding_problem(&f), None);
}

/// The frozen log the WEBSITE built and signed (scripts/tests/fixtures/org-log-v1.json, from
/// scripts/gen-org-fixture.mjs) folds here to the state written by hand beside it, as it does in
/// scripts/tests/org-log-web.test.js: the desktop checks the website's signatures and encoding, and
/// both apps give one answer. It is also the shape a saved log has (`orgs/<id>.json`), opened the
/// way the desktop opens one, so this is the saved shape's guard too: change the shape and this
/// fails until the frozen file is rebuilt on purpose.
///
/// Red first, 2026-11-14: with `need(st, perm::REMOVE)?` and the `inv.who != actor` check taken
/// out, the members differed from the record (Mo's attempt to remove Shy applied); the website's
/// twin, with its invitee check taken out, read `already_member` where the record says
/// `not_the_invitee`. Rung 2, 2026-11-15 (the log rebuilt on purpose with `prev`, a handover, a
/// wrong member's acceptance, a voided change and a held entry): with holding switched off, with
/// the voiding pass returning after the first fold, and with the handover's recipient check taken
/// out, this test failed each time.
#[test]
fn the_frozen_log_folds_as_recorded() {
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/org-log-v1.json");
    let text = std::fs::read_to_string(&path).expect("the frozen log");
    let fx: serde_json::Value = serde_json::from_str(&text).unwrap();
    let (org, objects) = parse_bundle(&text).expect("a saved log opens");
    assert_eq!(org, fx["org"].as_str().unwrap());
    let now = fx["now"].as_u64().unwrap();
    let st = fold(&org, &objects, now).expect("folds");
    let e = &fx["expect"];
    assert_eq!(st.name, e["name"].as_str().unwrap());
    assert_eq!(st.directory.as_str(), e["directory"].as_str().unwrap());
    let roles: Vec<&str> = st.roles.iter().map(|r| r.id.as_str()).collect();
    let want_roles: Vec<&str> = e["roles"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    assert_eq!(roles, want_roles);
    let members: serde_json::Map<String, serde_json::Value> = st
        .members
        .values()
        .map(|m| (m.did.clone(), serde_json::json!({ "role": m.role, "name": m.name, "listing": m.listing.as_str() })))
        .collect();
    assert_eq!(serde_json::Value::Object(members), e["members"]);
    let open: Vec<String> = st.open_invites(now).iter().map(|i| i.id.clone()).collect();
    let want_open: Vec<String> = e["open_invites"].as_array().unwrap().iter().map(|i| i.as_str().unwrap().to_string()).collect();
    assert_eq!(open, want_open);
    let log: Vec<serde_json::Value> = st.log.iter().map(|l| serde_json::json!({ "entry": l.entry_id, "refused": l.refused.map(Refusal::word) })).collect();
    assert_eq!(serde_json::Value::from(log), e["log"]);
    let reader = |who: &str| B64.decode(fx["reader_keys"][who].as_str().unwrap()).unwrap();
    let count = |v: DirectoryView| serde_json::json!({ "named": v.named.len(), "private": v.private });
    assert_eq!(count(st.directory_for(&reader("member")).unwrap()), e["directory_as_member"]);
    assert_eq!(count(st.directory_for(&reader("admin")).unwrap()), e["directory_as_admin"]);
    assert_eq!(st.directory_for(&reader("stranger")), Err(ReadRefusal::NotAMember));
    assert_eq!(e["stranger_refused"], "not_a_member");
    assert_eq!(st.held as u64, e["held"].as_u64().unwrap(), "the entry following one this log does not hold waits");
}

/// Rung 2: an entry delivered before an entry it follows waits (held: never applied, never refused)
/// and applies once that entry arrives, whatever order delivery took.
///
/// Red first, 2026-11-15: with the held loop in `fold_as` skipped (`hold &&` made `false &&`),
/// "nothing that follows it is refused" failed: B's listing, delivered before B's acceptance, was
/// refused as not_a_member instead of waiting.
#[test]
fn an_entry_that_arrives_before_what_it_follows_waits_then_applies() {
    let (owner, a, b) = (key(1), key(2), key(3));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a, ADMIN);
    let inv = w.invite(&a, &b, MEMBER);
    let accept = w.sign(&b, Act::Accept { invite: inv });
    let listing = w.sign(&b, Act::Listing { name: "Bea".into(), level: Listing::Members });
    let want = w.state();
    let early: Vec<Object> = w.objects.iter().filter(|o| o.object_id().unwrap().to_hex() != accept).cloned().collect();
    let st = fold(&w.org, &early, w.clock).unwrap();
    assert_eq!(st.held, 1, "the listing waits for the acceptance it follows");
    assert!(st.log.iter().all(|l| l.entry_id != listing), "nothing that follows it is refused, or applied");
    assert!(st.member(&b.public_key()).is_none());
    let mut all = early.clone();
    all.push(w.objects.iter().find(|o| o.object_id().unwrap().to_hex() == accept).unwrap().clone());
    let st = fold(&w.org, &all, w.clock).unwrap();
    assert_eq!(st.held, 0);
    assert_eq!(st.log, want.log);
    assert_eq!(st.member(&b.public_key()).and_then(|m| m.name.clone()).as_deref(), Some("Bea"));
}

/// Rung 2, the checkpoint (section 2.5): a removed admin signs a removal dated before their own
/// removal, following only what they had seen then. The owner, who removed them without seeing it,
/// is the checkpoint: the backdated change is voided on every app, so the member it named stays.
///
/// Red first, 2026-11-15: with the voiding loop in `fold_as` returning after the first pass, "the
/// backdated removal is voided" failed: it was applied and C was gone.
#[test]
fn a_change_backdated_before_its_signers_removal_is_voided() {
    let (owner, a, c) = (key(1), key(2), key(4));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a, ADMIN);
    w.join(&a, &c, MEMBER);
    let seen = w.state().heads;
    let at = w.clock;
    // The owner removes A, having seen nothing A did after this point.
    let removal = w.sign(&owner, Act::Remove { who: a.public_key() });
    // A, removed, signs a removal of C dated before their own removal, following what they had seen.
    let backdated = w.sign_as(&a, Act::Remove { who: c.public_key() }, &seen, at + 500);
    let st = w.state();
    assert_eq!(w.refusal_of(&removal), None);
    assert_eq!(w.refusal_of(&backdated), Some(Refusal::Voided), "the backdated removal is voided");
    assert!(st.member(&c.public_key()).is_some(), "C stays");
    // The same change the owner HAD seen (it is in what the removal follows) stands.
    let mut w2 = World::found(&owner, Directory::Members);
    w2.join(&owner, &a, ADMIN);
    w2.join(&a, &c, MEMBER);
    let seen_change = w2.sign(&a, Act::Listing { name: "Al".into(), level: Listing::Members });
    w2.sign(&owner, Act::Remove { who: a.public_key() });
    assert_eq!(w2.refusal_of(&seen_change), None, "a change the remover had seen stands");
}

/// Rung 2: the owner hands the organisation on only with both signatures: the owner's offer and the
/// named member's acceptance. Someone else's acceptance, an offer by a non-owner, a withdrawn offer
/// and an offer alone move nothing; the acceptance moves the owner role and makes the giver an admin.
///
/// Red first, 2026-11-15: with the `t.who != actor` check taken out of `AcceptTransfer`, "only the
/// member it names can take it" failed (C became the owner).
#[test]
fn a_handover_needs_both_signatures() {
    let (owner, a, c) = (key(1), key(2), key(4));
    let mut w = World::found(&owner, Directory::Members);
    w.join(&owner, &a, ADMIN);
    w.join(&a, &c, MEMBER);
    let by_admin = w.sign(&a, Act::Transfer { who: c.public_key(), until: w.clock + 86_400_000 });
    assert_eq!(w.refusal_of(&by_admin), Some(Refusal::OwnerOnly), "only the owner offers it");
    let offer = w.sign(&owner, Act::Transfer { who: a.public_key(), until: w.clock + 86_400_000 });
    assert_eq!(w.state().owner(), Some(owner.public_key().as_slice()), "an offer alone moves nothing");
    let wrong = w.sign(&c, Act::AcceptTransfer { transfer: offer.clone() });
    assert_eq!(w.refusal_of(&wrong), Some(Refusal::NotTheRecipient), "only the member it names can take it");
    assert_eq!(w.state().owner(), Some(owner.public_key().as_slice()));
    // A withdrawn offer cannot be taken.
    let mut w2 = World::found(&owner, Directory::Members);
    w2.join(&owner, &a, ADMIN);
    let offer2 = w2.sign(&owner, Act::Transfer { who: a.public_key(), until: w2.clock + 86_400_000 });
    w2.sign(&owner, Act::Withdraw { invite: offer2.clone() });
    let late = w2.sign(&a, Act::AcceptTransfer { transfer: offer2 });
    assert_eq!(w2.refusal_of(&late), Some(Refusal::HandoverClosed));
    // Both signatures: the role moves.
    let taken = w.sign(&a, Act::AcceptTransfer { transfer: offer });
    assert_eq!(w.refusal_of(&taken), None);
    let st = w.state();
    assert_eq!(st.owner(), Some(a.public_key().as_slice()));
    assert_eq!(st.member(&owner.public_key()).map(|m| m.role.as_str()), Some(ADMIN));
    // The new owner can now do what only an owner may: remove the old one, now an admin.
    let out = w.sign(&a, Act::Remove { who: owner.public_key() });
    assert_eq!(w.refusal_of(&out), None);
}
