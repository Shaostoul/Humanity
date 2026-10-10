// Child of engine/protected.rs (#[path]): the protected setup with the app state and a recording
// socket, no screen (step G of docs/design/blocking-and-safe-mode.md, 10h's proof list). Each test
// was seen failing once on purpose, recorded at the test.

use super::*;
use crate::net::dm_store::{DmStore, SentPass};
use crate::net::protected::{PinTry, Preset};
use crate::net::reach::{Audience, ContactRequest, ReachKind};
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use std::io::{Read, Write};

const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

/// A signed-in app for `seed` / `me`, on a server of its own (a unique address, so its DM store
/// file is its own), with a socket that records what is sent, a block list in the temp
/// directory, and the preset loaded. Returns the app and what its socket sent.
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
    assert!(ensure_preset(&mut gs), "the preset loads");
    (gs, sent)
}

fn tidy(gs: &GuiState) {
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    gs.block_list.as_ref().unwrap().remove_file_for_test();
}

/// Everything sent since the last look, as JSON.
fn frames(sent: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).expect("a JSON frame")).collect()
}

fn preset(gs: &GuiState) -> Preset {
    gs.protected.preset.clone().unwrap()
}

/// The setup on, as turning it on leaves it, with `pin` behind a quick verifier (few iterations,
/// so the tests that only USE a PIN stay fast; the tests of choosing one use the real 600,000),
/// and `approved` as the friends kept in its review.
fn protect(gs: &mut GuiState, pin: &str, approved: &[&str]) {
    let quick = PinVerifier::with_salt(pin, &[3u8; 16], 1_000);
    let kept = approved.iter().map(|k| k.to_string()).collect();
    gs.protected.setup = ProtectedSetup::applied(&preset(gs), quick, &gs.profile_public_key.clone(), kept);
}

/// Enter `pin` in the PIN prompt now.
fn enter(gs: &mut GuiState, pin: &str) {
    gs.protected.prompt_input = pin.to_string();
    submit_prompt(gs, now_secs());
}

fn waiting(gs: &GuiState) -> Option<ProtectedAction> {
    gs.protected.waiting_action().cloned()
}

/// A server on 127.0.0.1 that answers the next `n` HTTP requests with 200 and hands each
/// request's text to the receiver (a group's leave posts a signed object, then the group list is
/// asked for again). Loopback only, so no firewall prompt.
fn local_server(n: usize) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(n) {
            let Ok(mut s) = stream else { continue };
            let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let Ok(got) = s.read(&mut chunk) else { break };
                if got == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..got]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&buf).to_string();
            let body = if request.starts_with("GET") { "{\"groups\":[]}" } else { "{}" };
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = tx.send(request);
        }
    });
    (format!("http://127.0.0.1:{port}"), rx)
}

/// THE PIN ITSELF IS NEVER STORED (10h): turning the setup on with a PIN leaves in config.json
/// only a verifier, a 16-byte salt and a PBKDF2-SHA-256 hash at 600,000 iterations, and the PIN
/// fields are empty again; the config read back into a fresh app opens with that PIN and no other.
/// A PIN of the wrong shape, or two that differ, are refused with the preset's PIN rule before
/// anything is made.
/// Seen red 2026-10-10 with `verifier_from_fields` taking the fields without clearing them: "the
/// typed PIN is gone from the app" failed.
#[test]
fn turning_it_on_stores_a_verifier_and_never_the_pin() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(161);
    let (mut gs, _sent) = app(&me, &seed, "protected-store");
    begin(&mut gs);
    assert_eq!(gs.protected.step, Some(SetupStep::Read), "step 1, Read");
    read_done(&mut gs);
    assert_eq!(gs.protected.step, Some(SetupStep::ChoosePin), "step 2, Choose a PIN");
    for (a, b, why) in [("123", "123", "three digits"), ("12a4", "12a4", "a letter"), ("4821", "4822", "two that differ")] {
        gs.protected.pin_first = a.into();
        gs.protected.pin_second = b.into();
        assert!(!pin_chosen(&mut gs), "{why} is refused");
        assert_eq!(gs.protected.step, Some(SetupStep::ChoosePin));
        assert_eq!(gs.protected.line, preset(&gs).pin_rule(), "with the preset's rule");
    }
    gs.protected.pin_first = "48213907".into();
    gs.protected.pin_second = "48213907".into();
    assert!(pin_chosen(&mut gs));
    assert_eq!(gs.protected.step, Some(SetupStep::Review), "step 3, Review");
    assert!(gs.protected.pin_first.is_empty() && gs.protected.pin_second.is_empty(), "the typed PIN is gone from the app");
    apply(&mut gs);
    assert!(is_on(&gs), "it is on");
    assert_eq!(gs.protected.setup.identity, me, "under this identity");

    let verifier = gs.protected.setup.pin.clone().expect("a verifier");
    assert_eq!(verifier.iterations, 600_000);
    use base64::Engine;
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(&verifier.salt).unwrap().len(), 16, "a 16-byte salt");

    // Saved with the settings, into its own file beside config.json (never into config.json),
    // in a scratch folder rather than the person's real one.
    let dir = std::env::temp_dir().join(format!("protected-save-{}-{}", std::process::id(), now_secs()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.json");
    crate::config::AppConfig::from_gui_state(&gs).save_at(&config_path);
    let config_text = std::fs::read_to_string(&config_path).expect("config.json written");
    assert!(!config_text.contains("protected_setup") && !config_text.contains(&verifier.salt), "nothing of it in config.json");
    let setup_text = std::fs::read_to_string(crate::net::protected_store::path_beside(&config_path)).expect("its own file");
    assert!(setup_text.contains(&verifier.salt) && !setup_text.contains("48213907"), "its file holds the verifier, never the PIN");

    let mut fresh = GuiState::default();
    fresh.protected.setup = crate::net::protected_store::load(&crate::net::protected_store::path_beside(&config_path));
    assert!(fresh.protected.setup.on, "it is still on after a restart");
    assert_eq!(fresh.protected.setup.pin, gs.protected.setup.pin, "with its verifier read back whole");

    // A damaged config.json (which loads as the defaults) cannot turn it off: the file beside it
    // is read on its own (the 2026-10-10 review, item 7).
    std::fs::write(&config_path, "{ not json").unwrap();
    assert!(crate::net::protected_store::load(&crate::net::protected_store::path_beside(&config_path)).on, "still on beside a damaged config.json");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ensure_preset(&mut fresh));
    assert_eq!(try_pin(&mut fresh, "48213906", 10), PinTry::Wrong, "a wrong PIN is refused");
    assert_eq!(try_pin(&mut fresh, "48213907", 11), PinTry::Right, "the right PIN opens it");
    tidy(&gs);
}

