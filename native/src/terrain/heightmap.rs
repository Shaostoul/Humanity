//! Procedural terrain generation on icosphere faces using layered noise.
//!
//! Generates deterministic terrain from a seed — continental shapes, mountain ranges,
//! hills, and micro detail. Also classifies biomes based on latitude, elevation,
//! and moisture.

use noise::{NoiseFn, Perlin, Seedable};
use serde::{Deserialize, Serialize};

/// Biome classification based on latitude, elevation, and moisture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    DeepOcean,
    Beach,
    Grassland,
    Forest,
    Rainforest,
    Desert,
    Tundra,
    Taiga,
    Mountain,
    SnowyMountain,
    Swamp,
    Savanna,
    Steppe,
    Volcanic,
    IceCap,
}

/// Procedural terrain generator using layered Perlin noise.
///
/// All generation is deterministic given the same seed.
pub struct TerrainGenerator {
    /// Master seed for all noise layers.
    seed: u64,
    /// Fraction of max height treated as sea level (0.0-1.0, default 0.4).
    sea_level: f32,
    /// Maximum terrain height in meters (default 8848.0 for Earth-like).
    mountain_scale: f32,
    /// Fractal roughness controlling high-frequency detail (0.0-1.0).
    roughness: f32,

    // Pre-built noise generators for each layer (different seeds).
    continental: Perlin,
    mountain: Perlin,
    hills: Perlin,
    micro: Perlin,
    moisture_noise: Perlin,
}

impl TerrainGenerator {
    /// Create a new terrain generator with the given parameters.
    pub fn new(seed: u64, sea_level: f32, mountain_scale: f32, roughness: f32) -> Self {
        let base_seed = seed as u32;
        Self {
            seed,
            sea_level: sea_level.clamp(0.0, 1.0),
            mountain_scale: mountain_scale.max(1.0),
            roughness: roughness.clamp(0.0, 1.0),
            continental: Perlin::new(base_seed),
            mountain: Perlin::new(base_seed.wrapping_add(1)),
            hills: Perlin::new(base_seed.wrapping_add(2)),
            micro: Perlin::new(base_seed.wrapping_add(3)),
            moisture_noise: Perlin::new(base_seed.wrapping_add(4)),
        }
    }

    /// Create a terrain generator with Earth-like defaults.
    pub fn earth_like(seed: u64) -> Self {
        Self::new(seed, 0.4, 8848.0, 0.6)
    }

    /// Get the sea level fraction.
    pub fn sea_level(&self) -> f32 {
        self.sea_level
    }

    /// Get the sea level in meters.
    pub fn sea_level_meters(&self) -> f32 {
        self.sea_level * self.mountain_scale
    }

    /// Generate height at a lat/lon coordinate.
    ///
    /// `lat` in radians (-PI/2 to PI/2), `lon` in radians (-PI to PI).
    /// Returns height in meters (0.0 to mountain_scale).
    pub fn generate_height(&self, lat: f32, lon: f32) -> f32 {
        let lat_d = lat as f64;
        let lon_d = lon as f64;

        // Layer 1: Continental shapes — very low frequency, large amplitude.
        // Controls the broad land/ocean distribution.
        let continental_freq = 1.0;
        let continental_val = self.continental.get([
            lat_d * continental_freq,
            lon_d * continental_freq,
        ]);
        // Normalize from [-1,1] to [0,1]
        let continental_h = (continental_val as f32 + 1.0) * 0.5;

        // Layer 2: Mountain ranges — medium frequency, moderate amplitude.
        let mountain_freq = 4.0;
        let mountain_val = self.mountain.get([
            lat_d * mountain_freq,
            lon_d * mountain_freq,
        ]);
        // Sharpen mountains: square the positive parts for ridgelines.
        let mountain_h = {
            let v = mountain_val as f32;
            if v > 0.0 { v * v } else { 0.0 }
        };

        // Layer 3: Hills and valleys — high frequency, small amplitude.
        let hills_freq = 16.0;
        let hills_val = self.hills.get([
            lat_d * hills_freq,
            lon_d * hills_freq,
        ]);
        let hills_h = (hills_val as f32 + 1.0) * 0.5;

        // Layer 4: Micro detail — rocks and bumps. Amplitude scaled by roughness.
        let micro_freq = 64.0;
        let micro_val = self.micro.get([
            lat_d * micro_freq,
            lon_d * micro_freq,
        ]);
        let micro_h = (micro_val as f32 + 1.0) * 0.5;

        // Combine layers with weights.
        // Continental dominates, mountain adds peaks, hills add texture, micro adds grit.
        let combined = continental_h * 0.55
            + mountain_h * 0.25
            + hills_h * 0.12
            + micro_h * 0.08 * self.roughness;

        // Clamp and scale to meters.
        combined.clamp(0.0, 1.0) * self.mountain_scale
    }

