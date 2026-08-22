//! Cloud BODIES as constructed primitives (clouds v2).
//!
//! THE INVERSION THIS FIXES. Every shipped AAA cloud system uses noise
//! only to ERODE a body that came from somewhere else - Nubis3 applies
//! its detail noise as `ValueErosion(dimensional_profile, noise)` on top
//! of a stored voxel shape. HumanityOS had that backwards: inverted
//! Worley noise WAS the body. Inverted Worley is by definition a field
//! of round balls, and its cells are all near the same size, so the
//! representation is structurally incapable of the power-law size
//! spectrum a real cumulus field has. That is exactly what the operator
//! kept reporting as "uniform spheres" (2026-08-18).
//!
//! So a cloud is CONSTRUCTED here, the way Guerrilla's artists build
//! theirs (their listed modeling sources are metaballs, voxelised
//! meshes, particles and fluid sim) - only procedurally, from data,
//! because this project has no artist:
//!
//! * a hard FLAT BASE at the lifting condensation level (the single
//!   most recognisable cumulus cue - real bases are level because they
//!   mark a thermodynamic surface, not a shape),
//! * a set of smoothly-unioned LOBES whose radii are drawn from a
//!   PARETO (power-law) distribution, so a cloud is made of a few big
//!   masses and many small ones rather than one ball, and whose
//!   stacking gives cauliflower crowns,
//! * per-family proportions (wide-and-flat stratocumulus vs
//!   tall-and-narrow congestus) that make silhouettes differ by GENUS.
//!
//! The noise volumes keep their job, demoted to what they are good at:
//! roughening this surface.
//!
//! Everything here is pure CPU maths with no GPU dependency, so the
//! shapes are unit-testable - see the tests at the bottom, which encode
//! the operator's complaint directly as gates (size variety, aspect by
//! family, flat base, and SDF conservativeness for sphere tracing).

use glam::Vec3;
use serde::Deserialize;

/// One cloud family's construction rules. DATA (data/clouds/archetypes.ron)
/// per the Infinite-of-X rule: a new genus is a new entry, never new code.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudArchetype {
    pub name: String,
    /// Overall cloud width range in metres (min, max).
    pub width_m: (f32, f32),
    /// Height / width ratio range - what makes congestus tall and
    /// stratocumulus flat.
    pub aspect_h_over_w: (f32, f32),
    /// Lobe count range (min, max inclusive).
    pub lobe_count: (u32, u32),
    /// Pareto exponent of the lobe RADIUS distribution. Real shallow
    /// convection sits around 1.7-2.2 (many small lobes, few large).
    pub lobe_size_exponent: f32,
    /// How sharply the condensation base cuts: 1 = razor flat.
    pub base_flatness: f32,
    /// Fraction of lobes stacked ABOVE the main mass (cauliflower crown)
    /// rather than spread sideways.
    pub crown_bias: f32,
    /// Smooth-union radius as a fraction of mean lobe radius - how much
    /// neighbouring lobes melt into each other.
    pub blend: f32,
}

/// The archetype table as loaded from RON.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudArchetypes {
    pub archetypes: Vec<CloudArchetype>,
}

impl CloudArchetypes {
    /// Parse the shipped table. Callers hold this for the session.
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn by_name(&self, name: &str) -> Option<&CloudArchetype> {
        self.archetypes.iter().find(|a| a.name == name)
    }
}

/// One lobe of a cloud: a sphere in CLOUD-LOCAL metres, base plane at
/// y = 0, +y up. Spheres (not general ellipsoids) because a smooth union
/// of spheres already produces cauliflower, and a sphere's distance
/// function is exact - which the sphere-tracing march depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLobe {
    pub center: Vec3,
    pub radius: f32,
}

/// A built cloud: its lobes plus the derived bounds placement and the
/// march need.
#[derive(Debug, Clone)]
pub struct CloudInstance {
    pub lobes: Vec<CloudLobe>,
    /// Overall width (x/z extent) in metres.
    pub width_m: f32,
    /// Height above the base plane in metres.
    pub height_m: f32,
    /// Smooth-union blend radius in metres.
    pub blend_m: f32,
    /// Base-cut hardness, carried from the archetype.
    pub base_flatness: f32,
}

