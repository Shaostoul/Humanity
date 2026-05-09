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

    let accent = theme.accent();
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

                // Brand H — navigates to Chat. Operator 2026-05-08:
                // "the H stands for Humanity... If we make the H button
                // on the main header menu chat then it's kinda fitting."
                // Chat IS the cooperative platform's primary value prop;
                // dashboard would be a maintenance burden for low payoff.
                let brand = ui.add(
                    egui::Button::new(RichText::new("H").size(14.0).strong().color(accent))
                        .min_size(Vec2::new(28.0, 28.0))
                        .rounding(Rounding::same(6)),
                ).on_hover_text("Home — Chat (the cooperative platform)");
                if brand.clicked() {
                    state.clear_nav_back();
                    state.active_page = GuiPage::Chat;
                }

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Red group: identity (unchanged by context)
                let red_items = [
                    NavItem { label: "Chat", page: GuiPage::Chat, description: "" },
                    NavItem { label: "Wallet", page: GuiPage::Wallet, description: "" },
                    NavItem { label: "Donate", page: GuiPage::Donate, description: "" },
                ];
                nav_group(ui, &red_items, theme.nav_legacy_red(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Green group: context-sensitive
                let green_items = [
                    NavItem { label: "Profile", page: GuiPage::Profile, description: "" },
                    NavItem { label: "Identity", page: GuiPage::Identity, description: "" },
                    NavItem { label: "Governance", page: GuiPage::Governance, description: "" },
                    NavItem { label: "Recovery", page: GuiPage::Recovery, description: "" },
                    NavItem { label: "Tasks", page: GuiPage::Tasks, description: "" },
                    NavItem { label: "Inventory", page: GuiPage::Inventory, description: "" },
                    NavItem { label: "Maps", page: GuiPage::Maps, description: "" },
                    NavItem { label: "Market", page: GuiPage::Market, description: "" },
                    NavItem { label: "Crafting", page: GuiPage::Crafting, description: "" },
                    NavItem { label: "Civilization", page: GuiPage::Civilization, description: "" },
                    NavItem { label: "Studio", page: GuiPage::Studio, description: "" },
                ];
                nav_group(ui, &green_items, theme.nav_legacy_green(), text_muted, theme, state);

                ui.add_space(6.0);
                separator_dot(ui, border);
                ui.add_space(6.0);

                // Blue group: system
                let blue_items = [
                    NavItem { label: "Onboarding", page: GuiPage::Onboarding, description: "" },
                    NavItem { label: "Agents", page: GuiPage::Agents, description: "" },
                    NavItem { label: "AI Usage", page: GuiPage::AiUsage, description: "" },
                    NavItem { label: "Settings", page: GuiPage::Settings, description: "" },
                    NavItem { label: "Tools", page: GuiPage::Tools, description: "" },
                    NavItem { label: "Bugs", page: GuiPage::BugReport, description: "" },
                    NavItem { label: "Testing", page: GuiPage::Testing, description: "" },
                    NavItem { label: "Browser", page: GuiPage::Browser, description: "" },
                ];
                nav_group(ui, &blue_items, theme.nav_legacy_blue(), text_muted, theme, state);

                // Push Real/Sim toggle to the right.
                // Render order in right_to_left: first = rightmost. So:
                //   help_btn → sim → real  ⇒  visual order: [Real][Sim][?]
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    // Help "?" button for the Real/Sim toggle
                    ui.add_space(4.0);
                    let help_size = Vec2::new(18.0, 18.0);
                    let (help_rect, help_resp) = ui.allocate_exact_size(help_size, Sense::click());
                    if ui.is_rect_visible(help_rect) {
                        let (stroke_color, text_color) = if help_resp.hovered() {
                            (accent, accent)
                        } else {
                            (border, text_muted)
                        };
                        let painter = ui.painter();
                        painter.circle_stroke(help_rect.center(), 8.0, Stroke::new(1.0, stroke_color));
                        painter.text(
                            help_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "?",
                            egui::FontId::proportional(10.0),
                            text_color,
                        );
                    }
                    if help_resp.clicked() {
                        state.active_help_topic = Some("real-sim".to_string());
                    }
                    ui.add_space(4.0);
                    // v0.196.0: removed the 2T (two-tier) layout toggle.
                    // Operator decided the single-row nav is cleaner and
                    // we want fewer pages overall, not more nav layers.

                    let sim_active = !state.context_real;
                    let real_active = state.context_real;

                    // Sim button — uses the same purple as the two-tier
                    // nav's Sim category so the design language matches.
                    let sim_color = if sim_active {
                        theme.nav_sim()
                    } else {
                        border
                    };
                    let sim_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Sim").size(11.0).color(if sim_active {
                                Color32::WHITE
                            } else {
                                text_muted
                            }),
                        )
                        .fill(sim_color)
                        .rounding(Rounding {
                            nw: 0, ne: 4, se: 4, sw: 0,
                        })
                        .min_size(Vec2::new(36.0, 22.0)),
                    );
                    if sim_btn.clicked() {
                        state.context_real = false;
                    }

                    // Real button
                    let real_color = if real_active {
                        accent
                    } else {
                        border
                    };
                    let real_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Real").size(11.0).color(if real_active {
                                Color32::BLACK
                            } else {
                                text_muted
                            }),
                        )
                        .fill(real_color)
                        .rounding(Rounding {
                            nw: 4, ne: 0, se: 0, sw: 4,
                        })
                        .min_size(Vec2::new(36.0, 22.0)),
                    );
                    if real_btn.clicked() {
                        state.context_real = true;
                    }
                });
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
            state.active_page = item.page.clone();
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

