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
  // A friend code is a way to make a friend: with the protected setup on it
  // needs the PIN (10h, /shared/protected.js). The typed /friend-code comes
  // here too (chat-ui.js), so typing it does not get round the PIN.
  if (typeof protectedTake === 'function' && !protectedTake('friend_code')) {
    protectedAskThen('friend_code', () => sendFriendCodeRequest());
    return;
  }
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'friend_code_request' }));
  }
}

// A friend code redeemed with the PIN while the protected setup is on: the
// person the relay's answer names (friend_code_result's owner_key) is the
// friend the PIN was given for, so following them asks for no second PIN.
// Only the next answer counts, and only after a redeem made with the setup on.
let redeemApprovedPending = false;

/**
 * Redeem a friend code (the typed /redeem <code>, chat-ui.js): it makes a
 * friend, so with the protected setup on it needs the PIN (`friend_code`),
 * like making a code. Sends the relay's own `friend_code_redeem` frame.
 */
function redeemFriendCode(code) {
  code = String(code == null ? '' : code).trim();
  if (!code) {
    addSystemMessage('Usage: /redeem <code>');
    return false;
  }
  if (typeof protectedTake === 'function' && !protectedTake('friend_code')) {
    protectedAskThen('friend_code', () => redeemFriendCode(code));
    return false;
  }
  if (!ws || ws.readyState !== WebSocket.OPEN) return false;
  redeemApprovedPending = typeof protectedIsOn === 'function' && protectedIsOn();
  ws.send(JSON.stringify({ type: 'friend_code_redeem', code }));
  return true;
}
window.redeemFriendCode = redeemFriendCode;

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
      addSystemMessage(`🤝 Friend code accepted! Following ${name} and sending your hello; when they add you back, you'll be friends.`);
      // Follows removal (2026-08-24): the relay no longer creates the
      // edges — the redeemer's client opens the friendship exchange.
      if (msg.owner_key && typeof setFollowLocal === 'function') {
        // Redeemed with the PIN (protected setup): they are the friend it was given for.
        if (redeemApprovedPending && typeof protectedApprove === 'function') protectedApprove(msg.owner_key);
        redeemApprovedPending = false;
        setFollowLocal(msg.owner_key, true);
      }
    } else {
      redeemApprovedPending = false;
      addSystemMessage(`⚠️ Friend code failed: ${esc(msg.message || 'Unknown error')}`);
    }
    return;
  }
  // (follow_list/follow_update handlers removed 2026-08-24: the social
  // graph is client-side, fed by sealed control messages + the local
  // encrypted store. See the social layer at the bottom of this file.)
  // (Legacy group_list/group_message/group_history/group_members handlers
  // removed 2026-08-23: the plaintext relay-group system died server-side;
  // groups are the E2EE P2P signed-object system in chat-groups-p2p.js.)
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
        // Follows removal (2026-08-24): sealed control message, no server edge.
        setFollowLocal(key, !following);
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
  // legacy relay-mediated ones. NOTE: gate on myKey, loadP2pGroups bails
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
    // Reports members sent me about a group I created (10j, chat-reports.js):
    // their count, which opens Settings > Safety where they are listed.
    const reports = typeof groupReportCountFor === 'function' ? groupReportCountFor(g.group_id) : 0;
    const reportCount = reports
      ? `<span class="group-report-count" data-group-reports="${esc(g.group_id)}" title="${esc(groupReportCountTitle(reports))}" aria-label="${esc(groupReportCountTitle(reports))}" style="font-size:0.6rem;font-weight:700;color:var(--danger);border:1px solid var(--danger);border-radius:var(--radius-sm);padding:0 4px;margin-left:var(--space-xs);">${reports}</span>`
      : '';
    html += `<div class="channel-item${isActiveP2p ? ' active' : ''}" data-p2p-group-id="${esc(g.group_id)}" style="cursor:pointer;">
      <span style="opacity:0.6">${hosIcon('users', 16)} </span>${crown}${esc(g.name)}${reportCount}
      <span style="font-size:0.6rem;color:var(--text-muted);margin-left:auto;">${(g.members || []).length}</span>
    </div>`;
  }
  if (p2pGroups.length === 0) {
    html += '<div style="padding:var(--space-md);color:var(--text-muted);font-size:0.8rem;">No groups yet. Create one, or paste an invite ticket to join.</div>';
  }
  html += '<div style="display:flex;gap:var(--space-sm);padding:var(--space-sm) 0;">'
       + '<button class="vr-btn" onclick="promptCreateGroup()" style="flex:1;font-size:0.7rem;">+ Create Group</button>'
       + '<button class="vr-btn" onclick="promptJoinGroup()" style="flex:1;font-size:0.7rem;">+ Join Group</button>'
       + '</div>';
  container.innerHTML = html;
  // P2P group rows → switch the main chat to this group (channel-style).
  // Right-click → context menu with "Copy invite ticket" (no modal, no z-order
  // bugs, the menu is a tiny absolutely-positioned div that dismisses on
  // outside click, same pattern the legacy group menu uses below).
  container.querySelectorAll('[data-p2p-group-id]').forEach(el => {
    el.onclick = (e) => {
      // The report count opens the Safety page, where the reports are (10j).
      const t = e && e.target;
      if (t && typeof t.closest === 'function' && t.closest('[data-group-reports]') && typeof openSafetyPanel === 'function') {
        openSafetyPanel();
        return;
      }
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
        // Leave, available to anyone. Removes me from the roster (self-leave).
        // 3s hold (operator 2026-08-25): a short gate so a stray tap doesn't
        // drop you from a group you're active in; rejoining needs a new invite.
        { label: '🚪 Leave group', action: async () => {
          if (!await holdConfirm('Leave group "' + g.name + '"? You can rejoin with a new invite ticket.', { seconds: 3 })) return;
          if (typeof window.leaveP2pGroup !== 'function') return;
          window.leaveP2pGroup(gid).catch((err) => {
            if (typeof addNotice === 'function') addNotice('Leave failed: ' + err.message, 'red', 6);
          });
        }},
      ];
      // Disband, creator only (relay enforces; we hide it for non-creators to
      // avoid a confusing silent no-op). is_creator comes from /api/v2/groups.
      if (g.is_creator) {
        items.push({ label: hosIcon('trash', 14) + ' Disband group (for everyone)', html: true, action: async () => {
          if (!await holdConfirm('Disband "' + g.name + '" for EVERYONE? This cannot be undone.', { seconds: 5 })) return;
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
  if (typeof window.refreshUnifiedLeftHeaderCounts === 'function') window.refreshUnifiedLeftHeaderCounts();
}

// Create/join now use the P2P signed-object model (docs/design/p2p-groups.md):
// a group is a sovereign signed object, and joining uses a creator-signed invite
// ticket (works even when the creator is offline). The old relay-mediated
// group_create/group_join WS path is retired here (legacy groups still render
// until migrated, Phase 1 step e).
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
      ['Past conversations stay between who was there', 'Stronger forward secrecy, the group re-keys on each join'],
      ['Newcomers start with no context']) +
    _p2pgHistoryOption('shared', false, 'Shared history',
      'New members can read the full history from before they joined.',
      ['Newcomers get full context, good for onboarding'],
      ['Anyone invited later can read everything said earlier', 'Weaker forward secrecy, the key is not rotated on join']) +
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

// (openGroup removed 2026-08-23 with the legacy relay-group system.)

// When switching to a channel, clear group view
const _origSwitchChannelFollow = switchChannel;
switchChannel = function(channelId) {
  activeGroupId = null;
  activeGroupName = '';
  _origSwitchChannelFollow(channelId);
  if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
};

// (Legacy group_msg sendMessage patch removed 2026-08-23; the P2P group
// composer patch lives in chat-groups-p2p.js.)


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

// ── Client-side social graph (follows removal, 2026-08-24) ────────────────
// The server stores no follow edges. Following is local state persisted in
// the encrypted DM store; follow/unfollow notices and friendship
// certificates travel as sealed control messages over the DM mailbox, and
// self-copies keep every device of the same identity in sync.

/** Pull the social sets out of the store into the UI globals. */
function syncSocialFromStore() {
  if (!(window.hosDmStore && hosDmStore.ready)) return;
  myFollowing = new Set(hosDmStore.following);
  myFollowers = new Set(hosDmStore.followers);
  updateFriendIndicators();
  if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
}
window.syncSocialFromStore = syncSocialFromStore;

/**
 * Seal + send one control message (recipient copy + self copy). With `pass`
 * ({serial, may, kind}), the message gives a friendship pass and waits for the
 * server's answer (10l, holdPassPut): its self-copy is held back until then,
 * for every pass (10n N5). `ts`, when given, is the time it was made, which it
 * is signed with (an Unfollow made while not connected keeps its own).
 */
async function sendDmControl(peer, text, ctlCert, pass, ts) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return false;
  const built = await pqBuildDmPuts(text, peer, Number(ts) || Date.now(), ctlCert ? { ctlCert } : undefined);
  if (!built) {
    console.warn('control not sent (no key for peer yet):', text, peer.slice(0, 12));
    return false;
  }
  if (pass && !holdPassPut(peer, built, pass)) return false;
  ws.send(JSON.stringify(built.recipientPut));
  if (!pass) ws.send(JSON.stringify(built.selfPut));
  return true;
}

// ── A pass counts as given only once the server took it (10l, 2026-10-10,
// docs/design/blocking-and-safe-mode.md; the desktop app builds the same
// protocol from that spec). The relay can refuse a put (its burst limit, a
// new account's slower refill, a reach refusal), and it used to say nothing
// about which. So every put that changes friendship state (a pass, a contact
// request's) carries a `ref`, and the relay answers that ref alone:
// `dm_put_ok` once the pass is in the mailbox, `dm_put_refused` when it is not.
// Only `dm_put_ok` records the pass as given and withdraws the ones it
// replaces. A refusal, or no answer in 30 seconds, records nothing and leaves
// the friend owed a pass, which the next sweep sends again, carrying my choice.
//
// The self-copy (how my other devices learn which pass I gave) waits for the
// answer too, for every pass: a device that recorded a pass the friend never
// got would count it as standing, and this page gets its own self-copy back as
// well. (10m R2 sent an untick's self-copy at once, to carry the new choice to
// my other devices; 10n N5 withdrew that: the choice travels in its own note
// now, below, and the early self-copy was how other devices came to record
// passes the server had refused.)

const PASS_ANSWER_WAIT_MS = 30000;
// ref -> {ref, peer, serial, may, kind ('pass' | 'request'), selfPut, timer, onAnswer}
const _passPuts = new Map();

// Friends whose pass the server refused because of their own "who can reach
// me" setting (`dm_put_refused` with reason "reach", 10m R7): two friends whose
// settings both say Friends and who hold no pass from each other cannot give
// each other one, and re-sending on every member list repeated a "Not
// delivered" offer every minute. So for the rest of this page's life the sweep
// does not send them one by itself, until their pass reaches me (ingestDmControl)
// or the person follows, accepts or changes ticks for them (passPersonChoseFor).
const _passReachRefused = new Set();

/** A fresh ref for one put: 1 to 64 characters of [A-Za-z0-9_-] (10l), here "pass-" and 24 hex digits. */
function passPutRef() {
  const bytes = crypto.getRandomValues(new Uint8Array(12));
  return 'pass-' + Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Is a pass put to `peer` waiting for the server's answer? One at a time per friend. */
function passPutInFlight(peer) {
  for (const p of _passPuts.values()) if (p.peer === peer) return true;
  return false;
}

/**
 * Tag a built pass put with a ref and hold it until the server answers:
 * `built.recipientPut` gets the ref (the caller sends it), `built.selfPut` is
 * kept back until `dm_put_ok`. `pass`: {serial, may, kind, onAnswer?};
 * onAnswer({taken, outcome, reason}) runs once: `outcome` is 'ok', 'refused',
 * 'timeout', or 'withdrawn' (an Unfollow or a Block took it back first, 10n
 * N9), `reason` the server's ("rate", "reach", "size", "other") for a refusal,
 * and `taken` is true only when the pass was recorded as given (an 'ok' for a
 * pass withdrawn meanwhile records nothing). Returns false when there is no
 * store to keep it in (nothing is sent then).
 */
function holdPassPut(peer, built, pass) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !built || !built.recipientPut || !pass || !pass.serial) return false;
  const ref = passPutRef();
  built.recipientPut.ref = ref;
  // From here it may be standing on the server, answered or not. Past four
  // unanswered for this friend, the oldest are withdrawn now.
  if (store.passSending(peer, pass.serial, pass.may).length) sendPendingWithdrawals();
  const entry = {
    ref, peer, serial: pass.serial, may: pass.may, kind: pass.kind || 'pass',
    selfPut: built.selfPut || null,
    onAnswer: typeof pass.onAnswer === 'function' ? pass.onAnswer : null, timer: 0,
  };
  entry.timer = setTimeout(() => settlePassPut(ref, 'timeout', 'timeout'), PASS_ANSWER_WAIT_MS);
  _passPuts.set(ref, entry);
  return true;
}

/** Tell a held put's owner how it ended, once. */
function passPutAnswer(p, answer) {
  if (!p || !p.onAnswer) return;
  const fn = p.onAnswer;
  p.onAnswer = null;
  try { fn(answer); } catch (e) { console.warn('pass answer:', e && e.message); }
}

/**
 * A held pass put is settled: `outcome` is 'ok', 'refused' or 'timeout'.
 * Returns true when `ref` was one this page was waiting on.
 */
function settlePassPut(ref, outcome, reason) {
  const p = (typeof ref === 'string') ? _passPuts.get(ref) : null;
  if (!p) return false;
  _passPuts.delete(ref);
  clearTimeout(p.timer);
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  let taken = false;
  if (outcome === 'ok') {
    taken = !!(store && store.passTaken(p.peer, p.serial, p.may));
    if (taken) {
      // Now my other devices may hear of it.
      if (p.selfPut && ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(p.selfPut));
      // The passes it replaces, and any sent earlier with no answer.
      sendPendingWithdrawals();
    }
  } else if (outcome === 'refused') {
    if (store) store.passRefused(p.peer, p.serial);
    // Refused by their own setting (10m R7): not sent again by itself this session.
    if (reason === 'reach' && p.kind !== 'request') _passReachRefused.add(p.peer);
  }
  // No answer in time: nothing is recorded, and the pass stays among those
  // perhaps given (chat-dm-store.js passesUnsure), so a later Unfollow or
  // Block, or the next pass the server does take, withdraws it. A late answer
  // finds nothing here and changes nothing.
  passPutAnswer(p, { taken, outcome, reason: outcome === 'refused' ? (reason || 'other') : null });
  updateFriendIndicators();
  if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
  return true;
}

/** The relay's answer to a put that carried a ref (`dm_put_ok` / `dm_put_refused`, routed by app.js). */
function friendPassPutAnswered(msg) {
  if (!msg || typeof msg.ref !== 'string') return false;
  if (msg.type === 'dm_put_ok') return settlePassPut(msg.ref, 'ok', null);
  if (msg.type === 'dm_put_refused') return settlePassPut(msg.ref, 'refused', typeof msg.reason === 'string' ? msg.reason : 'other');
  return false;
}
window.friendPassPutAnswered = friendPassPutAnswered;
window.holdPassPut = holdPassPut;
window.passPutInFlight = passPutInFlight;

/**
 * The person decided about `peer` on this device (follow, accept, a tick): the
 * holds that keep their pass from being sent by itself go (10m R3 "changed on
 * my other device", R7 "refused by their setting").
 */
function passPersonChoseFor(peer) {
  _passReachRefused.delete(peer);
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (store) store.clearPassChangedElsewhere(peer);
}
window.passPersonChoseFor = passPersonChoseFor;

// ── Friendship passes v2 (2026-10-09, docs/design/blocking-and-safe-mode.md
// 10b), mirroring native src/engine/dm.rs. A pass names this server's did:hum
// (window.hosServerDid, from its identify_challenge), a random serial and what
// the friend may do; unfollowing withdraws it with `cert_revoke {serial}`,
// which the relay honours at once and answers with `cert_revoked`.
//
// What a pass lets a friend do is my CHOICE for them (10n, 2026-10-10): the
// ticks on "People I choose", kept per friend with when it was made
// (chat-dm-store.js passChoice), the same on every device of mine through a
// note to myself ([[hum:choice:v1]]<key>/<may>, /shared/friend-pass.js). The
// passes follow the choice: a pass granting beyond it is withdrawn at once,
// and the sweep gives a friend who holds none carrying it a new one. An echo
// of a pass from my other device is only a pass: it never changes the choice.

const _passMinting = new Set(); // peers a pass is being minted for right now (async)

/**
 * Issue + deliver MY friendship pass to `peer`, carrying my choice for them
 * (the defaults when there is none). Nothing when one carrying it stands, or a
 * pass to them is on its way (one at a time, 10l), or they are blocked, or my
 * other device withdrew theirs and its choice has not reached this one (10m
 * R3: the person's own follow, accept or tick here clears that first). Returns
 * true when it was sent; it counts as given once the server takes it (10l,
 * holdPassPut).
 */
async function sendFriendCertTo(peer) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !store.passOwed(peer) || _passMinting.has(peer) || passPutInFlight(peer)) return false;
  if (store.isBlocked(peer)) return false; // never a pass to someone I blocked (step C)
  if (store.passChangedOnOtherDevice(peer)) return false;
  // With the protected setup on, a pass goes only to someone the PIN holder
  // let be a friend (10h): so a follow made before the setup and followed
  // back after it does not make a friend without the PIN. They can still send
  // a contact request, whose Accept needs the PIN.
  if (typeof protectedPassAllowed === 'function' && !protectedPassAllowed(peer)) return false;
  if (!window.hosServerDid || store.passServer !== window.hosServerDid) return false; // swept again once known
  if (typeof getPeerEcdhPublic === 'function' && !getPeerEcdhPublic(peer)) return false; // cannot seal to them yet
  _passMinting.add(peer);
  try {
    const built = await pqBuildFriendCert(peer, store.choiceMay(peer).split(','));
    if (!built) return false;
    // Recorded as given only when the server takes it (settlePassPut).
    return await sendDmControl(peer, CTL_FRIEND_CERT, built.cert, { serial: built.serial, may: built.may, kind: 'pass' });
  } finally {
    _passMinting.delete(peer);
  }
}

