//! RGB-colored header nav bar for all tool pages.
//!
//! Mirrors the web shell.js color-coded navigation:
//! - Red group: identity pages (never change with context)
//! - Green group: context-sensitive pages (data changes with Real/Sim)
//! - Blue group: system/config pages
//! Plus a Real/Sim toggle on the right.

use egui::{Align, Color32, Frame, Layout, Rect, RichText, Rounding, Sense, Stroke, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};
use crate::gui::theme::Theme;
// Nav category colors / sizes used to live as `const` here. They moved to
// `data/gui/theme.ron` in v0.175.0 so the Settings page color editor can
// tune them. Read via `theme.nav_legacy_red()`, `theme.nav_reality()`, etc.

struct NavItem {
    label: &'static str,
    page: GuiPage,
    /// One-line description used by category overview landing pages
    /// (the new "all top buttons get a summary page" pattern in v0.181.0).
    /// Empty string means the page hides from overviews.
    description: &'static str,
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
}

/// Single-row RGB nav bar. Red identity / green contextual / blue
/// system groups, with each button now showing an icon next to its
/// label (v0.196.0). H brand button on the left navigates to Chat
/// (operator decision: "H stands for Humanity ... fitting" + chat is
/// the primary value prop). Real/Sim toggle on the right.
fn draw_nav_bar_one_tier(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Keep repainting so the RGB channeling animation stays smooth
    ctx.request_repaint();

    let text_muted = theme.text_muted();
    let border = theme.border();
    let attack_pulse = state.attack_pulse_active;

    egui::TopBottomPanel::top("escape_nav_bar")
        .frame(Frame::none().fill(theme.bg_card()).inner_margin(egui::Margin::symmetric(8, 4)))
        // Suppress the panel's default 1px gray separator line — the
        // rgb_separator below is the canonical visual divider. Without
        // this, the layout produces a stack of (active button border) +
        // (panel-bottom-stroke) + (rgb separator), which read as a
        // doubled line compared to other separators in the layout.
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // (v0.363) The separate brand "H" button was REMOVED — it
                // duplicated the Humanity tab (both opened GuiPage::Humanity) and
                // never got the active-tab RGB border the real tabs have. Humanity
                // is now a single NORMAL tab whose ICON is the "H"
                // (widgets::icons::paint_nav_icon) and whose label is "Humanity".
                // Operator 2026-06-04: "only have a single H/Humanity page ... the
                // icon can be the H and the text can be Humanity."

                // Page carve: six top-level tabs —
                // Humanity (the collective / mission), Chat (comms), Real (your
                // actual life), Play (the sim), Platform (the software). This
                // first step REGROUPS the existing pages under the five; the
                // next steps fold each group into one scrollable section_nav
                // page so the top row truly condenses. Decisions baked in: Maps
                // lives once under Real (toggleable Humanity layers come with
                // the merge); Recovery → Platform; Identity → Humanity (it's a
                // public directory lookup, not private); Civilization → the
                // Humanity Community Dashboard (what H now opens).

                // Play — the dedicated button that drops into the 3D first-person
                // game world (operator 2026-06-07: "add a dedicated button for the FPS
                // game part ... Click Play to start FPS game mode" + "move Play all the
                // way to the left"). Entering FPS mode = setting the page to None: the
                // nav bar + pages hide and the cursor is grabbed for mouse-look (the
                // post-egui reconcile in lib.rs); Esc brings the menu back. Leftmost,
                // so the way into the world is the very first thing in the nav.
                // Play enters the world (page None). With no default character it
                // opens the unified character picker (the showroom); with a default
                // set it skips straight into first-person. "Characters" ALWAYS opens
                // the picker (even when a default is set) so there is always a way
                // back to change homes/characters or clear the default -- otherwise
                // a default is a dead-end (operator, 2026-06-16). The nav_group click
                // handler branches on the label (v0.476.1).
                let play_items = [
                    NavItem { label: "Play", page: GuiPage::None, description: "" },
                    NavItem { label: "Characters", page: GuiPage::None, description: "Choose a home, character, or server. Always opens the picker." },
                ];
                nav_group(ui, &play_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Humanity — the collective + the mission, FOLDED into one tab.
                // Its sections (Mission Dashboard/Governance/Directory/Onboarding/
                // Donate/Resources) live in the Humanity page's section_nav. This
                // is what the H button opens.
                let humanity_items = [
                    NavItem { label: "Humanity", page: GuiPage::Humanity, description: "" },
                ];
                nav_group(ui, &humanity_items, theme.nav_reality(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Chat — communication.
                let chat_items = [
                    NavItem { label: "Chat", page: GuiPage::Chat, description: "" },
                ];
                nav_group(ui, &chat_items, theme.nav_legacy_red(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Studio — livestreaming, promoted to its own top-level tab right of
                // Chat (operator 2026-06-06: "move the studio page to the top level
                // main menu to the right of the chat button").
                let studio_items = [
                    NavItem { label: "Studio", page: GuiPage::Studio, description: "" },
                ];
                nav_group(ui, &studio_items, theme.nav_legacy_red(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Profile — your character / identity (operator 2026-06-07: "rename
                // real to profile"). Renamed from "Real"; still GuiPage::Real (the
                // folded page). After this batch its sidebar holds the Profile sections
                // + Wallet + Market only — Possessions/Tasks/Map became their own
                // top-level tabs and Streaming moved into Studio. A profile selector
                // sits at the top (one base character for now; servers store augmented
                // versions — see docs/design/homes-as-profiles.md).
                let profile_items = [
                    NavItem { label: "Profile", page: GuiPage::Real, description: "" },
                ];
                nav_group(ui, &profile_items, theme.nav_legacy_green(), text_muted, theme, state);

                // Home — your offline homestead (operator 2026-06-07: "let's do the
                // homes thing ... keep developing offline"). Surfaces the Fibonacci
                // homestead Design (rooms + bill-of-materials + power/water demand +
                // a self-sufficiency summary). Sits by Profile (you -> your home).
                let home_items = [
                    NavItem { label: "Home", page: GuiPage::Homes, description: "" },
                ];
                nav_group(ui, &home_items, theme.nav_legacy_green(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Quests — the learn-by-doing self-sufficiency path, its own
                // top-level tab (operator 2026-06-06: "add a top level quests page
                // for now").
                let quests_items = [
                    NavItem { label: "Quests", page: GuiPage::Quests, description: "" },
                ];
                nav_group(ui, &quests_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Tasks — promoted to its OWN top-level tab, to the right of Quests
                // (operator 2026-06-07: "move tasks to the right of quests"). Was a
                // section inside the old Real tab.
                let tasks_items = [
                    NavItem { label: "Tasks", page: GuiPage::Tasks, description: "" },
                ];
                nav_group(ui, &tasks_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Inventory — your possessions, promoted to its OWN top-level tab
                // (operator 2026-06-07: "the inventory page definitely need to become
                // a top level page"). Previously reachable only as Real's "Possessions"
                // section; now one click from the nav like every other core surface.
                let inventory_items = [
                    NavItem { label: "Inventory", page: GuiPage::Inventory, description: "" },
                ];
                nav_group(ui, &inventory_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Crafting — its OWN top-level tab (operator 2026-06-07: "let's have
                // the crafting page its own page for now at least" + "add crafting to
                // the main menu"). This replaces the retired "Play" fold, which held
                // only Crafting after Studio was promoted — so Play collapses into
                // this direct tab rather than a one-item sidebar.
                let crafting_items = [
                    NavItem { label: "Crafting", page: GuiPage::Crafting, description: "" },
                ];
                nav_group(ui, &crafting_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Map — the universal Cosmos map, promoted to its OWN top-level tab
                // (operator 2026-06-07: "move Map to the top menu as well"). Was a
                // section inside the old Real tab; GuiPage::Maps forwards to the Cosmos
                // page (the real map). Closes the game-colored cluster.
                let map_items = [
                    NavItem { label: "Map", page: GuiPage::Maps, description: "" },
                ];
                nav_group(ui, &map_items, theme.nav_sim(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Platform — system + dev, FOLDED (Recovery/Tools/Bugs/Testing/
                // Browser in its sidebar). Settings was pulled OUT to its own tab
                // (below) so it's never buried.
                let platform_items = [
                    NavItem { label: "Platform", page: GuiPage::Platform, description: "" },
                ];
                nav_group(ui, &platform_items, theme.nav_tools(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Settings — its OWN top-level tab (operator 2026-06-04: "have
                // settings as its own top level page ... always easily accessible,
                // never buried in another menu"). Big enough to warrant its own
                // page (section sidebar + long scroll), but it stays ONE click
                // away here, not nested inside Platform.
                // Library — the Humanity Accord and its reference companions, in a
                // nested tree (operator 2026-06-06: "a top level Library button that
                // contains all the docs sorted into nested categories").
                let library_items = [
                    NavItem { label: "Library", page: GuiPage::Library, description: "" },
                ];
                nav_group(ui, &library_items, theme.nav_reality(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                let settings_items = [
                    NavItem { label: "Settings", page: GuiPage::Settings, description: "" },
                ];
                nav_group(ui, &settings_items, theme.nav_settings(), text_muted, theme, state);
                // v0.479: the "Game Admin" nav button was removed -- game-world
                // bans are now a subsection of Server Settings > ADMIN.
            });
        });

    // RGB channeling separator below the nav — matches the two-tier
    // layout's separators so the design language is consistent.
    rgb_separator(ctx, theme, "nav_one_tier_sep", attack_pulse);
}


/// Convert HSV to RGB Color32. theme-exempt: pure math helper.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
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
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
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
            ui.painter().rect_filled(rect, Rounding::ZERO, color);
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
        let label_w = galley.size().x;
        let total_w = PAD_X + ICON_W + ICON_LABEL_GAP + label_w + PAD_X;

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
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60))
        };

        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(6), bg_fill);
        painter.rect_stroke(rect, Rounding::same(6), border_stroke, egui::StrokeKind::Inside);

        // Paint the icon centered vertically in a 14×14 box at the left.
        let icon_rect = Rect::from_min_size(
            egui::pos2(rect.left() + PAD_X, rect.center().y - ICON_W * 0.5),
            Vec2::splat(ICON_W),
        );
        // Icon color tracks text — active = white, otherwise muted —
        // so the icon reads as part of the label, not a separate element.
        let _has_icon = crate::gui::widgets::icons::paint_nav_icon(
            painter, icon_rect, item.page.clone(), text_color,
        );

        // Paint the label to the right of the icon, vertically centered.
        let label_pos = egui::pos2(
            rect.left() + PAD_X + ICON_W + ICON_LABEL_GAP,
            rect.center().y - galley.size().y * 0.5,
        );
        painter.galley(label_pos, galley, text_color);

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
                // "Characters" ALWAYS opens the picker (the way back when a default
                // is set). "Play" honors the default: with one set it skips the
                // picker and loads that character straight into first-person; with
                // none it opens the picker too. (The showroom is opened by the
                // per-frame handler in lib.rs when launcher_open_select is set.)
                let force_picker = item.label == "Characters";
                if force_picker || state.launcher_default_character.is_empty() {
                    state.launcher_open_select = true;
                    state.launcher_saves_loaded = false; // refresh the save list
                } else {
                    state.launcher_pending_load = Some(state.launcher_default_character.clone());
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
