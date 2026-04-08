const DEFAULTS = window.HOS_DEFAULTS || {};
const STORAGE_KEY = window.HOS_STORAGE_KEY || 'humanity_settings';

let prefs = Object.assign({}, DEFAULTS);

// crypto.js functions reference myIdentity/myName globals (set by app.js on the chat page).
// Settings loads crypto.js but not app.js, so we initialize them here from stored keys.
var myIdentity = null;
var myName = localStorage.getItem('humanity_name') || 'identity';
(async function initSettingsIdentity() {
  if (typeof getOrCreateIdentity === 'function') {
    try {
      myIdentity = await getOrCreateIdentity();
    } catch(e) {
      console.warn('Settings: could not load identity:', e);
    }
  }
})();

function loadPrefs() {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY));
    if (stored) Object.assign(prefs, stored);
  } catch(e) {}
}

function saveEli5Pref() {
  var s = document.getElementById('pref-show-simple');
  var e = document.getElementById('pref-show-expert');
  localStorage.setItem('hos_show_simple', s && s.checked ? '1' : '0');
  localStorage.setItem('hos_show_expert', e && e.checked ? '1' : '0');
  if (window.__hos_eli5_update) window.__hos_eli5_update();
}

function savePref() {
  // Read all inputs
  Object.keys(DEFAULTS).forEach(k => {
    const el = document.getElementById('pref-' + k);
    if (!el) return;
    if (el.type === 'checkbox') prefs[k] = el.checked;
    else prefs[k] = el.value;
  });
  localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  // Reflect debug-panel toggle: remove instantly; enable after brief delay (shell.js needs reload to inject)
  const dbgExisting = document.getElementById('hos-debug-overlay');
  if (!prefs['debug-panel'] && dbgExisting) dbgExisting.remove();
  else if (prefs['debug-panel'] && !dbgExisting) setTimeout(() => location.reload(), 400);
}

function applyPrefs() {
  Object.keys(DEFAULTS).forEach(k => {
    const el = document.getElementById('pref-' + k);
    if (!el) return;
    if (el.type === 'checkbox') el.checked = !!prefs[k];
    else el.value = prefs[k] !== undefined ? prefs[k] : DEFAULTS[k];
  });
  // Show public key if available
  try {
    const id = JSON.parse(localStorage.getItem('hos_identity'));
    if (id && id.publicKeyHex) {
      document.getElementById('pubkey-display').textContent = id.publicKeyHex.slice(0, 16) + '…' + id.publicKeyHex.slice(-8);
    }
  } catch(e) {}
  // Load ELI5 prefs
  var es = document.getElementById('pref-show-simple');
  var ee = document.getElementById('pref-show-expert');
  if (es) es.checked = localStorage.getItem('hos_show_simple') !== '0';
  if (ee) ee.checked = localStorage.getItem('hos_show_expert') !== '0';
  // Load customizer values from the unified prefs store
  try {
    var iwSlider = document.getElementById('pref-icon-weight');
    if (iwSlider) {
      var iw = prefs.iconWeight != null ? prefs.iconWeight : (window.hosGetIconWeight ? hosGetIconWeight() : 3);
      iwSlider.value = iw;
      document.getElementById('icon-weight-val').textContent = iw;
    }
    var isSlider = document.getElementById('pref-icon-size');
    if (isSlider) {
      var is = prefs.iconSize != null ? prefs.iconSize : 20;
      isSlider.value = is;
      document.getElementById('icon-size-val').textContent = is + 'px';
    }
    var fsSlider = document.getElementById('pref-font-size-slider');
    if (fsSlider) {
      var fs = prefs.fontSizePx != null ? prefs.fontSizePx : 16;
      fsSlider.value = fs;
      document.getElementById('font-size-val').textContent = fs + 'px';
    }
    var brSlider = document.getElementById('pref-border-radius');
    if (brSlider) {
      var br = prefs.borderRadius != null ? prefs.borderRadius : 8;
      brSlider.value = br;
      document.getElementById('border-radius-val').textContent = br + 'px';
    }
    var cwSlider = document.getElementById('pref-content-width');
    if (cwSlider) {
      var cw = prefs.contentWidth != null ? prefs.contentWidth : 0;
      cwSlider.value = cw;
      document.getElementById('content-width-val').textContent = cw === 0 ? 'Full' : cw + 'px';
    }
    var lhSlider = document.getElementById('pref-line-height');
    if (lhSlider) {
      var lh = prefs.lineHeight != null ? prefs.lineHeight : 1.6;
      lhSlider.value = lh;
      document.getElementById('line-height-val').textContent = parseFloat(lh).toFixed(1);
    }
    var compactCb = document.getElementById('pref-compact');
    if (compactCb) compactCb.checked = !!prefs.compact;
    var ssSlider = document.getElementById('pref-spacing-scale');
    if (ssSlider) {
      var ss = prefs.spacingScale != null ? prefs.spacingScale : 100;
      ssSlider.value = ss;
      document.getElementById('spacing-scale-val').textContent = ss + '%';
    }
    if (prefs.successColor) document.getElementById('pref-success-color').value = prefs.successColor;
    if (prefs.dangerColor) document.getElementById('pref-danger-color').value = prefs.dangerColor;
    if (prefs.warningColor) document.getElementById('pref-warning-color').value = prefs.warningColor;
  } catch(e) {}
  // Update accent hex display
  var accentEl = document.getElementById('pref-accent');
  var accentHex = document.getElementById('accent-hex');
  if (accentEl && accentHex) accentHex.textContent = accentEl.value;
  renderIconPreview();
}

function showSection(id) {
  document.querySelectorAll('.settings-section').forEach(el => el.classList.remove('active'));
  document.querySelectorAll('.s-nav').forEach(el => el.classList.toggle('active', el.dataset.sec === id));
  const sec = document.getElementById('sec-' + id);
  if (sec) sec.classList.add('active');
}

