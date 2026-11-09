//! MAP PACKS: the whole-Earth map files, one per depth-6 terrain patch (whole-Earth maps rung 2,
//! 2026-11-09; docs/design/whole-earth-maps.md sections 3.4, 9.2 and 12).
//!
//! A pack holds the map data for ONE tile of the terrain's own quadtree: its id is a `PatchId`
//! (the local pack's depth is `levels.ron`'s `local_pack_depth`, 6, about 110 km), and inside it
//! every layer is cut into the tiles of the data levels (6, 9 and 12) by the same key, so the map,
//! the heights and the mesh share one grid. This module is the FORMAT: the reader the game uses and
//! the writer the offline builder (`map_build`) uses, side by side so the two cannot drift. Pure std
//! plus `miniz_oxide` (deflate) and `ron`: relay-safe, no I/O.
//!
//! ## The file, "HOSMPAK1" (little-endian, no padding)
//!
//! ```text
//!   0..8    magic b"HOSMPAK1"
//!   8..12   u32 manifest_len, then manifest_len bytes of UTF-8 RON: PackManifest (the tile, the
//!           build, every layer's source, snapshot date and licence, and the notices the licences
//!           ask for: section 9.2, so the pack itself carries the ODbL offer)
//!   ..      u32 chunk_count, then chunk_count entries of 28 bytes:
//!             u8 kind, u8 level, u16 0, u64 tile path (the face is the pack's), u64 offset from
//!             the file's start, u32 stored length, u32 raw length
//!   ..      the chunks, each a zlib stream, in entry order (kind, level, path ascending)
//! ```
//!
//! Chunk kinds, one per layer and tile (section 3.4: no layer is ever edited using another layer's
//! data in the file, so a pack is a Collective Database and the joining happens at run time):
//! - [`ChunkKind::LandH`] land heights (GEDTM30) at levels 6, 9 and 12, and
//!   [`ChunkKind::SeaH`] seabed heights (ETOPO 2022) at levels 6 and 9: the tile's 64-segment
//!   nested lattice (`map_tiles::tile_lattice`, (64+1)(64+2)/2 = 2,145 samples in
//!   `map_tiles::lattice_index` order) as i32 decimetres, each the zigzag varint of its residual
//!   against the plane through its left, upper and upper-left neighbours; [`NODATA`] where the
//!   source has no value.
//! - [`ChunkKind::Cover`] land cover (ESA WorldCover classes, 0 = none) on the level-12 lattice, a
//!   byte per sample.
//! - [`ChunkKind::Water`] the water polygons clipped to a level-12 tile (`map_tiles::clip_ring`),
//!   every ring with water on its LEFT (sea and lake outlines counter-clockwise from outside the
//!   planet, islands clockwise), each edge flagged when the clip made it (a tile border, never a
//!   shoreline: section 3.5).
//! - [`ChunkKind::Ways`] roads, paths and rail clipped to a level-12 tile (`clip_polyline`).
//! - [`ChunkKind::Buildings`] footprints, each in the level-12 tile holding its first point.
//!
//! Points are OSM's own integers, latitude and longitude times 10^7 (section 3.3), delta-coded as
//! zigzag varints (a feature's first point against the previous feature's first point in its
//! chunk): lossless against the source. OSM ids are deltas too, and a ways chunk names each
//! street once in a table. The run time decodes them to f64.
//!
//! COST: decoding is per tile on demand (map_ground's residency); nothing here runs per frame.

use crate::planet_chunks::PatchId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The file's first eight bytes.
pub const PACK_MAGIC: &[u8; 8] = b"HOSMPAK1";
/// A pack's file extension (`data/maps/packs/<name>.hmpack`).
pub const PACK_EXT: &str = "hmpack";
/// A lattice sample with no source value (outside the source's coverage).
pub const NODATA: i32 = i32::MIN;
/// The height lattice's segments per tile edge: a level-L tile's lattice is its depth L+6 corners.
pub const LATTICE_SEGMENTS: u32 = 64;
/// Bytes of one chunk-directory entry.
const ENTRY_LEN: usize = 28;
/// A chunk may not inflate past this (a corrupt length, not a tile).
const MAX_RAW: usize = 64 << 20;

