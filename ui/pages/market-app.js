const API_BASE = '';
let marketWs = null;
let marketListings = [];
let marketMyKey = '';
let marketMyName = 'Visitor';
let marketMyRole = '';
/** Cache of seller ratings: { [seller_key]: { avg: number, count: number } } */
const sellerRatings = {};
/** Cache of reviews per listing: { [listing_id]: ReviewData[] } */
const listingReviews = {};
const CATEGORY_COLORS = {
  Electronics:'#4488ff', Vehicles:'#f80', Clothing:'#f48', Tools:'#8b4',
  Furniture:'#a67', Home:'#68a', 'Books/Media':'#a88', Gaming:'#84f',
  Sports:'#4b8', Crafts:'#fa8', 'Food/Garden':'#4a8', Services:'#88f',
  '3D Models':'#f84', Other:'#888'
};
const STORE_DIRECTORY = [];

function escHtml(s) { return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }

/** Render star display: filled gold stars + grey outline stars. */
function renderStars(rating, size) {
  size = size || 14;
  let html = '';
  const full = Math.round(rating);
  for (let i = 0; i < 5; i++) {
    if (i < full) {
      html += '<span style="color:#f5a623;font-size:' + size + 'px;">&#9733;</span>';
    } else {
      html += '<span style="color:#555;font-size:' + size + 'px;">&#9734;</span>';
    }
  }
  return html;
}

/** Render clickable star selector for review form. */
function renderStarSelector(currentRating) {
  let html = '<div id="review-star-selector" style="display:inline-flex;gap:2px;cursor:pointer;">';
  for (let i = 1; i <= 5; i++) {
    const filled = i <= currentRating;
    html += '<span onclick="setReviewRating(' + i + ')" onmouseenter="previewStars(' + i + ')" onmouseleave="previewStars(0)" data-star="' + i + '" style="color:' + (filled ? '#f5a623' : '#555') + ';font-size:24px;transition:color 0.1s;user-select:none;">&#9733;</span>';
  }
  html += '</div>';
  return html;
}

let _reviewRating = 0;

function setReviewRating(n) {
  _reviewRating = n;
  const stars = document.querySelectorAll('#review-star-selector span');
  stars.forEach(function(s, i) { s.style.color = (i < n) ? '#f5a623' : '#555'; });
}

function previewStars(n) {
  if (n === 0) { setReviewRating(_reviewRating); return; }
  const stars = document.querySelectorAll('#review-star-selector span');
  stars.forEach(function(s, i) { s.style.color = (i < n) ? '#f5a623' : '#555'; });
}

/** Fetch seller rating via REST API and cache it. */
async function fetchSellerRating(sellerKey) {
  if (sellerRatings[sellerKey]) return sellerRatings[sellerKey];
  try {
    const res = await fetch(API_BASE + '/api/sellers/' + encodeURIComponent(sellerKey) + '/rating');
    if (res.ok) {
      const data = await res.json();
      sellerRatings[sellerKey] = { avg: data.avg_rating || 0, count: data.review_count || 0 };
      return sellerRatings[sellerKey];
    }
  } catch (e) { /* ignore */ }
  return { avg: 0, count: 0 };
}

/** Fetch reviews for a listing via REST API. */
async function fetchListingReviews(listingId) {
  try {
    const res = await fetch(API_BASE + '/api/listings/' + encodeURIComponent(listingId) + '/reviews');
    if (res.ok) {
      const data = await res.json();
      listingReviews[listingId] = data.reviews || [];
      const listing = marketListings.find(function(l) { return l.id === listingId; });
      if (listing) {
        sellerRatings[listing.seller_key] = { avg: data.avg_rating || 0, count: data.review_count || 0 };
      }
      return data;
    }
  } catch (e) { /* ignore */ }
  return { reviews: [], avg_rating: 0, review_count: 0 };
}

function showMarketSection(section) {
  ['marketplace','stores','mylistings'].forEach(function(s) {
    document.getElementById('market-section-' + s).style.display = s === section ? '' : 'none';
    var btn = document.getElementById('market-nav-' + s);
    if (btn) { btn.classList.toggle('btn-clickable', s === section); }
  });
  if (section === 'marketplace') renderMarketListings();
  if (section === 'stores') renderStoreDirectory();
  if (section === 'mylistings') renderMyListings();
}