/// TURNING IT ON SENDS ONE `reach_set` AND NOTHING ELSE (10h, a recorded-frames test): reading,
/// choosing the PIN and the review send nothing; applying sends exactly the preset's rows as the
/// ordinary frame, with no field naming the setup, and turns warnings on (locally); the server's
/// answer, and a sign-in again later, send nothing more.
/// Seen red 2026-10-10 with `apply` also sending a `privacy_update`: "nothing else" failed.
#[test]
fn turning_it_on_sends_one_reach_set_and_nothing_else() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(162);
    let (mut gs, sent) = app(&me, &seed, "protected-frames");
    gs.settings.warnings_on_messages = false;
    begin(&mut gs);
    read_done(&mut gs);
    gs.protected.pin_first = "5150".into();
    gs.protected.pin_second = "5150".into();
    pin_chosen(&mut gs);
    assert!(frames(&sent).is_empty(), "reading, the PIN and the review send nothing");
    apply(&mut gs);
    let out = frames(&sent);
    assert_eq!(out, vec![serde_json::json!({ "type": "reach_set", "settings": { "message": "friends", "call": "chosen", "trade": "friends" } })], "one reach_set, the preset's rows, nothing else");
    for f in &out {
        assert!(!f.to_string().to_lowercase().contains("protect"), "no frame names the setup: {f}");
    }
    assert!(gs.settings.warnings_on_messages, "warnings on: local, not sent");

    let answer = serde_json::json!({ "type": "reach_settings", "settings": { "message": "friends", "call": "chosen", "trade": "friends" } });
    crate::engine::reach::on_frame(&mut gs, &answer);
    assert!(frames(&sent).is_empty(), "the server's answer sends nothing more");
    let looser = serde_json::json!({ "type": "reach_settings", "settings": { "message": "anyone", "call": "anyone", "trade": "anyone" } });
    crate::engine::reach::on_frame(&mut gs, &looser);
    assert!(frames(&sent).is_empty(), "nor does a later sign-in: exactly one, at turn-on");
    tidy(&gs);
}

/// Turning it on needs a live connection (as on the web chat): offline, "Keep the rest" says the
/// preset's `not_connected` and the setup stays off, with nothing sent and the step kept so it
/// can be applied once connected. Without this server's settings loaded, it does not start.
/// Seen red 2026-10-10 with `apply`'s connection check taken out: "it stays off" failed (on, with
/// the reach_set never sent).
#[test]
fn turning_it_on_needs_a_connection() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(173);
    let (mut gs, _sent) = app(&me, &seed, "protected-offline");
    begin(&mut gs);
    read_done(&mut gs);
    gs.protected.pin_first = "8080".into();
    gs.protected.pin_second = "8080".into();
    pin_chosen(&mut gs);
    gs.ws_client = None;
    apply(&mut gs);
    assert!(!is_on(&gs), "it stays off");
    assert_eq!(gs.protected.line, preset(&gs).labels.not_connected, "and says why");
    assert_eq!(gs.protected.step, Some(SetupStep::Review), "the step is kept");
    tidy(&gs);

    let mut locked = GuiState::default();
    assert!(ensure_preset(&mut locked));
    begin(&mut locked);
    assert_eq!(locked.protected.step, None, "no identity and no server: it does not start");
    assert_eq!(locked.protected.line, preset(&locked).labels.waiting_store);
}

