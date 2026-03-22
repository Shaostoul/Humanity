// ── chat-voice-streaming.js ───────────────────────────────────────────────
// Live streaming features:
//   - Stream sidebar (watch/hide toggle, auto-watch preference)
//   - AFK overlay + timer
//   - BRB overlay + timer
//   - Studio metrics (bitrate, FPS, resolution, RTT, packet loss, health)
//   - Bitrate control slider
//   - Scene presets (save/load/delete studio configurations)
//
// Depends on: chat-voice-rooms.js (window._roomPeerConnections,
//   window._currentRoomId, setupRoomAudio, cleanupRoomAudio, addSystemMessage,
//   hosIcon, esc)
// Depends on: chat-voice-webrtc.js (streamChatOverlayEnabled,
//   streamChatOverlayChannel, vrVideoStream, vrScreenStream)
// ─────────────────────────────────────────────────────────────────────────

// ── Stream/Watch State ──
let autoWatchStreams = localStorage.getItem('humanity-auto-watch-streams') === 'true';
window.autoWatchStreams = autoWatchStreams;
const activeStreams = new Map(); // id -> { name, wrapper, video, hidden, peerKey }
window.activeStreams = activeStreams;

function toggleAutoWatchStreams(enabled) {
  autoWatchStreams = !!enabled;
  window.autoWatchStreams = autoWatchStreams;
  localStorage.setItem('humanity-auto-watch-streams', enabled ? 'true' : 'false');
  activeStreams.forEach(s => {
    if (enabled && s.hidden) {
      s.hidden = false;
      s.wrapper.style.display = '';
    }
  });
  renderStreamSidebar();
}
window.toggleAutoWatchStreams = toggleAutoWatchStreams;

function renderStreamSidebar() {
  const list = document.getElementById('stream-list');
  const checkbox = document.getElementById('stream-auto-watch');
  if (!list) return;
  if (checkbox) checkbox.checked = autoWatchStreams;

  if (activeStreams.size === 0) {
    list.className = 'stream-empty';
    list.textContent = 'No active streams';
    return;
  }

  list.className = '';
  list.innerHTML = '';
  activeStreams.forEach((s, id) => {
    const row = document.createElement('div');
    row.className = 'stream-row';
    const title = document.createElement('span');
    title.textContent = s.name || id;
    const btn = document.createElement('button');
    btn.textContent = s.hidden ? 'Watch' : 'Hide';
    btn.onclick = () => {
      s.hidden = !s.hidden;
      s.wrapper.style.display = s.hidden ? 'none' : '';
      btn.textContent = s.hidden ? 'Watch' : 'Hide';
    };
    row.appendChild(title);
    row.appendChild(btn);
    list.appendChild(row);
  });
}
setTimeout(renderStreamSidebar, 0);

// ── Studio: AFK Overlay ──

let studioAfkActive = false;
let studioAfkTimer = null;
let studioAfkStartTime = null;

/**
 * Toggles an AFK overlay on the stream preview, showing elapsed away time.
 * Intended as a courtesy indicator for viewers — press when stepping away.
 */
window.toggleStudioAfk = function() {
  const btn = document.getElementById('studio-afk-btn');
  const preview = document.getElementById('stream-studio-preview');
  studioAfkActive = !studioAfkActive;
  if (studioAfkActive) {
    studioAfkStartTime = Date.now();
    if (btn) { btn.textContent = '🌙 AFK — On'; btn.classList.add('active'); btn.classList.remove('vc-muted'); }
    if (preview) {
      let ov = preview.querySelector('.studio-afk-overlay');
      if (!ov) {
        ov = document.createElement('div');
        ov.className = 'studio-afk-overlay';
        preview.appendChild(ov);
      }
      ov.textContent = '💤 AFK — 0:00';
      studioAfkTimer = setInterval(() => {
        const elapsed = Math.floor((Date.now() - studioAfkStartTime) / 1000);
        const m = Math.floor(elapsed / 60);
        const s = String(elapsed % 60).padStart(2, '0');
        ov.textContent = `💤 AFK — ${m}:${s}`;
      }, 1000);
    }
  } else {
    studioAfkStartTime = null;
    if (studioAfkTimer) { clearInterval(studioAfkTimer); studioAfkTimer = null; }
    if (btn) { btn.textContent = '🌙 AFK — Off'; btn.classList.remove('active', 'vc-muted'); }
    if (preview) { const ov = preview.querySelector('.studio-afk-overlay'); if (ov) ov.remove(); }
  }
};

