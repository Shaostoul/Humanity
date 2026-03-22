/* ── Fibonacci scope definitions ── */
const SCOPES = [
  { n:1,  key:'self',    label:'Self',    color:'#ff6b6b', desc:'Personal health, body, daily needs' },
  { n:1,  key:'mind',    label:'Mind',    color:'#ff9f43', desc:'Learning, mental health, personal skills' },
  { n:2,  key:'hearth',  label:'Hearth',  color:'#ffd32a', desc:'Home, close relationships, household' },
  { n:3,  key:'circle',  label:'Circle',  color:'#0be881', desc:'Friend group / small team (3–8 people)' },
  { n:5,  key:'village', label:'Village', color:'#05c46b', desc:'Neighborhood / working group (8–21 people)' },
  { n:8,  key:'city',    label:'City',    color:'#0fbcf9', desc:'Project level — HumanityOS (default)' },
  { n:13, key:'region',  label:'Region',  color:'#7f8fa6', desc:'Large organization / national / bioregional' },
  { n:21, key:'world',   label:'World',   color:'#a29bfe', desc:'Civilization-wide, planetary goals' },
  { n:34, key:'solar',   label:'Solar',   color:'#74b9ff', desc:'Solar System — space stations, Mars colony' },
  { n:55, key:'cosmos',  label:'Cosmos',  color:'#dfe6e9', desc:'Interstellar civilization' },
];

let allTasks = [];
let activeScope = 'cosmos';
let openDetailId = null; // track currently open detail drawer for real-time updates

/* ── Project state ── */
let activeProject = null;  // null = all projects, 'default' = uncategorized
let projects = [];
let editingProjectId = null; // non-null when editing an existing project

const PROJECT_COLORS = [
  '#4488ff', '#e53935', '#f0a500', '#4ec87a', '#a29bfe',
  '#0fbcf9', '#ff6b6b', '#ff9f43', '#ffd32a', '#05c46b',
  '#7f8fa6', '#e85d04', '#c8f', '#fa4', '#4cf',
];
const PROJECT_ICONS = ['📋', '🚀', '🏠', '🔧', '🎯', '🌍', '💡', '🎨', '📦', '🔬', '🏗️', '⚡', '🛡️', '📊', '🎮', '🌱'];

/* ── Init ── */
function init() {
  buildScopeTabs();
  buildScopeSelect();
  buildProjectColorPicker();
  buildProjectIconPicker();
  loadTasks();
  loadProjects();
  // Connect to relay for real-time updates if user has an identity
  setTimeout(ensureTaskWs, 1000);
}

function buildScopeTabs() {
  const container = document.getElementById('scope-tabs');
  container.innerHTML = SCOPES.map(s => `
    <button class="scope-tab${s.key === activeScope ? ' active' : ''}"
            data-key="${s.key}" onclick="setScope('${s.key}')"
            title="${s.desc}">
      <span class="scope-fib" style="color:${s.color}">${s.n}</span>
      <span class="scope-label">${s.label}</span>
      <span class="scope-tab-dot" style="background:${s.color}"></span>
    </button>`).join('');
}

function buildScopeSelect() {
  const sel = document.getElementById('f-scope');
  sel.innerHTML = SCOPES.map(s =>
    `<option value="${s.key}"${s.key === 'city' ? ' selected' : ''}>${s.n}·${s.label} — ${s.desc}</option>`
  ).join('');
}

function setScope(key) {
  activeScope = key;
  document.querySelectorAll('.scope-tab').forEach(t => t.classList.toggle('active', t.dataset.key === key));
  renderBoard();
  renderControls();
}

/* ── Data loading ── */
async function loadTasks() {
  try {
    const res = await fetch('/api/tasks');
    console.log('[tasks] HTTP', res.status);
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const data = await res.json();
    allTasks = data.tasks || [];
    renderBoard();
    renderControls();
    renderProjectSelectorBtn();
  } catch (e) {
    console.error('[tasks] load error:', e);
    document.getElementById('col-backlog').innerHTML = `<div class="empty-col">⚠️ Could not load tasks<br><small>${e.message}</small></div>`;
  }
}

/* ── Scope helpers ── */
function getTaskScope(task) {
  try {
    const labels = JSON.parse(task.labels || '[]');
    const sl = labels.find(l => l.startsWith('scope:'));
    return sl ? sl.replace('scope:', '') : 'city';
  } catch { return 'city'; }
}

function getNonScopeLabels(task) {
  try { return JSON.parse(task.labels || '[]').filter(l => !l.startsWith('scope:')); }
  catch { return []; }
}

function scopedTasks() {
  const q = (document.getElementById('task-search')?.value || '').trim().toLowerCase();
  return allTasks.filter(t => {
    // Project filter
    if (activeProject !== null) {
      const taskProject = t.project || 'default';
      if (taskProject !== activeProject) return false;
    }
    if (getTaskScope(t) !== activeScope) return false;
    if (!q) return true;
    return t.title.toLowerCase().includes(q) ||
           (t.description || '').toLowerCase().includes(q) ||
           (t.assignee || '').toLowerCase().includes(q);
  });
}

/* ── Render ── */
function renderBoard() {
  const tasks = scopedTasks();
  ['backlog','in_progress','testing','done'].forEach(status => {
    const col = document.getElementById('col-' + status);
    const count = document.getElementById('count-' + status);
    const filtered = tasks.filter(t => t.status === status);
    count.textContent = filtered.length;
    if (!filtered.length) { col.innerHTML = `<div class="empty-col">—</div>`; return; }
    try {
      col.innerHTML = filtered.map(renderCard).join('');
    } catch (e) {
      col.innerHTML = `<div class="empty-col">⚠️ Render error<br><small>${esc(e.message)}</small></div>`;
      console.error('renderCard error in', status, e);
    }
  });
}

function renderControls() {
  const scope = SCOPES.find(s => s.key === activeScope) || SCOPES[5];
  document.getElementById('scope-desc').innerHTML =
    `<strong style="color:${scope.color}">${scope.n}·${scope.label}</strong> &mdash; ${scope.desc}`;
  const tasks = scopedTasks();
  const ip = allTasks.filter(t => t.status === 'in_progress').length;
  const projLabel = activeProject === null ? 'all projects'
    : activeProject === 'default' ? 'General'
    : (projects.find(p => p.id === activeProject)?.name || activeProject);
  document.getElementById('stats-bar').innerHTML =
    `<span class="stat-pill"><span>${allTasks.length}</span> total</span>` +
    `<span class="stat-pill"><span>${tasks.length}</span> visible</span>` +
    `<span class="stat-pill"><span>${ip}</span> active</span>`;
}