// ── Theme Customizer live updates ──
function applyCustomizerLive() {
  var doc = document.documentElement;
  // Icon weight
  var iw = parseFloat(document.getElementById('pref-icon-weight').value);
  document.getElementById('icon-weight-val').textContent = iw;
  doc.style.setProperty('--icon-weight', iw);
  if (window.hosSetIconWeight) window.hosSetIconWeight(iw);
  // Icon size
  var is = parseInt(document.getElementById('pref-icon-size').value);
  document.getElementById('icon-size-val').textContent = is + 'px';
  doc.style.setProperty('--icon-size', is + 'px');
  // Font size
  var fs = parseInt(document.getElementById('pref-font-size-slider').value);
  document.getElementById('font-size-val').textContent = fs + 'px';
  doc.style.setProperty('--font-size-base', fs + 'px');
  doc.style.setProperty('font-size', fs + 'px');
  // Border radius — update all scale levels proportionally
  var br = parseInt(document.getElementById('pref-border-radius').value);
  document.getElementById('border-radius-val').textContent = br + 'px';
  doc.style.setProperty('--radius', br + 'px');
  doc.style.setProperty('--radius-sm', Math.max(1, Math.round(br * 0.5)) + 'px');
  doc.style.setProperty('--radius-lg', Math.round(br * 1.5) + 'px');
  // Content width
  var cw = parseInt(document.getElementById('pref-content-width').value);
  document.getElementById('content-width-val').textContent = cw === 0 ? 'Full' : cw + 'px';
  doc.style.setProperty('--content-width', cw === 0 ? 'none' : cw + 'px');
  // Line height
  var lh = parseFloat(document.getElementById('pref-line-height').value);
  document.getElementById('line-height-val').textContent = lh.toFixed(1);
  doc.style.setProperty('--line-height', lh);
  doc.style.setProperty('line-height', lh);
  // UI Spacing scale
  var ss = parseInt(document.getElementById('pref-spacing-scale').value);
  document.getElementById('spacing-scale-val').textContent = ss + '%';
  var scale = ss / 100;
  doc.style.setProperty('--space-xs', (0.15 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-sm', (0.3 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-md', (0.5 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-lg', (0.75 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-xl', (1.0 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-2xl', (1.5 * scale).toFixed(3) + 'rem');
  doc.style.setProperty('--space-3xl', (2.0 * scale).toFixed(3) + 'rem');
  // Compact mode
  var compact = document.getElementById('pref-compact').checked;
  if (compact) doc.setAttribute('data-compact', '');
  else doc.removeAttribute('data-compact');
  // Semantic colors
  var sc = document.getElementById('pref-success-color').value;
  var dc = document.getElementById('pref-danger-color').value;
  var wc = document.getElementById('pref-warning-color').value;
  doc.style.setProperty('--success', sc);
  doc.style.setProperty('--danger', dc);
  doc.style.setProperty('--warning', wc);
  renderIconPreview();
  saveCustomizer();
}

function saveCustomizer() {
  // Persist customizer values into the unified prefs store
  prefs.iconWeight = parseFloat(document.getElementById('pref-icon-weight').value);
  prefs.iconSize = parseInt(document.getElementById('pref-icon-size').value);
  prefs.fontSizePx = parseInt(document.getElementById('pref-font-size-slider').value);
  prefs.borderRadius = parseInt(document.getElementById('pref-border-radius').value);
  prefs.contentWidth = parseInt(document.getElementById('pref-content-width').value);
  prefs.lineHeight = parseFloat(document.getElementById('pref-line-height').value);
  prefs.compact = document.getElementById('pref-compact').checked;
  prefs.spacingScale = parseInt(document.getElementById('pref-spacing-scale').value);
  prefs.successColor = document.getElementById('pref-success-color').value;
  prefs.dangerColor = document.getElementById('pref-danger-color').value;
  prefs.warningColor = document.getElementById('pref-warning-color').value;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
}

function resetColor(id, defaultVal) {
  var el = document.getElementById('pref-' + id);
  if (el) { el.value = defaultVal; applyCustomizerLive(); }
}

function resetThemeDefaults() {
  document.getElementById('pref-icon-weight').value = 3;
  document.getElementById('pref-icon-size').value = 20;
  document.getElementById('pref-font-size-slider').value = 16;
  document.getElementById('pref-border-radius').value = 8;
  document.getElementById('pref-content-width').value = 0;
  document.getElementById('pref-line-height').value = 1.6;
  document.getElementById('pref-spacing-scale').value = 100;
  document.getElementById('pref-success-color').value = '#44aa99';
  document.getElementById('pref-danger-color').value = '#cc4444';
  document.getElementById('pref-warning-color').value = '#ccaa33';
  document.getElementById('pref-accent').value = '#FF8811';
  applyCustomizerLive();
  savePref();
}

function renderIconPreview() {
  var el = document.getElementById('icon-weight-preview');
  if (!el || !window.hosIcon) return;
  var sz = parseInt(document.getElementById('pref-icon-size').value) || 20;
  var names = ['home', 'chat', 'settings', 'lock', 'bell', 'search', 'star', 'user'];
  el.innerHTML = names.map(function(n) { return hosIcon(n, sz); }).join('');
  // Update top preview strip
  var tp = document.getElementById('tp-icons');
  if (tp) tp.innerHTML = ['network','games','profile','tasklist','calendar','map'].map(function(n){ return hosIcon(n, sz); }).join('');
}

function showSaved() {
  const btn = document.getElementById('settings-save');
  var saveIcon = window.hosIcon ? hosIcon('check', 16) : '✓';
  btn.innerHTML = saveIcon + ' Saved!';
  btn.style.background = '#1a5a3a';
  setTimeout(() => {
    var icon = window.hosIcon ? hosIcon('save', 16) : '';
    btn.innerHTML = icon + ' Save Settings';
    btn.style.background = '';
  }, 1800);
  savePref();
}

async function clearAppCache() {
  // Wipe all service-worker caches and force a full network reload.
  // Fixes stale JS/HTML after a deploy — same as Ctrl+Shift+Delete.
  try {
    const names = await caches.keys();
    await Promise.all(names.map(n => caches.delete(n)));
    const regs = await navigator.serviceWorker.getRegistrations();
    await Promise.all(regs.map(r => r.unregister()));
  } catch (_) {}
  location.reload(true);
}

function loadVoicePrefs() {
  const mode = localStorage.getItem('humanity-vc-input-mode') || 'open';
  const keyCode = localStorage.getItem('humanity-vc-ptt-key') || 'KeyV';
  const sel = document.getElementById('vc-input-mode-select');
  if (sel) sel.value = mode;
  updatePttKeyDisplay(keyCode);
  togglePttKeyRow(mode);
}

function saveVoicePref() {
  const sel = document.getElementById('vc-input-mode-select');
  if (!sel) return;
  const mode = sel.value;
  localStorage.setItem('humanity-vc-input-mode', mode);
  // Sync to live voice module if open
  if (typeof window.setVcInputMode === 'function') window.setVcInputMode(mode);
  togglePttKeyRow(mode);
}

function togglePttKeyRow(mode) {
  const row = document.getElementById('ptt-key-row');
  if (row) row.style.display = mode === 'ptt' ? '' : 'none';
}

function updatePttKeyDisplay(code) {
  const disp = document.getElementById('ptt-key-display');
  if (disp) disp.textContent = (code || 'KeyV').replace(/^Key/, '').replace(/^Digit/, '');
}

function beginPttRebind() {
  const btn = document.getElementById('ptt-rebind-btn');
  const disp = document.getElementById('ptt-key-display');
  if (btn) { btn.textContent = 'Press a key…'; btn.disabled = true; }
  if (disp) disp.textContent = '?';
  function capture(e) {
    if (['Escape','Tab'].includes(e.key)) {
      document.removeEventListener('keydown', capture, true);
      loadVoicePrefs();
      if (btn) { btn.textContent = 'Rebind'; btn.disabled = false; }
      return;
    }
    e.preventDefault(); e.stopPropagation();
    document.removeEventListener('keydown', capture, true);
    localStorage.setItem('humanity-vc-ptt-key', e.code);
    if (typeof window.setVcInputMode === 'function') {
      // Trigger reload of key in the voice module via a temporary hack
      // (the module reads the key on next keydown — just update display here)
    }
    updatePttKeyDisplay(e.code);
    if (btn) { btn.textContent = 'Rebind'; btn.disabled = false; }
  }
  document.addEventListener('keydown', capture, true);
}

function clearAllData() {
  if (!confirm('This will permanently delete all local HumanityOS data (skills, inventory, logbook, quests, calendar). Your chat identity is preserved.\n\nThis cannot be undone. Continue?')) return;
  const keys = ['hos_skills_v1','hos_inventory_v1','hos_logbook_v1','hos_quests_v1','hos_calendar_v1'];
  keys.forEach(k => localStorage.removeItem(k));
  alert('Local data cleared.');
}

function resetSettings() {
  if (!confirm('Reset all settings to defaults?')) return;
  prefs = Object.assign({}, DEFAULTS);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  applyPrefs();
}

function exportData() {
  const keys = [
    'hos_skills_v1','hos_inventory_v1','hos_logbook_v1','hos_quests_v1',
    'hos_calendar_v1','hos_homes_v2','hos_home_todos','hos_home_notes',
    'hos_notes_v1','hos_equipment_v1','humanity_settings','hos_identity','hos_profile_v1',
    'map_pins_v1','map_polygons_v1','map_home_location',
    'humanity_name','hos_dm_recent',
  ];
  const out = {};
  keys.forEach(k => {
    try { out[k] = JSON.parse(localStorage.getItem(k)); } catch(e) {}
  });
  const blob = new Blob([JSON.stringify(out, null, 2)], {type: 'application/json'});
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'humanityos-backup-' + new Date().toISOString().slice(0,10) + '.json';
  a.click();
}

function importData(e) {
  const file = e.target.files[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = function(ev) {
    try {
      const data = JSON.parse(ev.target.result);
      if (!confirm('Import this backup? It will overwrite matching local data.')) return;
      Object.keys(data).forEach(k => {
        if (data[k] !== null) localStorage.setItem(k, JSON.stringify(data[k]));
      });
      alert('Data imported successfully.');
      loadPrefs(); applyPrefs();
    } catch(err) { alert('Failed to parse backup file.'); }
  };
  reader.readAsText(file);
}

// ── Keyboard Shortcuts ──
const KEYBIND_STORAGE = 'hos_keybinds_v1';
const KEYBIND_DEFS = [
  { group: 'Navigation', binds: [
    { id: 'nav-network',    label: 'Go to Network',    default: '' },
    { id: 'nav-dashboard',  label: 'Go to Dashboard',  default: '' },
    { id: 'nav-profile',    label: 'Go to Profile',    default: '' },
    { id: 'nav-home',       label: 'Go to Home',       default: '' },
    { id: 'nav-skills',     label: 'Go to Skills',     default: '' },
    { id: 'nav-inventory',  label: 'Go to Inventory',  default: '' },
    { id: 'nav-equipment',  label: 'Go to Equipment',  default: '' },
    { id: 'nav-quests',     label: 'Go to Quests',     default: '' },
    { id: 'nav-calendar',   label: 'Go to Calendar',   default: '' },
    { id: 'nav-logbook',    label: 'Go to Logbook',    default: '' },
    { id: 'nav-notes',      label: 'Go to Notes',      default: '' },
    { id: 'nav-vault',      label: 'Go to Vault',      default: '' },
    { id: 'nav-tasks',      label: 'Go to Tasks',      default: '' },
    { id: 'nav-market',     label: 'Go to Market',     default: '' },
    { id: 'nav-maps',       label: 'Go to Maps',       default: '' },
    { id: 'nav-settings',   label: 'Go to Settings',   default: '' },
  ]},
  { group: 'Chat', binds: [
    { id: 'chat-search',     label: 'Open Search',      default: 'Ctrl+K' },
    { id: 'chat-command',    label: 'Command Palette',   default: 'Ctrl+Shift+P' },
    { id: 'chat-focus-input',label: 'Focus Chat Input',  default: '' },
    { id: 'chat-upload',     label: 'Upload File',       default: '' },
    { id: 'chat-emoji',      label: 'Open Emoji Picker', default: '' },
  ]},
  { group: 'Voice', binds: [
    { id: 'voice-mute',      label: 'Toggle Mute',       default: 'Ctrl+Shift+M' },
    { id: 'voice-deafen',    label: 'Toggle Deafen',     default: 'Ctrl+Shift+D' },
    { id: 'voice-ptt',       label: 'Push to Talk',      default: 'KeyV' },
    { id: 'voice-camera',    label: 'Toggle Camera',     default: '' },
    { id: 'voice-screen',    label: 'Toggle Screen Share',default: '' },
    { id: 'voice-leave',     label: 'Leave Voice',       default: '' },
  ]},
  { group: 'Media', binds: [
    { id: 'media-afk',       label: 'Toggle AFK',        default: '' },
    { id: 'media-brb',       label: 'Toggle BRB',        default: '' },
    { id: 'media-pip',       label: 'Picture in Picture', default: '' },
  ]},
  { group: 'App', binds: [
    { id: 'app-sidebar',     label: 'Toggle Sidebar',    default: '' },
    { id: 'app-clear-cache', label: 'Clear Cache',       default: 'Ctrl+Shift+Delete' },
    { id: 'app-theme',       label: 'Toggle Theme',      default: '' },
    { id: 'app-fullscreen',  label: 'Toggle Fullscreen', default: 'F11' },
  ]},
];

let keybinds = {};

function loadKeybinds() {
  try { keybinds = JSON.parse(localStorage.getItem(KEYBIND_STORAGE)) || {}; } catch { keybinds = {}; }
}

function saveKeybinds() {
  localStorage.setItem(KEYBIND_STORAGE, JSON.stringify(keybinds));
}

function getKeybind(id, def) { return keybinds[id] !== undefined ? keybinds[id] : def; }

function formatKey(code) {
  if (!code) return 'Not set';
  return code.replace(/^Key/, '').replace(/^Digit/, '').replace(/Control/,'Ctrl');
}

function renderKeybinds() {
  const container = document.getElementById('keybind-groups');
  if (!container) return;
  container.innerHTML = KEYBIND_DEFS.map(g => `
    <div class="settings-group">
      <div class="settings-group-title">${g.group}</div>
      ${g.binds.map(b => {
        const val = getKeybind(b.id, b.default);
        return `<div class="setting-row">
          <div class="setting-label">
            <strong>${b.label}</strong>
          </div>
          <div style="display:flex;align-items:center;gap:var(--space-md);">
            <kbd id="kb-${b.id}" style="background:#111;border:1px solid #46f;color:#aaf;padding:0.22rem var(--space-lg);border-radius:5px;font-family:monospace;font-size:0.82rem;min-width:60px;text-align:center;cursor:pointer;"
              onclick="captureKeybind('${b.id}')">${formatKey(val)}</kbd>
            <button onclick="clearKeybind('${b.id}')" style="background:transparent;border:1px solid #555;color:#888;padding:var(--space-xs) var(--space-md);border-radius:4px;cursor:pointer;font-size:0.7rem;" title="Clear">✕</button>
          </div>
        </div>`;
      }).join('')}
    </div>
  `).join('');
}

function captureKeybind(id) {
  const kbd = document.getElementById('kb-' + id);
  if (!kbd) return;
  kbd.textContent = 'Press keys…';
  kbd.style.borderColor = '#f80';
  function capture(e) {
    if (['Escape','Tab'].includes(e.key)) {
      document.removeEventListener('keydown', capture, true);
      renderKeybinds();
      return;
    }
    e.preventDefault(); e.stopPropagation();
    document.removeEventListener('keydown', capture, true);
    let combo = '';
    if (e.ctrlKey) combo += 'Ctrl+';
    if (e.altKey) combo += 'Alt+';
    if (e.shiftKey) combo += 'Shift+';
    if (e.metaKey) combo += 'Meta+';
    if (!['Control','Alt','Shift','Meta'].includes(e.key)) combo += e.code;
    keybinds[id] = combo;
    saveKeybinds();
    renderKeybinds();
  }
  document.addEventListener('keydown', capture, true);
}

function clearKeybind(id) {
  keybinds[id] = '';
  saveKeybinds();
  renderKeybinds();
}

// ── Audio & Video ──
let micTestStream = null;
let micTestAnalyser = null;
let micTestRaf = null;

async function populateDeviceLists() {
  try {
    await navigator.mediaDevices.getUserMedia({ audio: true, video: true }).then(s => s.getTracks().forEach(t => t.stop()));
  } catch {}
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const micSel = document.getElementById('pref-mic-device');
    const spkSel = document.getElementById('pref-speaker-device');
    const camSel = document.getElementById('pref-camera-device');
    if (micSel) {
      const mics = devices.filter(d => d.kind === 'audioinput');
      micSel.innerHTML = '<option value="">System Default</option>' + mics.map(d =>
        `<option value="${d.deviceId}">${d.label || 'Microphone ' + d.deviceId.slice(0,6)}</option>`
      ).join('');
      micSel.value = prefs['mic-device'] || '';
    }
    if (spkSel) {
      const spks = devices.filter(d => d.kind === 'audiooutput');
      spkSel.innerHTML = '<option value="">System Default</option>' + spks.map(d =>
        `<option value="${d.deviceId}">${d.label || 'Speaker ' + d.deviceId.slice(0,6)}</option>`
      ).join('');
      spkSel.value = prefs['speaker-device'] || '';
    }
    if (camSel) {
      const cams = devices.filter(d => d.kind === 'videoinput');
      camSel.innerHTML = '<option value="">System Default</option>' + cams.map(d =>
        `<option value="${d.deviceId}">${d.label || 'Camera ' + d.deviceId.slice(0,6)}</option>`
      ).join('');
      camSel.value = prefs['camera-device'] || '';
    }
  } catch {}
}

async function testMic() {
  const btn = document.getElementById('mic-test-btn');
  if (micTestStream) {
    micTestStream.getTracks().forEach(t => t.stop());
    micTestStream = null;
    if (micTestRaf) cancelAnimationFrame(micTestRaf);
    document.getElementById('mic-level-bar').style.width = '0%';
    if (btn) btn.textContent = 'Test Mic';
    return;
  }
  try {
    const deviceId = prefs['mic-device'] || undefined;
    micTestStream = await navigator.mediaDevices.getUserMedia({
      audio: deviceId ? { deviceId: { exact: deviceId } } : true
    });
    const ctx = new AudioContext();
    const src = ctx.createMediaStreamSource(micTestStream);
    micTestAnalyser = ctx.createAnalyser();
    micTestAnalyser.fftSize = 256;
    src.connect(micTestAnalyser);
    const data = new Uint8Array(micTestAnalyser.frequencyBinCount);
    if (btn) btn.textContent = 'Stop Test';
    function draw() {
      if (!micTestStream) return;
      micTestAnalyser.getByteFrequencyData(data);
      const avg = data.reduce((a, b) => a + b, 0) / data.length;
      document.getElementById('mic-level-bar').style.width = Math.min(100, avg * 1.5) + '%';
      micTestRaf = requestAnimationFrame(draw);
    }
    draw();
  } catch (e) {
    alert('Could not access microphone: ' + e.message);
  }
}

let camPreviewStream = null;
async function toggleCameraPreview() {
  const video = document.getElementById('settings-cam-preview');
  const btn = document.getElementById('cam-preview-btn');
  if (camPreviewStream) {
    camPreviewStream.getTracks().forEach(t => t.stop());
    camPreviewStream = null;
    video.style.display = 'none';
    if (btn) btn.textContent = 'Preview';
    return;
  }
  try {
    const deviceId = prefs['camera-device'] || undefined;
    const quality = parseInt(prefs['video-quality'] || '720');
    camPreviewStream = await navigator.mediaDevices.getUserMedia({
      video: { deviceId: deviceId ? { exact: deviceId } : undefined, height: { ideal: quality } }
    });
    video.srcObject = camPreviewStream;
    video.style.display = 'block';
    if (btn) btn.textContent = 'Stop';
  } catch (e) {
    alert('Could not access camera: ' + e.message);
  }
}

// ── Security ──
function checkKeyProtection() {
  const el = document.getElementById('key-protection-status');
  if (!el) return;
  const hasProtected = !!localStorage.getItem('hos_key_protected');
  el.textContent = hasProtected ? '🟢 Protected' : '🔴 Not protected';
  el.style.color = hasProtected ? '#4a9' : '#e44';
}

function checkBackupStatus() {
  // Check signals that user has backed up their identity
  var hasPassphrase = !!localStorage.getItem('hos_key_protected');
  var hasVault = !!localStorage.getItem('hos_vault_v1');
  var hasSeedDismissed = !!localStorage.getItem('hos_vault_seed_nudge_dismissed');
  var hasOnboarded = !!localStorage.getItem('humanity_onboarding_done');
  // If they completed onboarding AND have passphrase protection or vault, assume backed up
  var isBackedUp = hasPassphrase || hasVault || hasSeedDismissed;
  var warn = document.getElementById('backup-warning');
  var ok = document.getElementById('backup-ok');
  if (warn) warn.style.display = isBackedUp ? 'none' : 'block';
  if (ok) ok.style.display = isBackedUp ? 'block' : 'none';
}

// ── Storage ──
function calculateStorage() {
  const container = document.getElementById('storage-breakdown');
  if (!container) return;
  const modules = [
    { key: 'humanity_settings', label: 'Settings' },
    { key: 'hos_identity', label: 'Identity' },
    { key: 'hos_profile_v1', label: 'Profile' },
    { key: 'hos_notes_v1', label: 'Notes' },
    { key: 'hos_inventory_v1', label: 'Inventory' },
    { key: 'hos_logbook_v1', label: 'Logbook' },
    { key: 'hos_skills_v1', label: 'Skills' },
    { key: 'hos_quests_v1', label: 'Quests' },
    { key: 'hos_calendar_v1', label: 'Calendar' },
    { key: 'hos_equipment_v1', label: 'Equipment' },
    { key: 'hos_homes_v2', label: 'Homes' },
    { key: 'hos_keybinds_v1', label: 'Keybinds' },
    { key: 'hos_system_profile', label: 'System Profile' },
    { key: 'hos_dm_recent', label: 'DM History' },
  ];
  let totalBytes = 0;
  const rows = modules.map(m => {
    const val = localStorage.getItem(m.key);
    const bytes = val ? new Blob([val]).size : 0;
    totalBytes += bytes;
    const sizeStr = bytes < 1024 ? bytes + ' B' : (bytes / 1024).toFixed(1) + ' KB';
    return `<div style="display:flex;justify-content:space-between;padding:var(--space-sm) 0;border-bottom:1px solid #181818;font-size:0.82rem;">
      <span style="color:#aaa">${m.label}</span>
      <span style="color:#666;font-family:monospace;">${sizeStr}</span>
    </div>`;
  });
  const totalStr = totalBytes < 1024 ? totalBytes + ' B' : (totalBytes / 1024).toFixed(1) + ' KB';
  rows.push(`<div style="display:flex;justify-content:space-between;padding:var(--space-md) 0 0;font-size:0.85rem;font-weight:700;">
    <span style="color:#ddd">Total</span>
    <span style="color:#2a6;font-family:monospace;">${totalStr}</span>
  </div>`);
  container.innerHTML = rows.join('');
}

function clearModule(key, name) {
  if (!confirm('Clear all ' + name + ' data? This cannot be undone.')) return;
  localStorage.removeItem(key);
  calculateStorage();
}

// ── Gain/volume label updates ──
function updateRangeLabels() {
  const gainEl = document.getElementById('pref-mic-gain');
  const gainLabel = document.getElementById('mic-gain-label');
  if (gainEl && gainLabel) gainLabel.textContent = gainEl.value + '%';
  const volEl = document.getElementById('pref-speaker-vol');
  const volLabel = document.getElementById('speaker-vol-label');
  if (volEl && volLabel) volLabel.textContent = volEl.value + '%';
}

// Patch savePref to also update range labels
const _origSavePref = savePref;
savePref = function() { _origSavePref(); updateRangeLabels(); };

// Version tag
try {
  const vEl = document.getElementById('version-tag');
  if (vEl) vEl.textContent = 'HumanityOS — v0.90.4 · ' + new Date().getFullYear();
} catch(e) {}

// Inject hosIcon SVGs into action bar buttons
if (window.hosIcon) {
  document.getElementById('save-icon').innerHTML = hosIcon('save', 16);
  document.getElementById('reset-icon').innerHTML = hosIcon('refresh', 14);
} else {
  document.getElementById('settings-save').textContent = 'Save Settings';
}

// ── Migration: merge old hos_settings_v1 into humanity_settings ──
(function migrateOldSettingsStore() {
  try {
    var old = localStorage.getItem('hos_settings_v1');
    if (!old) return; // nothing to migrate
    var oldData = JSON.parse(old);
    var current = JSON.parse(localStorage.getItem('humanity_settings') || '{}');
    // Merge old keys into current — old values fill gaps, don't overwrite existing
    Object.keys(oldData).forEach(function(k) {
      if (current[k] === undefined) current[k] = oldData[k];
    });
    localStorage.setItem('humanity_settings', JSON.stringify(current));
    localStorage.removeItem('hos_settings_v1');
  } catch(e) {}
})();

// ── Push Notification Preferences ──

/** Populate DND hour dropdowns with 12-hour labels */
function initDndDropdowns() {
  var startSel = document.getElementById('pref-dnd-start');
  var endSel = document.getElementById('pref-dnd-end');
  if (!startSel || !endSel) return;
  var hours = [];
  for (var h = 0; h < 24; h++) {
    var label = h === 0 ? '12 AM' : h < 12 ? h + ' AM' : h === 12 ? '12 PM' : (h - 12) + ' PM';
    hours.push('<option value="' + h + '">' + label + '</option>');
  }
  startSel.innerHTML = hours.join('');
  endSel.innerHTML = hours.join('');
}

/** Read push prefs from localStorage and apply to UI */
function loadPushPrefs() {
  var enabled = localStorage.getItem('hos_notify_enabled') === 'true';
  var dms = localStorage.getItem('hos_notify_dms') !== 'false'; // default true
  var mentions = localStorage.getItem('hos_notify_mentions') !== 'false';
  var tasks = localStorage.getItem('hos_notify_tasks') !== 'false';
  var dndEnabled = localStorage.getItem('hos_dnd_start') !== null && localStorage.getItem('hos_dnd_end') !== null;
  var dndStart = parseInt(localStorage.getItem('hos_dnd_start')) || 22;
  var dndEnd = parseInt(localStorage.getItem('hos_dnd_end')) || 7;

  var elEnabled = document.getElementById('pref-push-enabled');
  var elDms = document.getElementById('pref-push-dms');
  var elMentions = document.getElementById('pref-push-mentions');
  var elTasks = document.getElementById('pref-push-tasks');
  var elDnd = document.getElementById('pref-push-dnd');
  var elDndStart = document.getElementById('pref-dnd-start');
  var elDndEnd = document.getElementById('pref-dnd-end');

  if (elEnabled) elEnabled.checked = enabled;
  if (elDms) elDms.checked = dms;
  if (elMentions) elMentions.checked = mentions;
  if (elTasks) elTasks.checked = tasks;
  if (elDnd) elDnd.checked = dndEnabled;
  if (elDndStart) elDndStart.value = dndStart;
  if (elDndEnd) elDndEnd.value = dndEnd;

  updatePushSubToggles(enabled);
  updateDndTimes(dndEnabled);
}

/** Enable/disable sub-toggles based on master toggle state */
function updatePushSubToggles(enabled) {
  var container = document.getElementById('push-sub-toggles');
  if (!container) return;
  var inputs = container.querySelectorAll('input[type="checkbox"]');
  inputs.forEach(function(inp) { inp.disabled = !enabled; });
  container.style.opacity = enabled ? '1' : '0.4';
  container.style.pointerEvents = enabled ? 'auto' : 'none';
}

/** Show/hide DND time selectors */
function updateDndTimes(enabled) {
  var container = document.getElementById('push-dnd-times');
  if (!container) return;
  container.style.opacity = enabled ? '1' : '0.4';
  container.style.pointerEvents = enabled ? 'auto' : 'none';
}

/** Master push toggle — subscribe or unsubscribe */
async function togglePushNotifications(enabled) {
  var statusEl = document.getElementById('push-status-msg');
  localStorage.setItem('hos_notify_enabled', enabled ? 'true' : 'false');
  updatePushSubToggles(enabled);

  if (enabled) {
    // Request notification permission
    if ('Notification' in window && Notification.permission === 'default') {
      var perm = await Notification.requestPermission();
      if (perm !== 'granted') {
        localStorage.setItem('hos_notify_enabled', 'false');
        var el = document.getElementById('pref-push-enabled');
        if (el) el.checked = false;
        updatePushSubToggles(false);
        if (statusEl) {
          statusEl.style.display = 'block';
          statusEl.textContent = 'Notification permission denied. Enable in browser settings.';
          statusEl.style.color = 'var(--danger)';
        }
        return;
      }
    }
    if ('Notification' in window && Notification.permission === 'denied') {
      localStorage.setItem('hos_notify_enabled', 'false');
      var el2 = document.getElementById('pref-push-enabled');
      if (el2) el2.checked = false;
      updatePushSubToggles(false);
      if (statusEl) {
        statusEl.style.display = 'block';
        statusEl.textContent = 'Notifications blocked by browser. Check site permissions.';
        statusEl.style.color = 'var(--danger)';
      }
      return;
    }

    // Try subscribing via chat-ui.js helper or direct push manager
    if (typeof window.subscribeToPush === 'function') {
      try {
        await window.subscribeToPush();
        if (statusEl) {
          statusEl.style.display = 'block';
          statusEl.textContent = 'Push notifications enabled.';
          statusEl.style.color = 'var(--success)';
        }
      } catch (e) {
        console.warn('Push subscribe failed:', e);
        if (statusEl) {
          statusEl.style.display = 'block';
          statusEl.textContent = 'Could not subscribe: ' + (e.message || e);
          statusEl.style.color = 'var(--warning)';
        }
      }
    } else {
      if (statusEl) {
        statusEl.style.display = 'block';
        statusEl.textContent = 'Push enabled. Open chat to complete subscription.';
        statusEl.style.color = 'var(--text-muted)';
      }
    }
  } else {
    // Unsubscribe
    try {
      if ('serviceWorker' in navigator) {
        var reg = await navigator.serviceWorker.ready;
        var sub = await reg.pushManager.getSubscription();
        if (sub) {
          await sub.unsubscribe();
          // Notify server
          fetch('/api/push/unsubscribe', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ endpoint: sub.endpoint })
          }).catch(function() {});
        }
      }
    } catch (e) {
      console.warn('Push unsubscribe error:', e);
    }
    if (statusEl) {
      statusEl.style.display = 'block';
      statusEl.textContent = 'Push notifications disabled.';
      statusEl.style.color = 'var(--text-muted)';
    }
  }
}

/** Save category preferences to localStorage and sync to server */
function savePushPref() {
  var elDms = document.getElementById('pref-push-dms');
  var elMentions = document.getElementById('pref-push-mentions');
  var elTasks = document.getElementById('pref-push-tasks');
  if (elDms) localStorage.setItem('hos_notify_dms', elDms.checked ? 'true' : 'false');
  if (elMentions) localStorage.setItem('hos_notify_mentions', elMentions.checked ? 'true' : 'false');
  if (elTasks) localStorage.setItem('hos_notify_tasks', elTasks.checked ? 'true' : 'false');
  syncNotifPrefsToServer();
}

/** Save DND schedule to localStorage and sync to server */
function savePushDnd() {
  var elDnd = document.getElementById('pref-push-dnd');
  var elStart = document.getElementById('pref-dnd-start');
  var elEnd = document.getElementById('pref-dnd-end');
  var enabled = elDnd && elDnd.checked;
  updateDndTimes(enabled);

  if (enabled) {
    localStorage.setItem('hos_dnd_start', elStart ? elStart.value : '22');
    localStorage.setItem('hos_dnd_end', elEnd ? elEnd.value : '7');
  } else {
    localStorage.removeItem('hos_dnd_start');
    localStorage.removeItem('hos_dnd_end');
  }
  syncNotifPrefsToServer();
}

/** Send current notification preferences to the server via WebSocket */
function syncNotifPrefsToServer() {
  var ws = window._humanityWs;
  if (!ws || ws.readyState !== 1) return;
  var dm = localStorage.getItem('hos_notify_dms') !== 'false';
  var mentions = localStorage.getItem('hos_notify_mentions') !== 'false';
  var tasks = localStorage.getItem('hos_notify_tasks') !== 'false';
  var dndStart = localStorage.getItem('hos_dnd_start');
  var dndEnd = localStorage.getItem('hos_dnd_end');
  var msg = {
    type: 'update_notification_prefs',
    dm: dm,
    mentions: mentions,
    tasks: tasks
  };
  if (dndStart !== null && dndEnd !== null) {
    msg.dnd_start = dndStart;
    msg.dnd_end = dndEnd;
  }
  ws.send(JSON.stringify(msg));
}

/** Request notification preferences from the server */
function requestNotifPrefsFromServer() {
  var ws = window._humanityWs;
  if (!ws || ws.readyState !== 1) return;
  ws.send(JSON.stringify({ type: 'get_notification_prefs' }));
}

/** Handle incoming notification_prefs_data from server — update UI and localStorage */
function handleNotifPrefsData(msg) {
  if (msg.dm !== undefined) localStorage.setItem('hos_notify_dms', msg.dm ? 'true' : 'false');
  if (msg.mentions !== undefined) localStorage.setItem('hos_notify_mentions', msg.mentions ? 'true' : 'false');
  if (msg.tasks !== undefined) localStorage.setItem('hos_notify_tasks', msg.tasks ? 'true' : 'false');
  if (msg.dnd_start !== undefined && msg.dnd_start !== null) {
    localStorage.setItem('hos_dnd_start', msg.dnd_start);
  }
  if (msg.dnd_end !== undefined && msg.dnd_end !== null) {
    localStorage.setItem('hos_dnd_end', msg.dnd_end);
  }
  if (msg.dnd_start === null && msg.dnd_end === null) {
    localStorage.removeItem('hos_dnd_start');
    localStorage.removeItem('hos_dnd_end');
  }
  loadPushPrefs();
}

// Listen for notification_prefs_data from the server via global WS
(function() {
  function hookWs(ws) {
    if (!ws || ws._notifPrefsHooked) return;
    ws._notifPrefsHooked = true;
    ws.addEventListener('message', function(e) {
      try {
        var msg = JSON.parse(e.data);
        if (msg.type === 'notification_prefs_data') handleNotifPrefsData(msg);
      } catch(ex) {}
    });
  }
  // Hook the existing WS if available
  if (window._humanityWs) hookWs(window._humanityWs);
  // Watch for new WS connections (set by chat or market pages)
  var origDescriptor = Object.getOwnPropertyDescriptor(window, '_humanityWs');
  if (!origDescriptor || !origDescriptor.set) {
    var _wsVal = window._humanityWs;
    Object.defineProperty(window, '_humanityWs', {
      get: function() { return _wsVal; },
      set: function(v) { _wsVal = v; hookWs(v); },
      configurable: true
    });
  }
  // Request prefs from server after a brief delay to let WS connect
  setTimeout(requestNotifPrefsFromServer, 1500);
})();

loadPrefs();
applyPrefs();
loadVoicePrefs();
loadKeybinds();
renderKeybinds();
populateDeviceLists();
checkKeyProtection();
checkBackupStatus();
calculateStorage();
updateRangeLabels();
initDndDropdowns();
loadPushPrefs();

// ══════════════════════════════════════════════════════════════════════════════
// ── Wallet Settings ──
// ══════════════════════════════════════════════════════════════════════════════

/** Initialize wallet settings from localStorage and identity */
(function initWalletSettings() {
  // Load saved network preference
  var network = localStorage.getItem('hos_solana_network') || 'mainnet';
  var networkEl = document.getElementById('wallet-network');
  if (networkEl) networkEl.value = network;

  // Show/hide custom RPC row
  var customRow = document.getElementById('wallet-custom-rpc-row');
  if (customRow) customRow.style.display = network === 'custom' ? '' : 'none';

  // Load custom RPC URL
  var rpcEl = document.getElementById('wallet-custom-rpc');
  if (rpcEl) rpcEl.value = localStorage.getItem('hos_solana_rpc') || '';

  // Load nav balance toggle
  var navBalEl = document.getElementById('wallet-show-nav-balance');
  if (navBalEl) navBalEl.checked = localStorage.getItem('hos_wallet_show_nav_balance') === 'true';

  // Derive and display Solana address from identity
  walletDisplayAddress();
})();

/** Derive Solana address from identity public key and display it */
function walletDisplayAddress() {
  var addrEl = document.getElementById('wallet-sol-address');
  var copyBtn = document.getElementById('wallet-copy-btn');
  if (!addrEl) return;

  if (window.HosWallet && window.myIdentity && myIdentity.publicKeyHex) {
    var fullAddr = HosWallet.publicKeyToSolanaAddress(myIdentity.publicKeyHex);
    var shortAddr = fullAddr.length > 8 ? fullAddr.substring(0, 4) + '...' + fullAddr.substring(fullAddr.length - 4) : fullAddr;
    addrEl.textContent = shortAddr;
    addrEl.title = fullAddr;
    if (copyBtn) {
      copyBtn.addEventListener('click', function() {
        navigator.clipboard.writeText(fullAddr).then(function() {
          copyBtn.textContent = 'Copied!';
          setTimeout(function() { copyBtn.textContent = 'Copy'; }, 2000);
        });
      });
    }
  } else if (window.myIdentity && myIdentity.publicKeyHex && typeof hexToBuf === 'function') {
    // Fallback: show hex public key if wallet.js not loaded
    var pk = myIdentity.publicKeyHex;
    var shortPk = pk.length > 8 ? pk.substring(0, 4) + '...' + pk.substring(pk.length - 4) : pk;
    addrEl.textContent = shortPk;
    addrEl.title = pk + ' (wallet.js not loaded — install to see Solana address)';
    if (copyBtn) {
      copyBtn.addEventListener('click', function() {
        navigator.clipboard.writeText(pk).then(function() {
          copyBtn.textContent = 'Copied!';
          setTimeout(function() { copyBtn.textContent = 'Copy'; }, 2000);
        });
      });
    }
  } else {
    addrEl.textContent = 'No identity loaded';
    addrEl.title = '';
  }
}

/** Handle network dropdown change */
function walletNetworkChanged() {
  var networkEl = document.getElementById('wallet-network');
  if (!networkEl) return;
  var network = networkEl.value;
  localStorage.setItem('hos_solana_network', network);

  var customRow = document.getElementById('wallet-custom-rpc-row');
  if (customRow) customRow.style.display = network === 'custom' ? '' : 'none';
}

/** Save custom RPC URL */
function walletSaveRpc() {
  var rpcEl = document.getElementById('wallet-custom-rpc');
  if (rpcEl) localStorage.setItem('hos_solana_rpc', rpcEl.value.trim());
}

/** Save nav balance toggle */
function walletSaveNavBalance() {
  var navBalEl = document.getElementById('wallet-show-nav-balance');
  if (navBalEl) localStorage.setItem('hos_wallet_show_nav_balance', navBalEl.checked ? 'true' : 'false');
}

// ══════════════════════════════════════════════════════════════════════════════
// ── Merge A: Server Info (from knowledge.html) ──
// ══════════════════════════════════════════════════════════════════════════════

// Collapsible info sections
document.querySelectorAll('#sec-server-info .info-section h2').forEach(h2 => {
  h2.addEventListener('click', () => {
    h2.parentElement.classList.toggle('collapsed');
  });
});

// Fetch server stats (ki_ prefix to avoid conflicts)
(async function ki_loadInfoStats() {
  try {
    const [statsRes, infoRes] = await Promise.all([
      fetch('/api/stats').then(r => r.ok ? r.json() : null).catch(() => null),
      fetch('/api/server-info').then(r => r.ok ? r.json() : null).catch(() => null)
    ]);
    if (statsRes) {
      if (statsRes.online !== undefined) document.getElementById('ki-info-online').textContent = statsRes.online;
      if (statsRes.registered !== undefined) document.getElementById('ki-info-registered').textContent = statsRes.registered;
      if (statsRes.uptime) {
        const s = statsRes.uptime;
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        document.getElementById('ki-info-uptime').textContent = h + 'h ' + m + 'm';
      }
    }
    if (infoRes) {
      if (infoRes.version) document.getElementById('ki-info-version').textContent = infoRes.version;
      else if (infoRes.name) document.getElementById('ki-info-version').textContent = infoRes.name;
    }
  } catch(e) { console.warn('Server info stats fetch failed:', e); }
})();

// ══════════════════════════════════════════════════════════════════════════════
// ── Merge B: Vault (from vault.html) ──
// All vault functions prefixed with vault_ to avoid conflicts with settings.
// ══════════════════════════════════════════════════════════════════════════════

(function() {
  'use strict';

  const VAULT_LS_KEY = 'hos_vault_v1';

  // In-memory state — cleared on lock
  let vaultKey     = null;
  let vault        = null;
  let activeId     = null;
  let editingId    = null;
  let selectedType = 'seed_phrase';

  // ── Crypto helpers ──

  const venc = new TextEncoder();
  const vdec = new TextDecoder();

  function vb64(buf) { return btoa(String.fromCharCode(...new Uint8Array(buf))); }
  function vunb64(s) { return Uint8Array.from(atob(s), c => c.charCodeAt(0)); }

  async function vault_deriveKey(passphrase, salt) {
    const raw = await crypto.subtle.importKey('raw', venc.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
    return crypto.subtle.deriveKey(
      { name: 'PBKDF2', hash: 'SHA-256', salt, iterations: 600_000 },
      raw,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt', 'decrypt']
    );
  }

  async function vault_encrypt(vaultObj, key, salt) {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, venc.encode(JSON.stringify(vaultObj)));
    return JSON.stringify({ v: 1, salt: vb64(salt), iv: vb64(iv), enc: vb64(ct) });
  }

  async function vault_decryptBlob(blob, key) {
    let pt;
    try {
      pt = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: vunb64(blob.iv) }, key, vunb64(blob.enc));
    } catch {
      throw new Error('Wrong passphrase or corrupted vault.');
    }
    return JSON.parse(vdec.decode(pt));
  }

  // ── Screen management ──

  function vault_showScreen(name) {
    document.querySelectorAll('#sec-vault .vault-screen').forEach(s => s.classList.remove('active'));
    const el = document.getElementById('vault-sec-' + name);
    if (el) el.classList.add('active');
  }
  window.vault_showScreen = vault_showScreen;

  function vault_determineInitialScreen() {
    const stored = localStorage.getItem(VAULT_LS_KEY);
    if (!stored) { vault_showScreen('setup'); return; }
    vault_showScreen('lock');
    setTimeout(() => { const lp = document.getElementById('vault-sec-lock-pass'); if (lp) lp.focus(); }, 80);
  }

  // ── Setup (create vault) ──

  async function vault_doSetup() {
    const p1  = document.getElementById('vault-sec-setup-pass1').value;
    const p2  = document.getElementById('vault-sec-setup-pass2').value;
    const msg = document.getElementById('vault-sec-setup-msg');
    const btn = document.getElementById('vault-sec-setup-btn');

    if (p1.length < 8) { vault_showMsg(msg, 'Passphrase must be at least 8 characters.', 'red'); return; }
    if (p1 !== p2)     { vault_showMsg(msg, 'Passphrases do not match.', 'red'); return; }

    btn.disabled = true; btn.textContent = 'Creating…';
    vault_showMsg(msg, '', '');

    try {
      const salt = crypto.getRandomValues(new Uint8Array(16));
      const key  = await vault_deriveKey(p1, salt);
      vault = { version: 1, entries: [], created_at: Date.now() };
      const blob = await vault_encrypt(vault, key, salt);
      localStorage.setItem(VAULT_LS_KEY, blob);
      vaultKey = key;
      vault_showUnlocked();
    } catch(e) {
      vault_showMsg(msg, 'Error: ' + e.message, 'red');
      btn.disabled = false; btn.textContent = 'Create Vault';
    }
  }
  window.vault_doSetup = vault_doSetup;

  // ── Unlock ──

  async function vault_doUnlock() {
    const pass = document.getElementById('vault-sec-lock-pass').value;
    const msg  = document.getElementById('vault-sec-lock-msg');
    const btn  = document.getElementById('vault-sec-lock-btn');

    if (!pass) { vault_showMsg(msg, 'Enter your passphrase.', 'red'); return; }

    btn.disabled = true; btn.textContent = 'Unlocking…';
    vault_showMsg(msg, '', '');

    try {
      const stored = localStorage.getItem(VAULT_LS_KEY);
      if (!stored) { vault_showScreen('setup'); return; }
      const blob = JSON.parse(stored);
      const salt = vunb64(blob.salt);
      const key  = await vault_deriveKey(pass, salt);
      vault      = await vault_decryptBlob(blob, key);
      vaultKey   = key;
      document.getElementById('vault-sec-lock-pass').value = '';
      vault_showUnlocked();
    } catch(e) {
      vault_showMsg(msg, e.message, 'red');
      btn.disabled = false; btn.textContent = 'Unlock Vault';
    }
  }
  window.vault_doUnlock = vault_doUnlock;

  // ── Lock ──

  function vault_lock() {
    clearTimeout(lockTimer);
    lockTimer = null; lockAt = null;
    vaultKey = null;
    vault    = null;
    activeId = null;
    document.getElementById('vault-sec-blur-overlay').style.display = 'none';
    vault_showScreen('lock');
    setTimeout(() => {
      const lp = document.getElementById('vault-sec-lock-pass');
      if (lp) { lp.value = ''; lp.focus(); }
    }, 80);
  }
  window.vault_lock = vault_lock;

  // ── Show vault (unlocked) ──

  function vault_showUnlocked() {
    vault_showScreen('main-screen');
    vault_renderEntryList();
    vault_showWelcome();
    vault_resetLockTimer();
  }

  // ── Persist vault to localStorage ──

  async function vault_persist() {
    if (!vaultKey || !vault) return;
    const stored = localStorage.getItem(VAULT_LS_KEY);
    const blob   = JSON.parse(stored);
    const salt   = vunb64(blob.salt);
    const newBlob = await vault_encrypt(vault, vaultKey, salt);
    localStorage.setItem(VAULT_LS_KEY, newBlob);
  }

  // ── Entry list ──

  function vault_renderEntryList() {
    const list  = document.getElementById('vault-sec-entry-list');
    const query = (document.getElementById('vault-sec-search-input').value || '').toLowerCase();
    const entries = (vault && vault.entries || [])
      .filter(e => !query || e.title.toLowerCase().includes(query) || e.type.includes(query));

    const count = document.getElementById('vault-sec-entry-count');
    if (count) count.textContent = `${vault && vault.entries.length || 0} ${vault && vault.entries.length === 1 ? 'entry' : 'entries'}`;

    if (!entries.length) {
      if (list) list.innerHTML = `<div class="no-entries">${query ? 'No matches.' : 'No entries yet — click + New to add one.'}</div>`;
      return;
    }

    if (list) list.innerHTML = entries.map(e => `
      <div class="entry-item ${e.id === activeId ? 'active' : ''}" onclick="vault_selectEntry('${e.id}')">
        <span class="entry-item-icon">${vault_entryIcon(e.type)}</span>
        <div class="entry-item-info">
          <div class="entry-item-title">${vault_esc(e.title)}</div>
          <div class="entry-item-type">${vault_typeLabel(e.type)}</div>
        </div>
      </div>
    `).join('');
  }
  window.vault_renderEntryList = vault_renderEntryList;

  // ── Entry selection / detail ──

  function vault_selectEntry(id) {
    activeId = id;
    vault_renderEntryList();
    vault_showEntryDetail(id);
  }
  window.vault_selectEntry = vault_selectEntry;

  function vault_showWelcome() {
    activeId = null;
    const w = document.getElementById('vault-sec-welcome');
    if (w) w.style.display = '';
    const d = document.getElementById('vault-sec-detail');
    if (d) d.classList.remove('visible');
    const qa = document.getElementById('vault-sec-quick-add-grid');
    if (qa) qa.style.display = vault && vault.entries.length ? 'none' : '';
    vault_renderEntryList();
    vault_checkSeedPhraseBanner();
  }

  function vault_checkSeedPhraseBanner() {
    const banner = document.getElementById('vault-sec-seed-nudge-banner');
    if (!banner) return;
    if (localStorage.getItem('hos_vault_seed_nudge_dismissed')) { banner.style.display = 'none'; return; }
    if (!localStorage.getItem('humanity_key')) { banner.style.display = 'none'; return; }
    const hasSeed = vault && vault.entries.some(e => e.type === 'seed_phrase');
    if (hasSeed) { banner.style.display = 'none'; return; }
    banner.style.display = 'flex';
  }

  function vault_dismissSeedNudge() {
    localStorage.setItem('hos_vault_seed_nudge_dismissed', '1');
    const b = document.getElementById('vault-sec-seed-nudge-banner');
    if (b) b.style.display = 'none';
  }
  window.vault_dismissSeedNudge = vault_dismissSeedNudge;

  function vault_showEntryDetail(id) {
    const entry = vault && vault.entries.find(e => e.id === id);
    if (!entry) { vault_showWelcome(); return; }

    const w = document.getElementById('vault-sec-welcome');
    if (w) w.style.display = 'none';
    const detail = document.getElementById('vault-sec-detail');
    if (detail) detail.classList.add('visible');

    document.getElementById('vault-sec-detail-icon').textContent  = vault_entryIcon(entry.type);
    document.getElementById('vault-sec-detail-title').textContent = entry.title;
    document.getElementById('vault-sec-detail-type').textContent  = vault_typeLabel(entry.type);

    const fieldsEl = document.getElementById('vault-sec-detail-fields');
    fieldsEl.innerHTML = entry.fields.map((f, i) => {
      const fid = 'vault-fv-' + i;
      if (f.secret) {
        return `
          <div class="field-display">
            <div class="field-display-label">${vault_esc(f.label)}</div>
            <div class="field-display-value secret" id="${fid}" onclick="vault_toggleReveal('${fid}')"
                 title="Click to reveal">${vault_esc(f.value)}</div>
            <div class="field-copy-row">
              <button class="btn-reveal" onclick="vault_toggleReveal('${fid}')">👁 Reveal</button>
              <button class="btn-copy-field" onclick="vault_copyField('${fid}', '${vault_escAttr(f.value)}', this)">📋 Copy</button>
              <span class="copy-confirm" id="${fid}-cc"></span>
            </div>
          </div>`;
      }
      return `
        <div class="field-display">
          <div class="field-display-label">${vault_esc(f.label)}</div>
          <div class="field-display-value" id="${fid}">${vault_esc(f.value)}</div>
          <div class="field-copy-row">
            <button class="btn-copy-field" onclick="vault_copyField('${fid}', '${vault_escAttr(f.value)}', this)">📋 Copy</button>
            <span class="copy-confirm" id="${fid}-cc"></span>
          </div>
        </div>`;
    }).join('');

    const ts = new Date(entry.updated_at || entry.created_at);
    const dateEl = document.createElement('p');
    dateEl.style.cssText = 'font-size:.66rem;color:#3a3a3a;margin-top:var(--space-xl)';
    dateEl.textContent = 'Last updated ' + ts.toLocaleDateString() + ' at ' + ts.toLocaleTimeString();
    fieldsEl.appendChild(dateEl);
  }

  function vault_toggleReveal(fid) {
    const el = document.getElementById(fid);
    if (!el) return;
    el.classList.toggle('revealed');
    const btn = el.nextElementSibling && el.nextElementSibling.querySelector('.btn-reveal');
    if (btn) btn.textContent = el.classList.contains('revealed') ? '🙈 Hide' : '👁 Reveal';
  }
  window.vault_toggleReveal = vault_toggleReveal;

  function vault_copyField(fid, value, btn) {
    navigator.clipboard.writeText(value).then(() => {
      const cc   = document.getElementById(fid + '-cc');
      const orig = btn.textContent;
      btn.textContent = 'Copied!';

      const fieldIndex = parseInt(fid.replace('vault-fv-', ''), 10);
      const entry = vault && vault.entries.find(e => e.id === activeId);
      const isSecret = entry && entry.fields[fieldIndex] && entry.fields[fieldIndex].secret;

      if (isSecret) {
        let remaining = 30;
        if (cc) cc.textContent = `✓ Copied — clipboard clears in ${remaining}s`;
        const iv = setInterval(() => {
          remaining--;
          if (remaining <= 0) {
            clearInterval(iv);
            navigator.clipboard.writeText('').catch(() => {});
            if (cc) { cc.textContent = '🧹 Clipboard cleared'; setTimeout(() => { cc.textContent = ''; }, 2000); }
            btn.textContent = orig;
          } else {
            if (cc) cc.textContent = `✓ Copied — clipboard clears in ${remaining}s`;
          }
        }, 1000);
      } else {
        if (cc) { cc.textContent = '✓ Copied'; setTimeout(() => { cc.textContent = ''; }, 2500); }
        setTimeout(() => { btn.textContent = orig; }, 2500);
      }
    }).catch(() => {});
  }
  window.vault_copyField = vault_copyField;

  // ── Delete entry ──

  async function vault_deleteEntry() {
    if (!activeId || !vault) return;
    const entry = vault.entries.find(e => e.id === activeId);
    if (!entry) return;
    if (!confirm(`Delete "${entry.title}"? This cannot be undone.`)) return;
    vault.entries = vault.entries.filter(e => e.id !== activeId);
    await vault_persist();
    activeId = null;
    vault_renderEntryList();
    vault_showWelcome();
  }
  window.vault_deleteEntry = vault_deleteEntry;

  // ── Entry modal ──

  const VAULT_TYPE_TEMPLATES = {
    seed_phrase: {
      label: 'Seed Phrase', icon: '🌱',
      fields: [
        { label: 'Seed Phrase (24 words)', key: 'phrase', placeholder: 'word1 word2 … word24', multiline: true, secret: true }
      ],
      note: 'These 24 words are your identity master key. Anyone who has them can use your account — guard them carefully.'
    },
    password: {
      label: 'Password', icon: '🔑',
      fields: [
        { label: 'Username or Email', key: 'username', placeholder: 'you@example.com', secret: false },
        { label: 'Password',          key: 'password', placeholder: '••••••••',         secret: true },
        { label: 'Notes (optional)',  key: 'notes',    placeholder: 'Any extra info…',  multiline: true, secret: false, optional: true }
      ]
    },
    note: {
      label: 'Secure Note', icon: '📝',
      fields: [
        { label: 'Note', key: 'content', placeholder: 'Your private note…', multiline: true, secret: true }
      ]
    },
    login: {
      label: 'Login', icon: '🔗',
      fields: [
        { label: 'Website',  key: 'url',      placeholder: 'https://example.com', secret: false },
        { label: 'Username', key: 'username', placeholder: 'your username',        secret: false },
        { label: 'Password', key: 'password', placeholder: '••••••••',             secret: true }
      ]
    },
    custom: {
      label: 'Custom', icon: '🪙',
      fields: [
        { label: 'Field 1', key: 'f1', placeholder: 'Label: value…', multiline: true, secret: false }
      ],
      note: 'Use this for anything that doesn\'t fit the other types. You can put any text you like here.'
    }
  };

  function vault_selectEntryType(type) {
    selectedType = type;
    document.querySelectorAll('#vault-sec-entry-modal .type-chip').forEach(c => c.classList.toggle('selected', c.dataset.type === type));
    vault_renderModalFields(type);
  }
  window.vault_selectEntryType = vault_selectEntryType;

  function vault_renderModalFields(type, existingFields) {
    const tmpl = VAULT_TYPE_TEMPLATES[type] || VAULT_TYPE_TEMPLATES.custom;
    const container = document.getElementById('vault-sec-modal-dynamic-fields');
    let html = '';

    if (tmpl.note) {
      html += `<div style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:7px;padding:var(--space-lg) var(--space-xl);margin-bottom:var(--space-xl);font-size:.76rem;color:#666;line-height:1.5">${vault_esc(tmpl.note)}</div>`;
    }

    tmpl.fields.forEach(f => {
      const existing = existingFields && existingFields.find(ef => ef.label === f.label);
      const val = existing ? existing.value : '';
      const input = f.multiline
        ? `<textarea id="vault-mf-${f.key}" placeholder="${vault_esc(f.placeholder || '')}" rows="3">${vault_esc(val)}</textarea>`
        : `<input type="${f.secret ? 'password' : 'text'}" id="vault-mf-${f.key}" placeholder="${vault_esc(f.placeholder || '')}" value="${vault_escAttr(val)}" autocomplete="off">`;
      html += `<div class="modal-field"><label>${vault_esc(f.label)}</label>${input}</div>`;
    });

    container.innerHTML = html;
  }

  function vault_openNewEntryModal(type) {
    type = type || 'seed_phrase';
    selectedType  = type;
    editingId     = null;

    document.getElementById('vault-sec-modal-title').textContent = 'New Entry';
    document.getElementById('vault-sec-modal-entry-title').value = '';
    document.getElementById('vault-sec-modal-msg').textContent   = '';
    document.getElementById('vault-sec-modal-save-btn').disabled = false;
    document.getElementById('vault-sec-modal-save-btn').textContent = 'Save';
    document.getElementById('vault-sec-type-selector-row').style.display = '';

    document.querySelectorAll('#vault-sec-entry-modal .type-chip').forEach(c => c.classList.toggle('selected', c.dataset.type === type));
    vault_renderModalFields(type);
    document.getElementById('vault-sec-entry-modal-overlay').classList.add('open');
    document.getElementById('vault-sec-modal-entry-title').focus();
  }
  window.vault_openNewEntryModal = vault_openNewEntryModal;

  function vault_openEditEntryModal() {
    if (!activeId) return;
    const entry = vault && vault.entries.find(e => e.id === activeId);
    if (!entry) return;

    editingId    = activeId;
    selectedType = entry.type;

    document.getElementById('vault-sec-modal-title').textContent = 'Edit Entry';
    document.getElementById('vault-sec-modal-entry-title').value = entry.title;
    document.getElementById('vault-sec-modal-msg').textContent   = '';
    document.getElementById('vault-sec-modal-save-btn').disabled = false;
    document.getElementById('vault-sec-modal-save-btn').textContent = 'Save';
    document.getElementById('vault-sec-type-selector-row').style.display = 'none';

    vault_renderModalFields(entry.type, entry.fields);

    const tmpl = VAULT_TYPE_TEMPLATES[entry.type] || VAULT_TYPE_TEMPLATES.custom;
    tmpl.fields.forEach(f => {
      const el = document.getElementById('vault-mf-' + f.key);
      if (el && el.type === 'password') el.type = 'text';
    });

    document.getElementById('vault-sec-entry-modal-overlay').classList.add('open');
    document.getElementById('vault-sec-modal-entry-title').focus();
  }
  window.vault_openEditEntryModal = vault_openEditEntryModal;

  function vault_closeEntryModal() {
    document.getElementById('vault-sec-entry-modal-overlay').classList.remove('open');
    editingId = null;
  }
  window.vault_closeEntryModal = vault_closeEntryModal;

  async function vault_saveEntry() {
    const title = document.getElementById('vault-sec-modal-entry-title').value.trim();
    const msg   = document.getElementById('vault-sec-modal-msg');
    const btn   = document.getElementById('vault-sec-modal-save-btn');

    if (!title) { vault_showMsg(msg, 'Please enter a title.', 'red'); return; }

    const tmpl = VAULT_TYPE_TEMPLATES[selectedType] || VAULT_TYPE_TEMPLATES.custom;
    const fields = tmpl.fields.map(f => ({
      label:  f.label,
      value:  (document.getElementById('vault-mf-' + f.key) || {}).value || '',
      secret: f.secret || false
    }));

    if (fields.every(f => !f.value.trim())) {
      vault_showMsg(msg, 'Fill in at least one field.', 'red'); return;
    }

    btn.disabled = true; btn.textContent = 'Saving…';
    vault_showMsg(msg, '', '');

    try {
      if (editingId) {
        const idx = vault.entries.findIndex(e => e.id === editingId);
        if (idx !== -1) {
          vault.entries[idx] = { ...vault.entries[idx], title, fields, updated_at: Date.now() };
        }
      } else {
        vault.entries.push({
          id: vault_uid(),
          type: selectedType,
          title,
          fields,
          created_at: Date.now(),
          updated_at: Date.now()
        });
      }

      await vault_persist();
      vault_closeEntryModal();
      vault_renderEntryList();

      const savedId = editingId || vault.entries[vault.entries.length - 1].id;
      activeId = savedId;
      vault_renderEntryList();
      vault_showEntryDetail(savedId);
    } catch(e) {
      vault_showMsg(msg, 'Error saving: ' + e.message, 'red');
      btn.disabled = false; btn.textContent = 'Save';
    }
  }
  window.vault_saveEntry = vault_saveEntry;

  // ── Export vault (encrypted) ──

  function vault_export() {
    const stored = localStorage.getItem(VAULT_LS_KEY);
    if (!stored) { alert('No vault to export.'); return; }
    const a = document.createElement('a');
    a.href = 'data:application/json;charset=utf-8,' + encodeURIComponent(stored);
    a.download = 'humanity-vault-backup.json';
    a.click();
  }
  window.vault_export = vault_export;

  async function vault_importData(file, passphrase) {
    const text = await file.text();
    const blob = JSON.parse(text);
    const salt = vunb64(blob.salt);
    const key  = await vault_deriveKey(passphrase, salt);
    const vaultObj = await vault_decryptBlob(blob, key);
    if (vault) {
      const existingIds = new Set(vault.entries.map(e => e.id));
      vaultObj.entries.forEach(e => { if (!existingIds.has(e.id)) vault.entries.push(e); });
      await vault_persist();
    } else {
      vault = vaultObj;
      vaultKey = key;
      localStorage.setItem(VAULT_LS_KEY, text);
    }
    vault_renderEntryList();
    vault_showWelcome();
  }

  // ── Import UI ──

  function vault_openImportUI() {
    document.getElementById('vault-sec-import-overlay').style.display = 'flex';
    setTimeout(() => document.getElementById('vault-sec-import-pass').focus(), 80);
  }
  window.vault_openImportUI = vault_openImportUI;

  async function vault_doImport() {
    const fileInput = document.getElementById('vault-sec-import-file');
    const pass      = document.getElementById('vault-sec-import-pass').value;
    const msg       = document.getElementById('vault-sec-import-msg');
    const btn       = document.getElementById('vault-sec-import-btn');

    if (!fileInput.files.length) { msg.innerHTML = '<span style="color:#e55">Select a backup file first.</span>'; return; }
    if (!pass)                   { msg.innerHTML = '<span style="color:#e55">Enter the passphrase used when this backup was created.</span>'; return; }

    btn.disabled = true; btn.textContent = 'Importing…'; msg.textContent = '';

    try {
      await vault_importData(fileInput.files[0], pass);
      msg.innerHTML = '<span style="color:#4ec87a">✓ Imported and merged. Entries are now in your vault.</span>';
      btn.textContent = 'Done';
      setTimeout(() => { document.getElementById('vault-sec-import-overlay').style.display = 'none'; }, 2000);
    } catch(e) {
      msg.innerHTML = `<span style="color:#e55">⚠ ${e.message} — wrong passphrase?</span>`;
      btn.disabled = false; btn.textContent = 'Import & Merge';
    }
  }
  window.vault_doImport = vault_doImport;

  // ── Utility ──

  function vault_uid() {
    return ([1e7]+-1e3+-4e3+-8e3+-1e11).replace(/[018]/g,
      c => (c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16));
  }

  function vault_esc(str) {
    return (str || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }
  function vault_escAttr(str) {
    return (str || '').replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
  }

  function vault_showMsg(el, text, type) {
    if (!el) return;
    el.textContent = text;
    el.style.color = type === 'red' ? '#e55' : type === 'green' ? '#4ec87a' : '#888';
  }

  function vault_entryIcon(type) {
    return { seed_phrase: '🌱', password: '🔑', note: '📝', login: '🔗', custom: '🪙' }[type] || '📄';
  }

  function vault_typeLabel(type) {
    return { seed_phrase: 'Seed Phrase', password: 'Password', note: 'Secure Note', login: 'Login', custom: 'Custom' }[type] || type;
  }

  // ── Auto-lock inactivity timer ──

  const VAULT_LOCK_TIMEOUT_MS = 5 * 60 * 1000;
  let lockTimer = null;
  let lockAt    = null;

  function vault_resetLockTimer() {
    if (!vault) return;
    clearTimeout(lockTimer);
    lockAt    = Date.now() + VAULT_LOCK_TIMEOUT_MS;
    lockTimer = setTimeout(vault_lock, VAULT_LOCK_TIMEOUT_MS);
  }

  function vault_updateLockCountdown() {
    const el = document.getElementById('vault-sec-lock-countdown');
    if (!el) return;
    if (!vault || !lockAt) { el.textContent = ''; return; }
    const rem = Math.max(0, lockAt - Date.now());
    const m   = Math.floor(rem / 60000);
    const s   = Math.floor((rem % 60000) / 1000);
    el.textContent = `Auto-locks in ${m}:${String(s).padStart(2, '0')}`;
  }

  setInterval(() => { if (vault) vault_updateLockCountdown(); }, 1000);

  ['mousemove', 'keydown', 'click', 'touchstart', 'scroll'].forEach(ev =>
    document.addEventListener(ev, () => { if (vault) vault_resetLockTimer(); }, { passive: true })
  );

  // ── Tab-hide blur overlay ──

  document.addEventListener('visibilitychange', () => {
    const overlay = document.getElementById('vault-sec-blur-overlay');
    if (!overlay || !vault) return;
    overlay.style.display = document.visibilityState === 'hidden' ? 'flex' : 'none';
  });

  // ── Cloud vault sync (relay-backed) ──

  async function vault_signSyncRequest() {
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
        console.warn('Vault sync: unrecognised key_backup format');
        return null;
      }
      const ts = Date.now();
      const payload = `vault_sync\n${ts}`;
      const sigBuf = await crypto.subtle.sign('Ed25519', privateKey, new TextEncoder().encode(payload));
      const sig = Array.from(new Uint8Array(sigBuf)).map(b => b.toString(16).padStart(2,'0')).join('');
      return { key: keyHex, timestamp: ts, sig };
    } catch(e) { console.warn('Vault sync sign failed:', e); return null; }
  }

  async function vault_syncToCloud() {
    const stored = localStorage.getItem(VAULT_LS_KEY);
    if (!stored) { alert('No vault to sync.'); return; }
    const auth = await vault_signSyncRequest();
    if (!auth) { alert('No Humanity identity found — vault sync requires a chat identity to authenticate.'); return; }
    const btn = document.getElementById('vault-sec-cloud-sync-btn');
    if (btn) { btn.disabled = true; btn.textContent = 'Syncing…'; }
    try {
      const res = await fetch('/api/vault/sync', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...auth, blob: stored })
      });
      if (!res.ok) { const t = await res.text(); throw new Error(t); }
      if (btn) { btn.textContent = '☁ Synced ✓'; setTimeout(() => { btn.disabled = false; btn.textContent = '☁ Sync'; }, 3000); }
    } catch(e) {
      alert('Sync failed: ' + e.message);
      if (btn) { btn.disabled = false; btn.textContent = '☁ Sync'; }
    }
  }
  window.vault_syncToCloud = vault_syncToCloud;

  async function vault_restoreFromCloud() {
    const auth = await vault_signSyncRequest();
    if (!auth) { alert('No Humanity identity found — vault sync requires a chat identity to authenticate.'); return; }
    if (!confirm('Restore vault from cloud? Any entries not in the cloud backup will be lost unless you export first.')) return;
    try {
      const url = `/api/vault/sync?key=${encodeURIComponent(auth.key)}&timestamp=${auth.timestamp}&sig=${encodeURIComponent(auth.sig)}`;
      const res = await fetch(url);
      if (res.status === 404) { alert('No cloud backup found for this identity.'); return; }
      if (!res.ok) { const t = await res.text(); throw new Error(t); }
      const { blob, updated_at } = await res.json();
      const date = new Date(updated_at).toLocaleString();
      if (!confirm(`Found cloud backup from ${date}. Overwrite local vault and reload?`)) return;
      localStorage.setItem(VAULT_LS_KEY, blob);
      location.reload();
    } catch(e) {
      alert('Restore failed: ' + e.message);
    }
  }
  window.vault_restoreFromCloud = vault_restoreFromCloud;

  // ── Keyboard shortcuts ──

  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') {
      if (document.getElementById('vault-sec-entry-modal-overlay').classList.contains('open')) {
        vault_closeEntryModal();
      }
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'l' && vault) {
      e.preventDefault();
      vault_lock();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'n' && vault) {
      // Only intercept if vault section is active
      const sec = document.getElementById('sec-vault');
      if (sec && sec.classList.contains('active')) {
        e.preventDefault();
        vault_openNewEntryModal();
      }
    }
  });

  // ── Enter key on setup/lock fields ──
  const sp1 = document.getElementById('vault-sec-setup-pass1');
  if (sp1) sp1.addEventListener('keydown', e => { if (e.key === 'Enter') document.getElementById('vault-sec-setup-pass2').focus(); });
  const sp2 = document.getElementById('vault-sec-setup-pass2');
  if (sp2) sp2.addEventListener('keydown', e => { if (e.key === 'Enter') vault_doSetup(); });

  // ── Boot vault ──
  vault_determineInitialScreen();
})();

