//! Entering the 3D world: `load_world` builds the player's home on its plot with what its start
//! carries, and the star sky, planets, sun and home station.
//! A start can carry livestock, wild animals, decoration plants and the solar-system hologram;
//! `refresh_home_content` builds those again when the home changes under a running world.
use glam::Vec3;
use std::time::Instant;
use humanity_core::ecs::components::Controllable;
use crate::engine::home_meshes::{apply_homestead_meshes, home_lights, rebuild_connection_objects, rebuild_door_panels, rebuild_hull};
use crate::engine::home_spawn::spawn_home_air_space;
use crate::engine::net_route::reload_planet_defs;
use crate::engine::registries::load_data_registries;
use crate::engine::state::EngineState;
use crate::engine::world_sim::{spawn_herd, spawn_wilds};
use crate::renderer::mesh::Mesh;
use humanity_ship::starts::HomeContent;

/// Place a blockman avatar standing on a podium at `base` (the podium floor position),
/// built from the player's `Appearance` (v0.440). A rudimentary humanoid from boxes +
/// a head sphere on a podium cylinder, drawn via the static placeholder path (cleared +
/// re-added each load, so no duplication). The face/limbs use skin tone; later
/// increments swap this for a skinned mesh with cosmetic slots.
pub(crate) fn place_avatar(
    state: &mut EngineState,
    base: Vec3,
    app: &humanity_core::ecs::components::Appearance,
    colors: &humanity_core::cosmetics::OutfitColors,
) {
    // Since character animation rung 1 (2026-11-08) the showroom figure is the skinned body, drawn
    // every frame by engine/animation.rs with the look and outfit applied; only the podium is placed
    // here. The boxes below stand in when the character data did not load.
    crate::engine::animation::ensure_loaded(state);
    if state.animation.draws_bodies() {
        let gpu = gpu!(state);
        let pm = gpu.renderer.add_mesh(Mesh::cylinder_capped(&gpu.renderer.device, 0.5, 0.15, 24));
        let pmat = gpu!(state).renderer.add_material_typed([0.45, 0.47, 0.50, 1.0], 0.3, 0.5, 0.0);
        state.placeholder_objects.push((pm, pmat, base));
        return;
    }
    let s = app.height_scale.clamp(0.5, 2.0);
    let skin = [app.skin_tone[0], app.skin_tone[1], app.skin_tone[2], 1.0];
    let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
    // Equipped cosmetics tint the matching slot; otherwise default body colors.
    let hair = colors.head.map(rgba).unwrap_or([app.hair_color[0], app.hair_color[1], app.hair_color[2], 1.0]);
    let shirt = colors.chest.map(rgba).unwrap_or([0.28, 0.38, 0.58, 1.0]);
    let pants = colors.legs.map(rgba).unwrap_or([0.22, 0.22, 0.28, 1.0]);
    let podium = [0.45, 0.47, 0.50, 1.0];
    // (w, h, d, color, x, y, z) box parts; y/positions scale with height.
    let podium_h = 0.15_f32;
    let leg_h = 0.85 * s;
    let torso_h = 0.62 * s;
    let head_r = 0.14 * s;
    let leg_base = podium_h;
    let torso_base = leg_base + leg_h;
    let head_cy = torso_base + torso_h + head_r;
    // Helper: push a box part at base + (x,y,z).
    let push_box = |st: &mut EngineState, w: f32, h: f32, d: f32, c: [f32; 4], x: f32, y: f32, z: f32| {
        let gpu = gpu!(st);
        let mi = gpu.renderer.add_mesh(Mesh::box_xyz(&gpu.renderer.device, w, h, d));
        let mat = gpu!(st).renderer.add_material_typed(c, 0.1, 0.75, 0.0);
        st.placeholder_objects.push((mi, mat, base + Vec3::new(x, y, z)));
    };
    // Legs (pants), torso (shirt), arms (skin), at the body's standing pose.
    push_box(state, 0.16, leg_h, 0.22, pants, -0.12, leg_base, 0.0);
    push_box(state, 0.16, leg_h, 0.22, pants, 0.12, leg_base, 0.0);
    push_box(state, 0.46, torso_h, 0.26, shirt, 0.0, torso_base, 0.0);
    push_box(state, 0.13, torso_h, 0.16, skin, -0.30, torso_base, 0.0);
    push_box(state, 0.13, torso_h, 0.16, skin, 0.30, torso_base, 0.0);
    // Hair cap (a thin box sitting on the head).
    push_box(state, 0.30, 0.10 * s, 0.30, hair, 0.0, head_cy + head_r * 0.4, 0.0);
    // Podium cylinder (capped so the top is a visible disc, not an open tube).
    let gpu = gpu!(state);
    let pm = gpu.renderer.add_mesh(Mesh::cylinder_capped(&gpu.renderer.device, 0.5, podium_h, 24));
    let pmat = gpu!(state).renderer.add_material_typed(podium, 0.3, 0.5, 0.0);
    state.placeholder_objects.push((pm, pmat, base));
    // Head sphere (center-origin; place its center directly).
    let gpu = gpu!(state);
    let hm = gpu.renderer.add_mesh(Mesh::sphere(&gpu.renderer.device, head_r, 12, 14));
    let hmat = gpu!(state).renderer.add_material_typed(skin, 0.1, 0.7, 0.0);
    state.placeholder_objects.push((hm, hmat, base + Vec3::new(0.0, head_cy, 0.0)));
}

/// Build the star sky NOW, synchronously, with the CURRENT settings and in
/// the renderer's current scene format. World entries after the first use it
/// (character switch, tier change), and so does the HDR scene target's A/B
/// switch (showcase `scene_format`, engine/ipc.rs), because the sky's
/// pipelines are built for the format the scene target had when it was made.
pub(crate) fn build_star_sky(state: &mut EngineState) {
    let star_tier_cap = crate::renderer::stars::StarCatalogTier::resolve_cap(
        &state.gui_state.settings.star_catalog_tier,
    );
    let star_catalog =
        crate::renderer::stars::StarCatalog::load(&state.data_dir, star_tier_cap);
    state.star_renderer = star_catalog.as_ref().and_then(|catalog| {
        let gpu = gpu!(state);
        crate::renderer::stars::StarRenderer::new(
            &gpu.renderer.device,
            &gpu.renderer.queue,
            gpu.renderer.scene_format(),
            catalog,
            &state.data_dir,
            // Ultra Milky Way glow tier (2026-07-11): built here with the
            // rest of the sky, so a tier change applies next world entry
            // (same convention as the star catalog tiers). Also gated by the
            // star-tier cap so the dev fast path (cap < 2) ALSO drops the
            // heavy Ultra glow texture, per the operator's "the mega skybox
            // thing AND the other versions".
            state.gui_state.settings.sky_glow_tier == "ultra" && star_tier_cap >= 2,
        )
    });
}

