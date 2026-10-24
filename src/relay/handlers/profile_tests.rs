// Child of profile.rs (#[path], so `use super::*` reaches everything it has): the profile
// field `accepts_files`, who a person accepts files in messages from (2026-10-11,
// docs/design/files-showcase.md "Files on the sender's device and in messages, exact", part 3),
// the person's own copy, and what the lookups show. Was msg_handlers_profile_tests.rs, a child of
// msg_handlers.rs, until the profile handlers moved to profile.rs (2026-10-12, BUG-191 Wave 0).
//
// Since BUG-191 a save no longer writes the copy kept for other servers (the person's own app
// signs it after the save), so a test that looks the profile up at /api/profile/{key} without
// going through the signing stores a copy itself. The tests at the end (BUG-191 Wave 1A, plan
// 3.3, 3.5 and 3.9) go through it: a save asks each device, the test plays the person's app
// (`app_signs`, what both apps do with an own copy that asks), and a fake linked server
// (`link_a_server`) receives what is passed on. Those tests need real keys: a copy is asked for
// and kept only for a key with the shape people's keys have (3,904 hex characters).

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use crate::relay::core::profile_copy::{is_withdrawal, next_copy_time, public_copy, sign_copy, verify_copy, withdrawal, PublicCopy};
use crate::relay::relay::{FederatedConnection, Peer, RelayState};
use crate::relay::storage::{now_millis, Storage};
use serde_json::{json, Value};

fn fresh_state() -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp("profile_files")))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// `key` signed in as `name` (a profile is saved under the socket's display name).
fn sign_in(st: &Arc<RelayState>, key: &str, name: &str) {
    st.db.register_name(name, key).unwrap();
    st.db.join_server(key, name).unwrap();
    online_as(st, key, name);
}

/// `key`'s socket is open and shows `name`, registered or not (a placeholder name is not).
fn online_as(st: &Arc<RelayState>, key: &str, name: &str) {
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer { public_key_hex: key.to_string(), display_name: Some(name.to_string()), upload_token: None, conn_id: 1 },
        );
    });
}

/// `device` linked to `owner`'s name with a link code (its own key under the name), and online.
fn link_device(st: &Arc<RelayState>, name: &str, owner: &str, device: &str) {
    let code = st.db.create_link_code(name, owner).unwrap();
    assert_eq!(st.db.redeem_link_code(&code, device).unwrap().as_deref(), Some(name), "test setup: the device is linked");
    st.db.join_server(device, name).unwrap();
    online_as(st, device, name);
}

/// A person's key as their app has it: the Dilithium3 key of the seed `[seed; 32]`.
fn key_of(seed: u8) -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32])).public_key())
}

/// A server this one is linked with: a fake link whose frames land in the returned receiver.
fn link_a_server(st: &Arc<RelayState>, id: &str) -> tokio::sync::mpsc::UnboundedReceiver<String> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let link = FederatedConnection { tx, server_id: id.to_string(), server_name: id.to_string(), trust_tier: 1, connected_at: Instant::now() };
    block(async { st.federation_connections.write().await.insert(id.to_string(), link) });
    rx
}

/// The `profile_gossip` frames a linked server has received since last asked: on their own (a
/// link's catch-up) or carried by a numbered frame (federation_log.rs, since 2026-10-22).
fn gossip_got(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<Value> {
    let mut got = Vec::new();
    while let Ok(text) = rx.try_recv() {
        let frame: Value = serde_json::from_str(&text).expect("a frame is JSON");
        let frame = if frame["type"] == "fed_event" { frame["event"].clone() } else { frame };
        if frame["type"] == "profile_gossip" {
            got.push(frame);
        }
    }
    got
}

/// The person's own copy among `sent` for `key` (the one carrying the privacy map), as the wire
/// carries it.
fn own_copy_for(sent: &[RelayMessage], key: &str) -> Value {
    sent.iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .find(|w| w["type"] == "profile_data" && w["target"] == key && w.get("privacy").is_some())
        .unwrap_or_else(|| panic!("no own copy for {}: {sent:?}", &key[..8.min(key.len())]))
}

/// What the person's app does with an own copy that asks (plan 3.10 and 3.11): the public copy of
/// the frame's name and fields under its privacy map (a field the frame leaves out is empty),
/// dated after the newest seen, signed with the seed `[seed; 32]`. Returns `(at, signature)`.
fn app_signs(frame: &Value, seed: u8, last_here: u64) -> (u64, String) {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    let copy = public_copy(&s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"), &s("pronouns"), &s("location"), &s("website"), &s("privacy"));
    let at = next_copy_time(now_millis(), frame["signed_at"].as_u64().unwrap_or(0), last_here);
    (at, sign_copy(&[seed; 32], &key_of(seed), &copy, at))
}

/// `key` sends `profile_sign`; what the relay sent (to people) while handling it.
fn send_sign(st: &Arc<RelayState>, key: &str, at: u64, signature: &str) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_profile_sign(st, key, at, signature.to_string()));
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        out.push(m);
    }
    out
}

/// The copy a gossip frame carries.
fn copy_in(frame: &Value) -> PublicCopy {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    PublicCopy {
        name: s("name"),
        bio: s("bio"),
        avatar_url: s("avatar_url"),
        banner_url: s("banner_url"),
        socials: s("socials"),
        pronouns: s("pronouns"),
        location: s("location"),
        website: s("website"),
    }
}

/// `GET /api/profile/{key}`: the status and the body.
fn lookup(st: &Arc<RelayState>, key: &str) -> (axum::http::StatusCode, Value) {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    let resp = block(crate::relay::api::get_signed_profile(State(st.clone()), Path(key.to_string()))).into_response();
    let status = resp.status();
    let bytes = block(axum::body::to_bytes(resp.into_body(), usize::MAX)).unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// `key` sends `frame`, the JSON an app writes, parsed as the relay parses it; what the relay
/// sent while handling it. `last` is the socket's profile rate-limit slot.
fn send_with(st: &Arc<RelayState>, key: &str, frame: serde_json::Value, last: &mut Option<Instant>) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    let msg: RelayMessage = serde_json::from_value(frame).expect("a profile frame parses");
    block(async {
        match msg {
            RelayMessage::ProfileUpdate { bio, socials, avatar_url, banner_url, pronouns, location, website, privacy, accepts_files } => {
                handle_profile_update(st, key, last, bio, socials, avatar_url, banner_url, pronouns, location, website, privacy, accepts_files).await
            }
            RelayMessage::ProfileRequest { name, friend_cert } => handle_profile_request(st, key, name, friend_cert).await,
            _ => panic!("not a profile frame"),
        }
    });
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        out.push(m);
    }
    out
}

/// As `send_with`, from a socket that has not updated its profile before (the 30-second limit is
/// not what these tests are about).
fn send(st: &Arc<RelayState>, key: &str, frame: serde_json::Value) -> Vec<RelayMessage> {
    send_with(st, key, frame, &mut None)
}

