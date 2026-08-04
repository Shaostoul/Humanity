//! FOLIAGE CARD ARRANGEMENT: how the cards of one tuft sit around a twig.
//!
//! Split out of `tree_mesh.rs` under the file-size ratchet
//! (`tests/file_size_ratchet.rs`), which asks for a coherent cluster rather
//! than a raised budget. This is one: every item here answers "where does a
//! foliage card go, and which way does it face", and nothing here knows how a
//! crown is planned or how many cards it can afford - `emit_cluster_cards`
//! keeps that job in the kernel.
//!
//! A CHILD module of `tree_mesh`, for the same reason `tree_species` is: a
//! card emitter touches the kernel's private vector helpers, its
//! `PlantMeshBuilder`, its uv encoding and its normal-blend constants, and a
//! child sees all of them through one `use super::*`. As a sibling every one
//! of those would need widening to `pub(crate)`.
//!
//! THE CUBE (v0.1110). The operator, on v0.1109.3: "the bunches of leaves look
//! a little weird, kind of like they're cubes." They were, and it took both
//! halves of the fix below to clear it - `PHYLLOTAXIS_RAD` spreads the card
//! normals AROUND a circle, `sleeve_tilts` gets them OFF it. The gate that
//! measures it, `cluster_cards_in_one_tuft_are_not_a_box`, moved down here
//! with the code it measures.

use super::*;

/// Radial offset of a card from its twig, as a fraction of the card side.
pub(super) const CLUSTER_SLEEVE_OFFSET: f32 = 0.30;

/// The golden angle, 137.507 degrees, in radians: the divergence between
/// successive leaves on an alternate/spiral shoot, which is what oak, birch and
/// cherry actually grow.
///
/// Used as the azimuth step between cluster cards on one twig. It is an
/// irrational fraction of a turn, so successive cards never repeat an
/// arrangement - which is precisely why nature uses it (no leaf ever sits
/// directly above another). The equal-azimuth arrangement it replaced put four
/// cards at 0/90/180/270 and two at 0/180.
///
/// It is HALF the fix, and on its own it was not enough. Spacing the azimuths
/// spreads the normals evenly AROUND a circle, but they all still lie ON that
/// circle, and four lines on a circle cannot dodge a near-right angle: measured
/// per sleeve, the golden angle alone still left 33% of card pairs within 5
/// degrees of perpendicular. The other half is `sleeve_tilts`.
const PHYLLOTAXIS_RAD: f32 = 2.399_963_2;

