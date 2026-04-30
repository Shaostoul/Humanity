// ══════════════════════════════════════════════
// Tab switching
// ══════════════════════════════════════════════

function switchTab(tab) {
  const isNotes = tab === 'notes';
  document.getElementById('notes-content').style.display = isNotes ? 'flex' : 'none';
  document.getElementById('log-content').style.display   = isNotes ? 'none' : 'flex';
  document.getElementById('tab-notes').classList.toggle('active', isNotes);
  document.getElementById('tab-log').classList.toggle('active', !isNotes);
  document.getElementById('hint-notes').style.display = isNotes ? '' : 'none';
  document.getElementById('hint-log').style.display   = isNotes ? 'none' : '';
}

// ══════════════════════════════════════════════
// Notes — Personal encrypted data store
// ══════════════════════════════════════════════
// Goal: local-first encrypted notes using AES-256-GCM. Each note can optionally
// have per-note passphrase encryption on top of the base localStorage layer.
// All data lives in localStorage under 'hos_notes_v1'.

const NOTES_KEY = 'hos_notes_v1';

let notes = [];      // Array of note objects
let activeId = null; // ID of the currently open note
let saveTimer = null;
let pendingUnlockId = null; // Note ID waiting for passphrase unlock

// ── Crypto helpers (stand-alone, no dependency on chat crypto.js) ──

async function aesEncrypt(plaintext, passphrase) {
  const enc = new TextEncoder();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv   = crypto.getRandomValues(new Uint8Array(12));
  const keyMat = await crypto.subtle.importKey('raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
  const key = await crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 300000, hash: 'SHA-256' },
    keyMat, { name: 'AES-GCM', length: 256 }, false, ['encrypt']
  );
  const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, enc.encode(plaintext));
  return {
    v: 1,
    salt: btoa(String.fromCharCode(...salt)),
    iv:   btoa(String.fromCharCode(...iv)),
    ct:   btoa(String.fromCharCode(...new Uint8Array(ct))),
  };
}

async function aesDecrypt(bundle, passphrase) {
  const salt = Uint8Array.from(atob(bundle.salt), c => c.charCodeAt(0));
  const iv   = Uint8Array.from(atob(bundle.iv),   c => c.charCodeAt(0));
  const ct   = Uint8Array.from(atob(bundle.ct),   c => c.charCodeAt(0));
  const enc  = new TextEncoder();
  const keyMat = await crypto.subtle.importKey('raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
  const key = await crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 300000, hash: 'SHA-256' },
    keyMat, { name: 'AES-GCM', length: 256 }, false, ['decrypt']
  );
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ct);
  return new TextDecoder().decode(plain);
}

// ── Data helpers ──

function loadNotes() {
  try {
    notes = JSON.parse(localStorage.getItem(NOTES_KEY) || '[]');
  } catch { notes = []; }
}

function saveNotes() {
  localStorage.setItem(NOTES_KEY, JSON.stringify(notes));
}

