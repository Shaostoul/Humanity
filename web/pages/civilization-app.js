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
    const roleText = Object.entries(roles).map(([r, c]) => c + ' ' + r).join(', ') || 'None';

    grid.innerHTML =
      civCard('Population', p.total_members || 0, 'people', [
        (p.online_now || 0) + ' online now',
        (p.new_this_week || 0) + ' joined this week',
        'Roles: ' + roleText
      ], '#27ae60') +
      civCard('Infrastructure', inf.channels || 0, 'channels', [
        (inf.voice_channels || 0) + ' voice channels',
        (inf.projects || 0) + ' projects',
        (inf.total_messages || 0).toLocaleString() + ' total messages'
      ], '#3498db') +
      civCard('Economy', eco.active_listings || 0, 'listings', [
        (eco.total_trades || 0) + ' trades',
        (eco.total_reviews || 0) + ' reviews'
      ], '#e67e22') +
      civCard('Resources', res.total_tasks || 0, 'tasks', [
        (res.tasks_completed || 0) + ' completed',
        (res.tasks_in_progress || 0) + ' in progress',
        (res.tasks_open || 0) + ' open'
      ], '#9b59b6') +
      civCard('Social', soc.total_follows || 0, 'connections', [
        (soc.total_dms || 0) + ' direct messages'
      ], '#e74c3c') +
      civCard('Activity', act.messages_today || 0, 'messages today', [
        'Most active: #' + (act.most_active_channel || 'general'),
        'Peak online: ' + (act.peak_online || 0)
      ], '#1abc9c');

    // Animate all stat numbers
    grid.querySelectorAll('.civ-stat-num').forEach(el => {
      const target = parseInt(el.dataset.target) || 0;
      animateCounter(el, target, 800);
    });
  }

  // ── Card HTML builder ─────────────────────────────────────
  function civCard(title, mainStat, unit, subStats, accentColor) {
    const subs = subStats.map(s => '<div class="civ-sub">' + s + '</div>').join('');
    return '<div class="civ-card">' +
      '<div class="civ-card-accent" style="background:' + accentColor + '"></div>' +
      '<h3 class="civ-card-title">' + title + '</h3>' +
      '<div class="civ-stat">' +
        '<span class="civ-stat-num" data-target="' + mainStat + '">0</span>' +
        '<span class="civ-stat-unit">' + unit + '</span>' +
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
      el.textContent = ago < 5 ? 'Just now' : ago + 's ago';
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
