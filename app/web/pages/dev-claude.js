/* The website's Dev > Claude (2026-10-22): the web twin of the desktop app's Dev > Claude
 * (src/gui/pages/dev_agents/), the Claude tab of the Dev Workbench (web/pages/dev.html).
 *
 * A website cannot read the developer's computer, so this shows ONLY what it can reach, and
 * says so plainly instead of faking the rest:
 *   - GitHub's newest release and newest commit on main (api.github.com, allowed by the site's
 *     connect-src);
 *   - the TIER 0 headings of GitHub's copy of docs/PRIORITIES.md;
 *   - the newest three journal decisions, from the copy of
 *     data/coordination/orchestrator_state.json this site serves (as of its last update).
 * Worktrees, uncommitted changes, the taskbar exe, the share guard, the disk guard and the boot
 * lock are named as desktop-only (dev_claude.web_pc_only).
 *
 * Read-only, and lazy: nothing is fetched until the Claude tab is opened, then only on Refresh
 * (three GitHub requests a load, well inside GitHub's 60 an hour for a visitor).
 *
 * The pure parts (journalTail, tier0Headings, ago) mirror the Rust readers in
 * src/gui/pages/dev_agents/read.rs and are tested in scripts/tests/dev-claude-web.test.js against
 * the same fixtures, so the two cannot drift apart unseen. The words are data/i18n/en.json's
 * dev_claude section, the same keys the app uses.
 */
