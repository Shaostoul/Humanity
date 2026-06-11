// ── chat-ui.js ────────────────────────────────────────────────────────────
// Notifications, sounds, help modal, user context menu, sidebar navigation,
// unread indicators, mobile UX, command palette, search.
// Depends on: app.js globals (ws, myKey, myName, activeChannel, peerData,
//   esc, switchChannel, openDmConversation, sendMessage)
// ─────────────────────────────────────────────────────────────────────────

// Channel property badges, mirror native's channel status icons
// (src/gui/pages/chat.rs paint_eye / paint_federation), muted color, drawn
// after the channel name. Eye = read-only, node-graph = federated.
const CH_BADGE_READONLY = '<span class="ch-badge" title="Read-only, only admins/mods can post"><svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" stroke-width="1.3"><path d="M1.6 8C3.1 5.1 5.3 3.8 8 3.8s4.9 1.3 6.4 4.2C12.9 10.9 10.7 12.2 8 12.2S3.1 10.9 1.6 8Z"/><circle cx="8" cy="8" r="1.8" fill="currentColor" stroke="none"/></svg></span>';
const CH_BADGE_FEDERATED = '<span class="ch-badge" title="Federated channel"><svg viewBox="0 0 16 16" width="11" height="11" fill="currentColor"><g stroke="currentColor" stroke-width="1"><line x1="8" y1="8" x2="8" y2="2.8"/><line x1="8" y1="8" x2="3.5" y2="10.6"/><line x1="8" y1="8" x2="12.5" y2="10.6"/></g><circle cx="8" cy="8" r="1.9"/><circle cx="8" cy="2.8" r="1.4"/><circle cx="3.5" cy="10.6" r="1.4"/><circle cx="12.5" cy="10.6" r="1.4"/></svg></span>';
// Mic glyph, native shows a clickable mic at the START of a voice-enabled
// channel row (left of the #), src/gui/pages/chat.rs ~1378 paint_mic. Click =
// join voice for that channel; click again = leave (sends voice_join/voice_leave
// with the channel name, exactly like native). The standalone "Voice Channels"
// section was removed in favour of this per-channel mic.
const MIC_SVG = '<svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" stroke-width="1.2"><rect x="6" y="1.8" width="4" height="7.2" rx="2" fill="currentColor" stroke="none"/><path d="M4.3 7.4a3.7 3.7 0 007.4 0"/><line x1="8" y1="11.1" x2="8" y2="13.6"/><line x1="5.8" y1="13.6" x2="10.2" y2="13.6"/></svg>';

// Section-header action icons (mirror native draw_*_section header buttons):
// DMs → cog (settings), Groups → plus (create) + arrow (join), Servers → plus
// (add server). Muted; brighten on hover via .uh-act-btn CSS.
const UH_ICON_COG = '<svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor"><path d="M8 5.2a2.8 2.8 0 100 5.6 2.8 2.8 0 000-5.6zm0 4.1a1.3 1.3 0 110-2.6 1.3 1.3 0 010 2.6z"/><path d="M13.5 8c0-.3 0-.6-.05-.9l1.3-1-1.3-2.2-1.5.6a4.6 4.6 0 00-1.5-.9L10.2 1.5h-2.6L7.4 3.1a4.6 4.6 0 00-1.5.9l-1.5-.6-1.3 2.2 1.3 1c-.05.3-.05.6-.05.9s0 .6.05.9l-1.3 1 1.3 2.2 1.5-.6c.45.4.95.7 1.5.9l.2 1.6h2.6l.25-1.6c.55-.2 1.05-.5 1.5-.9l1.5.6 1.3-2.2-1.3-1c.05-.3.05-.6.05-.9z" opacity="0.55"/></svg>';
const UH_ICON_PLUS = '<svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><line x1="8" y1="3.5" x2="8" y2="12.5"/><line x1="3.5" y1="8" x2="12.5" y2="8"/></svg>';
const UH_ICON_ARROW = '<svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><line x1="3" y1="8" x2="12" y2="8"/><polyline points="8.5,4.5 12.5,8 8.5,11.5"/></svg>';

// Track which channels we've joined voice on (client-side, mirrors native's
// ch.voice_joined). Sending voice_join/voice_leave matches native's wire format
// exactly; the relay does not yet bridge per-channel audio, that's the WebRTC
// transport track, so today this is the join/leave signal + visual state.
window._voiceJoinedChannels = window._voiceJoinedChannels || new Set();
function toggleChannelVoice(channel) {
  if (!channel) return;
  const joined = window._voiceJoinedChannels.has(channel);
  if (joined) window._voiceJoinedChannels.delete(channel);
  else window._voiceJoinedChannels.add(channel);
  if (typeof ws !== 'undefined' && ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: joined ? 'voice_leave' : 'voice_join', channel }));
  }
  if (typeof renderServerList === 'function') renderServerList();
}

// DM settings (the DMs-section cog). Native opens a small popup whose only live
// control is a DM-notifications toggle; mirror that intent as a mute toggle.
window.openDmSettings = function() {
  const key = 'hos_dm_notifications_muted';
  const muted = localStorage.getItem(key) === '1';
  localStorage.setItem(key, muted ? '0' : '1');
  const msg = muted ? '🔔 DM notifications unmuted' : '🔕 DM notifications muted';
  if (typeof addNotice === 'function') addNotice(msg, muted ? 'green' : 'orange', 4);
  else if (typeof addSystemMessage === 'function') addSystemMessage(msg);
};

// ── Key Bindings ──
document.getElementById('name-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') connect();
});

// Enter to send is handled on #msg-input directly (see below).

// ── Notifications ──
let windowFocused = document.hasFocus();
let unreadCount = 0;
const originalTitle = document.title;
let titleFlashInterval = null;

function startTitleFlash() {
  if (titleFlashInterval) return;
  titleFlashInterval = setInterval(() => {
    document.title = document.title === originalTitle
      ? `(${unreadCount}) New Messages`
      : originalTitle;
  }, 2000);
}

function stopTitleFlash() {
  if (titleFlashInterval) {
    clearInterval(titleFlashInterval);
    titleFlashInterval = null;
  }
  document.title = originalTitle;
}

window.addEventListener('focus', () => {
  windowFocused = true;
  unreadCount = 0;
  stopTitleFlash();
});

window.addEventListener('blur', () => {
  windowFocused = false;
});

document.addEventListener('visibilitychange', () => {
  if (!document.hidden) {
    windowFocused = true;
    unreadCount = 0;
    stopTitleFlash();
  } else {
    windowFocused = false;
  }
});

/** Check if a message content mentions the current user. */
function isMentioned(content) {
  if (!myName) return false;
  const pattern = new RegExp('@' + myName.replace(/[-_]/g, '[-_]'), 'i');
  return pattern.test(content);
}

// ── Notification Sounds ──
let audioCtx = null;
let soundEnabled = localStorage.getItem('humanity_sound_enabled') !== 'false';
let selectedSound = localStorage.getItem('humanity_sound') || 'chime';

// Loaded from /data/sounds/presets.json at startup. Empty until fetch resolves;
// playNotificationChime() returns silently if the preset isn't loaded yet.
let SOUND_PRESETS = {};
fetch('/data/sounds/presets.json', { cache: 'no-cache' })
  .then(function(r) { return r.ok ? r.json() : null; })
  .then(function(j) { if (j && j.presets) SOUND_PRESETS = j.presets; })
  .catch(function() { /* silent, sounds just won't play */ });

function playNotificationChime() {
  if (!soundEnabled) return;
  try {
    if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    const now = audioCtx.currentTime;
    const preset = SOUND_PRESETS[selectedSound] || SOUND_PRESETS.chime;
    preset.freqs.forEach(([freq, offset]) => {
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.type = preset.type;
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(preset.vol, now + offset);
      gain.gain.exponentialRampToValueAtTime(0.001, now + offset + preset.decay);
      osc.connect(gain);
      gain.connect(audioCtx.destination);
      osc.start(now + offset);
      osc.stop(now + offset + preset.decay);
    });
  } catch (e) { /* Audio not available */ }
}

function toggleSoundMenu() {
  const menu = document.getElementById('sound-menu');
  if (menu.style.display === 'none') {
    renderSoundOptions();
    menu.style.display = 'block';
    // Close on outside click.
    setTimeout(() => document.addEventListener('click', closeSoundMenuOutside), 0);
  } else {
    menu.style.display = 'none';
  }
}
function closeSoundMenuOutside(e) {
  const menu = document.getElementById('sound-menu');
  if (!menu.contains(e.target) && e.target.id !== 'sound-toggle') {
    menu.style.display = 'none';
    document.removeEventListener('click', closeSoundMenuOutside);
  }
}

// ── Account & Identity menu (header popover) ──
// Native parity: the left rail has NO persistent identity header. Profile,
// public key, contact-card share/add, peer sync, key protection, system info,
// and devices live OFF the channel list. Web mirrors this by relocating the
// #my-identity block into this header popover (see relocateIdentityToMenu),
// toggled by #account-toggle. Same open/close + outside-click pattern as the
// sound menu. Must be global (called from inline onclick).
function toggleIdentityMenu() {
  const menu = document.getElementById('identity-menu');
  if (!menu) return;
  if (menu.style.display === 'none' || !menu.style.display) {
    menu.style.display = 'block';
    setTimeout(() => document.addEventListener('click', closeIdentityMenuOutside), 0);
  } else {
    menu.style.display = 'none';
  }
}
function closeIdentityMenuOutside(e) {
  const menu = document.getElementById('identity-menu');
  if (!menu) return;
  // Keep open when the click is inside the menu or on its toggle button.
  if (!menu.contains(e.target) && !e.target.closest('#account-toggle')) {
    menu.style.display = 'none';
    document.removeEventListener('click', closeIdentityMenuOutside);
  }
}
function renderSoundOptions() {
  const container = document.getElementById('sound-options');
  container.innerHTML = Object.entries(SOUND_PRESETS).map(([key, preset]) => {
    const checked = key === selectedSound ? 'checked' : '';
    return `<label style="font-size:0.8rem;color:var(--text);display:flex;align-items:center;gap:var(--space-md);cursor:pointer;padding:var(--space-xs) 0;">
      <input type="radio" name="sound-choice" value="${key}" ${checked} onchange="selectSound('${key}')" style="accent-color:var(--accent);">
      ${esc(preset.label)}
      <button onclick="event.preventDefault();previewSound('${key}')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;padding:0 var(--space-sm);">▶</button>
    </label>`;
  }).join('');
  document.getElementById('sound-enabled').checked = soundEnabled;
}
function selectSound(key) {
  selectedSound = key;
  localStorage.setItem('humanity_sound', key);
  playNotificationChime();
}
function previewSound(key) {
  const prev = selectedSound;
  selectedSound = key;
  playNotificationChime();
  selectedSound = prev;
}
function toggleSoundEnabled() {
  soundEnabled = document.getElementById('sound-enabled').checked;
  localStorage.setItem('humanity_sound_enabled', soundEnabled);
  // Update bell icon.
  document.getElementById('sound-toggle').textContent = soundEnabled ? '🔔' : '🔕';
}
// Set initial bell icon.
document.getElementById('sound-toggle').textContent = soundEnabled ? '🔔' : '🔕';

