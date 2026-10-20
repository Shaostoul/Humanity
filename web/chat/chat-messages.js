// ── chat-messages.js ──────────────────────────────────────────────────────
// Reactions, message editing, pins, typing indicator, image upload, threads.
// Depends on: app.js globals (ws, myKey, myName, activeChannel, esc, formatBody,
//   addChatMessage, appendMessage)
// ─────────────────────────────────────────────────────────────────────────

// ── Emoji Reactions ──
// The palette lives in data/reactions.json (one source shared with the native
// picker and the relay allowlist; served by nginx at /data/reactions.json).
// The literal below is only the offline fallback. Entries are BARE (no U+FE0F)
// to match the relay's canonical stored form.
let REACTION_EMOJIS = ['👍', '❤', '😂', '😮', '😢', '🎉', '🔥', '👀'];
fetch('/data/reactions.json')
  .then(function (r) { return r.ok ? r.json() : null; })
  .then(function (d) {
    if (d && Array.isArray(d.top) && d.top.length) REACTION_EMOJIS = d.top;
  })
  .catch(function () { /* offline: keep the fallback */ });
// Track reactions: key = "fromKey:timestamp", value = { emoji: Set(reactorKeys) }
const messageReactions = {};

function showReactionPicker(btn, targetFrom, targetTs, msgEl) {
  // Close any existing picker.
  document.querySelectorAll('.reaction-picker').forEach(p => p.remove());

  const picker = document.createElement('div');
  picker.className = 'reaction-picker';
  picker.style.cssText = 'position:absolute;top:-2rem;right:0;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-xs);display:flex;gap:var(--space-xs);z-index:20;';
  REACTION_EMOJIS.forEach(emoji => {
    const btn = document.createElement('span');
    btn.textContent = emoji;
    btn.style.cssText = 'cursor:pointer;padding:var(--space-xs) var(--space-sm);border-radius:var(--radius-sm);font-size:0.9rem;';
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

/**
 * A P2P group is open (BUG-178), or the scratch pad (10m R10, the desktop
 * app's is_private_channel, which counts "scratchpad"): nothing that sends a
 * message's text or reactions to the server (a reaction, an edit, a server
 * pin, a delete).
 */
function privateViewOpen() {
  if (typeof window !== 'undefined' && window.activeP2pGroup) return true;
  return typeof scratchPadOpen === 'function' && scratchPadOpen();
}

function sendReaction(targetFrom, targetTs, emoji) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  if (privateViewOpen()) return;
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

/** Is this key someone I blocked (app.js isBlockedKey, step C)? Their reactions and thread replies are not shown. */
function fromBlockedKey(reactorKey) {
  return typeof isBlockedKey === 'function' && isBlockedKey(reactorKey);
}

function applyReaction(targetFrom, targetTs, emoji, reactorKey, reactorName) {
  if (fromBlockedKey(reactorKey)) return;
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
    // Counted without anyone I blocked (a reaction from before the block too).
    const shown = Array.from(users).filter((k) => !fromBlockedKey(k)).length;
    if (shown === 0) return '';
    const isMine = users.has(myKey);
    return `<span class="reaction-badge${isMine ? ' mine' : ''}" data-target-from="${esc(targetFrom)}" data-target-ts="${targetTs}" data-emoji="${esc(emoji)}">${esc(emoji)} <span class="count">${shown}</span></span>`;
  }).join('');
  if (window.twemoji) twemoji.parse(msgEl);
}

// Apply a reaction from sync (add-only, no toggle).
function applyReactionSync(targetFrom, targetTs, emoji, reactorKey) {
  if (fromBlockedKey(reactorKey)) return;
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
  saveBtn.onclick = async (e) => {
    e.stopPropagation();
    const newContent = textarea.value.trim();
    if (!newContent || newContent.length > getMaxMsgLength()) return;
    // Never my recovery phrase (step F, chat-warnings.js): the edit stays open to fix.
    if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(newContent, 'Your edit was not saved.')) return;
    // Send edit via WebSocket; never from a group, whose text is encrypted (BUG-178).
    if (privateViewOpen()) return;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'edit',
        from: myKey,
        timestamp: Number(timestamp),
        new_content: newContent,
        channel: activeChannel,
      }));
    }
    // Apply locally immediately, its Media cards with it (chat-files-card.js).
    const media = typeof filesCardsPlan === 'function' ? filesCardsPlan(newContent, fromKey) : null;
    bodyEl.innerHTML = formatBody(media ? media.words : newContent);
    if (typeof fillFileCards === 'function') fillFileCards(bodyEl);
    if (media) filesCardsPut(msgEl, media.ids, fromKey, timestamp);
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
  // Who wrote it goes along, so an edit cannot bring back a picture the
  // protected setup does not show (10h). Its Media cards follow it (chat-files-card.js).
  const media = typeof filesCardsPlan === 'function' ? filesCardsPlan(newContent, fromKey) : null;
  bodyEl.innerHTML = formatBody(media ? media.words : newContent, fromKey);
  if (typeof fillFileCards === 'function') fillFileCards(bodyEl);
  if (media) filesCardsPut(msgEl, media.ids, fromKey, timestamp);
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
  if (privateViewOpen()) return; // a group message's text never goes to the server (BUG-178)
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
  // (Someone I blocked never reaches here: app.js drops their `typing` by key.)
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