// ══════════════════════════════════════════════════════════════════════════════
// ── Settings-specific backup/seed modals ──
// These are standalone versions that do not depend on chat app functions.
// Requires crypto.js to be loaded first.
// ══════════════════════════════════════════════════════════════════════════════

// ── Settings-specific backup/seed modals ──
// These are standalone versions that don't depend on chat app functions.

function settingsAlert(msg) {
  var d = document.createElement('div');
  d.style.cssText = 'position:fixed;top:var(--space-xl);right:var(--space-xl);z-index:9999;background:#181818;border:1px solid #2a2a2a;border-radius:8px;padding:var(--space-lg) var(--space-2xl);color:#e0e0e0;font-size:0.85rem;max-width:400px;box-shadow:0 8px 24px rgba(0,0,0,0.5);';
  d.textContent = msg;
  document.body.appendChild(d);
  setTimeout(function() { d.remove(); }, 5000);
}

async function settingsOpenBackup() {
  var overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:8000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);box-sizing:border-box;';
  overlay.innerHTML = '<div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:480px;color:#e0e0e0;">' +
    '<h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 var(--space-md)">🔐 Download Encrypted Backup</h2>' +
    '<p style="font-size:.8rem;color:#888;line-height:1.5;margin:0 0 var(--space-xl)">Enter a passphrase to encrypt your private key. Store the file in your cloud — it\'s useless without the passphrase.</p>' +
    '<input id="set-bkp-pass" type="password" placeholder="Passphrase (8+ characters)" autocomplete="new-password" style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:6px;padding:var(--space-md) var(--space-lg);color:#e0e0e0;font-size:.85rem;outline:none;box-sizing:border-box;margin-bottom:var(--space-md);">' +
    '<div style="display:flex;gap:var(--space-md);justify-content:flex-end;margin-top:var(--space-lg);">' +
    '<button id="set-bkp-cancel" style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer">Cancel</button>' +
    '<button id="set-bkp-go" style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:var(--space-md) 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Download</button>' +
    '</div>' +
    '<div id="set-bkp-msg" style="font-size:.75rem;margin-top:var(--space-md);min-height:1em;"></div>' +
    '</div>';
  document.body.appendChild(overlay);
  overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#set-bkp-cancel').addEventListener('click', function() { overlay.remove(); });
  overlay.querySelector('#set-bkp-go').addEventListener('click', async function() {
    var pass = overlay.querySelector('#set-bkp-pass').value.trim();
    var msg = overlay.querySelector('#set-bkp-msg');
    if (pass.length < 8) { msg.innerHTML = '<span style="color:#e55">Passphrase must be at least 8 characters.</span>'; return; }
    this.disabled = true; this.textContent = 'Encrypting…';
    try {
      await exportEncryptedIdentityBackup(pass);
      msg.innerHTML = '<span style="color:#4ec87a">✓ Backup downloaded.</span>';
      this.textContent = 'Done!';
      localStorage.setItem('hos_key_protected', '1');
      checkBackupStatus();
    } catch(e) {
      msg.innerHTML = '<span style="color:#e55">' + e.message + '</span>';
      this.disabled = false; this.textContent = 'Download';
    }
  });
  overlay.querySelector('#set-bkp-pass').addEventListener('keydown', function(e) { if (e.key === 'Enter') overlay.querySelector('#set-bkp-go').click(); });
}

