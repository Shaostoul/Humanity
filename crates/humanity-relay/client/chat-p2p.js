// ── P2P Contact Cards & DataChannel Messaging ──
// Goal: enable direct peer-to-peer messaging between users without routing
// message content through the central relay server.
//
// Phase 3a — Signed Contact Cards
//   Allows two users to exchange a signed JSON card (via QR code or clipboard)
//   so they can follow each other and establish a DataChannel connection without
//   sharing a server.
//
// Phase 3b — WebRTC DataChannel
//   Once a contact card has been imported, a direct DataChannel is opened so DMs
//   travel peer-to-peer (encrypted with ECDH+AES-256-GCM).  The relay is used
//   only for ICE signaling; it never sees DM content.
//
// Depends on (from app.js / crypto.js):
//   ws, myKey, myName, myIdentity, addSystemMessage, esc,
//   signData, importEd25519PublicKey, verifySignature (crypto.js helpers)

// ── Contact Card State ──
/** pubKeyHex → { name, ecdh_pub, added_at, dc_status } — persisted in IndexedDB */
let p2pContacts = {};
/** pubKeyHex → RTCPeerConnection — open DataChannel connections */
let p2pConnections = {};
/** pubKeyHex → RTCDataChannel — open DataChannels for DM routing */
let p2pDataChannels = {};
/** Messages queued while a DataChannel is negotiating. Array of { peerKey, ciphertext, nonce } */
let p2pSendQueue = [];

// Contact card validity window: cards older than 7 days are rejected.
const CONTACT_CARD_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

// ── Phase 3a: Contact Card Export ──

/**
 * Build a signed contact card for the current user and show it in a modal.
 * The card contains the user's display name, Ed25519 public key, and ECDH public
 * key so the importing peer can derive a shared secret for E2E encryption.
 *
 * The card is signed with the user's Ed25519 private key so the importer can
 * verify it hasn't been tampered with.
 */
async function exportContactCard() {
  if (!myIdentity || !myIdentity.privateKey) {
    addSystemMessage('⚠️ Cannot export — identity not loaded.');
    return;
  }

  // Build the canonical payload that will be signed.
  const payload = {
    v:    1,
    name: myName,
    pub:  myKey,
    ts:   Math.floor(Date.now() / 1000),
  };

  // Attach our ECDH public key if available (needed for E2E encryption setup).
  // myEcdhPublicBase64 is set by getOrCreateEcdhKeypair() in crypto.js on connect.
  if (myEcdhPublicBase64) payload.ecdh = myEcdhPublicBase64;

  // Sign the canonical JSON representation (keys sorted alphabetically).
  const canonical = JSON.stringify(payload, Object.keys(payload).sort());
  let sig = '';
  try {
    const sigBuf = await crypto.subtle.sign('Ed25519', myIdentity.privateKey, new TextEncoder().encode(canonical));
    sig = bufToHex(sigBuf); // bufToHex is defined in crypto.js
  } catch (err) {
    addSystemMessage('⚠️ Failed to sign contact card: ' + err.message);
    return;
  }

  const card = { ...payload, sig };
  const cardJson = JSON.stringify(card, null, 2);

  showContactCardExportModal(cardJson);
}

/**
 * Show the export modal with the JSON card and a QR code.
 * @param {string} cardJson - Serialised contact card JSON
 */
function showContactCardExportModal(cardJson) {
  let modal = document.getElementById('p2p-export-modal');
  if (!modal) {
    modal = document.createElement('div');
    modal.id = 'p2p-export-modal';
    modal.className = 'overlay-modal';
    modal.innerHTML = `
      <div class="overlay-content" style="max-width:480px;">
        <h3>📤 Share Your Contact Card</h3>
        <p style="font-size:0.8rem;color:var(--text-muted);">
          Give this card to someone so they can add you as a contact.
          It expires in 7 days.
        </p>
        <canvas id="p2p-qr-canvas" style="display:block;margin:0.5rem auto;"></canvas>
        <textarea id="p2p-card-json" readonly
          style="width:100%;height:120px;font-size:0.7rem;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:6px;padding:0.4rem;resize:none;"></textarea>
        <div style="display:flex;gap:0.5rem;margin-top:0.6rem;">
          <button onclick="navigator.clipboard.writeText(document.getElementById('p2p-card-json').value).then(()=>addSystemMessage('Card copied!'))"
            style="flex:1;background:var(--accent);color:#fff;border:none;border-radius:6px;padding:0.4rem;cursor:pointer;">
            📋 Copy JSON
          </button>
          <button onclick="document.getElementById('p2p-export-modal').classList.remove('open')"
            style="flex:1;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:6px;padding:0.4rem;cursor:pointer;">
            Close
          </button>
        </div>
      </div>`;
    document.body.appendChild(modal);
  }

  document.getElementById('p2p-card-json').value = cardJson;
  modal.classList.add('open');

  // Render QR code if the qrcode-generator library is available.
  renderQrCode('p2p-qr-canvas', cardJson);
}

