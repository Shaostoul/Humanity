#!/usr/bin/env node
// Guard: web/chat/bip39-english.js MUST be the same 2048-word list the desktop app reads and
// writes recovery phrases with (the `bip39` crate), or a phrase written on one app won't
// restore on the other. Checked by fingerprint: the SHA-256 of the 2048 words joined by
// newlines must equal WORDLIST_SHA256 in src/net/identity.rs, which a Rust test computes from
// the crate's own list. Also runs the website's BIP39 algorithm over the official test vectors
// (the same strings src/net/identity.rs pins) and checks that one wrong word is refused, as both
// apps refuse it (2026-10-16). Run in CI (.github/workflows/verify.yml).
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const repo = path.resolve(__dirname, '..');

function webList() {
  const src = fs.readFileSync(path.join(repo, 'web/chat/bip39-english.js'), 'utf8');
  const m = src.match(/\(([\s\S]*?)\)\.split\(" "\)/);
  if (!m) throw new Error('could not parse web BIP39_ENGLISH');
  const joined = (m[1].match(/"([a-z ]*)"/g) || []).map(s => s.replace(/"/g, '')).join('');
  return joined.split(/\s+/).filter(Boolean);
}

/** The desktop app's pin: WORDLIST_SHA256 in src/net/identity.rs. */
function desktopPin() {
  const src = fs.readFileSync(path.join(repo, 'src/net/identity.rs'), 'utf8');
  const m = src.match(/WORDLIST_SHA256: &str = "([0-9a-f]{64})"/);
  if (!m) throw new Error('WORDLIST_SHA256 not found in src/net/identity.rs');
  return m[1];
}

const web = webList();
let fail = false;
const check = (ok, msg) => { if (!ok) { console.error('FAIL:', msg); fail = true; } else { console.log('ok:', msg); } };
check(web.length === 2048, `web list length ${web.length} === 2048`);
check(web[0] === 'abandon' && web[2047] === 'zoo', 'anchors abandon..zoo');
check(new Set(web).size === 2048, 'no word twice');
const fingerprint = crypto.createHash('sha256').update(web.join('\n')).digest('hex');
check(fingerprint === desktopPin(), `web list SHA-256 ${fingerprint} === the desktop's WORDLIST_SHA256`);

// ── The website's BIP39 algorithm (web/chat/crypto.js mnemonicFromSeed / seedFromMnemonic) ──
function encode(entropy) { // Buffer(32) -> 24 words
  const hash = crypto.createHash('sha256').update(entropy).digest();
  let bits = '';
  for (const b of entropy) bits += b.toString(2).padStart(8, '0');
  bits += hash[0].toString(2).padStart(8, '0'); // 8-bit checksum for 256-bit entropy
  const out = [];
  for (let i = 0; i < bits.length; i += 11) out.push(web[parseInt(bits.slice(i, i + 11), 2)]);
  return out.join(' ');
}
function decode(phrase) { // 24 words -> Buffer(32), validates checksum
  const idx = phrase.split(/\s+/).map(w => {
    const i = web.indexOf(w);
    if (i < 0) throw new Error(`unknown word: ${w}`);
    return i;
  });
  const bits = idx.map(i => i.toString(2).padStart(11, '0')).join('');
  const ent = bits.slice(0, 256), csum = bits.slice(256);
  const bytes = Buffer.alloc(32);
  for (let i = 0; i < 32; i++) bytes[i] = parseInt(ent.slice(i * 8, i * 8 + 8), 2);
  const hash = crypto.createHash('sha256').update(bytes).digest();
  if (hash[0].toString(2).padStart(8, '0') !== csum) throw new Error('checksum mismatch');
  return bytes;
}

// Official BIP39 vectors (trezor python-mnemonic) and the project's own [7;32] KAT: the same
// strings src/net/identity.rs pins for the bip39 crate.
const VECTORS = [
  [0x00, 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art'],
  [0x7f, 'legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title'],
  [0x80, 'letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic bless'],
  [0xff, 'zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote'],
  [0x07, 'alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright truly'],
];
for (const [byte, phrase] of VECTORS) {
  const seed = Buffer.alloc(32, byte);
  check(encode(seed) === phrase, `written for ${byte.toString(16)}: ${phrase.split(' ').slice(-1)[0]}`);
  check(Buffer.compare(decode(phrase), seed) === 0, `read back for ${byte.toString(16)}`);
}

// One wrong word (another real word) is refused, not read as a different key.
const words = VECTORS[4][1].split(' ');
words[0] = 'abandon';
let refused = false;
try { decode(words.join(' ')); } catch (e) { refused = /checksum/.test(e.message); }
check(refused, 'one wrong word is refused by its checksum');

process.exit(fail ? 1 : 0);