// The largest file sent into a private conversation (a DM, a group, the
// scratch pad) until the server has said my own private limit, and the
// fallback for a server's room until it has said my upload limit. The server
// says both in my own peer-list entry: `upload_limit_mb` (my role's limit for
// one file in a room, window.myUploadLimitBytes, 2026-10-10) and
// `private_limit_mb` (my role's largest file in a private conversation,
// window.myPrivateLimitBytes, 2026-10-11, Server Settings > Roles), which the
// server holds an encrypted upload to instead of the room's limit.
const MAX_ATTACHMENT_BYTES = 6 * 1024 * 1024;

/** The most a file may be for where it is going now: the desktop app's attach_send.rs limit_for. */
function attachmentLimit() {
  const known = (n) => Number.isFinite(n) && n > 0;
  // A private conversation: my role's private limit, once the server has said it.
  if (privateConversationNow()) {
    const priv = Number(window.myPrivateLimitBytes);
    return known(priv) ? priv : MAX_ATTACHMENT_BYTES;
  }
  const mine = Number(window.myUploadLimitBytes);
  return known(mine) ? mine : MAX_ATTACHMENT_BYTES;
}

/**
 * POST an upload, saying how much of it has gone (`onProgress(fraction)`, 0 to 1), so a slow phone
 * connection shows it moving rather than one unchanging line (the phone check of 2026-10-12).
 * Answers the way fetch's Response does: ok, status, text(), json(). Without XMLHttpRequest it is
 * plain fetch, with no progress.
 */
function uploadPost(url, body, onProgress) {
  if (typeof XMLHttpRequest !== 'function') return fetch(url, { method: 'POST', body });
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', url);
    if (xhr.upload && onProgress) {
      xhr.upload.onprogress = (e) => { if (e && e.lengthComputable && e.total > 0) onProgress(e.loaded / e.total); };
    }
    xhr.onload = () => {
      const text = xhr.responseText || '';
      resolve({ ok: xhr.status >= 200 && xhr.status < 300, status: xhr.status, text: async () => text, json: async () => JSON.parse(text) });
    };
    xhr.onerror = () => reject(new Error('the connection was lost'));
    xhr.onabort = () => reject(new Error('the upload was stopped'));
    xhr.send(body);
  });
}

/** The upload line's words with how much has gone: "Uploading clip.mp4… 42%". */
function uploadingWords(name, fraction) {
  const pct = Math.max(0, Math.min(100, Math.floor((fraction || 0) * 100)));
  return `Uploading ${name}… ${pct}%`;
}

/** The upload address with this session's token, or null before the server has sent one: a bare
 *  key proves nothing about the sender and the server no longer takes it (2026-10-11). */
function uploadBaseUrl() {
  if (!myUploadToken) {
    addSystemMessage('The file was not sent: this server has not finished signing you in yet. Try again in a moment.');
    return null;
  }
  return `/api/upload?token=${encodeURIComponent(myUploadToken)}`;
}

function attachmentTooLarge(file) {
  const limit = attachmentLimit();
  if (file && file.size > limit) {
    const sizeMB = (file.size / 1024 / 1024).toFixed(1);
    const limitMB = Math.round(limit / 1024 / 1024);
    addSystemMessage(privateConversationNow()
      ? `"${file.name}" is ${sizeMB} MB. Files in a private conversation can be up to ${limitMB} MB for your role on this server. An admin can change that in Server Settings > Roles.`
      : `"${file.name}" is ${sizeMB} MB, over your ${limitMB} MB limit on this server. An admin can raise it in Server Settings > Roles.`);
    return true;
  }
  return false;
}

// ── Files leave cleaned (2026-10-11) ──
// docs/design/files-showcase.md, "Files on the sender's device and in
// messages, exact", parts 1 to 3. Every file picked, pasted or dropped here,
// in a public room or a private conversation, is cleaned on this device before
// it is uploaded or encrypted (/shared/upload-clean.js over
// /shared/strip-metadata.js): where it was taken and which camera took it are
// removed, unless the person turns off "Remove location and camera details"
// (on by default, remembered on this device; with the protected setup on, it
// is always on and not shown). The attach step (showAttachStep) appears only
// when there is something to say or decide: hidden details found, a file that
// could not be checked (in public it is not sent; in a DM or a group the
// person is asked "Send it anyway?" and its marker says "unchecked": true),
// or, in a DM, that the other person accepts files only from friends (their
// profile's `accepts_files`). A file with nothing hidden in it goes at once.
// "Show the attach button in private conversations" (Files in messages,
// /shared/files-in-messages.js) hides the button in DMs and groups and stops
// a paste or a drop there too.

/** This device's storage, or null. */
function attachStorage() {
  try { return window.localStorage; } catch (e) { return null; }
}

/** Is the protected setup on, on this device? */
function attachProtectedOn() {
  if (typeof protectedIsOn === 'function') return !!protectedIsOn();
  return !!(window.HosUploadClean && HosUploadClean.protectedSetupOn(attachStorage()));
}

/** Does "Show the attach button in private conversations" let a file go in a DM or a group? */
function attachInPrivateAllowed() {
  const F = window.HosFilesInMessages;
  return !F || F.readSettings(attachStorage()).attach !== false;
}

/** Show or hide the attach button for the conversation on screen (Files in messages). */
function updateAttachButton() {
  const btn = document.getElementById('attach-btn');
  if (!btn || !btn.style) return;
  const where = privateConversationNow();
  const inPrivate = where.startsWith('dm:') || where.startsWith('group:');
  btn.style.display = inPrivate && !attachInPrivateAllowed() ? 'none' : '';
  if (where.startsWith('dm:')) askAcceptsFiles(where.slice(3));
}
window.updateAttachButton = updateAttachButton;

