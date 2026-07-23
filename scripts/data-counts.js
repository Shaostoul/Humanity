#!/usr/bin/env node
// Live game-data counts, so the docs' inventory figures can be re-synced in one
// command instead of drifting silently (they were ~2x off before 2026-07-23).
// Counts the canonical CSV rows and the data-file inventory under data/.
//
//   node scripts/data-counts.js       (or `just data-counts`)

const fs = require("fs");
const path = require("path");

const REPO = path.resolve(__dirname, "..");
const DATA = path.join(REPO, "data");

// CSV row count minus the header line (ignores a trailing blank line).
function csvRows(rel) {
  const p = path.join(DATA, rel);
  if (!fs.existsSync(p)) return null;
  const lines = fs.readFileSync(p, "utf8").split(/\r?\n/).filter((l) => l.trim().length);
  return Math.max(0, lines.length - 1);
}

function walk(dir, exts, acc) {
  for (const name of fs.readdirSync(dir)) {
    const p = path.join(dir, name);
    const st = fs.statSync(p);
    if (st.isDirectory()) walk(p, exts, acc);
    else {
      const e = path.extname(name).slice(1).toLowerCase();
      if (exts.includes(e)) acc[e] = (acc[e] || 0) + 1;
    }
  }
  return acc;
}

const csv = {
  items: csvRows("items.csv"),
  recipes: csvRows("recipes.csv"),
  plants: csvRows("plants.csv"),
  creatures: csvRows("creatures.csv"),
};
const files = walk(DATA, ["csv", "toml", "ron", "json"], {});
const totalFiles = Object.values(files).reduce((a, b) => a + b, 0);

console.log("\nGame data counts (data/)\n" + "-".repeat(40));
for (const [k, v] of Object.entries(csv)) {
  console.log(`  ${k.padEnd(12)} ${v == null ? "(missing)" : v}`);
}
console.log("-".repeat(40));
console.log(`  data files   ${totalFiles}   (` + Object.entries(files).map(([e, n]) => `${e}:${n}`).join(" ") + ")");
console.log(
  "\nUpdate the '~' figures in docs/STATUS.md + docs/FEATURES.md when these move materially.\n"
);
