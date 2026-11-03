//! RGB-colored header nav bar for all tool pages.
//!
//! THE ROW IS DATA since 2026-11-03 (docs/design/navigation.md, rung 1): its buttons, their
//! order, their words, their groups' colours and the page each opens are `today_row.desktop` in
//! data/gui/navigation.json (gui/navigation.rs reads it), the same file the website's top menu is
//! drawn from (web/shared/shell.js, through scripts/gen-navigation.js). A button is added, moved
//! or renamed there, never here. This file draws what the file says: the groups with a dot
//! between them, each button's icon (its page's, widgets::icons::nav_icon) and words, the Chat
//! button's unread dot, then the Aa switch; and what a click does (Play and Characters go to the
//! world, every other button to its page).

use egui::{Color32, Frame, Rect, RichText, CornerRadius, Stroke, Vec2};
use crate::gui::{GuiPage, GuiState};
use crate::gui::theme::Theme;
// Nav category colors / sizes used to live as `const` here. They moved to
// `data/gui/theme.ron` in v0.175.0 so the Settings page color editor can
// tune them. Read via `theme.nav_legacy_red()`, `theme.nav_reality()`, etc.

/// One button of the row, as the file says it (gui/navigation.rs `RowButton`), its words read.
struct NavItem {
    label: &'static str,
    page: GuiPage,
    /// Characters: always the picker, even when a last pairing is recorded.
    picker: bool,
}

/// Draw the RGB header nav bar at the top of the screen.
///
/// Single-row layout — red identity / green contextual / blue system
/// groups + Real/Sim toggle on the right. v0.196.0: the experimental
/// two-tier preview layout was removed entirely (operator 2026-05-08:
/// "I really want to not need so many pages... The single row main menu
/// is cleaner."). nav_two_tier field on GuiState is preserved for
/// backwards-compatible config deserialization but ignored.
pub fn draw_nav_bar(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    draw_nav_bar_one_tier(ctx, theme, state);
    // The notices bell (notifications rung 2), anchored beside the help button: every page with
    // this header has it (gui/notice_panel.rs).
    crate::gui::notice_panel::draw(ctx, theme, state);
}

