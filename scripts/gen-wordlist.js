#!/usr/bin/env node
// Regenerate the BIP39 English wordlist in web/chat/bip39-english.js from the
// canonical `bip39` crate the desktop app writes and reads recovery phrases with
// (src/net/identity.rs). The two MUST be the same list or a phrase written on one
// app won't restore on the other.
//
// A non-canonical list shipped for months and broke native->web restore
// ("<word> is an unknown word"). Guards: the identity.rs test
// bip39_wordlist_is_canonical_and_matches_web_kat pins the crate list's SHA-256
// (WORDLIST_SHA256), and scripts/check-bip39-wordlists.js holds the web list to it.
// The desktop app's own copy of the list (src/net/bip39_wordlist.rs) was deleted
// 2026-10-16 with the checksum-less fallback that was its only use.
//
// Usage: node scripts/gen-wordlist.js   (run after a `bip39` crate version bump)
const fs = require('fs');
const path = require('path');
const os = require('os');

function findCrateEnglish() {
  const roots = [
    process.env.CARGO_HOME && path.join(process.env.CARGO_HOME, 'registry', 'src'),
    path.join(os.homedir(), '.cargo', 'registry', 'src'),
  ].filter(Boolean);
  for (const root of roots) {
    if (!fs.existsSync(root)) continue;
    for (const idx of fs.readdirSync(root)) {
      const langDir = path.join(root, idx);
      let entries = [];
      try { entries = fs.readdirSync(langDir); } catch { continue; }
      const bip = entries.filter(d => d.startsWith('bip39-')).sort().reverse();
      for (const b of bip) {
        const f = path.join(langDir, b, 'src', 'language', 'english.rs');
        if (fs.existsSync(f)) return f;
      }
    }
  }
  throw new Error('bip39 crate english.rs not found under CARGO_HOME. Run `cargo fetch` first.');
}

const crateFile = findCrateEnglish();
const words = (fs.readFileSync(crateFile, 'utf8').match(/"([a-z]+)"/g) || []).map(s => s.replace(/"/g, ''));
if (words.length !== 2048) throw new Error('crate list not 2048: ' + words.length);
if (words[0] !== 'abandon' || words[2047] !== 'zoo') throw new Error('anchors wrong: ' + words[0] + '..' + words[2047]);
if (new Set(words).size !== 2048) throw new Error('crate list has duplicates');

const repo = path.resolve(__dirname, '..');

// web/chat/bip39-english.js -- crypto.js needs Array.isArray(window.BIP39_ENGLISH) && len 2048.
let chunks = [];
for (let i = 0; i < words.length; i += 12) chunks.push('"' + words.slice(i, i + 12).join(' ') + ' "');
chunks[chunks.length - 1] = chunks[chunks.length - 1].replace(/ "$/, '"');
const web = `/**
 * BIP39 English wordlist -- the canonical 2048-word list, byte-identical to the
 * \`bip39\` Rust crate, which the desktop app writes and reads recovery phrases
 * with. Source: BIP39 / trezor python-mnemonic english.txt.
 *
 * DO NOT hand-edit. Regenerate with \`node scripts/gen-wordlist.js\`. These lists
 * MUST stay identical or a recovery phrase written on one client will not restore on
 * another. Guard: scripts/check-bip39-wordlists.js (the list's SHA-256, pinned in
 * src/net/identity.rs WORDLIST_SHA256).
 *
 * Loaded as a global so crypto.js can reference it without a module system.
 */
window.BIP39_ENGLISH = (
${chunks.join('+\n')}).split(" ");

if (window.BIP39_ENGLISH.length !== 2048) {
  console.error('BIP39 wordlist length error:', window.BIP39_ENGLISH.length, '(expected 2048)');
}
`;
fs.writeFileSync(path.join(repo, 'web/chat/bip39-english.js'), web);

console.log('wrote web/chat/bip39-english.js (' + words.length + ' words)');
