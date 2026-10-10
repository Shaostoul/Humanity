// ── chat-privacy.js ───────────────────────────────────────────────────────
// Privacy tiers (2026-08-23, mirrors native src/gui/pages/privacy.rs).
// Every user chooses how visible to be, ONCE, at first connect — and the
// default is maximum privacy. Tiers come from /data/gui/privacy_tiers.json
// (one data source for both clients). A tier presets two real switches:
//   - hide_presence  → relay `privacy_update` (server-enforced: never
//     online, no last_seen stored, no join/leave/typing signals)
//   - directory_unlisted → privacy.directory in the profile privacy JSON
//     (the existing member-directory opt-out), applied by merging into
//     the local profile store and pushing via pushProfileToRelay().
// The choice persists in localStorage ('humanity_privacy_tier') and is
// re-asserted on every fresh connection (per-server flags).
// Depends on: app.js (ws, myKey), chat-profile.js (loadProfileLocal,
// saveProfileLocal, pushProfileToRelay), chat-ui.js (addSystemMessage).
// ─────────────────────────────────────────────────────────────────────────

let _privacyTiers = null;         // loaded tier defs
let _privacyModalShown = false;   // once per page load

async function loadPrivacyTiers() {
  if (_privacyTiers) return _privacyTiers;
  try {
    const res = await fetch('/data/gui/privacy_tiers.json');
    const data = await res.json();
    if (data && Array.isArray(data.tiers) && data.tiers.length) {
      _privacyTiers = { defaultTier: data.default_tier || 'private', tiers: data.tiers };
      return _privacyTiers;
    }
  } catch (e) {
    console.warn('privacy tiers load failed:', e && e.message);
  }
  // Fail-private fallback.
  _privacyTiers = {
    defaultTier: 'private',
    tiers: [{
      id: 'private', name: 'Private', tagline: 'Maximum privacy.',
      description: 'You never appear online and are not listed in the public directory.',
      hide_presence: true, directory_unlisted: true,
    }],
  };
  return _privacyTiers;
}

/** Apply a tier: server presence flag + directory listing + persistence. */
async function applyPrivacyTier(tierId) {
  const { tiers } = await loadPrivacyTiers();
  const tier = tiers.find((t) => t.id === tierId);
  if (!tier) return;
  // 1) Presence (server-enforced).
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'privacy_update', hide_presence: !!tier.hide_presence }));
  }
  // 2) Directory listing: merge into the local profile store so the push
  // path carries every other profile field unchanged.
  try {
    const local = loadProfileLocal();
    local.privacy = local.privacy || {};
    if (tier.directory_unlisted) local.privacy.directory = 'unlisted';
    else delete local.privacy.directory;
    saveProfileLocal(local);
    if (typeof pushProfileToRelay === 'function') pushProfileToRelay();
  } catch (e) {
    console.warn('privacy tier profile merge failed:', e && e.message);
  }
  // 3) Persist the choice; the modal never re-asks.
  localStorage.setItem('humanity_privacy_tier', tier.id);
  if (typeof addSystemMessage === 'function') {
    addSystemMessage('Privacy level set to <strong>' + esc(tier.name) + '</strong>. Change it any time from the account menu.');
  }
}

/** Re-assert the persisted choice on a fresh connection (flags are
 *  per-server; a new or wiped server learns the choice immediately). */
async function reassertPrivacyTier() {
  const chosen = localStorage.getItem('humanity_privacy_tier');
  if (!chosen) return;
  const { tiers } = await loadPrivacyTiers();
  const tier = tiers.find((t) => t.id === chosen);
  if (!tier) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'privacy_update', hide_presence: !!tier.hide_presence }));
  }
}

