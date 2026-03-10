// Parse static emoji on page load — skip hub-nav so emoji match other pages
document.addEventListener('DOMContentLoaded', () => {
  if (window.twemoji) {
    document.querySelectorAll('#login-screen, #chat-screen').forEach(el => twemoji.parse(el));
  }
});

// ── State ──
let ws = null;
let myKey = '';
let myName = '';
let myIdentity = null; // { publicKeyHex, privateKey, publicKey, canSign }
let reconnectTimer = null;
let reconnectDelay = 1000;
const MAX_RECONNECT_DELAY = 30000;
let seenTimestamps = new Set(); // Deduplicate messages

// Persist name across sessions — auto-login if returning user.
const savedName = localStorage.getItem('humanity_name');
if (savedName) {
  document.getElementById('name-input').value = savedName;
  // Skip login screen immediately — show chat with "Connecting..." status.
  document.getElementById('login-screen').style.display = 'none';
  document.getElementById('chat-screen').style.display = 'flex';
  setStatus('reconnecting', 'Connecting…');
  // Auto-connect after a tick (let DOM settle).
  setTimeout(() => connect(), 50);
}

let pendingLinkCode = null;
let pendingInviteCode = null;
let identityConfirmed = false;
let activeChannel = localStorage.getItem('humanity_channel') || 'general';
let channelList = [];
let replyTarget = null; // { author, body, fromKey, timestamp, element }
let peerData = {};

function resolveSenderName(rawName, fromKey) {
  const given = (rawName || '').trim();
  if (given && !/^anonymous$/i.test(given)) return given;
  const peer = fromKey ? peerData[fromKey] : null;
  const peerName = (peer && (peer.display_name || peer.name) ? String(peer.display_name || peer.name).trim() : '');
  if (peerName && !/^anonymous$/i.test(peerName)) return peerName;
  return shortKey(fromKey);
}

// ── Reply Bar ──
function setReplyTarget(author, body, fromKey, timestamp, element) {
  const shortBody = body.length > 80 ? body.substring(0, 80) + '…' : body;
  replyTarget = { author, body, fromKey, timestamp, element };
  const bar = document.getElementById('reply-bar');
  document.getElementById('reply-preview').innerHTML =
    `<span class="reply-author">${esc(author)}</span> ${esc(shortBody)}`;
  bar.style.display = 'flex';
  if (window.twemoji) twemoji.parse(bar);
}

function clearReplyTarget() {
  replyTarget = null;
  document.getElementById('reply-bar').style.display = 'none';
  document.getElementById('reply-preview').innerHTML = '';
}

// Click reply preview → scroll to the original message.
document.getElementById('reply-preview').addEventListener('click', () => {
  if (replyTarget && replyTarget.element) {
    replyTarget.element.scrollIntoView({ behavior: 'smooth', block: 'center' });
    // Brief highlight effect.
    replyTarget.element.style.background = 'var(--accent-dim)';
    setTimeout(() => { replyTarget.element.style.background = ''; }, 1500);
  }
});

// Cancel reply.
document.getElementById('reply-cancel').addEventListener('click', (e) => {
  e.stopPropagation();
  clearReplyTarget();
  document.getElementById('msg-input').focus();
});

// Event delegation: handle clicks on image placeholders (data-img-url).
document.getElementById('messages').addEventListener('click', function(e) {
  const placeholder = e.target.closest('[data-img-url]');
  if (placeholder) {
    loadImage(placeholder, placeholder.dataset.imgUrl);
    return;
  }
  // Handle clicks on reaction badges (data-target-from).
  const badge = e.target.closest('[data-target-from]');
  if (badge) {
    sendReaction(badge.dataset.targetFrom, Number(badge.dataset.targetTs), badge.dataset.emoji);
  }
});

// ── Connect ──
async function connect() {
  myName = document.getElementById('name-input').value.trim() || 'Anonymous';
  pendingLinkCode = document.getElementById('link-code-input').value.trim() || null;
  pendingInviteCode = document.getElementById('invite-code-input').value.trim() || null;

  // Validate name: only ASCII letters, numbers, underscores, dashes. Max 24 chars.
  if (!/^[A-Za-z0-9_-]{1,24}$/.test(myName)) {
    const errEl = document.getElementById('login-error');
    errEl.textContent = 'Names can only contain letters (A-Z), numbers, underscores, and dashes. Max 24 characters.';
    errEl.style.display = 'block';
    return;
  }

  localStorage.setItem('humanity_name', myName);

  // Hide any previous error, show connecting status.
  document.getElementById('login-error').style.display = 'none';
  document.getElementById('crypto-status').textContent = 'Connecting…';
  document.getElementById('crypto-status').style.color = 'var(--text-muted)';

  // Initialize Ed25519 identity.
  myIdentity = await getOrCreateIdentity();
  myKey = myIdentity.publicKeyHex;

  // Initialize ECDH P-256 keypair for E2E encrypted DMs (non-blocking).
  getOrCreateEcdhKeypair().catch(e => console.warn('ECDH init failed:', e));

  // Stay on login screen — we switch to chat only after server confirms identity.
  identityConfirmed = false;
  openSocket();
}

// ── User Data Sync ──
// --- Encrypted Sync Data (AES-256-GCM) ---
async function deriveSyncKey() {
  if (!myIdentity || !myIdentity.privateKey) return null;
  try {
    const pkcs8 = await crypto.subtle.exportKey('pkcs8', myIdentity.privateKey);
    const hash = await crypto.subtle.digest('SHA-256', pkcs8);
    return await crypto.subtle.importKey('raw', hash, { name: 'AES-GCM' }, false, ['encrypt', 'decrypt']);
  } catch (e) {
    console.warn('Failed to derive sync encryption key:', e);
    return null;
  }
}

async function encryptSyncData(plaintext) {
  const key = await deriveSyncKey();
  if (!key) return plaintext; // Fallback to plaintext if no key
  try {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encoded = new TextEncoder().encode(plaintext);
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, encoded);
    return JSON.stringify({
      v: 1,
      key: myKey,
      iv: btoa(String.fromCharCode(...iv)),
      encrypted: btoa(String.fromCharCode(...new Uint8Array(ciphertext)))
    });
  } catch (e) {
    console.warn('Sync encryption failed, sending plaintext:', e);
    return plaintext;
  }
}

async function decryptSyncData(data) {
  try {
    const parsed = JSON.parse(data);
    if (!parsed.v || !parsed.encrypted) return data; // Plaintext (backward compat)
    if (parsed.key && parsed.key !== myKey) {
      console.warn('Sync data encrypted by different device key:', parsed.key);
      return null; // Can't decrypt — different device
    }
    const key = await deriveSyncKey();
    if (!key) return null;
    const iv = Uint8Array.from(atob(parsed.iv), c => c.charCodeAt(0));
    const ciphertext = Uint8Array.from(atob(parsed.encrypted), c => c.charCodeAt(0));
    const plainBuf = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext);
    return new TextDecoder().decode(plainBuf);
  } catch (e) {
    // Could be old plaintext data that happens to be valid JSON but not our format
    console.warn('Sync decryption failed, treating as plaintext:', e);
    return data;
  }
}

const SYNC_KEYS = [
  'humanity_settings', 'humanity_notes', 'humanity_todos', 'humanity_garden', 'humanity_garden_v2',
  'humanity_blocked', 'humanity_pins', 'humanity_default_tab',
  'humanity_browse', 'humanity_dashboard',
  'footer_collapsed', 'sidebar_tab'
];
let syncDebounceTimer = null;
let syncInitialized = false;

function getSyncBlob() {
  const blob = {};
  for (const key of SYNC_KEYS) {
    const val = localStorage.getItem(key);
    if (val !== null) blob[key] = val;
  }
  return JSON.stringify(blob);
}

function applySyncBlob(jsonStr) {
  try {
    const blob = JSON.parse(jsonStr);
    for (const key of SYNC_KEYS) {
      if (key in blob) {
        localStorage.setItem(key, blob[key]);
      }
    }
    // Re-apply settings if they changed.
    if (blob.humanity_settings && window.humanitySettings) {
      try {
        const s = JSON.parse(blob.humanity_settings);
        if (typeof applySettings === 'function') applySettings(s);
      } catch (e) {}
    }
  } catch (e) {
    console.warn('Failed to apply sync blob:', e);
  }
}

function scheduleSyncSave() {
  if (!syncInitialized) return;
  clearTimeout(syncDebounceTimer);
  syncDebounceTimer = setTimeout(async () => {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const data = getSyncBlob();
    const now = Date.now();
    localStorage.setItem('sync_updated_at', String(now));
    const encrypted = await encryptSyncData(data);
    ws.send(JSON.stringify({ type: 'sync_save', data: encrypted }));
  }, 5000);
}

async function handleSyncData(payload) {
  if (payload === 'null') {
    // No server data — push local data to server.
    syncInitialized = true;
    scheduleSyncSave();
    return;
  }
  try {
    const resp = JSON.parse(payload);
    const serverData = resp.data;
    const serverUpdatedAt = resp.updated_at || 0;
    const localUpdatedAt = parseInt(localStorage.getItem('sync_updated_at') || '0', 10);

    if (!localUpdatedAt || localUpdatedAt < serverUpdatedAt) {
      // Server is newer — decrypt and apply server data.
      const decrypted = await decryptSyncData(serverData);
      if (decrypted) {
        applySyncBlob(decrypted);
        localStorage.setItem('sync_updated_at', String(serverUpdatedAt));
      } else {
        // Can't decrypt (different device key) — keep local, re-encrypt on next save.
        console.warn('Could not decrypt sync data from server, keeping local data.');
        setTimeout(() => scheduleSyncSave(), 1000);
      }
    } else {
      // Local is newer or equal — push to server.
      setTimeout(() => scheduleSyncSave(), 1000);
    }
  } catch (e) {
    console.warn('Sync data parse error:', e);
  }
  syncInitialized = true;
}

function requestSyncLoad() {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify({ type: 'sync_load' }));
}

// Watch localStorage changes — intercept setItem to detect changes.
(function() {
  const origSetItem = localStorage.setItem.bind(localStorage);
  localStorage.setItem = function(key, value) {
    origSetItem(key, value);
    if (SYNC_KEYS.includes(key)) {
      scheduleSyncSave();
    }
  };
})();

// Called once the server accepts our identity (sends peer_list).
function onIdentityConfirmed() {
  if (identityConfirmed) return;
  identityConfirmed = true;

  document.getElementById('login-screen').style.display = 'none';
  document.getElementById('chat-screen').style.display = 'flex';

  // Show identity in sidebar.
  document.getElementById('my-key-display').textContent = myKey;
  document.getElementById('my-sign-status').innerHTML = myIdentity.canSign
    ? '<span style="color:var(--success)">✓ Signing enabled</span>'
    : '<span style="color:var(--warning)">⚠ Unsigned mode</span>';

  // Auto-download identity backup on first registration.
  if (myIdentity && myIdentity.isNew) {
    myIdentity.isNew = false; // Only trigger once
    setTimeout(async () => {
      const ok = await downloadIdentityBackup(myName);
      if (ok) {
        addNotice("🔑 IMPORTANT: Your identity file was downloaded. This is your ONLY recovery method if browser data is cleared. Save it somewhere safe (cloud drive, USB, email to yourself). Without it, your identity is GONE forever.", 'red', 120);
      }
      // Request persistent storage
      requestPersistentStorage();
    }, 1500);
  }

  // Notify if identity was restored from backup
  if (myIdentity && myIdentity.restored) {
    addNotice("🔑 Your identity was restored from a local backup. Your IndexedDB was cleared but we recovered your key. Please export a backup file for safety.", 'yellow', 30);
  }

  // Request persistent storage to prevent browser eviction of keys.
  requestPersistentStorage();

  // Request notification permission once.
  requestNotifications();

  // Sync profile to server on connect.
  try { syncProfileOnConnect(); } catch (e) { console.warn('Profile sync error:', e); }

  // Request user data sync from server.
  requestSyncLoad();

  // Don't load history here — wait for channel_list to arrive,
  // then switchChannel will load it.
  // If channel_list already arrived, load now.
  if (channelList.length > 0) {
    switchChannel(activeChannel);
  } else {
    // Fallback: load history for current channel, then reactions.
    loadHistory().then(() => loadReactionsForChannel(activeChannel));
  }
}

// ── History ──
async function loadHistory() {
  try {
    const resp = await fetch(`/api/messages?limit=100&channel=${encodeURIComponent(activeChannel)}`);
    const data = await resp.json();
    if (data.messages && data.messages.length > 0) {
      const notice = document.createElement('div');
      notice.id = 'history-notice';
      notice.textContent = `── ${data.messages.length} earlier messages ──`;
      document.getElementById('messages').appendChild(notice);

      // "New messages" divider: show where user left off last time.
      const lastSeen = parseInt(localStorage.getItem('humanity_last_seen') || '0');
      let newMsgDividerShown = false;

      let lastDate = '';
      for (const msg of data.messages) {
        const msgDate = new Date(msg.timestamp).toLocaleDateString();
        if (msgDate !== lastDate) {
          addDateSeparator(msgDate);
          lastDate = msgDate;
        }

        // Insert "New messages" divider before first unseen message.
        if (!newMsgDividerShown && lastSeen > 0 && msg.timestamp > lastSeen && msg.from !== myKey) {
          const divider = document.createElement('div');
          divider.className = 'new-messages-divider';
          divider.textContent = 'New messages';
          document.getElementById('messages').appendChild(divider);
          newMsgDividerShown = true;
        }

        const key = msg.from + ':' + msg.timestamp;
        seenTimestamps.add(key);
        addChatMessage(
          resolveSenderName(msg.from_name, msg.from),
          msg.content,
          msg.timestamp,
          msg.from,
          true, // isHistory
          !!msg.signature,
          msg.reply_to || null,
          msg.thread_count || null
        );
      }

      // Update last-seen to the newest message timestamp.
      const newest = data.messages[data.messages.length - 1];
      if (newest) localStorage.setItem('humanity_last_seen', String(newest.timestamp));

      // Scroll: if there's a "New messages" divider, scroll to it; otherwise scroll to bottom.
      const messagesDiv = document.getElementById('messages');
      const newDivider = messagesDiv.querySelector('.new-messages-divider');
      if (newDivider) {
        newDivider.scrollIntoView({ behavior: 'instant', block: 'center' });
      } else {
        messagesDiv.scrollTop = messagesDiv.scrollHeight;
      }
    } else {
      // No history — ensure we're at bottom for new messages.
      document.getElementById('messages').scrollTop = document.getElementById('messages').scrollHeight;
    }
  } catch (e) {
    console.warn('Failed to load history:', e);
  }
}

