//! THE MAP PACK BUILDER (whole-Earth maps rung 2, 2026-11-09; docs/design/whole-earth-maps.md
//! section 8.2): our own Rust tool, no Planetiler and no GDAL, that turns the sources on the
//! operator's data drive into map packs (`map_pack`, HOSMPAK1) keyed by the terrain's own patches.
//!
//! Inputs, read from disk and never run (untrusted files):
//! - OpenStreetMap: a PBF extract (Geofabrik's Washington, `pbf` and `osm`).
//! - GEDTM30 v1.2 land heights: the fetched window of the global file's own tiles (`gedtm`), with
//!   the SCALE trap pinned by a unit test.
//! - ESA WorldCover 2021 v200 land cover and NOAA ETOPO 2022 seabed: their GeoTIFFs (`geotiff`).
//!
//! For each pack (a depth-6 tile) the builder writes, per data level, the land lattices (level 6
//! and 9 as area means of the 30 m heights, level 12 as the heights themselves), the seabed
//! lattices (levels 6 and 9) and the land cover lattice (level 12), then per level-12 tile the
//! OSM water rings (the sea closed tile by tile, `coast`; lakes clipped), ways and buildings.
//! No layer is edited with another's data (section 3.4); the shore band joins them at run time.
//! It then checks what it wrote: the pack is built twice and must be byte-identical, every chunk
//! must decode, and each named place inside it must pass its geography gates (data/maps/
//! places.ron: a known town on land, a known water point wet, local names present).
//!
//! Developer tooling: `just map-packs` (the example `build_map_packs`); its in-app button is CLI
//! debt in docs/design/in-app-ops.md. Below normal priority with the share guard; a pack takes
//! tens of seconds.

pub mod coast;
pub mod gedtm;
pub mod geotiff;
pub mod osm;
pub mod pbf;

use crate::map_levels::{MapLevels, MapPlaces, Place};
use crate::map_pack::{self, ChunkKind, LayerInfo, MapBuilding, MapWay, PackManifest, PackWriter, WaterRing, WaterRingKind};
use crate::map_tiles::{self, clip_polyline, clip_ring, descendants_at, e7_to_dir, encode_e7, tile_containing, tile_lattice};
use crate::osm_region::dir_to_latlon_f64;
use crate::planet_chunks::{patch_corners, patch_edge_arc_m, PatchId};
use coast::{close_sea, CoastSeg, Piece};
use geotiff::{GeoTiff, TileCache};
use glam::DVec3;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The planet radius the lattice spacing is figured with (metres).
const R_EARTH: f64 = 6_371_000.0;
/// The spatial index's cell, degrees.
const CELL_DEG: f64 = 0.01;
/// OSM features are read this far past the packs' corners (degrees), so a feature crossing a
/// pack's edge is whole.
const BBOX_MARGIN_DEG: f64 = 0.06;

/// Where the sources are.
#[derive(Clone, Debug)]
pub struct Sources {
    pub osm: PathBuf,
    pub gedtm: PathBuf,
    pub worldcover: PathBuf,
    pub etopo: PathBuf,
}

impl Sources {
    /// The operator's data drive (whole-earth-maps.md section 14) and the repo's ETOPO folder.
    pub fn defaults(repo_root: &Path) -> Self {
        Sources {
            osm: PathBuf::from(r"E:\HumanityOS\maps-src\osm\washington-261108.osm.pbf"),
            gedtm: PathBuf::from(gedtm::WASHINGTON_WINDOW),
            worldcover: PathBuf::from(r"E:\HumanityOS\maps-src\worldcover"),
            etopo: repo_root.join("ext_data").join("etopo2022_15s"),
        }
    }
}

type Cell = (i32, i32);

fn cell_of(lat: f64, lon: f64) -> Cell {
    ((lat / CELL_DEG).floor() as i32, (lon / CELL_DEG).floor() as i32)
}

fn deg(p: (i32, i32)) -> (f64, f64) {
    (map_tiles::decode_e7(p.0), map_tiles::decode_e7(p.1))
}

/// A lat/lon box (south, north, west, east), degrees.
#[derive(Clone, Copy, Debug)]
pub struct Box2 {
    pub s: f64,
    pub n: f64,
    pub w: f64,
    pub e: f64,
}

impl Box2 {
    fn of_points(pts: impl Iterator<Item = (f64, f64)>) -> Self {
        let mut b = Box2 { s: f64::MAX, n: f64::MIN, w: f64::MAX, e: f64::MIN };
        for (la, lo) in pts {
            b.s = b.s.min(la);
            b.n = b.n.max(la);
            b.w = b.w.min(lo);
            b.e = b.e.max(lo);
        }
        b
    }
    fn of_tile(id: &PatchId, pad: f64) -> Self {
        let b = Self::of_points(patch_corners(id).iter().map(|c| dir_to_latlon_f64(*c)));
        Box2 { s: b.s - pad, n: b.n + pad, w: b.w - pad, e: b.e + pad }
    }
    fn cells(&self) -> impl Iterator<Item = Cell> {
        let (a, b) = (cell_of(self.s, self.w), cell_of(self.n, self.e));
        (a.0..=b.0).flat_map(move |y| (a.1..=b.1).map(move |x| (y, x)))
    }
}

/// Segment and feature lookups by lat/lon cell.
#[derive(Default)]
struct Index {
    coast: HashMap<Cell, Vec<(u32, u32)>>,
    roads: HashMap<Cell, Vec<(u32, u32)>>,
    water: HashMap<Cell, Vec<u32>>,
}

fn index_polyline(map: &mut HashMap<Cell, Vec<(u32, u32)>>, fi: usize, pts: &[(i32, i32)], closed: bool) {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for s in 0..segs {
        let b = Box2::of_points([deg(pts[s]), deg(pts[(s + 1) % n])].into_iter());
        for c in b.cells() {
            map.entry(c).or_default().push((fi as u32, s as u32));
        }
    }
}

