// ── person-controls.js ────────────────────────────────────────────────────
// Per-person controls (2026-11-03, docs/design/per-person-controls.md): what a
// person chose for ONE other person on that person's profile card ("Your
// settings for <name>"). The desktop app's twin is src/net/person_controls.rs;
// both answer scripts/tests/fixtures/person-controls-cases.json the same way
// (scripts/tests/person-controls-web.test.js).
//
// Each control overrides a global setting, never copies it: with nothing
// chosen for anyone the chat behaves exactly as before.
//   notices   'all' (the default) | 'mentions' (only calls, mentions and
//             replies) | 'nothing'. THE notification rule reads it as the
//             candidate's from_level (/shared/notify-rules.js).
//   files     'mine' (the default: Files in messages decides) | 'ask' | 'up_to'
//             (files_mb, 1 to 1024, 10 until changed) | 'never'. fileVerdict
//             says what a file from them gets (chat-dms.js, chat-file-transfer.js).
//
// Kept in this browser by identity (localStorage hos_person_controls:
// {me: {their key: {notices, files, files_mb}}}), only what differs from the
// defaults. A choice names a person, so no server ever holds it and no sender is
// told it. Rung 2 of the design carries it between the person's devices.
//
// A classic script in the browser (HosPersonControls lands on window), a
// CommonJS module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  var KEY = 'hos_person_controls';
  var MB_DEFAULT = 10;
  var MB_MIN = 1;
  var MB_MAX = 1024;
  var NOTICES = ['all', 'mentions', 'nothing'];
  var FILES = ['mine', 'ask', 'up_to', 'never'];

  function lower(k) {
    return String(k || '').toLowerCase();
  }

  /** A record as kept: anything missing or unknown is its default. */
  function controlsFrom(raw) {
    var v = raw && typeof raw === 'object' ? raw : {};
    // A whole number of megabytes, as the desktop reads it; anything else is the default.
    var mb = Number.isInteger(v.files_mb) && v.files_mb >= 0 ? v.files_mb : MB_DEFAULT;
    return {
      notices: NOTICES.indexOf(v.notices) >= 0 ? v.notices : 'all',
      files: FILES.indexOf(v.files) >= 0 ? v.files : 'mine',
      files_mb: mb,
    };
  }

  /** "Up to N MB": N, kept in range. */
  function mb(c) {
    return Math.min(MB_MAX, Math.max(MB_MIN, controlsFrom(c).files_mb));
  }

  /** Nothing differs from the defaults (the size counts only while Up to is picked). */
  function isDefault(c) {
    var x = controlsFrom(c);
    return x.notices === 'all' && x.files === 'mine';
  }

  function readAll(storage) {
    var v = null;
    try { v = JSON.parse((storage && storage.getItem(KEY)) || '{}'); } catch (e) { v = null; }
    return v && typeof v === 'object' && !Array.isArray(v) ? v : {};
  }

  /** What identity `me` chose for `key` (the defaults when nothing). */
  function get(storage, me, key) {
    var mine = readAll(storage)[lower(me)];
    return controlsFrom(mine && typeof mine === 'object' ? mine[lower(key)] : null);
  }

  /** Keep `c` for `key` under `me`; the defaults are kept as no entry. Never for myself, nothing
   *  without both keys. False when this browser keeps nothing. */
  function set(storage, me, key, c) {
    me = lower(me);
    key = lower(key);
    if (!storage || !me || !key || me === key) return false;
    var all = readAll(storage);
    var mine = all[me] && typeof all[me] === 'object' ? all[me] : {};
    if (isDefault(c)) delete mine[key];
    else mine[key] = controlsFrom(c);
    if (Object.keys(mine).length) all[me] = mine;
    else delete all[me];
    try {
      storage.setItem(KEY, JSON.stringify(all));
      return true;
    } catch (e) {
      return false;
    }
  }

  /** The level THE rule is handed for a notice from `key`: null when nothing is chosen. */
  function levelOf(storage, me, key) {
    if (!key) return null;
    var n = get(storage, me, key).notices;
    return n === 'all' ? null : n;
  }

  /**
   * THE FILES RULE for one person: what a file of `size` bytes from them gets under `c`, as
   * {id, mb}: 'mine' (Files in messages decides), 'accept', 'ask_always', 'ask_over' (with the
   * limit it is over), 'never'. Under the protected setup "Up to N MB" only narrows.
   */
  function fileVerdict(c, size, protectedOn) {
    var x = controlsFrom(c);
    if (x.files === 'ask') return { id: 'ask_always' };
    if (x.files === 'never') return { id: 'never' };
    if (x.files !== 'up_to') return { id: 'mine' };
    var limit = mb(x);
    if (Number(size) > limit * 1024 * 1024) return { id: 'ask_over', mb: limit };
    return { id: protectedOn ? 'mine' : 'accept' };
  }

  /** The line a file from `name` shows under verdict `v` (t = i18n.t), or null. */
  function verdictLine(v, name, t) {
    var id = v && v.id;
    if (id === 'ask_always') return t('person.files_ask_line', { name: name });
    if (id === 'ask_over') return t('person.files_over_line', { name: name, mb: v.mb });
    if (id === 'never') return t('person.files_never_line', { name: name });
    return null;
  }

  var api = {
    KEY: KEY,
    MB_DEFAULT: MB_DEFAULT,
    MB_MIN: MB_MIN,
    MB_MAX: MB_MAX,
    NOTICES: NOTICES,
    FILES: FILES,
    controlsFrom: controlsFrom,
    mb: mb,
    isDefault: isDefault,
    get: get,
    set: set,
    levelOf: levelOf,
    fileVerdict: fileVerdict,
    verdictLine: verdictLine,
  };
  root.HosPersonControls = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
