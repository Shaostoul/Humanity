//! Settings > Safety (step B of docs/design/blocking-and-safe-mode.md, section 10c, 2026-10-09):
//! "Who can reach me", one row per kind of contact the server checks (Messages, Calls, Trades)
//! with the five audiences in plain words and a line under each saying what the current choice
//! means; "People who may call me", the person's friends with a tick each; and the Requests
//! list (also in Chat, under DMs).
//!
//! What the rows show is what the SERVER last said (`reach_settings`, kept in the DM store per
//! server), never what was clicked: a click sends `reach_set` and the row moves when the server
//! answers, so the page cannot show a choice the server is not enforcing. A tick re-issues that
//! friend's pass with or without `call` (engine/dm.rs `reissue_pass`).
//!
//! Persistence: nothing here is an AppConfig setting. The audiences live on the server (and in
//! the encrypted DM store as the last word heard), the ticks and requests in the DM store.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::net::reach::{Audience, ReachKind};

/// The section's content, drawn inside its tinted band by `pages::settings::draw`. `accent` is
/// the band's colour, which the subsection headers wear.
pub(crate) fn draw_safety_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    widgets::subsection_header(
        ui,
        theme,
        accent,
        "Who can reach me",
        "Choose who may contact you on this server. The server checks every message, call and trade \
         request against these choices, for everyone: admins and moderators included.",
    );
    if !crate::engine::dm::ensure_dm_store(state) {
        widgets::body_hint(
            ui,
            theme,
            "Connect to a server (with your identity unlocked) to choose who can reach you there. \
             Each server keeps its own settings.",
        );
        return;
    }
    let server_said = crate::engine::reach::current(state);
    let shown = server_said.unwrap_or_default();

    // Where the rows' values came from, and whether a change is still on its way.
    let note = match state.reach.asked {
        Some((_, at)) if at.elapsed() < crate::engine::reach::REACH_ANSWER_WAIT => {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            Some("Saving: waiting for the server to confirm.")
        }
        Some(_) => Some("The server has not confirmed the change. It may not support these settings yet."),
        None if server_said.is_none() => Some("These are the safe defaults. The server has not sent your settings yet."),
        None => None,
    };
    if let Some(note) = note {
        ui.label(RichText::new(note).size(theme.font_size_small).color(theme.warning()));
    }
    if !state.reach.status.is_empty() {
        ui.label(RichText::new(&state.reach.status).size(theme.font_size_small).color(theme.warning()));
    }
    ui.add_space(theme.spacing_sm);

    let mut clicked: Option<(ReachKind, Audience)> = None;
    for kind in ReachKind::ALL {
        let current = shown.get(kind);
        widgets::card(ui, theme, |ui| {
            ui.set_min_width(ui.available_width());
            widgets::subsection_label(ui, theme, kind.label());
            ui.horizontal_wrapped(|ui| {
                for audience in Audience::ALL {
                    let active = current == audience;
                    // Offline, a click says to connect (engine/reach.rs `ask`) rather than
                    // greying the choices out, so the page still reads plainly.
                    if widgets::Button::secondary(audience.label()).active(active).show(ui, theme) && !active {
                        clicked = Some((kind, audience));
                    }
                }
            });
            ui.add_space(theme.spacing_xs);
            widgets::body_hint(ui, theme, current.meaning(kind));
        });
        ui.add_space(theme.spacing_sm);
    }
    if let Some((kind, audience)) = clicked {
        crate::engine::reach::ask(state, kind, audience);
    }

    draw_may_call_list(ui, theme, state, accent, shown.call);

    widgets::subsection_header(
        ui,
        theme,
        accent,
        "Requests",
        "People who are not your friends can ask to be in contact. A request shows only a name. \
         Accept follows them back, which makes you friends; Ignore removes it and tells no one.",
    );
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        draw_requests_list(ui, theme, state);
    });
}

/// "People who may call me": each friend (a mutual follow) with a tick. Ticking re-issues the
/// pass they hold from us with `call` in it; unticking re-issues it without.
fn draw_may_call_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32, calls: Audience) {
    widgets::subsection_header(
        ui,
        theme,
        accent,
        "People who may call me",
        "Tick a friend to let them call you: the pass they hold from you is re-issued with calling \
         in it. Untick to take it away again.",
    );
    let Some(store) = state.dm_store.as_ref() else { return };
    let out_of_step = store.passes_out_of_step();
    let mut friends: Vec<(String, String, bool)> = store
        .following()
        .iter()
        .filter(|k| store.is_follower(k))
        .map(|k| (k.clone(), crate::engine::dm::dm_display_name(state, k), store.may_call(k)))
        .collect();
    friends.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    let mut toggled: Option<(String, bool)> = None;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        match calls {
            Audience::Chosen => {}
            Audience::Nobody => widgets::body_hint(ui, theme, "Calls is set to Nobody, so no one can call you, ticked or not."),
            other => widgets::body_hint(
                ui,
                theme,
                &format!("Calls is set to {}, so these ticks only decide who can call you if you change it to People I choose.", other.label()),
            ),
        }
        if friends.is_empty() {
            widgets::body_hint(ui, theme, "No friends on this server yet. People you follow who follow you back appear here.");
        }
        for (key, name, on) in &friends {
            ui.horizontal(|ui| {
                let mut tick = *on;
                if widgets::custom_checkbox(ui, theme, &mut tick) {
                    toggled = Some((key.clone(), tick));
                }
                ui.label(RichText::new(name).size(theme.font_size_body).color(theme.text_primary()));
                if out_of_step.contains(key) {
                    ui.label(RichText::new("(updating their pass)").size(theme.font_size_small).color(theme.text_muted()));
                }
            });
        }
    });
    if let Some((key, on)) = toggled {
        crate::engine::reach::set_may_call(state, &key, on);
    }
}

/// The Requests list: each request's name only, with Accept and Ignore. Shared by Settings >
/// Safety and the chat page's left rail, so the two never drift.
pub(crate) fn draw_requests_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let requests = state.dm_store.as_ref().map(|s| s.requests().to_vec()).unwrap_or_default();
    if requests.is_empty() {
        widgets::body_hint(ui, theme, "No requests.");
        return;
    }
    enum Act {
        Accept(String),
        Ignore(String),
    }
    let mut act: Option<Act> = None;
    for req in &requests {
        // The name the server's member list holds for the sender's signed key, never one the
        // request claimed (10c).
        let (name, on_list) = crate::engine::reach::request_name(state, req);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&name).size(theme.font_size_body).color(theme.text_primary()).strong());
            if !on_list {
                ui.label(RichText::new("(not on this server's member list right now)").size(theme.font_size_small).color(theme.text_muted()));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::success("Accept").tooltip("Follow them back: you become friends and can message each other.").show(ui, theme) {
                act = Some(Act::Accept(req.key.clone()));
            }
            if widgets::Button::secondary("Ignore").tooltip("Remove this request. They are not told.").show(ui, theme) {
                act = Some(Act::Ignore(req.key.clone()));
            }
        });
        ui.add_space(theme.spacing_xs);
    }
    match act {
        Some(Act::Accept(did)) => crate::engine::reach::accept_request(state, &did),
        Some(Act::Ignore(did)) => crate::engine::reach::ignore_request(state, &did),
        None => {}
    }
}
