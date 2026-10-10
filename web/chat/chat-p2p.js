// ── P2P: inject overlay modal CSS (no style.css dependency) ──
(function () {
  if (document.getElementById('p2p-modal-styles')) return;
  const s = document.createElement('style');
  s.id = 'p2p-modal-styles';
  s.textContent = `
    .overlay-modal {
      display: none;
      position: fixed;
      inset: 0;
      background: rgba(0,0,0,0.7);
      z-index: 8500;
      align-items: center;
      justify-content: center;
    }
    .overlay-modal.open { display: flex; }
    .overlay-modal .overlay-content {
      background: var(--bg-panel, #1a1a1a);
      border: 1px solid var(--border, #333);
      border-radius: var(--radius-lg);
      padding: 1.2rem;
      width: 90%;
      max-width: 480px;
      box-shadow: 0 8px 32px rgba(0,0,0,0.6);
    }
    .overlay-modal .overlay-content h3 {
      margin: 0 0 0.5rem;
      font-size: 1rem;
      color: var(--text, #ddd);
    }
  `;
  document.head.appendChild(s);
})();

// ── P2P Contact Cards & DataChannel Messaging ──
// Goal: enable direct peer-to-peer messaging between users without routing
// message content through the central relay server.
//
// Phase 3a, Signed Contact Cards
//   Allows two users to exchange a signed JSON card (via QR code or clipboard)
//   so they can follow each other and establish a DataChannel connection without
//   sharing a server.
//
// Phase 3b, WebRTC DataChannel
//   Once a contact card has been imported, a direct DataChannel is opened so DMs
//   travel peer-to-peer (encrypted with ECDH+AES-256-GCM).  The relay is used
//   only for ICE signaling; it never sees DM content.
//
// Depends on (from app.js / crypto.js):
//   ws, myKey, myName, myIdentity, addSystemMessage, esc,
//   signData, importEd25519PublicKey, verifySignature (crypto.js helpers)