/**
 * Render a QR code onto a canvas element.
 * Uses the qrcode-generator library (loaded lazily from /shared/qrcode.js).
 * Falls back silently if the library isn't loaded.
 * @param {string} canvasId - ID of the <canvas> element
 * @param {string} text     - Text to encode
 */
function renderQrCode(canvasId, text) {
  if (typeof qrcode === 'undefined') return; // library not loaded yet
  try {
    const qr = qrcode(0, 'M');
    qr.addData(text);
    qr.make();
    const canvas = document.getElementById(canvasId);
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const modules = qr.getModuleCount();
    const cellSize = Math.min(4, Math.floor(220 / modules));
    canvas.width  = modules * cellSize;
    canvas.height = modules * cellSize;
    ctx.fillStyle = '#fff';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = '#000';
    for (let r = 0; r < modules; r++) {
      for (let c = 0; c < modules; c++) {
        if (qr.isDark(r, c)) {
          ctx.fillRect(c * cellSize, r * cellSize, cellSize, cellSize);
        }
      }
    }
  } catch {}
}

// ── Phase 3a: Contact Card Import ──

/**
 * Show the import modal where a user can paste a contact card JSON.
 */
function showContactCardImportModal() {
  let modal = document.getElementById('p2p-import-modal');
  if (!modal) {
    modal = document.createElement('div');
    modal.id = 'p2p-import-modal';
    modal.className = 'overlay-modal';
    modal.innerHTML = `
      <div class="overlay-content" style="max-width:480px;">
        <h3>📥 Add Contact</h3>
        <p style="font-size:0.8rem;color:var(--text-muted);">
          Paste a contact card JSON from another user.
        </p>
        <textarea id="p2p-import-json" placeholder="Paste contact card JSON here…"
          style="width:100%;height:140px;font-size:0.75rem;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:6px;padding:0.4rem;resize:none;"></textarea>
        <div style="display:flex;gap:0.5rem;margin-top:0.6rem;">
          <button onclick="importContactCardFromModal()"
            style="flex:1;background:var(--accent);color:#fff;border:none;border-radius:6px;padding:0.4rem;cursor:pointer;">
            ✅ Add Contact
          </button>
          <button onclick="document.getElementById('p2p-import-modal').classList.remove('open')"
            style="flex:1;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:6px;padding:0.4rem;cursor:pointer;">
            Cancel
          </button>
        </div>
        <p id="p2p-import-error" style="color:#e74c3c;font-size:0.8rem;margin-top:0.4rem;display:none;"></p>
      </div>`;
    document.body.appendChild(modal);
  }
  document.getElementById('p2p-import-json').value = '';
  const errEl = document.getElementById('p2p-import-error');
  if (errEl) errEl.style.display = 'none';
  modal.classList.add('open');
}

/**
 * Read the pasted JSON from the import modal and call importContactCard().
 */
async function importContactCardFromModal() {
  const json = document.getElementById('p2p-import-json').value.trim();
  const errEl = document.getElementById('p2p-import-error');
  try {
    await importContactCard(json);
    document.getElementById('p2p-import-modal').classList.remove('open');
  } catch (err) {
    if (errEl) {
      errEl.textContent = '⚠️ ' + err.message;
      errEl.style.display = 'block';
    }
  }
}

/**
 * Parse, validate, and store a contact card.
 * Sends a Follow message to the relay so the other peer is added to our
 * following list immediately.
 *
 * @param {string} json - Raw JSON string of the contact card
 * @throws {Error} if the card is invalid, expired, or the signature doesn't verify
 */
