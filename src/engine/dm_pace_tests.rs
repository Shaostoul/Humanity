// Child of engine/dm.rs (#[path]): the friendship-pass sweep keeps to the server's pace (the
// 2026-10-10 server review). Each test was seen failing once on purpose, recorded at the test.

use super::*;
use crate::net::dm_store::DmStore;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use std::time::{Duration, Instant};

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

/// The `to` of every `dm_put` sent since the last look, our own self-copies left out. Each is
/// answered `dm_put_ok`, as the server does once it stored it (10l): only then is it recorded.
fn puts_to_others(gs: &mut GuiState, sent: &std::sync::mpsc::Receiver<String>, me: &str) -> Vec<String> {
    let frames: Vec<serde_json::Value> = sent.try_iter().filter_map(|f| serde_json::from_str(&f).ok()).collect();
    let mut out = Vec::new();
    for v in frames.iter().filter(|v| v["type"] == "dm_put" && v["to"] != me) {
        out.push(v["to"].as_str().unwrap_or("").to_string());
        let taken = serde_json::json!({ "type": "dm_put_ok", "ref": v["ref"] });
        assert!(crate::engine::put_answer::on_frame(gs, &taken), "every pass put carries a ref");
    }
    out
}

/// A SWEEP OWING TEN PASSES KEEPS TO THE SERVER'S PACE: the server lets a sender 8 `dm_put`s at
/// once and then one a second, and refuses (without delivering) a put over that; a pass is two
/// puts and is never sent again once recorded. So the sweep sends three passes (six puts) at
/// once, holds the rest, and sends them no faster than one put a second as the budget refills;
/// every pass recorded as given went out (and was taken, 10l), and in the end all ten have.
/// Seen red 2026-10-10 with `send_owed_passes` sending every owed pass back to back (the old
/// sweep): "six puts at once" failed (left: 20, right: 6).
#[test]
fn a_sweep_owing_many_passes_keeps_to_the_servers_pace() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(200);
    let mut gs = GuiState::default();
    gs.profile_public_key = me.clone();
    gs.private_key_bytes = Some(seed.clone());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://pace-{nanos}.example");
    let mut store = DmStore::load(&seed, &me, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    let mut friends = Vec::new();
    for n in 201..211u8 {
        let (s, k) = identity(n);
        gs.peer_kyber_keys.insert(k.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&s).unwrap().public_base64());
        store.set_following(&k, true);
        store.set_follower(&k, true);
        friends.push(k);
    }
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(&seed, &me, "pace"));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    // This connection's mailbox has been read (10n N7), so the pass sweep and passes run.
    gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();

    let t0 = Instant::now();
    sweep_friend_passes(&mut gs);
    let mut delivered = puts_to_others(&mut gs, &sent, &me);
    assert_eq!(delivered.len() * 2, 6, "six puts at once (three passes, each with its self-copy)");
    assert!(gs.pass_pacer.held, "the rest are held");
    let recorded = |gs: &GuiState| friends.iter().filter(|f| gs.dm_store.as_ref().unwrap().cert_sent_to(f)).cloned().collect::<Vec<_>>();
    assert_eq!(recorded(&gs).len(), delivered.len(), "only what went out is recorded as given");

    pace_owed_passes(&mut gs, t0 + Duration::from_millis(500));
    assert!(puts_to_others(&mut gs, &sent, &me).is_empty(), "half a second later: nothing yet");
    for secs in 1..=30u64 {
        pace_owed_passes(&mut gs, t0 + Duration::from_secs(secs));
        delivered.extend(puts_to_others(&mut gs, &sent, &me));
        assert!(delivered.len() * 2 <= 6 + secs as usize + 1, "no faster than one put a second: {} puts by {secs}s", delivered.len() * 2);
        let mut recorded_now = recorded(&gs);
        let mut out = delivered.clone();
        recorded_now.sort();
        out.sort();
        assert_eq!(recorded_now, out, "every pass recorded as given went out ({secs}s)");
    }
    assert_eq!(delivered.len(), 10, "in the end every friend has one");
    assert!(!gs.pass_pacer.held, "and nothing is held");
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    gs.block_list.as_ref().unwrap().remove_file_for_test();
}