// ── WebSocket ──
function openSocket() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    return;
  }

  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  ws = new WebSocket(`${proto}//${location.host}/ws`);

  ws.onopen = () => {
    reconnectDelay = 1000;
    clearTimeout(reconnectTimer);

    const identifyMsg = {
      type: 'identify',
      public_key: myKey,
      display_name: myName,
    };
    // E2EE: Include ECDH public key for end-to-end encrypted DMs.
    if (myEcdhPublicBase64) {
      identifyMsg.ecdh_public = myEcdhPublicBase64;
    }
    if (pendingLinkCode) {
      identifyMsg.link_code = pendingLinkCode;
      pendingLinkCode = null;
    }
    if (pendingInviteCode) {
      identifyMsg.invite_code = pendingInviteCode;
      pendingInviteCode = null;
    }
    ws.send(JSON.stringify(identifyMsg));

    // Don't switch screens yet — wait for server to confirm via peer_list.
    // If already confirmed (reconnect), re-enable input.
    if (identityConfirmed) {
      setStatus('connected', 'Connected');
      document.getElementById('msg-input').disabled = false;
      document.getElementById('send-btn').disabled = false;
      document.getElementById('msg-input').focus();
      updateStats();
    }
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      handleMessage(msg);
    } catch (e) {
      console.error('Message handler error:', e, event.data?.slice?.(0, 200));
    }
  };

  ws.onclose = () => {
    setStatus('disconnected', 'Disconnected');
    document.getElementById('msg-input').disabled = true;
    document.getElementById('send-btn').disabled = true;
    scheduleReconnect();
  };

  ws.onerror = () => {
    // onclose will fire after this.
  };
}

function scheduleReconnect() {
  clearTimeout(reconnectTimer);
  setStatus('reconnecting', `Reconnecting in ${Math.round(reconnectDelay/1000)}s…`);
  reconnectTimer = setTimeout(() => {
    setStatus('reconnecting', 'Reconnecting…');
    openSocket();
    reconnectDelay = Math.min(reconnectDelay * 1.5, MAX_RECONNECT_DELAY);
  }, reconnectDelay);
}

// ── Message Handling ──
async function handleMessage(msg) {
  switch (msg.type) {
    case 'chat': {
      // Only show messages for the active channel.
      const msgChannel = msg.channel || 'general';
      if (msgChannel !== activeChannel) return;
      const key = msg.from + ':' + msg.timestamp;
      if (seenTimestamps.has(key)) return; // Deduplicate
      seenTimestamps.add(key);
      const hasSig = !!msg.signature;
      // If message has a signature, verify it client-side.
      if (hasSig && msg.signature && msg.from && !msg.from.startsWith('bot_')) {
        verifyMessage(msg.from, msg.signature, msg.content, msg.timestamp).then(valid => {
          addChatMessage(resolveSenderName(msg.from_name, msg.from), msg.content, msg.timestamp, msg.from, false, valid, msg.reply_to || null, msg.thread_count || null);
        });
      } else {
        addChatMessage(resolveSenderName(msg.from_name, msg.from), msg.content, msg.timestamp, msg.from, false, hasSig, msg.reply_to || null, msg.thread_count || null);
      }
      // If this message is a reply, update the parent's thread count badge in the DOM.
      if (msg.reply_to) {
        updateThreadBadge(msg.reply_to.from, msg.reply_to.timestamp);
      }
      break;
    }
    case 'federated_chat': {
      // Cross-server federated message — display with server badge.
      const fedChannel = msg.channel || 'general';
      if (fedChannel !== activeChannel) return;
      const fedKey = 'fed:' + msg.server_id + ':' + msg.timestamp;
      if (seenTimestamps.has(fedKey)) return;
      seenTimestamps.add(fedKey);
      const badgeHtml = `<span class="fed-badge" title="From ${esc(msg.server_name)}">[${esc(msg.server_name)}]</span> `;
      const fedAuthor = badgeHtml + esc(msg.from_name);
      addChatMessage(fedAuthor, msg.content, msg.timestamp, 'fed:' + msg.server_id, false, !!msg.signature, null, null, true);
      break;
    }
    case 'federation_status': {
      // Update federation server connection states.
      if (msg.servers && typeof updateFederationStatus === 'function') {
        updateFederationStatus(msg.servers);
      }
      break;
    }
    case 'peer_joined':
      // Update peerData with new peer info — sidebar handles visibility.
      peerData[msg.public_key] = { public_key: msg.public_key, display_name: msg.display_name, role: msg.role || '', ecdh_public: msg.ecdh_public || null };
      updateStats();
      break;
    case 'peer_left':
      // Keep ecdh_public in peerData for offline decryption of history
      if (peerData[msg.public_key]) {
        peerData[msg.public_key]._offline = true;
      }
      updateStats();
      break;
    case 'channel_list':
      updateChannelList(msg.channels || []);
      updateChannelHeader();
      updateInputForChannel();
      break;
    case 'peer_list':
      // Auto-reload on server update: if server_version changed, unregister SW and refresh.
      if (msg.server_version) {
        if (!window._serverVersion) {
          window._serverVersion = msg.server_version;
        } else if (window._serverVersion !== msg.server_version) {
          console.log('Server updated, clearing SW cache and reloading…');
          if ('serviceWorker' in navigator) {
            navigator.serviceWorker.getRegistrations().then(regs => {
              regs.forEach(r => r.unregister());
              if ('caches' in window) caches.keys().then(ks => ks.forEach(k => caches.delete(k)));
              setTimeout(() => location.reload(), 200);
            }).catch(() => location.reload());
          } else {
            location.reload();
          }
          return;
        }
      }
      // Server sent peer_list = identity accepted!
      if (!identityConfirmed) {
        onIdentityConfirmed();
      }
      // Always re-enable input and update status (handles reconnects too).
      setStatus('connected', 'Connected');
      document.getElementById('msg-input').disabled = false;
      document.getElementById('send-btn').disabled = false;
      document.getElementById('msg-input').focus();
      updateStats();
      updatePeerList(msg.peers);
      // Refresh DM sidebar conversations on connect/reconnect.
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'chat', from: myKey, from_name: myName, content: '/dms', timestamp: Date.now(), channel: 'general' }));
      }
      break;
    case 'full_user_list':
      updateUserList(msg.users || []);
      break;
    case 'typing': {
      // Show "X is typing…" indicator, clear after 3 seconds.
      const typerName = resolveSenderName(msg.from_name, msg.from);
      showTypingIndicator(typerName);
      break;
    }
    case 'reaction': {
      applyReaction(msg.target_from, msg.target_timestamp, msg.emoji, msg.from, msg.from_name);
      break;
    }
    case 'reactions_sync': {
      if (msg.reactions && msg.reactions.length > 0) {
        for (const r of msg.reactions) {
          applyReactionSync(r.target_from, r.target_timestamp, r.emoji, r.reactor_key);
        }
      }
      break;
    }
    case 'delete': {
      // Remove the deleted message from the DOM.
      const msgs = document.querySelectorAll('.message[data-from="' + msg.from + '"][data-timestamp="' + msg.timestamp + '"]');
      msgs.forEach(m => m.remove());
      break;
    }
    case 'edit': {
      // Update the edited message in the DOM.
      applyEditToDOM(msg.from, msg.timestamp, msg.new_content);
      break;
    }
    case 'pins_sync': {
      // Full pin list for a channel.
      if (msg.channel === activeChannel) {
        currentPins = msg.pins || [];
        renderPinBar();
      }
      break;
    }
    case 'pin_added': {
      if (msg.channel === activeChannel) {
        currentPins.push(msg.pin);
        renderPinBar();
      }
      break;
    }
    case 'pin_removed': {
      if (msg.channel === activeChannel) {
        // Remove by 1-based index.
        const idx = msg.index - 1;
        if (idx >= 0 && idx < currentPins.length) {
          currentPins.splice(idx, 1);
        }
        renderPinBar();
      }
      break;
    }
    case 'profile_data': {
      // Cache profile data and show if we were waiting for it.
      if (msg.name) {
        profileCache[msg.name.toLowerCase()] = { bio: msg.bio || '', socials: msg.socials || '{}' };
        // If we have a pending view for this user, show it.
        if (pendingProfileView && pendingProfileView.name.toLowerCase() === msg.name.toLowerCase()) {
          showViewProfileCard(pendingProfileView.name, pendingProfileView.publicKey, msg.bio || '', msg.socials || '{}');
          pendingProfileView = null;
        }
        // If this is our own profile data on connect, update local storage.
        if (msg.name.toLowerCase() === myName.toLowerCase()) {
          try {
            const socials = JSON.parse(msg.socials || '{}');
            // Only overwrite local if server has data and local is empty.
            const local = loadProfileLocal();
            if ((!local.bio && !local.socials) || (Object.keys(local.socials || {}).length === 0 && !local.bio)) {
              saveProfileLocal(msg.bio || '', socials);
            }
          } catch {}
        }
      }
      break;
    }
    case 'dm': {
      // Incoming/outgoing DM event.
      const dmFrom = msg.from;
      const dmFromName = resolveSenderName(msg.from_name, dmFrom);
      const dmPartnerKey = (dmFrom === myKey) ? msg.to : dmFrom;
      const dmPartnerName = (dmFrom === myKey) ? (peerData[msg.to]?.display_name || shortKey(msg.to || '')) : dmFromName;
      let dmContent = msg.content;
      let dmIsEncrypted = !!msg.encrypted;
      // E2EE: Decrypt if encrypted.
      if (msg.encrypted && msg.nonce) {
        const senderEcdh = getPeerEcdhPublic(dmFrom);
        if (senderEcdh) {
          const plain = await decryptDmContent(msg.content, msg.nonce, senderEcdh);
          if (plain !== null) {
            dmContent = plain;
          } else {
            dmContent = '🔒 [Decryption failed]';
          }
        } else {
          dmContent = '🔒 [Cannot decrypt — missing sender key]';
        }
      }
      upsertDmConversation(dmPartnerKey, dmPartnerName, dmIsEncrypted ? '🔒 Encrypted message' : dmContent, msg.timestamp, dmFrom !== myKey);
      if (activeDmPartner && (dmFrom === activeDmPartner || dmFrom === myKey)) {
        addDmMessage(dmFromName, dmContent, msg.timestamp, dmFrom, msg.to, dmIsEncrypted);
      }
      // Notify.
      if (dmFrom !== myKey) {
        notifyNewMessage(dmFromName, dmIsEncrypted ? '🔒 Encrypted message' : dmContent, true);
      }
      break;
    }
    case 'dm_list': {
      dmConversations = msg.conversations || [];
      renderDmList();
      break;
    }
    case 'dm_history': {
      // Received conversation history for a DM.
      if (activeDmPartner === msg.partner) {
        document.getElementById('messages').innerHTML = '';
        const msgs = msg.messages || [];
        // E2EE status banner
        const partnerEcdh = getPeerEcdhPublic(msg.partner);
        const e2eeNotice = document.createElement('div');
        e2eeNotice.style.cssText = 'text-align:center;font-size:0.7rem;padding:0.3rem;color:var(--text-muted);';
        if (partnerEcdh && myEcdhKeyPair) {
          e2eeNotice.innerHTML = '🔒 Messages are end-to-end encrypted';
        } else {
          e2eeNotice.innerHTML = '🔓 Messages are <b>not</b> encrypted — the other party does not support E2EE';
        }
        document.getElementById('messages').appendChild(e2eeNotice);
        if (msgs.length > 0) {
          const notice = document.createElement('div');
          notice.id = 'history-notice';
          notice.textContent = `── ${msgs.length} earlier messages ──`;
          document.getElementById('messages').appendChild(notice);
        }
        for (const m of msgs) {
          let histContent = m.content;
          let histEncrypted = !!m.encrypted;
          if (m.encrypted && m.nonce) {
            // Determine peer key: if from me, use partner's key; if from partner, use partner's key
            const peerKeyForDecrypt = getPeerEcdhPublic(m.from === myKey ? m.to : m.from) || getPeerEcdhPublic(msg.partner);
            if (peerKeyForDecrypt) {
              const plain = await decryptDmContent(m.content, m.nonce, peerKeyForDecrypt);
              histContent = plain !== null ? plain : '🔒 [Decryption failed]';
            } else {
              histContent = '🔒 [Cannot decrypt — missing key]';
            }
          }
          addDmMessage(resolveSenderName(m.from_name, m.from), histContent, m.timestamp, m.from, m.to, histEncrypted);
        }
      }
      break;
    }
    case 'link_previews': {
      // Render link previews under the matching message.
      if (msg.channel !== activeChannel) break;
      const msgEl = document.querySelector(`.message[data-from="${msg.from}"][data-timestamp="${msg.timestamp}"]`);
      if (msgEl && msg.previews && msg.previews.length > 0) {
        const bodyEl = msgEl.querySelector('.body');
        if (bodyEl) {
          for (const p of msg.previews.slice(0, 3)) {
            const card = document.createElement('div');
            card.className = 'link-preview';
            card.onclick = () => card.classList.toggle('collapsed');
            let html = '<div class="lp-text">';
            if (p.site_name) html += `<div class="lp-site">${esc(p.site_name)}</div>`;
            if (p.title) html += `<div class="lp-title"><a href="${esc(p.url)}" target="_blank" rel="noopener">${esc(p.title)}</a></div>`;
            if (p.description) html += `<div class="lp-desc">${esc(p.description)}</div>`;
            html += '</div>';
            if (p.image) html += `<img class="lp-thumb" src="${esc(p.image)}" alt="" loading="lazy" onerror="this.style.display='none'">`;
            card.innerHTML = html;
            bodyEl.after(card);
          }
        }
      }
      break;
    }
    case 'device_list':
      renderDeviceList(msg.devices);
      break;
    case 'system':
      // Handle sync data responses (encoded as system messages with prefix).
      if (msg.message && msg.message.startsWith('__sync_data__:')) {
        const payload = msg.message.slice('__sync_data__:'.length);
        handleSyncData(payload);
        break;
      }
      if (msg.message === 'sync_ack') break; // Silent ack
      const handledAdminFeedback = handleChannelAdminFeedback(msg.message);
      if (!handledAdminFeedback) addSystemMessage(msg.message);
      break;
    case 'name_taken': {
      // Stop reconnecting — this is a permanent error, not a transient disconnect.
      clearTimeout(reconnectTimer);
      reconnectDelay = 1000;
      // Clear the saved name so auto-login doesn't loop.
      localStorage.removeItem('humanity_name');
      // Show login screen with error.
      document.getElementById('login-screen').style.display = 'flex';
      document.getElementById('chat-screen').style.display = 'none';
      const errEl = document.getElementById('login-error');
      errEl.textContent = msg.message;
      errEl.style.display = 'block';
      document.getElementById('crypto-status').textContent = '';
      identityConfirmed = false;
      if (ws) { ws.onclose = null; ws.close(); ws = null; }
      setStatus('disconnected', 'Choose a different name');
      break;
    }
  }
}