/// The single row: the groups of data/gui/navigation.json's `today_row.desktop`, each button an
/// icon beside its words (v0.196.0), then the Aa switch. (Its older notes, a Real/Sim toggle on the
/// right and an H button opening Chat, are long gone: the Real/Sim switch went in v0.197.0, and
/// the H is the Humanity button's icon since v0.363.)
fn draw_nav_bar_one_tier(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Keep repainting so the RGB channeling animation stays smooth
    ctx.request_repaint();

    let text_muted = theme.text_muted();
    let border = theme.border();
    let attack_pulse = state.attack_pulse_active;

    egui::TopBottomPanel::top("escape_nav_bar")
        .frame(Frame::NONE.fill(theme.bg_card()).inner_margin(egui::Margin::symmetric(8, 4)))
        // Suppress the panel's default 1px gray separator line — the
        // rgb_separator below is the canonical visual divider. Without
        // this, the layout produces a stack of (active button border) +
        // (panel-bottom-stroke) + (rgb separator), which read as a
        // doubled line compared to other separators in the layout.
        .show_separator_line(false)
        .show(ctx, |ui| {
            // Keep the top-right corner clear for the always-in-place help toggle
            // (pages/keymap.rs::draw_help_toggle, v0.1212). That button is anchored to
            // the window, not laid out in this row, so without reserving its width the
            // wrapped nav would run a tab underneath it. Reserved unconditionally so
            // the nav layout never shifts as the help panel opens and closes.
            // NOTE: `ui.set_max_width` here does NOT work - it leaves the wrapped row
            // exactly as wide as before (proved with an absurd 326 px reserve that moved
            // nothing). The child ui has to be given the narrower width when it is
            // ALLOCATED, which is what allocate_ui does.
            // Since notifications rung 2 (2026-10-31) the notices bell sits beside it, anchored
            // the same way (gui/notice_panel.rs), so the reserve holds both.
            let help_reserve = crate::gui::pages::keymap::HELP_TOGGLE_SIZE
                + crate::gui::notice_panel::BELL_SIZE
                + crate::gui::notice_panel::BELL_GAP
                + 10.0;
            let nav_width = (ui.available_width() - help_reserve).max(120.0);
            ui.allocate_ui(Vec2::new(nav_width, 0.0), |ui| {
            // horizontal_WRAPPED (v0.859): buttons that overflow the window width wrap
            // into a second row instead of being clipped off the right edge, matching
            // the web header. The TopBottomPanel auto-grows to fit the extra rows.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // THE ROW, from data/gui/navigation.json's today_row.desktop (rung 1 of
                // docs/design/navigation.md): each group in its colour, a dot between two groups.
                // How each button came to be where it is, in the operator's words: Play leftmost
                // and into the world (2026-06-07, "move Play all the way to the left"; it replays
                // the last pairing, Characters always opens the picker, 2026-06-16), one Humanity
                // button whose icon is the H (2026-06-04), Studio beside Chat (2026-06-06), Profile
                // renamed from Real (2026-06-07), Quests, Tasks, Inventory, Crafting and Maps each
                // a tab of its own (2026-06-06/07), Settings never buried (2026-06-04), the Library
                // with How-to beside it (2026-06-06, 2026-10-23), Dev in place of Platform
                // (2026-10-20), Apps beside Play (2026-10-19). The Servers tab went to the chat's
                // sidebar (2026-08-14) and Game Admin into Server Settings (v0.479); git history
                // of this file has the long versions.
                let menu = crate::gui::navigation::menu();
                for (gi, group) in menu.desktop_row.iter().enumerate() {
                    if gi > 0 {
                        ui.add_space(6.0);
                        separator_dot(ui, border);
                        ui.add_space(6.0);
                    }
                    let colour = crate::gui::navigation::colour_of(&group.colour).map_or(text_muted, |c| c(theme));
                    let items: Vec<NavItem> = group
                        .buttons
                        .iter()
                        .map(|b| NavItem { label: humanity_core::i18n::t_dyn(&b.label), page: b.page, picker: b.picker })
                        .collect();
                    nav_group(ui, &items, colour, text_muted, theme, state);
                }

                // Presentation-mode cycle (v0.859): icon+text / icon-only / text-only,
                // mirroring the web header's Aa button. Sits at the end of the row.
                ui.add_space(8.0);
                let mode_resp = ui.add(
                    egui::Button::new(
                        RichText::new(state.nav_display_mode.glyph())
                            .size(11.0)
                            .color(text_muted),
                    )
                    .fill(theme.bg_secondary())
                    .min_size(Vec2::new(26.0, 26.0)),
                );
                if mode_resp
                    .on_hover_text(
                        "How header buttons show: icon + text, icon only, or text only.",
                    )
                    .clicked()
                {
                    state.nav_display_mode = state.nav_display_mode.next();
                    // Persist immediately (v0.1066). Web has remembered this
                    // since v0.859; native was cycling in-memory only, so the
                    // choice was silently lost on every restart.
                    crate::config::AppConfig::from_gui_state(state).save();
                }
            });
            }); // allocate_ui: the width-reserved region for the help toggle
        });

    // RGB channeling separator below the nav — matches the two-tier
    // layout's separators so the design language is consistent.
    rgb_separator(ctx, theme, "nav_one_tier_sep", attack_pulse);
}


