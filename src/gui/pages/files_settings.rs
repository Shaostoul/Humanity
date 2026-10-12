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
//! PUBLISHING "Accept files from" (`accepts_files` in the person's profile on each server they
//! use, so a sender's app can say so before anything is sent). The choice is one per device; each
//! server holds its own copy of the word:
//! - the server saves a profile update WHOLE (a field left out is saved empty), so the word goes in
//!   the whole profile every save sends (own_profile.rs), built from that server's own copy, so
//!   publishing the choice never wipes a bio or a picture set elsewhere;
//! - the word goes only while it waits (`accept_unsent`, kept in the config, so a choice made
//!   offline still goes), and it counts as saved only when that server's copy comes back carrying
//!   it (BUG-196: it used to count as saved when sent, so an update the server turned down was
//!   lost, and the server's old word was then taken back);
//! - which word wins on a server is `files_policy::accept_step` (BUG-200): this device remembers
//!   the word it last saw each server hold (`accept_seen`). A server holding a different word now
//!   is one where another device changed it, and this device takes it (never a more open one while
//!   the protected setup is on). A server still holding what this device last saw, or one it has
//!   never seen (a new server, or one with no profile for the person, which answers "friends"),
//!   is sent this device's choice if the person ever chose here (`accept_chosen`); a device that
//!   never chose takes that server's word. Before BUG-200 any different word was taken whenever
//!   nothing waited, so a server that never got the choice turned it back, and switching between
//!   two servers flipped it.
//! Under the protected setup the choice can be closed here but not opened (the web asks for the
//! PIN; this app has no PIN step for it, so turning the setup off comes first).
//!
//! KEPT FILES: how much of the private files the person opened this device keeps (net/
//! kept_files.rs), what they take now, and "Clear kept files".

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::net::files_policy::{self as policy, AcceptFrom, AcceptStep, ShowPictures};

/// This person's own profile as their server holds it, the fields a profile update saves, from the
/// own copy the server sends over the chat socket (own_profile.rs `own_copy`).
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
    /// The word the server holds for them (None from a server older than the field).
    pub accepts_files: Option<String>,
    /// Their privacy map (JSON text).
    pub privacy: String,
}

/// Every frame, from own_profile.rs `pump` once the server the app is on has sent the person's own
/// copy: keep "Accept files from" in step with it, by `files_policy::accept_step` (see the module
/// notes).
/// - Settled (the server holds this device's word): a waiting choice counts as saved, and the word
///   is recorded as seen there.
/// - Send: the choice waits (`accept_unsent`) until that server's copy carries it, and goes with
///   the whole profile (`own_profile::want_save`) once per choice and server in a run of the app,
///   so an update the server refuses (it says why, in the chat) is not sent again and again.
/// - Take: the server's word becomes this device's, recorded as seen there.
pub(crate) fn sync_accepts(state: &mut GuiState) {
    let server = crate::gui::pages::own_profile::current_server(state);
    let Some(word) = crate::gui::pages::own_profile::current_copy(state).and_then(|c| c.accepts_files.clone()) else { return };
    let protected = crate::engine::protected::is_on(state);
    // The word seen is kept per server and person (a different recovery phrase starts with none).
    let seen_key = policy::accept_seen_key(&server, &state.profile_public_key);
    match policy::accept_step(&state.settings.files_in_messages, &seen_key, &word, protected) {
        AcceptStep::Settled => {
            let prefs = &mut state.settings.files_in_messages;
            if prefs.accept_unsent {
                // A choice of this device's is saved there: this device has chosen.
                prefs.accept_unsent = false;
                prefs.accept_chosen = true;
                state.settings_dirty = true;
            }
            state.own_profile.accept_sent.remove(&server);
            note_seen(state, seen_key, word);
        }
        AcceptStep::Send => {
            let prefs = &mut state.settings.files_in_messages;
            if !prefs.accept_unsent {
                prefs.accept_unsent = true;
                state.settings_dirty = true;
            }
            let mine = prefs.accept_from.word().to_string();
            if state.own_profile.accept_sent.get(&server) != Some(&mine) {
                state.own_profile.accept_sent.insert(server, mine);
                crate::gui::pages::own_profile::want_save(state);
            }
        }
        AcceptStep::Take(take) => {
            state.settings.files_in_messages.accept_from = take;
            state.settings_dirty = true;
            note_seen(state, seen_key, word);
        }
        AcceptStep::Keep => {}
    }
}

/// Record the word a server holds for the person once settled (kept in the config, so it holds
/// across runs). `seen_key` is empty without a server or a key: nothing is remembered then.
fn note_seen(state: &mut GuiState, seen_key: String, word: String) {
    let seen = &mut state.settings.files_in_messages.accept_seen;
    if !seen_key.is_empty() && seen.get(&seen_key) != Some(&word) {
        seen.insert(seen_key, word);
        state.settings_dirty = true;
    }
}

/// Settings > Privacy, "Files in messages".
pub(crate) fn draw_files_in_messages(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
        let prefs = &mut state.settings.files_in_messages;
        prefs.accept_from = accept;
        prefs.accept_unsent = true;
        prefs.accept_chosen = true;
        state.settings_dirty = true;
        state.own_profile.accept_sent.clear(); // a new choice goes, even one made before this run
    }
    widgets::setting_hint(
        ui,
        theme,
        hint,
        "A file from anyone else is not downloaded: you are asked first, and Accept takes that one \
         file. Your choice goes in your profile on each server you use, so people see it before \
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
