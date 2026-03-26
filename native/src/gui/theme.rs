//! Theme system loaded from data/gui/theme.ron.
//! Provides typed access to all styling variables and applies them to egui.

use egui::{Color32, Context, Rounding, Stroke, Vec2, Visuals};
use serde::Deserialize;

type C = (f32, f32, f32, f32);

#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    pub bg_primary: C,
    pub bg_secondary: C,
    pub bg_tertiary: C,
    pub bg_card: C,
    pub bg_modal: C,
    pub accent: C,
    pub accent_hover: C,
    pub accent_pressed: C,
    pub text_primary: C,
    pub text_secondary: C,
    pub text_muted: C,
    pub text_on_accent: C,
    pub success: C,
    pub warning: C,
    pub danger: C,
    pub info: C,
    pub border: C,
    pub border_focus: C,
    pub badge_admin: C,
    pub badge_mod: C,
    pub badge_verified: C,
    pub badge_donor: C,
    pub badge_live: C,
    pub font_size_small: f32,
    pub font_size_body: f32,
    pub font_size_heading: f32,
    pub font_size_title: f32,
    pub spacing_xs: f32,
    pub spacing_sm: f32,
    pub spacing_md: f32,
    pub spacing_lg: f32,
    pub spacing_xl: f32,
    pub border_radius: f32,
    pub border_radius_lg: f32,
    pub button_height: f32,
    pub button_padding_h: f32,
    pub input_height: f32,
    pub sidebar_width: f32,
    pub card_padding: f32,
    pub modal_width: f32,
}

impl Theme {
    pub fn c32(c: &C) -> Color32 {
        Color32::from_rgba_unmultiplied(
            (c.0 * 255.0) as u8,
            (c.1 * 255.0) as u8,
            (c.2 * 255.0) as u8,
            (c.3 * 255.0) as u8,
        )
    }

    pub fn accent(&self) -> Color32 { Self::c32(&self.accent) }
    pub fn accent_hover(&self) -> Color32 { Self::c32(&self.accent_hover) }
    pub fn accent_pressed(&self) -> Color32 { Self::c32(&self.accent_pressed) }
    pub fn bg_primary(&self) -> Color32 { Self::c32(&self.bg_primary) }
    pub fn bg_secondary(&self) -> Color32 { Self::c32(&self.bg_secondary) }
    pub fn bg_card(&self) -> Color32 { Self::c32(&self.bg_card) }
    pub fn text_primary(&self) -> Color32 { Self::c32(&self.text_primary) }
    pub fn text_secondary(&self) -> Color32 { Self::c32(&self.text_secondary) }
    pub fn text_muted(&self) -> Color32 { Self::c32(&self.text_muted) }
    pub fn text_on_accent(&self) -> Color32 { Self::c32(&self.text_on_accent) }
    pub fn success(&self) -> Color32 { Self::c32(&self.success) }
    pub fn warning(&self) -> Color32 { Self::c32(&self.warning) }
    pub fn danger(&self) -> Color32 { Self::c32(&self.danger) }
    pub fn border(&self) -> Color32 { Self::c32(&self.border) }

    /// Apply this theme to an egui Context (sets visuals, spacing).
    /// Colors are matched to the web theme.css for visual consistency.
    pub fn apply_to_egui(&self, ctx: &Context) {
        let mut visuals = Visuals::dark();

        // Panel and window fills matched to website
        visuals.panel_fill = self.bg_primary();                        // #0a0a0c
        visuals.window_fill = self.bg_secondary();                    // #141418
        visuals.faint_bg_color = self.bg_card();                      // #1a1a22
        visuals.extreme_bg_color = Self::c32(&self.bg_tertiary);     // #252530
        visuals.override_text_color = Some(self.text_primary());     // #e8e8ea

        // Noninteractive widgets (labels, separators)
        visuals.widgets.noninteractive.bg_fill = self.bg_secondary();  // #141418
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_primary()); // #e8e8ea

