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
    /// Odd-row stripe background for tables/lists (operator 2026-06-08: a subtle
    /// #020202 against the #000000 panel). Drives egui's faint_bg_color.
    #[serde(default = "default_row_stripe")]
    pub row_stripe: C,
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
    /// Solar-system orbit ring color in the FPS sky (v0.786, operator sky
    /// settings). Default = the near-black #020305 the rings always used.
    #[serde(default = "default_orbit_line")]
    pub orbit_line: C,
    /// Constellation figure line color in the FPS sky + Cosmos night-sky
    /// (v0.786). Default = the operator-chosen teal #224444.
    #[serde(default = "default_constellation_line")]
    pub constellation_line: C,
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
    /// Vertical padding inside buttons. With button_padding_h, gives buttons even
    /// slim padding; the font then drives the height (operator 2026-06-08: slim,
    /// even-padded buttons). Themeable.
    #[serde(default = "default_button_pad_y")]
    pub button_pad_y: f32,
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
    // Chat message-row avatar: the gutter square + its gap to the content.
    // Shared with the web view via gen-theme-css.js (--avatar-size/--avatar-gap)
    // so both platforms read ONE source. See widgets/row.rs.
    #[serde(default = "default_avatar_size")]
    pub avatar_size: f32,
    #[serde(default = "default_avatar_gap")]
    pub avatar_gap: f32,
    // Chat timestamp-pill corner radius. Shared with web (--pill-radius).
    #[serde(default = "default_pill_radius")]
    pub pill_radius: f32,
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

    // Stat / status row layout (the vitals + any "name | value | bar" table). All
    // themeable so the inventory's aligned columns and the capped status bars are
    // not hardcoded (operator 2026-06-08: universal styling variables, editable).
    #[serde(default = "default_stat_name_width")]
    pub stat_name_width: f32,
    #[serde(default = "default_stat_value_width")]
    pub stat_value_width: f32,
    #[serde(default = "default_status_bar_width")]
    pub status_bar_width: f32,
    /// Height of status / progress bars (vitals + the garden growth bars). Thin
    /// (operator 2026-06-08: ~5px) so rows stay tight; themeable.
    #[serde(default = "default_status_bar_height")]
    pub status_bar_height: f32,
    /// Height of the compact inline action buttons (a table row's
    /// Harvest/Water/Fertilize cluster) — short so several sit cleanly side by side.
    #[serde(default = "default_compact_button_height")]
    pub compact_button_height: f32,
    // Expandable-row column widths (the garden/mining spreadsheet rows + the
    // inventory tree's item column). Three sizes cover the row_cell layouts:
    // narrow (slot numbers), short (status/availability), name (the label column).
    #[serde(default = "default_cell_narrow_width")]
    pub cell_narrow_width: f32,
    #[serde(default = "default_cell_short_width")]
    pub cell_short_width: f32,
    #[serde(default = "default_cell_name_width")]
    pub cell_name_width: f32,

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

    // ── Studio source-type palette (v0.670) ──
    // Broadcasting Studio canvas colors: each source kind (camera, screen
    // share, mic, chat overlay, image, text, timer) gets a semantic fill so
    // the Program/Preview canvases stay readable at a glance. The source's
    // own opacity slider is applied at draw time on top of these RGB tokens
    // (see studio.rs `with_alpha`), so only the base colors live here.
    // Migrated from hardcoded Color32 literals in pages/studio.rs.
    #[serde(default = "default_studio_source_camera")]
    pub studio_source_camera: C,
    #[serde(default = "default_studio_source_screen")]
    pub studio_source_screen: C,
    #[serde(default = "default_studio_source_microphone")]
    pub studio_source_microphone: C,
    #[serde(default = "default_studio_source_chat")]
    pub studio_source_chat: C,
    #[serde(default = "default_studio_source_image")]
    pub studio_source_image: C,
    #[serde(default = "default_studio_source_text")]
    pub studio_source_text: C,
    #[serde(default = "default_studio_source_timer")]
    pub studio_source_timer: C,
    /// Outline stroke around each source rectangle on the studio canvases.
    #[serde(default = "default_studio_source_border")]
    pub studio_source_border: C,
    /// Source-name label text painted at the center of each source rectangle.
    #[serde(default = "default_studio_source_label")]
    pub studio_source_label: C,
    /// The "Away: h:mm:ss" AFK timer text in the studio controls bar.
    #[serde(default = "default_studio_afk")]
    pub studio_afk: C,
    /// Background trough of the studio audio-level meter (the fill on top
    /// uses success/warning/danger by level).
    #[serde(default = "default_studio_meter_bg")]
    pub studio_meter_bg: C,

    // Chat buffer size
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,

    // ── Nav category colors (v0.175.0) ──
    // Two-tier nav top categories. These define the canonical colors for
    // Reality (red), Sim (purple), Tools (blue), Settings (gray) in both
    // the active button border and the inactive tinted background. Editable
    // in-app via Settings; previously hardcoded in escape_menu.rs as const.
    #[serde(default = "default_nav_reality")]
    pub nav_reality: C,
    #[serde(default = "default_nav_sim")]
    pub nav_sim: C,
    #[serde(default = "default_nav_tools")]
    pub nav_tools: C,
    #[serde(default = "default_nav_settings")]
    pub nav_settings: C,
    /// Dev top-tier category color (amber). Visibility gated by
    /// `nav_dev_visible`; defaults to ON during the development period
    /// and will flip to OFF at v1.0 unless the user opts in.
    #[serde(default = "default_nav_dev")]
    pub nav_dev: C,

    // Legacy single-row nav group colors. Identity / contextual / system
    // groupings on the legacy nav. Same colors as Reality / (sub-set) /
    // Tools so the design language stays consistent if a user toggles
    // between layouts.
    #[serde(default = "default_nav_legacy_red")]
    pub nav_legacy_red: C,
    #[serde(default = "default_nav_legacy_green")]
    pub nav_legacy_green: C,
    #[serde(default = "default_nav_legacy_blue")]
    pub nav_legacy_blue: C,

    // Nav sizing knobs. Pulled out of the hardcoded constants so the
    // operator can tune nav presence without recompiling.
    #[serde(default = "default_nav_separator_height")]
    pub nav_separator_height: f32,
    #[serde(default = "default_nav_active_border_width")]
    pub nav_active_border_width: f32,
    #[serde(default = "default_nav_hover_border_width")]
    pub nav_hover_border_width: f32,

    // ── Animations (v0.177.0) ──
    // Master switch + per-element style/speed tokens. Lets users turn
    // off RGB cycling (motion-sensitivity / focus mode), pick a static
    // color instead, or change the in-menu attack indicator from red
    // pulse to yellow / flash / etc.
    //
    // animations_enabled = master switch. When false, all animations
    // freeze at their first frame (or are skipped entirely for separators).
    // Style enums use u8 codes so they round-trip cleanly through RON;
    // editor uses radio buttons. See `AnimationStyle` / `AttackStyle`.
    #[serde(default = "default_animations_enabled")]
    pub animations_enabled: bool,
    #[serde(default = "default_nav_separator_anim")]
    pub nav_separator_animation: u8,
    #[serde(default = "default_anim_speed")]
    pub nav_separator_animation_speed: f32,
    #[serde(default = "default_active_border_anim")]
    pub nav_active_border_animation: u8,
    #[serde(default = "default_attack_anim")]
    pub attack_indicator_style: u8,
    #[serde(default = "default_anim_speed")]
    pub attack_indicator_speed: f32,

    /// Show the Dev top-tier category in the nav (Testing / Bugs /
    /// Agents / AI Usage / Files). Default true during the dev period;
    /// at v1.0 this flips to false and only shows when explicitly
    /// toggled in Settings → Account → Developer mode.
    #[serde(default = "default_nav_dev_visible")]
    pub nav_dev_visible: bool,
    /// Master switch for the in-app dev/debug CHEATS — the "Dev:" provisioning
    /// buttons across the app (stock all materials, stock seeds, grow all crops,
    /// max skills). ON during development so every loop is testable in one click;
    /// turn OFF for a clean demo or a public server where players shouldn't be
    /// able to conjure resources. Toggle in Settings -> Animations -> Developer cheats.
    #[serde(default = "default_cheats_enabled")]
    pub cheats_enabled: bool,
}