function notifyNewMessage(author, content, isDm) {
  const mentioned = isMentioned(content);

  if (!windowFocused) {
    unreadCount++;
    startTitleFlash();
  }

  // DM notifications can be muted via the DMs-section cog (DM settings).
  const dmMuted = isDm && localStorage.getItem('hos_dm_notifications_muted') === '1';

  // Always notify on @mention or DM, even if focused (unless DMs are muted).
  if (!dmMuted && (mentioned || isDm || !windowFocused)) {
    playNotificationChime();
  }

  // Browser notification (if permitted).
  if (!dmMuted && Notification.permission === 'granted' && (!windowFocused || mentioned || isDm)) {
    const prefix = isDm ? '💬 DM from ' : '';
    const n = new Notification(prefix + author, {
      body: content.substring(0, 100),
      icon: '/favicon.png',
      tag: isDm ? 'humanity-dm' : 'humanity-msg',
    });
    n.onclick = () => {
      window.focus();
      n.close();
    };
  }
}

// Request notification permission and subscribe to WebPush (once).
function requestNotifications() {
  if (!('Notification' in window)) return;
  if (Notification.permission === 'default' && !localStorage.getItem('humanity_notif_asked')) {
    Notification.requestPermission().then(function(result) {
      localStorage.setItem('humanity_notif_asked', '1');
      if (result === 'granted') subscribeToPush();
    });
  } else if (Notification.permission === 'granted' && !localStorage.getItem('hos_push_subscribed')) {
    subscribeToPush();
  }
}

// Subscribe to WebPush via the relay's VAPID key.
function subscribeToPush() {
  if (!('serviceWorker' in navigator) || !('PushManager' in window)) return;
  if (!myIdentity || !myIdentity.canSign) return;

  fetch('/api/vapid-public-key')
    .then(function(r) { return r.json(); })
    .then(function(data) {
      if (!data.key) return;
      // Convert base64url to Uint8Array for applicationServerKey.
      var raw = atob(data.key.replace(/-/g, '+').replace(/_/g, '/'));
      var arr = new Uint8Array(raw.length);
      for (var i = 0; i < raw.length; i++) arr[i] = raw.charCodeAt(i);

      return navigator.serviceWorker.ready.then(function(reg) {
        return reg.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: arr
        });
      });
    })
    .then(function(sub) {
      if (!sub) return;
      var keys = sub.toJSON().keys;
      // Sign the request so the server knows which user this subscription belongs to.
      var ts = Date.now();
      return pqSignChatMessage('push_subscribe', ts).then(function(sig) { // full-PQ: Dilithium3 over push_subscribe\nts
        return fetch('/api/push/subscribe', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            public_key: myIdentity.publicKeyHex,
            endpoint: sub.endpoint,
            p256dh: keys.p256dh,
            auth: keys.auth,
            timestamp: ts,
            sig: sig
          })
        });
      });
    })
    .then(function(resp) {
      if (resp && resp.ok) {
        localStorage.setItem('hos_push_subscribed', '1');
        console.log('Push subscription registered');
      }
    })
    .catch(function(e) {
      console.warn('Push subscribe failed:', e);
    });
}

// Hook into message rendering to trigger notifications and update last-seen.
const _origHandleMessage = handleMessage;
handleMessage = function(msg) {
  _origHandleMessage(msg);
  if (msg.type === 'chat') {
    // Update last-seen timestamp.
    localStorage.setItem('humanity_last_seen', String(msg.timestamp));
    // Notify if from someone else.
    if (msg.from !== myKey) {
      notifyNewMessage(resolveSenderName(msg.from_name, msg.from) || 'Someone', msg.content, false);
    }
  }
};

// ── Auto-resize textarea to fit content ──
function autoResizeTextarea(el) {
  el.style.height = 'auto';
  el.style.height = Math.min(el.scrollHeight, 150) + 'px';
}

// ── Enter to send + Shift+Enter for newline + typing indicator ──
document.getElementById('msg-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  } else {
    // Any other key → send typing indicator (throttled).
    sendTypingIndicator();
  }
});

// Auto-resize + character counter on input.
function getMaxMsgLength() {
  const myRole = (peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
  return myRole === 'admin' ? 10000 : 2000;
}
const MAX_MSG_LENGTH = 2000; // default, updated dynamically
document.getElementById('msg-input').addEventListener('input', (e) => {
  autoResizeTextarea(e.target);
  updateCharCounter(e.target.value.length);
});

function updateCharCounter(len) {
  const counter = document.getElementById('char-counter');
  const limit = getMaxMsgLength();
  // Show counter when within 200 chars of limit.
  if (len > limit - 200) {
    counter.style.display = 'block';
    counter.textContent = `${len} / ${limit}`;
    counter.className = len > limit ? 'over' : len > limit - 100 ? 'warn' : '';
  } else {
    counter.style.display = 'none';
  }
}

// ── Crypto status check ──
(async () => {
  const has = await supportsEd25519();
  const el = document.getElementById('crypto-status');
  if (has) {
    el.textContent = '✓ Ed25519 signatures enabled, messages will be cryptographically signed';
    el.style.color = 'var(--success)';
  } else {
    el.textContent = '⚠ Ed25519 not supported in this browser, messages will not be signed';
    el.style.color = 'var(--warning)';
  }
})();

// Fetch and display server-wide message/user stats in the header bar.
async function updateStats() {
  try {
    const resp = await fetch('/api/stats');
    const data = await resp.json();
    document.getElementById('stats').textContent =
      `${data.total_messages} msgs · ${data.connected_peers} online`;
  } catch (e) { /* ignore, stats are cosmetic */ }
}

// Update stats every 30s.
setInterval(updateStats, 30000);

// ── Help Modal ──
function toggleHelpModal() {
  document.getElementById('help-modal-overlay').classList.toggle('open');
}
function closeHelpModal(e) {
  // Only close if clicking the overlay background.
  if (e.target === document.getElementById('help-modal-overlay')) {
    document.getElementById('help-modal-overlay').classList.remove('open');
  }
}

// ── Role badge helper ──
function roleBadge(role) {
  switch (role) {
    case 'admin': return '<span class="role-badge role-admin" title="Admin">A</span>';
    case 'mod': return '<span class="role-badge role-mod" title="Moderator">M</span>';
    case 'verified': return '<span class="role-badge role-verified" title="Verified">V</span>';
    case 'donor': return '<span class="role-badge role-donor" title="Donor">D</span>';
    default: return '';
  }
}

/** Returns a LIVE badge if the user is currently streaming. */
function streamingBadge(isLive) {
  if (!isLive) return '';
  return '<span class="role-badge role-streaming" title="Streaming">LIVE</span>';
}

// ── User Context Menu ──
let ctxMenuTarget = null; // { name, publicKey }
const ctxMenu = document.getElementById('user-context-menu');

function showUserContextMenu(e, name, publicKey) {
  e.preventDefault();
  e.stopPropagation();
  ctxMenuTarget = { name, publicKey };

  // Role lookups
  const targetPeer = peerData[publicKey] || {};
  const targetRole = targetPeer.role || 'user';
  const myRole = (peerData[myKey] && peerData[myKey].role) || 'user';
  const amMod   = myRole === 'mod' || myRole === 'admin';
  const amAdmin  = myRole === 'admin';

  // Color-coded ctx-item: user=default, mod=green left border, admin=blue, danger=red
  const ci = (onclick, label, tier) => {
    const borderStyle = {
      mod:    'border-left:3px solid var(--success);padding-left:9px;',
      admin:  'border-left:3px solid #56b;padding-left:9px;',
      danger: 'border-left:3px solid var(--danger);padding-left:9px;color:#e88;',
    }[tier] || '';
    return '<div class="ctx-item" style="' + borderStyle + '" onclick="' + onclick + '">' + label + '</div>';
  };

  // Role badge for target user header
  const roleBadge = {
    admin: '<span style="font-size:0.68rem;background:#56b;color:#fff;padding:1px 5px;border-radius:var(--radius-sm);margin-left:4px;">ADMIN</span>',
    mod:   '<span style="font-size:0.68rem;background:var(--success);color:#fff;padding:1px 5px;border-radius:var(--radius-sm);margin-left:4px;">MOD</span>',
  }[targetRole] || '<span style="font-size:0.68rem;background:#444;color:var(--text-muted);padding:1px 5px;border-radius:var(--radius-sm);margin-left:4px;">USER</span>';

  const isBot = publicKey && publicKey.startsWith('bot_');
  let html = '';

  if (isBot) {
    html += '<div class="ctx-item" style="font-weight:bold;color:var(--accent);pointer-events:none">\uD83E\uDD16 ' + esc(name) + '</div>';
    html += '<div class="ctx-sep"></div>';
    html += ci("botCommand('status')", '\uD83D\uDCCA Status', 'user');
    html += ci("botCommand('summary')", "\uD83D\uDCDD Today's Summary", 'user');
    html += ci("botCommand('tasks')", '\uD83D\uDCCB Current Tasks', 'user');
    html += ci("botCommand('help')", '\u2753 Help', 'user');
  } else {
    // Header with name + role badge
    html += '<div class="ctx-item" style="font-weight:600;pointer-events:none;padding-bottom:var(--space-xs);">' + esc(name) + roleBadge + '</div>';
    html += '<div class="ctx-sep"></div>';
    html += ci("viewProfileFromCtx()", '\uD83D\uDC64 View Profile', 'user');
    html += ci("copyPublicKey()", '\uD83D\uDCCB Copy Key', 'user');
    if (name !== myName) {
      html += ci("dmFromCtx()", '\uD83D\uDCAC Direct Message', 'user');
      if (typeof myFollowing !== 'undefined' && myFollowing.has(publicKey)) {
        html += ci("followFromCtx(false)", '\u274C Unfollow', 'user');
      } else {
        html += ci("followFromCtx(true)", '\uD83D\uDC41\uFE0F Follow', 'user');
      }
      if (isBlocked(name)) {
        html += ci("unblockFromCtx()", '\u2705 Unblock', 'user');
      } else {
        html += ci("blockFromCtx()", '\uD83D\uDEAB Block', 'user');
      }
      html += ci("reportUser()", '\uD83D\uDEA9 Report', 'danger');
      if (amMod) {
        html += '<div class="ctx-sep"></div>';
        html += '<div class="ctx-item" style="font-size:0.68rem;color:var(--success);pointer-events:none;">\u2014 Mod Actions \u2014</div>';
        html += ci("ctxCommand('/kick')", '\uD83D\uDC62 Kick', 'mod');
        html += ci("ctxCommand('/mute')", '\uD83D\uDD07 Mute', 'mod');
        html += ci("ctxCommand('/ban')", '\uD83D\uDEB7 Ban', 'danger');
      }
      if (amAdmin) {
        html += '<div class="ctx-item" style="font-size:0.68rem;color:#56b;pointer-events:none;padding-top:var(--space-sm);">\u2014 Admin Actions \u2014</div>';
        html += ci("ctxCommand('/verify')", '\u2736 Verify', 'admin');
        html += ci("ctxCommand('/mod')", '\u2B06\uFE0F Promote to Mod', 'admin');
        html += ci("ctxCommand('/unmod')", '\u2B07\uFE0F Demote', 'admin');
        html += ci("ctxCommand('/unban')", '\uD83D\uDD13 Unban', 'admin');
      }
    }
  }

  ctxMenu.innerHTML = html;
  if (window.twemoji) twemoji.parse(ctxMenu);

  const menuH = amAdmin ? 380 : amMod ? 300 : 220;
  const x = Math.min(e.clientX, window.innerWidth - 180);
  const y = Math.min(e.clientY, window.innerHeight - menuH);
  ctxMenu.style.left = x + 'px';
  ctxMenu.style.top = y + 'px';
  ctxMenu.classList.add('open');
}

function hideContextMenu() {
  ctxMenu.classList.remove('open');
  ctxMenuTarget = null;
}

function copyPublicKey() {
  if (ctxMenuTarget && ctxMenuTarget.publicKey) {
    navigator.clipboard.writeText(ctxMenuTarget.publicKey).then(() => {
      addSystemMessage('Public key copied to clipboard.');
    }).catch(() => {
      addSystemMessage('Failed to copy key.');
    });
  }
  hideContextMenu();
}

function ctxCommand(cmd) {
  if (!ctxMenuTarget) { console.warn('[ctxCommand] ctxMenuTarget is null'); return; }
  if (!ws || ws.readyState !== WebSocket.OPEN) { console.warn('[ctxCommand] WebSocket not open'); return; }
  const msg = `${cmd} ${ctxMenuTarget.name}`;
  const timestamp = Date.now();
  ws.send(JSON.stringify({
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: msg,
    timestamp: timestamp,
    channel: activeChannel,
  }));
  hideContextMenu();
}

function botCommand(cmd) {
  if (!ctxMenuTarget || !ws || ws.readyState !== WebSocket.OPEN) return;
  const content = `@Heron /${cmd}`;
  const timestamp = Date.now();
  ws.send(JSON.stringify({
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: content,
    timestamp: timestamp,
    channel: activeChannel,
  }));
  hideContextMenu();
}

function reportUser() {
  if (!ctxMenuTarget || !ws || ws.readyState !== WebSocket.OPEN) return;
  const targetName = ctxMenuTarget.name;
  hideContextMenu();
  const reason = prompt(`Report ${targetName}?\nEnter a reason (optional):`);
  if (reason === null) return; // User cancelled
  const content = reason ? `/report ${targetName} ${reason}` : `/report ${targetName}`;
  const timestamp = Date.now();
  ws.send(JSON.stringify({
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: content,
    timestamp: timestamp,
    channel: activeChannel,
  }));
}

function blockFromCtx() {
  if (!ctxMenuTarget) return;
  const name = ctxMenuTarget.name;
  hideContextMenu();
  blockUser(name);
}

function unblockFromCtx() {
  if (!ctxMenuTarget) return;
  const name = ctxMenuTarget.name;
  hideContextMenu();
  unblockUser(name);
}

function followFromCtx(doFollow) {
  if (!ctxMenuTarget) return;
  const pk = ctxMenuTarget.publicKey;
  hideContextMenu();
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: doFollow ? 'follow' : 'unfollow', target_key: pk }));
  }
}