/// Whether this world load has the view's GPU to upload to (the headless sim runner, increment 4).
/// Without one (a headless run, `EngineState::boot(.., None)`) the piece of drawing is skipped and
/// named in `state.gpu_dirty`, so a headless report can say what was not drawn: the simulation
/// the world load builds (world_sim.rs, the doors, the station's orbit, the planets' definitions,
/// the character) is built the same either way.
fn drawing(state: &mut EngineState, what: &'static str) -> bool {
    if state.gpu.is_some() {
        return true;
    }
    if !state.gpu_dirty.contains(&what) {
        state.gpu_dirty.push(what);
    }
    false
}

pub(crate) fn load_world(state: &mut EngineState) {
    log::info!("Loading 3D world...");
    let load_start = Instant::now();
    // A pages-only start (HUMANITY_PAGES_FIRST) builds the world's renderer here, the first time
    // the world is entered; a desktop built it at start, so this is a no-op (engine/renderer_slot.rs).
    if let Some(gpu) = state.gpu.as_mut() {
        gpu.renderer.ensure_world("entering the world");
    }
    // A ship loaded afresh may be the server's ship again: try joining it, and the next welcome
    // is an arrival (the player stands where the relay holds them). engine/home_plot.rs.
    crate::engine::home_plot::clear_refusal(state);
    state.home_arrived_on = None;

    // ── The world's simulation ── (engine/world_sim.rs, the headless sim runner's increment 1):
    // the ship assembled with this player's home on its plot, the save's pieces carried to it, the
    // walls, whose air each place breathes, where the player arrives, the household's machines as
    // live entities, the herd, the wilds and the home's air space, built without the GPU. What
    // follows uploads and draws it, in the order it ran when the two were interleaved here.
    let crate::engine::world_sim::WorldSim {
        homestead,
        room_info,
        boot_plot,
        fallback_layout,
        wall_colliders,
        sight_colliders,
        homestead_bounds,
        ship_air,
        aboard_bounds,
        hologram_room_center,
        spawn,
        machines,
        content,
    } = crate::engine::world_sim::build(crate::engine::world_sim::Parts {
        data_dir: &state.data_dir,
        world: &mut state.game_world.world,
        data_store: &mut state.data_store,
        gui: &mut state.gui_state,
    });
    state.boot_plot = boot_plot;
    if let Some(layout) = fallback_layout {
        state.homestead_layout = Some(layout);
    }
    state.wall_colliders = wall_colliders;
    state.sight_colliders = sight_colliders;

    // ── Homestead meshes ── the home's geometry, generated by the sim side as data, uploaded.
    if drawing(state, "the home's meshes, its rooms' light probes and the hull") {
        apply_homestead_meshes(state, homestead);
        // Room GI: each room's irradiance probes (engine/room_gi.rs).
        crate::engine::room_gi::rooms_changed(state, &room_info);
        // The hull wrap (ship-superstructure increment D): the exterior shell around the zone
        // cluster, generated from data/blueprints/hull_profile.ron. Ships with the world load;
        // rebuild_homestead regrows it on every structure edit.
        rebuild_hull(state);
    }

    // Room ceiling lights: the one shared fill (home_meshes.rs `auto_room_lights`), which hangs a
    // room's light under the slab over it (ship homes increment 7). This copy of the arithmetic
    // used to live here too and put the light inside the slab.
    let auto_lights = crate::engine::home_meshes::auto_room_lights(
        state.gui_state.ship_structure.as_ref(),
        room_info.iter().map(|r| (r.center, r.dimensions)),
    );
    // v0.571: placed lights (across ALL zones, v0.754) override the auto synthesis (empty -> auto).
    state.room_lights = home_lights(state.gui_state.ship_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);

    // The box around every room, whose air each place breathes and where aboard ends (built by
    // the sim side, survival_env.rs `ship_spaces`).
    state.homestead_bounds = homestead_bounds;
    state.ship_air = ship_air;
    state.aboard_bounds = aboard_bounds;
    state.hologram_room_center = hologram_room_center.unwrap_or(Vec3::new(-0.5, 1.0, 2.5));
    // Where the player arrives (world_sim.rs: the home's authored point, else its spawn room,
    // else beside the hologram).
    if let Some(s) = spawn {
        state.camera.position = s.position;
        state.camera.pitch = s.pitch;
        state.camera.yaw = s.yaw;
    }

    log::info!("Homestead: {} rooms, {} floors, walls: {}, {} lights",
        room_info.len(), state.homestead_floors.len(),
        state.homestead_walls.is_some(), state.room_lights.len());

    // Clear the per-frame object lists before (re)populating them this load. The old aeroponic
    // tower placeholders (a v0.383 pre-machine-system demo: tower_configs grey cylinders + helix
    // plant-marker spheres) were REMOVED in v0.529 -- the home.ron machine arrays (the
    // aeroponic_tower_* types) now render the real garden towers, which move + delete with the
    // room. The static markers did not respond, showing duplicate non-responsive towers with
    // spheres (operator feedback 2026-06-24).
    state.placeholder_objects.clear();
    state.machine_objects.clear();
    state.grow_positions.clear();

    // ── The machines' bodies ── for the machines the sim side placed (world_sim.rs).
    if drawing(state, "the machines' bodies (and the chairs and grow anchors their builder records), the screens, the decorations and the pipes") {
        match &machines {
            Some(placed) => {
                state.gui_state.machine_labels.clear();
                // A new world's chairs stand where they are placed (BUG-298).
                state.interact.chairs.clear();
                // The machines' bodies (each model with its texture, as every later rebuild draws it),
                // their labels, pick volumes and grow anchors, from the builder the editor's rebuild
                // uses, so the first rebuild after this load finds them in step with the placements
                // and only moves them (home_meshes::build_machine_bodies). Then the in-world screens
                // (rung 2): a page surface + display quad for every placed machine whose def carries a
                // `screen`, built here on the initial load as well as in rebuild_machine_objects, or a
                // fresh boot would show blank bodies until the first edit.
                let placements = crate::engine::home_meshes::placements_now(state).unwrap_or_default();
                crate::engine::home_meshes::build_machine_bodies(state, &placements);
                crate::engine::screens::sync_screens(state, &placements);
                // And the mirrors' glass (2026-11-21, engine/mirrors.rs).
                crate::engine::mirrors::sync(state, &placements);

                // ── Decoration plants (v0.909) ── only for a home whose start carries them
                // (`place_decorations`).
                place_decorations(state, &placed.world_pos, content.contains(&HomeContent::Decorations));
            }
            None => {
                // No machines this load: the bodies' slots from the last go to the home's spares.
                crate::engine::home_meshes::take_machine_slots(state, 0);
            }
        }
        // The grow machines' plots, lights and plot areas (2026-09-26). This load
        // path records the grow anchors itself, so it has to publish them too:
        // only `rebuild_machine_objects` did, which a fresh boot never runs, so
        // until the first machine edit every bed, tray and field plot counted as
        // one plant to the farm and drew as one plant.
        crate::engine::home_meshes::publish_grow_plots(state);

        // Build the live connection cylinders (replaces the old static routed pipes). (v0.530)
        rebuild_connection_objects(state);
    }
    // Build the door/window panels from the home structure's openings. (v0.537)
    rebuild_door_panels(state);

    // ── Solar system hologram (map-sync increment C) ── only for a home whose start carries it
    // (`build_hologram`): a cabin has no hologram room, and it fell back to a point on the ship.
    if drawing(state, "the hologram") {
        clear_hologram(state);
        if content.contains(&HomeContent::Hologram) {
            build_hologram(state);
        }
    }

    // ── Star skybox ──
    // The catalog is parsed ONCE here (stars.bin, ~1.8 MB binary; CSV
    // fallback) and shared by the skybox vertex builder and the
    // constellation resolver inside StarRenderer::new. Pre-v0.797 both
    // consumers re-read and re-parsed the 34 MB stars.csv separately.
    // Star-catalog tier CEILING (dev fast path): resolve from the saved
    // setting, with HUMANITY_STAR_TIER overriding it, so a scripted/verify
    // boot can force the shipped 120k catalog (cap 0) and skip the ~350 MB
    // Ultra read entirely. Default "auto" => cap 2 => biggest installed
    // wins (unchanged for players).
    let t_sky = Instant::now();
    if drawing(state, "the star sky") {
        if let Some(rx) = state.star_preload_rx.take() {
            // First entry: the boot-time background thread (see resumed())
            // built the sky already - or is about to finish; recv() blocks at
            // most as long as the old synchronous path took. Boot-time
            // settings apply, matching the "tier changes apply next world
            // entry" convention.
            state.star_renderer = rx.recv().ok().flatten();
        } else {
            // Later entries (character switch / tier change): synchronous
            // rebuild with the CURRENT settings, exactly as before.
            build_star_sky(state);
        }
    }
    state.boot_timer.since("star_catalog_and_glow", t_sky);

    // ── Planets (procedural fractal surfaces, v0.763) ──
    // One shared vertex-color material (shader type 12) renders every
    // procedural planet surface; the per-face colors ride in the mesh
    // itself (packed in the UV channel), so no per-planet material is
    // needed for the ground. Meshes are generated lazily per (body,
    // LOD level) in the per-frame sky loop and cached in
    // state.planet_mesh_cache.
    if drawing(state, "the planets' surface material") {
        state.planet_surface_material =
            gpu!(state).renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 12.0, 0.0);
    }
    // Per-body surface parameters are data files (infinite-of-X): a new
    // planet look = a new data/planets/<body_id>.ron, no code change.
    // Bodies without a def fall back to smooth LOD spheres with the
    // coarse body-type materials.
    //
    // The CPU half (reading the RONs, loading or synthesizing each elevation
    // grid, loading each colour grid, baking each surface texture: about
    // 1.2 s measured 2026-10-18, 1.9 s once the boot no longer waited for the
    // cloud PSOs and this ran beside them) is prebuilt on a boot thread like
    // the star sky (`planet_defs_preload_rx`), so the FIRST entry only waits
    // for whatever of it is unfinished and then installs and uploads. Any
    // later entry, or one after a hot reload made the prebuild stale, takes
    // the main-thread path exactly as before. The span keeps its name so the
    // boot-timing history stays comparable; it now measures the wait plus
    // the install, and the prebuild's own time is a BACKGROUND span.
    let t_planets = Instant::now();
    if let Some(rx) = state.planet_defs_preload_rx.take() {
        match rx.recv() {
            Ok((bakes, prebuild)) => {
                crate::engine::net_route::install_planet_defs(state, bakes);
                state.boot_timer.record_background("planet_defs_prebuild", prebuild);
            }
            // The prebuild thread died (a panic, already in run.log): bake
            // here instead, the way every later entry does.
            Err(_) => reload_planet_defs(state),
        }
    } else {
        reload_planet_defs(state);
    }
    state.boot_timer.since("planet_defs_bake", t_planets);

    // ── Home station: load its orbit from data and place the ship frame on it ──
    //
    // This used to be a hardcoded GEO position above Silverdale that never
    // survived a single frame: the frame loop's station block overwrote
    // ship_world_pos immediately with a wall-clock 400 km circle, so the
    // intent in the old comment was dead code for four hundred releases.
    // Now the orbit is data (data/stations/home.ron), it is propagated on the
    // GAME clock, and the same elements are used here and every frame after.
    {
        state.gui_state.credits = humanity_core::credits::Credits::load(&state.data_dir);
        let defs = humanity_ship::station::load_all(&state.data_dir);
        state.station_def = humanity_ship::station::home(&defs);
        let (pos, vel) = humanity_ship::station::orbit::propagate(
            &state.station_def.orbit,
            humanity_ship::station::orbit::MU_EARTH,
            humanity_ship::station::orbit::REAL_SECONDS_PER_DAY,
            0.0,
            humanity_systems::time::EARTH_DAY_S,
        );
        state.station_world_pos = pos;
        state.station_world_rot =
            humanity_ship::station::orbit::attitude(&state.station_def.attitude, pos, vel);
        state.ship_world_pos = pos;
        // The longitude the home hangs over, so its panels, grow lights and
        // crops see the sun the deck sees and the HUD reads the deck's time
        // (BUG-090). A home on any other orbit has no fixed place below it
        // and keeps the game clock (longitude 0).
        if let Some(lon) = humanity_ship::station::orbit::hang_longitude_deg(
            &state.station_def.orbit,
            humanity_systems::time::EARTH_DAY_S,
        ) {
            state.data_store.insert(humanity_systems::time::HOME_LONGITUDE_KEY, lon);
        }
        log::info!(
            "station: {} on a {:.0} km orbit of {}, {:?}",
            state.station_def.name,
            pos.length() / 1000.0,
            state.station_def.parent,
            state.station_def.attitude,
        );
    }

    // ── Sun setup ──
    // Sun world position: 1 AU from Earth, placed along the existing
    // shader sun_direction vector so the visible Sun disc matches where
    // the world is being lit from. sun_direction uniform is
    // [0.3, 1.0, 0.5] (see renderer/mod.rs:205) — the Sun sits along
    // that ray at 1 AU (149.6 million km).
    let sun_dir = glam::DVec3::new(0.3, 1.0, 0.5).normalize();
    const ONE_AU_M: f64 = 149_597_870_700.0;
    state.sun_world_pos = sun_dir * ONE_AU_M;
    // The Sun (type 17, 2026-10-21, renderer/sun_disc.rs): its disc at its
    // true size and real radiance, limb-darkened, with Baumbach's corona,
    // all from the view ray in the shader. base_color.rgb is a tint the
    // shader multiplies in, white: nothing from the player's state may dim
    // the disc (the air in front of it is the shell's, per pixel). No
    // intensity here, the light is the Sun's own. (This replaced v0.886-v0.887's
    // radial glow at intensity 30 and its 2.2 "corona halo" three times
    // wider, both on a disc floored at ten times the Sun's width.)
    if drawing(state, "the Sun's material") {
        state.sun_material = gpu!(state).renderer.add_material_full(
            [1.0, 1.0, 1.0, 1.0],
            0.0,
            1.0,
            17.0,
            0.0,
        );
    }

    // ── Real solar-system body materials (map sync, increment B) ──
    // Four simple PBR materials picked by SolBody.body_type so Mars
    // doesn't look like Earth. Not photoreal — that's a later pass;
    // the point of B is that the FPS sky IS the Maps page (real
    // bodies, real positions, real scale) instead of one lone
    // sphere. Colors are coarse real-imagery approximations.
    if drawing(state, "the bodies' materials") {
        state.solar_body_materials = [
            gpu!(state).renderer.add_material([0.62, 0.52, 0.42, 1.0], 0.0, 0.85), // rocky/terrestrial — tan-grey
            gpu!(state).renderer.add_material([0.80, 0.66, 0.46, 1.0], 0.0, 0.55), // gas giant — banded ochre (fallback)
            gpu!(state).renderer.add_material([0.72, 0.82, 0.92, 1.0], 0.0, 0.40), // icy / dwarf — pale blue-white
            gpu!(state).renderer.add_material([0.55, 0.55, 0.58, 1.0], 0.0, 0.80), // default — grey
            // v0.905: per-giant type-18 procedural band materials (params.w
            // selects the palette in the shader).
            gpu!(state).renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 0.0), // jupiter
            gpu!(state).renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 1.0), // saturn
            gpu!(state).renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 2.0), // uranus
            gpu!(state).renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 3.0), // neptune
        ];
    }

    // ── Orbit paths (v0.262.20 — thin world-space lines) ──
    // Was thick tube meshes (operator: "tubes are just too thick …
    // we wouldn't need all the verts … like a single edge"). Now we
    // just cache each body's TRUE Keplerian ellipse points
    // (humanity_core::cosmos::sample_orbit_points → same math the Maps page
    // draws) in PARENT-frame metres. Per frame they're offset to the
    // parent's Earth-relative position and drawn as a 1-px LineList
    // that the depth buffer occludes behind planets. 96 samples is
    // plenty smooth for an ellipse and a fraction of the tube verts.
    //
    // Each ring is the orbit as it lies NOW (the same wall-clock seconds since
    // J2000 the frame places the bodies by): the Moon's orbit turns, its node a
    // degree every 19 days, so a J2000 ring would miss the Moon by up to 10
    // degrees of latitude. Cached once per world entry; a session moves it a
    // few hundredths of a degree.
    state.solar_orbit_paths = solar_orbit_paths_now();
    state.solar_orbit_paths_at = Some(std::time::Instant::now());
    log::info!("Map-sync: cached {} FPS orbit paths (thin lines)", state.solar_orbit_paths.len());

    // ── Load CSV game-data registries into the runtime DataStore ──
    // Each registry is built from its data file and inserted under the key its
    // owning system reads (item_registry / recipe_registry / plant_registry).
    // Graceful per-registry: a missing or malformed file logs a warning and
    // skips (the system then runs on safe defaults), never panics. Reads from
    // the on-disk data dir so edits/mods to the CSV take effect. Mirrors the
    // container_registry wiring below.
    //
    // BEFORE v0.323 these three were loaded then DISCARDED (`let _ =
    // load_csv(...)` into throwaway {id,name} structs), so the runtime
    // DataStore stayed empty and CraftingSystem (no recipes), item
    // name/stack/mass lookups, and FarmingSystem species data all silently
    // no-op'd — the central finding of the 2026-05-29 game-code audit.
    // The registries are loaded EAGERLY at startup (load_data_registries, called
    // from resumed) — see that fn for why. This call re-loads them when the 3D
    // world opens (idempotent), so editing a data file + re-entering picks it up.
    load_data_registries(&mut state.data_store, state.asset_manager.data_dir());

    // ── Player avatar + character-select showroom (v0.440/441) ──
    // Place a blockman avatar on a podium in the respawner (where you wake) and OPEN the
    // showroom: hide the home, orbit the avatar against a backdrop, let the player edit
    // appearance, then "Enter your home" to emerge into first-person. The avatar is the
    // last thing added to placeholder_objects, so `avatar_obj_start` marks where it
    // begins (the showroom renders + rebuilds only this range).
    // Place the avatar + showroom assets at the "respawner" room (legacy
    // fibonacci layout) OR, when that room id does not exist, the room that
    // data/rooms.ron says is the appearance station, OR the spawn room
    // (v0.706 fix). The default HomeStructure home emits room ids "home" /
    // "room_N" with `is_spawn_room` set on the largest room, never
    // "respawner" — so this whole block used to be skipped on EVERY path,
    // leaving avatar_base at Vec3::ZERO. That made a fresh boot look empty
    // (no avatar body) and made the Play/Characters showroom orbit an empty
    // point. Falling back to the spawn room fixes both.
    //
    // The middle step was added 2026-09-19. The avatar is DRAWN in the world,
    // not only in the showroom, so with the acre partitioned the spawn-room
    // fallback (the largest room) would have stood a blockman on a podium in
    // the middle of the greenhouse. A figure on a podium is a dress form, so
    // it belongs in the room where clothes are changed, and failing that where
    // appearance is edited. Both are read from the room's ACTIONS in
    // data/rooms.ron rather than from a room id, so any authored home puts it
    // in the right place by naming a room type.
    let appearance_room = {
        let reg = humanity_ship::room_types::RoomTypeRegistry::load(&state.data_dir);
        let opens = |r: &humanity_ship::fibonacci::RoomInfo, page: &str| {
            reg.action_pages(r).iter().any(|p| p == page)
        };
        room_info
            .iter()
            .find(|r| opens(r, "wardrobe"))
            .or_else(|| room_info.iter().find(|r| opens(r, "appearance")))
            .map(|r| r.id.clone())
    };
    if let Some(r) = room_info
        .iter()
        .find(|r| r.id == "respawner")
        .or_else(|| {
            appearance_room
                .as_deref()
                .and_then(|id| room_info.iter().find(|r| r.id == id))
        })
        .or_else(|| room_info.iter().find(|r| r.is_spawn_room))
    {
        let floor = r.center.y - r.dimensions.y * 0.5;
        let base = Vec3::new(r.center.x, floor, r.center.z - 0.35);
        let (cname, app, outfit) = state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Name,
                &humanity_core::ecs::components::Appearance,
                &humanity_core::ecs::components::Outfit,
                &Controllable,
            )>()
            .iter()
            .next()
            .map(|(_, (n, a, o, _))| (n.0.clone(), a.clone(), o.clone()))
            .unwrap_or_else(|| ("Wanderer".to_string(), Default::default(), Default::default()));
        state.gui_state.character_name = cname;
        state.cosmetics = humanity_core::cosmetics::load_cosmetics(&state.data_dir);
        state.gui_state.cosmetics_list = state
            .cosmetics
            .iter()
            .map(|c| (c.id.clone(), c.name.clone(), c.slot.clone()))
            .collect();
        state.gui_state.appearance = app.clone();
        state.gui_state.outfit = outfit.clone();
        state.avatar_base = base;
        state.fps_spawn = state.camera.position; // the first-person spawn set above
        state.showroom_return_pos = state.camera.position;
        state.avatar_obj_start = state.placeholder_objects.len();
        let colors = humanity_core::cosmetics::resolve_outfit_colors(&outfit, &state.cosmetics);
        if drawing(state, "the avatar") {
            place_avatar(state, base, &app, &colors);
        }

        // Showroom SCENE ASSETS (backdrops, ground disc, body sphere) are loaded on
        // every world-load -- cheap, and needed so the wetroom mirror + bedroom
        // wardrobe can open the showroom later even when Play did not open the picker.
        state.showroom_backdrops = humanity_core::showroom::load_backdrops(&state.data_dir);
        state.gui_state.showroom_backdrop_names =
            state.showroom_backdrops.iter().map(|b| b.name.clone()).collect();
        state.gui_state.showroom_backdrop = 0;
        state.showroom_last_backdrop = usize::MAX;
        if drawing(state, "the showroom's ground and sphere") {
            let gpu = gpu!(state);
            let gmesh = gpu.renderer.add_mesh(Mesh::cylinder(&gpu.renderer.device, 9.0, 0.06, 32));
            let gmat = gpu!(state).renderer.add_material_typed([0.1, 0.1, 0.12, 1.0], 0.1, 0.9, 0.0);
            state.showroom_ground = Some((gmesh, gmat));
            // A planet sphere (radius 30) the avatar stands on for body backdrops (Earth/Mars).
            let gpu = gpu!(state);
            let body = gpu.renderer.add_mesh(Mesh::sphere(&gpu.renderer.device, 30.0, 24, 32));
            state.showroom_body = Some(body);
        }
        state.gui_state.appearance_dirty = false;
        state.gui_state.showroom_confirm = false;

        // load_world NO LONGER opens the character-select showroom (v0.476).
        // It just spawns you in first-person at the respawner. The unified
        // character picker is opened OPT-IN by the Play button, via the
        // per-frame open_showroom(0) call that runs right AFTER this load when
        // launcher_open_select is set. Because load_world only runs once (the
        // world_loaded guard) AND Esc enters the world without that flag, the
        // old picker (the "Wanderer" duplicate the operator hit on Esc) never
        // appears on Esc, and Play opens the picker every time, not just the
        // first. This is THE root-cause fix for the duplicate character-select.
        state.gui_state.showroom_active = false;
        state.controller.showroom_lock = false;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        state.camera.position = state.fps_spawn;
    }

    state.world_loaded = true;
    // Orbital home (v0.881): the spawn room is aboard the station by
    // definition - snap the player frame onto the orbit next frame.
    state.station_spawn_snap = true;
    log::info!("3D world loaded in {:.0}ms", load_start.elapsed().as_millis());
    // Boot-timing summary (dev tooling): fires exactly once (load_world is
    // only entered when !world_loaded). Records the whole world-load span,
    // then logs the per-phase breakdown + drops debug/boot_timing.json.
    state
        .boot_timer
        .record("world_load_total", load_start.elapsed());
    let boot_total = state.boot_timer.boot_start.elapsed();
    state.boot_timer.emit(boot_total);
}

