// ── chat-protected.js ─────────────────────────────────────────────────────
// The protected setup (step G, 2026-10-10, docs/design/blocking-and-safe-mode.md
// 10h), mirroring the desktop app: a PIN lock on this device's safety
// settings. It never checks age, never tells any server anything, and never
// says it makes anything safe. Every word it shows comes from
// /data/gui/safety_presets.json (the `protected` preset), except the short
// labels listed in /shared/protected.js PROTECTED_LABELS, which the file has
// no field for.
//
// Here: the Settings > Safety section (turn on, and while on: public rooms,
// the pictures rule, change the PIN, turn off), the setup's steps (read the
// sentences, choose a PIN, review who can already reach this device, apply),
// the PIN prompt (with the 60-second wait after three wrong PINs, and "Forgot
// the PIN?" through the recovery phrase), and what changes on screen while it
// is on: the always-visible line (top of Safety and above the DM list), public
// rooms hidden from the channel list (and their posts dropped before any
// handler sees them), pictures and files from non-friends not shown, and the
// recovery phrase (and every copy of the identity it comes from) shown only
// after the PIN. "Forgot the PIN?" takes only the phrase of the identity the
// setup was turned on under.
//
// The rules themselves (which action needs the PIN, the channel filter, the
// verifier) are /shared/protected.js, which the other chat scripts ask before
// each locked action; this file only draws and answers the prompt.
//
// Turning it on sends exactly one ordinary `reach_set` with the preset's
// values, which any adult could send too. Nothing else, no flag, in any frame:
// the setup's state stays in this device's storage and never goes in the self
// notes, the vault backup or any export.
//
// Depends on: /shared/protected.js, /shared/reach.js, app.js (ws, myKey,
// channelList, activeChannel, switchChannel, updateChannelList,
// addSystemMessage, SCRATCH_PAD_ID), chat-dm-store.js (hosDmStore),
// chat-social.js (setFollowLocal), chat-groups-p2p.js (leaveP2pGroup,
// loadP2pGroups), chat-voice-rooms.js (leaveVoiceRoom), chat-privacy.js
// (renderSafetyPanel, reachDisplayName, renderRequestsEverywhere),
// chat-warnings.js (recoveryPhraseWords, setMessageWarningsOn),
// chat-profile.js (confirmRevealSeedPhrase), crypto.js (openLinkDeviceModal,
// downloadIdentityBackup, which this file wraps).
// Test: scripts/tests/protected-web.test.js
// ─────────────────────────────────────────────────────────────────────────

// ── The preset (data) ────────────────────────────────────────────────────

let protectedPreset = null;        // protectedPresetFrom() output, once loaded
let protectedPresetLoading = null; // the fetch in flight
let protectedPresetTriedAt = 0;    // when the last fetch started
// After a failed fetch, wait this long before asking again (a fetch that fails
// at once must never turn into a loop; see chat-ui.js renderServerList).
const PROTECTED_PRESET_RETRY_MS = 60000;

/** The clock this file reads (one place, so a test can move it). */
function protectedNow() {
  return Date.now();
}

/** The `protected` preset from the data file; null while it cannot be read. */
function loadProtectedPreset() {
  if (protectedPreset) return Promise.resolve(protectedPreset);
  if (protectedPresetLoading) return protectedPresetLoading;
  if (protectedPresetTriedAt && protectedNow() - protectedPresetTriedAt < PROTECTED_PRESET_RETRY_MS) return Promise.resolve(null);
  protectedPresetTriedAt = protectedNow();
  protectedPresetLoading = Promise.resolve()
    .then(() => fetch(PROTECTED_PRESETS_URL, { cache: 'no-cache' }))
    .then((r) => (r && r.ok ? r.json() : null))
    .then((j) => protectedPresetFrom(j))
    .catch(() => null)
    .then((p) => {
      protectedPresetLoading = null;
      if (p) {
        protectedPreset = p;
        protectedRedrawAll();
      }
      return p;
    });
  return protectedPresetLoading;
}

function pLabel(key, fill) {
  return protectedLabel(protectedPreset, key, fill);
}