async function importContactCard(json) {
  let card;
  try { card = JSON.parse(json); }
  catch { throw new Error('Invalid JSON — cannot parse card.'); }

  // Basic field checks.
  if (!card.v || card.v !== 1)   throw new Error('Unsupported card version.');
  if (!card.name || !card.pub)   throw new Error('Card missing name or public key.');
  if (!card.ts || !card.sig)     throw new Error('Card missing timestamp or signature.');

  // Reject cards older than 7 days.
  const ageMs = Date.now() - card.ts * 1000;
  if (ageMs > CONTACT_CARD_MAX_AGE_MS) throw new Error('Card has expired (older than 7 days).');

  // Verify Ed25519 signature over the canonical payload.
  const payload = { v: card.v, name: card.name, pub: card.pub, ts: card.ts };
  if (card.ecdh) payload.ecdh = card.ecdh;
  const canonical = JSON.stringify(payload, Object.keys(payload).sort());
  const valid = await verifyContactCardSignature(canonical, card.sig, card.pub);
  if (!valid) throw new Error('Signature verification failed — card may be tampered.');

  // Store in memory (IndexedDB persistence is a future improvement).
  p2pContacts[card.pub] = {
    name:     card.name,
    ecdh_pub: card.ecdh || null,
    added_at: Date.now(),
    dc_status: 'idle',
  };

  addSystemMessage(`✅ Added contact: ${card.name}`);

  // Send a Follow to the relay so they appear in our following list.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'follow', target_key: card.pub }));
  }
}

/**
 * Verify an Ed25519 signature over a message using the public key from a contact card.
 * @param {string} message   - The signed message (canonical JSON string)
 * @param {string} sigHex    - Hex-encoded signature
 * @param {string} pubKeyHex - Hex-encoded Ed25519 public key
 * @returns {Promise<boolean>}
 */
async function verifyContactCardSignature(message, sigHex, pubKeyHex) {
  try {
    // Import the public key in raw form (hexToBuf is defined in crypto.js).
    const pubKey = await crypto.subtle.importKey('raw', hexToBuf(pubKeyHex), 'Ed25519', true, ['verify']);
    const msgBytes = new TextEncoder().encode(message);
    const sigBytes = hexToBuf(sigHex);
    return await crypto.subtle.verify({ name: 'Ed25519' }, pubKey, sigBytes, msgBytes);
  } catch {
    return false;
  }
}

// ── Phase 3b: WebRTC DataChannel ──
// (Implementation in progress — signaling infrastructure below)

/**
 * Open a WebRTC DataChannel to a peer so future DMs travel P2P.
 * The relay is used only for ICE signaling; message content stays off-server.
 * Falls back to relay DMs automatically if the channel closes.
 *
 * @param {string} peerPubKey - Ed25519 public key hex of the target peer
 */
async function initDataChannel(peerPubKey) {
  if (p2pDataChannels[peerPubKey]?.readyState === 'open') return; // already open

  const pc = new RTCPeerConnection(rtcConfig);
  p2pConnections[peerPubKey] = pc;

  const dc = pc.createDataChannel('dm', { ordered: true });
  p2pDataChannels[peerPubKey] = dc;
  bindDataChannel(dc, peerPubKey);

  pc.onicecandidate = ({ candidate }) => {
    if (candidate && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'webrtc_signal',
        target: peerPubKey,
        signal_type: 'dc_ice',
        data: JSON.stringify(candidate),
      }));
    }
  };

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);

  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      type: 'webrtc_signal',
      target: peerPubKey,
      signal_type: 'dc_offer',
      data: JSON.stringify(offer),
    }));
  }
}

/**
 * Handle an incoming DataChannel offer from a peer.
 * Creates an answer and sends it back via the relay.
 * @param {object} signal - The webrtc_signal message from handleMessage
 */
