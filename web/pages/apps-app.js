/* Apps page (web): the web mirror of the desktop app's Apps page (src/gui/pages/apps.rs, its
   toolbar and parts in apps/library.rs, its details in apps/detail.rs and apps/entry_detail.rs;
   2026-10-19, the Games library and the Tools page made one). The website's Tools page became
   this page on 2026-10-20, under the Apps name everywhere: its files, its address and the view
   it keeps (there are no users yet, so nothing points at the old ones).

   SIX PARTS, in the app's order, each under a band in its colour AND its name (a colour is never
   the only way to tell them apart), the colours the theme's (web/shared/theme.css, generated from
   data/gui/theme.ron):
     - RED, Games: a web page cannot see a computer's games, so one line says the desktop app
       lists and starts them, with a link to /download.
     - GREEN, HumanityOS tools: data/gui/humanityos_tools.json, the file the app reads, each
       opening the website's page for that tool (TOOL_PAGES below); a tool the website has no page
       for (Quests, Studio) is shown too, with "In the desktop app" where Open would be.
     - BLUE, Other programs, CYAN, Help, and MAGENTA, Services: data/external/catalog.json, the
       file the app reads; its "software" in Other programs, its "service" split by each
       category's "section" ("help" or "services"), as the app splits them.
     - YELLOW, Added by you: desktop only; one line says so.

   One toolbar as the app's: search (every word, across the parts), Sort (Name, and Category when
   a catalog part is shown: the app's sorts that mean something on a web page), Grid or List, and
   a chip per part with its count (the parts the website cannot list carry their name alone).
   Tiles as short as the app's: a name drawn once, on the plain cover; one line under it; a small
   button with one fact at its right. Clicking a cover opens its details at the right (under the
   parts on a phone). The view (Grid or List, the sort, the part chosen and each catalog part's
   category) is kept in this browser (localStorage `hos_apps_view`, the app's config.json
   `apps_view`), read and written inside try/catch: with storage refused the page still works.

   What it does not have, and says so on the page: a web page cannot see what is installed on the
   computer, start a program or install one, so there is no Launch, no Install, no "Installed
   first" sort, no "Only what is on this computer" and no games list. The desktop app does those
   (the dated finding docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md,
   which the Other programs part's Why opens in the Library).

   No logo is shown or fetched: each maker sets its own rules for its logo (the finding's judgement
   call 6), so every program and service has the plain cover. HumanityOS's own tools carry their
   page's nav icon from /shared/icons.js, as the app's covers carry theirs.

   Built with createElement and textContent only, never innerHTML, so the data's words are always
   written as text. Required by Node, where scripts/tests/apps-web.test.js runs it. */
