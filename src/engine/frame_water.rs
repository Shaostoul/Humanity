//! The planet's WATER SHELLS: the two quadtree layers that make a sea, built
//! and pushed once per frame for whatever body the camera is standing on
//! (extracted verbatim from lib.rs, v0.1320).
//!
//! There are two shells and the pair is the design:
//!
//! - the BACKSTOP, a coarse undisplaced layer ~4.7 m under the surface, drawn
//!   first so the long swells' cross-depth T-junction tears reveal water
//!   colour rather than pale seafloor or sky;
//! - the WAVE SHELL itself, a tighter quadtree at the sea radius whose vertex
//!   shader adds the Gerstner height.
//!
//! WHY IT IS A MODULE: 586 lines in the middle of the celestial draw loop,
//! with one job and a dependency surface small enough to write down (seven
//! `EngineState` fields, the body's `PlanetDef`, and where the camera is).
//! `src/lib.rs` is the repo's merge funnel (CLAUDE.md): the fewer reasons to
//! open it, the more sessions can work in parallel without three-way merges
//! over the frame loop.
//!
//! WHY THE TWO CONTEXT STRUCTS instead of `&mut EngineState`: the caller holds
//! `def` borrowed out of `state.planet_defs` for the whole body iteration, so a
//! whole-state borrow is an E0502 -- the same constraint documented on
//! `engine::near_tree_models::ensure_near_tree_models`. `WaterShellState` names
//! exactly the seven fields this code touches and nothing else, and `WaterView`
//! names the nine frame values it reads. Grouping them is not hiding them: the
//! struct definitions below ARE the dependency list, and the compiler checks
//! them in a way a doc comment would not be.
//!
//! A side benefit worth keeping: because the borrows arrive under the name
//! `state`, and the view is destructured back into the frame's own local
//! names, the body below is byte-identical to the lines it replaced. That is
//! what makes "zero behaviour change" checkable rather than asserted.

use crate::gui::GuiState;
use crate::renderer::camera::Camera;
use crate::renderer::mesh::Mesh;
use crate::renderer::{RenderObject, Renderer};
use crate::terrain::planet::PlanetDef;
use crate::terrain::planet_chunks as chunks;
use glam::{DQuat, DVec3, Quat, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

/// The slice of `EngineState` the water shells touch, borrowed field by field.
///
/// Named `state` at the call site on purpose (see the module header): the
/// moved code keeps reading `state.renderer`, `state.planet_chunk_states` and
/// the rest exactly as it did inline.
pub(crate) struct WaterShellState<'a> {
    /// Materials, meshes and the GPU device the patch meshes upload through.
    pub renderer: &'a mut Renderer,
    /// Patch caches, keyed by a synthetic id per shell ("<body>::water",
    /// "<body>::water_backstop") so the sea's quadtrees live beside the
    /// terrain's instead of colliding with it.
    pub planet_chunk_states: &'a mut HashMap<String, chunks::ChunkState>,
    /// One material per shell, created on first sight and then updated in
    /// place every frame (its base colour carries the planet centre, which
    /// the floating origin moves).
    pub planet_water_materials: &'a mut HashMap<String, usize>,
    /// Bathymetry, read to place the sea floor under each water patch.
    pub planet_heightmaps:
        &'a HashMap<String, Arc<crate::terrain::planet_heightmap::PlanetHeightmap>>,
    /// Mesh slots freed by evicted patches, reused before allocating new ones.
    pub planet_patch_free_slots: &'a mut Vec<usize>,
    /// Read for `surface_mode` (the parked-selection fast path).
    pub camera: &'a Camera,
    /// Read for the Settings water-detail knobs.
    pub gui_state: &'a GuiState,
}

