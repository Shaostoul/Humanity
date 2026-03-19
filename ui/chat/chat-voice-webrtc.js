// ── chat-voice-webrtc.js ──────────────────────────────────────────────────
// WebRTC plumbing shared by rooms and 1-on-1 calls:
//   - DM video start/stop, screen share start/stop
//   - Voice room video + screen share
//   - Video panel helpers (show/remove/update, PiP, drag-resize)
//   - Camera selection + preferred device
//   - Connection quality monitoring (RTT badges)
//   - Mic device selection + preferred mic
//
// Depends on: chat-voice-rooms.js (window._roomPeerConnections,
//   window._roomLocalStream, window._currentRoomId, cleanupRoomAudio,
//   connectToRoomPeer, setupRoomAudio, renderStreamSidebar, activeStreams,
//   autoWatchStreams, shortKey, hosIcon, addSystemMessage, esc)
// Depends on: chat-voice-calls.js (peerConnection, callPeerKey, callPeerName,
//   callState, cleanupCall, resetCallState, showCallBar, setupPeerConnection,
//   localStream, isMuted)
// ─────────────────────────────────────────────────────────────────────────

// ── DM Call Video ──
let dmVideoStream = null;
let dmScreenStream = null;
let dmVideoActive = false;
let dmScreenActive = false;
let streamChatOverlayEnabled = localStorage.getItem('humanity-stream-chat-overlay') === 'true';
let streamChatOverlayChannel = localStorage.getItem('humanity-stream-chat-channel') || 'general';

function toggleVideo() {
  if (!peerConnection) return;
  if (dmVideoActive) {
    stopDmVideo();
  } else {
    startDmVideo();
  }
}

async function startDmVideo() {
  if (!peerConnection) return;
  try {
    // Stop screen share if active
    if (dmScreenActive) stopDmScreenShare();
    dmVideoStream = await navigator.mediaDevices.getUserMedia({ video: getCameraConstraints(), audio: false });
    // Remember the selected camera
    const usedTrack = dmVideoStream.getVideoTracks()[0];
    if (usedTrack && usedTrack.getSettings().deviceId) setPreferredCamera(usedTrack.getSettings().deviceId);
    const videoTrack = dmVideoStream.getVideoTracks()[0];
    const sender = peerConnection.getSenders().find(s => s.track && s.track.kind === 'video');
    if (sender) {
      await sender.replaceTrack(videoTrack);
    } else {
      peerConnection.addTrack(videoTrack, dmVideoStream);
    }
    dmVideoActive = true;
    document.getElementById('video-btn').classList.add('active');
    document.getElementById('video-btn').innerHTML = hosIcon('video', 16) + ' On';
    showLocalVideo(dmVideoStream, 'dm-self');
  } catch (e) {
    addSystemMessage('⚠️ Camera access denied.');
  }
}

function stopDmVideo() {
  if (dmVideoStream) {
    dmVideoStream.getTracks().forEach(t => t.stop());
    dmVideoStream = null;
  }
  // Remove video sender
  if (peerConnection) {
    const sender = peerConnection.getSenders().find(s => s.track && s.track.kind === 'video');
    if (sender) { try { peerConnection.removeTrack(sender); } catch(e){} }
  }
  dmVideoActive = false;
  document.getElementById('video-btn').classList.remove('active');
  document.getElementById('video-btn').innerHTML = hosIcon('video', 16) + ' Video';
  removeVideoElement('dm-self');
  updateVideoPanel();
}

async function toggleScreenShare() {
  if (!peerConnection) return;
  if (dmScreenActive) {
    stopDmScreenShare();
  } else {
    startDmScreenShare();
  }
}

