// recovery.js: Recovery through trusted people, the website's twin of src/relay/core/recovery.rs
// (2026-10-21). The design: docs/design/recovery-through-trusted-people.md.
//
// Every byte layout and word here is the Rust side's. scripts/tests/fixtures/recovery-v1.json
// holds the two to each other: scripts/tests/recovery-web.test.js reads what Rust made and writes
// what this file makes, and the Rust test `recovery_fixture_rust_and_the_website_agree` reads
// that back. Change both or neither.
//
// The post-quantum and hash primitives are the vendored noble bundle (/shared/vendor/
// noble-pq.bundle.js), handed in as `m` so this module runs the same in the browser and in Node;
// AES-256-GCM and randomness are WebCrypto (globalThis.crypto in both).
//
// Shamir's secret sharing over GF(256) is written out here, because the Rust side uses the blahaj
// crate (MIT, Distrust) and the website has no build step to bring a package in. It is blahaj's
// scheme exactly: the field reduced by x^8 + x^4 + x^3 + x^2 + 1 (0x11d) with 2 as generator, a
// share is [x, y0 .. y31], piece i sits at x = i + 1, every coefficient above the secret is a
// uniform random byte (0 to 255, the RUSTSEC-2024-0398 fix), and joining is Lagrange's formula at
// x = 0. The test checks the field against blahaj's own multiplication and division vectors, and
// pieces made on each side join on the other.

import { cborBytes, cborMap, cborText, cborUint, decodeCanonicalCbor } from './canonical-cbor.js';
import { buildSignedObject, verifyObjectSubmission } from './pq-object.js';

export const SHARE_OBJECT_TYPE = 'recovery_share_v1';
export const SECRET_LEN = 32;
export const SHARE_LEN = 1 + SECRET_LEN;
export const TOKEN_LEN = 32;
export const MIN_PEOPLE = 2;
export const MAX_PEOPLE = 10;
export const MIN_NEEDED = 2;
export const NOTE_PLAIN_LEN = 256;
export const KYBER_PUBLIC_LEN = 1184;
export const KYBER_CIPHERTEXT_LEN = 1088;
export const SEAL_OVERHEAD = KYBER_CIPHERTEXT_LEN + 12 + 16;
export const PIECE_LEN = SEAL_OVERHEAD + SHARE_LEN + TOKEN_LEN;
export const NOTE_LEN = SEAL_OVERHEAD + NOTE_PLAIN_LEN;
export const ANSWER_LEN = SEAL_OVERHEAD + SHARE_LEN;
export const ASK_DAYS = 7;
export const CODE_BYTES = 10;
export const CODE_CHARS = 16;
export const CODE_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
export const PIECE_DOMAIN = 'hum/recovery-piece/v1';
export const NOTE_DOMAIN = 'hum/recovery-note/v1';
export const ANSWER_DOMAIN = 'hum/recovery-answer/v1';
export const ASK_DOMAIN = 'hum/recovery-ask/v1';
export const WITHDRAW_PURPOSE = 'recovery_withdraw';
export const NOT_ME_PURPOSE = 'recovery_not_me';

const enc = new TextEncoder();
const dec = new TextDecoder();

function concat(parts) {
  let n = 0;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

export function hex(u8) { return Array.from(u8, (b) => b.toString(16).padStart(2, '0')).join(''); }
export function unhex(s) {
  if (typeof s !== 'string' || s.length % 2 || /[^0-9a-f]/i.test(s)) throw new Error('not hex');
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(2 * i, 2 * i + 2), 16);
  return out;
}
export function b64(u8) { let s = ''; for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i]); return btoa(s); }
export function unb64(s) { const bin = atob(s); const out = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i); return out; }

function randomBytes(n) {
  const out = new Uint8Array(n);
  globalThis.crypto.getRandomValues(out);
  return out;
}

// ── GF(256), blahaj's field ──

export const GF_POLY = 0x11d;
const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
(function tables() {
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= GF_POLY;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255];
})();
export const gf = {
  EXP, LOG,
  mul(a, b) { return a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]]; },
  div(a, b) { return a === 0 ? 0 : EXP[LOG[a] + 255 - LOG[b]]; },
};

