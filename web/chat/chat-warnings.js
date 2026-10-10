// ── chat-warnings.js ──────────────────────────────────────────────────────
// Warnings on messages, and the recovery-phrase guard (step F, 2026-10-10,
// docs/design/blocking-and-safe-mode.md 6.3 and 10g), mirroring the desktop
// app. Everything here runs on this device: nothing it looks at leaves it,
// and a warning never blocks a message or reports anything.
//
// Warnings: under a received direct message (chat-dms.js addDmMessage) and a
// message in a P2P group (chat-groups-p2p.js), each entry of
// /data/safety/warnings.json the message matches (the matcher is
// /shared/warnings.js) and whose `applies_to` holds the sender: `friends` for
// a mutual follow (I follow them and they follow me, the test the pass code
// uses, hosDmStore.isFriendPeer), `strangers` for everyone else. Each shows its
// title, explanation and advice with a "Got it" that hides it for that
// message, in the file's order. Never on my own messages, and not on public
// channel posts in this step.
//
// A stranger's links: a direct message from someone who is not a friend that
// holds a link is drawn with its links held (nothing opens or loads), and one
// line under it says so, with an Open button that lets them open.
//
// The switch: Settings > Safety, "Warnings on messages", On by default, kept
// in the encrypted local store with the block list (hosDmStore.warningsOn).
// Off hides every warning; the link line is a separate rule and stays.
//
// The guard: before anything I wrote is sent, recoveryPhraseGuardStops (or
// recoveryPhraseIn, for a dialog that shows the sentence itself) derives my
// recovery phrase in the page from what the identity already holds (crypto.js
// identitySeed and mnemonicFromSeed, over bip39-english.js; the same way the
// backup screen shows it) and stops a send that holds 4 or more of its words
// in a row, in order. It is never kept: derived for each check and dropped.
// No switch, no "send anyway". When the identity is locked and there is no
// phrase to derive, the guard cannot run, and the send goes as it did before.
//
// Depends on: /shared/warnings.js, crypto.js (identitySeed, mnemonicFromSeed),
// app.js (myKey, appendMessage), chat-dm-store.js (hosDmStore), chat-social.js
// (isFriend, before the store loads), chat-privacy.js (renderSafetyPanel).
// Test: scripts/tests/warnings-web.test.js
// ─────────────────────────────────────────────────────────────────────────

// ── The patterns (data, 10g) ─────────────────────────────────────────────

let messageWarnings = null;        // warningsFrom() output, once loaded
let messageWarningsLoading = null; // the fetch in flight
let messageWarningsTriedAt = 0;    // when the last fetch started
// After a failed fetch, wait this long before asking again (a fetch that fails
// at once must never turn into a loop; see chat-ui.js renderServerList).
const MESSAGE_WARNINGS_RETRY_MS = 60000;
// Messages drawn before the file arrived get their warnings when it does.
const messageWarningsWaiting = [];
const MESSAGE_WARNINGS_WAITING_MAX = 500;

/** The warnings from the data file; an empty list while it cannot be read. */
function loadMessageWarnings() {
  if (messageWarnings) return Promise.resolve(messageWarnings);
  if (messageWarningsLoading) return messageWarningsLoading;
  if (messageWarningsTriedAt && Date.now() - messageWarningsTriedAt < MESSAGE_WARNINGS_RETRY_MS) return Promise.resolve([]);
  messageWarningsTriedAt = Date.now();
  messageWarningsLoading = Promise.resolve()
    .then(() => fetch(WARNINGS_URL, { cache: 'no-cache' }))
    .then((r) => (r && r.ok ? r.json() : null))
    .then((j) => warningsFrom(j))
    .catch(() => [])
    .then((list) => {
      messageWarningsLoading = null;
      if (list.length) {
        messageWarnings = list;
        for (const w of messageWarningsWaiting.splice(0)) drawMessageWarnings(w.el, w.opts);
      }
      return list;
    });
  return messageWarningsLoading;
}

// ── Who is a friend, and the switch ──────────────────────────────────────

