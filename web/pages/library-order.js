/* Library shelf order: the web twin of rail_order() in src/gui/pages/library.rs.

   The Library rail lists its shelves A to Z by default (operator, 2026-10-10:
   "it's hard to search through while not alphabetical"). The other choice,
   Suggested, is the catalog's own order: the learning path, for someone walking
   it rung by rung. Sections (Learn, The Accord, ...) keep the manifest's order
   either way.

   A to Z sorts the shelves within each section by name and the documents on
   each shelf by title, ignoring case and a leading "The ", "A " or "An " (for
   sorting only: titles are shown as written). Ties go to the whole title
   lowercased, then to the catalog position. Strings are compared as plain
   strings, never with localeCompare, so this orders the manifest exactly as the
   desktop app does.

   Loaded by library.html before library-app.js, and required by Node, where
   scripts/tests/library-order-web.test.js holds it to the native rule. */
(function(root) {
  'use strict';

  var ARTICLES = ['the ', 'a ', 'an '];

  /** Hover notes on the two order chips. The desktop app shows the same words
      (ORDER_TIP_AZ, ORDER_TIP_SUGGESTED in src/gui/pages/library.rs). */
  var ORDER_TIP_AZ = 'Every shelf and every document by title, ignoring a leading The, A or An.';
  var ORDER_TIP_SUGGESTED =
    'The order the guides are meant to be read in, each one building on the one before.';

  /** Where the choice is kept in this browser: 'az' (the default) or 'suggested'. */
  var ORDER_STORAGE_KEY = 'hos_library_order';

  function titleSortKey(title) {
    var t = String(title == null ? '' : title).trim().toLowerCase();
    for (var i = 0; i < ARTICLES.length; i++) {
      if (t.indexOf(ARTICLES[i]) === 0) return t.slice(ARTICLES[i].length).replace(/^\s+/, '');
    }
    return t;
  }

  function cmp(a, b) { return a < b ? -1 : (a > b ? 1 : 0); }

  /** Indices of `titles` in display order: as given when `suggested`, else A to Z. */
  function orderByTitle(titles, suggested) {
    var idx = titles.map(function(_, i) { return i; });
    if (suggested) return idx;
    var keys = titles.map(function(t) {
      return { key: titleSortKey(t), whole: String(t == null ? '' : t).trim().toLowerCase() };
    });
    return idx.sort(function(a, b) {
      return cmp(keys[a].key, keys[b].key) || cmp(keys[a].whole, keys[b].whole) || (a - b);
    });
  }

  /**
   * The rail's order for a manifest (data/library/index.json): one group per
   * section, in the manifest's declared order (then any section a category names
   * that the list forgot), each holding its shelves as { ci, dis }: indices into
   * manifest.categories and that category's docs. Indices, never copies, so
   * openDoc(ci, di), the folds and the Next footer keep addressing the catalog
   * as loaded. A manifest with no sections at all is one group named null.
   */
  function railOrder(manifest, suggested) {
    var cats = (manifest && manifest.categories) || [];
    var order = ((manifest && manifest.sections) || []).slice();
    cats.forEach(function(c) {
      if (c.section && order.indexOf(c.section) < 0) order.push(c.section);
    });
    if (!order.length) order = [null];
    return order.map(function(name) {
      var mine = [];
      cats.forEach(function(c, ci) {
        if (name === null || c.section === name) mine.push(ci);
      });
      var shelfOrder = orderByTitle(mine.map(function(ci) { return cats[ci].name; }), suggested);
      return {
        section: name,
        cats: shelfOrder.map(function(k) {
          var ci = mine[k];
          var docs = cats[ci].docs || [];
          return { ci: ci, dis: orderByTitle(docs.map(function(d) { return d.title; }), suggested) };
        }),
      };
    });
  }

  var api = {
    ORDER_TIP_AZ: ORDER_TIP_AZ,
    ORDER_TIP_SUGGESTED: ORDER_TIP_SUGGESTED,
    ORDER_STORAGE_KEY: ORDER_STORAGE_KEY,
    titleSortKey: titleSortKey,
    orderByTitle: orderByTitle,
    railOrder: railOrder,
  };
  if (typeof module === 'object' && module && module.exports) module.exports = api;
  else root.hosLibraryOrder = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
