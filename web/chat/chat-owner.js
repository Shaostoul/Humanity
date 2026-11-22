// chat-owner.js: the server's owner in the website's chat (2026-11-20, docs/design/hide-and-keep.md,
// "How the owner role is set"). The relay half is src/relay/handlers/owner.rs.
//
// WHY THE OWNER MATTERS. Hide and keep seals every held copy to the server owner's key card (and to
// the successor's too, when the owner has named one), so the owner is the one person whose device
// can ever open one. The operator, 2026-11-20: "We just want to make sure the system itself can't
// be abused." So the role is never an admin's to grant.
//
// OWNER CHANGES HAPPEN ONLY IN THE DESKTOP APP (the owner branch's seventh review, M2, 2026-11-21;
// a deliberate parity exception, written down in docs/design/hide-and-keep.md and
// docs/design/key-cards.md). Handing over ownership, naming a successor or removing one, and a
// recipient's check and acceptance of such a change, all rest on comparing what the server says
// (a member's key, its fingerprint, the new seal of every held copy) against what the people
// involved read on their own screens. This page is served BY the server being checked: a server
// that was not honest could send a page that shows the wrong fingerprint, or a check that always
// matches. Only the desktop app makes that comparison apart from the server. So this page:
//
//   1. CLAIM OWNERSHIP, for anyone signed in, only while the relay says `owner_state` has_owner
//      false. The relay prints a one-time claim code to its own log (and data/owner-claim-code.txt
//      on the server machine), so only someone with access to that machine can claim. The code is
//      pasted here and sent as `owner_claim`; the relay's `owner_reply` (kind claim) words are
//      shown marked as the server's. A line in the chat with a "Claim ownership" button opens it,
//      and so do `/claim` alone and the command palette. `/claim <code>` typed in the chat goes to
//      the server alone as `owner_claim` with `typed: true` (app.js `takeTypedClaim`).
//   2. READ ONLY, in Held copies (chat-held.js draws what `ownerHeldNodes` returns): who the owner
//      is (you, or the name the member list gives, when an honest relay could have listed it), who
//      the successor is, the owner log, and, for the owner, why the controls are not here: in words,
//      "this page comes from this server, so it cannot check this server; use the desktop app".
//      Held copies itself shows the held count.
//   3. A RECIPIENT'S CHECK (`owner_change_check`) puts a line in the chat in the app's own words
//      saying the desktop app does the check, with none of the frame's text in it (the seventh
//      review's G1 and G2: a free-text item id reached the prompt's words); nothing is fetched,
//      signed or sent, and the relay asks again when the desktop app connects. An
//      `owner_change_ready` is the answer to a begin this page never sends: logged and dropped.
//   4. MY OWN FINGERPRINT in the Account and Identity menu (/shared/own-fingerprint.js), with words
//      that it is only as trustworthy as the server that sent the page.
//
// Built as DOM: createElement, textContent and addEventListener only. No innerHTML and no data-*
// handlers (BUG-331's safe-markup rules). Words by key (data/i18n/en.json, chat_owner).
// Depends on chat globals: ws, myKey, peerData, window.HosKeyCard, handleMessage, appendMessage and
// addServerMessage; and chat-held.js, loaded before it, for Held copies (window.heldRender).
// Tested by scripts/tests/owner-chat-web.test.js.