async function startDmScreenShare() {
  if (!peerConnection) return;
  try {
    if (dmVideoActive) stopDmVideo();
    dmScreenStream = await navigator.mediaDevices.getDisplayMedia({ video: true });
    const videoTrack = dmScreenStream.getVideoTracks()[0];
    videoTrack.addEventListener('ended', () => { stopDmScreenShare(); });
    const sender = peerConnection.getSenders().find(s => s.track && s.track.kind === 'video');
    if (sender) {
      await sender.replaceTrack(videoTrack);
    } else {
      peerConnection.addTrack(videoTrack, dmScreenStream);
    }
    dmScreenActive = true;
    document.getElementById('screen-btn').classList.add('active');
    document.getElementById('screen-btn').innerHTML = hosIcon('monitor', 16) + ' On';
    showLocalVideo(dmScreenStream, 'dm-screen');
  } catch (e) {
    // User cancelled the screen share picker
  }
}

function stopDmScreenShare() {
  if (dmScreenStream) {
    dmScreenStream.getTracks().forEach(t => t.stop());
    dmScreenStream = null;
  }
  if (peerConnection) {
    const sender = peerConnection.getSenders().find(s => s.track && s.track.kind === 'video');
    if (sender) { try { peerConnection.removeTrack(sender); } catch(e){} }
  }
  dmScreenActive = false;
  const btn = document.getElementById('screen-btn');
  if (btn) { btn.classList.remove('active'); btn.innerHTML = hosIcon('monitor', 16) + ' Screen'; }
  removeVideoElement('dm-screen');
  updateVideoPanel();
}

// Patch cleanupCall to also clean up video
const _origCleanupCall = cleanupCall;
cleanupCall = function() {
  stopDmVideo();
  stopDmScreenShare();
  // Remove all remote video
  document.querySelectorAll('#video-panel .video-wrapper').forEach(el => el.remove());
  updateVideoPanel();
  _origCleanupCall();
};

// Patch resetCallState to reset video buttons
const _origResetCallState = resetCallState;
resetCallState = function() {
  _origResetCallState();
  const vb = document.getElementById('video-btn');
  if (vb) { vb.classList.remove('active'); vb.innerHTML = hosIcon('video', 16) + ' Video'; }
  const sb = document.getElementById('screen-btn');
  if (sb) { sb.classList.remove('active'); sb.innerHTML = hosIcon('monitor', 16) + ' Screen'; }
};

// Patch peerConnection.ontrack to handle video tracks
const _origSetupPeerConnection = setupPeerConnection;
setupPeerConnection = async function(isCaller) {
  const result = await _origSetupPeerConnection(isCaller);
  if (!result || !peerConnection) return result;
  const origOnTrack = peerConnection.ontrack;
  peerConnection.ontrack = function(event) {
    if (event.track.kind === 'video') {
      showRemoteVideo(event.streams[0], 'dm-remote-' + event.track.id, callPeerName || 'Peer');
    } else {
      if (origOnTrack) origOnTrack.call(this, event);
    }
  };
  return result;
};

// ── Voice Room Video ──
let vrVideoStream = null;
let vrScreenStream = null;
let vrVideoActive = false;
let vrScreenActive = false;

window.toggleVoiceRoomVideo = async function() {
  // No voice-room guard — camera preview works standalone; tracks are added to peers only if in a room.
  if (vrVideoActive) {
    stopVrVideo();
  } else {
    await startVrVideo();
  }
};

async function startVrVideo() {
  try {
    vrVideoStream = await navigator.mediaDevices.getUserMedia({ video: getCameraConstraints(), audio: false });
    const usedVrTrack = vrVideoStream.getVideoTracks()[0];
    if (usedVrTrack && usedVrTrack.getSettings().deviceId) setPreferredCamera(usedVrTrack.getSettings().deviceId);
    const videoTrack = vrVideoStream.getVideoTracks()[0];
    // Add video track to all peer connections
    for (const [key, pc] of Object.entries(window._roomPeerConnections)) {
      pc.addTrack(videoTrack, vrVideoStream);
    }
    vrVideoActive = true;
    const btn = document.getElementById('vc-video-btn');
    if (btn) {
      btn.classList.add('active');
      btn.classList.remove('vc-muted');
      // Show the active camera's label if known, otherwise just "On".
      const camLabel = localStorage.getItem('humanity-preferred-camera-label') || 'On';
      btn.innerHTML = hosIcon('video', 16) + ' Camera — ' + camLabel;
    }
    showLocalVideo(vrVideoStream, 'vr-self');
  } catch (e) {
    addSystemMessage('⚠️ Camera access denied.');
  }
}

