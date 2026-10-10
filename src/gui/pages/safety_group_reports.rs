//! Settings > Safety > Reports about your groups (section 10j of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10), and the count on a group in the chat's
//! group list that leads here.
//!
//! What the group's creator sees: each report a member of a group they created sent them, with
//! the person reported and the reason, the group (as the creator's own list names it), who
//! reported and when, their note, and each message the report named with its badge from the check
//! against the creator's own copy ("Found in your copy of the group, signed by <name>", or "Not
//! found in your copy"). Then the three actions: Remove them from the group (confirmed with a
//! three-second hold; a new group key for everyone else goes first, engine/group_report.rs),
//! Block them, Dismiss. The section shows when there is a report or when I created a group, so a
//! creator knows where reports would land. The web client's words, word for word
//! (net/group_report.rs).
//!
//! The reports live in this server's encrypted DM store until dismissed and are never sent
//! anywhere, so nothing here is a setting.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::net::group_report::{self as gr, KeptReport};

/// What a click in the section asks for, applied once the section is drawn.
enum Act {
    AskRemove(String),
    Remove(String),
    Cancel,
    Block(String),
    Dismiss(String),
}

/// The section, inside Settings > Safety's band (`accent` is the band's colour).
pub(crate) fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    let reports: Vec<KeptReport> = state.dm_store.as_ref().map(|s| s.group_reports().into_iter().cloned().collect()).unwrap_or_default();
    if reports.is_empty() && !state.p2p_groups.iter().any(|g| g.is_creator) {
        return;
    }
    // The reasons' labels come from the same file the Report dialog reads.
    crate::engine::report::ensure_reasons(state);
    widgets::subsection_header(ui, theme, accent, gr::TITLE, gr::HELP);
    let mut act: Option<Act> = None;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        if reports.is_empty() {
            widgets::body_hint(ui, theme, gr::NONE);
        }
        for (i, r) in reports.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            if let Some(a) = draw_report(ui, theme, state, r) {
                act = Some(a);
            }
        }
    });
    match act {
        Some(Act::AskRemove(id)) => state.reports.group.confirm_remove = Some(id),
        Some(Act::Remove(id)) => crate::engine::group_report::remove(state, &id),
        Some(Act::Cancel) => state.reports.group.confirm_remove = None,
        Some(Act::Block(id)) => crate::engine::group_report::block_them(state, &id),
        Some(Act::Dismiss(id)) => {
            crate::engine::group_report::dismiss(state, &id);
        }
        None => {}
    }
}

