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
            // NO `ui.allocate_rect(screen, Sense::hover())` here (BUG-076, 2026-09-18).
            // That line used to give the Area a full-screen widget rect "so it has a
            // size". egui's hit test does not care that the Area is non-interactable
            // or that the rect only senses hover: it walks widget rects top-down and
            // STOPS at the first one that covers the search area (hit_test.rs, the
            // `included_layers` loop), so every widget in a Background-order panel
            // underneath was dropped and could never be clicked. The F10 Cloud dev
            // SIDEBAR (a SidePanel, Background order, 2026-09-05) sat under this layer
            // and took no input at all; the construction editor's panels had hit the
            // same wall in v0.461, which is why lib.rs skips the HUD in build mode.
            // The HUD needs no rect: every element below paints at absolute screen
            // coordinates through the painter, whose clip rect is the whole screen
            // regardless of the Area's own (now empty) size. Pinned by
            // `cloud_dev::tests::a_sidebar_checkbox_click_flips_its_flag_under_the_hud`.
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

            // ── Survival needs + active quest (top-left, 2026-09-25) ──
            // The playable assessment's Tier A item 2: five things can kill
            // the player and the HUD showed one of them. Which rows show is
            // decided by `vital_rows` (pure, tested); drawing stacks them
            // under the credits, then the quest, then the co-presence block.
            let mut y = 56.0;
            for row in vital_rows(&state.vitals, state.settings.hud_vitals) {
                y = draw_vital_row(painter, theme, y, &row);
            }
            // Overloaded (BUG-136): what is wrong and what it does, so a slow
            // walk or a jump that never leaves the floor is never a mystery.
            if let Some(line) = state.carry.hud_line() {
                text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, &line, 11.0, theme.warning());
                y += 15.0;
            }
            if let Some(q) = state.quests.iter().find(|q| !q.completed) {
                y += 4.0;
                text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, &truncate_chars(&q.name, 48), 12.0, theme.accent());
                y += 15.0;
                let step = quest_line(&q.step_desc, q.step_index, q.step_total);
                text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, &truncate_chars(&step, QUEST_LINE_CHARS), 11.0, theme.text_secondary());
                y += 16.0;
            }

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
                text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, &header, 12.0, theme.accent());
                let others = state.copresence_names.len();
                let (roster, col) = if others == 0 {
                    ("no one else here yet".to_string(), theme.text_muted())
                } else {
                    // truncate_chars (below in this file) is char-safe AND trims
                    // a dangling ", " before the ellipsis (v0.779 reuse fix).
                    let names = truncate_chars(&state.copresence_names.join(", "), 48);
                    (format!("{others} here: {names}"), theme.success())
                };
                text_shadowed(painter, Pos2::new(16.0, y + 16.0), Align2::LEFT_TOP, &roster, 11.0, col);
            } else if let Some(note) = &state.copresence_refused_note {
                // Not joining this server's shared world, and why, for as long as that holds
                // (ship homes 1b, the third review: the only word of it was a 12 s notice).
                text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, "Not in the shared world", 12.0, theme.warning());
                for (i, line) in wrap_words(note, 64).iter().enumerate() {
                    text_shadowed(painter, Pos2::new(16.0, y + 16.0 + 14.0 * i as f32), Align2::LEFT_TOP, line, 11.0, theme.text_secondary());
                }
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
                // The clock a player READS is local solar time at their
                // site (sun-clock fix, 2026-08-18): the raw game hour is
                // lon-0 time and at Silverdale printed "20:04" beside a
                // noon sun. Off-planet falls back to the global hour.
                let wall = gt.local_hour.unwrap_or(gt.hour);
                let time_str = format!(
                    "Day {} {:02}:{:02} {}",
                    gt.day_count + 1,
                    wall as u32,
                    ((wall.fract()) * 60.0) as u32,
                    gt.season,
                );
                let day_color = if gt.is_daytime { theme.warning() } else { theme.info() };
                let clock_pos = Pos2::new(screen.right() - 16.0, 32.0);
                if gt.is_daytime {
                    text_shadowed(painter, clock_pos, Align2::RIGHT_TOP, &format!("☀ {time_str}"), 11.0, day_color);
                } else {
                    // The moon is painted, left of the text: its glyph
                    // (U+263E) is in none of the UI fonts and drew as a box
                    // (2026-09-28).
                    text_shadowed(painter, clock_pos, Align2::RIGHT_TOP, &time_str, 11.0, day_color);
                    let text_w = painter.layout_no_wrap(time_str.clone(), FontId::proportional(11.0), day_color).size().x;
                    let icon = Rect::from_min_size(Pos2::new(clock_pos.x - text_w - 14.0, clock_pos.y + 1.0), egui::vec2(11.0, 11.0));
                    crate::gui::widgets::icons::paint_moon(painter, icon, day_color);
                }
            }

            // ── Weather indicator (below time) ──
            if let Some(ref w) = state.weather {
                // The air the player stands in (2026-09-27): `weather_line`.
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 46.0),
                    Align2::RIGHT_TOP,
                    &hud_weather_text(&state.vitals, w),
                    11.0,
                    theme.text_secondary(),
                );
                // Hazard proximity warning (v0.1038): a Vortex event's
                // core within 3x its hazard radius. Danger-colored line
                // right under the weather readout.
                if !w.warning.is_empty() {
                    text_shadowed(
                        painter,
                        Pos2::new(screen.right() - 16.0, 60.0),
                        Align2::RIGHT_TOP,
                        &w.warning,
                        12.0,
                        theme.danger(),
                    );
                }
            }

            // ── Power balance (live home electrical sim, below weather) ──
            // Generation climbs at noon, falls to zero at night; net flips green->red.
            if state.power_generation > 0.0 || state.power_consumption > 0.0 {
                // On the ship's reactor (Station-supplied) a deficit is covered, not a fault.
                let ship = state.power_ship;
                let col = if state.power_balance >= 0.0 || ship.fed { theme.success() } else { theme.danger() };
                let reactor = if ship.fed { format!("  reactor {:.0}W", ship.drawn_w) } else { String::new() };
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 60.0),
                    Align2::RIGHT_TOP,
                    &format!(
                        "Power: gen {:.0}W  use {:.0}W  net {:+.0}W{reactor}",
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

            // ── Surface flight/walk readout (v0.867): altitude above the DRAWN
            // ground + current speed gear, shown only while a planet surface is
            // engaged. Direct answer to the documented "no altitude / ground-
            // distance reference" gap (PRIORITIES surface-lock follow-ups) - the
            // operator could not judge height or scale while landing.
            if let Some(alt) = state.surface_altitude_m {
                let alt_text = if alt >= 1000.0 {
                    format!("Alt: {:.1} km", alt / 1000.0)
                } else {
                    format!("Alt: {:.0} m", alt)
                };
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, 90.0),
                    Align2::RIGHT_TOP,
                    &format!("{alt_text}  Gear: x{:.0}", state.surface_speed_mult),
                    11.0,
                    theme.accent(),
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
                    text_shadowed(painter, Pos2::new(x, compass_y), Align2::CENTER_TOP, label, 14.0, color);
                }
            }

            // ── Movement-mode indicator (v0.791.x, under the compass;
            // extended v0.1109) ── Gravity behaves DIFFERENTLY between the
            // surface modes, so the mode is never a guess: on a planet the
            // line always reads WALK or FLY, with what F9 does spelled out.
            // Off-planet it keeps its original job of explaining why movement
            // is odd - shown whenever fly mode is on or the speed gear is
            // above 1x. The text is decided by `movement_line` (pure, tested;
            // BUG-149: it read only the F9 hover bit, so the fly mode Land and
            // Travel leave behind read WALK).
            let on_surface = state.surface_altitude_m.is_some();
            if let Some((label, warn)) = movement_line(
                state.dev_fly_mode,
                state.dev_hover,
                state.dev_fly_speed_mult,
                on_surface,
                state.dev_travel_away,
            ) {
                // Plain walking is the NORMAL state, so it reads as secondary
                // text; anything that changes how the player moves keeps the
                // warning colour it has always had.
                let mode_color = if warn { theme.warning() } else { theme.text_secondary() };
                text_shadowed(
                    painter,
                    Pos2::new(center.x, compass_y + 20.0),
                    Align2::CENTER_TOP,
                    &label,
                    12.0,
                    mode_color,
                );
            }

            // ── Machine labels (world-space, distance LOD + sightline occlusion) ──
            // dot within dot_dist -> +name within name_dist -> +card within card_dist.
            // A label hides when a wall stands between the camera and the machine
            // (v0.975 sightline test against gui_state.sight_blockers: solid piers and
            // closed doors block; doorways and window glass pass). Hold Tab to reveal
            // markers through walls at x3 distance. v0.429 room filter retired: its
            // containment box was the whole house, so every card bled through the
            // interior partitions (homestead increment 1 field note).
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

            // OpenStreetMap credit (bottom-right), shown ONLY while real OSM
            // geometry is actually on screen.
            //
            // This is a licence obligation, not a courtesy. ODbL treats the
            // drawn view of OSM data as a Produced Work, and the OSM
            // Foundation asks that the credit appear WITH the map. Until
            // v0.1226 the in-world view carried the notice only in a log line
            // and the F12 debug console, so a player walking real Silverdale
            // streets saw no credit at all. The Maps page footer had it right
            // all along; this brings the 3D view level with it, using the same
            // string (credits::OSM_NOTICE) so the two cannot drift.
            if !state.osm_regions_drawn.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(screen.right() - 16.0, screen.bottom() - 96.0),
                    Align2::RIGHT_BOTTOM,
                    crate::credits::OSM_NOTICE,
                    11.0,
                    theme.text_muted(),
                );
            }

            for (i, label) in state.machine_labels.iter().enumerate() {
                // Project at the label's WORLD position: home content rides
                // the orbital station, so the scene pass shifts it by
                // station_off - a label projected at the raw local position
                // floats where the home USED to be (v0.911).
                let wpos = label.pos + state.station_off;
                let cam_dist = (wpos - cam_pos).length();
                if cam_dist > dot_dist {
                    continue; // beyond the coarsest level of detail
                }
                // Sightline occlusion (v0.975): a wall or closed door between the
                // camera and the machine hides the label. Tab still reveals all.
                if !state.reveal_held
                    && crate::ship::wall_collision::sight_blocked(
                        (cam_pos.x, cam_pos.z),
                        (wpos.x, wpos.z),
                        &state.sight_blockers,
                    )
                {
                    continue;
                }
                let Some(sp) = world_to_screen(wpos, view_proj, screen) else { continue };
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
            // the HUD stays quiet at range. Sightline occlusion (v0.975) applies here too:
            // unlike the retired v0.429 ROOM filter (which would have blinked the plate at
            // every doorway), the segment test keeps a plate visible straight through an
            // open door or a window, and only hides it behind solid wall - matching the
            // amber figure itself, which walls also hide. No dot LOD on purpose.
            // ── Tracked target markers (v0.885, operator design) ── an
            // encapsulating ring + label for map-selected objects; v1 = the
            // orbital home station. Always visible when tracked (that is the
            // point of tracking), label + distance next to the ring, brighter
            // when the camera looks near it - the machine-marker pattern
            // scaled up for world-sized targets.
            //
            // Placed by DIRECTION, never through the camera's depth range
            // (BUG-148, 2026-10-04). The gameplay camera's far plane is the
            // Render distance setting (500 m by default, 2,000 m at most) and
            // the station is about 36,000 km up, so `world_to_screen` dropped
            // the marker on every frame from anywhere off the station's own
            // deck, the ground included. `marker_placement` ignores depth;
            // when the target is off screen or behind, the ring is pinned to
            // the screen's edge on the side to turn toward, with an arrow.
            for (name, pos, dist_m) in &state.target_markers {
                let Some(mark) = marker_placement(*pos, view_proj, screen, TARGET_MARKER_MARGIN) else { continue };
                let sp = mark.pos;
                let to_target = (*pos - cam_pos).normalize_or_zero();
                let fwd = (view_proj.inverse() * glam::Vec4::new(0.0, 0.0, 1.0, 0.0))
                    .truncate()
                    .normalize_or_zero();
                // Focus factor: 1 when looking straight at it. fwd from the
                // inverse view-proj is approximate but monotonic - good
                // enough to brighten the ring on approach.
                let focus = to_target.dot(-fwd).clamp(0.0, 1.0).powi(8);
                let ring_col = if focus > 0.5 { theme.accent() } else { Color32::from_white_alpha(140) };
                painter.circle_stroke(sp, 14.0, egui::Stroke::new(1.6, ring_col));
                painter.circle_stroke(sp, 2.0, egui::Stroke::new(1.2, ring_col));
                if let Some(dir) = mark.off_screen {
                    // Out of view: a small arrow just outside the ring, pointing
                    // the way to turn (the ring sits TARGET_MARKER_MARGIN in from
                    // the edge, so the arrow stays on screen).
                    let side = Vec2::new(-dir.y, dir.x);
                    let base = sp + dir * 17.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![sp + dir * 25.0, base + side * 6.0, base - side * 6.0],
                        ring_col,
                        egui::Stroke::NONE,
                    ));
                }
                let dist_txt = format!("{name} · {}", marker_distance(*dist_m));
                // The label goes on the side of the ring that faces the middle
                // of the screen, so a marker pinned to the right edge keeps its
                // words on screen rather than running off it.
                let (label_at, align) = if sp.x > screen.center().x + screen.width() * 0.25 {
                    (sp - Vec2::new(18.0, 0.0), Align2::RIGHT_CENTER)
                } else {
                    (sp + Vec2::new(18.0, 0.0), Align2::LEFT_CENTER)
                };
                text_shadowed(
                    painter,
                    label_at,
                    align,
                    &dist_txt,
                    12.0,
                    if focus > 0.5 { theme.accent() } else { Color32::WHITE },
                );
            }
            for label in &state.crew_labels {
                let cam_dist = (label.pos - cam_pos).length();
                let Some((name, activity)) = crew_label_lines(&label.name, &label.activity, cam_dist) else {
                    continue;
                };
                if !state.reveal_held
                    && crate::ship::wall_collision::sight_blocked(
                        (cam_pos.x, cam_pos.z),
                        (label.pos.x, label.pos.z),
                        &state.sight_blockers,
                    )
                {
                    continue;
                }
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
            // A built piece in hand with its ghost standing: E BUILDS it
            // (engine/build_place.rs runs ahead of the whole E chain), so no
            // other "[E] ..." prompt may show (review of the shelter commit:
            // the machine, door and talk prompts kept promising what E would
            // not do). The placing hint below says what E does instead.
            let e_builds = state.build_placing.as_ref().is_some_and(|p| p.ghost.is_some());
            // Crew NPC talk prompt (v0.797): looking at a crew member within
            // talk range shows "[E] Talk to X". Owns the +22 crosshair slot
            // while set -- the machine/door prompts below yield to it, which
            // matches the E chain in lib.rs (a faced person outranks the
            // machine behind them).
            if !state.npc_prompt.is_empty() && !e_builds {
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
            if state.npc_prompt.is_empty() && !e_builds {
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
            // A built piece in hand (2026-09-27, engine/build_place.rs): what
            // it is, how it is turned, and its keys. Placing owns E, so the
            // bed / chest prompt below is empty meanwhile.
            if let Some(hint) = state.build_placing.as_ref().map(|p| &p.hint).filter(|h| !h.is_empty()) {
                text_shadowed(painter, Pos2::new(center.x, center.y + 22.0), Align2::CENTER_TOP, hint, 13.0, theme.accent());
            }
            // Built bed / chest prompt (2026-09-27, engine/built_uses.rs): set
            // only when nothing the E chain tries first is targeted.
            if !state.structure_prompt.is_empty() && state.npc_prompt.is_empty() {
                text_shadowed(
                    painter,
                    Pos2::new(center.x, center.y + 22.0),
                    Align2::CENTER_TOP,
                    &state.structure_prompt,
                    13.0,
                    theme.accent(),
                );
            }
            // Door control panel prompt at the crosshair (v0.567): looking at a panel within reach
            // shows [E] open/close (or "locked"). Precomputed in the walk-up block in lib.rs.
            if !state.control_panel_prompt.is_empty() && state.npc_prompt.is_empty() && !e_builds {
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
            if !state.vehicle_prompt.is_empty() && !e_builds {
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
            if !state.livestock_prompt.is_empty() && !e_builds {
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

            // ── The ONE hotbar (v0.880) ── the first nine castable
            // abilities, bottom-center; 1-9 casts the matching slot (the
            // same order the abilities bridge sorts: castable first, by
            // name). The old second row (a display-only inventory-letter
            // strip below this) read as a duplicate set of number keys
            // (operator 2026-07-18: "we have two hotbars... only one set of
            // the 10 number keys") and is gone - carried items live on the
            // Inventory page; the HUD shows only the bar the keys drive.
            let slot_size = 44.0;
            let start_y = screen.bottom() - slot_size - 16.0;
            {
                let castable: Vec<_> = state
                    .abilities
                    .iter()
                    .filter(|a| a.castable_now)
                    .take(9)
                    .collect();
                if !castable.is_empty() {
                    // Full hotbar size at the bottom slot row (it IS the
                    // hotbar now, not a mini strip above one).
                    let a_size = slot_size;
                    let a_gap = 4.0;
                    let a_total = castable.len() as f32 * a_size
                        + (castable.len() - 1) as f32 * a_gap;
                    let ax0 = center.x - a_total / 2.0;
                    let ay = start_y;
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
                    // ONE line per message: the feed is a fixed 15 px grid, so
                    // a message with newlines (a list, a code block) painted
                    // its extra lines over the rows below and out past the
                    // bottom of the box. The line is the message READ the way
                    // the Chat page reads it, not its raw text: markers are
                    // styled and never shown (`__bold__`, a ```fence```), and
                    // the Chat page's own span painter draws it, so the two
                    // surfaces agree. Only blocks change shape (see
                    // `chat::one_line_formatted`). The galley elides at the
                    // box's edge with an ellipsis instead of a char count.
                    let (text, spans) = crate::gui::pages::chat::one_line_formatted(&m.content);
                    let line_job = |ink: Option<Color32>| {
                        let mut job = egui::text::LayoutJob::default();
                        job.wrap = egui::text::TextWrapping {
                            max_width: width - 8.0,
                            max_rows: 1,
                            // egui's advice for a one-row elision: its
                            // word-break path can still cut mid-word here.
                            break_anywhere: true,
                            overflow_character: Some('\u{2026}'),
                        };
                        job.append(
                            &format!("{name}: "),
                            0.0,
                            egui::TextFormat {
                                font_id: FontId::proportional(11.0),
                                color: theme.text_secondary(),
                                ..Default::default()
                            },
                        );
                        crate::gui::widgets::row::append_formatted(
                            &mut job,
                            theme,
                            &text,
                            &[],
                            &spans,
                            11.0,
                            theme.text_secondary(),
                        );
                        // The outline pass: every glyph, underline and strike
                        // in the outline ink, no code backgrounds.
                        if let Some(ink) = ink {
                            for s in &mut job.sections {
                                s.format.color = ink;
                                s.format.background = Color32::TRANSPARENT;
                                s.format.underline.color = ink;
                                s.format.strikethrough.color = ink;
                            }
                        }
                        job
                    };
                    job_shadowed(painter, Pos2::new(x0, y), line_job(None), line_job(Some(OUTLINE_INK)));
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

/// Whether the world view's crosshair dot is drawn (lib.rs, the in-game
/// overlay pass). Only on the bare world view, and only while the HUD is
/// shown: the movie mode's `hide_hud` (src/engine/movie.rs) turns the HUD
/// off for a recording, and a dot left in the middle of every frame of a
/// clip is HUD. (2026-10-04: it sat over the sky of the landing page's
/// night-to-sunrise clip.)
pub fn crosshair_visible(state: &GuiState) -> bool {
    state.active_page == crate::gui::GuiPage::None && state.show_hud
}

fn normalize_angle(a: f32) -> f32 {
    let mut a = a % (2.0 * std::f32::consts::PI);
    if a > std::f32::consts::PI { a -= 2.0 * std::f32::consts::PI; }
    if a < -std::f32::consts::PI { a += 2.0 * std::f32::consts::PI; }
    a
}

/// Below this the wind reads "calm": it would print as 0 m/s, and the
/// direction of a wind that rounds to nothing is noise.
const CALM_M_S: f32 = 0.5;

/// The eight compass points, clockwise from north.
const COMPASS_POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

/// The compass point a wind blows FROM (the weather convention: a north wind
/// comes out of the north), given the wind toward the east and the north.
pub(crate) fn wind_from(east: f32, north: f32) -> &'static str {
    // Bearing the air is heading toward, clockwise from north, then turned
    // round to where it comes from.
    let from = (east.atan2(north).to_degrees() + 180.0).rem_euclid(360.0);
    COMPASS_POINTS[((from / 45.0).round() as usize) % COMPASS_POINTS.len()]
}

/// THE HUD'S WEATHER LINE (2026-09-27): the air the player stands in, from the
/// at-player values the weather exports. The condition as it falls HERE (a
/// Rain system in freezing air reads Snow: `Weather::condition_at_player`), or
/// the running event's name; what is falling when that name does not already
/// say it ("rain", "snow", or "rain and snow" inside the band); the
/// temperature at the player; and the wind at the player with the point it
/// blows from. Pure, so it is tested without a window. No Settings choice
/// governs it: Settings > Gameplay's HUD survival bars (`HudVitals`) choose
/// the vitals rows, and the weather line was always shown.
/// The line under the clock (2026-09-28): the air the player breathes. In a
/// sealed space (the home aboard) that is the home's air, still and at its
/// own temperature, not the planet's weather far below, which the line used
/// to show aboard ("Clear 22C wind 4 m/s from W" in a room at 20 C); outside
/// it is [`weather_line`].
///
/// Outside, when the sun or a clear night sky makes the air feel at least
/// `FEELS_NOTE_C` warmer or colder than it is (2026-09-28, the body heat
/// model's operative temperature), the line ends with what it feels like:
/// "feels 35C" standing in the noon sun in 20 C air, so the player can see
/// why the body warms in the open and why shade helps.
pub(crate) fn hud_weather_text(v: &crate::gui::GuiVitals, w: &crate::gui::GuiWeather) -> String {
    if v.sealed {
        format!("Indoors {:.0}C, still air", v.air_c)
    } else if (v.feels_c - v.air_c).abs() >= FEELS_NOTE_C {
        format!("{}  feels {:.0}C", weather_line(w), v.feels_c)
    } else {
        weather_line(w)
    }
}

/// How far what the air feels like must be from its temperature before the
/// weather line says so, C: smaller differences are within what a person can
/// tell apart, and would make the line flicker.
const FEELS_NOTE_C: f32 = 2.0;

pub(crate) fn weather_line(w: &crate::gui::GuiWeather) -> String {
    let icon = match w.condition.as_str() {
        "Clear" => "☀",
        "Cloudy" => "☁",
        "Rain" => "🌧",
        "Storm" => "⛈",
        "Snow" => "❄",
        "Fog" => "🌫",
        "Sandstorm" => "🌪",
        _ => "?",
    };
    let phase = w.falling.phase_word();
    // An active extreme event names the sky ("Thunderstorm" beats "Rain",
    // v0.1035); the icon still keys off the condition.
    let base = if w.event.is_empty() { w.condition.as_str() } else { w.event.as_str() };
    let label = if phase.is_empty() {
        base.to_string()
    } else if w.event.is_empty() && matches!(base, "Rain" | "Snow") {
        // The condition IS the phase; say the mix when it is one.
        let mut c = phase.chars();
        c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
    } else {
        format!("{base}, {phase}")
    };
    let (east, north) = w.wind_at_player;
    let speed = east.hypot(north);
    let wind = if speed < CALM_M_S {
        "calm".to_string()
    } else {
        format!("wind {speed:.0} m/s from {}", wind_from(east, north))
    };
    format!("{icon} {label} {:.0}C  {wind}", w.temperature)
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

/// The most characters of the quest line the HUD shows under the quest's name;
/// a longer line is cut with "..." (2026-10-04: every shipped quest step fits it
/// with its counter, `systems::quests::opening_tests`).
pub(crate) const QUEST_LINE_CHARS: usize = 64;

/// The quest line under the quest's name: the step as the player reads it and
/// where it stands, "Eat something: press I, ... (2/8)", or the text alone for a
/// quest with no steps.
pub(crate) fn quest_line(step_desc: &str, step_index: usize, step_total: usize) -> String {
    if step_total > 0 {
        format!("{} ({}/{})", step_desc, (step_index + 1).min(step_total), step_total)
    } else {
        step_desc.to_string()
    }
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

/// Break `s` into lines of at most `max` characters at spaces (a word longer than `max` gets
/// a line of its own), for a sentence the HUD paints line by line.
fn wrap_words(s: &str, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= max => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
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

/// How far in from the screen's edge a tracked marker that is out of view is
/// pinned, in points: room for its 14-point ring and the arrow beyond it.
pub(crate) const TARGET_MARKER_MARGIN: f32 = 48.0;

/// Where a tracked marker goes on the screen (BUG-148, `marker_placement`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MarkerPlacement {
    /// The ring's centre, in screen points.
    pub pos: Pos2,
    /// When the target is off screen or behind the camera and the ring is
    /// pinned to the edge: the way to turn, as a unit screen direction (x to
    /// the right, y down). None when the target is in view.
    pub off_screen: Option<Vec2>,
}

/// Place a tracked marker by DIRECTION, at any distance (BUG-148, 2026-10-04).
///
/// `world_to_screen` drops anything outside the camera's depth range. That
/// is right for a machine label a few metres away and wrong for a waypoint:
/// the gameplay camera's far plane is the Render distance (500 m by default,
/// 2,000 m at most) and the Home Station is about 36,000 km up, so its
/// marker was dropped on every frame, from the ground and from anywhere else
/// farther than that.
///
/// A perspective projection builds clip x, y and w from the view alone;
/// only clip z involves the near and far planes. So this reads x, y and w and
/// never z. In front of the camera (w > 0) and inside the screen less
/// `margin`, the marker goes where the target projects. Otherwise it is
/// pinned to that inset edge along the way to turn. Clip x and y keep their
/// signs behind the eye (it is only the divide by a negative w that mirrors
/// them), so they point toward the target whether it is off to one side or
/// behind; a target dead behind goes to the bottom edge. None only when the
/// projection is not finite.
pub(crate) fn marker_placement(world: Vec3, view_proj: Mat4, screen: Rect, margin: f32) -> Option<MarkerPlacement> {
    let clip = view_proj * world.extend(1.0);
    if !clip.is_finite() {
        return None;
    }
    let centre = screen.center();
    let half = screen.size() * 0.5;
    let inner = screen.shrink(margin);
    if clip.w > 0.0 {
        let p = centre + Vec2::new(clip.x / clip.w * half.x, -clip.y / clip.w * half.y);
        if inner.contains(p) {
            return Some(MarkerPlacement { pos: p, off_screen: None });
        }
    }
    // Off screen or behind. The lateral part of the clip position, scaled to
    // screen points (y down), is the way to turn. Next to nothing of it, with
    // the target behind, means dead behind: point down, the usual "turn
    // around" cue.
    let lateral = Vec2::new(clip.x * half.x, -clip.y * half.y);
    let dead_behind = Vec2::new(clip.x, clip.y).length() <= 1e-5 * clip.w.abs();
    let dir = if dead_behind || lateral.length() == 0.0 { Vec2::new(0.0, 1.0) } else { lateral.normalized() };
    // Walk from the centre along `dir` to the inset rectangle's edge (a
    // window narrower than two margins pins it to the centre line).
    let reach = (inner.size() * 0.5).max(Vec2::ZERO);
    let t = (reach.x / dir.x.abs().max(1e-6)).min(reach.y / dir.y.abs().max(1e-6));
    Some(MarkerPlacement { pos: centre + dir * t, off_screen: Some(dir) })
}

/// The distance a tracked marker shows: metres below a kilometre, tenths of a
/// kilometre below ten, then whole kilometres in groups of three ("36,000
/// km", about how far the Home Station is from the ground).
pub(crate) fn marker_distance(dist_m: f64) -> String {
    if dist_m < 1_000.0 {
        format!("{:.0} m", dist_m.max(0.0))
    } else if dist_m < 10_000.0 {
        format!("{:.1} km", dist_m / 1_000.0)
    } else {
        format!("{} km", crate::gui::pages::settings::thousands((dist_m / 1_000.0) as f32))
    }
}

/// The movement line under the compass: its text, and whether it is drawn in
/// the warning colour; None for no line (BUG-149, 2026-10-04).
///
/// Two dev bits decide how the player moves, and the line must read both.
/// `fly_mode` (`GuiState::dev_fly_mode`) is what the camera controller
/// follows: lib.rs copies it into `CameraController::fly_mode` every frame,
/// and with it on, walls and built pieces stop no one, there are no
/// footsteps, the weather does not reach the body and the speed gear is not
/// capped. `hover` (`GuiState::dev_hover`, F9) is the no-gravity law on a
/// planet (`surface_move::MoveMode`). F9 sets both. The Dev page's Land and
/// Travel, and every dev teleport, set only `fly_mode`, which leaves the
/// player under gravity but flying, and this line used to read the hover bit
/// alone and say WALK. So: FLY whenever fly mode is on, or the hover is on
/// while on a planet (off a planet the hover law does not run), and the note
/// says which, and what F9 does from there (lib.rs's F9 flips both bits to
/// the opposite of the hover bit: from fly mode without the hover, F9 turns
/// the hover on).
pub(crate) fn movement_line(
    fly_mode: bool,
    hover: bool,
    speed_mult: f32,
    on_surface: bool,
    away: bool,
) -> Option<(String, bool)> {
    use crate::surface_move::MoveMode;
    let flying = fly_mode || (on_surface && hover);
    if !(flying || on_surface || speed_mult > 1.0) {
        return None;
    }
    // One source for the two words: the surface movement model's own.
    let word = if flying { MoveMode::DevFlight.hud_word() } else { MoveMode::Walk.hud_word() };
    let mut label = format!("{word} {}", crate::dev_travel::format_multiplier(speed_mult));
    if !flying {
        // On a planet this is the real walking model (gravity, the ground
        // clamp, walls, footsteps), so say how to leave it. Off a planet the
        // line only shows for a raised speed gear.
        label.push_str(if on_surface { " [F9 to fly]" } else { " (fly mode off)" });
    } else {
        if on_surface {
            label.push_str(if hover {
                " - hover, no gravity [F9 to walk]"
            } else {
                // Land, Travel, a dev teleport: fly mode without the hover.
                // Gravity and the ground clamp still hold (MoveMode::Walk),
                // so this is not hovering, and F9 turns the hover on.
                " - gravity on [F9 to hover]"
            });
        }
        // The ship itself flies (FTL) only on the controller's fly bit
        // (lib.rs's FTL block), above the local fly speed cap.
        if fly_mode && speed_mult > crate::renderer::camera::LOCAL_FLY_MULT_MAX {
            label.push_str(" FTL - ship flying");
        }
    }
    if away {
        label.push_str(" - away from home");
    }
    Some((label, flying || speed_mult > 1.0))
}

/// Draw text with a black OUTLINE (stroke) so it stays legible over any 3D background
/// without needing a panel behind it. Renders the text in black at 8 surrounding offsets,
/// then the colored text on top. (v0.444: was a 1px drop-shadow.)
/// One survival row on the HUD: a bar with its fill fraction, or a line of
/// text (body temperature reads as degrees, not as a bar).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VitalRow {
    pub label: &'static str,
    /// 0..1 fill; None for a text-only row.
    pub frac: Option<f32>,
    /// Text shown after the label (the temperature, or empty).
    pub text: String,
    /// 0 = fine, 1 = attention, 2 = danger. Drives the colour.
    pub severity: u8,
}

/// Normal core body temperature, degrees C. Outside it the row shows in
/// every mode but Off; outside the outer pair it reads as danger: the same
/// thresholds the body heat model sets its conditions at (hypothermia below
/// 35 C, heat exhaustion above 39 C; `systems::body_heat`).
const BODY_TEMP_OK: (f32, f32) = (36.0, 37.8);
const BODY_TEMP_DANGER: (f32, f32) =
    (crate::systems::body_heat::HYPOTHERMIA_C, crate::systems::body_heat::HEAT_EXHAUSTION_C);

/// Which survival rows the HUD draws for these vitals in this mode. Pure, so
/// the choice is tested without a painter. Fill needs (food, water, energy)
/// are low when they fall; waste is the other way round, a need when it
/// fills. Air and body temperature show whenever they are out of range in
/// both non-Off modes, because those kill fastest; Air also shows, even
/// full, while the air around the player cannot be breathed (vacuum, an
/// unbreathable world), because then it only goes down. Standing in
/// breathable open air (Earth's ground) is not a warning (2026-09-28: it
/// used to show a yellow Air bar there, keyed on being outside).
pub(crate) fn vital_rows(v: &crate::gui::GuiVitals, mode: crate::config::HudVitals) -> Vec<VitalRow> {
    use crate::config::HudVitals;
    let mut rows = Vec::new();
    // Not synced yet (menus before the world): nothing to say.
    if mode == HudVitals::Off || v.satiation_max <= 0.0 {
        return rows;
    }
    let always = mode == HudVitals::Always;
    let sev = |f: f32| if f > 0.5 { 0 } else if f > 0.25 { 1 } else { 2 };
    for (label, value, max) in [
        ("Food", v.satiation, v.satiation_max),
        ("Water", v.hydration, v.hydration_max),
        ("Energy", v.energy, v.energy_max),
    ] {
        let f = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
        if always || f < 0.5 {
            rows.push(VitalRow { label, frac: Some(f), text: String::new(), severity: sev(f) });
        }
    }
    let air = if v.oxygen_max > 0.0 { (v.oxygen / v.oxygen_max).clamp(0.0, 1.0) } else { 1.0 };
    if always || !v.breathing || air < 0.999 {
        rows.push(VitalRow { label: "Air", frac: Some(air), text: String::new(), severity: if v.breathing { sev(air) } else { sev(air).max(1) } });
    }
    let t = v.body_temp_c;
    let t_out = t < BODY_TEMP_OK.0 || t > BODY_TEMP_OK.1;
    if always || t_out {
        let severity = if t < BODY_TEMP_DANGER.0 || t > BODY_TEMP_DANGER.1 { 2 } else if t_out { 1 } else { 0 };
        rows.push(VitalRow { label: "Body", frac: None, text: format!("{t:.1} °C"), severity });
    }
    // Shelter (2026-09-27): outside, under a built roof, say whether the
    // walls keep the wind off too, or how many are still missing. In the
    // open (no roof) and indoors there is nothing to say.
    if !v.sealed && !v.shelter_note.is_empty() {
        rows.push(VitalRow { label: "Shelter", frac: None, text: v.shelter_note.clone(), severity: if v.sheltered { 0 } else { 1 } });
    }
    let waste = if v.waste_max > 0.0 { (v.waste / v.waste_max).clamp(0.0, 1.0) } else { 0.0 };
    if always || waste > 0.5 {
        // Shown as how full it is; the colour runs the other way.
        rows.push(VitalRow { label: "Waste", frac: Some(waste), text: String::new(), severity: sev(1.0 - waste) });
    }
    rows
}

/// Draw one survival row at `y`; returns the next row's y.
fn draw_vital_row(painter: &egui::Painter, theme: &Theme, y: f32, row: &VitalRow) -> f32 {
    let color = match row.severity {
        0 => theme.success(),
        1 => theme.warning(),
        _ => theme.danger(),
    };
    text_shadowed(painter, Pos2::new(16.0, y), Align2::LEFT_TOP, row.label, 11.0, theme.text_secondary());
    match row.frac {
        Some(f) => {
            let bar = Rect::from_min_size(Pos2::new(66.0, y + 3.0), Vec2::new(120.0, 8.0));
            painter.rect_filled(bar, Rounding::same(3), Color32::from_black_alpha(140));
            painter.rect_filled(Rect::from_min_size(bar.min, Vec2::new(120.0 * f, 8.0)), Rounding::same(3), color);
        }
        None => {
            text_shadowed(painter, Pos2::new(66.0, y), Align2::LEFT_TOP, &row.text, 11.0, color);
        }
    }
    y + 14.0
}

fn text_shadowed(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
) {
    let font = FontId::proportional(size);
    for off in OUTLINE_OFFSETS {
        painter.text(pos + off, anchor, text, font.clone(), OUTLINE_INK);
    }
    painter.text(pos, anchor, text, font, color);
}

/// The dark outline HUD text wears so it reads over any part of the world.
const OUTLINE_INK: Color32 = Color32::from_black_alpha(200);
/// Where the outline is stamped around each glyph, 1.2 px out in all eight
/// directions.
const OUTLINE_OFFSETS: [Vec2; 8] = {
    const O: f32 = 1.2;
    [
        Vec2::new(-O, -O), Vec2::new(0.0, -O), Vec2::new(O, -O),
        Vec2::new(-O, 0.0), Vec2::new(O, 0.0),
        Vec2::new(-O, O), Vec2::new(0.0, O), Vec2::new(O, O),
    ]
};

/// `text_shadowed` for a styled line: `outline` is the same job laid out in
/// the outline ink (see the chat feed), stamped at each offset, then `job`
/// on top, top-left at `pos`.
fn job_shadowed(
    painter: &egui::Painter,
    pos: Pos2,
    job: egui::text::LayoutJob,
    outline: egui::text::LayoutJob,
) {
    let outline = painter.layout_job(outline);
    for off in OUTLINE_OFFSETS {
        painter.galley(pos + off, outline.clone(), OUTLINE_INK);
    }
    painter.galley(pos, painter.layout_job(job), OUTLINE_INK);
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
        && state.machine_card_container_note.is_none()
        && state.machine_card_fluid_actions.is_empty()
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
                // What the vessel remembers (2026-09-26): residue that must be
                // cleaned before it holds anything else, or a toxic history
                // that bars food and drinking water for good.
                if let Some(note) = state.machine_card_container_note.clone() {
                    ui.label(
                        RichText::new(note)
                            .size(theme.font_size_small)
                            .color(theme.warning()),
                    );
                }
                if state.machine_card_can_clean
                    && crate::gui::widgets::Button::secondary("Clean")
                        .tooltip("Clean out the residue (it uses water from the home tanks) so it can hold something else. A toxic history stays.")
                        .show(ui, theme)
                {
                    state.machine_card_clean_pending = true;
                }
                // Fill and pour at a water tank (2026-09-26): the vessels the
                // player carries, filled from this tank or emptied into it.
                for (key, label, tip) in state.machine_card_fluid_actions.clone() {
                    if crate::gui::widgets::Button::secondary(&label).tooltip(&tip).show(ui, theme) {
                        state.machine_card_fluid_pending = Some(key);
                    }
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
mod crosshair_tests {
    use super::crosshair_visible;
    use crate::gui::{GuiPage, GuiState};

    /// The dot follows the HUD switch, which is what movie mode turns off.
    ///
    /// Red check, run 2026-10-04 with the dot drawn on the world view
    /// whatever the HUD switch said (the lib.rs condition before this):
    /// "no crosshair while the HUD is hidden" failed. This checks the rule;
    /// that lib.rs draws the dot under it is checked by
    /// tests/engine_wiring_lint.rs (sky_and_lens_call_sites_use_their_shared_rules).
    #[test]
    fn the_crosshair_hides_with_the_hud() {
        let mut s = GuiState::default();
        s.active_page = GuiPage::None;
        s.show_hud = true;
        assert!(crosshair_visible(&s), "the world view shows its crosshair");
        s.show_hud = false;
        assert!(!crosshair_visible(&s), "no crosshair while the HUD is hidden");
        s.show_hud = true;
        s.active_page = GuiPage::Settings;
        assert!(!crosshair_visible(&s), "no crosshair over a page");
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

    /// INDOORS THE LINE IS THE HOME'S AIR (2026-09-28). Sealed aboard, the
    /// line says the room's own temperature and still air, whatever the
    /// planet's weather is doing; outside, the weather line. Red check, run:
    /// returning `weather_line(w)` in both cases fails the first assertion.
    #[test]
    fn indoors_the_line_is_the_home_air_not_the_weather() {
        let w = sky("Clear", 22.0, (-4.0, 0.0), crate::systems::precipitation::Falling::NONE, "");
        let mut v = vitals(90.0, 90.0, 90.0, 100.0, true, 36.8, 10.0);
        v.air_c = 20.0;
        assert_eq!(hud_weather_text(&v, &w), "Indoors 20C, still air");
        v.sealed = false;
        assert_eq!(hud_weather_text(&v, &w), weather_line(&w));
    }

    /// OUTSIDE, THE LINE SAYS WHAT THE AIR FEELS LIKE (2026-09-28) when the
    /// sun or a clear night sky moves it 2 C or more from the air's
    /// temperature. Red check, run: always returning `weather_line(w)`
    /// outside fails the first assertion.
    #[test]
    fn outside_the_line_says_what_the_air_feels_like() {
        let w = sky("Clear", 20.0, (0.0, 0.0), crate::systems::precipitation::Falling::NONE, "");
        let mut v = vitals(90.0, 90.0, 90.0, 100.0, false, 36.8, 10.0);
        v.feels_c = 35.0;
        assert_eq!(hud_weather_text(&v, &w), format!("{}  feels 35C", weather_line(&w)));
        v.feels_c = 21.0;
        assert_eq!(hud_weather_text(&v, &w), weather_line(&w), "within 2 C, no note");
        v.feels_c = 14.0;
        assert!(hud_weather_text(&v, &w).ends_with("feels 14C"), "a clear night");
    }

    fn vitals(food: f32, water: f32, energy: f32, air: f32, sealed: bool, temp: f32, waste: f32) -> crate::gui::GuiVitals {
        crate::gui::GuiVitals {
            satiation: food,
            hydration: water,
            energy,
            oxygen: air,
            body_temp_c: temp,
            waste,
            satiation_max: 100.0,
            hydration_max: 100.0,
            energy_max: 100.0,
            oxygen_max: 100.0,
            waste_max: 100.0,
            sealed,
            breathing: sealed,
            air_c: 20.0,
            feels_c: 20.0,
            sheltered: false,
            shelter_note: String::new(),
            effects: Vec::new(),
        }
    }

    fn labels(rows: &[VitalRow]) -> Vec<&'static str> {
        rows.iter().map(|r| r.label).collect()
    }

    /// The survival rows (2026-09-25). Always shows everything; When low
    /// shows only what needs attention (and nothing for a healthy player);
    /// Off shows nothing. Air shows whenever the player is exposed, even at
    /// full, because it runs out fastest; a cold body shows as danger.
    #[test]
    fn vital_rows_show_what_needs_attention() {
        use crate::config::HudVitals;
        let fine = vitals(90.0, 90.0, 90.0, 100.0, true, 36.8, 10.0);
        assert_eq!(labels(&vital_rows(&fine, HudVitals::Always)), vec!["Food", "Water", "Energy", "Air", "Body", "Waste"]);
        assert!(vital_rows(&fine, HudVitals::WhenLow).is_empty(), "a healthy player sees no bars");
        assert!(vital_rows(&fine, HudVitals::Off).is_empty());

        let thirsty = vitals(90.0, 20.0, 45.0, 100.0, true, 36.8, 80.0);
        let rows = vital_rows(&thirsty, HudVitals::WhenLow);
        assert_eq!(labels(&rows), vec!["Water", "Energy", "Waste"]);
        assert_eq!(rows[0].severity, 2, "20% water is danger");
        assert_eq!(rows[1].severity, 1, "45% energy is attention");
        assert_eq!(rows[2].severity, 2, "80% waste is past the 75% line where the unsanitary debuff starts");

        let exposed = vitals(90.0, 90.0, 90.0, 100.0, false, 34.5, 10.0);
        let rows = vital_rows(&exposed, HudVitals::WhenLow);
        assert_eq!(labels(&rows), vec!["Air", "Body"]);
        assert_eq!(rows[0].severity, 1, "exposed air is never shown as fine");
        assert_eq!(rows[1].severity, 2, "34.5 C is hypothermia");
        assert_eq!(rows[1].text, "34.5 °C");
        assert!(vital_rows(&exposed, HudVitals::Off).is_empty(), "Off is the simple mode: health only");

        // Outside in air that can be breathed (Earth's ground, 2026-09-28):
        // full air is not a warning. Red check, run: keying the Air row on
        // `sealed` again shows a yellow Air bar here and fails this.
        let mut on_earth = vitals(90.0, 90.0, 90.0, 100.0, false, 36.8, 10.0);
        on_earth.breathing = true;
        assert!(vital_rows(&on_earth, HudVitals::WhenLow).is_empty(), "breathable open air, full air: no bars");
        on_earth.oxygen = 40.0;
        let rows = vital_rows(&on_earth, HudVitals::WhenLow);
        assert_eq!((labels(&rows), rows[0].severity), (vec!["Air"], 1), "short of breath still shows: 40% air is attention");

        let unsynced = crate::gui::GuiVitals::default();
        assert!(vital_rows(&unsynced, HudVitals::Always).is_empty(), "no vitals yet, no rows");
    }

    /// The HUD says "Sheltered" outside under a built shelter (2026-09-27),
    /// and what is missing under a roof with too few walls; nothing in the
    /// open, nothing indoors, nothing in Off. Red check, run: without the
    /// Shelter row in `vital_rows` the first assertion fails.
    #[test]
    fn the_hud_says_sheltered_under_a_built_shelter() {
        use crate::config::HudVitals;
        let mut v = vitals(90.0, 90.0, 90.0, 100.0, false, 36.8, 10.0);
        v.sheltered = true;
        v.shelter_note = "Sheltered".into();
        let rows = vital_rows(&v, HudVitals::WhenLow);
        let row = rows.iter().find(|r| r.label == "Shelter").expect("a Shelter row outside under a roof");
        assert_eq!((row.text.as_str(), row.severity), ("Sheltered", 0));
        v.sheltered = false;
        v.shelter_note = "Out of the rain; 100% of the wind gets in".into();
        let rows = vital_rows(&v, HudVitals::WhenLow);
        assert_eq!(rows.iter().find(|r| r.label == "Shelter").map(|r| r.severity), Some(1), "the wind getting in needs attention");
        v.shelter_note.clear();
        assert!(!labels(&vital_rows(&v, HudVitals::Always)).contains(&"Shelter"), "the open: no line");
        let mut home = vitals(90.0, 90.0, 90.0, 100.0, true, 36.8, 10.0);
        home.shelter_note = "Sheltered".into();
        assert!(!labels(&vital_rows(&home, HudVitals::Always)).contains(&"Shelter"), "sealed indoors says Sealed elsewhere");
        assert!(vital_rows(&v, HudVitals::Off).is_empty());
    }

    fn sky(condition: &str, temp: f32, wind: (f32, f32), falling: crate::systems::precipitation::Falling, event: &str) -> crate::gui::GuiWeather {
        crate::gui::GuiWeather {
            condition: condition.into(),
            intensity: falling.total(),
            temperature: temp,
            // The weather's own wind: the line must NOT read it.
            wind_speed: 2.0,
            wind_at_player: wind,
            falling,
            event: event.into(),
            warning: String::new(),
        }
    }

    /// THE HUD SAYS THE AIR THE PLAYER STANDS IN (2026-09-27): the
    /// temperature and the wind AT THE PLAYER (with the point it blows from),
    /// and whether it is raining or snowing there, including the mix inside
    /// the band, a storm's phase and an event's. Seen red: the line as it was
    /// (`{icon} {condition} {temp}C {wind_speed}m/s`, the weather's own 2 m/s
    /// and no phase) prints "❄ Snow -12C 2m/s" for the first case.
    #[test]
    fn the_weather_line_reads_the_air_at_the_player() {
        use crate::systems::precipitation::Falling;
        let snowing = sky("Snow", -12.4, (-6.0, -6.0), Falling { rain: 0.0, snow: 0.7 }, "");
        assert_eq!(weather_line(&snowing), "❄ Snow -12C  wind 8 m/s from NE");
        let mixed = sky("Rain", 1.0, (0.0, 3.0), Falling { rain: 0.3, snow: 0.3 }, "");
        assert_eq!(weather_line(&mixed), "🌧 Rain and snow 1C  wind 3 m/s from S");
        let blizzard = sky("Storm", -3.0, (10.0, 0.0), Falling { rain: 0.05, snow: 0.9 }, "");
        assert_eq!(weather_line(&blizzard), "⛈ Storm, snow -3C  wind 10 m/s from W");
        let event = sky("Rain", 18.0, (0.2, 0.1), Falling { rain: 0.8, snow: 0.0 }, "Thunderstorm");
        assert_eq!(weather_line(&event), "🌧 Thunderstorm, rain 18C  calm");
        let trades = sky("Clear", 25.0, (-4.0, 1.0), Falling::NONE, "");
        assert_eq!(weather_line(&trades), "☀ Clear 25C  wind 4 m/s from E");
        // A Rain system over the world below, nothing falling on the player
        // (no air here): the condition's name, no phase.
        let dry_here = sky("Rain", 20.0, (2.0, 0.0), Falling::NONE, "");
        assert_eq!(weather_line(&dry_here), "🌧 Rain 20C  wind 2 m/s from W");
        assert_eq!((wind_from(0.0, -5.0), wind_from(5.0, 0.0), wind_from(-3.0, 3.0)), ("N", "W", "SE"));
    }

    #[test]
    fn short_activity_is_untouched() {
        assert_eq!(truncate_chars("Watering the crops", 48), "Watering the crops");
    }
}

/// BUG-136: the HUD's overload line is DRAWN, not only composed. The text
/// itself is pinned in `systems::encumbrance`; this pins that the HUD paints
/// it, which a pure-function test cannot see.
#[cfg(test)]
mod carry_line_tests {
    use crate::gui::screen_surface::find_text_in_shapes;
    use crate::gui::GuiState;
    use crate::systems::encumbrance::{evaluate, CarryInput, CarryMode, ONE_G_M_S2};

    /// Two settle frames of the HUD alone, returning the second's shapes.
    fn hud_shapes(state: &GuiState) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut shapes = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                super::draw(ctx, &theme, state, 0.0, glam::Mat4::IDENTITY, glam::Vec3::ZERO);
            });
            shapes = out.shapes;
        }
        shapes
    }

    /// Overloaded in the Realistic mode, the HUD says so and says why the
    /// walk is slow; under the limit it says nothing. Seen red with the
    /// HUD's draw block removed: "assertion `left == right` failed: the HUD
    /// draws the overload line; left: None, right: Some(\"Overloaded 60 / 50
    /// kg: walking at 80%, no jumping\")".
    #[test]
    fn the_hud_draws_the_overload_line_only_when_overloaded() {
        let mut state = GuiState::default();
        state.carry = evaluate(
            CarryInput { carried_kg: 60.0, capacity_kg: 50.0, ..CarryInput::default() },
            ONE_G_M_S2,
            CarryMode::Realistic,
        );
        let line = "Overloaded 60 / 50 kg: walking at 80%, no jumping";
        let found = find_text_in_shapes(&hud_shapes(&state), line);
        assert_eq!(found.map(|f| f.text).as_deref(), Some(line), "the HUD draws the overload line");
        state.carry = evaluate(
            CarryInput { carried_kg: 40.0, capacity_kg: 50.0, ..CarryInput::default() },
            ONE_G_M_S2,
            CarryMode::Realistic,
        );
        assert!(find_text_in_shapes(&hud_shapes(&state), "Overloaded").is_none(), "under the limit, no line");
    }
}

/// BUG-148 (the tracked marker) and BUG-149 (the movement line), read back
/// from the shapes the HUD actually paints, through the game's own camera.
#[cfg(test)]
mod marker_and_mode_tests {
    use crate::gui::screen_surface::find_text_in_shapes;
    use crate::gui::GuiState;
    use crate::renderer::camera::Camera;
    use glam::Vec3;

    /// The test screen, the size the other HUD tests use.
    const SCREEN: egui::Vec2 = egui::vec2(1280.0, 900.0);

    /// The gameplay camera as a player has it on the ground: its far plane is
    /// the Render distance setting, 500 m by default (config.rs, applied at
    /// boot by lib.rs's settings block), and its aspect is the screen's.
    fn ground_camera() -> Camera {
        let mut cam = Camera::new();
        cam.far = 500.0;
        cam.aspect = SCREEN.x / SCREEN.y;
        cam
    }

    /// Two settle frames of the HUD alone, drawn through `cam` exactly as
    /// lib.rs calls it (yaw, view-projection, position); the second frame's
    /// shapes.
    fn hud_shapes(state: &GuiState, cam: &Camera) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut shapes = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), SCREEN)),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                super::draw(ctx, &theme, state, cam.yaw, cam.view_projection_matrix(), cam.position);
            });
            shapes = out.shapes;
        }
        shapes
    }

    /// Track the Home Station at `pos` (render space, metres), as lib.rs
    /// does from the station's orbit each frame.
    fn track(state: &mut GuiState, pos: Vec3) {
        state.target_markers = vec![("Home Station".to_string(), pos, pos.length() as f64)];
    }

    /// BUG-148: the tracked Home Station, 36,000 km straight ahead of a
    /// player standing on the ground, draws its ring and its label with the
    /// distance, at the middle of the screen. The marker was projected
    /// through the camera's depth range, whose far plane is the Render
    /// distance (500 m by default, 2,000 m at most), so anything farther,
    /// which is every tracked station, was dropped.
    ///
    /// Seen red before the fix: "the tracked station draws its label: none
    /// was drawn" (the `expect` below panicked).
    #[test]
    fn a_tracked_station_36000_km_away_draws_its_marker() {
        let cam = ground_camera();
        let mut state = GuiState::default();
        track(&mut state, cam.forward() * 3.6e7);
        let found = find_text_in_shapes(&hud_shapes(&state, &cam), "Home Station")
            .expect("the tracked station draws its label: none was drawn");
        assert_eq!(found.text, "Home Station · 36,000 km", "the label says how far it is");
        // The label sits just right of the ring, which is at the screen's
        // middle because the station is dead ahead.
        let mid = SCREEN * 0.5;
        assert!(
            (found.rect.center().y - mid.y).abs() < 12.0
                && found.rect.left() > mid.x
                && found.rect.left() < mid.x + 40.0,
            "the marker is at the middle of the screen: label at {:?}",
            found.rect
        );
    }

    /// BUG-148: behind the player the marker is not lost. It is pinned to
    /// the screen's edge on the side to turn toward (here the right: the
    /// station is behind and to the right), with its label inside the
    /// screen.
    ///
    /// Seen red before the fix: "a station behind the player still draws
    /// its marker: none was drawn" (the `expect` below panicked).
    #[test]
    fn a_tracked_station_behind_the_player_is_pinned_to_the_screen_edge() {
        let cam = ground_camera();
        let mut state = GuiState::default();
        let behind_right = (-cam.forward() * 3.0 + cam.right()).normalize();
        track(&mut state, behind_right * 3.6e7);
        let found = find_text_in_shapes(&hud_shapes(&state, &cam), "Home Station")
            .expect("a station behind the player still draws its marker: none was drawn");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
        assert!(screen.contains_rect(found.rect), "the label is on screen: {:?}", found.rect);
        assert!(
            found.rect.center().x > SCREEN.x * 0.75,
            "on the right, the side to turn toward: label at {:?}",
            found.rect
        );
    }

    /// The movement line under the compass, as drawn: the text of whichever
    /// line starts with WALK or FLY, if any.
    fn movement_text(state: &GuiState, cam: &Camera) -> Option<String> {
        let shapes = hud_shapes(state, cam);
        let walk = find_text_in_shapes(&shapes, "WALK").filter(|f| f.text.starts_with("WALK"));
        let fly = find_text_in_shapes(&shapes, "FLY").filter(|f| f.text.starts_with("FLY"));
        match (walk, fly) {
            (Some(w), Some(f)) => panic!("both WALK and FLY drawn: {:?} and {:?}", w.text, f.text),
            (w, f) => w.or(f).map(|f| f.text),
        }
    }

    /// BUG-149: the HUD says what is true. On the ground with fly mode on
    /// and hover off, which is what the Dev page's Land and Travel leave
    /// behind, the line reads FLY: walls do not stop the player, there are
    /// no footsteps and the weather does not reach the body, although
    /// gravity still holds them down, and F9 turns hover on. Walking reads
    /// WALK, and F9's hover reads FLY with its own note.
    ///
    /// Seen red before the fix: "fly mode on (Land, Travel): left:
    /// Some(\"WALK x1 [F9 to fly]\"), right: Some(\"FLY x1 - gravity on [F9
    /// to hover]\")".
    #[test]
    fn the_movement_line_reads_fly_whenever_fly_mode_is_on() {
        let cam = ground_camera();
        let mut state = GuiState::default();
        state.surface_altitude_m = Some(0.0);
        // What Land and Travel set (lib.rs): fly mode on, hover untouched.
        state.dev_fly_mode = true;
        state.dev_hover = false;
        assert_eq!(
            movement_text(&state, &cam).as_deref(),
            Some("FLY x1 - gravity on [F9 to hover]"),
            "fly mode on (Land, Travel)"
        );
        // Both off: walking for real.
        state.dev_fly_mode = false;
        assert_eq!(movement_text(&state, &cam).as_deref(), Some("WALK x1 [F9 to fly]"), "walking");
        // F9: hover, both bits on.
        state.dev_fly_mode = true;
        state.dev_hover = true;
        assert_eq!(
            movement_text(&state, &cam).as_deref(),
            Some("FLY x1 - hover, no gravity [F9 to walk]"),
            "F9 hover"
        );
    }
}