/// ── Decoration plants (v0.909, operator: "get the plants
/// added and the environment decorated") ── photoscanned CC0
/// Poly Haven models scattered around homestead anchors from
/// data/entities/decorations.ron. Meshes/materials cache
/// across world reloads; positions rebuild each load.
///
/// Only for a home whose start carries them (`carries`, ship homes increment 6); the previous
/// ones go either way.
pub(crate) fn place_decorations(state: &mut EngineState, machine_world_pos: &std::collections::HashMap<String, Vec3>, carries: bool) {
    #[derive(serde::Deserialize)]
    struct DecoEntry {
        model: String,
        near: String,
        #[serde(default)]
        offset: (f32, f32, f32),
        count: u32,
        spread: f32,
        #[serde(default = "one_f32")]
        scale: f32,
    }
    #[derive(serde::Deserialize)]
    struct DecoList {
        decorations: Vec<DecoEntry>,
    }
    fn one_f32() -> f32 {
        1.0
    }
    state.decoration_objects.clear();
    if !carries {
        return;
    }
    let path = state.asset_manager.data_dir().join("entities/decorations.ron");
    let parsed = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| ron::from_str::<DecoList>(&t).map_err(|e| e.to_string()));
    match parsed {
        Ok(list) => {
            let mut placed = 0u32;
            for (di, d) in list.decorations.iter().enumerate() {
                let Some(&anchor) = machine_world_pos.get(&d.near) else {
                    log::warn!("decorations.ron: anchor {} not placed", d.near);
                    continue;
                };
                let cached = state.decoration_mesh_cache.get(&d.model).copied();
                let (mesh_idx, mat_idx) = match cached {
                    Some(mm) => mm,
                    None => {
                        // Variant file lives in its base model's
                        // folder: grass_medium_02_v1 ->
                        // plants/grass_medium_02/grass_medium_02_v1.gltf.
                        let base = d
                            .model
                            .rfind("_v")
                            .map(|i| &d.model[..i])
                            .unwrap_or(d.model.as_str());
                        let rel = format!(
                            "assets/models/plants/{}/{}.gltf",
                            base, d.model
                        );
                        match state
                            .asset_manager
                            .parse_gltf_mesh_textured(&gpu!(state).renderer.device, &rel)
                        {
                            Ok((mesh, tex)) => {
                                let mesh_idx = gpu!(state).renderer.add_mesh(mesh);
                                let mat_idx = match tex {
                                    Some((rgba, w, h)) => {
                                        // Type 19: textured mesh
                                        // (albedo texture + alpha
                                        // cutout + standard sun-lit
                                        // shading).
                                        gpu!(state).renderer.add_textured_material(
                                            [1.0, 1.0, 1.0, 1.0],
                                            0.0,
                                            0.9,
                                            19.0,
                                            0.0,
                                            &rgba,
                                            w,
                                            h,
                                        )
                                    }
                                    None => gpu!(state).renderer.add_material_full(
                                        [0.35, 0.5, 0.3, 1.0],
                                        0.0,
                                        0.9,
                                        0.0,
                                        0.0,
                                    ),
                                };
                                state
                                    .decoration_mesh_cache
                                    .insert(d.model.clone(), (mesh_idx, mat_idx));
                                (mesh_idx, mat_idx)
                            }
                            Err(e) => {
                                log::warn!(
                                    "decorations.ron: {} failed to load: {e}",
                                    d.model
                                );
                                continue;
                            }
                        }
                    }
                };
                for i in 0..d.count {
                    // Golden-angle scatter with deterministic
                    // yaw/scale jitter: natural-looking, stable
                    // across reloads.
                    let a = i as f32 * 2.399_963 + di as f32 * 1.3;
                    let r = d.spread
                        * (0.3 + 0.7 * ((i as f32 + 0.7) / d.count.max(1) as f32));
                    let pos = anchor
                        + Vec3::new(
                            d.offset.0 + a.cos() * r,
                            d.offset.1,
                            d.offset.2 + a.sin() * r,
                        );
                    let yaw = (i as f32 * 73.13 + di as f32 * 31.7) % 360.0;
                    let scl = d.scale
                        * (0.85
                            + 0.3
                                * (((i * 37 + di as u32 * 11) % 100) as f32
                                    / 100.0));
                    state
                        .decoration_objects
                        .push((mesh_idx, mat_idx, pos, yaw, scl, anchor));
                    placed += 1;
                }
            }
            log::info!(
                "Loaded {placed} decoration plants from entities/decorations.ron"
            );
        }
        Err(e) => log::warn!("decorations.ron not loaded: {e}"),
    }
}

