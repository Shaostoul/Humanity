/**
 * The first five minutes, on the website (the ELI5 mandate, 2026-10-22): the
 * same short guided tour the desktop app gives a new person (src/gui/first_minutes.rs),
 * from the same steps (data/onboarding/tour.json; each step's words are keys in
 * data/i18n/). The recovery phrase written down, a first message, the first
 * steps in the world, and where help lives.
 *
 * A small card in the corner, never a screen-wide catcher: the tour used to put
 * a full-viewport overlay over the page and took the first tap anywhere (see the
 * note at the end), so the card leaves the page usable, and a step that points
 * at something rings it without blocking it.
 *
 * Opt-in, as it has been since that bug: the help button (?) and the footer's
 * "Take Tour" start it (window.startOnboardingTour). RESUMABLE: where the person
 * is lives in localStorage (hos_first_minutes), so "Take me there" can open
 * another page and the card carries on there; "Hide for now" puts it away until
 * the next start, which resumes rather than restarts; "Skip the tour" ends it.
 */
(function () {
  if (window.__HOS_TOUR_INIT__) return;
  window.__HOS_TOUR_INIT__ = true;

  var KEY = 'hos_first_minutes';
  var steps = [];
  var stepsLoaded = fetch('/data/onboarding/tour.json')
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (d) { if (d && Array.isArray(d.steps)) steps = d.steps; })
    .catch(function () { /* offline: the card says it cannot load */ });

  function T(key, vars) {
    return window.i18n && typeof window.i18n.t === 'function' ? window.i18n.t(key, vars) : key;
  }
  function read() {
    try { return JSON.parse(localStorage.getItem(KEY)) || {}; } catch (_) { return {}; }
  }
  function write(s) {
    try { localStorage.setItem(KEY, JSON.stringify(s)); } catch (_) { /* private window */ }
  }

  var card = null;
  var ring = null;

  function injectStyles() {
    if (document.getElementById('hos-tour-styles')) return;
    var st = document.createElement('style');
    st.id = 'hos-tour-styles';
    st.textContent =
      '#hos-tour-card{position:fixed;right:16px;bottom:calc(56px + var(--hos-wip-h, 0px));z-index:9500;width:min(340px,calc(100vw - 32px));' +
      'background:var(--bg-card);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-lg,10px);' +
      'padding:16px;box-shadow:0 8px 32px rgba(0,0,0,0.4);font-family:inherit;}' +
      '#hos-tour-card .tour-count{font-size:0.75rem;color:var(--text-muted);}' +
      '#hos-tour-card .tour-title{font-size:1.05rem;font-weight:700;color:var(--accent);margin:2px 0 6px;}' +
      '#hos-tour-card .tour-text{font-size:0.9rem;line-height:1.5;margin-bottom:10px;}' +
      '#hos-tour-card .tour-done{font-size:0.85rem;color:var(--success);margin-bottom:8px;}' +
      '#hos-tour-card .tour-row{display:flex;flex-wrap:wrap;gap:8px;margin-top:6px;}' +
      '#hos-tour-card button{min-height:36px;padding:6px 12px;border-radius:var(--radius,6px);font:inherit;font-size:0.85rem;cursor:pointer;' +
      'border:1px solid var(--border);background:var(--bg-tertiary);color:var(--text);}' +
      '#hos-tour-card button.primary{background:var(--accent);border-color:var(--accent);color:var(--text-on-accent);font-weight:600;}' +
      '#hos-tour-card button.quiet{background:transparent;border-color:transparent;color:var(--text-muted);}' +
      '#hos-tour-card button:focus-visible{outline:2px solid var(--border-focus);outline-offset:2px;}' +
      '#hos-tour-ring{position:fixed;z-index:9400;pointer-events:none;border:3px solid var(--accent);border-radius:var(--radius,6px);' +
      'box-shadow:0 0 0 4px rgba(0,0,0,0.25);}' +
      '@media (prefers-reduced-motion: no-preference){#hos-tour-card{transition:opacity 0.2s;}}';
    document.head.appendChild(st);
  }

  function button(cls, key, hintKey, onClick) {
    var b = document.createElement('button');
    b.type = 'button';
    if (cls) b.className = cls;
    b.textContent = T(key);
    if (window.i18n && typeof window.i18n.hint === 'function') window.i18n.hint(b, hintKey);
    b.addEventListener('click', onClick);
    return b;
  }

  function close() {
    if (card && card.parentNode) card.parentNode.removeChild(card);
    if (ring && ring.parentNode) ring.parentNode.removeChild(ring);
    card = null;
    ring = null;
  }

  function pointAt(sel) {
    if (ring && ring.parentNode) ring.parentNode.removeChild(ring);
    ring = null;
    if (!sel) return;
    var el = document.querySelector('.hub-nav ' + sel) || document.querySelector(sel);
    if (!el) return;
    var r = el.getBoundingClientRect();
    ring = document.createElement('div');
    ring.id = 'hos-tour-ring';
    ring.style.left = (r.left - 4) + 'px';
    ring.style.top = (r.top - 4) + 'px';
    ring.style.width = (r.width + 8) + 'px';
    ring.style.height = (r.height + 8) + 'px';
    document.body.appendChild(ring);
  }

  /** Is this the page a step points at (its path; a #sec- part is a Settings section)? */
  function onPage(page) {
    var path = String(page || '').split('#')[0];
    return !!path && location.pathname.replace(/\.html$/, '') === path;
  }
  /** Settings has no address per section, so open the step's section by hand. */
  function openSection(page) {
    var m = /#sec-([\w-]+)$/.exec(String(page || ''));
    if (m && typeof window.showSection === 'function') window.showSection(m[1]);
  }

  /** Has the website seen this step done? */
  function stepDone(state, step) {
    if (step.done_when === 'phrase_written') return !!state.phrase_written;
    return false;
  }

  function render() {
    var state = read();
    if (!steps.length || !state.started || state.hidden || state.done) { close(); return; }
    if (state.step >= steps.length) { state.done = true; write(state); close(); return; }
    var step = steps[state.step];
    injectStyles();
    if (!card) {
      card = document.createElement('div');
      card.id = 'hos-tour-card';
      card.setAttribute('role', 'dialog');
      card.setAttribute('aria-live', 'polite');
      document.body.appendChild(card);
    }
    while (card.firstChild) card.removeChild(card.firstChild);
    card.setAttribute('aria-label', T('tour.card_name'));

    var count = document.createElement('div');
    count.className = 'tour-count';
    count.textContent = T('tour.counter', { n: state.step + 1, total: steps.length });
    var title = document.createElement('div');
    title.className = 'tour-title';
    title.textContent = T(step.title);
    var text = document.createElement('div');
    text.className = 'tour-text';
    // A step may say something else on the website (the world is in the desktop app).
    var textKey = (step.web && step.web.text) || step.text;
    if (window.i18n && typeof window.i18n.fill === 'function') window.i18n.fill(text, textKey);
    else text.textContent = T(textKey);
    card.appendChild(count);
    card.appendChild(title);
    card.appendChild(text);
    if (stepDone(state, step)) {
      var done = document.createElement('div');
      done.className = 'tour-done';
      done.textContent = T('tour.step_done');
      card.appendChild(done);
    }

    var row1 = document.createElement('div');
    row1.className = 'tour-row';
    var web = step.web || {};
    if (web.page) {
      row1.appendChild(button('', 'tour.take_me_there', 'tour.take_me_there_hint', function () {
        if (onPage(web.page)) { openSection(web.page); pointAt(web.target); }
        else location.href = web.page;
      }));
    }
    if (step.done_when === 'phrase_written' && !stepDone(state, step)) {
      row1.appendChild(button('', 'tour.phrase_written', 'tour.phrase_written_hint', function () {
        var s = read(); s.phrase_written = true; write(s); render();
      }));
    }
    if (row1.childNodes.length) card.appendChild(row1);

    var row2 = document.createElement('div');
    row2.className = 'tour-row';
    if (state.step > 0) {
      row2.appendChild(button('quiet', 'tour.back', 'tour.back_hint', function () {
        var s = read(); s.step = Math.max(0, s.step - 1); write(s); render();
      }));
    }
    var last = state.step + 1 === steps.length;
    row2.appendChild(button('primary', last ? 'tour.finish' : 'tour.next', last ? 'tour.finish_hint' : 'tour.next_hint', function () {
      var s = read();
      if (last) s.done = true; else s.step += 1;
      write(s);
      render();
    }));
    card.appendChild(row2);

    var row3 = document.createElement('div');
    row3.className = 'tour-row';
    row3.appendChild(button('quiet', 'tour.hide', 'tour.hide_hint', function () {
      var s = read(); s.hidden = true; write(s); close();
    }));
    row3.appendChild(button('quiet', 'tour.skip', 'tour.skip_hint', function () {
      var s = read(); s.done = true; write(s); close();
    }));
    card.appendChild(row3);

    // Landed here from Take me there: open the step's Settings section once.
    if (onPage(web.page) && !openedOnce) { openedOnce = true; openSection(web.page); }
    pointAt(web.target && onPage(web.page) ? web.target : null);
  }
  var openedOnce = false;

  /** Start (or resume) the tour: from the help button, the footer link, or a page's own button. */
  function startTour() {
    stepsLoaded.then(function () {
      var s = read();
      if (!s.started || s.done) s = { started: true, step: 0, phrase_written: !!s.phrase_written };
      s.hidden = false;
      write(s);
      render();
    });
  }
  window.startOnboardingTour = startTour;

  // A tour in progress carries on on every page (resumable across "Take me
  // there"); a new visitor sees nothing until they ask.
  stepsLoaded.then(render);
  window.addEventListener('languagechange', render);
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && card) { var s = read(); s.hidden = true; write(s); close(); }
  });

  // ── Opt-in, always (kept from the tour this replaces) ──
  // The old tour fired 2 s after load for every first-time visitor, on every page,
  // and its overlay was a full-viewport click catcher, so it stole the first tap
  // anywhere on the page. The tour is reached deliberately: window.startOnboardingTour(),
  // the help panel, and the footer's "Take Tour". The card never covers the page.
})();
