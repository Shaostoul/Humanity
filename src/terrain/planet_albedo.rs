//! Real surface-color (albedo) grids for sky planets (Earth ships with one).
//!
//! A planet def (data/planets/<id>.ron) can point at a lat/lon RGB grid via
//! its `albedo` field. When present, `terrain::planet_surface` colors faces
//! from bilinear samples of this grid instead of the elevation-band
//! classifier, so Earth shows its REAL surface colors (Sahara sand, Amazon
//! green, deep-ocean bathymetry shading) the way orbital photos do. Earth's
//! grid is downsampled from NASA's Blue Marble Next Generation August
//! composite (public domain; see scripts/build-earth-albedo.js for the full
//! provenance + the exact file format written).
//!
//! File format ("HOSALB1", all little-endian):
//!   bytes 0..7   magic b"HOSALB1"
//!   bytes 7..11  u32 width   (longitude samples; wraps around)
//!   bytes 11..15 u32 height  (latitude samples; clamps at the poles)
//!   bytes 15..   width*height*3 sRGB bytes, row-major RGB, row 0 = the
//!                northernmost row, column 0 = the westernmost column.
//!
//! Grid registration is CELL-CENTERED and IDENTICAL to the elevation grid
//! (`planet_heightmap`); both samplers share the same lat/lon math
//! (`dir_to_latlon_deg` + `latlon_bilinear_tap` live in planet_heightmap),
//! so color can never mirror-flip or shift against the real elevation data.
//! A shared-code test below locks the agreement.
//!
//! Color space: the FILE stores sRGB bytes (maximum precision where eyes
//! see it, and directly comparable to the source imagery), but samples are
//! returned LINEAR. The whole material pipeline treats colors as linear
//! albedo (the hand-authored RON palette values are linear too -- the sRGB
//! encode happens once, on store to the sRGB render target), so the decode
//! belongs here, at the boundary. Bilinear interpolation happens AFTER the
//! per-texel linearization, which is the physically correct order.
//!
//! Everything here is pure math + std IO, no GPU and no native-only deps,
//! so it compiles in every feature set (terrain/ is an ungated module) and
//! is fully unit-testable headless.

use glam::Vec3;
use std::path::Path;
use std::sync::OnceLock;

use super::planet_heightmap::{dir_to_latlon_deg, latlon_bilinear_tap};

/// Magic bytes at the start of every HumanityOS planet albedo file.
pub const ALBEDO_MAGIC: &[u8; 7] = b"HOSALB1";

/// Header size in bytes: magic(7) + width(4) + height(4).
const HEADER_LEN: usize = 15;

/// sRGB byte -> linear f32, the exact IEC 61966-2-1 transfer curve.
/// Public so tests (and the build script's documented mirror) can verify
/// endpoints; the sampler goes through the LUT below.
pub fn srgb_byte_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// 256-entry decode LUT: the sampler touches 12 channel reads per bilinear
/// sample and mesh builds take thousands of samples, so skip the powf.
fn srgb_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = srgb_byte_to_linear(i as u8);
        }
        t
    })
}

/// A loaded planet surface-color grid. Earth's shipped grid is 4096x2048
/// (~0.088 degree cells) = ~25 MB resident, loaded once per session.
pub struct PlanetAlbedo {
    width: u32,
    height: u32,
    /// sRGB bytes, row-major RGB, north-to-south / west-to-east.
    rgb: Vec<u8>,
}

