/**
 * Donate page logic — fetches funding config, renders source cards
 * dynamically from the flexible addresses array (or legacy sources),
 * queries blockchain balances client-side, animates progress bar.
 */
(function () {
  'use strict';

  const CACHE_KEY = 'hos_donate_cache';
  const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

  // ── State ──
  let fundingConfig = null;
  let totals = { total: 0 };

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

  // ── Progress bar ──

  function updateProgressBar(raised, goal) {
    var section = document.getElementById('progress-section');
    if (!goal || goal <= 0) { section.style.display = 'none'; return; }
    section.style.display = '';

    var pct = Math.min((raised / goal) * 100, 100);
    document.getElementById('progress-raised').textContent = fmtUSD(raised);
    document.getElementById('progress-goal').textContent = fmtUSD(goal);
    document.getElementById('progress-pct').textContent = pct.toFixed(1) + '%';

    requestAnimationFrame(function () {
      document.getElementById('progress-fill').style.width = pct + '%';
    });
  }

  // ── Dynamic address card rendering (new flexible format) ──

  function renderAddressCards(addresses) {
    var grid = document.getElementById('source-grid');
    grid.innerHTML = '';

    addresses.forEach(function (entry, idx) {
      var card = document.createElement('div');
      card.className = 'source-card';

      var abbrev = networkAbbrev(entry.network);
      var color = networkColor(entry.network);
      var label = entry.label || '';
      var value = entry.value || '';
      var hasValue = value && value.length > 0;

      // Icon: colored circle with abbreviation
      var iconHtml = '<span class="source-icon" style="display:inline-flex;align-items:center;justify-content:center;width:32px;height:32px;border-radius:50%;background:' + color + ';color:#fff;font-size:0.7rem;font-weight:700;flex-shrink:0;">' + escHtml(abbrev) + '</span>';

      if (entry.type === 'url') {
        // URL-based source (e.g. GitHub Sponsors)
        card.innerHTML =
          '<h3>' + iconHtml + ' ' + escHtml(entry.network) + '</h3>' +
          '<p>' + escHtml(label) + '</p>' +
          (hasValue
            ? '<a href="' + escHtml(value) + '" target="_blank" rel="noopener" class="btn-sponsor">Open</a>'
            : '<div class="coming-soon">Link coming soon</div>');
      } else {
        // Address-based source (crypto address)
        var qrId = 'qr-addr-' + idx;
        card.innerHTML =
          '<h3>' + iconHtml + ' ' + escHtml(entry.network) + '</h3>' +
          '<p>' + escHtml(label) + '</p>' +
          (hasValue
            ? '<div class="addr-row"><span class="addr-text">' + escHtml(value) + '</span>' +
              '<button class="btn-copy" onclick="window.__donateCopy(\'' + escHtml(value) + '\', this)">Copy</button></div>' +
              '<div class="qr-container" id="' + qrId + '"></div>'
            : '<div class="coming-soon">Address coming soon</div>');
      }

      grid.appendChild(card);
    });

    // Render QR codes after cards are in the DOM
    addresses.forEach(function (entry, idx) {
      if (entry.type === 'address' && entry.value) {
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

  // ── Legacy source card rendering (backward compatible) ──

  function renderSourceCards(sources) {
    var grid = document.getElementById('source-grid');
    grid.innerHTML = '';

    sources.forEach(function (src) {
      var card = document.createElement('div');
      card.className = 'source-card';

      if (src.type === 'github_sponsors') {
        card.innerHTML =
          '<h3><span class="source-icon" style="color:#c678dd;">&#x1F49C;</span> GitHub Sponsors</h3>' +
          '<p>Recurring monthly support for full-time open-source development.</p>' +
          '<div class="fee-tag">0% fees — GitHub covers processing</div>' +
          '<a href="' + escHtml(src.url || 'https://github.com/sponsors/Shaostoul') + '" target="_blank" rel="noopener" class="btn-sponsor">' +
            '&#x2764; Sponsor on GitHub</a>';
      } else if (src.type === 'solana') {
        var solAddr = src.address || '';
        var hasAddr = solAddr && solAddr !== 'Coming soon';
        card.innerHTML =
          '<h3><span class="source-icon">&#x25CE;</span> Solana (SOL / USDC)</h3>' +
          '<p>Near-zero fees. Uses Ed25519 — the same cryptography as HumanityOS identity.</p>' +
          '<div class="fee-tag">~0% fees</div>' +
          (hasAddr
            ? '<div class="addr-row"><span class="addr-text">' + escHtml(solAddr) + '</span>' +
              '<button class="btn-copy" onclick="window.__donateCopy(\'' + escHtml(solAddr) + '\', this)">Copy</button></div>' +
              '<div class="qr-container" id="qr-solana"></div>'
            : '<div class="coming-soon">Address coming soon</div>');
      } else if (src.type === 'bitcoin') {
        var btcAddr = src.address || '';
        var hasBtc = btcAddr && btcAddr !== 'Coming soon';
        card.innerHTML =
          '<h3><span class="source-icon">&#x20BF;</span> Bitcoin</h3>' +
          '<p>Largest crypto network. Ideological reach and universal recognition.</p>' +
          '<div class="fee-tag">Network fee only</div>' +
          (hasBtc
            ? '<div class="addr-row"><span class="addr-text">' + escHtml(btcAddr) + '</span>' +
              '<button class="btn-copy" onclick="window.__donateCopy(\'' + escHtml(btcAddr) + '\', this)">Copy</button></div>' +
              '<div class="qr-container" id="qr-bitcoin"></div>'
            : '<div class="coming-soon">Address coming soon</div>');
      }

      grid.appendChild(card);
    });

    // Render QR codes after cards are in the DOM
    sources.forEach(function (src) {
      if (src.type === 'solana' && src.address && src.address !== 'Coming soon') {
        var qrEl = document.getElementById('qr-solana');
        if (qrEl) renderQR(qrEl, src.address);
      }
      if (src.type === 'bitcoin' && src.address && src.address !== 'Coming soon') {
        var qrEl = document.getElementById('qr-bitcoin');
        if (qrEl) renderQR(qrEl, 'bitcoin:' + src.address);
      }
    });
  }

  // ── Breakdown (works with both formats) ──

  function renderBreakdown(entries) {
    var section = document.getElementById('breakdown-section');
    var rows = document.getElementById('breakdown-rows');
    rows.innerHTML = '';

    entries.forEach(function (entry) {
      // Determine label: new format uses .network, legacy uses type-based labels
      var label = entry.network || entry.label || entry.type || 'Unknown';
      var row = document.createElement('div');
      row.className = 'breakdown-row';
      row.innerHTML =
        '<span class="label">' + escHtml(label) + '</span>' +
        '<span class="value">' + fmtUSD(0) + '</span>';
      rows.appendChild(row);
    });

    // Total row
    var totalRow = document.createElement('div');
    totalRow.className = 'breakdown-row';
    totalRow.innerHTML =
      '<span class="label" style="font-weight:600;color:var(--text);">Total</span>' +
      '<span class="value" style="color:var(--accent);">' + fmtUSD(totals.total) + '</span>';
    rows.appendChild(totalRow);

    section.style.display = '';
  }

  // ── Blockchain balance queries ──

  async function fetchSolanaBalance(address) {
    if (!address) return 0;
    try {
      var resp = await fetch('https://api.mainnet-beta.solana.com', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: '2.0', id: 1,
          method: 'getBalance',
          params: [address]
        })
      });
      var data = await resp.json();
      var lamports = (data.result && data.result.value) || 0;
      var sol = lamports / 1e9;

      var priceResp = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd');
      var priceData = await priceResp.json();
      var solPrice = (priceData.solana && priceData.solana.usd) || 0;

      return sol * solPrice;
    } catch (e) {
      return 0;
    }
  }

  async function fetchBitcoinBalance(address) {
    if (!address) return 0;
    try {
      var resp = await fetch('https://mempool.space/api/address/' + address);
      var data = await resp.json();
      var funded = (data.chain_stats && data.chain_stats.funded_txo_sum) || 0;
      var spent = (data.chain_stats && data.chain_stats.spent_txo_sum) || 0;
      var btc = (funded - spent) / 1e8;

      var priceResp = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd');
      var priceData = await priceResp.json();
      var btcPrice = (priceData.bitcoin && priceData.bitcoin.usd) || 0;

      return btc * btcPrice;
    } catch (e) {
      return 0;
    }
  }

  // ── Cache ──

  function loadCache() {
    try {
      var raw = localStorage.getItem(CACHE_KEY);
      if (!raw) return null;
      var cached = JSON.parse(raw);
      if (Date.now() - cached.timestamp > CACHE_TTL) return null;
      return cached;
    } catch (e) {
      return null;
    }
  }

  function saveCache(data) {
    try {
      localStorage.setItem(CACHE_KEY, JSON.stringify({
        timestamp: Date.now(),
        totals: data
      }));
    } catch (e) { /* quota exceeded or private mode */ }
  }

  // ── Fetch totals (works with both new addresses and legacy sources) ──

  async function fetchTotals(addresses) {
    var cached = loadCache();
    if (cached && cached.totals) {
      totals = cached.totals;
      return;
    }

    var total = 0;

    for (var i = 0; i < addresses.length; i++) {
      var entry = addresses[i];
      var addr = entry.value || entry.address || '';
      if (!addr) continue;

      var netLower = (entry.network || entry.type || '').toLowerCase();
      if (netLower.includes('solana') || entry.type === 'solana') {
        var bal = await fetchSolanaBalance(addr);
        total += bal;
      } else if (netLower.includes('bitcoin') || entry.type === 'bitcoin') {
        var bal = await fetchBitcoinBalance(addr);
        total += bal;
      }
    }

    totals.total = total;
    saveCache(totals);
  }

  // ── Default config ──

  var defaultConfig = {
    goal_usd: 100000,
    goal_label: 'Full-time development for 1 year',
    addresses: [
      { network: 'GitHub Sponsors', type: 'url', value: 'https://github.com/sponsors/Shaostoul', label: 'Recurring or one-time' },
      { network: 'Solana (SOL)', type: 'address', value: '', label: 'Send SOL or SPL tokens' },
      { network: 'Bitcoin (BTC)', type: 'address', value: '', label: 'Send BTC' }
    ],
    display_progress: true
  };

  // ── Init ──

  async function init() {
    try {
      var resp = await fetch('/api/server-info');
      var info = await resp.json();
      fundingConfig = (info && info.funding) ? info.funding : defaultConfig;
    } catch (e) {
      fundingConfig = defaultConfig;
    }

    // Prefer new "addresses" array; fall back to legacy "sources"
    var useNewFormat = Array.isArray(fundingConfig.addresses) && fundingConfig.addresses.length > 0;
    var entries = useNewFormat ? fundingConfig.addresses : fundingConfig.sources;

    if (!entries || entries.length === 0) {
      entries = defaultConfig.addresses;
      useNewFormat = true;
    }

    // Render cards using appropriate renderer
    if (useNewFormat) {
      renderAddressCards(entries);
    } else {
      renderSourceCards(entries);
    }

    // Fetch balances
    await fetchTotals(entries);

    // Progress bar
    if (fundingConfig.display_progress !== false) {
      updateProgressBar(totals.total, fundingConfig.goal_usd || 100000);
      if (fundingConfig.goal_label) {
        document.getElementById('progress-goal-label').textContent = fundingConfig.goal_label;
      }
    }

    // Breakdown
    renderBreakdown(entries);
  }

  // Expose copy function for inline onclick handlers
  window.__donateCopy = copyAddress;

  init();
})();
