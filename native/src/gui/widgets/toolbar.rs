//! Horizontal toolbar with icon buttons, separators, labels, and spacers.
//! Used by: file browser, editor, map controls, inventory actions.

use egui::{RichText, Rounding, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// A single item in a toolbar.
pub enum ToolbarItem {
    /// Clickable icon button with tooltip.
    Button {
        icon: &'static str,
        tooltip: &'static str,
        id: &'static str,
    },
    /// Vertical separator line.
    Separator,
    /// Non-interactive label text.
    Label(String),
    /// Flexible spacer that pushes subsequent items to the right.
    Spacer,
}

/// Render a horizontal toolbar.
///
/// Returns the `id` of the button that was clicked this frame, if any.
pub fn toolbar<'a>(
    ui: &mut Ui,
    theme: &Theme,
    items: &'a [ToolbarItem],
) -> Option<&'a str> {
    let mut clicked: Option<&str> = None;
    let rounding = Rounding::same(theme.border_radius as u8);

    ui.horizontal(|ui| {
        for item in items {
            match item {
                ToolbarItem::Button { icon, tooltip, id } => {
                    let btn = egui::Button::new(
                        RichText::new(*icon)
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, theme.border()))
                    .min_size(Vec2::new(theme.button_height, theme.button_height))
                    .rounding(rounding);

                    if ui.add(btn).on_hover_text(*tooltip).clicked() {
                        clicked = Some(id);
                    }
                }
                ToolbarItem::Separator => {
                    ui.separator();
                }
                ToolbarItem::Label(text) => {
                    ui.label(
                        RichText::new(text)
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                }
                ToolbarItem::Spacer => {
                    // Use all remaining width to push subsequent items right.
                    let remaining = ui.available_width() - theme.spacing_sm;
                    if remaining > 0.0 {
                        ui.add_space(remaining);
                    }
                }
            }
        }
    });

    clicked
}
