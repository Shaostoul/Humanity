// Organisations rung 2 on the website: changes delivered as sealed notes are taken and applied the
// same as on the desktop (docs/design/organisations.md, section 3; 2026-11-15).
//
// Run: node --test scripts/tests/org-mail-web.test.js   (in `just rig-tests`)
//
// scripts/tests/fixtures/org-note-v1.json holds the very note texts of one delivery (an invitation,
// the acceptance, the whole log from the inviter, a change), written and signed by the website
// (scripts/gen-org-note-fixture.mjs). This test takes them through web/shared/org-mail.js `judge`
// and the merge, and folds; src/gui/org_mail_tests.rs `the_notes_of_one_delivery_apply_as_recorded`
// takes the same texts through the desktop's rules. One set of bytes, one answer.
//
// Red first, 2026-11-15: with `judge` answering 'merge' for every sender, "a stranger's note is not
// taken" failed; with the `!v.ok` drop taken out of org-log.js `foldAs`, "a note whose signature
// was broken on the way is never applied" failed (the forged rename applied).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const FIXTURE = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'org-note-v1.json'), 'utf8'));
const key = (b64) => Uint8Array.from(Buffer.from(b64, 'base64'));

async function deps() {
  const noble = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
  const org = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'org-log.js')).href);
  const mail = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'org-mail.js')).href);
  const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
  const pqVerify = async (pub, msg, sig) => !!noble.ml_dsa65.verify(sig, msg, pub);
  return { org, mail, noble, blake3, d: { blake3, pqVerify } };
}

test('the marker and the note limit are the desktop app\'s', async () => {
  const { mail } = await deps();
  const rs = fs.readFileSync(path.join(REPO, 'src', 'net', 'dm_pq.rs'), 'utf8');
  assert.equal(rs.match(/pub const CTL_ORG: &str = "([^"]+)";/)[1], mail.CTL_ORG);
  const om = fs.readFileSync(path.join(REPO, 'src', 'gui', 'org_mail.rs'), 'utf8');
  assert.equal(Number(om.match(/pub const NOTE_TEXT_MAX: usize = ([0-9_]+);/)[1].replace(/_/g, '')), mail.NOTE_TEXT_MAX);
  const chat = fs.readFileSync(path.join(REPO, 'web', 'chat', 'crypto.js'), 'utf8');
  assert.equal(chat.match(/const CTL_ORG = '([^']+)';/)[1], mail.CTL_ORG, 'the chat swallows the same marker');
});

/** Take every fixture note (or those `pick` keeps) into fresh logs; the logs and each verdict. */
async function replay(pick = () => true, change = (t) => t) {
  const { org, mail, blake3, d } = await deps();
  // The inviter's own log starts with what it signed itself: the founding and the invitation.
  const logs = { a: mail.parseNote(FIXTURE.notes[0].text).objects.slice(), b: [] };
  const verdicts = [];
  for (const n of FIXTURE.notes.filter(pick)) {
    const parsed = mail.parseNote(change(n.text, n));
    assert.ok(parsed, 'a note reads');
    assert.equal(parsed.org, FIXTURE.org);
    const v = await mail.judge(FIXTURE.org, logs[n.to], parsed.objects, key(FIXTURE.keys[n.to]), key(FIXTURE.keys[n.from]), FIXTURE.now, d);
    verdicts.push(v);
    if (v === 'merge') org.merge(FIXTURE.org, logs[n.to], parsed.objects, blake3);
  }
  return { logs, verdicts, org, mail, d };
}

test('the notes of one delivery are taken and applied as recorded', async () => {
  const { logs, verdicts, org, d } = await replay();
  assert.ok(verdicts.every((v) => v === 'merge'), JSON.stringify(verdicts));
  const { state } = await org.fold(FIXTURE.org, logs.b, FIXTURE.now, d);
  const e = FIXTURE.expect;
  assert.equal(state.name, e.name);
  assert.deepEqual(Object.fromEntries([...state.members.values()].map((m) => [m.did, m.role]).sort()), e.members);
  assert.equal(state.held, e.held);
  assert.deepEqual(state.log.slice(1).filter((l) => !l.refused).map((l) => l.entryId), e.entries);
});

test('a stranger\'s note is not taken', async () => {
  const { mail, noble, d } = await deps();
  const stranger = noble.ml_dsa65.keygen(new Uint8Array(32).fill(69)).publicKey;
  const invitation = mail.parseNote(FIXTURE.notes[0].text).objects;
  // To an organisation B does not hold: only an invitation from its inviter is taken.
  assert.equal(await mail.judge(FIXTURE.org, [], invitation, key(FIXTURE.keys.b), stranger, FIXTURE.now, d), 'drop:not_invited');
  // To one B holds: parked for a log that proves them, never merged.
  assert.equal(await mail.judge(FIXTURE.org, invitation, invitation, key(FIXTURE.keys.b), stranger, FIXTURE.now, d), 'park');
});

test('a note whose signature was broken on the way is never applied', async () => {
  const { logs, org, d } = await replay(() => true, (text, n) => {
    if (n.why !== 'a change') return text;
    const v = JSON.parse(text.slice('[[hum:org:v1]]'.length));
    const sig = Buffer.from(v.objects[0].signature_b64, 'base64');
    sig[30] ^= 0xff;
    v.objects[0].signature_b64 = sig.toString('base64');
    return '[[hum:org:v1]]' + JSON.stringify(v);
  });
  const { state } = await org.fold(FIXTURE.org, logs.b, FIXTURE.now, d);
  assert.equal(state.name, 'Cart crew', 'the forged rename did not apply');
  assert.equal(state.dropped, 1);
});

test('a change goes in whole notes no longer than the limit', async () => {
  const { mail } = await deps();
  const all = FIXTURE.notes.flatMap((n) => mail.parseNote(n.text).objects);
  const notes = mail.noteTexts(FIXTURE.org, all);
  for (const t of notes) {
    const objs = mail.parseNote(t).objects;
    assert.ok(objs.length >= 1);
    assert.ok(t.length <= mail.NOTE_TEXT_MAX || objs.length === 1);
  }
  assert.equal(notes.flatMap((t) => mail.parseNote(t).objects).length, all.length);
});
