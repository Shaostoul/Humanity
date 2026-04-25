//! Theme system loaded from data/gui/theme.ron.
//! Provides typed access to all styling variables and applies them to egui.

use egui::{Color32, Context, Rounding, Stroke, Vec2, Visuals};
use serde::{Deserialize, Serialize};

type C = (f32, f32, f32, f32);

#[derive(Debug, Clone, Deserialize, Serialize)]
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

    // Widget variables -- shared across all widgets for consistent UI
    // Spacing
    #[serde(default = "default_row_gap")]
    pub row_gap: f32,
    #[serde(default = "default_section_gap")]
    pub section_gap: f32,
    #[serde(default = "default_item_padding")]
    pub item_padding: f32,
    #[serde(default = "default_panel_margin")]
    pub panel_margin: f32,

    // Sizing
    #[serde(default = "default_icon_size")]
    pub icon_size: f32,
    #[serde(default = "default_icon_small")]
    pub icon_small: f32,
    #[serde(default = "default_row_height")]
    pub row_height: f32,
    #[serde(default = "default_header_height")]
    pub header_height: f32,
    #[serde(default = "default_border_width")]
    pub border_width: f32,
    #[serde(default = "default_status_dot_size")]
    pub status_dot_size: f32,

    // Fonts
    #[serde(default = "default_name_size")]
    pub name_size: f32,
    #[serde(default = "default_body_size")]
    pub body_size: f32,
    #[serde(default = "default_small_size")]
    pub small_size: f32,
    #[serde(default = "default_heading_size")]
    pub heading_size: f32,
    #[serde(default = "default_title_size")]
    pub title_size: f32,

    // Borders
    #[serde(default = "default_border_radius_widget")]
    pub border_radius_widget: f32,

    // Settings layout
    #[serde(default = "default_settings_label_width")]
    pub settings_label_width: f32,

    // Slider styling
    #[serde(default = "default_slider_track_color")]
    pub slider_track: C,
    #[serde(default = "default_slider_track_height")]
    pub slider_track_height: f32,
    #[serde(default = "default_slider_thumb_radius")]
    pub slider_thumb_radius: f32,

    // Checkbox styling
    #[serde(default = "default_checkbox_size")]
    pub checkbox_size: f32,

    // Panel backgrounds (used across all pages, avoids hardcoded Color32::from_rgb)
    #[serde(default = "default_bg_panel")]
    pub bg_panel: C,
    #[serde(default = "default_bg_sidebar")]
    pub bg_sidebar: C,
    #[serde(default = "default_bg_sidebar_dark")]
    pub bg_sidebar_dark: C,

    // Badge styling
    #[serde(default = "default_badge_padding")]
    pub badge_padding_h: f32,
    #[serde(default = "default_badge_padding_v")]
    pub badge_padding_v: f32,
    #[serde(default = "default_badge_radius")]
    pub badge_radius: f32,

    // Chat section tint colors (DMs = red, Groups = green, Servers = blue)
    #[serde(default = "default_dm_bg")]
    pub dm_bg: C,
    #[serde(default = "default_dm_row_bg")]
    pub dm_row_bg: C,
    #[serde(default = "default_dm_row_hover")]
    pub dm_row_hover: C,
    #[serde(default = "default_group_bg")]
    pub group_bg: C,
    #[serde(default = "default_group_row_bg")]
    pub group_row_bg: C,
    #[serde(default = "default_group_row_hover")]
    pub group_row_hover: C,
    #[serde(default = "default_server_bg")]
    pub server_bg: C,
    #[serde(default = "default_server_row_bg")]
    pub server_row_bg: C,
    #[serde(default = "default_server_row_hover")]
    pub server_row_hover: C,

    // Chat buffer size
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
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
    pub fn slider_track(&self) -> Color32 { Self::c32(&self.slider_track) }
    pub fn bg_panel(&self) -> Color32 { Self::c32(&self.bg_panel) }
    pub fn bg_sidebar(&self) -> Color32 { Self::c32(&self.bg_sidebar) }
    pub fn bg_sidebar_dark(&self) -> Color32 { Self::c32(&self.bg_sidebar_dark) }
    pub fn badge_padding(&self) -> Vec2 { Vec2::new(self.badge_padding_h, self.badge_padding_v) }
    pub fn dm_bg(&self) -> Color32 { Self::c32(&self.dm_bg) }
    pub fn dm_row_bg(&self) -> Color32 { Self::c32(&self.dm_row_bg) }
    pub fn dm_row_hover(&self) -> Color32 { Self::c32(&self.dm_row_hover) }
    pub fn group_bg(&self) -> Color32 { Self::c32(&self.group_bg) }
    pub fn group_row_bg(&self) -> Color32 { Self::c32(&self.group_row_bg) }
    pub fn group_row_hover(&self) -> Color32 { Self::c32(&self.group_row_hover) }
    pub fn server_bg(&self) -> Color32 { Self::c32(&self.server_bg) }
    pub fn server_row_bg(&self) -> Color32 { Self::c32(&self.server_row_bg) }
    pub fn server_row_hover(&self) -> Color32 { Self::c32(&self.server_row_hover) }

    /// Icon circle radius (half icon_size minus border padding).
    pub fn icon_radius(&self) -> f32 { self.icon_size / 2.0 - 2.0 }

    /// Half of row_gap, used as inner gap between elements.
    pub fn half_gap(&self) -> f32 { self.row_gap / 2.0 }

    /// Save the current theme to data/gui/theme.ron.
    pub fn save(&self) {
        let paths = [
            std::path::PathBuf::from("data/gui/theme.ron"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("data/gui/theme.ron")))
                .unwrap_or_default(),
        ];

        let pretty = ron::ser::PrettyConfig::default();
        if let Ok(serialized) = ron::ser::to_string_pretty(self, pretty) {
            for path in &paths {
                if path.exists() || path.parent().map_or(false, |p| p.exists()) {
                    if let Err(e) = std::fs::write(path, &serialized) {
                        log::warn!("Failed to save theme to {}: {}", path.display(), e);
                    } else {
                        log::info!("Saved theme to {}", path.display());
                        return;
                    }
                }
            }
        }
    }

    /// Reset only the color tokens to canonical HumanityOS dark defaults.
    /// Widget sizing/spacing/fonts are unchanged. Used by the Settings color editor.
    pub fn reset_color_defaults(&mut self) {
        self.bg_primary = (0.039, 0.039, 0.047, 1.0);     // #0a0a0c
        self.bg_secondary = (0.078, 0.078, 0.094, 1.0);   // #141418
        self.bg_tertiary = (0.145, 0.145, 0.188, 1.0);    // #252530
        self.bg_card = (0.102, 0.102, 0.133, 1.0);        // #1a1a22
        self.bg_modal = (0.0, 0.0, 0.0, 0.7);
        self.accent = (0.929, 0.549, 0.141, 1.0);         // #ED8C24
        self.accent_hover = (1.0, 0.651, 0.239, 1.0);     // #FFA63D
        self.accent_pressed = (0.8, 0.451, 0.098, 1.0);   // #CC7319
        self.text_primary = (0.910, 0.910, 0.918, 1.0);   // #e8e8ea
        self.text_secondary = (0.533, 0.533, 0.580, 1.0); // #888894
        self.text_muted = (0.416, 0.416, 0.459, 1.0);     // #6a6a75
        self.text_on_accent = (1.0, 1.0, 1.0, 1.0);
        self.success = (0.165, 0.722, 0.439, 1.0);        // #2AB870
        self.warning = (0.961, 0.722, 0.231, 1.0);        // #F5B83B
        self.danger = (0.937, 0.310, 0.310, 1.0);         // #EF4F4F
        self.info = (0.310, 0.667, 0.937, 1.0);           // #4FAAEF
        self.border = (0.165, 0.165, 0.208, 1.0);         // #2a2a35
        self.border_focus = (0.929, 0.549, 0.141, 1.0);
        self.badge_admin = (0.937, 0.310, 0.310, 1.0);
        self.badge_mod = (0.310, 0.667, 0.937, 1.0);
        self.badge_verified = (0.165, 0.722, 0.439, 1.0);
        self.badge_donor = (0.929, 0.549, 0.141, 1.0);
        self.badge_live = (0.937, 0.310, 0.310, 1.0);
    }

    /// Reset only the widget variables to their defaults while keeping colors.
    pub fn reset_widget_defaults(&mut self) {
        self.row_gap = 2.0;
        self.section_gap = 4.0;
        self.item_padding = 4.0;
        self.panel_margin = 8.0;
        self.icon_size = 32.0;
        self.icon_small = 16.0;
        self.row_height = 18.0;
        self.header_height = 36.0;
        self.border_width = 1.0;
        self.status_dot_size = 8.0;
        self.name_size = 14.0;
        self.body_size = 14.0;
        self.small_size = 11.0;
        self.heading_size = 18.0;
        self.title_size = 24.0;
        self.border_radius_widget = 0.0;
        self.settings_label_width = 200.0;
        self.slider_track = (0.2, 0.2, 0.25, 1.0);
        self.slider_track_height = 4.0;
        self.slider_thumb_radius = 7.0;
        self.checkbox_size = 18.0;
    }

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

