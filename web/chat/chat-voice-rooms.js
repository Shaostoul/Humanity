// ── chat-voice-rooms.js ───────────────────────────────────────────────────
// Voice room management: creation, joining, leaving, room UI, participant
// list, voice control bar, speaking indicators, channel cog menus,
// unified right-sidebar presence rendering.
//
// Shared state exposed as window globals for other voice modules:
//   window._voiceChannels, window._roomPeerConnections,
//   window._roomLocalStream, window._currentRoomId, rtcConfig
//
// Depends on: app.js globals (ws, myKey, myName, peerData, esc,
//   addSystemMessage, openDmConversation, isFriend, shortKey, hosIcon,
//   resolveSenderName, renderServerList, addNotice, beginChannelAdminCmd,
//   failChannelAdminCmd, sendChatCommand, myGroups, groupMembersByGroup,
//   activeGroupId, dmConversations)
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
  // Hide peer video viewer in right sidebar
  if (typeof hidePeerStreamViewer === 'function') hidePeerStreamViewer();
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
    // Auto-rejoin voice room after a deploy-triggered reload.
    const rejoinId = sessionStorage.getItem('_rejoin_room');
    if (rejoinId && !window._currentRoomId) {
      const target = window._voiceChannels.find(c => String(c.id) === String(rejoinId));
      if (target) {
        sessionStorage.removeItem('_rejoin_room');
        console.log('Auto-rejoining voice room after reload:', rejoinId);
        setTimeout(() => joinVoiceRoom(rejoinId), 500); // brief delay so WS is fully settled
      } else {
        // Room no longer exists — clear the pending rejoin
        sessionStorage.removeItem('_rejoin_room');
      }
    }
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
    // Ensure our mic stream exists before answering — if not set up yet the
    // RTCPeerConnection will have no audio tracks and createAnswer() produces
    // a=recvonly SDP, meaning the remote peer never receives our audio.
    if (!window._roomLocalStream) await setupRoomAudio();
    if (!window._roomLocalStream) return; // mic denied — can't send audio
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
    .vr-btn { font-size:0.7rem; padding:var(--space-xs) var(--space-md); cursor:pointer; border-radius:var(--radius-sm); border:1px solid var(--border); background:var(--bg-input); color:var(--text-primary); }
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
    btn.innerHTML = vcMuted ? hosIcon('mic', 16) : hosIcon('mic', 16);
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
          label: hosIcon('mic', 14) + ` PTT [${pttKeyLabel()}]`,
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
      modeBtn.innerHTML = mi.label;
      modeBtn.setAttribute('data-tip-title', 'Input Mode — ' + mi.label.replace(/<svg[^>]*>.*?<\/svg>\s*/g, '').replace(/[🗣️🎙️]\s*/, ''));
      modeBtn.setAttribute('data-tip-desc', mi.desc);
      modeBtn.setAttribute('data-tip-detail', mi.detail);
    }
    if (sqBtn) {
      const gateOn = vcSquelch;
      sqBtn.innerHTML = gateOn ? hosIcon('block', 14) + ' Gate On' : hosIcon('block', 14) + ' Gate Off';
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
      presetBtn.innerHTML = info.label;
      presetBtn.setAttribute('data-tip-title', 'Mic Preset — ' + info.label.replace(/<svg[^>]*>.*?<\/svg>\s*/g, '').replace('🎚️ ', ''));
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
      document.getElementById('vc-bar-channel-name').innerHTML = hosIcon('speaker', 16) + ' Connected to: ' + name;
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
      if (btn) { btn.innerHTML = hosIcon('mic', 16); btn.classList.remove('vc-muted'); }
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
        // If this track is video, show it in the right sidebar peer viewer
        if (event.track && event.track.kind === 'video') {
          showPeerStreamViewer(peerKey, peerName || shortKey(peerKey), event.streams[0]);
        }
      };
    }
  };

  /** Show a peer's video feed in the right-sidebar viewer. */
  function showPeerStreamViewer(peerKey, peerName, stream) {
    const viewer = document.getElementById('peer-stream-viewer');
    const nameEl = document.getElementById('peer-stream-name');
    const videoEl = document.getElementById('peer-stream-video');
    if (!viewer || !videoEl) return;
    nameEl.textContent = peerName || shortKey(peerKey);
    videoEl.srcObject = stream;
    viewer.style.display = '';
    videoEl.play().catch(() => {});
  }

  /** Hide the peer stream viewer (called when the peer's video track ends or they leave). */
  function hidePeerStreamViewer() {
    const viewer = document.getElementById('peer-stream-viewer');
    const videoEl = document.getElementById('peer-stream-video');
    if (!viewer) return;
    viewer.style.display = 'none';
    if (videoEl) videoEl.srcObject = null;
  }
  window.hidePeerStreamViewer = hidePeerStreamViewer;

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

