/* Devlog page: renders the GitHub Releases feed as the public devlog.
   The release history is the project's only authoritative changelog (the
   Version SOP tags and writes notes for every ship), so rendering it live
   means this page can never drift from reality and needs no build step.
   Unauthenticated GitHub API allows 60 requests/hour/IP; a sessionStorage
   cache keeps a browsing session to ~1 request per page of releases. */
(function() {
  'use strict';

  var API = 'https://api.github.com/repos/Shaostoul/Humanity/releases';
  var PER_PAGE = 25;
  var CACHE_MS = 10 * 60 * 1000;

  var page = 1;
  var list = document.getElementById('devlog-list');
  var moreBtn = document.getElementById('devlog-more');

  function esc(s) {
    return String(s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function md(text) {
    if (window.hosMarkdown && window.hosMarkdown.render) return window.hosMarkdown.render(text);
    return '<p>' + esc(text).replace(/\n\n/g, '</p><p>').replace(/\n/g, '<br>') + '</p>';
  }

  function fmtDate(iso) {
    try {
      return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
    } catch (e) { return (iso || '').slice(0, 10); }
  }

  function render(releases) {
    if (page === 1) list.innerHTML = '';
    if (!releases.length && page === 1) {
      list.innerHTML = '<div class="devlog-empty">No releases visible right now. ' +
        'See the <a href="https://github.com/Shaostoul/Humanity/releases">GitHub releases page</a>.</div>';
      return;
    }
    var frag = document.createDocumentFragment();
    releases.forEach(function(r) {
      if (r.draft) return;
      var el = document.createElement('article');
      el.className = 'rel';
      el.innerHTML =
        '<div class="rel-head">' +
          '<h3><a href="' + esc(r.html_url) + '" target="_blank" rel="noopener">' +
            esc(r.name || r.tag_name) + '</a></h3>' +
          '<span class="rel-date">' + fmtDate(r.published_at) + '</span>' +
        '</div>' +
        (r.body ? '<div class="rel-body">' + md(r.body) + '</div>' : '');
      frag.appendChild(el);
    });
    list.appendChild(frag);
    moreBtn.style.display = releases.length >= PER_PAGE ? 'block' : 'none';
  }

  function load() {
    var key = 'devlog-page-' + page;
    try {
      var cached = JSON.parse(sessionStorage.getItem(key) || 'null');
      if (cached && Date.now() - cached.at < CACHE_MS) { render(cached.data); return; }
    } catch (e) {}

    fetch(API + '?per_page=' + PER_PAGE + '&page=' + page, {
      headers: { 'Accept': 'application/vnd.github+json' },
    })
      .then(function(r) {
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.json();
      })
      .then(function(data) {
        try { sessionStorage.setItem(key, JSON.stringify({ at: Date.now(), data: data })); } catch (e) {}
        render(data);
      })
      .catch(function(err) {
        console.error('devlog: release fetch failed', err);
        if (page === 1) {
          list.innerHTML = '<div class="devlog-empty">Could not load the release feed ' +
            '(possibly rate-limited). The <a href="https://github.com/Shaostoul/Humanity/releases">' +
            'GitHub releases page</a> always works.</div>';
        }
      });
  }

  moreBtn.addEventListener('click', function() { page += 1; load(); });
  load();
})();