/// Where the camera is and how this planet is oriented, this frame.
///
/// Copied out of the frame loop rather than recomputed: every value here is
/// already derived above the call site for the TERRAIN patches, and the sea
/// has to agree with the ground exactly or the shoreline separates.
#[derive(Clone, Copy)]
pub(crate) struct WaterView<'a> {
    /// Camera position in planet-local (unrotated) metres.
    pub cam_local: DVec3,
    /// Camera forward in the same frame, for the parked-selection test.
    pub cam_fwd_local: DVec3,
    /// The terrain selector's own frustum, already rebased to planet-local.
    pub frustum: &'a chunks::FrustumPlanes,
    /// Planet centre in render space (the floating origin moves it).
    pub render_off: DVec3,
    /// Planet orientation in f64, for placing patch anchors.
    pub rot_d: DQuat,
    /// The same orientation in f32, for the render objects.
    pub rotation: Quat,
    /// Screen-space split threshold, shared with the terrain quadtree.
    pub split_px: f32,
    /// Viewport height in pixels.
    pub viewport_h: f32,
    /// Vertical field of view in degrees. With `viewport_h` this gives the
    /// pixels-per-radian the selector measures its error in.
    pub fov_deg: f32,
}

/// Build and push this planet's backstop + wave shells into the transparent
/// celestial draw list.
///
/// Called only while terrain patches are actually drawing: below that handoff
/// the uniform sphere's clamped sea IS the water, and a second surface over it
/// would z-fight.
///
/// `b` is the body being drawn (its id keys the caches and materials), `d` its
/// planet definition (radius, terrain seed, sea depth), and `om` the
/// connected-ocean mask that makes bathymetry meaningful in the first place.
pub(crate) fn push_water_shells(
    state: &mut WaterShellState,
    view: WaterView,
    b: &crate::cosmos::SolBody,
    d: &PlanetDef,
    om: &crate::terrain::ocean_mask::OceanMask,
    celestial_transparent: &mut Vec<RenderObject>,
) {
    // Unpack the view under the names the moved code already used, so the body
    // below is the inline block byte for byte.
    let WaterView {
        cam_local,
        cam_fwd_local,
        frustum,
        render_off,
        rot_d,
        rotation,
        split_px,
        viewport_h,
        fov_deg,
    } = view;
    let wmat = match state.planet_water_materials.get(&b.id) {
        Some(&m) => m,
        None => {
            let m = state.renderer.add_material_full(
                [0.0, 0.0, 0.0, 1.0],
                0.0,
                0.05,
                16.0,
                0.0,
            );
            state.planet_water_materials.insert(b.id.clone(), m);
            m
        }
    };
    // The same centre feeds the SEA SPHERE
    // uniform (v0.1061), which is what lets the
    // underwater extinction become a per-ray
    // path integral instead of a screen-wide
    // switch.
    state.renderer.sea_sphere = [
        render_off.x as f32,
        render_off.y as f32,
        render_off.z as f32,
        d.radius as f32,
    ];
    // base_color.xyz = planet center in render
    // space, every frame (floating origin).
    state.renderer.update_material_full(
        wmat,
        [
            render_off.x as f32,
            render_off.y as f32,
            render_off.z as f32,
            1.0,
        ],
        0.0,
        0.05,
        16.0,
        0.0,
    );
    // ── Backstop shell (v0.1019, water arc) ──
    // A COARSE, undisplaced deep-water layer
    // ~4.7 m under the wave shell. Cross-depth
    // T-junction tears in the displaced
    // surface (the long swells sag ~1.2 m
    // across coarse patch edges; they cannot
    // be resolution-faded, they ARE the
    // visible sea) now reveal water-colored
    // backstop instead of pale seafloor or
    // sky. Drawn FIRST (this block precedes
    // the wave shell's pushes), so it always
    // composites behind the real surface -
    // above OR below the waterline (the
    // underwater sort is stable).
    {
        let bmat_key = format!("{}::backstop", b.id);
        let bmat = match state.planet_water_materials.get(&bmat_key) {
            Some(&m) => m,
            None => {
                let m = state.renderer.add_material_full(
                    [0.0, 0.0, 0.0, 1.0],
                    1.0, // metallic slot = the FLAT backstop flag
                    0.05,
                    16.0,
                    0.0,
                );
                state
                    .planet_water_materials
                    .insert(bmat_key.clone(), m);
                m
            }
        };
        state.renderer.update_material_full(
            bmat,
            [
                render_off.x as f32,
                render_off.y as f32,
                render_off.z as f32,
                1.0,
            ],
            1.0,
            0.05,
            16.0,
            0.0,
        );
        let bkey = format!("{}::water_backstop", b.id);
        let bparams = chunks::ChunkParams {
            occluder_r_m: None,
            radius_m: d.radius,
            band: chunks::water_band(d.radius),
            max_depth: 11,
            split_px,
            px_per_rad: viewport_h
                / fov_deg.max(1.0).to_radians(),
            max_leaves: 128,
            max_build_requests: 8,
        };
        let seed = d.terrain_seed;
        let bs = state
            .planet_chunk_states
            .entry(bkey)
            .or_insert_with(|| chunks::ChunkState::new(seed));
        bs.frame += 1;
        let bsel = chunks::select_patches_sticky(
            cam_local,
            Some(&frustum),
            &|id| bs.cache.get(id).map(|e| e.band),
            &bparams,
            Some(&bs.last_drawn),
        );
        bs.last_drawn = bsel.draws.iter().cloned().collect();
        let frame = bs.frame;
        for id in &bsel.draws {
            if let Some(e) = bs.cache.get_mut(id) {
                e.last_used = frame;
            }
        }
        // v0.1051: track the LIVE crest. The
        // backstop only has to sit below the
        // wave troughs; pinning it to a 12 m
        // worst case would park it 12 m down on
        // a glassy day, where the gap between
        // the two shells becomes its own
        // artifact at the waterline.
        let drop_m =
            -(state.renderer.sea_crest_m as f64 + 0.5);
        let mut builds = 0usize;
        for id in &bsel.build_requests {
            if builds >= 4 {
                break;
            }
            if bs.cache.contains_key(id) {
                continue;
            }
            let (mesh, anchor, band, bytes) =
                match chunks::build_water_patch_mesh_at(
                    d,
                    om,
                    state
                        .planet_heightmaps
                        .get(&b.id)
                        .map(|a| a.as_ref()),
                    id,
                    drop_m,
                ) {
                    Some(pm) => (
                        Mesh::from_planet_surface(
                            &state.renderer.device,
                            &pm.mesh,
                        ),
                        pm.anchor,
                        pm.band,
                        chunks::PATCH_MESH_BYTES,
                    ),
                    None => {
                        let c = chunks::patch_corners(id);
                        let a = (c[0] + c[1] + c[2]).normalize()
                            * d.radius;
                        (
                            Mesh::placeholder(
                                &state.renderer.device,
                            ),
                            a,
                            chunks::water_band(d.radius),
                            256,
                        )
                    }
                };
            builds += 1;
            let slot = if let Some(idx) =
                state.planet_patch_free_slots.pop()
            {
                state.renderer.replace_mesh(idx, mesh);
                idx
            } else {
                state.renderer.add_mesh(mesh)
            };
            bs.insert(*id, slot, bytes, anchor, band);
        }
        for id in &bsel.draws {
            if let Some(e) = bs.cache.get(id) {
                let anchor_render =
                    render_off + rot_d * e.anchor;
                celestial_transparent.push(RenderObject {
                    fade: 0.0,
                    position: Vec3::new(
                        anchor_render.x as f32,
                        anchor_render.y as f32,
                        anchor_render.z as f32,
                    ),
                    rotation,
                    scale: Vec3::ONE,
                    mesh: e.mesh,
                    material: bmat,
                });
            }
        }
    }

    let wkey = format!("{}::water", b.id);
    let wparams = chunks::ChunkParams {
        // TIGHT HORIZON OCCLUDER for water
        // (v0.1049). water_band subtracts
        // SKIRT_MAX_M (80 km) though the water
        // shell emits no skirts, so the cull's
        // occluder sat 80 km below the seabed
        // and every water patch out to ~2020 km
        // of arc counted as visible - roughly
        // 40% of the 1024-leaf budget refining
        // ocean beyond the horizon, which is
        // WHY the visible far field got such a
        // coarse cut.
        //
        // The margin RAMPS with altitude rather
        // than switching. It cannot simply be
        // 4.1 m always: horizon_culled bails
        // (culls nothing) once the camera sits
        // below the occluder, so a sea-level
        // occluder flips the entire cull on and
        // off as the player BOBS on the waves.
        // A hard switch is no better - measured
        // at the threshold it drew a DEPTH-0
        // leaf (a whole icosahedral face of
        // ocean) for a frame, because the step
        // invalidated the tree faster than the
        // 24-builds/frame budget could descend
        // it. Ramping keeps the horizon sweeping
        // in continuously, which is just camera
        // motion as far as the selector is
        // concerned. Below 3.6 m (the 3.1 m wave
        // ceiling plus the backstop drop) the
        // margin is exactly today's, so the
        // waterline case cannot churn.
        occluder_r_m: {
            let alt = (cam_local.length() - d.radius) as f32;
            let t = ((alt - 3.6) / 16.4).clamp(0.0, 1.0);
            let ramp = t * t * (3.0 - 2.0 * t);
            let margin = 4.1 + 80_000.0 * (1.0 - ramp as f64);
            Some(d.radius - margin)
        },
        radius_m: d.radius,
        band: chunks::water_band(d.radius),
        // Per-type LOD control (v0.965): the
        // Settings water-detail slider owns
        // the near-field depth cap live;
        // WATER_MAX_PATCH_DEPTH is its
        // ceiling + the default.
        max_depth: (state
            .gui_state
            .settings
            .water_detail_depth
            .round()
            .clamp(14.0, chunks::WATER_MAX_PATCH_DEPTH as f32))
            as u8,
        // WATER GETS ITS OWN ERROR FLOOR
        // (v0.1048). The terrain slider is
        // the pixel-error target, and leaves
        // wanted scale as 1/px^2 - so the
        // operator's terrain_split_px = 2
        // asked the WATER shell for ~4x the
        // patches that split_px = 4 does,
        // against a leaf budget that is a
        // small fraction of terrain's. The
        // selection then cut at a huge error
        // and could not cover the sea, and
        // the flat backstop showed through as
        // the operator's pale "blue
        // triangles" - present at 13 m (lots
        // of visible ocean), absent at 3 m
        // (little), which is exactly the
        // altitude signature they reported.
        // Water is a smooth wave field, not
        // silhouetted terrain: 4 px of error
        // is invisible on it, so clamp the
        // floor here instead of making the
        // sea hostage to the terrain slider.
        split_px: split_px.max(4.0),
        px_per_rad: viewport_h / fov_deg.max(1.0).to_radians(),
        max_leaves: chunks::WATER_MAX_LEAVES,
        max_build_requests: 24,
    };
    let seed = d.terrain_seed;
    let ws = state
        .planet_chunk_states
        .entry(wkey)
        .or_insert_with(|| chunks::ChunkState::new(seed));
    ws.frame += 1;
    // Parked skip for the WATER selection too
    // (v0.928) - same static-pose rule as the
    // terrain selection above.
    // v0.1047: 0.25 m -> 3.0 m. Floating at the
    // waterline the camera BOBS with the waves, so
    // the old tolerance re-ran the whole water
    // selection every frame; under the 512-leaf
    // budget the equal-error cut lands slightly
    // differently each time, so patches near the
    // cut flip in and out faster than the build
    // cap can refill them - holes, and the flat
    // backstop showing through as the operator's
    // "blue triangles [that] randomly pop up as I
    // bob up and down in the water". A metre of
    // altitude changes the pixel error of a patch
    // hundreds of metres away by nothing, so this
    // tolerance costs no visible detail.
    let wparked = state.camera.surface_mode
        && (cam_local - ws.last_sel_cam).length() < 3.0
        && cam_fwd_local.dot(ws.last_sel_fwd) > 0.99999
        && !ws.sel_dirty
        && ws.last_selection.as_ref().is_some_and(|s| {
            s.build_requests.is_empty() && s.fully_covered
        });
    let wsel = if wparked {
        ws.last_selection.clone().expect("wparked implies stored")
    } else {
        let sel = chunks::select_patches_sticky(
            cam_local,
            Some(&frustum),
            &|id| ws.cache.get(id).map(|e| e.band),
            &wparams,
            Some(&ws.last_drawn),
        );
        ws.last_sel_cam = cam_local;
        ws.last_sel_fwd = cam_fwd_local;
        ws.sel_dirty = false;
        ws.last_selection = Some(sel.clone());
        sel
    };
    ws.last_drawn = wsel.draws.iter().cloned().collect();
    let frame = ws.frame;
    for id in &wsel.draws {
        if let Some(e) = ws.cache.get_mut(id) {
            e.last_used = frame;
        }
    }
    // [WaterDiag] (v0.1049) - the water twin of
    // [ChunkDiag] above. The far-field plates are a
    // COVERAGE question (does the wave shell reach
    // the horizon, or does the backstop show through
    // 3.6 m lower?) and coverage is invisible in a
    // screenshot until you know whether the leaf
    // budget saturated and at what error it cut.
    // Permanent: this bug class is altitude-triggered
    // and recurs whenever the budget or the error
    // floor moves.
    if ws.frame % 60 == 0 {
        let wdmax =
            wsel.draws.iter().map(|d| d.depth).max().unwrap_or(0);
        let wdmin =
            wsel.draws.iter().map(|d| d.depth).min().unwrap_or(0);
        log::info!(
            "[WaterDiag] draws={} d={}..{} sat={} covered={} req={} cache={} budget={} split_px={:.1} alt={:.0}m refused=({:.0}px@d{}) maxleaf=({:.0}px@d{})",
            wsel.draws.len(),
            wdmin,
            wdmax,
            wsel.stats.budget_saturated,
            wsel.fully_covered,
            wsel.build_requests.len(),
            ws.cache.len(),
            wparams.max_leaves,
            wparams.split_px,
            cam_local.length() - d.radius,
            wsel.stats.max_refused_err,
            wsel.stats.max_refused_depth,
            wsel.stats.max_leaf_err,
            wsel.stats.max_leaf_depth,
        );
    }
    let mut wbuilds = 0usize;
    for id in &wsel.build_requests {
        // v0.1047: 8 -> 24 per frame (the
        // selection never requests more than
        // wparams.max_build_requests anyway). A
        // transient hole now closes in one frame
        // instead of three-plus, which is what
        // the eye actually catches.
        if wbuilds >= 24 {
            break;
        }
        if ws.cache.contains_key(id) {
            continue;
        }
        // All-land patches cache a degenerate
        // placeholder so selection stops
        // requesting them (drawing it is a
        // no-op triangle at the anchor).
        let (mesh, anchor, band, bytes) =
            match chunks::build_water_patch_mesh(d, om, state.planet_heightmaps.get(&b.id).map(|a| a.as_ref()), id) {
                Some(pm) => (
                    Mesh::from_planet_surface(
                        &state.renderer.device,
                        &pm.mesh,
                    ),
                    pm.anchor,
                    pm.band,
                    chunks::PATCH_MESH_BYTES,
                ),
                None => {
                    let c = chunks::patch_corners(id);
                    let a = (c[0] + c[1] + c[2]).normalize()
                        * d.radius;
                    (
                        Mesh::placeholder(&state.renderer.device),
                        a,
                        chunks::water_band(d.radius),
                        256,
                    )
                }
            };
        let slot = if let Some(idx) =
            state.planet_patch_free_slots.pop()
        {
            state.renderer.replace_mesh(idx, mesh);
            idx
        } else {
            state.renderer.add_mesh(mesh)
        };
        ws.insert(*id, slot, bytes, anchor, band);
        wbuilds += 1;
    }
    if wbuilds > 0 {
        ws.sel_dirty = true; // v0.928 parked skip
    }
    let wevicted =
        ws.collect_evictions(chunks::PATCH_CACHE_MAX_BYTES / 8);
    if !wevicted.is_empty() {
        ws.sel_dirty = true;
    }
    // Water patches stay on the classic mesh
    // path (transparent pipeline), so slots
    // are always None here; the arm exists
    // for signature parity only.
    for (_, mesh_idx, aslot) in wevicted {
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
    // Draw whatever is BUILT (v0.1017, water arc):
    // the old `fully_covered` gate was
    // all-or-nothing - one unbuilt patch
    // anywhere hid the ENTIRE ocean. From
    // underwater the selection wants deep
    // patches in every direction at once and
    // the 8-per-frame build cap never
    // catches up, so the surface simply
    // never drew until the player left the
    // water (operator: "the surface of the
    // water disappears... I have to wait
    // until I'm out of the water"). A
    // transient local gap while a patch
    // builds is far better than a vanishing
    // sea.
    if !wsel.draws.is_empty() {
        // ANCESTOR FALLBACK (v0.1046,
        // operator: "flat blue triangles...
        // the visual updates, most of the
        // triangles disappear, another
        // refresh happens then it seems to
        // revert"): water builds are capped
        // at 8/frame, so right after the
        // camera moves a chunk of the
        // selection is still UNBUILT. The
        // old loop simply skipped those,
        // leaving real holes through which
        // the flat BACKSTOP showed - big
        // pale plates that healed a second
        // later as the builds landed (a red
        // -tinted backstop proved it: ~100%
        // of the sea right after a teleport,
        // 0% once held still). Standard
        // streaming answer: never draw a
        // hole - fall back to the nearest
        // RESIDENT ancestor, which always
        // exists because roots are never
        // evicted. Its wave shading is the
        // real sea, just coarser, so the
        // transient reads as water instead
        // of a plate. Ancestors are deduped
        // so one coarse patch covering four
        // missing children draws once.
        let mut wdrawn: std::collections::HashSet<chunks::PatchId> =
            std::collections::HashSet::new();
        let mut wfallbacks = 0usize;
        for id in &wsel.draws {
            if ws.cache.contains_key(id) {
                continue;
            }
            let mut anc = id.parent();
            while let Some(a) = anc {
                if ws.cache.contains_key(&a) {
                    if wdrawn.insert(a) {
                        wfallbacks += 1;
                    }
                    break;
                }
                anc = a.parent();
            }
        }
        if wfallbacks > 0 {
            // Keep the fallbacks resident:
            // they are what covers the sea
            // while the real leaves stream.
            for a in &wdrawn {
                if let Some(e) = ws.cache.get_mut(a) {
                    e.last_used = frame;
                }
            }
        }
        // COARSE-LEAF FLOOR (v0.1049). A water
        // leaf shallower than this is not a
        // patch of sea, it is a continent-sized
        // flat triangle: depth 0 is a whole
        // icosahedral face (7053 km edge, 441 km
        // cells) and [WaterDiag] caught exactly
        // that being drawn for a frame after a
        // teleport, when restricted descent held
        // the root because its children had not
        // streamed in yet. Such a leaf can only
        // ever be wrong on screen, so drop it
        // and let the coarse BACKSTOP shell
        // (its own selection, max_depth 11) hold
        // that ground - since v0.1045 the
        // backstop shades identically to the
        // sea, so what shows through reads as
        // calm water rather than a plate. Depth
        // 6 keeps 110 km patches, whose 6.9 km
        // cells sag under a metre off the true
        // sphere (0.05 px past 20 km) and whose
        // waves the v0.1049 Nyquist gate has
        // already flattened - smooth sea, not
        // facets.
        //
        // ALTITUDE-GATED, and that is not
        // optional: from orbit a coarse leaf is
        // the CORRECT leaf. [WaterDiag] at
        // 400 km shows the selection drawing
        // depth 5..8 at 10 px of error, so a
        // blanket floor of 6 would delete every
        // depth-5 patch and punch holes in the
        // ocean seen from space. Below 20 km the
        // horizon is under ~500 km and a
        // sub-depth-6 leaf cannot be anything
        // but a streaming artifact; above it,
        // trust the selector.
        let walt = cam_local.length() - d.radius;
        let water_min_draw_depth: u8 =
            if walt < 20_000.0 { 6 } else { 0 };
        for id in wsel
            .draws
            .iter()
            .filter(|i| ws.cache.contains_key(i))
            .chain(wdrawn.iter())
            .filter(|i| i.depth >= water_min_draw_depth)
        {
            if let Some(e) = ws.cache.get(id) {
                let anchor_render =
                    render_off + rot_d * e.anchor;
                // TRANSPARENT list (alpha
                // blend, depth-test but no
                // depth-write): the shell's
                // Fresnel alpha needs real
                // blending over the seafloor;
                // the opaque pass would stamp
                // it solid. Pushed before the
                // atmo/cloud shells, so the
                // blend order stays
                // back-to-front from space.
                celestial_transparent.push(RenderObject { fade: 0.0,
                    position: Vec3::new(
                        anchor_render.x as f32,
                        anchor_render.y as f32,
                        anchor_render.z as f32,
                    ),
                    rotation,
                    scale: Vec3::ONE,
                    mesh: e.mesh,
                    material: wmat,
                });
            }
        }
    }
}
