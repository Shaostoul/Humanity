// Game Admin overlay (web mirror of the native Game Admin page, v0.477.x).
//
// Game-world bans are STRUCTURALLY SEPARATE from chat moderation. Operator
// directive: "The comms is the most important aspect of HumanityOS, I want to
// guarantee free speech. Being able to play video games with each other on the
// official MMO server is a privilege." So this surface issues GAME bans only:
// they block a player from the shared 3D world and never touch chat. It is a
// deliberately separate overlay (not folded into chat moderation), reached from
// the admin-only "Game Admin" command-palette item.
//
// Transport: the same authenticated chat WS the rest of the client uses. Sends
// game_ban / game_unban / game_banned_list_request (the global helpers in
// app.js); receives game_banned_list / game_admin_error (decoded in app.js's
// system/__game__ block, which calls renderGameAdminList / showGameAdminError).
// The Shared world clock section sends server_settings_update with world_time_scale and
// shows the speed app.js last heard (server_settings_state, game_time_sync).
// The relay is the authoritative admin gate; the client checks below only hide
// the UI. See src/gui/pages/game_admin.rs for the native original.

(function () {
  'use strict';

  // Defense-in-depth role gate (the relay is authoritative). Mirrors native
  // current_game_admin_role: admin or owner only.
  function gameAdminAmAdmin() {
    var r = (typeof peerData !== 'undefined' && peerData[myKey] && peerData[myKey].role) ||
            window.myPeerRole || '';
    return r === 'admin' || r === 'owner';
  }

  // Build the overlay DOM once, on first open. Static structure only (no user
  // content), so innerHTML here is safe; dynamic rows are built with
  // createElement + textContent in renderGameAdminList.
  function buildOverlay() {
    var overlay = document.createElement('div');
    overlay.id = 'gameadmin-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.innerHTML =
      '<div class="profile-modal" role="dialog" aria-label="Game Admin">' +
      '  <button class="close-btn" id="gameadmin-close" aria-label="Close">&times;</button>' +
      '  <h2>Game Admin</h2>' +
      '  <div class="gameadmin-disclaimer">' +
      '    <h3>Game bans do NOT affect chat</h3>' +
      '    <p>A game ban blocks a player from the shared 3D world only. Chat is a right: a ' +
      '       game-banned user keeps full access to every channel and every direct message. ' +
      '       Playing on the world is a privilege, and only that privilege is revoked here. ' +
      '       To moderate chat, use chat moderation instead, it is a separate system.</p>' +
      '  </div>' +
      '  <h3 class="gameadmin-section">Ban a player from the game</h3>' +
      '  <p class="gameadmin-hint">Enter the player\'s public key (their identity). The ban takes ' +
      '     effect immediately: if they are in the world they are removed, and their next join is ' +
      '     refused. Their chat is untouched.</p>' +
      '  <input type="text" id="gameadmin-key" placeholder="player public key (hex)" autocomplete="off" spellcheck="false">' +
      '  <input type="text" id="gameadmin-reason" placeholder="why (shown to admins, optional)" autocomplete="off">' +
      '  <button class="gameadmin-danger-btn" id="gameadmin-ban-btn">Game-ban player</button>' +
      '  <h3 class="gameadmin-section">Game-banned players</h3>' +
      '  <div class="gameadmin-toolbar">' +
      '    <button class="gameadmin-refresh-btn" id="gameadmin-refresh">Refresh</button>' +
      '    <span class="gameadmin-count" id="gameadmin-count"></span>' +
      '  </div>' +
      '  <div id="gameadmin-list"></div>' +
      '  <h3 class="gameadmin-section">Homes on the ship</h3>' +
      '  <p class="gameadmin-hint">Each player who joins holds one plot of the ship for their home, and ' +
      '     keeps it when they leave. To give a plot back (someone left for good, and the ship is full), ' +
      '     enter the plot\'s id (p1, p2, ...) or the public key of the player who holds it. It works only ' +
      '     while they are out of the world; their own home and saves are untouched, and they get a free ' +
      '     plot, or a guest place, when they come back.</p>' +
      '  <input type="text" id="gameadmin-plot-key" placeholder="plot id (p1) or player public key (hex)" autocomplete="off" spellcheck="false">' +
      '  <button class="gameadmin-refresh-btn" id="gameadmin-release-btn">Release plot</button>' +
      '  <h3 class="gameadmin-section">Shared world clock</h3>' +
      '  <p class="gameadmin-hint">How fast time passes in this server\'s shared world, for everyone in it: the sun, ' +
      '     crops, water tanks, batteries, the weather, and each player\'s hunger and thirst all follow this one ' +
      '     clock. A player\'s own Time setting applies only when they play alone. A change reaches every connected ' +
      '     game at once; the server does not restart, and the world keeps its date.</p>' +
      '  <p class="gameadmin-hint" id="gameadmin-clock-now"></p>' +
      '  <div class="gameadmin-toolbar gameadmin-clock-presets" id="gameadmin-clock-presets"></div>' +
      '  <div class="gameadmin-toolbar">' +
      '    <label for="gameadmin-clock-speed" class="gameadmin-hint">Custom speed</label>' +
      '    <input type="number" id="gameadmin-clock-speed" min="1" max="1000" step="1">' +
      '  </div>' +
      '  <p class="gameadmin-clock-if" id="gameadmin-clock-if"></p>' +
      '  <button class="gameadmin-refresh-btn" id="gameadmin-clock-apply">Apply to the shared world</button>' +
      '  <h3 class="gameadmin-section">Fleet supply</h3>' +
      '  <p class="gameadmin-hint">Whether the fleet\'s stores can run out in this server\'s shared world. Unlimited (the ' +
      '     default during early development): they never run out, no meal is ever refused, and each player\'s ledger shows ' +
      '     what they used and gave. Stocked (the realistic mode): the stores hold only what the ship\'s farms put in, and ' +
      '     an empty store means a missed meal, crew included. A change applies at once and never touches anyone\'s ledger.</p>' +
      '  <p class="gameadmin-hint" id="gameadmin-fleet-now"></p>' +
      '  <div class="gameadmin-toolbar" id="gameadmin-fleet-modes"></div>' +
      '  <button class="gameadmin-refresh-btn" id="gameadmin-fleet-apply">Apply to the fleet</button>' +
      '  <p class="gameadmin-hint">The fleet\'s totals are sums over every player with a ledger. They are shown only once at ' +
      '     least three players other than you have one, because with fewer, the totals minus your own lines would ' +
      '     be someone\'s own ledger. Even then, watching them change while you know who is online can hint at who did ' +
      '     what.</p>' +
      '  <div class="gameadmin-toolbar">' +
      '    <button class="gameadmin-refresh-btn" id="gameadmin-fleet-totals-btn">Show the fleet\'s totals</button>' +
      '    <span class="gameadmin-hint" id="gameadmin-fleet-totals"></span>' +
      '  </div>' +
      '  <div class="gameadmin-status" id="gameadmin-status"></div>' +
      '</div>';
    document.body.appendChild(overlay);

    // Close on backdrop click + close button (no inline handlers, CSP-safe).
    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) closeGameAdminModal();
    });
    overlay.querySelector('#gameadmin-close').addEventListener('click', closeGameAdminModal);
    overlay.querySelector('#gameadmin-refresh').addEventListener('click', function () {
      sendGameBannedListRequest();
      setStatus('Requested the latest game-ban list.');
    });
    overlay.querySelector('#gameadmin-ban-btn').addEventListener('click', submitBan);
    overlay.querySelector('#gameadmin-release-btn').addEventListener('click', submitRelease);
    buildClockControls(overlay);
    buildFleetControls(overlay);
    return overlay;
  }

  // ── Fleet supply (2026-10-04; the native original is draw_admin in
  // src/gui/pages/fleet_ledger.rs). The operator: "we'll say the fleet has unlimited of everything and
  // just track what they player uses and contributes." The relay keeps the server setting
  // fleet_supply_mode ("unlimited" by default, or "stocked", the stores that can run empty); an admin
  // changes it here with server_settings_update, and the running world follows at once. The mode shown
  // comes from server_settings_state (app.js keeps it in window.fleetSupplyMode). The totals are the
  // relay's game_fleet_totals: every player's ledger summed, naming no one (app.js keeps them in
  // window.fleetTotals), held back while fewer than three other players have a ledger. A player's
  // own ledger has its own read-only window (chat-fleet.js, "Your fleet ledger").
  var FLEET_MODES = [['unlimited', 'Unlimited (never runs out)'], ['stocked', 'Stocked (realistic: stores can run empty)']];
  var fleetDraft = null; // the admin's unapplied pick; null follows the server's mode

  function fleetModeName(m) {
    for (var i = 0; i < FLEET_MODES.length; i++) if (FLEET_MODES[i][0] === m) return FLEET_MODES[i][1];
    return FLEET_MODES[0][1];
  }
  // Credits as the game writes them (native credits()): whole numbers bare, else one decimal.
  function creditsText(v) { return (Math.abs(v - Math.round(v)) < 0.05 ? String(Math.round(v)) : v.toFixed(1)) + ' CR'; }

  function buildFleetControls(overlay) {
    var box = overlay.querySelector('#gameadmin-fleet-modes');
    FLEET_MODES.forEach(function (m) {
      var b = document.createElement('button');
      b.className = 'gameadmin-refresh-btn';
      b.textContent = m[1];
      b.setAttribute('data-mode', m[0]);
      b.addEventListener('click', function () { fleetDraft = m[0]; renderGameAdminFleet(); });
      box.appendChild(b);
    });
    overlay.querySelector('#gameadmin-fleet-apply').addEventListener('click', submitFleet);
    overlay.querySelector('#gameadmin-fleet-totals-btn').addEventListener('click', function () {
      if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { setStatus('Not connected to the server.'); return; }
      ws.send(JSON.stringify({ type: 'game_fleet_totals_request' }));
    });
  }

  // Paint the fleet section: the server's mode, the admin's pick, the totals when asked.
  function renderGameAdminFleet() {
    var now = document.getElementById('gameadmin-fleet-now');
    if (!now) return; // window not built yet
    var cur = typeof window.fleetSupplyMode === 'string' ? window.fleetSupplyMode : null;
    if (fleetDraft !== null && fleetDraft === cur) fleetDraft = null;
    now.textContent = cur === null ? 'Connect to a server to see its fleet\'s supply.' : 'Now: ' + fleetModeName(cur);
    var chosen = fleetDraft !== null ? fleetDraft : (cur !== null ? cur : 'unlimited');
    Array.prototype.forEach.call(document.querySelectorAll('#gameadmin-fleet-modes button'), function (b) {
      b.classList.toggle('active', b.getAttribute('data-mode') === chosen);
    });
    document.getElementById('gameadmin-fleet-apply').disabled = cur !== null && chosen === cur;
    var t = window.fleetTotals;
    var tot = document.getElementById('gameadmin-fleet-totals');
    if (t && typeof t.players === 'number') {
      var who = (t.players === 1 ? '1 player has' : t.players + ' players have') + ' a ledger';
      // Held back while too few other players have a ledger (native totals_sentence).
      tot.textContent = t.withheld
        ? who + '. The totals are shown once at least ' + (t.others_needed || 3) + ' players other than you have one.'
        : who + ': ' + creditsText(t.used_value || 0) + ' used from the fleet, ' + creditsText(t.contributed_value || 0) + ' given to it.';
    }
  }

  // Ask the relay to run the fleet's stores in the chosen mode. It checks the sender is an admin,
  // saves it and changes the running world; only a message that went out is reported as asked.
  function submitFleet() {
    var cur = typeof window.fleetSupplyMode === 'string' ? window.fleetSupplyMode : null;
    var chosen = fleetDraft !== null ? fleetDraft : cur;
    if (chosen === null) { setStatus('Pick a mode first.'); return; }
    if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { setStatus('Not connected to the server.'); return; }
    ws.send(JSON.stringify({ type: 'server_settings_update', fleet_supply_mode: chosen }));
    setStatus('Asked the server to make the fleet ' + chosen + '.');
  }

  // ── Shared world clock (2026-10-04; the native original is src/gui/pages/world_clock_admin.rs).
  // The operator: "let's do 72x but, make sure there's admin tools for me to adjust it from inside
  // the app." That evening the default became real time: "For normal mode, especially for my MMO
  // server, let's have everything be real time, not the 72x." The relay runs the shared world's
  // clock at the server setting world_time_scale (1, real time, on a new server); an admin changes
  // it here with server_settings_update, and the relay tells every connected game at once. The
  // speed shown comes from server_settings_state and game_time_sync (app.js keeps the latest in
  // window.worldClockSpeed).
  //
  // The speeds offered, as the native's systems::time::TIME_SPEED_PRESETS (a Rust test,
  // world_clock_admin.rs, checks the two lists agree).
  var CLOCK_PRESETS = [[1, 'Realistic'], [24, 'A day an hour'], [72, 'Simplified'], [720, 'Garden testing']];
  // The speed shown with no server to ask, a new server's (the relay's default_world_time_scale),
  // and what a speed that is not a number reads as (the native's clamp_time_speed): real time. The
  // same Rust test fails if it ever differs from the native section's.
  var CLOCK_DEFAULT = 1;
  // A lettuce's growing time in days, data/plants.csv growth_days: the website serves only the JSON
  // under data/, so the number is here, and the same Rust test fails if it ever differs from the CSV.
  var LETTUCE_GROWTH_DAYS = 45;
  var CLOCK_MIN = 1, CLOCK_MAX = 1000, SHARED_DAY_HOURS = 24;
  var clockDraft = null; // the admin's unapplied pick; null follows the server's speed

  function clampClock(v) {
    v = Number(v);
    if (!isFinite(v)) return CLOCK_DEFAULT;
    return Math.min(CLOCK_MAX, Math.max(CLOCK_MIN, v));
  }
  function numberText(v) { return Math.abs(v - Math.round(v)) < 0.05 ? String(Math.round(v)) : v.toFixed(1); }
  // How long a day of `hours` takes in real time at `speed` (native settings_time::day_in_real_time).
  function dayInRealTime(hours, speed) {
    var secs = hours * 3600 / Math.max(speed, 0.001);
    if (secs >= 2 * 3600) return Math.round(secs / 3600) + ' hours';
    if (secs >= 90) return Math.round(secs / 60) + ' minutes';
    return Math.round(secs) + ' seconds';
  }
  // A real stretch of time in the largest unit worth saying (native real_duration).
  function realDuration(secs) {
    if (secs >= 2 * 86400) return Math.round(secs / 86400) + ' days';
    if (secs >= 2 * 3600) return Math.round(secs / 3600) + ' hours';
    if (secs >= 90) return Math.round(secs / 60) + ' minutes';
    return Math.round(secs) + ' seconds';
  }
  // What a speed means, in words (native world_clock_admin::explainer, same sentences).
  function clockExplainer(speed) {
    speed = clampClock(speed);
    var crop = numberText(LETTUCE_GROWTH_DAYS) + ' days';
    if (Math.abs(speed - 1) < 1e-3) {
      return 'at 1x the world keeps real time: a day takes a real day, and a lettuce its real ' + crop + '.';
    }
    return 'at ' + numberText(speed) + 'x a day passes in ' + dayInRealTime(SHARED_DAY_HOURS, speed) +
      ', and a lettuce grows in about ' + realDuration(LETTUCE_GROWTH_DAYS * 86400 / speed) + ' instead of ' + crop + '.';
  }
  function currentClockSpeed() {
    return typeof window.worldClockSpeed === 'number' ? window.worldClockSpeed : null;
  }

  function buildClockControls(overlay) {
    var box = overlay.querySelector('#gameadmin-clock-presets');
    CLOCK_PRESETS.forEach(function (p) {
      var b = document.createElement('button');
      b.className = 'gameadmin-refresh-btn';
      b.textContent = p[1] + ' (' + numberText(p[0]) + 'x)';
      b.setAttribute('data-speed', String(p[0]));
      b.addEventListener('click', function () { clockDraft = p[0]; renderGameAdminClock(); });
      box.appendChild(b);
    });
    var input = overlay.querySelector('#gameadmin-clock-speed');
    input.addEventListener('input', function () {
      if (input.value === '') return;
      clockDraft = clampClock(Math.round(Number(input.value)));
      renderGameAdminClock();
    });
    overlay.querySelector('#gameadmin-clock-apply').addEventListener('click', submitClock);
  }

  // Paint the clock section: the server's speed now, the admin's pick, what the pick would mean.
  function renderGameAdminClock() {
    var now = document.getElementById('gameadmin-clock-now');
    if (!now) return; // window not built yet
    var cur = currentClockSpeed();
    // An applied pick is done once the server says it back.
    if (clockDraft !== null && cur !== null && Math.abs(clockDraft - cur) < 1e-3) clockDraft = null;
    now.textContent = cur === null ? 'Connect to a server to see how fast its world runs.' : 'Now: ' + clockExplainer(cur);
    var chosen = clockDraft !== null ? clockDraft : (cur !== null ? cur : CLOCK_DEFAULT);
    Array.prototype.forEach.call(document.querySelectorAll('#gameadmin-clock-presets button'), function (b) {
      b.classList.toggle('active', Math.abs(Number(b.getAttribute('data-speed')) - chosen) < 1e-3);
    });
    var input = document.getElementById('gameadmin-clock-speed');
    if (document.activeElement !== input) input.value = numberText(chosen);
    var differs = cur === null || Math.abs(cur - chosen) >= 1e-3;
    document.getElementById('gameadmin-clock-if').textContent = differs ? 'If applied: ' + clockExplainer(chosen) : '';
    document.getElementById('gameadmin-clock-apply').disabled = !differs;
  }

  // Ask the relay to run the shared world at the chosen speed. It checks the sender is an admin,
  // saves it, changes the running world and tells every game; only a message that went out is
  // reported as asked.
  function submitClock() {
    var cur = currentClockSpeed();
    var chosen = clockDraft !== null ? clockDraft : cur;
    if (chosen === null) { setStatus('Pick a speed first.'); return; }
    if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { setStatus('Not connected to the server.'); return; }
    ws.send(JSON.stringify({ type: 'server_settings_update', world_time_scale: chosen }));
    setStatus('Asked the server to run the shared world at ' + numberText(chosen) + 'x.');
  }

  // Give back a plot on the ship (ship homes increment 1b; the native original is
  // draw_plot_release in src/gui/pages/game_admin.rs), named by its id (p1) or by the public
  // key of the player who holds it. The relay is the authoritative admin gate, tells the two
  // apart, refuses while the holder is in the world, and answers with a game_admin_notice or
  // game_admin_error shown in the status line.
  function submitRelease() {
    var keyEl = document.getElementById('gameadmin-plot-key');
    var key = (keyEl.value || '').trim();
    if (!key) { setStatus('Enter the plot id, or the public key of the player whose plot to release.'); return; }
    if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { setStatus('Not connected to the server.'); return; }
    ws.send(JSON.stringify({ type: 'game_release_plot', target: key }));
    keyEl.value = '';
    setStatus('Asked the server to release ' + (key.length > 16 ? 'the plot of ' + shortKey(key) : 'plot ' + key) + '.');
  }

  function submitBan() {
    var keyEl = document.getElementById('gameadmin-key');
    var reasonEl = document.getElementById('gameadmin-reason');
    var key = (keyEl.value || '').trim();
    var reason = (reasonEl.value || '').trim();
    if (!key) { setStatus('Enter the player public key to ban.'); return; }
    sendGameBan(key, reason);
    keyEl.value = '';
    reasonEl.value = '';
    setStatus('Sent a game ban for ' + shortKey(key) + '. Chat is unaffected.');
  }

  function setStatus(msg) {
    var el = document.getElementById('gameadmin-status');
    if (el) el.textContent = msg || '';
  }

  function shortKey(k) {
    return (k && k.length > 20) ? (k.slice(0, 20) + '...') : (k || '');
  }

  // Unix ms -> "YYYY-MM-DD HH:MM" UTC (matches native format_ban_date).
  function fmtDate(ms) {
    var n = Number(ms);
    if (!n || n <= 0) return 'unknown';
    try { return new Date(n).toISOString().slice(0, 16).replace('T', ' '); }
    catch (e) { return 'unknown'; }
  }

  // Render window.gameBans into the list. createElement + textContent only, so a
  // hostile reason/key string can never inject HTML.
  function renderGameAdminList() {
    var list = document.getElementById('gameadmin-list');
    if (!list) return; // overlay not open
    var bans = Array.isArray(window.gameBans) ? window.gameBans : [];
    var countEl = document.getElementById('gameadmin-count');
    if (countEl) countEl.textContent = bans.length + ' game-banned';
    list.textContent = '';
    if (!bans.length) {
      var empty = document.createElement('p');
      empty.className = 'gameadmin-hint';
      empty.textContent = 'No one is game-banned. A clean slate.';
      list.appendChild(empty);
      return;
    }
    bans.forEach(function (b) {
      var row = document.createElement('div');
      row.className = 'gameadmin-row';

      var key = document.createElement('span');
      key.className = 'ga-key';
      key.textContent = shortKey(b.public_key);
      key.title = b.public_key || '';
      row.appendChild(key);

      var reason = document.createElement('span');
      reason.className = 'ga-reason';
      reason.textContent = (b.reason && b.reason.trim()) ? b.reason : '(no reason given)';
      row.appendChild(reason);

      var date = document.createElement('span');
      date.className = 'ga-date';
      date.textContent = fmtDate(b.banned_at);
      row.appendChild(date);

      var unban = document.createElement('button');
      unban.className = 'gameadmin-unban-btn';
      unban.textContent = 'Unban';
      unban.setAttribute('data-key', b.public_key || '');
      unban.addEventListener('click', function () {
        sendGameUnban(b.public_key);
        setStatus('Sent a game unban; the list will refresh.');
      });
      row.appendChild(unban);

      list.appendChild(row);
    });
  }

  function showGameAdminError(msg) {
    var el = document.getElementById('gameadmin-status');
    if (el) { el.textContent = msg || 'Game admin error.'; }
    else if (typeof addSystemMessage === 'function') { addSystemMessage(msg || 'Game admin error.'); }
    else { console.warn('game_admin_error:', msg); }
  }

  function openGameAdminModal() {
    // Never build the panel for a non-admin, so ban data never enters the DOM.
    if (!gameAdminAmAdmin()) {
      if (typeof addSystemMessage === 'function') addSystemMessage('Game Admin is limited to server admins.');
      return;
    }
    var overlay = document.getElementById('gameadmin-overlay') || buildOverlay();
    overlay.classList.add('open');
    setStatus('');
    renderGameAdminList();          // paint whatever we already have
    renderGameAdminClock();
    renderGameAdminFleet();
    sendGameBannedListRequest();    // then refresh from the relay
    // The server's settings, for the clock's speed (the relay answers with server_settings_state).
    if (typeof ws !== 'undefined' && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'server_settings_request' }));
    }
  }

  function closeGameAdminModal() {
    var overlay = document.getElementById('gameadmin-overlay');
    if (overlay) overlay.classList.remove('open');
  }

  // Expose the hooks app.js + the command palette call.
  window.openGameAdminModal = openGameAdminModal;
  window.closeGameAdminModal = closeGameAdminModal;
  window.renderGameAdminList = renderGameAdminList;
  window.renderGameAdminClock = renderGameAdminClock;
  window.renderGameAdminFleet = renderGameAdminFleet;
  window.showGameAdminError = showGameAdminError;
})();
