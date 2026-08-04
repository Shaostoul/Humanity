//! NEAR-FIELD TREE HARVEST: enumerating the planet-fixed vegetation stream
//! into concrete 3D tree instances standing on the drawn ground.
//!
//! Extracted from `planet_chunks` in v0.1106, which is the extraction the
//! file-size ratchet named two releases earlier. It is a coherent cluster with
//! one job - given a camera position and the drawn patch set, produce the list
//! of trees that should exist near the player - and it is about to become the
//! centre of real work: the operator's report of trees that cast shadows but
//! draw nothing, of billboards that vanish instead of promoting to a mesh, and
//! of trees that cannot be chopped down because they are re-derived from a hash
//! every time you walk away, all live in here.
//!
//! A `#[path]` CHILD module of `planet_chunks`, not a sibling: a sibling would
//! need `pub(crate)` on a long list of chunk internals, where a child sees them
//! through one `use super::*`. That also means `terrain/mod.rs` needs no edit
//! and every existing `planet_chunks::near_tree_instances` path still resolves.

use super::*;

// ── HOW MANY TREES EXIST (v0.1111) ───────────────────────────────────────────
// The card bake lives in `planet_chunks` and the model harvest lives here, but
// the two have to name the SAME trees, so the arithmetic that decides which
// trees exist belongs to neither of them alone - it belongs to the handoff,
// which is this module. `planet_chunks` re-exports all of it (`pub use
// near_trees::*`), so the bake still calls `trees_in_cell` unqualified and
// every `planet_chunks::` path outside keeps resolving.

/// The forest density the VEGETATION-AGNOSTIC entries build at (v0.1111).
///
/// The frame loop passes the live setting in explicitly. This exists for
/// callers that want ground geometry and do not care what grows on it: the
/// drawn-surface sampler, the grass tests, the walk probe.
///
/// It is a FIXED number rather than the live setting on purpose. A caller that
/// reaches for a mutable global to answer "how dense is the forest" is exactly
/// the coupling that let the card bake and the model harvest name different
/// trees; leaving one such caller behind would leave the door open. It tracks
/// `config::default_tree_density`, and `the_agnostic_default_is_the_shipped
/// _default` fails if the two part.
pub const AGNOSTIC_TREE_DENSITY: f32 = 0.6;

/// Slider range of the forest-density setting (`settings.rs`, `config.rs`).
pub const TREE_DENSITY_MIN: f32 = 0.1;
pub const TREE_DENSITY_MAX: f32 = 1.0;

/// The clamp, in one place, because a clamp that differs between two callers
/// diverges exactly like a rounding rule that differs between them.
#[inline]
pub fn tree_density_clamped(d: f32) -> f32 {
    if d.is_nan() {
        return TREE_DENSITY_MIN;
    }
    d.clamp(TREE_DENSITY_MIN, TREE_DENSITY_MAX)
}

/// HOW MANY TREES STAND IN ONE VEGETATION CELL, and the only place that
/// question is ever answered.
///
/// `cell_lat_rad` is the cell's CENTRE latitude. Lon cells narrow toward the
/// poles by cos(lat), so the per-cell count thins to match and density stays
/// constant per square kilometre.
///
/// WHY THIS IS A SHARED FUNCTION AND NOT TWO LINES OF ARITHMETIC. Two streams
/// enumerate this grid - the card bake in `build_patch_mesh_at_density` and the
/// 3D-model harvest below - and BOTH feed this number into the survival gate
/// `(item as f32) >= count as f32 * vw`. So `count` does not merely decide how
/// many items are considered: it decides WHICH items live. Through v0.1110 the
/// two rounded differently - the bake rounded twice (`round(TREES_PER_CELL * d)`
/// then `round(that * cos lat)`), the harvest once
/// (`round(TREES_PER_CELL * d * cos lat)`) - and the difference is not
/// theoretical: measured over all 43,478 northern cells, at density 0.6294 the
/// two disagreed by one tree in 32.51% of them, at 0.6295 in 26.69%. Even at the
/// shipped default 0.6 one cell row split (iy 10727, 21.2 degrees north, 447
/// trees against 448) - which is why this survived so long, and why a test
/// written only at the default proves nothing. The slider does not snap
/// (`*value = min + t * (max - min)`), so a drag lands on arbitrary f32 values,
/// and the operator drags it.
///
/// A cell where the two disagree has cards with no model: inside
/// `tree_card_hide_m` those cards discard in the colour pass and STILL CAST
/// SHADE, because the shadow pass deliberately does not mirror that discard.
/// That is the operator's "weird chunks that wouldn't spawn trees visually but,
/// they were still affecting the light".
///
/// The DOUBLE round is the one that survives, because it is the one the shipped
/// cards were baked with: adopting the harvest's single round would have
/// changed the forest on every planet at every density.
#[inline]
pub fn trees_in_cell(density: f32, cell_lat_rad: f64) -> u32 {
    let per_cell =
        (((TREES_PER_CELL as f32) * tree_density_clamped(density)).round() as u32).max(1);
    ((per_cell as f64) * cell_lat_rad.cos().max(0.0)).round() as u32
}

/// One near-field tree from the planet-fixed vegetation stream (v0.911):
/// the same cell hash the patch bake emits silhouette cards from,
/// re-enumerated at runtime so REAL 3D models can stand where the cards
/// are. dir is the planet-local unit direction, r_m the drawn ground
/// radius at the base; yaw/height mirror the card's own randoms.
pub struct NearTree {
    pub dir: DVec3,
    pub r_m: f64,
    pub yaw: f32,
    pub height_m: f32,
    /// 0 = fir, 1 = pine (stable per tree).
    pub species: u8,
    /// Shape variant 0-2 (the three photoscan variants per species).
    pub variant: u8,
}

/// A tree's base is sunk by this fraction of its own ROOT-FLARE RADIUS
/// (v0.1097). Not a constant metre offset: the harvest spans 4-18 m trees, and
/// a number that roots a 4 m sapling leaves an 18 m conifer hovering while a
/// number that roots the conifer buries the sapling to its first whorl.
///
/// WHY ANY SINK AT ALL, once the base is exact. A trunk is a cylinder and the
/// drawn ground under it is a PLANE: on a slope the downhill side of the bole
/// meets air, and the gap is the trunk radius times the tangent of the slope
/// (a 0.15 m flare on a 30 degree flank shows ~9 cm of daylight). Sinking the
/// base closes most of that without ever eating the flare - `tree_mesh`'s
/// stems are built at `r_base = height * 0.022..0.034` with a flare bump up to
/// 1.28x, so a quarter of the flare radius is ~1 cm on a sapling and ~4 cm on
/// a big conifer, exactly half the burial tolerance the CI gate allows.
pub const TREE_GROUND_SINK_FLARE_FRAC: f64 = 0.25;
/// Root-flare radius of a tree of this height, metres: the widest the bole
/// gets where it meets the ground. Mirrors `renderer::tree_mesh`'s stem
/// builders (`r_base = h * 0.030` for the fir, 0.022-0.034 across the four
/// species) times the 1.28 flare bump. Approximate ON PURPOSE - it sizes a
/// centimetre-scale sink and the gate's burial tolerance, not any geometry.
pub fn tree_flare_radius_m(height_m: f32) -> f64 {
    height_m as f64 * 0.030 * 1.28
}

/// The band the card layer and the model layer SHARE at the handoff, metres.
/// The nearest tree that did not get a model keeps its card, and so does
/// everything from there inward for this much, so the two representations
/// always overlap rather than meeting at a line that any rounding can open a
/// gap in.
pub const TREE_CARD_HANDOFF_OVERLAP_M: f64 = 8.0;

/// Trees the harvest keeps BEYOND the frame loop's 3D draw budget.
///
/// The harvest's `max_n` is not a performance knob: `near_tree_instances_on_drawn`
/// walks, gates, ground-samples and sorts the WHOLE disc before it truncates, so
/// a small cap buys nothing but a shorter Vec. What it costs is information. A
/// tree the harvest threw away is a tree the frame loop never sees, so the frame
/// loop cannot know that the models stop there - and the card layer, which is
/// told to hide inside a radius the frame loop computes, hides over ground the
/// models never reached.
///
/// Keeping a margin past the budget means the BUDGET is always the binding
/// constraint, and the budget is one the draw loop can observe directly.
pub const NEAR_TREE_HARVEST_SLACK: usize = 256;