/// Each ProfileData in `sent`: (who it is for, `accepts_files` as the wire carries it).
fn profile_datas(sent: &[RelayMessage]) -> Vec<(Option<String>, Option<String>)> {
    sent.iter()
        .filter(|m| matches!(m, RelayMessage::ProfileData { .. }))
        .map(|m| {
            let wire = serde_json::to_value(m).unwrap();
            let target = wire["target"].as_str().map(str::to_string);
            (target, wire["accepts_files"].as_str().map(str::to_string))
        })
        .collect()
}

/// The notices in `sent` for `key`.
fn notices(sent: &[RelayMessage], key: &str) -> Vec<String> {
    sent.iter()
        .filter_map(|m| match m {
            RelayMessage::Private { to, message } if to == key => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn some(word: &str) -> Option<String> {
    Some(word.to_string())
}

/// The copy for other servers of what this server stored for `name`, as the person's app signs it
/// after the echo of their save (a save no longer writes one: BUG-191).
fn signed_copy_of(st: &Arc<RelayState>, name: &str) -> PublicCopy {
    let (bio, socials, avatar, banner, pronouns, location, website, privacy) =
        st.db.get_profile_extended(name).unwrap().expect("a stored profile");
    public_copy(name, &bio, &avatar, &banner, &socials, &pronouns, &location, &website, &privacy)
}

/// A profile update that names the word stores it, and every profile answer carries it on the
/// wire: both ProfileData the update sends (the public one and the person's own), another
/// person's `profile_request` (no friendship pass needed: it is published so a stranger's app can
/// say so first), and the answer for someone with no profile at all (the default).
///
/// Seen red 2026-10-11 with `None` passed for the word to `save_profile_extended` in
/// `handle_profile_update`: "the update's two ProfileData carry the word: [(None,
/// Some(\"friends\")), (Some(\"ann_key\"), Some(\"friends\"))]".
#[test]
fn a_profile_update_sets_it_and_every_profile_answer_carries_it() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    sign_in(&st, "bob_key", "Bob");

    let sent = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "accepts_files": "anyone" }));
    let datas = profile_datas(&sent);
    assert_eq!(datas, vec![(None, some("anyone")), (some("ann_key"), some("anyone"))], "the update's two ProfileData carry the word: {datas:?}");
    assert!(notices(&sent, "ann_key").is_empty(), "{:?}", notices(&sent, "ann_key"));
    assert_eq!(st.db.accepts_files_of("Ann"), "anyone");

    let answer = send(&st, "bob_key", serde_json::json!({ "type": "profile_request", "name": "Ann" }));
    assert_eq!(profile_datas(&answer), vec![(some("bob_key"), some("anyone"))], "a stranger's profile_request is answered with it");

    let answer = send(&st, "ann_key", serde_json::json!({ "type": "profile_request", "name": "Bob" }));
    assert_eq!(profile_datas(&answer), vec![(some("ann_key"), some("friends"))], "no profile at all answers the default");
}

/// A word that is not one of the three (here a capital letter, as a careless app might send)
/// REFUSES THE WHOLE UPDATE with a notice that names the word and the three it may be, and
/// changes nothing: the stored word and the rest of the profile stay, no ProfileData goes out.
/// The refusal does not spend the 30-second limit, so the corrected update goes through at once.
///
/// Seen red 2026-10-11 with the word check taken out of `handle_profile_update` (the storage
/// still refused it, with its general "Failed to save profile."): "the notice names the word and
/// the three: [\"Failed to save profile.\"]".
#[test]
fn a_word_that_is_not_one_of_the_three_refuses_the_whole_update() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "first", "socials": "{}", "accepts_files": "nobody" }));

    let mut last = None;
    let sent = send_with(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "second", "socials": "{}", "accepts_files": "Friends" }), &mut last);
    let told = notices(&sent, "ann_key");
    assert!(
        told.len() == 1 && told[0].contains("not saved") && told[0].contains("\"Friends\"") && told[0].contains("nobody, friends, anyone"),
        "the notice names the word and the three: {told:?}"
    );
    assert!(profile_datas(&sent).is_empty(), "nothing was sent as saved");
    assert_eq!(st.db.accepts_files_of("Ann"), "nobody", "the stored word stays");
    assert_eq!(st.db.get_profile("Ann").unwrap().map(|p| p.0).as_deref(), Some("first"), "and so does the rest");

    let sent = send_with(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "second", "socials": "{}", "accepts_files": "friends" }), &mut last);
    assert_eq!(profile_datas(&sent).len(), 2, "the corrected update goes through at once: {:?}", notices(&sent, "ann_key"));
    assert_eq!(st.db.accepts_files_of("Ann"), "friends");
}

/// An older app (v0.1485.1 and before) sends a profile update without the field: the rest of the
/// profile saves, the stored word is KEPT, and the answers carry the kept word. Someone whose
/// first profile comes from an older app gets the default.
///
/// Seen red 2026-10-11 with the upsert's `COALESCE(?10, accepts_files)` changed to
/// `COALESCE(?10, ?11)` (not sent resets to the default): "an older app's update keeps the
/// word: [(None, Some(\"friends\")), (Some(\"ann_key\"), Some(\"friends\"))]".
#[test]
fn an_older_apps_update_without_the_field_keeps_the_stored_word() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    sign_in(&st, "cy_key", "Cy");
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "first", "socials": "{}", "accepts_files": "nobody" }));

    let older = |bio: &str| serde_json::json!({ "type": "profile_update", "bio": bio, "socials": "{}" });
    let sent = send(&st, "ann_key", older("changed by an older app"));
    let datas = profile_datas(&sent);
    assert_eq!(datas, vec![(None, some("nobody")), (some("ann_key"), some("nobody"))], "an older app's update keeps the word: {datas:?}");
    assert_eq!(st.db.accepts_files_of("Ann"), "nobody");
    assert_eq!(st.db.get_profile("Ann").unwrap().map(|p| p.0).as_deref(), Some("changed by an older app"), "the rest saved");

    let sent = send(&st, "cy_key", older("new here"));
    assert_eq!(profile_datas(&sent), vec![(None, some("friends")), (some("cy_key"), some("friends"))], "a first profile from an older app takes the default");
}