async function sendMessage() {
  const input = document.getElementById('msg-input');
  let content = input.value.trim();
  if (!content || !ws || ws.readyState !== WebSocket.OPEN) return;
  if (!identityConfirmed || !myKey || !myName || myName.toLowerCase() === 'anonymous') {
    addNotice('Identity still initializing. Please retry in a moment.', 'yellow', 6);
    return;
  }

  // Enforce character limit (on user's own text, before adding quote).
  const charLimit = getMaxMsgLength();
  if (content.length > charLimit) {
    addSystemMessage(`Message too long (${content.length}/${charLimit} chars). Please shorten it.`);
    return;
  }

  // Build reply_to reference if replying.
  let replyRef = null;
  if (replyTarget) {
    replyRef = {
      from: replyTarget.fromKey,
      from_name: replyTarget.author,
      content: replyTarget.body,
      timestamp: replyTarget.timestamp,
    };
    clearReplyTarget();
  }

  const timestamp = Date.now();

  // Sign the content if Ed25519 is available.
  let signature = null;
  if (myIdentity && myIdentity.canSign) {
    signature = await signMessage(myIdentity.privateKey, content, timestamp);
  }

  const msg = {
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: content,
    timestamp: timestamp,
    channel: activeChannel,
  };
  if (signature) {
    msg.signature = signature;
  }
  if (replyRef) {
    msg.reply_to = replyRef;
  }

  ws.send(JSON.stringify(msg));

  const key = myKey + ':' + timestamp;
  seenTimestamps.add(key);
  addChatMessage(myName, content, timestamp, myKey, false, !!signature, replyRef, null);
  input.value = '';
  input.style.height = 'auto'; // Reset textarea height after sending.
  input.focus();
}

let channelAdminCmdInFlight = null;

function beginChannelAdminCmd(opLabel) {
  if (channelAdminCmdInFlight) {
    addNotice('Another channel action is still in progress. Please wait.', 'yellow', 6);
    return false;
  }
  const timeout = setTimeout(() => {
    if (channelAdminCmdInFlight) {
      addNotice('Channel action timed out. Please retry.', 'red', 8);
      channelAdminCmdInFlight = null;
    }
  }, 12000);
  channelAdminCmdInFlight = { opLabel, timeout };
  return true;
}

function resolveChannelAdminCmd(successText) {
  if (!channelAdminCmdInFlight) return;
  clearTimeout(channelAdminCmdInFlight.timeout);
  channelAdminCmdInFlight = null;
  if (successText) addNotice(successText, 'green', 6);
}

function failChannelAdminCmd(reasonText) {
  if (!channelAdminCmdInFlight) return;
  clearTimeout(channelAdminCmdInFlight.timeout);
  channelAdminCmdInFlight = null;
  addNotice(reasonText, 'red', 10);
}

function handleChannelAdminFeedback(message) {
  if (!message || !channelAdminCmdInFlight) return false;
  const m = String(message);
  if (/^Channel #.+ created\.$/i.test(m) || /^Channel #.+ deleted\.$/i.test(m) || /^Channel #.+ renamed to #.+\.$/i.test(m)) {
    resolveChannelAdminCmd('✅ ' + m);
    return true;
  }
  if (/(Only admins|Only admins and mods|Cannot delete|Cannot rename|not found|Invalid channel name|Unable to rename|Usage: \/channel-)/i.test(m)) {
    failChannelAdminCmd(m);
    return true;
  }
  return false;
}

async function sendChatCommand(command, channelOverride) {
  if (!command) return false;
  if (!identityConfirmed || !myKey || !myName || myName.toLowerCase() === 'anonymous') {
    addSystemMessage('Identity not ready yet. Please wait a moment and retry.');
    return false;
  }
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    addSystemMessage('Not connected. Please reconnect and try again.');
    return false;
  }

  const timestamp = Date.now();
  const msg = {
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: command,
    timestamp,
    channel: channelOverride || activeChannel || 'general',
  };

  try {
    if (myIdentity && myIdentity.canSign) {
      const signature = await signMessage(myIdentity.privateKey, command, timestamp);
      if (signature) msg.signature = signature;
    }
  } catch (e) {
    console.warn('sendChatCommand: signature failed, sending unsigned command', e);
  }

  try {
    ws.send(JSON.stringify(msg));
    return true;
  } catch (e) {
    console.error('sendChatCommand: ws.send failed', e);
    addSystemMessage('Command failed to send. Check connection and try again.');
    return false;
  }
}

// ── Rendering ──
function addChatMessage(author, body, timestamp, fromKey, isHistory, signed, replyTo, threadCount, isFederated) {
  // Skip messages from blocked users entirely.
  if (author && isBlocked(author)) return;

  const el = document.createElement('div');
  el.className = 'message' + (isFederated ? ' federated-msg' : '');
  el.dataset.from = fromKey;
  el.dataset.timestamp = timestamp;

  const time = formatTime(timestamp);
  const isMe = fromKey === myKey;
  const isBot = fromKey && fromKey.startsWith('bot_');
  const isFed = fromKey && fromKey.startsWith('fed:');

  let authorClass = '';
  if (isMe) {
    authorClass = ' you';
  } else if (isBot) {
    authorClass = ' bot';
  }

  const sigBadge = signed
    ? '<span class="sig-badge" title="Ed25519 signed">✓</span>'
    : '';

  // Action buttons: react, reply, edit (own), pin (admin/mod), delete (own).
  const myRole = (peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
  const isStaff = myRole === 'admin' || myRole === 'mod';
  let actions = '<div class="msg-actions">';
  actions += '<button class="react-btn" title="React">😀</button>';
  actions += '<button class="reply-btn" title="Reply">↩</button>';
  if (isMe) {
    actions += '<button class="edit-btn" title="Edit">✏️</button>';
  }
  if (isStaff) {
    actions += '<button class="pin-btn" title="Pin (server)">📌</button>';
  }
  actions += '<button class="mypin-btn" title="Pin for me">⭐</button>';
  if (isMe) {
    actions += '<button class="delete-btn" title="Delete">✕</button>';
  }
  actions += '</div>';

  const isBotMsg = fromKey && fromKey.startsWith('bot_');
  const identiconSrc = (!isBotMsg && fromKey) ? generateIdenticon(fromKey, 20) : '';
  const identiconHtml = isBotMsg ? '<span class="identicon" style="font-size:18px;line-height:20px;">🤖</span>' : (identiconSrc ? `<img src="${identiconSrc}" class="identicon" alt="">` : '');

  // Look up role for author badge.
  const peerRole = (peerData[fromKey] && peerData[fromKey].role) ? peerData[fromKey].role : '';
  const badge = roleBadge(peerRole);

  // Check for todo-channel special rendering.
  let bodyHtml;
  const isTodoChannel = activeChannel === 'todo';
  const isHeronBot = fromKey && fromKey.startsWith('bot_') && (author === 'Heron 🪶' || author === 'Heron');
  if (isTodoChannel && isHeronBot) {
    const todoHtml = formatTodoMessage(body);
    bodyHtml = todoHtml || formatBody(body);
  } else {
    bodyHtml = formatBody(body);
  }

  // Reply indicator HTML.
  let replyIndicatorHtml = '';
  if (replyTo) {
    const replyPreview = (replyTo.content || '').substring(0, 60) + ((replyTo.content || '').length > 60 ? '…' : '');
    replyIndicatorHtml = `<div class="reply-indicator" data-reply-from="${esc(replyTo.from)}" data-reply-ts="${replyTo.timestamp}">
      <span>↩</span>
      <span class="reply-indicator-author">${esc(replyTo.from_name || 'Unknown')}</span>
      <span class="reply-indicator-preview">${esc(replyPreview)}</span>
    </div>`;
    el.classList.add('has-reply');
  }

  // Thread count badge HTML.
  let threadBadgeHtml = '';
  if (threadCount && threadCount > 0) {
    threadBadgeHtml = `<div class="thread-badge" data-thread-from="${esc(fromKey)}" data-thread-ts="${timestamp}">💬 ${threadCount} ${threadCount === 1 ? 'reply' : 'replies'}</div>`;
  }

  el.innerHTML = `
    ${replyIndicatorHtml}
    <div class="meta">
      ${identiconHtml}
      <span class="author${authorClass}" data-username="${isFed ? '' : esc(author)}" data-pubkey="${esc(fromKey)}" style="cursor:pointer;">${isFed ? author : esc(author)}</span>${badge}
      ${sigBadge}
      <span class="time">${time}</span>
    </div>
    <div class="body">${bodyHtml}</div>
    <div class="reactions" data-from="${esc(fromKey)}" data-ts="${timestamp}"></div>
    ${threadBadgeHtml}
    ${actions}
  `;

  // Context menu on author name click.
  const authorEl = el.querySelector('.author');
  if (authorEl) {
    authorEl.addEventListener('click', (e) => {
      e.stopPropagation();
      showUserContextMenu(e, author, fromKey);
    });
  }

  // Click react button → show emoji picker.
  el.querySelector('.react-btn').addEventListener('click', (e) => {
    e.stopPropagation();
    showReactionPicker(e.target, fromKey, timestamp, el);
  });

  // Click reply button → show reply preview bar above input.
  el.querySelector('.reply-btn').addEventListener('click', (e) => {
    e.stopPropagation();
    setReplyTarget(author, body, fromKey, timestamp, el);
    document.getElementById('msg-input').focus();
  });

  // Click edit button → inline edit mode.
  const editBtn = el.querySelector('.edit-btn');
  if (editBtn) {
    editBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      startEditMode(el, body, fromKey, timestamp);
    });
  }

  // Click pin button → server pin (admin/mod).
  const pinBtn = el.querySelector('.pin-btn');
  if (pinBtn) {
    pinBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      pinMessageFromUI(fromKey, author, body, timestamp);
    });
  }

  // Click ⭐ button → personal pin.
  const mypinBtn = el.querySelector('.mypin-btn');
  if (mypinBtn) {
    mypinBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      toggleMyPin(fromKey, author, body, timestamp);
    });
  }

  // Click delete button → send delete request.
  const delBtn = el.querySelector('.delete-btn');
  if (delBtn) {
    delBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'delete', from: myKey, timestamp: Number(timestamp) }));
        el.remove(); // Remove locally immediately.
      }
    });
  }

  // Click reply indicator → scroll to original message or show inline.
  const replyInd = el.querySelector('.reply-indicator');
  if (replyInd) {
    replyInd.addEventListener('click', (e) => {
      e.stopPropagation();
      const rFrom = replyInd.dataset.replyFrom;
      const rTs = replyInd.dataset.replyTs;
      // Find the original message in DOM.
      const origEl = document.querySelector(`.message[data-from="${rFrom}"][data-timestamp="${rTs}"]`);
      if (origEl) {
        origEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
        origEl.style.background = 'var(--accent-dim, rgba(88,166,255,0.15))';
        setTimeout(() => { origEl.style.background = ''; }, 1500);
      }
    });
  }

  // Click thread badge → open thread panel.
  const threadBadge = el.querySelector('.thread-badge');
  if (threadBadge) {
    threadBadge.addEventListener('click', (e) => {
      e.stopPropagation();
      openThreadPanel(fromKey, timestamp, author, body);
    });
  }

  appendMessage(el);
  if (window.twemoji) twemoji.parse(el);
}

function addSystemMessage(text) {
  // Route certain messages as ephemeral notices instead of permanent system messages.
  const lower = text.toLowerCase();
  // Link codes — ephemeral yellow, 5 minutes (matches server expiry)
  if (lower.includes('link code:')) {
    return addNotice(text, 'yellow', 300);
  }
  // Invite codes — ephemeral yellow, 24h display for 60s (code lasts 24h, notice fades)
  if (lower.includes('invite code:')) {
    return addNotice(text, 'yellow', 120);
  }
  // Rate limiting / slow mode — ephemeral cyan, 15s
  if (lower.includes('rate limit') || lower.includes('please wait') || lower.includes('slow mode')) {
    return addNotice(text, 'cyan', 15);
  }
  // Kick/ban/mute — important red, 30s
  if (lower.includes('kicked') || lower.includes('banned') || lower.includes('muted')) {
    return addNotice(text, 'red', 30);
  }
  // Lockdown — red, 30s
  if (lower.includes('lockdown')) {
    return addNotice(text, 'red', 30);
  }
  // Pin actions — green, 20s
  if (lower.includes('pinned a message')) {
    return addNotice(text, 'green', 20);
  }
  // Verified/donor — green, 20s
  if (lower.includes('verified') || lower.includes('donor')) {
    return addNotice(text, 'green', 20);
  }
  // Everything else — regular system message
  const el = document.createElement('div');
  el.className = 'message system';
  el.textContent = `• ${text}`;
  appendMessage(el);
}

function formatCountdown(secs) {
  if (secs >= 60) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return s > 0 ? `${m}m ${s}s` : `${m}m`;
  }
  return `${secs}s`;
}

/**
 * Add an ephemeral notice with countdown timer.
 * @param {string} text - Notice text
 * @param {string} color - red|yellow|green|blue|cyan|magenta
 * @param {number} seconds - Auto-dismiss after N seconds
 */
function addNotice(text, color, seconds) {
  const el = document.createElement('div');
  el.className = `notice notice-${color}`;
  const textSpan = document.createElement('span');
  textSpan.className = 'notice-text';
  textSpan.textContent = text;
  const timerSpan = document.createElement('span');
  timerSpan.className = 'notice-timer';
  let remaining = seconds;
  timerSpan.textContent = formatCountdown(remaining);
  el.appendChild(textSpan);
  el.appendChild(timerSpan);
  appendMessage(el);

  const interval = setInterval(() => {
    remaining--;
    if (remaining <= 0) {
      clearInterval(interval);
      el.classList.add('fading');
      setTimeout(() => el.remove(), 500);
    } else {
      timerSpan.textContent = formatCountdown(remaining);
    }
  }, 1000);
}

function addDateSeparator(dateStr) {
  const el = document.createElement('div');
  el.className = 'date-separator';
  el.textContent = dateStr;
  document.getElementById('messages').appendChild(el);
}

function appendMessage(el) {
  const container = document.getElementById('messages');
  const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 100;
  container.appendChild(el);
  if (isNearBottom) {
    container.scrollTop = container.scrollHeight;
  }
}

let myUploadToken = '';
function updatePeerList(peers) {
  // Update peerData from peer_list (online peers only — for backwards compat).
  for (const p of peers) {
    peerData[p.public_key] = p;
    // M-4: Capture our upload token.
    if (p.public_key === myKey && p.upload_token) {
      myUploadToken = p.upload_token;
    }
    // Track our role for UI rendering (federation admin buttons, etc.).
    if (p.public_key === myKey && p.role) {
      window.myPeerRole = p.role;
    }
  }
}

