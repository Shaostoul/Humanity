// ── chat-social.js ────────────────────────────────────────────────────────
// Follow/friend system, groups, friend indicators on peer list.
// Depends on: app.js globals (ws, myKey, myName, peerData, esc,
//   updateUserList, switchChannel, openDmConversation)
// ─────────────────────────────────────────────────────────────────────────

// ── Follow/Friend System (Client State) ──
let myFollowing = new Set(); // keys I'm following
let myFollowers = new Set(); // keys following me
let activeGroupId = null; // Currently viewing group
let activeGroupName = '';
let myGroups = []; // Array of { id, name, invite_code, role }
let groupMembersByGroup = {}; // group_id -> [{ key, role }]
let groupUnread = {}; // group_id -> unread message count

function isFriend(key) {
  return myFollowing.has(key) && myFollowers.has(key);
}

/** Send a friend_code_request to the relay; response arrives as friend_code_response. */
function sendFriendCodeRequest() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'friend_code_request' }));
  }
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
  if (msg.type === 'friend_code_response') {
    // Server generated a one-time friend code for us to share.
    const code = msg.code || '';
    const expires = msg.expires_at ? new Date(msg.expires_at).toLocaleString() : '24h';
    addSystemMessage(`🤝 Your friend code: <strong style="font-family:monospace;color:var(--accent);">${esc(code)}</strong> (expires ${expires}). Share it with someone; they can use /redeem ${esc(code)} to auto-follow each other.`);
    navigator.clipboard?.writeText(code);
    return;
  }
  if (msg.type === 'friend_code_result') {
    if (msg.success) {
      const name = esc(msg.name || 'them');
      addSystemMessage(`🤝 Friend code redeemed! You and ${name} now follow each other.`);
      // Refresh follow list
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'chat', content: '/friends', channel: activeChannel || 'general', from: myKey, from_name: myName, timestamp: Date.now() }));
      }
    } else {
      addSystemMessage(`⚠️ Friend code failed: ${esc(msg.message || 'Unknown error')}`);
    }
    return;
  }
  if (msg.type === 'follow_list') {
    myFollowing = new Set(msg.following || []);
    myFollowers = new Set(msg.followers || []);
    updateFriendIndicators();
    if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
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
    if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
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
    } else {
      // Track unread count for groups not currently in view
      groupUnread[msg.group_id] = (groupUnread[msg.group_id] || 0) + 1;
      renderGroupList();
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
  // Update friend/follow icons and role badges next to peers in the peer list
  document.querySelectorAll('.peer[data-pubkey]').forEach(el => {
    const key = el.dataset.pubkey;
    if (!key || key === myKey) return;
    // Remove old indicators
    el.querySelectorAll('.follow-indicator').forEach(x => x.remove());
    // Remove old streaming badges (re-applied below if still active)
    el.querySelectorAll('.role-streaming').forEach(x => x.remove());

    // Add streaming LIVE badge if peer's profile has streaming_live set
    if (typeof streamingBadge === 'function') {
      const peer = peerData[key];
      const isLive = peer && peer.streaming_live;
      if (isLive) {
        const wrapper = document.createElement('span');
        wrapper.innerHTML = streamingBadge(true);
        const liveEl = wrapper.firstElementChild;
        if (liveEl) el.appendChild(liveEl);
      }
    }

    // Friend/follow indicators
    if (isFriend(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.innerHTML = ' ' + hosIcon('users', 14);
      badge.title = 'Friend (mutual follow)';
      el.querySelector('.peer-name')?.appendChild(badge) || el.appendChild(badge);
    } else if (isFollowing(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.innerHTML = ' ' + hosIcon('eye', 14);
      badge.title = 'Following';
      el.querySelector('.peer-name')?.appendChild(badge) || el.appendChild(badge);
    } else if (myFollowers.has(key)) {
      const badge = document.createElement('span');
      badge.className = 'follow-indicator';
      badge.innerHTML = ' ' + hosIcon('eye', 14);
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
      menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:4px 0;min-width:140px;box-shadow:0 4px 12px rgba(0,0,0,0.3);';
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';

      const following = myFollowing.has(key);
      const item = document.createElement('div');
      item.style.cssText = 'padding:6px 12px;cursor:pointer;font-size:0.82rem;color:var(--text);';
      item.innerHTML = following ? hosIcon('close', 14) + ' Unfollow' : hosIcon('eye', 14) + ' Follow';
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
  // Lazily fetch P2P (sovereign signed-object) groups once identity is ready;
  // create/join re-fetch via window.loadP2pGroups(). These render above the
  // legacy relay-mediated ones. NOTE: gate on myKey — loadP2pGroups bails
  // without it, and it sets _p2pGroupsFetched itself only on a real attempt,
  // so we retry on the next render once identity loads (connect() also kicks
  // a proactive load). Without the myKey gate here the flag burned before
  // identity → "no groups until I interact" bug.
  if (!window._p2pGroupsFetched && typeof window.loadP2pGroups === 'function'
      && typeof myKey === 'string' && myKey) {
    window.loadP2pGroups();
  }
  const p2pGroups = window._p2pGroups || [];
  let html = '';
  // P2P groups (the new model). Click → switch the main chat to this group
  // (same surface as switching channels). Right-click → "Copy invite ticket".
  const activeP2p = window.activeP2pGroup;
  for (const g of p2pGroups) {
    const isActiveP2p = !!(activeP2p && activeP2p.id === g.group_id);
    // Crown = a group I created (own), vs one I merely joined. Gold tint;
    // sits just left of the name like a little ownership badge.
    const crown = g.is_creator
      ? `<span title="You created this group" style="margin-right:3px;display:inline-flex;vertical-align:middle;">${hosIcon('crown', 13, 'var(--warning)')}</span>`
      : '';
    html += `<div class="channel-item${isActiveP2p ? ' active' : ''}" data-p2p-group-id="${esc(g.group_id)}" style="cursor:pointer;">
      <span style="opacity:0.6">${hosIcon('users', 16)} </span>${crown}${esc(g.name)}
      <span style="font-size:0.6rem;color:var(--text-muted);margin-left:auto;">${(g.members || []).length}</span>
    </div>`;
  }
  // Legacy relay-mediated groups (shown until migrated — Phase 1 step e).
  for (const g of myGroups) {
    const isActive = activeGroupId === g.id;
    const unread = groupUnread[g.id] || 0;
    const badge = unread > 0 ? `<span style="background:var(--accent);color:#fff;border-radius:var(--radius-lg);padding:1px 6px;font-size:0.65rem;font-weight:700;margin-left:auto;">${unread}</span>` : `<span style="font-size:0.6rem;color:var(--text-muted);margin-left:auto;">${g.role}</span>`;
    html += `<div class="channel-item${isActive ? ' active' : ''}" data-group-id="${g.id}" style="cursor:pointer;">
      <span style="opacity:0.6">${hosIcon('users', 16)} </span>${esc(g.name)}
      ${badge}
    </div>`;
  }
  if (p2pGroups.length === 0 && myGroups.length === 0) {
    html += '<div style="padding:var(--space-md);color:var(--text-muted);font-size:0.8rem;">No groups yet. Create one, or paste an invite ticket to join.</div>';
  }
  html += '<div style="display:flex;gap:var(--space-sm);padding:var(--space-sm) 0;">'
       + '<button class="vr-btn" onclick="promptCreateGroup()" style="flex:1;font-size:0.7rem;">+ Create Group</button>'
       + '<button class="vr-btn" onclick="promptJoinGroup()" style="flex:1;font-size:0.7rem;">+ Join Group</button>'
       + '</div>';
  container.innerHTML = html;
  // P2P group rows → switch the main chat to this group (channel-style).
  // Right-click → context menu with "Copy invite ticket" (no modal, no z-order
  // bugs — the menu is a tiny absolutely-positioned div that dismisses on
  // outside click, same pattern the legacy group menu uses below).
  container.querySelectorAll('[data-p2p-group-id]').forEach(el => {
    el.onclick = () => {
      const gid = el.dataset.p2pGroupId;
      const g = (window._p2pGroups || []).find(x => x.group_id === gid);
      if (g && typeof window.openP2pGroup === 'function') window.openP2pGroup(gid, g.name);
    };
    el.oncontextmenu = (e) => {
      e.preventDefault();
      document.querySelectorAll('.group-ctx-menu').forEach(m => m.remove());
      const gid = el.dataset.p2pGroupId;
      const g = (window._p2pGroups || []).find(x => x.group_id === gid);
      if (!g) return;
      const menu = document.createElement('div');
      menu.className = 'group-ctx-menu';
      menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:4px 0;min-width:180px;box-shadow:0 4px 12px rgba(0,0,0,0.3);';
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';
      const items = [
        { label: hosIcon('copy', 14) + ' Copy invite ticket', html: true, action: async () => {
          if (typeof window.createP2pInvite !== 'function') return;
          try {
            const ticket = await window.createP2pInvite(gid, g.name);
            if (!ticket) return;
            try {
              await navigator.clipboard.writeText(ticket);
              if (typeof addSystemMessage === 'function') addSystemMessage('Invite ticket copied. Share within 7 days.');
            } catch {
              window.prompt('Copy this invite ticket (Ctrl+C):', ticket);
            }
          } catch (err) {
            if (typeof addNotice === 'function') addNotice('Invite failed: ' + err.message, 'red', 6);
          }
        }},
        // Leave — available to anyone. Removes me from the roster (self-leave).
        { label: '🚪 Leave group', action: () => {
          if (!confirm('Leave group "' + g.name + '"? You can rejoin with a new invite ticket.')) return;
          if (typeof window.leaveP2pGroup !== 'function') return;
          window.leaveP2pGroup(gid).catch((err) => {
            if (typeof addNotice === 'function') addNotice('Leave failed: ' + err.message, 'red', 6);
          });
        }},
      ];
      // Disband — creator only (relay enforces; we hide it for non-creators to
      // avoid a confusing silent no-op). is_creator comes from /api/v2/groups.
      if (g.is_creator) {
        items.push({ label: hosIcon('trash', 14) + ' Disband group (for everyone)', html: true, action: () => {
          if (!confirm('Disband "' + g.name + '" for EVERYONE? This cannot be undone.')) return;
          if (typeof window.disbandP2pGroup !== 'function') return;
          window.disbandP2pGroup(gid).catch((err) => {
            if (typeof addNotice === 'function') addNotice('Disband failed: ' + err.message, 'red', 6);
          });
        }});
      }
      items.forEach(it => {
        const div = document.createElement('div');
        div.style.cssText = 'padding:6px 12px;cursor:pointer;font-size:0.82rem;color:var(--text);';
        if (it.html) div.innerHTML = it.label; else div.textContent = it.label;
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
      menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:4px 0;min-width:150px;box-shadow:0 4px 12px rgba(0,0,0,0.3);';
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';
      const items = [
        { label: hosIcon('copy', 14) + ' Copy Invite Code', html: true, action: () => { navigator.clipboard.writeText(group.invite_code).then(() => addSystemMessage('Invite code copied: ' + group.invite_code)); }},
        { label: '👤 Invite User', action: () => { const name = prompt('Share this invite code with a user:\\n' + group.invite_code + '\\n\\nOr enter a username to tell them:'); if (name && name.trim()) { addSystemMessage('Share this invite code with ' + name.trim() + ': ' + group.invite_code); } }},
        { label: '🚪 Leave Group', action: () => { if (confirm('Leave group "' + group.name + '"?') && ws && ws.readyState === WebSocket.OPEN) { ws.send(JSON.stringify({ type: 'group_leave', group_id: gid })); if (activeGroupId === gid) { activeGroupId = null; activeGroupName = ''; } } }},
      ];
      items.forEach(it => {
        const div = document.createElement('div');
        div.style.cssText = 'padding:6px 12px;cursor:pointer;font-size:0.82rem;color:var(--text);';
        if (it.html) div.innerHTML = it.label; else div.textContent = it.label;
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
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}

// Create/join now use the P2P signed-object model (docs/design/p2p-groups.md):
// a group is a sovereign signed object, and joining uses a creator-signed invite
// ticket (works even when the creator is offline). The old relay-mediated
// group_create/group_join WS path is retired here (legacy groups still render
// until migrated — Phase 1 step e).
// One radio option (with pros/cons) for the create-group history choice.
function _p2pgHistoryOption(value, checked, title, desc, pros, cons) {
  const list = (items, sym, color) => items.map((t) =>
    '<li style="margin:2px 0;"><span style="color:' + color + ';font-weight:700;">' + sym + '</span> ' + esc(t) + '</li>').join('');
  return '<label style="display:block;border:1px solid var(--border,#333);border-radius:8px;padding:10px 12px;margin-bottom:8px;cursor:pointer;">' +
    '<div style="display:flex;align-items:center;gap:8px;">' +
      '<input type="radio" name="p2pg-history" value="' + value + '"' + (checked ? ' checked' : '') + '>' +
      '<span style="font-weight:600;">' + esc(title) + '</span>' +
    '</div>' +
    '<div style="margin:4px 0 6px 24px;color:var(--text-muted,#aaa);font-size:0.8rem;">' + esc(desc) + '</div>' +
    '<ul style="margin:0 0 0 24px;padding-left:14px;font-size:0.76rem;list-style:none;color:var(--text-muted,#aaa);">' +
      list(pros, '✓', 'var(--success,#4caf50)') + list(cons, '✕', 'var(--danger,#e57373)') +
    '</ul>' +
  '</label>';
}

// Create-group modal: name + history policy (with pros/cons). A plain prompt()
// can't show the choice, and the operator asked for it on the create window.
function promptCreateGroup() {
  if (typeof window.createP2pGroup !== 'function') return;
  const old = document.getElementById('p2pg-create-modal');
  if (old) old.remove();

  const overlay = document.createElement('div');
  overlay.id = 'p2pg-create-modal';
  // The card is a CHILD of the backdrop, so it always renders above it.
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.6);z-index:10000;display:flex;align-items:center;justify-content:center;';

  const card = document.createElement('div');
  card.style.cssText = 'background:var(--bg-elevated,#1b1b1b);color:var(--text-primary,#eee);border:1px solid var(--border,#333);border-radius:10px;max-width:460px;width:92%;padding:20px;box-shadow:0 8px 40px rgba(0,0,0,0.5);';
  card.innerHTML =
    '<h3 style="margin:0 0 12px;font-size:1.05rem;">Create group</h3>' +
    '<input id="p2pg-name" type="text" placeholder="Group name" autocomplete="off" ' +
      'style="width:100%;box-sizing:border-box;padding:9px 11px;border-radius:7px;border:1px solid var(--border,#333);background:var(--bg,#111);color:var(--text-primary,#eee);font-size:0.95rem;margin-bottom:16px;">' +
    '<div style="font-weight:600;margin-bottom:8px;font-size:0.85rem;">Message history for people who join later</div>' +
    _p2pgHistoryOption('private', true, 'Private (default)',
      'New members only see messages sent after they join.',
      ['Past conversations stay between who was there', 'Stronger forward secrecy — the group re-keys on each join'],
      ['Newcomers start with no context']) +
    _p2pgHistoryOption('shared', false, 'Shared history',
      'New members can read the full history from before they joined.',
      ['Newcomers get full context — good for onboarding'],
      ['Anyone invited later can read everything said earlier', 'Weaker forward secrecy — the key is not rotated on join']) +
    '<div style="display:flex;gap:8px;justify-content:flex-end;margin-top:16px;">' +
      '<button id="p2pg-cancel" class="vr-btn" style="font-size:0.85rem;">Cancel</button>' +
      '<button id="p2pg-create" class="vr-btn" style="font-size:0.85rem;background:var(--accent,#4a9);color:#fff;">Create group</button>' +
    '</div>';
  overlay.appendChild(card);
  document.body.appendChild(overlay);

  const nameInput = card.querySelector('#p2pg-name');
  try { nameInput.focus(); } catch (_e) {}
  const close = () => overlay.remove();
  const submit = () => {
    const name = (nameInput.value || '').trim();
    if (!name) { try { nameInput.focus(); } catch (_e) {} return; }
    const sharedEl = card.querySelector('input[name="p2pg-history"][value="shared"]');
    const shared = !!(sharedEl && sharedEl.checked);
    close();
    window.createP2pGroup(name, shared).catch((e) => {
      if (typeof addNotice === 'function') addNotice('Create failed: ' + e.message, 'red', 6);
    });
  };
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  card.querySelector('#p2pg-cancel').onclick = close;
  card.querySelector('#p2pg-create').onclick = submit;
  nameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); submit(); }
    else if (e.key === 'Escape') { e.preventDefault(); close(); }
  });
}

function promptJoinGroup() {
  const ticket = prompt('Paste your group invite ticket:');
  if (!ticket || !ticket.trim()) return;
  if (typeof window.joinP2pGroupByTicket !== 'function') return;
  window.joinP2pGroupByTicket(ticket.trim()).catch((e) => {
    if (typeof addNotice === 'function') addNotice('Join failed: ' + e.message, 'red', 6);
  });
}

function openGroup(groupId) {
  const group = myGroups.find(g => g.id === groupId);
  if (!group) return;
  activeGroupId = groupId;
  activeGroupName = group.name;
  groupUnread[groupId] = 0; // Clear unread on enter
  activeDmPartner = null; // Exit DM view — also deselect server channel + DM highlights
  renderChannelList();
  if (typeof renderDmList === 'function') renderDmList();
  // Update channel header — replace innerHTML fully so leftover DM spans don't linger.
  const header = document.getElementById('channel-header');
  if (header) {
    header.style.display = 'flex';
    header.innerHTML = `<span class="ch-name">${hosIcon('users', 16)} ${esc(group.name)}</span><span class="ch-desc">Group · Invite: ${esc(group.invite_code)}</span>`;
  }
  // Clear messages, set group context (forest-green tint + green stripes), request history.
  const msgsEl = document.getElementById('messages');
  msgsEl.dataset.ctx = 'group';
  if (typeof resetMsgStripe === 'function') resetMsgStripe();
  msgsEl.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:var(--space-xl);font-size:0.8rem;">Loading group history for ' + esc(group.name) + '...</div>';
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

// Patch sendMessage to route to group_msg when a group is active.
// Without this, pressing Enter while in a group view sends to the channel instead.
const _origSendMessageGroup = sendMessage;
sendMessage = async function() {
  if (!activeGroupId) return _origSendMessageGroup();
  const input = document.getElementById('msg-input');
  const content = input.value.trim();
  if (!content || !ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify({ type: 'group_msg', group_id: activeGroupId, content }));
  addMessageToChat(myName, content, Date.now(), true, myKey);
  input.value = '';
  input.style.height = 'auto';
  input.focus();
};

// Helper to add a message to the chat (for groups)
function addMessageToChat(name, content, timestamp, isYou, fromKey) {
  const messagesDiv = document.getElementById('messages');
  const div = document.createElement('div');
  const stripe = (typeof getStripeClass === 'function') ? getStripeClass(fromKey || name) : '';
  div.className = 'message' + (stripe ? ' ' + stripe : '');
  const time = new Date(timestamp);
  const timeStr = time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  div.innerHTML = `<div class="meta"><span class="author${isYou ? ' you' : ''}">${esc(name)}</span><span class="timestamp">${timeStr}</span></div><div class="body">${esc(content)}</div>`;
  messagesDiv.appendChild(div);
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
}