    /// Generate moisture value at a lat/lon coordinate.
    ///
    /// Returns moisture in range 0.0 (arid) to 1.0 (saturated).
    /// Moisture is influenced by latitude (equator is wetter) and noise variation.
    pub fn generate_moisture(&self, lat: f32, lon: f32) -> f32 {
        let lat_d = lat as f64;
        let lon_d = lon as f64;

        // Base moisture from latitude: higher near equator, lower at poles.
        let lat_factor = 1.0 - (lat.abs() / std::f32::consts::FRAC_PI_2);
        let lat_moisture = lat_factor * 0.6;

        // Noise-based variation.
        let noise_val = self.moisture_noise.get([lat_d * 3.0, lon_d * 3.0]);
        let noise_moisture = (noise_val as f32 + 1.0) * 0.5 * 0.4;

        (lat_moisture + noise_moisture).clamp(0.0, 1.0)
    }

    /// Determine biome from latitude, elevation, and moisture.
    ///
    /// `lat` in radians, `height` in meters, `moisture` in 0.0-1.0.
    pub fn get_biome(&self, lat: f32, height: f32, moisture: f32) -> Biome {
        let sea_m = self.sea_level_meters();
        let abs_lat = lat.abs();

        // --- Underwater ---
        if height < sea_m * 0.6 {
            return Biome::DeepOcean;
        }
        if height < sea_m {
            return Biome::Ocean;
        }

        // --- Shoreline ---
        let beach_threshold = sea_m + self.mountain_scale * 0.005;
        if height < beach_threshold {
            return if moisture > 0.8 { Biome::Swamp } else { Biome::Beach };
        }

        // --- High altitude ---
        let mountain_threshold = self.mountain_scale * 0.7;
        let snowy_threshold = self.mountain_scale * 0.85;

        if height > snowy_threshold {
            return Biome::SnowyMountain;
        }
        if height > mountain_threshold {
            return Biome::Mountain;
        }

        // --- Polar regions (latitude > ~60 degrees) ---
        let polar_lat = 60.0_f32.to_radians();
        let arctic_lat = 75.0_f32.to_radians();

        if abs_lat > arctic_lat {
            return Biome::IceCap;
        }
        if abs_lat > polar_lat {
            return if moisture > 0.4 { Biome::Taiga } else { Biome::Tundra };
        }

        // --- Temperate and tropical (by moisture) ---
        let tropical_lat = 23.5_f32.to_radians();

        if abs_lat < tropical_lat {
            // Tropical zone.
            if moisture > 0.7 {
                Biome::Rainforest
            } else if moisture > 0.4 {
                Biome::Savanna
            } else {
                Biome::Desert
            }
        } else {
            // Temperate zone.
            if moisture > 0.65 {
                Biome::Forest
            } else if moisture > 0.35 {
                Biome::Grassland
            } else if moisture > 0.15 {
                Biome::Steppe
            } else {
                Biome::Desert
            }
        }
    }