/**
 * Send `cert_revoke` for every withdrawal the relay has not confirmed yet. A
 * choice note or an Unfollow still waiting to go (made while not connected)
 * goes first (10n N1, N3), so my other devices hear the new choice before they
 * hear an old pass was withdrawn. The withdrawals go even when one of those
 * cannot go yet: taking consent back is never held up.
 */
function sendPendingWithdrawals() {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !ws || ws.readyState !== WebSocket.OPEN) return;
  if (store.choiceNotesPending.length || store.unfollowsPending.length) {
    flushSelfNotes().then(sendWithdrawalsNow, sendWithdrawalsNow);
    return;
  }
  sendWithdrawalsNow();
}

function sendWithdrawalsNow() {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !ws || ws.readyState !== WebSocket.OPEN) return;
  for (const serial of store.withdrawalsPending) {
    ws.send(JSON.stringify({ type: 'cert_revoke', serial }));
  }
}

/**
 * Take back every pass I gave `peer` (on Unfollow and Block): standing, never
 * answered, or still on its way, a contact request's too (10n N9: its answer,
 * when it comes, records nothing and I do not follow them). My choice for
 * them is cleared as of `at` (chat-dm-store.js withdrawPassesTo); `force`
 * (Block) clears it whatever its time.
 */
function withdrawPassesTo(peer, at, force) {
  if (!(window.hosDmStore && hosDmStore.ready)) return;
  for (const [ref, p] of _passPuts) {
    if (p.peer !== peer) continue;
    clearTimeout(p.timer);
    _passPuts.delete(ref);
    passPutAnswer(p, { taken: false, outcome: 'withdrawn', reason: null });
  }
  hosDmStore.withdrawPassesTo(peer, at, force);
  sendPendingWithdrawals();
}

