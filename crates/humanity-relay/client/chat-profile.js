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
  // Merge any pending sync from the standalone profile.html page.
  let local = loadProfileLocal();
  try {
    const pending = JSON.parse(localStorage.getItem('humanity_profile_pending_sync') || 'null');
    if (pending) {
      if (pending.bio)        local.bio        = pending.bio;
      if (pending.avatar_url) local.avatar_url = pending.avatar_url;
      if (pending.banner_url) local.banner_url = pending.banner_url;
      if (pending.pronouns)   local.pronouns   = pending.pronouns;
      if (pending.location)   local.location   = pending.location;
      if (pending.website)    local.website    = pending.website;
      saveProfileLocal(local);
      localStorage.removeItem('humanity_profile_pending_sync');
    }
  } catch (e) { /* ignore parse errors */ }

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
    html += '<div style="display:flex;gap:0.4rem;flex-wrap:wrap">';
    html += '<button id="profile-follow-btn" style="background:var(--accent);color:#fff;border:none;border-radius:6px;padding:0.3rem 0.8rem;font-size:0.78rem;cursor:pointer;">' + btnLabel + '</button>';
    html += '<button id="profile-endorse-btn" style="background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:6px;padding:0.3rem 0.8rem;font-size:0.78rem;cursor:pointer;" title="Ask this user to verify one of your skills">🏅 Ask to Endorse</button>';
    html += '</div></div>';
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
  // Endorse skill button — prompts for skill ID + level, then sends request to the peer
  if (publicKey && publicKey !== myKey) {
    const endorseBtn = document.getElementById('profile-endorse-btn');
    if (endorseBtn) {
      endorseBtn.addEventListener('click', () => {
        const skillId = prompt(`Ask ${name} to endorse which of your skills? (Enter skill ID, e.g. "Cooking", "Coding")`);
        if (!skillId || !skillId.trim()) return;
        const levelStr = prompt(`Your current level in "${skillId}":`, '1');
        const level = parseInt(levelStr, 10) || 1;
        if (typeof requestSkillEndorsement === 'function') {
          requestSkillEndorsement(name, skillId.trim(), level);
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

// ── Seed Phrase (BIP39) UI ──
// Goal: let users back up and restore their Ed25519 identity using a standard
// 24-word BIP39 mnemonic — writeable on paper, hardware-wallet compatible.

/**
 * Open a modal showing the identity as a 24-word BIP39 mnemonic.
 * Words are derived deterministically from the 32-byte Ed25519 seed via
 * SHA-256 checksum → 264 bits → 24×11-bit word indices.
 * The user can copy the phrase or write it on paper for offline recovery.
 */
async function openSeedPhraseModal() {
  let mnemonic;
  try {
    mnemonic = await generateMnemonic();
  } catch (e) {
    addSystemMessage('⚠️ Seed phrase unavailable — ' + e.message);
    return;
  }
  if (!mnemonic) {
    addSystemMessage('⚠️ Seed phrase unavailable — key may be non-extractable.');
    return;
  }

  const words = mnemonic.trim().split(/\s+/);
  const overlay = document.createElement('div');
  overlay.id = 'seed-phrase-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';

  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:600px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0;max-height:90vh;overflow-y:auto">
      <h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 .3rem">🌱 Identity Seed Phrase (24 words)</h2>
      <p style="font-size:.76rem;color:#888;line-height:1.5;margin:0 0 .8rem">
        These 24 words <em>are</em> your identity — anyone who has them can use your account.
        Store at least one copy somewhere safe. <strong style="color:#e55">Never photograph this screen.</strong>
      </p>

      <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:.4rem;margin-bottom:.9rem">
        ${words.map((w, i) => `
          <div style="background:#0f0f0f;border:1px solid #2a2a2a;border-radius:7px;padding:.4rem .55rem;display:flex;align-items:baseline;gap:.3rem">
            <span style="font-size:.6rem;color:#444;min-width:16px;text-align:right">${i+1}.</span>
            <span style="font-size:.86rem;color:#f0a500;font-weight:600">${w}</span>
          </div>`).join('')}
      </div>

      <p style="font-size:.7rem;color:#555;margin:0 0 .6rem">Pick at least one storage method:</p>

      <div style="display:grid;gap:.5rem;margin-bottom:1rem">
        <!-- Paper -->
        <div style="background:#0f0f0f;border:1px solid #222;border-radius:8px;padding:.7rem .9rem;display:flex;align-items:center;justify-content:space-between;gap:.75rem;flex-wrap:wrap">
          <div>
            <p style="font-size:.8rem;color:#e0e0e0;font-weight:600;margin:0 0 .15rem">📝 Paper — write it down</p>
            <p style="font-size:.72rem;color:#555;margin:0">Offline. Can't be hacked. Fireproof box or safe.</p>
          </div>
          <div style="display:flex;align-items:center;gap:.5rem">
            <button id="sp-copy-btn" style="background:none;border:1px solid #333;color:#aaa;border-radius:6px;padding:.3rem .8rem;font-size:.75rem;cursor:pointer">📋 Copy</button>
            <span id="sp-copy-msg" style="font-size:.68rem;color:#4ec87a"></span>
          </div>
        </div>

        <!-- Encrypted file -->
        <div style="background:#0f0f0f;border:1px solid #222;border-radius:8px;padding:.7rem .9rem">
          <p style="font-size:.8rem;color:#e0e0e0;font-weight:600;margin:0 0 .15rem">💾 Encrypted file — store in cloud</p>
          <p style="font-size:.72rem;color:#555;margin:0 0 .5rem">Lock the words with a passphrase → download a tiny file → store in Google Drive, Dropbox, etc. Useless without the passphrase, so keep them separate.</p>
          <div style="display:flex;gap:.4rem;align-items:center;flex-wrap:wrap">
            <input id="sp-enc-pass" type="password" placeholder="Choose a passphrase (8+ chars)…" autocomplete="new-password"
              style="flex:1;min-width:150px;background:#111;border:1px solid #2a2a2a;border-radius:6px;padding:.3rem .6rem;color:#e0e0e0;font-size:.76rem;outline:none">
            <button id="sp-enc-btn" style="background:none;border:1px solid #333;color:#aaa;border-radius:6px;padding:.3rem .8rem;font-size:.75rem;cursor:pointer;white-space:nowrap">💾 Download</button>
          </div>
          <span id="sp-enc-msg" style="font-size:.7rem;color:#4ec87a;display:block;margin-top:.3rem;min-height:1em"></span>
        </div>

        <!-- Password manager -->
        <div style="background:#0f0f0f;border:1px solid #222;border-radius:8px;padding:.7rem .9rem;display:flex;align-items:center;justify-content:space-between;gap:.75rem;flex-wrap:wrap">
          <div>
            <p style="font-size:.8rem;color:#e0e0e0;font-weight:600;margin:0 0 .15rem">🔐 Password manager Secure Note</p>
            <p style="font-size:.72rem;color:#555;margin:0">Copy → paste into <strong style="color:#777">Bitwarden</strong> or <strong style="color:#777">1Password</strong> as a Secure Note. Syncs everywhere.</p>
          </div>
          <div style="display:flex;align-items:center;gap:.5rem;flex-shrink:0">
            <button id="sp-pm-btn" style="background:none;border:1px solid #333;color:#aaa;border-radius:6px;padding:.3rem .8rem;font-size:.75rem;cursor:pointer">📋 Copy</button>
            <span id="sp-pm-msg" style="font-size:.68rem;color:#4ec87a"></span>
          </div>
        </div>
      </div>

      <p style="font-size:.66rem;color:#3a3a3a;margin:0 0 .9rem">
        Identity: <code style="color:#555">${(window.myIdentity && myIdentity.publicKeyHex || '').slice(0,20)}…</code>
      </p>
      <div style="display:flex;justify-content:flex-end">
        <button onclick="document.getElementById('seed-phrase-overlay').remove()"
          style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:.45rem 1.4rem;font-size:.82rem;font-weight:700;cursor:pointer">Done</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });

  // Wire copy buttons and encrypted download
  const _mn = mnemonic;
  overlay.querySelector('#sp-copy-btn').addEventListener('click', () => {
    navigator.clipboard.writeText(_mn).then(() => {
      overlay.querySelector('#sp-copy-msg').textContent = '✓ Copied';
      overlay.querySelector('#sp-copy-btn').textContent = 'Copied!';
    }).catch(() => { overlay.querySelector('#sp-copy-msg').textContent = 'Failed'; });
  });

  overlay.querySelector('#sp-pm-btn').addEventListener('click', () => {
    navigator.clipboard.writeText(_mn).then(() => {
      overlay.querySelector('#sp-pm-msg').textContent = '✓ Copied — paste into a Secure Note';
      overlay.querySelector('#sp-pm-btn').textContent = 'Copied!';
    }).catch(() => { overlay.querySelector('#sp-pm-msg').textContent = 'Failed'; });
  });

  overlay.querySelector('#sp-enc-btn').addEventListener('click', async () => {
    const pass = overlay.querySelector('#sp-enc-pass').value.trim();
    const encMsg = overlay.querySelector('#sp-enc-msg');
    const encBtn = overlay.querySelector('#sp-enc-btn');
    if (pass.length < 8) { encMsg.innerHTML = '<span style="color:#e55">Passphrase must be at least 8 characters.</span>'; return; }
    encBtn.disabled = true; encBtn.textContent = 'Encrypting…'; encMsg.textContent = '';
    try {
      await downloadEncryptedMnemonic(_mn, pass);
      encMsg.textContent = '✓ Downloaded — store the file in cloud, passphrase stays in your head.';
      encBtn.textContent = 'Downloaded!';
    } catch(e) {
      encMsg.innerHTML = `<span style="color:#e55">${e.message}</span>`;
      encBtn.disabled = false; encBtn.textContent = '💾 Download';
    }
  });
}

/**
 * Open the restore-from-mnemonic modal.
 * User pastes or types their 24 BIP39 words; on submit calls
 * restoreIdentityFromMnemonic() which validates the checksum, rebuilds the
 * Ed25519 keypair, stores it, then reloads to reconnect as the restored identity.
 */
function openRestoreFromMnemonicModal() {
  const overlay = document.createElement('div');
  overlay.id = 'restore-mnemonic-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.85);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';

  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:540px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0;max-height:90vh;overflow-y:auto">
      <h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin:0 0 .3rem">🌱 Restore from Seed Phrase</h2>
      <p style="font-size:.78rem;color:#888;line-height:1.5;margin:0 0 .9rem">
        <strong style="color:#e55">This will permanently replace your current identity on this device.</strong>
        Use one of the two methods below:
      </p>

      <!-- Tab: type words -->
      <div style="border:1px solid #2a2a2a;border-radius:9px;padding:.9rem 1rem;margin-bottom:.6rem">
        <p style="font-size:.82rem;color:#e0e0e0;font-weight:600;margin:0 0 .4rem">✍️ Type or paste your 24 words</p>
        <textarea id="rm-words" rows="3" placeholder="word1 word2 word3 … word24" autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
          style="width:100%;background:#0f0f0f;border:1px solid #2a2a2a;border-radius:7px;padding:.55rem .7rem;color:#e0e0e0;font-size:.85rem;font-family:'Courier New',monospace;resize:vertical;outline:none;box-sizing:border-box;line-height:1.6"></textarea>
        <div id="rm-word-count" style="font-size:.7rem;color:#555;margin:.3rem 0 0">0 / 24 words</div>
      </div>

      <!-- Tab: decrypt encrypted file -->
      <div style="border:1px solid #2a2a2a;border-radius:9px;padding:.9rem 1rem;margin-bottom:.9rem">
        <p style="font-size:.82rem;color:#e0e0e0;font-weight:600;margin:0 0 .25rem">💾 Restore from encrypted phrase file</p>
        <p style="font-size:.72rem;color:#555;margin:0 0 .5rem">If you saved a <code>humanity-phrase-backup.json</code> earlier, upload it here with the passphrase you chose.</p>
        <div style="display:flex;gap:.4rem;align-items:center;flex-wrap:wrap">
          <input id="rm-file" type="file" accept=".json,application/json"
            style="flex:1;min-width:120px;background:#0f0f0f;border:1px solid #2a2a2a;border-radius:6px;padding:.3rem .5rem;color:#aaa;font-size:.74rem;cursor:pointer">
          <input id="rm-file-pass" type="password" placeholder="Passphrase…" autocomplete="current-password"
            style="flex:1;min-width:110px;background:#0f0f0f;border:1px solid #2a2a2a;border-radius:6px;padding:.3rem .6rem;color:#e0e0e0;font-size:.76rem;outline:none">
          <button id="rm-file-btn"
            style="background:none;border:1px solid #333;color:#aaa;border-radius:6px;padding:.3rem .8rem;font-size:.74rem;cursor:pointer;white-space:nowrap">Decrypt</button>
        </div>
        <div id="rm-file-msg" style="font-size:.7rem;color:#4ec87a;min-height:1em;margin-top:.3rem"></div>
      </div>

      <div id="rm-msg" style="font-size:.75rem;min-height:1.2em;margin-bottom:.7rem"></div>
      <div style="display:flex;gap:.75rem;justify-content:flex-end">
        <button onclick="document.getElementById('restore-mnemonic-overlay').remove()"
          style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:.45rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="rm-btn" onclick="doRestoreFromMnemonic()"
          style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:.45rem 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Restore Identity</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });

  const ta = document.getElementById('rm-words');
  const counter = document.getElementById('rm-word-count');
  ta.addEventListener('input', () => {
    const count = ta.value.trim().split(/\s+/).filter(Boolean).length;
    counter.textContent = `${count} / 24 words`;
    counter.style.color = count === 24 ? '#4ec87a' : '#555';
  });
  ta.focus();

  // Wire the encrypted-file decrypt button — decrypts and fills the textarea.
  document.getElementById('rm-file-btn').addEventListener('click', async () => {
    const fileInput = document.getElementById('rm-file');
    const pass      = document.getElementById('rm-file-pass').value;
    const fileMsg   = document.getElementById('rm-file-msg');
    if (!fileInput.files.length) { fileMsg.innerHTML = '<span style="color:#e55">Select a file first.</span>'; return; }
    if (!pass) { fileMsg.innerHTML = '<span style="color:#e55">Enter your passphrase.</span>'; return; }
    try {
      const text   = await fileInput.files[0].text();
      const blob   = JSON.parse(text);
      const words  = await decryptMnemonic(blob, pass);
      ta.value = words;
      ta.dispatchEvent(new Event('input'));
      fileMsg.textContent = '✓ Decrypted — verify the words above, then click Restore Identity.';
    } catch(e) {
      fileMsg.innerHTML = `<span style="color:#e55">⚠ ${e.message}</span>`;
    }
  });
}

async function doRestoreFromMnemonic() {
  const ta  = document.getElementById('rm-words');
  const msg = document.getElementById('rm-msg');
  const btn = document.getElementById('rm-btn');
  const mnemonic = ta.value.trim().toLowerCase().replace(/\s+/g, ' ');

  const wordCount = mnemonic.split(' ').filter(Boolean).length;
  if (wordCount !== 24) {
    msg.innerHTML = `<span style="color:#e55">Expected 24 words, got ${wordCount}. Check for extra spaces or missing words.</span>`;
    return;
  }

  btn.disabled = true; btn.textContent = 'Restoring…'; msg.innerHTML = '';

  try {
    const identity = await restoreIdentityFromMnemonic(mnemonic);
    msg.innerHTML = `<span style="color:#4ec87a">✓ Identity restored! Public key: <code>${identity.publicKeyHex.slice(0,16)}…</code><br>Reloading in 2 seconds…</span>`;
    setTimeout(() => location.reload(), 2000);
  } catch (e) {
    msg.innerHTML = `<span style="color:#e55">⚠ ${e.message}</span>`;
    btn.disabled = false; btn.textContent = 'Restore Identity';
  }
}

// ── Identity Backup / Restore UI ──
// Goal: give users a secure, frictionless way to protect and recover their
// cryptographic identity from loss of device or browser data clear.

/**
 * Open the encrypted backup modal. Prompts user for a passphrase then downloads
 * an AES-256-GCM encrypted identity backup file they can store anywhere.
 */
function openEncryptedBackupModal() {
  const overlay = document.createElement('div');
  overlay.id = 'encrypted-backup-overlay';
  overlay.style.cssText = `
    position:fixed;inset:0;background:rgba(0,0,0,.75);z-index:6000;
    display:flex;align-items:center;justify-content:center;
  `;
  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:480px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0">
      <h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin-bottom:.5rem">🔑 Encrypted Identity Backup</h2>
      <p style="font-size:.82rem;color:#888;line-height:1.6;margin-bottom:1.25rem">
        Choose a passphrase to protect your backup. Anyone with the file AND passphrase can use your identity —
        so keep them <strong style="color:#e0e0e0">separate</strong> (file in cloud, passphrase memorised or in password manager).
      </p>
      <div style="margin-bottom:.9rem">
        <label style="display:block;font-size:.72rem;font-weight:600;color:#888;text-transform:uppercase;letter-spacing:.05em;margin-bottom:.3rem">Passphrase</label>
        <input id="eb-passphrase" type="password" placeholder="At least 8 characters…" autocomplete="new-password"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.85rem;outline:none">
      </div>
      <div style="margin-bottom:1.25rem">
        <label style="display:block;font-size:.72rem;font-weight:600;color:#888;text-transform:uppercase;letter-spacing:.05em;margin-bottom:.3rem">Confirm Passphrase</label>
        <input id="eb-passphrase2" type="password" placeholder="Repeat passphrase…" autocomplete="new-password"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.85rem;outline:none">
      </div>
      <div id="eb-msg" style="font-size:.75rem;margin-bottom:.9rem"></div>
      <div style="display:flex;gap:.75rem;justify-content:flex-end">
        <button onclick="this.closest('#encrypted-backup-overlay').remove()"
          style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:.45rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="eb-btn" onclick="doEncryptedBackup()"
          style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:.45rem 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Download Encrypted Backup</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('eb-passphrase').focus();
}

async function doEncryptedBackup() {
  const p1 = document.getElementById('eb-passphrase').value;
  const p2 = document.getElementById('eb-passphrase2').value;
  const msg = document.getElementById('eb-msg');

  if (p1.length < 8) { msg.innerHTML = '<span style="color:#e55">Passphrase must be at least 8 characters.</span>'; return; }
  if (p1 !== p2)     { msg.innerHTML = '<span style="color:#e55">Passphrases do not match.</span>'; return; }

  const btn = document.getElementById('eb-btn');
  btn.disabled = true; btn.textContent = 'Encrypting…';
  msg.innerHTML = '';

  try {
    await exportEncryptedIdentityBackup(p1);
    msg.innerHTML = '<span style="color:#4ec87a">✓ Backup downloaded. Keep the file and passphrase safe — separately.</span>';
    btn.textContent = 'Done';
    setTimeout(() => document.getElementById('encrypted-backup-overlay')?.remove(), 2500);
  } catch (e) {
    msg.innerHTML = `<span style="color:#e55">Error: ${e.message}</span>`;
    btn.disabled = false; btn.textContent = 'Download Encrypted Backup';
  }
}

/**
 * Open the restore identity modal. Accepts a file upload (plain or encrypted)
 * and optionally a passphrase for encrypted backups.
 */
function openRestoreIdentityModal() {
  const overlay = document.createElement('div');
  overlay.id = 'restore-identity-overlay';
  overlay.style.cssText = `
    position:fixed;inset:0;background:rgba(0,0,0,.75);z-index:6000;
    display:flex;align-items:center;justify-content:center;
  `;
  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:480px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0">
      <h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin-bottom:.5rem">📥 Restore Identity</h2>
      <p style="font-size:.82rem;color:#888;line-height:1.6;margin-bottom:1.25rem">
        Upload your identity backup file. If it was encrypted, enter the passphrase you used when creating it.
        <strong style="color:#e55">This will replace your current identity.</strong>
      </p>
      <div style="margin-bottom:.9rem">
        <label style="display:block;font-size:.72rem;font-weight:600;color:#888;text-transform:uppercase;letter-spacing:.05em;margin-bottom:.3rem">Backup File (.json)</label>
        <input id="ri-file" type="file" accept=".json,application/json"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.45rem .75rem;color:#e0e0e0;font-size:.82rem;cursor:pointer">
      </div>
      <div style="margin-bottom:1.25rem">
        <label style="display:block;font-size:.72rem;font-weight:600;color:#888;text-transform:uppercase;letter-spacing:.05em;margin-bottom:.3rem">Passphrase (if encrypted)</label>
        <input id="ri-passphrase" type="password" placeholder="Leave blank for plain backups…" autocomplete="current-password"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.85rem;outline:none">
      </div>
      <div id="ri-msg" style="font-size:.75rem;margin-bottom:.9rem"></div>
      <div style="display:flex;gap:.75rem;justify-content:flex-end">
        <button onclick="this.closest('#restore-identity-overlay').remove()"
          style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:.45rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="ri-btn" onclick="doRestoreIdentity()"
          style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:.45rem 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Restore Identity</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
}

async function doRestoreIdentity() {
  const fileInput = document.getElementById('ri-file');
  const passphrase = document.getElementById('ri-passphrase').value;
  const msg = document.getElementById('ri-msg');
  const btn = document.getElementById('ri-btn');

  if (!fileInput.files.length) { msg.innerHTML = '<span style="color:#e55">Please select a backup file.</span>'; return; }

  btn.disabled = true; btn.textContent = 'Restoring…'; msg.innerHTML = '';

  try {
    const text = await fileInput.files[0].text();
    const parsed = JSON.parse(text);
    const result = await importIdentityBackup(parsed, passphrase || undefined);

    msg.innerHTML = `<span style="color:#4ec87a">✓ Identity restored for <strong>${result.name}</strong>. Reloading…</span>`;
    setTimeout(() => location.reload(), 1800);
  } catch (e) {
    msg.innerHTML = `<span style="color:#e55">Error: ${e.message}</span>`;
    btn.disabled = false; btn.textContent = 'Restore Identity';
  }
}

// ── Passphrase Key Protection UI ──
// Goal: let users wrap their in-browser key with a passphrase so the raw private
// key material isn't sitting in plaintext localStorage.

/**
 * Open the key-protection modal. Shows current status and lets the user
 * enable, change, or (with care) remove passphrase protection.
 */
function openKeyProtectionModal() {
  const wrapped = isKeyWrapped();
  const overlay = document.createElement('div');
  overlay.id = 'key-protection-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.8);z-index:6000;display:flex;align-items:center;justify-content:center;';
  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #2a2a2a;border-radius:14px;padding:1.75rem;width:100%;max-width:500px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0">
      <h2 style="font-size:1rem;font-weight:700;color:#f0a500;margin-bottom:.5rem">🔒 Key Protection</h2>
      <div style="font-size:.78rem;color:#888;line-height:1.6;margin-bottom:1.1rem">
        ${wrapped
          ? `<span style="color:#4ec87a;font-weight:600">✅ Protected</span> — your private key in localStorage is encrypted with a passphrase. It is safe even if someone accesses your browser storage.`
          : `<span style="color:#f0a500;font-weight:600">⚠️ Not protected</span> — your private key is stored as readable plaintext in your browser's <code style="color:#ccc">localStorage</code>. Anyone with DevTools access, a malicious browser extension, or physical access to your browser profile directory could extract it. Set a passphrase to encrypt it at rest.`
        }
      </div>
      <div style="margin-bottom:.8rem">
        <label style="display:block;font-size:.72rem;font-weight:600;color:#888;text-transform:uppercase;letter-spacing:.05em;margin-bottom:.3rem">${wrapped ? 'New passphrase' : 'Set passphrase'}</label>
        <input id="kp-pass1" type="password" placeholder="At least 8 characters…" autocomplete="new-password"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.85rem;outline:none;margin-bottom:.5rem">
        <input id="kp-pass2" type="password" placeholder="Confirm passphrase…" autocomplete="new-password"
          style="width:100%;background:#111;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.85rem;outline:none">
      </div>
      <div id="kp-msg" style="font-size:.75rem;margin-bottom:.85rem;min-height:1.2em"></div>
      <div style="display:flex;gap:.5rem;flex-wrap:wrap;justify-content:flex-end">
        <button onclick="document.getElementById('key-protection-overlay').remove()"
          style="background:none;border:1px solid #333;color:#888;border-radius:7px;padding:.4rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        ${wrapped ? `<button id="kp-remove-btn" onclick="doRemoveKeyProtection()"
          style="background:none;border:1px solid #c44;color:#c44;border-radius:7px;padding:.4rem 1rem;font-size:.82rem;cursor:pointer"
          title="Remove passphrase protection — key will be stored in plaintext again">Remove Protection</button>` : ''}
        <button id="kp-save-btn" onclick="doEnableKeyProtection()"
          style="background:#f0a500;color:#000;border:none;border-radius:7px;padding:.4rem 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">
          ${wrapped ? 'Change Passphrase' : 'Protect Key'}</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('kp-pass1').focus();
}

async function doEnableKeyProtection() {
  const p1  = document.getElementById('kp-pass1').value;
  const p2  = document.getElementById('kp-pass2').value;
  const msg = document.getElementById('kp-msg');
  const btn = document.getElementById('kp-save-btn');
  if (p1.length < 8) { msg.innerHTML = '<span style="color:#e55">Passphrase must be at least 8 characters.</span>'; return; }
  if (p1 !== p2)     { msg.innerHTML = '<span style="color:#e55">Passphrases do not match.</span>'; return; }
  btn.disabled = true; btn.textContent = 'Encrypting…'; msg.innerHTML = '';
  try {
    await wrapAndStoreKey(p1);
    // Update sidebar status immediately.
    const protBtn = document.getElementById('key-protect-btn');
    if (protBtn) { protBtn.textContent = '🔒 Protected'; protBtn.style.color = '#4ec87a'; }
    // Show success briefly then close.
    msg.innerHTML = '<span style="color:#4ec87a">✅ Key encrypted with your passphrase.</span>';
    btn.textContent = 'Done ✓'; btn.disabled = false;
    // Change onclick to close instead of re-running protection.
    btn.onclick = () => document.getElementById('key-protection-overlay').remove();
    // Also update the remove-protection button text to be accurate.
    const removeBtn = document.getElementById('kp-remove-btn');
    if (removeBtn) removeBtn.title = 'Remove passphrase encryption — key reverts to plaintext in localStorage';
  } catch(e) {
    msg.innerHTML = `<span style="color:#e55">Error: ${e.message}</span>`;
    btn.disabled = false; btn.textContent = 'Protect Key';
  }
}

function doRemoveKeyProtection() {
  if (!confirm('Remove passphrase protection? Your private key will be stored in plaintext in localStorage again.')) return;
  try {
    localStorage.removeItem(WRAPPED_KEY_LS);
    localStorage.removeItem(WRAPPED_ECDH_LS);
    const msg = document.getElementById('kp-msg');
    if (msg) msg.innerHTML = '<span style="color:#f0a500">⚠️ Protection removed. Key is now stored in plaintext.</span>';
    const protBtn = document.getElementById('key-protect-btn');
    if (protBtn) { protBtn.textContent = '🔓 Protect Key'; protBtn.style.color = ''; }
  } catch(e) {}
}

// ── Key Rotation UI ───────────────────────────────────────────────────────────
// Goal: let a user generate a new Ed25519 identity that cryptographically
// inherits their old one. Both keys sign a rotation certificate so peers know
// the change was authorised — not an impersonation.

/**
 * Open the key rotation modal.
 * Rotation is a serious, irreversible action: explains the consequences in
 * plain language before letting the user proceed.
 */
function openKeyRotationModal() {
  const overlay = document.createElement('div');
  overlay.id = 'key-rotation-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,.88);z-index:6000;display:flex;align-items:center;justify-content:center;padding:1rem;box-sizing:border-box;';
  overlay.innerHTML = `
    <div style="background:#181818;border:1px solid #3a1515;border-radius:14px;padding:1.75rem;width:100%;max-width:520px;font-family:'Segoe UI',system-ui,sans-serif;color:#e0e0e0">
      <h2 style="font-size:1rem;font-weight:700;color:#e55;margin:0 0 .4rem">🔄 Rotate Identity Key</h2>
      <p style="font-size:.8rem;color:#888;line-height:1.55;margin:0 0 .9rem">
        This generates a <strong style="color:#e0e0e0">brand new identity</strong> and signs a certificate proving
        it was authorised by your current key. Peers who see the rotation will know the new key is yours.
      </p>
      <div style="background:#100000;border:1px solid #3a1515;border-radius:8px;padding:.8rem .9rem;margin-bottom:1rem;font-size:.78rem;color:#c44;line-height:1.55">
        ⚠ <strong>This is permanent.</strong> Your old key will be marked as rotated.<br>
        Back up your current seed phrase <em>before</em> rotating — you may need it to prove ownership later.<br>
        Followers and friends linked to your old key will need to update their contact list.
      </div>
      <div style="margin-bottom:1rem">
        <label style="display:block;font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.05em;color:#555;margin-bottom:.35rem">Type ROTATE to confirm</label>
        <input id="kr-confirm" type="text" placeholder="ROTATE" autocomplete="off"
          style="width:100%;background:#0d0d0d;border:1px solid #2a2a2a;border-radius:7px;padding:.5rem .75rem;color:#e0e0e0;font-size:.88rem;outline:none">
      </div>
      <div id="kr-msg" style="font-size:.75rem;min-height:1.2em;margin-bottom:.75rem"></div>
      <div style="display:flex;gap:.75rem;justify-content:flex-end">
        <button onclick="document.getElementById('key-rotation-overlay').remove()"
          style="background:none;border:1px solid #2a2a2a;color:#666;border-radius:7px;padding:.45rem 1rem;font-size:.82rem;cursor:pointer">Cancel</button>
        <button id="kr-btn" onclick="doKeyRotation()"
          style="background:#c44;color:#fff;border:none;border-radius:7px;padding:.45rem 1.2rem;font-size:.82rem;font-weight:700;cursor:pointer">Rotate Key</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('kr-confirm').focus();
}

async function doKeyRotation() {
  const confirm = document.getElementById('kr-confirm').value.trim();
  const msg     = document.getElementById('kr-msg');
  const btn     = document.getElementById('kr-btn');

  if (confirm !== 'ROTATE') {
    msg.innerHTML = '<span style="color:#e55">Type ROTATE (all caps) to confirm.</span>';
    return;
  }
  if (!myIdentity || !myIdentity.canSign) {
    msg.innerHTML = '<span style="color:#e55">Current identity is not signable — cannot rotate.</span>';
    return;
  }

  btn.disabled = true; btn.textContent = 'Generating…'; msg.textContent = '';

  try {
    // 1. Generate the new keypair
    const newKp  = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
    const rawPub = await crypto.subtle.exportKey('raw', newKp.publicKey);
    const newKeyHex = Array.from(new Uint8Array(rawPub)).map(b => b.toString(16).padStart(2,'0')).join('');

    const ts = Date.now();

    // 2. Sign with OLD key: sign(new_key + "\n" + timestamp)
    const payloadOld = `${newKeyHex}\n${ts}`;
    const sigBufOld  = await crypto.subtle.sign('Ed25519', myIdentity.privateKey, new TextEncoder().encode(payloadOld));
    const sigByOld   = Array.from(new Uint8Array(sigBufOld)).map(b => b.toString(16).padStart(2,'0')).join('');

    // 3. Sign with NEW key: sign(old_key + "\n" + timestamp)
    const payloadNew = `${myIdentity.publicKeyHex}\n${ts}`;
    const sigBufNew  = await crypto.subtle.sign('Ed25519', newKp.privateKey, new TextEncoder().encode(payloadNew));
    const sigByNew   = Array.from(new Uint8Array(sigBufNew)).map(b => b.toString(16).padStart(2,'0')).join('');

    // 4. Send rotation certificate to relay
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      msg.innerHTML = '<span style="color:#e55">Not connected to relay — connect first, then rotate.</span>';
      btn.disabled = false; btn.textContent = 'Rotate Key';
      return;
    }
    ws.send(JSON.stringify({
      type: 'key_rotation',
      old_key:   myIdentity.publicKeyHex,
      new_key:   newKeyHex,
      sig_by_old: sigByOld,
      sig_by_new: sigByNew,
      timestamp:  ts
    }));

    // 5. Store new identity and reload
    msg.innerHTML = '<span style="color:#4ec87a">✓ Rotation sent — storing new identity and reloading…</span>';
    btn.textContent = 'Done';

    // Store new keypair using existing loadOrCreateIdentity infrastructure
    await storeNewRotatedIdentity(newKp, newKeyHex);
    setTimeout(() => location.reload(), 2000);

  } catch(e) {
    msg.innerHTML = `<span style="color:#e55">Error: ${e.message}</span>`;
    btn.disabled = false; btn.textContent = 'Rotate Key';
  }
}

/**
 * Store the newly generated keypair as the active identity.
 * Writes to IndexedDB and localStorage backup so the next page load uses it.
 */
async function storeNewRotatedIdentity(keypair, publicKeyHex) {
  // Write to localStorage backup first (always accessible)
  try {
    const jwk = await crypto.subtle.exportKey('jwk', keypair.privateKey);
    localStorage.setItem('humanity_key', publicKeyHex);
    localStorage.setItem('humanity_key_backup', JSON.stringify({
      publicKeyHex, jwk, rotated: true, rotated_at: Date.now()
    }));
  } catch(e) { console.warn('localStorage backup of rotated key failed:', e); }

  // Write to IndexedDB (same pattern as crypto.js storeKeypair)
  try {
    const db = await new Promise((res, rej) => {
      const req = indexedDB.open('humanity_identity_v2', 1);
      req.onsuccess = () => res(req.result);
      req.onerror   = () => rej(req.error);
    });
    const tx    = db.transaction('keypairs', 'readwrite');
    const store = tx.objectStore('keypairs');
    store.put({ id: publicKeyHex, privateKey: keypair.privateKey, publicKey: keypair.publicKey });
    localStorage.setItem('humanity_key', publicKeyHex);
  } catch(e) { console.warn('IndexedDB store of rotated key failed (localStorage backup is set):', e); }
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
