// Child of own_profile.rs (#[path]): which profile is this person's own, one copy per server, and
// the whole profile every save sends, held until that server's copy is here and its limit allows
// (BUG-193, BUG-194, BUG-196, 2026-10-11).

use super::*;

const ONE: &str = "https://one.example";
const TWO: &str = "https://two.example";
const ANN_PRIVACY: &str = r#"{"directory":"unlisted","location":"friends"}"#;

/// Ann's own copy as a server sends it on connect, as the echo of her save, and as the answer to
/// her own profile request: every field she has, and her privacy map.
fn anns_own_copy() -> serde_json::Value {
    serde_json::json!({
        "type": "profile_data", "name": "Ann", "bio": "Beekeeper", "socials": r#"{"site":"x"}"#,
        "avatar_url": "https://a.example/ann.png", "banner_url": "https://a.example/banner.png",
        "pronouns": "she/her", "location": "Lisbon", "website": "https://ann.example",
        "accepts_files": "anyone", "privacy": ANN_PRIVACY, "target": "ann_key",
    })
}

/// Ann signed in on `server`, her socket recording what it sends.
fn ann_on(server: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.server_url = server.to_string();
    let (client, sent) = crate::net::ws_client::WsClient::recording_as("Ann");
    gs.ws_client = Some(client);
    gs.ws_identified = true;
    (gs, sent)
}

fn frames(sent: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    sent.try_iter().map(|t| serde_json::from_str(&t).unwrap()).collect()
}

fn updates(sent: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    frames(sent).into_iter().filter(|f| f["type"] == "profile_update").collect()
}

/// Seen red, 2026-10-11: with `names_socket` answering true for every frame (the handler as it
/// was), "someone else's profile" failed; with `own_copy` not asking for the privacy map, "the
/// public copy of her own save" failed.
#[test]
fn only_her_own_complete_copy_is_taken_as_the_servers() {
    assert!(names_socket(&anns_own_copy(), "Ann"));
    assert!(names_socket(&anns_own_copy(), "ann"), "whatever the case");
    let bobs = serde_json::json!({ "type": "profile_data", "name": "Bob", "bio": "his", "privacy": "{}", "target": "ann_key" });
    assert!(!names_socket(&bobs, "Ann"), "someone else's profile, even addressed to her");
    assert!(own_copy(&bobs, "Ann").is_none(), "someone else's profile");
    assert!(!names_socket(&anns_own_copy(), ""), "a socket with no name owns nothing");

    let copy = own_copy(&anns_own_copy(), "Ann").expect("her own copy");
    assert_eq!((copy.pronouns.as_str(), copy.location.as_str(), copy.privacy.as_str()), ("she/her", "Lisbon", ANN_PRIVACY));

    // The public copy of her save names her too, without pronouns, location, website or privacy.
    let public = serde_json::json!({ "type": "profile_data", "name": "Ann", "bio": "Beekeeper", "socials": "{}", "accepts_files": "anyone" });
    assert!(names_socket(&public, "Ann"));
    assert!(own_copy(&public, "Ann").is_none(), "the public copy of her own save is not the server's copy");
}

