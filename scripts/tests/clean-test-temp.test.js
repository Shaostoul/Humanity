// scripts/clean-test-temp.js (BUG-159): sweeps only old `hum_*` entries, files
// and folders, reports what it removed, and knows a build or a test run when it
// sees one. Every case runs in a scratch folder of its own, never the real temp
// folder's contents.
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { sweep, isBuildOrTest } = require('../clean-test-temp.js');

const DAY = 24 * 60 * 60 * 1000;

/** A scratch folder holding old and new `hum_*` entries and things that are not ours. */
function scratch() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'clean-test-temp-check-'));
  const old = new Date(Date.now() - 2 * DAY);
  const write = (rel, body) => {
    fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
    fs.writeFileSync(path.join(dir, rel), body);
  };
  write('hum_old_db_1_2_3.db', 'x'.repeat(1000));
  write('hum_old_db_1_2_3.db-wal', 'x'.repeat(500));
  write('hum_old_folder_4_5_6/inner/relay.db', 'x'.repeat(2000));
  write('hum_new_db_7_8_9.db', 'fresh');
  write('not_ours_old.txt', 'keep me');
  write('humanity-host-node-test/relay.db', 'not a hum_ entry');
  // Age the old ones (a folder's own date, set after its contents were written).
  for (const rel of ['hum_old_db_1_2_3.db', 'hum_old_db_1_2_3.db-wal', 'hum_old_folder_4_5_6', 'not_ours_old.txt', 'humanity-host-node-test']) {
    fs.utimesSync(path.join(dir, rel), old, old);
  }
  return dir;
}

test('only hum_ entries untouched for more than a day go, files and folders alike', (t) => {
  const dir = scratch();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const r = sweep({ dir, days: 1 });
  assert.deepStrictEqual(
    { seen: r.seen, old: r.old, files: r.removedFiles, folders: r.removedFolders, bytes: r.bytes, left: r.left },
    { seen: 4, old: 3, files: 2, folders: 1, bytes: 3500, left: [] }
  );
  const kept = fs.readdirSync(dir).sort();
  assert.deepStrictEqual(kept, ['hum_new_db_7_8_9.db', 'humanity-host-node-test', 'not_ours_old.txt']);
});

test('a dry run reports the same and deletes nothing', (t) => {
  const dir = scratch();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const before = fs.readdirSync(dir).sort();
  const r = sweep({ dir, days: 1, dryRun: true });
  assert.strictEqual(r.removedFiles + r.removedFolders, 3);
  assert.strictEqual(r.bytes, 3500);
  assert.deepStrictEqual(fs.readdirSync(dir).sort(), before);
});

test('a longer age keeps what is younger than it', (t) => {
  const dir = scratch();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const r = sweep({ dir, days: 3 });
  assert.strictEqual(r.old, 0);
  assert.strictEqual(fs.readdirSync(dir).length, 6);
});

test('builds and test runs are recognised by name, other programs are not', () => {
  for (const n of ['cargo.exe', 'rustc.exe', 'humanity_engine-01ca28c5dc016c23.exe', 'federation_two_relays-0123456789abcdef.exe', 'emdash_lint.test.exe', 'cargo', 'humanity_engine-01ca28c5dc016c23']) {
    assert.ok(isBuildOrTest(n), n);
  }
  for (const n of ['HumanityOS.exe', 'node.exe', 'chrome.exe', 'v0.1461.0_HumanityOS.exe', 'svchost.exe', 'bash.exe']) {
    assert.ok(!isBuildOrTest(n), n);
  }
});
