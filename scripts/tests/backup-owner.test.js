// The relay's database backups belong to the relay's own account, private, and the deploy's rollback
// copies of the program live in a folder of their own (2026-11-24, the operator: "Try accomplishing
// that"). Before, the backup job ran as root, so its copies were root's (chmod 640) and the relay's
// owner-only check warned about 16 files it could not make private, and the deploy dropped 44 MB
// program copies into the same folder without limit. Run: node --test scripts/tests/backup-owner.test.js
//
// Seen red 2026-11-24 against the files before the change (BACKUP_OWNER_ROOT pointed at a copy of
// the three files as committed before it): "the backup job runs as the relay's account" failed (no User=humanity).
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = process.env.BACKUP_OWNER_ROOT || path.join(__dirname, '..', '..');
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8');

test('the backup job runs as the relay\'s account and writes private files', () => {
  const unit = read('scripts/systemd/humanity-backup-db.service');
  assert.match(unit, /^User=humanity$/m, 'the backup job runs as the relay\'s account');
  assert.match(unit, /^Group=humanity$/m);
  assert.match(unit, /^UMask=0077$/m, 'every file it writes is private');
  const script = read('scripts/humanity-backup-db.sh');
  assert.match(script, /chmod 600 "\$OUT"/, 'each backup is readable by the relay\'s account alone');
  assert.match(script, /chmod 700 "\$BACKUP_DIR"/, 'the folder opens for the relay\'s account alone');
  assert.ok(!/chmod 640/.test(script), 'no group-readable backups');
});

test('the deploy keeps the program it replaces in rollback/, the newest five, never in backups/', () => {
  const deploy = read('.github/workflows/deploy.yml');
  assert.ok(!/cp "\$BIN" backups\//.test(deploy), 'no program copies in the backups folder');
  assert.match(deploy, /cp "\$BIN" rollback\/HumanityOS-/, 'the copy goes to rollback/');
  assert.match(deploy, /tail -n \+6/, 'only the newest five copies are kept');
});
