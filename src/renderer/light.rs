//! Light TYPES registry (v0.571): the data-driven catalog of placeable lights, loaded from
//! `data/lighting/light_types.ron`. Mirrors the `wall_materials` / `lock_types` pattern
//! (`include_str!` + `OnceLock` + lookup-by-id). Pure serde/data -- no GPU types -- so it parses
//! everywhere.
//!
//! The renderer's PBR shader evaluates up to 8 lights (pos + colour + intensity + range), plus a
//! directional sun + fill. Stage 1 (v0.571) placed lights as DATA and resolved every kind into the
//! point-light path; Stage 2 (v0.639) gave `Spot` a REAL cone (aim direction + inner/outer angle,
//! see `RoomLight` + `spot_cone_attenuation`) evaluated in `pbr_simple.wgsl`. `Bar`/`Emissive` still
//! resolve as points -- their own shader stages (length falloff, emissive-surface synthesis) are a
//! later follow-up.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// What the light is (a fixed shader capability, so a closed enum -- adding a kind needs shader work,
/// per infinite-of-X's "closed set with code cost" exception). Spot renders a real cone (v0.639);
/// Bar/Emissive still resolve as a point.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightKind {
    #[default]
    Point,
    /// A cone light (cone_*_deg used, real shader cone as of v0.639).
    Spot,
    /// A linear/area light (length_m used). Shader support: a later stage.
    Bar,
    /// A glowing surface that also lights the room (a TV). Synthesized from an emissive surface later.
    Emissive,
}

/// One entry in the light catalog. Add a type by adding a line to `light_types.ron`.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct LightType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: LightKind,
    /// Linear RGB colour.
    pub color: (f32, f32, f32),
    /// Brightness multiplier (the shader's inverse-square term scales from here).
    pub intensity: f32,
    /// Falloff radius in metres (the light fades to nothing by here).
    pub range: f32,
    /// Spot cone (degrees) -- inner = full bright, outer = edge. Unused for Point/Bar. (later stage)
    #[serde(default)]
    pub cone_inner_deg: f32,
    #[serde(default)]
    pub cone_outer_deg: f32,
    /// Bar length in metres. Unused for Point/Spot. (later stage)
    #[serde(default)]
    pub length_m: f32,
    #[serde(default)]
    pub note: String,
}

/// The light catalog, parsed once from the embedded RON.
pub fn light_types() -> &'static [LightType] {
    static REG: std::sync::OnceLock<Vec<LightType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/lighting/light_types.ron");
        match ron::from_str::<Vec<LightType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("light_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a light type by its `id` (what a placed light stores).
pub fn light_type(id: &str) -> Option<&'static LightType> {
    light_types().iter().find(|t| t.id == id)
}

/// A light resolved to real GPU-ready values, one per placed (or auto-filled) light (v0.639
/// spot-cone rebuild). Every light is a point light with an OPTIONAL cone: `cos_outer > -1.0`
/// marks it spot-shaped; the sentinel `cos_outer == -1.0` means "shine in all directions," and
/// the shader's cone term is skipped entirely for it -- so every pre-existing Point/Bar light is
/// bit-for-bit unaffected by this struct's existence.
#[derive(Debug, Clone, Copy)]
pub struct RoomLight {
    pub pos: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    /// Aim direction (light-to-illuminated-area sense). Unused unless `cos_outer > -1.0`.
    pub dir: Vec3,
    pub cos_inner: f32,
    pub cos_outer: f32,
}

impl RoomLight {
    /// A plain point light: no cone (the sentinel `cos_outer = -1.0`).
    pub fn point(pos: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self { pos, color, intensity, range, dir: Vec3::NEG_Y, cos_inner: -1.0, cos_outer: -1.0 }
    }

