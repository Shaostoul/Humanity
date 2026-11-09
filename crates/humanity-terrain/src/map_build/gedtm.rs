//! GEDTM30 v1.2's land heights, read from the window fetched onto the operator's data drive
//! (whole-Earth maps rung 2; `E:\HumanityOS\maps-src\gedtm30-v1.2-washington`, 112 of the global
//! file's own 2048-pixel tiles kept byte for byte, `index.json` describing them).
//!
//! THE TRAP (index.json's `decode` and `scaleNote`, measured 2026-11-09): the file's GDAL metadata
//! says SCALE 0.1, but the stored float32 values are already METRES. Mount Rainier decodes to
//! 4,412.7 and Mount Adams to 3,737.2 (summits 4,392 m and 3,743 m); applying the scale would make
//! Rainier 441 m. So this reader ignores the tag for this file, and
//! `gedtm30_reads_metres_not_decimetres` pins it on those two summits.
//!
//! A tile decodes as: zlib inflate, 2048 x 2048 little-endian 32-bit words, per row
//! `word[x] += word[x - 1]` (TIFF predictor 2 on 32-bit words, wrapping), reinterpreted as
//! float32 metres above EGM2008. The no-data value (3.4028e38) marks open ocean. Tile (row, col)
//! starts at longitude `origin_lon + col * 2048 * pixel_deg` and latitude
//! `origin_lat - row * 2048 * pixel_deg`, origin being the grid's outer corner (pixel centres fall
//! on whole arc-seconds).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What one pixel of the window holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Px {
    /// Metres above EGM2008.
    Metres(f32),
    /// The no-data value: open ocean.
    Ocean,
    /// Outside the fetched window (no tile file): no source here.
    Missing,
}

/// The fetched GEDTM30 window and its decoded tiles.
pub struct Gedtm {
    dir: PathBuf,
    pub origin_lon: f64,
    pub origin_lat: f64,
    pub pixel_deg: f64,
    pub tile: u32,
    pub nodata: f32,
    /// The window's bounds in degrees (south, north, west, east), from index.json.
    pub window: (f64, f64, f64, f64),
    tiles: HashMap<(u32, u32), Option<Vec<f32>>>,
}

/// The number after `"key":` in a JSON text (index.json's few scalar fields; no JSON crate).
fn json_num(text: &str, key: &str) -> Result<f64, String> {
    let at = text.find(&format!("\"{key}\"")).ok_or_else(|| format!("index.json has no {key}"))?;
    let rest = &text[at + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':').ok_or_else(|| format!("index.json {key}: no colon"))?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c))).unwrap_or(rest.len());
    rest[..end].parse::<f64>().map_err(|e| format!("index.json {key}: {e}"))
}

/// Decode one stored tile of `n` x `n` pixels to metres (no-data kept as its raw value).
pub fn decode_tile(z: &[u8], n: usize) -> Result<Vec<f32>, String> {
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(z, n * n * 4).map_err(|e| format!("GEDTM30 tile deflate: {e:?}"))?;
    if raw.len() != n * n * 4 {
        return Err(format!("GEDTM30 tile inflated to {} bytes, not {}", raw.len(), n * n * 4));
    }
    let mut out = Vec::with_capacity(n * n);
    for row in raw.chunks_exact(n * 4) {
        let mut acc = 0u32;
        for w in row.chunks_exact(4) {
            acc = acc.wrapping_add(u32::from_le_bytes([w[0], w[1], w[2], w[3]]));
            // METRES as stored: the file's SCALE 0.1 tag does not apply (see the module note).
            out.push(f32::from_bits(acc));
        }
    }
    Ok(out)
}

impl Gedtm {
    /// Open the window folder (its `index.json` and `tiles/`).
    pub fn open(dir: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(dir.join("index.json")).map_err(|e| format!("{}: {e}", dir.join("index.json").display()))?;
        Ok(Gedtm {
            dir: dir.to_path_buf(),
            origin_lon: json_num(&text, "originLon")?,
            origin_lat: json_num(&text, "originLat")?,
            pixel_deg: json_num(&text, "pixelDeg")?,
            tile: json_num(&text, "tile")? as u32,
            nodata: json_num(&text, "nodata")? as f32,
            window: (json_num(&text, "south")?, json_num(&text, "north")?, json_num(&text, "west")?, json_num(&text, "east")?),
            tiles: HashMap::new(),
        })
    }

    /// The pixel at global column `gx`, row `gy`.
    pub fn pixel(&mut self, gx: i64, gy: i64) -> Result<Px, String> {
        if gx < 0 || gy < 0 {
            return Ok(Px::Missing);
        }
        let t = self.tile as i64;
        let key = ((gy / t) as u32, (gx / t) as u32);
        if !self.tiles.contains_key(&key) {
            let path = self.dir.join("tiles").join(format!("r{}_c{}.deflate", key.0, key.1));
            let tile = match std::fs::read(&path) {
                Ok(z) => Some(decode_tile(&z, self.tile as usize).map_err(|e| format!("{}: {e}", path.display()))?),
                Err(_) => None,
            };
            self.tiles.insert(key, tile);
        }
        Ok(match &self.tiles[&key] {
            None => Px::Missing,
            Some(px) => {
                let v = px[((gy % t) * t + gx % t) as usize];
                if v == self.nodata || !v.is_finite() || v.abs() > 1.0e5 {
                    Px::Ocean
                } else {
                    Px::Metres(v)
                }
            }
        })
    }

    /// Fractional pixel coordinates of (lat, lon), pixel centres at whole numbers.
    fn frac(&self, lat: f64, lon: f64) -> (f64, f64) {
        ((lon - self.origin_lon) / self.pixel_deg - 0.5, (self.origin_lat - lat) / self.pixel_deg - 0.5)
    }