// ── Unified Right Sidebar Presence Rendering ──

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
  if (!window.activeStreams || !window.activeStreams.has(id)) return;
  const s = window.activeStreams.get(id);
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
  const active = window.activeStreams || new Map();

  // Map: publicKey → voiceChannelId (only for users actually in a VC participant list)
  const voiceMap = new Map();
  (window._voiceChannels || []).forEach(vc => {
    (vc.participants || []).forEach(p => {
      const pk = p.public_key || p.key;
      if (pk) voiceMap.set(pk, vc.id);
    });
  });

  // Map: publicKey → streamId (only actual peer streams)
  const streamMap = new Map();
  active.forEach((s, id) => { if (s.peerKey) streamMap.set(s.peerKey, id); });

  // Collapse state for top-level sections
  const colKey = 'humanity-unified-right-collapsed';
  let colState = {};
  try { colState = JSON.parse(localStorage.getItem(colKey) || '{}') || {}; } catch (_) {}

  // Build a single icon-row HTML for a user. isFriend controls whether chat icon shows.
  function userRow(u, showCall) {
    const pk = u.public_key;
    const name = u.name || shortKey(pk);
    const dot = u.online
      ? '<span class="status-dot online" title="Online"></span>'
      : '<span class="status-dot offline" title="Offline"></span>';
    let badges = '';
    if (showCall) {
      badges += `<button class="ulist-icon" onclick="openDmConversation('${esc(pk)}','${esc(name)}')" title="Message ${esc(name)}">${hosIcon('chat', 16)}</button>`;
    }
    if (voiceMap.has(pk)) {
      const vcId = voiceMap.get(pk);
      const vc = (window._voiceChannels || []).find(c => String(c.id) === String(vcId));
      const vcName = vc ? esc(vc.name) : 'Voice';
      badges += `<button class="ulist-icon" onclick="joinVoiceRoom(${vcId})" title="Join ${vcName}">${hosIcon('mic', 16)}</button>`;
    }
    if (streamMap.has(pk)) {
      const sid = streamMap.get(pk);
      const s = active.get(sid);
      badges += `<button class="ulist-icon" onclick="toggleStreamVisibilityById('${esc(sid)}')" title="${s && !s.hidden ? 'Hide stream' : 'Watch stream'}">📺</button>`;
    }
    // Role + streaming badges
    const role = typeof roleBadge === 'function' ? roleBadge(u.role) : '';
    const live = typeof streamingBadge === 'function' ? streamingBadge(u.streaming_live) : '';
    // Follow indicator
    const followed = typeof isFollowing === 'function' && isFollowing(pk) ? '<span class="role-badge" style="background:#555;color:#ccc" title="Following">F</span>' : '';
    return `<div class="unified-row peer" data-username="${esc(name)}" data-pubkey="${esc(pk)}">${dot}<span class="peer-name">${esc(name)}</span>${role}${live}${followed}${badges}</div>`;
  }

  // Build a collapsible section with a flat alphabetical user list
  function section(id, title, userList, showCall) {
    const c = !!colState[id];
    const sorted = [...userList].sort((a, b) => (a.name || '').localeCompare(b.name || ''));
    const rows = sorted.map(u => userRow(u, showCall)).join('') ||
      '<div class="stream-empty" style="padding:2px 4px;color:#555;font-size:0.78rem;">None</div>';
    const count = userList.length ? ` <span class="unified-subsummary">(${userList.length})</span>` : '';
    return `<div class="unified-section${c ? ' collapsed' : ''}" data-usid="${esc(id)}">
      <button class="unified-header" onclick="toggleUnifiedSection('${esc(id)}')">${esc(title)}${count} ${c ? '▸' : '▾'}</button>
      <div class="unified-body">${rows}</div>
    </div>`;
  }

  const sections = [];
  const friendKeys = new Set();

  // ── Friends (mutual follows) — shown once, never repeated below ──────────
  const friendUsers = users.filter(u => u.public_key !== myKey && isFriend(u.public_key));
  friendUsers.forEach(u => friendKeys.add(u.public_key));
  sections.push(section('friends', 'Friends', friendUsers, true));

  // ── Groups — exclude friends and self to avoid duplicates ────────────────
  const groups = myGroups || [];
  if (groups.length === 0) {
    sections.push(section('group-none', 'Groups', [], false));
  } else {
    groups.forEach(g => {
      const members = (groupMembersByGroup[g.id] || [])
        .map(m => byKey.get(m.key) || { public_key: m.key, name: shortKey(m.key), online: false })
        .filter(u => u.public_key !== myKey && !friendKeys.has(u.public_key));
      sections.push(section('group-' + g.id, `${esc(g.name)}`, members, false));
    });
  }

  // ── Server — exclude self, friends, and group members already shown ──────
  const shownKeys = new Set([...friendKeys, myKey]);
  groups.forEach(g => {
    (groupMembersByGroup[g.id] || []).forEach(m => shownKeys.add(m.key));
  });
  const serverUsers = users.filter(u => !shownKeys.has(u.public_key));
  sections.push(section('server-main', 'United-Humanity', serverUsers, false));

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
      peerList.innerHTML = '<div style="font-size:0.75rem;color:var(--text-muted);padding:var(--space-sm);">No friends yet. Mutual follow is required for DMs.</div>';
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const rows = friendRows.map(u => {
      const online = !!u.online;
      const dot = online ? hosIcon('dot-green', 10) : '⚫';
      const name = esc(u.name || shortKey(u.public_key));
      const dmConv = (dmConversations || []).find(c => c.partner_key === u.public_key);
      const unread = dmConv && dmConv.unread_count ? ` <span style="color:var(--accent);font-size:0.68rem;">(${dmConv.unread_count})</span>` : '';
      return `<div class="peer" data-pubkey="${esc(u.public_key)}" style="opacity:${online ? '1' : '0.65'}">${dot} ${name}${unread}</div>`;
    }).join('');
    peerList.innerHTML = `<div style="font-size:0.62rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:var(--space-sm);">Friends</div>${rows}`;
    if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
    return;
  }

  if (tab === 'groups') {
    if (usersHeader) usersHeader.textContent = 'Group Members';
    if (!activeGroupId) {
      peerList.innerHTML = '<div style="font-size:0.75rem;color:var(--text-muted);padding:var(--space-sm);">Open a group to view members.</div>';
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const g = (myGroups || []).find(x => x.id === activeGroupId);
    const gName = g ? esc(g.name) : 'Current Group';
    const members = groupMembersByGroup[activeGroupId] || [];
    const byKey = new Map((allUsersSnapshot || []).map(u => [u.public_key, u]));
    if (members.length === 0) {
      peerList.innerHTML = `<div style="font-size:0.75rem;color:var(--text-muted);padding:var(--space-sm);">Loading members for <b>${gName}</b>...</div>`;
      if (typeof applyCachedQualityBadges === 'function') setTimeout(applyCachedQualityBadges, 0);
      return;
    }
    const rows = members.map(m => {
      const u = byKey.get(m.key);
      const online = !!(u && u.online);
      const dot = online ? hosIcon('dot-green', 10) : '⚫';
      const name = esc((u && u.name) ? u.name : shortKey(m.key));
      const role = m.role ? ` <span style="font-size:0.64rem;color:var(--text-muted);">(${esc(m.role)})</span>` : '';
      return `<div class="peer" data-pubkey="${esc(m.key)}" style="opacity:${online ? '1' : '0.65'}">${dot} ${name}${role}</div>`;
    }).join('');
    peerList.innerHTML = `<div style="font-size:0.62rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:var(--space-sm);">${gName} (${members.length})</div>${rows}`;
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
    btn.innerHTML = hosIcon('phone-call', 16);
    btn.title = `Call ${name}`;
    btn.onclick = (e) => {
      e.stopPropagation();
      startCall(pk, name);
    };
    el.appendChild(btn);
    if (window.twemoji) twemoji.parse(btn);
  });
}

// Add call buttons to user list and re-render presence sidebar
const _origUpdateUserList = updateUserList;
updateUserList = function(users) {
  allUsersSnapshot = Array.isArray(users) ? users : [];
  _origUpdateUserList(users);
  addCallButtonsToPeerList();
  renderPresenceSidebarForActiveContext();
  if (typeof renderStreamSidebar === 'function') renderStreamSidebar();
};
