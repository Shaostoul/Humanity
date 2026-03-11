// ── Profile System ──
// Goal: manage local profile storage, the Edit Profile modal, the View Profile
// overlay, and the client-side block list.
// Depends on (from app.js): ws, myKey, myName, esc, generateIdenticon,
//   roleBadge, peerData, isFriend, isFollowing, myFollowing, myFollowers,
//   addSystemMessage, reRenderMessagesForBlockChange, rerenderUserList.

/** name (lowercase) → { bio, socials, avatar_url, banner_url, pronouns, location, website } */
let profileCache = {};
let lastProfileUpdateSent = 0;
let pendingProfileView = null; // name we're waiting for profile_data on
/** per-field privacy state while the edit modal is open: field → 'private' | 'public' */
let editPrivacyMap = {};

/** Persist the full profile object to localStorage for offline pre-fill. */
function saveProfileLocal(data) {
  localStorage.setItem('humanity_profile', JSON.stringify(data));
}
/** Load the locally cached profile object. */
function loadProfileLocal() {
  try {
    return JSON.parse(localStorage.getItem('humanity_profile') || '{}');
  } catch { return {}; }
}

/**
 * Toggle the privacy state of a profile field between public and private.
 * Called by the lock-icon button beside each privacy-capable field.
 * @param {string} field - The field name (e.g. 'pronouns', 'location', 'website').
 */
function togglePrivacyField(field) {
  const isPrivate = editPrivacyMap[field] === 'private';
  editPrivacyMap[field] = isPrivate ? 'public' : 'private';
  const btn = document.getElementById('privacy-' + field);
  if (btn) {
    btn.textContent = editPrivacyMap[field] === 'private' ? '🔒' : '🌐';
    btn.classList.toggle('is-private', editPrivacyMap[field] === 'private');
    btn.title = editPrivacyMap[field] === 'private' ? 'Visible to friends only — click to make public' : 'Visible to everyone — click to make private';
  }
}

// ── Edit Profile Modal ──
/**
 * Open the Edit Profile modal and pre-fill all fields from local storage.
 * Also resets the per-field privacy toggles to match the saved privacy map.
 */
function openEditProfileModal() {
  const overlay = document.getElementById('edit-profile-overlay');
  const local = loadProfileLocal();
  const socials = local.socials || {};

  // Core fields.
  document.getElementById('profile-bio').value = local.bio || '';
  document.getElementById('profile-avatar-url').value = local.avatar_url || '';
  document.getElementById('profile-banner-url').value = local.banner_url || '';
  document.getElementById('profile-pronouns').value = local.pronouns || '';
  document.getElementById('profile-location').value = local.location || '';
  document.getElementById('profile-website-url').value = local.website || '';

  // Social handles (stored inside the socials object).
  document.getElementById('profile-website').value = socials.website || '';
  document.getElementById('profile-discord').value = socials.discord || '';
  document.getElementById('profile-twitter').value = socials.twitter || '';
  document.getElementById('profile-youtube').value = socials.youtube || '';
  document.getElementById('profile-github').value = socials.github || '';

  // Restore privacy toggles.
  editPrivacyMap = Object.assign({}, local.privacy || {});
  for (const field of ['pronouns', 'location', 'website']) {
    const isPrivate = editPrivacyMap[field] === 'private';
    const btn = document.getElementById('privacy-' + field);
    if (btn) {
      btn.textContent = isPrivate ? '🔒' : '🌐';
      btn.classList.toggle('is-private', isPrivate);
    }
  }

  updateBioCounter();
  overlay.classList.add('open');
}

function closeEditProfileModal(e) {
  if (e.target === document.getElementById('edit-profile-overlay')) {
    closeEditProfileOverlay();
  }
}
function closeEditProfileOverlay() {
  document.getElementById('edit-profile-overlay').classList.remove('open');
}

function updateBioCounter() {
  const bio = document.getElementById('profile-bio').value;
  const counter = document.getElementById('bio-counter');
  counter.textContent = bio.length + ' / 280';
  counter.className = 'bio-counter' + (bio.length > 280 ? ' over' : bio.length > 240 ? ' warn' : '');
}