/// The two HTTP lookups that carry profile fields carry it too: `/api/members/{key}`, and
/// `/api/profile/{key}` for a key with an account on this server. A profile gossiped from
/// another server (no account here) is answered without it: this server does not know.
///
/// Seen red 2026-10-11 with the `accepts_files` lookup taken out of `api::get_signed_profile`:
/// "the signed profile of a local account carries it: null".
#[test]
fn the_member_and_profile_lookups_carry_it() {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "accepts_files": "nobody" }));
    let now = crate::relay::storage::now_millis();
    st.db.store_signed_profile("ann_key", &signed_copy_of(&st, "Ann"), now, "sig").unwrap();
    let far = PublicCopy { name: "Far".into(), bio: "from a peer server".into(), socials: "{}".into(), ..Default::default() };
    st.db.store_signed_profile("far_key", &far, now, "sig").unwrap();

    let body = |resp: axum::response::Response| -> serde_json::Value {
        let bytes = block(axum::body::to_bytes(resp.into_body(), usize::MAX)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let member = block(crate::relay::api::get_member_by_key(State(st.clone()), Path("ann_key".to_string()))).expect("a listed member");
    assert_eq!(member.0["accepts_files"], "nobody", "the member lookup carries it: {}", member.0);

    let signed = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("ann_key".to_string()))).into_response());
    assert_eq!(signed["accepts_files"], "nobody", "the signed profile of a local account carries it: {}", signed["accepts_files"]);

    let far = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("far_key".to_string()))).into_response());
    assert_eq!(far["bio"], "from a peer server");
    assert!(far.get("accepts_files").is_none(), "a gossiped profile is answered without it: {far}");
}

/// The person's privacy map rides THEIR OWN copy (the echo of their save, `target` = their key),
/// so their other apps can send the whole profile back (a profile update is saved whole,
/// BUG-194); the public copy and an answer to anyone else never carry it (their own request is
/// answered with it: `asking_for_your_own_profile_gets_your_complete_copy`).
///
/// Seen red 2026-10-11 with the own copy's `privacy` put back to `None` in
/// `handle_profile_update`: "the own copy carries the privacy map: [(None, None), (Some(\"ann_key\"),
/// None)]".
#[test]
fn only_the_persons_own_copy_carries_their_privacy_map() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    sign_in(&st, "bob_key", "Bob");
    let privacy = r#"{"directory":"unlisted","location":"friends"}"#;
    let sent = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "privacy": privacy }));
    let wires: Vec<(Option<String>, Option<String>)> = sent
        .iter()
        .filter(|m| matches!(m, RelayMessage::ProfileData { .. }))
        .map(|m| {
            let w = serde_json::to_value(m).unwrap();
            (w["target"].as_str().map(str::to_string), w["privacy"].as_str().map(str::to_string))
        })
        .collect();
    assert_eq!(wires, vec![(None, None), (some("ann_key"), some(privacy))], "the own copy carries the privacy map: {wires:?}");

    let answer = send(&st, "bob_key", serde_json::json!({ "type": "profile_request", "name": "Ann" }));
    let w = serde_json::to_value(&answer[0]).unwrap();
    assert_eq!(w["type"], "profile_data");
    assert!(w.get("privacy").is_none(), "an answer to someone else never carries it: {w}");
}

/// The bio limit is 280 CHARACTERS, as the refusal says: a bio in Greek (two bytes a letter) of 200
/// letters saves; 281 characters do not.
///
/// Seen red 2026-10-11 with the limit counted in bytes (as it was): "200 Greek letters save" failed.
#[test]
fn the_bio_limit_counts_characters() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    let greek: String = std::iter::repeat('α').take(200).collect();
    let sent = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": greek, "socials": "{}" }));
    assert!(notices(&sent, "ann_key").is_empty(), "200 Greek letters save: {:?}", notices(&sent, "ann_key"));
    assert_eq!(st.db.get_profile("Ann").unwrap().map(|p| p.0).as_deref(), Some(greek.as_str()));
    let long: String = std::iter::repeat('a').take(281).collect();
    let sent = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": long, "socials": "{}" }));
    assert!(notices(&sent, "ann_key").iter().any(|n| n.contains("Bio too long")), "281 characters do not");
}

/// The own copy reaches every device signed in under the name, a linked one with its own key
/// included, so its next save does not undo this one (the relay review of 2026-10-11).
///
/// Seen red 2026-10-11 with the echo sent to the saving key alone: "the linked phone gets the own
/// copy" failed.
#[test]
fn the_persons_own_copy_reaches_each_of_their_devices() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    let code = st.db.create_link_code("Ann", "ann_key").unwrap();
    assert_eq!(st.db.redeem_link_code(&code, "ann_phone").unwrap().as_deref(), Some("Ann"), "test setup: the phone is linked");
    let sent = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "privacy": "{\"directory\":\"unlisted\"}" }));
    let own: Vec<String> = sent
        .iter()
        .map(|m| serde_json::to_value(m).unwrap())
        .filter(|w| w["type"] == "profile_data" && w.get("privacy").is_some())
        .filter_map(|w| w["target"].as_str().map(str::to_string))
        .collect();
    assert!(own.contains(&"ann_key".to_string()), "the saving device gets it: {own:?}");
    assert!(own.contains(&"ann_phone".to_string()), "the linked phone gets the own copy: {own:?}");
}

/// Asking for your OWN profile (an app that switched to this server without a fresh connect has had
/// no own copy from it) gets the complete copy, privacy map and private fields included; someone
/// else asking gets neither; a person with no profile here gets an empty one with an empty map.
///
/// Seen red 2026-10-11 without the own-profile answer in `handle_profile_request`: "her own
/// request is answered with her privacy map" failed.
#[test]
fn asking_for_your_own_profile_gets_your_complete_copy() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    sign_in(&st, "bob_key", "Bob");
    let privacy = r#"{"location":"private"}"#;
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "privacy": privacy }));

    let answer = send(&st, "ann_key", serde_json::json!({ "type": "profile_request", "name": "Ann" }));
    let w = serde_json::to_value(&answer[0]).unwrap();
    assert_eq!(w["privacy"], privacy, "her own request is answered with her privacy map: {w}");
    assert_eq!(w["location"], "Lisbon", "and the field she keeps for friends");
    assert_eq!(w["target"], "ann_key");

    let answer = send(&st, "bob_key", serde_json::json!({ "type": "profile_request", "name": "Ann" }));
    let w = serde_json::to_value(&answer[0]).unwrap();
    assert!(w.get("privacy").is_none() && w.get("location").is_none(), "Bob gets neither: {w}");

    let answer = send(&st, "bob_key", serde_json::json!({ "type": "profile_request", "name": "Bob" }));
    let w = serde_json::to_value(&answer[0]).unwrap();
    assert_eq!((w["privacy"].as_str(), w["bio"].as_str()), (Some("{}"), Some("")), "no profile yet: an empty copy: {w}");
}

