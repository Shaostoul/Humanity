/* Tools page (web): the catalog of EXTERNAL things as a library, the web mirror of the desktop
   app's Tools page (src/gui/pages/tools.rs with tools_library.rs and tools_detail.rs, drawn on
   the shelf in src/gui/widgets/shelf.rs; v0.1520.0). Both read the SAME
   data/external/catalog.json, which holds two kinds:
     - "software": free programs you download and run yourself
     - "service" : real-world help websites and organizations
   Add entries to the data file, never here (Infinite-of-X).

   What it has, as the app does: Grid and List; search (every word, in the name, the category or
   the description); sort by Name or Category (the Grid sorted by category shows each category
   under its name); the kind pills (Everything, Software, Help and services, each with its count)
   and the category pills of the kind chosen; a plain tile per entry in its category's colour with
   its name, category, licence, platforms, size and main button (Download or Open website); and
   an entry's details at the right of the shelf (below it on a phone). The view, the sort and both
   filters are kept in this browser (localStorage `hos_tools_view`, the app's config.json
   `tools_view`), read and written inside try/catch: with storage refused the page still works.

   What it does not have, and says so on the page: a web page cannot see what is installed on the
   computer, start a program or install one, so there is no Launch, no Install, no "Installed
   first" sort and no Installed column. The desktop app does those (the dated finding,
   docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md, which the page's
   Why opens in the Library).

   No logo is shown or fetched: each maker sets its own rules for its logo (the finding's
   judgement call 6), so every tile is the plain one, the panel the app draws for a program it has
   not found on the computer.

   Built with createElement and textContent only, never innerHTML, so the catalog's words are
   always written as text. Required by Node, where scripts/tests/tools-web.test.js runs it. */