/// Convert HSV to RGB Color32. theme-exempt: pure math helper.
pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Active-element "channeling" color, theme-aware (v0.177.0).
///
/// Reads `theme.nav_active_border_animation` (RGB_CYCLE / SOLID / PULSE / OFF)
/// and `theme.attack_indicator_style` (PULSE_RED / PULSE_YELLOW / FLASH_WHITE
/// / BORDER_ONLY / NONE) plus `animations_enabled` master switch + the speed
/// multipliers. `fallback` is the contextual color used in SOLID mode (e.g.
/// the active category's tinted color) and as the BORDER_ONLY attack color.
///
/// Shared by active nav buttons + the RGB separator so all attention-grabbing
/// elements animate in lockstep.
pub fn channeling_color(
    theme: &Theme,
    time: f32,
    attack_pulse: bool,
    fallback: Color32,
) -> Color32 {
    use crate::gui::theme::{anim, attack as atk};

    if !theme.animations_enabled {
        // Hold a static color when animations are disabled. For attack
        // mode pick the danger color; otherwise use the fallback.
        return if attack_pulse { theme.danger() } else { fallback };
    }

    if attack_pulse {
        let t_speed = (time * 4.0 * theme.attack_indicator_speed).max(0.0);
        match theme.attack_indicator_style {
            atk::NONE => {} // fall through to non-attack path
            atk::PULSE_RED => {
                let t = t_speed.sin() * 0.5 + 0.5;
                let v = 0.55 + 0.45 * t;
                // theme-exempt: programmatic pulse from theme tokens.
                return Color32::from_rgb((v * 255.0) as u8, 0, 0);
            }
            atk::PULSE_YELLOW => {
                let t = t_speed.sin() * 0.5 + 0.5;
                let v = 0.55 + 0.45 * t;
                // theme-exempt: programmatic pulse from theme tokens.
                return Color32::from_rgb((v * 255.0) as u8, (v * 220.0) as u8, 0);
            }
            atk::FLASH_WHITE => {
                // Square-ish blink at ~6 Hz.
                let t = ((time * 12.0 * theme.attack_indicator_speed).sin() > 0.0) as u8;
                return if t == 1 { Color32::WHITE } else { theme.danger() };
            }
            atk::BORDER_ONLY => return theme.danger(),
            _ => {}
        }
    }

    // Non-attack path: separator/active-border animation style.
    let speed = theme.nav_separator_animation_speed.max(0.0);
    match theme.nav_active_border_animation {
        anim::OFF => fallback,
        anim::SOLID => fallback,
        anim::RGB_CYCLE => {
            let hue = (time * 0.3 * speed) % 1.0;
            hsv_to_rgb(hue.rem_euclid(1.0), 0.9, 1.0)
        }
        anim::PULSE => {
            let t = (time * 2.0 * speed).sin() * 0.5 + 0.5;
            let v = 0.4 + 0.6 * t;
            // theme-exempt: brightness modulation of fallback color.
            Color32::from_rgb(
                (fallback.r() as f32 * v) as u8,
                (fallback.g() as f32 * v) as u8,
                (fallback.b() as f32 * v) as u8,
            )
        }
        _ => fallback,
    }
}

/// Thin horizontal separator panel between nav tiers. Honors
/// `theme.nav_separator_animation` (OFF / SOLID / RGB_CYCLE / PULSE)
/// and `animations_enabled` master switch. Returns early without
/// allocating panel space when style == OFF.
fn rgb_separator(ctx: &egui::Context, theme: &Theme, panel_id: &'static str, attack_pulse: bool) {
    use crate::gui::theme::anim;
    if theme.nav_separator_animation == anim::OFF { return; }

    egui::TopBottomPanel::top(panel_id)
        .frame(Frame::NONE.fill(theme.bg_primary()).inner_margin(0.0))
        .exact_height(theme.nav_separator_height)
        .show_separator_line(false)
        .show(ctx, |ui| {
            let time = ui.ctx().input(|i| i.time) as f32;
            // Separator's "fallback" solid color is the accent — that's the
            // single color used when SOLID is selected or animations off.
            let fallback = theme.accent();
            let color = if theme.animations_enabled
                && theme.nav_separator_animation != anim::SOLID
            {
                separator_color(theme, time, attack_pulse, fallback)
            } else if attack_pulse && theme.animations_enabled {
                // Honor attack indicator even in SOLID mode for safety.
                channeling_color(theme, time, attack_pulse, fallback)
            } else {
                fallback
            };
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
        });
}

