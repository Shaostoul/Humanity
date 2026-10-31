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
    var local = { desktop: !!at.desktop, sound: !!at.sound, show: show(), mutes: loadMutes(), levels: loadLevels() };
    var out = R.decide(c, synced(), local, {
      now: now,
      minute: d.getHours() * 60 + d.getMinutes(),
      focused: !!at.focused,
      on_screen: !!at.on_screen,
      last_notice: lastNotice[c.conversation] === undefined ? null : lastNotice[c.conversation]
    });
    if (out.why !== 'notify') return out;
    lastNotice[c.conversation] = now;
    if (out.list) listAdd(c, local.show, now);
    var w = R.words(c, local.show, T);
    return { why: out.why, toast: out.toast, sound: out.sound, list: out.list, title: w[0], body: w[1], tag: 'hos-' + tagFor(c.conversation) };
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
        list.unshift(e);
        refreshList();
        return;
      }
    }
    list.unshift({ kind: c.kind, server: c.server, room: c.room || null, conversation: c.conversation, sender: c.sender, room_name: c.room_name || '', text: text, at: now, seen: panelOpen });
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
      var c = { kind: e.kind, sender: e.sender, room_name: e.room_name, text: e.text || '' };
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
    closePanel: closePanel
  };
})();
