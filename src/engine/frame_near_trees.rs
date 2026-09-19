//! NEAR-FIELD TREES: deciding which real 3D trees stand around the player this
//! frame, and where the far cards must stop so the two never overlap or leave
//! a gap (extracted from lib.rs, v0.1320).
//!
//! Two things happen here, and they are separate on purpose:
//!
//! 1. THE HARVEST. A gated walk of the drawn terrain patches that produces the
//!    set of trees within the Settings radius. It is gated because it is the
//!    only part with a cost that scales with the forest;
//!    `terrain::near_tree_gate` owns the rule (movement is eager, the depth
//!    and density changes are rate-limited) and hands back the reason so the
//!    log can say which term fired.
//! 2. THE DRAW PLAN. A walk of that set that decides, per tree, colour list /
//!    shadow-only / nothing, and tracks where the models stopped covering.
//!
//! WHY THE SECOND PART MATTERS MORE THAN IT LOOKS: the card-hide radius is a
//! PROMISE -- every terrain tree card inside it discards because a model stands
//! there -- so it has to be the NEAREST tree that got no model, not any of the
//! three proxies that were tried and broke (view-culled draw count, the
//! budget-th tree, the farthest drawn tree). `chunks::NearTreeDrawPlan` owns
//! that arithmetic; the stories are in docs/BUGS.md (BUG-068).
//!
//! WHY IT IS A MODULE: 501 lines in the middle of the celestial draw loop, and
//! the file-size ratchet named this region when it went red. `src/lib.rs` is
//! the repo's merge funnel (CLAUDE.md), so every block that leaves it widens
//! how many sessions can work in parallel.
//!
//! WHY THE TWO CONTEXT STRUCTS instead of `&mut EngineState`: the caller is
//! holding four disjoint borrows out of the state at once -- the chunk state
//! `cs`, the heightmap `hm`, the planet def `d`, and the ocean mask -- so a
//! whole-state borrow is an E0499/E0502. This is the same constraint recorded
//! on `near_tree_models::ensure_near_tree_models`, which this code calls.
//! `NearTreeState` names the thirteen state fields this block touches and
//! nothing else; `NearTreeView` names the ten frame values it reads. The
//! struct definitions ARE the dependency list, and the compiler checks them.
//!
//! BEHAVIOUR: none. The body is the previous inline block line for line apart
//! from fourteen mechanical characters, every one of them the `*` (or `*` in a
//! reborrow) that a borrowed field needs where the inline code had the field
//! itself:
//!
//! - seven ASSIGNMENTS: `*state.near_trees = ...` and the same for
//!   near_tree_depth, near_tree_density, near_tree_new, near_tree_born_s, and
//!   near_trees_center twice (once on recompute, once on the clear-on-leave);
//! - six READS in by-value positions: `*state.near_trees_center - cam_local`,
//!   the two `- *state.near_tree_born_s` subtractions, the two gate arguments
//!   `*state.near_tree_depth` / `*state.near_tree_density`, and the
//!   `(*.., *..)` tuple that snapshots them for the log line;
//! - one REBORROW: `&mut *celestial_objects`, because the sink is chosen
//!   between the caller's list and a local one and the parameter is already a
//!   `&mut`.
//!
//! The compiler rejects every one of these if it is wrong, so the list is a
//! reading aid, not a trust assumption.