/// A field the person keeps for friends stays out of the HTTP lookups anyone can call
/// (`/api/members/{key}` and `/api/profile/{key}`) and out of the copy kept for other servers;
/// a public one is shown. Also a copy cached before this rule is answered by the privacy map as
/// it is now.
///
/// Since BUG-191 the copy for other servers is the person's app's signed public copy of what was
/// stored, which this test stores itself (2026-10-12).
///
/// Seen red 2026-10-11 with the lookups as they were (the relay review of that day): "the member
/// lookup leaves out a private location" failed with "Lisbon".
#[test]
fn private_fields_stay_out_of_the_lookups() {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "pronouns": "she", "privacy": "{\"location\":\"private\"}" }));
    let kept = signed_copy_of(&st, "Ann");
    assert_eq!(kept.location, "", "the copy for other servers leaves it out");
    let now = crate::relay::storage::now_millis();
    st.db.store_signed_profile("ann_key", &kept, now, "sig").unwrap();
    let body = |resp: axum::response::Response| -> serde_json::Value {
        let bytes = block(axum::body::to_bytes(resp.into_body(), usize::MAX)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let member = block(crate::relay::api::get_member_by_key(State(st.clone()), Path("ann_key".to_string()))).expect("a member");
    assert_eq!(member.0["location"], "", "the member lookup leaves out a private location: {}", member.0);
    let signed = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("ann_key".to_string()))).into_response());
    assert_eq!(signed["location"], "", "the profile lookup too: {signed}");
    assert_eq!(signed["pronouns"], "she", "a public field is shown");

    // A copy cached before the rule, still holding the location.
    let older = PublicCopy { location: "Lisbon".to_string(), ..kept };
    st.db.store_signed_profile("ann_key", &older, now + 1, "sig").unwrap();
    let signed = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("ann_key".to_string()))).into_response());
    assert_eq!(signed["location"], "", "an older cached copy is answered by the map as it is now");
}

// `a_copy_for_someone_no_longer_here_leaves_out_the_private_fields` (2026-10-11) was deleted on
// 2026-10-12 (BUG-191 Wave 0): it stored this server's own UNSIGNED copy of someone with no account
// here and checked that /api/profile/{key} left out their friends-only fields. That case can no
// longer arise: the save stores no copy, the storage refuses an unsigned one, and the open drops
// the old unsigned rows (storage/signed_profiles.rs). Every copy held now is a public copy its
// owner signed, which leaves those fields out before it is signed.

/// The person's own copy says whether this server holds a profile for them (`stored`): false in
/// the answer to their own request when there is none yet (its words are defaults, so their app
/// sends its own "Accept files from" there), true once there is one, in that answer and on the
/// echo of a save; never on a copy for anyone else (BUG-200, the review of 2026-10-12).
///
/// Seen red 2026-10-12 with the own request's answer always `stored: Some(true)`: "no profile
/// here yet: stored false" failed.
#[test]
fn the_own_copy_says_whether_a_profile_is_stored_here() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    sign_in(&st, "bob_key", "Bob");
    let stored_of = |sent: &[RelayMessage]| -> Vec<(Option<String>, Option<bool>)> {
        sent.iter()
            .filter(|m| matches!(m, RelayMessage::ProfileData { .. }))
            .map(|m| {
                let w = serde_json::to_value(m).unwrap();
                (w["target"].as_str().map(str::to_string), w["stored"].as_bool())
            })
            .collect()
    };
    let ask = serde_json::json!({ "type": "profile_request", "name": "Ann" });
    assert_eq!(stored_of(&send(&st, "ann_key", ask.clone())), vec![(some("ann_key"), Some(false))], "no profile here yet: stored false");
    assert_eq!(stored_of(&send(&st, "bob_key", ask.clone())), vec![(some("bob_key"), None)], "never on an answer to someone else");

    let echo = send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}" }));
    assert_eq!(stored_of(&echo), vec![(None, None), (some("ann_key"), Some(true))], "the echo of a save: stored, on the own copy only");
    assert_eq!(stored_of(&send(&st, "ann_key", ask.clone())), vec![(some("ann_key"), Some(true))], "a profile here now: stored true");
    assert_eq!(stored_of(&send(&st, "bob_key", ask)), vec![(some("bob_key"), None)]);
}

// ── The copy for other servers: asked, checked, kept, passed on (BUG-191 Wave 1A) ──

/// (who it is for, `sign_now`, `signed_at`) of each ProfileData in `sent`, sorted.
fn asks_in(sent: &[RelayMessage]) -> Vec<(Option<String>, Option<bool>, Option<u64>)> {
    let mut asks: Vec<_> = sent
        .iter()
        .filter(|m| matches!(m, RelayMessage::ProfileData { .. }))
        .map(|m| {
            let w = serde_json::to_value(m).unwrap();
            (w["target"].as_str().map(str::to_string), w["sign_now"].as_bool(), w["signed_at"].as_u64())
        })
        .collect();
    asks.sort();
    asks
}

/// A SAVE ASKS EACH OF THE PERSON'S DEVICES TO SIGN: Ann's laptop and her linked phone (a key of
/// its own under her name) both get `sign_now` and `signed_at: 0` (nothing held yet); the public
/// copy everyone gets carries neither. Once the laptop's copy is kept, a save that changes nothing
/// anyone may see (here only who she accepts files from) does not ask the laptop again, but still
/// asks the phone, whose key has no copy yet.
///
/// Seen red 2026-10-12 with the echo's `sign_now` left `None`: "each device is asked: [(None,
/// None, None), (Some(\"9bb07f42\"), None, Some(0)), (Some(\"cc2f9782\"), None, Some(0))]" (the
/// message cuts keys to 8 characters).
#[test]
fn a_save_asks_each_of_the_persons_devices_to_sign() {
    let st = fresh_state();
    let (ann, phone) = (key_of(7), key_of(8));
    sign_in(&st, &ann, "Ann");
    link_device(&st, "Ann", &ann, &phone);

    let sent = send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon" }));
    let mut expected = vec![(None, None, None), (Some(ann.clone()), Some(true), Some(0)), (Some(phone.clone()), Some(true), Some(0))];
    expected.sort();
    let got = asks_in(&sent);
    assert!(got == expected, "each device is asked: {:?}", got.iter().map(|(t, s, a)| (t.as_deref().map(|k| &k[..8]), *s, *a)).collect::<Vec<_>>());

    let (at, sig) = app_signs(&own_copy_for(&sent, &ann), 7, 0);
    send_sign(&st, &ann, at, &sig);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_some(), "test setup: the laptop's copy is kept");

    let sent = send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "accepts_files": "anyone" }));
    let mut expected = vec![(None, None, None), (Some(ann.clone()), Some(false), Some(at)), (Some(phone.clone()), Some(true), Some(0))];
    expected.sort();
    assert!(asks_in(&sent) == expected, "nothing anyone may see changed: the laptop is not asked again");
}

/// A NAME NOBODY REGISTERED IS NEVER ASKED: a socket showing a placeholder name ("Player", which
/// is not registered, and whose profile row anyone using that name shares) can save, but its own
/// copy never asks it to sign a copy for other servers.
///
/// Seen red 2026-10-12 with the `registered.contains(&key)` test taken out of the echo:
/// "a placeholder name is not asked" (left Some(true), right Some(false)).
#[test]
fn a_name_nobody_registered_is_never_asked() {
    let st = fresh_state();
    let someone = key_of(9);
    online_as(&st, &someone, "Player");
    let sent = send(&st, &someone, json!({ "type": "profile_update", "bio": "hi", "socials": "{}" }));
    let own = own_copy_for(&sent, &someone);
    assert_eq!(own["sign_now"].as_bool(), Some(false), "a placeholder name is not asked");
}