/// EVERY LOCKED ACTION, refused without the PIN and done with it (10h, "While it is on, these need
/// the PIN"), each through its own entry point: nothing happens, nothing is sent and the PIN
/// prompt opens for it; a wrong PIN does nothing more; the right one does it.
/// Seen red 2026-10-10 with the gate taken out of `engine::reach::set_tick`: "Tick ... is refused
/// without the PIN" failed (Call ticked at once).
#[test]
fn every_locked_action_needs_the_pin() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(163);
    let (ben_seed, ben) = identity(164);
    let (eve_seed, eve) = identity(174);
    let (mut gs, sent) = app(&me, &seed, "protected-locked");
    for (k, s) in [(&ben, &ben_seed), (&eve, &eve_seed)] {
        gs.peer_kyber_keys.insert(k.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(s).unwrap().public_base64());
    }
    {
        let store = gs.dm_store.as_mut().unwrap();
        store.set_follower(&ben, true); // Ben follows us: "Follow back"
        store.record_pass_sent("cy", SentPass { serial: "c1".repeat(16), may: "invite,message,trade,voice_message".into() });
        store.add_request(ContactRequest { key: "dana".into(), ts: 1, pass: String::new() });
    }
    gs.chat_channels.push(crate::gui::ChatChannel { id: "lounge".into(), name: "lounge".into(), read_only: true, voice_enabled: true, ..Default::default() });
    gs.settings.warnings_on_messages = true;
    protect(&mut gs, "7391", &["cy"]);
    let _ = frames(&sent);

    // Each: the action, done through its entry point; then whether it has happened.
    type Done = fn(&GuiState, &[serde_json::Value]) -> bool;
    let ben_k = ben.clone();
    let eve_k = eve.clone();
    let cases: Vec<(ProtectedAction, Box<dyn Fn(&mut GuiState)>, Done)> = vec![
        (
            ProtectedAction::ReachRow(ReachKind::Message, Audience::Anyone),
            Box::new(|gs: &mut GuiState| crate::engine::reach::ask(gs, ReachKind::Message, Audience::Anyone)),
            |_: &GuiState, f: &[serde_json::Value]| f.iter().any(|f| f["type"] == "reach_set" && f["settings"]["message"] == "anyone"),
        ),
        (
            ProtectedAction::Tick("cy".into(), ReachKind::Call, true),
            Box::new(|gs: &mut GuiState| crate::engine::reach::set_tick(gs, "cy", ReachKind::Call, true)),
            |gs: &GuiState, _: &[serde_json::Value]| gs.dm_store.as_ref().unwrap().ticks("cy").call,
        ),
        (
            ProtectedAction::Follow(ben.clone()),
            Box::new(move |gs: &mut GuiState| crate::engine::dm::set_follow(gs, &ben_k, true)),
            |gs: &GuiState, f: &[serde_json::Value]| gs.dm_store.as_ref().unwrap().following().len() == 1 && f.iter().any(|f| f["type"] == "dm_put"),
        ),
        (
            ProtectedAction::AcceptRequest("dana".into()),
            Box::new(|gs: &mut GuiState| crate::engine::reach::accept_request(gs, "dana")),
            |gs: &GuiState, _: &[serde_json::Value]| gs.dm_store.as_ref().unwrap().requests().is_empty(),
        ),
        (
            ProtectedAction::SendRequest(eve.clone()),
            Box::new(move |gs: &mut GuiState| {
                let _ = crate::engine::reach::send_contact_request(gs, &eve_k);
            }),
            |_: &GuiState, f: &[serde_json::Value]| f.iter().any(|f| f["contact_request"] == true),
        ),
        (
            ProtectedAction::RedeemFriendCode("AB12CD34".into()),
            Box::new(|gs: &mut GuiState| {
                if allows_typed(gs, "/redeem AB12CD34") {
                    crate::gui::pages::chat::send_slash_command(gs, "/redeem AB12CD34");
                }
            }),
            |_: &GuiState, f: &[serde_json::Value]| f.iter().any(|f| f["content"] == "/redeem AB12CD34"),
        ),
        (
            ProtectedAction::JoinGroup("not-a-ticket".into()),
            Box::new(|gs: &mut GuiState| crate::gui::pages::chat::join_group_with_ticket(gs, "not-a-ticket")),
            |gs: &GuiState, _: &[serde_json::Value]| gs.join_group_status.starts_with("Join failed"),
        ),
        (
            ProtectedAction::JoinVoice("lounge".into()),
            Box::new(|gs: &mut GuiState| crate::gui::pages::chat::set_voice_room(gs, "lounge", true)),
            |gs: &GuiState, f: &[serde_json::Value]| gs.chat_channels[0].voice_joined && f.iter().any(|f| f["type"] == "voice_room" && f["action"] == "join"),
        ),
        (ProtectedAction::WarningsOff, Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::WarningsOff)), |gs: &GuiState, _: &[serde_json::Value]| !gs.settings.warnings_on_messages),
        (ProtectedAction::ShowPictures, Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::ShowPictures)), |gs: &GuiState, _: &[serde_json::Value]| !gs.protected.setup.pictures_hidden),
        (ProtectedAction::ShowPublicRooms, Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::ShowPublicRooms)), |gs: &GuiState, _: &[serde_json::Value]| !gs.protected.setup.public_rooms_hidden),
        (
            ProtectedAction::ChangePin,
            Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::ChangePin)),
            |gs: &GuiState, _: &[serde_json::Value]| gs.protected.prompt.as_ref().is_some_and(|p| p.mode == PromptMode::NewPin && p.action.is_none()),
        ),
        (ProtectedAction::ShowRecoveryPhrase, Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::ShowRecoveryPhrase)), |gs: &GuiState, _: &[serde_json::Value]| gs.protected.phrase_shown),
        // Last, because it ends the setup.
        (ProtectedAction::TurnOff, Box::new(|gs: &mut GuiState| perform(gs, ProtectedAction::TurnOff)), |gs: &GuiState, _: &[serde_json::Value]| !gs.protected.setup.on && gs.protected.setup.pin.is_none()),
    ];
    for (action, run, done) in cases {
        run(&mut gs);
        let out = frames(&sent);
        assert!(!done(&gs, &out), "{action:?} is refused without the PIN");
        assert!(out.is_empty(), "{action:?}: nothing is sent without the PIN: {out:?}");
        assert_eq!(waiting(&gs), Some(action.clone()), "{action:?}: the PIN prompt opens for it");
        enter(&mut gs, "0000");
        assert!(!done(&gs, &frames(&sent)), "{action:?}: a wrong PIN does nothing");
        assert_eq!(gs.protected.prompt_line, preset(&gs).labels.pin_wrong, "and says so");
        enter(&mut gs, "7391");
        let out = frames(&sent);
        assert!(done(&gs, &out), "{action:?} is done with the PIN: {out:?} {}", gs.join_group_status);
        assert!(gs.protected.granted.is_none(), "{action:?}: no permission is left over");
        // One wrong try each time: a right PIN starts the count again, so no wait builds up.
        assert_eq!(gs.protected.setup.wait_until, 0);
        if action == ProtectedAction::ChangePin {
            cancel_prompt(&mut gs);
        }
        assert!(gs.protected.prompt.is_none(), "{action:?}: the prompt closes");
    }
    tidy(&gs);
}

/// THE APPROVED LIST (as on the web chat): while it is on, a pass goes only to a friend the PIN
/// holder let be one. A follow made before the setup and followed back after it makes no friend
/// (no pass is minted or sent) until it is made with the PIN; befriending someone kept in the
/// review needs no PIN; the PIN given to befriend someone puts them on the list; Unfollow and
/// Block take them off it, so befriending them again needs the PIN.
/// Seen red 2026-10-10 with `mint_pass`'s approved-list check taken out: "but no pass for a
/// follow made before the setup" failed (a pass went out).
#[test]
fn a_pass_goes_only_to_friends_the_pin_holder_approved() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(175);
    let (ben_seed, ben) = identity(176);
    let (cy_seed, cy) = identity(177);
    let (mut gs, sent) = app(&me, &seed, "protected-approved");
    for (k, s) in [(&ben, &ben_seed), (&cy, &cy_seed)] {
        gs.peer_kyber_keys.insert(k.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(s).unwrap().public_base64());
    }
    gs.dm_store.as_mut().unwrap().set_following(&ben, true); // a follow made before the setup
    protect(&mut gs, "7391", &[&cy]); // Cy was kept in the review
    let _ = frames(&sent);

    let back = crate::net::dm_pq::DmInner { from: ben.clone(), to: me.clone(), ts: 1, text: crate::net::dm_pq::CTL_FOLLOW.into(), sig_b64: "x".into(), cert: None };
    crate::engine::dm::ingest_dm(&mut gs, &back);
    assert!(gs.dm_store.as_ref().unwrap().is_friend(&ben), "Ben followed back: a mutual follow");
    assert!(!gs.dm_store.as_ref().unwrap().cert_sent_to(&ben), "but no pass for a follow made before the setup");
    assert!(frames(&sent).iter().all(|f| f["type"] != "dm_put"), "nothing went to him");
    crate::engine::dm::sweep_friend_passes(&mut gs);
    assert!(!gs.dm_store.as_ref().unwrap().cert_sent_to(&ben), "not from the pass sweep either");

    crate::engine::dm::set_follow(&mut gs, &cy, true);
    assert!(waiting(&gs).is_none() && gs.dm_store.as_ref().unwrap().is_following(&cy), "following someone kept in the review needs no PIN");

    crate::engine::dm::set_follow(&mut gs, &ben, false);
    crate::engine::dm::set_follow(&mut gs, &ben, true);
    assert_eq!(waiting(&gs), Some(ProtectedAction::Follow(ben.clone())), "befriending anyone else asks for the PIN");
    enter(&mut gs, "7391");
    assert!(gs.protected.setup.is_approved(&ben), "the PIN given puts him on the list");
    assert!(gs.dm_store.as_ref().unwrap().cert_sent_to(&ben), "and his pass goes out now");

    crate::engine::dm::set_follow(&mut gs, &ben, false);
    assert!(!gs.protected.setup.is_approved(&ben), "Unfollow takes him off the list");
    crate::engine::block::block(&mut gs, &cy);
    assert!(!gs.protected.setup.is_approved(&cy), "and so does Block");
    tidy(&gs);
}