/** Whether `needed` of `total` is a setup shape this protocol allows; throws with words if not. */
export function shapeOk(needed, total) {
  if (!(Number.isInteger(total) && total >= MIN_PEOPLE && total <= MAX_PEOPLE)) {
    throw new Error(`a setup has ${MIN_PEOPLE} to ${MAX_PEOPLE} trusted people, not ${total}`);
  }
  if (!(Number.isInteger(needed) && needed >= MIN_NEEDED && needed <= total)) {
    throw new Error(`${needed} of ${total} cannot be: at least ${MIN_NEEDED}, at most all of them`);
  }
}

/**
 * Split the 32 secret bytes into `total` pieces, any `needed` of which rebuild them. Piece i has
 * x = i + 1. `coefficients(n)` gives n random bytes (WebCrypto's by default; a test may fix them).
 */
export function split(secret, needed, total, coefficients = randomBytes) {
  if (!(secret instanceof Uint8Array) || secret.length !== SECRET_LEN) throw new Error('the secret is 32 bytes');
  shapeOk(needed, total);
  const coeff = coefficients(SECRET_LEN * (needed - 1));
  const shares = [];
  for (let i = 0; i < total; i++) {
    const x = i + 1;
    const share = new Uint8Array(SHARE_LEN);
    share[0] = x;
    for (let s = 0; s < SECRET_LEN; s++) {
      // Horner's rule, highest coefficient first, the secret byte last (blahaj's order).
      let acc = 0;
      for (let k = 0; k < needed - 1; k++) acc = gf.mul(acc, x) ^ coeff[s * (needed - 1) + k];
      share[1 + s] = gf.mul(acc, x) ^ secret[s];
    }
    shares.push(share);
  }
  return shares;
}

/** Put the 32 bytes back from `needed` or more pieces (a repeated x counts once; x = 0 refused). */
export function combine(shares, needed) {
  if (needed < MIN_NEEDED) throw new Error(`at least ${MIN_NEEDED} pieces are needed`);
  const seen = new Set();
  const kept = [];
  for (const s of shares) {
    if (!(s instanceof Uint8Array) || s.length !== SHARE_LEN) throw new Error('a piece is 33 bytes');
    if (s[0] === 0) throw new Error('a piece with no x coordinate');
    if (!seen.has(s[0])) { seen.add(s[0]); kept.push(s); }
  }
  if (kept.length < needed) throw new Error(`${kept.length} different pieces, and ${needed} are needed`);
  const out = new Uint8Array(SECRET_LEN);
  for (let b = 0; b < SECRET_LEN; b++) {
    let sum = 0;
    for (const si of kept) {
      let prod = 1;
      for (const sj of kept) if (sj[0] !== si[0]) prod = gf.mul(prod, gf.div(sj[0], sj[0] ^ si[0]));
      sum ^= gf.mul(prod, si[1 + b]);
    }
    out[b] = sum;
  }
  return out;
}

// ── Identity, as both apps derive it ──

/** `did:hum:` and base58 of the first 16 bytes of BLAKE3 of a public key (relay/core/did.rs). */
export function didForPublicKey(m, pk) {
  const fp = m.blake3(pk).slice(0, 16);
  const A = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let n = 0n;
  for (const b of fp) n = (n << 8n) | BigInt(b);
  let out = '';
  while (n > 0n) { out = A[Number(n % 58n)] + out; n /= 58n; }
  for (const b of fp) { if (b !== 0) break; out = '1' + out; }
  return 'did:hum:' + (out || '1');
}

function deriveKey(m, domain, material, dkLen = 32) {
  return m.blake3.create({ context: enc.encode(domain), dkLen }).update(material).digest();
}

/** The Dilithium3 key pair the 32 secret bytes derive (`hum/dilithium3/v1`). */
export function dilithiumOf(m, secret) { return m.ml_dsa65.keygen(deriveKey(m, 'hum/dilithium3/v1', secret)); }
/** The Kyber768 key pair the 32 secret bytes derive (`hum/kyber768/v1`). */
export function kyberOf(m, secret) { return m.ml_kem768.keygen(deriveKey(m, 'hum/kyber768/v1', secret, 64)); }
/** The account id the 32 secret bytes derive. */
export function accountOf(m, secret) { return didForPublicKey(m, dilithiumOf(m, secret).publicKey); }

