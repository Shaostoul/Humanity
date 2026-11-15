// Builds scripts/tests/fixtures/org-note-v1.json: the organisation notes (rung 2,
// docs/design/organisations.md) of one delivery between two people, written and signed by the
// WEBSITE's code (web/shared/org-log.js, web/shared/org-mail.js, real ML-DSA-65 from the vendored
// noble bundle). Both apps take these very note texts through their arrival rules
// (scripts/tests/org-mail-web.test.js and src/gui/org_mail_tests.rs) and must reach the state
// written by hand below: the same bytes, the same state.
//
// Run: node scripts/gen-org-note-fixture.mjs   (only on purpose: never to make a test pass)
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { writeFileSync, readFileSync } from 'node:fs';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const noble = await import(pathToFileURL(join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
const org = await import(pathToFileURL(join(REPO, 'web', 'shared', 'org-log.js')).href);
const mail = await import(pathToFileURL(join(REPO, 'web', 'shared', 'org-mail.js')).href);

const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
const pqVerify = async (pub, msg, sig) => !!noble.ml_dsa65.verify(sig, msg, pub);
const deps = { blake3, pqVerify };
const person = (n) => {
  const kp = noble.ml_dsa65.keygen(new Uint8Array(32).fill(n));
  return { pub: kp.publicKey, sign: async (m) => noble.ml_dsa65.sign(m, kp.secretKey) };
};
const [a, b] = [61, 62].map(person);
const T0 = 1_800_000_000_000;
let clock = T0;
const book = org.roleBookFrom(JSON.parse(readFileSync(join(REPO, 'data', 'orgs', 'roles.json'), 'utf8')));

// Each person's log, as their browser keeps it.
const logs = { a: [], b: [] };
const keys = { a, b };
const notes = [];
const idOf = (s) => org.objectIdOf(s, blake3);
async function heads(who, invitation = false) {
  const r = invitation ? await org.foldInvitation(orgId, logs[who], clock, deps) : await org.fold(orgId, logs[who], clock, deps);
  return r.state.heads;
}
// `from` sends the objects `subs` to `to`: the notes, and the receiver's app taking them.
async function send(from, to, subs, why) {
  for (const text of mail.noteTexts(orgId, subs)) {
    const n = mail.parseNote(text);
    const verdict = await mail.judge(orgId, logs[to], n.objects, keys[to].pub, keys[from].pub, clock, deps);
    if (verdict !== 'merge') throw new Error(`${why}: ${to} would not take it (${verdict})`);
    org.merge(orgId, logs[to], n.objects, blake3);
    notes.push({ from, to, why, text });
  }
}

const founding = await org.buildFounding({ name: 'Cart crew', about: 'Saturday cans', directory: 'members', roles: book.roles }, { authorPublicKey: a.pub, sign: a.sign, blake3, createdAt: clock });
const orgId = founding.objectId;
logs.a.push(founding.submission);
clock += 1000;
const invite = await org.buildEntry(orgId, { act: 'invite', who: b.pub, role: 'member', until: clock + 86_400_000 }, { authorPublicKey: a.pub, sign: a.sign, blake3, createdAt: clock, prev: await heads('a') });
logs.a.push(invite.submission);
await send('a', 'b', [founding.submission, invite.submission], 'the invitation');
clock += 1000;
const accept = await org.buildEntry(orgId, { act: 'accept', invite: invite.objectId }, { authorPublicKey: b.pub, sign: b.sign, blake3, createdAt: clock, prev: await heads('b', true) });
logs.b.push(accept.submission);
await send('b', 'a', [accept.submission], 'the acceptance');
clock += 1000;
await send('a', 'b', logs.a.slice(), 'the whole log, from the inviter');
const rename = await org.buildEntry(orgId, { act: 'settings', name: 'Cart crew of Kitsap', about: null, directory: null }, { authorPublicKey: a.pub, sign: a.sign, blake3, createdAt: clock, prev: await heads('a') });
logs.a.push(rename.submission);
await send('a', 'b', [rename.submission], 'a change');

const now = clock + 1000;
const did = (k) => org.didFor(k, blake3);
const expect = {
  name: 'Cart crew of Kitsap',
  members: Object.fromEntries([[did(a.pub), 'owner'], [did(b.pub), 'member']].sort()),
  held: 0,
  entries: [invite.objectId, accept.objectId, rename.objectId],
};
for (const who of ['a', 'b']) {
  const { state } = await org.fold(orgId, logs[who], now, deps);
  const got = { name: state.name, members: Object.fromEntries([...state.members.values()].map((m) => [m.did, m.role]).sort()), held: state.held, entries: state.log.slice(1).filter((l) => !l.refused).map((l) => l.entryId) };
  if (JSON.stringify(got) !== JSON.stringify(expect)) throw new Error(`${who}'s fold disagrees: ${JSON.stringify(got)}`);
}
const out = {
  _purpose: 'The organisation notes of one delivery (docs/design/organisations.md, rung 2), built and signed by the website (scripts/gen-org-note-fixture.mjs). Both apps take these texts through their arrival rules and must reach `expect`. Never regenerate it to make a test pass.',
  org: orgId,
  now,
  keys: { a: Buffer.from(a.pub).toString('base64'), b: Buffer.from(b.pub).toString('base64') },
  expect,
  notes,
};
const path = join(REPO, 'scripts', 'tests', 'fixtures', 'org-note-v1.json');
writeFileSync(path, JSON.stringify(out, null, 1) + '\n');
console.log(`wrote ${path}: ${notes.length} notes`);
