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

/** Render star display: filled stars + muted outline stars.
 *  All styling lives in market.html (.stars / .star); the size arg only picks a
 *  size class, so stars follow the theme (dark/light/compact). */
function renderStars(rating, size) {
  size = size || 14;
  const val = Number(rating) || 0;
  const sizeCls = size >= 18 ? ' stars-lg' : (size >= 14 ? '' : (size >= 12 ? ' stars-sm' : ' stars-xs'));
  const full = Math.round(val);
  let html = '<span class="stars' + sizeCls + '" role="img" aria-label="Rated ' + val.toFixed(1) + ' out of 5">';
  for (let i = 0; i < 5; i++) {
    if (i < full) {
      html += '<span class="star on" aria-hidden="true">&#9733;</span>';
    } else {
      html += '<span class="star" aria-hidden="true">&#9734;</span>';
    }
  }
  html += '</span>';
  return html;
}

/** Render clickable star selector for review form. */
function renderStarSelector(currentRating) {
  let html = '<div id="review-star-selector" class="star-selector" role="group" aria-label="Rating">';
  for (let i = 1; i <= 5; i++) {
    const filled = i <= currentRating;
    html += '<span onclick="setReviewRating(' + i + ')" onkeydown="if(event.key===\'Enter\'||event.key===\' \'){event.preventDefault();setReviewRating(' + i + ');}" onmouseenter="previewStars(' + i + ')" onmouseleave="previewStars(0)" data-star="' + i + '" class="star' + (filled ? ' on' : '') + '" role="button" tabindex="0" aria-label="' + i + ' star' + (i === 1 ? '' : 's') + '">&#9733;</span>';
  }
  html += '</div>';
  return html;
}

let _reviewRating = 0;

function setReviewRating(n) {
  _reviewRating = n;
  const stars = document.querySelectorAll('#review-star-selector span');
  stars.forEach(function(s, i) { s.classList.toggle('on', i < n); });
}