// ── Studio Metrics ──
// Polls WebRTC stats every 2s and updates the metrics bar in the studio panel.
let studioMetricsInterval = null;
let studioMetricsPrev = { bytesSent: 0, timestamp: 0 };

function startStudioMetrics() {
  if (studioMetricsInterval) return;
  const bar = document.getElementById('studio-metrics-bar');
  if (bar) bar.style.display = '';
  studioMetricsInterval = setInterval(collectStudioMetrics, 2000);
}

function stopStudioMetrics() {
  if (studioMetricsInterval) { clearInterval(studioMetricsInterval); studioMetricsInterval = null; }
  const bar = document.getElementById('studio-metrics-bar');
  if (bar) bar.style.display = 'none';
  studioMetricsPrev = { bytesSent: 0, timestamp: 0 };
}

async function collectStudioMetrics() {
  const pcs = window._roomPeerConnections || {};
  const pcList = Object.values(pcs);
  const viewerCount = pcList.length;
  document.getElementById('studio-m-viewers').textContent = viewerCount;

  if (!pcList.length) {
    ['studio-m-bitrate','studio-m-fps','studio-m-res','studio-m-rtt','studio-m-loss'].forEach(id => {
      const el = document.getElementById(id);
      if (el) el.textContent = '—';
    });
    return;
  }

  // Aggregate stats from first PC that has outbound video
  let totalBytes = 0, fps = 0, width = 0, height = 0, rtt = 0, lostTotal = 0, sentTotal = 0;
  for (const pc of pcList) {
    try {
      const stats = await pc.getStats();
      for (const [, report] of stats) {
        if (report.type === 'outbound-rtp' && report.kind === 'video') {
          totalBytes += report.bytesSent || 0;
          fps = report.framesPerSecond || fps;
          width = report.frameWidth || width;
          height = report.frameHeight || height;
        }
        if (report.type === 'remote-inbound-rtp' && report.kind === 'video') {
          rtt = report.roundTripTime || rtt;
          lostTotal += report.packetsLost || 0;
          sentTotal += report.packetsSent || report.packetsReceived || 0;
        }
        if (report.type === 'candidate-pair' && report.state === 'succeeded') {
          rtt = report.currentRoundTripTime || rtt;
        }
      }
    } catch (_) {}
  }

  // Bitrate calculation
  const now = Date.now();
  if (studioMetricsPrev.timestamp) {
    const dt = (now - studioMetricsPrev.timestamp) / 1000;
    const bps = ((totalBytes - studioMetricsPrev.bytesSent) * 8) / dt;
    const kbps = Math.round(bps / 1000);
    document.getElementById('studio-m-bitrate').textContent = kbps > 1000 ? (kbps / 1000).toFixed(1) + ' Mbps' : kbps + ' kbps';
  }
  studioMetricsPrev = { bytesSent: totalBytes, timestamp: now };

  document.getElementById('studio-m-fps').textContent = fps ? Math.round(fps) + ' fps' : '—';
  document.getElementById('studio-m-res').textContent = width ? width + 'x' + height : '—';
  document.getElementById('studio-m-rtt').textContent = rtt ? Math.round(rtt * 1000) + ' ms' : '—';

  const lossPercent = sentTotal > 0 ? ((lostTotal / sentTotal) * 100).toFixed(1) : '0.0';
  document.getElementById('studio-m-loss').textContent = lossPercent + '%';

  // Health indicator
  const healthEl = document.getElementById('studio-m-health');
  if (healthEl) {
    if (rtt > 0.3 || parseFloat(lossPercent) > 5) healthEl.textContent = '🔴';
    else if (rtt > 0.1 || parseFloat(lossPercent) > 1) healthEl.textContent = '🟡';
    else healthEl.textContent = '🟢';
  }
}

