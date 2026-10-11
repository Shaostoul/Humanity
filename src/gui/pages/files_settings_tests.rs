//! Tests for files_settings.rs: publishing "Accept files from" by the web's rules (the whole
//! profile, only while a choice made here waits, at most once per 31 seconds) and taking another
//! device's choice from the server.

use super::*;
use crate::net::files_policy::{AcceptFrom, FilesInMessages};

const SERVER: &str = "https://files.example";

/// A signed-in, connected app (its socket records what it sends), and its server copy of the
/// person's profile as if just asked for.
fn app(server_copy: ServerProfile) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.server_url = SERVER.into();
    gs.profile_public_key = "me".into();
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    let k = (crate::gui::pages::chat::norm_server_url(SERVER), "me".to_string());
    gs.files_flow.own.got = Some((k, Instant::now(), Some(server_copy)));
    (gs, sent)
}

fn server_copy(word: &str) -> ServerProfile {
    let member = serde_json::json!({ "name": "Ada", "bio": "Gardener", "avatar_url": "/uploads/a.png", "location": "Lisbon", "accepts_files": word });
    let profile = serde_json::json!({ "banner_url": "/uploads/b.png", "pronouns": "she", "website": "https://ada.example", "socials": { "x": "ada" } });
    server_profile_from(&member, Some(&profile))
}

/// A choice made here goes once, in a profile update that carries the WHOLE profile (the server's
/// copy where this device has nothing, so nothing set elsewhere is wiped) and the word; then
/// nothing more goes. Seen red 2026-10-11 with the update built from this device's fields alone (as
/// the older senders do): "the bio set elsewhere is kept" failed (it went out empty).
#[test]
fn a_choice_made_here_goes_once_with_the_whole_profile() {
    let (mut gs, sent) = app(server_copy("friends"));
    gs.settings.files_in_messages = FilesInMessages { accept_from: AcceptFrom::Nobody, accept_unsent: true, ..Default::default() };
    sync_accepts(&mut gs);
    let frames: Vec<serde_json::Value> = sent.try_iter().map(|t| serde_json::from_str(&t).unwrap()).collect();
    assert_eq!(frames.len(), 1, "one update");
    let f = &frames[0];
    assert_eq!(f["type"], "profile_update");
    assert_eq!(f["accepts_files"], "nobody");
    assert_eq!(f["bio"], "Gardener", "the bio set elsewhere is kept");
    for (field, want) in [("avatar_url", "/uploads/a.png"), ("location", "Lisbon"), ("banner_url", "/uploads/b.png"), ("pronouns", "she"), ("website", "https://ada.example")] {
        assert_eq!(f[field], want, "{field} is sent as the server holds it");
    }
    assert_eq!(f["socials"], "{\"x\":\"ada\"}");
    assert!(!gs.settings.files_in_messages.accept_unsent && gs.settings_dirty, "nothing waits any more");

    // Changed again at once: it waits for the server's limit, then goes.
    gs.settings.files_in_messages.accept_from = AcceptFrom::Anyone;
    gs.settings.files_in_messages.accept_unsent = true;
    sync_accepts(&mut gs);
    assert!(sent.try_iter().next().is_none(), "not within 31 seconds of the last update");
    gs.files_flow.own.last_sent = Some(Instant::now() - Duration::from_secs(40));
    sync_accepts(&mut gs);
    assert_eq!(sent.try_iter().count(), 1, "then it goes");

    // This device's own bio wins where it has one.
    gs.profile_network_bio = "Beekeeper".into();
    let f = profile_update_with_accepts(&gs, &server_copy("friends"));
    assert_eq!(f["bio"], "Beekeeper");
}

/// While nothing waits here, no update goes, and a different word on the server (another device's
/// newer choice) is taken, never a more open one under the protected setup. Seen red 2026-10-11
/// with `sync_accepts` sending the word whenever it differed from the server's: "nothing is sent
/// while nothing waits".
#[test]
fn another_devices_choice_is_taken_and_nothing_is_sent() {
    let (mut gs, sent) = app(server_copy("nobody"));
    sync_accepts(&mut gs);
    assert!(sent.try_iter().next().is_none(), "nothing is sent while nothing waits");
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "another device's choice is taken");

    let (mut gs, _sent) = app(server_copy("anyone"));
    gs.protected.setup.on = true;
    sync_accepts(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Friends, "never more open under the protected setup");
}
