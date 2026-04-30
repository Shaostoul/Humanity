/* trade-app.js — Peer-to-peer trading UI */

let tradeWs = null;
let tradeMyKey = '';
let tradeMyName = 'Visitor';
let trades = [];
let activeTrade = null; // currently viewed trade

function escHtml(s) { return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }

// ── WebSocket connection ──

function tradeConnect() {
  var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  tradeWs = new WebSocket(proto + '//' + location.host + '/ws');
  tradeWs.onopen = function() {
    var storedKey = localStorage.getItem('humanity_key');
    if (!storedKey) {
      try {
        var backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
        if (backup && backup.publicKeyHex) storedKey = backup.publicKeyHex;
      } catch(e) {}
    }
    var storedName = localStorage.getItem('humanity_name');
    if (storedKey) {
      tradeMyKey = storedKey;
      tradeMyName = storedName || 'Anonymous';
      tradeWs.send(JSON.stringify({ type: 'identify', public_key: storedKey, display_name: storedName || null }));
    } else {
      tradeMyKey = 'viewer_' + Math.random().toString(36).slice(2, 10);
      tradeWs.send(JSON.stringify({ type: 'identify', public_key: tradeMyKey, display_name: null }));
    }
    // Show the new-trade button once connected with a real key
    if (storedKey) {
      document.getElementById('trade-new-btn').style.display = '';
    }
    // Request trade list
    setTimeout(function() {
      tradeWs.send(JSON.stringify({ type: 'trade_list_request' }));
    }, 500);
  };
  window._humanityWs = tradeWs;
  tradeWs.onmessage = function(e) {
    try {
      var msg = JSON.parse(e.data);
      handleTradeMessage(msg);
    } catch(ex) {}
  };
  tradeWs.onclose = function() { setTimeout(tradeConnect, 3000); };
  tradeWs.onerror = function() {};
}

// ── Message handling ──

