// recovery-app.js: the website's Recovery through trusted people page (/recovery), the mirror of
// the desktop app's Settings > Account > "Recovery through trusted people"
// (src/gui/pages/settings/recovery.rs), in the same words. The protocol is /shared/recovery.js,
// the design docs/design/recovery-through-trusted-people.md.
//
// Three parts:
//  1. Your trusted people: choose them from your friends, how many must help, send the pieces;
//     see who keeps them; take them all back. Needs this browser's account (the recovery phrase
//     Chat keeps here, read through /shared/pq-relay-auth.js).
//  2. Help someone get back in: who is asking, the guidance to check it is really them outside
//     the app, the code they read you, and your piece sent to their new device.
//  3. Lost your device? Ask for help: find your account, open a request, read the code to your
//     trusted people, and restore the account when enough pieces arrive (handed to Chat's own
//     "Restore from recovery phrase" in this tab).

import * as R from '/shared/recovery.js';

const API = '/api/v2/recovery';
const POLL_MS = 5000;
let m = null;

async function noble() {
  if (!m) m = await import('/shared/vendor/noble-pq.bundle.js');
  return m;
}

const $ = (id) => document.getElementById(id);
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}
function say(id, text, kind) {
  const el = $(id);
  if (!el) return;
  el.textContent = text || '';
  el.className = 'rec-msg' + (kind ? ' ' + kind : '');
}
function day(ms) {
  try { return new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }); } catch (_) { return ''; }
}
function names(list) {
  if (list.length <= 1) return list.join('');
  return list.slice(0, -1).join(', ') + ' and ' + list[list.length - 1];
}

async function getJson(path) {
  const res = await fetch(path, { credentials: 'same-origin' });
  let body = null;
  try { body = await res.json(); } catch (_) { /* not JSON */ }
  return { status: res.status, body };
}
async function postJson(path, data) {
  const res = await fetch(path, {
    method: 'POST', credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data),
  });
  let body = null;
  try { body = await res.json(); } catch (_) { /* not JSON */ }
  return { status: res.status, body };
}
function errorOf(r, fallback) {
  return (r.body && r.body.error) || fallback || ('The server answered with an error (' + r.status + ').');
}

// ── This browser's account ──

let me = null; // { secret, did, dilHex, kyber }

async function myAccount() {
  if (me) return me;
  if (typeof window.getPqRecoverySeed !== 'function') return null;
  const secret = await window.getPqRecoverySeed();
  if (!secret || secret.length !== 32) return null;
  const mm = await noble();
  const dil = R.dilithiumOf(mm, secret);
  me = { secret, did: R.didForPublicKey(mm, dil.publicKey), dilHex: R.hex(dil.publicKey), kyber: R.kyberOf(mm, secret) };
  return me;
}

