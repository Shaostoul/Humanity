/* ── phrase-cover.js ─────────────────────────────────────────────────────────
 * The recovery phrase, and the device-link code that carries the same key,
 * covered until the person shows them (operator, 2026-10-16: livestreamers
 * may reveal their phrase by accident while showing people what first run
 * looks like, and the same for anyone signing up in public).
 *
 * Wherever the 24 words or the code appear, they start covered: in their place
 * the numbers and dots (the code: a blank square), a line saying when it is
 * safe to show them, and one button. While covered the words are not in the
 * page at all (not blurred text a stylesheet could undo, and nothing a screen
 * reader or a page capture could read). They cover themselves again when the
 * browser window loses focus or the tab is hidden (the person switched to
 * their streaming or call app), and each time a window holding them opens.
 * The desktop app does the same (src/gui/widgets/phrase_cover.rs), in the
 * same words.
 *
 * HosPhraseCover.words(host, phrase, opts) and HosPhraseCover.code(host, draw)
 * fill `host` and return { hide(), isShown() }; opts.onChange(shown) follows
 * the words' cover, for a Copy button that must not copy covered words.
 */
(function (root) {
  'use strict';

  const WORDS = Object.freeze({
    warning: 'Only show these when nobody can see your screen: not while streaming, recording or sharing your screen, and not where people can look over your shoulder.',
    // Held for HOLD_SECONDS, so a stray click never shows them (2026-10-16: "a 3 second hold",
    // like the other hold-to-confirm buttons, /shared/hold-confirm.js).
    show: 'Hold to show the words',
    hide: 'Hide the words',
    coversItself: 'They cover themselves again when you switch to another window.',
    codeWarning: 'Only show this code when nobody can see your screen or record it: anyone who scans it, even from a recording, gets your account.',
    showCode: 'Hold to show the code',
    holdTip: 'Press and hold for 3 seconds, so a stray click never shows them.',
    holdMissing: 'This page could not load its hold-to-show button, so nothing is shown. Reload the page.',
    hideCode: 'Hide the code',
    // As wide as the longest word on the list (8 letters).
    coveredWord: '••••••••',
    codeHidden: 'Code hidden',
  });

  /** How long a show button is held before it shows. */
  const HOLD_SECONDS = 3;

  const LINE_STYLE = 'font-size:.75rem;line-height:1.5;margin:var(--space-sm) 0;';
  const BUTTON_STYLE = 'border-radius:var(--radius);padding:var(--space-sm) var(--space-xl);font-size:.8rem;cursor:pointer;';

  /**
   * A cover over `host`: `o.covered(body)` draws the covered state, `o.shown(body)` the shown one.
   * Covered first; the button switches; window blur or a hidden tab covers it again.
   */
  function cover(host, o) {
    const doc = host.ownerDocument;
    const win = doc.defaultView || root;
    let up = false;

    function render() {
      host.replaceChildren();
      const body = doc.createElement('div');
      body.className = up ? 'phrase-cover-shown' : 'phrase-cover-covered';
      // In the page before it is drawn: a QR is drawn onto a canvas found by its id.
      host.appendChild(body);
      (up ? o.shown : o.covered)(body);
      const line = doc.createElement('p');
      line.className = 'phrase-cover-line';
      line.textContent = up ? WORDS.coversItself : o.warning;
      line.style.cssText = LINE_STYLE + (up ? 'color:var(--text-muted);' : 'color:var(--warning);');
      const button = doc.createElement('button');
      button.type = 'button';
      button.className = up ? 'phrase-cover-hide' : 'phrase-cover-show';
      if (up) {
        // Hiding is always safe: one click.
        button.textContent = o.hideLabel;
        button.style.cssText = BUTTON_STYLE + 'background:none;border:1px solid var(--border);color:var(--text-muted);';
        button.addEventListener('click', () => { up = false; render(); });
      } else {
        // Showing is held for HOLD_SECONDS: the ring fills, letting go early starts over, and a
        // click does nothing (holdToConfirm, /shared/hold-confirm.js, whose CSS draws the ring).
        const ring = doc.createElement('span');
        ring.className = 'hold-confirm-ring';
        ring.setAttribute('aria-hidden', 'true');
        const label = doc.createElement('span');
        label.textContent = o.showLabel;
        button.append(ring, label);
        button.title = WORDS.holdTip;
        button.style.cssText = BUTTON_STYLE + 'display:inline-flex;align-items:center;gap:8px;background:var(--accent);border:none;color:var(--text-on-accent);font-weight:700;user-select:none;-webkit-user-select:none;touch-action:none;';
        const hold = root.holdToConfirm;
        if (typeof hold === 'function') hold(button, { seconds: HOLD_SECONDS, onConfirm: () => { up = true; render(); } });
        else { button.disabled = true; button.title = WORDS.holdMissing; }
      }
      if (up) host.append(button, line);
      else host.append(line, button);
      // Whatever beside the cover acts on what it covers (a Copy button) follows it.
      if (o.onChange) o.onChange(up);
    }

    function hide() {
      if (!up) return;
      up = false;
      render();
    }
    function detach() {
      win.removeEventListener('blur', onBlur);
      doc.removeEventListener('visibilitychange', onVisibility);
    }
    function onBlur() {
      if (!host.isConnected) return detach();
      hide();
    }
    function onVisibility() {
      if (!host.isConnected) return detach();
      if (doc.visibilityState === 'hidden') hide();
    }
    win.addEventListener('blur', onBlur);
    doc.addEventListener('visibilitychange', onVisibility);
    render();
    return { hide, isShown: () => up };
  }

  /** The 24 words of `phrase`, numbered in four columns, covered until Show the words.
   *  `opts.onChange(shown)` is called each time the cover is drawn, covered or shown, the first
   *  time included: a Copy button beside the words is there only while they are shown (BUG-214),
   *  as on the desktop. */
  function words(host, phrase, opts) {
    const list = String(phrase || '').trim().split(/\s+/).filter(Boolean);
    const doc = host.ownerDocument;
    const grid = (shown) => (body) => {
      body.style.cssText = 'display:grid;grid-template-columns:repeat(4,1fr);gap:var(--space-sm);margin:var(--space-lg) 0 var(--space-sm)';
      list.forEach((word, i) => {
        const cell = doc.createElement('div');
        cell.className = 'phrase-cover-cell';
        cell.style.cssText = 'background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);padding:var(--space-sm) var(--space-md);display:flex;align-items:baseline;gap:var(--space-sm)';
        const n = doc.createElement('span');
        n.textContent = (i + 1) + '.';
        n.style.cssText = 'font-size:.6rem;color:var(--text-muted);min-width:16px;text-align:right';
        const w = doc.createElement('span');
        w.className = 'phrase-cover-word';
        w.textContent = shown ? word : WORDS.coveredWord;
        w.style.cssText = 'font-size:.84rem;font-weight:600;font-family:monospace;color:' + (shown ? 'var(--accent)' : 'var(--text-muted)');
        cell.append(n, w);
        body.appendChild(cell);
      });
    };
    return cover(host, { covered: grid(false), shown: grid(true), warning: WORDS.warning, showLabel: WORDS.show, hideLabel: WORDS.hide, onChange: opts && opts.onChange });
  }

  /** A code that carries the account (the device-link QR and its text), drawn into `body` by
   *  `draw(body)`, covered until Show the code: in its place a blank square. */
  function code(host, draw) {
    const doc = host.ownerDocument;
    const covered = (body) => {
      const square = doc.createElement('div');
      square.className = 'phrase-cover-code-hidden';
      square.textContent = WORDS.codeHidden;
      square.style.cssText = 'width:220px;max-width:100%;aspect-ratio:1;margin:0 auto;display:flex;align-items:center;justify-content:center;background:var(--bg);border:1px dashed var(--border);border-radius:var(--radius);color:var(--text-muted);font-size:.8rem';
      body.appendChild(square);
    };
    return cover(host, { covered, shown: draw, warning: WORDS.codeWarning, showLabel: WORDS.showCode, hideLabel: WORDS.hideCode });
  }

  const api = { words, code, WORDS, HOLD_SECONDS };
  root.HosPhraseCover = api;
  if (typeof module === 'object' && module && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