/**
 * My choice for `peer` changed (made here, or a note from my other device, 10n
 * N4): every pass of mine to them that grants beyond it, standing, never
 * answered or still on its way, is withdrawn at once. Consent taken back takes
 * effect now; until a pass carrying the choice is taken they get what my
 * settings allow strangers. A pass on its way that goes frees the way for one
 * carrying the choice; its answer, when it comes, records nothing. (A contact
 * request's put is left to its answer, which then records nothing.)
 */
function applyChoiceToPasses(peer) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store) return;
  const beyond = (p) => store.grantsBeyondChoice(peer, p.may);
  for (const [ref, p] of _passPuts) {
    if (p.peer === peer && p.kind !== 'request' && beyond(p)) {
      clearTimeout(p.timer);
      _passPuts.delete(ref);
    }
  }
  if (store.withdrawPassesWhere(peer, beyond).length) sendPendingWithdrawals();
}

// ── Notes to my own devices that wait (10n N1, N3) ──────────────────────
// A choice made, or an Unfollow, while no server is connected is kept
// (chat-dm-store.js choiceNotesPending, unfollowsPending, across reloads) and
// goes on the next connection, before that friend's withdrawals and passes.
// One flush runs at a time, so they go once each, in order.
let selfNotesSending = Promise.resolve();
function flushSelfNotes() {
  selfNotesSending = selfNotesSending.then(sendQueuedSelfNotes, sendQueuedSelfNotes);
  return selfNotesSending;
}

