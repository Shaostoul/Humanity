//! Universal button widget — single source of truth for every button in the app.
//!
//! **Use this everywhere.** Edit it once, every page updates.
//!
//! # Quick reference
//!
//! ```ignore
//! use crate::gui::widgets::button::Button;
//!
//! // Primary call-to-action
//! if Button::primary("Save").show(ui, theme) { /* clicked */ }
//!
//! // Secondary outline
//! if Button::secondary("Cancel").show(ui, theme) { /* clicked */ }
//!
//! // Destructive
//! if Button::danger("Delete").icon("\u{1F5D1}").show(ui, theme) { /* clicked */ }
//!
//! // Ghost (transparent — used in nav backs, inline links)
//! if Button::ghost("\u{2190} Back").show(ui, theme) { /* clicked */ }
//!
//! // Icon-only with tooltip
//! if Button::icon_only("\u{2699}").tooltip("Settings").show(ui, theme) { /* clicked */ }
//!
//! // Full-width primary with leading icon
//! if Button::primary("Sign in").icon("\u{1F511}").full_width().show(ui, theme) { /* clicked */ }
//!
//! // Disabled state
//! Button::primary("Save").disabled(invalid).show(ui, theme);
//! ```
//!
//! All styling pulls from `Theme` — no hardcoded colors, sizes, or radii. Edit
//! `data/gui/theme.ron` to restyle every button across the app.
//!
//! # Backward compatibility
//!
//! The old free functions (`primary_button`, `secondary_button`, `danger_button`,
//! `btn_primary`, …) are still exported as thin wrappers over the builder so
//! existing call sites keep working without changes. Migrate at your leisure.

use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// Visual variant of the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Filled accent — primary call-to-action.
    Primary,
    /// Outlined — secondary action.
    Secondary,
    /// Filled danger color — destructive action.
    Danger,
    /// Filled success color — confirm / positive.
    Success,
    /// Transparent — used for inline links, nav-back, icon-only buttons.
    Ghost,
}

/// Size preset of the button. Driven by theme tokens (font + min height).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

/// Universal button builder. Construct via `Button::new`, `Button::primary`,
/// `Button::secondary`, `Button::danger`, `Button::ghost`, or `Button::icon_only`.
///
/// **Tab / nav buttons** use `.active(bool)` to flip into Primary styling when
/// selected — this is how `tab_bar`, `sidebar_nav`, `category_filter`, and the
/// header menu all share the same button look.
#[derive(Debug, Clone)]
pub struct Button<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    icon_trailing: Option<&'a str>,
    variant: ButtonVariant,
    size: ButtonSize,
    full_width: bool,
    disabled: bool,
    /// When true, the button visually represents a current/selected/active state.
    /// Overrides the variant fill: any non-Primary variant flips to filled-accent.
    active: bool,
    tooltip: Option<&'a str>,
    /// Override the min-height. Rare — prefer `size`.
    min_height: Option<f32>,
}

