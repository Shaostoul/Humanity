#!/usr/bin/env node
/**
 * build-stars-map-bin.js
 *
 * Converts data/stars.csv (HYG star catalog, ~34 MB, ~120k rows) into
 * data/stars-map.bin, the compact binary the native Maps page's GALAXY VIEW
 * renders. This is a DIFFERENT file from data/stars.bin (built by
 * scripts/build-stars-bin.js): stars.bin holds unit DIRECTIONS on the
 * celestial sphere for the in-world skybox, while stars-map.bin holds real
 * 3D POSITIONS in galactic light-years so the map can fly the camera around
 * the local stellar neighbourhood. The CSV stays the human-readable source of
 * truth; the .bin is the shipped runtime format (the release bundle tars the
 * whole data/ dir, so committing the .bin is all it takes to distribute it).
 * Re-run this script whenever stars.csv changes.
 *
 * Usage:
 *   node scripts/build-stars-map-bin.js             build + self-verify
 *   node scripts/build-stars-map-bin.js --verify    verify the existing file only
 *
 * ══ FORMAT SPEC "HOSMAP01" ════════════════════════════════════════════════
 * All integers and floats are LITTLE-ENDIAN. There is no padding or
 * alignment anywhere: every section starts immediately after the previous
 * one. The whole file is exactly
 *     16 + star_count*20 + sum(5 + name_len_i) bytes.
 *
 *   Header (16 bytes):
 *     0..8    magic  b"HOSMAP01" (8 ASCII bytes, no NUL)
 *     8..12   u32    star_count   (number of 20-byte star records)
 *     12..16  u32    named_count  (number of variable-length name records)
 *
 *   Star records: star_count consecutive 20-byte records, starting at
 *   offset 16. Record i lives at offset 16 + i*20:
 *     0..4    f32    gx   galactic X, LIGHT-YEARS
 *     4..8    f32    gy   galactic Y, LIGHT-YEARS
 *     8..12   f32    gz   galactic Z, LIGHT-YEARS
 *     12..16  f32    mag  apparent magnitude (HYG `mag`; lower = brighter)
 *     16..20  f32    ci   B-V color index (HYG `ci`; 0.0 when the CSV field
 *                         is blank or unparseable)
 *
 *     Coordinate frame: right-handed GALACTIC cartesian centred on Sol.
 *       +X points at the galactic centre (Sgr A*, galactic l=0, b=0)
 *       +Y points in the direction of galactic rotation (l=90 deg, b=0)
 *       +Z points at the north galactic pole (b=+90 deg)
 *     Units are light-years, NOT parsecs. Sol is at (0,0,0) and is NOT in
 *     the file (see exclusions below) -- the map draws Sol itself at the
 *     origin. So sqrt(gx^2+gy^2+gz^2) is the star's distance from Sol in ly.
 *
 *     ORDERING: records are sorted by `mag` ASCENDING (brightest first).
 *     Ties are broken by the HYG `id` column ascending, so the ordering is
 *     fully deterministic and stable across rebuilds. A reader that wants
 *     "the brightest N stars" can simply take the first N records; there is
 *     no need to scan or sort at load time.
 *
 *   Name records: named_count variable-length records, starting immediately
 *   after the last star record (offset 16 + star_count*20). Each record is:
 *     0..4    u32    star_index -- index into the SORTED star array above
 *                    (0 <= star_index < star_count), i.e. the record's bytes
 *                    begin at 16 + star_index*20
 *     4..5    u8     name_len, 1..32 (never 0, never > 32)
 *     5..     bytes  name_len bytes of UTF-8, the HYG `proper` name verbatim
 *                    (original case, NOT lowercased, no NUL terminator)
 *
 *     Only rows with a non-empty HYG `proper` name get a name record. Names
 *     longer than 32 BYTES are truncated to the largest whole number of
 *     UTF-8 characters that fits in 32 bytes (never split a multi-byte
 *     character), so the bytes always decode as valid UTF-8. Name records
 *     are emitted in ascending star_index order (i.e. brightest named star
 *     first), and each star_index appears at most once.
 *
 *   ROWS EXCLUDED FROM THE FILE:
 *     - Sol (HYG id 0 / proper "Sol" / dist 0) -- the map draws it at the
 *       origin itself, and a star at (0,0,0) has no direction.
 *     - dist <= 0 (no parallax / bad row).
 *     - dist >= 100000 -- HYG's sentinel for "unknown distance"; those rows
 *       carry a fabricated 100000 pc position that would smear a shell of
 *       fake stars across the map.
 *     - Rows too short to contain every column used here.
 *
 * ══ EQUATORIAL -> GALACTIC ROTATION ═══════════════════════════════════════
 * HYG's x,y,z columns are EQUATORIAL J2000 cartesian in PARSECS:
 *     x = dist*cos(dec)*cos(ra), y = dist*cos(dec)*sin(ra), z = dist*sin(dec)
 * (HYG stores `ra` in HOURS and `dec` in DEGREES; the x,y,z columns already
 * encode both, so they are used directly and ra/dec are only a fallback for
 * rows whose x,y,z are missing/zero.)
 *
 * Parsecs -> light-years uses LY_PER_PC = 3.2615638 (IAU 2012 exact-metre
 * definitions: 1 pc = 648000/pi AU, 1 ly = 9460730472580800 m).
 *
 * The rotation uses the standard IAU/Hipparcos J2000 galactic pole:
 *     alpha_NGP = 192.85948 deg   (RA of the north galactic pole)
 *     delta_NGP =  27.12825 deg   (Dec of the north galactic pole)
 *     l_NCP     = 122.93192 deg   (galactic longitude of the north
 *                                  celestial pole, equivalently the galactic
 *                                  longitude of the ascending node of the
 *                                  galactic plane on the equator + 90 deg)
 * built as the passive rotation
 *     M = Rz(180 deg - l_NCP) * Ry(90 deg - delta_NGP) * Rz(alpha_NGP)
 * which evaluates to the ESA SP-1200 (Hipparcos, Vol 1 sect 1.5.3) matrix
 * transpose A_G^T, i.e. [gx,gy,gz]^T = M * [x,y,z]^T with
 *
 *     M = [ -0.0548755604, -0.8734370902, -0.4838350155 ]
 *         [ +0.4941094279, -0.4448296300, +0.7469822445 ]
 *         [ -0.8676661490, -0.1980763734, +0.4559837762 ]
 *
 * (Row 0 of M is the unit vector toward the galactic centre expressed in
 * equatorial coords; row 2 is the north galactic pole. That is why row 0
 * dotted with an equatorial position yields the +X galactic component.)
 * The script recomputes M from the three angles at startup and asserts it
 * against those published constants to 1e-9, so a typo in either place
 * fails the build instead of silently tilting the galaxy.
 *
 * The Rust loader lives on the Maps side (galaxy view). Keep it in sync with
 * this spec -- this header is the contract.
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const CSV_PATH = path.join(ROOT, 'data', 'stars.csv');
const BIN_PATH = path.join(ROOT, 'data', 'stars-map.bin');

const MAGIC = 'HOSMAP01';
const HEADER_SIZE = 16;
const RECORD_SIZE = 20;
const MAX_NAME_BYTES = 32;

/** IAU 2012: 1 pc = 648000/pi AU, 1 ly = 9460730472580800 m -> 3.26156378... */
const LY_PER_PC = 3.2615638;