function stopVrVideo() {
  if (vrVideoStream) {
    const videoTrack = vrVideoStream.getVideoTracks()[0];
    for (const [key, pc] of Object.entries(window._roomPeerConnections)) {
      const sender = pc.getSenders().find(s => s.track === videoTrack);
      if (sender) { try { pc.removeTrack(sender); } catch(e){} }
    }
    vrVideoStream.getTracks().forEach(t => t.stop());
    vrVideoStream = null;
  }
  vrVideoActive = false;
  const btn = document.getElementById('vc-video-btn');
  if (btn) { btn.classList.remove('active', 'vc-muted'); btn.innerHTML = hosIcon('video', 16) + ' Camera — Off'; }
  removeVideoElement('vr-self');
  updateStudioLayout();
  updateStudioPreviewPanel();
  updateVideoPanel();
}

window.toggleVoiceRoomScreenShare = async function() {
  // No voice-room guard — screen share preview works standalone; tracks added to peers only if in a room.
  if (vrScreenActive) {
    stopVrScreenShare();
  } else {
    await startVrScreenShare();
  }
};

async function startVrScreenShare() {
  try {
    vrScreenStream = await navigator.mediaDevices.getDisplayMedia({ video: true });
    const videoTrack = vrScreenStream.getVideoTracks()[0];
    videoTrack.addEventListener('ended', () => { stopVrScreenShare(); });
    for (const [key, pc] of Object.entries(window._roomPeerConnections)) {
      pc.addTrack(videoTrack, vrScreenStream);
    }
    vrScreenActive = true;
    const btn = document.getElementById('vc-screen-btn');
    if (btn) { btn.classList.add('active'); btn.classList.remove('vc-muted'); btn.innerHTML = hosIcon('monitor', 16) + ' Screen — Active'; }
    showLocalVideo(vrScreenStream, 'vr-screen');
  } catch (e) {
    // User cancelled
  }
}

function stopVrScreenShare() {
  if (vrScreenStream) {
    const videoTrack = vrScreenStream.getVideoTracks()[0];
    for (const [key, pc] of Object.entries(window._roomPeerConnections)) {
      const sender = pc.getSenders().find(s => s.track === videoTrack);
      if (sender) { try { pc.removeTrack(sender); } catch(e){} }
    }
    vrScreenStream.getTracks().forEach(t => t.stop());
    vrScreenStream = null;
  }
  vrScreenActive = false;
  const btn = document.getElementById('vc-screen-btn');
  if (btn) { btn.classList.remove('active', 'vc-muted'); btn.innerHTML = hosIcon('monitor', 16) + ' Screen — Off'; }
  removeVideoElement('vr-screen');
  updateStudioLayout();
  updateStudioPreviewPanel();
  updateVideoPanel();
}

// Patch cleanupRoomAudio to stop video too
const _origCleanupRoomAudio2 = window.cleanupRoomAudio;
window.cleanupRoomAudio = function() {
  stopVrVideo();
  stopVrScreenShare();
  document.querySelectorAll('#video-panel .video-wrapper:not([data-id^="dm-"])').forEach(el => el.remove());
  const ov = document.querySelector('#video-panel .stream-chat-overlay');
  if (ov) ov.remove();
  updateStudioPreviewPanel();
  updateVideoPanel();
  _origCleanupRoomAudio2();
};

// Patch connectToRoomPeer to handle remote video tracks
const _origConnectToRoomPeer2 = window.connectToRoomPeer;
window.connectToRoomPeer = async function(peerKey, peerName, roomId, isCaller) {
  await _origConnectToRoomPeer2(peerKey, peerName, roomId, isCaller);
  const pc = window._roomPeerConnections[peerKey];
  if (!pc) return;
  const origOnTrack = pc.ontrack;
  pc.ontrack = function(event) {
    if (event.track.kind === 'video') {
      const label = peerName || shortKey(peerKey);
      const remoteId = 'vr-remote-' + peerKey + '-' + event.track.id;
      showRemoteVideo(event.streams[0], remoteId, label);
      event.track.addEventListener('ended', () => {
        removeVideoElement(remoteId);
        updateVideoPanel();
      });
    } else {
      if (origOnTrack) origOnTrack.call(this, event);
    }
  };
};

