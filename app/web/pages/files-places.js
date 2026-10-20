// ── files-places.js ───────────────────────────────────────────────────────
// The Files page's PLACES on the website (2026-10-19), the desktop app's
// src/gui/pages/files/places.rs: where the files shown come from. One Files page
// holds what were three (the Media page, the Shared Files page and the data
// browser at /files); the operator, 2026-10-19: "What's the real difference
// between these two? Why would we not want to merge them?"
//
// On the server: "Shared on <server>" (what people chose to show: the list,
// the featured shelf, the filters, then an item and its details; the page's own
// list), "Shared files" (the server's public file library: /pages/files-shared.js),
// and for an admin of the server "Admin" (who sees the page, its space, its
// categories and what to prune) and "The server's data folder" (read and edited
// as the data browser did: /pages/files-folder.js; the server serves it to its
// admins alone, BUG-206). In the desktop's order (places.rs `listed`: Shared on
// <server>, Shared files, Admin, then the computer's places). The desktop's
// "HumanityOS folder" is the app's own folder on the computer, so the server's
// is not called that here (2026-10-20: one name, one thing); its Your folders
// (folders on the computer) has no website version (operator, 2026-10-13: a web
// page cannot look through a folder the same way in every browser).
//
// The places are at the top of the left pane: in the list's pane while Shared
// on <server> is shown in the panes, at the top of the place's own left pane for
// Shared files and the server's data folder, and above the page otherwise (the
// Gallery, Admin). Each has its own address, so Back and a link name it:
// /files, /files#shared-files, /files#folder, /files#admin.
//
// A classic script in the browser (FilesPlaces on window), a CommonJS module
// under Node, where scripts/tests/files-web-places.test.js drives it.
// ──────────────────────────────────────────────────────────────────────────
(function (root) {
  'use strict';

  const NODE = typeof module === 'object' && module && module.exports;

  const WORDS = Object.freeze({
    onServer: 'On the server',
    sharedOn: (host) => (host ? 'Shared on ' + host : 'Shared on this server'),
    sharedFiles: 'Shared files',
    folder: 'The server\'s data folder',
    admin: 'Admin',
    label: 'Places',
  });

  /**
   * The places listed, in the desktop's order: Admin and the server's data folder for an admin of
   * the server only (the server lists and reads its data folder for its admins alone, BUG-206).
   */
  function listed(admin) {
    return admin ? ['shared', 'shared-files', 'admin', 'folder'] : ['shared', 'shared-files'];
  }
  /** The place an address's hash names (the page's own list for any other). */
  function placeOf(hash) {
    const h = String(hash || '');
    if (h === '#admin') return 'admin';
    if (h === '#shared-files') return 'shared-files';
    if (h === '#folder') return 'folder';
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
  /** A place's name in the list. */
  function labelOf(place, host) {
    if (place === 'shared') return WORDS.sharedOn(host);
    if (place === 'shared-files') return WORDS.sharedFiles;
    if (place === 'folder') return WORDS.folder;
    return WORDS.admin;
  }

  /**
   * The places list and the panes of the places that are not the page's own list.
   * env: { h, host(), isAdmin(), pick(place), MP (files-panes.js, or null), viewport(),
   *        shares(way), setShares(way, shares, settled), places: { 'shared-files': factory, folder: factory } }
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
      const item = (place) => h('button', {
        type: 'button', id: 'files-place-' + place,
        className: 'files-place' + (place === current ? ' files-place-active' : ''),
        text: labelOf(place, host),
        attrs: place === current ? { 'aria-current': 'page' } : {},
        on: { click: () => { if (place !== current && typeof env.pick === 'function') env.pick(place); } },
      });
      el.replaceChildren(h('p', { className: 'files-places-group', text: WORDS.onServer }), ...listed(admin).map(item));
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
      const own = current === 'shared-files' || current === 'folder';
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

    /** Whether files dropped while `current` is shown go to that place (Shared files takes them). */
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

  const api = { WORDS, listed, placeOf, addressOf, shownPlace, labelOf, createPlaces };
  if (NODE) module.exports = api;
  else root.FilesPlaces = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