/// EVERY NEVER-LOCKED ACTION goes without the PIN while the setup is on (10h: Block, Report,
/// Unfollow, leaving a group or a room), and no prompt opens.
/// Seen red 2026-10-10 with `needs_pin` saying true for `Unfollow`: "Unfollow goes without the
/// PIN" failed (still following, the prompt open).
#[test]
fn never_locked_actions_go_without_the_pin() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(165);
    let (mut gs, sent) = app(&me, &seed, "protected-free");
    {
        let store = gs.dm_store.as_mut().unwrap();
        for peer in ["ben", "cy"] {
            store.set_following(peer, true);
            store.set_follower(peer, true);
            store.record_pass_sent(peer, SentPass { serial: format!("{peer:0>32}"), may: "invite,message,trade,voice_message".into() });
        }
    }
    gs.chat_channels.push(crate::gui::ChatChannel { id: "lounge".into(), name: "lounge".into(), read_only: true, voice_enabled: true, voice_joined: true, ..Default::default() });
    gs.voice_active_room = Some("lounge".into());
    gs.p2p_groups.push(crate::net::api_v2::P2pGroupInfo { group_id: "g1".into(), name: "Book club".into(), members: vec![me.clone()], is_creator: false });
    protect(&mut gs, "7391", &["ben", "cy"]);

    for action in [
        ProtectedAction::Block("x".into()),
        ProtectedAction::Report("x".into()),
        ProtectedAction::Unfollow("x".into()),
        ProtectedAction::LeaveGroup("x".into()),
        ProtectedAction::LeaveRoom("x".into()),
    ] {
        assert!(allows(&mut gs, action.clone()), "{action:?} goes without the PIN");
    }
    assert!(gs.protected.prompt.is_none(), "and no prompt opens");

    crate::engine::block::block(&mut gs, "ben");
    assert!(crate::engine::block::is_blocked(&gs, "ben"), "Block goes without the PIN");
    crate::engine::block::unblock(&mut gs, "ben");
    assert!(!crate::engine::block::is_blocked(&gs, "ben"), "and so does Unblock");
    perform(&mut gs, ProtectedAction::Unfollow("cy".into()));
    assert!(!gs.dm_store.as_ref().unwrap().is_following("cy"), "Unfollow goes without the PIN");
    perform(&mut gs, ProtectedAction::Report("cy".into()));
    assert!(gs.reports.dialog.is_some(), "Report goes without the PIN");
    perform(&mut gs, ProtectedAction::LeaveRoom("lounge".into()));
    assert!(!gs.chat_channels[0].voice_joined && gs.voice_active_room.is_none(), "leaving a voice room goes without the PIN");
    assert!(frames(&sent).iter().any(|f| f["type"] == "voice_room" && f["action"] == "leave"));
    let (url, requests) = local_server(2);
    gs.server_url = url;
    perform(&mut gs, ProtectedAction::LeaveGroup("g1".into()));
    let posted = requests.recv_timeout(std::time::Duration::from_secs(10)).expect("the leave reaches the server");
    assert!(posted.starts_with("POST /api/v2/objects") && posted.contains("group_member_v1"), "leaving a group goes without the PIN: {posted}");
    assert!(gs.protected.prompt.is_none(), "no prompt opened for any of them");
    tidy(&gs);
}

/// THE REVIEW STEP (10h, step 3): it lists every friend (a pass given, and a mutual follow still
/// owed one), every group and every voice room joined, and Remove does Unfollow for a friend
/// (their passes withdrawn), Leave for a group (its signed leave posted) and Leave for a room.
/// "Keep the rest" makes the friends left the approved list.
/// Seen red 2026-10-10 with `ReviewItem::removal` giving `LeaveRoom` for a group: "Remove for a
/// group posts its leave" failed (nothing reached the server).
#[test]
fn the_review_steps_remove_for_a_friend_a_group_and_a_room() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(166);
    let (mut gs, sent) = app(&me, &seed, "protected-review");
    {
        let store = gs.dm_store.as_mut().unwrap();
        store.set_following("ben", true);
        store.set_follower("ben", true);
        store.record_pass_sent("ben", SentPass { serial: "b1".repeat(16), may: "invite,message,trade,voice_message".into() });
        store.set_following("cy", true); // a mutual follow still owed a pass
        store.set_follower("cy", true);
    }
    gs.chat_users.push(crate::gui::ChatUser { name: "Ben".into(), public_key: "ben".into(), role: String::new(), status: "online".into() });
    gs.chat_users.push(crate::gui::ChatUser { name: "Cy".into(), public_key: "cy".into(), role: String::new(), status: "online".into() });
    gs.p2p_groups.push(crate::net::api_v2::P2pGroupInfo { group_id: "g1".into(), name: "Book club".into(), members: vec![me.clone()], is_creator: false });
    gs.p2p_groups_last_fetch = Some(std::time::Instant::now());
    gs.chat_channels.push(crate::gui::ChatChannel { id: "lounge".into(), name: "lounge".into(), voice_enabled: true, voice_joined: true, ..Default::default() });
    gs.chat_channels.push(crate::gui::ChatChannel { id: "general".into(), name: "general".into(), voice_enabled: true, ..Default::default() });
    gs.voice_active_room = Some("lounge".into());

    let items = review_items(&gs);
    assert_eq!(
        items,
        vec![
            ReviewItem::Friend { key: "ben".into(), name: "Ben".into() },
            ReviewItem::Friend { key: "cy".into(), name: "Cy".into() },
            ReviewItem::Group { id: "g1".into(), name: "Book club".into() },
            ReviewItem::Room { id: "lounge".into(), name: "lounge".into() },
        ],
        "friends, groups, the voice room joined (not a room merely voice-enabled)"
    );

    review_remove(&mut gs, &items[0]);
    let store = gs.dm_store.as_ref().unwrap();
    assert!(!store.is_following("ben") && !store.cert_sent_to("ben"), "Remove for a friend unfollows and withdraws their pass");
    assert_eq!(store.pending_withdrawals(), ["b1".repeat(16)]);
    assert!(frames(&sent).iter().any(|f| f["type"] == "cert_revoke"), "and tells the server the pass is withdrawn");

    review_remove(&mut gs, &items[3]);
    assert!(!gs.chat_channels[0].voice_joined && gs.voice_active_room.is_none(), "Remove for a room leaves it");
    assert!(frames(&sent).iter().any(|f| f["type"] == "voice_room" && f["action"] == "leave" && f["room_id"] == "lounge"));

    let (url, requests) = local_server(2);
    gs.server_url = url;
    review_remove(&mut gs, &items[2]);
    let posted = requests.recv_timeout(std::time::Duration::from_secs(10)).expect("Remove for a group posts its leave");
    assert!(posted.starts_with("POST /api/v2/objects") && posted.contains("group_member_v1"), "a signed group_member_v1: {posted}");
    assert_eq!(review_items(&gs), vec![ReviewItem::Friend { key: "cy".into(), name: "Cy".into() }], "what is left is what is kept");

    gs.protected.step = Some(SetupStep::Review);
    gs.protected.chosen = Some(PinVerifier::with_salt("2580", &[4u8; 16], 1_000));
    apply(&mut gs);
    assert_eq!(gs.protected.setup.approved, ["cy"], "the friends kept are the approved list");
    tidy(&gs);
}

