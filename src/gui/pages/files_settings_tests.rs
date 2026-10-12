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

/// A choice made again after another device's was taken goes again (the second review of
/// 2026-10-11: the record of what was sent was never cleared, so the same word was never sent
/// twice in a run, and the device kept "Nobody" while the server told senders "Anyone").
///
/// Seen red 2026-10-11 with `accept_sent` not cleared on the server's confirmation: "made again,
/// it goes again" failed.
#[test]
fn a_choice_made_again_goes_again() {
    let (mut gs, sent) = app("friends");
    gs.settings.files_in_messages = FilesInMessages { accept_from: AcceptFrom::Nobody, accept_unsent: true, ..Default::default() };
    own_profile::pump(&mut gs);
    assert_eq!(updates(&sent).len(), 1);
    own_profile::keep_copy(&mut gs, SERVER, server_copy("nobody")); // confirmed
    own_profile::pump(&mut gs);
    own_profile::keep_copy(&mut gs, SERVER, server_copy("anyone")); // another device chose Anyone
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Anyone, "taken here");
    gs.settings.files_in_messages.accept_from = AcceptFrom::Nobody; // and Nobody again, here
    gs.settings.files_in_messages.accept_unsent = true;
    gs.own_profile.last_sent.insert(own_profile::current_server(&gs), Instant::now() - Duration::from_secs(40));
    own_profile::pump(&mut gs);
    let f = updates(&sent).pop().expect("made again, it goes again");
    assert_eq!(f["accepts_files"], "nobody");
}

// BUG-200: the choice is one per device, and each server holds its own copy of the word.

const TWO: &str = "https://two.example";

/// The person's own copy from a server with no profile for her: what the relay answers her own
/// profile request with (an empty profile and its default word, "friends").
fn no_profile_copy() -> ServerProfile {
    let frame = serde_json::json!({ "type": "profile_data", "name": "Ada", "bio": "", "socials": "{}", "accepts_files": "friends", "privacy": "{}" });
    own_profile::own_copy(&frame, "Ada").expect("her own copy")
}

/// "Accept files from" as this device holds it: `from`, chosen here or not, and the words seen
/// (each for Ada, the person the app is signed in as).
fn prefs(from: AcceptFrom, chosen: bool, seen: &[(&str, &str)]) -> FilesInMessages {
    FilesInMessages {
        accept_from: from,
        accept_chosen: chosen,
        accept_seen: seen.iter().map(|(s, w)| (ada_on(s), w.to_string())).collect(),
        ..Default::default()
    }
}

/// The key a word Ada's profile holds on `server` is remembered under.
fn ada_on(server: &str) -> String {
    crate::net::files_policy::accept_seen_key(server, "me")
}

/// A device whose person chose "Anyone" meets a server with no profile for her (the relay answers
/// "friends", its default): the choice is published there instead of the default being taken,
/// and it counts as saved, and seen, when the server's copy carries it.
///
/// Seen red 2026-10-11 with `accept_step` counting a server never seen as one another device
/// changed (`changed_there` true when no word was seen): "the choice is published there" failed
/// (the server's default was taken instead).
#[test]
fn a_device_that_chose_publishes_its_choice_on_a_server_with_no_profile() {
    let (mut gs, sent) = app("anyone");
    gs.settings.files_in_messages = prefs(AcceptFrom::Anyone, true, &[(SERVER, "anyone")]);
    own_profile::pump(&mut gs);
    assert!(updates(&sent).is_empty(), "the first server holds it already");

    gs.server_url = TWO.into();
    own_profile::keep_copy(&mut gs, TWO, no_profile_copy());
    own_profile::pump(&mut gs);
    let f = updates(&sent).pop().expect("the choice is published there");
    assert_eq!(f["accepts_files"], "anyone");
    assert_eq!(f["bio"], "", "with that server's own (empty) profile");
    let p = &gs.settings.files_in_messages;
    assert_eq!(p.accept_from, AcceptFrom::Anyone, "the default is not taken");
    assert!(p.accept_unsent, "waiting until that server's copy carries it");

    let mut echo = no_profile_copy();
    echo.accepts_files = Some("anyone".into());
    own_profile::keep_copy(&mut gs, TWO, echo);
    own_profile::pump(&mut gs);
    let p = &gs.settings.files_in_messages;
    assert!(!p.accept_unsent, "saved");
    assert_eq!(p.accept_seen.get(&ada_on(TWO)).map(String::as_str), Some("anyone"), "and seen there");
}

/// A device whose person never chose takes the word of a server it meets, sends nothing, and
/// remembers the word as seen there.
///
/// Seen red 2026-10-11 with `accept_step` publishing for every device (`accept_chosen` ignored):
/// "the server's word is taken" failed (still Friends).
#[test]
fn a_device_that_never_chose_takes_the_servers_word() {
    let (mut gs, sent) = app("nobody");
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "the server's word is taken");
    assert!(updates(&sent).is_empty(), "nothing is sent");
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&ada_on(SERVER)).map(String::as_str), Some("nobody"), "seen there");
    assert!(!gs.settings.files_in_messages.accept_chosen, "taking is not choosing");
}