// ── Video Panel Helpers ──
function makeStudioDragResize(wrapper, storageKey) {
  wrapper.style.resize = 'both';
  wrapper.style.overflow = 'hidden';
  let dragging = false;
  let ox = 0, oy = 0;
  const label = wrapper.querySelector('.video-label');
  if (!label) return;
  label.style.cursor = 'move';
  label.addEventListener('pointerdown', (e) => {
    dragging = true;
    const rect = wrapper.getBoundingClientRect();
    ox = e.clientX - rect.left;
    oy = e.clientY - rect.top;
    wrapper.setPointerCapture(e.pointerId);
  });
  label.addEventListener('pointermove', (e) => {
    if (!dragging) return;
    wrapper.style.left = Math.max(8, e.clientX - ox) + 'px';
    wrapper.style.top = Math.max(8, e.clientY - oy) + 'px';
    wrapper.style.right = 'auto';
    wrapper.style.bottom = 'auto';
  });
  label.addEventListener('pointerup', (e) => {
    dragging = false;
    try {
      localStorage.setItem(storageKey, JSON.stringify({
        left: wrapper.style.left || '',
        top: wrapper.style.top || '',
        width: wrapper.style.width || '',
        height: wrapper.style.height || ''
      }));
    } catch (_) {}
  });
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) || '{}');
    if (saved.left) wrapper.style.left = saved.left;
    if (saved.top) wrapper.style.top = saved.top;
    if (saved.width) wrapper.style.width = saved.width;
    if (saved.height) wrapper.style.height = saved.height;
    if (saved.left || saved.top) {
      wrapper.style.right = 'auto';
      wrapper.style.bottom = 'auto';
    }
  } catch (_) {}
}

function updateStudioLayout() {
  const panel = document.getElementById('video-panel');
  if (!panel) return;
  const cam = panel.querySelector('.video-wrapper[data-id="vr-self"]');
  const scr = panel.querySelector('.video-wrapper[data-id="vr-screen"]');
  [cam, scr].forEach(w => { if (w) { w.classList.remove('studio-main', 'studio-pip'); w.style.position=''; } });

  if (scr) {
    scr.classList.add('studio-main');
    scr.style.position = 'relative';
  }
  if (cam) {
    if (scr) {
      cam.classList.add('studio-pip');
      cam.style.position = 'absolute';
      cam.style.right = cam.style.right || '10px';
      cam.style.bottom = cam.style.bottom || '10px';
      makeStudioDragResize(cam, 'humanity-studio-cam-pip');
    } else {
      cam.classList.add('studio-main');
      cam.style.position = 'relative';
    }
  }
  ensureStreamChatOverlay();
}

function ensureStreamChatOverlay() {
  const panel = document.getElementById('video-panel');
  if (!panel) return;
  let ov = panel.querySelector('.stream-chat-overlay');
  if (!streamChatOverlayEnabled) {
    if (ov) ov.remove();
    return;
  }
  if (!ov) {
    ov = document.createElement('div');
    ov.className = 'stream-chat-overlay';
    panel.appendChild(ov);
    makeStudioDragResize(ov, 'humanity-studio-chat-overlay');
  }
  ov.innerHTML = `<div class="video-label">Chat Overlay · #${streamChatOverlayChannel}</div><div class="stream-chat-overlay-body">Chat overlay enabled for #${streamChatOverlayChannel}. (Live channel feed integration in progress)</div>`;
}

