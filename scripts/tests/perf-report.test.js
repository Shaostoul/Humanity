// perf-report.js must REFUSE to grade a contaminated capture.
//
// Run:  node --test scripts/tests/perf-report.test.js
//
// Marking a capture in the manifest is only half a guard: the half that
// matters is the grader downstream declining to turn that number into a
// verdict. On 2026-09-18 a contaminated reading (75.79 ms where every clean
// boot read 66.8 to 67.1, a second renderer mid-sweep) went straight into a
// comparison table, and a second one the same day (6.6 fps at the limb against
// a 15 ms GPU sum, a concurrent cargo/rustc build) read as a real collapse. So
// the contract tested here is: a contaminated vantage is printed as
// CONTAMINATED, is not compared to a floor or a baseline, and the process
// exits 2 - for either cause.
//
// The report is exercised as a real child process against a temp manifest -
// no mocking of its internals - because its EXIT CODE is what `just perf-sweep`
// and any CI gate read.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPORT = path.resolve(__dirname, "..", "perf-report.js");

// A manifest shaped exactly like probe-sweep's, minus the fields the report
// does not read. `graphics_source` keeps the provenance banner quiet.
function manifest(vantages, extra = {}) {
  return Object.assign(
    {
      stamp: "20260918-000000",
      captured: vantages.filter((v) => v.ok).length,
      total: vantages.length,
      panics: 0,
      contaminated: vantages.filter((v) => v.contaminated).length,
      graphics_source: "operator-mirrored",
      graphics: {
        ssao_strength: 1,
        veg_density: 1,
        fog_density: 1,
        render_distance: 2000,
        tree_model_distance: 400,
        godray_intensity: 1,
      },
      vantages,
    },
    extra
  );
}

function runReport(m, args = []) {
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "perf-report-test-")), "manifest.json");
  fs.writeFileSync(file, JSON.stringify(m, null, 2));
  const r = spawnSync(process.execPath, [REPORT, file, ...args], { encoding: "utf8" });
  return { status: r.status, out: (r.stdout || "") + (r.stderr || ""), file };
}

// A vantage that is comfortably ABOVE its floor: if the report still fails,
// the only possible reason is the contamination mark.
const fast = (extra = {}) =>
  Object.assign(
    { id: "fuji-forest-ground", ok: true, fps: 40, frame_ms: 25, perf_floor_fps: 9 },
    extra
  );

test("GREEN: a clean, above-floor sweep grades and exits 0", () => {
  const r = runReport(manifest([fast()]));
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /All captured vantages at\/above floor/);
  assert.doesNotMatch(r.out, /CONTAMINATED/);
});

test("RED: a second renderer marks it CONTAMINATED and exits 2", () => {
  const r = runReport(
    manifest([
      fast({
        contaminated: true,
        contaminated_by: [
          { pid: 7777, name: "HumanityOS.exe", exe: "C:\\Humanity\\target\\release\\HumanityOS.exe" },
        ],
      }),
    ])
  );
  assert.strictEqual(r.status, 2, `expected exit 2, got ${r.status}:\n${r.out}`);
  assert.match(r.out, /CONTAMINATED \(HumanityOS\.exe 7777\)/);
  // The status column must NOT say "ok" for that row: a reader skimming the
  // table is the person this protects.
  assert.doesNotMatch(r.out, /fuji-forest-ground.*\bok\b/);
  // And it must say what to do about it.
  assert.match(r.out, /tasklist/);
});

test("RED: a concurrent build marks it CONTAMINATED and exits 2", () => {
  // The CPU shape, which no earlier guard looked for at all: the fps is far
  // BELOW its floor, so without the mark this would be graded as a genuine
  // regression and somebody would go hunting for it in the renderer.
  const r = runReport(
    manifest([
      fast({
        id: "limb-400km",
        fps: 6.6,
        frame_ms: 151,
        perf_floor_fps: 25,
        contaminated: true,
        contaminated_by: [
          { pid: 5150, name: "cargo.exe", exe: "" },
          { pid: 5151, name: "rustc.exe", exe: "" },
        ],
      }),
    ])
  );
  assert.strictEqual(r.status, 2, `expected exit 2, got ${r.status}:\n${r.out}`);
  assert.match(r.out, /CONTAMINATED \(cargo\.exe 5150, rustc\.exe 5151\)/);
  // It must NOT be reported as below-floor: that would send the reader after a
  // renderer regression that does not exist.
  assert.doesNotMatch(r.out, /BELOW FLOOR/);
  assert.match(r.out, /rustc\.exe/);
});

test("RED: contamination beats a baseline comparison too", () => {
  // The dangerous case is a contaminated capture that LOOKS like a win against
  // a baseline. The delta column must not be computed at all.
  const baseDir = fs.mkdtempSync(path.join(os.tmpdir(), "perf-report-base-"));
  const basePath = path.join(baseDir, "baseline.json");
  fs.writeFileSync(basePath, JSON.stringify(manifest([fast({ fps: 12 })])));
  const r = runReport(
    manifest([
      fast({ fps: 40, contaminated: true, contaminated_by: [{ pid: 7777, name: "HumanityOS.exe", exe: "" }] }),
    ]),
    ["--baseline", basePath]
  );
  assert.strictEqual(r.status, 2, r.out);
  assert.match(r.out, /CONTAMINATED/);
  assert.doesNotMatch(r.out, /\+28\.0/, "a contaminated capture must not be reported as a +28 fps win");
});

test("a manifest predating the guard says so instead of implying a clean machine", () => {
  const m = manifest([fast()]);
  delete m.contaminated; // what every sweep before 2026-09-18 looks like
  const r = runReport(m);
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /UNRECORDED/);
});

test("a pre-boot wait is surfaced in the report header", () => {
  const r = runReport(
    manifest([fast()], {
      machine_guard: { own_pid: 4242, pre_boot_wait_s: 150, pre_boot_free: true, pre_boot_blockers: [] },
    })
  );
  assert.strictEqual(r.status, 0, r.out);
  assert.match(r.out, /waited 150 s/);
});