function updateUserList(users) {
  const list = document.getElementById('peer-list');

  // Update peerData from full user list.
  for (const u of users) {
    peerData[u.public_key] = { public_key: u.public_key, display_name: u.name, role: u.role || '', ecdh_public: u.ecdh_public || null };
  }

  const online = users.filter(u => u.online);
  const offline = users.filter(u => !u.online);

  function renderUser(u) {
    const isMe = u.name === myName;
    const isBot = u.public_key && u.public_key.startsWith('bot_');
    const icon = isBot ? '<span style="font-size:14px;vertical-align:middle;">🤖</span>' : (u.public_key ? `<img src="${generateIdenticon(u.public_key, 16)}" class="identicon" style="width:14px;height:14px;">` : '');
    const badge = isBot ? ' <img src="https://cdn.jsdelivr.net/npm/@twemoji/svg@latest/1fab6.svg" alt="🪶" style="width:12px;height:12px;vertical-align:middle;"> ' : roleBadge(u.role);
    const escapedName = esc(u.name);
    const escapedKey = esc(u.public_key);
    const deviceCount = (!isBot && u.key_count > 1) ? ` <span style="font-size:0.6rem;color:var(--text-muted)">(${u.key_count} devices)</span>` : '';
    const blocked = isBlocked(u.name);
    const blockIndicator = blocked ? ' <span class="block-indicator" title="Blocked" style="font-size:0.65rem;">🚫</span>' : '';
    const dimStyle = u.online ? (blocked ? ' style="opacity:0.5;text-decoration:line-through"' : '') : (blocked ? ' style="opacity:0.5;text-decoration:line-through"' : ' style="opacity:0.5"');
    const botClass = isBot ? ' is-bot' : '';
    return `<div class="peer${isMe ? ' is-you' : ''}${botClass}" data-username="${escapedName}" data-pubkey="${escapedKey}"${dimStyle}>
      ${icon} ${escapedName}${badge}${isMe ? ' (you)' : ''}${deviceCount}${blockIndicator}
    </div>`;
  }

  let html = '';
  html += `<div style="font-size:0.6rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:0.3rem;">Online (${online.length})</div>`;
  html += online.map(renderUser).join('');
  if (offline.length > 0) {
    html += `<div style="height:1px;background:var(--border);margin:0.5rem 0;"></div>`;
    html += `<div style="font-size:0.6rem;text-transform:uppercase;color:var(--text-muted);letter-spacing:0.08em;margin-bottom:0.3rem;">Offline (${offline.length})</div>`;
    html += offline.map(renderUser).join('');
  }

  list.innerHTML = html;
  if (window.twemoji) twemoji.parse(list);
}

function updateChannelList(channels) {
  channelList = channels;
  renderChannelList();
}

function renderChannelList() {
  // Legacy hidden channel-list (kept for compatibility)
  const list = document.getElementById('channel-list');
  list.innerHTML = channelList.map(ch => {
    const isActive = ch.id === activeChannel && !activeDmPartner;
    const title = ch.description ? ` title="${esc(ch.description)}"` : '';
    const lock = ch.read_only ? ' 🔒' : '';
    const fedIcon = ch.federated ? '<span class="fed-icon" title="Federated">🌐</span>' : '';
    return `<div class="channel-item${isActive ? ' active' : ''}"${title} onclick="switchChannel('${esc(ch.id)}')">${esc(ch.name)}${lock}${fedIcon}</div>`;
  }).join('');
  if (window.twemoji) twemoji.parse(list);
  // Re-apply unread dots after re-rendering.
  if (typeof renderUnreadDots === 'function') renderUnreadDots();
  // Also update the server list in the Servers tab
  if (typeof renderServerList === 'function') renderServerList();
}

function switchChannel(channelId) {
  // Clear DM view if active.
  activeDmPartner = null;
  activeDmPartnerName = '';
  renderDmList();

  // Switch to Servers tab in sidebar.
  if (typeof switchSidebarTab === 'function') switchSidebarTab('servers', true);

  activeChannel = channelId;
  localStorage.setItem('humanity_channel', channelId);
  document.getElementById('messages').innerHTML = '';
  seenTimestamps.clear();
  // Clear local reaction state for old channel messages.
  Object.keys(messageReactions).forEach(k => delete messageReactions[k]);
  renderChannelList();
  updateChannelHeader();
  updateInputForChannel();
  // Load pins for the new channel.
  loadPinsForChannel(channelId);
  // Close pin list when switching channels.
  document.getElementById('pin-list').classList.remove('open');
  loadHistory().then(() => {
    // Load persisted reactions after messages are rendered.
    loadReactionsForChannel(channelId);
  });
}

