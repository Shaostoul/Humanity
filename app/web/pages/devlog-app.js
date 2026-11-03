/* The Devlog page (web/pages/devlog.html): every release, newest first, one update card each, the
   same card the front door shows (web/shared/front-door.js updateCard), a signed release dressed
   its own way (docs/design/front-door.md, section 3.1; the operator, 2026-11-02: "Keep and update
   devlog too").

   WHERE THE LIST COMES FROM (2026-11-02): this server's GET /api/releases (src/relay/front_door.rs),
   the release posts of its #announcements channel a page at a time, each with whether the server
   checked a signature by the project's release keys. Until this day the page asked GitHub's
   releases API from every visitor's browser, which sent each visitor's address to GitHub (the
   reason the site's fonts are self-hosted) and allowed a browser 60 requests an hour. A visitor
   now talks only to this server; each card's Release notes link, which opens the full notes on
   GitHub, is the visitor's own choice to make. scripts/tests/front-door-web.test.js fails if this
   file or devlog.html names GitHub's API host again.

   Works in the browser (it starts itself) and under Node for that test (module.exports.createDevlog). */
(function (root) {
  'use strict';

  var NODE = typeof module === 'object' && module && module.exports;
  var PAGE = 25;

  /** env = { document, fetch, t, hint, icon, fd, now() }. */
  function createDevlog(env) {
    var doc = env.document;
    var T = env.t;
    var state = { next: null, shown: 0, loading: false };
    var list = doc.getElementById('devlog-list');
    var more = doc.getElementById('devlog-more');
    var status = doc.getElementById('devlog-status');

    function say(words) {
      status.textContent = words || '';
      status.hidden = !words;
    }

    function load() {
      if (state.loading) return Promise.resolve();
      state.loading = true;
      var url = '/api/releases?limit=' + PAGE + (state.next ? '&before=' + encodeURIComponent(state.next) : '');
      return env.fetch(url, { cache: 'no-cache' })
        .then(function (r) {
          if (!r.ok) throw new Error('HTTP ' + r.status);
          return r.json();
        })
        .then(function (j) {
          var deps = { document: doc, t: T, hint: env.hint, icon: env.icon, now: env.now ? env.now() : Date.now() };
          (j.releases || []).forEach(function (u) {
            if (!u || typeof u.title !== 'string') return;
            list.appendChild(env.fd.updateCard(u, deps));
            state.shown++;
          });
          state.next = j.next == null ? null : j.next;
          more.hidden = state.next == null;
          say(state.shown ? '' : T('devlog.none'));
        })
        .catch(function () {
          if (!state.shown) say(T('devlog.offline'));
        })
        .then(function () { state.loading = false; });
    }

    function start() {
      more.addEventListener('click', function () { load(); });
      say(T('devlog.loading'));
      return load();
    }

    return { start: start, load: load, state: state };
  }

  function bootInBrowser(win) {
    var doc = win.document;
    var i18n = win.i18n;
    var go = function () {
      var page = createDevlog({
        document: doc, fetch: function (u, i) { return win.fetch(u, i); },
        t: function (k, v) { return i18n ? i18n.t(k, v) : k; },
        hint: function (e, k) { if (i18n) i18n.hint(e, k); },
        icon: win.hosIcon ? function (n, s) { return win.hosIcon(n, s); } : null,
        fd: win.HosFrontDoor, now: function () { return Date.now(); }
      });
      page.start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  if (NODE) module.exports = { createDevlog: createDevlog };
  else bootInBrowser(root);
})(typeof window !== 'undefined' ? window : globalThis);