document.getElementById('profile-bio').addEventListener('input', updateBioCounter);

/**
 * Read all profile modal fields, save locally, and push to the server.
 * Includes the new extended fields (avatar, banner, pronouns, location, website)
 * along with the per-field privacy map collected from the lock-icon toggles.
 */
function saveProfile() {
  const bio = document.getElementById('profile-bio').value.trim().substring(0, 280);
  const avatar_url = document.getElementById('profile-avatar-url').value.trim().substring(0, 512);
  const banner_url = document.getElementById('profile-banner-url').value.trim().substring(0, 512);
  const pronouns   = document.getElementById('profile-pronouns').value.trim().substring(0, 64);
  const location   = document.getElementById('profile-location').value.trim().substring(0, 128);
  const website    = document.getElementById('profile-website-url').value.trim().substring(0, 256);

  const socials = {
    website: document.getElementById('profile-website').value.trim().substring(0, 200),
    discord: document.getElementById('profile-discord').value.trim().substring(0, 100),
    twitter: document.getElementById('profile-twitter').value.trim().substring(0, 100),
    youtube: document.getElementById('profile-youtube').value.trim().substring(0, 200),
    github:  document.getElementById('profile-github').value.trim().substring(0, 200),
  };

  // Strip empty socials fields before serialising.
  const cleanSocials = {};
  for (const [k, v] of Object.entries(socials)) {
    if (v) cleanSocials[k] = v;
  }

  // Build a clean privacy map: only include fields that are explicitly set to private.
  const privacyMap = {};
  for (const [field, state] of Object.entries(editPrivacyMap)) {
    if (state === 'private') privacyMap[field] = 'private';
  }

  // Save all fields locally so the modal pre-fills correctly next time.
  saveProfileLocal({ bio, socials: cleanSocials, avatar_url, banner_url, pronouns, location, website, privacy: privacyMap });

  // Push to server.
  if (ws && ws.readyState === WebSocket.OPEN) {
    const now = Date.now();
    if (now - lastProfileUpdateSent < 30000) {
      addSystemMessage('⏳ Please wait 30 seconds between profile updates.');
    } else {
      lastProfileUpdateSent = now;
      ws.send(JSON.stringify({
        type: 'profile_update',
        bio,
        socials: JSON.stringify(cleanSocials),
        avatar_url: avatar_url || undefined,
        banner_url: banner_url || undefined,
        pronouns:   pronouns   || undefined,
        location:   location   || undefined,
        website:    website    || undefined,
        privacy:    JSON.stringify(privacyMap),
      }));
      addSystemMessage('Profile saved.');
    }
  } else {
    addSystemMessage('Profile saved locally. It will sync when you connect.');
  }

  closeEditProfileOverlay();
}

/**
 * Push locally cached profile data to the server on connect so the server
 * has the latest version after a page reload or new device login.
 */
function syncProfileOnConnect() {
  const local = loadProfileLocal();
  const hasData = local.bio
    || (local.socials && Object.keys(local.socials).length > 0)
    || local.avatar_url || local.banner_url
    || local.pronouns  || local.location || local.website;
  if (!hasData) return;

  ws.send(JSON.stringify({
    type: 'profile_update',
    bio:        local.bio        || '',
    socials:    JSON.stringify(local.socials || {}),
    avatar_url: local.avatar_url || undefined,
    banner_url: local.banner_url || undefined,
    pronouns:   local.pronouns   || undefined,
    location:   local.location   || undefined,
    website:    local.website    || undefined,
    privacy:    JSON.stringify(local.privacy || {}),
  }));
  lastProfileUpdateSent = Date.now();
}

// ── View Profile Modal ──
/**
 * Show the view-profile overlay for a given user, fetching from the server
 * if their profile isn't already cached locally.
 * @param {string} name      - Display name
 * @param {string} publicKey - Ed25519 public key hex
 */