/** HYG's sentinel distance for "no usable parallax" (parsecs). */
const UNKNOWN_DIST_PC = 100000;

/** Galactic pole / node angles, degrees (IAU J2000, see header comment). */
const ALPHA_NGP_DEG = 192.85948;
const DELTA_NGP_DEG = 27.12825;
const L_NCP_DEG = 122.93192;

/** Published ESA SP-1200 A_G^T, used only to cross-check the built matrix. */
const HIPPARCOS_M = [
  [-0.0548755604, -0.8734370902, -0.4838350155],
  [+0.4941094279, -0.4448296300, +0.7469822445],
  [-0.8676661490, -0.1980763734, +0.4559837762],
];

const DEG = Math.PI / 180;

// ── Rotation matrix ────────────────────────────────────────────────────────

/** Passive rotation of the frame by `t` radians about Z (components in the
 *  rotated frame = Rz(t) * components in the old frame). */
function rotZ(t) {
  const c = Math.cos(t), s = Math.sin(t);
  return [[c, s, 0], [-s, c, 0], [0, 0, 1]];
}

/** Passive rotation of the frame by `t` radians about Y. */
function rotY(t) {
  const c = Math.cos(t), s = Math.sin(t);
  return [[c, 0, -s], [0, 1, 0], [s, 0, c]];
}

