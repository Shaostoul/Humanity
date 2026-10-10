//! Settings > Safety (step B of docs/design/blocking-and-safe-mode.md, section 10c, 2026-10-09):
//! "Who can reach me", one row per kind of contact the server checks (Messages, Calls, Trades)
//! with the five audiences in plain words and a line under each saying what the current choice
//! means; "People I choose" (10c-ii), the person's friends, each once with three ticks
//! (Message, Call, Trade); the Requests list (also in Chat, under DMs), where a request can
//! also be blocked; Reports about your groups (10j), drawn by safety_group_reports.rs; Warnings
//! (step F, 10g), the "Warnings on messages" switch; Blocked people (step C, 10d), each with the
//! date and Unblock; and the Protected setup (step G, 10h), drawn by
//! safety_protected.rs, whose always-visible line heads the page while it is on. While it is on,
//! a row, a tick, Accept and turning the warnings off ask its PIN (engine/protected.rs).
//!
//! What the rows show is what the SERVER last said (`reach_settings`, kept in the DM store per
//! server), never what was clicked: a click sends `reach_set` and the row moves when the server
//! answers, so the page cannot show a choice the server is not enforcing. A tick re-issues that
//! friend's pass to allow exactly what is ticked (engine/dm.rs `follow_choice`).
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
use crate::net::reach::{Audience, FriendTicks, ReachKind, ReachSettings};

