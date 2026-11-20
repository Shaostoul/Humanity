// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has).
//
// VOICE ROOM NAMES AT THE RELAY (BUG-331, 2026-11-20). An admin or mod names a voice room; the
// relay stored and broadcast the name as typed, with no length or character limit, and the web
// chat wrote it into markup unescaped ("Connected to: " + name), where a data-on-error attribute
// in it ran as script through safe-markup.js. The web chat now escapes it; the relay also refuses
// a name that is empty, longer than 48 characters, or holds a control character or a
// text-direction override (refused with a message, never silently rewritten).
//
// Seen red 2026-11-20 with voice_room_name_problem a stub that refused nothing: "a name with a
// control character or longer than 48 characters is refused: \"Lobby\u{7}\" was stored", the
// rename stored it too ("renaming to \"Lobby\u{7}\" is refused"), and 49 two-byte letters passed.

use super::*;
use crate::relay::relay::RelayState;

fn fresh_state() -> Arc<RelayState> {
    let db = Storage::open_temp("voicename");
    Arc::new(RelayState::new(db))
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(RelayMessage::Private { message, .. }) => out.push(message),
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(_) => break,
        }
    }
    out
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

fn names(st: &Arc<RelayState>) -> Vec<String> {
    st.db.list_voice_channels().unwrap().into_iter().map(|c| c.name).collect()
}

const BAD: &[&str] = &[
    "Lobby\u{7}",
    "a\nb",
    "tab\there",
    "\u{202E}moorb",
    "x\u{2066}y",
    "",
    "   ",
];

#[test]
fn a_voice_room_name_with_a_control_character_or_too_long_is_refused() {
    let st = fresh_state();
    st.db.set_role("admin_key", "admin").unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    let long = "x".repeat(49);
    for bad in BAD.iter().copied().chain(std::iter::once(long.as_str())) {
        block(handle_voice_room(&st, "admin_key", 1, "create".into(), None, Some(bad.to_string())));
        assert!(names(&st).is_empty(), "a name with a control character or longer than 48 characters is refused: {bad:?} was stored");
        let msgs = drain(&mut rx);
        assert!(msgs.iter().any(|m| m.contains("Voice channel name")), "the admin is told why, for {bad:?}: {msgs:?}");
    }
    // 48 characters, markup characters and letters beyond ASCII are fine: the page escapes them.
    let ok = format!("{}<3 Café", "y".repeat(41));
    assert_eq!(ok.chars().count(), 48);
    block(handle_voice_room(&st, "admin_key", 1, "create".into(), None, Some(ok.clone())));
    assert_eq!(names(&st), vec![ok]);
}

#[test]
fn a_voice_room_cannot_be_renamed_to_a_bad_name() {
    let st = fresh_state();
    st.db.set_role("mod_key", "mod").unwrap();
    block(handle_voice_room(&st, "mod_key", 1, "create".into(), None, Some("Lobby".into())));
    let id = st.db.list_voice_channels().unwrap()[0].id.to_string();
    let mut rx = st.broadcast_tx.subscribe();
    for bad in BAD {
        block(handle_voice_room(&st, "mod_key", 1, "rename".into(), Some(id.clone()), Some(bad.to_string())));
        assert_eq!(names(&st), vec!["Lobby".to_string()], "renaming to {bad:?} is refused");
        assert!(drain(&mut rx).iter().any(|m| m.contains("Voice channel name")), "the mod is told why, for {bad:?}");
    }
    block(handle_voice_room(&st, "mod_key", 1, "rename".into(), Some(id), Some("Quiet room".into())));
    assert_eq!(names(&st), vec!["Quiet room".to_string()]);
}

#[test]
fn the_name_rule_reads_characters_not_bytes() {
    assert_eq!(voice_room_name_problem(&"é".repeat(48)), None, "48 two-byte letters are 48 characters");
    assert!(voice_room_name_problem(&"é".repeat(49)).is_some());
    assert!(voice_room_name_problem("ok\u{200F}").is_some(), "a right-to-left mark is a direction control");
    assert_eq!(voice_room_name_problem("General voice"), None);
}
