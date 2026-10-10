//! The app-state half of 10j (src/engine/group_report.rs) without a socket or a server: the
//! Report dialog's choices for a group message, what Send builds (one sealed DM flagged
//! `group_report` to the creator, no self-copy, the inner text exactly the marker and the JSON),
//! the refusal, a report arriving (taken before the "show as a request" rule, then kept or
//! dropped by its check), and the creator's Remove, Block and Dismiss. DMs are really signed and
//! sealed here, and opened with the recipient's own key. Each test was seen red once on purpose,
//! recorded at the test.

use super::*;
use crate::net::api_v2::P2pGroupInfo;
use crate::net::dm_pq;
use crate::net::group_report::{CheckOutcome, Report};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

/// The group in these tests (its id is never checked against a signature here; the net tests do
/// that with real objects).
const G: &str = "abababababababababababababababababababababababababababababababab";
const OBJ: &str = "0101010101010101010101010101010101010101010101010101010101010101";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn kyber_of(seed: &[u8]) -> String {
    dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
}

/// Ann, Ben (who created the group) and Cy, each with a seed and a key.
struct People {
    ann: (Vec<u8>, String),
    ben: (Vec<u8>, String),
    cy: (Vec<u8>, String),
}

fn people(n: u8) -> People {
    People { ann: identity(n), ben: identity(n + 1), cy: identity(n + 2) }
}

/// The app signed in as `who`, on a server of its own, with its own DM store and block list, the
/// group "Hikers" in its list (Ann, Ben and Cy in it; `creator` says whether `who` created it),
/// the three in the member list, and the reasons loaded. Nothing outlives the test but the two
/// temp files, which `done` removes.
fn app(who: &(Vec<u8>, String), p: &People, tag: &str, creator: bool) -> GuiState {
    let mut gs = GuiState::default();
    gs.profile_public_key = who.1.clone();
    gs.private_key_bytes = Some(who.0.clone());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("wss://{tag}-{nanos}.example");
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    gs.dm_store = Some(DmStore::load(&who.0, &who.1, &server));
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(&who.0, &who.1, tag));
    gs.reports.reasons = crate::net::report::parse_reasons(br#"[{"id":"harassment","label":"Harassment","help":"h"}]"#).unwrap();
    gs.p2p_groups = vec![P2pGroupInfo {
        group_id: G.into(),
        name: "Hikers".into(),
        members: vec![p.ann.1.clone(), p.ben.1.clone(), p.cy.1.clone()],
        is_creator: creator,
    }];
    gs.p2p_groups_last_fetch = Some(Instant::now());
    for (name, key) in [("Ann", &p.ann.1), ("Ben", &p.ben.1), ("Cy", &p.cy.1)] {
        gs.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.clone(), role: String::new(), status: "online".into() });
    }
    gs
}

fn done(gs: &GuiState) {
    if let Some(s) = gs.dm_store.as_ref() {
        s.remove_file_for_test();
    }
    if let Some(b) = gs.block_list.as_ref() {
        b.remove_file_for_test();
    }
}

/// A message in the group by `from`, as its row carries it.
fn group_msg(from: &str, name: &str, ts: u64, text: &str) -> crate::gui::ChatMessage {
    crate::gui::ChatMessage {
        sender_key: from.into(),
        sender_name: name.into(),
        content: text.into(),
        timestamp_ms: ts,
        channel: format!("p2pgroup:{G}"),
        group_object_id: OBJ.into(),
        ..Default::default()
    }
}

fn group_part(gs: &GuiState) -> GroupTarget {
    gs.reports.dialog.as_ref().and_then(|d| d.group.clone()).expect("a group message's dialog")
}

