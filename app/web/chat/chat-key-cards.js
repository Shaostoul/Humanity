// ── chat-key-cards.js ─────────────────────────────────────────────────────
// Key cards in the website's chat (2026-10-22, docs/design/key-cards.md): the
// desktop app's src/engine/key_cards.rs, in the browser.
//
// THE ONE RULE: crypto.js getPeerEcdhPublic, which every seal to a person reads
// (pqBuildDmPuts: messages, follow notes, friendship passes, contact requests,
// reports to a group's creator, file offers; the direct channel of chat-p2p.js),
// answers ONLY with a key from a key card this page checked itself (signed by
// the person's own identity key and naming them, /shared/key-card.js cardFor)
// and pinned (hosDmStore.keyCards, this identity's on every server). A key a
// server merely says is never sealed to.
//
// HOW CARDS ARRIVE. The member lists carry only each person's card time
// (`key_card_at`). This page asks the server it is on for a card
// (`key_card_request`, at most 16 keys a frame) when it has none pinned for
// someone it may seal to (conversations, follows, followers, requests), when a
// conversation is opened, when something is about to be sealed to them, or when
// a list shows a card newer than the pinned one. Each `key_cards` answer is
// checked card by card. A newer card with a DIFFERENT key is a change: nothing
// is sealed to that person until I accept it (the notice with "Accept the new
// key", drawn in the conversation like the reach notices, chat-privacy.js).
//
// Depends on: /shared/key-card.js (HosKeyCard), chat-dm-store.js (hosDmStore),
// pq.js (window.pqVerifyMessage); at call time app.js (ws, myKey, peerData,
// appendMessage, addSystemMessage) and chat-dms.js (activeDmPartner). Loaded
// after chat-dm-store.js and before app.js.
// ──────────────────────────────────────────────────────────────────────────

/** How long a card asked for may take before it is asked for again (ms). */
const KEY_CARD_ASK_AGAIN_MS = 60000;
/** How long a send waits for a card it asked for (ms). */
const KEY_CARD_WAIT_MS = 8000;

