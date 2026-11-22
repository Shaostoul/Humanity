//! Server Settings: the server's owner (2026-11-20, docs/design/hide-and-keep.md, "How the owner
//! role is set"). The rules are src/net/owner.rs, the frames and the re-seal job
//! src/engine/owner.rs.
//!
//! - **Claim ownership**, at the top of the Overview tab, for EVERYONE signed in, only while the
//!   server says it has no owner (`owner_state`): paste the claim code from the server's own log
//!   (or data/owner-claim-code.txt on the server machine) and press Claim; the server's answer is
//!   shown beside it.
//! - **Ownership**, in the Moderator section under Held copies, for the OWNER only: the successor
//!   (or none); choose a member (their key card is fetched and checked on this device); ask them to
//!   read four groups of their own fingerprint from their own screen, at the positions shown, and
//!   type what they read (Check these groups; the listed key's fingerprint shows only once they
//!   match, and a mismatch is a warning that refuses the key on this server until the app
//!   restarts); type their name
//!   exactly and a reason; then Hand over ownership, Name as successor, or Remove successor.
//!   While this device seals every held copy again, the page shows how far it has come and offers
//!   Cancel.
//! - On Held copies, for every admin: the successor, whether each item is also sealed to them, and
//!   the owner log (claims, handovers, changes of successor: who, when, and the reason).

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets::hint::Hint;
use crate::gui::{widgets, GuiState};
use crate::net::held::{HeldItem, HeldList};
use crate::net::owner::{CardCheck, Change};
use humanity_core::i18n::{t, tf};

/// The most members the picker lists at once; the search box finds anyone after them.
const PICK_ROWS: usize = 8;

fn when(ms: i64) -> String {
    crate::gui::pages::game_admin::format_ban_date(ms)
}

/// Claim ownership, for everyone signed in, only while this server has no owner. After a claim
/// that worked, the server's words stay in its place.
pub(crate) fn draw_claim(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let o = &state.reports.owner;
    if o.claim_ok && !o.claim_words.is_empty() {
        ui.vertical_centered(|ui| {
            ui.set_max_width(super::SECTION_MAX_WIDTH);
            widgets::alert(ui, theme, widgets::AlertKind::Success, &state.reports.owner.claim_words);
        });
        ui.add_space(theme.spacing_md);
        return;
    }
    if o.has_owner != Some(false) || !state.ws_identified {
        return;
    }
    widgets::tinted_section(ui, theme, t("owner.claim_title"), theme.warning(), super::SECTION_MAX_WIDTH, |ui, theme| {
        widgets::body_hint(ui, theme, t("owner.claim_intro"));
        widgets::body_hint(ui, theme, t("owner.claim_where"));
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.reports.owner.code)
                    .desired_width(280.0)
                    .char_limit(64)
                    .id(egui::Id::new("owner_claim_code"))
                    .hint_text(t("owner.claim_code")),
            )
            .hint("owner.claim_code_hint");
            if widgets::Button::primary(t("owner.claim_button")).hint("owner.claim_button_hint").show(ui, theme) {
                crate::engine::owner::claim(state);
            }
        });
        if !state.reports.owner.claim_words.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&state.reports.owner.claim_words).color(theme.text_secondary()));
        }
    });
    ui.add_space(theme.spacing_lg);
}

