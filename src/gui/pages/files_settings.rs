//! Settings > Privacy, "Files in messages", and Settings > Data, "Kept files" (2026-10-11,
//! docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", parts 3
//! and 4; the operator's answers 6 and 9).
//!
//! FILES IN MESSAGES: who this person takes files from (Nobody / Friends / Anyone I talk to), whose
//! pictures show without a click (Friends / Nobody; the protected setup is always Nobody), whether
//! the attach button shows in private conversations ("receive only" is off), and the attach step's
//! remembered switch. The receiving app enforces them (chat/attach_view.rs `plan`); the server
//! cannot and is not asked to.
//!
//! PUBLISHING "Accept files from" (`accepts_files` in the person's profile on the server they are
//! on, so a sender's app can say so before anything is sent), by the web's rules of 2026-10-11:
//! - the server saves a profile update WHOLE (a field left out is saved empty), so the update that
//!   carries the word carries the whole profile: this device's own fields where it has them, the
//!   server's copy for the rest (`GET /api/members/{key}` and `/api/profile/{key}`), so publishing
//!   the choice never wipes a bio or a picture set elsewhere;
//! - the word goes only while a choice made on this device is waiting (`accept_unsent`, kept in the
//!   config, so a choice made offline still goes), at most once every 31 seconds (the server's own
//!   limit is one profile update per 30);
//! - while nothing waits, a different word on the server is another device's newer choice, and this
//!   device takes it, except a more open one while the protected setup is on.
//! Under the protected setup the choice can be closed here but not opened (the web asks for the
//! PIN; this app has no PIN step for it, so turning the setup off comes first).
//!
//! KEPT FILES: how much of the private files the person opened this device keeps (net/
//! kept_files.rs), what they take now, and "Clear kept files".

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::net::files_policy::{self as policy, AcceptFrom, ShowPictures};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// This person's own profile as their server holds it, the fields a profile update saves.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ServerProfile {
    pub bio: String,
    pub avatar_url: String,
    pub banner_url: String,
    pub pronouns: String,
    pub location: String,
    pub website: String,
    /// JSON text, "{}" when there are none.
    pub socials: String,
    /// The word the server holds for them.
    pub accepts_files: Option<String>,
}

/// Keeping this person's own choice in step with their server (`FilesFlow::own`).
#[derive(Default)]
pub struct OwnSync {
    /// (server, key) the profile below is for, when it came, and the profile (None: no answer).
    got: Option<((String, String), Instant, Option<ServerProfile>)>,
    asking: Option<((String, String), Receiver<Option<ServerProfile>>)>,
    last_sent: Option<Instant>,
}

/// How often the server's copy is asked for again (another device may have chosen since).
const ASK_AGAIN: Duration = Duration::from_secs(600);
/// The least time between two profile updates from here (the relay allows one per 30 seconds).
const SEND_GAP: Duration = Duration::from_secs(31);

/// The server's copy from its member answer and its profile answer (either may be missing).
pub(crate) fn server_profile_from(member: &serde_json::Value, profile: Option<&serde_json::Value>) -> ServerProfile {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut p = ServerProfile {
        bio: s(member, "bio"),
        avatar_url: s(member, "avatar_url"),
        location: s(member, "location"),
        socials: "{}".to_string(),
        accepts_files: member.get("accepts_files").and_then(|x| x.as_str()).map(str::to_string),
        ..Default::default()
    };
    if let Some(pr) = profile {
        p.banner_url = s(pr, "banner_url");
        p.pronouns = s(pr, "pronouns");
        p.website = s(pr, "website");
        if let Some(soc) = pr.get("socials").filter(|v| v.is_object()) {
            p.socials = soc.to_string();
        }
    }
    p
}

