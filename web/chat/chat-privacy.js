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

/**
 * The privacy explanation above the choice. Its second sentence (step D,
 * docs/design/blocking-and-safe-mode.md 10e, REPORT_PRIVACY_SENTENCE in
 * /shared/report.js) is the other side of reports the admins can check: a DM
 * is signed, so whoever receives it can show others who wrote it.
 */
function privacyExplanationText() {
  const signed = typeof REPORT_PRIVACY_SENTENCE === 'string' ? ' ' + REPORT_PRIVACY_SENTENCE : '';
  return 'Your messages are end-to-end encrypted whatever you pick, and this server keeps no record of who you message.'
    + signed
    + ' This only controls whether others can see you online and find you in directories. You can change it any time.';
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
        ${esc(privacyExplanationText())}
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

// (The "Relay my calls" switch that stood here since 2026-08-23 is gone: since
// step E every call and voice room goes through the server for everyone, with
// no direct option to switch to. chat-voice-rooms.js, "Calls go through the
// server".)

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
    '<button class="vr-btn" style="flex:1;font-size:0.7rem;" onclick="openSafetyPanel()" title="Choose who can message you, call you and send you trade requests, answer contact requests, see who you blocked, and turn warnings on messages on or off.">Safety</button>'
    + '<button class="vr-btn" style="flex:1;font-size:0.7rem;" onclick="exportMyAccountData()" title="Download everything this server stores about you as a JSON file.">Export my data</button>'
    + '<button class="vr-btn" style="flex:1;font-size:0.7rem;color:var(--danger);" onclick="deleteMyAccount()" title="Erase your account and its data from this server. Self-service, permanent.">Erase account</button>';
  host.appendChild(div);
}
setTimeout(injectAccountDataButtons, 500);

// ── Safety: who can reach me (step B, 2026-10-09) ────────────────────────
// docs/design/blocking-and-safe-mode.md 10c, mirroring native Settings >
// Safety. One row per kind of contact (Messages, Calls, Trades), each with
// one of five audiences; the words, defaults and rules are /shared/reach.js.
// The relay enforces the choice and its `reach_settings` is the source of
// truth for what this page shows: a change goes out as `reach_set` and the
// row shows the relay's answer, not our guess. Also here: the "People I
// choose" list (10c-ii: each friend with a Message, Call and Trade tick; a
// change re-issues their pass with the `may` the ticks give, chat-social.js
// setFriendTick), the refusal sentence with a Send request
// button (`reach_refused`), and the Requests list (contact requests, and
// DMs from people these settings refuse, shown by name only).

let reachKnown = null;   // the last `reach_settings` from the relay; null until it says
let reachSaving = null;  // {kind, audience, timer} while a `reach_set` awaits its answer
const REACH_SAVE_WAIT_MS = 8000;
// One refusal offer per person a minute: one refused send is often several
// puts (a follow notice, then the message), and each is refused.
const reachOfferShown = new Map();
// People whose contact request was refused this session (`reach_refused` with `request: true`):
// they are not taking requests, so no Send request button is offered to them again.
const reachNotTaking = new Set();
// The Send request offers drawn this session, by person ({text, btn, retired}),
// so a later "not taking requests" can take their buttons away.
const reachOffers = new Map();
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

function reachStore() {
  return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
}

/**
 * Would my settings let `peer` reach me for `kind`? The relay's rule, applied
 * with what this client knows, to something the relay already let through (a
 * DM's text, reachScreenDm; a call's ring, chat-voice-calls.js callerAllowed).
 * A pass sent whose answer has not come counts (10l): the relay honours it if
 * it stored it.
 *
 * Group membership is the server's call (10m R8): under "Friends and people in
 * my groups" someone it let through counts as sharing a group, because it
 * checked their membership against its own records. This page's own list of
 * groups (window._p2pGroups) loads on connect and after my own group changes,
 * so someone who joined a group since was dropped: their allowed ring never
 * rang, their DM became a request with its text gone. The other audiences do
 * not look at groups.
 */