(function (root) {
  'use strict';

  var REPO = 'https://api.github.com/repos/Shaostoul/Humanity';
  var JOURNAL = '/data/coordination/orchestrator_state.json';

  /** Every run of whitespace, line breaks included, as one space. */
  function oneLine(s) {
    return String(s || '').split(/\s+/).filter(Boolean).join(' ');
  }

  /** The newest n journal decisions, newest first, each { date, text } on one line. Throws when
   *  the journal has no recent_decisions list (said, not shown as an empty journal). */
  function journalTail(json, n) {
    var j = typeof json === 'string' ? JSON.parse(json) : json;
    var list = j && j.recent_decisions;
    if (!Array.isArray(list)) throw new Error('it has no recent_decisions list');
    return list.slice(-n).reverse().map(function (d) {
      return { date: String(d.date || d.at || '?'), text: oneLine(d.decision) };
    });
  }

  /** The ### headings under "## TIER 0", in order, at most max. */
  function tier0Headings(md, max) {
    var inside = false;
    var out = [];
    var lines = String(md || '').split('\n');
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i].replace(/\s+$/, '');
      if (line.indexOf('## ') === 0) {
        if (inside) break;
        inside = line.slice(3).replace(/^\s+/, '').indexOf('TIER 0') === 0;
        continue;
      }
      if (inside && line.indexOf('### ') === 0) {
        out.push(line.slice(4).trim());
        if (out.length >= max) break;
      }
    }
    return out;
  }

  /** "40 s ago", "12 min ago", "3 h ago", "2 days ago", with the app's thresholds. */
  function ago(thenMs, nowMs, T) {
    var secs = Math.floor((nowMs - thenMs) / 1000);
    if (secs < 5) return T('just_now');
    if (secs < 120) return T('ago_s', { n: secs });
    if (secs < 2 * 3600) return T('ago_m', { n: Math.floor(secs / 60) });
    if (secs < 2 * 86400) return T('ago_h', { n: Math.floor(secs / 3600) });
    return T('ago_d', { n: Math.floor(secs / 86400) });
  }

  // ── The words ──────────────────────────────────────────────────────────────────────────────

  var english = null; // en.json's dev_claude section, fetched by load() as the fallback

  function fill(s, vars) {
    return String(s).replace(/\{\{(\w+)\}\}/g, function (m, name) {
      return vars && vars[name] !== undefined ? String(vars[name]) : m;
    });
  }

  /** dev_claude.<key> in the visitor's language, else English, else the key itself. */
  function T(key, vars) {
    var full = 'dev_claude.' + key;
    if (root.i18n && typeof root.i18n.t === 'function') {
      var v = root.i18n.t(full, vars);
      if (v && v !== full) return v;
    }
    if (english && typeof english[key] === 'string') return fill(english[key], vars);
    return full;
  }

  // ── Fetching and drawing ───────────────────────────────────────────────────────────────────

  function getJson(url) {
    return fetch(url, { headers: { Accept: 'application/vnd.github+json' } }).then(function (r) {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.json();
    });
  }

  function getText(url, accept) {
    return fetch(url, { headers: { Accept: accept } }).then(function (r) {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.text();
    });
  }

  /** Each reading settles to { ok: value } or { err: message }; one failure never hides the rest. */
  function settle(p) {
    return p.then(function (v) { return { ok: v }; }, function (e) { return { err: String((e && e.message) || e) }; });
  }

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text; // never innerHTML: commit and journal text is data
    return e;
  }

  function card(title, hint) {
    var c = el('section', 'claude-card');
    c.appendChild(el('h2', null, title));
    if (hint) c.appendChild(el('p', 'claude-hint', hint));
    return c;
  }

  function row(parent, state, label, text) {
    var r = el('div', 'claude-row');
    r.appendChild(el('span', 'claude-dot claude-' + state));
    if (label) r.appendChild(el('span', 'claude-label', label + ':'));
    r.appendChild(el('span', 'claude-text', text));
    parent.appendChild(r);
  }

  function draw(box, d, nowMs) {
    box.textContent = '';
    var head = el('div', 'claude-head');
    head.appendChild(el('p', 'claude-hint', T('web_pc_only')));
    var btn = el('button', 'claude-refresh', T('web_refresh'));
    btn.type = 'button';
    btn.addEventListener('click', function () { load(box); });
    head.appendChild(btn);
    box.appendChild(head);

    // Version: the newest release and the newest commit on main.
    var v = card(T('version'), T('web_release_hint'));
    if (d.release.ok) {
      var rel = d.release.ok;
      row(v, 'muted', T('latest_release'), T('published', { tag: rel.tag_name, ago: ago(Date.parse(rel.published_at), nowMs, T) }));
    } else {
      row(v, 'muted', T('latest_release'), T('web_unreachable', { why: d.release.err }));
    }
    if (d.main.ok) {
      var c = d.main.ok.commit || {};
      var when = c.committer && c.committer.date ? ago(Date.parse(c.committer.date), nowMs, T) + ': ' : '';
      row(v, 'muted', T('web_main'), when + oneLine(String(c.message || '').split('\n')[0]));
    } else {
      row(v, 'muted', T('web_main'), T('web_unreachable', { why: d.main.err }));
    }
    box.appendChild(v);

    // The journal, as the site serves it.
    var j = card(T('journal'), T('web_journal_hint'));
    if (d.journal.ok) {
      d.journal.ok.forEach(function (x) {
        var r = el('div', 'claude-line');
        r.appendChild(el('span', 'claude-date', x.date));
        r.appendChild(el('span', 'claude-text', x.text));
        r.title = x.text;
        j.appendChild(r);
      });
    } else {
      j.appendChild(el('p', 'claude-hint', T('web_unreachable', { why: d.journal.err })));
    }
    box.appendChild(j);

    // TIER 0, from GitHub's copy of the priorities.
    var t = card(T('tier0'), T('web_tier0_hint'));
    if (d.tier0.ok) {
      var ol = el('ol', 'claude-tier0');
      d.tier0.ok.forEach(function (h) {
        var li = el('li', null, h);
        li.title = h;
        ol.appendChild(li);
      });
      t.appendChild(ol);
    } else {
      t.appendChild(el('p', 'claude-hint', T('web_unreachable', { why: d.tier0.err })));
    }
    box.appendChild(t);
  }

  var loading = false;

  /** Fetch everything once and draw it into box. */
  function load(box) {
    if (loading) return Promise.resolve();
    loading = true;
    box.textContent = T('waiting');
    var words = english
      ? Promise.resolve()
      : fetch('/data/i18n/en.json').then(function (r) { return r.json(); }).then(function (en) { english = en.dev_claude || {}; }, function () {});
    return words.then(function () {
      box.textContent = T('waiting');
      return Promise.all([
        settle(getJson(REPO + '/releases/latest')),
        settle(getJson(REPO + '/commits/main')),
        settle(fetch(JOURNAL).then(function (r) {
          if (!r.ok) throw new Error('HTTP ' + r.status);
          return r.json();
        }).then(function (j) { return journalTail(j, 3); })),
        settle(getText(REPO + '/contents/docs/PRIORITIES.md?ref=main', 'application/vnd.github.raw').then(function (md) {
          return tier0Headings(md, 6);
        })),
      ]);
    }).then(function (r) {
      draw(box, { release: r[0], main: r[1], journal: r[2], tier0: r[3] }, Date.now());
    }).then(function () { loading = false; }, function () { loading = false; });
  }

  /** Wire the Claude tab of dev.html: load on first open, and open it for /dev#claude. */
  function wire(doc) {
    var box = doc.getElementById('claude-panel');
    var bar = doc.getElementById('tab-bar');
    if (!box || !bar) return;
    var loaded = false;
    function opened() {
      if (!loaded) { loaded = true; load(box); }
    }
    bar.addEventListener('click', function (e) {
      var b = e.target.closest('.tab-btn');
      if (b && b.dataset.tab === 'claude') opened();
    });
    function fromHash() {
      if (root.location && root.location.hash === '#claude') {
        var b = doc.querySelector('.tab-btn[data-tab="claude"]');
        if (b) b.click();
      }
    }
    root.addEventListener('hashchange', fromHash);
    fromHash();
  }

  var api = { oneLine: oneLine, journalTail: journalTail, tier0Headings: tier0Headings, ago: ago, fill: fill };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.DevClaude = api;
    if (root.document) {
      if (root.document.readyState === 'loading') root.document.addEventListener('DOMContentLoaded', function () { wire(root.document); });
      else wire(root.document);
    }
  }
})(typeof window !== 'undefined' ? window : this);
