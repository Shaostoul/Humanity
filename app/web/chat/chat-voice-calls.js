// ── chat-voice-calls.js ───────────────────────────────────────────────────
// 1-on-1 voice/video calls: call initiation, acceptance, rejection,
// call UI (ringing, in-call controls), call state management,
// WebSocket disconnect auto-hangup, web push notifications for calls/DMs.
//
// Depends on: chat-voice-rooms.js (requestCallCredentials, relayOnlyRtcConfig,
//   sayCallsNeedServer, getMicConstraints), app.js (ws, myKey,
//   addSystemMessage, esc, hosIcon, resolveSenderName, playNotificationChime,
//   openSocket)
//
// A call connects only through the server (step E, 2026-10-09,
// docs/design/blocking-and-safe-mode.md 10f): once it is accepted, each side
// asks the relay for `call_credentials` for this call and builds its peer
// connection from exactly the reply, relay only, so the other person sees the
// server's address and never yours. No credentials: the call ends and the call
// bar says why; there is no direct fallback.
// ─────────────────────────────────────────────────────────────────────────

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
// This call's credentials request: { peer, promise }. Asked once per call, by
// the callee as it accepts and by the caller as the accept arrives.
let callCredentials = null;

/** The credentials for the call with `peerKey` (chat-voice-rooms.js requestCallCredentials). */
function callCredentialsFor(peerKey) {
  if (!callCredentials || callCredentials.peer !== peerKey) {
    callCredentials = { peer: peerKey, promise: requestCallCredentials({ call: peerKey }) };
  }
  return callCredentials.promise;
}

/** The call cannot go through the server: end it (telling the other side) and say why. */
function callCannotConnect() {
  hangupCall();
  sayCallsNeedServer();
}

function startCall(targetKey, targetName) {
  if (callState !== 'idle') {
    addSystemMessage('You are already in a call or ringing.');
    return;
  }
  if (!ws || ws.readyState !== WebSocket.OPEN) return;

  callState = 'ringing-out';
  callPeerKey = targetKey;
  callPeerName = targetName;

  const ring = { type: 'voice_call', from: myKey, to: targetKey, action: 'ring' };
  // The callee's friendship pass, when we hold one (passes v2, chat-social.js).
  const pass = typeof friendPassFor === 'function' ? friendPassFor(targetKey) : null;
  if (pass) ring.friend_cert = pass;
  ws.send(JSON.stringify(ring));

  // Show ringing status
  document.getElementById('ringing-status').innerHTML = `${hosIcon('phone-call', 16)} Calling ${esc(targetName)}…`;
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
  // Ask for this call's credentials now (the relay opened the call at the ring),
  // so they are usually here by the time the caller's offer is.
  callCredentialsFor(callPeerKey);

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
  callCredentials = null;
  isMuted = false;
  if (callTimerInterval) { clearInterval(callTimerInterval); callTimerInterval = null; }
  callStartTime = null;
  document.getElementById('call-bar').classList.remove('active');
  document.getElementById('ringing-status').classList.remove('active');
  document.getElementById('incoming-call-overlay').classList.remove('open');
  const muteBtn = document.getElementById('mute-btn');
  muteBtn.classList.remove('muted');
  muteBtn.innerHTML = hosIcon('mic', 16) + ' Mute';
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
  btn.innerHTML = isMuted ? hosIcon('mic', 16) + ' Unmute' : hosIcon('mic', 16) + ' Mute';
}

async function setupPeerConnection(isCaller) {
  // Through the server only (step E): this call's credentials, or no connection.
  const peer = callPeerKey;
  const creds = await callCredentialsFor(peer);
  if (callState !== 'in-call' || callPeerKey !== peer) return false; // ended while waiting
  if (!creds) { callCannotConnect(); return false; }
  const pc = new RTCPeerConnection(relayOnlyRtcConfig(creds));
  peerConnection = pc;
  // Addresses the forwarder gave this connection. None by the end of gathering
  // means the server's call port did not answer, so no connection can form.
  let relayAddresses = 0;

  // Get microphone
  try {
    localStream = await navigator.mediaDevices.getUserMedia({ audio: getMicConstraints(), video: false });
  } catch (e) {
    addSystemMessage('⚠️ Microphone access denied. Cannot make voice call.');
    hangupCall();
    return false;
  }
  if (peerConnection !== pc) {
    // The call ended while the microphone was being asked for.
    localStream.getTracks().forEach(t => t.stop());
    localStream = null;
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
    if (event.candidate) relayAddresses++;
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

  peerConnection.onicegatheringstatechange = () => {
    if (pc.iceGatheringState === 'complete' && relayAddresses === 0 && peerConnection === pc) {
      callCannotConnect();
    }
  };

  peerConnection.onconnectionstatechange = () => {
    if (peerConnection && (peerConnection.connectionState === 'disconnected' || peerConnection.connectionState === 'failed')) {
      // Never a direct retry: the call is through the server or not at all.
      if (peerConnection.connectionState === 'failed' && relayAddresses === 0) {
        callCannotConnect();
        return;
      }
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

/** Is this caller someone I blocked (app.js isBlockedKey, step C)? */
function callerBlocked(key) {
  return typeof isBlockedKey === 'function' && isBlockedKey(key);
}

function handleVoiceCallMessage(msg) {
  // A ring from someone I blocked is ignored without a word: no screen, no
  // chime, and no "reject" back, which would tell them I am online (step C,
  // docs/design/blocking-and-safe-mode.md 4.7). It rings out on their side.
  if (msg.action === 'ring' && callerBlocked(msg.from)) return;
  const fromName = resolveSenderName(msg.from_name, msg.from);
  switch (msg.action) {
    case 'ring':
      if (callState !== 'idle') {
        // Busy: nothing goes back, and it rings out on the caller's side, the way a blocked
        // caller's ring does. An automatic "reject" used to go back, which told a caller who
        // cannot see us online (hidden status) that we were there after all (BUG-177,
        // 2026-10-10; the desktop app's engine/call_relay.rs `on_ring` does the same).
        addSystemMessage(`Missed call from ${fromName}: you were in another call or a voice room.`);
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
  // handleDCOffer answers only your own devices (mayAnswerDirectOffer): a direct
  // link to anyone else would show them your address (step E).
  if (msg.signal_type === 'dc_offer')  { handleDCOffer(msg);  return; }
  if (msg.signal_type === 'dc_answer') { handleDCAnswer(msg); return; }
  if (msg.signal_type === 'dc_ice')    { handleDCIce(msg);    return; }

  // Voice/video signals count only from the call peer, and only once the call
  // was accepted (2026-10-09). Before, they were taken from anyone who was just
  // RINGING you: a modified client could ring and then send an offer straight
  // away, and handleOffer turned the microphone on and answered while the
  // incoming-call screen was still asking, which gave the caller your network
  // address and, where the browser already had microphone permission, your
  // voice, without Accept ever being pressed. In both shipped clients (web and
  // native) the caller sends its offer only after the accept, when both sides
  // are already 'in-call', so a real call is unchanged.
  // Test: scripts/tests/p2p-direct-offers.test.js
  if (callState !== 'in-call' || msg.from !== callPeerKey) return;
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

// ── Web Push Notifications (SW-based) ──
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
  if (msg.type === 'voice_call' && msg.action === 'ring' && document.hidden && !callerBlocked(msg.from)) {
    const callerName = resolveSenderName(msg.from_name, msg.from);
    sendSWNotification('Incoming call from ' + callerName, 'Tap to answer', 'call-' + msg.from, '/chat');
  }
  _origHandleMessage4(msg);
};