/// ── Solar system hologram (map-sync increment C, v0.262.13) ──
/// Driven by the CANONICAL humanity_core::cosmos model at the live date,
/// so the tabletop matches the Maps page + the FPS sky exactly.
/// Was the drifted solar_system.ron placed at fake golden angles
/// (operator: "isn't working — still showing an old
/// placeholder"). Sun is the room centre; bodies sit at their
/// REAL ecliptic longitude (orbit radii still log-compressed to
/// fit the room — true AU ratios can't show indoors).
pub(crate) fn build_hologram(state: &mut EngineState) {
    let sim_t_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0; // Unix secs at the J2000.0 epoch
    let hologram =
        crate::renderer::hologram::generate_hologram_from_cosmos(sim_t_now);

    let orbit_mat = gpu!(state).renderer.add_material([0.3, 0.7, 0.9, 0.8], 0.0, 0.3);
    let ring_disc_mat = gpu!(state).renderer.add_material([0.8, 0.7, 0.5, 0.6], 0.0, 0.4);
    let mut orbit_radii_used: Vec<f32> = Vec::new();

    for body in &hologram.bodies {
        if body.radius <= 0.0 { continue; }

        let stacks = if body.radius > 0.05 { 16 } else { 8 };
        let slices = if body.radius > 0.05 { 24 } else { 12 };
        let gpu = gpu!(state);
        let mesh_idx = gpu.renderer.add_mesh(
            crate::renderer::hologram::sphere_mesh(&gpu.renderer.device, body.radius, stacks, slices)
        );
        let (metallic, roughness, emissive) = if body.body_type == crate::renderer::hologram::BodyType::Star {
            (0.0, 0.2, 5.0) // Stars glow bright
        } else {
            (0.3, 0.5, 0.0)
        };
        let mat_idx = gpu!(state).renderer.add_material_full(body.color, metallic, roughness, 0.0, emissive);
        state.hologram_objects.push((mesh_idx, mat_idx, body.local_position, body.name.clone()));

        if body.orbit_radius > 0.01
            && body.parent.as_deref() == Some("Sun")
            && !orbit_radii_used.iter().any(|&r| (r - body.orbit_radius).abs() < 0.01)
        {
            let gpu = gpu!(state);
            let ring_mesh_idx = gpu.renderer.add_mesh(
                crate::renderer::hologram::orbit_ring_mesh(&gpu.renderer.device, body.orbit_radius, 128)
            );
            state.hologram_orbits.push((ring_mesh_idx, orbit_mat));
            orbit_radii_used.push(body.orbit_radius);
        }

        if body.has_rings && body.body_type == crate::renderer::hologram::BodyType::Planet {
            let inner_r = body.radius * 1.3;
            let outer_r = body.radius * 2.2;
            let gpu = gpu!(state);
            let disc_mesh = gpu.renderer.add_mesh(
                crate::renderer::hologram::ring_disc_mesh(&gpu.renderer.device, inner_r, outer_r, 32)
            );
            state.hologram_objects.push((disc_mesh, ring_disc_mat, body.local_position, format!("{} Rings", body.name)));
        }

        if body.body_type == crate::renderer::hologram::BodyType::Planet
            || body.body_type == crate::renderer::hologram::BodyType::DwarfPlanet
        {
            let gpu = gpu!(state);
            let pin_mesh_idx = gpu.renderer.add_mesh(
                crate::renderer::hologram::pin_marker_mesh(&gpu.renderer.device, 0.03, 0.12)
            );
            let pin_mat = gpu!(state).renderer.add_material(body.color, 0.0, 0.5);
            let pin_offset = Vec3::new(0.0, body.radius + 0.13, 0.0);
            state.hologram_pins.push((pin_mesh_idx, pin_mat, body.local_position + pin_offset, body.name.clone()));
        }
    }

    // ── GROUND-TRUTH INSTRUMENTATION (v0.262.24) ──
    // Operator: "the in-home map never updated, whatever you do
    // doesn't affect it" across many builds, while the skybox DID
    // update. Logic says this block runs and is correct; reality
    // disagrees. So stop reasoning — make the next run conclusive.
    //
    // 1) Log exactly what generate_hologram_from_cosmos produced.
    // 2) Spawn an UNCONDITIONAL bright-MAGENTA proof beacon at the
    //    orrery centre. If it is ABSENT in the operator's run, THIS
    //    code is not executing in their binary (a build/launch path
    //    issue) — a totally different root cause. If it is PRESENT
    //    but bodies look old, the generator output is the problem.
    // 3) Then the green HOME beacon, with a tolerant Earth lookup
    //    and a RED fallback at centre if Earth is somehow missing,
    //    so a silent failure becomes a visible one.
    {
        let names: Vec<&str> =
            hologram.bodies.iter().map(|b| b.name.as_str()).take(40).collect();
        let earth_idx = hologram
            .bodies
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case("earth"));
        log::info!(
            "ORRERY-DIAG: generate_hologram_from_cosmos -> {} bodies; earth_at={:?}; names={:?}",
            hologram.bodies.len(),
            earth_idx,
            names
        );

        // Magenta proof beacon removed in v0.262.26 — it confirmed
        // the orrery path executes + updates (operator saw it), so
        // the "in-home map never changes" was a misperception (the
        // rings are circles by nature; the cosmos model DOES drive
        // it). Keeping only the clean green HOME marker + the diag
        // log.

        // Green HOME beacon at Earth (tolerant lookup); RED
        // fallback at centre if Earth is missing from the model.
        let earth = hologram
            .bodies
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case("earth"));
        let (anchor, blip_col, pin_col, label) = match earth {
            Some(e) => (
                e.local_position + Vec3::new(0.0, e.radius, 0.0),
                [0.15, 1.0, 0.45, 1.0],
                [0.15, 1.0, 0.45, 1.0],
                "HOME",
            ),
            None => {
                log::error!("ORRERY-DIAG: NO 'earth' body in hologram — RED fallback");
                (Vec3::new(0.0, 0.10, 0.0), [1.0, 0.1, 0.1, 1.0], [1.0, 0.1, 0.1, 1.0], "HOME?")
            }
        };
        let gpu = gpu!(state);
        let blip_mesh = gpu.renderer.add_mesh(
            crate::renderer::hologram::sphere_mesh(&gpu.renderer.device, 0.045, 10, 14),
        );
        let blip_mat =
            gpu!(state).renderer.add_material_full(blip_col, 0.0, 0.3, 0.0, 8.0);
        state.hologram_objects.push((
            blip_mesh,
            blip_mat,
            anchor + Vec3::new(0.0, 0.10, 0.0),
            "Home (high Earth orbit)".to_string(),
        ));
        let gpu = gpu!(state);
        let home_pin_mesh = gpu.renderer.add_mesh(
            crate::renderer::hologram::pin_marker_mesh(&gpu.renderer.device, 0.07, 0.75),
        );
        let home_pin_mat =
            gpu!(state).renderer.add_material_full(pin_col, 0.0, 0.4, 0.0, 6.0);
        state.hologram_pins.push((
            home_pin_mesh,
            home_pin_mat,
            anchor + Vec3::new(0.0, 0.95, 0.0),
            label.to_string(),
        ));
        log::info!("ORRERY-DIAG: HOME marker + magenta proof beacon pushed");
    }
}