function requestViewProfile(name, publicKey) {
  pendingProfileView = { name, publicKey };
  // Check cache first — pass the full cached profile object.
  const cached = profileCache[name.toLowerCase()];
  if (cached) {
    showViewProfileCard(name, publicKey, cached);
    return;
  }
  // Request from server.
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'profile_request', name: name }));
    // Show loading state.
    document.getElementById('view-profile-content').innerHTML =
      '<div style="color:var(--text-muted);font-style:italic;">Loading profile…</div>';
    document.getElementById('view-profile-overlay').classList.add('open');
  }
}

/**
 * Renders a read-only profile card in the view-profile overlay.
 * Accepts a profile object (all fields optional) so callers pass profileCache[name]
 * directly; fields absent from the server response (privacy-filtered) are simply
 * not rendered — no placeholder text shown.
 *
 * @param {string} name      - Display name of the profile owner
 * @param {string} publicKey - Ed25519 public key hex (used for identicon + follow actions)
 * @param {object} profile   - Profile data: { bio, socials, avatar_url, banner_url, pronouns, location, website }
 */
function showViewProfileCard(name, publicKey, profile) {
  // Accept either the new object form or the legacy (bio, socialsStr) positional args.
  if (typeof profile === 'string') {
    // Legacy call: showViewProfileCard(name, key, bio, socialsStr) — 4th arg is socialsStr.
    // eslint-disable-next-line prefer-rest-params
    profile = { bio: profile, socials: arguments[3] || '{}' };
  }
  profile = profile || {};
  const bio        = profile.bio        || '';
  const avatarUrl  = profile.avatar_url || '';
  const bannerUrl  = profile.banner_url || '';
  const pronouns   = profile.pronouns   || '';
  const location   = profile.location   || '';
  const website    = profile.website    || '';
  let socials = {};
  try { socials = JSON.parse(profile.socials || '{}'); } catch {}

  const isBot = publicKey && publicKey.startsWith('bot_');
  const identiconSrc = !isBot && publicKey ? generateIdenticon(publicKey, 64) : '';

  // Banner strip — shown only when the user has set one.
  let html = '';
  if (bannerUrl) {
    html += '<div class="profile-card-banner" style="background-image:url(' + esc(bannerUrl) + ')"></div>';
  }

  html += '<div class="profile-card-header">';
  // Avatar: prefer user-set image, fall back to identicon, then bot emoji.
  if (avatarUrl) {
    html += '<img src="' + esc(avatarUrl) + '" class="profile-card-avatar" alt="">';
  } else if (isBot) {
    html += '<span class="identicon-large" style="font-size:48px;line-height:64px;display:inline-block;width:64px;text-align:center;">🤖</span>';
  } else if (identiconSrc) {
    html += '<img src="' + identiconSrc + '" class="identicon-large" alt="">';
  }

  // Look up role badge.
  const peerRole = (peerData[publicKey] && peerData[publicKey].role) ? peerData[publicKey].role : '';
  const badge = roleBadge(peerRole);

  html += '<div>';
  html += '<div class="profile-name">' + esc(name) + badge + '</div>';
  if (pronouns) {
    html += '<span class="profile-card-badge">' + esc(pronouns) + '</span>';
  }
  html += '</div>';
  html += '</div>'; // .profile-card-header

  const hasBio     = bio.trim().length > 0;
  const hasLocation = location.trim().length > 0;
  const hasWebsite  = website.trim().length > 0;
  const hasSocials  = Object.values(socials).some(v => v && v.trim());
  const hasAnything = hasBio || hasLocation || hasWebsite || hasSocials;

  if (!hasAnything) {
    html += '<div class="profile-card-empty">This user hasn\'t set up their profile yet.</div>';
  } else {
    if (hasBio) {
      html += '<div class="profile-card-bio">' + esc(bio) + '</div>';
    }
    // Location shown inline (privacy-filtered fields simply won't be present in profile).
    if (hasLocation) {
      html += '<div class="profile-card-socials"><div class="social-item"><span class="social-label">📍 Location</span> ' + esc(location) + '</div></div>';
    }
    if (hasWebsite || hasSocials) {
      html += '<div class="profile-card-socials">';
      if (hasWebsite) {
        if (website.startsWith('https://')) {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> <a href="' + esc(website) + '" target="_blank" rel="noopener">' + esc(website) + '</a></div>';
        } else {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> ' + esc(website) + '</div>';
        }
      }
      if (socials.website) {
        const url = socials.website;
        if (url.startsWith('https://')) {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> <a href="' + esc(url) + '" target="_blank" rel="noopener">' + esc(url) + '</a></div>';
        } else {
          html += '<div class="social-item"><span class="social-label">🌐 Website</span> ' + esc(url) + '</div>';
        }
      }
      if (socials.discord) {
        html += '<div class="social-item"><span class="social-label">💬 Discord</span> ' + esc(socials.discord) + '</div>';
      }
      if (socials.twitter) {
        const handle = socials.twitter.replace(/^@/, '');
        html += '<div class="social-item"><span class="social-label">𝕏 Twitter</span> <a href="https://x.com/' + esc(handle) + '" target="_blank" rel="noopener">@' + esc(handle) + '</a></div>';
      }
      if (socials.youtube) {
        const yt = socials.youtube;
        if (yt.startsWith('https://')) {
          html += '<div class="social-item"><span class="social-label">▶️ YouTube</span> <a href="' + esc(yt) + '" target="_blank" rel="noopener">' + esc(yt) + '</a></div>';
        } else {
          const ytUrl = 'https://youtube.com/@' + yt;
          html += '<div class="social-item"><span class="social-label">▶️ YouTube</span> <a href="' + esc(ytUrl) + '" target="_blank" rel="noopener">@' + esc(yt) + '</a></div>';
        }
      }
      if (socials.github) {
        const gh = socials.github.replace(/^@/, '');
        html += '<div class="social-item"><span class="social-label">🐙 GitHub</span> <a href="https://github.com/' + esc(gh) + '" target="_blank" rel="noopener">' + esc(gh) + '</a></div>';
      }
      html += '</div>';
    }
  }

  // Public key (click to copy) — use DOM API instead of inline onclick.
  if (publicKey) {
    const shortPk = publicKey.length > 24 ? publicKey.substring(0, 24) + '…' : publicKey;
    html += '<div class="profile-card-key" id="profile-pk-copy" title="Click to copy full key">🔑 ' + esc(shortPk) + '</div>';
  }

  // Follow/friend status + button
  if (publicKey && publicKey !== myKey) {
    const friend = isFriend(publicKey);
    const following = isFollowing(publicKey);
    const followsYou = myFollowers.has(publicKey);
    let statusText = '';
    if (friend) statusText = '🤝 Friends (mutual follow)';
    else if (following && followsYou) statusText = '🤝 Friends';
    else if (following) statusText = '👁️ You follow this user';
    else if (followsYou) statusText = '👁️‍🗨️ Follows you';
    const btnLabel = following ? '❌ Unfollow' : '👁️ Follow';
    html += '<div style="margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid var(--border);">';
    if (statusText) html += '<div style="font-size:0.75rem;color:var(--text-muted);margin-bottom:0.3rem;">' + statusText + '</div>';
    html += '<button id="profile-follow-btn" style="background:var(--accent);color:#fff;border:none;border-radius:6px;padding:0.3rem 0.8rem;font-size:0.78rem;cursor:pointer;">' + btnLabel + '</button>';
    html += '</div>';
  }

  document.getElementById('view-profile-content').innerHTML = html;
  // Attach click handler via DOM API (not inline onclick).
  if (publicKey) {
    const pkEl = document.getElementById('profile-pk-copy');
    if (pkEl) {
      pkEl.addEventListener('click', () => {
        navigator.clipboard.writeText(publicKey).then(() => addSystemMessage('Public key copied.'));
      });
    }
  }
  // Follow button handler
  if (publicKey && publicKey !== myKey) {
    const followBtn = document.getElementById('profile-follow-btn');
    if (followBtn) {
      followBtn.addEventListener('click', () => {
        if (ws && ws.readyState === WebSocket.OPEN) {
          const type = myFollowing.has(publicKey) ? 'unfollow' : 'follow';
          ws.send(JSON.stringify({ type, target_key: publicKey }));
          closeViewProfileOverlay();
        }
      });
    }
  }
  if (window.twemoji) twemoji.parse(document.getElementById('view-profile-content'));
  document.getElementById('view-profile-overlay').classList.add('open');
}