/// Tilt each card out of the twig-perpendicular plane, so a sleeve's card
/// planes spread over a SPHERE rather than around a circle.
///
/// This is the other half of the cube fix, and it is the half that carried the
/// broadleaves. Through v0.1109 every card in a sleeve took the twig axis as
/// its up-vector, so every card plane CONTAINED the twig direction and every
/// card normal lay in the single plane perpendicular to it. Measured per
/// sleeve, before the fix (`cluster_cards_in_one_tuft_are_not_a_box`):
///
/// - fir and pine, 2 cards: 100% of pairs PARALLEL. Every conifer tuft in the
///   world was two coplanar slabs, which is why they flashed as the camera
///   turned - a pair of parallel cards has no in-between view.
/// - oak, birch, sakura, momiji, acacia, 4 cards: 67% perpendicular, 33%
///   parallel, every other angle bin empty. That is a rectangular prism, and
///   it is exactly what the operator described.
///
/// Cards stay square, rigid and the same size - the tilt is a rotation of the
/// card's own frame - so leaf area, LAI, coverage and overdraw are all
/// untouched by it, and the gates that measure those still read the same.
///
/// Returns the tilts, in radians, for the `n` cards of one sleeve.
///
/// A sleeve is `n` PLANES, and two planes are the same plane whether their
/// normals agree or oppose, so what wants spreading is a set of LINES through
/// the origin, not points on a sphere. That is a line packing (the Tammes
/// problem in real projective space), and it is not the same problem as
/// scattering points: the best two POINTS are antipodal, while the best two
/// LINES are perpendicular, and antipodal normals are two PARALLEL cards - a
/// visible slab, which is half of what the operator called a cube.
///
/// The azimuths are already fixed by phyllotaxis, so the only free parameters
/// are the `n` tilts. They are solved for once per `n` by direct search over
/// the min-pair-angle, then reused: the sleeve's random `phase` rotates the
/// whole packing about the twig, which leaves every pair angle unchanged, so
/// one solved set serves every sleeve on every tree.
///
/// Stratifying the tilts instead (equal solid-angle bands, which is the right
/// answer for POINTS) measured worse than no tilt at all at n = 4: 42% of pairs
/// within 10 degrees of perpendicular, against 33% for azimuth spreading alone.
/// The measurement is `cluster_cards_in_one_tuft_are_not_a_box`.
fn sleeve_tilts(n: u32) -> Vec<f32> {
    let n = n.max(1) as usize;
    if n < 2 {
        return vec![0.0];
    }
    // The pair angle depends only on the azimuth GAP and the two tilts, so the
    // sleeve's phase drops out and this is solved in the sleeve's own frame.
    let az: Vec<f32> = (0..n).map(|k| k as f32 * PHYLLOTAXIS_RAD).collect();
    let dir = |k: usize, b: f32| {
        let (sb, cb) = b.sin_cos();
        let (sa, ca) = az[k].sin_cos();
        [ca * cb, sa * cb, -sb]
    };
    // Objective: the SMALLEST angle between any two card planes, folded to
    // 0..90 so a plane and its flip are one plane. Maximising it pushes the
    // set away from both parallel and perpendicular at once.
    let worst = |t: &[f32]| {
        let mut lo = f32::MAX;
        for i in 0..n {
            for j in (i + 1)..n {
                let d = dot(dir(i, t[i]), dir(j, t[j])).clamp(-1.0, 1.0).abs();
                lo = lo.min(d.acos().min(std::f32::consts::FRAC_PI_2 - d.acos().abs()));
            }
        }
        lo
    };
    // Seed: equal solid-angle bands, pulled in from the poles (a card at a pole
    // lies flat ACROSS the twig - a plate on the shoot, not leaves around it).
    let mut t: Vec<f32> = (0..n)
        .map(|k| (0.86 * (2.0 * (k as f32 + 0.5) / n as f32 - 1.0)).asin())
        .collect();
    let mut best = worst(&t);
    // Coordinate descent on a shrinking grid. Deterministic, and n is small.
    let mut step = 0.6f32;
    for _ in 0..40 {
        for k in 0..n {
            let (mut arg, keep) = (t[k], t[k]);
            for s in [-1.0f32, 1.0] {
                let cand = (keep + s * step).clamp(-1.2, 1.2);
                t[k] = cand;
                let v = worst(&t);
                if v > best {
                    best = v;
                    arg = cand;
                }
            }
            t[k] = arg;
        }
        step *= 0.8;
    }
    t
}

/// Solved sleeve tilts, memoised by card count. `sleeve_tilts` is a search, so
/// it must not run per sleeve; `n` is a handful of small values across the whole
/// registry, so a tiny map is the whole cache.
fn sleeve_tilts_cached(n: u32) -> &'static [f32] {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<u32, &'static [f32]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut g = cache.lock().unwrap_or_else(|e| e.into_inner());
    g.entry(n).or_insert_with(|| Box::leak(sleeve_tilts(n).into_boxed_slice()))
}