/// THE CHANNEL FILTER (10h): while it is on only read-only rooms are listed, the open room is
/// left out when it is a public one (the chat shows the line instead), a Commons room counts as
/// read-only when a carrier holds it so, and direct messages, groups and the scratchpad are never
/// rooms; after "Show public rooms" with the PIN everything is listed again.
/// Seen red 2026-10-10 with `hides_active_room` asking whether a listed channel of that id is
/// left out (so a channel not known yet counted as shown): "and a room not known yet" failed.
#[test]
fn only_read_only_rooms_are_listed() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(167);
    let (mut gs, _sent) = app(&me, &seed, "protected-rooms");
    let ch = |id: &str, read_only: bool, federated: bool| crate::gui::ChatChannel { id: id.into(), name: id.into(), read_only, federated, ..Default::default() };
    gs.chat_channels = vec![ch("announcements", true, false), ch("general", false, false), ch("news", true, true), ch("lobby", false, true)];
    let listed = |gs: &GuiState| gs.chat_channels.iter().filter(|c| lists_channel(gs, c)).map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(listed(&gs), ["announcements", "general", "news", "lobby"], "off: every room");
    protect(&mut gs, "7391", &[]);
    assert_eq!(listed(&gs), ["announcements", "news"], "on: the read-only rooms only");
    assert!(hides_public_rooms(&gs), "so the line shows");
    gs.chat_active_channel = "general".into();
    assert!(hides_active_room(&gs), "the open public room is left out");
    gs.chat_active_channel = "a-room-not-listed-yet".into();
    assert!(hides_active_room(&gs), "and a room not known yet");
    for open in ["announcements", "dm:ben", "p2pgroup:g1", "scratchpad", "commons:news"] {
        gs.chat_active_channel = open.into();
        assert!(!hides_active_room(&gs), "{open} is not left out");
    }
    gs.chat_active_channel = "commons:lobby".into();
    assert!(hides_active_room(&gs), "a public Commons room is left out");
    assert!(lists_commons_room(&gs, "news") && !lists_commons_room(&gs, "lobby"));

    perform(&mut gs, ProtectedAction::ShowPublicRooms);
    enter(&mut gs, "7391");
    assert_eq!(listed(&gs), ["announcements", "general", "news", "lobby"], "shown with the PIN");
    assert!(!hides_public_rooms(&gs) && !hides_active_room(&gs));
    tidy(&gs);
}

/// Pictures and files (10h): from someone who is not a friend they are left out while it is on.
/// A friend here is what the web chat counts: a mutual follow who also holds a pass from us; and
/// before this server's settings have loaded nobody is. Our own are never left out; with it off
/// nothing changes.
/// Seen red 2026-10-10 with `picture_friend` reading only `is_friend`: "a mutual follow without a
/// pass is not" failed.
#[test]
fn pictures_from_people_who_are_not_friends_are_left_out() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(168);
    let (mut gs, _sent) = app(&me, &seed, "protected-pictures");
    {
        let store = gs.dm_store.as_mut().unwrap();
        for k in ["ann", "bo"] {
            store.set_following(k, true);
            store.set_follower(k, true);
        }
        store.record_pass_sent("ann", SentPass { serial: "a1".repeat(16), may: "invite,message,trade,voice_message".into() });
        store.set_following("ivy", true);
    }
    assert!(!hides_pictures_from(&gs, "stranger"), "off: the ordinary rule");
    protect(&mut gs, "7391", &["ann"]);
    assert!(hides_pictures_from(&gs, "stranger"), "a stranger's are left out");
    assert!(hides_pictures_from(&gs, "ivy"), "a one-way follow is not a friend");
    assert!(hides_pictures_from(&gs, "bo"), "a mutual follow without a pass is not");
    assert!(!hides_pictures_from(&gs, "ann"), "a friend's follow the click-to-load rule");
    assert!(!hides_pictures_from(&gs, &me) && !hides_pictures_from(&gs, ""), "never our own, never a system line");
    tidy(&gs);
    gs.dm_store = None;
    assert!(hides_pictures_from(&gs, "ann"), "before the settings load, nobody is a friend");
}

