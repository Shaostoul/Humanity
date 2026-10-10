// Child of engine/friend_code.rs (#[path]): the server's friend-code answers with a recording
// socket and no screen (the 2026-10-10 batch review, item 6). Each test was seen failing once on
// purpose, recorded at the test.

use super::*;
use crate::net::dm_store::DmStore;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

/// A signed-in app on a server of its own, with a socket that records what is sent, a block list
/// in the temp directory, and the protected setup's preset loaded.
fn app(me: &str, seed: &[u8], tag: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = me.to_string();
    gs.private_key_bytes = Some(seed.to_vec());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://{tag}-{nanos}.example");
    let mut store = DmStore::load(seed, me, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(seed, me, tag));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    assert!(crate::engine::protected::ensure_preset(&mut gs));
    (gs, sent)
}

fn tidy(gs: &GuiState) {
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    gs.block_list.as_ref().unwrap().remove_file_for_test();
}

fn result(owner: &str) -> serde_json::Value {
    serde_json::json!({ "type": "friend_code_result", "success": true, "name": "Ann", "message": "accepted", "owner_key": owner })
}

/// The protected setup on with `pin` (a quick verifier) and nobody approved.
fn protect(gs: &mut GuiState, pin: &str) {
    let preset = gs.protected.preset.clone().unwrap();
    let quick = crate::net::protected::PinVerifier::with_salt(pin, &[3u8; 16], 1_000);
    gs.protected.setup = crate::net::protected::ProtectedSetup::applied(&preset, quick, &gs.profile_public_key.clone(), Vec::new());
}

/// A REDEEMED CODE FOLLOWS ITS OWNER (the review, item 6): the desktop app had no handler for
/// `friend_code_result`, so a code redeemed here followed nobody (the relay keeps no social graph
/// and creates no follow). Now a success follows the owner it names (a follow notice goes to
/// them), through Follow's own gate: with the setup off at once; with it on and the code redeemed
/// with the PIN, at once too (that PIN covers the friend the answer names, who is then approved);
/// with it on and no PIN given for a redeem, the PIN prompt opens for the follow. A failure says
/// why and follows no one; a code the server made is shown.
/// Seen red 2026-10-10 with `on_frame` not taking `friend_code_result` (no handler, as before):
/// "the frame is handled" failed; and with the PIN-given grant left out, "redeemed with the PIN:
/// no second prompt" failed (the prompt opened for Follow).
#[test]
fn a_redeemed_code_follows_its_owner_through_follows_gate() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(190);
    let (ann_seed, ann) = identity(191);
    let (mut gs, sent) = app(&me, &seed, "code-off");
    gs.peer_kyber_keys.insert(ann.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&ann_seed).unwrap().public_base64());
    assert!(on_frame(&mut gs, &result(&ann)), "the frame is handled");
    assert!(gs.dm_store.as_ref().unwrap().is_following(&ann), "a redeemed code follows its owner");
    assert!(sent.try_iter().any(|f| f.contains("\"dm_put\"")), "and the follow notice goes to them");
    assert!(gs.chat_messages.iter().any(|m| m.content.contains("you now follow Ann")), "and says so");
    tidy(&gs);

    // On, and no PIN given for a redeem: the follow asks for it.
    let (mut gs, _sent) = app(&me, &seed, "code-on");
    gs.peer_kyber_keys.insert(ann.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&ann_seed).unwrap().public_base64());
    protect(&mut gs, "7391");
    on_frame(&mut gs, &result(&ann));
    assert!(!gs.dm_store.as_ref().unwrap().is_following(&ann), "not followed without the PIN");
    assert_eq!(gs.protected.waiting_action(), Some(&ProtectedAction::Follow(ann.clone())), "the prompt opens for the follow");
    crate::engine::protected::cancel_prompt(&mut gs);

    // On, and the code redeemed with the PIN: its answer follows the owner without a second PIN.
    crate::engine::protected::perform(&mut gs, ProtectedAction::RedeemFriendCode("AB12CD34".into()));
    gs.protected.prompt_input = "7391".into();
    crate::engine::protected::submit_prompt(&mut gs, crate::engine::protected::now_secs());
    assert!(gs.protected.code_redeemed, "the PIN was given for a redeem");
    on_frame(&mut gs, &result(&ann));
    assert!(gs.protected.prompt.is_none(), "redeemed with the PIN: no second prompt");
    assert!(gs.dm_store.as_ref().unwrap().is_following(&ann) && gs.protected.setup.is_approved(&ann), "followed, and approved");
    assert!(!gs.protected.code_redeemed && gs.protected.granted.is_none(), "the PIN's cover is used up");
    tidy(&gs);

    // A failure follows no one and says why; our own key is never followed.
    let (mut gs, _sent) = app(&me, &seed, "code-fail");
    on_frame(&mut gs, &serde_json::json!({ "type": "friend_code_result", "success": false, "message": "Invalid or expired friend code." }));
    assert!(gs.dm_store.as_ref().unwrap().following().is_empty());
    assert!(gs.chat_messages.iter().any(|m| m.content == "The friend code did not work: Invalid or expired friend code."));
    on_frame(&mut gs, &result(&me));
    assert!(gs.dm_store.as_ref().unwrap().following().is_empty(), "never ourselves");

    // A code the server made is shown, in Chat and on Server Settings.
    assert!(on_frame(&mut gs, &serde_json::json!({ "type": "friend_code_response", "code": "Q7RT2ZKP", "target": me })));
    assert!(gs.chat_messages.iter().any(|m| m.content.starts_with("Your friend code: Q7RT2ZKP.")), "the code is shown");
    assert!(gs.server_settings_status.contains("Q7RT2ZKP"));
    assert!(!on_frame(&mut gs, &serde_json::json!({ "type": "something_else" })), "another frame is not taken");
    tidy(&gs);
}
