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
      preview: c.last_message ? c.last_message.slice(0, 80) : '',
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

  const time = formatTime(timestamp);
  const isMe = fromKey === myKey;

  const isBotMsg2 = fromKey && fromKey.startsWith('bot_');
  const identiconSrc = (!isBotMsg2 && fromKey) ? generateIdenticon(fromKey, 20) : '';
  const identiconHtml = isBotMsg2 ? '<span class="identicon" style="font-size:18px;line-height:20px;">🤖</span>' : (identiconSrc ? `<img src="${identiconSrc}" class="identicon" alt="">` : '');
  const e2eeBadge = isEncrypted ? '<span title="End-to-end encrypted" style="font-size:0.65rem;opacity:0.6;margin-left:0.3rem;">' + hosIcon('lock', 14) + '</span>' : '';

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
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}