function matMul(a, b) {
  const o = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
  for (let i = 0; i < 3; i++) {
    for (let j = 0; j < 3; j++) {
      o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
    }
  }
  return o;
}

/**
 * M = Rz(180 - l_NCP) * Ry(90 - delta_NGP) * Rz(alpha_NGP), derived from the
 * three IAU angles rather than pasted in, so the constants above are the only
 * magic numbers and the published-matrix assert below is a real check.
 */
function buildGalacticMatrix() {
  return matMul(
    matMul(rotZ((180 - L_NCP_DEG) * DEG), rotY((90 - DELTA_NGP_DEG) * DEG)),
    rotZ(ALPHA_NGP_DEG * DEG)
  );
}

const M = buildGalacticMatrix();

function assertMatrixMatchesPublished() {
  let worst = 0;
  for (let i = 0; i < 3; i++) {
    for (let j = 0; j < 3; j++) {
      worst = Math.max(worst, Math.abs(M[i][j] - HIPPARCOS_M[i][j]));
    }
  }
  if (!(worst < 1e-9)) {
    throw new Error(
      `rotation matrix disagrees with the published Hipparcos values by ${worst.toExponential(3)} ` +
      `(built: ${JSON.stringify(M)})`
    );
  }
  return worst;
}

/**
 * Rebuild HYG's equatorial cartesian columns from ra (HOURS) / dec (DEGREES) /
 * dist (parsecs) -- the fallback for rows whose x,y,z are missing or zero.
 * Currently 0 rows in stars.csv need it, so verify() exercises it explicitly
 * against rows that DO have x,y,z; otherwise it would be untested dead code.
 */
function equatorialXyzFromRaDec(raHours, decDeg, distPc) {
  const raRad = raHours * 15 * DEG, decRad = decDeg * DEG;
  return [
    distPc * Math.cos(decRad) * Math.cos(raRad),
    distPc * Math.cos(decRad) * Math.sin(raRad),
    distPc * Math.sin(decRad),
  ];
}

/** Equatorial-parsec (x,y,z) -> galactic light-years [gx,gy,gz]. */
function toGalacticLy(x, y, z) {
  const px = x * LY_PER_PC, py = y * LY_PER_PC, pz = z * LY_PER_PC;
  return [
    M[0][0] * px + M[0][1] * py + M[0][2] * pz,
    M[1][0] * px + M[1][1] * py + M[1][2] * pz,
    M[2][0] * px + M[2][1] * py + M[2][2] * pz,
  ];
}

// ── CSV helpers ────────────────────────────────────────────────────────────

const strip = (s) => s.trim().replace(/^"|"$/g, '');

/** Empty or non-numeric -> `def` (JS Number('') is 0, which would lie here). */
function num(s, def) {
  if (s === undefined || s === '') return def;
  const v = Number(s);
  return Number.isFinite(v) ? v : def;
}

/** Truncate a UTF-8 name to at most MAX_NAME_BYTES without splitting a char. */
function truncateUtf8(name) {
  let buf = Buffer.from(name, 'utf8');
  if (buf.length <= MAX_NAME_BYTES) return buf;
  let end = MAX_NAME_BYTES;
  // Back off while the byte at `end` is a UTF-8 continuation byte (10xxxxxx),
  // which would mean we are standing in the middle of a character.
  while (end > 0 && (buf[end] & 0xc0) === 0x80) end--;
  return buf.subarray(0, end);
}

// ── Build ──────────────────────────────────────────────────────────────────

