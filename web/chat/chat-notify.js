/* ── chat-notify.js ──────────────────────────────────────────────────────────
 * The chat's notifications (2026-10-30, docs/design/notifications.md), the
 * desktop app's engine/notify.rs, chat/mute_menu.rs and gui/notice_panel.rs
 * mirrored:
 *
 * - HosChatNotify.decide(c, at): THE rule (/shared/notify-rules.js) with the
 *   person's synced switches, quiet hours and quiet-hours exceptions AS THEIR
 *   SERVER HOLDS THEM (asked once the socket is accepted, prefsData; nothing
 *   notifies until it answered), what a notice shows (humanity_settings
 *   ['notif-show'], "name" by default: who it is from, never what was said),
 *   the mutes (hos_notify_mutes) and each room's level (hos_notify_levels).
 *   chat-ui.js notifyNewMessage acts on the answer.
 * - The in-app list (rung 2): whatever the rule lets through is listed, also
 *   with the tab in front; the bell in the chat page's header shows how many
 *   are not opened yet and opens the last 100, each opening its conversation.
 *   Kept in this tab's memory only, never written anywhere (rung 4 adds an
 *   encrypted store). Seen means the panel was opened.
 * - Mute server: the bell on this server's row in the sidebar, opening the
 *   lengths from /data/notifications/notifications.json (1 hour, 8 hours,
 *   until I unmute), or Unmute while muted. A server's mute stops notices from
 *   its rooms, never a direct message, and never hides a message or a dot.
 * - Mute room and the room's level (rung 2): the bell on a room's row, or a
 *   right-click on it; Mute conversation: the bell on a DM's row, or a
 *   right-click on it. The same lengths, the same rule.
 * - Rung 3: a task notice (onTaskNotice, the relay's task_notice), a call's
 *   notice and its repeating ring (ring / ringEnded, from chat-voice-calls.js),
 *   each kind's own sound (playSound, hos_notify_sounds), and the always-allow
 *   list (allowed / setAllowed / screenAllowNote, hos_quiet_allow), carried to
 *   my other devices as notes to myself.
 *
 * Every word by key from data/i18n/en.json.
 */