function dmFromCtx() {
  if (!ctxMenuTarget) return;
  const name = ctxMenuTarget.name;
  const pk = ctxMenuTarget.publicKey;
  hideContextMenu();
  // DM permission check
  const myRole = (window.myPeerRole || '').toLowerCase();
  if (myRole !== 'admin' && myRole !== 'mod' && !myKey.startsWith('bot_')) {
    if (myRole !== 'verified' && myRole !== 'donor') {
      addSystemMessage('🔒 Verify your account to send DMs.');
      return;
    }
    if (typeof isFriend === 'function' && !isFriend(pk)) {
      addSystemMessage('🔒 You must be friends with this user to DM them. Use /follow ' + name);
      return;
    }
  }
  openDmConversation(pk, name);
}

function viewProfileFromCtx() {
  if (!ctxMenuTarget) return;
  const name = ctxMenuTarget.name;
  const pk = ctxMenuTarget.publicKey;
  hideContextMenu();
  requestViewProfile(name, pk);
}

// Close context menu on click outside.
document.addEventListener('click', (e) => {
  if (!ctxMenu.contains(e.target)) hideContextMenu();
});

// Event delegation for peer list context menu clicks.
document.getElementById('peer-list').addEventListener('click', function(e) {
  const peerEl = e.target.closest('.peer[data-username]');
  if (peerEl) {
    showUserContextMenu(e, peerEl.dataset.username, peerEl.dataset.pubkey);
  }
});

// Store peer data (with roles) for context menu lookups.
peerData = peerData || {};

// Prevent the native browser/Tauri right-click menu inside the peer list so
// the custom action menu works on right-click too (left-click already works).
document.getElementById('peer-list').addEventListener('contextmenu', function(e) {
  const peerEl = e.target.closest('.peer[data-username]');
  if (peerEl) {
    e.preventDefault();
    showUserContextMenu(e, peerEl.dataset.username, peerEl.dataset.pubkey);
  }
});

// Profile system, block list -> see chat-profile.js

// ── Import file handler (login screen) ──
// Handles both plain JSON backups and passphrase-encrypted backups.
async function handleImportFile(event) {
  const file = event.target.files[0];
  if (!file) return;
  try {
    const text = await file.text();
    const jsonData = JSON.parse(text);

    if (jsonData.encrypted) {
      // Show passphrase modal for encrypted backups
      showPassphraseModal(jsonData);
    } else {
      const identity = await importIdentityFromJSON(jsonData);
      finishImport(identity);
    }
  } catch (e) {
    const errEl = document.getElementById('login-error');
    errEl.textContent = '❌ Import failed: ' + e.message;
    errEl.style.display = 'block';
  }
  // Reset file input so the same file can be re-selected
  event.target.value = '';
}

/** Complete import: update state and connect. */
function finishImport(identity) {
  document.getElementById('name-input').value = identity.name;
  myIdentity = identity;
  myKey = identity.publicKeyHex;
  myName = identity.name;
  addSystemMessage('✅ Identity imported successfully! Connecting...');
  connect();
}