function previewStars(n) {
  if (n === 0) { setReviewRating(_reviewRating); return; }
  const stars = document.querySelectorAll('#review-star-selector span');
  stars.forEach(function(s, i) { s.classList.toggle('on', i < n); });
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
  } else if (msg.type === 'listing_messages') {
    if (msg.listing_id) {
      listingMsgs[msg.listing_id] = msg.messages || [];
      if (_detailListingId === msg.listing_id) renderListingMessages(msg.listing_id);
    }
  } else if (msg.type === 'listing_message_new') {
    if (msg.listing_id && msg.message) {
      if (!listingMsgs[msg.listing_id]) listingMsgs[msg.listing_id] = [];
      listingMsgs[msg.listing_id].push(msg.message);
      if (_detailListingId === msg.listing_id) renderListingMessages(msg.listing_id);
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
  document.getElementById('listing-title').classList.remove('input-error');
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
    ? '<div class="detail-rating">' + renderStars(sr.avg) + '<span class="detail-rating-count">' + sr.avg.toFixed(1) + ' (' + sr.count + ' review' + (sr.count !== 1 ? 's' : '') + ')</span></div>'
    : '';

  content.innerHTML =
    '<button onclick="closeListingDetail()" class="detail-close" aria-label="Close listing details">' + hosIcon('close', 14) + '</button>' +
    '<h3 class="detail-title">' + escHtml(l.title) + '</h3>' +
    '<div class="detail-price">' + escHtml(l.price || 'Contact for price') + '</div>' +
    '<div class="detail-chips">' +
      '<span class="cat-chip" style="--cat:' + catColor + ';--cat-bg:' + catColor + '22;">' + escHtml(l.category) + '</span>' +
      (l.condition && l.condition !== 'N/A' ? '<span class="cond-chip">' + escHtml(l.condition) + '</span>' : '') +
    '</div>' +
    (l.description ? '<div class="detail-desc">' + escHtml(l.description) + '</div>' : '') +
    '<div class="detail-meta">Seller: <strong>' + escHtml(l.seller_name || 'Anonymous') + '</strong></div>' +
    sellerRatingHtml +
    (l.payment_methods ? '<div class="detail-meta">Payment: ' + escHtml(l.payment_methods) + '</div>' : '') +
    (l.location ? '<div class="detail-meta detail-location">' + hosIcon('mappin', 14) + ' ' + escHtml(l.location) + '</div>' : '') +
    (isMine ? '<div class="detail-actions">' +
      '<button onclick="editListing(\'' + l.id + '\');closeListingDetail()" class="detail-action">' + hosIcon('edit', 14) + ' Edit</button>' +
      '<button onclick="markListingSold(\'' + l.id + '\');closeListingDetail()" class="detail-action is-sold">' + hosIcon('check', 14) + ' Mark Sold</button>' +
      '<button onclick="deleteListing(\'' + l.id + '\');closeListingDetail()" class="detail-action is-delete">' + hosIcon('trash', 14) + ' Delete</button>' +
    '</div>' : '') +
    '<div id="listing-messages-section" class="detail-section">' +
      '<h4 class="detail-section-title">Messages</h4>' +
      '<div id="listing-messages-list" class="lmsg-list"></div>' +
      '<div id="listing-messages-empty" class="detail-empty">No messages yet. Ask a question or start a conversation.</div>' +
      (marketMyKey && marketMyKey.indexOf('viewer_') !== 0 ?
        '<div class="lmsg-compose">' +
          '<input id="listing-msg-input" type="text" maxlength="2000" class="lmsg-input" aria-label="Message the seller" placeholder="Type a message..." onkeydown="if(event.key===\'Enter\')sendListingMessage(\'' + escHtml(l.id) + '\')">' +
          '<button onclick="sendListingMessage(\'' + escHtml(l.id) + '\')" class="btn btn-clickable btn-sm">Send</button>' +
        '</div>'
      : '<div class="detail-empty">Sign in to send messages.</div>') +
    '</div>' +
    '<div id="listing-reviews-section" class="detail-section">' +
      '<h4 class="detail-section-title">Reviews</h4>' +
      '<div id="listing-reviews-loading" class="detail-empty">Loading reviews...</div>' +
      '<div id="listing-reviews-list"></div>' +
    '</div>';

  modal.style.display = '';
  fetchListingReviews(id).then(function(data) { renderDetailReviews(id, data); });
  requestListingMessages(id);
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
    html += '<div class="review-summary">' +
      renderStars(avg, 18) +
      '<span class="review-avg">' + avg.toFixed(1) + '</span>' +
      '<span class="review-count">(' + count + ' review' + (count !== 1 ? 's' : '') + ')</span>' +
    '</div>';
  }

  if (reviews.length > 1) {
    html += '<div class="review-sort">' +
      '<select id="review-sort-select" aria-label="Sort reviews" onchange="resortReviews(\'' + escHtml(listingId) + '\')">' +
        '<option value="newest"' + (sortVal === 'newest' ? ' selected' : '') + '>Newest</option>' +
        '<option value="highest"' + (sortVal === 'highest' ? ' selected' : '') + '>Highest</option>' +
        '<option value="lowest"' + (sortVal === 'lowest' ? ' selected' : '') + '>Lowest</option>' +
      '</select>' +
    '</div>';
  }

  if (canReview) {
    html += '<div class="review-form">' +
      '<div class="review-form-title">Write a Review</div>' +
      '<div class="review-field">' +
        '<label class="review-label">Rating</label>' +
        renderStarSelector(0) +
      '</div>' +
      '<div class="review-field">' +
        '<label class="review-label" for="review-comment">Comment</label>' +
        '<textarea id="review-comment" class="review-comment" rows="3" maxlength="2000" placeholder="Share your experience..."></textarea>' +
      '</div>' +
      '<button onclick="submitReview(\'' + escHtml(listingId) + '\')" class="btn btn-clickable btn-sm">Submit Review</button>' +
      '<div id="review-error" class="review-error" role="alert" style="display:none;"></div>' +
    '</div>';
  }

  if (sortedReviews.length === 0) {
    html += '<div class="reviews-empty">No reviews yet.</div>';
  } else {
    sortedReviews.forEach(function(r) {
      var canDelete = r.reviewer_key === marketMyKey || isAdmin;
      var dateStr = r.created_at ? new Date(r.created_at).toLocaleDateString() : '';
      html += '<div class="review-row">' +
        '<div class="review-row-head">' +
          '<div>' +
            '<strong class="review-author">' + escHtml(r.reviewer_name || 'Anonymous') + '</strong>' +
            '<span class="review-stars">' + renderStars(r.rating, 12) + '</span>' +
          '</div>' +
          '<div class="review-meta">' +
            '<span class="review-date">' + escHtml(dateStr) + '</span>' +
            (canDelete ? '<button onclick="deleteReview(\'' + escHtml(listingId) + '\',' + r.id + ')" class="review-delete" title="Delete review" aria-label="Delete review">' + hosIcon('trash', 12) + '</button>' : '') +
          '</div>' +
        '</div>' +
        (r.comment ? '<div class="review-body">' + escHtml(r.comment) + '</div>' : '') +
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
    ? '<div class="listing-rating">' + renderStars(sr.avg, 11) + '<span class="listing-rating-count">(' + sr.count + ')</span></div>'
    : '';
  var actions = (isMine || showActions) ?
    '<div class="listing-actions">' +
     (isMine ? '<button onclick="event.stopPropagation();editListing(\'' + l.id + '\')" class="listing-action">' + hosIcon('edit', 14) + ' Edit</button>' : '') +
     (isMine ? '<button onclick="event.stopPropagation();markListingSold(\'' + l.id + '\')" class="listing-action is-sold">' + hosIcon('check', 14) + ' Sold</button>' : '') +
     ((isMine||isAdmin) ? '<button onclick="event.stopPropagation();deleteListing(\'' + l.id + '\')" class="listing-action is-delete">' + hosIcon('trash', 14) + ' Delete</button>' : '') +
    '</div>' : '';
  var statusBadge = l.status === 'sold' ? '<span class="status-badge">SOLD</span>' :
            l.status === 'withdrawn' ? '<span class="status-badge withdrawn">WITHDRAWN</span>' : '';
  return '<div class="listing-card" role="button" tabindex="0" onclick="showListingDetail(\'' + l.id + '\')" onkeydown="if(event.key===\'Enter\'){showListingDetail(\'' + l.id + '\')}">' +
    '<div class="listing-thumb">' + (l.category === '3D Models' ? '&#129482;' : '&#128230;') + '</div>' +
    '<div class="listing-body">' +
      '<div class="listing-head">' +
       '<span class="listing-title">' + escHtml(l.title) + statusBadge + '</span>' +
      '</div>' +
      '<div class="listing-price">' + escHtml(l.price || 'Contact for price') + '</div>' +
      '<div class="listing-chips">' +
       '<span class="cat-chip" style="--cat:' + catColor + ';--cat-bg:' + catColor + '22;">' + escHtml(l.category) + '</span>' +
       (l.condition && l.condition !== 'N/A' ? '<span class="cond-chip">' + escHtml(l.condition) + '</span>' : '') +
      '</div>' +
      '<div class="listing-seller">by ' + escHtml(l.seller_name || 'Anonymous') + '</div>' +
      ratingHtml +
      (l.location ? '<div class="listing-location">' + hosIcon('mappin', 14) + ' ' + escHtml(l.location) + '</div>' : '') +
      actions +
    '</div>' +
  '</div>';
}

function closeListingModal() {
  document.getElementById('listing-modal').style.display = 'none';
}

function submitListing() {
  var titleEl = document.getElementById('listing-title');
  var title = titleEl.value.trim();
  titleEl.classList.remove('input-error');
  if (!title) { titleEl.classList.add('input-error'); titleEl.focus(); return; }
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
  if (!filtered.length) { grid.innerHTML = '<div class="stores-empty">No stores listed yet.</div>'; return; }
  grid.innerHTML = filtered.map(function(s) {
    return '<div class="store-card">' +
     '<div class="store-icon">' + s.icon + '</div>' +
     '<div class="store-name">' + escHtml(s.name) + '</div>' +
     '<div class="store-category">' + escHtml(s.category) + '</div>' +
     '<div class="store-desc">' + escHtml(s.description) + '</div>' +
     '<a href="' + s.url + '" target="_blank" rel="noopener" class="store-link">Visit Store</a>' +
    '</div>';
  }).join('');
}

// ── Listing Messages (buyer-seller conversations) ──

/** Cache of listing messages: { [listing_id]: MessageData[] } */
var listingMsgs = {};

/** Request message history for a listing */
function requestListingMessages(listingId) {
  if (marketWs && marketWs.readyState === 1) {
    marketWs.send(JSON.stringify({ type: 'listing_message_history', listing_id: listingId }));
  }
}

/** Send a message on a listing */
function sendListingMessage(listingId) {
  var input = document.getElementById('listing-msg-input');
  if (!input) return;
  var content = input.value.trim();
  if (!content) return;
  if (marketWs && marketWs.readyState === 1) {
    marketWs.send(JSON.stringify({
      type: 'listing_message_send',
      listing_id: listingId,
      content: content,
    }));
  }
  input.value = '';
}

/** Render listing messages inside the detail modal */
function renderListingMessages(listingId) {
  var container = document.getElementById('listing-messages-list');
  var emptyEl = document.getElementById('listing-messages-empty');
  if (!container) return;
  var msgs = listingMsgs[listingId] || [];
  if (msgs.length === 0) {
    container.innerHTML = '';
    if (emptyEl) emptyEl.style.display = '';
    return;
  }
  if (emptyEl) emptyEl.style.display = 'none';
  container.innerHTML = msgs.map(function(m) {
    var isMine = m.sender_key === marketMyKey;
    var time = new Date(m.timestamp).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
    return '<div class="lmsg' + (isMine ? ' mine' : '') + '">' +
      '<div class="lmsg-head">' +
        '<strong class="lmsg-author">' + escHtml(m.sender_name || 'Anonymous') + '</strong>' +
        '<span class="lmsg-time">' + escHtml(time) + '</span>' +
      '</div>' +
      '<div class="lmsg-body">' + escHtml(m.content) + '</div>' +
    '</div>';
  }).join('');
  container.scrollTop = container.scrollHeight;
}

document.addEventListener('DOMContentLoaded', function() {
  marketConnect();
  renderMarketListings();
});
