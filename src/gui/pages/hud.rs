//! In-game HUD: health bar, hotbar, crosshair, compass, FPS, weather, day/night.

use egui::{Align2, Area, Color32, FontId, Pos2, Rect, RichText, Rounding, Vec2};
use glam::{Mat4, Vec3};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::updater::UpdateState;

/// Death screen (v0.745, loop-map rung 1): a full-screen dim + the cause of
/// death + a Respawn button, drawn at ctx level OVER every page while
/// `player_death_cause` is Some. Respawn is handled by lib.rs (teleport to the
/// spawn room, reset vitals, remove Dead) via `pending_respawn`.
pub fn draw_death_screen(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(cause) = state.player_death_cause.clone() else { return };
    let screen = ctx.screen_rect();
    // Dim the world so the moment reads instantly (paint-only layer).
    Area::new(egui::Id::new("death_dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(false)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_black_alpha(190));
        });
    // Centered card with the cause + the one action. Created after the dim in
    // the same order, so it draws (and clicks) on top.
    Area::new(egui::Id::new("death_card"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Frame::window(&ctx.style())
                .fill(theme.bg_card())
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("YOU DIED")
                                .size(theme.font_size_heading * 1.6)
                                .strong()
                                .color(theme.danger()),
                        );
                        ui.add_space(theme.spacing_sm);
                        ui.label(
                            RichText::new(format!("Cause: {cause}"))
                                .size(theme.font_size_body)
                                .color(theme.text_primary()),
                        );
                        ui.add_space(theme.spacing_xs);
                        ui.label(
                            RichText::new(
                                "You wake in the respawner. Nothing was lost, but the \
                                 body remembers: keep fed, hydrated, warm, and breathing.",
                            )
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                        ui.add_space(theme.spacing_md);
                        if crate::gui::widgets::Button::primary("Respawn").show(ui, theme) {
                            state.pending_respawn = true;
                        }
                        ui.add_space(theme.spacing_xs);
                    });
                });
        });
}

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
            // ── Credits (under the health bar, v0.747) ──
            text_shadowed(
                painter,
                Pos2::new(16.0, 38.0),
                Align2::LEFT_TOP,
                &format!("{} CR", state.wallet_credits),
                12.0,
                theme.accent(),
            );

            // ── Shared-world co-presence (top-left, under credits, v0.774) ──
            // Only shown once we've joined the relay's shared game world (in-world
            // + connected). Makes the mission-critical co-presence legible: you can
            // see you're in the VPS shared session and watch when someone else
            // joins. The roster comes from GuiState, mirrored from the ECS each
            // frame by the multiplayer block in lib.rs.
            if state.copresence_active {
                // One shared URL-to-display-name formatter (v0.779): the launcher
                // row and chat sidebar use server_display_name; a third inline
                // trim chain here would drift on the next URL-shape change.
                let header = if state.server_url.is_empty() {
                    "Shared world".to_string()
                } else {
                    let host = crate::gui::pages::chat::server_display_name(&state.server_url);
                    format!("Shared world · {host}")
                };
                text_shadowed(painter, Pos2::new(16.0, 56.0), Align2::LEFT_TOP, &header, 12.0, theme.accent());
                let others = state.copresence_names.len();
                let (roster, col) = if others == 0 {
                    ("no one else here yet".to_string(), theme.text_muted())
                } else {
                    // truncate_chars (below in this file) is char-safe AND trims
                    // a dangling ", " before the ellipsis (v0.779 reuse fix).
                    let names = truncate_chars(&state.copresence_names.join(", "), 48);
                    (format!("{others} here: {names}"), theme.success())
                };
                text_shadowed(painter, Pos2::new(16.0, 72.0), Align2::LEFT_TOP, &roster, 11.0, col);
            }

            // ── FPS counter (top-right) ──
            text_shadowed(
                painter,
                Pos2::new(screen.right() - 16.0, 16.0),
                Align2::RIGHT_TOP,
                &format!("{:.0} FPS", state.fps),
                12.0,
                theme.text_muted(),
            );

            // ── Play-mode tag (task #50, left of the FPS corner) ──
            // Screenshot honesty: any non-Normal mode is labeled, ALWAYS --
            // including (especially) in a shared world, where other players'
            // screenshots must be able to tell a creative build from survival
            // play. Dev tools currently keep working while copresence_active
            // (the relay is the authority on shared state anyway); per-player
            // SERVER-ENFORCED permissions are the documented follow-up once
            // real players exist -- until then this tag is the honesty layer.
            if state.settings.play_mode != crate::config::PlayMode::Normal {
                let tag = match state.settings.play_mode {
                    crate::config::PlayMode::Dev => "DEV",
                    _ => "CREATIVE",
                };
                text_shadowed(
                    painter,
                    // Sits just left of the FPS text ("999 FPS" at size 12 is
                    // ~50 px wide, right-aligned at right-16).
                    Pos2::new(screen.right() - 74.0, 16.0),
                    Align2::RIGHT_TOP,
                    tag,
                    12.0,
                    theme.warning(),
                );
            }

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
                let day_color = if gt.is_daytime { theme.warning() } else { theme.info() };
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

            // ── Dev travel indicator (v0.791.x, under the compass) ──
            // Shown whenever fly mode is on or the FTL multiplier is above 1x,
            // so the operator always knows why movement behaves differently.
            if state.dev_fly_mode || state.dev_fly_speed_mult > 1.0 {
                let mut label = format!(
                    "FLY {}",
                    crate::dev_travel::format_multiplier(state.dev_fly_speed_mult)
                );
                if !state.dev_fly_mode {
                    label.push_str(" (fly mode off)");
                } else if state.dev_fly_speed_mult
                    > crate::renderer::camera::LOCAL_FLY_MULT_MAX
                {
                    label.push_str(" FTL - ship flying");
                }
                if state.dev_travel_away {
                    label.push_str(" - away from home");
                }
                text_shadowed(
                    painter,
                    Pos2::new(center.x, compass_y + 20.0),
                    Align2::CENTER_TOP,
                    &label,
                    12.0,
                    theme.warning(),
                );
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
            // ── Crew NPC nameplates (v0.667): name + live chore over each crew member ──
            // Same world_to_screen + text_shadowed path as machine labels. The name shows
            // within CREW_NAME_DIST; the activity line joins within CREW_ACTIVITY_DIST so
            // the HUD stays quiet at range. No room occlusion on purpose: crew WALK between
            // rooms (a room filter would blink the plate at every doorway), and the amber
            // figure itself is the far-range marker, so no dot LOD either.
            for label in &state.crew_labels {
                let cam_dist = (label.pos - cam_pos).length();
                let Some((name, activity)) = crew_label_lines(&label.name, &label.activity, cam_dist) else {
                    continue;
                };
                let Some(sp) = world_to_screen(label.pos, view_proj, screen) else { continue };
                // Name above the anchor, activity below it: the pair stays centered on the
                // head no matter how long the chore text is.
                text_shadowed(painter, sp, Align2::CENTER_BOTTOM, &name, 12.0, Color32::WHITE);
                if let Some(act) = activity {
                    // Accent while actively working at the chore site; muted while walking
                    // to it, so the state reads at a glance.
                    let col = if label.working { theme.accent() } else { theme.text_secondary() };
                    text_shadowed(painter, sp + Vec2::new(0.0, 2.0), Align2::CENTER_TOP, &act, 10.0, col);
                }
            }
            // Crew NPC talk prompt (v0.797): looking at a crew member within
            // talk range shows "[E] Talk to X". Owns the +22 crosshair slot
            // while set -- the machine/door prompts below yield to it, which
            // matches the E chain in lib.rs (a faced person outranks the
            // machine behind them).
            if !state.npc_prompt.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 22.0),
                    Align2::CENTER_TOP,
                    &state.npc_prompt,
                    13.0,
                    theme.accent(),
                );
            }
            // Walk-up interaction prompt at the crosshair (v0.431): looking at a machine
            // within reach shows [E] open/close.
            if state.npc_prompt.is_empty() {
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
            }
            // Door control panel prompt at the crosshair (v0.567): looking at a panel within reach
            // shows [E] open/close (or "locked"). Precomputed in the walk-up block in lib.rs.
            if !state.control_panel_prompt.is_empty() && state.npc_prompt.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 22.0),
                    Align2::CENTER_TOP,
                    &state.control_panel_prompt,
                    13.0,
                    theme.accent(),
                );
            }
            // Vehicle prompt (Stage 3 take-over, v0.690): "[E] drive X" at the
            // crosshair, or "[E] exit vehicle" while driving.
            if !state.vehicle_prompt.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 38.0),
                    Align2::CENTER_TOP,
                    &state.vehicle_prompt,
                    13.0,
                    theme.accent(),
                );
            }
            // Livestock prompt (v0.751): "[E] collect Egg (Chicken)" when the
            // faced animal is ready, or the regrow countdown while it is not.
            if !state.livestock_prompt.is_empty() {
                let ready = state.livestock_prompt.starts_with("[E]");
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 54.0),
                    Align2::CENTER_TOP,
                    &state.livestock_prompt,
                    13.0,
                    if ready { theme.accent() } else { theme.text_secondary() },
                );
            }
            // Collect feedback ("+1 Egg from Chicken"); lib.rs fades it after 3 s.
            if !state.livestock_notice.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 72.0),
                    Align2::CENTER_TOP,
                    &state.livestock_notice,
                    13.0,
                    theme.text_primary(),
                );
            }
            // Pinned (E-opened) machine card: CENTERED on screen (upper third,
            // clear of the crosshair) — was pinned tiny at the top-left, which
            // the operator reported as invisible-in-practice (v0.730).
            if let Some(i) = state.selected_machine {
                if let Some(label) = state.machine_labels.get(i) {
                    let size = machine_card_size(label);
                    let card = Rect::from_min_size(
                        Pos2::new(center.x - size.x * 0.5, screen.top() + screen.height() * 0.22),
                        size,
                    );
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

                // No slot numbers here: the 1-9 keys belong to the ability
                // bar above (numbers on a display-only row would lie).
            }

            // ── Ability bar (v0.754) ── the first nine castable abilities,
            // above the inventory hotbar; 1-9 casts the matching slot (the
            // same order the abilities bridge sorts: castable first, by name).
            {
                let castable: Vec<_> = state
                    .abilities
                    .iter()
                    .filter(|a| a.castable_now)
                    .take(9)
                    .collect();
                if !castable.is_empty() {
                    let a_size = 32.0;
                    let a_gap = 4.0;
                    let a_total = castable.len() as f32 * a_size
                        + (castable.len() - 1) as f32 * a_gap;
                    let ax0 = center.x - a_total / 2.0;
                    let ay = start_y - a_size - 8.0;
                    for (i, ab) in castable.iter().enumerate() {
                        let rect = Rect::from_min_size(
                            Pos2::new(ax0 + i as f32 * (a_size + a_gap), ay),
                            Vec2::splat(a_size),
                        );
                        painter.rect_filled(rect, Rounding::same(4), Color32::from_black_alpha(140));
                        painter.rect_stroke(rect, Rounding::same(4), egui::Stroke::new(1.0, theme.border()), egui::StrokeKind::Outside);
                        let ready = ab.cooldown_remaining <= 0.0;
                        // Cooldown sweep: a dark overlay that DRAINS downward
                        // as the ability recharges (full dark right after cast).
                        if !ready && ab.cooldown_s > 0.0 {
                            let frac = (ab.cooldown_remaining / ab.cooldown_s).clamp(0.0, 1.0);
                            let h = rect.height() * frac;
                            let overlay = Rect::from_min_max(
                                Pos2::new(rect.left(), rect.bottom() - h),
                                rect.right_bottom(),
                            );
                            painter.rect_filled(overlay, Rounding::same(4), Color32::from_black_alpha(160));
                        }
                        // Two-word initials ("First Aid" -> FA).
                        let initials: String = ab
                            .name
                            .split_whitespace()
                            .filter_map(|w| w.chars().next())
                            .take(2)
                            .collect();
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            initials,
                            FontId::proportional(13.0),
                            if ready { theme.accent() } else { theme.text_muted() },
                        );
                        // The key that casts this slot.
                        painter.text(
                            rect.left_top() + Vec2::new(3.0, 1.0),
                            Align2::LEFT_TOP,
                            format!("{}", i + 1),
                            FontId::proportional(9.0),
                            theme.text_muted(),
                        );
                        if !ready {
                            painter.text(
                                rect.right_bottom() + Vec2::new(-3.0, -1.0),
                                Align2::RIGHT_BOTTOM,
                                format!("{:.0}", ab.cooldown_remaining.max(1.0)),
                                FontId::proportional(9.0),
                                theme.text_muted(),
                            );
                        }
                    }
                    // Cast feedback above the bar (lib.rs fades it after 4 s).
                    if !state.ability_status.is_empty() {
                        text_shadowed(
                            painter,
                            Pos2::new(center.x, ay - 14.0),
                            Align2::CENTER_BOTTOM,
                            &state.ability_status,
                            12.0,
                            theme.text_primary(),
                        );
                    }
                }
            }

            // ── In-game chat feed (bottom-left, v0.771; follows active channel v0.772) ──
            // the SAME relay channel you see on the Chat page and the website,
            // live while you play. The app auto-connects to your saved server
            // (united-humanity.us by default) whenever your identity seed is
            // unlocked, so this fills without opening the Chat page. Read-only
            // here; press Enter for the interactive panel to type + switch
            // channels. Suppressed while that panel is open (it shows instead).
            // Follows `chat_active_channel`, so switching channels there (or on
            // the Chat page) updates this header + messages. Paint-only.
            // `hud_chat_feed_visible` (increment 1c) is the user's off switch,
            // toggled from the in-world panel's Options tab (persisted).
            if !state.chat_input_active && state.hud_chat_feed_visible {
                // PRIVACY (v0.779): the always-on feed shows PUBLIC channels
                // only. A DM or group conversation left active on the Chat page
                // must never paint private text on the world overlay (visible
                // on stream, over the shoulder). Fall back to #general.
                let active_raw = state.chat_active_channel.clone();
                let active = if active_raw.starts_with("dm:") || active_raw.starts_with("p2pgroup:") {
                    "general".to_string()
                } else {
                    active_raw
                };
                let label = crate::gui::pages::chat::channel_display_label(&active);
                let recent: Vec<&crate::gui::ChatMessage> = state
                    .chat_messages
                    .iter()
                    .filter(|m| m.channel == active)
                    .rev()
                    .take(7)
                    .collect();
                let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
                // Status line: what the relay link is doing right now, so an
                // offline / locked-identity state is legible in-world instead
                // of a silently empty box.
                let status = if connected {
                    format!("{label} - connected")
                } else if !state.ws_status.is_empty() {
                    format!("{label} - {}", state.ws_status)
                } else {
                    format!("{label} - offline")
                };
                let line_h = 15.0;
                let width = 420.0;
                let rows = recent.len().max(1);
                let box_h = line_h * (rows as f32 + 1.0) + 10.0;
                let x0 = 14.0;
                let bottom = screen.bottom() - 12.0;
                let top = bottom - box_h;
                // Legibility backing behind the feed.
                let bg = Rect::from_min_max(
                    Pos2::new(x0 - 6.0, top - 4.0),
                    Pos2::new(x0 + width, bottom + 2.0),
                );
                painter.rect_filled(bg, Rounding::same(4), Color32::from_black_alpha(120));
                // Header / connection status at the top of the box.
                let status_col = if connected { theme.success() } else { theme.warning() };
                text_shadowed(painter, Pos2::new(x0, top), Align2::LEFT_TOP, &status, 11.0, status_col);
                // Messages oldest -> newest downward, newest just above the bottom.
                for (i, m) in recent.iter().rev().enumerate() {
                    let y = top + line_h * (i as f32 + 1.0) + 2.0;
                    let name = if m.sender_name.is_empty() { "?" } else { m.sender_name.as_str() };
                    let mut line = format!("{name}: {}", m.content);
                    if line.chars().count() > 66 {
                        line = format!("{}...", line.chars().take(63).collect::<String>());
                    }
                    text_shadowed(painter, Pos2::new(x0, y), Align2::LEFT_TOP, &line, 11.0, theme.text_secondary());
                }
                if recent.is_empty() {
                    let hint = if connected {
                        "No messages yet - say hi (Enter opens chat)."
                    } else {
                        "Sign in (Settings > Security > Unlock) to join chat."
                    };
                    text_shadowed(painter, Pos2::new(x0, top + line_h + 2.0), Align2::LEFT_TOP, hint, 11.0, theme.text_muted());
                }
            }

            // ── Update notification toast (top-right, below weather) ──
            if let UpdateState::Available { ref version, .. } = state.updater.state {
                let toast_rect = Rect::from_min_size(
                    Pos2::new(screen.right() - 260.0, 64.0),
                    Vec2::new(244.0, 44.0),
                );
                // Panel-colored toast fill. RGB comes from the theme (bg_panel);
                // only the ~90% alpha is intentional here so the toast reads over the 3D scene.
                let toast_bg = theme.bg_panel();
                painter.rect_filled(toast_rect, Rounding::same(6), Color32::from_rgba_premultiplied(toast_bg.r(), toast_bg.g(), toast_bg.b(), 230));
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

/// Crew nameplate visibility (v0.667). The NAME shows out to this range; beyond it the
/// amber figure alone marks the crew member (no dot LOD -- the figure IS the marker).
const CREW_NAME_DIST: f32 = 40.0;
/// The chore ACTIVITY line joins the name within this range, so chore text only appears
/// once you are close enough to plausibly care what they are doing.
const CREW_ACTIVITY_DIST: f32 = 15.0;
/// Longest activity line drawn before truncation, in characters. Chore labels from
/// data/npc/chores.ron run ~20-35 chars today; this only guards pathological data.
const CREW_ACTIVITY_MAX_CHARS: usize = 48;

/// Which nameplate lines a crew member shows at `cam_dist` meters.
/// `None` = nothing (out of range, or a nameless NPC). Otherwise the name plus,
/// within CREW_ACTIVITY_DIST, the (truncated) activity line.
fn crew_label_lines(name: &str, activity: &str, cam_dist: f32) -> Option<(String, Option<String>)> {
    if name.is_empty() || !cam_dist.is_finite() || cam_dist > CREW_NAME_DIST {
        return None;
    }
    let activity_line = if cam_dist <= CREW_ACTIVITY_DIST && !activity.is_empty() {
        Some(truncate_chars(activity, CREW_ACTIVITY_MAX_CHARS))
    } else {
        None
    };
    Some((name.to_string(), activity_line))
}

/// Truncate to at most `max` characters, replacing the tail with "..." when cut.
/// Counts CHARS (not bytes) so multibyte text never splits a codepoint.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{}...", kept.trim_end())
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
    let Some(hs) = crate::ship::ship_structure::zone_body(&state.ship_structure, state.construction_zone) else { return };
    // The ACTIVE zone's world origin (v0.754): body coords are zone-local, labels paint in world.
    let zo = crate::ship::ship_structure::zone_origin(&state.ship_structure, state.construction_zone);
    let screen = ctx.screen_rect();
    let y = hs.height * 0.5; // label height: mid-wall
    let norm = |dx: f32, dz: f32| -> Option<(f32, f32)> {
        let l = (dx * dx + dz * dz).sqrt();
        if l > 1e-4 { Some((dx / l, dz / l)) } else { None }
    };
    // Paint-ONLY layer (v0.548): a layer_painter, NOT an interactable Area. An Area -- even
    // non-interactable -- registers a full-screen region under the pointer that makes
    // wants_pointer_input read false over the editor's side panels, so their clicks route to the
    // camera instead (the "panel shows but won't click" regression). A layer_painter only paints and
    // never participates in input routing. (`p` owns it; `painter` is a ref so the body is unchanged.)
    {
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("construction_dim_overlay"),
        ));
        let painter = &p;
            // Each interior wall's length at the BOTTOM middle of the wall (v0.559, by the floor like
            // the gizmo orb -- was mid-wall height).
            for (i, wall) in hs.walls.iter().enumerate() {
                let mid = Vec3::new((wall.a.0 + wall.b.0) * 0.5, 0.06, (wall.a.1 + wall.b.1) * 0.5) + zo;
                let len = ((wall.b.0 - wall.a.0).powi(2) + (wall.b.1 - wall.a.1).powi(2)).sqrt();
                let col = if state.construction_wall_selected == Some(i) { theme.accent() } else { theme.text_primary() };
                if let Some(sp) = world_to_screen(mid, view_proj, screen) {
                    text_shadowed(painter, sp, Align2::CENTER_CENTER, &format!("{len:.2} m"), 13.0, col);
                }
            }
            // Pie-slice angles at each corner (v0.551): the angle of EACH slice between consecutive
            // walls (sorted by heading), labelled ON THE GROUND at the slice's midpoint -- so a join
            // of 2+ walls shows ALL its angles, not just one, and the number sits in the slice it
            // measures instead of on the (confusing) wall edge. Pairs with the ground angle-ring.
            const RING_R: f32 = 1.1;
            let mut seen: Vec<(f32, f32)> = Vec::new();
            for wall in &hs.walls {
                for c in [wall.a, wall.b] {
                    if seen.iter().any(|s| (s.0 - c.0).abs() < 0.05 && (s.1 - c.1).abs() < 0.05) {
                        continue;
                    }
                    seen.push(c);
                    let mut headings: Vec<f32> = Vec::new();
                    for w in &hs.walls {
                        if (w.a.0 - c.0).abs() < 0.05 && (w.a.1 - c.1).abs() < 0.05 {
                            headings.push((w.b.1 - c.1).atan2(w.b.0 - c.0));
                        }
                        if (w.b.0 - c.0).abs() < 0.05 && (w.b.1 - c.1).abs() < 0.05 {
                            headings.push((w.a.1 - c.1).atan2(w.a.0 - c.0));
                        }
                    }
                    // Also count the box HULL edges at this corner (v0.555): when an interior wall
                    // ends ON the fixed perimeter, show the angle it makes with the hull on each side
                    // (the hull was invisible to the angle math before, so hull joins showed nothing).
                    let (bw, bd) = (hs.width, hs.depth);
                    let he = 0.05;
                    if c.1.abs() < he || (c.1 - bd).abs() < he {
                        if c.0 < bw - he { headings.push(0.0); }
                        if c.0 > he { headings.push(std::f32::consts::PI); }
                    }
                    if c.0.abs() < he || (c.0 - bw).abs() < he {
                        if c.1 < bd - he { headings.push(std::f32::consts::FRAC_PI_2); }
                        if c.1 > he { headings.push(-std::f32::consts::FRAC_PI_2); }
                    }
                    if headings.len() < 2 {
                        continue;
                    }
                    headings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let nh = headings.len();
                    for i in 0..nh {
                        let a0 = headings[i];
                        let a1 = if i + 1 < nh { headings[i + 1] } else { headings[0] + std::f32::consts::TAU };
                        let slice = a1 - a0;
                        if slice < 0.02 {
                            continue;
                        }
                        let mid = a0 + slice * 0.5;
                        let world = Vec3::new(c.0 + mid.cos() * RING_R * 0.62, 0.12, c.1 + mid.sin() * RING_R * 0.62) + zo;
                        // Drop labels that fall outside the box footprint (e.g. the exterior slice at a
                        // hull join) so only the meaningful in-room angles show.
                        if world.x < zo.x - 0.3 || world.x > zo.x + bw + 0.3 || world.z < zo.z - 0.3 || world.z > zo.z + bd + 0.3 {
                            continue;
                        }
                        if let Some(sp) = world_to_screen(world, view_proj, screen) {
                            text_shadowed(painter, sp, Align2::CENTER_CENTER, &format!("{:.0} deg", slice.to_degrees()), 12.0, theme.warning());
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
                                    let world = Vec3::new(ax + ux * mid, 0.35, az + uz * mid) + zo;
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
            // Door / window LABELS (v0.554): each opening's style + lock state, floating at the
            // opening so you can read what every door is at a glance (the operator's "text on doors").
            for wall in &hs.walls {
                let (ax, az) = wall.a;
                let (wdx, wdz) = (wall.b.0 - ax, wall.b.1 - az);
                let wlen = (wdx * wdx + wdz * wdz).sqrt();
                if wlen < 1e-4 {
                    continue;
                }
                let (ux, uz) = (wdx / wlen, wdz / wlen);
                for op in &wall.openings {
                    let s = op.at + op.width * 0.5;
                    let world = Vec3::new(ax + ux * s, op.sill + op.height * 0.5, az + uz * s) + zo;
                    if let Some(sp) = world_to_screen(world, view_proj, screen) {
                        let lock = if op.locked { " [locked]" } else { "" };
                        text_shadowed(painter, sp, Align2::CENTER_CENTER, &format!("{}{}", op.style, lock), 11.0, theme.text_primary());
                    }
                }
            }
            // Live readout while drawing: the pending segment length by the cursor.
            if let (Some(s), Some(cur)) = (state.construction_wall_start, state.construction_cursor_world) {
                let len = ((cur.0 - s.0).powi(2) + (cur.1 - s.1).powi(2)).sqrt();
                if let Some(sp) = world_to_screen(Vec3::new(cur.0, y, cur.1) + zo, view_proj, screen) {
                    text_shadowed(painter, sp + Vec2::new(22.0, -14.0), Align2::LEFT_CENTER, &format!("{len:.2} m"), 15.0, theme.accent());
                }
            }
    }
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

/// Interactive auto-recipe dropdown under the pinned machine card (v0.725,
/// info-window overhaul part 2 — the assembler's infinite-of-X vehicle
/// selector). SEPARATE from the paint-only HUD layer: that Area is
/// `.interactable(false)` and takes `&GuiState`, so this selector is its own
/// interactable Area with `&mut` — the pick lands in
/// `machine_card_recipe_pending`, which lib.rs applies to the machine
/// entity's AutoRefine next frame. Options are same-station recipes.csv rows
/// (published per frame by lib.rs); only shows when there's a real choice.
pub fn draw_machine_recipe_selector(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(i) = state.selected_machine else { return };
    // Show whenever the machine HAS an auto recipe (even a single option) —
    // seeing WHAT the machine builds matters as much as switching it, and a
    // one-option dropdown still tells you that (v0.730; was >= 2) — OR when
    // it's a container holding something (Take, v0.731) — OR when the player
    // carries something an EMPTY vessel accepts (Store, v0.733: the genset
    // drum starts empty; without this the deposit path would be unreachable).
    if state.machine_card_recipe_options.is_empty()
        && state.machine_card_container.is_none()
        && state.machine_card_storable.is_empty()
        && !state.machine_card_vendor
    {
        return;
    }
    // Card geometry mirrors the pinned card draw (now screen-centered, upper
    // third) so the selector sits right underneath the "[E] close" footer.
    let Some(card_h) = state.machine_labels.get(i).map(|l| machine_card_size(l).y) else { return };
    let screen = ctx.screen_rect();
    let sel_pos = Pos2::new(
        screen.center().x - CARD_W * 0.5,
        screen.top() + screen.height() * 0.22 + card_h + 20.0,
    );
    let cur_id = state.machine_card_recipe.clone().unwrap_or_default();
    let cur_name = state
        .machine_card_recipe_options
        .iter()
        .find(|(id, _)| *id == cur_id)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| cur_id.clone());
    Area::new(egui::Id::new("machine_recipe_selector"))
        .fixed_pos(sel_pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                if !state.machine_card_recipe_options.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Auto-build:")
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                        let mut picked = cur_id.clone();
                        egui::ComboBox::from_id_salt("machine_recipe_combo")
                            .selected_text(cur_name)
                            .width(170.0)
                            .show_ui(ui, |ui| {
                                for (id, name) in &state.machine_card_recipe_options {
                                    ui.selectable_value(&mut picked, id.clone(), name);
                                }
                            });
                        if picked != cur_id {
                            state.machine_card_recipe_pending = Some(picked);
                        }
                    });
                }
                // Container contents + Take (v0.731): deposits can be automatic
                // (harvest surplus routes in), this is the withdraw path.
                if let Some((_, name, qty)) = state.machine_card_container.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Holds: {}x {}", qty, name))
                                .size(theme.font_size_small)
                                .color(theme.text_primary()),
                        );
                        if crate::gui::widgets::Button::secondary("Take")
                            .tooltip("Move as much as fits into your backpack \
                                      (limited by your pack's free volume).")
                            .show(ui, theme)
                        {
                            state.machine_card_take_pending = true;
                        }
                    });
                }
                // Store (v0.733): per-item deposit buttons for compatible pack
                // items — how refined fuel gets from the pack into the genset
                // drum. Only accepted classes are listed, so a wrong-class
                // deposit (and its vessel damage) can't happen from here.
                let storable = state.machine_card_storable.clone();
                for (id, name, qty) in storable {
                    ui.horizontal(|ui| {
                        if crate::gui::widgets::Button::secondary(&format!("Store {}x {}", qty, name))
                            .tooltip("Pour this from your backpack into the vessel \
                                      (as much as fits).")
                            .show(ui, theme)
                        {
                            state.machine_card_store_pending = Some(id.clone());
                        }
                    });
                }
                // Trade (v0.747, ladder rung 3): the trading post's card opens
                // the vendor modal (buy at 125% of base, sell at 50%).
                if state.machine_card_vendor {
                    if crate::gui::widgets::Button::primary("Trade")
                        .tooltip("Buy and sell goods for credits.")
                        .show(ui, theme)
                    {
                        state.vendor_open = true;
                        state.vendor_status.clear();
                    }
                }
            });
        });
}

