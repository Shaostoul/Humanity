//! A small tiled GeoTIFF reader, our own (whole-Earth maps rung 2): exactly what ESA WorldCover
//! 2021 (8-bit classes, 1024-pixel tiles, deflate, a palette) and NOAA ETOPO 2022 (32-bit floats,
//! 256-pixel tiles, deflate, the floating-point predictor) need, and nothing else.
//!
//! TIFF 6.0 (classic, not BigTIFF): an image file directory of tags; the image is cut into tiles
//! (TileWidth 322, TileLength 323, TileOffsets 324, TileByteCounts 325), each compressed alone
//! (Compression 259 = 8, zlib deflate), optionally after a predictor (Predictor 317: 2 horizontal
//! differencing, 3 floating point, Adobe's TIFF Technical Note 3: the row's bytes are split into
//! byte planes, most significant first, and differenced). The georeference is GeoTIFF's
//! ModelPixelScale (33550) and ModelTiepoint (33922) with the raster type key (GeoKey 1025:
//! 1 = a pixel is an area, its tiepoint the corner; 2 = a pixel is a point).
//!
//! Untrusted input: offsets and counts are checked against the file, and a tile is read only when
//! asked (a 36,000-pixel WorldCover file is never decoded whole).

use std::collections::HashMap;

/// One opened GeoTIFF: its directory, georeference and the file's bytes.
pub struct GeoTiff {
    bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub tile_w: u32,
    pub tile_h: u32,
    pub bits: u16,
    /// 1 unsigned integer, 3 IEEE float.
    pub sample_format: u16,
    pub predictor: u16,
    offsets: Vec<u64>,
    counts: Vec<u64>,
    /// Longitude and latitude of pixel (0, 0)'s CENTRE, and the pixel size in degrees.
    pub lon0: f64,
    pub lat0: f64,
    pub dlon: f64,
    pub dlat: f64,
    /// GDAL's no-data value (tag 42113), when given.
    pub nodata: Option<f64>,
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(b.get(at..at + 2).ok_or("truncated TIFF")?.try_into().map_err(|_| "u16")?))
}
fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(b.get(at..at + 4).ok_or("truncated TIFF")?.try_into().map_err(|_| "u32")?))
}
fn f64_at(b: &[u8], at: usize) -> Result<f64, String> {
    Ok(f64::from_le_bytes(b.get(at..at + 8).ok_or("truncated TIFF")?.try_into().map_err(|_| "f64")?))
}

/// A tag's values as numbers (SHORT, LONG or DOUBLE), or its bytes as text (ASCII).
enum TagVal {
    Nums(Vec<f64>),
    Text(String),
}

