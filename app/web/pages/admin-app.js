/**
 * HumanityOS Admin Dashboard
 * Requires admin role, authenticates via Dilithium3 signature (was Ed25519
 * pre-v0.266.0; the relay's identity-keyed endpoints all verify Dilithium
 * now, so an Ed25519 sig here silently fails). Inc5c-tail (v0.277.2).
 *
 * Requires `/chat/pq.js` and `/shared/pq-relay-auth.js` to be loaded
 * before this script, they install the `window.getPqSignedAuth` and
 * `window.pqDeriveIdentity` globals we delegate to.
 */
(function() {
  'use strict';

  const authGate = document.getElementById('auth-gate');
  const dashboard = document.getElementById('dashboard');
  const authStatus = document.getElementById('auth-status');

  // ── Identity helpers ──

  // Thin wrapper around the shared Dilithium-signed-auth helper so call
  // sites read the same as the pre-cutover code. Returns null when
  // there's no plaintext identity backup in localStorage (wrapped-only
  // users have to use the chat client; standalone pages cannot re-derive
  // the seed without re-prompting for the vault passphrase, which is a
  // separate scope).
  async function getSignedAuth(purpose) {
    if (typeof window.getPqSignedAuth !== 'function') {
      console.warn('Admin auth: pq-relay-auth.js not loaded');
      return null;
    }
    return await window.getPqSignedAuth(purpose);
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

    // POST, not a query string: the Dilithium key + signature are ~10KB together,
    // which as URL params exceeded nginx's default header buffer and returned
    // HTTP 414 URI Too Long. The relay accepts the same signed auth in the body.
    try {
      const res = await fetch('/api/admin/stats', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          key: auth.key,
          timestamp: auth.timestamp,
          sig: auth.sig,
        }),
      });
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

    // System health (in-app-ops first slice)
    renderSystem(data.system || null);
  }

  // Render the System Health panel from the admin-stats `system` object.
  // Replaces the operator's SSH for disk / version / watchdog / backup.
  function renderSystem(sys) {
    const el = document.getElementById('system-health');
    if (!el) return;
    if (!sys) { el.innerHTML = '<p style="color:#666">No system data.</p>'; return; }
    const rows = [];
    rows.push(['Relay version', sys.version || '?']);
    // Watchdog state with a color cue.
    const wd = sys.watchdog_state || 'unknown';
    const wdColor = wd === 'up' ? '#2ecc71' : (wd === 'unknown' ? '#888' : '#e67e22');
    rows.push(['Watchdog', '<span style="color:' + wdColor + '">' + escapeHtml(wd) + '</span>']);
    // Disk.
    if (sys.disk) {
      const d = sys.disk;
      const pct = d.used_pct != null ? d.used_pct : '?';
      const pctColor = pct >= 90 ? '#e74c3c' : (pct >= 80 ? '#e67e22' : '#2ecc71');
      rows.push(['Disk', '<span style="color:' + pctColor + '">' + pct + '% used</span> (' +
        formatBytes(d.used_bytes) + ' / ' + formatBytes(d.total_bytes) + ', ' +
        formatBytes(d.avail_bytes) + ' free)']);
    } else {
      rows.push(['Disk', '<span style="color:#888">unavailable</span>']);
    }
    // Backup freshness.
    if (sys.backup) {
      const b = sys.backup;
      const ageMin = Math.floor((b.newest_age_secs || 0) / 60);
      const ageStr = ageMin < 60 ? ageMin + 'm ago' : Math.floor(ageMin / 60) + 'h ago';
      // Backups run every 30 min; flag if the newest is much older.
      const stale = (b.newest_age_secs || 0) > 3600;
      const ageColor = stale ? '#e67e22' : '#2ecc71';
      rows.push(['Newest backup', '<span style="color:' + ageColor + '">' + ageStr + '</span> (' +
        formatBytes(b.newest_size_bytes) + ', ' + b.count + ' kept)']);
    } else {
      rows.push(['Backups', '<span style="color:#e67e22">none found</span>']);
    }
    el.innerHTML = '<table style="width:100%;border-collapse:collapse">' +
      rows.map(r => '<tr><td style="padding:4px 12px 4px 0;color:#888;white-space:nowrap">' +
        r[0] + '</td><td style="padding:4px 0">' + r[1] + '</td></tr>').join('') +
      '</table>';
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

  // ── Federation: status AND management ──
  //
  // Adding, trusting and removing a peer used to be chat slash commands only
  // (/server-add, /server-add-key, /server-trust, /server-remove). The native
  // Server Settings page wraps them in a panel; this page could only LIST.
  // Both now go through POST /api/admin/federation, which is the same signed
  // admin auth as the stats call but over the purpose "admin_federation", so a
  // read-only stats signature cannot be replayed to change the peer registry.

  const TIER_LABELS = {
    0: '0, untrusted',
    1: '1, Accord but unverified',
    2: '2, verified (federates)',
    3: '3, verified and Accord (federates)',
  };

  function setFedStatus(msg, kind) {
    const el = document.getElementById('fed-status');
    if (!el) return;
    el.textContent = msg || '';
    el.className = 'fed-status' + (kind ? ' ' + kind : '');
  }

  function fedButtons(disabled) {
    document.querySelectorAll('.fed-card button, .fed-add-row button, .fed-card select')
      .forEach(function (b) { b.disabled = !!disabled; });
  }

  /** One federation action. Returns the fresh server list, or null on failure. */
  async function fedAction(body, workingMsg) {
    const auth = await getSignedAuth('admin_federation');
    if (!auth) {
      setFedStatus('No Humanity identity found in this browser. Sign in via Chat first.', 'err');
      return null;
    }
    fedButtons(true);
    setFedStatus(workingMsg || 'Working...');
    try {
      const res = await fetch('/api/admin/federation', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(Object.assign({
          key: auth.key, timestamp: auth.timestamp, sig: auth.sig,
        }, body)),
      });
      if (!res.ok) {
        setFedStatus(await res.text() || ('Request failed: HTTP ' + res.status), 'err');
        return null;
      }
      const data = await res.json();
      setFedStatus(data.message || 'Done.', 'ok');
      renderFederation(data.servers || []);
      return data.servers || [];
    } catch (e) {
      console.error('Federation action failed:', e);
      setFedStatus('Network error: ' + (e && e.message ? e.message : e), 'err');
      return null;
    } finally {
      fedButtons(false);
    }
  }

  function renderFederation(servers) {
    const container = document.getElementById('federation-list');
    if (!container) return;
    if (!servers.length) {
      container.innerHTML = '<p style="color:var(--text-muted)">No peers yet. Add one below.</p>';
      return;
    }
    container.innerHTML = servers.map(function (s) {
      const online = s.status === 'active' || s.status === 'connected';
      const id = escapeHtml(s.server_id);
      // "outbound-only" is the marker for a key-added peer with no address.
      const where = s.url === 'outbound-only'
        ? 'added by key, dials out to us'
        : escapeHtml(s.url);
      const options = [0, 1, 2, 3].map(function (t) {
        return '<option value="' + t + '"' + (t === s.trust_tier ? ' selected' : '') + '>'
          + TIER_LABELS[t] + '</option>';
      }).join('');
      return '<div class="fed-card">'
        + '<div class="fed-dot ' + (online ? 'online' : 'offline') + '"></div>'
        + '<div>'
        +   '<div class="fed-name">' + escapeHtml(s.name)
        +     (s.accord_compliant ? ' <span class="badge badge-verified">accord</span>' : '') + '</div>'
        +   '<div class="fed-url">' + where + ' &middot; ' + escapeHtml(s.status || 'unknown') + '</div>'
        + '</div>'
        + '<div class="fed-actions">'
        +   '<span class="fed-tier-note">Trust</span>'
        +   '<select data-fed-trust="' + id + '">' + options + '</select>'
        +   '<button type="button" class="danger" data-fed-remove="' + id + '">Remove</button>'
        + '</div>'
        + '</div>';
    }).join('');

    container.querySelectorAll('[data-fed-trust]').forEach(function (sel) {
      sel.addEventListener('change', function () {
        fedAction(
          { action: 'trust', server_id: sel.dataset.fedTrust, trust_tier: parseInt(sel.value, 10) },
          'Setting trust tier...'
        );
      });
    });
    container.querySelectorAll('[data-fed-remove]').forEach(function (btn) {
      btn.addEventListener('click', function () {
        const id = btn.dataset.fedRemove;
        // Removing a peer is not destructive to anyone's data, but it does cut
        // a live link, so it asks once.
        if (!window.confirm('Stop federating with ' + id + '?')) return;
        fedAction({ action: 'remove', server_id: id }, 'Removing...');
      });
    });
  }

  function wireFederationControls() {
    const urlBtn = document.getElementById('fed-add-url-btn');
    if (urlBtn) {
      urlBtn.addEventListener('click', async function () {
        const url = (document.getElementById('fed-add-url').value || '').trim();
        if (!/^https?:\/\/.+/.test(url)) {
          setFedStatus('Enter a full address starting with http:// or https://', 'err');
          return;
        }
        const name = (document.getElementById('fed-add-url-name').value || '').trim();
        const ok = await fedAction({ action: 'add', url: url, name: name }, 'Adding...');
        if (ok) {
          document.getElementById('fed-add-url').value = '';
          document.getElementById('fed-add-url-name').value = '';
        }
      });
    }
    const keyBtn = document.getElementById('fed-add-key-btn');
    if (keyBtn) {
      keyBtn.addEventListener('click', async function () {
        const key = (document.getElementById('fed-add-key').value || '').trim();
        if (!/^[0-9a-fA-F]{64}$/.test(key)) {
          setFedStatus('A federation key is exactly 64 hexadecimal characters.', 'err');
          return;
        }
        const name = (document.getElementById('fed-add-key-name').value || '').trim();
        const ok = await fedAction({ action: 'add_key', public_key: key, name: name }, 'Adding...');
        if (ok) {
          document.getElementById('fed-add-key').value = '';
          document.getElementById('fed-add-key-name').value = '';
        }
      });
    }
  }

  // ── Init ──

  async function init() {
    wireFederationControls();
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