async function sendQueuedSelfNotes() {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !ws || ws.readyState !== WebSocket.OPEN) return;
  for (const n of store.choiceNotesPending.slice()) {
    const text = typeof choiceNoteText === 'function' ? choiceNoteText(n.peer, n.may) : null;
    if (!text) { store.choiceNoteSent(n.peer, n.at); continue; }
    // Signed with the time the choice was made, which decides which choice wins.
    const built = typeof pqBuildSelfNote === 'function' ? await pqBuildSelfNote(text, n.at) : null;
    if (!built || !ws || ws.readyState !== WebSocket.OPEN) return; // tried again on the next connection
    // Its echo, when my mailbox gives it back to this page, is not applied a second time.
    await store.selfNoteFirstSight(built.inner && built.inner.sig);
    ws.send(JSON.stringify(built.put));
    store.choiceNoteSent(n.peer, n.at);
  }
  // An Unfollow of someone I follow again by now, or have blocked since, is
  // void: dropped, never sent (10o O3, the desktop app's rule). Nothing is ever
  // sent to someone I blocked, and the block's own note already tells my other
  // devices to stop following them.
  const voided = (peer) => store.following.has(peer) || store.isBlocked(peer);
  for (const u of store.unfollowsPending.slice()) {
    if (voided(u.peer)) { store.dropQueuedUnfollow(u.peer); continue; }
    const built = await pqBuildDmPuts(CTL_UNFOLLOW, u.peer, u.at);
    if (!built) continue; // their DM key is not known here yet: it waits for the member list
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    // A follow made while it was sealed replaced it.
    if (!store.unfollowsPending.some((x) => x.peer === u.peer && x.at === u.at)) continue;
    if (voided(u.peer)) { store.dropQueuedUnfollow(u.peer); continue; }
    ws.send(JSON.stringify(built.recipientPut));
    ws.send(JSON.stringify(built.selfPut));
    store.dropQueuedUnfollow(u.peer);
  }
}
window.flushSelfNotes = flushSelfNotes;