function updateStudioPreviewPanel() {
  const panel = document.getElementById('stream-studio-preview');
  if (!panel) return;
  panel.innerHTML = '';

  if (!vrVideoStream && !vrScreenStream) {
    panel.textContent = 'No active local feed';
    return;
  }

  if (vrScreenStream) {
    const s = document.createElement('video');
    s.autoplay = true; s.playsInline = true; s.muted = true;
    s.srcObject = vrScreenStream;
    panel.appendChild(s);
    const label = document.createElement('div');
    label.className = 'studio-label';
    label.textContent = 'Screen';
    panel.appendChild(label);
  }

  if (vrVideoStream) {
    const cWrap = document.createElement('div');
    cWrap.className = vrScreenStream ? 'studio-cam-pip' : '';
    const c = document.createElement('video');
    c.autoplay = true; c.playsInline = true; c.muted = true;
    c.srcObject = vrVideoStream;
    c.style.objectFit = 'cover';
    cWrap.appendChild(c);
    panel.appendChild(cWrap);
    if (!vrScreenStream) {
      const label = document.createElement('div');
      label.className = 'studio-label';
      label.textContent = 'Camera';
      panel.appendChild(label);
    }
  }

  if (streamChatOverlayEnabled) {
    const ov = document.createElement('div');
    ov.className = 'studio-chat-overlay';
    ov.textContent = `Chat Overlay: #${streamChatOverlayChannel}`;
    panel.appendChild(ov);
  }
}

window.setStudioPipSize = function(v) {
  const p = Math.max(20, Math.min(60, parseInt(v || '34', 10)));
  document.documentElement.style.setProperty('--studio-pip-width', p + '%');
  localStorage.setItem('humanity-studio-pip-width', String(p));
};

try {
  const savedPip = parseInt(localStorage.getItem('humanity-studio-pip-width') || '34', 10);
  if (!Number.isNaN(savedPip)) window.setStudioPipSize(savedPip);
} catch (_) {}

window.toggleStreamChatOverlay = function() {
  if (!streamChatOverlayEnabled && !streamChatOverlayChannel) {
    // First enable: prompt for channel name.
    const ch = prompt('Enter channel for chat overlay:', 'general');
    if (!ch) return;
    streamChatOverlayChannel = ch.trim();
    localStorage.setItem('humanity-stream-chat-channel', streamChatOverlayChannel);
  }
  streamChatOverlayEnabled = !streamChatOverlayEnabled;
  localStorage.setItem('humanity-stream-chat-overlay', streamChatOverlayEnabled ? 'true' : 'false');
  ensureStreamChatOverlay();
  updateStudioPreviewPanel();
  const btn = document.getElementById('vc-chat-overlay-btn');
  if (btn) {
    if (streamChatOverlayEnabled) {
      btn.classList.add('active');
      btn.innerHTML = hosIcon('chat', 16) + ' Overlay — #' + (streamChatOverlayChannel || 'general');
    } else {
      btn.classList.remove('active');
      btn.innerHTML = hosIcon('chat', 16) + ' Overlay — Off';
    }
  }
};

window.selectStreamChatChannel = function() {
  const ch = prompt('Enter channel id/name for stream chat overlay:', streamChatOverlayChannel || 'general');
  if (!ch) return;
  streamChatOverlayChannel = ch.trim();
  localStorage.setItem('humanity-stream-chat-channel', streamChatOverlayChannel);
  ensureStreamChatOverlay();
  updateStudioPreviewPanel();
  const btn = document.getElementById('vc-chat-overlay-btn');
  if (btn && streamChatOverlayEnabled) btn.innerHTML = hosIcon('chat', 16) + ' Overlay — #' + streamChatOverlayChannel;
};

function showLocalVideo(stream, id) {
  if (id === 'vr-self' || id === 'vr-screen') {
    // Local VR feeds are rendered in the right-panel Stream Studio preview.
    removeVideoElement('vr-self');
    removeVideoElement('vr-screen');
    updateStudioPreviewPanel();
    return;
  }
  removeVideoElement(id);
  const panel = document.getElementById('video-panel');
  const wrapper = document.createElement('div');
  wrapper.className = 'video-wrapper self-view';
  wrapper.dataset.id = id;
  const video = document.createElement('video');
  video.srcObject = stream;
  video.autoplay = true;
  video.playsInline = true;
  video.muted = true;
  video.style.transform = 'none';
  video.style.objectFit = id.includes('screen') ? 'contain' : 'cover';
  if (id.includes('screen')) wrapper.classList.add('local-screen-view');
  const label = document.createElement('div');
  label.className = 'video-label';
  label.textContent = id.includes('screen') ? 'You (Screen)' : 'You (Camera)';
  wrapper.appendChild(video);
  wrapper.appendChild(label);
  panel.appendChild(wrapper);
  updateStudioLayout();
  updateVideoPanel();
}