function settingsOpenRestore() {
  var overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:8000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);box-sizing:border-box;';
  overlay.innerHTML = '<div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:480px;color:#e0e0e0;">' +
    '<h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 var(--space-md)">📥 Restore from Backup File</h2>' +
    '<p style="font-size:.8rem;color:#e55;line-height:1.5;margin:0 0 var(--space-md)"><strong>This will replace your current identity on this device.</strong></p>' +
    '<p style="font-size:.8rem;color:#888;line-height:1.5;margin:0 0 var(--space-xl)">Select your encrypted backup file and enter the passphrase you used when creating it.</p>' +
    '<input id="set-rst-file" type="file" accept=".json,.bak" style="margin-bottom:var(--space-md);font-size:.82rem;color:#888;">' +
    '<input id="set-rst-pass" type="password" placeholder="Backup passphrase" style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:6px;padding:var(--space-md) var(--space-lg);color:#e0e0e0;font-size:.85rem;outline:none;box-sizing:border-box;margin-bottom:var(--space-md);">' +
    '<div style="display:flex;gap:var(--space-md);justify-content:flex-end;margin-top:var(--space-lg);">' +
    '<button id="set-rst-cancel" style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer">Cancel</button>' +
    '<button id="set-rst-go" style="background:#e55;color:#fff;border:none;border-radius:7px;padding:var(--space-md) 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Restore</button>' +
    '</div>' +
    '<div id="set-rst-msg" style="font-size:.75rem;margin-top:var(--space-md);min-height:1em;"></div>' +
    '</div>';
  document.body.appendChild(overlay);
  overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#set-rst-cancel').addEventListener('click', function() { overlay.remove(); });
  overlay.querySelector('#set-rst-go').addEventListener('click', async function() {
    var file = overlay.querySelector('#set-rst-file').files[0];
    var pass = overlay.querySelector('#set-rst-pass').value.trim();
    var msg = overlay.querySelector('#set-rst-msg');
    if (!file) { msg.innerHTML = '<span style="color:#e55">Select a backup file first.</span>'; return; }
    this.disabled = true; this.textContent = 'Restoring…';
    try {
      var text = await file.text();
      var data = JSON.parse(text);
      if (data.encrypted && !pass) { msg.innerHTML = '<span style="color:#e55">This backup is encrypted. Enter the passphrase.</span>'; this.disabled = false; this.textContent = 'Restore'; return; }
      if (typeof importIdentityBackup === 'function') {
        var result = await importIdentityBackup(data, pass || undefined);
        localStorage.setItem('humanity_name', result.name);
        localStorage.setItem('humanity_pubkey', result.publicKeyHex);
        msg.innerHTML = '<span style="color:#4ec87a">✓ Identity restored for ' + result.name + '. Reloading…</span>';
        setTimeout(function() { location.reload(); }, 1500);
      } else {
        msg.innerHTML = '<span style="color:#e55">Restore function not available. Try from the Network page.</span>';
        this.disabled = false; this.textContent = 'Restore';
      }
    } catch(e) {
      msg.innerHTML = '<span style="color:#e55">' + (e.message || 'Decryption failed — wrong passphrase?') + '</span>';
      this.disabled = false; this.textContent = 'Restore';
    }
  });
}

