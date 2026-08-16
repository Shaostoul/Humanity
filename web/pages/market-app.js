const API_BASE = '';
let marketWs = null;
let marketListings = [];
let marketMyKey = '';
let marketMyName = 'Visitor';
let marketMyRole = '';
/** Whether this identity's role may publish classifieds listings (set on peer_list). */
var marketCanList = false;
/** Which sub-section is showing; the Create Listing button follows it. */
var _marketSection = 'directory';
/** Cache of seller ratings: { [seller_key]: { avg: number, count: number } } */
const sellerRatings = {};
/** Cache of reviews per listing: { [listing_id]: ReviewData[] } */
const listingReviews = {};
/* Categorical badge palette for the need-shaped vocabulary in
 * /data/market/categories.json, keyed by lowercase id. Labels pass through
 * toLowerCase() so both sides of the id/label pair resolve. Mirrors the
 * native palette in src/gui/pages/market.rs::category_color. */
const CATEGORY_COLORS = {
  food:'#3c963c', water:'#3c91af', shelter:'#8b7765', energy:'#b48c32',
  health:'#be556e', care:'#c878a0', clothing:'#966ebe', tools:'#4682b4',
  materials:'#a08255', repair:'#cd7d3c', transport:'#8c50a0', growing:'#6eaa50',
  education:'#5a64c8', communication:'#3296c8', services:'#c86450',
  emergency:'#d24638', other:'#888'
};
function categoryColor(cat) {
  return CATEGORY_COLORS[String(cat || '').toLowerCase()] || '#888';
}

/* The live vocabulary, fetched from the same file the native app and the
 * relay validator read. The static <option> lists in market.html are only
 * the no-fetch fallback; this replaces them once loaded. */
var MARKET_CATEGORIES = [];
function fillCategorySelect(sel, allLabel) {
  if (!sel) return;
  var current = sel.value;
  sel.innerHTML = '';
  if (allLabel !== null) {
    var o0 = document.createElement('option');
    o0.value = '';
    o0.textContent = allLabel;
    sel.appendChild(o0);
  }
  MARKET_CATEGORIES.forEach(function(c) {
    var o = document.createElement('option');
    o.value = c.label;
    o.textContent = c.label;
    if (c.desc) o.title = c.desc;
    sel.appendChild(o);
  });
  if (current) sel.value = current;
}
function loadMarketCategories() {
  fetch('/data/market/categories.json', { cache: 'no-cache' })
    .then(function(r) { return r.json(); })
    .then(function(j) {
      if (!j || !Array.isArray(j.categories) || !j.categories.length) return;
      MARKET_CATEGORIES = j.categories;
      fillCategorySelect(document.getElementById('market-category-filter'), 'All Categories');
      fillCategorySelect(document.getElementById('store-category-filter'), 'All Categories');
      fillCategorySelect(document.getElementById('dir-category-filter'), 'All Categories');
      fillCategorySelect(document.getElementById('listing-category'), null);
      // The directory shows vocabulary LABELS for the ids in its payloads, so
      // redraw once the vocabulary lands (it may arrive after the first paint).
      renderDirectory();
    })
    .catch(function() { /* fallback: the static options in market.html */ });
}
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
  _marketSection = section;
  ['directory','marketplace','stores','mylistings'].forEach(function(s) {
    var el = document.getElementById('market-section-' + s);
    if (el) el.style.display = s === section ? '' : 'none';
    var btn = document.getElementById('market-nav-' + s);
    if (btn) { btn.classList.toggle('btn-clickable', s === section); }
  });
  // Create Listing publishes a classifieds listing, not a directory offering
  // (those are signed objects, published by the importer), so hide it here.
  var createBtn = document.getElementById('market-create-btn');
  if (createBtn) {
    createBtn.style.display = (section !== 'directory' && marketCanList) ? 'inline-flex' : 'none';
  }
  if (section === 'directory') loadDirectory(false);
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
    marketCanList = marketMyRole === 'admin' || marketMyRole === 'mod' || marketMyRole === 'verified' || marketMyRole === 'donor';
    var btn = document.getElementById('market-create-btn');
    var onDirectory = _marketSection === 'directory';
    if (btn) btn.style.display = (marketCanList && !onDirectory) ? 'inline-flex' : 'none';
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
  var catColor = categoryColor(l.category);
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
  var catColor = categoryColor(l.category);
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