/// The section's content, drawn inside its tinted band by `pages::settings::draw`. `accent` is
/// the band's colour, which the subsection headers wear.
pub(crate) fn draw_safety_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    // Step G: the protected setup's always-visible line, first on the page while it is on (10h).
    super::safety_protected::draw_status_line(ui, theme, state);
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
        super::safety_protected::draw_protected_section(ui, theme, state, accent);
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

    draw_chosen_list(ui, theme, state, accent, shown);

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
    // 10j: reports members of a group I created sent me (safety_group_reports.rs).
    super::safety_group_reports::draw_section(ui, theme, state, accent);

    draw_warnings_switch(ui, theme, state, accent);
    draw_blocked_people(ui, theme, state, accent);
    super::safety_protected::draw_protected_section(ui, theme, state, accent);
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
        let mut on = state.settings.warnings_on_messages;
        if widgets::toggle(ui, theme, "Warnings on messages", &mut on) {
            if on {
                state.settings.warnings_on_messages = true;
                state.settings_dirty = true;
            } else {
                // Step G: with the protected setup on, turning them off needs the PIN.
                crate::engine::protected::perform(state, crate::net::protected::ProtectedAction::WarningsOff);
            }
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

/// "People I choose" (10c-ii): each friend (someone we have given a pass, or a mutual follow
/// still owed one) once, with three ticks, Message, Call and Trade. Changing a tick re-issues
/// the pass they hold from us to allow exactly what is ticked (engine/reach.rs `set_tick`).
/// Above the list, one line saying when the ticks count and, directly under it, one saying
/// which rows use them now, built from the person's settings as the server last said them
/// (`shown`), so the ticks never look as though they decide a row set to "Friends".
/// The rows of "People I choose": each person once (net/dm_store.rs `people_to_choose`: everyone
/// holding a pass from us and every mutual follow; someone whose pass my other device withdrew,
/// 10m R3, only while still a mutual follow, 10n N6), sorted by name, with their ticks (the choice,
/// 10n; nothing ticked for one marked R3), and whether they hold no pass carrying the choice yet,
/// shown "(updating their pass)": a pass that could not go out (offline, or no DM key for them
/// yet), a first pass still owed, or one left to my other device (R3). The pass sweep sends it on
/// the next member list (not for R3: a note from that device, or the person's own tick here,
/// frees the row). The ticks stay live meanwhile: a change of mind made offline is kept with its
/// time and is simply what the sweep sends.
pub(crate) fn chosen_rows(state: &GuiState) -> Vec<(String, String, FriendTicks, bool)> {
    let Some(store) = state.dm_store.as_ref() else { return Vec::new() };
    let owed = store.owed_passes();
    let mut friends: Vec<(String, String, FriendTicks, bool)> = store
        .people_to_choose()
        .into_iter()
        .map(|k| {
            // 10n N6: a friend marked "changed on my other device" is drawn with nothing ticked:
            // this device does not know the choice made there, and drawing the old ticks let one
            // click give back what that device took away. A tick here gives exactly what is ticked.
            let marked = store.changed_elsewhere(&k);
            let updating = marked || owed.contains(&k);
            let ticks = if marked { FriendTicks::NONE } else { store.ticks(&k) };
            (k.clone(), crate::engine::dm::dm_display_name(state, &k), ticks, updating)
        })
        .collect();
    friends.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    friends
}

fn draw_chosen_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32, shown: ReachSettings) {
    widgets::subsection_header(
        ui,
        theme,
        accent,
        "People I choose",
        "Everyone you have given a pass, each with what they may do: message you, call you, or send \
         you trade requests. Changing a tick re-issues the pass they hold from you, so the server \
         goes by it at once. Unticking everything does not end a friendship.",
    );
    if state.dm_store.is_none() {
        return;
    }
    let friends = chosen_rows(state);

    let mut toggled: Option<(String, ReachKind, bool)> = None;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        // The two lines above the list (10c-ii): when the ticks count, then which rows use them now.
        widgets::body_hint(ui, theme, crate::net::reach::CHOSEN_TICKS_NOTE);
        widgets::body_hint(ui, theme, &crate::net::reach::chosen_in_use_line(&shown));
        ui.add_space(theme.spacing_xs);
        if friends.is_empty() {
            widgets::body_hint(
                ui,
                theme,
                "No friends on this server yet. People you follow who follow you back appear here, and so does anyone you send a contact request.",
            );
        }
        // A fixed name column, so the three ticks line up down the list; narrower on a narrow
        // window, where a long name is cut short rather than pushing the ticks off the edge.
        let name_w = theme.settings_label_width.min(ui.available_width() * 0.4);
        for (key, name, ticks, updating) in &friends {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(name_w, ui.spacing().interact_size.y),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(name_w);
                        ui.set_max_width(name_w);
                        ui.add(egui::Label::new(RichText::new(name).size(theme.font_size_body).color(theme.text_primary())).truncate());
                    },
                );
                for kind in ReachKind::ALL {
                    let mut tick = ticks.get(kind);
                    let mut changed = widgets::custom_checkbox(ui, theme, &mut tick);
                    // The word beside the box toggles it too, as a checkbox's label does.
                    let word = RichText::new(kind.tick_label()).size(theme.font_size_body).color(theme.text_secondary());
                    if ui.add(egui::Label::new(word).sense(egui::Sense::click())).clicked() {
                        tick = !tick;
                        changed = true;
                    }
                    if changed {
                        toggled = Some((key.clone(), kind, tick));
                    }
                    ui.add_space(theme.spacing_sm);
                }
                if *updating {
                    ui.label(RichText::new("(updating their pass)").size(theme.font_size_small).color(theme.text_muted()));
                }
            });
        }
    });
    if let Some((key, kind, on)) = toggled {
        crate::engine::reach::set_tick(state, &key, kind, on);
    }
}

/// The Requests list: each request's name only, with Accept and Ignore. Shared by Settings >
/// Safety and the chat page's left rail, so the two never drift.
pub(crate) fn draw_requests_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let requests = state.dm_store.as_ref().map(|s| s.requests().to_vec()).unwrap_or_default();
    // Step G: while the protected setup is on, Accept says it needs the PIN (10h).
    let accept = super::safety_protected::accept_label(state);
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
            if widgets::Button::success(&accept).tooltip("Follow them back: you become friends and can message each other.").show(ui, theme) {
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
