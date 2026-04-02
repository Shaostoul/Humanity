//! Modal dialog overlay.
//! Used by: confirmations, forms, passphrase entry, item details.

use egui::{Align2, Color32, RichText, Rounding, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// Action returned when the user interacts with a modal button.
#[derive(Debug, Clone, PartialEq)]
pub enum ModalAction {
    Confirm,
    Cancel,
    Custom(String),
}

/// Render a modal dialog as a centered egui::Window overlay.
///
/// `open` is set to `false` when the dialog is dismissed (Cancel, close, or Escape).
/// `add_content` draws arbitrary UI into the modal body.
/// `actions` lists the buttons at the bottom; each is `(label, action)`.
///
/// Returns the `ModalAction` if a button was clicked this frame.
pub fn modal_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    title: &str,
    open: &mut bool,
    add_content: impl FnOnce(&mut Ui),
    actions: &[(&str, ModalAction)],
) -> Option<ModalAction> {
    if !*open {
        return None;
    }

    let mut result: Option<ModalAction> = None;
    let mut should_close = false;

    // Semi-transparent backdrop
    let screen = ctx.screen_rect();
    let bg_modal = Theme::c32(&theme.bg_modal);
    egui::Area::new(egui::Id::new("modal_backdrop"))
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(screen, Rounding::ZERO, bg_modal);
            // Click backdrop to cancel
            if resp.clicked() {
                should_close = true;
                result = Some(ModalAction::Cancel);
            }
        });

    // Modal window (no .open() -- we manage open state ourselves to avoid borrow conflict)
    let modal_w = theme.modal_width;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(modal_w, 0.0))
        .title_bar(true)
        .frame(
            egui::Frame::none()
                .fill(Theme::c32(&theme.bg_card))
                .rounding(Rounding::same(theme.border_radius_lg as u8))
                .inner_margin(theme.card_padding)
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            // Body content
            add_content(ui);

            // Separator before action buttons
            if !actions.is_empty() {
                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_sm);

                ui.horizontal(|ui| {
                    // Right-align buttons
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Iterate reversed so the first action ends up rightmost
                        for (label, action) in actions.iter().rev() {
                            let btn = match action {
                                ModalAction::Confirm => {
                                    egui::Button::new(
                                        RichText::new(*label)
                                            .color(theme.text_on_accent())
                                            .size(theme.font_size_body),
                                    )
                                    .fill(theme.accent())
                                    .rounding(Rounding::same(theme.border_radius as u8))
                                }
                                ModalAction::Cancel => {
                                    egui::Button::new(
                                        RichText::new(*label)
                                            .color(theme.text_primary())
                                            .size(theme.font_size_body),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, theme.border()))
                                    .rounding(Rounding::same(theme.border_radius as u8))
                                }
                                ModalAction::Custom(_) => {
                                    egui::Button::new(
                                        RichText::new(*label)
                                            .color(theme.text_primary())
                                            .size(theme.font_size_body),
                                    )
                                    .fill(theme.bg_secondary())
                                    .rounding(Rounding::same(theme.border_radius as u8))
                                }
                            };

                            if ui.add(btn).clicked() {
                                result = Some(action.clone());
                                if matches!(action, ModalAction::Cancel) {
                                    should_close = true;
                                }
                            }
                        }
                    });
                });
            }
        });

    // Close via Escape key
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
        if result.is_none() {
            result = Some(ModalAction::Cancel);
        }
    }

    if should_close {
        *open = false;
    }

    result
}
