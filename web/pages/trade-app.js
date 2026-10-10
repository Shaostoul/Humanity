/* trade-app.js, Peer-to-peer trading UI */

/* ── Relay socket: signed in as the person, or not at all ──
 * The relay binds a socket to a person only after proof of the key (Inc3b,
 * src/relay/relay.rs). The page sends `identify` with the person's Dilithium3
 * public key, the relay replies `identify_challenge {nonce}`, and the page signs
 *   "hum/identify/v1\n" + nonce + "\n" + public_key_hex
 * and returns `identify_response {sig_b64}`. Until that check passes the relay
 * drops every trade message on the socket without a reply, and closes the
 * socket after 30 seconds. Its `peer_list` is the sign the check passed.
 * The key comes from the identity Chat keeps in this browser (getPqIdentity in
 * /shared/pq-relay-auth.js, derived by /chat/pq.js), the same way the Tasks
 * page signs in (tasks-app.js). Nothing is sent, and the New trade button stays
 * hidden, until the socket is signed in; no key is ever made up.
 *
 * Who can reach me (step B, docs/design/blocking-and-safe-mode.md 10c): a
 * trade request reaches someone only if their Trades audience lets the sender
 * through, Friends by default, which the relay checks with the friendship pass
 * they gave the sender, carried as `friend_cert`. Chat keeps the passes it
 * holds in its encrypted local store (/chat/chat-dm-store.js); this page opens
 * that store read-only, with the key Chat derives from the same seed
 * (getPqDmStoreKey in /shared/pq-relay-auth.js), and sends the pass. When the
 * relay still says no (`reach_refused`, kind "trade"), the page says so in the
 * chat's own words (REACH_REFUSED_TRADE in /shared/reach.js). */
let tradeWs = null;
let tradeWsBound = false;     // true once the relay accepted the proof (its peer_list arrived)
let tradeWsRefusal = '';      // the relay's reason when it refused this page's sign-in
let tradeWsRetryMs = 5000;    // reconnect delay, doubles up to a minute while the link keeps dropping
let tradeWsRetryTimer = null;
let tradeIdentity = null;     // { dilithiumPublicHex, dilithiumSecret } once derived
let tradeMyKey = '';          // the signed-in key: empty until the relay has accepted the proof
let tradePendingTarget = '';  // who the last trade request went to, until the server answers
let trades = [];
let activeTrade = null; // currently viewed trade

function escHtml(s) { return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }

/** The exact bytes the relay verifies: format!("hum/identify/v1\n{}\n{}", nonce, public_key). */
function tradeIdentifyPreimage(nonce, publicKeyHex) {
  return new TextEncoder().encode('hum/identify/v1\n' + nonce + '\n' + publicKeyHex);
}

/** Standard base64 (what the relay's B64.decode expects), without spreading 3309 bytes into one call. */
function tradeBytesToB64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
  return btoa(s);
}

/** The identify_response message answering the relay's challenge, or null if signing failed. */
async function tradeIdentifyResponse(identity, nonce) {
  if (!identity || !identity.dilithiumSecret || !identity.dilithiumPublicHex || !nonce) return null;
  if (typeof window.pqSignMessage !== 'function') return null;
  const sig = await window.pqSignMessage(identity.dilithiumSecret, tradeIdentifyPreimage(nonce, identity.dilithiumPublicHex));
  if (!sig) return null;
  return { type: 'identify_response', sig_b64: tradeBytesToB64(sig) };
}

/** This browser's Dilithium3 identity, or null when Chat has not set one up here. */
async function loadTradeIdentity() {
  if (tradeIdentity) return tradeIdentity;
  if (typeof window.getPqIdentity !== 'function') return null;
  try { tradeIdentity = await window.getPqIdentity(); } catch (e) { tradeIdentity = null; }
  return tradeIdentity;
}

/** Is the socket signed in as this person right now? */
function tradeSignedIn() {
  return !!(tradeWs && tradeWsBound && tradeWs.readyState === WebSocket.OPEN && tradeMyKey);
}

/** Send one frame on the signed-in socket. Returns false, sending nothing, when it is not signed in. */
function tradeSend(frame) {
  if (!tradeSignedIn()) return false;
  tradeWs.send(JSON.stringify(frame));
  return true;
}

/** One line above the list saying where the page stands. `warn` colours it; `retry` adds Try again. */
function tradeSay(text, opts) {
  const el = document.getElementById('trade-status');
  if (!el) return;
  el.textContent = text || '';
  el.className = 'trade-status' + (opts && opts.warn ? ' trade-status-warn' : '');
  el.style.display = text ? '' : 'none';
  if (text && opts && opts.retry) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn-primary btn-sm trade-retry';
    btn.textContent = 'Try again';
    btn.onclick = tradeRetryNow;
    el.appendChild(document.createTextNode(' '));
    el.appendChild(btn);
  }
}