function build() {
  const t0 = Date.now();
  const matrixErr = assertMatrixMatchesPublished();
  console.log(`matrix: built from IAU angles, max |delta| vs Hipparcos published = ${matrixErr.toExponential(2)}`);

  const csv = fs.readFileSync(CSV_PATH, 'utf8');
  const lines = csv.split(/\r?\n/);
  if (lines.length && lines[lines.length - 1] === '') lines.pop();

  const cols = lines[0].split(',').map(strip);
  const idx = (n) => {
    const i = cols.indexOf(n);
    if (i < 0) { console.error(`stars.csv: missing column '${n}'`); process.exit(1); }
    return i;
  };
  const idI = idx('id'), properI = idx('proper');
  const raI = idx('ra'), decI = idx('dec'), distI = idx('dist');
  const magI = idx('mag'), ciI = idx('ci');
  const xI = idx('x'), yI = idx('y'), zI = idx('z');
  const maxIdx = Math.max(idI, properI, raI, decI, distI, magI, ciI, xI, yI, zI);

  const stars = [];
  let read = 0;
  let dropShort = 0, dropSol = 0, dropDist = 0, dropPos = 0;
  let derivedFromRaDec = 0;

  for (let li = 1; li < lines.length; li++) {
    read++;
    const f = lines[li].split(',').map(strip);
    if (f.length <= maxIdx) { dropShort++; continue; }

    const id = num(f[idI], -1);
    const proper = f[properI];
    const dist = num(f[distI], 0);

    // Sol: the map draws it at the origin itself.
    if (id === 0 || proper === 'Sol') { dropSol++; continue; }
    // dist <= 0 = no parallax; >= 100000 = HYG's "unknown distance" sentinel.
    if (!(dist > 0) || dist >= UNKNOWN_DIST_PC) { dropDist++; continue; }

    let x = num(f[xI], 0), y = num(f[yI], 0), z = num(f[zI], 0);
    if (Math.abs(x) + Math.abs(y) + Math.abs(z) < 1e-9) {
      // No usable cartesian columns -- rebuild them from ra (HOURS) / dec
      // (DEGREES) / dist (parsecs), the same way HYG itself does.
      const ra = num(f[raI], NaN), dec = num(f[decI], NaN);
      if (!Number.isFinite(ra) || !Number.isFinite(dec)) { dropPos++; continue; }
      [x, y, z] = equatorialXyzFromRaDec(ra, dec, dist);
      derivedFromRaDec++;
    }

    const [gx, gy, gz] = toGalacticLy(x, y, z);
    if (!Number.isFinite(gx) || !Number.isFinite(gy) || !Number.isFinite(gz)) { dropPos++; continue; }

    stars.push({
      gx, gy, gz,
      mag: num(f[magI], 20.0), // blank magnitude sorts to the faint end
      ci: num(f[ciI], 0.0),
      proper,
      id,
    });
  }

  // Brightest first; HYG id breaks ties so rebuilds are byte-identical.
  stars.sort((a, b) => (a.mag - b.mag) || (a.id - b.id));

  const records = Buffer.alloc(stars.length * RECORD_SIZE);
  const nameParts = [];
  let namedCount = 0;
  let nameBytes = 0;
  for (let i = 0; i < stars.length; i++) {
    const s = stars[i];
    const off = i * RECORD_SIZE;
    records.writeFloatLE(s.gx, off);
    records.writeFloatLE(s.gy, off + 4);
    records.writeFloatLE(s.gz, off + 8);
    records.writeFloatLE(s.mag, off + 12);
    records.writeFloatLE(s.ci, off + 16);

    if (s.proper !== '') {
      const nb = truncateUtf8(s.proper);
      if (nb.length > 0) {
        const e = Buffer.alloc(5 + nb.length);
        e.writeUInt32LE(i, 0);
        e.writeUInt8(nb.length, 4);
        nb.copy(e, 5);
        nameParts.push(e);
        namedCount++;
        nameBytes += e.length;
      }
    }
  }

  const header = Buffer.alloc(HEADER_SIZE);
  header.write(MAGIC, 0, 'ascii');
  header.writeUInt32LE(stars.length, 8);
  header.writeUInt32LE(namedCount, 12);
  const bin = Buffer.concat([header, records, ...nameParts]);
  fs.writeFileSync(BIN_PATH, bin);

  console.log(
    `rows: read ${read} | kept ${stars.length} | dropped ${read - stars.length} ` +
    `(Sol ${dropSol}, bad/unknown dist ${dropDist}, no position ${dropPos}, short row ${dropShort}) ` +
    `| named ${namedCount} | derived xyz from ra/dec ${derivedFromRaDec}`
  );
  console.log(
    `stars-map.bin: ${bin.length} bytes (${(bin.length / 1024 / 1024).toFixed(2)} MiB) = ` +
    `16 header + ${stars.length * RECORD_SIZE} records + ${nameBytes} names, built in ${Date.now() - t0} ms`
  );
}