(function () {
  'use strict';

  var MUTES_KEY = 'hos_notify_mutes';
  var LEVELS_KEY = 'hos_notify_levels';
  var NAMES_KEY = 'hos_notify_names';
  var LIST_KEPT = 100;
  // A shared room's line older than this is a link catching up, not a live line (the desktop's
  // engine/notify.rs FEDERATED_LIVE_MS).
  var FEDERATED_LIVE_MS = 5 * 60 * 1000;
  var lastNotice = {};

  function T(key, vars) {
    return window.i18n ? window.i18n.t(key, vars) : key;
  }

  function nowSecs() {
    return Math.floor(Date.now() / 1000);
  }

  function rules() {
    return window.HosNotifyRules;
  }

  function esc(s) {
    return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
  }

  function rerender() {
    if (typeof renderServerList === 'function') renderServerList();
    if (typeof renderDmList === 'function') renderDmList();
  }

  if (typeof fetch === 'function') {
    fetch('/data/notifications/notifications.json', { cache: 'no-cache' })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (j) { if (j && rules()) rules().use(j); rerender(); })
      .catch(function () { /* no data file: nothing notifies, the bells offer no lengths */ });
  }

  /** This server, as the mutes name it. */
  function server() {
    return location.origin;
  }

  function loadJson(key) {
    try {
      var v = JSON.parse(localStorage.getItem(key));
      return v && typeof v === 'object' ? v : null;
    } catch (_) {
      return null;
    }
  }

  function saveJson(key, v) {
    try { localStorage.setItem(key, JSON.stringify(v)); } catch (_) { /* private window: this page only */ }
  }

  function loadMutes() {
    var m = loadJson(MUTES_KEY);
    if (m) {
      return { everything: m.everything === undefined ? null : m.everything, servers: m.servers || {}, rooms: m.rooms || {}, people: m.people || {} };
    }
    return { everything: null, servers: {}, rooms: {}, people: {} };
  }

  function saveMutes(m) {
    saveJson(MUTES_KEY, m);
  }

  /** Each room set to something other than mentions, `<server>|<room>` -> 'all' | 'nothing'. */
  function loadLevels() {
    return loadJson(LEVELS_KEY) || {};
  }

  function stored(key) {
    try { return localStorage.getItem(key); } catch (_) { return null; }
  }

  // ── The person's synced switches, as their server holds them (2026-10-30) ──
  // Asked of the server once this page's socket is accepted, and taken live whenever the server
  // sends them (its answer to a change made on any of the person's devices). Until the first
  // answer nothing notifies (the desktop's Why::NotLoaded) and the DM cog writes nothing: this
  // browser's own copy may be from long ago, or only the defaults, and must never be sent back
  // over what the person chose elsewhere. A change is sent as just the switch that changed; the
  // relay keeps every switch a message leaves out.
  var held = null;
  var PENDING_KEY = 'hos_notify_pending';
  var SWITCHES = { dm: 'boolean', mentions: 'boolean', tasks: 'boolean', dnd_start: 'clock', dnd_end: 'clock', quiet_friends: 'boolean', quiet_mentions: 'boolean' };

  function sock() {
    return typeof ws !== 'undefined' && ws && ws.readyState === 1 ? ws : null;
  }

  function ask() {
    var s = sock();
    if (s) s.send(JSON.stringify({ type: 'get_notification_prefs' }));
  }

  function keep(key, value) {
    try {
      if (value === null) localStorage.removeItem(key);
      else localStorage.setItem(key, value);
    } catch (_) { /* private window: this page only */ }
  }

  /** This browser's copy, for the Settings page to show. */
  function mirror() {
    keep('hos_notify_dms', held.dm ? 'true' : 'false');
    keep('hos_notify_mentions', held.mentions ? 'true' : 'false');
    keep('hos_notify_tasks', held.tasks ? 'true' : 'false');
    keep('hos_dnd_start', held.dnd_start);
    keep('hos_dnd_end', held.dnd_end);
    keep('hos_quiet_friends', held.quiet_friends ? 'true' : 'false');
    keep('hos_quiet_mentions', held.quiet_mentions ? 'true' : 'false');
  }

  /** The server's `notification_prefs_data`: the whole stored set. The exceptions are off unless
   *  the server says on (an exception lets more through quiet hours). */
  function prefsData(msg) {
    var clock = function (v) { return typeof v === 'string' && v ? v : null; };
    held = {
      dm: msg.dm !== false,
      mentions: msg.mentions !== false,
      tasks: msg.tasks !== false,
      dnd_start: clock(msg.dnd_start),
      dnd_end: clock(msg.dnd_end),
      quiet_friends: msg.quiet_friends === true,
      quiet_mentions: msg.quiet_mentions === true
    };
    mirror();
  }

  /** A change the Settings page queued (it has no socket): sent as it is, switches only. */
  function flushPending() {
    var s = sock();
    var raw = stored(PENDING_KEY);
    if (!s || raw === null) return false;
    var change = {};
    try { change = JSON.parse(raw) || {}; } catch (_) { change = {}; }
    var msg = { type: 'update_notification_prefs' };
    var any = false;
    Object.keys(SWITCHES).forEach(function (k) {
      if (!Object.prototype.hasOwnProperty.call(change, k)) return;
      var v = change[k];
      var ok = SWITCHES[k] === 'boolean' ? typeof v === 'boolean' : v === null || typeof v === 'string';
      if (ok) { msg[k] = v; any = true; }
    });
    keep(PENDING_KEY, null);
    if (any) s.send(JSON.stringify(msg));
    return any;
  }

  /** This socket's identity was accepted: send a queued change (the server answers with the
   *  whole set), else ask for the set. */
  function onIdentified() {
    if (!flushPending()) ask();
  }

  /** The DMs section's cog: the DM switch, flipped from what the server holds. */
  function toggleDm() {
    if (!held) {
      ask();
      if (typeof addNotice === 'function') addNotice(T('notifications.dm_wait'), 'orange', 4);
      return;
    }
    var s = sock();
    if (!s) {
      if (typeof addNotice === 'function') addNotice(T('notifications.not_connected'), 'orange', 4);
      return;
    }
    var on = !held.dm;
    s.send(JSON.stringify({ type: 'update_notification_prefs', dm: on }));
    held.dm = on;
    mirror();
    if (typeof addNotice === 'function') addNotice(T(on ? 'notifications.dm_on' : 'notifications.dm_off'), on ? 'green' : 'orange', 4);
  }

  if (window.hos && typeof window.hos.on === 'function') window.hos.on('socket-identified', onIdentified);
  // Settings open in another tab queued a change: send it now, while this socket is accepted.
  if (typeof window.addEventListener === 'function') {
    window.addEventListener('storage', function (e) { if (e && e.key === PENDING_KEY && held) flushPending(); });
  }

  function synced() {
    if (!held) return { dm: false, mentions: false, tasks: false, quiet_start: null, quiet_end: null, quiet_friends: false, quiet_mentions: false, loaded: false };
    return {
      dm: held.dm,
      mentions: held.mentions,
      tasks: held.tasks,
      quiet_start: held.dnd_start,
      quiet_end: held.dnd_end,
      quiet_friends: held.quiet_friends,
      quiet_mentions: held.quiet_mentions,
      loaded: true
    };
  }

  function show() {
    try {
      return (JSON.parse(stored('humanity_settings')) || {})['notif-show'] || 'name';
    } catch (_) {
      return 'name';
    }
  }

  /** c: {kind, from_me, server, room, conversation, sender, room_name, text, from_friend}; at:
   *  {focused, on_screen, desktop, sound}. Answers {why, toast, sound, list, title, body, tag}. */
  function decide(c, at) {
    var R = rules();
    if (!R) return { why: 'unknown_kind', toast: false, sound: false, list: false };
    var now = nowSecs();
    var d = new Date();
    // Rung 3: each kind's sound (Settings), and the always-allow list for a person's notice.
    var sounds = loadSounds();
    var local = { desktop: !!at.desktop, sound: !!at.sound, show: show(), mutes: loadMutes(), levels: loadLevels(), sounds: sounds, events: {} };
    if (c.from_allowed === undefined && c.conversation && c.conversation.indexOf('dm:') === 0) c.from_allowed = allowed(c.conversation.slice(3));
    // The sender's level from my profile card for them (per-person controls): a DM's
    // conversation names them, a room line carries their key (`from_key`).
    if (c.from_level === undefined) c.from_level = c.from_me ? null : personLevel(c.from_key || (c.conversation && c.conversation.indexOf('dm:') === 0 ? c.conversation.slice(3) : ''));
    var out = R.decide(c, synced(), local, {
      now: now,
      minute: d.getHours() * 60 + d.getMinutes(),
      focused: !!at.focused,
      on_screen: !!at.on_screen,
      last_notice: lastNotice[c.conversation] === undefined ? null : lastNotice[c.conversation]
    });
    if (out.why !== 'notify') return out;
    // A kind without the burst guard (a ring) keeps no time for it, so a DM right after is its own.
    var k = R.kind(c.kind);
    if (!k || k.burst !== false) lastNotice[c.conversation] = now;
    if (out.list) listAdd(c, local.show, now);
    var w = R.words(c, local.show, T);
    return { why: out.why, toast: out.toast, sound: out.sound, list: out.list, title: w[0], body: w[1], tag: 'hos-' + tagFor(c.conversation), sound_id: out.sound ? R.soundOf(sounds, c.kind) : null };
  }

  // ── Each kind's sound (rung 3): the data file's sounds, the files the desktop plays ──
  var SOUNDS_KEY = 'hos_notify_sounds';

  /** The kinds whose sound the person chose here, kind -> sound id or 'none'. */
  function loadSounds() {
    return loadJson(SOUNDS_KEY) || {};
  }

  /** Play the sound `id` (an entry of the data file's `sounds`) from /assets/, or the chat's own
   *  chime where this browser cannot play the file. */
  function playSound(id) {
    var R = rules();
    var s = R && id ? (R.file().sounds || []).find(function (x) { return x.id === id; }) : null;
    var chime = function () { if (typeof playNotificationChime === 'function') playNotificationChime(); };
    if (!s || typeof Audio !== 'function') { chime(); return; }
    // At the person's level for it (Settings > Audio & Video > Sound levels, /shared/sound-levels.js).
    if (window.HosSoundLevels) { window.HosSoundLevels.play(s.path, s.id, null, chime); return; }
    try {
      var a = new Audio('/assets/' + s.path);
      a.volume = 0.6;
      var p = a.play();
      if (p && typeof p.catch === 'function') p.catch(chime);
    } catch (_) { chime(); }
  }

  // ── The always-allow list (rung 3; the desktop's notifications::allow and engine/quiet_allow) ──
  // The people whose calls and messages come through quiet hours, nobody until the person adds
  // them. Kept in this browser by identity (hos_quiet_allow: {me: {key: [on, signed ms]}}), and
  // carried to my other devices as notes to myself, never told to a server.
  var ALLOW_KEY = 'hos_quiet_allow';
  var ALLOW_UNSENT_KEY = 'hos_quiet_allow_unsent';
  var ALLOW_NAMES_KEY = 'hos_quiet_allow_names';

  function me() {
    return typeof myKey === 'string' ? myKey : '';
  }

  function allowPeople() {
    var all = loadJson(ALLOW_KEY) || {};
    return all[me()] || {};
  }

  function saveAllowPeople(people) {
    var all = loadJson(ALLOW_KEY) || {};
    all[me()] = people;
    saveJson(ALLOW_KEY, all);
  }

  /** The level I set for `key` on their profile card ("Notifications from them",
   *  /shared/person-controls.js), or null: THE rule's from_level. */
  function personLevel(key) {
    if (!key || !window.HosPersonControls) return null;
    var storage = null;
    try { storage = window.localStorage; } catch (_) { storage = null; }
    return HosPersonControls.levelOf(storage, me(), key);
  }

  /** Is `key` on my always-allow list? */
  function allowed(key) {
    var e = allowPeople()[String(key || '').toLowerCase()];
    return !!(e && e[0]);
  }

  /** Send the notes to myself that have not gone yet (one at a time, in order). */
  var allowSending = Promise.resolve();
  function flushAllowNotes() {
    allowSending = allowSending.then(sendAllowNotes, sendAllowNotes);
    return allowSending;
  }

  async function sendAllowNotes() {
    var waiting = loadJson(ALLOW_UNSENT_KEY);
    if (!Array.isArray(waiting) || !waiting.length) return;
    if (!sock() || typeof pqBuildSelfNote !== 'function' || !me()) return;
    // Only this identity's notes (the Settings page names whose list it changed); others wait.
    var others = waiting.filter(function (n) { return n.owner && n.owner !== me(); });
    var mine = waiting.filter(function (n) { return !n.owner || n.owner === me(); });
    while (mine.length) {
      var built = await pqBuildSelfNote(mine[0].text, mine[0].at);
      if (!built || !sock()) break; // tried again on the next connection
      // At once: the allow-list note pairs with nothing and stays kept as unsent until it has
      // gone (shared/self-mail.js, BUG-259).
      if (typeof hosSelfMailSendNow === 'function') hosSelfMailSendNow(sock(), JSON.stringify(built.put));
      else sock().send(JSON.stringify(built.put));
      mine.shift();
      saveJson(ALLOW_UNSENT_KEY, others.concat(mine));
    }
  }

  /** Put `key` on my list (`on`) or take them off: here at once, then a note to my other devices. */
  function setAllowed(key, on, name) {
    var R = rules();
    key = String(key || '').toLowerCase();
    if (!R || !key || !me() || key === me().toLowerCase()) return;
    var at = Date.now();
    var people = allowPeople();
    R.allowApply(people, key, !!on, at);
    saveAllowPeople(people);
    var names = loadJson(ALLOW_NAMES_KEY) || {};
    if (on) names[key] = name || key.slice(0, 8);
    else delete names[key];
    saveJson(ALLOW_NAMES_KEY, names);
    var waiting = loadJson(ALLOW_UNSENT_KEY);
    waiting = Array.isArray(waiting) ? waiting : [];
    waiting.push({ owner: me(), text: R.allowNoteText(key, !!on), at: at });
    saveJson(ALLOW_UNSENT_KEY, waiting);
    flushAllowNotes();
    if (typeof addNotice === 'function') addNotice(T(on ? 'notifications.allow_person' : 'notifications.unallow_person'), 'green', 4);
    rerender();
  }

  /** An opened, signature-checked DM (chat-privacy.js blockScreenDm): true when it is a note about
   *  the list, which is never stored or shown; one from me to me is applied, the newest winning. */
  function screenAllowNote(inner) {
    var R = rules();
    if (!R || !inner || !R.isAllowNote(inner.text)) return false;
    if (inner.from !== me() || inner.to !== me()) return true;
    var n = R.allowNoteParse(inner.text);
    if (n) {
      var people = allowPeople();
      R.allowApply(people, n.key, n.on, Number(inner.ts) || 0);
      saveAllowPeople(people);
    }
    return true;
  }

  if (window.hos && typeof window.hos.on === 'function') window.hos.on('socket-identified', function () { flushAllowNotes(); });
  // The Settings page (no socket) took someone off the list: send its note now.
  if (typeof window.addEventListener === 'function') {
    window.addEventListener('storage', function (e) { if (e && e.key === ALLOW_UNSENT_KEY) flushAllowNotes(); });
  }

  // ── A ringing call (rung 3): THE rule's notice, its sound repeating while it rings ──
  var RING_EVERY_MS = 3000;
  var RING_FOR_MS = 45000;
  var ringing = null;

  /** A ring that rings here (chat-voice-calls.js, idle and not ignored). */
  function ring(key, name, focused, canShow) {
    ringEnded();
    var conv = 'dm:' + key;
    var d = decide({ kind: 'call', from_me: false, server: server(), room: null, conversation: conv, sender: name, room_name: '', text: '', from_friend: false, from_allowed: allowed(key) },
      { focused: focused, on_screen: false, desktop: canShow, sound: typeof soundEnabled === 'undefined' ? true : !!soundEnabled });
    if (d.why !== 'notify') return d;
    var r = { key: key, notice: null, timer: null, until: Date.now() + RING_FOR_MS };
    if (d.sound_id) {
      playSound(d.sound_id);
      r.timer = setInterval(function () {
        if (Date.now() >= r.until) { ringEnded(); return; }
        playSound(d.sound_id);
      }, RING_EVERY_MS);
    }
    if (d.toast && canShow && typeof Notification === 'function') {
      r.notice = new Notification(d.title, { body: d.body, icon: '/favicon.png', tag: d.tag, requireInteraction: false });
      // A click answers nothing: it brings the tab forward, where the ringing screen is.
      r.notice.onclick = function () { window.focus(); r.notice.close(); openEntry({ kind: 'call', conversation: conv, room: null, sender: name }); };
    }
    ringing = r;
    return d;
  }

  /** The ring ended, however it ended: its sound stops and its notice is withdrawn. */
  function ringEnded() {
    if (!ringing) return;
    if (ringing.timer) clearInterval(ringing.timer);
    if (ringing.notice) { try { ringing.notice.close(); } catch (_) { /* gone already */ } }
    ringing = null;
  }

  // ── A task given to me, or one of mine changed (rung 3): the relay's task_notice ──
  function onTaskNotice(msg, focused, canShow) {
    if (!msg || typeof msg.task_id !== 'number') return null;
    var byKey = String(msg.by_key || '');
    if (byKey && typeof isBlockedKey === 'function' && isBlockedKey(byKey)) return null;
    var c = { kind: 'task', from_me: !!byKey && byKey === me(), server: server(), room: null, conversation: 'task:' + msg.task_id,
      sender: String(msg.by_name || ''), room_name: '', text: String(msg.title || ''), from_friend: false, variant: msg.change === 'assigned' ? 'assigned' : '' };
    var d = decide(c, { focused: focused, on_screen: false, desktop: canShow, sound: typeof soundEnabled === 'undefined' ? true : !!soundEnabled });
    if (d.why !== 'notify') return d;
    if (d.sound_id) playSound(d.sound_id);
    if (d.toast && canShow && typeof Notification === 'function') {
      var n = new Notification(d.title, { body: d.body, icon: '/favicon.png', tag: d.tag });
      n.onclick = function () { window.focus(); n.close(); openEntry(c); };
    }
    return d;
  }

  /** A short tag per conversation (a later notice replaces the earlier one): a hash, never a name. */
  function tagFor(s) {
    var h = 0x811c9dc5;
    for (var i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
    return ('0000000' + h.toString(16)).slice(-8);
  }

  // ── The in-app list (rung 2): newest first, at most LIST_KEPT, this tab's memory only ──
  var list = [];
  var unseen = 0;
  var panelOpen = false;

  /** Add the notice `c` (what was said kept only when the person's setting shows it); a later
   *  notice for a conversation whose entry is not seen yet folds into that entry. */
  function listAdd(c, showChoice, now) {
    var text = showChoice === 'name_and_text' && c.text ? String(c.text) : null;
    for (var i = 0; i < list.length; i++) {
      var e = list[i];
      if (!e.seen && e.kind === c.kind && e.server === c.server && e.conversation === c.conversation) {
        list.splice(i, 1);
        e.sender = c.sender;
        e.room_name = c.room_name;
        e.text = text;
        e.at = now;
        e.variant = c.variant || '';
        list.unshift(e);
        refreshList();
        return;
      }
    }
    list.unshift({ kind: c.kind, server: c.server, room: c.room || null, conversation: c.conversation, sender: c.sender, room_name: c.room_name || '', text: text, at: now, seen: panelOpen, variant: c.variant || '' });
    if (!panelOpen) unseen++;
    while (list.length > LIST_KEPT) {
      var old = list.pop();
      if (!old.seen) unseen--;
    }
    refreshList();
  }

  function markSeen() {
    list.forEach(function (e) { e.seen = true; });
    unseen = 0;
  }

  function refreshList() {
    renderBell();
    if (panelOpen) renderPanel();
  }

  var BELL = '<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.3" aria-hidden="true"><path d="M3.2 11.6h9.6M4 11.4c.6-1 .7-2.4.7-4.1a3.3 3.3 0 016.6 0c0 1.7.1 3.1.7 4.1"/><circle cx="8" cy="13.6" r="1" fill="currentColor" stroke="none"/>';
  var SLASH = '<line x1="2" y1="2" x2="14" y2="14"/>';

  function bellSvg(muted, size) {
    var svg = size ? BELL.replace('width="12" height="12"', 'width="' + size + '" height="' + size + '"') : BELL;
    return svg + (muted ? SLASH : '') + '</svg>';
  }

  /** The header bell's count pill text: none, the number, then 99+ (the desktop's count_text). */
  function countText(n) {
    if (n <= 0) return '';
    return n > 99 ? '99+' : String(n);
  }

  function renderBell() {
    var b = document.getElementById('notice-bell');
    if (!b) return;
    var n = countText(unseen);
    b.classList.toggle('open', panelOpen);
    b.setAttribute('aria-expanded', panelOpen ? 'true' : 'false');
    b.setAttribute('aria-label', n ? T('notifications.bell') + ': ' + T('notifications.bell_count', { count: n }) : T('notifications.bell'));
    b.innerHTML = bellSvg(false, 16) + (n ? '<span class="notice-count">' + esc(n) + '</span>' : '');
  }

  function clockOf(secs) {
    var d = new Date(secs * 1000);
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return pad(d.getHours()) + ':' + pad(d.getMinutes());
  }

  function renderPanel() {
    var p = document.getElementById('notice-panel');
    var R = rules();
    if (!p || !R) return;
    p.innerHTML = '';
    var head = document.createElement('div');
    head.className = 'notice-panel-head';
    var h = document.createElement('strong');
    h.textContent = T('notifications.panel_heading');
    head.appendChild(h);
    if (list.length) {
      var clear = document.createElement('button');
      clear.type = 'button';
      clear.className = 'notice-clear';
      clear.textContent = T('notifications.panel_clear');
      if (window.i18n) window.i18n.hint(clear, 'notifications.panel_clear_hint');
      clear.addEventListener('click', function (e) { e.stopPropagation(); list = []; unseen = 0; refreshList(); });
      head.appendChild(clear);
    }
    p.appendChild(head);
    var note = document.createElement('div');
    note.className = 'notice-note';
    note.textContent = T('notifications.panel_note_web');
    p.appendChild(note);
    if (!list.length) {
      var empty = document.createElement('div');
      empty.className = 'notice-empty';
      empty.textContent = T('notifications.panel_empty');
      p.appendChild(empty);
      return;
    }
    var showChoice = show();
    list.forEach(function (e) {
      var row = document.createElement('button');
      row.type = 'button';
      row.className = 'notice-entry';
      if (window.i18n) window.i18n.hint(row, 'notifications.entry_hint');
      var c = { kind: e.kind, sender: e.sender, room_name: e.room_name, text: e.text || '', variant: e.variant || '' };
      var title = document.createElement('span');
      title.className = 'notice-title';
      title.textContent = R.words(c, showChoice, T)[0];
      row.appendChild(title);
      if (showChoice === 'name_and_text' && e.text) {
        var body = document.createElement('span');
        body.className = 'notice-body';
        body.textContent = R.clean(e.text, 120);
        row.appendChild(body);
      }
      var when = document.createElement('span');
      when.className = 'notice-when';
      when.textContent = clockOf(e.at);
      row.appendChild(when);
      row.addEventListener('click', function (ev) { ev.stopPropagation(); closePanel(); openEntry(e); });
      p.appendChild(row);
    });
  }

  /** Open the conversation an entry came from: the DM, or the room (this tab is one server). */
  function openEntry(e) {
    // A task (rung 3): the Tasks page, with that task open.
    if (e.conversation.indexOf('task:') === 0) {
      var id = parseInt(e.conversation.slice(5), 10);
      if (id > 0) location.href = '/tasks?task=' + id;
      return;
    }
    if (e.conversation.indexOf('dm:') === 0) {
      var key = e.conversation.slice(3);
      if (key && typeof openDmConversation === 'function') openDmConversation(key, e.kind === 'dm' ? e.sender : key.slice(0, 8));
    } else if (e.room && typeof switchChannel === 'function') {
      if (typeof closeDmView === 'function' && typeof activeDmPartner !== 'undefined' && activeDmPartner) closeDmView();
      switchChannel(e.room);
    }
  }

  function placePanel() {
    var p = document.getElementById('notice-panel');
    var b = document.getElementById('notice-bell');
    if (!p || !b || !b.getBoundingClientRect) return;
    var r = b.getBoundingClientRect();
    p.style.top = Math.round(r.bottom + 4) + 'px';
    p.style.right = Math.max(8, Math.round((window.innerWidth || 0) - r.right)) + 'px';
  }

  function openPanel() {
    var p = document.getElementById('notice-panel');
    if (!p) return;
    panelOpen = true;
    markSeen();
    p.hidden = false;
    renderPanel();
    placePanel();
    renderBell();
    setTimeout(function () { document.addEventListener('click', outsidePanel, true); }, 0);
  }

  function closePanel() {
    var p = document.getElementById('notice-panel');
    panelOpen = false;
    if (p) p.hidden = true;
    document.removeEventListener('click', outsidePanel, true);
    renderBell();
  }

  function outsidePanel(e) {
    var p = document.getElementById('notice-panel');
    var b = document.getElementById('notice-bell');
    if (p && !p.contains(e.target) && !(b && b.contains(e.target))) closePanel();
  }

  /** The bell in the chat page's header, left of the sound menu's, and its panel. */
  function wireHeaderBell() {
    if (document.getElementById('notice-bell')) return;
    var before = document.getElementById('sound-toggle');
    if (!before || !before.parentNode) return;
    var b = document.createElement('button');
    b.type = 'button';
    b.id = 'notice-bell';
    b.className = 'notice-bell';
    b.setAttribute('aria-haspopup', 'dialog');
    if (window.i18n) window.i18n.hint(b, 'notifications.bell_hint');
    b.addEventListener('click', function (e) { e.stopPropagation(); if (panelOpen) closePanel(); else openPanel(); });
    before.parentNode.insertBefore(b, before);
    var p = document.createElement('div');
    p.id = 'notice-panel';
    p.className = 'notice-panel';
    p.setAttribute('role', 'dialog');
    p.setAttribute('aria-label', T('notifications.panel_heading'));
    p.hidden = true;
    document.body.appendChild(p);
    renderBell();
  }

  // ── Mutes and levels ──

  function untilWords(until) {
    if (until === 0) return T('notifications.muted_until_unmuted');
    var d = new Date(until * 1000);
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return T('notifications.muted_until', { time: pad(d.getHours()) + ':' + pad(d.getMinutes()) });
  }

  /** Until when the mute at `map[key]` lasts, or null. */
  function activeUntil(map, key) {
    var R = rules();
    var u = (map || {})[key];
    return R && u !== undefined && u !== null && R.active(u, nowSecs()) ? u : null;
  }

  function serverMuted() {
    return activeUntil(loadMutes().servers, server());
  }

  function roomMuted(room) {
    return activeUntil(loadMutes().rooms, server() + '|' + room);
  }

  function personMuted(conversation) {
    return activeUntil(loadMutes().people, conversation);
  }

  /** Set (`seconds`: 0 for until unmuted) or clear (null) the mute in `field` ('servers',
   *  'rooms', 'people') at `key`. (Mute everything is the Settings page's, settings-app.js.) */
  function setMute(field, key, seconds) {
    var m = loadMutes();
    if (seconds === null) delete m[field][key];
    else m[field][key] = seconds === 0 ? 0 : nowSecs() + seconds;
    saveMutes(m);
    var u = activeUntil(m[field], key);
    var msg = u === null ? T('notifications.unmuted_notice') : T('notifications.muted_notice', { until: untilWords(u) });
    if (typeof addNotice === 'function') addNotice(msg, u === null ? 'green' : 'orange', 4);
    rerender();
  }

  function setServerMute(seconds) { setMute('servers', server(), seconds); }

  /** A room's level: 'all', 'mentions' (kept as no entry) or 'nothing'. */
  function setRoomLevel(room, level) {
    var levels = loadLevels();
    var key = server() + '|' + room;
    if (level === 'mentions') delete levels[key];
    else levels[key] = level;
    saveJson(LEVELS_KEY, levels);
    var choice = ((rules() && rules().file().levels) || []).find(function (l) { return l.id === level; });
    if (choice && typeof addNotice === 'function') addNotice(T('notifications.level_now', { level: T(choice.label) }), 'green', 4);
    rerender();
  }

  /** A bell for a row: a bell, crossed out while muted (the desktop's mark). */
  function rowBell(action, attr, value, until, hintKey, labelKey, cls) {
    var tip = until === null ? T(hintKey) : T('notifications.muted_mark_hint', { until: untilWords(until) });
    return '<span class="' + cls + (until === null ? '' : ' muted') + '" data-action="' + action + '" ' + attr + '="' + esc(value) + '" role="button" tabindex="0"'
      + ' aria-label="' + esc(T(labelKey)) + '"'
      + ' data-i18n-tip="' + hintKey + '" data-tip-desc="' + esc(tip) + '">'
      + bellSvg(until !== null) + '</span>';
  }

  /** The bell for this server's row. */
  function bellHtml() {
    return rowBell('server-mute', 'data-server', server(), serverMuted(), 'notifications.server_bell_hint', 'notifications.mute_server', 'srv-mute');
  }

  /** The bell for a room's row (rung 2). */
  function roomBellHtml(room) {
    return rowBell('room-mute', 'data-room', room, roomMuted(room), 'notifications.room_bell_hint', 'notifications.mute_room', 'row-mute');
  }

  /** The bell for a DM's row (rung 2). */
  function dmBellHtml(key) {
    return rowBell('dm-mute', 'data-dm', key, personMuted('dm:' + key), 'notifications.dm_bell_hint', 'notifications.mute_person', 'row-mute');
  }

  function closeMenu() {
    var old = document.getElementById('srv-mute-menu');
    if (old) old.remove();
    document.removeEventListener('click', outside, true);
  }

  function outside(e) {
    var menu = document.getElementById('srv-mute-menu');
    if (menu && !menu.contains(e.target) && !(e.target.closest && e.target.closest('[data-action$="-mute"]'))) closeMenu();
  }

  function heading(menu, words) {
    var head = document.createElement('div');
    head.className = 'srv-mute-head';
    head.textContent = words;
    menu.appendChild(head);
  }

  function item(menu, key, hintKey, onPick, checked) {
    var b = document.createElement('button');
    b.type = 'button';
    b.textContent = T(key);
    if (checked !== undefined) {
      b.setAttribute('role', 'menuitemradio');
      b.setAttribute('aria-checked', checked ? 'true' : 'false');
      if (checked) b.className = 'checked';
    }
    if (window.i18n) window.i18n.hint(b, hintKey);
    b.addEventListener('click', function (e) { e.stopPropagation(); closeMenu(); onPick(); });
    menu.appendChild(b);
  }

  /** The lengths (or Unmute while muted) for one mute. */
  function muteItems(menu, until, muteKey, unmuteKey, hintKey, set) {
    if (until === null) {
      heading(menu, T(muteKey));
      ((rules() && rules().file().mute_for) || []).forEach(function (m) {
        item(menu, m.label, m.hint, function () { set(m.seconds); });
      });
    } else {
      heading(menu, T('notifications.muted_notice', { until: untilWords(until) }));
      item(menu, unmuteKey, hintKey, function () { set(null); });
    }
  }

  function openMenu(anchor, fill, at) {
    closeMenu();
    var menu = document.createElement('div');
    menu.id = 'srv-mute-menu';
    menu.className = 'srv-mute-menu';
    menu.setAttribute('role', 'menu');
    fill(menu);
    var r = anchor.getBoundingClientRect();
    menu.style.left = Math.round(at ? at.x : r.left) + 'px';
    menu.style.top = Math.round(at ? at.y : r.bottom + 4) + 'px';
    document.body.appendChild(menu);
    var first = menu.querySelector('button');
    if (first) first.focus();
    setTimeout(function () { document.addEventListener('click', outside, true); }, 0);
  }

  function serverMenu(menu) {
    muteItems(menu, serverMuted(), 'notifications.mute_server', 'notifications.unmute_server', 'notifications.unmute_hint', setServerMute);
  }

  function roomMenu(room) {
    return function (menu) {
      // Copy link first (2026-11-03): the room's address on this server (app.js copyChannelLink).
      if (typeof copyChannelLink === 'function') {
        item(menu, 'chat.copy_link', 'chat.copy_link_hint', function () { copyChannelLink(room); });
        menu.appendChild(document.createElement('hr'));
      }
      muteItems(menu, roomMuted(room), 'notifications.mute_room', 'notifications.unmute_room', 'notifications.unmute_hint', function (s) { setMute('rooms', server() + '|' + room, s); });
      var sep = document.createElement('hr');
      menu.appendChild(sep);
      heading(menu, T('notifications.level_heading'));
      var current = rules() ? rules().roomLevel(loadLevels(), server(), room) : 'mentions';
      ((rules() && rules().file().levels) || []).forEach(function (l) {
        item(menu, l.label, l.hint, function () { setRoomLevel(room, l.id); }, l.id === current);
      });
    };
  }

  /** Mute or unmute the conversation with `key`, keeping its name beside the mute for the Settings
   *  page's Muted list (hos_notify_names; the page has no DM list of its own). */
  function setPersonMute(key, seconds) {
    var conv = 'dm:' + key;
    var names = loadJson(NAMES_KEY) || {};
    if (seconds === null) {
      delete names[conv];
    } else {
      var c = (typeof dmConversations !== 'undefined' && Array.isArray(dmConversations) ? dmConversations : []).find(function (x) { return x.partner_key === key; });
      names[conv] = (c && c.partner_name) || key.slice(0, 8);
    }
    saveJson(NAMES_KEY, names);
    setMute('people', conv, seconds);
  }

  function dmMenu(key) {
    return function (menu) {
      muteItems(menu, personMuted('dm:' + key), 'notifications.mute_person', 'notifications.unmute_person', 'notifications.unmute_hint', function (s) { setPersonMute(key, s); });
      // The always-allow list (rung 3): their calls and messages come through quiet hours.
      menu.appendChild(document.createElement('hr'));
      var on = allowed(key);
      item(menu, on ? 'notifications.unallow_person' : 'notifications.allow_person', on ? 'notifications.unallow_person_hint' : 'notifications.allow_person_hint', function () {
        var c = (typeof dmConversations !== 'undefined' && Array.isArray(dmConversations) ? dmConversations : []).find(function (x) { return x.partner_key === key; });
        setAllowed(key, !on, c && c.partner_name);
      });
    };
  }

  /** What a bell or a row's right-click opens, for the element `el` inside a list. */
  function menuFor(el) {
    var bell = el.closest && el.closest('[data-action$="-mute"]');
    if (bell) {
      var action = bell.getAttribute('data-action');
      if (action === 'server-mute') return { anchor: bell, fill: serverMenu };
      if (action === 'room-mute') return { anchor: bell, fill: roomMenu(bell.getAttribute('data-room')) };
      if (action === 'dm-mute') return { anchor: bell, fill: dmMenu(bell.getAttribute('data-dm')) };
    }
    return null;
  }

  // The bells sit inside rows that open or fold on click, so their click is taken here first
  // (capture), before the row's own handler.
  function onBell(e) {
    var m = menuFor(e.target);
    if (!m) return;
    if (e.type === 'keydown' && e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    e.stopPropagation();
    openMenu(m.anchor, m.fill);
  }

  /** A right-click on a room's or a DM's row opens its bell's menu where the pointer is. */
  function onRowMenu(e) {
    var row = e.target.closest && e.target.closest('.srv-chan[data-channel-id], .dm-item[data-dm-key]');
    if (!row) return;
    var room = row.getAttribute('data-channel-id');
    var key = row.getAttribute('data-dm-key');
    e.preventDefault();
    e.stopPropagation();
    openMenu(row, room ? roomMenu(room) : dmMenu(key), { x: e.clientX, y: e.clientY });
  }

  function wireList(id) {
    var el = document.getElementById(id);
    if (!el || el.dataset.muteWired) return;
    el.dataset.muteWired = '1';
    el.addEventListener('click', onBell, true);
    el.addEventListener('keydown', onBell, true);
    el.addEventListener('contextmenu', onRowMenu, true);
  }

  function wire() {
    wireList('server-list');
    wireList('dm-list');
    wireHeaderBell();
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', wire);
  else wire();
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Escape') return;
    closeMenu();
    if (panelOpen) closePanel();
  });

  window.HosChatNotify = {
    FEDERATED_LIVE_MS: FEDERATED_LIVE_MS,
    decide: decide,
    server: server,
    serverMuted: serverMuted,
    bellHtml: bellHtml,
    roomBellHtml: roomBellHtml,
    dmBellHtml: dmBellHtml,
    prefsData: prefsData,
    toggleDm: toggleDm,
    // The in-app list, for the panel and for tests.
    list: function () { return list.slice(); },
    unseen: function () { return unseen; },
    openPanel: openPanel,
    closePanel: closePanel,
    // Open a notice's conversation (an entry's {kind, conversation, room, sender}): the panel's
    // rows and a click on the browser's own notice (chat-ui.js notifyNewMessage, rung 2 part 2).
    openEntry: openEntry,
    // Rung 3: each kind's sound, the always-allow list, a ringing call, a task notice.
    playSound: playSound,
    allowed: allowed,
    setAllowed: setAllowed,
    personLevel: personLevel,
    // The profile card's "Your settings for <name>" (per-person controls): a conversation's mute.
    personMuted: personMuted,
    setPersonMute: setPersonMute,
    untilWords: untilWords,
    screenAllowNote: screenAllowNote,
    flushAllowNotes: flushAllowNotes,
    ring: ring,
    ringEnded: ringEnded,
    onTaskNotice: onTaskNotice
  };
})();
