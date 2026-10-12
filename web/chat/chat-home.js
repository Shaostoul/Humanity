// Where you live (ship homes increment 6, lane 1F, 2026-10-12): the website's mirror of the game's
// "Choose where you live" on a server's card in Characters (Play), docs/design/ship-homes-increment-6-plan.md
// sections 3.9 and 3.11. The native original is src/gui/pages/where_you_live.rs and the server card in
// src/gui/pages/showroom.rs.
//
// It shows the person's own home on this server's ship, or, when they hold none, the kinds of home
// the server offers with how many of each are free, and a [Choose] for each one that can be chosen.
//
// Every start's name and sentence, and every refusal's sentence, comes from the server's answer
// (`game_home`, src/ship/starts.rs `HomeAnswer`), never from a copy here: a server that renames its
// Cabin, or adds a kind of home, is shown as it says. The few words this file owns are about the
// window itself (its title, "You have no home on this server yet.", "12 free", the note).
//
// Nobody is told who lives where (plan 3.11): the server answers about the asker's own plot and
// gives counts, and this window reads only those fields of the answer (`plot`, `starts`, `chose`,
// `refused`, `message`), so nothing else an answer carried could reach the screen.
//
// Transport: the chat's own signed-in socket, as chat-fleet.js. Sends `game_home_request` (where do I
// live, what is free) and `game_choose_home` {start} (give me a home of this kind, while I hold none);
// app.js hands the relay's `game_home` (a __game__ private message, to this person alone) to
// renderWhereYouLive. Reached from the command palette ("Where you live", data/commands.json
// openWhereYouLive). Tested by scripts/tests/chat-home.test.js.