function showRemoteVideo(stream, id, name) {
  removeVideoElement(id);
  const panel = document.getElementById('video-panel');
  const wrapper = document.createElement('div');
  wrapper.className = 'video-wrapper';
  wrapper.dataset.id = id;
  const video = document.createElement('video');
  video.srcObject = stream;
  video.autoplay = true;
  video.playsInline = true;
  const label = document.createElement('div');
  label.className = 'video-label';
  label.textContent = name;
  const pipBtn = document.createElement('button');
  pipBtn.className = 'video-pip-btn';
  pipBtn.innerHTML = hosIcon('pin', 14);
  pipBtn.title = 'Pin/Unpin stream';
  pipBtn.onclick = () => {
    wrapper.classList.toggle('pinned-inapp');
    pipBtn.innerHTML = wrapper.classList.contains('pinned-inapp') ? '🗗' : hosIcon('pin', 14);
  };
  wrapper.appendChild(video);
  wrapper.appendChild(label);
  wrapper.appendChild(pipBtn);
  panel.appendChild(wrapper);
  video.play().catch(() => {});

  const hidden = !window.autoWatchStreams;
  if (hidden) wrapper.style.display = 'none';
  window.activeStreams.set(id, { name: name || id, wrapper, video, hidden });
  if (typeof renderStreamSidebar === 'function') renderStreamSidebar();
  updateVideoPanel();
}

function removeVideoElement(id) {
  const el = document.querySelector(`#video-panel .video-wrapper[data-id="${id}"]`);
  if (el) el.remove();
  window.activeStreams.delete(id);
  if (typeof renderStreamSidebar === 'function') renderStreamSidebar();
}

function updateVideoPanel() {
  const panel = document.getElementById('video-panel');
  const wrappers = panel.querySelectorAll('.video-wrapper');
  const hasVideos = wrappers.length > 0;
  panel.classList.toggle('active', hasVideos);
  // Single-remote mode for 1-on-1 calls (1 remote + optional self)
  const remotes = panel.querySelectorAll('.video-wrapper:not(.self-view)');
  panel.classList.toggle('single-remote', remotes.length === 1);
  // Gallery mode for 3+ videos
  panel.classList.toggle('gallery', wrappers.length >= 3);
}

// ── Picture-in-Picture ──
function togglePiP() {
  // In-app pin mode avoids browser PiP settings-page issues in desktop wrappers.
  const wrapper = document.querySelector('#video-panel .video-wrapper:not(.self-view)');
  if (!wrapper) {
    addSystemMessage('ℹ️ No remote video to display.');
    return;
  }
  wrapper.classList.toggle('pinned-inapp');
  addSystemMessage(wrapper.classList.contains('pinned-inapp') ? '📌 Stream pinned in-app.' : '📌 Stream unpinned.');
}

// ── Camera Selection ──
async function getVideoDevices() {
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices.filter(d => d.kind === 'videoinput');
  } catch (e) { return []; }
}

function getPreferredCamera() {
  return localStorage.getItem('humanity-preferred-camera') || null;
}
function setPreferredCamera(deviceId) {
  localStorage.setItem('humanity-preferred-camera', deviceId);
}

function getCameraConstraints() {
  const preferred = getPreferredCamera();
  const video = { width: 640, height: 480 };
  if (preferred) video.deviceId = { ideal: preferred };
  return video;
}

