// ── self-mail.js ──────────────────────────────────────────────────────────
// Mail to my own mailbox waits a random while before it goes (2026-11-06,
// docs/BUGS.md BUG-259): the website's twin of the desktop's
// src/net/self_mail.rs, held to it by scripts/tests/self-mail-web.test.js.
//
// WHY. A private message is two sealed envelopes: the recipient's copy, and my
// own copy in my own mailbox so my other devices can show what I sent. Both
// apps used to send the two one right after the other, and the server numbered
// mailbox rows in arrival order, so a copy of its database (a court order, a
// breach, a backup with its key) showed two neighbouring envelopes of the same
// size, one for the recipient and one for me, and paired who wrote to whom.
// The server now keeps rows by mailbox with nothing finer than the day
// (src/relay/storage/dms.rs). This is the other half: my own copy leaves
// between SELF_MAIL_DELAY_MIN_MS and SELF_MAIL_DELAY_MAX_MS later, at a random
// moment, so it never lands beside the recipient's copy in the server's write
// log or its continuous backup either, wherever other mail lands between.
//
// THE RULE: every `dm_put` to my own key sent on a socket this file is attached
// to waits (my copy of a message, of a follow or a pass, a choice note), in
// order: each one leaves no earlier than the one before it. A note that pairs
// with nothing and keeps its own "not sent yet" mark until it is sent (a block
// note, the allow-list note) uses `hosSelfMailSendNow`, which sends at once.
//
// What waits is kept until it goes: a socket that is not open when one is due
// keeps it until the next socket is attached and open, and leaving the page
// (pagehide) sends everything still waiting on an open socket at once. Only a
// browser killed in the minute or two before a copy leaves loses it, and then
// only my OTHER devices miss it: this one already keeps its own copy.
//
// A classic script in the browser (its names land on window), a CommonJS
// module in Node.
(function (root) {
  'use strict';

  /** The shortest wait, in milliseconds (the desktop's SELF_MAIL_DELAY_MIN_MS). */
  const SELF_MAIL_DELAY_MIN_MS = 20000;
  /** The longest wait, in milliseconds (the desktop's SELF_MAIL_DELAY_MAX_MS). */
  const SELF_MAIL_DELAY_MAX_MS = 120000;
  /** How soon to look again when a copy is due but no socket is open. */
  const RETRY_MS = 3000;
  const OPEN = 1; // WebSocket.OPEN

  /** True when `frame` (a JSON string or an object) is a `dm_put` to `me`. */
  function isSelfPut(frame, me) {
    if (!me) return false;
    let v = frame;
    if (typeof frame === 'string') {
      if (frame.indexOf('"dm_put"') < 0) return false;
      try { v = JSON.parse(frame); } catch (_) { return false; }
    }
    return !!v && typeof v === 'object' && v.type === 'dm_put' && v.to === me;
  }

  /** A random wait between the two limits, from the system's randomness where there is some. */
  function randomDelay(random) {
    let r;
    if (typeof random === 'function') {
      r = random();
    } else if (root.crypto && typeof root.crypto.getRandomValues === 'function') {
      const b = new Uint32Array(1);
      root.crypto.getRandomValues(b);
      r = b[0] / 4294967296;
    } else {
      r = Math.random();
    }
    return SELF_MAIL_DELAY_MIN_MS + Math.floor(r * (SELF_MAIL_DELAY_MAX_MS - SELF_MAIL_DELAY_MIN_MS + 1));
  }

  /**
   * One queue of mail to myself. `opts` (all optional, for tests): `now()` in
   * ms, `random()` in [0, 1), `setTimer(fn, ms)` / `clearTimer(id)`.
   */
  function createSelfMail(opts) {
    const o = opts || {};
    const now = o.now || (() => Date.now());
    const setTimer = o.setTimer || ((fn, ms) => setTimeout(fn, ms));
    const clearTimer = o.clearTimer || ((id) => clearTimeout(id));
    const held = []; // [{ due, data }], oldest first, due never decreasing
    let current = null; // { sock, raw, me }
    let timer = null;

    function schedule() {
      if (timer !== null) { clearTimer(timer); timer = null; }
      if (!held.length) return;
      const open = current && current.sock.readyState === OPEN;
      const wait = open ? Math.max(0, held[0].due - now()) : RETRY_MS;
      timer = setTimer(release, wait);
    }

    /** Send every copy that is due, when a socket is open; then wait for the next. */
    function release() {
      timer = null;
      if (current && current.sock.readyState === OPEN) {
        const t = now();
        while (held.length && held[0].due <= t) current.raw(held.shift().data);
      }
      schedule();
    }

    /** Hold `data` (the frame as sent) for a random while, after every copy already waiting. */
    function hold(data) {
      const last = held.length ? held[held.length - 1].due : 0;
      held.push({ due: Math.max(last, now() + randomDelay(o.random)), data });
      schedule();
    }

    /**
     * Attach to a freshly made socket: its `send` holds mail to `me` (a
     * function answering my key, read at each send) and passes everything else
     * straight on. Copies still waiting from an earlier socket go out on this
     * one when they are due. Returns the socket.
     */
    function attach(sock, me) {
      const raw = sock.send.bind(sock);
      const who = typeof me === 'function' ? me : () => me;
      current = { sock, raw, me: who };
      sock.send = function (data) {
        if (isSelfPut(data, who())) { hold(data); return undefined; }
        return raw(data);
      };
      sock.hosSendNow = raw;
      schedule();
      return sock;
    }

    /** Send everything still waiting at once, on an open socket (the page is going away). */
    function flush() {
      if (!current || current.sock.readyState !== OPEN) return 0;
      let n = 0;
      while (held.length) { current.raw(held.shift().data); n++; }
      schedule();
      return n;
    }

    return {
      attach, flush, release,
      waiting: () => held.map((h) => ({ due: h.due, data: h.data })),
    };
  }

  /** Send `data` at once on `sock`, past the wait: a note that pairs with nothing. */
  function hosSelfMailSendNow(sock, data) {
    if (!sock) return;
    if (typeof sock.hosSendNow === 'function') sock.hosSendNow(data);
    else sock.send(data);
  }

  // The page's one queue (the browser): every socket Chat opens attaches to it.
  const hosSelfMail = createSelfMail();
  if (root && typeof root.addEventListener === 'function') {
    root.addEventListener('pagehide', () => { hosSelfMail.flush(); });
  }

  const api = {
    SELF_MAIL_DELAY_MIN_MS, SELF_MAIL_DELAY_MAX_MS,
    isSelfPut, randomDelay, createSelfMail, hosSelfMail, hosSelfMailSendNow,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else Object.assign(root, api);
})(typeof globalThis !== 'undefined' ? globalThis : this);
