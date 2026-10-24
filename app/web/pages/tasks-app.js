/* ── Words ──
 * Every word this page shows comes from data/i18n/ by key, through
 * /shared/i18n.js, which tasks.html loads (with its English, i18n-en.js) before
 * this file; a test that runs this file loads the same two first
 * (scripts/tests/erase-sign-up-again.test.js). */

/* ── Fibonacci scope definitions ──
 * The taxonomy lives in data/tasks/scopes.json (infinite-of-x); boot() fetches
 * it before init(). The literal below is only the offline emergency fallback. */
let SCOPES = [
  { n:1,  key:'self',   label:'Self',   color:'#ff6b6b', desc:'Personal health, body, daily needs' },
  { n:8,  key:'city',   label:'City',   color:'#0fbcf9', desc:'Project level, HumanityOS (default)' },
  { n:55, key:'cosmos', label:'Cosmos', color:'#dfe6e9', desc:'Interstellar civilization' },
];
function defaultScope() {
  return SCOPES.find(s => s.key === 'city') || SCOPES[SCOPES.length - 1];
}

/* ── Skill XP curve (data/skills/xp_curve.json; fallback if fetch fails) ── */
let XP_PER_LEVEL = [100,150,225,338,507,760,1140,1710,2565,3848];
let MAX_LEVEL = 10;

let allTasks = [];
// Default to 'city', which is where tasks with no explicit scope actually land
// (see getTaskScope) and what the create form pre-selects. Defaulting to
// 'cosmos' hid the common case and made the board look empty on load.
let activeScope = 'city';
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
    <button class="scope-tab${s.key === activeScope ? ' active' : '' /* i18n-exempt: a class name */}"
            data-key="${s.key}" onclick="setScope('${s.key}')"
            title="${s.desc}" data-i18n-tip="tasks.scope_tab_hint">
      <span class="scope-fib scope-ink" style="--scope-c:${s.color}">${s.n}</span>
      <span class="scope-label">${s.label}</span>
      <span class="scope-tab-dot" style="background:${s.color}"></span>
    </button>`).join('');
  i18n.apply(container);
}

function buildScopeSelect() {
  const sel = document.getElementById('f-scope');
  sel.innerHTML = SCOPES.map(s =>
    `<option value="${s.key}"${s.key === 'city' ? ' selected' : '' /* i18n-exempt: an attribute, not words */}>${s.n}·${s.label}, ${s.desc}</option>`
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
    document.getElementById('col-backlog').innerHTML = `<div class="empty-col">⚠️ ${i18n.t('tasks.load_failed')}<br><small>${e.message}</small></div>`;
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
  // Whole-board empty state: a wall of "-" reads as broken. Say plainly what to
  // do. This board loads tasks from the server, so a logged-out or empty
  // visitor sees a friendly line instead of a void.
  const boardEl = document.getElementById('board') || document.getElementById('col-backlog')?.parentElement;
  if (!tasks.length && boardEl) {
    const anyAtAll = allTasks.length > 0;
    const msg = anyAtAll
      ? i18n.t('tasks.empty_scope')
      : i18n.t('tasks.empty_board');
    ['backlog','in_progress','testing','done'].forEach(status => {
      const c = document.getElementById('count-' + status);
      if (c) c.textContent = '0';
    });
    const first = document.getElementById('col-backlog');
    if (first) first.innerHTML = `<div class="empty-col">${esc(msg)}</div>`;
    ['in_progress','testing','done'].forEach(s => {
      const col = document.getElementById('col-' + s);
      if (col) col.innerHTML = '';
    });
    return;
  }
  ['backlog','in_progress','testing','done'].forEach(status => {
    const col = document.getElementById('col-' + status);
    const count = document.getElementById('count-' + status);
    const filtered = tasks.filter(t => t.status === status);
    count.textContent = filtered.length;
    if (!filtered.length) { col.innerHTML = `<div class="empty-col">-</div>`; return; }
    try {
      col.innerHTML = filtered.map(renderCard).join('');
      i18n.apply(col);
    } catch (e) {
      col.innerHTML = `<div class="empty-col">⚠️ ${i18n.t('tasks.render_error')}<br><small>${esc(e.message)}</small></div>`;
      console.error('renderCard error in', status, e);
    }
  });
}

function renderControls() {
  const scope = SCOPES.find(s => s.key === activeScope) || defaultScope();
  document.getElementById('scope-desc').innerHTML =
    `<strong class="scope-ink" style="--scope-c:${scope.color}">${scope.n}·${scope.label}</strong>: ${scope.desc}`;
  const tasks = scopedTasks();
  const ip = allTasks.filter(t => t.status === 'in_progress').length;
  // The counts are numbers, so their markup can ride in a word's placeholder.
  document.getElementById('stats-bar').innerHTML =
    `<span class="stat-pill">${i18n.t('tasks.stat_total', { n: `<span>${allTasks.length}</span>` })}</span>` +
    `<span class="stat-pill">${i18n.t('tasks.stat_visible', { n: `<span>${tasks.length}</span>` })}</span>` +
    `<span class="stat-pill">${i18n.t('tasks.stat_active', { n: `<span>${ip}</span>` })}</span>`;
}

function renderCard(task) {
  const scope = SCOPES.find(s => s.key === getTaskScope(task)) || defaultScope();
  const labels = getNonScopeLabels(task);
  // Test tally for testing-column cards
  let testTally = '';
  if (task.status === 'testing' || task.status === 'done') {
    const tv = loadVotes()[task.id] || {};
    const passes = (tv.community||[]).filter(v=>v.result==='pass').length + (tv.owner==='pass'?1:0);
    const fails  = (tv.community||[]).filter(v=>v.result==='fail').length + (tv.owner==='fail'?1:0);
    if (passes || fails) {
      testTally = `<span class="test-tally" title="${i18n.t('tasks.test_tally_title')}">${passes?'✅'+passes:''}${fails?'❌'+fails:''}</span>`;
    } else if (task.status === 'testing') {
      testTally = `<span class="test-tally" style="color:#555" title="${i18n.t('tasks.waiting_testing')}">🧪</span>`;
    }
  }
  return `<div role="button" tabindex="0" class="card priority-${task.priority}" onclick="openDetail(${task.id})" data-i18n-tip="tasks.card_hint">
    <div class="card-title">${esc(task.title)}</div>
    <div class="card-info">
      <span class="card-prio" style="color:${priorityColor(task.priority)}">${priorityName(task.priority).toUpperCase()}</span>
      <span class="card-sep">·</span>
      <span>${timeAgo(task.created_at)}</span>
      <span class="card-sep">·</span>
      <span class="scope-badge scope-ink" style="--scope-c:${scope.color}">${scope.n}·${scope.label}</span>
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