/** What to tell the person when the page cannot sign in. */
function tradeSignInHelp() {
  if (tradeWsRefusal) return 'The server did not accept this page\'s sign-in: ' + tradeWsRefusal;
  if (!localStorage.getItem('humanity_key_backup')) {
    return 'Not signed in on this page. Open Chat in this browser first (it keeps your identity here), then press Try again. A passphrase-protected identity cannot be read by this page yet.';
  }
  return 'Could not sign in to the server from this page. Check the connection and press Try again.';
}

/** Show what needs the signed-in socket only while it is signed in. */
function tradeShowSignedIn() {
  const on = tradeSignedIn();
  const btn = document.getElementById('trade-new-btn');
  if (btn) btn.style.display = on ? '' : 'none';
  const create = document.getElementById('ob-create-section');
  const book = document.getElementById('trade-section-orderbook');
  if (create) create.style.display = on && book && book.style.display !== 'none' ? '' : 'none';
}

/** The relay said no: remember why, stop retrying, and close (it would hold the socket open 30s). */
function refuseTradeWs(ws, reason) {
  tradeWsRefusal = reason || 'The server refused the sign-in.';
  console.warn('[trade] sign-in refused:', tradeWsRefusal);
  tradeSay(tradeSignInHelp(), { warn: true, retry: true });
  try { ws.close(); } catch (e) {}
}

/** Try again pressed: an earlier refusal gets one more try. */
function tradeRetryNow() {
  tradeWsRefusal = '';
  tradeWsRetryMs = 5000;
  clearTimeout(tradeWsRetryTimer);
  tradeStart();
}

/** Sign in, or say why this page cannot. */
async function tradeStart() {
  if (tradeWs) return;
  tradeSay('Signing in to the server...');
  const started = await ensureTradeWs();
  if (!started) {
    tradeSay(tradeSignInHelp(), { warn: true, retry: true });
    const list = document.getElementById('trade-list-container');
    if (list) list.innerHTML = '<p class="trade-empty">Your trades show here once this page is signed in.</p>';
  }
}

/** Open the socket and sign in. Resolves true if a socket is open or on its way, false if there is no identity. */
async function ensureTradeWs() {
  if (tradeWs) return true;
  const id = await loadTradeIdentity();
  if (!id) return false;
  if (tradeWs) return true; // another call opened it while the key was being derived
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(proto + '//' + location.host + '/ws');
  tradeWs = ws;
  tradeWsBound = false;
  ws.addEventListener('open', function() {
    // A placeholder name is never sent: the relay registers any real-looking
    // name to the key. The name is the one Chat saved in this browser; with
    // none, the relay signs the socket in under the name already registered.
    const name = (localStorage.getItem('humanity_name') || '').trim() || null;
    ws.send(JSON.stringify({ type: 'identify', public_key: id.dilithiumPublicHex, display_name: name }));
  });
  ws.addEventListener('message', function(e) {
    if (tradeWs !== ws) return;
    let m;
    try { m = JSON.parse(e.data); } catch (ex) { return; }
    // ── Sign-in phase: nothing else counts until the relay accepts the proof ──
    if (!tradeWsBound) {
      if (m.type === 'identify_challenge') {
        tradeIdentifyResponse(id, m.nonce).then(function(resp) {
          if (!resp) { refuseTradeWs(ws, 'this page could not sign the server\'s sign-in check with your identity.'); return; }
          if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(resp));
        });
      } else if (m.type === 'peer_list') {
        // The relay sends peer_list first thing after it binds the socket.
        tradeWsBound = true;
        tradeWsRefusal = '';
        tradeWsRetryMs = 5000;
        tradeMyKey = id.dilithiumPublicHex;
        tradeSay('');
        tradeShowSignedIn();
        if (!trades.length) {
          const list = document.getElementById('trade-list-container');
          if (list) list.innerHTML = '<p class="trade-empty">Loading your trades...</p>';
        }
        tradeSend({ type: 'trade_list_request' });
      } else if (m.type === 'account_erased') {
        // This identity's account was erased on this server (BUG-135). Coming
        // back is the person's choice, made on the Chat page.
        refuseTradeWs(ws, m.partial === true
          ? 'the erase of your account on this server did not finish, so open Chat, press Enter and use Erase account again.'
          : 'your account on this server was erased, so open Chat and press Enter there to sign up again.');
      } else if (m.type === 'name_taken' || m.type === 'system') {
        const text = String(m.message || '');
        // The per-connection throttle: the relay closes the socket, and the close handler backs off.
        if (m.type === 'system' && text.startsWith('Too many connection attempts')) return;
        refuseTradeWs(ws, text);
      }
      return;
    }
    handleTradeMessage(m);
  });
  ws.addEventListener('close', function() {
    if (tradeWs !== ws) return;
    tradeWs = null;
    tradeWsBound = false;
    tradeShowSignedIn();
    // After a refusal, retrying would only repeat it (and spend the relay's
    // per-connection allowance); Try again starts over instead.
    if (tradeWsRefusal) return;
    clearTimeout(tradeWsRetryTimer);
    tradeSay('Lost the connection to the server. Trying again in ' + Math.round(tradeWsRetryMs / 1000) + ' seconds.', { warn: true });
    tradeWsRetryTimer = setTimeout(tradeStart, tradeWsRetryMs);
    tradeWsRetryMs = Math.min(tradeWsRetryMs * 2, 60000);
  });
  return true;
}

