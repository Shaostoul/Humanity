// ── reach.js ──────────────────────────────────────────────────────────────
// "Who can reach me" (step B, 2026-10-09, docs/design/blocking-and-safe-mode.md
// 10c): the words, the safe defaults and the rules, shared by the web chat
// client (web/chat/chat-privacy.js draws Settings > Safety from them,
// web/chat/crypto.js builds and opens contact requests with them) and by Node,
// where scripts/tests/reach-web.test.js holds them to the spec.
//
// One audience per kind of contact, narrowest first:
//   nobody  - no one
//   chosen  - holders of a pass from me whose `may` includes this kind
//   friends - holders of any pass from me
//   groups  - friends, plus people who share a P2P group with me
//   anyone  - everyone (strangers still spend the relay's daily knock budget)
// The relay enforces it (message at dm_put, call at voice_call, trade at
// trade_request) and its `reach_settings` is the source of truth for what the
// Safety page shows; the client applies the same rule to DMs that reach it
// anyway, showing them as a contact request with no text.
//
// A contact request is a sealed DM whose plaintext is exactly the smallest
// padding bucket (256 bytes, the first of DM_PAD_BUCKETS in src/net/dm_pq.rs
// and web/chat/crypto.js) and carries only the sender's name:
//   {"v":2,"contact_request":true,"name":"<registered name>","pad":"   ..."}
// The name follows the relay's own rule for registered names (letters, digits,
// underscore, dash, at most 24), so a request cannot carry a sentence or a link.
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load before crypto.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The kinds the relay enforces now, in the order the Safety page lists them.
  // (invite and voice_message stay valid words in a pass for later: 10c.)
  const REACH_KINDS = ['message', 'call', 'trade'];
  // Narrowest first. Must match the relay's list.
  const REACH_AUDIENCES = ['nobody', 'chosen', 'friends', 'groups', 'anyone'];
  // A person with no saved settings (10c): messages and trades from friends,
  // calls only from people they choose.
  const REACH_DEFAULTS = Object.freeze({ message: 'friends', call: 'chosen', trade: 'friends' });

  const REACH_KIND_LABELS = Object.freeze({ message: 'Messages', call: 'Calls', trade: 'Trades' });
  const REACH_AUDIENCE_LABELS = Object.freeze({
    nobody: 'Nobody',
    chosen: 'People I choose',
    friends: 'Friends',
    groups: 'Friends and people in my groups',
    anyone: 'Anyone',
  });

  // The sentence a refused sender sees (10c), and its trade twin.
  const REACH_REFUSED_MESSAGE = 'This person only accepts messages from people they know. You can send a contact request: they will see only your name.';
  const REACH_REFUSED_TRADE = 'This person only accepts trade requests from people they know.';

  // The smallest padding bucket: a contact request's sealed plaintext is exactly this long.
  const CONTACT_REQUEST_BYTES = 256;
  // The relay's rule for a registered name (src/relay/relay.rs, identify).
  const REACH_NAME_RE = /^[A-Za-z0-9_-]{1,24}$/;

  /** One line under a row of the Safety page saying what the choice means. */
  function reachExplain(kind, audience) {
    const verb = { message: 'message you', call: 'call you', trade: 'send you trade requests' }[kind];
    if (!verb) return '';
    switch (audience) {
      case 'nobody':
        return kind === 'message'
          ? 'No one can message you, and no one can send you a contact request.'
          : `No one can ${verb}.`;
      case 'chosen':
        return kind === 'call'
          ? 'Only the friends on your "People who may call me" list can call you.'
          : `Only friends you have allowed to ${verb} can.`;
      case 'friends':
        return kind === 'message'
          ? 'Only your friends can message you. Anyone else can send a contact request that shows you only their name.'
          : `Only your friends can ${verb}.`;
      case 'groups':
        return `Your friends and people who share a group with you can ${verb}.`;
      case 'anyone':
        return kind === 'message'
          ? 'Anyone can message you. People who are not your friends can send a limited number a day.'
          : `Anyone can ${verb}.`;
      default:
        return '';
    }
  }

  /**
   * The settings a `reach_settings` frame carries, with every kind filled in:
   * a kind missing or holding a word this client does not know shows its default.
   */
  function reachSettingsFrom(raw) {
    const out = {};
    for (const kind of REACH_KINDS) {
      const v = raw && typeof raw === 'object' ? raw[kind] : undefined;
      out[kind] = REACH_AUDIENCES.includes(v) ? v : REACH_DEFAULTS[kind];
    }
    return out;
  }

  /**
   * The `reach_set` frame for these changes (any subset of kinds), or null when
   * there is nothing to send or a kind or audience is unknown (the relay would
   * refuse the whole set; we do not send it).
   */
  function reachSetFrame(changes) {
    if (!changes || typeof changes !== 'object') return null;
    const settings = {};
    for (const [kind, audience] of Object.entries(changes)) {
      if (!REACH_KINDS.includes(kind) || !REACH_AUDIENCES.includes(audience)) return null;
      settings[kind] = audience;
    }
    if (Object.keys(settings).length === 0) return null;
    return { type: 'reach_set', settings };
  }

  /**
   * Would this audience let a person reach me for `kind`?
   *   passMay     - the `may` of the passes I gave them that still stand
   *                 (comma-joined), or null when I gave them none
   *   sharesGroup - do we share a P2P group
   */
  function reachAllows(audience, kind, who) {
    const passMay = who && typeof who.passMay === 'string' && who.passMay ? who.passMay : null;
    const sharesGroup = !!(who && who.sharesGroup);
    switch (audience) {
      case 'nobody': return false;
      case 'chosen': return !!passMay && passMay.split(',').includes(kind);
      case 'friends': return !!passMay;
      case 'groups': return !!passMay || sharesGroup;
      case 'anyone': return true;
      default: return false; // an unknown word reaches no one
    }
  }

  function utf8Len(s) {
    return new TextEncoder().encode(s).length;
  }

  /**
   * The sealed plaintext of a contact request from `name`: exactly
   * CONTACT_REQUEST_BYTES bytes of JSON, or null when the name is not a
   * registered-name word.
   */
  function contactRequestJson(name) {
    if (typeof name !== 'string' || !REACH_NAME_RE.test(name)) return null;
    const bare = { v: 2, contact_request: true, name, pad: '' };
    const fill = CONTACT_REQUEST_BYTES - utf8Len(JSON.stringify(bare));
    if (fill < 0) return null;
    bare.pad = ' '.repeat(fill);
    return JSON.stringify(bare);
  }

  /**
   * Read a contact request's plaintext: {name}, or null when it is not one. A
   * payload longer than the smallest bucket, or one whose name is not a
   * registered-name word, is not a contact request.
   */
  function contactRequestParse(json) {
    if (typeof json !== 'string' || utf8Len(json) > CONTACT_REQUEST_BYTES) return null;
    let v;
    try { v = JSON.parse(json); } catch { return null; }
    if (!v || v.v !== 2 || v.contact_request !== true) return null;
    if (typeof v.name !== 'string' || !REACH_NAME_RE.test(v.name)) return null;
    return { name: v.name };
  }

  const api = {
    REACH_KINDS, REACH_AUDIENCES, REACH_DEFAULTS, REACH_KIND_LABELS, REACH_AUDIENCE_LABELS,
    REACH_REFUSED_MESSAGE, REACH_REFUSED_TRADE, CONTACT_REQUEST_BYTES, REACH_NAME_RE,
    reachExplain, reachSettingsFrom, reachSetFrame, reachAllows, contactRequestJson, contactRequestParse,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
