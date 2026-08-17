#!/usr/bin/env node
/**
 * fetch-osm-region.mjs
 *
 * Pulls REAL roads and building footprints for one bounding box out of
 * OpenStreetMap (through the public Overpass API) and converts them into
 * data/maps/regions/<region>.bin -- the compact binary the native Maps page's
 * PLANET (GPS) view draws, and which the in-world 3D extruder will later read
 * to raise real buildings on the chunked-LOD Earth. This is the maps ladder,
 * rung 3, first increment (docs/design/maps-ladder.md).
 *
 * WHEN THIS RUNS: at DEVELOPMENT / BUILD time, by hand, once per region. A
 * one-off bounding-box extract is well within Overpass API fair use (it is a
 * single query, cached to a committed file, not a per-user or per-frame call).
 * THE SHIPPED APP NEVER CALLS OSM OR OVERPASS. It only reads the .bin that
 * ships next to the exe, exactly like data/stars-map.bin. Do NOT wire any
 * runtime code to overpass-api.de or to the OSM tile servers: their usage
 * policy forbids application traffic, and a released client hammering them
 * would get the whole project blocked.
 *
 * ATTRIBUTION (REQUIRED, NOT OPTIONAL). Everything this script writes is:
 *     Data (c) OpenStreetMap contributors, licensed ODbL 1.0
 *     https://www.openstreetmap.org/copyright
 * ODbL is a share-alike licence for the DATA itself. Any UI that draws a
 * region file MUST show that credit line (the Maps page's Planet view, and
 * the in-world map/GPS surface), and any redistribution of a region file
 * carries the same licence forward. This is a licence obligation, not a
 * courtesy: dropping the credit makes the build non-compliant.
 *
 * Usage (these two invocations are the canonical shipped regions; re-run both
 * after any format change so every shipped .bin carries the current format):
 *   node scripts/fetch-osm-region.mjs \
 *     --bbox 47.6050,-122.3560,47.6280,-122.3280 \
 *     --name "Seattle Center" \
 *     --out data/maps/regions/seattle-center.bin
 *
 *   node scripts/fetch-osm-region.mjs \
 *     --bbox 47.5900,-122.7300,47.6900,-122.6200 \
 *     --name "Silverdale" \
 *     --out data/maps/regions/silverdale.bin
 *
 *   node scripts/fetch-osm-region.mjs --verify-only data/maps/regions/seattle-center.bin
 *
 *   --bbox         south,west,north,east in DEGREES (the Overpass argument
 *                  order). south < north, west < east.
 *   --name         display name stored in the file (<= 255 UTF-8 bytes).
 *   --out          output path; parent directories are created.
 *   --verify-only  skip the network entirely and just re-verify a file.
 *
 * ══ FORMAT SPEC "HOSMREG2" ════════════════════════════════════════════════
 * All integers and floats are LITTLE-ENDIAN. There is no padding and no
 * alignment anywhere: every field starts immediately after the previous one,
 * so the whole file is walked strictly front to back.
 *
 * v2 adds WATER: sea polygons assembled from the OSM coastline, inland water
 * surfaces (lakes, ponds, river surfaces), and islands. v1 ("HOSMREG1", a
 * 16-byte header without water_count) is dead: nothing shipped, both region
 * files are rebuilt as v2, and the Rust reader reads only v2.
 *
 *   Header (20 bytes):
 *     0..8    magic  b"HOSMREG2" (8 ASCII bytes, no NUL)
 *     8..12   u32    road_count      (number of road records)
 *     12..16  u32    building_count  (number of building records)
 *     16..20  u32    water_count     (number of water records)
 *
 *   Meta block (immediately after the header, 25 + name_len bytes):
 *     20..28  f64    origin_lat_deg      bbox CENTRE latitude, degrees
 *     28..36  f64    origin_lon_deg      bbox CENTRE longitude, degrees
 *     36..40  f32    half_span_east_m    bbox half-width, metres
 *     40..44  f32    half_span_north_m   bbox half-height, metres
 *     44..45  u8     name_len            0..=255, UTF-8 BYTES (not chars)
 *     45..    bytes  name_len bytes of UTF-8 display name, no NUL
 *
 *   COORDINATES. Every coordinate after the meta block is METRES EAST and
 *   METRES NORTH of the origin, as f32, in a local tangent-plane projection:
 *
 *       east_m  = (lon_deg - origin_lon_deg) * cos(origin_lat_rad) * 111320.0
 *       north_m = (lat_deg - origin_lat_deg) * 110540.0
 *
 *   Degrees in, metres out. The two constants are the standard spherical
 *   approximations: 111320.0 m is one degree of longitude at the equator
 *   (2*pi*R/360 for R = 6378137 m, the WGS84 equatorial radius), scaled by
 *   cos(origin_lat) to shrink toward the poles; 110540.0 m is one degree of
 *   latitude near the equator on the WGS84 ellipsoid. cos() is evaluated ONCE
 *   at the ORIGIN latitude, so the whole region shares one flat projection --
 *   that is what makes the format cheap for the renderer (no per-point trig)
 *   and is accurate to well under a metre over a region a few km across.
 *   The latitude constant is a fixed value, not a per-latitude series, so at
 *   mid latitudes north distances read about 0.6% short of the true WGS84
 *   meridian arc (at 47.6 deg the real figure is ~111180 m/deg). That is a
 *   deliberate, documented simplification: it is uniform across a region, so
 *   it scales the map very slightly rather than distorting it. Both constants
 *   are part of the CONTRACT -- the Rust reader must use the same two numbers
 *   or geometry will drift against the terrain.
 *
 *   Road records: road_count consecutive, variable length, starting right
 *   after the meta block:
 *     u8     class        0 motorway/trunk
 *                         1 primary
 *                         2 secondary
 *                         3 tertiary/residential/unclassified
 *                         4 service
 *                         5 footway/path/cycleway/pedestrian
 *     u8     name_len     0..=40 UTF-8 bytes; 0 means the way is unnamed
 *     bytes  name         name_len bytes of UTF-8, the OSM `name` tag,
 *                         truncated on a CHARACTER boundary to <= 40 bytes
 *                         (never splits a multi-byte character, so it always
 *                         decodes)
 *     u16    point_count  >= 2
 *     then point_count * (f32 east_m, f32 north_m)  -- 8 bytes per point
 *
 *   Building records: building_count consecutive, variable length, starting
 *   right after the last road record:
 *     f32    height_m     from the OSM `height` tag in metres; else
 *                         `building:levels` * 3.0; else 0.0, which means
 *                         UNKNOWN (not "flat") -- the renderer picks a
 *                         default for 0.0
 *     u16    point_count  >= 3
 *     then point_count * (f32 east_m, f32 north_m)  -- the footprint RING
 *
 *   The ring is the OSM way's closed ring with the DUPLICATED CLOSING POINT
 *   REMOVED (OSM repeats the first node at the end; this format does not), so
 *   a reader closes the polygon itself by joining the last point back to the
 *   first. Consecutive duplicate points are dropped. Rings left with fewer
 *   than 3 distinct points are degenerate and are not written at all.
 *
 *   Water records: water_count consecutive, variable length, starting right
 *   after the last building record:
 *     u8     kind         0 = sea: tidal water assembled from the OSM
 *                             natural=coastline, clipped to the EXACT bbox
 *                         1 = inland water: lake / pond / river surface,
 *                             from natural=water or waterway=riverbank ways
 *                             and multipolygon relations
 *                         2 = island: an inner LAND ring lying inside the
 *                             nearest PRECEDING kind 0 or kind 1 record (a
 *                             coastline island inside the sea, or a
 *                             multipolygon inner ring inside its lake)
 *     u8     name_len     0..=40 UTF-8 bytes, truncated on a CHARACTER
 *                         boundary exactly like road names; 0 = unnamed
 *     bytes  name         name_len bytes of UTF-8 (relation name for
 *                         multipolygon water, way name otherwise; a sea
 *                         polygon takes a name from a named coastline way
 *                         when one participates)
 *     u16    point_count  >= 3
 *     then point_count * (f32 east_m, f32 north_m)  -- the ring
 *
 *   Water rings follow the SAME conventions as building footprints: the
 *   duplicated closing point is removed (the reader closes the ring itself),
 *   consecutive duplicate points are dropped, and rings left with fewer than
 *   3 distinct points are degenerate and are not written at all. Same
 *   projection, same two constants, as every other coordinate in the file.
 *
 *   SEA ASSEMBLY (kind 0). OSM maps the sea as open natural=coastline lines,
 *   not polygons, with one hard convention: following a coastline way's node
 *   order, LAND is on the LEFT and WATER is on the RIGHT. Coastline ways are
 *   stitched into chains on shared endpoint NODE IDS (never reversed: the
 *   direction is the data). A chain that closes on itself entirely inside
 *   the bbox is an island, emitted as kind 2. A chain that crosses the bbox
 *   is clipped to the exact bbox rectangle, and each clipped piece is closed
 *   into a polygon by walking along the bbox boundary from the piece's EXIT
 *   point to the nearest piece ENTRY point CLOCKWISE (in east/north metre
 *   space, corners included as vertices). Clockwise is derived, not guessed:
 *   traversing the finished water polygon with each coastline piece in its
 *   OSM direction keeps the water (the polygon interior) on the RIGHT, so
 *   the polygon is clockwise, and the boundary-walk part of a clockwise
 *   polygon is exactly the walk that keeps the bbox interior on the right,
 *   which is the clockwise walk around the rectangle. Multiple pieces chain
 *   through this walk into one polygon per connected water region. verify()
 *   proves the convention on real data: a known open-water point must land
 *   INSIDE the assembled sea and a known on-land point must land OUTSIDE it.
 *
 *   ORDERING (byte-determinism): roads then buildings, each group in
 *   ascending OSM way id, exactly as v1 (a way split by the margin clip
 *   contributes its pieces consecutively, in geometry order). Then water:
 *   kind 0 sea polygons first, ordered by the smallest OSM way id
 *   participating in each polygon, each immediately followed by its kind 2
 *   islands ordered by their own smallest way id; then inland water ordered
 *   by OSM way id (plain ways) or relation id (multipolygons), each record
 *   immediately followed by its kind 2 inner islands ordered by their own
 *   smallest way id. A rebuild against the same OSM snapshot is
 *   byte-identical regardless of the order Overpass happened to answer in,
 *   and the build asserts it: the records are converted and serialized twice
 *   from the same parsed answer and the two buffers must match.
 *
 *   The file is exactly
 *       20 + 25 + region_name_len
 *          + sum over roads     (4 + name_len_i + 8*point_count_i)
 *          + sum over buildings (6 + 8*point_count_j)
 *          + sum over waters    (4 + name_len_k + 8*point_count_k)
 *   bytes, and verify() walks it record by record and asserts it lands
 *   exactly on EOF.
 *
 * ══ REGION MARGIN AND CLIPPING ════════════════════════════════════════════
 * Overpass answers a bbox query with every way that touches the box, and
 * `out geom` returns those ways' FULL geometry -- including the parts far
 * outside the box. Measured on this very query (Seattle centre, 2.1 x 2.5 km):
 * 9 of 10423 ways reach more than 400 m past the bbox and the worst (the
 * Elliott Bay Trail) runs 1394 m past it. Storing those tails whole would
 * contradict the meta block, which tells the app the region is half_span_east
 * by half_span_north: the app would hold, and try to draw, geometry it has no
 * terrain or context for.
 *
 * So this script keeps a KEEP RECTANGLE = the bbox grown by KEEP_MARGIN_M
 * (300 m) on all four sides, and:
 *   - a way lying ENTIRELY inside the keep rectangle is stored verbatim,
 *     point for point, untouched (this is the overwhelming majority);
 *   - a way that leaves the rectangle is clipped to it, with the crossing
 *     point computed exactly on the boundary, and split into one record per
 *     run that is inside (so a road that leaves and comes back becomes two
 *     records rather than one record with a false straight shortcut);
 *   - a way entirely outside the rectangle is dropped.
 * Buildings are NEVER clipped -- clipping a polygon changes its shape -- so a
 * footprint is kept whole if any of its points is inside the keep rectangle,
 * and dropped otherwise. That is why verify()'s coordinate bound is the half
 * spans plus 400 m: 300 m of margin plus 100 m of slack for a footprint that
 * straddles the margin edge.
 *
 * Drop and clip counts are printed on every run; a sudden change in them is
 * the signal that OSM data, or this script, moved.
 *
 * Water follows the same philosophy with one deliberate difference per kind:
 *   - SEA polygons (kind 0) are clipped to the EXACT bbox rectangle, because
 *     the bbox boundary is literally part of their geometry (the clockwise
 *     boundary walk that closes them); letting them spill into the margin
 *     would move that boundary.
 *   - INLAND water (kind 1) and multipolygon inner islands are polygons like
 *     buildings, but unlike buildings they can dwarf the region (Lake Union
 *     vs the Seattle Center bbox), so they ARE clipped, with a true polygon
 *     clip (Sutherland-Hodgman) against the keep rectangle.
 *   - Coastline islands (kind 2 from sea assembly) lie entirely inside the
 *     bbox by construction and are stored verbatim.
 * Multipolygon relation member ways are fetched with the bbox grown by
 * REL_MEMBER_MARGIN_DEG on every side so a lake moderately larger than the
 * bbox still closes its rings before clipping; a ring that cannot be closed
 * from the fetched data is skipped with a warning naming the relation id and
 * counted in the skipped-rings stat printed at the end of the run.
 *
 * The Rust loader lives on the Maps side (Planet view). Keep it in sync with
 * this spec -- this header is the contract.
 */