// ── Message handling (signed-in socket only) ──

function handleTradeMessage(msg) {
  // "Who can reach me" refused the request: the chat's own sentence.
  if (msg.type === 'reach_refused') {
    if (msg.kind === 'trade') {
      tradePendingTarget = '';
      tradeSay(typeof REACH_REFUSED_TRADE === 'string' ? REACH_REFUSED_TRADE : 'This person did not accept your trade request.', { warn: true });
    }
    return;
  }
  if (msg.type === 'system' && msg.message) {
    // Trade data pushed from server
    if (msg.message.startsWith('__trade_data__:')) {
      try {
        var payload = JSON.parse(msg.message.slice('__trade_data__:'.length));
        if (payload.trade) {
          if (tradePendingTarget && payload.trade.initiator_key === tradeMyKey && payload.trade.recipient_key === tradePendingTarget) {
            tradePendingTarget = '';
            tradeSay('Trade request sent.');
          }
          upsertTrade(payload.trade);
        }
      } catch(e) {}
      return;
    }
    // Trade list response
    if (msg.message.startsWith('__trade_list__:')) {
      try {
        var payload = JSON.parse(msg.message.slice('__trade_list__:'.length));
        if (payload.trades) {
          trades = payload.trades;
          renderTradeList();
        }
      } catch(e) {}
      return;
    }
    // Trade complete notification
    if (msg.message.startsWith('__trade_complete__:')) {
      tradeSend({ type: 'trade_list_request' });
      return;
    }
    // The server's own words about a trade (not online, too many trades, the
    // daily limit for people who have not befriended you, a shortened note).
    if (!msg.message.startsWith('__') && /trade|items format/i.test(msg.message)) {
      tradePendingTarget = '';
      tradeSay(msg.message, { warn: true });
    }
  }
}

function upsertTrade(trade) {
  var idx = trades.findIndex(function(t) { return t.id === trade.id; });
  if (idx >= 0) {
    trades[idx] = trade;
  } else {
    trades.unshift(trade);
  }
  renderTradeList();
  // If we're viewing this trade, refresh detail
  if (activeTrade && activeTrade.id === trade.id) {
    activeTrade = trade;
    renderTradeDetail();
  }
}

// ── Rendering ──