    /// Find the highest point by sampling the terrain.
    ///
    /// Returns `(lat, lon, height)` in (radians, radians, meters).
    /// Higher `samples` gives more accurate results but is slower.
    pub fn highest_point(&self, samples: u32) -> (f32, f32, f32) {
        let mut best_lat = 0.0_f32;
        let mut best_lon = 0.0_f32;
        let mut best_height = 0.0_f32;

        let steps = (samples as f32).sqrt().max(2.0) as u32;

        for lat_i in 0..steps {
            for lon_i in 0..steps {
                let lat = -std::f32::consts::FRAC_PI_2
                    + (lat_i as f32 / (steps - 1) as f32) * std::f32::consts::PI;
                let lon = -std::f32::consts::PI
                    + (lon_i as f32 / (steps - 1) as f32) * std::f32::consts::TAU;

                let h = self.generate_height(lat, lon);
                if h > best_height {
                    best_height = h;
                    best_lat = lat;
                    best_lon = lon;
                }
            }
        }

        (best_lat, best_lon, best_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_generation() {
        let gen = TerrainGenerator::earth_like(42);
        let h1 = gen.generate_height(0.5, 1.0);
        let h2 = gen.generate_height(0.5, 1.0);
        assert!((h1 - h2).abs() < f32::EPSILON, "Same inputs must produce same output");
    }

    #[test]
    fn test_height_in_range() {
        let gen = TerrainGenerator::earth_like(123);
        for lat_i in 0..10 {
            for lon_i in 0..10 {
                let lat = -1.5 + lat_i as f32 * 0.3;
                let lon = -3.0 + lon_i as f32 * 0.6;
                let h = gen.generate_height(lat, lon);
                assert!(h >= 0.0, "Height must be non-negative, got {}", h);
                assert!(h <= gen.mountain_scale, "Height must be <= mountain_scale, got {}", h);
            }
        }
    }

    #[test]
    fn test_moisture_in_range() {
        let gen = TerrainGenerator::earth_like(99);
        for i in 0..20 {
            let lat = -1.5 + i as f32 * 0.15;
            let m = gen.generate_moisture(lat, 0.0);
            assert!(m >= 0.0 && m <= 1.0, "Moisture out of range: {}", m);
        }
    }

    #[test]
    fn test_biome_ocean_below_sea_level() {
        let gen = TerrainGenerator::earth_like(42);
        let biome = gen.get_biome(0.0, 0.0, 0.5);
        assert_eq!(biome, Biome::DeepOcean);
    }

    #[test]
    fn test_biome_snowy_mountain() {
        let gen = TerrainGenerator::earth_like(42);
        let biome = gen.get_biome(0.5, 8000.0, 0.5);
        assert_eq!(biome, Biome::SnowyMountain);
    }

    #[test]
    fn test_biome_ice_cap_at_pole() {
        let gen = TerrainGenerator::earth_like(42);
        let biome = gen.get_biome(1.4, 4000.0, 0.3);
        assert_eq!(biome, Biome::IceCap);
    }

    #[test]
    fn test_highest_point_returns_valid() {
        let gen = TerrainGenerator::earth_like(42);
        let (lat, lon, height) = gen.highest_point(100);
        assert!(lat >= -std::f32::consts::FRAC_PI_2 && lat <= std::f32::consts::FRAC_PI_2);
        assert!(lon >= -std::f32::consts::PI && lon <= std::f32::consts::PI);
        assert!(height > 0.0);
    }

    #[test]
    fn test_different_seeds_differ() {
        let gen1 = TerrainGenerator::earth_like(1);
        let gen2 = TerrainGenerator::earth_like(2);
        // Different seeds should produce different heights (with overwhelming probability)
        let h1 = gen1.generate_height(0.3, 0.7);
        let h2 = gen2.generate_height(0.3, 0.7);
        assert!((h1 - h2).abs() > 0.01, "Different seeds should produce different terrain");
    }
}
