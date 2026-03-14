// Parse static emoji on page load — skip hub-nav so emoji match other pages
document.addEventListener('DOMContentLoaded', () => {
  if (window.twemoji) {
    document.querySelectorAll('#login-screen, #chat-screen').forEach(el => twemoji.parse(el));
  }
});

// Open Edit Profile modal when the Account nav button is clicked while already
// on /chat (hash navigation doesn't trigger a page reload, so we need this).
window.addEventListener('hashchange', () => {
  if (location.hash === '#profile') {
    history.replaceState(null, '', location.pathname);
    if (typeof openEditProfileModal === 'function') openEditProfileModal();
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

  // Update key-protection button to reflect current state.
  const kpBtn = document.getElementById('key-protect-btn');
  if (kpBtn && typeof isKeyWrapped === 'function') {
    if (isKeyWrapped()) {
      kpBtn.textContent = '🔒 Key Protected';
      kpBtn.style.color = 'var(--success)';
    }
  }

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

  // If the user arrived via the Account nav button (/chat#profile), open the
  // Edit Profile modal automatically once connected so they land on their profile.
  if (location.hash === '#profile') {
    history.replaceState(null, '', location.pathname); // clean the hash
    setTimeout(() => { if (typeof openEditProfileModal === 'function') openEditProfileModal(); }, 200);
  }

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
      // Refresh sidebar datasets on connect/reconnect.
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'chat', from: myKey, from_name: myName, content: '/dms', timestamp: Date.now(), channel: 'general' }));
        ws.send(JSON.stringify({ type: 'chat', from: myKey, from_name: myName, content: '/groups', timestamp: Date.now(), channel: 'general' }));
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
      // Cache all profile fields and show the card if we were waiting for this user's profile.
      if (msg.name) {
        profileCache[msg.name.toLowerCase()] = {
          bio:        msg.bio        || '',
          socials:    msg.socials    || '{}',
          avatar_url: msg.avatar_url || '',
          banner_url: msg.banner_url || '',
          pronouns:   msg.pronouns   || '',
          location:   msg.location   || '',
          website:    msg.website    || '',
        };
        // If we have a pending view for this user, show it now.
        if (pendingProfileView && pendingProfileView.name.toLowerCase() === msg.name.toLowerCase()) {
          const c = profileCache[msg.name.toLowerCase()];
          showViewProfileCard(pendingProfileView.name, pendingProfileView.publicKey, c);
          pendingProfileView = null;
        }
        // If this is our own full profile echoed back on connect, sync to local storage
        // only when local storage has no data yet (avoids overwriting unsaved edits).
        if (msg.name.toLowerCase() === myName.toLowerCase()) {
          try {
            const local = loadProfileLocal();
            const localIsEmpty = !local.bio && !local.avatar_url && !local.pronouns &&
                                 Object.keys(local.socials || {}).length === 0;
            if (localIsEmpty) {
              saveProfileLocal({
                bio:        msg.bio        || '',
                socials:    JSON.parse(msg.socials || '{}'),
                avatar_url: msg.avatar_url || '',
                banner_url: msg.banner_url || '',
                pronouns:   msg.pronouns   || '',
                location:   msg.location   || '',
                website:    msg.website    || '',
                privacy:    JSON.parse(msg.privacy || '{}'),
              });
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
  updateRulesBanner();
}

// ── #rules agree/disagree banner ──
function updateRulesBanner() {
  let banner = document.getElementById('rules-agree-banner');
  if (activeChannel !== 'rules') { if (banner) banner.style.display = 'none'; return; }
  if (!banner) {
    banner = document.createElement('div');
    banner.id = 'rules-agree-banner';
    banner.style.cssText = 'padding:0.8rem 1rem;border-top:1px solid var(--border);background:var(--bg-panel,#141414);display:flex;align-items:center;gap:0.8rem;flex-wrap:wrap;flex-shrink:0;';
    const inputArea = document.getElementById('input-area');
    if (inputArea && inputArea.parentNode) inputArea.parentNode.insertBefore(banner, inputArea);
  }
  banner.style.display = 'flex';
  const agreed = localStorage.getItem('humanity_rules_agreed');
  if (agreed === 'true') {
    banner.innerHTML = '<span style="color:#4a8;font-size:0.85rem;">✅ You have agreed to the community rules.</span>' +
      '<button onclick="rulesDisagree()" style="margin-left:auto;background:rgba(220,50,50,0.15);border:1px solid rgba(220,50,50,0.4);color:#e55;padding:0.3rem 0.8rem;border-radius:6px;cursor:pointer;font-size:0.78rem;">❌ Withdraw</button>';
  } else if (agreed === 'false') {
    banner.innerHTML = '<span style="color:#e55;font-size:0.85rem;">❌ You have not agreed to the rules.</span>' +
      '<button onclick="rulesAgree()" style="background:rgba(34,170,102,0.15);border:1px solid #4a8;color:#4a8;padding:0.3rem 0.8rem;border-radius:6px;cursor:pointer;font-size:0.78rem;">✅ I Agree</button>';
  } else {
    banner.innerHTML = '<span style="font-size:0.85rem;font-weight:600;">Do you agree to the Community Guidelines?</span>' +
      '<button onclick="rulesAgree()" style="background:rgba(34,170,102,0.9);border:none;color:#fff;padding:0.35rem 1.2rem;border-radius:6px;cursor:pointer;font-size:0.85rem;font-weight:600;">✅ I Agree</button>' +
      '<button onclick="rulesDisagree()" style="background:rgba(220,50,50,0.15);border:1px solid rgba(220,50,50,0.4);color:#e55;padding:0.35rem 1rem;border-radius:6px;cursor:pointer;font-size:0.85rem;">❌ Disagree</button>';
  }
}

function rulesAgree() {
  localStorage.setItem('humanity_rules_agreed', 'true');
  updateRulesBanner();
  addSystemMessage('✅ You have agreed to the Humanity Network community rules. Welcome! 💚');
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({type:'chat',from:myKey,from_name:myName,content:'/rules_agreed',timestamp:Date.now(),channel:'rules'}));
  }
}

function rulesDisagree() {
  localStorage.setItem('humanity_rules_agreed', 'false');
  updateRulesBanner();
  addSystemMessage('❌ You have not agreed to the rules. You can change your mind any time in #rules.');
}
function setStatus(cls, text) {
  const el = document.getElementById('status');
  el.className = cls;
  document.getElementById('status-text').textContent = text;
  // Reflect connection state on the nav Chat tab for visibility from any scroll position
  updateNavDot(cls);
}

function updateNavDot(cls) {
  let dot = document.getElementById('hos-ws-dot');
  if (!dot) {
    // Inject a small dot into the Chat nav tab
    const chatTab = document.querySelector('.hub-nav .tab[href="/chat"]');
    if (!chatTab) return;
    dot = document.createElement('span');
    dot.id = 'hos-ws-dot';
    dot.style.cssText = 'position:absolute;top:3px;right:3px;width:6px;height:6px;border-radius:50%;transition:background 0.4s';
    chatTab.appendChild(dot);
  }
  dot.style.background = cls === 'connected' ? '#4ec87a' : cls === 'reconnecting' ? '#f0c040' : '#e55';
  dot.title = cls === 'connected' ? 'Connected' : cls === 'reconnecting' ? 'Reconnecting…' : 'Disconnected';
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

