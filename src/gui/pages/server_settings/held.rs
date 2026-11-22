//! Server Settings > Moderator > Held copies (2026-11-20, docs/design/hide-and-keep.md rung 5), for
//! admins: the posts an admin hid and kept from a report, each sealed so only the server's owner
//! can open it. Shown: how many are held, each one's dates (held, deleted, legal hold), its size
//! and how often it was exported, and the signed action log. Never what an item holds, never its
//! package id, and no button that shows an item, for anyone, the owner included.
//!
//! The OWNER alone gets the actions, each signed with its reason (src/engine/held.rs): Legal hold
//! (days from today, at most 366; 0 lifts it), Destroy now (asks first and says it cannot be
//! undone), Export (a folder chooser that says what it is for, then this device opens the sealed
//! parts straight into one .zip file there, never drawn) and Restore (asks first; this device opens
//! each file and sends it back, and the server checks every byte before putting the post back).
//!
//! When the server cannot keep a copy (no owner, or no checked key card for the owner) the page
//! says so in the relay's own words, what works instead, and where to claim the server (Claim
//! ownership, server_settings/owner.rs). Every admin also sees the successor the owner named,
//! whether each item is sealed to them too, and the owner log (server_settings/owner.rs).

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::widgets::hint::Hint;
use crate::gui::{widgets, GuiState};
use crate::net::held::{Confirm, HeldItem, HOLD_MAX_DAYS};
use humanity_core::i18n::{t, tf, tn};

/// The egui memory slot the open Export folder chooser is kept in, with its item.
const PICKER_SLOT: &str = "held_export_picker";

fn when(ms: i64) -> String {
    crate::gui::pages::game_admin::format_ban_date(ms)
}

/// Why this server cannot keep a sealed copy, in the relay's own words, then what works instead
/// and what to do today. Shown on Held copies and beside a report's decisions.
pub(crate) fn draw_refusal(ui: &mut egui::Ui, theme: &Theme, why: &str) {
    widgets::alert(ui, theme, widgets::AlertKind::Warning, t("held.cannot_keep_title"));
    if !why.is_empty() {
        ui.label(RichText::new(why).color(theme.text_secondary()));
    }
    widgets::body_hint(ui, theme, t("held.instead"));
    widgets::body_hint(ui, theme, t("held.no_owner_today"));
}