/// A change waiting for THIS device's check of the re-wrapped held copies (the owner review's
/// finding 5): who asks and why, Check and accept, and Decline; while the check runs, how far it
/// is; after, whether the server took the answer, in the app's own words. For everyone signed in
/// (the new owner or the successor need not be an admin). Nothing a copy holds is ever drawn.
///
/// Drawn only for a check whose signed action verified, signed by the owner this device knows
/// (engine/owner.rs `on_check`, the sixth review's H1 (c)); its owner's name passes the
/// honest-name rule or shows as a refused name, and no text the relay wrote is drawn in it. No
/// fingerprint is drawn here (H1 (a)): a person's own is shown only in Settings > Account.
pub(crate) fn draw_check(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    crate::engine::owner::poll(state);
    let o = &state.reports.owner;
    if o.check.is_none() && o.check_words.is_empty() {
        return;
    }
    widgets::tinted_section(ui, theme, t("owner.check_title"), theme.warning(), super::SECTION_MAX_WIDTH, |ui, theme| {
        if let Some(c) = state.reports.owner.check.clone() {
            let owner_name = crate::net::owner::shown_name(&c.owner_name);
            let intro = match c.action.as_str() {
                "owner_handover" => tf("owner.check_handover", &[("owner", &owner_name)]),
                "owner_successor" => tf("owner.check_successor", &[("owner", &owner_name)]),
                _ => tf("owner.check_other", &[("owner", &owner_name)]),
            };
            widgets::body_hint(ui, theme, &intro);
            // Where to read your own fingerprint from: Settings > Account, never this prompt (the
            // sixth review's H1 (a), (b)).
            if matches!(c.action.as_str(), "owner_handover" | "owner_successor") {
                widgets::body_hint(ui, theme, t("owner.check_where_fp"));
            }
            widgets::body_hint(ui, theme, t("owner.check_how"));
            ui.add_space(theme.spacing_sm);
            if state.reports.owner.check_job.is_none() {
                ui.horizontal_wrapped(|ui| {
                    if widgets::Button::primary(t("owner.check_accept")).hint("owner.check_accept_hint").show(ui, theme) {
                        crate::engine::owner::start_check(state, c.clone());
                    }
                    if widgets::Button::secondary(t("owner.check_decline")).hint("owner.check_decline_hint").show(ui, theme) {
                        crate::engine::owner::decline_check(state);
                    }
                });
            }
        }
        if !state.reports.owner.check_words.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&state.reports.owner.check_words).color(theme.text_secondary()));
        }
    });
    ui.add_space(theme.spacing_lg);
}

/// Ownership, for the owner only: the successor, the member picker with its card check, and the
/// handover and successor controls; while a change runs, its progress and Cancel.
pub(crate) fn draw_owner(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !crate::engine::owner::i_am_owner(state) {
        return;
    }
    crate::engine::owner::poll(state);
    ui.add_space(theme.spacing_md);
    ui.separator();
    ui.add_space(theme.spacing_sm);
    widgets::subsection_label(ui, theme, t("owner.title"));
    widgets::body_hint(ui, theme, t("owner.intro"));
    let successor = state.reports.held.list.as_ref().and_then(|l| l.successor.clone());
    successor_line(ui, theme, state.reports.held.list.as_ref());
    widgets::body_hint(ui, theme, t("owner.successor_explain"));
    // A mismatch's warning is drawn by the picker, as a warning, beside the member it is about.
    let warned = state.reports.owner.picked.as_ref().is_some_and(|(k, n)| crate::engine::owner::is_refused(state, k) && state.reports.owner.status == crate::engine::owner::mismatch_words(n));
    if !state.reports.owner.status.is_empty() && !warned {
        ui.label(RichText::new(&state.reports.owner.status).color(theme.text_secondary()));
    }
    ui.add_space(theme.spacing_sm);

    if state.reports.owner.pending.is_some() {
        if let Some(p) = state.reports.owner.progress.filter(|p| p.parts > 0) {
            ui.add(egui::ProgressBar::new(p.parts_done as f32 / p.parts as f32).desired_width(360.0).fill(theme.accent()).show_percentage());
        }
        if widgets::Button::secondary(t("owner.cancel")).hint("owner.cancel_hint").show(ui, theme) {
            crate::engine::owner::cancel(state);
        }
        return;
    }

    draw_picker(ui, theme, state);
    ui.add_space(theme.spacing_sm);
    // Each field keeps its name above it, since the words inside go once something is typed.
    ui.label(RichText::new(t("owner.confirm")).size(theme.font_size_small).color(theme.text_secondary()));
    ui.add(
        egui::TextEdit::singleline(&mut state.reports.owner.confirm)
            .desired_width(360.0)
            .char_limit(120)
            .id(egui::Id::new("owner_confirm"))
            .hint_text(t("owner.confirm")),
    )
    .hint("owner.confirm_hint");
    ui.label(RichText::new(t("owner.reason")).size(theme.font_size_small).color(theme.text_secondary()));
    ui.add(
        egui::TextEdit::singleline(&mut state.reports.owner.reason)
            .desired_width(f32::INFINITY)
            .char_limit(500)
            .id(egui::Id::new("owner_reason"))
            .hint_text(t("owner.reason")),
    )
    .hint("owner.reason_hint");
    ui.add_space(theme.spacing_xs);
    widgets::alert(ui, theme, widgets::AlertKind::Warning, t("owner.handover_warning"));
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::danger(t("owner.handover")).hint("owner.handover_hint").show(ui, theme) {
            crate::engine::owner::begin(state, Change::Handover);
        }
        if widgets::Button::secondary(t("owner.name_successor")).hint("owner.name_successor_hint").show(ui, theme) {
            crate::engine::owner::begin(state, Change::Successor);
        }
        if successor.is_some() && widgets::Button::secondary(t("owner.remove_successor")).hint("owner.remove_successor_hint").show(ui, theme) {
            crate::engine::owner::begin(state, Change::SuccessorRemove);
        }
    });
    // Removing a successor is no revocation (the third review's S7): said beside the button.
    if successor.is_some() {
        widgets::body_hint(ui, theme, t("owner.remove_successor_limit"));
    }
}