/// Animation style enum encoded as u8 in theme.ron for serde stability.
/// Used by both the nav separator and the active-button border.
pub mod anim {
    pub const OFF:       u8 = 0; // no animation, paint nothing or static border
    pub const SOLID:     u8 = 1; // single solid color (uses contextual color)
    pub const RGB_CYCLE: u8 = 2; // hue spectrum cycle (current default)
    pub const PULSE:     u8 = 3; // single-color brightness breathing
}

/// Attack indicator style encoded as u8.
pub mod attack {
    pub const NONE:        u8 = 0; // no override (channeling stays cyclic)
    pub const PULSE_RED:   u8 = 1; // current default — bright<->dark red 2 Hz
    pub const PULSE_YELLOW:u8 = 2; // softer warning vibe
    pub const FLASH_WHITE: u8 = 3; // rapid bright white blink
    pub const BORDER_ONLY: u8 = 4; // hold border at solid danger color, no pulse
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
    pub fn row_stripe(&self) -> Color32 { Self::c32(&self.row_stripe) }
    /// Raised-surface / hover token (#252530). Already used internally
    /// for `widgets.hovered.bg_fill`; exposed v0.254 so the universal
    /// secondary button can render as a raised surface (was invisible
    /// transparent-fill before).
    pub fn bg_tertiary(&self) -> Color32 { Self::c32(&self.bg_tertiary) }
    pub fn text_primary(&self) -> Color32 { Self::c32(&self.text_primary) }
    pub fn text_secondary(&self) -> Color32 { Self::c32(&self.text_secondary) }
    pub fn text_muted(&self) -> Color32 { Self::c32(&self.text_muted) }
    pub fn text_on_accent(&self) -> Color32 { Self::c32(&self.text_on_accent) }
    pub fn success(&self) -> Color32 { Self::c32(&self.success) }
    pub fn warning(&self) -> Color32 { Self::c32(&self.warning) }
    pub fn danger(&self) -> Color32 { Self::c32(&self.danger) }
    pub fn info(&self) -> Color32 { Self::c32(&self.info) }
    pub fn orbit_line(&self) -> Color32 { Self::c32(&self.orbit_line) }
    pub fn constellation_line(&self) -> Color32 { Self::c32(&self.constellation_line) }
    pub fn border(&self) -> Color32 { Self::c32(&self.border) }
    pub fn border_focus(&self) -> Color32 { Self::c32(&self.border_focus) }

