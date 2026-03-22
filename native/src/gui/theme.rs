//! GUI theme — colors, spacing, and fonts for the HumanityOS UI.
//!
//! Provides a centralized theme that can be applied to the egui Context
//! so all pages share consistent styling.

use egui::{Color32, FontFamily, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

/// HumanityOS visual theme applied to all egui rendering.
pub struct Theme {
    /// Primary brand color (teal/cyan).
    pub primary: Color32,
    /// Secondary accent color.
    pub accent: Color32,
    /// Background color for panels/windows.
    pub panel_bg: Color32,
    /// Transparent background for overlays (chat, HUD).
    pub overlay_bg: Color32,
    /// Main text color.
    pub text: Color32,
    /// Dimmed/secondary text color.
    pub text_dim: Color32,
    /// Success/health color.
    pub success: Color32,
    /// Warning color.
    pub warning: Color32,
    /// Danger/damage color.
    pub danger: Color32,
    /// Standard corner rounding (u8 pixels).
    pub rounding: Rounding,
    /// Standard panel padding.
    pub padding: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color32::from_rgb(0, 180, 180),
            accent: Color32::from_rgb(100, 200, 255),
            panel_bg: Color32::from_rgba_premultiplied(20, 20, 30, 230),
            overlay_bg: Color32::from_rgba_premultiplied(10, 10, 20, 180),
            text: Color32::from_rgb(230, 230, 240),
            text_dim: Color32::from_rgb(140, 140, 160),
            success: Color32::from_rgb(80, 200, 80),
            warning: Color32::from_rgb(220, 180, 40),
            danger: Color32::from_rgb(220, 60, 60),
            rounding: Rounding::same(6),
            padding: 12.0,
        }
    }
}

/// Load the default HumanityOS theme.
pub fn load_theme() -> Theme {
    Theme::default()
}

impl Theme {
    /// Apply this theme to the egui Context so all widgets use our colors and fonts.
    pub fn apply_to_egui(&self, ctx: &egui::Context) {
        let mut style = Style::default();

        // Dark visuals as base
        let mut visuals = Visuals::dark();
        visuals.panel_fill = self.panel_bg;
        visuals.window_fill = self.panel_bg;
        visuals.window_stroke = Stroke::new(1.0, self.primary.linear_multiply(0.3));

        // Widget styling
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_premultiplied(30, 30, 45, 200);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text);

        visuals.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(40, 40, 60, 200);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_dim);
        visuals.widgets.inactive.corner_radius = self.rounding;

        visuals.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(50, 50, 80, 220);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.corner_radius = self.rounding;

        visuals.widgets.active.bg_fill = self.primary.linear_multiply(0.3);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.primary);
        visuals.widgets.active.corner_radius = self.rounding;

        visuals.selection.bg_fill = self.primary.linear_multiply(0.2);
        visuals.selection.stroke = Stroke::new(1.0, self.primary);

        style.visuals = visuals;

        // Font sizes for each text style
        style.text_styles.insert(TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));

        // Spacing
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(self.padding as i8);

        ctx.set_style(style);
    }

    /// Styled heading label.
    pub fn heading(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(28.0)
            .color(self.primary)
    }

    /// Styled subheading label.
    pub fn subheading(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(18.0)
            .color(self.accent)
    }

    /// Styled body text.
    pub fn body(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(14.0)
            .color(self.text)
    }

    /// Dimmed/secondary text.
    pub fn dimmed(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(12.0)
            .color(self.text_dim)
    }
}
