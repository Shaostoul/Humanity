//! Escape menu with RGB-colored header nav bar.
//!
//! Mirrors the web shell.js color-coded navigation:
//! - Red group: identity pages (never change with context)
//! - Green group: context-sensitive pages (data changes with Real/Sim)
//! - Blue group: system/config pages
//! Plus a Real/Sim toggle on the right.

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};

// Color constants matching web theme.css
const RED: Color32 = Color32::from_rgb(231, 76, 60);
const GREEN: Color32 = Color32::from_rgb(46, 204, 113);
const BLUE: Color32 = Color32::from_rgb(52, 152, 219);
const ACCENT: Color32 = Color32::from_rgb(0xED, 0x8C, 0x24);     // #ED8C24
const BG_DARK: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0c);   // #0a0a0c
const BG_BAR: Color32 = Color32::from_rgb(0x0f, 0x0f, 0x12);    // #0f0f12 (nav bar)
const TEXT_MUTED: Color32 = Color32::from_rgb(0x6a, 0x6a, 0x75); // #6a6a75

struct NavItem {
    label: &'static str,
    page: GuiPage,
}

/// Draw the RGB header nav bar at the top of the screen.
/// Reusable across all pages (escape menu, tool pages, etc.).
pub fn draw_nav_bar(ctx: &egui::Context, state: &mut GuiState) {
    // Keep repainting so the RGB channeling animation stays smooth
    ctx.request_repaint();

    egui::TopBottomPanel::top("escape_nav_bar")
        .frame(Frame::none().fill(BG_BAR).inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // Brand
                let brand = ui.add(
                    egui::Button::new(RichText::new("H").size(14.0).strong().color(ACCENT))
                        .min_size(Vec2::new(28.0, 28.0))
                        .rounding(Rounding::same(6)),
                );
                if brand.clicked() {
                    state.active_page = GuiPage::None;
                }

                ui.add_space(6.0);
                separator_dot(ui);
                ui.add_space(6.0);

                // Red group: identity (unchanged by context)
                let red_items = [
                    NavItem { label: "Chat", page: GuiPage::Chat },
                    NavItem { label: "Wallet", page: GuiPage::Wallet },
                    NavItem { label: "Donate", page: GuiPage::Donate },
                ];
                nav_group(ui, &red_items, RED, state);

                ui.add_space(6.0);
                separator_dot(ui);
                ui.add_space(6.0);

                // Green group: context-sensitive
                let green_items = [
                    NavItem { label: "Profile", page: GuiPage::Profile },
                    NavItem { label: "Tasks", page: GuiPage::Tasks },
                    NavItem { label: "Inventory", page: GuiPage::Inventory },
                    NavItem { label: "Maps", page: GuiPage::Maps },
                    NavItem { label: "Market", page: GuiPage::Market },
                    NavItem { label: "Crafting", page: GuiPage::Crafting },
                    NavItem { label: "Civilization", page: GuiPage::Civilization },
                ];
                nav_group(ui, &green_items, GREEN, state);

                ui.add_space(6.0);
                separator_dot(ui);
                ui.add_space(6.0);

                // Blue group: system
                let blue_items = [
                    NavItem { label: "Settings", page: GuiPage::Settings },
                    NavItem { label: "Tools", page: GuiPage::Tools },
                    NavItem { label: "Bugs", page: GuiPage::BugReport },
                ];
                nav_group(ui, &blue_items, BLUE, state);

                // Push Real/Sim toggle to the right
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    let sim_active = !state.context_real;
                    let real_active = state.context_real;

                    // Sim button
                    let sim_color = if sim_active {
                        Color32::from_rgb(108, 92, 231)
                    } else {
                        Color32::from_rgb(0x2a, 0x2a, 0x35)
                    };
                    let sim_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Sim").size(11.0).color(if sim_active {
                                Color32::WHITE
                            } else {
                                TEXT_MUTED
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
                        ACCENT
                    } else {
                        Color32::from_rgb(0x2a, 0x2a, 0x35)
                    };
                    let real_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Real").size(11.0).color(if real_active {
                                Color32::BLACK
                            } else {
                                TEXT_MUTED
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
}

