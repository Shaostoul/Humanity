/* ── notify-rules.js ─────────────────────────────────────────────────────────
 * THE notification rule on the website (2026-10-30, docs/design/notifications.md,
 * section 4): should this message notify, and how. The desktop app's twin is
 * src/notifications/decide.rs; both read /data/notifications/notifications.json
 * and both answer the shared cases in scripts/tests/fixtures/notify-cases.json
 * the same way (scripts/tests/notify-rules-web.test.js).
 *
 * Pure: everything it reads is an argument. The chat (web/chat/chat-notify.js)
 * hands it what the browser keeps (the synced switches' copy, what a notice
 * shows, the mutes) and shows the browser notice it allows.
 *
 *   HosNotifyRules.use(file)                          the data file, once fetched
 *   HosNotifyRules.decide(c, synced, local, at)       {why, toast, sound}
 *   HosNotifyRules.words(c, show, t)                  [title, body], t = i18n.t
 *   HosNotifyRules.isMention(text, name)              '@name', the whole name
 *   HosNotifyRules.parseClock / inQuiet / formatClock quiet hours
 *   HosNotifyRules.active / muteKey                   mutes
 */
(function (root) {
  'use strict';

  var BURST_SECS = 5;
  var TITLE_CHARS = 80;
  var BODY_CHARS = 240;
  var file = { kinds: [], mute_for: [], sounds: [] };

  function use(f) {
    file = f && typeof f === 'object' ? f : file;
    return file;
  }

  function kind(id) {
    return (file.kinds || []).find(function (k) { return k.id === id; }) || null;
  }

  /** Is a mute ending at `until` (0: never) still on at `now`? */
  function active(until, now) {
    return until === 0 || until > now;
  }

  function muteKey(server, room) {
    return server + '|' + room;
  }

  /** "22", "22:00", "7:30" to minutes after midnight; null for anything else. */
  function parseClock(s) {
    if (typeof s !== 'string') return null;
    var m = /^(\d{1,2})(?::(\d{1,2}))?$/.exec(s.trim());
    if (!m) return null;
    var h = parseInt(m[1], 10);
    var mi = m[2] === undefined ? 0 : parseInt(m[2], 10);
    return h < 24 && mi < 60 ? h * 60 + mi : null;
  }

  function formatClock(minutes) {
    var m = ((minutes % 1440) + 1440) % 1440;
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return pad(Math.floor(m / 60)) + ':' + pad(m % 60);
  }

  /** Inside the window? An end before the start runs past midnight; equal ends are empty. */
  function inQuiet(start, end, minute) {
    if (start === end) return false;
    if (start < end) return minute >= start && minute < end;
    return minute >= start || minute < end;
  }

  function quietWindow(start, end) {
    if (start == null || end == null) return null;
    var a = parseClock(start), b = parseClock(end);
    return a === null || b === null ? null : [a, b];
  }

  var WORD_AFTER = /[\p{L}\p{N}_-]/u;

  /** '@' and the whole name, any case, not followed by a letter, digit, '_' or '-'. */
  function isMention(text, name) {
    name = String(name || '').trim();
    if (!name) return false;
    var hay = String(text || '').toLowerCase();
    var needle = '@' + name.toLowerCase();
    var from = 0;
    for (;;) {
      var i = hay.indexOf(needle, from);
      if (i < 0) return false;
      var next = String.fromCodePoint(hay.codePointAt(i + needle.length) || 32);
      if (i + needle.length >= hay.length || !WORD_AFTER.test(next)) return true;
      from = i + 1;
    }
  }

  function flagOn(synced, flag) {
    if (flag === 'dm') return !!synced.dm;
    if (flag === 'mentions') return !!synced.mentions;
    if (flag === 'tasks') return !!synced.tasks;
    return true;
  }

  function muted(mutes, scope, c, now) {
    mutes = mutes || {};
    if (mutes.everything !== null && mutes.everything !== undefined && active(mutes.everything, now)) return 'all_muted';
    if (scope !== 'room') return null;
    var s = (mutes.servers || {})[c.server];
    if (s !== undefined && active(s, now)) return 'server_muted';
    if (c.room) {
      var r = (mutes.rooms || {})[muteKey(c.server, c.room)];
      if (r !== undefined && active(r, now)) return 'room_muted';
    }
    return null;
  }

  function quiet(why) {
    return { why: why, toast: false, sound: false };
  }

  /** THE RULE, in the desktop's order. Times in Unix seconds. */
  function decide(c, synced, local, at) {
    if (c.from_me) return quiet('own_message');
    var k = kind(c.kind);
    if (!k) return quiet('unknown_kind');
    if (!synced.loaded) return quiet('not_loaded');
    if (!flagOn(synced, k.server_flag)) return quiet('kind_off');
    var m = muted(local.mutes, k.scope, c, at.now);
    if (m) return quiet(m);
    var w = quietWindow(synced.quiet_start, synced.quiet_end);
    if (w && inQuiet(w[0], w[1], at.minute)) return quiet('quiet_hours');
    if (at.focused && at.on_screen) return quiet('on_screen');
    if (at.last_notice != null && at.now >= at.last_notice && at.now - at.last_notice < BURST_SECS) return quiet('too_soon');
    var toast = !!local.desktop && !at.focused;
    var sound = !!local.sound;
    if (!toast && !sound) return quiet('nothing_on');
    return { why: 'notify', toast: toast, sound: sound };
  }

  /** Control characters and new lines become spaces, runs close up, cut to `max` characters. */
  function clean(s, max) {
    var out = Array.from(String(s || '').replace(/[\p{Cc}\s]+/gu, ' ').trim());
    if (out.length <= max) return out.join('');
    return out.slice(0, Math.max(0, max - 1)).join('').trimEnd() + '…';
  }

  /** A notice's [title, body] as `show` allows ('nothing' | 'name' | 'name_and_text'). */
  function words(c, show, t) {
    var k = kind(c.kind);
    if (!k) return ['', ''];
    var named = function () {
      return clean(t(k.title, { name: clean(c.sender, TITLE_CHARS), room: clean(c.room_name, TITLE_CHARS) }), TITLE_CHARS);
    };
    if (show === 'nothing') return [t(k.title_hidden), t(k.body_hidden)];
    if (show === 'name_and_text') {
      var body = clean(c.text, BODY_CHARS);
      return [named(), body || t(k.body_hidden)];
    }
    return [named(), t(k.body_hidden)];
  }

  var api = {
    BURST_SECS: BURST_SECS,
    use: use,
    kind: kind,
    active: active,
    muteKey: muteKey,
    parseClock: parseClock,
    formatClock: formatClock,
    inQuiet: inQuiet,
    isMention: isMention,
    decide: decide,
    words: words,
    clean: clean,
    file: function () { return file; }
  };
  root.HosNotifyRules = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
