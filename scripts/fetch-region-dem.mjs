#!/usr/bin/env node
/**
 * fetch-region-dem.mjs
 *
 * Fetches REAL public-domain elevation for one shipped OSM region's bounding
 * box (plus a 2 km margin) and writes data/maps/regions/<region>.dem.bin --
 * a compact quantized digital elevation model (DEM) the Rust side overlays
 * on the global heightmap. The global heightmap has ~460 m cells, far too
 * coarse for coastal gradients, so carved water edges (Dyes Inlet, Elliott
 * Bay) currently read as cliffs; this file supplies the missing ~13 m
 * detail under each region.
 *
 * WHEN THIS RUNS: at DEVELOPMENT / BUILD time, by hand, once per region --
 * exactly like scripts/fetch-osm-region.mjs, whose .bin this script reads
 * for the region's georeference. THE SHIPPED APP NEVER CALLS THE TILE
 * SERVICE. It only reads the .dem.bin committed next to the region .bin.
 *
 * DATA SOURCE: the AWS Open Data "Terrain Tiles" (the Mapzen / Tilezen
 * "terrarium" encoding), a public S3 bucket, no auth, no API key:
 *
 *     https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png
 *
 * Underlying sources for the shipped Puget Sound regions are United States
 * public-domain datasets (USGS 3DEP/NED, NASA SRTM, NOAA bathymetry). The
 * dataset's attribution guidance:
 *     https://github.com/tilezen/joerd/blob/master/docs/attribution.md
 *     https://registry.opendata.aws/terrain-tiles/
 * A one-off dev-time fetch of a few dozen tiles, cached to a committed
 * file, is exactly the intended use of the open bucket. A real identifying
 * User-Agent is sent anyway (same habit as the Overpass fetcher).
 *
 * TERRARIUM DECODE (the pixel contract): each PNG pixel encodes elevation as
 *     elevation_m = (R * 256 + G + B / 256) - 32768
 * giving 1/256 m steps over [-32768, +32768) m. Tiles are 256 x 256, 8-bit,
 * RGB or RGBA, non-interlaced. Zoom 13 is used: the native ground spacing
 * at 47.6 deg latitude is ~12.9 m/px on both axes (156543 * cos(lat) /
 * 2^13), roughly 35x finer than the 460 m global heightmap cells. Ocean
 * pixels can read 0 or negative (the source composites bathymetry); values
 * are stored EXACTLY as sampled, nothing is clamped -- the Rust side
 * decides what to do below sea level.
 *
 * The PNG reader is implemented here with zero new dependencies (node core
 * zlib inflates the IDAT stream; chunk walk, IHDR validation and the five
 * per-scanline filters 0..4 are ~80 lines). Anything but bit depth 8,
 * color type 2 (RGB) or 6 (RGBA), interlace 0 is rejected loudly with the
 * tile URL, as is any decode whose pixel count or elevation band is wrong.
 *
 * ══ SAMPLING CONTRACT ═════════════════════════════════════════════════════
 * The region's georeference is read from the matching HOSMREG2 file (header
 * + meta block only; the full spec lives in scripts/fetch-osm-region.mjs).
 * The bbox is recovered from the meta by inverting the HOSMREG2 projection
 * with the SAME two constants that format mandates:
 *
 *     west  = origin_lon - half_span_east_m  / (cos(origin_lat_rad) * 111320.0)
 *     east  = origin_lon + half_span_east_m  / (cos(origin_lat_rad) * 111320.0)
 *     south = origin_lat - half_span_north_m / 110540.0
 *     north = origin_lat + half_span_north_m / 110540.0
 *
 * The DEM covers that bbox grown by MARGIN_M (2000 m) on all four sides,
 * converted to degrees with the same two constants, so terrain context (the
 * foothills around a region) is present for blending at the region edge.
 *
 * The output grid is a REGULAR lat/lon grid at the source's native
 * resolution:
 *     lon_step = 360 / (2^13 * 256) degrees   (one mercator pixel column)
 *     lat_step = lon_step * cos(origin_lat_rad)   (the mercator pixel ROW
 *                height, in latitude degrees, at the region's origin)
 * Row 0 is the NORTHERNMOST row; latitudes DECREASE by lat_step per row.
 * Each grid sample is BILINEARLY interpolated from the four surrounding
 * terrarium pixel CENTERS in global web-mercator pixel space (tile pixel
 * (px, py) of tile (tx, ty) has its center at global pixel coordinate
 * (tx*256 + px + 0.5, ty*256 + py + 0.5)). All fetched tiles are composited
 * into one mosaic first so bilinear lookups cross tile seams transparently.
 *
 * ══ FORMAT SPEC "HOSDEM1" ═════════════════════════════════════════════════
 * All integers and floats are LITTLE-ENDIAN. There is no padding and no
 * alignment anywhere: every field starts immediately after the previous
 * one. The Rust reader implements this exact layout; keep the two in sync,
 * this header is the contract. The header is 55 bytes:
 *
 *   0..7    magic  b"HOSDEM1" (7 ASCII bytes, no NUL)
 *   7..11   u32    width      columns, east-west, >= 2
 *   11..15  u32    height     rows, north-south, >= 2; ROW 0 = NORTH
 *   15..23  f64    lat_north  the LATITUDE of row 0's sample points, degrees
 *   23..31  f64    lon_west   the LONGITUDE of column 0's sample points, deg
 *   31..39  f64    lat_step   degrees between row sample points, POSITIVE;
 *                             row i latitude = lat_north - i * lat_step
 *   39..47  f64    lon_step   degrees between column sample points, positive
 *                             going east; col j lon = lon_west + j * lon_step
 *   47..51  f32    min_m      elevation quantization floor, metres
 *   51..55  f32    max_m      quantization ceiling, metres; max_m > min_m
 *   55..    u16[width*height] elevations, row-major from the north row:
 *                             sample (row i, col j) is at index i*width + j.
 *               q      = round((elev_m - min_m) / (max_m - min_m) * 65535)
 *               elev_m = min_m + (q / 65535) * (max_m - min_m)
 *
 * min_m / max_m are the sampled grid's true min / max widened by 1 cm and
 * rounded to f32, so every sample quantizes without clamping, and the
 * quantization step over a ~500 m relief range is ~8 mm -- far below the
 * source data's own accuracy. The file is exactly 55 + 2*width*height
 * bytes and verify() asserts it lands exactly on EOF.
 *
 * DETERMINISM: quantization and serialization are pure functions of the
 * sampled grid; the build serializes twice from the same grid and asserts
 * the two buffers are byte-identical (house rule, same as fetch-osm-region).
 *
 * Usage (the two shipped regions; re-run both after any format change):
 *   node scripts/fetch-region-dem.mjs --region data/maps/regions/silverdale.bin
 *   node scripts/fetch-region-dem.mjs --region data/maps/regions/seattle-center.bin
 *   node scripts/fetch-region-dem.mjs --verify-only data/maps/regions/silverdale.dem.bin
 *
 *   --region       path to the HOSMREG2 region .bin whose bbox to cover
 *   --out          output path; default = the region path with .bin
 *                  replaced by .dem.bin
 *   --verify-only  skip the network entirely and just re-verify a file
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, basename, resolve } from 'node:path';
import { inflateSync } from 'node:zlib';

// ── Format constants (the contract; verify() re-asserts the literals) ──────

const MAGIC = 'HOSDEM1'; // 7 ASCII bytes
const HEADER_SIZE = 55;

/** Metres per degree of longitude AT THE EQUATOR; scaled by cos(lat).
 *  MUST equal the HOSMREG2 constant or the recovered bbox drifts. */