/// Consecutive runs of candidate segment indices of one polyline: `(first seg, last seg)`, with a
/// closed chain's wrap through its first point joined into one run.
fn runs(mut segs: Vec<u32>, n_segs: u32, closed: bool) -> Vec<(u32, u32)> {
    segs.sort_unstable();
    segs.dedup();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for s in segs {
        match out.last_mut() {
            Some(r) if r.1 + 1 == s => r.1 = s,
            _ => out.push((s, s)),
        }
    }
    if closed && out.len() > 1 && out[0].0 == 0 && out.last().unwrap().1 == n_segs - 1 {
        let first = out.remove(0);
        out.last_mut().unwrap().1 = first.1 + n_segs;
    }
    out
}

/// The points of run `(a, b)` (segments a..=b, b may pass n for a wrapped closed chain).
fn run_points(pts: &[DVec3], (a, b): (u32, u32)) -> Vec<DVec3> {
    let n = pts.len() as u32;
    (a..=b + 1).map(|i| pts[(i % n) as usize]).collect()
}

/// Per-pack figures for the report and the plan's budgets.
#[derive(Default, Debug)]
pub struct PackStats {
    pub bytes: usize,
    pub seconds: f64,
    pub sea_rings: usize,
    pub lake_rings: usize,
    pub ways: usize,
    pub buildings: usize,
    pub open_coast_pieces: usize,
    pub walk_failures: usize,
}

/// The builder's state for one run: the sources opened and the OSM features indexed.
pub struct Builder {
    levels: MapLevels,
    osm: osm::OsmData,
    coast_dirs: Vec<Vec<DVec3>>,
    water_dirs: Vec<Vec<DVec3>>,
    road_dirs: Vec<Vec<DVec3>>,
    idx: Index,
    gedtm: gedtm::Gedtm,
    sources: Sources,
    worldcover: HashMap<String, Option<TileCache>>,
    etopo: HashMap<String, Option<TileCache>>,
    builder_name: String,
}

/// The lat/lon box holding every pack of `ids` plus the margin.
pub fn bbox_of(ids: &[PatchId]) -> Box2 {
    let b = Box2::of_points(ids.iter().flat_map(|id| patch_corners(id).into_iter().map(dir_to_latlon_f64)));
    Box2 { s: b.s - BBOX_MARGIN_DEG, n: b.n + BBOX_MARGIN_DEG, w: b.w - BBOX_MARGIN_DEG, e: b.e + BBOX_MARGIN_DEG }
}

impl Builder {
    /// Read the OSM extract for `bbox` and open the rasters.
    pub fn new(levels: MapLevels, sources: Sources, bbox: Box2, threads: usize, builder_name: String) -> Result<Self, String> {
        let t = Instant::now();
        let file = std::fs::read(&sources.osm).map_err(|e| format!("{}: {e}", sources.osm.display()))?;
        let osm = osm::extract(&file, osm::BboxE7::from_deg(bbox.s, bbox.n, bbox.w, bbox.e), &levels, threads)?;
        drop(file);
        println!("[map-build] OSM {} read in {:.1} s (snapshot {}):", sources.osm.display(), t.elapsed().as_secs_f64(), osm.snapshot);
        for (k, v) in &osm.stats {
            println!("[map-build]   {k}: {v}");
        }
        let mut idx = Index::default();
        for (i, c) in osm.coast.iter().enumerate() {
            index_polyline(&mut idx.coast, i, &c.points, c.closed);
        }
        for (i, r) in osm.roads.iter().enumerate() {
            index_polyline(&mut idx.roads, i, &r.points, false);
        }
        for (i, w) in osm.water.iter().enumerate() {
            for c in Box2::of_points(w.ring.iter().map(|p| deg(*p))).cells() {
                idx.water.entry(c).or_default().push(i as u32);
            }
        }
        let dirs = |pts: &[(i32, i32)]| pts.iter().map(|p| e7_to_dir(p.0, p.1)).collect::<Vec<_>>();
        Ok(Builder {
            coast_dirs: osm.coast.iter().map(|c| dirs(&c.points)).collect(),
            water_dirs: osm.water.iter().map(|w| dirs(&w.ring)).collect(),
            road_dirs: osm.roads.iter().map(|r| dirs(&r.points)).collect(),
            gedtm: gedtm::Gedtm::open(&sources.gedtm)?,
            levels,
            osm,
            idx,
            sources,
            worldcover: HashMap::new(),
            etopo: HashMap::new(),
            builder_name,
        })
    }

