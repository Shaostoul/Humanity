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
        // Non-interactable so this full-screen HUD layer never sits in front of an in-world
        // side panel and eats its clicks (the recurring "panel shows but won't click" bug).
        // The HUD is paint-only; it needs no input. (v0.461)
        .interactable(false)
        .show(ctx, |ui| {
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
            text_shadowed(
                painter,
                Pos2::new(screen.right() - 16.0, 16.0),
                Align2::RIGHT_TOP,
                &format!("{:.0} FPS", state.fps),
                12.0,
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
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 32.0),
                    Align2::RIGHT_TOP,
                    &format!("{} {}", day_icon, time_str),
                    11.0,
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
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 46.0),
                    Align2::RIGHT_TOP,
                    &format!("{} {} {:.0}C {:.0}m/s", weather_icon, w.condition, w.temperature, w.wind_speed),
                    11.0,
                    theme.text_secondary(),
                );
            }

            // ── Power balance (live home electrical sim, below weather) ──
            // Generation climbs at noon, falls to zero at night; net flips green->red.
            if state.power_generation > 0.0 || state.power_consumption > 0.0 {
                let col = if state.power_balance >= 0.0 { theme.success() } else { theme.danger() };
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 60.0),
                    Align2::RIGHT_TOP,
                    &format!(
                        "Power: gen {:.0}W  use {:.0}W  net {:+.0}W",
                        state.power_generation, state.power_consumption, state.power_balance
                    ),
                    11.0,
                    col,
                );
                // Battery line (v0.473): live state of charge + hours of autonomy, drawn under
                // the power line so the day/night swing reads as a draining/refilling number.
                if state.power_battery_capacity_wh > 0.0 {
                    let soc = (state.power_battery_wh / state.power_battery_capacity_wh * 100.0)
                        .clamp(0.0, 100.0);
                    let bcol = if soc > 20.0 { theme.text_secondary() } else { theme.danger() };
                    text_shadowed(
                        painter,
                        Pos2::new(screen.right() - 16.0, 75.0),
                        Align2::RIGHT_TOP,
                        &format!(
                            "Battery: {:.0}%  {:.1} kWh  ~{:.1} h autonomy",
                            soc,
                            state.power_battery_wh / 1000.0,
                            state.power_autonomy_hours
                        ),
                        11.0,
                        bcol,
                    );
                }
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
                    text_shadowed(painter, Pos2::new(x, compass_y), Align2::CENTER_TOP, label, 14.0, color);
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
            let current_room_info = state.room_bounds.iter().find(|r| {
                cam_pos.x >= r.min.x && cam_pos.x <= r.max.x
                    && cam_pos.y >= r.min.y && cam_pos.y <= r.max.y
                    && cam_pos.z >= r.min.z && cam_pos.z <= r.max.z
            });

            // ── Room purpose card (bottom-left): the walkable world now KNOWS what each
            // room is FOR, joined from data/rooms.ron. Name + purpose + the in-room actions
            // (the actions are shown as text for now; a later increment routes [E] to them).
            if let Some(room) = current_room_info {
                if !room.display_name.is_empty() {
                    let x = screen.left() + 16.0;
                    text_shadowed(
                        painter,
                        Pos2::new(x, screen.bottom() - 48.0),
                        Align2::LEFT_BOTTOM,
                        &room.display_name,
                        15.0,
                        theme.text_primary(),
                    );
                    if !room.purpose.is_empty() {
                        text_shadowed(
                            painter,
                            Pos2::new(x, screen.bottom() - 30.0),
                            Align2::LEFT_BOTTOM,
                            &room.purpose,
                            11.0,
                            theme.text_secondary(),
                        );
                    }
                    if !room.actions.is_empty() {
                        text_shadowed(
                            painter,
                            Pos2::new(x, screen.bottom() - 14.0),
                            Align2::LEFT_BOTTOM,
                            &format!("Here: {}", room.actions.join("  /  ")),
                            10.0,
                            theme.text_muted(),
                        );
                    }
                }
            }
            for (i, label) in state.machine_labels.iter().enumerate() {
                // Occlusion (v0.538): show a label only when its machine PHYSICALLY sits in the
                // camera's current room (geometric x/z containment), not by the machine's stored
                // room id -- which is advisory/stale in a HomeStructure box home, so an id compare
                // would wrongly hide a machine you're standing next to. Tab still reveals all.
                let in_current_room = current_room_info.map_or(false, |r| {
                    label.pos.x >= r.min.x && label.pos.x <= r.max.x
                        && label.pos.z >= r.min.z && label.pos.z <= r.max.z
                });
                if !state.reveal_held && !in_current_room {
                    continue;
                }
                let cam_dist = (label.pos - cam_pos).length();
                if cam_dist > dot_dist {
                    continue; // beyond the coarsest level of detail
                }
                let Some(sp) = world_to_screen(label.pos, view_proj, screen) else { continue };
                let is_target = state.targeted_machine == Some(i);
                // Marker dot; the machine you are looking at gets an accent ring.
                painter.circle_filled(sp, 1.7, Color32::from_white_alpha(220));
                painter.circle_stroke(
                    sp,
                    if is_target { 5.0 } else { 3.0 },
                    egui::Stroke::new(
                        if is_target { 1.5 } else { 1.0 },
                        if is_target { theme.accent() } else { Color32::from_black_alpha(150) },
                    ),
                );
                if cam_dist <= card_dist {
                    draw_machine_card(painter, theme, sp, label);
                } else if cam_dist <= name_dist {
                    text_shadowed(painter, sp + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, &label.name, 12.0, Color32::WHITE);
                }
            }
            // Walk-up interaction prompt at the crosshair (v0.431): looking at a machine
            // within reach shows [E] open/close.
            if let Some(i) = state.targeted_machine {
                if let Some(label) = state.machine_labels.get(i) {
                    let verb = if state.selected_machine == Some(i) { "close" } else { "open" };
                    text_shadowed(
                        painter,
                        Pos2::new(center.x, center.y + 22.0),
                        Align2::CENTER_TOP,
                        &format!("[E] {} {}", verb, label.name),
                        13.0,
                        theme.accent(),
                    );
                }
            }
            // Pinned (E-opened) machine card: fixed top-left, stays until E again.
            if let Some(i) = state.selected_machine {
                if let Some(label) = state.machine_labels.get(i) {
                    let size = machine_card_size(label);
                    let card = Rect::from_min_size(Pos2::new(16.0, 56.0), size);
                    draw_machine_card_body(painter, theme, card, label, true);
                    text_shadowed(painter, Pos2::new(card.left() + 2.0, card.bottom() + 9.0), Align2::LEFT_CENTER, "[E] close", 10.0, theme.text_muted());
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

/// Draw text with a black OUTLINE (stroke) so it stays legible over any 3D background
/// without needing a panel behind it. Renders the text in black at 8 surrounding offsets,
/// then the colored text on top. (v0.444: was a 1px drop-shadow.)
fn text_shadowed(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
) {
    let font = FontId::proportional(size);
    let outline = Color32::from_black_alpha(200);
    const O: f32 = 1.2;
    for (dx, dy) in [
        (-O, -O), (0.0, -O), (O, -O),
        (-O, 0.0), (O, 0.0),
        (-O, O), (0.0, O), (O, O),
    ] {
        painter.text(pos + Vec2::new(dx, dy), anchor, text, font.clone(), outline);
    }
    painter.text(pos, anchor, text, font, color);
}

/// Build-mode CAD dimension overlay (v0.545): each interior wall's length at its midpoint, the angle
/// where two walls meet at a corner, and -- while drawing -- a live length readout by the cursor.
/// Paint-only, reusing world_to_screen + text_shadowed. Only drawn in the construction editor.
pub fn draw_construction_overlay(ctx: &egui::Context, theme: &Theme, state: &GuiState, view_proj: Mat4) {
    let Some(hs) = &state.home_structure else { return };
    let screen = ctx.screen_rect();
    let y = hs.height * 0.5; // label height: mid-wall
    let norm = |dx: f32, dz: f32| -> Option<(f32, f32)> {
        let l = (dx * dx + dz * dz).sqrt();
        if l > 1e-4 { Some((dx / l, dz / l)) } else { None }
    };
    Area::new(egui::Id::new("construction_dim_overlay"))
        .fixed_pos([0.0, 0.0])
        .interactable(false)
        .show(ctx, |ui| {
            ui.allocate_rect(screen, egui::Sense::hover());
            let painter = ui.painter();
            // Each interior wall's length at its midpoint (the selected one in accent).
            for (i, wall) in hs.walls.iter().enumerate() {
                let mid = Vec3::new((wall.a.0 + wall.b.0) * 0.5, y, (wall.a.1 + wall.b.1) * 0.5);
                let len = ((wall.b.0 - wall.a.0).powi(2) + (wall.b.1 - wall.a.1).powi(2)).sqrt();
                let col = if state.construction_wall_selected == Some(i) { theme.accent() } else { theme.text_primary() };
                if let Some(sp) = world_to_screen(mid, view_proj, screen) {
                    text_shadowed(painter, sp, Align2::CENTER_CENTER, &format!("{len:.2} m"), 13.0, col);
                }
            }
            // Angle where two walls meet at a shared corner.
            let mut seen: Vec<(f32, f32)> = Vec::new();
            for wall in &hs.walls {
                for c in [wall.a, wall.b] {
                    if seen.iter().any(|s| (s.0 - c.0).abs() < 0.05 && (s.1 - c.1).abs() < 0.05) {
                        continue;
                    }
                    seen.push(c);
                    let mut dirs: Vec<(f32, f32)> = Vec::new();
                    for w in &hs.walls {
                        if (w.a.0 - c.0).abs() < 0.05 && (w.a.1 - c.1).abs() < 0.05 {
                            if let Some(d) = norm(w.b.0 - c.0, w.b.1 - c.1) { dirs.push(d); }
                        }
                        if (w.b.0 - c.0).abs() < 0.05 && (w.b.1 - c.1).abs() < 0.05 {
                            if let Some(d) = norm(w.a.0 - c.0, w.a.1 - c.1) { dirs.push(d); }
                        }
                    }
                    if dirs.len() >= 2 {
                        let dot = (dirs[0].0 * dirs[1].0 + dirs[0].1 * dirs[1].1).clamp(-1.0, 1.0);
                        let ang = dot.acos().to_degrees();
                        if let Some(sp) = world_to_screen(Vec3::new(c.0, y, c.1), view_proj, screen) {
                            text_shadowed(painter, sp + Vec2::new(0.0, 16.0), Align2::CENTER_TOP, &format!("{ang:.0} deg"), 12.0, theme.warning());
                        }
                    }
                }
            }
            // Feature distances (v0.547): on the SELECTED wall, the clear GAP in each span between
            // the wall ends + the openings (so you can place a door exactly N m from a window / wall
            // end). Labelled near the floor at each gap's midpoint.
            if let Some(sel) = state.construction_wall_selected {
                if let Some(wall) = hs.walls.get(sel) {
                    if !wall.openings.is_empty() {
                        let (ax, az) = wall.a;
                        let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
                        let len = (dx * dx + dz * dz).sqrt();
                        if let Some((ux, uz)) = norm(dx, dz).filter(|_| len > 1e-4) {
                            let mut ivals: Vec<(f32, f32)> = wall
                                .openings
                                .iter()
                                .map(|o| (o.at.clamp(0.0, len), (o.at + o.width).clamp(0.0, len)))
                                .collect();
                            ivals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                            let mut cursor = 0.0f32;
                            for (s, e) in ivals.iter().chain(std::iter::once(&(len, len))) {
                                let gap = s - cursor;
                                if gap > 0.05 {
                                    let mid = cursor + gap * 0.5;
                                    let world = Vec3::new(ax + ux * mid, 0.35, az + uz * mid);
                                    if let Some(sp) = world_to_screen(world, view_proj, screen) {
                                        text_shadowed(painter, sp, Align2::CENTER_CENTER, &format!("{gap:.2} m"), 11.0, theme.text_secondary());
                                    }
                                }
                                cursor = e.max(cursor);
                            }
                        }
                    }
                }
            }
            // Live readout while drawing: the pending segment length by the cursor.
            if let (Some(s), Some(cur)) = (state.construction_wall_start, state.construction_cursor_world) {
                let len = ((cur.0 - s.0).powi(2) + (cur.1 - s.1).powi(2)).sqrt();
                if let Some(sp) = world_to_screen(Vec3::new(cur.0, y, cur.1), view_proj, screen) {
                    text_shadowed(painter, sp + Vec2::new(22.0, -14.0), Align2::LEFT_CENTER, &format!("{len:.2} m"), 15.0, theme.accent());
                }
            }
        });
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
const CARD_ROW_H: f32 = 15.0;
const CARD_PAD: f32 = 7.0;
const CARD_W: f32 = 150.0;

fn machine_card_size(label: &crate::gui::MachineLabel) -> Vec2 {
    let h = CARD_PAD * 2.0 + CARD_ROW_H * (1.0 + label.stats.len() as f32) + 2.0;
    Vec2::new(CARD_W, h)
}

/// Draw the card body (background, name header, two-column stat rows) into `card`.
/// `pinned` gives it an accent border (the E-opened station).
fn draw_machine_card_body(painter: &egui::Painter, theme: &Theme, card: Rect, label: &crate::gui::MachineLabel, pinned: bool) {
    painter.rect_filled(card, Rounding::same(5), Color32::from_black_alpha(if pinned { 228 } else { 205 }));
    let border = if pinned { theme.accent() } else { Color32::from_white_alpha(45) };
    painter.rect_stroke(card, Rounding::same(5), egui::Stroke::new(1.0, border), egui::StrokeKind::Outside);
    let mut y = card.top() + CARD_PAD;
    painter.text(Pos2::new(card.left() + CARD_PAD, y), Align2::LEFT_TOP, &label.name, FontId::proportional(13.0), Color32::WHITE);
    y += CARD_ROW_H + 3.0;
    for s in &label.stats {
        let color = stat_status_color(&s.status, theme);
        let icon_rect = Rect::from_min_size(Pos2::new(card.left() + CARD_PAD, y), Vec2::splat(12.0));
        paint_stat_icon(painter, icon_rect, &s.kind, color);
        painter.text(Pos2::new(card.right() - CARD_PAD, y + 0.5), Align2::RIGHT_TOP, &s.value, FontId::proportional(11.0), theme.text_secondary());
        y += CARD_ROW_H;
    }
}

/// Floating card next to a machine's screen dot.
fn draw_machine_card(painter: &egui::Painter, theme: &Theme, anchor: Pos2, label: &crate::gui::MachineLabel) {
    let size = machine_card_size(label);
    let card = Rect::from_min_size(Pos2::new(anchor.x + 12.0, anchor.y - size.y * 0.5), size);
    draw_machine_card_body(painter, theme, card, label, false);
    // A connector tick from the dot to the card.
    painter.line_segment([anchor + Vec2::new(4.0, 0.0), Pos2::new(card.left(), anchor.y)], egui::Stroke::new(1.0, Color32::from_white_alpha(70)));
}

/// Map a stat kind to its painted icon, drawn in `color` (the status color).
fn paint_stat_icon(painter: &egui::Painter, rect: Rect, kind: &str, color: Color32) {
    use crate::gui::widgets::icons;
    match kind {
        "power" => icons::paint_bolt(painter, rect, color),
        "water" | "fuel" => icons::paint_droplet(painter, rect, color),
        "heat" => icons::paint_flame(painter, rect, color),
        "nutrient" => icons::paint_leaf(painter, rect, color),
        "storage" => icons::paint_box(painter, rect, color),
        "progress" => icons::paint_cog(painter, rect, color),
        _ => {
            painter.circle_filled(rect.center(), rect.width() * 0.22, color);
        }
    }
}