const M_PER_DEG_LON_EQ = 111320.0;
/** Metres per degree of latitude (fixed spherical approximation, ditto). */
const M_PER_DEG_LAT = 110540.0;

const DEG = Math.PI / 180;

// ── Source and sampling constants ─────────────────────────────────────────

const ZOOM = 13;
const N_TILES = 1 << ZOOM;         // tiles per world side at this zoom
const TILE_PX = 256;               // terrarium tiles are 256 x 256
const WORLD_PX = N_TILES * TILE_PX; // global mercator pixels per side
/** One mercator pixel column, in longitude degrees (exact). */
const LON_STEP_DEG = 360 / WORLD_PX;
/** The DEM covers the region bbox grown by this on all four sides. */
const MARGIN_M = 2000;
/** Web-mercator latitude limit; tiles do not exist beyond it. */
const MERC_LAT_LIMIT = 85.0511287798;

const TILE_URL = (z, x, y) => `https://s3.amazonaws.com/elevation-tiles-prod/terrarium/${z}/${x}/${y}.png`;
/** A real, identifying User-Agent (same habit as the Overpass fetcher). */
const USER_AGENT = 'HumanityOS build script; github.com/Shaostoul/Humanity';
const RETRY_STATUS = new Set([429, 500, 502, 503, 504]);
const RETRY_DELAY_MS = 5000;
const FETCH_POOL = 6;

/** Earth's real elevation band with slack (Challenger Deep ~-10935 m,
 *  Everest ~8849 m). A decoded value outside it means the PNG decode or the
 *  terrarium formula went wrong, never real terrain. */
const ELEV_PLAUSIBLE_MIN = -12000;
const ELEV_PLAUSIBLE_MAX = 9200;

/** Guard rails on grid size (memory + committed-file budget). */
const MAX_GRID_DIM = 20000;
const MAX_GRID_SAMPLES = 50_000_000;

/**
 * Per-region plausibility gates for verify(). Keyed by output FILE NAME so a
 * region carries its own expectations; add an entry when you add a region.
 * These are the checks that catch "the fetch answered, but with junk": a
 * shifted bbox, a broken decode, a lat/lon swap. Each point gate is tested
 * at the grid sample NEAREST the given lat/lon, against the DEQUANTIZED
 * value read back from the FILE bytes (never the in-memory grid), so the
 * gate exercises the whole pipeline including quantization.
 *   waterPoints / landPoints: [lat, lon, min_m, max_m, label]
 *   minPeakM: the highest sample in the whole grid must exceed this.
 */
const REGION_GATES = {
  'silverdale.dem.bin': {
    waterPoints: [
      // Mid Dyes Inlet, north lobe: must be at/below ~sea level. The inlet
      // is shallow (~10-20 m); the source may composite bathymetry, hence
      // the generous floor.
      [47.6230, -122.6870, -100, 2, 'mid Dyes Inlet, north lobe'],
    ],
    landPoints: [
      // Central Silverdale sits on a low glacial terrace above the inlet.
      [47.6450, -122.6950, 5, 120, 'central Silverdale'],
    ],
    // The hills west and north of Dyes Inlet (Newberry Hill and the ridge
    // toward Green/Gold Mountain) are inside the bbox + 2 km margin and
    // top 150 m.
    minPeakM: 150,
  },
  'seattle-center.dem.bin': {
    waterPoints: [
      // Elliott Bay off the piers; the bay is genuinely deep (~100+ m), so
      // the floor allows real bathymetry if the source carries it.
      [47.6080, -122.3520, -300, 2, 'Elliott Bay off the piers'],
    ],
    landPoints: [
      // The Seattle Center grounds, a low bench below Queen Anne Hill.
      [47.6205, -122.3493, 5, 120, 'Seattle Center grounds'],
    ],
    // Queen Anne Hill's summit area (~139 m at 47.634, -122.356) is inside
    // the bbox + 2 km margin.
    minPeakM: 100,
  },
};
/** Used for a region with no entry above: structure only, no local knowledge. */
const DEFAULT_GATES = { waterPoints: [], landPoints: [], minPeakM: 0 };