// Serde default functions for widget variables (backward-compatible RON loading)
fn default_row_gap() -> f32 { 2.0 }
fn default_section_gap() -> f32 { 4.0 }
fn default_item_padding() -> f32 { 4.0 }
fn default_panel_margin() -> f32 { 8.0 }
fn default_icon_size() -> f32 { 32.0 }
fn default_icon_small() -> f32 { 16.0 }
fn default_row_height() -> f32 { 18.0 }
fn default_header_height() -> f32 { 36.0 }
fn default_border_width() -> f32 { 1.0 }
fn default_status_dot_size() -> f32 { 8.0 }
fn default_name_size() -> f32 { 14.0 }
fn default_body_size() -> f32 { 14.0 }
fn default_small_size() -> f32 { 11.0 }
fn default_heading_size() -> f32 { 18.0 }
fn default_title_size() -> f32 { 24.0 }
fn default_border_radius_widget() -> f32 { 0.0 }
fn default_settings_label_width() -> f32 { 200.0 }
fn default_slider_track_color() -> C { (0.2, 0.2, 0.25, 1.0) }
fn default_slider_track_height() -> f32 { 4.0 }
fn default_slider_thumb_radius() -> f32 { 7.0 }
fn default_checkbox_size() -> f32 { 18.0 }
fn default_bg_panel() -> C { (0.078, 0.078, 0.098, 1.0) }      // rgb(20, 20, 25)
fn default_bg_sidebar() -> C { (0.086, 0.086, 0.110, 1.0) }    // rgb(22, 22, 28)
fn default_bg_sidebar_dark() -> C { (0.118, 0.118, 0.141, 1.0) } // rgb(30, 30, 36)
fn default_badge_padding() -> f32 { 6.0 }
fn default_badge_padding_v() -> f32 { 2.0 }
fn default_badge_radius() -> f32 { 3.0 }
fn default_dm_bg() -> C { (0.176, 0.059, 0.059, 1.0) }
fn default_dm_row_bg() -> C { (0.216, 0.078, 0.078, 1.0) }
fn default_dm_row_hover() -> C { (0.275, 0.098, 0.098, 1.0) }
fn default_group_bg() -> C { (0.059, 0.176, 0.059, 1.0) }
fn default_group_row_bg() -> C { (0.078, 0.216, 0.078, 1.0) }
fn default_group_row_hover() -> C { (0.098, 0.275, 0.098, 1.0) }
fn default_server_bg() -> C { (0.059, 0.059, 0.176, 1.0) }
fn default_server_row_bg() -> C { (0.078, 0.078, 0.216, 1.0) }
fn default_server_row_hover() -> C { (0.098, 0.098, 0.275, 1.0) }
fn default_max_messages() -> usize { 200 }

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
        // Widget variables
        row_gap: 2.0,
        section_gap: 4.0,
        item_padding: 4.0,
        panel_margin: 8.0,
        icon_size: 32.0,
        icon_small: 16.0,
        row_height: 18.0,
        header_height: 36.0,
        border_width: 1.0,
        status_dot_size: 8.0,
        name_size: 14.0,
        body_size: 14.0,
        small_size: 11.0,
        heading_size: 18.0,
        title_size: 24.0,
        border_radius_widget: 0.0,
        settings_label_width: 200.0,
        slider_track: (0.2, 0.2, 0.25, 1.0),
        slider_track_height: 4.0,
        slider_thumb_radius: 7.0,
        checkbox_size: 18.0,
        // Panel backgrounds
        bg_panel: default_bg_panel(),
        bg_sidebar: default_bg_sidebar(),
        bg_sidebar_dark: default_bg_sidebar_dark(),
        // Badge styling
        badge_padding_h: default_badge_padding(),
        badge_padding_v: default_badge_padding_v(),
        badge_radius: default_badge_radius(),
        // Chat section tint colors
        dm_bg: default_dm_bg(),
        dm_row_bg: default_dm_row_bg(),
        dm_row_hover: default_dm_row_hover(),
        group_bg: default_group_bg(),
        group_row_bg: default_group_row_bg(),
        group_row_hover: default_group_row_hover(),
        server_bg: default_server_bg(),
        server_row_bg: default_server_row_bg(),
        server_row_hover: default_server_row_hover(),
        max_messages: default_max_messages(),
    }
}