function renderTradeList() {
  var container = document.getElementById('trade-list-container');
  if (trades.length === 0) {
    container.innerHTML = '<p class="trade-empty">No trades yet. Start one with the + New Trade button.</p>';
    return;
  }

  // Sort: active/pending first, then by created_at desc
  var sorted = trades.slice().sort(function(a, b) {
    var order = { active: 0, pending: 1, completed: 2, cancelled: 3 };
    var oa = order[a.status] !== undefined ? order[a.status] : 4;
    var ob = order[b.status] !== undefined ? order[b.status] : 4;
    if (oa !== ob) return oa - ob;
    return (b.created_at || 0) - (a.created_at || 0);
  });

  var html = '';
  sorted.forEach(function(t) {
    var isInitiator = t.initiator_key === tradeMyKey;
    var partnerKey = isInitiator ? t.recipient_key : t.initiator_key;
    var partnerLabel = partnerKey.slice(0, 12) + '...';
    var myItemCount = isInitiator ? (t.initiator_items || []).length : (t.recipient_items || []).length;
    var theirItemCount = isInitiator ? (t.recipient_items || []).length : (t.initiator_items || []).length;

    html += '<div class="trade-card" role="button" tabindex="0" onclick="viewTrade(\'' + escHtml(t.id) + '\')" onkeydown="if(event.key===\'Enter\'){viewTrade(\'' + escHtml(t.id) + '\')}">';
    html += '<div class="trade-card-head">';
    html += '<span class="trade-card-title">' + (isInitiator ? 'Trade with ' : 'Trade from ') + escHtml(partnerLabel) + '</span>';
    html += '<span class="status status-' + escHtml(t.status) + '">' + escHtml(t.status) + '</span>';
    html += '</div>';
    if (t.message) {
      html += '<div class="trade-card-msg">"' + escHtml(t.message) + '"</div>';
    }
    html += '<div class="trade-card-counts">Your items: ' + myItemCount + ' | Their items: ' + theirItemCount + '</div>';
    if (t.status === 'pending' && !isInitiator) {
      html += '<div class="trade-card-actions">';
      html += '<button class="btn-confirm btn-sm" onclick="event.stopPropagation();respondToTrade(\'' + escHtml(t.id) + '\',true)">Accept</button>';
      html += '<button class="btn-cancel btn-sm" onclick="event.stopPropagation();respondToTrade(\'' + escHtml(t.id) + '\',false)">Decline</button>';
      html += '</div>';
    }
    html += '</div>';
  });
  container.innerHTML = html;
}

function renderTradeDetail() {
  if (!activeTrade) return;
  var t = activeTrade;
  var isInitiator = t.initiator_key === tradeMyKey;
  var partnerKey = isInitiator ? t.recipient_key : t.initiator_key;

  // Header
  var headerEl = document.getElementById('trade-detail-header');
  headerEl.innerHTML = '<div class="trade-detail-head">' +
    '<h2 class="trade-detail-title">Trade with ' + escHtml(partnerKey.slice(0, 16)) + '...</h2>' +
    '<span class="status status-' + escHtml(t.status) + '">' + escHtml(t.status) + '</span>' +
    '</div>' +
    (t.message ? '<p class="trade-detail-msg">"' + escHtml(t.message) + '"</p>' : '');

  // If pending and we're recipient, show accept/decline
  if (t.status === 'pending' && !isInitiator) {
    headerEl.innerHTML += '<div class="trade-detail-actions">' +
      '<button class="btn-confirm" onclick="respondToTrade(\'' + escHtml(t.id) + '\',true)">Accept Trade</button>' +
      '<button class="btn-cancel" onclick="respondToTrade(\'' + escHtml(t.id) + '\',false)">Decline Trade</button>' +
      '</div>';
  }

  var viewEl = document.getElementById('trade-detail-view');

  if (t.status === 'pending' && isInitiator) {
    viewEl.innerHTML = '<p class="trade-empty">Waiting for the other player to accept your trade request...</p>' +
      '<div class="trade-actions"><button class="btn-cancel" onclick="cancelTrade(\'' + escHtml(t.id) + '\')">Cancel Trade</button></div>';
    return;
  }

  if (t.status === 'cancelled' || t.status === 'completed') {
    viewEl.innerHTML = renderTwoColumns(t, isInitiator, false);
    return;
  }

  // Active trade, full interactive view
  viewEl.innerHTML = renderTwoColumns(t, isInitiator, true);
}

