//! The ONE shared grass TILLER MESH, and the DETAIL LADDER that sizes it.
//!
//! Extracted from `terrain::grass` (v0.1103) when the quality knob stopped
//! meaning "how much grass" and started meaning "how finely each tiller is
//! built" - see the module note on `grass_detail_for`. The extraction is what
//! paid for the new code: `grass.rs` was 77 lines under its ratchet budget,
//! and the mesh half was always a separable thing (the field/harvest decides
//! WHERE a tiller stands, this decides WHAT one is made of).
//!
//! Declared as a `#[path]` CHILD of `grass`, not a sibling of it, so every
//! existing `planet_chunks::grass_tiller_mesh` / `grass::GRASS_BLADES_*` path
//! still resolves through the parent's `pub use` and `terrain/mod.rs` needed
//! no edit at all.
//!
//! THE ONE INVARIANT EVERYTHING HERE EXISTS TO HOLD: whatever the detail
//! rung, a tiller carries `GRASS_LEAF_AREA_UNIT` of one-sided leaf area at
//! unit height. The builder MEASURES what its blades would carry and scales
//! their widths to hit that number exactly, so the canopy's leaf-area index
//! (the quantity the eye reads as "is this ground covered") is invariant under
//! the quality slider, and the slider buys back frame cost in TRIANGLES
//! instead of in bare ground.

use glam::Vec3;

use crate::renderer::plant_mesh::{Organ, PlantMeshBuilder};
use crate::terrain::planet_chunks::grass_detail;

/// Blades on one tiller at the SHIPPED DEFAULT quality (0.6), and the width
/// reference the whole ladder is normalized against. 5-9 is the real range for
/// a perennial-ryegrass tiller.
///
/// It is no longer "the" blade count: see `grass_detail_for`. It is the rung
/// the default sits on, kept as a named constant because `GRASS_LEAF_AREA_UNIT`
/// below was measured off exactly this mesh and the shipped look must not move
/// when the ladder is added underneath it.
pub const GRASS_BLADES_PER_TILLER: usize = 9;
/// The ladder's ends. At the LOW end a drawn blade stands for more real shoots
/// (see the bundle note on `GRASS_LEAF_AREA_UNIT`) and is correspondingly
/// wider; at the HIGH end the same leaf area is split into more, finer blades.
/// That is the whole quality knob.
pub const GRASS_BLADES_MIN: usize = 5;
pub const GRASS_BLADES_MAX: usize = 13;
/// Cross-sections along a blade. 3 is the shipped arch (two quads plus a
/// pointed tip); 2 still arches (one Bezier sample between root and tip) and
/// costs 6 triangles instead of 10. ONE is not offered: a single tapered
/// triangle from root to tip is a straight spike, which is the exact thing the
/// operator's "isolated dark spikes" report was about.
pub const GRASS_SEGMENTS_MAX: usize = 3;
pub const GRASS_SEGMENTS_MIN: usize = 2;
/// Quality at or above which a blade keeps its third segment.
pub const GRASS_SEGMENT_DROP_Q: f32 = 0.40;
/// The slider's floor, mirroring `veg_density`'s own clamp. The ladder is
/// parameterised on the SLIDER RANGE rather than on 0..1 so its bottom rung is
/// reachable: the operator's live setting is 0.1039.
pub const GRASS_QUALITY_MIN: f32 = 0.1;

/// TUSSOCK CROWN RADIUS, as a fraction of unit height: the disc over which one
/// tiller's blades actually ROOT. Min and max of the per-blade draw.
///
/// WHY THIS EXISTS (operator field report, v0.1091.1): with every blade rising
/// from ONE crown point the sward read as "a bunch of bundles tied together -
/// like someone went around, grabbed a handful of grass and put a rubber band
/// around them". That is exactly what a nine-blade fan from a single origin IS.
/// A real tussock is a spread base: daughter shoots ring the parent crown, so
/// the blades leave the ground over centimetres and neighbouring tussocks
/// interleave into a mat instead of standing as readable bouquets with bare
/// ground between them.
///
/// Expressed against UNIT HEIGHT because the mesh is shared and the instance
/// scale is uniform (the shader scales x, y and z by `height_m`), so a
/// tussock's crown grows with the tiller the way a real one does: at the
/// 0.24-0.52 m height range these are 1.7-13.5 cm of crown radius, centred on
/// 2.7-9.9 cm for a mean 0.38 m tiller.
pub const GRASS_ROOT_SPREAD_MIN: f32 = 0.07;
pub const GRASS_ROOT_SPREAD_MAX: f32 = 0.26;

/// Blade width at the crown, as a fraction of unit height, BEFORE the leaf-area
/// normalization below, and the exponent of its taper to a point:
/// `w(t) = W0 * (1 - t)^TAPER`.
///
/// These two numbers reproduce the shipped `SEG_W` table (0.209, 0.115, 0.041
/// at t = 0, 1/3, 2/3) to within 0.4% - it was a power taper all along, sampled
/// at three points. Stated as a CONTINUOUS profile because the segment count is
/// now a variable and a three-entry table cannot be sampled at two.
pub const GRASS_BLADE_W0: f32 = 0.209;
pub const GRASS_BLADE_TAPER: f32 = 1.4735;

