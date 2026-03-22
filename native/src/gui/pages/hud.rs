//! HUD overlay — health bar, hotbar, compass, day/night, FPS counter.

use egui::{Align2, Area, Color32, Rounding, Stroke, StrokeKind};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::GameTime;

/// Number of hotbar slots.
const HOTBAR_SLOTS: usize = 9;
/// Size of each hotbar slot in pixels.
const HOTBAR_SLOT_SIZE: f32 = 48.0;

/// Draw the full in-game HUD.
pub fn draw(ctx: &egui::Context, theme: &Theme, gui_state: &GuiState, data_store: &DataStore) {
    draw_health_bar(ctx, theme);
    draw_hotbar(ctx, theme);
    draw_crosshair(ctx);
    draw_compass(ctx, theme, data_store);
    draw_day_night(ctx, theme, data_store);
    draw_fps(ctx, theme, gui_state);
}

/// Health bar in the top-left corner.
fn draw_health_bar(ctx: &egui::Context, theme: &Theme) {
    Area::new(egui::Id::new("hud_health"))
        .fixed_pos(egui::pos2(16.0, 16.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{2764}")
                        .size(16.0)
                        .color(theme.danger),
                );
                // Default 80% health for demo
                widgets::progress_bar(ui, 0.8, theme.danger, "80 / 100", 160.0);
            });
        });
}

/// Hotbar at bottom center — 9 numbered slots.
fn draw_hotbar(ctx: &egui::Context, theme: &Theme) {
    let screen = ctx.screen_rect();
    let total_width = HOTBAR_SLOTS as f32 * (HOTBAR_SLOT_SIZE + 4.0) - 4.0;
    let x = (screen.width() - total_width) / 2.0;
    let y = screen.height() - HOTBAR_SLOT_SIZE - 20.0;

    Area::new(egui::Id::new("hud_hotbar"))
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for i in 0..HOTBAR_SLOTS {
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
                        egui::Sense::click(),
                    );

                    // Slot background
                    ui.painter().rect(
                        rect,
                        Rounding::same(4),
                        Color32::from_rgba_premultiplied(20, 20, 30, 180),
                        Stroke::new(1.0, theme.primary.linear_multiply(0.4)),
                        StrokeKind::Outside,
                    );

                    // Slot number in top-left
                    ui.painter().text(
                        rect.left_top() + egui::vec2(4.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", i + 1),
                        egui::FontId::new(10.0, egui::FontFamily::Proportional),
                        theme.text_dim,
                    );
                }
            });
        });
}

/// Small crosshair dot at screen center.
fn draw_crosshair(ctx: &egui::Context) {
    let screen = ctx.screen_rect();
    let center = screen.center();

    Area::new(egui::Id::new("hud_crosshair"))
        .fixed_pos(center - egui::vec2(3.0, 3.0))
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 2.5, Color32::from_rgba_premultiplied(255, 255, 255, 180));
        });
}

/// Compass at top center showing cardinal direction based on camera yaw.
fn draw_compass(ctx: &egui::Context, theme: &Theme, data_store: &DataStore) {
    let screen = ctx.screen_rect();
    let yaw = data_store.get::<f32>("camera_yaw").copied().unwrap_or(0.0);

    // Convert yaw radians to compass bearing (0 = North, PI/2 = East, etc.)
    let degrees = yaw.to_degrees().rem_euclid(360.0);
    let direction = match degrees as u32 {
        0..=22 | 338..=360 => "N",
        23..=67 => "NE",
        68..=112 => "E",
        113..=157 => "SE",
        158..=202 => "S",
        203..=247 => "SW",
        248..=292 => "W",
        293..=337 => "NW",
        _ => "N",
    };

    Area::new(egui::Id::new("hud_compass"))
        .fixed_pos(egui::pos2(screen.center().x - 30.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(direction)
                        .size(18.0)
                        .color(theme.accent),
                );
                ui.label(
                    egui::RichText::new(format!(" {:.0}\u{00B0}", degrees))
                        .size(12.0)
                        .color(theme.text_dim),
                );
            });
        });
}

/// Day/night indicator showing current game time.
fn draw_day_night(ctx: &egui::Context, theme: &Theme, data_store: &DataStore) {
    let screen = ctx.screen_rect();

    let (icon, time_str) = if let Some(game_time) = data_store.get::<GameTime>("game_time") {
        let icon = if game_time.hour >= 6.0 && game_time.hour < 18.0 {
            "\u{2600}" // Sun
        } else {
            "\u{263D}" // Moon
        };
        let hour = game_time.hour as u32;
        let minute = ((game_time.hour - hour as f32) * 60.0) as u32;
        (icon, format!("{:02}:{:02}", hour, minute))
    } else {
        ("\u{2600}", "08:00".to_string())
    };

    Area::new(egui::Id::new("hud_daytime"))
        .fixed_pos(egui::pos2(screen.center().x + 40.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(icon)
                        .size(16.0)
                        .color(theme.warning),
                );
                ui.label(
                    egui::RichText::new(&time_str)
                        .size(12.0)
                        .color(theme.text_dim),
                );
            });
        });
}

/// FPS counter in the top-right corner.
fn draw_fps(ctx: &egui::Context, theme: &Theme, gui_state: &GuiState) {
    let screen = ctx.screen_rect();

    Area::new(egui::Id::new("hud_fps"))
        .fixed_pos(egui::pos2(screen.width() - 80.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!("{} FPS", gui_state.current_fps))
                    .size(12.0)
                    .color(theme.text_dim)
                    .family(egui::FontFamily::Monospace),
            );
        });
}
