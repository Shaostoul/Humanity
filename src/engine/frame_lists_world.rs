//! The world's draw lists (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: deployed vehicles, the barn's visible storage, and the
//! game time and weather bridged from the data store.

use crate::engine::frame_prelude::*;

/// Adds the world's vehicles and storage to the draw lists and bridges time and weather.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = f.dt;
    let showroom = f.showroom;
    humanity_core::frame_costs::span("cpu.span.lists_world");
    // ── Deployed vehicles (economy Phase 2 Stage 1, v0.677) ──
    // Every Vehicle entity gets a primitive placeholder body composed
    // from ONE cached unit box + ONE cached wheel cylinder, scaled per
    // vehicle from the kit registry's proportions (same build-once
    // pattern as the drone dock above; real models come later). Hidden
    // in the showroom (avatar-only backdrop), visible everywhere else —
    // a parked truck is world content, not a machine, so it ignores the
    // "Machine" declutter toggle.
    if !showroom {
        if state.vehicle_meshes.is_none() {
            let unit_box = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            let wheel = state.renderer.add_mesh(Mesh::cylinder_capped(&state.renderer.device, 0.5, 1.0, 14));
            state.vehicle_meshes = Some([unit_box, wheel]);
        }
        if state.vehicle_mats.is_none() {
            // theme-exempt: placeholder vehicle paint/glass/rubber, not UI colors.
            let paint = state.renderer.add_material_typed([0.68, 0.24, 0.16, 1.0], 0.35, 0.45, 0.0);
            let glass = state.renderer.add_material_typed([0.45, 0.62, 0.72, 1.0], 0.85, 0.15, 0.0);
            let rubber = state.renderer.add_material_typed([0.10, 0.10, 0.11, 1.0], 0.05, 0.85, 0.0);
            state.vehicle_mats = Some([paint, glass, rubber]);
        }
        let [unit_box, wheel_mesh] = state.vehicle_meshes.unwrap();
        let [paint_mat, glass_mat, rubber_mat] = state.vehicle_mats.unwrap();
        if let Some(kits) = state
            .data_store
            .get::<humanity_systems::vehicles::VehicleKitRegistry>("vehicle_kit_registry")
        {
            for (_e, (veh, tf)) in state
                .game_world
                .world
                .query::<(&humanity_core::ecs::components::Vehicle, &humanity_core::ecs::components::Transform)>()
                .iter()
            {
                let Some(def) = kits.get_vehicle(&veh.item_id) else { continue };
                let (bl, bh, bw) = def.body_m;
                let (cl, ch, cw) = def.cabin_m;
                let r = def.wheel_radius_m;
                let rot = tf.rotation;
                let base = tf.position;
                // Body bed rides at axle height (box_xyz is y-bottom-origin).
                f.all_objects.push(RenderObject { fade: 0.0,
                    position: base + rot * Vec3::new(0.0, r, 0.0),
                    rotation: rot,
                    scale: Vec3::new(bl, bh, bw),
                    mesh: unit_box,
                    material: paint_mat,
                });
                // Cabin on top of the body, offset forward/back per the def.
                f.all_objects.push(RenderObject { fade: 0.0,
                    position: base + rot * Vec3::new(def.cabin_offset_x, r + bh, 0.0),
                    rotation: rot,
                    scale: Vec3::new(cl, ch, cw),
                    mesh: unit_box,
                    material: glass_mat,
                });
                // Four wheels at the body corners, cylinder axis laid to
                // the vehicle's lateral (Z) axis.
                let wheel_rot = rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                let wx = (bl * 0.5 - r * 1.2).max(r);
                let wz = bw * 0.5;
                for (dx, dz) in [(wx, wz), (wx, -wz), (-wx, wz), (-wx, -wz)] {
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: base + rot * Vec3::new(dx, r, dz),
                        rotation: wheel_rot,
                        scale: Vec3::new(r * 2.0, 0.3, r * 2.0),
                        mesh: wheel_mesh,
                        material: rubber_mat,
                    });
                }
            }
        }
    }

    // Visible storage: the barn fills with crates as the stock
    // grows, and each water tank shows its level.
    if !showroom {
        crate::engine::stock_piles::push_render_objects(state, &mut f.all_objects);
    }

    // ── Built pieces (v0.746; planet sites 2026-09-27, BUG-102) ── scaffolds
    // and finished pieces: home pieces in the home frame here, pieces on a
    // planet in the celestial list where they stand (engine/planet_build.rs).
    // `planet_ghost` (the piece in hand on a planet) is already in render
    // space, so it joins the scene list AFTER the station shift below.
    f.planet_ghost = Vec::new();
    // Packs left where the player fell, and their HUD markers (engine/death_pack.rs).
    if !showroom {
        crate::engine::death_pack::push_render_objects(state, &mut f.all_objects, &mut f.celestial_objects);
    }
    crate::engine::planet_build::push_render_objects(
        state,
        &mut f.all_objects,
        &mut f.celestial_objects,
        &mut f.planet_ghost,
        &mut f.transparent_objects,
        &mut f.celestial_transparent,
    );

    // ── Livestock render (v0.751, ladder rung 7) ── placeholder
    // block bodies until real models: body + head + 4 legs from
    // one lazy unit box, sized from the species' mass, tinted per
    // livestock.ron. Head sits at rotation * +Z to match the
    // graze amble's facing convention.
    {
        if state.livestock_mesh.is_none() {
            state.livestock_mesh = Some(
                state
                    .renderer
                    .add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0)),
            );
        }
        let unit_box = state.livestock_mesh.unwrap();
        let animals: Vec<(Vec3, Quat, f32, [f32; 3], bool)> = state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Creature,
                &humanity_core::ecs::components::Transform,
                Option<&humanity_core::ecs::components::Dead>,
            )>()
            .iter()
            .map(|(_e, (c, tf, dead))| {
                (tf.position, tf.rotation, c.body_side, c.tint, dead.is_some())
            })
            .collect();
        let mats = &mut state.livestock_mats;
        let renderer = &mut state.renderer;
        for (pos, rot, s, tint, is_dead) in animals {
            // A dead animal lies on its side until the corpse
            // decays (CombatSystem despawns it). (v0.760)
            let rot = if is_dead {
                rot * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
            } else {
                rot
            };
            // One material per distinct tint, built on first sight.
            let key = ((tint[0] * 255.0) as u64) << 16
                | ((tint[1] * 255.0) as u64) << 8
                | (tint[2] * 255.0) as u64;
            let body_mat = *mats.entry(key).or_insert_with(|| {
                renderer.add_material_typed(
                    [tint[0], tint[1], tint[2], 1.0],
                    0.0,
                    0.9,
                    0.0,
                )
            });
            let leg_h = s * 0.5;
            // Body: bottom-origin box riding on the legs.
            f.all_objects.push(RenderObject { fade: 0.0,
                position: pos + Vec3::Y * leg_h,
                rotation: rot,
                scale: Vec3::new(s * 0.9, s, s * 1.6),
                mesh: unit_box,
                material: body_mat,
            });
            // Head at the front (+Z), above the body line.
            f.all_objects.push(RenderObject { fade: 0.0,
                position: pos + rot * Vec3::new(0.0, leg_h + s * 0.75, s * 0.95),
                rotation: rot,
                scale: Vec3::splat(s * 0.55),
                mesh: unit_box,
                material: body_mat,
            });
            // Four legs at the body corners.
            for (dx, dz) in [
                (s * 0.3, s * 0.55),
                (s * 0.3, -s * 0.55),
                (-s * 0.3, s * 0.55),
                (-s * 0.3, -s * 0.55),
            ] {
                f.all_objects.push(RenderObject { fade: 0.0,
                    position: pos + rot * Vec3::new(dx, 0.0, dz),
                    rotation: rot,
                    scale: Vec3::new(s * 0.18, leg_h, s * 0.18),
                    mesh: unit_box,
                    material: body_mat,
                });
            }
        }
    }

    // Bridge game time from DataStore (if TimeSystem writes it)
    if let Some(gt) = state
        .data_store
        .get::<std::sync::Mutex<GameTime>>("game_time")
        .and_then(|m| m.lock().ok())
    {
        // LOCAL mean solar time at the frame-locked site
        // (sun-clock fix): the game clock is lon-0 solar
        // time by construction, so the wall clock a PLAYER
        // should read is hour + lon/15. The frame-lock
        // anchor is stored in the body's UNROTATED local
        // frame, so its lat/lon IS the geographic
        // coordinate.
        let local_hour = {
            let a = state.frame_lock_anchor;
            if a.length_squared() > 1.0 {
                let d = a.normalize();
                let (_, lon) =
                    humanity_terrain::planet_heightmap::dir_to_latlon_deg(
                        glam::Vec3::new(d.x as f32, d.y as f32, d.z as f32),
                    );
                Some(humanity_systems::time::local_hour(
                    gt.hour as f64, lon as f64, gt.hours_per_day,
                ) as f32)
            } else if state.aboard_station
                && state.data_store.get::<f64>(humanity_systems::time::HOME_LONGITUDE_KEY).is_some()
            {
                // Aboard the home: the deck's own time, the
                // clock its panels and crops follow (BUG-090).
                Some(humanity_systems::time::local_hour(
                    gt.hour as f64,
                    humanity_systems::time::home_longitude_deg(&state.data_store),
                    gt.hours_per_day,
                ) as f32)
            } else {
                None
            }
        };
        // On a 24-hour dial, so the sun is up 6 to 18 of any day length.
        let wall = local_hour.unwrap_or(gt.hour) * 24.0 / gt.hours_per_day.max(1) as f32;
        state.gui_state.game_time = Some(GuiGameTime {
            hour: gt.hour,
            day_count: gt.day_count,
            season: format!("{:?}", gt.season),
            // Day/night from the clock the player READS -
            // the local one where it exists.
            is_daytime: wall >= 6.0 && wall <= 18.0,
            local_hour,
            hours_per_day: gt.hours_per_day,
        });
    }

    // Bridge weather from DataStore (WeatherSystem writes it via Mutex).
    if let Some(w) = state
        .data_store
        .get::<std::sync::Mutex<Weather>>("weather")
        .and_then(|m| m.lock().ok())
    {
        state.gui_state.weather = Some(GuiWeather {
            intensity: w.intensity,
            condition: format!("{:?}", w.condition_at_player()), // rain or snow as it falls HERE
            // The HUD thermometer shows the temperature AT THE
            // PLAYER (the same value the survival exposure path
            // uses), not the body-global simulation reference:
            // the readout and the hypothermia math must agree
            // on a mountain top or at lunar noon. Farming and
            // hydrology keep reading the global w.temperature.
            temperature: w.temperature_at_player,
            wind_speed: w.wind_speed,
            wind_at_player: (w.wind_east_at_player, w.wind_north_at_player), falling: w.falling_at_player(),
            event: w.event_name.clone(),
            warning: String::new(),
        });
        // Foliage wind (v0.1080): publish the live weather
        // wind to the renderer so the trees lean and gust
        // with the SAME wind the ocean and clouds already
        // consume. Direction is a unit world vector; speed
        // rides in w. The renderer pokes it into both camera
        // buffers (colour + shadow) at offset 576.
        //
        // Routed through engine::ipc::published_foliage_wind
        // so the rig's {"wind":"0"} pin (EngineState::
        // foliage_wind_override) lands HERE, at the single
        // place the value reaches the shader, and therefore
        // pins the near-tree sway and the grass sway together.
        // That helper also holds the do-not-publish-a-literal-
        // zero rule: the shader reads speed <= 0 as "nobody is
        // publishing" and substitutes a 4 m/s breeze.
        {
            state.renderer.foliage_wind = crate::engine::ipc::published_foliage_wind(
                state.foliage_wind_override,
                w.wind_direction,
                w.wind_speed,
                state.renderer.foliage_wind,
            );
        }
        // Sea state from the wind AT THE PLAYER
        // (`Weather::sea_state_target`, v0.909's mapping, the
        // at-player wind since 2026-09-28). The showcase
        // {"sea":x} override wins for dev shots. Smoothed
        // (~30 s time constant) so weather regime flips roll
        // the ocean over instead of snapping it.
        if let Some(pin) = state.sea_state_override {
            // PINNED encoding (v0.922): the shader takes
            // max(local MODIS storm cell, this pad), so a
            // plain low pin could never calm a storm cell
            // for dev shots. Values >= 1.5 mean "pinned at
            // (value - 2), ignore MODIS".
            state.renderer.sea_state = pin.clamp(0.0, 1.0) + 2.0;
        } else {
            if state.renderer.sea_state > 1.5 {
                // Leaving pinned mode: snap back into the
                // 0..1 wind domain before smoothing.
                state.renderer.sea_state -= 2.0;
            }
            // The wind where the player is (2026-09-28).
            let target = w.sea_state_target();
            let k = (dt as f32 / 30.0).min(1.0);
            state.renderer.sea_state +=
                (target - state.renderer.sea_state) * k;
        }
        // Cloud wind-advection (v0.1032): integrate the live
        // wind into the deck's drift angle; the shader
        // rotates its MODIS lookup by the sum so cloud
        // masses MOVE at weather-dependent rates between
        // map refreshes (operator: clouds should not sit
        // pinned; "clouds move at varying rates depending
        // on the weather").
        let (a, dc) = crate::renderer::clouds::advance_cloud_advect(
            state.cloud_advect,
            state.cloud_advect_decaying,
            w.wind_speed,
            dt as f32,
        );
        state.cloud_advect = a;
        state.cloud_advect_decaying = dc;
        state.renderer.cloud_advect = a + dc;
        // Weather-event cloud override (v0.1037): ease the
        // deck's extra coverage + tint toward the active
        // event's cloud block (or back to neutral), ~10 s
        // time constant so a thunderstorm ROLLS the sky
        // over instead of snapping it.
        let (tb, tt) = if w.event_id.is_empty() {
            (0.0, [1.0f32; 3])
        } else {
            state
                .data_store
                .get::<humanity_systems::weather_events::WeatherEventRegistry>(
                    "weather_event_registry",
                )
                .and_then(|r| r.events.iter().find(|e| e.id == w.event_id))
                .map(|e| {
                    (
                        e.cloud.coverage_boost,
                        [e.cloud.tint.0, e.cloud.tint.1, e.cloud.tint.2],
                    )
                })
                .unwrap_or((0.0, [1.0; 3]))
        };
        let ek = (dt as f32 / 10.0).min(1.0);
        state.cloud_event_boost += (tb - state.cloud_event_boost) * ek;
        for i in 0..3 {
            state.cloud_event_tint[i] +=
                (tt[i] - state.cloud_event_tint[i]) * ek;
        }
        // Vortex core lifecycle + proximity (v0.1038,
        // log + HUD scope; forces/damage are the next
        // rung, and wind does not move the player yet so
        // no physics is invented here). Core is placed
        // ONCE per event, 200-600 m from the player in
        // PLANET-FRAME metres; a visible funnel needs the
        // planet-spin transform the celestial pass owns,
        // so it is deferred with the damage rung.
        let vortex_hazard = if w.event_id.is_empty() {
            None
        } else {
            state
                .data_store
                .get::<humanity_systems::weather_events::WeatherEventRegistry>(
                    "weather_event_registry",
                )
                .and_then(|r| r.events.iter().find(|e| e.id == w.event_id))
                .filter(|e| {
                    matches!(
                        e.wind_profile,
                        humanity_systems::weather_events::WindProfile::Vortex { .. }
                    )
                })
                .map(|e| e.hazard.radius_m)
        };
        match vortex_hazard {
            Some(radius_m) if state.frame_lock_body.is_some() => {
                let anchor = state.frame_lock_anchor;
                let core = *state.event_core.get_or_insert_with(|| {
                    // Cheap deterministic-enough rolls from
                    // the wall clock micros (placement is
                    // cosmetic; no gameplay fairness rides
                    // on it).
                    let us = state.start_time.elapsed().as_micros() as u64;
                    let bearing =
                        (us % 6283) as f64 / 1000.0;
                    let dist = 200.0 + (us / 7 % 400) as f64;
                    let c =
                        humanity_systems::weather_events::place_event_core(
                            anchor, bearing, dist,
                        );
                    log::info!(
                        "[WeatherEvent] vortex core placed {:.0} m from player",
                        (c - anchor).length()
                    );
                    c
                });
                let dist = (anchor - core).length();
                let (near, inside) =
                    humanity_systems::weather_events::hazard_proximity(
                        dist, radius_m,
                    );
                if near != state.event_hazard_near {
                    state.event_hazard_near = near;
                    log::info!(
                        "[WeatherEvent] hazard zone {} ({:.0} m from core, radius {radius_m} m)",
                        if near { "ENTERED (3x radius)" } else { "left" },
                        dist
                    );
                }
                if let Some(gw) = state.gui_state.weather.as_mut() {
                    gw.warning = if inside {
                        format!("{} - TAKE COVER!", w.event_name)
                    } else if near {
                        format!("{} nearby!", w.event_name)
                    } else {
                        String::new()
                    };
                }
            }
            _ => {
                if state.event_core.take().is_some() {
                    state.event_hazard_near = false;
                }
                if let Some(gw) = state.gui_state.weather.as_mut() {
                    gw.warning.clear();
                }
            }
        }
    }

}