/// The profile update that publishes `accepts_files`: the whole profile (a profile update saves
/// every field), this device's bio and picture where it has them, the server's copy for the rest.
pub(crate) fn profile_update_with_accepts(state: &GuiState, server: &ServerProfile) -> serde_json::Value {
    let mine_or = |mine: &str, theirs: &str| if mine.trim().is_empty() { theirs.to_string() } else { mine.trim().to_string() };
    let privacy = if state.profile_directory_listed { "{}" } else { "{\"directory\":\"unlisted\"}" };
    serde_json::json!({
        "type": "profile_update",
        "bio": mine_or(&state.profile_network_bio, &server.bio),
        "socials": server.socials,
        "avatar_url": mine_or(&state.profile_network_avatar, &server.avatar_url),
        "banner_url": server.banner_url,
        "pronouns": server.pronouns,
        "location": server.location,
        "website": server.website,
        "privacy": privacy,
        "accepts_files": state.settings.files_in_messages.accept_from.word(),
    })
}

/// Ask the server for this person's own profile, on a worker thread.
fn ask_own(state: &mut GuiState, k: (String, String)) {
    let (tx, rx) = std::sync::mpsc::channel();
    let base = state.server_url.trim_end_matches('/').to_string();
    let key = k.1.clone();
    std::thread::spawn(move || {
        let get = |url: String| {
            ureq::get(&url)
                .timeout(Duration::from_secs(10))
                .call()
                .ok()
                .and_then(|r| r.into_string().ok())
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        };
        let member = get(format!("{base}/api/members/{key}"));
        let profile = get(format!("{base}/api/profile/{key}"));
        let _ = tx.send(member.map(|m| server_profile_from(&m, profile.as_ref())));
    });
    state.files_flow.own.asking = Some((k, rx));
}

/// Every frame of the chat page and of Settings > Privacy: keep "Accept files from" in step with the
/// server this person is on (see the module notes).
pub(crate) fn sync_accepts(state: &mut GuiState) {
    let connected = state.ws_client.as_ref().is_some_and(|c| c.is_connected());
    let server = crate::gui::pages::chat::norm_server_url(&state.server_url);
    if !connected || server.is_empty() || state.profile_public_key.is_empty() {
        return;
    }
    let k = (server, state.profile_public_key.clone());
    if let Some((asked, rx)) = &state.files_flow.own.asking {
        match rx.try_recv() {
            Ok(got) => {
                let asked = asked.clone();
                state.files_flow.own.got = Some((asked, Instant::now(), got));
                state.files_flow.own.asking = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => state.files_flow.own.asking = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        }
    }
    let fresh = state.files_flow.own.got.as_ref().is_some_and(|(g, at, _)| *g == k && at.elapsed() < ASK_AGAIN);
    if !fresh {
        ask_own(state, k);
        return;
    }
    let Some((_, _, Some(server_copy))) = state.files_flow.own.got.clone() else { return };
    let prefs = state.settings.files_in_messages.clone();
    if prefs.accept_unsent {
        if state.files_flow.own.last_sent.is_some_and(|t| t.elapsed() < SEND_GAP) {
            return;
        }
        let fields = [state.profile_network_bio.clone(), state.profile_network_avatar.clone()];
        if crate::engine::warnings::guard_stops(state, &[&fields[0], &fields[1]]) {
            return;
        }
        let frame = profile_update_with_accepts(state, &server_copy).to_string();
        if let Some(client) = state.ws_client.as_ref() {
            client.send(&frame);
        }
        state.files_flow.own.last_sent = Some(Instant::now());
        state.settings.files_in_messages.accept_unsent = false;
        state.settings_dirty = true;
        if let Some((_, _, Some(p))) = state.files_flow.own.got.as_mut() {
            p.accepts_files = Some(prefs.accept_from.word().to_string());
        }
        return;
    }
    let protected = crate::engine::protected::is_on(state);
    if let Some(take) = server_copy.accepts_files.as_deref().and_then(|w| policy::adopt_from_server(&prefs, w, protected)) {
        state.settings.files_in_messages.accept_from = take;
        state.settings_dirty = true;
    }
}

/// Settings > Privacy, "Files in messages".
pub(crate) fn draw_files_in_messages(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    sync_accepts(state);
    let hint = state.settings.hint_display;
    let protected = crate::engine::protected::is_on(state);
    ui.label(RichText::new("Files in messages").size(theme.font_size_title).color(theme.text_primary()));
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "For direct messages and groups. Your own app keeps to these: the server cannot see which \
         messages carry files, and is not told.",
    );
    ui.add_space(theme.spacing_sm);

    let current = state.settings.files_in_messages.accept_from;
    let mut accept = current;
    widgets::form_row(ui, theme, "Accept files from", |ui| {
        for a in AcceptFrom::ALL {
            // The protected setup: closing it is fine, opening it waits until the setup is off.
            let allowed = !protected || a.openness() <= current.openness();
            ui.add_enabled_ui(allowed, |ui| {
                ui.radio_value(&mut accept, a, a.label());
            });
        }
    });
    if accept != current {
        state.settings.files_in_messages.accept_from = accept;
        state.settings.files_in_messages.accept_unsent = true;
        state.settings_dirty = true;
        sync_accepts(state);
    }
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "A file from anyone else is not downloaded: you are asked first, and Accept takes that one \
         file. Your choice is in your profile on the server you are on, so people see it before \
         they send.",
    );
    if protected {
        ui.label(RichText::new("The protected setup: this can be closed here, not opened.").size(theme.font_size_small).color(theme.text_muted()));
    }

    let mut show = state.settings.files_in_messages.show_pictures_from;
    widgets::form_row(ui, theme, "Show pictures without asking from", |ui| {
        if protected {
            ui.label(RichText::new("Nobody (the protected setup)").color(theme.text_secondary()));
        } else {
            ui.radio_value(&mut show, ShowPictures::Friends, "Friends");
            ui.radio_value(&mut show, ShowPictures::Nobody, "Nobody");
        }
    });
    if show != state.settings.files_in_messages.show_pictures_from {
        state.settings.files_in_messages.show_pictures_from = show;
        state.settings_dirty = true;
    }

    if widgets::toggle(ui, theme, "Show the attach button in private conversations", &mut state.settings.files_in_messages.attach_button) {
        state.settings_dirty = true;
    }
    widgets::setting_hint(ui, theme, hint, "Off: you still receive files, but send none from a direct message or a group.");

    if protected {
        ui.label(RichText::new(policy::PROTECTED_ALWAYS).size(theme.font_size_small).color(theme.text_secondary()));
    } else if widgets::toggle(ui, theme, policy::SWITCH_LABEL, &mut state.settings.files_in_messages.remove_details) {
        state.settings_dirty = true;
    }
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "Every file you send starts with this, in chat, Shared Files and Media, and the attach step \
         remembers your last choice. Groups have no file rules of their own yet; these apply there too.",
    );
}

