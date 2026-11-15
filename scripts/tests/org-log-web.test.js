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
// Rung 2, 2026-11-15 (the log rebuilt with `prev`, a handover, a voided change and a held entry):
// with the voiding pass returning after the first fold, "the frozen log folds as recorded" failed
// (the old owner's unseen rename applied); with holding switched off, it failed on `held`.

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
  assert.equal(state.held, e.held, 'the entry following one this log does not hold waits');
  assert.equal(org.auditLogFor(state, key(FIXTURE.reader_keys.stranger)).refused, 'not_a_member', 'a stranger reads no audit log');
  // Every applied change in the log is signed by its actor (the entry's own author key).
  const byId = new Map(FIXTURE.objects.map((s) => [org.objectIdOf(s, blake3), s]));
  for (const l of state.log.slice(1)) {
    assert.deepEqual(Buffer.from(l.actor).toString('base64'), byId.get(l.entryId).author_public_key_b64);
  }
});

// The entry ids of the frozen log's objects, in the order they were made.
async function ids() {
  const { org, blake3 } = await deps();
  return FIXTURE.objects.map((s) => org.objectIdOf(s, blake3));
}

// Rung 2: a forged acceptance of the handover (its signature broken) is dropped, so the owner role
// does not move, and the old owner's change it would have voided stands. Red first, 2026-11-15:
// with the `!v.ok` drop taken out of `foldAs`, the forged acceptance was applied (the owner moved).
test('a forged entry is dropped: the handover does not happen', async () => {
  const { org, blake3, pqVerify } = await deps();
  const objs = FIXTURE.objects.map((s) => ({ ...s }));
  const all = await ids();
  const i = all.indexOf(FIXTURE.expect.log[FIXTURE.expect.log.length - 1].entry); // "admin takes it"
  const sig = Buffer.from(objs[i].signature_b64, 'base64');
  sig[20] ^= 0xff;
  objs[i].signature_b64 = sig.toString('base64');
  const { state } = await org.fold(FIXTURE.org, objs, FIXTURE.now, { blake3, pqVerify });
  assert.equal(state.dropped, 1);
  assert.equal(state.log.length, FIXTURE.expect.log.length - 1);
  assert.deepEqual(Buffer.from(org.ownerOf(state)).toString('base64'), FIXTURE.objects[0].author_public_key_b64, 'the founder is still the owner');
  assert.ok(org.openTransfer(state, FIXTURE.now), 'the handover is still only offered');
});

// Rung 2: an entry delivered before an entry it follows is held (never applied, never refused) and
// is applied once that entry arrives. Red first, 2026-11-15: with the held loop disabled
// (`more = hold` set to false), "nothing that follows it is refused" failed (Mo's later changes
// were refused as not_a_member instead of waiting).
test('an entry that arrives before what it follows waits, then applies', async () => {
  const { org, blake3, pqVerify } = await deps();
  const all = await ids();
  const moAccepts = FIXTURE.expect.log[4].entry; // "mo accepts"
  const without = FIXTURE.objects.filter((_, n) => all[n] !== moAccepts);
  const early = (await org.fold(FIXTURE.org, without, FIXTURE.now, { blake3, pqVerify })).state;
  assert.ok(early.held > FIXTURE.expect.held, 'what follows the missing acceptance waits');
  assert.ok(early.log.every((l) => l.refused !== 'not_a_member'), 'nothing that follows it is refused');
  const later = (await org.fold(FIXTURE.org, [...without, FIXTURE.objects[all.indexOf(moAccepts)]], FIXTURE.now, { blake3, pqVerify })).state;
  assert.equal(later.held, FIXTURE.expect.held);
  assert.deepEqual(later.log.map((l) => ({ entry: l.entryId, refused: l.refused })), FIXTURE.expect.log);
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
  // A member's fold holds what follows entries an invitation leaves out; the invitee's fold places
  // them by what is there, so the invitation shows.
  assert.ok(!(await org.fold(parsed.org, parsed.objects, FIXTURE.now, { blake3, pqVerify })).state.invites.has(inv));
  const theirs = (await org.foldInvitation(parsed.org, parsed.objects, FIXTURE.now, { blake3, pqVerify })).state;
  assert.equal(theirs.members.size, 2, 'the invitee sees the owner and the inviter only');
  assert.ok(theirs.invites.has(inv));
});

test('opening the same text twice adds nothing', async () => {
  const { org, blake3 } = await deps();
  const log = [];
  assert.equal(org.merge(FIXTURE.org, log, FIXTURE.objects, blake3), FIXTURE.objects.length);
  assert.equal(org.merge(FIXTURE.org, log, FIXTURE.objects.slice().reverse(), blake3), 0);
});
