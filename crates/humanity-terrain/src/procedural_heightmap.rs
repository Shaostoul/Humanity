//! Synthesized elevation grids for bodies that ship NO measured heightmap
//! file (Moon, Mars, Pluto, and any modded world) — v0.919, the fix for the
//! operator's "icosphere stepping": without a `PlanetHeightmap` entry the
//! chunked-LOD ground never activates, so standing on the Moon meant standing
//! on the bare uniform icosphere — kilometer-wide flat facets with hard
//! creases (probe capture 2026-07-21). Synthesizing a grid at load time
//! lights up the ENTIRE existing Earth machinery for these bodies untouched:
//! chunked patches, the drawn==sampled ground clamp, detail-noise wrinkle,
//! and the per-pixel albedo texture bake (Moon/Mars/Pluto all ship real
//! albedo grids already).
//!
//! Everything is derived from fields the planet RON already carries
//! (`terrain_seed`, `noise_frequency`, `noise_octaves`, `atmosphere_color`,
//! `radius`) — no hardcoded per-body tables (infinite-of-x). Dedicated RON
//! knobs (crater density, relief amplitude) can be added as optional fields
//! when a modded body needs them.
//!
//! Shape recipe: fBm Perlin highlands + power-law stamped craters (parabolic
//! bowl, raised rim). Craters below ~2.5 grid cells are unresolvable at this
//! 1024x512 resolution — close-range small craters are a documented follow-up
//! (the moon.ron comment has tracked "real crater rings" since v0.763).

use noise::{NoiseFn, Perlin};

use super::planet::PlanetDef;
use super::planet_heightmap::{latlon_to_dir, quantize_meters, PlanetHeightmap};

/// Grid resolution: 0.35-degree cells (~10.6 km on the Moon). Macro relief
/// only — the chunked mesh layers `DetailNoise` sub-500 m octaves on top, so
/// close range still gets wrinkle. 1 MB resident per body.
pub const GRID_W: u32 = 1024;
pub const GRID_H: u32 = 512;

/// Peak-to-valley half-range in meters, scaled to the body. Moon (1737 km)
/// -> 8.7 km, Mars (3390 km) -> 12 km (clamped), Pluto (1188 km) -> 5.9 km —
/// all within a factor of ~2 of the real bodies' relief.
fn relief_m(def: &PlanetDef) -> f32 {
    (def.radius as f32 * 0.005).clamp(2_000.0, 12_000.0)
}

