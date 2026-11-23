/* ── secret-field.js ─────────────────────────────────────────────────────────
 * Every box a person types a secret into (a passphrase, a PIN, a token: an
 * <input type="password">) gets a Show / Hide button beside it, a drawn eye
 * (2026-11-22). The operator, on his phone, in the window that asks for a
 * passphrase to encrypt his key: "I also have no way of previewing the
 * passphrase in the encrypt your private key window. It's just the dots with
 * encrypt or skip." With only dots a mistyped passphrase cannot be checked,
 * and a mistyped passphrase locks a person out of their own key.
 *
 * - Hidden by default. Hidden again when the person switches to another
 *   window or tab (a streaming or screen-sharing app among them), and a box in
 *   a window that closes is gone with it: a window opened again starts hidden.
 * - The button: an eye to show, an eye struck through to hide, named Show or
 *   Hide for a screen reader (aria-label, aria-pressed), with its hint; the
 *   words come from data/i18n/en.json by key (secret_field.*), the colours
 *   from the theme's tokens.
 * - Loaded on every page by shell.js; boxes added later (a dialog opening) are
 *   found as they appear. A box that already has its own button
 *   (data-secret-field="own") is left as it is.
 *
 * The desktop app's twin is src/gui/widgets/secret_field.rs, in the same words.
 */
(function (root) {
  'use strict';

  const SVG = 'http://www.w3.org/2000/svg';
  const DONE = 'data-secret-field';

  /** The words by key, with English beside them for a page that has not loaded i18n yet. */
  function word(key, english) {
    const i18n = root.i18n;
    if (i18n && typeof i18n.t === 'function') {
      const w = i18n.t(key);
      if (w && w !== key) return w;
    }
    return english;
  }

  /** The eye, drawn (an almond and a pupil), struck through when `struck`. */
  function eye(doc, struck) {
    const svg = doc.createElementNS(SVG, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', '20');
    svg.setAttribute('height', '20');
    svg.setAttribute('aria-hidden', 'true');
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '2');
    const lens = doc.createElementNS(SVG, 'path');
    lens.setAttribute('d', 'M2 12 C5 6 19 6 22 12 C19 18 5 18 2 12 Z');
    const pupil = doc.createElementNS(SVG, 'circle');
    pupil.setAttribute('cx', '12');
    pupil.setAttribute('cy', '12');
    pupil.setAttribute('r', '3');
    svg.appendChild(lens);
    svg.appendChild(pupil);
    if (struck) {
      const line = doc.createElementNS(SVG, 'line');
      line.setAttribute('x1', '4');
      line.setAttribute('y1', '20');
      line.setAttribute('x2', '20');
      line.setAttribute('y2', '4');
      svg.appendChild(line);
    }
    return svg;
  }

  /** Give one password box its button. Returns { button, hide(), isShown() }, or null when it
   *  already has one. */
  function attach(input) {
    if (!input || input.getAttribute(DONE)) return null;
    const doc = input.ownerDocument;
    input.setAttribute(DONE, 'on');
    const button = doc.createElement('button');
    button.type = 'button';
    button.className = 'hos-secret-toggle';
    button.style.cssText = 'margin-left:4px;min-width:36px;min-height:36px;padding:4px;'
      + 'background:transparent;border:1px solid var(--border, transparent);border-radius:var(--radius, 4px);'
      + 'color:var(--text-secondary);cursor:pointer;vertical-align:middle;line-height:0;';
    let shown = false;
    function draw() {
      button.replaceChildren(eye(doc, shown));
      const name = shown ? word('secret_field.hide', 'Hide') : word('secret_field.show', 'Show');
      button.setAttribute('aria-label', name);
      button.setAttribute('aria-pressed', shown ? 'true' : 'false');
      button.title = shown
        ? word('secret_field.hide_hint', 'Hides what you typed behind dots again.')
        : word('secret_field.show_hint', 'Shows what you typed, to check it.');
      // The site's hint (shell.js shows data-tip-desc in the person's hint mode).
      button.dataset.tipDesc = button.title;
      input.type = shown ? 'text' : 'password';
    }
    function set(next) {
      shown = next;
      draw();
    }
    button.addEventListener('click', function () {
      set(!shown);
      if (typeof input.focus === 'function') input.focus();
    });
    draw();
    if (input.parentNode) {
      if (input.nextSibling) input.parentNode.insertBefore(button, input.nextSibling);
      else input.parentNode.appendChild(button);
    }
    attached.push({ input: input, hide: function () { set(false); } });
    return { button: button, hide: function () { set(false); }, isShown: function () { return shown; } };
  }

  const attached = [];

  /** Hide every shown secret (the window lost focus or the tab was hidden). */
  function hideAll() {
    attached.forEach(function (a) { a.hide(); });
  }

  /** Give every password box under `node` its button (data-secret-field="own" skipped). */
  function scan(node) {
    if (!node || typeof node.querySelectorAll !== 'function') return;
    node.querySelectorAll('input[type="password"]').forEach(function (input) { attach(input); });
  }

  function start(doc) {
    scan(doc);
    const win = doc.defaultView || root;
    if (win && win.addEventListener) win.addEventListener('blur', hideAll);
    if (doc.addEventListener) {
      doc.addEventListener('visibilitychange', function () {
        if (doc.visibilityState === 'hidden') hideAll();
      });
    }
    const MO = (win && win.MutationObserver) || root.MutationObserver;
    if (MO && doc.body) {
      new MO(function (records) {
        records.forEach(function (r) {
          r.addedNodes.forEach(function (n) {
            if (n.tagName === 'INPUT' && n.type === 'password') attach(n);
            else scan(n);
          });
        });
      }).observe(doc.body, { childList: true, subtree: true });
    }
  }

  const api = { attach: attach, hideAll: hideAll, scan: scan, start: start };
  root.HosSecretField = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (root.document && root.document.addEventListener) {
    if (root.document.readyState === 'loading') {
      root.document.addEventListener('DOMContentLoaded', function () { start(root.document); });
    } else {
      start(root.document);
    }
  }
})(typeof window !== 'undefined' ? window : globalThis);