function renderCard(task) {
  const scope = SCOPES.find(s => s.key === getTaskScope(task)) || SCOPES[5];
  const labels = getNonScopeLabels(task);
  // Test tally for testing-column cards
  let testTally = '';
  if (task.status === 'testing' || task.status === 'done') {
    const tv = loadVotes()[task.id] || {};
    const passes = (tv.community||[]).filter(v=>v.result==='pass').length + (tv.owner==='pass'?1:0);
    const fails  = (tv.community||[]).filter(v=>v.result==='fail').length + (tv.owner==='fail'?1:0);
    if (passes || fails) {
      testTally = `<span class="test-tally">${passes?'✅'+passes:''}${fails?'❌'+fails:''}</span>`;
    } else if (task.status === 'testing') {
      testTally = `<span class="test-tally" style="color:#555">🧪</span>`;
    }
  }
  return `<div class="card priority-${task.priority}" onclick="openDetail(${task.id})">
    <div class="card-title">${esc(task.title)}</div>
    <div class="card-info">
      <span class="card-prio" style="color:${priorityColor(task.priority)}">${task.priority.toUpperCase()}</span>
      <span class="card-sep">·</span>
      <span>${timeAgo(task.created_at)}</span>
      <span class="card-sep">·</span>
      <span class="scope-badge" style="color:${scope.color}">${scope.n}·${scope.label}</span>
      ${labels.slice(0,3).map(l=>`<span class="label-tag">${esc(l)}</span>`).join('')}
      ${testTally}
    </div>
  </div>`;
}

/* ── Modal ── */
function openModal() {
  document.getElementById('modal-overlay').classList.add('open');
  document.getElementById('f-title').focus();
  document.getElementById('form-msg').innerHTML = '';
  document.getElementById('f-scope').value = activeScope;
  buildProjectSelect();
}

function closeModal() {
  document.getElementById('modal-overlay').classList.remove('open');
}

// WebSocket connection for task creation by authenticated relay users.
let taskWs = null;
let taskWsReady = false;
let taskWsPending = null; // resolve fn waiting for task_created confirmation

function ensureTaskWs() {
  if (taskWs && taskWs.readyState === WebSocket.OPEN) return;
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const raw = localStorage.getItem('humanity_key_backup');
  if (!raw) return;
  let pub;
  try { pub = JSON.parse(raw).publicKeyHex; } catch { return; }
  if (!pub) return;
  const name = localStorage.getItem('humanity_name') || 'Task_User';
  taskWs = new WebSocket(`${proto}//${location.host}/ws`);
  taskWs.addEventListener('open', () => {
    taskWs.send(JSON.stringify({ type: 'identify', public_key: pub, display_name: name }));
    // Request project list after identifying
    setTimeout(requestProjectList, 300);
  });
  taskWs.addEventListener('message', e => {
    try {
      const m = JSON.parse(e.data);
      // Resolve pending create promise
      if (m.type === 'task_created' && taskWsPending) { taskWsPending(m.task); taskWsPending = null; }
      if (m.type === 'system' && m.message && taskWsPending) { taskWsPending(null, m.message); taskWsPending = null; }
      // Real-time board updates — add/update tasks without full reload
      if (m.type === 'task_created') {
        if (!allTasks.find(t => t.id === m.task.id)) { allTasks.push(m.task); renderBoard(); }
      }
      if (m.type === 'task_updated') {
        const idx = allTasks.findIndex(t => t.id === m.task.id);
        if (idx >= 0) { allTasks[idx] = { ...allTasks[idx], ...m.task }; } else { allTasks.push(m.task); }
        renderBoard();
        if (document.getElementById('detail-overlay').classList.contains('open') && openDetailId === m.task.id) openDetail(m.task.id);
      }
      if (m.type === 'task_moved') {
        const t = allTasks.find(t => t.id === m.id);
        if (t) { t.status = m.status; renderBoard(); }
      }
      if (m.type === 'task_deleted') {
        allTasks = allTasks.filter(t => t.id !== m.id);
        renderBoard();
        if (openDetailId === m.id) closeDetail();
      }
      // Project real-time updates
      if (m.type === 'project_list') {
        projects = m.projects || [];
        renderProjectDropdown();
        renderProjectSelectorBtn();
      }
      if (m.type === 'project_created') {
        if (m.project && !projects.find(p => p.id === m.project.id)) {
          projects.push(m.project);
          renderProjectDropdown();
        }
      }
      if (m.type === 'project_updated') {
        if (m.project) {
          const idx = projects.findIndex(p => p.id === m.project.id);
          if (idx >= 0) projects[idx] = { ...projects[idx], ...m.project };
          else projects.push(m.project);
          renderProjectDropdown();
          renderProjectSelectorBtn();
        }
      }
      if (m.type === 'project_deleted') {
        projects = projects.filter(p => p.id !== m.id);
        if (activeProject === m.id) { activeProject = null; renderBoard(); renderControls(); }
        renderProjectDropdown();
        renderProjectSelectorBtn();
      }
      // Decode task_comment_added system messages
      if (m.type === 'system' && m.message && m.message.startsWith('__task_comment__:')) {
        try {
          const c = JSON.parse(m.message.slice('__task_comment__:'.length));
          const t = allTasks.find(t => t.id === c.task_id);
          if (t) t.comment_count = (t.comment_count || 0) + 1;
          if (openDetailId === c.task_id) loadTaskComments(c.task_id);
        } catch {}
      }
    } catch {}
  });
  // Reconnect on close
  taskWs.addEventListener('close', () => {
    taskWs = null;
    setTimeout(ensureTaskWs, 5000);
  });
}

