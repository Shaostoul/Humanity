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
  el.innerHTML = messageRowHTML({
    isContinuation,
    identiconHtml,
    metaHtml,
    pillHtml: timestampPillHTML({ time: formatTimePill(timestamp), extra: e2eeBadge }),
    bodyHtml: formatBody(body),
  });

  appendMessage(el);
  if (window.twemoji) twemoji.parse(el);
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
  return raw;
}

/** Render the DM conversation list in the sidebar. */
function renderDmList() {
  const list = document.getElementById('dm-list');
  if (dmConversations.length === 0) {
    list.innerHTML = '<div style="font-size:0.7rem;color:var(--text-muted);padding:var(--space-sm) var(--space-md);">No conversations yet</div>';
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
  }).join('');
  if (window.twemoji) twemoji.parse(list);
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}