async function settingsOpenSeed() {
  var mnemonic;
  try { mnemonic = await generateMnemonic(); } catch(e) { mnemonic = null; }
  if (!mnemonic) {
    // Key is non-extractable (created before backup support). Offer rotation.
    settingsShowNonExtractableOverlay();
    return;
  }
  var words = mnemonic.trim().split(/\s+/);
  var overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:8000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);box-sizing:border-box;';
  var grid = words.map(function(w, i) {
    return '<div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:7px;padding:var(--space-md) var(--space-md);display:flex;align-items:baseline;gap:var(--space-sm)"><span style="font-size:.6rem;color:#444;min-width:16px;text-align:right">' + (i+1) + '.</span><span style="font-size:.86rem;color:#f0a500;font-weight:600">' + w + '</span></div>';
  }).join('');
  overlay.innerHTML = '<div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:600px;color:#e0e0e0;max-height:90vh;overflow-y:auto;">' +
    '<h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 var(--space-sm)">🌱 Your 24-Word Seed Phrase</h2>' +
    '<p style="font-size:.78rem;color:#e55;line-height:1.5;margin:0 0 var(--space-md)"><strong>Never screenshot this. Never share it. Anyone who has these words IS you.</strong></p>' +
    '<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-md);margin-bottom:var(--space-xl)">' + grid + '</div>' +
    '<div style="display:flex;gap:var(--space-md);flex-wrap:wrap;margin-bottom:var(--space-xl);">' +
    '<button id="set-seed-copy" style="background:none;border:1px solid #333;color:#aaa;border-radius:6px;padding:var(--space-sm) var(--space-xl);font-size:.75rem;cursor:pointer">📋 Copy to clipboard</button>' +
    '<span id="set-seed-msg" style="font-size:.7rem;color:#4ec87a;align-self:center;"></span>' +
    '</div>' +
    '<div style="display:flex;justify-content:flex-end"><button id="set-seed-done" style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:var(--space-md) 1.4rem;font-size:.82rem;font-weight:700;cursor:pointer">Done</button></div>' +
    '</div>';
  document.body.appendChild(overlay);
  overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#set-seed-done').addEventListener('click', function() { overlay.remove(); localStorage.setItem('hos_vault_seed_nudge_dismissed', '1'); checkBackupStatus(); });
  overlay.querySelector('#set-seed-copy').addEventListener('click', function() {
    navigator.clipboard.writeText(mnemonic).then(function() {
      overlay.querySelector('#set-seed-msg').textContent = '✓ Copied — write it down, then clear clipboard';
      overlay.querySelector('#set-seed-copy').textContent = 'Copied!';
    });
  });
}