// ── Verify ─────────────────────────────────────────────────────────────────

/**
 * Independent l/b derivation via the classical spherical formulas, used ONLY
 * by verify so the matrix path and the check path do not share code. Returns
 * galactic [gx,gy,gz] in light-years for HYG-style ra (hours) / dec (deg) /
 * dist (parsecs).
 */
function galacticFromRaDecSpherical(raHours, decDeg, distPc) {
  const ra = raHours * 15 * DEG, dec = decDeg * DEG;
  const aG = ALPHA_NGP_DEG * DEG, dG = DELTA_NGP_DEG * DEG, lNCP = L_NCP_DEG * DEG;
  const sinB = Math.sin(dec) * Math.sin(dG) + Math.cos(dec) * Math.cos(dG) * Math.cos(ra - aG);
  const b = Math.asin(Math.max(-1, Math.min(1, sinB)));
  const yTerm = Math.cos(dec) * Math.sin(ra - aG);
  const xTerm = Math.sin(dec) * Math.cos(dG) - Math.cos(dec) * Math.sin(dG) * Math.cos(ra - aG);
  const l = lNCP - Math.atan2(yTerm, xTerm);
  const d = distPc * LY_PER_PC;
  return [d * Math.cos(b) * Math.cos(l), d * Math.cos(b) * Math.sin(l), d * Math.sin(b)];
}

