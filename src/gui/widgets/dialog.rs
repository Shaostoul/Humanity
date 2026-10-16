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
    dialog_inner(ctx, theme, id, title, open, Align2::CENTER_CENTER, Vec2::ZERO, false, content)
}

/// A centered themed dialog whose whole body scrolls when it is taller than the screen, so
/// nothing at its end (a Send button) is ever cut off. Use it for a dialog whose height depends
/// on what is chosen in it. Do not also wrap the body in a ScrollArea: a window does not grow to
/// fit one, and whatever follows it is clipped (the report dialog, 2026-10-10).
pub fn dialog_scrolling(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    open: &mut bool,
    content: impl FnOnce(&mut Ui),
) -> bool {
    dialog_inner(ctx, theme, id, title, open, Align2::CENTER_CENTER, Vec2::ZERO, true, content)
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
    dialog_inner(ctx, theme, id, title, open, anchor, offset, false, content)
}

#[allow(clippy::too_many_arguments)]
fn dialog_inner(
    ctx: &egui::Context,
    theme: &Theme,
    id: &str,
    title: &str,
    open: &mut bool,
    anchor: Align2,
    offset: Vec2,
    vscroll: bool,
    content: impl FnOnce(&mut Ui),
) -> bool {
    let mut shown = false;
    let mut local_open = *open;
    let mut backdrop_layer = None;

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
        backdrop_layer = Some(area_resp.response.layer_id);
        if area_resp.inner.clicked() {
            local_open = false;
        }
    }

    let mut window = egui::Window::new(RichText::new(title).color(theme.text_primary()).strong());
    // A window with its own scroll area does not size itself to its content (it keeps egui's
    // default height and scrolls inside a short box), and once sized it keeps that size. So it is
    // held to exactly the height its content took last frame, up to the screen less a margin;
    // past that it scrolls. The first frame, before anything is measured, is only capped.
    let measured_key = egui::Id::new(id).with("content_height");
    if vscroll {
        let tallest = (ctx.screen_rect().height() - 80.0).max(200.0);
        let last: Option<f32> = ctx.data(|d| d.get_temp(measured_key));
        window = match last {
            Some(h) => {
                let want = h.min(tallest);
                window.min_height(want).max_height(want)
            }
            None => window.max_height(tallest),
        };
    }
    let win_resp = window
        .id(egui::Id::new(id))
        .open(&mut local_open)
        .anchor(anchor, offset)
        .resizable(false)
        .collapsible(false)
        // The window keeps inside the screen (egui constrains it), so with this its body scrolls
        // instead of running off the bottom.
        .vscroll(vscroll)
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
            let top = ui.min_rect().top();
            content(ui);
            if vscroll {
                // Inside the window's scroll area this is the content's whole height, clipped or not.
                let height = ui.min_rect().bottom() - top;
                ctx.data_mut(|d| d.insert_temp(measured_key, height));
            }
        });

    // Keep the window directly ABOVE its own backdrop, every frame, and let the backdrop decide
    // where the pair sits. Both live in `Order::Middle`, and egui raises a layer when it first
    // appears or is pressed, so the dialog opened last is in front, its backdrop dimming any
    // dialog under it. Without this the backdrop can end up above its own window, tinting it and
    // swallowing its clicks: "the tinted background claims the screen; the modal renders behind
    // it where I can't click anything" (v0.849). The v0.849 fix raised the window to the top on
    // EVERY frame instead, which made every open dialog ask for the top at once; egui then keeps
    // their earlier order, and a window it has seen before (closed and opened again) keeps its old
    // place. So the Media item's Edit window, opened a second time from the item's page, stayed
    // behind the page for good (operator, 2026-10-15: "Like our layering order is reversed").
    if let (Some(wr), Some(backdrop)) = (win_resp.as_ref(), backdrop_layer) {
        ctx.set_sublayer(backdrop, wr.response.layer_id);
    }

    *open = local_open;
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scrolling dialog's window rect after three headless frames (no GPU), on a screen
    /// `screen_h` tall, with a body exactly `body_h` tall and a marker row after it.
    fn window_rect(screen_h: f32, body_h: f32) -> egui::Rect {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut open = true;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, screen_h))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                dialog_scrolling(ctx, &theme, "dialog_test", "Test", &mut open, |ui| {
                    ui.allocate_space(egui::vec2(300.0, body_h));
                    let _ = ui.button("Send");
                });
            });
        }
        ctx.memory(|m| m.area_rect(egui::Id::new("dialog_test"))).expect("the window was drawn")
    }

    /// The report dialog, 2026-10-10: a window with its own scroll area kept egui's short
    /// default height, so everything past it (Send included) was cut off or scrolled out of a
    /// small box. The scrolling dialog fits its content when the screen allows, and stays on
    /// the screen (and scrolls) when it does not.
    ///
    /// Seen red 2026-10-10 with the measured height left out (only `max_height(tallest)`): "a
    /// 700 px body fits on a 1000 px screen" failed with a window about 470 px tall.
    #[test]
    fn a_scrolling_dialog_fits_its_content_and_stays_on_screen() {
        let fits = window_rect(1000.0, 700.0);
        assert!(fits.height() >= 700.0, "a 700 px body fits on a 1000 px screen: window {:.0} px", fits.height());
        assert!(fits.height() <= 1000.0, "and stays on the screen: window {:.0} px", fits.height());

        let tall = window_rect(1000.0, 2000.0);
        assert!(tall.height() <= 1000.0 && tall.min.y >= 0.0, "a 2000 px body stays on a 1000 px screen: {tall:?}");
        assert!(tall.height() >= 800.0, "using most of it before scrolling: window {:.0} px", tall.height());
    }

    /// One headless frame of two dialogs, "older" (large) and "newer" (small, drawn after it, the
    /// way a dialog opened from inside another is), each while its flag is set; `events` are the
    /// frame's input.
    fn two_dialogs(ctx: &egui::Context, theme: &Theme, older: &mut bool, newer: &mut bool, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            if *older {
                dialog(ctx, theme, "older_dialog", "Older", older, |ui| {
                    ui.allocate_space(egui::vec2(500.0, 400.0));
                });
            }
            if *newer {
                dialog(ctx, theme, "newer_dialog", "Newer", newer, |ui| {
                    ui.allocate_space(egui::vec2(200.0, 100.0));
                });
            }
        });
    }

    fn press_and_release(at: egui::Pos2) -> [Vec<egui::Event>; 2] {
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() };
        [vec![egui::Event::PointerMoved(at), button(true)], vec![button(false)]]
    }

    fn top_at_centre(ctx: &egui::Context) -> Option<egui::Id> {
        ctx.layer_id_at(egui::pos2(500.0, 400.0)).map(|l| l.id)
    }

    /// A DIALOG OPENED FROM ANOTHER IS IN FRONT OF IT, EVERY TIME (operator, 2026-10-15: the Media
    /// item's Edit window opened behind the item's page, "Like our layering order is reversed").
    /// Every dialog's window was raised on every frame, so egui kept their earlier order, and a
    /// window it had seen before kept its old place: the second time, the newer one stayed behind.
    ///
    /// Seen red 2026-10-15 with the every-frame `move_to_top` in place of `set_sublayer`: "opened
    /// again, the newer dialog is in front" found the older dialog's window.
    #[test]
    fn a_dialog_opened_from_another_is_in_front_of_it_every_time() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let (mut older, mut newer) = (true, false);
        for _ in 0..3 {
            two_dialogs(&ctx, &theme, &mut older, &mut newer, vec![]);
        }
        for words in ["the newer dialog is in front", "opened again, the newer dialog is in front"] {
            newer = true;
            for _ in 0..3 {
                two_dialogs(&ctx, &theme, &mut older, &mut newer, vec![]);
            }
            assert_eq!(top_at_centre(&ctx), Some(egui::Id::new("newer_dialog")), "{words}");
            // Closed the way the Edit window's Cancel closes it: its flag goes false.
            newer = false;
            for _ in 0..3 {
                two_dialogs(&ctx, &theme, &mut older, &mut newer, vec![]);
            }
            assert_eq!(top_at_centre(&ctx), Some(egui::Id::new("older_dialog")), "with the newer one closed, the older is in front");
        }
    }

    /// A DIALOG CLOSED BY ITS DIMMED BACKDROP OPENS ABOVE IT AGAIN (v0.849: "the tinted background
    /// claims the screen; the modal renders behind it where I can't click anything"). The press on
    /// the backdrop raises it, so without the window pinned above it the next opening is behind it.
    ///
    /// Seen red 2026-10-15 with the pin taken out (neither `move_to_top` nor `set_sublayer`):
    /// "opened again, the window is above its backdrop" found the backdrop's layer.
    #[test]
    fn a_dialog_closed_by_its_backdrop_opens_above_it_again() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let (mut older, mut newer) = (true, false);
        for _ in 0..3 {
            two_dialogs(&ctx, &theme, &mut older, &mut newer, vec![]);
        }
        for events in press_and_release(egui::pos2(6.0, 6.0)) {
            two_dialogs(&ctx, &theme, &mut older, &mut newer, events);
        }
        assert!(!older, "a click on the dimmed screen closes the dialog");
        older = true;
        for _ in 0..3 {
            two_dialogs(&ctx, &theme, &mut older, &mut newer, vec![]);
        }
        assert_eq!(top_at_centre(&ctx), Some(egui::Id::new("older_dialog")), "opened again, the window is above its backdrop");
    }
}
