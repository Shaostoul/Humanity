#!/usr/bin/env node
// verify-respawn: proves in the RUNNING game that respawners are places (docs/design/
// game-setting-and-intro.md rung 2, 2026-11-21, `just verify-respawn`): a rig dies through the
// showcase `die` verb, respawns through `respawn` (with a place: "home", "general-services-1"),
// and is judged where it stands.
//
// It is a probe sweep (scripts/probe-sweep.js: the freshness gate, the portable rig, the one-GPU
// machine guard, background and silent, never focused) over tests/visual/respawn-vantages.json in
// one boot, its rig .probe-rig/respawn, then scripts/lib/respawn-judge.js over the manifest:
//   - the bedroom's respawner and the death screen's list of where to wake, as pictures a person
//     looks at (2560 wide, the operator's display);
//   - waking at home from the front door stands the player UPSTAIRS in the bedroom at the pod
//     (the floor stood on is the bedroom's 3.0 m, the HUD names the Bedroom);
//   - waking at General Services stands them on the ship's deck inside General Services.
// The expectations come from the tree's data (the bedroom's box, General Services' box, the pod's
// row), never from what the game says it chose.
//
// Usage: node scripts/verify-respawn.js [--dry-verdict <manifest.json>] [probe-sweep args...]
// Exit 0 pass, 1 refused before booting (probe-sweep's), 2 a check failed.

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const RJ = require("./lib/respawn-judge.js");

const REPO = path.resolve(__dirname, "..");
const args = process.argv.slice(2);

function verdict(manifestPath) {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const r = RJ.judge(manifest, RJ.treeData(REPO));
  for (const c of r.checks) console.log(`${c.ok ? "PASS" : "FAIL"} ${c.id}: ${c.detail}`);
  console.log(r.pass ? "verify-respawn: PASS" : "verify-respawn: FAIL");
  fs.writeFileSync(path.join(path.dirname(manifestPath), "respawn-verdict.json"), JSON.stringify(r, null, 2));
  return r.pass ? 0 : 2;
}

const dry = args.indexOf("--dry-verdict");
if (dry >= 0) {
  process.exit(verdict(path.resolve(args[dry + 1])));
}

const stamp = new Date().toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15);
const rig = path.join(REPO, ".probe-rig", "respawn");
const out = path.join(rig, "runs", stamp);
const sweep = spawnSync(
  process.execPath,
  [path.join(__dirname, "probe-sweep.js"), "--rig", rig, "--out", out, "--vantages", path.join(REPO, "tests", "visual", "respawn-vantages.json"), ...args],
  { stdio: "inherit" },
);
if (sweep.status === 1) {
  console.log("verify-respawn: REFUSED before booting (see above); nothing was judged");
  process.exit(1);
}
const manifestPath = path.join(out, "manifest.json");
if (!fs.existsSync(manifestPath)) {
  console.log(`verify-respawn: FAIL, the sweep wrote no manifest at ${manifestPath}`);
  process.exit(2);
}
process.exit(verdict(manifestPath));