/// The search box and the members it finds (never me), each by NAME alone (a name an honest relay
/// could not list shows as a refused name: the third review, M2); the chosen one, what checking
/// their key card found, and the four POSITIONS of their fingerprint to ask them for, drawn at
/// random for every key every time. The listed key's fingerprint is not drawn here until the groups
/// typed matched it (the fifth review's H1: the person reads their own groups from their own
/// screen, so nothing here may give them away). A key whose groups did not match this session shows
/// the warning instead of the field.
fn draw_picker(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add(
        egui::TextEdit::singleline(&mut state.reports.owner.search)
            .desired_width(280.0)
            .char_limit(120)
            .id(egui::Id::new("owner_find"))
            .hint_text(t("owner.find")),
    )
    .hint("owner.find_hint");
    let needle = state.reports.owner.search.trim().to_lowercase();
    let members: Vec<(String, String)> = state
        .chat_users
        .iter()
        .filter(|u| u.public_key != state.profile_public_key && !u.public_key.is_empty())
        .filter(|u| needle.is_empty() || u.name.to_lowercase().contains(&needle))
        .take(PICK_ROWS)
        .map(|u| (u.public_key.clone(), u.name.clone()))
        .collect();
    if members.is_empty() {
        widgets::body_hint(ui, theme, t("owner.no_members"));
    }
    let picked = state.reports.owner.picked.as_ref().map(|(k, _)| k.clone());
    ui.horizontal_wrapped(|ui| {
        for (key, name) in &members {
            let shown = crate::net::owner::shown_name(name);
            if ui.selectable_label(picked.as_deref() == Some(key.as_str()), RichText::new(shown)).hint("owner.pick_hint").clicked() {
                crate::engine::owner::pick(state, key, name);
            }
        }
    });
    let Some((key, name)) = state.reports.owner.picked.clone() else { return };
    let name = crate::net::owner::shown_name(&name);
    ui.label(RichText::new(tf("owner.chosen", &[("name", &name)])).color(theme.text_primary()).strong());
    if crate::engine::owner::is_refused(state, &key) {
        // The groups typed for this key did not match: the warning, and no field to try again.
        widgets::alert(ui, theme, widgets::AlertKind::Warning, &crate::engine::owner::mismatch_words(&name));
    } else {
        // Four positions asked every time, for every key: no friend, pin or earlier confirmation is
        // an exemption (the fourth review's findings 1 and 5). Only the POSITIONS: the person reads
        // their own groups from their own screen (the fifth review's H1).
        let [a, b, c, d] = state.reports.owner.asked.map(humanity_protocol::key_card::asked_words).unwrap_or_default();
        let at: [(&str, &dyn std::fmt::Display); 5] = [("name", &name), ("a", &a), ("b", &b), ("c", &c), ("d", &d)];
        ui.label(RichText::new(tf("owner.fp_compare", &at)).size(theme.font_size_small).color(theme.warning()));
        ui.label(RichText::new(tf("owner.fp_confirm", &at)).size(theme.font_size_small).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            let typed = ui
                .add(
                    egui::TextEdit::singleline(&mut state.reports.owner.fp_confirm)
                        .desired_width(220.0)
                        .char_limit(24)
                        .id(egui::Id::new("owner_fp_confirm"))
                        .hint_text(tf("owner.fp_confirm", &at)),
                )
                .hint("owner.fp_confirm_hint");
            if typed.changed() {
                state.reports.owner.fp_ok = None;
            }
            if widgets::Button::secondary(t("owner.fp_check")).hint("owner.fp_check_hint").show(ui, theme) {
                crate::engine::owner::check_groups_now(state);
            }
        });
        // The listed key's whole fingerprint, only now the groups typed matched it.
        if state.reports.owner.fp_ok.as_deref() == Some(key.as_str()) {
            let fp = humanity_protocol::key_card::fingerprint(&key);
            ui.label(RichText::new(tf("owner.fp_matched", &[("name", &name), ("fp", &fp)])).size(theme.font_size_small).color(theme.success()));
        }
    }
    let (words, color) = match state.reports.owner.card.as_ref() {
        None => (tf("owner.card_checking", &[("name", &name)]), theme.text_muted()),
        Some(CardCheck::Good { at, .. }) => (tf("owner.card_ok", &[("name", &name), ("when", &when(*at as i64))]), theme.success()),
        Some(other) => (crate::net::owner::card_words(other, &name).unwrap_or_default(), theme.warning()),
    };
    ui.label(RichText::new(words).size(theme.font_size_small).color(color));
}

