// ── protected.js ──────────────────────────────────────────────────────────
// The protected setup (step G, 2026-10-10, docs/design/blocking-and-safe-mode.md
// 10h): a PIN lock on this device's safety settings that a parent or carer can
// apply for someone they look after, or anyone for themselves. It never checks
// age and never tells any server anything. Shared by the web chat client
// (web/chat/chat-protected.js draws the setup, the PIN prompt and the lines
// it shows; chat-privacy.js, chat-social.js, chat-groups-p2p.js,
// chat-voice-rooms.js, chat-warnings.js, chat-ui.js, chat-profile.js,
// chat-onboarding.js and app.js ask it which actions need the PIN) and by
// Node, where scripts/tests/protected-web.test.js holds it to the spec. The
// settings page (web/pages/settings-app.js) does not load it: it reads the
// same storage key itself and refuses the recovery phrase while the setup is on.
//
// What lives here:
//   - the preset reader: data/gui/safety_presets.json, entry `protected`, which
//     holds the values AND every word the feature puts on screen;
//   - the PIN verifier: PBKDF2-SHA-256, 600,000 iterations, a random 16-byte
//     salt, the same strength as the vaults; the PIN itself is never kept;
//   - the state as this device keeps it (localStorage, never synced, never in
//     any backup or export);
//   - the lock rules: which action needs the PIN, the public-rooms filter, the
//     pictures rule, the warnings audiences, the words test;
//   - the gate the chat scripts call before a locked action (protectedTake and
//     friends, at the bottom), which asks chat-protected.js for the PIN.
//
// Nothing here sends anything. Turning the setup on sends one ordinary
// `reach_set` (chat-protected.js), the same frame any adult can send; there is
// no "protected" flag in any frame, by design (6.1).
//
// A classic script in the browser (its names land on window), a CommonJS
// module under Node. Load after reach.js and before chat-privacy.js.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The reach words (reach.js): the page has them on window by now; Node asks for the file.
  const reachApi = (typeof module === 'object' && module && module.exports && typeof require === 'function')
    ? require('./reach.js')
    : root;

  const PROTECTED_PRESETS_URL = '/data/gui/safety_presets.json';
  const PROTECTED_PRESET_ID = 'protected';
  // Where this device keeps the setup. Deliberately outside the encrypted DM
  // store (whose block list travels to my other devices as self notes) and
  // outside every key list the backups and exports read, so it stays here.
  const PROTECTED_STORAGE_KEY = 'humanity_protected_setup';
  // The PIN verifier: the vaults' strength (CLAUDE.md, Vault encryption).
  const PROTECTED_PIN_KDF = 'PBKDF2-SHA-256';
  const PROTECTED_PIN_ITERATIONS = 600000;
  const PROTECTED_SALT_BYTES = 16;
  const PROTECTED_HASH_BITS = 256;
  // The right PIN unlocks ONE action, the one it was asked for: no time
  // window (the batch review, 2026-10-10, replaced a 10-second one, in which
  // a second action of the same kind, or the first one returning early before
  // it took the permission, ran without asking). See protectedAskThen.
  // A recovery phrase is 24 words; anything much shorter is not one.
  const PROTECTED_PHRASE_MIN_WORDS = 12;

  // ── The lock rules (10h) ──
  // While the setup is on, these need the PIN. Each name is one place in the
  // chat that asks: a "Who can reach me" row, a "People I choose" tick, making
  // a friend in any way (Follow, Follow back, accepting or sending a contact
  // request), a friend code (making one, or redeeming one, by button or by the
  // typed /friend-code and /redeem commands), joining a group by ticket,
  // starting a group (`start_group`), making or copying a group's invite
  // ticket (`group_invite`: each one lets someone in), joining a voice room,
  // turning warnings off, turning the pictures rule down, showing public rooms,
  // changing the PIN, turning the setup off, and showing the recovery phrase
  // (`show_phrase`), as the desktop app does: whoever has the phrase can set a
  // new PIN through "Forgot the PIN?", so the person the setup protects must
  // not be able to read it off this device. The same name covers every copy of
  // the identity the phrase comes from (a backup file, the device-link code),
  // because the phrase can be read back out of each.
  const PROTECTED_LOCKED_ACTIONS = Object.freeze([
    'reach_row', 'reach_tick', 'befriend', 'friend_code', 'join_group', 'start_group', 'group_invite',
    'join_voice_room',
    'warnings_off', 'pictures_down', 'show_public_rooms', 'change_pin', 'turn_off', 'show_phrase',
  ]);
  // Never locked: anything that reduces who can reach this device. A person
  // must always be able to get away from someone, PIN or no PIN.
  const PROTECTED_NEVER_LOCKED = Object.freeze([
    'block', 'report', 'unfollow', 'leave_group', 'leave_voice_room', 'mute',
    'ignore_request', 'warnings_on', 'pictures_up', 'hide_public_rooms',
  ]);

  // The safe values for a rule a stored state is missing (the preset's own).
  const PROTECTED_SAFE_RULES = Object.freeze({
    pin_digits_min: 4,
    pin_digits_max: 12,
    pin_wrong_tries_before_wait: 3,
    pin_wait_seconds: 60,
    warnings_on_friends: true,
    pictures_from_non_friends: 'never',
    public_rooms: 'read_only_only',
  });
  const PROTECTED_PICTURE_RULES = Object.freeze(['never', 'click']);
  const PROTECTED_ROOM_RULES = Object.freeze(['read_only_only', 'all']);

  // ── Words on screen that the preset file does not hold ──
  // The sentences are all in data/gui/safety_presets.json. These are the short
  // button and field labels the steps need that the file has no field for
  // (reported to the coordinator, 10h); a `labels` object in the preset, if
  // one is added, takes over each of them without a code change.
  const PROTECTED_LABELS = Object.freeze({
    continue: 'Continue',
    cancel: 'Cancel',
    remove: 'Remove',
    forgot: 'Forgot the PIN?',
    pin: 'PIN',
    pin_again: 'The same PIN again',
    pin_rule: 'Use {min} to {max} digits, the same both times.',
    pin_wrong: 'That PIN is not right.',
    pin_wait: 'Too many wrong tries. Try again in {seconds} seconds.',
    friends: 'Friends',
    groups: 'Groups',
    rooms: 'Voice rooms',
    none: 'None.',
    turn_off: 'Turn off the protected setup',
    change_pin: 'Change the PIN',
    show_rooms: 'Show public rooms',
    hide_rooms: 'Hide public rooms',
    pictures: 'Pictures and files from people who are not friends',
    pictures_never: 'Not shown',
    pictures_click: 'Shown after a click',
    phrase: 'Recovery phrase',
    phrase_wrong: 'That is not the recovery phrase of this identity.',
    show_phrase: 'Show the recovery phrase',
    phrase_needs_pin: 'While the protected setup is on, showing the recovery phrase or saving a backup of this identity needs the PIN. In the chat, open Safety and choose Show the recovery phrase.',
    waiting_store: 'Waiting for your settings on this device to load.',
    not_connected: 'Not connected, so the setting was not changed.',
  });

  // ── The preset ──

  const PRESET_TEXT_FIELDS = Object.freeze([
    'name', 'button', 'summary', 'review_intro', 'review_keep', 'status_line', 'routes_line',
    'public_rooms_hidden_line', 'public_rooms_explain', 'picture_hidden_line', 'accept_needs_pin',
    'forgot_pin_explain',
  ]);

  /**
   * The `protected` entry of the presets file, checked: every word field a
   * string, the sentences and the words to avoid lists of strings, the reach
   * values words the relay knows, the PIN rules sensible numbers. Null when
   * the file is not that shape (then the setup cannot be turned on: it has
   * nothing to say, and it never says something the file does not hold).
   */
  function protectedPresetFrom(data) {
    const list = data && Array.isArray(data.presets) ? data.presets : null;
    if (!list) return null;
    const p = list.find((x) => x && x.id === PROTECTED_PRESET_ID);
    if (!p) return null;
    for (const f of PRESET_TEXT_FIELDS) if (typeof p[f] !== 'string' || !p[f]) return null;
    if (!Array.isArray(p.sentences) || !p.sentences.length || !p.sentences.every((s) => typeof s === 'string' && s)) return null;
    if (!Array.isArray(p.avoid_words) || !p.avoid_words.every((s) => typeof s === 'string' && s)) return null;
    if (!reachApi.reachSetFrame || !reachApi.reachSetFrame(p.reach)) return null;
    for (const k of reachApi.REACH_KINDS) if (!(k in p.reach)) return null;
    const ints = ['pin_digits_min', 'pin_digits_max', 'pin_wrong_tries_before_wait', 'pin_wait_seconds'];
    for (const f of ints) if (!Number.isInteger(p[f]) || p[f] < 1) return null;
    if (p.pin_digits_max < p.pin_digits_min) return null;
    if (!PROTECTED_PICTURE_RULES.includes(p.pictures_from_non_friends)) return null;
    if (!PROTECTED_ROOM_RULES.includes(p.public_rooms)) return null;
    if (typeof p.warnings_on_friends !== 'boolean') return null;
    return p;
  }

  /** A label: the preset's own when it has one (a later `labels` object), else this file's. */
  function protectedLabel(preset, key, fill) {
    const own = preset && preset.labels && typeof preset.labels[key] === 'string' ? preset.labels[key] : null;
    let s = own || PROTECTED_LABELS[key] || '';
    if (fill) for (const [k, v] of Object.entries(fill)) s = s.split('{' + k + '}').join(String(v));
    return s;
  }

  /** The rules a new setup keeps, copied from the preset so they hold even when the file cannot be read later. */
  function protectedRulesFrom(preset) {
    const out = {};
    for (const k of Object.keys(PROTECTED_SAFE_RULES)) out[k] = preset && k in preset ? preset[k] : PROTECTED_SAFE_RULES[k];
    return out;
  }

  /** The one frame turning the setup on sends: an ordinary `reach_set` with the preset's values. */
  function protectedReachFrame(preset) {
    return preset && preset.reach ? reachApi.reachSetFrame(preset.reach) : null;
  }

  /** Every sentence and line from the preset that this feature can put on screen. */
  function protectedPresetStrings(preset) {
    if (!preset) return [];
    return PRESET_TEXT_FIELDS.map((f) => preset[f]).concat(preset.sentences);
  }

  // ── The words test (10h, the finding's section 6) ──

  /**
   * Each word or phrase from `avoidWords` that appears in any of `texts`,
   * letter case aside: [{text, word}]. Matched as a plain substring, so
   * "monitor" also catches "monitoring" and "verified" catches "unverified".
   */
  function protectedAvoidHits(texts, avoidWords) {
    const hits = [];
    const words = (avoidWords || []).map((w) => String(w).toLowerCase()).filter(Boolean);
    for (const t of texts || []) {
      const low = String(t == null ? '' : t).toLowerCase();
      for (const w of words) if (low.includes(w)) hits.push({ text: String(t), word: w });
    }
    return hits;
  }

  // ── The PIN ──

  /** Is this a PIN the rules allow: digits only (0 to 9), within the length? */
  function protectedPinOk(pin, rules) {
    const r = rules || PROTECTED_SAFE_RULES;
    if (typeof pin !== 'string' || !/^[0-9]+$/.test(pin)) return false;
    return pin.length >= r.pin_digits_min && pin.length <= r.pin_digits_max;
  }

  function bytesToB64(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return typeof btoa === 'function' ? btoa(s) : Buffer.from(s, 'latin1').toString('base64');
  }

  function b64ToBytes(b64) {
    const s = typeof atob === 'function' ? atob(b64) : Buffer.from(b64, 'base64').toString('latin1');
    const out = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
  }

  function cryptoOf(c) {
    const x = c || root.crypto || (typeof globalThis !== 'undefined' ? globalThis.crypto : null);
    if (!x || !x.subtle) throw new Error('WebCrypto is not available');
    return x;
  }

  async function pinBits(pin, salt, iterations, c) {
    const subtle = cryptoOf(c).subtle;
    const key = await subtle.importKey('raw', new TextEncoder().encode(pin), 'PBKDF2', false, ['deriveBits']);
    const bits = await subtle.deriveBits({ name: 'PBKDF2', salt, iterations, hash: 'SHA-256' }, key, PROTECTED_HASH_BITS);
    return new Uint8Array(bits);
  }

  /**
   * The verifier kept for a PIN: {kdf, iterations, salt, hash}, salt and hash
   * in base64. The PIN is not in it and cannot be read back out of it; checking
   * one costs 600,000 rounds of SHA-256, so trying every PIN takes a long time
   * even for someone who copied the file.
   */
  async function protectedMakeVerifier(pin, c) {
    const salt = cryptoOf(c).getRandomValues(new Uint8Array(PROTECTED_SALT_BYTES));
    const hash = await pinBits(pin, salt, PROTECTED_PIN_ITERATIONS, c);
    return { kdf: PROTECTED_PIN_KDF, iterations: PROTECTED_PIN_ITERATIONS, salt: bytesToB64(salt), hash: bytesToB64(hash) };
  }

  /** Is this a verifier this file made (the right algorithm, count and sizes)? */
  function protectedVerifierOk(v) {
    if (!v || typeof v !== 'object') return false;
    if (v.kdf !== PROTECTED_PIN_KDF || v.iterations !== PROTECTED_PIN_ITERATIONS) return false;
    if (typeof v.salt !== 'string' || typeof v.hash !== 'string') return false;
    try {
      return b64ToBytes(v.salt).length === PROTECTED_SALT_BYTES && b64ToBytes(v.hash).length === PROTECTED_HASH_BITS / 8;
    } catch { return false; }
  }

  /** Does `pin` match the verifier? Compared in constant time; false for a verifier that is not one. */
  async function protectedPinMatches(pin, verifier, c) {
    if (typeof pin !== 'string' || !pin || !protectedVerifierOk(verifier)) return false;
    const want = b64ToBytes(verifier.hash);
    const got = await pinBits(pin, b64ToBytes(verifier.salt), verifier.iterations, c);
    if (got.length !== want.length) return false;
    let diff = 0;
    for (let i = 0; i < want.length; i++) diff |= got[i] ^ want[i];
    return diff === 0;
  }

  // ── The state this device keeps ──
  //   {v: 1, on: true, pin: verifier, identity: key, rules: {...}, pictures,
  //    public_rooms, approved: [keys], wrong, wait_until}
  // `identity` is the public key of the identity the setup was turned on under
  // (the desktop app keeps the same): "Forgot the PIN?" takes only THAT
  // identity's recovery phrase, so making a new identity on this device (and
  // so knowing its phrase) does not open the lock. A missing or damaged one,
  // which only damage to what storage holds can leave (turning the setup on
  // always records one), falls back to the identity in use, so the recovery
  // phrase can still open a damaged setup rather than it locking for good
  // (protectedIdentityMatches; the desktop app's rule, 10h as built).
  // `approved` is everyone the PIN holder let be a friend: the friends kept in
  // the review step, and each friend made with the PIN since. A pass is given
  // only to them (chat-social.js sendFriendCertTo), so no route makes a friend
  // without the PIN, even a follow made before the setup that is followed back
  // after it. `wrong` and `wait_until` are kept too, so reloading the page does
  // not reset the wait after three wrong PINs.
  // `warnings_off` is the PIN holder's choice to turn "Warnings on messages"
  // off (with the PIN), kept here on the device like the desktop app's one
  // switch: while the setup is on it alone decides, never the per-identity
  // switch in the local store, so restoring an identity whose warnings were
  // off does not turn them off (the batch review, 2026-10-10).

  function normKey(k) {
    return typeof k === 'string' ? k.trim().toLowerCase() : '';
  }

  /** An identity key as the state keeps it: lower-case hex, or '' for anything that is not one. */
  function protectedIdentityKey(k) {
    const n = normKey(k);
    return /^[0-9a-f]+$/.test(n) ? n : '';
  }

  /**
   * The state from what storage holds: null when the setup is off (nothing
   * kept). Anything kept that cannot be read counts as ON with no PIN that
   * matches (fail closed): only the recovery phrase can then set a new PIN.
   */
  function protectedStateParse(raw) {
    if (raw == null || raw === '') return null;
    let v = null;
    try { v = JSON.parse(raw); } catch { v = null; }
    if (v && typeof v === 'object' && v.on === false) return null;
    const src = v && typeof v === 'object' ? v : {};
    const rules = {};
    const r = src.rules && typeof src.rules === 'object' ? src.rules : {};
    for (const [k, safe] of Object.entries(PROTECTED_SAFE_RULES)) {
      rules[k] = typeof r[k] === typeof safe ? r[k] : safe;
    }
    if (!Number.isInteger(rules.pin_digits_min) || rules.pin_digits_min < 1) rules.pin_digits_min = PROTECTED_SAFE_RULES.pin_digits_min;
    if (!Number.isInteger(rules.pin_digits_max) || rules.pin_digits_max < rules.pin_digits_min) rules.pin_digits_max = Math.max(rules.pin_digits_min, PROTECTED_SAFE_RULES.pin_digits_max);
    if (!Number.isInteger(rules.pin_wrong_tries_before_wait) || rules.pin_wrong_tries_before_wait < 1) rules.pin_wrong_tries_before_wait = PROTECTED_SAFE_RULES.pin_wrong_tries_before_wait;
    if (!Number.isInteger(rules.pin_wait_seconds) || rules.pin_wait_seconds < 1) rules.pin_wait_seconds = PROTECTED_SAFE_RULES.pin_wait_seconds;
    if (!PROTECTED_PICTURE_RULES.includes(rules.pictures_from_non_friends)) rules.pictures_from_non_friends = PROTECTED_SAFE_RULES.pictures_from_non_friends;
    if (!PROTECTED_ROOM_RULES.includes(rules.public_rooms)) rules.public_rooms = PROTECTED_SAFE_RULES.public_rooms;
    return {
      v: 1,
      on: true,
      pin: protectedVerifierOk(src.pin) ? src.pin : null,
      identity: protectedIdentityKey(src.identity),
      rules,
      pictures: PROTECTED_PICTURE_RULES.includes(src.pictures) ? src.pictures : rules.pictures_from_non_friends,
      public_rooms: PROTECTED_ROOM_RULES.includes(src.public_rooms) ? src.public_rooms : rules.public_rooms,
      approved: Array.isArray(src.approved) ? Array.from(new Set(src.approved.map(normKey).filter(Boolean))) : [],
      wrong: Number.isInteger(src.wrong) && src.wrong > 0 ? src.wrong : 0,
      wait_until: Number.isFinite(src.wait_until) && src.wait_until > 0 ? src.wait_until : 0,
      warnings_off: src.warnings_off === true,
    };
  }

  /** A new setup's state: the verifier, the identity it is turned on under, the preset's rules, the friends kept. */
  function protectedStateNew(preset, verifier, approved, identity) {
    const rules = protectedRulesFrom(preset);
    return protectedStateParse(JSON.stringify({
      v: 1, on: true, pin: verifier, identity: protectedIdentityKey(identity), rules,
      pictures: rules.pictures_from_non_friends, public_rooms: rules.public_rooms,
      approved: approved || [], wrong: 0, wait_until: 0, warnings_off: false,
    }));
  }

  function protectedStateRead(storage) {
    try { return protectedStateParse(storage ? storage.getItem(PROTECTED_STORAGE_KEY) : null); }
    catch { return null; }
  }

  function protectedStateWrite(storage, state) {
    if (!storage) return false;
    try {
      if (!state) storage.removeItem(PROTECTED_STORAGE_KEY);
      else storage.setItem(PROTECTED_STORAGE_KEY, JSON.stringify(state));
      return true;
    } catch { return false; }
  }

  /** Milliseconds left before the next PIN may be tried (0 when it may be tried now). */
  function protectedWaitLeftMs(state, now) {
    if (!state || !state.wait_until) return 0;
    return Math.max(0, state.wait_until - now);
  }

  /**
   * The state after a wrong PIN: one more wrong in a row, and at the limit
   * (three) a wait (60 seconds) before the next try; the count starts again
   * after the wait.
   */
  function protectedAfterWrong(state, now) {
    const s = Object.assign({}, state);
    const wrong = (Number(s.wrong) || 0) + 1;
    if (wrong >= s.rules.pin_wrong_tries_before_wait) {
      s.wrong = 0;
      s.wait_until = now + s.rules.pin_wait_seconds * 1000;
    } else {
      s.wrong = wrong;
    }
    return s;
  }

  /** The state after the right PIN: the wrong count starts again. */
  function protectedAfterRight(state) {
    return Object.assign({}, state, { wrong: 0, wait_until: 0 });
  }

  // ── The lock rules ──

  /** Does this action need the PIN now? Only while the setup is on; an unknown action is locked (fail closed). */
  function protectedActionLocked(action, state) {
    if (!state || !state.on) return false;
    if (PROTECTED_NEVER_LOCKED.includes(action)) return false;
    return true;
  }

  function protectedIsApproved(state, peer) {
    const k = normKey(peer);
    return !!(state && k && state.approved.includes(k));
  }

  /**
   * A typed chat command that makes a friend, which the relay would act on
   * as typed (src/relay/relay.rs: the first word, any letter case):
   * `/friend-code` makes a code, `/redeem <code>` uses one. Returns
   * {action: 'friend_code', command: 'friend-code'|'redeem', code} or null
   * for anything else. The chat sends these through the same gated paths
   * as its buttons, so typing one never gets round the PIN.
   */
  function protectedTypedCommand(text) {
    const words = String(text == null ? '' : text).trim().split(/\s+/);
    const first = (words[0] || '').toLowerCase();
    if (first === '/friend-code') return { action: 'friend_code', command: 'friend-code', code: '' };
    if (first === '/redeem') return { action: 'friend_code', command: 'redeem', code: words[1] || '' };
    return null;
  }

  /** Is this channel listed? With the setup on, only read-only rooms are, unless the PIN showed them. */
  function protectedChannelShown(channel, state) {
    if (!state || !state.on || state.public_rooms === 'all') return true;
    return !!(channel && channel.read_only === true);
  }

  /** The channels to list and how many were hidden: {shown, hidden}. */
  function protectedChannelsShown(channels, state) {
    const list = Array.isArray(channels) ? channels : [];
    const shown = list.filter((c) => protectedChannelShown(c, state));
    return { shown, hidden: list.length - shown.length };
  }

  /** Are pictures and files from this sender not shown? Never my own, never a friend's. */
  function protectedHidesPictures(state, who) {
    if (!state || !state.on || state.pictures !== 'never') return false;
    return !(who && (who.isMe || who.isFriend));
  }

  /**
   * Is "Warnings on messages" on? With the setup on, its own `warnings_off`
   * decides (only the PIN sets it), whatever `identitySwitch` (the per-identity
   * switch in the local store) says; while it is off, that switch does.
   */
  function protectedWarningsOn(state, identitySwitch) {
    if (state && state.on) return state.warnings_off !== true;
    return identitySwitch !== false;
  }

  /**
   * Which warning audiences a message from this sender is checked against
   * (10g): strangers' entries for a stranger, friends' for a friend, and with
   * the setup on, a friend's message gets the strangers' entries too.
   */
  function protectedWarningAudiences(isFriend, state) {
    if (!isFriend) return ['strangers'];
    if (state && state.on && state.rules.warnings_on_friends) return ['friends', 'strangers'];
    return ['friends'];
  }

  /**
   * A message body's HTML with every picture and file taken out and one line
   * in place of each (10h): the image placeholders, audio and video players and
   * file cards that app.js formatBody makes. The line is escaped here.
   */
  function protectedHidePicturesHtml(html, line) {
    const esc = String(line == null ? '' : line).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
    const put = `<span class="protected-picture-hidden" style="display:block;color:var(--text-muted);font-size:var(--text-sm);">${esc}</span>`;
    // The placeholder holds its icon (an <svg>, or an empty <span> for an icon
    // name the page does not know) and then its words, so the pattern names
    // that shape rather than stopping at the first </span>.
    return String(html == null ? '' : html)
      .replace(/<span class="img-placeholder"[^>]*>(?:<svg[\s\S]*?<\/svg>|<span[^>]*><\/span>)?[^<]*<\/span>/g, put)
      .replace(/<audio\b[^>]*>[\s\S]*?<\/audio>/gi, put)
      .replace(/<video\b[^>]*>[\s\S]*?<\/video>/gi, put)
      .replace(/<div class="file-card">[\s\S]*?<\/a><\/div>/g, put);
  }

  // ── Forgot the PIN: the recovery phrase ──

  /** A phrase as words: lower-cased, split at anything not a letter or digit, numbers on their own left out (a numbered list). */
  function protectedPhraseWords(text) {
    return String(text == null ? '' : text).toLowerCase()
      .split(/[^\p{Alphabetic}\p{N}]+/u)
      .filter((w) => w && !/^\p{N}+$/u.test(w));
  }

  /**
   * Is `key` the identity the setup was turned on under? False while it is off
   * and for a key that is not one. When the state holds no identity, or a
   * damaged one, the identity in use counts as it (see below), so its phrase
   * can still open "Forgot the PIN?".
   */
  function protectedIdentityMatches(state, key) {
    const want = state ? protectedIdentityKey(state.identity) : '';
    const got = protectedIdentityKey(key);
    if (!state || !got) return false;
    // A missing or malformed recorded identity, which only damage to what storage holds can
    // leave (turning the setup on always records one), falls back to the identity in use, so
    // the recovery phrase can still open a damaged setup instead of it locking for good; a
    // recorded identity that differs never matches (the desktop app's rule, 10h as built).
    return !want || want === got;
  }

  /** Is `typed` this identity's phrase (`words`, as derived on this device), word for word? */
  function protectedPhraseMatches(typed, words) {
    if (!Array.isArray(words) || words.length < PROTECTED_PHRASE_MIN_WORDS) return false;
    const want = words.map((w) => String(w).toLowerCase());
    const got = protectedPhraseWords(typed);
    return got.length === want.length && got.every((w, i) => w === want[i]);
  }

  // ── The gate the chat scripts call (browser) ──
  // The state is read from this device's storage on every check, so a change
  // made in another tab counts at once.
  //
  // One PIN, one action (the batch review, 2026-10-10). The right PIN given
  // for `action` holds a grant, in memory only, for the run that asked for it
  // (protectedAskThen's `fn`), which takes it once with protectedTake (or
  // protectedBefriendAllowed) for that same action. The grant is cleared when
  // that run returns, whatever way it returns: early before taking it, with a
  // value, through a promise that settles, or by throwing. It used to last 10
  // seconds, so an action that returned before taking it left the PIN open
  // for the next action of the same kind, unasked.

  let grantHeld = null; // {action}, while the run the PIN was given for is going

  function storage() {
    try { return root.localStorage || null; } catch { return null; }
  }

  /** The setup as this device keeps it now; null when it is off. */
  function protectedCurrent() {
    return protectedStateRead(storage());
  }

  /** Is the setup on? */
  function protectedIsOn() {
    return !!protectedCurrent();
  }

  /** Keep a changed state (the setup stays on). */
  function protectedSave(state) {
    return protectedStateWrite(storage(), state);
  }

  function grantFor(action) {
    return !!(grantHeld && grantHeld.action === action);
  }

  /**
   * May `action` run now? True when the setup is off, the action is never
   * locked, or the PIN was just given for exactly this action and the run it
   * was given for is going (the grant is used up). A locked action that gets
   * false must not run; it asks with protectedAskThen.
   */
  function protectedTake(action) {
    if (!protectedActionLocked(action, protectedCurrent())) return true;
    if (grantFor(action)) { grantHeld = null; return true; }
    return false;
  }

  /** Does `action` need the PIN asked for now (the setup locks it, and no grant for it is held)? */
  function protectedNeedsPin(action) {
    return protectedActionLocked(action, protectedCurrent()) && !grantFor(action);
  }

  /**
   * Ask for the PIN for `action` (chat-protected.js protectedAskPin draws the
   * prompt). When the prompt cannot be drawn, the answer is no: a lock that
   * cannot ask stays locked.
   */
  async function askPin(action) {
    const ask = root.protectedAskPin;
    if (typeof ask !== 'function') return false;
    try { return !!(await ask(action)); } catch { return false; }
  }

  /**
   * Ask for the PIN for `action`, then run `fn`, which takes the grant
   * (protectedTake or protectedBefriendAllowed). Resolves to what `fn`
   * returns, or false when the PIN was not given. The grant is gone once
   * `fn` returns or throws, taken or not. While the setup is off, or for an
   * action it never locks, `fn` just runs.
   */
  async function protectedAskThen(action, fn) {
    if (!protectedActionLocked(action, protectedCurrent())) return fn();
    if (!(await askPin(action))) return false;
    const grant = { action };
    grantHeld = grant;
    try {
      return await fn();
    } finally {
      if (grantHeld === grant) grantHeld = null;
    }
  }

  /**
   * May I make `peer` a friend now (Follow, Follow back, accepting or sending a
   * contact request)? With the setup on: when the PIN holder already let them
   * be one, or the PIN was just given, which records that they may.
   */
  function protectedBefriendAllowed(peer) {
    const state = protectedCurrent();
    if (!protectedActionLocked('befriend', state)) return true;
    if (protectedIsApproved(state, peer)) return true;
    if (grantFor('befriend')) {
      grantHeld = null;
      protectedApprove(peer);
      return true;
    }
    return false;
  }

  /** May this device give `peer` a pass (become friends at the relay)? With the setup on, only someone the PIN holder let be a friend. */
  function protectedPassAllowed(peer) {
    const state = protectedCurrent();
    return !state || protectedIsApproved(state, peer);
  }

  /** Record that the PIN holder lets `peer` be a friend (no effect while the setup is off). */
  function protectedApprove(peer) {
    const state = protectedCurrent();
    const k = normKey(peer);
    if (!state || !k || state.approved.includes(k)) return false;
    state.approved.push(k);
    return protectedSave(state);
  }

  /** `peer` is no longer a friend (Unfollow, Block): a new friendship needs the PIN again. */
  function protectedForget(peer) {
    const state = protectedCurrent();
    const k = normKey(peer);
    if (!state || !k || !state.approved.includes(k)) return false;
    state.approved = state.approved.filter((x) => x !== k);
    return protectedSave(state);
  }

  const api = {
    PROTECTED_PRESETS_URL, PROTECTED_PRESET_ID, PROTECTED_STORAGE_KEY,
    PROTECTED_PIN_KDF, PROTECTED_PIN_ITERATIONS, PROTECTED_SALT_BYTES, PROTECTED_HASH_BITS,
    PROTECTED_LOCKED_ACTIONS, PROTECTED_NEVER_LOCKED, PROTECTED_SAFE_RULES,
    PROTECTED_LABELS,
    protectedPresetFrom, protectedLabel, protectedRulesFrom, protectedReachFrame, protectedPresetStrings,
    protectedAvoidHits, protectedPinOk, protectedMakeVerifier, protectedVerifierOk, protectedPinMatches,
    protectedStateParse, protectedStateNew, protectedStateRead, protectedStateWrite,
    protectedWaitLeftMs, protectedAfterWrong, protectedAfterRight,
    protectedActionLocked, protectedIsApproved, protectedTypedCommand, protectedChannelShown, protectedChannelsShown,
    protectedHidesPictures, protectedWarningsOn, protectedWarningAudiences, protectedHidePicturesHtml,
    protectedPhraseWords, protectedPhraseMatches, protectedIdentityKey, protectedIdentityMatches,
    protectedCurrent, protectedIsOn, protectedSave, protectedTake, protectedNeedsPin,
    protectedAskThen, protectedBefriendAllowed, protectedPassAllowed, protectedApprove, protectedForget,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
