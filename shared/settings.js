/**
 * Humanity Settings Panel
 * Auto-initializes. Stores preferences in localStorage as `humanity_settings`.
 * Applies CSS variable overrides on load.
 */
(function () {
  const STORAGE_KEY = 'humanity_settings';
  const DEFAULTS = {
    accent: '#FF8811',
    fontSize: 'medium',
    theme: 'dark',
    soundEnabled: true,
    timestampMode: 'relative'
  };

  const ACCENT_PRESETS = [
    { name: 'Orange', color: '#FF8811' },
    { name: 'Blue', color: '#4488ff' },
    { name: 'Green', color: '#44cc66' },
    { name: 'Purple', color: '#9944ff' },
    { name: 'Red', color: '#ee4444' },
    { name: 'Pink', color: '#ee44aa' }
  ];

  const FONT_SIZES = [
    { label: 'Small', value: 'small', size: '14px' },
    { label: 'Medium', value: 'medium', size: '16px' },
    { label: 'Large', value: 'large', size: '18px' },
    { label: 'XL', value: 'xl', size: '20px' }
  ];

  const THEMES = {
    dark: {
      '--bg': '#0d0d0d',
      '--bg-secondary': '#1a1a1a',
      '--bg-card': '#161616',
      '--bg-card-hover': '#1c1c1c',
      '--bg-input': '#222',
      '--bg-hover': '#252525',
      '--text': '#e0e0e0',
      '--text-muted': '#888',
      '--border': '#333'
    },
    midnight: {
      '--bg': '#080812',
      '--bg-secondary': '#10101e',
      '--bg-card': '#0c0c18',
      '--bg-card-hover': '#14142a',
      '--bg-input': '#16162a',
      '--bg-hover': '#1a1a30',
      '--text': '#d0d0e0',
      '--text-muted': '#777',
      '--border': '#252540'
    },
    oled: {
      '--bg': '#000000',
      '--bg-secondary': '#0a0a0a',
      '--bg-card': '#050505',
      '--bg-card-hover': '#111111',
      '--bg-input': '#111111',
      '--bg-hover': '#161616',
      '--text': '#e0e0e0',
      '--text-muted': '#777',
      '--border': '#222222'
    }
  };

  function load() {
    try { return Object.assign({}, DEFAULTS, JSON.parse(localStorage.getItem(STORAGE_KEY))); }
    catch { return Object.assign({}, DEFAULTS); }
  }

  function save(settings) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }

  function hexToRgb(hex) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return { r, g, b };
  }

  function darken(hex, amount) {
    const { r, g, b } = hexToRgb(hex);
    const d = (v) => Math.max(0, Math.round(v * (1 - amount)));
    return '#' + [d(r), d(g), d(b)].map(v => v.toString(16).padStart(2, '0')).join('');
  }

  function applySettings(settings) {
    const doc = document.documentElement;

    // Accent
    const accent = settings.accent || DEFAULTS.accent;
    const { r, g, b } = hexToRgb(accent);
    doc.style.setProperty('--accent', accent);
    doc.style.setProperty('--accent-hover', darken(accent, 0.15));
    doc.style.setProperty('--accent-dim', `rgba(${r},${g},${b},0.15)`);

    // Font size
    const fs = FONT_SIZES.find(f => f.value === settings.fontSize) || FONT_SIZES[1];
    doc.style.setProperty('font-size', fs.size);

    // Theme
    const theme = THEMES[settings.theme] || THEMES.dark;
    Object.entries(theme).forEach(([k, v]) => doc.style.setProperty(k, v));

    // Expose settings globally for other scripts
    window.humanitySettings = settings;
  }

  function isOnChat() {
    return location.pathname.startsWith('/chat');
  }

  // ── Inject gear icon into nav ──
  function injectGearButton() {
    const nav = document.querySelector('.hub-nav');
    if (!nav) return;
    const spacer = nav.querySelector('.spacer');
    if (!spacer) return;

    const gear = document.createElement('button');
    gear.className = 'tab';
    gear.textContent = '⚙️';
    gear.title = 'Settings';
    gear.style.cssText = 'font-size:1rem;padding:0.3rem 0.5rem;border:none;background:transparent;cursor:pointer;';
    gear.addEventListener('click', togglePanel);
    spacer.parentNode.insertBefore(gear, spacer);
  }

  // ── Settings Panel ──
  let panelEl = null;

  function togglePanel() {
    if (panelEl) { closePanel(); return; }
    openPanel();
  }

  function closePanel() {
    if (panelEl) { panelEl.remove(); panelEl = null; }
  }

  function openPanel() {
    if (panelEl) return;
    const settings = load();

    const overlay = document.createElement('div');
    overlay.id = 'humanity-settings-overlay';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.6);z-index:9999;display:flex;align-items:center;justify-content:center;';
    overlay.addEventListener('click', (e) => { if (e.target === overlay) closePanel(); });

    const modal = document.createElement('div');
    modal.style.cssText = 'background:var(--bg-secondary);border:1px solid var(--border);border-radius:12px;max-width:480px;width:90%;max-height:85vh;overflow-y:auto;padding:1.5rem;position:relative;';

    const closeBtn = document.createElement('button');
    closeBtn.textContent = '✕';
    closeBtn.style.cssText = 'position:absolute;top:0.6rem;right:0.8rem;background:none;border:none;color:var(--text-muted);font-size:1.1rem;cursor:pointer;';
    closeBtn.onclick = closePanel;

    const title = document.createElement('h2');
    title.textContent = '⚙️ Settings';
    title.style.cssText = 'font-size:1.1rem;color:var(--accent);margin-bottom:1rem;';

    modal.appendChild(closeBtn);
    modal.appendChild(title);

    // ── Accent Color ──
    const accentSection = section('🎨 Accent Color');
    const presetRow = document.createElement('div');
    presetRow.style.cssText = 'display:flex;gap:0.5rem;flex-wrap:wrap;margin-bottom:0.5rem;';
    ACCENT_PRESETS.forEach(p => {
      const btn = document.createElement('button');
      btn.style.cssText = `width:32px;height:32px;border-radius:50%;border:2px solid ${settings.accent === p.color ? '#fff' : 'transparent'};background:${p.color};cursor:pointer;transition:border-color 0.15s;`;
      btn.title = p.name;
      btn.onclick = () => {
        settings.accent = p.color;
        save(settings);
        applySettings(settings);
        closePanel();
        openPanel();
      };
      presetRow.appendChild(btn);
    });
    accentSection.appendChild(presetRow);

    const customRow = document.createElement('div');
    customRow.style.cssText = 'display:flex;align-items:center;gap:0.5rem;';
    const colorInput = document.createElement('input');
    colorInput.type = 'color';
    colorInput.value = settings.accent;
    colorInput.style.cssText = 'width:36px;height:28px;border:none;background:none;cursor:pointer;';
    const colorLabel = document.createElement('span');
    colorLabel.style.cssText = 'font-size:0.8rem;color:var(--text-muted);';
    colorLabel.textContent = 'Custom: ' + settings.accent;
    colorInput.oninput = () => {
      settings.accent = colorInput.value;
      colorLabel.textContent = 'Custom: ' + colorInput.value;
      save(settings);
      applySettings(settings);
    };
    customRow.appendChild(colorInput);
    customRow.appendChild(colorLabel);
    accentSection.appendChild(customRow);
    modal.appendChild(accentSection);

    // ── Font Size ──
    const fsSection = section('🔤 Font Size');
    const fsRow = document.createElement('div');
    fsRow.style.cssText = 'display:flex;gap:0.4rem;';
    FONT_SIZES.forEach(f => {
      const btn = pill(f.label, settings.fontSize === f.value);
      btn.onclick = () => {
        settings.fontSize = f.value;
        save(settings);
        applySettings(settings);
        closePanel();
        openPanel();
      };
      fsRow.appendChild(btn);
    });
    fsSection.appendChild(fsRow);
    modal.appendChild(fsSection);

    // ── Theme ──
    const themeSection = section('🌙 Theme');
    const themeRow = document.createElement('div');
    themeRow.style.cssText = 'display:flex;gap:0.4rem;';
    [['Dark', 'dark'], ['Midnight', 'midnight'], ['OLED Black', 'oled']].forEach(([label, val]) => {
      const btn = pill(label, settings.theme === val);
      btn.onclick = () => {
        settings.theme = val;
        save(settings);
        applySettings(settings);
        closePanel();
        openPanel();
      };
      themeRow.appendChild(btn);
    });
    themeSection.appendChild(themeRow);
    modal.appendChild(themeSection);

    // ── Chat-specific ──
    if (isOnChat()) {
      const chatSection = section('💬 Chat Settings');

      // Timestamp mode
      const tsLabel = document.createElement('div');
      tsLabel.style.cssText = 'font-size:0.8rem;color:var(--text-muted);margin-bottom:0.3rem;';
      tsLabel.textContent = 'Timestamps';
      chatSection.appendChild(tsLabel);
      const tsRow = document.createElement('div');
      tsRow.style.cssText = 'display:flex;gap:0.4rem;margin-bottom:0.5rem;';
      [['Relative', 'relative'], ['Absolute', 'absolute']].forEach(([label, val]) => {
        const btn = pill(label, settings.timestampMode === val);
        btn.onclick = () => {
          settings.timestampMode = val;
          save(settings);
          applySettings(settings);
          closePanel();
          openPanel();
        };
        tsRow.appendChild(btn);
      });
      chatSection.appendChild(tsRow);

      // Sound toggle — syncs with chat's 🔔 menu
      const soundRow = document.createElement('label');
      soundRow.style.cssText = 'display:flex;align-items:center;gap:0.5rem;font-size:0.8rem;color:var(--text);cursor:pointer;margin-bottom:0.4rem;';
      const soundCb = document.createElement('input');
      soundCb.type = 'checkbox';
      // Read from chat's own localStorage key for truth
      const chatSoundEnabled = localStorage.getItem('humanity_sound_enabled') !== 'false';
      soundCb.checked = chatSoundEnabled;
      soundCb.style.accentColor = 'var(--accent)';
      soundCb.onchange = () => {
        settings.soundEnabled = soundCb.checked;
        save(settings);
        applySettings(settings);
        // Sync with chat's sound-enabled checkbox
        const existing = document.getElementById('sound-enabled');
        if (existing && existing.checked !== soundCb.checked) {
          existing.checked = soundCb.checked;
          existing.dispatchEvent(new Event('change'));
        }
        // Also update chat's localStorage directly
        localStorage.setItem('humanity_sound_enabled', soundCb.checked);
        // Update 🔔 icon
        const toggle = document.getElementById('sound-toggle');
        if (toggle) toggle.textContent = soundCb.checked ? '🔔' : '🔕';
      };
      soundRow.appendChild(soundCb);
      soundRow.appendChild(document.createTextNode('🔔 Notification sounds'));
      chatSection.appendChild(soundRow);

      // "Open sound picker" button — opens the chat's existing sound menu
      const soundPickerBtn = document.createElement('button');
      soundPickerBtn.textContent = '🎵 Choose notification sound…';
      soundPickerBtn.style.cssText = 'background:var(--bg-input);border:1px solid var(--border);color:var(--text-muted);padding:0.35rem 0.75rem;border-radius:6px;font-size:0.78rem;cursor:pointer;font-family:inherit;display:block;';
      soundPickerBtn.onmouseenter = () => { soundPickerBtn.style.borderColor = 'var(--accent)'; };
      soundPickerBtn.onmouseleave = () => { soundPickerBtn.style.borderColor = 'var(--border)'; };
      soundPickerBtn.onclick = () => {
        closePanel();
        // Open the chat's existing sound menu
        if (typeof toggleSoundMenu === 'function') toggleSoundMenu();
      };
      chatSection.appendChild(soundPickerBtn);

      modal.appendChild(chatSection);
    }

    // ── Export/Import Data ──
    const dataSection = section('💾 Data Management');

    const exportBtn = document.createElement('button');
    exportBtn.textContent = '📤 Export All Data';
    exportBtn.style.cssText = 'background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.45rem 1rem;border-radius:6px;font-size:0.8rem;cursor:pointer;width:100%;margin-bottom:0.4rem;font-family:inherit;';
    exportBtn.onmouseenter = () => { exportBtn.style.borderColor = 'var(--accent)'; };
    exportBtn.onmouseleave = () => { exportBtn.style.borderColor = 'var(--border)'; };
    exportBtn.onclick = () => {
      const data = {
        identity: {
          name: localStorage.getItem('humanity_name') || 'Unknown',
          publicKey: window.myKey || ''
        },
        settings: (() => { try { return JSON.parse(localStorage.getItem('humanity_settings') || '{}'); } catch { return {}; } })(),
        notes: (() => { try { return JSON.parse(localStorage.getItem('humanity_notes') || '[]'); } catch { return []; } })(),
        todos: (() => { try { return JSON.parse(localStorage.getItem('humanity_todos') || '[]'); } catch { return []; } })(),
        garden: (() => { try { return JSON.parse(localStorage.getItem('humanity_garden') || '{}'); } catch { return {}; } })(),
        pins: (() => { try { return JSON.parse(localStorage.getItem('humanity_pins') || '[]'); } catch { return []; } })(),
        blocked: (() => { try { return JSON.parse(localStorage.getItem('humanity_blocked') || '[]'); } catch { return []; } })(),
        exportedAt: new Date().toISOString()
      };
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const name = data.identity.name.replace(/[^a-zA-Z0-9_-]/g, '');
      const date = new Date().toISOString().slice(0, 10);
      a.href = url;
      a.download = `humanity-backup-${name}-${date}.json`;
      a.click();
      URL.revokeObjectURL(url);
    };
    dataSection.appendChild(exportBtn);

    const importBtn = document.createElement('button');
    importBtn.textContent = '📥 Import Data';
    importBtn.style.cssText = 'background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.45rem 1rem;border-radius:6px;font-size:0.8rem;cursor:pointer;width:100%;font-family:inherit;';
    importBtn.onmouseenter = () => { importBtn.style.borderColor = 'var(--accent)'; };
    importBtn.onmouseleave = () => { importBtn.style.borderColor = 'var(--border)'; };
    importBtn.onclick = () => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json';
      input.onchange = (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (ev) => {
          try {
            const data = JSON.parse(ev.target.result);
            // Validate
            if (!data.exportedAt) {
              alert('Invalid backup file: missing exportedAt field.');
              return;
            }
            // Merge into localStorage
            if (data.settings) localStorage.setItem('humanity_settings', JSON.stringify(data.settings));
            if (data.notes) localStorage.setItem('humanity_notes', JSON.stringify(data.notes));
            if (data.todos) localStorage.setItem('humanity_todos', JSON.stringify(data.todos));
            if (data.garden) localStorage.setItem('humanity_garden', JSON.stringify(data.garden));
            if (data.pins) localStorage.setItem('humanity_pins', JSON.stringify(data.pins));
            if (data.blocked) localStorage.setItem('humanity_blocked', JSON.stringify(data.blocked));
            // Re-apply settings
            if (data.settings) {
              applySettings(Object.assign({}, DEFAULTS, data.settings));
            }
            alert('Data imported successfully! Some changes may require a page reload.');
            closePanel();
          } catch (err) {
            alert('Failed to import: ' + err.message);
          }
        };
        reader.readAsText(file);
      };
      input.click();
    };
    dataSection.appendChild(importBtn);
    modal.appendChild(dataSection);

    // ── Reset ──
    const resetBtn = document.createElement('button');
    resetBtn.textContent = '↺ Reset to Defaults';
    resetBtn.style.cssText = 'margin-top:1rem;background:var(--bg-input);border:1px solid var(--border);color:var(--text-muted);padding:0.45rem 1rem;border-radius:6px;font-size:0.8rem;cursor:pointer;width:100%;';
    resetBtn.onmouseenter = () => { resetBtn.style.borderColor = 'var(--danger, #c44)'; resetBtn.style.color = 'var(--danger, #c44)'; };
    resetBtn.onmouseleave = () => { resetBtn.style.borderColor = 'var(--border)'; resetBtn.style.color = 'var(--text-muted)'; };
    resetBtn.onclick = () => {
      localStorage.removeItem(STORAGE_KEY);
      applySettings(DEFAULTS);
      closePanel();
      // Remove inline styles to restore CSS defaults
      const doc = document.documentElement;
      [...doc.style].forEach(p => doc.style.removeProperty(p));
      applySettings(DEFAULTS);
    };
    modal.appendChild(resetBtn);

    overlay.appendChild(modal);
    document.body.appendChild(overlay);
    panelEl = overlay;
  }

  // Helpers
  function section(label) {
    const div = document.createElement('div');
    div.style.cssText = 'margin-bottom:1rem;';
    const h = document.createElement('div');
    h.style.cssText = 'font-size:0.85rem;font-weight:600;color:var(--text);margin-bottom:0.4rem;';
    h.textContent = label;
    div.appendChild(h);
    return div;
  }

  function pill(text, active) {
    const btn = document.createElement('button');
    btn.textContent = text;
    btn.style.cssText = `padding:0.3rem 0.75rem;border-radius:20px;font-size:0.78rem;cursor:pointer;border:1px solid ${active ? 'var(--accent)' : 'var(--border)'};background:${active ? 'var(--accent-dim)' : 'var(--bg-input)'};color:${active ? 'var(--accent)' : 'var(--text-muted)'};font-family:inherit;`;
    return btn;
  }

  // ── Sync: listen for chat's sound toggle changes ──
  function observeChatSoundToggle() {
    const chatCb = document.getElementById('sound-enabled');
    if (!chatCb || chatCb._settingsObserved) return;
    chatCb._settingsObserved = true;
    chatCb.addEventListener('change', () => {
      const settings = load();
      settings.soundEnabled = chatCb.checked;
      save(settings);
    });
  }
  // Check periodically since chat elements load dynamically
  const _syncInterval = setInterval(() => {
    observeChatSoundToggle();
    if (document.getElementById('sound-enabled')) clearInterval(_syncInterval);
  }, 2000);

  // ── Init ──
  applySettings(load());
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => { injectGearButton(); observeChatSoundToggle(); });
  } else {
    injectGearButton();
    observeChatSoundToggle();
  }
})();