// Auto-start metrics when joining a voice room, stop when leaving
const _origSetupRoomAudioMetrics = window.setupRoomAudio;
window.setupRoomAudio = async function() {
  await _origSetupRoomAudioMetrics();
  startStudioMetrics();
};
const _origCleanupRoomAudioMetrics = window.cleanupRoomAudio;
window.cleanupRoomAudio = function() {
  _origCleanupRoomAudioMetrics();
  stopStudioMetrics();
};

// ── BRB Timer ──
let studioBrbActive = false;
let studioBrbTimer = null;
let studioBrbStartTime = null;

window.toggleStudioBrb = function() {
  const btn = document.getElementById('studio-brb-btn');
  const preview = document.getElementById('stream-studio-preview');
  studioBrbActive = !studioBrbActive;
  if (studioBrbActive) {
    // Turn off AFK if it's on
    if (studioAfkActive) window.toggleStudioAfk();
    studioBrbStartTime = Date.now();
    if (btn) { btn.textContent = '⏸️ BRB — On'; btn.classList.add('active'); }
    if (preview) {
      let ov = preview.querySelector('.studio-brb-overlay');
      if (!ov) {
        ov = document.createElement('div');
        ov.className = 'studio-brb-overlay';
        preview.appendChild(ov);
      }
      ov.textContent = '⏸️ BRB — 0:00';
      studioBrbTimer = setInterval(() => {
        const elapsed = Math.floor((Date.now() - studioBrbStartTime) / 1000);
        const m = Math.floor(elapsed / 60);
        const s = String(elapsed % 60).padStart(2, '0');
        ov.textContent = `⏸️ BRB — ${m}:${s}`;
      }, 1000);
    }
  } else {
    studioBrbStartTime = null;
    if (studioBrbTimer) { clearInterval(studioBrbTimer); studioBrbTimer = null; }
    if (btn) { btn.textContent = '⏸️ BRB — Off'; btn.classList.remove('active'); }
    if (preview) { const ov = preview.querySelector('.studio-brb-overlay'); if (ov) ov.remove(); }
  }
};

// ── Bitrate Control ──
window.setStudioBitrate = function(kbps) {
  const val = parseInt(kbps, 10);
  localStorage.setItem('humanity-studio-bitrate', String(val));
  // Apply to all outgoing video senders
  const pcs = window._roomPeerConnections || {};
  for (const pc of Object.values(pcs)) {
    try {
      pc.getSenders().forEach(sender => {
        if (sender.track && sender.track.kind === 'video') {
          const params = sender.getParameters();
          if (!params.encodings || !params.encodings.length) params.encodings = [{}];
          params.encodings[0].maxBitrate = val * 1000;
          sender.setParameters(params);
        }
      });
    } catch (_) {}
  }
};

// Restore saved bitrate on load
try {
  const savedBitrate = localStorage.getItem('humanity-studio-bitrate');
  if (savedBitrate) {
    const slider = document.getElementById('studio-bitrate-slider');
    if (slider) slider.value = savedBitrate;
  }
} catch (_) {}

// ── Scene Presets ──
const SCENES_KEY = 'humanity-studio-scenes';