async function submitTask() {
  const title = document.getElementById('f-title').value.trim();
  const apiKey = document.getElementById('f-apikey').value.trim();
  const msg = document.getElementById('form-msg');

  if (!title) { msg.innerHTML = '<div class="msg-error">Title is required.</div>'; return; }

  const rawLabels = document.getElementById('f-labels').value.trim();
  const labelArr = rawLabels ? rawLabels.split(',').map(l => l.trim()).filter(Boolean) : [];
  labelArr.push('scope:' + document.getElementById('f-scope').value);

  const body = {
    title,
    description: document.getElementById('f-desc').value.trim(),
    priority: document.getElementById('f-priority').value,
    status: document.getElementById('f-status').value,
    labels: JSON.stringify(labelArr),
    project: document.getElementById('f-project').value || 'default',
  };
  const assignee = document.getElementById('f-assignee').value.trim();
  if (assignee) body.assignee = assignee;

  const btn = document.getElementById('btn-submit');
  btn.disabled = true;
  btn.textContent = 'Creating…';

  // Use API key if provided; otherwise use relay WebSocket (signed-in users).
  if (apiKey) {
    try {
      const res = await fetch('/api/tasks', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': 'Bearer ' + apiKey },
        body: JSON.stringify(body),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.message || 'HTTP ' + res.status);
      msg.innerHTML = `<div class="msg-success">✓ Task #${data.id} created.</div>`;
      setTimeout(() => { closeModal(); loadTasks(); }, 800);
    } catch(e) {
      msg.innerHTML = `<div class="msg-error">Error: ${esc(e.message)}</div>`;
    } finally {
      btn.disabled = false;
      btn.textContent = 'Create Task';
    }
  } else {
    // Relay WebSocket path — requires humanity_key_backup in localStorage.
    ensureTaskWs();
    if (!taskWs || taskWs.readyState !== WebSocket.OPEN) {
      // Give it 1.5s to connect
      await new Promise(r => setTimeout(r, 1500));
    }
    if (!taskWs || taskWs.readyState !== WebSocket.OPEN) {
      msg.innerHTML = '<div class="msg-error">Not signed in. Enter Admin API Key or sign in at /chat first.</div>';
      btn.disabled = false;
      btn.textContent = 'Create Task';
      return;
    }
    try {
      const task = await new Promise((resolve, reject) => {
        taskWsPending = (task, err) => err ? reject(new Error(err)) : resolve(task);
        taskWs.send(JSON.stringify({ type: 'task_create', ...body }));
        setTimeout(() => { if (taskWsPending) { taskWsPending = null; reject(new Error('Timeout')); } }, 5000);
      });
      msg.innerHTML = `<div class="msg-success">✓ Task #${task?.id || '?'} created.</div>`;
      setTimeout(() => { closeModal(); loadTasks(); }, 800);
    } catch(e) {
      msg.innerHTML = `<div class="msg-error">Error: ${esc(e.message)}</div>`;
    } finally {
      btn.disabled = false;
      btn.textContent = 'Create Task';
    }
  }
}

function openDetail(id) {
  openDetailId = id;
  const task = allTasks.find(t => t.id === id);
  if (!task) return;

  const scope = SCOPES.find(s => s.key === getTaskScope(task)) || SCOPES[5];
  const labels = getNonScopeLabels(task);

  document.getElementById('detail-id').textContent = '#' + task.id;
  document.getElementById('detail-title').textContent = task.title;

  const scopeBadge = document.getElementById('detail-scope-badge');
  scopeBadge.textContent = scope.n + '·' + scope.label;
  scopeBadge.style.color = scope.color;

  const statusLabel = task.status.replace(/_/g, ' ');
  document.getElementById('detail-badges').innerHTML =
    `<span class="detail-badge badge-status-${task.status}">${statusLabel}</span>` +
    `<span class="detail-badge badge-priority-${task.priority}">${task.priority}</span>`;

  // Status action buttons — clicking moves the task to that status.
  const statusSteps = [
    { key: 'backlog',     label: (typeof hosIcon==='function'?hosIcon('tasklist',14):'')+' Backlog' },
    { key: 'in_progress', label: (typeof hosIcon==='function'?hosIcon('settings',14):'')+' In Progress' },
    { key: 'testing',     label: (typeof hosIcon==='function'?hosIcon('search',14):'')+' Testing' },
    { key: 'done',        label: (typeof hosIcon==='function'?hosIcon('check',14):'')+' Done' },
  ];
  document.getElementById('detail-status-btns').innerHTML =
    statusSteps.map(s =>
      s.key === task.status
        ? `<button class="btn-status current current-${s.key}" disabled title="Current status">${s.label}</button>`
        : `<button class="btn-status" onclick="changeTaskStatus(${task.id},'${s.key}')" title="Move to ${s.label}">${s.label}</button>`
    ).join('') +
    `<span class="status-move-label">← move to</span>`;

  // Priority change buttons.
  const priorities = [
    { key: 'low',      label: '▽ Low' },
    { key: 'medium',   label: '◆ Medium' },
    { key: 'high',     label: '▲ High' },
    { key: 'critical', label: '🔥 Critical' },
  ];
  document.getElementById('detail-priority-btns').innerHTML =
    priorities.map(p =>
      p.key === task.priority
        ? `<button class="btn-status current current-${p.key}" disabled title="Current priority">${p.label}</button>`
        : `<button class="btn-status" onclick="changeTaskPriority(${task.id},'${p.key}')" title="Set priority to ${p.label}">${p.label}</button>`
    ).join('') +
    `<span class="status-move-label">← priority</span>`;

  // Assignee inline editor.
  document.getElementById('detail-assignee-row').innerHTML =
    `<div class="assignee-row">
       <span style="font-size:0.68rem;color:var(--text-dim);min-width:65px;flex-shrink:0">Assignee</span>
       <input id="detail-assignee-input" type="text" value="${esc(task.assignee || '')}" placeholder="unassigned" maxlength="80">
       <button onclick="changeTaskAssignee(${task.id})" title="Save assignee">Save</button>
     </div>`;

  document.getElementById('detail-desc').textContent = task.description || '(No description)';

  document.getElementById('detail-labels').innerHTML = labels.length
    ? labels.map(l => `<span class="label-tag">${esc(l)}</span>`).join('')
    : '';

  const created = task.created_at ? new Date(task.created_at).toLocaleString() : '—';
  const age = task.created_at ? timeAgo(task.created_at) + ' ago' : '';
  document.getElementById('detail-meta').innerHTML =
    `<div class="detail-meta-row"><span class="detail-meta-label">Created</span><span>${created}${age ? ' · ' + age : ''}</span></div>` +
    (task.assignee ? `<div class="detail-meta-row"><span class="detail-meta-label">Assignee</span><span>${esc(task.assignee)}</span></div>` : '') +
    (task.comment_count > 0 ? `<div class="detail-meta-row"><span class="detail-meta-label">Comments</span><span>${task.comment_count}</span></div>` : '');

  // Test voting panel (shown for testing and done tasks)
  const testPanelEl = document.getElementById('detail-test-panel');
  if (task.status === 'testing' || task.status === 'done') {
    testPanelEl.innerHTML = renderTestPanel(task);
  } else {
    testPanelEl.innerHTML = '';
  }

  document.getElementById('detail-overlay').classList.add('open');

  // Load comments asynchronously
  loadTaskComments(id);
}

