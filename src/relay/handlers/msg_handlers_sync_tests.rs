// Child of msg_handlers.rs (#[path]). BUG-283 (2026-11-07): the website's settings sync (notes,
// to-dos, the garden, settings) is kept on the server only sealed with a key from the person's
// recovery phrase. The website sent it unsealed when it had no key, and the server stored any JSON
// it was given, readable by its operator.
//
// Seen red on 2026-11-07 against handle_sync_save before the fix: an unsealed copy was stored
// ("an unsealed copy is not kept" failed: load_user_data found it).

use super::*;
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;

fn fresh_state() -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp("sync_sealed")))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A copy shaped the way web/chat/app.js `encryptSyncData` seals one: a 12-byte nonce and a
/// ciphertext with its 16-byte tag, in base64 (the bytes do not matter: the server cannot open it).
fn sealed() -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    serde_json::json!({ "v": 1, "key": "k", "iv": B64.encode([7u8; 12]), "encrypted": B64.encode([9u8; 40]) }).to_string()
}

fn save(st: &Arc<RelayState>, key: &str, data: &str) {
    block(handle_sync_save(st, key, &serde_json::json!({ "type": "sync_save", "data": data })));
}

#[test]
fn an_unsealed_copy_is_not_kept() {
    let st = fresh_state();
    let mut rx = st.broadcast_tx.subscribe();
    let plain = r#"{"humanity_notes":"[{\"text\":\"my bank PIN hint\"}]","humanity_settings":"{}"}"#;
    save(&st, "ann", plain);
    assert!(st.db.load_user_data("ann").unwrap().is_none(), "an unsealed copy is not kept");
    let said: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|m| match m { RelayMessage::Private { to, message } if to == "ann" => Some(message), _ => None })
        .collect();
    assert!(said.iter().any(|m| m.contains("only sealed")), "the sender is told why, in words: {said:?}");
    assert!(!said.iter().any(|m| m == "sync_ack"), "and it is not acknowledged: {said:?}");
}

#[test]
fn a_sealed_copy_is_kept_and_anything_else_refused() {
    let st = fresh_state();
    save(&st, "bea", &sealed());
    assert_eq!(st.db.load_user_data("bea").unwrap().map(|(d, _)| d), Some(sealed()), "a sealed copy is kept as sent");
    for bad in [
        r#"{"v":1,"iv":"AAAA","encrypted":"AAAAAAAAAAAAAAAAAAAAAA=="}"#.to_string(), // a nonce of 3 bytes
        r#"{"v":2,"iv":"BwcHBwcHBwcHBwcH","encrypted":"AAAAAAAAAAAAAAAAAAAAAA=="}"#.to_string(), // a format nobody seals
        r#"{"v":1,"iv":"BwcHBwcHBwcHBwcH","encrypted":"AAAA"}"#.to_string(), // shorter than the tag
        r#"{"v":1,"iv":"BwcHBwcHBwcHBwcH"}"#.to_string(),
        "not json".to_string(),
    ] {
        assert!(!is_sealed_sync_blob(&bad), "refused: {bad}");
    }
    assert!(is_sealed_sync_blob(&sealed()));
}
