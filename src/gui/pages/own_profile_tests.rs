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
