/* ── chat-notify.js ──────────────────────────────────────────────────────────
 * The chat's notifications (2026-10-30, docs/design/notifications.md), the
 * desktop app's engine/notify.rs and chat/mute_menu.rs mirrored:
 *
 * - HosChatNotify.decide(c, at): THE rule (/shared/notify-rules.js) with what
 *   this browser keeps: the copy of the person's synced switches and quiet hours
 *   (localStorage hos_notify_dms / _mentions / _tasks, hos_dnd_start / _end,
 *   written by Settings), what a notice shows (humanity_settings['notif-show'],
 *   "name" by default: who it is from, never what was said), and the mutes
 *   (hos_notify_mutes). chat-ui.js notifyNewMessage acts on the answer.
 * - Mute server: the bell on this server's row in the sidebar, opening the
 *   lengths from /data/notifications/notifications.json (1 hour, 8 hours,
 *   until I unmute), or Unmute while muted. A server's mute stops notices from
 *   its rooms, never a direct message, and never hides a message or a dot.
 *
 * Every word by key from data/i18n/en.json.
 */
(function () {
  'use strict';

  var MUTES_KEY = 'hos_notify_mutes';
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

  fetch('/data/notifications/notifications.json', { cache: 'no-cache' })
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (j) { if (j && rules()) rules().use(j); if (typeof renderServerList === 'function') renderServerList(); })
    .catch(function () { /* no data file: nothing notifies, the bell offers no lengths */ });

  /** This server, as the mutes name it. */
  function server() {
    return location.origin;
  }

  function loadMutes() {
    try {
      var m = JSON.parse(localStorage.getItem(MUTES_KEY));
      if (m && typeof m === 'object') {
        return { everything: m.everything === undefined ? null : m.everything, servers: m.servers || {}, rooms: m.rooms || {} };
      }
    } catch (_) { /* unreadable: no mutes */ }
    return { everything: null, servers: {}, rooms: {} };
  }

  function saveMutes(m) {
    try { localStorage.setItem(MUTES_KEY, JSON.stringify(m)); } catch (_) { /* private window: this page only */ }
  }

  function stored(key) {
    try { return localStorage.getItem(key); } catch (_) { return null; }
  }

  function synced() {
    return {
      dm: stored('hos_notify_dms') !== 'false',
      mentions: stored('hos_notify_mentions') !== 'false',
      tasks: stored('hos_notify_tasks') !== 'false',
      quiet_start: stored('hos_dnd_start'),
      quiet_end: stored('hos_dnd_end'),
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

  /** c: {kind, from_me, server, room, conversation, sender, room_name, text}; at: {focused,
   *  on_screen, desktop, sound}. Answers {why, toast, sound, title, body, tag}. */
  function decide(c, at) {
    var R = rules();
    if (!R) return { why: 'unknown_kind', toast: false, sound: false };
    var now = nowSecs();
    var d = new Date();
    var local = { desktop: !!at.desktop, sound: !!at.sound, show: show(), mutes: loadMutes() };
    var out = R.decide(c, synced(), local, {
      now: now,
      minute: d.getHours() * 60 + d.getMinutes(),
      focused: !!at.focused,
      on_screen: !!at.on_screen,
      last_notice: lastNotice[c.conversation] === undefined ? null : lastNotice[c.conversation]
    });
    if (out.why !== 'notify') return out;
    lastNotice[c.conversation] = now;
    var w = R.words(c, local.show, T);
    return { why: out.why, toast: out.toast, sound: out.sound, title: w[0], body: w[1], tag: 'hos-' + tagFor(c.conversation) };
  }

  /** A short tag per conversation (a later notice replaces the earlier one): a hash, never a name. */
  function tagFor(s) {
    var h = 0x811c9dc5;
    for (var i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
    return ('0000000' + h.toString(16)).slice(-8);
  }

  /** Until when this server is muted, or null. */
  function serverMuted() {
    var R = rules();
    var u = loadMutes().servers[server()];
    return R && u !== undefined && R.active(u, nowSecs()) ? u : null;
  }

  function untilWords(until) {
    if (until === 0) return T('notifications.muted_until_unmuted');
    var d = new Date(until * 1000);
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return T('notifications.muted_until', { time: pad(d.getHours()) + ':' + pad(d.getMinutes()) });
  }

  function setServerMute(seconds) {
    var m = loadMutes();
    if (seconds === null) delete m.servers[server()];
    else m.servers[server()] = seconds === 0 ? 0 : nowSecs() + seconds;
    saveMutes(m);
    var u = serverMuted();
    var msg = u === null ? T('notifications.unmuted_notice') : T('notifications.muted_notice', { until: untilWords(u) });
    if (typeof addNotice === 'function') addNotice(msg, u === null ? 'green' : 'orange', 4);
    if (typeof renderServerList === 'function') renderServerList();
  }

  var BELL = '<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.3" aria-hidden="true"><path d="M3.2 11.6h9.6M4 11.4c.6-1 .7-2.4.7-4.1a3.3 3.3 0 016.6 0c0 1.7.1 3.1.7 4.1"/><circle cx="8" cy="13.6" r="1" fill="currentColor" stroke="none"/>';
  var SLASH = '<line x1="2" y1="2" x2="14" y2="14"/>';

  /** The bell for this server's row: a bell, crossed out while muted (the desktop's mark). */
  function bellHtml() {
    var u = serverMuted();
    var tip = u === null ? T('notifications.server_bell_hint') : T('notifications.muted_mark_hint', { until: untilWords(u) });
    var esc = function (s) { return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); };
    return '<span class="srv-mute' + (u === null ? '' : ' muted') + '" data-action="server-mute" role="button" tabindex="0"'
      + ' aria-label="' + esc(T('notifications.mute_server')) + '"'
      + ' data-i18n-tip="notifications.server_bell_hint" data-tip-desc="' + esc(tip) + '">'
      + BELL + (u === null ? '' : SLASH) + '</svg></span>';
  }

  function closeMenu() {
    var old = document.getElementById('srv-mute-menu');
    if (old) old.remove();
    document.removeEventListener('click', outside, true);
  }

  function outside(e) {
    var menu = document.getElementById('srv-mute-menu');
    if (menu && !menu.contains(e.target) && !e.target.closest('[data-action="server-mute"]')) closeMenu();
  }

  function item(menu, key, hintKey, onPick) {
    var b = document.createElement('button');
    b.type = 'button';
    b.textContent = T(key);
    if (window.i18n) window.i18n.hint(b, hintKey);
    b.addEventListener('click', function (e) { e.stopPropagation(); closeMenu(); onPick(); });
    menu.appendChild(b);
  }

  function openMenu(anchor) {
    closeMenu();
    var menu = document.createElement('div');
    menu.id = 'srv-mute-menu';
    menu.className = 'srv-mute-menu';
    menu.setAttribute('role', 'menu');
    var u = serverMuted();
    var head = document.createElement('div');
    head.className = 'srv-mute-head';
    head.textContent = u === null ? T('notifications.mute_server') : T('notifications.muted_notice', { until: untilWords(u) });
    menu.appendChild(head);
    if (u === null) {
      var lengths = (rules() && rules().file().mute_for) || [];
      lengths.forEach(function (m) {
        item(menu, m.label, m.hint, function () { setServerMute(m.seconds); });
      });
    } else {
      item(menu, 'notifications.unmute_server', 'notifications.unmute_hint', function () { setServerMute(null); });
    }
    var r = anchor.getBoundingClientRect();
    menu.style.left = Math.round(r.left) + 'px';
    menu.style.top = Math.round(r.bottom + 4) + 'px';
    document.body.appendChild(menu);
    var first = menu.querySelector('button');
    if (first) first.focus();
    setTimeout(function () { document.addEventListener('click', outside, true); }, 0);
  }

  // The bell sits inside the row's collapse toggle, so its click is taken here first (capture),
  // before the sidebar's own handler folds the server away.
  function onBell(e) {
    var bell = e.target.closest && e.target.closest('[data-action="server-mute"]');
    if (!bell) return;
    if (e.type === 'keydown' && e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    e.stopPropagation();
    openMenu(bell);
  }

  function wire() {
    var list = document.getElementById('server-list');
    if (!list || list.dataset.muteWired) return;
    list.dataset.muteWired = '1';
    list.addEventListener('click', onBell, true);
    list.addEventListener('keydown', onBell, true);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', wire);
  else wire();
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') closeMenu(); });

  window.HosChatNotify = { decide: decide, server: server, serverMuted: serverMuted, bellHtml: bellHtml };
})();
