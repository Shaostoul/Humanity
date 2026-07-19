//! Streamed high-detail terrain tiles (the downloadable terrain tier).
//!
//! The shipped base grid (planet_heightmap, 0.05 deg / ~5.5 km cells) is the
//! floor every install has. This module streams OPTIONAL 15x15-degree tiles
//! at 15 arc-seconds (~460 m cells, from NOAA ETOPO 2022; built locally by
//! examples/build_earth_tiles.rs, distributed later like the ultra star
//! catalog). Deep terrain patches sample a resident tile instead of the base
//! grid, so mountains get their real shapes (Fuji's cone) and coastlines
//! stop staircasing - see docs/design/terrain-detail.md.
//!
//! Residency: the caller asks `ensure_region(lat, lon)` every frame for the
//! camera's location; the center tile + its 8 neighbors load on background
//! threads (~26 MB each, one-time ~30 ms disk read apiece) and arrive via a
//! channel. `poll()` reports arrivals so the caller can invalidate built
//! patches. A small LRU keeps at most `MAX_RESIDENT` tiles (~400 MB).
//!
//! Sampling: Catmull-Rom bicubic over the GLOBAL virtual 15-arc-sec grid
//! (86400x43200, cell-centered, same registration family as the base grid).
//! The 4x4 stencil may span tile borders; taps resolve into whichever tile
//! holds them, and if ANY tap's tile is not resident the sample returns None
//! so the caller falls back to the base grid for the whole sample -
//! continuity over partial detail. Missing tile FILES (not downloaded) are
//! remembered as absent for the session and never re-requested.
//!
//! Pure std + the shared heightmap header format; testable headless.

use super::planet_heightmap::HEIGHTMAP_MAGIC;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Tile grid constants: 15x15 degrees at 15 arc-seconds.
pub const TILE_DEG: i32 = 15;
pub const TILE_PX: usize = 3600;
/// Global virtual grid the tiles collectively form.
pub const GLOBAL_W: i64 = 24 * TILE_PX as i64; // 86400
pub const GLOBAL_H: i64 = 12 * TILE_PX as i64; // 43200

/// Most tiles kept resident (center + 8 neighbors is 9; a little headroom
/// avoids thrash when crossing a border diagonally). ~26 MB per tile.
pub const MAX_RESIDENT: usize = 14;

/// One resident tile: quantized samples + its quantization window.
struct Tile {
    samples: Vec<u16>,
    min_m: f32,
    max_m: f32,
}

/// Tile key: (north latitude of the tile's top edge, west longitude of its
/// left edge), both multiples of 15 in degrees. Matches the builder's names.
pub type TileKey = (i32, i32);

/// Key of the tile containing a (lat, lon) point.
pub fn key_for(lat_deg: f32, lon_deg: f32) -> TileKey {
    // Tile rows span (top-15, top]; top values 90, 75, ..., -60.
    let mut top = (lat_deg / TILE_DEG as f32).ceil() as i32 * TILE_DEG;
    top = top.clamp(-90 + TILE_DEG, 90);
    // Columns span [west, west+15); west values -180..165, wrapping.
    let mut west = (lon_deg / TILE_DEG as f32).floor() as i32 * TILE_DEG;
    if west < -180 {
        west += 360;
    }
    if west >= 180 {
        west -= 360;
    }
    (top, west)
}

/// Builder-side file name for a key ("N45W135.bin").
pub fn file_name(key: TileKey) -> String {
    let (top, west) = key;
    let lat = if top > 0 {
        format!("N{:02}", top)
    } else if top == 0 {
        "N00".to_string()
    } else {
        format!("S{:02}", -top)
    };
    let lon = if west < 0 {
        format!("W{:03}", -west)
    } else {
        format!("E{:03}", west)
    };
    format!("{lat}{lon}.bin")
}

pub struct TerrainTiles {
    dir: PathBuf,
    resident: HashMap<TileKey, Tile>,
    pending: HashSet<TileKey>,
    /// Tile files confirmed missing on disk this session - never re-tried.
    absent: HashSet<TileKey>,
    /// Most recent region center, for LRU distance eviction.
    last_center: TileKey,
    tx: Sender<(TileKey, Option<Tile>)>,
    rx: Receiver<(TileKey, Option<Tile>)>,
}

impl TerrainTiles {
    /// `dir` is the tile directory (data/planets/earth_tiles). The set is
    /// usable even if the directory is empty/absent - every sample simply
    /// returns None and the base grid carries the terrain (no download, no
    /// detail tier, nothing breaks).
    pub fn new(dir: PathBuf) -> Self {
        let (tx, rx) = channel();
        Self {
            dir,
            resident: HashMap::new(),
            pending: HashSet::new(),
            absent: HashSet::new(),
            last_center: (90, -180),
            tx,
            rx,
        }
    }

