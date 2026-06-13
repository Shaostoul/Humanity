//! In-game HUD: health bar, hotbar, crosshair, compass, FPS, weather, day/night.

use egui::{Align2, Area, Color32, FontId, Pos2, Rect, RichText, Rounding, Vec2};
use glam::{Mat4, Vec3};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::updater::UpdateState;

pub fn draw(
    ctx: &egui::Context,
    theme: &Theme,
    state: &GuiState,
    camera_yaw: f32,
    view_proj: Mat4,
    cam_pos: Vec3,
) {
    let screen = ctx.screen_rect();

    Area::new(egui::Id::new("hud_layer"))
        .fixed_pos([0.0, 0.0])
        .show(ctx, |ui| {
            // Allocate full screen but don't consume input
            ui.allocate_rect(screen, egui::Sense::hover());
            let painter = ui.painter();

            // ── Health bar (top-left) ──
            let hp = if state.player_health_max > 0.0 {
                (state.player_health / state.player_health_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let hp_rect = Rect::from_min_size(Pos2::new(16.0, 16.0), Vec2::new(200.0, 16.0));
            painter.rect_filled(hp_rect, Rounding::same(4), Color32::from_black_alpha(140));
            let filled = Rect::from_min_size(hp_rect.min, Vec2::new(200.0 * hp, 16.0));
            let hp_color = if hp > 0.5 { theme.success() } else if hp > 0.25 { theme.warning() } else { theme.danger() };
            painter.rect_filled(filled, Rounding::same(4), hp_color);
            painter.text(
                hp_rect.center(),
                Align2::CENTER_CENTER,
                format!("{:.0}/{:.0}", state.player_health, state.player_health_max),
                FontId::proportional(11.0),
                Color32::WHITE,
            );

            // ── FPS counter (top-right) ──
            painter.text(
                Pos2::new(screen.right() - 16.0, 16.0),
                Align2::RIGHT_TOP,
                format!("{:.0} FPS", state.fps),
                FontId::proportional(12.0),
                theme.text_muted(),
            );

            // ── Day/Night + Time indicator (below FPS) ──
            if let Some(ref gt) = state.game_time {
                let time_str = format!(
                    "Day {} {:02}:{:02} {}",
                    gt.day_count + 1,
                    gt.hour as u32,
                    ((gt.hour.fract()) * 60.0) as u32,
                    gt.season,
                );
                let day_icon = if gt.is_daytime { "☀" } else { "☾" };
                let day_color = if gt.is_daytime { theme.warning() } else { Color32::from_rgb(140, 160, 220) };
                painter.text(
                    Pos2::new(screen.right() - 16.0, 32.0),
                    Align2::RIGHT_TOP,
                    format!("{} {}", day_icon, time_str),
                    FontId::proportional(11.0),
                    day_color,
                );
            }

            // ── Weather indicator (below time) ──
            if let Some(ref w) = state.weather {
                let weather_icon = match w.condition.as_str() {
                    "Clear" => "☀",
                    "Cloudy" => "☁",
                    "Rain" => "🌧",
                    "Storm" => "⛈",
                    "Snow" => "❄",
                    "Fog" => "🌫",
                    "Sandstorm" => "🌪",
                    _ => "?",
                };
                painter.text(
                    Pos2::new(screen.right() - 16.0, 46.0),
                    Align2::RIGHT_TOP,
                    format!("{} {} {:.0}C {:.0}m/s", weather_icon, w.condition, w.temperature, w.wind_speed),
                    FontId::proportional(11.0),
                    theme.text_secondary(),
                );
            }

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

            // ── Machine labels (world-space, distance LOD + room occlusion) ──
            // dot within dot_dist -> +name within name_dist -> +card within card_dist.
            // By default ONLY machines in the room you are in show (walls occlude the
            // rest). Hold Tab to reveal markers through walls across all owned/explored
            // rooms at x3 distance. v0.429.
            let mul = if state.reveal_held { 3.0 } else { 1.0 };
            let dot_dist = state.machine_label_dot_dist.max(0.5) * mul;
            let name_dist = state.machine_label_name_dist.max(0.5) * mul;
            let card_dist = state.machine_label_card_dist.max(0.5) * mul;
            // Which room is the camera in? (None = outside every room.)
            let current_room: Option<&str> = state.room_bounds.iter()
                .find(|r| {
                    cam_pos.x >= r.min.x && cam_pos.x <= r.max.x
                        && cam_pos.y >= r.min.y && cam_pos.y <= r.max.y
                        && cam_pos.z >= r.min.z && cam_pos.z <= r.max.z
                })
                .map(|r| r.id.as_str());
            for label in &state.machine_labels {
                // Occlusion: by default only the camera's room is visible; Tab reveals all.
                if !state.reveal_held && current_room != Some(label.room.as_str()) {
                    continue;
                }
                let cam_dist = (label.pos - cam_pos).length();
                if cam_dist > dot_dist {
                    continue; // beyond the coarsest level of detail
                }
                let Some(sp) = world_to_screen(label.pos, view_proj, screen) else { continue };
                // Marker dot (small).
                painter.circle_filled(sp, 1.7, Color32::from_white_alpha(220));
                painter.circle_stroke(sp, 3.0, egui::Stroke::new(1.0, Color32::from_black_alpha(150)));
                if cam_dist <= card_dist {
                    draw_machine_card(painter, theme, sp, label);
                } else if cam_dist <= name_dist {
                    text_shadowed(painter, sp + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, &label.name, 12.0, Color32::WHITE);
                }
            }

            // ── Hotbar (bottom-center) ──
            // Show first 9 inventory slots as the hotbar
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

                // Show item from inventory if available
                if let Some(Some(item)) = state.inventory_items.get(i) {
                    // First letter of item name as icon
                    let icon = item.name.chars().next().unwrap_or('?').to_string();
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        &icon,
                        FontId::proportional(18.0),
                        theme.text_primary(),
                    );
                    // Quantity in bottom-right
                    painter.text(
                        rect.right_bottom() + Vec2::new(-4.0, -2.0),
                        Align2::RIGHT_BOTTOM,
                        item.quantity.to_string(),
                        FontId::proportional(10.0),
                        theme.text_muted(),
                    );
                }

                // Slot number
                painter.text(
                    rect.left_top() + Vec2::new(4.0, 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", i + 1),
                    FontId::proportional(10.0),
                    theme.text_muted(),
                );
            }

            // ── Update notification toast (top-right, below weather) ──
            if let UpdateState::Available { ref version, .. } = state.updater.state {
                let toast_rect = Rect::from_min_size(
                    Pos2::new(screen.right() - 260.0, 64.0),
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

/// Project a world point to screen pixels (wgpu NDC: x,y in [-1,1] y-up, z in [0,1]).
/// Returns None when the point is at/behind the camera or outside the depth range.
fn world_to_screen(world: Vec3, view_proj: Mat4, screen: Rect) -> Option<Pos2> {
    let clip = view_proj * world.extend(1.0);
    if clip.w <= 0.0001 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < 0.0 || ndc.z > 1.0 {
        return None;
    }
    let x = screen.left() + (ndc.x * 0.5 + 0.5) * screen.width();
    let y = screen.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * screen.height();
    Some(Pos2::new(x, y))
}

/// Draw text with a 1px black drop-shadow so it stays legible over any 3D background.
fn text_shadowed(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
) {
    painter.text(pos + Vec2::splat(1.0), anchor, text, FontId::proportional(size), Color32::from_black_alpha(170));
    painter.text(pos, anchor, text, FontId::proportional(size), color);
}

/// Color a stat readout by its status.
fn stat_status_color(status: &str, theme: &Theme) -> Color32 {
    match status {
        "ok" => theme.success(),
        "warn" | "low" => theme.warning(),
        "off" => theme.danger(),
        _ => theme.text_secondary(),
    }
}

/// The expanded machine info card: a name header plus clean two-column stat rows
/// (kind on the left colored by status, value on the right). v0.428; icons replace the
/// kind text in a later pass.
fn draw_machine_card(painter: &egui::Painter, theme: &Theme, anchor: Pos2, label: &crate::gui::MachineLabel) {
    let row_h = 15.0;
    let pad = 7.0;
    let w = 144.0;
    let rows = label.stats.len() as f32;
    let h = pad * 2.0 + row_h * (1.0 + rows) + 2.0;
    let card = Rect::from_min_size(Pos2::new(anchor.x + 12.0, anchor.y - h * 0.5), Vec2::new(w, h));
    painter.rect_filled(card, Rounding::same(5), Color32::from_black_alpha(205));
    painter.rect_stroke(card, Rounding::same(5), egui::Stroke::new(1.0, Color32::from_white_alpha(45)), egui::StrokeKind::Outside);
    // A connector tick from the dot to the card.
    painter.line_segment([anchor + Vec2::new(4.0, 0.0), Pos2::new(card.left(), anchor.y)], egui::Stroke::new(1.0, Color32::from_white_alpha(70)));

    let mut y = card.top() + pad;
    painter.text(Pos2::new(card.left() + pad, y), Align2::LEFT_TOP, &label.name, FontId::proportional(13.0), Color32::WHITE);
    y += row_h + 3.0;
    for s in &label.stats {
        let color = stat_status_color(&s.status, theme);
        painter.text(Pos2::new(card.left() + pad, y), Align2::LEFT_TOP, &s.kind, FontId::proportional(11.0), color);
        painter.text(Pos2::new(card.right() - pad, y), Align2::RIGHT_TOP, &s.value, FontId::proportional(11.0), theme.text_secondary());
        y += row_h;
    }
}