/// How many trees a harvest should keep, given the frame loop's model budget.
/// Pass this as `max_n`; never a bare literal, or the cap becomes a second,
/// invisible handoff line (it was a hardcoded 600 through v0.1110, which bound
/// before any budget above ~600 could).
#[inline]
pub fn near_tree_harvest_cap(draw_budget: u32) -> usize {
    (draw_budget as usize).saturating_add(NEAR_TREE_HARVEST_SLACK)
}

/// HOW FAR OUT THE NEAR-FIELD 3D MODELS ACTUALLY COVER THE GROUND.
///
/// `renderer.tree_card_hide_m` is a PROMISE to the fragment shader: every tree
/// inside that radius has a real model standing on it, so the terrain's
/// silhouette card for it can discard. Break the promise and the tree has
/// neither representation - it is simply gone. That is the operator's report
/// of 2026-08-03: "the billboards for the lower LOD trees just kind of phase
/// out of existence instead of actually shifting to a higher LOD."
///
/// WHY THE OLD RULE BROKE IT. Through v0.1110 the radius was the distance to
/// the FARTHEST DRAWN tree (`covered_r2`, a running max), minus 8 m. That is a
/// different question from the one the promise asks. The models cover a disc
/// only out to the NEAREST tree they MISSED; past that the set is full of
/// holes. The two agree exactly when the draw order is perfectly nearest-first
/// - and it is not, because the harvest sorts once every 12 m of walking
/// (lib.rs hysteresis) while the camera keeps moving. Trees ahead of the player
/// get closer than trees the sort ranked before them, so as soon as a cap binds
/// the drawn set stops being a clean disc: max(drawn) runs PAST min(missed).
///
/// MEASURED, walking east 40 m at Fuji on the real Earth data, shipped defaults
/// (120 m model radius, 256 model budget, forest density 0.6): up to 32 trees
/// per frame sat inside the hide radius with no model, in a band 11.5 m deep at
/// ~92 m out, and 100% of them were AHEAD of the direction of travel. The count
/// climbs from 0 immediately after a re-harvest to its worst just before the
/// next one, so the ring of missing trees pulses at the 12 m walk period. At a
/// 400 m model radius it was 48 trees in an 11.1 m band, and RAISING the model
/// budget did not help at all, because the harvest's own hardcoded 600-tree cap
/// bound first (see `NEAR_TREE_HARVEST_SLACK`).
///
/// THE RULE HERE. Track the nearest tree that did NOT get a model - whatever
/// the reason: the draw budget ran out, or its mesh has not streamed in yet -
/// and hide cards only inside that, less the overlap. It is the definition of
/// the promise rather than a proxy for it, so it holds under a stale sort, a
/// bound cap, a missing mesh, or any combination. It also fails SAFE: the worst
/// it can do is show a card inside a model, where the model hides it anyway,
/// which is exactly what shipped before the cards learned to discard at all.
#[derive(Debug, Clone)]
pub struct ModelCoverage {
    /// The model radius (Settings), the ceiling on any hide radius: cards must
    /// never hide past where models are even attempted.
    tree_dist_m: f64,
    /// Squared distance of the nearest tree that got no model.
    nearest_uncovered_m2: f64,
    /// Squared distance of the farthest tree that did - DIAGNOSTIC ONLY (the
    /// [TreeHandoff] log line). Never feed this to the hide radius; that is the
    /// bug this type exists to make unrepresentable.
    farthest_drawn_m2: f64,
    drew_any: bool,
}

impl ModelCoverage {
    pub fn new(tree_dist_m: f64) -> Self {
        Self {
            tree_dist_m: tree_dist_m.max(0.0),
            nearest_uncovered_m2: f64::INFINITY,
            farthest_drawn_m2: 0.0,
            drew_any: false,
        }
    }

    /// A tree at `dist2` (squared metres from the camera) got its model.
    #[inline]
    pub fn drew(&mut self, dist2: f64) {
        self.drew_any = true;
        if dist2 > self.farthest_drawn_m2 {
            self.farthest_drawn_m2 = dist2;
        }
    }

    /// A tree at `dist2` did NOT get a model and still needs its card.
    #[inline]
    pub fn uncovered(&mut self, dist2: f64) {
        if dist2 < self.nearest_uncovered_m2 {
            self.nearest_uncovered_m2 = dist2;
        }
    }

    /// The radius terrain tree cards may discard inside, metres.
    pub fn hide_radius_m(&self) -> f32 {
        if !self.drew_any {
            return 0.0;
        }
        let reach = self.nearest_uncovered_m2.sqrt().min(self.tree_dist_m);
        ((reach - TREE_CARD_HANDOFF_OVERLAP_M).clamp(0.0, self.tree_dist_m)) as f32
    }

    /// Farthest drawn model, metres. For the 1 Hz [TreeHandoff] log only.
    pub fn covered_radius_m(&self) -> f64 {
        self.farthest_drawn_m2.sqrt()
    }
}

/// Enumerate trees within `radius_m` surface metres of `center_dir` on the
/// planet-fixed tree grid: the SAME deterministic per-cell stream, gates
/// (treeline, beach, imagery-green biome), and ground sampling as
/// build_patch_mesh's vegetation pass, so every returned tree coincides
/// with a baked card (the model hides its card inside it). Capped at
/// `max_n` (cells walk outward from the center row-major; a generous cap
/// simply stops early).
///
/// THE UNWIRED PATH (v0.1097). This overload passes `drawn_depth = 0`, which
/// means "the caller does not know what depth the ground under it is DRAWN
/// at", and the base then falls back to the direct depth-20 elevation sample
/// this harvest has always used - bit-identical to v0.1096. That fallback is
/// the bug the operator photographed (trees hovering over their slopes), and
/// it cannot be fixed from in here: the drawn depth is a property of the LOD
/// selector's chosen leaf set, which only the frame loop holds. Call
/// [`near_tree_instances_on_drawn`] with the drawn leaf depth instead - the
/// grass harvest already takes exactly that argument, from exactly that
/// source.
pub fn near_tree_instances(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    center_dir: DVec3,
    radius_m: f64,
    max_n: usize,
) -> Vec<NearTree> {
    near_tree_instances_on_drawn(def, source, albedo, center_dir, radius_m, 0, max_n)
}

/// THE VEGETATION-AGNOSTIC HARVEST: trees at `AGNOSTIC_TREE_DENSITY`.
///
/// For callers that want SOME forest on the ground and do not care how much -
/// the drawn-surface offset probe is the only one. The frame loop must NOT use
/// this: forest density is a shared input to two streams, this harvest and the
/// card bake, and only the frame loop can source it once and hand the same
/// number to both. It calls [`near_tree_instances_at_density`] with
/// `ChunkState::harvest_tree_density`, the value that provably covers every
/// card already on screen.
#[allow(clippy::too_many_arguments)]
pub fn near_tree_instances_on_drawn(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    center_dir: DVec3,
    radius_m: f64,
    drawn_depth: u8,
    max_n: usize,
) -> Vec<NearTree> {
    near_tree_instances_at_density(
        def,
        source,
        albedo,
        center_dir,
        radius_m,
        drawn_depth,
        AGNOSTIC_TREE_DENSITY,
        max_n,
    )
}