    /// Bilinear land height at (lat, lon): metres, `Some(0.0)`-weighted ocean pixels (the water
    /// side of a coast, where only the shore band reads land), None outside the window.
    pub fn bilinear(&mut self, lat: f64, lon: f64) -> Result<Option<f64>, String> {
        let (fx, fy) = self.frac(lat, lon);
        let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let mut acc = 0.0;
        for (dx, dy, w) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            match self.pixel(x0 + dx, y0 + dy)? {
                Px::Metres(m) => acc += w * m as f64,
                Px::Ocean => {}
                Px::Missing => return Ok(None),
            }
        }
        Ok(Some(acc))
    }

    /// The mean land height over the box of half-size (`half_lat`, `half_lon`) degrees around
    /// (lat, lon) (ocean pixels count as 0 m), the area average a coarse lattice sample stands for;
    /// None when any pixel in it is outside the window.
    pub fn box_mean(&mut self, lat: f64, lon: f64, half_lat: f64, half_lon: f64) -> Result<Option<f64>, String> {
        let (fx0, fy0) = self.frac(lat + half_lat, lon - half_lon);
        let (fx1, fy1) = self.frac(lat - half_lat, lon + half_lon);
        let (x0, x1, y0, y1) = (fx0.round() as i64, fx1.round() as i64, fy0.round() as i64, fy1.round() as i64);
        let (mut sum, mut n) = (0.0, 0usize);
        for y in y0..=y1.max(y0) {
            for x in x0..=x1.max(x0) {
                match self.pixel(x, y)? {
                    Px::Metres(m) => sum += m as f64,
                    Px::Ocean => {}
                    Px::Missing => return Ok(None),
                }
                n += 1;
            }
        }
        Ok(Some(sum / n.max(1) as f64))
    }

    /// The highest pixel within `radius_px` pixels of (lat, lon) (a summit's reading).
    pub fn peak(&mut self, lat: f64, lon: f64, radius_px: i64) -> Result<Option<f32>, String> {
        let (fx, fy) = self.frac(lat, lon);
        let (cx, cy) = (fx.round() as i64, fy.round() as i64);
        let mut best: Option<f32> = None;
        for y in cy - radius_px..=cy + radius_px {
            for x in cx - radius_px..=cx + radius_px {
                if let Px::Metres(m) = self.pixel(x, y)? {
                    best = Some(best.map_or(m, |b| b.max(m)));
                }
            }
        }
        Ok(best)
    }

    /// Drop the decoded tiles (between packs).
    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

/// Where rung 2's window was fetched (whole-earth-maps.md section 14, the operator's "Yes").
pub const WASHINGTON_WINDOW: &str = r"E:\HumanityOS\maps-src\gedtm30-v1.2-washington";

#[cfg(test)]
mod tests {
    use super::*;

    /// The tile decode: inflate, predictor 2 on 32-bit words, float32 bits as stored. Seen red
    /// 2026-11-09 with the accumulator carried from one row into the next: "row 1 decodes alone;
    /// left: [12.5, -0.25, 1.62097e-36, 0.12499999]".
    #[test]
    fn a_tile_decodes_by_index_json_s_recipe() {
        let vals: [f32; 4] = [12.5, -0.25, 4412.7, 3.4028235e38];
        let words: Vec<u32> = vals.iter().map(|v| v.to_bits()).collect();
        let mut raw = Vec::new();
        for row in words.chunks(2) {
            raw.extend_from_slice(&row[0].to_le_bytes());
            raw.extend_from_slice(&row[1].wrapping_sub(row[0]).to_le_bytes());
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        assert_eq!(decode_tile(&z, 2).unwrap(), vals.to_vec(), "row 1 decodes alone");
        assert!(decode_tile(&z, 3).is_err(), "a tile of the wrong size");
    }

    /// THE SCALE TRAP, pinned on the real file (whole-earth-maps.md section 14): the stored values
    /// are metres, not decimetres. Mount Rainier's highest pixel reads 4,412.7 m and Mount Adams's
    /// 3,737.2 m (surveyed summits 4,392 m and 3,743 m); Dyes Inlet reads 0. Applying the file's
    /// SCALE 0.1 would read Rainier at 441 m. Runs where the fetched window is on disk (the
    /// operator's PC; not CI), and says so when it is not. Seen red 2026-11-09 with
    /// `f32::from_bits(acc) * 0.1` in `decode_tile`: "Mount Rainier reads 441.3 m".
    #[test]
    fn gedtm30_reads_metres_not_decimetres() {
        let dir = Path::new(WASHINGTON_WINDOW);
        if !dir.join("index.json").exists() {
            println!("GEDTM30 window not on this machine ({WASHINGTON_WINDOW}): the summit check is skipped");
            return;
        }
        let mut g = Gedtm::open(dir).expect("index.json reads");
        let rainier = g.peak(46.8528, -121.7603, 6).unwrap().expect("Rainier is inside the window");
        let adams = g.peak(46.2024, -121.4909, 6).unwrap().expect("Adams is inside the window");
        assert!((4380.0..4420.0).contains(&rainier), "Mount Rainier reads {rainier:.1} m");
        assert!((3720.0..3750.0).contains(&adams), "Mount Adams reads {adams:.1} m");
        let dyes = g.bilinear(47.6230, -122.6870).unwrap().expect("Dyes Inlet is inside the window");
        println!("[gedtm30] Rainier {rainier:.1} m, Adams {adams:.1} m, Dyes Inlet {dyes:.2} m");
        assert!(dyes.abs() < 1.0, "Dyes Inlet reads {dyes:.2} m");
        assert_eq!(g.bilinear(50.0, -122.0).unwrap(), None, "north of the window is no source, not sea");
    }
}