function renderTwoColumns(t, isInitiator, editable) {
  var myItems = isInitiator ? (t.initiator_items || []) : (t.recipient_items || []);
  var theirItems = isInitiator ? (t.recipient_items || []) : (t.initiator_items || []);
  var myConfirmed = isInitiator ? t.initiator_confirmed : t.recipient_confirmed;
  var theirConfirmed = isInitiator ? t.recipient_confirmed : t.initiator_confirmed;

  var html = '<div class="trade-view">';

  // My side
  html += '<div class="trade-column">';
  html += '<h3>Your Items' + (myConfirmed ? '<span class="confirmed-badge">Confirmed</span>' : '') + '</h3>';
  if (myItems.length === 0) {
    html += '<p class="trade-col-empty">No items added yet.</p>';
  }
  myItems.forEach(function(item, idx) {
    html += '<div class="trade-item">';
    html += '<span class="item-name">' + escHtml(item.name) + '</span>';
    if (item.quantity > 1) html += '<span class="item-qty">x' + item.quantity + '</span>';
    if (item.description) html += '<span class="item-qty" title="' + escHtml(item.description) + '">(' + escHtml(item.item_type) + ')</span>';
    if (editable) html += '<button class="remove-btn" onclick="removeMyItem(' + idx + ')" title="Remove">&times;</button>';
    html += '</div>';
  });
  if (editable) {
    html += '<div class="add-item-form">';
    html += '<input type="text" id="add-item-name" aria-label="Item name" placeholder="Item name">';
    html += '<input type="number" id="add-item-qty" class="qty" aria-label="Quantity" placeholder="Qty" value="1" min="1" max="9999">';
    html += '<select id="add-item-type" class="item-type" aria-label="Item type"><option>goods</option><option>service</option><option>currency</option><option>digital</option><option>other</option></select>';
    html += '<button class="btn-primary" onclick="addMyItem()">Add</button>';
    html += '</div>';
  }
  html += '</div>';

  // Divider
  html += '<div class="trade-divider"><span>&harr;</span></div>';

  // Their side
  html += '<div class="trade-column">';
  html += '<h3>Their Items' + (theirConfirmed ? '<span class="confirmed-badge">Confirmed</span>' : '') + '</h3>';
  if (theirItems.length === 0) {
    html += '<p class="trade-col-empty">No items added yet.</p>';
  }
  theirItems.forEach(function(item) {
    html += '<div class="trade-item">';
    html += '<span class="item-name">' + escHtml(item.name) + '</span>';
    if (item.quantity > 1) html += '<span class="item-qty">x' + item.quantity + '</span>';
    if (item.description) html += '<span class="item-qty" title="' + escHtml(item.description) + '">(' + escHtml(item.item_type) + ')</span>';
    html += '</div>';
  });
  html += '</div>';

  html += '</div>'; // close trade-view

  // Actions
  if (editable) {
    html += '<div class="trade-actions">';
    if (!myConfirmed) {
      html += '<button class="btn-confirm" onclick="confirmTrade(\'' + escHtml(t.id) + '\')">Confirm Trade</button>';
    } else {
      html += '<button class="btn-confirm" disabled>Waiting for partner...</button>';
    }
    html += '<button class="btn-cancel" onclick="cancelTrade(\'' + escHtml(t.id) + '\')">Cancel Trade</button>';
    html += '</div>';
  }

  return html;
}

// ── Actions ──

function showTradeSection(section) {
  document.getElementById('trade-section-list').style.display = section === 'list' ? '' : 'none';
  document.getElementById('trade-section-detail').style.display = section === 'detail' ? '' : 'none';
  document.getElementById('trade-section-orderbook').style.display = section === 'orderbook' ? '' : 'none';
  // Update tab active state
  document.querySelectorAll('.trade-tab').forEach(function(t) { t.classList.remove('active'); });
  if (section === 'list') {
    document.getElementById('trade-nav-list').classList.add('active');
    activeTrade = null;
  } else if (section === 'orderbook') {
    document.getElementById('trade-nav-orderbook').classList.add('active');
    // Selling and history need the signed-in identity.
    tradeShowSignedIn();
    if (tradeSignedIn()) loadTradeHistory();
  }
}

function viewTrade(tradeId) {
  var t = trades.find(function(tr) { return tr.id === tradeId; });
  if (!t) return;
  activeTrade = t;
  showTradeSection('detail');
  renderTradeDetail();
}

function openTradeRequestModal() {
  if (!tradeSignedIn()) return;
  document.getElementById('trade-request-modal').style.display = 'flex';
  document.getElementById('trade-target-input').value = '';
  document.getElementById('trade-message-input').value = '';
  tradeRequestSay('');
  document.getElementById('trade-target-input').focus();
}

function closeTradeRequestModal() {
  document.getElementById('trade-request-modal').style.display = 'none';
}

/** A line inside the New trade form (a missing partner, a name not found). */
function tradeRequestSay(text) {
  const el = document.getElementById('trade-request-msg');
  if (!el) return;
  el.textContent = text || '';
  el.style.display = text ? '' : 'none';
}

// A registered name: what the relay allows (REACH_NAME_RE in /shared/reach.js).
const TRADE_NAME_RE = /^[A-Za-z0-9_-]{1,24}$/;
// A public key as the relay holds it: lowercase hex (a Dilithium3 key is 3904 characters).
const TRADE_KEY_RE = /^[0-9a-f]{64,}$/;
const TRADE_KEY_HINT = 'In Chat, open their profile and click the key to copy it.';

/**
 * The key of the person a trade request is for: {key}, or {error} saying why
 * not. A key is taken as given (in lowercase); a name is looked up in this
 * server's member list, and only a single exact match counts. The relay itself
 * takes keys only, so a name is never sent as one.
 */