/** Every way of choosing `k` of `n`, in order. */
function* choose(n, k) {
  const pick = Array.from({ length: k }, (_, i) => i);
  while (true) {
    yield pick.slice();
    let i = k - 1;
    while (i >= 0 && pick[i] === n - k + i) i--;
    if (i < 0) return;
    pick[i]++;
    for (let j = i + 1; j < k; j++) pick[j] = pick[j - 1] + 1;
  }
}

/** Rebuild `holderDid`'s secret from the pieces that came back, trying every choice of `needed`. */
export function rebuild(m, pieces, needed, holderDid) {
  if (needed < MIN_NEEDED || pieces.length < needed || pieces.length > 32) return null;
  for (const pick of choose(pieces.length, needed)) {
    let secret;
    try { secret = combine(pick.map((i) => pieces[i]), needed); } catch (_) { continue; }
    if (accountOf(m, secret) === holderDid) return secret;
  }
  return null;
}

// ── The code ──

export function base32(bytes) {
  let out = '', acc = 0, bits = 0;
  for (const b of bytes) {
    acc = ((acc << 8) | b) & 0xffff;
    bits += 8;
    while (bits >= 5) { bits -= 5; out += CODE_ALPHABET[(acc >> bits) & 31]; }
  }
  if (bits > 0) out += CODE_ALPHABET[(acc << (5 - bits)) & 31];
  return out;
}

/** BLAKE3 derive_key(ASK_DOMAIN, holder_did "\n" key), first 10 bytes, Crockford base 32. */
export function askCode(m, holderDid, oneTimeKey) {
  const digest = deriveKey(m, ASK_DOMAIN, concat([enc.encode(holderDid), enc.encode('\n'), oneTimeKey]));
  return base32(digest.slice(0, CODE_BYTES));
}

/** A code as typed or read: any case, spaces and dashes ignored, O as 0, I or L as 1. */
export function normalizeCode(typed) {
  let out = '';
  for (const ch of String(typed || '')) {
    let c = ch.toUpperCase();
    if (c === ' ' || c === '-' || c === '\t') continue;
    if (c === 'O') c = '0';
    if (c === 'I' || c === 'L') c = '1';
    if (c.length !== 1 || CODE_ALPHABET.indexOf(c) < 0) return null;
    out += c;
  }
  return out.length === CODE_CHARS ? out : null;
}

export function showCode(code) { return code.match(/.{1,4}/g).join('-'); }

// ── Sealing ──

export function pieceAad(holderDid, setId, slot, needed, total) { return `${PIECE_DOMAIN}\n${holderDid}\n${setId}\n${slot}\n${needed}\n${total}`; }
export function noteAad(holderDid, setId, slot) { return `${NOTE_DOMAIN}\n${holderDid}\n${setId}\n${slot}`; }
export function answerAad(code, holderDid, slot) { return `${ANSWER_DOMAIN}\n${code}\n${holderDid}\n${slot}`; }

async function aesKey(m, domain, ss, use) {
  return globalThis.crypto.subtle.importKey('raw', deriveKey(m, domain, ss), { name: 'AES-GCM' }, false, [use]);
}

/** ML-KEM-768 to `recipient`, BLAKE3 derive_key(domain, ss), AES-256-GCM with `aad`. */
export async function seal(m, domain, recipient, aad, plain) {
  if (!(recipient instanceof Uint8Array) || recipient.length !== KYBER_PUBLIC_LEN) throw new Error('a Kyber768 public key is 1184 bytes');
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(recipient);
  const key = await aesKey(m, domain, sharedSecret, 'encrypt');
  const nonce = randomBytes(12);
  const body = new Uint8Array(await globalThis.crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: nonce, additionalData: enc.encode(aad) }, key, plain));
  return concat([cipherText, nonce, body]);
}

/** Open what `seal` made for the holder of `secretKey`; null for anything else. */
export async function open(m, domain, secretKey, aad, sealed) {
  try {
    if (!(sealed instanceof Uint8Array) || sealed.length < SEAL_OVERHEAD) return null;
    const ss = m.ml_kem768.decapsulate(sealed.subarray(0, KYBER_CIPHERTEXT_LEN), secretKey);
    const key = await aesKey(m, domain, ss, 'decrypt');
    const nonce = sealed.subarray(KYBER_CIPHERTEXT_LEN, KYBER_CIPHERTEXT_LEN + 12);
    const body = sealed.subarray(KYBER_CIPHERTEXT_LEN + 12);
    return new Uint8Array(await globalThis.crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: nonce, additionalData: enc.encode(aad) }, key, body));
  } catch (_) {
    return null;
  }
}