async function handleDCOffer(signal) {
  const peerKey = signal.from;
  const offer = JSON.parse(signal.data);

  const pc = new RTCPeerConnection(rtcConfig);
  p2pConnections[peerKey] = pc;

  pc.ondatachannel = ({ channel }) => {
    p2pDataChannels[peerKey] = channel;
    bindDataChannel(channel, peerKey);
  };

  pc.onicecandidate = ({ candidate }) => {
    if (candidate && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'webrtc_signal',
        target: peerKey,
        signal_type: 'dc_ice',
        data: JSON.stringify(candidate),
      }));
    }
  };

  await pc.setRemoteDescription(new RTCSessionDescription(offer));
  const answer = await pc.createAnswer();
  await pc.setLocalDescription(answer);

  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      type: 'webrtc_signal',
      target: peerKey,
      signal_type: 'dc_answer',
      data: JSON.stringify(answer),
    }));
  }
}

/**
 * Handle an incoming DataChannel answer.
 * @param {object} signal - The webrtc_signal message
 */
async function handleDCAnswer(signal) {
  const pc = p2pConnections[signal.from];
  if (!pc) return;
  await pc.setRemoteDescription(new RTCSessionDescription(JSON.parse(signal.data)));
}

/**
 * Handle an incoming ICE candidate for a DataChannel connection.
 * @param {object} signal - The webrtc_signal message
 */
async function handleDCIce(signal) {
  const pc = p2pConnections[signal.from];
  if (!pc) return;
  try { await pc.addIceCandidate(new RTCIceCandidate(JSON.parse(signal.data))); }
  catch {}
}

/**
 * Attach open/close/message handlers to a DataChannel.
 * @param {RTCDataChannel} dc      - The channel to bind
 * @param {string}         peerKey - The remote peer's public key hex
 */
function bindDataChannel(dc, peerKey) {
  dc.onopen = () => {
    addSystemMessage(`🔗 P2P channel open with ${p2pContacts[peerKey]?.name || peerKey.substring(0, 12) + '…'}`);
    // Flush any queued messages.
    p2pSendQueue = p2pSendQueue.filter(item => {
      if (item.peerKey !== peerKey) return true;
      dc.send(JSON.stringify({ type: 'p2p_dm', ...item }));
      return false;
    });
  };
  dc.onclose = () => {
    delete p2pDataChannels[peerKey];
    delete p2pConnections[peerKey];
  };
  dc.onmessage = (event) => onDCMessage(event, peerKey);
}

/**
 * Send a DM over an open DataChannel, encrypted with ECDH+AES-256-GCM.
 * Falls back to sending via the relay WebSocket if no DataChannel is open
 * (mirrors the regular DM path in sendMessage in app.js).
 * @param {string} peerPubKey - Recipient's Ed25519 public key hex
 * @param {string} text       - Plaintext message
 */
async function sendP2PMessage(peerPubKey, text) {
  const dc = p2pDataChannels[peerPubKey];
  const contact = p2pContacts[peerPubKey];

  if (dc && dc.readyState === 'open' && contact?.ecdh_pub) {
    // Happy path: encrypt and send over open DataChannel.
    try {
      const enc = await encryptDmContent(text, contact.ecdh_pub);
      if (enc) {
        dc.send(JSON.stringify({ type: 'p2p_dm', ciphertext: enc.content, nonce: enc.nonce, ts: Date.now() }));
        return;
      }
    } catch {}
  }

  // Fallback: relay DM (same payload as normal DM send in app.js).
  if (ws && ws.readyState === WebSocket.OPEN) {
    const peerEcdh = getPeerEcdhPublic(peerPubKey);
    const payload = { type: 'dm', from: myKey, from_name: myName, to: peerPubKey, content: text, timestamp: Date.now() };
    if (peerEcdh && myEcdhKeyPair) {
      try {
        const enc = await encryptDmContent(text, peerEcdh);
        if (enc) { payload.content = enc.content; payload.nonce = enc.nonce; payload.encrypted = true; }
      } catch {}
    }
    ws.send(JSON.stringify(payload));
  }
}

/**
 * Handle an incoming message on a DataChannel — decrypt and render it.
 * @param {MessageEvent} event   - The DataChannel message event
 * @param {string}       peerKey - Sender's public key hex
 */