/** First-connect chooser. Called from app.js once identity is confirmed. */
async function maybeShowPrivacyTierModal() {
  if (_privacyModalShown) return;
  if (localStorage.getItem('humanity_privacy_tier')) {
    reassertPrivacyTier();
    return;
  }
  _privacyModalShown = true;
  const { defaultTier, tiers } = await loadPrivacyTiers();
  let selected = defaultTier;

  const overlay = document.createElement('div');
  overlay.id = 'privacy-tier-modal';
  overlay.style.cssText = 'position:fixed;inset:0;z-index:10000;background:rgba(0,0,0,0.6);display:flex;align-items:center;justify-content:center;padding:16px;';
  const cards = tiers.map((t) => `
    <label class="privacy-tier-card" data-tier="${esc(t.id)}" style="display:block;border:1px solid var(--border);border-radius:10px;padding:10px 12px;margin-bottom:8px;cursor:pointer;background:var(--bg-secondary);">
      <div style="display:flex;align-items:center;gap:8px;">
        <input type="radio" name="privacy-tier" value="${esc(t.id)}"${t.id === defaultTier ? ' checked' : ''}>
        <span style="font-weight:700;">${esc(t.name)}</span>
        <span style="color:var(--text-muted);font-size:0.78rem;">${esc(t.tagline)}</span>
      </div>
      <div style="margin:4px 0 0 24px;color:var(--text-muted);font-size:0.78rem;line-height:1.4;">${esc(t.description)}</div>
    </label>`).join('');
  overlay.innerHTML = `
    <div style="max-width:520px;width:100%;max-height:90vh;overflow-y:auto;background:var(--bg-primary);border:1px solid var(--border);border-radius:12px;padding:18px;">
      <h2 style="margin:0 0 6px;font-size:1.05rem;">How visible do you want to be?</h2>
      <p style="margin:0 0 12px;color:var(--text-muted);font-size:0.8rem;line-height:1.45;">
        Your messages are end-to-end encrypted whatever you pick, and this server keeps no
        record of who you message. This only controls whether others can see you online and
        find you in directories. You can change it any time.
      </p>
      ${cards}
      <button id="privacy-tier-apply" class="vr-btn" style="width:100%;margin-top:8px;font-size:0.85rem;padding:10px;">Use this privacy level</button>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelectorAll('input[name="privacy-tier"]').forEach((r) => {
    r.onchange = () => { selected = r.value; };
  });
  document.getElementById('privacy-tier-apply').onclick = async () => {
    await applyPrivacyTier(selected);
    overlay.remove();
    // Protection-by-default nudge: if the seed still sits unwrapped in
    // localStorage, walk straight into the key-protection flow (it has
    // its own explanation and can be skipped). A stolen laptop or a
    // malicious extension is the likeliest real-world attack on a user;
    // the passphrase wrap is the defense.
    try {
      if (typeof isKeyWrapped === 'function' && !isKeyWrapped()
          && typeof openKeyProtectionModal === 'function') {
        openKeyProtectionModal();
      }
    } catch {}
  };
}

// ── Call IP privacy (2026-08-23) ─────────────────────────────────────────
// WebRTC's classic property: a direct call reveals your IP address to the
// person you call. "Relay my calls" forces every call through the server's
// TURN relay instead (iceTransportPolicy: 'relay'). FAIL CLOSED: if no
// TURN allocation is available the call fails rather than leaking your
// address — which is what a privacy switch must do.
function applyRelayCallsPreference() {
  const on = localStorage.getItem('humanity_relay_calls_only') === '1';
  try {
    if (typeof rtcConfig === 'object' && rtcConfig) {
      if (on) rtcConfig.iceTransportPolicy = 'relay';
      else delete rtcConfig.iceTransportPolicy;
    }
  } catch {}
}
function setRelayCallsOnly(on) {
  localStorage.setItem('humanity_relay_calls_only', on ? '1' : '0');
  applyRelayCallsPreference();
  if (typeof addSystemMessage === 'function') {
    addSystemMessage(on
      ? 'Calls will be relayed through the server: people you call cannot learn your IP address. If the server has no relay capacity, calls fail rather than leak.'
      : 'Calls connect directly again (lower latency; the other party can see your IP address, which is how WebRTC normally works).');
  }
}
setTimeout(applyRelayCallsPreference, 300);

// ── Account sovereignty controls (2026-08-23) ────────────────────────────
// Export + erase, injected into the account/identity block so they are
// one click from where the user manages who they are.
//
// The export is an HTTP download as of 2026-09-06. It used to be a WebSocket
// request whose reply arrived as one account_export_data message, which meant
// the relay broadcast the whole export to every connected client's task before
// filtering it down to us, against a 128 KB socket message ceiling. A file
// download is what this always was; now it is served as one.

async function exportMyAccountData() {
  const say = (m) => { if (typeof addSystemMessage === 'function') addSystemMessage(m); };
  // Signed the same way every other chat-side REST call is: Dilithium3 over
  // the purpose-plus-timestamp preimage via pqSignChatMessage.
  // pq-relay-auth.js is for the standalone pages and is not loaded here.
  const timestamp = Date.now();
  const sig = await pqSignChatMessage('account_export', timestamp);
  if (!sig) {
    say('Cannot export: signing failed. Unlock your identity and try again.');
    return;
  }
  const auth = { key: myIdentity.publicKeyHex, timestamp, sig };
  say('Preparing your export...');
  try {
    const res = await fetch('/api/account/export', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(auth),
    });
    if (!res.ok) {
      say('Export failed: ' + (await res.text() || ('HTTP ' + res.status)));
      return;
    }
    const blob = await res.blob();
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'humanityos-account-export-' + new Date().toISOString().slice(0, 10) + '.json';
    document.body.appendChild(a);
    a.click();
    a.remove();
    // Revoke on the next tick: revoking synchronously can cancel the download
    // in some browsers before it has read the blob.
    setTimeout(() => URL.revokeObjectURL(a.href), 10000);
    say('Your account export downloaded.');
  } catch (e) {
    say('Export failed: ' + (e && e.message ? e.message : e));
  }
}

// What the server keeps AFTER an erase, said before the person decides (BUG-135, the
// operator's option 2, 2026-10-04): the server remembers for a limited time that the account
// was erased, so the person's other devices do not sign them up again by themselves. `days`
// is this server's own setting (GET /api/server-info `erased_accounts_ttl_days`); null when it
// could not be read, and then the sentence is empty: nothing is promised about a server whose
// number we do not have (review finding 6). "Up to" because the server forgets the entry
// within that many days, never later (finding 4); the backups clause because a copy rides in
// them until each is deleted (finding 2). The same words as native
// (src/relay/storage/erased_accounts.rs `erase_memory_sentence`).
function eraseMemorySentence(days) {
  if (!(Number.isInteger(days) && days > 0)) return '';
  const howLong = days === 1 ? 'for up to 1 day' : 'for up to ' + days + ' days';
  return 'After the erase this server remembers ' + howLong + ' that this account was erased, as a '
    + 'one-way fingerprint that is not your name or your data, so your other devices do not '
    + 'sign you up again by themselves; then the entry is deleted here, and a copy of it in '
    + 'this server\'s backups lasts until that backup is deleted.';
}

async function eraseMemoryDays() {
  try {
    const res = await fetch('/api/server-info');
    if (!res.ok) return null;
    const info = await res.json();
    return Number.isInteger(info && info.erased_accounts_ttl_days) ? info.erased_accounts_ttl_days : null;
  } catch (e) {
    return null;
  }
}

async function deleteMyAccount() {
  const remembered = eraseMemorySentence(await eraseMemoryDays());
  if (!await holdConfirm('Erase your entire account on this server (messages, uploads, profile, mailbox, membership, your progress in the shared world, what you built in the shared world, and your home\'s plot on the ship)? This is permanent. Data on your own devices stays.' + (remembered ? ' ' + remembered : ''), { seconds: 5, confirmLabel: 'Hold to erase account' })) return;
  const typed = prompt(
    'This ERASES your account on this server: messages, uploads, profile, '
    + 'mailbox, membership, your progress in the shared world, what you built in the shared world, '
    + 'and your home\'s plot on the ship '
    + '(it goes to the next player; if you come back you get a free plot or a guest place), '
    + 'permanently. Data on your own devices stays.\n\n'
    + (remembered ? remembered + '\n\n' : '')
    + 'Type your display name exactly to confirm:');
  if (!typed || !typed.trim()) return;
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'account_delete', confirm_name: typed.trim() }));
  }
}

function injectAccountDataButtons() {
  const host = document.getElementById('my-identity');
  if (!host || document.getElementById('account-data-controls')) return;
  const div = document.createElement('div');
  div.id = 'account-data-controls';
  div.style.cssText = 'display:flex;gap:6px;margin-top:8px;';
  div.innerHTML =
    '<button class="vr-btn" style="flex:1;font-size:0.7rem;" onclick="openSafetyPanel()" title="Choose who can message you, call you and send you trade requests, and answer contact requests.">Safety</button>'
    + '<button class="vr-btn" style="flex:1;font-size:0.7rem;" onclick="exportMyAccountData()" title="Download everything this server stores about you as a JSON file.">Export my data</button>'
    + '<button class="vr-btn" style="flex:1;font-size:0.7rem;color:var(--danger);" onclick="deleteMyAccount()" title="Erase your account and its data from this server. Self-service, permanent.">Erase account</button>';
  host.appendChild(div);
  const relayRow = document.createElement('label');
  relayRow.style.cssText = 'display:flex;align-items:center;gap:6px;margin-top:6px;font-size:0.72rem;color:var(--text-muted);cursor:pointer;';
  const relayOn = localStorage.getItem('humanity_relay_calls_only') === '1';
  relayRow.innerHTML = '<input type="checkbox" id="relay-calls-toggle"' + (relayOn ? ' checked' : '')
    + '> Relay my calls (hide my IP from people I call)';
  relayRow.querySelector('input').onchange = (e) => setRelayCallsOnly(e.target.checked);
  host.appendChild(relayRow);
}
setTimeout(injectAccountDataButtons, 500);

// ── Safety: who can reach me (step B, 2026-10-09) ────────────────────────
// docs/design/blocking-and-safe-mode.md 10c, mirroring native Settings >
// Safety. One row per kind of contact (Messages, Calls, Trades), each with
// one of five audiences; the words, defaults and rules are /shared/reach.js.
// The relay enforces the choice and its `reach_settings` is the source of
// truth for what this page shows: a change goes out as `reach_set` and the
// row shows the relay's answer, not our guess. Also here: the "People who
// may call me" list (a friend's pass re-issued with or without `call`,
// chat-social.js setFriendMayCall), the refusal sentence with a Send request
// button (`reach_refused`), and the Requests list (contact requests, and
// DMs from people these settings refuse, shown by name only).

let reachKnown = null;   // the last `reach_settings` from the relay; null until it says
let reachSaving = null;  // {kind, audience, timer} while a `reach_set` awaits its answer
const REACH_SAVE_WAIT_MS = 8000;
// One refusal offer per person a minute: one refused send is often several
// puts (a follow notice, then the message), and each is refused.
const reachOfferShown = new Map();
const REACH_OFFER_QUIET_MS = 60000;

/** The settings in force: the relay's word, or the safe defaults until it has spoken. */
function reachCurrent() {
  return reachKnown || reachSettingsFrom(null);
}

/** HTML-escape for the strings this section builds (pure: no DOM needed). */
function reachEsc(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function reachSay(text) {
  if (typeof addSystemMessage === 'function') addSystemMessage(text);
}

function reachPeers() {
  return (typeof peerData !== 'undefined' && peerData) ? peerData : {};
}

function reachDisplayName(key) {
  const p = reachPeers()[key];
  return (p && p.display_name) || (typeof shortKey === 'function' ? shortKey(key) : String(key).slice(0, 8));
}

/** The member whose registered name is `name` (any case), or null when there is none or more than one. */
function reachKeyForName(name) {
  const lower = String(name || '').toLowerCase();
  if (!lower) return null;
  let found = null;
  for (const [k, p] of Object.entries(reachPeers())) {
    if (k === myKey || !p) continue;
    if (String(p.display_name || '').toLowerCase() === lower) {
      if (found && found !== k) return null;
      found = k;
    }
  }
  return found;
}

/**
 * Do we share a P2P group (the `groups` audience)? From the group list
 * chat-groups-p2p.js keeps. Until that list has loaded the answer is not
 * known here, and then the relay's own check stands: a DM it let through is
 * not turned into a request, which would drop its text for good.
 */
function reachSharesGroupWith(peer) {
  if (!Array.isArray(window._p2pGroups)) return true;
  return window._p2pGroups.some((g) => g && Array.isArray(g.members) && g.members.includes(peer));
}

function reachStore() {
  return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
}

/** Would my settings let `peer` reach me for `kind`? The relay's rule, applied with what this client knows. */
function reachAllowsFrom(peer, kind) {
  const store = reachStore();
  return reachAllows(reachCurrent()[kind], kind, {
    passMay: store ? store.passMayTo(peer) : null,
    sharesGroup: reachSharesGroupWith(peer),
  });
}

/** The relay's answer: what the settings are now (after identify and after every reach_set). */
function onReachSettings(settings) {
  reachKnown = reachSettingsFrom(settings);
  if (reachSaving) {
    clearTimeout(reachSaving.timer);
    reachSaving = null;
  }
  renderSafetyPanel();
}

/** A row's choice changed: ask the relay; the row shows the answer when it comes. */
function chooseReachAudience(kind, audience) {
  const frame = reachSetFrame({ [kind]: audience });
  if (!frame || !reachKnown) return false;
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    reachSay('Not connected, so the setting was not changed.');
    renderSafetyPanel();
    return false;
  }
  ws.send(JSON.stringify(frame));
  if (reachSaving) clearTimeout(reachSaving.timer);
  // No answer in time: show what the relay last said again.
  reachSaving = { kind, audience, timer: setTimeout(() => { reachSaving = null; renderSafetyPanel(); }, REACH_SAVE_WAIT_MS) };
  renderSafetyPanel();
  return true;
}

// ── Contact requests ──

/**
 * Someone asked to reach me: a contact request ({name}) or a DM my settings
 * refuse ({key}). Listed under Requests by name only. Returns true when it is
 * listed (a "nobody" setting lists nothing; someone already let in needs no
 * request).
 */
function receiveContactRequest(req) {
  const store = reachStore();
  if (!store || !req) return false;
  if (reachCurrent().message === 'nobody') return false;
  let key = req.key || null;
  let name = req.name || '';
  if (!key && name) key = reachKeyForName(name);
  if (key === myKey) return false;
  if (key) {
    if (!req.key && reachAllowsFrom(key, 'message')) return false;
    if (reachPeers()[key] || !name) name = reachDisplayName(key);
    // One entry per person: a request listed by name before their key was known goes.
    store.removeContactRequest('name:' + name.toLowerCase());
  }
  if (!name) return false;
  if (store.addContactRequest({ key, name, ts: Date.now() })) {
    reachSay(`${name} sent you a contact request. Accept or ignore it under Requests in your DMs.`);
    if (typeof notifyNewMessage === 'function') notifyNewMessage(name, 'Contact request', true);
  }
  renderRequestsEverywhere();
  return true;
}

/**
 * A DM (opened and checked) from someone my settings refuse is shown as a
 * contact request, name only: its text is dropped, never stored or shown, so
 * a modified client gains nothing by skipping the flag (10c). Returns true
 * when it was screened out; the caller then neither stores nor shows it.
 */
function reachScreenDm(inner) {
  if (!inner || !inner.from || inner.from === myKey) return false;
  if (reachAllowsFrom(inner.from, 'message')) return false;
  receiveContactRequest({ key: inner.from });
  return true;
}

/** Accept: their request counts as their follow, and following back makes us friends (my pass goes to them). */
async function acceptContactRequest(id) {
  const store = reachStore();
  const req = store && store.contactRequests[id];
  if (!req) return false;
  const key = req.key || reachKeyForName(req.name);
  if (!key) {
    reachSay(`${req.name} is not on this server's member list right now, so they cannot be added yet. Try again when they are online.`);
    return false;
  }
  store.removeContactRequest(id);
  store.setFollower(key, true);
  if (typeof myFollowers !== 'undefined') myFollowers.add(key);
  if (typeof setFollowLocal === 'function') await setFollowLocal(key, true);
  renderRequestsEverywhere();
  return true;
}