/* ── Relay socket: live board updates + task edits ──
 * The relay binds a socket to a person only after proof of the key. The page
 * sends `identify` with the person's Dilithium3 public key, the relay replies
 * `identify_challenge {nonce}`, and the page signs
 *   "hum/identify/v1\n" + nonce + "\n" + public_key_hex
 * and returns `identify_response {sig_b64}` (src/relay/relay.rs, Inc3b). Until
 * that check passes the relay drops every task message on the socket without
 * a reply, and closes the socket after 30 seconds.
 * The key comes from the identity Chat keeps in this browser
 * (getPqIdentity in /shared/pq-relay-auth.js, derived by /chat/pq.js): the
 * same key Chat signs in with, and no WebCrypto Ed25519 is needed, so it also
 * works on older phone browsers. */
let taskWs = null;
let taskWsBound = false;     // true once the relay accepted the proof (its peer_list arrived)
let taskWsPending = null;    // resolve fn waiting for task_created confirmation
let taskWsWaiters = [];      // callbacks waiting for the socket to bind (true) or fail (false)
let taskWsRefusal = '';      // the relay's reason when it refused this page's sign-in
let taskWsRetryMs = 5000;    // reconnect delay, doubles up to a minute while the link keeps dropping
let taskWsRetryTimer = null;
let taskIdentity = null;     // { dilithiumPublicHex, dilithiumSecret } once derived

/** The exact bytes the relay verifies: format!("hum/identify/v1\n{}\n{}", nonce, public_key). */
function taskIdentifyPreimage(nonce, publicKeyHex) {
  return new TextEncoder().encode('hum/identify/v1\n' + nonce + '\n' + publicKeyHex);
}

/** Standard base64 (what the relay's B64.decode expects), without spreading 3309 bytes into one call. */
function taskBytesToB64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
  return btoa(s);
}

/** The identify_response message answering the relay's challenge, or null if signing failed. */
async function taskIdentifyResponse(identity, nonce) {
  if (!identity || !identity.dilithiumSecret || !identity.dilithiumPublicHex || !nonce) return null;
  if (typeof window.pqSignMessage !== 'function') return null;
  const sig = await window.pqSignMessage(identity.dilithiumSecret, taskIdentifyPreimage(nonce, identity.dilithiumPublicHex));
  if (!sig) return null;
  return { type: 'identify_response', sig_b64: taskBytesToB64(sig) };
}

/** This browser's Dilithium3 identity, or null when Chat has not set one up here. */
async function loadTaskIdentity() {
  if (taskIdentity) return taskIdentity;
  if (typeof window.getPqIdentity !== 'function') return null;
  try { taskIdentity = await window.getPqIdentity(); } catch { taskIdentity = null; }
  return taskIdentity;
}

function settleTaskWsWaiters(ok) {
  const waiters = taskWsWaiters;
  taskWsWaiters = [];
  waiters.forEach(fn => fn(ok));
}

/** The relay said no: remember why, stop retrying, and close (it would hold the socket open 30s). */
function refuseTaskWs(ws, reason) {
  taskWsRefusal = reason || i18n.t('tasks.refused_default');
  console.warn('[tasks] sign-in refused:', taskWsRefusal);
  settleTaskWsWaiters(false);
  try { ws.close(); } catch {}
}

/** What to tell the person when an edit needs the signed-in socket and it is not there. */
function taskSignInHelp() {
  if (taskWsRefusal) return i18n.t('tasks.signin_refused', { reason: taskWsRefusal });
  if (!localStorage.getItem('humanity_key_backup')) {
    return i18n.t('tasks.signin_open_chat');
  }
  return i18n.t('tasks.signin_failed');
}

/** Resolves true once the socket is signed in, false if it cannot be (no identity, refused, timeout). */
function whenTaskWsBound(timeoutMs) {
  if (taskWs && taskWsBound && taskWs.readyState === WebSocket.OPEN) return Promise.resolve(true);
  return new Promise(resolve => {
    let done = false;
    const timer = setTimeout(() => finish(false), timeoutMs || 8000);
    function finish(ok) { if (done) return; done = true; clearTimeout(timer); resolve(ok); }
    taskWsWaiters.push(finish);
    if (!taskWs) {
      // A click is a fresh request, so an earlier refusal gets one more try.
      taskWsRefusal = '';
      clearTimeout(taskWsRetryTimer);
      ensureTaskWs().then(started => { if (!started) settleTaskWsWaiters(false); });
    }
  });
}

