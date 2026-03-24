//! Escape menu — full navigation to all app pages.
//!
//! Appears when pressing Escape during gameplay. Semi-transparent
//! backdrop over the 3D scene with buttons for every tool/page.

use egui::{Align2, Area, Color32, Frame, RichText, Vec2};
use crate::gui::{GuiPage, GuiState, VERSION};

/// Draw the escape menu overlay.
pub fn draw(ctx: &egui::Context, state: &mut GuiState) {
    // Semi-transparent backdrop
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));

    Area::new(egui::Id::new("escape_menu"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            Frame::none()
                .inner_margin(32.0)
                .show(ui, |ui| {
                    ui.set_min_width(400.0);

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("HumanityOS")
                                .size(32.0)
                                .color(Color32::from_rgb(237, 140, 36)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("End poverty. Unite humanity.")
                                .size(14.0)
                                .color(Color32::from_rgb(150, 150, 160)),
                        );
                        ui.add_space(24.0);
                    });

                    // Resume button (prominent)
                    ui.vertical_centered(|ui| {
                        let resume = ui.add_sized(
                            Vec2::new(300.0, 40.0),
                            egui::Button::new(RichText::new("Resume").size(16.0)),
                        );
                        if resume.clicked() {
                            state.active_page = GuiPage::None;
                        }
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Two-column grid of page buttons
                    ui.columns(2, |cols| {
                        let left_pages = [
                            ("Inventory", GuiPage::Inventory),
                            ("Tasks", GuiPage::Tasks),
                            ("Maps", GuiPage::Maps),
                            ("Market", GuiPage::Market),
                            ("Crafting", GuiPage::Crafting),
                            ("Quests", GuiPage::Quests),
                            ("Calculator", GuiPage::Calculator),
                        ];

                        let right_pages = [
                            ("Profile", GuiPage::Profile),
                            ("Chat", GuiPage::Chat),
                            ("Civilization", GuiPage::Civilization),
                            ("Notes", GuiPage::Notes),
                            ("Calendar", GuiPage::Calendar),
                            ("Settings", GuiPage::Settings),
                            ("Bug Report", GuiPage::BugReport),
                        ];

                        for (label, page) in left_pages {
                            if cols[0]
                                .add_sized(
                                    Vec2::new(180.0, 32.0),
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                state.active_page = page;
                            }
                            cols[0].add_space(4.0);
                        }

                        for (label, page) in right_pages {
                            if cols[1]
                                .add_sized(
                                    Vec2::new(180.0, 32.0),
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                state.active_page = page;
                            }
                            cols[1].add_space(4.0);
                        }
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Bottom row: Main Menu and Quit
                    ui.horizontal(|ui| {
                        ui.add_space(50.0);
                        if ui
                            .add_sized(Vec2::new(140.0, 32.0), egui::Button::new("Main Menu"))
                            .clicked()
                        {
                            state.active_page = GuiPage::MainMenu;
                        }
                        ui.add_space(16.0);
                        if ui
                            .add_sized(
                                Vec2::new(140.0, 32.0),
                                egui::Button::new(
                                    RichText::new("Quit").color(Color32::from_rgb(231, 76, 60)),
                                ),
                            )
                            .clicked()
                        {
                            state.quit_requested = true;
                        }
                    });

                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!("v{}", VERSION))
                                .size(11.0)
                                .color(Color32::from_rgb(80, 80, 90)),
                        );
                    });
                });
        });
}
