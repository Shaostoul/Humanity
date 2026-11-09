//! The OpenStreetMap features a map pack carries, pulled out of a PBF extract (whole-Earth maps
//! rung 2): the coastline stitched into chains, water areas (closed ways and multipolygons) as
//! rings, roads, and building footprints. Three passes over the file, each inflating its blobs on
//! worker threads and merging in file order, so the result never depends on thread timing:
//! nodes inside the bounding box, then the water relations' members, then the ways.
//!
//! The rules ported from the retired `scripts/fetch-osm-region.mjs` (its self-checks live on in
//! the builder's gates): coastline ways are stitched on shared end NODE IDS and never reversed
//! (the direction is the data: land on the left); a closed coastline chain is an island; water
//! areas are `natural=water` and `waterway=riverbank`; a building's height is its `height` tag,
//! else `building:levels` times 3 m, else 0 (unknown); heights over 1,000 m are data errors.

use super::pbf::{self, tag, Want};
use crate::map_levels::MapLevels;
use std::collections::{HashMap, HashSet};

/// Heights over this are data errors (the tallest building is about 828 m).
const MAX_BUILDING_HEIGHT_M: f32 = 1000.0;
/// OSM's own default when only a floor count is known.
const METRES_PER_LEVEL: f32 = 3.0;

/// A coastline chain: ways stitched end to end in their own direction (land on the left).
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    /// The smallest way id in it (the chain's sort key, for byte-identical rebuilds).
    pub id: i64,
    pub points: Vec<(i32, i32)>,
    /// The chain returns to its first node (an island, or a closed sea).
    pub closed: bool,
}

/// What a water ring outlines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaKind {
    Lake,
    River,
}

/// One ring of a water area, oriented: an outer ring counter-clockwise (water on its left), an
/// inner ring (an island in it) clockwise.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterArea {
    /// The way or relation id (rings of one multipolygon share it).
    pub id: i64,
    pub kind: AreaKind,
    pub inner: bool,
    pub name: Option<String>,
    pub ring: Vec<(i32, i32)>,
}

/// One road.
#[derive(Clone, Debug, PartialEq)]
pub struct Road {
    pub id: i64,
    pub class: u8,
    pub bridge: bool,
    pub tunnel: bool,
    pub layer: i8,
    pub width_m: f32,
    pub name: Option<String>,
    pub points: Vec<(i32, i32)>,
}

/// One building footprint (outer ring, open).
#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    pub id: i64,
    pub height_m: f32,
    pub min_height_m: f32,
    pub levels: u8,
    pub ring: Vec<(i32, i32)>,
}

/// Everything extracted, each list in ascending id order.
#[derive(Default)]
pub struct OsmData {
    /// The extract's replication timestamp, ISO 8601 (UTC).
    pub snapshot: String,
    pub coast: Vec<Chain>,
    pub water: Vec<WaterArea>,
    pub roads: Vec<Road>,
    pub buildings: Vec<Building>,
    /// Counts for the build report.
    pub stats: Vec<(String, usize)>,
}

/// The bounding box kept, in OSM integers: (south, north, west, east).
#[derive(Clone, Copy, Debug)]
pub struct BboxE7 {
    pub s: i32,
    pub n: i32,
    pub w: i32,
    pub e: i32,
}

impl BboxE7 {
    pub fn from_deg(s: f64, n: f64, w: f64, e: f64) -> Self {
        let q = crate::map_tiles::encode_e7;
        BboxE7 { s: q(s), n: q(n), w: q(w), e: q(e) }
    }
    fn holds(&self, lat: i32, lon: i32) -> bool {
        lat >= self.s && lat <= self.n && lon >= self.w && lon <= self.e
    }
}

/// Seconds since 1970 to "YYYY-MM-DDTHH:MM:SSZ" (Howard Hinnant's civil-from-days).
pub fn iso8601(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Run `f` over every OSMData blob on `threads` workers, results in file order.
fn each_block<T: Send>(blobs: &[pbf::RawBlob], threads: usize, want: Want, f: impl Fn(pbf::Block) -> T + Sync) -> Result<Vec<T>, String> {
    let chunk = blobs.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = blobs
            .chunks(chunk)
            .map(|part| {
                let f = &f;
                s.spawn(move || -> Result<Vec<T>, String> {
                    part.iter().filter(|b| b.kind == "OSMData").map(|b| Ok(f(pbf::decode_block(&pbf::inflate_blob(b.blob)?, want)?))).collect()
                })
            })
            .collect();
        let mut out = Vec::new();
        for h in handles {
            out.extend(h.join().map_err(|_| "a PBF worker panicked".to_string())??);
        }
        Ok(out)
    })
}