use crate::assets::AssetManager;
// The model cache this block calls every frame. In lib.rs it arrived by bare
// name through `mod native_app`'s glob import of `crate::engine::*`.
use crate::engine::near_tree_models::ensure_near_tree_models;
use crate::gui::GuiState;
use crate::renderer::{RenderObject, Renderer};
use crate::terrain::planet::PlanetDef;
use crate::terrain::planet_chunks as chunks;
use glam::{DQuat, DVec3, Quat, Vec3};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// The slice of `EngineState` the near-tree block touches, borrowed field by
/// field.
///
/// Named `state` at the call site on purpose: the moved code keeps reading
/// `state.renderer`, `state.near_trees` and the rest exactly as it did inline.
pub(crate) struct NearTreeState<'a> {
    /// Meshes and materials, plus the two fields this block PUBLISHES to the
    /// renderer: `tree_card_hide_m` (the card-hide promise) and
    /// `celestial_colour_skip` (the tail range of shadow-only models the
    /// colour pass must skip).
    pub renderer: &'a mut Renderer,
    /// Source of the glTF tree models, read by the model-cache pass.
    pub asset_manager: &'a AssetManager,
    /// name -> (mesh index, material index) for every tree part built so far.
    pub decoration_mesh_cache: &'a mut HashMap<String, (usize, usize)>,
    /// One-shot guard so the sprite-atlas bake is attempted once per run.
    pub tree_atlas_attempted: &'a mut bool,
    /// Read for the Settings tree knobs (model distance, near-tree budget).
    pub gui_state: &'a GuiState,
    /// Surface imagery, used to tint harvested trees to their ground.
    pub planet_albedos:
        &'a HashMap<String, Arc<crate::terrain::planet_albedo::PlanetAlbedo>>,
    /// Session clock, for the fade-in and the harvest rate limiter.
    pub start_time: &'a Instant,
    /// THE SET: the trees standing around the player, distance-sorted.
    pub near_trees: &'a mut Vec<chunks::NearTree>,
    /// Where the camera was when the set was harvested (the movement gate).
    pub near_trees_center: &'a mut DVec3,
    /// Parallel to `near_trees`: true = this tree was not in the previous set,
    /// so it dissolves in instead of popping.
    pub near_tree_new: &'a mut Vec<bool>,
    /// When the current set was born, for that fade.
    pub near_tree_born_s: &'a mut f32,
    /// The terrain LOD depth the current set's bases were anchored to. A depth
    /// change under a standing player has to re-harvest or the trees float.
    pub near_tree_depth: &'a mut u8,
    /// The vegetation density the current set was harvested at, so a slider
    /// move re-harvests instead of orphaning cards.
    pub near_tree_density: &'a mut f32,
}

/// Where the camera is, how this planet is oriented, and what the terrain
/// patches decided -- all already computed above the call site.
#[derive(Clone, Copy)]
pub(crate) struct NearTreeView<'a> {
    /// Camera position in planet-local (unrotated) metres.
    pub cam_local: DVec3,
    /// Planet centre in render space (the floating origin moves it).
    pub render_off: DVec3,
    /// Planet orientation in f64, for placing tree bases.
    pub rot_d: DQuat,
    /// The same orientation in f32, for the render objects.
    pub rotation: Quat,
    /// The patch selector's own frustum, re-based on the camera here so the
    /// model cull runs on metre-scale positions.
    pub frustum: &'a chunks::FrustumPlanes,
    /// Whether terrain patches actually drew this frame. Trees anchor to the
    /// drawn mesh, so without it there is nothing to stand on.
    pub chunked_drawn: bool,
    /// The density the patch bake emitted cards at. The harvest must use the
    /// SAME number or the two streams disagree about which trees exist.
    pub veg_harvest_density: f32,
    /// Elevation sources for the harvest walk: the base heightmap, the
    /// streamed high-detail tiles, and the connected-ocean mask.
    pub hm: &'a crate::terrain::planet_heightmap::PlanetHeightmap,
    pub tiles_ref: Option<&'a crate::terrain::terrain_tiles::TerrainTiles>,
    pub ocean_ref: Option<&'a crate::terrain::ocean_mask::OceanMask>,
}