// Top-tier category color palette moved to theme.ron in v0.175.0.
// Read via `theme.nav_reality()`, `theme.nav_sim()`, etc.

struct TopCategory {
    id: &'static str,
    label: &'static str,
    color: Color32,
}

/// All possible top categories. Dev is filtered out at render time when
/// `theme.nav_dev_visible` is false (planned for v1.0; on by default
/// during the development period). Returns a Vec rather than a fixed
/// array because the Dev slot is conditional.
fn top_categories(theme: &Theme) -> Vec<TopCategory> {
    let mut cats = vec![
        TopCategory { id: "reality",  label: "Reality",  color: theme.nav_reality() },
        TopCategory { id: "sim",      label: "Sim",      color: theme.nav_sim() },
        TopCategory { id: "tools",    label: "Tools",    color: theme.nav_tools() },
        TopCategory { id: "settings", label: "Settings", color: theme.nav_settings() },
    ];
    if theme.nav_dev_visible {
        cats.push(TopCategory { id: "dev", label: "Dev", color: theme.nav_dev() });
    }
    cats
}

/// Sub-pages for a given top category. Source of truth for nav grouping
/// is `docs/PAGES.md` "Natural groupings" table — keep in sync.
///
/// Reality = identity, communication, civic life (works in both Real and
/// Sim contexts; the page itself disambiguates if needed).
/// Sim = in-game / character-bound activities.
/// Tools = utility apps that aren't bound to game state.
/// Settings = personal config + server admin.
/// Dev = developer / QA / inspection (visibility-gated).
/// Public view of `sub_pages_for` for the category-overview pages.
/// Returns (label, page, description) tuples so callers don't depend on
/// the private NavItem type. Empty Vec for unknown categories.
pub fn category_pages(category: &str) -> Vec<(&'static str, GuiPage, &'static str)> {
    sub_pages_for(category)
        .into_iter()
        .map(|n| (n.label, n.page, n.description))
        .collect()
}

/// Top category metadata for overview pages: id, label, color, description.
pub fn category_meta(category: &str, theme: &Theme) -> Option<(&'static str, Color32, &'static str)> {
    match category {
        "reality"  => Some(("Reality",  theme.nav_reality(),  "Identity, communication, civic life — the real you, your real money, your real community.")),
        "sim"      => Some(("Sim",      theme.nav_sim(),      "In-game / character-bound activities. Simulated economies, crafted items, ship interiors.")),
        "tools"    => Some(("Tools",    theme.nav_tools(),    "Utility apps that aren't bound to game state. Calculator, calendar, notes, browser.")),
        "settings" => Some(("Settings", theme.nav_settings(), "Personal preferences and server administration.")),
        "dev"      => Some(("Dev",      theme.nav_dev(),      "Developer / QA / inspection. Hidden by default at v1.0; on during the dev period.")),
        _ => None,
    }
}

