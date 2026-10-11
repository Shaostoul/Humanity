// ── relay-session.js ──────────────────────────────────────────────────────
// A standalone page's own signed-in connection to this server (2026-10-11,
// docs/design/media-showcase.md "Increment 2, exact").
//
// Why a page needs its own connection: the relay hands out a session's upload
// token (what `POST /api/upload?token=` and the members-only Media file route
// `GET /api/media/file/{id}?token=` take) only on a socket it has signed in, in
// that socket's first `peer_list`; and the server's settings (Media's
// visibility, budget and each person's share) come over the socket too
// (`server_settings_request` answered with `server_settings_state`). A page
// cannot borrow Chat's socket: it lives in another tab. So the page signs in
// the way the Trade and Tasks pages do (trade-app.js, tasks-app.js):
//   1. it sends `identify` with the person's Dilithium3 public key;
//   2. the relay answers `identify_challenge {nonce}`;
//   3. the page signs "hum/identify/v1\n{nonce}\n{public_key}" and answers
//      `identify_response {sig_b64}` (src/relay/relay.rs, Inc3b);
//   4. `peer_list` arrives once the relay accepted the proof: the person's own
//      entry carries the upload token, the role and the upload limit.
// Until then nothing else is sent, and no key is ever made up: with no
// identity in this browser the session stays "no_identity" and opens nothing.
//
// The identity is the one Chat keeps in this browser (getPqIdentity in
// /shared/pq-relay-auth.js, derived by /chat/pq.js). Signed REST requests go
// through getPqSignedAuth in the same file.
//
// A classic script in the browser (its names land on window), a CommonJS module
// under Node, where scripts/tests/media-web.test.js drives it with a stand-in
// socket.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const IDENTIFY_DOMAIN = 'hum/identify/v1';
  // The relay's per-connection throttle closes the socket; the close handler backs off.
  const THROTTLE_PREFIX = 'Too many connection attempts';
  const RETRY_FIRST_MS = 5000;
  const RETRY_MAX_MS = 60000;

  /** The exact words the relay verifies: format!("hum/identify/v1\n{}\n{}", nonce, public_key). */
  function identifyPreimage(nonce, publicKeyHex) {
    return IDENTIFY_DOMAIN + '\n' + nonce + '\n' + publicKeyHex;
  }

  /** Standard base64 (what the relay's B64.decode expects), one byte at a time. */
  function bytesToB64(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
  }

  /**
   * The session. `opts`:
   *   getIdentity()  -> Promise<{dilithiumPublicHex, dilithiumSecret}|null>
   *   signBytes(secret, bytes) -> Promise<Uint8Array|null>   (window.pqSignMessage)
   *   WebSocket, location {protocol, host}, storage (localStorage),
   *   setTimeout, clearTimeout
   * Its `state`: status ('idle', 'no_identity', 'connecting', 'bound',
   * 'refused', 'closed'), key, role, uploadToken, uploadLimitBytes, settings
   * (the last server_settings_state's settings, or null), refusal (the
   * relay's words when it refused the sign-in) and retryInSeconds (after a
   * drop, when the next try is). Listeners get
   * `{type: 'state'}` on any change and `{type: 'frame', msg}` for every frame
   * after the sign-in.
   */
  function createRelaySession(opts) {
    const o = opts || {};
    const listeners = new Set();
    const state = {
      status: 'idle',
      key: '',
      role: '',
      uploadToken: '',
      uploadLimitBytes: 0,
      settings: null,
      refusal: '',
      retryInSeconds: 0,
    };
    let ws = null;
    let identity = null;
    let retryMs = RETRY_FIRST_MS;
    let retryTimer = null;
    const later = o.setTimeout || ((fn, ms) => setTimeout(fn, ms));
    const cancel = o.clearTimeout || ((id) => clearTimeout(id));

    function emit(ev) {
      for (const fn of Array.from(listeners)) {
        try { fn(ev, state); } catch (e) { if (typeof console !== 'undefined') console.warn('[relay-session] listener failed', e); }
      }
    }
    function changed() { emit({ type: 'state' }); }

    function isOpen(sock) {
      return !!sock && sock.readyState === 1;
    }

    /** Send one frame on the signed-in socket; false, sending nothing, when it is not signed in. */
    function send(frame) {
      if (state.status !== 'bound' || !isOpen(ws)) return false;
      ws.send(JSON.stringify(frame));
      return true;
    }

    function refuse(sock, reason) {
      state.refusal = reason || 'The server refused the sign-in.';
      state.status = 'refused';
      changed();
      try { sock.close(); } catch (e) { /* already closed */ }
    }

    /** The person's own entry in a peer list, or null. */
    function ownEntry(peers) {
      if (!Array.isArray(peers)) return null;
      return peers.find((p) => p && p.public_key === state.key) || null;
    }

    /** Take what the person's own entry says; true when something changed. */
    function takePeerList(peers) {
      const me = ownEntry(peers);
      if (!me) return false;
      const before = state.uploadToken + '|' + state.role + '|' + state.uploadLimitBytes;
      if (typeof me.upload_token === 'string' && me.upload_token) state.uploadToken = me.upload_token;
      if (typeof me.role === 'string') state.role = me.role;
      if (Number.isFinite(me.upload_limit_mb) && me.upload_limit_mb > 0) state.uploadLimitBytes = me.upload_limit_mb * 1024 * 1024;
      return before !== state.uploadToken + '|' + state.role + '|' + state.uploadLimitBytes;
    }

    function onFrame(sock, m) {
      if (ws !== sock || !m || typeof m !== 'object') return;
      // ── Sign-in phase: nothing else counts until the relay accepts the proof ──
      if (state.status !== 'bound') {
        if (m.type === 'identify_challenge') {
          const preimage = new TextEncoder().encode(identifyPreimage(m.nonce, identity.dilithiumPublicHex));
          Promise.resolve(o.signBytes(identity.dilithiumSecret, preimage)).then((sig) => {
            if (!sig || !sig.length) { refuse(sock, 'this page could not sign the server\'s sign-in check with your identity.'); return; }
            if (isOpen(sock) && ws === sock) sock.send(JSON.stringify({ type: 'identify_response', sig_b64: bytesToB64(sig) }));
          }, () => refuse(sock, 'this page could not sign the server\'s sign-in check with your identity.'));
        } else if (m.type === 'peer_list') {
          // The relay sends peer_list first thing after it binds the socket.
          state.status = 'bound';
          state.refusal = '';
          retryMs = RETRY_FIRST_MS;
          takePeerList(m.peers);
          changed();
          send({ type: 'server_settings_request' });
        } else if (m.type === 'account_erased') {
          refuse(sock, m.partial === true
            ? 'the erase of your account on this server did not finish, so open Chat, press Enter and use Erase account again.'
            : 'your account on this server was erased, so open Chat and press Enter there to sign up again.');
        } else if (m.type === 'name_taken' || m.type === 'system') {
          const text = String(m.message || '');
          if (m.type === 'system' && text.startsWith(THROTTLE_PREFIX)) return;
          refuse(sock, text);
        }
        return;
      }
      if (m.type === 'peer_list') {
        // Every join and leave sends one; only a change to the person's own entry is news.
        if (takePeerList(m.peers)) changed();
      } else if (m.type === 'server_settings_state' && m.settings && typeof m.settings === 'object') {
        state.settings = m.settings;
        changed();
      }
      emit({ type: 'frame', msg: m });
    }

    /** Sign in, or say why not. Resolves true when a socket is open or on its way. */
    async function start() {
      if (ws) return true;
      if (!identity) {
        try { identity = o.getIdentity ? await o.getIdentity() : null; } catch (e) { identity = null; }
      }
      if (!identity || !identity.dilithiumPublicHex || !identity.dilithiumSecret || !o.WebSocket) {
        identity = null;
        state.status = 'no_identity';
        changed();
        return false;
      }
      if (ws) return true; // another call opened it while the key was being derived
      state.key = identity.dilithiumPublicHex;
      const loc = o.location || {};
      const proto = loc.protocol === 'https:' ? 'wss:' : 'ws:';
      const sock = new o.WebSocket(proto + '//' + loc.host + '/ws');
      ws = sock;
      state.status = 'connecting';
      state.uploadToken = '';
      changed();
      sock.addEventListener('open', () => {
        if (ws !== sock) return;
        // A placeholder name is never sent: the relay registers any real-looking
        // name to the key. The name is the one Chat saved in this browser; with
        // none, the relay signs the socket in under the name already registered.
        let name = null;
        try { name = ((o.storage && o.storage.getItem('humanity_name')) || '').trim() || null; } catch (e) { name = null; }
        sock.send(JSON.stringify({ type: 'identify', public_key: identity.dilithiumPublicHex, display_name: name }));
      });
      sock.addEventListener('message', (e) => {
        let m;
        try { m = JSON.parse(e.data); } catch (ex) { return; }
        onFrame(sock, m);
      });
      sock.addEventListener('close', () => {
        if (ws !== sock) return;
        ws = null;
        // A closed socket's upload token is gone on the relay too.
        state.uploadToken = '';
        if (state.status === 'refused') { changed(); return; }
        state.status = 'closed';
        state.retryInSeconds = Math.round(retryMs / 1000);
        changed();
        cancel(retryTimer);
        retryTimer = later(() => { retryTimer = null; start(); }, retryMs);
        retryMs = Math.min(retryMs * 2, RETRY_MAX_MS);
      });
      return true;
    }

    /** Try again pressed: an earlier refusal gets one more try. */
    function retry() {
      state.refusal = '';
      retryMs = RETRY_FIRST_MS;
      cancel(retryTimer);
      retryTimer = null;
      if (state.status === 'refused') state.status = 'idle';
      return start();
    }

    return {
      state,
      start,
      retry,
      send,
      /** Ask for the server's settings again (answered with server_settings_state). */
      requestSettings: () => send({ type: 'server_settings_request' }),
      subscribe(fn) { listeners.add(fn); return () => listeners.delete(fn); },
    };
  }

  const api = { IDENTIFY_DOMAIN, identifyPreimage, createRelaySession };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
