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

  async function createP2pGroup(name) {
    if (!pqReady()) return notReady();
    name = (name || '').trim();
    if (!name) return;
    const { obj, blake3 } = await mods();
    const { submission } = await obj.buildGroupV1({ name, authorPublicKey: authorPub(), sign: signer(), blake3 });
    await postObject(submission);
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

  // Group detail dialog: roster + "create invite" (copyable ticket). Kept in a
  // standalone overlay so it doesn't touch the message area (group messaging is
  // Phase 2). `members` is an array of pubkey-hex strings.
  function openP2pGroupDialog(groupId, name, members) {
    members = members || [];
    const overlay = document.createElement('div');
    overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.8);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';
    const meHex = (typeof myKey === 'string') ? myKey : '';
    const roster = members.map((pk) => {
      const isMe = pk === meHex;
      const label = isMe ? 'You' : (pk.slice(0, 12) + '…');
      return '<div style="padding:4px 0;font-size:0.82rem;color:var(--text);">👤 ' + label + '</div>';
    }).join('') || '<div style="color:var(--text-muted);font-size:0.8rem;">No members yet.</div>';
    overlay.innerHTML =
      '<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);max-width:440px;width:100%;padding:var(--space-xl);box-sizing:border-box;">' +
        '<div style="font-weight:700;font-size:1rem;margin-bottom:var(--space-xs);">🔒 ' + esc(name || groupId.slice(0, 8)) + '</div>' +
        '<div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:var(--space-md);">P2P group · ' + members.length + ' member' + (members.length === 1 ? '' : 's') + ' · end-to-end messaging lands in Phase 2</div>' +
        '<div style="max-height:180px;overflow:auto;margin-bottom:var(--space-md);">' + roster + '</div>' +
        '<button id="p2pg-invite" class="vr-btn" style="width:100%;margin-bottom:var(--space-sm);">🔗 Create invite link</button>' +
        '<div id="p2pg-ticket" style="display:none;"></div>' +
        '<button id="p2pg-close" class="vr-btn vr-leave" style="width:100%;">Close</button>' +
      '</div>';
    document.body.appendChild(overlay);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
    overlay.querySelector('#p2pg-close').onclick = () => overlay.remove();
    overlay.querySelector('#p2pg-invite').onclick = async () => {
      const btn = overlay.querySelector('#p2pg-invite');
      btn.disabled = true; btn.textContent = 'Minting invite…';
      try {
        const ticket = await createP2pInvite(groupId, name);
        if (!ticket) { btn.disabled = false; btn.textContent = '🔗 Create invite link'; return; }
        const box = overlay.querySelector('#p2pg-ticket');
        box.style.display = 'block';
        box.innerHTML =
          '<div style="font-size:0.7rem;color:var(--text-muted);margin:var(--space-sm) 0 4px;">Share this ticket (valid 7 days). Anyone with it can join — even while you’re offline.</div>' +
          '<textarea readonly style="width:100%;height:64px;font-size:0.7rem;font-family:monospace;background:var(--bg-primary);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);box-sizing:border-box;">' + esc(ticket) + '</textarea>' +
          '<button id="p2pg-copy" class="vr-btn" style="width:100%;margin-top:4px;">📋 Copy</button>';
        box.querySelector('#p2pg-copy').onclick = () => {
          navigator.clipboard.writeText(ticket).then(() => { if (typeof addSystemMessage === 'function') addSystemMessage('Invite ticket copied.'); });
        };
        btn.style.display = 'none';
      } catch (e) {
        btn.disabled = false; btn.textContent = '🔗 Create invite link';
        if (typeof addNotice === 'function') addNotice('Invite failed: ' + e.message, 'red', 6);
      }
    };
  }

  window.createP2pGroup = createP2pGroup;
  window.createP2pInvite = createP2pInvite;
  window.joinP2pGroupByTicket = joinP2pGroupByTicket;
  window.loadP2pGroups = loadP2pGroups;
  window.openP2pGroupDialog = openP2pGroupDialog;
})();