/// Settings > Data, "Kept files".
pub(crate) fn draw_kept_files(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    let dir = crate::net::kept_files::kept_dir();
    let size_words = crate::gui::widgets::file_browser::human_size;
    ui.label(RichText::new("Kept files").color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "Private files you opened in a direct message or a group are kept on this device, encrypted \
         under your identity, so they still open after the server removes its copy (after 30 days on \
         most servers). When this is full, the ones opened longest ago go first.",
    );
    let cap_mb = state.settings.files_in_messages.kept_cap_mb;
    let used = crate::net::kept_files::used(&dir);
    ui.label(RichText::new(format!("Using {} of {}.", size_words(used), size_words(cap_mb.saturating_mul(1024 * 1024)))).color(theme.text_primary()));
    let mut mb = cap_mb;
    let (lo, hi) = policy::KEPT_CAP_RANGE_MB;
    widgets::form_row(ui, theme, "Keep up to (MB)", |ui| {
        widgets::slider(ui, theme, egui::Slider::new(&mut mb, lo..=hi).logarithmic(true));
    });
    if mb != cap_mb {
        state.settings.files_in_messages.kept_cap_mb = mb;
        state.settings_dirty = true;
        crate::net::kept_files::evict(&dir, mb.saturating_mul(1024 * 1024));
    }
    if widgets::secondary_button(ui, theme, "Clear kept files") {
        let (n, bytes) = crate::net::kept_files::clear(&dir);
        state.files_flow.kept_note = format!("Removed {n} kept file{} ({}).", if n == 1 { "" } else { "s" }, size_words(bytes));
    }
    if !state.files_flow.kept_note.is_empty() {
        ui.label(RichText::new(&state.files_flow.kept_note).size(theme.font_size_small).color(theme.success()));
    }
}

#[cfg(test)]
#[path = "files_settings_tests.rs"]
mod tests;