import { readFileSync, writeFileSync, mkdirSync, statSync } from 'node:fs';
import { dirname, basename, resolve } from 'node:path';

// ── Format constants (the contract; verify() re-asserts the literals) ──────

const MAGIC = 'HOSMREG2';
const HEADER_SIZE = 20;
const MAX_ROAD_NAME_BYTES = 40;
/** Water record names share the road cap (and the same truncation helper). */
const MAX_WATER_NAME_BYTES = 40;
const MAX_REGION_NAME_BYTES = 255;
const MAX_POINTS = 65535; // u16 point_count ceiling

/** Water record kinds (u8); see the format spec above. */
const WATER_KIND_SEA = 0;    // tidal water assembled from natural=coastline
const WATER_KIND_INLAND = 1; // lake / pond / river surface polygon
const WATER_KIND_ISLAND = 2; // inner LAND ring inside the preceding 0 or 1
const MAX_WATER_KIND = 2;

/** Metres per degree of longitude AT THE EQUATOR; scaled by cos(lat). */
const M_PER_DEG_LON_EQ = 111320.0;
/** Metres per degree of latitude (fixed spherical approximation). */
const M_PER_DEG_LAT = 110540.0;

/** Half-width of the band kept around the bbox, metres (see header). */
const KEEP_MARGIN_M = 300;
/** verify() rejects any coordinate beyond half_span + this, metres. */
const COORD_SLOP_M = 400;
/**
 * Relation member ways are fetched with the bbox grown by this many degrees
 * on every side (about 5.5 km), so a water body moderately larger than the
 * bbox (Lake Union vs the Seattle Center bbox) still closes its multipolygon
 * rings before clipping. A body larger than even this margin fails ring
 * closure and is skipped with a warning; the sea (coastline) is unaffected.
 */
const REL_MEMBER_MARGIN_DEG = 0.05;
/** A clipped run endpoint within this of a bbox edge counts as ON it, metres. */
const BOUNDARY_EPS_M = 0.01;

/** Heights above this are data errors (tallest building on Earth ~828 m). */
const MAX_BUILDING_HEIGHT_M = 1000;
/** OSM's own default when only a floor count is known. */
const METRES_PER_LEVEL = 3.0;

// ── Overpass ──────────────────────────────────────────────────────────────

const OVERPASS_URL = 'https://overpass-api.de/api/interpreter';
/** A real, identifying User-Agent is an Overpass fair-use requirement. */
const USER_AGENT = 'HumanityOS build script; github.com/Shaostoul/Humanity';
/** Overpass load-shedding statuses. Retried ONCE, then we give up. */
const RETRY_STATUS = new Set([429, 502, 503, 504]);
const RETRY_DELAY_MS = 30_000;
const QUERY_TIMEOUT_S = 180;

/**
 * OSM highway tag -> road class byte. The Overpass query's highway regex is
 * BUILT FROM THIS MAP's keys, so the fetch and the classification can never
 * drift apart (fetching a value we cannot classify, or classifying one we
 * never fetch, is impossible by construction).
 */
const ROAD_CLASS = new Map([
  ['motorway', 0], ['trunk', 0],
  ['primary', 1],
  ['secondary', 2],
  ['tertiary', 3], ['residential', 3], ['unclassified', 3],
  ['service', 4],
  ['footway', 5], ['path', 5], ['cycleway', 5], ['pedestrian', 5],
]);
const MAX_ROAD_CLASS = 5;
const CLASS_LABEL = [
  'motorway/trunk', 'primary', 'secondary',
  'tertiary/residential/unclassified', 'service', 'footway/path/cycleway/pedestrian',
];

/**
 * Per-region plausibility gates for verify(). Keyed by output FILE NAME so a
 * region carries its own expectations; add an entry when you add a region.
 * These are the checks that catch "Overpass answered, but with junk": a
 * partial answer, a shifted bbox, a silently emptied layer. They are
 * deliberately specific (a downtown must contain its own named streets).
 */
const REGION_GATES = {
  'silverdale.bin': {
    minRoads: 200,
    minBuildings: 300,
    minBuildingsWithHeight: 0,
    // Silverdale's main drag + a crosstown arterial: local knowledge that
    // proves the bbox is really Silverdale, WA.
    anyRoadNamed: ['Silverdale Way Northwest', 'Silverdale Way NW', 'Bucklin Hill Road'],
    // Water: the whole northern lobe of Dyes Inlet is inside the bbox, so a
    // real sea polygon of several km^2 must exist, plus the suburb's many
    // ponds. Island Lake (a multipolygon relation) proves the relation ring
    // assembly, and its inner ring Clark Island proves the kind 2 path.
    minSeaPolygons: 1,
    minSeaAreaKm2: 2,
    minInlandWater: 10,
    anyInlandWaterNamed: ['Island Lake'],
    anyIslandNamed: ['Clark Island'],
    // The sea SIDE-CONVENTION proof (land left / water right): mid Dyes
    // Inlet north lobe must be INSIDE the assembled sea, central Silverdale
    // (dry land) must be OUTSIDE it. Point-in-polygon in region metre space.
    seaContainsLatLon: [[47.6230, -122.6870]],
    seaExcludesLatLon: [[47.6450, -122.6950]],
  },
  'seattle-center.bin': {
    minRoads: 100,
    minBuildings: 500,
    minBuildingsWithHeight: 50,
    // At least ONE road must carry one of these names.
    anyRoadNamed: ['Pike Street', 'Pine Street'],
    // Water: the Elliott Bay waterfront cuts the bbox's south-west corner,
    // and the south tip of Lake Union (a 31-member multipolygon relation)
    // pokes into the north-east corner.
    minSeaPolygons: 1,
    minSeaAreaKm2: 0.05,
    minInlandWater: 5,
    anyInlandWaterNamed: ['Lake Union'],
    anyIslandNamed: [],
    // Open water off the piers vs dry land at Seattle Center itself.
    seaContainsLatLon: [[47.6080, -122.3520]],
    seaExcludesLatLon: [[47.6205, -122.3493]],
  },
};
/** Used for a region with no entry above: structure only, no local knowledge. */
const DEFAULT_GATES = {
  minRoads: 1, minBuildings: 0, minBuildingsWithHeight: 0, anyRoadNamed: [],
  minSeaPolygons: 0, minSeaAreaKm2: 0, minInlandWater: 0,
  anyInlandWaterNamed: [], anyIslandNamed: [],
  seaContainsLatLon: [], seaExcludesLatLon: [],
};

const DEG = Math.PI / 180;

// ── Small helpers ─────────────────────────────────────────────────────────

function die(msg) {
  console.error(`fetch-osm-region: ${msg}`);
  process.exit(1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Truncate a UTF-8 string to at most `max` BYTES without splitting a char. */
function truncateUtf8(str, max) {
  const buf = Buffer.from(str, 'utf8');
  if (buf.length <= max) return buf;
  let end = max;
  // Walk back off any UTF-8 continuation byte (10xxxxxx): standing on one
  // means we are in the middle of a multi-byte character.
  while (end > 0 && (buf[end] & 0xc0) === 0x80) end--;
  return buf.subarray(0, end);
}

/** Parse `--flag value` pairs plus `--verify-only <path>`. */
function parseArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) die(`unexpected argument "${a}"`);
    const key = a.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    const val = argv[++i];
    if (val === undefined) die(`${a} needs a value`);
    args[key] = val;
  }
  return args;
}

/** "south,west,north,east" -> validated {south,west,north,east}. */
function parseBbox(s) {
  const parts = String(s).split(',').map((v) => Number(v.trim()));
  if (parts.length !== 4 || parts.some((v) => !Number.isFinite(v))) {
    die(`--bbox must be four numbers "south,west,north,east", got "${s}"`);
  }
  const [south, west, north, east] = parts;
  if (!(south > -90 && north < 90 && south < north)) {
    die(`--bbox latitudes must satisfy -90 < south (${south}) < north (${north}) < 90`);
  }
  if (!(west >= -180 && east <= 180 && west < east)) {
    die(`--bbox longitudes must satisfy -180 <= west (${west}) < east (${east}) <= 180`);
  }
  return { south, west, north, east };
}

// ── Projection (the format contract, in one place) ────────────────────────

/**
 * Build the projection for a bbox: the origin is the bbox CENTRE, and the
 * cos(lat) factor is evaluated once there, so the whole region shares one
 * flat local tangent plane. Returns the closure the whole script projects
 * through, plus the meta values written into the file.
 */
function makeProjection(bbox) {
  const originLat = (bbox.south + bbox.north) / 2;
  const originLon = (bbox.west + bbox.east) / 2;
  const lonScale = Math.cos(originLat * DEG) * M_PER_DEG_LON_EQ;
  return {
    originLat,
    originLon,
    halfSpanEast: ((bbox.east - bbox.west) / 2) * lonScale,
    halfSpanNorth: ((bbox.north - bbox.south) / 2) * M_PER_DEG_LAT,
    /** [lat_deg, lon_deg] -> [east_m, north_m] */
    project: (lat, lon) => [
      (lon - originLon) * lonScale,
      (lat - originLat) * M_PER_DEG_LAT,
    ],
  };
}

// ── Polyline clipping against the keep rectangle ──────────────────────────

/**
 * Liang-Barsky clip of ONE segment against an axis-aligned rectangle.
 * Returns the clipped [[e,n],[e,n]] or null if the segment misses entirely.
 * The classic parametric form: walk p = p0 + t*(p1-p0) and squeeze t into
 * [t0,t1] with one inequality per rectangle edge.
 */
function clipSegment(p0, p1, r) {
  const dE = p1[0] - p0[0];
  const dN = p1[1] - p0[1];
  let t0 = 0;
  let t1 = 1;
  const tests = [
    [-dE, p0[0] - r.minE], // left edge
    [dE, r.maxE - p0[0]],  // right edge
    [-dN, p0[1] - r.minN], // bottom edge
    [dN, r.maxN - p0[1]],  // top edge
  ];
  for (const [p, q] of tests) {
    if (p === 0) {
      if (q < 0) return null; // parallel to this edge and outside it
    } else {
      const t = q / p;
      if (p < 0) {
        if (t > t1) return null;
        if (t > t0) t0 = t;
      } else {
        if (t < t0) return null;
        if (t < t1) t1 = t;
      }
    }
  }
  return [
    [p0[0] + t0 * dE, p0[1] + t0 * dN],
    [p0[0] + t1 * dE, p0[1] + t1 * dN],
  ];
}

const inRect = (p, r) => p[0] >= r.minE && p[0] <= r.maxE && p[1] >= r.minN && p[1] <= r.maxN;

/**
 * Clip a polyline to the keep rectangle, returning zero or more runs.
 * A polyline wholly inside is returned VERBATIM (same array), so the common
 * case is bit-for-bit "kept in full" with no floating point round trip.
 */
function clipPolyline(points, r) {
  if (points.every((p) => inRect(p, r))) return [points];

  const runs = [];
  let run = null;
  for (let i = 0; i + 1 < points.length; i++) {
    const seg = clipSegment(points[i], points[i + 1], r);
    if (!seg) {
      if (run) { runs.push(run); run = null; }
      continue;
    }
    const [a, b] = seg;
    if (run && samePoint(run[run.length - 1], a)) {
      run.push(b);
    } else {
      if (run) runs.push(run);
      run = [a, b];
    }
  }
  if (run) runs.push(run);
  return runs;
}

/** Metre-scale equality: below a millimetre is the same point to f32 anyway. */
const samePoint = (a, b) => Math.abs(a[0] - b[0]) < 1e-3 && Math.abs(a[1] - b[1]) < 1e-3;

/** Drop consecutive duplicates (clipping and OSM both produce a few). */
function dedupeConsecutive(points) {
  const out = [];
  for (const p of points) {
    if (out.length === 0 || !samePoint(out[out.length - 1], p)) out.push(p);
  }
  return out;
}