function handleMarketMessage(msg) {
  if (msg.type === 'listing_list') {
    marketListings = msg.listings || [];
    var sellers = [];
    marketListings.forEach(function(l) {
      if (sellers.indexOf(l.seller_key) === -1) sellers.push(l.seller_key);
    });
    sellers.forEach(function(sk) {
      fetchSellerRating(sk).then(function() { renderMarketListings(); });
    });
    renderMarketListings();
    renderMyListings();
  } else if (msg.type === 'listing_new') {
    if (msg.listing) {
      marketListings = marketListings.filter(function(l) { return l.id !== msg.listing.id; });
      marketListings.unshift(msg.listing);
      fetchSellerRating(msg.listing.seller_key);
      renderMarketListings();
      renderMyListings();
    }
  } else if (msg.type === 'listing_updated') {
    if (msg.listing) {
      var idx = marketListings.findIndex(function(l) { return l.id === msg.listing.id; });
      if (idx >= 0) marketListings[idx] = msg.listing;
      else marketListings.unshift(msg.listing);
      renderMarketListings();
      renderMyListings();
    }
  } else if (msg.type === 'listing_deleted') {
    if (msg.id) marketListings = marketListings.filter(function(l) { return l.id !== msg.id; });
    renderMarketListings();
    renderMyListings();
  } else if (msg.type === 'review_created') {
    if (msg.review) {
      var lid = msg.review.listing_id;
      if (!listingReviews[lid]) listingReviews[lid] = [];
      listingReviews[lid] = listingReviews[lid].filter(function(r) { return r.id !== msg.review.id; });
      listingReviews[lid].unshift(msg.review);
      var listing = marketListings.find(function(l) { return l.id === lid; });
      if (listing) {
        delete sellerRatings[listing.seller_key];
        fetchSellerRating(listing.seller_key).then(function() {
          renderMarketListings();
          renderMyListings();
        });
      }
      refreshDetailReviews(lid);
    }
  } else if (msg.type === 'review_deleted') {
    if (msg.listing_id && msg.review_id) {
      var lid2 = msg.listing_id;
      if (listingReviews[lid2]) {
        listingReviews[lid2] = listingReviews[lid2].filter(function(r) { return r.id !== msg.review_id; });
      }
      var listing2 = marketListings.find(function(l) { return l.id === lid2; });
      if (listing2) {
        delete sellerRatings[listing2.seller_key];
        fetchSellerRating(listing2.seller_key).then(function() {
          renderMarketListings();
          renderMyListings();
        });
      }
      refreshDetailReviews(lid2);
    }
  } else if (msg.type === 'peer_list') {
    if (msg.peers && marketMyKey) {
      var me = msg.peers.find(function(p) { return p.public_key_hex === marketMyKey || p.public_key === marketMyKey; });
      if (me) { marketMyRole = me.role || ''; }
    }
    var canList = marketMyRole === 'admin' || marketMyRole === 'mod' || marketMyRole === 'verified' || marketMyRole === 'donor';
    var btn = document.getElementById('market-create-btn');
    if (btn) btn.style.display = canList ? 'inline-flex' : 'none';
    if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_browse' }));
  }
}

function openListingModal(editId) {
  document.getElementById('listing-edit-id').value = editId || '';
  document.getElementById('listing-modal-title').textContent = editId ? 'Edit Listing' : 'Create Listing';
  if (editId) {
    var l = marketListings.find(function(x) { return x.id === editId; });
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
  if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_update', id: id, status: 'sold' }));
}

function deleteListing(id) {
  if (!confirm('Delete this listing?')) return;
  if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify({ type: 'listing_delete', id: id }));
}

/** Currently displayed listing ID in detail modal (for live review updates). */
var _detailListingId = null;