/**
 * Bring the passes up to date (after the mailbox is read on each connection,
 * and on every member list, which carries the DM keys passes are sealed to):
 * note the server's identity, send what waited (choice notes and Unfollows
 * first, then the unconfirmed withdrawals), and give a pass carrying my choice
 * to every friend who holds none carrying it and has none on its way (10n N4):
 * every mutual follow, and everyone on the People I choose list. Covers a
 * server whose identity changed, a pass the server refused or never answered
 * (10l), and a choice from my other device. Not before the mailbox fetch on
 * this connection was read and applied (10n N7), so a device that was offline
 * learns my notes before it sends anything.
 */
// The server takes a burst of 8 private messages from one sender, then one a second
// (src/relay/handlers/dm_rate.rs). Since 10l the server says when it refused a pass, so a refused
// one is sent again by a later sweep; the pacing stays as a politeness, to be refused less: a sweep
// sends at most SWEEP_BURST passes at once, then one every SWEEP_GAP_MS, and only one sweep runs
// at a time (2026-10-10 batch review).
const SWEEP_BURST = 6;
const SWEEP_GAP_MS = 1100;
let _sweeping = false;

async function sweepFriendPasses() {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !window.hosServerDid || _sweeping) return;
  // 10n N7: on a new connection, not until the mailbox fetch was read and applied (app.js).
  if (typeof dmMailboxUnread !== 'undefined' && dmMailboxUnread) return;
  _sweeping = true;
  try {
    store.setPassServer(window.hosServerDid);
    // What was chosen or unfollowed while not connected goes first (N1, N3).
    await flushSelfNotes();
    sendPendingWithdrawals();
    let sent = 0;
    const pace = async () => {
      if (sent >= SWEEP_BURST) await new Promise((resolve) => setTimeout(resolve, SWEEP_GAP_MS));
    };
    // Not a friend whose pass my other device withdrew (10m R3), nor one whose
    // own setting refused my pass this session (R7): both wait for news or for
    // the person.
    const held = (peer) => store.passChangedOnOtherDevice(peer) || _passReachRefused.has(peer) || passPutInFlight(peer);
    for (const peer of store.passesOwed()) {
      if (held(peer)) continue;
      await pace();
      if (await sendFriendCertTo(peer)) sent++;
    }
  } finally {
    _sweeping = false;
  }
}
window.sweepFriendPasses = sweepFriendPasses;