/// What a chunk holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum ChunkKind {
    LandH = 1,
    SeaH = 2,
    Cover = 3,
    Water = 4,
    Ways = 5,
    Buildings = 6,
}

impl ChunkKind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::LandH,
            2 => Self::SeaH,
            3 => Self::Cover,
            4 => Self::Water,
            5 => Self::Ways,
            6 => Self::Buildings,
            _ => return None,
        })
    }
}

/// One layer's provenance, the licence's own words where it asks for them (section 9.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerInfo {
    /// The layer: `osm_water`, `osm_ways`, `osm_buildings`, `land_h`, `sea_h` or `cover`.
    pub name: String,
    /// The source, with the file it was read from.
    pub source: String,
    /// The source's own date: an OSM snapshot timestamp, a dataset's release.
    pub snapshot: String,
    /// The licence's SPDX id.
    pub licence: String,
    /// Where the licence is read in full.
    pub licence_url: String,
    /// The credit line the licence or the publisher asks for.
    pub notice: String,
}

/// What a pack says about itself: read before any chunk, shown in Settings > Credits, and the
/// reason a pack can travel alone (it carries its own notices and its format's location).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    /// Always "HOSMPAK1".
    pub format: String,
    /// Where this format is specified (the ODbL 4.6 offer: a documented format, free).
    pub spec: String,
    /// The pack's tile: (face, depth, path).
    pub tile: (u8, u8, u64),
    /// A content hash of the pack's chunks, the builder's run of them: the version the run time's
    /// in-memory reuse is tagged with (section 9.5).
    pub build_id: String,
    /// The tool and version that wrote it.
    pub builder: String,
    /// The data levels inside, ascending.
    pub levels: Vec<u8>,
    /// The height lattice's segments per tile edge.
    pub lattice_segments: u32,
    /// Every layer's source, date and licence.
    pub layers: Vec<LayerInfo>,
    /// The notices the licences ask to travel with the data (ODbL text and offer, CC BY, the
    /// Copernicus notice and liability sentence).
    pub notices: Vec<String>,
}

impl PackManifest {
    /// The pack's tile as a `PatchId`.
    pub fn patch(&self) -> PatchId {
        PatchId { face: self.tile.0, depth: self.tile.1, path: self.tile.2 }
    }
}

/// One entry of the chunk directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkEntry {
    pub kind: ChunkKind,
    pub level: u8,
    pub path: u64,
    offset: u64,
    stored: u32,
    raw: u32,
}

impl ChunkEntry {
    /// The chunk's size in the file (deflated).
    pub fn stored_len(&self) -> u32 {
        self.stored
    }
}

/// An opened pack: the bytes, the manifest and the directory. Chunks are inflated on demand.
pub struct Pack {
    pub manifest: PackManifest,
    bytes: Vec<u8>,
    dir: HashMap<(ChunkKind, u8, u64), ChunkEntry>,
}

impl std::fmt::Debug for Pack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pack").field("tile", &self.manifest.tile).field("chunks", &self.dir.len()).field("bytes", &self.bytes.len()).finish()
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

impl Pack {
    /// Open a pack from its bytes: the magic, the manifest and the directory are checked; a chunk
    /// whose range leaves the file refuses the whole pack.
    pub fn open(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.get(0..8) != Some(&PACK_MAGIC[..]) {
            return Err("not a map pack (no HOSMPAK1 magic)".into());
        }
        let mlen = u32_at(&bytes, 8).ok_or("truncated header")? as usize;
        let mtext = std::str::from_utf8(bytes.get(12..12 + mlen).ok_or("truncated manifest")?).map_err(|e| format!("manifest is not UTF-8: {e}"))?;
        let manifest: PackManifest = ron::from_str(mtext).map_err(|e| format!("manifest: {e}"))?;
        if manifest.format != "HOSMPAK1" {
            return Err(format!("manifest format {:?}", manifest.format));
        }
        let at = 12 + mlen;
        let count = u32_at(&bytes, at).ok_or("truncated directory")? as usize;
        let mut dir = HashMap::with_capacity(count);
        for i in 0..count {
            let e = at + 4 + i * ENTRY_LEN;
            let kind = ChunkKind::from_u8(*bytes.get(e).ok_or("truncated directory")?).ok_or("unknown chunk kind")?;
            let entry = ChunkEntry {
                kind,
                level: bytes[e + 1],
                path: u64_at(&bytes, e + 4).ok_or("truncated directory")?,
                offset: u64_at(&bytes, e + 12).ok_or("truncated directory")?,
                stored: u32_at(&bytes, e + 20).ok_or("truncated directory")?,
                raw: u32_at(&bytes, e + 24).ok_or("truncated directory")?,
            };
            let end = entry.offset.checked_add(entry.stored as u64).ok_or("chunk range overflows")?;
            if end > bytes.len() as u64 || entry.raw as usize > MAX_RAW {
                return Err(format!("chunk {kind:?} level {} path {} leaves the file", entry.level, entry.path));
            }
            dir.insert((kind, entry.level, entry.path), entry);
        }
        Ok(Pack { manifest, bytes, dir })
    }