async function tradeResolveTarget(raw) {
  const text = String(raw || '').trim();
  if (!text) return { error: 'Enter their name or public key.' };
  if (TRADE_KEY_RE.test(text.toLowerCase())) return { key: text.toLowerCase() };
  if (!TRADE_NAME_RE.test(text)) return { error: 'That is neither a name nor a public key. ' + TRADE_KEY_HINT };
  let data = null;
  try {
    const r = await fetch('/api/members?search=' + encodeURIComponent(text) + '&limit=50');
    if (!r.ok) throw new Error('HTTP ' + r.status);
    data = await r.json();
  } catch (e) {
    return { error: 'Could not look that name up. Check the connection, or paste their public key instead.' };
  }
  const want = text.toLowerCase();
  const keys = [];
  ((data && data.members) || []).forEach(function(m) {
    if (!m || typeof m.name !== 'string' || typeof m.public_key !== 'string') return;
    const key = m.public_key.toLowerCase();
    if (m.name.toLowerCase() === want && keys.indexOf(key) < 0) keys.push(key);
  });
  if (keys.length === 1) return { key: keys[0] };
  if (keys.length === 0) return { error: 'No one named "' + text + '" is listed on this server. Paste their public key instead. ' + TRADE_KEY_HINT };
  return { error: 'More than one person is listed as "' + text + '". Paste their public key instead. ' + TRADE_KEY_HINT };
}

/**
 * The friendship pass `peer` gave this person, read from Chat's store in this
 * browser, or null. Read afresh for each request, so a pass Chat received after
 * this page opened counts. Read-only: this page never writes Chat's store.
 */
async function tradePassFor(peer) {
  if (!tradeIdentity || !window.hosDmStore || typeof window.getPqDmStoreKey !== 'function') return null;
  // chat-dm-store.js asks window.getDmStoreKey for its key. Chat's crypto.js
  // defines it; this page does not load crypto.js and hands it the same key,
  // derived from the same seed.
  if (typeof window.getDmStoreKey !== 'function') window.getDmStoreKey = window.getPqDmStoreKey;
  try {
    const ok = await window.hosDmStore.init(tradeIdentity.dilithiumPublicHex, location.host, { readOnly: true });
    return ok ? window.hosDmStore.certFor(peer) : null;
  } catch (e) {
    return null;
  }
}

async function sendTradeRequest() {
  const sendBtn = document.getElementById('trade-send-btn');
  if (!tradeSignedIn()) { tradeRequestSay('This page is not signed in to the server right now.'); return; }
  const message = document.getElementById('trade-message-input').value.trim();
  if (sendBtn) sendBtn.disabled = true;
  try {
    const target = await tradeResolveTarget(document.getElementById('trade-target-input').value);
    if (target.error) { tradeRequestSay(target.error); return; }
    if (target.key === tradeMyKey) { tradeRequestSay('That is your own key.'); return; }
    const frame = { type: 'trade_request', target_key: target.key, message: message };
    const pass = await tradePassFor(target.key);
    if (pass) frame.friend_cert = pass;
    if (!tradeSend(frame)) { tradeRequestSay('This page is not signed in to the server right now.'); return; }
    tradePendingTarget = target.key;
    closeTradeRequestModal();
    tradeSay('Sending the trade request...');
  } finally {
    if (sendBtn) sendBtn.disabled = false;
  }
}

function respondToTrade(tradeId, accepted) {
  tradeSend({
    type: 'trade_response',
    trade_id: tradeId,
    accepted: accepted
  });
}

function addMyItem() {
  if (!activeTrade || activeTrade.status !== 'active') return;
  var nameEl = document.getElementById('add-item-name');
  var qtyEl = document.getElementById('add-item-qty');
  var typeEl = document.getElementById('add-item-type');
  var name = (nameEl.value || '').trim();
  if (!name) { nameEl.focus(); return; }
  var qty = parseInt(qtyEl.value) || 1;
  var itemType = typeEl.value || 'goods';

  var isInitiator = activeTrade.initiator_key === tradeMyKey;
  var myItems = isInitiator ? (activeTrade.initiator_items || []).slice() : (activeTrade.recipient_items || []).slice();
  myItems.push({ item_type: itemType, name: name, quantity: qty, description: '' });

  if (!tradeSend({
    type: 'trade_update_items',
    trade_id: activeTrade.id,
    items: myItems
  })) return;
  nameEl.value = '';
  qtyEl.value = '1';
}

