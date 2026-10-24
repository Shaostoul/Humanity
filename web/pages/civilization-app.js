/* Civilization page logic, fetches /api/civilization stats and renders dashboards */
(function() {
  'use strict';

  let lastFetch = 0;
  const REFRESH_INTERVAL = 60000; // 60 seconds
  let refreshTimer = null;

  // ── Animated counter ──────────────────────────────────────
  function animateCounter(el, target, duration) {
    const start = parseInt(el.textContent) || 0;
    const diff = target - start;
    if (diff === 0) { el.textContent = target; return; }
    const startTime = performance.now();
    duration = duration || 800;
    function step(now) {
      const elapsed = now - startTime;
      const progress = Math.min(elapsed / duration, 1);
      const eased = 1 - Math.pow(1 - progress, 3); // ease-out cubic
      el.textContent = Math.round(start + diff * eased).toLocaleString();
      if (progress < 1) requestAnimationFrame(step);
    }
    requestAnimationFrame(step);
  }

  // ── Render real-mode dashboard ────────────────────────────
  function renderReal(data) {
    const grid = document.getElementById('real-grid');
    if (!grid) return;

    const p = data.population || {};
    const inf = data.infrastructure || {};
    const eco = data.economy || {};
    const res = data.resources || {};
    const soc = data.social || {};
    const act = data.activity || {};

    const roles = p.roles || {};
    const roleText = Object.entries(roles).map(([r, c]) => c + ' ' + r).join(', ') || i18n.t('civilization.web_roles_none');

    grid.innerHTML =
      civCard(i18n.t('civilization.web_population'), p.total_members || 0, i18n.t('civilization.web_unit_people'), [
        i18n.t('civilization.web_online_now', { n: p.online_now || 0 }),
        i18n.t('civilization.web_joined_week', { n: p.new_this_week || 0 }),
        i18n.t('civilization.web_roles', { roles: roleText })
      ], '#27ae60') +
      civCard(i18n.t('civilization.web_infrastructure'), inf.channels || 0, i18n.t('civilization.web_unit_channels'), [
        i18n.t('civilization.web_voice_channels', { n: inf.voice_channels || 0 }),
        i18n.t('civilization.web_projects', { n: inf.projects || 0 }),
        i18n.t('civilization.web_total_messages', { n: (inf.total_messages || 0).toLocaleString() })
      ], 'var(--nav-blue)') +
      civCard(i18n.t('civilization.web_economy'), eco.active_listings || 0, i18n.t('civilization.web_unit_listings'), [
        i18n.t('civilization.web_trades', { n: eco.total_trades || 0 }),
        i18n.t('civilization.web_reviews', { n: eco.total_reviews || 0 })
      ], '#e67e22') +
      civCard(i18n.t('civilization.web_resources'), res.total_tasks || 0, i18n.t('civilization.web_unit_tasks'), [
        i18n.t('civilization.web_completed', { n: res.tasks_completed || 0 }),
        i18n.t('civilization.web_in_progress', { n: res.tasks_in_progress || 0 }),
        i18n.t('civilization.web_open', { n: res.tasks_open || 0 })
      ], '#9b59b6') +
      civCard(i18n.t('civilization.web_social'), soc.total_follows || 0, i18n.t('civilization.web_unit_connections'), [
        i18n.t('civilization.web_direct_messages', { n: soc.total_dms || 0 })
      ], 'var(--nav-red)') +
      civCard(i18n.t('civilization.activity'), act.messages_today || 0, i18n.t('civilization.web_unit_messages_today'), [
        i18n.t('civilization.web_most_active', { channel: act.most_active_channel || 'general' }),
        i18n.t('civilization.web_peak_online', { n: act.peak_online || 0 })
      ], '#1abc9c');

    // Animate all stat numbers
    grid.querySelectorAll('.civ-stat-num').forEach(el => {
      const target = parseInt(el.dataset.target) || 0;
      animateCounter(el, target, 800);
    });
  }

  // ── Card HTML builder ─────────────────────────────────────
  // Every word here is text, never markup: the stats carry names that came from
  // the server (a channel's name, the roles), so they are escaped on the way in
  // (2026-10-23, found while the page's words moved to data/i18n/).
  function escText(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }
  function civCard(title, mainStat, unit, subStats, accentColor) {
    const subs = subStats.map(s => '<div class="civ-sub">' + escText(s) + '</div>').join('');
    return '<div class="civ-card">' +
      '<div class="civ-card-accent" style="background:' + accentColor + '"></div>' +
      '<h3 class="civ-card-title">' + escText(title) + '</h3>' +
      '<div class="civ-stat">' +
        '<span class="civ-stat-num" data-target="' + (parseInt(mainStat, 10) || 0) + '">0</span>' +
        '<span class="civ-stat-unit">' + escText(unit) + '</span>' +
      '</div>' +
      '<div class="civ-subs">' + subs + '</div>' +
    '</div>';
  }

  // ── Fetch data ────────────────────────────────────────────
  async function fetchCivData() {
    try {
      const resp = await fetch('/api/civilization');
      if (!resp.ok) throw new Error('HTTP ' + resp.status);
      const data = await resp.json();
      lastFetch = Date.now();
      renderReal(data);
      updateTimestamp();
    } catch (err) {
      console.warn('Failed to fetch civilization stats:', err);
    }
  }

  function updateTimestamp() {
    const el = document.getElementById('civ-last-updated');
    if (el) {
      const ago = Math.round((Date.now() - lastFetch) / 1000);
      el.textContent = ago < 5 ? i18n.t('civilization.just_now') : i18n.t('civilization.seconds_ago', { n: ago });
    }
  }

  // ── Init ──────────────────────────────────────────────────
  // This is a real-life page (the live community dashboard). Game/colony
  // stats live inside the desktop app, not behind a web toggle. See
  // docs/design/two-realities.md.
  document.addEventListener('DOMContentLoaded', function() {
    fetchCivData();

    // Auto-refresh
    refreshTimer = setInterval(function() {
      fetchCivData();
      updateTimestamp();
    }, REFRESH_INTERVAL);

    // Update "ago" text every 10s
    setInterval(updateTimestamp, 10000);

    // Manual refresh button
    const btn = document.getElementById('civ-refresh');
    if (btn) btn.addEventListener('click', fetchCivData);
  });
})();
