// Funding figures editor in the Game Admin window (2026-11-22; the native original is
// src/gui/pages/funding_admin.rs, the plan docs/design/live-funding.md). The operator, that day:
// "Can we update the values live? So people can see what current funding is at least via other
// things not my second job?"
//
// An admin or the owner of a server enters what donations bring in each month, one row per way
// people give (Patreon, GitHub Sponsors, CashApp...), and what running the project costs, one row
// per cost; the Donate page of everyone connected shows them. The figures are edited as a draft and
// saved with one server_settings_update carrying funding_figures; the relay checks the sender is an
// admin or the owner, holds the figures to their shape and stamps each new or changed figure (or one
// marked "still right today") with today's date (src/relay/storage/funding_figures.rs). The figures
// shown come from server_settings_state (app.js keeps them in window.fundingFigures).
// chat-game-admin.js builds this section into its window (buildFundingAdmin) and app.js repaints it
// (renderFundingAdmin). DOM is built with createElement and textContent.
(function () {
  'use strict';

  var MAX_ROWS = 32, MAX_USD = 1000000;
  var draft = null; // the admin's unsaved edits; null follows the server's figures
  var submitted = false; // a save went out: the next settings from the server replace the draft

  function T(key, vars) { return window.i18n ? i18n.t(key, vars) : ''; }

  function clone(f) { return JSON.parse(JSON.stringify(f || { channels: [], costs: [] })); }
  function same(a, b) { return JSON.stringify(a) === JSON.stringify(b); }
  function current() {
    var f = window.fundingFigures;
    return f && Array.isArray(f.channels) && Array.isArray(f.costs) ? f : { channels: [], costs: [] };
  }

  // Each control carries its words: the hint a pointer resting on it shows (dataset.tipDesc) and its
  // accessible name (aria-label).
  function button(key, hintKey, onClick) {
    var b = document.createElement('button');
    b.className = 'gameadmin-refresh-btn';
    b.textContent = T(key);
    b.setAttribute('aria-label', T(key));
    b.dataset.tipDesc = T(hintKey);
    b.addEventListener('click', onClick);
    return b;
  }
  function textInput(value, hintKey, width, onChange) {
    var i = document.createElement('input');
    i.type = 'text';
    i.value = value;
    i.style.width = width;
    i.setAttribute('aria-label', T(hintKey));
    i.dataset.tipDesc = T(hintKey);
    i.addEventListener('input', function () { onChange(i.value); });
    return i;
  }
  function amountInput(value, onChange) {
    var i = document.createElement('input');
    i.type = 'number';
    i.min = '0';
    i.max = String(MAX_USD);
    i.step = '1';
    i.value = String(value);
    i.style.width = '7em';
    i.setAttribute('aria-label', T('funding_admin.amount_hint'));
    i.dataset.tipDesc = T('funding_admin.amount_hint');
    i.addEventListener('input', function () { onChange(Number(i.value) || 0); });
    return i;
  }

  function buildRow(list, i, withLink) {
    var r = list[i];
    var line = document.createElement('div');
    line.className = 'gameadmin-toolbar';
    line.appendChild(textInput(r.name, 'funding_admin.name_hint', '10em', function (v) { r.name = v; paintApply(); }));
    if (withLink) line.appendChild(textInput(r.link || '', 'funding_admin.link_hint', '16em', function (v) { r.link = v; paintApply(); }));
    line.appendChild(amountInput(r.monthly_usd, function (v) { r.monthly_usd = v; paintApply(); }));
    var note = document.createElement('span');
    note.className = 'gameadmin-hint';
    note.textContent = r.as_of ? T('funding_admin.as_of_now', { date: r.as_of }) : T('funding_admin.as_of_new');
    line.appendChild(note);
    line.appendChild(button('funding_admin.still_right', 'funding_admin.still_right_hint', function () { r.as_of = ''; render(); }));
    line.appendChild(button('funding_admin.remove', 'funding_admin.remove_hint', function () { list.splice(i, 1); render(); }));
    return line;
  }

  function paintApply() {
    var b = document.getElementById('gameadmin-funding-save');
    if (b) b.disabled = !draft || same(draft, current());
  }

  // Paint the section: the server's figures, or the admin's draft of them.
  function render() {
    var box = document.getElementById('gameadmin-funding-rows');
    if (!box) return; // the window is not built yet
    if (draft && same(draft, current())) draft = null;
    var f = draft || clone(current());
    if (!draft) draft = f; // edits below mutate the working copy; paintApply compares it to the server's
    while (box.firstChild) box.removeChild(box.firstChild);
    var h1 = document.createElement('div');
    h1.className = 'gameadmin-hint';
    h1.textContent = T('funding_admin.channels_heading');
    box.appendChild(h1);
    f.channels.forEach(function (_, i) { box.appendChild(buildRow(f.channels, i, true)); });
    var add1 = button('funding_admin.add_channel', 'funding_admin.add_channel_hint', function () {
      if (f.channels.length < MAX_ROWS) { f.channels.push({ name: '', link: '', monthly_usd: 0, as_of: '' }); render(); }
    });
    box.appendChild(add1);
    var h2 = document.createElement('div');
    h2.className = 'gameadmin-hint';
    h2.textContent = T('funding_admin.costs_heading');
    box.appendChild(h2);
    f.costs.forEach(function (_, i) { box.appendChild(buildRow(f.costs, i, false)); });
    var add2 = button('funding_admin.add_cost', 'funding_admin.add_cost_hint', function () {
      if (f.costs.length < MAX_ROWS) { f.costs.push({ name: '', monthly_usd: 0, as_of: '' }); render(); }
    });
    box.appendChild(add2);
    paintApply();
  }

  // Ask the relay to publish the figures. It checks the sender is an admin or the owner; only a
  // message that went out is reported as asked.
  function submit() {
    var status = document.getElementById('gameadmin-status');
    if (!draft) return;
    if (typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) { if (status) status.textContent = T('funding_admin.not_sent'); return; }
    ws.send(JSON.stringify({ type: 'server_settings_update', funding_figures: draft }));
    submitted = true;
    if (status) status.textContent = T('funding_admin.asked');
  }

  // Build the section into the Game Admin window, just above its status line.
  window.buildFundingAdmin = function (overlay) {
    var status = overlay.querySelector('#gameadmin-status');
    if (!status) return;
    var sec = document.createElement('div');
    var title = document.createElement('h3');
    title.className = 'gameadmin-section';
    title.textContent = T('funding_admin.title');
    var explainer = document.createElement('p');
    explainer.className = 'gameadmin-hint';
    explainer.textContent = T('funding_admin.explainer');
    var rows = document.createElement('div');
    rows.id = 'gameadmin-funding-rows';
    var save = button('funding_admin.save', 'funding_admin.save_hint', submit);
    save.id = 'gameadmin-funding-save';
    var note = document.createElement('p');
    note.className = 'gameadmin-hint';
    note.textContent = T('funding_admin.note');
    [title, explainer, rows, save, note].forEach(function (e) { sec.appendChild(e); });
    status.parentNode.insertBefore(sec, status);
    render();
  };
  // app.js calls this when the server's settings arrive. After this admin saved, the server's
  // figures (with their new dates) replace the draft.
  window.renderFundingAdmin = function () {
    if (submitted) { draft = null; submitted = false; }
    render();
  };
})();
