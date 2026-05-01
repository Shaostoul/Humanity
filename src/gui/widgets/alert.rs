//! Alert/notification banner widget — themed inline messages for info, success,
//! warning, and error states.
//!
//! Replaces ad-hoc `Frame::none().fill(Color32::from_rgb(...)).show(...)` calls
//! that hardcode colors and bypass the theme system.
//!
//! ```ignore
//! widgets::alert(ui, theme, AlertKind::Warning, "Unsaved changes — save before leaving");
//! widgets::alert(ui, theme, AlertKind::Error, "Failed to load profile data");
//! ```

use egui::{Color32, Frame, RichText, Rounding, Stroke, Ui};
use super::super::theme::Theme;

/// Severity of an alert. Drives the icon + accent colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Info,
    Success,
    Warning,
    Error,
}

impl AlertKind {
    fn icon(&self) -> &'static str {
        match self {
            AlertKind::Info    => "ℹ",
            AlertKind::Success => "✓",
            AlertKind::Warning => "⚠",
            AlertKind::Error   => "✕",
        }
    }

    fn accent(&self, theme: &Theme) -> Color32 {
        // Info uses the brand accent color (no separate info token in theme.ron yet).
        match self {
            AlertKind::Info    => theme.accent(),
            AlertKind::Success => theme.success(),
            AlertKind::Warning => theme.warning(),
            AlertKind::Error   => theme.danger(),
        }
    }
}

/// Render an inline alert banner with icon + message.
pub fn alert(ui: &mut Ui, theme: &Theme, kind: AlertKind, message: &str) {
    alert_inner(ui, theme, kind, message, None);
}

/// Render an alert banner with an additional title line above the message.
pub fn alert_with_title(
    ui: &mut Ui,
    theme: &Theme,
    kind: AlertKind,
    title: &str,
    message: &str,
) {
    alert_inner(ui, theme, kind, message, Some(title));
}

fn alert_inner(
    ui: &mut Ui,
    theme: &Theme,
    kind: AlertKind,
    message: &str,
    title: Option<&str>,
) {
    let accent = kind.accent(theme);
    // 18% alpha background tint of the accent — readable but unobtrusive.
    let bg = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 46);

    Frame::none()
        .fill(bg)
        .stroke(Stroke::new(1.0, accent))
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(kind.icon())
                        .size(theme.font_size_heading)
                        .color(accent)
                        .strong(),
                );
                ui.add_space(theme.spacing_sm);
                ui.vertical(|ui| {
                    if let Some(t) = title {
                        ui.label(
                            RichText::new(t)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                    }
                    ui.label(
                        RichText::new(message)
                            .size(theme.font_size_small)
                            .color(theme.text_primary()),
                    );
                });
            });
        });
}