/// A SIGNED COPY IS KEPT AND PASSED ON, WITHOUT WHAT FRIENDS SEE: Ann keeps her location for
/// friends; her app signs the public copy of her own copy; this server keeps it and the linked
/// server gets exactly one `profile_gossip`, with her location blank and her pronouns shown, whose
/// signature checks as hers. The ask is used up: the same frame again passes nothing on.
///
/// Seen red 2026-10-12 with the relay's copy built without the map (`"{}"` for the privacy map in
/// `handle_profile_sign`): "the other server got one copy" (left 0, right 1). The plan named the text
/// "Lisbon reached the other server while Ann kept it for friends", but the copy the relay then
/// builds no longer matches the one Ann's app signed, so nothing is passed on at all: the
/// signature check stops it first.
#[test]
fn a_signed_copy_is_kept_and_passed_on_without_what_friends_see() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let mut other_server = link_a_server(&st, "peer_b");
    let sent = send(&st, &ann, json!({
        "type": "profile_update", "bio": "hi", "socials": "{}", "pronouns": "she", "location": "Lisbon",
        "privacy": "{\"location\":\"private\"}",
    }));
    let own = own_copy_for(&sent, &ann);
    assert_eq!(own["location"], "Lisbon", "her own copy carries it");
    let (at, sig) = app_signs(&own, 7, 0);
    let told = send_sign(&st, &ann, at, &sig);
    assert!(notices(&told, &ann).is_empty(), "{:?}", notices(&told, &ann));

    let got = gossip_got(&mut other_server);
    assert_eq!(got.len(), 1, "the other server got one copy");
    assert_eq!(got[0]["location"], "", "Lisbon reached the other server while Ann kept it for friends");
    assert_eq!((got[0]["name"].as_str(), got[0]["bio"].as_str(), got[0]["pronouns"].as_str()), (Some("Ann"), Some("hi"), Some("she")));
    assert_eq!((got[0]["public_key"].as_str(), got[0]["timestamp"].as_u64()), (Some(ann.as_str()), Some(at)));
    assert!(verify_copy(&ann, &copy_in(&got[0]), at, got[0]["signature"].as_str().unwrap()), "the copy passed on is the one her key signed");
    let held = st.db.get_signed_profile(&ann).unwrap().expect("this server keeps it");
    assert_eq!((held.location.as_str(), held.timestamp), ("", at));

    send_sign(&st, &ann, at, &sig);
    assert!(gossip_got(&mut other_server).is_empty(), "the ask was used up: nothing more passed on");
}

/// A COPY THAT DOES NOT MATCH IS NOT KEPT OR PASSED ON, and nobody is told: neither another key's
/// signature over Ann's copy, nor Ann's over a copy with a bio she did not save. The ask stays (the
/// mismatch may be a race with a save from another device), so her real copy is still kept after.
///
/// Seen red 2026-10-12 with the verify result ignored (`good` always true): "a copy another key
/// signed was kept".
#[test]
fn a_copy_that_does_not_match_is_not_kept_or_passed_on() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let mut other_server = link_a_server(&st, "peer_b");
    let own = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}" })), &ann);

    let (at, _) = app_signs(&own, 7, 0);
    let (_, by_someone_else) = app_signs(&own, 8, 0);
    let told = send_sign(&st, &ann, at, &by_someone_else);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "a copy another key signed was kept");
    assert!(notices(&told, &ann).is_empty(), "and nobody is told: {:?}", notices(&told, &ann));

    let mut other_bio = own.clone();
    other_bio["bio"] = json!("hi!");
    let (at, sig) = app_signs(&other_bio, 7, 0);
    let told = send_sign(&st, &ann, at, &sig);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "a copy of a bio she did not save was kept");
    assert!(notices(&told, &ann).is_empty());
    assert!(gossip_got(&mut other_server).is_empty(), "nothing is passed on");

    let (at, sig) = app_signs(&own, 7, at);
    send_sign(&st, &ann, at, &sig);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_some(), "the ask stayed: her real copy is kept");
    assert_eq!(gossip_got(&mut other_server).len(), 1);
}

/// A COPY NOBODY ASKED FOR IS DROPPED: Ann's profile is stored here (no save frame, so no ask),
/// and her app's correct signature over its public copy is not kept or passed on. Nor is one sent
/// after its ask is more than a minute old.
///
/// Seen red 2026-10-12 with the ask check taken out of `handle_profile_sign`: "a copy nobody asked
/// for was kept".
#[test]
fn a_copy_nobody_asked_for_is_dropped() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let mut other_server = link_a_server(&st, "peer_b");
    st.db.save_profile_extended("Ann", "hi", "{}", "", "", "", "", "", "{}", None).unwrap();
    let copy = public_copy("Ann", "hi", "", "", "{}", "", "", "", "{}");
    let at = now_millis();
    let sig = sign_copy(&[7; 32], &ann, &copy, at);
    send_sign(&st, &ann, at, &sig);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "a copy nobody asked for was kept");
    assert!(gossip_got(&mut other_server).is_empty());

    if let Some(long_ago) = Instant::now().checked_sub(ASK_LIFETIME + Duration::from_secs(1)) {
        st.profile_sign_asks.lock().unwrap().insert(ann.clone(), long_ago);
        send_sign(&st, &ann, at, &sig);
        assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "a copy asked for more than a minute ago was kept");
    }
}

/// A COPY FOR AN OLDER SAVE DOES NOT USE UP THE ASK: Ann saves twice; her app's copy of the first
/// save arrives after the second (it no longer matches what is stored, so it is not kept), and
/// the copy of the second is still taken.
///
/// Seen red 2026-10-12 with the ask taken before the check (`remove` in place of `get` at step 1
/// of `handle_profile_sign`): "the copy of the newer
/// save is kept" (left None, right Some("second")).
#[test]
fn a_copy_for_an_older_save_does_not_use_up_the_ask() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let first = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "first", "socials": "{}" })), &ann);
    let second = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "second", "socials": "{}" })), &ann);

    let (at1, sig1) = app_signs(&first, 7, 0);
    send_sign(&st, &ann, at1, &sig1);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "the copy of the older save is not kept");

    let (at2, sig2) = app_signs(&second, 7, at1);
    send_sign(&st, &ann, at2, &sig2);
    let held = st.db.get_signed_profile(&ann).unwrap();
    assert_eq!(held.map(|h| h.bio).as_deref(), Some("second"), "the copy of the newer save is kept");
}

