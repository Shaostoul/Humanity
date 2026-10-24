// ── admin-actions.js ──────────────────────────────────────────────────────
// The server's admin action registry, from a web page (2026-10-22;
// docs/design/in-app-ops.md, the north star; the relay half is
// src/relay/admin/). The registry (data/admin/actions.json, served at
// GET /api/admin/actions) lists every action a server's admins and
// moderators can take; POST /api/admin/actions/run runs one, signed with the
// person's Dilithium3 key over the words `adminRunPurpose` builds, which are
// the relay's `run_purpose`: "admin_action", the action's id, and the BLAKE3
// of the exact inputs text. scripts/tests/admin-actions-web.test.js holds the
// two to the same example (the registry's `signing.example`).
//
// The pure rules (the purpose, the chat text, the confirm word, the inputs
// from a form, a masked secret in words) are here and tested in Node; the
// page's panels are web/pages/admin-console.js. The desktop app has the same
// panels (src/gui/pages/server_settings/).
//
// A classic script in the browser (window.HosAdminActions), a CommonJS
// module under Node. Needs /shared/blake3.js (HosBlake3) in the browser, and
// /shared/pq-relay-auth.js (getPqSignedAuth) to run anything.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const B3 = (typeof module === 'object' && module && module.exports)
    ? require('./blake3.js')
    : root.HosBlake3;

  /** The words a run signs (before "\n<timestamp>"). */
  function adminRunPurpose(action, inputsText) {
    const bytes = new TextEncoder().encode(String(inputsText));
    return 'admin_action\n' + action + '\n' + B3.toHex(B3.hash(bytes));
  }

  /** The chat command an action names, its inputs filled in, an empty one leaving no gap. */
  function adminChatText(action, inputs) {
    if (!action || !action.chat) return null;
    const words = action.chat.split(/\s+/).filter(Boolean).map(function (w) {
      const m = /^\{(.+)\}$/.exec(w);
      if (!m) return w;
      const v = inputs ? inputs[m[1]] : undefined;
      if (v === undefined || v === null) return '';
      return String(v).trim();
    }).filter(function (w) { return w !== ''; });
    return words.join(' ');
  }

  /** Whether an action cannot be undone and needs its confirm word typed. */
  function adminNeedsConfirm(action) {
    return !!action && action.danger === 'destructive';
  }

  /** Whether what was typed is the action's confirm word (trimmed, exact). */
  function adminConfirmOk(action, typed) {
    if (!adminNeedsConfirm(action)) return true;
    return String(typed || '').trim() === String(action.confirm_word || 'CONFIRM');
  }

  /**
   * A form's values as the inputs the action takes: numbers as numbers, an
   * unticked box as false, empty boxes left out. { ok, inputs } or { ok: false, error }.
   */
  function adminInputsFromForm(action, values) {
    const out = {};
    for (const i of (action.inputs || [])) {
      const raw = values ? values[i.name] : undefined;
      if (i.kind === 'bool') {
        out[i.name] = raw === true || raw === 'true';
        continue;
      }
      const text = raw === undefined || raw === null ? '' : String(raw).trim();
      if (!text) {
        if (i.required) return { ok: false, error: i.label + ' is needed.' };
        continue;
      }
      if (i.kind === 'number') {
        if (!/^-?\d+$/.test(text)) return { ok: false, error: i.label + ' must be a whole number.' };
        out[i.name] = parseInt(text, 10);
      } else {
        out[i.name] = text;
      }
    }
    return { ok: true, inputs: out };
  }

  /** A masked secret from the server ({set, host, ends}) in words: "discord.com, ends ab12". */
  function adminMaskedWords(m) {
    if (!m || m.set !== true) return 'not set';
    const host = m.host || '';
    const ends = m.ends || '';
    if (host && ends) return host + ', ends ' + ends;
    if (host) return host;
    if (ends) return 'ends ' + ends;
    return 'set';
  }

  /** "3 min ago" for a time in milliseconds since 1970. */
  function adminTimeAgo(ms, now) {
    const s = Math.max(0, Math.floor(((now || Date.now()) - ms) / 1000));
    if (s < 60) return 'just now';
    if (s < 3600) return Math.floor(s / 60) + ' min ago';
    if (s < 86400) return Math.floor(s / 3600) + ' h ago';
    return Math.floor(s / 86400) + ' days ago';
  }

  /** "2 h" for a span in seconds. */
  function adminSpan(secs) {
    if (secs < 60) return secs + ' s';
    if (secs < 3600) return Math.floor(secs / 60) + ' min';
    if (secs < 86400) return Math.floor(secs / 3600) + ' h';
    return Math.floor(secs / 86400) + ' days';
  }

  /** How a federation peer is doing, in words (the desktop's peer_state_words). */
  function adminPeerState(p) {
    // An admin's pause (the peer left out of federation), never the link's automatic hold-back.
    if (p.paused) return 'paused by an admin';
    // last_seen moves on every status change and, while a link is up, on every message from the
    // peer: "last heard" while linked, "since" for the status it has otherwise.
    const ago = p.last_seen ? adminTimeAgo(p.last_seen) : null;
    if (p.linked) return 'linked' + (p.linked_secs != null ? ' for ' + adminSpan(p.linked_secs) : '') + (ago ? ', last heard ' + ago : '');
    const why = (p.trust_tier || 0) < 2 ? ' (trust below 2: not federating)' : '';
    return 'not linked, ' + (p.status || 'unknown') + (ago ? ' since ' + ago : '') + why;
  }

  /**
   * The link's own state in words (the `link` object the relay serves, federation_log.rs
   * peer_view; the desktop's link_words): its flow (normally, slowed, or held back automatically
   * for sending too fast, which is not an admin's pause), how often it was held back, what was
   * missed or set aside, how far this server read the peer's log. Null with no link state.
   */
  function adminPeerLink(p) {
    const flow = p && p.link && p.link.flow;
    if (!flow || typeof flow !== 'object') return null;
    const parts = [flow.state === 'paused' ? 'held back automatically for sending too fast'
      : flow.state === 'slowed' ? 'slowed automatically for sending fast' : 'flowing normally'];
    if (flow.pauses > 0) parts.push('held back ' + flow.pauses + ' times');
    if (flow.missed > 0) parts.push(flow.missed + ' messages missed');
    if (flow.set_aside > 0) parts.push(flow.set_aside + ' set aside');
    const seq = p.link.cursor && p.link.cursor.seq;
    if (typeof seq === 'number') parts.push('read their log to message ' + seq);
    return 'Link: ' + parts.join('; ') + '.';
  }

  /** GET /api/admin/actions. */
  async function adminFetchRegistry() {
    const res = await fetch('/api/admin/actions', { cache: 'no-cache' });
    if (res.status === 404) throw new Error('This server is older than its list of admin actions: update it to see them here.');
    if (!res.ok) throw new Error('The server answered ' + res.status + '.');
    return await res.json();
  }

  /**
   * Run one action, signed. Resolves to the server's answer ({ok, message,
   * data}); rejects with the server's reason in words.
   */
  async function adminRun(action, inputs, confirm) {
    const text = JSON.stringify(inputs || {});
    if (typeof root.getPqSignedAuth !== 'function') throw new Error('Signing is not available on this page.');
    const auth = await root.getPqSignedAuth(adminRunPurpose(action, text));
    if (!auth) throw new Error('No Humanity identity in this browser: sign in via Chat first.');
    const body = { key: auth.key, timestamp: auth.timestamp, sig: auth.sig, action: action, inputs: text };
    if (confirm) body.confirm = confirm;
    const res = await fetch('/api/admin/actions/run', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    let data = null;
    try { data = await res.json(); } catch (e) { data = null; }
    if (!res.ok || !data || data.ok === false) {
      const why = data && data.error ? data.error
        : (res.status === 404 ? 'This server is older than the admin actions: update it first.' : 'The server answered ' + res.status + '.');
      throw new Error(why);
    }
    return data;
  }

  const api = {
    adminRunPurpose, adminChatText, adminNeedsConfirm, adminConfirmOk, adminInputsFromForm,
    adminMaskedWords, adminTimeAgo, adminSpan, adminPeerState, adminPeerLink, adminFetchRegistry, adminRun,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosAdminActions = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