/// A server whose word changed since this device last saw it (another device changed it there):
/// taken, even on a device whose person chose, and nothing is sent over it.
///
/// Seen red 2026-10-11 with the seen word ignored (`changed_there` always false): "another
/// device's change is taken" failed (still Anyone).
#[test]
fn a_word_changed_since_this_device_saw_it_is_taken() {
    let (mut gs, sent) = app("nobody");
    gs.settings.files_in_messages = prefs(AcceptFrom::Anyone, true, &[(SERVER, "anyone")]);
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "another device's change is taken");
    assert!(updates(&sent).is_empty(), "nothing goes over it");
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&ada_on(SERVER)).map(String::as_str), Some("nobody"));
}

/// Two servers holding different words, each seen before (the second never got this device's
/// choice): switching between them flips nothing. The choice goes to the second server once in a
/// run, even when it does not answer and the person switches away and back.
///
/// Seen red 2026-10-11 twice: with the rule as it was (any different word taken while nothing
/// waits), "switching to the second server flips nothing" failed (Friends); with `accept_sent`
/// one entry that the first server's copy cleared (as the Option it was), "not sent again on
/// switching back" failed.
#[test]
fn switching_between_two_servers_flips_nothing() {
    let (mut gs, sent) = app("anyone");
    gs.settings.files_in_messages = prefs(AcceptFrom::Anyone, true, &[(SERVER, "anyone"), (TWO, "friends")]);
    own_profile::keep_copy(&mut gs, TWO, server_copy("friends"));
    own_profile::pump(&mut gs);

    gs.server_url = TWO.into();
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Anyone, "switching to the second server flips nothing");
    assert_eq!(updates(&sent).pop().expect("the choice goes there")["accepts_files"], "anyone");

    // No answer from the second server; back to the first, and to the second again.
    gs.server_url = SERVER.into();
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Anyone, "nor back");
    gs.server_url = TWO.into();
    gs.own_profile.last_sent.insert(own_profile::current_server(&gs), Instant::now() - Duration::from_secs(40));
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Anyone);
    assert!(updates(&sent).is_empty(), "not sent again on switching back");

    // Its copy carries it at last: seen there.
    own_profile::keep_copy(&mut gs, TWO, server_copy("anyone"));
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&ada_on(TWO)).map(String::as_str), Some("anyone"));
    assert!(!gs.settings.files_in_messages.accept_unsent);
}

/// The protected setup never takes a more open word: not one another device set on a server, not
/// one a device that never chose meets. A more closed one is taken.
///
/// Seen red 2026-10-11 with `accept_step` taking the server's word without asking
/// `adopt_from_server` (no protected check): "not another device's more open word" failed.
#[test]
fn the_protected_setup_never_takes_a_more_open_word() {
    let (mut gs, _sent) = app("anyone");
    gs.protected.setup.on = true;
    gs.settings.files_in_messages = prefs(AcceptFrom::Friends, true, &[(SERVER, "friends")]);
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Friends, "not another device's more open word");
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&ada_on(SERVER)).map(String::as_str), Some("friends"), "and not recorded as settled");

    let (mut gs, _sent) = app("anyone");
    gs.protected.setup.on = true;
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Friends, "not for a device that never chose");

    let (mut gs, _sent) = app("nobody");
    gs.protected.setup.on = true;
    gs.settings.files_in_messages = prefs(AcceptFrom::Friends, true, &[(SERVER, "friends")]);
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Nobody, "a more closed word is taken");
}

/// A restore from a different recovery phrase on this device: the words seen were the first
/// person's, so on a server holding the default for the new one (no profile there), the device's
/// own choice is published instead of the old person's seen word turning the setting. Back as the
/// first person, their seen word still holds.
///
/// Seen red 2026-10-12 with `accept_seen_key` ignoring the person (the server alone): "the
/// device's choice stands for the new person" failed (Friends taken).
#[test]
fn a_different_person_on_this_device_starts_with_no_word_seen() {
    let (mut gs, sent) = app("anyone");
    gs.settings.files_in_messages = prefs(AcceptFrom::Anyone, true, &[(SERVER, "anyone")]);
    own_profile::pump(&mut gs);
    assert!(updates(&sent).is_empty(), "test setup: settled for the first person");

    // Restored as Bea: the copies were the old identity's (apply_pq_identity resets them).
    gs.profile_public_key = "bea".into();
    gs.own_profile = Default::default();
    own_profile::keep_copy(&mut gs, SERVER, no_profile_copy());
    own_profile::pump(&mut gs);
    assert_eq!(gs.settings.files_in_messages.accept_from, AcceptFrom::Anyone, "the device's choice stands for the new person");
    assert_eq!(updates(&sent).pop().expect("and goes to her profile there")["accepts_files"], "anyone");
    let bea = crate::net::files_policy::accept_seen_key(SERVER, "bea");
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&bea), None, "nothing seen for her before her copy carries it");
    assert_eq!(gs.settings.files_in_messages.accept_seen.get(&ada_on(SERVER)).map(String::as_str), Some("anyone"), "the first person's word stays");
}
