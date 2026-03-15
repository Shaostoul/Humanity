// ── chat-voice.js ─────────────────────────────────────────────────────────
// Voice rooms, 1-on-1 WebRTC calls, video panel, stream overlays,
// unified right sidebar presence rendering.
// Depends on: app.js globals (ws, myKey, myName, peerData, esc, addSystemMessage,
//   openDmConversation, isFriend)
// ─────────────────────────────────────────────────────────────────────────

// ── WebRTC Config (shared by rooms and 1-on-1 calls) ──
const rtcConfig = {
  iceServers: [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
    { urls: 'turn:united-humanity.us:3478', username: 'humanity', credential: 'turnRelay2026!secure' },
    { urls: 'turns:united-humanity.us:5349', username: 'humanity', credential: 'turnRelay2026!secure' },
  ],
};

// ── Voice Channels (Persistent, SQLite-backed) ──
window._voiceChannels = [];
window._roomPeerConnections = {}; // key → RTCPeerConnection for mesh
window._roomLocalStream = null;
window._currentRoomId = null;

function createVoiceRoom() {
  const name = prompt('Voice channel name:');
  if (!name || !name.trim()) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'voice_room', action: 'create', room_name: name.trim() }));
  }
}

function joinVoiceRoom(roomId) {
  if (window._currentRoomId) {
    addSystemMessage('Leave your current voice channel first.');
    return;
  }
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'voice_room', action: 'join', room_id: String(roomId) }));
    window._currentRoomId = String(roomId);
    setupRoomAudio();
  }
}

function leaveVoiceRoom() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'voice_room', action: 'leave' }));
  }
  cleanupRoomAudio();
}

function deleteVoiceChannel(vcId) {
  if (!confirm('Delete this voice channel permanently?')) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'voice_room', action: 'delete', room_id: String(vcId) }));
  }
}

function getMicConstraints() {
  const preset = localStorage.getItem('humanity-mic-preset') || 'clarity';
  if (preset === 'noise_block') {
    return {
      echoCancellation: { ideal: true },
      noiseSuppression: { ideal: true },
      autoGainControl: { ideal: true },
      channelCount: { ideal: 1 },
      sampleRate: { ideal: 48000 }
    };
  }
  if (preset === 'natural') {
    return {
      echoCancellation: { ideal: true },
      noiseSuppression: { ideal: false },
      autoGainControl: { ideal: false },
      channelCount: { ideal: 1 },
      sampleRate: { ideal: 48000 }
    };
  }
  // clarity default
  return {
    echoCancellation: { ideal: true },
    noiseSuppression: { ideal: true },
    autoGainControl: { ideal: false },
    channelCount: { ideal: 1 },
    sampleRate: { ideal: 48000 }
  };
}

async function setupRoomAudio() {
  try {
    window._roomLocalStream = await navigator.mediaDevices.getUserMedia({ audio: getMicConstraints(), video: false });
  } catch (e) {
    addSystemMessage('⚠️ Microphone access denied.');
    leaveVoiceRoom();
    return;
  }
  addSystemMessage('🎧 Echo tip: headphones recommended for the clearest VOIP.');
  // Wait for voice_room_update to know who is in the room, then connect
}

function cleanupRoomAudio() {
  if (window._roomLocalStream) {
    window._roomLocalStream.getTracks().forEach(t => t.stop());
    window._roomLocalStream = null;
  }
  for (const [key, pc] of Object.entries(window._roomPeerConnections)) {
    pc.close();
  }
  window._roomPeerConnections = {};
  window._currentRoomId = null;
  // Remove room audio elements
  document.querySelectorAll('.room-remote-audio').forEach(el => el.remove());
  if (typeof renderServerList === 'function') renderServerList();
}

async function connectToRoomPeer(peerKey, peerName, roomId, isCaller) {
  if (window._roomPeerConnections[peerKey]) return; // already connected
  const pc = new RTCPeerConnection(rtcConfig);
  window._roomPeerConnections[peerKey] = pc;

  if (window._roomLocalStream) {
    window._roomLocalStream.getTracks().forEach(t => pc.addTrack(t, window._roomLocalStream));
  }

  pc.ontrack = (event) => {
    const audio = document.createElement('audio');
    audio.srcObject = event.streams[0];
    audio.autoplay = true;
    audio.playsInline = true;
    audio.className = 'room-remote-audio';
    audio.dataset.peerKey = peerKey;
    document.body.appendChild(audio);
    // Mobile browsers block autoplay — explicitly play with user gesture fallback
    const playPromise = audio.play();
    if (playPromise) {
      playPromise.catch(() => {
        console.warn('Autoplay blocked for peer', peerKey, '— waiting for user interaction');
        const resumeAudio = () => {
          audio.play().catch(() => {});
          document.removeEventListener('click', resumeAudio);
          document.removeEventListener('touchstart', resumeAudio);
        };
        document.addEventListener('click', resumeAudio, { once: true });
        document.addEventListener('touchstart', resumeAudio, { once: true });
        addSystemMessage('⚠️ Tap anywhere to unmute incoming audio (browser autoplay restriction).');
      });
    }
  };

  pc.onicecandidate = (event) => {
    if (event.candidate && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'voice_room_signal',
        from: myKey,
        to: peerKey,
        room_id: roomId,
        signal_type: 'ice',
        data: event.candidate.toJSON()
      }));
    }
  };

  pc.onconnectionstatechange = () => {
    console.log(`Voice peer ${peerKey.substring(0,8)}: ${pc.connectionState}`);
    if (pc.connectionState === 'connected') {
      addSystemMessage(`🔊 Voice connected to peer`);
    } else if (pc.connectionState === 'failed') {
      addSystemMessage(`⚠️ Voice connection failed — may need TURN server for NAT traversal`);
      pc.close();
      delete window._roomPeerConnections[peerKey];
      const audioEl = document.querySelector(`.room-remote-audio[data-peer-key="${peerKey}"]`);
      if (audioEl) audioEl.remove();
    } else if (pc.connectionState === 'disconnected') {
      // Give it a moment — might recover
      setTimeout(() => {
        if (pc.connectionState === 'disconnected') {
          pc.close();
          delete window._roomPeerConnections[peerKey];
          const audioEl = document.querySelector(`.room-remote-audio[data-peer-key="${peerKey}"]`);
          if (audioEl) audioEl.remove();
        }
      }, 5000);
    }
  };
  pc.onicegatheringstatechange = () => {
    console.log(`Voice ICE gathering: ${pc.iceGatheringState}`);
  };

  if (isCaller) {
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    ws.send(JSON.stringify({
      type: 'voice_room_signal',
      from: myKey,
      to: peerKey,
      room_id: roomId,
      signal_type: 'offer',
      data: offer
    }));
  }
}