/// Warnings show for friends too (10h): a friend's message gets the strangers' entries as well as
/// the friends' ones while it is on, in the file's order.
/// Seen red 2026-10-10 with `shown_under` ignoring `warns_friends_as_strangers`: "on: everything a
/// stranger would get" failed (the friends' two only).
#[test]
fn warnings_show_under_friends_messages_too() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(169);
    let (mut gs, _sent) = app(&me, &seed, "protected-warnings");
    gs.warnings.list = Some(crate::net::warnings::parse_warnings(crate::embedded_data::WARNINGS_JSON.as_bytes()).unwrap());
    gs.settings.warnings_on_messages = true;
    {
        let store = gs.dm_store.as_mut().unwrap();
        store.set_following("dana", true);
        store.set_follower("dana", true);
    }
    let text = "I'm an admin. Verify your wallet: buy me a gift card and let's talk on Telegram.";
    let msg = crate::gui::ChatMessage { sender_name: "Dana".into(), sender_key: "dana".into(), content: text.into(), timestamp_ms: 1, channel: "dm:dana".into(), ..Default::default() };
    let ids = |gs: &GuiState| crate::engine::warnings::shown_under(gs, &msg).into_iter().map(|w| w.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&gs), ["money", "recovery_phrase"], "off: a friend gets the friends' entries");
    protect(&mut gs, "7391", &["dana"]);
    assert_eq!(ids(&gs), ["money", "recovery_phrase", "staff_claim", "move_elsewhere"], "on: everything a stranger would get");
    tidy(&gs);
}

/// "FORGOT THE PIN?" (10h), in the prompt as on the web chat: from a PIN prompt waiting on an
/// action, this identity's recovery phrase (typed as a numbered list) lets a new PIN be chosen,
/// the setup stays on, and the prompt then asks for the NEW PIN before the action runs; a wrong
/// phrase does not, nor another identity's phrase (the setup was turned on under this one), nor a
/// locked identity. From Settings, with nothing waiting, the prompt closes after the new PIN.
/// "Change the PIN" asks for the old one, then a new one.
/// Seen red 2026-10-10 with `submit_new_pin` running the waiting action straight away: "the
/// action waits for the new PIN" failed (rooms shown before any PIN was entered).
#[test]
fn forgot_the_pin_takes_this_identitys_recovery_phrase() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(170);
    let (mut gs, _sent) = app(&me, &seed, "protected-forgot");
    protect(&mut gs, "1111", &[]);
    let labels = preset(&gs).labels;
    let phrase = crate::net::identity::mnemonic_from_seed(&seed).unwrap();

    perform(&mut gs, ProtectedAction::ShowPublicRooms);
    open_forgot(&mut gs);
    assert_eq!(gs.protected.prompt, Some(PinPrompt { action: Some(ProtectedAction::ShowPublicRooms), mode: PromptMode::Forgot }), "the action keeps waiting");
    let mut wrong: Vec<&str> = phrase.split(' ').collect();
    wrong[23] = if wrong[23] == "zoo" { "abandon" } else { "zoo" };
    gs.protected.forgot_phrase = wrong.join(" ");
    assert!(!submit_phrase(&mut gs), "a wrong phrase does not");
    assert_eq!(gs.protected.prompt_line, labels.phrase_wrong);
    assert!(gs.protected.forgot_phrase.is_empty(), "the typed words are cleared");

    let numbered: String = phrase.split(' ').enumerate().map(|(i, w)| format!("{}. {w} ", i + 1)).collect();
    gs.protected.forgot_phrase = numbered;
    assert!(submit_phrase(&mut gs), "the right phrase does");
    assert_eq!(gs.protected.prompt.as_ref().map(|p| p.mode), Some(PromptMode::NewPin), "a new PIN may be chosen");
    gs.protected.pin_first = "2222".into();
    gs.protected.pin_second = "2223".into();
    assert!(!submit_new_pin(&mut gs));
    assert_eq!(gs.protected.prompt_line, preset(&gs).pin_rule(), "two that differ: the rule");
    gs.protected.pin_first = "2222".into();
    gs.protected.pin_second = "2222".into();
    assert!(submit_new_pin(&mut gs));
    assert!(is_on(&gs), "the setup stays on");
    assert!(gs.protected.setup.public_rooms_hidden, "the action waits for the new PIN");
    assert_eq!(gs.protected.prompt, Some(PinPrompt { action: Some(ProtectedAction::ShowPublicRooms), mode: PromptMode::Pin }), "the prompt asks for it");
    enter(&mut gs, "1111");
    assert!(gs.protected.setup.public_rooms_hidden, "the old PIN no longer opens it");
    enter(&mut gs, "2222");
    assert!(!gs.protected.setup.public_rooms_hidden && gs.protected.prompt.is_none(), "the new one does, and the action runs");

    // From Settings, nothing waiting: the prompt closes after the new PIN.
    open_forgot(&mut gs);
    gs.protected.forgot_phrase = phrase.clone();
    assert!(submit_phrase(&mut gs));
    gs.protected.pin_first = "3333".into();
    gs.protected.pin_second = "3333".into();
    assert!(submit_new_pin(&mut gs));
    assert!(gs.protected.prompt.is_none(), "nothing waiting: it closes");

    // "Change the PIN": the old one first, then a new one.
    perform(&mut gs, ProtectedAction::ChangePin);
    enter(&mut gs, "3333");
    gs.protected.pin_first = "4444".into();
    gs.protected.pin_second = "4444".into();
    assert!(submit_new_pin(&mut gs) && gs.protected.prompt.is_none());
    assert_eq!(try_pin(&mut gs, "4444", now_secs()), PinTry::Right, "the changed PIN opens it");

    // A new identity made on this device: its own phrase does not open a setup turned on under
    // the first one. A locked identity cannot be checked at all.
    let (other_seed, other) = identity(171);
    gs.profile_public_key = other;
    gs.private_key_bytes = Some(other_seed.clone());
    open_forgot(&mut gs);
    gs.protected.forgot_phrase = crate::net::identity::mnemonic_from_seed(&other_seed).unwrap();
    assert!(!submit_phrase(&mut gs), "another identity's phrase does not");
    assert_eq!(gs.protected.prompt_line, labels.phrase_wrong);
    // A missing or malformed recorded identity (only damage leaves one) falls back to the identity
    // in use, so a damaged setup can still be opened by its phrase rather than locking for good;
    // the web chat follows the same rule. Seen red 2026-10-10 with a malformed identity compared
    // as recorded: "a malformed recorded identity falls back" failed.
    for damaged in ["", "not a key!"] {
        gs.protected.setup.identity = damaged.to_string();
        open_forgot(&mut gs);
        gs.protected.forgot_phrase = crate::net::identity::mnemonic_from_seed(&other_seed).unwrap();
        assert!(submit_phrase(&mut gs), "a malformed recorded identity falls back to the one in use ({damaged:?})");
        cancel_prompt(&mut gs);
    }
    gs.protected.setup.identity = me.clone();
    gs.profile_public_key = me;
    gs.private_key_bytes = None;
    gs.protected.forgot_phrase = phrase.clone();
    assert!(!submit_phrase(&mut gs), "a locked identity cannot be checked");
    cancel_prompt(&mut gs);
    tidy(&gs);
}

