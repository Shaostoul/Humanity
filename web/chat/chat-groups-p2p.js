// chat-groups-p2p.js — P2P groups create / invite / join (Phase 1).
//
// Wires the chat UI to the sovereign signed-object group model
// (docs/design/p2p-groups.md): create a group_v1, mint a creator-signed invite
// (a copyable ticket carrying a secret), and join by revealing that secret — no
// creator/relay-owner online required for the join to be authorized.
//
// Crypto is the KAT-locked web object layer (web/shared/{canonical-cbor,pq-object}.js)
// + the vendored post-quantum bundle (blake3 + the chat's own Dilithium signer).
// The ESM modules are dynamic-imported lazily so non-group sessions don't pay for
// them. Depends on app.js globals: myKey (Dilithium pubkey hex), myDilithiumSecret,
// window.pqSignMessage, addSystemMessage, addNotice, renderGroupList.
//
// Phase 1 = identity + membership + invites. End-to-end group MESSAGING is Phase 2
// (E2EE epoch keys); a P2P group here shows its roster + invite controls.
(function () {
  let _mods = null;
  async function mods() {
    if (_mods) return _mods;
    const [obj, noble] = await Promise.all([
      import('/shared/pq-object.js'),
      import('/shared/vendor/noble-pq.bundle.js'),
    ]);
    if (!noble.blake3) throw new Error('vendored PQ bundle missing blake3');
    const blake3 = (data) => noble.blake3.create({ dkLen: 32 }).update(data).digest();
    _mods = { obj, blake3 };
    return _mods;
  }

  function hexToBytes(hex) {
    const a = new Uint8Array(hex.length / 2);
    for (let i = 0; i < a.length; i++) a[i] = parseInt(hex.substr(i * 2, 2), 16);
    return a;
  }
  function pqReady() {
    return typeof myKey === 'string' && myKey.length > 0
      && typeof myDilithiumSecret !== 'undefined' && myDilithiumSecret
      && typeof window.pqSignMessage === 'function';
  }
  function authorPub() { return hexToBytes(myKey); }
  function signer() { return async (bytes) => window.pqSignMessage(myDilithiumSecret, bytes); }
  function notReady() {
    if (typeof addNotice === 'function') addNotice('Connect first — your post-quantum identity isn’t ready yet.', 'red', 6);
    return false;
  }

  async function postObject(submission) {
    const res = await fetch('/api/v2/objects', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(submission),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(data.error || ('HTTP ' + res.status));
    return data;
  }

  // ── Phase 2 helpers (E2EE messages) ──────────────────────────────────────
  async function myFingerprint() {
    const { blake3 } = await mods();
    const pub = hexToBytes(myKey);
    const h = blake3(pub);                  // 32 bytes
    let s = '';
    for (let i = 0; i < 16; i++) s += h[i].toString(16).padStart(2, '0');
    return s;                                // matches author_fingerprint() in Rust
  }
  function b64ToBytes(b64) {
    const bin = atob(b64);
    const u = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
    return u;
  }
  /** Fetch + open the group's latest epoch key. Returns `{epoch, epochKey}` or null. */
  async function fetchEpochKey(groupId) {
    if (!pqReady() || typeof window.pqDmOpen !== 'function' || !myKyberSecret) return null;
    try {
      const res = await fetch('/api/v2/groups/' + encodeURIComponent(groupId) + '/epoch');
      if (!res.ok) return null;
      const epochObj = await res.json();
      const { obj } = await mods();
      const fp = await myFingerprint();
      return await obj.openGroupEpochKey(b64ToBytes(epochObj.payload_b64), fp, window.pqDmOpen, myKyberSecret);
    } catch (e) { console.warn('fetchEpochKey:', e); return null; }
  }
  /** Fetch + decrypt the group's encrypted message log. */
  async function fetchGroupMessages(groupId, epochKey) {
    const out = [];
    try {
      const res = await fetch('/api/v2/groups/' + encodeURIComponent(groupId) + '/messages');
      if (!res.ok) return out;
      const data = await res.json();
      const { obj } = await mods();
      for (const m of (data.messages || [])) {
        const parsed = obj.parseGroupMsgPayload(b64ToBytes(m.payload_b64));
        if (!parsed) continue;
        const text = await obj.aesGcmDecrypt(epochKey, parsed.nonce, parsed.ct);
        if (text === null) continue;
        out.push({ author_fp: m.author_fp, created_at: m.created_at, text });
      }
    } catch (e) { console.warn('fetchGroupMessages:', e); }
    return out;
  }
  /** Encrypt + post a message into a P2P group. */
  async function sendGroupMessage(groupId, epoch, epochKey, plaintext) {
    const { obj, blake3 } = await mods();
    const built = await obj.buildGroupMsgV1({
      groupId, epoch, epochKey, plaintext,
      authorPublicKey: authorPub(), sign: signer(), blake3,
    });
    await postObject(built.submission);
  }

  /**
   * If I am the creator AND new members have joined since the last epoch key
   * was issued, mint a fresh epoch key sealed to the FULL current roster +
   * post it. Returns `{epoch, epochKey, addedCount}` on rekey, null otherwise.
   *
   * This is what unblocks cross-identity chat: the create-time epoch is sealed
   * only to the creator; without this, new joiners can never decrypt anything.
   * Runs once when the dialog first opens, and again on manual refresh.
   */
  async function rekeyIfCreatorNeeds(groupId) {
    if (!pqReady()) return null;
    if (typeof window.pqDmSeal !== 'function') return null;
    if (typeof myKyberPublicBase64 === 'undefined' || !myKyberPublicBase64) return null;

    // (1) Am I the creator? Fetch group_v1 and compare author_public_key_b64.
    let groupObj;
    try {
      const r = await fetch('/api/v2/objects/' + encodeURIComponent(groupId));
      if (!r.ok) return null;
      groupObj = await r.json();
    } catch (e) { return null; }
    // btoa of the raw bytes of myKey (hex pubkey) — same encoding the relay used.
    const myPubB64 = (function() {
      const bytes = hexToBytes(myKey);
      let s = ''; for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
      return btoa(s);
    })();
    if (groupObj.author_public_key_b64 !== myPubB64) return null;

    const { obj, blake3 } = await mods();

    // (2) Current epoch + already-covered recipient fingerprints.
    let currentEpoch = 0;
    const coveredFps = new Set();
    try {
      const r = await fetch('/api/v2/groups/' + encodeURIComponent(groupId) + '/epoch');
      if (r.ok) {
        const epochObj = await r.json();
        const parsed = obj.parseGroupEpochKeyPayload(b64ToBytes(epochObj.payload_b64));
        if (parsed) {
          currentEpoch = parsed.epoch || 0;
          for (const rcp of parsed.recipients) {
            if (rcp && rcp.fp) coveredFps.add(rcp.fp);
          }
        }
      }
    } catch (e) { /* no epoch yet — treat as currentEpoch=0, empty set */ }

    // (3) Current roster with each member's Kyber public key.
    let allMembers = [];
    try {
      const r = await fetch('/api/v2/groups/' + encodeURIComponent(groupId) + '/members');
      if (!r.ok) return null;
      const data = await r.json();
      allMembers = data.members || [];
    } catch (e) { return null; }

    // (4) Compute each member's fingerprint; identify gaps. Members without a
    // registered Kyber pubkey are silently skipped (they cannot be sealed to
    // until they register).
    const sealable = [];
    let hasGap = false;
    for (const m of allMembers) {
      if (!m.kyber_public || !m.pubkey) continue;
      const pubBytes = hexToBytes(m.pubkey);
      const h = blake3(pubBytes);
      let fp = '';
      for (let i = 0; i < 16; i++) fp += h[i].toString(16).padStart(2, '0');
      sealable.push({ fp, kyber_public: m.kyber_public });
      if (!coveredFps.has(fp)) hasGap = true;
    }

    if (!hasGap) return null; // all current members already covered

    // (5) Mint a new epoch sealed to the full sealable roster.
    const newEpoch = currentEpoch + 1;
    const newEpochKey = obj.randomEpochKey();
    const ek = await obj.buildGroupEpochKeyV1({
      groupId, epoch: newEpoch, epochKey: newEpochKey,
      members: sealable,
      seal: window.pqDmSeal,
      authorPublicKey: authorPub(), sign: signer(), blake3,
    });
    await postObject(ek.submission);
    const addedCount = sealable.length - coveredFps.size;
    return { epoch: newEpoch, epochKey: newEpochKey, addedCount };
  }

  async function createP2pGroup(name) {
    if (!pqReady()) return notReady();
    name = (name || '').trim();
    if (!name) return;
    const { obj, blake3 } = await mods();
    const { objectId: groupId, submission } = await obj.buildGroupV1({ name, authorPublicKey: authorPub(), sign: signer(), blake3 });
    await postObject(submission);
    // Auto-issue an initial epoch key sealed to the creator so chat works
    // immediately — without this the group is identity+membership only and
    // you cannot send anything. Sealed to ourselves (and any future joiners
    // get re-keyed via a manual "rotate" — Phase-2 follow-up).
    if (typeof window.pqDmSeal === 'function' && typeof myKyberPublicBase64 !== 'undefined' && myKyberPublicBase64) {
      try {
        const fp = await myFingerprint();
        const epochKey = obj.randomEpochKey();
        const ek = await obj.buildGroupEpochKeyV1({
          groupId, epoch: 1, epochKey,
          members: [{ fp, kyber_public: myKyberPublicBase64 }],
          seal: window.pqDmSeal,
          authorPublicKey: authorPub(), sign: signer(), blake3,
        });
        await postObject(ek.submission);
      } catch (e) {
        console.warn('initial epoch key failed:', e);
        if (typeof addNotice === 'function') addNotice('Group created but epoch key failed — messaging may not work.', 'orange', 8);
      }
    }
    if (typeof addSystemMessage === 'function') addSystemMessage('✅ Created group "' + name + '".');
    await loadP2pGroups();
    // Auto-switch into the new group so the creator lands in it right away and
    // the epoch key is live immediately (forces the keygen path now, so a
    // joiner won't hit "no epoch key yet" while the creator sits elsewhere).
    if (typeof openP2pGroup === 'function') openP2pGroup(groupId, name);
  }

  // Mint a creator-signed invite for `groupId`, returning a shareable ticket string.
  async function createP2pInvite(groupId, groupName) {
    if (!pqReady()) { notReady(); return null; }
    const { obj, blake3 } = await mods();
    const secret = obj.randomInviteSecret();
    const expiresAt = Date.now() + 7 * 24 * 3600 * 1000; // 7-day invite
    const { objectId: inviteId, submission } =
      await obj.buildGroupInviteV1({ groupId, secret, expiresAt, authorPublicKey: authorPub(), sign: signer(), blake3 });
    await postObject(submission);
    return obj.encodeInviteTicket({ groupId, groupName, inviteId, secret });
  }

  async function joinP2pGroupByTicket(ticketStr) {
    if (!pqReady()) return notReady();
    const { obj, blake3 } = await mods();
    let t;
    try { t = obj.decodeInviteTicket((ticketStr || '').trim()); }
    catch (e) { if (typeof addNotice === 'function') addNotice('That doesn’t look like a valid invite ticket.', 'red', 6); return; }
    const { submission } =
      await obj.buildGroupJoinV1({ groupId: t.groupId, inviteId: t.inviteId, secret: t.secret, authorPublicKey: authorPub(), sign: signer(), blake3 });
    await postObject(submission);
    if (typeof addSystemMessage === 'function') addSystemMessage('✅ Joined group "' + (t.groupName || t.groupId.slice(0, 8)) + '".');
    await loadP2pGroups();
  }

  // Leave a group I'm in: post a group_member_v1 {action:"remove", subject:me}.
  // The relay authorizes self-removal for any member (you can always leave).
  async function leaveP2pGroup(groupId) {
    if (!pqReady()) return notReady();
    const { obj, blake3 } = await mods();
    const { submission } = await obj.buildGroupMemberV1({
      groupId, action: 'remove', subjectPubkey: authorPub(),
      authorPublicKey: authorPub(), sign: signer(), blake3,
    });
    await postObject(submission);
    // If I'm currently viewing it, drop back to a normal channel.
    if (window.activeP2pGroup && window.activeP2pGroup.id === groupId) {
      if (typeof closeP2pGroup === 'function') closeP2pGroup();
      if (typeof switchChannel === 'function') switchChannel('general');
    }
    if (typeof addSystemMessage === 'function') addSystemMessage('You left the group.');
    await loadP2pGroups();
  }

  // Disband a group I created: post a creator-signed group_disband_v1. The
  // relay hides it for EVERY member. Only honored if I'm the creator.
  async function disbandP2pGroup(groupId) {
    if (!pqReady()) return notReady();
    const { obj, blake3 } = await mods();
    const { submission } = await obj.buildGroupDisbandV1({
      groupId, authorPublicKey: authorPub(), sign: signer(), blake3,
    });
    await postObject(submission);
    if (window.activeP2pGroup && window.activeP2pGroup.id === groupId) {
      if (typeof closeP2pGroup === 'function') closeP2pGroup();
      if (typeof switchChannel === 'function') switchChannel('general');
    }
    if (typeof addSystemMessage === 'function') addSystemMessage('Group disbanded for everyone.');
    await loadP2pGroups();
  }

  // Fetch my P2P groups + rosters from the relay projection and re-render.
  // Sets `_p2pGroupsFetched` only once a real fetch is attempted (myKey ready)
  // — otherwise the lazy trigger in renderGroupList would burn the flag before
  // identity loads and the groups would never appear until the user interacted
  // (the "no groups on first load" bug).
  async function loadP2pGroups() {
    if (typeof myKey !== 'string' || !myKey) return;
    window._p2pGroupsFetched = true;
    try {
      const res = await fetch('/api/v2/groups?pubkey=' + encodeURIComponent(myKey));
      const data = await res.json();
      window._p2pGroups = (data && Array.isArray(data.groups)) ? data.groups : [];
    } catch (e) {
      window._p2pGroups = window._p2pGroups || [];
    }
    if (typeof renderGroupList === 'function') renderGroupList();
  }

  // ── Inline group conversation (NO modal) ─────────────────────────────────
  // openP2pGroup loads the group INTO the existing chat center panel, the same
  // way switchChannel loads a channel: header → group name; messages → decrypted
  // E2EE history rendered via the standard addChatMessage; composer → sends to
  // the group via the per-epoch AES-GCM key. No popup, no parallel UI; the
  // chat reuses everything (renderer, styling, scroll, identicons, theme).
  //
  // The modal version (openP2pGroupDialog) is removed — it duplicated the chat
  // UI inside a constrained window AND was hitting an egui-like z-order bug
  // where the backdrop kept landing in front of the modal. Inline is the
  // correct mental model: switching from "#general" to "My Group" is one
  // context change, not a popup.

  // Active P2P-group conversation lives on `window` so the cross-file
  // monkey-patches below can see it without import gymnastics.
  window.activeP2pGroup = null;

  let _p2pPollTimer = null;
  // Dedup key to skip re-rendering identical history (otherwise 4s polling
  // flickers + breaks scroll position).
  let _p2pRenderedKey = '';
  let _p2pRefreshing = false;

  function _stopP2pPoll() {
    if (_p2pPollTimer) { clearInterval(_p2pPollTimer); _p2pPollTimer = null; }
  }

  function _renderP2pPlaceholder(text) {
    if (!window.activeP2pGroup) return;
    const msgsEl = document.getElementById('messages');
    if (!msgsEl) return;
    msgsEl.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:var(--space-xl);font-size:0.8rem;">' + esc(text) + '</div>';
  }

  function _renderP2pMessages(msgs) {
    if (!window.activeP2pGroup) return;
    const ag = window.activeP2pGroup;
    const msgsEl = document.getElementById('messages');
    if (!msgsEl) return;
    msgsEl.innerHTML = '';
    if (typeof resetMsgStripe === 'function') resetMsgStripe();
    if (typeof seenTimestamps !== 'undefined' && seenTimestamps && typeof seenTimestamps.clear === 'function') seenTimestamps.clear();
    if (typeof messageReactions !== 'undefined' && messageReactions) {
      Object.keys(messageReactions).forEach((k) => delete messageReactions[k]);
    }
    if (msgs.length === 0) {
      msgsEl.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:var(--space-xl);font-size:0.8rem;">No messages yet. Be the first to chat — your messages are end-to-end encrypted under the group epoch key.</div>';
      return;
    }
    for (const m of msgs) {
      const isMe = ag.myFp && m.author_fp === ag.myFp;
      // For non-me messages, all we have on hand is the fingerprint. The author
      // map (member key → display name) is loaded as part of refresh below.
      const labelFromMap = !isMe && ag.fpToName ? ag.fpToName[m.author_fp] : null;
      const authorName = isMe
        ? (window.myName || 'You')
        : (labelFromMap || (m.author_fp || '').slice(0, 12) + '…');
      const fromKey = isMe ? myKey : (ag.fpToKey && ag.fpToKey[m.author_fp]) || m.author_fp;
      addChatMessage(authorName, m.text, m.created_at, fromKey, true, false, null, null);
    }
  }

  // Populate ag.fpToName + ag.fpToKey by fetching the roster + matching each
  // member's fingerprint. Best-effort: failures are silent (fall back to short fp).
  async function _loadRosterIndex(ag) {
    try {
      const r = await fetch('/api/v2/groups/' + encodeURIComponent(ag.id) + '/members');
      if (!r.ok) return;
      const data = await r.json();
      const { blake3 } = await mods();
      ag.fpToName = {};
      ag.fpToKey = {};
      for (const m of (data.members || [])) {
        if (!m.pubkey) continue;
        const h = blake3(hexToBytes(m.pubkey));
        let fp = '';
        for (let i = 0; i < 16; i++) fp += h[i].toString(16).padStart(2, '0');
        ag.fpToKey[fp] = m.pubkey;
        // For now we don't have a name lookup here — peerData might have one
        // if they've been seen on the relay. Fall back to short pubkey.
        const peer = (typeof peerData !== 'undefined' && peerData) ? peerData[m.pubkey] : null;
        ag.fpToName[fp] = (peer && peer.display_name) || (m.pubkey.slice(0, 8) + '…');
      }
    } catch (e) { /* best effort */ }
  }

  async function _p2pRefresh() {
    const ag = window.activeP2pGroup;
    if (!ag || _p2pRefreshing) return;
    _p2pRefreshing = true;
    try {
      if (!ag.myFp) ag.myFp = await myFingerprint().catch(() => '');
      if (!ag.fpToName) await _loadRosterIndex(ag);
      if (!ag.epochKey) {
        // (1) If I'm the creator AND new members joined, rotate the key.
        try {
          const rekey = await rekeyIfCreatorNeeds(ag.id);
          if (rekey) {
            ag.epoch = rekey.epoch;
            ag.epochKey = rekey.epochKey;
            // Roster changed if a rekey happened — reload the name map.
            await _loadRosterIndex(ag);
            if (typeof addSystemMessage === 'function') {
              addSystemMessage('Rotated group key for ' + rekey.addedCount + ' new member' + (rekey.addedCount === 1 ? '' : 's') + '.');
            }
          }
        } catch (e) { console.warn('rekey check:', e); }
        // (2) No rekey or non-creator → fetch the latest key.
        if (!ag.epochKey) {
          const ek = await fetchEpochKey(ag.id);
          if (ek) { ag.epoch = ek.epoch; ag.epochKey = ek.epochKey; }
        }
        if (!ag.epochKey) {
          _renderP2pPlaceholder('No epoch key yet. The group creator must open this group once for the first key to be issued, then refresh.');
          _p2pRenderedKey = ''; // so the next refresh repaints when the key shows up
          return;
        }
      }
      // (3) Fetch + decrypt + render (skip if nothing changed).
      const msgs = await fetchGroupMessages(ag.id, ag.epochKey);
      msgs.sort((a, b) => (a.created_at || 0) - (b.created_at || 0));
      const key = msgs.map((m) => (m.author_fp || '') + ':' + (m.created_at || 0)).join('|');
      if (key === _p2pRenderedKey) return;
      _p2pRenderedKey = key;
      _renderP2pMessages(msgs);
    } finally {
      _p2pRefreshing = false;
    }
  }

  /**
   * Switch the main chat to the given P2P group as if it were a channel.
   * Returns immediately (no awaits before view changes) — the data load
   * runs in the background and populates the panel when it arrives.
   */
  function openP2pGroup(groupId, name) {
    if (!pqReady()) { notReady(); return; }
    // (a) Clear competing contexts (DM, legacy group). Leave `activeChannel`
    //     untouched so switching back to a channel restores it.
    if (typeof activeDmPartner !== 'undefined') { activeDmPartner = null; activeDmPartnerName = ''; }
    if (typeof activeGroupId !== 'undefined') { activeGroupId = null; activeGroupName = ''; }
    window.activeP2pGroup = { id: groupId, name: name || '', epoch: 0, epochKey: null, myFp: '', fpToName: null, fpToKey: null };

    // (b) Sidebar: switch to Groups tab + redraw lists so highlights reflect state.
    if (typeof switchSidebarTab === 'function') switchSidebarTab('groups', true);
    if (typeof renderChannelList === 'function') renderChannelList();
    if (typeof renderDmList === 'function') renderDmList();
    if (typeof renderGroupList === 'function') renderGroupList();

    // (c) Hide pin bar (groups don't have pins yet).
    const pinBar = document.getElementById('pin-bar');
    if (pinBar) pinBar.style.display = 'none';
    const pinList = document.getElementById('pin-list');
    if (pinList) pinList.classList.remove('open');

    // (d) Header — name + invite link (no parallel modal, just a tiny inline link).
    const header = document.getElementById('channel-header');
    if (header) {
      const displayName = name || groupId.slice(0, 8);
      header.style.display = 'block';
      header.innerHTML = '<span class="ch-name">🔒 ' + esc(displayName) + '</span>' +
        '<span class="ch-desc">End-to-end encrypted group · ' +
        '<a href="#" id="p2pg-header-invite" style="color:var(--accent);text-decoration:none;">Copy invite ticket</a></span>';
      const inv = header.querySelector('#p2pg-header-invite');
      if (inv) inv.onclick = async (e) => {
        e.preventDefault();
        inv.textContent = 'Minting…';
        try {
          const ticket = await createP2pInvite(groupId, displayName);
          if (ticket) {
            try {
              await navigator.clipboard.writeText(ticket);
              if (typeof addSystemMessage === 'function') addSystemMessage('Invite ticket copied to clipboard. Share it within 7 days.');
            } catch {
              window.prompt('Copy this invite ticket (Ctrl+C):', ticket);
            }
          }
        } catch (err) {
          if (typeof addNotice === 'function') addNotice('Invite failed: ' + err.message, 'red', 6);
        } finally {
          inv.textContent = 'Copy invite ticket';
        }
      };
    }

    // (e) Clear messages, set group context (group tint), show Loading until
    //     the first refresh paints (or the placeholder if no key yet).
    const msgsEl = document.getElementById('messages');
    if (msgsEl) {
      msgsEl.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:var(--space-xl);font-size:0.8rem;">Loading…</div>';
      msgsEl.dataset.ctx = 'group';
    }
    if (typeof resetMsgStripe === 'function') resetMsgStripe();

    // (f) Composer.
    const input = document.getElementById('msg-input');
    const sendBtn = document.getElementById('send-btn');
    if (input) {
      input.disabled = false;
      input.placeholder = 'Message ' + (name || 'group') + '…';
      try { input.focus(); } catch {}
    }
    if (sendBtn) sendBtn.disabled = false;

    // (g) Reset dedup so the first refresh paints.
    _p2pRenderedKey = '';

    // (h) Kick off the first refresh in the background + start polling.
    _p2pRefresh();
    _stopP2pPoll();
    _p2pPollTimer = setInterval(_p2pRefresh, 4000);

    if (typeof closeSidebars === 'function' && typeof isMobile === 'function' && isMobile()) closeSidebars();
  }

  /** Called when switching away from a P2P group (channel, DM, legacy group). */
  function closeP2pGroup() {
    _stopP2pPoll();
    _p2pRenderedKey = '';
    if (window.activeP2pGroup) window.activeP2pGroup = null;
  }

  // ── Cross-file hooks ────────────────────────────────────────────────────
  // chat-groups-p2p.js loads AFTER chat-social.js (see index.html), so our
  // wrappers run BEFORE chat-social's already-wrapped versions and close the
  // P2P context before any channel/DM/legacy-group switch happens.
  if (typeof switchChannel === 'function') {
    const _origSwitchChannelP2p = switchChannel;
    // eslint-disable-next-line no-global-assign
    switchChannel = function (channelId) {
      closeP2pGroup();
      return _origSwitchChannelP2p(channelId);
    };
  }
  if (typeof openDmConversation === 'function') {
    const _origOpenDmP2p = openDmConversation;
    // eslint-disable-next-line no-global-assign
    openDmConversation = function () {
      closeP2pGroup();
      return _origOpenDmP2p.apply(this, arguments);
    };
  }
  if (typeof openGroup === 'function') {
    const _origOpenGroupP2p = openGroup;
    // eslint-disable-next-line no-global-assign
    openGroup = function () {
      closeP2pGroup();
      return _origOpenGroupP2p.apply(this, arguments);
    };
  }
  // sendMessage routing: when a P2P group is active, the composer sends to it.
  if (typeof sendMessage === 'function') {
    const _origSendMsgP2p = sendMessage;
    // eslint-disable-next-line no-global-assign
    sendMessage = async function () {
      const ag = window.activeP2pGroup;
      if (!ag) return _origSendMsgP2p.apply(this, arguments);
      const input = document.getElementById('msg-input');
      const sendBtn = document.getElementById('send-btn');
      if (!input) return;
      const text = (input.value || '').trim();
      if (!text) return;
      if (!ag.epochKey) {
        if (typeof addNotice === 'function') addNotice('Waiting for the group epoch key. The group creator must open the group once first.', 'orange', 6);
        return;
      }
      input.disabled = true;
      if (sendBtn) sendBtn.disabled = true;
      try {
        await sendGroupMessage(ag.id, ag.epoch || 1, ag.epochKey, text);
        input.value = '';
        input.style.height = 'auto';
        // Optimistic local echo so the user sees their message land
        // immediately — the next poll-refresh reconciles with what the relay
        // stored (dedup by author_fp + created_at).
        addChatMessage(window.myName || 'You', text, Date.now(), myKey, false, false, null, null);
        _p2pRefresh();
      } catch (e) {
        if (typeof addNotice === 'function') addNotice('Send failed: ' + e.message, 'red', 6);
      } finally {
        input.disabled = false;
        if (sendBtn) sendBtn.disabled = false;
        try { input.focus(); } catch {}
      }
    };
  }

  window.createP2pGroup = createP2pGroup;
  window.createP2pInvite = createP2pInvite;
  window.joinP2pGroupByTicket = joinP2pGroupByTicket;
  window.leaveP2pGroup = leaveP2pGroup;
  window.disbandP2pGroup = disbandP2pGroup;
  window.loadP2pGroups = loadP2pGroups;
  window.openP2pGroup = openP2pGroup;
  window.closeP2pGroup = closeP2pGroup;
  // Back-compat alias: anything still calling the old name gets routed into
  // the inline flow (no modal). Members arg ignored — roster is fetched
  // server-side now.
  window.openP2pGroupDialog = function (groupId, name) { return openP2pGroup(groupId, name); };
})();
