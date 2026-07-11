#!/usr/bin/env node
/**
 * build-gaia-bin.js
 *
 * Converts a Gaia G<14 extraction (~25M stars, chunked CSVs pulled from the
 * ESA Gaia TAP service) into the same HOSSTAR1 binary the renderer parses
 * (format spec: scripts/build-stars-bin.js; loader:
 * src/renderer/stars.rs::StarCatalog::from_bin). This is star-catalog
 * ladder RUNG 4: the ULTRA tier offered in Settings > Graphics, ~350 MB,
 * uploaded as a GitHub release asset under the non-v* tag `assets-stars-1`
 * (no CI trigger, free CDN) and downloaded in-app as
 * data/stars-gaia25m.bin.
 *
 * Usage:
 *   node scripts/build-gaia-bin.js <chunk1.csv[.gz]> [...more chunks]
 *       writes data/stars-gaia25m.bin (streaming; peak memory ~one 4 MB
 *       chunk, never the whole 375 MB)
 *   node scripts/build-gaia-bin.js --fixture [out.bin]
 *       writes the tiny 3-star test fixture (default
 *       tests/fixtures/stars-gaia-fixture.bin) that the Rust round-trip
 *       tests read. KEEP THE FIXTURE STAR LIST IN SYNC with
 *       stars.rs::gaia_fixture_parses_packs_and_survives_the_cutoff.
 *
 * Input CSV header: ra,dec,phot_g_mean_mag,bp_rp (column order free; extra
 * columns tolerated - resolved by NAME). COORDINATES ARE DEGREES: the Gaia
 * TAP service outputs ra/dec in DEGREES (ICRS, epoch 2016), never hours.
 * A wrong units assumption would compress all RA into 1/15th of the sky,
 * which the anchor frame check below catches loudly at build time.
 *
 * Mapping (keep in sync with the Rust fixture-test expectations):
 *   direction: unit vector from ra/dec degrees, same equatorial frame as
 *     HYG's x,y,z columns. Unit by construction - no normalize needed.
 *   mag = phot_g_mean_mag. Rows with empty/garbage mag are SKIPPED (a star
 *     we cannot brightness-scale would render as a wrong-brightness dot).
 *   ci  = 0.85*bp_rp - 0.1, the same empirical BP-RP -> B-V linearization
 *     scripts/build-galaxy-glow.js uses; empty/garbage bp_rp -> 0.65
 *     (neutral warm, also matching the glow bake).
 *   named-star sidecar: NONE (named_count = 0). Gaia sources are unnamed.
 *     The Rust loader compensates: StarCatalog::adopt_names_from_sidecar
 *     borrows the name sidecar from stars.bin so constellation figures
 *     keep resolving with the ultra catalog active. Do NOT bolt a sidecar
 *     on here without re-checking that fall-through.
 *
 * Self-checks:
 *   1. ANCHOR FRAME CHECK: bright named-star directions read from the
 *      data/stars.bin sidecar must reappear among the input's bright rows
 *      within 0.1 degrees. Gaia's epoch-2016 proper motion moves Sirius
 *      ~21 arcsec off its J2000 place (Arcturus ~37 arcsec) - well inside
 *      the tolerance - while a frame/units mistake moves everything by
 *      DEGREES, so discrimination is total. Sirius (G ~ -1.09) is reported
 *      explicitly when found; Gaia is known-incomplete for the very
 *      brightest saturated stars, so the check passes on >= 3 matched
 *      anchors even if Sirius itself is absent from the extraction.
 *   2. SPOT VERIFY: sampled records re-read from the written file must
 *      byte-match a fresh recomputation (catches offset/encode bugs at
 *      build time instead of as a corrupted sky).
 */

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const readline = require('readline');

const ROOT = path.resolve(__dirname, '..');
const OUT_PATH = path.join(ROOT, 'data', 'stars-gaia25m.bin');
const STD_BIN = path.join(ROOT, 'data', 'stars.bin');

const MAGIC = 'HOSSTAR1';
const HEADER_SIZE = 16;
const RECORD_SIZE = 15;
const DEG = Math.PI / 180;

