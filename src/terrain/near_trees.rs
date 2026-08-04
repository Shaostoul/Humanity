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
        let count = ((TREES_PER_CELL as f64)
            * (tree_density() as f64)
            * cell_lat.cos().max(0.0))
        .round() as u32;
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
        let harvest = |max_n: usize| {
            near_tree_instances_on_drawn(&def, &source, Some(&albedo), center, 240.0, 17, max_n)
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
        // The shipped Settings default; the atomic starts at 1.0 in a test bin.
        crate::terrain::planet_chunks::TREE_DENSITY_BITS
            .store(0.6f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
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
                        harvest = near_tree_instances_on_drawn(
                            &def,
                            &source,
                            Some(&albedo),
                            cam_local.normalize(),
                            tree_dist + 60.0,
                            17,
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
