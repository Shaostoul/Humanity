#!/usr/bin/env node
// The boot ledger's report (2026-11-14): how many game boots developers' scripts started on this
// machine, per worktree and per rig, and which were repeated, so the orchestrator can see wasted
// boots (each one shows on the operator's one screen's GPU, even hidden).
//
//   node scripts/boot-ledger.js                 the last 24 hours
//   node scripts/boot-ledger.js --hours 72      a longer window
//   node scripts/boot-ledger.js --json          the summary as JSON
//   node scripts/boot-ledger.js --list          every boot in the window, one line each
//
// The ledger itself: %LOCALAPPDATA%\HumanityOS\dev\boots.jsonl (lib/boot-ledger.js says who writes it).

const BL = require("./lib/boot-ledger.js");

const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : dflt;
};
const hours = Number(opt("--hours", "24")) || 24;
const file = BL.ledgerPath();
const since = Date.now() - hours * 3600 * 1000;
const boots = BL.readBoots(file, since);
const s = BL.summarize(boots);

if (args.includes("--json")) {
  console.log(JSON.stringify({ file, hours, ...s }, null, 2));
  process.exit(0);
}

const mins = (sec) => `${(sec / 60).toFixed(1)} min`;
const pad = (x, n) => String(x).padEnd(n);
console.log(`Boot ledger: ${file}`);
console.log(`Last ${hours} h: ${s.boots} boot(s), ${mins(s.seconds)} of game time${s.boots ? "" : " (nothing booted, or nothing recorded yet)"}`);
if (!s.boots) process.exit(0);

const table = (title, groups) => {
  console.log("");
  console.log(title);
  const w = Math.min(48, Math.max(...groups.map((g) => String(g.key).length), 8));
  for (const g of groups) {
    const ends = Object.entries(g.endings)
      .map(([k, v]) => `${v}x ${k}`)
      .join(", ");
    const vis = g.visible ? `, ${g.visible} VISIBLE` : "";
    const unknown = g.unknown_end ? `, ${g.unknown_end} with no end seen` : "";
    console.log(`  ${pad(String(g.key).slice(0, 48), w)}  ${pad(g.boots + " boot" + (g.boots === 1 ? "" : "s"), 9)} ${pad(mins(g.seconds), 10)} ${g.hidden} hidden${vis}${unknown}  [${ends}]`);
  }
};
table("Per worktree:", s.worktrees);
table("Per rig (or script):", s.rigs);
if (s.repeats.length) table("Repeated (the same worktree, rig and purpose more than once: worth asking whether each was needed):", s.repeats);
else console.log("\nNo repeats: no worktree booted the same rig for the same purpose twice.");

if (args.includes("--list")) {
  console.log("\nEvery boot:");
  for (const r of boots) {
    console.log(`  ${r.t}  ${pad(r.worktree || "?", 28)} ${pad(r.rig || r.script || r.writer || "?", 18)} ${pad(typeof r.duration_s === "number" ? r.duration_s + " s" : "?", 9)} ${r.hidden === false ? "VISIBLE " : ""}${r.purpose || ""}  (${r.ended || "end not seen"})`);
  }
}