/** Ignore: the request goes from the list; nothing is sent and no one is told. */
function ignoreContactRequest(id) {
  const store = reachStore();
  if (store) store.removeContactRequest(id);
  renderRequestsEverywhere();
}

/**
 * Send `peer` a contact request: a sealed DM carrying only my name, at the
 * smallest padding bucket, flagged `contact_request` (crypto.js
 * pqBuildContactRequest). I follow them from here on (no follow notice goes
 * out: their settings would refuse it), so their acceptance completes the
 * friendship.
 */
async function sendContactRequest(peer) {
  if (!peer || peer === myKey) return false;
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    reachSay('Not connected, so the request was not sent.');
    return false;
  }
  const put = await pqBuildContactRequest(peer, myName);
  if (!put) {
    reachSay('The request could not be sent: this person has not been online with a current client here yet, so there is no key to seal it to.');
    return false;
  }
  ws.send(JSON.stringify(put));
  const store = reachStore();
  if (store) store.setFollowing(peer, true);
  if (typeof myFollowing !== 'undefined') myFollowing.add(peer);
  if (typeof updateFriendIndicators === 'function') updateFriendIndicators();
  reachSay('Contact request sent. They will see only your name; if they accept, you become friends.');
  return true;
}

/** The relay refused a send: for a message, the sentence and a Send request button. */
function onReachRefused(msg) {
  const to = msg && msg.to;
  if (!to) return;
  if (msg.kind === 'trade') {
    reachSay(REACH_REFUSED_TRADE);
    return;
  }
  if (msg.kind !== 'message') return;
  const last = reachOfferShown.get(to) || 0;
  if (Date.now() - last < REACH_OFFER_QUIET_MS) return;
  reachOfferShown.set(to, Date.now());
  const el = document.createElement('div');
  el.className = 'message system reach-refused';
  el.style.cssText = 'padding:var(--space-sm) var(--space-md);border-left:3px solid var(--warning);';
  const status = document.createElement('div');
  status.style.cssText = 'font-weight:600;color:var(--text);';
  status.textContent = `Not delivered to ${reachDisplayName(to)}.`;
  const text = document.createElement('div');
  text.style.cssText = 'color:var(--text-muted);margin:var(--space-xs) 0;';
  text.textContent = REACH_REFUSED_MESSAGE;
  const btn = document.createElement('button');
  btn.className = 'vr-btn';
  btn.textContent = 'Send request';
  btn.onclick = async () => {
    btn.disabled = true;
    const ok = await sendContactRequest(to);
    btn.textContent = ok ? 'Request sent' : 'Send request';
    if (!ok) btn.disabled = false;
  };
  el.appendChild(status);
  el.appendChild(text);
  el.appendChild(btn);
  if (typeof appendMessage === 'function') appendMessage(el);
}