/** Open the socket and sign in. Resolves true if a socket is open or on its way, false if there is no identity. */
async function ensureTaskWs() {
  if (taskWs) return true;
  const id = await loadTaskIdentity();
  if (!id) return false;
  if (taskWs) return true; // another call opened it while the key was being derived
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(`${proto}//${location.host}/ws`);
  taskWs = ws;
  taskWsBound = false;
  ws.addEventListener('open', () => {
    // A placeholder name is never sent: the relay registers any real-looking
    // name to the key, so a made-up default would claim a name for this person.
    // The name is the one Chat saved in this browser ('humanity_name', written
    // by /chat/app.js and the profile editor). With none, the relay signs this
    // socket in under the name already registered to the key (2026-10-02), so
    // the board never shows the person to others as nameless.
    const name = (localStorage.getItem('humanity_name') || '').trim() || null;
    ws.send(JSON.stringify({ type: 'identify', public_key: id.dilithiumPublicHex, display_name: name }));
  });
  ws.addEventListener('message', e => {
    try {
      const m = JSON.parse(e.data);
      // ── Sign-in phase: nothing else counts until the relay accepts the proof ──
      if (!taskWsBound) {
        if (m.type === 'identify_challenge') {
          taskIdentifyResponse(id, m.nonce).then(resp => {
            if (!resp) { refuseTaskWs(ws, i18n.t('tasks.refused_sign_check')); return; }
            if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(resp));
          });
        } else if (m.type === 'peer_list') {
          // The relay sends peer_list first thing after it binds the socket.
          taskWsBound = true;
          taskWsRefusal = '';
          taskWsRetryMs = 5000;
          settleTaskWsWaiters(true);
          requestProjectList();
        } else if (m.type === 'account_erased') {
          // This identity's account was erased on this server (BUG-135): the relay remembers
          // that for a while and signs nothing up for it, then closes. A refusal, so the page
          // says why and waits for the next click instead of retrying every few seconds.
          // Coming back is the person's choice, made on the Chat page.
          refuseTaskWs(ws, m.partial === true
            ? i18n.t('tasks.refused_erase_unfinished')
            : i18n.t('tasks.refused_erased'));
        } else if (m.type === 'name_taken' || m.type === 'system') {
          const text = String(m.message || '');
          // The per-connection throttle: the relay closes the socket, and the close handler backs off.
          if (m.type === 'system' && text.startsWith('Too many connection attempts')) return;
          refuseTaskWs(ws, text);
        }
        return;
      }
      // Resolve pending create promise
      if (m.type === 'task_created' && taskWsPending) { taskWsPending(m.task); taskWsPending = null; }
      if (m.type === 'system' && m.message && taskWsPending && !m.message.startsWith('__')) { taskWsPending(null, m.message); taskWsPending = null; }
      // Real-time board updates, add/update tasks without full reload
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
      // Project real-time updates (the relay answers project_list with project_list_response)
      if (m.type === 'project_list_response') {
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
  ws.addEventListener('close', () => {
    if (taskWs !== ws) return;
    taskWs = null;
    taskWsBound = false;
    settleTaskWsWaiters(false);
    // After a refusal, retrying would only repeat it (and spend the relay's
    // per-connection allowance); the next click tries again instead.
    if (taskWsRefusal) return;
    clearTimeout(taskWsRetryTimer);
    taskWsRetryTimer = setTimeout(ensureTaskWs, taskWsRetryMs);
    taskWsRetryMs = Math.min(taskWsRetryMs * 2, 60000);
  });
  return true;
}

async function submitTask() {
  const title = document.getElementById('f-title').value.trim();
  const apiKey = document.getElementById('f-apikey').value.trim();
  const msg = document.getElementById('form-msg');

  if (!title) { msg.innerHTML = `<div class="msg-error">${i18n.t('tasks.title_required')}</div>`; return; }

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
  setWords(btn, 'tasks.creating', i18n.t('tasks.creating'));

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
      msg.innerHTML = `<div class="msg-success">✓ ${i18n.t('tasks.task_created', { id: data.id })}</div>`;
      setTimeout(() => { closeModal(); loadTasks(); }, 800);
    } catch(e) {
      msg.innerHTML = `<div class="msg-error">${i18n.t('tasks.error_message', { message: esc(e.message) })}</div>`;
    } finally {
      btn.disabled = false;
      setWords(btn, 'tasks.create_task', i18n.t('tasks.create_task'));
    }
  } else {
    // Relay WebSocket path: the socket must be signed in (see ensureTaskWs),
    // or the relay drops task_create without a word.
    if (!await whenTaskWsBound()) {
      msg.innerHTML = `<div class="msg-error">${esc(taskSignInHelp())}</div>`;
      btn.disabled = false;
      setWords(btn, 'tasks.create_task', i18n.t('tasks.create_task'));
      return;
    }
    try {
      const task = await new Promise((resolve, reject) => {
        taskWsPending = (task, err) => err ? reject(new Error(err)) : resolve(task);
        taskWs.send(JSON.stringify({ type: 'task_create', ...body }));
        setTimeout(() => { if (taskWsPending) { taskWsPending = null; reject(new Error('Timeout')); } }, 5000);
      });
      msg.innerHTML = `<div class="msg-success">✓ ${i18n.t('tasks.task_created', { id: task?.id || '?' })}</div>`;
      setTimeout(() => { closeModal(); loadTasks(); }, 800);
    } catch(e) {
      msg.innerHTML = `<div class="msg-error">${i18n.t('tasks.error_message', { message: esc(e.message) })}</div>`;
    } finally {
      btn.disabled = false;
      setWords(btn, 'tasks.create_task', i18n.t('tasks.create_task'));
    }
  }
}

function openDetail(id) {
  openDetailId = id;
  const task = allTasks.find(t => t.id === id);
  if (!task) return;

  const scope = SCOPES.find(s => s.key === getTaskScope(task)) || defaultScope();
  const labels = getNonScopeLabels(task);

  document.getElementById('detail-id').textContent = '#' + task.id;
  document.getElementById('detail-title').textContent = task.title;

  const scopeBadge = document.getElementById('detail-scope-badge');
  scopeBadge.textContent = scope.n + '·' + scope.label;
  scopeBadge.classList.add('scope-ink');
  scopeBadge.style.setProperty('--scope-c', scope.color);

  const statusLabel = statusName(task.status);
  document.getElementById('detail-badges').innerHTML =
    `<span class="detail-badge badge-status-${task.status}">${statusLabel}</span>` +
    `<span class="detail-badge badge-priority-${task.priority}">${priorityName(task.priority)}</span>`;

  // Status action buttons, clicking moves the task to that status.
  const statusSteps = [
    { key: 'backlog',     icon: 'tasklist' },
    { key: 'in_progress', icon: 'settings' },
    { key: 'testing',     icon: 'search' },
    { key: 'done',        icon: 'check' },
  ].map(s => ({ key: s.key, name: statusName(s.key), label: (typeof hosIcon==='function'?hosIcon(s.icon,14):'') + ' ' + statusName(s.key) }));
  // The title names the column in words only: the label carries the icon's markup.
  const statusBtns = document.getElementById('detail-status-btns');
  statusBtns.innerHTML =
    statusSteps.map(s =>
      s.key === task.status
        ? `<button class="btn-status current current-${s.key}" disabled title="${i18n.t('tasks.current_status')}" data-i18n-tip="tasks.status_current_hint">${s.label}</button>`
        : `<button class="btn-status" onclick="changeTaskStatus(${task.id},'${s.key}')" title="${i18n.t('tasks.move_to', { status: s.name })}" data-i18n-tip="tasks.status_button_hint">${s.label}</button>`
    ).join('') +
    `<span class="status-move-label">← ${i18n.t('tasks.move_to_label')}</span>`;
  i18n.apply(statusBtns);

  // Priority change buttons.
  const priorities = [
    { key: 'low',      glyph: '▽' },
    { key: 'medium',   glyph: '◆' },
    { key: 'high',     glyph: '▲' },
    { key: 'critical', glyph: '🔥' },
  ].map(p => ({ key: p.key, label: p.glyph + ' ' + priorityName(p.key) }));
  const priorityBtns = document.getElementById('detail-priority-btns');
  priorityBtns.innerHTML =
    priorities.map(p =>
      p.key === task.priority
        ? `<button class="btn-status current current-${p.key}" disabled title="${i18n.t('tasks.current_priority')}" data-i18n-tip="tasks.priority_current_hint">${p.label}</button>`
        : `<button class="btn-status" onclick="changeTaskPriority(${task.id},'${p.key}')" title="${i18n.t('tasks.set_priority_to', { priority: p.label })}" data-i18n-tip="tasks.priority_button_hint">${p.label}</button>`
    ).join('') +
    `<span class="status-move-label">← ${i18n.t('tasks.priority_arrow_label')}</span>`;
  i18n.apply(priorityBtns);

  // Assignee inline editor.
  const assigneeRow = document.getElementById('detail-assignee-row');
  assigneeRow.innerHTML =
    `<div class="assignee-row">
       <span style="font-size:0.68rem;color:var(--text-dim);min-width:65px;flex-shrink:0">${i18n.t('tasks.assignee')}</span>
       <input id="detail-assignee-input" type="text" value="${esc(task.assignee || '')}" placeholder="${i18n.t('tasks.unassigned')}" maxlength="80" data-i18n-tip="tasks.assignee_input_hint">
       <button onclick="changeTaskAssignee(${task.id})" title="${i18n.t('tasks.save_assignee')}" data-i18n-tip="tasks.assignee_save_hint">${i18n.t('common.save')}</button>
     </div>`;
  i18n.apply(assigneeRow);

  document.getElementById('detail-desc').textContent = task.description || i18n.t('tasks.no_description');

  document.getElementById('detail-labels').innerHTML = labels.length
    ? labels.map(l => `<span class="label-tag">${esc(l)}</span>`).join('')
    : '';

  const created = task.created_at ? new Date(task.created_at).toLocaleString() : '-';
  const age = task.created_at ? i18n.t('tasks.time_ago', { time: timeAgo(task.created_at) }) : '';
  document.getElementById('detail-meta').innerHTML =
    `<div class="detail-meta-row"><span class="detail-meta-label">${i18n.t('tasks.created')}</span><span>${created}${age ? ' · ' + age : ''}</span></div>` +
    (task.assignee ? `<div class="detail-meta-row"><span class="detail-meta-label">${i18n.t('tasks.assignee')}</span><span>${esc(task.assignee)}</span></div>` : '') +
    (task.comment_count > 0 ? `<div class="detail-meta-row"><span class="detail-meta-label">${i18n.t('tasks.comments')}</span><span>${task.comment_count}</span></div>` : '');

  // Test voting panel (shown for testing and done tasks)
  const testPanelEl = document.getElementById('detail-test-panel');
  if (task.status === 'testing' || task.status === 'done') {
    testPanelEl.innerHTML = renderTestPanel(task);
    i18n.apply(testPanelEl);
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
    el.innerHTML = comments.length === 0 ? `<div style="color:var(--text-muted);font-size:0.78rem;padding:var(--space-sm) 0">${i18n.t('tasks.no_comments')}</div>` :
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
  if (await whenTaskWsBound()) {
    taskWs.send(JSON.stringify({ type: 'task_comment', task_id: taskId, content }));
    input.value = '';
    // Update comment count optimistically
    const t = allTasks.find(t => t.id === taskId);
    if (t) t.comment_count = (t.comment_count || 0) + 1;
    await new Promise(r => setTimeout(r, 500));
    await loadTaskComments(taskId);
  } else {
    alert(taskSignInHelp());
  }
  input.disabled = false;
  input.focus();
}

function closeDetail() {
  document.getElementById('detail-overlay').classList.remove('open');
}

/* ── Test votes ──
 * Kept in this browser's localStorage and nowhere else: they are never sent
 * to the server, so nobody else sees them. They used to carry an Ed25519
 * signature that nothing ever checked, and that failed silently on browsers
 * without WebCrypto Ed25519. A signature implies someone verifies it, so it is
 * gone, and the panel says plainly where the votes live. */
const VOTES_KEY = 'hos_task_votes';
function loadVotes() { try { return JSON.parse(localStorage.getItem(VOTES_KEY)) || {}; } catch { return {}; } }
function saveVotes(v) { localStorage.setItem(VOTES_KEY, JSON.stringify(v)); }

/**
 * Cast the owner/determining test vote on a task (this browser only).
 * Toggling the same result clears it.
 */
function castOwnerVote(taskId, result) {
  const v = loadVotes();
  if (!v[taskId]) v[taskId] = { owner: null, community: [] };
  v[taskId].owner = v[taskId].owner === result ? null : result;
  saveVotes(v);
  openDetail(taskId);
  renderBoard();
}

/**
 * Cast a tester vote (advisory, this browser only). Re-voting replaces the
 * previous one; voting the same result again clears it.
 */
function castCommunityVote(taskId, result) {
  const v = loadVotes();
  if (!v[taskId]) v[taskId] = { owner: null, community: [] };
  if (!Array.isArray(v[taskId].community)) v[taskId].community = [];
  const mine = v[taskId].community.find(x => x.device === '_local');
  if (mine && mine.result === result) {
    v[taskId].community = v[taskId].community.filter(x => x.device !== '_local');
  } else if (mine) {
    mine.result = result;
    mine.time = Date.now();
  } else {
    v[taskId].community.push({ device: '_local', result, time: Date.now() });
  }
  saveVotes(v);
  openDetail(taskId);
  renderBoard();
}

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
          <span class="test-vote-name">${v.device === '_local' ? i18n.t('tasks.vote_you') : esc(v.name||i18n.t('tasks.vote_anonymous'))}</span>
          <span class="test-vote-time">${v.time ? i18n.t('tasks.time_ago', { time: timeAgo(v.time) }) : ''}</span>
        </div>`).join('')
    : `<div style="font-size:0.72rem;color:#555;padding:var(--space-xs) 0">${i18n.t('tasks.no_votes')}</div>`;
  const yourVote = ' ← ' + i18n.t('tasks.your_vote_marker');

  return `
    <div class="test-panel">
      <div class="test-panel-title">🧪 ${i18n.t('tasks.test_results')}</div>
      <div style="font-size:0.68rem;color:var(--text-muted);margin-bottom:var(--space-sm)">${i18n.t('tasks.votes_local_note')}</div>
      ${totalPass || totalFail
        ? `<div class="test-summary">
             <span class="test-summary-pass">✅ ${i18n.t('tasks.pass_count', { n: totalPass })}</span>
             <span class="test-summary-fail">❌ ${i18n.t('tasks.fail_count', { n: totalFail })}</span>
           </div>`
        : ''}
      <div class="test-owner-label">${i18n.t('tasks.your_vote')}</div>
      <div class="test-vote-row">
        <button class="btn-test-vote${ownerPass?' vote-pass':''}" onclick="castOwnerVote(${task.id},'pass')" data-i18n-tip="tasks.owner_pass_hint">
          ✅ ${i18n.t('tasks.vote_pass')}${ownerPass?yourVote:''}
        </button>
        <button class="btn-test-vote${ownerFail?' vote-fail':''}" onclick="castOwnerVote(${task.id},'fail')" data-i18n-tip="tasks.owner_fail_hint">
          ❌ ${i18n.t('tasks.vote_fail')}${ownerFail?yourVote:''}
        </button>
      </div>
      <div class="test-community">
        <div class="test-community-header">
          <span style="font-size:0.68rem;color:#666;font-weight:700;text-transform:uppercase;letter-spacing:.05em">${i18n.t('tasks.tester_votes')}</span>
          <span>${i18n.t('tasks.pass_count', { n: cPass })} · ${i18n.t('tasks.fail_count', { n: cFail })}</span>
        </div>
        ${communityRows}
        <div style="margin-top:var(--space-md);display:flex;gap:var(--space-md)">
          <button class="btn-test-vote${community.find(v=>v.device==='_local')?.result==='pass'?' vote-pass':''}"
                  style="font-size:.72rem;padding:var(--space-sm) var(--space-md)"
                  onclick="castCommunityVote(${task.id},'pass')" data-i18n-tip="tasks.tester_works_hint">✅ ${i18n.t('tasks.vote_works')}</button>
          <button class="btn-test-vote${community.find(v=>v.device==='_local')?.result==='fail'?' vote-fail':''}"
                  style="font-size:.72rem;padding:var(--space-sm) var(--space-md)"
                  onclick="castCommunityVote(${task.id},'fail')" data-i18n-tip="tasks.tester_broken_hint">❌ ${i18n.t('tasks.vote_broken')}</button>
        </div>
      </div>
    </div>`;
}

/**
 * Move a task to a new status via relay WebSocket (task_update message).
 * Tells the person why if the page cannot sign in, instead of showing a
 * change the server never received.
 */
async function changeTaskStatus(taskId, newStatus) {
  if (!await whenTaskWsBound()) {
    alert(taskSignInHelp());
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
async function changeTaskPriority(taskId, newPriority) {
  if (!await whenTaskWsBound()) {
    alert(taskSignInHelp());
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
async function changeTaskAssignee(taskId) {
  const input = document.getElementById('detail-assignee-input');
  if (!input) return;
  const assignee = input.value.trim();
  if (!await whenTaskWsBound()) {
    alert(taskSignInHelp());
    return;
  }
  taskWs.send(JSON.stringify({ type: 'task_update', task_id: taskId, assignee: assignee || '' }));
  const t = allTasks.find(t => t.id === taskId);
  if (t) { t.assignee = assignee || null; renderBoard(); }
}

/* ═══════════════════════════════════════════════════════════════
   Projects, project selector, CRUD, filtering
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

/** Also request project_list via WS once the socket is signed in */
function requestProjectList() {
  if (taskWs && taskWsBound && taskWs.readyState === WebSocket.OPEN) {
    taskWs.send(JSON.stringify({ type: 'project_list' }));
  }
}

/** Build the project select in the task create modal */
function buildProjectSelect() {
  const sel = document.getElementById('f-project');
  if (!sel) return;
  let html = `<option value="default">${i18n.t('tasks.project_general_default')}</option>`;
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
    setWords(labelEl, 'tasks.all_projects', i18n.t('tasks.all_projects'));
    if (countEl) countEl.textContent = i18n.tn('tasks.task_count', allTasks.length);
  } else if (activeProject === 'default') {
    iconEl.textContent = '📋';
    dotEl.style.background = '#888';
    setWords(labelEl, 'tasks.project_general', i18n.t('tasks.project_general'));
    const cnt = allTasks.filter(t => !t.project || t.project === 'default').length;
    if (countEl) countEl.textContent = i18n.tn('tasks.task_count', cnt);
  } else {
    const proj = projects.find(p => p.id === activeProject);
    if (proj) {
      iconEl.textContent = proj.icon || '📋';
      dotEl.style.background = proj.color || '#4488ff';
      // A project's own name is the person's words, not the page's: no key on it.
      labelEl.removeAttribute('data-i18n');
      labelEl.textContent = proj.name;
      const cnt = allTasks.filter(t => t.project === proj.id).length;
      if (countEl) countEl.textContent = i18n.tn('tasks.task_count', cnt);
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
    onclick="setActiveProject(null)" data-i18n-tip="tasks.all_projects_hint">
    <span class="proj-icon">📋</span>
    <span>${i18n.t('tasks.all_projects')}</span>
  </button>`;

  html += '<div class="project-dropdown-sep"></div>';

  // Default / General project
  const defaultCount = allTasks.filter(t => !t.project || t.project === 'default').length;
  html += `<button class="project-dropdown-item${activeProject === 'default' ? ' active' : ''}"
    onclick="setActiveProject('default')" data-i18n-tip="tasks.project_general_hint">
    <span class="proj-icon">📋</span>
    <span class="proj-dot" style="background:#888"></span>
    <span>${i18n.t('tasks.project_general')}</span>
    <span class="proj-vis">${defaultCount}</span>
  </button>`;

  // User projects
  projects.forEach(p => {
    const icon = p.icon || '📋';
    const color = p.color || '#4488ff';
    const vis = p.visibility === 'private' ? '🔒' : p.visibility === 'members-only' ? '👥' : '';
    const cnt = p.task_count != null ? p.task_count : allTasks.filter(t => t.project === p.id).length;
    html += `<button class="project-dropdown-item${activeProject === p.id ? ' active' : ''}"
      onclick="setActiveProject('${esc(p.id)}')" oncontextmenu="event.preventDefault();openProjectSettings('${esc(p.id)}')" data-i18n-tip="tasks.project_item_hint">
      <span class="proj-icon">${icon}</span>
      <span class="proj-dot" style="background:${color}"></span>
      <span>${esc(p.name)}</span>
      <span class="proj-vis">${vis} ${cnt}</span>
    </button>`;
  });

  html += '<div class="project-dropdown-sep"></div>';

  // Create new project
  html += `<button class="project-dropdown-item create-item" onclick="closeProjectDropdown();openProjectModal()" data-i18n-tip="tasks.new_project_button_hint">
    <span class="proj-icon">+</span>
    <span>${i18n.t('tasks.project_create_new')}</span>
  </button>`;

  dd.innerHTML = html;
  i18n.apply(dd);
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
    `<div role="button" tabindex="0" class="color-swatch${i === 0 ? ' selected' : ''}" style="background:${c}"
      data-color="${c}" aria-label="${i18n.t('tasks.color_swatch_label', { n: i + 1 })}" onclick="selectProjectColor(this)" data-i18n-tip="tasks.color_swatch_hint"></div>`
  ).join('') + `<input type="color" value="#4488ff" id="pf-color-custom"
    style="width:28px;height:28px;border:none;padding:0;cursor:pointer;background:transparent;border-radius:50%;"
    onchange="selectProjectColorCustom(this.value)" title="${i18n.t('tasks.custom_color')}" data-i18n-tip="tasks.custom_color_hint">`;
  i18n.apply(container);
}