/// xorshift64* — deterministic, seedable, no rand dependency (same family the
/// vegetation streamer uses).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    fn f01(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Synthesize a heightmap for a def with no measured grid. Deterministic in
/// `terrain_seed`. The caller inserts the result into `planet_heightmaps`
/// and deliberately does NOT override the RON `sea_level` (for an airless
/// body that value is a color-band threshold — the Moon's maria line — not a
/// physical coastline).
pub fn synthesize(def: &PlanetDef) -> PlanetHeightmap {
    let relief = relief_m(def);
    let w = GRID_W as usize;
    let h = GRID_H as usize;
    let mut field = vec![0.0_f32; w * h];

    // ── fBm highlands: octaves of Perlin summed on the unit sphere. ──
    // Seed offsets 201.. keep these decorrelated from the SurfaceSampler
    // (0..2) and DetailNoise (101..114) families on the same seed.
    let s = def.terrain_seed as u32;
    let octaves = def.noise_octaves.clamp(3, 6) as usize;
    let perlins: Vec<Perlin> = (0..octaves)
        .map(|i| Perlin::new(s.wrapping_add(201 + i as u32)))
        .collect();
    let base_freq = def.noise_frequency.max(0.5) as f64;
    // Normalize by the geometric amp sum so the octave count never changes
    // the overall amplitude, only the texture.
    let amp_sum: f64 = (0..octaves).map(|i| 0.5_f64.powi(i as i32)).sum();
    for y in 0..h {
        let lat = 90.0 - (y as f32 + 0.5) * 180.0 / GRID_H as f32;
        for x in 0..w {
            let lon = -180.0 + (x as f32 + 0.5) * 360.0 / GRID_W as f32;
            let d = latlon_to_dir(lat, lon);
            let mut amp = 1.0_f64;
            let mut freq = base_freq;
            let mut sum = 0.0_f64;
            for p in &perlins {
                sum += amp * p.get([d.x as f64 * freq, d.y as f64 * freq, d.z as f64 * freq]);
                amp *= 0.5;
                freq *= 2.0;
            }
            // Half the relief budget goes to rolling highlands; craters
            // spend the rest.
            field[y * w + x] = (sum / amp_sum) as f32 * relief * 0.5;
        }
    }

    // ── Stamped craters: power-law sizes, parabolic bowl + raised rim. ──
    // Count scales with how much atmosphere erases impact history: airless
    // bodies keep ~500, a thick-atmosphere body keeps few. (Bodies with a
    // measured heightmap never reach this module.)
    let atmo_alpha = def.atmosphere_color.map_or(0.0, |c| c[3]).clamp(0.0, 1.0);
    let crater_count = (500.0 * (1.0 - atmo_alpha)) as usize;
    let radius_m = def.radius as f32;
    let cell_m = std::f32::consts::TAU * radius_m / GRID_W as f32;
    let r_min = cell_m * 2.5;
    let r_max = radius_m * 0.15;
    let mut rng = Rng(def.terrain_seed ^ 0x00C8_A7E5_5EED_2026);
    for _ in 0..crater_count {
        // Uniform point on the sphere: z uniform in [-1,1], lon uniform.
        let sin_lat = rng.f01() * 2.0 - 1.0;
        let clat = sin_lat.clamp(-1.0, 1.0).asin().to_degrees();
        let clon = rng.f01() * 360.0 - 180.0;
        let center = latlon_to_dir(clat, clon);
        // u^3 biases small: most craters near r_min, a rare few near r_max.
        let u = rng.f01();
        let r_m = r_min * (r_max / r_min).powf(u * u * u);
        let depth = (r_m * 0.12).min(relief * 0.6);
        let rim_h = depth * 0.3;
        let a_rad = r_m / radius_m; // angular radius of the rim circle
        let a_deg = a_rad.to_degrees();
        // Lat/lon bounding window (1.3x reaches past the rim falloff). The
        // longitude span stretches toward the poles; clamp to the full grid
        // when it wraps. Great-circle distance below is pole-safe.
        let half_lat = a_deg * 1.3;
        let half_lon = (a_deg * 1.3 / clat.to_radians().cos().abs().max(0.15)).min(180.0);
        let y0 = (((90.0 - (clat + half_lat)) / 180.0 * GRID_H as f32) - 0.5).floor() as i64;
        let y1 = (((90.0 - (clat - half_lat)) / 180.0 * GRID_H as f32) + 0.5).ceil() as i64;
        let x0 = ((((clon - half_lon) + 180.0) / 360.0 * GRID_W as f32) - 0.5).floor() as i64;
        let x1 = ((((clon + half_lon) + 180.0) / 360.0 * GRID_W as f32) + 0.5).ceil() as i64;
        for gy in y0.max(0)..=y1.min(GRID_H as i64 - 1) {
            let lat = 90.0 - (gy as f32 + 0.5) * 180.0 / GRID_H as f32;
            for gx in x0..=x1 {
                let xi = gx.rem_euclid(GRID_W as i64) as usize;
                let lon = -180.0 + (xi as f32 + 0.5) * 360.0 / GRID_W as f32;
                let dir = latlon_to_dir(lat, lon);
                let ang = center.dot(dir).clamp(-1.0, 1.0).acos();
                let d = ang / a_rad;
                if d >= 1.3 {
                    continue;
                }
                let cell = &mut field[gy as usize * w + xi];
                if d < 1.0 {
                    // Parabolic bowl: -depth at the center, 0 at the rim.
                    *cell += depth * (d * d - 1.0);
                }
                // Raised rim centered just outside the bowl edge.
                let rw = (1.0 - ((d - 1.05) / 0.25).powi(2)).max(0.0);
                *cell += rim_h * rw * rw;
            }
        }
    }

    // ── Quantize to the shared u16 grid format. ──
    let mut min_m = f32::MAX;
    let mut max_m = f32::MIN;
    for v in &field {
        min_m = min_m.min(*v);
        max_m = max_m.max(*v);
    }
    // Degenerate-range guard (a zero-relief def): widen so from_grid's
    // max > min validation always holds.
    if !(max_m > min_m) {
        max_m = min_m + 1.0;
    }
    let samples: Vec<u16> = field
        .iter()
        .map(|&v| quantize_meters(v, min_m, max_m))
        .collect();
    PlanetHeightmap::from_grid(GRID_W, GRID_H, min_m, max_m, samples)
        .expect("synthesized grid dimensions are static and range is guarded")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moonish_def() -> PlanetDef {
        let ron = r#"(
            name: "Testmoon",
            radius: 1737400.0,
            gravity: 1.62,
            terrain_seed: 7,
            ore_seed: 777,
            atmosphere_color: None,
            sea_level: 0.45,
        )"#;
        ron::from_str(ron).expect("test def parses")
    }

    #[test]
    fn synthesize_is_deterministic_and_in_range() {
        let def = moonish_def();
        let a = synthesize(&def);
        let b = synthesize(&def);
        assert_eq!(a.width(), GRID_W);
        assert_eq!(a.height(), GRID_H);
        // Deterministic: same seed, same grid, sample-for-sample.
        for (lat, lon) in [(0.0, 0.0), (45.0, 90.0), (-60.0, -120.0), (89.0, 179.0)] {
            assert_eq!(
                a.sample_meters_latlon(lat, lon),
                b.sample_meters_latlon(lat, lon),
                "non-deterministic at {lat},{lon}"
            );
        }
        // Range sanity: highlands (0.5) + crater bowls (0.6) + rims stay
        // within a small multiple of the relief budget.
        let relief = relief_m(&def);
        assert!(a.max_meters() > a.min_meters());
        assert!(
            (a.max_meters() - a.min_meters()) < 3.0 * relief,
            "range {}..{} blew past the relief budget {relief}",
            a.min_meters(),
            a.max_meters()
        );
    }

    #[test]
    fn craters_bite_visible_relief_into_the_highlands() {
        let def = moonish_def();
        let hm = synthesize(&def);
        // The grid must not be flat: scan a coarse lattice and require real
        // variation (craters + highlands together always exceed this).
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for y in 0..24 {
            for x in 0..48 {
                let lat = 88.0 - y as f32 * 176.0 / 23.0;
                let lon = -179.0 + x as f32 * 358.0 / 47.0;
                let v = hm.sample_meters_latlon(lat, lon);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        let relief = relief_m(&def);
        assert!(
            hi - lo > relief * 0.4,
            "synthesized moon reads nearly flat: {lo}..{hi} over relief {relief}"
        );
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let a = synthesize(&moonish_def());
        let mut def_b = moonish_def();
        def_b.terrain_seed = 8;
        let b = synthesize(&def_b);
        let mut differs = false;
        for (lat, lon) in [(0.0, 0.0), (30.0, 60.0), (-45.0, 10.0)] {
            if (a.sample_meters_latlon(lat, lon) - b.sample_meters_latlon(lat, lon)).abs() > 1.0 {
                differs = true;
            }
        }
        assert!(differs, "two seeds produced the same terrain");
    }
}