/// The pure rules under the drawn tests above: where a tracked marker goes
/// (BUG-148), what its distance reads, and the movement line (BUG-149).
#[cfg(test)]
mod placement_and_line_tests {
    use super::{marker_distance, marker_placement, movement_line, world_to_screen, MarkerPlacement, TARGET_MARKER_MARGIN};
    use crate::renderer::camera::Camera;
    use egui::{pos2, vec2, Rect};
    use glam::Vec3;

    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 900.0))
    }

    /// The gameplay camera with the default Render distance (500 m) as its
    /// far plane.
    fn ground_camera() -> Camera {
        let mut cam = Camera::new();
        cam.far = 500.0;
        cam.aspect = 1280.0 / 900.0;
        cam
    }

    /// A target 36,000 km away along `dir`, placed for `cam`.
    fn place(cam: &Camera, dir: Vec3) -> MarkerPlacement {
        marker_placement(dir.normalize() * 3.6e7, cam.view_projection_matrix(), screen(), TARGET_MARKER_MARGIN)
            .expect("a finite projection is always placed")
    }

    /// Dead ahead and 72,000 times past the far plane: the middle of the
    /// screen, in view. The depth-range projection drops the same point,
    /// which is the whole of BUG-148.
    #[test]
    fn a_target_far_past_the_far_plane_projects_to_where_it_is() {
        let cam = ground_camera();
        let p = place(&cam, cam.forward());
        assert!((p.pos - screen().center()).length() < 0.5, "{p:?}");
        assert_eq!(p.off_screen, None, "in view");
        assert!(
            world_to_screen(cam.forward() * 3.6e7, cam.view_projection_matrix(), screen()).is_none(),
            "the depth-range projection drops it"
        );
        // A little up and to the right, still in view: up and to the right
        // of the middle.
        let p = place(&cam, cam.forward() + cam.right() * 0.2 + cam.up * 0.1);
        assert!(p.off_screen.is_none() && p.pos.x > 700.0 && p.pos.y < 420.0, "{p:?}");
    }

    /// Out of view, the ring is pinned TARGET_MARKER_MARGIN in from the edge
    /// on the side to turn toward, pointing that way: far right in front,
    /// behind on the right, straight overhead, and dead behind (the bottom
    /// edge, "turn around").
    #[test]
    fn a_target_out_of_view_is_pinned_to_the_edge_it_is_toward() {
        let cam = ground_camera();
        let s = screen();
        let m = TARGET_MARKER_MARGIN;
        let (f, r, u) = (cam.forward(), cam.right(), cam.up);
        let near = |a: f32, b: f32| (a - b).abs() < 0.5;

        let p = place(&cam, f * 0.17 + r); // 80 degrees right of ahead
        let d = p.off_screen.expect("out of view");
        assert!(near(p.pos.x, s.right() - m) && d.x > 0.99, "far right: {p:?}");

        let p = place(&cam, -f * 3.0 + r); // behind, on the right
        let d = p.off_screen.expect("behind");
        assert!(near(p.pos.x, s.right() - m) && d.x > 0.99, "behind on the right: {p:?}");

        let p = place(&cam, u); // straight overhead
        let d = p.off_screen.expect("overhead");
        assert!(near(p.pos.y, s.top() + m) && d.y < -0.99, "overhead: {p:?}");

        let p = place(&cam, -f); // dead behind
        assert_eq!(p.off_screen, Some(vec2(0.0, 1.0)), "dead behind points down");
        assert!(near(p.pos.y, s.bottom() - m) && near(p.pos.x, s.center().x), "dead behind: {p:?}");

        for target in [f * 0.17 + r, -f * 3.0 + r, u, -f, -f + u * 0.3 - r] {
            let p = place(&cam, target);
            assert!(s.shrink(m - 0.5).contains(p.pos), "always on screen, inside the margin: {p:?}");
        }
    }

    #[test]
    fn the_distance_reads_in_metres_then_kilometres() {
        assert_eq!(marker_distance(850.0), "850 m");
        assert_eq!(marker_distance(1_520.0), "1.5 km");
        assert_eq!(marker_distance(3.6e7), "36,000 km");
        assert_eq!(marker_distance(3.844e8), "384,400 km");
    }

    /// Every state of the two bits (BUG-149): WALK only when neither changes
    /// how the player moves; FLY with the note that is true for the state.
    /// Off a planet only the controller's fly bit counts: the hover law runs
    /// only on a planet's surface.
    #[test]
    fn the_movement_line_says_what_is_true_in_every_state() {
        let line = |fly, hover, surface| movement_line(fly, hover, 1.0, surface, false);
        // On a planet.
        assert_eq!(line(false, false, true), Some(("WALK x1 [F9 to fly]".to_string(), false)));
        assert_eq!(line(true, false, true), Some(("FLY x1 - gravity on [F9 to hover]".to_string(), true)), "Land, Travel");
        assert_eq!(line(true, true, true), Some(("FLY x1 - hover, no gravity [F9 to walk]".to_string(), true)), "F9");
        assert_eq!(
            line(false, true, true),
            Some(("FLY x1 - hover, no gravity [F9 to walk]".to_string(), true)),
            "the Dev page's Fly mode box unticked under F9's hover: still no gravity"
        );
        // Off a planet.
        assert_eq!(line(false, false, false), None, "walking aboard needs no line");
        assert_eq!(line(true, false, false), Some(("FLY x1".to_string(), true)), "Travel's free flight in space");
        assert_eq!(line(false, true, false), None, "the hover bit does nothing off a planet");
        // The speed gear, FTL and away notes.
        assert_eq!(movement_line(false, false, 10.0, false, false), Some(("WALK x10 (fly mode off)".to_string(), true)));
        assert_eq!(
            movement_line(true, false, 1.0e6, false, true),
            Some(("FLY x1M FTL - ship flying - away from home".to_string(), true))
        );
    }
}