    /// The pack's tile.
    pub fn patch(&self) -> PatchId {
        self.manifest.patch()
    }

    /// The file's size in bytes.
    pub fn len_bytes(&self) -> usize {
        self.bytes.len()
    }

    /// How many chunks of `kind` the pack holds.
    pub fn count(&self, kind: ChunkKind) -> usize {
        self.dir.keys().filter(|k| k.0 == kind).count()
    }

    /// Every directory entry, in no particular order.
    pub fn entries(&self) -> impl Iterator<Item = &ChunkEntry> {
        self.dir.values()
    }

    /// The raw (inflated) bytes of one chunk; None when the pack has no such chunk or it does not
    /// inflate to its recorded length.
    pub fn chunk(&self, kind: ChunkKind, level: u8, path: u64) -> Option<Vec<u8>> {
        let e = self.dir.get(&(kind, level, path))?;
        let stored = &self.bytes[e.offset as usize..(e.offset + e.stored as u64) as usize];
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(stored, e.raw as usize).ok()?;
        (raw.len() == e.raw as usize).then_some(raw)
    }

    /// A height lattice (land or sea) in metres, [`NODATA`] samples as NaN.
    pub fn heights(&self, kind: ChunkKind, level: u8, path: u64) -> Option<Vec<f32>> {
        let raw = self.chunk(kind, level, path)?;
        let dm = decode_lattice(&raw, crate::map_tiles::lattice_len(self.manifest.lattice_segments))?;
        Some(dm.into_iter().map(|v| if v == NODATA { f32::NAN } else { v as f32 * 0.1 }).collect())
    }

    /// A tile's water rings.
    pub fn water(&self, path: u64) -> Option<Vec<WaterRing>> {
        decode_water(&self.chunk(ChunkKind::Water, local_level(self), path)?)
    }

    /// A tile's ways.
    pub fn ways(&self, path: u64) -> Option<Vec<MapWay>> {
        decode_ways(&self.chunk(ChunkKind::Ways, local_level(self), path)?)
    }

    /// A tile's buildings.
    pub fn buildings(&self, path: u64) -> Option<Vec<MapBuilding>> {
        decode_buildings(&self.chunk(ChunkKind::Buildings, local_level(self), path)?)
    }
}

/// The finest level a pack holds (its vectors' level).
pub fn local_level(pack: &Pack) -> u8 {
    pack.manifest.levels.last().copied().unwrap_or(12)
}

// ── Varints ──────────────────────────────────────────────────────────────────────────────────

/// Append an unsigned LEB128 varint.
pub fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Read an unsigned LEB128 varint at `*pos`, advancing it.
pub fn get_varint(b: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*pos)?;
        *pos += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte < 0x80 {
            return Some(v);
        }
    }
    None
}

