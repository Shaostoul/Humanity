//! In-game HUD: health bar, hotbar, crosshair, compass, FPS.

use egui::{Align2, Area, Color32, FontId, Pos2, Rect, RichText, Rounding, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::updater::UpdateState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &GuiState, camera_yaw: f32) {
    let screen = ctx.screen_rect();

    Area::new(egui::Id::new("hud_layer"))
        .fixed_pos([0.0, 0.0])
        .show(ctx, |ui| {
            // Allocate full screen but don't consume input
            ui.allocate_rect(screen, egui::Sense::hover());
            let painter = ui.painter();

            // ── Health bar (top-left) ──
            let hp = 0.75_f32; // placeholder
            let hp_rect = Rect::from_min_size(Pos2::new(16.0, 16.0), Vec2::new(200.0, 16.0));
            painter.rect_filled(hp_rect, Rounding::same(4), Color32::from_black_alpha(140));
            let filled = Rect::from_min_size(hp_rect.min, Vec2::new(200.0 * hp, 16.0));
            let hp_color = if hp > 0.5 { theme.success() } else if hp > 0.25 { theme.warning() } else { theme.danger() };
            painter.rect_filled(filled, Rounding::same(4), hp_color);
            painter.text(hp_rect.center(), Align2::CENTER_CENTER, format!("{}%", (hp * 100.0) as i32), FontId::proportional(11.0), Color32::WHITE);

            // ── FPS counter (top-right) ──
            painter.text(
                Pos2::new(screen.right() - 16.0, 16.0),
                Align2::RIGHT_TOP,
                format!("{:.0} FPS", state.fps),
                FontId::proportional(12.0),
                theme.text_muted(),
            );

            // ── Crosshair (center) ──
            let center = screen.center();
            painter.circle_filled(center, 3.0, Color32::from_white_alpha(180));

            // ── Compass (top-center) ──
            let compass_y = 20.0;
            let directions = [
                (0.0_f32, "N"),
                (std::f32::consts::FRAC_PI_2, "E"),
                (std::f32::consts::PI, "S"),
                (-std::f32::consts::FRAC_PI_2, "W"),
            ];
            let compass_width = 200.0;
            for (angle, label) in &directions {
                let diff = normalize_angle(*angle - camera_yaw);
                if diff.abs() < std::f32::consts::FRAC_PI_2 {
                    let x = center.x + diff / std::f32::consts::FRAC_PI_2 * (compass_width / 2.0);
                    let color = if *label == "N" { theme.danger() } else { theme.text_secondary() };
                    painter.text(Pos2::new(x, compass_y), Align2::CENTER_TOP, *label, FontId::proportional(14.0), color);
                }
            }

            // ── Hotbar (bottom-center) ──
            let slot_size = 44.0;
            let slot_gap = 4.0;
            let slot_count = 9;
            let total_width = slot_count as f32 * slot_size + (slot_count - 1) as f32 * slot_gap;
            let start_x = center.x - total_width / 2.0;
            let start_y = screen.bottom() - slot_size - 16.0;

            for i in 0..slot_count {
                let x = start_x + i as f32 * (slot_size + slot_gap);
                let rect = Rect::from_min_size(Pos2::new(x, start_y), Vec2::splat(slot_size));
                painter.rect_filled(rect, Rounding::same(4), Color32::from_black_alpha(140));
                painter.rect_stroke(rect, Rounding::same(4), egui::Stroke::new(1.0, theme.border()), egui::StrokeKind::Outside);

                // Slot number
                painter.text(
                    rect.left_top() + Vec2::new(4.0, 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", i + 1),
                    FontId::proportional(10.0),
                    theme.text_muted(),
                );
            }

            // ── Update notification toast (top-right, below FPS) ──
            if let UpdateState::Available { ref version, .. } = state.updater.state {
                let toast_rect = Rect::from_min_size(
                    Pos2::new(screen.right() - 260.0, 36.0),
                    Vec2::new(244.0, 44.0),
                );
                painter.rect_filled(toast_rect, Rounding::same(6), Color32::from_rgba_premultiplied(20, 20, 25, 230));
                painter.rect_stroke(toast_rect, Rounding::same(6), egui::Stroke::new(1.0, theme.accent()), egui::StrokeKind::Outside);
                painter.text(
                    toast_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("Update {} available", version),
                    FontId::proportional(12.0),
                    theme.accent(),
                );
            }
        });
}

fn normalize_angle(a: f32) -> f32 {
    let mut a = a % (2.0 * std::f32::consts::PI);
    if a > std::f32::consts::PI { a -= 2.0 * std::f32::consts::PI; }
    if a < -std::f32::consts::PI { a += 2.0 * std::f32::consts::PI; }
    a
}