function removeMyItem(idx) {
  if (!activeTrade || activeTrade.status !== 'active') return;
  var isInitiator = activeTrade.initiator_key === tradeMyKey;
  var myItems = isInitiator ? (activeTrade.initiator_items || []).slice() : (activeTrade.recipient_items || []).slice();
  myItems.splice(idx, 1);
  tradeSend({
    type: 'trade_update_items',
    trade_id: activeTrade.id,
    items: myItems
  });
}

function confirmTrade(tradeId) {
  tradeSend({
    type: 'trade_confirm',
    trade_id: tradeId
  });
}

async function cancelTrade(tradeId) {
  if (!await holdConfirm('Cancel this trade?', { seconds: 3 })) return;
  tradeSend({
    type: 'trade_cancel',
    trade_id: tradeId
  });
}

// ── Order Book functions ──

/**
 * Sign an order book request the way the relay checks it
 * (verify_dilithium_signature in src/relay/handlers/broadcast.rs): Dilithium3
 * over `content + "\n" + timestamp`, the signature in hex. Null when this page
 * is not signed in or signing failed.
 */
async function tradeRestAuth(content) {
  if (!tradeSignedIn() || !tradeIdentity || typeof window.pqSignMessage !== 'function') return null;
  const timestamp = Date.now();
  const sig = await window.pqSignMessage(tradeIdentity.dilithiumSecret, new TextEncoder().encode(content + '\n' + timestamp));
  if (!sig) return null;
  return { key: tradeIdentity.dilithiumPublicHex, timestamp: timestamp, signature: bytesToHex(sig) };
}

const TRADE_NOT_SIGNED_IN = 'This page is not signed in to the server. Open Chat in this browser first.';

function searchOrderBook() {
  var itemType = (document.getElementById('ob-search-item').value || '').trim();
  if (!itemType) { document.getElementById('ob-search-item').focus(); return; }
  fetch('/api/trade/orders?item_type=' + encodeURIComponent(itemType))
    .then(function(r) { return r.json(); })
    .then(function(data) {
      renderOrderBook(data.orders || [], data.item_type, data.market_price);
    })
    .catch(function() {
      document.getElementById('ob-orders-container').innerHTML = '<p class="ob-error">Failed to load orders.</p>';
    });
}

function renderOrderBook(orders, itemType, marketPrice) {
  var priceEl = document.getElementById('ob-market-price');
  if (marketPrice != null) {
    priceEl.textContent = 'Last trade price for ' + itemType + ': ' + marketPrice.toFixed(2);
    priceEl.style.display = '';
  } else {
    priceEl.style.display = 'none';
  }

  var container = document.getElementById('ob-orders-container');
  if (orders.length === 0) {
    container.innerHTML = '<p class="ob-empty">No open orders for "' + escHtml(itemType) + '".</p>';
    return;
  }

  var html = '<table class="ob-table"><thead><tr>';
  html += '<th>Seller</th><th>Item</th><th>Qty</th><th>Price/Unit</th><th>Currency</th><th>Total</th><th></th>';
  html += '</tr></thead><tbody>';
  orders.forEach(function(o) {
    var sellerLabel = o.seller_key.slice(0, 12) + '...';
    var isMine = !!tradeMyKey && o.seller_key === tradeMyKey;
    html += '<tr>';
    html += '<td>' + escHtml(sellerLabel) + (isMine ? ' <em>(you)</em>' : '') + '</td>';
    html += '<td>' + escHtml(o.item_type) + (o.item_id ? ' (' + escHtml(o.item_id) + ')' : '') + '</td>';
    html += '<td>' + o.remaining_qty + '/' + o.quantity + '</td>';
    html += '<td>' + o.price_per_unit.toFixed(2) + '</td>';
    html += '<td>' + escHtml(o.currency) + '</td>';
    html += '<td>' + (o.remaining_qty * o.price_per_unit).toFixed(2) + '</td>';
    html += '<td>';
    if (isMine) {
      html += '<button class="cancel-order-btn" onclick="cancelOrder(' + o.id + ')">Cancel</button>';
    } else if (tradeSignedIn()) {
      html += '<button class="buy-btn" onclick="promptBuyOrder(' + o.id + ',' + o.remaining_qty + ',' + o.price_per_unit + ')">Buy</button>';
    }
    html += '</td>';
    html += '</tr>';
  });
  html += '</tbody></table>';
  container.innerHTML = html;
}

function promptBuyOrder(orderId, maxQty, pricePerUnit) {
  var qty = prompt('How many to buy? (max ' + maxQty + ', price ' + pricePerUnit.toFixed(2) + ' each)', maxQty);
  if (qty === null) return;
  qty = parseInt(qty);
  if (!qty || qty <= 0 || qty > maxQty) { alert('Invalid quantity.'); return; }
  fillOrder(orderId, qty);
}

