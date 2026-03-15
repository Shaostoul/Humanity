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
    const unread = groupUnread[g.id] || 0;
    const badge = unread > 0 ? `<span style="background:var(--accent);color:#fff;border-radius:10px;padding:1px 6px;font-size:0.65rem;font-weight:700;margin-left:auto;">${unread}</span>` : `<span style="font-size:0.6rem;color:var(--text-muted);margin-left:auto;">${g.role}</span>`;
    html += `<div class="channel-item${isActive ? ' active' : ''}" data-group-id="${g.id}" style="cursor:pointer;">
      <span style="opacity:0.6">👥 </span>${esc(g.name)}
      ${badge}
    </div>`;
  }
  html += '<div style="display:flex;gap:0.25rem;padding:0.3rem 0;">'
       + '<button class="vr-btn" onclick="promptCreateGroup()" style="flex:1;font-size:0.7rem;">+ Create Group</button>'
       + '<button class="vr-btn" onclick="promptJoinGroup()" style="flex:1;font-size:0.7rem;">+ Join Group</button>'
       + '</div>';
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
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
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
  groupUnread[groupId] = 0; // Clear unread on enter
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
  div.className = 'message';
  const time = new Date(timestamp);
  const timeStr = time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  div.innerHTML = `<div class="meta"><span class="author${isYou ? ' you' : ''}">${esc(name)}</span><span class="timestamp">${timeStr}</span></div><div class="body">${esc(content)}</div>`;
  messagesDiv.appendChild(div);
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
}