/// Emit ONE card: two windings of a quad (4 triangles), with each corner's
/// normal spherified twice - toward the cluster centre, then weakly toward the
/// crown centre - and the card's AO baked into its UV.
///
/// The same normal rides both windings on purpose. A card is a stand-in for a
/// ball of blades, and a blade lit from behind glows rather than going black
/// (the type-21 branch carries the leaf transmission term), so flipping the
/// normal on the back face would darken exactly the half that should be
/// luminous.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_card(
    b: &mut PlantMeshBuilder,
    centre: [f32; 3],
    facing: [f32; 3],
    wide: [f32; 3],
    tall: [f32; 3],
    half: f32,
    ao: f32,
    cluster_c: [f32; 3],
    crown_c: [f32; 3],
) {
    let corner = |sx: f32, sy: f32| add(add(centre, wide, sx * half), tall, sy * half);
    let p = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
    let uv01 = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut n = [[0.0f32; 3]; 4];
    let mut uv = [[0.0f32; 2]; 4];
    for i in 0..4 {
        // (a) CLUSTER SCALE: the tuft is a ball, so its normal at a point is
        //     mostly the direction out of the ball's centre.
        let out_cluster = norm(sub(p[i], cluster_c));
        let m1 = norm(mix3(facing, out_cluster, CLUSTER_NORMAL_BLEND));
        // (b) CROWN SCALE: and the crown is a bigger ball made of those.
        let out_crown = norm(sub(p[i], crown_c));
        n[i] = norm(mix3(m1, out_crown, CROWN_NORMAL_BLEND));
        uv[i] = encode_card_uv(uv01[i][0], uv01[i][1], ao);
    }
    b.card_tri([p[0], p[1], p[2]], [n[0], n[1], n[2]], [uv[0], uv[1], uv[2]]);
    b.card_tri([p[0], p[2], p[3]], [n[0], n[2], n[3]], [uv[0], uv[2], uv[3]]);
    // Reversed windings: the opaque pipeline back-culls, so without these a
    // card is invisible from one side.
    b.card_tri([p[0], p[2], p[1]], [n[0], n[2], n[1]], [uv[0], uv[2], uv[1]]);
    b.card_tri([p[0], p[3], p[2]], [n[0], n[3], n[2]], [uv[0], uv[3], uv[2]]);
}