// Handle voice_channel_list, voice_room_update, and voice_room_signal
const _origHandleMessageVR = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'voice_channel_list') {
    window._voiceChannels = (msg.channels || []).map(c => ({
      id: c.id,
      name: c.name,
      participants: (c.participants || []).map(p => ({
        public_key: p.public_key,
        display_name: p.display_name,
        muted: p.muted || false
      }))
    }));
    if (typeof renderServerList === 'function') renderServerList();
    // If we're in a room, connect to any new participants
    if (window._currentRoomId && window._roomLocalStream) {
      const ch = window._voiceChannels.find(c => String(c.id) === String(window._currentRoomId));
      if (ch) {
        for (const p of ch.participants) {
          if (p.public_key !== myKey && !window._roomPeerConnections[p.public_key]) {
            connectToRoomPeer(p.public_key, p.display_name, String(window._currentRoomId), true);
          }
        }
      } else {
        cleanupRoomAudio();
      }
    }
    return;
  }
  // Legacy voice_room_update — convert to voice_channel_list format
  if (msg.type === 'voice_room_update') {
    // Handled by voice_channel_list now; ignore.
    return;
  }
  if (msg.type === 'voice_room_signal') {
    handleVoiceRoomSignal(msg);
    return;
  }
  _origHandleMessageVR(msg);
};

async function handleVoiceRoomSignal(msg) {
  if (msg.to !== myKey) return;
  const peerKey = msg.from;
  const roomId = msg.room_id;

  if (msg.signal_type === 'new_participant') {
    // New person joined — they'll send us an offer, just wait
    return;
  }

  if (msg.signal_type === 'offer') {
    // Someone is sending us an offer — create connection and answer
    await connectToRoomPeer(peerKey, '', roomId, false);
    const pc = window._roomPeerConnections[peerKey];
    if (pc) {
      await pc.setRemoteDescription(new RTCSessionDescription(msg.data));
      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);
      ws.send(JSON.stringify({
        type: 'voice_room_signal',
        from: myKey,
        to: peerKey,
        room_id: roomId,
        signal_type: 'answer',
        data: answer
      }));
    }
    return;
  }

  if (msg.signal_type === 'answer') {
    const pc = window._roomPeerConnections[peerKey];
    if (pc) await pc.setRemoteDescription(new RTCSessionDescription(msg.data));
    return;
  }

  if (msg.signal_type === 'ice') {
    const pc = window._roomPeerConnections[peerKey];
    if (pc) {
      try { await pc.addIceCandidate(new RTCIceCandidate(msg.data)); } catch (e) {}
    }
    return;
  }
}

// Add voice room button styles
(function() {
  const style = document.createElement('style');
  style.textContent = `
    .vr-btn { font-size:0.7rem; padding:0.15rem 0.4rem; cursor:pointer; border-radius:4px; border:1px solid var(--border); background:var(--bg-input); color:var(--text-primary); }
    .vr-btn:hover { background:var(--bg-hover); }
    .vr-join { color:var(--success); border-color:var(--success); }
    .vr-leave { color:#e74c3c; border-color:#e74c3c; }
  `;
  document.head.appendChild(style);
})();