        // Inactive widgets (buttons at rest)
        visuals.widgets.inactive.bg_fill = self.bg_card();             // #1a1a22
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_secondary());

        // Hovered widgets
        visuals.widgets.hovered.bg_fill = Self::c32(&self.bg_tertiary); // #252530
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text_primary());

        // Active (pressed) widgets
        visuals.widgets.active.bg_fill = self.accent();                // #ED8C24
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.text_on_accent());

        // Selection
        visuals.selection.bg_fill = self.accent();                     // #ED8C24
        visuals.selection.stroke = Stroke::new(1.0, self.text_on_accent());

        // Window border
        visuals.window_stroke = Stroke::new(1.0, self.border());      // #2a2a35

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(self.spacing_sm, self.spacing_sm);
        style.spacing.button_padding = Vec2::new(self.button_padding_h, 6.0);
        ctx.set_style(style);
    }
}

/// Load theme from data/gui/theme.ron, falling back to defaults.
pub fn load_theme() -> Theme {
    let paths = [
        std::path::PathBuf::from("data/gui/theme.ron"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("data/gui/theme.ron")))
            .unwrap_or_default(),
    ];

    for path in &paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            match ron::from_str::<Theme>(&contents) {
                Ok(theme) => {
                    log::info!("Loaded theme from {}", path.display());
                    return theme;
                }
                Err(e) => {
                    log::warn!("Failed to parse theme from {}: {}", path.display(), e);
                }
            }
        }
    }

    log::info!("Using default theme (theme.ron not found)");
    default_theme()
}

fn default_theme() -> Theme {
    Theme {
        bg_primary: (0.039, 0.039, 0.047, 1.0),       // #0a0a0c
        bg_secondary: (0.078, 0.078, 0.094, 1.0),     // #141418
        bg_tertiary: (0.145, 0.145, 0.188, 1.0),      // #252530
        bg_card: (0.102, 0.102, 0.133, 1.0),           // #1a1a22
        bg_modal: (0.0, 0.0, 0.0, 0.7),
        accent: (0.929, 0.549, 0.141, 1.0),            // #ED8C24
        accent_hover: (1.0, 0.65, 0.24, 1.0),
        accent_pressed: (0.8, 0.45, 0.10, 1.0),
        text_primary: (0.910, 0.910, 0.918, 1.0),     // #e8e8ea
        text_secondary: (0.533, 0.533, 0.580, 1.0),   // #888894
        text_muted: (0.416, 0.416, 0.459, 1.0),       // #6a6a75
        text_on_accent: (0.05, 0.05, 0.05, 1.0),
        success: (0.2, 0.75, 0.3, 1.0),
        warning: (0.95, 0.75, 0.1, 1.0),
        danger: (0.9, 0.25, 0.2, 1.0),
        info: (0.2, 0.5, 0.9, 1.0),
        border: (0.165, 0.165, 0.208, 1.0),            // #2a2a35
        border_focus: (0.929, 0.549, 0.141, 1.0),      // #ED8C24
        badge_admin: (0.9, 0.5, 0.13, 1.0),
        badge_mod: (0.15, 0.68, 0.38, 1.0),
        badge_verified: (0.2, 0.58, 0.85, 1.0),
        badge_donor: (0.61, 0.35, 0.71, 1.0),
        badge_live: (0.9, 0.3, 0.24, 1.0),
        font_size_small: 12.0,
        font_size_body: 14.0,
        font_size_heading: 20.0,
        font_size_title: 28.0,
        spacing_xs: 4.0,
        spacing_sm: 8.0,
        spacing_md: 16.0,
        spacing_lg: 24.0,
        spacing_xl: 32.0,
        border_radius: 6.0,
        border_radius_lg: 12.0,
        button_height: 36.0,
        button_padding_h: 16.0,
        input_height: 36.0,
        sidebar_width: 280.0,
        card_padding: 16.0,
        modal_width: 480.0,
    }
}