function showListingDetail(id) {
  _detailListingId = id;
  var l = marketListings.find(function(x) { return x.id === id; });
  if (!l) return;
  var modal = document.getElementById('listing-detail-modal');
  var content = document.getElementById('listing-detail-content');
  var isMine = l.seller_key === marketMyKey;
  var catColor = CATEGORY_COLORS[l.category] || '#888';
  var sr = sellerRatings[l.seller_key];
  var sellerRatingHtml = sr && sr.count > 0
    ? '<div style="margin-bottom:var(--space-md);">' + renderStars(sr.avg) + ' <span style="font-size:0.75rem;color:var(--text-muted);">' + sr.avg.toFixed(1) + ' (' + sr.count + ' review' + (sr.count !== 1 ? 's' : '') + ')</span></div>'
    : '';

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
    sellerRatingHtml +
    (l.payment_methods ? '<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-sm);">Payment: ' + escHtml(l.payment_methods) + '</div>' : '') +
    (l.location ? '<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-lg);display:flex;align-items:center;gap:var(--space-sm);">' + hosIcon('mappin', 14) + ' ' + escHtml(l.location) + '</div>' : '') +
    (isMine ? '<div style="display:flex;gap:var(--space-md);margin-top:var(--space-md);margin-bottom:var(--space-xl);">' +
      '<button onclick="editListing(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;color:var(--text);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('edit', 14) + ' Edit</button>' +
      '<button onclick="markListingSold(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:6px;color:var(--success);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('check', 14) + ' Mark Sold</button>' +
      '<button onclick="deleteListing(\'' + l.id + '\');closeListingDetail()" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid rgba(229,85,85,0.4);border-radius:6px;color:var(--error);cursor:pointer;font-size:0.8rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-sm);">' + hosIcon('trash', 14) + ' Delete</button>' +
    '</div>' : '') +
    '<div id="listing-reviews-section" style="border-top:1px solid var(--border);padding-top:var(--space-xl);margin-top:var(--space-xl);">' +
      '<h4 style="color:var(--text);margin:0 0 var(--space-lg);font-size:0.9rem;">Reviews</h4>' +
      '<div id="listing-reviews-loading" style="color:var(--text-muted);font-size:0.8rem;font-style:italic;">Loading reviews...</div>' +
      '<div id="listing-reviews-list"></div>' +
    '</div>';

  modal.style.display = '';
  fetchListingReviews(id).then(function(data) { renderDetailReviews(id, data); });
}