function pEsc(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function pSay(text) {
  if (typeof addSystemMessage === 'function') addSystemMessage(text);
}

function pStore() {
  return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
}

function pSocketOpen() {
  return typeof ws !== 'undefined' && ws && ws.readyState === WebSocket.OPEN;
}

/** Draw again everything the setup changes. Each part may not be on the page yet. */
function protectedRedrawAll() {
  for (const name of ['renderServerList', 'renderChannelList', 'renderRequestsEverywhere', 'renderSafetyPanel']) {
    const fn = window[name];
    if (typeof fn === 'function') {
      try { fn(); } catch (e) { /* that part is not drawn yet */ }
    }
  }
  if (typeof renderDmList === 'function') {
    try { renderDmList(); } catch (e) { /* the DMs tab is not drawn yet */ }
  }
  renderProtectedSetup();
  renderProtectedPin();
}

// ── Who counts as a friend for pictures ──────────────────────────────────
// A mutual follow who holds a pass from me: someone the relay lets message me
// under "Friends" and whom I follow back. Before the store has loaded nobody
// is, so nothing from anyone shows early (fail safe).

function protectedSenderIsFriend(key) {
  const store = pStore();
  return !!(store && key && store.isFriendPeer(key) && store.certSentTo(key));
}

function protectedSenderIsMe(key) {
  return typeof myKey === 'string' && typeof key === 'string' && !!key && key.toLowerCase() === myKey.toLowerCase();
}

/** Are pictures and files from `fromKey` not shown now? */
function protectedHidesPicturesFrom(fromKey) {
  return protectedHidesPictures(protectedCurrent(), { isMe: protectedSenderIsMe(fromKey), isFriend: protectedSenderIsFriend(fromKey) });
}

/** The line put where a hidden picture or file was. */
function protectedPictureHiddenHtml() {
  return protectedHidePicturesHtml('<span class="img-placeholder"></span>', protectedPreset ? protectedPreset.picture_hidden_line : '');
}

/** app.js formatBody's last step for a message from `fromKey`: its pictures and files taken out when the rule says so. */
function protectedBodyHtml(html, fromKey) {
  if (!fromKey || !protectedHidesPicturesFrom(fromKey)) return html;
  return protectedHidePicturesHtml(html, protectedPreset ? protectedPreset.picture_hidden_line : '');
}

/** A link preview from `fromKey` without its picture when the rule says so (its words stay). */
function protectedPreviewFor(preview, fromKey) {
  if (!preview || !protectedHidesPicturesFrom(fromKey)) return preview;
  const out = Object.assign({}, preview);
  delete out.image;
  return out;
}

// ── Public rooms ─────────────────────────────────────────────────────────

/** Is this channel hidden now? The scratch pad never is (nothing in it comes from anyone). */
function protectedChannelHidden(id) {
  const state = protectedCurrent();
  if (!state || state.public_rooms === 'all') return false;
  if (typeof SCRATCH_PAD_ID !== 'undefined' && id === SCRATCH_PAD_ID) return false;
  const list = typeof channelList !== 'undefined' && Array.isArray(channelList) ? channelList : [];
  const ch = list.find((c) => c && c.id === id);
  // A channel this client does not know yet is hidden until the list says what it is.
  return !protectedChannelShown(ch || null, state);
}

/** Does the setup hide public rooms now? */
function protectedHidesRooms() {
  const state = protectedCurrent();
  return !!(state && state.public_rooms !== 'all');
}

/** Where to go instead of a hidden channel: the first one listed, or the scratch pad. */
function protectedFirstShownChannel() {
  const list = typeof channelList !== 'undefined' && Array.isArray(channelList) ? channelList : [];
  const shown = protectedChannelsShown(list, protectedCurrent()).shown;
  if (shown.length) return shown[0].id;
  return typeof SCRATCH_PAD_ID !== 'undefined' ? SCRATCH_PAD_ID : null;
}

/** The line in the channel list while rooms are hidden. */
function protectedRoomsHiddenHtml() {
  if (!protectedPreset) return '';
  return `<div class="protected-rooms-hidden" style="padding:var(--space-xs) var(--space-md);color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;">${pEsc(protectedPreset.public_rooms_hidden_line)}</div>`;
}

/**
 * If the channel on screen is hidden now, go to one that is listed. With a DM
 * or a group open, only the channel to come back to changes.
 */
function protectedEnsureChannelShown() {
  if (typeof activeChannel === 'undefined' || !protectedChannelHidden(activeChannel)) return;
  const to = protectedFirstShownChannel();
  if (!to) return;
  const inDm = typeof activeDmPartner !== 'undefined' && activeDmPartner;
  const inGroup = !!window.activeP2pGroup;
  if (inDm || inGroup) {
    activeChannel = to;
    try { localStorage.setItem('humanity_channel', to); } catch (e) { /* storage off */ }
    return;
  }
  if (typeof switchChannel === 'function') switchChannel(to);
}

// A hidden channel is never opened: by a click, a link, or the channel saved from last time.
const _origSwitchChannelProtected = switchChannel;
switchChannel = function (channelId) {
  const to = protectedChannelHidden(channelId) ? protectedFirstShownChannel() : channelId;
  return _origSwitchChannelProtected(to);
};

// When the server's list arrives, the saved channel may turn out to be a public room.
const _origUpdateChannelListProtected = updateChannelList;
updateChannelList = function (channels) {
  _origUpdateChannelListProtected(channels);
  protectedEnsureChannelShown();
};

// Posts, typing and reactions in a hidden room are dropped before any handler
// sees them, so nothing from them is drawn or notified (chat-ui.js notifies
// for a post in any channel).
const PROTECTED_ROOM_FRAMES = ['chat', 'federated_chat', 'typing', 'reaction', 'link_previews'];
const _origHandleMessageProtected = handleMessage;
handleMessage = function (msg) {
  if (msg && PROTECTED_ROOM_FRAMES.includes(msg.type) && protectedHidesRooms()) {
    const ch = typeof msg.channel === 'string' && msg.channel ? msg.channel
      : (msg.type === 'chat' || msg.type === 'federated_chat' ? 'general' : null);
    if (ch && protectedChannelHidden(ch)) return;
  }
  return _origHandleMessageProtected(msg);
};

// ── The recovery phrase ──────────────────────────────────────────────────
// Whoever has the phrase can set a new PIN through "Forgot the PIN?", so with
// the setup on, every way the phrase leaves this device asks for the PIN first
// (`show_phrase`, as the desktop app's Show Recovery Phrase does): the phrase
// itself (chat-profile.js openSeedPhraseModal, whose callers are the Seed
// button, /recovery, the onboarding launch pad and Safety's Show the recovery
// phrase), and every copy of the identity it comes from, because the phrase
// can be read back out of each one: the encrypted backup file (chat-profile.js
// openEncryptedBackupModal, also /backup), and the two crypto.js wraps here,
// the device-link code (openLinkDeviceModal) and the plain identity file
// (downloadIdentityBackup, /export). The onboarding guide shows no words while
// the setup is on (chat-onboarding.js).

const _origOpenLinkDeviceModalProtected = openLinkDeviceModal;
openLinkDeviceModal = function () {
  if (!protectedTake('show_phrase')) return protectedAskThen('show_phrase', () => openLinkDeviceModal());
  return _origOpenLinkDeviceModalProtected();
};

const _origDownloadIdentityBackupProtected = downloadIdentityBackup;
downloadIdentityBackup = function (name) {
  if (!protectedTake('show_phrase')) return protectedAskThen('show_phrase', () => downloadIdentityBackup(name));
  return _origDownloadIdentityBackupProtected(name);
};

// ── The always-visible line ──────────────────────────────────────────────

/** The status line (the same words whoever turned it on), or '' while the setup is off. */
function protectedStatusLineHtml(where) {
  if (!protectedIsOn() || !protectedPreset) return '';
  const pad = where === 'dm' ? 'var(--space-xs) var(--space-md)' : 'var(--space-xs) var(--space-sm)';
  return `<div class="protected-status" role="status" style="padding:${pad};margin:var(--space-xs) 0;border-left:3px solid var(--accent);color:var(--text);font-size:var(--text-sm);line-height:1.4;">${pEsc(protectedPreset.status_line)}</div>`;
}

/** Contact requests while it is on: the preset's `accept_needs_pin` instead of Accept. Null while off. */
function protectedAcceptLabel() {
  return protectedIsOn() && protectedPreset ? protectedPreset.accept_needs_pin : null;
}

// ── Settings > Safety: the section ───────────────────────────────────────

const P_H3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
const P_NOTE = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';
const P_ROW = 'display:flex;flex-wrap:wrap;gap:var(--space-sm);align-items:center;margin:var(--space-xs) 0 var(--space-sm);';
const P_INPUT = 'display:block;width:100%;box-sizing:border-box;margin:var(--space-xs) 0 var(--space-sm);padding:var(--space-sm);font-size:1rem;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);';

/** The section's HTML for the setup as it stands (pure but for reading the state and the store). */
function protectedSafetyHtml() {
  const p = protectedPreset;
  if (!p) return '';
  const state = protectedCurrent();
  let html = `<h3 class="protected-section" style="${P_H3}">${pEsc(p.name)}</h3>`;
  if (!state) {
    // 10h: the button, and under it the finding's sentence 1.
    const ready = !!pStore();
    html += `<div style="${P_ROW}"><button class="vr-btn" data-protected-on${ready ? '' : ' disabled'} style="font-size:0.8rem;">${pEsc(p.button)}</button></div>`;
    if (!ready) html += `<p style="${P_NOTE}">${pEsc(pLabel('waiting_store'))}</p>`;
    html += `<p style="${P_NOTE}">${pEsc(p.summary)}</p>`;
    return html;
  }
  html += `<p style="${P_NOTE}">${pEsc(p.routes_line)}</p>`
    + `<p style="${P_NOTE}">${pEsc(p.summary)}</p>`;
  // Public rooms: what showing them means, and the button (showing needs the PIN).
  const roomsShown = state.public_rooms === 'all';
  html += `<p style="${P_NOTE}">${pEsc(p.public_rooms_explain)}</p>`
    + `<div style="${P_ROW}"><button class="vr-btn" data-protected-rooms="${roomsShown ? 'read_only_only' : 'all'}" style="font-size:0.75rem;">`
    + `${pEsc(pLabel(roomsShown ? 'hide_rooms' : 'show_rooms'))}</button></div>`;
  // The pictures rule (turning it down needs the PIN).
  html += `<label style="display:block;color:var(--text);font-size:var(--text-sm);margin:var(--space-xs) 0;">${pEsc(pLabel('pictures'))}`
    + `<select data-protected-pictures aria-label="${pEsc(pLabel('pictures'))}" style="display:block;margin-top:var(--space-xs);max-width:100%;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-xs);">`
    + `<option value="never"${state.pictures === 'never' ? ' selected' : ''}>${pEsc(pLabel('pictures_never'))}</option>`
    + `<option value="click"${state.pictures === 'click' ? ' selected' : ''}>${pEsc(pLabel('pictures_click'))}</option>`
    + '</select></label>';
  // The recovery phrase: whoever keeps it can change the PIN, so showing it
  // needs the PIN (`show_phrase`). This is the place the settings page and
  // the onboarding guide send the PIN holder to.
  html += `<p style="${P_NOTE}">${pEsc(p.forgot_pin_explain)}</p>`
    + `<div style="${P_ROW}"><button class="vr-btn" data-protected-show-phrase style="font-size:0.75rem;">${pEsc(pLabel('show_phrase'))}</button></div>`;
  html += `<div style="${P_ROW}">`
    + `<button class="vr-btn" data-protected-change-pin style="font-size:0.75rem;">${pEsc(pLabel('change_pin'))}</button>`
    + `<button class="vr-btn" data-protected-off style="font-size:0.75rem;">${pEsc(pLabel('turn_off'))}</button>`
    + '</div>';
  return html;
}

/** Hook the section's controls inside `card` (chat-privacy.js renderSafetyPanel calls this). */
function wireProtectedSafety(card) {
  if (!card || typeof card.querySelector !== 'function') return;
  const on = card.querySelector('[data-protected-on]');
  if (on) on.onclick = () => openProtectedSetup();
  const rooms = card.querySelector('[data-protected-rooms]');
  if (rooms) rooms.onclick = () => protectedSetPublicRooms(rooms.dataset.protectedRooms);
  const pics = card.querySelector('[data-protected-pictures]');
  if (pics) pics.onchange = () => protectedSetPictures(pics.value);
  const phrase = card.querySelector('[data-protected-show-phrase]');
  if (phrase) phrase.onclick = () => protectedShowPhrase();
  const change = card.querySelector('[data-protected-change-pin]');
  if (change) change.onclick = () => protectedChangePin();
  const off = card.querySelector('[data-protected-off]');
  if (off) off.onclick = () => protectedTurnOff();
}

/**
 * The section's "Show the recovery phrase": chat-profile.js's own reveal (its
 * hold-to-confirm, then, with the setup on, the PIN). False when that script
 * is not on the page.
 */
function protectedShowPhrase() {
  if (typeof confirmRevealSeedPhrase !== 'function') return false;
  // The phrase's own box opens below Safety's overlay, so Safety closes first.
  const safety = typeof document !== 'undefined' && typeof document.getElementById === 'function' ? document.getElementById('safety-overlay') : null;
  if (safety && safety.classList) safety.classList.remove('open');
  return confirmRevealSeedPhrase();
}

/** The line saying where the PIN opens the recovery phrase (the onboarding guide shows it in place of the words). */
function protectedPhraseNeedsPinLine() {
  return pLabel('phrase_needs_pin');
}

/** Show public rooms (needs the PIN) or hide them again (never locked). Returns true when changed. */
function protectedSetPublicRooms(value) {
  const state = protectedCurrent();
  if (!state || (value !== 'all' && value !== 'read_only_only')) return false;
  const action = value === 'all' ? 'show_public_rooms' : 'hide_public_rooms';
  if (!protectedTake(action)) {
    protectedAskThen(action, () => protectedSetPublicRooms(value));
    protectedRedrawAll();
    return false;
  }
  protectedSave(Object.assign({}, protectedCurrent() || state, { public_rooms: value }));
  protectedRedrawAll();
  protectedEnsureChannelShown();
  return true;
}

/** The pictures rule: 'click' turns it down (needs the PIN), 'never' back up (never locked). */
function protectedSetPictures(value) {
  const state = protectedCurrent();
  if (!state || (value !== 'never' && value !== 'click')) return false;
  const action = value === 'click' ? 'pictures_down' : 'pictures_up';
  if (!protectedTake(action)) {
    protectedAskThen(action, () => protectedSetPictures(value));
    protectedRedrawAll();
    return false;
  }
  protectedSave(Object.assign({}, protectedCurrent() || state, { pictures: value }));
  protectedRedrawAll();
  return true;
}

/** Turn the setup off (needs the PIN). Nothing is sent: the "Who can reach me" rows stay as they are. */
function protectedTurnOff() {
  if (!protectedIsOn()) return false;
  if (!protectedTake('turn_off')) {
    protectedAskThen('turn_off', () => protectedTurnOff());
    return false;
  }
  protectedStateWrite(localStorage, null);
  protectedRedrawAll();
  return true;
}

/** Change the PIN: the current one first, then the new one twice. Resolves true when it changed. */
async function protectedChangePin() {
  if (!protectedIsOn()) return false;
  // The current PIN opens this one change (protectedAskThen), taken at once.
  return protectedAskThen('change_pin', () => {
    if (!protectedTake('change_pin')) return false;
    if (protectedPin) return false;
    return new Promise((resolve) => {
      protectedPin = { action: 'change_pin', resolve, mode: 'newpin', after: 'close', error: '', busy: false };
      renderProtectedPin();
    });
  });
}

// ── The setup's steps ────────────────────────────────────────────────────
// 1. Read the sentences. 2. Choose a PIN (twice). 3. Review who can already
// reach this device, with Remove for each. 4. Apply (the review's own button,
// `review_keep` in the preset).
// Nothing is kept until step 4; cancelling at any step leaves the setup off.

let protectedSetup = null; // {step: 'read'|'pin'|'review', verifier, error, busy}

/** Start the steps. False when the setup is on already, or the words or the local store are not loaded yet. */
function openProtectedSetup() {
  if (protectedIsOn() || protectedSetup) return false;
  if (!protectedPreset) { loadProtectedPreset(); return false; }
  if (!pStore()) return false; // the review needs the friends this device knows
  protectedSetup = { step: 'read', verifier: null, error: '', busy: false };
  // The review lists the groups I am in; ask for them now so they are there by step 3.
  if (typeof window.loadP2pGroups === 'function') {
    try { window.loadP2pGroups(); } catch (e) { /* listed as they are */ }
  }
  renderProtectedSetup();
  return true;
}

function closeProtectedSetup() {
  protectedSetup = null;
  renderProtectedSetup();
}

/** Step 1 to step 2. */
function protectedSetupContinue() {
  if (!protectedSetup || protectedSetup.step !== 'read') return false;
  protectedSetup.step = 'pin';
  protectedSetup.error = '';
  renderProtectedSetup();
  return true;
}

/** Step 2: the PIN twice. Its verifier is made now and kept in memory until step 4. */
async function protectedSetupChoosePin(pin, again) {
  const st = protectedSetup;
  if (!st || st.step !== 'pin' || st.busy) return false;
  const rules = protectedRulesFrom(protectedPreset);
  if (pin !== again || !protectedPinOk(pin, rules)) {
    st.error = pLabel('pin_rule', { min: rules.pin_digits_min, max: rules.pin_digits_max });
    renderProtectedSetup();
    return false;
  }
  st.busy = true;
  st.error = '';
  renderProtectedSetup();
  const verifier = await protectedMakeVerifier(pin);
  if (protectedSetup !== st) return false; // cancelled meanwhile
  st.verifier = verifier;
  st.busy = false;
  st.step = 'review';
  renderProtectedSetup();
  return true;
}

/**
 * Step 3's lists: who can already reach this device. Friends are everyone
 * holding a pass from me and every mutual follow still owed one (the pass
 * sweep would give them one, so they are friends already in all but the
 * pass; the desktop app's `people_to_choose`); groups are the ones I am in;
 * voice rooms the one I am in (and one this page is about to join again after
 * a reload). The friends kept here are the ones approved at step 4.
 */
function protectedReviewModel() {
  const store = pStore();
  const name = (k) => (typeof reachDisplayName === 'function' ? reachDisplayName(k) : String(k).slice(0, 8));
  const friendKeys = store
    ? Array.from(new Set(Object.keys(store.certsSent).filter((k) => store.certSentTo(k))
      .concat(typeof store.friendsWithoutPass === 'function' ? store.friendsWithoutPass() : [])))
    : [];
  const friends = friendKeys.map((key) => ({ key, name: name(key) }))
    .sort((a, b) => a.name.localeCompare(b.name));
  const groups = (Array.isArray(window._p2pGroups) ? window._p2pGroups : [])
    .filter((g) => g && g.group_id)
    .map((g) => ({ id: g.group_id, name: String(g.name || g.group_id.slice(0, 8)) }));
  const roomIds = [];
  if (window._currentRoomId) roomIds.push(String(window._currentRoomId));
  try {
    const rejoin = sessionStorage.getItem('_rejoin_room');
    if (rejoin && !roomIds.includes(String(rejoin))) roomIds.push(String(rejoin));
  } catch (e) { /* storage off */ }
  const roomName = (id) => {
    const vc = (window._voiceChannels || []).find((c) => String(c.id) === id);
    const ch = (typeof channelList !== 'undefined' && Array.isArray(channelList) ? channelList : []).find((c) => String(c.id) === id);
    return (vc && vc.name) || (ch && ch.name) || id;
  };
  const rooms = roomIds.map((id) => ({ id, name: String(roomName(id)) }));
  return { friends, groups, rooms };
}

/**
 * Remove one from the review: a friend is unfollowed (which withdraws their
 * pass), a group or voice room is left. None of these needs the PIN, ever.
 */
async function protectedReviewRemove(kind, id) {
  if (!protectedSetup || protectedSetup.step !== 'review' || !id) return false;
  try {
    if (kind === 'friend') {
      if (typeof setFollowLocal === 'function') await setFollowLocal(id, false);
      // The unfollow above withdraws the passes; make sure they are gone here even if it could not be sent.
      const store = pStore();
      if (store && store.certSentTo(id) && typeof withdrawPassesTo === 'function') withdrawPassesTo(id);
    } else if (kind === 'group') {
      if (typeof window.leaveP2pGroup === 'function') await window.leaveP2pGroup(id);
      if (Array.isArray(window._p2pGroups)) window._p2pGroups = window._p2pGroups.filter((g) => g && g.group_id !== id);
    } else if (kind === 'room') {
      try { if (String(sessionStorage.getItem('_rejoin_room')) === String(id)) sessionStorage.removeItem('_rejoin_room'); } catch (e) { /* storage off */ }
      if (String(window._currentRoomId) === String(id) && typeof leaveVoiceRoom === 'function') leaveVoiceRoom();
    } else {
      return false;
    }
  } catch (e) {
    if (typeof addNotice === 'function') addNotice(String(e && e.message ? e.message : e), 'red', 6);
  }
  renderProtectedSetup();
  return true;
}

/**
 * Step 4: apply. Sends one ordinary `reach_set` with the preset's values and
 * nothing else; the rest is kept on this device: the PIN's verifier, the
 * identity it is turned on under (whose recovery phrase alone opens "Forgot
 * the PIN?"), the rules, and the friends kept in step 3 (the only people a
 * pass may go to from now on without the PIN). Warnings go on, for friends too.
 */
function protectedSetupApply() {
  const st = protectedSetup;
  if (!st || st.step !== 'review' || !st.verifier || st.busy) return false;
  const frame = protectedReachFrame(protectedPreset);
  const identity = protectedIdentityKey(typeof myKey === 'string' ? myKey : '');
  if (!frame || !pSocketOpen() || !identity) {
    st.error = pLabel('not_connected');
    renderProtectedSetup();
    return false;
  }
  const kept = protectedReviewModel().friends.map((f) => f.key);
  ws.send(JSON.stringify(frame));
  protectedStateWrite(localStorage, protectedStateNew(protectedPreset, st.verifier, kept, identity));
  protectedSetup = null;
  if (typeof setMessageWarningsOn === 'function') setMessageWarningsOn(true);
  protectedRedrawAll();
  protectedEnsureChannelShown();
  return true;
}

/** The overlay's HTML for the step the setup is at (pure but for the lists it reads). */
function protectedSetupHtml(st) {
  const p = protectedPreset;
  if (!st || !p) return '';
  let html = `<h2 style="margin:0 0 var(--space-sm);">${pEsc(p.name)}</h2>`;
  const err = st.error ? `<p class="protected-error" role="alert" style="${P_NOTE}color:var(--danger);">${pEsc(st.error)}</p>` : '';
  const buttons = (primary, primaryAttr, disabled) => `<div style="${P_ROW}">`
    + `<button class="vr-btn" ${primaryAttr}${disabled ? ' disabled' : ''} style="font-size:0.8rem;">${pEsc(primary)}</button>`
    + `<button class="vr-btn" data-protected-setup-cancel style="font-size:0.8rem;">${pEsc(pLabel('cancel'))}</button></div>`;
  if (st.step === 'read') {
    return html
      + p.sentences.map((s) => `<p style="${P_NOTE}color:var(--text);">${pEsc(s)}</p>`).join('')
      + buttons(pLabel('continue'), 'data-protected-setup-continue', false);
  }
  if (st.step === 'pin') {
    const rules = protectedRulesFrom(p);
    html += `<p style="${P_NOTE}">${pEsc(p.forgot_pin_explain)}</p>`
      + `<label style="${P_NOTE}display:block;">${pEsc(pLabel('pin'))}`
      + `<input type="password" inputmode="numeric" autocomplete="off" data-protected-new-pin maxlength="${rules.pin_digits_max}" style="${P_INPUT}"></label>`
      + `<label style="${P_NOTE}display:block;">${pEsc(pLabel('pin_again'))}`
      + `<input type="password" inputmode="numeric" autocomplete="off" data-protected-new-pin-again maxlength="${rules.pin_digits_max}" style="${P_INPUT}"></label>`
      + `<p style="${P_NOTE}">${pEsc(pLabel('pin_rule', { min: rules.pin_digits_min, max: rules.pin_digits_max }))}</p>`
      + err
      + buttons(pLabel('continue'), 'data-protected-setup-pin', st.busy);
    return html;
  }
  if (st.step === 'review') {
    const m = protectedReviewModel();
    const list = (title, rows, kind) => `<h3 style="${P_H3}">${pEsc(title)}</h3>`
      + (rows.length
        ? rows.map((r) => `<div class="protected-review-row" data-review-kind="${kind}" style="display:flex;flex-wrap:wrap;align-items:center;gap:var(--space-sm);padding:var(--space-xs) 0;border-top:1px solid var(--border);">`
          + `<span style="flex:1 1 8rem;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text);">${pEsc(r.name)}</span>`
          + `<button class="vr-btn" data-protected-remove="${kind}" data-protected-remove-id="${pEsc(r.key || r.id)}" style="font-size:0.7rem;flex:none;">${pEsc(pLabel('remove'))}</button></div>`).join('')
        : `<p style="${P_NOTE}">${pEsc(pLabel('none'))}</p>`);
    html += `<p style="${P_NOTE}">${pEsc(p.review_intro)}</p>`
      + list(pLabel('friends'), m.friends, 'friend')
      + list(pLabel('groups'), m.groups, 'group')
      + list(pLabel('rooms'), m.rooms, 'room')
      + err
      + buttons(p.review_keep, 'data-protected-setup-apply', false);
    return html;
  }
  return html + err;
}

/** Draw the steps (or take them down when the setup is not being turned on). */
function renderProtectedSetup() {
  if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
  let overlay = document.getElementById('protected-setup-overlay');
  const st = protectedSetup;
  if (!st) {
    if (overlay && overlay.classList) overlay.classList.remove('open');
    return;
  }
  let card = document.getElementById('protected-setup-card');
  if (!overlay || !overlay.dataset || !overlay.dataset.built) {
    overlay = overlay || document.createElement('div');
    overlay.id = 'protected-setup-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.style.zIndex = '10001';
    if (overlay.dataset) overlay.dataset.built = '1';
    card = card || document.createElement('div');
    card.id = 'protected-setup-card';
    card.className = 'profile-modal';
    if (typeof card.setAttribute === 'function') {
      card.setAttribute('role', 'dialog');
      card.setAttribute('aria-label', protectedPreset ? protectedPreset.name : '');
    }
    card.onclick = (e) => { if (e) e.stopPropagation(); };
    overlay.appendChild(card);
    if (document.body) document.body.appendChild(overlay);
  }
  if (overlay.classList) overlay.classList.add('open');
  if (!card) return;
  card.innerHTML = protectedSetupHtml(st);
  if (typeof card.querySelector !== 'function') return;
  const cancel = card.querySelector('[data-protected-setup-cancel]');
  if (cancel) cancel.onclick = () => closeProtectedSetup();
  const next = card.querySelector('[data-protected-setup-continue]');
  if (next) next.onclick = () => protectedSetupContinue();
  const pin = card.querySelector('[data-protected-setup-pin]');
  if (pin) {
    pin.onclick = () => {
      const a = card.querySelector('[data-protected-new-pin]');
      const b = card.querySelector('[data-protected-new-pin-again]');
      protectedSetupChoosePin(a ? String(a.value || '') : '', b ? String(b.value || '') : '');
    };
  }
  if (typeof card.querySelectorAll === 'function') {
    card.querySelectorAll('[data-protected-remove]').forEach((b) => {
      b.onclick = () => { b.disabled = true; protectedReviewRemove(b.dataset.protectedRemove, b.dataset.protectedRemoveId); };
    });
  }
  const apply = card.querySelector('[data-protected-setup-apply]');
  if (apply) apply.onclick = () => protectedSetupApply();
}

// ── The PIN prompt ───────────────────────────────────────────────────────
// One at a time. Modes: 'pin' (enter it), 'forgot' (the recovery phrase),
// 'newpin' (choose one, twice). Three wrong PINs in a row wait 60 seconds,
// kept in storage so reloading does not skip it.

let protectedPin = null; // {action, resolve, mode, after, error, busy}

/** Ask for the PIN for `action`. Resolves true for the right PIN, false when cancelled. (/shared/protected.js protectedAskThen calls this.) */
function protectedAskPin(action) {
  if (protectedPin) return Promise.resolve(false);
  return new Promise((resolve) => {
    protectedPin = { action, resolve, mode: 'pin', after: null, error: '', busy: false };
    loadProtectedPreset();
    renderProtectedPin();
  });
}

function protectedPinClose(ok) {
  const st = protectedPin;
  protectedPin = null;
  renderProtectedPin();
  if (st && typeof st.resolve === 'function') st.resolve(!!ok);
}

function protectedWaitLine(ms) {
  return pLabel('pin_wait', { seconds: Math.max(1, Math.ceil(ms / 1000)) });
}

/** Enter the PIN. Resolves true when it was right (the prompt closes and the action runs). */
async function protectedPinSubmit(pin) {
  const st = protectedPin;
  if (!st || st.mode !== 'pin' || st.busy) return false;
  const state = protectedCurrent();
  if (!state) { protectedPinClose(true); return true; } // turned off meanwhile: nothing is locked
  const left = protectedWaitLeftMs(state, protectedNow());
  if (left > 0) {
    st.error = protectedWaitLine(left);
    renderProtectedPin();
    return false;
  }
  st.busy = true;
  st.error = '';
  renderProtectedPin();
  const ok = await protectedPinMatches(String(pin == null ? '' : pin), state.pin);
  const now = protectedCurrent() || state;
  st.busy = false;
  if (ok) {
    protectedSave(protectedAfterRight(now));
    protectedPinClose(true);
    return true;
  }
  const next = protectedAfterWrong(now, protectedNow());
  protectedSave(next);
  const wait = protectedWaitLeftMs(next, protectedNow());
  st.error = wait > 0 ? protectedWaitLine(wait) : pLabel('pin_wrong');
  renderProtectedPin();
  return false;
}

/** "Forgot the PIN?": ask for the recovery phrase instead. */
function protectedPinForgot() {
  const st = protectedPin;
  if (!st || st.mode !== 'pin') return false;
  st.mode = 'forgot';
  st.error = '';
  renderProtectedPin();
  return true;
}

/**
 * The recovery phrase typed: when it is the phrase of the identity the setup
 * was turned on under (derived on this device, never kept), a new PIN may be
 * chosen; the setup stays on. Under any other identity nothing typed opens it,
 * not even that identity's own correct phrase: a new identity made on this
 * device must not be a way round the lock. A setup that holds no identity, or
 * a damaged one (only damage to what storage holds can leave that: turning it
 * on always records one), falls back to the identity in use, so the phrase can
 * still open it rather than it locking for good (/shared/protected.js
 * protectedIdentityMatches, the desktop app's rule).
 */
async function protectedForgotSubmit(phrase) {
  const st = protectedPin;
  if (!st || st.mode !== 'forgot' || st.busy) return false;
  st.busy = true;
  renderProtectedPin();
  const sameIdentity = protectedIdentityMatches(protectedCurrent(), typeof myKey === 'string' ? myKey : '');
  const words = sameIdentity && typeof recoveryPhraseWords === 'function' ? await recoveryPhraseWords() : null;
  st.busy = false;
  if (!sameIdentity || !protectedPhraseMatches(phrase, words)) {
    st.error = pLabel('phrase_wrong');
    renderProtectedPin();
    return false;
  }
  st.mode = 'newpin';
  st.after = 'pin';
  st.error = '';
  renderProtectedPin();
  return true;
}

/** Choose the new PIN (twice). After "Forgot the PIN?" the prompt goes back to asking for it; after "Change the PIN" it closes. */
async function protectedNewPinSubmit(pin, again) {
  const st = protectedPin;
  if (!st || st.mode !== 'newpin' || st.busy) return false;
  const state = protectedCurrent();
  if (!state) { protectedPinClose(false); return false; }
  if (pin !== again || !protectedPinOk(pin, state.rules)) {
    st.error = pLabel('pin_rule', { min: state.rules.pin_digits_min, max: state.rules.pin_digits_max });
    renderProtectedPin();
    return false;
  }
  st.busy = true;
  st.error = '';
  renderProtectedPin();
  const verifier = await protectedMakeVerifier(pin);
  const now = protectedCurrent() || state;
  protectedSave(Object.assign({}, now, { pin: verifier, wrong: 0, wait_until: 0 }));
  st.busy = false;
  if (st.after === 'close') {
    protectedPinClose(true);
  } else {
    st.mode = 'pin';
    st.after = null;
    renderProtectedPin();
  }
  return true;
}

/** The prompt's HTML (pure but for reading the wait from the state). */
function protectedPinHtml(st) {
  if (!st) return '';
  const p = protectedPreset;
  const state = protectedCurrent();
  const rules = state ? state.rules : protectedRulesFrom(p);
  let html = `<h2 style="margin:0 0 var(--space-sm);">${pEsc(p ? p.name : '')}</h2>`;
  const err = st.error ? `<p class="protected-error" role="alert" style="${P_NOTE}color:var(--danger);">${pEsc(st.error)}</p>` : '';
  const row = (primary, attr, disabled) => `<div style="${P_ROW}">`
    + `<button class="vr-btn" ${attr}${disabled ? ' disabled' : ''} style="font-size:0.8rem;">${pEsc(primary)}</button>`
    + `<button class="vr-btn" data-protected-pin-cancel style="font-size:0.8rem;">${pEsc(pLabel('cancel'))}</button></div>`;
  if (st.mode === 'pin') {
    const left = state ? protectedWaitLeftMs(state, protectedNow()) : 0;
    html += `<label style="${P_NOTE}display:block;">${pEsc(pLabel('pin'))}`
      + `<input type="password" inputmode="numeric" autocomplete="off" data-protected-pin maxlength="${rules.pin_digits_max}" style="${P_INPUT}"></label>`
      + (left > 0 && !st.error ? `<p class="protected-error" role="alert" style="${P_NOTE}color:var(--danger);">${pEsc(protectedWaitLine(left))}</p>` : err)
      + row(pLabel('continue'), 'data-protected-pin-submit', st.busy || left > 0)
      + `<button class="vr-btn" data-protected-pin-forgot style="font-size:0.75rem;">${pEsc(pLabel('forgot'))}</button>`;
  } else if (st.mode === 'forgot') {
    html += `<p style="${P_NOTE}">${pEsc(p ? p.forgot_pin_explain : '')}</p>`
      + `<label style="${P_NOTE}display:block;">${pEsc(pLabel('phrase'))}`
      + `<textarea data-protected-phrase rows="4" autocomplete="off" spellcheck="false" style="${P_INPUT}"></textarea></label>`
      + err
      + row(pLabel('continue'), 'data-protected-phrase-submit', st.busy);
  } else if (st.mode === 'newpin') {
    html += `<p style="${P_NOTE}">${pEsc(p ? p.forgot_pin_explain : '')}</p>`
      + `<label style="${P_NOTE}display:block;">${pEsc(pLabel('pin'))}`
      + `<input type="password" inputmode="numeric" autocomplete="off" data-protected-pin-new maxlength="${rules.pin_digits_max}" style="${P_INPUT}"></label>`
      + `<label style="${P_NOTE}display:block;">${pEsc(pLabel('pin_again'))}`
      + `<input type="password" inputmode="numeric" autocomplete="off" data-protected-pin-new-again maxlength="${rules.pin_digits_max}" style="${P_INPUT}"></label>`
      + `<p style="${P_NOTE}">${pEsc(pLabel('pin_rule', { min: rules.pin_digits_min, max: rules.pin_digits_max }))}</p>`
      + err
      + row(pLabel('continue'), 'data-protected-pin-new-submit', st.busy);
  }
  return html;
}

/** Draw the prompt (or take it down). */
function renderProtectedPin() {
  if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
  let overlay = document.getElementById('protected-pin-overlay');
  const st = protectedPin;
  if (!st) {
    if (overlay && overlay.classList) overlay.classList.remove('open');
    return;
  }
  let card = document.getElementById('protected-pin-card');
  if (!overlay || !overlay.dataset || !overlay.dataset.built) {
    overlay = overlay || document.createElement('div');
    overlay.id = 'protected-pin-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.style.zIndex = '10002';
    if (overlay.dataset) overlay.dataset.built = '1';
    card = card || document.createElement('div');
    card.id = 'protected-pin-card';
    card.className = 'profile-modal';
    if (typeof card.setAttribute === 'function') {
      card.setAttribute('role', 'dialog');
      card.setAttribute('aria-label', pLabel('pin'));
    }
    card.onclick = (e) => { if (e) e.stopPropagation(); };
    overlay.appendChild(card);
    if (document.body) document.body.appendChild(overlay);
  }
  if (overlay.classList) overlay.classList.add('open');
  if (!card) return;
  card.innerHTML = protectedPinHtml(st);
  if (typeof card.querySelector !== 'function') return;
  const val = (sel) => { const el = card.querySelector(sel); return el ? String(el.value || '') : ''; };
  const cancel = card.querySelector('[data-protected-pin-cancel]');
  if (cancel) cancel.onclick = () => protectedPinClose(false);
  const submit = card.querySelector('[data-protected-pin-submit]');
  if (submit) submit.onclick = () => protectedPinSubmit(val('[data-protected-pin]'));
  const forgot = card.querySelector('[data-protected-pin-forgot]');
  if (forgot) forgot.onclick = () => protectedPinForgot();
  const phrase = card.querySelector('[data-protected-phrase-submit]');
  if (phrase) phrase.onclick = () => protectedForgotSubmit(val('[data-protected-phrase]'));
  const fresh = card.querySelector('[data-protected-pin-new-submit]');
  if (fresh) fresh.onclick = () => protectedNewPinSubmit(val('[data-protected-pin-new]'), val('[data-protected-pin-new-again]'));
  const input = card.querySelector('[data-protected-pin]');
  if (input && typeof input.addEventListener === 'function') {
    input.addEventListener('keydown', (e) => { if (e && e.key === 'Enter') protectedPinSubmit(val('[data-protected-pin]')); });
  }
  // Each redraw replaces the fields, so put the cursor back in the first one
  // (after a wrong PIN the person types again straight away).
  const first = card.querySelector(st.mode === 'forgot' ? '[data-protected-phrase]'
    : st.mode === 'newpin' ? '[data-protected-pin-new]' : '[data-protected-pin]');
  if (first && !st.busy && typeof first.focus === 'function') {
    try { first.focus(); } catch (e) { /* not focusable here */ }
  }
  // While the wait runs, count it down on screen.
  const state = protectedCurrent();
  if (st.mode === 'pin' && state && protectedWaitLeftMs(state, protectedNow()) > 0) {
    setTimeout(() => { if (protectedPin === st) renderProtectedPin(); }, 1000);
  }
}

// Start reading the words now, so the line and the section are there when first drawn.
loadProtectedPreset();

window.loadProtectedPreset = loadProtectedPreset;
window.protectedAskPin = protectedAskPin;
window.protectedBodyHtml = protectedBodyHtml;
window.protectedPreviewFor = protectedPreviewFor;
window.protectedHidesPicturesFrom = protectedHidesPicturesFrom;
window.protectedPictureHiddenHtml = protectedPictureHiddenHtml;
window.protectedChannelHidden = protectedChannelHidden;
window.protectedHidesRooms = protectedHidesRooms;
window.protectedRoomsHiddenHtml = protectedRoomsHiddenHtml;
window.protectedStatusLineHtml = protectedStatusLineHtml;
window.protectedAcceptLabel = protectedAcceptLabel;
window.protectedSafetyHtml = protectedSafetyHtml;
window.wireProtectedSafety = wireProtectedSafety;
window.openProtectedSetup = openProtectedSetup;
window.protectedTurnOff = protectedTurnOff;
window.protectedChangePin = protectedChangePin;
window.protectedSetPublicRooms = protectedSetPublicRooms;
window.protectedSetPictures = protectedSetPictures;
window.protectedShowPhrase = protectedShowPhrase;
window.protectedPhraseNeedsPinLine = protectedPhraseNeedsPinLine;