/// Like `channeling_color` but reads `nav_separator_animation` instead of
/// `nav_active_border_animation`. Lets the separator pick a different style
/// from the active button border (e.g. solid separator + RGB borders).
fn separator_color(theme: &Theme, time: f32, attack_pulse: bool, fallback: Color32) -> Color32 {
    use crate::gui::theme::anim;
    if attack_pulse {
        return channeling_color(theme, time, attack_pulse, fallback);
    }
    let speed = theme.nav_separator_animation_speed.max(0.0);
    match theme.nav_separator_animation {
        anim::OFF => fallback,
        anim::SOLID => fallback,
        anim::RGB_CYCLE => {
            let hue = (time * 0.3 * speed) % 1.0;
            hsv_to_rgb(hue.rem_euclid(1.0), 0.9, 1.0)
        }
        anim::PULSE => {
            let t = (time * 2.0 * speed).sin() * 0.5 + 0.5;
            let v = 0.4 + 0.6 * t;
            // theme-exempt: brightness modulation of fallback color.
            Color32::from_rgb(
                (fallback.r() as f32 * v) as u8,
                (fallback.g() as f32 * v) as u8,
                (fallback.b() as f32 * v) as u8,
            )
        }
        _ => fallback,
    }
}

/// Draw a group of nav buttons with border-based visual language.
///
/// Border states (from ops.html Color Reference):
/// - Default: thin 1px border in group color at low opacity
/// - Hover: blue 2px border glow
/// - Active (current page): animated RGB border cycling through hue spectrum
///
/// Group color subtly tints the button background.
fn nav_group(ui: &mut egui::Ui, items: &[NavItem], color: Color32, text_muted: Color32, theme: &Theme, state: &mut GuiState) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let attack_pulse = state.attack_pulse_active;

    // v0.196.0: each nav button now carries an icon alongside its label.
    // Operator 2026-05-08: "All the main menu buttons should have icons."
    // Icons come from widgets::icons::paint_nav_icon (router that maps
    // GuiPage → painter call). We allocate the button rect manually so
    // we have a Painter to draw the icon + position the label, instead
    // of going through egui::Button which only takes a single galley.

    const ICON_W: f32 = 14.0;
    const PAD_X: f32 = 8.0;
    const ICON_LABEL_GAP: f32 = 5.0;
    const BUTTON_H: f32 = 28.0;

    // Presentation mode (v0.859): icon+text / icon-only / text-only.
    let mode = state.nav_display_mode;
    let show_icon = mode.show_icon();
    let show_label = mode.show_label();

    for item in items {
        let is_active = std::mem::discriminant(&state.active_page)
            == std::mem::discriminant(&item.page);

        let text_color = if is_active { Color32::WHITE } else { text_muted };

        // Measure the label so we can size the button to fit icon + gap + text + padding.
        let galley = ui.painter().layout_no_wrap(
            item.label.to_string(),
            egui::FontId::proportional(11.0),
            text_color,
        );
        let label_w = if show_label { galley.size().x } else { 0.0 };
        // Width depends on which parts show. In icon-only mode there is no label or
        // gap; in text-only mode no icon or gap.
        let icon_part = if show_icon { ICON_W } else { 0.0 };
        let gap_part = if show_icon && show_label { ICON_LABEL_GAP } else { 0.0 };
        let total_w = PAD_X + icon_part + gap_part + label_w + PAD_X;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(total_w, BUTTON_H),
            egui::Sense::click(),
        );

        // Subtle group-color tinted background.
        let bg_fill = if is_active {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30)
        } else if response.hovered() {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18)
        } else {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 10)
        };

        // Border: active = animated channeling color. Inactive = thin tint.
        let border_stroke = if is_active {
            Stroke::new(
                theme.nav_active_border_width,
                channeling_color(theme, time, attack_pulse, color),
            )
        } else if response.hovered() {
            Stroke::new(theme.nav_hover_border_width, theme.nav_legacy_blue())
        } else {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60))
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(6), bg_fill);
        painter.rect_stroke(rect, CornerRadius::same(6), border_stroke, egui::StrokeKind::Inside);

        // Paint the icon centered vertically in a 14×14 box at the left (unless
        // text-only mode hides it).
        if show_icon {
            let icon_rect = Rect::from_min_size(
                egui::pos2(rect.left() + PAD_X, rect.center().y - ICON_W * 0.5),
                Vec2::splat(ICON_W),
            );
            // Icon color tracks text — active = white, otherwise muted —
            // so the icon reads as part of the label, not a separate element.
            let _has_icon = crate::gui::widgets::icons::paint_nav_icon(
                painter, icon_rect, item.page.clone(), text_color,
            );
        }

        // Paint the label after the icon (or at the left padding when there is no
        // icon), vertically centered. Hidden entirely in icon-only mode.
        if show_label {
            let label_x = if show_icon {
                rect.left() + PAD_X + ICON_W + ICON_LABEL_GAP
            } else {
                rect.left() + PAD_X
            };
            let label_pos = egui::pos2(label_x, rect.center().y - galley.size().y * 0.5);
            painter.galley(label_pos, galley, text_color);
        }

        // Unread indicator on the Chat tab: a small dot at the button's
        // top-right corner whenever any DM / group / channel holds unread
        // messages, so chat activity is visible from EVERY page — the
        // sidebar dots (v0.715-718) only help while Chat is open. (v0.719)
        if item.page == GuiPage::Chat {
            let any_unread = state.chat_dms.iter().any(|d| d.unread)
                || state.chat_channels.iter().any(|c| c.unread);
            if any_unread {
                painter.circle_filled(
                    egui::pos2(rect.right() - 5.0, rect.top() + 5.0),
                    3.0,
                    theme.danger(),
                );
            }
        }

        if response.clicked() {
            // Top-tier nav clicks are LATERAL — clear the back stack
            // so Esc on the new page goes straight to FPS instead of
            // through stale contextual flow entries.
            state.clear_nav_back();
            if item.page == GuiPage::None {
                // "Play" and "Characters" both enter the world (page None). Remember
                // where we came from so Esc out returns HERE, not a stale last_page.
                if state.active_page != GuiPage::None {
                    state.last_page = state.active_page;
                }
                // "Characters" ALWAYS opens the picker (the way back to change who
                // or where): its entry in data/gui/navigation.json opens the world's
                // `picker` part. "Play" replays the last pairing straight into
                // first-person; with none recorded it opens the picker too.
                // apply_last_pairing restores the selection, asks for the character
                // load, and sets solo/shared from the world kind. (The showroom is
                // opened by the per-frame handler in lib.rs when launcher_open_select
                // is set.)
                let force_picker = item.picker;
                if force_picker || !state.apply_last_pairing() {
                    state.launcher_open_select = true;
                    state.launcher_homes_loaded = false; // rescan the saves
                }
                state.active_page = GuiPage::None;
            } else {
                state.active_page = item.page.clone();
            }
        }
    }
}

