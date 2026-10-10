// Child of chat/reply.rs (#[path]): where a reply may go and what it keeps (the 2026-10-10 batch
// review of docs/design/blocking-and-safe-mode.md). Each test was seen failing once on purpose,
// recorded at the test.

use super::*;

/// A signed-in app viewing `channel`, connected through a socket that records what is sent. No
/// seed, so nothing is signed and the phrase guard has nothing to match.
fn app(channel: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.server_url = "https://chat.example".into();
    gs.profile_public_key = "me".into();
    gs.user_name = "Me".into();
    gs.chat_active_channel = channel.into();
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    (gs, sent)
}

fn message(channel: &str, from: &str, content: &str) -> ChatMessage {
    ChatMessage { sender_name: from.into(), sender_key: from.into(), content: content.into(), timestamp_ms: 41, channel: channel.into(), ..Default::default() }
}

/// THE LEAK (the review, item 1): a reply started on a message in a P2P group, then a move to a
/// public channel and a send there: the public frame carries no `reply_to` and none of the
/// group's words; a reply started in the public channel itself still goes with its own send. The
/// target is also dropped as soon as another conversation is open.
/// Seen red 2026-10-10 with `wire_reply` returning `state.chat_reply_to.clone()` (the old
/// behaviour): "the public send carries no reply" failed, its `reply_to.content` the group's
/// words.
#[test]
fn a_reply_to_a_private_message_never_reaches_a_public_channel() {
    let secret = "the meeting is at the old mill, bring the spare key";
    let (mut gs, sent) = app("p2pgroup:g1");
    gs.chat_reply_to = Some(context_for(&message("p2pgroup:g1", "ann", secret), "p2pgroup:g1", 41));
    assert!(gs.chat_reply_to.as_ref().unwrap().private, "a group message is private");
    gs.chat_active_channel = "general".into(); // moved, by any path that does not clear it
    assert!(super::super::send_composed_content(&mut gs, "hello all"));
    let frames: Vec<serde_json::Value> = sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).collect();
    let chat = frames.iter().find(|f| f["type"] == "chat").expect("the public send");
    assert!(chat.get("reply_to").is_none(), "the public send carries no reply: {chat}");
    assert!(frames.iter().all(|f| !f.to_string().contains("old mill")), "none of the group's words leave");
    assert!(gs.chat_messages.last().is_some_and(|m| m.reply_to.is_none()), "nor does the echo show one");

    // The same conversation, a DM this time: the reply shows on our own copy and goes nowhere.
    let (mut gs, _sent) = app("dm:ann");
    gs.chat_reply_to = Some(context_for(&message("dm:ann", "ann", secret), "dm:ann", 41));
    assert!(wire_reply(&gs, "dm:ann").is_none(), "a DM's reply never goes on the wire");
    assert!(made_in(&gs, "dm:ann").is_some(), "but it is the reply of this conversation");

    // A public reply in its own channel still goes with its send.
    let (mut gs, sent) = app("general");
    gs.chat_reply_to = Some(context_for(&message("general", "bo", "who has the map?"), "general", 41));
    assert!(super::super::send_composed_content(&mut gs, "I do"));
    let chat: serde_json::Value = sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).find(|f: &serde_json::Value| f["type"] == "chat").unwrap();
    assert_eq!(chat["reply_to"]["content"], "who has the map?", "a public reply keeps its quote");

    // Opening another conversation drops the target.
    gs.chat_reply_to = Some(context_for(&message("general", "bo", "x"), "general", 41));
    drop_if_elsewhere(&mut gs);
    assert!(gs.chat_reply_to.is_some(), "still here: kept");
    gs.chat_active_channel = "dm:bo".into();
    drop_if_elsewhere(&mut gs);
    assert!(gs.chat_reply_to.is_none(), "another conversation: dropped");
}

/// WHAT A PREVIEW KEEPS (the review, item 1): never a file marker or any part of it (its key opens
/// the file), but "Photo" or the file's name; text before a marker that does not read, without the
/// marker; and a cut that never splits a character, where byte slicing panicked.
/// Seen red 2026-10-10 two ways: with `preview_of` cutting `content` at 80 characters without
/// looking for the marker, "a file: its name" failed (the preview was the marker's own text, key
/// and all); and with `cut` slicing at byte `max` (the old way), "80 characters, not 80 bytes"
/// failed (40 accented letters), the cut that panics once byte 80 falls inside a character.
#[test]
fn a_preview_never_holds_a_file_key_and_is_cut_between_characters() {
    let att = crate::net::dm_pq::DmAttachment {
        url: "/uploads/0123.enc".into(),
        k: "S0VZLUtFWS1LRVktS0VZLUtFWS1LRVktS0VZLUtFWS0=".into(),
        n: "bm9uY2Vub25jZQ==".into(),
        name: "plan.pdf".into(),
        mime: "application/pdf".into(),
        size: 9,
    };
    let marker = crate::net::dm_pq::build_file_marker(&att);
    assert_eq!(preview_of(&marker), "plan.pdf", "a file: its name");
    let photo = crate::net::dm_pq::build_file_marker(&crate::net::dm_pq::DmAttachment { mime: "image/png".into(), ..att.clone() });
    assert_eq!(preview_of(&photo), "Photo", "a picture: Photo");
    for text in [format!("look at this {marker}"), format!("{}{}", "x ".repeat(10), "[[hum:file:v9]]c2VjcmV0LWtleS1wYXJ0")] {
        let p = preview_of(&text);
        assert!(!p.contains("[[hum:") && !p.contains(&att.k[..12]) && !p.contains("c2VjcmV0"), "no part of the marker: {p}");
    }
    assert_eq!(preview_of("[[hum:file:v2]]broken"), "File", "a marker alone that does not read");

    let accented = "é".repeat(100);
    assert_eq!(preview_of(&accented), format!("{}…", "é".repeat(80)), "80 characters, not 80 bytes");
    let mixed = format!("{}日本語のテキスト", "a".repeat(79));
    assert_eq!(cut(&mixed, 80), format!("{}日…", "a".repeat(79)));
    assert_eq!(cut("short", 60), "short", "nothing to cut");
    assert_eq!(cut(&"🙂".repeat(61), 60), format!("{}…", "🙂".repeat(60)));
}
