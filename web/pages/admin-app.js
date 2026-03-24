/**
 * HumanityOS Admin Dashboard
 * Requires admin role — authenticates via Ed25519 signature.
 */
(function() {
  'use strict';

  const authGate = document.getElementById('auth-gate');
  const dashboard = document.getElementById('dashboard');
  const authStatus = document.getElementById('auth-status');

  // ── Identity helpers ──

  async function getSignedAuth(purpose) {
    const backup = localStorage.getItem('humanity_key_backup');
    const keyHex = localStorage.getItem('humanity_key');
    if (!backup || !keyHex) return null;
    try {
      const parsed = JSON.parse(backup);
      let privateKey;
      if (parsed.jwk) {
        privateKey = await crypto.subtle.importKey('jwk', parsed.jwk, 'Ed25519', false, ['sign']);
      } else if (parsed.privateKeyPkcs8) {
        const pkcs8Buf = Uint8Array.from(atob(parsed.privateKeyPkcs8), c => c.charCodeAt(0));
        privateKey = await crypto.subtle.importKey('pkcs8', pkcs8Buf, 'Ed25519', false, ['sign']);
      } else {
        return null;
      }
      const ts = Date.now();
      const payload = `${purpose}\n${ts}`;
      const sigBuf = await crypto.subtle.sign('Ed25519', privateKey, new TextEncoder().encode(payload));
      const sig = Array.from(new Uint8Array(sigBuf)).map(b => b.toString(16).padStart(2, '0')).join('');
      return { key: keyHex, timestamp: ts, sig };
    } catch (e) {
      console.warn('Admin auth sign failed:', e);
      return null;
    }
  }

  // ── Formatting helpers ──

  function formatBytes(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }

  function formatUptime(seconds) {
    const d = Math.floor(seconds / 86400);
    const h = Math.floor((seconds % 86400) / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    if (d > 0) return d + 'd ' + h + 'h';
    if (h > 0) return h + 'h ' + m + 'm';
    return m + 'm';
  }

  function formatNumber(n) {
    if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (n >= 1000) return (n / 1000).toFixed(1) + 'K';
    return String(n);
  }

  function roleBadge(role) {
    const cls = role === 'admin' ? 'badge-admin'
      : role === 'mod' ? 'badge-mod'
      : role === 'verified' ? 'badge-verified'
      : 'badge-member';
    return `<span class="badge ${cls}">${role || 'member'}</span>`;
  }

  function escapeHtml(str) {
    const el = document.createElement('span');
    el.textContent = str || '';
    return el.innerHTML;
  }

  // ── Fetch admin stats ──

  async function fetchStats() {
    const auth = await getSignedAuth('admin_stats');
    if (!auth) {
      showAuthGate('No Humanity identity found. Sign in via Chat first.');
      return null;
    }

    const url = `/api/admin/stats?key=${encodeURIComponent(auth.key)}&timestamp=${auth.timestamp}&sig=${encodeURIComponent(auth.sig)}`;
    try {
      const res = await fetch(url);
      if (res.status === 403) {
        showAuthGate('Your account does not have admin privileges.');
        return null;
      }
      if (res.status === 401) {
        showAuthGate('Authentication failed. Please sign in again via Chat.');
        return null;
      }
      if (!res.ok) {
        const text = await res.text();
        console.error('Admin stats error:', text);
        showAuthGate('Failed to load admin stats: ' + text);
        return null;
      }
      return await res.json();
    } catch (e) {
      console.error('Admin stats fetch error:', e);
      showAuthGate('Network error loading admin stats.');
      return null;
    }
  }

  function showAuthGate(msg) {
    authGate.style.display = '';
    dashboard.style.display = 'none';
    authStatus.textContent = msg;
  }

  // ── Render dashboard ──

  function renderDashboard(data) {
    authGate.style.display = 'none';
    dashboard.style.display = '';

    // Overview cards
    document.getElementById('stat-users').textContent = formatNumber(data.user_count);
    document.getElementById('stat-online').textContent = data.online_count;
    document.getElementById('stat-messages-24h').textContent = formatNumber(data.message_count_24h);
    document.getElementById('stat-messages-total').textContent = formatNumber(data.total_messages) + ' total';
    document.getElementById('stat-storage').textContent = formatBytes(data.db_size_bytes + (data.upload_size_bytes || 0));
    document.getElementById('stat-storage-detail').textContent =
      'DB: ' + formatBytes(data.db_size_bytes) + ' / Uploads: ' + formatBytes(data.upload_size_bytes || 0);
    document.getElementById('stat-uptime').textContent = formatUptime(data.uptime_seconds);

    // Game world
    document.getElementById('stat-game').textContent = data.game_players + ' players';
    document.getElementById('stat-game-detail').textContent =
      data.game_entities + ' entities, t=' + (data.game_time || 0).toFixed(0) + 's';

    // Activity chart (24 bars, one per hour)
    renderActivityChart(data.hourly_messages || []);

    // Top channels
    renderTopChannels(data.top_channels || []);

    // Recent joins
    renderRecentJoins(data.recent_joins || []);

    // Federation
    renderFederation(data.federation || []);
  }

  function renderActivityChart(hourlyData) {
    const chart = document.getElementById('activity-chart');
    chart.innerHTML = '';

    // Build 24-hour buckets
    const buckets = new Array(24).fill(0);
    for (const entry of hourlyData) {
      const h = Math.floor(entry.hour);
      if (h >= 0 && h < 24) {
        buckets[h] = entry.count;
      }
    }

    const max = Math.max(1, ...buckets);
    for (let i = 0; i < 24; i++) {
      const bar = document.createElement('div');
      bar.className = 'chart-bar';
      const pct = (buckets[i] / max) * 100;
      bar.style.height = Math.max(2, pct) + '%';
      bar.setAttribute('data-count', buckets[i] + ' msgs');
      chart.appendChild(bar);
    }
  }

  function renderTopChannels(channels) {
    const tbody = document.getElementById('top-channels');
    if (!channels.length) {
      tbody.innerHTML = '<tr><td colspan="2" style="color:#666">No data</td></tr>';
      return;
    }
    tbody.innerHTML = channels.map(ch =>
      `<tr><td>#${escapeHtml(ch.channel)}</td><td>${formatNumber(ch.count)}</td></tr>`
    ).join('');
  }

  function renderRecentJoins(joins) {
    const tbody = document.getElementById('recent-joins');
    if (!joins.length) {
      tbody.innerHTML = '<tr><td colspan="3" style="color:#666">No data</td></tr>';
      return;
    }
    tbody.innerHTML = joins.map(j =>
      `<tr><td>${escapeHtml(j.name || 'Anonymous')}</td><td>${roleBadge(j.role)}</td><td style="color:#888;font-size:0.75rem">${escapeHtml(j.joined_at)}</td></tr>`
    ).join('');
  }

  function renderFederation(servers) {
    const container = document.getElementById('federation-list');
    if (!servers.length) {
      container.innerHTML = '<p style="color:#666">No federated servers configured.</p>';
      return;
    }
    container.innerHTML = servers.map(s => {
      const online = s.status === 'active' || s.status === 'connected';
      return `<div class="fed-card">
        <div class="fed-dot ${online ? 'online' : 'offline'}"></div>
        <div>
          <div class="fed-name">${escapeHtml(s.name)}</div>
          <div class="fed-url">${escapeHtml(s.url)} &middot; Trust tier ${s.trust_tier}</div>
        </div>
      </div>`;
    }).join('');
  }

  // ── Init ──

  async function init() {
    const data = await fetchStats();
    if (data) {
      renderDashboard(data);
    }

    // Auto-refresh every 30 seconds
    setInterval(async () => {
      const fresh = await fetchStats();
      if (fresh) renderDashboard(fresh);
    }, 30000);
  }

  // Wait for DOM
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