// The people whose profile was asked for here, so their `accepts_files` is known when a file is
// sent to them (one ask per person per page).
const acceptsFilesAsked = new Set();

/** The name a profile is asked for and kept under (the member list's name). */
function attachProfileName(key) {
  const p = (typeof peerData !== 'undefined' && peerData) ? peerData[key] : null;
  if (p && p.display_name) return p.display_name;
  return (typeof activeDmPartner !== 'undefined' && activeDmPartner === key && activeDmPartnerName) ? activeDmPartnerName : '';
}

/** Ask the server for this person's profile once, for its `accepts_files` (only with Files in messages loaded). */
function askAcceptsFiles(key) {
  if (!window.HosFilesInMessages || !key || acceptsFilesAsked.has(key)) return;
  const name = attachProfileName(key);
  if (!name) return;
  if (typeof profileCache !== 'undefined' && profileCache && profileCache[name.toLowerCase()]) return;
  if (typeof ws === 'undefined' || !ws || ws.readyState !== 1) return;
  acceptsFilesAsked.add(key);
  const req = { type: 'profile_request', name };
  const pass = typeof friendPassFor === 'function' ? friendPassFor(key) : null;
  if (pass) req.friend_cert = pass;
  ws.send(JSON.stringify(req));
}

/**
 * The attach step's line about the person a DM goes to, from their published `accepts_files`:
 * "Ann accepts files only from friends. ..." when their app will ask them first; null when it
 * opens for them as usual, or their choice is not known yet (it is asked for then).
 */
function acceptsFilesLine(key) {
  const F = window.HosFilesInMessages;
  const known = acceptsFilesKnown(key);
  if (!F || !known) return null;
  return F.senderLine(known.shown, known.value, known.friend);
}

/**
 * What the person a DM goes to published about files: {value ("nobody", "friends" or "anyone"),
 * friend (whether they count me one), shown (their name here)}, or null when it is not known yet
 * (it is asked for then). The device-to-device offer reads it too (chat-file-transfer.js).
 */
function acceptsFilesKnown(key) {
  const F = window.HosFilesInMessages;
  if (!F || !key) return null;
  const name = attachProfileName(key);
  const known = name && typeof profileCache !== 'undefined' && profileCache ? profileCache[name.toLowerCase()] : null;
  if (!known || !F.acceptsFilesOf(known.accepts_files)) {
    askAcceptsFiles(key);
    return null;
  }
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  // They count me a friend when we follow each other and they gave me a pass.
  const friend = !!(store && store.isFriendPeer(key) && store.certFor(key));
  const shown = typeof reachDisplayName === 'function' ? reachDisplayName(key) : name;
  return { value: known.accepts_files, friend, shown };
}

// The attach step's title and its two parts' titles (the desktop app's "Send a file",
// src/gui/pages/chat/attach_step.rs).
const ATTACH_STEP_TITLE = 'Send a file';
const ATTACH_STEP_FILE = 'The file';
const ATTACH_STEP_DETAILS = 'Hidden details';

/**
 * The attach step: the file (its type's badge, its name and size: "The file", blue), what is hidden
 * in it, the switch and its lines ("Hidden details", green), then the lines about the person it
 * goes to and the buttons, outside the parts. Built with createElement and textContent (the badge
 * is HosFileRisk.badgeHtml, every text in it escaped). Resolves {send, remove}: send false when
 * Cancel was pressed or the file cannot go.
 */