(function () {
  'use strict';

  // The wire (src/ship/starts.rs HOME_REQUEST, CHOOSE_HOME, HOME_ANSWER).
  var HOME_REQUEST = 'game_home_request';
  var CHOOSE_HOME = 'game_choose_home';
  var HOME_ANSWER = 'game_home';

  // How long to wait for an answer before saying there was none: a server from before increment 6
  // does not know the request and never answers it.
  var NO_ANSWER_MS = 10000;

  // The window's own words. None of them names a kind of home.
  var WORDS = {
    title: 'Where you live',
    intro: 'Your home on this server\'s ship. The server tells you about your own home only: nobody is told who lives where.',
    none: 'You have no home on this server yet.',
    noneOffered: 'This server has no homes to choose from right now.',
    note: 'On a server you start fresh: only your name and look come with you. ' +
      'The game keeps your things there on the computer you play on.',
    choosing: 'Choosing here keeps that home for you on this server: the game takes you to it the next time you enter this server\'s world.',
    held: 'Where you live here: ',
    asking: 'Asking the server where you live...',
    askingFor: 'Asking the server for ',
    chosen: 'Chosen: this is your home on this server now.',
    refusedNoWords: 'The server did not give you that home.',
    noAnswer: 'This server has not answered. It may not offer homes yet.',
    notConnected: 'Not connected to the server.',
    choose: 'Choose',
    refresh: 'Refresh',
    close: 'Close'
  };

  // "a Cabin", "an Apartment": a name from the answer, with its article.
  function aOrAn(name) {
    return (/^[aeiou]/i.test(name) ? 'an ' : 'a ') + name;
  }

  // "12 free", "1 free", "none free".
  function freeText(n) {
    return n > 0 ? n + ' free' : 'none free';
  }

  // ── The answer, read ────────────────────────────────────────────────────────

  // The last answer shown ({plot, starts, chose, refused, message}), and what is being asked:
  // null, 'request', or the id of the start being chosen.
  var shown = null;
  var asking = null;
  var waitTimer = null;

  function text(v) { return typeof v === 'string' ? v : ''; }

  // Only the fields of src/ship/starts.rs HomeAnswer are read, each checked; anything else the
  // answer carried is dropped here.
  function readAnswer(game) {
    if (!game || typeof game !== 'object' || game.type !== HOME_ANSWER) return null;
    var p = game.plot;
    var plot = p && typeof p === 'object' && text(p.id)
      ? { id: text(p.id), kind: text(p.kind), start: text(p.start) }
      : null;
    var starts = (Array.isArray(game.starts) ? game.starts : []).filter(function (s) {
      return s && typeof s === 'object' && text(s.id) && text(s.name);
    }).map(function (s) {
      var free = Number(s.free);
      return {
        id: text(s.id),
        name: text(s.name),
        sentence: text(s.sentence),
        offered: s.offered === true,
        why_not: text(s.why_not),
        free: Number.isFinite(free) && free > 0 ? Math.floor(free) : 0
      };
    });
    return {
      plot: plot,
      starts: starts,
      chose: game.chose === true,
      refused: text(game.refused),
      message: text(game.message)
    };
  }

  // A refusal that says nothing of the home or the starts (the server too busy, say) keeps what
  // was shown before, so the choices do not vanish under its sentence.
  function merged(answer, before) {
    if (answer.refused && !answer.plot && !answer.starts.length && before) {
      return { plot: before.plot, starts: before.starts, chose: false, refused: answer.refused, message: answer.message };
    }
    return answer;
  }

  // The start a held plot is, by its id in the answer; else the plot's kind as the server named it.
  function heldName(view) {
    var p = view.plot;
    for (var i = 0; i < view.starts.length; i++) {
      if (view.starts[i].id === p.start) return { name: view.starts[i].name, sentence: view.starts[i].sentence };
    }
    return { name: (p.kind || p.start || 'home').replace(/[_-]/g, ' '), sentence: '' };
  }

  function startName(id) {
    var list = shown ? shown.starts : [];
    for (var i = 0; i < list.length; i++) if (list[i].id === id) return list[i].name;
    return id;
  }

  // ── The window ──────────────────────────────────────────────────────────────

  function el(tag, cls, words) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (words !== undefined) e.textContent = words;
    return e;
  }

  // Built with createElement and textContent only: the answer's words never pass through innerHTML.
  function buildOverlay() {
    var overlay = el('div', 'profile-modal-overlay');
    overlay.id = 'where-overlay';
    var modal = el('div', 'profile-modal');
    modal.setAttribute('role', 'dialog');
    modal.setAttribute('aria-label', WORDS.title);
    var close = el('button', 'close-btn', '×');
    close.id = 'where-close';
    close.setAttribute('aria-label', WORDS.close);
    close.addEventListener('click', closeWhereYouLive);
    modal.appendChild(close);
    modal.appendChild(el('h2', '', WORDS.title));
    modal.appendChild(el('p', 'gameadmin-hint', WORDS.intro));
    var body = el('div');
    body.id = 'where-body';
    modal.appendChild(body);
    modal.appendChild(el('p', 'gameadmin-hint', WORDS.note));
    var bar = el('div', 'gameadmin-toolbar');
    var refresh = el('button', 'gameadmin-refresh-btn', WORDS.refresh);
    refresh.id = 'where-refresh';
    refresh.addEventListener('click', requestHome);
    bar.appendChild(refresh);
    modal.appendChild(bar);
    var status = el('div', 'gameadmin-status');
    status.id = 'where-status';
    status.setAttribute('aria-live', 'polite'); // the server's answer is read out when it comes
    modal.appendChild(status);
    overlay.appendChild(modal);
    overlay.addEventListener('click', function (e) { if (e.target === overlay) closeWhereYouLive(); });
    document.body.appendChild(overlay);
    return overlay;
  }

  function connected() {
    return typeof ws !== 'undefined' && ws && ws.readyState === WebSocket.OPEN;
  }

  function waitForAnswer() {
    if (waitTimer) clearTimeout(waitTimer);
    waitTimer = setTimeout(function () {
      waitTimer = null;
      if (!asking) return;
      asking = null;
      renderWhereYouLive();
      setStatus(WORDS.noAnswer);
    }, NO_ANSWER_MS);
  }

  function setStatus(t) {
    var s = document.getElementById('where-status');
    if (s) s.textContent = t || '';
  }

  // Where do I live here, and what is free?
  function requestHome() {
    if (!connected()) { setStatus(WORDS.notConnected); return; }
    ws.send(JSON.stringify({ type: HOME_REQUEST }));
    asking = 'request';
    setStatus(WORDS.asking);
    waitForAnswer();
  }

  // [Choose]: a home of this start, by its id (never its name, which is only words).
  function chooseHome(id) {
    if (!connected()) { setStatus(WORDS.notConnected); return; }
    ws.send(JSON.stringify({ type: CHOOSE_HOME, start: id }));
    asking = id;
    renderWhereYouLive();
    waitForAnswer();
  }

  // One row per start: its name and sentence; for one that can be chosen, how many are free and
  // [Choose]; for one that cannot, greyed, with the server's reason.
  function startRow(s) {
    var row = el('div', 'gameadmin-row');
    row.setAttribute('data-start', s.id);
    row.style.flexWrap = 'wrap'; // on a phone the count and Choose go under the words
    var words = el('div');
    words.style.flex = '1 1 14em';
    words.appendChild(el('strong', '', s.name));
    if (s.sentence) words.appendChild(el('p', 'gameadmin-hint', s.sentence));
    row.appendChild(words);
    if (!s.offered) {
      row.style.opacity = '0.6';
      if (s.why_not) words.appendChild(el('p', 'gameadmin-hint', s.why_not));
      return row;
    }
    // The count and Choose stay together when a narrow screen wraps them under the words.
    var side = el('div');
    side.style.display = 'flex';
    side.style.alignItems = 'center';
    side.style.gap = 'var(--space-md)';
    side.appendChild(el('span', 'gameadmin-count', freeText(s.free)));
    var btn = el('button', 'gameadmin-refresh-btn', WORDS.choose);
    btn.setAttribute('data-start', s.id);
    btn.setAttribute('aria-label', WORDS.choose + ' ' + aOrAn(s.name));
    btn.disabled = s.free === 0 || asking !== null;
    btn.addEventListener('click', function () { chooseHome(s.id); });
    side.appendChild(btn);
    row.appendChild(side);
    return row;
  }

  // Paint the last answer: the home held here, or the choice; and the status line.
  function renderWhereYouLive(game) {
    if (game !== undefined) {
      var answer = readAnswer(game);
      if (!answer) return;
      shown = merged(answer, shown);
      asking = null;
      if (waitTimer) { clearTimeout(waitTimer); waitTimer = null; }
    }
    var body = document.getElementById('where-body');
    if (!body) return; // the window is not built yet; the answer is kept for when it is
    body.textContent = '';
    var status = '';
    if (shown) {
      if (shown.plot) {
        var held = heldName(shown);
        body.appendChild(el('h3', 'fleet-headline', WORDS.held + aOrAn(held.name) + ', ' + shown.plot.id + '.'));
        if (held.sentence) body.appendChild(el('p', 'gameadmin-hint', held.sentence));
      } else {
        body.appendChild(el('h3', 'fleet-headline', WORDS.none));
        if (shown.starts.length) {
          shown.starts.forEach(function (s) { body.appendChild(startRow(s)); });
          var choosing = el('p', 'gameadmin-hint', WORDS.choosing);
          choosing.style.marginTop = 'var(--space-sm)';
          body.appendChild(choosing);
        } else if (!shown.refused) {
          body.appendChild(el('p', 'gameadmin-hint', WORDS.noneOffered));
        }
      }
      if (shown.refused) status = shown.message || WORDS.refusedNoWords;
      else if (shown.chose && shown.plot) status = WORDS.chosen;
    }
    if (asking === 'request') status = WORDS.asking;
    else if (asking) status = WORDS.askingFor + aOrAn(startName(asking)) + '...';
    setStatus(status);
  }

  function openWhereYouLive() {
    var overlay = document.getElementById('where-overlay') || buildOverlay();
    overlay.classList.add('open');
    renderWhereYouLive();
    requestHome();
  }

  function closeWhereYouLive() {
    var overlay = document.getElementById('where-overlay');
    if (overlay) overlay.classList.remove('open');
  }

  window.openWhereYouLive = openWhereYouLive;
  window.closeWhereYouLive = closeWhereYouLive;
  window.renderWhereYouLive = renderWhereYouLive;
})();