function reachAllowsFrom(peer, kind) {
  const store = reachStore();
  return reachAllows(reachCurrent()[kind], kind, {
    passMay: store ? store.passMayHeld(peer) : null,
    sharesGroup: true,
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
  // With the protected setup on, a row needs the PIN (10h, /shared/protected.js):
  // the row is drawn back as it was, and the change goes out once the PIN is given.
  if (typeof protectedTake === 'function' && !protectedTake('reach_row')) {
    protectedAskThen('reach_row', () => chooseReachAudience(kind, audience));
    renderSafetyPanel();
    return false;
  }
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
 * List someone under Requests: from a contact request whose pass checked
 * ({key, pass, ts}) or a DM my settings refuse ({key, ts}). Shown by the name
 * the member list has for that key, never a name they claimed. Returns true
 * when it is listed (a "nobody" setting lists nothing).
 */
function receiveContactRequest(req) {
  const store = reachStore();
  if (!store || !req || !req.key || req.key === myKey) return false;
  if (store.isBlocked(req.key)) return false; // never listed (step C)
  if (reachCurrent().message === 'nobody') return false;
  const name = reachDisplayName(req.key);
  if (store.addContactRequest({ key: req.key, name, pass: req.pass || null, ts: Number(req.ts) || Date.now() })) {
    reachSay(`${name} sent you a contact request. Accept or ignore it under Requests in your DMs.`);
    if (typeof notifyNewMessage === 'function') notifyNewMessage(name, 'Contact request', true);
  }
  renderRequestsEverywhere();
  return true;
}

/**
 * A DM (opened, its signature checked) whose text is a contact request:
 * returns true when it was one, and then the caller neither stores nor shows
 * it. From someone else, its pass must have been given to me, on this server,
 * by the signed sender; one that does not check is dropped. My own, echoed
 * from another of my devices, records the pass I gave and that I follow them.
 */
async function ingestContactRequest(inner) {
  if (!inner || !isContactRequestText(inner.text)) return false;
  const req = contactRequestParse(inner.text);
  const store = reachStore();
  if (inner.from === myKey) {
    const pass = req ? friendPassParse(req.pass) : null;
    // Sent from a device that had not heard I blocked them yet: the pass is
    // withdrawn at once and the follow is not taken up (step C).
    if (store && inner.to && store.isBlocked(inner.to)) {
      if (pass) store.recordPassSent(inner.to, pass.serial, pass.may);
      if (typeof withdrawPassesTo === 'function') withdrawPassesTo(inner.to);
      return true;
    }
    if (store && pass && inner.to) {
      // An echo of my own pass, like any other (10n N4): standing, unless it
      // grants beyond my choice for them (then withdrawn at once).
      if (store.adoptEchoedPass(inner.to, pass.serial, pass.may).length && typeof sendPendingWithdrawals === 'function') sendPendingWithdrawals();
      store.setFollowing(inner.to, true);
    }
    if (inner.to && typeof myFollowing !== 'undefined') myFollowing.add(inner.to);
    return true;
  }
  if (!req || !await pqVerifyFriendCert(inner.from, myKey, req.pass)) return true;
  receiveContactRequest({ key: inner.from, pass: req.pass, ts: inner.ts });
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
  receiveContactRequest({ key: inner.from, ts: inner.ts });
  return true;
}

/**
 * Accept: their request counts as their follow, and following back makes us
 * friends. The pass their request carried is kept first, so every put to them
 * from here (the follow notice, my pass) presents it as `friend_cert`
 * (crypto.js pqBuildDmPuts) and their relay's gate lets my reply in.
 */
async function acceptContactRequest(id) {
  const store = reachStore();
  const req = store && store.contactRequests[id];
  if (!req || !req.key) return false;
  const key = req.key;
  // Accepting makes a friend: with the protected setup on it needs the PIN
  // (10h), and the request stays listed until it is given.
  if (typeof protectedBefriendAllowed === 'function' && !protectedBefriendAllowed(key)) {
    return protectedAskThen('befriend', () => acceptContactRequest(id));
  }
  store.removeContactRequest(id);
  // Accepting is the person's own word about them (10m R3, R7): a pass to them
  // is no longer held back (setFollowLocal below says so too).
  if (typeof passPersonChoseFor === 'function') passPersonChoseFor(key);
  if (req.pass && await pqVerifyFriendCert(key, myKey, req.pass)) store.storeCertFrom(key, req.pass);
  store.setFollower(key, true);
  if (typeof myFollowers !== 'undefined') myFollowers.add(key);
  if (typeof setFollowLocal === 'function') await setFollowLocal(key, true);
  renderRequestsEverywhere();
  return true;
}

/** Ignore: the request (and the pass it carried) goes; nothing is sent and no one is told. */
function ignoreContactRequest(id) {
  const store = reachStore();
  if (store) store.removeContactRequest(id);
  renderRequestsEverywhere();
}

/**
 * Send `peer` a contact request (crypto.js pqBuildContactRequest): a signed,
 * sealed DM flagged `contact_request`, carrying my name and my pass for them
 * with the default `may`, plus the self-copy that tells my other devices. Once
 * the server takes it (10l: `dm_put_ok` for its ref, chat-social.js
 * holdPassPut) I follow them and they hold my pass, so their acceptance gets
 * through and completes the friendship; the self-copy goes then too. Returns
 * true when it was sent; `opts.onAnswer({taken, outcome, reason})` hears how
 * it went (the Send request button uses it).
 */
async function sendContactRequest(peer, opts) {
  if (!peer || peer === myKey) return false;
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    reachSay('Not connected, so the request was not sent.');
    return false;
  }
  // A request carries my pass for them and follows them: it makes a friend
  // once they accept, so with the protected setup on it needs the PIN (10h).
  if (typeof protectedBefriendAllowed === 'function' && !protectedBefriendAllowed(peer)) {
    return protectedAskThen('befriend', () => sendContactRequest(peer));
  }
  // A request carries my name: never my recovery phrase (step F, chat-warnings.js).
  if (typeof recoveryPhraseGuardStops === 'function' && await recoveryPhraseGuardStops(myName, 'The request was not sent.')) return false;
  // One pass on its way to someone at a time (10l): a request while one is
  // waiting for the server's answer would be a second pass for them.
  if (typeof passPutInFlight === 'function' && passPutInFlight(peer)) {
    reachSay('A request or pass to them is still waiting for this server to answer. Try again in a moment.');
    return false;
  }
  const built = await pqBuildContactRequest(peer, myName);
  if (!built) {
    reachSay('The request could not be sent yet: this person has not been online with a current client here, or your identity is still loading. Try again in a moment.');
    return false;
  }
  // Recorded only when the server takes it; the self-copy is held until then.
  const held = typeof holdPassPut === 'function' && holdPassPut(peer, built, {
    serial: built.serial, may: built.may, kind: 'request',
    onAnswer: (answer) => contactRequestAnswered(peer, answer, opts),
  });
  if (!held) {
    reachSay('The request could not be sent yet: your settings on this device are still loading. Try again in a moment.');
    return false;
  }
  ws.send(JSON.stringify(built.recipientPut));
  return true;
}

/**
 * The server's answer to my contact request (10l). Taken: the pass I gave
 * them is recorded (chat-social.js settlePassPut) and from here I follow them.
 * Refused or unanswered: nothing is recorded and I do not follow them; the
 * person is told, except for a reach refusal, which onReachRefused already
 * explains (they are not taking requests).
 */
function contactRequestAnswered(peer, answer, opts) {
  if (answer && answer.taken) {
    const store = reachStore();
    if (store) store.setFollowing(peer, true);
    if (typeof myFollowing !== 'undefined') myFollowing.add(peer);
    if (typeof updateFriendIndicators === 'function') updateFriendIndicators();
    reachSay('Contact request sent. They will see only your name; if they accept, you become friends.');
  } else if (answer && answer.outcome === 'refused' && answer.reason !== 'reach') {
    reachSay(`Your contact request to ${reachDisplayName(peer)} was not delivered. Try again later.`);
  } else if (answer && answer.outcome === 'timeout') {
    reachSay(`This server did not say whether your contact request to ${reachDisplayName(peer)} arrived, so it was not counted as sent. Try again later.`);
  }
  if (opts && typeof opts.onAnswer === 'function') opts.onAnswer(answer);
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
  // A refused contact request (only "Nobody" refuses one; the relay marks it `request: true`,
  // 2026-10-10): say they are not taking requests, once, and never offer one to them again
  // this session, where the offer used to come back each minute (the desktop app does the same).
  if (msg.request === true) {
    // Every offer already on screen for them goes too: its Send request
    // button would only ask again (the desktop app replaces its one notice
    // with these words the same way).
    reachRetireOffers(to);
    if (reachNotTaking.has(to)) return;
    reachNotTaking.add(to);
    reachSay(`Not delivered to ${reachDisplayName(to)}. ${REACH_NOT_TAKING_REQUESTS}`);
    return;
  }
  if (reachNotTaking.has(to)) return;
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
  const offer = { text, btn };
  btn.onclick = async () => {
    // They said no to requests since this was drawn: nothing is sent.
    if (reachNotTaking.has(to) || offer.retired) return;
    btn.disabled = true;
    // "Request sent" only once the server took it (10l); a request it refused
    // or never answered can be sent again from here.
    const ok = await sendContactRequest(to, {
      onAnswer: (answer) => {
        if (offer.retired) return; // a refusal of the request took the button away
        btn.textContent = answer && answer.taken ? 'Request sent' : 'Send request';
        btn.disabled = !!(answer && answer.taken);
      },
    });
    if (offer.retired) return; // a refusal of the request arrived while it was on its way
    if (!ok) {
      btn.textContent = 'Send request';
      btn.disabled = false;
    } else if (btn.disabled && btn.textContent === 'Send request') {
      btn.textContent = 'Sending...';
    }
  };
  el.appendChild(status);
  el.appendChild(text);
  el.appendChild(btn);
  if (!reachOffers.has(to)) reachOffers.set(to, []);
  reachOffers.get(to).push(offer);
  if (typeof appendMessage === 'function') appendMessage(el);
}

/**
 * They are not taking contact requests (a `reach_refused` with `request:
 * true`): every Send request offer drawn for them earlier says so instead, and
 * its button goes, so it cannot ask again.
 */
function reachRetireOffers(to) {
  const list = reachOffers.get(to) || [];
  for (const offer of list) {
    offer.retired = true;
    offer.text.textContent = REACH_NOT_TAKING_REQUESTS;
    offer.btn.disabled = true;
    offer.btn.style.display = 'none';
    offer.btn.onclick = null;
  }
  reachOffers.delete(to);
}

// ── Drawing ──

/** The Requests list (name only, Accept, Ignore and Block), shared by the Safety page and the DMs tab. */
function contactRequestsHtml(requests, opts) {
  const compact = !!(opts && opts.compact);
  if (!requests.length) {
    return compact ? '' : '<div style="color:var(--text-muted);font-size:var(--text-sm);">No requests.</div>';
  }
  // In the narrow DMs rail the name takes its own line and the buttons sit under it.
  // With the protected setup on, Accept says it needs the PIN (10h).
  const accept = (typeof protectedAcceptLabel === 'function' && protectedAcceptLabel()) || 'Accept';
  return requests.map((r) =>
    `<div class="reach-request${compact ? ' dm-item' : ''}" data-req-id="${reachEsc(r.id)}" style="display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) ${compact ? 'var(--space-md);flex-wrap:wrap' : '0'};">`
    + `<span class="dm-name" style="flex:1 1 ${compact ? '100%' : '0'};min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${reachEsc(r.key ? reachDisplayName(r.key) : r.name)}</span>`
    + `<button class="vr-btn" data-req-accept="${reachEsc(r.id)}" style="font-size:0.7rem;">${reachEsc(accept)}</button>`
    + `<button class="vr-btn" data-req-ignore="${reachEsc(r.id)}" style="font-size:0.7rem;">Ignore</button>`
    + `<button class="vr-btn" data-req-block="${reachEsc(r.id)}" title="Block them: you will not see anything from them, and they are not told." style="font-size:0.7rem;color:var(--danger);">Block</button>`
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
  container.querySelectorAll('[data-req-block]').forEach((b) => {
    b.onclick = (e) => { if (e) e.stopPropagation(); blockContactRequest(b.dataset.reqBlock); };
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
  // "People I choose" (10c-ii): each friend I have given a pass (or one on its
  // way, or a choice of mine, chat-dm-store.js passFriends), once, with the
  // ticks I chose for them (10n: the choice kept per friend, the same on all
  // my devices, chat-dm-store.js choiceMay), so a refused pass never puts the
  // ticks back. The rows' audiences as shown (a choice being saved included)
  // decide which rows the ticks count for.
  const given = store ? store.passFriends() : [];
  const shown = {};
  for (const r of rows) shown[r.kind] = r.audience;
  // A friend whose pass another of my devices withdrew (10m R3) is drawn
  // "(updating their pass)" too, with no tick given (chat-social.js
  // friendTicks), but their ticks stay free: a tick made here is the person's
  // own choice, and clears the mark. Only a pass being minted or waiting for
  // the server's answer holds the ticks still (`held`).
  const chosen = given.map((key) => {
    const held = typeof friendPassUpdating === 'function' && friendPassUpdating(key);
    const elsewhere = typeof friendPassChangedElsewhere === 'function' && friendPassChangedElsewhere(key);
    return {
      key,
      name: reachDisplayName(key),
      ticks: typeof friendTicks === 'function' ? friendTicks(key) : reachTicksFromMay(store.choiceMay(key)),
      updating: held || elsewhere,
      held,
    };
  }).sort((a, b) => a.name.localeCompare(b.name));
  return {
    known,
    rows,
    chosen,
    ticksInUse: reachTicksInUse(shown),
    requests: store ? store.contactRequestList() : [],
    // Reports about my groups (10j, chat-reports.js); null when there is nothing to show.
    groupReports: typeof groupReportsModel === 'function' ? groupReportsModel() : null,
    // Blocked people (step C): newest first, by the member list's name (or short key).
    blocked: store ? store.blockedList().map((b) => ({ key: b.key, name: reachDisplayName(b.key), ts: b.ts, date: blockDateLabel(b.ts) })) : [],
    // Warnings on messages (step F, chat-warnings.js): On by default; the
    // switch waits for the local store, where it is kept.
    warningsOn: typeof messageWarningsOn === 'function' ? messageWarningsOn() : true,
    warningsReady: !!store,
  };
}

const SAFETY_H3 = 'font-size:0.85rem;margin:var(--space-lg) 0 var(--space-xs);color:var(--text);';
const SAFETY_NOTE = 'color:var(--text-muted);font-size:var(--text-sm);line-height:1.4;margin:0 0 var(--space-sm);';

/** The Safety page's HTML for a model (pure, so it can be checked without a browser). */
function safetyPanelHtml(model) {
  let html = '<div style="display:flex;align-items:center;justify-content:space-between;gap:var(--space-sm);">'
    + '<h2 style="margin:0;">Safety</h2>'
    + '<button class="vr-btn" data-safety-close style="font-size:0.75rem;">Close</button></div>';
  // The protected setup's always-visible line, at the top while it is on (10h).
  if (typeof protectedStatusLineHtml === 'function') html += protectedStatusLineHtml('safety');
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
  // "People I choose" (10c-ii): one line saying what the ticks are for, one
  // saying which rows use them now, then each friend once with three ticks.
  // A row wraps on a narrow screen: the name above, the ticks under it.
  html += `<h3 style="${SAFETY_H3}">People I choose</h3>`
    + `<p class="safety-ticks-note" style="${SAFETY_NOTE}">${reachEsc(REACH_TICKS_NOTE)}</p>`
    + `<p class="safety-ticks-use" style="${SAFETY_NOTE}">${reachEsc(model.ticksInUse)}</p>`;
  if (model.chosen.length) {
    html += model.chosen.map((c) =>
      `<div class="safety-chosen" data-chosen-key="${reachEsc(c.key)}" style="display:flex;flex-wrap:wrap;align-items:center;gap:var(--space-xs) var(--space-md);padding:var(--space-xs) 0;border-top:1px solid var(--border);">`
      + `<span style="flex:1 1 8rem;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text);">${reachEsc(c.name)}`
      + (c.updating ? ` <span style="color:var(--text-muted);font-size:var(--text-sm);">(updating their pass)</span>` : '')
      + '</span>'
      + REACH_KINDS.map((kind) =>
        '<label style="display:inline-flex;align-items:center;gap:var(--space-xs);color:var(--text);cursor:pointer;">'
        + `<input type="checkbox" data-tick-key="${reachEsc(c.key)}" data-tick-kind="${kind}"`
        + ` aria-label="${reachEsc(c.name)}: ${reachEsc(REACH_TICK_LABELS[kind])}"`
        + `${c.ticks[kind] ? ' checked' : ''}${c.held ? ' disabled' : ''}>`
        + `<span>${reachEsc(REACH_TICK_LABELS[kind])}</span></label>`).join('')
      + '</div>').join('');
  } else {
    html += `<p style="${SAFETY_NOTE}">Friends appear here once you have some.</p>`;
  }
  html += `<h3 style="${SAFETY_H3}">Requests</h3>`
    + `<p style="${SAFETY_NOTE}">People who asked to reach you. You see only their name. Accept makes you friends; Ignore tells no one.</p>`
    + contactRequestsHtml(model.requests);
  // Reports about your groups (10j, chat-reports.js): when there are any, or I created a group.
  if (model.groupReports && typeof groupReportsSafetyHtml === 'function') html += groupReportsSafetyHtml(model.groupReports);
  html += `<h3 style="${SAFETY_H3}">Blocked people</h3>`
    + `<p style="${SAFETY_NOTE}">You see nothing from the people here, on any of your devices, and they are not told. Blocking does not stop them seeing what you post in public. Unblock lets them reach you again as your settings above allow; it does not make you friends again.</p>`;
  if (model.blocked.length) {
    html += model.blocked.map((b) =>
      `<div class="safety-blocked" data-blocked-key="${reachEsc(b.key)}" style="display:flex;align-items:center;gap:var(--space-sm);padding:var(--space-xs) 0;">`
      + '<div style="flex:1;min-width:0;">'
      + `<div style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text);">${reachEsc(b.name)}</div>`
      + `<div style="color:var(--text-muted);font-size:var(--text-sm);">Blocked ${reachEsc(b.date)}</div></div>`
      + `<button class="vr-btn" data-unblock="${reachEsc(b.key)}" style="font-size:0.7rem;flex:none;">Unblock</button></div>`).join('');
  } else {
    html += `<p style="${SAFETY_NOTE}">Nobody is blocked.</p>`;
  }
  html += safetyWarningsHtml(model);
  // The protected setup (10h, chat-protected.js): its own section, last.
  if (typeof protectedSafetyHtml === 'function') html += protectedSafetyHtml();
  return html;
}

/**
 * Warnings on messages (step F, 2026-10-10, blocking-and-safe-mode.md 10g):
 * the switch, what it does, that it runs on this device only (6.5), and the
 * two rules that have no switch. Empty when /shared/warnings.js is not loaded.
 */
function safetyWarningsHtml(model) {
  if (typeof WARNINGS_SWITCH_LABEL !== 'string') return '';
  return `<h3 style="${SAFETY_H3}">${reachEsc(WARNINGS_SWITCH_LABEL)}</h3>`
    + '<label class="safety-warnings" style="display:flex;align-items:center;gap:var(--space-sm);color:var(--text);cursor:pointer;">'
    + `<input type="checkbox" data-warnings-switch aria-label="${reachEsc(WARNINGS_SWITCH_LABEL)}"${model.warningsOn ? ' checked' : ''}${model.warningsReady ? '' : ' disabled'}>`
    + `<span>${reachEsc(WARNINGS_SWITCH_LABEL)}</span></label>`
    + (model.warningsReady ? '' : `<p style="${SAFETY_NOTE}margin-top:var(--space-xs);">Waiting for your settings on this device to load.</p>`)
    + `<p style="${SAFETY_NOTE}margin-top:var(--space-xs);">${reachEsc(WARNINGS_SWITCH_HELP)}</p>`
    + `<p style="${SAFETY_NOTE}">${reachEsc(WARNINGS_ON_DEVICE_SENTENCE)}</p>`
    + `<p style="${SAFETY_NOTE}">Links in a direct message from someone who is not your friend open only when you press Open under it.</p>`
    + `<p style="${SAFETY_NOTE}">Your recovery phrase is never sent: anything you write that holds it is stopped before it leaves this device. This is always on.</p>`;
}

/**
 * A tick on the "People I choose" list changed: it is my new choice for that
 * friend (chat-social.js setFriendTick, 10n), kept here and sent to my other
 * devices, and their pass follows it. The page is drawn again at once, so the
 * friend's ticks are held still while their pass is minted and while it waits
 * for the server's answer (10l), and again when that comes (chat-social.js
 * settlePassPut), showing my choice: a pass the server refused is sent again
 * by the next sweep, and the ticks stay as chosen meanwhile. With no server
 * connected the choice is kept and goes on the next connection. Returns true
 * when the choice was made.
 */
async function chooseFriendTick(peer, kind, on) {
  // With the protected setup on, the PIN first (10h): a cancelled prompt puts
  // the tick back as it was and says nothing else. The PIN opens this one
  // change only: setFriendTick takes it, and it is gone when this returns.
  if (typeof protectedNeedsPin === 'function' && protectedNeedsPin('reach_tick')) {
    let asked = false;
    const done = await protectedAskThen('reach_tick', () => { asked = true; return chooseFriendTick(peer, kind, on); });
    if (!asked) renderSafetyPanel();
    return done;
  }
  const pending = setFriendTick(peer, kind, on);
  renderSafetyPanel();
  const ok = await pending.catch(() => false);
  // (Since 10n a choice is kept even when their pass cannot go yet: their key
  // not known here, one still waiting for an answer, or no server connected;
  // the sweep sends it. Only a page whose settings have not loaded refuses.)
  if (!ok) reachSay('Could not change that now: your settings on this device are still loading. Try again in a moment.');
  renderSafetyPanel();
  return ok;
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
  card.querySelectorAll('input[data-tick-key]').forEach((box) => {
    box.onchange = () => chooseFriendTick(box.dataset.tickKey, box.dataset.tickKind, box.checked);
  });
  wireContactRequestButtons(card);
  if (typeof wireGroupReportButtons === 'function') wireGroupReportButtons(card);
  card.querySelectorAll('[data-unblock]').forEach((b) => {
    b.onclick = () => { b.disabled = true; unblockKey(b.dataset.unblock); };
  });
  const warningsSwitch = card.querySelector('[data-warnings-switch]');
  if (warningsSwitch && typeof setMessageWarningsOn === 'function') {
    warningsSwitch.onchange = () => setMessageWarningsOn(warningsSwitch.checked);
  }
  if (typeof wireProtectedSafety === 'function') wireProtectedSafety(card);
}

// ── Block (step C, 2026-10-09) ───────────────────────────────────────────
// docs/design/blocking-and-safe-mode.md 10d, mirroring the desktop app. A
// block is kept on my own devices only (section 4.4, option A); the relay
// does its part because blocking withdraws every pass I gave them, so under
// the safe defaults it refuses their messages, calls and trades from then on.
//
// Block, at once and without a confirmation (it is undoable):
//   1. puts their identity key (never a name) on the list, with the date;
//   2. withdraws every pass I gave them (`cert_revoke` for each serial, to the
//      relay) and unfollows them here, telling them nothing;
//   3. hides everything from them: DMs, knocks, follow notices, passes and
//      contact requests are dropped before they are stored or notified
//      (blockScreenDm, called by app.js and chat-p2p.js); channel posts,
//      replies, group messages, reactions and typing are hidden by key
//      (app.js, chat-messages.js, and blockScreenFrame below); a ring is
//      ignored with no reject sent (chat-voice-calls.js); a direct-connection
//      offer is not answered (chat-p2p.js mayAnswerDirectOffer);
//   4. tells my other devices with a sealed note to myself only,
//      [[hum:block:v1]]<key> (/shared/block.js), and shows one line.
// Unblock takes them off the list and sends [[hum:unblock:v1]]<key> the same
// way. It does not follow them again or give them a pass: being friends again
// is a fresh follow or contact request.

function blockSameKey(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a !== '' && a.toLowerCase() === b.toLowerCase();
}

/** The date a person was blocked, as the Safety page shows it. */
function blockDateLabel(ts) {
  const d = new Date(Number(ts) || 0);
  try { return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
  catch { return d.toISOString().slice(0, 10); }
}

/** The member list's mark on a person: dimmed and struck through while blocked. */
function markPeerBlocked(el, blocked) {
  if (!el || !el.style) return;
  let indicator = typeof el.querySelector === 'function' ? el.querySelector('.block-indicator') : null;
  if (blocked && !indicator && typeof document.createElement === 'function') {
    const span = document.createElement('span');
    span.className = 'block-indicator';
    span.title = 'Blocked';
    span.style.fontSize = '0.65rem';
    span.innerHTML = ' ' + (typeof hosIcon === 'function' ? hosIcon('block', 14) : '');
    el.appendChild(span);
  } else if (!blocked && indicator) {
    indicator.remove();
  }
  el.style.textDecoration = blocked ? 'line-through' : '';
  el.style.opacity = blocked ? '0.5' : '';
}

/**
 * Hide (or show again) what is already on screen from `key`: posts, replies
 * and group messages, their reactions, the member list's mark, the open DM's
 * header. What arrives while they are blocked is never drawn at all, so after
 * an Unblock it appears the next time the channel is opened.
 */
function applyBlockToView(key, hidden) {
  if (typeof document === 'undefined' || typeof document.querySelectorAll !== 'function') return;
  document.querySelectorAll('.message[data-from]').forEach((el) => {
    if (el && el.dataset && blockSameKey(el.dataset.from, key)) el.style.display = hidden ? 'none' : '';
  });
  if (typeof messageReactions !== 'undefined' && messageReactions && typeof renderReactions === 'function') {
    for (const rKey of Object.keys(messageReactions)) {
      const at = rKey.lastIndexOf(':');
      if (at > 0) renderReactions(rKey.slice(0, at), Number(rKey.slice(at + 1)));
    }
  }
  document.querySelectorAll('.peer[data-pubkey]').forEach((el) => {
    if (el && el.dataset && blockSameKey(el.dataset.pubkey, key)) markPeerBlocked(el, hidden);
  });
  if (typeof activeDmPartner !== 'undefined' && blockSameKey(activeDmPartner, key) && typeof renderDmHeader === 'function') {
    renderDmHeader();
  }
}

/** Redraw every list a block touches. */
function renderBlockEverywhere() {
  // The member list as the page draws it (chat-voice-rooms.js), with its marks.
  if (typeof renderPresenceSidebarForActiveContext === 'function') {
    try { renderPresenceSidebarForActiveContext(); } catch (e) { /* the member list is not drawn yet */ }
  }
  if (typeof updateFriendIndicators === 'function') {
    try { updateFriendIndicators(); } catch (e) { /* the member list is not drawn yet */ }
  }
  renderRequestsEverywhere();
}

/**
 * Block `key` on this device: the list, my passes withdrawn, the follow
 * dropped here (no notice to them), any request from them gone, and the
 * screen. Shared by Block and by a block note from another of my devices.
 * Returns false when they were already blocked.
 */
function blockLocally(key, ts) {
  const store = reachStore();
  if (!store || !store.setBlocked(key, true, ts)) return false;
  // Block never needs the PIN; with the protected setup on, being friends again needs it (10h).
  if (typeof protectedForget === 'function') protectedForget(key);
  store.setFollowing(key, false);
  if (typeof myFollowing !== 'undefined' && myFollowing) myFollowing.delete(key);
  // My choice for them is cleared whatever its time (10n N3; a choice note
  // about someone I blocked is ignored, so every device ends cleared).
  if (typeof withdrawPassesTo === 'function') withdrawPassesTo(key, ts, true);
  store.removeContactRequest(key);
  applyBlockToView(key, true);
  renderBlockEverywhere();
  return true;
}

/** Unblock `key` on this device. Returns false when they were not blocked. */
function unblockLocally(key) {
  const store = reachStore();
  if (!store || !store.setBlocked(key, false)) return false;
  applyBlockToView(key, false);
  renderBlockEverywhere();
  return true;
}

/**
 * Send the notes to myself that have not gone yet (queued by Block and
 * Unblock, and on every connection). A note goes to my own mailbox only, so
 * my other devices learn of it; the blocked person is sent nothing. One send
 * runs at a time, so a quick Block then Unblock go out once each, in order.
 */
let blockNotesSending = Promise.resolve();
function flushBlockNotes() {
  blockNotesSending = blockNotesSending.then(sendPendingBlockNotes, sendPendingBlockNotes);
  return blockNotesSending;
}

async function sendPendingBlockNotes() {
  const store = reachStore();
  if (!store || !ws || ws.readyState !== WebSocket.OPEN) return;
  for (const note of store.blockNotesPending.slice()) {
    const text = blockNoteText(note.action, note.key);
    if (!text) { store.blockNoteSent(note.action, note.key); continue; }
    // Signed with the time it was made (one sent late keeps its own), which is
    // the date my other devices show and the time they clear my choice for
    // them as of (10n).
    const built = typeof pqBuildSelfNote === 'function' ? await pqBuildSelfNote(text, note.at) : null;
    if (!built || !ws || ws.readyState !== WebSocket.OPEN) return; // tried again on the next connection
    ws.send(JSON.stringify(built.put));
    store.blockNoteSent(note.action, note.key);
  }
}

/** Block someone (the button and the command). Returns true when they are blocked now. */
async function blockKey(rawKey) {
  const key = blockKeyNorm(rawKey);
  if (!key) { reachSay('That is not someone this client can block.'); return false; }
  if (blockSameKey(key, myKey)) { reachSay("You can't block yourself."); return false; }
  const store = reachStore();
  if (!store) { reachSay('Your block list is still loading. Try again in a moment.'); return false; }
  if (store.isBlocked(key)) { reachSay(`${reachDisplayName(key)} is already blocked.`); return true; }
  const at = Date.now();
  blockLocally(key, at);
  store.queueBlockNote('block', key, at);
  await flushBlockNotes();
  reachSay(BLOCKED_LINE);
  return true;
}

/** Unblock someone. Returns true when they are not blocked now. */
async function unblockKey(rawKey) {
  const key = blockKeyNorm(rawKey);
  const store = reachStore();
  if (!key || !store) return false;
  if (!store.isBlocked(key)) { reachSay(`${reachDisplayName(key)} is not blocked.`); return true; }
  unblockLocally(key);
  store.queueBlockNote('unblock', key, Date.now());
  await flushBlockNotes();
  reachSay(UNBLOCKED_LINE);
  return true;
}

/** Block the person behind a contact request (Block instead of Ignore). */
function blockContactRequest(id) {
  const store = reachStore();
  const req = store && store.contactRequests[id];
  return blockKey((req && req.key) || id);
}

/**
 * The key for a name typed after /block or /unblock: the member list's name
 * (letter case aside), a whole key, or, for /unblock, the name or short key
 * a blocked person is listed under. Null when there is no such person.
 */
function blockKeyForName(name, blockedOnly) {
  const want = String(name || '').trim().replace(/^@/, '');
  if (!want) return null;
  const asKey = blockKeyNorm(want);
  if (asKey && asKey.length >= 64) return asKey;
  const lower = want.toLowerCase();
  const store = reachStore();
  if (blockedOnly && store) {
    for (const b of store.blockedList()) {
      if (reachDisplayName(b.key).toLowerCase() === lower || b.key.startsWith(lower)) return b.key;
    }
    return null;
  }
  for (const [key, p] of Object.entries(reachPeers())) {
    if (p && typeof p.display_name === 'string' && p.display_name.toLowerCase() === lower) return key;
  }
  return null;
}

/** `/block <name>`. */
function blockByName(name) {
  const key = blockKeyForName(name, false);
  if (!key) { reachSay(`No one called "${name}" is in the member list.`); return Promise.resolve(false); }
  return blockKey(key);
}

/** `/unblock <name>`. */
function unblockByName(name) {
  const key = blockKeyForName(name, true);
  if (!key) { reachSay(`You have not blocked anyone called "${name}".`); return Promise.resolve(false); }
  return unblockKey(key);
}

/** `/blocklist`: who is blocked, and where to change it. */
function showBlockList() {
  const store = reachStore();
  const list = store ? store.blockedList() : [];
  if (!list.length) { reachSay('You have not blocked anyone.'); return; }
  reachSay('Blocked: ' + list.map((b) => `${reachDisplayName(b.key)} (since ${blockDateLabel(b.ts)})`).join(', ')
    + '. Unblock with /unblock <name>, or in Safety under Blocked people.');
}

/**
 * Apply a note I sent myself (from another of my devices, or this one's own
 * coming back). A block note's date is when it was written, so every device
 * shows the same one.
 */
function applyBlockNote(note, ts) {
  if (!note || !note.key) return;
  if (note.action === 'block') blockLocally(note.key, Number(ts) || Date.now());
  else if (note.action === 'unblock') unblockLocally(note.key);
}

/**
 * Screen an opened, signature-checked DM (app.js dm_new and dm_batch,
 * chat-p2p.js). Returns true when the caller must neither store nor show it:
 * a block note (acted on only when I sent it to myself; a note addressed to
 * anyone else is ignored), or anything at all from someone I blocked.
 */
function blockScreenDm(inner) {
  if (!inner) return false;
  // (Guarded: on the DM path a missing /shared/block.js must not stop mail.)
  if (typeof isBlockNoteText === 'function' && isBlockNoteText(inner.text)) {
    const note = blockNoteFromSelf(inner, myKey);
    if (note) applyBlockNote(note, inner.ts);
    return true;
  }
  return !!(inner.from && !blockSameKey(inner.from, myKey) && isBlockedKey(inner.from));
}

/**
 * Screen a frame from the relay before anything else sees it: a post, typing,
 * a reaction or a ring from someone I blocked is dropped whole, so no handler
 * draws it and no notification fires. (Each handler also checks the key
 * itself, so the order of the handleMessage wrappers is not load-bearing.)
 */
function blockScreenFrame(msg) {
  if (!msg || typeof msg.from !== 'string' || !msg.from) return false;
  switch (msg.type) {
    case 'chat':
    case 'typing':
    case 'reaction':
      return isBlockedKey(msg.from);
    case 'voice_call':
      return msg.action === 'ring' && isBlockedKey(msg.from);
    case 'webrtc_signal':
      return msg.signal_type === 'dc_offer' && isBlockedKey(msg.from);
    default:
      return false;
  }
}

/** The store has loaded (app.js): hide what was drawn before, and send notes owed. */
function onBlockListLoaded() {
  const store = reachStore();
  if (!store) return;
  for (const b of store.blockedList()) applyBlockToView(b.key, true);
  // Warnings drawn before the store loaded used the default (On): follow the kept switch (step F).
  if (typeof applyWarningsSwitchToView === 'function') applyWarningsSwitchToView();
  renderBlockEverywhere();
  flushBlockNotes();
}

// The relay's frames for this section.
const _origHandleMessageReach = handleMessage;
handleMessage = function (msg) {
  if (blockScreenFrame(msg)) return;
  if (msg && msg.type === 'reach_settings') { onReachSettings(msg.settings); return; }
  if (msg && msg.type === 'reach_refused') { onReachRefused(msg); return; }
  return _origHandleMessageReach(msg);
};

window.openSafetyPanel = openSafetyPanel;
window.receiveContactRequest = receiveContactRequest;
window.ingestContactRequest = ingestContactRequest;
window.reachScreenDm = reachScreenDm;
window.acceptContactRequest = acceptContactRequest;
window.ignoreContactRequest = ignoreContactRequest;
window.sendContactRequest = sendContactRequest;
window.chooseReachAudience = chooseReachAudience;
window.blockKey = blockKey;
window.unblockKey = unblockKey;
window.blockByName = blockByName;
window.unblockByName = unblockByName;
window.showBlockList = showBlockList;
window.blockContactRequest = blockContactRequest;
window.blockScreenDm = blockScreenDm;
window.onBlockListLoaded = onBlockListLoaded;
window.flushBlockNotes = flushBlockNotes;

window.maybeShowPrivacyTierModal = maybeShowPrivacyTierModal;
window.applyPrivacyTier = applyPrivacyTier;
window.reassertPrivacyTier = reassertPrivacyTier;
window.exportMyAccountData = exportMyAccountData;
window.deleteMyAccount = deleteMyAccount;
window.eraseMemorySentence = eraseMemorySentence;
