  // ══════════════════════════════════════
  // STREAMS TAB
  // ══════════════════════════════════════

  function toggleStreamsCard(id) {
   const card = document.getElementById('streams-' + id);
   if (card) card.classList.toggle('collapsed');
   localStorage.setItem('streams_collapsed_' + id, card.classList.contains('collapsed'));
  }
  ['dashboard', 'live', 'servers'].forEach(id => {
   if (localStorage.getItem('streams_collapsed_' + id) === 'true') {
    const card = document.getElementById('streams-' + id);
    if (card) card.classList.add('collapsed');
   }
  });

  // ── Stream State ──
  let streamScreenStream = null;
  let streamWebcamStream = null;
  let streamCompositeStream = null;
  let streamIsLive = false;
  let streamStartTime = null;
  let streamDurationTimer = null;
  let streamPeerConnection = null;
  let streamViewerPC = null;
  let streamChatMessages = [];
  let streamChatFilter_ = 'all';
  let streamCurrentInfo = { active: false };
  let streamPipCorner = 'br';
  let streamCompositeAnimFrame = null;
  let streamMetricsTimer = null;
  let streamMetricsPrevBytes = 0;
  let streamMetricsPrevTime = 0;
  let viewerMetricsPrevBytes = 0;
  let viewerMetricsPrevTime = 0;

  // Audio state
  let streamAudioCtx = null;
  let streamMicStream = null;
  let streamMicGainNode = null;
  let streamMicSourceNode = null;
  let streamMicDestNode = null;
  let streamMicMuted = false;
  let streamBrbActive = false;
  let streamBrbStartAt = 0;
  let streamSafeMode = false;
  let streamPerfSlowFrames = 0;
  let streamLastLoopMs = 0;
  let streamLoopLastTs = 0;

  // Twitch IRC WebSocket for chat integration
  let twitchChatWs = null;

  function streamSetMicStatus(text, tone) {
   const el = document.getElementById('stream-mic-status');
   if (!el) return;
   el.textContent = text;
   const colors = {
    neutral: 'var(--text-muted)',
    good: 'var(--success)',
    warn: '#e0c860',
    bad: '#ff6b6b'
   };
   el.style.color = colors[tone || 'neutral'] || colors.neutral;
  }

  function streamNormalizeUiText() {
   // Repair any mojibake-heavy labels in Streams UI with plain text fallbacks.
   const goBtn = document.getElementById('stream-go-live-btn');
   if (goBtn) goBtn.textContent = 'GO LIVE';
   const stopBtn = document.getElementById('stream-stop-btn');
   if (stopBtn) stopBtn.textContent = 'END';

   const hdr = document.querySelector('#streams-dashboard .reality-card-header');
   if (hdr) {
    const icon = hdr.querySelector('.collapse-icon');
    hdr.textContent = 'Stream Control ';
    if (icon) hdr.appendChild(icon);
   }

   const liveHdr = document.querySelector('#streams-live .reality-card-header');
   if (liveHdr) {
    const icon = liveHdr.querySelector('.collapse-icon');
    liveHdr.textContent = 'Live Now ';
    if (icon) liveHdr.appendChild(icon);
   }

   const serversHdr = document.querySelector('#streams-servers .reality-card-header');
   if (serversHdr) {
    const icon = serversHdr.querySelector('.collapse-icon');
    serversHdr.textContent = 'Servers ';
    if (icon) serversHdr.appendChild(icon);
   }

   const sectionTitles = document.querySelectorAll('#tab-streams .settings-section-title');
   if (sectionTitles[0]) sectionTitles[0].textContent = 'Video';
   if (sectionTitles[1]) sectionTitles[1].textContent = 'Audio';
   if (sectionTitles[2]) sectionTitles[2].textContent = 'Camera';

   const panelTitles = document.querySelectorAll('#tab-streams .stream-panel > div[style*="font-weight:600"]');
   panelTitles.forEach(el => {
    const t = (el.textContent || '').toLowerCase();
    if (t.includes('scene')) el.textContent = 'Scenes';
    else if (t.includes('stream metrics')) el.textContent = 'Stream Metrics';
    else if (t.includes('stream info')) el.textContent = 'Stream Info';
    else if (t.includes('restream')) el.textContent = 'Restream To';
   });
  }

  // Default stream mic status on tab load.
  setTimeout(() => {
   streamSetMicStatus('Mic status: idle', 'neutral');
   streamNormalizeUiText();
   sanitizeUiMojibake();
  }, 0);

  // Warn before leaving while live (browser safety; desktop nav guard handled in shared shell).
  window.addEventListener('beforeunload', function(e) {
   if (streamIsLive) {
    e.preventDefault();
    e.returnValue = 'You are currently live. Leaving this page may stop your stream.';
   }
  });

  // ── Scenes v1 ──
  const STREAM_SCENES_KEY = 'stream_scenes_v1';
  const STREAM_DEFAULT_SCENES = {
   gameplay: {
    name: 'Gameplay',
    resolution: '1080', fps: '60', bitrate: '8',
    screen: true, webcam: false, desktopAudio: true,
    webcamSize: 'small', webcamQuality: 'medium', pipCorner: 'br',
    advancedDevices: false
   },
   chatting: {
    name: 'Chatting',
    resolution: '1080', fps: '30', bitrate: '6',
    screen: false, webcam: true, desktopAudio: false,
    webcamSize: 'large', webcamQuality: 'high', pipCorner: 'br',
    advancedDevices: false
   },
   brb: {
    name: 'BRB',
    resolution: '1080', fps: '30', bitrate: '4',
    screen: false, webcam: false, desktopAudio: false,
    webcamSize: 'small', webcamQuality: 'medium', pipCorner: 'br',
    advancedDevices: false
   }
  };

  function streamLoadScenes() {
   try {
    const saved = JSON.parse(localStorage.getItem(STREAM_SCENES_KEY) || 'null');
    if (saved && typeof saved === 'object' && Object.keys(saved).length) return saved;
   } catch(e) {}
   return JSON.parse(JSON.stringify(STREAM_DEFAULT_SCENES));
  }

  function streamSaveScenes(scenes) {
   localStorage.setItem(STREAM_SCENES_KEY, JSON.stringify(scenes));
  }

  function streamCurrentSceneState() {
   return {
    resolution: document.getElementById('stream-resolution')?.value || '1080',
    fps: document.getElementById('stream-fps')?.value || '30',
    bitrate: document.getElementById('stream-bitrate')?.value || '6',
    screen: !!document.getElementById('stream-screen-cb')?.checked,
    webcam: !!document.getElementById('stream-webcam-cb')?.checked,
    desktopAudio: !!document.getElementById('stream-desktop-audio-cb')?.checked,
    webcamSize: document.getElementById('stream-webcam-size')?.value || 'medium',
    webcamQuality: document.getElementById('stream-webcam-quality')?.value || 'medium',
    pipCorner: window.streamPipCorner || 'br',
    advancedDevices: !!document.getElementById('stream-advanced-devices')?.checked,
   };
  }

  function streamApplySceneState(s) {
   if (!s) return;
   if (s.resolution) document.getElementById('stream-resolution').value = s.resolution;
   if (s.fps) document.getElementById('stream-fps').value = s.fps;
   if (s.bitrate) {
    document.getElementById('stream-bitrate').value = s.bitrate;
    document.getElementById('stream-bitrate-val').textContent = s.bitrate + 'Mbps';
   }
   if (typeof s.screen !== 'undefined') document.getElementById('stream-screen-cb').checked = !!s.screen;
   if (typeof s.webcam !== 'undefined') document.getElementById('stream-webcam-cb').checked = !!s.webcam;
   if (typeof s.desktopAudio !== 'undefined') document.getElementById('stream-desktop-audio-cb').checked = !!s.desktopAudio;
   if (s.webcamSize) document.getElementById('stream-webcam-size').value = s.webcamSize;
   if (s.webcamQuality) document.getElementById('stream-webcam-quality').value = s.webcamQuality;
   if (s.pipCorner) streamSetPipCorner(s.pipCorner);
   if (typeof s.advancedDevices !== 'undefined') {
    document.getElementById('stream-advanced-devices').checked = !!s.advancedDevices;
    streamEnumerateDevices();
   }
   streamApplyBitrate();
   streamToggleDesktopAudio();
   streamToggleWebcam();
   streamNotify('Scene applied.', 'cyan', 4);
  }

  function streamNotify(text, tone, seconds) {
   if (typeof addNotice === 'function') addNotice(text, tone || 'cyan', seconds || 4);
   else console.log('[stream]', text);
  }

  function streamRenderScenes() {
   const scenes = streamLoadScenes();
   const sel = document.getElementById('stream-scene-select');
   if (!sel) return;
   const current = sel.value;
   sel.innerHTML = Object.entries(scenes).map(([id, scene]) => `<option value="${escapeHtml(id)}">${escapeHtml(scene.name || id)}</option>`).join('');
   if (current && scenes[current]) sel.value = current;
  }

  function streamApplySelectedScene() {
   const scenes = streamLoadScenes();
   const id = document.getElementById('stream-scene-select')?.value;
   if (!id || !scenes[id]) return;
   streamApplySceneState(scenes[id]);
  }

  function streamSaveCurrentScene() {
   const scenes = streamLoadScenes();
   const id = document.getElementById('stream-scene-select')?.value;
   if (!id || !scenes[id]) return;
   scenes[id] = { ...scenes[id], ...streamCurrentSceneState() };
   streamSaveScenes(scenes);
   streamNotify('Scene updated.', 'green', 4);
  }

  function streamCreateScene() {
   const name = prompt('Scene name:');
   if (!name || !name.trim()) return;
   const id = name.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 32);
   if (!id) return;
   const scenes = streamLoadScenes();
   scenes[id] = { name: name.trim(), ...streamCurrentSceneState() };
   streamSaveScenes(scenes);
   streamRenderScenes();
   document.getElementById('stream-scene-select').value = id;
   streamNotify('Scene created.', 'green', 4);
  }

  function streamDeleteScene() {
   const sel = document.getElementById('stream-scene-select');
   const id = sel?.value;
   if (!id) return;
   const scenes = streamLoadScenes();
   if (!scenes[id]) return;
   if (!confirm('Delete scene "' + (scenes[id].name || id) + '"?')) return;
   delete scenes[id];
   streamSaveScenes(scenes);
   streamRenderScenes();
   streamNotify('Scene deleted.', 'yellow', 4);
  }

  // ── Settings persistence ──
  function streamSaveSetting(key, val) {
   try {
    const s = JSON.parse(localStorage.getItem('stream_settings') || '{}');
    s[key] = val;
    localStorage.setItem('stream_settings', JSON.stringify(s));
   } catch(e) {}
  }
  function streamLoadSettings() {
   try {
    const s = JSON.parse(localStorage.getItem('stream_settings') || '{}');
    if (s.resolution) document.getElementById('stream-resolution').value = s.resolution;
    if (s.fps) document.getElementById('stream-fps').value = s.fps;
    if (s.bitrate) {
     document.getElementById('stream-bitrate').value = s.bitrate;
     document.getElementById('stream-bitrate-val').textContent = s.bitrate + 'Mbps';
    }
    if (s.webcamSize) document.getElementById('stream-webcam-size').value = s.webcamSize;
    if (s.webcamQuality) document.getElementById('stream-webcam-quality').value = s.webcamQuality;
    if (s.pipCorner) streamSetPipCorner(s.pipCorner);
    if (typeof s.advancedDevices !== 'undefined') document.getElementById('stream-advanced-devices').checked = !!s.advancedDevices;
   } catch(e) {}
  }
  streamLoadSettings();
  streamRenderScenes();

  // ── Enumerate devices ──
  function streamPrettyDeviceLabel(label) {
   let s = String(label || '').trim();
   // Remove role prefixes that create duplicate-looking rows.
   s = s.replace(/^(Default|Communications)\s*-\s*/i, '');
   // Remove trailing parenthesized endpoint metadata aggressively.
   s = s.replace(/\s*\([^)]*\)\s*$/i, '');
   // Remove Windows endpoint id fragments like {0.0.1.00000000}.{GUID}
   s = s.replace(/\{[0-9a-fA-F.\-]+\}/g, '');
   // Remove long hex-ish tokens often appended by audio stacks.
   s = s.replace(/\b(?:0x)?[0-9a-fA-F]{8,}\b/g, '');
   // Collapse repeated separators and whitespace.
   s = s.replace(/\s*-\s*$/g, '').replace(/\s+/g, ' ').trim();
   if (!s) return 'Microphone';
   return s;
  }

  async function streamEnumerateDevices() {
   try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const micSel = document.getElementById('stream-mic-device');
    const camSel = document.getElementById('stream-webcam-device');
    const showAdvanced = !!document.getElementById('stream-advanced-devices')?.checked;
    micSel.innerHTML = '<option value="">Default Microphone</option>';
    camSel.innerHTML = '<option value="">Default Camera</option>';

    const seenMics = new Set();
    const seenCams = new Set();

    if (!showAdvanced) return;

    devices.forEach(d => {
     if (d.kind === 'audioinput') {
      const rawLabel = (d.label || '').trim();
      if (!rawLabel) return; // skip anonymous duplicates before permission
      if (/disabled|unplugged/i.test(rawLabel)) return; // hide obviously unavailable devices
      const pretty = streamPrettyDeviceLabel(rawLabel) || rawLabel;
      const norm = pretty.toLowerCase();
      if (seenMics.has(norm)) return;
      seenMics.add(norm);
      const short = pretty.length > 42 ? pretty.slice(0, 39) + '…' : pretty;
      micSel.innerHTML += '<option value="' + d.deviceId + '" title="' + escapeHtml(pretty) + '">' + escapeHtml(short) + '</option>';
     }
     if (d.kind === 'videoinput') {
      const rawLabel = (d.label || '').trim();
      if (!rawLabel) return;
      if (/disabled|unplugged/i.test(rawLabel)) return;
      const pretty = streamPrettyDeviceLabel(rawLabel) || rawLabel;
      const norm = pretty.toLowerCase();
      if (seenCams.has(norm)) return;
      seenCams.add(norm);
      const short = pretty.length > 42 ? pretty.slice(0, 39) + '…' : pretty;
      camSel.innerHTML += '<option value="' + d.deviceId + '" title="' + escapeHtml(pretty) + '">' + escapeHtml(short) + '</option>';
     }
    });
   } catch(e) {}
  }
  streamEnumerateDevices();
  if (navigator.mediaDevices && navigator.mediaDevices.addEventListener) {
   navigator.mediaDevices.addEventListener('devicechange', streamEnumerateDevices);
  }

  // ── PiP Compositing ──
  const compositeCanvas = document.getElementById('stream-composite-canvas');
  const compositeCtx = compositeCanvas ? compositeCanvas.getContext('2d') : null;
  const hiddenScreenVideo = document.createElement('video');
  hiddenScreenVideo.muted = true; hiddenScreenVideo.playsInline = true;

  function streamCompositeLoop() {
   if (!compositeCtx) return;
   const loopStart = performance.now();
   const w = compositeCanvas.width, h = compositeCanvas.height;
   compositeCtx.fillStyle = '#000';
   compositeCtx.fillRect(0, 0, w, h);

   if (streamScreenStream && hiddenScreenVideo.readyState >= 2) {
    const vw = hiddenScreenVideo.videoWidth, vh = hiddenScreenVideo.videoHeight;
    if (vw && vh) {
     const scale = Math.min(w / vw, h / vh);
     const dw = vw * scale, dh = vh * scale;
     compositeCtx.drawImage(hiddenScreenVideo, (w - dw) / 2, (h - dh) / 2, dw, dh);
    }
   }

   const pipVideo = document.getElementById('pip-webcam-video');
   if (!streamSafeMode && streamWebcamStream && pipVideo && pipVideo.readyState >= 2 && document.getElementById('stream-webcam-cb').checked) {
    const sizeMap = { small: 0.15, medium: 0.2, large: 0.3 };
    const sizeFrac = sizeMap[document.getElementById('stream-webcam-size').value] || 0.2;
    const pw = w * sizeFrac;
    const ph = pw * (pipVideo.videoHeight / pipVideo.videoWidth || 0.75);
    const margin = 16;
    let px, py;
    if (streamPipCorner === 'tl') { px = margin; py = margin; }
    else if (streamPipCorner === 'tr') { px = w - pw - margin; py = margin; }
    else if (streamPipCorner === 'bl') { px = margin; py = h - ph - margin; }
    else { px = w - pw - margin; py = h - ph - margin; }

    compositeCtx.save();
    compositeCtx.beginPath();
    compositeCtx.roundRect(px, py, pw, ph, 12);
    compositeCtx.clip();
    compositeCtx.drawImage(pipVideo, px, py, pw, ph);
    compositeCtx.restore();
    compositeCtx.strokeStyle = 'rgba(153,102,255,0.6)';
    compositeCtx.lineWidth = 3;
    compositeCtx.beginPath();
    compositeCtx.roundRect(px, py, pw, ph, 12);
    compositeCtx.stroke();
   }

   if (streamBrbActive) {
    const elapsed = Math.max(0, Math.floor((Date.now() - streamBrbStartAt) / 1000));
    const mm = String(Math.floor(elapsed / 60)).padStart(2, '0');
    const ss = String(elapsed % 60).padStart(2, '0');

    compositeCtx.save();
    compositeCtx.fillStyle = 'rgba(0,0,0,0.58)';
    compositeCtx.fillRect(0, 0, w, h);

    compositeCtx.fillStyle = '#f5f5f5';
    compositeCtx.textAlign = 'center';
    compositeCtx.font = 'bold 84px system-ui, -apple-system, Segoe UI, Roboto, sans-serif';
    compositeCtx.fillText('BRB', w / 2, h / 2 - 20);
    compositeCtx.font = '500 36px system-ui, -apple-system, Segoe UI, Roboto, sans-serif';
    compositeCtx.fillText('AFK ' + mm + ':' + ss, w / 2, h / 2 + 42);
    compositeCtx.restore();
   }

   streamLastLoopMs = Math.max(0, performance.now() - loopStart);
   streamLoopLastTs = Date.now();
   if (streamLastLoopMs > 42) streamPerfSlowFrames++; else streamPerfSlowFrames = Math.max(0, streamPerfSlowFrames - 1);
   if (!streamSafeMode && streamPerfSlowFrames >= 8) {
    streamSafeMode = true;
    const webcamCb = document.getElementById('stream-webcam-cb');
    if (webcamCb) webcamCb.checked = false;
    streamSetMicStatus('Mic status: safe mode (webcam overlay disabled)', 'warn');
   }
   const loopEl = document.getElementById('stream-stat-loop');
   if (loopEl) {
    const suffix = streamSafeMode ? ' [SAFE]' : '';
    loopEl.textContent = 'Loop: ' + streamLastLoopMs.toFixed(1) + 'ms' + suffix;
   }

   streamCompositeAnimFrame = requestAnimationFrame(streamCompositeLoop);
  }

  function streamStartCompositing() {
   if (streamCompositeAnimFrame) cancelAnimationFrame(streamCompositeAnimFrame);
   streamCompositeLoop();
   document.getElementById('stream-preview-placeholder').style.display = 'none';
   const fps = parseInt(document.getElementById('stream-fps').value) || 30;
   streamCompositeStream = compositeCanvas.captureStream(fps);
   // Add desktop audio from screen capture if enabled
   if (streamScreenStream && document.getElementById('stream-desktop-audio-cb').checked) {
    streamScreenStream.getAudioTracks().forEach(t => streamCompositeStream.addTrack(t));
   }
   // Add mic audio if active
   if (streamMicDestNode) {
    streamMicDestNode.stream.getAudioTracks().forEach(t => streamCompositeStream.addTrack(t));
   }
  }

  function streamStopCompositing() {
   if (streamCompositeAnimFrame) { cancelAnimationFrame(streamCompositeAnimFrame); streamCompositeAnimFrame = null; }
   streamCompositeStream = null;
   document.getElementById('stream-preview-placeholder').style.display = 'flex';
  }

  // ── Screen Capture ──
  async function streamStartScreen() {
   try {
    if (streamScreenStream) { streamScreenStream.getTracks().forEach(t => t.stop()); }
    const resSel = document.getElementById('stream-resolution').value;
    const fpsSel = parseInt(document.getElementById('stream-fps').value) || 30;
    const resMap = { '720': {w:1280,h:720}, '1080': {w:1920,h:1080}, '1440': {w:2560,h:1440}, '2160': {w:3840,h:2160} };
    const res = resMap[resSel] || { w: 1920, h: 1080 };
    const constraints = { video: { width: { ideal: res.w }, height: { ideal: res.h }, frameRate: { ideal: fpsSel } }, audio: true };
    streamScreenStream = await navigator.mediaDevices.getDisplayMedia(constraints);
    hiddenScreenVideo.srcObject = streamScreenStream;
    await hiddenScreenVideo.play();
    streamScreenStream.getVideoTracks()[0].onended = () => {
     streamScreenStream = null;
     hiddenScreenVideo.srcObject = null;
     if (!streamWebcamStream) streamStopCompositing();
    };
    // Update canvas size to match
    if (resSel !== 'source') {
     compositeCanvas.width = res.w;
     compositeCanvas.height = res.h;
    } else {
     const vt = streamScreenStream.getVideoTracks()[0];
     if (vt) {
      const s = vt.getSettings();
      if (s.width && s.height) { compositeCanvas.width = s.width; compositeCanvas.height = s.height; }
     }
    }
    document.getElementById('stream-screen-cb').checked = true;
    streamStartCompositing();
    unlockAchievement('streamer');
   } catch (e) {
    console.warn('Screen capture failed:', e);
   }
  }

  function streamToggleScreen() {
   if (!document.getElementById('stream-screen-cb').checked && streamScreenStream) {
    streamScreenStream.getTracks().forEach(t => t.stop());
    streamScreenStream = null;
    hiddenScreenVideo.srcObject = null;
    if (!streamWebcamStream) streamStopCompositing();
   }
  }

  // ── Desktop Audio Toggle ──
  function streamToggleDesktopAudio() {
   // Rebuild composite stream to add/remove desktop audio
   if (streamCompositeStream && streamScreenStream) {
    // Remove existing audio tracks from screen
    const screenAudioTracks = streamScreenStream.getAudioTracks();
    if (document.getElementById('stream-desktop-audio-cb').checked) {
     screenAudioTracks.forEach(t => {
      if (!streamCompositeStream.getTrackById(t.id)) {
       streamCompositeStream.addTrack(t);
       // Update live WebRTC connections
       streamUpdateLiveTracks();
      }
     });
    } else {
     screenAudioTracks.forEach(t => {
      streamCompositeStream.removeTrack(t);
      streamUpdateLiveTracks();
     });
    }
   }
  }

  // ── Microphone ──
  async function streamStartMic(deviceId) {
   // Clean up previous mic
   streamStopMic();
   try {
    const constraints = { audio: deviceId ? { deviceId: { exact: deviceId } } : true };
    streamMicStream = await navigator.mediaDevices.getUserMedia(constraints);
    if (!streamAudioCtx) streamAudioCtx = new AudioContext();
    streamMicSourceNode = streamAudioCtx.createMediaStreamSource(streamMicStream);
    streamMicGainNode = streamAudioCtx.createGain();
    streamMicGainNode.gain.value = parseInt(document.getElementById('stream-mic-volume').value) / 100;
    streamMicDestNode = streamAudioCtx.createMediaStreamDestination();
    streamMicSourceNode.connect(streamMicGainNode);
    streamMicGainNode.connect(streamMicDestNode);
    // Add to composite if active
    if (streamCompositeStream) {
     streamMicDestNode.stream.getAudioTracks().forEach(t => streamCompositeStream.addTrack(t));
     streamUpdateLiveTracks();
    }
    // Re-enumerate to get labels
    streamEnumerateDevices();
    streamSetMicStatus('Mic status: active', 'good');
   } catch(e) {
    console.warn('Mic start failed:', e);
    streamSetMicStatus('Mic status: unavailable or denied', 'bad');
   }
  }

  function streamStopMic() {
   if (streamMicStream) { streamMicStream.getTracks().forEach(t => t.stop()); streamMicStream = null; }
   if (streamMicSourceNode) { streamMicSourceNode.disconnect(); streamMicSourceNode = null; }
   if (streamMicGainNode) { streamMicGainNode.disconnect(); streamMicGainNode = null; }
   if (streamMicDestNode) {
    if (streamCompositeStream) {
     streamMicDestNode.stream.getAudioTracks().forEach(t => {
      try { streamCompositeStream.removeTrack(t); } catch(e) {}
     });
    }
    streamMicDestNode = null;
   }
   streamSetMicStatus('Mic status: idle', 'neutral');
  }

  async function streamSwitchMic() {
   const deviceId = document.getElementById('stream-mic-device').value;
   if (deviceId || streamMicStream) {
    await streamStartMic(deviceId || undefined);
   }
  }

  function streamToggleMicMute() {
   streamMicMuted = !streamMicMuted;
   const btn = document.getElementById('stream-mic-mute-btn');
   if (streamMicMuted) {
    btn.textContent = 'Mic Off';
    btn.classList.add('muted');
    if (streamMicGainNode) streamMicGainNode.gain.value = 0;
   } else {
    btn.textContent = 'Mic On';
    btn.classList.remove('muted');
    if (streamMicGainNode) streamMicGainNode.gain.value = parseInt(document.getElementById('stream-mic-volume').value) / 100;
   }
  }

  function streamSetMicVolume(val) {
   if (streamMicGainNode && !streamMicMuted) {
    streamMicGainNode.gain.value = parseInt(val) / 100;
   }
   streamSaveSetting('micVolume', val);
  }

  // ── Webcam ──
  async function streamToggleWebcam() {
   const cb = document.getElementById('stream-webcam-cb');
   if (cb.checked) {
    await streamSwitchWebcam();
    if (!streamCompositeAnimFrame) streamStartCompositing();
   } else {
    if (streamWebcamStream) { streamWebcamStream.getTracks().forEach(t => t.stop()); streamWebcamStream = null; }
    document.getElementById('pip-webcam-video').srcObject = null;
   }
  }

  async function streamSwitchWebcam() {
   if (!document.getElementById('stream-webcam-cb').checked) return;
   try {
    if (streamWebcamStream) { streamWebcamStream.getTracks().forEach(t => t.stop()); }
    const deviceId = document.getElementById('stream-webcam-device').value;
    const qualityMap = { low: {w:640,h:480}, medium: {w:1280,h:720}, high: {w:1920,h:1080} };
    const q = qualityMap[document.getElementById('stream-webcam-quality').value] || qualityMap.medium;
    const constraints = { video: { width: { ideal: q.w }, height: { ideal: q.h } }, audio: false };
    if (deviceId) constraints.video.deviceId = { exact: deviceId };
    streamWebcamStream = await navigator.mediaDevices.getUserMedia(constraints);
    const pipVid = document.getElementById('pip-webcam-video');
    pipVid.srcObject = streamWebcamStream;
    pipVid.play().catch(() => {});
    streamEnumerateDevices();
   } catch(e) {
    document.getElementById('stream-webcam-cb').checked = false;
    console.warn('Webcam failed:', e);
   }
  }

  function streamUpdatePipSize() { /* Compositing loop reads the select value each frame */ }

  function streamSetPipCorner(corner) {
   streamPipCorner = corner;
   streamSaveSetting('pipCorner', corner);
   document.querySelectorAll('#pip-corner-picker span').forEach(s => s.classList.toggle('active', s.dataset.corner === corner));
  }

  // ── Bitrate control ──
  function streamApplyBitrate() {
   const mbps = parseFloat(document.getElementById('stream-bitrate').value) || 6;
   const bps = mbps * 1000000;
   // Apply to all viewer PCs
   Object.values(streamViewerPCs).forEach(pc => {
    pc.getSenders().forEach(sender => {
     if (sender.track && sender.track.kind === 'video') {
      const params = sender.getParameters();
      if (!params.encodings || params.encodings.length === 0) params.encodings = [{}];
      params.encodings[0].maxBitrate = bps;
      sender.setParameters(params).catch(() => {});
     }
    });
   });
  }

  // ── Update live WebRTC tracks (for on-the-fly changes) ──
  function streamUpdateLiveTracks() {
   // This is complex - for now, new viewers get updated tracks automatically
   // Existing viewers would need renegotiation; skip for simplicity
  }

  // ── Go Live / Stop ──
  async function streamGoLive() {
   if (streamIsLive) return;
   if (!streamCompositeStream && !streamScreenStream) {
    await streamStartScreen();
    if (!streamScreenStream) return;
   }
   // Ensure composite stream is fresh (fixes retry bug)
   if (streamCompositeStream) {
    // Check if tracks are still live
    const tracks = streamCompositeStream.getTracks();
    const allLive = tracks.length > 0 && tracks.every(t => t.readyState === 'live');
    if (!allLive) {
     streamStartCompositing();
    }
   }
   // Ensure microphone path is attempted (selected device or default mic).
   if (!streamMicStream) {
    const selectedMic = document.getElementById('stream-mic-device').value || undefined;
    await streamStartMic(selectedMic);
    if (!streamMicStream) {
     streamSetMicStatus('Mic status: missing (streaming without mic)', 'warn');
    }
   }

   const title = document.getElementById('stream-title').value || 'Untitled Stream';
   const category = document.getElementById('stream-category').value || '';

   if (typeof ws !== 'undefined' && ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_start', title, category }));
   }

   streamIsLive = true;
   streamStartTime = Date.now();
   streamSafeMode = false;
   streamPerfSlowFrames = 0;
   document.getElementById('stream-go-live-btn').style.display = 'none';
   document.getElementById('stream-stop-btn').style.display = 'inline-flex';
   document.getElementById('stream-brb-btn').style.display = 'inline-flex';
   document.getElementById('stream-stop-btn').classList.remove('btn-disabled');
   document.getElementById('stream-stop-btn').classList.add('btn-activated');

   // Duration timer
   streamDurationTimer = setInterval(() => {
    const elapsed = Math.floor((Date.now() - streamStartTime) / 1000);
    const h = Math.floor(elapsed / 3600);
    const m = Math.floor((elapsed % 3600) / 60).toString().padStart(2, '0');
    const s = (elapsed % 60).toString().padStart(2, '0');
    document.getElementById('stream-stat-duration').textContent = 'Duration: ' + (h ? h + ':' : '') + m + ':' + s;
   }, 1000);

   // Update quality stats
   if (streamScreenStream) {
    const vt = streamScreenStream.getVideoTracks()[0];
    if (vt) {
     const settings = vt.getSettings();
     document.getElementById('stream-stat-quality').textContent = 'Quality: ' + (settings.width || '?') + 'x' + (settings.height || '?') + ' @ ' + (settings.frameRate ? Math.round(settings.frameRate) + 'fps' : '?');
    }
   }

   // Apply bitrate setting
   streamApplyBitrate();

   // Start metrics collection
   streamStartMetrics();

   // External streams
   const externalUrls = [];
   if (document.getElementById('stream-twitch-cb').checked) {
    externalUrls.push({ platform: 'twitch', url: document.getElementById('stream-twitch-url').value });
   }
   if (document.getElementById('stream-youtube-cb').checked) {
    externalUrls.push({ platform: 'youtube', url: document.getElementById('stream-youtube-url').value });
   }
   if (document.getElementById('stream-rumble-cb').checked) {
    externalUrls.push({ platform: 'rumble', url: document.getElementById('stream-rumble-url').value });
   }
   if (externalUrls.length > 0 && ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_set_external', urls: externalUrls }));
   }

   const twitchUrl = document.getElementById('stream-twitch-url').value;
   if (document.getElementById('stream-twitch-cb').checked && twitchUrl) {
    const twitchChannel = twitchUrl.replace(/.*twitch\.tv\/?/i, '').replace(/\//g, '').toLowerCase();
    if (twitchChannel) connectTwitchChat(twitchChannel);
   }
  }

  function streamToggleBrb() {
   if (!streamIsLive) return;
   streamBrbActive = !streamBrbActive;
   const btn = document.getElementById('stream-brb-btn');
   if (streamBrbActive) {
    streamBrbStartAt = Date.now();
    if (btn) {
     btn.textContent = 'BACK';
     btn.classList.add('btn-activated');
    }
    streamSetMicStatus('Mic status: BRB mode', 'warn');
   } else {
    if (btn) {
     btn.textContent = 'BRB';
     btn.classList.remove('btn-activated');
    }
    streamSetMicStatus('Mic status: active', 'good');
   }
  }

  function streamStop() {
   if (!streamIsLive) return;
   streamIsLive = false;

   if (streamDurationTimer) { clearInterval(streamDurationTimer); streamDurationTimer = null; }
   if (streamMetricsTimer) { clearInterval(streamMetricsTimer); streamMetricsTimer = null; }
   if (streamViewerPollInt) { clearInterval(streamViewerPollInt); streamViewerPollInt = null; }
   if (streamStatsPollInt) { clearInterval(streamStatsPollInt); streamStatsPollInt = null; }
   if (streamHeartbeatInt) { clearInterval(streamHeartbeatInt); streamHeartbeatInt = null; }

   if (streamCompositeAnimFrame) {
    try { cancelAnimationFrame(streamCompositeAnimFrame); } catch(e) {}
    streamCompositeAnimFrame = null;
   }

   // Stop all local capture streams/tracks to prevent lingering GPU/encoder load.
   [streamScreenStream, streamWebcamStream, streamCompositeStream].forEach(s => {
    if (!s) return;
    try { s.getTracks().forEach(t => t.stop()); } catch(e) {}
   });
   streamScreenStream = null;
   streamWebcamStream = null;
   streamCompositeStream = null;

   // Release media element sources.
   try {
    const pv = document.getElementById('stream-preview-video');
    if (pv) pv.srcObject = null;
    if (screenVideo) screenVideo.srcObject = null;
    if (pipVideo) pipVideo.srcObject = null;
   } catch(e) {}

   // Close WebAudio context used for compositing.
   try { if (streamAudioCtx) streamAudioCtx.close(); } catch(e) {}
   streamAudioCtx = null;
   streamAudioDestNode = null;
   streamDeskSourceNode = null;
   streamWebcamAudioSourceNode = null;

   document.getElementById('stream-go-live-btn').style.display = 'inline-flex';
   document.getElementById('stream-stop-btn').style.display = 'none';
   document.getElementById('stream-brb-btn').style.display = 'none';
   document.getElementById('stream-brb-btn').textContent = 'BRB';
   document.getElementById('stream-brb-btn').classList.remove('btn-activated');
   streamBrbActive = false;
   streamBrbStartAt = 0;
   const statDuration = document.getElementById('stream-stat-duration');
   const statQuality = document.getElementById('stream-stat-quality');
   const statBitrate = document.getElementById('stream-stat-bitrate');
   const statTracks = document.getElementById('stream-stat-tracks');
   const statLoop = document.getElementById('stream-stat-loop');
   if (statDuration) statDuration.textContent = 'Duration: --:--';
   if (statQuality) statQuality.textContent = 'Quality: --';
   if (statBitrate) statBitrate.textContent = 'Bitrate: --';
   if (statTracks) statTracks.textContent = 'Tracks: --';
   if (statLoop) statLoop.textContent = 'Loop: --';
   streamLastLoopMs = 0;
   streamLoopLastTs = 0;
   streamSafeMode = false;
   streamPerfSlowFrames = 0;

   if (ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_stop' }));
   }

   // Close all viewer WebRTC connections
   Object.keys(streamViewerPCs).forEach(key => {
    try { streamViewerPCs[key].close(); } catch(e) {}
    delete streamViewerPCs[key];
   });
   if (streamPeerConnection) { streamPeerConnection.close(); streamPeerConnection = null; }
   if (twitchChatWs) { twitchChatWs.close(); twitchChatWs = null; }

   // Stop mic
   streamStopMic();
  }

  // ── Stream Metrics ──
  function streamStartMetrics() {
   streamMetricsPrevBytes = 0;
   streamMetricsPrevTime = Date.now();
   if (streamMetricsTimer) clearInterval(streamMetricsTimer);
   streamMetricsTimer = setInterval(streamCollectMetrics, 2000);
  }

  async function streamCollectMetrics() {
   // Streamer metrics: aggregate from all viewer PCs
   const pcEntries = Object.entries(streamViewerPCs);
   const pcs = pcEntries.map(e => e[1]);
   // Show PC states in stats bar for debugging
   const states = pcEntries.map(([k,pc]) => k.substring(0,6) + ':' + pc.connectionState).join(' ');
   const stateEl = document.getElementById('stream-stat-bitrate');
   if (stateEl && pcEntries.length > 0) stateEl.title = 'PCs: ' + states;
   if (pcs.length === 0) return;
   // Use the first PC for codec/resolution info, aggregate bandwidth
   let totalBytesSent = 0, totalPacketsSent = 0, totalPacketsLost = 0;
   let fps = 0, resW = 0, resH = 0, codec = '--', rtt = 0, jitter = 0;
   let pcCount = 0;

   for (const pc of pcs) {
    if (pc.connectionState === 'closed') continue;
    try {
     const stats = await pc.getStats();
     stats.forEach(report => {
      if (report.type === 'outbound-rtp' && report.kind === 'video') {
       totalBytesSent += report.bytesSent || 0;
       totalPacketsSent += report.packetsSent || 0;
       if (report.framesPerSecond) fps = report.framesPerSecond;
       if (report.frameWidth) { resW = report.frameWidth; resH = report.frameHeight; }
       if (report.codecId) {
        stats.forEach(r => { if (r.id === report.codecId) codec = (r.mimeType || '').replace('video/', ''); });
       }
      }
      if (report.type === 'remote-inbound-rtp' && report.kind === 'video') {
       totalPacketsLost += report.packetsLost || 0;
       if (report.roundTripTime) rtt = report.roundTripTime;
       if (report.jitter) jitter = report.jitter;
      }
      if (report.type === 'candidate-pair' && report.state === 'succeeded') {
       if (report.currentRoundTripTime) rtt = report.currentRoundTripTime;
      }
     });
     pcCount++;
    } catch(e) {}
   }

   const now = Date.now();
   const dt = (now - streamMetricsPrevTime) / 1000;
   const bps = dt > 0 ? ((totalBytesSent - streamMetricsPrevBytes) * 8) / dt : 0;
   streamMetricsPrevBytes = totalBytesSent;
   streamMetricsPrevTime = now;

   const formatBps = (b) => b > 1000000 ? (b/1000000).toFixed(1) + ' Mbps' : (b/1000).toFixed(0) + ' kbps';
   const formatBytes = (b) => b > 1073741824 ? (b/1073741824).toFixed(2) + ' GB' : b > 1048576 ? (b/1048576).toFixed(1) + ' MB' : (b/1024).toFixed(0) + ' KB';

   // Update stats bar
   document.getElementById('stream-stat-bitrate').textContent = 'Bitrate: ' + formatBps(bps);

   // Update metrics panel
   const el = (id) => document.getElementById(id);
   el('sm-upload').textContent = formatBps(bps);
   el('sm-pkts-sent').textContent = totalPacketsSent.toLocaleString();
   el('sm-pkts-lost').textContent = totalPacketsLost.toLocaleString();
   el('sm-fps').textContent = fps ? Math.round(fps) + ' fps' : '--';
   el('sm-resolution').textContent = resW ? resW + 'x' + resH : '--';
   el('sm-codec').textContent = codec;
   el('sm-rtt').textContent = rtt ? (rtt * 1000).toFixed(0) + ' ms' : '--';
   el('sm-jitter').textContent = jitter ? (jitter * 1000).toFixed(1) + ' ms' : '--';
   el('sm-total').textContent = formatBytes(totalBytesSent);
   el('sm-connections').textContent = pcCount + '/' + pcs.length + ' (' + states + ')';
  }

  async function viewerCollectMetrics() {
   if (!streamViewerPC || streamViewerPC.connectionState === 'closed') return;
   try {
    const stats = await streamViewerPC.getStats();
    let bytesReceived = 0, packetsReceived = 0, packetsLost = 0;
    let fps = 0, resW = 0, resH = 0, codec = '--', jitterBuffer = 0;

    stats.forEach(report => {
     if (report.type === 'inbound-rtp' && report.kind === 'video') {
      bytesReceived += report.bytesReceived || 0;
      packetsReceived += report.packetsReceived || 0;
      packetsLost += report.packetsLost || 0;
      if (report.framesPerSecond) fps = report.framesPerSecond;
      if (report.frameWidth) { resW = report.frameWidth; resH = report.frameHeight; }
      if (report.jitterBufferDelay && report.jitterBufferEmittedCount) {
       jitterBuffer = report.jitterBufferDelay / report.jitterBufferEmittedCount;
      }
      if (report.codecId) {
       stats.forEach(r => { if (r.id === report.codecId) codec = (r.mimeType || '').replace('video/', ''); });
      }
     }
    });

    const now = Date.now();
    const dt = (now - viewerMetricsPrevTime) / 1000;
    const bps = dt > 0 ? ((bytesReceived - viewerMetricsPrevBytes) * 8) / dt : 0;
    viewerMetricsPrevBytes = bytesReceived;
    viewerMetricsPrevTime = now;

    const formatBps = (b) => b > 1000000 ? (b/1000000).toFixed(1) + ' Mbps' : (b/1000).toFixed(0) + ' kbps';
    const el = (id) => document.getElementById(id);
    el('vm-download').textContent = formatBps(bps);
    el('vm-pkts-recv').textContent = packetsReceived.toLocaleString();
    el('vm-pkts-lost').textContent = packetsLost.toLocaleString();
    el('vm-fps').textContent = fps ? Math.round(fps) + ' fps' : '--';
    el('vm-resolution').textContent = resW ? resW + 'x' + resH : '--';
    el('vm-codec').textContent = codec;
    el('vm-jitter').textContent = jitterBuffer ? (jitterBuffer * 1000).toFixed(0) + ' ms' : '--';
   } catch(e) {}
  }

  // ── Stream Chat ──
  function streamChatFilter(filter, context) {
   streamChatFilter_ = filter;
   const tabsId = context === 'viewer' ? 'viewer-chat-tabs' : 'stream-chat-tabs';
   document.querySelectorAll('#' + tabsId + ' button').forEach(b => b.classList.toggle('active', b.textContent.toLowerCase() === filter || (filter === 'all' && b.textContent === 'All')));
   streamRenderChat(context);
  }

  function streamRenderChat(context) {
   const containerId = context === 'viewer' ? 'viewer-chat-messages' : 'stream-chat-messages';
   const container = document.getElementById(containerId);
   if (!container) return;
   const filtered = streamChatFilter_ === 'all' ? streamChatMessages : streamChatMessages.filter(m => m.source === streamChatFilter_);
   container.innerHTML = filtered.map(m => {
    const srcClass = 'src-' + (m.source || 'humanity');
    const srcIcon = { humanity: '🟠', twitch: '🟣', youtube: '🔴', rumble: '🟢' }[m.source] || '⚪';
    const name = m.from_name || m.source_user || 'Anonymous';
    return '<div class="stream-chat-msg">' + srcIcon + ' <span class="' + srcClass + '" style="font-weight:600;">' + escapeHtml(name) + '</span>: ' + escapeHtml(m.content) + '</div>';
   }).join('');
   container.scrollTop = container.scrollHeight;
  }

  function streamChatSend(context) {
   const inputId = context === 'viewer' ? 'viewer-chat-input' : 'stream-chat-input';
   const input = document.getElementById(inputId);
   if (!input || !input.value.trim()) return;
   const content = input.value.trim();
   input.value = '';
   if (ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_chat', content, source: 'humanity' }));
   }
  }

  function streamHandleMessage(msg) {
   switch (msg.type) {
    case 'stream_info':
     streamCurrentInfo = msg;
     streamUpdateLiveList();
     document.getElementById('stream-stat-viewers').textContent = 'Viewers: ' + (msg.viewer_count || 0);
     if (document.getElementById('viewer-stat-viewers')) {
      document.getElementById('viewer-stat-viewers').textContent = 'Viewers ' + (msg.viewer_count || 0);
     }
     document.getElementById('server-live-count').textContent = msg.active ? '1 live' : '0 live';
     break;

    case 'stream_chat': {
     streamChatMessages.push(msg);
     if (streamChatMessages.length > 500) streamChatMessages.shift();
     streamRenderChat();
     streamRenderChat('viewer');
     break;
    }

    case 'stream_offer':
    case 'stream_answer':
    case 'stream_ice':
     streamHandleSignaling(msg);
     break;
   }
  }

  function streamUpdateLiveList() {
   const container = document.getElementById('stream-live-list');
   const empty = document.getElementById('stream-live-empty');
   if (!streamCurrentInfo.active) {
    if (empty) empty.style.display = 'block';
    container.querySelectorAll('.viewer-card').forEach(c => c.remove());
    document.getElementById('stream-viewer-panel').classList.remove('active');
    return;
   }
   if (empty) empty.style.display = 'none';
   container.querySelectorAll('.viewer-card').forEach(c => c.remove());

   const card = document.createElement('div');
   card.className = 'viewer-card';
   card.onclick = () => streamViewerJoin();
   card.innerHTML = '<div class="thumb">🔴</div><div class="meta"><h3>' + escapeHtml(streamCurrentInfo.title || 'Live Stream') + '</h3><div class="sub">' + escapeHtml(streamCurrentInfo.streamer_name || 'Unknown') + ' · Viewers ' + (streamCurrentInfo.viewer_count || 0) + (streamCurrentInfo.category ? ' · ' + escapeHtml(streamCurrentInfo.category) : '') + '</div></div>';
   container.appendChild(card);

   if (streamCurrentInfo.external_urls) {
    streamCurrentInfo.external_urls.forEach(eu => {
     const ecard = document.createElement('div');
     ecard.className = 'viewer-card';
     ecard.onclick = () => streamViewerJoinExternal(eu.platform, eu.url);
     const icon = { twitch: '🟣', youtube: '🔴', rumble: '🟢' }[eu.platform] || 'LIVE';
     ecard.innerHTML = '<div class="thumb">' + icon + '</div><div class="meta"><h3>' + escapeHtml(eu.platform.charAt(0).toUpperCase() + eu.platform.slice(1)) + '</h3><div class="sub">' + escapeHtml(eu.url) + '</div></div>';
     container.appendChild(ecard);
    });
   }
  }

  // ── Viewer: Join/Leave Stream ──
  let viewerMetricsTimer = null;

  function streamViewerJoin() {
   const panel = document.getElementById('stream-viewer-panel');
   panel.classList.add('active');
   document.getElementById('viewer-stream-title').textContent = 'LIVE ' + (streamCurrentInfo.title || 'Live Stream');

   // Clean up previous viewer PC if exists (fixes retry bug)
   if (streamViewerPC) {
    streamViewerPC.close();
    streamViewerPC = null;
   }

   if (ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_viewer_join' }));
    if (streamCurrentInfo && streamCurrentInfo.streamer_key) {
     ws.send(JSON.stringify({ type: 'stream_offer', to: streamCurrentInfo.streamer_key, data: { type: 'request' }, from: marketMyKey }));
    }
   }

   // Start viewer metrics
   viewerMetricsPrevBytes = 0;
   viewerMetricsPrevTime = Date.now();
   if (viewerMetricsTimer) clearInterval(viewerMetricsTimer);
   viewerMetricsTimer = setInterval(viewerCollectMetrics, 2000);
  }

  function streamViewerJoinExternal(platform, url) {
   const panel = document.getElementById('stream-viewer-panel');
   panel.classList.add('active');
   document.getElementById('viewer-stream-title').textContent = 'LIVE ' + platform.charAt(0).toUpperCase() + platform.slice(1) + ' Stream';
   document.getElementById('viewer-video').style.display = 'none';
   const embedContainer = document.getElementById('viewer-embed-container');
   embedContainer.style.display = 'block';

   if (platform === 'twitch') {
    const channel = url.replace(/.*twitch\.tv\/?/i, '').replace(/\//g, '');
    embedContainer.innerHTML = '<iframe src="https://player.twitch.tv/?channel=' + encodeURIComponent(channel) + '&parent=' + location.hostname + '" width="100%" height="100%" frameborder="0" allowfullscreen></iframe>';
    if (channel) connectTwitchChat(channel);
   } else if (platform === 'youtube') {
    const videoId = url.match(/(?:live\/|watch\?v=|youtu\.be\/)([^&?/]+)/);
    if (videoId) {
     embedContainer.innerHTML = '<iframe src="https://www.youtube.com/embed/' + encodeURIComponent(videoId[1]) + '?autoplay=1" width="100%" height="100%" frameborder="0" allowfullscreen></iframe>';
    }
   } else if (platform === 'rumble') {
    embedContainer.innerHTML = '<iframe src="' + escapeHtml(url) + '" width="100%" height="100%" frameborder="0" allowfullscreen></iframe>';
   }
  }

  function streamViewerLeave() {
   document.getElementById('stream-viewer-panel').classList.remove('active');
   document.getElementById('viewer-video').style.display = 'block';
   document.getElementById('viewer-embed-container').style.display = 'none';
   document.getElementById('viewer-embed-container').innerHTML = '';
   if (ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_viewer_leave' }));
   }
   if (streamViewerPC) { streamViewerPC.close(); streamViewerPC = null; }
   if (viewerMetricsTimer) { clearInterval(viewerMetricsTimer); viewerMetricsTimer = null; }
   if (twitchChatWs) { twitchChatWs.close(); twitchChatWs = null; }
  }

  // ── WebRTC Signaling ──
  const streamViewerPCs = {};

  function streamCleanupViewerPC(key) {
   if (streamViewerPCs[key]) {
    try { streamViewerPCs[key].close(); } catch(e) {}
    delete streamViewerPCs[key];
   }
  }

  function streamCreateOfferForViewer(viewerKey) {
   // Close existing connection to this viewer
   streamCleanupViewerPC(viewerKey);

   const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }, { urls: 'stun:stun1.l.google.com:19302' }, { urls: 'stun:stun2.l.google.com:19302' }] });
   streamViewerPCs[viewerKey] = pc;
   streamPeerConnection = pc;

   // Clean up on disconnect
   pc.onconnectionstatechange = () => {
    if (pc.connectionState === 'disconnected' || pc.connectionState === 'failed' || pc.connectionState === 'closed') {
     streamCleanupViewerPC(viewerKey);
    }
   };

   if (streamCompositeStream) {
    streamCompositeStream.getTracks().forEach(t => pc.addTrack(t, streamCompositeStream));
   }

   // Apply bitrate
   const mbps = parseFloat(document.getElementById('stream-bitrate').value) || 6;
   const bps = mbps * 1000000;

   pc.onicecandidate = (e) => {
    if (e.candidate && ws && ws.readyState === 1) {
     ws.send(JSON.stringify({ type: 'stream_ice', to: viewerKey, data: e.candidate, from: marketMyKey }));
    }
   };

   pc.createOffer().then(offer => {
    pc.setLocalDescription(offer);
    if (ws && ws.readyState === 1) {
     ws.send(JSON.stringify({ type: 'stream_offer', to: viewerKey, data: offer, from: marketMyKey }));
    }
    // Apply bitrate after offer
    setTimeout(() => {
     pc.getSenders().forEach(sender => {
      if (sender.track && sender.track.kind === 'video') {
       const params = sender.getParameters();
       if (!params.encodings || params.encodings.length === 0) params.encodings = [{}];
       params.encodings[0].maxBitrate = bps;
       sender.setParameters(params).catch(() => {});
      }
     });
    }, 100);
   }).catch(e => console.warn('WebRTC offer failed:', e));
  }

  function streamHandleSignaling(msg) {
   // Streamer: handle request from viewer
   if (msg.type === 'stream_offer' && streamIsLive && msg.data && msg.data.type === 'request' && msg.from) {
    streamCreateOfferForViewer(msg.from);
   }

   // Viewer: handle offer from streamer
   if (msg.type === 'stream_offer' && !streamIsLive && msg.data && msg.data.type !== 'request') {
    // Close previous viewer PC (fixes retry + duplicate offer bug)
    if (streamViewerPC) {
     if (streamViewerPC.connectionState === 'connecting' || streamViewerPC.connectionState === 'connected') {
      // Already handling an offer, skip duplicate
      return;
     }
     streamViewerPC.close();
     streamViewerPC = null;
    }

    const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }, { urls: 'stun:stun1.l.google.com:19302' }, { urls: 'stun:stun2.l.google.com:19302' }] });
    streamViewerPC = pc;
    pc.ontrack = (e) => {
     document.getElementById('viewer-video').srcObject = e.streams[0];
    };
    pc.onicecandidate = (e) => {
     if (e.candidate && ws && ws.readyState === 1) {
      ws.send(JSON.stringify({ type: 'stream_ice', to: msg.from, data: e.candidate, from: marketMyKey }));
     }
    };
    pc.setRemoteDescription(new RTCSessionDescription(msg.data)).then(() => pc.createAnswer()).then(answer => {
     pc.setLocalDescription(answer);
     if (ws && ws.readyState === 1) {
      ws.send(JSON.stringify({ type: 'stream_answer', to: msg.from, data: answer, from: marketMyKey }));
     }
    }).catch(e => console.warn('WebRTC answer failed:', e));
   }

   // Streamer: handle answer from viewer
   if (msg.type === 'stream_answer' && streamIsLive) {
    const pc = streamViewerPCs[msg.from] || streamPeerConnection;
    if (pc) {
     pc.setRemoteDescription(new RTCSessionDescription(msg.data)).catch(e => console.warn('setRemoteDescription failed:', e));
    }
   }

   // ICE candidates
   if (msg.type === 'stream_ice') {
    let pc;
    if (streamIsLive) {
     pc = streamViewerPCs[msg.from] || streamPeerConnection;
    } else {
     pc = streamViewerPC;
    }
    if (pc) {
     pc.addIceCandidate(new RTCIceCandidate(msg.data)).catch(e => console.warn('addIceCandidate failed:', e));
    }
   }
  }

  // ── Twitch IRC Chat Integration ──
  function connectTwitchChat(channel) {
   if (twitchChatWs) { twitchChatWs.close(); }
   try {
    twitchChatWs = new WebSocket('wss://irc-ws.chat.twitch.tv:443');
    twitchChatWs.onopen = () => {
     twitchChatWs.send('CAP REQ :twitch.tv/tags');
     twitchChatWs.send('NICK justinfan' + Math.floor(Math.random() * 99999));
     twitchChatWs.send('JOIN #' + channel.toLowerCase());
    };
    twitchChatWs.onmessage = (e) => {
     const lines = e.data.split('\r\n');
     for (const line of lines) {
      if (line.startsWith('PING')) {
       twitchChatWs.send('PONG :tmi.twitch.tv');
       continue;
      }
      const privmsgMatch = line.match(/:([^!]+)![^ ]+ PRIVMSG #[^ ]+ :(.+)/);
      if (privmsgMatch) {
       const twitchUser = privmsgMatch[1];
       const twitchMsg = privmsgMatch[2];
       streamChatMessages.push({ content: twitchMsg, source: 'twitch', source_user: twitchUser, from_name: twitchUser, timestamp: Date.now() });
       if (streamChatMessages.length > 500) streamChatMessages.shift();
       streamRenderChat();
       streamRenderChat('viewer');
      }
     }
    };
    twitchChatWs.onerror = () => {};
    twitchChatWs.onclose = () => { twitchChatWs = null; };
   } catch (e) { console.warn('Twitch chat connect failed:', e); }
  }

  // Helper: escape HTML
  function escapeHtml(str) {
   if (typeof str !== 'string') return '';
   return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  // ── Request stream info on connect + periodically ──
  function streamRequestInfo() {
   if (ws && ws.readyState === 1) {
    ws.send(JSON.stringify({ type: 'stream_info_request' }));
   }
  }
  setInterval(streamRequestInfo, 15000);

  // Track tab visits for explorer achievement
  const visitedTabs = JSON.parse(localStorage.getItem('humanity_visited_tabs') || '[]');
  const origSwitchTab = switchTab;
  switchTab = function(tabId, pushState) {
   origSwitchTab(tabId, pushState);
   if (!visitedTabs.includes(tabId)) {
    visitedTabs.push(tabId);
    localStorage.setItem('humanity_visited_tabs', JSON.stringify(visitedTabs));
   }
   if (['reality', 'fantasy', 'streams', 'debug'].every(t => visitedTabs.includes(t))) {
    unlockAchievement('explorer');
   }
   // Refresh stream info when switching to Streams tab
   if (tabId === 'streams') streamRequestInfo();
  };