(function (root) {
  'use strict';

  const DATA_URL = '/data/external/catalog.json';
  /** Where the view is kept in this browser. */
  const VIEW_KEY = 'hos_tools_view';
  const DETAILS_ID = 'tools-details';
  const DETAILS_TITLE_ID = 'tools-details-title';
  /** Below this width the details sit under the shelf (the page's CSS uses the same width). */
  const NARROW = '(max-width: 900px)';

  /** The words on the page. The desktop app's (src/gui/pages/tools.rs and its parts) unless
      marked "web": those say what only a web page has to say. */
  const WORDS = {
    searchHint: 'Search tools and services',
    sort: 'Sort',
    show: 'Show',
    everything: 'Everything',
    all: 'All',
    download: 'Download',
    openWebsite: 'Open website',
    website: 'Website',
    downloadPage: 'Download page',
    close: 'Close',
    about: 'About',
    runsInBrowser: 'Runs in your web browser',
    comesFrom: "It comes from its own project: Download page and Website open the project's own pages in your browser.",
    // web: the app's "No Install here." names the button it leaves out; a web page has none.
    noInstall: 'The desktop app has no Install for it. ',
    // web: the app's NOT_IN_WINGET, said of the desktop app.
    notInWinget: "The desktop app has no Install for it: it is not in winget, Windows' own package manager, so Download opens its maker's own page.",
    nothingMatches: 'Nothing here matches the filters.',
    loading: 'Loading the catalog...',
    loadFailed: 'Could not load the catalog. Reload the page, or report it on the Bugs page.',
    tipDownload: "Opens the project's own download page in your browser.",
    tipWebsite: 'Opens its website in your browser.',
    tipClose: 'Close its details.',
    tipSortBy: 'Sort by this',
  };

  /** The orders the shelf is shown in (the app's SortBy, less "Installed first"). */
  const SORTS = [
    { id: 'name', words: 'Name', tip: 'A to Z.' },
    { id: 'category', words: 'Category', tip: 'By category, each under its name.' },
  ];
  /** Grid or List (the app's shelf::Layout), with their hover words. */
  const LAYOUTS = [
    { id: 'grid', words: 'Grid', tip: 'A tile for each.' },
    { id: 'list', words: 'List', tip: 'A row for each, its facts in columns.' },
  ];

  // ── The catalog ───────────────────────────────────────────────────────

  /** The catalog's entries in one flat list, each tagged with its category and kind (the app's
      software::catalog::parse). `i` is its place in the list. */
  function entriesOf(catalog) {
    const out = [];
    const cats = (catalog && Array.isArray(catalog.categories)) ? catalog.categories : [];
    for (const cat of cats) {
      const name = String(cat.name || '');
      const id = cat.id ? String(cat.id) : name.toLowerCase();
      const extensions = Array.isArray(cat.extensions) ? cat.extensions.map(String) : [];
      for (const e of (Array.isArray(cat.entries) ? cat.entries : [])) {
        out.push({
          i: out.length,
          name: String(e.name || ''),
          description: String(e.description || ''),
          url: String(e.url || ''),
          license: String(e.license || ''),
          platforms: Array.isArray(e.platforms) ? e.platforms.map(String) : [],
          size: String(e.size || ''),
          download: e.download ? String(e.download) : null,
          winget: e.winget ? String(e.winget) : null,
          websiteOnly: e.website_only ? String(e.website_only) : null,
          category: name,
          categoryId: id,
          extensions,
          kind: String(cat.kind || ''),
        });
      }
    }
    return out;
  }

  const isSoftware = (e) => e.kind === 'software';
  /** Runs only in a web browser: nothing to download. */
  const webOnly = (e) => e.platforms.length > 0 && e.platforms.every((p) => p === 'web');
  /** The page its Download opens: its download page, else its website. */
  const downloadPage = (e) => e.download || e.url;
  /** Only a web address is ever made a link (the app's open_web). */
  const safeUrl = (u) => (/^https?:\/\//i.test(String(u || '')) ? String(u) : null);

  /** Its website's host ("www.211.org" is "211.org"). */
  function hostOf(e) {
    const parts = e.url.split('://');
    const rest = parts.length > 1 ? parts[1] : e.url;
    const host = rest.split(/[/?#]/)[0];
    return host.indexOf('www.') === 0 ? host.slice(4) : host;
  }

  /** A kind id's words. */
  function kindLabel(kind) {
    if (kind === 'software') return 'Software';
    if (kind === 'service') return 'Help and services';
    return kind;
  }

  function platformWords(p) {
    return ({ windows: 'Windows', mac: 'Mac', linux: 'Linux', web: 'Web browser' })[p] || p;
  }
  const platformsWords = (e) => e.platforms.map(platformWords).join(', ');

  /** The line under a tile's name: its category, and its licence when it has one. */
  const metaLine = (e) => (e.license ? e.category + ' · ' + e.license : e.category);

  /** The tile's third line: a help service's website, "Runs in your web browser", or where it
      runs and its size. */
  function thirdLine(e) {
    if (!isSoftware(e)) return hostOf(e);
    if (webOnly(e)) return WORDS.runsInBrowser;
    return platformsWords(e) + (e.size ? ' · ' + e.size : '');
  }

  /** The main button: Download for a program to download, else Open website. */
  function mainLink(e) {
    if (isSoftware(e) && !webOnly(e)) return { words: WORDS.download, url: downloadPage(e), tip: WORDS.tipDownload };
    return { words: WORDS.openWebsite, url: e.url, tip: WORDS.tipWebsite };
  }

  /** The kinds, in the catalog's order. */
  function kindsOf(entries) {
    const out = [];
    for (const e of entries) if (e.kind && out.indexOf(e.kind) < 0) out.push(e.kind);
    return out;
  }

  /** The categories of `kind` (null: every kind), in the catalog's order, as { id, name }. */
  function categoriesOf(entries, kind) {
    const out = [];
    for (const e of entries) {
      if (kind != null && e.kind !== kind) continue;
      if (!out.some((c) => c.id === e.categoryId)) out.push({ id: e.categoryId, name: e.category });
    }
    return out;
  }

  const cmp = (a, b) => (a < b ? -1 : (a > b ? 1 : 0));

  /** The shelf's order (the app's order_of): the entries narrowed to the view's kind and
      category and to those whose name, category or description hold every word of `search`
      (letter case ignored), sorted by name (then category, then catalog place), or by category
      and then by name. Strings compared as plain strings, as the app compares them. */
  function orderOf(entries, view, search) {
    const words = String(search || '').split(/\s+/).filter(Boolean).map((w) => w.toLowerCase());
    const out = entries
      .filter((e) => view.kind == null || e.kind === view.kind)
      .filter((e) => view.category == null || e.categoryId === view.category)
      .filter((e) => {
        const text = (e.name + '\n' + e.category + '\n' + e.description).toLowerCase();
        return words.every((w) => text.indexOf(w) >= 0);
      })
      .map((e) => e.i);
    const byName = (a, b) => cmp(entries[a].name.toLowerCase(), entries[b].name.toLowerCase())
      || cmp(entries[a].category, entries[b].category) || a - b;
    if (view.sort === 'category') {
      out.sort((a, b) => cmp(entries[a].category.toLowerCase(), entries[b].category.toLowerCase()) || byName(a, b));
    } else {
      out.sort(byName);
    }
    return out;
  }

  /** `order` cut into runs of one category each, for the Grid sorted by category. */
  function groupsOf(entries, order) {
    const out = [];
    for (const i of order) {
      const last = out[out.length - 1];
      if (last && last.categoryId === entries[i].categoryId) last.items.push(i);
      else out.push({ categoryId: entries[i].categoryId, category: entries[i].category, items: [i] });
    }
    return out;
  }

  /** What an entry's details show (the app's tools_detail.rs, less what only the app can know):
      its links, why the desktop app has no Install for it (where the catalog says, or where
      winget has no package), its description, its facts, and where a program comes from. */
  function detailsOf(e) {
    const sw = isSoftware(e);
    const links = [{ words: sw ? WORDS.website : WORDS.openWebsite, url: e.url, tip: WORDS.tipWebsite }];
    if (sw && !webOnly(e) && downloadPage(e) !== e.url) {
      links.push({ words: WORDS.downloadPage, url: downloadPage(e), tip: WORDS.tipDownload });
    }
    let note = null;
    if (e.websiteOnly) note = WORDS.noInstall + e.websiteOnly;
    else if (sw && !webOnly(e) && !e.winget) note = WORDS.notInWinget;
    const rows = [];
    if (e.license) rows.push(['Licence', e.license]);
    if (e.platforms.length) rows.push(['Runs on', platformsWords(e)]);
    if (e.size && !webOnly(e)) rows.push(['Download size', e.size]);
    if (sw && e.extensions.length) rows.push(['Opens', e.extensions.join(', ')]);
    rows.push(['Website', hostOf(e)]);
    return {
      name: e.name,
      category: e.category,
      links: links.filter((l) => safeUrl(l.url)),
      note,
      description: e.description,
      rows,
      comesFrom: sw && !webOnly(e) ? WORDS.comesFrom : null,
    };
  }

  /** A category's hue, 0 to 359: the FNV-1a hash of its id, as the app's widgets::swatch_color
      picks it, so a category is the same colour on both. */
  function hueOf(seed) {
    const s = String(seed || '');
    const bytes = typeof TextEncoder !== 'undefined'
      ? new TextEncoder().encode(s)
      : unescape(encodeURIComponent(s)).split('').map((c) => c.charCodeAt(0));
    let h = 2166136261;
    for (const b of bytes) {
      h ^= b;
      h = Math.imul(h, 16777619) >>> 0;
    }
    return (h >>> 0) % 360;
  }

  /** The first letter or digit of a name, in capitals, for a plain cover. */
  function monogram(name) {
    const m = String(name || '').match(/[\p{L}\p{N}]/u);
    return m ? m[0].toUpperCase() : '';
  }

  // ── The view, kept in this browser ────────────────────────────────────

  /** A view with every field known: Grid by name, every kind and category, unless it says. */
  function normalView(v) {
    const o = v && typeof v === 'object' ? v : {};
    return {
      layout: LAYOUTS.some((l) => l.id === o.layout) ? o.layout : 'grid',
      sort: SORTS.some((s) => s.id === o.sort) ? o.sort : 'name',
      kind: typeof o.kind === 'string' && o.kind ? o.kind : null,
      category: typeof o.category === 'string' && o.category ? o.category : null,
    };
  }

  /** The view kept in `storage`, or the default when there is none, it does not read, or the
      storage is refused. */
  function readView(storage) {
    let raw = null;
    try { raw = storage ? storage.getItem(VIEW_KEY) : null; } catch (e) { raw = null; }
    let parsed = null;
    try { parsed = raw ? JSON.parse(raw) : null; } catch (e) { parsed = null; }
    return normalView(parsed);
  }

  /** Keep `view` in `storage`. False when the storage refused it (the page carries on). */
  function writeView(storage, view) {
    try {
      if (!storage) return false;
      const v = normalView(view);
      storage.setItem(VIEW_KEY, JSON.stringify({ layout: v.layout, sort: v.sort, kind: v.kind, category: v.category }));
      return true;
    } catch (e) {
      return false;
    }
  }

  /** `view` with a kind or a category the catalog no longer has (or a category of another kind)
      let go, so a kept filter never leaves the shelf empty for no reason the page shows. */
  function fitView(view, entries) {
    const v = normalView(view);
    const kind = v.kind != null && entries.some((e) => e.kind === v.kind) ? v.kind : null;
    const category = v.category != null && entries.some((e) => e.categoryId === v.category && (kind == null || e.kind === kind)) ? v.category : null;
    return { layout: v.layout, sort: v.sort, kind, category };
  }

  // ── The page ──────────────────────────────────────────────────────────

  /**
   * The page, drawn into `opts.root` (#tools-app): the toolbar, the kind and category pills and
   * the shelf, with the details beside them. `opts`: document, root, fetch, storage (null when
   * refused), window (for the phone-width check; optional).
   */
  function createToolsPage(opts) {
    const doc = opts.document;
    const host = opts.root;
    const storage = opts.storage || null;
    const win = opts.window || null;
    const st = { entries: null, failed: false, view: readView(storage), search: '', selected: null };

    function el(tag, attrs, kids) {
      const n = doc.createElement(tag);
      if (attrs) {
        for (const k of Object.keys(attrs)) {
          const v = attrs[k];
          if (v == null || v === false) continue;
          if (k === 'class') n.className = v;
          else if (k === 'text') n.textContent = v;
          else if (k === 'hidden') n.hidden = true;
          else n.setAttribute(k, v === true ? '' : String(v));
        }
      }
      for (const c of (kids || [])) {
        if (c != null) n.appendChild(typeof c === 'string' ? doc.createTextNode(c) : c);
      }
      return n;
    }

    function setHue(node, categoryId) {
      if (node.style && typeof node.style.setProperty === 'function') node.style.setProperty('--tool-hue', String(hueOf(categoryId)));
    }

    // The parts that stay for the page's life (the search field keeps its focus and its words).
    const search = el('input', {
      type: 'search', id: 'tools-search', class: 'tools-search', placeholder: WORDS.searchHint,
      'aria-label': WORDS.searchHint, autocomplete: 'off', spellcheck: 'false',
    });
    const sortGroup = el('div', { class: 'tools-pills', role: 'group', 'aria-labelledby': 'tools-sort-label' });
    const layoutGroup = el('div', { class: 'tools-pills', role: 'group', 'aria-labelledby': 'tools-show-label' });
    const toolbar = el('div', { class: 'tools-toolbar' }, [
      search,
      el('span', { class: 'tools-label', id: 'tools-sort-label', text: WORDS.sort }), sortGroup,
      el('span', { class: 'tools-label', id: 'tools-show-label', text: WORDS.show }), layoutGroup,
    ]);
    const kindsRow = el('div', { class: 'tools-pills tools-row', id: 'tools-kinds', role: 'group', 'aria-label': 'Kind' });
    const catsRow = el('div', { class: 'tools-pills tools-row', id: 'tools-cats', role: 'group', 'aria-label': 'Category' });
    const status = el('p', { class: 'sr-only', id: 'tools-status', role: 'status' });
    const shelf = el('div', { class: 'tools-shelf', id: 'tools-shelf' });
    const main = el('div', { class: 'tools-main' }, [toolbar, kindsRow, catsRow, status, shelf]);
    const details = el('aside', { class: 'tools-details', id: DETAILS_ID, 'aria-labelledby': DETAILS_TITLE_ID, tabindex: '-1', hidden: true });

    // ── Pills: rebuilt only when the choices change, so a pressed pill keeps its focus.
    const pillKeys = new Map();
    function pills(group, options, chosen, onPick) {
      const key = options.map((o) => o.words + '\u0001' + (o.tip || '')).join('\u0002');
      if (pillKeys.get(group) !== key) {
        pillKeys.set(group, key);
        group.replaceChildren();
        options.forEach((o, n) => {
          const b = el('button', { type: 'button', class: 'tools-pill', 'data-pick': n, title: o.tip || null, text: o.words });
          b.addEventListener('click', () => onPick(n));
          group.appendChild(b);
        });
      }
      // An HTMLCollection in a browser, which has no forEach of its own.
      Array.prototype.forEach.call(group.children, (b, n) => b.setAttribute('aria-pressed', n === chosen ? 'true' : 'false'));
    }

    function setView(patch) {
      st.view = Object.assign({}, st.view, patch);
      writeView(storage, st.view);
      render();
    }

    function renderToolbar() {
      const v = st.view;
      pills(sortGroup, SORTS, Math.max(0, SORTS.findIndex((s) => s.id === v.sort)), (n) => setView({ sort: SORTS[n].id }));
      pills(layoutGroup, LAYOUTS, Math.max(0, LAYOUTS.findIndex((l) => l.id === v.layout)), (n) => setView({ layout: LAYOUTS[n].id }));
      const entries = st.entries || [];
      const kinds = kindsOf(entries);
      const kindOptions = [{ id: null, words: WORDS.everything + ' ' + entries.length }]
        .concat(kinds.map((k) => ({ id: k, words: kindLabel(k) + ' ' + entries.filter((e) => e.kind === k).length })));
      // Categories belong to a kind, so a kind chosen lets go of the category.
      pills(kindsRow, kindOptions, Math.max(0, kindOptions.findIndex((o) => o.id === v.kind)), (n) => setView({ kind: kindOptions[n].id, category: null }));
      const cats = categoriesOf(entries, v.kind);
      const catOptions = [{ id: null, words: WORDS.all }].concat(cats.map((c) => ({ id: c.id, words: c.name })));
      pills(catsRow, catOptions, Math.max(0, catOptions.findIndex((o) => o.id === v.category)), (n) => setView({ category: catOptions[n].id }));
      kindsRow.hidden = !st.entries;
      catsRow.hidden = !st.entries;
    }

    function cover(e, cls) {
      const c = el('span', { class: cls, 'aria-hidden': 'true' }, [
        el('span', { class: 'tool-cover-letter', text: monogram(e.name) }),
        el('span', { class: 'tool-cover-band', text: e.name }),
      ]);
      setHue(c, e.categoryId);
      return c;
    }

    function dot(e) {
      const d = el('span', { class: 'tool-dot', 'aria-hidden': 'true' });
      setHue(d, e.categoryId);
      return d;
    }

    /** The main button: a link that opens the maker's page in a new tab. */
    function mainButton(e, full) {
      const m = mainLink(e);
      const url = safeUrl(m.url);
      if (!url) return null;
      return el('a', {
        class: 'tool-go' + (full ? ' full' : ''), href: url, target: '_blank', rel: 'noopener noreferrer',
        title: m.tip, 'aria-label': m.words + ' ' + e.name, 'data-go': e.i, text: m.words,
      });
    }

    /** A tile: its cover, name, category and licence, one more line, all one button that opens
        its details; and its main button under them. */
    function tile(e) {
      const chosen = st.selected === e.i;
      // Named in so many words (its name first, as it is seen): the cover inside is only a picture.
      const open = el('button', {
        type: 'button', class: 'tool-open', 'data-open': e.i, 'aria-controls': DETAILS_ID, 'aria-expanded': chosen ? 'true' : 'false',
        'aria-label': e.name + ', ' + metaLine(e) + ', ' + thirdLine(e),
      }, [
        cover(e, 'tool-cover'),
        el('span', { class: 'tool-name', text: e.name }),
        el('span', { class: 'tool-meta', text: metaLine(e) }),
        el('span', { class: 'tool-line', text: thirdLine(e) }),
      ]);
      open.addEventListener('click', () => select(e.i));
      return el('article', { class: 'tool-tile' + (chosen ? ' selected' : ''), 'data-i': e.i }, [open, mainButton(e, true)]);
    }

    function grid(items) {
      return el('div', { class: 'tools-grid' }, items.map((i) => tile(st.entries[i])));
    }

    /** The List: a table, its Name and Category headings sorting by themselves; the columns give
        way on a narrow page (Size first, then Runs on, then Licence, Category last), in the CSS. */
    function list(order) {
      const v = st.view;
      const head = (words, cls, sortId) => {
        if (!sortId) return el('th', { scope: 'col', class: cls, text: words });
        const on = v.sort === sortId;
        const b = el('button', { type: 'button', class: 'tools-sort' + (on ? ' on' : ''), 'data-sort': sortId, title: WORDS.tipSortBy, text: words });
        b.addEventListener('click', () => setView({ sort: sortId }));
        return el('th', { scope: 'col', class: cls, 'aria-sort': on ? 'ascending' : null }, [b]);
      };
      const thead = el('thead', null, [el('tr', null, [
        el('th', { scope: 'col', class: 'col-act' }, [el('span', { class: 'sr-only', text: 'Link' })]),
        head('Name', 'col-name', 'name'),
        head('Category', 'col-cat', 'category'),
        head('Licence', 'col-lic'),
        head('Runs on', 'col-plat'),
        head('Size', 'col-size'),
      ])]);
      const rows = order.map((i) => {
        const e = st.entries[i];
        const chosen = st.selected === i;
        const name = el('button', {
          type: 'button', class: 'tool-row-name', 'data-open': i, 'aria-controls': DETAILS_ID, 'aria-expanded': chosen ? 'true' : 'false', text: e.name,
        });
        name.addEventListener('click', () => select(i));
        return el('tr', { class: chosen ? 'selected' : null, 'data-i': i }, [
          el('td', { class: 'col-act' }, [mainButton(e, false)]),
          el('td', { class: 'col-name' }, [name]),
          el('td', { class: 'col-cat', text: e.category }),
          el('td', { class: 'col-lic', text: e.license }),
          el('td', { class: 'col-plat', text: platformsWords(e) }),
          el('td', { class: 'col-size', text: webOnly(e) ? '' : e.size }),
        ]);
      });
      return el('div', { class: 'tools-table-wrap' }, [
        el('table', { class: 'tools-table' }, [el('caption', { class: 'sr-only', text: 'Tools and services' }), thead, el('tbody', null, rows)]),
      ]);
    }

    function renderShelf() {
      if (st.failed) {
        shelf.replaceChildren(el('p', { class: 'tools-empty', text: WORDS.loadFailed }));
        status.textContent = WORDS.loadFailed;
        return;
      }
      if (!st.entries) {
        shelf.replaceChildren(el('p', { class: 'tools-empty', text: WORDS.loading }));
        return;
      }
      const order = orderOf(st.entries, st.view, st.search);
      status.textContent = 'Showing ' + order.length + ' of ' + st.entries.length + '.';
      if (!order.length) {
        const q = st.search.trim();
        shelf.replaceChildren(el('p', { class: 'tools-empty', text: q ? 'Nothing here matches "' + q + '".' : WORDS.nothingMatches }));
        return;
      }
      if (st.view.layout === 'list') {
        shelf.replaceChildren(list(order));
      } else if (st.view.sort === 'category') {
        // Each category under its name, with how many.
        shelf.replaceChildren(...groupsOf(st.entries, order).map((g) => el('section', { class: 'tools-group-wrap' }, [
          el('h2', { class: 'tools-group' }, [dot(st.entries[g.items[0]]), el('span', { text: g.category }), el('span', { class: 'tools-group-count', text: String(g.items.length) })]),
          grid(g.items),
        ])));
      } else {
        shelf.replaceChildren(grid(order));
      }
    }

    function renderDetails() {
      const e = st.selected != null && st.entries ? st.entries[st.selected] : null;
      if (host.classList) host.classList.toggle('has-details', !!e);
      details.replaceChildren();
      details.hidden = !e;
      if (!e) return;
      const d = detailsOf(e);
      const banner = el('div', { class: 'tool-banner' }, [
        el('span', { class: 'tool-cover-letter', 'aria-hidden': 'true', text: monogram(e.name) }),
        el('h2', { class: 'tool-cover-band', id: DETAILS_TITLE_ID, text: d.name }),
      ]);
      setHue(banner, e.categoryId);
      const closeBtn = el('button', { type: 'button', class: 'tool-close', title: WORDS.tipClose, 'aria-label': WORDS.close + ' ' + d.name, text: WORDS.close });
      closeBtn.addEventListener('click', close);
      const links = el('div', { class: 'tool-links' }, d.links.map((l) => el('a', {
        class: 'tool-btn', href: safeUrl(l.url), target: '_blank', rel: 'noopener noreferrer', title: l.tip, 'data-link': l.words, text: l.words,
      })));
      const facts = el('dl', { class: 'tool-facts' });
      for (const [label, value] of d.rows) facts.append(el('dt', { text: label }), el('dd', { text: value }));
      // Only nodes: a browser's append() writes a null as the word "null".
      details.append(...[
        banner,
        el('div', { class: 'tool-detail-cat' }, [dot(e), el('span', { class: 'tool-detail-cat-name', text: d.category }), closeBtn]),
        links,
        d.note ? el('p', { class: 'tool-note', text: d.note }) : null,
        el('p', { class: 'tool-desc', text: d.description }),
        el('h3', { class: 'tool-section', text: WORDS.about }),
        facts,
        d.comesFrom ? el('p', { class: 'tool-comes-from', text: d.comesFrom }) : null,
      ].filter(Boolean));
    }

    function render() {
      renderToolbar();
      renderShelf();
      renderDetails();
    }

    function isNarrow() {
      try { return !!(win && win.matchMedia && win.matchMedia(NARROW).matches); } catch (e) { return false; }
    }

    /** Open entry `i`'s details, and move to them (on a phone they are under the shelf). */
    function select(i) {
      if (!st.entries || !st.entries[i]) return;
      st.selected = i;
      renderShelf();
      renderDetails();
      try { details.focus({ preventScroll: true }); } catch (e) { /* an old browser: no focus options */ }
      if (isNarrow() && typeof details.scrollIntoView === 'function') details.scrollIntoView({ block: 'start' });
    }

    /** Close the details, and give the focus back to what opened them (the search field when a
        filter has since hidden it). */
    function close() {
      if (st.selected == null) return;
      const i = st.selected;
      st.selected = null;
      renderShelf();
      renderDetails();
      const back = shelf.querySelector('[data-open="' + i + '"]');
      if (back) back.focus(); else search.focus();
    }

    function onKey(ev) {
      if (ev.key === 'Escape' && st.selected != null) {
        if (ev.preventDefault) ev.preventDefault();
        close();
      }
    }

    function start() {
      host.replaceChildren(main, details);
      search.addEventListener('input', () => { st.search = search.value; renderShelf(); });
      doc.addEventListener('keydown', onKey);
      render();
      return Promise.resolve()
        .then(() => opts.fetch(DATA_URL, { cache: 'no-cache' }))
        .then((resp) => {
          if (!resp || !resp.ok) throw new Error('HTTP ' + (resp && resp.status));
          return resp.json();
        })
        .then((json) => {
          st.entries = entriesOf(json);
          st.view = fitView(st.view, st.entries);
          render();
        })
        .catch((err) => {
          if (root.console) root.console.error('tools: could not load ' + DATA_URL, err);
          st.failed = true;
          render();
        });
    }

    return { start, select, close, setView, state: st, parts: { search, shelf, details, sortGroup, layoutGroup, kindsRow, catsRow, status } };
  }

  // ── In the browser ──
  function bootInBrowser() {
    const doc = root.document;
    if (!doc) return;
    const go = () => {
      const host = doc.getElementById('tools-app');
      if (!host) return;
      let storage = null;
      try { storage = root.localStorage; } catch (e) { storage = null; }
      createToolsPage({ document: doc, root: host, fetch: (u, i) => root.fetch(u, i), storage, window: root }).start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    DATA_URL, VIEW_KEY, WORDS, SORTS, LAYOUTS,
    entriesOf, isSoftware, webOnly, downloadPage, safeUrl, hostOf, kindLabel, platformWords, platformsWords,
    metaLine, thirdLine, mainLink, kindsOf, categoriesOf, orderOf, groupsOf, detailsOf, hueOf, monogram,
    normalView, readView, writeView, fitView, createToolsPage,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    root.HosTools = api;
    bootInBrowser();
  }
})(typeof window !== 'undefined' ? window : globalThis);
