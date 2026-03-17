// ── chat-messages.js ──────────────────────────────────────────────────────
// Reactions, message editing, pins, typing indicator, image upload, threads.
// Depends on: app.js globals (ws, myKey, myName, activeChannel, esc, formatBody,
//   addChatMessage, appendMessage)
// ─────────────────────────────────────────────────────────────────────────

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
    newPlaceholder.innerHTML = hosIcon('image', 14) + ' Image (click to load)';
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
    badge.innerHTML = `${hosIcon('chat', 16)} ${count} ${count === 1 ? 'reply' : 'replies'}`;
  } else {
    // Create new badge.
    badge = document.createElement('div');
    badge.className = 'thread-badge';
    badge.dataset.threadFrom = parentFrom;
    badge.dataset.threadTs = parentTimestamp;
    badge.innerHTML = hosIcon('chat', 16) + ' 1 reply';
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