export function tokenHash(m, token) { return m.blake3(token); }
export function closeHash(m, secret) { return hex(m.blake3(secret)); }
export function newSetId() { return hex(randomBytes(16)); }
export function setIdOk(s) { return typeof s === 'string' && /^[0-9a-f]{32}$/.test(s); }
export function accountIdOk(s) { return typeof s === 'string' && /^did:hum:[A-Za-z0-9]{16,32}$/.test(s); }

/** The note to self's plaintext: its JSON, zero bytes to 256; a name too long is cut. */
export function notePlain(note) {
  let name = String(note.name || '');
  for (;;) {
    const json = enc.encode(JSON.stringify({ did: note.did, name }));
    if (json.length <= NOTE_PLAIN_LEN) {
      const out = new Uint8Array(NOTE_PLAIN_LEN);
      out.set(json);
      return out;
    }
    name = Array.from(name).slice(0, -1).join('');
  }
}

export function parseNote(plain) {
  try {
    let end = plain.indexOf(0);
    if (end < 0) end = plain.length;
    const v = JSON.parse(dec.decode(plain.subarray(0, end)));
    return v && typeof v.did === 'string' && typeof v.name === 'string' ? { did: v.did, name: v.name } : null;
  } catch (_) {
    return null;
  }
}

// ── The object a setup posts ──

export function sharePayloadCbor(p) {
  return cborMap([
    [cborText('set_id'), cborText(p.setId)],
    [cborText('slot'), cborUint(p.slot)],
    [cborText('needed'), cborUint(p.needed)],
    [cborText('total'), cborUint(p.total)],
    [cborText('piece'), cborBytes(p.piece)],
    [cborText('note'), cborBytes(p.note)],
    [cborText('token_hash'), cborBytes(p.tokenHash)],
  ]);
}

const PAYLOAD_KEYS = ['needed', 'note', 'piece', 'set_id', 'slot', 'token_hash', 'total'];

/** Read and check a payload exactly as the relay does (SharePayload::parse). Throws with words. */
export function parseSharePayload(bytes) {
  const v = decodeCanonicalCbor(bytes);
  if (!v || typeof v !== 'object' || Array.isArray(v)) throw new Error('the payload is not a map');
  const keys = Object.keys(v).sort();
  if (keys.join(',') !== PAYLOAD_KEYS.join(',')) throw new Error('a recovery_share_v1 carries exactly ' + PAYLOAD_KEYS.join(', '));
  const small = (n) => Number.isInteger(n) && n >= 0 && n <= 255;
  if (!setIdOk(v.set_id)) throw new Error('set_id is not 32 lowercase hex characters');
  if (!small(v.slot) || !small(v.needed) || !small(v.total)) throw new Error('slot, needed and total are small numbers');
  shapeOk(v.needed, v.total);
  if (v.slot >= v.total) throw new Error(`slot ${v.slot} is outside a setup of ${v.total}`);
  if (!(v.piece instanceof Uint8Array) || v.piece.length !== PIECE_LEN) throw new Error(`the piece is not ${PIECE_LEN} bytes`);
  if (!(v.note instanceof Uint8Array) || v.note.length !== NOTE_LEN) throw new Error(`the note is not ${NOTE_LEN} bytes`);
  if (!(v.token_hash instanceof Uint8Array) || v.token_hash.length !== 32) throw new Error('token_hash is not 32 bytes');
  return { setId: v.set_id, slot: v.slot, needed: v.needed, total: v.total, piece: v.piece, note: v.note, tokenHash: v.token_hash };
}

/**
 * Build a whole setup. `secret` is the 32 bytes the recovery phrase writes down; `people` is
 * [{did, name, kyberPublic}]. Returns { setId, submissions } (POST /api/v2/objects shapes).
 */