/// THE DIALOG'S CHOICES: a group message's Report names the message by its signed-object id,
/// finds the creator, and offers the three with the creator first; only the admins when I created
/// the group (from the record, or from my own list when the record cannot be read), when the
/// person reported did, or when who created it cannot be found out. Seen red 2026-10-10 with the
/// fallback to my own list taken out of `creator_found`: "my list says I created it" got the
/// CREATOR_UNKNOWN line.
#[test]
fn a_group_messages_report_finds_the_creator_and_offers_the_creator_first() {
    let p = people(101);
    let mut gs = app(&p.ann, &p, "gr-dialog", false);
    gs.reports.group.creators.insert(G.into(), p.ben.1.clone()); // found earlier this session
    crate::engine::report::open_for_message(&mut gs, &group_msg(&p.cy.1, "Cy", 1_000, "you are all idiots"));
    let g = group_part(&gs);
    assert_eq!(g.item, Some(gr::Item { id: OBJ.into(), from: p.cy.1.clone(), ts: 1_000, text: "you are all idiots".into() }), "the message, by its signed-object id");
    assert_eq!((g.name.as_str(), g.finding, g.creator.as_deref()), ("Hikers", false, Some(p.ben.1.as_str())));
    let c = g.choices(&p.ann.1, &p.cy.1);
    assert_eq!((c.choices, g.destination(&p.ann.1, &p.cy.1)), (Dest::ALL.to_vec(), Some(Dest::Creator)), "all three, the creator by default");

    // Not known yet: the dialog waits, then settles on what the lookup said.
    gs.reports.group.creators.clear();
    crate::engine::report::open_for_message(&mut gs, &group_msg(&p.cy.1, "Cy", 1_000, "you are all idiots"));
    assert!(group_part(&gs).finding && group_part(&gs).destination(&p.ann.1, &p.cy.1).is_none(), "Send waits while finding");
    creator_found(&mut gs, G, Some(p.ben.1.clone()));
    assert_eq!(group_part(&gs).destination(&p.ann.1, &p.cy.1), Some(Dest::Creator));
    assert_eq!(gs.reports.group.creators.get(G), Some(&p.ben.1), "remembered: a group's creator never changes");
    creator_found(&mut gs, G, None);
    assert_eq!(group_part(&gs).choices(&p.ann.1, &p.cy.1).line, Some(gr::CREATOR_UNKNOWN), "cannot be found out: only the admins");

    // The person reported created the group.
    gs.reports.group.creators.insert(G.into(), p.ben.1.clone());
    crate::engine::report::open_for_message(&mut gs, &group_msg(&p.ben.1, "Ben", 2_000, "rules"));
    assert_eq!(group_part(&gs).choices(&p.ann.1, &p.ben.1).line, Some(gr::THEY_CREATED));
    done(&gs);

    // I created it: from the record, or from my own list when the record cannot be read.
    let mut ben = app(&p.ben, &p, "gr-dialog-ben", true);
    crate::engine::report::open_for_message(&mut ben, &group_msg(&p.cy.1, "Cy", 1_000, "you are all idiots"));
    creator_found(&mut ben, G, None);
    let g = group_part(&ben);
    assert_eq!((g.choices(&p.ben.1, &p.cy.1).line, g.destination(&p.ben.1, &p.cy.1)), (Some(gr::YOU_CREATED), Some(Dest::Admins)), "my list says I created it");
    // A group message with a file is no item (step D's rule).
    let att = dm_pq::DmAttachment { url: "u".into(), k: "k".into(), n: "n".into(), name: "a.jpg".into(), mime: "image/jpeg".into(), size: 1 };
    crate::engine::report::open_for_message(&mut ben, &group_msg(&p.cy.1, "Cy", 3_000, &dm_pq::build_file_marker(&att)));
    assert_eq!(group_part(&ben).item, None, "a message with a file is no item");
    done(&ben);
}

/// Open a `dm_put`'s envelope with the recipient's own key and check its signature.
fn open(put: &Value, seed: &[u8]) -> DmInner {
    let me = dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap();
    dm_pq::parse_verify_inner(&dm_pq::open_v2(&me, put["content"].as_str().unwrap()).unwrap()).unwrap()
}