// ── Drawing ──

/** The Requests list (name only, Accept and Ignore), shared by the Safety page and the DMs tab. */
function contactRequestsHtml(requests, opts) {
  const compact = !!(opts && opts.compact);
  if (!requests.length) {
    return compact ? '' : '<div style="color:var(--text-muted);font-size:var(--text-sm);">No requests.</div>';
  }
  // In the narrow DMs rail the name takes its own line and the buttons sit under it.
  return requests.map((r) =>
    `<div class="reach-request${compact ? ' dm-item' : ''}" data-req-id="${reachEsc(r.id)}" style="display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) ${compact ? 'var(--space-md);flex-wrap:wrap' : '0'};">`
    + `<span class="dm-name" style="flex:1 1 ${compact ? '100%' : '0'};min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${reachEsc(r.name)}</span>`
    + `<button class="vr-btn" data-req-accept="${reachEsc(r.id)}" style="font-size:0.7rem;">Accept</button>`
    + `<button class="vr-btn" data-req-ignore="${reachEsc(r.id)}" style="font-size:0.7rem;">Ignore</button>`
    + '</div>').join('');
}

/** The Requests block at the top of the DMs tab (empty when there are none). */
function contactRequestsSidebarHtml() {
  const store = reachStore();
  const list = store ? store.contactRequestList() : [];
  if (!list.length) return '';
  return '<div class="dm-requests" style="border-bottom:1px solid var(--border);padding-bottom:var(--space-xs);margin-bottom:var(--space-xs);">'
    + '<div style="font-size:0.6rem;color:var(--text-muted);font-weight:600;letter-spacing:0.05em;padding:var(--space-sm) var(--space-md) 0;">REQUESTS</div>'
    + contactRequestsHtml(list, { compact: true })
    + '</div>';
}

