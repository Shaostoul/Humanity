/**
 * Donate page logic. Mirrors the desktop app's Donate page
 * (src/gui/pages/donate.rs) and reads the same data files, which
 * tests/page_parity_lint.rs checks on both sides:
 *
 *   /data/donate/routes.json     the ways to give, shown first: the nonprofit
 *                                Sponsor-a-Can (tax-deductible) and the
 *                                maintainer on Patreon (not), each with one
 *                                plain sentence saying where the money goes
 *   /data/donate/methods.json    more direct links to the maintainer
 *   /data/donate/charities.json  charities the maintainer endorses
 *   /data/donate/faq.json        the FAQ
 *
 * plus the connected server's funding block from /api/server-info (its goal
 * and any addresses it lists). Entries with no link or address are not shown,
 * the same as native: a "coming soon" card is a dead end for someone trying to
 * give. There is no "raised so far" figure, because nothing tracks one; the
 * goal is shown on its own, as native does.
 */
(function () {
  'use strict';

  // ── Network icon colors for the colored-circle abbreviation display ──
  var networkColors = {
    'github sponsors': '#c678dd',
    'solana (sol)':    '#9945ff',
    'bitcoin (btc)':   '#f7931a',
    'ethereum (eth)':  '#627eea',
    'monero (xmr)':    '#ff6600',
    'litecoin (ltc)':  '#bfbbbb',
    'polygon (matic)': '#8247e5',
    'avalanche (avax)':'#e84142',
    'cardano (ada)':   '#0033ad',
    'dogecoin (doge)': '#c2a633'
  };

  /** Extract a short abbreviation from network name, e.g. "Solana (SOL)" -> "SOL" */
  function networkAbbrev(name) {
    var match = name.match(/\(([^)]+)\)/);
    if (match) return match[1];
    // Fallback: first 3 chars uppercase
    return name.replace(/[^a-zA-Z]/g, '').substring(0, 3).toUpperCase();
  }

  /** Get icon color for a network name (case-insensitive lookup, fallback to accent) */
  function networkColor(name) {
    return networkColors[name.toLowerCase()] || '#4a9';
  }

  // ── Helpers ──

  /** Format USD amount with commas */
  function fmtUSD(n) {
    return '$' + Math.round(n).toLocaleString('en-US');
  }

  /** Copy text to clipboard, show "Copied!" feedback on button */
  function copyAddress(addr, btnEl) {
    if (!addr) return;
    navigator.clipboard.writeText(addr).then(function () {
      btnEl.textContent = 'Copied!';
      btnEl.classList.add('copied');
      setTimeout(function () {
        btnEl.textContent = 'Copy';
        btnEl.classList.remove('copied');
      }, 2000);
    }).catch(function () {
      var ta = document.createElement('textarea');
      ta.value = addr;
      ta.style.position = 'fixed';
      ta.style.left = '-9999px';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
      btnEl.textContent = 'Copied!';
      btnEl.classList.add('copied');
      setTimeout(function () {
        btnEl.textContent = 'Copy';
        btnEl.classList.remove('copied');
      }, 2000);
    });
  }

  /** Generate a QR code SVG for the given text, append to container */
  function renderQR(container, text) {
    if (!text || typeof qrcode === 'undefined') return;
    try {
      var qr = qrcode(0, 'M');
      qr.addData(text);
      qr.make();
      container.innerHTML = qr.createSvgTag(3, 2);
      var svg = container.querySelector('svg');
      if (svg) {
        svg.style.width = '120px';
        svg.style.height = '120px';
        svg.style.background = '#fff';
        svg.style.padding = '6px';
        svg.style.borderRadius = '6px';
      }
    } catch (e) {
      // QR generation failed
    }
  }

  /** Minimal HTML escape */
  function escHtml(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }

  /** Colored circle with a short abbreviation, the icon every card uses. */
  function iconHtml(abbrev, color) {
    return '<span class="source-icon" style="display:inline-flex;align-items:center;justify-content:center;width:32px;height:32px;border-radius:50%;background:' + escHtml(color) + ';color:#fff;font-size:0.7rem;font-weight:700;flex-shrink:0;">' + escHtml(abbrev) + '</span>';
  }

  /** Fetch one data file and return the named array (empty on any failure). */
  async function loadList(path, key) {
    try {
      var resp = await fetch(path, { cache: 'no-cache' });
      var json = await resp.json();
      return (json && Array.isArray(json[key])) ? json[key] : [];
    } catch (e) {
      return [];
    }
  }

  /** Compare links loosely: case, a trailing slash and http/https do not make two different links. */
  function sameLink(a, b) {
    function norm(u) { return String(u || '').trim().toLowerCase().replace(/^https?:\/\//, '').replace(/\/+$/, ''); }
    return norm(a) !== '' && norm(a) === norm(b);
  }

  // ── Ways to give (data/donate/routes.json) ──

  function renderRoutes(routes) {
    var section = document.getElementById('routes-section');
    var grid = document.getElementById('routes-grid');
    grid.innerHTML = '';
    routes.forEach(function (r) {
      if (!r || !r.name || !r.url) return;
      var card = document.createElement('div');
      card.className = 'route-card';
      var abbrev = r.abbrev || networkAbbrev(r.name);
      var color = r.color || networkColor(r.name);
      card.innerHTML =
        '<h3>' + iconHtml(abbrev, color) + ' ' + escHtml(r.name) + '</h3>' +
        (r.kind ? '<div class="route-kind">' + escHtml(r.kind) + '</div>' : '') +
        '<div class="tax-badge' + (r.tax_deductible ? ' yes' : '') + '">' +
          (r.tax_deductible ? 'Tax-deductible' : 'Not tax-deductible') + '</div>' +
        (r.about ? '<p class="route-about">' + escHtml(r.about) + '</p>' : '') +
        '<p class="route-goes">' + escHtml(r.goes_to || '') + '</p>' +
        (r.note ? '<div class="route-note">' + escHtml(r.note) + '</div>' : '') +
        '<div class="route-action"><a href="' + escHtml(r.url) + '" target="_blank" rel="noopener" class="btn-sponsor">' + escHtml(r.button || 'Open') + '</a>' +
        '<span class="route-url">' + escHtml(r.url) + '</span></div>';
      grid.appendChild(card);
    });
    section.style.display = grid.children.length ? '' : 'none';
  }

  // ── More direct links + the server's addresses ──

  function renderAddressCards(addresses) {
    var grid = document.getElementById('source-grid');
    grid.innerHTML = '';

    addresses.forEach(function (entry, idx) {
      var card = document.createElement('div');
      card.className = 'source-card';

      var abbrev = entry.abbrev || networkAbbrev(entry.network);
      var color = entry.color || networkColor(entry.network);
      var label = entry.label || '';
      var value = entry.value;

      if (entry.type === 'url') {
        card.innerHTML =
          '<h3>' + iconHtml(abbrev, color) + ' ' + escHtml(entry.network) + '</h3>' +
          '<p>' + escHtml(label) + '</p>' +
          '<a href="' + escHtml(value) + '" target="_blank" rel="noopener" class="btn-sponsor">Open</a>';
      } else {
        // Address-based source (crypto address)
        card.innerHTML =
          '<h3>' + iconHtml(abbrev, color) + ' ' + escHtml(entry.network) + '</h3>' +
          '<p>' + escHtml(label) + '</p>' +
          '<div class="addr-row"><span class="addr-text">' + escHtml(value) + '</span>' +
          '<button class="btn-copy" onclick="window.__donateCopy(\'' + escHtml(value) + '\', this)">Copy</button></div>' +
          '<div class="qr-container" id="qr-addr-' + idx + '"></div>';
      }

      grid.appendChild(card);
    });

    // Render QR codes after cards are in the DOM
    addresses.forEach(function (entry, idx) {
      if (entry.type === 'address') {
        var qrEl = document.getElementById('qr-addr-' + idx);
        if (qrEl) {
          // Prefix with protocol for Bitcoin-style URI
          var qrText = entry.value;
          var netLower = entry.network.toLowerCase();
          if (netLower.includes('bitcoin')) qrText = 'bitcoin:' + entry.value;
          else if (netLower.includes('ethereum')) qrText = 'ethereum:' + entry.value;
          renderQR(qrEl, qrText);
        }
      }
    });
  }

  /**
   * The server's goal, shown as an amount and its label with no progress bar,
   * the same as native. Until 2026-10-04 this page drew "$0 raised" against the
   * goal, and $0 beside every source, because the only thing it could total was
   * crypto balances and no address is configured; native had already dropped
   * that figure as made up.
   */
  function renderGoal(funding) {
    var section = document.getElementById('goal-section');
    var goal = funding && Number(funding.goal_usd);
    if (!goal || goal <= 0) { section.style.display = 'none'; return false; }
    document.getElementById('goal-amount').textContent = fmtUSD(goal);
    document.getElementById('goal-label').textContent = funding.goal_label || '';
    section.style.display = '';
    return true;
  }

  // ── Charities the maintainer endorses (data/donate/charities.json) ──
  // Independent nonprofits, not HumanityOS funding. Rendered into their own grid.

  function renderCharities(list) {
    var section = document.getElementById('charities-section');
    var grid = document.getElementById('charities-grid');
    grid.innerHTML = '';
    list.forEach(function (c) {
      if (!c || !c.name) return;
      var abbrev = c.abbrev || c.name.replace(/[^a-zA-Z]/g, '').substring(0, 3).toUpperCase();
      var card = document.createElement('div');
      card.className = 'source-card';
      card.innerHTML =
        '<h3>' + iconHtml(abbrev, c.color || '#4a9') + ' ' + escHtml(c.name) + '</h3>' +
        '<p>' + escHtml(c.mission || '') + '</p>' +
        (c.note ? '<p>' + escHtml(c.note) + '</p>' : '') +
        (c.url ? '<a href="' + escHtml(c.url) + '" target="_blank" rel="noopener" class="btn-sponsor">Donate</a>' : '');
      grid.appendChild(card);
    });
    section.style.display = grid.children.length ? '' : 'none';
  }

  // ── FAQ (data/donate/faq.json) ──

  function renderFaq(entries) {
    var section = document.getElementById('faq-section');
    var list = document.getElementById('faq-list');
    list.innerHTML = '';
    entries.forEach(function (e, i) {
      if (!e || !e.question) return;
      var item = document.createElement('div');
      item.className = 'faq-item' + (i === 0 ? ' open' : '');
      item.innerHTML =
        '<div class="faq-q">' + escHtml(e.question) + '</div>' +
        '<div class="faq-a">' + escHtml(e.answer || '') + '</div>';
      item.querySelector('.faq-q').addEventListener('click', function () { item.classList.toggle('open'); });
      list.appendChild(item);
    });
    section.style.display = list.children.length ? '' : 'none';
  }

  // ── Init ──

  async function init() {
    var funding = null;
    try {
      var resp = await fetch('/api/server-info');
      var info = await resp.json();
      funding = (info && info.funding) ? info.funding : null;
    } catch (e) { /* offline or no relay: the data files below still render */ }

    var routes = await loadList('/data/donate/routes.json', 'routes');
    renderRoutes(routes);

    // More direct links first (data/donate/methods.json), then the server's
    // addresses, de-duped by network name and by link, and never repeating a
    // route that already has its own card above. Native's build_donation_sources
    // applies the same three rules.
    var entries = [];
    var seenNetwork = {};
    function add(e) {
      if (!e || !e.network || !e.value) return;
      if (seenNetwork[e.network.toLowerCase()]) return;
      if (routes.some(function (r) { return sameLink(r.url, e.value); })) return;
      if (entries.some(function (x) { return sameLink(x.value, e.value); })) return;
      seenNetwork[e.network.toLowerCase()] = true;
      entries.push(e);
    }
    (await loadList('/data/donate/methods.json', 'methods')).forEach(add);
    if (funding && Array.isArray(funding.addresses)) funding.addresses.forEach(add);
    renderAddressCards(entries);
    var hasGoal = renderGoal(funding);
    document.getElementById('direct-section').style.display = (entries.length || hasGoal) ? '' : 'none';

    renderCharities(await loadList('/data/donate/charities.json', 'charities'));
    renderFaq(await loadList('/data/donate/faq.json', 'entries'));
  }

  // Expose copy function for inline onclick handlers
  window.__donateCopy = copyAddress;

  init();
})();
