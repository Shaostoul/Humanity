// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has): a person's
// notification switches as the relay changes them, from the message on the wire to the stored
// row and the answer every one of that person's devices receives (2026-10-30, the seams review
// of the notifications batch).
//
// THE FAULT IT GUARDS: `update_notification_prefs` used to REPLACE all five stored values, a
// missing switch arriving as on and missing quiet hours as none. The website's DM cog sent only
// what that browser happened to remember, so one click in a browser that had never opened
// Settings wiped the person's quiet hours and turned mentions back on, on every device. Now only
// the fields a message carries change: an absent one keeps what the server holds, and quiet hours
// sent as null clear them (the desktop sends all five, with null for none).
//
// Seen red 2026-10-30 against the old handler (bool fields defaulting to true, `Option<String>`
// quiet hours): "a DM-only change keeps mentions off" failed (mentions came back on), and
// "absent: kept" failed with the quiet hours None after a tasks-only change.

use super::*;
use crate::relay::relay::RelayState;

fn fresh_state() -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp("notif_prefs")))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// The message as a client sends it, through the relay's own parse and handler.
fn send(state: &Arc<RelayState>, key: &str, frame: serde_json::Value) {
    let msg: RelayMessage = serde_json::from_value(frame).expect("update_notification_prefs parses");
    let RelayMessage::UpdateNotificationPrefs { dm, mentions, tasks, dnd_start, dnd_end, quiet_friends, quiet_mentions } = msg else {
        panic!("not an update_notification_prefs");
    };
    let change = crate::relay::storage::NotifPrefsChange { dm, mentions, tasks, dnd_start, dnd_end, quiet_friends, quiet_mentions };
    block(handle_update_notification_prefs(state, key, change));
}

/// The last `notification_prefs_data` on the broadcast channel.
fn last_answer(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Option<RelayMessage> {
    let mut last = None;
    while let Ok(m) = rx.try_recv() {
        last = Some(m);
    }
    last
}

/// The quiet-hours exceptions (notifications rung 2, 2026-10-31) travel both ways: a client sends
/// just the one it changed, every device is told both, and a change of something else keeps them.
/// Seen red 2026-10-31 with the answer (`prefs_data`) sending `quiet_friends: false` whatever was
/// stored: "a DM-only change keeps the friends exception" failed.
#[test]
fn the_quiet_hours_exceptions_travel_and_are_kept() {
    let st = fresh_state();
    let mut answers = st.broadcast_tx.subscribe();
    send(&st, "k1", serde_json::json!({ "type": "update_notification_prefs", "quiet_friends": true }));
    send(&st, "k1", serde_json::json!({ "type": "update_notification_prefs", "dm": false }));
    match last_answer(&mut answers) {
        Some(RelayMessage::NotificationPrefsData { dm, quiet_friends, quiet_mentions, .. }) => {
            assert!(!dm);
            assert!(quiet_friends, "a DM-only change keeps the friends exception");
            assert!(!quiet_mentions, "off until set");
        }
        other => panic!("expected the stored prefs back, got {other:?}"),
    }
    // On the wire the answer always names both, so a client never guesses.
    block(handle_get_notification_prefs(&st, "k2"));
    let wire = serde_json::to_value(last_answer(&mut answers).expect("an answer")).unwrap();
    assert_eq!((wire["quiet_friends"].as_bool(), wire["quiet_mentions"].as_bool()), (Some(false), Some(false)), "{wire}");
}

#[test]
fn a_change_touches_only_the_switches_it_names() {
    let st = fresh_state();
    let mut answers = st.broadcast_tx.subscribe();
    // What the person chose on their desktop: DMs on, mentions off, quiet 22:00 to 07:00.
    send(&st, "k1", serde_json::json!({
        "type": "update_notification_prefs", "dm": true, "mentions": false, "tasks": true,
        "dnd_start": "22:00", "dnd_end": "07:00",
    }));
    // The website's DM cog: only the DM switch.
    send(&st, "k1", serde_json::json!({ "type": "update_notification_prefs", "dm": false }));
    let p = st.db.get_notification_prefs("k1").expect("stored");
    assert!(!p.dm_enabled, "the DM switch changed");
    assert!(!p.mentions_enabled, "a DM-only change keeps mentions off");
    assert!(p.tasks_enabled, "and tasks as they were");
    assert_eq!((p.dnd_start.as_deref(), p.dnd_end.as_deref()), (Some("22:00"), Some("07:00")), "and the quiet hours");

    // Every device of theirs is told the whole stored set, not the half that was sent.
    match last_answer(&mut answers) {
        Some(RelayMessage::NotificationPrefsData { dm, mentions, tasks, dnd_start, dnd_end, target, .. }) => {
            assert_eq!((dm, mentions, tasks), (false, false, true));
            assert_eq!((dnd_start.as_deref(), dnd_end.as_deref()), (Some("22:00"), Some("07:00")));
            assert_eq!(target.as_deref(), Some("k1"));
        }
        other => panic!("expected the stored prefs back, got {other:?}"),
    }
}

#[test]
fn quiet_hours_sent_as_null_are_cleared_and_absent_ones_are_kept() {
    let st = fresh_state();
    send(&st, "k1", serde_json::json!({
        "type": "update_notification_prefs", "dm": true, "mentions": true, "tasks": true,
        "dnd_start": "22:00", "dnd_end": "07:00",
    }));
    send(&st, "k1", serde_json::json!({ "type": "update_notification_prefs", "tasks": false }));
    assert_eq!(st.db.get_notification_prefs("k1").unwrap().dnd_start.as_deref(), Some("22:00"), "absent: kept");
    // The desktop's frame with quiet hours off (settings/notifications.rs `synced_frame`).
    send(&st, "k1", serde_json::json!({
        "type": "update_notification_prefs", "dm": true, "mentions": true, "tasks": false,
        "dnd_start": null, "dnd_end": null,
    }));
    let p = st.db.get_notification_prefs("k1").unwrap();
    assert_eq!((p.dnd_start, p.dnd_end), (None, None), "null: cleared");
    assert!(!p.tasks_enabled);
    // Someone who never saved anything starts from the defaults (every switch on, no quiet hours).
    send(&st, "k2", serde_json::json!({ "type": "update_notification_prefs", "mentions": false }));
    let q = st.db.get_notification_prefs("k2").unwrap();
    assert!(q.dm_enabled && !q.mentions_enabled && q.tasks_enabled && q.dnd_start.is_none());
}