function updateInputForChannel() {
  const ch = channelList.find(c => c.id === activeChannel);
  const input = document.getElementById('msg-input');
  const sendBtn = document.getElementById('send-btn');
  // Check if the current user is admin/mod.
  const myRole = (peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
  const isStaff = myRole === 'admin' || myRole === 'mod';

  if (ch && ch.read_only && !isStaff) {
    input.disabled = true;
    input.placeholder = 'This channel is read-only';
    sendBtn.disabled = true;
  } else {
    input.disabled = false;
    input.placeholder = 'Type a message…';
    sendBtn.disabled = false;
  }
}

function updateChannelHeader() {
  const header = document.getElementById('channel-header');
  const ch = channelList.find(c => c.id === activeChannel);
  if (ch) {
    const lock = ch.read_only ? ' 🔒' : '';
    header.innerHTML = `<span class="ch-name"># ${esc(ch.name)}${lock}</span>${ch.description ? `<span class="ch-desc">— ${esc(ch.description)}</span>` : ''}`;
    header.style.display = 'block';
    if (window.twemoji) twemoji.parse(header);
  } else {
    header.style.display = 'none';
  }
}

async function updateStats() {
  try {
    const resp = await fetch('/api/stats');
    const data = await resp.json();
    document.getElementById('stats').textContent =
      `${data.total_messages} msgs · ${data.connected_peers} online`;
  } catch (e) { /* ignore */ }
}

// ── Utilities ──
function setStatus(cls, text) {
  const el = document.getElementById('status');
  el.className = cls;
  document.getElementById('status-text').textContent = text;
}

function formatTime(ts) {
  const d = new Date(ts);
  const now = new Date();
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (d.toDateString() !== now.toDateString()) {
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' }) + ' ' + time;
  }
  return time;
}

function formatBody(text) {
  // Step 1: Extract code blocks BEFORE escaping (they get special treatment).
  const codeBlocks = [];
  const CODE_PLACEHOLDER = '\x00CB';
  let processedText = text.replace(/```(\w*)\n?([\s\S]*?)```/g, (match, lang, code) => {
    codeBlocks.push({ lang: lang || '', code: code.replace(/\n$/, '') });
    return CODE_PLACEHOLDER + (codeBlocks.length - 1) + CODE_PLACEHOLDER;
  });

  // Step 2: Escape HTML for the non-code parts.
  let safe = esc(processedText);

  // Step 3: File URLs → inline players/file cards.
  // Audio files
  safe = safe.replace(
    /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:mp3|ogg|wav)(?:\?[^\s<]*)?)/gi,
    '<audio controls preload="none" src="$1"></audio>'
  );
  // Video files
  safe = safe.replace(
    /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:mp4|webm)(?:\?[^\s<]*)?)/gi,
    '<video controls preload="none" src="$1" style="max-height:300px;"></video>'
  );
  // Document/archive files → file cards
  safe = safe.replace(
    /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(pdf|txt|md|json|zip|tar\.gz|gz)(?:\?[^\s<]*)?)/gi,
    (match, url, ext) => {
      const icon = ['zip','tar.gz','gz'].includes(ext.toLowerCase()) ? '📦' :
                   ['mp3','ogg','wav'].includes(ext.toLowerCase()) ? '🎵' : '📄';
      const fname = url.split('/').pop().split('?')[0];
      return `<div class="file-card"><span class="file-icon">${icon}</span><div class="file-info"><div class="file-name">${esc(fname)}</div></div><a href="${url}" target="_blank" rel="noopener" class="file-download">Download</a></div>`;
    }
  );

  // Image URLs → collapsed image placeholders.
  safe = safe.replace(
    /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:png|jpe?g|gif|webp)(?:\?[^\s<]*)?)/gi,
    '<span class="img-placeholder" data-img-url="$1">🖼️ Image (click to load)</span>'
  );

  // Other URLs → clickable links.
  safe = safe.replace(
    /(?<!["=])(https?:\/\/[^\s<]+)(?![^<]*<\/a>|[^<]*<\/span>|[^<]*<\/audio>|[^<]*<\/video>)/g,
    '<a href="$1" target="_blank" rel="noopener" style="color:var(--accent)">$1</a>'
  );

  // Step 4: Markdown formatting.
  // __bold__ or **bold**
  safe = safe.replace(/__(.+?)__/g, '<strong>$1</strong>');
  safe = safe.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  // *italic* or _italic_ (single, not inside words)
  safe = safe.replace(/\*(.+?)\*/g, '<em>$1</em>');
  // ~~strikethrough~~
  safe = safe.replace(/~~(.+?)~~/g, '<del>$1</del>');
  // `inline code` (but not inside code blocks)
  safe = safe.replace(/`([^`\n]+)`/g, '<code>$1</code>');

  // @mentions — highlight usernames.
  safe = safe.replace(/@([A-Za-z0-9_-]+)/g, (match, name) => {
    const isMe = myName && name.toLowerCase() === myName.toLowerCase();
    const cls = isMe ? 'mention mention-me' : 'mention';
    return `<span class="${cls}">@${esc(name)}</span>`;
  });

  // Step 5: Process line-level formatting (quotes, lists).
  const lines = safe.split('\n');
  let result = [];
  let quoteLines = [];
  let listItems = [];

  function flushQuote() {
    if (quoteLines.length > 0) {
      const full = quoteLines.join('<br>');
      const plainLen = quoteLines.join(' ').length;
      if (plainLen > 120 || quoteLines.length > 2) {
        const previewText = quoteLines[0].substring(0, 80) + (plainLen > 80 ? '…' : '');
        result.push(
          '<div class="quote-block" onclick="this.classList.toggle(\'expanded\')">' +
            '<span class="quote-preview">' + previewText +
              '<span class="quote-expand">▸ show more</span>' +
            '</span>' +
            '<span class="quote-full">' + full +
              '<br><span class="quote-expand">▴ show less</span>' +
            '</span>' +
          '</div>'
        );
      } else {
        result.push('<div class="quote-block">' + full + '</div>');
      }
      quoteLines = [];
    }
  }

  function flushList() {
    if (listItems.length > 0) {
      result.push('<ul class="md-list">' + listItems.map(li => '<li>' + li + '</li>').join('') + '</ul>');
      listItems = [];
    }
  }

  for (const line of lines) {
    if (line.startsWith('&gt; ')) {
      flushList();
      quoteLines.push(line.substring(5));
    } else if (/^[-*] /.test(line)) {
      flushQuote();
      listItems.push(line.substring(2));
    } else {
      flushQuote();
      flushList();
      result.push(line);
    }
  }
  flushQuote();
  flushList();

  safe = result.join('\n');

  // Step 6: Restore code blocks with styled rendering.
  safe = safe.replace(new RegExp(CODE_PLACEHOLDER.replace(/\0/g, '\\0') + '(\\d+)' + CODE_PLACEHOLDER.replace(/\0/g, '\\0'), 'g'), (match, idx) => {
    const block = codeBlocks[parseInt(idx)];
    if (!block) return match;
    const escapedCode = esc(block.code);
    const langLabel = block.lang ? `<span class="code-lang">${esc(block.lang)}</span>` : '';
    return `<div class="code-block-wrapper">${langLabel}<button class="code-copy" onclick="navigator.clipboard.writeText(this.parentElement.querySelector('code').textContent);this.textContent='✓ Copied';setTimeout(()=>this.textContent='📋 Copy',1500)">📋 Copy</button><pre><code>${escapedCode}</code></pre></div>`;
  });

  return safe;
}

/**
 * Format todo-channel messages from Heron bot.
 * Detects [ACTIVE], [COMPLETED], [INACTIVE] section markers and renders
 * them as collapsible <details> elements with color-coded backgrounds.
 */
function formatTodoMessage(text) {
  const sectionRegex = /\[(ACTIVE|COMPLETED|INACTIVE)\]/g;
  const parts = [];
  let lastIndex = 0;
  let match;
  const matches = [];

  while ((match = sectionRegex.exec(text)) !== null) {
    matches.push({ type: match[1], index: match.index, end: match.index + match[0].length });
  }

  if (matches.length === 0) return null; // Not a todo message

  // Text before first section
  const preamble = text.substring(0, matches[0].index).trim();
  if (preamble) {
    parts.push('<div style="margin-bottom:0.4rem;">' + esc(preamble) + '</div>');
  }

  for (let i = 0; i < matches.length; i++) {
    const m = matches[i];
    const nextStart = (i + 1 < matches.length) ? matches[i + 1].index : text.length;
    const sectionContent = text.substring(m.end, nextStart).trim();
    const cssClass = 'todo-' + m.type.toLowerCase();
    const label = m.type.charAt(0) + m.type.slice(1).toLowerCase();
    const icon = m.type === 'ACTIVE' ? '🔵' : m.type === 'COMPLETED' ? '✅' : '🔴';
    parts.push(
      '<details class="todo-section ' + cssClass + '" open>' +
        '<summary>' + icon + ' ' + esc(label) + '</summary>' +
        '<div class="todo-items">' + esc(sectionContent) + '</div>' +
      '</details>'
    );
  }

  return parts.join('');
}

function shortKey(hex) {
  if (!hex) return '???';
  return hex.substring(0, 8) + '…';
}

function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML.replace(/'/g, '&#39;').replace(/"/g, '&quot;');
}

// ── Identicon Generator ──
// Creates a 5x5 symmetric pixel art from a hex key string.
// WHY: Visual identity at a glance, unique per key, no upload needed.
const identiconCache = {};
function generateIdenticon(hexKey, size) {
  size = size || 24;
  const cacheKey = hexKey + ':' + size;
  if (identiconCache[cacheKey]) return identiconCache[cacheKey];

  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');

  // Parse first 15 bytes from hex for the 5x5 grid (only need half — mirror for symmetry).
  const bytes = [];
  for (let i = 0; i < 30 && i < hexKey.length; i += 2) {
    bytes.push(parseInt(hexKey.substr(i, 2), 16) || 0);
  }

  // Color from bytes 0-2 (ensure visible on dark bg by keeping values 80-220).
  const r = 80 + (bytes[0] % 140);
  const g = 80 + (bytes[1] % 140);
  const b = 80 + (bytes[2] % 140);
  const color = `rgb(${r},${g},${b})`;
  const bg = '#1a1a1a';

  const cellSize = size / 5;
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = color;

  // 5x5 grid, horizontally symmetric (columns 0-2 mirror to 4-2).
  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 3; col++) {
      const byteIdx = 3 + row * 3 + col;
      if (byteIdx < bytes.length && bytes[byteIdx] % 2 === 0) {
        ctx.fillRect(col * cellSize, row * cellSize, cellSize, cellSize);
        // Mirror: column 4-col.
        if (col < 2) {
          ctx.fillRect((4 - col) * cellSize, row * cellSize, cellSize, cellSize);
        }
      }
    }
  }

  const dataUrl = canvas.toDataURL();
  identiconCache[cacheKey] = dataUrl;
  return dataUrl;
}

// ── Emoji Reactions ──
const REACTION_EMOJIS = ['👍', '❤️', '😂', '😮', '😢', '🎉', '🔥', '👀'];
// Track reactions: key = "fromKey:timestamp", value = { emoji: Set(reactorKeys) }
const messageReactions = {};

function showReactionPicker(btn, targetFrom, targetTs, msgEl) {
  // Close any existing picker.
  document.querySelectorAll('.reaction-picker').forEach(p => p.remove());

  const picker = document.createElement('div');
  picker.className = 'reaction-picker';
  picker.style.cssText = 'position:absolute;top:-2rem;right:0;background:var(--bg-secondary);border:1px solid var(--border);border-radius:6px;padding:0.2rem;display:flex;gap:0.15rem;z-index:20;';
  REACTION_EMOJIS.forEach(emoji => {
    const btn = document.createElement('span');
    btn.textContent = emoji;
    btn.style.cssText = 'cursor:pointer;padding:0.15rem 0.25rem;border-radius:3px;font-size:0.9rem;';
    btn.onmouseover = () => btn.style.background = 'var(--bg-hover)';
    btn.onmouseout = () => btn.style.background = '';
    btn.onclick = (e) => {
      e.stopPropagation();
      sendReaction(targetFrom, targetTs, emoji);
      picker.remove();
    };
    picker.appendChild(btn);
  });
  if (window.twemoji) twemoji.parse(picker);

  msgEl.style.position = 'relative';
  msgEl.appendChild(picker);
  // Close picker when clicking elsewhere.
  setTimeout(() => {
    document.addEventListener('click', function closePicker() {
      picker.remove();
      document.removeEventListener('click', closePicker);
    }, { once: true });
  }, 0);
}

function sendReaction(targetFrom, targetTs, emoji) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify({
    type: 'reaction',
    target_from: targetFrom,
    target_timestamp: Number(targetTs),
    emoji: emoji,
    from: myKey,
    from_name: myName,
    channel: activeChannel,
  }));
  // Apply locally immediately.
  applyReaction(targetFrom, Number(targetTs), emoji, myKey, myName);
}

function applyReaction(targetFrom, targetTs, emoji, reactorKey, reactorName) {
  const rKey = targetFrom + ':' + targetTs;
  if (!messageReactions[rKey]) messageReactions[rKey] = {};
  if (!messageReactions[rKey][emoji]) messageReactions[rKey][emoji] = new Set();

  const set = messageReactions[rKey][emoji];
  if (set.has(reactorKey)) {
    set.delete(reactorKey); // Toggle off.
    if (set.size === 0) delete messageReactions[rKey][emoji];
  } else {
    set.add(reactorKey); // Toggle on.
  }
  renderReactions(targetFrom, targetTs);
}

function renderReactions(targetFrom, targetTs) {
  const rKey = targetFrom + ':' + targetTs;
  const reactions = messageReactions[rKey] || {};
  // Find the reactions div in the DOM.
  const msgEl = document.querySelector(`.reactions[data-from="${targetFrom}"][data-ts="${targetTs}"]`);
  if (!msgEl) return;

  msgEl.innerHTML = Object.entries(reactions).map(([emoji, users]) => {
    const isMine = users.has(myKey);
    return `<span class="reaction-badge${isMine ? ' mine' : ''}" data-target-from="${esc(targetFrom)}" data-target-ts="${targetTs}" data-emoji="${esc(emoji)}">${esc(emoji)} <span class="count">${users.size}</span></span>`;
  }).join('');
  if (window.twemoji) twemoji.parse(msgEl);
}

// Apply a reaction from sync (add-only, no toggle).
function applyReactionSync(targetFrom, targetTs, emoji, reactorKey) {
  const rKey = targetFrom + ':' + targetTs;
  if (!messageReactions[rKey]) messageReactions[rKey] = {};
  if (!messageReactions[rKey][emoji]) messageReactions[rKey][emoji] = new Set();
  messageReactions[rKey][emoji].add(reactorKey);
  renderReactions(targetFrom, targetTs);
}

// Load reactions from the API for the current channel and apply them.
async function loadReactionsForChannel(channelId) {
  try {
    const resp = await fetch(`/api/reactions?channel=${encodeURIComponent(channelId)}&limit=500`);
    const data = await resp.json();
    if (data.reactions && data.reactions.length > 0) {
      for (const r of data.reactions) {
        applyReactionSync(r.target_from, r.target_timestamp, r.emoji, r.reactor_key);
      }
    }
  } catch (e) {
    console.warn('Failed to load reactions:', e);
  }
}

// ── Message Editing ──
function startEditMode(msgEl, originalBody, fromKey, timestamp) {
  const bodyEl = msgEl.querySelector('.body');
  if (!bodyEl || msgEl.querySelector('.edit-area')) return; // Already editing.

  // Strip quote lines from the original body for editing (quotes are read-only context).
  const lines = originalBody.split('\n');
  const editableLines = [];
  let pastQuotes = false;
  for (const line of lines) {
    if (!pastQuotes && line.startsWith('> ')) continue;
    pastQuotes = true;
    editableLines.push(line);
  }
  const editableText = editableLines.join('\n').trim() || originalBody;

  const savedHtml = bodyEl.innerHTML;
  bodyEl.innerHTML = '';

  const editArea = document.createElement('div');
  editArea.className = 'edit-area';

  const textarea = document.createElement('textarea');
  textarea.value = editableText;
  textarea.rows = Math.min(5, editableText.split('\n').length + 1);

  const buttons = document.createElement('div');
  buttons.className = 'edit-buttons';

  const saveBtn = document.createElement('button');
  saveBtn.className = 'edit-save';
  saveBtn.textContent = 'Save';
  saveBtn.onclick = (e) => {
    e.stopPropagation();
    const newContent = textarea.value.trim();
    if (!newContent || newContent.length > getMaxMsgLength()) return;
    // Send edit via WebSocket.
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'edit',
        from: myKey,
        timestamp: Number(timestamp),
        new_content: newContent,
        channel: activeChannel,
      }));
    }
    // Apply locally immediately.
    bodyEl.innerHTML = formatBody(newContent);
    if (!bodyEl.querySelector('.edited-marker')) {
      const marker = document.createElement('span');
      marker.className = 'edited-marker';
      marker.textContent = '(edited)';
      bodyEl.appendChild(marker);
    }
    if (window.twemoji) twemoji.parse(bodyEl);
  };

  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'edit-cancel';
  cancelBtn.textContent = 'Cancel';
  cancelBtn.onclick = (e) => {
    e.stopPropagation();
    bodyEl.innerHTML = savedHtml;
    if (window.twemoji) twemoji.parse(bodyEl);
  };

  // Escape to cancel, Ctrl+Enter to save.
  textarea.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { cancelBtn.click(); }
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { saveBtn.click(); }
  });

  buttons.appendChild(cancelBtn);
  buttons.appendChild(saveBtn);
  editArea.appendChild(textarea);
  editArea.appendChild(buttons);
  bodyEl.appendChild(editArea);
  textarea.focus();
  textarea.setSelectionRange(textarea.value.length, textarea.value.length);
}

function applyEditToDOM(fromKey, timestamp, newContent) {
  const msgEl = document.querySelector(`.message[data-from="${fromKey}"][data-timestamp="${timestamp}"]`);
  if (!msgEl) return;
  const bodyEl = msgEl.querySelector('.body');
  if (!bodyEl) return;
  bodyEl.innerHTML = formatBody(newContent);
  // Add (edited) marker if not present.
  if (!bodyEl.querySelector('.edited-marker')) {
    const marker = document.createElement('span');
    marker.className = 'edited-marker';
    marker.textContent = '(edited)';
    bodyEl.appendChild(marker);
  }
  if (window.twemoji) twemoji.parse(bodyEl);
}

// ── Pin System ──
let currentPins = []; // Array of pin objects for the active channel.

function togglePinList() {
  const list = document.getElementById('pin-list');
  list.classList.toggle('open');
}

function renderPinBar() {
  const bar = document.getElementById('pin-bar');
  const countEl = document.getElementById('pin-count');
  const serverSection = document.getElementById('server-pins-section');
  const serverContainer = document.getElementById('server-pins');
  const mySection = document.getElementById('my-pins-section');
  const myContainer = document.getElementById('my-pins');
  const myPins = getMyPins();
  const total = currentPins.length + myPins.length;

  if (total === 0) {
    bar.style.display = 'none';
    document.getElementById('pin-list').classList.remove('open');
    serverSection.style.display = 'none';
    mySection.style.display = 'none';
    return;
  }

  bar.style.display = 'block';
  countEl.textContent = total;

  // Determine if user is admin/mod for showing server unpin buttons.
  const myRole = (peerData[myKey] && peerData[myKey].role) ? peerData[myKey].role : '';
  const isStaff = myRole === 'admin' || myRole === 'mod';

  // Server pins.
  if (currentPins.length > 0) {
    serverSection.style.display = 'block';
    let html = '';
    currentPins.forEach((pin, i) => {
      const time = formatTime(pin.original_timestamp);
      const unpinBtn = isStaff
        ? `<button class="pin-unpin" onclick="event.stopPropagation();unpinServer(${i + 1})" title="Unpin">✕</button>`
        : '';
      html += `<div class="pin-card" onclick="this.classList.toggle('expanded')">${unpinBtn}
        <div class="pin-card-author">${esc(pin.from_name)}</div>
        <div class="pin-card-body">${esc(pin.content)}</div>
        <div class="pin-expand-hint"><span class="hint-expand">▸ Click to expand</span><span class="hint-collapse">▴ Click to collapse</span></div>
        <div class="pin-card-meta">Pinned by ${esc(pin.pinned_by)} · ${time}</div>
      </div>`;
    });
    serverContainer.innerHTML = html;
  } else {
    serverSection.style.display = 'none';
    serverContainer.innerHTML = '';
  }

  // Personal pins.
  if (myPins.length > 0) {
    mySection.style.display = 'block';
    let html = '';
    myPins.forEach((pin, i) => {
      const time = formatTime(pin.original_timestamp);
      html += `<div class="pin-card" onclick="this.classList.toggle('expanded')"><button class="pin-unpin" onclick="event.stopPropagation();removeMyPin(${i})" title="Remove">✕</button>
        <div class="pin-card-author">${esc(pin.from_name)}</div>
        <div class="pin-card-body">${esc(pin.content)}</div>
        <div class="pin-expand-hint"><span class="hint-expand">▸ Click to expand</span><span class="hint-collapse">▴ Click to collapse</span></div>
        <div class="pin-card-meta">${time}</div>
      </div>`;
    });
    myContainer.innerHTML = html;
  } else {
    mySection.style.display = 'none';
    myContainer.innerHTML = '';
  }

  if (window.twemoji) { twemoji.parse(bar); twemoji.parse(serverContainer); twemoji.parse(myContainer); }
}

function unpinServer(index) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'chat', from: myKey, from_name: myName, content: '/unpin ' + index, timestamp: Date.now(), channel: activeChannel }));
  }
}

function removeMyPin(index) {
  let pins = getMyPins();
  pins.splice(index, 1);
  setMyPins(pins);
  renderPinBar();
}

async function loadPinsForChannel(channelId) {
  try {
    const resp = await fetch(`/api/pins?channel=${encodeURIComponent(channelId)}`);
    const data = await resp.json();
    currentPins = data.pins || [];
    renderPinBar();
  } catch (e) {
    console.warn('Failed to load pins:', e);
    currentPins = [];
    renderPinBar();
  }
}

function pinMessageFromUI(fromKey, fromName, content, timestamp) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      type: 'pin_request',
      from_key: fromKey,
      from_name: fromName,
      content: content,
      timestamp: Number(timestamp),
      channel: activeChannel,
    }));
  }
}

// ── Personal Pins (client-side, localStorage) ──
function getMyPins() {
  try { return JSON.parse(localStorage.getItem('my_pins_' + activeChannel) || '[]'); }
  catch { return []; }
}
function setMyPins(pins) {
  localStorage.setItem('my_pins_' + activeChannel, JSON.stringify(pins));
}
function toggleMyPin(fromKey, fromName, content, timestamp) {
  let pins = getMyPins();
  const idx = pins.findIndex(p => p.from_key === fromKey && p.original_timestamp === Number(timestamp));
  if (idx >= 0) {
    pins.splice(idx, 1);
  } else {
    pins.push({ from_key: fromKey, from_name: fromName, content: content, original_timestamp: Number(timestamp), pinned_at: Date.now() });
  }
  setMyPins(pins);
  renderPinBar();
}

// ── Typing Indicator ──
let typingTimers = {};   // key → timeout id
let typingNames = {};    // key → display name
let lastTypingSent = 0;  // throttle outbound typing events

function showTypingIndicator(name) {
  // Suppress typing indicators from blocked users.
  if (isBlocked(name)) return;
  // Track who is typing, clear after 3 seconds of no updates.
  const key = name;
  typingNames[key] = name;
  clearTimeout(typingTimers[key]);
  typingTimers[key] = setTimeout(() => {
    delete typingNames[key];
    delete typingTimers[key];
    renderTypingIndicator();
  }, 3000);
  renderTypingIndicator();
}

function renderTypingIndicator() {
  const el = document.getElementById('typing-indicator');
  const names = Object.values(typingNames);
  if (names.length === 0) {
    el.textContent = '';
  } else if (names.length === 1) {
    el.textContent = names[0] + ' is typing…';
  } else if (names.length === 2) {
    el.textContent = names[0] + ' and ' + names[1] + ' are typing…';
  } else {
    el.textContent = 'Several people are typing…';
  }
}

// ── Image handling ──
function loadImage(placeholder, url) {
  // Replace placeholder with loaded image. Click image to collapse, right-click/long-press for full size.
  const img = document.createElement('img');
  img.className = 'img-loaded';
  img.src = url;
  img.alt = 'Image';
  img.title = 'Click to collapse · Right-click to open full size';
  img.onclick = (e) => {
    e.preventDefault();
    // Collapse back to placeholder.
    const newPlaceholder = document.createElement('span');
    newPlaceholder.className = 'img-placeholder';
    newPlaceholder.textContent = '🖼️ Image (click to load)';
    newPlaceholder.onclick = () => loadImage(newPlaceholder, url);
    img.replaceWith(newPlaceholder);
    if (window.twemoji) twemoji.parse(newPlaceholder);
  };
  // Middle-click or Ctrl+click opens in new tab.
  img.onauxclick = (e) => { if (e.button === 1) window.open(url, '_blank'); };
  placeholder.replaceWith(img);
}

async function uploadImage(file) {
  const indicator = document.getElementById('upload-indicator');
  indicator.textContent = `Uploading ${file.name}…`;
  indicator.style.display = 'block';

  try {
    const formData = new FormData();
    formData.append('file', file);

    const uploadUrl = myUploadToken ? `/api/upload?token=${encodeURIComponent(myUploadToken)}` : (myKey ? `/api/upload?key=${encodeURIComponent(myKey)}` : '/api/upload');
    const resp = await fetch(uploadUrl, { method: 'POST', body: formData });
    if (!resp.ok) {
      const text = await resp.text();
      addSystemMessage(`Upload failed: ${text}`);
      return null;
    }

    const data = await resp.json();
    return data.url;
  } catch (e) {
    addSystemMessage(`Upload failed: ${e.message}`);
    return null;
  } finally {
    indicator.style.display = 'none';
  }
}

// Handle file attachment (📎 button).
async function handleFileAttachment(event) {
  const file = event.target.files[0];
  if (!file) return;
  event.target.value = ''; // Reset for re-selection

  const url = await uploadImage(file); // Reuse existing upload function
  if (url && ws && ws.readyState === WebSocket.OPEN) {
    const timestamp = Date.now();
    const content = url;
    let signature = null;
    if (myIdentity && myIdentity.canSign) {
      signature = await signMessage(myIdentity.privateKey, content, timestamp);
    }
    const msg = { type: 'chat', from: myKey, from_name: myName, content, timestamp, channel: activeChannel };
    if (signature) msg.signature = signature;
    ws.send(JSON.stringify(msg));
    const key = myKey + ':' + timestamp;
    seenTimestamps.add(key);
    addChatMessage(myName, content, timestamp, myKey, false, !!signature);
  }
}

// Paste image from clipboard → upload and send.
document.getElementById('msg-input').addEventListener('paste', async (e) => {
  const items = e.clipboardData?.items;
  if (!items) return;

  for (const item of items) {
    if (item.type.startsWith('image/')) {
      e.preventDefault();
      const file = item.getAsFile();
      if (!file) return;

      const url = await uploadImage(file);
      if (url && ws && ws.readyState === WebSocket.OPEN) {
        const timestamp = Date.now();
        const content = url;
        let signature = null;
        if (myIdentity && myIdentity.canSign) {
          signature = await signMessage(myIdentity.privateKey, content, timestamp);
        }
        const msg = { type: 'chat', from: myKey, from_name: myName, content, timestamp };
        if (signature) msg.signature = signature;
        ws.send(JSON.stringify(msg));
        const key = myKey + ':' + timestamp;
        seenTimestamps.add(key);
        addChatMessage(myName, content, timestamp, myKey, false, !!signature);
      }
      return;
    }
  }
});

// Drag and drop image → upload and send.
const chatArea = document.getElementById('chat-area');
chatArea.addEventListener('dragover', (e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; });
chatArea.addEventListener('drop', async (e) => {
  e.preventDefault();
  const files = e.dataTransfer?.files;
  if (!files) return;

  for (const file of files) {
    if (file.type.startsWith('image/')) {
      const url = await uploadImage(file);
      if (url && ws && ws.readyState === WebSocket.OPEN) {
        const timestamp = Date.now();
        const content = url;
        let signature = null;
        if (myIdentity && myIdentity.canSign) {
          signature = await signMessage(myIdentity.privateKey, content, timestamp);
        }
        const msg = { type: 'chat', from: myKey, from_name: myName, content, timestamp };
        if (signature) msg.signature = signature;
        ws.send(JSON.stringify(msg));
        const key = myKey + ':' + timestamp;
        seenTimestamps.add(key);
        addChatMessage(myName, content, timestamp, myKey, false, !!signature);
      }
    }
  }
});

// ── Thread panel functions ──
let currentThread = null; // { from, timestamp, author, body }

function openThreadPanel(fromKey, timestamp, author, body) {
  currentThread = { from: fromKey, timestamp: Number(timestamp), author, body };
  const panel = document.getElementById('thread-panel');
  panel.classList.add('open');
  // Show parent message.
  const messagesDiv = document.getElementById('thread-panel-messages');
  messagesDiv.innerHTML = `<div class="thread-msg thread-parent">
    <span class="thread-msg-author">${esc(author)}</span>
    <span class="thread-msg-time">${formatTime(timestamp)}</span>
    <div class="thread-msg-body">${formatBody(body)}</div>
  </div>
  <div style="font-size:0.72rem;color:var(--text-muted);margin-bottom:0.5rem;">Loading replies...</div>`;
  // Request thread from server.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'thread_request', from: fromKey, timestamp: Number(timestamp) }));
  }
  document.getElementById('thread-input').focus();
}

function closeThreadPanel() {
  currentThread = null;
  document.getElementById('thread-panel').classList.remove('open');
}

function renderThreadMessages(messages) {
  const messagesDiv = document.getElementById('thread-panel-messages');
  if (!currentThread) return;
  // Keep parent, rebuild replies.
  const parentHtml = `<div class="thread-msg thread-parent">
    <span class="thread-msg-author">${esc(currentThread.author)}</span>
    <span class="thread-msg-time">${formatTime(currentThread.timestamp)}</span>
    <div class="thread-msg-body">${formatBody(currentThread.body)}</div>
  </div>`;
  let repliesHtml = '';
  if (messages.length === 0) {
    repliesHtml = '<div style="font-size:0.8rem;color:var(--text-muted);padding:0.5rem;">No replies yet. Be the first!</div>';
  } else {
    for (const m of messages) {
      repliesHtml += `<div class="thread-msg">
        <span class="thread-msg-author">${esc(m.from_name || 'Unknown')}</span>
        <span class="thread-msg-time">${formatTime(m.timestamp)}</span>
        <div class="thread-msg-body">${formatBody(m.content)}</div>
      </div>`;
    }
  }
  messagesDiv.innerHTML = parentHtml + repliesHtml;
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
  if (window.twemoji) twemoji.parse(messagesDiv);
}

async function sendThreadReply() {
  const input = document.getElementById('thread-input');
  const content = input.value.trim();
  if (!content || !ws || ws.readyState !== WebSocket.OPEN || !currentThread) return;

  const timestamp = Date.now();
  let signature = null;
  if (myIdentity && myIdentity.canSign) {
    signature = await signMessage(myIdentity.privateKey, content, timestamp);
  }

  const msg = {
    type: 'chat',
    from: myKey,
    from_name: myName,
    content: content,
    timestamp: timestamp,
    channel: activeChannel,
    reply_to: {
      from: currentThread.from,
      from_name: currentThread.author,
      content: currentThread.body,
      timestamp: currentThread.timestamp,
    },
  };
  if (signature) msg.signature = signature;
  ws.send(JSON.stringify(msg));

  // Add to thread panel immediately.
  const messagesDiv = document.getElementById('thread-panel-messages');
  messagesDiv.innerHTML += `<div class="thread-msg">
    <span class="thread-msg-author">${esc(myName)}</span>
    <span class="thread-msg-time">${formatTime(timestamp)}</span>
    <div class="thread-msg-body">${formatBody(content)}</div>
  </div>`;
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
  if (window.twemoji) twemoji.parse(messagesDiv);

  // Also add to main chat.
  const key = myKey + ':' + timestamp;
  seenTimestamps.add(key);
  addChatMessage(myName, content, timestamp, myKey, false, !!signature, msg.reply_to, null);

  input.value = '';
}

// Update thread badge count when a new reply arrives.
function updateThreadBadge(parentFrom, parentTimestamp) {
  const parentEl = document.querySelector(`.message[data-from="${parentFrom}"][data-timestamp="${parentTimestamp}"]`);
  if (!parentEl) return;
  let badge = parentEl.querySelector('.thread-badge');
  if (badge) {
    // Increment existing count.
    const text = badge.textContent;
    const match = text.match(/(\d+)/);
    const count = match ? parseInt(match[1]) + 1 : 1;
    badge.textContent = `💬 ${count} ${count === 1 ? 'reply' : 'replies'}`;
  } else {
    // Create new badge.
    badge = document.createElement('div');
    badge.className = 'thread-badge';
    badge.dataset.threadFrom = parentFrom;
    badge.dataset.threadTs = parentTimestamp;
    badge.textContent = '💬 1 reply';
    badge.addEventListener('click', (e) => {
      e.stopPropagation();
      // Find parent message content.
      const bodyEl = parentEl.querySelector('.body');
      const authorEl = parentEl.querySelector('.author');
      openThreadPanel(parentFrom, parentTimestamp, authorEl ? authorEl.textContent : 'Unknown', bodyEl ? bodyEl.textContent : '');
    });
    const actions = parentEl.querySelector('.msg-actions');
    if (actions) parentEl.insertBefore(badge, actions);
    else parentEl.appendChild(badge);
  }
}

// Handle thread_input Enter key.
document.getElementById('thread-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendThreadReply();
  }
});

function sendTypingIndicator() {
  // Throttle: send at most once every 2 seconds.
  const now = Date.now();
  if (now - lastTypingSent < 2000) return;
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  lastTypingSent = now;
  ws.send(JSON.stringify({ type: 'typing', from: myKey, from_name: myName }));
}

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

  const isBot = publicKey && publicKey.startsWith('bot_');
  let html = '';
  if (isBot) {
    // Bot-specific context menu
    html += `<div class="ctx-item" style="font-weight:bold;color:var(--accent);pointer-events:none">🤖 ${esc(name)}</div>`;
    html += '<div class="ctx-sep"></div>';
    html += `<div class="ctx-item" onclick="botCommand('status')">📊 Status</div>`;
    html += `<div class="ctx-item" onclick="botCommand('summary')">📝 Today's Summary</div>`;
    html += `<div class="ctx-item" onclick="botCommand('tasks')">📋 Current Tasks</div>`;
    html += `<div class="ctx-item" onclick="botCommand('help')">❓ Help</div>`;
  } else {
    html += `<div class="ctx-item" onclick="viewProfileFromCtx()">👤 View Profile</div>`;
    html += `<div class="ctx-item" onclick="copyPublicKey()">📋 Copy public key</div>`;
    if (name !== myName) {
      html += `<div class="ctx-item" onclick="dmFromCtx()">💬 Direct Message</div>`;
      // Follow/unfollow toggle
      if (typeof myFollowing !== 'undefined' && myFollowing.has(publicKey)) {
        html += `<div class="ctx-item" onclick="followFromCtx(false)">❌ Unfollow</div>`;
      } else {
        html += `<div class="ctx-item" onclick="followFromCtx(true)">👁️ Follow</div>`;
      }
      // Block/unblock toggle.
      if (isBlocked(name)) {
        html += `<div class="ctx-item" onclick="unblockFromCtx()">✅ Unblock</div>`;
      } else {
        html += `<div class="ctx-item" onclick="blockFromCtx()">🚫 Block</div>`;
      }
      html += `<div class="ctx-item" onclick="reportUser()">🚩 Report</div>`;
      html += '<div class="ctx-sep"></div>';
      html += `<div class="ctx-item" onclick="ctxCommand('/kick')">👢 Kick</div>`;
      html += `<div class="ctx-item" onclick="ctxCommand('/mute')">🔇 Mute</div>`;
      html += `<div class="ctx-item" onclick="ctxCommand('/ban')">🚫 Ban</div>`;
      html += `<div class="ctx-item" onclick="ctxCommand('/verify')">✦ Verify</div>`;
    }
  }

  ctxMenu.innerHTML = html;
  if (window.twemoji) twemoji.parse(ctxMenu);

  // Position near click.
  const x = Math.min(e.clientX, window.innerWidth - 170);
  const y = Math.min(e.clientY, window.innerHeight - 200);
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
  if (!ctxMenuTarget || !ws || ws.readyState !== WebSocket.OPEN) return;
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

