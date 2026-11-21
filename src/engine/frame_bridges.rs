//! The GUI's requests into the world, first half (the file-size split, step 1, 2026-11-14), moved
//! out of `window_event` in src/lib.rs as it was: built beds and chests, the dev teleport, the dev
//! surface landing, the dev spawn tool and the Dev page's creature count.
//!
//! Since the headless sim runner's increment 3 (2026-11-15) the world-feeding half runs as
//! `frame_sim::bridges_slice` on the sim's parts, the one the soak runs too: the body's environment
//! (where the person is read here as plain values, `BodyPlace`), the drone's panel and the GUI's
//! requests (this file's craft to deploy and frame_bridges_npc.rs's summon to the dev seed set,
//! now engine/frame_sim_requests.rs). What stays here reads the camera or the window.
//!
//! Order against the bodies it replaced (the read-after-write audit):
//! - The Planet Tuner's readout ran between the body's environment and the survival context; it
//!   runs after the slice now. It reads the frame lock, the planets, the weather and the body's
//!   environment (published first in the slice, as it was published just before it then; nothing
//!   later in the slice writes it again) and writes only the tuner page's readout, which the slice
//!   does not read.
//! - frame_bridges_npc.rs's requests (summon to the dev seed set) ran after the dev teleport, the
//!   dev surface landing, the dev spawn, the creature count, the creature editor and the NPC card;
//!   they run before them now. Those read the camera, the frame lock, the planets, the creature
//!   registry, the world's creatures and NPCs and their own `pending_dev_*`, edit and talk fields;
//!   the requests read only their own `pending_*` fields and the settings and write the systems'
//!   DataStore slots, `inflight_take_origins`, the garden's pests and its weed request. Neither side
//!   reads what the other writes, so every value each reads is the one it read before.
//! - The water tank card's Fill and Pour moved from frame_cards.rs (after the tick) into the
//!   requests (before it); see engine/frame_sim_requests.rs. The click lands on the frame it did,
//!   earlier in it: this frame's tick sees the tank and the bottles after the fill instead of the
//!   next frame's, and the card's list (still built in frame_cards.rs) reads them after it as before.

use crate::engine::frame_prelude::*;
use crate::engine::frame_sim::{BodyPlace, SimParts};
use crate::engine::planet_build::{GroundSources, PlayerView};