/** Hook the Accept and Ignore buttons inside `container`. */
function wireContactRequestButtons(container) {
  if (!container || typeof container.querySelectorAll !== 'function') return;
  container.querySelectorAll('[data-req-accept]').forEach((b) => {
    b.onclick = (e) => { if (e) e.stopPropagation(); acceptContactRequest(b.dataset.reqAccept); };
  });
  container.querySelectorAll('[data-req-ignore]').forEach((b) => {
    b.onclick = (e) => { if (e) e.stopPropagation(); ignoreContactRequest(b.dataset.reqIgnore); };
  });
}

function renderRequestsEverywhere() {
  if (typeof renderDmList === 'function') {
    try { renderDmList(); } catch (e) { /* the DMs tab is not drawn yet */ }
  }
  renderSafetyPanel();
}

/** What the Safety page shows, from the relay's settings and the passes this client gave. */
function safetyModel() {
  const settings = reachCurrent();
  const known = !!reachKnown;
  const rows = REACH_KINDS.map((kind) => {
    const saving = !!(reachSaving && reachSaving.kind === kind);
    const audience = saving ? reachSaving.audience : settings[kind];
    return {
      kind,
      label: REACH_KIND_LABELS[kind],
      audience,
      explain: reachExplain(kind, audience),
      saving,
      disabled: !known,
      options: REACH_AUDIENCES.map((a) => ({ value: a, label: REACH_AUDIENCE_LABELS[a] })),
    };
  });
  const store = reachStore();
  const given = store ? Object.keys(store.certsSent).filter((p) => store.certSentTo(p)) : [];
  const named = (keys) => keys.map((key) => ({ key, name: reachDisplayName(key) }))
    .sort((a, b) => a.name.localeCompare(b.name));
  const mayCall = (p) => { const m = store.passMayTo(p); return !!m && m.split(',').includes('call'); };
  return {
    known,
    rows,
    callAudience: settings.call,
    callers: named(given.filter(mayCall)),
    others: named(given.filter((p) => !mayCall(p))),
    requests: store ? store.contactRequestList() : [],
  };
}

