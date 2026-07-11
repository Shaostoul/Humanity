//! Real-elevation heightmaps for sky planets (Earth ships with one).
//!
//! A planet def (data/planets/<id>.ron) can point at a lat/lon elevation grid
//! via its `heightmap` field. When present, `terrain::planet_surface` samples
//! vertex elevations from this grid instead of the seeded Perlin noise, so
//! the sky planet shows REAL continents (Earth's grid is downsampled from
//! NOAA ETOPO1, public domain; see scripts/build-earth-heightmap.js for the
//! full provenance + the exact file format written).
//!
//! File format ("HOSHGT1", all little-endian):
//!   bytes 0..7   magic b"HOSHGT1"
//!   bytes 7..11  u32 width   (longitude samples; wraps around)
//!   bytes 11..15 u32 height  (latitude samples; clamps at the poles)
//!   bytes 15..19 f32 min_m   (elevation in meters encoded by quantum 0)
//!   bytes 19..23 f32 max_m   (elevation in meters encoded by quantum 65535)
//!   bytes 23..   width*height u16 quantized samples, row-major, row 0 = the
//!                northernmost row, column 0 = the westernmost column.
//!
//! Grid registration is CELL-CENTERED: sample (row r, col c) is the mean
//! elevation of the cell centered at
//!   lat = 90 - (r + 0.5) * 180 / height    (degrees, +north)
//!   lon = -180 + (c + 0.5) * 360 / width   (degrees, +east)
//! Bilinear interpolation between the four surrounding cell centers gives a
//! continuous field; longitude wraps (col width-1 blends into col 0 across
//! the antimeridian), latitude clamps (above the northernmost cell center
//! the top row's value holds -- correct for polar caps, which are near-flat).
//!
//! Everything here is pure math + std IO, no GPU and no native-only deps, so
//! it compiles in every feature set (terrain/ is an ungated module) and is
//! fully unit-testable headless.

use glam::Vec3;
use std::path::Path;

/// Magic bytes at the start of every HumanityOS planet heightmap file.
pub const HEIGHTMAP_MAGIC: &[u8; 7] = b"HOSHGT1";

/// Header size in bytes: magic(7) + width(4) + height(4) + min(4) + max(4).
const HEADER_LEN: usize = 23;

/// A loaded planet elevation grid. Earth's shipped grid is 3600x1800
/// (0.1 degree cells) = ~12.4 MB resident, loaded once per session.
pub struct PlanetHeightmap {
    width: u32,
    height: u32,
    /// Elevation (meters relative to sea level) encoded by quantum 0.
    min_m: f32,
    /// Elevation (meters relative to sea level) encoded by quantum 65535.
    max_m: f32,
    /// Quantized samples, row-major, north-to-south / west-to-east.
    samples: Vec<u16>,
}