/** Render the reviews section inside the detail modal. */
function renderDetailReviews(listingId, data) {
  var container = document.getElementById('listing-reviews-list');
  var loading = document.getElementById('listing-reviews-loading');
  if (!container) return;
  if (loading) loading.style.display = 'none';

  var l = marketListings.find(function(x) { return x.id === listingId; });
  var isMine = l && l.seller_key === marketMyKey;
  var reviews = data.reviews || listingReviews[listingId] || [];
  var isAdmin = marketMyRole === 'admin' || marketMyRole === 'mod';
  var alreadyReviewed = reviews.some(function(r) { return r.reviewer_key === marketMyKey; });
  var canReview = !isMine && !alreadyReviewed && marketMyKey && marketMyKey.indexOf('viewer_') !== 0;

  var sortedReviews = reviews.slice();
  var sortSel = document.getElementById('review-sort-select');
  var sortVal = sortSel ? sortSel.value : 'newest';

  if (sortVal === 'highest') sortedReviews.sort(function(a, b) { return b.rating - a.rating; });
  else if (sortVal === 'lowest') sortedReviews.sort(function(a, b) { return a.rating - b.rating; });

  var html = '';

  if (data.avg_rating > 0 || reviews.length > 0) {
    var avg = data.avg_rating || 0;
    var count = data.review_count || reviews.length;
    html += '<div style="margin-bottom:var(--space-lg);display:flex;align-items:center;gap:var(--space-md);">' +
      renderStars(avg, 18) +
      '<span style="font-size:0.85rem;font-weight:600;color:var(--text);">' + avg.toFixed(1) + '</span>' +
      '<span style="font-size:0.75rem;color:var(--text-muted);">(' + count + ' review' + (count !== 1 ? 's' : '') + ')</span>' +
    '</div>';
  }

  if (reviews.length > 1) {
    html += '<div style="margin-bottom:var(--space-lg);">' +
      '<select id="review-sort-select" onchange="resortReviews(\'' + escHtml(listingId) + '\')" style="padding:var(--space-sm) var(--space-md);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--text);font-size:0.75rem;">' +
        '<option value="newest"' + (sortVal === 'newest' ? ' selected' : '') + '>Newest</option>' +
        '<option value="highest"' + (sortVal === 'highest' ? ' selected' : '') + '>Highest</option>' +
        '<option value="lowest"' + (sortVal === 'lowest' ? ' selected' : '') + '>Lowest</option>' +
      '</select>' +
    '</div>';
  }

  if (canReview) {
    html += '<div style="background:var(--bg-panel);border:1px solid var(--border);border-radius:8px;padding:var(--space-lg);margin-bottom:var(--space-xl);">' +
      '<div style="font-size:0.8rem;font-weight:600;color:var(--text);margin-bottom:var(--space-md);">Write a Review</div>' +
      '<div style="margin-bottom:var(--space-md);">' +
        '<label style="font-size:0.7rem;color:var(--text-muted);display:block;margin-bottom:var(--space-xs);">Rating</label>' +
        renderStarSelector(0) +
      '</div>' +
      '<div style="margin-bottom:var(--space-md);">' +
        '<textarea id="review-comment" rows="3" maxlength="2000" placeholder="Share your experience..." style="width:100%;padding:var(--space-md);background:var(--bg-input);border:1px solid var(--border);border-radius:6px;color:var(--text);font-size:0.8rem;resize:vertical;font-family:inherit;box-sizing:border-box;"></textarea>' +
      '</div>' +
      '<button onclick="submitReview(\'' + escHtml(listingId) + '\')" class="btn btn-clickable" style="min-width:auto;min-height:32px;padding:var(--space-sm) var(--space-xl);font-size:0.8rem;">Submit Review</button>' +
      '<div id="review-error" style="color:var(--danger);font-size:0.75rem;margin-top:var(--space-sm);display:none;"></div>' +
    '</div>';
  }

  if (sortedReviews.length === 0) {
    html += '<div style="color:var(--text-muted);font-style:italic;font-size:0.8rem;padding:var(--space-lg) 0;">No reviews yet.</div>';
  } else {
    sortedReviews.forEach(function(r) {
      var canDelete = r.reviewer_key === marketMyKey || isAdmin;
      var dateStr = r.created_at ? new Date(r.created_at).toLocaleDateString() : '';
      html += '<div style="border-bottom:1px solid var(--border);padding:var(--space-lg) 0;">' +
        '<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:var(--space-sm);">' +
          '<div>' +
            '<strong style="font-size:0.8rem;color:var(--text);">' + escHtml(r.reviewer_name || 'Anonymous') + '</strong>' +
            '<span style="margin-left:var(--space-md);">' + renderStars(r.rating, 12) + '</span>' +
          '</div>' +
          '<div style="display:flex;align-items:center;gap:var(--space-md);">' +
            '<span style="font-size:0.68rem;color:var(--text-muted);">' + escHtml(dateStr) + '</span>' +
            (canDelete ? '<button onclick="deleteReview(\'' + escHtml(listingId) + '\',' + r.id + ')" style="background:none;border:none;color:var(--danger);cursor:pointer;font-size:0.7rem;padding:2px;" title="Delete review">' + hosIcon('trash', 12) + '</button>' : '') +
          '</div>' +
        '</div>' +
        (r.comment ? '<div style="font-size:0.8rem;color:var(--text-muted);line-height:1.4;">' + escHtml(r.comment) + '</div>' : '') +
      '</div>';
    });
  }

  container.innerHTML = html;
  _reviewRating = 0;
}

function resortReviews(listingId) {
  var listing = marketListings.find(function(l) { return l.id === listingId; });
  var sr = listing ? sellerRatings[listing.seller_key] : null;
  renderDetailReviews(listingId, {
    reviews: listingReviews[listingId] || [],
    avg_rating: sr ? sr.avg : 0,
    review_count: sr ? sr.count : 0,
  });
}