/// ONE-SIDED leaf area a tiller carries at UNIT HEIGHT, m^2 per m^2 of scale.
/// THE AUTHORED QUANTITY, and the reason this module exists.
///
/// MEASURED off the shipped 9-blade / 3-segment mesh (0.8549) and then frozen
/// as the TARGET every rung of the detail ladder is normalized to, so a tiller
/// carries the same leaf area whether it is drawn with 5 wide blades or 13 fine
/// ones. Scale by `height_m^2` for a real instance; multiply by tillers per
/// m^2 for the canopy's LAI, which is what `grass_peak_per_m2` inverts.
///
/// WHY A DRAWN BLADE IS A BUNDLE, stated plainly so nobody "corrects" the width
/// later: a real turf carries 3,000-30,000 SHOOTS/m^2, which no renderer draws
/// as geometry. Each blade here stands for roughly 5-8 real blades at the
/// default rung and carries their combined projected area, which is why its
/// base is centimetres wide rather than the 3-5 mm of a single ryegrass blade.
/// Turning the quality slider down does not thin the turf, it bundles it more
/// coarsely - at the bottom rung a blade stands for ~15 real shoots and is
/// 1.8x wider. The quantity that has to stay right is the one the eye reads,
/// which is leaf AREA per ground area.
pub const GRASS_LEAF_AREA_UNIT: f32 = 0.855;

/// How finely one tiller is built. Both fields are pure functions of the
/// quality slider; nothing here changes how MUCH grass there is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrassDetail {
    pub blades: usize,
    pub segments: usize,
}

impl GrassDetail {
    /// Triangles one tiller costs at this rung: 4 per quad segment plus 2 for
    /// the pointed tip, doubled because the opaque pipeline back-culls and a
    /// blade must be visible from behind.
    pub fn triangles(&self) -> usize {
        self.blades * (4 * (self.segments.max(1) - 1) + 2)
    }
    /// Vertices per blade - `tri_smooth` pushes three fresh vertices per
    /// triangle, so blade k owns `k * verts_per_blade ..` and every per-blade
    /// reader indexes through this rather than through a hardcoded 30.
    pub fn verts_per_blade(&self) -> usize {
        12 * (self.segments.max(1) - 1) + 6
    }
    /// Index, within a blade's own vertex run, of the TIP APEX - the third
    /// corner of the first tip triangle.
    pub fn tip_vertex(&self) -> usize {
        12 * (self.segments.max(1) - 1) + 2
    }
}

/// THE QUALITY KNOB, and the one place it is interpreted (v0.1103).
///
/// Settings > Graphics "Vegetation density" used to multiply the tiller COUNT,
/// so turning it down turned the ground bald: at the operator's live 0.1039 the
/// sward measured LAI 0.58 against 3.33 at the 0.6 default, and the world read
/// as isolated spikes on a lawn. Coverage is not a quality setting - it is the
/// authored look - so it moved to `grass_peak_per_m2`, derived from a target
/// LAI, and the knob buys frame cost HERE instead: fewer, wider blades and
/// fewer segments per blade, with the width scaling holding projected leaf area
/// constant (the builder normalizes to `GRASS_LEAF_AREA_UNIT` exactly).
///
/// This is the standard technique - Jahrmann & Wimmer, "Responsive Real-Time
/// Grass Rendering for General 3D Scenes" (I3D 2017) - and it is also how the
/// same paper handles DISTANCE LOD, which is the follow-up this shape was
/// chosen for: bucket the instance list by distance and issue one instanced
/// draw per rung, and far tillers cost 30 triangles instead of 130 without the
/// far field thinning out. Nothing here has to change when that lands; it needs
/// a second mesh slot and a second draw in the renderer.
///
/// Rungs, and what the operator gets:
///   * 0.10 (their setting): 5 blades x 2 segments = 30 triangles/tiller
///   * 0.40:                 8 blades x 3 segments = 80
///   * 0.60 (the default):   9 blades x 3 segments = 90 - BIT-IDENTICAL to
///                           what shipped, which is deliberate: the default
///                           look must not move when the ladder appears under
///                           it
///   * 1.00:                13 blades x 3 segments = 130, on 40% fewer tillers
///                           than the old 1.0 setting drew, so max quality is
///                           CHEAPER per m^2 than it used to be
pub fn grass_detail_for(quality: f32) -> GrassDetail {
    let q = quality.clamp(GRASS_QUALITY_MIN, 1.0);
    let t = (q - GRASS_QUALITY_MIN) / (1.0 - GRASS_QUALITY_MIN);
    let span = (GRASS_BLADES_MAX - GRASS_BLADES_MIN) as f32;
    let blades = (GRASS_BLADES_MIN as f32 + span * t).round() as usize;
    GrassDetail {
        blades: blades.clamp(GRASS_BLADES_MIN, GRASS_BLADES_MAX),
        segments: if q >= GRASS_SEGMENT_DROP_Q {
            GRASS_SEGMENTS_MAX
        } else {
            GRASS_SEGMENTS_MIN
        },
    }
}

/// The detail rung the live Settings slider asks for.
pub fn grass_tiller_detail() -> GrassDetail {
    grass_detail_for(grass_detail())
}

/// CACHE KEY for the shared GPU mesh, and the reason it is public.
///
/// `Renderer::ensure_grass_mesh` is first-call-wins. While the mesh was a
/// constant that was correct; now that its blade count follows the slider, a
/// cache that never invalidates would make the slider LOOK like it worked (the
/// harvest responds instantly) while the old mesh stayed on screen until the
/// next restart - a bug shaped exactly like the fix. The renderer stores this
/// key beside the mesh and rebuilds when it changes.
pub fn grass_detail_key() -> u32 {
    let d = grass_tiller_detail();
    (d.blades as u32) * 8 + d.segments as u32
}

/// Blade half-width at parameter `t` along the midrib, before normalization.
#[inline]
fn blade_width(t: f32) -> f32 {
    GRASS_BLADE_W0 * (1.0 - t).max(0.0).powf(GRASS_BLADE_TAPER)
}