impl PlanetHeightmap {
    /// Parse a heightmap from raw file bytes. Errors are strings because the
    /// only caller path is "log a warning + fall back to procedural noise".
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN {
            return Err(format!("heightmap too small: {} bytes", bytes.len()));
        }
        if &bytes[0..7] != HEIGHTMAP_MAGIC {
            return Err("bad heightmap magic (expected HOSHGT1)".to_string());
        }
        let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f32_at = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let width = u32_at(7);
        let height = u32_at(11);
        let min_m = f32_at(15);
        let max_m = f32_at(19);
        if width == 0 || height == 0 {
            return Err(format!("heightmap has degenerate dimensions {width}x{height}"));
        }
        if !(max_m > min_m) {
            // Also catches NaN: a NaN range would poison every sample.
            return Err(format!("heightmap range invalid: min {min_m} max {max_m}"));
        }
        let expected = width as usize * height as usize * 2;
        let payload = &bytes[HEADER_LEN..];
        if payload.len() != expected {
            return Err(format!(
                "heightmap payload is {} bytes, expected {expected} for {width}x{height}",
                payload.len()
            ));
        }
        let samples: Vec<u16> = payload
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(Self { width, height, min_m, max_m, samples })
    }

    /// Load a heightmap file from disk.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn min_meters(&self) -> f32 { self.min_m }
    pub fn max_meters(&self) -> f32 { self.max_m }

    /// Where real sea level (0 m) lands in the normalized 0..1 elevation
    /// domain that `planet_surface`'s color/sea machinery consumes. The
    /// loader overrides the RON `sea_level` with this so the data's true
    /// coastline is exact instead of hand-tuned.
    pub fn sea_level_normalized(&self) -> f32 {
        (0.0 - self.min_m) / (self.max_m - self.min_m)
    }

    /// Dequantized elevation (meters) of one grid cell, with longitude WRAP
    /// and latitude CLAMP applied to the integer coordinates. Accepting i64
    /// lets the bilinear caller pass raw floor()+1 neighbors without its own
    /// edge handling.
    fn grid_meters(&self, x: i64, y: i64) -> f32 {
        let w = self.width as i64;
        let h = self.height as i64;
        // rem_euclid wraps negatives correctly (-1 -> w-1), unlike %.
        let xi = x.rem_euclid(w) as usize;
        let yi = y.clamp(0, h - 1) as usize;
        let q = self.samples[yi * self.width as usize + xi] as f32;
        self.min_m + (q / 65535.0) * (self.max_m - self.min_m)
    }

    /// Bilinear elevation (meters) at a geographic coordinate.
    /// lat in degrees (+north), lon in degrees (+east); any lon accepted.
    pub fn sample_meters_latlon(&self, lat_deg: f32, lon_deg: f32) -> f32 {
        // Continuous grid coordinates of this lat/lon: cell centers sit at
        // (index + 0.5) cell-widths from the grid edge, so subtract 0.5 to
        // make integer coordinates land exactly on cell centers.
        let fx = (lon_deg + 180.0) / 360.0 * self.width as f32 - 0.5;
        let fy = (90.0 - lat_deg) / 180.0 * self.height as f32 - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (x0, y0) = (x0 as i64, y0 as i64);
        // Four surrounding cell centers; grid_meters handles wrap/clamp.
        let h00 = self.grid_meters(x0, y0);
        let h10 = self.grid_meters(x0 + 1, y0);
        let h01 = self.grid_meters(x0, y0 + 1);
        let h11 = self.grid_meters(x0 + 1, y0 + 1);
        let top = h00 + (h10 - h00) * tx;
        let bot = h01 + (h11 - h01) * tx;
        top + (bot - top) * ty
    }

    /// Bilinear elevation (meters) at a unit-sphere direction.
    ///
    /// Coordinate convention (matches the icosphere/renderer): +Y is the
    /// north pole, so sin(latitude) = y. Longitude = atan2(-z, x): the -z
    /// makes EAST point to a viewer's right when looking at the sphere from
    /// outside with north up (right-handed Y-up space), so the continents
    /// are not mirror-flipped. The absolute lon-0 meridian orientation is
    /// arbitrary (the planet visibly rotates anyway); the handedness is not.
    pub fn sample_meters(&self, unit: Vec3) -> f32 {
        let lat = unit.y.clamp(-1.0, 1.0).asin().to_degrees();
        let lon = (-unit.z).atan2(unit.x).to_degrees();
        self.sample_meters_latlon(lat, lon)
    }

    /// Elevation at a unit-sphere direction, normalized to the 0..1 domain
    /// `planet_surface::displaced_radius` / `classify_color` consume
    /// (0 = min_m, 1 = max_m -- pair with `sea_level_normalized`).
    pub fn normalized_at(&self, unit: Vec3) -> f32 {
        ((self.sample_meters(unit) - self.min_m) / (self.max_m - self.min_m)).clamp(0.0, 1.0)
    }
}