// Anchor stars for the frame check: bright, spread over both hemispheres,
// all guaranteed present in the stars.bin sidecar (HYG proper names).
const ANCHOR_NAMES = [
  'sirius', 'canopus', 'arcturus', 'vega', 'capella', 'rigel', 'procyon',
  'betelgeuse', 'achernar', 'altair', 'aldebaran', 'spica', 'antares',
  'pollux', 'fomalhaut', 'deneb', 'regulus',
];
const ANCHOR_TOL_DOT = Math.cos(0.1 * DEG);

/** ra/dec (DEGREES) -> unit equatorial direction, HYG x,y,z convention. */
function dirFromRaDec(raDeg, decDeg) {
  const ra = raDeg * DEG;
  const dec = decDeg * DEG;
  const cd = Math.cos(dec);
  return [cd * Math.cos(ra), cd * Math.sin(ra), Math.sin(dec)];
}

/** BP-RP color string -> B-V-ish ci (see the doc header for the mapping). */
function ciFromBpRp(s) {
  if (s === '' || s === undefined) return 0.65;
  const v = Number(s);
  if (Number.isNaN(v)) return 0.65;
  return 0.85 * v - 0.1;
}

/** Encode one 15-byte HOSSTAR1 record (identical math to the other two
 *  converter scripts; the Rust loader dequantizes the mirror image). */
function encodeRecord(buf, off, dx, dy, dz, mag, ciRaw) {
  buf.writeFloatLE(dx, off);
  buf.writeFloatLE(dy, off + 4);
  buf.writeFloatLE(dz, off + 8);
  buf.writeUInt16LE(Math.min(65535, Math.max(0, Math.round((mag + 2.0) * 1024))), off + 12);
  const civ = Math.min(2.0, Math.max(-0.4, ciRaw));
  buf.writeUInt8(Math.round(((civ + 0.4) / 2.4) * 255), off + 14);
}

/** Scan a HOSSTAR1 buffer's sidecar for a kind-0 (proper name) key.
 *  (Same walker as build-athyg-bin.js.) */
function findNamed(bin, wanted) {
  const starCount = bin.readUInt32LE(8);
  const namedCount = bin.readUInt32LE(12);
  let o = HEADER_SIZE + starCount * RECORD_SIZE;
  for (let i = 0; i < namedCount; i++) {
    const kind = bin.readUInt8(o);
    const klen = bin.readUInt8(o + 1);
    const key = bin.subarray(o + 2, o + 2 + klen).toString('utf8');
    const d = o + 2 + klen;
    if (kind === 0 && key === wanted) {
      return [bin.readFloatLE(d), bin.readFloatLE(d + 4), bin.readFloatLE(d + 8)];
    }
    o = d + 12;
  }
  return null;
}

/** All sidecar entries of one kind as [key, direction] pairs (kind 1 =
 *  Bayer keys - the fainter, Gaia-present anchor pool). */
function allNamed(bin, wantedKind) {
  const starCount = bin.readUInt32LE(8);
  const namedCount = bin.readUInt32LE(12);
  let o = HEADER_SIZE + starCount * RECORD_SIZE;
  const out = [];
  for (let i = 0; i < namedCount; i++) {
    const kind = bin.readUInt8(o);
    const klen = bin.readUInt8(o + 1);
    const key = bin.subarray(o + 2, o + 2 + klen).toString('utf8');
    const d = o + 2 + klen;
    if (kind === wantedKind) {
      out.push([key, [bin.readFloatLE(d), bin.readFloatLE(d + 4), bin.readFloatLE(d + 8)]]);
    }
    o = d + 12;
  }
  return out;
}