async function loadTaskComments(taskId) {
  const el = document.getElementById('detail-comments');
  if (!el) return;
  try {
    const res = await fetch(`/api/tasks/${taskId}/comments`);
    if (!res.ok) { el.innerHTML = ''; return; }
    const data = await res.json();
    const comments = data.comments || [];
    el.innerHTML = comments.length === 0 ? '<div style="color:var(--text-muted);font-size:0.78rem;padding:var(--space-sm) 0">No comments yet.</div>' :
      comments.map(c => `<div style="border-left:2px solid var(--border);padding:var(--space-sm) var(--space-md);margin-bottom:var(--space-md);">
        <div style="font-size:0.72rem;color:var(--accent);font-weight:600">${esc(c.author_name)}</div>
        <div style="font-size:0.8rem;white-space:pre-wrap">${esc(c.content)}</div>
        <div style="font-size:0.68rem;color:var(--text-muted)">${new Date(c.created_at * 1000).toLocaleString()}</div>
      </div>`).join('');
  } catch { el.innerHTML = ''; }
}

async function submitComment(taskId) {
  const input = document.getElementById('comment-input');
  const content = (input && input.value || '').trim();
  if (!content) return;
  input.disabled = true;
  if (taskWs && taskWs.readyState === WebSocket.OPEN) {
    taskWs.send(JSON.stringify({ type: 'task_comment', task_id: taskId, content }));
    input.value = '';
    // Update comment count optimistically
    const t = allTasks.find(t => t.id === taskId);
    if (t) t.comment_count = (t.comment_count || 0) + 1;
    await new Promise(r => setTimeout(r, 500));
    await loadTaskComments(taskId);
  } else {
    alert('Sign in at /chat to post comments.');
  }
  input.disabled = false;
  input.focus();
}

function closeDetail() {
  document.getElementById('detail-overlay').classList.remove('open');
}

/* ── Test vote storage ── */
const VOTES_KEY = 'hos_task_votes';
function loadVotes() { try { return JSON.parse(localStorage.getItem(VOTES_KEY)) || {}; } catch { return {}; } }
function saveVotes(v) { localStorage.setItem(VOTES_KEY, JSON.stringify(v)); }

/* ── Lightweight identity reader (uses chat page's localStorage backup) ── */
let _voteId = null, _voteIdTried = false;
async function getVoteIdentity() {
  if (_voteIdTried) return _voteId;
  _voteIdTried = true;
  try {
    const raw = localStorage.getItem('humanity_key_backup');
    if (!raw) return null;
    const { publicKeyHex, privateKeyPkcs8 } = JSON.parse(raw);
    if (!publicKeyHex || !privateKeyPkcs8) return null;
    const buf = Uint8Array.from(atob(privateKeyPkcs8), c => c.charCodeAt(0));
    const pk  = await crypto.subtle.importKey('pkcs8', buf, 'Ed25519', false, ['sign']);
    _voteId = { publicKeyHex, privateKey: pk };
  } catch(e) { /* unsigned votes still work */ }
  return _voteId;
}
async function signVote(taskId, result) {
  const id = await getVoteIdentity();
  if (!id) return null;
  try {
    const ts  = Date.now();
    const msg = `vote:${taskId}:${result}:${ts}`;
    const sig = await crypto.subtle.sign('Ed25519', id.privateKey, new TextEncoder().encode(msg));
    return { sig: [...new Uint8Array(sig)].map(b=>b.toString(16).padStart(2,'0')).join(''), pub: id.publicKeyHex, ts };
  } catch { return null; }
}

/**
 * Cast the owner/determining test vote on a task. Signs with Ed25519 if
 * the chat identity is available in localStorage. Toggling the same result clears it.
 */
async function castOwnerVote(taskId, result) {
  const v = loadVotes();
  if (!v[taskId]) v[taskId] = { owner: null, community: [] };
  if (v[taskId].owner === result) {
    v[taskId].owner = null;
    delete v[taskId].ownerSig;
    delete v[taskId].ownerPub;
  } else {
    v[taskId].owner = result;
    const proof = await signVote(taskId, result);
    if (proof) { v[taskId].ownerSig = proof.sig; v[taskId].ownerPub = proof.pub; v[taskId].ownerTs = proof.ts; }
  }
  saveVotes(v);
  openDetail(taskId);
  renderBoard();
}

/**
 * Cast a community test vote (non-determining, but visible to the owner).
 * Re-voting replaces the previous community vote from this browser.
 */
async function castCommunityVote(taskId, result) {
  const v = loadVotes();
  if (!v[taskId]) v[taskId] = { owner: null, community: [] };
  const already = v[taskId].community.find(x => x.device === '_local');
  if (already) {
    if (already.result === result) {
      v[taskId].community = v[taskId].community.filter(x => x.device !== '_local');
    } else {
      already.result = result;
      already.time = Date.now();
    }
  } else {
    const id = await getVoteIdentity();
    v[taskId].community.push({
      device: '_local',
      name: id ? id.publicKeyHex.slice(0,12) + '…' : 'Anonymous',
      pub: id ? id.publicKeyHex : null,
      result,
      time: Date.now(),
    });
  }
  saveVotes(v);
  openDetail(taskId);
  renderBoard();
}
castCommunityVote = async function(taskId, result) {
  const v = loadVotes();
  if (!v[taskId]) v[taskId] = { owner: null, community: [] };
  const already = v[taskId].community.find(x => x.device === '_local');
  if (already) {
    if (already.result === result) {
      v[taskId].community = v[taskId].community.filter(x => x.device !== '_local');
    } else {
      already.result = result;
      already.time = Date.now();
    }
    saveVotes(v); openDetail(taskId); renderBoard();
  } else {
    const id = await getVoteIdentity();
    v[taskId].community.push({
      device: '_local',
      name: id ? id.publicKeyHex.slice(0,12) + '…' : 'Anonymous',
      pub: id ? id.publicKeyHex : null,
      result, time: Date.now(),
    });
    saveVotes(v); openDetail(taskId); renderBoard();
  }
};

/**
 * Build the test-voting panel HTML for the task detail drawer.
 * Owner vote is the determining vote; community votes are advisory.
 */
