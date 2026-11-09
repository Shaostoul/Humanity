// BUG-285 (2026-11-07): the relay's files readable by its own account alone. On united-humanity.us
// the live relay.db was 0644 in a 0755 folder (every account on the box could read every member's
// data) because nothing set a umask. The relay now tightens what is there at each start
// (src/relay/owner_only.rs, tested in Rust), and every new file is made private by `UMask=0077`:
// in the unit provision-vps.sh writes, in the unit SELF-HOSTING.md gives, and in a drop-in the
// deploy and the follower install on every server (scripts/server-files.txt), so the live servers
// get it without anyone editing their hand-made unit.
//
// Seen red on 2026-11-07 against the tree before the fix: no drop-in, no list line, and neither
// unit had a UMask line ("the drop-in exists", "provision-vps.sh's relay unit sets UMask=0077" and
// "SELF-HOSTING.md's relay unit sets UMask=0077" all failed), and run_relay called nothing.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const read = (p) => fs.readFileSync(path.join(ROOT, p), "utf8");
const DROPIN = "scripts/systemd-dropins/humanity-relay-owner-only.conf";
const INSTALLED = "/etc/systemd/system/humanity-relay.service.d/owner-only.conf";
const { parseManifest } = require("../lib/server-files.js");

/** The [Service] section of a unit's text (to the next section or the end). */
function serviceSection(unit) {
  const m = unit.replace(/\r\n/g, "\n").match(/^\[Service\]\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m);
  return m ? m[1] : "";
}

test("the drop-in exists and sets UMask=0077 for the relay's service", () => {
  assert.ok(fs.existsSync(path.join(ROOT, DROPIN)), "the drop-in exists");
  const text = read(DROPIN);
  assert.match(serviceSection(text), /^UMask=0077$/m, "under [Service]");
  assert.ok(read(".gitattributes").includes("scripts/systemd-dropins/*.conf text eol=lf"), "kept LF for systemd");
});

test("every server installs it as a drop-in of humanity-relay.service", () => {
  const files = parseManifest(read("scripts/server-files.txt")).files;
  const line = files.find((f) => f.src === DROPIN);
  assert.ok(line, "scripts/server-files.txt lists the drop-in");
  assert.strictEqual(line.dest, INSTALLED);
  assert.strictEqual(line.role, "all", "both the main server and the follow node");
  assert.strictEqual(line.mode, "644");
});

test("provision-vps.sh's relay unit sets UMask=0077", () => {
  const sh = read("scripts/provision-vps.sh");
  const unit = sh.match(/cat > \/etc\/systemd\/system\/humanity-relay\.service <<UNIT\n([\s\S]*?)\nUNIT\n/);
  assert.ok(unit, "the relay unit's heredoc is found");
  assert.match(serviceSection(unit[1]), /^UMask=0077$/m);
});

test("SELF-HOSTING.md's relay unit sets UMask=0077", () => {
  const md = read("docs/admin/SELF-HOSTING.md").replace(/\r\n/g, "\n");
  const unit = md.match(/# \/etc\/systemd\/system\/humanity-relay\.service\n([\s\S]*?)\n```/);
  assert.ok(unit, "the relay unit's block is found");
  assert.match(serviceSection(unit[1]), /^UMask=0077$/m);
});

test("run_relay tightens the private files right after the database opens", () => {
  const src = read("src/relay/mod.rs").replace(/\r\n/g, "\n");
  const start = src.indexOf("pub async fn run_relay()");
  const open = src.indexOf("Storage::open_resilient(", start);
  const tighten = src.indexOf("owner_only::tighten(", start);
  assert.ok(start > 0 && open > start, "run_relay opens the database");
  assert.ok(tighten > open, "and then calls owner_only::tighten");
});