    /// Request residency for the tile under (lat, lon) plus its 8 neighbors.
    /// Cheap when everything is already resident/pending/absent.
    pub fn ensure_region(&mut self, lat_deg: f32, lon_deg: f32) {
        let center = key_for(lat_deg, lon_deg);
        self.last_center = center;
        let (ctop, cwest) = center;
        for dlat in [-TILE_DEG, 0, TILE_DEG] {
            for dlon in [-TILE_DEG, 0, TILE_DEG] {
                let top = (ctop + dlat).clamp(-90 + TILE_DEG, 90);
                let mut west = cwest + dlon;
                if west < -180 {
                    west += 360;
                }
                if west >= 180 {
                    west -= 360;
                }
                let key = (top, west);
                if self.resident.contains_key(&key)
                    || self.pending.contains(&key)
                    || self.absent.contains(&key)
                {
                    continue;
                }
                self.pending.insert(key);
                let path = self.dir.join(file_name(key));
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let tile = load_tile(&path);
                    let _ = tx.send((key, tile));
                });
            }
        }
    }

    /// Drain arrivals. Returns true when at least one NEW tile became
    /// resident (the caller should invalidate patches built from base data).
    pub fn poll(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((key, tile)) = self.rx.try_recv() {
            self.pending.remove(&key);
            match tile {
                Some(t) => {
                    log::info!(
                        "[Tiles] terrain tile {} resident ({} tiles held)",
                        file_name(key),
                        self.resident.len() + 1
                    );
                    self.resident.insert(key, t);
                    arrived = true;
                }
                None => {
                    self.absent.insert(key);
                }
            }
        }
        if arrived {
            self.evict_far();
        }
        arrived
    }

    /// Keep at most MAX_RESIDENT tiles: drop the ones farthest from the
    /// current region center (simple grid distance with lon wrap).
    fn evict_far(&mut self) {
        while self.resident.len() > MAX_RESIDENT {
            let (ctop, cwest) = self.last_center;
            let far = self
                .resident
                .keys()
                .max_by_key(|(top, west)| {
                    let dlat = (top - ctop).abs();
                    let mut dlon = (west - cwest).abs();
                    if dlon > 180 {
                        dlon = 360 - dlon;
                    }
                    dlat + dlon
                })
                .copied();
            match far {
                Some(k) => {
                    self.resident.remove(&k);
                }
                None => break,
            }
        }
    }

    /// Any tiles on disk at all? (Cheap gate so integration points can skip
    /// per-sample work when the tier is not installed.)
    /// Resident tile count (diagnostics).
    pub fn resident_count(&self) -> usize {
        self.resident.len()
    }

    pub fn tier_installed(&self) -> bool {
        !self.resident.is_empty() || !self.pending.is_empty() || self.dir.is_dir()
    }

    /// Bicubic elevation in meters at (lat, lon), or None when any stencil
    /// tap's tile is not resident (caller falls back to the base grid).
    pub fn sample_meters_smooth(&self, lat_deg: f32, lon_deg: f32) -> Option<f32> {
        if self.resident.is_empty() {
            return None;
        }
        fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
            p1 + 0.5
                * t
                * (p2 - p0
                    + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3
                        + t * (3.0 * (p1 - p2) + p3 - p0)))
        }
        // Global virtual-grid fractional coordinates (cell-centered).
        let fx = (lon_deg + 180.0) / 360.0 * GLOBAL_W as f32 - 0.5;
        let fy = (90.0 - lat_deg) / 180.0 * GLOBAL_H as f32 - 0.5;
        let x0 = fx.floor() as i64;
        let y0 = fy.floor() as i64;
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let mut rows = [0.0f32; 4];
        for (i, row) in rows.iter_mut().enumerate() {
            let y = y0 + i as i64 - 1;
            let mut cols = [0.0f32; 4];
            for (j, col) in cols.iter_mut().enumerate() {
                let x = x0 + j as i64 - 1;
                *col = self.global_sample_m(x, y)?;
            }
            *row = catmull(cols[0], cols[1], cols[2], cols[3], tx);
        }
        Some(catmull(rows[0], rows[1], rows[2], rows[3], ty))
    }

    /// One global virtual-grid cell in meters, or None if its tile is not
    /// resident. x wraps, y clamps (poles), mirroring the base grid rules.
    fn global_sample_m(&self, x: i64, y: i64) -> Option<f32> {
        let x = x.rem_euclid(GLOBAL_W);
        let y = y.clamp(0, GLOBAL_H - 1);
        let tile_col = (x / TILE_PX as i64) as i32; // 0..23 west->east
        let tile_row = (y / TILE_PX as i64) as i32; // 0..11 north->south
        let key = (90 - tile_row * TILE_DEG, -180 + tile_col * TILE_DEG);
        let tile = self.resident.get(&key)?;
        let lx = (x % TILE_PX as i64) as usize;
        let ly = (y % TILE_PX as i64) as usize;
        let q = tile.samples[ly * TILE_PX + lx] as f32;
        Some(tile.min_m + (q / 65535.0) * (tile.max_m - tile.min_m))
    }
}