/// One SLEEVE: `cards` cards around the twig axis at one station, each offset
/// off the twig and facing outward, and each tilted out of the twig-
/// perpendicular plane by its solved `sleeve_tilts` angle.
///
/// Facing outward rather than edge-on matters: the spherified normal then
/// AGREES with the card's own facing near its centre and only bends at the
/// corners, so a tuft rounds instead of fighting itself.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_sleeve(
    b: &mut PlantMeshBuilder,
    station: [f32; 3],
    axis: [f32; 3],
    side: f32,
    cards: u32,
    ao: f32,
    crown_c: [f32; 3],
    phase: f32,
) -> u32 {
    let up = if axis[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let s1 = norm(cross(axis, up));
    let s2 = norm(cross(axis, s1));
    let half = side * 0.5;
    let off = side * CLUSTER_SLEEVE_OFFSET;
    let n = cards.max(1);
    let tilts = sleeve_tilts_cached(n);
    for k in 0..n {
        // GOLDEN-ANGLE PHYLLOTAXIS, not equal azimuths (v0.1110).
        //
        // The operator: "the bunches of leaves look a little weird, kind of
        // like they're cubes." They were cubes. Equal azimuths with n = 4 put
        // cards at 0/90/180/270, so 0 and 2 are antiparallel planes, 1 and 3
        // are the other parallel pair, and the two pairs are exactly
        // perpendicular - the definition of a rectangular prism. Every card
        // also took `axis` as its up-vector, so every edge in a tuft ran
        // either along the twig or square to it, and the edge-on pair
        // projected to straight slivers spanning the tuft's full height.
        //
        // No plant does this. Oak, birch and cherry are alternate/spiral, and
        // successive leaves on a shoot are separated by the golden angle,
        // 137.507 degrees - the arrangement that never repeats and therefore
        // never lets two leaves shade each other the same way twice. Adopting
        // the real number is free: same card count, same triangles, same leaf
        // area, same LAI, same coverage, same overdraw.
        //
        // It is not SUFFICIENT, though, and the gate is what said so: azimuth
        // spacing alone still left a third of every 4-card tuft within 5
        // degrees of a right angle, because all four normals still lay on one
        // circle. The tilt below is the half that finished it.
        let th = phase + k as f32 * PHYLLOTAXIS_RAD;
        let (st, ct) = (th.sin(), th.cos());
        let r = norm([
            s1[0] * ct + s2[0] * st,
            s1[1] * ct + s2[1] * st,
            s1[2] * ct + s2[2] * st,
        ]);
        let t = norm(cross(axis, r));

        // TILT out of the twig-perpendicular plane, so the sleeve's normals
        // cover a sphere instead of a circle - the other half of the cube fix.
        // See `sleeve_tilts`. The card is rotated about its own WIDE
        // axis `t`, so it stays square and centred: `wide` is untouched, and
        // the pair (facing, tall) turns together through `tilt`.
        //
        // `emit_card`'s geometric normal is cross(wide, tall), and with
        // wide = t that equals `facing` for the pair below - the shading
        // normal and the drawn plane stay in agreement, which is the whole
        // reason the sleeve faces outward in the first place.
        //
        // The frame is right-handed as (axis, r, t): axis x r = t, r x t =
        // axis, t x axis = r. Rotating a vector v about t by b gives
        // v cos b + (t x v) sin b, so axis -> axis cb + r sb and r -> r cb -
        // axis sb. Those two signs are not interchangeable: get one wrong and
        // `facing` stops being perpendicular to `tall`, which silently shears
        // every card in the crown.
        let (sb, cb) = tilts[k as usize].sin_cos();
        let facing = norm([
            r[0] * cb - axis[0] * sb,
            r[1] * cb - axis[1] * sb,
            r[2] * cb - axis[2] * sb,
        ]);
        let tall = norm([
            axis[0] * cb + r[0] * sb,
            axis[1] * cb + r[1] * sb,
            axis[2] * cb + r[2] * sb,
        ]);

        // The OFFSET stays along the untilted radial: the card has to sit off
        // the twig it sleeves, and tilting the offset with the card would walk
        // it along the twig and out of the run of wood it is meant to cover.
        let c = add(station, r, off);
        emit_card(b, c, facing, t, tall, half, ao, station, crown_c);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::{as_procedural, registry};

/// THE BOX GATE. Operator, v0.1109.3: "the bunches of leaves look a little
/// weird, kind of like they're cubes."
///
/// They were cubes, and no gate could see it. `cluster_cards_reach_target_
/// lai_and_fit_the_budget` measures AREA, `..._envelop_every_twig_tip`
/// measures COVERAGE, `cluster_sprite_geometry_fits_its_card` measures
/// FIT - none of them looks at how the cards of one tuft are ORIENTED
/// relative to each other, which is the only thing that decides whether a
/// clump reads as a ball of leaves or as a rectangular prism.
///
/// Equal azimuths with `cards_per_sleeve = 4` put cards at 0/90/180/270:
/// two pairs of parallel planes, the pairs exactly perpendicular. Measured
/// on the drawn mesh before the fix, over intra-tuft card pairs, 66.7% sat
/// in the 80-90 degree bin and 33.3% in 0-10 - and every other bin was
/// EMPTY. A golden-angle helix lands near the 0.11 uniform value.
///
/// The face normal is recovered from the quad's own positions rather than
/// read from the stored vertex normal, because the stored one is
/// deliberately spherified toward the crown and would hide the very thing
/// this measures.
#[test]
fn cluster_cards_in_one_tuft_are_not_a_box() {
    let reg = registry();
    let mut checked = 0usize;
    for t in reg.trees.iter().map(as_procedural) {
        let Some(cd) = t.clusters.clone() else { continue };
        for (layer, ld) in [("leaf", &cd.leaf), ("blossom", &cd.blossom)] {
            let per = ld.cards_per_sleeve.max(1);
            if per < 2 {
                continue;
            }
            // Drive `emit_sleeve` itself, at the species' own card count,
            // over many twig directions and phases. Grouping is exact by
            // construction: one call is one sleeve. Recovering sleeve
            // membership from the finished crown mesh is not possible
            // without the untilted radial, and a proximity guess mixes
            // neighbouring sleeves in and dilutes the very signal here.
            let mut bins = [0usize; 9];
            let mut pairs = 0usize;
            let mut rng = Rng::new(0x5EED_B0_u64 ^ per as u64);
            for _ in 0..256 {
                let axis = norm([
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                ]);
                if !axis[0].is_finite() {
                    continue;
                }
                let mut b = PlantMeshBuilder::new();
                emit_sleeve(
                    &mut b,
                    [0.0, 0.0, 0.0],
                    axis,
                    ld.size_m.max(0.05),
                    per,
                    1.0,
                    [0.0, 40.0, 0.0], // crown centre far off: no spherify bias
                    rng.range(0.0, std::f32::consts::TAU),
                );
                // Face normal from POSITIONS. The stored vertex normal is
                // deliberately spherified toward the crown and would hide
                // exactly what this measures.
                let ns: Vec<[f32; 3]> = b
                    .vertices
                    .chunks_exact(12)
                    .map(|q| {
                        norm(cross(
                            sub(q[1].position, q[0].position),
                            sub(q[2].position, q[0].position),
                        ))
                    })
                    .collect();
                assert_eq!(ns.len(), per as usize, "{}: sleeve card count", t.id);
                for i in 0..ns.len() {
                    for j in (i + 1)..ns.len() {
                        // Fold to 0..90: a plane and its flip are one
                        // plane, so 170 degrees apart is 10, not 170.
                        let a = dot(ns[i], ns[j]).clamp(-1.0, 1.0).acos().to_degrees();
                        let a = a.min(180.0 - a);
                        bins[((a / 10.0) as usize).min(8)] += 1;
                        pairs += 1;
                    }
                }
            }
            assert!(pairs >= 200, "{} {layer}: only {pairs} pairs", t.id);
            checked += 1;
            let f = |b: usize| bins[b] as f32 / pairs as f32;
            // THE BOX BINS. Parallel (0-10) plus perpendicular (80-90) is
            // what a prism IS; no other part of the histogram distinguishes
            // a box from a spread. Normals uniform on the sphere put
            // 1-cos(10deg) + cos(80deg) = 0.189 here, so 0.25 admits real
            // structure and still fails anything that clusters on right
            // angles. Pre-fix: 1.000 at equal azimuths (n=2), 0.333 with
            // the golden angle alone (n=4).
            const BOXY_MAX: f32 = 0.25;
            let boxy = f(0) + f(8);
            let hist: Vec<String> = (0..9)
                .map(|b| format!("{}-{}:{:.2}", b * 10, b * 10 + 10, f(b)))
                .collect();
            eprintln!(
                "[box] {} {layer} x{per}: boxy={boxy:.3} (sphere 0.189) {}",
                t.id,
                hist.join(" ")
            );
            assert!(
                boxy <= BOXY_MAX,
                "{} {layer}: {:.1}% of the card pairs inside ONE sleeve are \
                 either parallel or perpendicular to each other (uniform on \
                 the sphere gives 18.9%). That is the definition of a \
                 rectangular prism, and it is what the operator saw: 'the \
                 bunches of leaves look a little weird, kind of like \
                 they're cubes.' Both halves of the fix are load-bearing - \
                 golden-angle azimuths spread the normals AROUND a circle, \
                 and the solved `sleeve_tilts` get them OFF it.",
                t.id,
                boxy * 100.0,
            );
        }
    }
    assert!(checked >= 8, "only {checked} layers reached the box gate");
}
}