function closeViewProfileModal(e) {
  if (e.target === document.getElementById('view-profile-overlay')) {
    closeViewProfileOverlay();
  }
}
function closeViewProfileOverlay() {
  document.getElementById('view-profile-overlay').classList.remove('open');
  pendingProfileView = null;
}

// ── Block List (client-side) ──
// Stores blocked usernames in localStorage; messages from blocked users are hidden
// client-side without any server interaction (server never knows about blocks).
function getBlockList() {
  try { return JSON.parse(localStorage.getItem('humanity_blocks') || '[]'); }
  catch { return []; }
}
function setBlockList(list) {
  localStorage.setItem('humanity_blocks', JSON.stringify(list));
}
function isBlocked(name) {
  return getBlockList().some(b => b.toLowerCase() === name.toLowerCase());
}

function blockUser(name) {
  if (name.toLowerCase() === myName.toLowerCase()) {
    addSystemMessage("You can't block yourself.");
    return;
  }
  const list = getBlockList();
  if (list.some(b => b.toLowerCase() === name.toLowerCase())) {
    addSystemMessage(`${name} is already blocked.`);
    return;
  }
  list.push(name);
  setBlockList(list);
  addSystemMessage(`🚫 Blocked ${name}. Their messages are now hidden.`);
  reRenderMessagesForBlockChange();
  rerenderUserList();
}