/// WHAT SEND BUILDS: to the creator, ONE `dm_put` to them flagged `group_report`, sealed to them
/// alone (no self-copy, no report_v2), whose signed inner text is exactly the marker and the JSON;
/// Both builds both; the admins alone build report_v2 only. Nothing goes while the creator is being
/// found, and nothing is half built when the creator cannot be sealed for. Seen red 2026-10-10 with
/// the `group_report` flag left off the put in `creator_put`: "flagged group_report" failed.
#[test]
fn send_builds_one_sealed_flagged_dm_to_the_creator_with_exactly_the_marker() {
    let p = people(111);
    let mut gs = app(&p.ann, &p, "gr-send", false);
    gs.reports.group.creators.insert(G.into(), p.ben.1.clone());
    gs.peer_kyber_keys.insert(p.ben.1.clone(), kyber_of(&p.ben.0));
    crate::engine::report::open_for_message(&mut gs, &group_msg(&p.cy.1, "Cy", 1_000, "you are all idiots"));
    let d = gs.reports.dialog.as_mut().unwrap();
    d.reason = "harassment".into();
    d.note = "  please look  ".into();

    let out = prepare(&gs, 5_000).expect("built");
    let (creator, put) = out.to_creator.expect("to the creator");
    assert!(out.to_admins.is_none(), "no report_v2 when only the creator is chosen");
    assert_eq!(creator, p.ben.1);
    assert_eq!((put["type"].as_str(), put["to"].as_str()), (Some("dm_put"), Some(p.ben.1.as_str())), "one dm_put, to Ben");
    assert_eq!(put["group_report"], Value::Bool(true), "flagged group_report");
    let inner = open(&put, &p.ben.0);
    assert_eq!((inner.from.as_str(), inner.to.as_str()), (p.ann.1.as_str(), p.ben.1.as_str()), "signed by me, to Ben");
    let want = gr::build_text(&Report {
        group_id: G.into(),
        group_name: "Hikers".into(),
        target: p.cy.1.clone(),
        reason: "harassment".into(),
        note: "please look".into(),
        items: vec![gr::Item { id: OBJ.into(), from: p.cy.1.clone(), ts: 1_000, text: "you are all idiots".into() }],
    })
    .unwrap();
    assert_eq!(inner.text, want, "the sealed inner text is exactly the marker and the JSON");
    assert!(dm_pq::DmPqKeypair::from_bip39_seed(&p.ann.0).ok().and_then(|me| dm_pq::open_v2(&me, put["content"].as_str().unwrap()).ok()).is_none(), "sealed to Ben alone");

    // Both: both built. The admins alone: report_v2 only.
    gs.reports.dialog.as_mut().unwrap().group.as_mut().unwrap().send_to = Some(Dest::Both);
    let both = prepare(&gs, 5_000).unwrap();
    let frame: Value = serde_json::from_str(both.to_admins.as_deref().expect("and to the admins")).unwrap();
    assert!(both.to_creator.is_some() && frame["type"] == "report_v2" && frame["context"] == "group", "both");
    gs.reports.dialog.as_mut().unwrap().group.as_mut().unwrap().send_to = Some(Dest::Admins);
    let admins = prepare(&gs, 5_000).unwrap();
    assert!(admins.to_creator.is_none() && admins.to_admins.is_some(), "the admins alone");

    // No DM key for Ben yet: nothing is built, not even the admins' half of Both.
    gs.reports.dialog.as_mut().unwrap().group.as_mut().unwrap().send_to = Some(Dest::Both);
    gs.peer_kyber_keys.clear();
    assert_eq!(prepare(&gs, 5_000).err().as_deref(), Some(gr::CANNOT_SEAL), "never half a report");
    // While the creator is being found, nothing goes.
    gs.reports.dialog.as_mut().unwrap().group.as_mut().unwrap().finding = true;
    assert_eq!(prepare(&gs, 5_000).err().as_deref(), Some(gr::STILL_FINDING));
    done(&gs);
}

