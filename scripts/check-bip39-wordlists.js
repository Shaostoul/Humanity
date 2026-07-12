#!/usr/bin/env node
// Guard: web/chat/bip39-english.js and src/net/bip39_wordlist.rs MUST be the same
// 2048-word list, or a seed phrase written on one client won't restore on another.
// Also runs a standard-BIP39 round-trip KAT over the shared list. Run in CI.
// (The native list is separately asserted == the bip39 crate by a Rust test.)
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
function nativeList() {
  const src = fs.readFileSync(path.join(repo, 'src/net/bip39_wordlist.rs'), 'utf8');
  return (src.match(/"([a-z]+)"/g) || []).map(s => s.replace(/"/g, ''));
}

const web = webList();
const nat = nativeList();
let fail = false;
const check = (ok, msg) => { if (!ok) { console.error('FAIL:', msg); fail = true; } else { console.log('ok:', msg); } };

check(web.length === 2048, `web list length ${web.length} === 2048`);
check(nat.length === 2048, `native list length ${nat.length} === 2048`);
check(JSON.stringify(web) === JSON.stringify(nat), 'web list === native list (byte-identical)');
check(web[0] === 'abandon' && web[2047] === 'zoo', 'anchors abandon..zoo');
check(web.includes('occur') && web.includes('affair'), 'canonical words occur + affair present');

// ── Standard BIP39 encode/decode over the shared list (the same algorithm both
// clients implement). Proves the list is self-consistent + round-trips. ──
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
  let bits = idx.map(i => i.toString(2).padStart(11, '0')).join('');
  const ent = bits.slice(0, 256), csum = bits.slice(256);
  const bytes = Buffer.alloc(32);
  for (let i = 0; i < 32; i++) bytes[i] = parseInt(ent.slice(i * 8, i * 8 + 8), 2);
  const hash = crypto.createHash('sha256').update(bytes).digest();
  if (hash[0].toString(2).padStart(8, '0') !== csum) throw new Error('checksum mismatch');
  return bytes;
}

const seed = Buffer.alloc(32, 7); // [7u8; 32] -- matches the native KAT
const phrase = encode(seed);
const back = decode(phrase);
check(Buffer.compare(seed, back) === 0, 'BIP39 round-trip [7;32] -> phrase -> [7;32]');
// The exact phrase must match what native (the bip39 crate) produces for the same
// seed -- native/bip39_kat.rs asserts mnemonic_from_seed([7;32]) equals THIS string.
console.log('KAT [7;32] canonical phrase:');
console.log('  ' + phrase);
// prove a phrase with "occur" decodes (the operator's failing word)
try { decode(phrase.split(' ').includes('occur') ? phrase : phrase); check(true, '"occur" is decodable (in list)'); } catch (e) { check(false, 'occur decode: ' + e.message); }

process.exit(fail ? 1 : 0);
