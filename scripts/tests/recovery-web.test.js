// Recovery through trusted people, the website's half (2026-10-21): web/shared/recovery.js against
// the Rust side (src/relay/core/recovery.rs) and the vendored noble bundle the website ships.
// The design: docs/design/recovery-through-trusted-people.md.
//
// Run: node --test scripts/tests/recovery-web.test.js   (in `just rig-tests`)
// With HOS_WRITE_RECOVERY_FIXTURE=1 it writes the website's half of
// scripts/tests/fixtures/recovery-v1.json, which the Rust test
// `recovery_fixture_rust_and_the_website_agree` reads (the Rust half is written by
// `recovery_fixture_write_rust_half` with the same variable).
//
// What it proves:
//  1. The field is blahaj's: its own multiplication and division vectors (MIT, Distrust, copied
//     into the fixture as `blahaj_field`) come out the same here.
//  2. Split and join round trip for every shape; a repeated piece counts once; below the
//     threshold the join is refused and two pieces of a 3-of-5 leave every secret byte possible.
//  3. Pieces Rust split join here; a setup Rust built verifies here, each trusted person opens
//     only their own piece, the notes to self open with the person's own key, and the pieces
//     rebuild the account; an answer Rust sealed opens here; the codes Rust computed are the
//     codes computed here.
//  4. The website's own half still reads back (the fixture is not stale).
//
// Seen red 2026-10-21 with GF_POLY set to 0x11b (the AES polynomial) instead of blahaj's 0x11d:
// "the field is blahaj's: mul" and "pieces Rust split join here" both failed.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const FIXTURE = path.join(__dirname, 'fixtures', 'recovery-v1.json');

let m, R;
async function load() {
  if (!m) {
    m = await import(pathToFileURL(path.join(WEB, 'shared', 'vendor', 'noble-pq.bundle.js')).href);
    R = await import(pathToFileURL(path.join(WEB, 'shared', 'recovery.js')).href);
  }
  return { m, R };
}

function fixture() {
  return JSON.parse(fs.readFileSync(FIXTURE, 'utf8'));
}

function* choose(n, k, start = 0, cur = []) {
  if (cur.length === k) { yield cur.slice(); return; }
  for (let i = start; i < n; i++) { cur.push(i); yield* choose(n, k, i + 1, cur); cur.pop(); }
}

const randomSecret = () => { const s = new Uint8Array(32); globalThis.crypto.getRandomValues(s); return s; };
const same = (a, b) => Buffer.from(a).equals(Buffer.from(b));

test('the field is blahaj\x27s', async () => {
  const { R } = await load();
  const v = fixture().blahaj_field;
  assert.ok(v && v.mul.length === 256 && v.div.length === 256, 'the fixture carries blahaj\x27s field vectors');
  for (let i = 0; i < 256; i++) {
    assert.strictEqual(R.gf.mul(R.gf.LOG[i], R.gf.EXP[i]), v.mul[i], `the field is blahaj\x27s: mul ${i}`);
    assert.strictEqual(R.gf.div(R.gf.LOG[i], R.gf.EXP[i]), v.div[i], `the field is blahaj\x27s: div ${i}`);
  }
});

test('split and join round trip for every shape', async () => {
  const { R } = await load();
  for (let total = 2; total <= 10; total++) {
    for (let needed = 2; needed <= total; needed++) {
      const secret = randomSecret();
      const shares = R.split(secret, needed, total);
      shares.forEach((s, i) => assert.strictEqual(s[0], i + 1));
      let n = 0;
      for (const pick of choose(total, needed)) {
        if (n++ > 30) break;
        assert.ok(same(R.combine(pick.map((i) => shares[i]), needed), secret), `${needed} of ${total}: ${pick}`);
      }
    }
  }
  const s = R.split(randomSecret(), 2, 3);
  const secret = R.combine(s.slice(0, 2), 2);
  assert.ok(same(R.combine([s[0], s[0], s[1]], 2), secret), 'a repeated piece counts once');
  assert.throws(() => R.combine([s[0], s[0]], 2), /different pieces/, 'one piece twice is still one piece');
});

