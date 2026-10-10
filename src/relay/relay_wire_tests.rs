// Child of relay.rs (#[path], so `use super::*` reaches everything it has, private items
// included): the wire shapes of RelayMessage, and how the send loop treats a socket that fell
// behind. Moved out unchanged under the file-size ratchet on 2026-10-09
// (tests/file_size_ratchet.rs), from the modules `channel_update_wire_tests` and
// `broadcast_lag_tests`, to make room for "who can reach me" (handlers/reach.rs).

use super::*;

// ── channel_update on the wire ──

// Guards the wire contract the native chat channel-edit modal now relies on.
// The modal used to send `{"type":"channel_edit", ...}` — a type the relay
// never had a handler for — so name, description, AND the voice/read-only
// toggles were all silently dropped on Save (operator: "I disabled voice on
// #announcements but it came back"). It now sends `channel_update`, which
// MUST deserialize into RelayMessage::ChannelUpdate with the flags set.
#[test]
fn channel_update_deserializes_with_flags() {
    let json = r#"{
        "type": "channel_update",
        "channel_id": "announcements",
        "name": "announcements",
        "description": "Project updates and news",
        "read_only": true,
        "voice_enabled": false
    }"#;
    let msg: RelayMessage = serde_json::from_str(json).expect("channel_update must parse");
    match msg {
        RelayMessage::ChannelUpdate { channel_id, name, description, read_only, voice_enabled, federated } => {
            assert_eq!(channel_id, "announcements");
            assert_eq!(name.as_deref(), Some("announcements"));
            assert_eq!(description.as_deref(), Some("Project updates and news"));
            assert_eq!(read_only, Some(true));
            assert_eq!(voice_enabled, Some(false));
            // Omitted fields stay None so the relay leaves them unchanged.
            assert_eq!(federated, None);
        }
        other => panic!("expected ChannelUpdate, got {other:?}"),
    }
}

// The legacy `channel_edit` type must NOT map to ChannelUpdate — that's
// precisely why the old modal was a silent no-op for channel flags.
#[test]
fn legacy_channel_edit_is_not_a_channel_update() {
    let json = r#"{"type":"channel_edit","channel_id":"x","name":"y","description":"z"}"#;
    if let Ok(RelayMessage::ChannelUpdate { .. }) = serde_json::from_str::<RelayMessage>(json) {
        panic!("channel_edit must not deserialize as ChannelUpdate");
    }
}

// ── "Who can reach me" on the wire (handlers/reach.rs) ──

/// The exact shapes of docs/design/blocking-and-safe-mode.md 10c, which both clients are built
/// against: `reach_settings` carries only `settings` with the three kinds (the key it is routed
/// by is not sent), `reach_refused` carries `kind` and `to` (the person refused to the sender,
/// whose own key routes it and is not sent), and `dm_put` reads `contact_request`, false when
/// absent.
///
/// Seen red 2026-10-09 with the `#[serde(skip)]` taken off `ReachRefused::sender`: "the refusal
/// carries exactly type, kind and to" (it carried the sender's own key as well).
#[test]
fn reach_messages_have_the_exact_shapes_the_clients_read() {
    use crate::relay::handlers::reach::ReachSettings;
    let settings = RelayMessage::ReachSettings {
        to: "me".into(),
        settings: ReachSettings { message: "friends".into(), call: "chosen".into(), trade: "friends".into() },
    };
    assert_eq!(
        serde_json::to_value(&settings).unwrap(),
        serde_json::json!({ "type": "reach_settings", "settings": { "message": "friends", "call": "chosen", "trade": "friends" } }),
        "the settings carry exactly type and settings"
    );
    let refused = RelayMessage::ReachRefused { sender: "me".into(), kind: "message".into(), to: "them".into() };
    assert_eq!(
        serde_json::to_value(&refused).unwrap(),
        serde_json::json!({ "type": "reach_refused", "kind": "message", "to": "them" }),
        "the refusal carries exactly type, kind and to"
    );
    let put = |extra: serde_json::Value| -> bool {
        let mut v = serde_json::json!({ "type": "dm_put", "to": "them", "content": "{}" });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        match serde_json::from_value::<RelayMessage>(v).expect("dm_put parses") {
            RelayMessage::DmPut { contact_request, .. } => contact_request,
            other => panic!("expected DmPut, got {other:?}"),
        }
    };
    assert!(put(serde_json::json!({ "contact_request": true })), "a contact request is read");
    assert!(!put(serde_json::json!({})), "and an ordinary dm_put is not one");
}