function renderTestPanel(task) {
  const tv = loadVotes()[task.id] || { owner: null, community: [] };
  const ownerPass = tv.owner === 'pass';
  const ownerFail = tv.owner === 'fail';
  const community = tv.community || [];
  const cPass = community.filter(v=>v.result==='pass').length;
  const cFail = community.filter(v=>v.result==='fail').length;
  const totalPass = cPass + (ownerPass?1:0);
  const totalFail = cFail + (ownerFail?1:0);

  const communityRows = community.length
    ? community.map(v => `
        <div class="test-vote-item">
          <span class="test-vote-result">${v.result==='pass'?'✅':'❌'}</span>
          <span class="test-vote-name">${esc(v.name||'Anonymous')}</span>
          <span class="test-vote-time">${v.time ? timeAgo(v.time)+' ago' : ''}</span>
        </div>`).join('')
    : `<div style="font-size:0.72rem;color:#555;padding:var(--space-xs) 0">No community votes yet.</div>`;

  return `
    <div class="test-panel">
      <div class="test-panel-title">🧪 Test Results</div>
      ${totalPass || totalFail
        ? `<div class="test-summary">
             <span class="test-summary-pass">✅ ${totalPass} pass</span>
             <span class="test-summary-fail">❌ ${totalFail} fail</span>
           </div>`
        : ''}
      <div class="test-owner-label">Your vote (determining)${tv.ownerPub ? ` · <code style="font-size:.6rem;color:#555">${tv.ownerPub.slice(0,16)}…</code> ✍️` : ' · unsigned'}:</div>
      <div class="test-vote-row">
        <button class="btn-test-vote${ownerPass?' vote-pass':''}" onclick="castOwnerVote(${task.id},'pass')">
          ✅ Pass${ownerPass?' ← your vote':''}
        </button>
        <button class="btn-test-vote${ownerFail?' vote-fail':''}" onclick="castOwnerVote(${task.id},'fail')">
          ❌ Fail${ownerFail?' ← your vote':''}
        </button>
      </div>
      <div class="test-community">
        <div class="test-community-header">
          <span style="font-size:0.68rem;color:#666;font-weight:700;text-transform:uppercase;letter-spacing:.05em">Community votes</span>
          <span>${cPass} pass · ${cFail} fail</span>
        </div>
        ${communityRows}
        <div style="margin-top:var(--space-md);display:flex;gap:var(--space-md)">
          <button class="btn-test-vote${community.find(v=>v.device==='_local')?.result==='pass'?' vote-pass':''}"
                  style="font-size:.72rem;padding:var(--space-sm) var(--space-md)"
                  onclick="castCommunityVote(${task.id},'pass')">✅ I tested it — works</button>
          <button class="btn-test-vote${community.find(v=>v.device==='_local')?.result==='fail'?' vote-fail':''}"
                  style="font-size:.72rem;padding:var(--space-sm) var(--space-md)"
                  onclick="castCommunityVote(${task.id},'fail')">❌ Broken</button>
        </div>
      </div>
    </div>`;
}

/**
 * Move a task to a new status via relay WebSocket (task_update message).
 * Falls back to a page alert if the user's WS is not connected.
 */
function changeTaskStatus(taskId, newStatus) {
  ensureTaskWs();
  if (!taskWs || taskWs.readyState !== WebSocket.OPEN) {
    alert('Sign in at /chat to update tasks.');
    return;
  }
  taskWs.send(JSON.stringify({ type: 'task_update', task_id: taskId, status: newStatus }));
  // Optimistic update so the detail drawer immediately reflects the change.
  const t = allTasks.find(t => t.id === taskId);
  if (t) {
    t.status = newStatus;
    openDetail(taskId);
    renderBoard();
  }
}

/**
 * Change the priority of a task via relay WebSocket.
 */
function changeTaskPriority(taskId, newPriority) {
  ensureTaskWs();
  if (!taskWs || taskWs.readyState !== WebSocket.OPEN) {
    alert('Sign in at /chat to update tasks.');
    return;
  }
  taskWs.send(JSON.stringify({ type: 'task_update', task_id: taskId, priority: newPriority }));
  const t = allTasks.find(t => t.id === taskId);
  if (t) {
    t.priority = newPriority;
    openDetail(taskId);
    renderBoard();
  }
}

/**
 * Save the assignee field for a task via relay WebSocket.
 */
function changeTaskAssignee(taskId) {
  const input = document.getElementById('detail-assignee-input');
  if (!input) return;
  const assignee = input.value.trim();
  ensureTaskWs();
  if (!taskWs || taskWs.readyState !== WebSocket.OPEN) {
    alert('Sign in at /chat to update tasks.');
    return;
  }
  taskWs.send(JSON.stringify({ type: 'task_update', task_id: taskId, assignee: assignee || '' }));
  const t = allTasks.find(t => t.id === taskId);
  if (t) { t.assignee = assignee || null; renderBoard(); }
}

/* ═══════════════════════════════════════════════════════════════
   Projects — project selector, CRUD, filtering
   ═══════════════════════════════════════════════════════════════ */

/** Fetch projects from REST API */
async function loadProjects() {
  try {
    const res = await fetch('/api/projects');
    if (!res.ok) { console.warn('[projects] HTTP', res.status); return; }
    const data = await res.json();
    projects = data || [];
    renderProjectDropdown();
    renderProjectSelectorBtn();
  } catch (e) {
    console.warn('[projects] load error:', e);
  }
}

/** Also request project_list via WS on connect */
function requestProjectList() {
  if (taskWs && taskWs.readyState === WebSocket.OPEN) {
    taskWs.send(JSON.stringify({ type: 'project_list' }));
  }
}

/** Build the project select in the task create modal */
function buildProjectSelect() {
  const sel = document.getElementById('f-project');
  if (!sel) return;
  let html = '<option value="default">General (default)</option>';
  projects.forEach(p => {
    const icon = p.icon || '📋';
    const selected = activeProject === p.id ? ' selected' : '';
    html += `<option value="${esc(p.id)}"${selected}>${icon} ${esc(p.name)}</option>`;
  });
  sel.innerHTML = html;
  // Pre-select active project if set
  if (activeProject && activeProject !== 'default') {
    sel.value = activeProject;
  }
}

/** Render the project selector button text/icon */
function renderProjectSelectorBtn() {
  const iconEl = document.getElementById('proj-btn-icon');
  const dotEl = document.getElementById('proj-btn-dot');
  const labelEl = document.getElementById('proj-btn-label');
  const countEl = document.getElementById('project-task-count');
  if (!iconEl || !dotEl || !labelEl) return;

  if (activeProject === null) {
    iconEl.textContent = '📋';
    dotEl.style.background = '#4488ff';
    labelEl.textContent = 'All Projects';
    if (countEl) countEl.textContent = allTasks.length + ' tasks';
  } else if (activeProject === 'default') {
    iconEl.textContent = '📋';
    dotEl.style.background = '#888';
    labelEl.textContent = 'General';
    const cnt = allTasks.filter(t => !t.project || t.project === 'default').length;
    if (countEl) countEl.textContent = cnt + ' tasks';
  } else {
    const proj = projects.find(p => p.id === activeProject);
    if (proj) {
      iconEl.textContent = proj.icon || '📋';
      dotEl.style.background = proj.color || '#4488ff';
      labelEl.textContent = proj.name;
      const cnt = allTasks.filter(t => t.project === proj.id).length;
      if (countEl) countEl.textContent = cnt + ' tasks';
    }
  }
}