    /// A LINE light (v0.786): the whole segment `a`..`b` emits. The shader
    /// lights each fragment from the CLOSEST point on the segment (the
    /// "capsule light" representative-point technique), so a light strip
    /// washes the full length of a wall instead of pooling at one point.
    /// Wire packing: pos = `a`, the `dir`/spot slot carries `b`, and
    /// `cos_outer = -2.0` is the LINE sentinel (< -1.5 in the shader; the
    /// plain point sentinel stays -1.0, a real spot is > -1.0).
    pub fn line(a: Vec3, b: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self { pos: a, color, intensity, range, dir: b, cos_inner: -1.0, cos_outer: -2.0 }
    }

    /// A spot light aimed along `dir` (normalized on construction; a zero-length input falls back
    /// to straight down so a light with an unset `PlacedLight::dir` still renders instead of NaN-ing
    /// the cone math), with `cone_inner_deg`/`cone_outer_deg` from the light's `LightType`.
    pub fn spot(
        pos: Vec3,
        color: [f32; 3],
        intensity: f32,
        range: f32,
        dir: Vec3,
        cone_inner_deg: f32,
        cone_outer_deg: f32,
    ) -> Self {
        let dir = if dir.length_squared() > 1e-6 { dir.normalize() } else { Vec3::NEG_Y };
        Self {
            pos,
            color,
            intensity,
            range,
            dir,
            cos_inner: cone_inner_deg.to_radians().cos(),
            cos_outer: cone_outer_deg.to_radians().cos(),
        }
    }
}

/// Hard clamp for a strip's corner subdivision (v0.792): the panel's drag
/// value stops at 100, and the samplers below clamp again so a hand-edited
/// save can't request a million samples per span.
pub const MAX_STRIP_SUBDIVISION: u32 = 100;

/// Tessellate a STRIP light's control path into render points (v0.781;
/// subdivision steps v0.792). The authoring model is a Blender-style path:
/// `points` are the control points (world coords, first = the light's pos),
/// and `subdivision` picks the corner style (operator: "like 1, 2, 3, up to
/// 100? 0 is sharp corners, everything after that is smooth"). 0 = the points
/// verbatim (hard mitered corners between straight tube segments). N >= 1 = a
/// Catmull-Rom curve THROUGH every control point (the same basis as the road
/// centerlines) with N EXTRA samples inserted per span (clamped to 100), the
/// end control points mirrored so the curve still starts/ends exactly at the
/// first and last points. It is N extra samples (N + 1 per span) rather than
/// N total because a single sample per span just re-emits the span's start
/// control point -- the sharp polyline again, the opposite of the operator's
/// "everything after 0 is smooth". Fewer than 2 points can't make a strip ->
/// returned as-is.
pub fn sample_strip_path(points: &[Vec3], subdivision: u32) -> Vec<Vec3> {
    if points.len() < 2 || subdivision == 0 {
        return points.to_vec();
    }
    let n = points.len();
    let get = |i: isize| -> Vec3 {
        if i < 0 {
            // Mirror before the start so the curve begins AT points[0].
            points[0] * 2.0 - points[1]
        } else if i as usize >= n {
            points[n - 1] * 2.0 - points[n - 2]
        } else {
            points[i as usize]
        }
    };
    let steps = subdivision.min(MAX_STRIP_SUBDIVISION) as usize + 1;
    let mut out = Vec::with_capacity((n - 1) * steps + 1);
    for seg in 0..(n - 1) {
        let p0 = get(seg as isize - 1);
        let p1 = get(seg as isize);
        let p2 = get(seg as isize + 1);
        let p3 = get(seg as isize + 2);
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            // Catmull-Rom basis (same coefficients as road_edge_centerline).
            out.push(
                (p1 * 2.0
                    + (p2 - p0) * t
                    + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
                    + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3)
                    * 0.5,
            );
        }
    }
    out.push(points[n - 1]);
    out
}