function showAttachStep(o) {
  const C = window.HosUploadClean;
  const W = C.WORDS;
  return new Promise((resolve) => {
    const old = document.getElementById('attach-step-overlay');
    if (old && typeof old.remove === 'function') old.remove();
    const mk = (tag, cls, text) => {
      const el = document.createElement(tag);
      if (cls) el.className = cls;
      if (text != null) el.textContent = text;
      return el;
    };
    const overlay = mk('div', 'profile-modal-overlay open');
    overlay.id = 'attach-step-overlay';
    overlay.style.zIndex = '10000';
    const card = mk('div', 'profile-modal attach-step');
    card.setAttribute('role', 'dialog');
    card.setAttribute('aria-label', ATTACH_STEP_TITLE);
    card.appendChild(mk('h3', 'attach-step-heading', ATTACH_STEP_TITLE));
    // Two parts in the chat sidebar's colours, darker (/shared/tint-section.css, 2026-10-17; the
    // desktop app's "Send a file", gui/widgets/tint_section.rs): the file, and what is hidden in it.
    const section = (tint, title) => {
      const s = mk('div', 'tint-section tint-' + tint);
      s.appendChild(mk('div', 'tint-section-title', title));
      card.appendChild(s);
      return s;
    };
    const fileSection = section('blue', ATTACH_STEP_FILE);
    const sizeMB = (Number(o.file.size) || 0) / 1024 / 1024;
    const fileRow = mk('div', 'attach-step-file');
    // Its type, just before its name, always (app.js fileBadgeFill): by its name now, then from its
    // first bytes, read on this device, so a file named like a picture over a program's shows red.
    const badge = fileRow.appendChild(mk('span', 'file-badge-slot attach-step-badge'));
    fileRow.appendChild(mk('span', 'attach-step-title', o.file.name));
    fileRow.appendChild(mk('span', 'attach-step-size', sizeMB >= 0.1 ? sizeMB.toFixed(1) + ' MB' : Math.max(1, Math.round((Number(o.file.size) || 0) / 1024)) + ' KB'));
    fileSection.appendChild(fileRow);
    if (typeof fileBadgeFill === 'function') {
      fileBadgeFill(badge, o.file.name, null);
      try {
        if (o.file && typeof o.file.slice === 'function') {
          Promise.resolve(o.file.slice(0, 16).arrayBuffer())
            .then((b) => fileBadgeFill(badge, o.file.name, new Uint8Array(b)))
            .catch(() => { /* not readable: its name's badge stays */ });
        }
      } catch (e) { /* not readable: its name's badge stays */ }
    }
    const details = section('green', ATTACH_STEP_DETAILS);
    if (o.clean.summary) {
      details.appendChild(mk('p', 'attach-step-summary', o.clean.summary));
      const list = mk('ul', 'attach-step-details');
      for (const d of o.clean.details || []) list.appendChild(mk('li', null, d.words));
      details.appendChild(list);
    }
    let box = null;
    if (o.ctx.protectedOn) {
      details.appendChild(mk('p', 'attach-step-protected', W.protectedLine));
    } else {
      const label = mk('label', 'attach-step-switch');
      box = document.createElement('input');
      box.type = 'checkbox';
      box.checked = o.ctx.remove !== false;
      box.dataset.attachStep = 'remove';
      label.appendChild(box);
      label.appendChild(mk('span', null, W.switchLabel));
      details.appendChild(label);
    }
    const offLine = mk('p', 'attach-step-off', '');
    const said = mk('p', 'attach-step-said', '');
    details.appendChild(offLine);
    details.appendChild(said);
    if (o.extraLine) card.appendChild(mk('p', 'attach-step-recipient', o.extraLine));
    // A file over my private limit in a DM goes device to device (chat-file-transfer.js): why, and
    // that the page must stay open; and, when their published choice means their app will not ask
    // them, that too, with Send off.
    if (o.direct) for (const line of o.direct.lines || []) card.appendChild(mk('p', 'attach-step-direct', line));
    const buttons = mk('div', 'attach-step-buttons');
    const send = mk('button', 'vr-btn attach-step-send', W.send);
    send.type = 'button';
    send.dataset.attachStep = 'send';
    const cancel = mk('button', 'vr-btn attach-step-cancel', W.cancel);
    cancel.type = 'button';
    cancel.dataset.attachStep = 'cancel';
    buttons.appendChild(send);
    buttons.appendChild(cancel);
    card.appendChild(buttons);
    overlay.appendChild(card);

    const remove = () => (box ? !!box.checked : true);
    const draw = () => {
      const d = C.decide(o.clean, Object.assign({}, o.ctx, { remove: remove() }));
      offLine.textContent = !o.ctx.protectedOn && !remove() ? W.switchOffLine : '';
      said.textContent = d.words || '';
      send.textContent = d.action === 'ask' ? W.sendAnyway : W.send;
      send.disabled = d.action === 'refuse' || !!(o.direct && o.direct.refuse);
    };
    const done = (yes) => {
      if (typeof overlay.remove === 'function') overlay.remove();
      resolve({ send: !!yes, remove: remove() });
    };
    if (box) box.onchange = draw;
    send.onclick = () => { if (!send.disabled) done(true); };
    cancel.onclick = () => done(false);
    overlay.onclick = (e) => { if (e && e.target === overlay) done(false); };
    draw();
    document.body.appendChild(overlay);
  });
}

/**
 * Clean a file before it leaves and ask what needs asking. `where` is privateConversationNow()'s
 * answer ('' for a public room). Resolves null when the file is not to be sent, else
 * {file, keepMetadata, unchecked}: the file to send (cleaned, or as picked when the person kept
 * the details), whether the upload says keep_metadata=1, and whether its marker says it could not
 * be checked. `opts.direct` ({lines, refuse}): the file goes device to device instead
 * (chat-file-transfer.js offerFileDirect), so the step is always shown, with those lines.
 */
async function prepareAttachment(file, where, opts) {
  const C = window.HosUploadClean;
  if (!C) {
    addSystemMessage('The file was not sent: this page could not load its file checker. Reload the page and try again.');
    return null;
  }
  const storage = attachStorage();
  const ctx = { where: where ? 'private' : 'public', remove: C.removeDetailsChosen(storage), protectedOn: attachProtectedOn() };
  const clean = await C.cleanFile(file);
  const direct = opts && opts.direct ? opts.direct : null;
  // (Device to device, the direct lines say what the recipient's choice means for it.)
  const extraLine = !direct && where && where.startsWith('dm:') ? acceptsFilesLine(where.slice(3)) : null;
  if (!direct && !C.stepNeeded(clean, ctx, extraLine)) {
    const d = C.decide(clean, ctx);
    return d.action === 'send' ? d : null;
  }
  const choice = await showAttachStep({ file, clean, ctx, extraLine, direct });
  if (!choice || !choice.send) return null;
  if (!ctx.protectedOn) C.rememberRemoveDetails(storage, choice.remove);
  const d = C.decide(clean, Object.assign({}, ctx, { remove: choice.remove }));
  return d.action === 'refuse' ? null : d;
}

