// What each rig needs from the machine (scripts/lib/rig-needs.js, 2026-10-22): a rig whose checks
// are about behaviour runs beside the operator's GPU-heavy program with its game in LOW IMPACT; a
// rig that judges the picture or times frames waits for it and refuses low impact. Pure node, no
// game: runs in `just rig-tests`.
//
// RED FIRST (2026-10-22): with the table written and no rig yet asking it, "every rig that starts
// the game plans its boot" failed with 'boot-timing.js starts the game as "boot-timing" but never
// asks rig-needs for its plan'; with RIGS' "verify-falls" entry renamed away, "every rig that
// starts the game is in the table" failed with 'verify-falls.js boots as "verify-falls", which
// scripts/lib/rig-needs.js RIGS does not name'.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const RN = require("../lib/rig-needs.js");

const SCRIPTS = path.resolve(__dirname, "..");
const HD2 = [{ pid: 7777, name: "helldivers2.exe", exe: "D:\\Games\\helldivers2.exe" }];

/** Every top-level rig script that starts the game, with the rigName(s) it passes spawnGame. */
function bootingRigs() {
  const out = [];
  for (const f of fs.readdirSync(SCRIPTS)) {
    if (!f.endsWith(".js")) continue;
    const text = fs.readFileSync(path.join(SCRIPTS, f), "utf8");
    if (!/\bspawnGame\(/.test(text)) continue;
    const names = [...new Set([...text.matchAll(/rigName:\s*"([^"]+)"/g)].map((m) => m[1]))];
    out.push({ file: f, text, names });
  }
  return out;
}

test("every rig that starts the game is in the table, by the name it boots under", () => {
  const rigs = bootingRigs();
  assert.ok(rigs.length >= 9, `found only ${rigs.length} rig scripts that start the game; the scan is broken`);
  for (const r of rigs) {
    assert.ok(r.names.length, `${r.file} starts the game without a literal rigName: the table cannot be checked against it`);
    for (const n of r.names) assert.ok(RN.RIGS[n], `${r.file} boots as "${n}", which scripts/lib/rig-needs.js RIGS does not name: say which kind it is`);
  }
});

test("every rig that starts the game plans its boot through the table", () => {
  for (const r of bootingRigs()) {
    for (const n of r.names) {
      assert.ok(
        r.text.includes(`planOrRefuse("${n}"`) || r.text.includes(`planBoot("${n}"`),
        `${r.file} starts the game as "${n}" but never asks rig-needs for its plan: it would neither run light beside the operator's program nor refuse low impact`,
      );
    }
  }
});

test("a rig that judges the picture or times frames refuses low impact and waits for his program", () => {
  const visual = Object.keys(RN.RIGS).filter((k) => ["picture", "timing"].includes(RN.RIGS[k].kind));
  assert.ok(visual.includes("probe-sweep") && visual.includes("photograph-home") && visual.includes("make-clips") && visual.includes("boot-timing"));
  for (const rig of visual) {
    const asked = RN.planBoot(rig, { argv: ["--low-impact"], env: {}, apps: [] });
    assert.ok(asked.refuse && asked.refuse.join(" ").includes("REFUSED"), `${rig} ran with --low-impact`);
    const inherited = RN.planBoot(rig, { argv: [], env: { HUMANITY_LOW_IMPACT: "1" }, apps: [] });
    assert.ok(inherited.refuse, `${rig} ran with HUMANITY_LOW_IMPACT inherited from the shell`);
    const off = RN.planBoot(rig, { argv: [], env: { HUMANITY_LOW_IMPACT: "0" }, apps: [] });
    assert.strictEqual(off.refuse, null, `${rig} refused HUMANITY_LOW_IMPACT=0, which is off`);
    const beside = RN.planBoot(rig, { argv: [], env: {}, apps: HD2 });
    assert.strictEqual(beside.lowImpact, null, `${rig} would boot in low impact`);
    assert.ok(!beside.own.lowImpact, `${rig} would not wait for the operator's program`);
    assert.match(beside.why, /waits for helldivers2\.exe/);
  }
});

test("a behaviour rig runs light beside his program, at full graphics without it", () => {
  const behaviour = Object.keys(RN.RIGS).filter((k) => RN.RIGS[k].kind === "behaviour");
  for (const rig of ["verify-runtime", "verify-storeys", "verify-falls", "verify-copresence", "verify-screens", "verify-live-screen"]) {
    assert.ok(behaviour.includes(rig), `${rig} is not a behaviour rig`);
  }
  for (const rig of behaviour) {
    const beside = RN.planBoot(rig, { argv: [], env: {}, apps: HD2 });
    assert.strictEqual(beside.refuse, null);
    assert.strictEqual(beside.lowImpact, RN.RIGS[rig].lowImpact, `${rig} beside his program`);
    assert.strictEqual(beside.own.lowImpact, true, `${rig} would still wait for his program`);
    const alone = RN.planBoot(rig, { argv: [], env: {}, apps: [] });
    assert.strictEqual(alone.lowImpact, null, `${rig} ran light with nothing of his running`);
    assert.ok(!alone.own.lowImpact);
    const forced = RN.planBoot(rig, { argv: ["--low-impact"], env: {}, apps: [] });
    assert.strictEqual(forced.lowImpact, RN.RIGS[rig].lowImpact, `${rig} ignored --low-impact`);
    const full = RN.planBoot(rig, { argv: ["--full-graphics"], env: {}, apps: HD2 });
    assert.strictEqual(full.lowImpact, null, `${rig} ran light under --full-graphics`);
    assert.ok(!full.own.lowImpact, `${rig} --full-graphics would not wait for his program`);
    assert.ok(RN.planBoot(rig, { argv: ["--low-impact", "--full-graphics"], env: {}, apps: [] }).refuse, "both flags at once must be refused");
  }
});

test("every rig's low impact value is one the engine reads", () => {
  // src/engine/low_impact.rs parse: "1", or a comma list of fps=5..60 and window=WxH / keep. The
  // Rust side reads these same strings out of this file (low_impact.rs tests), so the two cannot
  // drift: here, the shape; there, the engine's own parse of every one.
  for (const [rig, e] of Object.entries(RN.RIGS)) {
    if (e.kind !== "behaviour") {
      assert.ok(!e.lowImpact, `${rig} is a ${e.kind} rig with a low impact value`);
      continue;
    }
    for (const tok of String(e.lowImpact).split(",")) {
      assert.match(tok.trim(), /^(1|fps=([5-9]|[1-5]\d|60)|window=(keep|\d{3,4}x\d{3,4}))$/, `${rig}: "${tok}" is not a part the engine reads`);
    }
    assert.ok(e.why && e.why.length > 20, `${rig} gives no reason`);
  }
});

test("an unknown rig is an error, not a guess", () => {
  assert.throws(() => RN.needsOf("verify-something-new"), /not in scripts\/lib\/rig-needs\.js RIGS/);
});

test("the manifest record says why the run was light and what it did not cover", () => {
  const p = RN.planBoot("verify-runtime", { argv: [], env: {}, apps: HD2 });
  const rec = RN.manifestRecord(p, { on: true, fps: 15 });
  assert.deepStrictEqual(rec.because, ["helldivers2.exe"]);
  assert.strictEqual(rec.value, "1");
  assert.match(rec.not_covered, /clouds/);
  assert.strictEqual(RN.manifestRecord(RN.planBoot("verify-runtime", { argv: [], env: {}, apps: [] }), null), null);
});
