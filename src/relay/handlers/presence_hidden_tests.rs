// Child of live_conns.rs (#[path]). BUG-278 (2026-11-07): a presence-hidden person (every new
// member, and anyone who turned off "Show others when I am online") is never named as online
// anywhere outside the server: not by the public `GET /api/peers`, not by a "joined" announcement,
// and not by the peer list broadcast to every socket after a role or status change.
//
// Seen red on 2026-11-07, each against the code before its fix (the fix reverted in the working
// tree, the test run, the fix put back):
// - `api_peers_leaves_out_presence_hidden_people`: get_peers listed the hidden key with
//   status "online" ("the hidden member is not listed" failed).
// - `a_new_member_joining_hidden_is_not_announced`: join_as_member sent MemberJoined with the
//   newcomer's key and name to every socket.
// - `a_broadcast_peer_list_names_hidden_people_only_to_themselves`: with `for_socket` passing
//   every message unchanged (the send loop before the fix), the visible member's copy named the
//   hidden one.

use super::*;
use crate::relay::relay::{Peer, RelayMessage, RelayState};
use crate::relay::storage::Storage;
use std::sync::Arc;

fn fresh_state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("presence_hidden_{tag}"))))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// `key` signed in, with a name, the way a socket that finished identify is.
fn connect(st: &Arc<RelayState>, key: &str) {
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer { public_key_hex: key.to_string(), display_name: Some(key.to_string()), upload_token: None, conn_id: 1 },
        );
    });
}

/// A member keeping the default: hidden.
fn hidden_member(st: &Arc<RelayState>, key: &str) {
    st.db.join_server(key, key).unwrap();
    assert!(st.db.presence_hidden(key), "precondition: new members join hidden");
}

/// A member who chose to show that they are online.
fn visible_member(st: &Arc<RelayState>, key: &str) {
    st.db.join_server(key, key).unwrap();
    st.db.set_hide_presence(key, false).unwrap();
}

fn keys(peers: &[crate::relay::relay::PeerInfo]) -> Vec<String> {
    let mut k: Vec<String> = peers.iter().map(|p| p.public_key.clone()).collect();
    k.sort();
    k
}

#[test]
fn api_peers_leaves_out_presence_hidden_people() {
    let st = fresh_state("api");
    hidden_member(&st, "hidden_key");
    visible_member(&st, "shown_key");
    connect(&st, "hidden_key");
    connect(&st, "shown_key");
    let axum::Json(list) = block(crate::relay::api::get_peers(axum::extract::State(st.clone())));
    let listed = keys(&list);
    assert!(listed.contains(&"shown_key".to_string()), "a member who shows their presence is listed: {listed:?}");
    assert!(!listed.contains(&"hidden_key".to_string()), "the hidden member is not listed: {listed:?}");
}

#[test]
fn a_new_member_joining_hidden_is_not_announced() {
    let st = fresh_state("join");
    let mut rx = st.broadcast_tx.subscribe();
    assert!(crate::relay::handlers::sign_ups::join_as_member(&st, "newbie_key", &Some("Newbie".to_string())));
    assert!(st.db.is_member("newbie_key") && st.db.presence_hidden("newbie_key"), "joined, hidden");
    while let Ok(msg) = rx.try_recv() {
        assert!(
            !matches!(msg, RelayMessage::MemberJoined { .. }),
            "a hidden newcomer's joining is not sent to every socket: {msg:?}"
        );
    }
}

#[test]
fn a_broadcast_peer_list_names_hidden_people_only_to_themselves() {
    let st = fresh_state("broadcast");
    hidden_member(&st, "hidden_key");
    visible_member(&st, "shown_key");
    connect(&st, "hidden_key");
    connect(&st, "shown_key");
    let mut rx = st.broadcast_tx.subscribe();
    block(crate::relay::handlers::broadcast::broadcast_peer_list(&st));
    let msg = rx.try_recv().expect("the peer list went on the bus");

    let seen = |me: &str| match for_socket(msg.clone(), me) {
        RelayMessage::PeerList { peers, .. } => keys(&peers),
        other => panic!("still a peer list: {other:?}"),
    };
    assert_eq!(seen("shown_key"), vec!["shown_key".to_string()], "another socket's copy leaves the hidden member out");
    assert_eq!(
        seen("hidden_key"),
        vec!["hidden_key".to_string(), "shown_key".to_string()],
        "the hidden member's own copy keeps them (their role and status come from it)"
    );
    // The list of hidden keys itself never goes on the wire, even uncut.
    let wire = serde_json::to_string(&msg).unwrap();
    assert!(!wire.contains("hidden_keys"), "the list of who hides is never sent: {wire}");
}