/// Every save sends the whole profile: the fields this app does not edit come from the server's
/// copy, and the website's per-field privacy stays, only the directory choice changing.
///
/// Seen red, 2026-10-11: with `whole_update` sending what the Profile page's Save used to (bio,
/// "{}" links, a directory-only privacy map), "pronouns kept" failed with `Null`.
#[test]
fn a_save_sends_the_whole_profile_and_keeps_the_websites_privacy_choices() {
    let (mut gs, sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    pump(&mut gs);
    gs.profile_network_bio = "Beekeeper and baker".into();
    gs.profile_directory_listed = true;
    want_save(&mut gs);
    pump(&mut gs);
    let sent_now = updates(&sent);
    assert_eq!(sent_now.len(), 1, "one update");
    let f = &sent_now[0];
    assert_eq!(f["bio"], "Beekeeper and baker", "her edit");
    assert_eq!(f["avatar_url"], "https://a.example/ann.png");
    assert_eq!(f["pronouns"], "she/her", "pronouns kept");
    assert_eq!(f["location"], "Lisbon");
    assert_eq!(f["website"], "https://ann.example");
    assert_eq!(f["banner_url"], "https://a.example/banner.png");
    assert_eq!(f["socials"], r#"{"site":"x"}"#);
    let privacy: serde_json::Value = serde_json::from_str(f["privacy"].as_str().unwrap()).unwrap();
    assert_eq!(privacy, serde_json::json!({ "location": "friends" }), "listed now, the website's choice for location kept");
    assert!(f.get("accepts_files").is_none(), "the stored word is left to the server");
    assert_eq!(gs.profile_network_saved_note, "Saving to the server...");
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    assert_eq!(gs.profile_network_saved_note, "Saved to server.", "said once the server's copy comes back");
}

/// A copy coming in fills the Profile page's fields she has not touched, and leaves an unsaved edit.
#[test]
fn a_copy_coming_in_fills_untouched_fields_and_keeps_an_unsaved_edit() {
    let (mut gs, _sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    pump(&mut gs);
    assert_eq!(gs.profile_network_bio, "Beekeeper", "filled on connect");
    assert_eq!(gs.profile_network_avatar, "https://a.example/ann.png");
    assert!(!gs.profile_directory_listed, "her server says unlisted");

    gs.profile_network_bio = "Half-typed".into();
    let mut newer = anns_own_copy();
    newer["bio"] = "Changed on her phone".into();
    newer["avatar_url"] = "https://a.example/new.png".into();
    keep_copy(&mut gs, ONE, own_copy(&newer, "Ann").unwrap());
    pump(&mut gs);
    assert_eq!(gs.profile_network_bio, "Half-typed", "an unsaved edit stays");
    assert_eq!(gs.profile_network_avatar, "https://a.example/new.png", "an untouched field follows the server");
    assert_eq!(current_copy(&gs).unwrap().bio, "Changed on her phone", "the server's copy is the newest");
}

/// BUG-196: each server has its own copy, kept from the socket kept open in the background too; a
/// switch shows that server's profile and a save there carries that server's fields. On a server
/// that has sent none, the app asks for it, and a save waits for it instead of going out empty.
///
/// Seen red, 2026-10-11, with `keep_copy` filing only the active socket's copy (as the first BUG-194
/// fix did: one copy, for the server it came from): "the second server's profile is shown" failed,
/// still "Beekeeper".
#[test]
fn each_server_has_its_own_copy_and_a_save_waits_for_it() {
    let (mut gs, sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    let mut on_two = anns_own_copy();
    on_two["bio"] = "Ann on two".into();
    on_two["pronouns"] = "they/them".into();
    keep_copy(&mut gs, TWO, own_copy(&on_two, "Ann").unwrap()); // from the background socket
    pump(&mut gs);
    assert_eq!(gs.profile_network_bio, "Beekeeper");

    gs.server_url = TWO.into(); // the switch: a parked socket, no fresh connect
    pump(&mut gs);
    assert_eq!(gs.profile_network_bio, "Ann on two", "the second server's profile is shown");
    want_save(&mut gs);
    pump(&mut gs);
    let f = updates(&sent).pop().expect("a save");
    assert_eq!(f["pronouns"], "they/them", "a save on the second server carries its pronouns");
    assert_eq!(f["bio"], "Ann on two");

    // A third server has sent no copy: asked for, and the save waits.
    let three = "https://three.example";
    gs.server_url = three.into();
    gs.profile_network_bio = "Typed for three".into();
    want_save(&mut gs);
    pump(&mut gs);
    let sent_now = frames(&sent);
    assert!(sent_now.iter().all(|f| f["type"] != "profile_update"), "nothing is saved before that server's copy is here");
    assert!(sent_now.iter().any(|f| f["type"] == "profile_request" && f["name"] == "Ann"), "her own copy is asked for");
    pump(&mut gs);
    assert!(frames(&sent).is_empty(), "asked once, not every frame");
    keep_copy(&mut gs, three, own_copy(&serde_json::json!({ "type": "profile_data", "name": "Ann", "bio": "", "socials": "{}", "privacy": "{}" }), "Ann").unwrap());
    pump(&mut gs);
    let f = updates(&sent).pop().expect("then the save goes");
    assert_eq!(f["bio"], "Typed for three", "with what she typed");
}

/// Saves asked for close together go as one, at most one per 31 seconds per server (the relay
/// takes one update per 30 and turns the rest down).
///
/// Seen red, 2026-10-11, with the limit check taken out of `pump`: "not within 31 seconds" failed.
#[test]
fn saves_close_together_go_as_one_within_the_servers_limit() {
    let (mut gs, sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    want_save(&mut gs);
    pump(&mut gs);
    assert_eq!(updates(&sent).len(), 1);
    gs.profile_network_bio = "Second".into();
    want_save(&mut gs);
    pump(&mut gs);
    gs.profile_network_bio = "Third".into();
    want_save(&mut gs);
    pump(&mut gs);
    assert!(updates(&sent).is_empty(), "not within 31 seconds");
    gs.own_profile.last_sent.insert(current_server(&gs), Instant::now() - Duration::from_secs(40));
    pump(&mut gs);
    let sent_now = updates(&sent);
    assert_eq!(sent_now.len(), 1, "then one update");
    assert_eq!(sent_now[0]["bio"], "Third", "with the latest");
}

/// A new socket's sign-in drops that server's copy (another device may have saved while the socket
/// was down), so a waiting save waits for the fresh one; a new identity drops every copy (a save
/// under the new key must never carry the old one's fields). The second review of 2026-10-11.
///
/// Seen red 2026-10-11 with `socket_signed_in` keeping the copy: "a waiting save does not go on
/// the copy from before the drop" failed.
#[test]
fn a_new_socket_or_identity_drops_the_old_copy() {
    let (mut gs, sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    pump(&mut gs);
    gs.ws_identified = false; // the socket dropped and a new one signs in
    gs.socket_signed_in();
    want_save(&mut gs);
    pump(&mut gs);
    assert!(updates(&sent).is_empty(), "a waiting save does not go on the copy from before the drop");
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    pump(&mut gs);
    assert_eq!(updates(&sent).len(), 1, "it goes on the new socket's copy");

    let mut gs = GuiState::default();
    gs.private_key_bytes = Some(vec![7u8; 32]);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    gs.apply_pq_identity();
    assert!(gs.own_profile.copies.is_empty(), "a new identity starts with no copy");
}

/// The Profile page's note: a save the server never answers says so, instead of "Saved" when some
/// later copy comes. Seen red 2026-10-11 with the echo wait taken out of `pump`: "not taken"
/// failed (the note stayed "Saving to the server...").
#[test]
fn a_save_the_server_does_not_take_says_so() {
    let (mut gs, _sent) = ann_on(ONE);
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap());
    want_save(&mut gs);
    pump(&mut gs);
    assert_eq!(gs.profile_network_saved_note, "Saving to the server...");
    gs.own_profile.awaiting_echo = Some((current_server(&gs), Instant::now() - Duration::from_secs(20)));
    pump(&mut gs);
    assert_eq!(gs.profile_network_saved_note, NOT_TAKEN, "not taken");
    keep_copy(&mut gs, ONE, own_copy(&anns_own_copy(), "Ann").unwrap()); // a later copy
    assert_eq!(gs.profile_network_saved_note, NOT_TAKEN, "a later copy does not say Saved");
}

// ── The copy other servers keep (BUG-191, 2026-10-12, plan 3.10) ──

use crate::relay::core::profile_copy::{self, PublicCopy};

/// The shared fixture both apps read, written by the relay's own builder.
const COPY_FIXTURE: &str = "scripts/tests/fixtures/profile-copy-v1.json";
/// A clock for the tests that date copies, in Unix milliseconds.
const NOW_MS: u64 = 1_760_000_000_000;

/// The 32-byte seed `[7; 32]` every Rust fixture signs with, and its Dilithium3 key in hex.
fn seed_and_key() -> (Vec<u8>, String) {
    let seed = vec![7u8; 32];
    let key = hex::encode(crate::net::identity::pq_object_keypair(&seed).public_key());
    (seed, key)
}

/// Ann signed in on `server` with her identity unlocked, her socket recording what it sends.
fn ann_unlocked_on(server: &str) -> (GuiState, std::sync::mpsc::Receiver<String>, String) {
    let (mut gs, sent) = ann_on(server);
    let (seed, key) = seed_and_key();
    gs.private_key_bytes = Some(seed);
    gs.profile_public_key = key.clone();
    (gs, sent, key)
}

/// Ann's own copy as the echo of her save sends it to her key, asking her device to sign the copy
/// for other servers: she keeps her location for friends, and the server holds a copy dated
/// `signed_at` (0: none).
fn anns_copy_asking(key: &str, signed_at: u64) -> serde_json::Value {
    let mut f = anns_own_copy();
    f["privacy"] = r#"{"location":"private"}"#.into();
    f["target"] = key.into();
    f["signed_at"] = signed_at.into();
    f["sign_now"] = true.into();
    f
}

/// The public copy of a frame's fields, built straight from the frame (an absent field is blank),
/// not through the app's own filing: what the server rebuilds and checks the signature against.
fn public_copy_of(frame: &serde_json::Value) -> PublicCopy {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    profile_copy::public_copy(
        &s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"),
        &s("pronouns"), &s("location"), &s("website"), &s("privacy"),
    )
}

fn at_of(signed: Option<String>) -> u64 {
    let f: serde_json::Value = serde_json::from_str(&signed.expect("asked, so signed")).unwrap();
    f["at"].as_u64().expect("a time")
}

/// A copy that asks (`sign_now: true`) is signed once, as one `profile_sign` frame carrying only
/// the time and the signature, and the signature verifies over the public copy of exactly the
/// fields the frame carries (her location, kept for friends, left out). Without `sign_now` (absent
/// or false), on the public copy of her save (no privacy map), or on a copy addressed to another
/// of her keys, nothing is signed; the copy is filed all the same. A copy from a socket kept open
/// in the background is dated and recorded under THAT server, not the one the app is on.
///
/// Seen red 2026-10-12 with the `sign_now` check taken out of `take_own_copy_at` (every own copy
/// signed): "no sign_now, nothing signed" failed.
#[test]
fn a_copy_that_asks_is_signed_once_for_its_own_server() {
    let (mut gs, sent, key) = ann_unlocked_on(ONE);
    let asking = anns_copy_asking(&key, 0);
    let signed = take_own_copy(&mut gs, ONE, &asking, "Ann").expect("asked, so signed");
    gs.ws_client.as_ref().unwrap().send(&signed); // what the arrival site does with it
    pump(&mut gs);
    let out = frames(&sent);
    let signs: Vec<&serde_json::Value> = out.iter().filter(|f| f["type"] == "profile_sign").collect();
    assert_eq!(signs.len(), 1, "one profile_sign, and pump sends no other");
    let f = signs[0];
    assert_eq!(f.as_object().unwrap().len(), 3, "type, at and signature only: the server rebuilds the copy itself");
    let at = f["at"].as_u64().expect("a time");
    let sig = f["signature"].as_str().expect("a signature");
    assert!(profile_copy::is_signature_text(sig), "6,618 lowercase hex characters");
    let public = public_copy_of(&asking);
    assert_eq!((public.name.as_str(), public.location.as_str()), ("Ann", ""), "her location, kept for friends, is not in it");
    assert_eq!(public.pronouns, "she/her");
    assert!(profile_copy::verify_copy(&key, &public, at, sig), "it signs the public copy of what the server stored");
    let with_location = PublicCopy { location: "Lisbon".into(), ..public.clone() };
    assert!(!profile_copy::verify_copy(&key, &with_location, at, sig), "and not one with her location in it");
    assert!(current_copy(&gs).is_some_and(|c| c.location == "Lisbon"), "the copy is filed as ever, every field in it");

    let mut not_asked = asking.clone();
    not_asked.as_object_mut().unwrap().remove("sign_now");
    assert!(take_own_copy(&mut gs, ONE, &not_asked, "Ann").is_none(), "no sign_now, nothing signed");
    not_asked["sign_now"] = false.into();
    assert!(take_own_copy(&mut gs, ONE, &not_asked, "Ann").is_none(), "sign_now false, nothing signed");
    let mut public_frame = asking.clone();
    public_frame.as_object_mut().unwrap().remove("privacy");
    assert!(take_own_copy(&mut gs, ONE, &public_frame, "Ann").is_none(), "the public copy of her save is not hers to sign");
    let mut phones = asking.clone();
    phones["target"] = "ab".repeat(profile_copy::KEY_HEX_LEN / 2).into();
    assert!(take_own_copy(&mut gs, ONE, &phones, "Ann").is_none(), "her linked phone's copy: the phone signs its own");
    let mut bobs = asking.clone();
    bobs["name"] = "Bob".into();
    assert!(take_own_copy(&mut gs, ONE, &bobs, "Ann").is_none(), "someone else's profile");
    assert!(frames(&sent).is_empty(), "take_own_copy sends nothing itself");

    // The background socket of the second server: its own copy, its own time.
    let one = crate::gui::pages::chat::norm_server_url(ONE);
    let two = crate::gui::pages::chat::norm_server_url(TWO);
    let ahead = at + 90_000;
    let at_two = at_of(take_own_copy(&mut gs, TWO, &anns_copy_asking(&key, ahead), "Ann"));
    assert_eq!(at_two, ahead + 1, "dated after the copy the second server holds");
    assert_eq!(gs.own_profile.signed_here.get(&two), Some(&at_two), "recorded under the server it came from");
    assert_eq!(gs.own_profile.signed_here.get(&one), Some(&at), "the first server's time untouched");
    assert!(gs.own_profile.copies.contains_key(&two), "filed under the second server");
}

/// The desktop signs each of the shared fixture's own copies to the fixture's time and signature
/// byte for byte (ML-DSA signing in RustCrypto is deterministic), through the arrival path with
/// the fixture's clock and through the pure signer, and the signature verifies over the fixture's
/// copy. The socket here signed in as "ann" while the server's copy names her "Ann" (a server keeps
/// its own spelling of a name; a link code replaces it): the app signs the name the COPY carries,
/// because that is the name the server rebuilds the copy under.
///
/// Seen red 2026-10-12 with the name taken from the socket instead of the copy (`let name =
/// socket_name;` in `take_own_copy_at`): "case 0: the signature, byte for byte" failed.
#[test]
fn the_desktop_signs_the_fixtures_copies_byte_for_byte() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(COPY_FIXTURE);
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let seed = vec![fx["signer_master_byte"].as_u64().expect("the seed byte") as u8; 32];
    let key = fx["public_key"].as_str().expect("the key").to_string();
    assert_eq!(key, hex::encode(crate::net::identity::pq_object_keypair(&seed).public_key()), "the fixture's key is its seed's");
    let cases = fx["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 4, "four cases");
    for (i, case) in cases.iter().enumerate() {
        let frame = &case["frame"];
        let now_ms = case["now_ms"].as_u64().expect("the clock");
        let mut gs = GuiState::default();
        gs.server_url = ONE.to_string();
        gs.private_key_bytes = Some(seed.clone());
        gs.profile_public_key = key.clone();
        let signed = take_own_copy_at(&mut gs, ONE, frame, "ann", now_ms).unwrap_or_else(|| panic!("case {i}: asked, so signed"));
        let f: serde_json::Value = serde_json::from_str(&signed).unwrap();
        assert_eq!(f["type"], "profile_sign", "case {i}");
        assert_eq!(f["at"], case["at"], "case {i}: the time");
        assert_eq!(f["signature"], case["signature"], "case {i}: the signature, byte for byte");

        let copy = current_copy(&gs).unwrap_or_else(|| panic!("case {i}: filed")).clone();
        let name = frame["name"].as_str().unwrap();
        let signed_at = frame["signed_at"].as_u64().unwrap();
        let pure = sign_frame(&seed, &key, name, &copy, signed_at, 0, now_ms).unwrap_or_else(|| panic!("case {i}: within the limits"));
        assert_eq!(pure, f, "case {i}: the pure signer makes the same frame");

        let fixed: PublicCopy = serde_json::from_value(case["copy"].clone()).expect("a copy");
        assert_eq!(public_copy_of(frame), fixed, "case {i}: the fixture's copy is the frame's");
        assert!(
            profile_copy::verify_copy(&key, &fixed, f["at"].as_u64().unwrap(), f["signature"].as_str().unwrap()),
            "case {i}: it verifies over the fixture's copy"
        );
    }
}

/// Each copy this app signs for a server is dated after the clock, after the copy that server
/// holds (`signed_at`), and after the last one this app signed for it, so two saves in the same
/// millisecond, or a server whose word lags behind a copy signed here, never give two copies the
/// same time or an older one. Another server's times are its own.
///
/// Seen red 2026-10-12 with `signed_here` not recorded in `take_own_copy_at`: "dated after the
/// last one signed here" failed.
#[test]
fn each_edit_is_dated_after_the_newest_seen_on_that_server() {
    let (mut gs, _sent, key) = ann_unlocked_on(ONE);
    let first = at_of(take_own_copy_at(&mut gs, ONE, &anns_copy_asking(&key, 0), "Ann", NOW_MS));
    assert_eq!(first, NOW_MS, "nothing newer seen: the clock");
    let second = at_of(take_own_copy_at(&mut gs, ONE, &anns_copy_asking(&key, 0), "Ann", NOW_MS));
    assert_eq!(second, NOW_MS + 1, "dated after the last one signed here");
    let ahead = NOW_MS + 90_000; // a copy from a device whose clock runs fast
    let third = at_of(take_own_copy_at(&mut gs, ONE, &anns_copy_asking(&key, ahead), "Ann", NOW_MS));
    assert_eq!(third, ahead + 1, "dated after the copy the server holds");
    let fourth = at_of(take_own_copy_at(&mut gs, ONE, &anns_copy_asking(&key, 0), "Ann", NOW_MS + 1_000));
    assert_eq!(fourth, ahead + 2, "and after it still, while the server's word lags");
    let on_two = at_of(take_own_copy_at(&mut gs, TWO, &anns_copy_asking(&key, 0), "Ann", NOW_MS));
    assert_eq!(on_two, NOW_MS, "a server's times are its own");
}

/// With the identity locked (the seed not in memory), a seed that is not 32 bytes, or no key,
/// nothing is signed and nothing is sent; the copy is filed all the same. Unlocked, the same ask
/// is signed.
///
/// Seen red 2026-10-12 with the lock check taken out of `take_own_copy_at` (the seed read as
/// `private_key_bytes.clone().unwrap_or_default()`, the key check kept): "locked: nothing signed"
/// failed.
#[test]
fn a_locked_identity_signs_nothing() {
    let (mut gs, sent) = ann_on(ONE);
    let (seed, key) = seed_and_key();
    gs.profile_public_key = key.clone(); // the app knows who she is; the seed stays locked away
    let asking = anns_copy_asking(&key, 0);
    assert!(take_own_copy(&mut gs, ONE, &asking, "Ann").is_none(), "locked: nothing signed");
    assert!(current_copy(&gs).is_some(), "the copy is filed all the same");
    gs.private_key_bytes = Some(seed[..16].to_vec());
    assert!(take_own_copy(&mut gs, ONE, &asking, "Ann").is_none(), "a seed that is not 32 bytes signs nothing");
    gs.private_key_bytes = Some(seed.clone());
    gs.profile_public_key = String::new();
    assert!(take_own_copy(&mut gs, ONE, &asking, "Ann").is_none(), "no key, nothing signed");
    pump(&mut gs);
    assert!(frames(&sent).iter().all(|f| f["type"] != "profile_sign"), "nothing sent");
    assert!(gs.own_profile.signed_here.is_empty(), "no time recorded");
    gs.profile_public_key = key;
    assert!(take_own_copy(&mut gs, ONE, &asking, "Ann").is_some(), "unlocked, the same ask is signed");
}

/// AN ERASE CARRIES HER WITHDRAWAL (BUG-191 Wave 2B): Settings > Account's erase sends the typed
/// name and the withdrawal of the copy of her profile that other servers keep, signed with her key
/// and dated after the copy the server she is on holds (its last own copy said a minute ahead,
/// without asking her to sign anything), so the server keeps it and passes it on before it erases.
/// The relay reads the frame as an erase with that withdrawal. A copy addressed to another of her
/// keys, and another server's copy, do not move the time; a second press is dated after the
/// first. Locked, the erase goes without one.
///
/// Seen red 2026-10-12 with `held_at` not recorded in `take_own_copy_at`: "dated after the copy
/// the server holds" (left 1760000000000, right 1760000060001); and with the withdrawal left out
/// of `erase_frame_at`: "the erase carries the withdrawal".
#[test]
fn an_erase_carries_her_withdrawal_dated_after_the_copy_held() {
    let (mut gs, _sent, key) = ann_unlocked_on(ONE);
    let ahead = NOW_MS + 60_000;
    let mut quiet = anns_copy_asking(&key, ahead);
    quiet["sign_now"] = false.into();
    assert!(take_own_copy_at(&mut gs, ONE, &quiet, "Ann", NOW_MS).is_none(), "test setup: nothing asked, nothing signed");
    let mut phones = quiet.clone();
    phones["target"] = "ab".repeat(profile_copy::KEY_HEX_LEN / 2).into();
    phones["signed_at"] = (ahead + 900_000).into();
    take_own_copy_at(&mut gs, ONE, &phones, "Ann", NOW_MS);
    let mut on_two = quiet.clone();
    on_two["signed_at"] = (ahead + 500_000).into();
    take_own_copy_at(&mut gs, TWO, &on_two, "Ann", NOW_MS);

    let f = erase_frame_at(&mut gs, "Ann", NOW_MS);
    assert_eq!((f["type"].as_str(), f["confirm_name"].as_str()), (Some("account_delete"), Some("Ann")));
    let w = &f["withdrawal"];
    assert!(w.is_object(), "the erase carries the withdrawal: {f}");
    assert_eq!(w.as_object().unwrap().len(), 2, "the time and the signature only: {w}");
    let at = w["at"].as_u64().expect("a time");
    assert_eq!(at, ahead + 1, "dated after the copy the server holds");
    let sig = w["signature"].as_str().expect("a signature");
    assert!(profile_copy::verify_copy(&key, &profile_copy::withdrawal(), at, sig), "her signature over the withdrawal");
    assert!(!profile_copy::verify_copy(&key, &public_copy_of(&quiet), at, sig), "and over nothing else");
    match serde_json::from_value::<crate::relay::relay::RelayMessage>(f.clone()) {
        Ok(crate::relay::relay::RelayMessage::AccountDelete { confirm_name, withdrawal: Some(w) }) => {
            assert_eq!((confirm_name.as_str(), w.at, w.signature.as_str()), ("Ann", at, sig), "the relay reads what was sent");
        }
        other => panic!("the relay reads it as an erase with its withdrawal: {other:?}"),
    }

    let again = erase_frame_at(&mut gs, "Ann", NOW_MS);
    assert_eq!(again["withdrawal"]["at"].as_u64(), Some(at + 1), "a second press is dated after the first");

    gs.private_key_bytes = None;
    let locked = erase_frame_at(&mut gs, "Ann", NOW_MS);
    assert_eq!(locked, serde_json::json!({ "type": "account_delete", "confirm_name": "Ann" }), "locked: the erase goes without one");
}
