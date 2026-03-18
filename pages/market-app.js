const API_BASE = '';
let marketWs = null;
let marketListings = [];
let marketMyKey = '';
let marketMyName = 'Visitor';
let marketMyRole = '';
const CATEGORY_COLORS = {
  Electronics:'#4488ff', Vehicles:'#f80', Clothing:'#f48', Tools:'#8b4',
  Furniture:'#a67', Home:'#68a', 'Books/Media':'#a88', Gaming:'#84f',
  Sports:'#4b8', Crafts:'#fa8', 'Food/Garden':'#4a8', Services:'#88f',
  '3D Models':'#f84', Other:'#888'
};
const STORE_DIRECTORY = [];

function escHtml(s) { return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }

function showMarketSection(section) {
  ['marketplace','stores','mylistings'].forEach(s => {
    document.getElementById('market-section-' + s).style.display = s === section ? '' : 'none';
    const btn = document.getElementById('market-nav-' + s);
    if (btn) { btn.classList.toggle('btn-clickable', s === section); }
  });
  if (section === 'marketplace') renderMarketListings();
  if (section === 'stores') renderStoreDirectory();
  if (section === 'mylistings') renderMyListings();
}

function handleMarketMessage(msg) {
  // Relay uses listing_list (response to listing_browse), listing_new, listing_updated, listing_deleted
  if (msg.type === 'listing_list') {
    marketListings = msg.listings || [];
    renderMarketListings();
    renderMyListings();
  } else if (msg.type === 'listing_new') {
    if (msg.listing) {
      marketListings = marketListings.filter(l => l.id !== msg.listing.id);
      marketListings.unshift(msg.listing);
      renderMarketListings();
      renderMyListings();
    }
  } else if (msg.type === 'listing_updated') {
    if (msg.listing) {
      const idx = marketListings.findIndex(l => l.id === msg.listing.id);
      if (idx >= 0) marketListings[idx] = msg.listing;
      else marketListings.unshift(msg.listing);
      renderMarketListings();
      renderMyListings();
    }
  } else if (msg.type === 'listing_deleted') {
    if (msg.id) marketListings = marketListings.filter(l => l.id !== msg.id);
    renderMarketListings();
    renderMyListings();
  } else if (msg.type === 'peer_list') {
    if (msg.peers && marketMyKey) {
      const me = msg.peers.find(p => p.public_key_hex === marketMyKey || p.public_key === marketMyKey);
      if (me) { marketMyRole = me.role || ''; }
    }
    const canList = marketMyRole === 'admin' || marketMyRole === 'mod' || marketMyRole === 'verified' || marketMyRole === 'donor';
    const btn = document.getElementById('market-create-btn');
    if (btn) btn.style.display = canList ? 'inline-flex' : 'none';
    // Request full listing catalog
    if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_browse' }));
  }
}

function openListingModal(editId) {
  document.getElementById('listing-edit-id').value = editId || '';
  document.getElementById('listing-modal-title').textContent = editId ? 'Edit Listing' : 'Create Listing';
  if (editId) {
    const l = marketListings.find(x => x.id === editId);
    if (l) {
      document.getElementById('listing-title').value = l.title || '';
      document.getElementById('listing-description').value = l.description || '';
      document.getElementById('listing-category').value = l.category || 'Other';
      document.getElementById('listing-condition').value = l.condition || 'N/A';
      document.getElementById('listing-price').value = l.price || '';
      document.getElementById('listing-payment').value = l.payment_methods || '';
      document.getElementById('listing-location').value = l.location || '';
    }
  } else {
    document.getElementById('listing-title').value = '';
    document.getElementById('listing-description').value = '';
    document.getElementById('listing-price').value = '';
    document.getElementById('listing-payment').value = '';
    document.getElementById('listing-location').value = '';
  }
  document.getElementById('listing-modal').style.display = '';
}

function editListing(id) { openListingModal(id); }

function markListingSold(id) {
  if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_update', id, status: 'sold' }));
}

function deleteListing(id) {
  if (!confirm('Delete this listing?')) return;
  if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_delete', id }));
}

