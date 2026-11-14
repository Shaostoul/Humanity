// Organisations on the website fold the same as on the desktop (rung 1, 2026-11-14,
// docs/design/organisations.md, section 8).
//
// Run: node --test scripts/tests/org-log-web.test.js   (in `just rig-tests`)
//
// The frozen log scripts/tests/fixtures/org-log-v1.json was built and signed by the website's
// code (scripts/gen-org-fixture.mjs, real ML-DSA-65 from the vendored noble bundle) with what its
// fold must say written by hand. This test folds it with web/shared/org-log.js and the REAL
// signature check; org::tests::the_frozen_log_folds_as_recorded folds the same file on the
// desktop. Together they hold the two apps to one answer.
//
// Red first, 2026-11-14: with the `not_the_invitee` check taken out of org-log.js `apply`, "the
// frozen log folds as recorded" failed (Mo's acceptance of x's invitation was refused for the
// wrong reason, `already_member`, where the record says `not_the_invitee`); with the `!v.ok` drop
// taken out of `fold`, "a broken signature is dropped" failed (`0 == 1`: the forged entry counted).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const FIXTURE = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'org-log-v1.json'), 'utf8'));

async function deps() {
  const noble = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
  const org = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'org-log.js')).href);
  const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
  const pqVerify = async (pub, msg, sig) => !!noble.ml_dsa65.verify(sig, msg, pub);
  return { org, blake3, pqVerify };
}

const key = (b64) => Uint8Array.from(Buffer.from(b64, 'base64'));

test('the frozen log folds as recorded', async () => {
  const { org, blake3, pqVerify } = await deps();
  const { state, error } = await org.fold(FIXTURE.org, FIXTURE.objects, FIXTURE.now, { blake3, pqVerify });
  assert.equal(error, undefined);
  const e = FIXTURE.expect;
  assert.equal(state.name, e.name);
  assert.equal(state.directory, e.directory);
  assert.deepEqual(state.roles.map((r) => r.id), e.roles);
  const members = Object.fromEntries([...state.members.values()].map((m) => [m.did, { role: m.role, name: m.name, listing: m.listing }]).sort());
  assert.deepEqual(members, e.members);
  assert.deepEqual(org.openInvites(state, FIXTURE.now).map((i) => i.id), e.open_invites);
  assert.deepEqual(state.log.map((l) => ({ entry: l.entryId, refused: l.refused })), e.log);
  const asMember = org.directoryFor(state, key(FIXTURE.reader_keys.member));
  assert.deepEqual({ named: asMember.named.length, private: asMember.private }, e.directory_as_member);
  const asAdmin = org.directoryFor(state, key(FIXTURE.reader_keys.admin));
  assert.deepEqual({ named: asAdmin.named.length, private: asAdmin.private }, e.directory_as_admin);
  assert.equal(org.directoryFor(state, key(FIXTURE.reader_keys.stranger)).refused, e.stranger_refused);
  assert.equal(org.auditLogFor(state, key(FIXTURE.reader_keys.stranger)).refused, 'not_a_member', 'a stranger reads no audit log');
  // Every applied change in the log is signed by its actor (the entry's own author key).
  const byId = new Map(FIXTURE.objects.map((s) => [org.objectIdOf(s, blake3), s]));
  for (const l of state.log.slice(1)) {
    assert.deepEqual(Buffer.from(l.actor).toString('base64'), byId.get(l.entryId).author_public_key_b64);
  }
});

test('a broken signature is dropped and changes nothing', async () => {
  const { org, blake3, pqVerify } = await deps();
  const objs = FIXTURE.objects.map((s) => ({ ...s }));
  // Break the signature of "admin removes r" (the second to last applied removal).
  const i = objs.length - 3;
  const sig = Buffer.from(objs[i].signature_b64, 'base64');
  sig[20] ^= 0xff;
  objs[i].signature_b64 = sig.toString('base64');
  const { state } = await org.fold(FIXTURE.org, objs, FIXTURE.now, { blake3, pqVerify });
  assert.equal(state.dropped, 1);
  assert.equal(state.log.length, FIXTURE.expect.log.length - 1);
  assert.equal(state.members.size, Object.keys(FIXTURE.expect.members).length + 1, 'the removal did not happen');
});

test('the invitation carries only its proof chain', async () => {
  const { org, blake3, pqVerify } = await deps();
  const { state } = await org.fold(FIXTURE.org, FIXTURE.objects, FIXTURE.now, { blake3, pqVerify });
  const [inv] = FIXTURE.expect.open_invites;
  const text = org.inviteBundle(state, FIXTURE.objects, inv, blake3);
  const parsed = org.parseBundle('Join us: ' + text);
  assert.equal(parsed.org, FIXTURE.org);
  // The founding, the admin's invite and acceptance, and x's invite: nobody else's entries.
  assert.equal(parsed.objects.length, 4);
  const theirs = (await org.fold(parsed.org, parsed.objects, FIXTURE.now, { blake3, pqVerify })).state;
  assert.equal(theirs.members.size, 2, 'the invitee sees the owner and the inviter only');
  assert.ok(theirs.invites.has(inv));
});

test('opening the same text twice adds nothing', async () => {
  const { org, blake3 } = await deps();
  const log = [];
  assert.equal(org.merge(FIXTURE.org, log, FIXTURE.objects, blake3), FIXTURE.objects.length);
  assert.equal(org.merge(FIXTURE.org, log, FIXTURE.objects.slice().reverse(), blake3), 0);
});