window.openSceneManager = function() {
  const scenes = JSON.parse(localStorage.getItem(SCENES_KEY) || '[]');
  let html = '<div style="padding:var(--space-md);">';
  html += '<p style="font-size:0.75rem;color:var(--text-muted);margin-bottom:var(--space-md);">Save your current studio setup as a scene, or load a saved one.</p>';

  if (scenes.length) {
    html += scenes.map((s, i) => `
      <div style="display:flex;align-items:center;gap:var(--space-md);padding:var(--space-sm) 0;border-bottom:1px solid var(--border);font-size:0.78rem;">
        <span style="flex:1;color:var(--text);">${s.name}</span>
        <button onclick="loadScene(${i})" style="background:var(--accent-dim);border:1px solid var(--accent);color:var(--accent);padding:var(--space-xs) var(--space-md);border-radius:var(--radius-sm);cursor:pointer;font-size:0.72rem;">Load</button>
        <button onclick="deleteScene(${i})" style="background:transparent;border:1px solid var(--danger);color:var(--danger);padding:var(--space-xs) var(--space-sm);border-radius:var(--radius-sm);cursor:pointer;font-size:0.72rem;">✕</button>
      </div>
    `).join('');
  } else {
    html += '<p style="font-size:0.78rem;color:var(--text-muted);">No saved scenes yet.</p>';
  }

  html += '<div style="display:flex;gap:var(--space-md);margin-top:var(--space-md);">';
  html += '<input type="text" id="scene-name-input" placeholder="Scene name" style="flex:1;padding:var(--space-sm) var(--space-md);background:var(--bg-input);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--text);font-size:0.78rem;">';
  html += '<button onclick="saveCurrentScene()" style="background:var(--accent);border:none;color:#fff;padding:var(--space-sm) var(--space-lg);border-radius:var(--radius-sm);cursor:pointer;font-size:0.78rem;">Save Current</button>';
  html += '</div></div>';

  // Use a simple overlay
  let overlay = document.getElementById('scene-manager-overlay');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'scene-manager-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.onclick = function(e) { if (e.target === overlay) overlay.classList.remove('open'); };
    overlay.innerHTML = '<div class="profile-modal" onclick="event.stopPropagation()" style="max-width:380px;"><button class="close-btn" onclick="document.getElementById(\'scene-manager-overlay\').classList.remove(\'open\')">✕</button><h2>' + hosIcon('film', 16) + ' Studio Scenes</h2><div id="scene-manager-body"></div></div>';
    document.body.appendChild(overlay);
  }
  document.getElementById('scene-manager-body').innerHTML = html;
  overlay.classList.add('open');
};

window.saveCurrentScene = function() {
  const nameInput = document.getElementById('scene-name-input');
  const name = nameInput ? nameInput.value.trim() : '';
  if (!name) { alert('Enter a scene name.'); return; }
  const scene = {
    name,
    micDevice: document.getElementById('studio-mic-select')?.value || '',
    bitrate: document.getElementById('studio-bitrate-slider')?.value || '2500',
    pipSize: localStorage.getItem('humanity-studio-pip-width') || '34',
    chatOverlay: streamChatOverlayEnabled,
    chatChannel: streamChatOverlayChannel || 'general',
  };
  const scenes = JSON.parse(localStorage.getItem(SCENES_KEY) || '[]');
  scenes.push(scene);
  localStorage.setItem(SCENES_KEY, JSON.stringify(scenes));
  window.openSceneManager(); // re-render
};

window.loadScene = function(idx) {
  const scenes = JSON.parse(localStorage.getItem(SCENES_KEY) || '[]');
  const s = scenes[idx];
  if (!s) return;
  if (s.micDevice) { const sel = document.getElementById('studio-mic-select'); if (sel) { sel.value = s.micDevice; window.setStudioMic(s.micDevice); } }
  if (s.bitrate) { const sl = document.getElementById('studio-bitrate-slider'); if (sl) { sl.value = s.bitrate; window.setStudioBitrate(s.bitrate); } }
  if (s.pipSize) window.setStudioPipSize(s.pipSize);
  if (s.chatOverlay && !streamChatOverlayEnabled) { streamChatOverlayChannel = s.chatChannel; window.toggleStreamChatOverlay(); }
  else if (!s.chatOverlay && streamChatOverlayEnabled) window.toggleStreamChatOverlay();
  document.getElementById('scene-manager-overlay')?.classList.remove('open');
  if (typeof addSystemMessage === 'function') addSystemMessage('Scene loaded: ' + s.name);
};

window.deleteScene = function(idx) {
  const scenes = JSON.parse(localStorage.getItem(SCENES_KEY) || '[]');
  scenes.splice(idx, 1);
  localStorage.setItem(SCENES_KEY, JSON.stringify(scenes));
  window.openSceneManager(); // re-render
};