function warnStore() {
  return (window.hosDmStore && hosDmStore.ready) ? hosDmStore : null;
}

function warnSameKey(a, b) {
  return typeof a === 'string' && typeof b === 'string' && a !== '' && a.toLowerCase() === b.toLowerCase();
}

/** Is `key` a friend: a mutual follow, the test the pass code uses? */
function warningSenderIsFriend(key) {
  if (!key) return false;
  const store = warnStore();
  if (store) return store.isFriendPeer(key);
  return typeof isFriend === 'function' ? !!isFriend(key) : false;
}

/**
 * Is the "Warnings on messages" switch on? On until the person turns it off.
 * With the protected setup on, only its own choice counts (made with the PIN,
 * kept on this device: /shared/protected.js protectedWarningsOn), never the
 * per-identity switch in the local store, so restoring an identity whose
 * warnings were off does not turn them off.
 */
function messageWarningsOn() {
  const store = warnStore();
  const own = store ? store.warningsOn !== false : true;
  if (typeof protectedWarningsOn === 'function' && typeof protectedCurrent === 'function') {
    return protectedWarningsOn(protectedCurrent(), own);
  }
  return own;
}

/** The Safety switch changed. Returns false when the store is not loaded yet (the switch is disabled then). */
function setMessageWarningsOn(on) {
  const store = warnStore();
  if (!store) return false;
  // With the protected setup on, turning warnings off needs the PIN (10h,
  // /shared/protected.js); turning them on never does. The switch is drawn
  // back as it was until the PIN is given.
  if (!on && typeof protectedTake === 'function' && !protectedTake('warnings_off')) {
    protectedAskThen('warnings_off', () => setMessageWarningsOn(false));
    if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
    return false;
  }
  store.setWarningsOn(!!on);
  // With the setup on, the choice is the setup's (made with the PIN when it is
  // off), kept on this device so another identity's switch cannot undo it.
  const prot = typeof protectedCurrent === 'function' ? protectedCurrent() : null;
  if (prot && typeof protectedSave === 'function') protectedSave(Object.assign({}, prot, { warnings_off: !on }));
  applyWarningsSwitchToView();
  if (typeof renderSafetyPanel === 'function') renderSafetyPanel();
  return true;
}

/** Show or hide every warning on screen as the switch says (one dismissed with "Got it" stays hidden). */
function applyWarningsSwitchToView() {
  if (typeof document === 'undefined' || typeof document.querySelectorAll !== 'function') return;
  const on = messageWarningsOn();
  document.querySelectorAll('.msg-warning').forEach((box) => {
    if (!box || !box.style || (box.dataset && box.dataset.dismissed)) return;
    box.style.display = on ? '' : 'none';
  });
}

// ── Drawing under a message ──────────────────────────────────────────────

// "Got it" and Open, remembered for this session so a conversation drawn
// again keeps them (in memory only: nothing about a message is kept anywhere new).
const warningsDismissed = new Set(); // `${context}:${from}:${ts}:${id}`
const heldLinksOpened = new Set();   // `${from}:${ts}`

/**
 * Should this received direct message's links be held? It holds a link, and
 * the sender is not a friend, and the reader has not pressed Open on it.
 */
function dmLinksHeld(fromKey, ts, text) {
  if (!fromKey || (typeof myKey === 'string' && warnSameKey(fromKey, myKey))) return false;
  if (typeof text === 'string' && text.startsWith('[[hum:')) return false; // a file card, not text
  if (!messageHasLink(text)) return false;
  if (heldLinksOpened.has(`${fromKey}:${ts}`)) return false;
  return !warningSenderIsFriend(fromKey);
}

/** The place under a message's text where these lines go (inside its content column), one per message. */
const messageSafetySlots = new WeakMap();
function messageSafetySlot(el) {
  const had = messageSafetySlots.get(el);
  if (had) return had;
  const slot = document.createElement('div');
  slot.className = 'msg-safety';
  slot.style.cssText = 'clear:both;';
  // The row is [avatar | content] side by side (messages.css), so the lines go
  // in the content column, under the text.
  const main = typeof el.querySelector === 'function' ? el.querySelector('.msg-main') : null;
  (main && typeof main.appendChild === 'function' ? main : el).appendChild(slot);
  messageSafetySlots.set(el, slot);
  return slot;
}