    fn raster<'a>(cache: &'a mut HashMap<String, Option<TileCache>>, dir: &Path, name: String) -> Result<Option<&'a mut TileCache>, String> {
        if !cache.contains_key(&name) {
            let path = dir.join(&name);
            let t = match std::fs::read(&path) {
                Ok(b) => Some(TileCache::new(GeoTiff::open(b).map_err(|e| format!("{}: {e}", path.display()))?)),
                Err(_) => None,
            };
            cache.insert(name.clone(), t);
        }
        Ok(cache.get_mut(&name).and_then(|c| c.as_mut()))
    }

    /// ETOPO 2022 15" at (lat, lon), bilinear; None outside the files on disk.
    fn etopo_at(&mut self, lat: f64, lon: f64) -> Result<Option<f64>, String> {
        let (nw_lat, nw_lon) = ((lat / 15.0).ceil() as i32 * 15, (lon / 15.0).floor() as i32 * 15);
        let ns = if nw_lat >= 0 { format!("N{nw_lat:02}") } else { format!("S{:02}", -nw_lat) };
        let ew = if nw_lon >= 0 { format!("E{nw_lon:03}") } else { format!("W{:03}", -nw_lon) };
        let dir = self.sources.etopo.clone();
        match Self::raster(&mut self.etopo, &dir, format!("ETOPO_2022_v1_15s_{ns}{ew}_surface.tif"))? {
            Some(t) => Ok(t.bilinear(lat, lon)?.map(|v| v as f64)),
            None => Ok(None),
        }
    }

    /// The ESA WorldCover class at (lat, lon): the most common of the 3 x 3 pixels around it.
    fn cover_at(&mut self, lat: f64, lon: f64) -> Result<u8, String> {
        let (sw_lat, sw_lon) = ((lat / 3.0).floor() as i32 * 3, (lon / 3.0).floor() as i32 * 3);
        let ns = if sw_lat >= 0 { format!("N{sw_lat:02}") } else { format!("S{:02}", -sw_lat) };
        let ew = if sw_lon >= 0 { format!("E{sw_lon:03}") } else { format!("W{:03}", -sw_lon) };
        let dir = self.sources.worldcover.clone();
        let Some(t) = Self::raster(&mut self.worldcover, &dir, format!("ESA_WorldCover_10m_2021_v200_{ns}{ew}_Map.tif"))? else {
            return Ok(0);
        };
        let Some((x, y)) = t.tiff.pixel_of(lat, lon) else { return Ok(0) };
        let mut counts: Vec<(u8, u8)> = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(v) = t.pixel(x + dx, y + dy)? {
                    let v = v as u8;
                    match counts.iter_mut().find(|c| c.0 == v) {
                        Some(c) => c.1 += 1,
                        None => counts.push((v, 1)),
                    }
                }
            }
        }
        // Most common, ties to the smaller class number (deterministic).
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(counts.first().map_or(0, |c| c.0))
    }

    /// Is the point in the sea, by the nearest coastline segment's side; ETOPO decides where no
    /// coastline is within half a degree.
    fn is_sea(&mut self, dir: DVec3) -> Result<bool, String> {
        let (lat, lon) = dir_to_latlon_f64(dir);
        let c = cell_of(lat, lon);
        let mut found_at = None;
        for r in 0..=50 {
            let any = (-r..=r).any(|dy: i32| (-r..=r).any(|dx: i32| (dy.abs() == r || dx.abs() == r) && self.idx.coast.contains_key(&(c.0 + dy, c.1 + dx))));
            if any {
                found_at = Some(r);
                break;
            }
        }
        let Some(r) = found_at else {
            return Ok(self.etopo_at(lat, lon)?.is_some_and(|h| h < 0.0));
        };
        let reach = r + 2;
        let mut segs = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                for &(ci, si) in self.idx.coast.get(&(c.0 + dy, c.1 + dx)).map(Vec::as_slice).unwrap_or(&[]) {
                    let (chain, d) = (&self.osm.coast[ci as usize], &self.coast_dirs[ci as usize]);
                    let n = d.len();
                    let (i, j) = (si as usize, (si as usize + 1) % n);
                    let prev = if i > 0 || chain.closed { Some(d[(i + n - 1) % n]) } else { None };
                    let next = if j + 1 < n || chain.closed { Some(d[(j + 1) % n]) } else { None };
                    segs.push(CoastSeg { a: d[i], b: d[j], prev, next });
                }
            }
        }
        Ok(coast::water_side(dir, &segs).unwrap_or(false))
    }

    /// One tile's land lattice in decimetres (`NODATA` outside the GEDTM30 window).
    fn land_lattice(&mut self, tile: &PatchId) -> Result<Vec<i32>, String> {
        let segs = self.levels.lattice_segments;
        let finest = tile.depth == self.levels.finest();
        let spacing = patch_edge_arc_m(tile.depth, R_EARTH) / segs as f64;
        let mut out = Vec::new();
        for d in tile_lattice(tile, segs).ok_or("lattice")? {
            let (lat, lon) = dir_to_latlon_f64(d);
            let h = if finest {
                self.gedtm.bilinear(lat, lon)?
            } else {
                let half_lat = spacing * 0.5 / 111_195.0;
                self.gedtm.box_mean(lat, lon, half_lat, half_lat / lat.to_radians().cos().max(0.05))?
            };
            out.push(h.map_or(map_pack::NODATA, map_pack::metres_to_dm));
        }
        Ok(out)
    }

    /// One tile's seabed lattice (ETOPO, the mean of nine samples over a coarse cell).
    fn sea_lattice(&mut self, tile: &PatchId) -> Result<Vec<i32>, String> {
        let segs = self.levels.lattice_segments;
        let spacing = patch_edge_arc_m(tile.depth, R_EARTH) / segs as f64;
        let coarse = spacing > 400.0;
        let mut out = Vec::new();
        for d in tile_lattice(tile, segs).ok_or("lattice")? {
            let (lat, lon) = dir_to_latlon_f64(d);
            let h = if coarse {
                let (dl, dn) = (spacing / 3.0 / 111_195.0, spacing / 3.0 / 111_195.0 / lat.to_radians().cos().max(0.05));
                let mut acc = Some(0.0);
                for i in -1..=1 {
                    for j in -1..=1 {
                        acc = match (acc, self.etopo_at(lat + i as f64 * dl, lon + j as f64 * dn)?) {
                            (Some(a), Some(v)) => Some(a + v / 9.0),
                            _ => None,
                        };
                    }
                }
                acc
            } else {
                self.etopo_at(lat, lon)?
            };
            out.push(h.map_or(map_pack::NODATA, map_pack::metres_to_dm));
        }
        Ok(out)
    }

    /// One level-12 tile's water rings.
    fn tile_water(&mut self, tile: &PatchId, st: &mut PackStats) -> Result<Vec<WaterRing>, String> {
        let c = patch_corners(tile);
        let bx = Box2::of_tile(tile, 0.0005);
        let mut by_chain: HashMap<u32, Vec<u32>> = HashMap::new();
        for cell in bx.cells() {
            for &(ci, si) in self.idx.coast.get(&cell).map(Vec::as_slice).unwrap_or(&[]) {
                by_chain.entry(ci).or_default().push(si);
            }
        }
        let mut chains: Vec<u32> = by_chain.keys().copied().collect();
        chains.sort_unstable();
        let (mut pieces, mut islands) = (Vec::new(), Vec::new());
        for ci in chains {
            let chain = &self.osm.coast[ci as usize];
            let pts = &self.coast_dirs[ci as usize];
            let n_segs = if chain.closed { pts.len() } else { pts.len() - 1 } as u32;
            for run in runs(by_chain.remove(&ci).unwrap_or_default(), n_segs, chain.closed) {
                let line = run_points(pts, run);
                let whole = chain.closed && run.1 - run.0 + 1 == n_segs;
                let mut got: Vec<Piece> = clip_polyline(tile, &line).into_iter().map(|p| Piece { pts: p.points, start_clipped: p.start_clipped, end_clipped: p.end_clipped }).collect();
                // A whole closed chain starts and ends at its first point: where that point is
                // inside the tile but the chain leaves it, the last piece runs on into the first.
                if whole && got.len() > 1 && !got[0].start_clipped && !got[got.len() - 1].end_clipped {
                    let first = got.remove(0);
                    let last = got.last_mut().unwrap();
                    last.pts.extend_from_slice(&first.pts[1..]);
                    last.end_clipped = first.end_clipped;
                }
                for p in got {
                    if whole && !p.start_clipped && !p.end_clipped {
                        islands.push(p.pts[..p.pts.len() - 1].to_vec());
                    } else {
                        pieces.push(p);
                    }
                }
            }
        }
        let mut rings: Vec<WaterRing> = Vec::new();
        let to_e7 = |d: &DVec3| {
            let (la, lo) = dir_to_latlon_f64(*d);
            (encode_e7(la), encode_e7(lo))
        };
        let push = |kind: WaterRingKind, feature: u64, name: Option<String>, pts: &[DVec3], clip: &[bool], rings: &mut Vec<WaterRing>| {
            // To OSM integers, dropping repeated points (a zero-length edge takes the next one's flag).
            let mut p: Vec<(i32, i32)> = Vec::new();
            let mut f: Vec<bool> = Vec::new();
            for (i, d) in pts.iter().enumerate() {
                let q = to_e7(d);
                if p.last() == Some(&q) {
                    *f.last_mut().unwrap() = clip[i];
                    continue;
                }
                p.push(q);
                f.push(clip[i]);
            }
            while p.len() > 1 && p.first() == p.last() {
                p.pop();
                f.pop();
            }
            if p.len() >= 3 {
                rings.push(WaterRing { kind, feature, name, points: p, clip: f });
            }
        };
        let crossing = pieces.iter().filter(|p| p.start_clipped && p.end_clipped).count();
        st.open_coast_pieces += pieces.len() - crossing;
        for p in pieces.iter().filter(|p| !(p.start_clipped && p.end_clipped)) {
            let end = if p.start_clipped { *p.pts.last().unwrap() } else { p.pts[0] };
            let (la, lo) = dir_to_latlon_f64(end);
            println!("[map-build]   a coastline ends inside tile {}/{} at {la:.6}, {lo:.6} (an open chain: the extract's edge, or a gap in OSM's coastline)", tile.depth, tile.path);
        }
        let centre = (c[0] + c[1] + c[2]).normalize();
        let mut sea_whole = false;
        if crossing > 0 {
            match close_sea(&c, &pieces) {
                Ok((sea, _)) => {
                    for (pts, clip) in sea {
                        push(WaterRingKind::Sea, 0, None, &pts, &clip, &mut rings);
                    }
                }
                Err(e) => {
                    st.walk_failures += 1;
                    eprintln!("[map-build] tile {:?}: {e}; its sea is decided by its centre", tile);
                    sea_whole = self.is_sea(centre)?;
                }
            }
        } else {
            sea_whole = self.is_sea(centre)?;
        }
        if sea_whole {
            push(WaterRingKind::Sea, 0, None, &c, &[true, true, true], &mut rings);
        }
        for isl in islands {
            // OSM puts land on the left: a counter-clockwise closed chain is an island, a
            // clockwise one a sea enclosed by coast. Reversed, the water is on the left either way.
            let ccw = map_tiles::ring_area_sr(&isl, centre) > 0.0;
            let rev: Vec<DVec3> = isl.into_iter().rev().collect();
            let flags = vec![false; rev.len()];
            push(if ccw { WaterRingKind::Island } else { WaterRingKind::Sea }, 0, None, &rev, &flags, &mut rings);
        }
        st.sea_rings += rings.len();
        // Lakes and rivers.
        let mut cand: Vec<u32> = bx.cells().filter_map(|cl| self.idx.water.get(&cl)).flatten().copied().collect();
        cand.sort_unstable();
        cand.dedup();
        let debug_name = std::env::var("HUMANITY_MAP_DEBUG_NAME").ok();
        for wi in cand {
            let w = &self.osm.water[wi as usize];
            if debug_name.is_some() && w.name == debug_name {
                println!("[map-debug] tile {} candidate {} {:?}: clip {:?}", tile.path, w.id, w.name, clip_ring(tile, &self.water_dirs[wi as usize]).map(|c| c.points.len()));
            }
            if let Some(cr) = clip_ring(tile, &self.water_dirs[wi as usize]) {
                let kind = if w.inner {
                    WaterRingKind::Island
                } else if w.kind == osm::AreaKind::River {
                    WaterRingKind::River
                } else {
                    WaterRingKind::Lake
                };
                let before = rings.len();
                push(kind, w.id as u64, w.name.clone(), &cr.points, &cr.clip_edge, &mut rings);
                st.lake_rings += rings.len() - before;
            }
        }
        Ok(rings)
    }

    /// One level-12 tile's ways.
    fn tile_ways(&self, tile: &PatchId) -> Vec<MapWay> {
        let bx = Box2::of_tile(tile, 0.0005);
        let mut by_road: HashMap<u32, Vec<u32>> = HashMap::new();
        for cell in bx.cells() {
            for &(ri, si) in self.idx.roads.get(&cell).map(Vec::as_slice).unwrap_or(&[]) {
                by_road.entry(ri).or_default().push(si);
            }
        }
        let mut ids: Vec<u32> = by_road.keys().copied().collect();
        ids.sort_unstable();
        let mut out = Vec::new();
        for ri in ids {
            let r = &self.osm.roads[ri as usize];
            let pts = &self.road_dirs[ri as usize];
            for run in runs(by_road.remove(&ri).unwrap_or_default(), pts.len() as u32 - 1, false) {
                for p in clip_polyline(tile, &run_points(pts, run)) {
                    let mut q: Vec<(i32, i32)> = p.points.iter().map(|d| {
                        let (la, lo) = dir_to_latlon_f64(*d);
                        (encode_e7(la), encode_e7(lo))
                    }).collect();
                    q.dedup();
                    if q.len() >= 2 {
                        out.push(MapWay {
                            osm_id: r.id as u64,
                            class: r.class,
                            bridge: r.bridge,
                            tunnel: r.tunnel,
                            layer: r.layer,
                            width_m: r.width_m,
                            name: r.name.clone(),
                            points: q,
                            start_clipped: p.start_clipped,
                            end_clipped: p.end_clipped,
                        });
                    }
                }
            }
        }
        out
    }

    /// Build one pack's bytes.
    pub fn build_pack(&mut self, pack: PatchId, layers: &[LayerInfo], notices: &[String]) -> Result<(Vec<u8>, PackStats), String> {
        let t = Instant::now();
        let mut st = PackStats::default();
        let mut w = PackWriter::new();
        let levels = self.levels.levels.clone();
        let finest = self.levels.finest();
        // Heights and cover.
        for &l in &levels {
            for tile in descendants_at(pack, l) {
                let land = self.land_lattice(&tile)?;
                w.add(ChunkKind::LandH, l, tile.path, map_pack::encode_lattice(&land));
                if self.levels.sea_levels.contains(&l) {
                    let sea = self.sea_lattice(&tile)?;
                    w.add(ChunkKind::SeaH, l, tile.path, map_pack::encode_lattice(&sea));
                }
                if l == finest {
                    let mut cover = Vec::new();
                    for d in tile_lattice(&tile, self.levels.lattice_segments).ok_or("lattice")? {
                        let (lat, lon) = dir_to_latlon_f64(d);
                        cover.push(self.cover_at(lat, lon)?);
                    }
                    w.add(ChunkKind::Cover, l, tile.path, cover);
                }
            }
            self.gedtm.clear();
        }
        // Buildings by the level-12 tile holding their first point.
        let mut bld: HashMap<u64, Vec<MapBuilding>> = HashMap::new();
        for b in &self.osm.buildings {
            let (la, lo) = deg(b.ring[0]);
            let tile = tile_containing(la, lo, finest);
            if map_tiles::ancestor_at(tile, pack.depth) == Some(pack) {
                bld.entry(tile.path).or_default().push(MapBuilding { osm_id: b.id as u64, height_m: b.height_m, min_height_m: b.min_height_m, levels: b.levels, ring: b.ring.clone() });
            }
        }
        for tile in descendants_at(pack, finest) {
            let water = self.tile_water(&tile, &mut st)?;
            w.add(ChunkKind::Water, finest, tile.path, if water.is_empty() { Vec::new() } else { map_pack::encode_water(&water) });
            let ways = self.tile_ways(&tile);
            st.ways += ways.len();
            w.add(ChunkKind::Ways, finest, tile.path, if ways.is_empty() { Vec::new() } else { map_pack::encode_ways(&ways) });
            if let Some(bs) = bld.remove(&tile.path) {
                st.buildings += bs.len();
                w.add(ChunkKind::Buildings, finest, tile.path, map_pack::encode_buildings(&bs));
            }
        }
        let manifest = PackManifest {
            format: "HOSMPAK1".into(),
            spec: "docs/design/whole-earth-maps.md (sections 3, 9.2) and crates/humanity-terrain/src/map_pack.rs in https://github.com/Shaostoul/Humanity".into(),
            tile: (pack.face, pack.depth, pack.path),
            build_id: w.content_hash(),
            builder: self.builder_name.clone(),
            levels,
            lattice_segments: self.levels.lattice_segments,
            layers: layers.to_vec(),
            notices: notices.to_vec(),
        };
        let bytes = w.finish(&manifest)?;
        st.bytes = bytes.len();
        st.seconds = t.elapsed().as_secs_f64();
        Ok((bytes, st))
    }

    /// The OSM snapshot date.
    pub fn osm_snapshot(&self) -> &str {
        &self.osm.snapshot
    }
}