/// Harvest the near-field tree set if the gate says to, then plan and push
/// this frame's 3D tree draws.
///
/// `cs` is the planet's chunk state, read for the drawn-patch depth and the
/// detail noise; `b` the body (its id keys the albedo); `d` its planet
/// definition. `celestial_objects` receives both the visible models and, at
/// its end, the off-screen shadow-only ones.
pub(crate) fn draw_near_trees(
    state: &mut NearTreeState,
    view: NearTreeView,
    cs: &chunks::ChunkState,
    b: &crate::cosmos::SolBody,
    d: &PlanetDef,
    celestial_objects: &mut Vec<RenderObject>,
) {
    // Unpack the view under the names the moved code already used, so the body
    // below stays line for line what it was inline.
    let NearTreeView {
        cam_local,
        render_off,
        rot_d,
        rotation,
        frustum,
        chunked_drawn,
        veg_harvest_density,
        hm,
        tiles_ref,
        ocean_ref,
    } = view;
// ── Near-field REAL trees (v0.911, operator:
// "get the plants on the Earth to use a
// variety... instead of the simple geometry
// placeholder trees") ── within the Settings
// radius, photoscanned conifers stand where
// the vegetation stream put their silhouette
// cards (the card hides inside the model).
// The card system stays as the far LOD.
let tree_dist = state
    .gui_state
    .settings
    .tree_model_distance
    .clamp(0.0, crate::config::TREE_MODEL_MAX_M) as f64;
// Hoisted (v0.1110.2) so the HARVEST is sized
// from it too. The draw budget is the only cap
// the draw loop can observe, so it has to be
// the binding one - see near_tree_harvest_cap.
let near_tree_draw_budget: u32 =
    state.gui_state.settings.near_tree_budget
        .clamp(1.0, crate::config::NEAR_TREE_BUDGET_MAX)
        as u32;
let alt_over = cam_local.length() - d.radius;
// Default: no card hiding unless the model
// loop below actually covered a radius, and
// no shadow-only range unless it culled one.
state.renderer.tree_card_hide_m = 0.0;
state.renderer.celestial_colour_skip = 0..0;
if chunked_drawn && tree_dist > 1.0 && alt_over < 2500.0 {
    let moved =
        (*state.near_trees_center - cam_local).length();
    // v0.995 (operator: "I'll turn, walk a bit,
    // then some trees blink into existence in
    // front of me"): 40 m of hysteresis kept the
    // model set centered well BEHIND a walking
    // player - thin coverage ahead, then a whole
    // batch popping on the threshold. 12 m keeps
    // the set centered on you; the stream walk
    // is sub-millisecond, so eager is cheap.
    // Re-harvest on movement OR when the drawn
    // terrain LOD changed under our feet (the
    // world-entry walk-up): bases anchor to the
    // drawn mesh, so a depth change with stale
    // trees leaves them on the previous mesh.
    let tree_draw_depth = cs
        .last_drawn
        .iter()
        .map(|p| p.depth)
        .max()
        .unwrap_or(0);
    // The DENSITY term matters as much as the
    // other two: without it a slider move
    // changes what the next patch bake emits
    // while the harvest waits up to 12 m of
    // walking to notice, and that window is
    // exactly the orphaned-card window.
    // The gate itself lives in
    // terrain::near_tree_gate (pure, tested):
    // movement is eager, the two change
    // terms are rate-limited, and the reason
    // comes back for the log. The 2026-09-18
    // measurement saw this fire every frame
    // while parked; the log line below now
    // says which term did it.
    let since_last_s = state.start_time.elapsed().as_secs_f32()
        - *state.near_tree_born_s;
    let recompute_why =
        crate::terrain::near_tree_gate::near_tree_recompute_reason(
            moved,
            tree_draw_depth,
            *state.near_tree_depth,
            veg_harvest_density,
            *state.near_tree_density,
            since_last_s,
        );
    if let Some(why) = recompute_why {
        let (prev_depth, prev_density) =
            (*state.near_tree_depth, *state.near_tree_density);
        let src = chunks::ElevationSource::Heightmap {
            hm,
            detail: &cs.detail,
            tiles: tiles_ref,
            ocean: ocean_ref,
        };
        // Previous set's identity keys (quantized
        // planet-local base), so recompute can
        // tell survivors from newcomers - only
        // NEW trees fade in (v0.995).
        let prev_keys: std::collections::HashSet<u64> = state
            .near_trees
            .iter()
            .map(|t| {
                let p = t.dir * t.r_m;
                ((p.x * 2.0).round() as i64 as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ ((p.y * 2.0).round() as i64 as u64)
                        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
                    ^ ((p.z * 2.0).round() as i64 as u64)
            })
            .collect();
        let _cost_tree_harvest =
            crate::renderer::frame_costs::stage("cpu.near_tree_harvest");
        // v0.1097, the 13 m floating-trees fix:
        // bases interpolate the DRAWN mesh at
        // this depth (computed above the gate
        // so a standing-still LOD change also
        // re-harvests).
        *state.near_tree_depth = tree_draw_depth;
        *state.near_tree_density = veg_harvest_density;
        *state.near_trees = chunks::near_tree_instances_at_density(
            d,
            &src,
            state.planet_albedos.get(&b.id).map(|a| a.as_ref()),
            cam_local.normalize(),
            tree_dist + 60.0,
            tree_draw_depth,
            veg_harvest_density,
            chunks::near_tree_harvest_cap(near_tree_draw_budget),
        );
        *state.near_tree_new = state
            .near_trees
            .iter()
            .map(|t| {
                let p = t.dir * t.r_m;
                let k = ((p.x * 2.0).round() as i64 as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ ((p.y * 2.0).round() as i64 as u64)
                        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
                    ^ ((p.z * 2.0).round() as i64 as u64);
                !prev_keys.contains(&k)
            })
            .collect();
        *state.near_tree_born_s =
            state.start_time.elapsed().as_secs_f32();
        *state.near_trees_center = cam_local;
        // Debug level: at info this was a
        // per-frame flood while parked. The
        // reason and the before/after values
        // are what a future reader needs to
        // tell a walk from an oscillating
        // input.
        log::debug!(
            "[NearTree] recompute ({}): {} trees within {:.0} m (alt {:.0} m; moved {:.1} m, depth {} -> {}, density {:.2} -> {:.2}, {:.2} s since last)",
            why.label(),
            state.near_trees.len(),
            tree_dist + 60.0,
            alt_over,
            moved,
            prev_depth,
            tree_draw_depth,
            prev_density,
            veg_harvest_density,
            since_last_s,
        );
    }
    // Near-tree MODEL CACHE + sprite atlas
    // bake (extracted to
    // engine/near_tree_models.rs, v0.1108):
    // ~420 lines with one job and no frame
    // inputs at all. Called every frame on
    // purpose - every branch inside is
    // guarded by a cache lookup, so a species
    // that streams in later still gets its
    // mesh. Four named borrows rather than
    // &mut state: cs (above) holds
    // planet_chunk_states.
    ensure_near_tree_models(
        &mut state.renderer,
        &state.asset_manager,
        &mut state.decoration_mesh_cache,
        &mut state.tree_atlas_attempted,
    );
    // Per-variant model heights from the split
    // report (metres): scale = target / model.
    const TREE_MODEL_H: [[f32; 3]; 2] =
        [[1.27, 0.92, 0.70], [1.06, 0.85, 0.83]];
    // The photoscans are 120-190k tris each;
    // cap DRAWN trees (nearest first - the
    // list is distance-sorted) so a dense
    // forest stays in budget. Cards cover the
    // rest.
    //
    // v0.914 (operator: "the nearest trees to
    // me are disappearing"): cards hid across
    // the WHOLE slider radius while models
    // only covered the nearest 64 - everything
    // past the cap had neither card nor model,
    // and approaching a dense stand rotated
    // nearby trees out of existence. Track how
    // far the drawn models actually reach and
    // hide cards only inside that.
    //
    // v0.1110.2: the reach is now the NEAREST
    // tree that got no model, not the FARTHEST
    // that got one. Those agree only if the set
    // is perfectly nearest-first, and it is not
    // - the harvest re-sorts every 12 m while
    // the camera keeps moving, so trees ahead
    // of the player overtake the ranking. See
    // near_trees::ModelCoverage.
    //
    // Frame-cost arc increment V1 (2026-09-18):
    // the COLOUR draw list is frustum-culled;
    // the model set (nearest N all round) and
    // the coverage arithmetic are not, and an
    // off-screen model still casts its shadow.
    // All of it lives in
    // near_trees::NearTreeDrawPlan, so the cull
    // cannot reach the hide radius (the three
    // ways that went wrong before are BUG-068).
    // The frustum is the patch selector's own,
    // built above from the same f64 view-
    // projection the celestial draw uses, and
    // re-based on the camera here so the test
    // runs on metre-scale camera-relative
    // positions: a planet-scale coordinate
    // never reaches it.
    let tree_frustum =
        frustum.into_local(glam::DQuat::IDENTITY, cam_local);
    // The draw budget (hoisted above, where the
    // harvest cap is derived from it) bounds how
    // many 3D trees can draw at ANY distance, so
    // raising the distance without it just packs
    // the same set into a tighter ring.
    let mut plan = chunks::NearTreeDrawPlan::new(
        Some(&tree_frustum),
        chunks::ModelCoverage::new(tree_dist),
        near_tree_draw_budget,
    );
    // Off-screen models go here instead of the
    // colour list; appended to the END of
    // celestial_objects after the loop, and the
    // renderer is told the range so the colour
    // pass skips them while the sun shadow pass
    // (which walks the whole list) still draws
    // them. A conifer behind the player shades
    // the ground in front at a low sun.
    let mut near_tree_shadow_only: Vec<RenderObject> = Vec::new();
    let now_s = state.start_time.elapsed().as_secs_f32();
    let fade_in =
        ((now_s - *state.near_tree_born_s) / 0.35).clamp(0.0, 1.0);
    for (ti, tr) in state.near_trees.iter().enumerate() {
        let base_local = tr.dir * tr.r_m;
        // Camera-relative, f64: the range test,
        // the frustum test and the coverage
        // distance all read this one vector.
        let rel = base_local - cam_local;
        // RANGE FIRST, then the frustum, then
        // the two budgets. The plan CONTINUES
        // past an exhausted budget rather than
        // breaking: the coverage tracker needs
        // the tail to know where the models
        // stopped, and a `break` would throw
        // away exactly the trees that define
        // it. The tail is a few hundred f64
        // compares and six dot products each.
        let (slot, d2) = plan.consider(rel, tr.height_m);
        let chunks::TreeSlot::Lookup { .. } = slot else {
            continue;
        };
        // New trees dissolve in; survivors and
        // fully-faded sets draw normally.
        let obj_fade = if fade_in < 1.0
            && state.near_tree_new.get(ti).copied().unwrap_or(false)
        {
            fade_in.max(1.0 / 32.0)
        } else {
            0.0
        };
        // v0.1066: species is now an index into
        // data/vegetation/trees.ron. Procedural
        // species resolve to a single generated
        // mesh; model species keep the glTF
        // stem + _bark pair they always had.
        let va = (tr.variant % 3) as usize;
        let tdef =
            crate::renderer::tree_mesh::registry()
                .get(tr.species as usize);
        let procedural =
            tdef.map(|t| t.is_procedural()).unwrap_or(false);
        // TREE_MODEL_H is indexed by the two
        // photoscans, so map by MODEL NAME
        // rather than registry order - a
        // reordered data file must not silently
        // rescale every conifer.
        let sp = match tdef.map(|t| t.model.as_str()) {
            Some("pine_sapling_small") => 1,
            _ => 0,
        };
        // use_proc = the species is procedural by
        // DATA, or its model FAILED to load (the
        // usize::MAX sentinel) and the loader
        // built its procedural twin - the
        // shipped-build path. All three branches
        // below (stem key, scale, suffixes) must
        // agree on this one flag.
        let use_proc = procedural
            || tdef.map_or(false, |t| {
                state
                    .decoration_mesh_cache
                    .get(&format!("{}_v{}", t.model, va + 1))
                    .map_or(false, |&(mi, _)| mi == usize::MAX)
            });
        let stem = match tdef {
            Some(t) if use_proc => {
                format!("proc:{}_v{}", t.id, va)
            }
            Some(t) => format!("{}_v{}", t.model, va + 1),
            None => format!("fir_sapling_v{}", va + 1),
        };
        // MESHES FIRST, draws after (V1): the
        // plan has to know whether this tree
        // has any mesh before anything is
        // pushed, because a tree the coverage
        // budget credits may be off-screen
        // (coverage yes, draw no) and a tree
        // the draw budget wants may be past
        // where the coverage stopped (draw yes,
        // coverage no).
        //
        // A procedural tree is ONE mesh; the
        // photoscans are a trunk + foliage pair.
        // v0.1089: a procedural tree is TWO
        // meshes now - foliage (type 20) and
        // the baked-bark wood (type 22) - the
        // same shape the photoscans have always
        // had. A missing ":wood" key just
        // `continue`s, so a species built
        // before this (or with no wood at all)
        // still draws.
        let suffixes: &[&str] =
            if use_proc { &["", ":wood"] } else { &["", "_bark"] };
        let mut parts: [Option<(usize, usize)>; 2] = [None, None];
        for (pi, suffix) in suffixes.iter().enumerate() {
            let key = format!("{stem}{suffix}");
            let Some(&(mi, ma)) =
                state.decoration_mesh_cache.get(&key)
            else {
                continue;
            };
            if mi == usize::MAX {
                continue;
            }
            parts[pi] = Some((mi, ma));
        }
        let has_mesh = parts.iter().any(|p| p.is_some());
        // The plan feeds the coverage tracker
        // here (drew, or uncovered because the
        // mesh has not streamed in yet - the
        // case the old rule was blind to) and
        // says where the meshes go: the colour
        // list, or the shadow-only list for an
        // off-screen model.
        let sink: &mut Vec<RenderObject> =
            match plan.resolve(slot, d2, has_mesh) {
                chunks::TreeDraw::Colour => &mut *celestial_objects,
                chunks::TreeDraw::ShadowOnly => {
                    &mut near_tree_shadow_only
                }
                chunks::TreeDraw::None => continue,
            };
        let pos_render = render_off + rot_d * base_local;
        // Y-up model onto the local radial up,
        // spun by its own yaw, riding the
        // planet's rotation.
        let up_arc = Quat::from_rotation_arc(
            Vec3::Y,
            tr.dir.as_vec3().normalize(),
        );
        let obj_rot = rotation
            * up_arc
            * Quat::from_rotation_y(tr.yaw);
        // Procedural meshes are generated AT the
        // species height, so the instance scale
        // only carries the per-tree jitter.
        let scl = match tdef {
            Some(t) if use_proc => {
                tr.height_m / t.height_m.max(0.01)
            }
            _ => tr.height_m / TREE_MODEL_H[sp][va],
        };
        for (mi, ma) in parts.into_iter().flatten() {
            sink.push(RenderObject { fade: obj_fade,
                position: Vec3::new(
                    pos_render.x as f32,
                    pos_render.y as f32,
                    pos_render.z as f32,
                ),
                rotation: obj_rot,
                scale: Vec3::splat(scl),
                mesh: mi,
                material: ma,
            });
        }
        // Cluster-card layers ride with the wood
        // (v0.1088): the crown mass of a clustered
        // species lives on textured cards, so a
        // sakura without them is a bare skeleton.
        if use_proc {
            if let Some(t) = tdef {
                for ci in 0..4usize {
                    let ckey = format!(
                        "proc:{}_v{}:card{ci}",
                        t.id, va
                    );
                    let Some(&(cmi, cma)) =
                        state.decoration_mesh_cache.get(&ckey)
                    else {
                        break;
                    };
                    sink.push(RenderObject {
                        fade: obj_fade,
                        position: Vec3::new(
                            pos_render.x as f32,
                            pos_render.y as f32,
                            pos_render.z as f32,
                        ),
                        rotation: obj_rot,
                        scale: Vec3::splat(scl),
                        mesh: cmi,
                        material: cma,
                    });
                }
            }
        }
    }
    // The off-screen models ride at the END of
    // the celestial list so the skip is one
    // contiguous range; everything pushed after
    // this point (later bodies, station parts)
    // lands past the range and draws normally.
    let skip_from = celestial_objects.len();
    celestial_objects.append(&mut near_tree_shadow_only);
    state.renderer.celestial_colour_skip =
        skip_from..celestial_objects.len();
    let set_n = state.near_trees.len();
    // THE CARD-HIDE RADIUS IS A PROMISE: every
    // terrain tree card inside it discards,
    // because a 3D model stands there. So it
    // has to be the NEAREST tree that got no
    // model - budget spent, or mesh not yet
    // streamed. `ModelCoverage` owns that
    // definition and the arithmetic; this line
    // is the whole of the frame loop's share.
    //
    // Three earlier rules each used a PROXY for
    // it and each broke where the proxy parted
    // from the promise (view-culled draw count
    // v0.995, the budget-th tree v0.1107, the
    // farthest DRAWN tree v0.1110.1). The
    // stories are in docs/BUGS.md. The V1
    // frustum cull is a fourth thing that could
    // have leaked in; the plan's counters are
    // what keep it out, and the `[NearTree]`
    // line below is where a leak would show
    // (hide moving with the heading).
    let hide_m: f32 = plan.hide_radius_m();
    state.renderer.tree_card_hide_m = hide_m;
    // Handoff diag (v0.994.1): 1 Hz numbers for
    // the model/card boundary.
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static LASTT: AtomicU64 = AtomicU64::new(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if LASTT.swap(now, Ordering::Relaxed) != now {
            let c = plan.counts;
            log::info!(
                "[TreeHandoff] near={} drawn={} covered={:.0}m window={:.0}m hide={:.0}m",
                set_n,
                c.drawn,
                plan.covered_radius_m(),
                tree_dist + 60.0,
                hide_m,
            );
            // The V1 counter line the frame-cost
            // arc asked for: harvested / in
            // range / frustum passed / drawn,
            // plus the pre-cull budget counter
            // the coverage saw and the hide
            // radius at full f32 precision
            // (`{:?}` round-trips), so two
            // boots can be diffed to the bit.
            log::info!(
                "[NearTree] harvested={} in_range={} frustum={} drawn={} shadow_only={} budget={} cov_models={} hide={:?}m",
                set_n,
                c.in_range,
                c.frustum_passed,
                c.drawn,
                c.shadow_only,
                near_tree_draw_budget,
                c.cov_models,
                hide_m,
            );
        }
    }
} else if !state.near_trees.is_empty() && alt_over > 4000.0 {
    state.near_trees.clear();
    *state.near_trees_center = glam::DVec3::splat(f64::MAX);
}
}