function uid() {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

function getNote(id) { return notes.find(n => n.id === id) || null; }

// ── Rendering ──

function renderList() {
  const q = document.getElementById('search-input').value.toLowerCase();
  const list = document.getElementById('note-list');
  const sortMode = document.getElementById('sort-select') ? document.getElementById('sort-select').value : 'modified';
  const filtered = notes.filter(n =>
    !q ||
    (n.title || '').toLowerCase().includes(q) ||
    (!n.encrypted && (n.content || '').toLowerCase().includes(q))
  ).sort((a, b) => {
    if (sortMode === 'created') return (b.createdAt || 0) - (a.createdAt || 0);
    if (sortMode === 'alpha') return (a.title || '').localeCompare(b.title || '');
    return (b.updatedAt || 0) - (a.updatedAt || 0);
  });

  if (!filtered.length) {
    list.innerHTML = '<div style="padding:var(--space-xl);color:#444;font-size:.78rem;text-align:center">No notes yet.</div>';
    return;
  }

  list.innerHTML = filtered.map(n => {
    const title = n.title || 'Untitled';
    const preview = n.encrypted ? (typeof hosIcon==='function'?hosIcon('lock',12):'') + ' encrypted' : (n.content || '').slice(0, 60).replace(/\n/g, ' ');
    const date = new Date(n.updatedAt).toLocaleDateString();
    return `<div class="note-item${n.id === activeId ? ' active' : ''}" onclick="openNote('${n.id}')">
      <div class="note-item-title">${esc(title)}</div>
      <div class="note-item-preview">${esc(preview)}</div>
      <div class="note-item-date">${date}${n.encrypted ? ' <span class="note-item-encrypted">encrypted</span>' : ''}</div>
    </div>`;
  }).join('');
}

function showNotePanel(show) {
  document.getElementById('note-toolbar').style.display = show ? 'flex' : 'none';
  document.getElementById('note-body').style.display = show ? 'block' : 'none';
  document.getElementById('note-status').style.display = show ? 'flex' : 'none';
  document.getElementById('empty-state').style.display = show ? 'none' : 'flex';
}

function updateWordCount() {
  const text = document.getElementById('note-content').value;
  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  document.getElementById('word-count').textContent = words + ' words · ' + text.length + ' chars';
}

function setStatus(msg, timeout) {
  document.getElementById('status-msg').textContent = msg;
  if (timeout) setTimeout(() => {
    const cur = document.getElementById('status-msg');
    if (cur && cur.textContent === msg) cur.textContent = 'All changes saved';
  }, timeout);
}

// ── Note operations ──

function newNote() {
  const note = {
    id: uid(),
    title: '',
    content: '',
    encrypted: false,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
  notes.unshift(note);
  saveNotes();
  openNote(note.id);
}

function openNote(id) {
  const note = getNote(id);
  if (!note) return;

  if (note.encrypted && typeof note.content === 'object') {
    // Need passphrase to unlock
    pendingUnlockId = id;
    document.getElementById('unlock-overlay').classList.add('open');
    document.getElementById('unlock-err').textContent = '';
    document.getElementById('unlock-pass').value = '';
    showNotePanel(true);
    document.getElementById('note-title-input').value = note.title || '';
    document.getElementById('note-content').value = '';
    document.getElementById('note-content').placeholder = 'Enter passphrase to view…';
    document.getElementById('note-content').readOnly = true;
    setTimeout(() => document.getElementById('unlock-pass').focus(), 100);
  } else {
    pendingUnlockId = null;
    document.getElementById('unlock-overlay').classList.remove('open');
    activeId = id;
    showNotePanel(true);
    document.getElementById('note-title-input').value = note.title || '';
    document.getElementById('note-content').value = note.content || '';
    document.getElementById('note-content').placeholder = 'Start writing…';
    document.getElementById('note-content').readOnly = false;
    const encBtn = document.getElementById('btn-encrypt-toggle');
    encBtn.innerHTML = note.encrypted ? (typeof hosIcon==='function'?hosIcon('lock',14):'') + ' Encrypted' : (typeof hosIcon==='function'?hosIcon('unlock',14):'') + ' Plain';
    encBtn.classList.toggle('encrypt-on', note.encrypted);
    // Reset markdown preview to edit mode
    if (mdPreviewOn) {
      mdPreviewOn = false;
      document.getElementById('md-preview').style.display = 'none';
      document.getElementById('note-content').style.display = '';
      document.getElementById('btn-md-toggle').classList.remove('active-toggle');
      document.getElementById('btn-md-toggle').textContent = 'Preview';
    }
    updateWordCount();
    setStatus('');
    renderList();
    document.getElementById('note-content').focus();
  }
}

async function doUnlock() {
  const passphrase = document.getElementById('unlock-pass').value;
  const note = getNote(pendingUnlockId);
  if (!note || !passphrase) return;

  try {
    const plain = await aesDecrypt(note.content, passphrase);
    note._unlockPassphrase = passphrase; // keep for re-encrypting on save
    activeId = pendingUnlockId;
    pendingUnlockId = null;
    document.getElementById('unlock-overlay').classList.remove('open');
    document.getElementById('note-content').value = plain;
    document.getElementById('note-content').placeholder = 'Start writing…';
    document.getElementById('note-content').readOnly = false;
    const encBtn = document.getElementById('btn-encrypt-toggle');
    encBtn.innerHTML = (typeof hosIcon==='function'?hosIcon('lock',14):'') + ' Encrypted';
    encBtn.classList.add('encrypt-on');
    updateWordCount();
    setStatus('');
    renderList();
    document.getElementById('note-content').focus();
  } catch (e) {
    document.getElementById('unlock-err').textContent = 'Wrong passphrase.';
  }
}

function scheduleAutoSave() {
  if (!activeId) return;
  updateWordCount();
  setStatus('Saving…');
  clearTimeout(saveTimer);
  saveTimer = setTimeout(autoSave, 800);
}

async function autoSave() {
  const note = getNote(activeId);
  if (!note) return;
  const title   = document.getElementById('note-title-input').value;
  const content = document.getElementById('note-content').value;
  note.title     = title;
  note.updatedAt = Date.now();

  if (note.encrypted && note._unlockPassphrase) {
    note.content = await aesEncrypt(content, note._unlockPassphrase);
  } else {
    note.content = content;
  }

  saveNotes();
  renderList();
  setStatus('Saved ' + new Date().toLocaleTimeString(), 3000);
}

function toggleEncrypt() {
  const note = getNote(activeId);
  if (!note) return;

  if (!note.encrypted) {
    // Enable encryption — prompt for passphrase
    const pass = prompt('Choose a passphrase for this note.\nYou must enter it every time you open the note.');
    if (!pass || pass.length < 4) { alert('Passphrase too short (min 4 characters).'); return; }
    note.encrypted = true;
    note._unlockPassphrase = pass;
    const encBtn = document.getElementById('btn-encrypt-toggle');
    encBtn.innerHTML = (typeof hosIcon==='function'?hosIcon('lock',14):'') + ' Encrypted';
    encBtn.classList.add('encrypt-on');
    scheduleAutoSave(); // triggers aesEncrypt on the content
    setStatus('Note will be encrypted on next save.');
  } else {
    // Disable encryption
    if (!confirm('Remove encryption from this note? The content will be stored in plain text.')) return;
    note.encrypted = false;
    note._unlockPassphrase = null;
    note.content = document.getElementById('note-content').value;
    note.updatedAt = Date.now();
    saveNotes();
    const encBtn = document.getElementById('btn-encrypt-toggle');
    encBtn.innerHTML = (typeof hosIcon==='function'?hosIcon('unlock',14):'') + ' Plain';
    encBtn.classList.remove('encrypt-on');
    renderList();
    setStatus('Encryption removed.');
  }
}

function deleteActiveNote() {
  if (!activeId) return;
  const note = getNote(activeId);
  if (!confirm('Delete "' + (note?.title || 'Untitled') + '"? This cannot be undone.')) return;
  notes = notes.filter(n => n.id !== activeId);
  activeId = null;
  saveNotes();
  showNotePanel(false);
  renderList();
}

function esc(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ── Markdown preview ──

let mdPreviewOn = false;

function toggleMarkdownPreview() {
  mdPreviewOn = !mdPreviewOn;
  const textarea = document.getElementById('note-content');
  const preview = document.getElementById('md-preview');
  const btn = document.getElementById('btn-md-toggle');
  if (mdPreviewOn) {
    preview.innerHTML = renderMarkdown(textarea.value);
    preview.style.display = 'block';
    textarea.style.display = 'none';
    btn.classList.add('active-toggle');
    btn.textContent = 'Edit';
  } else {
    preview.style.display = 'none';
    textarea.style.display = '';
    btn.classList.remove('active-toggle');
    btn.textContent = 'Preview';
  }
}

function renderMarkdown(text) {
  if (!text) return '<p style="color:var(--text-muted)">Nothing to preview.</p>';
  let html = esc(text);
  // Code blocks (fenced)
  html = html.replace(/```([\s\S]*?)```/g, function(_, code) {
    return '<pre><code>' + code.trim() + '</code></pre>';
  });
  // Inline code
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
  // Headers
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');
  // Bold and italic
  html = html.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  // Blockquotes
  html = html.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>');
  // Horizontal rules
  html = html.replace(/^---$/gm, '<hr>');
  // Unordered lists
  html = html.replace(/^[*-] (.+)$/gm, '<li>$1</li>');
  html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul>$&</ul>');
  // Links
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
  // Line breaks (double newline = paragraph, single = br)
  html = html.replace(/\n\n/g, '</p><p>');
  html = html.replace(/\n/g, '<br>');
  html = '<p>' + html + '</p>';
  // Clean up empty paragraphs around block elements
  html = html.replace(/<p><(h[1-3]|pre|blockquote|ul|hr)/g, '<$1');
  html = html.replace(/<\/(h[1-3]|pre|blockquote|ul|hr)><\/p>/g, '</$1>');
  return html;
}

// ── Export ──

function toggleExportMenu() {
  document.getElementById('export-menu').classList.toggle('open');
}

function exportNote(format) {
  document.getElementById('export-menu').classList.remove('open');
  const note = getNote(activeId);
  if (!note) return;
  const title = (note.title || 'Untitled').replace(/[^a-zA-Z0-9_\- ]/g, '');
  const content = document.getElementById('note-content').value;
  const ext = format === 'md' ? '.md' : '.txt';
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = title + ext;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// Close export menu on click outside
document.addEventListener('click', function(e) {
  const menu = document.getElementById('export-menu');
  if (menu && !e.target.closest('.export-drop')) {
    menu.classList.remove('open');
  }
});

// ── Notes Init ──

loadNotes();
renderList();
showNotePanel(false);


// ══════════════════════════════════════════════
// Log — Personal logbook (namespaced with log_ prefix)
// ══════════════════════════════════════════════

const LOG_STORAGE_KEY = 'hos_logbook_v1';
let log_entries = [];
let log_activeId = null;
let log_saveTimer = null;

const LOG_TAG_COLORS = ['#4cf','#c8f','#4d9','#fa4','#f46','#0cf'];

function log_load() {
  try { log_entries = JSON.parse(localStorage.getItem(LOG_STORAGE_KEY)) || []; }
  catch(e) { log_entries = []; }
}
function log_save() {
  localStorage.setItem(LOG_STORAGE_KEY, JSON.stringify(log_entries));
  const ind = document.getElementById('log-save-indicator');
  ind.classList.add('visible');
  setTimeout(() => ind.classList.remove('visible'), 1500);
}

function log_uid() {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

function log_esc(s) {
  return (s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

function log_newEntry() {
  const now = new Date();
  const e = {
    id: log_uid(),
    title: 'Entry — ' + now.toLocaleDateString('en-US', {month:'short', day:'numeric', year:'numeric'}),
    body: '',
    tags: [],
    created: now.toISOString(),
    updated: now.toISOString()
  };
  log_entries.unshift(e);
  log_save();
  log_renderSidebar();
  log_openEntry(e.id);
}

function log_openEntry(id) {
  log_activeId = id;
  const e = log_entries.find(x => x.id === id);
  if (!e) return;

  document.getElementById('log-empty').style.display = 'none';
  document.getElementById('log-textarea').style.display = 'block';
  document.getElementById('log-footer-bar').style.display = 'flex';
  document.getElementById('log-entry-title-input').style.display = 'block';
  document.getElementById('log-tag-input').style.display = 'block';
  document.getElementById('log-delete-btn').style.display = 'block';

  document.getElementById('log-entry-title-input').value = e.title;
  document.getElementById('log-textarea').value = e.body;
  document.getElementById('log-tag-input').value = (e.tags || []).join(', ');
  document.getElementById('log-date-display').textContent = new Date(e.created).toLocaleString('en-US', {
    month:'short', day:'numeric', year:'numeric', hour:'2-digit', minute:'2-digit'
  });

  log_updateWordCount();
  log_renderSidebar();
  document.getElementById('log-textarea').focus();
}

function log_deleteCurrentEntry() {
  if (!log_activeId) return;
  if (!confirm('Delete this entry? This cannot be undone.')) return;
  log_entries = log_entries.filter(x => x.id !== log_activeId);
  log_activeId = null;
  log_save();

  // Show empty or open next
  if (log_entries.length) {
    log_openEntry(log_entries[0].id);
  } else {
    document.getElementById('log-empty').style.display = 'flex';
    document.getElementById('log-textarea').style.display = 'none';
    document.getElementById('log-footer-bar').style.display = 'none';
    document.getElementById('log-entry-title-input').style.display = 'none';
    document.getElementById('log-tag-input').style.display = 'none';
    document.getElementById('log-delete-btn').style.display = 'none';
    document.getElementById('log-entry-title-input').value = '';
    document.getElementById('log-textarea').value = '';
    document.getElementById('log-date-display').textContent = '';
    log_renderSidebar();
  }
}

function log_onTitleInput() {
  const e = log_entries.find(x => x.id === log_activeId);
  if (!e) return;
  e.title = document.getElementById('log-entry-title-input').value;
  e.updated = new Date().toISOString();
  log_scheduleSave();
  log_renderSidebar();
}

function log_onTextInput() {
  const e = log_entries.find(x => x.id === log_activeId);
  if (!e) return;
  e.body = document.getElementById('log-textarea').value;
  e.updated = new Date().toISOString();
  log_updateWordCount();
  log_scheduleSave();
  log_renderSidebar();
}

function log_onTagInput() {
  const e = log_entries.find(x => x.id === log_activeId);
  if (!e) return;
  e.tags = document.getElementById('log-tag-input').value
    .split(',').map(t => t.trim()).filter(Boolean);
  log_scheduleSave();
  log_renderSidebar();
}

function log_scheduleSave() {
  clearTimeout(log_saveTimer);
  log_saveTimer = setTimeout(log_save, 800);
}

function log_updateWordCount() {
  const txt = document.getElementById('log-textarea').value;
  const words = txt.trim() ? txt.trim().split(/\s+/).length : 0;
  const chars = txt.length;
  const lines = txt ? txt.split('\n').length : 0;
  document.getElementById('wc-words').textContent = words + ' word' + (words !== 1 ? 's' : '');
  document.getElementById('wc-chars').textContent = chars + ' char' + (chars !== 1 ? 's' : '');
  document.getElementById('wc-lines').textContent = lines + ' line' + (lines !== 1 ? 's' : '');
}

function log_tagColor(tag) {
  let h = 0;
  for (let i = 0; i < tag.length; i++) h = (h * 31 + tag.charCodeAt(i)) | 0;
  return LOG_TAG_COLORS[Math.abs(h) % LOG_TAG_COLORS.length];
}

function log_renderSidebar() {
  const search = (document.getElementById('log-search').value || '').toLowerCase();
  let visible = log_entries;
  if (search) {
    visible = log_entries.filter(e =>
      e.title.toLowerCase().includes(search) ||
      (e.body || '').toLowerCase().includes(search) ||
      (e.tags || []).some(t => t.toLowerCase().includes(search))
    );
  }
  const list = document.getElementById('log-entry-list');
  if (!visible.length) {
    list.innerHTML = '<div style="color:#444;font-size:0.8rem;text-align:center;padding:var(--space-2xl) var(--space-md);">' +
      (search ? 'No entries match your search.' : 'No entries yet.') + '</div>';
    return;
  }
  list.innerHTML = visible.map(e => {
    const isActive = e.id === log_activeId;
    const preview = (e.body || '').replace(/\n/g, ' ').trim().slice(0, 60);
    const tags = (e.tags || []).slice(0, 3).map(t =>
      `<span class="log-tag" style="background:${log_tagColor(t)}22;color:${log_tagColor(t)};border:1px solid ${log_tagColor(t)}44;">${log_esc(t)}</span>`
    ).join('');
    return `<div class="log-entry-item${isActive ? ' active' : ''}" onclick="log_openEntry('${e.id}')">
      <div class="log-item-date">${new Date(e.created).toLocaleDateString('en-US',{month:'short',day:'numeric',year:'numeric'})}</div>
      <div class="log-item-title">${log_esc(e.title)}</div>
      ${preview ? `<div class="log-item-preview">${log_esc(preview)}</div>` : ''}
      ${tags ? `<div style="margin-top:3px;">${tags}</div>` : ''}
    </div>`;
  }).join('');
}

document.addEventListener('keydown', function(e) {
  // Ctrl+N / Ctrl+S — route to correct tab
  if ((e.ctrlKey || e.metaKey) && e.key === 'n') {
    e.preventDefault();
    if (document.getElementById('log-content').style.display !== 'none') {
      log_newEntry();
    } else {
      newNote();
    }
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault();
    if (document.getElementById('log-content').style.display !== 'none') {
      log_save();
    } else {
      autoSave();
    }
  }
});

// ── Log Init ──
log_load();
log_renderSidebar();
if (log_entries.length) log_openEntry(log_entries[0].id);

// ── Replace data-icon placeholders with SVG icons ──
document.querySelectorAll('.pg-ico[data-icon]').forEach(function(el) {
  if (window.hosIcon) el.innerHTML = hosIcon(el.dataset.icon, parseInt(el.dataset.size) || 20);
});