// ── Fixture mode ────────────────────────────────────────────────────────
// KEEP IN SYNC with stars.rs::gaia_fixture_parses_packs_and_survives_the_cutoff.
// Three stars chosen to exercise the pipeline's edges: Sirius (bright clamp +
// the cross-catalog frame anchor), a red galactic-center marker (ci near the
// top of the quantization domain), and a faint pole star at the G<14 boundary
// with EMPTY bp_rp (the ci=0.65 default + the dec=90 direction edge + the
// dimmest star that must still survive the renderer's brightness cutoff).
const FIXTURE_STARS = [
  { ra: 101.28715533, dec: -16.71611586, mag: -1.09, bp_rp: '0.00' },
  { ra: 266.405, dec: -28.936, mag: 8.0, bp_rp: '2.00' },
  { ra: 0.0, dec: 90.0, mag: 13.9, bp_rp: '' },
];

function writeFixture(outPath) {
  const buf = Buffer.alloc(HEADER_SIZE + FIXTURE_STARS.length * RECORD_SIZE);
  buf.write(MAGIC, 0, 'ascii');
  buf.writeUInt32LE(FIXTURE_STARS.length, 8);
  buf.writeUInt32LE(0, 12); // NAMELESS, exactly like the real ultra catalog
  FIXTURE_STARS.forEach((s, i) => {
    const [dx, dy, dz] = dirFromRaDec(s.ra, s.dec);
    encodeRecord(buf, HEADER_SIZE + i * RECORD_SIZE, dx, dy, dz, s.mag, ciFromBpRp(s.bp_rp));
  });
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, buf);

  // Frame sanity for the fixture itself: fixture star 0 IS Sirius (J2000),
  // so it must agree with the standard catalog's sidecar direction.
  if (fs.existsSync(STD_BIN)) {
    const std = fs.readFileSync(STD_BIN);
    const s = findNamed(std, 'sirius');
    if (s) {
      const [dx, dy, dz] = dirFromRaDec(FIXTURE_STARS[0].ra, FIXTURE_STARS[0].dec);
      const dot = dx * s[0] + dy * s[1] + dz * s[2];
      if (dot < Math.cos(0.05 * DEG)) {
        throw new Error(`fixture frame check FAILED: sirius dot = ${dot}`);
      }
      console.log(`fixture frame check: sirius agrees with stars.bin (dot = ${dot.toFixed(8)})`);
    }
  }
  console.log(`fixture: ${FIXTURE_STARS.length} stars (nameless) -> ${outPath} (${buf.length} B)`);
}