/** Modal for entering passphrase to decrypt an encrypted backup. */
function showPassphraseModal(jsonData) {
  const overlay = document.createElement('div');
  overlay.id = 'passphrase-modal-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.8);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';
  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:12px;padding:1.5rem;width:100%;max-width:420px;color:#e0e0e0;font-family:'Segoe UI',system-ui,sans-serif">
      <h2 style="font-size:1rem;font-weight:700;color:var(--accent,#f80);margin:0 0 .5rem">🔒 Encrypted Backup</h2>
      <p style="font-size:.82rem;color:#888;line-height:1.5;margin:0 0 1rem">Enter the passphrase you used when creating this backup.</p>
      <div style="position:relative;margin-bottom:.75rem">
        <input id="pp-input" type="password" placeholder="Passphrase" autocomplete="current-password"
          style="width:100%;background:#111;border:1px solid #333;border-radius:6px;padding:.5rem .75rem;padding-right:2.5rem;color:#e0e0e0;font-size:.85rem;outline:none;box-sizing:border-box">
        <button id="pp-toggle" type="button" style="position:absolute;right:6px;top:50%;transform:translateY(-50%);background:none;border:none;color:#666;cursor:pointer;font-size:.75rem;padding:4px 6px" title="Show/hide passphrase">Show</button>
      </div>
      <div id="pp-msg" style="font-size:.75rem;min-height:1.2em;margin-bottom:.75rem"></div>
      <div style="display:flex;gap:.5rem;justify-content:flex-end">
        <button id="pp-cancel" style="background:none;border:1px solid #333;color:#888;border-radius:6px;padding:.4rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="pp-submit" style="background:var(--accent,#f80);color:#000;border:none;border-radius:6px;padding:.4rem 1rem;font-size:.82rem;font-weight:700;cursor:pointer">Decrypt & Import</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);

  const input = document.getElementById('pp-input');
  const toggle = document.getElementById('pp-toggle');
  const msg = document.getElementById('pp-msg');
  const submit = document.getElementById('pp-submit');

  // Show/hide passphrase toggle
  toggle.addEventListener('click', () => {
    const isPassword = input.type === 'password';
    input.type = isPassword ? 'text' : 'password';
    toggle.textContent = isPassword ? 'Hide' : 'Show';
  });

  // Cancel
  document.getElementById('pp-cancel').addEventListener('click', () => overlay.remove());
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  // Submit
  async function doSubmit() {
    const pass = input.value;
    if (!pass) { msg.innerHTML = '<span style="color:#e55">Enter your passphrase.</span>'; return; }
    submit.disabled = true; submit.textContent = 'Decrypting…';
    msg.innerHTML = '';
    try {
      const identity = await importIdentityBackup(jsonData, pass);
      overlay.remove();
      finishImport(identity);
    } catch (e) {
      msg.innerHTML = '<span style="color:#e55">' + (e.message || 'Decryption failed') + '</span>';
      submit.disabled = false; submit.textContent = 'Decrypt & Import';
    }
  }
  submit.addEventListener('click', doSubmit);
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') doSubmit(); });
  input.focus();
}

// ── Login-screen seed phrase recovery ──
// Shows a modal with a textarea for 24 words + a name input, then restores
// the identity and connects to the network without a page reload.
function openLoginSeedRecovery() {
  const overlay = document.createElement('div');
  overlay.id = 'login-seed-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';

  overlay.innerHTML = `
    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius-lg);padding:var(--space-2xl);width:100%;max-width:540px;font-family:'Segoe UI',system-ui,sans-serif;color:var(--text);max-height:90vh;overflow-y:auto">
      <h2 style="font-size:1rem;font-weight:700;color:var(--accent);margin:0 0 var(--space-sm)">🌱 Recover from Seed Phrase</h2>
      <p style="font-size:.78rem;color:var(--text-muted);line-height:1.5;margin:0 0 var(--space-xl)">
        Enter the 24-word recovery phrase and choose a display name to rejoin the network.
      </p>

      <div style="margin-bottom:var(--space-lg)">
        <label for="lsr-name" style="font-size:.78rem;color:var(--text-muted);display:block;margin-bottom:var(--space-sm)">Display name</label>
        <input id="lsr-name" type="text" placeholder="Choose a name" autocomplete="off" maxlength="24"
          style="width:100%;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md) var(--space-lg);color:var(--text);font-size:.85rem;outline:none;box-sizing:border-box">
      </div>

      <div style="margin-bottom:var(--space-lg)">
        <label for="lsr-words" style="font-size:.78rem;color:var(--text-muted);display:block;margin-bottom:var(--space-sm)">Recovery phrase (24 words)</label>
        <textarea id="lsr-words" rows="3" placeholder="word1 word2 word3 … word24" autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
          style="width:100%;background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md) var(--space-lg);color:var(--text);font-size:.85rem;font-family:'Courier New',monospace;resize:vertical;outline:none;box-sizing:border-box;line-height:1.6"></textarea>
        <div id="lsr-word-count" style="font-size:.7rem;color:var(--text-muted);margin:var(--space-sm) 0 0">0 / 24 words</div>
      </div>

      <div id="lsr-msg" style="font-size:.75rem;min-height:1.2em;margin-bottom:var(--space-lg)"></div>
      <div style="display:flex;gap:var(--space-lg);justify-content:flex-end">
        <button id="lsr-cancel"
          style="background:none;border:1px solid var(--border);color:var(--text-muted);border-radius:var(--radius);padding:var(--space-md) var(--space-xl);font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="lsr-submit"
          style="background:var(--accent);color:#000;border:none;border-radius:var(--radius);padding:var(--space-md) var(--space-xl);font-size:.82rem;font-weight:700;cursor:pointer">Recover & Connect</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);

  // Close on overlay click or Cancel
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('lsr-cancel').addEventListener('click', () => overlay.remove());

  // Word counter
  const ta = document.getElementById('lsr-words');
  const counter = document.getElementById('lsr-word-count');
  ta.addEventListener('input', () => {
    const count = ta.value.trim().split(/\s+/).filter(Boolean).length;
    counter.textContent = `${count} / 24 words`;
    counter.style.color = count === 24 ? 'var(--success)' : 'var(--text-muted)';
  });

  // Pre-fill name from login input if user already typed one
  const loginName = document.getElementById('name-input');
  if (loginName && loginName.value.trim()) {
    document.getElementById('lsr-name').value = loginName.value.trim();
  }

  // Submit handler
  const submitBtn = document.getElementById('lsr-submit');
  const msgEl = document.getElementById('lsr-msg');

  async function doRecover() {
    const name = document.getElementById('lsr-name').value.trim();
    const mnemonic = ta.value.trim().toLowerCase().replace(/\s+/g, ' ');
    const wordCount = mnemonic.split(' ').filter(Boolean).length;

    if (!name || !/^[A-Za-z0-9_-]{1,24}$/.test(name)) {
      msgEl.innerHTML = '<span style="color:var(--danger)">Enter a valid name (letters, numbers, underscores, dashes, max 24 chars).</span>';
      return;
    }
    if (wordCount !== 24) {
      msgEl.innerHTML = `<span style="color:var(--danger)">Expected 24 words, got ${wordCount}. Check for extra spaces or missing words.</span>`;
      return;
    }

    submitBtn.disabled = true;
    submitBtn.textContent = 'Recovering…';
    msgEl.innerHTML = '';

    try {
      // Validate checksum and restore identity
      await restoreIdentityFromMnemonic(mnemonic);

      // Set name and populate login input so connect() picks it up
      localStorage.setItem('humanity_name', name);
      document.getElementById('name-input').value = name;

      overlay.remove();
      connect();
    } catch (e) {
      const isChecksum = /checksum/i.test(e.message);
      msgEl.innerHTML = `<span style="color:var(--danger)">${isChecksum ? 'Invalid recovery phrase, check your words and try again.' : e.message}</span>`;
      submitBtn.disabled = false;
      submitBtn.textContent = 'Recover & Connect';
    }
  }

  submitBtn.addEventListener('click', doRecover);
  ta.addEventListener('keydown', e => { if (e.key === 'Enter' && e.ctrlKey) doRecover(); });
  document.getElementById('lsr-name').focus();
}

// Handle /profile, /block, /unblock, /blocklist commands.
// Patching into the existing sendMessage to intercept client-side commands.
const _origSendMessage2 = sendMessage;
sendMessage = async function() {
  const input = document.getElementById('msg-input');
  const val = input.value.trim();
  if (val === '/profile') {
    input.value = '';
    openEditProfileModal();
    return;
  }
  if (val === '/export') {
    input.value = '';
    downloadIdentityBackup(myName);
    return;
  }
  if (val === '/blocklist') {
    input.value = '';
    showBlockList();
    return;
  }
  if (val === '/dms') {
    input.value = '';
    // Request updated DM list from server.
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'chat', from: myKey, from_name: myName, content: '/dms', timestamp: Date.now(), channel: activeChannel }));
    }
    return;
  }
  if (val.startsWith('/block ') && !val.startsWith('/blocklist')) {
    const name = val.substring(7).trim();
    if (name) {
      input.value = '';
      blockUser(name);
      return;
    }
  }
  if (val.startsWith('/unblock ')) {
    const name = val.substring(9).trim();
    if (name) {
      input.value = '';
      unblockUser(name);
      return;
    }
  }
  if (val.startsWith('/follow ') && !val.startsWith('/follow-')) {
    const name = val.substring(8).trim();
    if (name && ws && ws.readyState === WebSocket.OPEN) {
      input.value = '';
      // Resolve name to key from peer list
      const targetKey = resolveNameToKey(name);
      if (targetKey) {
        ws.send(JSON.stringify({ type: 'follow', target_key: targetKey }));
      } else {
        addSystemMessage('User "' + name + '" not found in peer list.');
      }
      return;
    }
  }
  if (val.startsWith('/unfollow ')) {
    const name = val.substring(10).trim();
    if (name && ws && ws.readyState === WebSocket.OPEN) {
      input.value = '';
      const targetKey = resolveNameToKey(name);
      if (targetKey) {
        ws.send(JSON.stringify({ type: 'unfollow', target_key: targetKey }));
      } else {
        addSystemMessage('User "' + name + '" not found in peer list.');
      }
      return;
    }
  }
  if (val.startsWith('/group-create ')) {
    const name = val.substring(14).trim();
    if (name && ws && ws.readyState === WebSocket.OPEN) {
      input.value = '';
      ws.send(JSON.stringify({ type: 'group_create', name: name }));
      return;
    }
  }
  if (val.startsWith('/group-join ')) {
    const code = val.substring(12).trim();
    if (code && ws && ws.readyState === WebSocket.OPEN) {
      input.value = '';
      ws.send(JSON.stringify({ type: 'group_join', invite_code: code }));
      return;
    }
  }
  if (val.startsWith('/group-leave')) {
    input.value = '';
    if (activeGroupId && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'group_leave', group_id: activeGroupId }));
    } else {
      addSystemMessage('You are not viewing a group. Switch to a group first.');
    }
    return;
  }
  if (val.startsWith('/group-invite')) {
    input.value = '';
    if (activeGroupId) {
      const group = myGroups.find(g => g.id === activeGroupId);
      if (group) {
        navigator.clipboard.writeText(group.invite_code).then(() => {
          addSystemMessage('📋 Invite code copied: ' + group.invite_code + ', Share it with /group-join ' + group.invite_code);
        }).catch(() => {
          addSystemMessage('📋 Invite code: ' + group.invite_code + ', Share it with /group-join ' + group.invite_code);
        });
      }
    } else {
      addSystemMessage('Switch to a group first to get its invite code.');
    }
    return;
  }
  // If in group view, send as group message.
  if (activeGroupId) {
    if (val && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'group_msg',
        group_id: activeGroupId,
        content: val,
      }));
      input.value = '';
      input.style.height = 'auto';
    }
    return;
  }
  // If in DM view, send as DM instead of chat.
  if (activeDmPartner) {
    // Client-side DM permission pre-check
    const myRole = (window.myPeerRole || '').toLowerCase();
    if (myRole !== 'admin' && myRole !== 'mod' && !myKey.startsWith('bot_')) {
      if (myRole !== 'verified' && myRole !== 'donor') {
        addSystemMessage('🔒 Verify your account to send DMs.');
        return;
      }
      if (!isFriend(activeDmPartner)) {
        addSystemMessage('🔒 You must be friends to DM this user. Use /follow <name>, if they follow you back, you\'ll be friends.');
        return;
      }
    }
    if (val && ws && ws.readyState === WebSocket.OPEN) {
      // The relay only length-limits PLAINTEXT DMs (a PQ ciphertext blob
      // is opaque and ~9 KB even for a short note), so enforce the
      // user-visible limit here, before sealing.
      const DM_PLAINTEXT_MAX = 2000;
      if (val.length > DM_PLAINTEXT_MAX) {
        addSystemMessage(`Message too long (${val.length}/${DM_PLAINTEXT_MAX} chars). Please shorten it.`);
        return;
      }
      const peerKyber = getPeerEcdhPublic(activeDmPartner); // Kyber768 pub now
      let dmPayload = {
        type: 'dm',
        from: myKey,
        from_name: myName,
        to: activeDmPartner,
        content: val,
        timestamp: Date.now(),
      };
      // Full-PQ E2EE, FAIL CLOSED. A DM is only ever sent sealed. If the
      // recipient hasn't advertised a Kyber key yet, or our own PQ
      // identity isn't ready, ABORT, never transmit plaintext to the
      // relay. The relay is zero-knowledge; friendship is access control,
      // NOT confidentiality (the operator can read the DB). (Security
      // review HIGH-1: the old "graceful plaintext fallback" leaked DMs.)
      if (!peerKyber) {
        addSystemMessage("🔒 Can't send yet, this person hasn't come online with a current post-quantum client, so there's no key to encrypt to. Try again once they've reconnected.");
        return;
      }
      const enc = await encryptDmContent(val, peerKyber);
      if (!enc) {
        addSystemMessage("🔒 Your encryption identity isn't ready yet. Wait a moment and resend (reload the page if it persists).");
        return;
      }
      dmPayload.content = enc.content;
      dmPayload.nonce = enc.nonce;
      dmPayload.encrypted = true;
      ws.send(JSON.stringify(dmPayload));
      // Show locally immediately (plaintext) and keep DM list persistent.
      const sentTs = Date.now();
      addDmMessage(myName, val, sentTs, myKey, activeDmPartner, false);
      upsertDmConversation(activeDmPartner, activeDmPartnerName || (peerData[activeDmPartner]?.display_name || shortKey(activeDmPartner)), val, sentTs, false);
      input.value = '';
      input.style.height = 'auto';
    }
    return;
  }
  await _origSendMessage2();
};

// ── Sidebar Tab Navigation ──
// Federated servers cache (fetched from API).
var federatedServers = [];
var federatedServersFetched = false;

(function initSidebarTabs() {
  const SIDEBAR_TAB_KEY = 'humanity_sidebar_tab';
  const SERVER_ORDER_KEY = 'humanity_server_order';
  const SERVER_COLLAPSE_KEY = 'humanity_server_collapsed';

  // Tab click handler via event delegation, register FIRST before anything that might throw
  document.getElementById('sidebar-tabs').addEventListener('click', function(e) {
    const tab = e.target.closest('.sidebar-tab');
    if (!tab) return;
    const tabName = tab.getAttribute('data-tab');
    if (tabName) switchSidebarTab(tabName, true);
  });

  // Restore saved tab
  const savedTab = localStorage.getItem(SIDEBAR_TAB_KEY) || 'servers';
  try { switchSidebarTab(savedTab, false); } catch(e) { console.warn('Sidebar init error:', e); }

  function switchSidebarTab(tabName, save) {
    // Update tab buttons
    document.querySelectorAll('#sidebar-tabs .sidebar-tab').forEach(btn => {
      btn.classList.toggle('active', btn.getAttribute('data-tab') === tabName);
    });
    // Update tab content panels
    document.querySelectorAll('.sidebar-tab-content').forEach(panel => {
      panel.classList.toggle('active', panel.id === 'tab-' + tabName);
    });
    if (save) localStorage.setItem(SIDEBAR_TAB_KEY, tabName);
    // Render the active tab's content
    if (tabName === 'servers') renderServerList();
    if (tabName === 'dms') renderDmList();
    if (typeof renderPresenceSidebarForActiveContext === 'function') {
      renderPresenceSidebarForActiveContext();
    }
  }
  window.switchSidebarTab = switchSidebarTab;

  function initUnifiedLeftSidebar() {
    const tabs = document.getElementById('sidebar-tabs');
    const tabServers = document.getElementById('tab-servers');
    const tabGroups = document.getElementById('tab-groups');
    const tabDms = document.getElementById('tab-dms');
    const sidebar = document.getElementById('sidebar');
    if (!tabs || !tabServers || !tabGroups || !tabDms || !sidebar) return;

    tabs.style.display = 'none';

    let unified = document.getElementById('sidebar-unified-left');
    if (!unified) {
      unified = document.createElement('div');
      unified.id = 'sidebar-unified-left';
      unified.className = 'sidebar-unified-left';
      tabs.insertAdjacentElement('afterend', unified);
    }

    // Header = a flex row: a [collapse toggle button] + an optional [action
    // icons] span. Buttons are siblings (not nested) so the markup is valid and
    // each gets its own click. Mirrors native draw_*_section header buttons.
    const mkSection = (id, label, panel, actions) => {
      const wrap = document.createElement('div');
      wrap.className = 'unified-section';
      wrap.dataset.sid = id;
      const head = document.createElement('div');
      head.className = 'unified-header';
      const toggle = document.createElement('button');
      toggle.className = 'uh-toggle';
      toggle.type = 'button';
      toggle.dataset.baseLabel = label;
      toggle.textContent = label + ' ▾';
      toggle.onclick = () => {
        wrap.classList.toggle('collapsed');
        refreshUnifiedLeftHeaderCounts();
      };
      head.appendChild(toggle);
      if (actions && actions.length) {
        const act = document.createElement('span');
        act.className = 'uh-actions';
        for (const a of actions) {
          const b = document.createElement('button');
          b.className = 'uh-act-btn';
          b.type = 'button';
          b.title = a.title;
          b.innerHTML = a.icon;
          b.onclick = (e) => { e.stopPropagation(); try { a.onClick(); } catch (err) { console.error(err); } };
          act.appendChild(b);
        }
        head.appendChild(act);
      }
      const body = document.createElement('div');
      body.className = 'unified-body';
      body.appendChild(panel);
      panel.classList.add('force-show');
      wrap.appendChild(head);
      wrap.appendChild(body);
      return wrap;
    };

    // Native-parity section header actions:
    //   DMs    → cog (DM settings: toggle DM notification mute)
    //   Groups → + (create group), → (join by invite)
    //   Servers→ + (add server)
    const dmActions = [
      { title: 'DM settings', icon: UH_ICON_COG, onClick: () => window.openDmSettings && window.openDmSettings() },
    ];
    const groupActions = [
      { title: 'Create group', icon: UH_ICON_PLUS, onClick: () => window.promptCreateGroup && window.promptCreateGroup() },
      { title: 'Join group by invite', icon: UH_ICON_ARROW, onClick: () => window.promptJoinGroup && window.promptJoinGroup() },
    ];
    const serverActions = [
      { title: 'Add server', icon: UH_ICON_PLUS, onClick: () => window.promptAddServer && window.promptAddServer() },
    ];

    // Requested order: DMs (top), Groups (middle), Servers (bottom)
    if (!unified.querySelector('[data-sid="dms"]')) unified.appendChild(mkSection('dms', 'DMs', tabDms, dmActions));
    if (!unified.querySelector('[data-sid="groups"]')) unified.appendChild(mkSection('groups', 'Groups', tabGroups, groupActions));
    if (!unified.querySelector('[data-sid="servers"]')) unified.appendChild(mkSection('servers', 'Servers', tabServers, serverActions));

    // ── Scratchpad: standalone top row (native parity) ──
    // Native (`draw_left_panel`) renders a "# scratchpad" row at the very TOP
    // of the left rail, above DMs/Groups/Servers, for a local-only workspace
    // not attached to any server/group/DM. Web previously nested its
    // `__scratch__` channel INSIDE the Humanity server's channel list (so it
    // vanished when that server collapsed). Promote it to a peer top row here;
    // the in-server copy is removed in renderServerList to avoid duplication.
    let scratchRow = document.getElementById('unified-scratch-row');
    if (!scratchRow) {
      scratchRow = document.createElement('button');
      scratchRow.id = 'unified-scratch-row';
      scratchRow.type = 'button';
      scratchRow.className = 'unified-scratch-row';
      scratchRow.title = 'Local workspace. Nothing sent to anyone.';
      scratchRow.textContent = '# scratch-pad';
      scratchRow.onclick = function() {
        if (typeof switchChannel === 'function') switchChannel('__scratch__');
      };
      unified.insertBefore(scratchRow, unified.firstChild);
    }
    // Keep the row's active highlight in sync with the current context.
    function refreshScratchActive() {
      const isScratch = (typeof activeChannel !== 'undefined' && activeChannel === '__scratch__')
        && !(typeof activeDmPartner !== 'undefined' && activeDmPartner)
        && !(typeof activeGroupId !== 'undefined' && activeGroupId);
      scratchRow.classList.toggle('active', !!isScratch);
    }
    window.refreshScratchActive = refreshScratchActive;
    // Wrap switchChannel once so every context change refreshes the highlight
    // (channel-list re-render handles the in-list items; this top row is
    // outside that container, so it needs its own sync hook).
    if (!window.__scratchRowWrapped && typeof switchChannel === 'function') {
      const _origSwitchForScratch = switchChannel;
      switchChannel = function(id) {
        _origSwitchForScratch(id);
        try { refreshScratchActive(); } catch (e) {}
      };
      window.__scratchRowWrapped = true;
    }
    refreshScratchActive();

    function refreshUnifiedLeftHeaderCounts() {
      const serverCount = (channelList || []).length;
      const groupCount = (myGroups || []).length;
      const dmCount = (dmConversations || []).length;
      const mapping = {
        servers: { label: 'Servers', count: serverCount },
        groups: { label: 'Groups', count: groupCount },
        dms: { label: 'DMs', count: dmCount },
      };
      unified.querySelectorAll('.unified-section[data-sid]').forEach(sec => {
        const sid = sec.getAttribute('data-sid');
        // Update only the collapse-toggle's label, the action icons live in a
        // sibling .uh-actions span and must not be clobbered.
        const head = sec.querySelector('.unified-header .uh-toggle') || sec.querySelector('.unified-header');
        if (!head || !mapping[sid]) return;
        const collapsed = sec.classList.contains('collapsed');
        head.textContent = `${mapping[sid].label} (${mapping[sid].count}) ${collapsed ? '▸' : '▾'}`;
      });
    }
    window.refreshUnifiedLeftHeaderCounts = refreshUnifiedLeftHeaderCounts;

    // In unified mode, always render all sections; external tab switch calls expand relevant section only.
    window.switchSidebarTab = function(tabName, save) {
      if (tabName === 'servers') renderServerList();
      if (tabName === 'dms') renderDmList();
      if (typeof renderGroupsTab === 'function') renderGroupsTab();
      const sec = unified.querySelector(`[data-sid="${tabName}"]`);
      if (sec) sec.classList.remove('collapsed');
      if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
      if (typeof refreshUnifiedLeftHeaderCounts === 'function') refreshUnifiedLeftHeaderCounts();
      if (save) localStorage.setItem(SIDEBAR_TAB_KEY, tabName);
    };

    refreshUnifiedLeftHeaderCounts();
  }

  function initPanelResizers() {
    const sidebar = document.getElementById('sidebar');
    const right = document.getElementById('right-sidebar');
    const leftResizer = document.getElementById('left-resizer');
    const rightResizer = document.getElementById('right-resizer');
    const leftLockBtn = document.getElementById('left-lock-btn');
    const rightLockBtn = document.getElementById('right-lock-btn');
    if (!sidebar || !right || !leftResizer || !rightResizer) return;

    const lk = JSON.parse(localStorage.getItem('humanity-panel-locks') || '{"left":false,"right":false}');
    const widths = JSON.parse(localStorage.getItem('humanity-panel-widths') || '{}');
    if (widths.left) sidebar.style.width = `${Math.max(150, Math.min(420, widths.left))}px`;
    if (widths.right) right.style.width = `${Math.max(180, Math.min(460, widths.right))}px`;

    function applyLockUI() {
      if (leftLockBtn) {
        leftLockBtn.textContent = lk.left ? '🔒' : '🔓';
        leftLockBtn.classList.toggle('locked', !!lk.left);
      }
      if (rightLockBtn) {
        rightLockBtn.textContent = lk.right ? '🔒' : '🔓';
        rightLockBtn.classList.toggle('locked', !!lk.right);
      }
    }
    applyLockUI();

    window.togglePanelLock = function(side) {
      if (side !== 'left' && side !== 'right') return;
      lk[side] = !lk[side];
      localStorage.setItem('humanity-panel-locks', JSON.stringify(lk));
      applyLockUI();
    };

    function attachDrag(handle, side) {
      let dragging = false;
      handle.addEventListener('mousedown', (e) => {
        if (lk[side]) return;
        e.preventDefault();
        dragging = true;
      });
      window.addEventListener('mouseup', () => { dragging = false; });
      window.addEventListener('mousemove', (e) => {
        if (!dragging) return;
        if (side === 'left') {
          const w = Math.max(150, Math.min(420, e.clientX));
          sidebar.style.width = `${w}px`;
          widths.left = w;
        } else {
          const w = Math.max(180, Math.min(460, window.innerWidth - e.clientX));
          right.style.width = `${w}px`;
          widths.right = w;
        }
        localStorage.setItem('humanity-panel-widths', JSON.stringify(widths));
      });
    }

    attachDrag(leftResizer, 'left');
    attachDrag(rightResizer, 'right');
  }

  setTimeout(initUnifiedLeftSidebar, 0);
  setTimeout(initPanelResizers, 0);

  // Web-native parity (Track W): the native chat puts streaming/studio on
  // the RIGHT for streamers, not the left. The static markup still has the
  // studio panel inside the left #sidebar; relocate it at runtime to the
  // TOP of the right rail (above the people/friends list), matching the
  // parity design. Runtime move (vs HTML cut-paste) keeps it reversible
  // and preserves every studio control's id/handler, the element just
  // changes parent. Mirrors how initUnifiedLeftSidebar restructures the
  // left rail. See docs/design/web-native-parity.md + studio-streaming.md.
  function relocateStudioToRightRail() {
    const studio = document.getElementById('stream-studio-panel');
    const rightRail = document.getElementById('right-sidebar');
    if (!studio || !rightRail) return;
    // Already moved? (idempotent, init can run more than once.)
    if (studio.parentElement === rightRail) return;
    rightRail.insertBefore(studio, rightRail.firstChild);
  }
  setTimeout(relocateStudioToRightRail, 0);
  window.relocateStudioToRightRail = relocateStudioToRightRail;

  // Web-native parity (Track W): native's left rail starts clean (scratchpad →
  // DMs → Groups → Servers) with NO persistent identity header. Web's
  // #my-identity block sat at the top of the left #sidebar; relocate it at
  // runtime into the header #identity-menu popover (toggled by #account-toggle),
  // keeping every control's id/handler intact, same runtime-move approach as
  // relocateStudioToRightRail. Graceful: if this never runs, the block simply
  // stays in the sidebar. See docs/design/web-native-parity.md (parity step 1).
  function relocateIdentityToMenu() {
    const block = document.getElementById('my-identity');
    const menu = document.getElementById('identity-menu');
    if (!block || !menu) return;
    if (menu.contains(block)) return; // already moved (idempotent)
    block.style.marginBottom = '0'; // shed sidebar spacing inside the popover
    menu.appendChild(block);
  }
  setTimeout(relocateIdentityToMenu, 0);
  window.relocateIdentityToMenu = relocateIdentityToMenu;

  // ── Server List Rendering ──
  function getServerOrder() {
    try {
      const order = JSON.parse(localStorage.getItem(SERVER_ORDER_KEY));
      if (Array.isArray(order)) return order;
    } catch(_) {}
    return null;
  }

  function getCollapsedServers() {
    try {
      const c = JSON.parse(localStorage.getItem(SERVER_COLLAPSE_KEY));
      if (Array.isArray(c)) return new Set(c);
    } catch(_) {}
    return new Set();
  }

  function saveCollapsedServers(set) {
    localStorage.setItem(SERVER_COLLAPSE_KEY, JSON.stringify([...set]));
  }

  // (moved above initSidebarTabs)

  async function fetchFederatedServers() {
    try {
      const resp = await fetch('/api/federation/servers');
      if (resp.ok) {
        federatedServers = await resp.json();
        federatedServersFetched = true;
      }
    } catch (e) {
      console.warn('Failed to fetch federated servers:', e);
    }
  }

  function renderServerList() {
    const container = document.getElementById('server-list');
    if (!container) return;

    // Fetch federated servers if not yet loaded.
    if (!federatedServersFetched) {
      fetchFederatedServers().then(() => renderServerList());
    }

    // Current server (always first, highlighted).
    const collapsed = getCollapsedServers();
    const isCollapsed = collapsed.has('Humanity');
    const myRoleCh = (window.myPeerRole || '').toLowerCase();

    // Every channel renders as a normal row, mirrors native, where
    // #announcements is just a read-only channel, not a separate widget.
    // Property badges after the name match native's channel status icons
    // (src/gui/pages/chat.rs ~1027): eye = read-only, node-graph = federated,
    // both drawn in muted color.
    const channelsHtml = channelList.map(ch => {
      const isActive = ch.id === activeChannel && !activeDmPartner && !activeGroupId;
      const title = ch.description ? ` title="${esc(ch.description)}"` : '';
      const badges = (ch.read_only ? CH_BADGE_READONLY : '') + (ch.federated ? CH_BADGE_FEDERATED : '');
      // Mic sits LEFT of the # (native order: mic, cog, #name). Clickable -
      // toggles voice join/leave for the channel; joined = accent + filled.
      const voiceJoined = !!(window._voiceJoinedChannels && window._voiceJoinedChannels.has(ch.name));
      const micHtml = ch.voice_enabled
        ? `<span class="ch-mic${voiceJoined ? ' joined' : ''}" data-voice-channel="${esc(ch.name)}" title="${voiceJoined ? 'Leave voice' : 'Join voice'}">${MIC_SVG}</span>`
        : '';
      const cogHtml = (myRoleCh === 'admin' || myRoleCh === 'mod') ? `<span class="channel-cog" data-cog-type="text" data-cog-id="${esc(ch.id)}" data-cog-name="${esc(ch.name)}">⚙️</span>` : '';
      // .srv-chan suppresses the auto "# " ::before so the mic can sit before the
      // hash; the hash is rendered as part of the label instead.
      return `<div class="channel-item srv-chan${isActive ? ' active' : ''}"${title} data-channel-id="${esc(ch.id)}">${micHtml}${cogHtml}<span class="ch-label"># ${esc(ch.name)}</span>${badges}</div>`;
    }).join('');

    // Text channel create button (admin/mod only)
    let createChannelBtn = '';
    if (myRoleCh === 'admin' || myRoleCh === 'mod') {
      createChannelBtn = '<div style="padding:var(--space-xs) 0;"><button class="vr-btn" data-action="create-text-channel" style="width:100%;margin-top:var(--space-xs);font-size:0.7rem;">+ Create Channel</button></div>';
    }

    // Native parity: there is NO standalone "Voice Channels" section. Voice is
    // surfaced as a mic indicator on each voice-enabled text channel row above
    // (CH_ICON_VOICE), mirroring native's draw_servers_section. The old
    // window._voiceChannels rooms UI was removed here in v0.290.x.

    // Scratch-pad moved to a standalone top row in the unified left sidebar
    // (see initUnifiedLeftSidebar → #unified-scratch-row) to match native,
    // which renders it above DMs/Groups/Servers. No longer nested here.

    let html = `<div class="server-group${isCollapsed ? ' collapsed' : ''}" data-server="Humanity">
      <div class="server-group-header" data-server-toggle="Humanity" style="font-weight:bold;">
        <span class="collapse-arrow">▼</span>
        <span class="srv-name">🟢 ${esc(location.host || 'united-humanity.us')}</span>
      </div>
      <div class="server-group-channels">${channelsHtml}${createChannelBtn}</div>
    </div>`;

    // Federated servers.
    if (federatedServers.length > 0) {
      html += '<div style="padding:var(--space-sm) var(--space-md) var(--space-xs);font-size:0.7rem;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.05em;">Federation</div>';
      for (const s of federatedServers) {
        const tierBadge = s.trust_tier === 3 ? '🟢' : s.trust_tier === 2 ? '🟡' : s.trust_tier === 1 ? '🔵' : '⚪';
        const fedLive = (window._federationStatus || {})[s.server_id];
        const statusDot = (fedLive && fedLive.connected) ? '🟢' : s.status === 'online' ? '🟡' : s.status === 'unreachable' ? '🔴' : '⚫';
        html += `<div class="server-group" data-server="${esc(s.name)}">
          <div class="server-group-header" data-federated-url="${esc(s.url)}" title="Tier ${s.trust_tier}, ${esc(s.status)}\n${esc(s.url)}">
            <span>${statusDot} ${tierBadge} ${esc(s.name)}</span>
          </div>
        </div>`;
      }
    }

    // Add Server button (only show for admins).
    const myRole = (window.myPeerRole || '').toLowerCase();
    if (myRole === 'admin') {
      html += `<div style="padding:var(--space-md) var(--space-md);">
        <button onclick="promptAddServer()" style="font-size:0.75rem;padding:var(--space-xs) var(--space-md);cursor:pointer;background:var(--bg-hover);border:1px solid var(--border);border-radius:var(--radius-sm);color:var(--text-primary);width:100%;">+ Add Server</button>
      </div>`;
    }

    container.innerHTML = html;
    if (window.twemoji) twemoji.parse(container);
    if (typeof renderUnreadDots === 'function') renderUnreadDots();
    if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
  }
  window.renderServerList = renderServerList;

  // Federation Phase 2: track live connection status.
  window._federationStatus = {};
  window.updateFederationStatus = function(servers) {
    for (const s of servers) {
      window._federationStatus[s.server_id] = s;
    }
    renderServerList();
  };
  window.switchSidebarTab = switchSidebarTab;

  // Prompt to add a federated server via /server-add command.
  function promptAddServer() {
    const url = prompt('Enter server URL (e.g. https://chat.example.com):');
    if (!url) return;
    const name = prompt('Server name (optional):') || '';
    const cmd = name ? `/server-add ${url} ${name}` : `/server-add ${url}`;
    // Send the command as a chat message (the server intercepts slash commands).
    if (ws && ws.readyState === WebSocket.OPEN) {
      const msg = { type: 'chat', content: cmd, timestamp: Date.now(), channel: activeChannel || 'general' };
      ws.send(JSON.stringify(msg));
    }
    // Refresh after a delay to pick up the new server.
    setTimeout(() => { federatedServersFetched = false; renderServerList(); }, 3000);
  }
  window.promptAddServer = promptAddServer;

  // Event delegation for server list interactions
  document.getElementById('server-list').addEventListener('click', function(e) {
    // Federated server click, navigate to it.
    const fedHeader = e.target.closest('[data-federated-url]');
    if (fedHeader) {
      const url = fedHeader.getAttribute('data-federated-url');
      if (url && confirm(`Switch to server: ${url}?\n\nThis will open the server in a new tab.`)) {
        window.open(url, '_blank');
      }
      return;
    }
    // Toggle server collapse
    const toggle = e.target.closest('[data-server-toggle]');
    if (toggle) {
      const serverName = toggle.getAttribute('data-server-toggle');
      const group = toggle.closest('.server-group');
      const collapsed = getCollapsedServers();
      if (group.classList.contains('collapsed')) {
        group.classList.remove('collapsed');
        collapsed.delete(serverName);
      } else {
        group.classList.add('collapsed');
        collapsed.add(serverName);
      }
      saveCollapsedServers(collapsed);
      return;
    }
    // Channel mic, toggle voice join/leave for this channel (don't switch to it).
    const micEl = e.target.closest('.ch-mic');
    if (micEl) {
      const vch = micEl.getAttribute('data-voice-channel');
      if (vch) toggleChannelVoice(vch);
      return;
    }
    // Channel click (skip if clicking the settings cog)
    if (e.target.closest('.channel-cog')) return;
    const chItem = e.target.closest('.channel-item');
    if (chItem) {
      const channelId = chItem.getAttribute('data-channel-id');
      if (channelId) switchChannel(channelId);
      return;
    }
    // Voice channel actions (event delegation, no inline onclick)
    const actionBtn = e.target.closest('[data-action]');
    if (actionBtn) {
      const action = actionBtn.getAttribute('data-action');
      if (action === 'vc-join') {
        const vcId = actionBtn.getAttribute('data-vc-id');
        if (vcId) joinVoiceRoom(vcId);
      } else if (action === 'vc-leave') {
        leaveVoiceRoom();
      } else if (action === 'vc-create') {
        createVoiceRoom();
      } else if (action === 'create-text-channel') {
        const name = prompt('Channel name (letters, numbers, dashes, underscores):');
        if (name && name.trim()) {
          if (!ws || ws.readyState !== WebSocket.OPEN) {
            addNotice('Not connected. Reconnect, then retry create.', 'red', 8);
            return;
          }
          const normalized = name.trim().replace(/^#/, '').toLowerCase();
          if (!/^[a-z0-9_-]{1,24}$/.test(normalized)) {
            addSystemMessage('Invalid channel name. Use 1-24 chars: letters, numbers, dashes, underscores.');
          } else {
            const cmd = '/channel-create ' + normalized;
            if (!beginChannelAdminCmd('create')) return;
            addSystemMessage('⏳ Creating #' + normalized + ' ...');
            // Route admin channel-management commands through #general for consistent server handling.
            sendChatCommand(cmd, 'general').then(ok => { if (!ok) failChannelAdminCmd('Create command failed to send.'); }).catch(console.error);
          }
        }
      }
      return;
    }
  });
})();