/// A `reach_refused` for the creator within a minute of a report to them is about the report: it
/// is said as such and is NOT turned into an offer of a contact request; later, or with no report
/// sent, it is an ordinary refusal. Seen red 2026-10-10 with `refused`'s call taken out of
/// engine/reach.rs: "the report did not reach the creator" failed.
#[test]
fn a_refusal_within_a_minute_says_the_report_did_not_get_through() {
    let p = people(121);
    let mut gs = app(&p.ann, &p, "gr-refused", false);
    let refusal = serde_json::json!({ "type": "reach_refused", "kind": "message", "to": p.ben.1 });
    note_sent(&mut gs, &p.ben.1);
    assert_eq!(gs.pending_notices.last().map(String::as_str), Some(gr::SENT_LINE), "Report sent to the group's creator.");
    crate::engine::reach::on_frame(&mut gs, &refusal);
    assert_eq!(gs.pending_notices.last().map(String::as_str), Some(gr::REFUSED_LINE), "the report did not reach the creator");
    assert!(!gs.reach.refused.contains_key(&p.ben.1), "and no contact request is offered for it");
    crate::engine::reach::on_frame(&mut gs, &refusal);
    assert!(gs.reach.refused.contains_key(&p.ben.1), "a second refusal is an ordinary one");
    gs.reach.refused.clear();
    gs.reports.group.sent.insert(p.ben.1.clone(), Instant::now().checked_sub(Duration::from_secs(61)).unwrap());
    crate::engine::reach::on_frame(&mut gs, &refusal);
    assert!(gs.reach.refused.contains_key(&p.ben.1), "and so is one more than a minute later");
    done(&gs);
}

/// A verified report DM from Ann, as it arrives in Ben's mailbox.
fn arriving(p: &People, report: &Report, ts: u64) -> DmInner {
    let inner = dm_pq::build_signed_inner(&p.ann.0, &p.ann.1, &p.ben.1, ts, &gr::build_text(report).unwrap()).unwrap();
    dm_pq::parse_verify_inner(&inner).unwrap()
}

fn report_about(p: &People, group_id: &str) -> Report {
    Report { group_id: group_id.into(), group_name: "Totally not Hikers".into(), target: p.cy.1.clone(), reason: "harassment".into(), note: "it keeps happening".into(), items: vec![] }
}

/// A REPORT ARRIVING at the creator: taken before the "show as a request" rule (Ann is not Ben's
/// friend, and under the safe defaults her ordinary DM would become a request), never stored or
/// shown as a message, waiting in the store for its check; then kept (announced once, by the names
/// the member list holds and the group's name from Ben's own list) or dropped. A parked server's
/// store keeps one waiting the same way. Seen red 2026-10-10 with the `ingest` call taken out of
/// `ingest_dm`: "not a request" failed (the report became one).
#[test]
fn a_report_arriving_waits_for_its_check_and_is_then_kept_or_dropped() {
    let p = people(131);
    let mut gs = app(&p.ben, &p, "gr-arrive", true);
    let inner = arriving(&p, &report_about(&p, G), 7_000);
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner), "never shown as a message");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.requests().is_empty(), "not a request, although Ann is not my friend");
    assert!(store.conversation(&p.ann.1).is_empty(), "never stored as a message");
    let waiting = store.pending_group_reports().to_vec();
    assert_eq!(waiting.len(), 1, "it waits for its check");

    // Its check: kept, said once.
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    let kept = gr::resolve(&p.ben.1, &waiting[0], &gs.p2p_groups, &[], &std::collections::HashMap::new()).unwrap();
    apply_check(&mut gs, "wss://another.example", &waiting[0].id, CheckOutcome::Kept(kept.clone()));
    assert_eq!(gs.dm_store.as_ref().unwrap().pending_group_reports().len(), 1, "an answer for another server changes nothing here");
    apply_check(&mut gs, &server, &waiting[0].id, CheckOutcome::Kept(kept));
    assert_eq!(
        gs.pending_notices.last().map(String::as_str),
        Some("Ann sent you a report about Cy in Hikers. It is in Safety, under Reports about your groups.")
    );
    assert_eq!(count_for(&gs, G), 1, "a count of 1 on the group");
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner), "handed over again: still never a message");
    assert!(gs.dm_store.as_ref().unwrap().pending_group_reports().is_empty(), "and kept once, not waiting again");

    // One about a group Ben does not hold: dropped by its check, and nothing said.
    let other = arriving(&p, &report_about(&p, &"cd".repeat(32)), 8_000);
    crate::engine::dm::ingest_dm(&mut gs, &other);
    let id = gs.dm_store.as_ref().unwrap().pending_group_reports()[0].id.clone();
    let said = gs.pending_notices.len();
    apply_check(&mut gs, &server, &id, CheckOutcome::Dropped(gr::Why::NotHeld));
    assert!(gs.dm_store.as_ref().unwrap().pending_group_reports().is_empty() && gs.pending_notices.len() == said, "dropped quietly");
    assert_eq!(gs.dm_store.as_ref().unwrap().group_reports().len(), 1);

    // On a parked server's store it waits there too.
    let mut parked = DmStore::load(&p.ben.0, &p.ben.1, &format!("{server}-parked"));
    assert!(take_into(&mut parked, &p.ben.1, &inner) && parked.pending_group_reports().len() == 1);
    // My own report, echoed from another device: nothing to keep.
    let mine = dm_pq::parse_verify_inner(&dm_pq::build_signed_inner(&p.ben.0, &p.ben.1, &p.ann.1, 9, &gr::build_text(&report_about(&p, G)).unwrap()).unwrap()).unwrap();
    assert!(!take_into(&mut parked, &p.ben.1, &mine));
    done(&gs);
}