/// A COPY DATED MORE THAN A DAY AHEAD IS REFUSED WITH WORDS: the one refusal the person can fix
/// (their clock), so they are told, in words that name the clock; nothing is kept or passed on.
/// The ask stays, so a copy dated right is still taken.
///
/// Seen red 2026-10-12 with the clock check taken out of `handle_profile_sign` (left to the
/// store, which refuses it silently): "the notice names the clock: []".
#[test]
fn a_copy_dated_more_than_a_day_ahead_is_refused_with_words() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let mut other_server = link_a_server(&st, "peer_b");
    let own = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}" })), &ann);

    let ahead = now_millis() + MAX_AHEAD_MS + 60_000;
    let copy = public_copy("Ann", "hi", "", "", "{}", "", "", "", "{}");
    let told = notices(&send_sign(&st, &ann, ahead, &sign_copy(&[7; 32], &ann, &copy, ahead)), &ann);
    assert!(told.len() == 1 && told[0].contains("clock is more than a day ahead"), "the notice names the clock: {told:?}");
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "nothing is kept");
    assert!(gossip_got(&mut other_server).is_empty(), "nothing is passed on");

    let (at, sig) = app_signs(&own, 7, 0);
    send_sign(&st, &ann, at, &sig);
    assert!(st.db.get_signed_profile(&ann).unwrap().is_some(), "a copy dated right is still taken");
}

/// GOING UNLISTED SIGNS A WITHDRAWAL THAT REPLACES THE COPY: once Ann goes unlisted, this
/// server's lookup answers 404 at once; her own copy asks her to sign, her app's public copy is
/// the withdrawal, and the linked server gets it (empty name, every field blank), replacing what
/// it held. This server holds the withdrawal (so it can replace the copy everywhere) and still
/// answers 404.
///
/// Seen red 2026-10-12 with the old delete-only path put back after the echo (this server's row
/// deleted when unlisted): "the linked server gets the withdrawal" (left 0, right 1): the store
/// keeps no withdrawal where nothing is held, so nothing is passed on and the linked server keeps
/// her listed copy.
#[test]
fn going_unlisted_signs_a_withdrawal_that_replaces_the_copy() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let mut other_server = link_a_server(&st, "peer_b");
    let own = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon" })), &ann);
    let (at1, sig1) = app_signs(&own, 7, 0);
    send_sign(&st, &ann, at1, &sig1);
    assert_eq!(gossip_got(&mut other_server).len(), 1, "test setup: her listed copy went out");
    assert_eq!(lookup(&st, &ann).0, axum::http::StatusCode::OK);

    let sent = send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "privacy": "{\"directory\":\"unlisted\"}" }));
    assert_eq!(lookup(&st, &ann).0, axum::http::StatusCode::NOT_FOUND, "the lookup answers 404 at once");
    let own = own_copy_for(&sent, &ann);
    assert_eq!((own["sign_now"].as_bool(), own["signed_at"].as_u64()), (Some(true), Some(at1)), "she is asked to sign the withdrawal");

    let (at2, sig2) = app_signs(&own, 7, at1);
    send_sign(&st, &ann, at2, &sig2);
    let got = gossip_got(&mut other_server);
    assert_eq!(got.len(), 1, "the linked server gets the withdrawal");
    assert!(is_withdrawal(&copy_in(&got[0])) && copy_in(&got[0]) == withdrawal(), "it carries nothing but the mark: {}", got[0]);
    assert!(verify_copy(&ann, &copy_in(&got[0]), at2, got[0]["signature"].as_str().unwrap()), "signed by her key");
    let held = st.db.get_signed_profile(&ann).unwrap().expect("the withdrawal is held");
    assert_eq!((held.copy(), held.timestamp), (withdrawal(), at2));
    assert_eq!(lookup(&st, &ann).0, axum::http::StatusCode::NOT_FOUND);
}

/// A CONNECT (and the person's own request, which asks the same way) ASKS ONLY WHEN THE COPY OUT
/// THERE SHOWS MORE THAN ALLOWED HERE: not when nothing changed, not when the profile here differs
/// in a way anyone may see (a newer save made on another server must never be overwritten by a
/// device that merely connects), but yes once a field in that copy is kept for friends here, and
/// yes once the person is unlisted here. After the narrowed copy is kept, no more.
///
/// Seen red 2026-10-12 with the own request passing `just_saved: true` to `ask_for_copy`:
/// "a benign difference asked to sign on connect" (left (Some(true), Some(1791836990831)), right
/// (Some(false), Some(1791836990831))).
#[test]
fn a_connect_asks_only_when_the_copy_out_there_shows_more_than_allowed() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    let own = own_copy_for(&send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon" })), &ann);
    let (at, sig) = app_signs(&own, 7, 0);
    send_sign(&st, &ann, at, &sig);

    let ask = json!({ "type": "profile_request", "name": "Ann" });
    let asked = |st: &Arc<RelayState>| {
        let own = own_copy_for(&send(st, &ann, ask.clone()), &ann);
        (own["sign_now"].as_bool(), own["signed_at"].as_u64())
    };
    assert_eq!(asked(&st), (Some(false), Some(at)), "nothing changed: not asked");

    st.db.save_profile_extended("Ann", "changed elsewhere", "{}", "", "", "", "Lisbon", "", "{}", None).unwrap();
    assert_eq!(asked(&st), (Some(false), Some(at)), "a benign difference asked to sign on connect");

    st.db.save_profile_extended("Ann", "changed elsewhere", "{}", "", "", "", "Lisbon", "", "{\"location\":\"private\"}", None).unwrap();
    let narrowed = own_copy_for(&send(&st, &ann, ask.clone()), &ann);
    assert_eq!(narrowed["sign_now"].as_bool(), Some(true), "her location out there is now kept for friends here: asked");
    let (at2, sig2) = app_signs(&narrowed, 7, at);
    send_sign(&st, &ann, at2, &sig2);
    assert_eq!(st.db.get_signed_profile(&ann).unwrap().map(|h| h.location), Some(String::new()), "the narrowed copy is kept");
    assert_eq!(asked(&st), (Some(false), Some(at2)), "and then no more");

    st.db.save_profile_extended("Ann", "changed elsewhere", "{}", "", "", "", "Lisbon", "", "{\"directory\":\"unlisted\"}", None).unwrap();
    assert_eq!(asked(&st).0, Some(true), "unlisted here: asked for the withdrawal");
}

/// THE OWN COPY NAMES THE SOCKET: Ann asks for "ann"; her own copy is named "Ann", as her socket
/// is, because her app signs that name into the copy for other servers and the relay rebuilds the
/// copy under the socket's name.
///
/// Seen red 2026-10-12 with the asked name used for the own copy: "her own copy names her as her socket
/// does" (left String("ann"), right "Ann").
#[test]
fn the_own_copy_names_the_socket() {
    let st = fresh_state();
    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    send(&st, &ann, json!({ "type": "profile_update", "bio": "hi", "socials": "{}" }));
    let own = own_copy_for(&send(&st, &ann, json!({ "type": "profile_request", "name": "ann" })), &ann);
    assert_eq!(own["name"], "Ann", "her own copy names her as her socket does");
}