/** Render the project dropdown menu */
function renderProjectDropdown() {
  const dd = document.getElementById('project-dropdown');
  if (!dd) return;
  let html = '';

  // All Projects option
  html += `<button class="project-dropdown-item${activeProject === null ? ' active' : ''}"
    onclick="setActiveProject(null)">
    <span class="proj-icon">📋</span>
    <span>All Projects</span>
  </button>`;

  html += '<div class="project-dropdown-sep"></div>';

  // Default / General project
  const defaultCount = allTasks.filter(t => !t.project || t.project === 'default').length;
  html += `<button class="project-dropdown-item${activeProject === 'default' ? ' active' : ''}"
    onclick="setActiveProject('default')">
    <span class="proj-icon">📋</span>
    <span class="proj-dot" style="background:#888"></span>
    <span>General</span>
    <span class="proj-vis">${defaultCount}</span>
  </button>`;

  // User projects
  projects.forEach(p => {
    const icon = p.icon || '📋';
    const color = p.color || '#4488ff';
    const vis = p.visibility === 'private' ? '🔒' : p.visibility === 'members-only' ? '👥' : '';
    const cnt = p.task_count != null ? p.task_count : allTasks.filter(t => t.project === p.id).length;
    html += `<button class="project-dropdown-item${activeProject === p.id ? ' active' : ''}"
      onclick="setActiveProject('${esc(p.id)}')" oncontextmenu="event.preventDefault();openProjectSettings('${esc(p.id)}')">
      <span class="proj-icon">${icon}</span>
      <span class="proj-dot" style="background:${color}"></span>
      <span>${esc(p.name)}</span>
      <span class="proj-vis">${vis} ${cnt}</span>
    </button>`;
  });

  html += '<div class="project-dropdown-sep"></div>';

  // Create new project
  html += `<button class="project-dropdown-item create-item" onclick="closeProjectDropdown();openProjectModal()">
    <span class="proj-icon">+</span>
    <span>Create New Project</span>
  </button>`;

  dd.innerHTML = html;
}

function toggleProjectDropdown() {
  const dd = document.getElementById('project-dropdown');
  if (!dd) return;
  const isOpen = dd.classList.contains('open');
  if (isOpen) {
    closeProjectDropdown();
  } else {
    renderProjectDropdown();
    dd.classList.add('open');
    // Close on outside click
    setTimeout(() => {
      document.addEventListener('click', _closeDropdownOutside, { once: true, capture: true });
    }, 0);
  }
}

function closeProjectDropdown() {
  const dd = document.getElementById('project-dropdown');
  if (dd) dd.classList.remove('open');
}

function _closeDropdownOutside(e) {
  const sel = document.getElementById('project-selector');
  if (sel && !sel.contains(e.target)) {
    closeProjectDropdown();
  } else {
    // Re-add listener if click was inside
    setTimeout(() => {
      document.addEventListener('click', _closeDropdownOutside, { once: true, capture: true });
    }, 0);
  }
}

/** Set active project and re-render */
function setActiveProject(id) {
  activeProject = id;
  closeProjectDropdown();
  renderProjectSelectorBtn();
  renderBoard();
  renderControls();
}

/** Build color picker swatches in the project modal */
function buildProjectColorPicker() {
  const container = document.getElementById('pf-colors');
  if (!container) return;
  container.innerHTML = PROJECT_COLORS.map((c, i) =>
    `<div class="color-swatch${i === 0 ? ' selected' : ''}" style="background:${c}"
      data-color="${c}" onclick="selectProjectColor(this)"></div>`
  ).join('') + `<input type="color" value="#4488ff" id="pf-color-custom"
    style="width:28px;height:28px;border:none;padding:0;cursor:pointer;background:transparent;border-radius:50%;"
    onchange="selectProjectColorCustom(this.value)" title="Custom color">`;
}

/** Build icon picker in the project modal */
function buildProjectIconPicker() {
  const container = document.getElementById('pf-icons');
  if (!container) return;
  container.innerHTML = PROJECT_ICONS.map((ic, i) =>
    `<div class="icon-option${i === 0 ? ' selected' : ''}" data-icon="${ic}" onclick="selectProjectIcon(this)">${ic}</div>`
  ).join('');
}

function selectProjectColor(el) {
  document.querySelectorAll('#pf-colors .color-swatch').forEach(s => s.classList.remove('selected'));
  el.classList.add('selected');
}

function selectProjectColorCustom(color) {
  document.querySelectorAll('#pf-colors .color-swatch').forEach(s => s.classList.remove('selected'));
  document.getElementById('pf-color-custom').value = color;
}

function selectProjectIcon(el) {
  document.querySelectorAll('#pf-icons .icon-option').forEach(s => s.classList.remove('selected'));
  el.classList.add('selected');
}

function getSelectedProjectColor() {
  const selected = document.querySelector('#pf-colors .color-swatch.selected');
  if (selected) return selected.dataset.color;
  return document.getElementById('pf-color-custom').value || '#4488ff';
}

function getSelectedProjectIcon() {
  const selected = document.querySelector('#pf-icons .icon-option.selected');
  return selected ? selected.dataset.icon : '📋';
}

/** Open the create/edit project modal */
function openProjectModal(existingId) {
  editingProjectId = existingId || null;
  const modal = document.getElementById('project-modal-overlay');
  const title = document.getElementById('project-modal-title');
  const btn = document.getElementById('btn-project-submit');
  const msg = document.getElementById('project-form-msg');
  if (msg) msg.innerHTML = '';

  if (editingProjectId) {
    const p = projects.find(x => x.id === editingProjectId);
    title.textContent = 'Edit Project';
    btn.textContent = 'Save Changes';
    if (p) {
      document.getElementById('pf-name').value = p.name;
      document.getElementById('pf-desc').value = p.description || '';
      // Select color
      document.querySelectorAll('#pf-colors .color-swatch').forEach(s => {
        s.classList.toggle('selected', s.dataset.color === p.color);
      });
      if (!document.querySelector('#pf-colors .color-swatch.selected')) {
        document.getElementById('pf-color-custom').value = p.color || '#4488ff';
      }
      // Select icon
      document.querySelectorAll('#pf-icons .icon-option').forEach(s => {
        s.classList.toggle('selected', s.dataset.icon === (p.icon || '📋'));
      });
      // Visibility
      const vis = p.visibility || 'public';
      document.querySelectorAll('input[name="pf-visibility"]').forEach(r => { r.checked = r.value === vis; });
    }
  } else {
    title.textContent = 'New Project';
    btn.textContent = 'Create Project';
    document.getElementById('pf-name').value = '';
    document.getElementById('pf-desc').value = '';
    buildProjectColorPicker();
    buildProjectIconPicker();
    document.querySelector('input[name="pf-visibility"][value="public"]').checked = true;
  }

  modal.classList.add('open');
  setTimeout(() => document.getElementById('pf-name').focus(), 50);
}

