//! Tests for files_settings.rs: publishing "Accept files from" by the web's rules (in the whole
//! profile, only while a choice made here waits, counted as saved only when the server's copy
//! carries it) and taking another device's choice from the server.

use super::*;
use crate::gui::pages::own_profile;
use crate::net::files_policy::{AcceptFrom, FilesInMessages};
use std::time::{Duration, Instant};

const SERVER: &str = "https://files.example";

/// Ada signed in and connected (her socket records what it sends), with her server's own copy of
/// her profile holding `word`.
fn app(word: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.server_url = SERVER.into();
    gs.profile_public_key = "me".into();
    let (client, sent) = crate::net::ws_client::WsClient::recording_as("Ada");
    gs.ws_client = Some(client);
    gs.ws_identified = true;
    own_profile::keep_copy(&mut gs, SERVER, server_copy(word));
    (gs, sent)
}

fn server_copy(word: &str) -> ServerProfile {
    let frame = serde_json::json!({
        "type": "profile_data", "name": "Ada", "bio": "Gardener", "avatar_url": "https://a.example/a.png",
        "location": "Lisbon", "banner_url": "https://a.example/b.png", "pronouns": "she",
        "website": "https://ada.example", "socials": "{\"x\":\"ada\"}", "accepts_files": word, "privacy": "{}",
    });
    own_profile::own_copy(&frame, "Ada").expect("her own copy")
}

fn updates(sent: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    sent.try_iter().map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap()).filter(|f| f["type"] == "profile_update").collect()
}

/// A choice made here goes in a profile update that carries the WHOLE profile (the server's copy
/// where this device has nothing, so nothing set elsewhere is wiped) and the word; it keeps waiting
/// until the server's copy carries it, and is not sent again meanwhile (an update the server turns
/// down says why in the chat; it does not go every 31 seconds).
///
/// Seen red 2026-10-11 (BUG-196) with `sync_accepts` counting the choice as saved when sent, as it
/// did: "still waiting until the server's copy carries it" failed.
#[test]
fn a_choice_made_here_goes_with_the_whole_profile_and_waits_for_the_server() {
    let (mut gs, sent) = app("friends");
    gs.settings.files_in_messages = FilesInMessages { accept_from: AcceptFrom::Nobody, accept_unsent: true, ..Default::default() };
    own_profile::pump(&mut gs);
    let frames = updates(&sent);
    assert_eq!(frames.len(), 1, "one update");
    let f = &frames[0];
    assert_eq!(f["accepts_files"], "nobody");
    assert_eq!(f["bio"], "Gardener", "the bio set elsewhere is kept");
    for (field, want) in [("avatar_url", "https://a.example/a.png"), ("location", "Lisbon"), ("banner_url", "https://a.example/b.png"), ("pronouns", "she"), ("website", "https://ada.example")] {
        assert_eq!(f[field], want, "{field} is sent as the server holds it");
    }
    assert_eq!(f["socials"], "{\"x\":\"ada\"}");
    assert!(gs.settings.files_in_messages.accept_unsent, "still waiting until the server's copy carries it");

    // No echo (the server turned it down): not sent again, even after the server's limit.
    gs.own_profile.last_sent.insert(own_profile::current_server(&gs), Instant::now() - Duration::from_secs(40));
    own_profile::pump(&mut gs);
    assert!(updates(&sent).is_empty(), "not sent again and again");

    // The echo carries it: saved.
    own_profile::keep_copy(&mut gs, SERVER, server_copy("nobody"));
    own_profile::pump(&mut gs);
    assert!(!gs.settings.files_in_messages.accept_unsent && gs.settings_dirty, "nothing waits any more");
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "and the choice stands");

    // A new choice goes, once the server's limit allows.
    gs.settings.files_in_messages.accept_from = AcceptFrom::Anyone;
    gs.settings.files_in_messages.accept_unsent = true;
    own_profile::pump(&mut gs);
    let f = updates(&sent).pop().expect("the new choice goes");
    assert_eq!(f["accepts_files"], "anyone");
}

/// While nothing waits here, no update goes, and a different word on the server (another device's
/// newer choice) is taken, never a more open one under the protected setup. Seen red 2026-10-11
/// with `sync_accepts` sending the word whenever it differed from the server's: "nothing is sent
/// while nothing waits".
#[test]
fn another_devices_choice_is_taken_and_nothing_is_sent() {
    let (mut gs, sent) = app("nobody");
    own_profile::pump(&mut gs);
    assert!(updates(&sent).is_empty(), "nothing is sent while nothing waits");
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "another device's choice is taken");

    let (mut gs, _sent) = app("anyone");
    gs.protected.setup.on = true;
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Friends, "never more open under the protected setup");
}