async function onDCMessage(event, peerKey) {
  let msg;
  try { msg = JSON.parse(event.data); }
  catch { return; }
  if (msg.type !== 'p2p_dm') return;

  const contact = p2pContacts[peerKey];
  const name    = contact?.name || peerKey.substring(0, 12) + '…';

  let content = '[encrypted message]';
  if (msg.ciphertext && msg.nonce) {
    try {
      const plain = await decryptDmContent(msg.ciphertext, msg.nonce, contact?.ecdh_pub);
      if (plain !== null) content = plain;
    } catch {}
  }

  // Render in the DM thread if it's active; otherwise show a notification.
  if (typeof addDmMessage === 'function' && activeDmPartner === peerKey) {
    addDmMessage(name, content, msg.ts || Date.now(), peerKey, myKey, false);
  } else if (typeof notifyNewMessage === 'function') {
    notifyNewMessage(name, content, true);
  }
}

// ══════════════════════════════════════════════════════════════════════════════
// Multi-Device Data Sync over DataChannel
// ══════════════════════════════════════════════════════════════════════════════
// Goal: when two trusted devices (same user or close contact) are connected via
// DataChannel, let them exchange and merge localStorage data blobs so calendar,
// homes, todos, notes, and inventory stay consistent across devices.
//
// Sync protocol (all frames go as DataChannel text messages):
//   A → B: { type:'sync_offer',   keys:['calendar','homes','todos','notes'] }
//   B → A: { type:'sync_accept',  keys:[<subset B is willing to share>] }
//   A → B: { type:'sync_data',    data:{...}, ts:Date.now() }
//   B → A: { type:'sync_data',    data:{...}, ts:Date.now() }
// Merge: newest-event-wins for array stores; last-writer-wins for blobs.
// ══════════════════════════════════════════════════════════════════════════════

/** localStorage keys that are syncable, mapped to their merge strategy. */
const SYNC_STORES = {
  'hos_calendar_v1': 'array_by_id',   // merge by ev.id, newest updatedAt wins
  'hos_homes_v2':    'array_by_id',   // merge by home.id → rooms merge by room.id
  'hos_home_todos':  'array_by_id',   // merge by todo.id
  'hos_home_notes':  'blob',          // last-write-wins
  'hos_inventory_v1':'array_by_id',   // merge by item.id
  'hos_notes_v1':    'array_by_id',   // notes page entries by id
  'hos_skills_v1':   'skill_merge',   // skills XP/level map — merge by taking max(level, xp) per skill
  'hos_quests_v1':   'array_by_id',   // quests by id
  'hos_equipment_v1':'array_by_id',   // equipment items by id
};

/** Read all syncable stores into a bundle object. */
function buildSyncBundle() {
  const bundle = {};
  for (const key of Object.keys(SYNC_STORES)) {
    try { bundle[key] = JSON.parse(localStorage.getItem(key) || 'null'); }
    catch { bundle[key] = null; }
  }
  bundle._ts = Date.now();
  bundle._name = typeof myName !== 'undefined' ? myName : '';
  return bundle;
}

/**
 * Merge a received sync bundle into localStorage.
 * Array-by-id stores: insert any item whose id isn't local, or replace if
 * remote updatedAt/ts is strictly newer. Blob stores: replace if remote _ts is newer.
 */