function closeProjectModal() {
  document.getElementById('project-modal-overlay').classList.remove('open');
  editingProjectId = null;
}

/** Submit create or update project via REST API */
async function submitProject() {
  const name = document.getElementById('pf-name').value.trim();
  const msg = document.getElementById('project-form-msg');
  const btn = document.getElementById('btn-project-submit');

  if (!name) { msg.innerHTML = '<div class="msg-error">Name is required.</div>'; return; }

  const body = {
    name,
    description: document.getElementById('pf-desc').value.trim(),
    color: getSelectedProjectColor(),
    icon: getSelectedProjectIcon(),
    visibility: document.querySelector('input[name="pf-visibility"]:checked')?.value || 'public',
  };

  btn.disabled = true;

  try {
    if (editingProjectId) {
      // Update via REST
      const res = await fetch('/api/projects/' + editingProjectId, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || 'HTTP ' + res.status);
      }
      const updated = await res.json().catch(() => body);
      const idx = projects.findIndex(p => p.id === editingProjectId);
      if (idx >= 0) projects[idx] = { ...projects[idx], ...body, ...updated };
      msg.innerHTML = '<div class="msg-success">Project updated.</div>';
    } else {
      // Create via WS if available, else REST
      if (taskWs && taskWs.readyState === WebSocket.OPEN) {
        taskWs.send(JSON.stringify({ type: 'project_create', ...body }));
        msg.innerHTML = '<div class="msg-success">Project created.</div>';
      } else {
        const res = await fetch('/api/projects', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
        if (!res.ok) {
          const data = await res.json().catch(() => ({}));
          throw new Error(data.message || 'HTTP ' + res.status);
        }
        const created = await res.json();
        if (created && created.id) projects.push(created);
        msg.innerHTML = '<div class="msg-success">Project created.</div>';
      }
    }
    renderProjectDropdown();
    renderProjectSelectorBtn();
    setTimeout(closeProjectModal, 600);
  } catch (e) {
    msg.innerHTML = `<div class="msg-error">Error: ${esc(e.message)}</div>`;
  } finally {
    btn.disabled = false;
  }
}

/** Open project settings (edit) — triggered by right-click on dropdown item */
function openProjectSettings(id) {
  closeProjectDropdown();
  openProjectModal(id);
}

/** Delete a project via REST API */
async function deleteProject(id) {
  if (!confirm('Delete this project? Its tasks will be moved to General.')) return;
  try {
    const res = await fetch('/api/projects/' + id, { method: 'DELETE' });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    projects = projects.filter(p => p.id !== id);
    if (activeProject === id) activeProject = null;
    // Reassign tasks locally
    allTasks.forEach(t => { if (t.project === id) t.project = 'default'; });
    renderProjectDropdown();
    renderProjectSelectorBtn();
    renderBoard();
    renderControls();
    closeProjectModal();
  } catch (e) {
    alert('Failed to delete project: ' + e.message);
  }
}

/* ── Helpers ── */
function esc(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

function priorityColor(p) {
  return { critical:'#e53935', high:'#f0a500', medium:'#2196f3', low:'#555' }[p] || '#555';
}

function timeAgo(ms) {
  const s = Math.floor((Date.now() - ms) / 1000);
  if (s < 60) return s + 's';
  if (s < 3600) return Math.floor(s/60) + 'm';
  if (s < 86400) return Math.floor(s/3600) + 'h';
  return Math.floor(s/86400) + 'd';
}

/* Close modal on overlay click */
document.getElementById('modal-overlay').addEventListener('click', e => {
  if (e.target === e.currentTarget) closeModal();
});

/* Escape key */
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') { closeDetail(); closeModal(); closeProjectModal(); closeProjectDropdown(); quest_closeModal(); }
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') quest_saveQuest();
});

init();

/* ═══════════════════════════════════════════════════════════════
   View tab switching (Kanban / Quests)
   ═══════════════════════════════════════════════════════════════ */
function switchView(view) {
  document.querySelectorAll('.view-tab').forEach(t => t.classList.toggle('active', t.dataset.view === view));
  document.getElementById('kanban-content').style.display = view === 'kanban' ? 'flex' : 'none';
  document.getElementById('quests-content').style.display = view === 'quests' ? '' : 'none';
  document.getElementById('hint-kanban').style.display = view === 'kanban' ? '' : 'none';
  document.getElementById('hint-quests').style.display = view === 'quests' ? '' : 'none';
}

/* ═══════════════════════════════════════════════════════════════
   Quests — local quest tracker (localStorage hos_quests_v1)
   All functions prefixed with quest_ to avoid collisions.
   ═══════════════════════════════════════════════════════════════ */
const QUEST_STORAGE_KEY = 'hos_quests_v1';
let quest_list = [];
let quest_currentFilter = 'all';
let quest_editingId = null;

