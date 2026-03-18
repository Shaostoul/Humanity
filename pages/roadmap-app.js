/**
 * Roadmap page — fetches tasks from /api/tasks, groups them by tier based on labels,
 * and renders progress bars + expandable cards.
 */
(function () {
  'use strict';

  let allTasks = [];
  let activeFilter = 'all';

  // ── Tier definitions ──
  const TIERS = [
    { key: 'tier1', title: 'Tier 1: Foundational', color: '#4a9', keywords: ['federation', 'security', 'identity', 'auth', 'crypto', 'encryption'] },
    { key: 'tier2', title: 'Tier 2: Core OS', color: '#2196f3', keywords: ['feature', 'chat', 'calendar', 'notes', 'tasks', 'profile', 'voice', 'dms'] },
    { key: 'tier3', title: 'Tier 3: Civilization', color: '#f0a500', keywords: ['governance', 'marketplace', 'education', 'economy', 'accord', 'voting'] },
    { key: 'tier4', title: 'Tier 4: Reach', color: '#e040fb', keywords: ['pwa', 'api', 'mobile', 'desktop', 'notification', 'push', 'tauri'] },
    { key: 'game', title: 'Game Engine', color: '#e53935', keywords: ['game', 'audio', 'vr', 'ar', 'engine', '3d', 'studio'] },
    { key: 'infra', title: 'Infrastructure', color: '#888', keywords: ['refactor', 'devops', 'docs', 'ci', 'deploy', 'test', 'cleanup', 'bug'] },
  ];

  // Collapsed state per tier (all start expanded)
  const collapsed = {};

  // ── Classify a task into a tier ──
  function classifyTask(task) {
    let labels = [];
    try { labels = JSON.parse(task.labels || '[]'); } catch (e) { /* skip */ }
    // Strip scope: labels for matching
    const matchLabels = labels.filter(l => !l.startsWith('scope:')).map(l => l.toLowerCase());
    const titleLower = (task.title || '').toLowerCase();
    const descLower = (task.description || '').toLowerCase();

    for (const tier of TIERS) {
      for (const kw of tier.keywords) {
        if (matchLabels.some(l => l.includes(kw)) || titleLower.includes(kw) || descLower.includes(kw)) {
          return tier.key;
        }
      }
    }
    // Fallback: use priority to infer tier
    if (task.priority === 'critical' || task.priority === 'high') return 'tier1';
    if (task.priority === 'medium') return 'tier2';
    return 'infra';
  }

  // ── Fetch tasks ──
  async function loadTasks() {
    try {
      const res = await fetch('/api/tasks');
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const data = await res.json();
      allTasks = data.tasks || [];
    } catch (e) {
      console.warn('[roadmap] Failed to load tasks:', e);
      allTasks = [];
    }
    render();
  }

  // ── Render everything ──
  function render() {
    const container = document.getElementById('tiers-container');
    if (!container) return;

    // Group tasks by tier
    const groups = {};
    for (const tier of TIERS) groups[tier.key] = [];
    for (const task of allTasks) {
      const tierKey = classifyTask(task);
      if (groups[tierKey]) groups[tierKey].push(task);
      else groups.infra.push(task);
    }

    // Apply filter
    const filterFn = activeFilter === 'all'
      ? () => true
      : (t) => t.status === activeFilter;

    // Overall stats (unfiltered)
    const totalAll = allTasks.length;
    const doneAll = allTasks.filter(t => t.status === 'done').length;
    document.getElementById('overview-done').textContent = doneAll;
    document.getElementById('overview-total').textContent = totalAll;
    document.getElementById('overview-fill').style.width = totalAll ? ((doneAll / totalAll) * 100) + '%' : '0';

    // Build tier sections
    let html = '';
    for (const tier of TIERS) {
      const tasks = groups[tier.key];
      const filtered = tasks.filter(filterFn);
      const done = tasks.filter(t => t.status === 'done').length;
      const total = tasks.length;
      const pct = total ? Math.round((done / total) * 100) : 0;
      const isCollapsed = collapsed[tier.key];

      html += '<section class="tier-section" data-tier="' + tier.key + '">';
      html += '<div class="tier-header" onclick="window.__roadmapToggleTier(\'' + tier.key + '\')">';
      html += '<span class="tier-chevron' + (isCollapsed ? ' collapsed' : '') + '">' + chevronSvg() + '</span>';
      html += '<h2>' + tier.title + '</h2>';
      html += '<span class="tier-badge">' + pct + '% complete</span>';
      html += '</div>';

      html += '<div class="tier-progress">';
      html += '<div class="progress-wrap"><div class="progress-fill" style="width:' + pct + '%;background:' + tier.color + '"></div></div>';
      html += '<span class="tier-count">' + done + ' / ' + total + '</span>';
      html += '</div>';

      if (!isCollapsed) {
        if (filtered.length === 0) {
          html += '<div class="tier-empty">No tasks match the current filter.</div>';
        } else {
          html += '<div class="tier-cards">';
          for (const task of filtered) {
            html += renderCard(task);
          }
          html += '</div>';
        }
      }

      html += '</section>';
    }

    container.innerHTML = html;
  }

  // ── Render a single task card ──
  function renderCard(task) {
    let labels = [];
    try { labels = JSON.parse(task.labels || '[]').filter(l => !l.startsWith('scope:')); } catch (e) { /* skip */ }
    const statusClass = task.status === 'in_progress' ? 'in-progress' : (task.status || 'backlog');
    const statusLabel = (task.status || 'backlog').replace('_', ' ');

    let html = '<div class="task-card" data-task-id="' + task.id + '" onclick="window.__roadmapToggleCard(this)">';
    html += '<div class="task-card-top">';
    html += '<span class="task-card-title">' + esc(task.title || 'Untitled') + '</span>';
    html += '<span class="status-badge ' + statusClass + '">' + statusLabel + '</span>';
    html += '</div>';

    if (task.priority) {
      html += '<span class="task-card-priority ' + task.priority + '">' + task.priority + '</span>';
    }

    if (labels.length) {
      html += '<div class="task-card-labels">';
      for (const l of labels) {
        html += '<span class="label-tag">' + esc(l) + '</span>';
      }
      html += '</div>';
    }

    html += '<div class="task-detail">';
    html += '<div class="task-detail-desc">' + esc(task.description || 'No description.') + '</div>';
    html += '<div class="task-detail-comments" id="comments-' + task.id + '"></div>';
    html += '</div>';

    html += '</div>';
    return html;
  }

  // ── Load comments for a task ──
  async function loadComments(taskId) {
    const el = document.getElementById('comments-' + taskId);
    if (!el || el.dataset.loaded) return;
    el.dataset.loaded = '1';
    try {
      const res = await fetch('/api/tasks/' + taskId + '/comments');
      if (!res.ok) return;
      const data = await res.json();
      const comments = data.comments || [];
      if (comments.length === 0) {
        el.innerHTML = '<div style="font-size:var(--text-xs);color:var(--text-muted)">No comments yet.</div>';
        return;
      }
      let h = '<h4>Comments (' + comments.length + ')</h4>';
      for (const c of comments) {
        h += '<div class="comment-item">';
        h += '<span class="comment-author">' + esc(c.author_name || 'Anonymous') + '</span>';
        h += '<div class="comment-text">' + esc(c.content || '') + '</div>';
        h += '</div>';
      }
      el.innerHTML = h;
    } catch (e) {
      console.warn('[roadmap] Failed to load comments for task', taskId, e);
    }
  }

  // ── Helpers ──
  function esc(s) {
    var d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  function chevronSvg() {
    return '<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4,2 12,8 4,14"/></svg>';
  }

  // ── Global handlers (called from onclick in rendered HTML) ──
  window.__roadmapToggleTier = function (key) {
    collapsed[key] = !collapsed[key];
    render();
  };

  window.__roadmapToggleCard = function (el) {
    el.classList.toggle('expanded');
    if (el.classList.contains('expanded')) {
      var tid = el.dataset.taskId;
      if (tid) loadComments(tid);
    }
  };

  // ── Filter buttons ──
  document.getElementById('filter-bar').addEventListener('click', function (e) {
    var btn = e.target.closest('.filter-btn');
    if (!btn) return;
    document.querySelectorAll('.filter-btn').forEach(function (b) { b.classList.remove('active'); });
    btn.classList.add('active');
    activeFilter = btn.dataset.filter;
    render();
  });

  // ── Init ──
  loadTasks();
})();