function refreshDetailReviews(listingId) {
  if (_detailListingId === listingId) {
    resortReviews(listingId);
  }
}

function submitReview(listingId) {
  if (_reviewRating < 1 || _reviewRating > 5) {
    var err = document.getElementById('review-error');
    if (err) { err.textContent = 'Please select a rating (1-5 stars).'; err.style.display = ''; }
    return;
  }
  var commentEl = document.getElementById('review-comment');
  var comment = commentEl ? commentEl.value.trim() : '';
  if (marketWs && marketWs.readyState === 1) {
    marketWs.send(JSON.stringify({
      type: 'review_create',
      listing_id: listingId,
      rating: _reviewRating,
      comment: comment,
    }));
  }
}

function deleteReview(listingId, reviewId) {
  if (!confirm('Delete this review?')) return;
  if (marketWs && marketWs.readyState === 1) {
    marketWs.send(JSON.stringify({
      type: 'review_delete',
      listing_id: listingId,
      review_id: reviewId,
    }));
  }
}

function closeListingDetail() {
  _detailListingId = null;
  document.getElementById('listing-detail-modal').style.display = 'none';
}

function streamHandleMessage(msg) { /* WebRTC streaming not yet implemented in market view */ }

function marketConnect() {
  var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  marketWs = new WebSocket(proto + '//' + location.host + '/ws');
  marketWs.onopen = function() {
    var storedKey = localStorage.getItem('humanity_key');
    if (!storedKey) {
      try {
        var backup = JSON.parse(localStorage.getItem('humanity_key_backup') || 'null');
        if (backup && backup.publicKeyHex) storedKey = backup.publicKeyHex;
      } catch(e) {}
    }
    var storedName = localStorage.getItem('humanity_name');
    if (storedKey) {
      marketMyKey = storedKey;
      marketMyName = storedName;
      marketWs.send(JSON.stringify({ type: 'identify', public_key: storedKey, display_name: storedName || null }));
    } else {
      marketMyKey = 'viewer_' + Math.random().toString(36).slice(2, 10);
      marketWs.send(JSON.stringify({ type: 'identify', public_key: marketMyKey, display_name: null }));
    }
  };
  ws = marketWs;
  window._humanityWs = marketWs;
  marketWs.onmessage = function(e) {
    try {
      var msg = JSON.parse(e.data);
      handleMarketMessage(msg);
      if (msg.type && msg.type.startsWith('stream_')) {
        streamHandleMessage(msg);
      }
      if (msg.type === 'private' && msg.message) {
        if (msg.message.startsWith('__skill_verify_req__:')) {
          try {
            var payload = JSON.parse(msg.message.slice('__skill_verify_req__:'.length));
            if (confirm(payload.from_name + ' claims ' + payload.skill_id + ' Lv ' + payload.level + ' \u2014 can you verify?\n\nClick OK to verify, Cancel to decline.')) {
              var note = prompt('Add a note (optional):') || 'Verified';
              window._humanityWs.send(JSON.stringify({ type: 'skill_verify_response', skill_id: payload.skill_id, to_key: payload.from_key, approved: true, note: note }));
            }
          } catch(e2) {}
        }
        if (msg.message.startsWith('__skill_verify_resp__:')) {
          try {
            var payload2 = JSON.parse(msg.message.slice('__skill_verify_resp__:'.length));
            if (window._sdHandleVerifyResponse) window._sdHandleVerifyResponse(payload2);
          } catch(e3) {}
        }
      }
    } catch(ex) {}
  };
  marketWs.onclose = function() { setTimeout(marketConnect, 3000); };
  marketWs.onerror = function() {};
}