/**
 * The relay confirmed a withdrawal (`cert_revoked {serial}`). One this device
 * did not make came from another of my devices; a friend it left with no pass
 * is drawn as "updating their pass" (10m R3, chat-dm-store.js withdrawalConfirmed).
 */
function friendPassWithdrawn(serial) {
  if (!(window.hosDmStore && hosDmStore.ready && serial)) return;
  const marked = hosDmStore.withdrawalConfirmed(serial);
  if (Array.isArray(marked) && marked.length) {
    // Any pass of mine still on its way to them went with it (chat-dm-store.js).
    sendPendingWithdrawals();
    updateFriendIndicators();
    if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
  }
}
window.friendPassWithdrawn = friendPassWithdrawn;

/**
 * `peer`'s ticks on the "People I choose" list ({message, call, trade},
 * 10c-ii): my choice for them (10n), or the defaults (Message and Trade, not
 * Call) when I have made none. Unfollow and Block clear it, so a friendship
 * begun again starts from the defaults, the same as a new pass.
 */
function friendTicks(peer) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  // A friend whose pass my other device withdrew (10m R3, 10n N6), before its
  // choice reached this one: no tick is drawn as given, so a tick made here
  // gives exactly what is ticked, never back what the other device took away.
  if (store && store.passChangedOnOtherDevice(peer)) {
    const none = {};
    for (const kind of REACH_KINDS) none[kind] = false;
    return none;
  }
  return reachTicksFromMay(store ? store.choiceMay(peer) : null);
}
window.friendTicks = friendTicks;

/**
 * Did another of my devices withdraw `peer`'s pass, leaving them none from me,
 * with its choice not heard here yet (10m R3)? The Safety page draws them
 * "(updating their pass)", their ticks still free to change.
 */
function friendPassChangedElsewhere(peer) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  return !!(store && store.passChangedOnOtherDevice(peer));
}
window.friendPassChangedElsewhere = friendPassChangedElsewhere;

// Friends whose new choice is being sent and acted on right now (makeFriendChoice).
const _choiceApplying = new Set();

/**
 * Is a choice for `peer` being sent, or a pass for them minted or waiting for
 * the server's answer, right now (the page holds their ticks still meanwhile)?
 */
function friendPassUpdating(peer) {
  return _choiceApplying.has(peer) || _passMinting.has(peer) || passPutInFlight(peer);
}
window.friendPassUpdating = friendPassUpdating;

/**
 * The person chose `mayWords` for `peer` on this device (10n N1): kept with
 * the time it was made, and sent to my own mailbox as a note for my other
 * devices (at once when connected, otherwise on the next connection, before
 * that friend's withdrawals and passes). Then the passes follow it (N4): any
 * pass granting beyond it is withdrawn at once, and one carrying it goes now
 * when it can (otherwise the sweep sends it). Returns true when it was made.
 */
