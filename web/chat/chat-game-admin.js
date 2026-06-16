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
    return overlay;
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
    sendGameBannedListRequest();    // then refresh from the relay
  }

  function closeGameAdminModal() {
    var overlay = document.getElementById('gameadmin-overlay');
    if (overlay) overlay.classList.remove('open');
  }

  // Expose the hooks app.js + the command palette call.
  window.openGameAdminModal = openGameAdminModal;
  window.closeGameAdminModal = closeGameAdminModal;
  window.renderGameAdminList = renderGameAdminList;
  window.showGameAdminError = showGameAdminError;
})();
