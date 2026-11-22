/**
 * The Donate page's "This month" card (2026-11-22; docs/design/live-funding.md): what the connected
 * server's donations bring in each month, channel by channel, against what running the project
 * costs, each figure with the date it was last confirmed, and one plain sentence ("Donations cover
 * $20 of $241 a month."). The mirror of src/gui/pages/donate_figures.rs: the same sentence, the
 * same bounds, nothing drawn when a server publishes no figures.
 *
 * The figures come from /api/server-info's funding.channels and funding.costs, set by the
 * server's owner in Server Settings (the relay holds them to their shape, storage/funding_figures.rs;
 * this reader holds them to it again, so a rogue server cannot draw a thousand rows or a link that
 * is not https). Words come from data/i18n/en.json by key through i18n.t; DOM is built with
 * createElement and textContent, never innerHTML.
 */
(function (root) {
  'use strict';

  var MAX_ROWS = 32, MAX_NAME = 80, MAX_LINK = 300, MAX_USD = 1000000;

  function okAmount(v) { return typeof v === 'number' && isFinite(v) && v >= 0 && v <= MAX_USD; }
  function okName(s) { return typeof s === 'string' && s.trim().length >= 1 && s.trim().length <= MAX_NAME; }
  function okLink(s) {
    if (s === undefined || s === null || s === '') return true;
    return typeof s === 'string' && s.length <= MAX_LINK && /^https:\/\/\S+$/.test(s);
  }

  /** A dollar amount as a person reads it: 20, 12.50 (whole dollars without cents). */
  function money(v) { return v === Math.floor(v) ? String(v) : v.toFixed(2); }

  /** The figures a server's funding object carries, rows not shaped right skipped. */
  function read(funding) {
    var out = { channels: [], costs: [] };
    if (!funding || typeof funding !== 'object') return out;
    (Array.isArray(funding.channels) ? funding.channels : []).forEach(function (c) {
      if (c && okName(c.name) && okLink(c.link) && okAmount(c.monthly_usd) && out.channels.length < MAX_ROWS) {
        out.channels.push({ name: c.name, link: c.link || '', monthly_usd: c.monthly_usd, as_of: typeof c.as_of === 'string' ? c.as_of : '' });
      }
    });
    (Array.isArray(funding.costs) ? funding.costs : []).forEach(function (c) {
      if (c && okName(c.name) && okAmount(c.monthly_usd) && out.costs.length < MAX_ROWS) {
        out.costs.push({ name: c.name, monthly_usd: c.monthly_usd, as_of: typeof c.as_of === 'string' ? c.as_of : '' });
      }
    });
    return out;
  }

  function total(rows) { return rows.reduce(function (s, r) { return s + r.monthly_usd; }, 0); }

  /** The one sentence: what donations cover of what the project costs. */
  function sentence(f, t) {
    var given = total(f.channels), cost = total(f.costs);
    if (cost <= 0) return t('donate.figures_brings', { given: money(given) });
    if (given >= cost) return t('donate.figures_covered_all', { cost: money(cost) });
    return t('donate.figures_covered', { given: money(given), cost: money(cost) });
  }

  function node(doc, tag, cls, text) {
    var e = doc.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function row(doc, t, r, withLink) {
    var line = node(doc, 'div', 'figures-row');
    line.appendChild(node(doc, 'span', 'figures-name', r.name));
    line.appendChild(node(doc, 'span', 'figures-amount', t('donate.figures_per_month', { amount: money(r.monthly_usd) })));
    if (r.as_of) line.appendChild(node(doc, 'span', 'figures-asof', t('donate.figures_as_of', { date: r.as_of })));
    if (withLink && r.link) {
      var a = node(doc, 'a', 'figures-open', t('donate.open'));
      a.setAttribute('href', r.link);
      a.setAttribute('target', '_blank');
      a.setAttribute('rel', 'noopener noreferrer');
      a.setAttribute('data-tip-desc', t('donate.figures_open_hint'));
      line.appendChild(a);
    }
    return line;
  }

  /**
   * Draw the card into `section` (an element whose children this replaces), or hide it when the
   * server publishes no figures. Returns whether anything is shown.
   */
  function render(section, funding, t, doc) {
    var f = read(funding);
    while (section.firstChild) section.removeChild(section.firstChild);
    if (!f.channels.length && !f.costs.length) { section.style.display = 'none'; return false; }
    section.appendChild(node(doc, 'h3', 'figures-title', t('donate.figures_title')));
    section.appendChild(node(doc, 'p', 'figures-sentence', sentence(f, t)));
    if (f.channels.length) {
      section.appendChild(node(doc, 'div', 'figures-heading', t('donate.figures_channels')));
      f.channels.forEach(function (c) { section.appendChild(row(doc, t, c, true)); });
    }
    if (f.costs.length) {
      section.appendChild(node(doc, 'div', 'figures-heading', t('donate.figures_costs')));
      f.costs.forEach(function (c) { section.appendChild(row(doc, t, c, false)); });
    }
    section.appendChild(node(doc, 'p', 'figures-note', t('donate.figures_note')));
    section.style.display = '';
    return true;
  }

  var api = { read: read, sentence: sentence, render: render, money: money };
  root.DonateFigures = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