/// Hard ceiling on LINE-light segments emitted per strip (v0.792). The lights
/// storage buffer is uncapped (v0.782), so this is not a correctness limit --
/// it stops a 100-subdivision strip with many control points from pushing
/// thousands of per-fragment light evaluations for zero visible gain. Past the
/// ceiling only the EMISSION subdivision coarsens; the tube MESH keeps the
/// full subdivision (mesh vertices are cheap, shader lights are not).
pub const MAX_STRIP_EMISSION_SEGMENTS: usize = 256;

/// The line-light EMISSION segments for a strip (v0.792): `(a, b, share)` per
/// segment, sampled along the SAME curve the tube mesh renders. This is the
/// fix for the operator's screenshot complaint -- emission used to follow the
/// straight CONTROL polyline, so a curved strip's rounded sections floated
/// dark, lighting nothing. Each segment's `share` is its length fraction of
/// the whole sampled strip (shares sum to 1), so multiplying by the light's
/// intensity conserves total output no matter the subdivision. Pure math (no
/// GPU, no PlacedLight) so the split is unit-testable.
pub fn strip_emission_segments(points: &[Vec3], subdivision: u32) -> Vec<(Vec3, Vec3, f32)> {
    if points.len() < 2 {
        return Vec::new();
    }
    // Clamp the emission subdivision so total segments stay under the ceiling:
    // spans * (sub + 1) segments, so the largest sub that fits is
    // ceiling/spans - 1. A pathological strip with MORE control points than
    // the ceiling can't be coarsened below its own polyline (that would change
    // its shape), so it just emits one segment per span and eats the cost.
    let spans = points.len() - 1;
    let cap = ((MAX_STRIP_EMISSION_SEGMENTS / spans).max(1) - 1) as u32;
    let sub = subdivision.min(MAX_STRIP_SUBDIVISION).min(cap);
    let sampled = sample_strip_path(points, sub);
    let total: f32 = sampled.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f32>().max(1e-3);
    sampled
        .windows(2)
        .map(|w| (w[0], w[1], (w[1] - w[0]).length() / total))
        .collect()
}

/// The point on segment `a`..`b` closest to `fragment` (v0.786) -- the LINE
/// light's representative emission point. Pure-Rust mirror of the WGSL clamp
/// in `pbr_simple.wgsl`'s light loop, kept in lockstep so it is unit-testable
/// without a GPU (same pattern as `spot_cone_attenuation`).
pub fn line_light_closest_point(a: Vec3, b: Vec3, fragment: Vec3) -> Vec3 {
    let ab = b - a;
    let denom = ab.dot(ab).max(1e-6);
    let t = ((fragment - a).dot(ab) / denom).clamp(0.0, 1.0);
    a + ab * t
}

/// Cone attenuation factor in [0, 1] for a fragment at direction `light_to_fragment` (normalized,
/// pointing FROM the light TOWARD the fragment) against a spot aimed along `dir` (same sense).
/// Pure-Rust mirror of the `pbr_simple.wgsl` fragment-shader cone term -- kept in lockstep so this
/// is unit-testable without a GPU. `cos_outer <= -1.0` (the Point/Bar sentinel) always returns 1.0,
/// so a non-spot light is never touched by cone math.
pub fn spot_cone_attenuation(light_to_fragment: Vec3, dir: Vec3, cos_inner: f32, cos_outer: f32) -> f32 {
    if cos_outer <= -1.0 {
        return 1.0;
    }
    let cos_angle = dir.normalize_or_zero().dot(light_to_fragment.normalize_or_zero());
    smoothstep(cos_outer, cos_inner, cos_angle)
}