fn sub_pages_for(category: &str) -> Vec<NavItem> {
    match category {
        "reality" => vec![
            NavItem { label: "Profile",      page: GuiPage::Profile,      description: "Public-facing identity, bio, socials, avatar." },
            NavItem { label: "Chat",         page: GuiPage::Chat,         description: "Cooperative messaging across servers and DMs." },
            NavItem { label: "Wallet",       page: GuiPage::Wallet,       description: "Self-custodied crypto wallet (Solana SOL + tokens)." },
            NavItem { label: "Donate",       page: GuiPage::Donate,       description: "Support development via crypto / GitHub Sponsors." },
            NavItem { label: "Tasks",        page: GuiPage::Tasks,        description: "Personal + shared kanban with project grouping." },
            NavItem { label: "Market",       page: GuiPage::Market,       description: "P2P marketplace: browse, list, message sellers." },
            NavItem { label: "Civilization", page: GuiPage::Civilization, description: "Community / colony stats, charts, timeline." },
            NavItem { label: "Governance",   page: GuiPage::Governance,   description: "Proposals, votes, tally — local + civilization scope." },
            NavItem { label: "Maps",         page: GuiPage::Maps,         description: "Solar system + planet detail browser." },
            NavItem { label: "Recovery",     page: GuiPage::Recovery,     description: "Social key recovery (Shamir M-of-N guardians)." },
            NavItem { label: "Identity",     page: GuiPage::Identity,     description: "DID, Verifiable Credentials, trust score, AI status." },
        ],
        "sim" => vec![
            NavItem { label: "Inventory", page: GuiPage::Inventory, description: "Item grid, equipment slots, weight tracking." },
            NavItem { label: "Crafting",  page: GuiPage::Crafting,  description: "Recipes by category with craft queue + progress." },
            NavItem { label: "Studio",    page: GuiPage::Studio,    description: "OBS-style scene + source manager for streams." },
            NavItem { label: "Guilds",    page: GuiPage::Guilds,    description: "Guild browser, members, chat, create form." },
            NavItem { label: "Trade",     page: GuiPage::Trade,     description: "P2P trades with escrow + dual confirmation." },
        ],
        "tools" => vec![
            NavItem { label: "Calculator", page: GuiPage::Calculator, description: "Scientific calculator with history." },
            NavItem { label: "Calendar",   page: GuiPage::Calendar,   description: "Month view + add events with time and color." },
            NavItem { label: "Notes",      page: GuiPage::Notes,      description: "Notes app with autosave + word count." },
            NavItem { label: "Resources",  page: GuiPage::Resources,  description: "Curated resource directory (Real / Sim aware)." },
            NavItem { label: "Tools",      page: GuiPage::Tools,      description: "Open-source tools catalog with search + filters." },
            NavItem { label: "Browser",    page: GuiPage::Browser,    description: "Curated bookmarks; opens in your default browser." },
        ],
        "settings" => vec![
            NavItem { label: "Account",       page: GuiPage::SettingsAccount,       description: "Display name, public key, ECDH DM key, seed-phrase backup." },
            NavItem { label: "Appearance",    page: GuiPage::SettingsAppearance,    description: "Dark mode, font size, theme colors, nav category colors." },
            NavItem { label: "Animations",    page: GuiPage::SettingsAnimations,    description: "RGB style + speed + attack indicator picker." },
            NavItem { label: "Widgets",       page: GuiPage::SettingsWidgets,       description: "Sizing, spacing, fonts, borders, slider + checkbox." },
            NavItem { label: "Notifications", page: GuiPage::SettingsNotifications, description: "DM, mentions, tasks, do-not-disturb window." },
            NavItem { label: "Wallet",        page: GuiPage::SettingsWallet,        description: "Solana RPC, network, default tip amounts." },
            NavItem { label: "Audio",         page: GuiPage::SettingsAudio,         description: "Master / music / SFX volume + voice devices." },
            NavItem { label: "Graphics",      page: GuiPage::SettingsGraphics,      description: "Fullscreen, vsync, FOV, render distance." },
            NavItem { label: "Controls",      page: GuiPage::SettingsControls,      description: "Mouse sensitivity, key rebinds, gamepad." },
            NavItem { label: "Privacy",       page: GuiPage::SettingsPrivacy,       description: "Public profile fields, message visibility, federation opt-ins." },
            NavItem { label: "Data",          page: GuiPage::SettingsData,          description: "Local storage, vault sync, export, restore." },
            NavItem { label: "Updates",       page: GuiPage::SettingsUpdates,       description: "Version, check for updates, channel selector." },
            NavItem { label: "Onboarding",    page: GuiPage::Onboarding,            description: "First-run orientation + permanent reference quests." },
            NavItem { label: "Server Admin",  page: GuiPage::ServerSettings,        description: "Server / group admin (USER / MOD / ADMIN tiered)." },
        ],
        "dev" => vec![
            NavItem { label: "Testing",  page: GuiPage::Testing,   description: "QA checklist; Mark Passed / Report Issue posts to chat." },
            NavItem { label: "Bugs",     page: GuiPage::BugReport, description: "Submit bug reports with severity + category." },
            NavItem { label: "Agents",   page: GuiPage::Agents,    description: "Multi-AI scope coordination dashboard." },
            NavItem { label: "AI Usage", page: GuiPage::AiUsage,   description: "AI subscription quota tracker + usage log." },
            NavItem { label: "Files",    page: GuiPage::Files,     description: "Browse + edit text files in the data/ directory." },
        ],
        _ => Vec::new(),
    }
}

// v0.196.0: removed `draw_nav_bar_two_tier`. Operator decided the
// single-row nav is cleaner and we want to reduce total page count
// rather than add layout layers. `top_categories` and `sub_pages_for`
// are kept because the category-overview pages (Reality / Sim / Tools /
// Settings / Dev) still use them via `pub_sub_pages_for`. The
// `state.nav_two_tier` and `state.nav_top_category` fields stay on
// GuiState for backwards-compatible config deserialization but the
// single-row layout ignores them.
