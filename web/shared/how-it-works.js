// ── how-it-works.js ───────────────────────────────────────────────────────
// "How this page works" on the website (2026-10-20), the web twin of the desktop
// app's src/gui/widgets/how_it_works.rs: a page's explanations folded behind one
// small toggle by its heading, so the page shows its things first and its
// reasons one click away. The Apps page (/pages/apps-app.js) and the Files page
// (/pages/files-app.js) use it, as the desktop's Apps and Files pages do.
//
// Closed until the person opens it, and remembered in this browser under one
// key per page (localStorage `hos_apps_how_open`, `hos_files_how_open`: "1" open,
// "0" closed; the desktop keeps config.json `apps_how_open`, `files_how_open`).
// Every read and write is inside try/catch: with storage refused (a private
// window, blocked site data) the fold still opens and closes, and is closed again
// at the next visit.
//
// The toggle is a real <button>: Enter and Space press it, Tab reaches it, and
// its aria-expanded says whether the fold is open, with aria-controls naming the
// fold. Its triangle points right when closed and down when open, as the
// desktop's; its words and hover words are the desktop's LABEL and TIP, which
// scripts/tests/apps-web.test.js reads out of the Rust.
//
// The fold is a box the page's width, a heading per part of the page (a dot in
// the part's colour beside its name, never the colour alone) and its lines. Its
// parts are drawn only while it is open (the page's `fill`), so a closed fold
// holds nothing a screen reader or a search could meet. Styles:
// /shared/how-it-works.css, theme variables only.
//
// Built with createElement and textContent only. A classic script in the browser
// (HosHowItWorks on window), a CommonJS module under Node.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** The toggle's words (the desktop's widgets::how_it_works::LABEL). */
  const LABEL = 'How this page works';
  /** Its hover words (the desktop's TIP). */
  const TIP = 'What this page does and does not do, and why, with the research behind each reason.';
  /** The value kept while the fold is open, and while it is closed. */
  const OPEN = '1';
  const CLOSED = '0';
  /** The colours a part's dot may take (the six of the Apps page's parts; the CSS gives each). */
  const TINTS = ['red', 'green', 'blue', 'cyan', 'magenta', 'yellow'];

  /** Whether the fold kept under `key` is open: closed when nothing is kept, the value is not
      "1", or the storage is refused. */
  function readOpen(storage, key) {
    try {
      return !!storage && storage.getItem(key) === OPEN;
    } catch (e) {
      return false;
    }
  }

  /** Keep whether the fold is open. False when the storage refused it (the fold carries on). */
  function writeOpen(storage, key, open) {
    try {
      if (!storage) return false;
      storage.setItem(key, open ? OPEN : CLOSED);
      return true;
    } catch (e) {
      return false;
    }
  }

  /**
   * One page's fold. `opts`:
   *   document, storage (localStorage, or null when refused), key (where it is kept),
   *   id (the fold's element id; the toggle's is id + '-toggle'),
   *   fill(f) (draws the parts while it is open: f.part(name, tint) a part's heading, f.line(kids)
   *   one line of explanation, `kids` words or nodes), onChange(open) (optional).
   * Returns { toggle, panel, isOpen(), setOpen(open), render() }: the page puts `toggle` by its
   * heading and `panel` under the heading's line, and calls render() when what `fill` draws changes.
   */
  function createFold(opts) {
    const doc = opts.document;
    const id = String(opts.id || 'how');
    let open = readOpen(opts.storage, opts.key);

    function el(tag, cls, text) {
      const n = doc.createElement(tag);
      if (cls) n.className = cls;
      if (text != null) n.textContent = String(text);
      return n;
    }

    const toggle = el('button', 'how-toggle');
    toggle.setAttribute('type', 'button');
    toggle.setAttribute('id', id + '-toggle');
    toggle.setAttribute('aria-controls', id);
    toggle.setAttribute('title', TIP);
    const mark = el('span', 'how-mark');
    mark.setAttribute('aria-hidden', 'true');
    toggle.appendChild(mark);
    toggle.appendChild(el('span', 'how-label', LABEL));

    const panel = el('div', 'how-panel');
    panel.setAttribute('id', id);
    panel.setAttribute('role', 'region');
    panel.setAttribute('aria-labelledby', id + '-toggle');

    /** What `fill` draws with: a part's heading and a line, appended to the fold in order. */
    const drawer = {
      part(name, tint) {
        const h = el('h2', 'how-part', name);
        if (TINTS.indexOf(tint) >= 0) h.setAttribute('data-tint', tint);
        panel.appendChild(h);
        return h;
      },
      line(kids) {
        const p = el('p', 'how-line');
        for (const k of [].concat(kids == null ? [] : kids)) {
          if (k == null || k === false) continue;
          p.appendChild(typeof k === 'string' ? doc.createTextNode(k) : k);
        }
        panel.appendChild(p);
        return p;
      },
    };

    function render() {
      toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
      toggle.className = 'how-toggle' + (open ? ' how-open' : '');
      panel.hidden = !open;
      panel.replaceChildren();
      if (open && typeof opts.fill === 'function') opts.fill(drawer);
    }

    function setOpen(v) {
      open = !!v;
      writeOpen(opts.storage, opts.key, open);
      render();
      if (typeof opts.onChange === 'function') opts.onChange(open);
    }

    toggle.addEventListener('click', () => setOpen(!open));
    render();

    return { toggle, panel, isOpen: () => open, setOpen, render };
  }

  const api = { LABEL, TIP, OPEN, CLOSED, TINTS, readOpen, writeOpen, createFold };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.HosHowItWorks = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