impl GeoTiff {
    /// Parse the first image directory of a little-endian classic TIFF.
    pub fn open(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.get(0..4) != Some(&[b'I', b'I', 42, 0][..]) {
            return Err("not a little-endian classic TIFF (BigTIFF and big-endian are not read)".into());
        }
        let ifd = u32_at(&bytes, 4)? as usize;
        let n = u16_at(&bytes, ifd)? as usize;
        let mut tags: HashMap<u16, TagVal> = HashMap::new();
        for i in 0..n {
            let e = ifd + 2 + i * 12;
            let (tag, ty, count) = (u16_at(&bytes, e)?, u16_at(&bytes, e + 2)?, u32_at(&bytes, e + 4)? as usize);
            let size = match ty {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            if count > (1 << 24) {
                return Err(format!("tag {tag} with {count} values"));
            }
            let at = if size * count <= 4 { e + 8 } else { u32_at(&bytes, e + 8)? as usize };
            let data = bytes.get(at..at + size * count).ok_or_else(|| format!("tag {tag} past the end of the file"))?;
            let val = match ty {
                2 => TagVal::Text(String::from_utf8_lossy(data).trim_end_matches('\0').to_string()),
                3 => TagVal::Nums((0..count).map(|k| u16_at(data, 2 * k).map(|v| v as f64)).collect::<Result<_, _>>()?),
                4 => TagVal::Nums((0..count).map(|k| u32_at(data, 4 * k).map(|v| v as f64)).collect::<Result<_, _>>()?),
                12 => TagVal::Nums((0..count).map(|k| f64_at(data, 8 * k)).collect::<Result<_, _>>()?),
                _ => continue,
            };
            tags.insert(tag, val);
        }
        let nums = |t: u16| -> Result<&Vec<f64>, String> {
            match tags.get(&t) {
                Some(TagVal::Nums(v)) => Ok(v),
                _ => Err(format!("TIFF tag {t} missing")),
            }
        };
        let one = |t: u16| -> Result<f64, String> { nums(t)?.first().copied().ok_or_else(|| format!("TIFF tag {t} empty")) };
        let opt = |t: u16, d: f64| -> f64 { nums(t).ok().and_then(|v| v.first().copied()).unwrap_or(d) };
        if one(259)? != 8.0 {
            return Err(format!("compression {} (only deflate, 8, is read)", one(259)?));
        }
        if opt(277, 1.0) != 1.0 {
            return Err("more than one sample per pixel".into());
        }
        let (scale, tie) = (nums(33550)?, nums(33922)?);
        if scale.len() < 2 || tie.len() < 6 {
            return Err("no GeoTIFF pixel scale or tiepoint".into());
        }
        // GeoKey 1025 (GTRasterTypeGeoKey) in the key directory (34735): 1 area, 2 point.
        let mut raster_type = 1.0;
        if let Ok(keys) = nums(34735) {
            for k in keys.chunks_exact(4).skip(1) {
                if k[0] == 1025.0 && k[1] == 0.0 {
                    raster_type = k[3];
                }
            }
        }
        let (dlon, dlat) = (scale[0], scale[1]);
        // The tiepoint maps raster (i, j) to (lon, lat); an area pixel's tiepoint is its corner.
        let half = if raster_type == 1.0 { 0.5 } else { 0.0 };
        let lon0 = tie[3] + (half - tie[0]) * dlon;
        let lat0 = tie[4] - (half - tie[1]) * dlat;
        let nodata = match tags.get(&42113) {
            Some(TagVal::Text(t)) => t.trim().parse::<f64>().ok(),
            _ => None,
        };
        let offsets: Vec<u64> = nums(324)?.iter().map(|v| *v as u64).collect();
        let counts: Vec<u64> = nums(325)?.iter().map(|v| *v as u64).collect();
        let g = GeoTiff {
            width: one(256)? as u32,
            height: one(257)? as u32,
            tile_w: one(322)? as u32,
            tile_h: one(323)? as u32,
            bits: one(258)? as u16,
            sample_format: opt(339, 1.0) as u16,
            predictor: opt(317, 1.0) as u16,
            offsets,
            counts,
            lon0,
            lat0,
            dlon,
            dlat,
            nodata,
            bytes,
        };
        if g.tile_w == 0 || g.tile_h == 0 {
            return Err("a zero tile size".into());
        }
        let tiles = g.tiles_across() as usize * g.height.div_ceil(g.tile_h) as usize;
        if g.offsets.len() < tiles || g.counts.len() < tiles {
            return Err(format!("{} tile offsets for {tiles} tiles", g.offsets.len()));
        }
        if !matches!((g.bits, g.sample_format), (8, 1) | (32, 3)) {
            return Err(format!("{} bits, sample format {} (8-bit integers and 32-bit floats are read)", g.bits, g.sample_format));
        }
        Ok(g)
    }

    /// Tiles across the image.
    pub fn tiles_across(&self) -> u32 {
        self.width.div_ceil(self.tile_w)
    }

    /// Decode tile (tx, ty) to its samples as f32 (an 8-bit class becomes its number), row-major,
    /// `tile_w * tile_h` long (an edge tile is padded, as stored).
    pub fn tile(&self, tx: u32, ty: u32) -> Result<Vec<f32>, String> {
        let i = (ty * self.tiles_across() + tx) as usize;
        let (off, cnt) = (*self.offsets.get(i).ok_or("tile index")? as usize, *self.counts.get(i).ok_or("tile index")? as usize);
        let z = self.bytes.get(off..off + cnt).ok_or("tile past the end of the file")?;
        let bps = (self.bits / 8) as usize;
        let (w, h) = (self.tile_w as usize, self.tile_h as usize);
        let mut raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(z, w * h * bps).map_err(|e| format!("tile ({tx}, {ty}) deflate: {e:?}"))?;
        if raw.len() != w * h * bps {
            return Err(format!("tile ({tx}, {ty}) inflated to {} bytes, not {}", raw.len(), w * h * bps));
        }
        match (self.predictor, bps) {
            (1, _) => {}
            (2, 1) => {
                for row in raw.chunks_exact_mut(w) {
                    for x in 1..w {
                        row[x] = row[x].wrapping_add(row[x - 1]);
                    }
                }
            }
            (3, 4) => {
                for row in raw.chunks_exact_mut(w * 4) {
                    for x in 1..row.len() {
                        row[x] = row[x].wrapping_add(row[x - 1]);
                    }
                    // Byte planes (most significant first) back to little-endian floats.
                    let planes = row.to_vec();
                    for x in 0..w {
                        for b in 0..4 {
                            row[x * 4 + b] = planes[(3 - b) * w + x];
                        }
                    }
                }
            }
            (p, _) => return Err(format!("predictor {p} with {} bits", self.bits)),
        }
        Ok(match bps {
            1 => raw.into_iter().map(|v| v as f32).collect(),
            _ => raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
        })
    }

    /// The pixel holding (lat, lon), as (column, row), when inside the image.
    pub fn pixel_of(&self, lat: f64, lon: f64) -> Option<(i64, i64)> {
        let x = ((lon - self.lon0) / self.dlon + 0.5).floor() as i64;
        let y = ((self.lat0 - lat) / self.dlat + 0.5).floor() as i64;
        (x >= 0 && y >= 0 && x < self.width as i64 && y < self.height as i64).then_some((x, y))
    }
}

/// A GeoTIFF's tiles, decoded on demand and kept for the build (a builder's working set).
pub struct TileCache {
    pub tiff: GeoTiff,
    tiles: HashMap<(u32, u32), Vec<f32>>,
}

impl TileCache {
    pub fn new(tiff: GeoTiff) -> Self {
        TileCache { tiff, tiles: HashMap::new() }
    }