(function () {
  'use strict';

  /** The owner log's kinds and their words. */
  const LOG_WORDS = {
    claim: 'chat_owner.log_claim',
    owner_handover: 'chat_owner.log_owner_handover',
    owner_successor: 'chat_owner.log_owner_successor',
    owner_successor_remove: 'chat_owner.log_owner_successor_remove',
  };

  const owner = {
    hasOwner: null,       // from owner_state; null until the relay says
    claimOpen: false,     // the Claim ownership window
    code: '',             // the claim code being pasted
    claimStatus: '',      // our words on the last claim, or the relay's marked as the server's
    notice: null,         // the button of the chat line saying the server has no owner
    checkLines: 0,        // check lines drawn (one per round asked), for the tests
    checkRounds: new Set(), // rounds already told about (only ever compared, never drawn)
  };

  /** The vendored hash bundle alone, for my own fingerprint at startup (the sixth review's H1 (a)):
   *  never waiting on a frame from the server. */
  let _noble = null;
  async function hashMod() {
    if (!_noble) _noble = await import('/shared/vendor/noble-pq.bundle.js');
    return _noble;
  }

  function t(key, vars) { return (typeof i18n !== 'undefined' && i18n && typeof i18n.t === 'function') ? i18n.t(key, vars) : key; }
  function hint(node, key) { if (typeof i18n !== 'undefined' && i18n && typeof i18n.hint === 'function') i18n.hint(node, key); }
  /** Body text with its glossary marks drawn (i18n.fill builds each as a [data-term] span). */
  function marked(node, key) {
    if (typeof i18n !== 'undefined' && i18n && typeof i18n.fill === 'function') i18n.fill(node, key);
    else node.textContent = t(key);
    return node;
  }
  /** The server's own words, marked as the server's (app.js addServerMessage's words), one line. */
  function fromServer(text) { return t('chat.from_server', { text: String(text == null ? '' : text).replace(/\s+/g, ' ').trim() }); }

  function make(tag, cls, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }
  function button(textKey, hintKey, onClick, cls) {
    const b = make('button', 'vr-btn' + (cls ? ' ' + cls : ''), t(textKey));
    b.type = 'button';
    hint(b, hintKey);
    b.addEventListener('click', onClick);
    return b;
  }
  function muted(node) {
    node.style.color = 'var(--text-muted)';
    node.style.fontSize = 'var(--text-sm)';
    return node;
  }
  function row(cls) {
    const r = make('div', cls);
    r.style.display = 'flex';
    r.style.flexWrap = 'wrap';
    r.style.gap = 'var(--space-xs)';
    r.style.alignItems = 'flex-end';
    r.style.margin = 'var(--space-xs) 0';
    return r;
  }
  /** A control with its name above it, so every box says what it is for. */
  function labelled(labelKey, control, vars) {
    const lab = make('label', 'owner-field');
    lab.style.display = 'flex';
    lab.style.flexDirection = 'column';
    lab.style.gap = 'var(--space-xs)';
    lab.appendChild(muted(make('span', 'owner-field-name', t(labelKey, vars))));
    lab.appendChild(control);
    return lab;
  }
  function textBox(cls, labelKey, hintKey, value, onInput, maxLength) {
    const box = make('input', cls);
    box.type = 'text';
    box.maxLength = maxLength || 500;
    box.setAttribute('aria-label', t(labelKey));
    hint(box, hintKey);
    box.value = value || '';
    box.addEventListener('input', () => onInput(box.value));
    return box;
  }

  function socketOpen() { return typeof ws !== 'undefined' && ws && ws.readyState === 1; }
  function send(frame) { ws.send(JSON.stringify(frame)); }
  function when(ms) {
    if (!ms) return '';
    const d = new Date(ms);
    try { return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
    catch (_) { return d.toISOString().slice(0, 10); }
  }
  function peers() { return (typeof peerData !== 'undefined' && peerData) ? peerData : {}; }
  function me() { return typeof myKey === 'string' ? myKey : ''; }
  /** A name as the relay gave it, when an honest relay could have listed it (/shared/key-card.js
   *  honestName, the third review's M2); a refused name in words otherwise, never the relay's text. */
  function honestOr(raw) {
    const K = window.HosKeyCard;
    return K && typeof K.honestName === 'function' && K.honestName(raw) ? String(raw) : t('owner.name_refused');
  }
  /** A person's name as the member list shows it (honest, or a refused name); the start of their
   *  key when none is listed. */
  function nameOf(key) {
    const p = peers()[key];
    const raw = String((p && (p.display_name || p.name)) || '');
    return raw ? honestOr(raw) : String(key || '').slice(0, 8);
  }
  /** The key the member list names as the owner ('' when none does). */
  function listedOwner() {
    const all = peers();
    return Object.keys(all).find((k) => all[k] && all[k].role === 'owner') || '';
  }

  // ── Claim ownership ──

  /** `owner_reply`: the relay's words on a claim, marked as the server's. Any other kind answers a
   *  change this page never makes (they are the desktop app's), and is only logged. */
  function onReply(msg) {
    if (msg.kind !== 'claim') { console.warn('owner_reply for a change this page does not make:', msg.kind); return; }
    const words = String(msg.message || '');
    owner.claimStatus = words ? fromServer(words) : '';
    if (msg.ok) { owner.hasOwner = true; owner.code = ''; }
    renderAll();
  }

  function claim() {
    const code = String(owner.code || '').trim();
    if (!code) { owner.claimStatus = t('chat_owner.claim_empty'); renderAll(); return false; }
    if (!socketOpen()) { owner.claimStatus = t('held.not_connected'); renderAll(); return false; }
    send({ type: 'owner_claim', code });
    // The code is the server's key to itself: not kept on the page once it is sent.
    owner.code = '';
    owner.claimStatus = t('chat_owner.claim_sending');
    renderAll();
    return true;
  }

  /** The claim box: the code field and Claim while the server has no owner, else why not. */
  function claimNodes() {
    const box = make('div', 'owner-claim');
    if (owner.hasOwner !== false) {
      box.appendChild(muted(make('p', 'owner-claim-has', t(owner.hasOwner ? 'chat_owner.claim_has_owner' : 'chat_owner.claim_unknown'))));
      if (owner.claimStatus) box.appendChild(make('p', 'owner-claim-status', owner.claimStatus));
      return box;
    }
    box.appendChild(muted(marked(make('p', 'owner-claim-intro'), 'chat_owner.claim_intro')));
    const line = row('owner-claim-row');
    const code = textBox('owner-claim-code', 'chat_owner.claim_code', 'chat_owner.claim_code_hint', owner.code, (v) => { owner.code = v; }, 64);
    code.placeholder = t('chat_owner.claim_code');
    code.autocomplete = 'off';
    code.spellcheck = false;
    code.addEventListener('keydown', (e) => { if (e && e.key === 'Enter') claim(); });
    line.appendChild(labelled('chat_owner.claim_code', code));
    line.appendChild(button('chat_owner.claim_button', 'chat_owner.claim_button_hint', () => claim()));
    box.appendChild(line);
    box.appendChild(muted(make('p', 'owner-claim-where', t('chat_owner.claim_where'))));
    box.appendChild(muted(make('p', 'owner-claim-command', t('chat_owner.claim_command'))));
    if (owner.claimStatus) {
      const s = make('p', 'owner-claim-status', owner.claimStatus);
      s.setAttribute('role', 'status');
      box.appendChild(s);
    }
    return box;
  }

  function openClaimView() {
    owner.claimOpen = true;
    renderClaim();
    return true;
  }
  function closeClaimView() { owner.claimOpen = false; renderClaim(); }

  function renderClaim() {
    if (typeof document === 'undefined' || typeof document.getElementById !== 'function') return;
    let overlay = document.getElementById('owner-claim-overlay');
    if (!owner.claimOpen) {
      if (overlay && overlay.classList) overlay.classList.remove('open');
      return;
    }
    let card = document.getElementById('owner-claim-card');
    if (!overlay) {
      overlay = make('div', 'profile-modal-overlay');
      overlay.id = 'owner-claim-overlay';
      overlay.style.zIndex = 'var(--z-shade)';
      overlay.addEventListener('click', (e) => { if (e && e.target === overlay) closeClaimView(); });
      card = make('div', 'profile-modal');
      card.id = 'owner-claim-card';
      card.setAttribute('role', 'dialog');
      card.setAttribute('aria-label', t('chat_owner.claim_title'));
      card.addEventListener('click', (e) => { if (e && e.stopPropagation) e.stopPropagation(); });
      overlay.appendChild(card);
      if (document.body) document.body.appendChild(overlay);
    }
    if (overlay.classList) overlay.classList.add('open');
    if (!card) return;
    while (card.firstChild) card.removeChild(card.firstChild);
    const top = make('div', 'owner-claim-top');
    top.style.display = 'flex';
    top.style.justifyContent = 'space-between';
    top.style.alignItems = 'center';
    top.appendChild(make('h2', 'owner-claim-title', t('chat_owner.claim_title')));
    top.appendChild(button('held.close', 'chat_owner.close_hint', () => closeClaimView()));
    card.appendChild(top);
    card.appendChild(claimNodes());
  }

  /** The chat line on a server with no owner, with its "Claim ownership" button. */
  function showNotice() {
    if (owner.notice || typeof appLine !== 'function' || typeof document === 'undefined') return;
    // The app's own voice, by key: app.js `appLine`, the one way to the app's mark (the eighth review).
    const el = appLine('chat_owner.notice', null, { className: 'owner-unclaimed' });
    el.style.padding = 'var(--space-sm) var(--space-md)';
    const b = button('chat_owner.claim_title', 'chat_owner.open_hint', () => openClaimView());
    el.appendChild(b);
    owner.notice = b;
  }

  // ── A recipient's check: the desktop app's (the seventh review, M2) ──

  /** `owner_change_check`: one line per round in the app's own words, that the desktop app does
   *  this check; nothing of the frame is drawn (G1, G2), and nothing is fetched, signed or sent. */
  function onCheck(msg) {
    const round = msg && typeof msg.round === 'string' ? msg.round : '';
    console.warn('owner_change_check: an owner change to check, which only the desktop app does.');
    if (owner.checkRounds.has(round) || typeof appLine !== 'function' || typeof document === 'undefined') return;
    owner.checkRounds.add(round);
    const el = appLine('chat_owner.check_desktop', null, { className: 'owner-check-desktop' });
    el.style.padding = 'var(--space-sm) var(--space-md)';
    owner.checkLines += 1;
  }

  // ── In Held copies (chat-held.js) ──

  /** What Held copies says, in its "cannot keep" box, about what to do: claim the server, or wait
   *  for the owner's key card. */
  function refusalNodes() {
    if (owner.hasOwner === false) {
      return [
        make('div', 'held-refusal-today', t('chat_owner.refusal_claim')),
        button('chat_owner.claim_title', 'chat_owner.open_hint', () => openClaimView()),
      ];
    }
    if (owner.hasOwner === true) return [make('div', 'held-refusal-today', t('chat_owner.refusal_card'))];
    return [];
  }

  /** The "Server owner" part of Held copies, from the relay's held_items frame `d`: read only. */
  function heldNodes(d) {
    const nodes = [];
    nodes.push(make('h3', 'owner-title', t('chat_owner.section_title')));
    if (owner.hasOwner === false) nodes.push(claimNodes());
    else if (owner.claimStatus) nodes.push(make('p', 'owner-claim-status', owner.claimStatus)); // a claim made from here: the relay's answer, marked
    const ownerKey = listedOwner();
    nodes.push(make('p', 'owner-who', d && d.i_am_owner ? t('chat_owner.owner_line_me') : (ownerKey ? t('chat_owner.owner_line', { name: nameOf(ownerKey) }) : t('chat_owner.owner_line_none'))));
    const successor = d && d.successor && d.successor.key ? d.successor : null;
    nodes.push(make('p', 'owner-successor', successor ? t('chat_owner.successor_line', { name: successor.name ? honestOr(successor.name) : nameOf(successor.key) }) : t('chat_owner.successor_none')));
    nodes.push(muted(marked(make('p', 'owner-successor-about'), 'chat_owner.successor_about')));
    if (d && d.i_am_owner) {
      // Why the owner's controls are not on this page (the seventh review, M2).
      nodes.push(make('h4', 'owner-desktop-title', t('chat_owner.desktop_only_title')));
      nodes.push(muted(marked(make('p', 'owner-desktop-only'), 'chat_owner.desktop_only')));
    }
    nodes.push(make('h3', 'owner-log-title', t('chat_owner.log_title')));
    const log = (d && Array.isArray(d.owner_log)) ? d.owner_log : [];
    if (!log.length) nodes.push(muted(make('p', 'owner-log-empty', t('chat_owner.log_empty'))));
    for (const l of log) {
      const key = l && LOG_WORDS[l.kind];
      if (!key) continue;
      // The names an honest relay could list, or a refused name; the reason is the owner's own
      // signed words, marked as the server's since this page cannot check that signature.
      const line = make('div', 'owner-log-line', t(key, { when: when(Number(l.at) || 0), old: l.old_name ? honestOr(l.old_name) : '', new: l.new_name ? honestOr(l.new_name) : '', reason: l.reason ? fromServer(l.reason) : '' }));
      line.style.fontSize = 'var(--text-sm)';
      nodes.push(line);
    }
    return nodes;
  }

  function renderAll() {
    renderClaim();
    renderMyFingerprint();
    if (typeof window.heldRender === 'function') window.heldRender();
  }

  /** BLAKE3 derive_key with a 32-byte output, once the hash is loaded; null before. */
  function deriver() {
    if (!_noble) return null;
    return (ctx, bytes) => _noble.blake3.create({ context: new TextEncoder().encode(ctx), dkLen: 32 }).update(bytes).digest();
  }

  /** My own key's fingerprint in the Account and Identity menu (#my-fingerprint), in words, worked
   *  out from my own key the moment the page knows it (/shared/own-fingerprint.js), never waiting on
   *  a frame from the server (the sixth review's H1 (a)), with words that this page came from the
   *  server and is only as trustworthy as it (the seventh review, M2). */
  function renderMyFingerprint() {
    const el = typeof document !== 'undefined' && document.getElementById ? document.getElementById('my-fingerprint') : null;
    const F = window.HosOwnFingerprint;
    const derive = deriver();
    if (!el || !F || typeof F.fill !== 'function' || !derive || !me()) return;
    F.fill(el, me(), derive, window.HosKeyCard);
  }

  /** Load the hash and draw my own fingerprint in the menu (app.js calls it once it knows my key). */
  async function showMyFingerprint() {
    try { await hashMod(); } catch (_) { return; }
    renderMyFingerprint();
  }

  // The relay's frames for this file. Everything else goes on down the chain.
  const _orig = handleMessage;
  handleMessage = function (msg) {
    if (msg && msg.type === 'owner_state') {
      owner.hasOwner = !!msg.has_owner;
      if (owner.hasOwner === false) showNotice();
      else if (owner.notice) owner.notice.disabled = true;
      renderAll();
      return;
    }
    if (msg && msg.type === 'owner_reply') { onReply(msg); return; }
    if (msg && msg.type === 'owner_change_ready') { console.warn('owner_change_ready for a change this page never begins: dropped.'); return; }
    if (msg && msg.type === 'owner_change_check') { onCheck(msg); return; }
    return _orig(msg);
  };

  window.hosShowMyFingerprint = showMyFingerprint;
  if (me()) showMyFingerprint();
  window.openOwnerClaim = openClaimView;
  window.closeOwnerClaim = closeClaimView;
  window.ownerHeldNodes = heldNodes;
  window.ownerRefusalNodes = refusalNodes;
  // For scripts/tests/owner-chat-web.test.js: the state, read only by the test.
  window.__ownerState = owner;
})();
