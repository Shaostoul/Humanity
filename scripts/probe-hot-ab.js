// Same-boot A/B driver through the megashader hot reload.
//
// Precondition: `node scripts/probe-sweep.js --only <v> --keep-open ...` has
// booted the rig exe, entered the world and left it running. Afterwards kill
// that instance by the pid in .probe-rig/probe_pid.txt (taskkill /T /F).
//
//   node scripts/probe-hot-ab.js <plan.json> <outdir>
//
// Why: two boots differ (cloud advection, streaming, the first-capture state),
// so a small shader effect needs both arms in ONE boot. Put a repeat of the
// first arm LAST: its distance from the first is the same-boot floor. And
// make one arm a positive control (tint at the LAST write) before trusting a
// null result. Written 2026-09-27 (the ocean stripe and reg.tint A/Bs).
// The checkout may be CRLF (core.autocrlf); patches are written with "\n"
// and matched against the file's own line endings.
//
// plan = { pins: {showcase pins applied to every vantage},
//          vantages: ["id from tests/visual/vantages.json" | {id, camera, showcase, settle_s, hold_altitude}],
//          arms: [{name, patches: [{file, find, replace, count?}], showcase?: {pins for this arm only}}],
//          shot?: {width, height}, same_park?: true }
// An arm's `showcase` is merged LAST, so it wins over the plan and vantage
// pins: it is how an arm flips a runtime switch instead of a shader (the
// HDR scene target's `present_direct`, 2026-09-27). An arm with no patches
// and a showcase needs no reload. `shot` asks every capture for a hi-res
// off-screen render instead of the window grab.
//
// `same_park: true` parks each vantage ONCE and takes every arm there in
// turn (vantage-major), instead of re-parking per arm. Measured 2026-09-27:
// two re-parked captures of the SAME arm in one boot differed in 18 to 33
// percent of pixels (a re-park never lands exactly where the last one did),
// which drowns a bit-exact claim; consecutive captures of one park differ
// only where the scene itself moves. In this mode an arm's showcase is sent
// on its own, so every arm must set its switch explicitly (it persists).
// Every arm starts from the ORIGINAL file text captured at start, applies its
// patches (each `find` must occur exactly `count` (default 1) times, or the
// arm is refused: a patch that silently did not apply is the classic null
// result), writes, waits for a NEW "[HotReload] megashader reassembled" line,
// then runs the rig's vantage protocol and captures <arm>__<id>.png.
// Originals are restored at the end (and on any error).
const fs = require("fs");
const path = require("path");

const REPO = path.resolve(__dirname, "..");
const RIG = path.join(REPO, ".probe-rig");
const DEBUG = path.join(RIG, "debug");
const LOG = path.join(RIG, "logs", "run.log");
const plan = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const OUT = path.resolve(process.argv[3]);
fs.mkdirSync(OUT, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (m) => console.log(`[ab] ${m}`);

const spec = JSON.parse(fs.readFileSync(path.join(REPO, "tests", "visual", "vantages.json"), "utf8"));
const vantages = plan.vantages.map((v) => {
  if (typeof v !== "string") return v;
  const f = spec.vantages.find((x) => x.id === v);
  if (!f) throw new Error(`no vantage ${v}`);
  return f;
});

function req(name, body) {
  fs.writeFileSync(path.join(DEBUG, name), JSON.stringify(body));
}
function clearDone(n) {
  const p = path.join(DEBUG, n);
  if (fs.existsSync(p)) fs.unlinkSync(p);
}
async function waitFile(name, ms) {
  const p = path.join(DEBUG, name);
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (fs.existsSync(p)) {
      try { return JSON.parse(fs.readFileSync(p, "utf8")); } catch { /* half-written */ }
    }
    await sleep(300);
  }
  return null;
}
const logText = () => (fs.existsSync(LOG) ? fs.readFileSync(LOG, "utf8") : "");
const count = (s, re) => (s.match(re) || []).length;

const files = new Set();
for (const a of plan.arms) for (const p of a.patches || []) files.add(p.file);
const originals = {};
for (const f of files) originals[f] = fs.readFileSync(path.join(REPO, f), "utf8");

function armText(arm) {
  const out = Object.assign({}, originals);
  for (const p of arm.patches || []) {
    const want = p.count ?? 1;
    // The checkout may be CRLF (core.autocrlf): match the file's own endings.
    const crlf = out[p.file].includes("\r\n");
    const find = crlf ? p.find.replace(/\r?\n/g, "\r\n") : p.find;
    const repl = crlf ? p.replace.replace(/\r?\n/g, "\r\n") : p.replace;
    const n = out[p.file].split(find).length - 1;
    if (n !== want) throw new Error(`arm ${arm.name}: patch find occurs ${n}x (want ${want}) in ${p.file}: ${p.find.slice(0, 80)}`);
    out[p.file] = out[p.file].split(find).join(repl);
  }
  return out;
}