function quest_load() {
  try { quest_list = JSON.parse(localStorage.getItem(QUEST_STORAGE_KEY)) || []; }
  catch(e) { quest_list = []; }
}
function quest_save() {
  localStorage.setItem(QUEST_STORAGE_KEY, JSON.stringify(quest_list));
}
function quest_uid() {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}
function quest_esc(s) {
  return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

function quest_setFilter(f) {
  quest_currentFilter = f;
  document.querySelectorAll('.q-cat').forEach(el => el.classList.toggle('active', el.dataset.filter === f));
  const labels = { all:'All Quests', daily:'Daily Quests', story:'Story Quests',
    side:'Side Quests', personal:'Personal Quests', done:'Completed' };
  document.getElementById('quest-heading').textContent = labels[f] || 'Quests';
  quest_renderList();
}

function quest_renderList() {
  const search = (document.getElementById('quest-search').value || '').toLowerCase();
  const list = document.getElementById('quest-list');

  // Update counts
  const counts = { all:0, daily:0, story:0, side:0, personal:0, done:0 };
  quest_list.forEach(q => {
    if (!q.done) { counts.all++; counts[q.type] = (counts[q.type] || 0) + 1; }
    else counts.done++;
  });
  Object.keys(counts).forEach(k => {
    const el = document.getElementById('cnt-' + k);
    if (el) el.textContent = counts[k];
  });

  let visible = quest_list.filter(q => {
    if (quest_currentFilter === 'done')  return q.done;
    if (quest_currentFilter !== 'all')   return !q.done && q.type === quest_currentFilter;
    return !q.done;
  });
  if (search) {
    visible = visible.filter(q =>
      q.title.toLowerCase().includes(search) ||
      (q.desc || '').toLowerCase().includes(search)
    );
  }

  if (!visible.length) {
    list.innerHTML = `<div class="quest-empty-state"><div class="empty-icon">${typeof hosIcon==='function'?hosIcon('tasklist',48):''}</div>
      <p>${search ? 'No quests match your search.' : quest_currentFilter === 'done' ? 'No completed quests yet.' : 'No quests here yet.'}</p>
      ${!search && quest_currentFilter !== 'done' ? '<button class="quest-btn-save" onclick="quest_openModal()" style="margin-top:var(--space-md);">+ Add Your First Quest</button>' : ''}</div>`;
    return;
  }

  const today = new Date().toISOString().slice(0, 10);
  list.innerHTML = visible.map(q => {
    const overdue = q.due && q.due < today && !q.done;
    const badgeClass = 'badge-' + q.type;
    return `<div class="quest-card${q.done ? ' done' : ''}" onclick="quest_openModal('${q.id}')">
      <div class="quest-check" onclick="event.stopPropagation();quest_toggleDone('${q.id}')">${q.done ? '✓' : ''}</div>
      <div class="quest-body">
        <div class="quest-title">${quest_esc(q.title)}</div>
        ${q.desc ? `<div class="quest-desc">${quest_esc(q.desc)}</div>` : ''}
        <div class="quest-meta">
          <span class="quest-xp">⭐ ${q.xp || 0} XP</span>
          <span class="quest-type-badge ${badgeClass}">${q.type}</span>
          ${q.due ? `<span class="quest-due${overdue ? ' overdue' : ''}">${overdue ? '⚠ ' : ''}${q.due}</span>` : ''}
        </div>
      </div>
      <button class="quest-delete" onclick="event.stopPropagation();quest_deleteQuest('${q.id}')" title="Delete">✕</button>
    </div>`;
  }).join('');
}

function quest_toggleDone(id) {
  const q = quest_list.find(x => x.id === id);
  if (!q) return;
  const wasNotDone = !q.done;
  q.done = !q.done;
  q.updatedAt = Date.now();
  quest_save();
  quest_renderList();
  // Award XP to linked skill when a quest is completed (not un-completed)
  if (wasNotDone && q.skill_id && q.xp > 0) {
    awardSkillXp(q.skill_id, q.xp);
  }
}

/**
 * Award XP to a skill in hos_skills_v1 when a linked quest is completed.
 * Applies level-ups automatically using the same XP table as skills.html.
 */
function awardSkillXp(skillId, amount) {
  const XP_PER_LEVEL = [100,150,225,338,507,760,1140,1710,2565,3848];
  const MAX_LEVEL = 10;
  try {
    const data = JSON.parse(localStorage.getItem('hos_skills_v1') || '{}');
    if (!data[skillId]) data[skillId] = { level: 0, xp: 0 };
    const s = data[skillId];
    if (s.level >= MAX_LEVEL) return;
    s.xp = (s.xp || 0) + amount;
    let leveled = false;
    while (s.level < MAX_LEVEL && s.xp >= (XP_PER_LEVEL[s.level] || 100)) {
      s.xp -= XP_PER_LEVEL[s.level] || 100;
      s.level++;
      leveled = true;
    }
    localStorage.setItem('hos_skills_v1', JSON.stringify(data));
    const msg = leveled
      ? `🎉 Quest complete! +${amount} XP to "${skillId}" → Level ${s.level}!`
      : `✅ Quest complete! +${amount} XP awarded to "${skillId}"`;
    quest_showToast(msg, 4000);
  } catch(e) { console.warn('awardSkillXp error:', e); }
}

function quest_deleteQuest(id) {
  quest_list = quest_list.filter(x => x.id !== id);
  quest_save(); quest_renderList();
}

function quest_openModal(id) {
  quest_editingId = id || null;
  const q = id ? quest_list.find(x => x.id === id) : null;
  document.getElementById('quest-modal-title').textContent = q ? 'Edit Quest' : 'New Quest';
  document.getElementById('q-title').value = q ? q.title : '';
  document.getElementById('q-desc').value  = q ? (q.desc || '') : '';
  document.getElementById('q-type').value  = q ? q.type : 'personal';
  document.getElementById('q-xp').value    = q ? (q.xp || 25) : 25;
  document.getElementById('q-due').value   = q ? (q.due || '') : '';
  document.getElementById('q-skill').value = q ? (q.skill_id || '') : '';
  document.getElementById('quest-modal').classList.add('open');
  setTimeout(() => document.getElementById('q-title').focus(), 50);
}

function quest_closeModal() {
  document.getElementById('quest-modal').classList.remove('open');
  quest_editingId = null;
}

function quest_saveQuest() {
  const title = document.getElementById('q-title').value.trim();
  if (!title) { document.getElementById('q-title').focus(); return; }
  if (quest_editingId) {
    const q = quest_list.find(x => x.id === quest_editingId);
    if (q) {
      q.title = title;
      q.desc  = document.getElementById('q-desc').value.trim();
      q.type  = document.getElementById('q-type').value;
      q.xp       = parseInt(document.getElementById('q-xp').value) || 0;
      q.due      = document.getElementById('q-due').value || null;
      q.skill_id = document.getElementById('q-skill').value.trim().toLowerCase() || null;
    }
  } else {
    quest_list.unshift({
      id:       quest_uid(),
      title:    title,
      desc:     document.getElementById('q-desc').value.trim(),
      type:     document.getElementById('q-type').value,
      xp:       parseInt(document.getElementById('q-xp').value) || 0,
      due:      document.getElementById('q-due').value || null,
      skill_id: document.getElementById('q-skill').value.trim().toLowerCase() || null,
      done:     false,
      created:  new Date().toISOString(),
      updatedAt: Date.now(),
    });
  }
  quest_save(); quest_closeModal(); quest_renderList();
}

// Close quest modal on backdrop click
document.getElementById('quest-modal').addEventListener('click', function(e) {
  if (e.target === this) quest_closeModal();
});

function quest_showToast(msg, duration) {
  let el = document.getElementById('quest-toast');
  if (!el) {
    el = document.createElement('div');
    el.id = 'quest-toast';
    el.style.cssText = 'position:fixed;bottom:3rem;left:50%;transform:translateX(-50%);background:#222;border:1px solid #f0a500;color:#e0e0e0;padding:var(--space-lg) 1.2rem;border-radius:8px;font-size:0.82rem;z-index:9999;max-width:340px;text-align:center;transition:opacity .3s;';
    document.body.appendChild(el);
  }
  el.textContent = msg;
  el.style.opacity = '1';
  clearTimeout(el._timer);
  el._timer = setTimeout(() => { el.style.opacity = '0'; }, duration || 3000);
}

// Initialize quests
quest_load();
quest_renderList();
document.querySelectorAll('.pg-ico[data-icon]').forEach(function(el) {
  if (window.hosIcon) el.innerHTML = hosIcon(el.dataset.icon, parseInt(el.dataset.size) || 20);
});