fn zig(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}
fn unzig(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// Append a signed varint (zigzag).
pub fn put_svarint(out: &mut Vec<u8>, v: i64) {
    put_varint(out, zig(v));
}

/// Read a signed varint (zigzag).
pub fn get_svarint(b: &[u8], pos: &mut usize) -> Option<i64> {
    get_varint(b, pos).map(unzig)
}

fn put_str(out: &mut Vec<u8>, s: Option<&str>) {
    let s = s.unwrap_or("");
    put_varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn get_str(b: &[u8], pos: &mut usize) -> Option<Option<String>> {
    let n = get_varint(b, pos)? as usize;
    let s = std::str::from_utf8(b.get(*pos..*pos + n)?).ok()?.to_string();
    *pos += n;
    Some((!s.is_empty()).then_some(s))
}

/// A feature's points, delta-coded; its first point as a delta from the previous feature's first
/// point in the same chunk (`anchor`, starting at 0, 0), so neighbours cost bytes, not words.
fn put_points(out: &mut Vec<u8>, pts: &[(i32, i32)], anchor: &mut (i64, i64)) {
    put_varint(out, pts.len() as u64);
    let mut prev = *anchor;
    for (i, &(la, lo)) in pts.iter().enumerate() {
        put_svarint(out, la as i64 - prev.0);
        put_svarint(out, lo as i64 - prev.1);
        prev = (la as i64, lo as i64);
        if i == 0 {
            *anchor = prev;
        }
    }
}

fn get_points(b: &[u8], pos: &mut usize, anchor: &mut (i64, i64)) -> Option<Vec<(i32, i32)>> {
    let n = get_varint(b, pos)? as usize;
    if n > b.len() {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    let mut prev = *anchor;
    for i in 0..n {
        prev.0 += get_svarint(b, pos)?;
        prev.1 += get_svarint(b, pos)?;
        out.push((i32::try_from(prev.0).ok()?, i32::try_from(prev.1).ok()?));
        if i == 0 {
            *anchor = prev;
        }
    }
    Some(out)
}

// ── Height lattices ──────────────────────────────────────────────────────────────────────────

/// The rows of a triangular lattice of `n` samples ((s+1)(s+2)/2 for s segments), or None.
fn lattice_rows(n: usize) -> Option<usize> {
    let mut rows = 0;
    while rows * (rows + 1) / 2 < n {
        rows += 1;
    }
    (rows * (rows + 1) / 2 == n).then_some(rows)
}

/// The predicted value of lattice sample (r, c) from the ones before it in
/// `map_tiles::lattice_index` order: the plane through its left, upper and upper-left neighbours
/// (the residual of smooth ground is then near zero, a byte as a varint), the left or the upper
/// neighbour on the edges, 0 where a neighbour has no data.
fn predict(v: &[i32], r: usize, c: usize) -> i64 {
    let at = |r: usize, c: usize| v[r * (r + 1) / 2 + c];
    let ok = |x: i32| (x != NODATA).then_some(x as i64);
    let p = if r == 0 {
        Some(0)
    } else if c == 0 {
        ok(at(r - 1, 0))
    } else if c == r {
        match (ok(at(r, c - 1)), ok(at(r - 1, c - 1))) {
            (Some(l), Some(u)) if c >= 2 => ok(at(r - 1, c - 2)).map(|ul| l + u - ul),
            (Some(l), _) => Some(l),
            _ => None,
        }
    } else {
        match (ok(at(r, c - 1)), ok(at(r - 1, c)), ok(at(r - 1, c - 1))) {
            (Some(l), Some(u), Some(ul)) => Some(l + u - ul),
            _ => None,
        }
    };
    p.unwrap_or(0)
}

/// A lattice of i32 decimetres as zigzag varints of each sample's residual against
/// [`predict`] (rung 2's size pass: about 40% smaller than plain deltas on 30 m terrain).
pub fn encode_lattice(dm: &[i32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(dm.len());
    let Some(rows) = lattice_rows(dm.len()) else {
        return out;
    };
    for r in 0..rows {
        for c in 0..=r {
            put_svarint(&mut out, dm[r * (r + 1) / 2 + c] as i64 - predict(dm, r, c));
        }
    }
    out
}

/// [`encode_lattice`]'s inverse; None unless exactly `n` samples (a triangular number) decode.
pub fn decode_lattice(b: &[u8], n: usize) -> Option<Vec<i32>> {
    let rows = lattice_rows(n)?;
    let mut pos = 0;
    let mut out = vec![0i32; n];
    for r in 0..rows {
        for c in 0..=r {
            let v = predict(&out, r, c) + get_svarint(b, &mut pos)?;
            out[r * (r + 1) / 2 + c] = i32::try_from(v).ok()?;
        }
    }
    (pos == b.len()).then_some(out)
}

/// Metres to the stored decimetres, rounded; NaN (no data) to [`NODATA`].
pub fn metres_to_dm(m: f64) -> i32 {
    if m.is_finite() {
        (m * 10.0).round().clamp(-2.0e9, 2.0e9) as i32
    } else {
        NODATA
    }
}

// ── Water ────────────────────────────────────────────────────────────────────────────────────

/// What a water ring outlines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WaterRingKind {
    /// Tidal water assembled from OSM's `natural=coastline` (its surface is sea level).
    Sea = 0,
    /// A lake, pond or reservoir (`natural=water`): a flat surface found at run time.
    Lake = 1,
    /// A river's area (`waterway=riverbank`, `water=river`).
    River = 2,
    /// Land inside water: a coastline island in the sea, or an inner ring of a lake.
    Island = 3,
}

impl WaterRingKind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Sea,
            1 => Self::Lake,
            2 => Self::River,
            3 => Self::Island,
            _ => return None,
        })
    }
    /// Water rather than land inside water.
    pub fn is_water(self) -> bool {
        !matches!(self, Self::Island)
    }
}