// ── Mobile Sidebar Management ──
function isMobile() {
  return window.innerWidth <= 640;
}

function toggleSidebar(sidebarId) {
  const sidebar = document.getElementById(sidebarId);
  const overlay = document.getElementById('sidebar-overlay');
  const otherSidebar = sidebarId === 'sidebar'
    ? document.getElementById('right-sidebar')
    : document.getElementById('sidebar');

  // Close the other sidebar first.
  otherSidebar.classList.remove('open');

  if (sidebar.classList.contains('open')) {
    sidebar.classList.remove('open');
    overlay.classList.remove('open');
  } else {
    sidebar.classList.add('open');
    if (isMobile()) overlay.classList.add('open');
  }
}

function closeSidebars() {
  document.getElementById('sidebar').classList.remove('open');
  document.getElementById('right-sidebar').classList.remove('open');
  document.getElementById('sidebar-overlay').classList.remove('open');
}

// Close sidebars when tapping the overlay backdrop.
document.getElementById('sidebar-overlay').addEventListener('click', closeSidebars);

// ── Close sidebar on channel select (mobile) ──
// Patch switchChannel to close sidebar on mobile.
const _origSwitchChannel = switchChannel;
switchChannel = function(channelId) {
  // Clear unread for this channel.
  clearUnread(channelId);
  _origSwitchChannel(channelId);
  if (isMobile()) closeSidebars();
};

