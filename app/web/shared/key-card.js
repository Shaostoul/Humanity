// ── key-card.js ───────────────────────────────────────────────────────────
// Key cards (2026-10-22, docs/design/key-cards.md): a person's Kyber768
// (ML-KEM-768) key signed by their own Dilithium3 (ML-DSA-65) identity, so the
// app can check that a key it is about to seal something to really is that
// person's, whatever server delivered it. The website's twin of the relay's
// src/relay/core/key_card.rs, held to it byte for byte by
// scripts/tests/key-card-web.test.js (which reads the pinned words out of that
// file) and by the shared fixture scripts/tests/fixtures/key-card-v1.json (made
// by the Rust builder, read by both apps' tests).
//
// THE CARD, as it travels: {"v":1,"key":K,"kyber":Y,"at":T,"sig":S}
//   key   the person's Dilithium3 public key, lowercase hex (3,904 characters)
//   kyber their Kyber768 key, standard base64 with padding (1,580 characters)
//   at    when it was signed, Unix milliseconds
//   sig   Dilithium3 by `key` over the words below, standard base64 (4,412)
// The words signed, as UTF-8:  hum/key-card/v1\n{key}\n{kyber}\n{at}
// No server is named in a card: it is good on every server.
//
// ONE SPELLING: both base64 fields must be the canonical spelling of their
// bytes (decoding and encoding again gives the same text), the key lowercase.
//
// PINNING (the same rule as the desktop app, key_card.rs `compare` and
// net/key_pins.rs): the first good card for a person is pinned. A later good
// card with the same key changes nothing (its time is noted); a different key
// with a newer time is a CHANGE, kept aside, and nothing is sealed to them
// until it is accepted; a different key with an older or equal time is ignored.
// A change waiting is dropped when a newer card brings the pinned key back.
//
// A classic script in the browser (window.HosKeyCard) and a CommonJS module
// under Node (the tests), the way profile-copy.js is. Checking a signature is
// handed a `verify(publicKeyBytes, messageBytes, signatureBytes)` (the chat's
// window.pqVerifyMessage, or noble in the tests), so this file loads nothing.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const CARD_TAG = 'hum/key-card/v1';
  const CARD_VERSION = 1;
  /** A Dilithium3 public key is 1,952 bytes: 3,904 lowercase hex characters. */
  const KEY_HEX_LEN = 1952 * 2;
  /** A Kyber768 key is 1,184 bytes. */
  const KYBER_LEN = 1184;
  /** A Dilithium3 signature is 3,309 bytes. */
  const SIG_LEN = 3309;
  const KYBER_B64_LEN = Math.ceil(KYBER_LEN / 3) * 4;
  const SIG_B64_LEN = Math.ceil(SIG_LEN / 3) * 4;
  /** The most cards one key_card_request asks for. */
  const REQUEST_MAX = 16;
  /** The button under the change notice. */
  const ACCEPT_LABEL = 'Accept the new key';

  /** The words a card's signature covers (sign their UTF-8 bytes). Byte for byte the relay's `card_text`. */
  function cardText(key, kyber, at) {
    return `${CARD_TAG}\n${key}\n${kyber}\n${String(at)}`;
  }

  const b64enc = (bytes) => {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
  };

  /** `text` as the bytes it is the canonical base64 of, exactly `len` of them; null otherwise. */
  function canonicalB64(text, len) {
    if (typeof text !== 'string') return null;
    let bytes;
    try {
      bytes = Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
    } catch (e) {
      return null;
    }
    return bytes.length === len && b64enc(bytes) === text ? bytes : null;
  }

  const isLowerHex = (s) => /^[0-9a-f]*$/.test(s);

  /** Is `key` a person's key as cards carry it: 3,904 lowercase hex characters. */
  function isKeyText(key) {
    return typeof key === 'string' && key.length === KEY_HEX_LEN && isLowerHex(key);
  }

  function hexBytes(hex) {
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
    return out;
  }

  /** The one shape a card may have (no signature checked). */
  function shapeOk(card) {
    return !!card && typeof card === 'object'
      && card.v === CARD_VERSION
      && isKeyText(card.key)
      && Number.isSafeInteger(card.at) && card.at > 0
      && typeof card.kyber === 'string' && card.kyber.length === KYBER_B64_LEN
      && typeof card.sig === 'string' && card.sig.length === SIG_B64_LEN
      && !!canonicalB64(card.kyber, KYBER_LEN)
      && !!canonicalB64(card.sig, SIG_LEN);
  }

  /**
   * A card from JSON: an object with exactly a card's five fields, of the right
   * types (the relay's `parse_card`); null for anything else, so nothing
   * unsigned rides along.
   */
  function parseCard(v) {
    if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
    const keys = Object.keys(v);
    if (keys.length !== 5) return null;
    const { v: ver, key, kyber, at, sig } = v;
    if (typeof ver !== 'number' || typeof key !== 'string' || typeof kyber !== 'string'
        || typeof at !== 'number' || typeof sig !== 'string') return null;
    return { v: ver, key, kyber, at, sig };
  }

  /** Is this a good card: the right shape, and signed by the very key it names. */
  async function verifyCard(card, verify) {
    if (!shapeOk(card) || typeof verify !== 'function') return false;
    try {
      const ok = await verify(hexBytes(card.key), new TextEncoder().encode(cardText(card.key, card.kyber, card.at)), canonicalB64(card.sig, SIG_LEN));
      return ok === true;
    } catch (e) {
      return false;
    }
  }

  /** Is this a good card FOR `person`: what the app checks before it seals anything to them. */
  async function cardFor(card, person, verify) {
    return !!card && card.key === person && verifyCard(card, verify);
  }

  /** The pin rule: 'first' | 'same' | 'changed' | 'stale' (the relay's `compare`). `pin` is {kyber, at} or null. */
  function compare(pin, kyber, at) {
    if (!pin) return 'first';
    if (pin.kyber === kyber) return 'same';
    return at > pin.at ? 'changed' : 'stale';
  }

  /**
   * Take a good card (already checked) into `state` = {pins, changes}, both
   * person -> {kyber, at}. Returns {seen, dirty}: what it meant, and whether
   * `state` changed (and should be saved). The desktop app's KeyPins::take.
   */
  function take(state, card) {
    const person = card.key;
    const pins = state.pins, changes = state.changes;
    const pin = Object.prototype.hasOwnProperty.call(pins, person) ? pins[person] : null;
    const change = Object.prototype.hasOwnProperty.call(changes, person) ? changes[person] : null;
    const seen = compare(pin, card.kyber, card.at);
    let dirty = false;
    if (seen === 'first') {
      pins[person] = { kyber: card.kyber, at: card.at };
      dirty = true;
    } else if (seen === 'same') {
      if (card.at > pin.at) { pin.at = card.at; dirty = true; }
      if (change && card.at > change.at) { delete changes[person]; dirty = true; }
    } else if (seen === 'changed') {
      if (!change || card.at > change.at) { changes[person] = { kyber: card.kyber, at: card.at }; dirty = true; }
    }
    return { seen, dirty };
  }

  /** The key to seal to `person`: the pinned one, none while a change waits or when nothing is pinned. */
  function sealingKey(state, person) {
    if (Object.prototype.hasOwnProperty.call(state.changes, person)) return null;
    const pin = Object.prototype.hasOwnProperty.call(state.pins, person) ? state.pins[person] : null;
    return pin ? pin.kyber : null;
  }

  /** Accept `person`'s changed key: it becomes the pin. Returns the new key, or null when none waited. */
  function accept(state, person) {
    if (!Object.prototype.hasOwnProperty.call(state.changes, person)) return null;
    const change = state.changes[person];
    delete state.changes[person];
    state.pins[person] = change;
    return change.kyber;
  }

  /**
   * Why nothing was sealed to someone, in the words both apps show (the relay's
   * `Refusal::text`): kind 'no_card', 'fetching' or 'changed'.
   */
  function refusalText(kind, name) {
    if (kind === 'changed') {
      return `Nothing was sent: ${name}'s encryption key changed. Accept the new key in your conversation with them first.`;
    }
    if (kind === 'fetching') {
      return `Nothing was sent: ${name}'s key card is still on its way from the server. Try again in a moment.`;
    }
    return `Nothing was sent: ${name} has no signed key card on this server, so there is no key we can check is really theirs. It appears once they sign in with a current version of the app.`;
  }

  /** The notice in a conversation with someone whose key changed (the relay's `change_notice`). */
  function changeNotice(name) {
    return `${name}'s encryption key changed. This happens when someone resets their keys. If you did not expect it, check with ${name} in person or on a call you trust before you accept. Until you do, nothing is sent to them.`;
  }

  /**
   * Sign my own card: `sign(messageBytes)` returns the Dilithium3 signature
   * (the chat's pqSignMessage with my secret). `kyberB64` is my Kyber768 key
   * as base64. Returns the card, or null when signing failed.
   */
  async function signCard(keyHex, kyberB64, at, sign) {
    const sig = await sign(new TextEncoder().encode(cardText(keyHex, kyberB64, at)));
    if (!sig) return null;
    return { v: CARD_VERSION, key: keyHex, kyber: kyberB64, at, sig: b64enc(sig) };
  }

  const api = {
    CARD_TAG, CARD_VERSION, KEY_HEX_LEN, KYBER_LEN, SIG_LEN, KYBER_B64_LEN, SIG_B64_LEN, REQUEST_MAX, ACCEPT_LABEL,
    cardText, isKeyText, shapeOk, parseCard, verifyCard, cardFor, compare, take, sealingKey, accept,
    refusalText, changeNotice, signCard, canonicalB64,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  if (root) root.HosKeyCard = api;
})(typeof window !== 'undefined' ? window : null);
