 // ── Knowledge Docs (info tab) ──
 let KNOWLEDGE_DOCS = [
  'docs/index.md','README.md','AGENTS.md','OPERATING_CONTRACT.md','TOOLS.md',
  'design/README.md','design/feature_web.md','design/ui/app_shell_information_architecture.md','design/runtime/update_distribution_architecture.md',
  'knowledge/index.md','knowledge/OPERATIONS_RUNBOOK.md','accord/humanity_accord.md'
 ];
 let KNOWLEDGE_TEXT = {
  'docs/index.md': 'Top-level docs index for easier navigation.',
  'README.md': 'Project root overview and entrypoint for contributors.',
  'design/feature_web.md': 'Feature Showcase system goals, model, and roadmap.',
  'knowledge/OPERATIONS_RUNBOOK.md': 'Deploy and operations runbook.'
 };
 async function knowledgeLoadManifest() {
  try {
   const res = await fetch('/docs/knowledge_manifest.json', { cache: 'no-store' });
   if (!res.ok) throw new Error('manifest http ' + res.status);
   const data = await res.json();
   if (data && Array.isArray(data.docs) && data.docs.length) {
     KNOWLEDGE_DOCS = data.docs.map(d => d.path).filter(Boolean);
     KNOWLEDGE_TEXT = {};
     data.docs.forEach(d => { if (d.path) KNOWLEDGE_TEXT[d.path] = d.summary || ''; });
   }
  } catch {}
 }
 function knowledgeRenderList() {
  const qEl = document.getElementById('knowledge-search');
  const treeEl = document.getElementById('knowledge-tree');
  if (!treeEl) return;
  const q = ((qEl && qEl.value) || '').trim().toLowerCase();
  const list = KNOWLEDGE_DOCS.filter(p => !q || p.toLowerCase().includes(q));
  const groups = {};
  list.forEach(p => {
   let g = 'root';
   if (p.includes('/')) {
     const parts = p.split('/');
     g = (parts[0] === 'design' && parts.length > 2) ? parts[1] : parts[0];
   }
   if (!groups[g]) groups[g] = [];
   groups[g].push(p);
  });
  const order = ['root','docs','design','core','security','gameplay','product','ui','runtime','rfc','knowledge','accord','website'];
  treeEl.innerHTML = order.filter(g => groups[g] && groups[g].length).map(g => {
   const items = groups[g].map(p => {
     let label = p;
     if (p.startsWith('design/')) {
       const parts = p.split('/');
       label = parts.slice(parts.length > 2 ? 2 : 1).join('/');
     } else if (p.includes('/')) {
       label = p.split('/').slice(1).join('/');
     }
     return '<button onclick="knowledgeOpenDoc(\'' + p + '\')" style="text-align:left;background:none;border:1px solid var(--border);color:var(--text);padding:0.25rem 0.4rem;border-radius:6px;font-size:0.72rem;cursor:pointer;">' + label + '</button>';
   }).join('');
   const label = g === 'root' ? 'root' : g;
   return '<details open><summary style="cursor:pointer;font-size:0.72rem;color:var(--text-muted);">' + label + '</summary><div style="display:flex;flex-direction:column;gap:0.2rem;padding:0.25rem 0 0.25rem 0.45rem;">' + items + '</div></details>';
  }).join('');
 }
 async function knowledgeOpenDoc(path) {
  const titleEl = document.getElementById('knowledge-title');
  const bodyEl = document.getElementById('knowledge-body');
  const metaEl = document.getElementById('knowledge-meta');
  if (titleEl) titleEl.textContent = path;
  if (bodyEl) bodyEl.textContent = 'Loading…';
  const base = 'https://raw.githubusercontent.com/Shaostoul/Humanity/main/';
  try {
   const res = await fetch(base + path, { cache: 'no-store' });
   if (!res.ok) throw new Error('HTTP ' + res.status);
   const text = await res.text();
   if (bodyEl) bodyEl.textContent = text.slice(0, 10000);
   if (metaEl) metaEl.innerHTML = '<div><strong>Path:</strong> ' + path + '</div><div><strong>Approx length:</strong> ' + text.length + ' chars</div><div><strong>Summary:</strong> ' + (KNOWLEDGE_TEXT[path] || 'No summary yet.') + '</div>';
  } catch (e) {
   if (bodyEl) bodyEl.textContent = 'Could not load remote markdown for ' + path + '.\n\n' + (KNOWLEDGE_TEXT[path] || 'No local preview available.');
   if (metaEl) metaEl.innerHTML = '<div><strong>Path:</strong> ' + path + '</div><div><strong>Status:</strong> load failed</div><div><strong>Summary:</strong> ' + (KNOWLEDGE_TEXT[path] || 'No summary yet.') + '</div>';
  }
 }
 knowledgeLoadManifest().then(() => knowledgeRenderList());

 // ── Info Tab: collapsible sections + server stats ──
 document.querySelectorAll('#tab-info .info-section h2').forEach(h2 => {
  h2.addEventListener('click', () => {
   h2.parentElement.classList.toggle('collapsed');
  });
 });
 // Fetch server stats
 (async function loadInfoStats() {
  try {
   const [statsRes, infoRes] = await Promise.all([
    fetch('/api/stats').then(r => r.ok ? r.json() : null).catch(() => null),
    fetch('/api/server-info').then(r => r.ok ? r.json() : null).catch(() => null)
   ]);
   if (statsRes) {
    if (statsRes.online !== undefined) document.getElementById('info-online').textContent = statsRes.online;
    if (statsRes.registered !== undefined) document.getElementById('info-registered').textContent = statsRes.registered;
    if (statsRes.uptime) {
     const s = statsRes.uptime;
     const h = Math.floor(s / 3600);
     const m = Math.floor((s % 3600) / 60);
     document.getElementById('info-uptime').textContent = h + 'h ' + m + 'm';
    }
   }
   if (infoRes) {
    if (infoRes.version) document.getElementById('info-version').textContent = infoRes.version;
    else if (infoRes.name) document.getElementById('info-version').textContent = infoRes.name;
   }
  } catch(e) { console.warn('Info stats fetch failed:', e); }
 })();