// ── Unread Indicators ──
// Track unread state and per-channel message counts.
var unreadChannels = new Set();
window.unreadChannelCounts = window.unreadChannelCounts || {};

function markUnread(channelId) {
  if (channelId === activeChannel) return; // Don't mark current channel.
  unreadChannels.add(channelId);
  window.unreadChannelCounts[channelId] = (window.unreadChannelCounts[channelId] || 0) + 1;
  renderUnreadDots();
}

function clearUnread(channelId) {
  unreadChannels.delete(channelId);
  delete window.unreadChannelCounts[channelId];
  renderUnreadDots();
}

function renderUnreadDots() {
  document.querySelectorAll('.channel-item').forEach(el => {
    // Get the channel id from data attribute or onclick attribute.
    let chId = el.getAttribute('data-channel-id');
    if (!chId) {
      const onclick = el.getAttribute('onclick') || '';
      const match = onclick.match(/switchChannel\('([^']+)'\)/);
      if (!match) return;
      chId = match[1];
    }

    // Remove existing dot.
    const existingDot = el.querySelector('.unread-dot');
    if (existingDot) existingDot.remove();
    el.classList.remove('has-unread');

    if (unreadChannels.has(chId)) {
      el.classList.add('has-unread');
      const dot = document.createElement('span');
      dot.className = 'unread-dot';
      el.appendChild(dot);
    }
  });
}

