//! This person's own profile on each server they use (2026-10-11, BUG-193, BUG-194, BUG-196).
//!
//! WHICH PROFILE IS THEIRS. The server sends every profile as `profile_data`: the person's own copy
//! (on connect, as the echo of each save, which goes to every device signed in under their name,
//! and as the answer when they ask for their own) and everyone else's (to everyone when someone
//! saves theirs, and as the answer to a profile request, which is addressed to the asker, so
//! `target` alone cannot tell them apart). A frame is about this person when it names the person
//! the socket signed in as (`names_socket`); it is their own COMPLETE copy when it also carries
//! their privacy map (`own_copy`), which the server puts on their own copies only. The public copy
//! of their own save names them too, but leaves pronouns, location and website out.
//!
//! ONE COPY PER SERVER. A person has a separate profile on each server. Every socket, the active
//! one and those kept open in the background, files its own copy under its server's address
//! (`keep_copy`), so switching servers has that server's copy at once. When the server the app is on
//! has sent none yet (a server switched to without a fresh connect, or a person with no profile
//! there), the app asks for it (`profile_request` for their own name, which the server answers with
//! their complete copy, or an empty one with no profile yet).
//!
//! SAVING THE WHOLE PROFILE. The server saves a profile update whole: a field left out is saved
//! empty. So every save (the Profile page, the privacy choices, publishing "Accept files from")
//! asks for one (`want_save`) and `pump`, every frame, sends it once that server's copy is here
//! and the server's limit of one update per 30 seconds allows: this device's bio and picture, the
//! server's copy for every field this app does not edit (banner, pronouns, location, website,
//! links), and the server's privacy map with only the directory choice changed. Asks that come
//! close together go as one update. A waiting "Accept files from" goes with it and counts as
//! saved only when that server's copy comes back carrying it (files_settings.rs).
//!
//! THE PROFILE PAGE'S FIELDS. When the copy for the server the app is on is new to the page, it
//! fills the bio, picture and directory choice where the person has not changed them since the
//! last copy the page took (from whichever server), so an unsaved edit stays, and switching
//! servers shows that server's profile.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::gui::pages::files_settings::ServerProfile;
use crate::gui::GuiState;

/// The least time between two profile updates to one server (the relay allows one per 30 seconds).
pub(crate) const SEND_GAP: Duration = Duration::from_secs(31);
/// How long to wait before asking a server for the person's own copy again.
const ASK_AGAIN: Duration = Duration::from_secs(30);
/// How long a sent save waits for its echo before the Profile page says it was not taken.
const ECHO_WAIT: Duration = Duration::from_secs(15);
/// The Profile page's note for a save the server did not take.
pub(crate) const NOT_TAKEN: &str = "Not saved: the server turned it down (the chat says why).";

/// Everything this app keeps about the person's own profile (`GuiState::own_profile`).
#[derive(Default)]
pub struct OwnProfile {
    /// Each server's own complete copy, by normalised server address.
    pub(crate) copies: HashMap<String, ServerProfile>,
    /// The copy the Profile page's fields last took, and its server: the untouched rule compares
    /// against it, whatever server it came from.
    pub(crate) shown: Option<(String, ServerProfile)>,
    /// A save is wanted on this server (normalised address), sent by `pump`.
    pub(crate) wanted: Option<String>,
    /// When the last update went to each server.
    pub(crate) last_sent: HashMap<String, Instant>,
    /// When the app last asked each server for the person's own copy.
    pub(crate) asked: HashMap<String, Instant>,
    /// A save sent to this server, and when, whose echo has not come back yet (the Profile page's
    /// note). Unanswered after ECHO_WAIT, the server turned it down (it says why in the chat).
    pub(crate) awaiting_echo: Option<(String, Instant)>,
    /// The "Accept files from" word last asked to go to a server, so a choice is sent once per
    /// server in a run of the app (files_settings.rs `sync_accepts`).
    pub(crate) accept_sent: Option<(String, String)>,
}

/// The normalised address of the server the app is on.
pub(crate) fn current_server(state: &GuiState) -> String {
    crate::gui::pages::chat::norm_server_url(&state.server_url)
}