impl PlanetAlbedo {
    /// Parse an albedo grid from raw file bytes. Errors are strings because
    /// the only caller path is "log a warning + fall back to the elevation
    /// band classifier".
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN {
            return Err(format!("albedo file too small: {} bytes", bytes.len()));
        }
        if &bytes[0..7] != ALBEDO_MAGIC {
            return Err("bad albedo magic (expected HOSALB1)".to_string());
        }
        let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let width = u32_at(7);
        let height = u32_at(11);
        if width == 0 || height == 0 {
            return Err(format!("albedo grid has degenerate dimensions {width}x{height}"));
        }
        let expected = width as usize * height as usize * 3;
        let payload = &bytes[HEADER_LEN..];
        if payload.len() != expected {
            return Err(format!(
                "albedo payload is {} bytes, expected {expected} for {width}x{height} RGB",
                payload.len()
            ));
        }
        Ok(Self { width, height, rgb: payload.to_vec() })
    }

    /// Load an albedo file from disk.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }

    /// LINEAR RGB of one grid cell, with longitude WRAP and latitude CLAMP
    /// applied to the integer coordinates (same edge policy as the
    /// elevation grid: wrap across the antimeridian, hold the row value
    /// above the polar cell centers).
    fn grid_linear(&self, x: i64, y: i64) -> [f32; 3] {
        let w = self.width as i64;
        let h = self.height as i64;
        // rem_euclid wraps negatives correctly (-1 -> w-1), unlike %.
        let xi = x.rem_euclid(w) as usize;
        let yi = y.clamp(0, h - 1) as usize;
        let o = (yi * self.width as usize + xi) * 3;
        let lut = srgb_lut();
        [lut[self.rgb[o] as usize], lut[self.rgb[o + 1] as usize], lut[self.rgb[o + 2] as usize]]
    }

    /// Bilinear LINEAR RGB at a geographic coordinate.
    /// lat in degrees (+north), lon in degrees (+east); any lon accepted.
    pub fn sample_linear_latlon(&self, lat_deg: f32, lon_deg: f32) -> [f32; 3] {
        // Shared tap derivation with the elevation sampler (see module doc).
        let t = latlon_bilinear_tap(lat_deg, lon_deg, self.width, self.height);
        let c00 = self.grid_linear(t.x0, t.y0);
        let c10 = self.grid_linear(t.x0 + 1, t.y0);
        let c01 = self.grid_linear(t.x0, t.y0 + 1);
        let c11 = self.grid_linear(t.x0 + 1, t.y0 + 1);
        let mut out = [0.0f32; 3];
        for i in 0..3 {
            let top = c00[i] + (c10[i] - c00[i]) * t.tx;
            let bot = c01[i] + (c11[i] - c01[i]) * t.tx;
            out[i] = top + (bot - top) * t.ty;
        }
        out
    }

    /// Bilinear LINEAR RGB at a unit-sphere direction, through the shared
    /// `dir_to_latlon_deg` handedness (identical to the elevation sampler,
    /// so color and terrain always align).
    pub fn sample_linear(&self, unit: Vec3) -> [f32; 3] {
        let (lat, lon) = dir_to_latlon_deg(unit);
        self.sample_linear_latlon(lat, lon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::planet_heightmap::{quantize_meters, PlanetHeightmap, HEIGHTMAP_MAGIC};

    /// Build an in-memory albedo grid from sRGB byte triples (row-major,
    /// north-first) via the same byte format the JS script writes.
    fn synth(width: u32, height: u32, texels: &[[u8; 3]]) -> PlanetAlbedo {
        assert_eq!(texels.len(), (width * height) as usize);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ALBEDO_MAGIC);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        for t in texels {
            bytes.extend_from_slice(t);
        }
        PlanetAlbedo::from_bytes(&bytes).expect("synthetic albedo parses")
    }

    /// 4 wide x 2 tall test grid mirroring planet_heightmap's grid_2x4:
    /// cell centers at lats +45/-45 and lons -135, -45, +45, +135. Red
    /// encodes the column (0, 60, 120, 180), green the row (0 north, 200
    /// south), blue constant.
    fn grid_2x4() -> PlanetAlbedo {
        synth(
            4,
            2,
            &[
                [0, 0, 40], [60, 0, 40], [120, 0, 40], [180, 0, 40],
                [0, 200, 40], [60, 200, 40], [120, 200, 40], [180, 200, 40],
            ],
        )
    }

    fn lin(b: u8) -> f32 {
        srgb_byte_to_linear(b)
    }

    #[test]
    fn srgb_decode_endpoints_and_monotonic() {
        assert_eq!(srgb_byte_to_linear(0), 0.0);
        assert!((srgb_byte_to_linear(255) - 1.0).abs() < 1e-6);
        // Mid-gray: sRGB 128 is ~0.2158 linear (the classic gamma check).
        assert!((srgb_byte_to_linear(128) - 0.2158).abs() < 0.002);
        for i in 1..256 {
            assert!(srgb_byte_to_linear(i as u8) > srgb_byte_to_linear((i - 1) as u8));
        }
    }

    #[test]
    fn exact_cell_centers_return_stored_values() {
        let a = grid_2x4();
        let c = a.sample_linear_latlon(45.0, -135.0);
        assert!((c[0] - lin(0)).abs() < 1e-6);
        assert!((c[1] - lin(0)).abs() < 1e-6);
        assert!((c[2] - lin(40)).abs() < 1e-6);
        let c = a.sample_linear_latlon(-45.0, 45.0);
        assert!((c[0] - lin(120)).abs() < 1e-6);
        assert!((c[1] - lin(200)).abs() < 1e-6);
    }

    #[test]
    fn bilinear_midpoints_average_neighbors_in_linear_space() {
        let a = grid_2x4();
        // Halfway between cols 0 and 1 on the north row: linear average of
        // lin(0) and linear(60) -- NOT the sRGB byte average (30).
        let c = a.sample_linear_latlon(45.0, -90.0);
        assert!((c[0] - (lin(0) + lin(60)) / 2.0).abs() < 1e-6);
        // Halfway between rows at col 1: green (0 + 200)/2 linear.
        let c = a.sample_linear_latlon(0.0, -45.0);
        assert!((c[1] - (lin(0) + lin(200)) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn longitude_wraps_latitude_clamps() {
        let a = grid_2x4();
        // lon 180 is halfway between col 3 and col 0 (wrapped); +180 and
        // -180 must agree.
        let hi = a.sample_linear_latlon(45.0, 180.0);
        let lo = a.sample_linear_latlon(45.0, -180.0);
        assert!((hi[0] - lo[0]).abs() < 1e-6);
        assert!((hi[0] - (lin(180) + lin(0)) / 2.0).abs() < 1e-6);
        // Above the northernmost cell centers the top row holds.
        let n = a.sample_linear_latlon(90.0, -135.0);
        assert!((n[1] - lin(0)).abs() < 1e-6);
        let s = a.sample_linear_latlon(-90.0, -135.0);
        assert!((s[1] - lin(200)).abs() < 1e-6);
        // No panic at absurd inputs.
        let _ = a.sample_linear_latlon(9999.0, -9999.0);
    }

    #[test]
    fn direction_sampling_matches_latlon_convention() {
        let a = grid_2x4();
        // +Y is the north pole -> clamps to the north row (green 0).
        let north = a.sample_linear(Vec3::new(0.0, 1.0, 0.0));
        assert!((north[1] - lin(0)).abs() < 1e-6);
        // -Z on the equator must be lon +90 EAST (handedness: east is -z).
        let mz = a.sample_linear(Vec3::new(0.0, 0.0, -1.0));
        let ll = a.sample_linear_latlon(0.0, 90.0);
        for i in 0..3 {
            assert!((mz[i] - ll[i]).abs() < 1e-6);
        }
    }

    /// THE shared-code alignment test: build an elevation grid and an
    /// albedo grid encoding the SAME east-ramp pattern, then check that at
    /// a spread of directions the two samplers land on proportional values.
    /// Because both route through `dir_to_latlon_deg` +
    /// `latlon_bilinear_tap`, any future handedness or registration edit
    /// that touches only one of them fails this immediately.
    #[test]
    fn albedo_and_heightmap_samplers_share_handedness() {
        let (w, h) = (8u32, 4u32);
        // Pattern: value rises linearly with column (i.e. with longitude).
        let mut hm_bytes = Vec::new();
        hm_bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        hm_bytes.extend_from_slice(&w.to_le_bytes());
        hm_bytes.extend_from_slice(&h.to_le_bytes());
        hm_bytes.extend_from_slice(&0.0f32.to_le_bytes());
        hm_bytes.extend_from_slice(&700.0f32.to_le_bytes());
        let mut al_texels = Vec::new();
        for _row in 0..h {
            for col in 0..w {
                let m = col as f32 * 100.0; // 0..700 m
                hm_bytes.extend_from_slice(&quantize_meters(m, 0.0, 700.0).to_le_bytes());
                al_texels.push([(col * 30) as u8, 0, 0]); // red 0..210
            }
        }
        let hm = PlanetHeightmap::from_bytes(&hm_bytes).expect("hm parses");
        let al = synth(w, h, &al_texels);

        // Directions spread around the equator + midlatitudes. Stay inside
        // the outermost cell centers (-157.5 / +157.5 for an 8-col grid):
        // beyond them the bilinear blends across the antimeridian, where
        // the RAMP ITSELF wraps 210 -> 0 and the col+tx reconstruction
        // below stops being linear.
        for lon in [-150.0f32, -90.0, -30.0, 0.0, 45.0, 120.0, 150.0] {
            for lat in [-40.0f32, 0.0, 40.0] {
                let (sin_lat, cos_lat) = lat.to_radians().sin_cos();
                let (sin_lon, cos_lon) = lon.to_radians().sin_cos();
                // Inverse of dir_to_latlon_deg: x = cos(lat)cos(lon),
                // y = sin(lat), z = -cos(lat)sin(lon).
                let dir = Vec3::new(cos_lat * cos_lon, sin_lat, -cos_lat * sin_lon);
                let m = hm.sample_meters(dir);
                let r = al.sample_linear(dir)[0];
                // Reconstruct the column coordinate each sampler saw. The
                // elevation ramp is exactly linear in meters; the albedo
                // ramp is linear in sRGB BYTES, so invert per-tap: both
                // sampled the same two columns with the same tx, and since
                // the meters ramp gives us col+tx directly, re-encode it
                // through the albedo's own math and compare.
                let col_plus_tx = m / 100.0;
                let c0 = col_plus_tx.floor();
                let tx = col_plus_tx - c0;
                let b0 = ((c0 as i64).rem_euclid(w as i64) * 30) as u8;
                let b1 = (((c0 as i64) + 1).rem_euclid(w as i64) * 30) as u8;
                let expect_r = lin(b0) + (lin(b1) - lin(b0)) * tx;
                assert!(
                    (r - expect_r).abs() < 2e-3,
                    "lat {lat} lon {lon}: albedo red {r} disagrees with heightmap-derived {expect_r}"
                );
            }
        }
    }

    #[test]
    fn rejects_malformed_files() {
        assert!(PlanetAlbedo::from_bytes(b"short").is_err());
        // Wrong magic.
        let mut bad = Vec::from(*b"NOTALB1");
        bad.extend_from_slice(&[0u8; 8]);
        assert!(PlanetAlbedo::from_bytes(&bad).is_err());
        // Right magic, payload size mismatch (claims 2x2 but carries 1 texel).
        let mut trunc = Vec::from(*ALBEDO_MAGIC);
        trunc.extend_from_slice(&2u32.to_le_bytes());
        trunc.extend_from_slice(&2u32.to_le_bytes());
        trunc.extend_from_slice(&[10u8, 20, 30]);
        assert!(PlanetAlbedo::from_bytes(&trunc).is_err());
        // Degenerate dimensions.
        let mut degen = Vec::from(*ALBEDO_MAGIC);
        degen.extend_from_slice(&0u32.to_le_bytes());
        degen.extend_from_slice(&2u32.to_le_bytes());
        assert!(PlanetAlbedo::from_bytes(&degen).is_err());
    }

    /// Anchor test over the COMMITTED Earth grid: reads the shipped
    /// data/planets/earth_albedo.bin directly and checks real-world surface
    /// colors are where they belong. Locks provenance + byte order +
    /// row/column orientation + channel order all at once (a flipped,
    /// byte-shifted, or BGR file cannot pass all four). Values are LINEAR
    /// (the sampler's output domain).
    #[test]
    fn shipped_earth_albedo_has_real_colors() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("planets")
            .join("earth_albedo.bin");
        let al = PlanetAlbedo::load(&path).expect("shipped earth albedo loads");
        assert_eq!(al.width(), 4096, "expected the 4096x2048 build");
        assert_eq!(al.height(), 2048, "expected the 4096x2048 build");

        // Sahara (23 N, 10 E): sandy -- red > green > blue, and bright.
        let sahara = al.sample_linear_latlon(23.0, 10.0);
        assert!(
            sahara[0] > sahara[1] && sahara[1] > sahara[2] && sahara[0] > 0.3,
            "Sahara not sandy: {sahara:?}"
        );

        // Mid-Atlantic (10 N, 30 W): deep ocean -- blue dominant, dark.
        let atlantic = al.sample_linear_latlon(10.0, -30.0);
        assert!(
            atlantic[2] > atlantic[0] && atlantic[2] < 0.1,
            "mid-Atlantic not dark blue: {atlantic:?}"
        );

        // Amazon (4 S, 63 W): rainforest -- green dominant.
        let amazon = al.sample_linear_latlon(-4.0, -63.0);
        assert!(
            amazon[1] > amazon[0] && amazon[1] > amazon[2],
            "Amazon not green: {amazon:?}"
        );

        // Antarctica (85 S, 0 E): ice -- all channels bright.
        let antarctica = al.sample_linear_latlon(-85.0, 0.0);
        assert!(
            antarctica.iter().all(|&c| c > 0.5),
            "Antarctica not bright ice: {antarctica:?}"
        );
    }
}