/* ═══════════════════════════════════════════════════════════════════════════
 * Market Directory: the signed provider/offering catalog.
 *
 * Web mirror of the native Directory tab (src/gui/pages/market_directory.rs).
 * Every view here is a query over GET /api/v2/objects (object_type=provider_v1
 * / offering_v1). Payloads are canonical CBOR signed by the merchant's chat
 * identity, so LATEST-revision resolution happens client-side: an offering's
 * identity is (provider root, offering_key), a provider's identity is its root
 * object id, and the newest updated_at wins. Settlement is directory-only by
 * design: this view lists and introduces, it never moves money.
 * ═══════════════════════════════════════════════════════════════════════════ */

var DIRECTORY = {
  sub: 'offerings',        // 'offerings' | 'providers'
  providers: [],
  offerings: [],
  providerFilter: null,    // provider root_id, set by clicking a provider card
  loaded: false,
  loading: false,
  error: '',
};

/* canonical-cbor.js is an ES module and this file is a classic script, so the
 * bridge is a lazily cached dynamic import (the same pattern chat-groups-p2p.js
 * uses for pq-object.js). Nothing here races DOMContentLoaded: every caller
 * awaits this promise before touching the decoder. */
var _dirCborModule = null;
function directoryCbor() {
  if (!_dirCborModule) _dirCborModule = import('/shared/canonical-cbor.js');
  return _dirCborModule;
}