/**
 * Count DISTINCT points (not just non-repeating neighbours), millimetre
 * resolution. This is the "< 3 distinct points" degeneracy test from the
 * spec, and verify() re-runs the same RULE on the source so the build and
 * the check cannot disagree about which rings are degenerate.
 */
const distinctCount = (points) => new Set(points.map(([e, n]) => `${e.toFixed(3)},${n.toFixed(3)}`)).size;

// ── Polygon helpers (water rings) ─────────────────────────────────────────

/**
 * Signed shoelace area of an open ring, square metres. Positive means the
 * ring winds counter-clockwise in east/north space (the mathematical
 * positive direction). The assembled sea polygons come out CLOCKWISE by
 * construction (water kept on the right), coastline islands as mapped in
 * OSM come out counter-clockwise (land kept on the left).
 */
function signedAreaM2(ring) {
  let s = 0;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    s += ring[j][0] * ring[i][1] - ring[i][0] * ring[j][1];
  }
  return s / 2;
}

/**
 * Even-odd ray-casting point-in-ring test on an open ring (a duplicated
 * closing point is harmless: the wrap edge is zero-length). Used to attach
 * islands to the water record that contains them, and by verify() to prove
 * the sea side convention against known on-water / on-land points.
 */
function pointInRing(pt, ring) {
  let inside = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i];
    const [xj, yj] = ring[j];
    if ((yi > pt[1]) !== (yj > pt[1])
      && pt[0] < ((xj - xi) * (pt[1] - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/**
 * True polygon clip (Sutherland-Hodgman) of an OPEN ring against an
 * axis-aligned rectangle, one half-plane at a time. Returns a new OPEN
 * ring, possibly empty. Buildings are kept whole or dropped, but inland
 * water can dwarf the region (Lake Union vs the Seattle Center bbox), so
 * water genuinely gets cut. The intersection helpers are only called when
 * the segment truly crosses the clip line, so the divisions never see 0.
 */
function clipRingToRect(ring, r) {
  const crossE = (a, b, e) => [e, a[1] + ((e - a[0]) / (b[0] - a[0])) * (b[1] - a[1])];
  const crossN = (a, b, n) => [a[0] + ((n - a[1]) / (b[1] - a[1])) * (b[0] - a[0]), n];
  const passes = [
    [(p) => p[0] >= r.minE, (a, b) => crossE(a, b, r.minE)],
    [(p) => p[0] <= r.maxE, (a, b) => crossE(a, b, r.maxE)],
    [(p) => p[1] >= r.minN, (a, b) => crossN(a, b, r.minN)],
    [(p) => p[1] <= r.maxN, (a, b) => crossN(a, b, r.maxN)],
  ];
  let poly = ring;
  for (const [inside, cross] of passes) {
    const out = [];
    for (let i = 0; i < poly.length; i++) {
      const cur = poly[i];
      const prev = poly[(i + poly.length - 1) % poly.length];
      const curIn = inside(cur);
      const prevIn = inside(prev);
      if (curIn) {
        if (!prevIn) out.push(cross(prev, cur));
        out.push(cur);
      } else if (prevIn) {
        out.push(cross(prev, cur));
      }
    }
    poly = out;
    if (poly.length === 0) return poly;
  }
  return poly;
}

// ── Tag parsing ───────────────────────────────────────────────────────────

/**
 * OSM `height` -> metres, or NaN when it is not parseable. Bare numbers are
 * metres by OSM convention; an explicit metre suffix is allowed, and so are
 * the imperial spellings OSM tolerates ("40 ft", "40'", "40'6\"").
 */
function parseHeightTag(v) {
  if (typeof v !== 'string') return NaN;
  const s = v.trim().toLowerCase();
  let m = /^(-?\d+(?:\.\d+)?)\s*(?:m|meter|meters|metre|metres)?$/.exec(s);
  if (m) return Number(m[1]);
  m = /^(-?\d+(?:\.\d+)?)\s*(?:ft|feet|foot|')$/.exec(s);
  if (m) return Number(m[1]) * 0.3048;
  m = /^(\d+)\s*'\s*(\d+(?:\.\d+)?)\s*"?$/.exec(s); // feet and inches
  if (m) return (Number(m[1]) + Number(m[2]) / 12) * 0.3048;
  return NaN;
}

/** OSM `building:levels` -> floor count, or NaN ("3;4" takes the first). */
function parseLevelsTag(v) {
  if (typeof v !== 'string') return NaN;
  const m = /^(-?\d+(?:\.\d+)?)/.exec(v.trim());
  return m ? Number(m[1]) : NaN;
}

/**
 * Building height in metres: `height` tag first, then levels * 3 m, then 0.0
 * meaning UNKNOWN. Returns { height, source } so the build can report how
 * many footprints actually carry real height data.
 */
function buildingHeight(tags) {
  const h = parseHeightTag(tags.height);
  if (Number.isFinite(h) && h > 0 && h <= MAX_BUILDING_HEIGHT_M) {
    return { height: h, source: 'height' };
  }
  const levels = parseLevelsTag(tags['building:levels']);
  if (Number.isFinite(levels) && levels > 0 && levels * METRES_PER_LEVEL <= MAX_BUILDING_HEIGHT_M) {
    return { height: levels * METRES_PER_LEVEL, source: 'levels' };
  }
  const rejected = (tags.height !== undefined && !(Number.isFinite(h) && h > 0 && h <= MAX_BUILDING_HEIGHT_M))
    || (tags['building:levels'] !== undefined && !(Number.isFinite(levels) && levels > 0));
  return { height: 0.0, source: rejected ? 'unparseable' : 'none' };
}

// ── Fetch ─────────────────────────────────────────────────────────────────

/**
 * The Overpass QL query. Still ONE POST, now three parts:
 *   1. The bbox union of ways: highways whose value is in ROAD_CLASS (the
 *      regex is generated from it), anything tagged building=*, the three
 *      water sources (natural=water, waterway=riverbank surfaces,
 *      natural=coastline). `out body geom` gives tags, the NODE ID list AND
 *      the node coordinates inline; the node ids are what water ring
 *      stitching joins on, so no second node lookup is needed.
 *   2. Water multipolygon relations touching the bbox, `out body` (members
 *      and tags; their geometry comes from part 3).
 *   3. `way(r)` -- the member ways of those relations, limited to the bbox
 *      grown by REL_MEMBER_MARGIN_DEG, with `out body geom` again. A way in
 *      both part 1 and part 3 is printed twice; convert() dedupes by id.
 * Building multipolygon relations (courtyards) are still deliberately not
 * requested: ways are the bulk of the data there. Water is different: real
 * lakes (Island Lake, Lake Union) ARE multipolygon relations, so skipping
 * relations would lose exactly the water the format exists to show.
 */
function buildQuery(bbox) {
  const box = `${bbox.south},${bbox.west},${bbox.north},${bbox.east}`;
  const m = REL_MEMBER_MARGIN_DEG;
  const rbox = `${bbox.south - m},${bbox.west - m},${bbox.north + m},${bbox.east + m}`;
  const highways = [...ROAD_CLASS.keys()].join('|');
  return `[out:json][timeout:${QUERY_TIMEOUT_S}];
(
  way["highway"~"^(${highways})$"](${box});
  way["building"](${box});
  way["natural"="water"](${box});
  way["waterway"="riverbank"](${box});
  way["natural"="coastline"](${box});
);
out body geom;
(
  relation["natural"="water"](${box});
  relation["waterway"="riverbank"](${box});
);
out body;
way(r)(${rbox});
out body geom;`;
}

/** POST the query; retry ONCE on an Overpass load-shed status or a network error. */
async function overpassFetch(query) {
  for (let attempt = 1; attempt <= 2; attempt++) {
    const last = attempt === 2;
    try {
      const res = await fetch(OVERPASS_URL, {
        method: 'POST',
        headers: {
          'Content-Type': 'text/plain; charset=utf-8',
          'User-Agent': USER_AGENT,
          Accept: 'application/json',
        },
        body: query,
      });
      if (res.ok) return await res.text();
      const body = await res.text().catch(() => '');
      if (RETRY_STATUS.has(res.status) && !last) {
        console.warn(`overpass: HTTP ${res.status} ${res.statusText}; retrying once in ${RETRY_DELAY_MS / 1000}s`);
        await sleep(RETRY_DELAY_MS);
        continue;
      }
      die(`overpass HTTP ${res.status} ${res.statusText}\n${body.slice(0, 600)}`);
    } catch (err) {
      if (last) die(`overpass request failed: ${err && err.message ? err.message : err}`);
      console.warn(`overpass: ${err && err.message ? err.message : err}; retrying once in ${RETRY_DELAY_MS / 1000}s`);
      await sleep(RETRY_DELAY_MS);
    }
  }
  return die('unreachable');
}

/** Parse + sanity-check the Overpass answer (it reports failures in-band). */
function parseOverpass(text) {
  let json;
  try {
    json = JSON.parse(text);
  } catch (err) {
    die(`overpass did not return JSON (${err.message}); first 600 bytes:\n${text.slice(0, 600)}`);
  }
  if (json.remark) die(`overpass reported a problem: ${json.remark}`);
  if (!Array.isArray(json.elements)) die('overpass answer has no elements array');
  if (json.elements.length === 0) die('overpass returned 0 elements -- wrong bbox, or the query matched nothing');
  return json;
}

// ── Water assembly ────────────────────────────────────────────────────────

/**
 * Project one fetched way into water-pass form: parallel node-id and
 * projected-point arrays plus the name tag. Returns null (and counts it)
 * when Overpass did not deliver node ids alongside the geometry; water ring
 * stitching joins on NODE IDS, the one identity that survives OSM splitting
 * a shoreline at arbitrary vertices.
 */
function waterWayGeom(el, proj, stats) {
  const geom = el.geometry;
  const nodes = el.nodes;
  if (!Array.isArray(geom) || geom.length === 0
    || !Array.isArray(nodes) || nodes.length !== geom.length) {
    stats.waterWaysBadGeometry++;
    return null;
  }
  return {
    id: el.id,
    nodes,
    pts: geom.map((p) => proj.project(p.lat, p.lon)),
    name: el.tags && typeof el.tags.name === 'string' ? el.tags.name : '',
  };
}

/**
 * Assemble closed rings from an UNORDERED set of relation member ways by
 * joining shared endpoint node ids, reversing members where needed (relation
 * members carry no reliable direction; only the ring matters). One ring may
 * span several member ways, and one member list may hold several rings (a
 * relation with many separate outer lakes). Deterministic: ring starts and
 * joins always take the smallest unused way id. Rings that cannot be closed
 * from the data present come back in `failures`; the caller logs the
 * relation id and counts them in the skipped-rings stat.
 */
function assembleRings(items) {
  const rings = [];
  const failures = [];
  const ordered = items.slice().sort((a, b) => a.id - b.id);
  const used = new Set();
  for (const start of ordered) {
    if (used.has(start.id)) continue;
    used.add(start.id);
    const nodes = start.nodes.slice();
    const pts = start.pts.slice();
    const wayIds = [start.id];
    let guard = ordered.length + 1;
    while (nodes[0] !== nodes[nodes.length - 1] && guard-- > 0) {
      const tail = nodes[nodes.length - 1];
      let next = null;
      let reversed = false;
      for (const cand of ordered) {
        if (used.has(cand.id)) continue;
        if (cand.nodes[0] === tail) { next = cand; reversed = false; break; }
        if (cand.nodes[cand.nodes.length - 1] === tail) { next = cand; reversed = true; break; }
      }
      if (!next) break;
      used.add(next.id);
      wayIds.push(next.id);
      if (reversed) {
        nodes.push(...next.nodes.slice(0, -1).reverse());
        pts.push(...next.pts.slice(0, -1).reverse());
      } else {
        nodes.push(...next.nodes.slice(1));
        pts.push(...next.pts.slice(1));
      }
    }
    if (nodes.length >= 2 && nodes[0] === nodes[nodes.length - 1]) {
      rings.push({ pts, wayIds, minWayId: Math.min(...wayIds) });
    } else {
      failures.push({ wayIds });
    }
  }
  return { rings, failures };
}

/**
 * Stitch coastline ways into chains WITHOUT ever reversing one: coastline
 * direction is meaningful (land left, water right), so ways join only where
 * one way's LAST node id is another way's FIRST node id. Deterministic:
 * chain starts are walked in ascending way id, and if the data ever branches
 * (two coastline ways starting at one node, which is broken data) the
 * smallest way id wins and the branch is counted. Open chains are walked
 * first from the ways nothing flows into; whatever remains sits on closed
 * loops (islands mapped as one or more coastline ways).
 */
function stitchCoastChains(ways, stats) {
  const byFirst = new Map(); // first node id -> ways ascending id
  for (const w of ways) {
    const list = byFirst.get(w.nodes[0]) || [];
    list.push(w);
    byFirst.set(w.nodes[0], list);
  }
  for (const list of byFirst.values()) {
    list.sort((a, b) => a.id - b.id);
    if (list.length > 1) stats.coastlineBranches++;
  }
  const continuations = new Set(); // ways some other way flows into
  for (const w of ways) {
    for (const c of byFirst.get(w.nodes[w.nodes.length - 1]) || []) continuations.add(c.id);
  }
  const used = new Set();
  const chains = [];
  const walk = (start) => {
    const members = [];
    const nodes = [];
    const pts = [];
    let cur = start;
    let guard = ways.length + 1;
    while (cur && !used.has(cur.id) && guard-- > 0) {
      used.add(cur.id);
      members.push(cur);
      const skip = nodes.length > 0 ? 1 : 0; // the join node is already present
      nodes.push(...cur.nodes.slice(skip));
      pts.push(...cur.pts.slice(skip));
      if (nodes[0] === nodes[nodes.length - 1]) break; // chain closed on itself
      cur = (byFirst.get(nodes[nodes.length - 1]) || []).find((w) => !used.has(w.id)) || null;
    }
    const named = members.filter((m) => m.name).sort((a, b) => a.id - b.id);
    chains.push({
      wayIds: members.map((m) => m.id),
      minWayId: Math.min(...members.map((m) => m.id)),
      pts,
      closed: nodes.length >= 2 && nodes[0] === nodes[nodes.length - 1],
      name: named.length > 0 ? named[0].name : '',
    });
  };
  const ordered = ways.slice().sort((a, b) => a.id - b.id);
  for (const w of ordered) if (!used.has(w.id) && !continuations.has(w.id)) walk(w);
  for (const w of ordered) if (!used.has(w.id)) walk(w);
  return chains;
}

/**
 * Sea assembly: coastline chains in, closed kind 0 water polygons plus
 * fully-inside island chains out. The format spec header tells the whole
 * story; the mechanical core is: clip every boundary-crossing chain to the
 * EXACT bbox, then close each clipped piece by walking the bbox boundary
 * CLOCKWISE (east/north metre space, SW -> NW -> NE -> SE parameterisation)
 * from the piece's exit to the nearest piece entry, inserting the corner
 * vertices passed on the way. Clockwise keeps the water side (the RIGHT of
 * the coastline direction) enclosed; verify() proves that against known
 * on-water and on-land points.
 */
function assembleSea(chains, rect, warnings, stats) {
  const H = rect.maxN - rect.minN;
  const W = rect.maxE - rect.minE;
  const P = 2 * H + 2 * W;
  const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), hi);
  const perimPos = (p) => {
    if (Math.abs(p[0] - rect.minE) <= BOUNDARY_EPS_M) return clamp(p[1] - rect.minN, 0, H);
    if (Math.abs(p[1] - rect.maxN) <= BOUNDARY_EPS_M) return H + clamp(p[0] - rect.minE, 0, W);
    if (Math.abs(p[0] - rect.maxE) <= BOUNDARY_EPS_M) return H + W + clamp(rect.maxN - p[1], 0, H);
    if (Math.abs(p[1] - rect.minN) <= BOUNDARY_EPS_M) return H + W + H + clamp(rect.maxE - p[0], 0, W);
    return null; // not on the boundary at all
  };
  const corners = [
    { pos: 0, pt: [rect.minE, rect.minN] },         // SW
    { pos: H, pt: [rect.minE, rect.maxN] },         // NW
    { pos: H + W, pt: [rect.maxE, rect.maxN] },     // NE
    { pos: 2 * H + W, pt: [rect.maxE, rect.minN] }, // SE
  ];
  const cyc = (d) => ((d % P) + P) % P;

  const islands = [];
  const pieces = [];
  for (const chain of chains) {
    let pts = chain.pts;
    if (chain.closed) {
      // A closed chain either lies entirely inside the bbox (an island) or
      // crosses the boundary; when it crosses, rotate it to start at a
      // vertex OUTSIDE the rectangle so the closing wrap cannot split one
      // crossing run in two, then clip it exactly like an open chain.
      const k = pts.findIndex((p) => !inRect(p, rect));
      if (k < 0) { islands.push(chain); continue; }
      const open = pts.slice(0, -1);
      pts = [...open.slice(k), ...open.slice(0, k), open[k]];
    }
    for (const run of clipPolyline(pts, rect)) {
      if (run.length < 2) continue;
      const entryPos = perimPos(run[0]);
      const exitPos = perimPos(run[run.length - 1]);
      if (entryPos === null || exitPos === null) {
        warnings.push(`coastline chain starting at way ${chain.wayIds[0]} has an endpoint `
          + 'strictly inside the bbox (broken coastline data); run skipped');
        stats.seaRunsSkippedInsideEnd++;
        continue;
      }
      pieces.push({ run, entryPos, exitPos, minWayId: chain.minWayId, name: chain.name });
    }
  }
  stats.seaPieces = pieces.length;

  const seas = [];
  const used = new Set();
  const startOrder = pieces.slice()
    .sort((a, b) => (a.minWayId - b.minWayId) || (a.entryPos - b.entryPos));
  for (const start of startOrder) {
    if (used.has(start)) continue;
    const poly = [];
    const wayIds = [];
    const names = [];
    let cur = start;
    let ok = true;
    let guard = pieces.length + 1;
    for (;;) {
      if (guard-- <= 0) {
        warnings.push('sea assembly failed to terminate on a polygon; abandoned');
        stats.seaPolygonsAbandoned++;
        ok = false;
        break;
      }
      used.add(cur);
      poly.push(...cur.run);
      wayIds.push(cur.minWayId);
      if (cur.name) names.push({ id: cur.minWayId, name: cur.name });
      // The nearest piece ENTRY walking clockwise from this piece's exit.
      // Only the start piece may be revisited (that is what closes the
      // polygon); every other used piece belongs to a finished polygon.
      let next = null;
      let nextDelta = Infinity;
      for (const cand of pieces) {
        if (used.has(cand) && cand !== start) continue;
        const d = cyc(cand.entryPos - cur.exitPos);
        if (d < nextDelta - 1e-9
          || (Math.abs(d - nextDelta) <= 1e-9 && next !== null && cand.minWayId < next.minWayId)) {
          next = cand;
          nextDelta = d;
        }
      }
      if (next === null) {
        warnings.push('sea assembly found no entry point to continue to; polygon abandoned');
        stats.seaPolygonsAbandoned++;
        ok = false;
        break;
      }
      // Consistency: in valid data no piece EXIT lies strictly between this
      // exit and the entry we walk to (two coastlines would have to cross).
      for (const cand of pieces) {
        if (cand === cur) continue;
        const d = cyc(cand.exitPos - cur.exitPos);
        if (d > 1e-9 && d < nextDelta - 1e-9) { stats.seaBoundaryInconsistencies++; break; }
      }
      // Corner vertices passed on the clockwise walk from exit to entry.
      const passed = corners
        .map((c) => ({ pt: c.pt, d: cyc(c.pos - cur.exitPos) }))
        .filter((c) => c.d > 1e-9 && c.d < nextDelta - 1e-9)
        .sort((a, b) => a.d - b.d);
      for (const c of passed) poly.push(c.pt);
      if (next === start) break; // polygon closed
      cur = next;
    }
    if (!ok) continue;
    const named = names.sort((a, b) => a.id - b.id);
    seas.push({
      pts: poly,
      minWayId: Math.min(...wayIds),
      name: named.length > 0 ? named[0].name : '',
    });
  }
  return { seas, islands };
}

// ── Build ─────────────────────────────────────────────────────────────────

/**
 * Convert an Overpass answer into the road, building and water records of
 * the format. Returns { roads, buildings, waters, stats, warnings } with
 * coordinates already in metres east/north. Deterministic by construction:
 * the same parsed answer always yields the same record set, and main()
 * proves it by converting and serializing twice and comparing buffers.
 */
function convert(json, proj) {
  const keep = {
    minE: -proj.halfSpanEast - KEEP_MARGIN_M,
    maxE: proj.halfSpanEast + KEEP_MARGIN_M,
    minN: -proj.halfSpanNorth - KEEP_MARGIN_M,
    maxN: proj.halfSpanNorth + KEEP_MARGIN_M,
  };
  // The EXACT bbox rectangle: the sea is assembled against this, never the
  // keep rectangle, because the bbox edge is part of the sea geometry.
  const exact = {
    minE: -proj.halfSpanEast, maxE: proj.halfSpanEast,
    minN: -proj.halfSpanNorth, maxN: proj.halfSpanNorth,
  };

  const roads = [];
  const buildings = [];
  const warnings = [];
  const stats = {
    elements: json.elements.length,
    notWay: 0, noGeometry: 0, unclassified: 0, dualTagged: 0,
    relationsSeen: 0, duplicateElements: 0, relationMemberOnlyWays: 0,
    roadWaysSeen: 0, roadWaysKept: 0, roadWaysClipped: 0, roadWaysDroppedOutside: 0,
    roadWaysDroppedShort: 0, roadExtraRecordsFromSplits: 0, roadWaysSplitForU16: 0,
    roadPoints: 0, roadNamesTruncated: 0, roadsNamed: 0,
    buildingWaysSeen: 0, buildingWaysKept: 0, buildingTagNo: 0,
    buildingDroppedOutside: 0, buildingDroppedDegenerate: 0, buildingDroppedTooManyPoints: 0,
    buildingClosingPointsDropped: 0, buildingPoints: 0,
    heightFromTag: 0, heightFromLevels: 0, heightUnknown: 0, heightUnparseable: 0,
    coastlineWays: 0, coastlineChains: 0, coastlineClosedChains: 0, coastlineBranches: 0,
    seaPieces: 0, seaPolygons: 0, seaIslands: 0, seaAreaM2: 0,
    seaRunsSkippedInsideEnd: 0, seaPolygonsAbandoned: 0, seaBoundaryInconsistencies: 0,
    waterRelations: 0, waterRelationsNotMultipolygon: 0, waterRelationMembersMissing: 0,
    waterRingsAssembled: 0, waterRingsSkipped: 0,
    waterWaysStandalone: 0, waterWaysInRelations: 0, waterWaysUnclosed: 0,
    waterWaysBadGeometry: 0, waterAndBuildingTagged: 0,
    waterDroppedOutside: 0, waterDroppedDegenerate: 0, waterDroppedTooManyPoints: 0,
    waterIslandsUnassigned: 0, waterNamesTruncated: 0,
    waterInlandRecords: 0, waterInnerIslands: 0, waterRecords: 0, waterPoints: 0,
  };

  // ── Index the answer. The query prints a way TWICE when it matches the
  // bbox union AND is a member of a fetched relation; dedupe by id so every
  // pass below sees each way exactly once.
  const waysById = new Map();
  const relations = [];
  for (const el of json.elements) {
    if (el.type === 'way') {
      if (waysById.has(el.id)) { stats.duplicateElements++; continue; }
      waysById.set(el.id, el);
    } else if (el.type === 'relation') {
      relations.push(el);
      stats.relationsSeen++;
    } else {
      stats.notWay++;
    }
  }

  const isWaterTags = (tags) => tags.natural === 'water' || tags.waterway === 'riverbank';

  // Spec conventions shared by every water ring: drop the duplicated closing
  // point, drop consecutive duplicates, reject rings left with fewer than 3
  // distinct points or more points than a u16 can count.
  const finishWaterRing = (pts) => {
    let ring = pts;
    if (ring.length >= 2 && samePoint(ring[0], ring[ring.length - 1])) ring = ring.slice(0, -1);
    ring = dedupeConsecutive(ring);
    if (ring.length < 3 || distinctCount(ring) < 3) { stats.waterDroppedDegenerate++; return null; }
    if (ring.length > MAX_POINTS) { stats.waterDroppedTooManyPoints++; return null; }
    return ring;
  };
  const waterNameBuf = (name) => {
    if (!name) return Buffer.alloc(0);
    const buf = truncateUtf8(name, MAX_WATER_NAME_BYTES);
    if (buf.length < Buffer.byteLength(name, 'utf8')) stats.waterNamesTruncated++;
    return buf;
  };
  // The name of an assembled ring: the name tag of its smallest-id named
  // member way (relation inner rings are how "Clark Island" gets its name).
  const ringName = (ring, items) => {
    const byId = new Map(items.map((w) => [w.id, w]));
    for (const id of ring.wayIds.slice().sort((a, b) => a - b)) {
      const w = byId.get(id);
      if (w && w.name) return w.name;
    }
    return '';
  };

  // ══ WATER: multipolygon relations (outer -> kind 1, inner -> kind 2) ══
  const memberConsumed = new Set();
  const inland = []; // { sortA, rank, sortB, nameBuf, ring, unclipped, islands }
  for (const rel of relations.slice().sort((a, b) => a.id - b.id)) {
    const tags = rel.tags || {};
    if (!isWaterTags(tags)) continue;
    stats.waterRelations++;
    if (tags.type !== 'multipolygon') {
      stats.waterRelationsNotMultipolygon++;
      warnings.push(`relation ${rel.id}: type is "${tags.type || ''}", not multipolygon; skipped`);
      continue;
    }
    const groups = { outer: [], inner: [] };
    for (const m of rel.members || []) {
      if (m.type !== 'way') continue; // label nodes, subarea members, etc.
      const role = m.role === 'inner' ? 'inner' : 'outer'; // empty role = outer by OSM convention
      const way = waysById.get(m.ref);
      if (!way) { stats.waterRelationMembersMissing++; continue; }
      memberConsumed.add(m.ref);
      const g = waterWayGeom(way, proj, stats);
      if (g) groups[role].push(g);
    }
    const relName = typeof tags.name === 'string' ? tags.name : '';

    const outerAsm = assembleRings(groups.outer);
    const innerAsm = assembleRings(groups.inner);
    for (const f of [...outerAsm.failures, ...innerAsm.failures]) {
      stats.waterRingsSkipped++;
      warnings.push(`relation ${rel.id}: ring over ways [${f.wayIds.join(', ')}] cannot be `
        + 'closed from the fetched data; ring skipped');
    }
    stats.waterRingsAssembled += outerAsm.rings.length + innerAsm.rings.length;

    const outerRecords = [];
    for (const ring of outerAsm.rings.sort((a, b) => a.minWayId - b.minWayId)) {
      const clipped = clipRingToRect(ring.pts.slice(0, -1), keep);
      if (clipped.length === 0) { stats.waterDroppedOutside++; continue; }
      const finished = finishWaterRing(clipped);
      if (!finished) continue;
      outerRecords.push({
        sortA: rel.id, rank: 1, sortB: ring.minWayId,
        nameBuf: waterNameBuf(relName), ring: finished,
        unclipped: ring.pts, islands: [],
      });
    }
    for (const ring of innerAsm.rings.sort((a, b) => a.minWayId - b.minWayId)) {
      // Attach the island to the outer ring that CONTAINS it (tested against
      // the unclipped outer, since the island may sit outside the clip).
      const parent = outerRecords.find((o) => pointInRing(ring.pts[0], o.unclipped));
      if (!parent) {
        stats.waterIslandsUnassigned++;
        warnings.push(`relation ${rel.id}: inner ring over ways [${ring.wayIds.join(', ')}] `
          + 'is not inside any surviving outer ring; island skipped');
        continue;
      }
      const clipped = clipRingToRect(ring.pts.slice(0, -1), keep);
      if (clipped.length === 0) { stats.waterDroppedOutside++; continue; }
      const finished = finishWaterRing(clipped);
      if (!finished) continue;
      parent.islands.push({
        minWayId: ring.minWayId,
        nameBuf: waterNameBuf(ringName(ring, groups.inner)),
        ring: finished,
      });
    }
    inland.push(...outerRecords);
  }

  // ══ WATER: standalone closed ways tagged natural=water / riverbank ══
  for (const el of [...waysById.values()].sort((a, b) => a.id - b.id)) {
    const tags = el.tags || {};
    if (!isWaterTags(tags)) continue;
    if (memberConsumed.has(el.id)) { stats.waterWaysInRelations++; continue; }
    const g = waterWayGeom(el, proj, stats);
    if (!g) continue;
    stats.waterWaysStandalone++;
    if (typeof tags.building === 'string' && tags.building !== 'no') stats.waterAndBuildingTagged++;
    if (g.nodes[0] !== g.nodes[g.nodes.length - 1]) {
      stats.waterWaysUnclosed++;
      warnings.push(`way ${el.id}: tagged `
        + `${tags.natural === 'water' ? 'natural=water' : 'waterway=riverbank'}`
        + ' but not a closed ring; skipped');
      continue;
    }
    const clipped = clipRingToRect(g.pts.slice(0, -1), keep);
    if (clipped.length === 0) { stats.waterDroppedOutside++; continue; }
    const finished = finishWaterRing(clipped);
    if (!finished) continue;
    inland.push({
      sortA: el.id, rank: 0, sortB: 0,
      nameBuf: waterNameBuf(typeof tags.name === 'string' ? tags.name : ''),
      ring: finished, unclipped: g.pts, islands: [],
    });
  }

  // ══ WATER: the sea, assembled from the coastline ══
  const coastGeoms = [];
  for (const el of waysById.values()) {
    if ((el.tags || {}).natural !== 'coastline') continue;
    const g = waterWayGeom(el, proj, stats);
    if (g) coastGeoms.push(g);
  }
  stats.coastlineWays = coastGeoms.length;
  const chains = stitchCoastChains(coastGeoms, stats);
  stats.coastlineChains = chains.length;
  stats.coastlineClosedChains = chains.filter((c) => c.closed).length;
  const { seas, islands: coastIslands } = assembleSea(chains, exact, warnings, stats);

  const seaRecords = [];
  for (const sea of seas.sort((a, b) => a.minWayId - b.minWayId)) {
    const finished = finishWaterRing(sea.pts);
    if (!finished) continue;
    const area = signedAreaM2(finished);
    if (area >= 0) {
      // Water-on-the-right makes every assembled sea polygon clockwise
      // (negative area); a counter-clockwise result means the assembly and
      // the data disagree somewhere. Emit it anyway (the gates will judge)
      // but say so loudly.
      warnings.push(`assembled sea polygon (smallest way ${sea.minWayId}) is `
        + 'counter-clockwise; coastline direction and assembly disagree');
      stats.seaBoundaryInconsistencies++;
    }
    stats.seaAreaM2 += Math.abs(area);
    seaRecords.push({
      minWayId: sea.minWayId, nameBuf: waterNameBuf(sea.name), ring: finished, islands: [],
    });
  }
  stats.seaPolygons = seaRecords.length;
  for (const isl of coastIslands.sort((a, b) => a.minWayId - b.minWayId)) {
    const finished = finishWaterRing(isl.pts);
    if (!finished) continue;
    const parent = seaRecords.find((s) => pointInRing(finished[0], s.ring));
    if (!parent) {
      stats.waterIslandsUnassigned++;
      warnings.push(`coastline island (ways [${isl.wayIds.join(', ')}]) is not inside any `
        + 'assembled sea polygon; island skipped');
      continue;
    }
    parent.islands.push({ minWayId: isl.minWayId, nameBuf: waterNameBuf(isl.name), ring: finished });
    stats.seaIslands++;
  }

  // ══ WATER: final record order (the byte-determinism contract) ══
  const waters = [];
  const pushWater = (kind, nameBuf, ring) => {
    waters.push({ kind, nameBuf, points: ring });
    stats.waterRecords++;
    stats.waterPoints += ring.length;
  };
  for (const sea of seaRecords) {
    pushWater(WATER_KIND_SEA, sea.nameBuf, sea.ring);
    for (const isl of sea.islands.sort((a, b) => a.minWayId - b.minWayId)) {
      pushWater(WATER_KIND_ISLAND, isl.nameBuf, isl.ring);
    }
  }
  for (const rec of inland.sort((a, b) => (a.sortA - b.sortA) || (a.rank - b.rank) || (a.sortB - b.sortB))) {
    pushWater(WATER_KIND_INLAND, rec.nameBuf, rec.ring);
    stats.waterInlandRecords++;
    for (const isl of rec.islands.sort((a, b) => a.minWayId - b.minWayId)) {
      pushWater(WATER_KIND_ISLAND, isl.nameBuf, isl.ring);
      stats.waterInnerIslands++;
    }
  }

  // ══ ROADS AND BUILDINGS (identical record logic to v1) ══
  for (const el of waysById.values()) {
    const tags = el.tags || {};
    const geom = el.geometry;
    if (!Array.isArray(geom) || geom.length === 0) { stats.noGeometry++; continue; }

    const isRoad = typeof tags.highway === 'string' && ROAD_CLASS.has(tags.highway);
    const isBuilding = typeof tags.building === 'string';
    if (isRoad && isBuilding) stats.dualTagged++; // classified as a road

    const pts = geom.map((p) => proj.project(p.lat, p.lon));

    if (isRoad) {
      stats.roadWaysSeen++;
      const cls = ROAD_CLASS.get(tags.highway);
      const nameBuf = typeof tags.name === 'string' && tags.name.length > 0
        ? truncateUtf8(tags.name, MAX_ROAD_NAME_BYTES)
        : Buffer.alloc(0);
      if (nameBuf.length > 0 && nameBuf.length < Buffer.byteLength(tags.name, 'utf8')) {
        stats.roadNamesTruncated++;
      }

      const deduped = dedupeConsecutive(pts);
      if (deduped.length < 2) { stats.roadWaysDroppedShort++; continue; }

      const runs = clipPolyline(deduped, keep);
      if (runs.length === 0) { stats.roadWaysDroppedOutside++; continue; }
      if (runs.length > 1 || runs[0] !== deduped) stats.roadWaysClipped++;

      let emitted = 0;
      for (const run of runs) {
        const clean = dedupeConsecutive(run);
        if (clean.length < 2) continue;
        // A u16 point_count cannot express more than 65535 points. OSM caps
        // ways at 2000 nodes so this is unreachable today, but splitting on a
        // SHARED point keeps the geometry continuous if that ever changes.
        for (let start = 0; start < clean.length - 1; start += MAX_POINTS - 1) {
          const chunk = clean.slice(start, start + MAX_POINTS);
          if (chunk.length < 2) break;
          if (start > 0) stats.roadWaysSplitForU16++;
          roads.push({ id: el.id, cls, nameBuf, points: chunk });
          stats.roadPoints += chunk.length;
          emitted++;
        }
      }
      if (emitted === 0) { stats.roadWaysDroppedShort++; continue; }
      stats.roadWaysKept++;
      stats.roadExtraRecordsFromSplits += emitted - 1;
      if (nameBuf.length > 0) stats.roadsNamed += emitted;
      continue;
    }

    // Water-tagged ways belong to the water pass above (water wins over a
    // building tag: a pond is a surface, not a structure), coastline ways
    // to the sea assembly, and bare relation-member ways to their relation.
    if (isWaterTags(tags)) continue;

    if (isBuilding) {
      stats.buildingWaysSeen++;
      // building=no is an explicit "there is no building here".
      if (tags.building === 'no') { stats.buildingTagNo++; continue; }

      let ring = pts;
      if (ring.length >= 2 && samePoint(ring[0], ring[ring.length - 1])) {
        ring = ring.slice(0, -1);
        stats.buildingClosingPointsDropped++;
      }
      ring = dedupeConsecutive(ring);
      if (ring.length < 3 || distinctCount(ring) < 3) { stats.buildingDroppedDegenerate++; continue; }
      if (ring.length > MAX_POINTS) { stats.buildingDroppedTooManyPoints++; continue; }
      // Footprints are never clipped (that would deform the polygon), so the
      // keep test is "any vertex inside the keep rectangle".
      if (!ring.some((p) => inRect(p, keep))) { stats.buildingDroppedOutside++; continue; }

      const { height, source } = buildingHeight(tags);
      if (source === 'height') stats.heightFromTag++;
      else if (source === 'levels') stats.heightFromLevels++;
      else if (source === 'unparseable') { stats.heightUnparseable++; stats.heightUnknown++; }
      else stats.heightUnknown++;

      buildings.push({ id: el.id, height, points: ring });
      stats.buildingWaysKept++;
      stats.buildingPoints += ring.length;
      continue;
    }

    if (tags.natural === 'coastline') continue;
    if (memberConsumed.has(el.id)) { stats.relationMemberOnlyWays++; continue; }
    stats.unclassified++;
  }

  // Deterministic ordering: ascending OSM way id, split pieces in order.
  // (Overpass already sorts by id, but relying on that would make a rebuild
  // depend on server behaviour.)
  const stable = (arr) => arr
    .map((r, i) => ({ r, i }))
    .sort((a, b) => (a.r.id - b.r.id) || (a.i - b.i))
    .map((x) => x.r);

  return { roads: stable(roads), buildings: stable(buildings), waters, stats, warnings };
}

/** Serialize records into the HOSMREG2 byte layout. */
function serialize(proj, regionNameBuf, roads, buildings, waters) {
  const header = Buffer.alloc(HEADER_SIZE);
  header.write(MAGIC, 0, 'ascii');
  header.writeUInt32LE(roads.length, 8);
  header.writeUInt32LE(buildings.length, 12);
  header.writeUInt32LE(waters.length, 16);

  const meta = Buffer.alloc(25 + regionNameBuf.length);
  meta.writeDoubleLE(proj.originLat, 0);
  meta.writeDoubleLE(proj.originLon, 8);
  meta.writeFloatLE(proj.halfSpanEast, 16);
  meta.writeFloatLE(proj.halfSpanNorth, 20);
  meta.writeUInt8(regionNameBuf.length, 24);
  regionNameBuf.copy(meta, 25);

  const parts = [header, meta];

  for (const r of roads) {
    const buf = Buffer.alloc(4 + r.nameBuf.length + r.points.length * 8);
    buf.writeUInt8(r.cls, 0);
    buf.writeUInt8(r.nameBuf.length, 1);
    r.nameBuf.copy(buf, 2);
    let o = 2 + r.nameBuf.length;
    buf.writeUInt16LE(r.points.length, o);
    o += 2;
    for (const [e, n] of r.points) {
      buf.writeFloatLE(e, o); buf.writeFloatLE(n, o + 4); o += 8;
    }
    parts.push(buf);
  }

  for (const b of buildings) {
    const buf = Buffer.alloc(6 + b.points.length * 8);
    buf.writeFloatLE(b.height, 0);
    buf.writeUInt16LE(b.points.length, 4);
    let o = 6;
    for (const [e, n] of b.points) {
      buf.writeFloatLE(e, o); buf.writeFloatLE(n, o + 4); o += 8;
    }
    parts.push(buf);
  }

  for (const w of waters) {
    const buf = Buffer.alloc(4 + w.nameBuf.length + w.points.length * 8);
    buf.writeUInt8(w.kind, 0);
    buf.writeUInt8(w.nameBuf.length, 1);
    w.nameBuf.copy(buf, 2);
    let o = 2 + w.nameBuf.length;
    buf.writeUInt16LE(w.points.length, o);
    o += 2;
    for (const [e, n] of w.points) {
      buf.writeFloatLE(e, o); buf.writeFloatLE(n, o + 4); o += 8;
    }
    parts.push(buf);
  }

  return Buffer.concat(parts);
}

// ── Verify ────────────────────────────────────────────────────────────────

let checks = 0;
let failures = 0;
function check(label, ok, detail) {
  checks++;
  if (!ok) failures++;
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail ? ` -- ${detail}` : ''}`);
}

function report() {
  if (failures > 0) {
    console.error(`verify: ${failures}/${checks} checks FAILED`);
    process.exit(1);
  }
  console.log(`verify: all ${checks} checks passed`);
}

/**
 * Great-circle distance in metres, spherical earth (mean radius). Used ONLY
 * by verify, as an INDEPENDENT second opinion on the projection: it shares no
 * code and none of the two magic constants with makeProjection(), so a
 * dropped cos(lat), a degree/radian slip or a swapped lat/lon shows up as a
 * gross disagreement instead of quietly agreeing with itself.
 */
const EARTH_MEAN_RADIUS_M = 6371008.8;
function haversineMetres(lat1, lon1, lat2, lon2) {
  const dLat = (lat2 - lat1) * DEG;
  const dLon = (lon2 - lon1) * DEG;
  const a = Math.sin(dLat / 2) ** 2
    + Math.cos(lat1 * DEG) * Math.cos(lat2 * DEG) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_MEAN_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(a)));
}

/**
 * Re-open the written file, walk every byte of it, and assert everything the
 * Rust reader is entitled to assume. `source` (the Overpass JSON) and `bbox`
 * are present on a build run and unlock the cross-checks against the input;
 * --verify-only runs the structural and region gates only.
 */
function verify(path, { source = null, bbox = null, expectedName = null, determinismOk = null } = {}) {
  console.log(`verify: re-opening ${path}`);
  const bin = readFileSync(path);
  const gates = REGION_GATES[basename(path)] || DEFAULT_GATES;
  const gateName = REGION_GATES[basename(path)] ? basename(path) : 'default (no region entry)';

  // The Rust parser hardcodes these numbers, so assert the LITERALS, not the
  // constants above: otherwise editing a constant would move the whole format
  // while every self-consistent check kept passing.
  check('spec literals: magic "HOSMREG2", 20-byte header, 25-byte meta stem, 40-byte road and water name caps',
    MAGIC === 'HOSMREG2' && HEADER_SIZE === 20 && MAX_ROAD_NAME_BYTES === 40
    && MAX_WATER_NAME_BYTES === 40 && MAX_WATER_KIND === 2
    && M_PER_DEG_LON_EQ === 111320.0 && M_PER_DEG_LAT === 110540.0,
    `magic "${MAGIC}", header ${HEADER_SIZE}, name caps ${MAX_ROAD_NAME_BYTES}/${MAX_WATER_NAME_BYTES}, ` +
    `lon ${M_PER_DEG_LON_EQ} m/deg, lat ${M_PER_DEG_LAT} m/deg`);

  if (bin.length < HEADER_SIZE + 25) {
    check('file is at least header + meta stem', false, `${bin.length} bytes`);
    return report();
  }

  const magic = bin.subarray(0, 8).toString('ascii');
  check('magic is "HOSMREG2"', magic === MAGIC, `got "${magic}"`);
  if (magic !== MAGIC) return report();

  const roadCount = bin.readUInt32LE(8);
  const buildingCount = bin.readUInt32LE(12);
  const waterCount = bin.readUInt32LE(16);
  const originLat = bin.readDoubleLE(20);
  const originLon = bin.readDoubleLE(28);
  const halfE = bin.readFloatLE(36);
  const halfN = bin.readFloatLE(40);
  const nameLen = bin.readUInt8(44);
  const nameBytes = bin.subarray(45, 45 + nameLen);
  const regionName = nameBytes.toString('utf8');

  check('meta: origin is a real lat/lon',
    Number.isFinite(originLat) && Number.isFinite(originLon)
    && Math.abs(originLat) <= 90 && Math.abs(originLon) <= 180,
    `${originLat.toFixed(6)}, ${originLon.toFixed(6)}`);
  check('meta: half spans are positive and finite',
    Number.isFinite(halfE) && Number.isFinite(halfN) && halfE > 0 && halfN > 0,
    `east ${halfE.toFixed(1)} m, north ${halfN.toFixed(1)} m`);
  check('meta: region name fits and is valid UTF-8',
    45 + nameLen <= bin.length && nameLen <= MAX_REGION_NAME_BYTES
    && Buffer.byteLength(regionName, 'utf8') === nameLen,
    `"${regionName}" (${nameLen} bytes)`);
  if (expectedName !== null) {
    check('meta: region name is the one requested on the command line',
      regionName === expectedName, `file "${regionName}" vs argument "${expectedName}"`);
  }

  // ── Walk every record. Any overrun, any impossible field, and the walk
  // stops with structureOk=false rather than reading garbage.
  let off = 45 + nameLen;
  const roads = [];
  const buildings = [];
  const waters = [];
  let structureOk = true;
  let stopReason = '';
  const fail = (why) => { structureOk = false; stopReason = why; };

  for (let i = 0; i < roadCount && structureOk; i++) {
    if (off + 2 > bin.length) { fail(`road ${i}: truncated header at ${off}`); break; }
    const cls = bin.readUInt8(off);
    const nlen = bin.readUInt8(off + 1);
    if (cls > MAX_ROAD_CLASS) { fail(`road ${i}: class ${cls} > ${MAX_ROAD_CLASS}`); break; }
    if (nlen > MAX_ROAD_NAME_BYTES) { fail(`road ${i}: name_len ${nlen} > ${MAX_ROAD_NAME_BYTES}`); break; }
    if (off + 2 + nlen + 2 > bin.length) { fail(`road ${i}: truncated name/count at ${off}`); break; }
    const nameRaw = bin.subarray(off + 2, off + 2 + nlen);
    const name = nameRaw.toString('utf8');
    if (Buffer.byteLength(name, 'utf8') !== nlen) { fail(`road ${i}: name is not valid UTF-8`); break; }
    const pc = bin.readUInt16LE(off + 2 + nlen);
    if (pc < 2) { fail(`road ${i}: point_count ${pc} < 2`); break; }
    const pointsOff = off + 4 + nlen;
    if (pointsOff + pc * 8 > bin.length) { fail(`road ${i}: truncated points at ${pointsOff}`); break; }
    roads.push({ cls, name, pc, pointsOff });
    off = pointsOff + pc * 8;
  }

  const buildingsStart = off;
  for (let i = 0; i < buildingCount && structureOk; i++) {
    if (off + 6 > bin.length) { fail(`building ${i}: truncated header at ${off}`); break; }
    const height = bin.readFloatLE(off);
    const pc = bin.readUInt16LE(off + 4);
    if (!Number.isFinite(height) || height < 0 || height > MAX_BUILDING_HEIGHT_M) {
      fail(`building ${i}: height ${height} outside [0, ${MAX_BUILDING_HEIGHT_M}]`); break;
    }
    if (pc < 3) { fail(`building ${i}: point_count ${pc} < 3`); break; }
    if (off + 6 + pc * 8 > bin.length) { fail(`building ${i}: truncated points at ${off + 6}`); break; }
    buildings.push({ height, pc, pointsOff: off + 6 });
    off += 6 + pc * 8;
  }

  // ── Water records: kind, name, ring. The walk enforces every field range
  // the Rust reader is entitled to assume, and tracks each island's parent
  // kind (the nearest preceding non-island record) for the gates below.
  const watersStart = off;
  let lastParentKind = null;
  for (let i = 0; i < waterCount && structureOk; i++) {
    if (off + 2 > bin.length) { fail(`water ${i}: truncated header at ${off}`); break; }
    const kind = bin.readUInt8(off);
    const nlen = bin.readUInt8(off + 1);
    if (kind > MAX_WATER_KIND) { fail(`water ${i}: kind ${kind} > ${MAX_WATER_KIND}`); break; }
    if (nlen > MAX_WATER_NAME_BYTES) { fail(`water ${i}: name_len ${nlen} > ${MAX_WATER_NAME_BYTES}`); break; }
    if (off + 2 + nlen + 2 > bin.length) { fail(`water ${i}: truncated name/count at ${off}`); break; }
    const nameRaw = bin.subarray(off + 2, off + 2 + nlen);
    const name = nameRaw.toString('utf8');
    if (Buffer.byteLength(name, 'utf8') !== nlen) { fail(`water ${i}: name is not valid UTF-8`); break; }
    const pc = bin.readUInt16LE(off + 2 + nlen);
    if (pc < 3) { fail(`water ${i}: point_count ${pc} < 3`); break; }
    const pointsOff = off + 4 + nlen;
    if (pointsOff + pc * 8 > bin.length) { fail(`water ${i}: truncated points at ${pointsOff}`); break; }
    if (kind !== WATER_KIND_ISLAND) lastParentKind = kind;
    waters.push({
      kind, name, pc, pointsOff,
      parentKind: kind === WATER_KIND_ISLAND ? lastParentKind : null,
    });
    off = pointsOff + pc * 8;
  }

  check(`walked all ${roadCount} road + ${buildingCount} building + ${waterCount} water records`,
    structureOk && roads.length === roadCount && buildings.length === buildingCount
    && waters.length === waterCount,
    structureOk ? `${roads.length} roads, ${buildings.length} buildings, ${waters.length} water`
      : `stopped: ${stopReason}`);
  check('records end exactly at EOF', structureOk && off === bin.length,
    `ended at ${off}, file is ${bin.length}`);
  if (!structureOk) return report();

  // Independent size arithmetic, using the literals a Rust reader would use.
  const expectedLen = 20 + 25 + nameLen
    + roads.reduce((a, r) => a + 4 + Buffer.byteLength(r.name, 'utf8') + 8 * r.pc, 0)
    + buildings.reduce((a, b) => a + 6 + 8 * b.pc, 0)
    + waters.reduce((a, w) => a + 4 + Buffer.byteLength(w.name, 'utf8') + 8 * w.pc, 0);
  check('file length matches 20 + 25 + name + sum(road) + sum(building) + sum(water)',
    expectedLen === bin.length, `computed ${expectedLen}, actual ${bin.length}`);

  check(`every road class is 0..=${MAX_ROAD_CLASS}`,
    roads.every((r) => r.cls <= MAX_ROAD_CLASS));

  // ── Coordinates: finite, and inside the region plus its margin slop.
  const boundE = halfE + COORD_SLOP_M;
  const boundN = halfN + COORD_SLOP_M;
  let allFinite = true;
  let worstE = 0; let worstN = 0;
  let minE = Infinity; let maxE = -Infinity; let minN = Infinity; let maxN = -Infinity;
  const scan = (recs) => {
    for (const r of recs) {
      for (let k = 0; k < r.pc; k++) {
        const e = bin.readFloatLE(r.pointsOff + k * 8);
        const n = bin.readFloatLE(r.pointsOff + k * 8 + 4);
        if (!Number.isFinite(e) || !Number.isFinite(n)) { allFinite = false; continue; }
        if (Math.abs(e) > worstE) worstE = Math.abs(e);
        if (Math.abs(n) > worstN) worstN = Math.abs(n);
        if (e < minE) minE = e; if (e > maxE) maxE = e;
        if (n < minN) minN = n; if (n > maxN) maxN = n;
      }
    }
  };
  scan(roads); scan(buildings); scan(waters);
  check('every coordinate is finite', allFinite);
  check(`every coordinate is inside the half spans + ${COORD_SLOP_M} m`,
    worstE <= boundE && worstN <= boundN,
    `worst |east| ${worstE.toFixed(1)} m (bound ${boundE.toFixed(1)}), ` +
    `worst |north| ${worstN.toFixed(1)} m (bound ${boundN.toFixed(1)})`);

  // ── Water ring conventions, re-checked from the BYTES rather than the
  // build's in-memory rings: closing point removed, no consecutive
  // duplicates, 3+ distinct points, sea confined to the exact bbox, and
  // islands only ever following a parent water record.
  const readRing = (w) => {
    const ring = [];
    for (let k = 0; k < w.pc; k++) {
      ring.push([bin.readFloatLE(w.pointsOff + k * 8), bin.readFloatLE(w.pointsOff + k * 8 + 4)]);
    }
    return ring;
  };
  let ringsOk = true;
  let ringProblem = 'all rings clean';
  for (let i = 0; i < waters.length && ringsOk; i++) {
    const ring = readRing(waters[i]);
    if (distinctCount(ring) < 3) {
      ringsOk = false; ringProblem = `water ${i}: fewer than 3 distinct points`;
    } else if (samePoint(ring[0], ring[ring.length - 1])) {
      ringsOk = false; ringProblem = `water ${i}: duplicated closing point survived`;
    } else {
      for (let k = 1; k < ring.length; k++) {
        if (samePoint(ring[k - 1], ring[k])) {
          ringsOk = false; ringProblem = `water ${i}: consecutive duplicate at point ${k}`;
          break;
        }
      }
    }
  }
  check('every water ring has 3+ distinct points, no closing duplicate, no consecutive duplicates',
    ringsOk, ringProblem);

  let seaInBbox = true;
  for (const w of waters) {
    if (w.kind !== WATER_KIND_SEA) continue;
    for (const [e, n] of readRing(w)) {
      if (Math.abs(e) > halfE + 0.5 || Math.abs(n) > halfN + 0.5) { seaInBbox = false; break; }
    }
    if (!seaInBbox) break;
  }
  check('sea polygons are clipped to the exact bbox (every kind 0 point within half spans + 0.5 m)',
    seaInBbox);

  check('a kind 2 island only ever follows a kind 0 or kind 1 record',
    waters.every((w) => w.kind !== WATER_KIND_ISLAND || w.parentKind !== null));

  // ── Region plausibility gates.
  const named = roads.filter((r) => r.name.length > 0);
  const withHeight = buildings.filter((b) => b.height > 0);
  check(`gates [${gateName}]: at least ${gates.minRoads} roads`,
    roadCount >= gates.minRoads, `${roadCount} roads`);
  check(`gates [${gateName}]: at least ${gates.minBuildings} buildings`,
    buildingCount >= gates.minBuildings, `${buildingCount} buildings`);
  check(`gates [${gateName}]: at least ${gates.minBuildingsWithHeight} buildings carry a nonzero height`,
    withHeight.length >= gates.minBuildingsWithHeight, `${withHeight.length} with height`);
  if (gates.anyRoadNamed.length > 0) {
    const hit = gates.anyRoadNamed.filter((want) => named.some((r) => r.name === want));
    check(`gates [${gateName}]: a road named ${gates.anyRoadNamed.map((n) => `"${n}"`).join(' or ')} exists`,
      hit.length > 0, hit.length ? `found ${hit.map((n) => `"${n}"`).join(', ')}` : 'found none');
  }

  // ── Water gates: counts, shoelace area, named records, and the sea
  // side-convention proof by point-in-polygon in region metre space.
  const seasV = waters.filter((w) => w.kind === WATER_KIND_SEA);
  const inlandV = waters.filter((w) => w.kind === WATER_KIND_INLAND);
  const islandsV = waters.filter((w) => w.kind === WATER_KIND_ISLAND);
  const seaAreaKm2 = seasV.reduce((a, w) => a + Math.abs(signedAreaM2(readRing(w))), 0) / 1e6;
  check(`gates [${gateName}]: at least ${gates.minSeaPolygons} sea polygons`,
    seasV.length >= gates.minSeaPolygons, `${seasV.length} sea polygons`);
  if (gates.minSeaAreaKm2 > 0) {
    check(`gates [${gateName}]: total sea area over ${gates.minSeaAreaKm2} km^2 (shoelace)`,
      seaAreaKm2 > gates.minSeaAreaKm2, `${seaAreaKm2.toFixed(3)} km^2`);
  }
  check(`gates [${gateName}]: at least ${gates.minInlandWater} inland water records`,
    inlandV.length >= gates.minInlandWater, `${inlandV.length} inland`);
  if (gates.anyInlandWaterNamed.length > 0) {
    const hit = gates.anyInlandWaterNamed.filter((want) => inlandV.some((w) => w.name === want));
    check(`gates [${gateName}]: inland water named ${gates.anyInlandWaterNamed.map((n) => `"${n}"`).join(' or ')} exists`,
      hit.length > 0, hit.length ? `found ${hit.map((n) => `"${n}"`).join(', ')}` : 'found none');
  }
  if (gates.anyIslandNamed.length > 0) {
    const hit = gates.anyIslandNamed.filter((want) => islandsV.some((w) => w.name === want));
    check(`gates [${gateName}]: an island named ${gates.anyIslandNamed.map((n) => `"${n}"`).join(' or ')} exists`,
      hit.length > 0, hit.length ? `found ${hit.map((n) => `"${n}"`).join(', ')}` : 'found none');
  }
  // Known coordinates projected through the FILE's own meta (origin + the
  // two spec constants), tested against the rings read back from the bytes.
  // Inside the sea = in some kind 0 ring and not inside any island the sea
  // carries; outside = in no kind 0 ring at all. This is the proof that the
  // land-left/water-right walking direction was derived correctly.
  const projectGate = ([lat, lon]) => [
    (lon - originLon) * Math.cos(originLat * DEG) * M_PER_DEG_LON_EQ,
    (lat - originLat) * M_PER_DEG_LAT,
  ];
  const seaIslandsV = islandsV.filter((w) => w.parentKind === WATER_KIND_SEA);
  for (const ll of gates.seaContainsLatLon) {
    const pt = projectGate(ll);
    const inSea = seasV.some((w) => pointInRing(pt, readRing(w)))
      && !seaIslandsV.some((w) => pointInRing(pt, readRing(w)));
    check(`gates [${gateName}]: open-water point ${ll[0]}, ${ll[1]} is INSIDE the assembled sea`,
      inSea, `${pt[0].toFixed(1)} m E, ${pt[1].toFixed(1)} m N`);
  }
  for (const ll of gates.seaExcludesLatLon) {
    const pt = projectGate(ll);
    const inSea = seasV.some((w) => pointInRing(pt, readRing(w)));
    check(`gates [${gateName}]: on-land point ${ll[0]}, ${ll[1]} is OUTSIDE the assembled sea`,
      !inSea, `${pt[0].toFixed(1)} m E, ${pt[1].toFixed(1)} m N`);
  }

  if (determinismOk !== null) {
    check('rebuild from the same parsed data is byte-identical (convert + serialize run twice)',
      determinismOk === true);
  }

  // ── Meta vs the requested bbox (build runs only).
  if (bbox) {
    const p = makeProjection(bbox);
    check('meta origin is the bbox centre',
      Math.abs(originLat - p.originLat) < 1e-9 && Math.abs(originLon - p.originLon) < 1e-9,
      `file ${originLat.toFixed(9)},${originLon.toFixed(9)} vs bbox centre ${p.originLat.toFixed(9)},${p.originLon.toFixed(9)}`);
    check('meta half spans match the projection of the bbox corners',
      Math.abs(halfE - p.halfSpanEast) < 0.01 && Math.abs(halfN - p.halfSpanNorth) < 0.01,
      `file ${halfE.toFixed(3)} x ${halfN.toFixed(3)} m vs computed ${p.halfSpanEast.toFixed(3)} x ${p.halfSpanNorth.toFixed(3)} m`);

    // INDEPENDENT projection opinion: the half spans, re-derived by haversine
    // from the origin to the bbox edges. Different formula, different earth
    // constant, so this catches a missing cos(lat), a radian/degree slip or a
    // lat/lon swap. It does NOT catch the ~0.6% the fixed 110540 m/deg
    // latitude constant runs short at this latitude, which is expected and
    // documented in the header; tolerance is set just wide of it.
    const trueHalfN = haversineMetres(p.originLat, p.originLon, bbox.north, p.originLon);
    const trueHalfE = haversineMetres(p.originLat, p.originLon, p.originLat, bbox.east);
    const errN = Math.abs(halfN - trueHalfN) / trueHalfN;
    const errE = Math.abs(halfE - trueHalfE) / trueHalfE;
    check('half spans agree with an independent haversine measurement to 2%',
      errN < 0.02 && errE < 0.02,
      `east ${halfE.toFixed(1)} vs ${trueHalfE.toFixed(1)} m (${(errE * 100).toFixed(2)}%), ` +
      `north ${halfN.toFixed(1)} vs ${trueHalfN.toFixed(1)} m (${(errN * 100).toFixed(2)}%)`);
  }

  // ── Cross-checks against the Overpass answer (build runs only). These are
  // written independently of convert(): they re-derive the expected result
  // straight from the source tags rather than reusing the build's helpers.
  if (source) {
    const keep = {
      minE: -halfE - KEEP_MARGIN_M, maxE: halfE + KEEP_MARGIN_M,
      minN: -halfN - KEEP_MARGIN_M, maxN: halfN + KEEP_MARGIN_M,
    };
    // Projection rebuilt from the FILE's own meta, not from the build's
    // closure, so a wrong origin in the meta block shows up here as a count
    // or extent disagreement instead of cancelling out.
    const lonScale = Math.cos(originLat * DEG) * M_PER_DEG_LON_EQ;

    /**
     * Height, re-implemented from the SPEC TEXT (plain metres, else levels
     * times 3, else 0) rather than reusing buildingHeight(): a regex bug in
     * the build parser must not be able to agree with itself. Tag values that
     * are not a plain number fall back to the shared parser and are counted,
     * so the coverage of this cross-check is reported honestly.
     */
    let fallbacks = 0;
    const independentHeight = (tags) => {
      const raw = typeof tags.height === 'string' ? tags.height.trim() : '';
      if (raw !== '') {
        let h;
        if (/^\d+(\.\d+)?$/.test(raw)) h = Number(raw);
        else if (/^\d+(\.\d+)?\s*m$/i.test(raw)) h = Number(raw.replace(/\s*m$/i, ''));
        else { h = parseHeightTag(raw); fallbacks++; }
        if (Number.isFinite(h) && h > 0 && h <= MAX_BUILDING_HEIGHT_M) return h;
      }
      const lv = typeof tags['building:levels'] === 'string' ? tags['building:levels'].trim() : '';
      if (/^\d+(\.\d+)?/.test(lv)) {
        const levels = parseFloat(lv);
        if (levels > 0 && levels * METRES_PER_LEVEL <= MAX_BUILDING_HEIGHT_M) return levels * METRES_PER_LEVEL;
      }
      return 0;
    };

    let srcBuildings = 0;
    let srcRoadWays = 0;
    let srcTallest = 0;
    let srcTallestPoints = 0;
    const srcHeights = [];
    const countedWays = new Set(); // way(r) can print a way twice; count once
    for (const el of source.elements) {
      if (el.type !== 'way' || !Array.isArray(el.geometry) || el.geometry.length === 0) continue;
      if (countedWays.has(el.id)) continue;
      countedWays.add(el.id);
      const tags = el.tags || {};
      if (typeof tags.highway === 'string' && ROAD_CLASS.has(tags.highway)) { srcRoadWays++; continue; }
      // Water-tagged ways belong to the water records, never to buildings
      // (mirrors the build's classification, re-derived from the spec text).
      if (tags.natural === 'water' || tags.waterway === 'riverbank') continue;
      if (typeof tags.building !== 'string' || tags.building === 'no') continue;
      const ring = el.geometry.map((p) => [
        (p.lon - originLon) * lonScale, (p.lat - originLat) * M_PER_DEG_LAT,
      ]);
      const closed = ring.length >= 2 && samePoint(ring[0], ring[ring.length - 1]);
      const open = closed ? ring.slice(0, -1) : ring;
      const distinct = distinctCount(open);
      if (distinct < 3) continue;
      if (!ring.some((p) => inRect(p, keep))) continue;
      srcBuildings++;
      const h = independentHeight(tags);
      srcHeights.push(h);
      if (h > srcTallest) { srcTallest = h; srcTallestPoints = dedupeConsecutive(open).length; }
    }

    check('building_count equals an independent recount of eligible source ways',
      srcBuildings === buildingCount, `source ${srcBuildings} vs file ${buildingCount}`);

    // Buildings are 1:1 with source ways, so the two height multisets must be
    // identical, not merely similar. This catches a dropped tag, an off-by-one
    // in the levels fallback, and a mis-ordered record walk at once.
    const fileHeights = buildings.map((b) => b.height).sort((a, b) => b - a);
    const srcSorted = srcHeights.slice().sort((a, b) => b - a);
    let worstHeightDelta = Infinity;
    if (fileHeights.length === srcSorted.length) {
      worstHeightDelta = 0;
      for (let i = 0; i < fileHeights.length; i++) {
        worstHeightDelta = Math.max(worstHeightDelta, Math.abs(fileHeights[i] - srcSorted[i]));
      }
    }
    check('every building height matches an independent parse of the source tags',
      worstHeightDelta < 1e-3,
      fileHeights.length === srcSorted.length
        ? `worst delta ${worstHeightDelta.toExponential(2)} m over ${fileHeights.length} buildings ` +
          `(${fallbacks} tag values needed the imperial parser)`
        : `count mismatch: file ${fileHeights.length} vs source ${srcSorted.length}`);
    check('tallest building in the file is the tallest in the source',
      Math.abs((fileHeights[0] || 0) - srcTallest) < 1e-3,
      `file ${(fileHeights[0] || 0).toFixed(2)} m vs source ${srcTallest.toFixed(2)} m (${srcTallestPoints} pts)`);

    // Roads can split at the margin and can be dropped when fully outside, so
    // the record count is not equal to the source way count -- but it must
    // stay within a couple of percent of it. A layer silently emptied, or a
    // clip that shattered every way, both land far outside this band.
    const roadDrift = Math.abs(roadCount - srcRoadWays) / Math.max(1, srcRoadWays);
    check('road_count stays within 2% of the source road-way count (clip splits and outside drops only)',
      roadDrift < 0.02,
      `source road ways ${srcRoadWays}, file road records ${roadCount} ` +
      `(${roadCount >= srcRoadWays ? '+' : ''}${roadCount - srcRoadWays}, ${(roadDrift * 100).toFixed(2)}%)`);
  }

  // ── Summary a human can eyeball.
  const classHist = new Array(MAX_ROAD_CLASS + 1).fill(0);
  for (const r of roads) classHist[r.cls]++;
  const tallest = buildings.reduce((best, b) => (b.height > (best ? best.height : -1) ? b : best), null);
  const bytes = statSync(path).size;
  console.log(
    `verify: "${regionName}" at ${originLat.toFixed(5)}, ${originLon.toFixed(5)} | ` +
    `${roadCount} roads (${named.length} named), ${buildingCount} buildings ` +
    `(${withHeight.length} with height), ${waterCount} water | ` +
    `${bytes} bytes (${(bytes / 1024).toFixed(1)} KiB)`
  );
  const kindLabel = ['sea', 'inland', 'island'];
  const namedWater = waters.filter((w) => w.name.length > 0);
  console.log(
    `verify: water -- ${seasV.length} sea (${seaAreaKm2.toFixed(2)} km^2 by shoelace), ` +
    `${inlandV.length} inland, ${islandsV.length} islands` +
    (namedWater.length > 0
      ? ` | named: ${namedWater.slice(0, 4).map((w) => `"${w.name}" (${kindLabel[w.kind]})`).join(', ')}` +
        (namedWater.length > 4 ? ` and ${namedWater.length - 4} more` : '')
      : ' | none named')
  );
  console.log(
    `verify: extent east [${minE.toFixed(1)}, ${maxE.toFixed(1)}] m, ` +
    `north [${minN.toFixed(1)}, ${maxN.toFixed(1)}] m ` +
    `(half spans ${halfE.toFixed(1)} x ${halfN.toFixed(1)} m + ${KEEP_MARGIN_M} m margin)`
  );
  console.log('verify: road classes ' +
    classHist.map((c, i) => `${i}=${c} (${CLASS_LABEL[i]})`).join(', '));
  if (tallest) {
    console.log(`verify: tallest building ${tallest.height.toFixed(1)} m with ${tallest.pc} footprint points`);
  }
  // Three named roads, spread through the file, with their first point, so a
  // human can sanity-check the geometry against a real map.
  // (i < named.length matters: a region with no named roads at all must print
  // a note, not index past the end and crash before report() runs.)
  const samples = [0, Math.floor(named.length / 2), named.length - 1]
    .filter((i, k, a) => i >= 0 && i < named.length && a.indexOf(i) === k)
    .slice(0, 3)
    .map((i) => named[i]);
  if (samples.length === 0) console.log('verify: no named roads in this region, no samples to show');
  for (const s of samples) {
    const e = bin.readFloatLE(s.pointsOff);
    const n = bin.readFloatLE(s.pointsOff + 4);
    const lat = originLat + n / M_PER_DEG_LAT;
    const lon = originLon + e / (Math.cos(originLat * DEG) * M_PER_DEG_LON_EQ);
    console.log(
      `verify: sample road "${s.name}" (class ${s.cls}, ${s.pc} pts) starts at ` +
      `${e.toFixed(1)} m E, ${n.toFixed(1)} m N = ${lat.toFixed(6)}, ${lon.toFixed(6)}`
    );
  }
  console.log(`verify: data (c) OpenStreetMap contributors, ODbL 1.0 -- credit required wherever this file is drawn`);

  // buildingsStart / watersStart are only used for the debug line below;
  // keeping them named documents the section starts for anyone hex-diving.
  if (process.env.HOSMREG_DEBUG) {
    console.log(`verify: building section starts at byte ${buildingsStart}, water section at ${watersStart}`);
  }

  return report();
}

// ── Main ──────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv);

  if (args.verifyOnly) {
    verify(resolve(args.verifyOnly));
    return;
  }

  if (!args.bbox) die('--bbox south,west,north,east is required (or --verify-only <file>)');
  if (!args.name) die('--name "<display name>" is required');
  if (!args.out) die('--out <path>.bin is required');

  const bbox = parseBbox(args.bbox);
  const outPath = resolve(args.out);
  const regionNameBuf = truncateUtf8(args.name, MAX_REGION_NAME_BYTES);
  if (regionNameBuf.length === 0) die('--name must not be empty');

  const proj = makeProjection(bbox);
  console.log(
    `region "${regionNameBuf.toString('utf8')}": bbox ${bbox.south},${bbox.west},${bbox.north},${bbox.east} | ` +
    `origin ${proj.originLat.toFixed(6)}, ${proj.originLon.toFixed(6)} | ` +
    `half spans ${proj.halfSpanEast.toFixed(1)} x ${proj.halfSpanNorth.toFixed(1)} m ` +
    `(${(proj.halfSpanEast * 2 / 1000).toFixed(2)} x ${(proj.halfSpanNorth * 2 / 1000).toFixed(2)} km)`
  );

  const query = buildQuery(bbox);
  console.log(`overpass: POST ${OVERPASS_URL}\n${query.split('\n').map((l) => `  | ${l}`).join('\n')}`);

  const t0 = Date.now();
  const text = await overpassFetch(query);
  const fetchMs = Date.now() - t0;
  const json = parseOverpass(text);
  console.log(`overpass: ${text.length} bytes, ${json.elements.length} elements in ${fetchMs} ms`);

  const t1 = Date.now();
  const { roads, buildings, waters, stats, warnings } = convert(json, proj);
  for (const w of warnings) console.warn(`convert: WARNING: ${w}`);
  console.log(
    `convert: roads -- ${stats.roadWaysSeen} source ways, ${stats.roadWaysKept} kept, ` +
    `${stats.roadWaysClipped} clipped at the ${KEEP_MARGIN_M} m margin ` +
    `(+${stats.roadExtraRecordsFromSplits} extra records from splits), ` +
    `${stats.roadWaysDroppedOutside} dropped outside, ${stats.roadWaysDroppedShort} dropped as < 2 points, ` +
    `${roads.length} records / ${stats.roadPoints} points, ` +
    `${stats.roadsNamed} named (${stats.roadNamesTruncated} names truncated to ${MAX_ROAD_NAME_BYTES} bytes)`
  );
  console.log(
    `convert: buildings -- ${stats.buildingWaysSeen} source ways, ${stats.buildingWaysKept} kept, ` +
    `${stats.buildingTagNo} dropped building=no, ${stats.buildingDroppedDegenerate} dropped degenerate, ` +
    `${stats.buildingDroppedOutside} dropped outside, ${stats.buildingDroppedTooManyPoints} dropped over ${MAX_POINTS} points, ` +
    `${stats.buildingClosingPointsDropped} closing points removed, ${stats.buildingPoints} points | ` +
    `height: ${stats.heightFromTag} from tag, ${stats.heightFromLevels} from levels, ` +
    `${stats.heightUnknown} unknown (${stats.heightUnparseable} unparseable tags)`
  );
  console.log(
    `convert: water/sea -- ${stats.coastlineWays} coastline ways -> ${stats.coastlineChains} chains ` +
    `(${stats.coastlineClosedChains} closed), ${stats.seaPieces} bbox-crossing pieces -> ` +
    `${stats.seaPolygons} sea polygons (${(stats.seaAreaM2 / 1e6).toFixed(2)} km^2) + ` +
    `${stats.seaIslands} coastline islands` +
    (stats.seaRunsSkippedInsideEnd || stats.seaPolygonsAbandoned || stats.seaBoundaryInconsistencies
      ? ` | PROBLEMS: ${stats.seaRunsSkippedInsideEnd} runs skipped, ` +
        `${stats.seaPolygonsAbandoned} polygons abandoned, ` +
        `${stats.seaBoundaryInconsistencies} boundary inconsistencies`
      : '')
  );
  console.log(
    `convert: water/inland -- ${stats.waterRelations} relations ` +
    `(${stats.waterRingsAssembled} rings assembled, ${stats.waterRelationsNotMultipolygon} not multipolygon, ` +
    `${stats.waterRelationMembersMissing} members unfetched), ` +
    `${stats.waterWaysStandalone} standalone ways (${stats.waterWaysUnclosed} unclosed, ` +
    `${stats.waterWaysInRelations} consumed by relations) | ` +
    `${stats.waterInlandRecords} inland records + ${stats.waterInnerIslands} inner islands, ` +
    `${stats.waterDroppedOutside} dropped outside, ${stats.waterDroppedDegenerate} degenerate, ` +
    `${stats.waterIslandsUnassigned} islands unassigned, ${stats.waterNamesTruncated} names truncated`
  );
  console.log(
    `convert: water -- ${waters.length} records / ${stats.waterPoints} points | ` +
    `skipped rings total: ${stats.waterRingsSkipped}`
  );
  if (stats.notWay || stats.noGeometry || stats.unclassified || stats.dualTagged
    || stats.relationsSeen || stats.duplicateElements || stats.relationMemberOnlyWays) {
    console.log(
      `convert: other -- ${stats.notWay} non-way elements, ${stats.relationsSeen} relations, ` +
      `${stats.duplicateElements} duplicate way prints, ${stats.relationMemberOnlyWays} bare member ways, ` +
      `${stats.noGeometry} ways without geometry, ${stats.unclassified} ways matching neither filter, ` +
      `${stats.dualTagged} tagged both road and building (kept as road)`
    );
  }

  const bin = serialize(proj, regionNameBuf, roads, buildings, waters);

  // Byte-determinism proof: convert and serialize AGAIN from the same parsed
  // answer; the ordering contract in the spec says the two buffers must be
  // identical. (The second run's warnings are the same set; not re-printed.)
  const again = convert(json, proj);
  const bin2 = serialize(proj, regionNameBuf, again.roads, again.buildings, again.waters);
  const determinismOk = bin.equals(bin2);

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, bin);
  console.log(
    `write: ${outPath} -- ${bin.length} bytes (${(bin.length / 1024).toFixed(1)} KiB), ` +
    `converted + serialized in ${Date.now() - t1} ms`
  );

  verify(outPath, { source: json, bbox, expectedName: regionNameBuf.toString('utf8'), determinismOk });
}

await main();
