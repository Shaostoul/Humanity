/**
 * Roadmap page. Renders data/roadmap.json, which is generated from docs/ROADMAP.md
 * (the single source of truth, also the maintainers' build to-do list) by
 * scripts/roadmap-to-json.js. Shows the "Right now" queue, the themed sections with
 * progress, and the recently-shipped list. Status badges: done / building / next /
 * planned / future.
 */
(function () {
  'use strict';

  let data = null;
  let activeFilter = 'all';
  const collapsed = {};

  const STATUS_LABEL = {
    done: 'done',
    building: 'building',
    next: 'next',
    planned: 'planned',
    future: 'future',
  };
  // Order for "is this filtered out": the filter matches a status directly.
  const FILTERS = [
    { key: 'all', label: 'All' },
    { key: 'building', label: 'Building' },
    { key: 'next', label: 'Next' },
    { key: 'planned', label: 'Planned' },
    { key: 'future', label: 'Future' },
    { key: 'done', label: 'Done' },
  ];

  async function load() {
    try {
      const res = await fetch('/data/roadmap.json', { cache: 'no-store' });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      data = await res.json();
    } catch (e) {
      console.warn('[roadmap] Failed to load roadmap.json:', e);
      const c = document.getElementById('tiers-container');
      if (c) c.innerHTML = '<div class="tier-empty">Could not load the roadmap right now.</div>';
      return;
    }
    buildFilterBar();
    render();
  }

  function buildFilterBar() {
    const bar = document.getElementById('filter-bar');
    if (!bar) return;
    bar.innerHTML = FILTERS.map(function (f) {
      return '<button class="filter-btn' + (f.key === 'all' ? ' active' : '') +
        '" data-filter="' + f.key + '">' + f.label + '</button>';
    }).join('');
  }

  function render() {
    const container = document.getElementById('tiers-container');
    if (!container || !data) return;

    // Overall progress across themed items.
    const done = (data.summary && data.summary.done) || 0;
    const total = (data.summary && data.summary.total) || 0;
    setText('overview-done', done);
    setText('overview-total', total);
    const fill = document.getElementById('overview-fill');
    if (fill) fill.style.width = (total ? (done / total) * 100 : 0) + '%';

    let html = '';

    // ── Right now ──
    if (data.now && data.now.length) {
      html += '<section class="now-section">';
      html += '<div class="now-header"><h2>Right now</h2>' +
        '<span class="now-sub">what is being worked on next</span></div>';
      html += '<ol class="now-list">';
      data.now.forEach(function (item) {
        html += '<li class="now-item">' + statusBadge(item.status) +
          '<span class="now-text">' + esc(item.text) + '</span></li>';
      });
      html += '</ol></section>';
    }

    // ── Themes ──
    (data.themes || []).forEach(function (theme, i) {
      const key = 'theme' + i;
      const pct = theme.total ? Math.round((theme.done / theme.total) * 100) : 0;
      const isCollapsed = collapsed[key];
      const filtered = (theme.items || []).filter(passesFilter);

      html += '<section class="tier-section">';
      html += '<div class="tier-header" onclick="window.__rmToggle(\'' + key + '\')">';
      html += '<span class="tier-chevron' + (isCollapsed ? ' collapsed' : '') + '">' + chevron() + '</span>';
      html += '<h2>' + esc(theme.title) + '</h2>';
      html += '<span class="tier-badge">' + pct + '%</span>';
      html += '</div>';

      if (theme.description) {
        html += '<p class="tier-desc">' + esc(theme.description) + '</p>';
      }

      html += '<div class="tier-progress">';
      html += '<div class="progress-wrap"><div class="progress-fill" style="width:' + pct +
        '%;background:var(--success)"></div></div>';
      html += '<span class="tier-count">' + theme.done + ' / ' + theme.total + '</span>';
      html += '</div>';

      if (!isCollapsed) {
        if (filtered.length === 0) {
          html += '<div class="tier-empty">Nothing here matches the current filter.</div>';
        } else {
          html += '<ul class="rm-items">';
          filtered.forEach(function (item) {
            html += '<li class="rm-item">' + statusBadge(item.status) +
              '<span class="rm-text">' + esc(item.text) +
              (item.version ? ' <span class="rm-ver">' + esc(item.version) + '</span>' : '') +
              '</span></li>';
          });
          html += '</ul>';
        }
      }
      html += '</section>';
    });

    // ── Recently shipped ──
    if (data.recent && data.recent.length && (activeFilter === 'all' || activeFilter === 'done')) {
      html += '<section class="tier-section recent-section">';
      html += '<div class="tier-header"><h2>Recently shipped</h2></div>';
      html += '<ul class="rm-items">';
      data.recent.forEach(function (item) {
        html += '<li class="rm-item">' +
          (item.version ? '<span class="rm-ver lead">' + esc(item.version) + '</span>' : '') +
          '<span class="rm-text">' + esc(item.text) + '</span></li>';
      });
      html += '</ul></section>';
    }

    container.innerHTML = html;
  }

  function passesFilter(item) {
    if (activeFilter === 'all') return true;
    return item.status === activeFilter;
  }

  function statusBadge(status) {
    const s = status || 'planned';
    return '<span class="status-badge ' + s + '">' + esc(STATUS_LABEL[s] || s) + '</span>';
  }

  function setText(id, v) {
    const el = document.getElementById(id);
    if (el) el.textContent = v;
  }

  function esc(s) {
    const d = document.createElement('div');
    d.textContent = s == null ? '' : s;
    return d.innerHTML;
  }

  function chevron() {
    return '<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4,2 12,8 4,14"/></svg>';
  }

  window.__rmToggle = function (key) {
    collapsed[key] = !collapsed[key];
    render();
  };

  const filterBar = document.getElementById('filter-bar');
  if (filterBar) {
    filterBar.addEventListener('click', function (e) {
      const btn = e.target.closest('.filter-btn');
      if (!btn) return;
      document.querySelectorAll('.filter-btn').forEach(function (b) { b.classList.remove('active'); });
      btn.classList.add('active');
      activeFilter = btn.dataset.filter;
      render();
    });
  }

  load();
})();