/// Walk-up NPC dialogue card (v0.797, operator: "I can't interact with NPCs
/// at all"). Same interactive-panel family as the recipe selector above and
/// the walk-up creature editor: its own interactable Area with `&mut` state,
/// screen-centered in the upper third like the pinned machine card. Shows the
/// NPC's name, live chore line, and ONE dialogue line at a time; More (or a
/// repeat E press, handled in lib.rs's modal key guard) cycles the lines.
/// Close / Esc / click-away / walking >4 m away all close it. Every line came
/// from the relay's NPC data via the welcome snapshot -- the client displays,
/// it never authors dialogue.
pub fn draw_npc_talk_card(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.npc_talk_target.is_none() {
        return;
    }
    let mut close = false;
    let mut more = false;
    let screen = ctx.screen_rect();
    let area = Area::new(egui::Id::new("npc_talk_card"))
        .fixed_pos(Pos2::new(
            screen.center().x - 180.0,
            screen.top() + screen.height() * 0.22,
        ))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&state.npc_talk_name)
                                .strong()
                                .color(theme.accent()),
                        );
                        // The live chore line doubles as the "who is this"
                        // role context ("Tending the hydroponic racks").
                        if !state.npc_talk_activity.is_empty() {
                            ui.label(
                                RichText::new(&state.npc_talk_activity)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        }
                    });
                    ui.separator();
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new(&state.npc_talk_line)
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);
                    ui.separator();
                    ui.horizontal(|ui| {
                        // Only offer More when there is more than one line to
                        // cycle -- a one-liner NPC would just repeat itself.
                        let line_count =
                            state.npc_talk_dialog.len().max(state.npc_talk_greetings.len());
                        if line_count > 1
                            && crate::gui::widgets::Button::secondary("More (E)").show(ui, theme)
                        {
                            more = true;
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if crate::gui::widgets::Button::secondary("Close (Esc)")
                                    .show(ui, theme)
                                {
                                    close = true;
                                }
                            },
                        );
                    });
                });
        });
    // Click-away closes, mirroring the in-world chat panel convention
    // (v0.773): a click OUTSIDE the card rect returns to gameplay.
    let clicked_outside = ctx.input(|i| i.pointer.any_click())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .map_or(false, |p| !area.response.rect.contains(p));
    if more {
        state.npc_talk_advance();
    }
    if close || clicked_outside {
        state.npc_talk_target = None;
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
        // "contents" = what's inside a typed container (v0.728) — same box
        // family as storage, reads as "the stuff in the box".
        "storage" | "contents" => icons::paint_box(painter, rect, color),
        "progress" => icons::paint_cog(painter, rect, color),
        _ => {
            painter.circle_filled(rect.center(), rect.width() * 0.22, color);
        }
    }
}