    /// The sample at pixel (x, y); None outside the image or at its no-data value.
    pub fn pixel(&mut self, x: i64, y: i64) -> Result<Option<f32>, String> {
        let t = &self.tiff;
        if x < 0 || y < 0 || x >= t.width as i64 || y >= t.height as i64 {
            return Ok(None);
        }
        let (tx, ty) = (x as u32 / t.tile_w, y as u32 / t.tile_h);
        let (tw, th) = (t.tile_w, t.tile_h);
        let nodata = t.nodata;
        if !self.tiles.contains_key(&(tx, ty)) {
            let d = self.tiff.tile(tx, ty)?;
            self.tiles.insert((tx, ty), d);
        }
        let v = self.tiles[&(tx, ty)][((y as u32 % th) * tw + x as u32 % tw) as usize];
        Ok((nodata != Some(v as f64) && v.is_finite()).then_some(v))
    }

    /// Bilinear between the four pixel centres around (lat, lon); None when any is missing.
    pub fn bilinear(&mut self, lat: f64, lon: f64) -> Result<Option<f32>, String> {
        let fx = (lon - self.tiff.lon0) / self.tiff.dlon;
        let fy = (self.tiff.lat0 - lat) / self.tiff.dlat;
        let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let (Some(a), Some(b), Some(c), Some(d)) = (self.pixel(x0, y0)?, self.pixel(x0 + 1, y0)?, self.pixel(x0, y0 + 1)?, self.pixel(x0 + 1, y0 + 1)?) else {
            return Ok(None);
        };
        let top = a as f64 * (1.0 - tx) + b as f64 * tx;
        let bot = c as f64 * (1.0 - tx) + d as f64 * tx;
        Ok(Some((top * (1.0 - ty) + bot * ty) as f32))
    }