/// Quantize meters to the file's u16 domain. Lives here (not only in the JS
/// build script) so the round-trip precision is lockable by a Rust test;
/// keep in sync with scripts/build-earth-heightmap.js `quantize()`.
pub fn quantize_meters(h_m: f32, min_m: f32, max_m: f32) -> u16 {
    let t = ((h_m - min_m) / (max_m - min_m)).clamp(0.0, 1.0);
    (t * 65535.0).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an in-memory heightmap from a meters grid (row-major,
    /// north-first) via the same byte format the JS script writes.
    fn synth(width: u32, height: u32, min_m: f32, max_m: f32, meters: &[f32]) -> PlanetHeightmap {
        assert_eq!(meters.len(), (width * height) as usize);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&min_m.to_le_bytes());
        bytes.extend_from_slice(&max_m.to_le_bytes());
        for &m in meters {
            bytes.extend_from_slice(&quantize_meters(m, min_m, max_m).to_le_bytes());
        }
        PlanetHeightmap::from_bytes(&bytes).expect("synthetic heightmap parses")
    }

    /// 4 wide x 2 tall test grid. Cell centers: lats +45 (row 0) and -45
    /// (row 1); lons -135, -45, +45, +135 (cols 0..3). Range chosen so
    /// every test value quantizes EXACTLY (multiples of 1000 over a 65535
    /// span do not, so assertions below use a 1-quantum epsilon).
    fn grid_2x4() -> PlanetHeightmap {
        synth(
            4,
            2,
            -1000.0,
            1000.0,
            &[
                // north row: -135, -45, +45, +135
                100.0, 200.0, 300.0, 400.0,
                // south row
                -100.0, -200.0, -300.0, -400.0,
            ],
        )
    }

    /// One quantum of the test grid's 2000 m range: the max error the u16
    /// encoding can introduce on any single stored value.
    const Q: f32 = 2000.0 / 65535.0;

    #[test]
    fn exact_cell_centers_return_stored_values() {
        let hm = grid_2x4();
        assert!((hm.sample_meters_latlon(45.0, -135.0) - 100.0).abs() <= Q);
        assert!((hm.sample_meters_latlon(45.0, 45.0) - 300.0).abs() <= Q);
        assert!((hm.sample_meters_latlon(-45.0, 135.0) - -400.0).abs() <= Q);
    }

    #[test]
    fn bilinear_midpoints_average_neighbors() {
        let hm = grid_2x4();
        // Halfway between cols 0 and 1 on the north row: (100+200)/2.
        assert!((hm.sample_meters_latlon(45.0, -90.0) - 150.0).abs() <= Q);
        // Halfway between the rows at col 1: (200 + -200)/2 = 0.
        assert!((hm.sample_meters_latlon(0.0, -45.0) - 0.0).abs() <= Q);
        // Center of all four: (100+200-100-200)/4 = 0.
        assert!((hm.sample_meters_latlon(0.0, -90.0) - 0.0).abs() <= Q);
    }

    #[test]
    fn longitude_wraps_across_antimeridian() {
        let hm = grid_2x4();
        // lon 180 is halfway between col 3 (+135) and col 0 (-135, wrapped):
        // north row (400+100)/2 = 250. Both +180 and -180 must agree.
        assert!((hm.sample_meters_latlon(45.0, 180.0) - 250.0).abs() <= Q);
        assert!((hm.sample_meters_latlon(45.0, -180.0) - 250.0).abs() <= Q);
    }

    #[test]
    fn latitude_clamps_at_poles() {
        let hm = grid_2x4();
        // Above the northernmost cell centers the top row holds; the pole
        // itself still bilinears in LONGITUDE along that row.
        assert!((hm.sample_meters_latlon(90.0, -135.0) - 100.0).abs() <= Q);
        assert!((hm.sample_meters_latlon(-90.0, -135.0) - -100.0).abs() <= Q);
        // No panic at extreme inputs beyond the physical range.
        let _ = hm.sample_meters_latlon(9999.0, -9999.0);
    }

    #[test]
    fn direction_sampling_matches_latlon_convention() {
        let hm = grid_2x4();
        // +Y is the north pole -> clamps to the north row.
        let north = hm.sample_meters(Vec3::new(0.0, 1.0, 0.0));
        assert!((north - hm.sample_meters_latlon(90.0, 0.0)).abs() <= Q);
        // +X on the equator is lon 0 (atan2(-0, 1) = 0).
        let px = hm.sample_meters(Vec3::new(1.0, 0.0, 0.0));
        assert!((px - hm.sample_meters_latlon(0.0, 0.0)).abs() <= Q);
        // -Z on the equator must be lon +90 EAST (handedness: east is -z).
        let mz = hm.sample_meters(Vec3::new(0.0, 0.0, -1.0));
        assert!((mz - hm.sample_meters_latlon(0.0, 90.0)).abs() <= Q);
    }

    #[test]
    fn quantization_roundtrip_within_one_quantum() {
        let (min_m, max_m) = (-11000.0_f32, 9000.0_f32);
        let quantum = (max_m - min_m) / 65535.0; // ~0.305 m for Earth's range
        for h in [-11000.0, -7000.0, -3210.5, 0.0, 1.25, 848.86, 8848.0, 9000.0] {
            let q = quantize_meters(h, min_m, max_m);
            let back = min_m + (q as f32 / 65535.0) * (max_m - min_m);
            assert!(
                (back - h).abs() <= quantum,
                "{h} m round-tripped to {back} m (err {} > quantum {quantum})",
                (back - h).abs()
            );
        }
        // Out-of-range inputs clamp instead of wrapping.
        assert_eq!(quantize_meters(-99999.0, min_m, max_m), 0);
        assert_eq!(quantize_meters(99999.0, min_m, max_m), 65535);
    }

    #[test]
    fn sea_level_normalized_maps_zero_meters() {
        let hm = grid_2x4();
        // min -1000, max 1000 -> 0 m sits exactly at 0.5.
        assert!((hm.sea_level_normalized() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn normalized_at_spans_zero_to_one() {
        let hm = grid_2x4();
        for dir in [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.577, 0.577, 0.577),
        ] {
            let n = hm.normalized_at(dir);
            assert!((0.0..=1.0).contains(&n), "normalized out of range: {n}");
        }
    }

    #[test]
    fn rejects_malformed_files() {
        assert!(PlanetHeightmap::from_bytes(b"short").is_err());
        // Wrong magic.
        let mut bad = Vec::from(*b"NOTHGT1");
        bad.extend_from_slice(&[0u8; 16]);
        assert!(PlanetHeightmap::from_bytes(&bad).is_err());
        // Right magic, payload size mismatch (claims 2x2 but carries 1 sample).
        let mut trunc = Vec::from(*HEIGHTMAP_MAGIC);
        trunc.extend_from_slice(&2u32.to_le_bytes());
        trunc.extend_from_slice(&2u32.to_le_bytes());
        trunc.extend_from_slice(&0.0f32.to_le_bytes());
        trunc.extend_from_slice(&1.0f32.to_le_bytes());
        trunc.extend_from_slice(&[0u8, 0u8]);
        assert!(PlanetHeightmap::from_bytes(&trunc).is_err());
        // Inverted range.
        let mut inv = Vec::from(*HEIGHTMAP_MAGIC);
        inv.extend_from_slice(&1u32.to_le_bytes());
        inv.extend_from_slice(&1u32.to_le_bytes());
        inv.extend_from_slice(&5.0f32.to_le_bytes());
        inv.extend_from_slice(&(-5.0f32).to_le_bytes());
        inv.extend_from_slice(&[0u8, 0u8]);
        assert!(PlanetHeightmap::from_bytes(&inv).is_err());
    }

    /// Anchor test over the COMMITTED Earth grid: reads the shipped
    /// data/planets/earth_heightmap.bin directly and checks real-world
    /// features are where they belong. Locks provenance + byte order +
    /// row/column orientation all at once (a flipped or byte-swapped file
    /// cannot pass all three).
    #[test]
    fn shipped_earth_heightmap_has_real_features() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("planets")
            .join("earth_heightmap.bin");
        let hm = PlanetHeightmap::load(&path).expect("shipped earth heightmap loads");
        assert_eq!(hm.width(), 3600, "expected 0.1-degree grid");
        assert_eq!(hm.height(), 1800, "expected 0.1-degree grid");
        // The fixed -11000..+6500 m quantization window (see build script):
        // sea level must land at 11000/17500 = ~0.62857 of the domain.
        assert!((hm.sea_level_normalized() - 11000.0 / 17500.0).abs() < 1e-3);

        // Everest region (28.0 N, 86.9 E): 0.1-degree cell averaging pulls
        // the 8849 m summit down, but the Himalayan massif stays high.
        let everest = hm.sample_meters_latlon(28.0, 86.9);
        assert!(everest > 5000.0, "Everest region too low: {everest} m");

        // Challenger Deep, Mariana Trench (11.37 N, 142.59 E).
        let mariana = hm.sample_meters_latlon(11.37, 142.59);
        assert!(mariana < -7000.0, "Mariana region too shallow: {mariana} m");

        // Mid-Pacific abyssal plain (5 N, 135 W): open deep ocean.
        let pacific = hm.sample_meters_latlon(5.0, -135.0);
        assert!(pacific < -3000.0, "mid-Pacific too shallow: {pacific} m");

        // Sahara (23 N, 10 E): dry land well above sea level, far below
        // mountain heights -- catches an inverted elevation encoding.
        let sahara = hm.sample_meters_latlon(23.0, 10.0);
        assert!(
            (100.0..3000.0).contains(&sahara),
            "Sahara implausible: {sahara} m"
        );
    }
}