// Hook into handleMessage to track unread for other channels.
const _origHandleMessage2 = handleMessage;
handleMessage = function(msg) {
  // Intercept chat messages for other channels BEFORE the original handler skips them.
  if (msg.type === 'chat') {
    const msgChannel = msg.channel || 'general';
    if (msgChannel !== activeChannel) {
      markUnread(msgChannel);
    }
  }
  _origHandleMessage2(msg);
};

// ── Improved Context Menu Positioning ──
// Patch showUserContextMenu to prevent overflow on mobile.
const _origShowCtxMenu = showUserContextMenu;
showUserContextMenu = function(e, name, publicKey) {
  _origShowCtxMenu(e, name, publicKey);
  // Reposition if overflowing.
  const menu = document.getElementById('user-context-menu');
  const rect = menu.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  if (rect.right > vw) {
    menu.style.left = Math.max(4, vw - rect.width - 8) + 'px';
  }
  if (rect.bottom > vh) {
    menu.style.top = Math.max(4, vh - rect.height - 8) + 'px';
  }
  if (rect.left < 0) {
    menu.style.left = '4px';
  }
  if (rect.top < 0) {
    menu.style.top = '4px';
  }
};

// ── Mobile: Tap message to show/hide action buttons ──
if ('ontouchstart' in window || navigator.maxTouchPoints > 0) {
  document.getElementById('messages').addEventListener('click', function(e) {
    const msgEl = e.target.closest('.message:not(.system)');
    if (!msgEl) return;
    // Don't interfere with button clicks, links, quotes, images.
    if (e.target.closest('button, a, .quote-block, .img-placeholder, .img-loaded, .reaction-badge')) return;

    // Toggle mobile-selected on this message, deselect others.
    const wasSelected = msgEl.classList.contains('mobile-selected');
    document.querySelectorAll('.message.mobile-selected').forEach(m => m.classList.remove('mobile-selected'));
    if (!wasSelected) {
      msgEl.classList.add('mobile-selected');
    }
  });
}

// ── Improved Timestamp: "Yesterday" format ──
// Override formatTime to show "Yesterday HH:MM" for yesterday's messages.
formatTime = function(ts) {
  const d = new Date(ts);
  const now = new Date();
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  // Same day.
  if (d.toDateString() === now.toDateString()) {
    return time;
  }

  // Yesterday.
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) {
    return 'Yesterday ' + time;
  }

  // Same year.
  if (d.getFullYear() === now.getFullYear()) {
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' }) + ' ' + time;
  }

  // Different year.
  return d.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' }) + ' ' + time;
};

// ── Reaction picker: better mobile positioning ──
const _origShowReactionPicker = showReactionPicker;
showReactionPicker = function(btn, targetFrom, targetTs, msgEl) {
  // Close any existing picker.
  document.querySelectorAll('.reaction-picker').forEach(p => p.remove());

  const picker = document.createElement('div');
  picker.className = 'reaction-picker';
  // Base styles.
  picker.style.cssText = 'position:absolute;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-sm);display:flex;flex-wrap:wrap;gap:var(--space-xs);z-index:20;box-shadow:0 4px 12px rgba(0,0,0,0.4);';

  if (isMobile()) {
    // On mobile: position below the message, centered, larger buttons.
    picker.style.position = 'relative';
    picker.style.marginTop = 'var(--space-sm)';
    picker.style.justifyContent = 'center';
  } else {
    picker.style.top = '-2rem';
    picker.style.right = '0';
  }

  REACTION_EMOJIS.forEach(emoji => {
    const emojiBtn = document.createElement('span');
    emojiBtn.textContent = emoji;
    const size = isMobile() ? 'padding:var(--space-sm) var(--space-md);font-size:1.2rem;min-width:36px;text-align:center;' : 'padding:var(--space-xs) var(--space-sm);font-size:0.9rem;';
    emojiBtn.style.cssText = 'cursor:pointer;border-radius:var(--radius-sm);' + size;
    emojiBtn.onmouseover = () => emojiBtn.style.background = 'var(--bg-hover)';
    emojiBtn.onmouseout = () => emojiBtn.style.background = '';
    emojiBtn.onclick = (e) => {
      e.stopPropagation();
      sendReaction(targetFrom, targetTs, emoji);
      picker.remove();
    };
    picker.appendChild(emojiBtn);
  });
  if (window.twemoji) twemoji.parse(picker);

  msgEl.style.position = 'relative';
  msgEl.appendChild(picker);

  // Close picker on click elsewhere.
  setTimeout(() => {
    document.addEventListener('click', function closePicker(e) {
      if (!picker.contains(e.target)) {
        picker.remove();
        document.removeEventListener('click', closePicker);
      }
    });
  }, 0);
};