/// Deterministic per-blade jitter, 0..1, from an integer hash of the blade
/// index and a salt.
///
/// WHY A HASH AND NOT ANOTHER SINE (v0.1093): the first fan drew every varying
/// quantity from `(k * c).sin()` - the azimuth wobble from one multiplier, the
/// tip height AND the outward reach from a single shared `vary` term. Sines of
/// the same argument are smooth functions of `k`, and driving two quantities
/// from ONE of them makes them perfectly rank-correlated: the blade that stood
/// tallest was always the blade that reached furthest, so the tussock resolved
/// into a tidy fountain with a smooth outline. Measured on the shipped mesh:
/// Pearson(tip height, reach) = 1.000, with tip heights spanning only
/// 0.785-0.844 of unit height. A hash decorrelates them by construction
/// (measured 0.26 across nine blades, tip heights 0.704-0.935) and is still a
/// pure function of `k`, so the mesh is identical from run to run - and so a
/// 5-blade tiller is the 13-blade one's first five blades, which is what keeps
/// the rungs looking like the same plant.
#[inline]
fn blade_jitter01(k: usize, salt: u32) -> f32 {
    let mut h = (k as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

/// Metadata the caller needs about the shared tiller mesh, returned beside it
/// so nothing has to re-derive it from the vertex buffer.
#[derive(Clone, Copy, Debug)]
pub struct GrassTillerStats {
    /// ONE-SIDED leaf area of the whole tiller at unit height, m^2. The mesh
    /// is emitted double-sided (the opaque pipeline back-culls and a blade
    /// must be visible from behind), so this is half the raw triangle area.
    /// Scale by height^2 for a real instance. Normalized to
    /// `GRASS_LEAF_AREA_UNIT` at every detail rung.
    pub one_sided_area_unit: f32,
    pub blades: usize,
    pub segments: usize,
    pub triangles: usize,
    /// Horizontal reach of the widest vertex from the tussock axis, at unit
    /// height: the radius of ground one instance's blades hang over. Scale by
    /// `height_m` for a real instance. Measured off the built mesh rather
    /// than derived from the constants, so it tracks any shape change, and
    /// used by the sward tests to measure how much ground the layer actually
    /// covers (which is the quantity the "bare ground between bundles"
    /// report was about).
    pub footprint_unit: f32,
    /// Vertex-run stride per blade, so readers never hardcode a segment count.
    pub verts_per_blade: usize,
    /// Index of the tip apex within a blade's vertex run.
    pub tip_vertex: usize,
}

/// The shared tiller mesh at the live quality setting.
pub fn grass_tiller_mesh() -> (PlantMeshBuilder, GrassTillerStats) {
    grass_tiller_mesh_at(grass_tiller_detail())
}

/// The ONE mesh every grass instance draws: `detail.blades` arching, tapered
/// blades rooted across a SPREAD CROWN, built at UNIT height in a canonical
/// Y-up frame so an instance's uniform scale IS its height in metres.
///
/// Shape, from the real plant rather than from convenience:
///   * The blades ROOT OVER A DISC, not at a point (v0.1093). This is the
///     single most important thing about the silhouette and it was wrong until
///     the operator named it: "looks like a bunch of bundles tied together -
///     like someone went around, grabbed a handful of grass and put a rubber
///     band around them". Nine blades from one origin IS a bound bouquet, and
///     a field of bouquets has readable bare ground between them. A real
///     tussock's daughter shoots ring the parent crown, so its base is
///     centimetres across and neighbouring tussocks interleave into a mat.
///     Root radius per blade is `GRASS_ROOT_SPREAD_MIN..MAX` of unit height
///     (2.7-9.9 cm on a mean 0.38 m tiller), area-weighted so the outer ring
///     is not under-populated.
///   * A blade emerges near-vertical and ARCHES over. A straight blade reads
///     as wheat stubble or a bristle brush; the arch is the entire reason a
///     sward reads as a soft mass. The midrib follows a quadratic Bezier from
///     its own root to a tip that has fallen outward and down.
///   * It LEANS AWAY FROM THE TUSSOCK CENTRE, loosely. A spread base with
///     random lean directions reads as a tangle; a spread base leaning outward
///     reads as a tussock, because that is the geometry that makes the crown
///     open and the tips ring it.
///   * It TAPERS to a point. The old card had a ruler-straight top edge, which
///     is what made the tufts photograph as pieces of pale tape.
///   * Tip height, outward reach and how early the blade bends are drawn
///     INDEPENDENTLY (`blade_jitter01`), so the outline is ragged rather than
///     a neat fountain.
///
/// LEAF AREA IS NORMALIZED, not emergent (v0.1103). The blade midribs are laid
/// out first, the area they WOULD carry at the nominal width profile is
/// measured, and every width is then scaled by the one factor that lands the
/// total on `GRASS_LEAF_AREA_UNIT`. That single step does three jobs: it makes
/// blades wider as the ladder drops their count (so the canopy's LAI does not
/// follow the quality slider), it absorbs the few-percent area difference
/// between a 2-segment and a 3-segment discretization of the same midrib, and
/// it absorbs the sampling difference between blade sets of different sizes.
///
/// Shading, all of it free because the vertices exist anyway:
///   * PER-CORNER NORMALS blended half-way between the blade's own facing and
///     the crown's radial up. That gives a real lit side and a shaded side
///     across one tiller (the deleted card pinned every normal to the radial
///     up, so every tuft on the planet lit identically), while structurally
///     preventing N.L from reaching zero - which is what the v0.896 radial pin
///     was protecting against. It is the same 0.5 spherification the cluster
///     cards use.
///   * The LEAF ORGAN BIT on every face, so the shipped `is_leaf` transmission
///     path lights a backlit blade as thin tissue. Green is the band leaf
///     tissue transmits best; a sward with the sun behind it glows.
///   * A VERTICAL SHADE RAMP, dark at the crown and full at the tips. This is
///     Beer-Lambert, not decoration: at LAI 2-4 with grass's low extinction
///     the base of a sward sits at 20-30% of the top-of-canopy irradiance,
///     and a uniformly lit blade is a large part of what reads as a sticker.
///
/// A NOTE ON THE NORMALS, because correct grass lighting currently depends on
/// two sign inversions cancelling: the per-corner normal built here is in
/// OBJECT space and the grass instance basis in `00-bindings-vertex.wgsl`
/// (lines 604-605) has determinant -1, which reverses rasterized winding. The
/// pairing is therefore correct in WORLD space (measured dot in +0.594..+1.000)
/// even though object space reads inverted. Do NOT "fix" one without the other
/// in the same commit: flipping this alone takes front-lit grass at a 20 degree
/// sun from 89.6% lit to 4.4%.
///
/// THE MESH CARRIES NO COLOUR, and must not be given one. It is drawn ONCE,
/// instanced, for every tiller in the field; the moment its packed UV holds a
/// tint it stops being shareable and becomes one mesh per tiller, which is
/// the thing this design exists to avoid. So the packed channel carries the
/// GREY RAMP (0.30 at the crown to 1.00 at the tip) and the per-instance
/// albedo in `NearGrass::color` multiplies it in the fragment stage.
pub fn grass_tiller_mesh_at(detail: GrassDetail) -> (PlantMeshBuilder, GrassTillerStats) {
    let blades = detail.blades.clamp(1, 64);
    let segs = detail.segments.clamp(1, 8);
    let up = Vec3::Y;
    // Parameter samples along a midrib: segs + 1 cross-sections, ending at the
    // pointed tip.
    let ts: Vec<f32> = (0..=segs).map(|s| s as f32 / segs as f32).collect();

    // ── PASS 1: lay out the midribs and MEASURE the leaf area they carry ──
    // Nothing is emitted yet: the width correction below has to be known
    // before the first vertex is pushed.
    struct BladeGeo {
        mids: Vec<Vec3>,
        across: Vec3,
        nrm: Vec3,
    }
    let mut geo: Vec<BladeGeo> = Vec::with_capacity(blades);
    let mut nominal_area = 0.0f32;
    for k in 0..blades {
        // ── ROOT: where this blade leaves the ground ──
        // The golden angle rings the crown without ever repeating an azimuth,
        // and a per-blade wobble keeps two neighbours from looking like a
        // mirrored pair. The radius is area-weighted (sqrt of the draw), so
        // the outer ring of the crown carries as many shoots per unit area as
        // the middle - a linear draw crowds the centre and puts the pinch
        // back.
        let kf = k as f32;
        let ra = kf * 2.399_963_2 + (blade_jitter01(k, 1) - 0.5) * 1.1;
        let rr = GRASS_ROOT_SPREAD_MIN
            + (GRASS_ROOT_SPREAD_MAX - GRASS_ROOT_SPREAD_MIN) * blade_jitter01(k, 2).sqrt();
        let root = Vec3::new(ra.cos() * rr, 0.0, ra.sin() * rr);
        // ── LEAN: loosely away from the tussock centre ──
        // Not exactly outward (that is a parasol) and not free (that is a
        // tangle): +-34 degrees of the root's own bearing.
        let az = ra + (blade_jitter01(k, 3) - 0.5) * 1.2;
        let side = Vec3::new(az.cos(), 0.0, az.sin());
        // ── SPLAY: three INDEPENDENT draws ──
        // Tip height, outward reach and the bend point come from different
        // hash salts, so a tall blade is not automatically the far-reaching
        // one. See `blade_jitter01` for what the shipped single-`vary` fan
        // measured (correlation 1.000, tip heights inside a 6% band).
        let tip_h = 0.68 + blade_jitter01(k, 4) * 0.28; // 0.68..0.96 of unit height
        let reach = 0.14 + blade_jitter01(k, 5) * 0.48; // outward fall of the tip
        let bend = 0.70 + blade_jitter01(k, 6) * 0.20; // how late it arches over
        // Quadratic Bezier control point: high and close in, so the blade
        // leaves its root near-vertical and only bends over near the tip.
        let p0 = root;
        let p1 = root + up * (tip_h * bend);
        let p2 = root + up * tip_h + side * reach;
        let at = |t: f32| -> Vec3 {
            let u = 1.0 - t;
            p0 * (u * u) + p1 * (2.0 * u * t) + p2 * (t * t)
        };
        let mids: Vec<Vec3> = ts.iter().map(|t| at(*t)).collect();
        // Blade-plane frame: `side` is the fall direction, so the blade's
        // WIDTH runs across it.
        let across = up.cross(side).normalize();
        // Per-corner normal: the blade's own facing (perpendicular to the
        // blade plane) blended HALF-WAY toward the crown's radial up. Capped
        // at 0.5 so N.L can never reach 0 - the black-slab guard.
        let facing = across.cross(mids[segs] - mids[0]).normalize_or_zero();
        let mut facing = if facing.length_squared() < 0.5 { side } else { facing };
        // The blade's UPPER surface faces the sky, so orient the facing
        // upward before blending. Without this the cross product's sign flips
        // with the fan azimuth and half the blades ship an inverted normal
        // (measured: n.y = -0.55 on the first pass), which at grazing sun is
        // the black-slab defect the v0.896 radial pin was fighting. It is
        // UNCONDITIONAL, not azimuth-dependent - the shipped comment here said
        // otherwise for two releases and was simply wrong.
        if facing.y < 0.0 {
            facing = -facing;
        }
        let nrm = (facing * 0.5 + up * 0.5).normalize();
        for s in 0..segs {
            let (w0, w1) = (blade_width(ts[s]), blade_width(ts[s + 1]));
            nominal_area += 0.5 * (w0 + w1) * (mids[s + 1] - mids[s]).length();
        }
        geo.push(BladeGeo { mids, across, nrm });
    }
    // THE NORMALIZATION. Every width is scaled by this one factor, so the
    // emitted one-sided area is `GRASS_LEAF_AREA_UNIT` at every rung of the
    // ladder - which is what decouples the canopy's LAI from the quality knob.
    let corr = if nominal_area > 1.0e-6 {
        GRASS_LEAF_AREA_UNIT / nominal_area
    } else {
        1.0
    };

    // ── PASS 2: emit ──
    let mut b = PlantMeshBuilder::new();
    b.set_organ(Organ::Leaf);
    let mut one_sided = 0.0f32;
    for g in &geo {
        // Vertical Beer-Lambert ramp, packed as a GREY (see the no-colour
        // note above). Flat-interpolated, so it is per-FACE - one shade per
        // segment, which is exactly the resolution the blade carries.
        let shade = |t: f32| -> [f32; 3] {
            let k = 0.30 + 0.70 * t; // crown 30% of tip irradiance
            [k, k, k]
        };
        for s in 0..segs {
            let (m0, m1) = (g.mids[s], g.mids[s + 1]);
            let w0 = blade_width(ts[s]) * corr * 0.5;
            let w1 = if s + 1 == segs {
                0.0
            } else {
                blade_width(ts[s + 1]) * corr * 0.5
            };
            let c = shade((ts[s] + ts[s + 1]) * 0.5);
            let a0 = m0 - g.across * w0;
            let a1 = m0 + g.across * w0;
            if s + 1 == segs {
                // Pointed tip: one triangle, no top edge.
                b.tri_smooth(
                    [a0.to_array(), a1.to_array(), m1.to_array()],
                    [g.nrm.to_array(); 3],
                    c,
                );
                b.tri_smooth(
                    [a0.to_array(), m1.to_array(), a1.to_array()],
                    [(-g.nrm).to_array(); 3],
                    c,
                );
                one_sided += 0.5 * (a1 - a0).length() * (m1 - m0).length();
            } else {
                let b0 = m1 - g.across * w1;
                let b1 = m1 + g.across * w1;
                for (p, n) in [
                    ([a0, a1, b1], g.nrm),
                    ([a0, b1, b0], g.nrm),
                    ([a0, b1, a1], -g.nrm),
                    ([a0, b0, b1], -g.nrm),
                ] {
                    b.tri_smooth(
                        [p[0].to_array(), p[1].to_array(), p[2].to_array()],
                        [n.to_array(); 3],
                        c,
                    );
                }
                one_sided += 0.5 * ((a1 - a0).length() + (b1 - b0).length())
                    * (m1 - m0).length();
            }
        }
    }
    b.set_organ(Organ::Stem);
    let triangles = b.indices.len() / 3;
    let footprint_unit = b
        .vertices
        .iter()
        .map(|v| (v.position[0] * v.position[0] + v.position[2] * v.position[2]).sqrt())
        .fold(0.0f32, f32::max);
    let d = GrassDetail { blades, segments: segs };
    (
        b,
        GrassTillerStats {
            one_sided_area_unit: one_sided,
            blades,
            segments: segs,
            triangles,
            footprint_unit,
            verts_per_blade: d.verts_per_blade(),
            tip_vertex: d.tip_vertex(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::grass::{GRASS_HEIGHT_MAX_M, GRASS_HEIGHT_MIN_M};

    /// Every rung of the ladder, so the structural gates below run against all
    /// of them rather than against whichever one the test process's atomic
    /// happens to hold.
    fn rungs() -> Vec<GrassDetail> {
        let mut v: Vec<GrassDetail> = Vec::new();
        let mut q = GRASS_QUALITY_MIN;
        while q <= 1.0001 {
            let d = grass_detail_for(q);
            if !v.contains(&d) {
                v.push(d);
            }
            q += 0.05;
        }
        v
    }

    /// THE DECOUPLING GATE (v0.1103), and the reason this module exists.
    ///
    /// The quality slider used to multiply the tiller COUNT, so the operator's
    /// live 0.1039 setting was drawing a canopy of LAI 0.58 against the 0.6
    /// default's 3.33 - the ground read as isolated spikes on a lawn, and a
    /// large part of what was reported as grass DEFECTS was that. Coverage is
    /// now derived from a target LAI and the slider moves DETAIL instead, which
    /// is only true if a tiller carries the same leaf area at every rung. This
    /// measures that off the real built geometry.
    #[test]
    fn grass_detail_ladder_conserves_leaf_area() {
        let all = rungs();
        assert!(all.len() >= 4, "the ladder collapsed to {} rungs", all.len());
        let mut tris = Vec::new();
        for d in &all {
            let (b, s) = grass_tiller_mesh_at(*d);
            let err = (s.one_sided_area_unit - GRASS_LEAF_AREA_UNIT).abs() / GRASS_LEAF_AREA_UNIT;
            println!(
                "[grass detail] {} blades x {} segments: {} triangles, {} verts, leaf area \
                 {:.4} unit^2 ({:+.2}% off target), footprint {:.3}",
                s.blades,
                s.segments,
                s.triangles,
                b.vertices.len(),
                s.one_sided_area_unit,
                (s.one_sided_area_unit / GRASS_LEAF_AREA_UNIT - 1.0) * 100.0,
                s.footprint_unit
            );
            assert!(
                err < 0.005,
                "a {} blade x {} segment tiller carries {:.4} unit^2 of leaf against the \
                 authored {GRASS_LEAF_AREA_UNIT:.4} ({:+.1}%) - the width normalization is not \
                 holding, so the quality slider is back to changing how much canopy there is",
                s.blades,
                s.segments,
                s.one_sided_area_unit,
                (s.one_sided_area_unit / GRASS_LEAF_AREA_UNIT - 1.0) * 100.0
            );
            assert_eq!(s.triangles, d.triangles(), "triangle arithmetic disagrees with the mesh");
            assert_eq!(b.vertices.len(), s.blades * s.verts_per_blade, "vertex stride wrong");
            tris.push(s.triangles);
        }
        // The knob has to actually BUY something, or the whole exchange is a
        // no-op that costs coverage nothing and gains frame time nothing.
        let (lo, hi) = (tris[0], *tris.last().unwrap());
        assert!(
            hi >= lo * 3,
            "the ladder spans {lo}..{hi} triangles per tiller - turning quality down has to \
             buy back real geometry, that is the whole point of paying for it in detail"
        );
        // And the top rung must not cost more PER SQUARE METRE than the old
        // veg_density 1.0 did (45 tillers/m2 x 90 triangles = 4,050), because
        // coverage no longer rises with the slider.
        let peak = crate::terrain::grass::grass_peak_per_m2();
        let per_m2 = peak * hi as f32;
        println!("[grass detail] top rung: {peak:.1} tillers/m2 x {hi} tris = {per_m2:.0} tris/m2");
        assert!(
            per_m2 <= 4_050.0,
            "max quality now costs {per_m2:.0} triangles per m2 of ground against the 4,050 the \
             old veg_density 1.0 drew"
        );
    }

    /// The default rung must be the mesh that shipped. The whole change is
    /// meant to be invisible AT 0.6 and only fix what happens either side of
    /// it, so if this moves, the default look moved with it.
    #[test]
    fn the_shipped_default_rung_is_unchanged() {
        let d = grass_detail_for(0.6);
        assert_eq!(d.blades, GRASS_BLADES_PER_TILLER, "default blade count moved");
        assert_eq!(d.segments, GRASS_SEGMENTS_MAX, "default segment count moved");
        let (b, s) = grass_tiller_mesh_at(d);
        assert_eq!(s.triangles, GRASS_BLADES_PER_TILLER * 10);
        assert_eq!(b.vertices.len(), GRASS_BLADES_PER_TILLER * 30);
        // The width profile replaced a three-entry table with the power curve
        // it was fitting; the correction factor then lands the area exactly.
        // Both together must leave the shipped crown width where it was.
        let w0 = (Vec3::from_array(b.vertices[1].position)
            - Vec3::from_array(b.vertices[0].position))
        .length();
        assert!(
            (w0 - GRASS_BLADE_W0).abs() < 0.01,
            "the default rung's crown blade is {w0:.3} of unit height against the shipped \
             {GRASS_BLADE_W0:.3}"
        );
    }

    /// The shared tiller mesh has to be a fan of ARCHING, TAPERED blades with
    /// per-corner normals and the leaf organ bit - every one of those is a
    /// specific defect the deleted card had (ruler-straight edges, a normal
    /// pinned to the radial up so every tuft on the planet lit identically,
    /// and no transmission path at all because it went through the type-12
    /// terrain branch). Run at EVERY rung: a mesh that is only valid at the
    /// default is a mesh that ships broken to anyone who moves the slider.
    #[test]
    fn grass_tiller_is_a_fan_of_arching_tapered_blades() {
        use crate::terrain::planet_surface::unpack_uv_to_color;
        for d in rungs() {
            let (b, stats) = grass_tiller_mesh_at(d);
            assert_eq!(stats.blades, d.blades);
            assert_eq!(stats.triangles, b.indices.len() / 3);
            // THE VERTEX LAYOUT the per-blade checks below index into:
            // `tri_smooth` pushes three fresh vertices per triangle (no
            // sharing), and blade k emits its whole run uninterrupted, so blade
            // k owns vertices k*stride..k*stride+stride.
            assert_eq!(
                b.vertices.len(),
                stats.blades * stats.verts_per_blade,
                "the mesh is no longer {} vertices per blade - the per-blade slices below are \
                 reading the wrong geometry",
                stats.verts_per_blade
            );
            assert!(
                stats.triangles <= GRASS_BLADES_MAX * 10,
                "{} triangles per tiller - a 22 m harvest draws ~12,700 tillers, so every \
                 triangle here is multiplied by five figures and again by the shadow pass",
                stats.triangles
            );
            // Every face carries the LEAF organ bit (shader `is_leaf`), so a
            // backlit sward glows instead of being shaded as bark.
            for v in &b.vertices {
                let packed = v.uv[0].round().max(0.0) as u32;
                assert!(
                    packed & 524_288 != 0,
                    "a grass face is not tagged as leaf tissue - it would shade as stem"
                );
            }
            // ARCH + TAPER, measured PER BLADE from its OWN root (v0.1093).
            // The shipped version of this check compared the widest vertex
            // radius near the top of the mesh against the widest near the
            // bottom, which only worked while every blade rose from the origin.
            // With a SPREAD CROWN a blade rooted 0.26 out and standing straight
            // up would pass that on its root offset alone, so the arch is
            // measured where it lives: how far each tip has fallen from the
            // root of its own blade.
            let blade = |k: usize| -> (Vec3, Vec3, f32) {
                let v = |i: usize| {
                    let p = b.vertices[k * stats.verts_per_blade + i].position;
                    Vec3::new(p[0], p[1], p[2])
                };
                let (a0, a1) = (v(0), v(1));
                (((a0 + a1) * 0.5), v(stats.tip_vertex), (a1 - a0).length())
            };
            let mut fall = Vec::new();
            for k in 0..stats.blades {
                let (root, tip, w) = blade(k);
                let horiz = ((tip.x - root.x).powi(2) + (tip.z - root.z).powi(2)).sqrt();
                assert!(
                    w > 0.05,
                    "blade {k}'s root cross-section is {w:.3} wide - the crown has lost its taper"
                );
                assert!(
                    tip.y > 0.5,
                    "blade {k}'s tip stands at {:.3} of unit height - the mesh no longer reaches \
                     the height its instance scale claims",
                    tip.y
                );
                fall.push(horiz / tip.y);
            }
            let mean_fall = fall.iter().sum::<f32>() / fall.len() as f32;
            println!(
                "[grass mesh] {}x{}: blade tip fall/height min {:.2} max {:.2} mean \
                 {mean_fall:.2}; footprint {:.3}; leaf area {:.4} unit^2",
                stats.blades,
                stats.segments,
                fall.iter().cloned().fold(f32::MAX, f32::min),
                fall.iter().cloned().fold(0.0, f32::max),
                stats.footprint_unit,
                stats.one_sided_area_unit
            );
            assert!(
                fall.iter().all(|f| *f > 0.10) && mean_fall > 0.30,
                "blade tips fall outward by {mean_fall:.2} of their height on average (min \
                 {:.2}) - the blades are not arching, they are standing straight up like bristles",
                fall.iter().cloned().fold(f32::MAX, f32::min)
            );
            // NORMALS: not all identical (the card's defect - every tuft on the
            // planet lit the same because `nrm = up`), and never more than
            // half-way from the radial up toward the blade facing (the v0.896
            // black-slab guard: N.L must not be able to reach 0). Undersides
            // carry the mirrored normal, which is correct thin-tissue geometry,
            // so the guard is on the MAGNITUDE.
            let tops: Vec<_> = b.vertices.iter().filter(|v| v.normal[1] > 0.0).collect();
            assert!(!tops.is_empty(), "no upward-facing grass normals at all");
            let n0 = tops[0].normal;
            assert!(
                tops.iter()
                    .any(|v| (v.normal[0] - n0[0]).abs() + (v.normal[2] - n0[2]).abs() > 0.3),
                "every corner carries the same normal - one tiller has no lit side and no \
                 shaded side, which is exactly what the deleted card did"
            );
            for v in &b.vertices {
                assert!(
                    v.normal[1].abs() >= 0.35,
                    "a grass normal has tipped past half-way toward the blade facing \
                     (n.y {:.2}); at grazing sun that face goes black",
                    v.normal[1]
                );
            }
            // BEER-LAMBERT RAMP: the crown segment must be markedly darker than
            // the tip segment. Colour rides the packed UV, one shade per face.
            // Selected by SEGMENT (the blade-local triangle run) rather than by
            // a band of absolute height: with tip heights drawn independently
            // per blade, a height band mixes one blade's mid segment with
            // another's tip and dilutes the very thing being measured.
            let per_blade_tris = stats.triangles / stats.blades;
            let seg_shade = |tri_lo: usize, tri_hi: usize| -> f32 {
                let mut sum = 0.0;
                let mut n = 0.0;
                for k in 0..stats.blades {
                    for t in tri_lo..tri_hi {
                        let (c, _) =
                            unpack_uv_to_color(b.vertices[k * stats.verts_per_blade + t * 3].uv);
                        sum += c[1];
                        n += 1.0;
                    }
                }
                sum / n
            };
            let (baseg, tipg) = (seg_shade(0, 4), seg_shade(per_blade_tris - 2, per_blade_tris));
            assert!(
                baseg > 0.0 && tipg > 0.0,
                "no faces found in the base/tip bands ({baseg}, {tipg})"
            );
            assert!(
                baseg < tipg * 0.60,
                "sward base reads {baseg:.3} against tips {tipg:.3} - a real sward's base sits \
                 at 20-30% of top-of-canopy irradiance; a uniformly lit blade is a sticker"
            );
        }
    }

    /// THE RUBBER-BAND GATE (v0.1093), CI twin for the operator's field report
    /// at v0.1091.1: the sward "looks like a bunch of bundles tied together -
    /// like someone went around, grabbed a handful of grass, put a rubber band
    /// around them".
    ///
    /// That is a geometric fact about the mesh, not a lighting or density
    /// problem: every blade rose from ONE crown point, which IS a bound
    /// bouquet, and a field of bouquets has readable bare ground between them
    /// however many you scatter. A real tussock's shoots ring the parent
    /// crown, so its base is centimetres across and neighbouring tussocks
    /// interleave.
    ///
    /// The gate is stated in METRES ON THE GROUND, at the mean tiller height,
    /// because that is the scale the eye judges: the mesh is unit-height and
    /// the instance scale is uniform, so a unit-space radius of 0.13 is 5 cm
    /// on a 0.38 m tiller. Nothing here may be satisfied by a wider blade -
    /// it is measured on the blade ROOT MIDPOINTS, so a fan of wide-based
    /// blades sharing one origin still fails.
    #[test]
    fn grass_tussock_blades_root_over_a_spread_crown() {
        for d in rungs() {
            let (b, stats) = grass_tiller_mesh_at(d);
            // Blade k's root is the midpoint of its first cross-section.
            let roots: Vec<Vec3> = (0..stats.blades)
                .map(|k| {
                    let v = |i: usize| {
                        let p = b.vertices[k * stats.verts_per_blade + i].position;
                        Vec3::new(p[0], p[1], p[2])
                    };
                    (v(0) + v(1)) * 0.5
                })
                .collect();
            for (k, r) in roots.iter().enumerate() {
                assert!(
                    r.y.abs() < 1.0e-5,
                    "blade {k} roots at y {:.4} instead of on the ground plane - a crown that \
                     starts above the ground hovers when the instance is planted",
                    r.y
                );
            }
            // Mean tiller height: the scale a unit-space radius is judged at.
            let h = ((GRASS_HEIGHT_MIN_M + GRASS_HEIGHT_MAX_M) * 0.5) as f32;
            let centroid = roots.iter().fold(Vec3::ZERO, |a, r| a + *r) / roots.len() as f32;
            let radii: Vec<f32> = roots
                .iter()
                .map(|r| ((r.x - centroid.x).powi(2) + (r.z - centroid.z).powi(2)).sqrt() * h)
                .collect();
            let (mut min_pair, mut sum_pair, mut n_pair) = (f32::MAX, 0.0f32, 0usize);
            for i in 0..roots.len() {
                for j in (i + 1)..roots.len() {
                    let dd = ((roots[i].x - roots[j].x).powi(2)
                        + (roots[i].z - roots[j].z).powi(2))
                    .sqrt()
                        * h;
                    min_pair = min_pair.min(dd);
                    sum_pair += dd;
                    n_pair += 1;
                }
            }
            let mean_pair = sum_pair / n_pair as f32;
            let max_r = radii.iter().cloned().fold(0.0, f32::max);
            let mean_r = radii.iter().sum::<f32>() / radii.len() as f32;
            println!(
                "[grass crown] {} blades on a {:.2} m tiller: root radius mean {:.1} cm, max \
                 {:.1} cm; pairwise root separation min {:.1} cm, mean {:.1} cm",
                stats.blades,
                h,
                mean_r * 100.0,
                max_r * 100.0,
                min_pair * 100.0,
                mean_pair * 100.0
            );
            // THE PINCH GUARD. A point crown scores 0 on every one of these.
            assert!(
                radii.iter().filter(|r| **r > 0.02).count() >= stats.blades * 2 / 3,
                "{} of {} blades root within 2 cm of the tussock centre - that is the \
                 rubber-band bundle the field report named",
                radii.iter().filter(|r| **r <= 0.02).count(),
                stats.blades
            );
            assert!(
                (0.03..=0.14).contains(&mean_r),
                "mean root radius {:.1} cm - a tussock crown is centimetres across; under ~3 cm \
                 it reads as a bound bundle and past ~14 cm the blades stop belonging to one \
                 plant",
                mean_r * 100.0
            );
            assert!(
                min_pair > 0.005,
                "two blades root {:.1} cm apart - they will read as one doubled blade rather \
                 than as two shoots",
                min_pair * 100.0
            );
            assert!(
                mean_pair > 0.05,
                "blade roots average {:.1} cm apart - the crown is still effectively a point",
                mean_pair * 100.0
            );
        }
    }

    /// SPLAY DECORRELATION (v0.1093). A spread crown is not enough on its own:
    /// the shipped fan drew tip height AND outward reach from ONE `vary` term
    /// (both `(k*3.1).sin()`-derived), so the tallest blade was always the
    /// furthest-reaching one and the tussock resolved into a tidy fountain
    /// with a smooth outline. Measured on the shipped mesh: Pearson 1.000,
    /// with every tip inside a 0.785-0.844 band of unit height, i.e. a 6%
    /// spread of heights across nine blades.
    ///
    /// Measured off the built mesh, not off the constants, so a future
    /// "simplification" back to one shared jitter term fails here.
    #[test]
    fn grass_tussock_outline_is_ragged_not_a_fountain() {
        for d in rungs() {
            let (b, stats) = grass_tiller_mesh_at(d);
            let mut heights = Vec::new();
            let mut reaches = Vec::new();
            for k in 0..stats.blades {
                let v = |i: usize| {
                    let p = b.vertices[k * stats.verts_per_blade + i].position;
                    Vec3::new(p[0], p[1], p[2])
                };
                let root = (v(0) + v(1)) * 0.5;
                let tip = v(stats.tip_vertex);
                heights.push(tip.y);
                reaches.push(((tip.x - root.x).powi(2) + (tip.z - root.z).powi(2)).sqrt());
            }
            let mean = |a: &[f32]| a.iter().sum::<f32>() / a.len() as f32;
            let (mh, mr) = (mean(&heights), mean(&reaches));
            let (mut num, mut d1, mut d2) = (0.0f32, 0.0f32, 0.0f32);
            for k in 0..stats.blades {
                num += (heights[k] - mh) * (reaches[k] - mr);
                d1 += (heights[k] - mh).powi(2);
                d2 += (reaches[k] - mr).powi(2);
            }
            let pearson = num / (d1 * d2).sqrt().max(1.0e-9);
            let hspread = (heights.iter().cloned().fold(0.0, f32::max)
                - heights.iter().cloned().fold(f32::MAX, f32::min))
                / mh;
            println!(
                "[grass splay] {} blades: tip heights {:.3}..{:.3} (spread {:.0}% of mean), \
                 reaches {:.3}..{:.3}, pearson(height, reach) {pearson:+.3}",
                stats.blades,
                heights.iter().cloned().fold(f32::MAX, f32::min),
                heights.iter().cloned().fold(0.0, f32::max),
                hspread * 100.0,
                reaches.iter().cloned().fold(f32::MAX, f32::min),
                reaches.iter().cloned().fold(0.0, f32::max),
            );
            assert!(
                pearson.abs() < 0.6,
                "tip height and outward reach are correlated at {pearson:+.3} across the blades \
                 - they are being driven by one jitter term again, which draws a neat fountain"
            );
            assert!(
                hspread > 0.20,
                "the tallest blade is only {:.0}% taller than the shortest - the outline is a \
                 smooth dome, not a ragged tussock",
                hspread * 100.0
            );
        }
    }
}
