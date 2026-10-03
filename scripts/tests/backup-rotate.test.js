// The VPS backup script's rotation (scripts/humanity-backup-db.sh), run for
// real under the script's own shell options (BUG-122, 2026-10-02).
//
// The old rotation was `ls -1t relay-*.db | tail | xargs rm`. Under
// `set -euo pipefail` an ls whose glob matches nothing exits 2 and ends the
// script, so once the last plain copy was deleted every run stopped before
// the .db.aes line and the sealed copies piled up without limit. This test
// pulls the CURRENT rotate_keep() out of the script and runs it in a scratch
// folder holding only sealed copies, and runs the old line there too, so the
// harness is shown to catch the failure it exists for.
//
// Run: node --test scripts/tests/backup-rotate.test.js (in `just rig-tests`).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const SCRIPT = path.join(__dirname, "..", "humanity-backup-db.sh");

// Git for Windows' bash, not the WSL launcher in System32 that a bare
// "bash" can resolve to from PowerShell.
function bashPath() {
  if (process.platform !== "win32") return "bash";
  for (const p of ["C:/Program Files/Git/bin/bash.exe", "C:/Program Files/Git/usr/bin/bash.exe"]) {
    if (fs.existsSync(p)) return p;
  }
  return "bash";
}

// The KEEP line and the rotate_keep() function, exactly as the script has them.
function rotationSource() {
  const src = fs.readFileSync(SCRIPT, "utf8").replace(/\r\n/g, "\n");
  const keep = src.match(/^KEEP=.*$/m);
  const fn = src.match(/^rotate_keep\(\) \{\n[\s\S]*?\n\}$/m);
  assert.ok(keep && fn, "humanity-backup-db.sh still defines KEEP and rotate_keep()");
  return `${keep[0]}\n${fn[0]}\n`;
}

// A scratch backups folder with `n` sealed copies, one minute apart, oldest first.
function scratch(n) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum-rotate-"));
  const t0 = Math.floor(Date.now() / 1000) - n * 60;
  for (let i = 0; i < n; i++) {
    const f = path.join(dir, `relay-2026100${i < 10 ? "0" : ""}${i}.db.aes`);
    fs.writeFileSync(f, "Salted__");
    fs.utimesSync(f, t0 + i * 60, t0 + i * 60);
  }
  return dir;
}

function run(body, dir) {
  const posix = dir.replace(/\\/g, "/").replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`);
  const script = `set -euo pipefail\nBACKUP_DIR='${posix}'\n${body}\necho done\n`;
  return spawnSync(bashPath(), ["-c", script], { encoding: "utf8" });
}

test("rotation keeps the newest 15 sealed copies when no plain copy exists", () => {
  const dir = scratch(20);
  const r = run(`${rotationSource()}rotate_keep 'relay-*.db'\nrotate_keep 'relay-*.db.aes'`, dir);
  assert.strictEqual(r.status, 0, `the script carried on (stderr: ${r.stderr})`);
  assert.match(r.stdout, /done/);
  const left = fs.readdirSync(dir).sort();
  assert.strictEqual(left.length, 15, `15 kept, got ${left.length}`);
  assert.ok(left.includes("relay-202610019.db.aes"), "the newest is kept");
  assert.ok(!left.includes("relay-2026100" + "00.db.aes"), "the oldest went");
  fs.rmSync(dir, { recursive: true, force: true });
});

test("a smaller KEEP from the environment is honoured", () => {
  const dir = scratch(6);
  const r = run(`HUMANITY_DB_BACKUP_ROTATE_KEEP=2\n${rotationSource()}rotate_keep 'relay-*.db.aes'`, dir);
  assert.strictEqual(r.status, 0, r.stderr);
  assert.strictEqual(fs.readdirSync(dir).length, 2);
  fs.rmSync(dir, { recursive: true, force: true });
});

test("control: the old ls line ends the script in the same folder", () => {
  const dir = scratch(3);
  const r = run(`ls -1t "$BACKUP_DIR"/relay-*.db 2>/dev/null | tail -n +16 | xargs -r rm -f`, dir);
  assert.notStrictEqual(r.status, 0, "the harness must see the old failure, or it proves nothing");
  assert.doesNotMatch(r.stdout, /done/);
  fs.rmSync(dir, { recursive: true, force: true });
});
