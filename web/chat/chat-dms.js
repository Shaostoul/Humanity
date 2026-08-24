// ── chat-dms.js ───────────────────────────────────────────────────────────
// DM state, conversation list, and DM message rendering.
// Depends on: app.js (ws, myKey, myName, activeChannel, esc, formatBody,
//   appendMessage, formatTime, generateIdenticon, shortKey, switchChannel,
//   renderChannelList)
// chat-ui.js (isMobile, closeSidebars, switchSidebarTab)
// ─────────────────────────────────────────────────────────────────────────

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
  // Persist recent DMs summary for dashboard widget.
  try {
    const recent = dmConversations.slice(0, 10).map(c => ({
      name: c.partner_name,
      preview: dmSafePreview(c.last_message).slice(0, 80),
      time: c.last_timestamp ? new Date(Number(c.last_timestamp) * (c.last_timestamp < 1e12 ? 1000 : 1)).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }) : '',
      unread: c.unread_count || 0,
    }));
    localStorage.setItem('hos_dm_recent', JSON.stringify(recent));
  } catch {}
}

/** Switch to DM conversation view. */
function openDmConversation(partnerKey, partnerName) {
  if (!partnerKey) return;
  // Resolve best display name: passed arg > peerData > short key
  const resolvedName = partnerName ||
    (window.peerData && window.peerData[partnerKey]?.display_name) ||
    (typeof shortKey === 'function' ? shortKey(partnerKey) : partnerKey.slice(0, 8));
  activeDmPartner = partnerKey;
  activeDmPartnerName = resolvedName;
  // Clear group context so sendMessage doesn't accidentally route to the active group.
  if (typeof activeGroupId !== 'undefined') { activeGroupId = null; activeGroupName = ''; }

  // Ensure conversation appears in DM list immediately, even before server confirms.
  // (A brand-new conversation won't be in dm_list yet, so we seed it locally.)
  upsertDmConversation(partnerKey, resolvedName, '', Date.now(), false);

  // Switch to DMs tab in sidebar.
  if (typeof switchSidebarTab === 'function') switchSidebarTab('dms', true);

  // Clear unread for this conversation and update sidebar highlighting.
  const row = dmConversations.find(c => c.partner_key === partnerKey);
  if (row) row.unread_count = 0;
  renderDmList();
  renderChannelList(); // Deselect server channels
  if (typeof renderGroupList === 'function') renderGroupList(); // Deselect groups

  // Hide pin bar in DM view.
  document.getElementById('pin-bar').style.display = 'none';
  document.getElementById('pin-list').classList.remove('open');

  // Update channel header.
  const header = document.getElementById('channel-header');
  header.innerHTML = `<span class="ch-name" style="cursor:pointer;" onclick="closeDmView()">← Back</span> <span class="ch-name">${hosIcon('chat', 16)} ${esc(partnerName)}</span>`;
  header.style.display = 'block';

  // Clear messages area and set DM context (crimson tint + red stripes).
  const msgsEl = document.getElementById('messages');
  msgsEl.innerHTML = '';
  msgsEl.dataset.ctx = 'dm';
  if (typeof resetMsgStripe === 'function') resetMsgStripe();

  // Enable input.
  const input = document.getElementById('msg-input');
  input.disabled = false;
  input.placeholder = `Message ${partnerName}…`;
  document.getElementById('send-btn').disabled = false;

  // History renders from the LOCAL encrypted store — the relay keeps no
  // DM history any more (sealed-sender: its mailbox is a sender-less
  // delivery window that expires).
  renderDmConversationFromStore(partnerKey);

  if (isMobile()) closeSidebars();
}

/** Render a DM conversation from the LOCAL history store into #messages. */
function renderDmConversationFromStore(partnerKey) {
  const msgsEl = document.getElementById('messages');
  msgsEl.innerHTML = '';
  const banner = document.createElement('div');
  banner.style.cssText = 'text-align:center;font-size:0.7rem;padding:var(--space-sm);color:var(--text-muted);';
  banner.innerHTML = hosIcon('lock', 14) + ' End-to-end encrypted, sender sealed inside (post-quantum). History lives on your devices; the server keeps no readable copy.';
  msgsEl.appendChild(banner);
  if (!(window.hosDmStore && hosDmStore.ready)) return;
  const msgs = hosDmStore.conversation(partnerKey);
  if (msgs.length > 0) {
    const notice = document.createElement('div');
    notice.id = 'history-notice';
    notice.textContent = `── ${msgs.length} earlier messages ──`;
    msgsEl.appendChild(notice);
  }
  for (const m of msgs) {
    const isMe = m.from === myKey;
    const name = isMe
      ? myName
      : ((window.peerData && peerData[m.from]?.display_name) || activeDmPartnerName || shortKey(m.from));
    addDmMessage(name, m.text, m.ts, m.from, m.to, true);
  }
  const last = msgs[msgs.length - 1];
  if (last) hosDmStore.markRead(partnerKey, last.ts);
}