/// A PRIVACY MAP MUST BE WORDS: an object whose every value is a word. Anything else (a number, a
/// list, a map that is not an object) is refused with "Invalid privacy map." and nothing is saved;
/// the words both apps write are saved.
///
/// Seen red 2026-10-12 with the old check (any JSON at all): "a map that is not words is refused:
/// {"location":1}" (left [], right ["Invalid privacy map."]).
#[test]
fn a_privacy_map_must_be_words() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    for bad in [r#"{"location":1}"#, r#"{"directory":["unlisted"]}"#, "[]", r#""private""#] {
        let sent = send(&st, "ann_key", json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "privacy": bad }));
        assert_eq!(notices(&sent, "ann_key"), vec!["Invalid privacy map.".to_string()], "a map that is not words is refused: {bad}");
        assert!(profile_datas(&sent).is_empty() && st.db.get_profile("Ann").unwrap().is_none(), "nothing is saved: {bad}");
    }
    let words = r#"{"location":"private","pronouns":"public","directory":"unlisted"}"#;
    let sent = send(&st, "ann_key", json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "privacy": words }));
    assert!(notices(&sent, "ann_key").is_empty(), "{:?}", notices(&sent, "ann_key"));
    assert_eq!(st.db.get_profile_extended("Ann").unwrap().map(|r| r.7).as_deref(), Some(words));
}

/// A LINE BREAK IN A ONE-LINE FIELD IS REFUSED: pronouns, location, the three links and the
/// social links, with words that say so, and nothing is saved (the signed words split by line, and
/// only the bio may hold one). The bio keeps its line breaks; a line break at the end of a field
/// is trimmed away like any other space.
///
/// Seen red 2026-10-12 with the line-break check taken out of `handle_profile_update`:
/// "a line break in location is refused" (left [], right ["Pronouns, location and links cannot
/// hold a line break."]).
#[test]
fn a_line_break_in_a_one_line_field_is_refused() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    let cases = [
        ("location", json!("Lis\nbon")),
        ("pronouns", json!("she\nher")),
        ("avatar_url", json!("https://ann.example/\na.png")),
        ("banner_url", json!("https://ann.example/\nb.png")),
        ("website", json!("https://ann.example/\nx")),
        ("socials", json!("{\"github\":\"ann\",\n\"discord\":\"ann#1\"}")),
    ];
    for (field, value) in cases {
        let mut frame = json!({ "type": "profile_update", "bio": "hi", "socials": "{}" });
        frame[field] = value;
        let sent = send(&st, "ann_key", frame);
        assert_eq!(
            notices(&sent, "ann_key"),
            vec!["Pronouns, location and links cannot hold a line break.".to_string()],
            "a line break in {field} is refused"
        );
        assert!(st.db.get_profile("Ann").unwrap().is_none(), "nothing is saved: {field}");
    }
    let sent = send(&st, "ann_key", json!({ "type": "profile_update", "bio": "line one\nline two", "socials": "{}", "location": "Lisbon\n" }));
    assert!(notices(&sent, "ann_key").is_empty(), "{:?}", notices(&sent, "ann_key"));
    let row = st.db.get_profile_extended("Ann").unwrap().expect("saved");
    assert_eq!((row.0.as_str(), row.5.as_str()), ("line one\nline two", "Lisbon"));
}

/// THE LINKS MUST BE AN OBJECT: any other JSON (a list, null, a word, a number) is refused with
/// "Invalid socials data." and nothing is saved, because the copy for other servers is held to an
/// object and a profile it could never be signed for would leave the copy already out there as it
/// was. An object is saved.
///
/// Seen red 2026-10-12 with the old check (any JSON at all): "links that are not an object are
/// refused: []" (left [], right ["Invalid socials data."]).
#[test]
fn links_that_are_not_an_object_are_refused() {
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    for bad in ["[]", "null", "\"ann\"", "1"] {
        let sent = send(&st, "ann_key", json!({ "type": "profile_update", "bio": "hi", "socials": bad }));
        assert_eq!(notices(&sent, "ann_key"), vec!["Invalid socials data.".to_string()], "links that are not an object are refused: {bad}");
        assert!(st.db.get_profile("Ann").unwrap().is_none(), "nothing is saved: {bad}");
    }
    let sent = send(&st, "ann_key", json!({ "type": "profile_update", "bio": "hi", "socials": "{\"github\":\"ann\"}" }));
    assert!(notices(&sent, "ann_key").is_empty(), "{:?}", notices(&sent, "ann_key"));
    assert_eq!(st.db.get_profile("Ann").unwrap().map(|p| p.1).as_deref(), Some("{\"github\":\"ann\"}"));
}

/// THE LOOKUP ANSWERS NOT FOUND FOR A WITHDRAWAL AND FOR AN UNLISTED ACCOUNT (plan 3.9), with the
/// same answer as for a key never seen: a withdrawal held for someone with no account here (it
/// arrived from a linked server), and a listed copy of someone whose account here is unlisted now
/// (the seconds before their device signs the withdrawal). Listed again, the copy is answered.
///
/// Seen red 2026-10-12 with the withdrawal check taken out of `api::get_signed_profile`:
/// "a withdrawal is not found" (left 200, right 404); and with the unlisted check taken out: "an account
/// unlisted here is not found" (left 200, right 404).
#[test]
fn the_lookup_answers_not_found_for_a_withdrawal_and_an_unlisted_account() {
    use axum::http::StatusCode;
    let st = fresh_state();
    let now = now_millis();
    let never_seen = lookup(&st, &key_of(9));
    assert_eq!(never_seen.0, StatusCode::NOT_FOUND);

    let far = key_of(8);
    let listed = PublicCopy { name: "Far".into(), bio: "from a peer".into(), socials: "{}".into(), ..Default::default() };
    st.db.store_signed_profile(&far, &listed, now - 2, "sig").unwrap();
    assert_eq!(lookup(&st, &far).0, StatusCode::OK, "test setup: the listed copy is answered");
    st.db.store_signed_profile(&far, &withdrawal(), now - 1, "sig").unwrap();
    let answer = lookup(&st, &far);
    assert_eq!(answer.0, StatusCode::NOT_FOUND, "a withdrawal is not found");
    assert_eq!(answer.1, never_seen.1, "the same answer as for a key never seen");

    let ann = key_of(7);
    sign_in(&st, &ann, "Ann");
    st.db.save_profile_extended("Ann", "hi", "{}", "", "", "", "", "", "{\"directory\":\"unlisted\"}", None).unwrap();
    st.db.store_signed_profile(&ann, &public_copy("Ann", "hi", "", "", "{}", "", "", "", "{}"), now, "sig").unwrap();
    let answer = lookup(&st, &ann);
    assert_eq!(answer.0, StatusCode::NOT_FOUND, "an account unlisted here is not found");
    assert_eq!(answer.1, never_seen.1);
    st.db.save_profile_extended("Ann", "hi", "{}", "", "", "", "", "", "{}", None).unwrap();
    assert_eq!(lookup(&st, &ann).0, StatusCode::OK, "listed again: answered");
}