// ── Voice Control Bar + Speaking Indicators + Channel Cog ──
(function() {
  // Voice control bar state
  let vcMuted = false;
  let vcVolume = 100;
  let vcInputMode = localStorage.getItem('humanity-vc-input-mode') || 'open'; // open|ptt|vad
  let vcSquelch = localStorage.getItem('humanity-vc-squelch') === 'true';
  let vcThreshold = parseFloat(localStorage.getItem('humanity-vc-threshold') || '24');
  let vcPttKey = localStorage.getItem('humanity-vc-ptt-key') || 'KeyV';
  let vcPttDown = false;
  let vcSpeaking = false;
  let audioCtx = null;
  let localAnalyser = null;
  let speakingPollInterval = null;
  let remoteAnalysers = {}; // peerKey → { analyser, source, interval }

  document.addEventListener('keydown', (e) => {
    if (e.code === vcPttKey) {
      vcPttDown = true;
      applyTxGate();
    }
  });
  document.addEventListener('keyup', (e) => {
    if (e.code === vcPttKey) {
      vcPttDown = false;
      applyTxGate();
    }
  });

  window.toggleVoiceRoomMute = function() {
    if (!window._roomLocalStream) return;
    vcMuted = !vcMuted;
    applyTxGate();
    const btn = document.getElementById('vc-mute-btn');
    btn.textContent = vcMuted ? '🔇' : '🎤';
    btn.classList.toggle('vc-muted', vcMuted);
    btn.title = vcMuted ? 'Unmute' : 'Mute';
  };

  window.setVoiceRoomVolume = function(val) {
    vcVolume = parseInt(val);
    // Soft limit at 85% to reduce eardrum spikes.
    const applied = Math.min(85, Math.max(0, vcVolume));
    document.querySelectorAll('.room-remote-audio').forEach(el => {
      el.volume = applied / 100;
    });
  };

  function applyTxGate() {
    if (!window._roomLocalStream) return;
    let allow = !vcMuted;
    if (allow) {
      if (vcInputMode === 'ptt') {
        allow = vcPttDown;
      } else if (vcInputMode === 'vad' || vcSquelch) {
        allow = vcSpeaking && vcThreshold <= 100;
      }
    }
    window._roomLocalStream.getAudioTracks().forEach(t => { t.enabled = !!allow; });
  }

  function pttKeyLabel() {
    // Convert code like 'KeyV' → 'V', 'Space' → 'Space', 'F5' → 'F5'
    return vcPttKey.replace(/^Key/, '').replace(/^Digit/, '');
  }

  function refreshVoiceButtons() {
    const modeBtn = document.getElementById('vc-mode-btn');
    const sqBtn = document.getElementById('vc-squelch-btn');
    const presetBtn = document.getElementById('vc-mic-preset-btn');
    if (modeBtn) {
      const MODE_INFO = {
        open: {
          label: '🗣️ Open',
          desc: 'Open Mic — your mic transmits continuously whenever you are unmuted.',
          detail: `Best for: quiet rooms, casual chat · Next: PTT [${pttKeyLabel()}]`
        },
        ptt: {
          label: `🎙️ PTT [${pttKeyLabel()}]`,
          desc: `Push-to-Talk — hold ${pttKeyLabel()} to transmit. Mic is silent when the key is released.`,
          detail: 'Best for: noisy rooms, background audio, gaming · Next: VAD'
        },
        vad: {
          label: '🗣️ VAD',
          desc: 'Voice Activated — mic gates open automatically when your voice exceeds the noise threshold.',
          detail: 'Best for: hands-free use · Adjust threshold with Noise Gate · Next: Open'
        }
      };
      const mi = MODE_INFO[vcInputMode] || MODE_INFO.open;
      modeBtn.textContent = mi.label;
      modeBtn.setAttribute('data-tip-title', 'Input Mode — ' + mi.label.replace(/[🗣️🎙️]\s*/, ''));
      modeBtn.setAttribute('data-tip-desc', mi.desc);
      modeBtn.setAttribute('data-tip-detail', mi.detail);
    }
    if (sqBtn) {
      const gateOn = vcSquelch;
      sqBtn.textContent = gateOn ? '🚫 Gate On' : '🚫 Gate Off';
      sqBtn.setAttribute('data-tip-title', gateOn ? 'Noise Gate — On' : 'Noise Gate — Off');
      sqBtn.setAttribute('data-tip-desc', gateOn
        ? 'Gate is active — audio below the volume threshold is muted before it reaches others.'
        : 'Gate is off — all audio above the mic floor passes through (use with PTT or a quiet room).');
      sqBtn.setAttribute('data-tip-detail', 'Best for: suppressing keyboard clicks, mouse noise, breathing · Works alongside VAD mode');
    }
    if (presetBtn) {
      const p = localStorage.getItem('humanity-mic-preset') || 'clarity';
      const PRESET_INFO = {
        clarity: {
          label: '🎚️ Clarity',
          desc: 'Best for most calls. Removes background noise while keeping your volume natural — no pumping or over-compression.',
          detail: 'Echo cancel ✓ · Noise suppress ✓ · Auto-gain off · Next: Noise Block'
        },
        noise_block: {
          label: '🎚️ Noise Block',
          desc: 'Best for loud rooms (fans, keyboard, AC, café). Auto-adjusts your volume level and aggressively filters all background sound.',
          detail: 'Echo cancel ✓ · Noise suppress ✓ · Auto-gain ✓ · Next: Natural'
        },
        natural: {
          label: '🎚️ Natural',
          desc: 'Minimal processing — ideal for music, podcasting, or a quality mic in a quiet room. Captures your voice exactly as the mic hears it.',
          detail: 'Echo cancel ✓ · Noise suppress off · Auto-gain off · Use headphones to avoid echo · Next: Clarity'
        }
      };
      const info = PRESET_INFO[p] || PRESET_INFO.clarity;
      presetBtn.textContent = info.label;
      presetBtn.setAttribute('data-tip-title', 'Mic Preset — ' + info.label.replace('🎚️ ', ''));
      presetBtn.setAttribute('data-tip-desc', info.desc);
      presetBtn.setAttribute('data-tip-detail', info.detail);
    }
  }

  window.toggleVoiceInputMode = function() {
    vcInputMode = vcInputMode === 'open' ? 'ptt' : (vcInputMode === 'ptt' ? 'vad' : 'open');
    localStorage.setItem('humanity-vc-input-mode', vcInputMode);
    refreshVoiceButtons();
    applyTxGate();
  };

  window.toggleVoiceSquelch = function() {
    vcSquelch = !vcSquelch;
    localStorage.setItem('humanity-vc-squelch', vcSquelch ? 'true' : 'false');
    refreshVoiceButtons();
    applyTxGate();
  };

  window.cycleMicPreset = function() {
    const cur = localStorage.getItem('humanity-mic-preset') || 'clarity';
    const next = cur === 'clarity' ? 'noise_block' : (cur === 'noise_block' ? 'natural' : 'clarity');
    localStorage.setItem('humanity-mic-preset', next);
    refreshVoiceButtons();
    addSystemMessage('🎚️ Mic preset set to ' + next + '. Rejoin channel to fully apply capture constraints.');
  };

  setTimeout(refreshVoiceButtons, 0);

  /** Called from settings page to change input mode externally. */
  window.setVcInputMode = function(mode) {
    if (!['open','ptt','vad'].includes(mode)) return;
    vcInputMode = mode;
    localStorage.setItem('humanity-vc-input-mode', mode);
    refreshVoiceButtons();
    applyTxGate();
  };

  /** Capture the next keypress and assign it as the PTT key. */
  window.startVcPttRebind = function(onDone) {
    const btn = document.getElementById('vc-mode-btn');
    if (btn) btn.textContent = '⌨️ Press key…';
    function capture(e) {
      if (['Escape','Tab','Enter'].includes(e.key)) {
        document.removeEventListener('keydown', capture, true);
        refreshVoiceButtons();
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      document.removeEventListener('keydown', capture, true);
      vcPttKey = e.code;
      localStorage.setItem('humanity-vc-ptt-key', vcPttKey);
      refreshVoiceButtons();
      if (typeof onDone === 'function') onDone(vcPttKey);
    }
    document.addEventListener('keydown', capture, true);
  };

  /** Returns the current PTT key code (for settings display). */
  window.getVcPttKey = function() { return vcPttKey; };

  function updateVoiceControlBar() {
    const bar = document.getElementById('voice-control-bar');
    if (!bar) return;
    if (window._currentRoomId && window._roomLocalStream) {
      const ch = (window._voiceChannels || []).find(c => String(c.id) === String(window._currentRoomId));
      const name = ch ? ch.name : 'Unknown';
      document.getElementById('vc-bar-channel-name').textContent = '🔊 Connected to: ' + name;
      bar.classList.add('active');
    } else {
      bar.classList.remove('active');
      stopSpeakingDetection();
    }
  }

  // Speaking detection for local mic
  function startLocalSpeakingDetection() {
    if (!window._roomLocalStream) return;
    try {
      audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      const source = audioCtx.createMediaStreamSource(window._roomLocalStream);
      localAnalyser = audioCtx.createAnalyser();
      localAnalyser.fftSize = 256;
      source.connect(localAnalyser);
      const dataArray = new Uint8Array(localAnalyser.frequencyBinCount);
      speakingPollInterval = setInterval(() => {
        if (!localAnalyser) return;
        localAnalyser.getByteFrequencyData(dataArray);
        const avg = dataArray.reduce((a, b) => a + b, 0) / dataArray.length;
        const speaking = avg > vcThreshold;
        vcSpeaking = speaking;
        applyTxGate();
        const el = document.querySelector(`.vr-participant[data-participant-key="${myKey}"]`);
        if (el) el.classList.toggle('speaking', speaking);
      }, 100);
    } catch (e) { console.warn('Speaking detection failed:', e); }
  }

  // Speaking detection for remote streams
  function startRemoteSpeakingDetection(peerKey, stream) {
    if (remoteAnalysers[peerKey]) return;
    try {
      if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      const source = audioCtx.createMediaStreamSource(stream);
      const analyser = audioCtx.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);
      const dataArray = new Uint8Array(analyser.frequencyBinCount);
      const interval = setInterval(() => {
        analyser.getByteFrequencyData(dataArray);
        const avg = dataArray.reduce((a, b) => a + b, 0) / dataArray.length;
        const speaking = avg > 20;
        const el = document.querySelector(`.vr-participant[data-participant-key="${peerKey}"]`);
        if (el) el.classList.toggle('speaking', speaking);
      }, 100);
      remoteAnalysers[peerKey] = { analyser, source, interval };
    } catch (e) { console.warn('Remote speaking detection failed:', e); }
  }

  function stopSpeakingDetection() {
    if (speakingPollInterval) { clearInterval(speakingPollInterval); speakingPollInterval = null; }
    localAnalyser = null;
    for (const [key, r] of Object.entries(remoteAnalysers)) {
      clearInterval(r.interval);
    }
    remoteAnalysers = {};
    if (audioCtx) { audioCtx.close().catch(() => {}); audioCtx = null; }
    // Remove speaking classes
    vcSpeaking = false;
    document.querySelectorAll('.vr-participant.speaking').forEach(el => el.classList.remove('speaking'));
  }

  // Patch setupRoomAudio to start speaking detection + update bar
  const _origSetupRoomAudio = setupRoomAudio;
  window.setupRoomAudio = async function() {
    await _origSetupRoomAudio();
    if (window._roomLocalStream) {
      startLocalSpeakingDetection();
      updateVoiceControlBar();
      // Reset mute state
      vcMuted = false;
      vcPttDown = false;
      vcSpeaking = false;
      const btn = document.getElementById('vc-mute-btn');
      if (btn) { btn.textContent = '🎤'; btn.classList.remove('vc-muted'); }
      refreshVoiceButtons();
      applyTxGate();
    }
  };

  // Patch cleanupRoomAudio to hide bar
  const _origCleanupRoomAudio = cleanupRoomAudio;
  window.cleanupRoomAudio = function() {
    stopSpeakingDetection();
    _origCleanupRoomAudio();
    updateVoiceControlBar();
  };

  // Patch connectToRoomPeer to add remote speaking detection + volume
  const _origConnectToRoomPeer = connectToRoomPeer;
  window.connectToRoomPeer = async function(peerKey, peerName, roomId, isCaller) {
    await _origConnectToRoomPeer(peerKey, peerName, roomId, isCaller);
    const pc = window._roomPeerConnections[peerKey];
    if (pc) {
      const origOnTrack = pc.ontrack;
      pc.ontrack = function(event) {
        if (origOnTrack) origOnTrack.call(this, event);
        // Apply volume to new audio elements
        setTimeout(() => {
          document.querySelectorAll('.room-remote-audio').forEach(el => { el.volume = vcVolume / 100; });
          // Start speaking detection for this remote stream
          if (event.streams[0]) startRemoteSpeakingDetection(peerKey, event.streams[0]);
        }, 100);
      };
    }
  };

  // Patch renderServerList to update voice control bar
  const _origRenderServerList = window.renderServerList;
  window.renderServerList = function() {
    _origRenderServerList();
    updateVoiceControlBar();
  };

  // ── Channel Settings Cog ──
  let activeCogDropdown = null;

  document.addEventListener('click', function(e) {
    // Close any open cog dropdown
    if (activeCogDropdown && !e.target.closest('.cog-dropdown') && !e.target.closest('.channel-cog')) {
      activeCogDropdown.remove();
      activeCogDropdown = null;
    }

    const cog = e.target.closest('.channel-cog');
    if (!cog) return;
    e.stopPropagation();
    e.preventDefault();

    // Close existing
    if (activeCogDropdown) { activeCogDropdown.remove(); activeCogDropdown = null; }

    const type = cog.dataset.cogType;
    const id = cog.dataset.cogId;
    const name = cog.dataset.cogName;

    const dropdown = document.createElement('div');
    dropdown.className = 'cog-dropdown';

    if (type === 'text') {
      dropdown.innerHTML = `
        <div class="cog-item" data-cog-action="rename">✏️ Rename</div>
        <div class="cog-item danger" data-cog-action="delete">🗑️ Delete</div>
      `;
      dropdown.addEventListener('click', function(ev) {
        const item = ev.target.closest('.cog-item');
        if (!item) return;
        const action = item.dataset.cogAction;
        if (action === 'rename') {
          const newName = prompt('New channel name:', name);
          if (newName && newName.trim() && newName.trim() !== name) {
            if (!ws || ws.readyState !== WebSocket.OPEN) {
              addNotice('Not connected. Reconnect, then retry rename.', 'red', 8);
              return;
            }
            if (!beginChannelAdminCmd('rename')) return;
            addSystemMessage('⏳ Renaming #' + name + ' → #' + newName.trim().toLowerCase() + ' ...');
            sendChatCommand('/channel-edit ' + name + ' name ' + newName.trim(), 'general').then(ok => { if (!ok) failChannelAdminCmd('Rename command failed to send.'); }).catch(console.error);
          }
        } else if (action === 'delete') {
          if (confirm('Delete channel "' + name + '"? This cannot be undone.')) {
            if (!ws || ws.readyState !== WebSocket.OPEN) {
              addNotice('Not connected. Reconnect, then retry delete.', 'red', 8);
              return;
            }
            const normalized = String(name || '').trim().replace(/^#/, '').toLowerCase();
            if (!beginChannelAdminCmd('delete')) return;
            addSystemMessage('⏳ Deleting #' + normalized + ' ...');
            // Route admin channel-management commands through #general for consistent server handling.
            sendChatCommand('/channel-delete ' + normalized, 'general').then(ok => { if (!ok) failChannelAdminCmd('Delete command failed to send.'); }).catch(console.error);
          }
        }
        dropdown.remove();
        activeCogDropdown = null;
      });
    } else if (type === 'voice') {
      dropdown.innerHTML = `
        <div class="cog-item" data-cog-action="rename">✏️ Rename</div>
        <div class="cog-item danger" data-cog-action="delete">🗑️ Delete</div>
      `;
      dropdown.addEventListener('click', function(ev) {
        const item = ev.target.closest('.cog-item');
        if (!item) return;
        const action = item.dataset.cogAction;
        if (action === 'rename') {
          const newName = prompt('New voice channel name:', name);
          if (newName && newName.trim() && newName.trim() !== name) {
            if (!ws || ws.readyState !== WebSocket.OPEN) {
              addNotice('Not connected. Reconnect, then retry voice rename.', 'red', 8);
              return;
            }
            ws.send(JSON.stringify({ type: 'voice_room', action: 'rename', room_id: String(id), room_name: newName.trim() }));
            addNotice('Voice channel rename sent.', 'cyan', 4);
          }
        } else if (action === 'delete') {
          if (confirm('Delete voice channel "' + name + '"?')) {
            if (!ws || ws.readyState !== WebSocket.OPEN) {
              addNotice('Not connected. Reconnect, then retry voice delete.', 'red', 8);
              return;
            }
            ws.send(JSON.stringify({ type: 'voice_room', action: 'delete', room_id: String(id) }));
            addNotice('Voice channel delete sent.', 'cyan', 4);
          }
        }
        dropdown.remove();
        activeCogDropdown = null;
      });
    }

    cog.style.position = 'relative';
    cog.appendChild(dropdown);
    activeCogDropdown = dropdown;
  });
})();

// ── Voice Call / WebRTC (1-on-1 DM calls) ──
let callState = 'idle'; // idle | ringing-out | ringing-in | in-call
let callPeerKey = null;
let callPeerName = '';
let peerConnection = null;
let pendingIceCandidates = []; // Buffer ICE candidates arriving before PC is ready
let remoteDescriptionSet = false; // Track whether remote description has been set
let localStream = null;
let callTimerInterval = null;
let callStartTime = null;
let isMuted = false;

function startCall(targetKey, targetName) {
  if (callState !== 'idle') {
    addSystemMessage('You are already in a call or ringing.');
    return;
  }
  if (!ws || ws.readyState !== WebSocket.OPEN) return;

  callState = 'ringing-out';
  callPeerKey = targetKey;
  callPeerName = targetName;

  ws.send(JSON.stringify({
    type: 'voice_call',
    from: myKey,
    to: targetKey,
    action: 'ring'
  }));

  // Show ringing status
  document.getElementById('ringing-status').textContent = `📞 Calling ${esc(targetName)}…`;
  document.getElementById('ringing-status').classList.add('active');

  // Auto-cancel after 30s
  setTimeout(() => {
    if (callState === 'ringing-out') {
      hangupCall();
      addSystemMessage(`${targetName} didn't answer.`);
    }
  }, 30000);
}

function acceptIncomingCall() {
  if (callState !== 'ringing-in') return;
  callState = 'in-call';
  document.getElementById('incoming-call-overlay').classList.remove('open');

  ws.send(JSON.stringify({
    type: 'voice_call',
    from: myKey,
    to: callPeerKey,
    action: 'accept'
  }));

  // Callee waits for the offer from caller
  showCallBar();
}

function rejectIncomingCall() {
  if (callState !== 'ringing-in') return;
  document.getElementById('incoming-call-overlay').classList.remove('open');

  ws.send(JSON.stringify({
    type: 'voice_call',
    from: myKey,
    to: callPeerKey,
    action: 'reject'
  }));

  resetCallState();
}

function hangupCall() {
  if (callState === 'idle') return;

  if (ws && ws.readyState === WebSocket.OPEN && callPeerKey) {
    ws.send(JSON.stringify({
      type: 'voice_call',
      from: myKey,
      to: callPeerKey,
      action: 'hangup'
    }));
  }

  cleanupCall();
}

function cleanupCall() {
  if (peerConnection) {
    peerConnection.close();
    peerConnection = null;
  }
  if (localStream) {
    localStream.getTracks().forEach(t => t.stop());
    localStream = null;
  }
  pendingIceCandidates = [];
  remoteDescriptionSet = false;
  resetCallState();
}

function resetCallState() {
  callState = 'idle';
  callPeerKey = null;
  callPeerName = '';
  isMuted = false;
  if (callTimerInterval) { clearInterval(callTimerInterval); callTimerInterval = null; }
  callStartTime = null;
  document.getElementById('call-bar').classList.remove('active');
  document.getElementById('ringing-status').classList.remove('active');
  document.getElementById('incoming-call-overlay').classList.remove('open');
  const muteBtn = document.getElementById('mute-btn');
  muteBtn.classList.remove('muted');
  muteBtn.textContent = '🎤 Mute';
}

function showCallBar() {
  document.getElementById('call-peer-name').textContent = `In call with ${callPeerName}`;
  document.getElementById('call-bar').classList.add('active');
  document.getElementById('ringing-status').classList.remove('active');
  callStartTime = Date.now();
  callTimerInterval = setInterval(updateCallTimer, 1000);
}

function updateCallTimer() {
  if (!callStartTime) return;
  const elapsed = Math.floor((Date.now() - callStartTime) / 1000);
  const m = Math.floor(elapsed / 60).toString().padStart(2, '0');
  const s = (elapsed % 60).toString().padStart(2, '0');
  document.getElementById('call-timer').textContent = `${m}:${s}`;
}

function toggleMute() {
  if (!localStream) return;
  isMuted = !isMuted;
  localStream.getAudioTracks().forEach(t => { t.enabled = !isMuted; });
  const btn = document.getElementById('mute-btn');
  btn.classList.toggle('muted', isMuted);
  btn.textContent = isMuted ? '🔇 Unmute' : '🎤 Mute';
}

async function setupPeerConnection(isCaller) {
  peerConnection = new RTCPeerConnection(rtcConfig);

  // Get microphone
  try {
    localStream = await navigator.mediaDevices.getUserMedia({ audio: getMicConstraints(), video: false });
  } catch (e) {
    addSystemMessage('⚠️ Microphone access denied. Cannot make voice call.');
    hangupCall();
    return false;
  }

  localStream.getTracks().forEach(t => peerConnection.addTrack(t, localStream));

  // Play remote audio
  peerConnection.ontrack = (event) => {
    const audio = new Audio();
    audio.srcObject = event.streams[0];
    audio.autoplay = true;
    audio.playsInline = true;
    audio.id = 'remote-audio';
    // Remove old one if any
    const old = document.getElementById('remote-audio');
    if (old) old.remove();
    document.body.appendChild(audio);
    const pp = audio.play();
    if (pp) pp.catch(() => {
      addSystemMessage('⚠️ Tap anywhere to hear incoming audio.');
      const resume = () => { audio.play().catch(()=>{}); document.removeEventListener('click', resume); document.removeEventListener('touchstart', resume); };
      document.addEventListener('click', resume, { once: true });
      document.addEventListener('touchstart', resume, { once: true });
    });
  };

  // ICE candidates → send to peer
  peerConnection.onicecandidate = (event) => {
    if (event.candidate && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'webrtc_signal',
        from: myKey,
        to: callPeerKey,
        signal_type: 'ice',
        data: event.candidate.toJSON()
      }));
    }
  };

  peerConnection.onconnectionstatechange = () => {
    if (peerConnection && (peerConnection.connectionState === 'disconnected' || peerConnection.connectionState === 'failed')) {
      addSystemMessage('Call disconnected.');
      cleanupCall();
    }
  };

  return true;
}