fn is_water_area(tags: &[(String, String)]) -> Option<AreaKind> {
    let water = tag(tags, "water");
    let river = matches!(water, Some("river" | "canal" | "stream" | "ditch" | "drain"));
    if tag(tags, "waterway") == Some("riverbank") {
        return Some(AreaKind::River);
    }
    if tag(tags, "natural") == Some("water") {
        return Some(if river { AreaKind::River } else { AreaKind::Lake });
    }
    None
}

fn parse_metres(v: &str) -> Option<f32> {
    let t = v.trim().split(';').next()?.trim();
    let num: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let x: f32 = num.parse().ok()?;
    let rest = t[num.len()..].trim();
    let m = if rest.starts_with("ft") || rest.starts_with('\'') { x * 0.3048 } else { x };
    (m.is_finite() && m > 0.0).then_some(m)
}

/// Signed area (counter-clockwise positive) in degree units, for orientation only.
fn ring_sign(r: &[(i32, i32)]) -> f64 {
    let n = r.len();
    (0..n).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        a.1 as f64 * b.0 as f64 - b.1 as f64 * a.0 as f64
    }).sum()
}

/// Orient a ring: counter-clockwise when `ccw`, else clockwise.
fn orient(mut r: Vec<(i32, i32)>, ccw: bool) -> Vec<(i32, i32)> {
    if (ring_sign(&r) > 0.0) != ccw {
        r.reverse();
    }
    r
}

/// Drop consecutive duplicates and the closing repeat.
fn tidy_ring(mut r: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    r.dedup();
    while r.len() > 1 && r.first() == r.last() {
        r.pop();
    }
    r
}

/// Join member ways into closed rings at shared end nodes (reversing as needed), as node ids.
fn join_rings(mut parts: Vec<Vec<i64>>) -> (Vec<Vec<i64>>, usize) {
    let mut rings = Vec::new();
    let mut open = 0;
    while let Some(mut cur) = parts.pop() {
        loop {
            if cur.len() > 2 && cur.first() == cur.last() {
                rings.push(cur);
                break;
            }
            let end = *cur.last().unwrap_or(&0);
            let Some(i) = parts.iter().position(|p| p.first() == Some(&end) || p.last() == Some(&end)) else {
                open += 1;
                break;
            };
            let mut p = parts.swap_remove(i);
            if p.first() != Some(&end) {
                p.reverse();
            }
            cur.extend_from_slice(&p[1..]);
        }
    }
    (rings, open)
}

