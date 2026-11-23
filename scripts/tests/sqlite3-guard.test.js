// Tests for scripts/vps/sqlite3-guard, the servers' /usr/local/bin/sqlite3 (2026-11-22): the live
// relay database is always opened read-only, so a hand-typed check can never delete its log files
// under the running relay (docs/ai/storage.md). Run: node --test scripts/tests/sqlite3-guard.test.js
//
// Seen red 2026-11-22 with the guard's live-path rewrite taken out (every argument passed through):
// "the live path is opened read-only" got "/opt/Humanity/data/relay.db SELECT 1".
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const path = require('node:path');

const GUARD = path.join(__dirname, '..', 'vps', 'sqlite3-guard');
const LIVE = '/opt/Humanity/data/relay.db';

/** Run the guard with `echo` standing in for the real tool: returns what the tool would be given. */
function run(args, cwd) {
  const out = execFileSync('bash', [GUARD, ...args], {
    env: { ...process.env, SQLITE3_REAL: 'echo', HUMANITY_LIVE_DB: LIVE },
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  return out.trim();
}

test('the live path is opened read-only', () => {
  assert.strictEqual(run([LIVE, 'SELECT 1']), `file:${LIVE}?mode=ro SELECT 1`);
});

test('a file: URI of the live database without mode=ro gets it', () => {
  assert.strictEqual(run([`file:${LIVE}`, '.tables']), `file:${LIVE}?mode=ro .tables`);
  assert.strictEqual(run([`file:${LIVE}?cache=shared`, '.tables']), `file:${LIVE}?cache=shared&mode=ro .tables`);
});

test('the backup script\'s read-only URI passes through unchanged', () => {
  assert.strictEqual(run([`file:${LIVE}?mode=ro`, ".backup '/tmp/x.db'"]), `file:${LIVE}?mode=ro .backup '/tmp/x.db'`);
});

test('a copy and any other file pass through unchanged, so recovery work can still write', () => {
  assert.strictEqual(run(['/tmp/copy/relay.db', 'PRAGMA wal_checkpoint(TRUNCATE);']), '/tmp/copy/relay.db PRAGMA wal_checkpoint(TRUNCATE);');
  assert.strictEqual(run(['/opt/Humanity/data/recover-1/relay.db', 'SELECT 1']), '/opt/Humanity/data/recover-1/relay.db SELECT 1');
  assert.strictEqual(run(['-readonly', '/tmp/other.db']), '-readonly /tmp/other.db');
});
