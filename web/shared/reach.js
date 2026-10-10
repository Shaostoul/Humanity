// ── reach.js ──────────────────────────────────────────────────────────────
// "Who can reach me" (step B, 2026-10-09, docs/design/blocking-and-safe-mode.md
// 10c): the words, the safe defaults and the rules, shared by the web chat
// client (web/chat/chat-privacy.js draws Settings > Safety from them,
// web/chat/crypto.js builds and opens contact requests with them,
// web/chat/chat-social.js gives a friend the pass their "People I choose"
// ticks call for, 10c-ii) and by Node,
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
// A contact request (10c, as amended in review the same day) is an ordinary
// signed, sealed v2 DM deposited with `"contact_request": true`, whose text is
//   [[hum:contact-request:v1]]{"name":"<sender's registered name>","pass":"<pass JSON>"}
// where the pass is the sender's v2 friendship pass for the recipient with the
// default `may`: asking to connect is consenting to hear back, and the reply
// gets through the sender's relay gate because it carries that pass. The DM's
// own signature says who sent it; the recipient checks the pass and shows only
// the name its member list has for that key, never the claimed one.
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

  // "People I choose" (10c-ii, 2026-10-10): each friend once, with a tick per
  // kind, labelled in the singular. The ticks decide what the pass I give that
  // friend allows, and so who gets through a row set to "People I choose".
  const REACH_TICK_LABELS = Object.freeze({ message: 'Message', call: 'Call', trade: 'Trade' });
  // A friend with no saved choice: Message and Trade ticked, Call not, which is
  // exactly what a new pass carries (step A's FRIEND_PASS_DEFAULT_MAY).
  const REACH_TICK_DEFAULTS = Object.freeze({ message: true, call: false, trade: true });
  // What each tick puts in the pass. An invitation and a voice message are
  // forms of messaging, so they travel with Message (the relay enforces only
  // `message` today).
  const REACH_TICK_WORDS = Object.freeze({
    message: Object.freeze(['invite', 'message', 'voice_message']),
    call: Object.freeze(['call']),
    trade: Object.freeze(['trade']),
  });
  // The pass a friend keeps with all three ticks off. The pass format refuses
  // an empty `may` (FriendMay::from_words in src/relay/core/pq_crypto.rs, and
  // friendPassMay in /shared/friend-pass.js), and `invite` gives nothing the
  // relay enforces today, so it is the harmless word that keeps the pass valid.
  // Unticking everything does not end the friendship: under "Friends" this
  // friend still gets through.
  const REACH_EMPTY_MAY = Object.freeze(['invite']);
  // The sentences under the list (10c-ii): which rows use the ticks now.
  const REACH_TICKS_NOTE = 'These ticks count for a row set to People I choose.';
  const REACH_TICKS_UNUSED = 'Not in use now: no row is set to People I choose.';
  // For a row not set to "People I choose": who gets through instead.
  const REACH_THROUGH = Object.freeze({
    nobody: 'no one gets through',
    friends: 'every friend gets through',
    groups: 'every friend and everyone in your groups gets through',
    anyone: 'anyone gets through',
  });

  // The sentence a refused sender sees (10c), and its trade twin.
  const REACH_REFUSED_MESSAGE = 'This person only accepts messages from people they know. You can send a contact request: they will see only your name.';
  const REACH_REFUSED_TRADE = 'This person only accepts trade requests from people they know.';
  // A refused contact request (only "Nobody" refuses one; the relay marks it `request: true`,
  // 2026-10-10): the same words the desktop app shows, and no button to ask again.
  const REACH_NOT_TAKING_REQUESTS = 'They are not taking contact requests right now, so nothing was sent.';

  // The control marker a contact request's text starts with. Must match native.
  const CONTACT_REQUEST_MARKER = '[[hum:contact-request:v1]]';
  // The relay's rule for a registered name (src/relay/relay.rs, identify).
  const REACH_NAME_RE = /^[A-Za-z0-9_-]{1,24}$/;

  // The line under each row, by kind and audience: the desktop app's words exactly
  // (src/net/reach.rs `Audience::meaning`, the source of truth; the batch review of
  // 2026-10-10 found the two clients differing in 11 of these 15).
  // scripts/tests/reach-web.test.js reads them out of that Rust file and holds this
  // table to them. The three "People I choose" lines name the list the ticks are on
  // (10c-ii); a contact request still gets through under it (the relay refuses one
  // only under "Nobody"), so the Messages line says so.
  const REACH_EXPLAIN = Object.freeze({
    message: Object.freeze({
      nobody: 'No one can message you here, friends included, and contact requests are turned off.',
      chosen: 'Only the friends you tick for Message on your "People I choose" list can message you. Anyone else can send a contact request that shows you only their name.',
      friends: 'Your friends can message you. Anyone else can send a contact request with just their name.',
      groups: 'Friends and people who share a group with you can message you. Anyone else can send a contact request.',
      anyone: 'Anyone on this server can message you. People who are not your friends have a small daily limit.',
    }),
    call: Object.freeze({
      nobody: 'No one can call you, friends included.',
      chosen: 'Only the friends you tick for Call on your "People I choose" list can call you.',
      friends: 'Any of your friends can call you.',
      groups: 'Friends and people who share a group with you can call you.',
      anyone: 'Anyone on this server can call you.',
    }),
    trade: Object.freeze({
      nobody: 'No one can send you trade requests.',
      chosen: 'Only the friends you tick for Trade on your "People I choose" list can send you trade requests.',
      friends: 'Your friends can send you trade requests.',
      groups: 'Friends and people who share a group with you can send you trade requests.',
      anyone: 'Anyone on this server can send you trade requests. People who are not your friends have a small daily limit.',
    }),
  });

  /** One line under a row of the Safety page saying what the choice means ('' for a kind or audience it does not know). */
  function reachExplain(kind, audience) {
    const row = Object.prototype.hasOwnProperty.call(REACH_EXPLAIN, kind) ? REACH_EXPLAIN[kind] : null;
    return row && Object.prototype.hasOwnProperty.call(row, audience) ? row[audience] : '';
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

  /**
   * A friend's ticks on the "People I choose" list, read from the `may` of the
   * passes I gave them (comma-joined): the pass itself is the record (10c-ii),
   * so there is no second list to fall out of step with it. With no pass, the
   * defaults, which is what the next pass minted for them will carry.
   */
  function reachTicksFromMay(may) {
    if (typeof may !== 'string' || !may) return Object.assign({}, REACH_TICK_DEFAULTS);
    const words = may.split(',');
    const out = {};
    for (const kind of REACH_KINDS) out[kind] = words.includes(kind);
    return out;
  }

  /**
   * The `may` words (sorted, the canonical order) of the pass a friend with
   * these ticks holds: Message gives message, invite and voice_message; Call
   * gives call; Trade gives trade; a kind not given in `ticks` takes its
   * default. All three off gives `invite` alone (see REACH_EMPTY_MAY).
   */
  function reachMayFromTicks(ticks) {
    const words = [];
    for (const kind of REACH_KINDS) {
      const on = ticks && typeof ticks[kind] === 'boolean' ? ticks[kind] : REACH_TICK_DEFAULTS[kind];
      if (on) for (const w of REACH_TICK_WORDS[kind]) if (!words.includes(w)) words.push(w);
    }
    return (words.length ? words : REACH_EMPTY_MAY.slice()).sort();
  }

  /** "A", "A and B", "A, B and C". */
  function reachListWords(items) {
    if (items.length <= 1) return items.join('');
    return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
  }

  /**
   * The line under "These ticks count for a row set to People I choose."
   * (10c-ii): which rows use the ticks now, and for the rows that do not, who
   * gets through instead, in plain words. `settings` is {message, call, trade}
   * as the Safety page shows them (unknown words read as the defaults).
   */
  function reachTicksInUse(settings) {
    const s = reachSettingsFrom(settings);
    const used = REACH_KINDS.filter((k) => s[k] === 'chosen');
    if (used.length === 0) return REACH_TICKS_UNUSED;
    let line = `In use now: ${reachListWords(used.map((k) => REACH_KIND_LABELS[k]))}.`;
    // The other rows, grouped by their audience in the page's order, so two
    // rows on the same audience read as one sentence ("Messages and Trades
    // are set to Friends"). The row names are plural nouns, so "are" fits each.
    const groups = [];
    for (const kind of REACH_KINDS) {
      if (s[kind] === 'chosen') continue;
      let g = groups.find((x) => x.audience === s[kind]);
      if (!g) groups.push(g = { audience: s[kind], kinds: [] });
      g.kinds.push(kind);
    }
    for (const g of groups) {
      line += ` ${reachListWords(g.kinds.map((k) => REACH_KIND_LABELS[k]))} are set to ${REACH_AUDIENCE_LABELS[g.audience]},`
        + ` so ${REACH_THROUGH[g.audience]} for those.`;
    }
    return line;
  }

  /**
   * The text of a contact request from `name` carrying `passJson` (the
   * sender's pass for the recipient), or null when the name is not a
   * registered-name word or there is no pass.
   */
  function contactRequestText(name, passJson) {
    if (typeof name !== 'string' || !REACH_NAME_RE.test(name)) return null;
    if (typeof passJson !== 'string' || !passJson) return null;
    return CONTACT_REQUEST_MARKER + JSON.stringify({ name, pass: passJson });
  }

  /** Is this DM text a contact request (whatever follows the marker)? */
  function isContactRequestText(text) {
    return typeof text === 'string' && text.startsWith(CONTACT_REQUEST_MARKER);
  }

  /**
   * Read a contact request's text: {name, pass}, or null when it is not one or
   * its body is not the JSON object with both strings. The pass is not checked
   * here (crypto.js pqVerifyFriendCert does that) and the name is never shown.
   */
  function contactRequestParse(text) {
    if (!isContactRequestText(text)) return null;
    let v;
    try { v = JSON.parse(text.slice(CONTACT_REQUEST_MARKER.length)); } catch { return null; }
    if (!v || typeof v !== 'object' || typeof v.name !== 'string' || typeof v.pass !== 'string' || !v.pass) return null;
    return { name: v.name, pass: v.pass };
  }

  const api = {
    REACH_KINDS, REACH_AUDIENCES, REACH_DEFAULTS, REACH_KIND_LABELS, REACH_AUDIENCE_LABELS,
    REACH_REFUSED_MESSAGE, REACH_REFUSED_TRADE, REACH_NOT_TAKING_REQUESTS, CONTACT_REQUEST_MARKER, REACH_NAME_RE,
    REACH_TICK_LABELS, REACH_TICK_DEFAULTS, REACH_TICK_WORDS, REACH_EMPTY_MAY,
    REACH_TICKS_NOTE, REACH_TICKS_UNUSED, REACH_THROUGH,
    reachExplain, reachSettingsFrom, reachSetFrame, reachAllows,
    reachTicksFromMay, reachMayFromTicks, reachListWords, reachTicksInUse,
    contactRequestText, isContactRequestText, contactRequestParse,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