async function makeFriendChoice(peer, mayWords) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !store.makeChoice(peer, mayWords)) return false;
  // The person chose for them here (10m R3, R7): the holds go.
  passPersonChoseFor(peer);
  _choiceApplying.add(peer);
  try {
    // The note first, so my other devices hear the choice before anything else.
    await flushSelfNotes();
    applyChoiceToPasses(peer);
    await sendFriendCertTo(peer);
  } finally {
    _choiceApplying.delete(peer);
  }
  return true;
}

/**
 * Tick or untick `kind` (message, call or trade) for `peer` on the "People I
 * choose" list: my choice for them becomes the `may` the new ticks give
 * (reachMayFromTicks; makeFriendChoice). For a friend marked "changed on my
 * other device" the ticks start empty (friendTicks), so a tick there gives
 * exactly what is ticked. Only for someone on the list (chat-dm-store.js
 * passFriends). Returns true when the choice was made, also with no server
 * connected (it goes on the next connection), or when the tick already stood
 * that way.
 */
async function setFriendTick(peer, kind, on) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (!store || !store.passFriends().includes(peer) || !REACH_KINDS.includes(kind)) return false;
  const ticks = friendTicks(peer);
  if (ticks[kind] === !!on) return true;
  // With the protected setup on, a tick needs the PIN (10h, /shared/protected.js).
  if (typeof protectedTake === 'function' && !protectedTake('reach_tick')) {
    return protectedAskThen('reach_tick', () => setFriendTick(peer, kind, on));
  }
  ticks[kind] = !!on;
  return makeFriendChoice(peer, reachMayFromTicks(ticks));
}
window.setFriendTick = setFriendTick;

/** The pass `peer` gave me, to attach when I reach them (DM, call ring, direct offer). */
function friendPassFor(peer) {
  try { return (window.hosDmStore && hosDmStore.ready) ? hosDmStore.certFor(peer) : null; } catch { return null; }
}
window.friendPassFor = friendPassFor;

/** Follow / unfollow (the UI entry point everywhere in the web client). */
async function setFollowLocal(peer, on) {
  if (!peer || peer === myKey) return;
  // With the protected setup on, following someone (or following back) makes a
  // friend, so it needs the PIN unless the PIN holder already let them be one
  // (10h, /shared/protected.js). Unfollowing never does, and a new friendship
  // after it needs the PIN again.
  if (on && typeof protectedBefriendAllowed === 'function' && !protectedBefriendAllowed(peer)) {
    return protectedAskThen('befriend', () => setFollowLocal(peer, on));
  }
  if (!on && typeof protectedForget === 'function') protectedForget(peer);
  // Following them here is the person's own word (10m R3, R7): their pass may
  // be sent again, from what this device knows.
  if (on) passPersonChoseFor(peer);
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  if (store) store.setFollowing(peer, on);
  if (on) myFollowing.add(peer); else myFollowing.delete(peer);
  if (on) {
    // A follow replaces an Unfollow still waiting to go (10n N3).
    if (store) store.dropQueuedUnfollow(peer);
    await sendDmControl(peer, CTL_FOLLOW);
    // Mutual now: complete the friendship with our pass.
    if (myFollowers.has(peer)) await sendFriendCertTo(peer);
  } else {
    // 10n N3: the Unfollow goes to them and to my own mailbox (both copies) now
    // when connected, otherwise on the next connection, so my other devices
    // unfollow them too. Its time is the one my choice for them is cleared as
    // of, on every device.
    const at = store ? store.choiceTimeNow(peer) : Date.now();
    if (store) {
      store.queueUnfollow(peer, at);
      await flushSelfNotes();
    } else {
      await sendDmControl(peer, CTL_UNFOLLOW, undefined, undefined, at);
    }
    // An unfollow means we no longer consent to the friend lane: the passes we
    // gave them are withdrawn, and the relay honours that at once.
    withdrawPassesTo(peer, at);
  }
  updateFriendIndicators();
  if (typeof renderPresenceSidebarForActiveContext === 'function') renderPresenceSidebarForActiveContext();
  addSystemMessage(on
    ? '✅ Following. If they follow you back, your clients exchange friendship credentials automatically.'
    : '✅ Unfollowed.');
}
window.setFollowLocal = setFollowLocal;

/**
 * A choice note (10n N2), from a DM opened and checked: applied when it is
 * mine (from me, to me, chat-dm-store.js applyChoiceNote), once (by its
 * signature), and only when it is newer than the choice kept (at an equal
 * time, the larger `may` text); then the passes follow it (N4). It clears a
 * mark "changed on my other device" (N6). One about someone I blocked is
 * ignored; one from anyone else is dropped unread. The sweep, not this, sends
 * the pass that carries a choice from my other device: that device is sending
 * its own, and its echo usually reaches this one first.
 */