function showListingDetail(id) {
  const l = marketListings.find(x => x.id === id);
  if (!l) return;
  const modal = document.getElementById('listing-detail-modal');
  const content = document.getElementById('listing-detail-content');
  const isMine = l.seller_key === marketMyKey;
  const catColor = CATEGORY_COLORS[l.category] || '#888';
  content.innerHTML =
    '<button onclick="closeListingDetail()" style="float:right;background:none;border:none;color:var(--text-muted);font-size:1.2rem;cursor:pointer;">' + hosIcon('close', 14) + '</button>' +
    '<h3 style="color:var(--text);margin:0 0 var(--space-md);font-size:1.05rem;">' + escHtml(l.title) + '</h3>' +
    '<div style="font-size:1.1rem;font-weight:700;color:var(--accent);margin-bottom:var(--space-md);">' + escHtml(l.price || 'Contact for price') + '</div>' +
    '<div style="display:flex;gap:var(--space-md);flex-wrap:wrap;margin-bottom:var(--space-lg);">' +
      '<span style="background:' + catColor + '22;color:' + catColor + ';font-size:0.7rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">' + escHtml(l.category) + '</span>' +
      (l.condition && l.condition !== 'N/A' ? '<span style="background:rgba(255,255,255,0.05);color:var(--text-muted);font-size:0.7rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">' + escHtml(l.condition) + '</span>' : '') +
    '</div>' +
    (l.description ? '<div style="font-size:0.85rem;color:var(--text-muted);margin-bottom:var(--space-lg);white-space:pre-wrap;line-height:1.5;">' + escHtml(l.description) + '</div>' : '') +
    '<div style="font-size:0.8rem;color:var(--text-muted);margin-bottom:var(--space-sm);">Seller: <strong style="color:var(--text);">' + escHtml(l.seller_name || 'Anonymous') + '</strong></div>' +
    (l.payment_methods ? '<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-sm);">Payment: ' + escHtml(l.payment_methods) + '</div>' : '') +
    (l.location ? '<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-lg);display:flex;align-items:center;gap:var(--space-sm);">' + hosIcon('mappin', 14) + ' ' + escHtml(l.location) + '</div>' : '') +
    (isMine ? '<div style="display:flex;gap:var(--space-md);margin-top:var(--space-md);">' +
      '<button onclick="editListing(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;color:var(--text);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('edit', 14) + ' Edit</button>' +
      '<button onclick="markListingSold(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;color:var(--success);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('check', 14) + ' Mark Sold</button>' +
      '<button onclick="deleteListing(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid rgba(229,85,85,0.4);border-radius:6px;color:var(--error);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('trash', 14) + ' Delete</button>' +
    '</div>' : '');
  modal.style.display = '';
}

function closeListingDetail() { document.getElementById('listing-detail-modal').style.display = 'none'; }