// ── Contact Card State ──
/** pubKeyHex → { name, ecdh_pub, added_at, dc_status }, persisted in IndexedDB */
let p2pContacts = {};
/** pubKeyHex → RTCPeerConnection, open DataChannel connections */
let p2pConnections = {};
/** pubKeyHex → RTCDataChannel, open DataChannels for DM routing */
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
  if (!myIdentity || !(myIdentity.privateKey || myIdentity.seed32)) {
    addSystemMessage('⚠️ Cannot export, identity not loaded.');
    return;
  }

  // Build the canonical payload that will be signed.
  const payload = {
    v:    1,
    name: myName,
    pub:  myKey,
    ts:   Math.floor(Date.now() / 1000),
  };

  // Full-PQ: attach our Kyber768 public key so the peer can seal P2P
  // DMs to us (pure ML-KEM, same envelope as relay DMs). Set by
  // attachPqIdentity() in crypto.js on connect.
  if (myKyberPublicBase64) payload.kyber = myKyberPublicBase64;

  // Sign the canonical JSON with Dilithium3, `pub` IS the Dilithium
  // identity key, so the card must be Dilithium-signed (not Ed25519).
  const canonical = JSON.stringify(payload, Object.keys(payload).sort());
  let sig = '';
  try {
    const sigBytes = await window.pqSignMessage(myDilithiumSecret, new TextEncoder().encode(canonical));
    if (!sigBytes) throw new Error('post-quantum identity not ready');
    sig = bufToHex(sigBytes); // bufToHex is defined in crypto.js
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
        <h3>${hosIcon('save', 14)} Share Your Contact Card</h3>
        <p style="font-size:0.8rem;color:var(--text-muted);">
          Give this card to someone so they can add you as a contact.
          It expires in 7 days.
        </p>
        <canvas id="p2p-qr-canvas" style="display:block;margin:var(--space-md) auto;"></canvas>
        <textarea id="p2p-card-json" readonly
          style="width:100%;height:120px;font-size:0.7rem;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md);resize:none;"></textarea>
        <div style="display:flex;gap:var(--space-md);margin-top:var(--space-lg);">
          <button id="p2p-copy-btn"
            onclick="(function(btn){navigator.clipboard.writeText(document.getElementById('p2p-card-json').value).then(function(){btn.innerHTML=hosIcon('check',14)+' Copied!';btn.style.background='#1a5c2a';setTimeout(function(){btn.innerHTML=hosIcon('copy',14)+' Copy JSON';btn.style.background='';},2000);}).catch(function(){btn.innerHTML=hosIcon('close',14)+' Failed';setTimeout(function(){btn.innerHTML=hosIcon('copy',14)+' Copy JSON';},2000);})})(this)"
            style="flex:1;background:var(--accent);color:#fff;border:none;border-radius:var(--radius);padding:var(--space-md);cursor:pointer;">
            ${hosIcon('copy', 14)} Copy JSON
          </button>
          <button onclick="var m=document.getElementById('p2p-export-modal');if(m)m.classList.remove('open');"
            style="flex:1;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md);cursor:pointer;">
            ✕ Close
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
        <h3>${hosIcon('save', 14)} Add Contact</h3>
        <p style="font-size:0.8rem;color:var(--text-muted);">
          Paste a contact card JSON from another user.
        </p>
        <textarea id="p2p-import-json" placeholder="Paste contact card JSON here…"
          style="width:100%;height:140px;font-size:0.75rem;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md);resize:none;"></textarea>
        <div style="display:flex;gap:var(--space-md);margin-top:var(--space-lg);">
          <button onclick="importContactCardFromModal()"
            style="flex:1;background:var(--accent);color:#fff;border:none;border-radius:var(--radius);padding:var(--space-md);cursor:pointer;">
            ${hosIcon('check', 14)} Add Contact
          </button>
          <button onclick="document.getElementById('p2p-import-modal').classList.remove('open')"
            style="flex:1;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-md);cursor:pointer;">
            Cancel
          </button>
        </div>
        <p id="p2p-import-error" style="color:var(--danger,#e64033);font-size:0.8rem;margin-top:var(--space-md);display:none;"></p>
        <p id="p2p-import-success" style="color:var(--success,#2a6);font-size:0.85rem;font-weight:600;margin-top:var(--space-md);display:none;"></p>
      </div>`;
    document.body.appendChild(modal);
  }
  document.getElementById('p2p-import-json').value = '';
  const errEl = document.getElementById('p2p-import-error');
  if (errEl) { errEl.style.display = 'none'; errEl.textContent = ''; }
  const okEl = document.getElementById('p2p-import-success');
  if (okEl)  { okEl.style.display = 'none';  okEl.textContent = ''; }
  modal.classList.add('open');
}

/**
 * Read the pasted JSON from the import modal and call importContactCard().
 */
async function importContactCardFromModal() {
  const json = document.getElementById('p2p-import-json').value.trim();
  const errEl = document.getElementById('p2p-import-error');
  const okEl  = document.getElementById('p2p-import-success');
  if (errEl) { errEl.style.display = 'none'; errEl.textContent = ''; }
  if (okEl)  { okEl.style.display  = 'none'; okEl.textContent  = ''; }
  try {
    const name = await importContactCard(json);
    // Clear textarea so it's blank if modal is reopened
    const ta = document.getElementById('p2p-import-json');
    if (ta) ta.value = '';
    // Show success banner, then auto-close after 1.5s
    if (okEl) {
      okEl.textContent = '✅ Contact added' + (name ? ': ' + name : '') + '!';
      okEl.style.display = 'block';
    }
    setTimeout(function () {
      const m = document.getElementById('p2p-import-modal');
      if (m) m.classList.remove('open');
    }, 1500);
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
  catch { throw new Error('Invalid JSON, cannot parse card.'); }

  // Basic field checks.
  if (!card.v || card.v !== 1)   throw new Error('Unsupported card version.');
  if (!card.name || !card.pub)   throw new Error('Card missing name or public key.');
  if (!card.ts || !card.sig)     throw new Error('Card missing timestamp or signature.');

  // Reject cards older than 7 days.
  const ageMs = Date.now() - card.ts * 1000;
  if (ageMs > CONTACT_CARD_MAX_AGE_MS) throw new Error('Card has expired (older than 7 days).');

  // Verify the Dilithium3 signature over the canonical payload.
  const payload = { v: card.v, name: card.name, pub: card.pub, ts: card.ts };
  if (card.kyber) payload.kyber = card.kyber;
  const canonical = JSON.stringify(payload, Object.keys(payload).sort());
  const valid = await verifyContactCardSignature(canonical, card.sig, card.pub);
  if (!valid) throw new Error('Signature verification failed, card may be tampered.');

  // Store in memory (IndexedDB persistence is a future improvement).
  p2pContacts[card.pub] = {
    name:      card.name,
    kyber_pub: card.kyber || null,
    added_at:  Date.now(),
    dc_status: 'idle',
  };

  addSystemMessage(`✅ Added contact: ${card.name}`);

  // Follows removal (2026-08-24): following is client-side; a sealed
  // control message notifies them and starts the friendship exchange.
  if (typeof setFollowLocal === 'function') {
    setFollowLocal(card.pub, true);
  }

  return card.name;
}

/**
 * Verify a Dilithium3 signature over a message using the public key from a contact card.
 * @param {string} message   - The signed message (canonical JSON string)
 * @param {string} sigHex    - Hex-encoded Dilithium3 signature
 * @param {string} pubKeyHex - Hex-encoded Dilithium3 public key
 * @returns {Promise<boolean>}
 */
async function verifyContactCardSignature(message, sigHex, pubKeyHex) {
  try {
    // Full-PQ: `pub` is the Dilithium3 identity key; verify with ML-DSA-65.
    // hexToBuf is defined in crypto.js.
    const pubBytes = hexToBuf(pubKeyHex);
    const sigBytes = hexToBuf(sigHex);
    const msgBytes = new TextEncoder().encode(message);
    return await window.pqVerifyMessage(pubBytes, msgBytes, sigBytes);
  } catch {
    return false;
  }
}

// ── Phase 3b: WebRTC DataChannel ──
// (Implementation in progress, signaling infrastructure below)

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
        to: peerPubKey,
        signal_type: 'dc_ice',
        data: JSON.stringify(candidate),
      }));
    }
  };

  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);

  if (ws && ws.readyState === WebSocket.OPEN) {
    const signal = { type: 'webrtc_signal', to: peerPubKey, signal_type: 'dc_offer', data: JSON.stringify(offer) };
    // The target's friendship pass, when we hold one (passes v2, chat-social.js).
    const pass = typeof friendPassFor === 'function' ? friendPassFor(peerPubKey) : null;
    if (pass) signal.friend_cert = pass;
    ws.send(JSON.stringify(signal));
  }
}

// WHO MAY OPEN A DIRECT CONNECTION TO THIS BROWSER (2026-10-09). Answering a
// direct-connection offer hands the other side this device's network address
// (which gives away a rough location and the internet provider) and opens a
// channel to them. Until this date handleDCOffer answered every offer, and the
// relay forwards one from anyone online, so anyone could learn your address
// without a call, and open a channel to try the data sync on
// (docs/design/blocking-and-safe-mode.md, defects 3.7.1 and 3.7.2, section 7.1
// item 4). Now an offer is answered only from:
//   - your own key (your other devices, signed in with your recovery phrase),
//   - a contact you added from their contact card (p2pContacts),
//   - a member of a P2P group you are in (your group list, or the open group's roster),
//   - the person you are in a call with, once the call was accepted,
//   - someone in the voice room you are in.
// Anyone else gets no answer at all, so they learn nothing, not even that the
// offer arrived. Someone you blocked gets none either, even when they are one
// of the people above (step C). These are the people every feature that opens a direct
// connection reaches anyway: the group mesh (ensureGroupMesh) offers only to
// roster members, and a call or a voice room already connects its people
// directly. Test: scripts/tests/p2p-direct-offers.test.js

/** The same key? Hex keys are compared without regard to letter case. */
function sameKeyHex(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a !== '' && a.toLowerCase() === b.toLowerCase();
}

/** Is `peerKey` in the roster of a P2P group this browser knows you are in? */
function isP2pGroupMate(peerKey) {
  // The group list (loaded on connect, from /api/v2/groups?pubkey=) carries
  // each group's members.
  for (const g of (window._p2pGroups || [])) {
    if (g && Array.isArray(g.members) && g.members.some(k => sameKeyHex(k, peerKey))) return true;
  }
  // The open group's roster, loaded when it was opened (it can be newer than the list).
  const ag = window.activeP2pGroup;
  if (ag && ag.fpToKey && Object.values(ag.fpToKey).some(k => sameKeyHex(k, peerKey))) return true;
  return false;
}

/** Is `peerKey` the person you are in a call with, or someone in your voice room? */
function isCallOrRoomPartner(peerKey) {
  // A 1:1 call counts only once it was accepted: someone who is merely ringing
  // you is not a partner yet (chat-voice-calls.js).
  if (typeof callState !== 'undefined' && callState === 'in-call'
      && typeof callPeerKey !== 'undefined' && sameKeyHex(callPeerKey, peerKey)) return true;
  // The voice room you are in (chat-voice-rooms.js): its roster from the relay,
  // or a voice connection already made to them in this room.
  const roomId = window._currentRoomId;
  if (roomId) {
    const room = (window._voiceChannels || []).find(c => String(c.id) === String(roomId));
    if (room && (room.participants || []).some(p => p && sameKeyHex(p.public_key, peerKey))) return true;
    if (window._roomPeerConnections && Object.keys(window._roomPeerConnections).some(k => sameKeyHex(k, peerKey))) return true;
  }
  return false;
}

/**
 * May this browser answer a direct-connection offer from `peerKey`? See the
 * note above. Never someone I blocked (step C, 2026-10-09), whatever else
 * they are to me: a contact, a group mate, a voice-room neighbour.
 */
function mayAnswerDirectOffer(peerKey) {
  if (typeof peerKey !== 'string' || peerKey === '') return false;
  if (sameKeyHex(typeof myKey === 'string' ? myKey : '', peerKey)) return true;
  if (typeof isBlockedKey === 'function' && isBlockedKey(peerKey)) return false;
  if (Object.keys(p2pContacts).some(k => sameKeyHex(k, peerKey))) return true;
  if (isP2pGroupMate(peerKey)) return true;
  if (isCallOrRoomPartner(peerKey)) return true;
  return false;
}

/**
 * Handle an incoming DataChannel offer from a peer.
 * Creates an answer and sends it back via the relay, but only to someone
 * mayAnswerDirectOffer allows; anyone else is ignored without an answer.
 * @param {object} signal - The webrtc_signal message from handleMessage
 */
async function handleDCOffer(signal) {
  const peerKey = signal.from;
  if (!mayAnswerDirectOffer(peerKey)) {
    // Silent on screen on purpose (a stranger could otherwise fill the chat
    // with notices); the console line is for whoever is debugging a connection.
    console.info('Ignored a direct-connection offer from ' + String(peerKey || '').slice(0, 12)
      + '…: not your own device, a contact, a group member, or a call or voice-room partner.');
    return;
  }
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
        to: peerKey,
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
      to: peerKey,
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

  if (dc && dc.readyState === 'open' && contact?.kyber_pub) {
    // Happy path: sealed-sender v2 envelope over the open DataChannel.
    // Same signed-inner format as the relay path, so the receiver
    // verifies authorship identically (and can dedupe against relay
    // copies of the same message).
    try {
      const sentTs = Date.now();
      const preimage = `hum/dm/v2\n${myKey}\n${peerPubKey}\n${sentTs}\n${text}`;
      const sigBytes = await window.pqSignMessage(myDilithiumSecret, new TextEncoder().encode(preimage));
      if (sigBytes) {
        const inner = { v: 2, from: myKey, to: peerPubKey, ts: sentTs, text, sig: btoa(String.fromCharCode(...sigBytes)) };
        const sealed = await window.pqDmSeal(contact.kyber_pub, JSON.stringify(inner));
        if (sealed) {
          const env = JSON.stringify({ v: 2, ek_ct_b64: sealed.ek_ct_b64, nonce_b64: sealed.nonce_b64, ct_b64: sealed.ct_b64 });
          dc.send(JSON.stringify({ type: 'p2p_dm', env }));
          if (window.hosDmStore && hosDmStore.ready) await hosDmStore.insert(inner);
          return;
        }
      }
    } catch {}
  }

  // Fallback: relay mailbox deposit, FAIL CLOSED. Only ever send sealed
  // v2 envelopes; there is no plaintext field in the protocol at all.
  if (ws && ws.readyState === WebSocket.OPEN) {
    const sentTs = Date.now();
    let built = null;
    try { built = await pqBuildDmPuts(text, peerPubKey, sentTs); } catch {}
    if (!built) return;
    ws.send(JSON.stringify(built.recipientPut));
    ws.send(JSON.stringify(built.selfPut));
    if (window.hosDmStore && hosDmStore.ready) await hosDmStore.insert(built.inner);
  }
}

/**
 * Handle an incoming message on a DataChannel, decrypt and render it.
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

  // Sealed-sender v2: open with our own key, verify the inner Dilithium
  // signature, and REQUIRE the verified sender to match the DataChannel
  // peer — a spoofed inner never renders.
  if (!msg.env) return;
  let inner = null;
  try { inner = await pqOpenDmEnvelope(msg.env); } catch {}
  if (!inner || inner.from !== peerKey) return;
  // From someone I blocked (or a block note, which never comes this way):
  // dropped before it is stored or notified, as for mail (chat-privacy.js).
  if (typeof blockScreenDm === 'function' && blockScreenDm(inner)) return;
  // A contact request, or a DM from someone my "who can reach me" settings
  // refuse (step B): a request, name only, its text dropped, as for mail
  // (chat-privacy.js ingestContactRequest, reachScreenDm).
  if (typeof ingestContactRequest === 'function' && await ingestContactRequest(inner)) return;
  if (typeof reachScreenDm === 'function' && reachScreenDm(inner)) return;
  if (window.hosDmStore && hosDmStore.ready) {
    const isNew = await hosDmStore.insert(inner);
    if (!isNew) return; // already have it (relay copy arrived first)
  }

  // Render in the DM thread if it's active; otherwise show a notification.
  if (typeof addDmMessage === 'function' && activeDmPartner === peerKey) {
    addDmMessage(name, inner.text, inner.ts || Date.now(), peerKey, myKey, true);
  } else if (typeof notifyNewMessage === 'function') {
    notifyNewMessage(name, inner.text, true);
  }
  if (typeof loadDmListFromStore === 'function') loadDmListFromStore();
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
  'hos_skills_v1':   'skill_merge',   // skills XP/level map, merge by taking max(level, xp) per skill
  'hos_quests_v1':   'array_by_id',   // quests by id
  'hos_equipment_v1':'array_by_id',   // equipment items by id
  'hos_logbook_v1':  'array_by_id',   // logbook journal entries by id
  'map_pins_v1':     'blob',          // map pins array (no id field; last-write-wins)
  'map_polygons_v1': 'array_by_id',   // map zone polygons by id
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
        // Keep whichever was written more recently, only replace if local is absent
        if (!localRaw) { localStorage.setItem(key, JSON.stringify(remoteVal)); }
        continue;
      }

      if (strategy === 'skill_merge') {
        // Skills data: { skill_id: { level, xp } }, take the higher level+xp per skill.
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

// DATA SYNC IS BETWEEN YOUR OWN DEVICES ONLY (2026-10-09). Your devices share one
// identity (the same recovery phrase gives the same key), so "own device" means the
// peer's key is yours. Until this date ANY peer who opened a direct connection could
// send a sync_offer and get back the whole bundle above (calendar, home records,
// notes, inventory, map pins, which can locate a real home), and any sync_data it
// sent was merged into this browser's storage unasked. Now: a sync frame from anyone
// else is ignored and said so in chat; data from your own device is merged only if
// this browser asked for it, and only after you confirm.
// (docs/design/blocking-and-safe-mode.md, defect 3.7.1; test: scripts/tests/p2p-sync-own-devices.test.js)

/** Is `peerKey` this identity's own key, that is, another of your own devices? */
function isOwnDevice(peerKey) {
  return typeof myKey === 'string' && myKey !== '' && peerKey === myKey;
}

/** Own devices this browser asked to sync with, or agreed to sync with, this session. */
const syncAskedFrom = new Set();

/** Ask before your other device's data is merged into this browser. */
function confirmSyncMerge(name) {
  if (typeof confirm !== 'function') return false;
  return confirm(`Merge the calendar, homes, notes, inventory and map data from your other device (${name}) into this browser?`);
}

/** Initiate a data-sync offer over an existing DataChannel (your own devices only). */
function offerDataSync(peerKey) {
  const dc = p2pDataChannels[peerKey];
  if (!dc || dc.readyState !== 'open') {
    addSystemMessage('⚠️ No open P2P channel to that peer.');
    return;
  }
  if (!isOwnDevice(peerKey)) {
    addSystemMessage('Data sync works only between your own devices (the ones signed in with your recovery phrase).');
    return;
  }
  syncAskedFrom.add(peerKey);
  dc.send(JSON.stringify({ type: 'sync_offer', keys: Object.keys(SYNC_STORES) }));
  addSystemMessage(`🔄 Sync offer sent to ${p2pContacts[peerKey]?.name || peerKey.slice(0,12) + '…'}`);
}

/** Handle an incoming sync frame on a DataChannel. Called from onDCMessage. */
async function handleSyncFrame(msg, peerKey) {
  const dc = p2pDataChannels[peerKey];
  if (!dc) return;
  const name = p2pContacts[peerKey]?.name || peerKey.slice(0,12) + '…';

  if (!isOwnDevice(peerKey)) {
    if (msg.type === 'sync_offer' || msg.type === 'sync_data') {
      addSystemMessage(`Ignored a data-sync request from ${name}: sync works only between your own devices, and nothing was sent or changed.`);
    }
    return;
  }

  if (msg.type === 'sync_offer') {
    // Your own other device asked: share with it, and take its data back (after
    // you confirm) since this is a two-way sync.
    syncAskedFrom.add(peerKey);
    const accepted = (msg.keys || []).filter(k => SYNC_STORES[k]);
    dc.send(JSON.stringify({ type: 'sync_accept', keys: accepted }));
    dc.send(JSON.stringify({ type: 'sync_data', data: buildSyncBundle() }));
    addSystemMessage(`🔄 Sync request from your other device (${name}), sending data…`);
    return;
  }

  if (msg.type === 'sync_accept') {
    if (!syncAskedFrom.has(peerKey)) return;
    dc.send(JSON.stringify({ type: 'sync_data', data: buildSyncBundle() }));
    return;
  }

  if (msg.type === 'sync_data') {
    if (!syncAskedFrom.has(peerKey)) return;
    syncAskedFrom.delete(peerKey);
    if (!confirmSyncMerge(name)) {
      addSystemMessage(`Sync from ${name} not merged.`);
      return;
    }
    applySyncBundle(msg.data || {});
    addSystemMessage(`✅ Sync from ${name} complete. Data merged.`);
  }
}

/**
 * Offer a data sync to every open DataChannel to one of your own devices.
 * Triggered by the "🔄 Sync" button in the identity sidebar.
 */
function syncAllPeers() {
  const openKeys = Object.keys(p2pDataChannels).filter(k => p2pDataChannels[k]?.readyState === 'open' && isOwnDevice(k));
  if (openKeys.length === 0) {
    addSystemMessage('ℹ️ No open connection to another of your own devices. Data sync works only between devices signed in with your recovery phrase.');
    return;
  }
  openKeys.forEach(offerDataSync);
  addSystemMessage(`🔄 Sync initiated with ${openKeys.length} of your device${openKeys.length > 1 ? 's' : ''}…`);
}

// ── Patch onDCMessage to handle sync frames ──
const _onDCMessageOrig = onDCMessage;
onDCMessage = async function(event, peerKey) {
  let msg;
  try { msg = JSON.parse(event.data); } catch { return; }
  // Phase 3: P2P group messages, a signed group_msg_v1 object pushed by a
  // group peer. chat-groups-p2p.js verifies + decrypts + renders it.
  if (msg.type === 'p2p_group_obj') {
    if (typeof window.handleP2pGroupObj === 'function') await window.handleP2pGroupObj(msg, peerKey);
    return;
  }
  if (msg.type && msg.type.startsWith('sync_')) {
    await handleSyncFrame(msg, peerKey);
    return;
  }
  return _onDCMessageOrig.call(this, event, peerKey);
};

// hexToBuf and bufToHex are defined in crypto.js and available globally.