async function applyAndReload(texts, label) {
  let changed = false;
  for (const f of files) {
    const cur = fs.readFileSync(path.join(REPO, f), "utf8");
    if (cur !== texts[f]) changed = true;
  }
  if (!changed) { log(`${label}: shader unchanged, no reload needed`); return; }
  const before = logText();
  const ok0 = count(before, /\[HotReload\] megashader reassembled/g);
  const bad0 = count(before, /\[HotReload\].*(REJECTED|failed|missing)/g);
  for (const f of files) fs.writeFileSync(path.join(REPO, f), texts[f]);
  const t0 = Date.now();
  while (Date.now() - t0 < 120000) {
    await sleep(500);
    const t = logText();
    if (count(t, /\[HotReload\].*(REJECTED|failed|missing)/g) > bad0) {
      const line = t.split("\n").filter((l) => /\[HotReload\].*(REJECTED|failed|missing)/.test(l)).pop();
      throw new Error(`${label}: hot reload REJECTED: ${line}`);
    }
    if (count(t, /\[HotReload\] megashader reassembled/g) > ok0) {
      const line = t.split("\n").filter((l) => /megashader reassembled/.test(l)).pop();
      log(`${label}: ${line.trim().slice(-110)}`);
      await sleep(1500);
      return;
    }
  }
  throw new Error(`${label}: no [HotReload] reassembled line within 120 s`);
}

async function capture(v, arm, armShowcase) {
  await park(v, armShowcase);
  await shoot(v, arm);
}

async function park(v, armShowcase) {
  const sc = Object.assign(
    { map_diag: "0", cloud_top_bound: "0", cloud_uniform_step: "0", cloud_step_m: "0", wind: "auto", anim_clock: "auto", aurora: "1" },
    plan.pins || {},
    v.showcase || {},
    armShowcase || {}
  );
  req("showcase_request.json", sc);
  await sleep(3500);
  clearDone("camera_done.json");
  req("camera_request.json", v.camera);
  const cam = await waitFile("camera_done.json", 60000);
  if (!cam || cam.ok !== true) throw new Error(`camera: ${JSON.stringify(cam)}`);
  req("showcase_request.json", { time_scale: "0" });
  await sleep((v.settle_s ?? 8) * 1000);
  clearDone("camera_done.json");
  req("camera_request.json", v.camera);
  const re = await waitFile("camera_done.json", 60000);
  if (!re || re.ok !== true) throw new Error(`re-park: ${JSON.stringify(re)}`);
  await sleep(v.hold_altitude ? 900 : 6000);
}

async function shoot(v, arm) {
  clearDone("screenshot_done.json");
  req("screenshot_request.json", plan.shot || {});
  const shot = await waitFile("screenshot_done.json", 60000);
  if (!shot || shot.ok !== true) throw new Error(`screenshot: ${JSON.stringify(shot)}`);
  const dest = `${arm}__${v.id}.png`;
  fs.copyFileSync(path.join(RIG, shot.path), path.join(OUT, dest));
  const fc = path.join(DEBUG, "frame_costs.json");
  if (fs.existsSync(fc)) fs.copyFileSync(fc, path.join(OUT, `${arm}__${v.id}-costs.json`));
  log(`  ${dest}  (${shot.fps ?? "?"} fps)`);
}

(async () => {
  const results = [];
  try {
    const texts = plan.arms.map((a) => armText(a)); // refuse bad patches BEFORE touching anything
    // SAME-PARK mode: park each vantage ONCE and take every arm there, arm
    // after arm, with the arm's shader reload or showcase switch in between.
    if (plan.same_park) {
      for (const v of vantages) {
        log(`vantage ${v.id} (same park)`);
        try {
          await park(v, {});
        } catch (e) {
          log(`  FAILED park ${v.id}: ${e.message}`);
          for (const arm of plan.arms) results.push({ arm: arm.name, id: v.id, ok: false, error: e.message });
          continue;
        }
        for (let i = 0; i < plan.arms.length; i++) {
          const arm = plan.arms[i];
          try {
            await applyAndReload(texts[i], arm.name);
            if (arm.showcase) req("showcase_request.json", arm.showcase);
            await sleep(2500); // the switch lands; the frame-cost EMA settles
            await shoot(v, arm.name);
            results.push({ arm: arm.name, id: v.id, ok: true });
          } catch (e) {
            log(`  FAILED ${arm.name} ${v.id}: ${e.message}`);
            results.push({ arm: arm.name, id: v.id, ok: false, error: e.message });
          }
        }
      }
      return;
    }
    for (let i = 0; i < plan.arms.length; i++) {
      const arm = plan.arms[i];
      log(`arm ${arm.name}`);
      await applyAndReload(texts[i], arm.name);
      for (const v of vantages) {
        try {
          await capture(v, arm.name, arm.showcase);
          results.push({ arm: arm.name, id: v.id, ok: true });
        } catch (e) {
          log(`  FAILED ${v.id}: ${e.message}`);
          results.push({ arm: arm.name, id: v.id, ok: false, error: e.message });
        }
      }
    }
  } catch (e) {
    log(`ABORT: ${e.message}`);
    results.push({ abort: e.message });
  } finally {
    try { await applyAndReload(originals, "restore"); } catch (e) {
      for (const f of files) fs.writeFileSync(path.join(REPO, f), originals[f]);
      log(`restore reload: ${e.message} (files restored on disk)`);
    }
    const panics = count(logText(), /PANIC/g);
    fs.writeFileSync(path.join(OUT, "ab.json"), JSON.stringify({ results, panics }, null, 1));
    log(`done: panics=${panics}`);
  }
})();