function streamHandleMessage(msg) { /* WebRTC streaming not yet implemented in market view */ }

  function marketConnect() {
   const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
   marketWs = new WebSocket(proto + '//' + location.host + '/ws');
   marketWs.onopen = () => {
    let storedKey = localStorage.getItem('humanity_key');
    // Also check the Ed25519 key backup (set by chat client's crypto.js)
    if (!storedKey) {
     try {
      const backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
      if (backup && backup.publicKeyHex) storedKey = backup.publicKeyHex;
     } catch(e) {}
    }
    const storedName = localStorage.getItem('humanity_name');
    if (storedKey) {
     marketMyKey = storedKey;
     marketMyName = storedName;
     marketWs.send(JSON.stringify({ type: 'identify', public_key: storedKey, display_name: storedName || null }));
    } else {
     marketMyKey = 'viewer_' + Math.random().toString(36).slice(2, 10);
     marketWs.send(JSON.stringify({ type: 'identify', public_key: marketMyKey, display_name: null }));
    }
   };
   ws = marketWs; // Alias for streaming code
   window._humanityWs = marketWs; // Alias for Skill DNA
   marketWs.onmessage = (e) => {
    try {
     const msg = JSON.parse(e.data);
     handleMarketMessage(msg);
     // Route stream messages
     if (msg.type && msg.type.startsWith('stream_')) {
      streamHandleMessage(msg);
     }
     // (stream_viewer_ready via Private/System removed — using direct stream_offer request instead)
     // Handle skill verification messages via Private/system
     if (msg.type === 'private' && msg.message) {
      if (msg.message.startsWith('__skill_verify_req__:')) {
       try {
        const payload = JSON.parse(msg.message.slice('__skill_verify_req__:'.length));
        if (confirm(`${payload.from_name} claims ${payload.skill_id} Lv ${payload.level} — can you verify?\n\nClick OK to verify, Cancel to decline.`)) {
         const note = prompt('Add a note (optional):') || 'Verified';
         window._humanityWs.send(JSON.stringify({ type: 'skill_verify_response', skill_id: payload.skill_id, to_key: payload.from_key, approved: true, note }));
        }
       } catch(e) {}
      }
      if (msg.message.startsWith('__skill_verify_resp__:')) {
       try {
        const payload = JSON.parse(msg.message.slice('__skill_verify_resp__:'.length));
        if (window._sdHandleVerifyResponse) window._sdHandleVerifyResponse(payload);
       } catch(e) {}
      }
     }
    } catch {}
   };
   marketWs.onclose = () => { setTimeout(marketConnect, 3000); };
   marketWs.onerror = () => {};
  }

  function renderMarketListings() {
   const search = (document.getElementById('market-search').value || '').toLowerCase();
   const catFilter = document.getElementById('market-category-filter').value;
   const condFilter = document.getElementById('market-condition-filter').value;
   const sort = document.getElementById('market-sort').value;

   let filtered = marketListings.filter(l => {
    if (l.status !== 'active') return false;
    if (catFilter && l.category !== catFilter) return false;
    if (condFilter && l.condition !== condFilter) return false;
    if (search && !l.title.toLowerCase().includes(search) && !(l.description||'').toLowerCase().includes(search) && !(l.seller_name||'').toLowerCase().includes(search)) return false;
    return true;
   });

   if (sort === 'oldest') filtered.sort((a, b) => (a.created_at || '').localeCompare(b.created_at || ''));
   else if (sort === 'alpha') filtered.sort((a, b) => a.title.localeCompare(b.title));
   else filtered.sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));

   const grid = document.getElementById('market-listings-grid');
   const empty = document.getElementById('market-listings-empty');
   if (filtered.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
   } else {
    empty.style.display = 'none';
    grid.innerHTML = filtered.map(l => renderListingCard(l, false)).join('');
   }
  }

  function renderListingCard(l, showActions) {
   const catColor = CATEGORY_COLORS[l.category] || '#888';
   const isMine = l.seller_key === marketMyKey;
   const isAdmin = marketMyRole === 'admin' || marketMyRole === 'mod';
   const actions = (isMine || showActions) ? `
    <div style="display:flex;gap:var(--space-sm);margin-top:var(--space-md);">
     ${isMine?`<button onclick="editListing('${l.id}')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--text-muted);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">${hosIcon('edit', 14)} Edit</button>`:''}
     ${isMine?`<button onclick="markListingSold('${l.id}')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--success);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">${hosIcon('check', 14)} Sold</button>`:''}
     ${(isMine||isAdmin)?`<button onclick="deleteListing('${l.id}')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--error);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">${hosIcon('trash', 14)} Delete</button>`:''}
    </div>` : '';
   const statusBadge = l.status === 'sold' ? '<span style="background:#4a8;color:#fff;font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;margin-left:var(--space-sm);">SOLD</span>' :
             l.status === 'withdrawn' ? '<span style="background:#888;color:#fff;font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;margin-left:var(--space-sm);">WITHDRAWN</span>' : '';
   return `
    <div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;overflow:hidden;cursor:pointer;transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'" onclick="showListingDetail('${l.id}')">
     <div style="height:120px;background:linear-gradient(135deg,rgba(255,255,255,0.02),rgba(255,255,255,0.06));display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:2rem;">${l.category === '3D Models' ? '🧊' : '📦'}</div>
     <div style="padding:var(--space-xl);">
      <div style="display:flex;justify-content:space-between;align-items:start;margin-bottom:var(--space-sm);">
       <span style="font-weight:600;font-size:0.85rem;color:var(--text);flex:1;">${escHtml(l.title)}${statusBadge}</span>
      </div>
      <div style="font-size:0.95rem;font-weight:700;color:var(--accent);margin-bottom:var(--space-sm);">${escHtml(l.price || 'Contact for price')}</div>
      <div style="display:flex;gap:var(--space-sm);flex-wrap:wrap;margin-bottom:var(--space-sm);">
       <span style="background:${catColor}22;color:${catColor};font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">${escHtml(l.category)}</span>
       ${l.condition && l.condition !== 'N/A' ? `<span style="background:rgba(255,255,255,0.05);color:var(--text-muted);font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">${escHtml(l.condition)}</span>` : ''}
      </div>
      <div style="font-size:0.72rem;color:var(--text-muted);">by ${escHtml(l.seller_name || 'Anonymous')}</div>
      ${l.location ? `<div style="font-size:0.68rem;color:var(--text-muted);display:flex;align-items:center;gap:var(--space-xs);">${hosIcon('mappin', 14)} ${escHtml(l.location)}</div>` : ''}
      ${actions}
     </div>
    </div>`;
  }

  function closeListingModal() {
   document.getElementById('listing-modal').style.display = 'none';
  }

  function submitListing() {
   const title = document.getElementById('listing-title').value.trim();
   if (!title) { document.getElementById('listing-title').style.borderColor = '#e55'; return; }
   const editId = document.getElementById('listing-edit-id').value;
   const data = {
    title,
    description: document.getElementById('listing-description').value.trim(),
    category: document.getElementById('listing-category').value,
    condition: document.getElementById('listing-condition').value,
    price: document.getElementById('listing-price').value.trim(),
    payment_methods: document.getElementById('listing-payment').value.trim(),
    location: document.getElementById('listing-location').value.trim(),
   };
   if (editId) {
    data.id = editId;
    if (marketWs && marketWs.readyState === 1) {
     marketWs.send(JSON.stringify({ type: 'listing_update', ...data }));
    }
   } else {
    data.id = Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
    if (marketWs && marketWs.readyState === 1) {
     marketWs.send(JSON.stringify({ type: 'listing_create', ...data }));
    }
   }
   closeListingModal();
  }

  function renderMyListings() {
   const mine = marketListings.filter(l => l.seller_key === marketMyKey);
   const grid = document.getElementById('my-listings-grid');
   const empty = document.getElementById('my-listings-empty');
   if (mine.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
   } else {
    empty.style.display = 'none';
    grid.innerHTML = mine.map(l => renderListingCard(l, true)).join('');
   }
  }

  function renderStoreDirectory() {
   const filter = document.getElementById('store-category-filter').value;
   const filtered = filter ? STORE_DIRECTORY.filter(s => s.category === filter) : STORE_DIRECTORY;
   const grid = document.getElementById('store-directory-grid');
   if (!grid) return;
   if (!filtered.length) { grid.innerHTML = '<div style="color:var(--text-muted);font-style:italic;padding:var(--space-3xl);text-align:center;">No stores listed yet.</div>'; return; }
   grid.innerHTML = filtered.map(s => `
    <div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;padding:var(--space-xl);transition:border-color 0.2s;" onmouseenter="this.style.borderColor='rgba(255,136,17,0.3)'" onmouseleave="this.style.borderColor='var(--border)'">
     <div style="font-size:1.5rem;margin-bottom:var(--space-md);">${s.icon}</div>
     <div style="font-weight:600;font-size:0.9rem;color:var(--text);margin-bottom:var(--space-xs);">${escHtml(s.name)}</div>
     <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:var(--space-md);">${escHtml(s.category)}</div>
     <div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-lg);">${escHtml(s.description)}</div>
     <a href="${s.url}" target="_blank" rel="noopener" style="display:inline-block;padding:var(--space-sm) var(--space-xl);background:var(--accent);color:#fff;border-radius:6px;text-decoration:none;font-size:0.75rem;font-weight:600;">Visit Store →</a>
    </div>`).join('');
  }

document.addEventListener('DOMContentLoaded', function() {
  marketConnect();
  renderMarketListings();
});