/// Small dot separator between nav groups.
fn separator_dot(ui: &mut egui::Ui, color: Color32) {
    ui.label(RichText::new("·").size(14.0).color(color));
}

// ─────────────────────────────────────────────────────────────────────────
// Two-tier nav preview (v0.164.0)
//
// Top tier: 4 wide category tabs (Reality / Sim / Tools / Settings).
// Sub tier: pages within the selected top category.
// Both tiers honor the existing theme tokens. The Reality tier uses red,
// Sim purple, Tools blue, Settings gray. A `[≡]` button on the right of
// the top tier flips back to the legacy single-row nav.
//
// Replaces the Real/Sim pill + single-row nav. Reality and Sim are now
// separate top categories so users don't accidentally cross contexts
// (e.g. spending real crypto when they meant in-game tokens).
// ─────────────────────────────────────────────────────────────────────────

// v0.699.0: removed TopCategory + top_categories() + category_meta() +
// category_pages() + sub_pages_for(). These fed the category-overview landing
// pages (the two-tier-nav-era "browse this category" grid), which were deleted
// this release as unreachable dead pages. The single-row nav (built directly
// from NavItem lists above) never used them.

// v0.196.0: removed `draw_nav_bar_two_tier` (single-row nav is cleaner).
// v0.699.0: removed the category-browse helpers that outlived it
// (category_pages / category_meta / sub_pages_for) once their only consumer,
// the category-overview landing pages, was deleted. The `state.nav_two_tier`
// and `state.nav_top_category` fields stay on GuiState for backwards-compatible
// config deserialization but the single-row layout ignores them.