/**
 * Show overlay explaining that the current key is non-extractable and offering
 * a "Rotate Key" action to generate a new extractable keypair with seed phrase.
 */
function settingsShowNonExtractableOverlay() {
  var overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:8000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);box-sizing:border-box;';
  overlay.innerHTML = '<div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:540px;color:#e0e0e0;max-height:90vh;overflow-y:auto;">' +
    '<h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 var(--space-md)">Seed Phrase Unavailable</h2>' +
    '<p style="font-size:.82rem;color:#ccc;line-height:1.6;margin:0 0 var(--space-xl)">' +
      'Your key was created before backup support was added. The private key stored in your browser ' +
      'is marked as non-extractable, so a seed phrase cannot be generated from it.' +
    '</p>' +
    '<div style="background:#0f1a0f;border:1px solid #1a3a1a;border-radius:8px;padding:var(--space-xl);margin-bottom:var(--space-xl);font-size:.8rem;color:#8cc88c;line-height:1.6">' +
      '<strong style="color:#4ec87a">Solution: Rotate your key.</strong><br>' +
      'This generates a new extractable keypair with full seed phrase backup. ' +
      'Your profile, messages, and reputation transfer automatically via a dual-signature certificate.' +
    '</div>' +
    '<div style="display:flex;gap:var(--space-md);justify-content:flex-end">' +
      '<button id="ne-cancel" style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer">Cancel</button>' +
      '<button id="ne-rotate" style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:var(--space-md) 1.4rem;font-size:.82rem;font-weight:700;cursor:pointer">Rotate Key</button>' +
    '</div>' +
  '</div>';
  document.body.appendChild(overlay);
  overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#ne-cancel').addEventListener('click', function() { overlay.remove(); });
  overlay.querySelector('#ne-rotate').addEventListener('click', async function() {
    var btn = overlay.querySelector('#ne-rotate');
    btn.disabled = true; btn.textContent = 'Generating...';
    try {
      // Generate new extractable keypair directly (no WebSocket needed)
      var newKp = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
      var rawPub = await crypto.subtle.exportKey('raw', newKp.publicKey);
      var newKeyHex = bufToHex(rawPub);

      // Store in IndexedDB
      var db = await openKeyDB();
      await storeKeypair(db, newKeyHex, { privateKey: newKp.privateKey, publicKey: newKp.publicKey });

      // Backup to localStorage (must use PKCS8 format to match restoreKeyFromLocalStorage)
      try {
        var pkcs8 = await crypto.subtle.exportKey('pkcs8', newKp.privateKey);
        var b64 = btoa(String.fromCharCode.apply(null, new Uint8Array(pkcs8)));
        localStorage.setItem('humanity_key', newKeyHex);
        localStorage.setItem('humanity_key_backup', JSON.stringify({
          publicKeyHex: newKeyHex, privateKeyPkcs8: b64
        }));
      } catch(e2) { console.warn('localStorage backup failed:', e2); }

      // Update in-memory identity so View Seed works without reload
      myIdentity = {
        publicKeyHex: newKeyHex,
        privateKey: newKp.privateKey,
        publicKey: newKp.publicKey,
        canSign: true
      };
      btn.textContent = 'Done! Reloading...';
      setTimeout(function() { location.reload(); }, 1500);
    } catch(e) {
      btn.disabled = false; btn.textContent = 'Rotate Key';
      settingsAlert('Error: ' + e.message);
    }
  });
}

