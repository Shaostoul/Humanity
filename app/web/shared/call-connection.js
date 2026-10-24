// ── call-connection.js ────────────────────────────────────────────────────
// How this person's calls and voice rooms connect (docs/design/blocking-and-safe-mode.md 10p,
// 2026-10-22; the operator: "what if someone is okay using google servers?"). The desktop app's
// twin is src/net/call_connection.rs; both read their words from /data/net/call_connection.json,
// so both apps say the same thing. Shared by the web chat (chat-voice-rooms.js and
// chat-voice-calls.js build each peer connection with rtcConfig; chat-privacy.js draws the choice
// in Settings > Safety) and by Node, where scripts/tests/calls-through-server.test.js holds it to
// the spec.
//
// Three modes, kept on this device (localStorage `hos_call_connection`, in try/catch):
//   private             (the default) only the server's call relay is offered to the other side,
//                       iceTransportPolicy 'relay' (step E, 10f, unchanged): nobody in the call
//                       sees this device's network address.
//   direct              this device's own addresses are offered too (iceTransportPolicy 'all',
//                       with the relay's own stun: address from the call credentials), so two
//                       people who both chose Direct can connect device to device. The relay is
//                       still offered, so a Direct person and a Private person connect through it.
//   direct_public_stun  as direct, and the public STUN server the person picked from the data
//                       file's list (`hos_call_public_stun`, default the first) is asked as well.
// Each person's choice governs only their OWN address. Anything else saved reads as private, and
// with the protected setup on every call is private (effective).
//
// A classic script in the browser (HosCallConnection lands on window), a CommonJS module under
// Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const MODE_KEY = 'hos_call_connection';
  const PUBLIC_KEY = 'hos_call_public_stun';
  const MODES = Object.freeze(['private', 'direct', 'direct_public_stun']);
  const DATA_URL = '/data/net/call_connection.json';
  // The line for a Direct call the server cannot relay, when the data file has not loaded (the
  // desktop app's DIRECT_WITHOUT_SERVER_FALLBACK, word for word).
  const DIRECT_WITHOUT_SERVER_FALLBACK = 'This server cannot carry calls yet, so this one connects device to device where your networks allow it. The people in it can see your network address.';

  /** A saved value read back: one of MODES, else 'private'. */
  function modeOf(id) {
    return MODES.includes(id) ? id : 'private';
  }

  /** The mode kept on this device. */
  function readMode(storage) {
    try { return modeOf(storage ? storage.getItem(MODE_KEY) : null); } catch (e) { return 'private'; }
  }

  /** Keep a mode on this device. False when the browser would not keep it. */
  function writeMode(storage, mode) {
    try { storage.setItem(MODE_KEY, modeOf(mode)); return true; } catch (e) { return false; }
  }

  /** The public service picked for the third mode ('' when none was). */
  function readPublic(storage) {
    try { return String((storage && storage.getItem(PUBLIC_KEY)) || ''); } catch (e) { return ''; }
  }

  function writePublic(storage, id) {
    try { storage.setItem(PUBLIC_KEY, String(id || '')); return true; } catch (e) { return false; }
  }

  /** The mode that applies: the protected setup is always private. */
  function effective(mode, protectedOn) {
    return protectedOn ? 'private' : modeOf(mode);
  }

  /** Whether this device's own network address is offered to the other side. */
  function showsMyAddress(mode) {
    return modeOf(mode) !== 'private';
  }

  /** Whether a public STUN server is asked as well. */
  function asksPublicStun(mode) {
    return modeOf(mode) === 'direct_public_stun';
  }

  /** The public service with id `chosen` in the data file's list, else the first; null when none. */
  function publicStunOf(words, chosen) {
    const list = words && Array.isArray(words.public_stun) ? words.public_stun : [];
    return list.find((s) => s && s.id === chosen) || list[0] || null;
  }

  /** The words for one mode (its id as the label when the file does not describe it). */
  function modeWords(words, mode) {
    const list = words && Array.isArray(words.modes) ? words.modes : [];
    return list.find((m) => m && m.id === mode) || { id: mode, label: mode.replace(/_/g, ' '), summary: '', detail: '' };
  }

  /** The line said when a Direct call cannot use the server's relay. */
  function directWithoutServerLine(words) {
    return (words && typeof words.direct_without_server === 'string' && words.direct_without_server) || DIRECT_WITHOUT_SERVER_FALLBACK;
  }

  /** Whether an RTCIceCandidate (or its JSON) is a relayed one. */
  function isRelayCandidate(c) {
    if (!c) return false;
    if (c.type) return c.type === 'relay';
    return / typ relay( |$)/.test(String(c.candidate || ''));
  }

  /**
   * The RTCPeerConnection settings for a call or voice room in `mode`, from the relay's call
   * credentials `creds` ({urls, username, credential}, or null when none came) and, in the third
   * mode, the public STUN address `publicStunUrl`:
   *   private: exactly the credentials, iceTransportPolicy 'relay' (10f); null without them.
   *   direct:  the credentials (their turn: and the relay's own stun: address), policy 'all'.
   *   direct_public_stun: and the public STUN address after them.
   * A Direct mode without credentials still connects device to device where it can.
   */
  function rtcConfig(mode, creds, publicStunUrl) {
    const m = modeOf(mode);
    const servers = creds ? [{ urls: creds.urls.slice(), username: creds.username, credential: creds.credential }] : [];
    if (m === 'private') return creds ? { iceServers: servers, iceTransportPolicy: 'relay' } : null;
    if (m === 'direct_public_stun' && typeof publicStunUrl === 'string' && publicStunUrl.startsWith('stun:')) {
      servers.push({ urls: publicStunUrl });
    }
    return { iceServers: servers, iceTransportPolicy: 'all' };
  }

  // The data file, fetched once per page when first wanted.
  let wordsPromise = null;
  let wordsLoaded = null;
  function loadWords(fetchFn) {
    if (wordsPromise) return wordsPromise;
    const f = fetchFn || (typeof root.fetch === 'function' ? root.fetch.bind(root) : null);
    if (!f) return Promise.resolve(null);
    wordsPromise = Promise.resolve()
      .then(() => f(DATA_URL, { cache: 'no-cache' }))
      .then((r) => (r && r.ok ? r.json() : null))
      .then((w) => { wordsLoaded = w && typeof w === 'object' ? w : null; return wordsLoaded; })
      .catch(() => { wordsPromise = null; return null; });
    return wordsPromise;
  }

  /** The words if they have loaded (null until then). */
  function words() {
    return wordsLoaded;
  }

  const api = {
    MODE_KEY, PUBLIC_KEY, MODES, DATA_URL, DIRECT_WITHOUT_SERVER_FALLBACK,
    modeOf, readMode, writeMode, readPublic, writePublic, effective, showsMyAddress, asksPublicStun,
    publicStunOf, modeWords, directWithoutServerLine, isRelayCandidate, rtcConfig, loadWords, words,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosCallConnection = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
