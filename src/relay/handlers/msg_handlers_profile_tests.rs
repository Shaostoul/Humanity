// Child of msg_handlers.rs (#[path], so `use super::*` reaches everything it has): the profile
// field `accepts_files`, who a person accepts files in messages from (2026-10-11,
// docs/design/media-showcase.md "Files on the sender's device and in messages, exact", part 3).
// Kept out of msg_handlers.rs under the file-size ratchet (tests/file_size_ratchet.rs).

use super::*;
use crate::relay::relay::{Peer, RelayState};

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
    block(async {
        st.peers.write().await.insert(
            key.to_string(),
            Peer { public_key_hex: key.to_string(), display_name: Some(name.to_string()), upload_token: None, kyber_public: None, conn_id: 1 },
        );
    });
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
    st.db.store_signed_profile("far_key", "Far", "from a peer server", "", "", "{}", "", "", "", now, "sig").unwrap();

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
/// Seen red 2026-10-11 with the lookups as they were (the relay review of that day): "the member
/// lookup leaves out a private location" failed with "Lisbon".
#[test]
fn private_fields_stay_out_of_the_lookups() {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    let st = fresh_state();
    sign_in(&st, "ann_key", "Ann");
    send(&st, "ann_key", serde_json::json!({ "type": "profile_update", "bio": "hi", "socials": "{}", "location": "Lisbon", "pronouns": "she", "privacy": "{\"location\":\"private\"}" }));
    let body = |resp: axum::response::Response| -> serde_json::Value {
        let bytes = block(axum::body::to_bytes(resp.into_body(), usize::MAX)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let member = block(crate::relay::api::get_member_by_key(State(st.clone()), Path("ann_key".to_string()))).expect("a member");
    assert_eq!(member.0["location"], "", "the member lookup leaves out a private location: {}", member.0);
    let signed = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("ann_key".to_string()))).into_response());
    assert_eq!(signed["location"], "", "the profile lookup too: {signed}");
    assert_eq!(signed["pronouns"], "she", "a public field is shown");
    let kept = st.db.get_signed_profile("ann_key").unwrap().expect("the copy for other servers");
    assert_eq!(kept.location, "", "the copy for other servers leaves it out");

    // A copy cached before the rule, still holding the location.
    st.db.store_signed_profile("ann_key", "Ann", "hi", "", "", "{}", "she", "Lisbon", "", crate::relay::storage::now_millis(), "").unwrap();
    let signed = body(block(crate::relay::api::get_signed_profile(State(st.clone()), Path("ann_key".to_string()))).into_response());
    assert_eq!(signed["location"], "", "an older cached copy is answered by the map as it is now");
}

/// This server's own copy of someone no longer here (no account: kicked, banned, name released; no
/// signature, which every copy from another server carries) is answered at /api/profile/{key}
/// without the fields people keep for friends: their privacy map went with the account. The second
/// review of 2026-10-11.
///
/// Seen red 2026-10-11 without that branch in `api::get_signed_profile`: "a copy for someone no
/// longer here leaves out the location" failed with "Lisbon".
#[test]
fn a_copy_for_someone_no_longer_here_leaves_out_the_private_fields() {
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    let st = fresh_state();
    let now = crate::relay::storage::now_millis();
    st.db.store_signed_profile("gone_key", "Gone", "bio", "", "", "{}", "she", "Lisbon", "https://g.example", now, "").unwrap();
    st.db.store_signed_profile("far_key", "Far", "bio", "", "", "{}", "they", "Porto", "", now, "sig").unwrap();
    let body = |key: &str| -> serde_json::Value {
        let resp = block(crate::relay::api::get_signed_profile(State(st.clone()), Path(key.to_string()))).into_response();
        let bytes = block(axum::body::to_bytes(resp.into_body(), usize::MAX)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    let gone = body("gone_key");
    assert_eq!(gone["location"], "", "a copy for someone no longer here leaves out the location: {gone}");
    assert_eq!((gone["pronouns"].as_str(), gone["website"].as_str()), (Some(""), Some("")));
    assert_eq!(body("far_key")["location"], "Porto", "a signed copy from another server is served as it came");
}