/// Take the hologram down (before it is built again, or for a home that carries none). The
/// renderer keeps the meshes it had, as every world load always has.
pub(crate) fn clear_hologram(state: &mut EngineState) {
    state.hologram_objects.clear();
    state.hologram_orbits.clear();
    state.hologram_pins.clear();
}

/// Where the hologram stands for a home on a plot whose box starts at `home_min`: the point the
/// world load falls back to with the home on p1 (at the ship's origin), carried to this plot, as a
/// move of the home carries it (engine/home_plot.rs `carry_with_home`). A ship home has no
/// hologram room of its own (`home_structure` marks none), so this is where it has always stood.
pub(crate) fn hologram_spot(home_min: Vec3) -> Vec3 {
    home_min + Vec3::new(-0.5, 1.0, 2.5)
}

/// The live home's world content again, after the home changed under a running world (a change
/// of save, engine/save_slot.rs `build_home`, or a welcome giving a plot of another kind,
/// engine/home_plot.rs): the herd, the wild spawns, the decoration plants and the hologram its
/// start carries (`spawn_herd`, `spawn_wilds`, `place_decorations`, `build_hologram`), anchored to
/// the machines as they stand now, and its own air when it breathes its own (`replace_home_air`).
/// What the start does not carry is taken away: a homestead's herd does not follow a player into
/// a cabin.
pub(crate) fn refresh_home_content(state: &mut EngineState) {
    let start = crate::engine::save_slot::live_start(&state.data_dir);
    let carries = |c: HomeContent| start.home_content.contains(&c);
    let anchors: std::collections::HashMap<String, Vec3> = crate::engine::home_meshes::current_placements(state)
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id, Vec3::new(p.pos.0, p.pos.1, p.pos.2)))
        .collect();
    spawn_herd(&mut state.game_world.world, &state.data_store, &anchors, carries(HomeContent::Herd));
    place_decorations(state, &anchors, carries(HomeContent::Decorations));
    spawn_wilds(&mut state.game_world.world, &state.data_store, state.gui_state.settings.level_value("hostile_wildlife"), carries(HomeContent::WildSpawns));
    if let Some(b) = state.gui_state.ship_structure.as_ref().and_then(crate::engine::home_plot::home_box) {
        state.hologram_room_center = hologram_spot(b.0);
    }
    clear_hologram(state);
    if carries(HomeContent::Hologram) {
        build_hologram(state);
    }
    let kcal = state.gui_state.home_machines.as_ref().map_or(0.0, crate::engine::home_spawn::home_metabolic_kcal);
    let m3 = crate::engine::home_spawn::own_home_air_m3(&state.data_dir, state.gui_state.ship_structure.as_ref());
    replace_home_air(&mut state.game_world.world, start.own_air, kcal, m3);
}