/// The section, inside the Moderator band, for admins.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, t("held.title"));
    widgets::body_hint(ui, theme, t("held.intro"));
    ui.add_space(theme.spacing_xs);
    if !state.reports.held.requested {
        crate::engine::held::request_list(state);
    }
    crate::engine::held::poll(state);

    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary(t("held.refresh")).hint("held.refresh_hint").show(ui, theme) {
            crate::engine::held::request_list(state);
        }
        if !state.reports.held.status.is_empty() {
            ui.label(RichText::new(&state.reports.held.status).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    ui.add_space(theme.spacing_sm);

    let Some(list) = state.reports.held.list.clone() else {
        widgets::body_hint(ui, theme, t("held.asking"));
        return;
    };
    if !list.can_keep {
        draw_refusal(ui, theme, &list.cannot_keep_why);
        ui.add_space(theme.spacing_sm);
    }
    ui.label(RichText::new(tn("held.count", list.items.len() as i64, &[])).color(theme.text_primary()).strong());
    let c = list.counts;
    ui.label(
        RichText::new(tf("held.counts_line", &[("held", &c.held), ("exported", &c.exported), ("holds", &c.legal_holds), ("destroyed", &c.destroyed), ("restored", &c.restored)]))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    // Hides whose copy is not sealed yet, said in words (the fourth review's finding 5a).
    if c.waiting > 0 {
        ui.label(RichText::new(tf("held.waiting_line", &[("n", &c.waiting)])).size(theme.font_size_small).color(theme.warning()));
    }
    if c.unidentified > 0 {
        ui.label(RichText::new(tf("held.unidentified_line", &[("n", &c.unidentified)])).size(theme.font_size_small).color(theme.danger()));
    }
    super::owner::successor_line(ui, theme, Some(&list));
    if !list.i_am_owner {
        widgets::body_hint(ui, theme, t("held.owner_only"));
    }
    if list.items.is_empty() {
        widgets::body_hint(ui, theme, t("held.none"));
    }
    for item in &list.items {
        widgets::card(ui, theme, |ui| {
            ui.set_min_width(ui.available_width());
            draw_item(ui, theme, state, item, &list);
        });
        ui.add_space(theme.spacing_sm);
    }

    widgets::subsection_label(ui, theme, t("held.log_title"));
    if list.actions.is_empty() {
        widgets::body_hint(ui, theme, t("held.log_empty"));
    }
    for a in &list.actions {
        let action = humanity_core::i18n::t_dyn(&format!("held.action_{}", a.action));
        ui.label(
            RichText::new(tf("held.log_line", &[("when", &when(a.signed_at)), ("name", &a.signer_name), ("action", &action), ("target", &a.target), ("reason", &a.reason)]))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    }
    super::owner::draw_log(ui, theme, &list); // claims, handovers and changes of successor

    draw_picker(ui.ctx(), theme, state, &list.items);
}

/// One held item: its numbers and dates, and for the owner the actions.
fn draw_item(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &HeldItem, list: &crate::net::held::HeldList) {
    let owner = list.i_am_owner;
    ui.label(RichText::new(tf("held.item_line", &[("id", &item.id), ("report", &item.report_id)])).color(theme.text_primary()).strong());
    let line = |ui: &mut egui::Ui, words: String| {
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary()));
    };
    // Which post it is: one report can hold several (the fourth review's finding 8).
    line(ui, tf("held.post_line", &[("when", &when(item.stamp)), ("name", &item.author_name)]));
    line(ui, tf("held.held_line", &[("when", &when(item.held_at)), ("name", &item.held_by_name)]));
    line(ui, tf("held.expires_line", &[("when", &when(item.deleted_at.max(item.delete_after)))]));
    line(ui, if item.legal_hold_until > 0 { tf("held.hold_line", &[("when", &when(item.legal_hold_until))]) } else { t("held.no_hold").to_string() });
    let size = crate::gui::widgets::file_browser::human_size(item.sealed_bytes.max(0) as u64);
    line(ui, tf("held.size_line", &[("files", &item.file_count), ("size", &size), ("exports", &item.exported_count)]));
    if !item.sealed_to_owner {
        line(ui, t("held.not_owner_sealed").to_string());
    }
    super::owner::item_successor_line(ui, theme, list, item);
    if !owner {
        return;
    }
    ui.add_space(theme.spacing_xs);
    let busy = state.reports.held.pending.is_some();
    let reason = state.reports.held.reasons.entry(item.id).or_default();
    ui.add(
        egui::TextEdit::singleline(reason)
            .desired_width(f32::INFINITY)
            .char_limit(500)
            .id(egui::Id::new(("held_reason", item.id)))
            .hint_text(t("held.reason")),
    )
    .hint("held.reason_hint");
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(t("held.hold_days")).color(theme.text_secondary()));
        let days = state.reports.held.days.entry(item.id).or_insert(90);
        ui.add(egui::DragValue::new(days).range(0..=HOLD_MAX_DAYS)).hint("held.hold_days_hint");
        let days = *days;
        if widgets::Button::secondary(t("held.hold_set")).hint("held.hold_set_hint").disabled(busy).show(ui, theme) {
            crate::engine::held::legal_hold(state, item.id, days);
        }
        if item.legal_hold_until > 0 && widgets::Button::secondary(t("held.hold_lift")).hint("held.hold_lift_hint").disabled(busy).show(ui, theme) {
            crate::engine::held::legal_hold(state, item.id, 0);
        }
        if item.sealed_to_owner {
            if widgets::Button::secondary(t("held.export")).hint("held.export_hint").disabled(busy).show(ui, theme) {
                if state.reports.held.reasons.get(&item.id).is_none_or(|r| r.trim().is_empty()) {
                    state.reports.held.status = t("held.reason_needed").to_string(); // asked before the chooser, not after
                } else {
                    let picker = FilePickerState::new_dir_picker(None).with_pick_verb(t("held.export_pick_verb"));
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(PICKER_SLOT), (item.id, picker)));
                }
            }
            if widgets::Button::secondary(t("held.restore")).hint("held.restore_hint").disabled(busy).show(ui, theme) {
                crate::engine::held::ask(state, item.id, Confirm::Restore);
            }
        }
        if widgets::Button::danger(t("held.destroy")).hint("held.destroy_hint").disabled(busy).show(ui, theme) {
            crate::engine::held::ask(state, item.id, Confirm::Destroy);
        }
    });
    let Some((id, what)) = state.reports.held.confirm.filter(|(id, _)| *id == item.id) else { return };
    ui.add_space(theme.spacing_xs);
    let words = match what {
        Confirm::Destroy => tf("held.destroy_confirm", &[("id", &id)]),
        Confirm::Restore => tf("held.restore_confirm", &[("id", &id)]),
    };
    widgets::alert(ui, theme, if what == Confirm::Destroy { widgets::AlertKind::Error } else { widgets::AlertKind::Warning }, &words);
    ui.horizontal(|ui| {
        // Each button names its hint key where it is made, so the hint ratchet (tests/i18n_lint.rs)
        // sees both.
        let button = match what {
            Confirm::Destroy => widgets::Button::danger(t("held.destroy_yes")).hint("held.destroy_yes_hint"),
            Confirm::Restore => widgets::Button::primary(t("held.restore_yes")).hint("held.restore_yes_hint"),
        };
        if button.show(ui, theme) {
            match what {
                Confirm::Destroy => crate::engine::held::destroy(state, id),
                Confirm::Restore => crate::engine::held::restore(state, item),
            }
        }
        if widgets::Button::secondary(t("held.cancel")).hint("held.cancel_hint").show(ui, theme) {
            state.reports.held.confirm = None;
        }
    });
}

/// The Export folder chooser, while it is open: its title says what the folder is for.
fn draw_picker(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, items: &[HeldItem]) {
    let slot = egui::Id::new(PICKER_SLOT);
    let Some((id, mut picker)) = ctx.data_mut(|d| d.get_temp::<(i64, FilePickerState)>(slot)) else { return };
    let title = tf("held.export_picker_title", &[("id", &id)]);
    match file_picker_modal(ctx, theme, &mut picker, &title) {
        FilePickerResult::Open => ctx.data_mut(|d| d.insert_temp(slot, (id, picker))),
        FilePickerResult::Cancelled => ctx.data_mut(|d| d.remove::<(i64, FilePickerState)>(slot)),
        FilePickerResult::Picked(paths) => {
            ctx.data_mut(|d| d.remove::<(i64, FilePickerState)>(slot));
            if let (Some(folder), Some(item)) = (paths.first(), items.iter().find(|i| i.id == id)) {
                crate::engine::held::export(state, item, folder);
            }
        }
    }
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