/// Whether a `profile_data` frame names the person this socket signed in as. Names are matched
/// without regard to case, as the server holds them.
pub(crate) fn names_socket(val: &serde_json::Value, socket_name: &str) -> bool {
    !socket_name.is_empty()
        && val.get("name").and_then(|v| v.as_str()).is_some_and(|n| n.to_lowercase() == socket_name.to_lowercase())
}

/// This person's own complete copy, from a `profile_data` frame that is one: it names them and
/// carries their privacy map. A field the frame leaves out is empty (the server leaves out empty
/// fields on the own copy).
pub(crate) fn own_copy(val: &serde_json::Value, socket_name: &str) -> Option<ServerProfile> {
    if !names_socket(val, socket_name) {
        return None;
    }
    let privacy = val.get("privacy")?.as_str()?.to_string();
    let s = |k: &str| val.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let socials = s("socials");
    Some(ServerProfile {
        bio: s("bio"),
        avatar_url: s("avatar_url"),
        banner_url: s("banner_url"),
        pronouns: s("pronouns"),
        location: s("location"),
        website: s("website"),
        socials: if socials.trim().is_empty() { "{}".to_string() } else { socials },
        accepts_files: val.get("accepts_files").and_then(|x| x.as_str()).map(str::to_string),
        privacy,
    })
}

/// File a server's own copy (from any socket, active or in the background).
pub(crate) fn keep_copy(state: &mut GuiState, server: &str, copy: ServerProfile) {
    let server = crate::gui::pages::chat::norm_server_url(server);
    if state.own_profile.awaiting_echo.as_ref().is_some_and(|(s, at)| *s == server && at.elapsed() < ECHO_WAIT) {
        state.own_profile.awaiting_echo = None;
        if server == current_server(state) {
            state.profile_network_saved_note = "Saved to server.".to_string();
        }
    }
    state.own_profile.copies.insert(server, copy);
}

/// The copy of the server the app is on, when it has come.
pub(crate) fn current_copy(state: &GuiState) -> Option<&ServerProfile> {
    state.own_profile.copies.get(&current_server(state))
}

/// Whether the directory choice in a privacy map is "listed" (anything but `"unlisted"`).
fn listed_in(privacy: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(privacy)
        .ok()
        .and_then(|p| p.get("directory").and_then(|d| d.as_str()).map(|d| d != "unlisted"))
        .unwrap_or(true)
}

/// Show `copy` on the Profile page: its bio, picture and directory choice go into the fields that
/// still hold what the last copy shown said (or are empty when none was), so an unsaved edit stays.
fn show(state: &mut GuiState, server: &str, copy: &ServerProfile) {
    let before = state.own_profile.shown.as_ref().map(|(_, p)| p.clone());
    let untouched = |field: &str, was: Option<&str>| match was {
        Some(w) => field.trim() == w.trim(),
        None => field.trim().is_empty(),
    };
    if untouched(&state.profile_network_bio, before.as_ref().map(|p| p.bio.as_str())) {
        state.profile_network_bio = copy.bio.clone();
    }
    if untouched(&state.profile_network_avatar, before.as_ref().map(|p| p.avatar_url.as_str())) {
        state.profile_network_avatar = copy.avatar_url.clone();
    }
    let listed_before = before.as_ref().map(|p| listed_in(&p.privacy)).unwrap_or(true);
    if state.profile_directory_listed == listed_before {
        state.profile_directory_listed = listed_in(&copy.privacy);
    }
    state.own_profile.shown = Some((server.to_string(), copy.clone()));
}

/// The server's privacy map with only the directory choice set from this device: unlisted, or the
/// key taken out (listed). Anything else in it (a per-field choice made on the website) stays.
pub(crate) fn privacy_with_directory(server_privacy: &str, listed: bool) -> String {
    let mut map = serde_json::from_str::<serde_json::Value>(server_privacy)
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let obj = map.as_object_mut().expect("an object");
    if listed {
        obj.remove("directory");
    } else {
        obj.insert("directory".into(), serde_json::Value::String("unlisted".into()));
    }
    map.to_string()
}