/** My friends (people I follow who follow me back), read from Chat's store in this browser. */
async function myFriends(acct) {
  if (!window.hosDmStore || typeof window.getPqDmStoreKey !== 'function') return [];
  if (typeof window.getDmStoreKey !== 'function') window.getDmStoreKey = window.getPqDmStoreKey;
  const ok = await window.hosDmStore.init(acct.dilHex, location.host, { readOnly: true }).catch(() => false);
  if (!ok) return [];
  const keys = Array.from(window.hosDmStore.following).filter((k) => window.hosDmStore.followers.has(k));
  const mm = await noble();
  const K = window.HosKeyCard;
  const verify = (pub, msg, sig) => { try { return !!mm.ml_dsa65.verify(sig, msg, pub); } catch (_) { return false; } };
  const out = [];
  for (const key of keys) {
    let name = key.slice(0, 8), kyberB64 = null, changed = false;
    const r = await getJson('/api/members/' + encodeURIComponent(key)).catch(() => ({ status: 0 }));
    if (r.status === 200 && r.body && r.body.name) name = r.body.name;
    // Their Kyber768 key ONLY from a key card checked here (key cards, 2026-10-22): the one
    // Chat pinned in this browser, else their card from the server, signed by their own
    // identity and naming them. A key that changed waits for them to be accepted in Chat.
    if (K && window.hosDmStore.keyChange(key)) {
      changed = true;
    } else if (K && window.hosDmStore.keySealing(key)) {
      kyberB64 = window.hosDmStore.keySealing(key);
    } else if (K) {
      const c = await getJson('/api/key-card/' + encodeURIComponent(key)).catch(() => ({ status: 0 }));
      const card = c.status === 200 ? K.parseCard(c.body) : null;
      if (card && await K.cardFor(card, key, verify)) kyberB64 = card.kyber;
    }
    let kyber = null;
    if (kyberB64) { try { kyber = R.unb64(kyberB64); } catch (_) { kyber = null; } }
    let did = '';
    try { did = R.didForPublicKey(mm, R.unhex(key)); } catch (_) { continue; }
    out.push({ key, did, name, changed, kyberPublic: kyber && kyber.length === R.KYBER_PUBLIC_LEN ? kyber : null });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}

// ── 1. Your trusted people ──

let friends = [];

async function loadMySetup() {
  const acct = await myAccount();
  const area = $('mine-status');
  if (!acct) {
    area.innerHTML = '<p class="rec-muted">This browser is not signed in to an account yet, so there is nothing to protect here. Open Chat and sign in first. (Lost your device? See the last part of this page.)</p>';
    $('mine-actions').hidden = true;
    return;
  }
  $('mine-actions').hidden = false;
  area.innerHTML = '<p class="rec-muted">Looking up who keeps your pieces…</p>';
  const r = await getJson(API + '/setup/' + encodeURIComponent(acct.did));
  const mm = await noble();
  let html = '';
  if (r.status === 404) {
    html = '<p>No one keeps a piece of your recovery phrase yet.</p>';
    $('mine-choose').textContent = 'Choose trusted people';
    $('mine-withdraw').hidden = true;
  } else if (r.status !== 200) {
    html = '<p class="rec-warn">' + esc(errorOf(r, 'Could not look it up.')) + '</p>';
  } else {
    const active = r.body.active;
    if (active) {
      const rows = [];
      for (const p of active.pieces) {
        let who = 'someone (this piece\'s note could not be read)';
        try {
          const sp = await R.servedPiece(mm, p.object);
          const note = sp.holderDid === acct.did ? await R.openNote(mm, acct.kyber.secretKey, sp) : null;
          if (note) who = note.name + ' (' + note.did + ')';
        } catch (_) { /* shown as unread */ }
        rows.push(who);
      }
      const short = rows.map((w) => w.replace(/ \(did:hum:.*\)$/, ''));
      html += '<p>' + esc(names(short)) + ' each keep a piece. It takes ' + active.needed + ' of them to put your recovery phrase back together.</p>';
      html += '<ul class="rec-list">' + rows.map((w, i) => '<li>Piece ' + (i + 1) + ': ' + esc(w) + '</li>').join('') + '</ul>';
      $('mine-choose').textContent = 'Change who keeps the pieces';
      $('mine-withdraw').hidden = false;
    }
    if (r.body.sending) {
      html += '<p class="rec-muted">Sending new pieces: ' + r.body.sending.have + ' of ' + r.body.sending.total + ' have arrived. Until all have, the pieces before them stay.</p>';
    }
  }
  // Someone asking for help with my own account.
  const asks = await getJson(API + '/asks').catch(() => ({ status: 0 }));
  const mine = asks.status === 200 && Array.isArray(asks.body) ? asks.body.filter((a) => a.holder_did === acct.did) : [];
  if (mine.length) {
    html += '<div class="rec-alert"><strong>Someone asked your trusted people for help to get into your account</strong> (on ' +
      esc(day(mine[0].opened_at)) + '). If that was not you, tell your trusted people not to send anything, and press the button below.' +
      '<br><button type="button" id="mine-not-me" class="rec-btn">That was not me: stop it</button></div>';
  }
  area.innerHTML = html;
  const notMe = $('mine-not-me');
  if (notMe) notMe.addEventListener('click', notMeClicked);
}

async function notMeClicked() {
  const auth = typeof window.getPqSignedAuth === 'function' ? await window.getPqSignedAuth(R.NOT_ME_PURPOSE) : null;
  if (!auth) { say('mine-msg', 'This browser could not sign for your account. Open Chat and sign in first.', 'warn'); return; }
  const r = await postJson(API + '/not-me', auth);
  say('mine-msg', r.status === 200 ? 'Stopped. Every request for help with your account on this server is closed.' : errorOf(r), r.status === 200 ? 'ok' : 'warn');
  loadMySetup();
}

async function openEditor() {
  const acct = await myAccount();
  if (!acct) return;
  $('editor').hidden = false;
  $('editor-list').innerHTML = '<p class="rec-muted">Reading your friends…</p>';
  friends = await myFriends(acct);
  if (!friends.length) {
    $('editor-list').innerHTML = '<p>You have no friends in Chat on this browser yet. Trusted people are friends: people you follow who follow you back.</p>';
    return;
  }
  $('editor-list').innerHTML = friends.map((f, i) =>
    '<label class="rec-person' + (f.kyberPublic ? '' : ' rec-off') + '"><input type="checkbox" data-i="' + i + '"' + (f.kyberPublic ? '' : ' disabled') + '> ' +
    esc(f.name) + (f.kyberPublic ? '' : ' <span class="rec-muted">' + (f.changed
      ? '(their encryption key changed: accept it in your conversation with them in Chat first)'
      : '(they have no signed key card on this server yet: it appears once they sign in with a current version of the app)') + '</span>') + '</label>').join('');
  $('editor-list').querySelectorAll('input[type=checkbox]').forEach((c) => c.addEventListener('change', refreshNeeded));
  refreshNeeded();
}

function chosenPeople() {
  return Array.from($('editor-list').querySelectorAll('input[type=checkbox]:checked')).map((c) => friends[Number(c.dataset.i)]);
}

function refreshNeeded() {
  const n = chosenPeople().length;
  const sel = $('editor-needed');
  const was = Number(sel.value) || 0;
  sel.innerHTML = '';
  for (let k = R.MIN_NEEDED; k <= Math.max(R.MIN_NEEDED, n); k++) {
    const o = document.createElement('option');
    o.value = String(k);
    o.textContent = k + ' of ' + Math.max(n, R.MIN_NEEDED);
    sel.appendChild(o);
  }
  // Suggested: more than half of them (2 of 3, 3 of 5), never fewer than 2.
  sel.value = String(was >= R.MIN_NEEDED && was <= n ? was : Math.max(R.MIN_NEEDED, Math.min(n, Math.floor(n / 2) + 1)));
  $('editor-send').disabled = n < R.MIN_PEOPLE || n > R.MAX_PEOPLE;
  say('editor-msg', n > R.MAX_PEOPLE ? 'At most ' + R.MAX_PEOPLE + ' trusted people.' : n < R.MIN_PEOPLE ? 'Choose at least ' + R.MIN_PEOPLE + '.' : '');
}

async function sendPieces() {
  const acct = await myAccount();
  const people = chosenPeople();
  const needed = Number($('editor-needed').value);
  if (!acct || people.length < R.MIN_PEOPLE) return;
  $('editor-send').disabled = true;
  say('editor-msg', 'Locking the pieces…');
  try {
    const mm = await noble();
    const { submissions } = await R.buildSetup(mm, { secret: acct.secret, people, needed, createdAt: Date.now() });
    for (let i = 0; i < submissions.length; i++) {
      say('editor-msg', 'Sending piece ' + (i + 1) + ' of ' + submissions.length + '…');
      const r = await postJson('/api/v2/objects', submissions[i]);
      if (r.status !== 200) throw new Error(errorOf(r));
    }
    say('editor-msg', 'Done. ' + names(people.map((p) => p.name)) + ' each keep a piece now. Tell them, so they know what it is if you ever call.', 'ok');
    $('editor').hidden = true;
    loadMySetup();
  } catch (e) {
    say('editor-msg', 'The pieces were not all sent: ' + (e.message || e) + ' Your earlier setup, if any, still stands.', 'warn');
  } finally {
    $('editor-send').disabled = false;
  }
}

async function withdrawAll() {
  if (!confirm('Take every piece off the server? Nobody will be able to help you back in until you choose trusted people again.')) return;
  const auth = typeof window.getPqSignedAuth === 'function' ? await window.getPqSignedAuth(R.WITHDRAW_PURPOSE) : null;
  if (!auth) { say('mine-msg', 'This browser could not sign for your account. Open Chat and sign in first.', 'warn'); return; }
  const r = await postJson(API + '/withdraw', auth);
  say('mine-msg', r.status === 200 ? 'Taken back: the server no longer keeps any piece of your recovery phrase.' : errorOf(r), r.status === 200 ? 'ok' : 'warn');
  loadMySetup();
}

// ── 2. Help someone get back in ──

/** The piece I keep for `holderDid`, opened: { piece, opened } or null. */
async function myPieceFor(acct, holderDid) {
  const r = await getJson(API + '/setup/' + encodeURIComponent(holderDid));
  if (r.status !== 200 || !r.body || !r.body.active) return null;
  const mm = await noble();
  for (const p of r.body.active.pieces) {
    let sp;
    try { sp = await R.servedPiece(mm, p.object); } catch (_) { continue; }
    if (sp.holderDid !== holderDid) continue;
    const opened = await R.openPiece(mm, acct.kyber.secretKey, sp);
    if (opened) return { piece: sp, opened, needed: r.body.active.needed };
  }
  return null;
}

let friendNames = null;
async function nameOf(acct, did) {
  if (!friendNames) {
    friendNames = {};
    for (const f of await myFriends(acct)) friendNames[f.did] = f.name;
  }
  return friendNames[did] || did;
}

async function checkAsking() {
  const acct = await myAccount();
  const area = $('help-asking');
  if (!acct) {
    area.innerHTML = '<p class="rec-muted">Open this page in the browser where you use your account to help someone.</p>';
    $('help-form').hidden = true;
    return;
  }
  $('help-form').hidden = false;
  area.innerHTML = '<p class="rec-muted">Checking who is asking for help…</p>';
  const asks = await getJson(API + '/asks').catch(() => ({ status: 0 }));
  const lines = [];
  if (asks.status === 200 && Array.isArray(asks.body)) {
    const seen = new Set();
    for (const a of asks.body) {
      if (seen.has(a.holder_did) || a.holder_did === acct.did) continue;
      seen.add(a.holder_did);
      if (await myPieceFor(acct, a.holder_did)) {
        lines.push('<p class="rec-asking"><strong>' + esc(await nameOf(acct, a.holder_did)) + ' is asking for your help to recover their account</strong> (since ' + esc(day(a.opened_at)) + ').</p>');
      }
    }
  }
  area.innerHTML = lines.length ? lines.join('') : '<p class="rec-muted">Nobody whose piece you keep is asking for help right now.</p>';
}

let lookup = null; // { code, holderDid, key, name, slot, share, token, needed }

/** Look up the code a person read me: the request, a check that its key is the one the code
 *  names, and my piece for its account. Nothing is sent yet. */
async function lookUpCode() {
  const acct = await myAccount();
  if (!acct) return;
  lookup = null;
  $('help-found').hidden = true;
  $('help-confirm').checked = false;
  $('help-send').disabled = true;
  const code = R.normalizeCode($('help-code').value);
  if (!code) { say('help-msg', 'That is not a code: it is 16 letters and numbers, like ABCD-EFGH-JKMN-PQRS.', 'warn'); return; }
  say('help-msg', 'Looking up the request…');
  try {
    const r = await getJson(API + '/ask/' + encodeURIComponent(code));
    if (r.status !== 200) { say('help-msg', r.status === 404 ? 'There is no open request for help with that code on this server. Check the code with them.' : errorOf(r), 'warn'); return; }
    const mm = await noble();
    const key = R.unb64(r.body.one_time_key_b64);
    if (R.askCode(mm, r.body.holder_did, key) !== code) {
      say('help-msg', 'This request does not match the code they read you, so nothing can be sent. Tell them, and do not send anything for this code.', 'warn');
      return;
    }
    const mine = await myPieceFor(acct, r.body.holder_did);
    if (!mine) { say('help-msg', 'You do not keep a piece for the account that made this request, so there is nothing to send.', 'warn'); return; }
    const name = await nameOf(acct, r.body.holder_did);
    lookup = { code, holderDid: r.body.holder_did, key, name, slot: mine.piece.payload.slot, share: mine.opened.share, token: mine.opened.token, needed: mine.needed };
    $('help-who').textContent = name + ' is asking for your help to recover their account.';
    $('help-confirm-words').textContent = 'I spoke to ' + name + ' myself, by a call on a number I already had or face to face, and they read me this code.';
    $('help-found').hidden = false;
    say('help-msg', '');
  } catch (e) {
    say('help-msg', 'Could not look it up: ' + (e.message || e), 'warn');
  }
}

/** Send my piece to the device that asked, sealed to its one-time key. */
async function sendMyPiece() {
  if (!lookup || !$('help-confirm').checked) return;
  $('help-send').disabled = true;
  try {
    const mm = await noble();
    const sealed = await R.sealAnswer(mm, lookup.key, lookup.code, lookup.holderDid, lookup.slot, lookup.share);
    const a = await postJson(API + '/answer', { code: lookup.code, slot: lookup.slot, token_b64: R.b64(lookup.token), sealed_b64: R.b64(sealed) });
    if (a.status !== 200) { say('help-msg', errorOf(a), 'warn'); $('help-send').disabled = false; return; }
    say('help-msg', 'Sent to ' + lookup.name + '\'s new device. It needs ' + lookup.needed + ' pieces in all; ' + a.body.answered + ' have arrived.', 'ok');
    lookup = null;
    $('help-found').hidden = true;
    $('help-code').value = '';
  } catch (e) {
    say('help-msg', 'Nothing was sent: ' + (e.message || e), 'warn');
    $('help-send').disabled = false;
  }
}

async function piecesIKeep() {
  const acct = await myAccount();
  if (!acct) return;
  const area = $('help-kept');
  area.innerHTML = '<p class="rec-muted">Checking your friends…</p>';
  const kept = [];
  for (const f of await myFriends(acct)) {
    if (await myPieceFor(acct, f.did)) kept.push(f.name);
  }
  area.innerHTML = kept.length
    ? '<p>You keep a piece for ' + esc(names(kept)) + '.</p>'
    : '<p class="rec-muted">You keep no piece for any of your friends on this server.</p>';
}

// ── 3. Lost your device? Ask for help ──

let ask = null; // { holderDid, name, code, oneTime, needed, total, timer }

async function findAccount() {
  const typed = $('ask-who').value.trim();
  const area = $('ask-found');
  area.innerHTML = '';
  if (!typed) return;
  if (R.accountIdOk(typed)) { chooseAccount(typed, typed); return; }
  const r = await getJson('/api/members?search=' + encodeURIComponent(typed) + '&limit=20');
  const mm = await noble();
  const matches = (r.body && r.body.members ? r.body.members : []).filter((x) => x.name && x.name.toLowerCase() === typed.toLowerCase());
  if (!matches.length) { area.innerHTML = '<p class="rec-warn">No one named "' + esc(typed) + '" is listed on this server. Check the spelling, or type your account ID.</p>'; return; }
  area.innerHTML = matches.map((x, i) => {
    let did = '';
    try { did = R.didForPublicKey(mm, R.unhex(x.public_key)); } catch (_) { did = ''; }
    return did ? '<button type="button" class="rec-btn rec-pick" data-did="' + esc(did) + '" data-name="' + esc(x.name) + '">' + esc(x.name) + ' (' + esc(did) + ')</button>' : '';
  }).join('');
  area.querySelectorAll('.rec-pick').forEach((b) => b.addEventListener('click', () => chooseAccount(b.dataset.did, b.dataset.name)));
  if (matches.length === 1) area.querySelector('.rec-pick') && area.querySelector('.rec-pick').click();
}

async function chooseAccount(did, name) {
  const r = await getJson(API + '/setup/' + encodeURIComponent(did));
  if (r.status !== 200 || !r.body.active) {
    $('ask-found').innerHTML = '<p class="rec-warn">This account has no trusted people on this server, so there is no one to ask here.</p>';
    return;
  }
  ask = { holderDid: did, name, needed: r.body.active.needed, total: r.body.active.total };
  $('ask-found').innerHTML = '<p>' + esc(name) + ': it takes ' + ask.needed + ' of your ' + ask.total + ' trusted people.</p>' +
    '<button type="button" id="ask-open" class="rec-btn rec-primary">Ask my trusted people for help</button>';
  $('ask-open').addEventListener('click', openAsk);
}

async function openAsk() {
  const mm = await noble();
  const one = R.newOneTimeAsk(mm);
  const r = await postJson(API + '/ask', { holder_did: ask.holderDid, one_time_key_b64: R.b64(one.publicKey), close_hash: R.closeHash(mm, one.closeSecret) });
  if (r.status !== 200) { $('ask-found').innerHTML += '<p class="rec-warn">' + esc(errorOf(r)) + '</p>'; return; }
  const code = R.askCode(mm, ask.holderDid, one.publicKey);
  if (r.body.code !== code) { $('ask-found').innerHTML += '<p class="rec-warn">The server filed this request under another code, so it was not used.</p>'; return; }
  Object.assign(ask, { code, oneTime: one, needed: r.body.needed, total: r.body.total });
  $('ask-step1').hidden = true;
  $('ask-step2').hidden = false;
  $('ask-code').textContent = R.showCode(code);
  const link = location.origin + '/recovery#help=' + code;
  drawQr($('ask-qr'), link);
  pollAnswers();
}

function drawQr(canvas, text) {
  if (typeof window.qrcode !== 'function' || !canvas) return;
  const qr = window.qrcode(0, 'M');
  qr.addData(text);
  qr.make();
  const n = qr.getModuleCount();
  const cell = Math.max(2, Math.floor(200 / n));
  canvas.width = canvas.height = n * cell + 2 * cell;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#fff'; // theme-exempt: a QR code is dark on light whatever the theme
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = '#000'; // theme-exempt: as above
  for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) if (qr.isDark(r, c)) ctx.fillRect((c + 1) * cell, (r + 1) * cell, cell, cell);
}

