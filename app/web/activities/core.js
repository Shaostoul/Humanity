 // ── Hub Tab System (SPA with pushState) ──
 const HUB_TABS = ['map', 'board', 'quests', 'calendar', 'logbook', 'inventory', 'reality', 'fantasy', 'market', 'learn', 'browse', 'dashboard', 'streams', 'info', 'source', 'debug'];
 let currentTab = 'dashboard';

 function getTabFromPath() {
  const path = window.location.pathname.replace(/^\//, '').replace(/\/$/, '').toLowerCase();
  if (HUB_TABS.includes(path)) return path;
  return null;
 }

 function switchTab(tabId, pushState) {
  if (!HUB_TABS.includes(tabId)) tabId = 'dashboard';
  currentTab = tabId;

  // Update nav active state
  document.querySelectorAll('.hub-nav .tab').forEach(el => {
   el.classList.toggle('active', el.dataset.tab === tabId);
  });

  // Show correct content panel
  document.querySelectorAll('.tab-content').forEach(el => el.classList.remove('active'));
  const tabEl = document.getElementById('tab-' + tabId);
  if (tabEl) tabEl.classList.add('active');
  if (tabId === 'map' && typeof mapRender === 'function') {
   requestAnimationFrame(() => mapRender());
  }

  // Update page title
  const titles = { map: 'Maps', board: 'Systems', quests: 'Quests', calendar: 'Calendar', logbook: 'Logbook', inventory: 'Inventory', reality: 'Profile', fantasy: 'Skills', market: 'Market', learn: 'Learn', browse: 'Browse', dashboard: 'Dashboard', streams: 'Streams', info: 'Knowledge', source: 'Equipment', debug: 'Ops' };
  document.title = 'Humanity - ' + (titles[tabId] || 'Hub');

  // Update URL
  if (pushState !== false) {
   const newPath = '/' + tabId;
   if (window.location.pathname !== newPath) {
    history.pushState({ tab: tabId }, '', newPath);
   }
  }

  sanitizeUiMojibake();
 }

 function sanitizeUiMojibake() {
  // Normalize card headers that picked up mojibake bytes.
  document.querySelectorAll('.reality-card-header').forEach(h => {
   const icon = h.querySelector('.collapse-icon');
   let label = '';
   h.childNodes.forEach(n => {
    if (n.nodeType === Node.TEXT_NODE) label += n.nodeValue || '';
   });
   label = label.replace(/[^\x20-\x7E]+/g, ' ').replace(/\s+/g, ' ').trim();
   if (label) {
    if (h.firstChild && h.firstChild.nodeType === Node.TEXT_NODE) {
     h.firstChild.nodeValue = label + ' ';
    } else {
     h.prepend(document.createTextNode(label + ' '));
    }
   }
   if (icon) icon.textContent = '';
  });

  // Global visible-text cleanup fallback for mojibake-heavy labels.
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const badMarker = /(?:d\x59[^\s]{0,8}|\u00C2|\u00E2|\u00F0|\uFFFD)/;
  let node;
  while ((node = walker.nextNode())) {
   const p = node.parentElement;
   if (!p) continue;
   const tag = p.tagName;
   if (tag === 'SCRIPT' || tag === 'STYLE' || tag === 'TEXTAREA') continue;
   const t = node.nodeValue || '';
   if (!badMarker.test(t)) continue;

   let clean = t;
   clean = clean.replace(/d\x59[^\s]{0,8}/g, '');
   clean = clean.replace(/Â/g, '');
   clean = clean.replace(/â–¼/g, '▼').replace(/â–²/g, '▲');
   clean = clean.replace(/⏱/g, 'Duration').replace(/👁/g, 'Viewers').replace(/📊/g, 'Quality').replace(/📶/g, 'Bitrate');
   clean = clean.replace(/📺/g, 'LIVE').replace(/🎬/g, 'PREVIEW');
   clean = clean.replace(/\uFFFD/g, '');
   clean = clean.replace(/\s+/g, ' ');
   if (clean !== t) node.nodeValue = clean;
  }

  const statusDot = document.getElementById('server-status-dot');
  if (statusDot) statusDot.textContent = '●';

  const v = document.getElementById('viewer-stat-viewers');
  if (v) v.textContent = (v.textContent || '').replace(/^[^A-Za-z0-9]+\s*/, 'Viewers: ');
  const d = document.getElementById('viewer-stat-duration');
  if (d) d.textContent = (d.textContent || '').replace(/^[^A-Za-z0-9]+\s*/, 'Duration: ');
 }

 // ── Page Models: Quests / Calendar / Logbook ──
 function loadJson(key, fallback) {
  try { return JSON.parse(localStorage.getItem(key) || 'null') || fallback; } catch { return fallback; }
 }
 function saveJson(key, value) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch {}
 }

 function questsRender() {
  const listEl = document.getElementById('quests-list');
  if (!listEl) return;
  const items = loadJson('humanity_quests_v1', []);
  const tracks = ['daily','weekly','milestone','legacy'];
  tracks.forEach(t => {
   const c = items.filter(q => q.track === t && !q.done).length;
   const el = document.getElementById('q-count-' + t);
   if (el) el.textContent = c + ' active';
  });
  if (!items.length) {
   listEl.innerHTML = '<div style="color:var(--text-muted);font-size:0.8rem;">No quests yet. Add one above.</div>';
   return;
  }
  listEl.innerHTML = items.map((q, i) => '<div style="border:1px solid var(--border);border-radius:8px;padding:0.5rem;background:' + (q.done ? 'rgba(122,214,97,0.08)' : 'rgba(255,255,255,0.02)') + ';"><div style="display:flex;justify-content:space-between;gap:0.6rem;"><strong>' + escHtml(q.title) + '</strong><span style="font-size:0.7rem;color:var(--text-muted);text-transform:uppercase;">' + escHtml(q.track) + '</span></div><div style="font-size:0.74rem;color:var(--text-muted);margin-top:0.2rem;">' + new Date(q.ts || Date.now()).toLocaleString() + '</div><div style="margin-top:0.35rem;display:flex;gap:0.35rem;"><button onclick="questsToggle(' + i + ')" style="background:none;border:1px solid var(--border);color:var(--text);padding:0.2rem 0.45rem;border-radius:6px;cursor:pointer;font-size:0.72rem;">' + (q.done ? 'Reopen' : 'Complete') + '</button><button onclick="questsDelete(' + i + ')" style="background:none;border:1px solid rgba(229,85,85,0.45);color:#e88;padding:0.2rem 0.45rem;border-radius:6px;cursor:pointer;font-size:0.72rem;">Delete</button></div></div>').join('');
  }
 function questsAdd() {
  const titleEl = document.getElementById('quests-new-title');
  const trackEl = document.getElementById('quests-new-track');
  if (!titleEl || !titleEl.value.trim()) return;
  const items = loadJson('humanity_quests_v1', []);
  items.unshift({ title: titleEl.value.trim(), track: (trackEl && trackEl.value) || 'daily', done: false, ts: Date.now() });
  saveJson('humanity_quests_v1', items);
  titleEl.value = '';
  questsRender();
 }
 function questsToggle(i) {
  const items = loadJson('humanity_quests_v1', []);
  if (!items[i]) return;
  items[i].done = !items[i].done;
  saveJson('humanity_quests_v1', items);
  questsRender();
 }
 function questsDelete(i) {
  const items = loadJson('humanity_quests_v1', []);
  if (!items[i]) return;
  items.splice(i, 1);
  saveJson('humanity_quests_v1', items);
  questsRender();
 }

 function calendarRender() {
  const listEl = document.getElementById('calendar-list');
  if (!listEl) return;
  const items = loadJson('humanity_calendar_v1', []);
  if (!items.length) {
   listEl.innerHTML = '<div style="color:var(--text-muted);font-size:0.8rem;">No events scheduled.</div>';
   return;
  }
  listEl.innerHTML = items.map((e, i) => '<div style="border:1px solid var(--border);border-radius:8px;padding:0.5rem;"><div style="display:flex;justify-content:space-between;gap:0.6rem;"><strong>' + escHtml(e.title) + '</strong><span style="font-size:0.73rem;color:var(--text-muted);">' + escHtml(e.date || '--') + '</span></div><div style="margin-top:0.35rem;"><button onclick="calendarDelete(' + i + ')" style="background:none;border:1px solid rgba(229,85,85,0.45);color:#e88;padding:0.2rem 0.45rem;border-radius:6px;cursor:pointer;font-size:0.72rem;">Delete</button></div></div>').join('');
 }
 function calendarAdd() {
  const titleEl = document.getElementById('cal-title');
  const dateEl = document.getElementById('cal-date');
  if (!titleEl || !titleEl.value.trim()) return;
  const items = loadJson('humanity_calendar_v1', []);
  items.unshift({ title: titleEl.value.trim(), date: (dateEl && dateEl.value) || '' });
  saveJson('humanity_calendar_v1', items);
  titleEl.value = '';
  if (dateEl) dateEl.value = '';
  calendarRender();
 }
 function calendarDelete(i) {
  const items = loadJson('humanity_calendar_v1', []);
  if (!items[i]) return;
  items.splice(i, 1);
  saveJson('humanity_calendar_v1', items);
  calendarRender();
 }

 function logbookRender() {
  const listEl = document.getElementById('logbook-list');
  if (!listEl) return;
  const items = loadJson('humanity_logbook_v1', []);
  if (!items.length) {
   listEl.innerHTML = '<div style="color:var(--text-muted);font-size:0.8rem;">No log entries yet.</div>';
   return;
  }
  listEl.innerHTML = items.map((it, i) => '<div style="border:1px solid var(--border);border-radius:8px;padding:0.5rem;"><div style="display:flex;justify-content:space-between;gap:0.6rem;"><strong>' + escHtml(it.title) + '</strong><span style="font-size:0.7rem;color:var(--text-muted);text-transform:uppercase;">' + escHtml(it.type) + '</span></div><div style="font-size:0.74rem;color:var(--text-muted);margin-top:0.2rem;white-space:pre-wrap;">' + escHtml(it.body || '') + '</div><div style="margin-top:0.35rem;"><button onclick="logbookDelete(' + i + ')" style="background:none;border:1px solid rgba(229,85,85,0.45);color:#e88;padding:0.2rem 0.45rem;border-radius:6px;cursor:pointer;font-size:0.72rem;">Delete</button></div></div>').join('');
 }
 function logbookAdd() {
  const typeEl = document.getElementById('logbook-type');
  const titleEl = document.getElementById('logbook-title');
  const bodyEl = document.getElementById('logbook-body');
  if (!titleEl || !titleEl.value.trim()) return;
  const items = loadJson('humanity_logbook_v1', []);
  items.unshift({ type: (typeEl && typeEl.value) || 'journal', title: titleEl.value.trim(), body: (bodyEl && bodyEl.value) || '', ts: Date.now() });
  saveJson('humanity_logbook_v1', items);
  titleEl.value = '';
  if (bodyEl) bodyEl.value = '';
  logbookRender();
 }
 function logbookDelete(i) {
  const items = loadJson('humanity_logbook_v1', []);
  if (!items[i]) return;
  items.splice(i, 1);
  saveJson('humanity_logbook_v1', items);
  logbookRender();
 }

 function initPageModels() {
  questsRender();
  calendarRender();
  logbookRender();
 }

 // ── V1 Page Buildout: Dashboard / Profile / Inventory ──
 function ensureTab(id) {
  let el = document.getElementById('tab-' + id);
  if (el) return el;
  const tabs = document.getElementById('tabs-container') || document.querySelector('.tabs-container') || document.body;
  el = document.createElement('div');
  el.id = 'tab-' + id;
  el.className = 'tab-content';
  el.style.padding = '1rem';
  el.style.overflowY = 'auto';
  tabs.appendChild(el);
  return el;
 }

 function renderDashboardPage() {
  const el = ensureTab('dashboard');
  if (!el) return;
  const quests = loadJson('humanity_quests_v1', []);
  const logs = loadJson('humanity_logbook_v1', []);
  const pending = quests.filter(q => !q.done);
  const byTrack = {
   daily: pending.filter(q => q.track === 'daily').length,
   weekly: pending.filter(q => q.track === 'weekly').length,
   milestone: pending.filter(q => q.track === 'milestone').length,
   legacy: pending.filter(q => q.track === 'legacy').length,
  };
  const stats = loadJson('humanity_profile_stats_v1', { level: 1, health: 100, stamina: 100, rep: 0 });
  el.innerHTML = '' +
   '<h2 style="margin:0 0 0.65rem;color:var(--accent);font-size:1.1rem;">🏠 H Dashboard</h2>' +
   '<div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:0.55rem;margin-bottom:0.7rem;">' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.55rem;"><div style="font-size:0.72rem;color:var(--text-muted);">Character</div><div style="font-weight:700;">Lvl ' + (stats.level||1) + '</div><div style="font-size:0.75rem;color:var(--text-muted);">Health ' + (stats.health||100) + '% · Stamina ' + (stats.stamina||100) + '%</div></div>' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.55rem;"><div style="font-size:0.72rem;color:var(--text-muted);">Skills</div><div style="font-weight:700;">Active Tracks</div><div style="font-size:0.75rem;color:var(--text-muted);">Welding, Farming, Logistics</div></div>' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.55rem;"><div style="font-size:0.72rem;color:var(--text-muted);">Character Inventory</div><div style="font-weight:700;">Quick Slots Ready</div><div style="font-size:0.75rem;color:var(--text-muted);">Tools, Medpack, Multi-kit</div></div>' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.55rem;"><div style="font-size:0.72rem;color:var(--text-muted);">Quests</div><div style="font-weight:700;">' + pending.length + ' active</div><div style="font-size:0.75rem;color:var(--text-muted);">D ' + byTrack.daily + ' · W ' + byTrack.weekly + ' · M ' + byTrack.milestone + ' · L ' + byTrack.legacy + '</div></div>' +
   '</div>' +
   '<div style="display:grid;grid-template-columns:1.1fr 1fr;gap:0.7rem;">' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.6rem;">' +
       '<div style="font-weight:700;margin-bottom:0.4rem;">Resume / Next Actions</div>' +
       '<div style="display:flex;gap:0.4rem;flex-wrap:wrap;">' +
         '<button onclick="switchTab(\'quests\')" style="background:none;border:1px solid var(--border);color:var(--text);padding:0.26rem 0.55rem;border-radius:6px;cursor:pointer;">Open Quests</button>' +
         '<button onclick="switchTab(\'inventory\')" style="background:none;border:1px solid var(--border);color:var(--text);padding:0.26rem 0.55rem;border-radius:6px;cursor:pointer;">Open Inventory</button>' +
         '<button onclick="switchTab(\'reality\')" style="background:none;border:1px solid var(--border);color:var(--text);padding:0.26rem 0.55rem;border-radius:6px;cursor:pointer;">Open Profile</button>' +
         '<button onclick="switchTab(\'learn\')" style="background:none;border:1px solid var(--border);color:var(--text);padding:0.26rem 0.55rem;border-radius:6px;cursor:pointer;">Learn Next</button>' +
       '</div>' +
     '</div>' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.6rem;">' +
       '<div style="font-weight:700;margin-bottom:0.4rem;">Recent Logbook</div>' +
       '<div style="font-size:0.78rem;color:var(--text-muted);white-space:pre-wrap;">' + escHtml((logs[0] ? (logs[0].title + ' — ' + (logs[0].body||'').slice(0,120)) : 'No entries yet.')) + '</div>' +
     '</div>' +
   '</div>';
 }

 function renderProfilePage() {
  const el = ensureTab('reality');
  if (!el) return;
  const profile = loadJson('humanity_profile_v1', { name: 'Crew Member', pronouns: '', bio: '', timezone: '', callSign: '' });
  const stats = loadJson('humanity_profile_stats_v1', { level: 1, health: 100, stamina: 100, rep: 0 });
  el.innerHTML = '' +
   '<h2 style="margin:0 0 0.65rem;color:var(--accent);font-size:1.05rem;">🟢 Profile</h2>' +
   '<div style="display:grid;grid-template-columns:1fr 1fr;gap:0.7rem;">' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.6rem;">' +
       '<div style="font-weight:700;margin-bottom:0.45rem;">Identity</div>' +
       '<input id="profile-name" value="' + escHtml(profile.name||'') + '" placeholder="Display name" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;margin-bottom:0.35rem;">' +
       '<input id="profile-callsign" value="' + escHtml(profile.callSign||'') + '" placeholder="Call sign" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;margin-bottom:0.35rem;">' +
       '<input id="profile-timezone" value="' + escHtml(profile.timezone||'') + '" placeholder="Timezone" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;margin-bottom:0.35rem;">' +
       '<textarea id="profile-bio" rows="4" placeholder="Bio" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;resize:vertical;">' + escHtml(profile.bio||'') + '</textarea>' +
       '<div style="margin-top:0.45rem;"><button onclick="profileSave()" class="btn btn-clickable" style="min-width:auto;min-height:34px;padding:0.3rem 0.75rem;">Save Profile</button></div>' +
     '</div>' +
     '<div style="border:1px solid var(--border);border-radius:8px;padding:0.6rem;">' +
       '<div style="font-weight:700;margin-bottom:0.45rem;">Stats</div>' +
       '<div style="font-size:0.8rem;color:var(--text-muted);">Level: <strong style="color:var(--text);">' + (stats.level||1) + '</strong></div>' +
       '<div style="font-size:0.8rem;color:var(--text-muted);">Health: <strong style="color:var(--text);">' + (stats.health||100) + '%</strong></div>' +
       '<div style="font-size:0.8rem;color:var(--text-muted);">Stamina: <strong style="color:var(--text);">' + (stats.stamina||100) + '%</strong></div>' +
       '<div style="font-size:0.8rem;color:var(--text-muted);">Reputation: <strong style="color:var(--text);">' + (stats.rep||0) + '</strong></div>' +
       '<hr style="border:none;border-top:1px solid var(--border);margin:0.55rem 0;">' +
       '<div style="font-size:0.76rem;color:var(--text-muted);">Privacy defaults: local-first, analytics off, cloud backup opt-in.</div>' +
     '</div>' +
   '</div>';
 }

 function profileSave() {
  const p = {
   name: (document.getElementById('profile-name')?.value || '').trim() || 'Crew Member',
   callSign: (document.getElementById('profile-callsign')?.value || '').trim(),
   timezone: (document.getElementById('profile-timezone')?.value || '').trim(),
   bio: (document.getElementById('profile-bio')?.value || '').trim(),
  };
  saveJson('humanity_profile_v1', p);
  setStatus('Profile saved');
  renderDashboardPage();
 }

 function renderInventoryPage() {
  const el = ensureTab('inventory');
  if (!el) return;
  const inv = loadJson('humanity_inventory_v1', [
   { name: 'Multi-tool', qty: 1, cat: 'tool' },
   { name: 'Water Canister', qty: 2, cat: 'consumable' },
   { name: 'Seed Pack', qty: 6, cat: 'resource' }
  ]);
  const rows = inv.map((it, i) => '<tr><td style="padding:0.35rem 0.4rem;border-bottom:1px solid var(--border);">' + escHtml(it.name) + '</td><td style="padding:0.35rem 0.4rem;border-bottom:1px solid var(--border);">' + escHtml(it.cat||'misc') + '</td><td style="padding:0.35rem 0.4rem;border-bottom:1px solid var(--border);text-align:right;">' + Number(it.qty||0) + '</td><td style="padding:0.35rem 0.4rem;border-bottom:1px solid var(--border);text-align:right;"><button onclick="inventoryDel(' + i + ')" style="background:none;border:1px solid rgba(229,85,85,0.45);color:#e88;padding:0.15rem 0.35rem;border-radius:6px;cursor:pointer;">Del</button></td></tr>').join('');
  el.innerHTML = '' +
   '<h2 style="margin:0 0 0.65rem;color:var(--accent);font-size:1.05rem;">🎒 Inventory</h2>' +
   '<div style="display:flex;gap:0.4rem;flex-wrap:wrap;margin-bottom:0.55rem;">' +
     '<input id="inv-name" placeholder="Item name" style="flex:1;min-width:180px;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;">' +
     '<input id="inv-cat" placeholder="Category" style="width:140px;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;">' +
     '<input id="inv-qty" type="number" min="1" value="1" style="width:90px;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem 0.5rem;border-radius:6px;">' +
     '<button onclick="inventoryAdd()" class="btn btn-clickable" style="min-width:auto;min-height:34px;padding:0.3rem 0.75rem;">Add Item</button>' +
   '</div>' +
   '<table style="width:100%;border-collapse:collapse;background:rgba(255,255,255,0.01);border:1px solid var(--border);border-radius:8px;overflow:hidden;">' +
     '<thead><tr><th style="text-align:left;padding:0.38rem 0.4rem;border-bottom:1px solid var(--border);font-size:0.74rem;color:var(--text-muted);">Item</th><th style="text-align:left;padding:0.38rem 0.4rem;border-bottom:1px solid var(--border);font-size:0.74rem;color:var(--text-muted);">Category</th><th style="text-align:right;padding:0.38rem 0.4rem;border-bottom:1px solid var(--border);font-size:0.74rem;color:var(--text-muted);">Qty</th><th style="width:56px;border-bottom:1px solid var(--border);"></th></tr></thead>' +
     '<tbody>' + rows + '</tbody>' +
   '</table>';
 }

 function inventoryAdd() {
  const n = (document.getElementById('inv-name')?.value || '').trim();
  if (!n) return;
  const c = (document.getElementById('inv-cat')?.value || '').trim() || 'misc';
  const q = Math.max(1, Number(document.getElementById('inv-qty')?.value || 1));
  const inv = loadJson('humanity_inventory_v1', []);
  inv.unshift({ name: n, cat: c, qty: q });
  saveJson('humanity_inventory_v1', inv);
  renderInventoryPage();
  renderDashboardPage();
 }

 function inventoryDel(i) {
  const inv = loadJson('humanity_inventory_v1', []);
  if (!inv[i]) return;
  inv.splice(i, 1);
  saveJson('humanity_inventory_v1', inv);
  renderInventoryPage();
  renderDashboardPage();
 }

 function initV1CorePages() {
  renderProfilePage();
  renderInventoryPage();
  renderDashboardPage();
 }

 // SPA navigation: intercept hub tab clicks
 document.querySelectorAll('.hub-nav .tab[data-tab]').forEach(link => {
  link.addEventListener('click', (e) => {
   e.preventDefault();
   switchTab(link.dataset.tab);
  });
 });

 // Handle browser back/forward
 window.addEventListener('popstate', (e) => {
  const tab = (e.state && e.state.tab) || getTabFromPath();
  if (tab) switchTab(tab, false);
 });

 // Initialize from URL
 const initialTab = getTabFromPath() || 'dashboard';
 switchTab(initialTab, false);
 initPageModels();
 initV1CorePages();

 // ── Debug Info ──
 (function populateDebugInfo() {
  const el = (id) => document.getElementById(id);
  el('debug-version').textContent = '0.1.0-dev';
  el('debug-server').textContent = window.location.origin;
  el('debug-ua').textContent = navigator.userAgent.slice(0, 100);
  el('debug-screen').textContent = screen.width + '×' + screen.height + ' · ' + (window.devicePixelRatio || 1) + 'x DPR';
  el('debug-loaded').textContent = new Date().toISOString();

  // Check relay connection
  async function checkConnection() {
   try {
    const res = await fetch('/api/stats');
    if (res.ok) {
     const data = await res.json();
     el('debug-connection').textContent = '✅ Connected — ' + (data.online_users || 0) + ' users online';
     el('debug-connection').style.color = '#4a8';
     if (data.version) el('debug-version').textContent = data.version;
    } else {
     el('debug-connection').textContent = '⚠️ Server responded ' + res.status;
     el('debug-connection').style.color = '#eb4';
    }
   } catch (e) {
    el('debug-connection').textContent = '❌ Cannot reach server';
    el('debug-connection').style.color = '#e55';
   }
  }
  checkConnection();
 })();

 // ── Global: Click activates red border for 1s ──
 document.addEventListener('click', function(e) {
  const btn = e.target.closest('.btn');
  if (!btn || btn.classList.contains('btn-disabled') || btn.classList.contains('btn-cooldown')) return;
  const wasChanneling = btn.classList.contains('btn-channeling');
  btn.classList.add('btn-activated');
  if (wasChanneling) btn.style.animation = 'none';
  setTimeout(() => {
   btn.classList.remove('btn-activated');
   if (wasChanneling) btn.style.animation = '';
  }, 1000);
 });

 // ── Button Test Functions ──
 function testBtnClick(btn) {
  btn.textContent = '✓ Clicked!';
  setTimeout(() => { btn.textContent = 'Hover / Click Me'; }, 800);
 }

 const states = ['btn-clickable', 'btn-channeling', 'btn-activated', 'btn-cooldown', 'btn-disabled'];
 function cycleState(btn) {
  const current = states.find(s => btn.classList.contains(s)) || 'btn-clickable';
  const idx = states.indexOf(current);
  const next = states[(idx + 1) % states.length];
  states.forEach(s => btn.classList.remove(s));
  btn.classList.add(next);
  btn.querySelectorAll('.btn-cooldown-timer').forEach(t => t.remove());
  if (next === 'btn-cooldown') {
   btn.innerHTML = btn.textContent.replace(/[\d.]+s/g, '').trim();
   const topT = document.createElement('span');
   topT.className = 'btn-cooldown-timer top';
   topT.textContent = '∞';
   const botT = document.createElement('span');
   botT.className = 'btn-cooldown-timer bottom';
   botT.textContent = '∞';
   btn.appendChild(topT);
   btn.appendChild(botT);
  }
 }

 function startCooldown(btn, seconds) {
  if (btn.classList.contains('btn-cooldown')) return;
  const originalText = btn.textContent;
  btn.classList.remove('btn-clickable');
  btn.classList.add('btn-cooldown');

  const topT = document.createElement('span');
  topT.className = 'btn-cooldown-timer top';
  const botT = document.createElement('span');
  botT.className = 'btn-cooldown-timer bottom';
  btn.appendChild(topT);
  btn.appendChild(botT);

  let remaining = seconds;
  const update = () => {
   topT.textContent = remaining.toFixed(1) + 's';
   botT.textContent = remaining.toFixed(1) + 's';
  };
  update();

  const interval = setInterval(() => {
   remaining -= 0.1;
   if (remaining <= 0) {
    clearInterval(interval);
    btn.classList.remove('btn-cooldown');
    btn.classList.add('btn-clickable');
    topT.remove();
    botT.remove();
    btn.textContent = originalText;
   } else {
    update();
   }
  }, 100);
 }

 // toggleGoLive removed — replaced by streamGoLive/streamStop

 function toggleChannel(btn) {
  if (btn.classList.contains('btn-channeling')) {
   btn.classList.remove('btn-channeling');
   btn.classList.add('btn-clickable');
   btn.textContent = btn.textContent.replace('Stop', 'Channel');
  } else {
   btn.classList.remove('btn-clickable');
   btn.classList.add('btn-channeling');
   btn.textContent = btn.textContent.replace('Channel', 'Stop');
  }
 }