/**
 * Draw what goes under a received message: the stranger link line (a direct
 * message whose links were held: pass `liveBodyHtml`, the body as it would be
 * drawn with its links) and the warnings it matches. `opts`: {text, from, ts,
 * context ('dm' or 'group'), name, liveBodyHtml}.
 */
function addMessageSafetyLines(el, opts) {
  const o = opts || {};
  if (!el || !o.from) return;
  if (typeof myKey === 'string' && warnSameKey(o.from, myKey)) return; // never on my own messages
  if (o.liveBodyHtml != null) drawStrangerLinkLine(el, o);
  if (typeof o.text === 'string' && o.text.startsWith('[[hum:')) return; // a file card, not words
  if (messageWarnings) {
    drawMessageWarnings(el, o);
  } else {
    if (messageWarningsWaiting.length >= MESSAGE_WARNINGS_WAITING_MAX) messageWarningsWaiting.shift();
    messageWarningsWaiting.push({ el, opts: o });
    loadMessageWarnings();
  }
}

/** The warnings a message matches, under it, in the file's order. */
function drawMessageWarnings(el, o) {
  const friend = warningSenderIsFriend(o.from);
  // With the protected setup on, a friend's message is checked against the
  // strangers' entries too (10h); otherwise each sender gets their own audience.
  const audiences = (typeof protectedWarningAudiences === 'function' && typeof protectedCurrent === 'function')
    ? protectedWarningAudiences(friend, protectedCurrent())
    : [friend ? 'friends' : 'strangers'];
  const list = messageWarnings || [];
  const matched = new Set();
  for (const who of audiences) for (const w of warningMatches(o.text, who, list)) matched.add(w.id);
  const hits = list.filter((w) => matched.has(w.id)); // the file's order, each once
  if (!hits.length) return;
  const on = messageWarningsOn();
  const slot = messageSafetySlot(el);
  for (const w of hits) {
    const key = `${o.context || ''}:${o.from}:${o.ts}:${w.id}`;
    if (warningsDismissed.has(key)) continue;
    const box = document.createElement('div');
    box.className = 'msg-warning';
    box.dataset.warningId = w.id;
    if (typeof box.setAttribute === 'function') box.setAttribute('role', 'note');
    box.style.cssText = 'margin-top:var(--space-xs);padding:var(--space-xs) var(--space-sm);'
      + 'border-left:3px solid var(--warning);background:var(--bg-tertiary);border-radius:var(--radius-sm);'
      + 'font-size:var(--text-sm);line-height:1.4;';
    box.style.display = on ? '' : 'none';
    const title = document.createElement('div');
    title.className = 'msg-warning-title';
    title.style.cssText = 'font-weight:600;color:var(--text);';
    title.textContent = w.title;
    const explain = document.createElement('div');
    explain.className = 'msg-warning-explain';
    explain.style.cssText = 'color:var(--text-muted);';
    explain.textContent = w.explain;
    const advice = document.createElement('div');
    advice.className = 'msg-warning-advice';
    advice.style.cssText = 'color:var(--text);';
    advice.textContent = w.advice;
    const ok = document.createElement('button');
    ok.className = 'vr-btn msg-warning-ok';
    ok.style.cssText = 'margin-top:var(--space-xs);font-size:0.7rem;';
    ok.textContent = WARNING_GOT_IT;
    ok.onclick = (e) => {
      if (e && typeof e.stopPropagation === 'function') e.stopPropagation();
      warningsDismissed.add(key);
      box.dataset.dismissed = '1';
      box.style.display = 'none';
    };
    box.appendChild(title);
    box.appendChild(explain);
    box.appendChild(advice);
    box.appendChild(ok);
    slot.appendChild(box);
  }
}

