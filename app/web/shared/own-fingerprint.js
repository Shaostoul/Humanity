// own-fingerprint.js: a person's OWN key fingerprint, in words (2026-11-21, the owner branch's
// fifth review, H1 (a)). When the owner of a server hands someone the owner role or names them
// successor, the owner's page names four POSITIONS ("ask them to read you groups 2, 3, 5 and 6 of
// their own fingerprint") and the owner types what the person reads out. So every person must be
// able to see their own key's fingerprint where they look after their account: the Settings page's
// Account section (#own-fingerprint) and the chat's Account and Identity menu (#my-fingerprint,
// filled by chat-owner.js the moment the page knows its key). Nowhere else (the sixth review's H1):
// not on a check prompt, never in a chat line; and the words beside it say so, so a fingerprint a
// server writes into a chat line or a prompt is never read out. The desktop app's twin is
// src/gui/pages/settings/own_fingerprint.rs.
//
// The fingerprint is /shared/key-card.js `fingerprint` (BLAKE3 derive_key over the Dilithium3 key,
// six groups of four hex digits). Built as DOM: textContent only. A classic script in the browser
// (window.HosOwnFingerprint) and a CommonJS module under Node (scripts/tests/owner-chat-web.test.js).
(function (root) {
  'use strict';

  function words(key, vars) {
    const i = (typeof root !== 'undefined' && root && root.i18n) || (typeof i18n !== 'undefined' ? i18n : null);
    return i && typeof i.t === 'function' ? i.t(key, vars) : key;
  }
  function hintOn(node, key) {
    const i = (typeof root !== 'undefined' && root && root.i18n) || (typeof i18n !== 'undefined' ? i18n : null);
    if (i && typeof i.hint === 'function') i.hint(node, key);
  }

  /**
   * Fill `el` with the fingerprint of `keyHex` ("Your key's fingerprint: 3fa2 91c0 ...", monospace,
   * one line, with its hint) and, beneath it, why it is there and that it is shown only here (the
   * sixth review's H1 (b)). `deriveKey(context, bytes)` is BLAKE3 derive_key (32
   * bytes out); `K` is /shared/key-card.js. Empties `el` when there is no key. Returns the
   * fingerprint ('' for none).
   */
  function fill(el, keyHex, deriveKey, K) {
    if (!el) return '';
    const fp = K && typeof K.fingerprint === 'function' && keyHex ? K.fingerprint(String(keyHex), deriveKey) : '';
    el.textContent = '';
    if (!fp) return '';
    const doc = el.ownerDocument || (typeof document !== 'undefined' ? document : null);
    const line = doc.createElement('div');
    line.className = 'own-fp-line';
    line.textContent = words('owner.my_fp', { fp });
    line.style.fontFamily = 'var(--font-mono)';
    hintOn(line, 'owner.my_fp_hint');
    const why = doc.createElement('div');
    why.className = 'own-fp-explain';
    why.textContent = words('owner.my_fp_explain');
    why.style.color = 'var(--text-muted)';
    why.style.fontSize = 'var(--text-sm)';
    const only = doc.createElement('div');
    only.className = 'own-fp-only';
    only.textContent = words('owner.my_fp_only_here');
    only.style.color = 'var(--warning)';
    only.style.fontSize = 'var(--text-sm)';
    only.style.fontWeight = '600';
    // This page came from the server: what it shows is only as trustworthy as the server that sent
    // it (the owner branch's seventh review, M2); the desktop app works it out apart from any server.
    const trust = doc.createElement('div');
    trust.className = 'own-fp-trust';
    trust.textContent = words('chat_owner.my_fp_server_page');
    trust.style.color = 'var(--text-muted)';
    trust.style.fontSize = 'var(--text-sm)';
    el.appendChild(line);
    el.appendChild(why);
    el.appendChild(only);
    el.appendChild(trust);
    return fp;
  }

  /** The Settings page: the identity's key from the stored recovery seed (/shared/pq-relay-auth.js
   *  getPqIdentity), the hash from the vendored noble bundle (/chat/pq.js `_pqLoad`), into
   *  #own-fingerprint. Says nothing when no identity can be read here. */
  async function showOnSettings() {
    const el = typeof document !== 'undefined' ? document.getElementById('own-fingerprint') : null;
    if (!el || typeof root.getPqIdentity !== 'function' || typeof root._pqLoad !== 'function') return '';
    try {
      const [id, noble] = await Promise.all([root.getPqIdentity(), root._pqLoad()]);
      if (!id || !noble || !noble.blake3) return '';
      const derive = (ctx, bytes) => noble.blake3.create({ context: new TextEncoder().encode(ctx), dkLen: 32 }).update(bytes).digest();
      return fill(el, id.dilithiumPublicHex, derive, root.HosKeyCard);
    } catch (e) {
      console.warn('Settings: the fingerprint could not be shown:', e);
      return '';
    }
  }

  const api = { fill, showOnSettings };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  if (root) root.HosOwnFingerprint = api;
})(typeof window !== 'undefined' ? window : null);