(function (root) {
  'use strict';

  const CATALOG_URL = '/data/external/catalog.json';
  const TOOLS_URL = '/data/gui/humanityos_tools.json';
  /** Where the view is kept in this browser. */
  const VIEW_KEY = 'hos_apps_view';
  const DETAILS_ID = 'apps-details';
  const DETAILS_TITLE_ID = 'apps-details-title';
  /** Below this width the details sit under the parts (the page's CSS uses the same width). */
  const NARROW = '(max-width: 900px)';
  /** The desktop app's download page. */
  const DOWNLOAD_URL = '/download';

  /** The words on the page. The desktop app's (src/gui/pages/apps.rs and its parts) unless
      marked "web": those say what only a web page has to say. */
  const WORDS = {
    searchHint: 'Search your apps and help',
    sort: 'Sort',
    show: 'Show',
    all: 'All',
    open: 'Open',
    download: 'Download',
    openWebsite: 'Open website',
    website: 'Website',
    downloadPage: 'Download page',
    close: 'Close',
    about: 'About',
    inBrowser: 'In browser',
    comesFrom: "It comes from its own project: Download page and Website open the project's own pages in your browser.",
    // web: the app's "No Install here." names the button it leaves out; a web page has none.
    noInstall: 'The desktop app has no Install for it. ',
    // web: the app's NOT_IN_WINGET, said of the desktop app.
    notInWinget: "The desktop app has no Install for it: it is not in winget, Windows' own package manager, so Download opens its maker's own page.",
    nothingMatches: 'Nothing here matches the filters.',
    loading: 'Loading...',
    // web
    loadFailed: 'Could not load this part. Reload the page, or report it on the Bugs page.',
    inDesktopApp: 'In the desktop app',
    getDesktopApp: 'Get the desktop app',
    gamesLine: 'The desktop app lists the games installed through Steam, the Epic Games Store and GOG, and starts them through each store\'s own app.',
    addedLine: 'In the desktop app, you can add any game or program on your computer by its program, such as one downloaded from its maker\'s own website.',
    programsDesktop: 'The desktop app can also find the programs on your computer, start them, and install them after you agree to their licence.',
    // The app's WHERE_FROM up to its Install, which a web page does not have (the line above says
    // what the desktop app does).
    whereFrom: "Each program comes from its maker: Download and Website open the maker's own pages in your browser.",
    toolOpensHere: 'Part of HumanityOS: it opens its page on this website.',
    toolDesktopOnly: 'This website has no page for it: it is in the desktop app.',
    tipOpen: 'Opens its page.',
    tipDesktopOnly: 'This website has no page for it. Opens the page where you can get the desktop app.',
    tipDownload: "Opens the project's own download page in your browser.",
    tipWebsite: 'Opens its website in your browser.',
    tipClose: 'Close its details.',
    tipSortBy: 'Sort by this',
    tipAll: 'Every part.',
  };

  /** The app's lines, word for word (scripts/tests/apps-web.test.js reads them out of the Rust). */
  const LINES = {
    safety: "Get programs from their makers, never from an advert, even a top search result. The programs here are free from their makers' own sites; some makers also sell store copies to fund their work, and anyone else asking you to pay for one is not the maker.",
    notAffiliated: 'HumanityOS is not made by, or affiliated with, the makers of these programs.',
    gamesNotAffiliated: 'HumanityOS is not affiliated with Valve, Epic Games or GOG.',
    help: 'Help when it is urgent or you need protecting: crisis lines, health, emergencies and legal aid. Each opens its own website in your browser; in danger, call your local emergency number first.',
    services: 'Services for everyday needs: water, food, housing, money, work and learning. Each opens its own website in your browser. Everything here is external: we point, you choose.',
  };

  /** The Why after where the programs come from: the dated finding in the Library, and its hover
      words (the app's). */
  const WHY = {
    words: 'Why',
    slug: '2026-10-19-tools-page-install-launch-and-icons',
    tip: 'What Microsoft, the makers and the law say about finding, installing and starting programs, their licences and their logos, as researched on 19 October 2026, in the Library.',
  };

  /** The page's six parts, in the app's order (its `Section`): each one's id (the app's), its
      name, its colour, its chip's hover words, and for a catalog part which entries it shows (the
      catalog's kind, and for a help service its category's "section"). `desktop`: the website
      cannot list it. */
  const SECTIONS = [
    { id: 'games', words: 'Games', tint: 'red', desktop: true, tip: 'Only the games from Steam, the Epic Games Store and GOG.' },
    { id: 'tools', words: 'HumanityOS tools', tint: 'green', tip: "Only HumanityOS's own tools: each opens its page here." },
    { id: 'programs', words: 'Other programs', tint: 'blue', kind: 'software', part: null, tip: 'Only the other programs: free programs from their makers.' },
    { id: 'help', words: 'Help', tint: 'cyan', kind: 'service', part: 'help', tip: 'Only help for urgent and protective needs: crisis lines, health, emergencies, legal aid.' },
    { id: 'services', words: 'Services', tint: 'magenta', kind: 'service', part: 'services', tip: 'Only services for everyday needs: water, food, housing, money, work, learning.' },
    { id: 'added', words: 'Added by you', tint: 'yellow', desktop: true, tip: 'Only what you added yourself, games or any other program.' },
  ];
  const sectionOf = (id) => SECTIONS.find((s) => s.id === id) || null;
  const isCatalogPart = (s) => !!(s && s.kind);

  /** The orders the parts are shown in (the app's SortBy that mean something on a web page:
      "Installed first" and the games' sorts need the computer). Category orders a catalog part. */
  const SORTS = [
    { id: 'name', words: 'Name', tip: 'A to Z.' },
    { id: 'category', words: 'Category', tip: 'By category, each under its name.' },
  ];
  /** Grid or List (the app's shelf::Layout), with their hover words. */
  const LAYOUTS = [
    { id: 'grid', words: 'Grid', tip: 'A tile for each.' },
    { id: 'list', words: 'List', tip: 'A row for each, its facts in columns.' },
  ];

  /** The website's page for each of HumanityOS's own tools, by the page id the data file gives it
      (data/gui/humanityos_tools.json "page", the id the app's gui/dispatch.rs gives the page), and
      the nav icon (/shared/icons.js) its cover carries, as the app's covers carry its page's nav
      icon (none: the large first letter, as the app draws for a page without one). `href` null:
      the website has no page for it, and the tile says "In the desktop app". A tool the data file
      gains and this table lacks is shown the same way until its web page is added here. */
  const TOOL_PAGES = {
    maps: { href: '/maps', icon: 'map' },
    tasks: { href: '/tasks', icon: 'tasklist' },
    inventory: { href: '/inventory', icon: 'inventory' },
    crafting: { href: '/crafting', icon: 'crafting' },
    quests: { href: null, icon: 'compass' },
    homes: { href: '/home', icon: 'home' },
    library: { href: '/library', icon: 'journal' },
    notes: { href: '/notes', icon: null },
    calendar: { href: '/calendar', icon: null },
    calculator: { href: '/calculator', icon: null },
    chat: { href: '/chat', icon: 'chat' },
    studio: { href: null, icon: null },
    watch: { href: '/watch', icon: 'video' },
    media: { href: '/media', icon: 'image' },
    market: { href: '/market', icon: 'market' },
    wallet: { href: '/wallet', icon: 'coin' },
    browser: { href: '/web', icon: 'globe' },
  };

  // ── The data ──────────────────────────────────────────────────────────

  /** The catalog's entries in one flat list, each tagged with its category, its kind and its
      section (the app's software::catalog::parse). `i` is its place in the list. */
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
          section: String(cat.section || ''),
        });
      }
    }
    return out;
  }

  /** HumanityOS's own tools in the data file's order (the app's own_tools::parse), each with its
      web page and icon (TOOL_PAGES). `i` is its place in the list. */
  function toolsOf(file) {
    const list = (file && Array.isArray(file.tools)) ? file.tools : [];
    return list.map((t, i) => {
      const page = String((t && t.page) || '');
      const known = Object.prototype.hasOwnProperty.call(TOOL_PAGES, page) ? TOOL_PAGES[page] : null;
      return {
        i,
        name: String((t && t.name) || ''),
        page,
        line: String((t && t.line) || ''),
        href: known ? known.href : null,
        icon: known ? known.icon : null,
      };
    });
  }

  /** Whether catalog entry `e` belongs in part `section`. */
  function inPart(e, section) {
    return isCatalogPart(section) && e.kind === section.kind && (section.part == null || e.section === section.part);
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

  function platformWords(p) {
    return ({ windows: 'Windows', mac: 'Mac', linux: 'Linux', web: 'Web browser' })[p] || p;
  }
  const platformsWords = (e) => e.platforms.map(platformWords).join(', ');

  /** The line under an entry's cover: its category, and its licence when it has one. */
  const metaLine = (e) => (e.license ? e.category + ' · ' + e.license : e.category);

  /** The fact at the right of an entry's button (the app's entry_fact, less "installed"): a help
      service's website, "In browser" for a browser program, else its download size. */
  function entryFact(e) {
    if (!isSoftware(e)) return hostOf(e);
    if (webOnly(e)) return WORDS.inBrowser;
    return e.size;
  }

  /** The main button: Download for a program to download, else Open website. */
  function mainLink(e) {
    if (isSoftware(e) && !webOnly(e)) return { words: WORDS.download, url: downloadPage(e), tip: WORDS.tipDownload };
    return { words: WORDS.openWebsite, url: e.url, tip: WORDS.tipWebsite };
  }

  /** The categories of part `section`'s entries, in the catalog's order, as { id, name }. */
  function categoriesOf(entries, section) {
    const out = [];
    for (const e of entries) {
      if (!inPart(e, section)) continue;
      if (!out.some((c) => c.id === e.categoryId)) out.push({ id: e.categoryId, name: e.category });
    }
    return out;
  }

  /** How many a part holds (before the search narrows it): null for a part the website cannot
      list, or whose data has not come. */
  function countOf(st, section) {
    if (section.desktop) return null;
    if (section.id === 'tools') return st.tools ? st.tools.length : null;
    return st.entries ? st.entries.filter((e) => inPart(e, section)).length : null;
  }

  const cmp = (a, b) => (a < b ? -1 : (a > b ? 1 : 0));
  const wordsOf = (search) => String(search || '').split(/\s+/).filter(Boolean).map((w) => w.toLowerCase());

  /** A catalog part's order (the app's programs::order_of): its entries narrowed to `category`
      (null: every one) and to those whose name, category or description hold every word of
      `search` (letter case ignored), sorted by name (then category, then catalog place), or by
      category and then by name. */
  function catalogOrder(entries, section, category, sort, search) {
    const words = wordsOf(search);
    const out = entries
      .filter((e) => inPart(e, section))
      .filter((e) => category == null || e.categoryId === category)
      .filter((e) => {
        const text = (e.name + '\n' + e.category + '\n' + e.description).toLowerCase();
        return words.every((w) => text.indexOf(w) >= 0);
      })
      .map((e) => e.i);
    const byName = (a, b) => cmp(entries[a].name.toLowerCase(), entries[b].name.toLowerCase())
      || cmp(entries[a].category, entries[b].category) || a - b;
    if (sort === 'category') {
      out.sort((a, b) => cmp(entries[a].category.toLowerCase(), entries[b].category.toLowerCase()) || byName(a, b));
    } else {
      out.sort(byName);
    }
    return out;
  }

  /** HumanityOS's own tools' order (the app's own_tools::order_of): those whose name or line
      holds every word of `search`, by name. */
  function toolsOrder(tools, search) {
    const words = wordsOf(search);
    return tools
      .filter((t) => {
        const text = (t.name + '\n' + t.line).toLowerCase();
        return words.every((w) => text.indexOf(w) >= 0);
      })
      .map((t) => t.i)
      .sort((a, b) => cmp(tools[a].name.toLowerCase(), tools[b].name.toLowerCase()) || a - b);
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

  /** What an entry's details show (the app's entry_detail.rs, less what only the app can know):
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

  /** A hue, 0 to 359: the FNV-1a hash of `seed`, as the app's widgets::swatch_color picks it, so a
      category (or a tool's page) is the same colour on both. */
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

  /** An icon of /shared/icons.js as a CSS image, for a cover's mask (painted in a theme colour by
      the page's CSS): `svgOf(name)` is hosIcon, giving the icon's SVG; null when there is none. */
  function iconImage(svgOf, name) {
    if (!name || typeof svgOf !== 'function') return null;
    let svg = '';
    try { svg = String(svgOf(name, 48, '#000') || ''); } catch (e) { return null; }
    if (svg.indexOf('<svg') !== 0 || svg.indexOf('<g') < 0) return null;
    // As a picture of its own an SVG needs its namespace, which an inline one does without.
    if (svg.indexOf('xmlns=') < 0) svg = svg.replace('<svg', '<svg xmlns="http://www.w3.org/2000/svg"');
    return 'url("data:image/svg+xml,' + encodeURIComponent(svg) + '")';
  }

  // ── The view, kept in this browser ────────────────────────────────────

  const CATEGORY_KEYS = { programs: 'programs_category', help: 'help_category', services: 'services_category' };
  const str = (v) => (typeof v === 'string' && v ? v : null);

  /** A view with every field known: Grid by name, every part and category, unless it says. The
      fields are the app's `apps_view`, less what needs the computer. */
  function normalView(v) {
    const o = v && typeof v === 'object' ? v : {};
    return {
      layout: LAYOUTS.some((l) => l.id === o.layout) ? o.layout : 'grid',
      sort: SORTS.some((s) => s.id === o.sort) ? o.sort : 'name',
      section: sectionOf(o.section) ? o.section : null,
      programs_category: str(o.programs_category),
      help_category: str(o.help_category),
      services_category: str(o.services_category),
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
      storage.setItem(VIEW_KEY, JSON.stringify(normalView(view)));
      return true;
    } catch (e) {
      return false;
    }
  }

  /** `view` with a category its part no longer has let go, so a kept filter never leaves a part
      empty for no reason the page shows. */
  function fitView(view, entries) {
    const v = normalView(view);
    for (const id of Object.keys(CATEGORY_KEYS)) {
      const key = CATEGORY_KEYS[id];
      const s = sectionOf(id);
      if (v[key] != null && !entries.some((e) => inPart(e, s) && e.categoryId === v[key])) v[key] = null;
    }
    return v;
  }

  /** Whether `view` shows part `id`. */
  const shows = (view, id) => view.section == null || view.section === id;

  /** The sorts offered: those that order a part shown (Name always; Category with a catalog
      part), as the app's sorts_for. */
  function sortsFor(view) {
    const catalogShown = SECTIONS.some((s) => isCatalogPart(s) && shows(view, s.id));
    return SORTS.filter((s) => s.id === 'name' || catalogShown);
  }

  // ── The page ──────────────────────────────────────────────────────────

  /**
   * The page, drawn into `opts.root` (#apps-app): the toolbar, the chips and the six parts, with
   * the details beside them. `opts`: document, root, fetch, storage (null when refused), window
   * (for the phone-width check; optional), icon (hosIcon from /shared/icons.js; optional).
   */
  function createAppsPage(opts) {
    const doc = opts.document;
    const host = opts.root;
    const storage = opts.storage || null;
    const win = opts.window || null;
    const icon = typeof opts.icon === 'function' ? opts.icon : null;
    const st = {
      entries: null, tools: null, failedCatalog: false, failedTools: false,
      view: readView(storage), search: '', selected: null,
    };

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

    function setVar(node, name, value) {
      if (value != null && node.style && typeof node.style.setProperty === 'function') node.style.setProperty(name, String(value));
    }

    // The parts that stay for the page's life (the search field keeps its focus and its words).
    const search = el('input', {
      type: 'search', id: 'apps-search', class: 'apps-search', placeholder: WORDS.searchHint,
      'aria-label': WORDS.searchHint, autocomplete: 'off', spellcheck: 'false',
    });
    const sortGroup = el('div', { class: 'apps-pills', role: 'group', 'aria-labelledby': 'apps-sort-label' });
    const layoutGroup = el('div', { class: 'apps-pills', role: 'group', 'aria-labelledby': 'apps-show-label' });
    const toolbar = el('div', { class: 'apps-toolbar' }, [
      search,
      el('span', { class: 'apps-label', id: 'apps-sort-label', text: WORDS.sort }), sortGroup,
      el('span', { class: 'apps-label', id: 'apps-show-label', text: WORDS.show }), layoutGroup,
    ]);
    const chipsRow = el('div', { class: 'apps-pills apps-chips', id: 'apps-chips', role: 'group', 'aria-label': 'Parts' });
    const status = el('p', { class: 'sr-only', id: 'apps-status', role: 'status' });
    const parts = el('div', { class: 'apps-parts', id: 'apps-parts' });
    const main = el('div', { class: 'apps-main' }, [toolbar, chipsRow, status, parts]);
    const details = el('aside', { class: 'apps-details', id: DETAILS_ID, 'aria-labelledby': DETAILS_TITLE_ID, tabindex: '-1', hidden: true });

    // ── Pills: rebuilt only when the choices change, so a pressed pill keeps its focus.
    const pillKeys = new Map();
    function pills(group, options, chosen, onPick, cls) {
      const key = options.map((o) => o.words + '\u0001' + (o.tip || '') + '\u0001' + (o.tint || '')).join('\u0002');
      if (pillKeys.get(group) !== key) {
        pillKeys.set(group, key);
        group.replaceChildren();
        options.forEach((o, n) => {
          const b = el('button', {
            type: 'button', class: (cls || 'apps-pill') + (o.tint ? ' apps-tint-' + o.tint : ''),
            'data-pick': n, 'data-part': o.part || null, title: o.tip || null, text: o.words,
          });
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
      const sorts = sortsFor(v);
      // A sort not offered shows as Name, and stays kept for when it is offered again.
      pills(sortGroup, sorts, Math.max(0, sorts.findIndex((s) => s.id === v.sort)), (n) => setView({ sort: sorts[n].id }));
      pills(layoutGroup, LAYOUTS, Math.max(0, LAYOUTS.findIndex((l) => l.id === v.layout)), (n) => setView({ layout: LAYOUTS[n].id }));
      // The chips: All, and a part each in its colour with its name and how many.
      const counts = SECTIONS.map((s) => countOf(st, s));
      const total = counts.reduce((n, c) => n + (c || 0), 0);
      const options = [{ part: null, words: WORDS.all + ' ' + total, tip: WORDS.tipAll }]
        .concat(SECTIONS.map((s, k) => ({ part: s.id, tint: s.tint, words: counts[k] == null ? s.words : s.words + ' ' + counts[k], tip: s.tip })));
      pills(chipsRow, options, Math.max(0, options.findIndex((o) => o.part === v.section)), (n) => {
        const id = options[n].part;
        // A chip pressed again lets go of its part, as the app's does.
        setView({ section: id != null && st.view.section === id ? null : id });
      }, 'apps-chip');
    }

    // ── Pieces of a part

    function band(section, count, kids) {
      return el('h2', { class: 'apps-band apps-tint-' + section.tint, id: 'apps-band-' + section.id }, [
        el('span', { class: 'apps-band-name', text: section.words }),
        count == null ? null : el('span', { class: 'apps-band-count', text: String(count) }),
      ].concat(kids || []));
    }

    function line(text, cls) {
      return el('p', { class: 'apps-line' + (cls ? ' ' + cls : ''), text });
    }

    function desktopLine(text) {
      return el('p', { class: 'apps-line apps-desktop' }, [text + ' ', el('a', { href: DOWNLOAD_URL, text: WORDS.getDesktopApp })]);
    }

    function cover(name, seed, cls, iconName) {
      const img = iconImage(icon, iconName);
      const c = el('span', { class: cls + (img ? ' has-icon' : ''), 'aria-hidden': 'true' }, [
        img ? el('span', { class: 'app-cover-icon' }) : el('span', { class: 'app-cover-letter', text: monogram(name) }),
        el('span', { class: 'app-cover-band', text: name }),
      ]);
      setVar(c, '--app-hue', hueOf(seed));
      if (img) setVar(c.firstChild, '--app-icon', img);
      return c;
    }

    function dot(seed) {
      const d = el('span', { class: 'app-dot', 'aria-hidden': 'true' });
      setVar(d, '--app-hue', hueOf(seed));
      return d;
    }

    /** A catalog entry's main button: a link that opens the maker's page in a new tab. */
    function entryButton(e, size) {
      const m = mainLink(e);
      const url = safeUrl(m.url);
      if (!url) return null;
      return el('a', {
        class: 'app-btn ' + size, href: url, target: '_blank', rel: 'noopener noreferrer',
        title: m.tip, 'aria-label': m.words + ' ' + e.name, 'data-go': e.i, text: m.words,
      });
    }

    /** A tool's Open (its page on this website), or "In the desktop app" (a link to /download). */
    function toolButton(t, size) {
      if (t.href) {
        return el('a', { class: 'app-btn primary ' + size, href: t.href, title: WORDS.tipOpen, 'aria-label': WORDS.open + ' ' + t.name, 'data-go': 't' + t.i, text: WORDS.open });
      }
      return el('a', { class: 'app-desktop-only', href: DOWNLOAD_URL, title: WORDS.tipDesktopOnly, 'aria-label': t.name + ': ' + WORDS.inDesktopApp, 'data-go': 't' + t.i, text: WORDS.inDesktopApp });
    }

    /** A tile: its cover, a button that opens the details (named by its name, then its line); its
        line; and its button row (the button at the left, one fact at the right). */
    function tile(key, name, seed, meta, iconName, button, fact) {
      const chosen = st.selected === key;
      const open = el('button', {
        type: 'button', class: 'app-open', 'data-open': key, 'aria-controls': DETAILS_ID,
        'aria-expanded': chosen ? 'true' : 'false', 'aria-label': name + ', ' + meta + (fact ? ', ' + fact : ''),
      }, [cover(name, seed, 'app-cover', iconName)]);
      open.addEventListener('click', () => select(key));
      return el('article', { class: 'app-tile' + (chosen ? ' selected' : ''), 'data-key': key }, [
        open,
        el('span', { class: 'app-meta', title: meta, text: meta }),
        el('div', { class: 'app-row' }, [button, fact ? el('span', { class: 'app-fact', title: fact, text: fact }) : null]),
      ]);
    }

    function entryTile(e) {
      return tile('e' + e.i, e.name, e.categoryId, metaLine(e), null, entryButton(e, 'small'), entryFact(e));
    }

    function toolTile(t) {
      return tile('t' + t.i, t.name, t.page, t.line, t.icon, toolButton(t, 'small'), null);
    }

    function grid(nodes) {
      return el('div', { class: 'apps-grid' }, nodes);
    }

    /** A name in a List row: a button that opens the details. */
    function rowName(key, name) {
      const chosen = st.selected === key;
      const b = el('button', { type: 'button', class: 'app-row-name', 'data-open': key, 'aria-controls': DETAILS_ID, 'aria-expanded': chosen ? 'true' : 'false', text: name });
      b.addEventListener('click', () => select(key));
      return b;
    }

    /** A List: a table whose sortable headings sort the parts. `cols`: [words, class, sort id]. */
    function table(caption, cols, rows) {
      const v = st.view;
      const head = (words, cls, sortId) => {
        if (!sortId) return el('th', { scope: 'col', class: cls, text: words });
        const on = v.sort === sortId;
        const b = el('button', { type: 'button', class: 'apps-sort' + (on ? ' on' : ''), 'data-sort': sortId, 'data-focus': caption + ':' + sortId, title: WORDS.tipSortBy, text: words });
        b.addEventListener('click', () => setView({ sort: sortId }));
        return el('th', { scope: 'col', class: cls, 'aria-sort': on ? 'ascending' : null }, [b]);
      };
      const thead = el('thead', null, [el('tr', null, [el('th', { scope: 'col', class: 'col-act' }, [el('span', { class: 'sr-only', text: 'Link' })])]
        .concat(cols.map((c) => head(c[0], c[1], c[2]))))]);
      return el('div', { class: 'apps-table-wrap' }, [
        el('table', { class: 'apps-table' }, [el('caption', { class: 'sr-only', text: caption }), thead, el('tbody', null, rows)]),
      ]);
    }

    function row(key, cells) {
      return el('tr', { class: st.selected === key ? 'selected' : null, 'data-key': key }, cells);
    }

    // ── The parts

    function gamesPart(section) {
      return [band(section, null), desktopLine(WORDS.gamesLine), line(LINES.gamesNotAffiliated, 'apps-muted')];
    }

    function addedPart(section) {
      return [band(section, null), desktopLine(WORDS.addedLine)];
    }

    function toolsPart(section) {
      const out = [band(section, countOf(st, section))];
      if (st.failedTools) return out.concat(line(WORDS.loadFailed, 'apps-muted'));
      if (!st.tools) return out.concat(line(WORDS.loading, 'apps-muted'));
      const order = toolsOrder(st.tools, st.search);
      shown.n += order.length;
      shown.of += st.tools.length;
      if (!order.length) return out.concat(line('No HumanityOS tool holds "' + st.search.trim() + '".', 'apps-muted'));
      if (st.view.layout === 'list') {
        const rows = order.map((i) => {
          const t = st.tools[i];
          return row('t' + i, [
            el('td', { class: 'col-act' }, [toolButton(t, 'medium')]),
            el('td', { class: 'col-name' }, [rowName('t' + i, t.name)]),
            el('td', { class: 'col-line', text: t.line }),
          ]);
        });
        return out.concat(table(section.words, [['Name', 'col-name'], ['What it is', 'col-line']], rows));
      }
      return out.concat(grid(order.map((i) => toolTile(st.tools[i]))));
    }

    function catalogPart(section) {
      const v = st.view;
      const out = [band(section, countOf(st, section))];
      if (st.failedCatalog) return out.concat(line(WORDS.loadFailed, 'apps-muted'));
      if (!st.entries) return out.concat(line(WORDS.loading, 'apps-muted'));
      const key = CATEGORY_KEYS[section.id];
      const cats = categoriesOf(st.entries, section);
      if (cats.length > 1) {
        const group = el('div', { class: 'apps-pills apps-cats', role: 'group', 'aria-label': section.words + ': category', 'data-cats': section.id });
        const options = [{ id: null, words: WORDS.all }].concat(cats.map((c) => ({ id: c.id, words: c.name })));
        const chosen = Math.max(0, options.findIndex((o) => o.id === v[key]));
        options.forEach((o, n) => {
          const b = el('button', { type: 'button', class: 'apps-pill', 'data-focus': section.id + ':cat:' + (o.id || ''), 'aria-pressed': n === chosen ? 'true' : 'false', text: o.words });
          b.addEventListener('click', () => setView({ [key]: o.id }));
          group.appendChild(b);
        });
        out.push(group);
      }
      const sort = sortsFor(v).some((s) => s.id === v.sort) ? v.sort : 'name';
      const order = catalogOrder(st.entries, section, v[key], sort, st.search);
      shown.n += order.length;
      shown.of += countOf(st, section);
      if (!order.length) {
        const q = st.search.trim();
        out.push(line(q ? 'Nothing here matches "' + q + '".' : WORDS.nothingMatches, 'apps-muted'));
      } else if (v.layout === 'list') {
        const programs = section.id === 'programs';
        const cols = programs
          ? [['Name', 'col-name', 'name'], ['Category', 'col-cat', 'category'], ['Licence', 'col-lic'], ['Runs on', 'col-plat'], ['Size', 'col-size']]
          : [['Name', 'col-name', 'name'], ['Category', 'col-cat', 'category'], ['Website', 'col-web']];
        const rows = order.map((i) => {
          const e = st.entries[i];
          const cells = [el('td', { class: 'col-act' }, [entryButton(e, 'medium')]), el('td', { class: 'col-name' }, [rowName('e' + i, e.name)]), el('td', { class: 'col-cat', text: e.category })];
          if (programs) cells.push(el('td', { class: 'col-lic', text: e.license }), el('td', { class: 'col-plat', text: platformsWords(e) }), el('td', { class: 'col-size', text: webOnly(e) ? '' : e.size }));
          else cells.push(el('td', { class: 'col-web', text: hostOf(e) }));
          return row('e' + i, cells);
        });
        out.push(table(section.words, cols, rows));
      } else if (sort === 'category') {
        // Each category under its name, with how many.
        for (const g of groupsOf(st.entries, order)) {
          out.push(el('h3', { class: 'apps-group' }, [dot(g.categoryId), el('span', { text: g.category }), el('span', { class: 'apps-group-count', text: String(g.items.length) })]));
          out.push(grid(g.items.map((i) => entryTile(st.entries[i]))));
        }
      } else {
        out.push(grid(order.map((i) => entryTile(st.entries[i]))));
      }
      // Its lines below, as the app's.
      if (section.id === 'programs') {
        out.push(el('p', { class: 'apps-line apps-where' }, [WORDS.whereFrom, el('a', { class: 'apps-why', href: '/library#' + WHY.slug, title: WHY.tip, text: WHY.words })]));
        out.push(line(LINES.safety, 'apps-safety'));
        out.push(desktopLine(WORDS.programsDesktop));
        out.push(line(LINES.notAffiliated, 'apps-muted'));
      } else {
        out.push(line(section.id === 'help' ? LINES.help : LINES.services));
      }
      return out;
    }

    /** How many items the parts show, of how many they hold (the status line). */
    const shown = { n: 0, of: 0 };

    function renderParts() {
      // The pill, heading or chip that had the focus, to give it back once the parts are redrawn.
      const had = doc.activeElement && parts.contains && parts.contains(doc.activeElement) && doc.activeElement.getAttribute
        ? doc.activeElement.getAttribute('data-focus') : null;
      shown.n = 0;
      shown.of = 0;
      const nodes = [];
      for (const s of SECTIONS) {
        if (!shows(st.view, s.id)) continue;
        let kids;
        if (s.id === 'games') kids = gamesPart(s);
        else if (s.id === 'added') kids = addedPart(s);
        else if (s.id === 'tools') kids = toolsPart(s);
        else kids = catalogPart(s);
        nodes.push(el('section', { class: 'apps-part', 'data-part': s.id, 'aria-labelledby': 'apps-band-' + s.id }, kids));
      }
      parts.replaceChildren(...nodes);
      status.textContent = st.entries || st.tools ? 'Showing ' + shown.n + ' of ' + shown.of + '.' : '';
      if (had) {
        const again = parts.querySelector('[data-focus="' + had + '"]');
        if (again) again.focus();
      }
    }

    // ── The details

    /** The details' banner (its cover, wide, the name its heading) and the line under it: what
        it is (a category with its dot, or the part's name) and Close. */
    function detailsHead(name, seed, iconName, label, labelSeed) {
      const img = iconImage(icon, iconName);
      const banner = el('div', { class: 'app-banner' + (img ? ' has-icon' : '') }, [
        img ? el('span', { class: 'app-cover-icon', 'aria-hidden': 'true' }) : el('span', { class: 'app-cover-letter', 'aria-hidden': 'true', text: monogram(name) }),
        el('h2', { class: 'app-cover-band', id: DETAILS_TITLE_ID, text: name }),
      ]);
      setVar(banner, '--app-hue', hueOf(seed));
      if (img) setVar(banner.firstChild, '--app-icon', img);
      const closeBtn = el('button', { type: 'button', class: 'app-close', title: WORDS.tipClose, 'aria-label': WORDS.close + ' ' + name, text: WORDS.close });
      closeBtn.addEventListener('click', close);
      return [banner, el('div', { class: 'app-detail-cat' }, [labelSeed ? dot(labelSeed) : null, el('span', { class: 'app-detail-cat-name', text: label }), closeBtn])];
    }

    function entryDetails(e) {
      const d = detailsOf(e);
      const links = el('div', { class: 'app-links' }, d.links.map((l) => el('a', {
        class: 'app-btn medium', href: safeUrl(l.url), target: '_blank', rel: 'noopener noreferrer', title: l.tip, 'data-link': l.words, text: l.words,
      })));
      const facts = el('dl', { class: 'app-facts' });
      for (const [label, value] of d.rows) facts.append(el('dt', { text: label }), el('dd', { text: value }));
      return detailsHead(d.name, e.categoryId, null, d.category, e.categoryId).concat([
        links,
        d.note ? el('p', { class: 'app-note', text: d.note }) : null,
        el('p', { class: 'app-desc', text: d.description }),
        el('h3', { class: 'app-section', text: WORDS.about }),
        facts,
        d.comesFrom ? el('p', { class: 'app-comes-from', text: d.comesFrom }) : null,
      ]);
    }

    function toolDetails(t) {
      const tools = sectionOf('tools');
      return detailsHead(t.name, t.page, t.icon, tools.words, null).concat([
        el('p', { class: 'app-desc', text: t.line }),
        el('div', { class: 'app-links' }, [t.href
          ? el('a', { class: 'app-btn primary large', href: t.href, title: WORDS.tipOpen, 'data-link': WORDS.open, text: WORDS.open })
          : el('a', { class: 'app-btn large', href: DOWNLOAD_URL, title: WORDS.tipDesktopOnly, 'data-link': WORDS.getDesktopApp, text: WORDS.getDesktopApp })]),
        el('p', { class: 'app-comes-from', text: t.href ? WORDS.toolOpensHere : WORDS.toolDesktopOnly }),
      ]);
    }

    /** What `key` names ("e12": catalog entry 12, "t3": tool 3), when its data is here. */
    function itemOf(key) {
      if (typeof key !== 'string') return null;
      const n = Number(key.slice(1));
      if (key[0] === 'e' && st.entries && st.entries[n]) return { kind: 'entry', item: st.entries[n] };
      if (key[0] === 't' && st.tools && st.tools[n]) return { kind: 'tool', item: st.tools[n] };
      return null;
    }

    function renderDetails() {
      const it = itemOf(st.selected);
      if (host.classList) host.classList.toggle('has-details', !!it);
      details.replaceChildren();
      details.hidden = !it;
      if (!it) return;
      // Only nodes: a browser's append() writes a null as the word "null".
      details.append(...(it.kind === 'entry' ? entryDetails(it.item) : toolDetails(it.item)).filter(Boolean));
    }

    function render() {
      renderToolbar();
      renderParts();
      renderDetails();
    }

    function isNarrow() {
      try { return !!(win && win.matchMedia && win.matchMedia(NARROW).matches); } catch (e) { return false; }
    }

    /** Open `key`'s details, and move to them (on a phone they are under the parts). */
    function select(key) {
      if (!itemOf(key)) return;
      st.selected = key;
      renderParts();
      renderDetails();
      try { details.focus({ preventScroll: true }); } catch (e) { /* an old browser: no focus options */ }
      if (isNarrow() && typeof details.scrollIntoView === 'function') details.scrollIntoView({ block: 'start' });
    }

    /** Close the details, and give the focus back to what opened them (the search field when a
        filter has since hidden it). */
    function close() {
      if (st.selected == null) return;
      const key = st.selected;
      st.selected = null;
      renderParts();
      renderDetails();
      const back = parts.querySelector('[data-open="' + key + '"]');
      if (back) back.focus(); else search.focus();
    }

    function onKey(ev) {
      if (ev.key === 'Escape' && st.selected != null) {
        if (ev.preventDefault) ev.preventDefault();
        close();
      }
    }

    /** Fetch `url` as JSON. */
    function load(url) {
      return Promise.resolve()
        .then(() => opts.fetch(url, { cache: 'no-cache' }))
        .then((resp) => {
          if (!resp || !resp.ok) throw new Error('HTTP ' + (resp && resp.status));
          return resp.json();
        });
    }

    function start() {
      host.replaceChildren(main, details);
      search.addEventListener('input', () => { st.search = search.value; renderParts(); });
      doc.addEventListener('keydown', onKey);
      render();
      const failed = (what) => (err) => {
        if (root.console) root.console.error('apps: could not load ' + what, err);
      };
      return Promise.all([
        load(CATALOG_URL).then((json) => {
          st.entries = entriesOf(json);
          st.view = fitView(st.view, st.entries);
        }).catch((err) => { st.failedCatalog = true; failed(CATALOG_URL)(err); }),
        load(TOOLS_URL).then((json) => {
          st.tools = toolsOf(json);
        }).catch((err) => { st.failedTools = true; failed(TOOLS_URL)(err); }),
      ]).then(() => render());
    }

    return { start, select, close, setView, state: st, parts: { search, parts, details, sortGroup, layoutGroup, chipsRow, status } };
  }

  // ── In the browser ──
  function bootInBrowser() {
    const doc = root.document;
    if (!doc) return;
    const go = () => {
      const host = doc.getElementById('apps-app');
      if (!host) return;
      let storage = null;
      try { storage = root.localStorage; } catch (e) { storage = null; }
      createAppsPage({ document: doc, root: host, fetch: (u, i) => root.fetch(u, i), storage, window: root, icon: root.hosIcon }).start();
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', go);
    else go();
  }

  const api = {
    CATALOG_URL, TOOLS_URL, VIEW_KEY, DOWNLOAD_URL, WORDS, LINES, WHY, SECTIONS, SORTS, LAYOUTS, TOOL_PAGES,
    entriesOf, toolsOf, inPart, isSoftware, webOnly, downloadPage, safeUrl, hostOf, platformWords, platformsWords,
    metaLine, entryFact, mainLink, categoriesOf, catalogOrder, toolsOrder, groupsOf, detailsOf, hueOf, monogram,
    iconImage, normalView, readView, writeView, fitView, sortsFor, createAppsPage,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else {
    root.HosApps = api;
    bootInBrowser();
  }
})(typeof window !== 'undefined' ? window : globalThis);