// ── Full-catalog streaming build ────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--fixture') {
    const out = args[1]
      ? path.resolve(args[1])
      : path.join(ROOT, 'tests', 'fixtures', 'stars-gaia-fixture.bin');
    writeFixture(out);
    return;
  }
  if (args.length === 0) {
    console.error('usage: node scripts/build-gaia-bin.js <gaia-chunk.csv[.gz]> [...more chunks]');
    console.error('       node scripts/build-gaia-bin.js --fixture [out.bin]');
    process.exit(1);
  }

  const t0 = Date.now();

  // Anchor directions from the standard catalog's name sidecar. LESSON from
  // the first real run (2026-07-11): the proper-name anchors are all G < 3
  // giants that Gaia DR3 OMITS (saturated) - 0/17 matched and the check
  // cried "wrong frame" when the frame was perfect (independently verified:
  // the and / lam cas / kap cas matched Gaia rows to < 0.013 deg). So the
  // anchor set now also samples the BAYER sidecar (mostly mag 2.5-6 stars
  // Gaia does contain), spread across the sky, and the row filter below
  // admits mag < 9 so those anchors can actually meet their counterparts.
  const anchors = [];
  if (fs.existsSync(STD_BIN)) {
    const std = fs.readFileSync(STD_BIN);
    for (const name of ANCHOR_NAMES) {
      const d = findNamed(std, name);
      if (d) anchors.push({ name, d, bestDot: -1, bestMag: null });
    }
    // Every 40th Bayer entry: ~40 fainter anchors spread over both
    // hemispheres (sidecar order follows the catalog, which is sky-mixed).
    const bayer = allNamed(std, 1);
    for (let i = 0; i < bayer.length; i += 40) {
      const [key, d] = bayer[i];
      anchors.push({ name: key, d, bestDot: -1, bestMag: null });
    }
  }
  if (anchors.length === 0) {
    console.warn('frame check DISABLED: data/stars.bin missing or has no name sidecar');
  }

  // Streaming write: a placeholder header now, the real star count patched
  // in at the end. Keeps peak memory at one 4 MB chunk instead of buffering
  // the whole ~375 MB output like the smaller converters do.
  const fd = fs.openSync(OUT_PATH, 'w');
  fs.writeSync(fd, Buffer.alloc(HEADER_SIZE));

  const CHUNK_STARS = 262_144;
  let chunk = Buffer.alloc(CHUNK_STARS * RECORD_SIZE);
  let chunkUsed = 0;
  let starCount = 0;
  let skippedNoMag = 0;
  let skippedBadRow = 0;

  // Spot-verify samples: a prime stride spreads them across every chunk
  // file; the last record is added after the loop. Values are retained so
  // verification recomputes the record independently of the write path.
  const SAMPLE_STRIDE = 1_000_003;
  const samples = [];
  let last = null; // scalars of the most recent record (avoids 25M object churn)

  let cols = null;
  let ri, di, mi, bi, maxIdx;
  const strip = (s) => s.trim().replace(/^"|"$/g, '');

  for (const input of args) {
    const raw = fs.createReadStream(input);
    const stream = input.endsWith('.gz') ? raw.pipe(zlib.createGunzip()) : raw;
    const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
    let firstLine = true;
    for await (const line of rl) {
      if (firstLine) {
        firstLine = false;
        const hdr = line.split(',').map(strip);
        if (cols === null) {
          // The first file must carry the header row; columns are resolved
          // by NAME so extra columns / different order still work.
          if (!hdr.includes('ra') || !hdr.includes('dec')) {
            console.error(
              `${input}: first input must carry a header with ra,dec,phot_g_mean_mag,bp_rp ` +
              `(got '${line.slice(0, 60)}')`
            );
            process.exit(1);
          }
          cols = hdr;
          ri = cols.indexOf('ra');
          di = cols.indexOf('dec');
          mi = cols.indexOf('phot_g_mean_mag');
          bi = cols.indexOf('bp_rp');
          for (const [name, i] of [['ra', ri], ['dec', di], ['phot_g_mean_mag', mi], ['bp_rp', bi]]) {
            if (i < 0) {
              console.error(`${input}: missing column '${name}'`);
              process.exit(1);
            }
          }
          maxIdx = Math.max(ri, di, mi, bi);
          continue;
        }
        if (hdr.join(',') === cols.join(',')) continue; // later chunk repeating the header
        // Headerless continuation chunk: fall through, parse this line as DATA.
      }
      const f = line.split(',').map(strip);
      if (f.length <= maxIdx) { skippedBadRow++; continue; }

      // No magnitude = no brightness to render; skip rather than guess.
      const magStr = f[mi];
      const mag = Number(magStr);
      if (magStr === '' || Number.isNaN(mag)) { skippedNoMag++; continue; }

      const ra = Number(f[ri]);
      const dec = Number(f[di]);
      if (Number.isNaN(ra) || Number.isNaN(dec)) { skippedBadRow++; continue; }

      const [dx, dy, dz] = dirFromRaDec(ra, dec);
      const ciRaw = ciFromBpRp(f[bi]);

      if (chunkUsed === CHUNK_STARS) {
        fs.writeSync(fd, chunk);
        chunkUsed = 0;
      }
      encodeRecord(chunk, chunkUsed * RECORD_SIZE, dx, dy, dz, mag, ciRaw);
      chunkUsed++;

      // Frame-check anchors: mag < 9 admits the fainter Bayer anchors that
      // Gaia actually contains (see the anchor-set lesson above). Roughly a
      // couple million of 17M rows x ~55 anchors - a few seconds, worth it.
      if (mag < 9 && anchors.length) {
        for (const a of anchors) {
          const dot = dx * a.d[0] + dy * a.d[1] + dz * a.d[2];
          if (dot > a.bestDot) { a.bestDot = dot; a.bestMag = mag; }
        }
      }

      if (starCount % SAMPLE_STRIDE === 0) {
        samples.push({ index: starCount, dx, dy, dz, mag, ciRaw });
      }
      last = { index: starCount, dx, dy, dz, mag, ciRaw };
      starCount++;
      if (starCount % 5_000_000 === 0) {
        console.log(`  ${starCount / 1e6}M stars in ${((Date.now() - t0) / 1000).toFixed(0)} s ...`);
      }
    }
  }
  if (chunkUsed > 0) fs.writeSync(fd, chunk.subarray(0, chunkUsed * RECORD_SIZE));
  if (last && last.index !== samples[samples.length - 1]?.index) samples.push(last);

  // Patch the real header in place. u32 star_count holds 25M with room to
  // spare (< 2^32); named_count stays 0 (see the doc header).
  const header = Buffer.alloc(HEADER_SIZE);
  header.write(MAGIC, 0, 'ascii');
  header.writeUInt32LE(starCount, 8);
  header.writeUInt32LE(0, 12);
  fs.writeSync(fd, header, 0, HEADER_SIZE, 0);
  fs.closeSync(fd);

  if (starCount === 0) throw new Error('no stars written - wrong input files?');

  // ── Self-check 2: spot verify the written file ──
  verifyFile(OUT_PATH, samples, starCount);

  // ── Self-check 1: the anchor frame check ──
  if (anchors.length) {
    const matched = anchors.filter((a) => a.bestDot >= ANCHOR_TOL_DOT);
    const sirius = anchors.find((a) => a.name === 'sirius');
    if (sirius && sirius.bestDot >= ANCHOR_TOL_DOT) {
      console.log(
        `frame check: sirius found in input (dot = ${sirius.bestDot.toFixed(8)}, G = ${sirius.bestMag})`
      );
    } else {
      console.warn(
        'frame check: sirius NOT among the bright input rows (Gaia is incomplete for the very ' +
        'brightest saturated stars) - relying on the other anchors'
      );
    }
    console.log(
      `frame check: ${matched.length}/${anchors.length} anchors matched within 0.1 deg: ` +
      matched.map((a) => a.name).join(', ')
    );
    // Threshold 5: the proper-name giants may ALL be absent (saturated out
    // of Gaia), but a healthy fraction of the ~40 Bayer anchors must land -
    // a frame/unit error misses every single one by whole degrees.
    if (matched.length < 5) {
      throw new Error(
        'frame check FAILED: fewer than 5 anchor stars matched - ra/dec units or frame are ' +
        'wrong (hours vs degrees? swapped columns?)'
      );
    }
  }

  const mb = (HEADER_SIZE + starCount * RECORD_SIZE) / 1_048_576;
  console.log(
    `stars-gaia25m.bin: ${starCount} stars (nameless) -> ${mb.toFixed(1)} MB; ` +
    `skipped ${skippedNoMag} magless + ${skippedBadRow} malformed rows; ${Date.now() - t0} ms`
  );
}