/** Rebuild the sidebar conversation list from the local store. */
function loadDmListFromStore() {
  if (!(window.hosDmStore && hosDmStore.ready)) return;
  const summaries = hosDmStore.summaries();
  const known = new Set(summaries.map(s => s.peer));
  // Preserve locally-seeded entries (brand-new, still-empty conversations).
  const localOnly = dmConversations.filter(c => !known.has(c.partner_key));
  dmConversations = summaries.map(s => ({
    partner_key: s.peer,
    partner_name: (window.peerData && peerData[s.peer]?.display_name)
      || dmConversations.find(c => c.partner_key === s.peer)?.partner_name
      || shortKey(s.peer),
    last_message: (s.lastFromMe ? 'You: ' : '') + dmSafePreview(s.lastText),
    last_timestamp: s.lastTs,
    unread_count: s.unread ? 1 : 0,
  })).concat(localOnly);
  dmConversations.sort((a, b) => Number(b.last_timestamp || 0) - Number(a.last_timestamp || 0));
  renderDmList();
}

/** Sealed-sender privacy control: delete every envelope currently queued
 *  for us server-side (they auto-expire after the server TTL anyway; this
 *  is the immediate scrub). Local history on this device is untouched. */
function purgeServerMailbox() {
  if (!confirm('Delete all encrypted DM envelopes currently stored for you on the server?\n\nMessages already saved on your devices stay. Another device that hasn\'t synced yet won\'t receive what you delete.')) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'dm_purge' }));
  }
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
  const stripe = (typeof getStripeClass === 'function') ? getStripeClass(fromKey || author) : '';
  el.className = 'message dm-message' + (stripe ? ' ' + stripe : '');
  el.dataset.from = fromKey;
  el.dataset.timestamp = timestamp;

  // Native-parity sender grouping (mirrors the main-channel builder).
  const isContinuation = (typeof isMessageContinuation === 'function') && isMessageContinuation(fromKey, timestamp);
  if (isContinuation) el.classList.add('continuation');

  const isMe = fromKey === myKey;

  const isBotMsg2 = fromKey && fromKey.startsWith('bot_');
  const identiconSrc = (!isBotMsg2 && fromKey) ? generateIdenticon(fromKey, 32) : '';
  const identiconHtml = isBotMsg2 ? '<span class="identicon" style="font-size:calc(var(--avatar-size) * 0.75);line-height:var(--avatar-size);text-align:center;">🤖</span>' : (identiconSrc ? `<img src="${identiconSrc}" class="identicon" alt="">` : '');
  const e2eeBadge = isEncrypted ? '<span class="dm-e2ee" title="End-to-end encrypted" style="opacity:0.6;margin-left:var(--space-xs);">' + hosIcon('lock', 12) + '</span>' : '';

  const metaHtml = `<div class="meta"><span class="author${isMe ? ' you' : ''}">${esc(author)}</span></div>`;

  // Encrypted attachment (2026-08-24): a [[hum:file:v1]] marker renders as a
  // decrypt-on-view card, not raw text. The file's ciphertext is public but
  // useless; the key rode in this sealed message.
  const fileMeta = (typeof pqParseFileMarker === 'function') ? pqParseFileMarker(body) : null;
  const bodyHtml = fileMeta ? encAttachmentPlaceholder(fileMeta) : formatBody(body);

  el.innerHTML = messageRowHTML({
    isContinuation,
    identiconHtml,
    metaHtml,
    pillHtml: timestampPillHTML({ time: formatTimePill(timestamp), extra: e2eeBadge }),
    bodyHtml,
  });

  appendMessage(el);
  if (fileMeta) hydrateEncAttachment(el, fileMeta);
  if (window.twemoji) twemoji.parse(el);
}

function _fmtBytes(n) {
  n = Number(n) || 0;
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(0) + ' KB';
  return (n / 1024 / 1024).toFixed(1) + ' MB';
}

/** The card shown before (and instead of, for non-images) decryption. */
function encAttachmentPlaceholder(meta) {
  const isImg = (meta.mime || '').startsWith('image/');
  return `<div class="enc-attach" data-enc="1">
    <div class="enc-attach-head">${hosIcon('lock', 12)} <span>${esc(meta.name || 'file')}</span>
      <span class="enc-attach-size">${_fmtBytes(meta.size)}</span></div>
    <div class="enc-attach-body">${isImg
      ? '<div class="enc-attach-loading">Decrypting image…</div>'
      : '<button class="enc-attach-dl">Decrypt & download</button>'}</div>
  </div>`;
}