export async function buildSetup(m, { secret, people, needed, createdAt }) {
  const total = people.length;
  shapeOk(needed, total);
  const holder = dilithiumOf(m, secret);
  const holderDid = didForPublicKey(m, holder.publicKey);
  const holderKyber = kyberOf(m, secret).publicKey;
  people.forEach((p, i) => {
    if (p.did === holderDid) throw new Error('you cannot keep a piece of your own');
    if (people.slice(0, i).some((q) => q.did === p.did)) throw new Error(`${p.name} is chosen twice`);
    if (!(p.kyberPublic instanceof Uint8Array) || p.kyberPublic.length !== KYBER_PUBLIC_LEN) throw new Error(`${p.name}'s key is not known`);
  });
  const setId = newSetId();
  const shares = split(secret, needed, total);
  const submissions = [];
  for (let slot = 0; slot < total; slot++) {
    const person = people[slot];
    const token = randomBytes(TOKEN_LEN);
    const piece = await seal(m, PIECE_DOMAIN, person.kyberPublic, pieceAad(holderDid, setId, slot, needed, total), concat([shares[slot], token]));
    const note = await seal(m, NOTE_DOMAIN, holderKyber, noteAad(holderDid, setId, slot), notePlain({ did: person.did, name: person.name }));
    const payload = sharePayloadCbor({ setId, slot, needed, total, piece, note, tokenHash: tokenHash(m, token) });
    const { submission } = await buildSignedObject({
      objectType: SHARE_OBJECT_TYPE,
      payload,
      authorPublicKey: holder.publicKey,
      sign: async (bytes) => m.ml_dsa65.sign(bytes, holder.secretKey),
      blake3: (b) => m.blake3(b),
      createdAt,
    });
    submissions.push(submission);
  }
  return { setId, holderDid, submissions };
}

/** Read a setup object the relay served: verified, of the right type, its payload parsed. */
export async function servedPiece(m, obj) {
  if (!obj || obj.object_type !== SHARE_OBJECT_TYPE) throw new Error('not a ' + SHARE_OBJECT_TYPE);
  const v = await verifyObjectSubmission(obj, {
    blake3: (b) => m.blake3(b),
    pqVerify: async (pub, msg, sig) => m.ml_dsa65.verify(sig, msg, pub),
  });
  if (!v.ok) throw new Error('the signature does not verify');
  const authorPub = unb64(obj.author_public_key_b64);
  return { holderDid: didForPublicKey(m, authorPub), createdAt: obj.created_at || 0, payload: parseSharePayload(unb64(obj.payload_b64)) };
}

/** Try to open a piece with my Kyber secret key: { share, token } only for my own. */
export async function openPiece(m, kyberSecret, p) {
  const pl = p.payload;
  const plain = await open(m, PIECE_DOMAIN, kyberSecret, pieceAad(p.holderDid, pl.setId, pl.slot, pl.needed, pl.total), pl.piece);
  if (!plain || plain.length !== SHARE_LEN + TOKEN_LEN || plain[0] !== pl.slot + 1) return null;
  const share = plain.slice(0, SHARE_LEN);
  const token = plain.slice(SHARE_LEN);
  return hex(tokenHash(m, token)) === hex(pl.tokenHash) ? { share, token } : null;
}

/** Open a note to self with my own Kyber secret key. */
export async function openNote(m, kyberSecret, p) {
  const plain = await open(m, NOTE_DOMAIN, kyberSecret, noteAad(p.holderDid, p.payload.setId, p.payload.slot), p.payload.note);
  return plain ? parseNote(plain) : null;
}

export async function sealAnswer(m, oneTimeKey, code, holderDid, slot, share) {
  return seal(m, ANSWER_DOMAIN, oneTimeKey, answerAad(code, holderDid, slot), share);
}

export async function openAnswer(m, oneTimeSecret, code, holderDid, slot, sealed) {
  const plain = await open(m, ANSWER_DOMAIN, oneTimeSecret, answerAad(code, holderDid, slot), sealed);
  return plain && plain.length === SHARE_LEN && plain[0] === slot + 1 ? plain : null;
}

/** A new device's one-time key pair and the secret that closes its ask. Memory only. */
export function newOneTimeAsk(m) {
  const keys = m.ml_kem768.keygen(randomBytes(64));
  return { publicKey: keys.publicKey, secretKey: keys.secretKey, closeSecret: randomBytes(32) };
}