async function uploadImage(file) {
  // Client-side size guard: abort before uploading anything over my limit here (attachmentLimit).
  if (attachmentTooLarge(file)) return null;
  // The file's name goes to the server with it: never my recovery phrase
  // (step F, chat-warnings.js).
  if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(file && file.name, 'The file was not sent.')) return null;
  // Cleaned on this device first (part 1); a file that could not be checked never goes to a room.
  const ready = await prepareAttachment(file, '');
  if (!ready) return null;
  const indicator = document.getElementById('upload-indicator');
  indicator.textContent = `Uploading ${file.name}…`;
  indicator.style.display = 'block';

  try {
    const formData = new FormData();
    formData.append('file', ready.file, file.name);

    // Shared-file library (v0.675): attaching a 3D/model file in chat is an
    // act of publishing (you are handing people a printable/openable design),
    // so those formats are also listed in the public library page. Ordinary
    // chat media (photos, clips, docs) stays unlisted, exactly as before.
    const SHARE_EXTENSIONS = ['blend', 'stl', 'obj', 'gltf', 'glb'];
    const ext = (file.name.split('.').pop() || '').toLowerCase();
    const shareParam = SHARE_EXTENSIONS.includes(ext) ? '&share=1' : '';
    // The person chose to keep the file's hidden details: the server then keeps them too (B4).
    const keepParam = ready.keepMetadata ? '&keep_metadata=1' : '';
    const uploadBase = uploadBaseUrl();
    if (!uploadBase) return null;
    const resp = await uploadPost(uploadBase + shareParam + keepParam, formData, (f) => { indicator.textContent = uploadingWords(file.name, f); });
    if (!resp.ok) {
      const text = await resp.text();
      // Make error messages more user-friendly
      let friendly = text;
      if (resp.status === 413 || text.includes('too large')) {
        // The server says the person's own limit in its words (2026-10-10);
        // a refusal from in front of it (an HTML page) gets plain words here.
        const sizeMB = (file.size / 1024 / 1024).toFixed(1);
        friendly = text.includes('your limit on this server') ? text : `"${file.name}" (${sizeMB} MB) is too large for this server.`;
      } else if (text.includes('Unsupported')) {
        friendly = `File type not allowed. Supported: images (png, jpg, gif, webp), documents (pdf, txt), audio (mp3, ogg, wav), video (mp4, webm).`;
      }
      addSystemMessage(`Upload failed: ${friendly}`);
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
// Several chosen together (2026-10-16) go one at a time, each its own message, to the
// conversation they were chosen in: leave it and the rest are not sent, since a file chosen
// for a DM must never land anywhere else (the desktop app's attach_send.rs next_queued).
async function handleFileAttachment(event) {
  const files = Array.from(event.target.files || []);
  event.target.value = ''; // Reset for re-selection
  await sendChosenFiles(files);
}

/**
 * Files chosen together, by the attach button or dropped on the page (2026-10-17): the same
 * path for both, one at a time, each its own message, to the conversation they were chosen in.
 */
async function sendChosenFiles(files) {
  if (!files || !files.length) return;
  const chosenIn = conversationNow();
  for (let i = 0; i < files.length; i++) {
    if (conversationNow() !== chosenIn) {
      addSystemMessage(leftBeforeTheRest(files.length - i));
      return;
    }
    await sendAttachment(files[i]);
  }
}

/** The conversation on screen, private or public: what several files chosen together belong to. */
function conversationNow() {
  return privateConversationNow() || ('channel:' + activeChannel);
}

/** What is said when the person leaves a conversation while files chosen for it are still to go. */
function leftBeforeTheRest(n) {
  return n === 1
    ? 'The last file you chose was not sent, because you left the conversation it was for.'
    : 'The last ' + n + ' files you chose were not sent, because you left the conversation they were for.';
}

/**
 * The private conversation on screen: 'dm:<key>' for a direct message,
 * 'group:<id>' for a P2P group, '' for a public channel.
 */
function privateConversationNow() {
  // The scratch pad is local only: a file there is encrypted like a DM's, and
  // its marker (with the key) is kept in this browser, in the encrypted DM
  // store (app.js scratchPadKeep).
  if (typeof isScratchPad === 'function' && isScratchPad()) return 'scratch';
  if (typeof activeDmPartner !== 'undefined' && activeDmPartner) return 'dm:' + activeDmPartner;
  if (window.activeP2pGroup && window.activeP2pGroup.id) return 'group:' + window.activeP2pGroup.id;
  return '';
}

/**
 * Send a picked, pasted or dropped file to the conversation on screen. In a
 * DM or a P2P group the FILE must be as private as the message (2026-08-24
 * for picked files in a DM; 10k, 2026-10-10, for everything else): it is
 * encrypted on this device, only ciphertext is uploaded, and the marker with
 * its key travels inside the sealed DM or the group's encrypted message. The
 * scratch pad, local only, encrypts it the same way and keeps the marker here.
 * In a public channel, public is public: the plain upload and its address.
 */
async function sendAttachment(file) {
  if (!file) return;
  const where = privateConversationNow();
  if (where) {
    // "Show the attach button in private conversations" turned off: no file goes in a DM or a
    // group, picked, pasted or dropped (the scratch pad stays on this device and keeps it).
    if ((where.startsWith('dm:') || where.startsWith('group:')) && !attachInPrivateAllowed()) {
      addSystemMessage(window.HosFilesInMessages.WORDS.attachOff);
      return;
    }
    await sendEncryptedAttachment(file);
    return;
  }
  const url = await uploadImage(file);
  if (url) await window.sendComposedContent(url);
}

/** Encrypt a file, upload the ciphertext, and send it as a marker in the DM or group on screen. */
async function sendEncryptedAttachment(file) {
  // The conversation it goes to, fixed now: never a public channel (the marker
  // holds the key, so it would make the file public).
  const target = privateConversationNow();
  if (!target) return;
  // A file over my role's private limit in a DM never goes to the server's disk: it goes straight
  // to their device once they say yes (device-to-device big files, 2026-10-11,
  // chat-file-transfer.js offerFileDirect). In a group or the scratch pad it is refused with the
  // limit's words (attachmentLimit), which the server holds an encrypted upload to.
  if (target.startsWith('dm:') && file && file.size > attachmentLimit() && typeof offerFileDirect === 'function') {
    await offerFileDirect(file, target.slice(3));
    return;
  }
  if (attachmentTooLarge(file)) return;
  // The file's name rides in the sealed message: never my recovery phrase
  // (step F, chat-warnings.js).
  if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(file && file.name, 'The file was not sent.')) return;
  // A group without its key yet cannot take the message: upload nothing.
  if (target.startsWith('group:') && !(typeof window.p2pGroupCanSend === 'function' && window.p2pGroupCanSend())) return;
  // The scratch pad keeps the marker (the file's key) in the encrypted DM
  // store (10n N8): while that is still loading, upload nothing.
  if (target === 'scratch' && typeof window.scratchPadReady === 'function' && !window.scratchPadReady()) {
    if (typeof SCRATCH_PAD_LOADING === 'string') addSystemMessage(SCRATCH_PAD_LOADING);
    return;
  }
  // This session's token: checked before anything is encrypted or uploaded.
  const uploadBase0 = uploadBaseUrl();
  if (!uploadBase0) return;
  // Cleaned on this device first (part 1): the server only ever receives a sealed blob here, so
  // this is the only place the location can come out. A file that could not be checked goes only
  // on the person's yes, and its marker says so (part 2).
  const ready = await prepareAttachment(file, target);
  if (!ready) return;
  if (privateConversationNow() !== target) {
    addSystemMessage('The file was not sent because another conversation was opened while it was being prepared.');
    return;
  }
  const sending = ready.file;
  const indicator = document.getElementById('upload-indicator');
  try {
    if (indicator) { indicator.textContent = `Encrypting ${file.name}…`; indicator.style.display = 'block'; }
    // WebCrypto's AES-GCM takes the whole file and gives the whole ciphertext, so while it
    // encrypts the page holds both. After that only one copy is kept at a time (round 3,
    // review 7): the plain bytes are let go before the upload, and the ciphertext once the
    // upload's Blob holds its own copy of it.
    let bytes = new Uint8Array(await sending.arrayBuffer());
    const enc = await pqEncryptFile(bytes);
    bytes = null;
    if (indicator) indicator.textContent = `Uploading ${file.name}…`;
    const uploadBase = `${uploadBase0}&encrypted=1`;
    const sealed = new Blob([enc.cipherBytes], { type: 'application/octet-stream' });
    enc.cipherBytes = null;
    const fd = new FormData();
    fd.append('file', sealed, 'attachment.enc');
    const resp = await uploadPost(uploadBase, fd, (f) => { if (indicator) indicator.textContent = uploadingWords(file.name, f); });
    if (!resp.ok) { addSystemMessage(`Upload failed: ${await resp.text()}`); return; }
    const data = await resp.json();
    // The fields in the desktop app's order (src/net/dm_pq.rs DmAttachment), so both apps build
    // the same marker; `unchecked` only when its sender's app could not check it and the person
    // sent it anyway (scripts/tests/fixtures/file-marker-unchecked.json holds both to it).
    const meta = {
      url: data.url,
      k: enc.keyB64,
      n: enc.nonceB64,
      name: file.name,
      mime: file.type || 'application/octet-stream',
      size: sending.size,
    };
    if (ready.unchecked) meta.unchecked = true;
    const marker = pqBuildFileMarker(meta);
    // Another conversation opened while it uploaded: it is not sent there.
    if (privateConversationNow() !== target) {
      addSystemMessage('The file was not sent because another conversation was opened while it uploaded.');
      return;
    }
    // The scratch pad keeps the marker here and sends nothing.
    if (target === 'scratch') {
      window.scratchPadKeep(marker);
      return;
    }
    // Goes through the exact send path as any message: the DM's seal, or the
    // group's encrypted message (chat-ui.js sendComposedContent).
    await window.sendComposedContent(marker);
  } catch (e) {
    addSystemMessage(`Encrypted upload failed: ${e && e.message}`);
  } finally {
    if (indicator) indicator.style.display = 'none';
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
      // To the conversation in view, never unconditionally to the public
      // channel (privacy fix 2026-07-04), and encrypted in a DM or a group (10k).
      await sendAttachment(file);
      return;
    }
  }
});