function handleTradeMessage(msg) {
  if (msg.type === 'system' && msg.message) {
    // Trade data pushed from server
    if (msg.message.startsWith('__trade_data__:')) {
      try {
        var payload = JSON.parse(msg.message.slice('__trade_data__:'.length));
        if (payload.trade) {
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
      try {
        var payload = JSON.parse(msg.message.slice('__trade_complete__:'.length));
        // Refresh trade data
        if (tradeWs && tradeWs.readyState === 1) {
          tradeWs.send(JSON.stringify({ type: 'trade_list_request' }));
        }
      } catch(e) {}
      return;
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
    container.innerHTML = '<p style="color:var(--text-muted);text-align:center;padding:var(--space-2xl);">No trades yet. Start one with the + New Trade button.</p>';
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

    html += '<div class="trade-card" onclick="viewTrade(\'' + escHtml(t.id) + '\')">';
    html += '<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:var(--space-md);">';
    html += '<span style="font-size:.85rem;font-weight:600;">' + (isInitiator ? 'Trade with ' : 'Trade from ') + escHtml(partnerLabel) + '</span>';
    html += '<span class="status status-' + escHtml(t.status) + '">' + escHtml(t.status) + '</span>';
    html += '</div>';
    if (t.message) {
      html += '<div style="font-size:.8rem;color:var(--text-muted);margin-bottom:var(--space-sm);">"' + escHtml(t.message) + '"</div>';
    }
    html += '<div style="font-size:.75rem;color:var(--text-muted);">Your items: ' + myItemCount + ' | Their items: ' + theirItemCount + '</div>';
    if (t.status === 'pending' && !isInitiator) {
      html += '<div style="margin-top:var(--space-md);display:flex;gap:var(--space-md);">';
      html += '<button class="btn-confirm" onclick="event.stopPropagation();respondToTrade(\'' + escHtml(t.id) + '\',true)" style="font-size:.75rem;padding:var(--space-sm) var(--space-lg);">Accept</button>';
      html += '<button class="btn-cancel" onclick="event.stopPropagation();respondToTrade(\'' + escHtml(t.id) + '\',false)" style="font-size:.75rem;padding:var(--space-sm) var(--space-lg);">Decline</button>';
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
  headerEl.innerHTML = '<div style="display:flex;justify-content:space-between;align-items:center;">' +
    '<h2 style="margin:0;font-size:1.1rem;">Trade with ' + escHtml(partnerKey.slice(0, 16)) + '...</h2>' +
    '<span class="status status-' + escHtml(t.status) + '">' + escHtml(t.status) + '</span>' +
    '</div>' +
    (t.message ? '<p style="color:var(--text-muted);font-size:.8rem;margin:var(--space-sm) 0 0 0;">"' + escHtml(t.message) + '"</p>' : '');

  // If pending and we're recipient, show accept/decline
  if (t.status === 'pending' && !isInitiator) {
    headerEl.innerHTML += '<div style="margin-top:var(--space-lg);display:flex;gap:var(--space-md);">' +
      '<button class="btn-confirm" onclick="respondToTrade(\'' + escHtml(t.id) + '\',true)">Accept Trade</button>' +
      '<button class="btn-cancel" onclick="respondToTrade(\'' + escHtml(t.id) + '\',false)">Decline Trade</button>' +
      '</div>';
  }

  var viewEl = document.getElementById('trade-detail-view');

  if (t.status === 'pending' && isInitiator) {
    viewEl.innerHTML = '<p style="color:var(--text-muted);text-align:center;padding:var(--space-2xl);">Waiting for the other player to accept your trade request...</p>' +
      '<div class="trade-actions"><button class="btn-cancel" onclick="cancelTrade(\'' + escHtml(t.id) + '\')">Cancel Trade</button></div>';
    return;
  }

  if (t.status === 'cancelled' || t.status === 'completed') {
    viewEl.innerHTML = renderTwoColumns(t, isInitiator, false);
    return;
  }

  // Active trade — full interactive view
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
    html += '<p style="color:var(--text-muted);font-size:.8rem;">No items added yet.</p>';
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
    html += '<input type="text" id="add-item-name" placeholder="Item name" style="font-size:.8rem;">';
    html += '<input type="number" id="add-item-qty" placeholder="Qty" value="1" min="1" max="9999" style="max-width:60px;font-size:.8rem;">';
    html += '<select id="add-item-type" style="font-size:.8rem;max-width:100px;"><option>goods</option><option>service</option><option>currency</option><option>digital</option><option>other</option></select>';
    html += '<button class="btn-primary" onclick="addMyItem()" style="font-size:.75rem;padding:var(--space-sm) var(--space-md);">Add</button>';
    html += '</div>';
  }
  html += '</div>';

  // Divider
  html += '<div class="trade-divider"><span>&harr;</span></div>';

  // Their side
  html += '<div class="trade-column">';
  html += '<h3>Their Items' + (theirConfirmed ? '<span class="confirmed-badge">Confirmed</span>' : '') + '</h3>';
  if (theirItems.length === 0) {
    html += '<p style="color:var(--text-muted);font-size:.8rem;">No items added yet.</p>';
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
    var bothConfirmed = myConfirmed && theirConfirmed;
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
    // Show create section if connected
    if (tradeMyKey && !tradeMyKey.startsWith('viewer_')) {
      document.getElementById('ob-create-section').style.display = '';
      loadTradeHistory();
    }
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
  document.getElementById('trade-request-modal').style.display = 'flex';
  document.getElementById('trade-target-input').value = '';
  document.getElementById('trade-message-input').value = '';
  document.getElementById('trade-target-input').focus();
}

function closeTradeRequestModal() {
  document.getElementById('trade-request-modal').style.display = 'none';
}

function sendTradeRequest() {
  var target = document.getElementById('trade-target-input').value.trim();
  var message = document.getElementById('trade-message-input').value.trim();
  if (!target) { alert('Please enter a trade partner key or name.'); return; }
  if (!tradeWs || tradeWs.readyState !== 1) { alert('Not connected.'); return; }
  tradeWs.send(JSON.stringify({
    type: 'trade_request',
    target_key: target,
    message: message
  }));
  closeTradeRequestModal();
}

function respondToTrade(tradeId, accepted) {
  if (!tradeWs || tradeWs.readyState !== 1) return;
  tradeWs.send(JSON.stringify({
    type: 'trade_response',
    trade_id: tradeId,
    accepted: accepted
  }));
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

  tradeWs.send(JSON.stringify({
    type: 'trade_update_items',
    trade_id: activeTrade.id,
    items: myItems
  }));
  nameEl.value = '';
  qtyEl.value = '1';
}

function removeMyItem(idx) {
  if (!activeTrade || activeTrade.status !== 'active') return;
  var isInitiator = activeTrade.initiator_key === tradeMyKey;
  var myItems = isInitiator ? (activeTrade.initiator_items || []).slice() : (activeTrade.recipient_items || []).slice();
  myItems.splice(idx, 1);
  tradeWs.send(JSON.stringify({
    type: 'trade_update_items',
    trade_id: activeTrade.id,
    items: myItems
  }));
}

function confirmTrade(tradeId) {
  if (!tradeWs || tradeWs.readyState !== 1) return;
  tradeWs.send(JSON.stringify({
    type: 'trade_confirm',
    trade_id: tradeId
  }));
}

function cancelTrade(tradeId) {
  if (!confirm('Cancel this trade?')) return;
  if (!tradeWs || tradeWs.readyState !== 1) return;
  tradeWs.send(JSON.stringify({
    type: 'trade_cancel',
    trade_id: tradeId
  }));
}

// ── Order Book functions ──

function searchOrderBook() {
  var itemType = (document.getElementById('ob-search-item').value || '').trim();
  if (!itemType) { document.getElementById('ob-search-item').focus(); return; }
  fetch('/api/trade/orders?item_type=' + encodeURIComponent(itemType))
    .then(function(r) { return r.json(); })
    .then(function(data) {
      renderOrderBook(data.orders || [], data.item_type, data.market_price);
    })
    .catch(function() {
      document.getElementById('ob-orders-container').innerHTML = '<p style="color:var(--danger,#f44);text-align:center;">Failed to load orders.</p>';
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
    container.innerHTML = '<p style="color:var(--text-muted);text-align:center;padding:var(--space-xl);">No open orders for "' + escHtml(itemType) + '".</p>';
    return;
  }

  var html = '<table class="ob-table"><thead><tr>';
  html += '<th>Seller</th><th>Item</th><th>Qty</th><th>Price/Unit</th><th>Currency</th><th>Total</th><th></th>';
  html += '</tr></thead><tbody>';
  orders.forEach(function(o) {
    var sellerLabel = o.seller_key.slice(0, 12) + '...';
    var isMine = o.seller_key === tradeMyKey;
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
    } else if (tradeMyKey && !tradeMyKey.startsWith('viewer_')) {
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
  if (!tradeMyKey || tradeMyKey.startsWith('viewer_')) { alert('Not authenticated.'); return; }
  var timestamp = Date.now();
  var sigContent = 'fill_order\n' + orderId + '\n' + quantity + '\n' + timestamp;
  var signature = '';
  try {
    var backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
    if (backup && backup.privateKeyHex) {
      var privBytes = hexToBytes(backup.privateKeyHex);
      var keyPair = nacl.sign.keyPair.fromSeed(privBytes.slice(0, 32));
      var msgBytes = new TextEncoder().encode(sigContent);
      signature = bytesToHex(nacl.sign.detached(msgBytes, keyPair.secretKey));
    }
  } catch(e) { alert('Could not sign request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders/' + orderId + '/fill', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ public_key: tradeMyKey, timestamp: timestamp, signature: signature, quantity: quantity })
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
  if (!tradeMyKey || tradeMyKey.startsWith('viewer_')) { alert('Not authenticated.'); return; }
  var itemType = (document.getElementById('ob-create-item').value || '').trim();
  var qty = parseInt(document.getElementById('ob-create-qty').value) || 0;
  var price = parseFloat(document.getElementById('ob-create-price').value) || 0;
  var currency = document.getElementById('ob-create-currency').value || 'credits';

  if (!itemType) { document.getElementById('ob-create-item').focus(); return; }
  if (qty <= 0) { alert('Quantity must be positive.'); return; }
  if (price <= 0) { alert('Price must be positive.'); return; }

  var timestamp = Date.now();
  var sigContent = 'trade_order\n' + itemType + '\n' + qty + '\n' + price + '\n' + timestamp;
  var signature = '';
  try {
    var backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
    if (backup && backup.privateKeyHex) {
      var privBytes = hexToBytes(backup.privateKeyHex);
      var keyPair = nacl.sign.keyPair.fromSeed(privBytes.slice(0, 32));
      var msgBytes = new TextEncoder().encode(sigContent);
      signature = bytesToHex(nacl.sign.detached(msgBytes, keyPair.secretKey));
    }
  } catch(e) { alert('Could not sign request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        public_key: tradeMyKey, timestamp: timestamp, signature: signature,
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
  if (!confirm('Cancel this sell order?')) return;
  if (!tradeMyKey || tradeMyKey.startsWith('viewer_')) { alert('Not authenticated.'); return; }
  var timestamp = Date.now();
  var sigContent = 'cancel_order\n' + orderId + '\n' + timestamp;
  var signature = '';
  try {
    var backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
    if (backup && backup.privateKeyHex) {
      var privBytes = hexToBytes(backup.privateKeyHex);
      var keyPair = nacl.sign.keyPair.fromSeed(privBytes.slice(0, 32));
      var msgBytes = new TextEncoder().encode(sigContent);
      signature = bytesToHex(nacl.sign.detached(msgBytes, keyPair.secretKey));
    }
  } catch(e) { alert('Could not sign request.'); return; }

  try {
    var resp = await fetch('/api/trade/orders/' + orderId + '?key=' + encodeURIComponent(tradeMyKey) +
      '&timestamp=' + timestamp + '&sig=' + encodeURIComponent(signature), { method: 'DELETE' });
    var data = await resp.json();
    if (resp.ok) {
      searchOrderBook();
    } else {
      alert('Error: ' + (data.message || JSON.stringify(data)));
    }
  } catch(e) { alert('Network error.'); }
}

function loadTradeHistory() {
  if (!tradeMyKey || tradeMyKey.startsWith('viewer_')) return;
  fetch('/api/trade/history?key=' + encodeURIComponent(tradeMyKey) + '&limit=20')
    .then(function(r) { return r.json(); })
    .then(function(history) { renderTradeHistory(history); })
    .catch(function() {});
}

function renderTradeHistory(history) {
  var container = document.getElementById('ob-history-container');
  if (!history || history.length === 0) {
    container.innerHTML = '<p style="color:var(--text-muted);font-size:.85rem;">No trade history yet.</p>';
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

// Hex utilities for signing
function hexToBytes(hex) {
  var bytes = new Uint8Array(hex.length / 2);
  for (var i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
  return bytes;
}
function bytesToHex(bytes) {
  return Array.from(bytes).map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
}

// Click outside modal to close
document.getElementById('trade-request-modal').addEventListener('click', function(e) {
  if (e.target === this) closeTradeRequestModal();
});

// Start
tradeConnect();
