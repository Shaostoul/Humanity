// ── files-folder.js ───────────────────────────────────────────────────────
// The Files page's "HumanityOS folder" place on the website (2026-10-19): the
// data folder of the server this website runs on, as a tree, with built-in
// views for its common formats and a text editor with line numbers. It was the
// data browser at /files (its own page, "Files") until the Files page took it in
// as a place; the desktop app's twin is src/gui/pages/files/app_folder.rs, which
// starts one folder up (the app's own folder, so mods/ shows too). A website can
// only reach the server's data/ through /api/data-files, so this one starts there.
//
// The tree on the left under the places (Refresh above it), the file in the
// middle (the bar with where it is, Wrap and Save above it), and on the right
// what it is and what editing it means (its CAUTION, which the page's "How this
// page works" says too, 2026-10-20). FilesFolder on window; the inline
// handlers (breadcrumb, JSON toggles) name it, so there is one on the page.
// ──────────────────────────────────────────────────────────────────────────
const FilesFolder = (function () {
  'use strict';

  /** What the place says about editing these files. */
  const CAUTION = 'The data folder of the server this website runs on: the files HumanityOS reads for its plants, recipes, maps and the rest. Changing one changes how the app works for everyone on this server.';
  let mounted = false;

  // ── State ──
  let currentPath = null;       // Currently open file path
  let originalContent = null;   // Content as loaded (for dirty detection)
  let isWrapping = false;
  let expandedDirs = new Set(['data']);  // Track which dirs are expanded
  let activeTreeItem = null;    // Currently highlighted tree item element

  // ── Extension categories ──
  const TEXT_EXTS = new Set([
    'txt','md','rs','js','py','toml','json','csv','html','css',
    'ron','yaml','yml','xml','cfg','ini','sh','bat'
  ]);
  const IMAGE_EXTS = new Set(['png','jpg','jpeg','gif','svg','webp']);
  const AUDIO_EXTS = new Set(['mp3','ogg','wav']);
  const VIDEO_EXTS = new Set(['mp4','webm']);

  // ── DOM refs ──
  const $ = (id) => document.getElementById(id);

  // ── Helpers ──
  function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  function formatDate(epoch) {
    if (!epoch) return '';
    const d = new Date(epoch * 1000);
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function getExtension(name) {
    const dot = name.lastIndexOf('.');
    return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
  }

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  // ── Breadcrumb ──
  function updateBreadcrumb(path) {
    const bc = $('breadcrumb');
    if (!path) {
      bc.innerHTML = '<span class="breadcrumb-seg active">data/</span>';
      return;
    }
    const parts = path.split('/');
    let html = '';
    for (let i = 0; i < parts.length; i++) {
      const partial = parts.slice(0, i + 1).join('/');
      const isLast = i === parts.length - 1;
      if (i > 0) html += '<span class="breadcrumb-sep">/</span>';
      if (isLast) {
        html += '<span class="breadcrumb-seg active">' + escapeHtml(parts[i]) + '</span>';
      } else {
        html += '<button class="breadcrumb-seg" onclick="FilesFolder.openDir(\'' +
          partial.replace(/'/g, "\\'") + '\')">' + escapeHtml(parts[i]) + '</button>';
      }
    }
    bc.innerHTML = html;
  }

  // ── File tree ──
  async function fetchDir(dirPath) {
    try {
      const res = await fetch('/api/data-files?path=' + encodeURIComponent(dirPath));
      const data = await res.json();
      if (data.error) throw new Error(data.error);
      return data.entries || [];
    } catch (e) {
      console.error('[Files] fetchDir error:', e);
      return [];
    }
  }

  function fileIcon(entry) {
    if (entry.is_directory) return '\u{1F4C1}';
    const ext = entry.extension;
    if (IMAGE_EXTS.has(ext)) return '\u{1F5BC}';
    if (AUDIO_EXTS.has(ext)) return '\u{1F3B5}';
    if (VIDEO_EXTS.has(ext)) return '\u{1F3AC}';
    if (ext === 'json') return '{}';
    if (ext === 'csv') return '\u{1F4CA}';
    if (ext === 'md') return '\u{1F4DD}';
    if (ext === 'toml' || ext === 'yaml' || ext === 'yml' || ext === 'cfg' || ext === 'ini') return '\u2699';
    if (ext === 'rs') return '\u{1F980}';
    if (ext === 'ron') return '\u{1F4E6}';
    return '\u{1F4C4}';
  }

  function renderTreeLevel(entries, parentEl, depth) {
    entries.forEach(entry => {
      const item = document.createElement('button');
      item.className = 'tree-item' + (entry.is_directory ? ' dir' : '');
      item.style.paddingLeft = (12 + depth * 16) + 'px';

      const arrow = entry.is_directory
        ? (expandedDirs.has(entry.path) ? '\u25BE ' : '\u25B8 ')
        : '';

      item.innerHTML =
        '<span class="icon">' + (entry.is_directory ? arrow : '') + fileIcon(entry) + '</span>' +
        '<span class="name">' + escapeHtml(entry.name) + '</span>' +
        (!entry.is_directory ? '<span class="size">' + formatSize(entry.size) + '</span>' : '');

      if (entry.is_directory) {
        item.onclick = () => toggleDir(entry.path, item, depth);
      } else {
        item.onclick = () => openFile(entry.path, entry);
        item.dataset.path = entry.path;
      }

      parentEl.appendChild(item);

      // If this dir was previously expanded, load its children.
      if (entry.is_directory && expandedDirs.has(entry.path)) {
        const childContainer = document.createElement('div');
        childContainer.className = 'tree-children open';
        childContainer.id = 'children-' + entry.path.replace(/[^a-zA-Z0-9]/g, '_');
        parentEl.appendChild(childContainer);
        // Load children async.
        fetchDir(entry.path).then(children => {
          renderTreeLevel(children, childContainer, depth + 1);
        });
      }
    });
  }

  async function toggleDir(dirPath, itemEl, depth) {
    const childId = 'children-' + dirPath.replace(/[^a-zA-Z0-9]/g, '_');
    let childContainer = document.getElementById(childId);

    if (expandedDirs.has(dirPath)) {
      // Collapse.
      expandedDirs.delete(dirPath);
      if (childContainer) childContainer.remove();
      // Update arrow.
      const iconSpan = itemEl.querySelector('.icon');
      if (iconSpan) iconSpan.innerHTML = '\u25B8 ' + fileIcon({ is_directory: true });
    } else {
      // Expand.
      expandedDirs.add(dirPath);
      const iconSpan = itemEl.querySelector('.icon');
      if (iconSpan) iconSpan.innerHTML = '\u25BE ' + fileIcon({ is_directory: true });

      childContainer = document.createElement('div');
      childContainer.className = 'tree-children open';
      childContainer.id = childId;
      // Insert after the item.
      itemEl.insertAdjacentElement('afterend', childContainer);

      const children = await fetchDir(dirPath);
      renderTreeLevel(children, childContainer, depth + 1);
    }
  }

  async function refreshTree() {
    const treeList = $('tree-list');
    treeList.innerHTML = '<div class="loading">Loading...</div>';

    const entries = await fetchDir('data');
    treeList.innerHTML = '';
    renderTreeLevel(entries, treeList, 0);
  }

  // ── Open directory (from breadcrumb) ──
  function openDir(dirPath) {
    expandedDirs.add(dirPath);
    // Expand parents too.
    const parts = dirPath.split('/');
    for (let i = 1; i < parts.length; i++) {
      expandedDirs.add(parts.slice(0, i + 1).join('/'));
    }
    refreshTree();
    updateBreadcrumb(dirPath);
    showEmpty();
  }

  // ── File opening ──
  async function openFile(filePath, entry) {
    // Check for unsaved changes.
    if (isDirty() && !confirm('You have unsaved changes. Discard them?')) return;

    currentPath = filePath;
    updateBreadcrumb(filePath);

    // Highlight in tree.
    if (activeTreeItem) activeTreeItem.classList.remove('active');
    const items = document.querySelectorAll('.tree-item[data-path="' + CSS.escape(filePath) + '"]');
    if (items.length) {
      activeTreeItem = items[0];
      activeTreeItem.classList.add('active');
    }

    const ext = getExtension(filePath);
    const viewer = $('file-viewer');

    // Image/audio/video: show media viewer (no API call needed for binary).
    if (IMAGE_EXTS.has(ext)) {
      showMediaViewer('image', filePath, entry);
      return;
    }
    if (AUDIO_EXTS.has(ext)) {
      showMediaViewer('audio', filePath, entry);
      return;
    }
    if (VIDEO_EXTS.has(ext)) {
      showMediaViewer('video', filePath, entry);
      return;
    }

    // Text-based: fetch content.
    if (TEXT_EXTS.has(ext)) {
      viewer.innerHTML = '<div class="loading">Loading...</div>';
      try {
        const res = await fetch('/api/data-files/read?path=' + encodeURIComponent(filePath));
        const data = await res.json();
        if (data.error) throw new Error(data.error);

        showFileInfo(data.size, data.modified);

        if (ext === 'json') {
          showJsonViewer(data.content, filePath);
        } else if (ext === 'csv') {
          showCsvViewer(data.content, filePath);
        } else if (ext === 'md') {
          showMarkdownViewer(data.content, filePath);
        } else {
          showEditor(data.content, filePath);
        }
      } catch (e) {
        viewer.innerHTML = '<div class="viewer-empty"><p>Error: ' + escapeHtml(e.message) + '</p></div>';
      }
      return;
    }

    // Unknown file type.
    showUnknownViewer(filePath, entry);
  }

  // ── Viewers ──

  function showEmpty() {
    currentPath = null;
    originalContent = null;
    $('file-viewer').innerHTML =
      '<div class="viewer-empty"><div class="empty-icon">&#128193;</div>' +
      '<p>Select a file to view or edit</p></div>';
    $('file-info').textContent = '';
    $('btn-save').classList.remove('visible');
    $('btn-wrap').classList.remove('visible');
    $('unsaved-dot').classList.remove('visible');
  }

  function showFileInfo(size, modified) {
    const info = [];
    if (size) info.push(formatSize(size));
    if (modified) info.push(formatDate(modified));
    $('file-info').textContent = info.join(' \u2022 ');
  }

  function showEditor(content, filePath) {
    originalContent = content;
    const viewer = $('file-viewer');

    const lines = content.split('\n');
    let lineNumsHtml = '';
    for (let i = 1; i <= lines.length; i++) {
      lineNumsHtml += i + '\n';
    }

    viewer.innerHTML =
      '<div class="editor-wrap">' +
        '<pre class="line-numbers" id="line-nums">' + lineNumsHtml + '</pre>' +
        '<textarea class="editor-textarea' + (isWrapping ? ' wrap' : '') +
        '" id="editor" spellcheck="false">' + escapeHtml(content) + '</textarea>' +
      '</div>';

    const editor = $('editor');
    const lineNums = $('line-nums');

    // Sync scroll.
    editor.addEventListener('scroll', () => {
      lineNums.scrollTop = editor.scrollTop;
    });

    // Update line numbers on input.
    editor.addEventListener('input', () => {
      updateLineNumbers();
      updateDirtyState();
    });

    // Tab key inserts spaces.
    editor.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') {
        e.preventDefault();
        const start = editor.selectionStart;
        const end = editor.selectionEnd;
        const val = editor.value;
        editor.value = val.substring(0, start) + '    ' + val.substring(end);
        editor.selectionStart = editor.selectionEnd = start + 4;
        editor.dispatchEvent(new Event('input'));
      }
      // Ctrl+S to save.
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        save();
      }
    });

    $('btn-save').classList.add('visible');
    $('btn-wrap').classList.add('visible');
    $('unsaved-dot').classList.remove('visible');
  }

  function updateLineNumbers() {
    const editor = $('editor');
    const lineNums = $('line-nums');
    if (!editor || !lineNums) return;
    const count = editor.value.split('\n').length;
    let html = '';
    for (let i = 1; i <= count; i++) html += i + '\n';
    lineNums.textContent = html;
  }

  function showJsonViewer(content, filePath) {
    originalContent = content;
    const viewer = $('file-viewer');

    let parsed;
    try {
      parsed = JSON.parse(content);
    } catch (e) {
      // Invalid JSON, fall back to text editor.
      showEditor(content, filePath);
      return;
    }

    // Render formatted JSON with collapsible sections.
    viewer.innerHTML =
      '<div class="json-viewer" id="json-view">' + renderJson(parsed, 0) + '</div>' +
      '<div style="display:none"><textarea id="editor-hidden">' + escapeHtml(content) + '</textarea></div>';

    // Also show editor button.
    $('btn-save').classList.add('visible');
    $('btn-wrap').classList.remove('visible');

    // Add a toggle to switch to raw editor.
    const toggle = document.createElement('button');
    toggle.textContent = 'Edit Raw';
    toggle.className = 'btn-wrap visible';
    toggle.style.position = 'absolute';
    toggle.style.top = '8px';
    toggle.style.right = '8px';
    toggle.onclick = () => showEditor(content, filePath);
    viewer.style.position = 'relative';
    viewer.appendChild(toggle);
  }

  function renderJson(obj, depth) {
    if (obj === null) return '<span class="json-null">null</span>';
    if (typeof obj === 'boolean') return '<span class="json-bool">' + obj + '</span>';
    if (typeof obj === 'number') return '<span class="json-num">' + obj + '</span>';
    if (typeof obj === 'string') return '<span class="json-str">"' + escapeHtml(obj) + '"</span>';

    const indent = '  '.repeat(depth);
    const innerIndent = '  '.repeat(depth + 1);

    if (Array.isArray(obj)) {
      if (obj.length === 0) return '[]';
      const id = 'jt_' + Math.random().toString(36).slice(2, 8);
      let html = '<span class="json-toggle" onclick="FilesFolder.toggleJson(\'' + id + '\')">\u25BE</span>[<div id="' + id + '">';
      obj.forEach((item, i) => {
        html += innerIndent + renderJson(item, depth + 1);
        if (i < obj.length - 1) html += ',';
        html += '\n';
      });
      html += indent + '</div>' + indent + ']';
      return html;
    }

    if (typeof obj === 'object') {
      const keys = Object.keys(obj);
      if (keys.length === 0) return '{}';
      const id = 'jt_' + Math.random().toString(36).slice(2, 8);
      let html = '<span class="json-toggle" onclick="FilesFolder.toggleJson(\'' + id + '\')">\u25BE</span>{<div id="' + id + '">';
      keys.forEach((key, i) => {
        html += innerIndent + '<span class="json-key">"' + escapeHtml(key) + '"</span>: ' +
          renderJson(obj[key], depth + 1);
        if (i < keys.length - 1) html += ',';
        html += '\n';
      });
      html += indent + '</div>' + indent + '}';
      return html;
    }

    return escapeHtml(String(obj));
  }

  function toggleJson(id) {
    const el = document.getElementById(id);
    if (!el) return;
    const toggle = el.previousElementSibling;
    if (el.classList.contains('json-collapsed')) {
      el.classList.remove('json-collapsed');
      if (toggle) toggle.textContent = '\u25BE';
    } else {
      el.classList.add('json-collapsed');
      if (toggle) toggle.textContent = '\u25B8';
    }
  }

  function showCsvViewer(content, filePath) {
    originalContent = content;
    const viewer = $('file-viewer');

    const lines = content.trim().split('\n');
    if (lines.length === 0) {
      showEditor(content, filePath);
      return;
    }

    // Simple CSV parse (handles basic comma-separated, no quoted commas).
    function parseCsvLine(line) {
      const result = [];
      let current = '';
      let inQuotes = false;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') {
          if (inQuotes && line[i + 1] === '"') {
            current += '"';
            i++;
          } else {
            inQuotes = !inQuotes;
          }
        } else if (ch === ',' && !inQuotes) {
          result.push(current.trim());
          current = '';
        } else {
          current += ch;
        }
      }
      result.push(current.trim());
      return result;
    }

    const headers = parseCsvLine(lines[0]);
    let tableHtml = '<div class="csv-viewer"><table class="csv-table"><thead><tr>';
    headers.forEach(h => { tableHtml += '<th>' + escapeHtml(h) + '</th>'; });
    tableHtml += '</tr></thead><tbody>';

    for (let i = 1; i < lines.length; i++) {
      if (!lines[i].trim()) continue;
      const cols = parseCsvLine(lines[i]);
      tableHtml += '<tr>';
      headers.forEach((_, j) => {
        tableHtml += '<td>' + escapeHtml(cols[j] || '') + '</td>';
      });
      tableHtml += '</tr>';
    }
    tableHtml += '</tbody></table></div>';

    viewer.innerHTML = tableHtml;

    // Add edit raw button.
    $('btn-save').classList.add('visible');
    $('btn-wrap').classList.remove('visible');

    const toggle = document.createElement('button');
    toggle.textContent = 'Edit Raw';
    toggle.className = 'btn-wrap visible';
    toggle.style.cssText = 'position:absolute;top:8px;right:8px;';
    toggle.onclick = () => showEditor(content, filePath);
    viewer.style.position = 'relative';
    viewer.appendChild(toggle);
  }

  function showMarkdownViewer(content, filePath) {
    originalContent = content;
    const viewer = $('file-viewer');

    const html = window.hosMarkdown.render(content);
    viewer.innerHTML = '<div class="md-viewer">' + html + '</div>';

    $('btn-save').classList.add('visible');
    $('btn-wrap').classList.remove('visible');

    const toggle = document.createElement('button');
    toggle.textContent = 'Edit Raw';
    toggle.className = 'btn-wrap visible';
    toggle.style.cssText = 'position:absolute;top:8px;right:8px;';
    toggle.onclick = () => showEditor(content, filePath);
    viewer.style.position = 'relative';
    viewer.appendChild(toggle);
  }

  // Markdown rendering moved to the shared web/shared/markdown.js module
  // (window.hosMarkdown.render) so the Accord doc browser can reuse it.

  function showMediaViewer(type, filePath, entry) {
    const viewer = $('file-viewer');
    // Media files are not served through the API; they would need
    // a direct URL. Since we only serve data/ text files via the API,
    // show a placeholder with file info.
    let html = '<div class="unknown-viewer">';
    if (type === 'image') {
      html += '<div class="file-icon">&#128444;</div>';
    } else if (type === 'audio') {
      html += '<div class="file-icon">&#127925;</div>';
    } else {
      html += '<div class="file-icon">&#127916;</div>';
    }
    html += '<div class="file-name">' + escapeHtml(filePath.split('/').pop()) + '</div>';
    if (entry) {
      html += '<div class="file-meta">' + formatSize(entry.size) + '</div>';
    }
    html += '<div class="note">Binary files are not served through the file API.<br>Use the native desktop app or external tools to view this file.</div>';
    html += '</div>';
    viewer.innerHTML = html;

    $('btn-save').classList.remove('visible');
    $('btn-wrap').classList.remove('visible');
    $('unsaved-dot').classList.remove('visible');
    if (entry) showFileInfo(entry.size, entry.modified);
  }

  function showUnknownViewer(filePath, entry) {
    const viewer = $('file-viewer');
    const name = filePath.split('/').pop();
    viewer.innerHTML =
      '<div class="unknown-viewer">' +
        '<div class="file-icon">&#128196;</div>' +
        '<div class="file-name">' + escapeHtml(name) + '</div>' +
        (entry ? '<div class="file-meta">' + formatSize(entry.size) + ' \u2022 .' + escapeHtml(entry.extension) + '</div>' : '') +
        '<div class="note">This file type cannot be previewed in the browser.<br>Use the native desktop app or an external tool to open it.</div>' +
      '</div>';

    $('btn-save').classList.remove('visible');
    $('btn-wrap').classList.remove('visible');
    $('unsaved-dot').classList.remove('visible');
    if (entry) showFileInfo(entry.size, entry.modified);
  }

  // ── Dirty state ──
  function isDirty() {
    const editor = $('editor');
    if (!editor || originalContent === null) return false;
    return editor.value !== originalContent;
  }

  function updateDirtyState() {
    const dirty = isDirty();
    $('unsaved-dot').classList.toggle('visible', dirty);
    const name = currentPath ? currentPath.split('/').pop() : 'HumanityOS folder';
  }

  // ── Save ──
  async function save() {
    const editor = $('editor');
    if (!editor || !currentPath) return;

    const content = editor.value;
    $('btn-save').textContent = 'Saving...';
    $('btn-save').disabled = true;

    try {
      // Only this server's admins may change its data files (2026-10-20: the route took no
      // signature before). The signature covers the path and the BLAKE3 of the exact content,
      // the words the relay rebuilds in src/relay/api.rs `write_purpose`.
      const hash = typeof window.pqBlake3 === 'function' ? await window.pqBlake3(new TextEncoder().encode(content)) : null;
      const auth = hash && typeof window.getPqSignedAuth === 'function'
        ? await window.getPqSignedAuth('data_files_write\n' + currentPath + '\n' + Array.from(hash, (b) => b.toString(16).padStart(2, '0')).join(''))
        : null;
      if (!auth) throw new Error('Only this server\'s admins can change its data files. Sign in via Chat first, as an admin.');
      const res = await fetch('/api/data-files/write', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: currentPath, content, key: auth.key, timestamp: auth.timestamp, sig: auth.sig }),
      });
      const data = await res.json();
      if (data.error) throw new Error(data.error);

      originalContent = content;
      updateDirtyState();
      $('btn-save').textContent = 'Saved!';
      setTimeout(() => { $('btn-save').textContent = 'Save'; }, 1500);
    } catch (e) {
      alert('Save failed: ' + e.message);
      $('btn-save').textContent = 'Save';
    } finally {
      $('btn-save').disabled = false;
    }
  }

  // ── Word wrap toggle ──
  function toggleWrap() {
    isWrapping = !isWrapping;
    const editor = $('editor');
    if (editor) {
      editor.classList.toggle('wrap', isWrapping);
    }
    $('btn-wrap').textContent = isWrapping ? 'No Wrap' : 'Wrap';
  }

  // ── Keyboard shortcut (global Ctrl+S) ──
  if (typeof document !== 'undefined') document.addEventListener('keydown', (e) => {
    if (mounted && (e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault();
      if (currentPath && $('editor')) save();
    }
  });

  // Warn on page leave with unsaved changes.
  if (typeof window !== 'undefined') window.addEventListener('beforeunload', (e) => {
    if (mounted && isDirty()) {
      e.preventDefault();
      e.returnValue = '';
    }
  });

  // ── The place ──
  /**
   * The place's three parts for the Files page (/pages/files-places.js): the tree, the file and what
   * it is, built once; the tree is read when the place is first shown.
   */
  function createFolderPlace() {
    const left = document.createElement('div');
    left.className = 'files-folder fd-tree';
    left.innerHTML =
      '<div class="tree-header"><h2 class="files-place-heading">HumanityOS folder</h2>' +
      '<button class="btn-refresh-tree" onclick="FilesFolder.refreshTree()" title="Read the folder again">Refresh</button></div>' +
      '<div class="tree-list" id="tree-list"><div class="loading">Loading...</div></div>';
    const middle = document.createElement('div');
    middle.className = 'files-folder fd-middle';
    middle.innerHTML =
      '<div class="fd-topbar">' +
        '<span class="unsaved-dot" id="unsaved-dot" title="Unsaved changes">*</span>' +
        '<div class="breadcrumb" id="breadcrumb"></div>' +
        '<button class="btn-wrap" id="btn-wrap" onclick="FilesFolder.toggleWrap()">Wrap</button>' +
        '<button class="btn-save" id="btn-save" onclick="FilesFolder.save()">Save</button>' +
      '</div>' +
      '<div class="file-viewer" id="file-viewer"><div class="viewer-empty"><p>Choose a file on the left to read it here.</p></div></div>';
    const right = document.createElement('div');
    right.className = 'files-folder fd-details';
    const info = document.createElement('p');
    info.className = 'file-info';
    info.id = 'file-info';
    const caution = document.createElement('p');
    caution.className = 'files-hint';
    caution.textContent = CAUTION;
    right.append(info, caution);
    return {
      left, middle, right,
      start() {
        mounted = true;
        updateBreadcrumb(null);
        refreshTree();
      },
    };
  }

  // ── Public API ──
  return {
    CAUTION,
    createFolderPlace,
    refreshTree,
    openDir,
    save,
    toggleWrap,
    toggleJson,
  };
})();
if (typeof window !== 'undefined') window.FilesFolder = FilesFolder;
// Under Node (scripts/tests/files-web-places.test.js, and files-app.js for the caution its "How this
// page works" says): nothing here reaches for the document until a place is made.
if (typeof module === 'object' && module && module.exports) module.exports = FilesFolder;