/// One water ring clipped to a tile, water on its left.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterRing {
    pub kind: WaterRingKind,
    /// The water body it belongs to: the OSM way or relation id of a lake or river (an island's:
    /// the body around it), 0 for the sea. Rings of one body in many tiles share it, so a lake's
    /// surface is found once for the whole lake.
    pub feature: u64,
    pub name: Option<String>,
    /// Latitude and longitude times 10^7, closed implicitly (the last point joins the first).
    pub points: Vec<(i32, i32)>,
    /// `clip[i]` is true when the edge from point i to the next was made by the clip: a tile
    /// border, never a shoreline.
    pub clip: Vec<bool>,
}

/// A tile's water rings to the chunk's bytes.
pub fn encode_water(rings: &[WaterRing]) -> Vec<u8> {
    let mut out = Vec::new();
    put_varint(&mut out, rings.len() as u64);
    let mut anchor = (0i64, 0i64);
    for r in rings {
        out.push(r.kind as u8);
        put_varint(&mut out, r.feature);
        put_str(&mut out, r.name.as_deref());
        put_points(&mut out, &r.points, &mut anchor);
        let mut bits = vec![0u8; r.points.len().div_ceil(8)];
        for (i, c) in r.clip.iter().enumerate().take(r.points.len()) {
            if *c {
                bits[i / 8] |= 1 << (i % 8);
            }
        }
        out.extend_from_slice(&bits);
    }
    out
}

/// [`encode_water`]'s inverse.
pub fn decode_water(b: &[u8]) -> Option<Vec<WaterRing>> {
    let mut pos = 0;
    let n = get_varint(b, &mut pos)? as usize;
    let mut out = Vec::with_capacity(n.min(b.len()));
    let mut anchor = (0i64, 0i64);
    for _ in 0..n {
        let kind = WaterRingKind::from_u8(*b.get(pos)?)?;
        pos += 1;
        let feature = get_varint(b, &mut pos)?;
        let name = get_str(b, &mut pos)?;
        let points = get_points(b, &mut pos, &mut anchor)?;
        let nb = points.len().div_ceil(8);
        let bits = b.get(pos..pos + nb)?;
        pos += nb;
        let clip = (0..points.len()).map(|i| bits[i / 8] & (1 << (i % 8)) != 0).collect();
        out.push(WaterRing { kind, feature, name, points, clip });
    }
    (pos == b.len()).then_some(out)
}

// ── Ways ─────────────────────────────────────────────────────────────────────────────────────

