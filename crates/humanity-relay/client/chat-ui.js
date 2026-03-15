// ── chat-ui.js ────────────────────────────────────────────────────────────
// Notifications, sounds, help modal, user context menu, sidebar navigation,
// unread indicators, mobile UX, command palette, search.
// Depends on: app.js globals (ws, myKey, myName, activeChannel, peerData,
//   esc, switchChannel, openDmConversation, sendMessage)
// ─────────────────────────────────────────────────────────────────────────

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

const SOUND_PRESETS = {
  chime:  { label: 'Chime',  freqs: [[523.25, 0], [659.25, 0.12]], type: 'sine', vol: 0.15, decay: 0.6 },
  ping:   { label: 'Ping',   freqs: [[880, 0]], type: 'sine', vol: 0.12, decay: 0.3 },
  bell:   { label: 'Bell',   freqs: [[1046.5, 0], [784, 0.08]], type: 'sine', vol: 0.1, decay: 0.8 },
  pop:    { label: 'Pop',    freqs: [[600, 0]], type: 'triangle', vol: 0.2, decay: 0.15 },
  drop:   { label: 'Drop',   freqs: [[800, 0], [400, 0.08]], type: 'sine', vol: 0.12, decay: 0.4 },
  blip:   { label: 'Blip',   freqs: [[1200, 0], [900, 0.06]], type: 'square', vol: 0.06, decay: 0.15 },
};

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
function renderSoundOptions() {
  const container = document.getElementById('sound-options');
  container.innerHTML = Object.entries(SOUND_PRESETS).map(([key, preset]) => {
    const checked = key === selectedSound ? 'checked' : '';
    return `<label style="font-size:0.8rem;color:var(--text);display:flex;align-items:center;gap:0.4rem;cursor:pointer;padding:0.15rem 0;">
      <input type="radio" name="sound-choice" value="${key}" ${checked} onchange="selectSound('${key}')" style="accent-color:var(--accent);">
      ${esc(preset.label)}
      <button onclick="event.preventDefault();previewSound('${key}')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;padding:0 0.3rem;">▶</button>
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

  // Always notify on @mention or DM, even if focused.
  if (mentioned || isDm || !windowFocused) {
    playNotificationChime();
  }

  // Browser notification (if permitted).
  if (Notification.permission === 'granted' && (!windowFocused || mentioned || isDm)) {
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

// Request notification permission (once, stored in localStorage).
function requestNotifications() {
  if ('Notification' in window && Notification.permission === 'default') {
    if (!localStorage.getItem('humanity_notif_asked')) {
      Notification.requestPermission();
      localStorage.setItem('humanity_notif_asked', '1');
    }
  }
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
    el.textContent = '✓ Ed25519 signatures enabled — messages will be cryptographically signed';
    el.style.color = 'var(--success)';
  } else {
    el.textContent = '⚠ Ed25519 not supported in this browser — messages will not be signed';
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
  } catch (e) { /* ignore — stats are cosmetic */ }
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
    case 'admin': return '<span class="role-badge" title="Admin">👑</span>';
    case 'mod': return '<span class="role-badge" title="Moderator">🛡️</span>';
    case 'verified': return '<span class="role-badge" title="Verified">✦</span>';
    case 'donor': return '<span class="role-badge" title="Donor">💎</span>';
    default: return '';
  }
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
      mod:    'border-left:3px solid #4a8;padding-left:9px;',
      admin:  'border-left:3px solid #56b;padding-left:9px;',
      danger: 'border-left:3px solid #e55;padding-left:9px;color:#e88;',
    }[tier] || '';
    return '<div class="ctx-item" style="' + borderStyle + '" onclick="' + onclick + '">' + label + '</div>';
  };

  // Role badge for target user header
  const roleBadge = {
    admin: '<span style="font-size:0.68rem;background:#56b;color:#fff;padding:1px 5px;border-radius:3px;margin-left:4px;">ADMIN</span>',
    mod:   '<span style="font-size:0.68rem;background:#4a8;color:#fff;padding:1px 5px;border-radius:3px;margin-left:4px;">MOD</span>',
  }[targetRole] || '<span style="font-size:0.68rem;background:#444;color:#aaa;padding:1px 5px;border-radius:3px;margin-left:4px;">USER</span>';

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
    html += '<div class="ctx-item" style="font-weight:600;pointer-events:none;padding-bottom:0.2rem;">' + esc(name) + roleBadge + '</div>';
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
        html += '<div class="ctx-item" style="font-size:0.68rem;color:#4a8;pointer-events:none;">\u2014 Mod Actions \u2014</div>';
        html += ci("ctxCommand('/kick')", '\uD83D\uDC62 Kick', 'mod');
        html += ci("ctxCommand('/mute')", '\uD83D\uDD07 Mute', 'mod');
        html += ci("ctxCommand('/ban')", '\uD83D\uDEB7 Ban', 'danger');
      }
      if (amAdmin) {
        html += '<div class="ctx-item" style="font-size:0.68rem;color:#56b;pointer-events:none;padding-top:0.3rem;">\u2014 Admin Actions \u2014</div>';
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
async function handleImportFile(event) {
  const file = event.target.files[0];
  if (!file) return;
  try {
    const text = await file.text();
    const jsonData = JSON.parse(text);
    const identity = await importIdentityFromJSON(jsonData);
    // Update state and connect
    document.getElementById('name-input').value = identity.name;
    myIdentity = identity;
    myKey = identity.publicKeyHex;
    myName = identity.name;
    addSystemMessage('✅ Identity imported successfully! Connecting...');
    connect();
  } catch (e) {
    const errEl = document.getElementById('login-error');
    errEl.textContent = '❌ Import failed: ' + e.message;
    errEl.style.display = 'block';
  }
  // Reset file input so the same file can be re-selected
  event.target.value = '';
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
          addSystemMessage('📋 Invite code copied: ' + group.invite_code + ' — Share it with /group-join ' + group.invite_code);
        }).catch(() => {
          addSystemMessage('📋 Invite code: ' + group.invite_code + ' — Share it with /group-join ' + group.invite_code);
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
        addSystemMessage('🔒 You must be friends to DM this user. Use /follow <name> — if they follow you back, you\'ll be friends.');
        return;
      }
    }
    if (val && ws && ws.readyState === WebSocket.OPEN) {
      const peerEcdh = getPeerEcdhPublic(activeDmPartner);
      let dmPayload = {
        type: 'dm',
        from: myKey,
        from_name: myName,
        to: activeDmPartner,
        content: val,
        timestamp: Date.now(),
      };
      // E2EE: encrypt if both parties have ECDH keys.
      if (peerEcdh && myEcdhKeyPair) {
        const enc = await encryptDmContent(val, peerEcdh);
        if (enc) {
          dmPayload.content = enc.content;
          dmPayload.nonce = enc.nonce;
          dmPayload.encrypted = true;
        }
      }
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

  // Tab click handler via event delegation — register FIRST before anything that might throw
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

    const mkSection = (id, label, panel) => {
      const wrap = document.createElement('div');
      wrap.className = 'unified-section';
      wrap.dataset.sid = id;
      const head = document.createElement('button');
      head.className = 'unified-header';
      head.dataset.baseLabel = label;
      head.textContent = label + ' ▾';
      const body = document.createElement('div');
      body.className = 'unified-body';
      body.appendChild(panel);
      panel.classList.add('force-show');
      head.onclick = () => {
        wrap.classList.toggle('collapsed');
        refreshUnifiedLeftHeaderCounts();
      };
      wrap.appendChild(head);
      wrap.appendChild(body);
      return wrap;
    };

    // Requested order: DMs (top), Groups (middle), Servers (bottom)
    if (!unified.querySelector('[data-sid="dms"]')) unified.appendChild(mkSection('dms', 'DMs', tabDms));
    if (!unified.querySelector('[data-sid="groups"]')) unified.appendChild(mkSection('groups', 'Groups', tabGroups));
    if (!unified.querySelector('[data-sid="servers"]')) unified.appendChild(mkSection('servers', 'Servers', tabServers));

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
        const head = sec.querySelector('.unified-header');
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
    const channelsHtml = channelList.map(ch => {
      const isActive = ch.id === activeChannel && !activeDmPartner;
      const title = ch.description ? ` title="${esc(ch.description)}"` : '';
      const lock = ch.read_only ? ' 🔒' : '';
      const cogHtml = (myRoleCh === 'admin' || myRoleCh === 'mod') ? `<span class="channel-cog" data-cog-type="text" data-cog-id="${esc(ch.id)}" data-cog-name="${esc(ch.name)}">⚙️</span>` : '';
      return `<div class="channel-item${isActive ? ' active' : ''}"${title} data-channel-id="${esc(ch.id)}">${cogHtml}${esc(ch.name)}${lock}</div>`;
    }).join('');

    // Text channel create button (admin/mod only)
    let createChannelBtn = '';
    if (myRoleCh === 'admin' || myRoleCh === 'mod') {
      createChannelBtn = '<div style="padding:0.2rem 0;"><button class="vr-btn" data-action="create-text-channel" style="width:100%;margin-top:0.2rem;font-size:0.7rem;">+ Create Channel</button></div>';
    }

    // Persistent voice channels section
    const voiceChannels = window._voiceChannels || [];
    let voiceHtml = '<div class="voice-rooms-section"><h4>🔊 Voice Channels</h4>';
    for (const vc of voiceChannels) {
      const inRoom = vc.participants.some(p => p.public_key === myKey);
      const hasParticipants = vc.participants.length > 0;
      const dimClass = hasParticipants ? '' : ' vc-empty';
      const vcCogHtml = (myRoleCh === 'admin' || myRoleCh === 'mod') ? `<span class="channel-cog" data-cog-type="voice" data-cog-id="${vc.id}" data-cog-name="${esc(vc.name)}">⚙️</span>` : '';
      voiceHtml += `<div class="voice-room-item${inRoom ? ' in-room' : ''}${dimClass}" data-vc-id="${vc.id}">
        <div class="vr-name">${vcCogHtml}🔊 ${esc(vc.name)}${hasParticipants ? ' <span class="vr-count">(' + vc.participants.length + ')</span>' : ''}</div>`;
      if (hasParticipants) {
        voiceHtml += '<div class="vr-participants">';
        const qMap = window._peerQualityCache || new Map();
        for (const p of vc.participants) {
          const q = qMap.get(p.public_key) || '';
          const qBadge = q ? ` <span class="quality-indicator">${q}</span>` : '';
          voiceHtml += `<div class="vr-participant" data-participant-key="${p.public_key}">🎤 ${esc(p.display_name)}${qBadge}</div>`;
        }
        voiceHtml += '</div>';
      }
      voiceHtml += '<div style="margin-top:0.2rem;">';
      if (inRoom) {
        voiceHtml += '<button class="vr-btn vr-leave" data-action="vc-leave">Leave</button>';
      } else {
        voiceHtml += `<button class="vr-btn vr-join" data-action="vc-join" data-vc-id="${vc.id}">Join</button>`;
      }
      voiceHtml += '</div></div>';
    }
    if (myRoleCh === 'admin' || myRoleCh === 'mod') {
      voiceHtml += '<button class="vr-btn" data-action="vc-create" style="margin-top:0.3rem;width:100%;">+ Create Voice Channel</button>';
    }
    voiceHtml += '</div>';

    let html = `<div class="server-group${isCollapsed ? ' collapsed' : ''}" data-server="Humanity">
      <div class="server-group-header" data-server-toggle="Humanity" style="font-weight:bold;">
        <span class="collapse-arrow">▼</span>
        <span>🟢 🅷 Humanity</span>
      </div>
      <div class="server-group-channels">${channelsHtml}${createChannelBtn}${voiceHtml}</div>
    </div>`;

    // Federated servers.
    if (federatedServers.length > 0) {
      html += '<div style="padding:0.3rem 0.5rem 0.1rem;font-size:0.7rem;color:var(--text-muted);text-transform:uppercase;letter-spacing:0.05em;">Federation</div>';
      for (const s of federatedServers) {
        const tierBadge = s.trust_tier === 3 ? '🟢' : s.trust_tier === 2 ? '🟡' : s.trust_tier === 1 ? '🔵' : '⚪';
        const fedLive = (window._federationStatus || {})[s.server_id];
        const statusDot = (fedLive && fedLive.connected) ? '🟢' : s.status === 'online' ? '🟡' : s.status === 'unreachable' ? '🔴' : '⚫';
        html += `<div class="server-group" data-server="${esc(s.name)}">
          <div class="server-group-header" data-federated-url="${esc(s.url)}" title="Tier ${s.trust_tier} — ${esc(s.status)}\n${esc(s.url)}">
            <span>${statusDot} ${tierBadge} ${esc(s.name)}</span>
          </div>
        </div>`;
      }
    }

    // Add Server button (only show for admins).
    const myRole = (window.myPeerRole || '').toLowerCase();
    if (myRole === 'admin') {
      html += `<div style="padding:0.4rem 0.5rem;">
        <button onclick="promptAddServer()" style="font-size:0.75rem;padding:0.2rem 0.5rem;cursor:pointer;background:var(--bg-hover);border:1px solid var(--border);border-radius:4px;color:var(--text-primary);width:100%;">+ Add Server</button>
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
    // Federated server click — navigate to it.
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
    // Channel click (skip if clicking the settings cog)
    if (e.target.closest('.channel-cog')) return;
    const chItem = e.target.closest('.channel-item');
    if (chItem) {
      const channelId = chItem.getAttribute('data-channel-id');
      if (channelId) switchChannel(channelId);
      return;
    }
    // Voice channel actions (event delegation — no inline onclick)
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
// Track unread state per channel.
var unreadChannels = new Set();

function markUnread(channelId) {
  if (channelId === activeChannel) return; // Don't mark current channel.
  unreadChannels.add(channelId);
  renderUnreadDots();
}

function clearUnread(channelId) {
  unreadChannels.delete(channelId);
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
  picker.style.cssText = 'position:absolute;background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:0.3rem;display:flex;flex-wrap:wrap;gap:0.2rem;z-index:20;box-shadow:0 4px 12px rgba(0,0,0,0.4);';

  if (isMobile()) {
    // On mobile: position below the message, centered, larger buttons.
    picker.style.position = 'relative';
    picker.style.marginTop = '0.3rem';
    picker.style.justifyContent = 'center';
  } else {
    picker.style.top = '-2rem';
    picker.style.right = '0';
  }

  REACTION_EMOJIS.forEach(emoji => {
    const emojiBtn = document.createElement('span');
    emojiBtn.textContent = emoji;
    const size = isMobile() ? 'padding:0.35rem 0.45rem;font-size:1.2rem;min-width:36px;text-align:center;' : 'padding:0.15rem 0.25rem;font-size:0.9rem;';
    emojiBtn.style.cssText = 'cursor:pointer;border-radius:4px;' + size;
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

/* ── Command Palette ── */
function getCmdPaletteItems() {
const myRole = (typeof peerData !== 'undefined' && typeof myKey !== 'undefined' && peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
const isMod = myRole === 'admin' || myRole === 'mod';
const isAdmin = myRole === 'admin';

const cats = [
  { name: '📱 Social', items: [
    { icon: '👁️', label: 'Follow User', desc: '/follow', cmd: '/follow ', prefill: true },
    { icon: '🚫', label: 'Unfollow User', desc: '/unfollow', cmd: '/unfollow ', prefill: true },
    { icon: '🚷', label: 'Block User', desc: '/block', cmd: '/block ', prefill: true },
    { icon: '✅', label: 'Unblock User', desc: '/unblock', cmd: '/unblock ', prefill: true },
    { icon: '📋', label: 'Block List', desc: 'View blocks', cmd: '/blocklist' },
    { icon: '🎟️', label: 'Share Friend Code', desc: 'Generate code', action: function(){ sendFriendCodeRequest(); } },
    { icon: '🔓', label: 'Redeem Friend Code', desc: '/redeem', cmd: '/redeem ', prefill: true },
  ]},
  { name: '💬 Messaging', items: [
    { icon: '📩', label: 'Direct Message', desc: '/dm', cmd: '/dm ', prefill: true },
    { icon: '👥', label: 'Create Group', desc: '/group-create', cmd: '/group-create ', prefill: true },
    { icon: '📨', label: 'Invite to Group', desc: '/group-invite', cmd: '/group-invite ', prefill: true },
    { icon: '🚪', label: 'Leave Group', desc: 'Leave current', cmd: '/group-leave' },
  ]},
  { name: '👤 Profile', items: [
    { icon: '📝', label: 'Set Bio', desc: '/bio', cmd: '/bio ', prefill: true },
    { icon: '🔗', label: 'Set Social Link', desc: '/social', cmd: '/social ', prefill: true },
    { icon: '👀', label: 'View Profile', desc: '/profile', cmd: '/profile ', prefill: true },
  ]},
  { name: '🔍 Search', items: [
    { icon: '🔍', label: 'Search Messages', desc: 'Open search panel', action: () => toggleSearch() },
    { icon: '🔎', label: 'Search Command', desc: '/search query', cmd: '/search ', prefill: true },
  ]},
  { name: '📌 Pins', items: [
    { icon: '📌', label: 'Pin Message', desc: '/pin', cmd: '/pin ', prefill: true },
    { icon: '📌', label: 'Personal Pin', desc: '/mypin', cmd: '/mypin ', prefill: true },
  ]},
];

if (isMod) {
  cats.push({ name: '🛡️ Moderation', items: [
    { icon: '👢', label: 'Kick', desc: '/kick', cmd: '/kick ', prefill: true },
    { icon: '🔨', label: 'Ban', desc: '/ban', cmd: '/ban ', prefill: true },
    { icon: '🔇', label: 'Mute', desc: '/mute', cmd: '/mute ', prefill: true },
    { icon: '📋', label: 'View Reports', desc: 'See reports', cmd: '/reports' },
  ]});
}

if (isAdmin) {
  cats.push({ name: '⚙️ Admin', items: [
    { icon: '✅', label: 'Verify User', desc: '/verify', cmd: '/verify ', prefill: true },
    { icon: '🛡️', label: 'Make Mod', desc: '/mod', cmd: '/mod ', prefill: true },
    { icon: '🔒', label: 'Lockdown', desc: 'Toggle lock', cmd: '/lockdown' },
    { icon: '📢', label: 'Create Channel', desc: '/channel-create', cmd: '/channel-create ', prefill: true },
  ]});
}

cats.push({ name: '🔧 Utility', items: [
  { icon: '❓', label: 'Help', desc: 'Show help', cmd: '/help' },
  { icon: '🔑', label: 'Export Identity', desc: 'Backup keys', cmd: '/export' },
  { icon: '🔗', label: 'Link Device', desc: 'Multi-device', cmd: '/link' },
  { icon: '📊', label: 'Server Stats', desc: 'View stats', cmd: '/stats', action: function(){ window.open('/info','_blank'); } },
]});

cats.push({ name: '🧭 Navigate', items: [
  { icon: '📊', label: 'Dashboard', desc: 'Go to /dashboard', action: function(){ location.href='/dashboard'; } },
  { icon: '🏠', label: 'Home', desc: 'Go to /home', action: function(){ location.href='/home'; } },
  { icon: '🧠', label: 'Skills', desc: 'Go to /skills', action: function(){ location.href='/skills'; } },
  { icon: '🎯', label: 'Tasks', desc: 'Go to /tasks', action: function(){ location.href='/tasks'; } },
  { icon: '⚔️', label: 'Quests', desc: 'Go to /quests', action: function(){ location.href='/quests'; } },
  { icon: '📅', label: 'Calendar', desc: 'Go to /calendar', action: function(){ location.href='/calendar'; } },
  { icon: '🗺️', label: 'Maps', desc: 'Go to /maps', action: function(){ location.href='/maps'; } },
  { icon: '📦', label: 'Inventory', desc: 'Go to /inventory', action: function(){ location.href='/inventory'; } },
  { icon: '📝', label: 'Notes', desc: 'Go to /notes', action: function(){ location.href='/notes'; } },
]});

return cats;
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

if ('serviceWorker' in navigator) {
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
    // Don't filter by channel — search all. User can filter from dropdown later.
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