async function createAndSendOffer() {
  if (!await setupPeerConnection(true)) return;
  const offer = await peerConnection.createOffer();
  await peerConnection.setLocalDescription(offer);
  ws.send(JSON.stringify({
    type: 'webrtc_signal',
    from: myKey,
    to: callPeerKey,
    signal_type: 'offer',
    data: offer
  }));
  showCallBar();
}

async function handleOffer(data) {
  if (!await setupPeerConnection(false)) return;
  await peerConnection.setRemoteDescription(new RTCSessionDescription(data));
  remoteDescriptionSet = true;
  await flushPendingIceCandidates();
  const answer = await peerConnection.createAnswer();
  await peerConnection.setLocalDescription(answer);
  ws.send(JSON.stringify({
    type: 'webrtc_signal',
    from: myKey,
    to: callPeerKey,
    signal_type: 'answer',
    data: answer
  }));
}

async function handleAnswer(data) {
  if (peerConnection) {
    await peerConnection.setRemoteDescription(new RTCSessionDescription(data));
    remoteDescriptionSet = true;
    await flushPendingIceCandidates();
  }
}

async function handleIceCandidate(data) {
  if (peerConnection && remoteDescriptionSet) {
    try {
      await peerConnection.addIceCandidate(new RTCIceCandidate(data));
    } catch (e) {
      console.warn('ICE candidate error:', e);
    }
  } else {
    // Buffer candidates until PC + remote description are ready
    pendingIceCandidates.push(data);
  }
}

