//! The sun's light and shadows (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs (the solar system block, now `frame_sky`) as it was: the sun's
//! direction into the hull, the key light (or build mode's fixed light), the light released above the
//! air, the sky's bodies and disc for the dome, the sun light and the fill light.

use crate::engine::frame_prelude::*;

/// Sets this frame's sun, sky-body and fill lights from where the sun, the moon and the drawn
/// bodies are.
pub(crate) fn run(state: &mut EngineState, sun_rel_earth_m: glam::DVec3, moon_rel_earth_m: glam::DVec3, body_draws: &[crate::renderer::key_lights::BodyDraw], disc_cands: &[crate::renderer::key_lights::DiscCandidate]) {
    humanity_core::frame_costs::span("cpu.span.sky_light");
    // HULL FRAME (v0.1225), site 2 of 3, and the one that actually
    // lights the deck. Note this cannot be replaced by "move the
    // station and let the direction follow": the sun is 1 AU away, so
    // a whole geosynchronous circle moves this vector by 0.03 degrees.
    // The change comes entirely from the hull TURNING under it.
    let sun_dir = humanity_ship::station::to_hull(
        state.station_ride,
        state.station_world_rot,
        (sun_rel_earth_m - state.ship_world_pos).normalize_or_zero(),
    )
    .normalize_or_zero();
    // GI master switch (v0.571): when OFF, zero the sun + fill so only LOCAL placed
    // lights illuminate -- the operator's "turn off global illumination and still
    // see" test. Default ON restores the normal sun (2.5) + the cool fill (0.6).
    let gi = state.gui_state.gi_enabled;
    // Construction-editor sun override (operator: the real astronomical sun is
    // tied to Earth's slow orbital drift + a ship position that never rotates, so
    // a bad real-world sun angle can leave a build permanently hard to see with no
    // way to fix it). Only applies while actively editing; the visible Sun disc
    // position (sun_world_pos above) is left alone -- only the LIGHT ray direction/
    // color used for illumination is swapped, trading disc/light consistency for
    // being able to actually see the build.
    let (light_dir, light_color) = if state.gui_state.construction_active
        && state.gui_state.construction_sun_override
    {
        let dir = humanity_systems::time::TimeSystem::sun_direction(
            state.gui_state.construction_sun_override_hour,
        );
        (dir, humanity_systems::time::TimeSystem::sun_color(
            state.gui_state.construction_sun_override_hour,
        ))
    } else {
        (
            Vec3::new(sun_dir.x as f32, sun_dir.y as f32, sun_dir.z as f32),
            crate::renderer::key_lights::SUN_RGB,
        )
    };
    // Sky-arc stage 3a (v0.945): keep the atmosphere LUTs
    // in sync with the frame-locked body. Cheap per frame:
    // the renderer no-ops unless the params changed.
    if let Some((ac, scale, sh, radius)) = state
        .frame_lock_body
        .as_deref()
        .and_then(|b| state.planet_defs.get(b))
        .and_then(|d| {
            d.atmosphere_color.map(|ac| {
                (ac, d.atmosphere_scale, d.scale_height_or_default(), d.radius)
            })
        })
    {
        let (rp, h) =
            crate::renderer::atmosphere::shell_packing(scale, sh, radius);
        gpu!(state).renderer.update_atmo_luts(
            crate::renderer::atmo_luts::TransLutParams {
                tint: [ac[0], ac[1], ac[2]],
                density_mul: ac[3],
                rp,
                h,
            },
        );
        // Stage 3b-2: this frame's sky-view inputs. The
        // shell radius is radius * (1 + 2*scale) - the
        // drawn shell-mesh expression shell_packing pairs
        // with. (The v0.915 tint block below used to
        // divide by radius*scale, pegging ITS cam_r at
        // 1.0 - fixed v0.1004 after the operator's
        // floodlit-ground-at-night screenshot traced to
        // exactly that peg.)
        let shell_r_m = radius * (1.0 + scale.max(0.005) as f64 * 2.0);
        let cam_r = ((state.frame_lock_anchor.length()
            / shell_r_m.max(1.0))
            as f32)
            .clamp(rp, 1.0);
        let up_world = crate::engine::planet_build::frame_body_rot(state)
            * state.frame_lock_anchor.normalize_or_zero();
        let mu_s = up_world.dot(sun_dir) as f32;
        gpu!(state).renderer.sky_view_uniform =
            Some(crate::renderer::sky_view::SkyViewUniform {
                tint_density: ac,
                geom: [cam_r, mu_s, rp, h],
            });
        // W1 (environment program): the water's
        // sky-mirror gate, the SAME retirement law the
        // atmosphere applies to this LUT toward orbit -
        // (1 - max(w_alt, w_far)). Uses the UNCLAMPED
        // camera ratio for the far band (FAR_R = 2.5
        // sits beyond the clamp above).
        let cam_ratio = (state.frame_lock_anchor.length()
            / shell_r_m.max(1.0))
            as f32;
        let ss = |e0: f32, e1: f32, x: f32| {
            let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let w_far = ss(
            crate::renderer::atmosphere::NEAR_R,
            crate::renderer::atmosphere::FAR_R,
            cam_ratio,
        );
        let w_alt = ss(rp, 1.0, cam_r);
        gpu!(state).renderer.water_lut_gate =
            (1.0 - w_far.max(w_alt)).clamp(0.0, 1.0);
    } else {
        gpu!(state).renderer.sky_view_uniform = None;
        gpu!(state).renderer.water_lut_gate = 0.0;
    }

    // Sun transmittance tint (v0.915, research roadmap
    // item 1): direct sunlight dims AND reddens through
    // the atmosphere - blue extinguishes first, so a low
    // sun paints terrain, water, foam, and clouds in
    // sunset tones, and daylight stops being a fixed
    // white. Only when standing at a planet with an
    // atmosphere; the construction-editor override keeps
    // its hand-picked color.
    //
    // ── THE CAMERA'S SUN LETS GO ABOVE THE AIR (2026-10-21) ──
    // The tint here and the day factor below are the sun AT THE
    // CAMERA, shut after sunset there. From low orbit over the night
    // side that shut the whole frame body, the day side past the
    // terminator and its lit limb included, though nothing near the
    // camera is left that needs the gate: the ground, sea, clouds and
    // air each have their own terminator. Above the frame lock's
    // co-rotation ceiling (the air's top, 100 km for Earth) both ease
    // to the plain Sun (key_lights::camera_sun_release, release_tests).
    let sun_release = crate::engine::frame_lock::above_air(state);
    let light_color = if !(state.gui_state.construction_active
        && state.gui_state.construction_sun_override)
    {
        let tint = state
            .frame_lock_body
            .as_deref()
            .and_then(|b| state.planet_defs.get(b))
            .and_then(|d| d.atmosphere_color.map(|ac| (d, ac)))
            .map(|(d, ac)| {
                let (rp, h) = crate::renderer::atmosphere::shell_packing(
                    d.atmosphere_scale,
                    d.scale_height_or_default(),
                    d.radius,
                );
                // v0.1004 (operator night screenshot, the
                // "floodlit ground pool at 05:30" bug): this
                // used radius * scale as the shell radius -
                // a tiny number that pegged cam_r at 1.0,
                // the TOP of the atmosphere. Transmittance
                // was then evaluated where below-horizon
                // sun rays still graze past the planet's
                // limb, so direct sun kept lighting
                // terrain until ~12 degrees below the
                // horizon: any east-tilted slope glowed
                // like a floodlight pool before dawn. The
                // REAL shell radius (same expression as
                // the sky-view block above) puts cam_r at
                // the surface, where a below-horizon ray
                // strikes the planet and od_to_space
                // returns infinite depth -> the sun now
                // dies AT the geometric horizon.
                let shell_r_m = d.radius
                    * (1.0 + d.atmosphere_scale.max(0.005) as f64 * 2.0);
                let cam_r = ((state.frame_lock_anchor.length()
                    / shell_r_m.max(1.0))
                    as f32)
                    .clamp(rp, 1.0);
                let up_world = crate::engine::planet_build::frame_body_rot(state) * state.frame_lock_anchor.normalize_or_zero();
                let mu = up_world.dot(sun_dir) as f32;
                // The Sun's disc glare (renderer/sun_disc.rs) takes the
                // air on the WHOLE line of sight from where the camera
                // really is: from orbit that is the limb's air, which the
                // lighting's radius, pinned to the shell top, cannot see.
                let cam_r_true = ((state.frame_lock_anchor.length() / shell_r_m.max(1.0)) as f32).max(rp);
                (
                    crate::renderer::atmosphere::sun_transmittance(cam_r, mu, rp, h, ac),
                    crate::renderer::atmosphere::sun_transmittance(cam_r_true, mu, rp, h, ac),
                )
            })
            .unwrap_or(([1.0, 1.0, 1.0], [1.0, 1.0, 1.0]));
        let (tint, sun_los) = tint;
        let tint = crate::renderer::key_lights::released_camera_sun(tint, 1.0, sun_release).0;
        // The Sun's glare takes the air on the whole line of sight.
        gpu!(state).renderer.sun_air_los = sun_los;
        [
            light_color[0] * tint[0],
            light_color[1] * tint[1],
            light_color[2] * tint[2],
        ]
    } else {
        light_color
    };
    // ── A BODY IN THE SKY TAKES ITS OWN SUN (2026-10-21) ──
    // The sun just built, and the day factor below, are the camera's:
    // shut after sunset, which is right for the ground and the near
    // scene and drew the Moon black at night. Every other body's
    // surface carries the Sun before any air, coloured by the air on
    // the camera's line of sight to it (renderer::key_lights).
    {
        let air = state
            .frame_lock_body
            .as_deref()
            .and_then(|b| state.planet_defs.get(b))
            .and_then(|d| d.atmosphere_color.map(|ac| (d, ac)))
            .map(|(d, ac)| {
                let (rp, h) = crate::renderer::atmosphere::shell_packing(d.atmosphere_scale, d.scale_height_or_default(), d.radius);
                // The same camera radius as the sun's tint above.
                let shell_r_m = d.radius * (1.0 + d.atmosphere_scale.max(0.005) as f64 * 2.0);
                let cam_r = ((state.frame_lock_anchor.length() / shell_r_m.max(1.0)) as f32).clamp(rp, 1.0);
                let up_w = crate::engine::planet_build::frame_body_rot(state) * state.frame_lock_anchor.normalize_or_zero();
                let up = humanity_ship::station::to_hull(state.station_ride, state.station_world_rot, up_w);
                crate::renderer::key_lights::CameraAir { cam_r, rp, h, color: ac, up }
            });
        for (m, p2) in crate::renderer::key_lights::sky_body_params(&body_draws, air.as_ref()) {
            gpu!(state).renderer.update_material_params2(m, p2);
        }
        // The body the camera's air lets show through by day (the Moon):
        // the dome takes its disc from here (30-atmosphere.wgsl).
        gpu!(state).renderer.sky_disc = crate::renderer::key_lights::sky_disc(&disc_cands);
    }
    if light_dir != Vec3::ZERO {
        gpu!(state).renderer.set_sun_light(
            light_dir,
            light_color,
            if gi { 2.5 } else { 0.0 },
        );
    }
    // Aerial perspective params (v0.916, research item
    // 2): exponential height haze fading distant land
    // and sea toward the sky's in-scatter color. Sigma
    // folds the Settings strength and the camera's
    // altitude density; the sky color rides the SAME
    // sunset transmittance as the sun light, so evening
    // haze warms while noon haze stays blue-grey.
    {
        // Celestial-pass day factor (v0.1083, BUG-057 #1):
        // the celestial pass stamps sun intensity as a
        // CONSTANT 2.5, so trees/props on the night side
        // were lit by a below-horizon sun at full strength
        // (terrain has its own per-fragment terminator
        // gate; nothing else in that pass did). Camera-
        // local day factor, same formula as the sky below;
        // off-planet stays 1.0 (the sun never sets in
        // space).
        gpu!(state).renderer.celestial_sun_day = 1.0;
        if state.frame_lock_body.is_some() {
            let up_w = crate::engine::planet_build::frame_body_rot(state)
                * state.frame_lock_anchor.normalize_or_zero();
            let mu = up_w.dot(sun_dir) as f32;
            // The ground's gate; a body in the sky ignores it
            // (its own sun, the block before set_sun_light).
            gpu!(state).renderer.celestial_sun_day = crate::renderer::key_lights::released_camera_sun(
                [1.0; 3],
                crate::renderer::key_lights::camera_day_factor(mu),
                sun_release,
            )
            .1;
        }
        humanity_terrain::planet_chunks::GRASS_DENSITY_BITS.store(
            state.gui_state.settings.grass_density.to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        humanity_terrain::grass::GRASS_FAR_BITS.store(
            state.gui_state.settings.grass_far_m.to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        humanity_terrain::planet_chunks::GRASS_DETAIL_BITS.store(
            state.gui_state.settings.grass_detail.to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        let s = state.gui_state.settings.aerial_strength.clamp(0.0, 2.0);
        let mut sigma = 0.0f32;
        let mut cap = 25_000.0f32;
        let mut sky = [0.0f32; 3];
        // The weather's share of the layer's optical depth (0 in clear
        // air): what hides the stars and the Moon behind the fog
        // (2026-10-21, key_lights::fog_share).
        let mut fog_share = 0.0f32;
        // Local radial up, published for EVERY frame-locked
        // draw as of v0.1104 (was set only inside the aerial
        // branch below, so a zero haze slider left it at a
        // world-fixed [0,1,0]). The megashader's sky_ambient
        // reads this same pad as the local up of every
        // non-planet surface.
        let up_w = crate::engine::planet_build::frame_body_rot(state)
            * state.frame_lock_anchor.normalize_or_zero();
        let mut up = [0.0f32, 1.0, 0.0];
        if state.station_ride {
            // HULL FRAME (v0.1225), site 3 of 3. Aboard, the render
            // frame IS the hull body frame and LVLH puts the hull
            // zenith on body +Y, so local up is exactly +Y. That is
            // also what the fallback below produces, but by
            // accident - the anchor happens to be zero. Stating it
            // means a future anchor change cannot silently tilt the
            // homestead's ambient.
            up = [0.0, 1.0, 0.0];
        } else if up_w.length_squared() > 0.5 {
            up = [up_w.x as f32, up_w.y as f32, up_w.z as f32];
        }
        if s > 0.001 {
            if let Some((d, _ac)) = state
                .frame_lock_body
                .as_deref()
                .and_then(|b| state.planet_defs.get(b))
                .and_then(|d| d.atmosphere_color.map(|ac| (d, ac)))
            {
                let h_m = d.scale_height_or_default().max(500.0);
                let alt =
                    (state.frame_lock_anchor.length() - d.radius) as f32;
                let dens = (-(alt.max(0.0)) / h_m).exp();
                // Base visibility ~45 km at sea level at
                // strength 1.
                sigma = s * 2.2e-5 * dens;
                cap = h_m * 3.0;
                let mu = up_w.dot(sun_dir) as f32;
                let day = ((mu + 0.02) / 0.27).clamp(0.0, 1.0);
                // Hue from the sunset-tinted sun light,
                // normalized so the tint shifts color
                // without double-dimming.
                let m = light_color[0]
                    .max(light_color[1])
                    .max(light_color[2])
                    .max(0.05);
                sky = [
                    0.58 * day * (light_color[0] / m),
                    0.70 * day * (light_color[1] / m),
                    0.86 * day * (light_color[2] / m),
                ];
            }
        }
        // ── WEATHER-DRIVEN FOG (v0.1059) ──
        // Operator, twice: "I didn't notice anything with
        // change with fog or sandstorm... the air/sky didn't
        // change at all."
        //
        // Correct, and the reason is simple: weather.visibility
        // was computed every tick and consumed by NOTHING. No
        // shader uniform ever read it. Meanwhile the engine
        // already has exactly the right term - the aerial
        // perspective haze, an exponential extinction toward a
        // sky colour, with the sigma poked from right here. So
        // fog does not need a new pass, a new uniform or a new
        // shader: it needs the weather to be allowed to raise
        // this sigma and tint its colour.
        //
        // Sigma for a target visibility V is ln(50)/V (the
        // Koschmieder 2% contrast threshold), so 100 m of fog
        // is 0.039 /m against the clear-air 2.2e-5 - about
        // 1800x - and the exponential does the rest.
        if let Some(w) = state.gui_state.weather.as_ref() {
            let cond = w.condition.as_str();
            // Visibility floor per condition, in metres, at
            // full intensity. Rain and snow cut visibility
            // hard but nothing like fog; sandstorm is the
            // worst and also the most coloured.
            let (vis_min_m, tint) = match cond {
                "Fog" => (60.0_f32, [0.70_f32, 0.74, 0.78]),
                "Sandstorm" => (90.0, [0.62, 0.46, 0.28]),
                "Snow" => (400.0, [0.80, 0.83, 0.88]),
                "Storm" => (900.0, [0.48, 0.52, 0.58]),
                "Rain" => (1500.0, [0.52, 0.56, 0.62]),
                _ => (0.0, [0.0, 0.0, 0.0]),
            };
            if vis_min_m > 0.0 && sigma > 0.0 {
                // Intensity interpolates between clear air and
                // that floor, in SIGMA space so the ramp is
                // perceptually even rather than crowding at
                // the dense end.
                // ── FOG IS A GROUND LAYER (v0.1063) ──
                // Operator: "bad weather causes the whole
                // Earth to go foggy" - and from 469 km the
                // entire planet really did render as a
                // uniform white ball.
                //
                // The bug: the clear-air sigma above is
                // multiplied by exp(-alt / scale_height),
                // so aerial haze correctly thins out with
                // altitude - but the weather fog REPLACED
                // that sigma with a fixed density carrying
                // no falloff at all. It stayed at full
                // ground strength into orbit, and the sky
                // shell (v0.1060) reads the same uniform,
                // so the planet's whole dome went with it.
                //
                // Real fog and dust live in the boundary
                // layer: radiation fog is 100-300 m deep,
                // and even a big dust storm tops out
                // around 1-2 km. So the weather term gets
                // its OWN scale height, far shorter than
                // the atmosphere's ~8 km, and above it the
                // air simply returns to clear.
                const FOG_LAYER_H_M: f32 = 700.0;
                // LIVE altitude (v0.1064). v0.1063 read
                // frame_lock_anchor, which tracks the camera
                // only in surface-WALK mode - in fly mode it
                // is the anchor captured at the last
                // teleport and never moves. So the fog layer
                // was evaluated at a stale height and the
                // operator saw it inverted: thin at the
                // waterline, thick on the way up, gone
                // higher still, all of it tracking where
                // they had teleported FROM. surface_altitude_m
                // is what the HUD reads and is recomputed
                // every frame in both modes.
                let fog_alt_m =
                    state.gui_state.surface_altitude_m.unwrap_or(0.0);
                let fog_alt_fade =
                    (-(fog_alt_m.max(0.0)) / FOG_LAYER_H_M).exp();
                let t = (w.intensity
                    * state.gui_state.settings.fog_density
                    * fog_alt_fade)
                    .clamp(0.0, 1.0);
                let sigma_fog = (50.0_f32).ln() / vis_min_m;
                let sigma_w = sigma * (1.0 - t) + sigma_fog * t;
                // The slant cap exists to keep a look-UP ray
                // from accumulating a whole atmosphere of
                // haze. Fog is a GROUND layer a few tens of
                // metres deep, so as it thickens the cap has
                // to come down with it or the zenith turns to
                // soup. 80 m of fog layer at full intensity.
                let cap_w = cap * (1.0 - t) + 80.0 * t;
                // Tint toward the fog colour, keeping some of
                // the real sky so sunset still reads through a
                // light mist.
                // No luminance floor (BUG-057 #4): the old
                // lum.max(0.25) resurrected daytime-bright
                // fog at midnight (measured 139/143/147
                // against a predicted 139/143/146 - exact).
                // Fog scatters the light that exists; at
                // night that is none. sky[] is already
                // day-scaled, so lum carries the gate.
                let lum = (sky[0] + sky[1] + sky[2]) / 3.0;
                let sky_w = [
                    sky[0] * (1.0 - t) + tint[0] * lum * t,
                    sky[1] * (1.0 - t) + tint[1] * lum * t,
                    sky[2] * (1.0 - t) + tint[2] * lum * t,
                ];
                fog_share = crate::renderer::key_lights::fog_share(sigma, cap, sigma_w, cap_w);
                sigma = sigma_w;
                cap = cap_w;
                sky = sky_w;
            }
        }
        gpu!(state).renderer.aerial_sigma = sigma;
        gpu!(state).renderer.aerial_fog_share = fog_share;
        gpu!(state).renderer.aerial_slant_cap = cap;
        gpu!(state).renderer.aerial_sky = sky;
        gpu!(state).renderer.aerial_up = up;
    }
    // The fill is otherwise set once at init; re-assert it each frame so the GI
    // toggle is authoritative (restores the default when GI is back on).
    //
    // THE DAYLIGHT FILL IS DELETED (v0.1104). It was a
    // WORLD-FIXED (-0.5, 0.3, -0.3) blue at intensity 0.6 -
    // ~12.4% of the sun and the closest thing this engine had
    // to indirect light - so on a spinning planet it sank
    // below the local horizon over a day, and being an
    // evaluate_light call it also painted a meaningless
    // specular lobe. Real sky irradiance now comes per
    // fragment from sky_ambient (see renderer::sky_ambient);
    // keeping both would double-count ~27% onto every
    // up-facing surface. Moonlight below is untouched - that
    // one is a real key light tracking the real Moon.
    //
    // MOONLIGHT (v0.1052). The terrain terminator gate that
    // ships alongside this correctly stops a below-horizon
    // sun from lighting the ground - and that leaves a
    // moonless night at ambient only, i.e. unusably black
    // (measured in the rig: the desert-night vantage went to
    // near pure black once the streak was gone). Real nights
    // are not black because the Moon is a key light, so aim
    // the FILL at the actual Moon after sunset. Reusing the
    // fill slot means no new light plumbing and no extra
    // shader term.
    //
    // Physically the Moon is ~1e-6 of daylight, which no
    // display can show; like every game we lift it to a few
    // percent so a night reads as night rather than as a
    // fault. Phase is real though: full moon is bright, new
    // moon is not, and it tracks the true Moon position, so
    // moonlight comes from where the Moon actually is.
    //
    // The math moved to renderer::key_lights in v0.1104 so
    // it could be unit-tested (the anchor is PLANET-LOCAL, so
    // world up needs the spin applied exactly as the
    // aerial-perspective block above does - without that,
    // both the sunset and the moonrise test are meaningless
    // and night never triggers at all).
    let fill = crate::renderer::key_lights::moon_fill(
        (crate::engine::planet_build::frame_body_rot(state)
            * state.frame_lock_anchor.normalize_or_zero())
        .normalize_or_zero(),
        (moon_rel_earth_m - state.ship_world_pos).normalize_or_zero(),
        glam::DVec3::new(sun_dir.x as f64, sun_dir.y as f64, sun_dir.z as f64),
    );
    gpu!(state).renderer.set_fill_light(
        fill.dir,
        fill.rgb,
        if gi { fill.intensity } else { 0.0 },
    );
}
