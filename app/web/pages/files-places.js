// ── files-places.js ───────────────────────────────────────────────────────
// The Files page's PLACES on the website (2026-10-19), the desktop app's
// src/gui/pages/files/places.rs: where the files shown come from. One Files page
// holds what were three (the Media page, the Shared Files page and the data
// browser at /files); the operator, 2026-10-19: "What's the real difference
// between these two? Why would we not want to merge them?"
//
// IN THE LAUNCHER'S RED, GREEN AND BLUE (2026-11-03), as the desktop's are. The
// operator, on the navigation mock-up of the page: "I feel like that first image
// should be ordered as your private local files files, your publicly shared files,
// then files by friends/servers? Maintains the RGB styling. R for private, B for
// servers." Top to bottom, each group headed and tinted in its colour (the theme's
// --danger, --success and --info, the tokens the desktop's character and server
// picker titles its three trust models in):
//
// - RED, private, on this computer: the desktop's HumanityOS folder and Your
//   folders. Neither has a website version (a web page cannot look through a
//   folder the same way in every browser, operator 2026-10-13), so the group is
//   its heading and a line saying where they are.
// - GREEN, what you share: "Yours on <server>" (the page's own list, asked for
//   with the person filter set to you: files-app.js `listFilters`) and "Your
//   shared files" (/pages/files-shared.js with only your rows).
// - BLUE, what everyone shares: "Shared on <server>" (what people chose to show:
//   the list, the featured shelf, the filters, then an item and its details; the
//   page's own list), "Shared files" (the server's public file library) and for an
//   admin of the server "Admin" (who sees the page, its space, its categories and
//   what to prune) and "The server's data folder" (read and edited as the data
//   browser did: /pages/files-folder.js; the server serves it to its admins alone,
//   BUG-206). The desktop's "HumanityOS folder" is the app's own folder on the
//   computer, so the server's is not called that here (2026-10-20: one name, one
//   thing).
//
// The page still opens on Shared on <server> (/files), as the desktop's does.
//
// The places are at the top of the left pane: in the list's pane while Shared
// on <server> or Yours on <server> is shown in the panes, at the top of the
// place's own left pane for the two shared files places and the server's data
// folder, and above the page otherwise (the Gallery, Admin). Each has its own
// address, so Back and a link name it: /files, /files#yours,
// /files#your-shared-files, /files#shared-files, /files#folder, /files#admin.
//
// The words are keys in data/i18n/en.json (files.places.*): the page's i18n
// (web/shared/i18n.js) when it has it, else English (window.HOS_I18N_EN, or under
// Node en.json itself).
//
// A classic script in the browser (FilesPlaces on window), a CommonJS module
// under Node, where scripts/tests/files-web-places.test.js drives it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const NODE = typeof module === 'object' && module && module.exports;

  /** English, by key, when the page has no i18n: the generated table, or under Node en.json. */
  function english() {
    if (root.HOS_I18N_EN) return root.HOS_I18N_EN;
    if (!NODE) return null;
    try {
      const path = require('path');
      return require(path.join(__dirname, '..', '..', 'data', 'i18n', 'en.json'));
    } catch (e) {
      return null;
    }
  }
  function lookup(table, key) {
    let cur = table;
    for (const part of String(key).split('.')) {
      if (cur == null || typeof cur !== 'object') return undefined;
      cur = cur[part];
    }
    return typeof cur === 'string' ? cur : undefined;
  }
  /** A key's words, its {{name}}s filled. */
  function words(key, vars) {
    if (root.i18n && typeof root.i18n.t === 'function') {
      const v = root.i18n.t(key, vars);
      if (v && v !== key) return v;
    }
    const s = lookup(english(), key);
    if (s === undefined) return key;
    return s.replace(/\{\{(\w+)\}\}/g, (m, name) => (vars && vars[name] !== undefined ? String(vars[name]) : m));
  }

  const WORDS = Object.freeze({
    get label() { return words('files.places.label'); },
    get sharedFiles() { return words('files.places.shared_files'); },
    get yourSharedFiles() { return words('files.places.your_shared_files'); },
    get folder() { return words('files.places.server_folder'); },
    get admin() { return words('files.places.admin'); },
    get privateLine() { return words('files.places.private_web'); },
  });

  /**
   * The groups, top to bottom, each with its colour (the theme's token, files-places.css) and its
   * places. The private group has no place on the website: the desktop's two are on the computer.
   */
  const GROUPS = Object.freeze([
    Object.freeze({ id: 'private', tint: 'red', heading: 'files.places.private', places: Object.freeze([]) }),
    Object.freeze({ id: 'you-share', tint: 'green', heading: 'files.places.you_share', places: Object.freeze(['yours', 'your-shared-files']) }),
    Object.freeze({ id: 'everyone-shares', tint: 'blue', heading: 'files.places.everyone_shares', places: Object.freeze(['shared', 'shared-files', 'admin', 'folder']) }),
  ]);
  /** The places an admin of the server alone has (the server serves its data folder to its admins alone, BUG-206). */
  const ADMINS = Object.freeze(['admin', 'folder']);
  /** Each place's hint, by its key. */
  const HINTS = Object.freeze({
    yours: 'files.places.yours_hint',
    'your-shared-files': 'files.places.your_shared_files_hint',
    shared: 'files.places.shared_hint',
    'shared-files': 'files.places.shared_files_hint',
    admin: 'files.places.admin_hint',
    folder: 'files.places.server_folder_hint',
  });

  /** The places listed, in the desktop's order: green, then blue, an admin's two last. */
  function listed(admin) {
    const out = [];
    for (const g of GROUPS) for (const p of g.places) if (admin || !ADMINS.includes(p)) out.push(p);
    return out;
  }
  /** The group a place is in. */
  function groupOf(place) {
    return GROUPS.find((g) => g.places.includes(place)) || null;
  }
  /** The place an address's hash names (the page's own list for any other). */
  function placeOf(hash) {
    const h = String(hash || '');
    for (const g of GROUPS) for (const p of g.places) if (p !== 'shared' && h === '#' + p) return p;
    return 'shared';
  }
  /** A place's own address. */
  function addressOf(place) {
    return place === 'shared' ? '/files' : '/files#' + place;
  }
  /**
   * The place shown for `place`: Admin and the server's data folder are an admin's alone, and
   * anyone else on one of them sees what is shared.
   */
  function shownPlace(place, admin) {
    return listed(admin).includes(place) ? place : 'shared';
  }
  /** Whether `place` shows the page's own list: everyone's, or yours alone. */
  function showsTheList(place) {
    return place === 'shared' || place === 'yours';
  }
  /** A place's name in the list. */
  function labelOf(place, host) {
    if (place === 'shared') return host ? words('files.places.shared_on', { host }) : words('files.places.shared_on_this_server');
    if (place === 'yours') return host ? words('files.places.yours_on', { host }) : words('files.places.yours_on_this_server');
    if (place === 'your-shared-files') return WORDS.yourSharedFiles;
    if (place === 'shared-files') return WORDS.sharedFiles;
    if (place === 'folder') return WORDS.folder;
    return WORDS.admin;
  }
  /** A place's hint: what it holds, in plain words. */
  function hintOf(place) {
    return HINTS[place] ? words(HINTS[place]) : '';
  }
  /** Which list the page's own panes show for `place`: 'yours' for Yours on <server>, else 'shared'. */
  function listOf(place, admin) {
    return shownPlace(place, admin) === 'yours' ? 'yours' : 'shared';
  }
  /**
   * The filters the list is asked for with: for Yours on <server> only this person's items, newest
   * first, whatever the filters picked for everyone's say (they stay for when that list is shown
   * again); for everyone's, the filters picked. The desktop's files.rs `list_filters`.
   */
  function listFilters(list, filters, myKey) {
    return list === 'yours' ? { by: myKey || '' } : filters;
  }
  /**
   * "How this page works" for the places (files-app.js hands in the fold's drawer, `f`): each place's
   * name in its group's colour, then its line, in the places' order, as the desktop's files/how.rs
   * `panel` draws them; the desktop's private places are on the computer, so the website has no line
   * for them. o: { host, admin, pageLine (Shared on's), sharedFilesLine, folderLine (an admin's) }.
   */
  function fillFold(f, o) {
    const lines = {
      yours: hintOf('yours'), 'your-shared-files': hintOf('your-shared-files'), shared: o.pageLine,
      'shared-files': o.sharedFilesLine, admin: hintOf('admin'), folder: o.folderLine,
    };
    for (const p of listed(!!o.admin)) {
      if (!lines[p]) continue;
      f.part(labelOf(p, o.host), groupOf(p).tint);
      f.line(lines[p]);
    }
  }

  /**
   * The places list and the panes of the places that are not the page's own list.
   * env: { h, host(), isAdmin(), pick(place), MP (files-panes.js, or null), viewport(),
   *        shares(way), setShares(way, shares, settled),
   *        places: { 'shared-files': factory, 'your-shared-files': factory, folder: factory } }
   * Each factory(env) returns { left, middle, right, start() } and, for a place that takes dropped
   * files, dropDecision() and takeFiles(files).
   */
  function createPlaces(env) {
    const h = env.h;
    const el = h('nav', { id: 'files-places', className: 'files-places', attrs: { 'aria-label': WORDS.label } });
    const area = h('div', { id: 'files-place-area', className: 'files-place-area', hidden: true });
    const made = {};      // the places made so far, by name
    let panes = null;     // the place area's three panes
    let leftTop = null;   // where the places go at the top of the place area's left pane

    function render(current, admin) {
      const host = typeof env.host === 'function' ? env.host() : '';
      const item = (place, tint) => {
        const b = h('button', {
          type: 'button', id: 'files-place-' + place,
          className: 'files-place files-place-' + tint + (place === current ? ' files-place-active' : ''),
          text: labelOf(place, host),
          attrs: place === current ? { 'aria-current': 'page' } : {},
          on: { click: () => { if (place !== current && typeof env.pick === 'function') env.pick(place); } },
        });
        // Its hint, shown on hover and focus by the person's hint mode (web/shared/shell.js).
        b.dataset.tipDesc = hintOf(place);
        return b;
      };
      const kids = [];
      for (const g of GROUPS) {
        const here = g.places.filter((p) => listed(admin).includes(p));
        kids.push(h('p', { className: 'files-places-group files-places-' + g.tint, dataset: { group: g.id }, text: words(g.heading) }));
        if (!here.length) kids.push(h('p', { className: 'files-places-none files-hint', text: WORDS.privateLine }));
        for (const p of here) kids.push(item(p, g.tint));
      }
      el.replaceChildren(...kids);
    }

    function ensureArea() {
      if (panes || leftTop) return;
      const MP = env.MP || null;
      leftTop = h('div', { id: 'files-place-left', className: 'files-pane files-place-left' });
      const middle = h('div', { id: 'files-place-middle', className: 'files-pane files-place-middle' });
      const right = h('div', { id: 'files-place-right', className: 'files-pane files-place-right' });
      if (MP) {
        panes = MP.createPanes({
          h, id: 'files-place-panes', count: 3, viewport: env.viewport, measure: env.viewport, win: env.window || null,
          shares: env.shares, defaults: (way) => (way === 'across' ? MP.ACROSS : MP.DOWN), setShares: env.setShares,
        });
        panes.el.append(leftTop, panes.handle(0), middle, panes.handle(1), right);
        area.appendChild(panes.el);
      } else {
        area.appendChild(h('div', { className: 'files-place-columns' }, [leftTop, middle, right]));
      }
      area.slots = { middle, right };
    }

    /** The place made once, its parts put in the area's panes. */
    function placeFor(name) {
      if (made[name]) return made[name];
      const factory = env.places && env.places[name];
      if (typeof factory !== 'function') return null;
      made[name] = factory(env);
      return made[name];
    }

    /**
     * Show `current`: the list drawn with it picked, the list put where it belongs (`listPane`, the
     * page's list pane while its panes show, else `top`), and the area shown for a place of its own.
     */
    function show(current, admin, where) {
      render(current, admin);
      const own = !showsTheList(current) && current !== 'admin';
      area.hidden = !own;
      if (own) {
        ensureArea();
        const place = placeFor(current);
        const parts = place ? [place.left, place.middle, place.right] : [null, null, null];
        leftTop.replaceChildren(el, ...(parts[0] ? [parts[0]] : []));
        area.slots.middle.replaceChildren(...(parts[1] ? [parts[1]] : []));
        area.slots.right.replaceChildren(...(parts[2] ? [parts[2]] : []));
        // Started once, after its parts are on the page (the server's data folder finds its own by id).
        if (place && !place.started && typeof place.start === 'function') { place.started = true; place.start(); }
        if (panes && typeof panes.layout === 'function') panes.layout();
        return;
      }
      const at = where && where.listPane ? where.listPane : where && where.top;
      if (at && el.parentNode !== at) {
        if (typeof at.prepend === 'function') at.prepend(el);
        else at.insertBefore(el, at.firstChild || null);
      }
    }

    /** Whether files dropped while `current` is shown go to that place (the shared files places take them). */
    function takesDrops(current) {
      const p = made[current];
      return !!(p && typeof p.dropDecision === 'function');
    }
    function dropDecision(current) {
      const p = made[current];
      return p && typeof p.dropDecision === 'function' ? p.dropDecision() : { take: false, reason: '' };
    }
    function takeFiles(current, files) {
      const p = made[current];
      if (p && typeof p.takeFiles === 'function') p.takeFiles(files);
    }
    function layout() {
      if (panes && !area.hidden && typeof panes.layout === 'function') panes.layout();
    }

    return { el, area, render, show, takesDrops, dropDecision, takeFiles, layout, made };
  }

  const api = { WORDS, GROUPS, words, listed, groupOf, placeOf, addressOf, shownPlace, showsTheList, labelOf, hintOf, listOf, listFilters, fillFold, createPlaces };
  if (NODE) module.exports = api;
  else root.FilesPlaces = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