/// "Successor: <name>", or that none is named (Held copies and Ownership).
pub(crate) fn successor_line(ui: &mut egui::Ui, theme: &Theme, list: Option<&HeldList>) {
    let words = match list.and_then(|l| l.successor.as_ref()) {
        Some(s) => tf("owner.successor_line", &[("name", &s.name)]),
        None => t("owner.no_successor").to_string(),
    };
    ui.label(RichText::new(words).color(theme.text_secondary()));
}

/// Whether an item is also sealed to the successor, in words; nothing when none is named.
pub(crate) fn item_successor_line(ui: &mut egui::Ui, theme: &Theme, list: &HeldList, item: &HeldItem) {
    if list.successor.is_none() {
        return;
    }
    let words = if item.sealed_to_successor { t("owner.sealed_to_successor") } else { t("owner.not_successor_sealed") };
    ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary()));
}

/// The owner log: each claim, handover and change of successor, who, when and why.
pub(crate) fn draw_log(ui: &mut egui::Ui, theme: &Theme, list: &HeldList) {
    widgets::subsection_label(ui, theme, t("owner.log_title"));
    if list.owner_log.is_empty() {
        widgets::body_hint(ui, theme, t("owner.log_empty"));
    }
    for l in &list.owner_log {
        let key = match l.kind.as_str() {
            "claim" => "owner.log_claim",
            "owner_handover" => "owner.log_owner_handover",
            "owner_successor" => "owner.log_owner_successor",
            "owner_successor_remove" => "owner.log_owner_successor_remove",
            "owner_change_accept" => "owner.log_owner_change_accept",
            _ => continue,
        };
        let line = tf(key, &[("when", &when(l.at)), ("old", &l.old_name), ("new", &l.new_name), ("reason", &l.reason)]);
        ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary()));
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
