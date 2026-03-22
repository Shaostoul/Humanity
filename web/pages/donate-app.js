/**
 * Donate page logic — fetches funding config, renders source cards,
 * queries blockchain balances client-side, animates progress bar.
 */
(function () {
  'use strict';

  const CACHE_KEY = 'hos_donate_cache';
  const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

  // ── State ──
  let fundingConfig = null;
  let totals = { github: 0, solana: 0, bitcoin: 0, total: 0 };

  // ── Helpers ──

  /** Format USD amount with commas */
  function fmtUSD(n) {
    return '$' + Math.round(n).toLocaleString('en-US');
  }

  /** Copy text to clipboard, show "Copied!" feedback on button */
  function copyAddress(addr, btnEl) {
    if (!addr || addr === 'Coming soon') return;
    navigator.clipboard.writeText(addr).then(function () {
      btnEl.textContent = 'Copied!';
      btnEl.classList.add('copied');
      setTimeout(function () {
        btnEl.textContent = 'Copy';
        btnEl.classList.remove('copied');
      }, 2000);
    }).catch(function () {
      // Fallback for insecure contexts
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
    if (!text || text === 'Coming soon' || typeof qrcode === 'undefined') return;
    try {
      var qr = qrcode(0, 'M');
      qr.addData(text);
      qr.make();
      container.innerHTML = qr.createSvgTag(3, 2);
      // Style the SVG for dark/light theme
      var svg = container.querySelector('svg');
      if (svg) {
        svg.style.width = '120px';
        svg.style.height = '120px';
        svg.style.background = '#fff';
        svg.style.padding = '6px';
        svg.style.borderRadius = '6px';
      }
    } catch (e) {
      // QR generation failed — just show text address
    }
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

    // Animate fill after a brief delay so the transition is visible
    requestAnimationFrame(function () {
      document.getElementById('progress-fill').style.width = pct + '%';
    });
  }

  // ── Source card rendering ──

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

  /** Minimal HTML escape */
  function escHtml(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }

  // ── Breakdown ──

  function renderBreakdown(sources) {
    var section = document.getElementById('breakdown-section');
    var rows = document.getElementById('breakdown-rows');
    rows.innerHTML = '';

    var labels = {
      github_sponsors: 'GitHub Sponsors',
      solana: 'Solana wallet',
      bitcoin: 'Bitcoin wallet'
    };

    var hasData = false;
    sources.forEach(function (src) {
      var amt = totals[src.type === 'github_sponsors' ? 'github' : src.type] || 0;
      var row = document.createElement('div');
      row.className = 'breakdown-row';
      row.innerHTML =
        '<span class="label">' + (labels[src.type] || src.type) + '</span>' +
        '<span class="value">' + fmtUSD(amt) + '</span>';
      rows.appendChild(row);
      if (amt > 0) hasData = true;
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

  // ── Blockchain balance queries (placeholders) ──

  /**
   * Fetch Solana wallet balance in USD.
   * Uses public Solana RPC + CoinGecko for price conversion.
   * Returns 0 for now until addresses are configured.
   */
  async function fetchSolanaBalance(address) {
    if (!address || address === 'Coming soon') return 0;
    try {
      // Query SOL balance via public RPC
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

      // Get SOL price in USD
      var priceResp = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd');
      var priceData = await priceResp.json();
      var solPrice = (priceData.solana && priceData.solana.usd) || 0;

      return sol * solPrice;
    } catch (e) {
      return 0;
    }
  }

  /**
   * Fetch Bitcoin wallet balance in USD.
   * Uses mempool.space public API + CoinGecko for price.
   * Returns 0 for now until addresses are configured.
   */
  async function fetchBitcoinBalance(address) {
    if (!address || address === 'Coming soon') return 0;
    try {
      var resp = await fetch('https://mempool.space/api/address/' + address);
      var data = await resp.json();
      // chain_stats.funded_txo_sum - chain_stats.spent_txo_sum = balance in sats
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

  // ── Fetch totals ──

  async function fetchTotals(sources) {
    // Try cache first
    var cached = loadCache();
    if (cached && cached.totals) {
      totals = cached.totals;
      return;
    }

    var solSource = sources.find(function (s) { return s.type === 'solana'; });
    var btcSource = sources.find(function (s) { return s.type === 'bitcoin'; });

    var solBal = solSource ? await fetchSolanaBalance(solSource.address) : 0;
    var btcBal = btcSource ? await fetchBitcoinBalance(btcSource.address) : 0;

    totals.solana = solBal;
    totals.bitcoin = btcBal;
    totals.github = 0; // No public API — manual entry in config
    totals.total = solBal + btcBal + totals.github;

    saveCache(totals);
  }

  // ── Default config (used when server-info has no funding block) ──

  var defaultConfig = {
    goal_usd: 100000,
    goal_label: 'Full-time development for 1 year',
    sources: [
      { type: 'github_sponsors', url: 'https://github.com/sponsors/Shaostoul' },
      { type: 'solana', address: 'Coming soon' },
      { type: 'bitcoin', address: 'Coming soon' }
    ],
    display_progress: true
  };

  // ── Init ──

  async function init() {
    // Fetch server-info for funding config
    try {
      var resp = await fetch('/api/server-info');
      var info = await resp.json();
      fundingConfig = (info && info.funding) ? info.funding : defaultConfig;
    } catch (e) {
      fundingConfig = defaultConfig;
    }

    var sources = fundingConfig.sources || defaultConfig.sources;

    // Render cards immediately (addresses might be "Coming soon")
    renderSourceCards(sources);

    // Fetch balances and update
    await fetchTotals(sources);

    // Progress bar
    if (fundingConfig.display_progress !== false) {
      updateProgressBar(totals.total, fundingConfig.goal_usd || 100000);
      if (fundingConfig.goal_label) {
        document.getElementById('progress-goal-label').textContent = fundingConfig.goal_label;
      }
    }

    // Breakdown
    renderBreakdown(sources);
  }

  // Expose copy function for inline onclick handlers
  window.__donateCopy = copyAddress;

  init();
})();