/// The tree harvest, standing on the ground you can actually SEE.
///
/// `drawn_depth` is the patch-tree depth of the ground being DRAWN under the
/// camera (`cs.last_drawn.iter().map(|p| p.depth).max()`, the same value
/// `near_grass_instances` takes). Zero means "unknown", and only then does a
/// base come from a direct elevation sample.
///
/// WHY THE DIRECT SAMPLE IS WRONG, measured (v0.1091, on grass): the drawn
/// mesh samples the elevation field at ITS OWN lattice - 3.36 m apart at depth
/// 17 - and the rasteriser interpolates linearly between those samples, while
/// a direct sample lands on whatever f32 heightmap tread it happens to hit.
/// At Fuji the two disagreed by 1.06 m at the 95th percentile. A grass tiller
/// is 30 cm tall so that buried a quarter of the sward; a tree is 4-18 m tall
/// so it hovers instead, which is precisely the operator's screenshot. This
/// harvest was worse off than grass ever was, on two counts: it sampled at a
/// FIXED depth 20 regardless of what was drawn, and it used `tile_or_base`
/// alone - no `DetailNoise` term at all - while every drawn vertex carries the
/// land-masked detail displacement. On detailed ground that is metres.
///
/// [`DrawnPatchSurface`] removes the whole class: it reproduces
/// `build_patch_mesh`'s geometry exactly (same lattice directions, same
/// depth-gated elevation including detail, same flat triangle between them)
/// and returns where a ray from the planet centre leaves that triangle.
///
/// COST: one `DrawnPatchSurface` per harvest (the vertex memo makes repeats
/// nearly free) and one `radius_at` per SURVIVING tree - a few hundred, after
/// the elevation and biome gates have thrown most candidates away.
///
/// `tree_density` is an ARGUMENT, not a global read (v0.1111). It used to be
/// `tree_density()`, a process-wide atomic that this stream and the card bake
/// each sampled for themselves, which cost three separate defects: the two
/// rounded the per-cell count differently (see `trees_in_cell` - 32.51% of
/// northern cells disagreed at density 0.6294), a slider move left the models
/// on one density while the cached cards stayed on another, and two unit tests
/// in this file fought over the atomic under the parallel harness (a ~50% flake
/// in `the_harvest_is_nearest_first_even_when_the_cap_binds`). All three were
/// the same hidden input; passing it in removes the class rather than each
/// instance.
#[allow(clippy::too_many_arguments)]
pub fn near_tree_instances_at_density(
    def: &PlanetDef,
    source: &ElevationSource,
    albedo: Option<&PlanetAlbedo>,
    center_dir: DVec3,
    radius_m: f64,
    drawn_depth: u8,
    tree_density: f32,
    max_n: usize,
) -> Vec<NearTree> {
    let mut out = Vec::new();
    let center = center_dir.normalize();
    let lat_c = center.y.clamp(-1.0, 1.0).asin();
    // No trees on the caps (mirrors the bake's polar gate).
    if lat_c.abs() > 1.5 {
        return out;
    }
    // Gate diagnostics (operator field report 2026-07-25: every recompute
    // returned 0 while cards rendered): count WHY candidates die so the
    // run.log names the guilty gate instead of just "0 trees".
    let mut n_total = 0u32;
    let mut n_out = 0u32;
    let mut n_elev = 0u32;
    let mut n_green = 0u32;
    let mut elev_samples: Vec<f32> = Vec::new();
    let mut green_samples: Vec<[f32; 3]> = Vec::new();
    let lon_c = (-center.z).atan2(center.x);
    let ang = radius_m / def.radius.max(1.0);
    let cos_ang = ang.cos();
    let sea = def.sea_level.clamp(0.0, 1.0);
    let range_m = match source {
        ElevationSource::Heightmap { hm, .. } => hm.max_meters() - hm.min_meters(),
        ElevationSource::Noise(_) => 1.0,
    };
    let bathymetric = matches!(source, ElevationSource::Heightmap { ocean: Some(_), .. });
    let cell = TREE_CELL_RAD;
    // THE DRAWN GROUND (v0.1097). Built once per harvest and pinned to the
    // whole disc, exactly as the grass harvest does it: `set_region` walks the
    // patch-tree levels every query would share - a 460 m disc against a
    // 7,000 km root face, so 10-14 of them - a single time, leaving each tree
    // to walk only the last few. `None` when the caller did not say what depth
    // the ground is drawn at; see the doc above for why that is not
    // recoverable from in here.
    let mut ground = (drawn_depth > 0).then(|| {
        let mut s = DrawnPatchSurface::new(def, source, drawn_depth);
        s.set_region(center, ang + cell);
        s
    });
    let salt: u64 = 0x51F0_A11C;
    let lat_span = ang / cell;
    let lon_span = ang / (cell * lat_c.cos().max(0.05));
    let ylo = ((lat_c / cell).floor() as i64) - lat_span.ceil() as i64 - 1;
    let yhi = ((lat_c / cell).floor() as i64) + lat_span.ceil() as i64 + 1;
    let xlo = ((lon_c / cell).floor() as i64) - lon_span.ceil() as i64 - 1;
    let xhi = ((lon_c / cell).floor() as i64) + lon_span.ceil() as i64 + 1;
    // Walk cells NEAREST-FIRST (v0.969, operator: "I see their shadows, but
    // I can never get close"): the old row-major walk filled the max_n cap
    // from the disc's south-west corner, which was harmless at the old
    // density but at 8x (v0.963) the cap fills before the walk ever reaches
    // the camera's own cell - every drawn model sat in a southern stripe,
    // the card-hide radius engaged anyway, and the player stood on
    // shadowed-but-treeless ground (card shadows persist because the shadow
    // pass's "camera" is the sun, so the hide-discard never fires there).
    // Distance-sorted cells make the cap collect the trees AROUND the
    // camera; lon distance is cos(lat)-weighted to keep rings round.
    let cy = lat_c / cell;
    let cx = lon_c / cell;
    let coslat = lat_c.cos().max(0.05);
    let mut cells: Vec<(i64, i64)> = Vec::with_capacity(
        ((yhi - ylo + 1) * (xhi - xlo + 1)).max(0) as usize,
    );
    for iy in ylo..=yhi {
        for ix in xlo..=xhi {
            cells.push((iy, ix));
        }
    }
    cells.sort_by(|a, b| {
        let d = |c: &(i64, i64)| {
            let dy = c.0 as f64 + 0.5 - cy;
            let dx = (c.1 as f64 + 0.5 - cx) * coslat;
            dy * dy + dx * dx
        };
        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    for (iy, ix) in cells {
        let cell_lat = (iy as f64 + 0.5) * cell;
        // THE shared count (v0.1111), the same call the card bake makes. This
        // used to be a second, single-rounded copy of that arithmetic living
        // here; `count` is not just a loop bound, it is the right-hand side of
        // the survival gate below, so a one-tree difference changes WHICH trees
        // exist and leaves cards with no model. See `trees_in_cell`.
        let count = trees_in_cell(tree_density, cell_lat);
        {
            // Identical stream to the bake: 6 randoms per item BEFORE any
            // gate, so positions/looks agree exactly with the cards.
            let mut s = (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (iy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                ^ salt;
            if s == 0 {
                s = 0x94D0_49BB_1331_11EB;
            }
            let mut next = move || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            for item in 0..count {
                let r0 = next();
                let r1 = next();
                let r2 = next();
                let r3 = next();
                let _r4 = next();
                let r5 = next();
                // NOTE deliberately NO early return on `max_n` here. There
                // was one, and it made the "nearest-first" sort below
                // unreachable in EVERY saturated frame - measured across
                // 3587 logged frames, the cap bound in all of them, so the
                // caller's 256-model draw budget was spent in raw CELL-WALK
                // ORDER: entry #256 could land 162 m out while a tree 30 m
                // away got nothing. That one bug was the operator's
                // "billboards phase out of existence" AND most of "open
                // field, then suddenly a forest". The harvest now collects
                // the whole disc and truncates AFTER sorting; the disc is
                // radius-bounded, so the superset is a few thousand at
                // worst, and the sort is the cost of being correct.
                n_total += 1;
                let lat = (iy as f64 + (r0 % 4096) as f64 / 4096.0) * cell;
                let lon = (ix as f64 + (r1 % 4096) as f64 / 4096.0) * cell;
                let cl = lat.cos();
                let dir = DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin());
                if dir.dot(center) < cos_ang {
                    n_out += 1;
                    continue;
                }
                let (e, _tile) = match source {
                    ElevationSource::Heightmap { hm, tiles, .. } => {
                        // Depth 20 = the deepest bake tier, so the tile-
                        // gated sampler matches close patches.
                        tile_or_base(hm, *tiles, dir, 20)
                    }
                    ElevationSource::Noise(sm) => (sm.elevation_at(dir.as_vec3()), false),
                };
                let elev_m = (e - sea) * range_m;
                if elev_m < 6.0 || elev_m > TREELINE_M {
                    n_elev += 1;
                    if elev_samples.len() < 4 {
                        elev_samples.push(elev_m);
                    }
                    continue;
                }
                // v0.1108: the biome gate is a DENSITY WEIGHT now, thinned by
                // the item's index in the cell stream. MUST stay byte-identical
                // to the card bake's copy in `build_patch_mesh` - the model has
                // to hide its own card, so if the two streams disagree about
                // which items survive, a tree and its billboard both draw.
                let sc = surface_color(def, albedo, dir.as_vec3(), e);
                let vw = veg_biome_weight(sc);
                if vw < VEG_WEIGHT_MIN || (item as f32) >= count as f32 * vw {
                    n_green += 1;
                    if green_samples.len() < 4 {
                        green_samples.push(sc);
                    }
                    continue;
                }
                let yaw = (r2 % 6283) as f32 / 1000.0;
                // v0.913: wider size spread (operator: "varied size... they all
                            // seem uniform height"). 4-18 m, skewed toward
                            // younger trees; BOTH stream sites (bake + the
                            // near-model mirror) must stay identical.
                            // v0.1066: MUST match the bake site above exactly -
                            // same inputs, same helper - or a tree changes
                            // species as you walk toward it.
                            let sp_i = pick_tree_species(def, dir, elev_m, r5);
                            let (sp_h, sp_jit) = match crate::renderer::tree_mesh::registry().get(sp_i)
                            {
                                Some(t) => (t.height_m, t.height_jitter),
                                None => (22.0, 0.12),
                            };
                            let jitter =
                                1.0 - sp_jit + (r3 % 100) as f32 / 100.0 * (sp_jit * 2.0);
                            let h = sp_h * jitter;
                // WHERE THE TRUNK MEETS THE GROUND. Interpolated off the drawn
                // patch face when the caller knows what depth the ground is
                // drawn at, and only otherwise from the direct elevation
                // sample the gates above already took (v0.1096 behaviour, kept
                // bit-identical so the unwired path cannot regress).
                //
                // The sink needs `h`, which is why the radius is computed down
                // here rather than beside the gates: a tree's root flare - and
                // so how far it can be pushed into the ground before the flare
                // stops reading - scales with its height.
                let r = match ground.as_mut() {
                    Some(g) => {
                        g.radius_at(dir) - tree_flare_radius_m(h) * TREE_GROUND_SINK_FLARE_FRAC
                    }
                    None => {
                        def.radius
                            * if bathymetric {
                                displaced_radius_f64_true(def, e as f64)
                            } else {
                                displaced_radius_f64(def, e as f64)
                            }
                    }
                };
                out.push(NearTree {
                    dir,
                    r_m: r,
                    yaw,
                    height_m: h,
                    species: sp_i as u8,
                    variant: ((r5 >> 11) % 3) as u8,
                });
            }
        }
    }
    // Gate autopsy in the log whenever the harvest comes back empty-handed
    // but candidates existed - names the guilty gate with sample values.
    if out.is_empty() && n_total > 0 {
        log::info!(
            "[NearTree] gates: {} candidates, {} outside radius, {} elev-gated \
             (samples {:?} m), {} green-gated (samples {:?})",
            n_total,
            n_out,
            n_elev,
            elev_samples,
            n_green,
            green_samples
        );
    }
    // Nearest-first so the caller's draw cap keeps the trees beside the
    // camera, not whichever cell enumerated first - and nearest by TRUE 3D
    // DISTANCE to the camera point, the same quantity the draw clip tests,
    // not the angular dot(dir, center) proxy it used to be (which ignores
    // radius and misorders on slopes).
    //
    // The camera stands on the ground at `center`. Every tree's base is
    // `dir * r_m` in the same planet-local frame, so putting the camera at
    // the MEAN ground radius of the harvest makes the key a true 3D chord
    // rather than a great-circle arc. Only the ORDER matters, so a constant
    // camera radius is exact for ranking even where terrain undulates.
    let cam_r = if out.is_empty() {
        0.0
    } else {
        out.iter().map(|t| t.r_m).sum::<f64>() / out.len() as f64
    };
    let cam = center * cam_r;
    out.sort_by(|a, b| {
        let da = (a.dir * a.r_m - cam).length_squared();
        let db = (b.dir * b.r_m - cam).length_squared();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    // Truncate AFTER the sort, which is the whole point.
    out.truncate(max_n);
    out
}

#[cfg(test)]
mod near_tree_order_tests {
    use super::*;
    use crate::terrain::planet_albedo::PlanetAlbedo;
    use crate::terrain::planet_heightmap::PlanetHeightmap;

    fn real_earth() -> (PlanetHeightmap, PlanetAlbedo, PlanetDef) {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin"))
            .expect("earth heightmap loads");
        let albedo =
            PlanetAlbedo::load(&base.join("earth_albedo.bin")).expect("earth albedo loads");
        let mut def = super::tests::earth_like();
        def.sea_level = hm.sea_level_normalized();
        (hm, albedo, def)
    }

    /// THE HARVEST MUST RETURN ITS TREES NEAREST-FIRST, and it must still be
    /// true when the cap BINDS - which is the only case the caller cares about.
    ///
    /// This test did not exist, and its absence let a comment lie for about 18
    /// releases. `near_tree_instances_on_drawn` ended with a "nearest-first"
    /// sort and a comment saying the draw cap would therefore keep the trees
    /// beside the camera - but an early `return out` fired the moment the
    /// harvest hit `max_n`, several hundred lines ABOVE that sort. Measured
    /// across 3587 logged frames the cap bound in every single one, so the
    /// sort never ran in a shipped frame and the caller's 256-model budget was
    /// spent in raw cell-walk order. Cells enumerate at 220 m quantisation, so
    /// entry #256 could sit 162 m away while a tree 30 m from the player got
    /// neither a model nor a card.
    ///
    /// That is why the operator saw billboards "phase out of existence" instead
    /// of promoting to a mesh, and why walking produced "a big open field, then
    /// suddenly a forest".
    ///
    /// Requesting a SMALL max_n is the whole point: a test that only checks the
    /// unsaturated path passes on the broken code.
    #[test]
    fn the_harvest_is_nearest_first_even_when_the_cap_binds() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let source = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let (lat, lon) = (35.29_f64.to_radians(), 138.79_f64.to_radians());
        let center =
            DVec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()).normalize();
        // Density passed IN (v0.1111). While it came from the process-global
        // atomic this test read whatever a CONCURRENT test had last stored -
        // `the_card_hide_radius_never_outruns_the_models` wrote 0.6 into it -
        // and failed about half the time under the default parallel harness.
        let harvest = |max_n: usize| {
            near_tree_instances_at_density(
                &def,
                &source,
                Some(&albedo),
                center,
                240.0,
                17,
                0.6,
                max_n,
            )
        };

        // The full sorted harvest, then the capped ones. The contract is that
        // a cap keeps a PREFIX of the sorted list - which is exactly what the
        // caller's draw budget relies on and exactly what the early return
        // destroyed. Comparing against the full list avoids re-deriving the
        // distance key in the test, which would only ever test my arithmetic
        // against itself.
        let full = harvest(100_000);
        assert!(
            full.len() > 300,
            "fixture must produce a big harvest to be meaningful, got {}",
            full.len()
        );

        for max_n in [16usize, 64, 256] {
            let capped = harvest(max_n);
            assert_eq!(
                capped.len(),
                max_n,
                "max_n {max_n}: the cap must BIND for this test to mean anything"
            );
            for (k, (c, f)) in capped.iter().zip(full.iter()).enumerate() {
                assert!(
                    (c.dir - f.dir).length() < 1e-12 && (c.r_m - f.r_m).abs() < 1e-6,
                    "max_n {max_n}: tree {k} is not the one the full sorted harvest                      puts there. A cap must keep the NEAREST trees; before v0.1107 an                      early return fired hundreds of lines above the sort, so in every                      saturated frame - measured, all 3587 of them - the 256-model draw                      budget was spent in raw cell-walk order and left a treeless ring                      around the player."
                );
            }
        }

        // And the sort is real: the full list must be non-decreasing in the
        // same key the sort used.
        let cam_r = full.iter().map(|t| t.r_m).sum::<f64>() / full.len() as f64;
        let cam = center * cam_r;
        let d = |t: &NearTree| (t.dir * t.r_m - cam).length();
        let mut worst = 0.0_f64;
        for w in full.windows(2) {
            worst = worst.max(d(&w[0]) - d(&w[1]));
        }
        assert!(
            worst < 1e-6,
            "the harvest is NOT sorted nearest-first - out of order by {worst:.3} m"
        );
    }

    /// One frame's worth of the lib.rs near-tree block, reduced to the two
    /// numbers that decide whether a tree is visible at all.
    struct FrameOutcome {
        /// Indices (into the uncapped harvest) that got a 3D model.
        drawn: std::collections::HashSet<usize>,
        /// The radius the terrain cards were told to discard inside.
        hide_m: f64,
    }

    /// Replay the frame loop's model/card handoff for one camera position.
    ///
    /// `old_rule` selects the pre-fix hide radius (farthest DRAWN tree) so the
    /// test can be shown to go red on the code it was written against; the
    /// shipped path is `ModelCoverage`.
    fn run_frame(
        harvest: &[NearTree],
        cap: usize,
        cam_local: DVec3,
        tree_dist: f64,
        budget: u32,
        old_rule: bool,
    ) -> FrameOutcome {
        // `near_tree_instances_on_drawn` truncates AFTER sorting, so the set
        // the frame loop holds is exactly this prefix - no second harvest.
        let td2 = tree_dist * tree_dist;
        let mut drawn_n = 0u32;
        let mut drawn = std::collections::HashSet::new();
        let mut cov = ModelCoverage::new(tree_dist);
        let mut old_covered2 = 0.0_f64;
        for i in 0..harvest.len().min(cap) {
            let d2 = (harvest[i].dir * harvest[i].r_m - cam_local).length_squared();
            if d2 > td2 {
                continue;
            }
            if drawn_n >= budget {
                cov.uncovered(d2);
                continue;
            }
            drawn_n += 1;
            drawn.insert(i);
            cov.drew(d2);
            if d2 > old_covered2 {
                old_covered2 = d2;
            }
        }
        let hide_m = if old_rule {
            if drawn_n == 0 {
                0.0
            } else {
                (old_covered2.sqrt() - TREE_CARD_HANDOFF_OVERLAP_M).clamp(0.0, tree_dist)
            }
        } else {
            cov.hide_radius_m() as f64
        };
        FrameOutcome { drawn, hide_m }
    }

    /// THE CARD HIDE RADIUS IS A PROMISE, AND THIS WALKS A PLAYER ACROSS IT.
    ///
    /// `renderer.tree_card_hide_m` tells the fragment shader "every tree inside
    /// here has a 3D model, so discard its silhouette card". A tree inside that
    /// radius with no model has NO representation at all - it is gone. The
    /// operator, 2026-08-03: "the billboards for the lower LOD trees just kind
    /// of phase out of existence instead of actually shifting to a higher LOD."
    ///
    /// WHY A WALK AND NOT A SINGLE FRAME. Immediately after a harvest the drawn
    /// set IS a clean nearest-first disc and the old max-of-drawn rule is
    /// exactly right; a one-frame test passes on the broken code. The harvest
    /// re-sorts only every 12 m of walking (lib.rs hysteresis) while the camera
    /// keeps going, so trees ahead of the player overtake trees the sort ranked
    /// before them, and once a cap binds the drawn set stops being a disc. The
    /// gap therefore opens gradually between harvests and resets at each one -
    /// only a moving camera sees it.
    ///
    /// Two configurations, because the two caps are different bugs: the shipped
    /// 120 m / 256 defaults let the DRAW BUDGET bind, and 400 m / 1024 makes the
    /// HARVEST cap bind instead (which is why raising the budget alone did
    /// nothing - see `near_tree_harvest_cap`).
    #[test]
    fn the_card_hide_radius_never_outruns_the_models() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let source = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // The shipped Settings default, passed IN (v0.1111). This used to be
        // `TREE_DENSITY_BITS.store(0.6, ...)`, a write to a process-global that
        // every other test in this binary was reading - it landed mid-run
        // inside `the_harvest_is_nearest_first_even_when_the_cap_binds` under
        // the parallel harness and failed it about half the time
        // (`-- --test-threads=1` always passed, which is the signature). The
        // flake was not a harness problem, it was the hidden input: two tests
        // could disagree about the density for the same reason the card bake
        // and this harvest could.
        const DENSITY: f32 = 0.6;
        let (lat, lon) = (35.29_f64.to_radians(), 138.79_f64.to_radians());
        let start =
            DVec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()).normalize();
        let east = DVec3::Y.cross(start).normalize();
        // `old_rule = true` reproduces the shipped-through-v0.1110 radius, so
        // the same walk can be asserted to FIND the bug as well as to be free
        // of it. A gate that has never been seen red is not a gate.
        for old_rule in [false, true] {
            let mut found_orphans = 0usize;
            for (tree_dist, budget) in [(120.0_f64, 256u32), (400.0, 1024)] {
                let cap = near_tree_harvest_cap(budget);
                let mut harvest_center = DVec3::splat(f64::MAX);
                let mut harvest: Vec<NearTree> = Vec::new();
                let mut ground_r = def.radius;
                for step in 0..40 {
                    let cam_dir = (start + east * (step as f64 / def.radius)).normalize();
                    let cam_local = cam_dir * (ground_r + 1.7);
                    if (harvest_center - cam_local).length() > 12.0 {
                        // Uncapped: this is the CARD set (the patch bake walks
                        // the same cells through the same gates), and its
                        // prefix is what the frame loop actually holds.
                        harvest = near_tree_instances_at_density(
                            &def,
                            &source,
                            Some(&albedo),
                            cam_local.normalize(),
                            tree_dist + 60.0,
                            17,
                            DENSITY,
                            usize::MAX,
                        );
                        assert!(
                            harvest.len() > cap,
                            "fixture must saturate the cap ({} trees vs cap {cap}) or the \
                             handoff is never under load",
                            harvest.len()
                        );
                        ground_r =
                            harvest.iter().map(|t| t.r_m).sum::<f64>() / harvest.len() as f64;
                        harvest_center = cam_local;
                    }
                    let out =
                        run_frame(&harvest, cap, cam_local, tree_dist, budget, old_rule);
                    let mut orphans = 0usize;
                    let mut nearest = f64::MAX;
                    for (i, t) in harvest.iter().enumerate() {
                        let d = (t.dir * t.r_m - cam_local).length();
                        if d < out.hide_m && !out.drawn.contains(&i) {
                            orphans += 1;
                            nearest = nearest.min(d);
                        }
                    }
                    found_orphans += orphans;
                    if !old_rule {
                        assert_eq!(
                            orphans, 0,
                            "model radius {tree_dist} m, budget {budget}, step {step} m: \
                             {orphans} trees sit inside the {:.1} m card-hide radius with no \
                             3D model - the nearest at {nearest:.1} m. Those trees draw \
                             NOTHING: their card discarded because the hide radius promised a \
                             model that the draw budget (or the harvest cap) never delivered.",
                            out.hide_m
                        );
                    }
                }
            }
            if old_rule {
                assert!(
                    found_orphans > 0,
                    "the pre-v0.1111 rule (hide radius = farthest DRAWN tree) produced NO \
                     orphans on this walk, so this test could not have caught the bug it was \
                     written for. Either the fixture stopped saturating the caps or the walk \
                     stopped outrunning the harvest - fix the fixture, do not delete the check."
                );
            }
        }
    }

    /// The helper above can be perfect and never be called. This checks that
    /// the frame loop actually computes its card-hide radius through
    /// `ModelCoverage`, and that the old max-of-drawn accumulator is gone.
    ///
    /// The v0.1107 fix to this same handoff shipped with exactly this shape of
    /// hole: `near_tree_instances_on_drawn` grew a correct nearest-first sort
    /// while an early return several hundred lines above it kept the sort from
    /// ever running. A source check is crude, but it is the only thing standing
    /// between "the module is right" and "the picture is right".
    #[test]
    fn the_frame_loop_uses_the_measured_coverage_radius() {
        const PENDING: &str = "PENDING WIRING REQUEST (near-tree LOD handoff). This is not a \
             regression you caused: the fix lives in terrain::near_trees and the five-hunk \
             src/lib.rs edit that calls it has not been applied yet. ";
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("lib.rs"),
        )
        .expect("lib.rs reads");
        assert!(
            src.contains("ModelCoverage::new("),
            "{PENDING}src/lib.rs does not build a terrain::near_trees::ModelCoverage. The \
             near-tree block must derive tree_card_hide_m from the NEAREST TREE IT MISSED, \
             not from the farthest one it drew - see ModelCoverage's doc for the measurement."
        );
        assert!(
            src.contains("hide_radius_m()"),
            "{PENDING}src/lib.rs never asks ModelCoverage for the hide radius"
        );
        assert!(
            !src.contains("covered_r2"),
            "{PENDING}src/lib.rs still carries the `covered_r2` running MAX of drawn-tree \
             distance. That is the pre-fix rule: it promises the card layer a model at every \
             tree inside the FARTHEST drawn one, which is false the moment the draw order is \
             not perfectly nearest-first."
        );
        assert!(
            src.contains("near_tree_harvest_cap("),
            "{PENDING}src/lib.rs still passes a literal max_n to \
             near_tree_instances_on_drawn. A cap below the draw budget is an invisible second \
             handoff line the draw loop cannot see - use \
             terrain::near_trees::near_tree_harvest_cap(budget)."
        );
    }
}