/// One road, path or rail line clipped to a tile.
#[derive(Clone, Debug, PartialEq)]
pub struct MapWay {
    pub osm_id: u64,
    /// The road class (`osm_region::OsmRoad::class`'s scale: 0 motorway/trunk .. 5 footway; 6 rail).
    pub class: u8,
    pub bridge: bool,
    pub tunnel: bool,
    /// OSM's `layer` (0 when untagged).
    pub layer: i8,
    /// The tagged width in metres, 0 when untagged (the class default applies).
    pub width_m: f32,
    pub name: Option<String>,
    pub points: Vec<(i32, i32)>,
    /// The piece begins / ends on the tile's border because the clip cut it there.
    pub start_clipped: bool,
    pub end_clipped: bool,
}

/// A tile's ways to the chunk's bytes: a table of the tile's distinct names first (a street's name
/// is written once, not once per piece), then each way with its id as a delta from the last.
pub fn encode_ways(ways: &[MapWay]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut names: Vec<&str> = ways.iter().filter_map(|w| w.name.as_deref()).collect();
    names.sort_unstable();
    names.dedup();
    put_varint(&mut out, names.len() as u64);
    for n in &names {
        put_str(&mut out, Some(n));
    }
    put_varint(&mut out, ways.len() as u64);
    let mut prev = 0i64;
    let mut anchor = (0i64, 0i64);
    for w in ways {
        put_svarint(&mut out, w.osm_id as i64 - prev);
        prev = w.osm_id as i64;
        out.push(w.class);
        out.push(w.bridge as u8 | (w.tunnel as u8) << 1 | (w.start_clipped as u8) << 2 | (w.end_clipped as u8) << 3);
        out.push(w.layer as u8);
        out.extend_from_slice(&((w.width_m * 10.0).round().clamp(0.0, 65535.0) as u16).to_le_bytes());
        put_varint(&mut out, w.name.as_deref().and_then(|n| names.binary_search(&n).ok()).map_or(0, |i| i as u64 + 1));
        put_points(&mut out, &w.points, &mut anchor);
    }
    out
}

/// [`encode_ways`]'s inverse.
pub fn decode_ways(b: &[u8]) -> Option<Vec<MapWay>> {
    let mut pos = 0;
    let nn = get_varint(b, &mut pos)? as usize;
    let mut names = Vec::with_capacity(nn.min(b.len()));
    for _ in 0..nn {
        names.push(get_str(b, &mut pos)?.unwrap_or_default());
    }
    let n = get_varint(b, &mut pos)? as usize;
    let mut out = Vec::with_capacity(n.min(b.len()));
    let mut prev = 0i64;
    let mut anchor = (0i64, 0i64);
    for _ in 0..n {
        prev += get_svarint(b, &mut pos)?;
        let osm_id = prev as u64;
        let (class, flags, layer) = (*b.get(pos)?, *b.get(pos + 1)?, *b.get(pos + 2)? as i8);
        let width_dm = u16::from_le_bytes(b.get(pos + 3..pos + 5)?.try_into().ok()?);
        pos += 5;
        let name = match get_varint(b, &mut pos)? as usize {
            0 => None,
            i => Some(names.get(i - 1)?.clone()),
        };
        let points = get_points(b, &mut pos, &mut anchor)?;
        out.push(MapWay {
            osm_id,
            class,
            bridge: flags & 1 != 0,
            tunnel: flags & 2 != 0,
            layer,
            width_m: width_dm as f32 * 0.1,
            name,
            points,
            start_clipped: flags & 4 != 0,
            end_clipped: flags & 8 != 0,
        });
    }
    (pos == b.len()).then_some(out)
}

// ── Buildings ────────────────────────────────────────────────────────────────────────────────

/// One building footprint, whole (never clipped: clipping changes a footprint's shape).
#[derive(Clone, Debug, PartialEq)]
pub struct MapBuilding {
    pub osm_id: u64,
    /// From `height`, else `building:levels` times 3 m; 0 = unknown (a class default applies).
    pub height_m: f32,
    /// From `min_height` (a part that starts above the ground), else 0.
    pub min_height_m: f32,
    /// `building:levels`, 0 when untagged.
    pub levels: u8,
    /// The outer ring, open (the last point joins the first), as OSM integers.
    pub ring: Vec<(i32, i32)>,
}