let checks = 0, failures = 0;
function check(label, ok, detail) {
  checks++;
  if (!ok) failures++;
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail ? ` -- ${detail}` : ''}`);
}

function verify() {
  console.log('verify: re-opening data/stars-map.bin');
  const bin = fs.readFileSync(BIN_PATH);

  // The Rust parser hardcodes these numbers, so assert the LITERALS rather
  // than the constants above -- otherwise editing a constant would silently
  // move the whole format while every self-consistent check still passed.
  check('spec literals: magic "HOSMAP01", 16-byte header, 20-byte record, 32-byte name cap',
    MAGIC === 'HOSMAP01' && HEADER_SIZE === 16 && RECORD_SIZE === 20 && MAX_NAME_BYTES === 32,
    `magic "${MAGIC}", header ${HEADER_SIZE}, record ${RECORD_SIZE}, name cap ${MAX_NAME_BYTES}`);

  const magic = bin.subarray(0, 8).toString('ascii');
  check('magic is "HOSMAP01"', magic === MAGIC, `got "${magic}"`);
  if (magic !== MAGIC) { report(); return; }

  const starCount = bin.readUInt32LE(8);
  const namedCount = bin.readUInt32LE(12);

  // ── Structure: sizes and offsets must line up exactly.
  const namesStart = HEADER_SIZE + starCount * RECORD_SIZE;
  check('star section fits in the file', namesStart <= bin.length,
    `names start ${namesStart}, file ${bin.length}`);

  const names = [];       // { index, name }
  const byName = new Map();
  let off = namesStart;
  let walked = 0;
  let truncatedNames = 0;
  let structureOk = true;
  while (walked < namedCount) {
    if (off + 5 > bin.length) { structureOk = false; break; }
    const si = bin.readUInt32LE(off);
    const len = bin.readUInt8(off + 4);
    if (len === 0 || len > MAX_NAME_BYTES || off + 5 + len > bin.length || si >= starCount) {
      structureOk = false;
      break;
    }
    const raw = bin.subarray(off + 5, off + 5 + len);
    const name = raw.toString('utf8');
    if (Buffer.byteLength(name, 'utf8') !== len) { structureOk = false; break; } // invalid UTF-8 (split char)
    if (len === MAX_NAME_BYTES) truncatedNames++;
    names.push({ index: si, name });
    if (!byName.has(name)) byName.set(name, si);
    off += 5 + len;
    walked++;
  }
  check(`walked all ${namedCount} name records with valid index/len/UTF-8`,
    structureOk && walked === namedCount, `walked ${walked}`);
  check('name section ends exactly at EOF', off === bin.length,
    `ended at ${off}, file is ${bin.length}`);
  check('every name record has a non-empty name <= 32 bytes',
    names.every((n) => n.name.length > 0 && Buffer.byteLength(n.name, 'utf8') <= MAX_NAME_BYTES));
  check('name records are in ascending star_index order with no duplicates',
    names.every((n, i) => i === 0 || n.index > names[i - 1].index));

  const readStar = (i) => {
    const o = HEADER_SIZE + i * RECORD_SIZE;
    return {
      gx: bin.readFloatLE(o), gy: bin.readFloatLE(o + 4), gz: bin.readFloatLE(o + 8),
      mag: bin.readFloatLE(o + 12), ci: bin.readFloatLE(o + 16),
    };
  };

  // ── Counts and sort order.
  // Literal 16/20/5 on purpose (see the spec-literals check above): this is
  // the size arithmetic the Rust reader will do.
  const expectedLen = 16 + starCount * 20 +
    names.reduce((a, n) => a + 5 + Buffer.byteLength(n.name, 'utf8'), 0);
  check('file length matches 16 + 20*star_count + name bytes',
    expectedLen === bin.length, `computed ${expectedLen}, actual ${bin.length}`);

  let minMag = Infinity, minAt = -1, sorted = true, finite = true;
  for (let i = 0; i < starCount; i++) {
    const s = readStar(i);
    if (!Number.isFinite(s.gx) || !Number.isFinite(s.gy) || !Number.isFinite(s.gz) ||
      !Number.isFinite(s.mag) || !Number.isFinite(s.ci)) finite = false;
    if (s.mag < minMag) { minMag = s.mag; minAt = i; }
    if (i > 0 && s.mag < readStar(i - 1).mag) sorted = false;
  }
  check('all record fields are finite', finite);
  check('records are sorted by mag ascending (full scan)', sorted);
  check('first record holds the file minimum mag',
    minAt === 0, `min mag ${minMag} at index ${minAt}, record 0 mag ${readStar(0).mag}`);

  // ── No Sol, no sentinel-distance shell.
  let nearest = Infinity, farthest = 0;
  for (let i = 0; i < starCount; i++) {
    const s = readStar(i);
    const d = Math.hypot(s.gx, s.gy, s.gz);
    if (d < nearest) nearest = d;
    if (d > farthest) farthest = d;
  }
  check('no star sits at the origin (Sol excluded)', nearest > 0.01,
    `nearest star ${nearest.toFixed(4)} ly`);
  check('no "unknown distance" shell (all < 100000 pc)',
    farthest < UNKNOWN_DIST_PC * LY_PER_PC, `farthest ${farthest.toFixed(0)} ly`);
  check('no name record refers to "Sol"', !byName.has('Sol'));

  // ── Astronomy spot-checks. Each names a star, pulls ITS record, and
  // compares against externally known values, so a wrong axis convention, a
  // parsec/light-year slip, or a transposed matrix all fail loudly.
  const spot = (name, expectDistLy, distTol, note) => {
    if (!byName.has(name)) { check(`"${name}" present in the names section`, false); return null; }
    check(`"${name}" present in the names section`, true, `star_index ${byName.get(name)}`);
    const i = byName.get(name);
    const s = readStar(i);
    const d = Math.hypot(s.gx, s.gy, s.gz);
    check(`"${name}" distance ~= ${expectDistLy} ly`, Math.abs(d - expectDistLy) <= distTol,
      `got ${d.toFixed(3)} ly (mag ${s.mag.toFixed(2)}${note ? `, ${note}` : ''})`);
    return { i, s, d };
  };

  const sirius = spot('Sirius', 8.6, 0.2);
  if (sirius) {
    check('Sirius is among the brightest 20 records', sirius.i < 20, `index ${sirius.i}`);
  }
  spot('Rigil Kentaurus', 4.4, 0.2, 'HYG name for Alpha Centauri A');

  // Polaris: a celestial-pole star must NOT land near the galactic pole.
  // Its true galactic latitude is +26.46 deg, so |gz|/d = sin(26.46).
  const polaris = spot('Polaris', 432.6, 2.0);
  if (polaris) {
    const bDeg = Math.asin(polaris.s.gz / polaris.d) / DEG;
    check('Polaris galactic latitude ~= +26.5 deg (frame really is galactic)',
      Math.abs(bDeg - 26.5) < 3.0,
      `b = ${bDeg.toFixed(2)} deg, |gz|/dist = ${Math.abs(polaris.s.gz / polaris.d).toFixed(4)} ` +
      `(sin 26.5 = ${Math.sin(26.5 * DEG).toFixed(4)}); a pole-aligned bug would give ~90 deg`);
  }

  // ── Independent re-derivation straight from the CSV. Two flavours:
  //   (a) row policy: recount which rows belong in the file at all;
  //   (b) geometry: recompute a spread of named stars through the SPHERICAL
  //       l/b formulas, a different derivation than the matrix the file was
  //       built with, so a bad matrix cannot agree with itself.
  const csv = fs.readFileSync(CSV_PATH, 'utf8');
  const lines = csv.split(/\r?\n/);
  if (lines.length && lines[lines.length - 1] === '') lines.pop();
  const cols = lines[0].split(',').map(strip);
  const cIdx = {
    id: cols.indexOf('id'), proper: cols.indexOf('proper'), ra: cols.indexOf('ra'),
    dec: cols.indexOf('dec'), dist: cols.indexOf('dist'), mag: cols.indexOf('mag'),
    ci: cols.indexOf('ci'), x: cols.indexOf('x'), y: cols.indexOf('y'), z: cols.indexOf('z'),
  };
  const wanted = new Map(); // proper -> csv fields
  const sample = ['Sirius', 'Rigil Kentaurus', 'Polaris', 'Canopus', 'Vega', 'Betelgeuse',
    'Rigel', 'Arcturus', 'Deneb', 'Proxima Centauri', 'Barnard\'s Star', 'Aldebaran'];
  let csvKept = 0, csvNamed = 0, csvMinMag = Infinity, csvMaxMag = -Infinity;
  const fallbackSample = [];
  for (let li = 1; li < lines.length; li++) {
    const f = lines[li].split(',').map(strip);
    const p = f[cIdx.proper];
    const dist = num(f[cIdx.dist], 0);
    if (num(f[cIdx.id], -1) === 0 || p === 'Sol') continue;
    if (!(dist > 0) || dist >= UNKNOWN_DIST_PC) continue;
    csvKept++;
    if (csvKept % 137 === 0) fallbackSample.push(f); // co-prime stride, whole-catalog spread
    if (p !== '') csvNamed++;
    const m = num(f[cIdx.mag], 20.0);
    if (m < csvMinMag) csvMinMag = m;
    if (m > csvMaxMag) csvMaxMag = m;
    if (p && sample.includes(p) && !wanted.has(p)) wanted.set(p, f);
  }
  check('star_count equals an independent recount of eligible CSV rows',
    csvKept === starCount, `csv ${csvKept} vs file ${starCount}`);
  check('named_count equals an independent recount of eligible rows with a proper name',
    csvNamed === namedCount, `csv ${csvNamed} vs file ${namedCount}`);
  check('first/last record mags are the CSV min/max',
    Math.abs(readStar(0).mag - csvMinMag) < 1e-5 &&
    Math.abs(readStar(starCount - 1).mag - csvMaxMag) < 1e-5,
    `file [${readStar(0).mag.toFixed(3)}, ${readStar(starCount - 1).mag.toFixed(3)}] ` +
    `vs csv [${csvMinMag.toFixed(3)}, ${csvMaxMag.toFixed(3)}]`);

  // The ra/dec fallback is dead code for today's stars.csv (every eligible row
  // has x,y,z), so exercise it head-on: for a spread of rows that DO have
  // x,y,z, the fallback must reproduce their direction. Without this, an
  // "ra is in degrees" style bug would sit undetected until a future catalog
  // revision started using the fallback.
  let fbChecked = 0, worstFb = 0;
  for (const f of fallbackSample) {
    const x = num(f[cIdx.x], 0), y = num(f[cIdx.y], 0), z = num(f[cIdx.z], 0);
    const r = Math.hypot(x, y, z);
    if (!(r > 0)) continue;
    const [ax, ay, az] = equatorialXyzFromRaDec(
      num(f[cIdx.ra], NaN), num(f[cIdx.dec], NaN), num(f[cIdx.dist], NaN));
    const ra = Math.hypot(ax, ay, az);
    const e = Math.hypot(x / r - ax / ra, y / r - ay / ra, z / r - az / ra);
    if (e > worstFb) worstFb = e;
    fbChecked++;
  }
  check(`ra/dec fallback reproduces the CSV x,y,z direction on ${fbChecked} sampled rows`,
    fbChecked >= 500 && worstFb < 3e-3,
    `worst unit-direction error ${worstFb.toExponential(2)}, tol 0.003`);

  // HYG's own x,y,z are not perfectly consistent with its rounded ra/dec/dist
  // columns (measured worst over a 792-row sample: 2.9e-4 in unit direction,
  // 4.8e-4 relative in radius), so the tolerances below sit just above that
  // catalog noise. They are still ~1000x tighter than any real coordinate bug:
  // a transposed matrix, a swapped axis or a pc/ly slip all move a star by an
  // order-1 fraction of its distance.
  const DIR_TOL = 2e-3;   // unit-vector L2 distance (~7 arcmin)
  const RAD_TOL = 5e-3;   // relative radius error
  let crossChecked = 0, worstDir = 0, worstDirWho = '', worstRad = 0, worstRadWho = '';
  let magMismatch = 0, ciMismatch = 0;
  for (const [p, f] of wanted) {
    if (!byName.has(p)) continue;
    const s = readStar(byName.get(p));
    const [ex, ey, ez] = galacticFromRaDecSpherical(
      num(f[cIdx.ra], NaN), num(f[cIdx.dec], NaN), num(f[cIdx.dist], NaN));
    const de = Math.hypot(ex, ey, ez), df = Math.hypot(s.gx, s.gy, s.gz);
    const dirErr = Math.hypot(s.gx / df - ex / de, s.gy / df - ey / de, s.gz / df - ez / de);
    const radErr = Math.abs(df - de) / de;
    if (dirErr > worstDir) { worstDir = dirErr; worstDirWho = p; }
    if (radErr > worstRad) { worstRad = radErr; worstRadWho = p; }
    if (Math.abs(s.mag - num(f[cIdx.mag], NaN)) > 1e-4) magMismatch++;
    if (Math.abs(s.ci - num(f[cIdx.ci], 0.0)) > 1e-4) ciMismatch++;
    crossChecked++;
  }
  check(`${crossChecked} named stars: direction matches an independent spherical l/b derivation`,
    crossChecked >= 8 && worstDir < DIR_TOL,
    `worst unit-direction error ${worstDir.toExponential(2)} (${worstDirWho}), tol ${DIR_TOL}`);
  check(`${crossChecked} named stars: radius matches dist * 3.2615638 ly/pc`,
    crossChecked >= 8 && worstRad < RAD_TOL,
    `worst relative radius error ${worstRad.toExponential(2)} (${worstRadWho}), tol ${RAD_TOL}`);
  check('cross-checked stars carry the CSV magnitude verbatim', magMismatch === 0,
    `${magMismatch} mismatched`);
  check('cross-checked stars carry the CSV color index verbatim', ciMismatch === 0,
    `${ciMismatch} mismatched`);

  // A whole-file ci sanity: HYG has a color index for the overwhelming
  // majority of rows, so an all-zero (or constant) ci column means the field
  // was dropped somewhere between the CSV and the record.
  let ciNonZero = 0;
  for (let i = 0; i < starCount; i++) if (readStar(i).ci !== 0) ciNonZero++;
  check('color index is populated for most stars (field not dropped)',
    ciNonZero > starCount * 0.9, `${ciNonZero}/${starCount} records have ci != 0`);

  console.log(
    `verify: star_count ${starCount}, named_count ${namedCount}, ` +
    `${truncatedNames} names at the 32-byte cap, nearest ${nearest.toFixed(2)} ly, ` +
    `farthest ${farthest.toFixed(0)} ly, brightest mag ${minMag.toFixed(2)}`
  );
  report();
}

function report() {
  if (failures > 0) {
    console.error(`verify: ${failures}/${checks} checks FAILED`);
    process.exit(1);
  }
  console.log(`verify: all ${checks} checks passed`);
}

function main() {
  const verifyOnly = process.argv.includes('--verify');
  if (!verifyOnly) build();
  verify();
}

main();