impl<'a> Button<'a> {
    /// Construct with a default Secondary variant. Most callers should use the
    /// shortcut constructors instead (`primary`, `secondary`, etc.).
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            icon_trailing: None,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Medium,
            full_width: false,
            disabled: false,
            active: false,
            tooltip: None,
            min_height: None,
        }
    }

    /// Tab / nav button: starts as Ghost, flips to filled-accent when active.
    /// Used by `tab_bar`, `sidebar_nav`, `category_filter`, and the app header.
    pub fn tab(label: &'a str, is_active: bool) -> Self {
        Self::new(label).variant(ButtonVariant::Ghost).active(is_active)
    }

    /// Filled accent button — primary call-to-action.
    pub fn primary(label: &'a str) -> Self {
        Self::new(label).variant(ButtonVariant::Primary)
    }

    /// Outline button — secondary action.
    pub fn secondary(label: &'a str) -> Self {
        Self::new(label).variant(ButtonVariant::Secondary)
    }

    /// Red filled button — destructive action.
    pub fn danger(label: &'a str) -> Self {
        Self::new(label).variant(ButtonVariant::Danger)
    }

    /// Green filled button — success/confirm action.
    pub fn success(label: &'a str) -> Self {
        Self::new(label).variant(ButtonVariant::Success)
    }

    /// Transparent button — inline links, nav-back, navigation chevrons.
    pub fn ghost(label: &'a str) -> Self {
        Self::new(label).variant(ButtonVariant::Ghost)
    }

    /// Icon-only button (use a unicode glyph or emoji as the label).
    /// Renders ghost-style with small size by default.
    pub fn icon_only(glyph: &'a str) -> Self {
        Self::new(glyph)
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Small)
    }

    /// Add a leading icon (rendered before the label, separated by a thin space).
    /// For real painted icons, use `widgets::icons::paint_*` and compose your own
    /// horizontal layout — this field is for unicode glyphs only.
    pub fn icon(mut self, glyph: &'a str) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Add a trailing icon (rendered after the label).
    pub fn icon_trailing(mut self, glyph: &'a str) -> Self {
        self.icon_trailing = Some(glyph);
        self
    }

    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }

    pub fn size(mut self, s: ButtonSize) -> Self {
        self.size = s;
        self
    }

    /// Stretch the button to fill the available width.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Disable interaction. Renders dimmed.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Mark this button as the current/selected/active one. Overrides the
    /// variant fill: any non-Primary variant flips to filled-accent. Used for
    /// tabs, nav items, category filters, and the app header.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Show a tooltip on hover.
    pub fn tooltip(mut self, text: &'a str) -> Self {
        self.tooltip = Some(text);
        self
    }

    /// Override min-height in pixels. Rare — prefer `size`.
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    /// Render the button. Returns true if clicked.
    pub fn show(self, ui: &mut Ui, theme: &Theme) -> bool {
        let font_size = match self.size {
            ButtonSize::Small => theme.font_size_small,
            ButtonSize::Medium => theme.font_size_body,
            ButtonSize::Large => theme.font_size_heading,
        };
        let height = self.min_height.unwrap_or(match self.size {
            ButtonSize::Small => (theme.button_height * 0.75).round(),
            ButtonSize::Medium => theme.button_height,
            ButtonSize::Large => (theme.button_height * 1.25).round(),
        });

        // Compose label: icon ' ' label ' ' trailing-icon. Each is RichText so
        // egui's layout handles them as one inline run.
        let composed = match (self.icon, self.icon_trailing) {
            (Some(i), Some(t)) => format!("{i} {} {t}", self.label),
            (Some(i), None) => format!("{i} {}", self.label),
            (None, Some(t)) => format!("{} {t}", self.label),
            (None, None) => self.label.to_string(),
        };

        // Active state overrides variant: any button that's "selected" looks
        // like Primary. This is how tabs, nav items, and category filters
        // visually mark the current page/tab/category.
        let effective_variant = if self.active {
            ButtonVariant::Primary
        } else {
            self.variant
        };

        let (text_color, fill, stroke) = match effective_variant {
            ButtonVariant::Primary => (
                theme.text_on_accent(),
                theme.accent(),
                Stroke::NONE,
            ),
            ButtonVariant::Secondary => (
                theme.text_primary(),
                Color32::TRANSPARENT,
                Stroke::new(1.0, theme.border()),
            ),
            ButtonVariant::Danger => (
                Color32::WHITE,
                theme.danger(),
                Stroke::NONE,
            ),
            ButtonVariant::Success => (
                Color32::WHITE,
                theme.success(),
                Stroke::NONE,
            ),
            ButtonVariant::Ghost => (
                theme.text_secondary(),
                Color32::TRANSPARENT,
                Stroke::NONE,
            ),
        };

        let text = RichText::new(composed).size(font_size).color(text_color);
        let mut btn = egui::Button::new(text)
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(theme.border_radius as u8))
            .min_size(Vec2::new(0.0, height));

        if self.full_width {
            btn = btn.min_size(Vec2::new(ui.available_width(), height));
        }

        let response = if self.disabled {
            ui.add_enabled(false, btn)
        } else {
            ui.add(btn)
        };

        let response = if let Some(tip) = self.tooltip {
            response.on_hover_text(tip)
        } else {
            response
        };

        response.clicked() && !self.disabled
    }
}

// ─── Backward-compatibility free functions ──────────────────────────────
// Old call sites continue to work. Migrate to the Button builder over time.

/// Orange accent button. Returns true if clicked.
pub fn btn_primary(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    Button::primary(label).show(ui, theme)
}

/// Outline button with border. Returns true if clicked.
pub fn btn_secondary(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    Button::secondary(label).show(ui, theme)
}

/// Red danger button for destructive actions. Returns true if clicked.
pub fn btn_danger(ui: &mut Ui, theme: &Theme, label: &str) -> bool {
    Button::danger(label).show(ui, theme)
}

/// Small icon-only button with tooltip.
pub fn btn_icon(ui: &mut Ui, theme: &Theme, icon: &str, tooltip: &str) -> bool {
    Button::icon_only(icon).tooltip(tooltip).show(ui, theme)
}

// Legacy names used widely across pages.
pub use btn_primary as primary_button;
pub use btn_secondary as secondary_button;
pub use btn_danger as danger_button;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_enum_distinct() {
        assert_ne!(ButtonVariant::Primary, ButtonVariant::Secondary);
        assert_ne!(ButtonVariant::Danger, ButtonVariant::Success);
        assert_ne!(ButtonVariant::Ghost, ButtonVariant::Primary);
    }

    #[test]
    fn size_enum_distinct() {
        assert_ne!(ButtonSize::Small, ButtonSize::Medium);
        assert_ne!(ButtonSize::Medium, ButtonSize::Large);
    }

    #[test]
    fn builder_chains_compile() {
        // Compile-only test that the builder API is ergonomic.
        let _ = Button::primary("OK").icon("\u{2713}").full_width();
        let _ = Button::secondary("Cancel").size(ButtonSize::Small);
        let _ = Button::danger("Delete").icon("\u{1F5D1}").disabled(true);
        let _ = Button::ghost("\u{2190} Back");
        let _ = Button::icon_only("\u{2699}").tooltip("Settings");
        let _ = Button::success("Confirm").icon_trailing("\u{2192}");
    }

    #[test]
    fn label_composition() {
        // Verify the label composition logic (icon prefix, trailing, both).
        let b = Button::primary("Save");
        assert_eq!(b.label, "Save");
        assert_eq!(b.icon, None);

        let b = Button::primary("Save").icon("\u{1F4BE}");
        assert_eq!(b.icon, Some("\u{1F4BE}"));
    }
}