async function ingestChoiceNote(inner) {
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  const note = (store && typeof choiceNoteFromSelf === 'function') ? choiceNoteFromSelf(inner, myKey) : null;
  if (!note) return;
  // One about someone I blocked is ignored before it is recorded as seen
  // (10o O7, the desktop app's order): after an Unblock, the same note
  // delivered again can still apply.
  if (store.isBlocked(note.key)) return;
  if (!await store.selfNoteFirstSight(inner.sig)) return;
  if (store.applyChoiceNote(note.key, note.may, inner.ts)) applyChoiceToPasses(note.key);
  updateFriendIndicators();
  if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
}

/**
 * Act on a verified control message; returns true when it was one (the
 * caller then skips rendering it). Self-copies sync our own state from
 * other devices.
 */
async function ingestDmControl(inner) {
  // A choice note (10n): a text that starts with its marker is never shown.
  if (typeof isChoiceNoteText === 'function' && isChoiceNoteText(inner.text)) {
    await ingestChoiceNote(inner);
    return true;
  }
  if (![CTL_FOLLOW, CTL_UNFOLLOW, CTL_FRIEND_CERT].includes(inner.text)) return false;
  const fromMe = inner.from === myKey;
  const peer = fromMe ? inner.to : inner.from;
  const store = (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
  // My own follow or pass for someone I have since blocked, echoed from a
  // device that had not heard of the block yet: the block wins. (Their own
  // notices never get this far: blockScreenDm drops them first.)
  if (fromMe && store && store.isBlocked(peer)) {
    if (inner.text === CTL_FRIEND_CERT && inner.cert) {
      const pass = friendPassParse(inner.cert);
      if (pass) store.recordPassSent(peer, pass.serial, pass.may);
      withdrawPassesTo(peer);
    }
    return true;
  }
  if (inner.text === CTL_FOLLOW) {
    if (fromMe) {
      // My own follow, echoed from another device: an Unfollow of them still
      // waiting to go from this one is void (10o O3), as a follow made here
      // voids it (setFollowLocal).
      if (store) {
        store.setFollowing(peer, true);
        store.dropQueuedUnfollow(peer);
      }
      myFollowing.add(peer);
    } else {
      if (store) store.setFollower(peer, true);
      myFollowers.add(peer);
      const peerName = (window.peerData && peerData[peer]?.display_name) || shortKey(peer);
      addSystemMessage(`👁️ ${esc(peerName)} is now following you.`);
      if (myFollowing.has(peer)) await sendFriendCertTo(peer);
    }
  } else if (inner.text === CTL_UNFOLLOW) {
    if (fromMe) {
      // Our own unfollow from another device: the passes we gave go too
      // (that device withdrew the ones it knew of; a repeat is harmless), my
      // choice for them is cleared as of when it was made (10n N3, the same
      // time on every device), and with the protected setup on they leave its
      // approved list here too, as an Unfollow made on this device does
      // (setFollowLocal): a new friendship with them needs the PIN again.
      if (store) store.setFollowing(peer, false);
      myFollowing.delete(peer);
      withdrawPassesTo(peer, inner.ts);
      if (typeof protectedForget === 'function') protectedForget(peer);
    } else {
      // They unfollowed, and so withdrew the pass they gave us.
      if (store) { store.setFollower(peer, false); store.forgetCertFrom(peer); }
      myFollowers.delete(peer);
    }
  } else if (inner.text === CTL_FRIEND_CERT && inner.cert) {
    if (fromMe) {
      // A pass we gave, echoed from another device (10n N4): recorded as
      // standing so any of our devices can withdraw it, unless it grants
      // beyond my choice for them (withdrawn at once) or this device is
      // already withdrawing it. It never changes the choice and never
      // withdraws another pass (chat-dm-store.js adoptEchoedPass).
      const pass = friendPassParse(inner.cert);
      if (store && pass && store.adoptEchoedPass(peer, pass.serial, pass.may).length) sendPendingWithdrawals();
    } else if (await pqVerifyFriendCert(inner.from, myKey, inner.cert)) {
      const hadOne = store ? !!store.certFor(inner.from) : false;
      if (store) store.storeCertFrom(inner.from, inner.cert);
      // Their pass reached me: mine, refused by their setting before (10m
      // R7), can go now, presenting theirs.
      _passReachRefused.delete(inner.from);
      if (!hadOne) {
        const peerName = (window.peerData && peerData[peer]?.display_name) || shortKey(peer);
        addSystemMessage(`🤝 You and ${esc(peerName)} are friends now: messages between you are unlimited.`);
      }
    } else {
      console.warn('friendship pass failed its check; dropped');
    }
  }
  updateFriendIndicators();
  return true;
}
window.ingestDmControl = ingestDmControl;
