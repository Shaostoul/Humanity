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
 *   HosNotifyRules.roomLevel(levels, server, room)    a room's level (rung 2)
 *   HosNotifyRules.soundOf / eventOn / gameEvent      a kind's sound, a game event (rung 3)
 *   HosNotifyRules.allowNoteText / allowNoteParse /   the always-allow list's note to oneself
 *     isAllowNote / allowApply                        and its newest-wins merge (rung 3)
 */
(function (root) {
  'use strict';

  var BURST_SECS = 5;
  var TITLE_CHARS = 80;
  var BODY_CHARS = 240;
  var file = { kinds: [], levels: [], mute_for: [], sounds: [] };

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
    if (scope === 'person') {
      var p = (mutes.people || {})[c.conversation];
      return p !== undefined && active(p, now) ? 'person_muted' : null;
    }
    if (scope !== 'room' && scope !== 'server') return null;
    var s = (mutes.servers || {})[c.server];
    if (s !== undefined && active(s, now)) return 'server_muted';
    // A server's own record (rung 3: a task) answers to its server's mute only.
    if (scope === 'server') return null;
    if (c.room) {
      var r = (mutes.rooms || {})[muteKey(c.server, c.room)];
      if (r !== undefined && active(r, now)) return 'room_muted';
    }
    return null;
  }

  /** A room's level (rung 2): nothing < mentions < all; anything unknown is mentions. */
  var LEVEL_RANK = { nothing: 0, mentions: 1, all: 2 };
  function levelOf(id) {
    return Object.prototype.hasOwnProperty.call(LEVEL_RANK, id) ? id : 'mentions';
  }

  /** What `room` on `server` is set to, from `levels` (`<server>|<room>` -> id). */
  function roomLevel(levels, server, room) {
    return room ? levelOf((levels || {})[muteKey(server, room)]) : 'mentions';
  }

  /** Does the synced exception `e` let a notice through quiet hours? */
  function excepts(synced, e, fromFriend) {
    if (e === 'friends') return !!synced.quiet_friends && !!fromFriend;
    if (e === 'mentions') return !!synced.quiet_mentions;
    return false;
  }

  function quiet(why) {
    return { why: why, toast: false, sound: false, list: false };
  }

  function gameEvent(id) {
    return (file.game_events || []).find(function (e) { return e.id === id; }) || null;
  }

  /** Is the game event `id` on: the person's choice (`events`), else the file's default (rung 3). */
  function eventOn(events, id) {
    var e = gameEvent(id);
    if (!e) return false;
    var v = (events || {})[id];
    return typeof v === 'boolean' ? v : !!e.on;
  }

  var NO_SOUND = 'none';

  /** The sound id kind `kindId` plays: the person's choice (`sounds`), else the kind's default;
   *  null when it is set to none (rung 3, per-kind sound). */
  function soundOf(sounds, kindId) {
    var k = kind(kindId);
    var v = (sounds || {})[kindId];
    if (typeof v !== 'string') v = k ? k.sound : '';
    return v && v !== NO_SOUND ? v : null;
  }

  // ── The always-allow list's note to oneself (rung 3; the desktop's notifications::allow) ──
  var ALLOW_MARKER = '[[hum:quiet-allow:v1]]';

  /** `[[hum:quiet-allow:v1]]<key>/on` or `/off`, the key in lower-case hex. */
  function allowNoteText(key, on) {
    return ALLOW_MARKER + String(key || '').toLowerCase() + '/' + (on ? 'on' : 'off');
  }

  /** {key, on}, or null for a text that is not a usable note. */
  function allowNoteParse(text) {
    if (typeof text !== 'string' || text.indexOf(ALLOW_MARKER) !== 0) return null;
    var rest = text.slice(ALLOW_MARKER.length);
    var slash = rest.indexOf('/');
    if (slash < 0) return null;
    var key = rest.slice(0, slash);
    var state = rest.slice(slash + 1);
    if (!key || key.length > 8192 || !/^[0-9a-f]+$/.test(key)) return null;
    if (state !== 'on' && state !== 'off') return null;
    return { key: key, on: state === 'on' };
  }

  function isAllowNote(text) {
    return typeof text === 'string' && text.indexOf(ALLOW_MARKER) === 0;
  }

  /** Take a note into `people` ({key: [on, at]}): only one newer than what is held counts. True
   *  when it changed who is on the list. */
  function allowApply(people, key, on, at) {
    key = String(key || '').toLowerCase();
    var held = people[key];
    if (held && held[1] >= at) return false;
    var changed = !held || held[0] !== on;
    people[key] = [on, at];
    return changed;
  }

  /** THE RULE, in the desktop's order. Times in Unix seconds. Whatever passes every check is
   *  listed in the in-app list (`list`), also with the window in front and with the notice and
   *  the sound both off; what a check stops is not. */
  function decide(c, synced, local, at) {
    if (c.from_me) return quiet('own_message');
    var k = kind(c.kind);
    if (!k) return quiet('unknown_kind');
    if (!synced.loaded) return quiet('not_loaded');
    if (!flagOn(synced, k.server_flag)) return quiet('kind_off');
    if (k.scope === 'game') {
      if (!gameEvent(c.event)) return quiet('unknown_kind');
      if (!eventOn(local.events, c.event)) return quiet('event_off');
    }
    var m = muted(local.mutes, k.scope, c, at.now);
    if (m) return quiet(m);
    if (k.scope === 'room') {
      var set = LEVEL_RANK[roomLevel(local.levels, c.server, c.room)];
      if (set < LEVEL_RANK[levelOf(k.level)]) return quiet('room_level');
    }
    var w = quietWindow(synced.quiet_start, synced.quiet_end);
    // The always-allow list lets a person's calls and messages through (rung 3, the operator's decision).
    var allowed = k.scope === 'person' && !!c.from_allowed;
    if (w && inQuiet(w[0], w[1], at.minute) && !excepts(synced, k.quiet_exception, c.from_friend) && !allowed) return quiet('quiet_hours');
    if (at.focused && at.on_screen) return quiet('on_screen');
    var burst = k.burst !== false;
    if (burst && at.last_notice != null && at.now >= at.last_notice && at.now - at.last_notice < BURST_SECS) return quiet('too_soon');
    var sound = !!local.sound && soundOf(local.sounds, k.id) !== null;
    return { why: 'notify', toast: !!local.desktop && !at.focused, sound: sound, list: true };
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
    var e = gameEvent(c.event);
    if (e && show !== 'nothing') {
      // A game event's own words (rung 3). The website raises none: its twin, for parity.
      var n = Math.max(1, c.count || 1);
      var body = t(e.body + (n === 1 ? '.one' : '.other'), { n: n, where: clean(c.room_name, TITLE_CHARS) });
      return [clean(t(e.title), TITLE_CHARS), clean(body, BODY_CHARS)];
    }
    var titleKey = (k.variants && c.variant && k.variants[c.variant]) || k.title;
    var named = function () {
      return clean(t(titleKey, { name: clean(c.sender, TITLE_CHARS), room: clean(c.room_name, TITLE_CHARS) }), TITLE_CHARS);
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
    roomLevel: roomLevel,
    isMention: isMention,
    decide: decide,
    words: words,
    clean: clean,
    gameEvent: gameEvent,
    eventOn: eventOn,
    soundOf: soundOf,
    NO_SOUND: NO_SOUND,
    allowNoteText: allowNoteText,
    allowNoteParse: allowNoteParse,
    isAllowNote: isAllowNote,
    allowApply: allowApply,
    file: function () { return file; }
  };
  root.HosNotifyRules = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