test('fewer than needed reveals nothing usable', async () => {
  const { R } = await load();
  const secret = randomSecret();
  const shares = R.split(secret, 3, 5);
  assert.throws(() => R.combine(shares.slice(0, 2), 3), /needed/, 'two pieces of a 3-of-5 are refused');
  for (const byte of [0, 7, 31]) {
    const values = new Set();
    for (let y = 0; y < 256; y++) {
      const third = shares[2].slice();
      third[1 + byte] = y;
      values.add(R.combine([shares[0], shares[1], third], 3)[byte]);
    }
    assert.strictEqual(values.size, 256, `byte ${byte}: two pieces leave every value possible`);
  }
});

test('the code reads back the way people say it', async () => {
  const { R } = await load();
  assert.strictEqual(R.base32(new Uint8Array([0x08, 0x86, 0x42, 0x98, 0xe8])), '12345678');
  assert.strictEqual(R.normalizeCode('oooo-iiii-llll-0000'), '0000111111110000');
  assert.strictEqual(R.normalizeCode('UUUU-0000-0000-0000'), null);
  assert.strictEqual(R.normalizeCode('0000 0000 0000'), null);
  assert.strictEqual(R.showCode('0123456789ABCDEF'), '0123-4567-89AB-CDEF');
});

test('Rust and the website agree (the website\x27s side)', async () => {
  const { m, R } = await load();
  const fx = fixture();
  const rust = fx.rust;
  assert.ok(rust, 'the fixture carries Rust\x27s half');

  // Pieces Rust split join here.
  const secret = R.unhex(rust.split.secret_hex);
  const shares = rust.split.shares_hex.map(R.unhex);
  for (const pick of choose(shares.length, rust.split.needed)) {
    assert.ok(same(R.combine(pick.map((i) => shares[i]), rust.split.needed), secret), `pieces Rust split join here: ${pick}`);
  }

  // A setup Rust built verifies and opens here.
  const hs = R.unhex(rust.setup.holder_secret_hex);
  const holderDid = R.accountOf(m, hs);
  const holderKyber = R.kyberOf(m, hs);
  const ts = rust.setup.trusted_secrets_hex.map(R.unhex);
  const back = [];
  for (let slot = 0; slot < rust.setup.objects.length; slot++) {
    const p = await R.servedPiece(m, rust.setup.objects[slot]);
    assert.strictEqual(p.holderDid, holderDid);
    assert.strictEqual(p.payload.slot, slot);
    for (let other = 0; other < ts.length; other++) {
      const opened = await R.openPiece(m, R.kyberOf(m, ts[other]).secretKey, p);
      assert.strictEqual(!!opened, other === slot, `slot ${slot} opens only for its own trusted person, not ${other}`);
      if (opened) back.push(opened.share);
    }
    const note = await R.openNote(m, holderKyber.secretKey, p);
    assert.strictEqual(note && note.did, R.accountOf(m, ts[slot]), 'the note to self names who keeps the piece');
  }
  assert.ok(same(R.rebuild(m, back.slice(0, rust.setup.needed), rust.setup.needed, holderDid), hs), 'Rust\x27s setup rebuilds the account here');
  const bent = JSON.parse(JSON.stringify(rust.setup.objects[0]));
  bent.created_at += 1;
  await assert.rejects(() => R.servedPiece(m, bent), /does not verify/, 'an altered object does not verify');

  // An answer Rust sealed opens here.
  const a = rust.answer;
  const oneTime = m.ml_kem768.keygen(R.unhex(a.one_time_seed_hex));
  assert.strictEqual(R.askCode(m, a.holder_did, oneTime.publicKey), a.code, 'Rust\x27s code for its ask is this side\x27s');
  const share = await R.openAnswer(m, oneTime.secretKey, a.code, a.holder_did, a.slot, R.unhex(a.sealed_hex));
  assert.ok(share && share[0] === a.slot + 1, 'Rust\x27s answer opens here');
  assert.strictEqual(await R.openAnswer(m, oneTime.secretKey, a.code, a.holder_did, a.slot + 1, R.unhex(a.sealed_hex)), null, 'bound to its slot');

  for (const c of rust.codes) {
    const k = m.ml_kem768.keygen(R.unhex(c.kyber_seed_hex));
    assert.strictEqual(R.askCode(m, c.holder_did, k.publicKey), c.code, 'the codes Rust computed are this side\x27s');
  }
});