/// The whole profile update: `bio` and `avatar` from this device, every other field from `server`,
/// and `server`'s privacy map with the directory choice `listed`. `accepts_files` goes only while a
/// choice made on this device waits (`accept`); left out, the server keeps the stored word.
pub(crate) fn whole_update(server: &ServerProfile, bio: &str, avatar: &str, listed: bool, accept: Option<&str>) -> serde_json::Value {
    let mut f = serde_json::json!({
        "type": "profile_update",
        "bio": bio.trim(),
        "socials": if server.socials.trim().is_empty() { "{}" } else { server.socials.as_str() },
        "avatar_url": avatar.trim(),
        "banner_url": server.banner_url,
        "pronouns": server.pronouns,
        "location": server.location,
        "website": server.website,
        "privacy": privacy_with_directory(&server.privacy, listed),
    });
    if let Some(word) = accept {
        f["accepts_files"] = word.into();
    }
    f
}

/// The update `pump` sends for the server the app is on, from its copy: this device's bio and
/// picture as they stand, the directory choice, and a waiting "Accept files from".
pub(crate) fn save_frame(state: &GuiState, copy: &ServerProfile) -> serde_json::Value {
    let prefs = &state.settings.files_in_messages;
    let accept = prefs.accept_unsent.then(|| prefs.accept_from.word());
    whole_update(copy, &state.profile_network_bio, &state.profile_network_avatar, state.profile_directory_listed, accept)
}

/// Ask for the person's profile to be saved on the server the app is on (the Profile page, a
/// privacy choice, "Accept files from"). `pump` sends it.
pub(crate) fn want_save(state: &mut GuiState) {
    state.own_profile.wanted = Some(current_server(state));
}

/// Every frame: show a copy new to the Profile page, ask a server for the person's own copy when it
/// has sent none, keep "Accept files from" in step, and send a wanted save when it may go.
pub(crate) fn pump(state: &mut GuiState) {
    let server = current_server(state);
    let connected = state.ws_client.as_ref().is_some_and(|c| c.is_connected()) && state.ws_identified;
    if server.is_empty() {
        return;
    }
    // A sent save never answered: the server turned it down.
    if state.own_profile.awaiting_echo.as_ref().is_some_and(|(_, at)| at.elapsed() >= ECHO_WAIT) {
        state.own_profile.awaiting_echo = None;
        state.profile_network_saved_note = NOT_TAKEN.to_string();
    }
    // A wanted save for a server the app has left is dropped (the person was looking at that one).
    if state.own_profile.wanted.as_ref().is_some_and(|w| *w != server) {
        state.own_profile.wanted = None;
    }
    let Some(copy) = state.own_profile.copies.get(&server).cloned() else {
        let due = state.own_profile.asked.get(&server).is_none_or(|t| t.elapsed() >= ASK_AGAIN);
        if connected && due {
            if let Some(client) = state.ws_client.as_ref() {
                let name = client.user_name().to_string();
                if !name.is_empty() {
                    client.send(&serde_json::json!({ "type": "profile_request", "name": name }).to_string());
                    state.own_profile.asked.insert(server, Instant::now());
                }
            }
        }
        return;
    };
    if state.own_profile.shown.as_ref().is_none_or(|(s, p)| *s != server || *p != copy) {
        show(state, &server, &copy);
    }
    crate::gui::pages::files_settings::sync_accepts(state);
    if state.own_profile.wanted.is_none() || !connected {
        return;
    }
    if state.own_profile.last_sent.get(&server).is_some_and(|t| t.elapsed() < SEND_GAP) {
        return;
    }
    // Step F's recovery-phrase guard on the fields this sends: holding the phrase, nothing goes.
    let fields = [state.profile_network_bio.clone(), state.profile_network_avatar.clone()];
    if crate::engine::warnings::guard_stops(state, &[&fields[0], &fields[1]]) {
        state.own_profile.wanted = None;
        state.profile_network_saved_note = "Not saved: a profile field holds your recovery phrase.".to_string();
        return;
    }
    let frame = save_frame(state, &copy).to_string();
    if let Some(client) = state.ws_client.as_ref() {
        client.send(&frame);
    }
    state.own_profile.wanted = None;
    state.own_profile.last_sent.insert(server.clone(), Instant::now());
    state.own_profile.awaiting_echo = Some((server, Instant::now()));
    state.profile_network_saved_note = "Saving to the server...".to_string();
}

#[cfg(test)]
#[path = "own_profile_tests.rs"]
mod tests;