const SAFETY_H3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
const SAFETY_NOTE = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';

/** The Safety page's HTML for a model (pure, so it can be checked without a browser). */
function safetyPanelHtml(model) {
  let html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
    + '<h2 style="margin:0;">Safety</h2>'
    + '<button class="vr-btn" data-safety-close style="font-size:0.75rem;">Close</button></div>';
  html += `<h3 style="${SAFETY_H3}">Who can reach me</h3>`
    + `<p style="${SAFETY_NOTE}">Choose who can reach you for each kind of contact. This server enforces your choice.</p>`;
  if (!model.known) {
    html += `<p class="safety-waiting" style="${SAFETY_NOTE}">Waiting for this server to send your settings. Until it does, these show the safe defaults and cannot be changed.</p>`;
  }
  for (const row of model.rows) {
    html += `<div class="safety-row" data-kind="${row.kind}" style="padding:var(--space-sm) 0;border-top:1px solid var(--border);">`
      + '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
      + `<span style="font-weight:600;color:var(--text);">${reachEsc(row.label)}</span>`
      + `<select data-reach-kind="${row.kind}" aria-label="Who can reach me: ${reachEsc(row.label)}"${row.disabled ? ' disabled' : ''} style="background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-xs);">`
      + row.options.map((o) => `<option value="${o.value}"${o.value === row.audience ? ' selected' : ''}>${reachEsc(o.label)}</option>`).join('')
      + '</select></div>'
      + `<div class="safety-explain" style="${SAFETY_NOTE}margin-top:var(--space-xs);">${reachEsc(row.explain)}${row.saving ? ' (Saving...)' : ''}</div>`
      + '</div>';
  }
  html += `<h3 style="${SAFETY_H3}">People who may call me</h3>`;
  if (model.callAudience !== 'chosen') {
    html += `<p style="${SAFETY_NOTE}">Calls are set to "${reachEsc(REACH_AUDIENCE_LABELS[model.callAudience] || model.callAudience)}", so this list is used only when Calls is set to "People I choose".</p>`;
  }
  if (model.callers.length) {
    html += model.callers.map((c) =>
      `<div class="safety-caller" style="display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) 0;">`
      + `<span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;color:var(--text);">${reachEsc(c.name)}</span>`
      + `<button class="vr-btn" data-call-remove="${reachEsc(c.key)}" style="font-size:0.7rem;">Remove</button></div>`).join('');
  } else {
    html += `<p style="${SAFETY_NOTE}">Nobody yet.</p>`;
  }
  if (model.others.length) {
    html += '<div style="display:flex;gap:var(--space-sm);align-items:center;margin-top:var(--space-xs);">'
      + `<select data-call-add-pick aria-label="A friend to let call you" style="flex:1;background:var(--bg-input);color:var(--text);border:1px solid var(--border);border-radius:var(--radius-sm);padding:var(--space-xs);">`
      + model.others.map((o) => `<option value="${reachEsc(o.key)}">${reachEsc(o.name)}</option>`).join('')
      + '</select><button class="vr-btn" data-call-add style="font-size:0.7rem;">Add</button></div>';
  } else if (!model.callers.length) {
    html += `<p style="${SAFETY_NOTE}">Friends appear here once you have some.</p>`;
  }
  html += `<h3 style="${SAFETY_H3}">Requests</h3>`
    + `<p style="${SAFETY_NOTE}">People who asked to reach you. You see only their name. Accept makes you friends; Ignore tells no one.</p>`
    + contactRequestsHtml(model.requests);
  return html;
}