const hosKeyCards = {
  _asked: new Map(),    // person -> when last asked (ms)
  _queue: [],           // people to ask for, in order
  _noCard: new Set(),   // people the server holds no good card for
  _waiters: new Map(),  // person -> [resolve]
  _flushing: false,

  /** The pins are this identity's and loaded. */
  ready() {
    return !!(window.hosDmStore && hosDmStore.ready && typeof myKey !== 'undefined' && hosDmStore.me === myKey && typeof HosKeyCard !== 'undefined');
  },

  /** The key to seal to `person` (from a card checked and pinned here), or null. */
  sealingKey(person) {
    return this.ready() ? hosDmStore.keySealing(person) : null;
  },

  /** Has `person`'s key changed and is it waiting to be accepted. */
  changed(person) {
    return this.ready() && !!hosDmStore.keyChange(person);
  },

  _asking(person) {
    const at = this._asked.get(person);
    return this._queue.includes(person) || (at !== undefined && Date.now() - at < KEY_CARD_ASK_AGAIN_MS);
  },

  /** Ask for `person`'s card. `update`: ask even though a key is pinned (a newer card is listed). */
  want(person, update) {
    if (!person || person === myKey || typeof HosKeyCard === 'undefined' || !HosKeyCard.isKeyText(person)) return;
    if (!update && this.sealingKey(person)) return;
    if (this._asking(person)) return;
    this._queue.push(person);
    if (!this._flushing) {
      this._flushing = true;
      setTimeout(() => { this._flushing = false; this._flush(); }, 0);
    }
  },

  /** Send what is queued on the socket, 16 keys a frame. Kept queued while there is no socket. */
  _flush() {
    if (!this.ready() || typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) return;
    while (this._queue.length) {
      const batch = this._queue.splice(0, HosKeyCard.REQUEST_MAX);
      ws.send(JSON.stringify({ type: 'key_card_request', keys: batch }));
      const now = Date.now();
      for (const k of batch) this._asked.set(k, now);
    }
  },

  /** The people I may seal to: conversations, follows, followers, requests, the open conversation. */
  _people() {
    const out = new Set();
    if (window.hosDmStore && hosDmStore.ready) {
      for (const k of hosDmStore.conversations.keys()) out.add(k);
      for (const k of hosDmStore.following) out.add(k);
      for (const k of hosDmStore.followers) out.add(k);
      for (const k of Object.keys(hosDmStore.contactRequests || {})) out.add(k);
    }
    if (typeof activeDmPartner !== 'undefined' && activeDmPartner) out.add(activeDmPartner);
    return out;
  },

  /** The store is loaded for this identity (app.js channel_list): ask for the cards I lack. */
  onStoreReady() {
    this._asked.clear();
    for (const k of this._people()) this.want(k, false);
    this._flush();
  },

  /**
   * A list named `person` with the time of the card this server holds (null:
   * none). Ask when it is newer than the pinned one, or when none is pinned and
   * I may seal to them. No key ever comes from a list.
   */
  noteListed(person, at) {
    if (!person || person === myKey || !this.ready()) return;
    if (typeof at !== 'number') {
      if (!this.sealingKey(person)) this._noCard.add(person);
      return;
    }
    const pin = hosDmStore.keyCards.pins[person] || null;
    const change = hosDmStore.keyChange(person);
    const seen = change || pin;
    if (seen && at <= seen.at) return;
    if (pin) { this.want(person, true); return; }
    if (this._people().has(person)) { this._noCard.delete(person); this.want(person, false); }
  },

  /** A `key_cards` answer, from the server I am on: every card checked, then taken by the pin rule. */
  async onFrame(msg) {
    let pinned = false;
    for (const raw of (msg && Array.isArray(msg.cards)) ? msg.cards : []) {
      if ((await this.ingest(raw)) === 'first') pinned = true;
    }
    for (const k of (msg && Array.isArray(msg.missing)) ? msg.missing : []) {
      if (typeof k !== 'string') continue;
      this._asked.delete(k);
      if (!this.sealingKey(k)) this._noCard.add(k);
      this._settle(k);
    }
    // Friends who could not be given a pass for want of a key can be now (chat-social.js).
    if (pinned && typeof sweepFriendPasses === 'function') sweepFriendPasses();
  },

  /** Check one card and take it. A card that does not check out gives no key. Returns what it meant, or null. */
  async ingest(raw) {
    const card = typeof HosKeyCard !== 'undefined' ? HosKeyCard.parseCard(raw) : null;
    if (!card) return null;
    this._asked.delete(card.key);
    const good = await HosKeyCard.cardFor(card, card.key, (pub, m, sig) => window.pqVerifyMessage(pub, m, sig));
    if (!good || card.key === myKey || !this.ready()) {
      if (!good) {
        console.warn('key cards: refused a card that does not check out as its owner\'s');
        if (!this.sealingKey(card.key)) this._noCard.add(card.key);
      }
      this._settle(card.key);
      return null;
    }
    const seen = hosDmStore.takeKeyCard(card);
    this._noCard.delete(card.key);
    this._settle(card.key);
    if (seen === 'changed' && typeof activeDmPartner !== 'undefined' && activeDmPartner === card.key) this.showChangeNotice(card.key);
    return seen;
  },

  _settle(person) {
    const waiting = this._waiters.get(person);
    if (!waiting) return;
    this._waiters.delete(person);
    for (const resolve of waiting) resolve();
  },

  /**
   * The key to seal to `person`, asking for their card first when none is
   * pinned and waiting up to 8 seconds for it. Null when there is none (the
   * caller says why: `say`).
   */
  async ensure(person) {
    const now = this.sealingKey(person);
    if (now || this.changed(person) || !this.ready()) return now;
    // Nothing to ask for (not a person's key), or no socket to ask on: no wait.
    if (!HosKeyCard.isKeyText(person) || typeof ws === 'undefined' || !ws || ws.readyState !== WebSocket.OPEN) return null;
    this._asked.delete(person);
    this._noCard.delete(person);
    const arrived = new Promise((resolve) => {
      if (!this._waiters.has(person)) this._waiters.set(person, []);
      this._waiters.get(person).push(resolve);
    });
    this.want(person, false);
    this._flush();
    await Promise.race([arrived, new Promise((r) => setTimeout(r, KEY_CARD_WAIT_MS))]);
    return this.sealingKey(person);
  },

  /** Why nothing can be sealed to `person` now: 'changed' | 'fetching' | 'no_card', or null. */
  refusal(person) {
    if (this.sealingKey(person)) return null;
    if (this.changed(person)) return 'changed';
    if (this._noCard.has(person) || typeof HosKeyCard === 'undefined' || !HosKeyCard.isKeyText(person)) return 'no_card';
    return 'fetching';
  },

  _name(person) {
    const p = typeof peerData !== 'undefined' ? peerData[person] : null;
    return (p && p.display_name) || (typeof shortKey === 'function' ? shortKey(person) : String(person).slice(0, 8));
  },

  /** The sentence for `refusal`, naming them the way the page does. */
  refusalText(person) {
    const why = this.refusal(person);
    return why ? HosKeyCard.refusalText(why, this._name(person)) : null;
  },

  /** Say in the conversation why nothing was sent to `person`: the change notice, or the sentence. */
  say(person) {
    if (this.refusal(person) === 'changed') { this.showChangeNotice(person); return; }
    const text = this.refusalText(person);
    if (text && typeof addSystemMessage === 'function') addSystemMessage(text);
  },

  /** A conversation with `person` was opened: ask for their card, and show a change waiting. */
  onConversationOpened(person) {
    this.want(person, false);
    this._flush();
    if (this.changed(person)) this.showChangeNotice(person);
  },

  /**
   * A group roster member's Kyber key (`GET /api/v2/groups/{id}/members`
   * serves each member's card): only from a card that checks out as that very
   * member's, null otherwise, so a group key is never sealed to a key the
   * server put in. (Not held against the pins: see docs/design/key-cards.md.)
   */
  async memberKey(m) {
    if (!m || typeof m.pubkey !== 'string' || typeof HosKeyCard === 'undefined') return null;
    const card = HosKeyCard.parseCard(m.key_card);
    if (!card) return null;
    const good = await HosKeyCard.cardFor(card, m.pubkey, (pub, msg, sig) => window.pqVerifyMessage(pub, msg, sig));
    return good ? card.kyber : null;
  },

  /** Accept `person`'s changed key: it is sealed to from now on. */
  accept(person) {
    return this.ready() ? !!hosDmStore.acceptKey(person) : false;
  },

  /**
   * The notice that `person`'s key changed, with Accept, drawn in the
   * conversation the way the reach notices are (chat-privacy.js onReachRefused).
   */
  showChangeNotice(person) {
    if (typeof appendMessage !== 'function' || typeof document === 'undefined') return;
    const el = document.createElement('div');
    el.className = 'message system key-changed';
    el.style.cssText = 'padding:var(--space-sm) var(--space-md);border-left:3px solid var(--warning);';
    const text = document.createElement('div');
    text.style.cssText = 'color:var(--text);margin:0 0 var(--space-xs) 0;';
    text.textContent = HosKeyCard.changeNotice(this._name(person));
    const btn = document.createElement('button');
    btn.className = 'vr-btn';
    btn.textContent = HosKeyCard.ACCEPT_LABEL;
    btn.onclick = () => {
      if (this.accept(person)) {
        btn.disabled = true;
        btn.textContent = 'Accepted';
      }
    };
    el.appendChild(text);
    el.appendChild(btn);
    appendMessage(el);
  },
};

window.hosKeyCards = hosKeyCards;