async function flushPendingIceCandidates() {
  if (!peerConnection) return;
  const candidates = pendingIceCandidates.splice(0);
  for (const data of candidates) {
    try {
      await peerConnection.addIceCandidate(new RTCIceCandidate(data));
    } catch (e) {
      console.warn('ICE candidate error (buffered):', e);
    }
  }
}

// Handle voice_call and webrtc_signal messages
const _origHandleMessage3 = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'voice_call') {
    handleVoiceCallMessage(msg);
    return;
  }
  if (msg.type === 'webrtc_signal') {
    handleWebrtcSignalMessage(msg);
    return;
  }
  _origHandleMessage3(msg);
};

function handleVoiceCallMessage(msg) {
  const fromName = resolveSenderName(msg.from_name, msg.from);
  switch (msg.action) {
    case 'ring':
      if (callState !== 'idle') {
        // Already busy — auto-reject
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'voice_call', from: myKey, to: msg.from, action: 'reject' }));
        }
        return;
      }
      callState = 'ringing-in';
      callPeerKey = msg.from;
      callPeerName = fromName;
      document.getElementById('incoming-caller-name').textContent = fromName;
      document.getElementById('incoming-call-overlay').classList.add('open');
      playNotificationChime();
      break;
    case 'accept':
      if (callState === 'ringing-out' && msg.from === callPeerKey) {
        callState = 'in-call';
        // Caller creates the offer
        createAndSendOffer();
      }
      break;
    case 'reject':
      if ((callState === 'ringing-out') && msg.from === callPeerKey) {
        addSystemMessage(`${callPeerName} rejected the call.`);
        resetCallState();
      }
      break;
    case 'hangup':
      if (msg.from === callPeerKey) {
        addSystemMessage(`${callPeerName} hung up.`);
        cleanupCall();
      }
      break;
  }
}