/// A tile's buildings to the chunk's bytes.
pub fn encode_buildings(bs: &[MapBuilding]) -> Vec<u8> {
    let mut out = Vec::new();
    put_varint(&mut out, bs.len() as u64);
    let dm = |m: f32| ((m * 10.0).round().clamp(0.0, 65535.0) as u16).to_le_bytes();
    let mut prev = 0i64;
    let mut anchor = (0i64, 0i64);
    for b in bs {
        put_svarint(&mut out, b.osm_id as i64 - prev);
        prev = b.osm_id as i64;
        out.extend_from_slice(&dm(b.height_m));
        out.extend_from_slice(&dm(b.min_height_m));
        out.push(b.levels);
        put_points(&mut out, &b.ring, &mut anchor);
    }
    out
}

/// [`encode_buildings`]'s inverse.
pub fn decode_buildings(b: &[u8]) -> Option<Vec<MapBuilding>> {
    let mut pos = 0;
    let n = get_varint(b, &mut pos)? as usize;
    let mut out = Vec::with_capacity(n.min(b.len()));
    let dm = |at: usize| -> Option<f32> { Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as f32 * 0.1) };
    let mut prev = 0i64;
    let mut anchor = (0i64, 0i64);
    for _ in 0..n {
        prev += get_svarint(b, &mut pos)?;
        let osm_id = prev as u64;
        let (height_m, min_height_m, levels) = (dm(pos)?, dm(pos + 2)?, *b.get(pos + 4)?);
        pos += 5;
        let ring = get_points(b, &mut pos, &mut anchor)?;
        out.push(MapBuilding { osm_id, height_m, min_height_m, levels, ring });
    }
    (pos == b.len()).then_some(out)
}

/// Which water ring (if any) holds `dir`, among one tile's rings: the innermost water ring, when
/// more water rings than island rings contain the point (a lake on an island in the sea is water;
/// the island around it is land). None is land. Point-in-polygon in the gnomonic plane about the
/// point itself, where every great-circle edge is a straight line, so the test is exact on the
/// sphere. Shared by the builder's geography gates and the run time's shore band.
pub fn point_class(rings: &[WaterRing], dir: glam::DVec3) -> Option<&WaterRing> {
    let c = dir.normalize();
    let e = if c.y.abs() < 0.9 { glam::DVec3::Y.cross(c).normalize() } else { glam::DVec3::X.cross(c).normalize() };
    let n = c.cross(e);
    let plane = |p: glam::DVec3| -> Option<(f64, f64)> {
        let d = p.dot(c);
        (d > 0.0).then(|| (p.dot(e) / d, p.dot(n) / d))
    };
    let mut depth = 0i32;
    let mut best: Option<(f64, &WaterRing)> = None;
    for r in rings {
        let pts: Option<Vec<(f64, f64)>> = r.points.iter().map(|&(la, lo)| plane(crate::map_tiles::e7_to_dir(la, lo))).collect();
        let Some(pts) = pts else { continue };
        let k = pts.len();
        let mut inside = false;
        let mut area = 0.0;
        for i in 0..k {
            let (a, b) = (pts[i], pts[(i + 1) % k]);
            area += a.0 * b.1 - b.0 * a.1;
            if (a.1 > 0.0) != (b.1 > 0.0) && a.0 + (0.0 - a.1) * (b.0 - a.0) / (b.1 - a.1) > 0.0 {
                inside = !inside;
            }
        }
        if inside {
            if r.kind.is_water() {
                depth += 1;
                if best.is_none_or(|(ba, _)| area.abs() < ba) {
                    best = Some((area.abs(), r));
                }
            } else {
                depth -= 1;
            }
        }
    }
    if depth > 0 {
        best.map(|(_, r)| r)
    } else {
        None
    }
}

// ── Notices (whole-earth-maps.md 9.2; the finding 2026-11-08) ────────────────────────────────

