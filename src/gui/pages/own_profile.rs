//! This person's own profile on the server they are on (2026-10-11, BUG-193 and BUG-194).
//!
//! WHICH PROFILE IS THEIRS. The server sends every profile as `profile_data`: the person's own copy
//! (on connect, and the echo of each save, which goes to every device they are signed in on) and
//! everyone else's (to everyone when someone saves theirs, and as the answer to a profile request,
//! which is addressed to the asker, so `target` alone cannot tell them apart). A frame is about this
//! person when it names the person this socket signed in as (`names_socket`); it is their own
//! COMPLETE copy when it also carries their privacy map (`own_copy`), which the server puts on
//! their own copies only. The public copy of their own save names them too, but leaves pronouns,
//! location and website out, so it is never taken as the server's copy.
//!
//! SAVING THE WHOLE PROFILE. The server saves a profile update whole: a field left out is saved
//! empty. So every save from this app sends the whole profile (`whole_update`): this device's bio
//! and picture, the server's copy for each field this app does not edit (banner, pronouns,
//! location, website, links), and the server's privacy map with only the directory choice changed.
//! Before the own copy has come (or from a server older than v0.1486.0, which sends none) those
//! fields go empty, as every save from this app did before 2026-10-11.
//!
//! THE NETWORK PROFILE'S FIELDS. A copy that comes in fills the bio, picture and directory choice
//! on the Profile page where the person has not changed them since the last copy (`take_in`), so a
//! save echoed back, or a reconnect, never throws away an edit they have not saved.

use crate::gui::pages::files_settings::ServerProfile;
use crate::gui::GuiState;

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

/// Whether the directory choice in a privacy map is "listed" (anything but `"unlisted"`).
fn listed_in(privacy: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(privacy)
        .ok()
        .and_then(|p| p.get("directory").and_then(|d| d.as_str()).map(|d| d != "unlisted"))
        .unwrap_or(true)
}

/// Take in this person's own copy from the server `server`: kept as the server's copy, and the
/// Profile page's bio, picture and directory choice filled where they still hold what the last
/// copy said (or are empty and nothing came before), so an unsaved edit stays.
pub(crate) fn take_in(state: &mut GuiState, server: &str, copy: ServerProfile) {
    let before = state.own_server_profile.as_ref().filter(|(s, _)| s == server).map(|(_, p)| p.clone());
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
    state.own_server_profile = Some((server.to_string(), copy));
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

/// The server's copy for the server this app is on now, or an empty one before it has come.
pub(crate) fn server_copy(state: &GuiState) -> ServerProfile {
    state
        .own_server_profile
        .as_ref()
        .filter(|(s, _)| s == &state.server_url)
        .map(|(_, p)| p.clone())
        .unwrap_or_default()
}

/// The whole profile update: `bio` and `avatar` from this device, every other field from `server`,
/// and `server`'s privacy map with the directory choice `listed`. `accepts_files` is not sent
/// (the server keeps the stored word); Files in messages adds it when publishing a choice.
pub(crate) fn whole_update(server: &ServerProfile, bio: &str, avatar: &str, listed: bool) -> serde_json::Value {
    serde_json::json!({
        "type": "profile_update",
        "bio": bio.trim(),
        "socials": if server.socials.trim().is_empty() { "{}" } else { server.socials.as_str() },
        "avatar_url": avatar.trim(),
        "banner_url": server.banner_url,
        "pronouns": server.pronouns,
        "location": server.location,
        "website": server.website,
        "privacy": privacy_with_directory(&server.privacy, listed),
    })
}

/// The save from the Profile page and the privacy choices: this device's bio and picture as they
/// stand, the rest from the server's copy.
pub(crate) fn save_frame(state: &GuiState) -> serde_json::Value {
    save_frame_listed(state, state.profile_directory_listed)
}

/// As `save_frame`, with the directory choice `listed` (a privacy choice being made now).
pub(crate) fn save_frame_listed(state: &GuiState, listed: bool) -> serde_json::Value {
    whole_update(&server_copy(state), &state.profile_network_bio, &state.profile_network_avatar, listed)
}

#[cfg(test)]
#[path = "own_profile_tests.rs"]
mod tests;
