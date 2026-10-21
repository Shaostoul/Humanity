// The web Maps page's orbit model (web/shared/sol-orbits.js), the web twin of
// src/cosmos.rs and the Maps page's belts (Maps increment 2), fed the real
// data files from disk. Pure node, no browser: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/sol-orbits-web.test.js
//
// What it holds: the planets go round the Sun the way they really do
// (counter-clockwise seen from ecliptic north, BUG-140 on the app's side), at
// their real distances; the trojans of the shipped belts gather 60 degrees
// either side of Jupiter on the page's date, as the app's own test finds; and
// the colours come from sol.json's map_color through the app's exposure curve.
//
// Seen red 2026-10-21 with the perifocal Q axis negated (every orbit run
// backwards): "AssertionError [ERR_ASSERTION]: mercury goes round the Sun
// clockwise seen from ecliptic north".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "..", "..");
// The page fetches /data/...; here the same files come off the disk.
global.fetch = (url) =>
  Promise.resolve({
    ok: true,
    json: () => Promise.resolve(JSON.parse(fs.readFileSync(path.join(ROOT, url.replace(/^\//, "")), "utf8"))),
  });
const Sol = require("../../web/shared/sol-orbits.js");

// 2026-09-27 12:00 UTC, the app's snapshot date.
const T = Sol.secondsSinceJ2000(Date.UTC(2026, 8, 27, 12, 0, 0));

test("the planets go round the Sun counter-clockwise from ecliptic north, at their real distances", async () => {
  const sol = await Sol.loadSol();
  assert.ok(sol && sol.orbits.size > 50, "sol.json did not load");
  for (const id of ["mercury", "venus", "earth", "mars", "jupiter"]) {
    const orb = sol.orbits.get(id);
    const a = Sol.positionAU(orb, T);
    const b = Sol.positionAU(orb, T + 5 * 86400);
    const cross = a[0] * b[1] - a[1] * b[0];
    assert.ok(cross > 0, `${id} goes round the Sun clockwise seen from ecliptic north`);
    const r = Math.hypot(...a);
    assert.ok(Math.abs(r / orb.a - 1) <= orb.e + 1e-9, `${id} is ${r.toFixed(3)} AU out, off its orbit`);
  }
  // The body is a point of its own orbit (the app's exact-vertex rule).
  const mars = sol.orbits.get("mars");
  const at = Sol.orbitPoint(mars, Sol.eccentricAnomalyAt(mars, T), T);
  assert.deepStrictEqual(at, Sol.positionAU(mars, T));
});

test("the shipped belts load, and Jupiter's trojans swarm sixty degrees either side of it", async () => {
  const sol = await Sol.loadSol();
  const belts = await Sol.loadBelts();
  assert.ok(belts && belts.n >= 19000, "belts.json did not load");
  assert.match(belts.credit, /Minor Planet Center/);
  const pos = Sol.propagateBelts(belts, T);
  const j = Sol.positionAU(sol.orbits.get("jupiter"), T);
  const tro = belts.classes.findIndex((c) => c.id === "trojan");
  const jl = Math.atan2(j[1], j[0]);
  let all = 0;
  let near = 0;
  let ahead = 0;
  for (let i = 0; i < belts.n; i++) {
    if (belts.cls[i] !== tro) continue;
    all++;
    let d = Math.atan2(pos[i * 3 + 1], pos[i * 3]) - jl;
    d = ((d + 3 * Math.PI) % (2 * Math.PI)) - Math.PI;
    const deg = Math.abs(d) * 180 / Math.PI;
    if (deg >= 40 && deg <= 80) {
      near++;
      if (d > 0) ahead++;
    }
  }
  assert.ok(near / all > 0.6, `only ${Math.round((100 * near) / all)}% of the trojans are 40 to 80 degrees from Jupiter`);
  assert.ok(ahead > all / 5 && near - ahead > all / 5, `the two swarms: ${ahead} ahead, ${near - ahead} behind`);
});

test("a body's colour is its map_color through the app's exposure curve", async () => {
  const sol = await Sol.loadSol();
  const mars = Sol.iconColours(sol.bodies.get("mars").map_color);
  const [r, g, b] = mars.lit.match(/\d+/g).map(Number);
  assert.ok(r > g + 80 && r > b + 100, `Mars is not red: ${mars.lit}`);
  const comet = Sol.iconColours(sol.bodies.get("halley").map_color);
  assert.ok(Number(comet.lit.match(/\d+/)[0]) > 100, `a 4% comet is drawn too dark to see: ${comet.lit}`);
  assert.ok(Sol.glareAt(0.5) > 0.9 && Sol.glareAt(25) < 0.003, "the glare does not fall off as the app's");
});