/// PER DEVICE, NEVER SYNCED (10h): with the setup on, the self-sync note Block deposits for our
/// other devices holds exactly the block and the key, and nothing of the setup (no word of it, not
/// its salt or hash); the export request carries only the key, the time and the signature.
/// Seen red 2026-10-10 with block.rs `note_put` appending " protected" to the note's text: "the
/// note is the block and the key, nothing else" failed.
#[test]
fn the_setup_stays_out_of_the_self_sync_notes_and_the_export() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(172);
    let (mut gs, sent) = app(&me, &seed, "protected-sync");
    protect(&mut gs, "7391", &["ben"]);
    let verifier = gs.protected.setup.pin.clone().unwrap();

    crate::engine::block::block(&mut gs, "ben");
    let out = frames(&sent);
    let note = out.iter().find(|f| f["type"] == "dm_put" && f["to"] == me.as_str()).expect("the note to our other devices");
    let mine = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&seed).unwrap();
    let inner = crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&mine, note["content"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(inner.text, "[[hum:block:v1]]ben", "the note is the block and the key, nothing else");
    for f in &out {
        let text = f.to_string();
        assert!(!text.to_lowercase().contains("protect") && !text.contains(&verifier.salt) && !text.contains(&verifier.hash), "no frame carries the setup: {text}");
    }

    let export = crate::gui::pages::settings::account_export_request(&me, &seed, 1_791_504_000_000);
    let mut keys: Vec<&str> = export.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["key", "sig", "timestamp"], "the export request is the key, the time and the signature");
    tidy(&gs);
}

/// THE ROUTES THE NEW WORDS NAME ARE LOCKED (the 2026-10-10 review, items 6 and 12): making a
/// friend code (the button and a typed `/friend-code`), starting a group and making a group's
/// invite ticket are each refused without the PIN, with nothing sent or made and the prompt open
/// for exactly that action; a wrong PIN does nothing more; the right one does it, through the
/// action's own entry point.
/// Seen red 2026-10-10 four ways, one gate at a time taken out: with `request_code` asking no
/// gate, and again with `typed_command` not knowing `/friend-code`, "MakeFriendCode is refused
/// without the PIN" failed; with `create_p2p_group` asking none, "StartGroup is refused without
/// the PIN" failed; with `invite_to_group` asking none, "InviteToGroup(\"g1\", \"Book club\") is
/// refused without the PIN" failed.
#[test]
fn starting_or_inviting_to_a_group_and_making_a_friend_code_need_the_pin() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(178);
    let (mut gs, sent) = app(&me, &seed, "protected-groups");
    gs.new_group_name = "Book club".into();
    protect(&mut gs, "7391", &[]);
    let preset = preset(&gs);
    assert!(preset.status_line.contains("starting a group") && preset.routes_line.contains("inviting someone to a group"), "the words that name these routes");

    type Done = fn(&GuiState, &[serde_json::Value]) -> bool;
    let cases: Vec<(ProtectedAction, Box<dyn Fn(&mut GuiState)>, Done)> = vec![
        (
            ProtectedAction::MakeFriendCode,
            Box::new(|gs: &mut GuiState| {
                crate::engine::friend_code::request_code(gs);
            }),
            |_: &GuiState, f: &[serde_json::Value]| f.iter().any(|f| f["content"] == "/friend-code"),
        ),
        (
            ProtectedAction::MakeFriendCode,
            Box::new(|gs: &mut GuiState| {
                if allows_typed(gs, "/friend-code") {
                    crate::gui::pages::chat::send_slash_command(gs, "/friend-code");
                }
            }),
            |_: &GuiState, f: &[serde_json::Value]| f.iter().any(|f| f["content"] == "/friend-code"),
        ),
        (
            ProtectedAction::StartGroup,
            Box::new(|gs: &mut GuiState| crate::gui::pages::chat::create_p2p_group(gs)),
            |gs: &GuiState, _: &[serde_json::Value]| gs.create_group_status.starts_with("Create failed"),
        ),
        (
            ProtectedAction::InviteToGroup("g1".into(), "Book club".into()),
            Box::new(|gs: &mut GuiState| {
                let _ = crate::gui::pages::chat::invite_to_group(gs, "g1", "Book club");
            }),
            |gs: &GuiState, _: &[serde_json::Value]| gs.p2p_group_invite_status.starts_with("Invite failed"),
        ),
    ];
    for (action, run, done) in cases {
        gs.create_group_status.clear();
        gs.p2p_group_invite_status.clear();
        run(&mut gs);
        let out = frames(&sent);
        assert!(!done(&gs, &out), "{action:?} is refused without the PIN");
        assert!(out.is_empty(), "{action:?}: nothing is sent without the PIN: {out:?}");
        assert_eq!(waiting(&gs), Some(action.clone()), "{action:?}: the PIN prompt opens for it");
        enter(&mut gs, "0000");
        assert!(!done(&gs, &frames(&sent)), "{action:?}: a wrong PIN does nothing");
        enter(&mut gs, "7391");
        assert!(done(&gs, &frames(&sent)), "{action:?} is done with the PIN ({} / {})", gs.create_group_status, gs.p2p_group_invite_status);
        assert!(gs.protected.granted.is_none() && gs.protected.prompt.is_none(), "{action:?}: nothing left over");
    }
    tidy(&gs);
}