/** Build icon picker in the project modal */
function buildProjectIconPicker() {
  const container = document.getElementById('pf-icons');
  if (!container) return;
  container.innerHTML = PROJECT_ICONS.map((ic, i) =>
    `<div role="button" tabindex="0" class="icon-option${i === 0 ? ' selected' : '' /* i18n-exempt: a class name */}" data-icon="${ic}" onclick="selectProjectIcon(this)" data-i18n-tip="tasks.icon_option_hint">${ic}</div>`
  ).join('');
  i18n.apply(container);
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
    setWords(title, 'tasks.project_edit', i18n.t('tasks.project_edit'));
    setWords(btn, 'tasks.project_save_changes', i18n.t('tasks.project_save_changes'));
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
    setWords(title, 'tasks.project_new', i18n.t('tasks.project_new'));
    setWords(btn, 'tasks.project_create', i18n.t('tasks.project_create'));
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

  if (!name) { msg.innerHTML = `<div class="msg-error">${i18n.t('tasks.name_required')}</div>`; return; }

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
      msg.innerHTML = `<div class="msg-success">${i18n.t('tasks.project_updated')}</div>`;
    } else {
      // Create via WS if signed in, else REST
      if (taskWs && taskWsBound && taskWs.readyState === WebSocket.OPEN) {
        taskWs.send(JSON.stringify({ type: 'project_create', ...body }));
        msg.innerHTML = `<div class="msg-success">${i18n.t('tasks.project_created')}</div>`;
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
        msg.innerHTML = `<div class="msg-success">${i18n.t('tasks.project_created')}</div>`;
      }
    }
    renderProjectDropdown();
    renderProjectSelectorBtn();
    setTimeout(closeProjectModal, 600);
  } catch (e) {
    msg.innerHTML = `<div class="msg-error">${i18n.t('tasks.error_message', { message: esc(e.message) })}</div>`;
  } finally {
    btn.disabled = false;
  }
}