/// The home's own sealed air space (`atmosphere::HomeAir`), made again for the home standing now:
/// the old one goes, and a new one of `volume_m3` comes when the home breathes its own air
/// (`own_air`); a home on the ship's ventilation (a cabin) gets none, and its player breathes the
/// ship's air there (ship/ship_space.rs `air_spaces_for`). Pure on the world.
pub(crate) fn replace_home_air(world: &mut hecs::World, own_air: bool, metabolic_kcal_per_day: f32, volume_m3: f32) {
    let old: Vec<hecs::Entity> = world.query::<&humanity_systems::atmosphere::HomeAir>().iter().map(|(e, _)| e).collect();
    for e in old {
        let _ = world.despawn(e);
    }
    if own_air {
        spawn_home_air_space(world, metabolic_kcal_per_day, volume_m3);
    }
}


/// Every orbiting body's ring as it lies now (`load_world` says why now): ellipse points in
/// parent-frame metres, the parent's id, "planet" or "moon", and the body's id.
pub(crate) fn solar_orbit_paths_now() -> Vec<(Vec<glam::DVec3>, String, String, String)> {
    let mut out = Vec::new();
    let ring_sim_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0; // Unix secs at J2000.0 epoch
    for b in humanity_core::cosmos::sol_bodies() {
        // Cache EVERY orbiting body's ring, tagged planet/moon so the Sky
        // settings filter at draw time (v0.786). Direct sun-orbiters =
        // planets/dwarfs; everything else = a moon around its planet.
        // (Fixes the latent v0.262 bug where the lone intended moon ring
        // never drew: the guard checked id "moon" but the data id is
        // "luna".) Sun has no orbit (sample empty) → skipped naturally.
        let Some(parent) = b.parent.clone() else { continue };
        let kind = if parent == "sun" { "planet" } else { "moon" };
        let pts_au = humanity_core::cosmos::sample_orbit_points(b, 96, ring_sim_t);
        if pts_au.len() < 3 { continue; }
        // Keep the vertices in f64 (v0.791): they are ~1.5e11 m and the
        // per-frame offset is the same magnitude with the opposite sign,
        // so an f32 cache left tens-of-km cancellation jitter near the
        // body -- one of the two reasons Earth sat visibly off its ring.
        let pts_m: Vec<glam::DVec3> = pts_au
            .iter()
            .map(|p| *p * humanity_core::cosmos::M_PER_AU)
            .collect();
        out.push((pts_m, parent, kind.to_string(), b.id.clone()));
    }
    out
}

