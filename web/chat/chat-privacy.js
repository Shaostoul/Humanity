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
// one click from where the user manages who they are. The relay's
// account_export_data reply triggers a JSON download (app.js).

function exportMyAccountData() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'account_export' }));
    if (typeof addSystemMessage === 'function') addSystemMessage('Export requested; the download starts when the server replies.');
  }
}

async function deleteMyAccount() {
  if (!await holdConfirm('Erase your entire account on this server (messages, uploads, profile, mailbox, membership)? This is permanent. Data on your own devices stays.', { seconds: 5, confirmLabel: 'Hold to erase account' })) return;
  const typed = prompt(
    'This ERASES your account on this server: messages, uploads, profile, follows, '
    + 'mailbox, and membership, permanently. Data on your own devices stays.\n\n'
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
    '<button class="vr-btn" style="flex:1;font-size:0.7rem;" onclick="exportMyAccountData()" title="Download everything this server stores about you as a JSON file.">Export my data</button>'
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

window.maybeShowPrivacyTierModal = maybeShowPrivacyTierModal;
window.applyPrivacyTier = applyPrivacyTier;
window.reassertPrivacyTier = reassertPrivacyTier;
window.exportMyAccountData = exportMyAccountData;
window.deleteMyAccount = deleteMyAccount;
window.setRelayCallsOnly = setRelayCallsOnly;