async function showCameraSelector(context) {
  const selectorId = context === 'dm' ? 'camera-selector-dm' : null;
  // Create inline selector near the button
  let selector = selectorId ? document.getElementById(selectorId) : null;
  if (!selector) {
    // For voice room, create a temporary popup
    selector = document.createElement('div');
    selector.className = 'camera-selector';
    selector.style.position = 'fixed';
    selector.style.bottom = '60px';
    selector.style.right = '20px';
    document.body.appendChild(selector);
    setTimeout(() => { if (selector.parentNode) selector.parentNode.removeChild(selector); }, 10000);
  }
  selector.innerHTML = '';
  const devices = await getVideoDevices();
  if (devices.length === 0) {
    const opt = document.createElement('div');
    opt.className = 'cam-option';
    opt.textContent = 'No cameras found';
    selector.appendChild(opt);
  } else {
    const preferred = getPreferredCamera();
    devices.forEach((d, i) => {
      const opt = document.createElement('div');
      opt.className = 'cam-option' + (d.deviceId === preferred ? ' selected' : '');
      opt.textContent = d.label || `Camera ${i + 1}`;
      opt.onclick = async () => {
        setPreferredCamera(d.deviceId);
        localStorage.setItem('humanity-preferred-camera-label', d.label || `Camera ${i + 1}`);
        selector.classList.remove('open');
        // If video is active, switch to new camera
        if (context === 'dm' && dmVideoActive) {
          stopDmVideo();
          await startDmVideo();
        } else if (context === 'vr' && vrVideoActive) {
          stopVrVideo();
          await startVrVideo();
        }
      };
      selector.appendChild(opt);
    });
  }
  selector.classList.toggle('open');
  // Close on outside click
  const closeHandler = (e) => {
    if (!selector.contains(e.target)) {
      selector.classList.remove('open');
      document.removeEventListener('click', closeHandler);
    }
  };
  setTimeout(() => document.addEventListener('click', closeHandler), 10);
}

// ── Connection Quality Stats ──
let qualityStatsInterval = null;
window._peerQualityCache = window._peerQualityCache || new Map();

function applyCachedQualityBadges() {
  const qMap = window._peerQualityCache || new Map();
  document.querySelectorAll('.vr-participant[data-participant-key]').forEach(el => {
    const key = el.getAttribute('data-participant-key');
    if (!key) return;
    const q = qMap.get(key);
    if (!q) return;
    let badge = el.querySelector('.quality-indicator');
    if (!badge) {
      badge = document.createElement('span');
      badge.className = 'quality-indicator';
      el.appendChild(badge);
    }
    badge.textContent = q;
  });
}

function startQualityStats() {
  if (qualityStatsInterval) return;
  qualityStatsInterval = setInterval(async () => {
    // Voice room peers
    for (const [peerKey, pc] of Object.entries(window._roomPeerConnections || {})) {
      const indicator = await getQualityIndicator(pc);
      window._peerQualityCache.set(peerKey, indicator);
    }
    applyCachedQualityBadges();

    // DM call peer
    if (peerConnection && callState === 'in-call') {
      const ind = await getQualityIndicator(peerConnection);
      const nameEl = document.getElementById('call-peer-name');
      if (nameEl) {
        // Strip old indicator
        nameEl.textContent = nameEl.textContent.replace(/ [🟢🟡🔴⚫]$/, '') + ' ' + ind;
      }
    }
  }, 3000);
}

function stopQualityStats() {
  if (qualityStatsInterval) { clearInterval(qualityStatsInterval); qualityStatsInterval = null; }
}

async function getQualityIndicator(pc) {
  try {
    const stats = await pc.getStats();
    for (const [, report] of stats) {
      if (report.type === 'candidate-pair' && report.state === 'succeeded' && report.currentRoundTripTime != null) {
        const rtt = report.currentRoundTripTime * 1000; // seconds to ms
        if (rtt < 100) return '🟢';
        if (rtt <= 300) return '🟡';
        return '🔴';
      }
    }
    return '⚫';
  } catch (e) {
    return '⚫';
  }
}

// Start quality stats when in voice room or call
const _origShowCallBar = showCallBar;
showCallBar = function() {
  _origShowCallBar();
  startQualityStats();
};

const _origResetCallState2 = resetCallState;
resetCallState = function() {
  _origResetCallState2();
  if (!window._currentRoomId) stopQualityStats();
};