/// Read + validate one tile bin (HOSHGT1, 3600x3600).
fn load_tile(path: &std::path::Path) -> Option<Tile> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 23 || &bytes[0..7] != HEIGHTMAP_MAGIC {
        log::warn!("terrain tile {} malformed (magic/size)", path.display());
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let f32_at = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let (w, h) = (u32_at(7) as usize, u32_at(11) as usize);
    if (w, h) != (TILE_PX, TILE_PX) {
        log::warn!("terrain tile {} has unexpected dims {w}x{h}", path.display());
        return None;
    }
    let (min_m, max_m) = (f32_at(15), f32_at(19));
    let payload = &bytes[23..];
    if payload.len() != w * h * 2 || !(max_m > min_m) {
        log::warn!("terrain tile {} payload/range invalid", path.display());
        return None;
    }
    let samples = payload
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(Tile { samples, min_m, max_m })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_names_round_the_globe() {
        assert_eq!(key_for(35.36, 138.73), (45, 135)); // Fuji tile
        assert_eq!(file_name((45, 135)), "N45E135.bin");
        assert_eq!(key_for(21.3, -157.8), (30, -165)); // Oahu
        assert_eq!(file_name((30, -165)), "N30W165.bin");
        assert_eq!(key_for(-33.9, 18.4), (-30, 15)); // Cape Town
        assert_eq!(file_name((-30, 15)), "S30E015.bin");
        // Equator: lat 0 belongs to the N00 tile (top edge inclusive; the
        // global-grid row math resolves lat 0 to tile row 6 = top 0).
        assert_eq!(key_for(0.0, 0.0), (0, 0));
        assert_eq!(file_name(key_for(-7.0, 110.0)), "N00E105.bin"); // Java
        // Antimeridian wrap: lon 179.9 is in the E165 tile, lon -179.9 in W180.
        assert_eq!(key_for(10.0, 179.9), (15, 165));
        assert_eq!(key_for(10.0, -179.9), (15, -180));
    }

    #[test]
    fn synthetic_tile_samples_through_the_global_grid() {
        // Build one synthetic tile (N45E135, the Fuji tile) whose value is a
        // known constant, drop it in a temp dir, and confirm region loading +
        // sampling inside the tile returns it while outside returns None.
        let dir = std::env::temp_dir().join("hos_tile_test");
        let _ = std::fs::create_dir_all(&dir);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HEIGHTMAP_MAGIC);
        bytes.extend_from_slice(&(TILE_PX as u32).to_le_bytes());
        bytes.extend_from_slice(&(TILE_PX as u32).to_le_bytes());
        bytes.extend_from_slice(&(-1000.0f32).to_le_bytes());
        bytes.extend_from_slice(&(1000.0f32).to_le_bytes());
        // All samples at quantum 49151 -> -1000 + 0.75*2000 = 500 m.
        let q: u16 = 49151;
        let sample_bytes = q.to_le_bytes();
        let mut payload = vec![0u8; TILE_PX * TILE_PX * 2];
        for c in payload.chunks_exact_mut(2) {
            c.copy_from_slice(&sample_bytes);
        }
        bytes.extend_from_slice(&payload);
        std::fs::write(dir.join("N45E135.bin"), &bytes).expect("write synthetic tile");

        let mut tiles = TerrainTiles::new(dir.clone());
        tiles.ensure_region(35.36, 138.73);
        // Wait for the loader thread (bounded).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !tiles.poll() {
            assert!(std::time::Instant::now() < deadline, "tile never arrived");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let m = tiles
            .sample_meters_smooth(38.0, 140.0)
            .expect("interior sample resolves");
        assert!((m - 500.0).abs() < 1.0, "expected ~500 m, got {m}");
        // A point whose stencil needs a neighbor tile that is absent -> None.
        assert!(tiles.sample_meters_smooth(35.0, 100.0).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