    /// Release every decoded tile (between packs).
    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-tile float GeoTIFF built by hand with the floating-point predictor, read back:
    /// samples, georeference and no-data. Seen red 2026-11-09 with the byte planes taken least
    /// significant first: "the floating-point predictor undoes; left: [6.8965e-41, 6.009e-42, ...".
    #[test]
    fn a_hand_built_float_tiff_reads_back() {
        let (w, h) = (4u32, 2u32);
        let vals: Vec<f32> = vec![1.5, -2.25, 4412.7, 0.0, -9999.0, 3.0, 8.5, 100.0];
        // Encode: per row, byte planes MSB first, then horizontal differencing on bytes.
        let mut raw = Vec::new();
        for row in vals.chunks(w as usize) {
            let mut planes = vec![0u8; row.len() * 4];
            for (x, v) in row.iter().enumerate() {
                let b = v.to_le_bytes();
                for p in 0..4 {
                    planes[p * row.len() + x] = b[3 - p];
                }
            }
            for x in (1..planes.len()).rev() {
                planes[x] = planes[x].wrapping_sub(planes[x - 1]);
            }
            raw.extend_from_slice(&planes);
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        // Header, one IFD, tags out of line where they do not fit.
        let mut f: Vec<u8> = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
        let tags: Vec<(u16, u16, u32, Vec<u8>)> = vec![
            (256, 3, 1, (w as u16).to_le_bytes().to_vec()),
            (257, 3, 1, (h as u16).to_le_bytes().to_vec()),
            (258, 3, 1, 32u16.to_le_bytes().to_vec()),
            (259, 3, 1, 8u16.to_le_bytes().to_vec()),
            (317, 3, 1, 3u16.to_le_bytes().to_vec()),
            (322, 3, 1, (w as u16).to_le_bytes().to_vec()),
            (323, 3, 1, (h as u16).to_le_bytes().to_vec()),
            (324, 4, 1, vec![0; 4]),
            (325, 4, 1, (z.len() as u32).to_le_bytes().to_vec()),
            (339, 3, 1, 3u16.to_le_bytes().to_vec()),
            (33550, 12, 3, [0.25f64, 0.25, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect()),
            (33922, 12, 6, [0.0f64, 0.0, 0.0, -135.0, 60.0, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect()),
            (42113, 2, 6, b"-9999\0".to_vec()),
        ];
        let ifd_len = 2 + tags.len() * 12 + 4;
        let mut extra = Vec::new();
        let mut entries = Vec::new();
        let mut tile_off_entry = 0;
        for (i, (tag, ty, count, data)) in tags.iter().enumerate() {
            entries.extend_from_slice(&tag.to_le_bytes());
            entries.extend_from_slice(&ty.to_le_bytes());
            entries.extend_from_slice(&count.to_le_bytes());
            if *tag == 324 {
                tile_off_entry = 8 + 2 + i * 12 + 8;
            }
            if data.len() <= 4 {
                let mut d = data.clone();
                d.resize(4, 0);
                entries.extend_from_slice(&d);
            } else {
                entries.extend_from_slice(&((8 + ifd_len + extra.len()) as u32).to_le_bytes());
                extra.extend_from_slice(data);
            }
        }
        f.extend_from_slice(&(tags.len() as u16).to_le_bytes());
        f.extend_from_slice(&entries);
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&extra);
        let tile_at = f.len() as u32;
        f[tile_off_entry..tile_off_entry + 4].copy_from_slice(&tile_at.to_le_bytes());
        f.extend_from_slice(&z);
        let g = GeoTiff::open(f).expect("opens");
        assert_eq!(g.tile(0, 0).unwrap(), vals, "the floating-point predictor undoes");
        assert_eq!((g.lon0, g.lat0), (-134.875, 59.875), "an area pixel's centre is half a pixel in from the tiepoint");
        let mut c = TileCache::new(g);
        assert_eq!(c.pixel(0, 1).unwrap(), None, "the no-data value reads as none");
        assert_eq!(c.pixel(2, 0).unwrap(), Some(4412.7));
        let mid = c.bilinear(59.875, -134.5).unwrap().unwrap();
        assert!((mid - (4412.7 - 2.25) / 2.0).abs() < 1e-3, "halfway between two pixel centres: {mid}");
        assert_eq!(c.bilinear(59.875, -134.75).unwrap(), None, "a no-data neighbour leaves no value");
        assert_eq!(c.tiff.pixel_of(59.9, -134.9), Some((0, 0)));
        assert_eq!(c.tiff.pixel_of(61.0, -134.9), None);
    }
}