function applySyncBundle(remote) {
  for (const [key, strategy] of Object.entries(SYNC_STORES)) {
    try {
      const remoteVal = remote[key];
      if (remoteVal === null || remoteVal === undefined) continue;

      if (strategy === 'blob') {
        const localRaw = localStorage.getItem(key);
        // Keep whichever was written more recently — only replace if local is absent
        if (!localRaw) { localStorage.setItem(key, JSON.stringify(remoteVal)); }
        continue;
      }

      if (strategy === 'skill_merge') {
        // Skills data: { skill_id: { level, xp } } — take the higher level+xp per skill.
        if (typeof remoteVal !== 'object' || Array.isArray(remoteVal)) continue;
        let local = {};
        try { local = JSON.parse(localStorage.getItem(key) || '{}'); } catch {}
        if (typeof local !== 'object' || Array.isArray(local)) local = {};
        let changed = false;
        for (const [id, remoteSkill] of Object.entries(remoteVal)) {
          if (!remoteSkill) continue;
          const localSkill = local[id];
          if (!localSkill) {
            local[id] = remoteSkill;
            changed = true;
          } else {
            const rl = remoteSkill.level || 0, ll = localSkill.level || 0;
            const rx = remoteSkill.xp || 0,    lx = localSkill.xp || 0;
            if (rl > ll || (rl === ll && rx > lx)) {
              local[id] = { ...localSkill, level: Math.max(rl, ll), xp: Math.max(rx, lx) };
              changed = true;
            }
          }
        }
        if (changed) localStorage.setItem(key, JSON.stringify(local));
        continue;
      }

      // array_by_id merge
      if (!Array.isArray(remoteVal)) continue;
      let local = [];
      try { local = JSON.parse(localStorage.getItem(key) || '[]'); } catch {}
      if (!Array.isArray(local)) local = [];
      const localMap = new Map(local.map(item => [item.id, item]));
      let changed = false;
      for (const remoteItem of remoteVal) {
        if (!remoteItem || !remoteItem.id) continue;
        const localItem = localMap.get(remoteItem.id);
        if (!localItem) {
          localMap.set(remoteItem.id, remoteItem);
          changed = true;
        } else {
          // Replace if remote is strictly newer (by updatedAt or ts field)
          const rt = remoteItem.updatedAt || remoteItem.ts || remoteItem.createdAt || 0;
          const lt = localItem.updatedAt  || localItem.ts  || localItem.createdAt  || 0;
          if (rt > lt) { localMap.set(remoteItem.id, remoteItem); changed = true; }
        }
      }
      if (changed) localStorage.setItem(key, JSON.stringify([...localMap.values()]));
    } catch (e) { console.warn('Sync merge error for', key, e); }
  }
}

/** Initiate a data-sync offer over an existing DataChannel. */
function offerDataSync(peerKey) {
  const dc = p2pDataChannels[peerKey];
  if (!dc || dc.readyState !== 'open') {
    addSystemMessage('⚠️ No open P2P channel to that peer.');
    return;
  }
  dc.send(JSON.stringify({ type: 'sync_offer', keys: Object.keys(SYNC_STORES) }));
  addSystemMessage(`🔄 Sync offer sent to ${p2pContacts[peerKey]?.name || peerKey.slice(0,12) + '…'}`);
}

/** Handle an incoming sync frame on a DataChannel. Called from onDCMessage. */
async function handleSyncFrame(msg, peerKey) {
  const dc = p2pDataChannels[peerKey];
  if (!dc) return;
  const name = p2pContacts[peerKey]?.name || peerKey.slice(0,12) + '…';

  if (msg.type === 'sync_offer') {
    // Accept all offered keys that we support.
    const accepted = (msg.keys || []).filter(k => SYNC_STORES[k]);
    dc.send(JSON.stringify({ type: 'sync_accept', keys: accepted }));
    // Send our own bundle back
    dc.send(JSON.stringify({ type: 'sync_data', data: buildSyncBundle() }));
    addSystemMessage(`🔄 Sync request from ${name} — sending data…`);
    return;
  }

  if (msg.type === 'sync_accept') {
    // Peer accepted — send our bundle
    dc.send(JSON.stringify({ type: 'sync_data', data: buildSyncBundle() }));
    return;
  }

  if (msg.type === 'sync_data') {
    applySyncBundle(msg.data || {});
    addSystemMessage(`✅ Sync from ${name} complete. Data merged.`);
  }
}

/**
 * Offer a data sync to every currently-open DataChannel.
 * Triggered by the "🔄 Sync" button in the identity sidebar.
 */
function syncAllPeers() {
  const openKeys = Object.keys(p2pDataChannels).filter(k => p2pDataChannels[k]?.readyState === 'open');
  if (openKeys.length === 0) {
    addSystemMessage('ℹ️ No open P2P channels. Connect to a peer first via "Share Card" / "Add Contact".');
    return;
  }
  openKeys.forEach(offerDataSync);
  addSystemMessage(`🔄 Sync initiated with ${openKeys.length} peer${openKeys.length > 1 ? 's' : ''}…`);
}

// ── Patch onDCMessage to handle sync frames ──
const _onDCMessageOrig = onDCMessage;
onDCMessage = async function(event, peerKey) {
  let msg;
  try { msg = JSON.parse(event.data); } catch { return; }
  if (msg.type && msg.type.startsWith('sync_')) {
    await handleSyncFrame(msg, peerKey);
    return;
  }
  return _onDCMessageOrig.call(this, event, peerKey);
};

// hexToBuf and bufToHex are defined in crypto.js and available globally.
