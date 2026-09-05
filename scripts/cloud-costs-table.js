// Tabulate the per-vantage GPU pass costs a HUMANITY_FRAME_COSTS=1 sweep left
// beside its captures (<id>-costs.json, copied by probe-sweep.js from the
// rig's debug/frame_costs.json after each capture).
//   node scripts/cloud-costs-table.js <sweep-dir> [key ...]
// Default keys: gpu.cloud_screen gpu.celestial cpu.patch_build. Prints the
// timing mode too: "timestamps" is a real GPU measurement, "cpu_fallback" is
// not (treat those rows as frame-time subtractions, 2x slop).
const fs = require("fs"), path = require("path");
const dir = process.argv[2];
const keys = process.argv.length > 3 ? process.argv.slice(3) : ["gpu.cloud_screen", "gpu.celestial", "cpu.patch_build"];
const files = fs.readdirSync(dir).filter(f => f.endsWith("-costs.json")).sort();
if (!files.length) { console.log("no *-costs.json in " + dir); process.exit(1); }
const get = (o, k) => { const g = k.split(".")[0]; const m = o[g + "_ms"] || {}; return m[k]; };
console.log("id".padEnd(26), "frame_ms".padStart(9), keys.map(k => k.padStart(18)).join(""), "  timing");
for (const f of files) {
  const j = JSON.parse(fs.readFileSync(path.join(dir, f), "utf8"));
  const id = f.replace(/-costs\.json$/, "");
  const vals = keys.map(k => { const v = get(j, k); return (v === undefined ? "-" : Number(v).toFixed(2)).padStart(18); });
  console.log(id.padEnd(26), Number(j.frame_ms).toFixed(1).padStart(9), vals.join(""), "  " + j.gpu_timing);
}
// The full key list of the first file, so a reader can ask for more.
const j0 = JSON.parse(fs.readFileSync(path.join(dir, files[0]), "utf8"));
console.log("\nkeys: gpu[" + Object.keys(j0.gpu_ms || {}).join(", ") + "]  cpu[" + Object.keys(j0.cpu_ms || {}).join(", ") + "]");
