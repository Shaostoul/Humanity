//! Walking on a planet (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as it was: the planet frame lock, which keeps the player standing on a turning
//! body, its ground and ocean contact, and flight near the surface.

use crate::engine::frame_prelude::*;

/// Moves the player on or above a planet while the frame lock holds.
pub(crate) fn run(state: &mut EngineState) {
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.planet_walk");
    // Planet frame-lock (v0.819): while locked onto a body (dev
    // travel / camera_request), ride its rotating + orbiting
    // frame so the surface holds STILL for the viewer instead of
    // sweeping past at ~64 km/s (the planet spins a fast visible
    // day). Runs AFTER the fly integration above: while the
    // operator world-thrusts (or is far from the body) we only
    // refresh the anchor so releasing thrust re-locks HERE; while
    // parked near the body we set ship_world_pos + a matching yaw
    // delta so both position and view co-rotate. Pure math +
    // tests in dev_travel::frame_lock_*.
    // Surface HUD readout resets each frame; the engage branch
    // below repopulates it while a surface is actually locked.
    state.gui_state.surface_altitude_m = None;
    state.gui_state.surface_gravity_now = None;
    // Underwater resets UNCONDITIONALLY here (the surface
    // branch re-sets it when truly submerged): a teleport
    // that leaves the lock envelope skips that branch the
    // frame it releases, and the tint + depth HUD used to
    // LATCH into deep space (first Agartha capture, 2026-08-13).
    state.gui_state.underwater = false;
    state.gui_state.underwater_depth_m = 0.0;
    // ── Frame-lock AUTO-SWITCH by proximity (v0.927, operator:
    // "I teleported to the moon then flew to the Earth and it
    // wouldn't act like I was reentering... I just flew as if
    // I wasn't transitioning to the surface of a planet"):
    // the locked body used to change only on TELEPORT, so a
    // manual interplanetary flight kept the OLD body's frame
    // and the destination's surface bands never engaged. Each
    // frame, find the body whose scaled frame envelope (the
    // per-body inertial-blend ceiling) the camera sits inside;
    // switch with hysteresis (enter below 0.9x, leave above
    // 1.2x) and capture a fresh anchor at the CURRENT spot so
    // nothing jumps. The station keeps precedence - the ship
    // IS the frame while riding OR aboard (v0.928.x, operator:
    // "not spawning in my home... just floating in space
    // above Earth" - the station orbits INSIDE Earth's
    // envelope, so during spawn/docking frames where the ride
    // flag was not yet set the switch grabbed Earth's frame
    // with a degenerate anchor and Earth's surface bands
    // yanked the player out of the hull).
    if !state.station_ride && !state.aboard_station {
        let sim_t = state.clock.sim_epoch_s; // the frame's clock (frame_sim::FrameClock)
        let earth_helio = humanity_core::cosmos::find_body("earth")
            .map(|e| humanity_core::cosmos::body_world_position_3d_au(e, sim_t))
            .unwrap_or(glam::DVec3::ZERO);
        let cam_world = state.ship_world_pos
            + glam::DVec3::new(
                state.camera.position.x as f64,
                state.camera.position.y as f64,
                state.camera.position.z as f64,
            );
        let body_ratio = |id: &str| -> Option<(glam::DVec3, f64)> {
            let b = humanity_core::cosmos::find_body(id)?;
            if b.body_type == "star" {
                return None;
            }
            let center = if b.id == "earth" {
                glam::DVec3::ZERO
            } else {
                (humanity_core::cosmos::body_world_position_3d_au(b, sim_t)
                    - earth_helio)
                    * humanity_core::cosmos::M_PER_AU
            };
            let radius = b.radius_km * 1000.0;
            let band_k = (radius / 6_371_000.0).clamp(0.001, 4.0);
            let ceiling = (INERTIAL_BLEND_MAX_ALT * band_k).max(20_000.0);
            let alt = (cam_world - center).length() - radius;
            Some((center, alt / ceiling))
        };
        let mut best: Option<(String, f64, glam::DVec3)> = None;
        for b in humanity_core::cosmos::sol_bodies() {
            if let Some((center, ratio)) = body_ratio(&b.id) {
                // ratio <= 0 = camera AT or INSIDE the body's
                // radius: a degenerate placement (spawn frames
                // before the station snap, mid-teleport), not
                // a real approach. Never engage from it - the
                // captured anchor would be garbage and the
                // surface bands would clamp the player to the
                // planet (v0.928.x spawn regression).
                if ratio > 0.0
                    && ratio < 1.0
                    && best.as_ref().map_or(true, |(_, r, _)| ratio < *r)
                {
                    best = Some((b.id.clone(), ratio, center));
                }
            }
        }
        let cur_ratio = state
            .frame_lock_body
            .as_deref()
            .and_then(|id| body_ratio(id))
            .map(|(_, r)| r);
        match (&state.frame_lock_body, &best) {
            (cur, Some((cand, ratio, center)))
                if *ratio < 0.9 && cur.as_deref() != Some(cand.as_str()) =>
            {
                // The new body's own turn, never Earth's (2026-10-21).
                let rot = crate::engine::planet_build::body_world_rot(cand, state.current_spin, state.clock.sim_epoch_s);
                state.frame_lock_anchor = humanity_core::dev_travel::frame_lock_capture(
                    *center, rot, cam_world,
                );
                state.frame_lock_last_rot = rot;
                state.frame_lock_body = Some(cand.clone());
                log::info!(
                    "[FrameLock] switched to {cand} by proximity (envelope {:.2})",
                    ratio
                );
            }
            (Some(cur), _) if cur_ratio.is_none_or(|r| r > 1.2) => {
                log::info!("[FrameLock] released {cur} (left its envelope)");
                state.frame_lock_body = None;
                // Clear the ANCHOR too (v0.1225): it is read outside
                // the frame_lock_body guard when local up is published,
                // so leaving it set lets a stale anchor from an earlier
                // planet visit keep steering the lighting.
                state.frame_lock_anchor = glam::DVec3::ZERO;
            }
            _ => {}
        }
    }
    if let Some(lock_body) = state.frame_lock_body.clone() {
        // The locked body's own turn (2026-10-21; it was Earth's spin for every
        // body, so the Moon turned with Earth's day).
        let rot = crate::engine::planet_build::frame_body_rot(state);
        // Body centre in the celestial frame (0 for Earth, the
        // frame origin; real orbital position otherwise).
        let body_center = if lock_body == "earth" {
            glam::DVec3::ZERO
        } else {
            let sim_t = state.clock.sim_epoch_s; // the frame's clock (frame_sim::FrameClock)
            let earth_helio = humanity_core::cosmos::find_body("earth")
                .map(|e| humanity_core::cosmos::body_world_position_3d_au(e, sim_t))
                .unwrap_or(glam::DVec3::ZERO);
            humanity_core::cosmos::find_body(&lock_body)
                .map(|b| {
                    (humanity_core::cosmos::body_world_position_3d_au(b, sim_t)
                        - earth_helio)
                        * humanity_core::cosmos::M_PER_AU
                })
                .unwrap_or(glam::DVec3::ZERO)
        };
        let cam_local = glam::DVec3::new(
            state.camera.position.x as f64,
            state.camera.position.y as f64,
            state.camera.position.z as f64,
        );
        let cam_world = state.ship_world_pos + cam_local;
        let to_cam = cam_world - body_center;
        let dist = to_cam.length();

        // ── Surface FPS engage test (task #76 increment 1) ──
        // The unrotated surface point under the camera is the
        // frame-lock anchor's direction; sample the real ground
        // radius there (heightmap -> displaced_radius, which
        // carries earth.ron's vertical exaggeration so the stand
        // height matches the DRAWN ground). Engage surface mode
        // ONLY when genuinely close (within SURFACE_ENGAGE_ALT of
        // that ground): the co-rotation that holds a surface point
        // still belongs to the planet's ROTATING frame, which is
        // only physical near the surface. In orbit the correct
        // look is INERTIAL (stars fixed, planet turning beneath -
        // the ISS view), so above this altitude we do NOT co-rotate
        // (operator feedback: holding the frame out to tens of
        // radii slowly wheeled the whole sky).
        let radial = if dist > 1.0 {
            (to_cam / dist).as_vec3()
        } else {
            glam::Vec3::Y
        };
        let anchor_pre =
            humanity_core::dev_travel::frame_lock_capture(body_center, rot, cam_world);
        let def = state.planet_defs.get(&lock_body);
        let hm = state.planet_heightmaps.get(&lock_body).map(|a| a.as_ref());
        // ── WHERE THE ALTITUDE IS MEASURED (v0.1109.4) ──
        // `anchor_pre` trails the player by one frame of
        // planetary spin (33,360 m/s of ground slip on the
        // 20-minute day: 1.1 km at 30 fps, 4.7 km at 7), so
        // sampling terrain there made `alt` a function of the
        // FRAME RATE - and `alt` gates the walk clamp's
        // DRAWN_GROUND_MAX_ALT_M, which flips stand height by
        // 2.45 m. That is the operator's "look up and I fall,
        // look level and I float". See surface_move::
        // ground_sample_dir for the full derivation.
        let ground_dir = humanity_systems::surface_move::ground_sample_dir(
            anchor_pre,
            state.frame_lock_anchor,
            state.camera.surface_mode,
        );
        // ...and against the ground the CLAMP stands you on -
        // base grid + land-masked detail + the streamed tile
        // tier - not the bare 5.5 km base grid, which reads
        // tens of metres under your boots. Built once here and
        // reused by the clamp below.
        let detail = def.map(|d| {
            humanity_terrain::planet_chunks::DetailNoise::new(d.terrain_seed)
        });
        let ground_r = ground_radius_m(
            def,
            hm,
            detail.as_ref(),
            (lock_body == "earth").then_some(&state.terrain_tiles),
            ground_dir,
        );
        // Altitude bands (v0.872; per-body scaled v0.909,
        // operator: "the distances for any planet are fixed
        // to Earth's settings"): walk/gravity, co-rotating
        // flight, and the up-vector blend all stretch with
        // body radius - Earth keeps 10/100/1000 km, the Moon
        // gets ~2.7/27/273 km, an asteroid gets metres, with
        // floors so tiny rocks still have usable bands.
        // Altitude reference (v0.909.x): ground_radius_m
        // returns the TRUE seafloor on water worlds (the
        // diving change), so raw dist-ground_r over a 3 km
        // deep ocean read "Alt 3.1 km" while you hovered
        // 100 m over the waves - and the walk band engaged
        // 3 km early. Altitude is measured from whichever
        // surface you would actually stand on: the sea
        // surface over open ocean, the ground on land.
        // Divers still go negative (relative to sea), which
        // keeps every band test sane underwater.
        let sea_r = def
            .filter(|d| d.has_water)
            .map(|d| d.radius)
            .unwrap_or(0.0);
        let alt = dist - ground_r.max(sea_r);
        let band_k = (def.map(|d| d.radius).unwrap_or(6_371_000.0)
            / 6_371_000.0)
            .clamp(0.001, 4.0);
        let surface_engage_alt = (SURFACE_ENGAGE_ALT * band_k).max(200.0);
        let co_rotate_max_alt = (CO_ROTATE_MAX_ALT * band_k).max(2_000.0);
        let inertial_blend_max_alt =
            (INERTIAL_BLEND_MAX_ALT * band_k).max(20_000.0);
        // Edge hysteresis (v0.909, operator: "the transitions
        // ... keep breaking and locking me at fixed
        // distances"): hovering exactly AT a band edge used
        // to flip the mode every frame (translation
        // ownership toggled, gear reset re-armed), which
        // felt like an invisible wall. Once inside a band
        // you stay in it until ~6% past its ceiling.
        let walk_h = if state.surface_walk_band { 1.06 } else { 1.0 };
        let coro_h = if state.camera.surface_mode { 1.06 } else { 1.0 };
        let in_walk_band = alt < surface_engage_alt * walk_h;
        let in_corotate_band = alt < co_rotate_max_alt * coro_h;
        // (underwater reset: unconditional block by
        // surface_altitude_m, see the teleport-latch note.)
        let in_blend_band = alt < inertial_blend_max_alt;
        // Night fill dimming (v0.998, operator: "trees were
        // still being illuminated at night"): the cool fill
        // light is a camera-frame constant that never set with
        // the sun, so night forests glowed. Scale it by the
        // camera-local daylight while near a body; a whisper
        // (0.10) stays so night is readable, not void-black.
        // Space keeps 1.0 (the approved orbital look) via the
        // blend to 1.0 across the inertial handoff band.
        {
            let sun_d =
                (state.sun_world_pos - state.ship_world_pos).normalize_or_zero();
            let sun_f = Vec3::new(sun_d.x as f32, sun_d.y as f32, sun_d.z as f32);
            let elev = radial.dot(sun_f);
            let t = ((elev + 0.05) / 0.20).clamp(0.0, 1.0);
            let daylight = t * t * (3.0 - 2.0 * t);
            let near_scale = 0.10 + 0.90 * daylight;
            // Fade the dimming back out across the blend band so
            // the inertial/orbit regime sees the full fill.
            let w_space = ((alt - co_rotate_max_alt)
                / (inertial_blend_max_alt - co_rotate_max_alt))
                .clamp(0.0, 1.0) as f32;
            // The sky pass's fill scale: the view's, set only when there is one (increment 4).
            if let Some(gpu) = state.gpu.as_mut() {
                gpu.renderer.fill_scale = near_scale * (1.0 - w_space) + w_space;
            }
        }

        if in_corotate_band {
            // Was surface mode already engaged last frame? MUST be
            // read BEFORE set_surface_up (which flips it true).
            let just_engaged = !state.camera.surface_mode;
            // Touchdown gear reset (v0.867, operator "weird issues
            // moving around" report): the mouse-wheel fly multiplier
            // CARRIES OVER from space flight, so arriving from an
            // FTL descent left walking geared up to 2000x - one W
            // tap crossed a county. Snap the wheel to 1x when
            // entering the WALK band specifically (v0.872: the
            // co-rotate band now reaches 100 km, where a gear
            // reset would be premature).
            if in_walk_band && (!state.surface_walk_band || just_engaged) {
                // v0.1006: also fires when SPAWNING directly
                // into the band (just_engaged) - the operator
                // spawned at 1.5 km with the wheel still at
                // 1e9 from FTL, and the edge-only reset never
                // saw an edge ([Dive] diag: gear=1000000000
                // inside the walk band).
                state.controller.fly_speed_mult = 1.0;
            }
            state.surface_walk_band = in_walk_band;
            state.surface_owns_translation = true;
            // Surface HUD readout (v0.867): altitude above the
            // drawn ground + wheel gear, so height and speed are
            // never a guess while landing/walking.
            state.gui_state.surface_altitude_m = Some(alt.max(0.0) as f32);
            state.gui_state.surface_speed_mult =
                state.controller.fly_speed_mult as f32;
            // Look co-rotation (v0.994, operator: "my cursor
            // seems to follow the sun even though I'm not
            // moving"): the position anchor rides the spin
            // below, but set_surface_up preserves the WORLD
            // forward - so the aim stayed inertial and the
            // ground panned under a standing player. Nudge the
            // look by this frame's spin delta first, so aim is
            // planet-fixed like everything else down here. The
            // 0.01 rad guard skips stale spins (teleports).
            if !just_engaged {
                let d_rot = rot * state.frame_lock_last_rot.inverse();
                let d_ang = crate::engine::planet_build::turn_angle(d_rot);
                if d_ang > 1e-12 && d_ang < 0.01 {
                    state.camera.co_rotate_look(d_rot.as_quat());
                }
            }
            // Radial "up" so "down" points at the body centre and
            // the horizon is level (the transition preserves the
            // current look direction across the basis change).
            state.camera.set_surface_up(radial);

            // ── CO-ROTATION (bug fix 2026-07-12) ──
            // The anchor is the camera's position in the planet's
            // UNROTATED local frame. It must be captured ONCE, on
            // the frame surface mode engages, then RIDE the spin:
            // ship_world_pos = body_center + Ry(spin)*anchor -
            // cam_local grows Ry(spin) with the planet mesh, so the
            // camera co-rotates and the ground holds STILL. The old
            // code re-captured anchor_pre = Ry(-spin)*(cam_world -
            // bc) EVERY frame and re-placed it at the SAME spin,
            // which is the identity (ship_world_pos unchanged) --
            // so the camera stayed put in world space while the
            // mesh spun under it (operator: "the Earth spins
            // without me... it clips through me"). Persist the
            // anchor across frames instead; only walk + gravity
            // move it, in the unrotated frame.
            let mut anchor = if just_engaged {
                anchor_pre
            } else {
                state.frame_lock_anchor
            };
            // Diving (v0.903, operator: "I can't go into the
            // water... the water surface is still solid"):
            // below the nominal sea radius the camera is
            // UNDERWATER - movement flies where you look,
            // the true bathymetry is the floor (the Marianas
            // trench is reachable), and depth holds
            // neutrally until you swim up or down.
            let submerged = def
                .map(|d| anchor.length() < d.radius - 1.0)
                .unwrap_or(false);
            // Wish semantics per band (v0.882, operator: "I
            // have to press shift to descend even though I'm
            // looking down"): WALKING keeps the tangent-plane
            // WASD (surface_wish_dir - looking at your feet
            // must not drive you into the ground), but the
            // FLIGHT band (10-100 km) flies WHERE YOU LOOK,
            // exactly like FTL above it: W with the nose down
            // descends, no separate descend key needed.
            let dir0 = anchor.normalize_or_zero();
            // ── MOVEMENT MODEL (v0.1109) ── Walk or dev
            // flight; see crates/humanity-systems/src/surface_move.rs for the report
            // this split exists for. It is a VIEW of the
            // existing dev-flight flag (F9 / Dev page), so
            // the HUD word and the physics read one bit.
            let move_mode = humanity_systems::surface_move::MoveMode::from_dev_flight(
                state.gui_state.dev_hover,
            );
            // v0.1006 (operator: "I'm flying perpendicularly
            // to the surface but I seem to just be in
            // constant freefall"): DEV FLY MODE flies where
            // you look in EVERY band - the tangent-plane
            // walking wish is for feet on the ground, and
            // forcing it below 10 km made nose-down + W do
            // nothing vertical while gravity pulled.
            let wish = if move_mode.uses_tangent_wish(in_walk_band, submerged) {
                state.controller.surface_wish_dir(&state.camera)
            } else {
                // Flight band above, SWIMMING below (v0.903):
                // both move where you look. Space/Shift thrust
                // along the LOCAL RADIAL up (v0.923, operator:
                // "the moment I press space the planet and I
                // seem to decouple") - world +Y is TANGENTIAL
                // at the equator, so the old fixed axis slid
                // you along the surface instead of lifting.
                let up_w = rot * dir0;
                state.controller.fly_wish_dir_up(
                    &state.camera,
                    Vec3::new(up_w.x as f32, up_w.y as f32, up_w.z as f32),
                )
            };
            let wish_unrot = rot.inverse()
                * glam::DVec3::new(wish.x as f64, wish.y as f64, wish.z as f64);
            // WALK quantises the radial share to exactly
            // {-1, 0, +1} so forward input can NEVER lift
            // (operator v0.1109: "while I press W I
            // immediately start flying"); dev flight keeps
            // the analog share so W still flies where you
            // look. See surface_move::split_wish.
            let (tangential, radial_wish) =
                humanity_systems::surface_move::split_wish(wish_unrot, dir0, move_mode);
            // A carried load scales (an overload stops) the Space jump, BUG-136.
            let radial_wish = humanity_systems::surface_move::carry_gated_radial(
                radial_wish, move_mode, in_walk_band, submerged, state.controller.jump_scale);
            // ── Unified flight speed (v0.880, operator stuck
            // at the 100 km boundary + "speed resets") ──
            // WALK band: the wheel stays bounded (~10 km/s) so
            // a tap never flings you off the planet.
            // FLIGHT band (10-100 km): the FULL wheel gear
            // works - same mental model as FTL above 100 km -
            // and safety comes from the same approach governor
            // instead of a hard cap: per-frame movement never
            // exceeds half the height above the ground, so ANY
            // gear glides in smoothly and hands over to the
            // walk band, which auto-settles. The old 2000x
            // clamp made the band a crawl (400 m/frame from
            // 100 km) while one wheel notch past the boundary
            // resumed FTL at a billion-x - the fast/slow
            // oscillation that kept the surface unreachable.
            // v0.1007 (operator: "I liked moving fast when I
            // had the speed maxed out... now I have to
            // teleport off Earth to get speed again"): the
            // walk-band gear cap applies to WALKING only.
            // Dev fly mode keeps the full wheel at every
            // altitude; safety comes from the approach
            // governor below (step <= half the height above
            // ground per frame), which makes climb-outs
            // exponential-fast and descents auto-glide
            // instead of clipping in - the same mental
            // model as the flight band and FTL.
            let surface_mult = if in_walk_band && !state.controller.fly_mode {
                (state.controller.fly_speed_mult as f64)
                    .clamp(1.0, SURFACE_SPEED_MULT_MAX)
            } else {
                (state.controller.fly_speed_mult as f64).max(1.0)
            };
            let walk_speed = (state.controller.speed
                * state.controller.speed_multiplier)
                .max(0.0) as f64
                * surface_mult
                // A carried load slows WALKING only (BUG-136).
                * humanity_systems::surface_move::carry_walk_factor(state.controller.carry_speed_factor,
                    move_mode, state.controller.fly_mode, in_walk_band, submerged);
            let step_cap = if in_walk_band && !state.controller.fly_mode {
                // Walking: the 50x gear clamp above already
                // bounds speed; no per-frame cap needed.
                f64::MAX
            } else if in_walk_band {
                // Fly mode near the ground: the approach
                // governor with a tighter floor (50 m/frame
                // ~ 3 km/s hugging the deck) - huge steps
                // high up, exponential glide-in low down.
                (alt * 0.5).max(50.0)
            } else {
                (alt * 0.5).max(200.0)
            };
            // Swim speed cap (v0.984, post-audit item 6 residue):
            // water is DENSE - underwater motion is capped at a
            // strong-swimmer ~2.5 m/s at gear 1. The mouse-wheel
            // gear scales it (v0.1016, operator: "we need to
            // increase the swim speed with the mouse wheel. When
            // I go under it feels like I'm normal speed despite
            // having dev mode on") - same multiplier that speeds
            // walking, so max walk gear swims ~125 m/s. Dev fly
            // mode stays exempt: noclip stays noclip.
            let step = if submerged && !state.controller.fly_mode {
                (walk_speed * dt as f64).min(2.5 * surface_mult * dt as f64)
            } else {
                (walk_speed * dt as f64).min(step_cap)
            };
            // (The detail-inclusive ground sampler that stands
            // the eye on the DRAWN mesh rather than the base
            // heightmap under it is built once above, where the
            // altitude reading needs the same reference.)
            // v0.1008 (operator: "I don't teleport up/down
            // but, forward/back/left/right" while holding
            // Space): a pure-vertical wish leaves ~1e-8 of
            // float residue on the tangent plane, and the
            // old 1e-9 guard + normalize() amplified that
            // residue into a FULL-SPEED lateral step in
            // whatever direction the rounding noise pointed
            // - kilometres per frame at high gear, and the
            // root of every "I move parallel to the Earth
            // on takeoff" report. Same deliberate-input
            // dead zone as the radial axis.
            if tangential.length() > humanity_systems::surface_move::RADIAL_WISH_EPS {
                let before = anchor;
                anchor += tangential.normalize() * step;
                // Built pieces are solid on the ground too (2026-09-28); fly mode stays noclip.
                if !state.controller.fly_mode {
                    let eye_h = state.controller.eye_height();
                    let reg = state.data_store.get::<humanity_systems::construction::BlueprintRegistry>("blueprint_registry");
                    anchor = crate::engine::planet_build::collide_on_site(&state.game_world.world, reg, &lock_body, before, anchor, eye_h);
                }
                // Skills rung 1: distance swum is swimming practice (data/skills/practice.json).
                if submerged && !state.controller.fly_mode {
                    humanity_systems::skills::practice_distance(&state.data_store, "swim", (anchor - before).length() as f32, None);
                }
            }
            // Ground radius at the (possibly moved) surface point.
            // v0.1108, THE 2.5 m STILT: the eye now rests on the
            // DRAWN patch face, the same reference grass (v0.1091)
            // and near trees (v0.1097) moved onto, instead of on
            // the elevation FIELD plus 2.5 m of slop to cover the
            // difference. Depth proxy = the finest patch the
            // renderer drew last frame, exactly what those two
            // harvests use, so player, grass and trees stand on
            // one surface. ground_radius_m stays the fallback (and
            // the altitude reference) for the frames before any
            // selection exists, and above DRAWN_GROUND_MAX_ALT_M,
            // where the drawn face costs 0.6 ms a frame to place
            // a floor nobody is standing on (see the measured
            // note on walk_ground_radius).
            let dir1 = anchor.normalize_or_zero();
            let tiles_for_clamp =
                (lock_body == "earth").then_some(&state.terrain_tiles);
            let ocean_for_clamp = (lock_body == "earth")
                .then_some(state.ocean_mask.as_ref())
                .flatten();
            let drawn_depth = state
                .planet_chunk_states
                .get(&lock_body)
                .and_then(|cs| cs.last_drawn.iter().map(|p| p.depth).max())
                .unwrap_or(0);
            let field_r =
                ground_radius_m(def, hm, detail.as_ref(), tiles_for_clamp, dir1);
            // The gate asks "are your feet on the ground", so
            // measure against the clamp's OWN reference at the
            // player's OWN direction - never the band altitude,
            // which is sampled before this frame's step and can
            // carry a different ground definition (v0.1109.4).
            let alt_over_ground = anchor.length() - field_r;
            let (g0, clearance) = humanity_systems::surface_walk::walk_ground_radius(
                field_r,
                def,
                hm,
                detail.as_ref(),
                tiles_for_clamp,
                ocean_for_clamp,
                drawn_depth,
                alt_over_ground,
                dir1,
            );
            let mut g = g0;
            // Real water (v0.876 Stage 1): over connected
            // ocean the terrain mesh is the SEAFLOOR, but the
            // player floats on the drawn water shell -- sea
            // radius + the analytic wave height (the CPU twin
            // of the shader's vertex displacement; drawn ==
            // sampled). Diving is the Stage 3 follow-up.
            // Diving gate (v0.903): the water surface is a
            // FLOOR only while at/above it and not holding
            // descend. Holding descend (Shift in fly wish)
            // pierces it; once submerged the floors are the
            // real seafloor from the bathymetry sampler.
            let pierce = submerged || radial_wish < -0.05;
            let mut wave_float = false;
            if lock_body == "earth" && !pierce {
                if let (Some(om), Some(d)) = (state.ocean_mask.as_ref(), def) {
                    if om.is_ocean(dir1.as_vec3()) {
                        wave_float = true;
                        // Same clock the shader's vertex
                        // displacement runs on (sun_color.w
                        // = start-relative seconds).
                        let t = state.start_time.elapsed().as_secs_f32();
                        // f64 all the way into the twin
                        // (v0.1018 audit find: this used to
                        // narrow to f32 BEFORE the wave
                        // phase - 0.5 m of position noise
                        // fed the buoyancy float).
                        let p = dir1 * d.radius;
                        // Shoaled twin (v0.957): g is the drawn
                        // seafloor here, so radius - g is the
                        // local depth - the float height dies
                        // toward the waterline exactly like the
                        // vertex displacement (drawn == sampled).
                        let depth_m = (d.radius - g).max(0.0) as f32;
                        // FFT-ocean mode swaps in its twin
                        // (v0.1029): same shoal damping, same
                        // clock, same array the GPU tile drew
                        // this frame.
                        let wave = if let Some(fft) = state
                            .ocean_fft
                            .as_ref()
                            .filter(|_| state.gui_state.settings.water_fft)
                        {
                            humanity_terrain::ocean_fft::wave_height_shoaled_fft_m(
                                p, t, depth_m, fft,
                            )
                        } else {
                            humanity_terrain::ocean_waves::wave_height_shoaled_m(
                                p, t, depth_m,
                            )
                        }
                            as f64;
                        // Ocean disaster events ride the same
                        // float (ABYSSAL rung 2): the RIDE
                        // height (rogue = envelope only, per
                        // the twin's documented rule), shoaled
                        // with the same live crest the
                        // shader's shoal fade reads.
                        let wave = if let Some(pin) =
                            state.ocean_event_pin.as_ref()
                        {
                            wave + pin
                                .events
                                .ride_height_m(pin.frame.project(p))
                                * humanity_terrain::ocean_waves::shoal_factor_at(
                                    depth_m,
                                    // The crest the sea is drawn with (the renderer's default with no view).
                                    state.gpu.as_ref().map_or(humanity_terrain::ocean_waves::MAX_WAVE_HEIGHT_M, |g| g.renderer.sea_crest_m),
                                ) as f64
                        } else {
                            wave
                        };
                        g = g.max(
                            d.radius
                                + humanity_terrain::ocean_waves::SURFACE_LIFT_M as f64
                                + wave,
                        );
                    }
                }
            }
            // v0.896 backstop: ground DEEP below sea level is
            // ocean floor even when the mask misses it (the
            // mask can still be loading right after world
            // entry, and its coastal cells miss fjords - an
            // Iceland park kept settling to the seafloor
            // under the water shell). Threshold 60 m keeps
            // genuine below-sea dry land (Death Valley)
            // walkable; deep basins float you at sea level.
            if lock_body == "earth" && !pierce {
                if let Some(d) = def {
                    if g < d.radius - 60.0 {
                        g = d.radius
                            + humanity_terrain::ocean_waves::SURFACE_LIFT_M as f64;
                    }
                }
            }
            // ── Ground-reference pop filter (v0.1008,
            // operator "terrain trenches / character bobs"
            // + the [Dive] diag showing g-R swinging -0.8
            // to +3.4 m at a near-fixed spot) ── detail
            // tiles streaming in/out change the SAMPLED
            // elevation by metres while the DRAWN terrain
            // crossfades smoothly, so the standing clamp
            // yanked the player up/down with every
            // residency change. When essentially
            // stationary (< ~3 m/s of lateral motion), a
            // ground jump is a data pop, not terrain: ease
            // toward it at 2 m/s. Real slopes while moving
            // stay instant, and wave floating keeps its
            // live bob (drawn == sampled).
            {
                let radius_m =
                    def.map(|d| d.radius).unwrap_or(6.371e6);
                let moved_m = ((dir1 - state.last_ground_dir)
                    * radius_m)
                    .length();
                if state.last_ground_r > 0.0
                    && !wave_float
                    && moved_m < 0.05
                {
                    let max_dv = 2.0 * dt as f64;
                    let dgap = g - state.last_ground_r;
                    if dgap.abs() > max_dv {
                        g = state.last_ground_r
                            + dgap.signum() * max_dv;
                    }
                }
                state.last_ground_dir = dir1;
                state.last_ground_r = g;
            }
            // Ease the CLEARANCE too, and unlike the ground
            // filter above do it whether moving or not: a jump
            // in ground radius can be a real slope, but a jump
            // in clearance is only ever the reference switching
            // between the drawn face (0.05 m) and the field
            // fallback (2.5 m). Applied raw that is a 2.45 m
            // teleport followed by a fall - the operator's
            // report, which they could reproduce standing
            // perfectly still because the drawn depth flickers
            // as patches stream.
            let clearance = {
                if state.last_clearance <= 0.0 {
                    state.last_clearance = clearance;
                }
                let step = 2.0 * dt as f64;
                let d = clearance - state.last_clearance;
                state.last_clearance += d.clamp(-step, step);
                state.last_clearance
            };
            let r0 = anchor.length();
            // Built pieces are floors too (2026-10-20): a roof,
            // a wall's top, a foundation under the body within a
            // step up of its feet (planet_build::built_floor_radius,
            // in the build site's frame, f64). Before, the floor
            // was the terrain alone and the walk went straight
            // through a roof. Fly mode stays noclip, as it does
            // for the walls (collide_on_site above).
            let built_top = if state.controller.fly_mode {
                None
            } else {
                let reg = state.data_store.get::<humanity_systems::construction::BlueprintRegistry>("blueprint_registry");
                crate::engine::planet_build::built_floor_radius(
                    &state.game_world.world,
                    reg,
                    &lock_body,
                    dir1,
                    r0 - humanity_core::ecs::components::EYE_HEIGHT_M,
                )
            };
            // Standing height AND the ground clamp - they are
            // the same floor by construction, so radial_step
            // only needs this one number: over the terrain, or
            // over the built top when that is higher.
            let rest = humanity_systems::surface_move::walk_rest(g, clearance, built_top);
            // Band entry / teleport never inherits a stale
            // fall speed from a previous surface session.
            if just_engaged {
                state.surface_vr = 0.0;
            }
            // ── ONE FRAME OF RADIAL MOTION (v0.1109) ──
            // Extracted whole to surface_move::radial_step so
            // the three regimes (walking ballistics + settle
            // glue, flight-band altitude hold, DEV FLIGHT
            // hover) can be gated headless. Dev flight takes
            // no gravity and no ground clamp and holds its
            // altitude bit-exactly with the stick still, which
            // is what the operator asked F9 for: "maintaining
            // some arbitrary height, like 5,000 feet or
            // whatever" for a hovering scenery shot.
            // Per-body gravity at the player's altitude:
            // constant for natural bodies, curve-varying on
            // manufactured worlds (artificial-planet.md).
            // 0.01 floor keeps declared 0 g bands weightless
            // without a literal 0; captured for F2's readout.
            let g_now = def
                .map(|d| d.gravity_at(r0 - d.radius) as f64)
                .unwrap_or(9.81)
                .max(0.01);
            state.gui_state.surface_gravity_now = Some(g_now as f32);
            let vs = humanity_systems::surface_move::radial_step(
                move_mode,
                in_walk_band,
                submerged,
                r0,
                state.surface_vr,
                rest,
                g_now,
                SURFACE_SETTLE_RATE,
                dt as f64,
                radial_wish,
                step,
            );
            let r = vs.r;
            state.surface_vr = vs.v_r;
            anchor = dir1 * r;
            state.frame_lock_anchor = anchor;
            state.frame_lock_last_rot = rot;
            // Grounded = standing within a boot's reach of the
            // rest height (v0.996): the footstep gate. Airborne
            // (jump thrust, falling, swimming, hovering in dev
            // flight) is silent - radial_step decides.
            state.on_ground_planet = vs.grounded;
            // Falls (2026-10-20, systems::falls): what the ground stopped
            // this frame, to the controller's watch. Water, dev flight (fly
            // mode, F9's hover, the flight band) and the rig's camera hold
            // hurt nobody.
            state.controller.surface_step(
                r0,
                vs.r,
                vs.landing(),
                move_mode.is_flight() || state.controller.fly_mode || !in_walk_band || state.probe_hold.is_some(),
                wave_float || submerged,
            );
            // Any has_water world tints when submerged (the
            // old earth-only gate predates per-body oceans;
            // Agartha's seas are as wet as the Pacific).
            state.gui_state.underwater =
                submerged && def.map(|d| d.has_water).unwrap_or(false);
            // v0.907: metres below sea level for the tint's
            // depth grading + HUD readout (planet def radius
            // IS sea level on has_water worlds).
            state.gui_state.underwater_depth_m = if state.gui_state.underwater {
                def.map(|d| (d.radius - r).max(0.0) as f32).unwrap_or(0.0)
            } else {
                0.0
            };
            // 1 Hz dive diag (dev tooling).
            {
                use std::sync::atomic::{AtomicU64, Ordering};
                static LASTD: AtomicU64 = AtomicU64::new(0);
                let now = state.clock.sim_epoch_s as u64; // the frame's clock, whole seconds (a log throttle)
                let below = def.map(|d| r < d.radius).unwrap_or(false);
                if LASTD.swap(now, Ordering::Relaxed) != now || below {
                    if let Some(d) = def {
                        // [FlightDiag] fields (v0.1005): the
                        // data source for the operator's
                        // backwards-takeoff / 100 km-jump /
                        // jitter reports - band, vertical
                        // velocity, gear, and the wish split
                        // in one greppable line.
                        log::info!(
                            "[Dive] sub={} eng={} band={} alt={:.0} r-R={:.1} g-R={:.1} rest-R={:.1} wish={:.3} tan={:.3} vr={:.2} gear={:.0} step={:.2}",
                            submerged,
                            just_engaged,
                            if in_walk_band { "walk" } else { "flight" },
                            alt,
                            r - d.radius,
                            g - d.radius,
                            rest - d.radius,
                            radial_wish,
                            tangential.length(),
                            state.surface_vr,
                            state.controller.fly_speed_mult,
                            step,
                        );
                    }
                }
            }
            state.ship_world_pos = humanity_core::dev_travel::frame_lock_ship_pos(
                body_center,
                rot,
                anchor,
                cam_local,
            );
            // No view yaw co-rotation in surface mode: the tangent
            // basis is derived from surface_up (which co-rotates
            // with the surface) + world Y, so a FIXED yaw already
            // holds the same compass bearing on the ground.
            // Subtracting a delta here would spin the world under
            // a standing player.
        } else if in_blend_band {
            // ── Ascent transition (100-1000 km, v0.872) ──
            // Position is inertial (no co-rotation - the ISS
            // regime begins), but the camera's up-vector EASES
            // from radial to world-Y across the band instead of
            // snapping the moment co-rotation ends (operator:
            // "my camera resets... I'm still very close to
            // Earth"). set_surface_up preserves the look
            // direction each frame, so the handoff is seamless
            // in both directions.
            state.surface_walk_band = false;
            let t = ((alt - co_rotate_max_alt)
                / (inertial_blend_max_alt - co_rotate_max_alt))
                .clamp(0.0, 1.0);
            let s = (t * t * (3.0 - 2.0 * t)) as f32; // smoothstep
            let up = (radial * (1.0 - s) + glam::Vec3::Y * s).normalize();
            // Look co-rotation through the blend band (v0.994):
            // position rides the spin with the `keep` curve
            // below - the aim uses the SAME curve, so the view
            // stays ground-stable low in the band and hands
            // over to the fixed-starfield ISS regime at the top.
            if state.camera.surface_mode {
                let d_rot = rot * state.frame_lock_last_rot.inverse();
                let d_ang = crate::engine::planet_build::turn_angle(d_rot);
                if d_ang > 1e-12 && d_ang < 0.01 {
                    let shed = ((t - 0.8) / 0.2).clamp(0.0, 1.0);
                    let keep = 1.0 - shed * shed * (3.0 - 2.0 * shed);
                    state.camera.co_rotate_look(
                        crate::engine::planet_build::turn_part(d_rot, keep).as_quat(),
                    );
                }
            }
            state.camera.set_surface_up(up);
            state.gui_state.surface_altitude_m = Some(alt.max(0.0) as f32);
            state.gui_state.surface_speed_mult =
                state.controller.fly_speed_mult as f32;
            // Translation is NOT owned here: the normal local
            // fly + FTL wheel integration handles movement (the
            // ownership flag below releases the FTL gate).
            state.surface_owns_translation = false;
            // ── FULL co-rotation through the band (v0.923,
            // operator momentum directive: "we're relative to
            // the Earth so our movement should act relative
            // to Earth" - supersedes the v0.890 partial
            // (1-s) ride). Lifting off no longer sheds the
            // planet's frame at all until the very top of
            // the band: the ride stays FULL through 80% of
            // it, then fades over the last 20% so entering
            // the inertial ISS regime is still seamless.
            // The up-vector ease above is untouched - only
            // POSITION inheritance changed. The 0.01 rad
            // guard skips stale anchors (e.g. the first
            // frame after a teleport wrote a fresh
            // last_spin far in the past).
            let d_ang = crate::engine::planet_build::turn_angle(rot * state.frame_lock_last_rot.inverse());
            let mut cam_world_now = cam_world;
            if d_ang > 1e-12 && d_ang < 0.01 {
                let rot_new = rot;
                let rot_old = state.frame_lock_last_rot;
                let shed = ((t - 0.8) / 0.2).clamp(0.0, 1.0);
                let keep = 1.0 - shed * shed * (3.0 - 2.0 * shed);
                let ride = (rot_new * state.frame_lock_anchor
                    - rot_old * state.frame_lock_anchor)
                    * keep;
                state.ship_world_pos += ride;
                cam_world_now += ride;
            }
            state.frame_lock_anchor = humanity_core::dev_travel::frame_lock_capture(
                body_center,
                rot,
                cam_world_now,
            );
            state.frame_lock_last_rot = rot;
        } else {
            // ── Inertial orbit (the ISS view) ── Beyond the
            // blend band we do NOT co-rotate: the position is
            // left where it is (ship_world_pos untouched) and
            // the view yaw is NOT wheeled, so the STARFIELD
            // stays fixed and the planet rotates naturally
            // beneath the camera. We only TRACK the anchor
            // (refresh it from the current position) so a later
            // descent re-locks onto the ground right here.
            state.surface_walk_band = false;
            state.camera.clear_surface();
            state.frame_lock_anchor =
                humanity_core::dev_travel::frame_lock_capture(body_center, rot, cam_world);
            state.frame_lock_last_rot = rot;
        }
    } else {
        state.surface_owns_translation = false;
        // Not locked to any body (e.g. after "Return home"):
        // ensure the surface FPS basis is released so the ship /
        // space camera uses world-Y up again.
        state.camera.clear_surface();
        // No body context: full fill (the orbital/ship look).
        if let Some(gpu) = state.gpu.as_mut() {
            gpu.renderer.fill_scale = 1.0;
        }
    }

}