async function fillOrder(orderId, quantity) {
  if (!tradeSignedIn()) { alert(TRADE_NOT_SIGNED_IN); return; }
  var auth = await tradeRestAuth('fill_order\n' + orderId + '\n' + quantity);
  if (!auth) { alert('Could not sign the request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders/' + orderId + '/fill', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ public_key: auth.key, timestamp: auth.timestamp, signature: auth.signature, quantity: quantity })
    });
    var data = await resp.json();
    if (resp.ok) {
      alert('Purchase complete!');
      searchOrderBook();
      loadTradeHistory();
    } else {
      alert('Error: ' + (data.message || JSON.stringify(data)));
    }
  } catch(e) { alert('Network error.'); }
}

async function createSellOrder() {
  if (!tradeSignedIn()) { alert(TRADE_NOT_SIGNED_IN); return; }
  var itemType = (document.getElementById('ob-create-item').value || '').trim();
  var qty = parseInt(document.getElementById('ob-create-qty').value) || 0;
  var price = parseFloat(document.getElementById('ob-create-price').value) || 0;
  var currency = document.getElementById('ob-create-currency').value || 'credits';

  if (!itemType) { document.getElementById('ob-create-item').focus(); return; }
  if (qty <= 0) { alert('Quantity must be positive.'); return; }
  if (price <= 0) { alert('Price must be positive.'); return; }

  // The relay signs over format!("trade_order\n{}\n{}\n{}", item_type, quantity, price_per_unit).
  var auth = await tradeRestAuth('trade_order\n' + itemType + '\n' + qty + '\n' + price);
  if (!auth) { alert('Could not sign the request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        public_key: auth.key, timestamp: auth.timestamp, signature: auth.signature,
        item_type: itemType, quantity: qty, price_per_unit: price, currency: currency
      })
    });
    var data = await resp.json();
    if (resp.ok) {
      alert('Sell order posted!');
      document.getElementById('ob-search-item').value = itemType;
      searchOrderBook();
    } else {
      alert('Error: ' + (data.message || JSON.stringify(data)));
    }
  } catch(e) { alert('Network error.'); }
}

async function cancelOrder(orderId) {
  if (!await holdConfirm('Cancel this sell order?', { seconds: 3 })) return;
  if (!tradeSignedIn()) { alert(TRADE_NOT_SIGNED_IN); return; }
  var auth = await tradeRestAuth('cancel_order\n' + orderId);
  if (!auth) { alert('Could not sign the request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders/' + orderId + '?key=' + encodeURIComponent(auth.key) +
      '&timestamp=' + auth.timestamp + '&sig=' + encodeURIComponent(auth.signature), { method: 'DELETE' });
    var data = await resp.json();
    if (resp.ok) {
      searchOrderBook();
    } else {
      alert('Error: ' + (data.message || JSON.stringify(data)));
    }
  } catch(e) { alert('Network error.'); }
}

function loadTradeHistory() {
  if (!tradeSignedIn()) return;
  fetch('/api/trade/history?key=' + encodeURIComponent(tradeMyKey) + '&limit=20')
    .then(function(r) { return r.json(); })
    .then(function(history) { renderTradeHistory(history); })
    .catch(function() {});
}

function renderTradeHistory(history) {
  var container = document.getElementById('ob-history-container');
  if (!history || history.length === 0) {
    container.innerHTML = '<p class="ob-history-empty">No trade history yet.</p>';
    return;
  }
  var html = '<table class="ob-table"><thead><tr>';
  html += '<th>Date</th><th>Item</th><th>Qty</th><th>Price/Unit</th><th>Total</th><th>Role</th>';
  html += '</tr></thead><tbody>';
  history.forEach(function(h) {
    var date = new Date(h.timestamp).toLocaleDateString();
    var role = h.buyer_key === tradeMyKey ? 'Bought' : 'Sold';
    html += '<tr>';
    html += '<td>' + escHtml(date) + '</td>';
    html += '<td>' + escHtml(h.item_type) + '</td>';
    html += '<td>' + h.quantity + '</td>';
    html += '<td>' + h.price_per_unit.toFixed(2) + '</td>';
    html += '<td>' + h.total_price.toFixed(2) + '</td>';
    html += '<td>' + role + '</td>';
    html += '</tr>';
  });
  html += '</tbody></table>';
  container.innerHTML = html;
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
}

// Click outside modal to close
document.getElementById('trade-request-modal').addEventListener('click', function(e) {
  if (e.target === this) closeTradeRequestModal();
});

// Start
tradeStart();
