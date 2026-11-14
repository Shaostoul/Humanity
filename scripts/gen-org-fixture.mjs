// Writes scripts/tests/fixtures/org-log-v1.json: an organisation's log built and signed by the
// WEBSITE's code (web/shared/org-log.js over pq-object.js, real ML-DSA-65 from the vendored noble
// bundle), with what its fold must say written by hand beside each step. Both apps then fold the
// frozen file: scripts/tests/org-log-web.test.js (the website) and
// org::tests::the_frozen_log_folds_as_recorded (the desktop, which so checks the website's
// signatures and encoding). docs/design/organisations.md, section 8.
//
// Run once: node scripts/gen-org-fixture.mjs. It refuses to write unless the website's own fold
// agrees with every hand-written expectation. Never regenerate a committed fixture to make a test
// pass: the file is the contract between the two apps (and the shape a saved log has, so the
// desktop's "a saved log still opens" test reads it too).

import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { writeFileSync, readFileSync } from 'node:fs';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const noble = await import(pathToFileURL(join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
const org = await import(pathToFileURL(join(REPO, 'web', 'shared', 'org-log.js')).href);

const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
const pqVerify = async (pub, msg, sig) => !!noble.ml_dsa65.verify(sig, msg, pub);
const person = (n) => {
  const kp = noble.ml_dsa65.keygen(new Uint8Array(32).fill(n));
  return { pub: kp.publicKey, sign: async (m) => noble.ml_dsa65.sign(m, kp.secretKey) };
};
const [owner, admin, mo, shy, x, r] = [1, 2, 3, 4, 5, 6].map(person);

const T0 = 1_800_000_000_000;
let clock = T0;
const book = org.roleBookFrom(JSON.parse(readFileSync(join(REPO, 'data', 'orgs', 'roles.json'), 'utf8')));
const founding = await org.buildFounding(
  { name: 'Cart crew', about: 'Saturday cans, Kitsap', directory: 'members', roles: book.roles },
  { authorPublicKey: owner.pub, sign: owner.sign, blake3, createdAt: clock },
);
const orgId = founding.objectId;
const subs = [founding.submission];
const steps = [];
const ids = {};
async function step(name, who, act, refused = null) {
  clock += 1000;
  const { objectId, submission } = await org.buildEntry(orgId, act, { authorPublicKey: who.pub, sign: who.sign, blake3, createdAt: clock });
  subs.push(submission);
  ids[name] = objectId;
  steps.push({ name, entry: objectId, refused });
  return objectId;
}
const until = () => clock + 7 * 86_400_000;

await step('invite admin', owner, { act: 'invite', who: admin.pub, role: 'admin', until: until() });
await step('admin accepts', admin, { act: 'accept', invite: ids['invite admin'] });
await step('invite mo', admin, { act: 'invite', who: mo.pub, role: 'member', until: until() });
await step('mo accepts', mo, { act: 'accept', invite: ids['invite mo'] });
await step('invite shy', admin, { act: 'invite', who: shy.pub, role: 'member', until: until() });
await step('shy accepts', shy, { act: 'accept', invite: ids['invite shy'] });
await step('shy keeps to organisers', shy, { act: 'listing', name: 'Shy', level: 'organisers' });
await step('mo names herself', mo, { act: 'listing', name: 'Mo', level: 'members' });
await step('invite x', admin, { act: 'invite', who: x.pub, role: 'member', until: until() });
await step('mo accepts for x', mo, { act: 'accept', invite: ids['invite x'] }, 'not_the_invitee');
await step('owner defines driver', owner, { act: 'role_set', role: { id: 'driver', name: 'Driver', rank: 20, perms: ['calendar.edit'] } });
await step('admin makes mo a driver', admin, { act: 'assign', who: mo.pub, role: 'driver' });
await step('mo tries to remove shy', mo, { act: 'remove', who: shy.pub }, 'lacks_permission');
await step('admin tries to define', admin, { act: 'role_set', role: { id: 'boss', name: 'Boss', rank: 5, perms: [] } }, 'lacks_permission');
await step('invite r', admin, { act: 'invite', who: r.pub, role: 'member', until: until() });
await step('r accepts', r, { act: 'accept', invite: ids['invite r'] });
await step('admin removes r', admin, { act: 'remove', who: r.pub });
await step('admin tries to remove owner', admin, { act: 'remove', who: owner.pub }, 'owner_stays');
await step('owner renames', owner, { act: 'settings', name: 'Cart crew of Kitsap', about: null, directory: null });
const now = clock + 1000;

const did = (k) => org.didFor(k, blake3);
const expect = {
  name: 'Cart crew of Kitsap',
  directory: 'members',
  roles: ['owner', 'admin', 'member', 'driver'],
  members: Object.fromEntries([
    [did(owner.pub), { role: 'owner', name: null, listing: 'members' }],
    [did(admin.pub), { role: 'admin', name: null, listing: 'members' }],
    [did(mo.pub), { role: 'driver', name: 'Mo', listing: 'members' }],
    [did(shy.pub), { role: 'member', name: 'Shy', listing: 'organisers' }],
  ].sort()),
  open_invites: [ids['invite x']],
  // The directory as Mo (an ordinary member) reads it, and as the admin does.
  directory_as_member: { named: 3, private: 1 },
  directory_as_admin: { named: 4, private: 0 },
  // A stranger (x, who never accepted) reads neither.
  stranger_refused: 'not_a_member',
  // Every line of the log, the founding first: the entry id and why it was refused (null = applied).
  log: [{ entry: orgId, refused: null }, ...steps.map((s) => ({ entry: s.entry, refused: s.refused }))],
};

const { state, error } = await org.fold(orgId, subs, now, { blake3, pqVerify });
if (error) throw new Error(error);
const got = {
  name: state.name,
  directory: state.directory,
  roles: state.roles.map((x) => x.id),
  members: Object.fromEntries([...state.members.values()].map((m) => [m.did, { role: m.role, name: m.name, listing: m.listing }]).sort()),
  open_invites: org.openInvites(state, now).map((i) => i.id),
  directory_as_member: (({ named, private: p }) => ({ named: named.length, private: p }))(org.directoryFor(state, mo.pub)),
  directory_as_admin: (({ named, private: p }) => ({ named: named.length, private: p }))(org.directoryFor(state, admin.pub)),
  stranger_refused: org.directoryFor(state, x.pub).refused,
  log: state.log.map((l) => ({ entry: l.entryId, refused: l.refused })),
};
if (JSON.stringify(got) !== JSON.stringify(expect)) {
  console.error('the website fold disagrees with the hand-written expectation:');
  console.error(JSON.stringify({ got, expect }, null, 1));
  process.exit(1);
}
const out = {
  _purpose: 'Frozen organisation log (docs/design/organisations.md, section 8), built and signed by the website (scripts/gen-org-fixture.mjs). Both apps fold it to `expect`; it is also the shape a saved log has. Never regenerate it to make a test pass.',
  hum_org: 1,
  org: orgId,
  now,
  readers: { member: did(mo.pub), admin: did(admin.pub), stranger: did(x.pub) },
  reader_keys: { member: Buffer.from(mo.pub).toString('base64'), admin: Buffer.from(admin.pub).toString('base64'), stranger: Buffer.from(x.pub).toString('base64') },
  expect,
  objects: subs,
};
const path = join(REPO, 'scripts', 'tests', 'fixtures', 'org-log-v1.json');
writeFileSync(path, JSON.stringify(out, null, 1) + '\n');
console.log(`wrote ${path}: ${subs.length} objects, org ${orgId}`);