/// Extract the features inside `bbox` from a PBF file held in memory.
pub fn extract(file: &[u8], bbox: BboxE7, levels: &MapLevels, threads: usize) -> Result<OsmData, String> {
    let mut raw = Vec::new();
    let mut it = pbf::blobs(file);
    let mut snapshot = String::from("unknown");
    while let Some(b) = it.next_raw() {
        let b = b?;
        if b.kind == "OSMHeader" {
            if let Some(t) = pbf::header_timestamp(&pbf::inflate_blob(b.blob)?)? {
                snapshot = iso8601(t);
            }
        }
        raw.push(b);
    }
    // Pass 1: the nodes inside the box, sorted by id.
    let mut nodes: Vec<(i64, i32, i32)> = each_block(&raw, threads, Want { nodes: true, ways: false, relations: false }, |b| {
        b.nodes.into_iter().filter(|n| bbox.holds(n.lat_e7, n.lon_e7)).map(|n| (n.id, n.lat_e7, n.lon_e7)).collect::<Vec<_>>()
    })?
    .into_iter()
    .flatten()
    .collect();
    nodes.sort_unstable_by_key(|n| n.0);
    let at = |id: i64| -> Option<(i32, i32)> { nodes.binary_search_by_key(&id, |n| n.0).ok().map(|i| (nodes[i].1, nodes[i].2)) };
    // Pass 2: water multipolygons and their member ways.
    let rels: Vec<pbf::Relation> = each_block(&raw, threads, Want { nodes: false, ways: false, relations: true }, |b| {
        b.relations.into_iter().filter(|r| tag(&r.tags, "type") == Some("multipolygon") && is_water_area(&r.tags).is_some()).collect::<Vec<_>>()
    })?
    .into_iter()
    .flatten()
    .collect();
    let members: HashSet<i64> = rels.iter().flat_map(|r| r.members.iter().filter(|m| m.kind == 1).map(|m| m.id)).collect();
    // Pass 3: the ways we keep.
    let ways: Vec<pbf::Way> = each_block(&raw, threads, Want { nodes: false, ways: true, relations: false }, |b| {
        b.ways
            .into_iter()
            .filter(|w| {
                members.contains(&w.id)
                    || tag(&w.tags, "natural") == Some("coastline")
                    || is_water_area(&w.tags).is_some()
                    || tag(&w.tags, "highway").is_some_and(|h| levels.road_class(h).is_some())
                    || tag(&w.tags, "building").is_some_and(|v| v != "no")
            })
            .filter(|w| w.refs.iter().any(|r| at(*r).is_some()))
            .collect::<Vec<_>>()
    })?
    .into_iter()
    .flatten()
    .collect();
    drop(raw);
    let mut out = OsmData { snapshot, ..Default::default() };
    let coords = |refs: &[i64]| -> Vec<(i32, i32)> { refs.iter().filter_map(|r| at(*r)).collect() };

    // ── The coastline: runs of present nodes, stitched on end node ids, never reversed ──
    let mut pieces: Vec<(i64, Vec<i64>)> = Vec::new();
    for w in ways.iter().filter(|w| tag(&w.tags, "natural") == Some("coastline")) {
        let mut run: Vec<i64> = Vec::new();
        for &r in &w.refs {
            if at(r).is_some() {
                run.push(r);
            } else if !run.is_empty() {
                if run.len() >= 2 {
                    pieces.push((w.id, std::mem::take(&mut run)));
                }
                run.clear();
            }
        }
        if run.len() >= 2 {
            pieces.push((w.id, run));
        }
    }
    pieces.sort_by_key(|p| p.0);
    let mut by_start: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        by_start.entry(p.1[0]).or_default().push(i);
    }
    let mut used = vec![false; pieces.len()];
    // Start chains at pieces nobody ends into first (open chains), then whatever is left (closed).
    let ends: HashSet<i64> = pieces.iter().map(|p| *p.1.last().unwrap()).collect();
    let order: Vec<usize> = (0..pieces.len()).filter(|i| !ends.contains(&pieces[*i].1[0])).chain(0..pieces.len()).collect();
    for start in order {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (mut id, mut refs) = (pieces[start].0, pieces[start].1.clone());
        loop {
            let end = *refs.last().unwrap();
            if refs.len() > 2 && end == refs[0] {
                break;
            }
            let Some(&next) = by_start.get(&end).and_then(|v| v.iter().find(|i| !used[**i])) else { break };
            used[next] = true;
            id = id.min(pieces[next].0);
            refs.extend_from_slice(&pieces[next].1[1..]);
        }
        let closed = refs.len() > 3 && refs.first() == refs.last();
        let mut pts = coords(&refs);
        pts.dedup();
        if closed {
            pts.pop();
        }
        if pts.len() >= 2 {
            out.coast.push(Chain { id, points: pts, closed });
        }
    }
    out.coast.sort_by_key(|c| c.id);

    // ── Water areas: closed ways, then multipolygons ──
    let way_by_id: HashMap<i64, &pbf::Way> = ways.iter().map(|w| (w.id, w)).collect();
    let mut open_rings = 0;
    for w in &ways {
        let Some(kind) = is_water_area(&w.tags) else { continue };
        if w.refs.len() < 4 || w.refs.first() != w.refs.last() || w.refs.iter().any(|r| at(*r).is_none()) {
            continue;
        }
        let ring = tidy_ring(coords(&w.refs));
        if ring.len() >= 3 {
            out.water.push(WaterArea { id: w.id, kind, inner: false, name: tag(&w.tags, "name").map(str::to_string), ring: orient(ring, true) });
        }
    }
    for r in &rels {
        let kind = is_water_area(&r.tags).unwrap_or(AreaKind::Lake);
        for inner in [false, true] {
            let parts: Vec<Vec<i64>> = r
                .members
                .iter()
                .filter(|m| m.kind == 1 && (m.role == "inner") == inner)
                .filter_map(|m| way_by_id.get(&m.id).map(|w| w.refs.clone()))
                .collect();
            let (rings, open) = join_rings(parts);
            open_rings += open;
            for ids in rings {
                if ids.iter().any(|n| at(*n).is_none()) {
                    open_rings += 1;
                    continue;
                }
                let ring = tidy_ring(coords(&ids));
                if ring.len() >= 3 {
                    out.water.push(WaterArea { id: r.id, kind, inner, name: tag(&r.tags, "name").map(str::to_string), ring: orient(ring, !inner) });
                }
            }
        }
    }
    out.water.sort_by(|a, b| (a.id, a.inner, a.ring.first()).cmp(&(b.id, b.inner, b.ring.first())));

    // ── Roads and buildings ──
    for w in &ways {
        if let Some(class) = tag(&w.tags, "highway").and_then(|h| levels.road_class(h)) {
            // Runs of present nodes: a road leaving the box is cut there, never joined by a false straight line.
            let mut run: Vec<(i32, i32)> = Vec::new();
            let flush = |run: &mut Vec<(i32, i32)>, out: &mut OsmData| {
                run.dedup();
                if run.len() >= 2 {
                    out.roads.push(Road {
                        id: w.id,
                        class,
                        bridge: tag(&w.tags, "bridge").is_some_and(|v| v != "no"),
                        tunnel: tag(&w.tags, "tunnel").is_some_and(|v| v != "no"),
                        layer: tag(&w.tags, "layer").and_then(|v| v.trim().parse::<i8>().ok()).unwrap_or(0),
                        width_m: tag(&w.tags, "width").and_then(parse_metres).unwrap_or(0.0),
                        name: tag(&w.tags, "name").map(str::to_string),
                        points: std::mem::take(run),
                    });
                }
                run.clear();
            };
            for r in &w.refs {
                match at(*r) {
                    Some(p) => run.push(p),
                    None => flush(&mut run, &mut out),
                }
            }
            flush(&mut run, &mut out);
        }
        if tag(&w.tags, "building").is_some_and(|v| v != "no") && w.refs.len() >= 4 && w.refs.first() == w.refs.last() && w.refs.iter().all(|r| at(*r).is_some()) {
            let ring = tidy_ring(coords(&w.refs));
            if ring.len() < 3 {
                continue;
            }
            let levels_tag = tag(&w.tags, "building:levels").and_then(|v| v.trim().split(';').next()?.trim().parse::<f32>().ok()).filter(|l| *l > 0.0 && *l < 255.0);
            let height = tag(&w.tags, "height").and_then(parse_metres).or(levels_tag.map(|l| l * METRES_PER_LEVEL)).filter(|h| *h <= MAX_BUILDING_HEIGHT_M).unwrap_or(0.0);
            out.buildings.push(Building {
                id: w.id,
                height_m: height,
                min_height_m: tag(&w.tags, "min_height").and_then(parse_metres).filter(|h| *h < height).unwrap_or(0.0),
                levels: levels_tag.map_or(0, |l| l.round() as u8),
                ring,
            });
        }
    }
    out.roads.sort_by_key(|r| r.id);
    out.buildings.sort_by_key(|b| b.id);
    out.stats = vec![
        ("nodes in the box".into(), nodes.len()),
        ("ways kept".into(), ways.len()),
        ("water relations".into(), rels.len()),
        ("coastline chains".into(), out.coast.len()),
        ("closed coastline chains".into(), out.coast.iter().filter(|c| c.closed).count()),
        ("water rings".into(), out.water.len()),
        ("water rings that would not close".into(), open_rings),
        ("road pieces".into(), out.roads.len()),
        ("buildings".into(), out.buildings.len()),
    ];
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The snapshot date, and the tag parsers. Seen red 2026-11-09 with the month taken before
    /// the March shift (`mp + 1`): "an extract's evening timestamp".
    #[test]
    fn dates_and_tags() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_794_172_860), "2026-11-08T21:21:00Z", "an extract's evening timestamp");
        assert_eq!(iso8601(951_782_400), "2000-02-29T00:00:00Z", "a leap day");
        assert_eq!(parse_metres("12.5"), Some(12.5));
        assert_eq!(parse_metres("40 m"), Some(40.0));
        assert!((parse_metres("100 ft").unwrap() - 30.48).abs() < 1e-4);
        assert_eq!(parse_metres("tall"), None);
        assert_eq!(is_water_area(&[("natural".into(), "water".into()), ("water".into(), "river".into())]), Some(AreaKind::River));
        assert_eq!(is_water_area(&[("natural".into(), "water".into())]), Some(AreaKind::Lake));
        assert_eq!(is_water_area(&[("natural".into(), "wood".into())]), None);
    }

    /// Member ways join into rings at shared ends, reversed where they run backwards; an open
    /// ring is counted, not invented. Seen red 2026-11-09 without the reversal: "two halves of
    /// a lake, one drawn backwards, make one ring; left: 0".
    #[test]
    fn member_ways_join_into_rings() {
        let (rings, open) = join_rings(vec![vec![1, 2, 3], vec![1, 4, 3]]);
        assert_eq!(rings.len(), 1, "two halves of a lake, one drawn backwards, make one ring");
        assert_eq!(rings[0].first(), rings[0].last(), "closed");
        assert_eq!(rings[0].len(), 5);
        assert_eq!(open, 0);
        let (rings, open) = join_rings(vec![vec![1, 2, 3], vec![3, 4]]);
        assert!(rings.is_empty() && open >= 1, "an unclosed ring is counted, never invented: {} rings, {open} open", rings.len());
        let ccw = orient(vec![(0, 0), (0, 10), (10, 10)], true);
        assert!(ring_sign(&ccw) > 0.0);
    }
}
