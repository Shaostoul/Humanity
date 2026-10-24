//! Settings > Privacy, "Calls and voice rooms" (docs/design/blocking-and-safe-mode.md 10p,
//! 2026-10-22): how this person's calls connect, and who can see their network address.
//!
//! Three choices, explained on screen in the words of data/net/call_connection.json (the web
//! chat's Settings > Safety reads the same file, so both apps say the same thing): Private (the
//! default: only the server's call relay is offered to the other side), Direct (this device's own
//! addresses too, found with the server's own STUN responder), and Direct also asking a public
//! service the person picks. Each mode's one-line summary, which says who can see the address, is
//! always shown, whatever the hint setting; the longer detail follows the hint setting like every
//! other row.
//!
//! The choice applies to calls and voice rooms started from then on (src/engine/call_relay.rs
//! hands it to the WebRTC manager every frame). With the protected setup on, every call is
//! Private and the choice cannot be opened here.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::net::call_connection::{self as cc, CallConnection};

/// Settings > Privacy, "Calls and voice rooms".
pub(crate) fn draw_calls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    let words = cc::words();
    let protected = crate::engine::protected::is_on(state);
    let heading = if words.heading.is_empty() { "Calls and voice rooms" } else { words.heading.as_str() };
    ui.label(RichText::new(heading).size(theme.font_size_title).color(theme.text_primary()));
    if !words.intro.is_empty() {
        widgets::setting_hint(ui, theme, hint, &words.intro);
    }
    ui.add_space(theme.spacing_sm);
    if protected && !words.protected_line.is_empty() {
        ui.label(RichText::new(&words.protected_line).size(theme.font_size_small).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
    }

    let current = state.settings.call_connection;
    let mut chosen = current;
    for mode in CallConnection::ALL {
        let mw = words.mode(mode);
        // The protected setup: everything is Private, and nothing else can be picked here.
        ui.add_enabled_ui(!protected || mode == CallConnection::Private, |ui| {
            ui.radio_value(&mut chosen, mode, RichText::new(&mw.label).color(theme.text_primary()));
        });
        ui.indent(("call_connection_words", mode.id()), |ui| {
            if !mw.summary.is_empty() {
                ui.label(RichText::new(&mw.summary).size(theme.font_size_small).color(theme.text_secondary()));
            }
            if !mw.detail.is_empty() {
                widgets::setting_hint(ui, theme, hint, &mw.detail);
            }
        });
    }
    if chosen != current {
        state.settings.call_connection = chosen;
        state.settings_dirty = true;
    }

    // The third mode: which public service, named, so the person knows who sees the address.
    if state.settings.call_connection == CallConnection::DirectPublicStun && !protected && !words.public_stun.is_empty() {
        let picked = words.public_stun(&state.settings.call_public_stun).map(|s| s.id.clone()).unwrap_or_default();
        let mut pick = picked.clone();
        let label = if words.public_stun_label.is_empty() { "Which service" } else { words.public_stun_label.as_str() };
        widgets::form_row(ui, theme, label, |ui| {
            for s in &words.public_stun {
                ui.radio_value(&mut pick, s.id.clone(), &s.name);
            }
        });
        if pick != picked || state.settings.call_public_stun.is_empty() {
            state.settings.call_public_stun = pick;
            state.settings_dirty = true;
        }
    }
}