function handleWebrtcSignalMessage(msg) {
  // DataChannel P2P signals are handled by chat-p2p.js; route them there first.
  if (msg.signal_type === 'dc_offer')  { handleDCOffer(msg);  return; }
  if (msg.signal_type === 'dc_answer') { handleDCAnswer(msg); return; }
  if (msg.signal_type === 'dc_ice')    { handleDCIce(msg);    return; }

  // Voice/video signals are only valid from the current call peer.
  if (msg.from !== callPeerKey) return;
  switch (msg.signal_type) {
    case 'offer':  handleOffer(msg.data);        break;
    case 'answer': handleAnswer(msg.data);       break;
    case 'ice':    handleIceCandidate(msg.data); break;
  }
}

// Auto-hangup on WebSocket disconnect
const _origWsOnClose = null; // We'll patch the openSocket function
const _origOpenSocket = openSocket;
openSocket = function() {
  _origOpenSocket();
  // Patch onclose to also cleanup call
  const currentWs = ws;
  if (currentWs) {
    const origOnClose = currentWs.onclose;
    currentWs.onclose = function() {
      if (callState !== 'idle') {
        addSystemMessage('Call ended (disconnected).');
        cleanupCall();
      }
      if (origOnClose) origOnClose.apply(this, arguments);
    };
  }
};

let allUsersSnapshot = [];
window.__UNIFIED_RIGHT_SIDEBAR__ = true;

function getActiveSidebarTabName() {
  const el = document.querySelector('#sidebar-tabs .sidebar-tab.active');
  return el ? el.getAttribute('data-tab') : 'servers';
}

function toggleUnifiedSection(id) {
  const key = 'humanity-unified-right-collapsed';
  let state = {};
  try { state = JSON.parse(localStorage.getItem(key) || '{}') || {}; } catch (_) {}
  state[id] = !state[id];
  localStorage.setItem(key, JSON.stringify(state));
  // Also toggle static HTML sections (e.g. #stream-studio-panel) that aren't
  // re-rendered by renderUnifiedRightSidebar.
  const staticEl = document.querySelector(`.unified-section[data-usid="${CSS.escape(id)}"]`);
  if (staticEl) {
    staticEl.classList.toggle('collapsed', !!state[id]);
    const btn = staticEl.querySelector('.unified-header');
    if (btn) {
      const title = btn.textContent.replace(/\s*[▾▸]\s*$/, '').trim();
      btn.textContent = title + ' ' + (state[id] ? '▸' : '▾');
    }
  }
  renderUnifiedRightSidebar();
}
window.toggleUnifiedSection = toggleUnifiedSection;

function toggleUnifiedSubSection(id) {
  const key = 'humanity-unified-right-sub-collapsed';
  let state = {};
  try { state = JSON.parse(localStorage.getItem(key) || '{}') || {}; } catch (_) {}
  state[id] = !state[id];
  localStorage.setItem(key, JSON.stringify(state));
  renderUnifiedRightSidebar();
}
window.toggleUnifiedSubSection = toggleUnifiedSubSection;

function toggleStreamVisibilityById(id) {
  if (!activeStreams || !activeStreams.has(id)) return;
  const s = activeStreams.get(id);
  s.hidden = !s.hidden;
  s.wrapper.style.display = s.hidden ? 'none' : '';
  renderUnifiedRightSidebar();
}
window.toggleStreamVisibilityById = toggleStreamVisibilityById;

function renderUnifiedSection(id, title, streamRows, voipRows, onlineRows, offlineRows, previewHtml) {
  const key = 'humanity-unified-right-collapsed';
  const subKey = 'humanity-unified-right-sub-collapsed';
  let state = {};
  let sub = {};
  try { state = JSON.parse(localStorage.getItem(key) || '{}') || {}; } catch (_) {}
  try { sub = JSON.parse(localStorage.getItem(subKey) || '{}') || {}; } catch (_) {}
  const collapsed = !!state[id];

  const sectionSub = (name, label, rowsArr, noneLabel, preview = '') => {
    const sid = `${id}:${name}`;
    const c = !!sub[sid];
    const summary = rowsArr.length ? `(${rowsArr.length})` : '(none)';
    const body = rowsArr.length ? rowsArr.join('') : `<div class="stream-empty"></div>`;
    return `<div class="unified-subblock${c ? ' collapsed' : ''}" data-subid="${esc(sid)}">
      <button class="unified-subhead-toggle" onclick="toggleUnifiedSubSection('${esc(sid)}')">
        <span>${label} ${c ? '▸' : '▾'}</span>
        <span class="unified-subsummary">${summary}</span>
      </button>
      <div class="unified-subcontent">${preview}${body}</div>
    </div>`;
  };

  return `<div class="unified-section${collapsed ? ' collapsed' : ''}" data-usid="${esc(id)}">
    <button class="unified-header" onclick="toggleUnifiedSection('${esc(id)}')">${esc(title)} ${collapsed ? '▸' : '▾'}</button>
    <div class="unified-body">
      ${sectionSub('streaming', 'Streaming', streamRows, 'No active streams', previewHtml || '')}
      ${sectionSub('voip', 'VOIP', voipRows, 'No active voice')}
      ${sectionSub('online', 'Online', onlineRows, 'No online users')}
      ${sectionSub('offline', 'Offline', offlineRows, 'No offline users')}
    </div>
  </div>`;
}