/** Open project settings (edit), triggered by right-click on dropdown item */
function openProjectSettings(id) {
  closeProjectDropdown();
  openProjectModal(id);
}

/** Delete a project via REST API */
async function deleteProject(id) {
  if (!await holdConfirm(i18n.t('tasks.delete_project_confirm'), { seconds: 3 })) return;
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
    alert(i18n.t('tasks.delete_project_failed', { message: e.message }));
  }
}

/* ── Helpers ── */
function esc(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

function priorityColor(p) {
  return { critical:'#e53935', high:'#f0a500', medium:'#2196f3', low:'#555' }[p] || '#555';
}

/** A priority's name in the person's language ('medium' -> "Medium"); an
 *  unknown value from the server shows as it came. */
function priorityName(p) {
  const names = {
    low: i18n.t('tasks.priority_low'),
    medium: i18n.t('tasks.priority_medium'),
    high: i18n.t('tasks.priority_high'),
    critical: i18n.t('tasks.priority_critical'),
  };
  return names[p] || String(p);
}

/** A column's name in the person's language ('in_progress' -> "In Progress"). */
function statusName(s) {
  const names = {
    backlog: i18n.t('tasks.status_backlog'),
    in_progress: i18n.t('tasks.status_in_progress'),
    testing: i18n.t('tasks.status_testing'),
    done: i18n.t('tasks.status_done'),
  };
  return names[s] || String(s).replace(/_/g, ' ');
}

/** Show a key's words in an element this script relabels (a heading, a
 *  button), and keep the key on it, so a change of language refills it with
 *  the words it shows now rather than the ones it started with. */
function setWords(el, key, words) {
  if (!el) return;
  el.setAttribute('data-i18n', key);
  el.textContent = words;
}

function timeAgo(ms) {
  const s = Math.floor((Date.now() - ms) / 1000);
  if (s < 60) return i18n.t('tasks.time_seconds', { n: s });
  if (s < 3600) return i18n.t('tasks.time_minutes', { n: Math.floor(s/60) });
  if (s < 86400) return i18n.t('tasks.time_hours', { n: Math.floor(s/3600) });
  return i18n.t('tasks.time_days', { n: Math.floor(s/86400) });
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

/* ── Boot: hydrate the data-driven registries, then init. Both files are
 * same-origin static JSON; on failure the embedded fallbacks keep the page
 * functional offline. ── */
Promise.allSettled([
  fetch('/data/tasks/scopes.json').then(r => r.ok ? r.json() : null).then(d => {
    if (d && Array.isArray(d.scopes) && d.scopes.length) SCOPES = d.scopes;
  }),
  fetch('/data/skills/xp_curve.json').then(r => r.ok ? r.json() : null).then(d => {
    if (d && Array.isArray(d.xp_per_level) && d.xp_per_level.length) {
      XP_PER_LEVEL = d.xp_per_level;
      MAX_LEVEL = d.max_level || XP_PER_LEVEL.length;
    }
  }),
]).then(init);

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
   Quests, local quest tracker (localStorage hos_quests_v1)
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
  /* "Tasks" is this page's name (nav + title); "Quests" is a DIFFERENT nav
   * entry (/onboarding), so quest-vocabulary here collided with it. */
  const heading = document.getElementById('quest-heading');
  if (f === 'all') setWords(heading, 'tasks.quest_cat_all', i18n.t('tasks.quest_cat_all'));
  else if (f === 'daily') setWords(heading, 'tasks.quest_heading_daily', i18n.t('tasks.quest_heading_daily'));
  else if (f === 'story') setWords(heading, 'tasks.quest_heading_story', i18n.t('tasks.quest_heading_story'));
  else if (f === 'side') setWords(heading, 'tasks.quest_heading_side', i18n.t('tasks.quest_heading_side'));
  else if (f === 'personal') setWords(heading, 'tasks.quest_heading_personal', i18n.t('tasks.quest_heading_personal'));
  else if (f === 'done') setWords(heading, 'tasks.quest_cat_done', i18n.t('tasks.quest_cat_done'));
  else setWords(heading, 'tasks.title', i18n.t('tasks.title'));
  quest_renderList();
}

/** A quest type's name in the person's language ('daily' -> "Daily"). */
function quest_typeName(type) {
  const names = {
    daily: i18n.t('tasks.quest_cat_daily'),
    story: i18n.t('tasks.quest_cat_story'),
    side: i18n.t('tasks.quest_cat_side'),
    personal: i18n.t('tasks.quest_cat_personal'),
  };
  return names[type] || String(type);
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
      <p>${search ? i18n.t('tasks.quest_none_match') : quest_currentFilter === 'done' ? i18n.t('tasks.quest_none_done') : i18n.t('tasks.quest_none')}</p>
      ${!search && quest_currentFilter !== 'done' ? `<button class="quest-btn-save" onclick="quest_openModal()" style="margin-top:var(--space-md);" data-i18n-tip="tasks.quest_add_hint">${i18n.t('tasks.quest_add_first')}</button>` : ''}</div>`;
    i18n.apply(list);
    return;
  }

  const today = new Date().toISOString().slice(0, 10);
  list.innerHTML = visible.map(q => {
    const overdue = q.due && q.due < today && !q.done;
    const badgeClass = 'badge-' + q.type;
    return `<div role="button" tabindex="0" class="quest-card${q.done ? ' done' : ''}" onclick="quest_openModal('${q.id}')" data-i18n-tip="tasks.quest_card_hint">
      <div role="button" tabindex="0" class="quest-check" onclick="event.stopPropagation();quest_toggleDone('${q.id}')" data-i18n-tip="tasks.quest_check_hint">${q.done ? '✓' : ''}</div>
      <div class="quest-body">
        <div class="quest-title">${quest_esc(q.title)}</div>
        ${q.desc ? `<div class="quest-desc">${quest_esc(q.desc)}</div>` : ''}
        <div class="quest-meta">
          <span class="quest-xp">⭐ ${i18n.t('tasks.quest_xp_amount', { n: q.xp || 0 })}</span>
          <span class="quest-type-badge ${badgeClass}">${quest_typeName(q.type)}</span>
          ${q.due ? `<span class="quest-due${overdue ? ' overdue' : '' /* i18n-exempt: a class name */}">${overdue ? '⚠ ' : ''}${q.due}</span>` : ''}
        </div>
      </div>
      <button class="quest-delete" onclick="event.stopPropagation();quest_deleteQuest('${q.id}')" title="${i18n.t('common.delete')}" data-i18n-tip="tasks.quest_delete_hint">✕</button>
    </div>`;
  }).join('');
  i18n.apply(list);
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
      ? '🎉 ' + i18n.t('tasks.quest_xp_level', { xp: amount, skill: skillId, level: s.level })
      : '✅ ' + i18n.t('tasks.quest_xp_awarded', { xp: amount, skill: skillId });
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
  const modalTitle = document.getElementById('quest-modal-title');
  if (q) setWords(modalTitle, 'tasks.quest_edit', i18n.t('tasks.quest_edit'));
  else setWords(modalTitle, 'tasks.quest_new', i18n.t('tasks.quest_new'));
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
    el.style.cssText = 'position:fixed;bottom:3rem;left:50%;transform:translateX(-50%);background:var(--tk-toast-bg);border:1px solid #f0a500;color:var(--tk-toast-ink);padding:var(--space-lg) 1.2rem;border-radius:8px;font-size:0.82rem;z-index:9999;max-width:340px;text-align:center;transition:opacity .3s;';
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

// A change of language (Settings) re-fills the static words itself; the words
// this script built are drawn again here, in the new language.
if (typeof window.addEventListener === 'function') {
  window.addEventListener('languagechange', () => {
    renderBoard();
    renderControls();
    renderProjectSelectorBtn();
    renderProjectDropdown();
    quest_setFilter(quest_currentFilter);
    if (openDetailId !== null && document.getElementById('detail-overlay').classList.contains('open')) openDetail(openDetailId);
  });
}
document.querySelectorAll('.pg-ico[data-icon]').forEach(function(el) {
  if (window.hosIcon) el.innerHTML = hosIcon(el.dataset.icon, parseInt(el.dataset.size) || 20);
});