/** Re-read sampled records from the finished file and byte-compare against
 *  an independent recomputation. */
function verifyFile(file, samples, starCount) {
  const fd = fs.openSync(file, 'r');
  const head = Buffer.alloc(HEADER_SIZE);
  fs.readSync(fd, head, 0, HEADER_SIZE, 0);
  if (head.subarray(0, 8).toString('ascii') !== MAGIC) throw new Error('verify: bad magic');
  if (head.readUInt32LE(8) !== starCount) throw new Error('verify: star count mismatch');
  if (head.readUInt32LE(12) !== 0) throw new Error('verify: ultra catalog must be nameless');
  const rec = Buffer.alloc(RECORD_SIZE);
  const expect = Buffer.alloc(RECORD_SIZE);
  for (const s of samples) {
    fs.readSync(fd, rec, 0, RECORD_SIZE, HEADER_SIZE + s.index * RECORD_SIZE);
    encodeRecord(expect, 0, s.dx, s.dy, s.dz, s.mag, s.ciRaw);
    if (!rec.equals(expect)) throw new Error(`verify: record mismatch at star ${s.index}`);
  }
  fs.closeSync(fd);
  console.log(`verify: ${samples.length} sampled records byte-match recomputation`);
}

main().catch((e) => { console.error(e); process.exit(1); });