#[cfg(test)]
mod crew_label_tests {
    use super::*;

    #[test]
    fn close_range_shows_name_and_activity() {
        let got = crew_label_lines("Vex", "Taking reactor readings", 3.0);
        assert_eq!(
            got,
            Some(("Vex".to_string(), Some("Taking reactor readings".to_string())))
        );
    }

    #[test]
    fn mid_range_shows_name_only() {
        // Just past the activity radius: the chore line drops, the name stays.
        let got = crew_label_lines("Vex", "Taking reactor readings", CREW_ACTIVITY_DIST + 0.1);
        assert_eq!(got, Some(("Vex".to_string(), None)));
    }

    #[test]
    fn activity_boundary_is_inclusive() {
        let got = crew_label_lines("Vex", "Cleaning", CREW_ACTIVITY_DIST);
        assert_eq!(got, Some(("Vex".to_string(), Some("Cleaning".to_string()))));
    }

    #[test]
    fn beyond_name_range_shows_nothing() {
        assert_eq!(crew_label_lines("Vex", "Cleaning", CREW_NAME_DIST + 0.1), None);
        assert_eq!(crew_label_lines("Vex", "Cleaning", f32::NAN), None);
    }

    #[test]
    fn empty_name_or_activity_degrade_gracefully() {
        // Nameless NPC: no plate at all (never a floating activity with no owner).
        assert_eq!(crew_label_lines("", "Cleaning", 3.0), None);
        // Empty activity close up: name only, no blank second line.
        assert_eq!(crew_label_lines("Vex", "", 3.0), Some(("Vex".to_string(), None)));
    }

    #[test]
    fn long_activity_is_truncated_with_ellipsis() {
        let long = "Recalibrating the atmospheric scrubber intake manifold assembly unit";
        let (_, act) = crew_label_lines("Vex", long, 3.0).unwrap();
        let act = act.unwrap();
        assert!(act.ends_with("..."), "cut text must signal the cut: {act}");
        assert!(
            act.chars().count() <= CREW_ACTIVITY_MAX_CHARS,
            "stays within the cap: {} chars",
            act.chars().count()
        );
    }

    #[test]
    fn truncation_counts_chars_not_bytes() {
        // 60 multibyte chars (3 bytes each in UTF-8): byte-indexed slicing would panic
        // or split a codepoint; char-based truncation must stay well-formed.
        let s: String = std::iter::repeat('日').take(60).collect();
        let out = truncate_chars(&s, CREW_ACTIVITY_MAX_CHARS);
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= CREW_ACTIVITY_MAX_CHARS);
    }

    #[test]
    fn short_activity_is_untouched() {
        assert_eq!(truncate_chars("Watering the crops", 48), "Watering the crops");
    }
}