// ── Files dragged from the computer onto the page (2026-10-17) ──
// The desktop app's drag and drop, in its words (/shared/drop-overlay.js; the desktop's
// src/gui/pages/chat/attach_send.rs offer_drop). While files are dragged over the page it is
// dimmed with one line saying what a drop will do:
//   - one of this server's channels open: "Drop to upload to Files and post in #general", and the
//     drop opens the upload window (/shared/upload-window.js): the Media page's own Add to Media,
//     every control and check, in a window over the chat, "Also post in chat" ticked for that
//     channel; nothing goes until its Upload (the operator, 2026-10-17: "First window popups with
//     all the controls of what to do with the file ... upon clicking upload it also posts the image
//     with the title and description to the general chat"). A read-only channel the person cannot
//     post in: "Drop to upload to Files", nothing ticked.
//   - a private conversation open (a direct message, a group, the scratch pad, or a room this
//     server does not list): "Drop to send in Ben" (the group's name in a group), and the dropped
//     files take the attach button's own path (sendChosenFiles): one at a time, each its own
//     message, encrypted in a DM or a group. A private conversation's file is never offered to a
//     public upload.
// Where a drop would do nothing, the overlay says "Files cannot be dropped here." and why, and the
// drop does nothing. A dragged link or text is left to the browser.

/** Why a file cannot be dropped while this server has not signed you in (or the connection is down). */
const DROP_NOT_SIGNED_IN = 'This server has not finished signing you in yet. Try again in a moment.';