/// A CONTACT REQUEST FROM SOMEONE WE ALREADY FOLLOW (the 2026-10-10 review, item 5): with the
/// setup on and them not on its approved list, the request does NOT complete the friendship by
/// itself (which stored their pass and counted them a friend while our own pass to them was
/// refused, leaving them stuck): it is listed, the Requests list says "Accept (needs the PIN)",
/// and Accept asks for the PIN, after which they are a friend and our pass goes to them. Someone
/// on the approved list still completes at once, as with the setup off.
/// Seen red 2026-10-10 with `contact_request_in` ignoring `may_complete`: "listed, not
/// completed" failed.
#[test]
fn a_request_from_someone_followed_is_listed_for_the_pin() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(179);
    let (ann_seed, ann) = identity(180);
    let (cy_seed, cy) = identity(181);
    let (mut gs, sent) = app(&me, &seed, "protected-asked");
    gs.peer_kyber_keys.insert(ann.clone(), crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&ann_seed).unwrap().public_base64());
    {
        let store = gs.dm_store.as_mut().unwrap();
        store.set_following(&ann, true); // followed before the setup
        store.set_following(&cy, true);
    }
    protect(&mut gs, "7391", &[&cy]);
    let _ = frames(&sent);

    // Ann and Cy each send us a contact request, as their own apps make them.
    let mine = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&seed).unwrap();
    let request_from = |asker_seed: &[u8], key: &str, tag: &str| {
        let (mut asker, _s) = app(key, asker_seed, tag);
        asker.user_name = "Asker".into();
        asker.peer_kyber_keys.insert(me.clone(), mine.public_base64());
        let (theirs, _ours, _pass) = crate::engine::reach::contact_request_puts(&asker, &me).expect("a request");
        tidy(&asker);
        crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&mine, theirs["content"].as_str().unwrap()).unwrap()).unwrap()
    };
    let from_ann = request_from(&ann_seed, &ann, "protected-asked-ann");
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &from_ann), "a request is never a message");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.requests().iter().any(|r| r.key == ann), "listed, not completed");
    assert!(!store.is_follower(&ann) && store.cert_for(&ann).is_none(), "her pass is not taken until Accept");
    assert!(frames(&sent).is_empty(), "and nothing goes to her");
    assert_eq!(crate::gui::pages::safety_protected::accept_label(&mut gs), preset(&gs).accept_needs_pin, "Accept (needs the PIN)");

    crate::engine::reach::accept_request(&mut gs, &ann);
    assert_eq!(waiting(&gs), Some(ProtectedAction::AcceptRequest(ann.clone())), "Accept asks for the PIN");
    enter(&mut gs, "7391");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.is_friend(&ann) && store.requests().is_empty(), "with it, she is a friend");
    assert!(gs.protected.setup.is_approved(&ann) && store.cert_sent_to(&ann), "approved, and our pass goes to her");

    let from_cy = request_from(&cy_seed, &cy, "protected-asked-cy");
    crate::engine::dm::ingest_dm(&mut gs, &from_cy);
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.is_follower(&cy) && store.requests().iter().all(|r| r.key != cy), "someone approved still completes at once");
    tidy(&gs);
}

/// THE RECOVERY PHRASE'S LOCKED CONTROLS AND THE SETUP'S SECTION SAY ONLY THE PRESET'S WORDS (the
/// 2026-10-10 review, item 9): while the setup is on, Settings' phrase and device-link QR show the
/// preset's `show_phrase` button and `phrase_needs_pin` line (both from Settings' code, which asks
/// `phrase_lock` for each); after the PIN, nothing is locked; off, nothing is locked. A preset that
/// could not be read shows no words of its own: no parser's message, in the section or its line.
/// Seen red 2026-10-10 two ways: with `phrase_lock` answering the old typed words "Show Recovery
/// Phrase", "the preset's button and line" failed; and with the section drawing a parser's message
/// again (`body_hint`), "nothing is drawn when the preset cannot be read" failed.
#[test]
fn the_phrase_lock_and_the_section_say_only_the_presets_words() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(182);
    let (mut gs, _sent) = app(&me, &seed, "protected-phrase");
    assert_eq!(crate::gui::pages::safety_protected::phrase_lock(&mut gs), None, "off: nothing locked");
    protect(&mut gs, "7391", &[]);
    let labels = preset(&gs).labels;
    let lock = crate::gui::pages::safety_protected::phrase_lock(&mut gs).expect("on: locked");
    assert_eq!((lock.button.as_str(), lock.line.as_str()), (labels.show_phrase.as_str(), labels.phrase_needs_pin.as_str()), "the preset's button and line");
    assert_eq!(labels.show_phrase, "Show the recovery phrase");
    let settings_src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/gui/pages/settings.rs")).unwrap();
    assert_eq!(settings_src.matches("safety_protected::phrase_lock(state)").count(), 2, "the phrase and the QR each ask it");
    assert!(!settings_src.contains("\"Show device-link QR\")"), "no locked button typed into Settings");
    perform(&mut gs, ProtectedAction::ShowRecoveryPhrase);
    enter(&mut gs, "7391");
    assert_eq!(crate::gui::pages::safety_protected::phrase_lock(&mut gs), None, "after the PIN: shown");
    tidy(&gs);

    // A preset that could not be read: the section draws nothing at all, and Turn on says nothing.
    let mut broken = GuiState::default();
    broken.protected.preset_failed = true;
    begin(&mut broken);
    assert!(broken.protected.line.is_empty() && broken.protected.step.is_none(), "turning it on says nothing and does not start");
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| crate::gui::pages::safety_protected::draw_protected_section(ui, &theme, &mut broken, theme.accent()));
    });
    let mut texts = Vec::new();
    fn walk(shape: &egui::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => texts.push(t.galley.job.text.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
    assert!(texts.is_empty(), "nothing is drawn when the preset cannot be read: {texts:?}");
}

/// THE ALWAYS-VISIBLE LINE AND THE ROUTES LINE ARE THE FILE'S (the 2026-10-10 review, item 12: the
/// words changed to name groups): Settings > Safety draws data/gui/safety_presets.json's
/// `status_line` at its top and its `routes_line` in the section, read from the data file on disk
/// (the built-in copy is held to be the same bytes).
/// Seen red 2026-10-10 with `draw_status_line` drawing the old sentence typed into the code
/// instead of the preset's: "the file's status line is drawn" failed.
#[test]
fn the_status_and_routes_lines_are_the_files() {
    crate::config::keep_saves_off_disk();
    let (seed, me) = identity(183);
    let (mut gs, _sent) = app(&me, &seed, "protected-lines");
    protect(&mut gs, "7391", &[]);
    let file = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/gui/safety_presets.json")).unwrap();
    let on_disk = crate::net::protected::parse_presets(&file).unwrap();
    assert!(on_disk.status_line.contains("direct messages") && on_disk.status_line.contains("starting a group"), "the corrected words");
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 1600.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::gui::pages::safety_protected::draw_status_line(ui, &theme, &mut gs);
            crate::gui::pages::safety_protected::draw_protected_section(ui, &theme, &mut gs, theme.accent());
        });
    });
    for (what, line) in [("status line", &on_disk.status_line), ("routes line", &on_disk.routes_line)] {
        assert!(crate::gui::screen_surface::find_text_in_shapes(&out.shapes, line).is_some_and(|f| f.text == *line), "the file's {what} is drawn");
    }
    tidy(&gs);
}
