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

  // Fetch my P2P groups + rosters from the relay projection and re-render.
  async function loadP2pGroups() {
    if (typeof myKey !== 'string' || !myKey) return;
    try {
      const res = await fetch('/api/v2/groups?pubkey=' + encodeURIComponent(myKey));
      const data = await res.json();
      window._p2pGroups = (data && Array.isArray(data.groups)) ? data.groups : [];
    } catch (e) {
      window._p2pGroups = window._p2pGroups || [];
    }
    if (typeof renderGroupList === 'function') renderGroupList();
  }

  // Group chat dialog: roster + LIVE E2EE message log + compose + invite-mint.
  // Phase 2: messages are AES-GCM under the group's epoch key (fetched via
  // /api/v2/groups/{id}/epoch and unsealed with our Kyber secret). Polls every
  // 4s while open. Closing tears down the refresh interval.
  function openP2pGroupDialog(groupId, name, members) {
    members = members || [];
    const overlay = document.createElement('div');
    overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.8);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';
    overlay.innerHTML =
      '<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);max-width:560px;width:100%;max-height:80vh;display:flex;flex-direction:column;box-sizing:border-box;">' +
        '<div style="padding:var(--space-md) var(--space-lg);border-bottom:1px solid var(--border);display:flex;align-items:center;justify-content:space-between;">' +
          '<div>' +
            '<div style="font-weight:700;font-size:1rem;">🔒 ' + esc(name || groupId.slice(0, 8)) + '</div>' +
            '<div id="p2pg-sub" style="font-size:0.7rem;color:var(--text-muted);">' + members.length + ' member' + (members.length === 1 ? '' : 's') + ' · end-to-end encrypted</div>' +
          '</div>' +
          '<button id="p2pg-close" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:1.2rem;padding:4px 8px;">✕</button>' +
        '</div>' +
        '<div id="p2pg-msgs" style="flex:1;overflow-y:auto;padding:var(--space-md) var(--space-lg);font-size:0.85rem;color:var(--text);min-height:220px;max-height:50vh;">' +
          '<div style="color:var(--text-muted);font-size:0.8rem;">Loading messages…</div>' +
        '</div>' +
        '<div style="padding:var(--space-md) var(--space-lg);border-top:1px solid var(--border);">' +
          '<div style="display:flex;gap:var(--space-sm);align-items:center;">' +
            '<input id="p2pg-compose" type="text" placeholder="Type a message…" style="flex:1;padding:var(--space-sm) var(--space-md);background:var(--bg-primary);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);font-size:0.9rem;outline:none;" />' +
            '<button id="p2pg-send" class="vr-btn">Send</button>' +
          '</div>' +
          '<div style="display:flex;gap:var(--space-sm);margin-top:var(--space-sm);">' +
            '<button id="p2pg-invite" class="vr-btn" style="flex:1;font-size:0.75rem;">🔗 Create invite</button>' +
            '<button id="p2pg-refresh" class="vr-btn" style="font-size:0.75rem;" title="Refresh">↻</button>' +
          '</div>' +
          '<div id="p2pg-ticket" style="display:none;margin-top:var(--space-sm);"></div>' +
        '</div>' +
      '</div>';
    document.body.appendChild(overlay);

    const state = { epoch: 0, epochKey: null, myFp: '', refreshTimer: null, busy: false };
    const msgsBox = overlay.querySelector('#p2pg-msgs');
    const composeInput = overlay.querySelector('#p2pg-compose');
    const sendBtn = overlay.querySelector('#p2pg-send');
    const ticketBox = overlay.querySelector('#p2pg-ticket');

    function close() {
      if (state.refreshTimer) { clearInterval(state.refreshTimer); state.refreshTimer = null; }
      overlay.remove();
    }
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
    overlay.querySelector('#p2pg-close').onclick = close;

    async function refresh() {
      if (state.busy) return;
      state.busy = true;
      try {
        if (!state.myFp) state.myFp = await myFingerprint();
        if (!state.epochKey) {
          // If I am the creator AND new members have joined since the last
          // epoch was issued, rotate the key sealed to the full roster so they
          // can decrypt. This is what unblocks cross-identity chat.
          try {
            const rekey = await rekeyIfCreatorNeeds(groupId);
            if (rekey) {
              state.epoch = rekey.epoch;
              state.epochKey = rekey.epochKey;
              if (typeof addSystemMessage === 'function') {
                addSystemMessage('Rotated group key for ' + rekey.addedCount + ' new member' + (rekey.addedCount === 1 ? '' : 's') + '.');
              }
            }
          } catch (e) { console.warn('rekey check failed:', e); }
          // If no rekey happened (or it failed), just fetch the latest key.
          if (!state.epochKey) {
            const ek = await fetchEpochKey(groupId);
            if (ek) { state.epoch = ek.epoch; state.epochKey = ek.epochKey; }
          }
          if (!state.epochKey) {
            msgsBox.innerHTML = '<div style="color:var(--text-muted);font-size:0.8rem;">No epoch key available yet — wait for the group creator to issue one (it happens automatically the first time they open this dialog). Then ↻ refresh.</div>';
            return;
          }
        }
        const msgs = await fetchGroupMessages(groupId, state.epochKey);
        if (msgs.length === 0) {
          msgsBox.innerHTML = '<div style="color:var(--text-muted);font-size:0.8rem;">No messages yet. Be the first to chat — your messages are end-to-end encrypted under the group epoch key.</div>';
        } else {
          msgs.sort((a, b) => (a.created_at || 0) - (b.created_at || 0));
          msgsBox.innerHTML = msgs.map(function(m) {
            var isMe = m.author_fp === state.myFp;
            var label = isMe ? 'You' : ((m.author_fp || '').slice(0, 12) + '…');
            var time = m.created_at ? new Date(m.created_at).toLocaleTimeString([], { hour:'2-digit', minute:'2-digit' }) : '';
            var nameColor = isMe ? 'var(--accent)' : 'var(--text)';
            return '<div style="margin-bottom:var(--space-sm);">' +
              '<span style="font-weight:600;color:' + nameColor + ';">' + esc(label) + '</span> ' +
              '<span style="font-size:0.7rem;color:var(--text-muted);">' + time + '</span>' +
              '<div style="margin-top:2px;white-space:pre-wrap;">' + esc(m.text) + '</div>' +
              '</div>';
          }).join('');
          msgsBox.scrollTop = msgsBox.scrollHeight;
        }
      } finally { state.busy = false; }
    }

    async function doSend() {
      var text = composeInput.value.trim();
      if (!text) return;
      if (!state.epochKey) {
        if (typeof addNotice === 'function') addNotice('Waiting for the group epoch key. Try ↻ refresh.', 'orange', 6);
        return;
      }
      composeInput.disabled = true; sendBtn.disabled = true;
      try {
        await sendGroupMessage(groupId, state.epoch || 1, state.epochKey, text);
        composeInput.value = '';
        await refresh();
      } catch (e) {
        if (typeof addNotice === 'function') addNotice('Send failed: ' + e.message, 'red', 6);
      } finally {
        composeInput.disabled = false; sendBtn.disabled = false; composeInput.focus();
      }
    }
    sendBtn.onclick = doSend;
    composeInput.addEventListener('keydown', function(e) { if (e.key === 'Enter') { e.preventDefault(); doSend(); } });

    overlay.querySelector('#p2pg-refresh').onclick = function() { state.epochKey = null; refresh(); };

    overlay.querySelector('#p2pg-invite').onclick = async function() {
      var btn = overlay.querySelector('#p2pg-invite');
      btn.disabled = true; btn.textContent = 'Minting…';
      try {
        var ticket = await createP2pInvite(groupId, name);
        if (!ticket) { btn.disabled = false; btn.textContent = '🔗 Create invite'; return; }
        ticketBox.style.display = 'block';
        ticketBox.innerHTML =
          '<div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:4px;">Share this ticket (valid 7 days). Joiners can self-admit even when you are offline.</div>' +
          '<textarea readonly style="width:100%;height:54px;font-size:0.7rem;font-family:monospace;background:var(--bg-primary);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);box-sizing:border-box;">' + esc(ticket) + '</textarea>' +
          '<button id="p2pg-copy" class="vr-btn" style="width:100%;margin-top:4px;font-size:0.75rem;">📋 Copy ticket</button>';
        ticketBox.querySelector('#p2pg-copy').onclick = function() {
          navigator.clipboard.writeText(ticket).then(function() {
            if (typeof addSystemMessage === 'function') addSystemMessage('Invite ticket copied.');
          });
        };
        btn.disabled = false; btn.textContent = '🔗 Create invite';
      } catch (e) {
        btn.disabled = false; btn.textContent = '🔗 Create invite';
        if (typeof addNotice === 'function') addNotice('Invite failed: ' + e.message, 'red', 6);
      }
    };

    composeInput.focus();
    refresh().then(function() {
      state.refreshTimer = setInterval(refresh, 4000);
    });
  }

  window.createP2pGroup = createP2pGroup;
  window.createP2pInvite = createP2pInvite;
  window.joinP2pGroupByTicket = joinP2pGroupByTicket;
  window.loadP2pGroups = loadP2pGroups;
  window.openP2pGroupDialog = openP2pGroupDialog;
})();