/// Draw the escape menu center content (Resume, Main Menu, Quit).
/// The nav bar is drawn separately by the engine for all pages.
pub fn draw(ctx: &egui::Context, state: &mut GuiState) {
    // Center content area with Resume and secondary nav
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::TRANSPARENT))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);

                ui.label(
                    RichText::new("HumanityOS")
                        .size(36.0)
                        .color(ACCENT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("End poverty. Unite humanity.")
                        .size(14.0)
                        .color(TEXT_MUTED),
                );
                ui.add_space(24.0);

                // Resume button
                if ui
                    .add_sized(
                        Vec2::new(200.0, 40.0),
                        egui::Button::new(RichText::new("Resume").size(16.0).color(Color32::WHITE))
                            .fill(ACCENT),
                    )
                    .clicked()
                {
                    state.active_page = GuiPage::None;
                }

                ui.add_space(16.0);

                // Secondary row
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 200.0) / 2.0);
                    if ui
                        .add_sized(Vec2::new(90.0, 28.0), egui::Button::new("Main Menu"))
                        .clicked()
                    {
                        state.active_page = GuiPage::MainMenu;
                    }
                    ui.add_space(8.0);
                    if ui
                        .add_sized(
                            Vec2::new(90.0, 28.0),
                            egui::Button::new(RichText::new("Quit").color(RED)),
                        )
                        .clicked()
                    {
                        state.quit_requested = true;
                    }
                });

                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("v{}", VERSION))
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            });
        });
}

/// Convert HSV to RGB Color32.
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

/// Draw a group of nav buttons with border-based visual language.
///
/// Border states (from ops.html Color Reference):
/// - Default: thin 1px border in group color at low opacity
/// - Hover: blue 2px border glow
/// - Active (current page): animated RGB border cycling through hue spectrum
///
/// Group color subtly tints the button background.
fn nav_group(ui: &mut egui::Ui, items: &[NavItem], color: Color32, state: &mut GuiState) {
    let time = ui.ctx().input(|i| i.time) as f32;

    for item in items {
        let is_active = std::mem::discriminant(&state.active_page)
            == std::mem::discriminant(&item.page);

        let text_color = if is_active { Color32::WHITE } else { TEXT_MUTED };

        // Subtle group-color tinted background
        let bg_fill = if is_active {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30)
        } else {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 10)
        };

        // Border: active = animated RGB, default = thin group color
        let border_stroke = if is_active {
            let hue = (time * 0.3) % 1.0;
            let rgb = hsv_to_rgb(hue, 0.9, 1.0);
            Stroke::new(2.0, rgb)
        } else {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60))
        };

        let response = ui.add(
            egui::Button::new(RichText::new(item.label).size(11.0).color(text_color))
                .fill(bg_fill)
                .stroke(border_stroke)
                .rounding(Rounding::same(6))
                .min_size(Vec2::new(0.0, 28.0)),
        );

        // Override border on hover: blue 2px glow
        if response.hovered() && !is_active {
            let rect = response.rect;
            let painter = ui.painter();
            let hover_color = Color32::from_rgb(52, 152, 219); // BLUE
            painter.rect_stroke(
                rect,
                Rounding::same(6),
                Stroke::new(2.0, hover_color),
                egui::StrokeKind::Outside,
            );
        }

        if response.clicked() {
            state.active_page = item.page.clone();
        }
    }
}

/// Small dot separator between nav groups.
fn separator_dot(ui: &mut egui::Ui) {
    ui.label(RichText::new("·").size(14.0).color(Color32::from_rgb(0x2a, 0x2a, 0x35)));
}