/**
 * The channel a drop now would upload for and post in: {id, name, postable} when one of this
 * server's channels is open (not a DM, a group, the scratch pad), else null. `postable`: the person
 * may post there (a read-only channel only for an admin, as Add to Media's channel list has it).
 */
function dropChannelNow() {
  if (privateConversationNow()) return null;
  const ch = (typeof channelList !== 'undefined' && Array.isArray(channelList)) ? channelList.find((c) => c.id === activeChannel) : null;
  if (!ch) return null;
  const myRole = (typeof peerData === 'object' && peerData && peerData[myKey] && peerData[myKey].role) || '';
  return { id: ch.id, name: '#' + ch.name, postable: !ch.read_only || myRole === 'admin' || myRole === 'owner' };
}

// The upload window over the chat (/shared/upload-window.js), made the first time a drop opens it.
let chatUploadWindowHost = null;
function chatUploadWindow() {
  if (!chatUploadWindowHost && window.HosUploadWindow) {
    chatUploadWindowHost = window.HosUploadWindow.createHost({
      window,
      document,
      // What the window says while it is closed and an upload carries on in it (published, posted
      // in #general): a notice here.
      onStatus: (text) => { if (typeof addNotice === 'function') addNotice(text, 'green', 10); },
    });
  }
  return chatUploadWindowHost;
}

/** How the chat names the conversation on screen: "#general", the person's name, the group's name. */
function conversationLabel() {
  if (typeof isScratchPad === 'function' && isScratchPad()) return '#scratch-pad';
  if (typeof activeDmPartner !== 'undefined' && activeDmPartner) {
    const known = typeof peerData === 'object' && peerData && peerData[activeDmPartner];
    return (typeof activeDmPartnerName === 'string' && activeDmPartnerName) || (known && known.name) || activeDmPartner.slice(0, 8);
  }
  const group = window.activeP2pGroup;
  if (group && group.id) return group.name || String(group.id).slice(0, 8);
  const ch = (typeof channelList !== 'undefined' && Array.isArray(channelList)) ? channelList.find((c) => c.id === activeChannel) : null;
  return '#' + (ch ? ch.name : activeChannel);
}

/** What a file dropped now would do: {take, line} or {take: false, reason} (drop-overlay.js). */
function chatDropDecision() {
  const D = window.HosDropOverlay;
  const channel = dropChannelNow();
  if (channel && window.HosUploadWindow) {
    // The upload window signs in to this server itself; it needs the identity this browser keeps.
    if (!myKey) return { take: false, reason: DROP_NOT_SIGNED_IN };
    return { take: true, line: channel.postable ? D.uploadAndPostIn(channel.name) : D.WORDS.uploadToFiles };
  }
  const signedIn = identityConfirmed && !!myUploadToken && typeof ws !== 'undefined' && ws && ws.readyState === 1;
  if (!signedIn) return { take: false, reason: DROP_NOT_SIGNED_IN };
  const where = privateConversationNow();
  // "Show the attach button in private conversations" turned off: its own words (Files in messages).
  if ((where.startsWith('dm:') || where.startsWith('group:')) && !attachInPrivateAllowed()) {
    return { take: false, reason: window.HosFilesInMessages.WORDS.attachOff };
  }
  return { take: true, line: D.sendIn(conversationLabel()) };
}

/** The dropped files: the upload window over a channel, the private send anywhere else (above). */
function chatDropped(files) {
  const channel = dropChannelNow();
  const host = channel ? chatUploadWindow() : null;
  if (host) {
    host.show(files, channel.postable ? channel.id : '');
    return;
  }
  sendChosenFiles(files);
}

if (window.HosDropOverlay) {
  window.HosDropOverlay.attach({
    window,
    document,
    decide: chatDropDecision,
    onDrop: chatDropped,
  });
}

// ── Thread panel functions ──
let currentThread = null; // { from, timestamp, author, body }

// The words each row of the message list was drawn from (app.js addChatMessage), so a thread
// opened from a reply count that came later (updateThreadBadge) shows its parent as it was
// written, a Media item's address included, not the row's text with the card's address taken
// out of it (2026-10-18).
const threadParentWords = new WeakMap();
function rememberThreadParent(row, body) {
  if (row && typeof row === 'object' && typeof body === 'string') threadParentWords.set(row, body);
}

/**
 * The Media cards a thread message's text gets (2026-10-18), by the message list's own rule
 * (app.js addChatMessage): chat-files-card.js filesCardsPlan, which takes this site's item
 * addresses out of the words and draws no card where the protected setup holds back a picture or
 * file link from someone who is not a friend; none for Heron's lists in #todo, which the list
 * draws as lists. Null when there is no card.
 */
function threadFilesPlan(body, fromKey, author) {
  if (typeof filesCardsPlan !== 'function' || typeof filesCardsPut !== 'function') return null;
  const heronList = activeChannel === 'todo' && !!fromKey && String(fromKey).startsWith('bot_') && (author === 'Heron 🪶' || author === 'Heron');
  if (heronList) return null;
  const plan = filesCardsPlan(body, fromKey);
  return plan && plan.ids && plan.ids.length ? plan : null;
}

/**
 * Put one message at the end of the thread panel `panel` (2026-10-18): its author, time and
 * words, then, as in the message list, a Media item's card just after the words with its address
 * out of them, whether it is open remembered under the list's own key (the sender, the time, the
 * item: a card opened in the list is open here too). A row of its own, so the cards already in the
 * panel keep their buttons (the panel's HTML was written again for each reply, which drew them
 * dead). `m`: {from, author, timestamp, body, parent}. Returns the row.
 */