test('the website\x27s half of the fixture (written with HOS_WRITE_RECOVERY_FIXTURE=1)', async () => {
  const { m, R } = await load();
  const fx = fixture();
  if (process.env.HOS_WRITE_RECOVERY_FIXTURE === '1') {
    const secret = randomSecret();
    const shares = R.split(secret, 3, 5);
    const hs = new Uint8Array(32).fill(0x24);
    const ts = [0x48, 0x49, 0x4a].map((b) => new Uint8Array(32).fill(b));
    const people = ts.map((t, i) => ({ did: R.accountOf(m, t), name: `Web friend ${i}`, kyberPublic: R.kyberOf(m, t).publicKey }));
    const { submissions } = await R.buildSetup(m, { secret: hs, people, needed: 2, createdAt: 1761000000000 });
    // Trusted person 2 answers an ask.
    const p2 = await R.servedPiece(m, submissions[2]);
    const opened = await R.openPiece(m, R.kyberOf(m, ts[2]).secretKey, p2);
    const oneTimeSeed = new Uint8Array(64).fill(0x88);
    const oneTime = m.ml_kem768.keygen(oneTimeSeed);
    const holderDid = R.accountOf(m, hs);
    const code = R.askCode(m, holderDid, oneTime.publicKey);
    const sealed = await R.sealAnswer(m, oneTime.publicKey, code, holderDid, 2, opened.share);
    fx.web = {
      split: { secret_hex: R.hex(secret), needed: 3, total: 5, shares_hex: shares.map(R.hex) },
      setup: { holder_secret_hex: R.hex(hs), trusted_secrets_hex: ts.map(R.hex), needed: 2, objects: submissions },
      answer: { one_time_seed_hex: R.hex(oneTimeSeed), holder_did: holderDid, code, slot: 2, sealed_hex: R.hex(sealed) },
      codes: [0xa0, 0xa1, 0xa2].map((b) => {
        const seed = new Uint8Array(64).fill(b);
        const did = R.accountOf(m, new Uint8Array(32).fill(b - 0x40));
        return { holder_did: did, kyber_seed_hex: R.hex(seed), code: R.askCode(m, did, m.ml_kem768.keygen(seed).publicKey) };
      }),
    };
    fs.writeFileSync(FIXTURE, JSON.stringify(fx, null, 2) + '\n');
  }
  const web = fixture().web;
  assert.ok(web, 'the fixture carries the website\x27s half');
  const hs = R.unhex(web.setup.holder_secret_hex);
  const ts = web.setup.trusted_secrets_hex.map(R.unhex);
  const back = [];
  for (let slot = 0; slot < web.setup.objects.length; slot++) {
    const p = await R.servedPiece(m, web.setup.objects[slot]);
    back.push((await R.openPiece(m, R.kyberOf(m, ts[slot]).secretKey, p)).share);
  }
  assert.ok(same(R.rebuild(m, back, web.setup.needed, R.accountOf(m, hs)), hs), 'the website\x27s own setup still rebuilds');
  assert.ok(same(R.combine(web.split.shares_hex.map(R.unhex).slice(1, 4), 3), R.unhex(web.split.secret_hex)));
});

// The page itself: its words are for people, in the desktop app's words, and every route it
// calls is one the relay has.
test('the page speaks plainly and calls only routes the relay has', () => {
  const html = fs.readFileSync(path.join(WEB, 'pages', 'recovery.html'), 'utf8');
  const app = fs.readFileSync(path.join(WEB, 'pages', 'recovery-app.js'), 'utf8');
  const text = html.replace(/<style[\s\S]*?<\/style>/g, '').replace(/<script[\s\S]*?<\/script>/g, '').replace(/<!--[\s\S]*?-->/g, '').replace(/<[^>]+>/g, ' ');
  const shown = text + '\n' + (app.match(/'[^'\n]*'/g) || []).join('\n');
  for (const word of ['Shamir', 'Kyber', 'DID', 'guardian', 'threshold', 'ciphertext', 'seed phrase', 'Not finished']) {
    assert.ok(!shown.includes(word), `the page says ${word}`);
  }
  for (const part of ['Your trusted people', 'Help someone get back in', 'Lost your device? Ask your trusted people for help']) {
    assert.ok(text.includes(part), `the page has its part ${part}`);
  }
  const router = fs.readFileSync(path.join(REPO, 'src', 'relay', 'mod.rs'), 'utf8');
  const routes = (app.match(/API \+ '\/[a-z-]+/g) || []).map((s) => '/api/v2/recovery' + s.slice(7));
  assert.ok(routes.length >= 6, 'the page calls the recovery routes');
  for (const r of routes) assert.ok(router.includes('.route("' + r), `the relay has ${r}`);
});