/* ── Command Palette ──
 *
 * Categories + items live in /data/commands.json (loaded at startup into
 * CMD_PALETTE_DATA). Items with action_id dispatch through CMD_PALETTE_ACTIONS.
 * Items with nav_url navigate the page. Categories with requires_role are
 * gated to mod/admin.
 */
let CMD_PALETTE_DATA = { categories: [] };
const CMD_PALETTE_ACTIONS = {
  sendFriendCodeRequest: function() { sendFriendCodeRequest(); },
  toggleSearch:          function() { toggleSearch(); },
  openServerStats:       function() { window.open('/info', '_blank'); },
};
fetch('/data/commands.json', { cache: 'no-cache' })
  .then(function(r) { return r.ok ? r.json() : null; })
  .then(function(j) { if (j && Array.isArray(j.categories)) CMD_PALETTE_DATA = j; })
  .catch(function() { /* silent, palette will be empty on first open */ });

function getCmdPaletteItems() {
const myRole = (typeof peerData !== 'undefined' && typeof myKey !== 'undefined' && peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
const isMod = myRole === 'admin' || myRole === 'mod';
const isAdmin = myRole === 'admin';

return CMD_PALETTE_DATA.categories
  .filter(function(cat) {
    if (!cat.requires_role) return true;
    if (cat.requires_role === 'mod') return isMod;
    if (cat.requires_role === 'admin') return isAdmin;
    return false;
  })
  .map(function(cat) {
    return {
      name: cat.name,
      items: cat.items.map(function(item) {
        // Hydrate action_id / nav_url into runtime action callbacks.
        var hydrated = Object.assign({}, item);
        if (item.action_id && CMD_PALETTE_ACTIONS[item.action_id]) {
          hydrated.action = CMD_PALETTE_ACTIONS[item.action_id];
        } else if (item.nav_url) {
          var url = item.nav_url;
          hydrated.action = function() { location.href = url; };
        }
        return hydrated;
      }),
    };
  });
}

function renderCmdPalette() {
const el = document.getElementById('cmd-palette');
const cats = getCmdPaletteItems();
let html = '';
cats.forEach(function(cat) {
  html += '<div class="cp-category">' + cat.name + '</div>';
  cat.items.forEach(function(item, i) {
    html += '<div class="cp-item" data-cat="' + cat.name + '" data-idx="' + i + '">' +
      '<span class="cp-icon">' + item.icon + '</span>' +
      '<span class="cp-label">' + item.label + '</span>' +
      '<span class="cp-desc">' + item.desc + '</span></div>';
  });
});
el.innerHTML = html;

el.querySelectorAll('.cp-item').forEach(function(row) {
  row.addEventListener('click', function() {
    const catName = row.dataset.cat;
    const idx = parseInt(row.dataset.idx);
    const cat = cats.find(function(c) { return c.name === catName; });
    if (!cat) return;
    const item = cat.items[idx];
    if (!item) return;
    closeCmdPalette();
    if (item.action) { item.action(); return; }
    const input = document.getElementById('msg-input');
    if (item.prefill) {
      input.value = item.cmd;
      input.focus();
      input.setSelectionRange(item.cmd.length, item.cmd.length);
    } else {
      input.value = item.cmd;
      sendMessage();
    }
  });
});
}

function toggleCmdPalette() {
const overlay = document.getElementById('cmd-palette-overlay');
if (overlay.classList.contains('open')) {
  closeCmdPalette();
} else {
  renderCmdPalette();
  // Position palette above input area
  const btn = document.getElementById('cmd-palette-btn');
  const palette = document.getElementById('cmd-palette');
  const inputArea = document.getElementById('input-area');
  const rect = inputArea.getBoundingClientRect();
  palette.style.bottom = (window.innerHeight - rect.top + 4) + 'px';
  palette.style.left = Math.max(4, rect.left) + 'px';
  overlay.classList.add('open');
}
}

function closeCmdPalette() {
document.getElementById('cmd-palette-overlay').classList.remove('open');
}

// Escape to close command palette
document.addEventListener('keydown', function(e) {
if (e.key === 'Escape') {
  const overlay = document.getElementById('cmd-palette-overlay');
  if (overlay && overlay.classList.contains('open')) { closeCmdPalette(); e.stopPropagation(); }
}
}, true);

// Skip SW in Tauri desktop, files are local, no caching needed, and
// Tauri serves missing files as text/html (the SPA fallback).
if ('serviceWorker' in navigator && !window.__TAURI__) {
navigator.serviceWorker.register('/sw.js')
  .then(reg => console.log('SW registered:', reg.scope))
  .catch(err => console.error('SW failed:', err));
}

/* ── User Status ── */
window.setMyStatus = function(status) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'set_status', status, text: '' }));
    document.querySelectorAll('.status-option').forEach(el => {
      el.classList.toggle('active', el.dataset.status === status);
    });
  }
};

window.clearMyStatus = function() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'set_status', status: 'online', text: '' }));
    document.querySelectorAll('.status-option').forEach(el => el.classList.remove('active'));
  }
};

/* ── Message Search ── */
(function() {
let searchTimer = null;
let lastSearchTime = 0;

window.toggleSearch = function() {
  const bar = document.getElementById('search-bar');
  bar.classList.toggle('open');
  if (bar.classList.contains('open')) {
    document.getElementById('search-input').focus();
  } else {
    closeSearch();
  }
};

window.closeSearch = function() {
  const bar = document.getElementById('search-bar');
  bar.classList.remove('open');
  document.getElementById('search-input').value = '';
  document.getElementById('search-from').value = '';
  document.getElementById('search-results').innerHTML = '';
  document.getElementById('search-results').classList.remove('open');
  document.getElementById('search-count').textContent = '';
};

function doSearch() {
  const query = document.getElementById('search-input').value.trim();
  const fromUser = document.getElementById('search-from').value.trim();
  if (query.length < 2) {
    document.getElementById('search-results').innerHTML = '';
    document.getElementById('search-results').classList.remove('open');
    document.getElementById('search-count').textContent = '';
    return;
  }
  // Rate limit client-side
  const now = Date.now();
  if (now - lastSearchTime < 2000) return;
  lastSearchTime = now;

  const msg = { type: 'search', query: query };
  if (typeof currentChannel !== 'undefined' && currentChannel) {
    // Don't filter by channel, search all. User can filter from dropdown later.
  }
  if (fromUser) msg.from = fromUser;
  if (typeof ws !== 'undefined' && ws && ws.readyState === 1) {
    ws.send(JSON.stringify(msg));
  }
}

document.getElementById('search-input').addEventListener('input', function() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(doSearch, 300);
});
document.getElementById('search-from').addEventListener('input', function() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(doSearch, 300);
});
document.getElementById('search-input').addEventListener('keydown', function(e) {
  if (e.key === 'Escape') closeSearch();
  if (e.key === 'Enter') { clearTimeout(searchTimer); doSearch(); }
});

function renderSearchResults(data) {
  const container = document.getElementById('search-results');
  const countEl = document.getElementById('search-count');
  if (!data.results || data.results.length === 0) {
    container.innerHTML = '<div style="padding:12px;color:var(--text-muted);text-align:center;">No results found</div>';
    container.classList.add('open');
    countEl.textContent = '0 results';
    return;
  }
  countEl.textContent = data.total + ' result' + (data.total !== 1 ? 's' : '');
  const query = data.query.toLowerCase();
  container.innerHTML = data.results.map(r => {
    const time = new Date(r.timestamp).toLocaleString();
    // Highlight match in content
    const escaped = r.content.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    const highlighted = escaped.replace(new RegExp('(' + query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi'), '<mark>$1</mark>');
    // Truncate content preview
    const preview = highlighted.length > 300 ? highlighted.substring(0, 300) + '…' : highlighted;
    return '<div class="search-result" data-channel="' + (r.channel || '') + '" data-timestamp="' + r.timestamp + '">' +
      '<div class="sr-meta"><span class="sr-author">' + (r.from_name || 'Unknown') + '</span>' +
      '<span class="sr-channel">#' + (r.channel || '?') + '</span>' +
      '<span class="sr-time">' + time + '</span></div>' +
      '<div class="sr-body">' + preview + '</div></div>';
  }).join('');
  container.classList.add('open');

  // Click handler for results
  container.querySelectorAll('.search-result').forEach(el => {
    el.addEventListener('click', function() {
      const ch = this.dataset.channel;
      const ts = parseInt(this.dataset.timestamp);
      if (ch && ch !== 'DM' && typeof switchChannel === 'function') {
        switchChannel(ch);
      }
      closeSearch();
      // Try to scroll to message near that timestamp
      setTimeout(() => {
        const msgs = document.querySelectorAll('.message');
        let closest = null, closestDiff = Infinity;
        msgs.forEach(m => {
          const mts = parseInt(m.dataset.timestamp || '0');
          const diff = Math.abs(mts - ts);
          if (diff < closestDiff) { closestDiff = diff; closest = m; }
        });
        if (closest) closest.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }, 500);
    });
  });
}

// Monkey-patch handleMessage for search_results
const _origHandleMessageSearch = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'search_results') {
    renderSearchResults(msg);
  } else {
    _origHandleMessageSearch(msg);
  }
};
})();

// ── Sidebar initial render ──
// Populate the unified right sidebar sections (Friends/Groups/Servers) immediately on load
// so the colored boxes are visible even before any WebSocket data arrives.
setTimeout(function() {
  if (window.__UNIFIED_RIGHT_SIDEBAR__ && typeof renderUnifiedRightSidebar === 'function') {
    renderUnifiedRightSidebar();
  }
  // Also ensure server channel list section is populated if channelList was already received.
  if (typeof renderServerList === 'function') renderServerList();
}, 250);