// ── A socket that fell behind the broadcast ring ──

fn sys(n: usize) -> RelayMessage {
    RelayMessage::System { message: format!("m{n}") }
}

/// The setup must genuinely overflow the ring buffer, or the test below would
/// pass against the very bug it exists to catch. This asserts the channel
/// that must move: a raw `recv()` on this receiver really does report
/// `Lagged`, which is the exact error the old `while let Ok(..)` swallowed as
/// a disconnect.
#[tokio::test]
async fn the_setup_really_does_lag() {
    let (tx, mut rx) = broadcast::channel::<RelayMessage>(2);
    for i in 0..5 {
        tx.send(sys(i)).unwrap();
    }
    match rx.recv().await {
        Err(broadcast::error::RecvError::Lagged(skipped)) => {
            assert_eq!(skipped, 3, "5 sends into 2 slots should drop exactly 3");
        }
        other => panic!("expected Lagged, got {:?}", other.is_ok()),
    }
}

/// The fix: a socket that fell behind keeps its connection and resumes at the
/// present. Against the previous inline `while let Ok(msg) = rx.recv().await`
/// this is the failing case -- that form yields no message and ends the loop,
/// which tore down the whole WebSocket and evicted the player from the game
/// world and chat alike.
#[tokio::test]
async fn a_lagged_socket_skips_ahead_instead_of_disconnecting() {
    let (tx, mut rx) = broadcast::channel::<RelayMessage>(2);
    for i in 0..5 {
        tx.send(sys(i)).unwrap();
    }
    let got = recv_skipping_lag(&mut rx).await;
    let Some(RelayMessage::System { message }) = got else {
        panic!("a lagged socket must keep receiving, not be dropped");
    };
    // Oldest two survive in a 2-slot ring, so the resume point is m3.
    assert_eq!(message, "m3", "should resume at the present, not replay stale frames");

    // And it keeps working afterwards: lag is not a terminal state.
    tx.send(sys(99)).unwrap();
    let Some(RelayMessage::System { message }) = recv_skipping_lag(&mut rx).await else {
        panic!("the receiver must stay usable after skipping");
    };
    assert_eq!(message, "m4");
}

/// A CLOSED channel is the one case that SHOULD end the forwarding loop:
/// the relay is shutting down and there is nothing left to send.
#[tokio::test]
async fn a_closed_channel_still_ends_the_loop() {
    let (tx, mut rx) = broadcast::channel::<RelayMessage>(2);
    drop(tx);
    assert!(
        recv_skipping_lag(&mut rx).await.is_none(),
        "a closed channel must end the loop, or shutdown would spin forever"
    );
}

// ── Reports on the wire (handlers/reports.rs) ──

/// The exact shapes of docs/design/blocking-and-safe-mode.md 10e that the relay sends:
/// `report_received` carries only `id`, and `reports` only `state` and `items`; the key each is
/// routed by is never sent (a list's evidence is readable text, so a `to` that leaked onto the
/// wire would also be a routing field someone could forget).
///
/// Seen red 2026-10-09 with the `#[serde(skip)]` taken off `Reports::to`: "the list carries
/// exactly type, state and items" (it carried `to` as well).
#[test]
fn report_messages_have_the_exact_shapes_the_clients_read() {
    let received = RelayMessage::ReportReceived { to: "me".into(), id: 7 };
    assert_eq!(
        serde_json::to_value(&received).unwrap(),
        serde_json::json!({ "type": "report_received", "id": 7 }),
        "the receipt carries exactly type and id"
    );
    let list = RelayMessage::Reports { to: "me".into(), state: "open".into(), items: vec![serde_json::json!({ "id": 7 })] };
    assert_eq!(
        serde_json::to_value(&list).unwrap(),
        serde_json::json!({ "type": "reports", "state": "open", "items": [{ "id": 7 }] }),
        "the list carries exactly type, state and items"
    );
}