// Start/stop quality stats with voice room
const _origSetupRoomAudio2 = window.setupRoomAudio;
window.setupRoomAudio = async function() {
  await _origSetupRoomAudio2();
  startQualityStats();
};

const _origCleanupRoomAudio3 = window.cleanupRoomAudio;
window.cleanupRoomAudio = function() {
  _origCleanupRoomAudio3();
  if (callState === 'idle') stopQualityStats();
};

// ── Mic Device Selection ──

function getPreferredMic() {
  return localStorage.getItem('humanity-preferred-mic') || null;
}
function savePreferredMic(deviceId) {
  localStorage.setItem('humanity-preferred-mic', deviceId);
}

// Patch getMicConstraints to honour preferred mic device.
const _origGetMicConstraints = getMicConstraints;
getMicConstraints = function() {
  const c = _origGetMicConstraints();
  const preferred = getPreferredMic();
  if (preferred) c.deviceId = { ideal: preferred };
  return c;
};

/**
 * Populates #studio-mic-select with available audio-input devices.
 * Called on load and whenever the OS device list changes.
 */
window.populateMicDevices = async function() {
  const sel = document.getElementById('studio-mic-select');
  if (!sel) return;
  let devices = [];
  try {
    const all = await navigator.mediaDevices.enumerateDevices();
    const raw = all.filter(d => d.kind === 'audioinput');
    // Windows exposes each physical mic as up to 3 entries: Default, Communications, actual hardware.
    // Keep the "default" system entry (auto-follows OS setting), skip "Communications", and
    // deduplicate real hardware entries by groupId so each physical device appears once.
    const seenGroups = new Set();
    devices = raw.filter(d => {
      if (d.deviceId === 'default') return true;
      if (d.label && /^communications\s*[-–]/i.test(d.label)) return false;
      if (d.groupId && seenGroups.has(d.groupId)) return false;
      if (d.groupId) seenGroups.add(d.groupId);
      return true;
    });
  } catch (_) { return; }
  const preferred = getPreferredMic();
  sel.innerHTML = '';
  if (devices.length === 0) {
    const opt = document.createElement('option');
    opt.value = ''; opt.textContent = '🎙️ No microphones found';
    sel.appendChild(opt);
    return;
  }
  devices.forEach((d, i) => {
    const opt = document.createElement('option');
    opt.value = d.deviceId;
    // Simplify "Default - Realtek HD Audio..." → just "Default" for the virtual entry.
    let label = d.label || `Microphone ${i + 1}`;
    if (d.deviceId === 'default') label = 'Default';
    else label = label.replace(/^default\s*[-–]\s*/i, '');
    opt.textContent = '🎙️ ' + label;
    if (d.deviceId === preferred) opt.selected = true;
    sel.appendChild(opt);
  });
};

/**
 * Switches the active microphone to the chosen device.
 * If currently in a voice room, replaces the audio track in all peer connections
 * without dropping the call.
 */
window.setStudioMic = async function(deviceId) {
  if (!deviceId) return;
  savePreferredMic(deviceId);
  if (window._currentRoomId && window._roomLocalStream) {
    const oldTrack = window._roomLocalStream.getAudioTracks()[0];
    try {
      const newStream = await navigator.mediaDevices.getUserMedia({ audio: getMicConstraints(), video: false });
      const newTrack = newStream.getAudioTracks()[0];
      for (const pc of Object.values(window._roomPeerConnections || {})) {
        const sender = pc.getSenders().find(s => s.track && s.track.kind === 'audio');
        if (sender) await sender.replaceTrack(newTrack);
      }
      if (oldTrack) oldTrack.stop();
      window._roomLocalStream = newStream;
      // Apply current mute state to new track
      if (typeof isMuted !== 'undefined') newTrack.enabled = !isMuted;
    } catch (e) {
      addSystemMessage('⚠️ Could not switch microphone: ' + e.message);
    }
  }
};

// Populate mic list on load and whenever OS device list changes.
if (navigator.mediaDevices) {
  window.populateMicDevices();
  navigator.mediaDevices.addEventListener('devicechange', window.populateMicDevices);
}