    // Nav tokens (v0.175.0). All formerly-hardcoded constants in
    // escape_menu.rs migrated to theme.ron so the Settings page color
    // editor can tune them.
    pub fn nav_reality(&self) -> Color32 { Self::c32(&self.nav_reality) }
    pub fn nav_sim(&self) -> Color32 { Self::c32(&self.nav_sim) }
    pub fn nav_tools(&self) -> Color32 { Self::c32(&self.nav_tools) }
    pub fn nav_settings(&self) -> Color32 { Self::c32(&self.nav_settings) }
    pub fn nav_dev(&self) -> Color32 { Self::c32(&self.nav_dev) }
    pub fn nav_legacy_red(&self) -> Color32 { Self::c32(&self.nav_legacy_red) }
    pub fn nav_legacy_green(&self) -> Color32 { Self::c32(&self.nav_legacy_green) }
    pub fn nav_legacy_blue(&self) -> Color32 { Self::c32(&self.nav_legacy_blue) }
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

    // Studio source-type palette (v0.670) — see field docs above.
    pub fn studio_source_camera(&self) -> Color32 { Self::c32(&self.studio_source_camera) }
    pub fn studio_source_screen(&self) -> Color32 { Self::c32(&self.studio_source_screen) }
    pub fn studio_source_microphone(&self) -> Color32 { Self::c32(&self.studio_source_microphone) }
    pub fn studio_source_chat(&self) -> Color32 { Self::c32(&self.studio_source_chat) }
    pub fn studio_source_image(&self) -> Color32 { Self::c32(&self.studio_source_image) }
    pub fn studio_source_text(&self) -> Color32 { Self::c32(&self.studio_source_text) }
    pub fn studio_source_timer(&self) -> Color32 { Self::c32(&self.studio_source_timer) }
    pub fn studio_source_border(&self) -> Color32 { Self::c32(&self.studio_source_border) }
    pub fn studio_source_label(&self) -> Color32 { Self::c32(&self.studio_source_label) }
    pub fn studio_afk(&self) -> Color32 { Self::c32(&self.studio_afk) }
    pub fn studio_meter_bg(&self) -> Color32 { Self::c32(&self.studio_meter_bg) }

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
        self.row_stripe = (0.0157, 0.0157, 0.0157, 1.0);   // #040404
        self.bg_modal = (0.0, 0.0, 0.0, 0.7);
        self.accent = (0.929, 0.549, 0.141, 1.0);         // #ED8C24
        self.accent_hover = (1.0, 0.651, 0.239, 1.0);     // #FFA63D
        self.accent_pressed = (0.8, 0.451, 0.098, 1.0);   // #CC7319
        self.text_primary = (0.910, 0.910, 0.918, 1.0);   // #e8e8ea
        // v0.195.0: brighten secondary + muted to address operator's
        // astigmatism — the old #888894 / #6a6a75 grays were hard to
        // read against tinted card backgrounds without glasses.
        // Hierarchy preserved: primary > secondary > muted, just shifted
        // up the brightness scale.
        self.text_secondary = (0.706, 0.706, 0.745, 1.0); // #b4b4be (was #888894)
        self.text_muted = (0.580, 0.580, 0.624, 1.0);     // #94949f (was #6a6a75)
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
        self.stat_name_width = 86.0;
        self.stat_value_width = 82.0;
        self.status_bar_width = 200.0;
        self.status_bar_height = 5.0;
        self.compact_button_height = 18.0;
        self.cell_narrow_width = 60.0;
        self.cell_short_width = 90.0;
        self.cell_name_width = 150.0;
        self.button_pad_y = 3.0;
    }

    /// Apply this theme to an egui Context (sets visuals, spacing).
    /// Colors are matched to the web theme.css for visual consistency.
    pub fn apply_to_egui(&self, ctx: &Context) {
        let mut visuals = Visuals::dark();

        // Panel and window fills matched to website
        visuals.panel_fill = self.bg_primary();                        // #0a0a0c
        visuals.window_fill = self.bg_secondary();                    // #141418
        visuals.faint_bg_color = self.row_stripe();                   // #020202 (subtle odd-row stripe)
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
        style.spacing.button_padding = Vec2::new(self.button_padding_h, self.button_pad_y);
        // egui defaults `debug.show_unaligned` ON in debug builds, painting an orange
        // "Unaligned" marker over any Ui whose rect isn't pixel-aligned. That bleeds
        // into dev runs and the headless UI snapshots (release builds never show it),
        // so switch it off. The `Style.debug` field only exists under debug_assertions.
        #[cfg(debug_assertions)]
        {
            style.debug.show_unaligned = false;
        }
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
fn default_avatar_size() -> f32 { 32.0 }
fn default_avatar_gap() -> f32 { 8.0 }
fn default_pill_radius() -> f32 { 9.0 }
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
fn default_stat_name_width() -> f32 { 86.0 }
fn default_stat_value_width() -> f32 { 82.0 }
fn default_status_bar_width() -> f32 { 200.0 }
fn default_status_bar_height() -> f32 { 5.0 }
fn default_compact_button_height() -> f32 { 18.0 }
fn default_cell_narrow_width() -> f32 { 60.0 }
fn default_cell_short_width() -> f32 { 90.0 }
fn default_cell_name_width() -> f32 { 150.0 }
fn default_button_pad_y() -> f32 { 3.0 }
fn default_bg_panel() -> C { (0.078, 0.078, 0.098, 1.0) }      // rgb(20, 20, 25)
fn default_row_stripe() -> C { (0.0157, 0.0157, 0.0157, 1.0) } // #040404
fn default_orbit_line() -> C { (0.0078, 0.0118, 0.0196, 1.0) } // #020305 near-black
fn default_constellation_line() -> C { (0.133, 0.267, 0.267, 1.0) } // #224444 teal
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

// Studio source-type palette defaults (v0.670). Same RGB values as the
// pre-migration hardcoded literals in pages/studio.rs so existing themes
// look identical post-migration.
fn default_studio_source_camera()     -> C { (0.18039216, 0.52549020, 0.75686275, 1.0) } // #2E86C1 — blue
fn default_studio_source_screen()     -> C { (0.55686275, 0.26666667, 0.67843137, 1.0) } // #8E44AD — purple
fn default_studio_source_microphone() -> C { (0.90588235, 0.29803922, 0.23529412, 1.0) } // #E74C3C — red
fn default_studio_source_chat()       -> C { (0.18039216, 0.80000000, 0.44313725, 1.0) } // #2ECC71 — green
fn default_studio_source_image()      -> C { (0.94509804, 0.76862745, 0.05882353, 1.0) } // #F1C40F — yellow
fn default_studio_source_text()       -> C { (0.92549020, 0.94117647, 0.94509804, 1.0) } // #ECF0F1 — near-white
fn default_studio_source_timer()      -> C { (0.90196078, 0.49411765, 0.13333334, 1.0) } // #E67E22 — orange
fn default_studio_source_border()     -> C { (1.0, 1.0, 1.0, 1.0) }                      // white
fn default_studio_source_label()      -> C { (1.0, 1.0, 1.0, 1.0) }                      // white
fn default_studio_afk()               -> C { (0.60784314, 0.34901961, 0.71372549, 1.0) } // #9B59B6 — purple
fn default_studio_meter_bg()          -> C { (0.11764706, 0.11764706, 0.15686275, 1.0) } // #1E1E28 — dark trough

// Nav category colors (v0.175.0). Matches the original constants from
// escape_menu.rs so existing themes look identical post-migration.
fn default_nav_reality()  -> C { (0.906, 0.298, 0.235, 1.0) } // #E74C3C — red
fn default_nav_sim()      -> C { (0.424, 0.361, 0.906, 1.0) } // #6C5CE7 — purple
fn default_nav_tools()    -> C { (0.204, 0.596, 0.859, 1.0) } // #3498DB — blue
fn default_nav_settings() -> C { (0.498, 0.549, 0.553, 1.0) } // #7F8C8D — gray
fn default_nav_dev()      -> C { (0.953, 0.612, 0.071, 1.0) } // #F39C12 — amber
fn default_nav_legacy_red()   -> C { (0.906, 0.298, 0.235, 1.0) } // #E74C3C
fn default_nav_legacy_green() -> C { (0.180, 0.800, 0.443, 1.0) } // #2ECC71
fn default_nav_legacy_blue()  -> C { (0.204, 0.596, 0.859, 1.0) } // #3498DB
fn default_nav_separator_height() -> f32 { 3.0 }
fn default_nav_active_border_width() -> f32 { 2.0 }
fn default_nav_hover_border_width() -> f32 { 2.0 }
// Animation defaults (v0.177.0).
fn default_animations_enabled() -> bool { true }
fn default_nav_separator_anim() -> u8 { 2 }   // RGB_CYCLE
fn default_active_border_anim() -> u8 { 2 }   // RGB_CYCLE
fn default_attack_anim() -> u8 { 1 }          // PULSE_RED
fn default_anim_speed() -> f32 { 1.0 }
fn default_nav_dev_visible() -> bool { true } // ON during dev period; flip at v1.0
fn default_cheats_enabled() -> bool { true } // ON during dev; turn off for demos / servers

fn default_theme() -> Theme {
    Theme {
        bg_primary: (0.039, 0.039, 0.047, 1.0),       // #0a0a0c
        bg_secondary: (0.078, 0.078, 0.094, 1.0),     // #141418
        bg_tertiary: (0.145, 0.145, 0.188, 1.0),      // #252530
        bg_card: (0.102, 0.102, 0.133, 1.0),           // #1a1a22
        row_stripe: (0.0157, 0.0157, 0.0157, 1.0),     // #040404
        bg_modal: (0.0, 0.0, 0.0, 0.7),
        accent: (0.929, 0.549, 0.141, 1.0),            // #ED8C24
        accent_hover: (1.0, 0.65, 0.24, 1.0),
        accent_pressed: (0.8, 0.45, 0.10, 1.0),
        text_primary: (0.910, 0.910, 0.918, 1.0),     // #e8e8ea
        // v0.195.0: brighter secondary + muted for accessibility (see
        // matching values in `apply_dark_defaults`).
        text_secondary: (0.706, 0.706, 0.745, 1.0),   // #b4b4be
        text_muted: (0.580, 0.580, 0.624, 1.0),       // #94949f
        text_on_accent: (0.05, 0.05, 0.05, 1.0),
        success: (0.2, 0.75, 0.3, 1.0),
        warning: (0.95, 0.75, 0.1, 1.0),
        danger: (0.9, 0.25, 0.2, 1.0),
        info: (0.2, 0.5, 0.9, 1.0),
        orbit_line: default_orbit_line(),
        constellation_line: default_constellation_line(),
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
        button_padding_h: 8.0,
        button_pad_y: 3.0,
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
        avatar_size: 32.0,
        avatar_gap: 8.0,
        pill_radius: 9.0,
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
        stat_name_width: 86.0,
        stat_value_width: 82.0,
        status_bar_width: 200.0,
        status_bar_height: 5.0,
        compact_button_height: 18.0,
        cell_narrow_width: 60.0,
        cell_short_width: 90.0,
        cell_name_width: 150.0,
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
        // Studio source-type palette (v0.670)
        studio_source_camera: default_studio_source_camera(),
        studio_source_screen: default_studio_source_screen(),
        studio_source_microphone: default_studio_source_microphone(),
        studio_source_chat: default_studio_source_chat(),
        studio_source_image: default_studio_source_image(),
        studio_source_text: default_studio_source_text(),
        studio_source_timer: default_studio_source_timer(),
        studio_source_border: default_studio_source_border(),
        studio_source_label: default_studio_source_label(),
        studio_afk: default_studio_afk(),
        studio_meter_bg: default_studio_meter_bg(),
        max_messages: default_max_messages(),
        // Nav tokens (v0.175.0) — see default_nav_* helpers above.
        nav_reality: default_nav_reality(),
        nav_sim: default_nav_sim(),
        nav_tools: default_nav_tools(),
        nav_settings: default_nav_settings(),
        nav_dev: default_nav_dev(),
        nav_legacy_red: default_nav_legacy_red(),
        nav_legacy_green: default_nav_legacy_green(),
        nav_legacy_blue: default_nav_legacy_blue(),
        nav_separator_height: default_nav_separator_height(),
        nav_active_border_width: default_nav_active_border_width(),
        nav_hover_border_width: default_nav_hover_border_width(),
        animations_enabled: default_animations_enabled(),
        nav_separator_animation: default_nav_separator_anim(),
        nav_separator_animation_speed: default_anim_speed(),
        nav_active_border_animation: default_active_border_anim(),
        attack_indicator_style: default_attack_anim(),
        attack_indicator_speed: default_anim_speed(),
        nav_dev_visible: default_nav_dev_visible(),
        cheats_enabled: default_cheats_enabled(),
    }
}
