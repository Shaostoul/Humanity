// Validate the name `just snapshot <name>` was given, BEFORE cargo runs.
//
// Why this exists: `cargo test --lib snapshot_<name>` exits 0 when the filter
// matches NOTHING, and the recipe used to follow it with an unconditional
// `@echo "Wrote tests/snapshots/<name>.png"`. So a typo, or a page whose test
// was deleted, reported success and rendered nothing - and if a stale PNG of
// that name happened to be sitting in tests/snapshots/, opening it showed a
// picture and the whole thing looked like it had worked.
//
// Exits 1 with the real list when the name matches no test.

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const src = fs.readFileSync(path.join(root, "src/gui/ui_snapshots.rs"), "utf8");
const lines = src.split(/\r?\n/);

const names = new Set();
for (let i = 0; i < lines.length; i++) {
  // Macro-generated: page_snapshot!(snapshot_x, "x", x, w, h);
  const m = lines[i].trim().match(/^page_snapshot!\(snapshot_([a-z0-9_]+)\s*,/);
  if (m) { names.add(m[1]); continue; }
  // Hand-written, but only when it really carries #[test] - cargo's filter
  // will not match a plain helper function.
  const f = lines[i].trim().match(/^fn snapshot_([a-z0-9_]+)\s*\(/);
  if (f && lines.slice(Math.max(0, i - 4), i).join(" ").includes("#[test]")) {
    names.add(f[1]);
  }
}

const wanted = (process.argv[2] || "").trim();
if (!wanted) {
  console.error("usage: node scripts/snapshot-name.js <name>");
  process.exit(1);
}
if (names.has(wanted)) process.exit(0);

const all = [...names].sort();
console.error(`No snapshot test named "snapshot_${wanted}".`);
console.error("");
console.error("cargo exits 0 on a filter that matches nothing, so without this check the");
console.error("recipe would have told you it wrote a PNG it never rendered.");
console.error("");
console.error(`The ${all.length} names that do exist:`);
for (let i = 0; i < all.length; i += 4) {
  console.error("  " + all.slice(i, i + 4).join("  "));
}
process.exit(1);
