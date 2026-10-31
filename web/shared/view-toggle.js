// The VIEW TOGGLE for the web (2026-10-30): the twin of the desktop app's
// src/gui/widgets/view_toggle.rs. One list, two ways to see it: a TABLE (one row each, its facts in
// columns, to compare at a glance) or CARDS (one card each, easier on the eye). Two joined buttons,
// each a small drawn picture and its word, the chosen one in the accent; the choice is kept on this
// device per list. The look is `.view-toggle` in web/shared/list-views.css; the words and hints are
// keys in data/i18n/en.json (common.view_table, common.view_cards and their _hint).
(function (root) {
  'use strict';

  /** The views, in the order the toggle draws them (the app's ListView::ALL). */
  const VIEWS = ['table', 'cards'];

  /** A stored choice made safe: one of VIEWS, else `fallback`. */
  function normalize(value, fallback) {
    return VIEWS.indexOf(value) >= 0 ? value : fallback;
  }

  function storageKey(id) {
    return 'hos_view_' + id;
  }

  function stored(id, fallback) {
    try {
      return normalize(root.localStorage && root.localStorage.getItem(storageKey(id)), fallback);
    } catch (e) {
      return fallback;
    }
  }

  function store(id, view) {
    try {
      if (root.localStorage) root.localStorage.setItem(storageKey(id), view);
    } catch (e) { /* private window: the choice lasts this visit */ }
  }

  function words(key) {
    return root.i18n && typeof root.i18n.t === 'function' ? root.i18n.t(key) : key;
  }

  /** Draw the toggle into `el` (emptied first) for the list `id`, `fallback` until a choice is
   *  kept; `onPick(view)` when the other view is chosen. Returns the view to draw now. */
  function render(el, id, fallback, onPick) {
    let view = stored(id, normalize(fallback, 'cards'));
    const doc = el.ownerDocument || root.document;
    el.textContent = '';
    el.classList.add('view-toggle');
    el.setAttribute('role', 'group');
    const buttons = [];
    for (const v of VIEWS) {
      const seg = doc.createElement('button');
      seg.type = 'button';
      seg.className = 'view-toggle-seg view-' + v;
      seg.dataset.view = v;
      const icon = doc.createElement('span');
      icon.className = 'view-toggle-icon';
      icon.setAttribute('aria-hidden', 'true');
      const word = doc.createElement('span');
      word.textContent = words('common.view_' + v);
      seg.appendChild(icon);
      seg.appendChild(word);
      if (root.i18n && typeof root.i18n.hint === 'function') root.i18n.hint(seg, 'common.view_' + v + '_hint');
      else seg.dataset.tipDesc = words('common.view_' + v + '_hint');
      seg.addEventListener('click', function () {
        if (view === v) return;
        view = v;
        store(id, v);
        for (const b of buttons) b.setAttribute('aria-pressed', b.dataset.view === view ? 'true' : 'false');
        if (onPick) onPick(v);
      });
      seg.setAttribute('aria-pressed', v === view ? 'true' : 'false');
      buttons.push(seg);
      el.appendChild(seg);
    }
    return view;
  }

  const api = { VIEWS, normalize, render };
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  } else {
    root.HosViewToggle = api;
  }
})(typeof window !== 'undefined' ? window : this);