/// Every layer's provenance for this build's sources.
pub fn layers(osm_snapshot: &str, sources: &Sources) -> Vec<LayerInfo> {
    let osm = |name: &str| LayerInfo {
        name: name.into(),
        source: format!("OpenStreetMap, from the Geofabrik extract {}", sources.osm.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        snapshot: osm_snapshot.into(),
        licence: "ODbL-1.0".into(),
        licence_url: "https://opendatacommons.org/licenses/odbl/1-0/".into(),
        notice: crate::map_pack::OSM_NOTICE.into(),
    };
    vec![
        osm("osm_water"),
        osm("osm_ways"),
        osm("osm_buildings"),
        LayerInfo {
            name: "land_h".into(),
            source: "GEDTM30 v1.2, Global Ensemble Digital Terrain Model 30 m (OpenGeoHub), https://zenodo.org/records/18887460".into(),
            snapshot: "2026-03-06".into(),
            licence: "CC-BY-4.0".into(),
            licence_url: "https://creativecommons.org/licenses/by/4.0/".into(),
            notice: crate::map_pack::GEDTM30_NOTICE.into(),
        },
        LayerInfo {
            name: "sea_h".into(),
            source: "ETOPO 2022 15 Arc-Second Global Relief Model (NOAA NCEI), DOI 10.25921/fd45-gt74".into(),
            snapshot: "2022".into(),
            licence: "CC0-1.0".into(),
            licence_url: "https://creativecommons.org/publicdomain/zero/1.0/".into(),
            notice: "NOAA National Centers for Environmental Information. 2022: ETOPO 2022 15 Arc-Second Global Relief Model. DOI: 10.25921/fd45-gt74".into(),
        },
        LayerInfo {
            name: "cover".into(),
            source: "ESA WorldCover 10 m 2021 v200, https://esa-worldcover.org".into(),
            snapshot: "2021".into(),
            licence: "CC-BY-4.0".into(),
            licence_url: "https://creativecommons.org/licenses/by/4.0/".into(),
            notice: crate::map_pack::WORLDCOVER_NOTICE.into(),
        },
    ]
}

/// The notices every pack carries (whole-earth-maps.md 9.2; finding 2026-11-08).
pub fn notices() -> Vec<String> {
    vec![
        "OpenStreetMap layers (osm_water, osm_ways, osm_buildings): Map data (c) OpenStreetMap contributors. This pack's OpenStreetMap layers are made available under the Open Database License (ODbL) 1.0, https://opendatacommons.org/licenses/odbl/1-0/ ; see https://www.openstreetmap.org/copyright . The format is documented at the spec named in this manifest, so the pack itself is the ODbL's offer of the derived database, free.".into(),
        "The other layers are separate databases, never edited with the OpenStreetMap data: land_h and cover under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/), sea_h under CC0 1.0.".into(),
        crate::map_pack::COPERNICUS_NOTICE.into(),
    ]
}

/// The packs any part of a place's box falls in (Silverdale's old rectangle straddles two).
pub fn place_packs(p: &Place, depth: u8) -> Vec<PatchId> {
    let (dlat, dlon) = (p.half_north_m / 110_540.0, p.half_east_m / (111_320.0 * p.lat.to_radians().cos()));
    let mut v = Vec::new();
    for i in 0..=20 {
        for j in 0..=20 {
            let t = tile_containing(p.lat - dlat + 2.0 * dlat * i as f64 / 20.0, p.lon - dlon + 2.0 * dlon * j as f64 / 20.0, depth);
            if !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// The packs holding the home places' boxes (data/maps/places.ron's `home: true`).
pub fn home_packs(places: &MapPlaces, levels: &MapLevels) -> Vec<PatchId> {
    let mut v: Vec<PatchId> = places.places.iter().filter(|p| p.home).flat_map(|p| place_packs(p, levels.local_pack_depth)).collect();
    v.sort_by_key(|p| (p.face, p.path));
    v.dedup();
    v
}

/// The packs wholly inside the GEDTM30 window (`--tiles washington`).
pub fn window_packs(window: (f64, f64, f64, f64), levels: &MapLevels) -> Vec<PatchId> {
    let (s, n, w, e) = window;
    let mut out = Vec::new();
    let step = 0.05;
    let mut lat = s;
    while lat <= n {
        let mut lon = w;
        while lon <= e {
            let id = tile_containing(lat, lon, levels.local_pack_depth);
            if !out.contains(&id) && patch_corners(&id).iter().map(|c| dir_to_latlon_f64(*c)).all(|(la, lo)| la >= s && la <= n && lo >= w && lo <= e) {
                out.push(id);
            }
            lon += step;
        }
        lat += step;
    }
    out.sort_by_key(|p| (p.face, p.path));
    out
}

/// A place's gates against the packs holding its box. Returns (passed, failed) lines.
pub fn check_place(packs: &[map_pack::Pack], place: &Place) -> (Vec<String>, Vec<String>) {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    let mut check = |good: bool, what: String| if good { ok.push(what) } else { bad.push(what) };
    let Some(first) = packs.first() else { return (ok, vec![format!("{}: no pack", place.name)]) };
    let finest = map_pack::local_level(first);
    let pack_of = |t: &PatchId| packs.iter().find(|p| map_tiles::ancestor_at(*t, p.patch().depth) == Some(p.patch()));
    let water = |t: &PatchId| pack_of(t).and_then(|p| p.water(t.path)).unwrap_or_default();
    let class_at = |la: f64, lo: f64| -> Option<WaterRingKind> {
        let tile = tile_containing(la, lo, finest);
        map_pack::point_class(&water(&tile), crate::osm_region::latlon_to_dir_f64(la, lo)).map(|r| r.kind)
    };
    for &(la, lo) in &place.gates.sea_at {
        let c = class_at(la, lo);
        check(c == Some(WaterRingKind::Sea), format!("{}: {la}, {lo} is sea (reads {c:?})", place.name));
    }
    for &(la, lo) in &place.gates.land_at {
        let c = class_at(la, lo);
        check(c.is_none(), format!("{}: {la}, {lo} is land (reads {c:?})", place.name));
    }
    for &(la, lo) in &place.gates.lake_at {
        let c = class_at(la, lo);
        check(c == Some(WaterRingKind::Lake), format!("{}: {la}, {lo} is a lake (reads {c:?})", place.name));
    }
    // Counts and names over the level-12 tiles the place's box touches.
    let m_lat = 110_540.0;
    let m_lon = 111_320.0 * place.lat.to_radians().cos();
    let (dlat, dlon) = (place.half_north_m / m_lat, place.half_east_m / m_lon);
    let mut tiles: Vec<PatchId> = Vec::new();
    let mut la = place.lat - dlat;
    while la <= place.lat + dlat {
        let mut lo = place.lon - dlon;
        while lo <= place.lon + dlon {
            let t = tile_containing(la, lo, finest);
            if !tiles.contains(&t) {
                tiles.push(t);
            }
            lo += 0.004;
        }
        la += 0.004;
    }
    let inside = |p: (i32, i32)| {
        let (a, o) = deg(p);
        (a - place.lat).abs() <= dlat && (o - place.lon).abs() <= dlon
    };
    let (mut roads, mut blds, mut road_names, mut water_names) = (std::collections::HashSet::new(), 0usize, Vec::new(), Vec::new());
    for t in &tiles {
        let Some(pack) = pack_of(t) else { continue };
        for w in pack.ways(t.path).unwrap_or_default() {
            if w.points.iter().any(|p| inside(*p)) {
                roads.insert(w.osm_id);
                road_names.extend(w.name);
            }
        }
        blds += pack.buildings(t.path).unwrap_or_default().iter().filter(|b| inside(b.ring[0])).count();
        water_names.extend(water(t).into_iter().filter_map(|r| r.name));
    }
    check(roads.len() >= place.gates.min_roads, format!("{}: {} roads (at least {})", place.name, roads.len(), place.gates.min_roads));
    check(blds >= place.gates.min_buildings, format!("{}: {blds} buildings (at least {})", place.name, place.gates.min_buildings));
    if !place.gates.road_named_any.is_empty() {
        check(place.gates.road_named_any.iter().any(|n| road_names.contains(n)), format!("{}: a road named one of {:?}", place.name, place.gates.road_named_any));
    }
    if !place.gates.water_named_any.is_empty() {
        let found = place.gates.water_named_any.iter().any(|n| water_names.contains(n));
        let mut elsewhere = String::new();
        if !found {
            // Say where it is, if anywhere in the packs, so a gate on a too-small box is obvious.
            for pack in packs {
                for e in pack.entries().filter(|e| e.kind == ChunkKind::Water) {
                    for r in pack.water(e.path).unwrap_or_default() {
                        if r.name.as_ref().is_some_and(|n| place.gates.water_named_any.contains(n)) && elsewhere.is_empty() {
                            let (la, lo) = deg(r.points[0]);
                            elsewhere = format!(" (found outside the box at {la:.5}, {lo:.5})");
                        }
                    }
                }
            }
        }
        check(found, format!("{}: water named one of {:?}{elsewhere}", place.name, place.gates.water_named_any));
    }
    (ok, bad)
}

/// The command line: `build_map_packs [--tiles home|washington|<pack names>] [--out <dir>]
/// [--osm <pbf>] [--gedtm <dir>] [--worldcover <dir>] [--etopo <dir>] [--threads n] [--once]`.
pub fn run(args: &[String], repo_root: &Path) -> Result<(), String> {
    let mut sources = Sources::defaults(repo_root);
    let (mut tiles_arg, mut out, mut threads, mut twice) = ("home".to_string(), repo_root.join("data").join("maps").join("packs"), 4usize, true);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--tiles" => tiles_arg = val()?,
            "--out" => out = PathBuf::from(val()?),
            "--osm" => sources.osm = PathBuf::from(val()?),
            "--gedtm" => sources.gedtm = PathBuf::from(val()?),
            "--worldcover" => sources.worldcover = PathBuf::from(val()?),
            "--etopo" => sources.etopo = PathBuf::from(val()?),
            "--threads" => threads = val()?.parse().map_err(|e| format!("--threads: {e}"))?,
            "--once" => twice = false,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let data = repo_root.join("data");
    let levels = MapLevels::load(&data)?;
    let places = MapPlaces::load(&data)?;
    let gedtm_window = gedtm::Gedtm::open(&sources.gedtm)?.window;
    let packs: Vec<PatchId> = match tiles_arg.as_str() {
        "home" => home_packs(&places, &levels),
        "washington" => window_packs(gedtm_window, &levels),
        list => list.split(',').map(|n| map_pack::parse_pack_file_name(n.trim()).ok_or_else(|| format!("{n} is not a pack name (f07-213301)"))).collect::<Result<_, _>>()?,
    };
    println!("[map-build] {} pack(s): {}", packs.len(), packs.iter().map(map_pack::pack_file_name).collect::<Vec<_>>().join(", "));
    if let Some(spec) = std::env::var("HUMANITY_MAP_INSPECT").ok() {
        // Dev aid: list the coastline chains ending near "lat,lon" (a coastline that will not close).
        let ll: Vec<f64> = spec.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let b = Builder::new(MapLevels::load(&repo_root.join("data"))?, sources.clone(), bbox_of(&packs), threads, String::new())?;
        let p = crate::osm_region::latlon_to_dir_f64(ll[0], ll[1]);
        for (i, c) in b.osm.coast.iter().enumerate() {
            let d = &b.coast_dirs[i];
            let near = d.iter().map(|q| (*q - p).length() * R_EARTH).fold(f64::MAX, f64::min);
            if near < 500.0 {
                let (f, l) = (deg(c.points[0]), deg(*c.points.last().unwrap()));
                println!("[map-inspect] chain {} closed {} points {} nearest {near:.1} m first {:.7},{:.7} last {:.7},{:.7}", c.id, c.closed, c.points.len(), f.0, f.1, l.0, l.1);
            }
        }
        for w in &b.osm.water {
            let near = w.ring.iter().map(|q| (e7_to_dir(q.0, q.1) - p).length() * R_EARTH).fold(f64::MAX, f64::min);
            if near < 3000.0 {
                println!("[map-inspect] water {} {:?} inner {} {:?} points {} nearest {near:.0} m", w.id, w.kind, w.inner, w.name, w.ring.len());
            }
        }
        let tile = tile_containing(ll[0], ll[1], 12);
        for pack in &packs {
            if let Ok(bytes) = std::fs::read(out.join(map_pack::pack_file_name(pack))) {
                let pk = map_pack::Pack::open(bytes)?;
                for r in pk.water(tile.path).unwrap_or_default() {
                    println!("[map-inspect] pack tile {} ring {:?} feature {} {:?} points {}", tile.path, r.kind, r.feature, r.name, r.points.len());
                }
            }
        }
        return Ok(());
    }
    let total = Instant::now();
    let name = format!("build_map_packs (humanity-terrain {})", env!("CARGO_PKG_VERSION"));
    let mut b = Builder::new(levels, sources.clone(), bbox_of(&packs), threads, name)?;
    let layers = layers(b.osm_snapshot(), &sources);
    let notices = notices();
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut failed = 0;
    let mut built: Vec<map_pack::Pack> = Vec::new();
    for pack in &packs {
        let (bytes, st) = b.build_pack(*pack, &layers, &notices)?;
        if twice {
            let (again, _) = b.build_pack(*pack, &layers, &notices)?;
            if again != bytes {
                return Err(format!("{}: two builds from the same sources differ (the byte-identical rebuild check)", map_pack::pack_file_name(pack)));
            }
        }
        let opened = map_pack::Pack::open(bytes.clone())?;
        let mut undecodable = 0;
        for e in opened.entries() {
            let ok = match e.kind {
                ChunkKind::LandH | ChunkKind::SeaH => opened.heights(e.kind, e.level, e.path).is_some(),
                ChunkKind::Cover => opened.chunk(e.kind, e.level, e.path).is_some_and(|c| c.len() == map_tiles::lattice_len(64)),
                ChunkKind::Water => opened.water(e.path).is_some(),
                ChunkKind::Ways => opened.ways(e.path).is_some(),
                ChunkKind::Buildings => opened.buildings(e.path).is_some(),
            };
            undecodable += (!ok) as usize;
        }
        let path = out.join(map_pack::pack_file_name(pack));
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "[map-build] {} : {:.1} MB in {:.1} s{} ({} sea rings, {} lake rings, {} ways, {} buildings; {} coastline pieces ended inside a tile; {} border walks failed; {} chunks undecodable)",
            path.display(),
            st.bytes as f64 / 1e6,
            st.seconds,
            if twice { ", built twice, byte-identical" } else { "" },
            st.sea_rings,
            st.lake_rings,
            st.ways,
            st.buildings,
            st.open_coast_pieces,
            st.walk_failures,
            undecodable
        );
        for kind in [ChunkKind::LandH, ChunkKind::SeaH, ChunkKind::Cover, ChunkKind::Water, ChunkKind::Ways, ChunkKind::Buildings] {
            let bytes: u64 = opened.entries().filter(|e| e.kind == kind).map(|e| e.stored_len() as u64).sum();
            println!("[map-build]   {kind:?}: {} chunks, {:.2} MB", opened.count(kind), bytes as f64 / 1e6);
        }
        failed += undecodable;
        built.push(opened);
    }
    // Each named place whose packs were all built, against them together.
    for place in &places.places {
        let need = place_packs(place, built.first().map_or(6, |p| p.patch().depth));
        if !need.iter().all(|n| built.iter().any(|b| b.patch() == *n)) {
            continue;
        }
        let (ok, bad) = check_place(&built, place);
        for l in ok {
            println!("[map-build]   PASS {l}");
        }
        for l in &bad {
            println!("[map-build]   FAIL {l}");
        }
        failed += bad.len();
    }
    println!("[map-build] done in {:.1} s", total.elapsed().as_secs_f64());
    if failed > 0 {
        return Err(format!("{failed} check(s) failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Candidate segments group into runs, and a closed chain's wrap is one run. Seen red
    /// 2026-11-09 without the wrap join: "a closed chain's run through its first point is one run;
    /// left: [(0, 1), (8, 9)]".
    #[test]
    fn runs_join_a_closed_chains_wrap() {
        assert_eq!(runs(vec![3, 1, 2, 7], 10, false), vec![(1, 3), (7, 7)]);
        assert_eq!(runs(vec![0, 1, 8, 9], 10, true), vec![(8, 11)], "a closed chain's run through its first point is one run");
        let pts: Vec<DVec3> = (0..10).map(|i| DVec3::new(i as f64, 1.0, 0.0)).collect();
        let p = run_points(&pts, (8, 11));
        assert_eq!(p.iter().map(|v| v.x as i32).collect::<Vec<_>>(), vec![8, 9, 0, 1, 2]);
        assert_eq!(runs(vec![0, 1, 8, 9], 10, false), vec![(0, 1), (8, 9)], "an open chain does not wrap");
    }

    /// The home packs are the depth-6 tiles holding Silverdale and Seattle Center.
    #[test]
    fn the_home_packs_hold_the_home_places() {
        let data = Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
        let (levels, places) = (MapLevels::load(&data).unwrap(), MapPlaces::load(&data).unwrap());
        let home = home_packs(&places, &levels);
        assert!(!home.is_empty() && home.len() <= 2);
        for p in places.places.iter().filter(|p| p.home) {
            assert!(home.contains(&tile_containing(p.lat, p.lon, 6)), "{} is in a home pack", p.name);
        }
    }
}
