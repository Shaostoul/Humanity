// Child of presence.rs (#[path]). THE SAME CHOICE SENT AGAIN SAYS NOTHING (the operator,
// 2026-11-22: the presence line reappeared on every reconnect, three times over): a person's first
// change is answered once, and the same choice sent again on the next sign-in is stored silently.
//
// Seen red 2026-11-22 against the handler before the fix (the unchanged check taken out): the
// repeat sent a second "Presence visible" line ("a repeat of the same choice says nothing").

use super::*;
use crate::relay::storage::Storage;

fn private_lines(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        if let RelayMessage::Private { to, message } = m {
            if to == key {
                out.push(message);
            }
        }
    }
    out
}

#[tokio::test]
async fn a_change_is_answered_once_and_a_repeat_says_nothing() {
    let state = Arc::new(RelayState::new(Storage::open_temp("presence_repeat")));
    let key = "presence_key";
    state.db.join_server(key, "Ada").unwrap();
    // A new member joins hidden (members.rs join_server: fail-private).
    assert!(state.db.presence_hidden(key), "a new member starts hidden");
    let mut rx = state.broadcast_tx.subscribe();
    handle_privacy_update(&state, key, false).await;
    assert_eq!(private_lines(&mut rx, key), vec![PRESENCE_VISIBLE_WORDS.to_string()], "the change is told once");
    assert!(!state.db.presence_hidden(key), "the change is stored");
    handle_privacy_update(&state, key, false).await;
    assert!(private_lines(&mut rx, key).is_empty(), "a repeat of the same choice says nothing");
    handle_privacy_update(&state, key, true).await;
    assert_eq!(private_lines(&mut rx, key), vec![PRESENCE_HIDDEN_WORDS.to_string()], "a real change back is told");
    assert!(state.db.presence_hidden(key));
}
