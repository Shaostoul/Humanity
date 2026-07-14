//! Themed dialog (Window) wrapper — replaces the dozens of bare
//! `egui::Window::new(...)` calls scattered across pages with a consistent
//! frame, padding, title styling, and close behaviour.
//!
//! Examples replaced by this widget:
//! - `chat.rs` had ~6 custom Window declarations with copy-pasted Frame styling
//! - `main_menu.rs` had 2
//! - `settings.rs` modal sections used inline Window
//!
//! Two flavours:
//! - `dialog(ctx, theme, id, title, open, content)` — modal-style centered dialog
//! - `dialog_anchored(ctx, theme, id, title, open, anchor, content)` — pin to a screen edge

use egui::{Align2, Color32, Frame, RichText, Rounding, Stroke, Ui, Vec2};
// Vec2 used by `dialog_anchored` callers via the `offset` parameter.
use super::super::theme::Theme;

/// Render a centered themed dialog. Returns true if the dialog was shown.
///
/// Closing the dialog (X button) sets `*open = false`. Content callback runs
/// inside a themed Frame so child widgets inherit the right padding/colors.
pub fn dialog(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    open: &mut bool,
    content: impl FnOnce(&mut Ui),
) -> bool {
    dialog_inner(ctx, theme, id, title, open, Align2::CENTER_CENTER, Vec2::ZERO, content)
}

/// Render a themed dialog anchored to a specific position on the screen.
pub fn dialog_anchored(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    open: &mut bool,
    anchor: Align2,
    offset: Vec2,
    content: impl FnOnce(&mut Ui),
) -> bool {
    dialog_inner(ctx, theme, id, title, open, anchor, offset, content)
}

fn dialog_inner(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    open: &mut bool,
    anchor: Align2,
    offset: Vec2,
    content: impl FnOnce(&mut Ui),
) -> bool {
    let mut shown = false;
    let mut local_open = *open;

    // Semi-transparent backdrop — paints a dimmed overlay over the rest of
    // the UI and catches clicks behind the modal. Clicking the backdrop
    // CLOSES the modal (standard click-outside-to-dismiss UX). That dismissal
    // is also what avoids egui's z-order bug where interacting with a same-
    // layer Area can shove the Window behind it (operator-reported v0.297).
    if local_open {
        let area_resp = egui::Area::new(egui::Id::new(format!("{id}-backdrop")))
            .order(egui::Order::Middle)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(140));
                ui.allocate_rect(screen, egui::Sense::click())
            });
        if area_resp.inner.clicked() {
            local_open = false;
        }
    }

    let win_resp = egui::Window::new(RichText::new(title).color(theme.text_primary()).strong())
        .id(egui::Id::new(id))
        .open(&mut local_open)
        .anchor(anchor, offset)
        .resizable(false)
        .collapsible(false)
        .frame(
            Frame::none()
                .fill(theme.bg_card())
                .stroke(Stroke::new(1.0, theme.border()))
                .rounding(Rounding::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(64),
                }),
        )
        .show(ctx, |ui| {
            shown = true;
            content(ui);
        });

    // Force the modal window to the TOP of its order every frame. The backdrop
    // Area and this Window both live in `Order::Middle`; without this the
    // persisted backdrop area can end up above the window and (a) paint its tint
    // OVER the modal and (b) swallow clicks meant for the modal's own buttons,
    // so nothing inside the modal is clickable. `move_to_top` makes the window
    // paint + receive input above the backdrop, while the backdrop (now strictly
    // below the window) still catches click-outside-to-close in the surrounding
    // area. Long-standing operator bug: "the tinted background claims the screen;
    // the modal renders behind it where I can't click anything." (v0.849)
    if let Some(ref wr) = win_resp {
        ctx.move_to_top(wr.response.layer_id);
    }

    *open = local_open;
    shown
}