/** Fetch the ciphertext, decrypt with the in-envelope key, render/offer it. */
async function hydrateEncAttachment(el, meta) {
  const card = el.querySelector('.enc-attach');
  const bodyEl = card && card.querySelector('.enc-attach-body');
  if (!bodyEl) return;
  const isImg = (meta.mime || '').startsWith('image/');
  try {
    const decryptToBlob = async () => {
      const resp = await fetch(meta.url);
      if (!resp.ok) throw new Error('fetch ' + resp.status);
      const ct = new Uint8Array(await resp.arrayBuffer());
      const plain = await pqDecryptFile(ct, meta.k, meta.n);
      if (!plain) throw new Error('decrypt failed');
      return new Blob([plain], { type: meta.mime || 'application/octet-stream' });
    };
    if (isImg) {
      const blob = await decryptToBlob();
      const url = URL.createObjectURL(blob);
      const img = document.createElement('img');
      img.src = url;
      img.alt = meta.name || 'image';
      img.className = 'enc-attach-img';
      img.loading = 'lazy';
      img.onclick = () => window.open(url, '_blank');
      bodyEl.innerHTML = '';
      bodyEl.appendChild(img);
    } else {
      const btn = bodyEl.querySelector('.enc-attach-dl');
      if (btn) {
        btn.onclick = async () => {
          btn.disabled = true; btn.textContent = 'Decrypting…';
          try {
            const blob = await decryptToBlob();
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url; a.download = meta.name || 'attachment';
            document.body.appendChild(a); a.click(); a.remove();
            setTimeout(() => URL.revokeObjectURL(url), 4000);
            btn.textContent = 'Downloaded';
          } catch (e) {
            btn.disabled = false; btn.textContent = 'Decrypt & download';
            addSystemMessage('Could not decrypt attachment.');
          }
        };
      }
    }
  } catch (e) {
    bodyEl.innerHTML = '<div class="enc-attach-loading">🔒 Attachment unavailable (expired or unreachable).</div>';
  }
}

// DM previews loaded from the zero-knowledge relay arrive as the raw E2EE
// envelope ({"v":1,"r":{...}}), the relay can't decrypt them. Never show that
// ciphertext; collapse it to a lock placeholder (matches the incoming-DM
// handler in app.js and native's clean DM list).
function dmSafePreview(raw) {
  raw = String(raw || '');
  if (/^\s*\{\s*"v"\s*:\s*\d/.test(raw) || raw.includes('"ek_ct') || /"r"\s*:\s*\{/.test(raw)) {
    return '🔒 Encrypted message';
  }
  // Encrypted attachment marker: show a friendly label, never the base64.
  const fm = (typeof pqParseFileMarker === 'function') ? pqParseFileMarker(raw) : null;
  if (fm) return ((fm.mime || '').startsWith('image/') ? '🔒 Photo' : '🔒 ' + (fm.name || 'File'));
  return raw;
}

/** Render the DM conversation list in the sidebar. */
function renderDmList() {
  const list = document.getElementById('dm-list');
  // Sealed-sender scrub control, always available at the foot of the list.
  const purgeRow = '<div class="dm-item" style="opacity:0.7;" onclick="purgeServerMailbox()" title="Deletes the encrypted envelopes queued for you on the server. Local history stays.">'
    + '<span class="dm-name">🗑 Delete my server mailbox</span></div>';
  if (dmConversations.length === 0) {
    list.innerHTML = '<div style="font-size:0.7rem;color:var(--text-muted);padding:var(--space-sm) var(--space-md);">No conversations yet</div>' + purgeRow;
    return;
  }

  list.innerHTML = dmConversations.map(c => {
    const isActive = activeDmPartner === c.partner_key;
    const unread = c.unread_count > 0 ? '<span class="dm-unread"></span>' : '';
    const timeStr = formatTime(c.last_timestamp);
    // Web keeps name-only (+ unread dot) with a right-aligned time, NO
    // message preview: the relay-stored DM body is an opaque E2EE envelope
    // here, so a sidebar preview would mostly render the lock placeholder
    // (original decision: operator, 2026-05-27). NOTE the native app DOES
    // show a decrypted last-message preview under each DM name as of
    // v0.715 (operator-approved, 2026-07-06) — it decrypts on arrival, so
    // its preview is real text. Don't "fix" native back to name-only for
    // parity; the two clients intentionally differ until web can decrypt
    // at list-render time.
    return `<div class="dm-item${isActive ? ' active' : ''}" onclick="openDmConversation('${esc(c.partner_key)}', '${esc(c.partner_name)}')">
      <span class="dm-name">${esc(c.partner_name)} ${unread}</span>
      <span class="dm-time">${timeStr}</span>
    </div>`;
  }).join('') + purgeRow;
  if (window.twemoji) twemoji.parse(list);
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}
