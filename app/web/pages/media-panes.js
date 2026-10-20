// ── media-panes.js ────────────────────────────────────────────────────────
// The Media page in three panes on the website (2026-10-19), the desktop's own layout mirrored:
// src/gui/pages/media/panes.rs (the page's split, its words and its view switch) and
// src/gui/widgets/panes.rs (panes that are cards, side by side or one above another, with a handle
// between each two that a drag moves). The operator, 2026-10-19: "a left side card that is the list
// of all the media and right side is like viewing the in-depth details of one media item. Maybe
// into thirds? Left side is the list of all of them, middle is the fit to area picture, 3D model,
// sound/music visualizer, then right side is data/details/info. We could do that top, middle bottom
// too."
//
// - WHICH WAY THEY GO follows the area's shape (`arrangeFor`, the desktop's arrange_for): side by
//   side when the page is at least as wide as the window is tall and every pane fits at its
//   narrowest (280 px), else one above another (each at least 170 px tall). A phone stacks them.
// - THE SPLIT is each pane's share of the area along the way they go, summing to one, kept in this
//   browser (localStorage `humanity_media_layout`, the desktop's config.json `media_layout` with its
//   field names: `gallery`, `across`, `down`), side by side and stacked apart. A drag on a handle
//   moves the boundary between its two panes, never past either one's narrowest (`drag`); a
//   double-click puts those two back; the arrow keys move it too. Storage that refuses (a private
//   window, blocked site data) leaves the defaults, and the page works the same.
// - The panes are a CSS grid (`template`): each pane `minmax(<narrowest>px, <share>fr)` with the
//   handle's track between, so a pane never goes below its narrowest however the window is sized;
//   as tall as the window has room for below their top (`fill`), as the desktop's fill the page.
// - THE LEFT PANE'S ROWS (`listRow`, the desktop's media/list.rs): each item its small picture,
//   title, by-line and a line of facts, the whole row one button, the item shown marked.
//
// A classic script in the browser (before media-item.js and media-app.js), a CommonJS module under
// Node, where scripts/tests/media-web-panes.test.js runs it. Every element is built with
// createElement and textContent; the styles are /shared/media.css, theme tokens only.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  /** Where the view and the split are kept in this browser. */
  const LAYOUT_KEY = 'humanity_media_layout';
  /** The narrowest a pane gets side by side, and the shortest stacked (the desktop's MIN_PANE, MIN_PANE_DOWN). */
  const MIN_PANE = 280;
  const MIN_PANE_DOWN = 170;
  /** The handle's track between two panes: var(--space-md), 10 px at the theme's default size. */
  const GAP = 10;
  /** The first split side by side (list, item, details) and stacked (the desktop's ACROSS, DOWN). */
  const ACROSS = Object.freeze([0.28, 0.42, 0.30]);
  const DOWN = Object.freeze([0.30, 0.36, 0.34]);
  /** The item window over the chat: the item and its details (the desktop's draw_window). */
  const WINDOW_SPLIT = Object.freeze([0.55, 0.45]);
  /** How far an arrow key moves a handle. */
  const KEY_STEP = 24;
  /** The shortest the panes get side by side (px), and the room kept under them (the site's footer button). */
  const MIN_HEIGHT = 480;
  const FOOT_ROOM = 12;

  const WORDS = Object.freeze({
    panes: 'Panes',
    panesTip: 'The list, the item and its details side by side (one above another on a tall window).',
    gallery: 'Gallery',
    galleryTip: 'Every item as a tile across the page. Press one to see it in the panes.',
    viewLabel: 'How the Media page is shown',
    choose: 'Choose something in the list to see it here.',
    details: 'Its details, licence and files show here, with Edit for your own.',
    handle: 'Drag to change the size of the panes beside it. Double-click to put them back.',
    handleLabel: 'Resize the panes',
  });

  /**
   * The way `count` panes go in an area `width` wide in a window `height` tall, none narrower than
   * `min` with `gap` between each two: 'across' when it is at least as wide as it is tall and they
   * all fit, else 'down' (the desktop's widgets/panes.rs arrange_for).
   */
  function arrangeFor(width, height, count, min, gap) {
    const n = Math.max(1, Number(count) || 1);
    const w = Number(width) || 0;
    const h = Number(height) || 0;
    const fits = w >= (Number(min) || 0) * n + (Number(gap) || 0) * (n - 1);
    return w >= h && fits ? 'across' : 'down';
  }

  /**
   * `shares` made usable: one per default, each above 0.01 and finite, summing to one; else the
   * defaults (a split written by hand, or by an older page with another number of panes, can hold
   * anything). The desktop's widgets/panes.rs checked.
   */
  function checked(shares, defaults) {
    const d = Array.from(defaults || []);
    const usable = Array.isArray(shares) && shares.length === d.length && shares.every((s) => typeof s === 'number' && Number.isFinite(s) && s > 0.01);
    const from = usable ? shares : d;
    const sum = from.reduce((a, b) => a + b, 0);
    if (!(sum > 0)) return d.map(() => 1 / Math.max(1, d.length));
    return from.map((s) => s / sum);
  }

  /**
   * The boundary after pane `i` moved by `delta` px in an area `total` px long (the panes' own
   * length, the handles' tracks left out): pane `i` grows by it and pane `i + 1` shrinks by it, never
   * either below `min`; the others keep theirs. A new array (the desktop's drag, which changes its own).
   */
  function drag(shares, i, delta, total, min) {
    const out = Array.from(shares || []);
    if (i < 0 || i + 1 >= out.length || !(total > 0)) return out;
    const both = (out[i] + out[i + 1]) * total;
    if (both < 2 * min) return out;
    const a = Math.min(Math.max(out[i] * total + (Number(delta) || 0), min), both - min);
    out[i] = a / total;
    out[i + 1] = (both - a) / total;
    return out;
  }

  /** Panes `i` and `i + 1` put back to their default proportion, keeping their sum. A new array. */
  function resetPair(shares, i, defaults) {
    const out = Array.from(shares || []);
    if (i < 0 || i + 1 >= out.length || i + 1 >= defaults.length) return out;
    const both = out[i] + out[i + 1];
    const want = defaults[i] + defaults[i + 1];
    if (!(want > 0)) return out;
    out[i] = both * defaults[i] / want;
    out[i + 1] = both * defaults[i + 1] / want;
    return out;
  }

  /** The grid's tracks for `shares`: each pane at least `min` px, then its share; a handle's track between. */
  function template(shares, min) {
    return shares.map((s) => 'minmax(' + min + 'px, ' + (Math.round(s * 10000) / 100) + 'fr)').join(' var(--space-md) ');
  }

  /** The default layout: the panes, each split as it first is. */
  function defaultLayout() {
    return { gallery: false, across: ACROSS.slice(), down: DOWN.slice() };
  }

  /** The view and split kept in `storage` (localStorage), checked; the defaults when none is kept, or storage refuses. */
  function readLayout(storage) {
    let raw = null;
    try { raw = storage ? storage.getItem(LAYOUT_KEY) : null; } catch (e) { raw = null; }
    let v = null;
    if (typeof raw === 'string' && raw) { try { v = JSON.parse(raw); } catch (e) { v = null; } }
    if (!v || typeof v !== 'object') return defaultLayout();
    return { gallery: v.gallery === true, across: checked(v.across, ACROSS), down: checked(v.down, DOWN) };
  }

  /** Keep `layout` in `storage`; storage that refuses leaves it for this visit only. */
  function saveLayout(storage, layout) {
    try {
      if (storage) storage.setItem(LAYOUT_KEY, JSON.stringify({ gallery: !!layout.gallery, across: layout.across, down: layout.down }));
      return true;
    } catch (e) {
      return false;
    }
  }

  /**
   * Panes in a grid, with the handles between them. `o`:
   *   h (the page's element maker), id, className (more classes), count, label (for its region),
   *   viewport() -> {width, height} (the area's width and the window's height: which way they go),
   *   shares(arrange) -> the split now, setShares(arrange, shares, settled) (kept by the caller;
   *   `settled` once a drag ends: the time to save it), defaults(arrange),
   *   measure() -> {width, height} of the grid when the element cannot say (the tests' stand-in),
   *   win (the window, for `fill`; optional).
   * Returns {el, arrange(), layout(), apply(), fill(), handle(i)}: `layout` decides the way they go
   * again (the page calls it on a resize), `fill` makes them as tall as the window has room for
   * (the page calls it when what is above them changes size), `handle(i)` makes the handle between
   * pane i and pane i + 1.
   */
  function createPanes(o) {
    const h = o.h;
    const count = o.count || 3;
    const el = h('div', { id: o.id || undefined, className: 'media-panes' + (o.className ? ' ' + o.className : '') });
    let way = 'across';
    const minOf = () => (way === 'across' ? MIN_PANE : MIN_PANE_DOWN);

    function apply() {
      const t = template(checked(o.shares(way), o.defaults(way)), minOf());
      if (way === 'across') {
        el.style.gridTemplateColumns = t;
        el.style.gridTemplateRows = '';
      } else {
        el.style.gridTemplateRows = t;
        el.style.gridTemplateColumns = 'minmax(0, 1fr)';
      }
      el.dataset.shares = checked(o.shares(way), o.defaults(way)).map((s) => s.toFixed(4)).join(',');
    }
    /**
     * As tall as the window has room for below the panes' own top, as the desktop's panes fill what
     * is left of the page, so the foot of each (View full size, Save and Cancel) is in view without
     * scrolling the page; never shorter than MIN_HEIGHT side by side, or than every pane at its
     * shortest stacked. Measured where the page is laid out (`win`), skipped while hidden.
     */
    function fill() {
      const win = o.win || null;
      if (!win || el.hidden || typeof el.getBoundingClientRect !== 'function') return;
      let r = null;
      try { r = el.getBoundingClientRect(); } catch (e) { r = null; }
      if (!r) return;
      const top = r.top + (Number(win.scrollY) || 0);
      const tall = Number(win.innerHeight) || 0;
      // Stacked (a phone, where the page's head takes much of the screen), they are at least most of
      // the window tall, so once scrolled to they fill it rather than sharing a strip.
      const least = way === 'across' ? MIN_HEIGHT : Math.max(count * MIN_PANE_DOWN + GAP * (count - 1), Math.round(tall * 0.85));
      el.style.height = Math.max(least, Math.floor(tall - top - FOOT_ROOM)) + 'px';
    }
    function layout() {
      const v = (typeof o.viewport === 'function' ? o.viewport() : null) || { width: 1280, height: 900 };
      const next = arrangeFor(v.width, v.height, count, MIN_PANE, GAP);
      const changed = next !== way || el.dataset.arrange !== next;
      way = next;
      el.dataset.arrange = way;
      el.className = 'media-panes media-panes-' + way + (o.className ? ' ' + o.className : '');
      if (changed) {
        for (const hd of el.querySelectorAll('[data-handle]')) hd.setAttribute('aria-orientation', way === 'across' ? 'vertical' : 'horizontal');
      }
      apply();
      fill();
      return changed;
    }
    /** The panes' own length along the way they go: the grid's, less the handles' tracks. */
    function lengthAlong() {
      let r = null;
      try { r = typeof el.getBoundingClientRect === 'function' ? el.getBoundingClientRect() : null; } catch (e) { r = null; }
      if (!r || !(r.width > 0)) r = typeof o.measure === 'function' ? o.measure() : null;
      if (!r) return 0;
      return Math.max(0, (way === 'across' ? r.width : r.height) - GAP * (count - 1));
    }
    function set(shares, settled) {
      o.setShares(way, shares, !!settled);
      apply();
    }

    /** The handle between pane `i` and pane `i + 1`: dragged, double-clicked, or moved with the arrow keys. */
    function handle(i) {
      const hd = h('div', {
        className: 'media-pane-handle', title: WORDS.handle, dataset: { handle: String(i) },
        attrs: { role: 'separator', tabindex: '0', 'aria-label': WORDS.handleLabel, 'aria-orientation': way === 'across' ? 'vertical' : 'horizontal' },
      }, [h('span', { className: 'media-pane-grip', attrs: { 'aria-hidden': 'true' } })]);
      let held = null; // {at, shares, len}
      const at = (e) => Number(way === 'across' ? e.clientX : e.clientY) || 0;
      const letGo = (e) => {
        if (!held) return;
        held = null;
        hd.className = 'media-pane-handle';
        try { if (e && e.pointerId != null && typeof hd.releasePointerCapture === 'function') hd.releasePointerCapture(e.pointerId); } catch (err) { /* already released */ }
        set(o.shares(way), true);
      };
      hd.addEventListener('pointerdown', (e) => {
        if (e.button != null && e.button !== 0) return;
        held = { at: at(e), shares: checked(o.shares(way), o.defaults(way)), len: lengthAlong() };
        hd.className = 'media-pane-handle media-pane-handle-held';
        try { if (e.pointerId != null && typeof hd.setPointerCapture === 'function') hd.setPointerCapture(e.pointerId); } catch (err) { /* follows the pointer while over it */ }
        if (typeof e.preventDefault === 'function') e.preventDefault();
      });
      hd.addEventListener('pointermove', (e) => {
        if (!held) return;
        set(drag(held.shares, i, at(e) - held.at, held.len, minOf()), false);
      });
      hd.addEventListener('pointerup', letGo);
      hd.addEventListener('pointercancel', letGo);
      hd.addEventListener('lostpointercapture', letGo);
      hd.addEventListener('dblclick', () => {
        held = null;
        set(resetPair(checked(o.shares(way), o.defaults(way)), i, o.defaults(way)), true);
      });
      hd.addEventListener('keydown', (e) => {
        const back = way === 'across' ? 'ArrowLeft' : 'ArrowUp';
        const on = way === 'across' ? 'ArrowRight' : 'ArrowDown';
        if (e.key !== back && e.key !== on) return;
        if (typeof e.preventDefault === 'function') e.preventDefault();
        set(drag(checked(o.shares(way), o.defaults(way)), i, e.key === on ? KEY_STEP : -KEY_STEP, lengthAlong(), minOf()), true);
      });
      return hd;
    }

    layout();
    return { el, arrange: () => way, layout, apply, fill, handle };
  }

  // ── The list pane's rows (the desktop's media/list.rs) ──

  /**
   * One item as a row of the list pane: its small picture, its title, "by Ann in Fire flow arts", and
   * a line with its kind, picks and downloads, size and badges (its labels last, in the warning
   * colour). The whole row is one button: pressed (`c.open`), the item shows in the middle and on the
   * right. It has no buttons of its own (they are on the right; the Gallery's tiles keep theirs).
   * `where`: 'featured' (the shelf) or 'list'. `c`: h, st (the page's state: viewer, kinds, labels,
   * revealed), lib {kindWords, labelsOf, itemName, downloadName, sizeWords, num, isFeatured}, MI
   * (media-item.js: coverOf, byLine, plainName), FR (file-risk.js, or null), keptBadge (the words),
   * hiddenLine(item) (the protected setup's), pictureSource(pic) and open(item).
   */
  function listRow(c, item, where) {
    const { h, st, lib, MI } = c;
    // A file item by its type, once the file-type list is in; any other by its kind's word.
    const kind = item.kind === 'file' && c.FR && typeof c.FR.ready === 'function' && c.FR.ready()
      ? c.FR.fileType(MI.plainName(lib.downloadName(item, st.kinds)), null).label
      : lib.kindWords(item.kind, st.kinds).one;
    // The small picture under the tiles' rules (the protected setup hides it; a labelled item shows
    // its kind's word until it is shown in the middle pane; a members-only picture through its
    // object URL), else the kind's word.
    const mark = () => h('span', { className: 'media-row-thumb media-row-mark media-mark-' + lib.kindWords(item.kind, st.kinds).mark, attrs: { 'aria-hidden': 'true' } }, [
      h('span', { className: 'media-mark-word', text: kind }),
    ]);
    let thumb = null;
    const pic = MI.coverOf(item) || (item.kind === 'picture' ? item : null);
    const veiled = lib.labelsOf(item, st.labels).length > 0 && !st.revealed.has(item.id);
    if (pic && c.hiddenLine(item) === null && !veiled) {
      const at = c.pictureSource(pic);
      if (at.src) thumb = h('span', { className: 'media-row-thumb', attrs: { 'aria-hidden': 'true' } }, [h('img', { className: 'media-row-img', src: at.src, alt: '', loading: 'lazy' })]);
    }
    const chosen = !!st.viewer && st.viewer.id === item.id;
    const picks = Number(item.picks) || 0;
    const downloads = Number(item.downloads) || 0;
    const facts = [
      kind,
      picks + (picks === 1 ? ' pick' : ' picks') + ', ' + downloads + (downloads === 1 ? ' download' : ' downloads'),
      lib.sizeWords(Math.max(lib.num(item.total_bytes), lib.num(item.size_bytes))),
    ];
    if (lib.isFeatured(item)) facts.push(item.featured === 'main' ? 'Featured' : 'Featured in its category');
    if (item.kept) facts.push(c.keptBadge);
    if (item.visibility === 'members') facts.push('Members only');
    const labels = lib.labelsOf(item, st.labels).map((l) => l.label);
    const title = lib.itemName(item, st.kinds);
    // The site's hover hint (/shared/shell.js) names it in full (a long title is cut short in the
    // row) with its by-line, rather than guessing a description from the button's words.
    return h('button', {
      type: 'button', className: 'media-row' + (chosen ? ' media-row-chosen' : ''), dataset: { id: item.id, row: where, tipTitle: title, tipDesc: MI.byLine(item) },
      attrs: { 'aria-current': chosen ? 'true' : 'false' }, on: { click: () => c.open(item) },
    }, [
      thumb || mark(),
      h('span', { className: 'media-row-text' }, [
        h('span', { className: 'media-row-title', text: title }),
        h('span', { className: 'media-row-by', text: MI.byLine(item) }),
        h('span', { className: 'media-row-facts' + (labels.length ? ' media-row-labelled' : ''), text: facts.concat(labels).join(', ') }),
      ]),
    ]);
  }

  /** The rows of `root` marked for the item shown now (`id`, or null for none), without drawing them again. */
  function markChosen(root, id) {
    for (const r of Array.from(root && typeof root.querySelectorAll === 'function' ? root.querySelectorAll('.media-row') : [])) {
      const chosen = id != null && String(id) === String(r.dataset.id);
      r.className = 'media-row' + (chosen ? ' media-row-chosen' : '');
      r.setAttribute('aria-current', chosen ? 'true' : 'false');
    }
  }

  const api = {
    LAYOUT_KEY, MIN_PANE, MIN_PANE_DOWN, GAP, ACROSS, DOWN, WINDOW_SPLIT, KEY_STEP, WORDS,
    arrangeFor, checked, drag, resetPair, template, defaultLayout, readLayout, saveLayout, createPanes, listRow, markChosen,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.MediaPanes = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
