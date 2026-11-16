//! The world's render passes (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: on a frame no page covers, the swap-chain image is
//! acquired and the stars, sky, scene, water, shadows and the rest of the 3D passes are recorded into
//! it; on a frame a page covers, the surface is only cleared.

use crate::engine::frame_prelude::*;

/// Acquires this frame's surface and records the world's passes into it (or clears it when a page
/// covers the world).
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
    let page_active = f.page_active;
    humanity_core::frame_costs::span("cpu.span.world_passes");
    let scene_result = if page_active {
        // UI-only frame: skip 3D render, clear to dark background
        gpu!(state).renderer.acquire_surface_cleared(wgpu::Color {
            r: 0.07,
            g: 0.07,
            b: 0.086,
            a: 1.0,
        })
    } else {
        // In-game: render stars first, then scene objects on top
        match gpu!(state).renderer.acquire_surface() {
            Ok((output, swap_view)) => {
                // HDR scene target (renderer/scene_target.rs): every pass below draws into the
                // scene target, and `present_scene` copies it to the swapchain before egui.
                let view = gpu!(state).renderer.scene_view_for(&swap_view);
                // DAYLIGHT GATE (v0.1059): inside an
                // atmosphere with the sun well up, the
                // stars are washed out by the sky drawn
                // over them, so the star draw is skipped.
                // ONE function decides it for this pass and
                // for every off-screen view (the camera
                // screens, the hi-res screenshot), so a
                // camera can never show stars by day while
                // the window shows none; the rule itself
                // is documented on `sky_daylight`.
                let daylight = crate::engine::ipc::sky_daylight(state);
                // The sky's turn and twilight fades: the same
                // rule as every off-screen view (ipc, BUG-139).
                let (sky_rot, sky_fades) =
                    (crate::engine::ipc::sky_rotation(state), crate::engine::ipc::star_fades(state));
                // Pass 1: Stars (clear to black + draw star points)
                if let Some(ref mut star_r) = state.star_renderer {
                    // `cpu.stars`: the submission twin of the
                    // `gpu.stars` pass below, so the no-timestamp
                    // fallback (`gpu.x` reads `cpu.x`) has a number
                    // for the Star field row (it read 0 before
                    // 2026-09-18). Same pair as `SceneView::Main`'s
                    // `sky_ids`, which the screenshot path uses.
                    let _cost = humanity_core::frame_costs::stage("cpu.stars");
                    // Sky settings (v0.786): constellation toggle +
                    // theme color. Rebuilds the line buffer only
                    // when the color actually changed.
                    let cc = state.theme.constellation_line();
                    star_r.set_constellation_style(
                        &gpu!(state).renderer.device,
                        state.gui_state.settings.sky_constellations,
                        [
                            cc.r() as f32 / 255.0,
                            cc.g() as f32 / 255.0,
                            cc.b() as f32 / 255.0,
                            cc.a() as f32 / 255.0,
                        ],
                        humanity_core::reuse::enabled(),
                    );
                    // Milky Way glow (2026-07-10): toggle +
                    // intensity, applied live (the uniform
                    // write is skipped while unchanged).
                    star_r.set_glow_style(
                        &gpu!(state).renderer.queue,
                        state.gui_state.settings.sky_milkyway_glow,
                        state.gui_state.settings.sky_milkyway_intensity,
                    );
                    // Star halos (2026-07-11): a plain
                    // visibility flag, no GPU state to sync
                    // (per-star amplitude/size are baked in
                    // the halo vertex buffer at world load).
                    star_r.show_star_halos =
                        state.gui_state.settings.sky_star_halos;
                    star_r.update_camera(&gpu!(state).renderer.queue, &state.camera, sky_rot, sky_fades);
                    let mut encoder = gpu!(state).renderer.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("Star Encoder") },
                    );
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Star Pass"),
                            timestamp_writes: gpu!(state).renderer.pass_timer("gpu.stars"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });
                        // `daylight` was decided above, by the
                        // same function the off-screen views use.
                        star_r.render_pass(&mut pass, daylight);
                    }
                    gpu!(state).renderer.queue.submit(std::iter::once(encoder.finish()));
                }

                // Update point lights (up to 8 nearest to camera)
                {
                    let cam_pos = state.camera.position;
                    let mut lights = state.room_lights.clone();
                    // EMISSIVE surfaces emit light (v0.576): an ENERGY door is a glowing
                    // field, so add a local point light at it (green unlocked / red
                    // locked) -- regardless of the GI toggle, since it's a local source.
                    // (Generalizing to any emissive material -- TVs, lava, emissive walls
                    // -- is a follow-up; the energy door is the case the operator flagged.)
                    let dl = state.door_locks.clone();
                    for (i, (p, _open)) in state.door_panels.iter().enumerate() {
                        if p.style != "energy" {
                            continue;
                        }
                        let locked = door_locked_now(p, dl.get(i));
                        let color = if locked { [1.0, 0.25, 0.28] } else { [0.30, 1.0, 0.45] };
                        let c = p.center;
                        lights.push(crate::renderer::light::RoomLight::point(
                            Vec3::new(c.x, c.y + p.size.y * 0.5, c.z), color, 5.0, 4.5,
                        ));
                    }
                    // v0.911 (operator: "the full string light...
                    // refuses to render the light from all parts
                    // except the ones a meter from me"): the old
                    // nearest-8 cap predates the v0.782 growable
                    // lights STORAGE buffer - the shader loops
                    // num_lights with an early range rejection
                    // that makes far lights nearly free, so a
                    // string's every bulb can light its own
                    // surroundings. Sort by how far outside each
                    // light's influence sphere the camera sits
                    // (distance minus range - a big far light
                    // outranks a small near one) and keep a
                    // generous 64.
                    lights.sort_by(|a, b| {
                        let da = ((a.pos - cam_pos).length() - a.range).max(0.0);
                        let db = ((b.pos - cam_pos).length() - b.range).max(0.0);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    // 256 (v0.912, operator: "could we have
                    // 1,000 or more?"): the storage buffer
                    // grows unbounded and out-of-range lights
                    // early-out per pixel, so the ceiling is
                    // the per-pixel loop, ~10 ALU per light.
                    // 256 is safe on midrange GPUs; 1000+
                    // needs screen-tile light clustering -
                    // queued as its own arc in PRIORITIES.
                    let tiled = state.gui_state.settings.lights_tiled;
                    lights.truncate(if tiled { 2048 } else { 256 });
                    // Home lights ride the station (v0.881).
                    // BUG-061 (v0.1095): LINE lights pack their
                    // SECOND ENDPOINT in the `dir` field (the
                    // cos_outer <= -1.5 sentinel). Offsetting
                    // only pos left endpoint B at the render
                    // origin - the player - so every strip
                    // light became a segment from orbit down
                    // to the player's feet, and the shader's
                    // closest-point math pooled faint light
                    // spots on the ground that TRACKED the
                    // player through jumps (operator report;
                    // their player-space-vs-world-space
                    // hypothesis was exactly right). It also
                    // fed planet-sized bounding spheres into
                    // the light tiler, flooding all 144 tiles
                    // and silently evicting real lights at
                    // TILE_CAP.
                    for l in lights.iter_mut() {
                        l.pos += state.station_off;
                        if l.cos_outer <= -1.5 {
                            l.dir += state.station_off;
                        }
                    }
                    // Dev test lights (showcase lights:N): regenerated
                    // around the CURRENT camera position every frame,
                    // because render space rebases with the floating
                    // origin - stored positions went stale in seconds.
                    if state.debug_test_light_count > 0 {
                        let n = state.debug_test_light_count;
                        // The RENDER eye (effective_position), not
                        // .position: third-person/orbit modes offset
                        // the eye, and lights must hug what the view
                        // actually renders from.
                        let cam = state.camera.effective_position();
                        // Grid in the RENDERED-VIEW frame (v0.976): the
                        // v0.954 "camera-local" fix used camera.forward()/
                        // right(), which are the raw Y-up yaw/pitch
                        // formulas - in frame-locked SURFACE mode the real
                        // view runs through the surface basis, so the grid
                        // spawned ~20 m ABOVE the view line at any aim
                        // (the [lights-diag] smoking gun: last_light y
                        // +19..23 m over the eye no matter the pitch,
                        // 6 m radius never touching terrain). Deriving the
                        // basis from the view matrix's inverse rows is
                        // mode-proof: what the view renders is what the
                        // grid hugs, on ships AND planet surfaces.
                        let inv_view = state.camera.view_matrix().inverse();
                        let right = (inv_view * glam::Vec4::new(1.0, 0.0, 0.0, 0.0)).truncate().normalize_or_zero();
                        let up_l = (inv_view * glam::Vec4::new(0.0, 1.0, 0.0, 0.0)).truncate().normalize_or_zero();
                        let fwd = (inv_view * glam::Vec4::new(0.0, 0.0, -1.0, 0.0)).truncate().normalize_or_zero();
                        let side = (n as f32).cbrt().ceil() as i32;
                        let mut count = 0usize;
                        'lgrid: for y in 0..side {
                            for z in 0..side {
                                for x in 0..side {
                                    if count >= n { break 'lgrid; }
                                    let p = cam
                                        + right * ((x - side / 2) as f32 * 4.0)
                                        + up_l * ((y % 3) as f32 * 2.5 - 1.0)
                                        + fwd * ((z - side / 2) as f32 * 4.0 + 20.0);
                                    let hue = (count as f32 * 0.618_034) % 1.0;
                                    let (r, g, b) = hsv_to_rgb(hue, 0.85, 1.0);
                                    lights.push(crate::renderer::light::RoomLight::point(p, [r, g, b], state.debug_test_light_intensity, 6.0));
                                    count += 1;
                                }
                            }
                        }
                    }
                    // Clustering diagnostics (throttled): the
                    // lit-then-dark dev-light mystery needs a
                    // deterministic answer - what actually
                    // reaches the GPU and where.
                    if state.debug_test_light_count > 0 {
                        static DIAG: std::sync::atomic::AtomicU32 =
                            std::sync::atomic::AtomicU32::new(0);
                        let n = DIAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if n % 120 == 0 {
                            let cam = state.camera.position;
                            let eye = state.camera.effective_position();
                            let last = lights.last().map(|l| l.pos).unwrap_or(Vec3::ZERO);
                            log::info!(
                                "[lights-diag] assembled={} cam=({:.1},{:.1},{:.1}) eye=({:.1},{:.1},{:.1}) last_light=({:.1},{:.1},{:.1}) tiled={} station_off=({:.1},{:.1},{:.1})",
                                lights.len(), cam.x, cam.y, cam.z, eye.x, eye.y, eye.z,
                                last.x, last.y, last.z, tiled,
                                state.station_off.x, state.station_off.y, state.station_off.z
                            );
                        }
                    }
                    gpu!(state).renderer.set_point_lights(&lights);
                    let vp = state.camera.view_projection_matrix();
                    let screen = gpu!(state).renderer.surface_size();
                    gpu!(state).renderer.update_light_tiles(&lights, &vp, state.camera.effective_position(), screen, tiled);
                }

                // Sun disc/halo cloud occlusion (v0.1243, the
                // operator's "sun bleeding out the clouds as if
                // they don't exist"): the type-17 disc + corona
                // are drawn in the transparent pass BEFORE the
                // cloud composite, but they ACES-saturate to
                // near-white, so a bank at alpha 0.5-0.8 still
                // passed 20-50% white - greenscreen sun. Scale
                // their intensity by the transmittance along
                // the camera-to-sun ray through the drawn deck
                // (exp(-4a): a real bank kills the disc), from
                // the SAME sun_cloud_alpha the god-ray dim
                // uses, so shafts and glare always agree.
                //
                // THE SUN AS A BODY (2026-10-21, renderer/sun_disc.rs)
                // replaced both of those intensities. The disc is
                // light at its real radiance now: the cloud
                // composite and the air in front of it dim it per
                // pixel, and the planet hides it by depth, so its
                // material carries NOTHING from the player's state:
                // the v0.1541 disc was multiplied by the player's
                // horizon and cloud, so any view whose camera was not
                // where those were computed (the Maps world layer, a
                // camera screen, a lock left on a ground point) could
                // lose it while the god rays, from geometry, still drew
                // their glow. The cloud transmittance and the
                // planet's limb go to its GLARE, which is added in
                // the present over everything and so cannot see
                // them: the cloud from the same sun_cloud_alpha
                // the god rays dim by, the limb (for the part of
                // the disc off the screen; the rest is measured by
                // depth taps) from frame_lock::sun_disc_clear.
                {
                    let sca =
                        crate::engine::frame_lock::sun_cloud_alpha(state);
                    let clear = crate::engine::frame_lock::sun_disc_clear(state);
                    let los = gpu!(state).renderer.sun_air_los;
                    // The frame body's air, so a view above it (the Maps
                    // world layer) does not take the player's air and cloud.
                    let fb = gpu!(state).renderer.frame_body_sphere;
                    let air_top = state
                        .frame_lock_body
                        .as_deref()
                        .and_then(|b| state.planet_defs.get(b))
                        .filter(|_| fb[3] > 0.0)
                        .map(|d| (Vec3::new(fb[0], fb[1], fb[2]), (d.radius * (1.0 + d.atmosphere_scale.max(0.005) as f64 * 2.0)) as f32));
                    if let Some(f) = gpu!(state).renderer.sun_frame.as_mut() {
                        f.air = los;
                        f.cloud = (-4.0 * sca).exp();
                        f.off_screen = clear;
                        f.air_top = air_top;
                    }
                }
                // Pass 1.5: celestial bodies (planet + Sun + solar bodies)
                // with a HUGE far plane, behind the interior, lit by the
                // REAL Sun direction. (v0.450 bodies, v0.451 lighting)
                // Viewer-relative like the scene light above (v0.791.x):
                // sun_world_pos is Earth-relative, so after a dev-travel
                // jump the un-shifted vector would light the destination
                // planet's wrong hemisphere. At home the correction is
                // ~3e-7 rad (GEO vs 1 AU) -- invisible.
                let sun_dir_f = crate::engine::ipc::celestial_sun_dir(state); // render frame: the hull's axes aboard (BUG-210)
                // The elapsed-seconds arg is the cloud-deck clock
                // (shader type 15 animation); app-start-relative so
                // f32 stays precise for days of uptime.
                gpu!(state).renderer.detail_distance =
                    state.gui_state.settings.terrain_detail_distance.clamp(0.5, 3.0);
                // Vegetation LOD (v0.923): the tree-card far
                // cutoff slider applies live.
                gpu!(state).renderer.tree_card_far_m =
                    humanity_terrain::far_trees::effective_card_far_m(
                        state.gui_state.settings.veg_tree_card_m
                            .clamp(100.0, crate::config::TREE_CARD_MAX_M),
                        humanity_terrain::far_trees::measured_card_reach_m(),
                    );
                // v0.907: the lighting-pass knobs from
                // Settings > Planets apply every frame.
                gpu!(state).renderer.sun_shadows =
                    state.gui_state.settings.sun_shadows;
                // Room light shadows (2026-11-10, renderer/local_shadows.rs): the level id.
                if gpu!(state).renderer.local_shadows.level != state.gui_state.settings.local_shadows {
                    gpu!(state).renderer.local_shadows.level = state.gui_state.settings.local_shadows.clone();
                }
                // Frosted plastic (2026-11-13, renderer/film_blur.rs): Blurred or Simple; the
                // colour pyramid is made, remade for a new window size, or dropped to match.
                gpu!(state).renderer.film_blur.on = state.gui_state.settings.film_blur;
                gpu!(state).renderer.ensure_film_pyramid();
                gpu!(state).renderer.shadow_strength =
                    state.gui_state.settings.shadow_strength.clamp(0.0, 1.0);
                gpu!(state).renderer.godray_intensity =
                    state.gui_state.settings.godray_intensity.clamp(0.0, 1.5);
                // Underwater extinction (v0.1054): only
                // while submerged, and only as far as the
                // Settings clarity slider allows. 1 - clarity so
                // the slider reads as "how far can I see".
                // Wave shell materials cast shadows (v0.1057).
                // Only the WAVE shell, never the backstop.
                gpu!(state).renderer.water_caster_mats = state
                    .planet_water_materials
                    .iter()
                    .filter(|(k, _)| !k.contains("backstop"))
                    .map(|(_, v)| *v)
                    .collect();
                // v0.1061: no longer gated on the binary
                // camera-is-submerged flag. Operator: "I don't
                // really see the water passing in front of me
                // until the camera suddenly just goes blue."
                // That flip WAS this gate - one point test on
                // the camera switching the whole screen. The
                // shader now integrates extinction over the
                // submerged PART of each ray, so a camera at
                // the waterline gets air above the surface and
                // water below it in the same frame, which is
                // the over-under look. Always on; the sea
                // sphere decides per pixel.
                gpu!(state).renderer.underwater_ext =
                    1.0 - state.gui_state.settings.water_clarity.clamp(0.0, 1.0);
                gpu!(state).renderer.ssao_strength =
                    state.gui_state.settings.ssao_strength.clamp(0.0, 1.5);
                // Anti-aliasing and its sharpening (renderer/taa.rs).
                if gpu!(state).renderer.taa.mode != state.gui_state.settings.anti_aliasing {
                    gpu!(state).renderer.taa.mode = state.gui_state.settings.anti_aliasing.clone();
                }
                gpu!(state).renderer.taa.sharpen = state.gui_state.settings.sharpening.clamp(0.0, 1.0);
                // (tree_card_hide_m is owned by the near-tree
                // draw loop since v0.914 - it tracks the
                // radius models actually cover each frame.)
                // Submerged blend order (v0.1018, operator:
                // "when I go under water all that happens is
                // the sky gets a blue tint"): the transparent
                // list is ordered back-to-front FROM ABOVE
                // (water first, then atmo/clouds over it).
                // From underwater that is exactly backwards -
                // the atmosphere and cloud shells blended
                // over the sea surface, washing it out with
                // sky. When submerged, water objects move to
                // the END (nearest), so the surface composites
                // over the sky/clouds seen through it.
                // v0.1053 (operator: "when the waves are really
                // tall they get hidden by the sky... the horizon
                // is perfectly flat instead of seeing the peaks
                // of the waves close to me going above the
                // horizon"): the SAME reordering is needed
                // whenever the camera is inside the atmosphere,
                // not only when submerged. The transparent
                // pipeline does not write depth - the water
                // shell's Fresnel alpha has to blend over the
                // seafloor - so the atmosphere and cloud shells
                // drawn AFTER water have no water depth to test
                // against and simply composite over it. Their
                // limb term is DENSEST at grazing angles, which
                // is precisely where a near wave crest rises
                // into the sky band, so tall crests got washed
                // to sky colour and the horizon read as a
                // ruler-straight line. From any viewpoint inside
                // the atmosphere the sea surface is NEARER than
                // the sky, so water belongs last. Outside the
                // atmosphere the approved space look (blue limb
                // hazing the deck) is untouched.
                // Gate on ALTITUDE, not camera.surface_mode:
                // surface_mode is false in dev-fly/travel mode
                // (measured - the first attempt at this fix
                // never fired in the probe rig for exactly that
                // reason), and the ordering question is purely
                // "is the camera inside the air". 120 km clears
                // every atmosphere shell this engine builds.
                let near_planet_air = state
                    .frame_lock_body
                    .as_deref()
                    .and_then(|b| state.planet_defs.get(b))
                    .map(|d| {
                        (state.frame_lock_anchor.length() - d.radius) < 120_000.0
                    })
                    .unwrap_or(false);
                let water_over_sky =
                    state.gui_state.underwater || near_planet_air;
                // Same gate drives the depth-writing water
                // pipeline (v0.1060): only correct when water
                // is last in the list.
                gpu!(state).renderer.water_depth_write = near_planet_air;
                // The transparent celestial list is drawn
                // through a pipeline picked per object by
                // MATERIAL CLASS (increment P2 of the
                // frame-cost arc, renderer/pipeline.rs
                // `shader_class`), and grouping the list
                // keeps the pipeline switches to a handful
                // per frame instead of one per object. The
                // grouping key is the planet LAYER BAND
                // (renderer/celestial_order.rs: types 13 to
                // 16, the dome or atmosphere, the cloud deck,
                // the ocean and its backstop), NOT the class:
                // the type-13 fallback dome is General class
                // and a class key lifted it in front of the
                // sea and the clouds whenever "Scattering
                // atmosphere" was off (the P2 review's
                // finding). A STABLE sort on (is a layer, is
                // water when water goes last) keeps every
                // ordering rule above: the cloud/atmosphere
                // order the approach-vanish fix depends on
                // is preserved within the layer run (equal
                // keys are never reordered), water still
                // sinks to the very end whenever the camera
                // is inside the air or underwater, and the
                // sun's blended core and halo, which the body
                // order already puts first, stay first.
                let water_mats: std::collections::HashSet<usize> =
                    if water_over_sky {
                        state.planet_water_materials.values().copied().collect()
                    } else {
                        std::collections::HashSet::new()
                    };
                // Built windows' glass sorts after everything (2026-09-28).
                let glass_mat = state.structure_mats.map(|m| m[5]);
                f.celestial_transparent.sort_by_key(|o| {
                    crate::renderer::celestial_order::key_for(
                        &gpu!(state).renderer,
                        o.material,
                        water_mats.contains(&o.material),
                        glass_mat == Some(o.material),
                    )
                });
                // Pass 0.85: TEMPORAL ANTI-ALIASING's frame start (renderer/taa.rs): this
                // frame's jitter, last frame's matrices in this frame's render space, and
                // whether the history survives. The scene list is the home moved by the station
                // offset (applied to it just below, Pass 2), so that offset is the scene's origin.
                gpu!(state).renderer.taa_begin_frame(&state.camera, state.station_off);
                // Pass 0.9: THE SHADOW ATLAS (2026-11-10, renderer/local_shadows.rs): the room
                // lights' shadows and the near sun view, drawn from the scene's casters (the home's
                // list rides the station) and the celestial build pieces, BEFORE the celestial pass,
                // so the terrain under a person and everything in the home sample this frame's maps.
                {
                    use crate::renderer::local_shadows::CasterList;
                    // Its own span (2026-11-13): the shadow atlas's processor time measured on
                    // this thread alone, which a busy machine does not inflate as it does the
                    // wall-clock cpu.shadow_* stages.
                    humanity_core::frame_costs::span("cpu.span.shadow_atlas");
                    let so = state.station_off;
                    gpu!(state).renderer.render_shadow_atlas(
                        &state.camera,
                        &[
                            (&f.all_objects, so, CasterList::Scene),
                            // The see-through list: its glass and frosted film cast PARTIAL
                            // shadows (renderer/shadow_transmit.rs, 2026-11-13); the rest of it
                            // casts nothing.
                            (&f.transparent_objects, so, CasterList::SeeThrough),
                            (&f.planet_ghost, Vec3::ZERO, CasterList::Scene),
                            (&f.people_render_space, Vec3::ZERO, CasterList::Scene),
                            (&f.celestial_objects, Vec3::ZERO, CasterList::CelestialSurfaces),
                        ],
                    );
                    humanity_core::frame_costs::span("cpu.span.world_passes");
                }
                // The 5th argument is this pass's ANIMATION CLOCK: live wall time unless the rig
                // pinned it (showcase_request {"anim_clock":"300"}). The pass stamps it into
                // sun_color.w on BOTH the colour and the shadow camera buffers, and the vertex
                // wind branch, the ocean wave phase and the cloud advection are all sines of it.
                // Pinning it is the only way two boots of one exe produce comparable forest
                // pixels: the sway's amplitude floor is wind-independent, so the wind pin alone
                // cannot freeze a canopy. tests/rig_pin_lint.rs requires the pin AT every call
                // site of this function, which is why it is written out here rather than hoisted.
                let (cloud_ground, ground_at, ocean_at) = (cloud_ground_params(state), ground_anchor(state), ocean_anchor256(state));
                gpu!(state).renderer.render_celestial_onto(&state.camera, &f.celestial_objects, &f.celestial_transparent, sun_dir_f, state.anim_clock_pin.unwrap_or_else(|| state.start_time.elapsed().as_secs_f32()), cloud_ground, ground_at, ocean_at, &view);
                // Pass 1.55: how much of the Sun's disc this view can
                // see, from the celestial depth, before the orbit lines
                // write theirs (renderer/sun_glare.rs, step 1).
                gpu!(state).renderer.render_sun_vis_onto(&state.camera);
                // Pass 1.6: orbit rings at celestial scale — between the
                // bodies and the interior so a ring behind a planet is
                // occluded by that body, and walls then draw over the
                // rings. AU-scale, so the gameplay far would clip them.
                // (v0.451; empty in the showroom — build is !showroom-gated.)
                gpu!(state).renderer.draw_celestial_lines_onto(&state.camera, &f.orbit_lines, &view);
                // Pass 1.7: crepuscular god rays (v0.895) —
                // marches the celestial depth (terrain +
                // bodies silhouettes) toward the sun BEFORE
                // the scene pass clears depth. Additive;
                // self-gates when the sun is off-camera.
                let godray = godray_scale(state);
                gpu!(state).renderer.render_godrays_onto(&state.camera, sun_dir_f, &view, godray);
                // Pass 1.8: SSAO contact shade (v0.901) -
                // same slot, same depth, multiplies creases
                // and tree bases darker before the interior
                // pass draws over its own regions.
                gpu!(state).renderer.render_ssao_onto(&state.camera, &view);
                // Pass 2: Scene objects (LoadOp::Load preserves stars + bodies)
                // -- Orbital home offset (v0.881) --
                // The ENTIRE scene pass is the home frame's
                // contents (floors, machines, crops, NPCs,
                // vehicles, gizmos), so ONE translation puts
                // it all at the station's orbital position.
                // Riding aboard the offset is ~zero; from
                // Earth's surface the station is a moving
                // ~4 px glint 400 km up - the ISS look.
                let so = state.station_off;
                if so.length_squared() > 1.0e-6 {
                    for o in f.all_objects.iter_mut() {
                        o.position += so;
                    }
                    for o in f.transparent_objects.iter_mut() {
                        o.position += so;
                    }
                    for v in f.ring_lines.iter_mut() {
                        v.position[0] += so.x;
                        v.position[1] += so.y;
                        v.position[2] += so.z;
                    }
                    // BUG-061 companion: overlay gizmos were
                    // the one list this loop missed - editor
                    // handles drew at the render origin (the
                    // player) instead of at the station.
                    for o in f.overlay_objects.iter_mut() {
                        o.position += so;
                    }
                }
                // The piece in hand on a planet: already in render space (engine/planet_build.rs).
                f.all_objects.append(&mut f.planet_ghost);
                f.all_objects.append(&mut f.people_render_space);
                // `SceneView::Main`: these four are the live frame's
                // `gpu.scene` / `gpu.transparent` / `gpu.overlay` /
                // `gpu.lines`; a camera screen's re-render of the same
                // lists is keyed `gpu.screen_*` (`render_view_onto`).
                let main_view = crate::renderer::gpu_costs::SceneView::Main;
                crate::engine::room_gi::before_scene(state); // room GI probe update (engine/room_gi.rs)
                crate::engine::furniture_detail::before_scene(state); // a changed Furniture detail or model pack rebuilds the bodies
                gpu!(state).renderer.render_scene_onto(&state.camera, &f.all_objects, &view, main_view);
                // Pass 2.5: transparent surfaces (glass windows) blended over
                // the opaque scene so you can see through them. (v0.456)
                gpu!(state).renderer.render_transparent_onto(&state.camera, &f.transparent_objects, &view, main_view);
                // Pass 2.55: THE TAA RESOLVE (renderer/taa.rs): blend this frame into the
                // history and sharpen it back into the scene target. Everything after it (the
                // overlay, the lines, the particles) writes no motion and draws unjittered.
                gpu!(state).renderer.taa_resolve();
                // Pass 2.6: editor gizmos on top (depth off), visible through walls. (v0.560)
                gpu!(state).renderer.render_overlay_onto(&state.camera, &f.overlay_objects, &view, main_view);
                // Pass 2.7: door auto-open rings as constant-width lines (v0.565).
                gpu!(state).renderer.draw_lines_onto(&state.camera, &f.ring_lines, &view, main_view);
                // Pass 2.8: particle billboards (v0.966) -
                // drifting leaves, space dust - blended over
                // everything, depth-tested, no depth write.
                {
                    let (pa, pad) = state.particle_system.collect_split();
                    gpu!(state).renderer.draw_particles_onto(&state.camera, &pa, &pad, &view);
                    // GPU-simulated precipitation draws after
                    // the CPU pool, same pipeline and blend
                    // (v0.1068). Nothing here knows where the
                    // vertex data came from - the compute pass
                    // wrote the identical layout.
                    if state.gui_state.settings.gpu_particles {
                        gpu!(state)
                            .renderer
                            .draw_gpu_particles_onto(&state.camera, &view);
                    }
                }
                gpu!(state).renderer.present_scene(&swap_view);
                Ok((output, swap_view))
            }
            Err(e) => Err(e),
        }
    };
    scene_result
}