async function pollAnswers() {
  if (!ask || !ask.code) return;
  const r = await getJson(API + '/ask/' + encodeURIComponent(ask.code)).catch(() => ({ status: 0 }));
  if (r.status === 404) { say('ask-progress', 'This request has ended (it lasts ' + R.ASK_DAYS + ' days). Ask again for a new code.', 'warn'); return; }
  if (r.status === 200) {
    const mm = await noble();
    const shares = [];
    for (const a of r.body.answers || []) {
      const s = await R.openAnswer(mm, ask.oneTime.secretKey, ask.code, ask.holderDid, a.slot, R.unb64(a.sealed_b64));
      if (s) shares.push(s);
    }
    say('ask-progress', 'Pieces arrived: ' + shares.length + ' of the ' + r.body.needed + ' needed.');
    if (r.body.needed && shares.length >= r.body.needed) {
      const secret = R.rebuild(mm, shares, r.body.needed, ask.holderDid);
      if (secret) { await rebuilt(secret); return; }
      say('ask-progress', 'The pieces that arrived do not fit together. Ask your trusted people to check it was your account they helped.', 'warn');
    }
  }
  ask.timer = setTimeout(pollAnswers, POLL_MS);
}

async function rebuilt(secret) {
  const words = typeof window.pqPhraseWords === 'function' && Array.isArray(window.BIP39_ENGLISH)
    ? await window.pqPhraseWords(secret, window.BIP39_ENGLISH) : null;
  await postJson(API + '/close', { code: ask.code, close_secret_b64: R.b64(ask.oneTime.closeSecret) }).catch(() => null);
  $('ask-step2').hidden = true;
  $('ask-step3').hidden = false;
  if (!words) { say('ask-done-msg', 'Your account is back together, but this page could not write out its recovery phrase. Reload and ask again.', 'warn'); return; }
  $('ask-restore').onclick = () => {
    try {
      sessionStorage.setItem('hum_restore_words', words.join(' '));
      sessionStorage.setItem('hum_restore_name', ask.name && !R.accountIdOk(ask.name) ? ask.name : '');
    } catch (_) { /* the restore box will be empty; the words are not shown here */ }
    location.href = '/chat';
  };
}