function dirB64Bytes(b64) {
  var bin = atob(String(b64 || ''));
  var out = new Uint8Array(bin.length);
  for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function dirHex(bytes) {
  var hex = '';
  for (var i = 0; i < bytes.length; i++) hex += bytes[i].toString(16).padStart(2, '0');
  return hex;
}

/* Field readers over a decoded CBOR map. Mirrors the native decoder's shapes:
 * a missing or wrong-typed field is absent, never a thrown error, because one
 * malformed row must not blank the whole directory. */
function dirText(m, k) { return m && typeof m[k] === 'string' ? m[k] : ''; }
function dirNum(m, k) {
  var v = m ? m[k] : undefined;
  if (typeof v === 'bigint') return Number(v);
  return typeof v === 'number' ? v : null;
}
function dirStrs(m, k) {
  var v = m ? m[k] : undefined;
  return Array.isArray(v) ? v.filter(function(x) { return typeof x === 'string'; }) : [];
}
function dirTable(m, k) {
  var v = m ? m[k] : undefined;
  return v && typeof v === 'object' && !Array.isArray(v) && !(v instanceof Uint8Array) ? v : null;
}

function dirDecodeProvider(row, cbor) {
  var p;
  try { p = cbor.decodeCanonicalCbor(dirB64Bytes(row.payload_b64)); } catch (e) { return null; }
  if (!p || typeof p !== 'object') return null;
  var name = dirText(p, 'display_name');
  if (!name) return null;
  var contact = dirTable(p, 'contact');
  return {
    // A provider UPDATE carries provider_ref back to the root it revises; the
    // first revision IS the root, so it has no provider_ref of its own.
    root_id: dirText(p, 'provider_ref') || String(row.object_id || ''),
    author_key_hex: row.author_public_key_b64 ? dirHex(dirB64Bytes(row.author_public_key_b64)) : '',
    display_name: name,
    kind: dirText(p, 'kind'),
    description: dirText(p, 'description'),
    status: dirText(p, 'status'),
    contact_preferred: contact ? dirText(contact, 'preferred') : '',
    website: contact ? dirText(contact, 'website') : '',
    updated_at: dirNum(p, 'updated_at') || 0,
  };
}

function dirDecodeOffering(row, cbor) {
  var o;
  try { o = cbor.decodeCanonicalCbor(dirB64Bytes(row.payload_b64)); } catch (e) { return null; }
  if (!o || typeof o !== 'object') return null;
  var providerRoot = dirText(o, 'provider_ref');
  var key = dirText(o, 'offering_key');
  var title = dirText(o, 'title');
  if (!providerRoot || !key || !title) return null;
  var price = dirTable(o, 'price');
  var settlement = dirTable(o, 'settlement');
  var good = dirTable(o, 'good');
  var service = dirTable(o, 'service');
  var availability = service ? dirTable(service, 'availability') : null;
  return {
    provider_root: providerRoot,
    offering_key: key,
    kind: dirText(o, 'kind'),
    reality: dirText(o, 'reality') || 'real',
    title: title,
    // schemas/offering.toml names this field `description` (required).
    summary: dirText(o, 'description') || dirText(o, 'summary'),
    category: dirText(o, 'category'),
    tags: dirStrs(o, 'tags'),
    status: dirText(o, 'status'),
    updated_at: dirNum(o, 'updated_at') || 0,
    expires_at: dirNum(o, 'expires_at'),
    ttl_days: dirNum(o, 'ttl_days') != null ? dirNum(o, 'ttl_days') : 30,
    fulfillment: dirStrs(o, 'fulfillment'),
    price_mode: price ? dirText(price, 'mode') : '',
    price_amount: price ? dirNum(price, 'amount') : null,
    price_amount_max: price ? dirNum(price, 'amount_max') : null,
    price_currency: price ? dirText(price, 'currency') : '',
    price_unit: price ? dirText(price, 'unit') : '',
    price_accepts: price ? dirStrs(price, 'accepts') : [],
    price_notes: price ? dirText(price, 'notes') : '',
    contact_via: settlement ? dirText(settlement, 'contact_via') : '',
    checkout_uri: settlement ? dirText(settlement, 'checkout_uri') : '',
    instructions: settlement ? dirText(settlement, 'instructions') : '',
    condition: good ? dirText(good, 'condition') : '',
    availability_mode: good ? dirText(good, 'availability_mode') : '',
    quantity_available: good ? dirNum(good, 'quantity_available') : null,
    lead_time_days: good ? dirNum(good, 'lead_time_days') : null,
    action: service ? dirText(service, 'action') : '',
    schedule_kind: availability ? dirText(availability, 'schedule_kind') : '',
    duration_minutes: service ? dirNum(service, 'duration_minutes') : null,
    location_mode: dirTable(o, 'location') ? dirText(dirTable(o, 'location'), 'mode') : '',
  };
}

/** Stable selection key for one offering. */
function dirSelKey(o) { return o.provider_root + '/' + o.offering_key; }

/* Hidden once past its explicit expiry, or past ttl_days since the last touch
 * (default 30, the schema's freshness contract: re-importing IS the keepalive,
 * so the directory never fills with ghosts). Timestamps are milliseconds. */
function dirExpired(o, nowMs) {
  if (o.expires_at != null) return o.expires_at <= nowMs;
  var ttl = Math.min(365, Math.max(1, o.ttl_days || 30));
  return o.updated_at + ttl * 86400000 <= nowMs;
}

/** Human price line: "Free", "24.99 USD each", "10.00 to 20.00 USD", "Ask". */
function dirPriceLine(o) {
  var unit = !o.price_unit ? ''
    : (o.price_unit === 'each' ? ' each' : ' per ' + o.price_unit.replace(/_/g, ' '));
  var a = o.price_amount;
  var b = o.price_amount_max;
  switch (o.price_mode) {
    case 'free': return 'Free';
    case 'trade': return 'Trade';
    case 'donation': return 'Donation';
    case 'inquire': return 'Ask';
    case 'pay_what_you_can': return 'Pay what you can';
    case 'sliding_scale':
      return (a != null && b != null)
        ? 'Sliding ' + a.toFixed(2) + ' to ' + b.toFixed(2) + ' ' + o.price_currency + unit
        : 'Sliding scale';
    case 'range':
      return (a != null && b != null)
        ? a.toFixed(2) + ' to ' + b.toFixed(2) + ' ' + o.price_currency + unit
        : 'Range';
    case 'fixed':
      return a != null ? a.toFixed(2) + ' ' + o.price_currency + unit : 'Priced';
    default:
      return String(o.price_mode || '').replace(/_/g, ' ');
  }
}

/** "today", "3d ago", "2mo ago" from a millisecond timestamp. */
function dirAge(updatedMs, nowMs) {
  var days = Math.floor(Math.max(0, nowMs - updatedMs) / 86400000);
  if (days === 0) return 'today';
  if (days <= 59) return days + 'd ago';
  return Math.floor(days / 30) + 'mo ago';
}

/** Enum ids ride the wire as snake_case; humans read them with spaces. */
function dirEnumLabel(s) { return String(s || '').replace(/_/g, ' '); }

/** Vocabulary label for a category id (falls back to the id itself). */
function dirCategoryLabel(id) {
  for (var i = 0; i < MARKET_CATEGORIES.length; i++) {
    if (MARKET_CATEGORIES[i].id === id) return MARKET_CATEGORIES[i].label;
  }
  return id;
}

/* The category <select> carries LABELS (shared with the classifieds filter),
 * while offering payloads carry the lowercase snake_case id, so map back. */
function dirCategoryId(label) {
  if (!label) return '';
  for (var i = 0; i < MARKET_CATEGORIES.length; i++) {
    if (MARKET_CATEGORIES[i].label === label) return MARKET_CATEGORIES[i].id;
  }
  return String(label).toLowerCase();
}

/** Escape a value for use inside a single-quoted JS string in an onclick. */
function dirAttr(s) {
  return escHtml(String(s == null ? '' : s).replace(/\\/g, '\\\\').replace(/'/g, "\\'"));
}

function dirProviderByRoot(root) {
  return DIRECTORY.providers.find(function(p) { return p.root_id === root; }) || null;
}

// ── Fetch + latest-revision resolution ──

function dirFetchRows(objectType) {
  return fetch(API_BASE + '/api/v2/objects?object_type=' + objectType + '&limit=500', { cache: 'no-cache' })
    .then(function(r) {
      if (!r.ok) throw new Error(objectType + ': HTTP ' + r.status);
      return r.json();
    })
    .then(function(j) { return Array.isArray(j) ? j : []; });
}

/** Cached: one round per object type on section entry, then only on Refresh. */
function loadDirectory(force) {
  if (DIRECTORY.loading) return;
  if (DIRECTORY.loaded && !force) { renderDirectory(); return; }
  DIRECTORY.loading = true;
  DIRECTORY.error = '';
  renderDirectory();
  Promise.all([directoryCbor(), dirFetchRows('provider_v1'), dirFetchRows('offering_v1')])
    .then(function(res) {
      var cbor = res[0];

      // Providers: newest revision per root id wins.
      var byRoot = {};
      res[1].forEach(function(row) {
        var p = dirDecodeProvider(row, cbor);
        if (!p || !p.root_id) return;
        var old = byRoot[p.root_id];
        if (!old || old.updated_at < p.updated_at) byRoot[p.root_id] = p;
      });

      // Offerings: newest revision per (provider root, offering_key) wins;
      // received_at breaks updated_at ties (a re-import touches freshness).
      var byKey = {};
      res[2].forEach(function(row) {
        var o = dirDecodeOffering(row, cbor);
        if (!o) return;
        var received = typeof row.received_at === 'number' ? row.received_at : 0;
        var k = o.provider_root + ' ' + o.offering_key;
        var old = byKey[k];
        if (!old || old.o.updated_at < o.updated_at
            || (old.o.updated_at === o.updated_at && old.received < received)) {
          byKey[k] = { received: received, o: o };
        }
      });

      var newest = function(a, b) { return b.updated_at - a.updated_at; };
      DIRECTORY.providers = Object.keys(byRoot).map(function(k) { return byRoot[k]; }).sort(newest);
      DIRECTORY.offerings = Object.keys(byKey).map(function(k) { return byKey[k].o; }).sort(newest);
      DIRECTORY.loaded = true;
      DIRECTORY.error = '';
    })
    .catch(function(e) {
      DIRECTORY.error = (e && e.message) ? e.message : String(e);
    })
    .then(function() {
      DIRECTORY.loading = false;
      renderDirectory();
    });
}

// ── Views ──

function setDirectorySub(sub) {
  DIRECTORY.sub = sub;
  renderDirectory();
}

function clearDirectoryProviderFilter() {
  DIRECTORY.providerFilter = null;
  renderDirectory();
}

/** Click a provider card: filter offerings down to that storefront. */
function showProviderStorefront(root) {
  DIRECTORY.providerFilter = root;
  DIRECTORY.sub = 'offerings';
  renderDirectory();
}

/** Active + unexpired + filtered, in the current sub-view's order. */
function directoryVisibleOfferings() {
  var now = Date.now();
  var searchEl = document.getElementById('dir-search');
  var catEl = document.getElementById('dir-category-filter');
  var needle = ((searchEl && searchEl.value) || '').trim().toLowerCase();
  var catId = dirCategoryId(catEl ? catEl.value : '');
  return DIRECTORY.offerings.filter(function(o) {
    if (o.status !== 'active') return false;
    if (dirExpired(o, now)) return false;
    if (catId && o.category !== catId) return false;
    if (DIRECTORY.providerFilter && o.provider_root !== DIRECTORY.providerFilter) return false;
    if (needle) {
      var hit = o.title.toLowerCase().indexOf(needle) !== -1
        || o.summary.toLowerCase().indexOf(needle) !== -1
        || o.tags.some(function(t) { return t.toLowerCase().indexOf(needle) !== -1; });
      if (!hit) return false;
    }
    return true;
  });
}

function directoryActiveOfferingCount(root) {
  var now = Date.now();
  return DIRECTORY.offerings.filter(function(o) {
    return o.provider_root === root && o.status === 'active' && !dirExpired(o, now);
  }).length;
}

function renderDirectory() {
  var grid = document.getElementById('dir-grid');
  if (!grid) return;
  var empty = document.getElementById('dir-empty');
  var status = document.getElementById('dir-status');
  var errEl = document.getElementById('dir-error');
  var filters = document.getElementById('dir-filters');
  var crumb = document.getElementById('dir-breadcrumb');
  var refresh = document.getElementById('dir-refresh-btn');

  ['offerings', 'providers'].forEach(function(s) {
    var b = document.getElementById('dir-sub-' + s);
    if (b) b.classList.toggle('btn-clickable', DIRECTORY.sub === s);
  });
  if (filters) filters.style.display = DIRECTORY.sub === 'offerings' ? '' : 'none';
  if (refresh) {
    refresh.disabled = DIRECTORY.loading;
    refresh.textContent = DIRECTORY.loading ? 'Loading...' : 'Refresh';
  }

  var plural = function(n, word) { return n === 1 ? '1 ' + word : n + ' ' + word + 's'; };
  if (status) {
    status.textContent = plural(DIRECTORY.offerings.length, 'offering') + ' from '
      + plural(DIRECTORY.providers.length, 'provider')
      + '. Listings and introductions only: money never moves through the platform.';
  }
  if (errEl) {
    errEl.style.display = DIRECTORY.error ? '' : 'none';
    errEl.textContent = DIRECTORY.error ? 'Fetch failed: ' + DIRECTORY.error : '';
  }
  if (crumb) {
    var filtered = DIRECTORY.providerFilter ? dirProviderByRoot(DIRECTORY.providerFilter) : null;
    crumb.innerHTML = filtered
      ? '<span class="dir-crumb">Storefront: ' + escHtml(filtered.display_name)
        + ' <button class="btn dir-toggle" onclick="clearDirectoryProviderFilter()">All providers</button></span>'
      : '';
  }

  if (DIRECTORY.sub === 'providers') renderDirectoryProviders(grid, empty);
  else renderDirectoryOfferings(grid, empty);
}

function renderDirectoryOfferings(grid, empty) {
  var rows = directoryVisibleOfferings();
  if (!rows.length) {
    grid.innerHTML = '';
    if (empty) {
      empty.style.display = '';
      empty.textContent = DIRECTORY.loading
        ? 'Loading directory...'
        : 'No offerings here yet. Publish yours with scripts/import-offerings.mjs (docs/admin/market-importer.md).';
    }
    return;
  }
  if (empty) empty.style.display = 'none';
  var now = Date.now();
  grid.innerHTML = rows.map(function(o) {
    var catColor = categoryColor(o.category);
    var provider = dirProviderByRoot(o.provider_root);
    var providerName = provider ? provider.display_name : 'Unknown provider';
    var kindNote = o.kind === 'service'
      ? 'service: ' + dirEnumLabel(o.action)
      : dirEnumLabel(o.condition);
    var sel = dirAttr(dirSelKey(o));
    var thumb = window.hosIcon ? hosIcon(o.kind === 'service' ? 'tool' : 'box', 28) : '';
    return '<div class="listing-card" role="button" tabindex="0"' +
      ' onclick="showOfferingDetail(\'' + sel + '\')"' +
      ' onkeydown="if(event.key===\'Enter\'){showOfferingDetail(\'' + sel + '\')}">' +
      '<div class="listing-thumb">' + thumb + '</div>' +
      '<div class="listing-body">' +
        '<div class="listing-head">' +
          '<span class="listing-title">' + escHtml(o.title) + '</span>' +
          '<span class="dir-age">' + escHtml(dirAge(o.updated_at, now)) + '</span>' +
        '</div>' +
        '<div class="listing-price">' + escHtml(dirPriceLine(o)) + '</div>' +
        '<div class="listing-chips">' +
          '<span class="cat-chip" style="--cat:' + catColor + ';--cat-bg:' + catColor + '22;">' +
            escHtml(dirCategoryLabel(o.category)) + '</span>' +
          (o.reality === 'sim' ? '<span class="cond-chip">sim</span>' : '') +
        '</div>' +
        '<div class="listing-seller">by ' + escHtml(providerName) + '</div>' +
        (kindNote.trim() ? '<div class="dir-kind">' + escHtml(kindNote) + '</div>' : '') +
        (o.summary ? '<div class="dir-summary">' + escHtml(o.summary) + '</div>' : '') +
      '</div>' +
    '</div>';
  }).join('');
}

function renderDirectoryProviders(grid, empty) {
  var rows = DIRECTORY.providers.filter(function(p) { return p.status === 'active'; });
  if (!rows.length) {
    grid.innerHTML = '';
    if (empty) {
      empty.style.display = '';
      empty.textContent = DIRECTORY.loading
        ? 'Loading directory...'
        : 'No providers yet. Publish yours with scripts/import-offerings.mjs (docs/admin/market-importer.md).';
    }
    return;
  }
  if (empty) empty.style.display = 'none';
  grid.innerHTML = rows.map(function(p) {
    var count = directoryActiveOfferingCount(p.root_id);
    var root = dirAttr(p.root_id);
    var icon = window.hosIcon ? hosIcon('storefront', 24) : '';
    return '<div class="store-card provider-card" role="button" tabindex="0"' +
      ' onclick="showProviderStorefront(\'' + root + '\')"' +
      ' onkeydown="if(event.key===\'Enter\'){showProviderStorefront(\'' + root + '\')}">' +
      '<div class="store-icon">' + icon + '</div>' +
      '<div class="store-name">' + escHtml(p.display_name) + '</div>' +
      '<div class="store-category">' + escHtml(dirEnumLabel(p.kind)) + '</div>' +
      '<div class="dir-provider-count">' + count + ' active offering' + (count === 1 ? '' : 's') + '</div>' +
      (p.description ? '<div class="store-desc">' + escHtml(p.description) + '</div>' : '') +
    '</div>';
  }).join('');
}

// ── Offering detail ──

function showOfferingDetail(selKey) {
  var o = DIRECTORY.offerings.find(function(x) { return dirSelKey(x) === selKey; });
  if (!o) return;
  var modal = document.getElementById('offering-detail-modal');
  var content = document.getElementById('offering-detail-content');
  if (!modal || !content) return;
  var provider = dirProviderByRoot(o.provider_root);
  var catColor = categoryColor(o.category);

  var facts = '';
  var fact = function(label, value) {
    if (value) facts += '<div class="dir-fact"><span>' + escHtml(label) + '</span> ' + escHtml(value) + '</div>';
  };
  if (o.kind === 'good') {
    fact('Condition:', dirEnumLabel(o.condition));
    var avail;
    if (o.availability_mode === 'in_stock' && o.quantity_available != null) {
      avail = o.quantity_available + ' in stock';
    } else if (o.availability_mode === 'one_off') {
      avail = 'one of a kind';
    } else if (o.availability_mode === 'made_to_order') {
      avail = o.lead_time_days != null
        ? 'made to order, ' + o.lead_time_days + ' day lead'
        : 'made to order';
    } else {
      avail = dirEnumLabel(o.availability_mode);
    }
    fact('Availability:', avail);
  } else {
    fact('Service:', dirEnumLabel(o.action));
    fact('Schedule:', dirEnumLabel(o.schedule_kind));
    if (o.duration_minutes != null) fact('Duration:', o.duration_minutes + ' min');
  }
  fact('Fulfillment:', o.fulfillment.map(dirEnumLabel).join(', '));
  fact('Location:', dirEnumLabel(o.location_mode));
  if (o.tags.length) fact('Tags:', o.tags.join(', '));
  fact('Updated:', dirAge(o.updated_at, Date.now()));

  var providerHtml;
  if (provider) {
    providerHtml =
      '<div class="dir-fact"><strong>' + escHtml(provider.display_name) + '</strong> ' +
        '<span>' + escHtml(dirEnumLabel(provider.kind)) + '</span></div>' +
      (provider.description ? '<div class="detail-desc">' + escHtml(provider.description) + '</div>' : '') +
      '<div class="dir-fact"><span>Contact:</span> ' + escHtml(dirEnumLabel(o.contact_via)) + '</div>' +
      (provider.website ? '<div class="dir-fact"><span>Website:</span> ' + escHtml(provider.website) + '</div>' : '') +
      (provider.author_key_hex
        ? '<div class="dir-fact"><span>Provider key (paste into Chat to send a direct message):</span></div>' +
          '<div class="dir-key" id="offering-provider-key">' + escHtml(provider.author_key_hex) + '</div>' +
          '<button class="btn btn-sm" id="offering-copy-key" onclick="copyProviderKey()">Copy provider key</button>'
        : '');
  } else {
    providerHtml = '<div class="detail-empty">Provider entry not found on this server.</div>';
  }

  content.innerHTML =
    '<button onclick="closeOfferingDetail()" class="detail-close" aria-label="Close offering details">' +
      (window.hosIcon ? hosIcon('close', 14) : 'x') + '</button>' +
    '<h3 class="detail-title">' + escHtml(o.title) + '</h3>' +
    '<div class="detail-chips">' +
      '<span class="cat-chip" style="--cat:' + catColor + ';--cat-bg:' + catColor + '22;">' +
        escHtml(dirCategoryLabel(o.category)) + '</span>' +
      (o.reality === 'sim' ? '<span class="cond-chip">sim</span>' : '') +
    '</div>' +
    '<div class="detail-price">' + escHtml(dirPriceLine(o)) + '</div>' +
    (o.price_accepts.length
      ? '<div class="detail-meta">Accepts: ' + escHtml(o.price_accepts.map(dirEnumLabel).join(', ')) + '</div>' : '') +
    (o.price_notes ? '<div class="detail-meta">' + escHtml(o.price_notes) + '</div>' : '') +
    (o.summary ? '<div class="detail-desc">' + escHtml(o.summary) + '</div>' : '') +
    '<div class="detail-section">' +
      '<h4 class="detail-section-title">Details</h4>' + facts +
    '</div>' +
    '<div class="detail-section">' +
      '<h4 class="detail-section-title">Provider</h4>' + providerHtml +
      (o.checkout_uri ? '<div class="dir-fact"><span>Checkout:</span> ' + escHtml(o.checkout_uri) + '</div>' : '') +
      (o.instructions ? '<div class="detail-desc">' + escHtml(o.instructions) + '</div>' : '') +
      '<div class="dir-note">Money never moves through the platform: contact the provider to ' +
        'arrange payment and handoff.</div>' +
    '</div>';

  modal.style.display = '';
}

function closeOfferingDetail() {
  var modal = document.getElementById('offering-detail-modal');
  if (modal) modal.style.display = 'none';
}

/* Copy the provider's Dilithium key. navigator.clipboard is undefined on
 * insecure origins (a http:// LAN mirror of this page), so keep the
 * execCommand fallback rather than leaving the button dead there. */
function copyProviderKey() {
  var keyEl = document.getElementById('offering-provider-key');
  var btn = document.getElementById('offering-copy-key');
  if (!keyEl) return;
  var hex = keyEl.textContent || '';
  var report = function(ok) {
    if (!btn) return;
    btn.textContent = ok ? 'Copied' : 'Copy failed, select the key above';
    setTimeout(function() { btn.textContent = 'Copy provider key'; }, 2000);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(hex).then(
      function() { report(true); },
      function() { report(dirLegacyCopy(hex)); }
    );
  } else {
    report(dirLegacyCopy(hex));
  }
}

function dirLegacyCopy(text) {
  try {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    var ok = document.execCommand('copy');
    document.body.removeChild(ta);
    return ok;
  } catch (e) {
    return false;
  }
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
  loadMarketCategories();
  marketConnect();
  renderMarketListings();
  // Directory is the landing section, mirroring the native page's default tab.
  showMarketSection('directory');
});