/// WGSL-equivalent `smoothstep(edge0, edge1, x)`: 0 at/below `edge0`, 1 at/above `edge1`, smooth
/// (cubic Hermite) between. `edge0 < edge1` is the caller's job to ensure (cos_outer < cos_inner
/// holds for any real cone since a wider angle has a smaller cosine).
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_type_registry_parses_and_has_a_point_preset() {
        let types = light_types();
        assert!(!types.is_empty(), "expected the seeded catalog");
        let p = types.iter().find(|t| t.kind == LightKind::Point).expect("a Point preset exists");
        assert!(p.intensity > 0.0 && p.range > 0.0, "a usable point light");
        assert!(light_type(&p.id).is_some());
        assert!(light_type("nope").is_none());
    }

    #[test]
    fn light_type_registry_has_a_spot_preset_with_a_real_cone() {
        let types = light_types();
        let s = types.iter().find(|t| t.kind == LightKind::Spot).expect("a Spot preset exists");
        assert!(s.cone_outer_deg > s.cone_inner_deg, "outer cone must be wider than inner");
        assert!(s.cone_inner_deg > 0.0);
    }

    #[test]
    fn spot_cone_full_bright_on_axis() {
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let cos_inner = 30.0_f32.to_radians().cos();
        let cos_outer = 45.0_f32.to_radians().cos();
        // Fragment directly below the fixture: light_to_fragment == dir exactly.
        let f = spot_cone_attenuation(dir, dir, cos_inner, cos_outer);
        assert!((f - 1.0).abs() < 1e-4, "on-axis should be full bright, got {f}");
    }

    #[test]
    fn spot_cone_cuts_off_past_the_outer_angle() {
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let cos_inner = 15.0_f32.to_radians().cos();
        let cos_outer = 30.0_f32.to_radians().cos();
        // 90 degrees off-axis: well past the outer cone.
        let perpendicular = Vec3::new(1.0, 0.0, 0.0);
        let f = spot_cone_attenuation(perpendicular, dir, cos_inner, cos_outer);
        assert_eq!(f, 0.0, "90 degrees off-axis must be fully cut off");
    }

    #[test]
    fn spot_cone_falls_off_monotonically_between_inner_and_outer() {
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let cos_inner = 10.0_f32.to_radians().cos();
        let cos_outer = 40.0_f32.to_radians().cos();
        let mut prev = 1.0_f32;
        for deg in [0.0_f32, 10.0, 20.0, 30.0, 40.0, 50.0] {
            let rad = deg.to_radians();
            let sample = Vec3::new(rad.sin(), 0.0, -rad.cos()).normalize();
            let f = spot_cone_attenuation(sample, dir, cos_inner, cos_outer);
            assert!(f <= prev + 1e-4, "cone factor must not increase as angle grows ({deg} deg: {f} > {prev})");
            prev = f;
        }
        assert_eq!(prev, 0.0, "well past the outer angle must be fully cut off");
    }

    #[test]
    fn spot_cone_sentinel_leaves_point_and_bar_lights_untouched() {
        // cos_outer == -1.0 is the Point/Bar sentinel: full bright from every direction.
        let dir = Vec3::new(0.3, -0.9, 0.1);
        for probe in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, dir] {
            let f = spot_cone_attenuation(probe, dir, -1.0, -1.0);
            assert_eq!(f, 1.0, "a non-spot light must never be dimmed by cone math");
        }
    }

    #[test]
    fn room_light_point_and_spot_constructors_produce_expected_sentinels() {
        let p = RoomLight::point(Vec3::ZERO, [1.0, 1.0, 1.0], 5.0, 3.0);
        assert_eq!(p.cos_outer, -1.0, "point light must carry the no-cone sentinel");

        let s = RoomLight::spot(Vec3::ZERO, [1.0, 1.0, 1.0], 5.0, 3.0, Vec3::new(0.0, -2.0, 0.0), 20.0, 40.0);
        assert!((s.dir.length() - 1.0).abs() < 1e-5, "spot direction must be normalized");
        assert!(s.cos_outer > -1.0 && s.cos_outer < s.cos_inner, "a real cone: outer wider than inner");

        // A zero-length dir must not NaN the light -- falls back to straight down.
        let degenerate = RoomLight::spot(Vec3::ZERO, [1.0, 1.0, 1.0], 5.0, 3.0, Vec3::ZERO, 20.0, 40.0);
        assert_eq!(degenerate.dir, Vec3::NEG_Y);
    }

    /// LINE light representative point (v0.786): fragments beside the segment
    /// light from the perpendicular foot; fragments past an end light from
    /// that endpoint (clamped). A degenerate zero-length segment behaves as a
    /// point light at `a`. Mirrors the WGSL clamp exactly.
    #[test]
    fn line_light_closest_point_clamps_to_segment() {
        let a = Vec3::new(0.0, 2.0, 0.0);
        let b = Vec3::new(4.0, 2.0, 0.0);
        // Beside the middle of the bar -> the perpendicular foot.
        let mid = line_light_closest_point(a, b, Vec3::new(2.0, 0.0, 1.0));
        assert!((mid - Vec3::new(2.0, 2.0, 0.0)).length() < 1e-5);
        // Way past the B end -> clamped to B.
        let past = line_light_closest_point(a, b, Vec3::new(9.0, 0.0, 0.0));
        assert!((past - b).length() < 1e-5);
        // Before the A end -> clamped to A.
        let before = line_light_closest_point(a, b, Vec3::new(-3.0, 2.0, 2.0));
        assert!((before - a).length() < 1e-5);
        // Zero-length segment -> a point light at A (no NaN from the 1e-6 floor).
        let degen = line_light_closest_point(a, a, Vec3::new(7.0, 7.0, 7.0));
        assert!((degen - a).length() < 1e-5);

        // The line-light RoomLight carries the packing contract: pos=a, dir=b,
        // cos_outer=-2.0 (the LINE sentinel the shader tests with < -1.5).
        let l = RoomLight::line(a, b, [1.0, 1.0, 1.0], 6.0, 5.0);
        assert_eq!(l.pos, a);
        assert_eq!(l.dir, b);
        assert!(l.cos_outer < -1.5 && l.cos_outer > -2.5);
        // And spot cone math must SKIP it (sentinel is not a real cone).
        assert_eq!(spot_cone_attenuation(Vec3::X, Vec3::X, l.cos_inner, l.cos_outer), 1.0);
    }

    /// Strip path tessellation (v0.781, subdivision steps v0.792): 0 = points
    /// verbatim (sharp); N >= 1 = a Catmull-Rom curve that still STARTS and
    /// ENDS exactly on the first/last control points (mirrored ends), passes
    /// through the middle control points, and produces N + 1 samples per span.
    /// An L-shaped smooth strip must actually round the corner (its curve
    /// deviates from the straight legs near the corner).
    #[test]
    fn strip_path_sampling_sharp_and_subdivided() {
        let l_shape = vec![
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(4.0, 2.0, 0.0),
            Vec3::new(4.0, 2.0, 4.0),
        ];

        // Subdivision 0: verbatim (sharp corners).
        let sharp = sample_strip_path(&l_shape, 0);
        assert_eq!(sharp, l_shape);

        // Subdivision 8 (the serde default / old smooth look): 2 spans * 9 + terminal point.
        let smooth = sample_strip_path(&l_shape, 8);
        assert_eq!(smooth.len(), 2 * 9 + 1);
        assert!((smooth[0] - l_shape[0]).length() < 1e-4, "curve starts at the first point");
        assert!((smooth[18] - l_shape[2]).length() < 1e-4, "curve ends at the last point");
        // The corner control point is ON the curve (Catmull-Rom interpolates).
        assert!((smooth[9] - l_shape[1]).length() < 1e-4, "curve passes through the corner point");
        // But near the corner the curve bows off the sharp L (rounding): the
        // sample two-thirds along span 0 is pulled off the straight segment.
        let straight_pt = Vec3::new(4.0 * (6.0 / 9.0), 2.0, 0.0); // 2/3 along the sharp first leg
        let curved_pt = smooth[6]; // t = 6/9 of span 0
        assert!(
            (curved_pt - straight_pt).length() > 0.05,
            "smooth curve must deviate from the sharp corner path (got {curved_pt:?})"
        );

        // Subdivision 1 must ALREADY round (operator: "0 is sharp corners,
        // everything after that is smooth"): one extra sample per span, and
        // that midpoint sample sits off the straight leg.
        let sub1 = sample_strip_path(&l_shape, 1);
        assert_eq!(sub1.len(), 2 * 2 + 1);
        let mid = sub1[1]; // t = 0.5 of span 0
        assert!(
            (mid - Vec3::new(2.0, 2.0, 0.0)).length() > 0.05,
            "subdivision 1 must visibly round the corner (got {mid:?})"
        );

        // Out-of-range subdivision clamps to 100 (2 spans * 101 + terminal).
        let clamped = sample_strip_path(&l_shape, 5000);
        assert_eq!(clamped.len(), 2 * 101 + 1);

        // Degenerate inputs pass through untouched.
        assert_eq!(sample_strip_path(&l_shape[..1], 8).len(), 1);
        assert!(sample_strip_path(&[], 8).is_empty());
    }

    /// Strip EMISSION segments (v0.792): the light segments follow the SAME
    /// sampled curve as the tube mesh, split the intensity by length share
    /// (shares sum to 1 -- energy conservation), and never exceed the
    /// per-strip segment ceiling however high the subdivision goes.
    #[test]
    fn strip_emission_segments_split_intensity_by_length() {
        // Sharp two-leg path: 1 m then 3 m -> shares 0.25 / 0.75 exactly.
        let path = vec![
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(4.0, 2.0, 0.0),
        ];
        let segs = strip_emission_segments(&path, 0);
        assert_eq!(segs.len(), 2, "subdivision 0 emits one segment per control leg");
        assert!((segs[0].2 - 0.25).abs() < 1e-4, "1 m of 4 m total = 0.25, got {}", segs[0].2);
        assert!((segs[1].2 - 0.75).abs() < 1e-4, "3 m of 4 m total = 0.75, got {}", segs[1].2);
        assert_eq!(segs[0].0, path[0]);
        assert_eq!(segs[1].1, path[2]);

        // Subdivided: segments follow the sampled curve, shares still sum to 1.
        let l_shape = vec![
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(4.0, 2.0, 0.0),
            Vec3::new(4.0, 2.0, 4.0),
        ];
        let segs = strip_emission_segments(&l_shape, 8);
        assert_eq!(segs.len(), 2 * 9, "one segment per sampled step");
        let sum: f32 = segs.iter().map(|s| s.2).sum();
        assert!((sum - 1.0).abs() < 1e-3, "shares must sum to 1, got {sum}");
        // Consecutive segments are CONTIGUOUS (each starts where the last ended)
        // so the emitted line lights trace one unbroken curve.
        for w in segs.windows(2) {
            assert!((w[0].1 - w[1].0).length() < 1e-5, "segments must chain end-to-start");
        }

        // The ceiling: 8 control points (7 spans) at subdivision 100 would be
        // 7 * 101 = 707 segments raw; the emission clamp coarsens it under 256
        // while the shares still cover the whole strip.
        let long: Vec<Vec3> = (0..8).map(|i| Vec3::new(i as f32 * 2.0, 2.0, (i % 2) as f32)).collect();
        let segs = strip_emission_segments(&long, 100);
        assert!(segs.len() <= MAX_STRIP_EMISSION_SEGMENTS, "capped, got {}", segs.len());
        assert!(segs.len() > 7, "still subdivided (not collapsed to the control legs)");
        let sum: f32 = segs.iter().map(|s| s.2).sum();
        assert!((sum - 1.0).abs() < 1e-3, "capped shares still sum to 1, got {sum}");

        // Degenerate: fewer than 2 points emit nothing.
        assert!(strip_emission_segments(&l_shape[..1], 8).is_empty());
        assert!(strip_emission_segments(&[], 8).is_empty());
    }
}