// ── Profile System ──
let profileCache = {}; // name (lowercase) → { bio, socials }
let lastProfileUpdateSent = 0;
let pendingProfileView = null; // name we're waiting for profile_data on

// Local storage for offline profile editing.
function saveProfileLocal(bio, socials) {
  localStorage.setItem('humanity_profile', JSON.stringify({ bio, socials }));
}
function loadProfileLocal() {
  try {
    return JSON.parse(localStorage.getItem('humanity_profile') || '{}');
  } catch { return {}; }
}

// ── Edit Profile Modal ──
function openEditProfileModal() {
  const overlay = document.getElementById('edit-profile-overlay');
  const local = loadProfileLocal();
  const socials = local.socials || {};
  document.getElementById('profile-bio').value = local.bio || '';
  document.getElementById('profile-website').value = socials.website || '';
  document.getElementById('profile-discord').value = socials.discord || '';
  document.getElementById('profile-twitter').value = socials.twitter || '';
  document.getElementById('profile-youtube').value = socials.youtube || '';
  document.getElementById('profile-github').value = socials.github || '';
  updateBioCounter();
  overlay.classList.add('open');
}

function closeEditProfileModal(e) {
  if (e.target === document.getElementById('edit-profile-overlay')) {
    closeEditProfileOverlay();
  }
}
function closeEditProfileOverlay() {
  document.getElementById('edit-profile-overlay').classList.remove('open');
}

function updateBioCounter() {
  const bio = document.getElementById('profile-bio').value;
  const counter = document.getElementById('bio-counter');
  counter.textContent = bio.length + ' / 280';
  counter.className = 'bio-counter' + (bio.length > 280 ? ' over' : bio.length > 240 ? ' warn' : '');
}

document.getElementById('profile-bio').addEventListener('input', updateBioCounter);

function saveProfile() {
  const bio = document.getElementById('profile-bio').value.trim().substring(0, 280);
  const socials = {
    website: document.getElementById('profile-website').value.trim().substring(0, 200),
    discord: document.getElementById('profile-discord').value.trim().substring(0, 100),
    twitter: document.getElementById('profile-twitter').value.trim().substring(0, 100),
    youtube: document.getElementById('profile-youtube').value.trim().substring(0, 200),
    github: document.getElementById('profile-github').value.trim().substring(0, 200),
  };

  // Remove empty fields.
  const cleanSocials = {};
  for (const [k, v] of Object.entries(socials)) {
    if (v) cleanSocials[k] = v;
  }

  saveProfileLocal(bio, cleanSocials);

  // Send to server if connected.
  if (ws && ws.readyState === WebSocket.OPEN) {
    const now = Date.now();
    if (now - lastProfileUpdateSent < 30000) {
      addSystemMessage('⏳ Please wait 30 seconds between profile updates.');
    } else {
      lastProfileUpdateSent = now;
      ws.send(JSON.stringify({
        type: 'profile_update',
        bio: bio,
        socials: JSON.stringify(cleanSocials),
      }));
      addSystemMessage('Profile saved.');
    }
  } else {
    addSystemMessage('Profile saved locally. It will sync when you connect.');
  }

  closeEditProfileOverlay();
}

