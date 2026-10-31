//! From engine/reach.rs's tests (the GUI's reach rules): a ring our call setting refuses rings
//! nothing, through the engine's call handling (`call_relay::on_ring`).

use crate::engine::reach::*;
use crate::engine::reach::tests::app;
use crate::engine::reach::tests::identity;
use crate::engine::reach::tests::inner;
use crate::gui::GuiState;
use crate::net::dm_store::DmStore;
use crate::net::dm_store::SentPass;
use crate::net::reach::Audience;
use crate::net::reach::ReachSettings;

/// Ring an idle app as `who`; true when it rang.
fn rings(gs: &mut GuiState, who: &str) -> bool {
    gs.call_incoming = None;
    crate::engine::call_relay::on_ring(gs, who.to_string(), "Someone".into());
    gs.call_incoming.is_some()
}

/// A ring our own call setting would not let through is ignored here too (the relay checks
/// first; this is for a server that passes one on anyway): under the safe defaults (calls from
/// People I choose) a stranger's ring, and a friend's whose pass does not name calls, ring
/// nothing, leave no line and send nothing back; a friend whose pass names calls rings, and so
/// does one whose pass went unanswered (10l: the relay honours it if it stored it), whose DMs
/// are DMs too, not requests. Under Anyone a stranger rings.
/// Seen red 2026-10-10 two ways: without the check in `on_ring`, "a stranger's ring rings
/// nothing"; with `passes_held_by` reading the record only, "and so does one whose pass went
/// unanswered".
#[test]
fn rings_our_call_setting_refuses_are_ignored() {
    let (my_seed, me) = identity(94);
    let (_s, stranger) = identity(95);
    let (_f, friend) = identity(96);
    let (_c, caller) = identity(97);
    let (_u, unsure) = identity(98);
    let mut gs = app(&me, &my_seed, "reach-rings");
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    let store = gs.dm_store.as_mut().unwrap();
    store.record_pass_sent(&friend, SentPass { serial: "ab".repeat(16), may: "invite,message,trade,voice_message".into() });
    store.record_pass_sent(&caller, SentPass { serial: "cd".repeat(16), may: "call,invite,message,trade,voice_message".into() });
    store.pass_on_its_way(&unsure, SentPass { serial: "ef".repeat(16), may: "call,invite,message,trade,voice_message".into() });

    assert!(!rings(&mut gs, &stranger), "a stranger's ring rings nothing under the defaults");
    assert!(!rings(&mut gs, &friend), "nor a friend's whose pass does not name calls");
    assert!(gs.pending_notices.is_empty(), "an ignored ring leaves no line");
    assert!(sent.try_recv().is_err(), "and sends nothing back");
    assert!(rings(&mut gs, &caller), "a friend whose pass names calls rings");
    assert!(rings(&mut gs, &unsure), "and so does one whose pass went unanswered");
    assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&unsure, &me, "hello")), "whose DM is a DM, not a request");

    gs.dm_store.as_mut().unwrap().set_reach_settings(ReachSettings { call: Audience::Anyone, ..Default::default() });
    assert!(rings(&mut gs, &stranger), "under Anyone a stranger rings");
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
}

/// 10m R8: GROUP MEMBERSHIP IN THIS APP'S OWN CHECKS IS THE SERVER'S CALL. Under "Friends and
/// people in my groups" someone the server let a DM or a ring through from counts as sharing
/// a group: this app's own group list goes stale (refreshed only while Chat is open), and
/// here it has loaded empty, as a stale one is when Gus has just joined. His DM is a DM, not a
/// request, on the active server and on a parked server's store, and his ring rings. Under
/// Friends nothing changes: the same DM is a request.
/// Seen red 2026-10-10 two ways: with `file_if_refused_in` given the stale list's answer (no
/// group) instead of the server's, "a DM the server let through under Groups is a DM" failed;
/// and with `on_ring` asking `shares_group` (the stale list) again, "and his ring rings".
#[test]
fn group_membership_is_the_servers_call() {
    let (my_seed, me) = identity(84);
    let (_g, gus) = identity(85);
    let mut gs = app(&me, &my_seed, "reach-groups");
    gs.p2p_groups_last_fetch = Some(std::time::Instant::now());
    assert!(!shares_group(&gs, &gus), "the setup: this app's own list knows no group with Gus");
    let groups = ReachSettings { message: Audience::Groups, call: Audience::Groups, ..Default::default() };
    gs.dm_store.as_mut().unwrap().set_reach_settings(groups);
    assert!(crate::engine::dm::ingest_dm(&mut gs, &inner(&gus, &me, "see you at the garden")), "a DM the server let through under Groups is a DM");
    assert!(rings(&mut gs, &gus), "and his ring rings");
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut parked = DmStore::load(&my_seed, &me, &format!("wss://reach-parked-{nanos}.example"));
    parked.set_reach_settings(groups);
    assert!(!file_if_refused_in(&mut parked, &inner(&gus, &me, "on another server")), "and on a parked server's store");

    gs.dm_store.as_mut().unwrap().set_reach_settings(ReachSettings::default());
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &inner(&gus, &me, "under Friends")), "under Friends it is a request");
    assert!(gs.dm_store.as_ref().unwrap().requests().iter().any(|r| r.key == gus));
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    parked.remove_file_for_test();
}
