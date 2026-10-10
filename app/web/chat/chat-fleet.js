// Your fleet ledger (2026-10-04): the web's read-only mirror of the game's Inventory > The fleet
// (the native original is src/gui/pages/fleet_ledger.rs draw_section).
//
// The operator: "we'll say the fleet has unlimited of everything and just track what they player
// uses and contributes. That way they can be in the red or black so they can gauge what they're
// actually using/contributing." The server keeps each player's ledger: meals taken from the ship's
// stores and power from the ship's reactor (used), items given at a fleet store and power sent
// back (given). The ledger is the server's record, so this browser, signed in as the player, can
// show it with no game world. Taking a meal and giving stay in the game, because they happen at a
// store in the shared world. (The review of 2026-10-04, finding 14: the reason given before, that
// the web has no game world, was true of giving and never of reading.)
//
// Transport: the chat's own signed-in socket. Sends game_fleet_ledger_request; app.js hands the
// relay's game_fleet_ledger (a __game__ private message) to renderFleetLedger. The relay builds the
// ledger from the socket's own key alone, so only the player's own ledger is ever shown here.
// Reached from the command palette ("Your fleet ledger", data/commands.json openFleetLedger).

(function () {
  'use strict';

  // The same sentences as the native panel (a native test reads this file for them).
  var INTRO = 'The ship\'s fleet supplies everyone aboard: meals from the mess hall\'s stores and power from ' +
    'the ship\'s reactor. Its ledger keeps what you used and what you gave back, so you can see whether ' +
    'you are in the red or in the black.';
  var POWER_NOTE = 'Power for your home is counted by itself every minute you are in the shared world, and ' +
    'power your home\'s own panels send back to the ship counts as given.';
  var SUPPLY = {
    unlimited: 'This server\'s fleet is unlimited: its stores never run out. This ledger is how you see what you take from it and what you give it.',
    stocked: 'This server\'s fleet is stocked: its stores hold only what is in them, and an empty store means a missed meal.'
  };

  // Credits as the game writes them (native credits()): whole numbers bare, else one decimal.
  function credits(v) { return (Math.abs(v - Math.round(v)) < 0.05 ? String(Math.round(v)) : v.toFixed(1)) + ' CR'; }

  // The headline, as native standing_sentence: a difference that prints as 0 CR reads even.
  function standingSentence(l) {
    var used = l.used_value || 0, given = l.contributed_value || 0;
    var diff = Math.abs(given - used);
    if (used === 0 && given === 0) return 'Nothing yet: you have not used anything from the fleet or given it anything.';
    if (credits(diff) === '0 CR') return 'Even: you have given the fleet as much as you have used from it.';
    if (l.standing === 'black') return 'In the black: you have given the fleet ' + credits(diff) + ' more than you have used from it.';
    if (l.standing === 'red') return 'In the red: you have used ' + credits(diff) + ' more from the fleet than you have given it.';
    return 'Even: you have given the fleet as much as you have used from it.';
  }

  // How many of a thing, as native amount_text.
  function amountText(q, unit, name) {
    var n = Math.abs(q - Math.round(q)) < 1e-6 ? String(Math.round(q)) : q.toFixed(2);
    if (name) return n + ' ' + name;
    if (unit === 'meal') return Math.abs(q - 1) < 1e-6 ? '1 meal' : n + ' meals';
    if (unit === '') return Math.abs(q - 1) < 1e-6 ? '1 item' : n + ' items';
    return n + ' ' + unit;
  }

  // "Day N (YYYY-MM-DD)": the game day and the real date, no finer (the server keeps no finer).
  function whenText(gameTime, realDay) {
    var day = Math.floor((gameTime || 0) / 86400) + 1;
    var date = new Date((realDay || 0) * 86400000).toISOString().slice(0, 10);
    return 'Day ' + day + ' (' + date + ')';
  }

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  // Static structure only (no user content), so innerHTML here is safe; the ledger's rows are
  // built with createElement + textContent in renderFleetLedger.
  function buildOverlay() {
    var overlay = document.createElement('div');
    overlay.id = 'fleet-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.innerHTML =
      '<div class="profile-modal" role="dialog" aria-label="Your fleet ledger">' +
      '  <button class="close-btn" id="fleet-close" aria-label="Close">&times;</button>' +
      '  <h2>Your fleet ledger</h2>' +
      '  <p class="gameadmin-hint" id="fleet-intro"></p>' +
      '  <p class="gameadmin-hint" id="fleet-power-note"></p>' +
      '  <div id="fleet-body"></div>' +
      '  <p class="gameadmin-hint">Taking a meal and giving happen in the game, at a fleet store in the shared world. ' +
      '     This page shows your ledger only.</p>' +
      '  <div class="gameadmin-toolbar"><button class="gameadmin-refresh-btn" id="fleet-refresh">Refresh</button></div>' +
      '  <div class="gameadmin-status" id="fleet-status"></div>' +
      '</div>';
    document.body.appendChild(overlay);
    overlay.querySelector('#fleet-intro').textContent = INTRO;
    overlay.querySelector('#fleet-power-note').textContent = POWER_NOTE;
    overlay.querySelector('#fleet-close').addEventListener('click', closeFleetLedger);
    overlay.addEventListener('click', function (e) { if (e.target === overlay) closeFleetLedger(); });
    overlay.querySelector('#fleet-refresh').addEventListener('click', requestLedger);
    return overlay;
  }

  function setStatus(t) { var s = document.getElementById('fleet-status'); if (s) s.textContent = t || ''; }

  function requestLedger() {
    if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { setStatus('Not connected to the server.'); return; }
    ws.send(JSON.stringify({ type: 'game_fleet_ledger_request' }));
    setStatus('Asking the server for your ledger...');
  }

  // Paint the ledger app.js last received (window.fleetLedger): the headline, the totals, power's
  // share, the supply mode, each kind, the newest lines.
  function renderFleetLedger() {
    var body = document.getElementById('fleet-body');
    if (!body) return; // window not built yet
    var l = window.fleetLedger;
    body.textContent = '';
    if (!l) { body.appendChild(el('p', 'gameadmin-hint', 'The ledger is kept by the server whose shared world you play in.')); return; }
    if (l.error) { setStatus('The server could not read your ledger.'); return; }
    setStatus('');
    var head = standingSentence(l);
    var tone = head.indexOf('In the black') === 0 ? 'black' : head.indexOf('In the red') === 0 ? 'red' : '';
    body.appendChild(el('h3', 'fleet-headline ' + tone, head));
    body.appendChild(el('p', 'fleet-totals', 'Used from the fleet: ' + credits(l.used_value || 0) + '   Given to the fleet: ' + credits(l.contributed_value || 0)));
    var pu = 0, pg = 0;
    (l.kinds || []).forEach(function (k) {
      if (String(k.kind).indexOf('power') !== 0) return;
      if (k.direction === 'used') pu += k.value || 0; else pg += k.value || 0;
    });
    if (pu > 0 || pg > 0) body.appendChild(el('p', 'gameadmin-hint', 'Of which power: ' + credits(pu) + ' used, ' + credits(pg) + ' given back.'));
    var supply = typeof window.fleetSupplyMode === 'string' ? window.fleetSupplyMode : l.supply;
    body.appendChild(el('p', 'gameadmin-hint', SUPPLY[supply] || SUPPLY.unlimited));
    if ((l.kinds || []).length) {
      body.appendChild(el('h3', 'gameadmin-section', 'By kind'));
      var t = el('table', 'fleet-table');
      l.kinds.forEach(function (k) {
        var tr = el('tr');
        tr.appendChild(el('td', '', k.label));
        tr.appendChild(el('td', 'fleet-muted', amountText(k.quantity || 0, k.unit || '', null)));
        var used = k.direction === 'used';
        tr.appendChild(el('td', used ? 'fleet-used' : 'fleet-given', credits(k.value || 0) + (used ? ' used' : ' given')));
        t.appendChild(tr);
      });
      body.appendChild(t);
    }
    if ((l.recent || []).length) {
      body.appendChild(el('h3', 'gameadmin-section', 'Newest lines'));
      var r = el('table', 'fleet-table');
      l.recent.forEach(function (line) {
        var tr = el('tr');
        var used = line.direction === 'used';
        tr.appendChild(el('td', 'fleet-muted', whenText(line.game_time, line.real_day)));
        tr.appendChild(el('td', '', line.label));
        tr.appendChild(el('td', 'fleet-muted', amountText(line.quantity || 0, line.unit || '', line.item_name)));
        tr.appendChild(el('td', used ? 'fleet-used' : 'fleet-given', (used ? '-' : '+') + credits(line.value || 0)));
        r.appendChild(tr);
      });
      body.appendChild(r);
    }
  }

  function openFleetLedger() {
    var overlay = document.getElementById('fleet-overlay') || buildOverlay();
    overlay.classList.add('open');
    renderFleetLedger();
    requestLedger();
  }

  function closeFleetLedger() {
    var overlay = document.getElementById('fleet-overlay');
    if (overlay) overlay.classList.remove('open');
  }

  window.openFleetLedger = openFleetLedger;
  window.closeFleetLedger = closeFleetLedger;
  window.renderFleetLedger = renderFleetLedger;
})();
