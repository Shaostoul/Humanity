//! Ocean mask: which cells of a planet's lat/lon grid are CONNECTED ocean.
//!
//! Derived from the heightmap by `scripts/build-ocean-mask.js` (flood fill
//! from a mid-Pacific seed across cells <= 0 m), so land below sea level
//! (Death Valley, the Dead Sea shore, the Caspian depression) is correctly
//! NOT ocean - the reason a naive water sphere at sea level is wrong. See
//! docs/design/ocean.md; this is Stage 0 groundwork for real water.
//!
//! File format ("HOSOCM1", little-endian):
//!   bytes 0..7   magic b"HOSOCM1"
//!   bytes 7..11  u32 width   (longitude cells; wraps)
//!   bytes 11..15 u32 height  (latitude cells; clamps)
//!   bytes 15..   ceil(w*h/8) bytes, bit-packed row-major, LSB-first
//!                (bit set = ocean), row 0 = northernmost row.
//!
//! Registration matches the heightmap exactly (cell-centered equirect, row 0
//! north, col 0 at lon -180): the two files describe the same grid, and the
//! build script errors out rather than write a mismatched mask. Pure std +
//! shared tap math, testable headless in every feature set.

use super::planet_heightmap::dir_to_latlon_deg;
use glam::Vec3;
use std::path::Path;

/// Magic bytes at the start of every HumanityOS ocean mask file.
pub const OCEAN_MASK_MAGIC: &[u8; 7] = b"HOSOCM1";

const HEADER_LEN: usize = 15;

/// A loaded ocean mask. Earth's shipped mask is 3600x1800 -> ~0.77 MB.
pub struct OceanMask {
    width: u32,
    height: u32,
    /// Bit-packed cells, row-major, LSB-first within each byte.
    bits: Vec<u8>,
}

impl OceanMask {
    /// Parse a mask from raw file bytes (same error convention as the
    /// heightmap loader: the caller logs + treats the planet as maskless).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN {
            return Err(format!("ocean mask too small: {} bytes", bytes.len()));
        }
        if &bytes[0..7] != OCEAN_MASK_MAGIC {
            return Err("bad ocean mask magic (expected HOSOCM1)".to_string());
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let width = u32_at(7);
        let height = u32_at(11);
        if width == 0 || height == 0 {
            return Err(format!("ocean mask has degenerate dimensions {width}x{height}"));
        }
        let expected = (width as usize * height as usize).div_ceil(8);
        let payload = &bytes[HEADER_LEN..];
        if payload.len() != expected {
            return Err(format!(
                "ocean mask payload is {} bytes, expected {expected} for {width}x{height}",
                payload.len()
            ));
        }
        Ok(Self { width, height, bits: payload.to_vec() })
    }

    /// Load a mask file from disk.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Is the grid cell containing (lat, lon) connected ocean? Nearest-cell
    /// lookup (a mask has no meaningful interpolation); longitude wraps,
    /// latitude clamps, mirroring the heightmap's edge rules.
    pub fn is_ocean_latlon(&self, lat_deg: f32, lon_deg: f32) -> bool {
        let w = self.width as i64;
        let h = self.height as i64;
        let x = (((lon_deg + 180.0) / 360.0 * self.width as f32).floor() as i64).rem_euclid(w);
        let y = (((90.0 - lat_deg) / 180.0 * self.height as f32).floor() as i64).clamp(0, h - 1);
        let i = (y * w + x) as usize;
        (self.bits[i >> 3] >> (i & 7)) & 1 == 1
    }

    /// Is the cell under a unit-sphere direction connected ocean? Routes
    /// through the SAME handedness as the heightmap/albedo samplers so the
    /// mask can never mirror-flip against the terrain.
    pub fn is_ocean(&self, unit: Vec3) -> bool {
        let (lat, lon) = dir_to_latlon_deg(unit);
        self.is_ocean_latlon(lat, lon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny synthetic mask: 8x4 grid, only the northwest cell wet.
    fn tiny() -> OceanMask {
        let (w, h) = (8u32, 4u32);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OCEAN_MASK_MAGIC);
        bytes.extend_from_slice(&w.to_le_bytes());
        bytes.extend_from_slice(&h.to_le_bytes());
        let mut bits = vec![0u8; 4]; // 32 cells -> 4 bytes
        bits[0] |= 1; // cell (0,0): row 0 (north), col 0 (lon -180 edge)
        bytes.extend_from_slice(&bits);
        OceanMask::from_bytes(&bytes).expect("synthetic mask parses")
    }

    #[test]
    fn bit_lookup_and_edge_rules() {
        let m = tiny();
        // Cell (0,0) spans lat 45..90, lon -180..-135; its interior is wet.
        assert!(m.is_ocean_latlon(60.0, -170.0));
        // Neighbor cell east of it is dry.
        assert!(!m.is_ocean_latlon(60.0, -130.0));
        // Longitude wraps: lon 190 == lon -170 -> wet.
        assert!(m.is_ocean_latlon(60.0, 190.0));
        // Latitude clamps above the top row -> still row 0 -> wet.
        assert!(m.is_ocean_latlon(89.9, -170.0));
    }

    #[test]
    fn rejects_malformed_files() {
        assert!(OceanMask::from_bytes(b"short").is_err());
        let mut bad = Vec::new();
        bad.extend_from_slice(b"NOTMAG1");
        bad.extend_from_slice(&8u32.to_le_bytes());
        bad.extend_from_slice(&4u32.to_le_bytes());
        bad.extend_from_slice(&[0u8; 4]);
        assert!(OceanMask::from_bytes(&bad).is_err());
        // Truncated payload.
        let mut trunc = Vec::new();
        trunc.extend_from_slice(OCEAN_MASK_MAGIC);
        trunc.extend_from_slice(&8u32.to_le_bytes());
        trunc.extend_from_slice(&4u32.to_le_bytes());
        trunc.extend_from_slice(&[0u8; 2]);
        assert!(OceanMask::from_bytes(&trunc).is_err());
    }

    #[test]
    fn shipped_earth_mask_keeps_below_sea_level_land_dry() {
        // The whole point of the mask (docs/design/ocean.md): Death Valley
        // sits below sea level but must NOT be ocean, while open water is.
        // Skips silently if the shipped file is absent (fresh checkout
        // before the build script has run).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/planets/earth_ocean_mask.bin");
        let Ok(mask) = OceanMask::load(&path) else { return };
        assert!(mask.is_ocean_latlon(30.0, -40.0), "mid-Atlantic must be ocean");
        assert!(mask.is_ocean_latlon(0.0, -160.0), "mid-Pacific must be ocean");
        assert!(
            !mask.is_ocean_latlon(36.23, -116.82),
            "Death Valley is below sea level but must stay dry"
        );
        assert!(!mask.is_ocean_latlon(27.98, 86.92), "Everest is not ocean");
        // The Caspian is water in reality but endorheic: correctly NOT
        // connected ocean under this mask (a lake class comes later).
        assert!(!mask.is_ocean_latlon(41.0, 50.5), "Caspian is not CONNECTED ocean");
    }
}