function putThreadMessage(panel, m) {
  const media = threadFilesPlan(m.body, m.from, m.author);
  const row = document.createElement('div');
  row.className = m.parent ? 'thread-msg thread-parent' : 'thread-msg';
  row.innerHTML = `<span class="thread-msg-author">${esc(m.author)}</span>
    <span class="thread-msg-time">${formatTime(m.timestamp)}</span>
    <div class="thread-msg-body">${formatBody(media ? media.words : m.body, m.from)}</div>`;
  panel.appendChild(row);
  if (typeof fillFileCards === 'function') fillFileCards(row);
  if (media) filesCardsPut(row, media.ids, m.from, m.timestamp);
  return row;
}

/** A line of the thread panel's own ("Loading replies...", "No replies yet. Be the first!"). */
function threadNote(panel, text, css, cls) {
  const note = document.createElement('div');
  if (cls) note.className = cls;
  note.style.cssText = css;
  note.textContent = text;
  panel.appendChild(note);
  return note;
}

function openThreadPanel(fromKey, timestamp, author, body) {
  currentThread = { from: fromKey, timestamp: Number(timestamp), author, body };
  const panel = document.getElementById('thread-panel');
  panel.classList.add('open');
  // Show parent message.
  const messagesDiv = document.getElementById('thread-panel-messages');
  messagesDiv.innerHTML = '';
  putThreadMessage(messagesDiv, { from: fromKey, author, timestamp: currentThread.timestamp, body, parent: true });
  threadNote(messagesDiv, 'Loading replies...', 'font-size:0.72rem;color:var(--text-muted);margin-bottom:var(--space-md);');
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
  // Replies from someone I blocked are not shown (by key, step C).
  messages = (messages || []).filter((m) => !(m && fromBlockedKey(m.from)));
  // Keep parent, rebuild replies; each with its Media cards, as in the message list.
  messagesDiv.innerHTML = '';
  const t = currentThread;
  putThreadMessage(messagesDiv, { from: t.from, author: t.author, timestamp: t.timestamp, body: t.body, parent: true });
  if (messages.length === 0) {
    threadNote(messagesDiv, 'No replies yet. Be the first!', 'font-size:0.8rem;color:var(--text-muted);padding:var(--space-md);', 'thread-none');
  } else {
    for (const m of messages) {
      putThreadMessage(messagesDiv, { from: m.from, author: m.from_name || 'Unknown', timestamp: m.timestamp, body: m.content });
    }
  }
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
  if (window.twemoji) twemoji.parse(messagesDiv);
}

async function sendThreadReply() {
  const input = document.getElementById('thread-input');
  const content = input.value.trim();
  if (!content || !ws || ws.readyState !== WebSocket.OPEN || !currentThread) return;
  // Never my recovery phrase (step F, chat-warnings.js): the reply stays in its box to edit.
  if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(content)) return;
  // /friend-code and /redeem <code> typed here would reach the relay as commands
  // too: they take the buttons' gated paths (chat-social.js), as in the composer.
  const typed = typeof protectedTypedCommand === 'function' ? protectedTypedCommand(content) : null;
  if (typed) {
    input.value = '';
    if (typed.command === 'friend-code') sendFriendCodeRequest();
    else redeemFriendCode(typed.code);
    return;
  }

  const timestamp = Date.now();

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
      // Never a file's marker, nor any part of one: it holds the file's key (app.js replyQuoteText).
      content: typeof replyQuoteText === 'function' ? replyQuoteText(currentThread.body) : '',
      timestamp: currentThread.timestamp,
    },
  };
  // Full-PQ: Dilithium3 `pq_signature` only (no Ed25519 chat sig).
  const pqSig = await pqSignChatMessage(content, timestamp);
  if (pqSig) msg.pq_signature = pqSig;
  ws.send(JSON.stringify(msg));

  // Add to thread panel immediately, as a row of its own (with its Media cards): the rows already
  // there, their cards included, stay as they are.
  const messagesDiv = document.getElementById('thread-panel-messages');
  const none = typeof messagesDiv.querySelector === 'function' ? messagesDiv.querySelector('.thread-none') : null;
  if (none && typeof none.remove === 'function') none.remove();
  const row = putThreadMessage(messagesDiv, { from: myKey, author: myName, timestamp, body: content });
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
  if (window.twemoji) twemoji.parse(row);

  // Also add to main chat.
  const key = myKey + ':' + timestamp;
  seenTimestamps.add(key);
  addChatMessage(myName, content, timestamp, myKey, false, !!pqSig, msg.reply_to, null);

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
      // Find parent message content: the words it was drawn from (a Media item's address
      // included, so its card shows in the thread too), else its text as shown.
      const bodyEl = parentEl.querySelector('.body');
      const authorEl = parentEl.querySelector('.author');
      const written = threadParentWords.get(parentEl);
      openThreadPanel(parentFrom, parentTimestamp, authorEl ? authorEl.textContent : 'Unknown', typeof written === 'string' ? written : (bodyEl ? bodyEl.textContent : ''));
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
  // Nothing while the scratch pad is open (10m R10): what is typed there goes
  // to no one, so no one is told someone is typing.
  if (typeof scratchPadOpen === 'function' && scratchPadOpen()) return;
  // Throttle: send at most once every 2 seconds.
  const now = Date.now();
  if (now - lastTypingSent < 2000) return;
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  lastTypingSent = now;
  ws.send(JSON.stringify({ type: 'typing', from: myKey, from_name: myName }));
}