/** The line under a stranger's direct message whose links are held, and its Open button. */
function drawStrangerLinkLine(el, o) {
  const slot = messageSafetySlot(el);
  const line = document.createElement('div');
  line.className = 'msg-link-hold';
  line.style.cssText = 'display:flex;align-items:center;flex-wrap:wrap;gap:var(--space-sm);margin-top:var(--space-xs);'
    + 'color:var(--text-muted);font-size:var(--text-sm);';
  const words = document.createElement('span');
  words.textContent = strangerLinkLine(o.name || (typeof shortKey === 'function' ? shortKey(o.from) : String(o.from).slice(0, 8)));
  const open = document.createElement('button');
  open.className = 'vr-btn msg-link-open';
  open.style.cssText = 'font-size:0.7rem;';
  open.textContent = LINK_OPEN;
  open.onclick = (e) => {
    if (e && typeof e.stopPropagation === 'function') e.stopPropagation();
    heldLinksOpened.add(`${o.from}:${o.ts}`);
    const body = typeof el.querySelector === 'function' ? el.querySelector('.body') : null;
    if (body) {
      body.innerHTML = o.liveBodyHtml;
      if (window.twemoji) twemoji.parse(body);
    }
    line.style.display = 'none';
    if (typeof line.remove === 'function') line.remove();
  };
  line.appendChild(words);
  line.appendChild(open);
  slot.appendChild(line);
}

// ── The recovery-phrase guard (always on) ────────────────────────────────

/** My recovery phrase's words, derived now from the identity; null when it is locked. Never kept. */
async function recoveryPhraseWords() {
  try {
    if (typeof identitySeed !== 'function' || typeof mnemonicFromSeed !== 'function') return null;
    const seed = await identitySeed();
    if (!seed || seed.length !== 32) return null;
    const phrase = await mnemonicFromSeed(seed);
    return phrase ? phrase.split(' ') : null;
  } catch {
    return null;
  }
}

/**
 * Does any of these texts hold 4 or more words of my recovery phrase in a
 * row, in its order? False when the identity is locked (the guard cannot run).
 */
async function recoveryPhraseIn(texts) {
  const list = (Array.isArray(texts) ? texts : [texts]).filter((t) => typeof t === 'string' && t.trim());
  if (!list.length) return false;
  const words = await recoveryPhraseWords();
  if (!words) return false;
  return list.some((t) => phraseRunFound(t, words));
}

/** Say that a send was stopped, in the message area. `what` names it when it was not the composer (e.g. "Your profile was not sent."). */
function showPhraseGuardStop(what) {
  const text = (what ? what + ' ' : '') + PHRASE_GUARD_SENTENCE;
  if (typeof document === 'undefined' || typeof document.createElement !== 'function') return text;
  const el = document.createElement('div');
  el.className = 'message system phrase-guard-stop';
  if (typeof el.setAttribute === 'function') el.setAttribute('role', 'alert');
  el.style.cssText = 'padding:var(--space-sm) var(--space-md);border-left:3px solid var(--danger);color:var(--text);';
  el.textContent = text;
  if (typeof appendMessage === 'function') appendMessage(el);
  return text;
}

/**
 * The guard for a send: true when it must not go (and the sentence is shown),
 * false when it may. Every path that sends something I wrote awaits this first.
 */
async function recoveryPhraseGuardStops(texts, what) {
  if (!await recoveryPhraseIn(texts)) return false;
  showPhraseGuardStop(what);
  return true;
}

// Start reading the patterns now, so the first messages drawn have them.
loadMessageWarnings();

window.loadMessageWarnings = loadMessageWarnings;
window.messageWarningsOn = messageWarningsOn;
window.setMessageWarningsOn = setMessageWarningsOn;
window.applyWarningsSwitchToView = applyWarningsSwitchToView;
window.dmLinksHeld = dmLinksHeld;
window.addMessageSafetyLines = addMessageSafetyLines;
window.recoveryPhraseIn = recoveryPhraseIn;
window.recoveryPhraseGuardStops = recoveryPhraseGuardStops;
window.showPhraseGuardStop = showPhraseGuardStop;
