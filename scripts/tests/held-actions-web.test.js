// Hide and keep's signed actions (2026-11-20, docs/design/hide-and-keep.md rung 4): the website's
// mod_action_v1 builder (web/shared/pq-object.js) accepts exactly the actions, target kinds and
// held actions crates/humanity-protocol/src/moderation.rs does, and refuses the same mismatches,
// so an owner's action built on the website is one the relay checks.
// Run: node --test scripts/tests/held-actions-web.test.js
//
// Seen red 2026-11-20 with 'held_destroy' left out of pq-object.js's MOD_ACTIONS: "the website's
// actions are the protocol's" (the lists differed by held_destroy).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const ROOT = join(fileURLToPath(import.meta.url), '..', '..', '..');
const RUST = readFileSync(join(ROOT, 'crates', 'humanity-protocol', 'src', 'moderation.rs'), 'utf8');
const obj = await import(pathToFileURL(join(ROOT, 'web', 'shared', 'pq-object.js')).href);

/** The strings of the Rust `pub const NAME: &[&str] = &[ ... ];`. */
function rustList(name) {
  const m = RUST.match(new RegExp(`pub const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`));
  assert.ok(m, `moderation.rs has ${name}`);
  return [...m[1].matchAll(/"([a-z_]+)"/g)].map((x) => x[1]);
}

test('the website builds the same held actions the protocol defines', () => {
  assert.deepEqual(obj.MOD_ACTIONS, rustList('MOD_ACTIONS'), "the website's actions are the protocol's");
  assert.deepEqual(obj.TARGET_KINDS, rustList('TARGET_KINDS'), "the website's target kinds are the protocol's");
  assert.deepEqual(obj.HELD_ACTIONS, rustList('HELD_ACTIONS'), "the website's held actions are the protocol's");
  for (const a of obj.HELD_ACTIONS) assert.equal(obj.heldTargetKind(a), a === 'hide_and_keep' ? 'report' : 'held');
  assert.equal(obj.heldTargetKind('ban'), null);
});

test('a held action naming the wrong target, or carrying an end it may not, is refused', () => {
  const base = { target: '7', reason: 'the content policy', rule: '', expiresAt: 0, role: '' };
  assert.ok(obj.modActionV1Payload({ ...base, action: 'hide_and_keep', targetKind: 'report' }));
  assert.ok(obj.modActionV1Payload({ ...base, action: 'held_legal_hold', targetKind: 'held', expiresAt: 1_900_000_000_000 }));
  assert.throws(() => obj.modActionV1Payload({ ...base, action: 'hide_and_keep', targetKind: 'held' }), /names a report/);
  assert.throws(() => obj.modActionV1Payload({ ...base, action: 'ban', targetKind: 'report' }), /nothing else names either/);
  assert.throws(() => obj.modActionV1Payload({ ...base, action: 'held_destroy', targetKind: 'held', expiresAt: 5 }), /only held_legal_hold/);
});

// A recipient's acceptance of an owner change (the owner review's finding 5, 2026-11-20): it names
// the change by its object id and carries no end, as moderation.rs's OWNER_ACCEPT rule says.
// Seen red 2026-11-20: before 'owner_change_accept' joined pq-object.js, the first test failed ("the
// website's actions are the protocol's": the protocol had it, the website did not); with the
// target-kind rule taken out of a copy, "an acceptance naming a person is refused" failed.
test("an owner change's acceptance names the change, an object, and carries no end", () => {
  const base = { target: 'ab'.repeat(32), reason: 'checked ' + 'cd'.repeat(32), rule: '', expiresAt: 0, role: '' };
  assert.equal(obj.OWNER_ACCEPT, 'owner_change_accept');
  assert.ok(obj.modActionV1Payload({ ...base, action: 'owner_change_accept', targetKind: 'object' }));
  assert.throws(() => obj.modActionV1Payload({ ...base, action: 'owner_change_accept', targetKind: 'identity' }), /names the change/, 'an acceptance naming a person is refused');
  assert.throws(() => obj.modActionV1Payload({ ...base, action: 'owner_change_accept', targetKind: 'object', expiresAt: 5 }), /only held_legal_hold/);
});