// Send stored profile on connect.
function syncProfileOnConnect() {
  const local = loadProfileLocal();
  if (local.bio || (local.socials && Object.keys(local.socials).length > 0)) {
    const socialsStr = JSON.stringify(local.socials || {});
    ws.send(JSON.stringify({
      type: 'profile_update',
      bio: local.bio || '',
      socials: socialsStr,
    }));
    lastProfileUpdateSent = Date.now();
  }
}

// ── View Profile Modal ──
function requestViewProfile(name, publicKey) {
  pendingProfileView = { name, publicKey };
  // Check cache first.
  const cached = profileCache[name.toLowerCase()];
  if (cached) {
    showViewProfileCard(name, publicKey, cached.bio, cached.socials);
    return;
  }
  // Request from server.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'profile_request', name: name }));
    // Show loading state.
    document.getElementById('view-profile-content').innerHTML =
      '<div style="color:var(--text-muted);font-style:italic;">Loading profile…</div>';
    document.getElementById('view-profile-overlay').classList.add('open');
  }
}

function showViewProfileCard(name, publicKey, bio, socialsStr) {
  let socials = {};
  try { socials = JSON.parse(socialsStr || '{}'); } catch {}

  const isBot = publicKey && publicKey.startsWith('bot_');
  const identiconSrc = !isBot && publicKey
    ? generateIdenticon(publicKey, 64) : '';
  const identiconHtml = isBot
    ? '<span class="identicon-large" style="font-size:48px;line-height:64px;display:inline-block;width:64px;text-align:center;">🤖</span>'
    : (identiconSrc ? '<img src="' + identiconSrc + '" class="identicon-large" alt="">' : '');

  // Look up role.
  const peerRole = (peerData[publicKey] && peerData[publicKey].role) ? peerData[publicKey].role : '';
  const badge = roleBadge(peerRole);

  let html = '<div class="profile-card-header">';
  html += identiconHtml;
  html += '<div><div class="profile-name">' + esc(name) + badge + '</div></div>';
  html += '</div>';

  const hasBio = bio && bio.trim().length > 0;
  const hasSocials = Object.values(socials).some(v => v && v.trim());

  if (!hasBio && !hasSocials) {
    html += '<div class="profile-card-empty">This user hasn\'t set up their profile yet.</div>';
  } else {
    if (hasBio) {
      html += '<div class="profile-card-bio">' + esc(bio) + '</div>';
    }
    if (hasSocials) {
      html += '<div class="profile-card-socials">';
      if (socials.website) {
        const url = socials.website;
        if (url.startsWith('https://')) {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> <a href="' + esc(url) + '" target="_blank" rel="noopener">' + esc(url) + '</a></div>';
        } else {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> ' + esc(url) + '</div>';
        }
      }
      if (socials.discord) {
        html += '<div class="social-item"><span class="social-label">💬 Discord</span> ' + esc(socials.discord) + '</div>';
      }
      if (socials.twitter) {
        const handle = socials.twitter.replace(/^@/, '');
        html += '<div class="social-item"><span class="social-label">𝕏 Twitter</span> <a href="https://x.com/' + esc(handle) + '" target="_blank" rel="noopener">@' + esc(handle) + '</a></div>';
      }
      if (socials.youtube) {
        const yt = socials.youtube;
        if (yt.startsWith('https://')) {
          html += '<div class="social-item"><span class="social-label">▶️ YouTube</span> <a href="' + esc(yt) + '" target="_blank" rel="noopener">' + esc(yt) + '</a></div>';
        } else {
          const ytUrl = 'https://youtube.com/@' + yt;
          html += '<div class="social-item"><span class="social-label">▶️ YouTube</span> <a href="' + esc(ytUrl) + '" target="_blank" rel="noopener">@' + esc(yt) + '</a></div>';
        }
      }
      if (socials.github) {
        const gh = socials.github.replace(/^@/, '');
        html += '<div class="social-item"><span class="social-label">🐙 GitHub</span> <a href="https://github.com/' + esc(gh) + '" target="_blank" rel="noopener">' + esc(gh) + '</a></div>';
      }
      html += '</div>';
    }
  }

  // Public key (click to copy) — M-3: use DOM API instead of inline onclick.
  if (publicKey) {
    const shortPk = publicKey.length > 24 ? publicKey.substring(0, 24) + '…' : publicKey;
    html += '<div class="profile-card-key" id="profile-pk-copy" title="Click to copy full key">🔑 ' + esc(shortPk) + '</div>';
  }

  // Follow/friend status + button
  if (publicKey && publicKey !== myKey) {
    const friend = isFriend(publicKey);
    const following = isFollowing(publicKey);
    const followsYou = myFollowers.has(publicKey);
    let statusText = '';
    if (friend) statusText = '🤝 Friends (mutual follow)';
    else if (following && followsYou) statusText = '🤝 Friends';
    else if (following) statusText = '👁️ You follow this user';
    else if (followsYou) statusText = '👁️‍🗨️ Follows you';
    const btnLabel = following ? '❌ Unfollow' : '👁️ Follow';
    html += '<div style="margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid var(--border);">';
    if (statusText) html += '<div style="font-size:0.75rem;color:var(--text-muted);margin-bottom:0.3rem;">' + statusText + '</div>';
    html += '<button id="profile-follow-btn" style="background:var(--accent);color:#fff;border:none;border-radius:6px;padding:0.3rem 0.8rem;font-size:0.78rem;cursor:pointer;">' + btnLabel + '</button>';
    html += '</div>';
  }

  document.getElementById('view-profile-content').innerHTML = html;
  // Attach click handler via DOM API (not inline onclick).
  if (publicKey) {
    const pkEl = document.getElementById('profile-pk-copy');
    if (pkEl) {
      pkEl.addEventListener('click', () => {
        navigator.clipboard.writeText(publicKey).then(() => addSystemMessage('Public key copied.'));
      });
    }
  }
  // Follow button handler
  if (publicKey && publicKey !== myKey) {
    const followBtn = document.getElementById('profile-follow-btn');
    if (followBtn) {
      followBtn.addEventListener('click', () => {
        if (ws && ws.readyState === WebSocket.OPEN) {
          const type = myFollowing.has(publicKey) ? 'unfollow' : 'follow';
          ws.send(JSON.stringify({ type, target_key: publicKey }));
          closeViewProfileOverlay();
        }
      });
    }
  }
  if (window.twemoji) twemoji.parse(document.getElementById('view-profile-content'));
  document.getElementById('view-profile-overlay').classList.add('open');
}

function closeViewProfileModal(e) {
  if (e.target === document.getElementById('view-profile-overlay')) {
    closeViewProfileOverlay();
  }
}
function closeViewProfileOverlay() {
  document.getElementById('view-profile-overlay').classList.remove('open');
  pendingProfileView = null;
}

// ── Block List (client-side) ──
function getBlockList() {
  try { return JSON.parse(localStorage.getItem('humanity_blocks') || '[]'); }
  catch { return []; }
}
function setBlockList(list) {
  localStorage.setItem('humanity_blocks', JSON.stringify(list));
}
function isBlocked(name) {
  return getBlockList().some(b => b.toLowerCase() === name.toLowerCase());
}

function blockUser(name) {
  if (name.toLowerCase() === myName.toLowerCase()) {
    addSystemMessage("You can't block yourself.");
    return;
  }
  const list = getBlockList();
  if (list.some(b => b.toLowerCase() === name.toLowerCase())) {
    addSystemMessage(`${name} is already blocked.`);
    return;
  }
  list.push(name);
  setBlockList(list);
  addSystemMessage(`🚫 Blocked ${name}. Their messages are now hidden.`);
  reRenderMessagesForBlockChange();
  rerenderUserList();
}

function unblockUser(name) {
  const list = getBlockList();
  const idx = list.findIndex(b => b.toLowerCase() === name.toLowerCase());
  if (idx === -1) {
    addSystemMessage(`${name} is not blocked.`);
    return;
  }
  list.splice(idx, 1);
  setBlockList(list);
  addSystemMessage(`✅ Unblocked ${name}.`);
  reRenderMessagesForBlockChange();
  rerenderUserList();
}

function showBlockList() {
  const list = getBlockList();
  if (list.length === 0) {
    addSystemMessage('No blocked users.');
  } else {
    addSystemMessage('🚫 Blocked users: ' + list.join(', '));
  }
}

// Re-filter visible messages after block/unblock change.
function reRenderMessagesForBlockChange() {
  const container = document.getElementById('messages');
  const msgs = container.querySelectorAll('.message[data-from]');
  msgs.forEach(el => {
    const authorEl = el.querySelector('.author');
    if (!authorEl) return;
    const authorName = authorEl.dataset.username;
    if (authorName && isBlocked(authorName)) {
      el.style.display = 'none';
    } else {
      el.style.display = '';
    }
  });
}

// Force re-render user list with updated block indicators.
function rerenderUserList() {
  // Trigger a full_user_list refresh if we have cached data.
  // The user list is already rendered from updateUserList; just re-render peer-list.
  const list = document.getElementById('peer-list');
  const peers = list.querySelectorAll('.peer[data-username]');
  peers.forEach(el => {
    const name = el.dataset.username;
    if (!name) return;
    const blocked = isBlocked(name);
    // Add/remove blocked indicator.
    let indicator = el.querySelector('.block-indicator');
    if (blocked && !indicator) {
      const span = document.createElement('span');
      span.className = 'block-indicator';
      span.textContent = ' 🚫';
      span.title = 'Blocked';
      span.style.fontSize = '0.65rem';
      el.appendChild(span);
      el.style.textDecoration = 'line-through';
      el.style.opacity = '0.5';
    } else if (!blocked && indicator) {
      indicator.remove();
      el.style.textDecoration = '';
      el.style.opacity = el.classList.contains('is-you') ? '' : '';
      // Restore original opacity from online/offline status.
      if (el.style.opacity === '') el.removeAttribute('style');
    }
  });
}

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

// ── DM State ──
let activeDmPartner = null; // Public key of active DM partner, or null for channel view.
let activeDmPartnerName = '';
let dmConversations = []; // Array of { partner_key, partner_name, last_message, last_timestamp, unread_count }

function upsertDmConversation(partnerKey, partnerName, lastMessage, lastTimestamp, incoming) {
  if (!partnerKey) return;
  const idx = dmConversations.findIndex(c => c.partner_key === partnerKey);
  if (idx >= 0) {
    const row = dmConversations[idx];
    row.partner_name = partnerName || row.partner_name;
    row.last_message = String(lastMessage || row.last_message || '');
    row.last_timestamp = Number(lastTimestamp || row.last_timestamp || Date.now());
    if (incoming && activeDmPartner !== partnerKey) {
      row.unread_count = Number(row.unread_count || 0) + 1;
    }
  } else {
    dmConversations.push({
      partner_key: partnerKey,
      partner_name: partnerName || shortKey(partnerKey),
      last_message: String(lastMessage || ''),
      last_timestamp: Number(lastTimestamp || Date.now()),
      unread_count: (incoming && activeDmPartner !== partnerKey) ? 1 : 0,
    });
  }
  dmConversations.sort((a, b) => Number(b.last_timestamp || 0) - Number(a.last_timestamp || 0));
  renderDmList();
}

/** Switch to DM conversation view. */
function openDmConversation(partnerKey, partnerName) {
  activeDmPartner = partnerKey;
  activeDmPartnerName = partnerName;

  // Switch to DMs tab in sidebar.
  if (typeof switchSidebarTab === 'function') switchSidebarTab('dms', true);

  // Clear unread for this conversation and update sidebar highlighting.
  const row = dmConversations.find(c => c.partner_key === partnerKey);
  if (row) row.unread_count = 0;
  renderDmList();
  renderChannelList(); // Deselect channels

  // Hide pin bar in DM view.
  document.getElementById('pin-bar').style.display = 'none';
  document.getElementById('pin-list').classList.remove('open');

  // Update channel header.
  const header = document.getElementById('channel-header');
  header.innerHTML = `<span class="ch-name" style="cursor:pointer;" onclick="closeDmView()">← Back</span> <span class="ch-name">💬 ${esc(partnerName)}</span>`;
  header.style.display = 'block';

  // Clear messages area.
  document.getElementById('messages').innerHTML = '';

  // Enable input.
  const input = document.getElementById('msg-input');
  input.disabled = false;
  input.placeholder = `Message ${partnerName}…`;
  document.getElementById('send-btn').disabled = false;

  // Request DM history from server.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'dm_open', partner: partnerKey }));
  }

  if (isMobile()) closeSidebars();
}

/** Close DM view and return to channel view. */
function closeDmView() {
  activeDmPartner = null;
  activeDmPartnerName = '';
  renderDmList();
  switchChannel(activeChannel);
}

/** Add a DM message to the message area. */
function addDmMessage(author, body, timestamp, fromKey, toKey, isEncrypted) {
  const el = document.createElement('div');
  el.className = 'message dm-message';
  el.dataset.from = fromKey;
  el.dataset.timestamp = timestamp;

  const time = formatTime(timestamp);
  const isMe = fromKey === myKey;

  const isBotMsg2 = fromKey && fromKey.startsWith('bot_');
  const identiconSrc = (!isBotMsg2 && fromKey) ? generateIdenticon(fromKey, 20) : '';
  const identiconHtml = isBotMsg2 ? '<span class="identicon" style="font-size:18px;line-height:20px;">🤖</span>' : (identiconSrc ? `<img src="${identiconSrc}" class="identicon" alt="">` : '');
  const e2eeBadge = isEncrypted ? '<span title="End-to-end encrypted" style="font-size:0.65rem;opacity:0.6;margin-left:0.3rem;">🔒</span>' : '';

  el.innerHTML = `
    <div class="meta">
      ${identiconHtml}
      <span class="author${isMe ? ' you' : ''}">${esc(author)}</span>
      <span class="time">${time}</span>${e2eeBadge}
    </div>
    <div class="body">${formatBody(body)}</div>
  `;

  appendMessage(el);
  if (window.twemoji) twemoji.parse(el);
}