/// THE CREATOR'S THREE ACTIONS on a kept report about Cy: Remove (a key that cannot be made
/// removes no one and says so; done, the report says so and an open view of the group sends
/// under the new key), Block them (Cy, never the reporter; the report stays), Dismiss (gone from
/// this device, and the count off the group). Seen red 2026-10-10 twice: with `block_them`
/// blocking the report's sender, "Cy is blocked" failed; with `dismiss` leaving the report in the
/// store, "gone from this device" failed.
#[test]
fn the_creators_three_actions_remove_block_and_dismiss() {
    let p = people(141);
    let mut gs = app(&p.ben, &p, "gr-actions", true);
    // The report as kept (arriving is the test above): straight into the store.
    let inner = arriving(&p, &report_about(&p, G), 7_000);
    assert!(take_into(gs.dm_store.as_mut().unwrap(), &p.ben.1, &inner));
    let waiting = gs.dm_store.as_ref().unwrap().pending_group_reports()[0].clone();
    let kept = gr::resolve(&p.ben.1, &waiting, &gs.p2p_groups, &[], &std::collections::HashMap::new()).unwrap();
    let server = crate::gui::pages::chat::norm_server_url(&gs.server_url);
    apply_check(&mut gs, &server, &waiting.id, CheckOutcome::Kept(kept));
    let id = waiting.id.clone();

    removal_done(&mut gs, &id, Err(RemoveError::NoKey("HTTP 502".into())));
    assert_eq!(gs.pending_notices.last().map(String::as_str), Some("Could not remove Cy from Hikers: a new group key could not be made."));
    assert!(!gs.dm_store.as_ref().unwrap().group_report(&id).unwrap().removed, "no one removed");

    gs.p2p_group_active_id = G.into();
    let new_key = vec![9u8; 32];
    removal_done(&mut gs, &id, Ok((2, new_key.clone())));
    assert_eq!(gs.pending_notices.last().map(String::as_str), Some("Cy was removed from Hikers."));
    assert!(gs.dm_store.as_ref().unwrap().group_report(&id).unwrap().removed, "Removed from the group.");
    assert_eq!((gs.p2p_group_chat_epoch, gs.p2p_group_chat_epoch_key.as_deref()), (2, Some(new_key.as_slice())), "the open group sends under the new key");

    block_them(&mut gs, &id);
    assert!(crate::engine::block::is_blocked(&gs, &p.cy.1), "Cy is blocked");
    assert!(!crate::engine::block::is_blocked(&gs, &p.ann.1), "never the one who reported");
    assert!(gs.dm_store.as_ref().unwrap().group_report(&id).is_some(), "the report stays until dismissed");

    gs.reports.group.confirm_remove = Some(id.clone());
    assert!(dismiss(&mut gs, &id));
    assert!(gs.dm_store.as_ref().unwrap().group_reports().is_empty(), "gone from this device");
    assert_eq!((count_for(&gs, G), gs.reports.group.confirm_remove.as_deref()), (0, None), "and the count off the group");
    done(&gs);
}