async function stopAsking() {
  if (ask && ask.code) {
    clearTimeout(ask.timer);
    await postJson(API + '/close', { code: ask.code, close_secret_b64: R.b64(ask.oneTime.closeSecret) }).catch(() => null);
  }
  ask = null;
  $('ask-step2').hidden = true;
  $('ask-step1').hidden = false;
  $('ask-found').innerHTML = '';
}

// ── Wiring ──

function wire() {
  $('mine-choose').addEventListener('click', openEditor);
  $('mine-withdraw').addEventListener('click', withdrawAll);
  $('editor-send').addEventListener('click', sendPieces);
  $('editor-cancel').addEventListener('click', () => { $('editor').hidden = true; });
  $('help-look').addEventListener('click', lookUpCode);
  $('help-code').addEventListener('keydown', (e) => { if (e.key === 'Enter') lookUpCode(); });
  $('help-confirm').addEventListener('change', () => { $('help-send').disabled = !($('help-confirm').checked && lookup); });
  $('help-send').addEventListener('click', sendMyPiece);
  $('help-check').addEventListener('click', checkAsking);
  $('help-kept-btn').addEventListener('click', piecesIKeep);
  $('ask-find').addEventListener('click', findAccount);
  $('ask-who').addEventListener('keydown', (e) => { if (e.key === 'Enter') findAccount(); });
  $('ask-stop').addEventListener('click', stopAsking);
  const help = /[#&]help=([0-9A-Za-z-]+)/.exec(location.hash);
  if (help) {
    $('help-code').value = help[1];
    $('help').scrollIntoView();
  }
  loadMySetup().catch((e) => say('mine-msg', String(e && e.message || e), 'warn'));
  checkAsking().catch((e) => say('help-msg', String(e && e.message || e), 'warn'));
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', wire); else wire();