/// THE TWO VEGETATION STREAMS MUST AGREE ABOUT WHICH TREES EXIST.
///
/// The card bake (`build_patch_mesh_at_density`) and this model harvest walk
/// the same planet-fixed cell grid through the same gates, and the near-field
/// LOD handoff is built on the assumption that they land on the SAME SET: a
/// model hides its own card, and `tree_card_hide_m` promises the shader that
/// every card inside it has one. A card the models missed does not just lose
/// its mesh - it discards in the colour pass and goes on casting shade, because
/// the shadow pass deliberately does not mirror that discard.
#[cfg(test)]
mod tree_stream_agreement_tests {
    use super::*;
    use crate::terrain::planet_albedo::PlanetAlbedo;
    use crate::terrain::planet_heightmap::PlanetHeightmap;

    fn real_earth() -> (PlanetHeightmap, PlanetAlbedo, PlanetDef) {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin"))
            .expect("earth heightmap loads");
        let albedo =
            PlanetAlbedo::load(&base.join("earth_albedo.bin")).expect("earth albedo loads");
        let mut def = super::tests::earth_like();
        def.sea_level = hm.sea_level_normalized();
        (hm, albedo, def)
    }

    fn dir_of(lat_deg: f64, lon_deg: f64) -> DVec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        DVec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()).normalize()
    }

    fn inside_patch(id: &PatchId, d: DVec3) -> bool {
        let c = patch_corners(id);
        let cn = [c[0].normalize(), c[1].normalize(), c[2].normalize()];
        let en = [cn[0].cross(cn[1]), cn[1].cross(cn[2]), cn[2].cross(cn[0])];
        let es = [en[0].dot(cn[2]), en[1].dot(cn[0]), en[2].dot(cn[1])];
        (0..3).all(|i| en[i].dot(d) * es[i] >= 0.0)
    }

    /// Descend from the root face into the patch that covers `dir`.
    fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
        let d = dir.normalize();
        let mut id = (0..20u8)
            .map(PatchId::root)
            .find(|r| inside_patch(r, d))
            .expect("some root face contains the direction");
        while id.depth < depth {
            id = (0..4u32)
                .map(|c| id.child(c))
                .find(|ch| inside_patch(ch, d))
                .expect("some child contains the direction");
        }
        id
    }

    /// EVERY TREE CARD THE BAKE PUT IN THIS PATCH, as a planet-local unit
    /// direction - read back out of the finished mesh, not re-derived.
    ///
    /// WHY THIS RECOVERS THE DIRECTION EXACTLY. A card's first two emitted
    /// corners always straddle its own vertical axis: the sprite emitter pushes
    /// `c00 = foot - side*(s/2)` then `c10 = foot + side*(s/2)`, and the
    /// coloured fallback pushes `p00 = base + up*h0 - side*(w/2)` then
    /// `p01 = base + up*h0 + side*(w/2)`. Either midpoint therefore lands ON
    /// the tree's radial line, and a point displaced radially has exactly the
    /// tree's direction. So no card height, footprint, drop or ground radius
    /// has to be reproduced here - the one quantity the two streams must agree
    /// about comes straight off the geometry the GPU will draw.
    fn card_dirs(pm: &PatchMesh) -> Vec<DVec3> {
        let corners: Vec<glam::Vec3> = pm
            .mesh
            .vertices
            .iter()
            .filter(|v| v.tree_card)
            .map(|v| glam::Vec3::from(v.position))
            .collect();
        assert_eq!(
            corners.len() % 4,
            0,
            "tree cards are emitted four vertices at a time; got {}",
            corners.len()
        );
        corners
            .chunks(4)
            .map(|q| (pm.anchor + ((q[0] + q[1]) * 0.5).as_dvec3()).normalize())
            .collect()
    }

    /// Two trees in a cell are quantised no finer than `TREE_CELL_RAD / 4096`
    /// = 8.4e-9 rad apart, and reading a direction back off an f32 vertex a few
    /// hundred metres from its anchor costs about 3e-12 rad. 1e-9 sits two
    /// orders clear of both.
    const DIR_TOL: f64 = 1e-9;

    fn dedup(dirs: &[DVec3]) -> Vec<DVec3> {
        let mut out: Vec<DVec3> = Vec::new();
        for d in dirs {
            if !out.iter().any(|o| (*o - *d).length() < DIR_TOL) {
                out.push(*d);
            }
        }
        out
    }

    /// Members of `a` with no partner in `b`.
    fn missing_from(a: &[DVec3], b: &[DVec3]) -> Vec<DVec3> {
        a.iter()
            .filter(|x| !b.iter().any(|y| (**x - *y).length() < DIR_TOL))
            .cloned()
            .collect()
    }

    /// Every depth-`depth` descendant of `ancestor`.
    fn leaves_under(ancestor: &PatchId, depth: u8) -> Vec<PatchId> {
        let mut cur = vec![*ancestor];
        while cur[0].depth < depth {
            cur = cur
                .iter()
                .flat_map(|id| (0..4u32).map(|c| id.child(c)).collect::<Vec<_>>())
                .collect();
        }
        cur
    }

    /// The card bake's per-cell count through v0.1110: round to a per-cell
    /// figure first, then thin THAT by cos(lat).
    fn old_bake_count(d: f32, lat: f64) -> u32 {
        let per_cell = (((TREES_PER_CELL as f32) * d).round() as u32).max(1);
        ((per_cell as f64) * lat.cos().max(0.0)).round() as u32
    }

    /// The near-model harvest's per-cell count through v0.1110: one round, at
    /// the end. Every cell row where this differs from `old_bake_count` is a
    /// row whose cards and models were drawn from different trees.
    fn old_harvest_count(d: f32, lat: f64) -> u32 {
        ((TREES_PER_CELL as f64) * (d as f64) * lat.cos().max(0.0)).round() as u32
    }

    /// THE TEST. One real patch of real Earth, five densities, and the two
    /// streams must produce the SAME TREES.
    ///
    /// 0.6294 and 0.6295 are in the list because they are measured
    /// counter-examples to the pre-v0.1111 code, not decoration: with the
    /// harvest rounding once and the bake rounding twice, 32.51% and 26.69% of
    /// northern cells respectively came out one tree apart. 0.6 is in the list
    /// because it is the shipped default and the ONE value where the old code
    /// happened to agree everywhere - which is exactly why this shipped for so
    /// long, and why a test written only at the default would have passed on
    /// the broken code.
    ///
    /// The slider does not snap (`*value = min + t * (max - min)` in
    /// `widgets::labeled_slider`), so every f32 in 0.1..=1.0 is reachable by
    /// dragging, and the operator drags it.
    ///
    /// WHY A WHOLE REGION AND NOT ONE PATCH, AND WHY THIS EXACT LATITUDE.
    /// `count` depends on the cell ROW, and a single depth-15 patch - the
    /// largest that carries cards - is about 215 m across against a 220 m cell,
    /// so it samples one or two rows and a handful of cells. Written that way
    /// this test PASSED against the reverted formula: the cells under 35.29 N
    /// simply were not among the 32.51% that split. The split rows are not
    /// scattered either, they come in bands (468 runs over the northern
    /// hemisphere at 0.6294, tens of rows each), so a small region lands wholly
    /// inside a band or wholly outside one. 35.234 N sits in the middle of a
    /// band that BOTH counter-example densities share - rows 17809..17840,
    /// 35.187 N to 35.265 N - and the 16 depth-15 leaves of the depth-13
    /// ancestor there span about 4 of those rows. The fixture guard at the
    /// bottom refuses to let the test pass if that ever stops being true.
    #[test]
    fn the_card_bake_and_the_model_harvest_agree_on_which_trees_exist() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // The Fuji foothills, nudged 6 km south onto the shared split band.
        let region = patch_containing(dir_of(35.234, 138.79), TREE_MIN_DEPTH - 2);
        let leaves = leaves_under(&region, TREE_MIN_DEPTH);
        let cn = patch_corners(&region);
        let center = (cn[0].normalize() + cn[1].normalize() + cn[2].normalize()).normalize();
        let radius_m = 900.0_f64;
        let ang = radius_m / def.radius;
        // Cell rows the region covers, so the guard can ask whether any of them
        // is a row the two old formulas counted differently.
        let lat_lo = cn.iter().map(|c| c.normalize().y.asin()).fold(f64::MAX, f64::min);
        let lat_hi = cn.iter().map(|c| c.normalize().y.asin()).fold(f64::MIN, f64::max);
        let rows: Vec<f64> = {
            let (a, b) = ((lat_lo / TREE_CELL_RAD).floor() as i64, (lat_hi / TREE_CELL_RAD).floor() as i64);
            (a..=b).map(|iy| (iy as f64 + 0.5) * TREE_CELL_RAD).collect()
        };

        let mut exercised = 0usize;
        for density in [0.6_f32, 0.6294, 0.6295] {
            let split_rows = rows
                .iter()
                .filter(|lat| old_bake_count(density, **lat) != old_harvest_count(density, **lat))
                .count();
            exercised += split_rows;
            let cards = dedup(
                &leaves
                    .iter()
                    .flat_map(|id| {
                        card_dirs(&build_patch_mesh_at_density(
                            &def,
                            &src,
                            Some(&albedo),
                            id,
                            density,
                        ))
                    })
                    .collect::<Vec<_>>(),
            );
            assert!(
                cards.len() > 200,
                "density {density}: only {} cards across {} patches - the fixture is not a \
                 forest, so agreeing about it would prove nothing",
                cards.len(),
                leaves.len(),
            );
            // The harvest disc has to CONTAIN the region or the comparison is
            // measuring the disc edge instead of the two streams.
            let worst = cards
                .iter()
                .map(|c| c.dot(center).clamp(-1.0, 1.0).acos())
                .fold(0.0_f64, f64::max);
            assert!(
                worst < ang * 0.8,
                "fixture: a card sits {worst:.3e} rad out, too close to the {ang:.3e} rad \
                 harvest disc edge"
            );

            let harvest = near_tree_instances_at_density(
                &def,
                &src,
                Some(&albedo),
                center,
                radius_m,
                TREE_MIN_DEPTH,
                density,
                usize::MAX,
            );
            let models: Vec<DVec3> = harvest
                .iter()
                .map(|t| t.dir)
                .filter(|d| inside_patch(&region, *d))
                .collect();

            // Cards with no model: the operator's "chunks that wouldn't spawn
            // trees visually but were still affecting the light".
            let orphans = missing_from(&cards, &models);
            // Models with no card: harmless on its own (the model IS the better
            // LOD) but it means the streams parted company, so it fails too.
            let ghosts = missing_from(&models, &cards);
            assert!(
                orphans.is_empty() && ghosts.is_empty(),
                "density {density}: the card bake and the model harvest disagree about which \
                 trees exist - {} of {} cards have no model, {} of {} models have no card. \
                 Both streams feed `count` into the survival gate \
                 `(item as f32) >= count as f32 * vw`, so a one-tree difference in the per-cell \
                 count does not shift the count alone, it changes WHICH items live. Every card \
                 in the orphan list draws nothing inside tree_card_hide_m and goes on casting a \
                 shadow. Route both through `trees_in_cell`.",
                orphans.len(),
                cards.len(),
                ghosts.len(),
                models.len(),
            );
        }

        // THE FIXTURE GUARD. Agreement is only evidence while the region
        // actually spans cell rows the two old roundings split - otherwise this
        // test passes on the broken code, which is exactly what the one-patch
        // version of it did.
        assert!(
            exercised >= 3,
            "the tested region covers only {exercised} cell rows that the pre-v0.1111 \
             roundings disagreed about, across all densities. This test then proves nothing: \
             move the site or widen the region until it does. Do not delete the guard."
        );
    }

    /// THE SHARED COUNT IS THE BAKE'S ROUNDING, AND THE HARVEST'S OLD ROUNDING
    /// REALLY DID DIVERGE.
    ///
    /// Two jobs. First it pins `trees_in_cell` to the formula the shipped cards
    /// were baked with, written out here independently, so a future refactor
    /// cannot quietly re-round the forest on every planet at every density.
    /// Second it MEASURES the divergence the shared helper removed, which is
    /// what stops the test above from being a tautology: if the two roundings
    /// had agreed everywhere, agreement between the streams would prove
    /// nothing.
    #[test]
    fn the_shared_count_is_the_bake_rounding_and_the_old_harvest_rounding_diverged() {
        // The card bake's formula through v0.1110: round to a per-cell count
        // first, then thin THAT by cos(lat).
        let bake = |d: f32, lat: f64| -> u32 {
            let per_cell = (((TREES_PER_CELL as f32) * d).round() as u32).max(1);
            ((per_cell as f64) * lat.cos().max(0.0)).round() as u32
        };
        // The near-model harvest's formula through v0.1110: one round, at the
        // end. Every cell where these two differ is a cell whose cards and
        // models were different trees.
        let old_harvest = |d: f32, lat: f64| -> u32 {
            ((TREES_PER_CELL as f64) * (d as f64) * lat.cos().max(0.0)).round() as u32
        };

        let cells = (1.5 / TREE_CELL_RAD) as i64; // the northern hemisphere's cells
        assert_eq!(cells, 43_478, "cell grid changed - re-measure the divergence below");

        for density in [0.1_f32, 0.6, 0.6294, 0.6295, 0.75, 1.0] {
            for iy in [0_i64, 1, 7, 1000, 20_000, cells - 1] {
                let lat = (iy as f64 + 0.5) * TREE_CELL_RAD;
                assert_eq!(
                    trees_in_cell(density, lat),
                    bake(density, lat),
                    "trees_in_cell no longer reproduces the shipped CARD count at density \
                     {density}, cell {iy}. That number decides which trees exist on every \
                     planet; changing it is a forest-wide change, never a cleanup."
                );
            }
        }

        let disagree = |d: f32| -> usize {
            (0..cells)
                .filter(|iy| {
                    let lat = (*iy as f64 + 0.5) * TREE_CELL_RAD;
                    trees_in_cell(d, lat) != old_harvest(d, lat)
                })
                .count()
        };
        // The default is very nearly - but NOT quite - safe, which is why this
        // survived: exactly one cell row in the northern hemisphere split, at
        // iy 10727, 0.370 rad = 21.2 degrees north (Mexico, India, Vietnam),
        // where the bake counted 447 trees and the harvest 448. 1 of 43,478 is
        // 0.0023%, which rounds to the 0.00% the finding reported - but it is a
        // whole latitude band of the real Earth where every card had no model.
        let at_default = disagree(0.6);
        assert_eq!(
            at_default, 1,
            "the two roundings agreed at the 0.6 default everywhere except one cell row, \
             which is why this bug survived - if that is no longer true the measurements \
             below need redoing"
        );
        for (d, floor) in [(0.6294_f32, 0.30_f64), (0.6295, 0.24)] {
            let frac = disagree(d) as f64 / cells as f64;
            assert!(
                frac > floor,
                "density {d}: only {:.2}% of the {cells} northern cells split between the two \
                 old roundings (measured 32.51% at 0.6294, 26.69% at 0.6295). The agreement \
                 test above is only meaningful while these densities are real counter-examples.",
                frac * 100.0
            );
        }
    }

    /// THE SPLIT-BRAIN INVARIANT: the harvest density must cover every card
    /// still on screen, not merely match the slider.
    ///
    /// A patch is baked ONCE and drawn until it leaves the cache; this harvest
    /// re-runs every 12 m of walking. So the instant the slider moves, "the
    /// density" is two different numbers - what the cards were built with, and
    /// what the setting says now - and lib.rs's own comment describes the
    /// consequence as intended behaviour: "existing patches keep their density
    /// until they rebuild, so a slider change appears patch by patch as you
    /// move." That is fine for a layer on its own. It is not fine for two
    /// layers that have to name the same trees.
    ///
    /// `ChunkState::harvest_tree_density` answers with the maximum over the
    /// drawn patches, which works because the enumeration is MONOTONE: each
    /// item's randoms depend only on its index, and both the loop bound and the
    /// survival threshold rise with `count`, so a higher density yields a
    /// superset. This test proves the naive answer (just use the setting)
    /// really does orphan cards, and that the shipped answer does not.
    #[test]
    fn the_harvest_density_covers_every_card_still_on_screen() {
        // The cards were baked before the operator dragged the slider down.
        const BAKED_AT: f32 = 0.9;
        const SETTING_NOW: f32 = 0.3;

        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let id = patch_containing(dir_of(35.29, 138.79), TREE_MIN_DEPTH);
        let cn = patch_corners(&id);
        let center = (cn[0].normalize() + cn[1].normalize() + cn[2].normalize()).normalize();
        let pm = build_patch_mesh_at_density(&def, &src, Some(&albedo), &id, BAKED_AT);
        assert_eq!(
            pm.tree_density, BAKED_AT,
            "a patch that emits cards must record the density it emitted them at"
        );
        let cards = dedup(&card_dirs(&pm));
        assert!(cards.len() > 40, "fixture: only {} cards", cards.len());

        // What the frame loop knows: this patch is on screen, and the mesh says
        // what its cards are.
        let mut cs = ChunkState::new(def.terrain_seed);
        cs.insert_built(id, 0, None, 1, pm.anchor, pm.band, pm.tree_density);
        cs.last_drawn.insert(id);

        let models_at = |density: f32| -> Vec<DVec3> {
            near_tree_instances_at_density(
                &def,
                &src,
                Some(&albedo),
                center,
                600.0,
                id.depth,
                density,
                usize::MAX,
            )
            .iter()
            .map(|t| t.dir)
            .filter(|d| inside_patch(&id, *d))
            .collect()
        };

        // The rule that shipped: harvest at whatever the slider says now.
        let orphaned = missing_from(&cards, &models_at(SETTING_NOW));
        assert!(
            !orphaned.is_empty(),
            "harvesting at the live setting while the cached cards were baked at {BAKED_AT} \
             left NO orphans, so this test cannot detect the split-brain it was written for - \
             fix the fixture (is the patch still carrying cards at both densities?), do not \
             delete the check"
        );

        // The rule this module ships.
        let covering = cs.harvest_tree_density(SETTING_NOW);
        assert_eq!(
            covering, BAKED_AT,
            "harvest_tree_density must not fall below a drawn patch's own bake density"
        );
        assert!(
            missing_from(&cards, &models_at(covering)).is_empty(),
            "{} of {} cards on screen still have no model at the covering density {covering}. \
             Those cards discard inside tree_card_hide_m and keep casting shade - the \
             operator's \"chunks that wouldn't spawn trees visually but, they were still \
             affecting the light\".",
            missing_from(&cards, &models_at(covering)).len(),
            cards.len(),
        );

        // And once the stale patch is off screen the setting takes over again,
        // so the slider is not permanently pinned by one old patch.
        cs.last_drawn.clear();
        assert_eq!(cs.harvest_tree_density(SETTING_NOW), SETTING_NOW);
    }

    /// DENSITY IS AN ARGUMENT, AND STAYS ONE.
    ///
    /// The compiler already enforces that both streams take it as a parameter.
    /// What it cannot enforce is that nobody adds a second reader of the
    /// process-global bridge back into a stream body - which is what produced
    /// all three symptoms at once: the rounding divergence, the split-brain,
    /// and a ~50% flake between two tests in this file that fought over the
    /// atomic under the parallel harness.
    /// `AGNOSTIC_TREE_DENSITY` is what the vegetation-agnostic entries build
    /// at, and its whole justification is that it is the SHIPPED default rather
    /// than an invented number. If `config::default_tree_density` moves and
    /// this does not, every ground-geometry caller quietly starts building a
    /// forest the game never ships, and the drawn-surface offset probe starts
    /// measuring a different world from the one the player stands on.
    ///
    /// Read out of the source because the config default is private, and
    /// making it public purely to be asserted against would widen a surface
    /// for a test's convenience.
    #[test]
    fn the_agnostic_default_is_the_shipped_default() {
        let cfg = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("config.rs"),
        )
        .expect("config.rs reads");
        let at = cfg
            .find("fn default_tree_density() -> f32 {")
            .expect("config::default_tree_density is gone - find where the shipped default went");
        let body = &cfg[at..][..cfg[at..].find('}').expect("unterminated fn")];
        let shipped: f32 = body
            .rsplit('{')
            .next()
            .unwrap()
            .trim()
            .parse()
            .expect("default_tree_density is no longer a bare literal - read it another way");
        assert_eq!(
            shipped, AGNOSTIC_TREE_DENSITY,
            "config::default_tree_density is {shipped} but AGNOSTIC_TREE_DENSITY is \
             {AGNOSTIC_TREE_DENSITY}. Every caller that wants ground geometry without \
             caring about the forest builds at the second number; when it stops being \
             the shipped default, those callers are building a world the game does not."
        );
    }

    #[test]
    fn only_one_function_reads_the_published_density() {
        // Split so this scanner does not match its own source line.
        let needle = concat!("TREE_DENSITY_BITS", ".load");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut readers: Vec<String> = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("src/ reads") {
                let p = e.expect("dir entry").path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                    let txt = std::fs::read_to_string(&p).expect("source reads");
                    for (i, line) in txt.lines().enumerate() {
                        if line.contains(needle) {
                            readers.push(format!("{}:{}", p.display(), i + 1));
                        }
                    }
                }
            }
        }
        // At most one, not exactly one: the bridge is meant to reach ZERO
        // readers when the wiring request lands and it is deleted outright.
        assert!(
            readers.len() <= 1,
            "at most one function may read TREE_DENSITY_BITS (`published_tree_density`, the \
             wiring bridge); found {readers:?}. A stream that reads the density for itself is \
             a stream that can disagree with the other one about which trees exist."
        );
        let near = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("terrain")
                .join("near_trees.rs"),
        )
        .expect("near_trees.rs reads");
        // Production half only: the tests below DO carry a copy of the old
        // single-rounded formula on purpose, as the reference the shared helper
        // is measured against.
        let production = near.split("#[cfg(test)]").next().expect("split yields a head");
        assert!(
            !production.contains("TREES_PER_CELL as f64"),
            "near_trees.rs is deriving a per-cell count again. The count lives in \
             planet_chunks::trees_in_cell and nowhere else - a second copy is how the cards \
             and the models came to name different trees."
        );
    }

    /// The module can be right and never be called. This checks the frame loop
    /// actually sources the density once and hands the same number to both
    /// streams - the same shape of gate as
    /// `the_frame_loop_uses_the_measured_coverage_radius` above, which exists
    /// because the v0.1107 fix shipped correct and unreachable.
    #[test]
    fn the_frame_loop_sources_the_density_once_for_both_streams() {
        const PENDING: &str = "PENDING WIRING REQUEST (tree density as an argument). Not a \
             regression you caused: the fix lives in terrain::{near_trees,planet_chunks} and \
             the four-hunk src/lib.rs edit that calls it has not been applied yet. ";
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("lib.rs"),
        )
        .expect("lib.rs reads");
        assert!(
            src.contains("harvest_tree_density("),
            "{PENDING}src/lib.rs does not ask ChunkState::harvest_tree_density what density the \
             near-tree harvest may use. Harvesting at the raw setting orphans every card that \
             was baked at a higher one."
        );
        assert!(
            src.contains("near_tree_instances_at_density("),
            "{PENDING}src/lib.rs still calls near_tree_instances_on_drawn, which sources the \
             density itself from the process-global bridge."
        );
        assert!(
            src.contains("build_patch_mesh_at_density("),
            "{PENDING}src/lib.rs still calls build_patch_mesh, which sources the density itself \
             from the process-global bridge - so the card bake and the harvest can sample the \
             setting at different instants."
        );
        assert!(
            src.contains("insert_built("),
            "{PENDING}src/lib.rs still inserts built patches through insert_slotted, which \
             records tree_density = 0.0. Without PatchMesh::tree_density in the cache, \
             harvest_tree_density cannot see what the cards on screen were baked at."
        );
        assert!(
            !src.contains("TREE_DENSITY_BITS.store"),
            "{PENDING}src/lib.rs still publishes the density through the process-global bridge. \
             Once both streams take it as an argument the bridge has no readers - delete the \
             store, then TREE_DENSITY_BITS and published_tree_density in planet_chunks.rs."
        );
    }
}