function unblockUser(name) {
  const list = getBlockList();
  const idx = list.findIndex(b => b.toLowerCase() === name.toLowerCase());
  if (idx === -1) {
    addSystemMessage(`${name} is not blocked.`);
    return;
  }
  list.splice(idx, 1);
  setBlockList(list);
  addSystemMessage(`✅ Unblocked ${name}.`);
  reRenderMessagesForBlockChange();
  rerenderUserList();
}

function showBlockList() {
  const list = getBlockList();
  if (list.length === 0) {
    addSystemMessage('No blocked users.');
  } else {
    addSystemMessage('🚫 Blocked users: ' + list.join(', '));
  }
}

/** Re-filter visible messages after a block/unblock change. */
function reRenderMessagesForBlockChange() {
  const container = document.getElementById('messages');
  const msgs = container.querySelectorAll('.message[data-from]');
  msgs.forEach(el => {
    const authorEl = el.querySelector('.author');
    if (!authorEl) return;
    const authorName = authorEl.dataset.username;
    if (authorName && isBlocked(authorName)) {
      el.style.display = 'none';
    } else {
      el.style.display = '';
    }
  });
}

/** Force re-render user list with updated block indicators. */
function rerenderUserList() {
  const list = document.getElementById('peer-list');
  const peers = list.querySelectorAll('.peer[data-username]');
  peers.forEach(el => {
    const name = el.dataset.username;
    if (!name) return;
    const blocked = isBlocked(name);
    let indicator = el.querySelector('.block-indicator');
    if (blocked && !indicator) {
      const span = document.createElement('span');
      span.className = 'block-indicator';
      span.textContent = ' 🚫';
      span.title = 'Blocked';
      span.style.fontSize = '0.65rem';
      el.appendChild(span);
      el.style.textDecoration = 'line-through';
      el.style.opacity = '0.5';
    } else if (!blocked && indicator) {
      indicator.remove();
      el.style.textDecoration = '';
      if (el.style.opacity === '') el.removeAttribute('style');
    }
  });
}
