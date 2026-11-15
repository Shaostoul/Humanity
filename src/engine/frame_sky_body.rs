//! One body of the solar system (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs (the loop over the bodies in the solar system block, now `frame_sky`)
//! as it was: the body placed at its true position from the canonical Keplerian model, put into the
//! hull frame aboard the station, sized and lit, and drawn as a chunked planet (its patches selected,
//! built and drawn, with grass and trees near the ground) or as a sphere, with its rings.

use crate::engine::frame_prelude::*;

/// Places and draws one body `b` of the solar system for this frame; `sky` carries the values the
/// bodies share.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars, b: &'static humanity_core::cosmos::SolBody, sky: &mut crate::engine::frame_sky::SkyBodies) {
    let sim_t = sky.sim_t;
    let earth_helio_au = sky.earth_helio_au;
    let procedural = sky.procedural;
    let px_per_level = sky.px_per_level;
    let max_level = sky.max_level;
    let viewport_h = sky.viewport_h;
    let fov_deg = sky.fov_deg;
    let spin_f64 = sky.spin_f64;
    let dt = f.dt;
    let showroom = f.showroom;
    // The Sun + everything that directly orbits it
    // (planets, dwarfs, named belt bodies) + Earth +
    // our Moon. Other planets' moons are skipped to
    // keep the sky readable. Mirrors the Maps
    // left-list.
    let is_sun = b.body_type == "star";
    let direct_solar = b.parent.as_deref() == Some("sun");
    if !is_sun && !direct_solar && b.id != "moon" { return; }

    let helio_au =
        humanity_core::cosmos::body_world_position_3d_au(b, sim_t);
    let rel_earth_m =
        (helio_au - earth_helio_au) * humanity_core::cosmos::M_PER_AU;
    if is_sun { sky.sun_rel_earth_m = rel_earth_m; }
    // v0.1052: the Moon is the night KEY LIGHT. With the
    // terrain terminator gate landing in the same
    // release, a moonless night is genuinely black - so
    // the fill light aims at the real Moon after sunset.
    if b.id == "moon" { sky.moon_rel_earth_m = rel_earth_m; }
    // The Sun's light direction is needed even in the
    // showroom; the sky bodies themselves are hidden
    // there (a dark Earth limb would fill the view).
    if showroom { return; }
    // HULL FRAME (v0.1225), site 1 of 3. Aboard the station the
    // render frame is the hull's body frame, so every external
    // body has to be counter-rotated into it. This is the single
    // place a celestial position becomes a render position, so
    // Sun, Earth, Moon and planets are all covered here at once.
    let render_off = humanity_ship::station::to_hull(
        state.station_ride,
        state.station_world_rot,
        rel_earth_m - state.ship_world_pos,
    );
    // Planet model rotation this frame: THIS body's own turn
    // (Earth's spin from the game clock, every other body's from
    // its catalog on the clock that places it: until 2026-10-21 every
    // body took Earth's spin, so the Moon showed a face that went round
    // once a day) composed in f64 with the hull frame (v0.1225): a point
    // at planet-local p renders at render_off + R_hull * R_body * p. ONE
    // binding: the terrain, the near trees, the OSM buildings and the
    // pieces built on the planet all use it, and `rotation` (every
    // draw's orientation: the uniform sphere, the patches, the shells)
    // is the same narrowed to f32. Aboard it includes the hull frame: the
    // spin alone turned Earth's north pole toward the ship (2026-10-21,
    // `body_rot_tests` in engine/planet_build.rs).
    let rot_d = crate::engine::planet_build::body_render_rot(state.station_ride, state.station_world_rot, crate::engine::planet_build::body_world_rot(&b.id, spin_f64, sim_t));
    let rotation = rot_d.as_quat();
    crate::engine::planet_build::note_body(state, &b.id, render_off, rot_d); // pieces built on it draw here
    // Distance from the CAMERA, not the frame origin
    // (v0.1238 starburst-far forensics). This was
    // render_off.length(): the body's distance from
    // the SHIP-LOCAL ORIGIN, indistinguishable from
    // camera distance in normal play (the camera sits
    // ~30 m from the origin) but wrong by the whole
    // journey for a camera FLOWN tens of thousands of
    // km (no floating-origin rebase). At 4.3 km inside
    // Earth's cloud layer the instrument read px=280
    // mix=0.00: the engine believed the planet disc
    // was 280 px wide and rendered the ORBITAL far-map
    // cloud regime + a starved LOD level from inside
    // the atmosphere - the operator's murky veil,
    // "clouds regen all over when I fly around", and
    // the shelf that followed the camera up. camr
    // (cam_r_ratio) always knew the truth because it
    // measures from cam_p; dist now does too. f64
    // throughout, so the subtraction is exact at these
    // magnitudes.
    let cam_rel = glam::DVec3::new(
        state.camera.position.x as f64,
        state.camera.position.y as f64,
        state.camera.position.z as f64,
    );
    let dist = (render_off - cam_rel).length().max(1.0);
    // Its surface draws below, for key_lights::sky_body_params.
    let frame_body = state.frame_lock_body.as_deref() == Some(b.id.as_str());
    let body_dir = (render_off - cam_rel).normalize_or_zero();
    let body_draw = move |material: usize| crate::renderer::key_lights::BodyDraw { material, frame_body, dir: body_dir };
    let radius_m = (b.radius_km * 1000.0) as f64;

    // Visibility floor: without a bloom/billboard
    // pass a real planet from tens of millions of
    // km is sub-pixel. Clamp the on-screen disc to
    // a minimum angular size so it reads as a body
    // (true POSITION is always exact -- only the
    // disc is floored, how Elite/KSP draw distant
    // bodies).
    //
    // NOT THE SUN (2026-10-21, renderer/sun_disc.rs).
    // Its disc was floored at dist * 0.045, ten times
    // its width, and read as "a giant glare/bloom spot"
    // with no body (the operator). It is now drawn at
    // its TRUE angular size by the shader, from the
    // view ray (sun_disc() in 80-fragment-shared.wgsl),
    // and this sphere only has to cover it and its
    // corona: three solar radii, or the disc's own
    // pixel floor plus two pixels when it is tiny.
    let visual_scale = if is_sun {
        let px_angle = (fov_deg as f64).to_radians() / (viewport_h as f64).max(1.0);
        state.renderer.sun_disc_pad = crate::renderer::sun_disc::camera_pads(dist, body_dir);
        // Its glare's frame; the air and cloud factors are
        // filled in once they are known this frame (the
        // sun-material block before the celestial pass).
        state.renderer.sun_frame = Some(crate::renderer::sun_glare::SunFrame {
            dir: body_dir.as_vec3(),
            dist_m: dist,
            air: [1.0; 3],
            cloud: 1.0,
            off_screen: 1.0,
            air_top: None,
        });
        (dist * crate::renderer::sun_disc::mesh_angular_radius(dist, px_angle).tan()) as f32
    } else {
        radius_m.max(dist * 0.0028) as f32
    };

    // Projected on-screen size (of the floored disc,
    // since that is what actually rasterizes) drives
    // the subdivision level.
    let px = humanity_terrain::planet::projected_pixel_diameter(
        visual_scale as f64,
        dist,
        viewport_h,
        fov_deg,
    );
    let level = humanity_terrain::planet::lod_level_for_pixels(
        px,
        px_per_level,
        max_level,
    );
    // The disc the camera's air may let show through (key_lights::sky_disc,
    // the daytime Moon) and the frame body's sphere (the shells' sun),
    // 2026-10-21. A disc drawn larger than the body is a floored point.
    sky.disc_cands.push(crate::renderer::key_lights::DiscCandidate {
        dir: body_dir,
        ang_radius: (visual_scale as f64 / dist).min(1.0).asin(),
        floored: visual_scale as f64 > radius_m * 1.0001,
        excluded: is_sun || frame_body,
    });
    if frame_body {
        state.renderer.frame_body_sphere =
            [render_off.x as f32, render_off.y as f32, render_off.z as f32, radius_m as f32];
    }

    // Procedural fractal surface when a def exists in
    // data/planets/<id>.ron and the master toggle is
    // on. The Sun stays a smooth emissive sphere.
    // Bodies without a def share plain icosphere
    // meshes cached under the reserved "_flat" id.
    let def = if procedural && !is_sun {
        state.planet_defs.get(&b.id)
    } else {
        None
    };

    // Per-pixel surface imagery (v0.811): a planet
    // with a baked albedo texture draws through its
    // OWN type-12 material (params.w flags the
    // shader's texture path). Its base_color.xyz is
    // repurposed as the planet CENTER in render
    // space, rewritten here every frame because the
    // floating origin moves it -- the shader
    // reconstructs the planet-local unit direction
    // from that center, which works identically for
    // the uniform sphere AND the chunk patches
    // (whose model matrices sit at patch anchors,
    // not the planet center). Both mesh paths below
    // use this same material, so the LOD handoff
    // never changes appearance.
    let textured_mat = if def.is_some() {
        state.planet_textured_materials.get(&b.id).copied()
    } else {
        None
    };
    if let Some(mi) = textured_mat {
        // params.w is the type-12 BIT FIELD (v0.816):
        // bit 0 = texture present, bit 1 = the
        // Settings > Graphics > Planets "Surface
        // detail" toggle (ocean waves + land
        // micro-texture). Rewritten every frame, so
        // flipping the toggle applies LIVE.
        let mut surf_flags = if state
            .gui_state
            .settings
            .planet_surface_detail
        {
            3.0
        } else {
            1.0
        };
        // bit 2 (v0.961): tree-card sprite atlas
        // resident - sprite cards sample it; until
        // then they shade flat conifer green.
        if state.renderer.tree_atlas_ready {
            surf_flags += 4.0;
        }
        state.renderer.update_material_full(
            mi,
            [
                render_off.x as f32,
                render_off.y as f32,
                render_off.z as f32,
                1.0,
            ],
            0.0,
            0.9,
            12.0,
            surf_flags,
        );
    }

    // ── Chunked planetary LOD (2026-07-11) ──
    // When a heightmap-bearing planet's disc grows
    // past the ladder's level-8 rung (the planet
    // fills the screen; px_per_level * 2^7, 1280 px
    // at the default threshold), the quadtree patch
    // path takes over from the uniform sphere:
    // detail follows the camera down to ~54 m
    // triangles, the far side + off-screen patches
    // are culled during tree descent, and patch
    // meshes stream in budgeted per frame. Full
    // architecture: terrain::planet_chunks.
    let chunk_activation_px = px_per_level
        * (1u32
            << (humanity_terrain::planet_chunks::CHUNK_ACTIVATION_LADDER_LEVEL
                - 1)) as f32;
    // Engage on EITHER trigger: the pixel rung (the
    // planet visually dominates -- resolution/FOV
    // aware), OR a plain proximity floor (within half
    // a radius of the surface). The floor exists
    // because the pixel rung is resolution-dependent:
    // on a small window the ladder legitimately tops
    // out below the rung, but a player descending to
    // the surface needs ground detail regardless of
    // how many pixels their window has (2026-07-11
    // LOD field report).
    let chunked_on = state.gui_state.settings.planet_chunked
        && def.is_some()
        && state.planet_heightmaps.contains_key(&b.id)
        && (px > chunk_activation_px || dist < radius_m * 1.5);
    let mut chunked_drawn = false;
    if chunked_on {
        use humanity_terrain::planet_chunks as chunks;
        let d = def.expect("chunked_on implies def");
        let hm = state
            .planet_heightmaps
            .get(&b.id)
            .expect("chunked_on implies heightmap");
        // The planet's rotation this frame is `rot_d`, bound once
        // above beside note_body (f64; narrow to f32 only at the end).
        // Camera position in the planet's UNROTATED
        // local frame, f64 end to end. The camera
        // lives at metre scale near the origin (dev
        // travel moves ship_world_pos instead), so
        // widening it to f64 is exact.
        let cam_pos = state.camera.position;
        let cam_render = glam::DVec3::new(
            cam_pos.x as f64,
            cam_pos.y as f64,
            cam_pos.z as f64,
        );
        let cam_local = rot_d.inverse() * (cam_render - render_off);
        // Camera frustum, rebuilt in f64 from the one
        // celestial projection (camera.rs) or culling
        // would disagree with what actually rasterizes.
        let proj_d = humanity_terrain::celestial_projection::celestial_projection_f64(
            fov_deg.max(1.0) as f64,
            state.camera.aspect.max(0.01) as f64,
        );
        let view_d = state.camera.view_matrix().as_dmat4();
        let frustum = chunks::FrustumPlanes::from_view_proj(
            &(proj_d * view_d),
        )
        .into_local(rot_d, render_off);
        // Conservative planet-wide radial band from
        // the def (tracks surface_relief + sea_level
        // automatically); built patches report their
        // measured band for tight culling.
        let band = chunks::RadialBand {
            min_r_m: d.radius
                * humanity_terrain::planet_surface::displaced_radius_f64(
                    d, 0.0,
                ),
            max_r_m: d.radius
                * humanity_terrain::planet_surface::displaced_radius_f64(
                    d, 1.0,
                ),
        };
        // Streamed terrain tiles (Earth): keep the
        // camera's 3x3 tile region resident and drain
        // arrivals. New/deeper patches then sample the
        // 460 m data; earlier base-built patches
        // refine progressively as LOD splits past
        // them (no forced invalidation - skirts and
        // zero-mean deltas cover the mixed window).
        if b.id == "earth" {
            let u = cam_local.normalize();
            let (tla, tlo) =
                humanity_terrain::planet_heightmap::dir_to_latlon_deg(
                    u.as_vec3(),
                );
            state.terrain_tiles.ensure_region(tla, tlo);
            if state.terrain_tiles.poll() {
                // New tile resident: a fresh selection
                // may now choose deeper patches - the
                // parked skip must not hold (v0.928).
                if let Some(cs) =
                    state.planet_chunk_states.get_mut(&b.id)
                {
                    cs.sel_dirty = true;
                }
            }
            // The whole-Earth map packs (rung 2): the
            // ground published into the one formula,
            // tile residency riding the last drawn
            // patches, and the OSM districts standing
            // in the world, all in map_meshes::tick.
            // ODbL: publish which packs are actually
            // DRAWN so the credit layer carries the
            // notice (the obligation is what is ON SCREEN).
            let map_published = crate::engine::map_meshes::tick(
                &mut state.map_meshes,
                d,
                hm,
                state.planet_chunk_states.get(&b.id).map(|cs| &cs.last_drawn),
                &mut state.renderer,
                &mut f.celestial_objects,
                cam_local,
                render_off,
                rot_d,
                rotation,
            );
            state.gui_state.osm_packs_drawn =
                crate::engine::map_meshes::drawn_names(&state.map_meshes, cam_local);
            // The map packs just changed the
            // elevation formula itself: every
            // terrain/water patch cached before
            // this frame is stale. In the normal
            // flow this tick precedes the world's
            // first patch build, so the purge finds
            // empty caches; it is load-bearing for
            // ordering surprises and future
            // mid-session pack installs.
            if map_published {
                let keys: Vec<String> = state
                    .planet_chunk_states
                    .keys()
                    .filter(|k| {
                        k.as_str() == b.id
                            || k.starts_with(&format!("{}::", b.id))
                    })
                    .cloned()
                    .collect();
                let mut purged = 0usize;
                for k in keys {
                    let Some(cs2) =
                        state.planet_chunk_states.get_mut(&k)
                    else {
                        continue;
                    };
                    for (_, mesh_idx, aslot) in cs2.purge_all() {
                        purged += 1;
                        if let Some(s) = aslot {
                            state.renderer.patch_arena_release(s);
                        } else if mesh_idx != usize::MAX {
                            state.renderer.replace_mesh(
                                mesh_idx,
                                Mesh::placeholder(
                                    &state.renderer.device,
                                ),
                            );
                            state
                                .planet_patch_free_slots
                                .push(mesh_idx);
                        }
                    }
                }
                if purged > 0 {
                    log::info!(
                        "[Map] packs published: purged {purged} patches built before them"
                    );
                }
            }
        }
        let tiles_ref = (b.id == "earth"
            && state.terrain_tiles.tier_installed())
        .then_some(&state.terrain_tiles);
        // Connected-ocean mask (v0.876): flips the
        // patch builder to true bathymetry; the
        // water shell draws the sea itself.
        let ocean_ref = (b.id == "earth")
            .then_some(state.ocean_mask.as_ref())
            .flatten();
        // Planet LOD knobs from Settings (v0.873),
        // clamped defensively: the patch budget must
        // stay well under the renderer's shared
        // MAX_OBJECTS 1024 (machines/walls/sky use
        // the rest).
        let split_px = state
            .gui_state
            .settings
            .terrain_split_px
            .clamp(2.0, 24.0);
        let patch_budget = state
            .gui_state
            .settings
            .terrain_patch_budget
            .clamp(64.0, 12288.0)
            as usize;
        let params = chunks::ChunkParams {
            occluder_r_m: None,
            radius_m: d.radius,
            band,
            // The tile tier earns deeper patches
            // (depth 20, ~0.4 m triangles, the 1 m
            // ladder); base-only stops where its
            // data goes flat.
            max_depth: if tiles_ref.is_some() {
                chunks::TILE_MAX_PATCH_DEPTH
            } else {
                chunks::MAX_PATCH_DEPTH
            },
            // An installed map pack's tile earns the 1 m ladder with or
            // without the tile tier (whole-Earth maps, rung 2b).
            deep: crate::engine::map_meshes::deep_tiles(&b.id),
            // The tree card stage's depth-15 ground within the card
            // distance, tile tier or not (BUG-301).
            card_floor: crate::engine::tree_card_stage::floor_for(d, &state.gui_state.settings),
            split_px,
            px_per_rad: viewport_h / fov_deg.max(1.0).to_radians(),
            max_leaves: patch_budget,
            max_build_requests: chunks::MAX_BUILD_REQUESTS,
        };
        let seed = d.terrain_seed;
        let cs = state
            .planet_chunk_states
            .entry(b.id.clone())
            .or_insert_with(|| chunks::ChunkState::new(seed));
        cs.frame += 1;
        // ── Parked-selection skip (v0.928, several
        // ms/frame at rest): in co-rotating surface
        // mode the planet-local camera pose is truly
        // static while parked (the frustum above is
        // already transformed into the local frame),
        // so when nothing has invalidated the last
        // selection - no builds, evictions, tile
        // arrivals, settings changes, or outstanding
        // build requests - the ~30k-node walk is
        // skipped and the last selection reused.
        // Inertial orbit views never skip (the
        // planet turns beneath a static camera).
        let cam_fwd_local = (rot_d.inverse()
            * state.camera.forward().as_dvec3())
        .normalize_or_zero();
        let split_now = params.split_px;
        let budget_now = patch_budget as f32;
        // The kept selection is a reuse: with the switch off the walk runs every frame.
        let parked = chunks::selection_kept(
            humanity_core::reuse::enabled(),
            state.camera.surface_mode
                && (cam_local - cs.last_sel_cam).length() < 0.25
                && cam_fwd_local.dot(cs.last_sel_fwd) > 0.99999
                && cs.last_sel_split_px == split_now
                && cs.last_sel_budget == budget_now
                && cs.last_sel_card_floor == params.card_floor
                && !cs.sel_dirty
                && cs.last_selection.as_ref().is_some_and(|s| {
                    s.build_requests.is_empty() && s.fully_covered
                }),
        );
        let selection = if parked {
            cs.last_selection.clone().expect("parked implies stored")
        } else {
            let sel = chunks::select_patches_sticky(
                cam_local,
                Some(&frustum),
                &|id| cs.cache.get(id).map(|e| e.band),
                &params,
                Some(&cs.last_drawn),
            );
            cs.last_sel_cam = cam_local;
            cs.last_sel_fwd = cam_fwd_local;
            cs.last_sel_split_px = split_now;
            cs.last_sel_budget = budget_now;
            cs.last_sel_card_floor = params.card_floor;
            cs.sel_dirty = false;
            cs.last_selection = Some(sel.clone());
            sel
        };
        // v0.1111: publish this frame's MEASURED
        // tree-card reach. The renderer's card cutoff
        // and the Settings row both read it, so
        // neither can promise cards on ground the LOD
        // walk never built. Travels with the parked
        // selection too (Selection is cloned whole),
        // so a stationary camera keeps its reading.
        humanity_terrain::far_trees::publish_card_reach_m(
            selection.stats.card_reach_m as f32,
        );
        let now_drawn: std::collections::HashSet<chunks::PatchId> =
            selection.draws.iter().cloned().collect();
        // Geomorph fades (v0.920): diff the drawn set
        // and dissolve every split/merge over
        // FADE_SECONDS instead of popping. Selection
        // itself is untouched (the v0.913 stability
        // machinery stays authoritative); this is
        // presentation only. Gated off during
        // activation (everything "appears" at once)
        // and by the Settings toggle.
        let fade_on = state.gui_state.settings.terrain_lod_fade
            && cs.active_last_frame
            && selection.fully_covered;
        if fade_on {
            let appeared: Vec<chunks::PatchId> =
                now_drawn.difference(&cs.last_drawn).cloned().collect();
            let vanished: Vec<chunks::PatchId> =
                cs.last_drawn.difference(&now_drawn).cloned().collect();
            cs.ingest_lod_swaps(&appeared, &vanished, dt);
        } else {
            cs.fades.clear();
        }
        cs.last_drawn = now_drawn;
        // Stamp every selected patch as used THIS
        // frame BEFORE eviction runs below: a patch
        // that just re-entered view after a long
        // absence still carries an old stamp, and
        // evicting it between selection and draw
        // would open a one-frame hole.
        let frame = cs.frame;
        // draws AND required (v0.1077): a patch the
        // selector depends on without drawing must not
        // age out of the LRU, or its subtree collapses
        // (the standstill terrain flicker).
        for id in selection.draws.iter().chain(selection.required.iter()) {
            if let Some(e) = cs.cache.get_mut(id) {
                e.last_used = frame;
            }
        }
        // TEMP diag (v0.887 bring-up): 1 Hz selection stats.
        if cs.frame % 60 == 0 {

            let dmax = selection.draws.iter().map(|d| d.depth).max().unwrap_or(0);
            let hist: Vec<usize> = (0..=20).map(|dd| selection.draws.iter().filter(|x| x.depth == dd).count()).collect();
            log::info!("[ChunkDiag] draws={} dmax={} sat={} req={} cache={} tiles={} resident={} budget={} alt={:.0}m refused=({:.0}px@d{}) hot(ve/bu/mi/sp)=({},{},{},{}) maxleaf=({:.0}px@d{}) hist={:?}", selection.draws.len(), dmax, selection.stats.budget_saturated, selection.build_requests.len(), cs.cache.len(), tiles_ref.is_some(), state.terrain_tiles.resident_count(), patch_budget, cam_local.length() - d.radius, selection.stats.max_refused_err, selection.stats.max_refused_depth, selection.stats.hot_vis_empty, selection.stats.hot_budget, selection.stats.hot_missing, selection.stats.hot_split, selection.stats.max_leaf_err, selection.stats.max_leaf_depth, hist);
        }
        // Budgeted patch builds, worst screen error
        // first; new meshes enter NEXT frame's
        // selection (progressive refinement, no
        // frame hitch).
        // v0.922 (operator: "a hard hang that lasts
        // several seconds at the transitionary
        // period" descending to the ocean): the
        // count budget alone let 64 x ~1 ms builds
        // stack ~60 ms on a single frame, and the
        // LOD-activation ramp holds that for seconds
        // of ~10 FPS. A WALL-CLOCK cap (3 ms) rules
        // the frame instead - the backlog just
        // spreads across more frames, which the
        // v0.920 crossfades already smooth over.
        // ── THREADED patch builds (v0.926, operator
        // mandate: tune every hang to max efficiency)
        // ── the v0.922 3 ms wall cap bounded the
        // frame but stretched the descent ramp; now
        // the CPU meshing fans across cores in a
        // scoped burst, so the WHOLE per-frame budget
        // (64 patches) completes inside one ~4 ms
        // slice. Real imagery colors when Earth ships
        // an albedo grid; classifier otherwise (same
        // source as the uniform sphere, so the LOD
        // handoff never changes hue). Borrow safety:
        // everything the workers touch (heightmap,
        // detail noise, tiles, ocean mask, albedo,
        // def) is read-only inside the scope, and the
        // frame thread WAITS in it, so nothing can
        // alias the tile streamer's &mut poll (which
        // runs sequentially elsewhere this frame).
        let build_t0 = Instant::now();
        let _cost_patch_build =
            humanity_core::frame_costs::stage("cpu.patch_build");
        // ONE read of the forest-density setting per
        // frame, handed to both vegetation streams
        // (v0.1111). The bake DEFINES the cards, so it
        // takes the setting; the harvest has to COVER
        // the cards already on screen, so it takes
        // harvest_tree_density (the max over the drawn
        // patches' own bake densities). cs.last_drawn
        // is this frame's selection, set above.
        let veg_bake_density = chunks::tree_density_clamped(
            state.gui_state.settings.tree_density,
        );
        let veg_harvest_density =
            cs.harvest_tree_density(veg_bake_density);
        let budget = state
            .gui_state
            .settings
            .terrain_builds_per_frame
            .clamp(1.0, 64.0) as usize;
        // The setting is the frame's, not each body's
        // (2026-11-01: this count was kept and never
        // read, so two chunk-active bodies could each
        // build the whole budget).
        let budget_left = budget.saturating_sub(sky.patch_builds_this_frame);
        let new_builds: Vec<chunks::PatchId> = selection
            .build_requests
            .iter()
            .filter(|id| !cs.cache.contains_key(id))
            .take(budget_left)
            .cloned()
            .collect();
        // The reuse switch off: once nothing new is wanted, the drawn patches
        // built longest ago are built again in place, a few a frame, through
        // the same workers and deadline (`chunks::reuse_off_rebuilds`).
        let rebuilds = chunks::reuse_off_rebuilds(
            selection.draws.iter().filter_map(|id| cs.cache.get(id).map(|e| (*id, e.built_frame))),
            humanity_core::reuse::enabled(),
            chunks::reuse_off_rebuild_share(new_builds.len()),
        );
        // The showcase `card_hull` switch: the patches carrying tree cards built again in place
        // with the new outline (engine/tree_card_stage.rs).
        let hull = crate::engine::tree_card_stage::hull_rebuilds(&b.id, cs, budget_left.max(8), &rebuilds);
        let to_build: Vec<chunks::PatchId> = new_builds.into_iter().chain(rebuilds).chain(hull).collect();
        if !to_build.is_empty() {
            let albedo = state.planet_albedos.get(&b.id).map(|a| a.as_ref());
            let detail = &cs.detail;
            let deadline =
                build_t0 + std::time::Duration::from_millis(4);
            let workers = std::thread::available_parallelism()
                .map(|n| n.get().saturating_sub(2).max(1))
                .unwrap_or(4)
                .min(to_build.len());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let to_build_ref = &to_build;
            let built: Vec<(chunks::PatchId, chunks::PatchMesh)> =
                std::thread::scope(|sc| {
                    let handles: Vec<_> = (0..workers)
                        .map(|_| {
                            let next = &next;
                            sc.spawn(move || {
                                let mut out = Vec::new();
                                loop {
                                    // Deadline check between
                                    // items keeps low-core
                                    // machines bounded; the
                                    // first item per worker
                                    // always runs so progress
                                    // is guaranteed.
                                    if !out.is_empty()
                                        && Instant::now() > deadline
                                    {
                                        break;
                                    }
                                    let i = next.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    if i >= to_build_ref.len() {
                                        break;
                                    }
                                    let id = to_build_ref[i];
                                    let src = chunks::ElevationSource::Heightmap {
                                        hm,
                                        detail,
                                        tiles: tiles_ref,
                                        ocean: ocean_ref,
                                    };
                                    out.push((
                                        id,
                                        chunks::build_patch_mesh_at_density(
                                            d, &src, albedo, &id,
                                            veg_bake_density,
                                        ),
                                    ));
                                }
                                out
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .flat_map(|h| {
                            h.join().expect("patch build worker panicked")
                        })
                        .collect()
                });
            // GPU upload + cache insert stay on the
            // frame thread (cheap; keeps the renderer
            // single-writer).
            if !built.is_empty() {
                cs.sel_dirty = true; // v0.928 parked skip
            }
            for (id, pm) in built {
                // A patch built again in place (the reuse
                // switch off): free what it drew from first.
                if let Some((old_mesh, old_slot)) = cs.take_for_rebuild(&id) {
                    if let Some(s) = old_slot {
                        state.renderer.patch_arena_release(s);
                    } else if old_mesh != usize::MAX {
                        state.renderer.replace_mesh(old_mesh, Mesh::placeholder(&state.renderer.device));
                        state.planet_patch_free_slots.push(old_mesh);
                    }
                }
                // Mega-buffer arena first (draw-
                // batching increment 1): geometry
                // uploads into the shared buffers,
                // no per-patch GPU buffer at all.
                // REAL byte accounting (vegetation
                // cards make patches bigger than
                // the old flat estimate), so the
                // LRU cap now tracks true VRAM.
                let verts =
                    Mesh::planet_surface_vertices(&pm.mesh);
                let real_bytes = verts.len()
                    * std::mem::size_of::<
                        humanity_core::vertex::Vertex,
                    >()
                    + pm.mesh.indices.len() * 4;
                if let Some(aslot) = state
                    .renderer
                    .patch_arena_upload(&verts, &pm.mesh.indices)
                {
                    cs.insert_built(
                        id,
                        usize::MAX,
                        Some(aslot),
                        real_bytes,
                        pm.anchor,
                        pm.band,
                        pm.tree_density,
                        pm.card_trees,
                    );
                } else {
                    // Arena full: classic per-patch
                    // mesh fallback (logged by the
                    // arena; recycles a freed slot
                    // when one exists).
                    let mesh = Mesh::from_vertices(
                        &state.renderer.device,
                        &verts,
                        &pm.mesh.indices,
                    );
                    let slot = if let Some(idx) =
                        state.planet_patch_free_slots.pop()
                    {
                        state.renderer.replace_mesh(idx, mesh);
                        idx
                    } else {
                        state.renderer.add_mesh(mesh)
                    };
                    cs.insert_built(
                        id,
                        slot,
                        None,
                        real_bytes,
                        pm.anchor,
                        pm.band,
                        pm.tree_density,
                        pm.card_trees,
                    );
                }
                sky.patch_builds_this_frame += 1;
            }
        }
        // Sustained build-budget saturation is worth
        // seeing in the log (normal for a few seconds
        // during an approach; suspicious if constant).
        // `budget` is the LIVE per-frame build
        // budget (the terrain_builds_per_frame
        // setting, clamped, computed above); the
        // old line compared against and printed the
        // PATCH_BUILDS_PER_FRAME constant (24),
        // which the setting had long since replaced,
        // so the log claimed 24 while 64 were built.
        if selection.build_requests.len() > budget
            && cs.frame.saturating_sub(cs.last_saturation_log) > 300
        {
            cs.last_saturation_log = cs.frame;
            log::info!(
                "Planet chunks '{}': build budget saturated ({} requested, {} per frame) - refining progressively",
                b.id,
                selection.build_requests.len(),
                budget,
            );
        }
        // LRU eviction past the byte cap: swap the GPU
        // mesh for a placeholder and bank the slot.
        let evicted = cs.collect_evictions(chunks::PATCH_CACHE_MAX_BYTES);
        if !evicted.is_empty() {
            cs.sel_dirty = true; // v0.928 parked skip
        }
        for (_, mesh_idx, aslot) in evicted {
            if let Some(s) = aslot {
                // Batched patch: return its arena
                // ranges to the free lists.
                state.renderer.patch_arena_release(s);
            } else if mesh_idx != usize::MAX {
                state.renderer.replace_mesh(
                    mesh_idx,
                    Mesh::placeholder(&state.renderer.device),
                );
                state.planet_patch_free_slots.push(mesh_idx);
            }
        }
        // END OF PATCH BUILD - the guard stops HERE so
        // `cpu.patch_build` means what its name says.
        // It used to run to the end of the whole
        // `if chunked_on` block (~2,030 lines): BUG-066.
        // The rest is `cpu.chunk_veg_and_draws`, still
        // a bucket - split it into patch_draws /
        // near_trees / veg_bake / far_cards /
        // water_shell, which also ends the double-count
        // of the harvest stages nested inside it.
        drop(_cost_patch_build);
        let _cost_chunk_rest = humanity_core::frame_costs::stage(
            "cpu.chunk_veg_and_draws",
        );
        // Draw patches only once the visible surface
        // is FULLY covered; until then (the first
        // frames after activation, while the 20 roots
        // build) the uniform sphere below keeps
        // drawing so the planet never blinks out.
        if selection.fully_covered && !selection.draws.is_empty() {
            // Per-patch crossfade values (v0.920):
            // positive = rising, absent = normal.
            let fade_map = cs.fade_values();
            // Draw-batching increment 1: arena
            // patches go into the batched list (one
            // draw each, shared bind state); only
            // arena-overflow patches still ride
            // celestial_objects. patch_material /
            // rot are shared batch state.
            let patch_material = textured_mat
                .unwrap_or(state.planet_surface_material);
            let push_patch = |e: &chunks::PatchEntry,
                                  fade: f32,
                                  batched: &mut Vec<
                crate::renderer::PatchDraw,
            >,
                                  classic: &mut Vec<
                RenderObject,
            >| {
                // Per-patch translation composed in
                // f64 (render offset + rotated
                // anchor), narrowed to f32 at the
                // END: the patch's own vertices are
                // small offsets, so nothing here
                // ever puts planet-radius magnitudes
                // through f32 math.
                let anchor_render =
                    render_off + rot_d * e.anchor;
                let position = Vec3::new(
                    anchor_render.x as f32,
                    anchor_render.y as f32,
                    anchor_render.z as f32,
                );
                if let Some(slot) = e.slot {
                    batched.push(crate::renderer::PatchDraw {
                        slot,
                        position,
                        fade,
                    });
                } else {
                    classic.push(RenderObject {
                        fade,
                        position,
                        rotation,
                        scale: Vec3::ONE,
                        mesh: e.mesh,
                        // Per-pixel imagery when baked
                        // (v0.811), per-face fallback
                        // otherwise -- same choice as
                        // the uniform sphere below.
                        material: patch_material,
                    });
                }
            };
            let classic_before = f.celestial_objects.len();
            let mut batched: Vec<crate::renderer::PatchDraw> =
                Vec::with_capacity(selection.draws.len());
            for id in &selection.draws {
                if let Some(e) = cs.cache.get(id) {
                    // A drawn patch can only be
                    // rising (falling ids are purged
                    // from the drawn set); clamp
                    // keeps a stray negative from
                    // hiding a normal draw.
                    let fade = fade_map
                        .get(id)
                        .copied()
                        .unwrap_or(0.0)
                        .max(0.0);
                    push_patch(
                        e,
                        fade,
                        &mut batched,
                        &mut f.celestial_objects,
                    );
                }
            }
            // Falling patches (v0.920): the outgoing
            // generation keeps drawing with the
            // complementary Bayer mask until its
            // clock retires it; its pixels partition
            // exactly against the risers above.
            for (id, fade) in fade_map.iter() {
                if *fade < 0.0 && !cs.last_drawn.contains(id) {
                    if let Some(e) = cs.cache.get(id) {
                        push_patch(
                            e,
                            *fade,
                            &mut batched,
                            &mut f.celestial_objects,
                        );
                    }
                }
            }
            // Hand the batch to the renderer. Only
            // one planet is ever chunk-active, so a
            // plain assign is safe; patch_draws was
            // cleared at the top of this frame's
            // celestial build.
            // Front-to-back sort (near first): with
            // reverse-Z Greater + depth write, early-z
            // then rejects far-patch fragments hiding
            // behind near terrain -- at horizon looks
            // the type-12 fragment is the frame's most
            // expensive shader and overdraw was eating
            // multiple ms. ~12k-element sort is well
            // under 0.5 ms; instance indices stay
            // consistent because the storage upload
            // happens AFTER this handoff.
            let cam_eff = state.camera.effective_position();
            batched.sort_unstable_by(|a, b| {
                let da = (a.position - cam_eff).length_squared();
                let db = (b.position - cam_eff).length_squared();
                da.partial_cmp(&db)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            state.renderer.patch_batch_rot =
                glam::Mat4::from_quat(rotation);
            state.renderer.patch_batch_material = patch_material;
            // 5 s diag: batched vs classic-fallback
            // split (fallback should be 0 unless the
            // arena overflowed; see [PatchArena]).
            if cs.frame % 300 == 0 {
                log::info!(
                    "[PatchBatch] {} batched, {} classic-fallback",
                    batched.len(),
                    f.celestial_objects.len() - classic_before,
                );
            }
            state.renderer.patch_draws = batched;
            chunked_drawn = true;
            sky.body_draws.push(body_draw(patch_material));
        }
        // ── Near-field REAL trees (v0.911) ──
        // The harvest, the draw plan and the card-hide
        // promise live in engine/frame_near_trees.rs:
        // 501 lines with one job. Add near-tree work
        // THERE, not here (monolith ratchet, v0.1320).
        // The borrows are named field by field because
        // this call site is already holding `cs`, `hm`,
        // `d` and the ocean mask out of the state, so
        // `&mut state` is an E0499 here.
        crate::engine::frame_near_trees::draw_near_trees(
            &mut crate::engine::frame_near_trees::NearTreeState {
                renderer: &mut state.renderer,
                asset_manager: &state.asset_manager,
                decoration_mesh_cache: &mut state.decoration_mesh_cache,
                tree_atlas_attempted: &mut state.tree_atlas_attempted,
                near_tree_retry_s: &mut state.near_tree_retry_s,
                gui_state: &state.gui_state,
                planet_albedos: &state.planet_albedos,
                start_time: &state.start_time,
                near_trees: &mut state.near_trees,
                near_trees_center: &mut state.near_trees_center,
                near_tree_new: &mut state.near_tree_new,
                near_tree_born_s: &mut state.near_tree_born_s,
                near_tree_inputs: &mut state.near_tree_inputs,
            },
            crate::engine::frame_near_trees::NearTreeView {
                cam_local,
                render_off,
                rot_d,
                rotation,
                frustum: &frustum,
                chunked_drawn,
                veg_harvest_density,
                hm,
                tiles_ref,
                ocean_ref,
            },
            cs,
            b,
            d,
            &mut f.celestial_objects,
        );
        // The card stage's counter (BUG-301): the [TreeCards] line and the done file's reading,
        // taken here because the near trees have just set this frame's hide radius.
        crate::engine::tree_card_stage::note(cs, d.radius, state.renderer.tree_card_hide_m, state.renderer.tree_card_far_m, state.renderer.tree_atlas_ready);
        // ── Near-field GRASS STRANDS (v0.1091) ──
        //
        // TWO CLOCKS, and the split is the whole
        // design (see near_grass_instances):
        //
        //  * The HARVEST runs at most once every
        //    GRASS_REHARVEST_M of walking and returns
        //    a SUPERSET - every tiller any camera
        //    within GRASS_HARVEST_MARGIN_M could
        //    want. It is the expensive half.
        //  * The DRAW set is re-derived EVERY FRAME
        //    from the live camera distance. So the
        //    density ramp is always anchored on where
        //    the player actually is, and recentring
        //    the harvest changes nothing on screen.
        //
        // The alternative (harvest on hysteresis and
        // draw whatever it returned) re-anchors the
        // ramp every few metres: a tiller 6 m from a
        // new pose was 14 m from the old one, where
        // grass_density_at is 4x smaller, so a whole
        // annulus of blades pops in at once. That is
        // exactly the population ring the vantage's
        // NO GRASS RING regression forbids.
        //
        // The harvest is INLINE, not on a worker,
        // deliberately: it has to see the same
        // streamed tile tier the patch mesh saw or
        // every base sits on ground that is not the
        // ground you can see, and TerrainTiles is
        // owned + mutated by the frame thread. What
        // makes that affordable is the drawn-surface
        // vertex memo - at the depths the selector
        // actually reaches, a few thousand tillers
        // share a few hundred lattice samples.
        //
        // HEIGHT ABOVE THE GROUND, not above the
        // datum. `alt_over` is elevation above the
        // nominal sphere, which at Fuji is 1,244 m
        // while the player is standing ON it - a
        // datum-keyed gate switches the whole layer
        // off at every vantage that has any relief,
        // silently (measured: the first probe run of
        // this increment harvested 0 tillers at
        // Fuji and logged nothing after world entry).
        // `last_ground_r` is the sampled ground the
        // walk clamp is already using, so this asks
        // the same question the player's feet do.
        let cam_dir_now = cam_local.normalize();
        let above_ground = if state.last_ground_r > 0.0
            && (state.last_ground_dir - cam_dir_now).length()
                * d.radius
                < 200.0
        {
            cam_local.length() - state.last_ground_r
        } else {
            f64::MAX
        };
        // The ramp is zero past GRASS_FAR_M of SURFACE
        // distance, so an eye much higher than that
        // sees nothing anyway; the headroom covers
        // eye height, jumping and standing on a rock.
        // "0 = off" for BOTH grass knobs lives in one
        // pure predicate (2026-09-18): cover 0 has
        // nothing to harvest, detail 0 has nothing to
        // draw, and skipping the harvest here is what
        // keeps a zero-blade mesh away from the
        // renderer. The else branch below clears the
        // set, so flipping to 0 empties the sward on
        // the same frame.
        let grass_on = chunked_drawn
            && above_ground < 60.0
            && humanity_terrain::grass::grass_layer_on(
                state.gui_state.settings.grass_density,
                state.gui_state.settings.grass_detail,
            );
        if grass_on {
            // The drawn ground's own LOD: the finest
            // patch on screen is the one under the
            // camera. Bases are interpolated on THAT
            // mesh, so a change here invalidates the
            // whole set.
            let draw_depth = cs
                .last_drawn
                .iter()
                .map(|p| p.depth)
                .max()
                .unwrap_or(0);
            let moved =
                (state.near_grass_center - cam_local).length();
            // Rate limit, for the streaming walk-up.
            // During world entry the selector climbs
            // depth 1 -> 13 over a handful of frames
            // and each step is a depth CHANGE, so the
            // layer re-harvested 13 times in a row at
            // one standing pose - 13 ms apiece, 170 ms
            // of hitching, twelve of them thrown away.
            // A depth floor would be the wrong fix:
            // 13 IS the settled depth at Fuji (the T1
            // cap - 53.8 m triangles), and grass has to
            // draw on whatever the ground is really
            // drawn at. The trigger stays latched, so a
            // skipped change is taken on the next frame
            // past the limit.
            let now_s = state.start_time.elapsed().as_secs_f32();
            let ready = state.near_grass.is_empty()
                || now_s - state.near_grass_at_s >= 0.4;
            if draw_depth > 0
                && ready
                && (moved > chunks::GRASS_REHARVEST_M
                    || draw_depth != state.near_grass_depth)
            {
                let src = chunks::ElevationSource::Heightmap {
                    hm,
                    detail: &cs.detail,
                    tiles: tiles_ref,
                    ocean: ocean_ref,
                };
                let t0 = std::time::Instant::now();
                let _cost_grass_harvest =
                    humanity_core::frame_costs::stage("cpu.grass_harvest");
                // Instance ceiling, now a setting
                // (v0.1108.2). It BINDS at large draw
                // distances - the superset grows with
                // the area - and when it binds it cuts
                // the far rim off rather than thinning,
                // so the warning below is not optional.
                let grass_cap = state.gui_state.settings.grass_harvest_cap
                    .clamp(1000.0, crate::config::GRASS_HARVEST_CAP_MAX)
                    as usize;
                state.near_grass = chunks::near_grass_instances(
                    d,
                    &src,
                    state
                        .planet_albedos
                        .get(&b.id)
                        .map(|a| a.as_ref()),
                    cam_local.normalize(),
                    chunks::grass_far_m() as f64,
                    chunks::GRASS_HARVEST_MARGIN_M,
                    draw_depth,
                    grass_cap,
                );
                state.near_grass_center = cam_local;
                state.near_grass_depth = draw_depth;
                state.near_grass_at_s = now_s;
                log::info!(
                    "[Grass] harvest: {} tillers (superset) at depth {} \
                                             in {:.1} ms (eye {:.1} m above ground)",
                    state.near_grass.len(),
                    draw_depth,
                    t0.elapsed().as_secs_f64() * 1000.0,
                    above_ground
                );
                // A bound cap cuts the FAR EDGE off (the
                // walk is nearest-first and breaks), so
                // say so rather than let a hard circle
                // look like a design choice.
                if state.near_grass.len() >= grass_cap {
                    log::warn!(
                        "[Grass] harvest TRUNCATED at the {} cap - the \
                                                 field is cut off at its outer rim and will end \
                                                 in a hard circle. Lower 'Grass: ground cover' \
                                                 or raise the harvest cap beside it.",
                        grass_cap
                    );
                }
            }
        } else if !state.near_grass.is_empty() {
            state.near_grass.clear();
            state.near_grass_center =
                glam::DVec3::splat(f64::MAX);
            state.near_grass_depth = 0;
            state.near_grass_at_s = -1.0;
        }
        // Per-frame: the LIVE gate. Reject in f64
        // against the real camera, keep what is
        // visible, and hand the renderer one packed
        // instance array for a single draw.
        {
            let tg = std::time::Instant::now();
            let cam_dir = cam_local.normalize();
            let radius = d.radius.max(1.0);
            let inst = &mut state.grass_instances;
            inst.clear();
            for t in &state.near_grass {
                // Small-angle surface distance: the
                // chord IS the arc under 30 m on a
                // 6,371 km sphere, and this is the
                // same measure the harvest used.
                let d_m =
                    ((t.dir - cam_dir).length() * radius) as f32;
                let e = chunks::grass_live_emerge(t.thr, d_m);
                if e <= 0.0 {
                    continue;
                }
                let base_local = t.dir * t.r_m;
                let p = render_off + rot_d * base_local;
                let up = (rot_d * t.dir).as_vec3().normalize();
                inst.push(
                    crate::renderer::mesh::GrassInstance {
                        pos_fade: [
                            p.x as f32,
                            p.y as f32,
                            p.z as f32,
                            0.0,
                        ],
                        up_scale: [
                            up.x,
                            up.y,
                            up.z,
                            t.height_m * e,
                        ],
                        grass: [
                            t.yaw,
                            t.phase,
                            crate::renderer::mesh::GrassInstance
                                ::pack_color(t.color),
                            0.0,
                        ],
                    },
                );
            }
            let n = inst.len();
            let insts =
                std::mem::take(&mut state.grass_instances);
            state.renderer.set_grass_instances(&insts);
            state.grass_instances = insts;
            // 1 Hz diag beside the tree handoff.
            if n > 0 {
                use std::sync::atomic::{AtomicU64, Ordering};
                static LASTG: AtomicU64 = AtomicU64::new(0);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|x| x.as_secs())
                    .unwrap_or(0);
                if LASTG.swap(now, Ordering::Relaxed) != now {
                    log::info!(
                        "[Grass] drawn={} of {} harvested (depth {}), \
                                                 {} tris, per-frame CPU {:.2} ms",
                        n,
                        state.near_grass.len(),
                        state.near_grass_depth,
                        n * 90,
                        tg.elapsed().as_secs_f64() * 1000.0,
                    );
                }
            }
        }
        // ── Far-tree card sheet (v0.1022, vegetation
        // instancing arc increment 1) ── one streamed
        // mesh of hash-decimated clump cards covering
        // ~1.2-150 km, decoupled from patch LOD.
        // Closes the "trees aren't loaded from
        // altitude" gap and buries the square (patch
        // edge) / circle (card slider) boundary
        // shapes. Rebuilt on a worker when the camera
        // ground point moves; same cell hash as the
        // baked cards so the handoff never teleports
        // a tree.
        // Default OFF (v0.1029): at altitude the clump
        // cards read as "black squares in a grid"
        // (operator, twice). Proper far coverage is
        // the impostor arc; the sheet stays behind
        // this Settings toggle for A/B until then.
        if b.id == "earth"
            && chunked_drawn
            && state.gui_state.settings.far_tree_sheet
        {
            if let Some(rx) = &state.far_tree_pending {
                match rx.try_recv() {
                    Ok(result) => {
                        state.far_tree_pending = None;
                        log::info!(
                            "[FarTrees] sheet built: {} cards",
                            result
                                .as_ref()
                                .map(|(m, _)| m.vertices.len() / 12)
                                .unwrap_or(0)
                        );
                        if let Some((md, anchor)) = result {
                            let mesh = Mesh::from_planet_surface(
                                &state.renderer.device,
                                &md,
                            );
                            let idx = if let Some(i) = state.far_tree_mesh
                            {
                                state.renderer.replace_mesh(i, mesh);
                                i
                            } else if let Some(i) =
                                state.planet_patch_free_slots.pop()
                            {
                                state.renderer.replace_mesh(i, mesh);
                                i
                            } else {
                                state.renderer.add_mesh(mesh)
                            };
                            state.far_tree_mesh = Some(idx);
                            state.far_tree_anchor = anchor;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        state.far_tree_pending = None;
                    }
                }
            }
            if state.far_tree_pending.is_none()
                && (cam_local - state.far_tree_built_cam).length()
                    > humanity_terrain::far_trees::FAR_TREE_REBUILD_M
            {
                if let Some(hm) = state.planet_heightmaps.get(&b.id) {
                    let (tx, rx) = std::sync::mpsc::channel();
                    state.far_tree_pending = Some(rx);
                    state.far_tree_built_cam = cam_local;
                    let hm = hm.clone();
                    let albedo =
                        state.planet_albedos.get(&b.id).cloned();
                    let def = d.clone();
                    let cam = cam_local;
                    std::thread::spawn(move || {
                        let _ = tx.send(
                            humanity_terrain::far_trees::build_far_tree_sheet(
                                &def,
                                &hm,
                                albedo.as_deref(),
                                cam,
                            ),
                        );
                    });
                }
            }
            // Staleness guard (v0.1024, operator:
            // sky-blocking squares): fast flight
            // outruns the async rebuild, and a sheet
            // built kilometres away parks its far
            // bands around/over the player. Hide an
            // outrun sheet - briefly treeless beats
            // wrong.
            let sheet_fresh = (cam_local
                - state.far_tree_built_cam)
                .length()
                < humanity_terrain::far_trees::FAR_TREE_REBUILD_M * 3.0;
            if let (Some(mi), true) = (state.far_tree_mesh, sheet_fresh)
            {
                let anchor_render =
                    render_off + rot_d * state.far_tree_anchor;
                f.celestial_objects.push(RenderObject {
                    fade: 0.0,
                    position: Vec3::new(
                        anchor_render.x as f32,
                        anchor_render.y as f32,
                        anchor_render.z as f32,
                    ),
                    rotation,
                    scale: Vec3::ONE,
                    mesh: mi,
                    material: textured_mat
                        .unwrap_or(state.planet_surface_material),
                });
            }
        }
        if chunked_drawn != cs.active_last_frame {
            cs.active_last_frame = chunked_drawn;
            log::info!(
                "Planet chunks '{}': {} ({} patches drawn, {} cached, {:.1} MB)",
                b.id,
                if chunked_drawn {
                    "ACTIVE"
                } else {
                    "inactive (uniform sphere)"
                },
                selection.draws.len(),
                cs.cache.len(),
                cs.total_bytes as f64 / (1024.0 * 1024.0),
            );
        }

        // ── Water shell (v0.876 real-water Stage 1) ──
        // The translucent sea surface over connected
        // ocean. Only while terrain patches draw
        // (below the handoff the uniform sphere's
        // clamped sea IS the water); its own small
        // quadtree at the sea radius, capped shallow
        // (waves only need ~27 m mesh; the vertex
        // shader adds the height).
        if chunked_drawn && ocean_ref.is_some() {
            let om = ocean_ref.unwrap();
            // The two shells (coarse backstop, then
            // the displaced wave surface) live in
            // engine/frame_water.rs: 586 lines with
            // one job. Add water work THERE, not here
            // (monolith ratchet, v0.1320). The borrows
            // are named field by field because `d` is
            // borrowed out of state.planet_defs for the
            // whole body loop, so `&mut state` is an
            // E0502 here.
            crate::engine::frame_water::push_water_shells(
                &mut crate::engine::frame_water::WaterShellState {
                    renderer: &mut state.renderer,
                    planet_chunk_states: &mut state.planet_chunk_states,
                    planet_water_materials:
                        &mut state.planet_water_materials,
                    planet_heightmaps: &state.planet_heightmaps,
                    planet_patch_free_slots:
                        &mut state.planet_patch_free_slots,
                    camera: &state.camera,
                    gui_state: &state.gui_state,
                },
                crate::engine::frame_water::WaterView {
                    cam_local,
                    cam_fwd_local,
                    frustum: &frustum,
                    render_off,
                    rot_d,
                    rotation,
                    split_px,
                    viewport_h,
                    fov_deg,
                },
                b,
                d,
                om,
                &mut f.celestial_transparent,
            );
        }
    } else if let Some(cs) =
        state.planet_chunk_states.get_mut(&b.id)
    {
        // The camera left this planet (chunked mode
        // dropped out). Shrink the parked cache ONCE
        // to the warm floor so a departed planet
        // holds ~64 MB (fast re-approach; roots
        // pinned) instead of the full working set.
        if cs.active_last_frame {
            cs.active_last_frame = false;
            // Advance the frame stamp so nothing
            // counts as used-this-frame and the LRU
            // shrink can actually run.
            cs.frame += 1;
            cs.sel_dirty = true; // v0.928 parked skip
            log::info!(
                "Planet chunks '{}': deactivated ({:.1} MB cached) - shrinking to the warm floor over the next frames",
                b.id,
                cs.total_bytes as f64 / (1024.0 * 1024.0),
            );
        }
        // AMORTIZED shrink (v0.930, operator: "10+
        // second hang" crossing away from Earth):
        // the old one-shot eviction paid the WHOLE
        // session cache on a single frame - with the
        // quadratic LRU that was ~10 s. Eviction is
        // linear now AND capped per call, and this
        // runs every frame while over the warm
        // floor, so a big departure shrink costs a
        // few ms across a handful of frames. Free
        // (early-returns) once under the floor.
        // Keep the frame stamp ticking while
        // deactivated (it normally only advances
        // when chunk-active) or the 120-frame
        // recency guard freezes and the drain
        // stalls two batches in.
        if cs.total_bytes
            > humanity_terrain::planet_chunks::PATCH_CACHE_WARM_BYTES
        {
            cs.frame += 1;
        }
        let evicted = cs.collect_evictions(
            humanity_terrain::planet_chunks::PATCH_CACHE_WARM_BYTES,
        );
        if !evicted.is_empty() {
            cs.sel_dirty = true;
            log::info!(
                "Planet chunks '{}': warm shrink batch ({} evicted, {:.1} MB remain)",
                b.id,
                evicted.len(),
                cs.total_bytes as f64 / (1024.0 * 1024.0),
            );
            for (_, mesh_idx, aslot) in evicted {
                if let Some(s) = aslot {
                    state.renderer.patch_arena_release(s);
                } else if mesh_idx != usize::MAX {
                    state.renderer.replace_mesh(
                        mesh_idx,
                        Mesh::placeholder(&state.renderer.device),
                    );
                    state.planet_patch_free_slots.push(mesh_idx);
                }
            }
        }
    }

    // While chunked mode is engaged, cap the uniform
    // FALLBACK sphere at level 7 (327k faces): levels
    // 8-9 are exactly the several-second build hitch
    // the patch path exists to replace, and the
    // fallback only shows for the few frames before
    // patch cover completes.
    let level = if chunked_on { level.min(7) } else { level };

    let cache_id: &str = if def.is_some() { b.id.as_str() } else { "_flat" };
    if !chunked_drawn {
        let key = (cache_id.to_string(), level);
        // Deliver a finished background sky-sphere
        // build for this body+level (v0.930): GPU
        // upload + cache insert here, ~tens of ms.
        if let Some(rx) = state.sky_mesh_pending.get(&key) {
            if let Ok(data) = rx.try_recv() {
                let mesh = Mesh::from_planet_surface(
                    &state.renderer.device,
                    &data,
                );
                let m = state.renderer.add_mesh(mesh);
                state.planet_mesh_cache.insert(key.clone(), m);
                state.sky_mesh_pending.remove(&key);
                log::info!(
                    "Sky-planet mesh built ASYNC: {} level {} (~{:.0} px)",
                    cache_id, level, px
                );
            }
        }
        let body_mesh = if let Some(&m) = state.planet_mesh_cache.get(&key) {
            m
        } else if def.is_some() && level >= 7 {
            // ── Heavy tier goes OFF-THREAD (v0.930,
            // operator: "10+ second hang" at the
            // disc-fills-the-screen boundary) ──
            // level 7 = 327k faces, 8 = 1.3M, 9 =
            // 5.2M, each vertex sampling the
            // heightmap - seconds of CPU meshing
            // that used to run on the frame thread.
            // The Arc'd grids make the spawn free;
            // the current lower level keeps drawing
            // until the worker delivers (progressive
            // refinement, same idea as the terrain
            // patches).
            let d = def.expect("guarded by def.is_some");
            if !state.sky_mesh_pending.contains_key(&key) {
                let (tx, rx) = std::sync::mpsc::channel();
                let d2 = d.clone();
                let hm2 = state.planet_heightmaps.get(&b.id).cloned();
                let al2 = state.planet_albedos.get(&b.id).cloned();
                let lvl = level;
                std::thread::spawn(move || {
                    let data =
                        humanity_terrain::planet_surface::build_surface_mesh(
                            &d2,
                            hm2.as_deref(),
                            al2.as_deref(),
                            lvl,
                        );
                    let _ = tx.send(data);
                });
                state.sky_mesh_pending.insert(key.clone(), rx);
                log::info!(
                    "Sky-planet mesh building in BACKGROUND: {} level {}",
                    cache_id, level
                );
            }
            // Best cached lower level carries the
            // frame meanwhile.
            let mut fallback = None;
            for l in (0..level).rev() {
                if let Some(&m) = state
                    .planet_mesh_cache
                    .get(&(cache_id.to_string(), l))
                {
                    fallback = Some(m);
                    break;
                }
            }
            if let Some(m) = fallback {
                m
            } else {
                // Fresh far teleport with nothing
                // cached: sync-build a modest level
                // (fast) so the planet never blinks
                // out; the heavy level swaps in when
                // the worker lands.
                let lvl = level.min(5);
                let key5 = (cache_id.to_string(), lvl);
                if let Some(&m) = state.planet_mesh_cache.get(&key5) {
                    m
                } else {
                    let hm = state
                        .planet_heightmaps
                        .get(&b.id)
                        .map(|a| a.as_ref());
                    let al = state
                        .planet_albedos
                        .get(&b.id)
                        .map(|a| a.as_ref());
                    let data =
                        humanity_terrain::planet_surface::build_surface_mesh(
                            d, hm, al, lvl,
                        );
                    let mesh = Mesh::from_planet_surface(
                        &state.renderer.device,
                        &data,
                    );
                    let m = state.renderer.add_mesh(mesh);
                    state.planet_mesh_cache.insert(key5, m);
                    log::info!(
                        "Sky-planet mesh built (fallback): {} level {}",
                        cache_id, lvl
                    );
                    m
                }
            }
        } else {
            let mesh = if let Some(d) = def {
                // Real heightmap (Earth) beats noise,
                // real albedo imagery beats the band
                // classifier; both feed the same
                // mesh builder. Levels <= 6 build in
                // well under a frame's budget.
                let hm = state.planet_heightmaps.get(&b.id).map(|a| a.as_ref());
                let al = state.planet_albedos.get(&b.id).map(|a| a.as_ref());
                let data =
                    humanity_terrain::planet_surface::build_surface_mesh(d, hm, al, level);
                Mesh::from_planet_surface(&state.renderer.device, &data)
            } else {
                let mut ico = humanity_terrain::icosphere::Icosphere::new();
                ico.subdivide_n(level);
                Mesh::from_icosphere(&state.renderer.device, &ico, 1.0)
            };
            let m = state.renderer.add_mesh(mesh);
            state.planet_mesh_cache.insert(key, m);
            log::info!(
                "Sky-planet mesh built: {} level {} (~{:.0} px on screen)",
                cache_id, level, px
            );
            m
        };

        let material = if is_sun {
            state.sun_material
        } else if def.is_some() {
            // Per-pixel imagery when baked (v0.811),
            // per-face packed colors otherwise.
            textured_mat.unwrap_or(state.planet_surface_material)
        } else {
            // v0.905: the four giants get their own
            // type-18 band palettes; everything else
            // keeps the coarse per-type colors.
            match b.id.as_str() {
                "jupiter" => state.solar_body_materials[4],
                "saturn" => state.solar_body_materials[5],
                "uranus" => state.solar_body_materials[6],
                "neptune" => state.solar_body_materials[7],
                _ =>
            match b.body_type.as_str() {
                "gas_giant" | "gas giant" => state.solar_body_materials[1],
                "ice_giant" | "ice giant" => state.solar_body_materials[1],
                "dwarf_planet" | "dwarf" | "kuiper" => {
                    state.solar_body_materials[2]
                }
                "terrestrial" | "moon" | "asteroid" => {
                    state.solar_body_materials[0]
                }
                _ => state.solar_body_materials[3],
            }
            }
        };
        if is_sun {
            // The Sun (type 17, renderer/sun_disc.rs)
            // blends: its antialiased rim and its corona
            // ride the transparent list, first, so the
            // air and clouds in front draw over it.
            f.celestial_transparent.push(RenderObject { fade: 0.0,
                position: Vec3::new(
                    render_off.x as f32,
                    render_off.y as f32,
                    render_off.z as f32,
                ),
                rotation,
                scale: Vec3::splat(visual_scale),
                mesh: body_mesh,
                material,
            });
        } else {
            f.celestial_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(
                    render_off.x as f32,
                    render_off.y as f32,
                    render_off.z as f32,
                ),
                rotation,
                scale: Vec3::splat(visual_scale),
                mesh: body_mesh,
                material,
            });
            sky.body_draws.push(body_draw(material));
        }
        // (The v0.886 "corona halo", a radial glow on a
        // sphere three times the floored disc, was drawn
        // here until 2026-10-21. The corona is now
        // Baumbach's, in the Sun's own shader, and the
        // glare is the aperture's, in the present pass.)
    }
    let position = Vec3::new(
        render_off.x as f32,
        render_off.y as f32,
        render_off.z as f32,
    );
    // Atmosphere shell (v0.763 fresnel; v0.807
    // analytic scattering): a slightly larger
    // translucent shell whose fragments evaluate a
    // single-scattering integral (shader type 14) or
    // the old fresnel limb tint (type 13) when the
    // Settings fallback is chosen. Data-driven per
    // planet; airless bodies (atmosphere_color None
    // or zero alpha) never spawn one, and sub-8px
    // discs skip it (a shell thinner than a pixel
    // just shimmers). The shell composites over BOTH
    // surface paths -- the uniform sphere and the
    // chunked-LOD patches -- because both draw as
    // opaque celestial objects before this
    // transparent list blends over them.
    if px >= 8.0 {
        if let Some(d) = def {
            // The cloud deck, the atmosphere dome and
            // the view-dependent order between them
            // live in engine/frame_shells.rs: 1,057
            // lines with one job. Add shell work
            // THERE, not here (monolith ratchet,
            // v0.1320). The borrows are named field by
            // field because `d` is borrowed out of
            // state.planet_defs for the whole body
            // loop, so `&mut state` is an E0502 here.
            crate::engine::frame_shells::push_planet_shells(
                &mut crate::engine::frame_shells::ShellState {
                    renderer: &mut state.renderer,
                    planet_mesh_cache: &mut state.planet_mesh_cache,
                    planet_cloud_materials:
                        &mut state.planet_cloud_materials,
                    planet_atmo_materials:
                        &mut state.planet_atmo_materials,
                    camera: &state.camera,
                    data_store: &state.data_store,
                    gui_state: &state.gui_state,
                    start_time: &state.start_time,
                    cloud_cover_override: &state.cloud_cover_override,
                    cloud_type_override: &state.cloud_type_override,
                    cloud_event_boost: &state.cloud_event_boost,
                    cloud_event_tint: &state.cloud_event_tint,
                    cloud_map_anchor: &mut state.cloud_map_anchor,
                    cloud_map_reanchors: &mut state.cloud_map_reanchors,
                    weather_anchor: &mut state.weather_anchor,
                    cloud_map_regime: &mut state.cloud_map_regime,
                    cloud_prev_cam_local:
                        &mut state.cloud_prev_cam_local,
                    cloud_prev_rot: &mut state.cloud_prev_rot,
                    cloud_ref_frame: &mut state.cloud_ref_frame,
                },
                crate::engine::frame_shells::ShellView {
                    render_off,
                    rot64: rot_d,
                    rotation,
                    visual_scale,
                    position,
                    px,
                },
                b,
                d,
                &mut f.celestial_transparent,
            );
        }
    }

    // One-shot debug line so Earth's LOD params are
    // inspectable in the console/debug overlay.
    if b.id == "earth" {
        static LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            humanity_core::debug::push_debug(format!(
                "Earth: dist={:.0}km, disc={:.0}px, LOD level {} ({} faces), procedural={}",
                dist / 1000.0,
                px,
                level,
                humanity_terrain::icosphere::Icosphere::face_count_at_level(level),
                def.is_some()
            ));
        }
    }
}