/// With the reuse switch off, how often the orbit rings are worked out again from the clock,
/// seconds (a schedule rather than every frame: every orbiting body's 96 ellipse points).
pub(crate) const ORBIT_PATHS_REUSE_OFF_S: f32 = 1.0;

/// Are the orbit rings worked out again this frame? Never with reuse on (once per world entry: a
/// session moves them a few hundredths of a degree); with it off, every
/// [`ORBIT_PATHS_REUSE_OFF_S`].
/// reuse: site: the orbit rings are worked out again from the clock every second with the switch
/// off.
pub(crate) fn orbit_paths_due(reuse: bool, since_s: f32) -> bool {
    !reuse && since_s >= ORBIT_PATHS_REUSE_OFF_S
}

/// Once a frame before the rings draw: with the reuse switch off, work them out again on their
/// schedule (`orbit_paths_due`).
pub(crate) fn refresh_orbit_paths(state: &mut EngineState) {
    let since = state.solar_orbit_paths_at.map_or(f32::MAX, |t| t.elapsed().as_secs_f32());
    if orbit_paths_due(humanity_core::reuse::enabled(), since) {
        state.solar_orbit_paths = solar_orbit_paths_now();
        state.solar_orbit_paths_at = Some(std::time::Instant::now());
    }
}

#[cfg(test)]
#[path = "world_load_tests.rs"]
mod home_content_tests;