function renderUnifiedRightSidebar() {
  const peerList = document.getElementById('peer-list');
  if (!peerList) return;

  const users = Array.isArray(allUsersSnapshot) ? allUsersSnapshot : [];
  const byKey = new Map(users.map(u => [u.public_key, u]));
  const active = activeStreams || new Map();

  const friendUsers = users.filter(u => u.public_key !== myKey && isFriend(u.public_key));
  const friendOnline = friendUsers.filter(u => !!u.online);
  const friendOffline = friendUsers.filter(u => !u.online);
  const friendStreamRows = [];
  const friendPreview = [];
  active.forEach((s, id) => {
    const row = `<div class="unified-row"><span>${esc(s.name || id)}</span><button onclick="toggleStreamVisibilityById('${esc(id)}')">${s.hidden ? 'Watch' : 'Hide'}</button></div>`;
    if ((s.name || '').toLowerCase().includes('dm') || (s.name || '').toLowerCase().includes('friend')) friendStreamRows.push(row);
  });

  const friendVoip = friendOnline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>🎤 ${esc(u.name || shortKey(u.public_key))}</span></div>`);
  const friendOnlineRows = friendOnline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>🟢 ${esc(u.name || shortKey(u.public_key))}</span></div>`);
  const friendOfflineRows = friendOffline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>⚫ ${esc(u.name || shortKey(u.public_key))}</span></div>`);

  const sections = [];
  sections.push(renderUnifiedSection('friends', 'Friends', friendStreamRows, friendVoip, friendOnlineRows, friendOfflineRows, friendPreview.join('')));

  const groups = (myGroups || []);
  if (groups.length === 0) {
    sections.push(renderUnifiedSection('group-none', 'Groups (none)', [], [], [], [], ''));
  } else {
    groups.forEach(g => {
      const members = (groupMembersByGroup[g.id] || []).map(m => byKey.get(m.key) || { public_key: m.key, name: shortKey(m.key), online: false });
      const online = members.filter(m => !!m.online);
      const offline = members.filter(m => !m.online);
      const streamRows = [];
      active.forEach((s, id) => {
        if ((s.name || '').toLowerCase().includes((g.name || '').toLowerCase())) {
          streamRows.push(`<div class="unified-row"><span>${esc(s.name || id)}</span><button onclick="toggleStreamVisibilityById('${esc(id)}')">${s.hidden ? 'Watch' : 'Hide'}</button></div>`);
        }
      });
      const voipRows = online.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>🎤 ${esc(u.name || shortKey(u.public_key))}</span></div>`);
      const onlineRows = online.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>🟢 ${esc(u.name || shortKey(u.public_key))}</span></div>`);
      const offlineRows = offline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>⚫ ${esc(u.name || shortKey(u.public_key))}</span></div>`);
      sections.push(renderUnifiedSection('group-' + g.id, `Groups (${g.name})`, streamRows, voipRows, onlineRows, offlineRows, ''));
    });
  }

  const serverOnline = users.filter(u => !!u.online && u.public_key !== myKey);
  const serverOffline = users.filter(u => !u.online && u.public_key !== myKey);
  const serverVoipRows = [];
  (window._voiceChannels || []).forEach(vc => {
    vc.participants.forEach(p => {
      serverVoipRows.push(`<div class="unified-row"><span>🎤 ${esc(p.display_name)} · ${esc(vc.name)}</span></div>`);
    });
  });
  if (window._currentRoomId) {
    const ch = (window._voiceChannels || []).find(c => String(c.id) === String(window._currentRoomId));
    const label = ch ? ch.name : ('Room ' + window._currentRoomId);
    if (!serverVoipRows.some(r => r.includes('(you)'))) {
      serverVoipRows.unshift(`<div class="unified-row"><span>🎤 ${esc(myName || shortKey(myKey))} (you) · ${esc(label)}</span></div>`);
    }
  }
  const serverStreamRows = [];
  active.forEach((s, id) => {
    serverStreamRows.push(`<div class="unified-row"><span>${esc(s.name || id)}</span><button onclick="toggleStreamVisibilityById('${esc(id)}')">${s.hidden ? 'Watch' : 'Hide'}</button></div>`);
  });
  const serverOnlineRows = serverOnline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>🟢 ${esc(u.name || shortKey(u.public_key))}</span></div>`);
  const serverOfflineRows = serverOffline.map(u => `<div class="unified-row peer" data-username="${esc(u.name || shortKey(u.public_key))}" data-pubkey="${esc(u.public_key)}"><span>⚫ ${esc(u.name || shortKey(u.public_key))}</span></div>`);
  sections.push(renderUnifiedSection('server-main', 'Servers (United-Humanity)', serverStreamRows, serverVoipRows, serverOnlineRows, serverOfflineRows, ''));

  peerList.innerHTML = sections.join('');
}

function renderPresenceSidebarForActiveContext() {
  const peerList = document.getElementById('peer-list');
  const usersHeader = document.querySelector('#right-sidebar h3');
  if (!peerList) return;

  if (window.__UNIFIED_RIGHT_SIDEBAR__) {
    if (usersHeader) usersHeader.textContent = 'People & Streams';
    renderUnifiedRightSidebar();
    return;
  }

  const tab = getActiveSidebarTabName();

  if (tab === 'servers') {
    if (usersHeader) usersHeader.textContent = 'Users';
    // Re-render normal global list from snapshot.
    if (Array.isArray(allUsersSnapshot) && allUsersSnapshot.length > 0) {
      _origUpdateUserList(allUsersSnapshot);
      addCallButtonsToPeerList();
      if (typeof updateFriendIndicators === 'function') updateFriendIndicators();
    }
    if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
    return;
  }

  if (tab === 'dms') {
    if (usersHeader) usersHeader.textContent = 'Friends (DM)';
    const byKey = new Map((allUsersSnapshot || []).map(u => [u.public_key, u]));
    const friendRows = (allUsersSnapshot || []).filter(u => u.public_key !== myKey && isFriend(u.public_key));
    if (friendRows.length === 0) {
      peerList.innerHTML = '<div style="font-size:0.75rem;color:var(--text-muted);padding:0.35rem;">No friends yet. Mutual follow is required for DMs.</div>';
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const rows = friendRows.map(u => {
      const online = !!u.online;
      const dot = online ? '🟢' : '⚫';
      const name = esc(u.name || shortKey(u.public_key));
      const dmConv = (dmConversations || []).find(c => c.partner_key === u.public_key);
      const unread = dmConv && dmConv.unread_count ? ` <span style="color:var(--accent);font-size:0.68rem;">(${dmConv.unread_count})</span>` : '';
      return `<div class="peer" data-pubkey="${esc(u.public_key)}" style="opacity:${online ? '1' : '0.65'}">${dot} ${name}${unread}</div>`;
    }).join('');
    peerList.innerHTML = `<div style="font-size:0.62rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:0.3rem;">Friends</div>${rows}`;
    if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
    return;
  }

  if (tab === 'groups') {
    if (usersHeader) usersHeader.textContent = 'Group Members';
    if (!activeGroupId) {
      peerList.innerHTML = '<div style="font-size:0.75rem;color:var(--text-muted);padding:0.35rem;">Open a group to view members.</div>';
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const g = (myGroups || []).find(x => x.id === activeGroupId);
    const gName = g ? esc(g.name) : 'Current Group';
    const members = groupMembersByGroup[activeGroupId] || [];
    const byKey = new Map((allUsersSnapshot || []).map(u => [u.public_key, u]));
    if (members.length === 0) {
      peerList.innerHTML = `<div style="font-size:0.75rem;color:var(--text-muted);padding:0.35rem;">Loading members for <b>${gName}</b>...</div>`;
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const rows = members.map(m => {
      const u = byKey.get(m.key);
      const online = !!(u && u.online);
      const dot = online ? '🟢' : '⚫';
      const name = esc((u && u.name) ? u.name : shortKey(m.key));
      const role = m.role ? ` <span style="font-size:0.64rem;color:var(--text-muted);">(${esc(m.role)})</span>` : '';
      return `<div class="peer" data-pubkey="${esc(m.key)}" style="opacity:${online ? '1' : '0.65'}">${dot} ${name}${role}</div>`;
    }).join('');
    peerList.innerHTML = `<div style="font-size:0.62rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:0.3rem;">${gName} (${members.length})</div>${rows}`;
    if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
    return;
  }
}

function addCallButtonsToPeerList() {
  const peerList = document.getElementById('peer-list');
  if (!peerList) return;
  peerList.querySelectorAll('.peer[data-pubkey]').forEach(el => {
    const pk = el.dataset.pubkey;
    const name = el.dataset.username;
    if (pk === myKey || (pk && pk.startsWith('bot_'))) return;
    // Only add to online users
    if (el.style.opacity === '0.5') return; // offline users have opacity 0.5
    // Check if already has call button
    if (el.querySelector('.call-btn')) return;
    const btn = document.createElement('button');
    btn.className = 'call-btn';
    btn.textContent = '📞';
    btn.title = `Call ${name}`;
    btn.onclick = (e) => {
      e.stopPropagation();
      startCall(pk, name);
    };
    el.appendChild(btn);
    if (window.twemoji) twemoji.parse(btn);
  });
}

// Add 📞 call buttons to user list
const _origUpdateUserList = updateUserList;
updateUserList = function(users) {
  allUsersSnapshot = Array.isArray(users) ? users : [];
  _origUpdateUserList(users);
  addCallButtonsToPeerList();
  renderPresenceSidebarForActiveContext();
  renderStreamSidebar();
};

  // ── Phase 2: Video Calls + Screen Share ──

  // Stream/watch state (default off for auto-watch)
  let autoWatchStreams = localStorage.getItem('humanity-auto-watch-streams') === 'true';
  const activeStreams = new Map(); // id -> { name, wrapper, video, hidden }

  function toggleAutoWatchStreams(enabled) {
    autoWatchStreams = !!enabled;
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

  // --- DM Call Video ---
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
  document.getElementById('video-btn').textContent = '📹 On';
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
document.getElementById('video-btn').textContent = '📹 Video';
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
  document.getElementById('screen-btn').textContent = '🖥️ On';
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
if (btn) { btn.classList.remove('active'); btn.textContent = '🖥️ Screen'; }
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
if (vb) { vb.classList.remove('active'); vb.textContent = '📹 Video'; }
const sb = document.getElementById('screen-btn');
if (sb) { sb.classList.remove('active'); sb.textContent = '🖥️ Screen'; }
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

  // --- Voice Room Video ---
  let vrVideoStream = null;
  let vrScreenStream = null;
  let vrVideoActive = false;
  let vrScreenActive = false;

  window.toggleVoiceRoomVideo = async function() {
if (!window._currentRoomId) return;
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
  if (btn) { btn.classList.add('vc-muted'); btn.textContent = '📹✓'; }
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
if (btn) { btn.classList.remove('vc-muted'); btn.textContent = '📹'; }
removeVideoElement('vr-self');
updateStudioLayout();
updateStudioPreviewPanel();
updateVideoPanel();
  }

  window.toggleVoiceRoomScreenShare = async function() {
if (!window._currentRoomId) return;
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
  if (btn) { btn.classList.add('vc-muted'); btn.textContent = '🖥️✓'; }
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
if (btn) { btn.classList.remove('vc-muted'); btn.textContent = '🖥️'; }
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

  // --- Video Panel Helpers ---
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
streamChatOverlayEnabled = !streamChatOverlayEnabled;
localStorage.setItem('humanity-stream-chat-overlay', streamChatOverlayEnabled ? 'true' : 'false');
ensureStreamChatOverlay();
updateStudioPreviewPanel();
  };

  window.selectStreamChatChannel = function() {
const ch = prompt('Enter channel id/name for stream chat overlay:', streamChatOverlayChannel || 'general');
if (!ch) return;
streamChatOverlayChannel = ch.trim();
localStorage.setItem('humanity-stream-chat-channel', streamChatOverlayChannel);
ensureStreamChatOverlay();
updateStudioPreviewPanel();
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
pipBtn.textContent = '📌';
pipBtn.title = 'Pin/Unpin stream';
pipBtn.onclick = () => {
  wrapper.classList.toggle('pinned-inapp');
  pipBtn.textContent = wrapper.classList.contains('pinned-inapp') ? '🗗' : '📌';
};
wrapper.appendChild(video);
wrapper.appendChild(label);
wrapper.appendChild(pipBtn);
panel.appendChild(wrapper);
video.play().catch(() => {});

const hidden = !autoWatchStreams;
if (hidden) wrapper.style.display = 'none';
activeStreams.set(id, { name: name || id, wrapper, video, hidden });
renderStreamSidebar();
updateVideoPanel();
  }

  function removeVideoElement(id) {
const el = document.querySelector(`#video-panel .video-wrapper[data-id="${id}"]`);
if (el) el.remove();
activeStreams.delete(id);
renderStreamSidebar();
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

  // --- Picture-in-Picture ---
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

  // --- Camera Selection ---
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

  // ── Phase 3: Connection Quality Stats ──
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

  // ── Phase 4: Web Push Notifications (SW-based) ──
  function sendSWNotification(title, body, tag, url) {
if (!document.hidden) return; // Only notify when tab is backgrounded
if (!('serviceWorker' in navigator) || !navigator.serviceWorker.controller) return;
// Request permission if needed
if (Notification.permission === 'default') {
  Notification.requestPermission();
  return;
}
if (Notification.permission !== 'granted') return;
navigator.serviceWorker.controller.postMessage({
  type: 'notification',
  title: title,
  body: body,
  tag: tag || 'humanity',
  url: url || '/chat'
});
  }

  // Request notification permission on first interaction
  document.addEventListener('click', function requestNotifPerm() {
if ('Notification' in window && Notification.permission === 'default') {
  Notification.requestPermission();
}
document.removeEventListener('click', requestNotifPerm);
  }, { once: true });

  // Patch handleMessage to send notifications for DMs and calls
  const _origHandleMessage4 = handleMessage;
  handleMessage = function(msg) {
// Notification for incoming DM
if (msg.type === 'private' && msg.from !== myKey && document.hidden) {
  const senderName = resolveSenderName(msg.from_name, msg.from);
  sendSWNotification('DM from ' + senderName, msg.content || 'New message', 'dm-' + msg.from, '/chat');
}
// Notification for incoming call
if (msg.type === 'voice_call' && msg.action === 'ring' && document.hidden) {
  const callerName = resolveSenderName(msg.from_name, msg.from);
  sendSWNotification('Incoming call from ' + callerName, 'Tap to answer', 'call-' + msg.from, '/chat');
}
_origHandleMessage4(msg);
  };