function renderMarketListings() {
  var search = (document.getElementById('market-search').value || '').toLowerCase();
  var catFilter = document.getElementById('market-category-filter').value;
  var condFilter = document.getElementById('market-condition-filter').value;
  var sort = document.getElementById('market-sort').value;

  var filtered = marketListings.filter(function(l) {
    if (l.status !== 'active') return false;
    if (catFilter && l.category !== catFilter) return false;
    if (condFilter && l.condition !== condFilter) return false;
    if (search && !l.title.toLowerCase().includes(search) && !(l.description||'').toLowerCase().includes(search) && !(l.seller_name||'').toLowerCase().includes(search)) return false;
    return true;
  });

  if (sort === 'oldest') filtered.sort(function(a, b) { return (a.created_at || '').localeCompare(b.created_at || ''); });
  else if (sort === 'alpha') filtered.sort(function(a, b) { return a.title.localeCompare(b.title); });
  else filtered.sort(function(a, b) { return (b.created_at || '').localeCompare(a.created_at || ''); });

  var grid = document.getElementById('market-listings-grid');
  var empty = document.getElementById('market-listings-empty');
  if (filtered.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
  } else {
    empty.style.display = 'none';
    grid.innerHTML = filtered.map(function(l) { return renderListingCard(l, false); }).join('');
  }
}

function renderListingCard(l, showActions) {
  var catColor = CATEGORY_COLORS[l.category] || '#888';
  var isMine = l.seller_key === marketMyKey;
  var isAdmin = marketMyRole === 'admin' || marketMyRole === 'mod';
  var sr = sellerRatings[l.seller_key];
  var ratingHtml = sr && sr.count > 0
    ? '<div style="display:flex;align-items:center;gap:4px;margin-top:var(--space-xs);">' + renderStars(sr.avg, 11) + '<span style="font-size:0.6rem;color:var(--text-muted);">(' + sr.count + ')</span></div>'
    : '';
  var actions = (isMine || showActions) ?
    '<div style="display:flex;gap:var(--space-sm);margin-top:var(--space-md);">' +
     (isMine ? '<button onclick="event.stopPropagation();editListing(\'' + l.id + '\')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--text-muted);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">' + hosIcon('edit', 14) + ' Edit</button>' : '') +
     (isMine ? '<button onclick="event.stopPropagation();markListingSold(\'' + l.id + '\')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--success);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">' + hosIcon('check', 14) + ' Sold</button>' : '') +
     ((isMine||isAdmin) ? '<button onclick="event.stopPropagation();deleteListing(\'' + l.id + '\')" style="flex:1;padding:var(--space-sm);background:var(--bg-panel);border:1px solid var(--border);border-radius:4px;color:var(--error);cursor:pointer;font-size:0.7rem;display:inline-flex;align-items:center;justify-content:center;gap:var(--space-xs);">' + hosIcon('trash', 14) + ' Delete</button>' : '') +
    '</div>' : '';
  var statusBadge = l.status === 'sold' ? '<span style="background:#4a8;color:#fff;font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;margin-left:var(--space-sm);">SOLD</span>' :
            l.status === 'withdrawn' ? '<span style="background:#888;color:#fff;font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;margin-left:var(--space-sm);">WITHDRAWN</span>' : '';
  return '<div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;overflow:hidden;cursor:pointer;transition:border-color 0.2s;" onmouseenter="this.style.borderColor=\'rgba(255,136,17,0.3)\'" onmouseleave="this.style.borderColor=\'var(--border)\'" onclick="showListingDetail(\'' + l.id + '\')">' +
    '<div style="height:120px;background:linear-gradient(135deg,rgba(255,255,255,0.02),rgba(255,255,255,0.06));display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:2rem;">' + (l.category === '3D Models' ? '&#129482;' : '&#128230;') + '</div>' +
    '<div style="padding:var(--space-xl);">' +
      '<div style="display:flex;justify-content:space-between;align-items:start;margin-bottom:var(--space-sm);">' +
       '<span style="font-weight:600;font-size:0.85rem;color:var(--text);flex:1;">' + escHtml(l.title) + statusBadge + '</span>' +
      '</div>' +
      '<div style="font-size:0.95rem;font-weight:700;color:var(--accent);margin-bottom:var(--space-sm);">' + escHtml(l.price || 'Contact for price') + '</div>' +
      '<div style="display:flex;gap:var(--space-sm);flex-wrap:wrap;margin-bottom:var(--space-sm);">' +
       '<span style="background:' + catColor + '22;color:' + catColor + ';font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">' + escHtml(l.category) + '</span>' +
       (l.condition && l.condition !== 'N/A' ? '<span style="background:rgba(255,255,255,0.05);color:var(--text-muted);font-size:0.6rem;padding:var(--space-xs) var(--space-md);border-radius:4px;">' + escHtml(l.condition) + '</span>' : '') +
      '</div>' +
      '<div style="font-size:0.72rem;color:var(--text-muted);">by ' + escHtml(l.seller_name || 'Anonymous') + '</div>' +
      ratingHtml +
      (l.location ? '<div style="font-size:0.68rem;color:var(--text-muted);display:flex;align-items:center;gap:var(--space-xs);">' + hosIcon('mappin', 14) + ' ' + escHtml(l.location) + '</div>' : '') +
      actions +
    '</div>' +
  '</div>';
}