/// One report: who and why, where and from whom, the note, the messages with their badges, and
/// the actions.
fn draw_report(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, r: &KeptReport) -> Option<Act> {
    let name = |key: &str| crate::engine::dm::dm_display_name(state, key);
    let target = name(&r.target);
    let reason = state.reports.reasons.iter().find(|x| x.id == r.reason).map(|x| x.label.clone()).unwrap_or_else(|| r.reason.clone());
    ui.label(RichText::new(format!("{target}: {reason}")).size(theme.font_size_body).color(theme.text_primary()).strong());
    let when = crate::gui::pages::game_admin::format_ban_date(r.ts as i64);
    widgets::body_hint(ui, theme, &format!("In {} · reported by {} · {when}", r.group_name, name(&r.from)));
    if !r.note.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Their note:").color(theme.text_muted()));
            ui.label(RichText::new(&r.note).color(theme.text_primary()));
        });
    }
    if r.items.is_empty() {
        widgets::body_hint(ui, theme, gr::NO_ITEMS);
    }
    for it in &r.items {
        let signer = if it.signer.is_empty() { &it.from } else { &it.signer };
        let (found, badge) = gr::badge(it, &name(signer));
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            let colour = if found { theme.success() } else { theme.warning() };
            ui.label(RichText::new(badge).size(theme.font_size_small).color(colour).strong());
            let at = crate::gui::pages::game_admin::format_ban_date(it.ts as i64);
            ui.label(RichText::new(at).size(theme.font_size_small).color(theme.text_muted()));
        });
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(crate::engine::dm::dm_preview_text(&it.text)).color(theme.text_secondary()));
        });
    }
    ui.add_space(theme.spacing_sm);

    let mut act = None;
    let removing = state.reports.group.removing.iter().any(|(id, _)| *id == r.id);
    if state.reports.group.confirm_remove.as_deref() == Some(r.id.as_str()) && !r.removed {
        // The web's hold confirm: the sentence, a three-second hold, and Cancel.
        widgets::alert(ui, theme, widgets::AlertKind::Warning, &gr::remove_confirm(&target, &r.group_name));
        let hold_id = egui::Id::new(("group_report_remove_hold", r.id.as_str()));
        if crate::gui::pages::chat::hold_to_confirm_button(ui, theme, hold_id, "Hold to confirm", "Hold to confirm", 3.0, "Press and hold for 3 seconds.") {
            act = Some(Act::Remove(r.id.clone()));
        }
        if widgets::Button::secondary("Cancel").show(ui, theme) {
            act = Some(Act::Cancel);
        }
        ui.add_space(theme.spacing_xs);
    }
    ui.horizontal_wrapped(|ui| {
        if r.removed {
            ui.label(RichText::new(gr::REMOVED).size(theme.font_size_small).color(theme.text_muted()));
        } else if crate::engine::group_report::in_group(state, &r.group_id, &r.target) {
            let tip = gr::remove_confirm(&target, &r.group_name);
            if widgets::Button::danger(gr::ACTION_REMOVE).disabled(removing).tooltip(&tip).show(ui, theme) {
                act = Some(Act::AskRemove(r.id.clone()));
            }
        } else {
            ui.label(RichText::new(gr::NOT_IN_GROUP).size(theme.font_size_small).color(theme.text_muted()));
        }
        if crate::engine::block::is_blocked(state, &r.target) {
            ui.label(RichText::new(gr::BLOCKED).size(theme.font_size_small).color(theme.text_muted()));
        } else if widgets::Button::danger(gr::ACTION_BLOCK)
            .tooltip("Hide everything from them from now on, on every server. They are not told.")
            .show(ui, theme)
        {
            act = Some(Act::Block(r.id.clone()));
        }
        if widgets::Button::secondary(gr::ACTION_DISMISS).tooltip("Remove this report from this device. Nobody is told.").show(ui, theme) {
            act = Some(Act::Dismiss(r.id.clone()));
        }
    });
    act
}

/// The count on a group I created, in the chat's group list (10j): a small number just after the
/// group's name (`name` is where the name was painted), saying on hover what it counts. A click
/// opens Settings > Safety, where the reports are. Nothing is drawn for a group with none.
pub(crate) fn group_count(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, name: egui::Rect, group_id: &str) {
    let n = crate::engine::group_report::count_for(state, group_id);
    if n == 0 {
        return;
    }
    let galley = ui.painter().layout_no_wrap(n.to_string(), egui::FontId::proportional(theme.font_size_small), theme.danger());
    let size = galley.size() + egui::vec2(8.0, 2.0);
    let rect = egui::Rect::from_min_size(egui::pos2(name.right() + theme.spacing_xs, name.center().y - size.y / 2.0), size);
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(3), egui::Stroke::new(1.0, theme.danger()), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.danger());
    // Interacted after the row, so a click here opens Safety rather than the group.
    let resp = ui.interact(rect, egui::Id::new(("group_report_count", group_id)), egui::Sense::click()).on_hover_text(gr::count_title(n));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        state.settings.category = crate::gui::SettingsCategory::Safety;
        state.settings.scroll_to_section = Some(crate::gui::SettingsCategory::Safety);
        state.push_nav_to(crate::gui::GuiPage::Settings);
    }
}
