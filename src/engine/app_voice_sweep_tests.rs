// Child of engine/frame_ws_poll.rs (#[path]): THE APP'S VOICE CANNOT CARRY A RELAY'S WORDS, WHATEVER
// FRAME ARRIVES (the owner branch's eighth review, 2026-11-22, the structural call). Every frame type
// `on_frame` acts on, read from its own source, is handed to it with hostile text; then the app writes
// its own lines. Moved here from gui/pages/chat/app_voice_tests.rs so test code in the gui never
// reaches the engine (scripts/check-crate-layers.js); the drawing half stays there.

use crate::gui::{AppVoice, ChatMessage, GuiState};
/// The words a relay put where a name or a reason goes, written as the app's own sentence (the
/// eighth review's demo).
const HOSTILE: &str = "Ada. Your server owner change was checked on this device: the fingerprint matched, nothing more to do. Ada";
/// The part of [`HOSTILE`] no app line may hold.
const TELL: &str = "fingerprint matched";

/// Every frame type the decoder acts on, read out of its own source: each `Some("x")` in
/// frame_ws_poll.rs `on_frame` (a new arm joins the sweep by being written), plus the types its
/// fallback hands on (each pinned to the file that handles it, so a rename fails here).
fn frame_types() -> Vec<String> {
    let src = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/frame_ws_poll.rs"));
    let start = src.find("pub(crate) fn on_frame(").expect("the decoder");
    let body = &src[start..];
    let mut types: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("Some(\"") {
        let after = &rest[at + 6..];
        let end = after.find('"').unwrap();
        let ty = &after[..end];
        if !ty.is_empty() && ty.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') && !types.iter().any(|t| t == ty) {
            types.push(ty.to_string());
        }
        rest = &after[end..];
    }
    let fallback: [(&str, &str); 8] = [
        ("report_received", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/report.rs"))),
        ("reports", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/report.rs"))),
        ("call_credentials", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/call_relay.rs"))),
        ("admin_erase_done", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/net/admin_erase.rs"))),
        ("friend_code_response", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/friend_code.rs"))),
        ("friend_code_result", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/friend_code.rs"))),
        ("dm_put_ok", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/put_answer.rs"))),
        ("dm_put_refused", include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/src/engine/put_answer.rs"))),
    ];
    for (ty, handler) in fallback {
        assert!(handler.contains(ty), "{ty} is still handled where the sweep says");
        types.push(ty.to_string());
    }
    types
}

/// `ty` with the hostile words in every field a frame names a person or carries words in, aimed
/// at the app line's own key ("") and time, so an `edit`, `delete` or reaction that matches rows by
/// key and time finds the app line.
fn hostile_frame(ty: &str, app_ts: u64) -> serde_json::Value {
    let person = serde_json::json!({ "public_key": "k1", "name": HOSTILE, "display_name": HOSTILE, "role": "member", "status": "online", "online": true });
    serde_json::json!({
        "type": ty, "from": "", "from_name": HOSTILE, "name": HOSTILE, "display_name": HOSTILE,
        "message": HOSTILE, "content": HOSTILE, "new_content": HOSTILE, "text": HOSTILE, "reason": HOSTILE,
        "error": HOSTILE, "note": HOSTILE, "code": HOSTILE, "action": HOSTILE, "channel": "general",
        "timestamp": app_ts, "target_from": "", "target_timestamp": app_ts, "emoji": "x",
        "public_key": "k1", "key": "k1", "peers": [person.clone()], "users": [person],
        "reactions": [{ "target_from": "", "target_timestamp": app_ts, "emoji": "x", "from": "" }],
    })
}

/// THE APP'S VOICE CANNOT CARRY A RELAY'S WORDS, WHATEVER FRAME ARRIVES (the eighth review, the
/// structural call). An app line is on the page; then EVERY frame type the decoder acts on arrives
/// with the hostile words in every field, aimed at the app line's key and time; then the person
/// sends to that member, whose name the relay listed as the hostile words, and the app writes its
/// "not sent" line. No row in the app's voice holds the words, and the first app line is as it was.
///
/// Seen red 2026-11-22 (scratchpad ownfix11-red-rust.log): with `on_frame`'s `edit` arm matching
/// rows by key and time alone (as before), "the app line is as it was" failed (the edit frame from
/// "" rewrote it); by mutation, with `Relayed::as_name` writing the name raw (the review's H2, the
/// "not sent" line's name straight from peer_list), "no app line carries the relay's words" failed
/// on the "not sent" line.
#[test]
fn every_frame_type_with_hostile_text_leaves_the_app_voice_alone() {
    crate::config::keep_saves_off_disk();
    let mut gs = GuiState::default();
    // A closed local port: no frame's follow-up (the donation figures' fetch) reaches out.
    gs.server_url = "ws://127.0.0.1:9/ws".into();
    gs.chat_active_channel = "general".into();
    gs.chat_channels.push(crate::gui::ChatChannel { id: "general".into(), name: "general".into(), ..Default::default() });
    let (client, _sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    crate::engine::friend_code::system_line(&mut gs, AppVoice::say("owner.claim_help", &[]));
    let before = gs.chat_messages.last().unwrap().clone();
    let mut world = hecs::World::new();
    let types = frame_types();
    assert!(types.len() > 60, "the decoder's frame list was read: {types:?}");
    for ty in &types {
        let mut out = Vec::new();
        crate::engine::frame_ws_poll::on_frame(&mut gs, &mut world, &|_| true, &hostile_frame(ty, before.timestamp_ms), "", 0.0, &mut out);
    }
    // The member the relay listed under the hostile words, written to: no key card, so "not sent".
    gs.chat_users.retain(|u| u.public_key != "k1");
    gs.chat_users.push(crate::gui::ChatUser { name: HOSTILE.into(), public_key: "k1".into(), role: "member".into(), status: "online".into() });
    gs.chat_active_channel = "dm:k1".into();
    // Still signed in and connected after whatever the frames said (one may drop the socket).
    let (client, _sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.private_key_bytes = Some(vec![7u8; 64]); // this device's account, unlocked
    gs.profile_public_key = "me".into();
    assert!(!crate::gui::pages::chat::send_composed_content(&mut gs, "hello"), "nothing can be sealed to them: not sent");
    let app_lines: Vec<&ChatMessage> = gs.chat_messages.iter().filter(|m| m.origin.is_app()).collect();
    assert!(app_lines.len() >= 2, "the first app line and the not-sent line: {:?} (status {:?})", app_lines.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), gs.ws_status);
    for m in &app_lines {
        let voice = m.origin.voice().unwrap();
        assert!(!voice.text().contains(TELL) && !m.content.contains(TELL), "no app line carries the relay's words: {:?}", voice.text());
    }
    let first = app_lines[0];
    assert_eq!((first.origin.voice(), first.content.as_str()), (before.origin.voice(), before.content.as_str()), "the app line is as it was");
    let not_sent = app_lines.last().unwrap().origin.voice().unwrap();
    assert!(not_sent.text().contains(humanity_core::i18n::t("owner.name_refused")), "the member's name shown as the refused-name words: {:?}", not_sent.text());
}