/** Open Settings > Safety. */
function openSafetyPanel() {
  let overlay = document.getElementById('safety-overlay');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'safety-overlay';
    overlay.className = 'profile-modal-overlay';
    overlay.style.zIndex = '10000';
    overlay.onclick = (e) => { if (e.target === overlay) overlay.classList.remove('open'); };
    const card = document.createElement('div');
    card.id = 'safety-card';
    card.className = 'profile-modal';
    card.setAttribute('role', 'dialog');
    card.setAttribute('aria-label', 'Safety');
    card.onclick = (e) => e.stopPropagation();
    overlay.appendChild(card);
    document.body.appendChild(overlay);
  }
  const menu = document.getElementById('identity-menu');
  if (menu) menu.style.display = 'none';
  overlay.classList.add('open');
  renderSafetyPanel();
}

/** Redraw the Safety page when it is open. */
function renderSafetyPanel() {
  const overlay = typeof document.getElementById === 'function' ? document.getElementById('safety-overlay') : null;
  const card = overlay && document.getElementById('safety-card');
  if (!card || !overlay.classList || !overlay.classList.contains('open')) return;
  card.innerHTML = safetyPanelHtml(safetyModel());
  const close = card.querySelector('[data-safety-close]');
  if (close) close.onclick = () => overlay.classList.remove('open');
  card.querySelectorAll('select[data-reach-kind]').forEach((sel) => {
    sel.onchange = () => chooseReachAudience(sel.dataset.reachKind, sel.value);
  });
  card.querySelectorAll('[data-call-remove]').forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      if (!await setFriendMayCall(b.dataset.callRemove, false)) reachSay('Could not change that now: their key is not known here yet. Try again when they are online.');
      renderSafetyPanel();
    };
  });
  const add = card.querySelector('[data-call-add]');
  const pick = card.querySelector('[data-call-add-pick]');
  if (add && pick) {
    add.onclick = async () => {
      add.disabled = true;
      if (!await setFriendMayCall(pick.value, true)) reachSay('Could not change that now: their key is not known here yet. Try again when they are online.');
      renderSafetyPanel();
    };
  }
  wireContactRequestButtons(card);
}

// The relay's frames for this section.
const _origHandleMessageReach = handleMessage;
handleMessage = function (msg) {
  if (msg && msg.type === 'reach_settings') { onReachSettings(msg.settings); return; }
  if (msg && msg.type === 'reach_refused') { onReachRefused(msg); return; }
  return _origHandleMessageReach(msg);
};

window.openSafetyPanel = openSafetyPanel;
window.receiveContactRequest = receiveContactRequest;
window.reachScreenDm = reachScreenDm;
window.acceptContactRequest = acceptContactRequest;
window.ignoreContactRequest = ignoreContactRequest;
window.sendContactRequest = sendContactRequest;
window.chooseReachAudience = chooseReachAudience;

window.maybeShowPrivacyTierModal = maybeShowPrivacyTierModal;
window.applyPrivacyTier = applyPrivacyTier;
window.reassertPrivacyTier = reassertPrivacyTier;
window.exportMyAccountData = exportMyAccountData;
window.deleteMyAccount = deleteMyAccount;
window.eraseMemorySentence = eraseMemorySentence;
window.setRelayCallsOnly = setRelayCallsOnly;