// ── Small helpers ─────────────────────────────────────────────────────────

function die(msg) {
  console.error(`fetch-region-dem: ${msg}`);
  process.exit(1);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Parse `--flag value` pairs (`--verify-only x` becomes args.verifyOnly). */
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

// ── Mercator math (slippy tile scheme) ────────────────────────────────────

/** Longitude in degrees -> global mercator pixel x at ZOOM. */
const lonToGx = (lon) => ((lon + 180) / 360) * WORLD_PX;
/** Latitude in degrees -> global mercator pixel y at ZOOM. */
const latToGy = (lat) => {
  const r = lat * DEG;
  return ((1 - Math.log(Math.tan(r) + 1 / Math.cos(r)) / Math.PI) / 2) * WORLD_PX;
};

// ── Region georeference (HOSMREG2 header + meta block only) ───────────────

/**
 * Read just the header and meta block of a HOSMREG2 region file and recover
 * the bbox in degrees by inverting its projection (the exact formulas in
 * this file's header comment). Throws on anything malformed; the records
 * after the meta block are not touched (fetch-osm-region --verify-only owns
 * their verification).
 */
function readRegionMeta(path) {
  const fail = (msg) => { throw new Error(`${path}: ${msg}`); };
  if (!existsSync(path)) fail('file does not exist');
  const bin = readFileSync(path);
  if (bin.length < 45) fail(`only ${bin.length} bytes; HOSMREG2 needs at least 45`);
  const magic = bin.toString('ascii', 0, 8);
  if (magic !== 'HOSMREG2') fail(`magic "${magic}", expected "HOSMREG2"`);
  const originLat = bin.readDoubleLE(20);
  const originLon = bin.readDoubleLE(28);
  const halfE = bin.readFloatLE(36);
  const halfN = bin.readFloatLE(40);
  const nameLen = bin.readUInt8(44);
  if (45 + nameLen > bin.length) fail('region name overruns the file');
  const name = bin.toString('utf8', 45, 45 + nameLen);
  if (!Number.isFinite(originLat) || Math.abs(originLat) > 90
    || !Number.isFinite(originLon) || Math.abs(originLon) > 180) {
    fail(`origin ${originLat}, ${originLon} is not a real lat/lon`);
  }
  if (!(halfE > 0 && halfN > 0 && halfE < 1e6 && halfN < 1e6)) {
    fail(`half spans ${halfE} x ${halfN} m are implausible`);
  }
  const lonScale = Math.cos(originLat * DEG) * M_PER_DEG_LON_EQ;
  return {
    name, originLat, originLon, halfE, halfN,
    south: originLat - halfN / M_PER_DEG_LAT,
    north: originLat + halfN / M_PER_DEG_LAT,
    west: originLon - halfE / lonScale,
    east: originLon + halfE / lonScale,
  };
}

/** The 2 km margin, in degrees, at this region's origin latitude. */
function marginDeg(region) {
  const lonScale = Math.cos(region.originLat * DEG) * M_PER_DEG_LON_EQ;
  return { lat: MARGIN_M / M_PER_DEG_LAT, lon: MARGIN_M / lonScale };
}

/**
 * The output grid definition: bbox + margin, at native tile resolution.
 * ceil() + 1 guarantees the last row/column lies at or beyond the wanted
 * southern/eastern edge, so the grid COVERS bbox + margin, never undershoots.
 */
function makeGridDef(region) {
  const m = marginDeg(region);
  const latStep = LON_STEP_DEG * Math.cos(region.originLat * DEG);
  const lonWest = region.west - m.lon;
  const latNorth = region.north + m.lat;
  const width = Math.ceil((region.east + m.lon - lonWest) / LON_STEP_DEG) + 1;
  const height = Math.ceil((latNorth - (region.south - m.lat)) / latStep) + 1;
  return { width, height, latNorth, lonWest, latStep, lonStep: LON_STEP_DEG };
}

// ── Minimal PNG decoder (node core zlib only) ─────────────────────────────

const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/**
 * Decode a non-interlaced 8-bit RGB/RGBA PNG: walk the chunks (IHDR, the
 * IDAT concatenation, IEND), inflate, and reverse the per-scanline filter
 * (types 0..4: None, Sub, Up, Average, Paeth) per the PNG spec. Everything
 * else -- palette, 16-bit, grayscale, interlace -- is rejected loudly with
 * the tile URL. Returns { width, height, bpp, pixels } where pixels is the
 * unfiltered byte stream, width*bpp bytes per row.
 */
function decodePng(buf, url) {
  const fail = (msg) => die(`png decode failed for ${url}: ${msg}`);
  if (buf.length < 8 + 25) fail(`only ${buf.length} bytes`);
  for (let i = 0; i < 8; i++) {
    if (buf[i] !== PNG_SIG[i]) fail('bad PNG signature');
  }
  let off = 8;
  let ihdr = null;
  const idat = [];
  let sawEnd = false;
  while (off + 8 <= buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('ascii', off + 4, off + 8);
    const dataStart = off + 8;
    if (dataStart + len + 4 > buf.length) fail(`chunk ${type} overruns the file`);
    const data = buf.subarray(dataStart, dataStart + len);
    if (type === 'IHDR') {
      if (len !== 13) fail(`IHDR is ${len} bytes, expected 13`);
      ihdr = {
        width: data.readUInt32BE(0), height: data.readUInt32BE(4),
        bitDepth: data[8], colorType: data[9],
        compression: data[10], filter: data[11], interlace: data[12],
      };
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      sawEnd = true;
      break;
    }
    off = dataStart + len + 4; // skip the CRC; structure is validated instead
  }
  if (!ihdr) fail('no IHDR chunk');
  if (!sawEnd) fail('no IEND chunk');
  if (idat.length === 0) fail('no IDAT chunks');
  if (ihdr.bitDepth !== 8) fail(`bit depth ${ihdr.bitDepth}; only 8 supported`);
  if (ihdr.colorType !== 2 && ihdr.colorType !== 6) {
    fail(`color type ${ihdr.colorType}; only 2 (RGB) and 6 (RGBA) supported`);
  }
  if (ihdr.compression !== 0) fail(`compression method ${ihdr.compression}, expected 0`);
  if (ihdr.filter !== 0) fail(`filter method ${ihdr.filter}, expected 0`);
  if (ihdr.interlace !== 0) fail('interlaced PNG not supported');
  if (!(ihdr.width > 0 && ihdr.height > 0 && ihdr.width <= 4096 && ihdr.height <= 4096)) {
    fail(`implausible dimensions ${ihdr.width} x ${ihdr.height}`);
  }

  const bpp = ihdr.colorType === 2 ? 3 : 4;
  let raw;
  try {
    raw = inflateSync(Buffer.concat(idat));
  } catch (err) {
    fail(`IDAT inflate failed: ${err.message}`);
  }
  const stride = ihdr.width * bpp;
  if (raw.length !== ihdr.height * (1 + stride)) {
    fail(`inflated to ${raw.length} bytes, expected ${ihdr.height * (1 + stride)}`);
  }

  const out = Buffer.alloc(ihdr.height * stride);
  for (let y = 0; y < ihdr.height; y++) {
    const ft = raw[y * (1 + stride)];
    const src = y * (1 + stride) + 1;
    const dst = y * stride;
    const prev = dst - stride; // previous unfiltered row (y > 0 only)
    switch (ft) {
      case 0: // None
        raw.copy(out, dst, src, src + stride);
        break;
      case 1: // Sub: left neighbour (same channel, bpp bytes back)
        for (let x = 0; x < stride; x++) {
          out[dst + x] = (raw[src + x] + (x >= bpp ? out[dst + x - bpp] : 0)) & 0xff;
        }
        break;
      case 2: // Up: same byte in the previous row
        for (let x = 0; x < stride; x++) {
          out[dst + x] = (raw[src + x] + (y > 0 ? out[prev + x] : 0)) & 0xff;
        }
        break;
      case 3: // Average: floor((left + up) / 2)
        for (let x = 0; x < stride; x++) {
          const a = x >= bpp ? out[dst + x - bpp] : 0;
          const b = y > 0 ? out[prev + x] : 0;
          out[dst + x] = (raw[src + x] + ((a + b) >> 1)) & 0xff;
        }
        break;
      case 4: // Paeth predictor over left, up, up-left
        for (let x = 0; x < stride; x++) {
          const a = x >= bpp ? out[dst + x - bpp] : 0;
          const b = y > 0 ? out[prev + x] : 0;
          const c = x >= bpp && y > 0 ? out[prev + x - bpp] : 0;
          const p = a + b - c;
          const pa = Math.abs(p - a);
          const pb = Math.abs(p - b);
          const pc = Math.abs(p - c);
          const pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
          out[dst + x] = (raw[src + x] + pred) & 0xff;
        }
        break;
      default:
        fail(`scanline ${y}: unknown filter type ${ft}`);
    }
  }
  return { width: ihdr.width, height: ihdr.height, bpp, pixels: out };
}

// ── Tile fetch + mosaic ───────────────────────────────────────────────────

/** GET one tile; retry ONCE on a load-shed status or a network error. */
async function fetchTileBytes(url) {
  for (let attempt = 1; attempt <= 2; attempt++) {
    const last = attempt === 2;
    try {
      const res = await fetch(url, { headers: { 'User-Agent': USER_AGENT } });
      if (res.ok) return Buffer.from(await res.arrayBuffer());
      if (RETRY_STATUS.has(res.status) && !last) {
        console.warn(`fetch: HTTP ${res.status} for ${url}; retrying once in ${RETRY_DELAY_MS / 1000}s`);
        await sleep(RETRY_DELAY_MS);
        continue;
      }
      die(`tile fetch failed: HTTP ${res.status} ${res.statusText} for ${url}`);
    } catch (err) {
      if (last) die(`tile fetch failed for ${url}: ${err && err.message ? err.message : err}`);
      console.warn(`fetch: ${err && err.message ? err.message : err} for ${url}; retrying once in ${RETRY_DELAY_MS / 1000}s`);
      await sleep(RETRY_DELAY_MS);
    }
  }
  return die('unreachable');
}

/**
 * Fetch every z13 tile covering the grid (plus 2 pixels of bilinear slack),
 * decode each, hard-check each (256 x 256, every pixel inside the plausible
 * elevation band), and composite them into one Float32Array mosaic. f32 is
 * exact for terrarium values (1/256 m steps over +-32768 m fit a 24-bit
 * mantissa), so the mosaic loses nothing.
 */
async function fetchMosaic(def) {
  const gxMin = lonToGx(def.lonWest);
  const gxMax = lonToGx(def.lonWest + (def.width - 1) * def.lonStep);
  const gyMin = latToGy(def.latNorth);
  const gyMax = latToGy(def.latNorth - (def.height - 1) * def.latStep);
  const tx0 = Math.floor((gxMin - 2) / TILE_PX);
  const tx1 = Math.floor((gxMax + 2) / TILE_PX);
  const ty0 = Math.floor((gyMin - 2) / TILE_PX);
  const ty1 = Math.floor((gyMax + 2) / TILE_PX);
  if (!(tx0 >= 0 && tx1 < N_TILES && ty0 >= 0 && ty1 < N_TILES && tx0 <= tx1 && ty0 <= ty1)) {
    die(`tile range z${ZOOM} x ${tx0}..${tx1}, y ${ty0}..${ty1} is outside the world `
      + '(regions touching the antimeridian or the mercator poles are unsupported)');
  }

  const tiles = [];
  for (let ty = ty0; ty <= ty1; ty++) {
    for (let tx = tx0; tx <= tx1; tx++) tiles.push({ tx, ty });
  }
  const mosaicW = (tx1 - tx0 + 1) * TILE_PX;
  const mosaicH = (ty1 - ty0 + 1) * TILE_PX;
  const elev = new Float32Array(mosaicW * mosaicH);

  let bytes = 0;
  let rgbaTiles = 0;
  let tileMin = Infinity;
  let tileMax = -Infinity;
  const t0 = Date.now();
  let next = 0;
  const worker = async () => {
    for (;;) {
      const k = next++;
      if (k >= tiles.length) return;
      const { tx, ty } = tiles[k];
      const url = TILE_URL(ZOOM, tx, ty);
      const png = await fetchTileBytes(url);
      bytes += png.length;
      const img = decodePng(png, url);
      // The decoder self-checks, on REAL fetched data: exact terrarium tile
      // dimensions, and every decoded pixel inside Earth's elevation band.
      // A broken filter reversal produces values tens of thousands of
      // metres out, so the band check catches it immediately.
      if (img.width !== TILE_PX || img.height !== TILE_PX) {
        die(`tile is ${img.width} x ${img.height}, expected ${TILE_PX} x ${TILE_PX}: ${url}`);
      }
      if (img.bpp === 4) rgbaTiles++;
      const baseX = (tx - tx0) * TILE_PX;
      const baseY = (ty - ty0) * TILE_PX;
      for (let py = 0; py < TILE_PX; py++) {
        const row = py * TILE_PX * img.bpp;
        const dst = (baseY + py) * mosaicW + baseX;
        for (let px = 0; px < TILE_PX; px++) {
          const o = row + px * img.bpp;
          const e = img.pixels[o] * 256 + img.pixels[o + 1] + img.pixels[o + 2] / 256 - 32768;
          if (!(e >= ELEV_PLAUSIBLE_MIN && e <= ELEV_PLAUSIBLE_MAX)) {
            die(`pixel (${px}, ${py}) decodes to ${e} m, outside `
              + `[${ELEV_PLAUSIBLE_MIN}, ${ELEV_PLAUSIBLE_MAX}]: ${url}`);
          }
          if (e < tileMin) tileMin = e;
          if (e > tileMax) tileMax = e;
          elev[dst + px] = e;
        }
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(FETCH_POOL, tiles.length) }, worker));

  console.log(
    `fetch: ${tiles.length} tiles at z${ZOOM} (x ${tx0}..${tx1}, y ${ty0}..${ty1}), `
    + `${(bytes / 1024).toFixed(0)} KiB in ${Date.now() - t0} ms | `
    + `${rgbaTiles} RGBA, ${tiles.length - rgbaTiles} RGB | `
    + `tile elevations span [${tileMin.toFixed(1)}, ${tileMax.toFixed(1)}] m`
  );
  return { tx0, ty0, mosaicW, mosaicH, elev, tileCount: tiles.length, bytes };
}

/**
 * Sample the regular lat/lon grid bilinearly from the mosaic, in global
 * mercator pixel-center space. An out-of-mosaic lookup is a bug in the
 * tile-range math above and dies rather than clamping.
 */
function sampleGrid(def, mosaic) {
  const { tx0, ty0, mosaicW, mosaicH, elev } = mosaic;
  const values = new Float64Array(def.width * def.height);
  const colMx = new Float64Array(def.width);
  for (let j = 0; j < def.width; j++) {
    colMx[j] = lonToGx(def.lonWest + j * def.lonStep) - 0.5 - tx0 * TILE_PX;
  }
  for (let i = 0; i < def.height; i++) {
    const my = latToGy(def.latNorth - i * def.latStep) - 0.5 - ty0 * TILE_PX;
    const y0 = Math.floor(my);
    const fy = my - y0;
    if (y0 < 0 || y0 + 1 >= mosaicH) {
      die(`grid row ${i} samples outside the fetched mosaic (tile-range math bug)`);
    }
    for (let j = 0; j < def.width; j++) {
      const mx = colMx[j];
      const x0 = Math.floor(mx);
      const fx = mx - x0;
      if (x0 < 0 || x0 + 1 >= mosaicW) {
        die(`grid column ${j} samples outside the fetched mosaic (tile-range math bug)`);
      }
      const a = elev[y0 * mosaicW + x0];
      const b = elev[y0 * mosaicW + x0 + 1];
      const c = elev[(y0 + 1) * mosaicW + x0];
      const d = elev[(y0 + 1) * mosaicW + x0 + 1];
      values[i * def.width + j] = (1 - fy) * ((1 - fx) * a + fx * b) + fy * ((1 - fx) * c + fx * d);
    }
  }
  return values;
}

// ── Serialize ─────────────────────────────────────────────────────────────

/** Serialize the sampled grid into the HOSDEM1 byte layout. */
function serialize(def, values, minM, maxM) {
  const n = def.width * def.height;
  const buf = Buffer.alloc(HEADER_SIZE + 2 * n);
  buf.write(MAGIC, 0, 'ascii');
  buf.writeUInt32LE(def.width, 7);
  buf.writeUInt32LE(def.height, 11);
  buf.writeDoubleLE(def.latNorth, 15);
  buf.writeDoubleLE(def.lonWest, 23);
  buf.writeDoubleLE(def.latStep, 31);
  buf.writeDoubleLE(def.lonStep, 39);
  buf.writeFloatLE(minM, 47);
  buf.writeFloatLE(maxM, 51);
  const range = maxM - minM;
  for (let k = 0; k < n; k++) {
    let q = Math.round(((values[k] - minM) / range) * 65535);
    if (q < 0) q = 0;
    else if (q > 65535) q = 65535;
    buf.writeUInt16LE(q, HEADER_SIZE + 2 * k);
  }
  return buf;
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
 * Re-open the written file, walk every byte of it, and assert everything
 * the Rust reader is entitled to assume. `build` (grid def + sampled floats
 * + the region georeference + the determinism result) is present on a build
 * run and unlocks the cross-checks against the input; --verify-only runs
 * the structural checks and the region gates only (the gates read the file
 * bytes, so they need no network and no source data).
 */
function verify(path, { build = null } = {}) {
  console.log(`verify: re-opening ${path}`);
  const bin = readFileSync(path);
  const gates = REGION_GATES[basename(path)] || DEFAULT_GATES;
  const gateName = REGION_GATES[basename(path)] ? basename(path) : 'default (no region entry)';

  // The Rust parser hardcodes these numbers, so assert the LITERALS, not
  // the constants above: editing a constant must not silently move the
  // whole format while every self-consistent check keeps passing.
  check('spec literals: magic "HOSDEM1" is 7 bytes, header is 55 bytes, HOSMREG2 projection constants',
    MAGIC === 'HOSDEM1' && MAGIC.length === 7 && HEADER_SIZE === 55
    && M_PER_DEG_LON_EQ === 111320.0 && M_PER_DEG_LAT === 110540.0,
    `magic "${MAGIC}", header ${HEADER_SIZE}, lon ${M_PER_DEG_LON_EQ} m/deg, lat ${M_PER_DEG_LAT} m/deg`);

  if (bin.length < HEADER_SIZE) {
    check(`file is at least the ${HEADER_SIZE}-byte header`, false, `${bin.length} bytes`);
    return report();
  }

  const magic = bin.toString('ascii', 0, 7);
  check('magic is "HOSDEM1"', magic === MAGIC, `got "${magic}"`);
  if (magic !== MAGIC) return report();

  const width = bin.readUInt32LE(7);
  const height = bin.readUInt32LE(11);
  const latNorth = bin.readDoubleLE(15);
  const lonWest = bin.readDoubleLE(23);
  const latStep = bin.readDoubleLE(31);
  const lonStep = bin.readDoubleLE(39);
  const minM = bin.readFloatLE(47);
  const maxM = bin.readFloatLE(51);

  check('dims: width and height are 2..20000',
    width >= 2 && width <= MAX_GRID_DIM && height >= 2 && height <= MAX_GRID_DIM,
    `${width} x ${height}`);
  if (!(width >= 2 && width <= MAX_GRID_DIM && height >= 2 && height <= MAX_GRID_DIM)) return report();

  const latSouth = latNorth - (height - 1) * latStep;
  const lonEast = lonWest + (width - 1) * lonStep;
  check('georef: grid latitudes lie inside the web-mercator band',
    Number.isFinite(latNorth) && Number.isFinite(latStep)
    && latNorth < MERC_LAT_LIMIT && latSouth > -MERC_LAT_LIMIT && latNorth > latSouth,
    `rows ${latNorth.toFixed(6)} down to ${latSouth.toFixed(6)} deg`);
  check('georef: grid longitudes are real and ascend east',
    Number.isFinite(lonWest) && Number.isFinite(lonStep)
    && lonWest >= -180 && lonEast <= 180 && lonEast > lonWest,
    `cols ${lonWest.toFixed(6)} to ${lonEast.toFixed(6)} deg`);
  check('georef: steps are positive and finer than 0.01 deg',
    latStep > 0 && latStep < 0.01 && lonStep > 0 && lonStep < 0.01,
    `lat_step ${latStep.toExponential(6)}, lon_step ${lonStep.toExponential(6)} deg`);
  check('quantization: min_m < max_m, both finite and inside Earth\'s elevation band',
    Number.isFinite(minM) && Number.isFinite(maxM) && minM < maxM
    && minM >= ELEV_PLAUSIBLE_MIN && maxM <= ELEV_PLAUSIBLE_MAX,
    `[${minM.toFixed(2)}, ${maxM.toFixed(2)}] m`);

  // ── The EOF walk: the sample block must land exactly on end of file.
  const n = width * height;
  const expectedLen = HEADER_SIZE + 2 * n;
  check('samples end exactly at EOF (55 + 2*width*height bytes)',
    bin.length === expectedLen, `computed ${expectedLen}, actual ${bin.length}`);
  if (bin.length !== expectedLen) return report();

  // ── Dequantize every sample from the BYTES (what the Rust reader sees).
  const range = maxM - minM;
  const dq = new Float64Array(n);
  let seenMin = Infinity;
  let seenMax = -Infinity;
  let sum = 0;
  let peakIdx = 0;
  let atOrBelowSea = 0;
  for (let k = 0; k < n; k++) {
    const e = minM + (bin.readUInt16LE(HEADER_SIZE + 2 * k) / 65535) * range;
    dq[k] = e;
    if (e < seenMin) seenMin = e;
    if (e > seenMax) { seenMax = e; peakIdx = k; }
    sum += e;
    if (e <= 0) atOrBelowSea++;
  }
  const mean = sum / n;

  // ── Build-run cross-checks against the in-memory sampled grid.
  if (build) {
    const { def, values, region, determinismOk } = build;
    check('dims match the sampled grid', width === def.width && height === def.height,
      `file ${width} x ${height} vs grid ${def.width} x ${def.height}`);
    check('georef fields are bit-exact copies of the grid definition (f64 round trip)',
      latNorth === def.latNorth && lonWest === def.lonWest
      && latStep === def.latStep && lonStep === def.lonStep);

    let gridMin = Infinity;
    let gridMax = -Infinity;
    for (let k = 0; k < values.length; k++) {
      if (values[k] < gridMin) gridMin = values[k];
      if (values[k] > gridMax) gridMax = values[k];
    }
    check('min_m / max_m bracket the sampled grid strictly (no sample ever clamps)',
      minM < gridMin && maxM > gridMax,
      `file [${minM.toFixed(3)}, ${maxM.toFixed(3)}] vs grid [${gridMin.toFixed(3)}, ${gridMax.toFixed(3)}] m`);

    let worst = 0;
    for (let k = 0; k < n; k++) {
      const d = Math.abs(dq[k] - values[k]);
      if (d > worst) worst = d;
    }
    const step = range / 65535;
    check('every quantized value round-trips within (max-min)/65535 of the sampled float',
      worst <= step + 1e-9,
      `worst ${(worst * 1000).toFixed(3)} mm, step ${(step * 1000).toFixed(3)} mm, ${n} samples`);

    check('serializing the same sampled grid twice is byte-identical', determinismOk === true);

    const m = marginDeg(region);
    check(`DEM grid covers the region bbox plus the ${MARGIN_M} m margin`,
      lonWest <= region.west - m.lon + 1e-9 && lonEast >= region.east + m.lon - 1e-9
      && latNorth >= region.north + m.lat - 1e-9 && latSouth <= region.south - m.lat + 1e-9,
      `DEM ${latSouth.toFixed(4)}..${latNorth.toFixed(4)}, ${lonWest.toFixed(4)}..${lonEast.toFixed(4)} vs `
      + `bbox ${region.south.toFixed(4)}..${region.north.toFixed(4)}, ${region.west.toFixed(4)}..${region.east.toFixed(4)}`);
  } else {
    // --verify-only: if the sibling region .bin is present, still prove
    // coverage (the georeference contract between the two files).
    const sibling = path.replace(/\.dem\.bin$/, '.bin');
    if (sibling !== path && existsSync(sibling)) {
      try {
        const region = readRegionMeta(sibling);
        const m = marginDeg(region);
        check(`DEM grid covers the sibling region bbox plus the ${MARGIN_M} m margin`,
          lonWest <= region.west - m.lon + 1e-9 && lonEast >= region.east + m.lon - 1e-9
          && latNorth >= region.north + m.lat - 1e-9 && latSouth <= region.south - m.lat + 1e-9,
          `sibling ${sibling}`);
      } catch (err) {
        check('sibling region file parses (needed for the coverage check)', false, err.message);
      }
    } else {
      console.log('verify: no sibling region .bin found; coverage check skipped');
    }
  }

  // ── Region gates: real-geography checks at the sample NEAREST a known
  // lat/lon, evaluated from the file bytes. This is what catches "the fetch
  // answered, but with junk": a lat/lon swap, a row-order flip, a shifted
  // grid or a broken decode all move these values far out of band.
  const nearest = (lat, lon) => {
    const i = Math.round((latNorth - lat) / latStep);
    const j = Math.round((lon - lonWest) / lonStep);
    if (i < 0 || i >= height || j < 0 || j >= width) return null;
    return { i, j, elev: dq[i * width + j] };
  };
  for (const [lat, lon, lo, hi, label] of gates.waterPoints) {
    const s = nearest(lat, lon);
    check(`gates [${gateName}]: water at ${lat}, ${lon} (${label}) is ${lo}..${hi} m`,
      s !== null && s.elev >= lo && s.elev <= hi,
      s ? `${s.elev.toFixed(2)} m at row ${s.i}, col ${s.j}` : 'point is outside the grid');
  }
  for (const [lat, lon, lo, hi, label] of gates.landPoints) {
    const s = nearest(lat, lon);
    check(`gates [${gateName}]: land at ${lat}, ${lon} (${label}) is ${lo}..${hi} m`,
      s !== null && s.elev >= lo && s.elev <= hi,
      s ? `${s.elev.toFixed(2)} m at row ${s.i}, col ${s.j}` : 'point is outside the grid');
  }
  const peakLat = latNorth - Math.floor(peakIdx / width) * latStep;
  const peakLon = lonWest + (peakIdx % width) * lonStep;
  if (gates.minPeakM > 0) {
    check(`gates [${gateName}]: the highest sample exceeds ${gates.minPeakM} m`,
      seenMax > gates.minPeakM,
      `${seenMax.toFixed(1)} m at ${peakLat.toFixed(5)}, ${peakLon.toFixed(5)}`);
  }

  // ── Summary a human can eyeball: extent, resolution, stats, histogram.
  const latMid = (latNorth + latSouth) / 2;
  const mLat = latStep * M_PER_DEG_LAT;
  const mLon = lonStep * Math.cos(latMid * DEG) * M_PER_DEG_LON_EQ;
  console.log(
    `verify: ${width} x ${height} samples (${n} total), `
    + `~${mLon.toFixed(1)} x ${mLat.toFixed(1)} m ground spacing | `
    + `${bin.length} bytes (${(bin.length / 1024).toFixed(1)} KiB)`
  );
  console.log(
    `verify: elevation min ${seenMin.toFixed(2)} m, max ${seenMax.toFixed(2)} m `
    + `(at ${peakLat.toFixed(5)}, ${peakLon.toFixed(5)}), mean ${mean.toFixed(2)} m | `
    + `${((atOrBelowSea / n) * 100).toFixed(1)}% of samples at or below 0 m`
  );
  const BINS = 12;
  const binW = (seenMax - seenMin) / BINS || 1;
  const counts = new Array(BINS).fill(0);
  for (let k = 0; k < n; k++) {
    let b = Math.floor((dq[k] - seenMin) / binW);
    if (b >= BINS) b = BINS - 1; // the max value lands in the last bin
    counts[b]++;
  }
  const maxCount = Math.max(...counts);
  console.log(`verify: histogram (${BINS} bins over [${seenMin.toFixed(1)}, ${seenMax.toFixed(1)}] m):`);
  for (let b = 0; b < BINS; b++) {
    const lo = seenMin + b * binW;
    const hi = lo + binW;
    const pct = (counts[b] / n) * 100;
    const bar = '#'.repeat(Math.max(counts[b] > 0 ? 1 : 0, Math.round((counts[b] / maxCount) * 40)));
    console.log(
      `  [${lo.toFixed(1).padStart(8)} ..${hi.toFixed(1).padStart(8)}) `
      + `${pct.toFixed(1).padStart(5)}% ${String(counts[b]).padStart(8)} ${bar}`
    );
  }
  console.log('verify: elevation data from AWS Open Data Terrain Tiles '
    + '(Mapzen terrarium; USGS 3DEP/NED, NASA SRTM, NOAA -- US public domain)');

  return report();
}

// ── Main ──────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv);

  if (args.verifyOnly) {
    verify(resolve(args.verifyOnly));
    return;
  }

  if (!args.region) die('--region <region .bin> is required (or --verify-only <dem file>)');
  const regionPath = resolve(args.region);
  if (!/\.bin$/i.test(regionPath) || /\.dem\.bin$/i.test(regionPath)) {
    die('--region must point at a HOSMREG2 region .bin (not a .dem.bin)');
  }
  const outPath = resolve(args.out || regionPath.replace(/\.bin$/i, '.dem.bin'));

  let region;
  try {
    region = readRegionMeta(regionPath);
  } catch (err) {
    die(err.message);
  }
  console.log(
    `region "${region.name}" (${regionPath}): bbox `
    + `${region.south.toFixed(6)},${region.west.toFixed(6)},`
    + `${region.north.toFixed(6)},${region.east.toFixed(6)} | `
    + `origin ${region.originLat.toFixed(6)}, ${region.originLon.toFixed(6)} | `
    + `half spans ${region.halfE.toFixed(1)} x ${region.halfN.toFixed(1)} m`
  );

  const def = makeGridDef(region);
  if (!(def.width >= 2 && def.width <= MAX_GRID_DIM
    && def.height >= 2 && def.height <= MAX_GRID_DIM
    && def.width * def.height <= MAX_GRID_SAMPLES)) {
    die(`grid ${def.width} x ${def.height} is outside the guard rails `
      + `(<= ${MAX_GRID_DIM} per side, <= ${MAX_GRID_SAMPLES} samples)`);
  }
  console.log(
    `grid: ${def.width} x ${def.height} samples, row 0 at ${def.latNorth.toFixed(6)} N, `
    + `col 0 at ${def.lonWest.toFixed(6)} E | steps ${def.latStep.toExponential(6)} x `
    + `${def.lonStep.toExponential(6)} deg (bbox + ${MARGIN_M} m margin at native z${ZOOM} resolution)`
  );

  const mosaic = await fetchMosaic(def);

  const t0 = Date.now();
  const values = sampleGrid(def, mosaic);
  let gridMin = Infinity;
  let gridMax = -Infinity;
  for (let k = 0; k < values.length; k++) {
    const v = values[k];
    if (!Number.isFinite(v) || v < ELEV_PLAUSIBLE_MIN || v > ELEV_PLAUSIBLE_MAX) {
      die(`sample ${k} is ${v} m -- not finite plausible elevation (decode or sampling bug)`);
    }
    if (v < gridMin) gridMin = v;
    if (v > gridMax) gridMax = v;
  }
  console.log(
    `sample: ${values.length} bilinear samples in ${Date.now() - t0} ms | `
    + `grid elevation [${gridMin.toFixed(2)}, ${gridMax.toFixed(2)}] m`
  );

  // Quantization bounds: the true min/max widened by 1 cm THEN rounded to
  // f32 (the file stores f32), so quantization never clamps and dequant in
  // verify uses the exact same endpoints the file carries.
  const minM = Math.fround(gridMin - 0.01);
  const maxM = Math.fround(gridMax + 0.01);
  if (!(minM < gridMin && maxM > gridMax)) {
    die('quantization widening failed to bracket the grid (f32 rounding surprise)');
  }

  const bin = serialize(def, values, minM, maxM);
  // Byte-determinism proof: serialize AGAIN from the same sampled grid;
  // the two buffers must be identical.
  const bin2 = serialize(def, values, minM, maxM);
  const determinismOk = bin.equals(bin2);

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, bin);
  console.log(`write: ${outPath} -- ${bin.length} bytes (${(bin.length / 1024).toFixed(1)} KiB)`);

  verify(outPath, { build: { def, values, region, determinismOk } });
}

await main();