function settingsOpenRestoreSeed() {
  var overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:8000;display:flex;align-items:center;justify-content:center;padding:var(--space-xl);box-sizing:border-box;';
  overlay.innerHTML = '<div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:540px;color:#e0e0e0;max-height:90vh;overflow-y:auto;">' +
    '<h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 var(--space-sm)">🌱 Restore from Seed Phrase</h2>' +
    '<p style="font-size:.8rem;color:#e55;line-height:1.5;margin:0 0 var(--space-md)"><strong>This will permanently replace your current identity on this device.</strong></p>' +
    '<p style="font-size:.8rem;color:#888;line-height:1.5;margin:0 0 var(--space-xl)">Enter your 24 words separated by spaces:</p>' +
    '<textarea id="set-rseed-words" rows="4" placeholder="word1 word2 word3 ... word24" style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:6px;padding:var(--space-md) var(--space-lg);color:#e0e0e0;font-size:.85rem;outline:none;box-sizing:border-box;resize:vertical;font-family:monospace;"></textarea>' +
    '<div style="display:flex;gap:var(--space-md);justify-content:flex-end;margin-top:var(--space-lg);">' +
    '<button id="set-rseed-cancel" style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer">Cancel</button>' +
    '<button id="set-rseed-go" style="background:#e55;color:#fff;border:none;border-radius:7px;padding:var(--space-md) 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Restore Identity</button>' +
    '</div>' +
    '<div id="set-rseed-msg" style="font-size:.75rem;margin-top:var(--space-md);min-height:1em;"></div>' +
    '</div>';
  document.body.appendChild(overlay);
  overlay.addEventListener('click', function(e) { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#set-rseed-cancel').addEventListener('click', function() { overlay.remove(); });
  overlay.querySelector('#set-rseed-go').addEventListener('click', async function() {
    var words = overlay.querySelector('#set-rseed-words').value.trim();
    var msg = overlay.querySelector('#set-rseed-msg');
    if (!words) { msg.innerHTML = '<span style="color:#e55">Enter your 24 words.</span>'; return; }
    var arr = words.split(/\s+/);
    if (arr.length !== 24) { msg.innerHTML = '<span style="color:#e55">Expected 24 words, got ' + arr.length + '.</span>'; return; }
    this.disabled = true; this.textContent = 'Restoring…';
    try {
      if (typeof restoreIdentityFromMnemonic === 'function') {
        await restoreIdentityFromMnemonic(words);
        msg.innerHTML = '<span style="color:#4ec87a">✓ Identity restored. Reloading…</span>';
        setTimeout(function() { location.reload(); }, 1500);
      } else {
        msg.innerHTML = '<span style="color:#e55">Restore function not available.</span>';
        this.disabled = false; this.textContent = 'Restore Identity';
      }
    } catch(e) {
      msg.innerHTML = '<span style="color:#e55">' + (e.message || 'Invalid seed phrase.') + '</span>';
      this.disabled = false; this.textContent = 'Restore Identity';
    }
  });
}