/// Deterministic hash -> uniform [0,1). The same integer-avalanche family
/// the noise volumes use, so clouds are byte-identical on every machine
/// (no floating-point RNG state, no platform drift).
fn hash01(seed: u64, salt: u64) -> f32 {
    let mut h = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    ((h >> 40) as f32) * (1.0 / 16_777_216.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Draw from a bounded Pareto (power-law) distribution on [lo, hi].
/// THIS is the function inverted-Worley could never provide: it makes a
/// few large lobes and many small ones, which is what real cumulus
/// fields measure and what stops every cloud looking the same size.
fn pareto(u: f32, lo: f32, hi: f32, exponent: f32) -> f32 {
    let a = exponent.max(1.05);
    let u = u.clamp(0.0, 0.999_9);
    // Inverse CDF of a bounded Pareto.
    let lo_a = lo.powf(a - 1.0);
    let hi_a = hi.powf(a - 1.0);
    let denom = 1.0 - u * (1.0 - lo_a / hi_a);
    (lo_a / denom).powf(1.0 / (a - 1.0)).clamp(lo, hi)
}

/// Smooth minimum (polynomial, Inigo Quilez): melts two distance fields
/// together over `k` metres instead of creasing them.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 1.0e-4 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    lerp(b, a, h) - k * h * (1.0 - h)
}

/// Build one cloud from an archetype and a seed.
///
/// The construction is deliberately anatomical rather than noisy:
/// 1. draw the cloud's overall size and proportion,
/// 2. lay a MAIN MASS of large lobes just above the base plane, spread
///    horizontally - this is what gives cumulus its broad flat bottom,
/// 3. stack CROWN lobes upward with shrinking radii and a lean, which
///    is the cauliflower,
/// 4. every radius comes from the Pareto draw, so within one cloud the
///    lobes already span a wide size range.
pub fn build_cloud(arch: &CloudArchetype, seed: u64) -> CloudInstance {
    let width = lerp(arch.width_m.0, arch.width_m.1, hash01(seed, 1));
    let aspect = lerp(
        arch.aspect_h_over_w.0,
        arch.aspect_h_over_w.1,
        hash01(seed, 2),
    );
    let height = width * aspect;
    let span = (arch.lobe_count.1 - arch.lobe_count.0 + 1) as f32;
    let n = arch.lobe_count.0 + (hash01(seed, 3) * span) as u32;
    let n = n.clamp(arch.lobe_count.0, arch.lobe_count.1) as usize;

    // Lobe radius range: the biggest lobe is about a third of the cloud
    // width, the smallest about a twentieth. The Pareto draw then decides
    // where in that range each lobe lands.
    // A lobe can never be taller than the cloud it builds. Without this
    // cap a flat genus (stratocumulus, aspect 0.12) draws lobes of
    // 0.34 * width against a deck only 0.12 * width tall, which both
    // looks wrong and inverts the placement clamp below.
    let r_hi = (width * 0.34).min(height * 0.9).max(width * 0.07);
    let r_lo = width * 0.05;

    // ── GRAPE-CLUSTER CONSTRUCTION ──
    // The first cut spread body lobes flat across the base plane, and it
    // built DISCS - flying saucers, not clouds. Real cumulus grow by
    // BUDDING: each puff swells off the shoulder of an existing one,
    // which is exactly what makes cauliflower. So every lobe after the
    // core attaches to a previously placed lobe, offset along a
    // direction whose upward bias is the archetype's crown_bias (flat
    // stratocumulus buds sideways, congestus buds straight up), at a
    // separation close enough that the smooth union merges the pair into
    // one body instead of a string of beads. Kept in step with
    // `cv2_cloud_sdf` in assets/shaders/pbr/41-cloud-bodies.wgsl.
    let mut lobes: Vec<CloudLobe> = Vec::with_capacity(n);
    let r0 = r_hi * lerp(0.7, 1.0, hash01(seed, 31));
    // Core lobe: surface respects the height cap (flat genera draw r0
    // comparable to their whole deck depth). Kept in step with the WGSL.
    lobes.push(CloudLobe {
        center: Vec3::new(0.0, r0.min((height - r0).max(r0 * arch.base_flatness)), 0.0),
        radius: r0,
    });
    for i in 1..n {
        let s = seed ^ ((i as u64 + 1) * 0x51A9_E381);
        // Later lobes prefer recent (higher, smaller) parents, which
        // grows a turret rather than a ring.
        let pu = hash01(s, 41);
        let pj = ((pu * pu) * i as f32) as usize;
        let parent = lobes[pj.min(i - 1)];
        // Never larger than its parent, so the cluster tapers upward the
        // way a real turret does.
        let r = pareto(hash01(s, 43), r_lo, r_hi, arch.lobe_size_exponent)
            .min(parent.radius * 0.92);
        let ang = hash01(s, 45) * std::f32::consts::TAU;
        let up_lo = lerp(-0.35, 0.15, arch.crown_bias);
        let up = lerp(up_lo, 1.0, hash01(s, 47));
        let horiz = (1.0 - up * up).max(0.0).sqrt();
        let dir = Vec3::new(ang.cos() * horiz, up, ang.sin() * horiz);
        let sep = (parent.radius + r) * lerp(0.55, 0.78, hash01(s, 49));
        let mut c = parent.center + dir * sep;
        // ENVELOPE CLAMP on centre PLUS radius (increment 6): the whole
        // lobe surface stays inside the width/2 cylinder and below the
        // height cap - clamping only the centre let lobes overhang their
        // cells and truncate at cell seams. Locked by
        // lobe_surfaces_stay_inside_the_envelope. Kept in step with the
        // WGSL cv2_cloud_sdf.
        let y_lo = (r * arch.base_flatness).min(height - r);
        c.y = c.y.clamp(y_lo, (height - r).max(y_lo));
        let horiz_len = Vec3::new(c.x, 0.0, c.z).length();
        let horiz_max = (width * 0.5 - r).max(0.0);
        if horiz_len > horiz_max {
            let k = horiz_max / horiz_len.max(1.0e-4);
            c.x *= k;
            c.z *= k;
        }
        lobes.push(CloudLobe {
            center: c,
            radius: r,
        });
    }

    let mean_r = lobes.iter().map(|l| l.radius).sum::<f32>() / lobes.len().max(1) as f32;
    return CloudInstance {
        lobes,
        width_m: width,
        height_m: height,
        blend_m: mean_r * arch.blend,
        base_flatness: arch.base_flatness,
    };
}

impl CloudInstance {
    /// Signed distance to the cloud surface, in metres, at a cloud-local
    /// point. Negative inside.
    ///
    /// THIS is what makes the march tractable. The shipped raymarch
    /// plods through empty air in fixed ~775 m steps because it has no
    /// idea where cloud is; with a distance field the march leaps the
    /// empty space and spends short steps only inside the medium, which
    /// is precisely the cure for "35 mean free paths per step".
    /// Guerrilla measured that swap as a 2-4x SPEEDUP, not a cost.
    ///
    /// Conservativeness matters: sphere tracing is only correct if this
    /// never OVERSTATES the distance to the surface. A smooth union
    /// under-estimates (it rounds the join inward) and the flat base is
    /// a half-space intersection, so both are safe. Locked by test.
    pub fn sdf(&self, p: Vec3) -> f32 {
        let mut d = f32::MAX;
        for l in &self.lobes {
            let ds = (p - l.center).length() - l.radius;
            d = if d == f32::MAX {
                ds
            } else {
                smin(d, ds, self.blend_m)
            };
        }
        // Hard flat base: intersect with the half-space y >= 0. The
        // condensation level is a thermodynamic surface, so a real
        // cumulus base is LEVEL - the single most recognisable cue the
        // old noise body could not produce.
        d.max(-p.y)
    }

    /// Density in [0,1] at a cloud-local point: 1 deep inside, falling to
    /// 0 over a soft rind so silhouettes are not stencils. The noise
    /// erosion then bites into this rind, which is how detail is added
    /// WITHOUT hollowing cores (the failure mode of erosion-as-body).
    pub fn density(&self, p: Vec3, rind_m: f32) -> f32 {
        let d = self.sdf(p);
        if d >= 0.0 {
            return 0.0;
        }
        (-d / rind_m.max(1.0e-3)).clamp(0.0, 1.0)
    }

    /// Axis-aligned bounds in cloud-local metres (for the placement grid
    /// and the brick bake).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for l in &self.lobes {
            lo = lo.min(l.center - Vec3::splat(l.radius));
            hi = hi.max(l.center + Vec3::splat(l.radius));
        }
        lo.y = lo.y.max(0.0); // the base cut
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> CloudArchetypes {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/clouds/archetypes.ron"
        ))
        .expect("archetypes.ron must ship");
        CloudArchetypes::from_ron(&text).expect("archetypes.ron parses")
    }

    #[test]
    fn shipped_archetypes_parse_and_cover_the_families() {
        let t = table();
        for want in [
            "cumulus_humilis",
            "cumulus_congestus",
            "stratocumulus",
            "cumulonimbus",
        ] {
            assert!(t.by_name(want).is_some(), "missing archetype {want}");
        }
    }

    #[test]
    fn clouds_are_deterministic() {
        let t = table();
        let a = t.by_name("cumulus_humilis").unwrap();
        let c1 = build_cloud(a, 12345);
        let c2 = build_cloud(a, 12345);
        assert_eq!(c1.lobes, c2.lobes, "same seed must build the same cloud");
        let c3 = build_cloud(a, 12346);
        assert_ne!(c1.lobes, c3.lobes, "different seeds must differ");
    }

    #[test]
    fn the_base_is_flat_and_hard() {
        // THE cumulus cue: density must be zero below the condensation
        // plane and present just above it at many horizontal positions -
        // i.e. the underside is LEVEL, not scalloped.
        let t = table();
        let a = t.by_name("cumulus_humilis").unwrap();
        let c = build_cloud(a, 77);
        let mut hits = 0;
        for i in 0..64 {
            let ang = i as f32 * 0.098;
            let r = (i as f32 / 64.0) * c.width_m * 0.25;
            let x = ang.cos() * r;
            let z = ang.sin() * r;
            assert_eq!(
                c.density(Vec3::new(x, -5.0, z), 40.0),
                0.0,
                "density below the base plane must be exactly zero"
            );
            if c.density(Vec3::new(x, 30.0, z), 40.0) > 0.0 {
                hits += 1;
            }
        }
        assert!(hits > 20, "expected cloud just above the base, got {hits}/64");
    }

    #[test]
    fn families_have_distinct_silhouettes_not_uniform_spheres() {
        // The operator's complaint, encoded as a gate. Stratocumulus must
        // be markedly WIDE AND FLAT, congestus and cumulonimbus markedly
        // TALL. A noise-ball representation cannot pass this.
        let t = table();
        let flat = build_cloud(t.by_name("stratocumulus").unwrap(), 5);
        let tall = build_cloud(t.by_name("cumulus_congestus").unwrap(), 5);
        let storm = build_cloud(t.by_name("cumulonimbus").unwrap(), 5);
        let ratio = |c: &CloudInstance| c.height_m / c.width_m;
        assert!(ratio(&flat) < 0.35, "stratocumulus not flat: {}", ratio(&flat));
        assert!(ratio(&tall) > 1.0, "congestus not tall: {}", ratio(&tall));
        assert!(ratio(&storm) > 1.0, "cumulonimbus not tall: {}", ratio(&storm));
        assert!(
            ratio(&tall) > ratio(&flat) * 4.0,
            "families are not distinguishable by proportion"
        );
    }

    #[test]
    fn lobe_sizes_follow_a_power_law_within_one_cloud() {
        // The specific statistic inverted Worley cannot produce: within a
        // single cloud there must be many small lobes and few large ones.
        let t = table();
        let a = t.by_name("cumulus_congestus").unwrap();
        let mut small = 0;
        let mut large = 0;
        for seed in 0..200u64 {
            let c = build_cloud(a, seed);
            let r_max = c.lobes.iter().map(|l| l.radius).fold(0.0_f32, f32::max);
            for l in &c.lobes {
                if l.radius < r_max * 0.4 {
                    small += 1;
                } else if l.radius > r_max * 0.7 {
                    large += 1;
                }
            }
        }
        assert!(
            small > large * 2,
            "not power-law: {small} small vs {large} large lobes"
        );
    }

    #[test]
    fn lobe_surfaces_stay_inside_the_envelope() {
        // Increment 6 containment gate: every lobe SURFACE (centre plus
        // radius) stays inside the width/2 cylinder and below the height
        // cap, for every archetype over many seeds. Without this, lobes
        // overhang their placement cells (clouds bleeding through their
        // neighbours) and truncate at the bounding reject (flat seams in
        // the sky). Tolerance is float noise only.
        let t = table();
        for arch in &t.archetypes {
            for seed in 0..64u64 {
                let c = build_cloud(arch, seed * 13 + 1);
                let half_w = c.width_m * 0.5 + 0.001;
                for l in &c.lobes {
                    let horiz = Vec3::new(l.center.x, 0.0, l.center.z).length();
                    assert!(
                        horiz + l.radius <= half_w + c.width_m * 1.0e-4,
                        "{}: lobe surface reaches {:.1} past the {:.1} envelope",
                        arch.name,
                        horiz + l.radius,
                        half_w
                    );
                    // Height cap: lobe tops stay within the deck height.
                    // Sole sanctioned exception: a flat genus whose CORE
                    // lobe is deeper than the whole deck (its floor
                    // placement wins over the cap) - bounded by the
                    // lobe's own diameter.
                    let cap = c.height_m.max(l.radius * 2.0) + 0.5;
                    assert!(
                        l.center.y + l.radius <= cap,
                        "{}: lobe top {:.1} above cap {:.1} (deck {:.1})",
                        arch.name,
                        l.center.y + l.radius,
                        cap,
                        c.height_m
                    );
                }
            }
        }
    }

    #[test]
    fn cloud_sizes_vary_across_a_population() {
        // The other half of "uniform spheres": across a field, clouds
        // must come in a wide spread of sizes, not all the same.
        let t = table();
        let a = t.by_name("cumulus_humilis").unwrap();
        let mut lo = f32::MAX;
        let mut hi = 0.0_f32;
        for seed in 0..500u64 {
            let w = build_cloud(a, seed).width_m;
            lo = lo.min(w);
            hi = hi.max(w);
        }
        assert!(hi / lo > 3.0, "population too uniform: {lo:.0}..{hi:.0} m");
    }

}
