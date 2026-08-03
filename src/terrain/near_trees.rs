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
}