/// OpenStreetMap's credit, the one string every surface drawing OSM data shows.
pub const OSM_NOTICE: &str = humanity_core::credits::OSM_NOTICE;
/// GEDTM30's credit (CC BY 4.0 asks for the creator and the licence).
pub const GEDTM30_NOTICE: &str = "Land heights: GEDTM30 v1.2, Global Ensemble Digital Terrain Model (Ho, Hengl, Parente et al., OpenGeoHub, 2026), CC BY 4.0";
/// ESA WorldCover's own credit line, as the project asks for it.
pub const WORLDCOVER_NOTICE: &str = "© ESA WorldCover project 2021 / Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover consortium";
/// The Copernicus DEM notice and liability sentence, carried because GEDTM30 is built partly from
/// it (the finding's open question 1: until a written answer, show them wherever GEDTM30 is).
pub const COPERNICUS_NOTICE: &str = "GEDTM30 is built partly from the Copernicus DEM: © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.";

// ── Writing ──────────────────────────────────────────────────────────────────────────────────

/// Collects chunks and writes the file: chunks ordered by (kind, level, path), each deflated at
/// one fixed level, so the same input always writes the same bytes (the builder's byte-identical
/// rebuild check, section 8.2).
#[derive(Default)]
pub struct PackWriter {
    chunks: Vec<((ChunkKind, u8, u64), Vec<u8>)>,
}

impl PackWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one chunk's raw bytes (an empty chunk is skipped: absence means empty).
    pub fn add(&mut self, kind: ChunkKind, level: u8, path: u64, raw: Vec<u8>) {
        if !raw.is_empty() {
            self.chunks.push(((kind, level, path), raw));
        }
    }

    /// A content hash of every chunk (FNV-1a 64 over the kind, level, path and raw bytes, in
    /// order): the manifest's `build_id`.
    pub fn content_hash(&mut self) -> String {
        self.chunks.sort_by_key(|c| c.0);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for ((k, l, p), raw) in &self.chunks {
            eat(&[*k as u8, *l]);
            eat(&p.to_le_bytes());
            eat(&(raw.len() as u64).to_le_bytes());
            eat(raw);
        }
        format!("{h:016x}")
    }

    /// The file's bytes.
    pub fn finish(mut self, manifest: &PackManifest) -> Result<Vec<u8>, String> {
        self.chunks.sort_by_key(|c| c.0);
        let mtext = ron::ser::to_string_pretty(manifest, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
        let stored: Vec<Vec<u8>> = self.chunks.iter().map(|(_, raw)| miniz_oxide::deflate::compress_to_vec_zlib(raw, 8)).collect();
        let mut out = Vec::new();
        out.extend_from_slice(PACK_MAGIC);
        out.extend_from_slice(&(mtext.len() as u32).to_le_bytes());
        out.extend_from_slice(mtext.as_bytes());
        out.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        let mut offset = (out.len() + self.chunks.len() * ENTRY_LEN) as u64;
        for (((k, l, p), raw), s) in self.chunks.iter().zip(&stored) {
            out.push(*k as u8);
            out.push(*l);
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(&p.to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            offset += s.len() as u64;
        }
        for s in stored {
            out.extend_from_slice(&s);
        }
        Ok(out)
    }
}

// ── Names ────────────────────────────────────────────────────────────────────────────────────

/// A pack's file name: `f<face>-<path digits>.hmpack`, one digit (0 to 3) per level from the root
/// face down, e.g. `f07-213301.hmpack` for a depth-6 tile of face 7.
pub fn pack_file_name(id: &PatchId) -> String {
    let digits: String = (0..id.depth).map(|l| char::from(b'0' + ((id.path >> (2 * l as u32)) & 3) as u8)).collect();
    format!("f{:02}-{digits}.{PACK_EXT}", id.face)
}

/// [`pack_file_name`]'s inverse, for a name with or without its extension.
pub fn parse_pack_file_name(name: &str) -> Option<PatchId> {
    let stem = name.strip_suffix(&format!(".{PACK_EXT}")).unwrap_or(name);
    let (face, digits) = stem.strip_prefix('f')?.split_once('-')?;
    let face: u8 = face.parse().ok().filter(|f| *f < 20)?;
    if digits.len() > 31 {
        return None;
    }
    let mut path = 0u64;
    for (l, ch) in digits.chars().enumerate() {
        let d = ch.to_digit(4)? as u64;
        path |= d << (2 * l);
    }
    Some(PatchId { face, depth: digits.len() as u8, path })
}

#[cfg(test)]
#[path = "map_pack_tests.rs"]
mod tests;