function closeListingModal() {
  document.getElementById('listing-modal').style.display = 'none';
}

function submitListing() {
  var title = document.getElementById('listing-title').value.trim();
  if (!title) { document.getElementById('listing-title').style.borderColor = '#e55'; return; }
  var editId = document.getElementById('listing-edit-id').value;
  var data = {
    title: title,
    description: document.getElementById('listing-description').value.trim(),
    category: document.getElementById('listing-category').value,
    condition: document.getElementById('listing-condition').value,
    price: document.getElementById('listing-price').value.trim(),
    payment_methods: document.getElementById('listing-payment').value.trim(),
    location: document.getElementById('listing-location').value.trim(),
  };
  if (editId) {
    data.type = 'listing_update';
    data.id = editId;
    if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify(data));
  } else {
    data.type = 'listing_create';
    data.id = Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
    if (marketWs && marketWs.readyState === 1) marketWs.send(JSON.stringify(data));
  }
  closeListingModal();
}

function renderMyListings() {
  var mine = marketListings.filter(function(l) { return l.seller_key === marketMyKey; });
  var grid = document.getElementById('my-listings-grid');
  var empty = document.getElementById('my-listings-empty');
  if (mine.length === 0) {
    grid.innerHTML = '';
    empty.style.display = '';
  } else {
    empty.style.display = 'none';
    grid.innerHTML = mine.map(function(l) { return renderListingCard(l, true); }).join('');
  }
}

function renderStoreDirectory() {
  var filter = document.getElementById('store-category-filter').value;
  var filtered = filter ? STORE_DIRECTORY.filter(function(s) { return s.category === filter; }) : STORE_DIRECTORY;
  var grid = document.getElementById('store-directory-grid');
  if (!grid) return;
  if (!filtered.length) { grid.innerHTML = '<div style="color:var(--text-muted);font-style:italic;padding:var(--space-3xl);text-align:center;">No stores listed yet.</div>'; return; }
  grid.innerHTML = filtered.map(function(s) {
    return '<div style="background:var(--bg-card);border:1px solid var(--border);border-radius:10px;padding:var(--space-xl);transition:border-color 0.2s;" onmouseenter="this.style.borderColor=\'rgba(255,136,17,0.3)\'" onmouseleave="this.style.borderColor=\'var(--border)\'">' +
     '<div style="font-size:1.5rem;margin-bottom:var(--space-md);">' + s.icon + '</div>' +
     '<div style="font-weight:600;font-size:0.9rem;color:var(--text);margin-bottom:var(--space-xs);">' + escHtml(s.name) + '</div>' +
     '<div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:var(--space-md);">' + escHtml(s.category) + '</div>' +
     '<div style="font-size:0.78rem;color:var(--text-muted);margin-bottom:var(--space-lg);">' + escHtml(s.description) + '</div>' +
     '<a href="' + s.url + '" target="_blank" rel="noopener" style="display:inline-block;padding:var(--space-sm) var(--space-xl);background:var(--accent);color:#fff;border-radius:6px;text-decoration:none;font-size:0.75rem;font-weight:600;">Visit Store</a>' +
    '</div>';
  }).join('');
}

document.addEventListener('DOMContentLoaded', function() {
  marketConnect();
  renderMarketListings();
});