/** Render the DM conversation list in the sidebar. */
function renderDmList() {
  const list = document.getElementById('dm-list');
  if (dmConversations.length === 0) {
    list.innerHTML = '<div style="font-size:0.7rem;color:var(--text-muted);padding:0.3rem 0.5rem;">No conversations yet</div>';
    return;
  }

  list.innerHTML = dmConversations.map(c => {
    const isActive = activeDmPartner === c.partner_key;
    const unread = c.unread_count > 0 ? '<span class="dm-unread"></span>' : '';
    const preview = c.last_message.length > 30 ? c.last_message.substring(0, 30) + '…' : c.last_message;
    const timeStr = formatTime(c.last_timestamp);
    return `<div class="dm-item${isActive ? ' active' : ''}" onclick="openDmConversation('${esc(c.partner_key)}', '${esc(c.partner_name)}')">
      <div style="flex:1;min-width:0;">
        <div class="dm-name">${esc(c.partner_name)} ${unread}</div>
        <div class="dm-preview">${esc(preview)}</div>
      </div>
      <div class="dm-time">${timeStr}</div>
    </div>`;
  }).join('');
  if (window.twemoji) twemoji.parse(list);
}

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
  return {
    echoCancellation: { ideal: true },
    noiseSuppression: { ideal: true },
    autoGainControl: { ideal: true },
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
  let audioCtx = null;
  let localAnalyser = null;
  let speakingPollInterval = null;
  let remoteAnalysers = {}; // peerKey → { analyser, source, interval }

  window.toggleVoiceRoomMute = function() {
    if (!window._roomLocalStream) return;
    vcMuted = !vcMuted;
    window._roomLocalStream.getAudioTracks().forEach(t => { t.enabled = !vcMuted; });
    const btn = document.getElementById('vc-mute-btn');
    btn.textContent = vcMuted ? '🔇' : '🎤';
    btn.classList.toggle('vc-muted', vcMuted);
    btn.title = vcMuted ? 'Unmute' : 'Mute';
  };

  window.setVoiceRoomVolume = function(val) {
    vcVolume = parseInt(val);
    document.querySelectorAll('.room-remote-audio').forEach(el => {
      el.volume = vcVolume / 100;
    });
  };

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
        const speaking = avg > 20;
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
      const btn = document.getElementById('vc-mute-btn');
      if (btn) { btn.textContent = '🎤'; btn.classList.remove('vc-muted'); }
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

// ── Follow/Friend System (Client State) ──
let myFollowing = new Set(); // keys I'm following
let myFollowers = new Set(); // keys following me
let activeGroupId = null; // Currently viewing group
let activeGroupName = '';
let myGroups = []; // Array of { id, name, invite_code, role }
let groupMembersByGroup = {}; // group_id -> [{ key, role }]

function isFriend(key) {
  return myFollowing.has(key) && myFollowers.has(key);
}

function isFollowing(key) {
  return myFollowing.has(key);
}

function resolveNameToKey(name) {
  // Search through the known user list for a matching name
  const lowerName = name.toLowerCase();
  const peerList = document.getElementById('peer-list');
  if (!peerList) return null;
  const peers = peerList.querySelectorAll('.peer[data-pubkey]');
  for (const el of peers) {
    const peerName = (el.dataset.username || '').toLowerCase();
    if (peerName === lowerName) return el.dataset.pubkey;
  }
  return null;
}

// Handle follow/friend/group messages from server
const _origHandleMessageFollow = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'follow_list') {
    myFollowing = new Set(msg.following || []);
    myFollowers = new Set(msg.followers || []);
    updateFriendIndicators();
    return;
  }
  if (msg.type === 'follow_update') {
    if (msg.follower_key === myKey) {
      if (msg.action === 'follow') myFollowing.add(msg.followed_key);
      else myFollowing.delete(msg.followed_key);
    }
    if (msg.followed_key === myKey) {
      if (msg.action === 'follow') myFollowers.add(msg.follower_key);
      else myFollowers.delete(msg.follower_key);
    }
    updateFriendIndicators();
    return;
  }
  if (msg.type === 'group_list') {
    myGroups = msg.groups || [];
    renderGroupList();
    return;
  }
  if (msg.type === 'group_message') {
    if (activeGroupId === msg.group_id) {
      const name = resolveSenderName(msg.from_name, msg.from);
      const isYou = msg.from === myKey;
      addMessageToChat(name, msg.content, msg.timestamp, isYou, msg.from);
    }
    return;
  }
  if (msg.type === 'group_history') {
    if (msg.group_id === activeGroupId) {
      const messagesDiv = document.getElementById('messages');
      messagesDiv.innerHTML = '';
      for (const m of (msg.messages || [])) {
        const isYou = m.from === myKey;
        addMessageToChat(resolveSenderName(m.from_name, m.from), m.content, m.timestamp, isYou, m.from);
      }
    }
    return;
  }
  if (msg.type === 'group_members') {
    groupMembersByGroup[msg.group_id] = (msg.members || []).map(([key, role]) => ({ key, role }));
    if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
    return;
  }
  _origHandleMessageFollow(msg);
};

function updateFriendIndicators() {
  // Update friend/follow icons next to peers in the peer list
  document.querySelectorAll('.peer[data-pubkey]').forEach(el => {
    const key = el.dataset.pubkey;
    if (!key || key === myKey) return;
    // Remove old indicators
    el.querySelectorAll('.follow-indicator').forEach(x => x.remove());
    if (isFriend(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.textContent = ' 🤝';
      badge.title = 'Friend (mutual follow)';
      el.querySelector('.peer-name')?.appendChild(badge) || el.appendChild(badge);
    } else if (isFollowing(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.textContent = ' 👁️';
      badge.title = 'Following';
      el.querySelector('.peer-name')?.appendChild(badge) || el.appendChild(badge);
    } else if (myFollowers.has(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.textContent = ' 👁️‍🗨️';
      badge.title = 'Follows you';
      el.querySelector('.peer-name')?.appendChild(badge) || el.appendChild(badge);
    }
  });
}

// Patch updateUserList to add friend indicators after render
const _origUpdateUserListFollow = updateUserList;
updateUserList = function(users) {
  _origUpdateUserListFollow(users);
  updateFriendIndicators();
  addFollowContextMenu();
};

function addFollowContextMenu() {
  document.querySelectorAll('.peer[data-pubkey]').forEach(el => {
    const key = el.dataset.pubkey;
    if (!key || key === myKey) return;
    el.removeEventListener('contextmenu', el._followCtx);
    el._followCtx = function(e) {
      e.preventDefault();
      // Remove any existing context menu
      document.querySelectorAll('.follow-ctx-menu').forEach(m => m.remove());
      const menu = document.createElement('div');
      menu.className = 'follow-ctx-menu';
      menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:4px 0;min-width:140px;box-shadow:0 4px 12px rgba(0,0,0,0.3);';
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';

      const following = myFollowing.has(key);
      const item = document.createElement('div');
      item.style.cssText = 'padding:6px 12px;cursor:pointer;font-size:0.82rem;color:var(--text);';
      item.textContent = following ? '❌ Unfollow' : '👁️ Follow';
      item.onmouseenter = () => { item.style.background = 'var(--bg-hover)'; };
      item.onmouseleave = () => { item.style.background = ''; };
      item.onclick = () => {
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: following ? 'unfollow' : 'follow', target_key: key }));
        }
        menu.remove();
      };
      menu.appendChild(item);

      document.body.appendChild(menu);
      const closeMenu = (ev) => { if (!menu.contains(ev.target)) { menu.remove(); document.removeEventListener('click', closeMenu); } };
      setTimeout(() => document.addEventListener('click', closeMenu), 0);
    };
    el.addEventListener('contextmenu', el._followCtx);
  });
}

function renderGroupList() {
  const container = document.getElementById('tab-groups');
  if (!container) return;
  if (myGroups.length === 0) {
    container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.8rem;">No groups yet.<br>Use <code>/group-create &lt;name&gt;</code> to create one.</div>';
    return;
  }
  let html = '';
  for (const g of myGroups) {
    const isActive = activeGroupId === g.id;
    html += `<div class="channel-item${isActive ? ' active' : ''}" data-group-id="${g.id}" style="cursor:pointer;">
      <span style="opacity:0.6">👥 </span>${esc(g.name)}
      <span style="font-size:0.6rem;color:var(--text-muted);margin-left:auto;">${g.role}</span>
    </div>`;
  }
  html += '<div style="padding:0.3rem 0;"><button class="vr-btn" onclick="promptCreateGroup()" style="width:100%;font-size:0.7rem;">+ Create Group</button></div>';
  container.innerHTML = html;
  // Click handler for groups
  container.querySelectorAll('[data-group-id]').forEach(el => {
    el.onclick = () => openGroup(el.dataset.groupId);
    el.oncontextmenu = (e) => {
      e.preventDefault();
      document.querySelectorAll('.group-ctx-menu').forEach(m => m.remove());
      const gid = el.dataset.groupId;
      const group = myGroups.find(g => g.id === gid);
      if (!group) return;
      const menu = document.createElement('div');
      menu.className = 'group-ctx-menu';
      menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:4px 0;min-width:150px;box-shadow:0 4px 12px rgba(0,0,0,0.3);';
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';
      const items = [
        { label: '📋 Copy Invite Code', action: () => { navigator.clipboard.writeText(group.invite_code).then(() => addSystemMessage('Invite code copied: ' + group.invite_code)); }},
        { label: '👤 Invite User', action: () => { const name = prompt('Share this invite code with a user:\\n' + group.invite_code + '\\n\\nOr enter a username to tell them:'); if (name && name.trim()) { addSystemMessage('Share this invite code with ' + name.trim() + ': ' + group.invite_code); } }},
        { label: '🚪 Leave Group', action: () => { if (confirm('Leave group "' + group.name + '"?') && ws && ws.readyState === WebSocket.OPEN) { ws.send(JSON.stringify({ type: 'group_leave', group_id: gid })); if (activeGroupId === gid) { activeGroupId = null; activeGroupName = ''; } } }},
      ];
      items.forEach(it => {
        const div = document.createElement('div');
        div.style.cssText = 'padding:6px 12px;cursor:pointer;font-size:0.82rem;color:var(--text);';
        div.textContent = it.label;
        div.onmouseenter = () => { div.style.background = 'var(--bg-hover)'; };
        div.onmouseleave = () => { div.style.background = ''; };
        div.onclick = (ev) => { ev.stopPropagation(); menu.remove(); it.action(); };
        menu.appendChild(div);
      });
      document.body.appendChild(menu);
      const closeMenu = (ev) => { if (!menu.contains(ev.target)) { menu.remove(); document.removeEventListener('click', closeMenu); } };
      setTimeout(() => document.addEventListener('click', closeMenu), 0);
    };
  });
}

function promptCreateGroup() {
  const name = prompt('Group name:');
  if (name && name.trim() && ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'group_create', name: name.trim() }));
  }
}

function promptJoinGroup() {
  const code = prompt('Enter group invite code:');
  if (code && code.trim() && ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'group_join', invite_code: code.trim() }));
  }
}

function openGroup(groupId) {
  const group = myGroups.find(g => g.id === groupId);
  if (!group) return;
  activeGroupId = groupId;
  activeGroupName = group.name;
  activeDmPartner = null; // Exit DM view
  // Update channel header
  const header = document.getElementById('channel-header');
  if (header) {
    header.style.display = 'flex';
    header.querySelector('.ch-name').textContent = '👥 ' + group.name;
    header.querySelector('.ch-desc').textContent = 'Group • Invite: ' + group.invite_code;
  }
  // Clear messages and request group history.
  document.getElementById('messages').innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:1rem;font-size:0.8rem;">Loading group history for ' + esc(group.name) + '...</div>';
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'group_history_request', group_id: groupId }));
    ws.send(JSON.stringify({ type: 'group_members_request', group_id: groupId }));
  }
  renderGroupList();
  if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
}

// When switching to a channel, clear group view
const _origSwitchChannelFollow = switchChannel;
switchChannel = function(channelId) {
  activeGroupId = null;
  activeGroupName = '';
  _origSwitchChannelFollow(channelId);
  if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
};

// Helper to add a message to the chat (for groups)
function addMessageToChat(name, content, timestamp, isYou, fromKey) {
  const messagesDiv = document.getElementById('messages');
  const div = document.createElement('div');
  div.className = 'message';
  const time = new Date(timestamp);
  const timeStr = time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  div.innerHTML = `<div class="meta"><span class="author${isYou ? ' you' : ''}">${esc(name)}</span><span class="timestamp">${timeStr}</span></div><div class="body">${esc(content)}</div>`;
  messagesDiv.appendChild(div);
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

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
  if (msg.from !== callPeerKey) return; // Ignore signals from unexpected peers
  switch (msg.signal_type) {
    case 'offer':
      handleOffer(msg.data);
      break;
    case 'answer':
      handleAnswer(msg.data);
      break;
    case 'ice':
      handleIceCandidate(msg.data);
      break;
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

function getActiveSidebarTabName() {
  const el = document.querySelector('#sidebar-tabs .sidebar-tab.active');
  return el ? el.getAttribute('data-tab') : 'servers';
}

function renderPresenceSidebarForActiveContext() {
  const peerList = document.getElementById('peer-list');
  const usersHeader = document.querySelector('#right-sidebar h3');
  if (!peerList) return;

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
  if (vrScreenActive) stopVrScreenShare();
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
  if (vrVideoActive) stopVrVideo();
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
updateVideoPanel();
  }

  // Patch cleanupRoomAudio to stop video too
  const _origCleanupRoomAudio2 = window.cleanupRoomAudio;
  window.cleanupRoomAudio = function() {
stopVrVideo();
stopVrScreenShare();
document.querySelectorAll('#video-panel .video-wrapper:not([data-id^="dm-"])').forEach(el => el.remove());
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
  function showLocalVideo(stream, id) {
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
label.textContent = id.includes('screen') ? 'You (Screen)' : 'You';
wrapper.appendChild(video);
wrapper.appendChild(label);
panel.appendChild(wrapper);
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

  // Global Escape key handler to close modals/dropdowns
  document.addEventListener('keydown', function(e) {
// Ctrl+F opens search
if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
  e.preventDefault();
  toggleSearch();
  return;
}
if (e.key === 'Escape') {
  // Close search panel
  const searchBar = document.getElementById('search-bar');
  if (searchBar && searchBar.classList.contains('open')) { closeSearch(); return; }
  // Close help modal
  const helpOverlay = document.getElementById('help-modal-overlay');
  if (helpOverlay && helpOverlay.classList.contains('open')) { helpOverlay.classList.remove('open'); return; }
  // Close view profile modal
  const profileOverlay = document.getElementById('view-profile-overlay');
  if (profileOverlay && profileOverlay.classList.contains('open')) { profileOverlay.classList.remove('open'); return; }
  // Close edit profile modal
  const editOverlay = document.getElementById('edit-profile-overlay');
  if (editOverlay && editOverlay.classList.contains('open')) { editOverlay.classList.remove('open'); return; }
  // Close cog dropdown
  if (typeof activeCogDropdown !== 'undefined' && activeCogDropdown) { activeCogDropdown.remove(); activeCogDropdown = null; return; }
  // Close context menus
  document.querySelectorAll('.follow-ctx-menu, .group-ctx-menu').forEach(m => m.remove());
  const ctxMenu = document.getElementById('user-context-menu');
  if (ctxMenu && ctxMenu.classList.contains('open')) { ctxMenu.classList.remove('open'); return; }
}
  });

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
