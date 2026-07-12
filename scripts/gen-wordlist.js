#!/usr/bin/env node
// Regenerate the BIP39 English wordlist in web/chat/bip39-english.js and
// src/net/bip39_wordlist.rs from the canonical `bip39` crate that native uses to
// GENERATE + display seed phrases (Mnemonic::from_entropy). All three MUST be
// byte-identical or a phrase written on one client won't restore on another.
//
// A non-canonical list shipped for months and broke native->web restore
// ("<word> is an unknown word"). Guards: tests/bip39_wordlist_canonical.rs
// (native == crate) and scripts/check-bip39-wordlists.js (web == native).
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
 * \`bip39\` Rust crate (which native uses to GENERATE + display phrases) and to
 * src/net/bip39_wordlist.rs. Source: BIP39 / trezor python-mnemonic english.txt.
 *
 * DO NOT hand-edit. Regenerate with \`node scripts/gen-wordlist.js\`. These lists
 * MUST stay identical or a seed phrase written on one client will not restore on
 * another. Guards: tests/bip39_wordlist_canonical.rs + scripts/check-bip39-wordlists.js.
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

// src/net/bip39_wordlist.rs -- fallback decoder; the crate is the primary parser.
const rows = words.map(w => `    "${w}",`).join('\n');
const rust = `//! BIP39 English wordlist -- the canonical 2048-word list, byte-identical to the
//! \`bip39\` crate (which native uses to GENERATE + display phrases via
//! Mnemonic::from_entropy) and to web/chat/bip39-english.js. This array is ONLY
//! the fallback decoder for derive_keypair_from_mnemonic (the checksum-less path);
//! the crate is the primary parser. It MUST equal the crate or the fallback would
//! decode a phrase to the wrong seed. Guarded by tests/bip39_wordlist_canonical.rs.
//!
//! DO NOT hand-edit -- regenerate with \`node scripts/gen-wordlist.js\`.

pub const WORDLIST: [&str; 2048] = [
${rows}
];
`;
fs.writeFileSync(path.join(repo, 'src/net/bip39_wordlist.rs'), rust);

console.log('Regenerated from', path.basename(path.dirname(path.dirname(path.dirname(crateFile)))));
console.log('  web/chat/bip39-english.js  (' + web.length + ' bytes)');
console.log('  src/net/bip39_wordlist.rs  (' + rust.length + ' bytes)');
console.log('  occur present:', /\boccur\b/.test(web) && /"occur"/.test(rust));