/// Carries this frame's GUI requests (beds, chests, dev tools) into the world.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.bridges");
    // Built beds and chests: prompt + chests in the places tree (2026-09-27).
    crate::engine::built_uses::frame(state);
    // The quests' glue: the own front door (2026-10-04; the views and guides are reported on
    // the step, frame_sim::bridges_slice).
    crate::engine::quest_hooks::frame(state);
    // A pack left where you fell, in reach: its prompt (2026-10-04).
    crate::engine::death_pack::frame(state);
    // A landing hard enough to hurt becomes the player's injury (falls, 2026-10-20).
    if let Some(impact) = state.controller.take_landing() {
        humanity_systems::falls::player_landed(&mut state.game_world.world, impact);
    }

    // The world-feeding half on the sim's parts (frame_sim::bridges_slice, the step the soak runs
    // too): the body's environment from where the person is, the drone's panel and the GUI's
    // requests. Where the person is, read here once as plain values.
    let (moving, sprinting) = state.controller.gait();
    let place = BodyPlace {
        view: PlayerView::of(state),
        moving,
        sprinting,
        driving: state.driving_vehicle.is_some(),
        fly_mode: state.controller.fly_mode,
        felt: humanity_systems::flight::felt_gravity(&state.flight, &state.flight_data.dampener),
        air: crate::engine::survival_env::air_spaces(state.gui_state.ship_structure.is_some(), &state.ship_air, state.homestead_bounds),
        ground: Some(GroundSources {
            defs: &state.planet_defs,
            heightmaps: &state.planet_heightmaps,
            tiles: &state.terrain_tiles,
            ocean: state.ocean_mask.as_ref(),
            chunk_states: &state.planet_chunk_states,
        }),
    };
    let parts = SimParts {
        runner: &mut state.system_runner,
        world: &mut state.game_world.world,
        data: &mut state.data_store,
        gui: &mut state.gui_state,
        world_loaded: state.world_loaded,
    };
    crate::engine::frame_sim::bridges_slice(parts, &place);
    crate::engine::frame_lock::publish_planet_tuner_readout(state);

    // Dev travel teleport (v0.791.x, Platform > Dev > Travel):
    // jump the viewpoint to any rendered solar body, or back
    // home. HOW it moves: planets sit up to 4.5e12 m away in
    // Earth-relative metres -- hopeless for the f32 local
    // camera (ulp at that scale is ~0.5e6 m) -- so the teleport
    // moves ship_world_pos (f64), the offset the celestial pass
    // subtracts from every body. The whole local frame (home +
    // player) arrives together and the camera stays at metre-
    // scale local coordinates; dev_travel::teleport_viewpoint
    // picks a sunlit vantage ~4 TRUE radii out (the sky pass's
    // min-angular-size disc floor is inactive at that range, so
    // visual radius == true radius and the body fills ~29
    // degrees of view on arrival).
    if let Some(target) = state.gui_state.pending_dev_teleport.take() {
        // Shared-world handling (v0.801, operator: "As an admin this
        // should be overridden. If I can't teleport then I can't
        // moderate."): in DEV play mode the teleport STEPS OUT of the
        // shared world first (game_leave via copresence_solo - others
        // see you leave honestly, no frozen ghost avatar, no fighting
        // the position sync), travels, and Return home steps back in.
        // Non-Dev modes keep the hard gate.
        let dev_mode =
            state.gui_state.settings.play_mode == crate::config::PlayMode::Dev;
        if state.gui_state.copresence_active && !dev_mode {
            log::warn!("Dev travel: requires Dev play mode while in the shared world");
        } else if target == "home" {
            if let Some((ship, cam, yaw, pitch)) = state.dev_travel_home.take() {
                // Drop the surface FPS basis (task #76) before
                // restoring the saved world-basis yaw/pitch, or
                // they would be reinterpreted in the tangent
                // frame and the home view would point wrong.
                state.camera.clear_surface();
                // v0.911 (operator: "TPd back to home [and] the
                // home wasn't actually there but all the markers
                // ... were displaying"): since the v0.881 orbital
                // station, the home MOVES at ~7.66 km/s, so the
                // position stashed at departure is thousands of
                // km stale after any wander - the player returned
                // to empty space while the label overlays (which
                // never ride station_off) still projected home
                // content. Dock to the station's CURRENT frame;
                // the stashed f64 is only the fallback for the
                // pre-orbit window where the station position is
                // still zero.
                let station = state.station_world_pos;
                if station.length_squared() > 1.0 {
                    state.ship_world_pos = station;
                    state.station_ride = true;
                } else {
                    state.ship_world_pos = ship;
                }
                state.camera.roll = 0.0;
                state.camera.position = cam;
                state.camera.yaw = yaw;
                state.camera.pitch = pitch;
                // Back inside the hull: fly mode off, gravity +
                // walls + survival rules resume.
                state.gui_state.dev_fly_mode = false;
                state.controller.fly_mode = false;
                state.gui_state.dev_travel_away = false;
                // Back home: release the planet frame-lock, anchor and
                // all (v0.1225 - see the other release site).
                state.frame_lock_body = None;
                state.frame_lock_anchor = glam::DVec3::ZERO;
                // Rejoin the shared world if this session was shared
                // before travel stepped out (launcher intent rules:
                // only clear solo if the step-out set it).
                if state.dev_travel_stepped_out {
                    state.dev_travel_stepped_out = false;
                    state.gui_state.copresence_solo = false;
                }
                log::info!("Dev travel: returned home");
            }
        } else if let Some(body) = humanity_core::cosmos::find_body(&target) {
            // Engaging travel while joined: step out first (the solo
            // branch above sends game_leave next frame).
            if state.gui_state.copresence_active && !state.gui_state.copresence_solo {
                state.gui_state.copresence_solo = true;
                state.dev_travel_stepped_out = true;
                log::info!("Dev travel: stepping out of the shared world to travel");
            }
            // Same live sim time the celestial pass uses, so the
            // vantage matches where the body is drawn this frame.
            let sim_t = state.clock.sim_epoch_s; // the frame's clock (frame_sim::FrameClock)
            let earth_helio_au = humanity_core::cosmos::find_body("earth")
                .map(|e| humanity_core::cosmos::body_world_position_3d_au(e, sim_t))
                .unwrap_or(glam::DVec3::ZERO);
            let body_rel_earth =
                (humanity_core::cosmos::body_world_position_3d_au(body, sim_t)
                    - earth_helio_au)
                    * humanity_core::cosmos::M_PER_AU;
            // Sun's heliocentric position is the origin, so
            // Earth-relative it is just -earth.
            let sun_rel_earth = -earth_helio_au * humanity_core::cosmos::M_PER_AU;
            let is_sun = body.body_type == "star";
            let vantage = humanity_core::dev_travel::teleport_viewpoint(
                body_rel_earth,
                sun_rel_earth,
                body.radius_km * 1000.0,
                is_sun,
            );
            // Stash home exactly once (first departure), so
            // chained teleports still return to the true home.
            if state.dev_travel_home.is_none() {
                state.dev_travel_home = Some((
                    state.ship_world_pos,
                    state.camera.position,
                    state.camera.yaw,
                    state.camera.pitch,
                ));
            }
            state.ship_world_pos = vantage;
            // Park the camera just above the hull so the ship
            // doesn't block the view (it rides along below you),
            // and aim at the body. First person + fly mode so
            // gravity/walls don't yank the traveler around.
            if state.camera.mode
                != crate::renderer::camera::CameraMode::FirstPerson
            {
                state
                    .camera
                    .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
            }
            let hull_top = state
                .homestead_bounds
                .map(|(_, mx)| mx.y)
                .unwrap_or(20.0);
            state.camera.position = Vec3::new(0.0, hull_top + 30.0, 0.0);
            // Reset to the world-Y basis before aiming: if
            // surface FPS mode (task #76) is still engaged from a
            // previous surface visit, these world-basis yaw/pitch
            // would be misread in the tangent frame. The frame-
            // lock re-engages surface mode on descent and
            // preserves the look direction across the change.
            state.camera.clear_surface();
            let (yaw, pitch) =
                humanity_core::dev_travel::look_angles(body_rel_earth - vantage);
            state.camera.yaw = yaw;
            state.camera.pitch = pitch;
            state.gui_state.dev_fly_mode = true;
            state.controller.fly_mode = true; // effective this frame
            state.gui_state.dev_travel_away = true;
            // Frame-lock onto the target so its surface holds
            // still (see the field docs + dev_travel::frame_lock_*).
            {
                let rot = crate::engine::planet_build::body_world_rot(&target, state.current_spin, state.clock.sim_epoch_s);
                let cam_local = glam::DVec3::new(
                    state.camera.position.x as f64,
                    state.camera.position.y as f64,
                    state.camera.position.z as f64,
                );
                state.frame_lock_anchor = humanity_core::dev_travel::frame_lock_capture(
                    body_rel_earth,
                    rot,
                    vantage + cam_local,
                );
                state.frame_lock_last_rot = rot;
                state.frame_lock_body = Some(target.clone());
            }
            log::info!(
                "Dev travel: teleported to {} ({:.0} km out, sunlit side)",
                body.name,
                (body_rel_earth - vantage).length() / 1000.0
            );
        } else {
            log::warn!("Dev travel: unknown body id {target}");
        }
    }

    // Dev SURFACE landing (2026-07-12): drop to LOW altitude over
    // a surface point + engage surface mode, instead of the 4-radii
    // orbit `pending_dev_teleport` gives (operator: the GUI teleport
    // left the planet "rotating super fast underneath" because it
    // parks in orbit; the AI camera tool lands on the surface via
    // lat/lon). This mirrors poll_camera_request's lat/lon branch:
    // park just above the DRAWN ground at a default coordinate so
    // the per-frame frame-lock engages surface mode (radial up,
    // gravity settle, planet held still) next frame.
    if let Some(target) = state.gui_state.pending_dev_surface.take() {
        let dev_mode =
            state.gui_state.settings.play_mode == crate::config::PlayMode::Dev;
        if state.gui_state.copresence_active && !dev_mode {
            log::warn!("Dev travel: surface landing requires Dev play mode while in the shared world");
        } else if let Some(body) = humanity_core::cosmos::find_body(&target) {
            if state.gui_state.copresence_active && !state.gui_state.copresence_solo {
                state.gui_state.copresence_solo = true;
                state.dev_travel_stepped_out = true;
            }
            let sim_t = state.clock.sim_epoch_s; // the frame's clock (frame_sim::FrameClock)
            let earth_helio_au = humanity_core::cosmos::find_body("earth")
                .map(|e| humanity_core::cosmos::body_world_position_3d_au(e, sim_t))
                .unwrap_or(glam::DVec3::ZERO);
            let body_rel_earth = (humanity_core::cosmos::body_world_position_3d_au(body, sim_t)
                - earth_helio_au)
                * humanity_core::cosmos::M_PER_AU;
            // A pleasant default landing coordinate: Earth -> the
            // Oahu coast (a known-good scenic spot); other bodies
            // -> the equatorial prime meridian.
            let (lat, lon) = if target == "earth" {
                (21.3_f32, -157.8_f32)
            } else {
                (0.0_f32, 0.0_f32)
            };
            let altitude_km = 1.5_f64;
            // The target body's own turn (its lat/lon are on ITS ground).
            let rot = crate::engine::planet_build::body_world_rot(&target, state.current_spin, state.clock.sim_epoch_s);
            let unit = humanity_terrain::planet_heightmap::latlon_to_dir(lat, lon);
            let world_dir = rot
                * glam::DVec3::new(unit.x as f64, unit.y as f64, unit.z as f64);
            let dir_out = world_dir.normalize_or_zero();
            let surface_radius = ground_radius_m(
                state.planet_defs.get(&target),
                state.planet_heightmaps.get(&target).map(|a| a.as_ref()),
                None,
                None,
                unit.as_dvec3(),
            );
            let vantage =
                body_rel_earth + dir_out * (surface_radius + altitude_km * 1000.0);
            if state.dev_travel_home.is_none() {
                state.dev_travel_home = Some((
                    state.ship_world_pos,
                    state.camera.position,
                    state.camera.yaw,
                    state.camera.pitch,
                ));
            }
            state.ship_world_pos = vantage;
            if state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson {
                state
                    .camera
                    .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
            }
            let hull_top =
                state.homestead_bounds.map(|(_, mx)| mx.y).unwrap_or(20.0);
            state.camera.position = Vec3::new(0.0, hull_top + 30.0, 0.0);
            // Aim toward the horizon (85 deg off straight-down) so
            // the landing view shows ground + sky; surface mode
            // preserves this look across the basis change.
            let fwd = (body_rel_earth - vantage).normalize_or_zero();
            let aim = {
                let mut right = fwd.cross(glam::DVec3::Y);
                if right.length_squared() < 1e-9 {
                    right = fwd.cross(glam::DVec3::X);
                }
                let right = right.normalize_or_zero();
                let ang = 85.0_f64.to_radians();
                (fwd * ang.cos() + right.cross(fwd) * ang.sin()).normalize_or_zero()
            };
            state.camera.clear_surface();
            let (yaw, pitch) = humanity_core::dev_travel::look_angles(aim);
            state.camera.yaw = yaw;
            state.camera.pitch = pitch;
            state.gui_state.dev_fly_mode = true;
            state.controller.fly_mode = true;
            state.gui_state.dev_travel_away = true;
            let cam_local = glam::DVec3::new(
                state.camera.position.x as f64,
                state.camera.position.y as f64,
                state.camera.position.z as f64,
            );
            state.frame_lock_anchor = humanity_core::dev_travel::frame_lock_capture(
                body_rel_earth,
                rot,
                vantage + cam_local,
            );
            state.frame_lock_last_rot = rot;
            state.frame_lock_body = Some(target.clone());
            log::info!(
                "Dev travel: landed on {} surface at ({:.1}, {:.1}), {:.1} km alt (surface mode engages next frame)",
                body.name,
                lat,
                lon,
                altitude_km
            );
        } else {
            log::warn!("Dev travel: unknown body id {target} (surface)");
        }
    }

    // Dev spawn tool (v0.777, Platform > Dev): spawn any creature/
    // NPC ~2 m in front of the player, despawn them all, and
    // publish the live count for the Dev page. Reads the camera
    // pose (published earlier this frame) and the CreatureRegistry
    // the game already loaded, so the spawned entity is identical
    // to a placed one (renders + walk-up + combat all work).
    if let Some(def_id) = state.gui_state.pending_dev_spawn.take() {
        let cam = state
            .data_store
            .get::<Vec3>("camera_position")
            .copied()
            .unwrap_or(Vec3::ZERO);
        let fwd = state
            .data_store
            .get::<Vec3>("camera_forward")
            .copied()
            .unwrap_or(Vec3::NEG_Z);
        // Flatten to horizontal so it lands beside you, not in the air.
        let flat = Vec3::new(fwd.x, 0.0, fwd.z);
        let flat = if flat.length_squared() > 1e-4 {
            flat.normalize()
        } else {
            Vec3::NEG_Z
        };
        // Ground it at the player's feet. NO player entity means the
        // world hasn't been entered yet (main menu / fresh boot) --
        // skip instead of spawning at a default camera pose that
        // load_world's despawn-and-rebuild would silently wipe on
        // world entry anyway (the "spawn randomly failed" trap). (v0.779)
        let feet_y = state
            .game_world
            .world
            .query::<(&Controllable, &Transform)>()
            .iter()
            .next()
            .map(|(_, (_, t))| t.position.y);
        if let Some(feet_y) = feet_y {
            let pos = Vec3::new(cam.x + flat.x * 2.0, feet_y, cam.z + flat.z * 2.0);
            let items = state.data_store.get::<ItemRegistry>("item_registry");
            if let Some(reg) = state
                .data_store
                .get::<humanity_systems::livestock::CreatureRegistry>("creature_registry")
            {
                if let Some(def) = reg.get(&def_id) {
                    let e = humanity_systems::livestock::spawn_creature_at(
                        &mut state.game_world.world,
                        def,
                        items,
                        pos,
                        [0.7, 0.6, 0.5],
                    );
                    log::info!("Dev: spawned {} ({:?}) at {:?}", def.name, e, pos);
                } else {
                    log::warn!("Dev spawn: unknown creature id {def_id}");
                }
            }
        } else {
            log::warn!("Dev spawn: no player in the world yet -- enter the world first");
        }
    }
    if std::mem::take(&mut state.gui_state.pending_dev_despawn_creatures) {
        let all: Vec<hecs::Entity> = state
            .game_world
            .world
            .query::<&humanity_core::ecs::components::Creature>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        let n = all.len();
        for e in all {
            let _ = state.game_world.world.despawn(e);
        }
        log::info!("Dev: despawned {n} creatures");
    }
    // Live creature count for the Dev page. Only refreshed while the
    // Dev tab is open (its only consumer) -- no reason to walk
    // the archetypes at 60fps during normal play. (v0.779)
    if state.gui_state.active_page == GuiPage::Dev {
        state.gui_state.dev_creature_count = state
            .game_world
            .world
            .query::<&humanity_core::ecs::components::Creature>()
            .iter()
            .count();
    }

}