// ── An erase takes the copy off the linked servers (BUG-191 Wave 2B, plan section 9 decision 2) ──

/// `key` erases their account, typing `name`, with `withdrawal`; what the relay sent meanwhile.
fn erase(st: &Arc<RelayState>, key: &str, name: &str, withdrawal: Option<Withdrawal>) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    block(crate::relay::handlers::msg_handlers::handle_account_delete(st, key, name.to_string(), withdrawal));
    let mut out = Vec::new();
    while let Ok(m) = rx.try_recv() {
        out.push(m);
    }
    out
}

/// What the person's app sends with an erase: the withdrawal, dated after the copy the server holds
/// (`signed_at`, from their own copy), signed with the seed `[seed; 32]` for the key of `[for_key; 32]`.
fn withdrawal_by(seed: u8, for_key: u8, signed_at: u64) -> Withdrawal {
    let at = next_copy_time(now_millis(), signed_at, 0);
    Withdrawal { at, signature: sign_copy(&[seed; 32], &key_of(for_key), &withdrawal(), at) }
}

/// `name` (the key of `[seed; 32]`) saved a listed profile here, and their app signed its copy,
/// which this server kept and passed on to `other_server`. Returns their key and the copy's time.
fn listed_and_passed_on(st: &Arc<RelayState>, other_server: &mut tokio::sync::mpsc::UnboundedReceiver<String>, seed: u8, name: &str) -> (String, u64) {
    let key = key_of(seed);
    sign_in(st, &key, name);
    let own = own_copy_for(&send(st, &key, json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon" })), &key);
    let (at, sig) = app_signs(&own, seed, 0);
    send_sign(st, &key, at, &sig);
    assert_eq!(gossip_got(other_server).len(), 1, "test setup: {name}'s listed copy went out");
    (key, at)
}

/// AN ERASE PASSES ON ONLY THE PERSON'S OWN WITHDRAWAL, BEFORE THEIR ACCOUNT GOES. Ann erases her
/// account with the withdrawal her app signed: the linked server gets it (nothing in it but the
/// mark, her signature over it), and her account and the copy held here are gone. An erase the
/// server refuses (her name mistyped) passes nothing on and keeps her copy. Bo's erase carries a
/// withdrawal another key signed, Cy's her own signature over her LISTED copy rather than the
/// withdrawal, and Di's none (an older app): each is erased, and nothing is passed on. An admin's
/// erase of Eve cannot carry one (only her key can sign it): erased, nothing passed on, and the
/// linked server keeps her copy.
///
/// Seen red 2026-10-12 with the `withdraw_on_erase` call taken out of `handle_account_delete`:
/// "Ann's withdrawal reaches the linked server" (left 0, right 1); with it called AFTER `carry_out`
/// instead of before: the same (the copy held here is gone with the account, so the store keeps
/// no withdrawal and nothing goes); and with the signature check skipped in `withdraw_on_erase`
/// (`good` always true): "a withdrawal another key signed was passed on".
#[test]
fn an_erase_passes_on_only_the_persons_own_withdrawal() {
    let st = fresh_state();
    let mut other_server = link_a_server(&st, "peer_b");
    let (ann, ann_at) = listed_and_passed_on(&st, &mut other_server, 7, "Ann");

    erase(&st, &ann, "ann", Some(withdrawal_by(7, 7, ann_at)));
    assert!(gossip_got(&mut other_server).is_empty(), "a refused erase passes nothing on");
    assert!(st.db.get_signed_profile(&ann).unwrap().is_some_and(|h| !is_withdrawal(&h.copy())), "and keeps her copy");

    let mine = withdrawal_by(7, 7, ann_at);
    erase(&st, &ann, "Ann", Some(mine.clone()));
    let got = gossip_got(&mut other_server);
    assert_eq!(got.len(), 1, "Ann's withdrawal reaches the linked server");
    assert_eq!(copy_in(&got[0]), withdrawal(), "nothing in it but the mark: {}", got[0]);
    assert_eq!((got[0]["public_key"].as_str(), got[0]["timestamp"].as_u64()), (Some(ann.as_str()), Some(mine.at)));
    assert!(verify_copy(&ann, &withdrawal(), mine.at, got[0]["signature"].as_str().unwrap()), "signed by her key");
    assert_eq!(st.db.name_for_key(&ann).unwrap(), None, "her account is erased");
    assert!(st.db.get_signed_profile(&ann).unwrap().is_none(), "and the copy held here with it");

    let (bo, bo_at) = listed_and_passed_on(&st, &mut other_server, 8, "Bo");
    erase(&st, &bo, "Bo", Some(withdrawal_by(9, 8, bo_at)));
    assert!(gossip_got(&mut other_server).is_empty(), "a withdrawal another key signed was passed on");
    assert_eq!(st.db.name_for_key(&bo).unwrap(), None, "Bo is erased all the same");

    let (cy, cy_at) = listed_and_passed_on(&st, &mut other_server, 10, "Cy");
    let listed = public_copy("Cy", "hi", "", "", "{}", "", "Lisbon", "", "{}");
    let at = next_copy_time(now_millis(), cy_at, 0);
    erase(&st, &cy, "Cy", Some(Withdrawal { at, signature: sign_copy(&[10; 32], &cy, &listed, at) }));
    assert!(gossip_got(&mut other_server).is_empty(), "her signature over her listed copy is not a withdrawal");
    assert_eq!(st.db.name_for_key(&cy).unwrap(), None);

    let (di, _) = listed_and_passed_on(&st, &mut other_server, 11, "Di");
    erase(&st, &di, "Di", None);
    assert!(gossip_got(&mut other_server).is_empty(), "no withdrawal sent, nothing passed on");
    assert_eq!(st.db.name_for_key(&di).unwrap(), None, "an older app's erase still erases");

    let (eve, _) = listed_and_passed_on(&st, &mut other_server, 12, "Eve");
    sign_in(&st, "admin_key", "Admin");
    st.db.set_role("admin_key", "admin").unwrap();
    block(crate::relay::handlers::account_erase::handle_admin_erase(&st, "admin_key", eve.clone(), "Eve".to_string()));
    assert_eq!(st.db.name_for_key(&eve).unwrap(), None, "the admin erased Eve");
    assert!(gossip_got(&mut other_server).is_empty(), "an admin's erase has no withdrawal to pass on");
}
