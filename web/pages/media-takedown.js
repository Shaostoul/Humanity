// ── media-takedown.js ─────────────────────────────────────────────────────
// "Report a copyright problem" and the public takedown log on the website
// (2026-10-13; the operator's answers of that day: "a simple form so it's
// easy... a dedicated button somewhere on the media page... We can log the
// reports publicly"). The relay half is src/relay/media_takedown.rs, whose
// header holds the routes, bodies and answers this file builds to.
//
// It holds three things:
// - The FORM (`createTakedownForm`): opened from an item's own page and from the
//   foot of the Media page, in the page's dialog style. It works signed out: it
//   sends POST /api/media/takedown with {item, name, contact, statement,
//   good_faith}, unsigned, with no token, and says in one sentence what happens
//   next. Send stays off until the box is ticked.
// - The ADMINS' VIEW (`mountTakedownAdmin`): on the Media page's Admin tab, the
//   reports still open (POST /api/media/takedowns/open, signed
//   `media_takedowns`), each with what the person wrote, a note for the public
//   log, and Remove item or Keep (POST /api/media/takedown/decide, signed
//   `media_takedown:<id>:<removed|kept>`).
// - The PUBLIC LOG (`renderTakedownLog`, `loadTakedownLog`): GET
//   /api/media/takedowns, shown on /rules (the element #takedown-log). It shows
//   the item, the dates, the outcome and the admin's note; the relay never sends
//   who reported or how to reach them, and this file reads only the fields it
//   shows, so nothing else could reach the page.
//
// Built with createElement and textContent only: nothing the server sends is
// ever read as HTML. Colours come from the theme (classes in /shared/media.css
// and the rules page's own styles, both theme tokens). A classic script in the
// browser (before /pages/media-item.js and /pages/media-app.js on the Media
// page; alone on /rules), a CommonJS module under Node, where
// scripts/tests/media-takedown-web.test.js runs it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  // The relay's limits (src/relay/media_takedown.rs NAME_MAX, CONTACT_MAX, STATEMENT_MAX, NOTE_MAX).
  const NAME_MAX = 200;
  const CONTACT_MAX = 300;
  const STATEMENT_MAX = 2000;
  const NOTE_MAX = 1000;

  const TAKEDOWN_URL = '/api/media/takedown';
  const LOG_URL = '/api/media/takedowns';
  const OPEN_URL = '/api/media/takedowns/open';
  const DECIDE_URL = '/api/media/takedown/decide';
  // Where the public log is shown (web/pages/rules.html).
  const LOG_PAGE = '/rules#takedown-log';

  const WORDS = Object.freeze({
    button: 'Report a copyright problem',
    buttonTip: 'Is this your work, put here without your permission? Tell the admins.',
    title: 'Report a copyright problem',
    lead: "If something on this Media page is your work and it was put here without your permission, tell this server's admins. You do not need to be signed in.",
    // The one sentence on what happens next (the operator's design, 2026-10-13).
    next: 'An admin looks at it and removes the item or keeps it; the outcome appears in the public takedown log, without your name or contact.',
    itemLabel: "The item's number or address",
    itemHint: 'For example 12, or united-humanity.us/media/12.',
    nameLabel: 'Your name (optional)',
    contactLabel: 'How to reach you (optional; only the admins see it)',
    statementLabel: 'Why the work is yours',
    statementHint: 'For example: where and when you first published it.',
    // The relay's GOOD_FAITH_LINE, word for word (the web test holds them together).
    goodFaith: 'I believe in good faith that this use is not allowed by me or the law, and what I wrote is accurate.',
    send: 'Send',
    sending: 'Sending...',
    cancel: 'Cancel',
    close: 'Close',
    needItem: "Give the item's number, or paste its address.",
    needStatement: 'Say why the work is yours.',
    needBox: 'Tick the box first.',
    noAnswer: 'The server could not be reached, so the report was not sent. Try again in a moment.',
    logLink: 'The public takedown log',
    // The log.
    logEmpty: 'No copyright report has been decided yet.',
    logFailed: 'The takedown log could not be loaded: ',
    removed: 'Removed',
    kept: 'Kept',
    // The admins' view.
    adminLead: "Reports sent with Report a copyright problem. Your decision and your note go into the public takedown log; the person's name, contact and statement never do.",
    adminNone: 'No copyright reports are waiting.',
    adminNotePlaceholder: 'A note for the public log (optional). Do not put the person\'s name or contact in it.',
    removeItem: 'Remove item',
    keep: 'Keep',
    gone: '(no longer on the Media page)',
    removeConfirm: 'Remove this item from the Media page? The decision and your note go into the public takedown log.',
    cannotSign: 'This page could not sign the request with your identity. Open Chat in this browser first, then try again.',
  });

  // ── Small helpers ──

  function makeH(doc) {
    return function h(tag, props, kids) {
      const el = doc.createElement(tag);
      const p = props || {};
      for (const k of Object.keys(p)) {
        const v = p[k];
        if (v === undefined || v === null) continue;
        if (k === 'text') el.textContent = String(v);
        else if (k === 'on') { for (const ev of Object.keys(v)) el.addEventListener(ev, v[ev]); }
        else if (k === 'dataset') { for (const d of Object.keys(v)) el.dataset[d] = String(v[d]); }
        else if (k === 'attrs') { for (const a of Object.keys(v)) el.setAttribute(a, String(v[a])); }
        else el[k] = v;
      }
      for (const c of [].concat(kids == null ? [] : kids)) {
        if (c == null || c === false) continue;
        el.appendChild(typeof c === 'string' ? doc.createTextNode(c) : c);
      }
      return el;
    };
  }

  function jsonPost(body) {
    return { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
  }

  /** The relay's words for a refusal: its plain text, or the status when it sent a page or nothing. */
  async function refusalWords(res) {
    let text = '';
    try { text = String(await res.text()).trim(); } catch (e) { text = ''; }
    if (text.startsWith('{')) {
      try { const j = JSON.parse(text); if (j && typeof j.error === 'string' && j.error) return j.error; } catch (e) { /* not JSON */ }
    }
    if (!text || text.startsWith('<') || text.startsWith('{')) return 'The server answered ' + res.status + '.';
    return text;
  }

  /** A JSON answer, or throws Error(the relay's words). */
  async function answerOf(res) {
    if (!res || !res.ok) throw new Error(res ? await refusalWords(res) : WORDS.noAnswer);
    return res.json();
  }

  /** The day of a Unix-ms time, as 2026-10-13 (UTC), or '' when there is none. */
  function dayOf(ms) {
    const n = Number(ms);
    if (!Number.isFinite(n) || n <= 0) return '';
    try { return new Date(n).toISOString().slice(0, 10); } catch (e) { return ''; }
  }

  /**
   * The item's number from what a person typed: "12", "#12", "/media/12" or a whole address
   * ending in /media/12 (a query or a fragment after it is fine). Null when it names none.
   */
  function itemNumberFrom(text) {
    const t = String(text == null ? '' : text).trim();
    let m = /^#?(\d{1,15})$/.exec(t);
    if (!m) m = /\/media\/(\d{1,15})\/?(?:[?#].*)?$/.exec(t);
    if (!m) return null;
    const n = Number(m[1]);
    return Number.isSafeInteger(n) && n > 0 ? n : null;
  }

  /** The body the relay takes: {item, name, contact, statement, good_faith}, the words trimmed. */
  function takedownBody(f) {
    return {
      item: f.item,
      name: String(f.name || '').trim(),
      contact: String(f.contact || '').trim(),
      statement: String(f.statement || '').trim(),
      good_faith: f.goodFaith === true,
    };
  }

  /** What is missing from a form before it can be sent, in words, or '' when nothing is. */
  function formProblem(f) {
    if (!f.item) return WORDS.needItem;
    if (!String(f.statement || '').trim()) return WORDS.needStatement;
    if (f.goodFaith !== true) return WORDS.needBox;
    return '';
  }

  /** How the log names an item: its title in quotes, or "Item 12" when it shows none. */
  function itemWords(id, title) {
    const t = String(title || '').trim();
    return t ? 'Item ' + id + ', "' + t + '"' : 'Item ' + id;
  }

  // ── The form ──

  /**
   * The form, drawn into `env.container` (an element, or a function answering one) when opened.
   * `env`: document, fetch, container. Returns {open(item), close(), isOpen(), state()}.
   * `open(item)` with an item ({id, title}) names it; with none, the form asks for its number.
   */
  function createTakedownForm(env) {
    const doc = env.document;
    const h = makeH(doc);
    let f = null;
    const box = () => (typeof env.container === 'function' ? env.container() : env.container);

    function open(item) {
      const id = item && Number.isSafeInteger(Number(item.id)) && Number(item.id) > 0 ? Number(item.id) : null;
      f = {
        item: id, title: id ? String((item && item.title) || '') : '', itemText: '', fixed: !!id,
        name: '', contact: '', statement: '', goodFaith: false, sending: false, error: '', done: '',
      };
      render();
    }
    function close() {
      f = null;
      const c = box();
      if (c) c.replaceChildren();
    }

    function render() {
      const c = box();
      if (!c) return;
      c.replaceChildren();
      if (!f) return;
      const kids = [
        h('button', { type: 'button', id: 'media-takedown-close', className: 'media-btn media-viewer-close', text: WORDS.close, on: { click: close } }),
        h('h2', { className: 'media-viewer-title', text: WORDS.title }),
      ];
      if (f.done) {
        kids.push(h('p', { className: 'media-hint', id: 'media-takedown-done', attrs: { role: 'status' }, text: f.done }));
        kids.push(h('p', { className: 'media-hint' }, [h('a', { href: LOG_PAGE, id: 'media-takedown-log-link', text: WORDS.logLink })]));
        kids.push(h('div', { className: 'media-form-buttons' }, [
          h('button', { type: 'button', id: 'media-takedown-ok', className: 'media-btn media-btn-primary', text: WORDS.close, on: { click: close } }),
        ]));
      } else {
        kids.push(h('p', { className: 'media-hint', id: 'media-takedown-lead', text: WORDS.lead }));
        kids.push(h('p', { className: 'media-hint', id: 'media-takedown-next', text: WORDS.next }));
        if (f.fixed) {
          kids.push(h('p', { className: 'media-viewer-meta', id: 'media-takedown-item', text: 'About: ' + itemWords(f.item, f.title) }));
        } else {
          const itemIn = h('input', { type: 'text', id: 'media-takedown-item-input', className: 'media-input', value: f.itemText, maxLength: 300, attrs: { 'aria-label': WORDS.itemLabel } });
          itemIn.addEventListener('input', () => { f.itemText = itemIn.value; f.item = itemNumberFrom(itemIn.value); });
          kids.push(h('label', { className: 'media-field' }, [WORDS.itemLabel, itemIn]));
          kids.push(h('p', { className: 'media-hint', text: WORDS.itemHint }));
        }
        const name = h('input', { type: 'text', id: 'media-takedown-name', className: 'media-input', value: f.name, maxLength: NAME_MAX, attrs: { autocomplete: 'name' } });
        name.addEventListener('input', () => { f.name = name.value; });
        const contact = h('input', { type: 'text', id: 'media-takedown-contact', className: 'media-input', value: f.contact, maxLength: CONTACT_MAX, attrs: { autocomplete: 'email' } });
        contact.addEventListener('input', () => { f.contact = contact.value; });
        const statement = h('textarea', { id: 'media-takedown-statement', className: 'media-input', value: f.statement, maxLength: STATEMENT_MAX, rows: 5, placeholder: WORDS.statementHint });
        statement.addEventListener('input', () => { f.statement = statement.value; });
        const tick = h('input', { type: 'checkbox', id: 'media-takedown-good-faith', checked: f.goodFaith });
        const send = h('button', {
          type: 'button', id: 'media-takedown-send', className: 'media-btn media-btn-danger', text: f.sending ? WORDS.sending : WORDS.send,
          disabled: f.sending || !f.goodFaith,
        });
        // Ticking the box turns Send on without drawing the form again (what was typed stays put).
        tick.addEventListener('change', () => { f.goodFaith = tick.checked === true; send.disabled = f.sending || !f.goodFaith; });
        send.addEventListener('click', submit);
        kids.push(
          h('label', { className: 'media-field' }, [WORDS.nameLabel, name]),
          h('label', { className: 'media-field' }, [WORDS.contactLabel, contact]),
          h('label', { className: 'media-field' }, [WORDS.statementLabel, statement]),
          h('label', { className: 'media-check media-takedown-check' }, [tick, ' ' + WORDS.goodFaith]),
        );
        if (f.error) kids.push(h('p', { className: 'media-warn', id: 'media-takedown-error', attrs: { role: 'alert' }, text: f.error }));
        kids.push(h('div', { className: 'media-form-buttons' }, [
          send,
          h('button', { type: 'button', id: 'media-takedown-cancel', className: 'media-btn', text: WORDS.cancel, on: { click: close } }),
        ]));
      }
      const overlay = h('div', { id: 'media-takedown-open', className: 'media-viewer', attrs: { role: 'dialog', 'aria-label': WORDS.title } }, [
        h('div', { className: 'media-viewer-box media-report-box media-takedown-box' }, kids),
      ]);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
      c.appendChild(overlay);
    }

    async function submit() {
      const mine = f;
      if (!mine || mine.sending) return;
      const problem = formProblem(mine);
      if (problem) { mine.error = problem; render(); return; }
      mine.sending = true;
      mine.error = '';
      render();
      let words = '';
      let ok = false;
      try {
        const a = await answerOf(await env.fetch(TAKEDOWN_URL, jsonPost(takedownBody(mine))));
        ok = true;
        words = a && typeof a.words === 'string' && a.words ? a.words : 'Received.';
      } catch (e) {
        words = e && e.message ? e.message : WORDS.noAnswer;
      }
      if (f !== mine) return; // closed meanwhile
      mine.sending = false;
      if (ok) mine.done = words; else mine.error = words;
      render();
    }

    return { open, close, isOpen: () => !!f, state: () => f };
  }

  // ── The public log ──

  /** Draw the log answer `answer` ({takedowns: [...]}) into `container`. Only the shown fields are read. */
  function renderTakedownLog(container, answer, doc) {
    const h = makeH(doc);
    const rows = answer && Array.isArray(answer.takedowns) ? answer.takedowns : [];
    if (!rows.length) {
      container.replaceChildren(h('p', { className: 'takedown-log-empty', id: 'takedown-log-empty', text: WORDS.logEmpty }));
      return;
    }
    const list = h('ul', { className: 'takedown-log-list', id: 'takedown-log-list' });
    for (const r of rows) {
      const id = Number(r && r.id) || 0;
      const itemId = Number(r && r.item_id) || 0;
      const outcome = r && r.outcome === 'removed' ? WORDS.removed : r && r.outcome === 'kept' ? WORDS.kept : '';
      const when = 'Reported ' + (dayOf(r && r.created_at) || 'on an unknown day') + ', decided ' + (dayOf(r && r.decided_at) || 'on an unknown day');
      const note = r && typeof r.note === 'string' ? r.note.trim() : '';
      list.appendChild(h('li', { className: 'takedown-log-row', dataset: { takedown: id } }, [
        h('strong', { className: 'takedown-log-outcome takedown-log-' + (r && r.outcome === 'removed' ? 'removed' : 'kept'), text: outcome }),
        ' ',
        h('span', { className: 'takedown-log-item', text: itemWords(itemId, r && r.item_title) }),
        h('span', { className: 'takedown-log-when', text: ' · ' + when + ' · #' + id }),
        note ? h('p', { className: 'takedown-log-note', text: note }) : null,
      ]));
    }
    container.replaceChildren(list);
  }

  /** Fetch the log and draw it into `container`; a failure says so in words. `env`: document, fetch. */
  async function loadTakedownLog(container, env) {
    try {
      const a = await answerOf(await env.fetch(LOG_URL, { cache: 'no-cache' }));
      renderTakedownLog(container, a, env.document);
    } catch (e) {
      const h = makeH(env.document);
      container.replaceChildren(h('p', { className: 'takedown-log-empty', id: 'takedown-log-error', text: WORDS.logFailed + (e && e.message ? e.message : String(e)) }));
    }
  }

  // ── The admins' view ──

  /** The purpose a decision is signed over: media_takedown:<id>:<removed|kept>. */
  function decidePurpose(id, state) {
    return 'media_takedown:' + id + ':' + state;
  }

  /**
   * The open reports, with Remove item and Keep, drawn into `container`. `env`: document, fetch,
   * sign(purpose) -> Promise<{key, timestamp, sig}|null>, confirm(words) -> Promise<boolean>
   * (optional), onChange() (optional: the Media list changed). Returns {state, ready, reload}.
   */
  function mountTakedownAdmin(container, env) {
    const h = makeH(env.document);
    const st = { requests: null, notes: new Map(), busy: new Set(), msg: '', warn: false };

    async function signedPost(url, purpose, body) {
      const auth = await env.sign(purpose);
      if (!auth) throw new Error(WORDS.cannotSign);
      return answerOf(await env.fetch(url, jsonPost(Object.assign({}, body || {}, auth))));
    }

    async function load() {
      try {
        const a = await signedPost(OPEN_URL, 'media_takedowns');
        st.requests = Array.isArray(a.requests) ? a.requests : [];
      } catch (e) {
        st.requests = st.requests || [];
        st.msg = 'The copyright reports could not be loaded: ' + e.message;
        st.warn = true;
      }
      render();
    }

    async function decide(r, state) {
      if (st.busy.has(r.id)) return;
      if (state === 'removed' && typeof env.confirm === 'function' && !(await env.confirm(WORDS.removeConfirm))) return;
      st.busy.add(r.id);
      render();
      try {
        const a = await signedPost(DECIDE_URL, decidePurpose(r.id, state), { id: r.id, state, note: String(st.notes.get(r.id) || '').trim() });
        st.msg = 'Report #' + r.id + ': ' + (state === 'removed'
          ? (a && a.item_already_gone ? 'the item was already gone; recorded as removed.' : 'the item was removed.')
          : 'the item was kept.') + ' It is in the public takedown log.';
        st.warn = false;
        st.notes.delete(r.id);
        if (state === 'removed' && typeof env.onChange === 'function') env.onChange();
      } catch (e) {
        st.msg = e.message;
        st.warn = true;
      }
      st.busy.delete(r.id);
      await load();
    }

    function render() {
      const kids = [h('p', { className: 'media-hint', id: 'media-takedowns-lead', text: WORDS.adminLead })];
      if (st.requests === null) kids.push(h('p', { className: 'media-hint', text: 'Loading...' }));
      else if (!st.requests.length) kids.push(h('p', { className: 'media-hint', id: 'media-takedowns-none', text: WORDS.adminNone }));
      for (const r of st.requests || []) {
        const busy = st.busy.has(r.id);
        const note = h('textarea', { className: 'media-input', rows: 2, maxLength: NOTE_MAX, value: st.notes.get(r.id) || '', placeholder: WORDS.adminNotePlaceholder, dataset: { takedownNote: r.id } });
        note.addEventListener('input', () => { st.notes.set(r.id, note.value); });
        const item = h('a', { href: '/media/' + (Number(r.item_id) || 0), text: itemWords(Number(r.item_id) || 0, r.item_title) });
        kids.push(h('div', { className: 'media-admin-row media-takedown-row', dataset: { takedown: r.id } }, [
          h('div', { className: 'media-admin-row-text' }, [
            h('strong', {}, ['Report #' + r.id + ' about ', item, r.item_on_page === false ? ' ' + WORDS.gone : '']),
            h('div', { className: 'media-hint', text: 'Received ' + (dayOf(r.created_at) || 'on an unknown day') }),
            h('div', { className: 'media-hint', text: 'Name: ' + (String(r.name || '').trim() || 'not given') }),
            h('div', { className: 'media-hint', text: 'Contact: ' + (String(r.contact || '').trim() || 'not given') }),
            h('p', { className: 'media-viewer-desc', text: String(r.statement || '') }),
            note,
          ]),
          h('button', { type: 'button', className: 'media-btn media-btn-small media-btn-danger', text: WORDS.removeItem, disabled: busy, dataset: { decide: 'removed' }, on: { click: () => decide(r, 'removed') } }),
          h('button', { type: 'button', className: 'media-btn media-btn-small', text: WORDS.keep, disabled: busy, dataset: { decide: 'kept' }, on: { click: () => decide(r, 'kept') } }),
        ]));
      }
      if (st.msg) kids.push(h('p', { className: 'media-hint' + (st.warn ? ' media-warn' : ''), id: 'media-takedowns-msg', text: st.msg }));
      kids.push(h('p', { className: 'media-hint' }, [h('a', { href: LOG_PAGE, text: WORDS.logLink })]));
      container.replaceChildren(...kids);
    }

    render();
    const ready = load();
    return { state: st, ready, reload: load };
  }

  // ── In the browser: the public log on whatever page holds #takedown-log (/rules) ──
  function bootLog() {
    const doc = root.document;
    const el = doc && doc.getElementById('takedown-log');
    if (!el) return;
    loadTakedownLog(el, { document: doc, fetch: (u, i) => root.fetch(u, i) });
  }

  const api = {
    NAME_MAX, CONTACT_MAX, STATEMENT_MAX, NOTE_MAX, TAKEDOWN_URL, LOG_URL, OPEN_URL, DECIDE_URL, LOG_PAGE, WORDS,
    itemNumberFrom, takedownBody, formProblem, itemWords, dayOf, decidePurpose,
    createTakedownForm, renderTakedownLog, loadTakedownLog, mountTakedownAdmin,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    root.MediaTakedown = api;
    const doc = root.document;
    if (doc && doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', bootLog);
    else bootLog();
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
