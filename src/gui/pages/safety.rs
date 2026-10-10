//! Settings > Safety (step B of docs/design/blocking-and-safe-mode.md, section 10c, 2026-10-09):
//! "Who can reach me", one row per kind of contact the server checks (Messages, Calls, Trades)
//! with the five audiences in plain words and a line under each saying what the current choice
//! means; "People who may call me", the person's friends with a tick each; the Requests
//! list (also in Chat, under DMs), where a request can also be blocked; Warnings (step F, 10g),
//! the "Warnings on messages" switch; and Blocked people (step C, 10d), each with the date and
//! Unblock.
//!
//! What the rows show is what the SERVER last said (`reach_settings`, kept in the DM store per
//! server), never what was clicked: a click sends `reach_set` and the row moves when the server
//! answers, so the page cannot show a choice the server is not enforcing. A tick re-issues that
//! friend's pass with or without `call` (engine/dm.rs `reissue_pass`).
//!
//! Persistence: one AppConfig setting, "Warnings on messages" (step F, 10g, saved through
//! `settings_dirty` and checked by tests/settings_persistence_lint.rs, which scans this file as
//! well as settings.rs). The audiences live on the server (and in the encrypted DM store as the
//! last word heard), the ticks and requests in the DM store, and the block list in its own
//! encrypted file per identity (net/block_list.rs).

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
        // The warnings switch and the block list are this device's, not a server's, so they are
        // shown offline too.
        draw_warnings_switch(ui, theme, state, accent);
        draw_blocked_people(ui, theme, state, accent);
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

    draw_warnings_switch(ui, theme, state, accent);
    draw_blocked_people(ui, theme, state, accent);
}

/// Settings > Safety > Warnings (step F of docs/design/blocking-and-safe-mode.md, 10g): the
/// "Warnings on messages" switch, On by default and saved with the other settings (AppConfig
/// `warnings_on_messages`, through `settings_dirty`), and the plain account of what the warnings
/// and the recovery-phrase guard do and cannot do (6.5).
pub(crate) fn draw_warnings_switch(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    widgets::subsection_header(
        ui,
        theme,
        accent,
        "Warnings",
        "A short explanation under a direct message or a group message that asks for money or for \
         your recovery phrase, claims to be staff, wants to move you to another app, or pushes you \
         to hurry or keep a secret. A warning never blocks a message and never reports anyone.",
    );
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        if widgets::toggle(ui, theme, "Warnings on messages", &mut state.settings.warnings_on_messages) {
            state.settings_dirty = true;
        }
        widgets::body_hint(
            ui,
            theme,
            "On: warnings show under messages from people who are not your friends, and the ones about \
             money and your recovery phrase under friends' messages too, because a friend's account can \
             be taken over. Off: no warnings are shown.",
        );
        widgets::body_hint(
            ui,
            theme,
            "Whatever this switch says, links in a direct message from someone who is not your friend \
             open only when you choose Open, the way their pictures wait for a click.",
        );
        widgets::body_hint(
            ui,
            theme,
            "Messages are end-to-end encrypted. Nobody, including server admins, can read them to look \
             for danger. Warnings are checked on this device only.",
        );
        widgets::body_hint(
            ui,
            theme,
            "Your recovery phrase is never sent, and this has no switch: if anything you are about to \
             send holds four or more of its words in order, it stops and nothing leaves this device. \
             This works while your identity is unlocked.",
        );
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
        Block(String),
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
            // Instead of Ignore, for someone you never want to hear from (step C).
            if widgets::Button::danger("Block")
                .tooltip("Remove this request and hide everything from them from now on. They are not told.")
                .show(ui, theme)
            {
                act = Some(Act::Block(req.key.clone()));
            }
        });
        ui.add_space(theme.spacing_xs);
    }
    match act {
        Some(Act::Accept(did)) => crate::engine::reach::accept_request(state, &did),
        Some(Act::Ignore(did)) => crate::engine::reach::ignore_request(state, &did),
        Some(Act::Block(did)) => crate::engine::block::block(state, &did),
        None => {}
    }
}

/// Settings > Safety > Blocked people (step C of docs/design/blocking-and-safe-mode.md, 10d): each
/// person by the member list's name (or the start of their key), the date they were blocked, and
/// Unblock. The list is this identity's on this device, so it is the same on every server.
pub(crate) fn draw_blocked_people(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    widgets::subsection_header(
        ui,
        theme,
        accent,
        "Blocked people",
        "You see nothing from someone you block: no messages, posts, reactions, calls, trade requests \
         or contact requests, on every server you use from this device. Blocking also takes back the \
         pass you gave them. They are not told. Your other devices on the same server learn it too.",
    );
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        crate::engine::block::ensure_block_list(state);
        let entries = state.block_list.as_ref().map(|l| l.entries()).unwrap_or_default();
        if state.block_list.is_none() {
            widgets::body_hint(ui, theme, "Unlock your identity to see the people you blocked.");
        } else if entries.is_empty() {
            widgets::body_hint(ui, theme, "Nobody. Block someone from a message's menu, their profile, a DM or a contact request.");
        }
        let mut lift: Option<String> = None;
        for (key, at) in &entries {
            let (name, on_list) = crate::engine::reach::member_name(state, key);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&name).size(theme.font_size_body).color(theme.text_primary()).strong());
                if !on_list {
                    ui.label(RichText::new("(not on this server's member list right now)").size(theme.font_size_small).color(theme.text_muted()));
                }
                ui.label(RichText::new(format!("Blocked {}", blocked_on(*at))).size(theme.font_size_small).color(theme.text_muted()));
                if widgets::Button::secondary("Unblock")
                    .tooltip("Show their messages again. They are not told, and nothing is given back: to be friends again, follow them.")
                    .show(ui, theme)
                {
                    lift = Some(key.clone());
                }
            });
            ui.add_space(theme.spacing_xs);
        }
        if let Some(key) = lift {
            crate::engine::block::unblock(state, &key);
        }
    });
    widgets::body_hint(
        ui,
        theme,
        "What blocking cannot do: it cannot stop them seeing your public posts or public profile, cannot stop \
         someone making a new key, cannot remove them from a public channel, voice room or group you do not \
         run, and cannot stop them standing near you in a shared world.",
    );
}

/// The day a block was made, `YYYY-MM-DD` (UTC).
fn blocked_on(ms: u64) -> String {
    let stamp = crate::gui::pages::game_admin::format_ban_date(ms as i64);
    stamp.split(' ').next().unwrap_or("").to_string()
}
