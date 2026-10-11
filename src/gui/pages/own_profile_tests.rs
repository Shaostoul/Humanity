// Child of own_profile.rs (#[path]): which profile is this person's own, and the whole profile
// every save sends (BUG-193, BUG-194, 2026-10-11).

use super::*;

const ANN_PRIVACY: &str = r#"{"directory":"unlisted","location":"friends"}"#;

/// Ann's own copy as the server sends it on connect and as the echo of her save: every field she
/// has, and her privacy map.
fn anns_own_copy() -> serde_json::Value {
    serde_json::json!({
        "type": "profile_data", "name": "Ann", "bio": "Beekeeper", "socials": r#"{"site":"x"}"#,
        "avatar_url": "https://a.example/ann.png", "banner_url": "https://a.example/banner.png",
        "pronouns": "she/her", "location": "Lisbon", "website": "https://ann.example",
        "accepts_files": "anyone", "privacy": ANN_PRIVACY, "target": "ann_key",
    })
}

fn signed_in_on(server: &str) -> GuiState {
    let mut gs = GuiState::default();
    gs.server_url = server.to_string();
    gs
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
    let mut gs = signed_in_on("https://one.example");
    take_in(&mut gs, "https://one.example", own_copy(&anns_own_copy(), "Ann").unwrap());
    gs.profile_network_bio = "Beekeeper and baker".into();
    gs.profile_directory_listed = true;
    let f = save_frame(&gs);
    assert_eq!(f["type"], "profile_update");
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

    gs.profile_directory_listed = false;
    let privacy: serde_json::Value = serde_json::from_str(save_frame(&gs)["privacy"].as_str().unwrap()).unwrap();
    assert_eq!(privacy, serde_json::json!({ "directory": "unlisted", "location": "friends" }));
}

/// A copy coming in fills the Profile page's fields she has not touched, and leaves an unsaved edit.
#[test]
fn a_copy_coming_in_fills_untouched_fields_and_keeps_an_unsaved_edit() {
    let mut gs = signed_in_on("https://one.example");
    take_in(&mut gs, "https://one.example", own_copy(&anns_own_copy(), "Ann").unwrap());
    assert_eq!(gs.profile_network_bio, "Beekeeper", "filled on connect");
    assert_eq!(gs.profile_network_avatar, "https://a.example/ann.png");
    assert!(!gs.profile_directory_listed, "her server says unlisted");

    gs.profile_network_bio = "Half-typed".into();
    let mut newer = anns_own_copy();
    newer["bio"] = "Changed on her phone".into();
    newer["avatar_url"] = "https://a.example/new.png".into();
    take_in(&mut gs, "https://one.example", own_copy(&newer, "Ann").unwrap());
    assert_eq!(gs.profile_network_bio, "Half-typed", "an unsaved edit stays");
    assert_eq!(gs.profile_network_avatar, "https://a.example/new.png", "an untouched field follows the server");
    assert_eq!(server_copy(&gs).bio, "Changed on her phone", "the server's copy is the newest");
}

/// The server's copy belongs to the server it came from: on another server a save does not carry
/// it (that server sends its own on connect).
#[test]
fn the_servers_copy_is_used_only_on_that_server() {
    let mut gs = signed_in_on("https://one.example");
    take_in(&mut gs, "https://one.example", own_copy(&anns_own_copy(), "Ann").unwrap());
    gs.server_url = "https://two.example".into();
    assert_eq!(server_copy(&gs), ServerProfile::default());
    assert_eq!(save_frame(&gs)["pronouns"], "");
}
