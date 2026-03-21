/**
 * Wallet Dashboard — page logic for wallet.html
 *
 * Depends on window.HosWallet (from wallet.js) for all blockchain calls.
 * Identity comes from localStorage (humanity_pubkey) or window.myIdentity.
 */
(function () {
  'use strict';

  // ── State ──

  let solAddress = '';
  let publicKeyHex = '';
  let currentTab = 'overview';
  let slippage = 0.5; // percent
  let cachedBalances = null;
  let balanceCacheTime = 0;
  const BALANCE_CACHE_MS = 30000;
  let cachedPrice = null;
  let priceCacheTime = 0;
  const PRICE_CACHE_MS = 60000;
  let validators = [];
  let stakeAccounts = [];
  let ownedNFTs = [];
  let selectedNFT = null;

  // Token mints (Solana mainnet)
  const USDC_MINT = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
  const USDT_MINT = 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB';

  // ── Initialization ──

  document.addEventListener('DOMContentLoaded', init);

  function init() {
    publicKeyHex = localStorage.getItem('humanity_pubkey') || localStorage.getItem('humanity_key') || '';
    if (!publicKeyHex && window.myIdentity) {
      publicKeyHex = window.myIdentity.publicKeyHex || '';
    }

    if (!publicKeyHex) {
      document.getElementById('wallet-no-identity').style.display = '';
      document.getElementById('wallet-main').style.display = 'none';
      return;
    }

    // Check that HosWallet is available
    if (!window.HosWallet) {
      console.warn('wallet.js not loaded — HosWallet unavailable');
      document.getElementById('wallet-no-identity').style.display = '';
      document.getElementById('wallet-no-identity').querySelector('h2').textContent = 'Wallet module loading...';
      document.getElementById('wallet-no-identity').querySelector('p').innerHTML =
        'The wallet module (wallet.js) is not yet available. Please try refreshing the page.';
      document.getElementById('wallet-main').style.display = 'none';
      return;
    }

    // Derive Solana address
    try {
      solAddress = HosWallet.publicKeyToSolanaAddress(publicKeyHex);
    } catch (e) {
      console.error('Failed to derive Solana address:', e);
      solAddress = '';
    }

    if (!solAddress) {
      document.getElementById('wallet-no-identity').style.display = '';
      document.getElementById('wallet-main').style.display = 'none';
      return;
    }

    document.getElementById('wallet-no-identity').style.display = 'none';
    document.getElementById('wallet-main').style.display = '';

    // Display address
    document.getElementById('sol-address').textContent = solAddress;
    document.getElementById('receive-address').textContent = solAddress;

    // Generate QR codes
    generateQR('address-qr', solAddress);
    generateQR('receive-qr', solAddress);

    // Load data
    loadBalances();
    loadTransactions();
  }

  // ── Tab Management ──

  window.switchTab = function (tab) {
    currentTab = tab;
    document.querySelectorAll('.wallet-tab').forEach(function (btn) {
      btn.classList.toggle('active', btn.dataset.tab === tab);
    });
    document.querySelectorAll('.tab-panel').forEach(function (panel) {
      panel.classList.toggle('active', panel.id === 'tab-' + tab);
    });

    // Lazy-load tab data
    if (tab === 'stake') {
      loadValidators();
      loadStakeAccounts();
    } else if (tab === 'nfts') {
      loadNFTs();
    } else if (tab === 'overview') {
      loadBalances();
    }
  };

  // ── Warning Banner ──

  window.dismissWarning = function () {
    var el = document.getElementById('wallet-warning');
    if (el) el.style.display = 'none';
  };

  // ── Address & QR ──

  window.copyAddress = function () {
    if (!solAddress) return;
    navigator.clipboard.writeText(solAddress).then(function () {
      // Show feedback on all copy buttons
      document.querySelectorAll('.copy-btn').forEach(function (btn) {
        var orig = btn.textContent;
        btn.textContent = 'Copied!';
        btn.style.color = 'var(--success)';
        btn.style.borderColor = 'var(--success)';
        setTimeout(function () {
          btn.textContent = orig;
          btn.style.color = '';
          btn.style.borderColor = '';
        }, 1500);
      });
    }).catch(function () {
      // Fallback: select text
      var el = document.getElementById('sol-address');
      if (el) {
        var range = document.createRange();
        range.selectNodeContents(el);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
      }
    });
  };

  function generateQR(containerId, text) {
    var container = document.getElementById(containerId);
    if (!container) return;
    container.innerHTML = '';

    // Use qrcode.js if available
    if (window.QRCode) {
      try {
        new QRCode(container, {
          text: text,
          width: 160,
          height: 160,
          colorDark: '#000000',
          colorLight: '#ffffff',
          correctLevel: QRCode.CorrectLevel.M
        });
        return;
      } catch (e) {
        console.warn('QRCode render failed:', e);
      }
    }

    // Fallback: just show the address text
    container.innerHTML = '<div style="font-size:0.8rem;color:var(--text-muted);padding:var(--space-lg);border:1px dashed var(--border);border-radius:8px;">QR code unavailable — share the address above</div>';
  }

  // ── Price Fetching ──

  async function getSOLPrice() {
    var now = Date.now();
    if (cachedPrice && now - priceCacheTime < PRICE_CACHE_MS) {
      return cachedPrice;
    }
    try {
      if (HosWallet.getSOLPrice) {
        cachedPrice = await HosWallet.getSOLPrice();
      } else {
        // Fallback: CoinGecko
        var resp = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd');
        var data = await resp.json();
        cachedPrice = data.solana.usd;
      }
      priceCacheTime = now;
    } catch (e) {
      console.warn('Failed to fetch SOL price:', e);
      cachedPrice = cachedPrice || 0;
    }
    return cachedPrice;
  }

  // ── Balances ──

  async function loadBalances() {
    var now = Date.now();
    if (cachedBalances && now - balanceCacheTime < BALANCE_CACHE_MS) {
      renderBalances(cachedBalances);
      return;
    }

    try {
      var [solBal, usdcBal, solPrice] = await Promise.all([
        HosWallet.getBalance(solAddress),
        HosWallet.getTokenBalance ? HosWallet.getTokenBalance(solAddress, USDC_MINT).catch(function () { return 0; }) : Promise.resolve(0),
        getSOLPrice()
      ]);

      cachedBalances = { sol: solBal, usdc: usdcBal, solPrice: solPrice };
      balanceCacheTime = now;
      renderBalances(cachedBalances);
    } catch (e) {
      console.error('Failed to load balances:', e);
      document.getElementById('balance-sol').textContent = 'Error loading balance';
    }
  }

  function renderBalances(b) {
    var solDisplay = formatAmount(b.sol, 4);
    var solUsd = b.sol * b.solPrice;
    var totalUsd = solUsd + b.usdc;

    document.getElementById('balance-sol').textContent = solDisplay + ' SOL';
    document.getElementById('balance-sol-usd').textContent = '$' + formatUSD(solUsd);
    document.getElementById('balance-usdc').textContent = '$' + formatUSD(b.usdc);
    document.getElementById('portfolio-total').textContent = '$' + formatUSD(totalUsd);
  }

  // ── Transaction History ──

  async function loadTransactions() {
    var loadingEl = document.getElementById('tx-loading');
    var listEl = document.getElementById('tx-list');
    var emptyEl = document.getElementById('tx-empty');

    try {
      var txs = [];
      if (HosWallet.getTransactions) {
        txs = await HosWallet.getTransactions(solAddress, 10);
      }
      loadingEl.style.display = 'none';
      renderTransactionList(txs);
    } catch (e) {
      console.error('Failed to load transactions:', e);
      loadingEl.innerHTML = '<span style="color:var(--danger)">Failed to load transactions.</span>';
    }
  }

  function renderTransactionList(txs) {
    var listEl = document.getElementById('tx-list');
    var emptyEl = document.getElementById('tx-empty');

    if (!txs || txs.length === 0) {
      listEl.innerHTML = '';
      emptyEl.style.display = '';
      return;
    }
    emptyEl.style.display = 'none';

    listEl.innerHTML = txs.map(function (tx) {
      var isSent = tx.from === solAddress || tx.direction === 'sent';
      var dirClass = isSent ? 'sent' : 'received';
      var dirLabel = isSent ? 'Sent' : 'Recv';
      var addr = isSent ? (tx.to || '—') : (tx.from || '—');
      var shortAddr = addr.length > 12 ? addr.slice(0, 6) + '...' + addr.slice(-4) : addr;
      var amount = tx.amount != null ? formatAmount(tx.amount, 4) : '—';
      var token = tx.token || 'SOL';
      var time = tx.timestamp ? formatTime(tx.timestamp) : '';
      var sig = tx.signature || tx.hash || '';
      var link = sig ? '<a class="tx-link" href="https://solscan.io/tx/' + sig + '" target="_blank" rel="noopener">View</a>' : '';

      return '<li class="tx-item">' +
        '<span class="tx-dir ' + dirClass + '">' + dirLabel + '</span>' +
        '<span class="tx-addr" title="' + escapeHtml(addr) + '">' + escapeHtml(shortAddr) + '</span>' +
        '<span class="tx-amount">' + amount + ' ' + token + '</span>' +
        '<span class="tx-time">' + time + '</span>' +
        link +
        '</li>';
    }).join('');
  }

  // ── Send ──

  window.pasteAddress = async function () {
    try {
      var text = await navigator.clipboard.readText();
      document.getElementById('send-to').value = text.trim();
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  };

  window.fillMaxBalance = function () {
    if (!cachedBalances) return;
    var token = document.getElementById('send-token').value;
    if (token === 'SOL') {
      // Reserve ~0.005 SOL for fees
      var max = Math.max(0, cachedBalances.sol - 0.005);
      document.getElementById('send-amount').value = formatAmount(max, 9);
    } else if (token === 'USDC') {
      document.getElementById('send-amount').value = formatAmount(cachedBalances.usdc, 6);
    }
    updateSendEstimate();
  };

  window.updateSendEstimate = function () {
    var amount = parseFloat(document.getElementById('send-amount').value) || 0;
    var token = document.getElementById('send-token').value;
    var el = document.getElementById('send-estimate');

    if (!amount || !cachedPrice) {
      el.textContent = '';
      return;
    }

    if (token === 'SOL') {
      el.textContent = '~$' + formatUSD(amount * cachedPrice);
    } else if (token === 'USDC') {
      el.textContent = '~$' + formatUSD(amount);
    }
  };

  window.prepareSend = async function () {
    var to = document.getElementById('send-to').value.trim();
    var amount = parseFloat(document.getElementById('send-amount').value);
    var token = document.getElementById('send-token').value;

    // Validate
    if (!to) return showError('send-status', 'Please enter a recipient address.');
    if (!amount || amount <= 0) return showError('send-status', 'Please enter a valid amount.');

    // Validate address format (basic base58 check)
    if (!/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(to)) {
      return showError('send-status', 'Invalid Solana address format.');
    }

    // Check sufficient balance
    if (cachedBalances) {
      if (token === 'SOL' && amount > cachedBalances.sol - 0.000005) {
        return showError('send-status', 'Insufficient SOL balance (need to reserve fee).');
      }
      if (token === 'USDC' && amount > cachedBalances.usdc) {
        return showError('send-status', 'Insufficient USDC balance.');
      }
    }

    // Populate confirmation modal
    var shortFrom = solAddress.slice(0, 8) + '...' + solAddress.slice(-4);
    var shortTo = to.slice(0, 8) + '...' + to.slice(-4);
    document.getElementById('confirm-from').textContent = shortFrom;
    document.getElementById('confirm-from').title = solAddress;
    document.getElementById('confirm-to').textContent = shortTo;
    document.getElementById('confirm-to').title = to;
    document.getElementById('confirm-amount').textContent = amount + ' ' + token;

    var usdValue = '—';
    if (token === 'SOL' && cachedPrice) {
      usdValue = '$' + formatUSD(amount * cachedPrice);
    } else if (token === 'USDC') {
      usdValue = '$' + formatUSD(amount);
    }
    document.getElementById('confirm-usd').textContent = usdValue;

    // High-value warning
    var usdAmt = token === 'SOL' ? amount * (cachedPrice || 0) : amount;
    document.getElementById('send-high-value-warn').style.display = usdAmt > 100 ? '' : 'none';

    hideTxStatus('send-status');
    document.getElementById('send-confirm-modal').classList.add('open');
  };

  window.confirmSend = async function () {
    var to = document.getElementById('send-to').value.trim();
    var amount = parseFloat(document.getElementById('send-amount').value);
    var token = document.getElementById('send-token').value;

    closeSendModal();
    showPending('send-status', 'Broadcasting transaction...');

    try {
      var privateKey = await getPrivateKey();
      if (!privateKey) {
        return showError('send-status', 'Cannot access signing key. Make sure you are connected to the network.');
      }

      var txHash;
      if (token === 'SOL') {
        txHash = await HosWallet.sendSOL(to, amount, privateKey);
      } else if (token === 'USDC') {
        txHash = await HosWallet.sendToken(to, amount, USDC_MINT, privateKey);
      }

      if (txHash) {
        showSuccess('send-status',
          'Transaction confirmed! <a class="tx-link" href="https://solscan.io/tx/' + txHash + '" target="_blank" rel="noopener">View on Solscan</a>'
        );
        // Invalidate balance cache
        cachedBalances = null;
        balanceCacheTime = 0;
        // Reload after a brief delay
        setTimeout(function () {
          loadBalances();
          loadTransactions();
        }, 3000);
      }
    } catch (e) {
      console.error('Send failed:', e);
      showError('send-status', 'Transaction failed: ' + (e.message || e));
    }
  };

  window.closeSendModal = function () {
    document.getElementById('send-confirm-modal').classList.remove('open');
  };

  // ── Receive ──

  window.showReceiveModal = function () {
    document.getElementById('receive-modal').classList.add('open');
    generateQR('receive-qr', solAddress);
  };

  window.closeReceiveModal = function () {
    document.getElementById('receive-modal').classList.remove('open');
  };

  // ── Swap ──

  var swapQuoteTimeout = null;

  window.onSwapInputChange = function () {
    // Debounce quote fetching
    clearTimeout(swapQuoteTimeout);
    swapQuoteTimeout = setTimeout(fetchSwapQuote, 500);
  };

  async function fetchSwapQuote() {
    var inputAmount = parseFloat(document.getElementById('swap-input-amount').value);
    var inputToken = document.getElementById('swap-input-token').value;
    var outputToken = document.getElementById('swap-output-token').value;

    if (!inputAmount || inputAmount <= 0 || inputToken === outputToken) {
      document.getElementById('swap-output-amount').value = '';
      document.getElementById('swap-rate').textContent = '';
      document.getElementById('swap-impact').style.display = 'none';
      document.getElementById('btn-swap').disabled = true;
      return;
    }

    document.getElementById('swap-output-amount').value = 'Loading...';
    document.getElementById('btn-swap').disabled = true;

    try {
      var inputMint = tokenToMint(inputToken);
      var outputMint = tokenToMint(outputToken);
      var inputDecimals = tokenDecimals(inputToken);
      var amountLamports = Math.round(inputAmount * Math.pow(10, inputDecimals));

      if (HosWallet.getSwapQuote) {
        var quote = await HosWallet.getSwapQuote(inputMint, outputMint, amountLamports, slippage * 100);
        var outputDecimals = tokenDecimals(outputToken);
        var outAmount = quote.outAmount / Math.pow(10, outputDecimals);

        document.getElementById('swap-output-amount').value = formatAmount(outAmount, 6);

        // Calculate rate
        var rate = outAmount / inputAmount;
        document.getElementById('swap-rate').textContent =
          '1 ' + inputToken + ' = ' + formatAmount(rate, 4) + ' ' + outputToken;

        // Price impact
        var impact = quote.priceImpactPct || 0;
        var impactEl = document.getElementById('swap-impact');
        if (impact > 1) {
          impactEl.textContent = 'Price impact: ' + impact.toFixed(2) + '% — proceed with caution';
          impactEl.style.display = '';
        } else {
          impactEl.style.display = 'none';
        }

        document.getElementById('btn-swap').disabled = false;
      } else {
        document.getElementById('swap-output-amount').value = '';
        document.getElementById('swap-rate').textContent = 'Swap not available — wallet.js does not support quotes';
        document.getElementById('btn-swap').disabled = true;
      }
    } catch (e) {
      console.error('Swap quote failed:', e);
      document.getElementById('swap-output-amount').value = '';
      document.getElementById('swap-rate').textContent = 'Failed to fetch quote';
      document.getElementById('btn-swap').disabled = true;
    }
  }

  window.flipSwapDirection = function () {
    var inputToken = document.getElementById('swap-input-token');
    var outputToken = document.getElementById('swap-output-token');
    var temp = inputToken.value;
    inputToken.value = outputToken.value;
    outputToken.value = temp;

    // Move output amount to input
    var outputVal = document.getElementById('swap-output-amount').value;
    if (outputVal && !isNaN(parseFloat(outputVal))) {
      document.getElementById('swap-input-amount').value = outputVal;
    }
    document.getElementById('swap-output-amount').value = '';

    onSwapInputChange();
  };

  window.setSlippage = function (val, btn) {
    slippage = val;
    document.querySelectorAll('.swap-slippage button').forEach(function (b) {
      b.classList.remove('active');
    });
    if (btn && btn.tagName === 'BUTTON') btn.classList.add('active');
    // Re-fetch quote with new slippage
    if (document.getElementById('swap-input-amount').value) {
      onSwapInputChange();
    }
  };

  window.prepareSwap = function () {
    var inputAmount = document.getElementById('swap-input-amount').value;
    var inputToken = document.getElementById('swap-input-token').value;
    var outputAmount = document.getElementById('swap-output-amount').value;
    var outputToken = document.getElementById('swap-output-token').value;
    var rate = document.getElementById('swap-rate').textContent;

    document.getElementById('confirm-swap-input').textContent = inputAmount + ' ' + inputToken;
    document.getElementById('confirm-swap-output').textContent = outputAmount + ' ' + outputToken;
    document.getElementById('confirm-swap-rate').textContent = rate;
    document.getElementById('confirm-swap-slippage').textContent = slippage + '%';

    hideTxStatus('swap-status');
    document.getElementById('swap-confirm-modal').classList.add('open');
  };

  window.confirmSwap = async function () {
    closeSwapModal();
    showPending('swap-status', 'Executing swap...');

    try {
      var privateKey = await getPrivateKey();
      if (!privateKey) {
        return showError('swap-status', 'Cannot access signing key.');
      }

      var inputToken = document.getElementById('swap-input-token').value;
      var outputToken = document.getElementById('swap-output-token').value;
      var inputAmount = parseFloat(document.getElementById('swap-input-amount').value);
      var inputMint = tokenToMint(inputToken);
      var outputMint = tokenToMint(outputToken);
      var inputDecimals = tokenDecimals(inputToken);
      var amountLamports = Math.round(inputAmount * Math.pow(10, inputDecimals));

      if (!HosWallet.executeSwap) {
        return showError('swap-status', 'Swap not implemented in wallet.js yet.');
      }

      var txHash = await HosWallet.executeSwap(inputMint, outputMint, amountLamports, slippage * 100, privateKey);

      showSuccess('swap-status',
        'Swap confirmed! <a class="tx-link" href="https://solscan.io/tx/' + txHash + '" target="_blank" rel="noopener">View on Solscan</a>'
      );
      cachedBalances = null;
      balanceCacheTime = 0;
      setTimeout(function () { loadBalances(); }, 3000);
    } catch (e) {
      console.error('Swap failed:', e);
      showError('swap-status', 'Swap failed: ' + (e.message || e));
    }
  };

  window.closeSwapModal = function () {
    document.getElementById('swap-confirm-modal').classList.remove('open');
  };

  // ── Stake ──

  async function loadValidators() {
    if (validators.length > 0) return; // Already loaded

    var listEl = document.getElementById('validator-list');
    listEl.innerHTML = '<div style="text-align:center;padding:var(--space-lg);color:var(--text-muted)"><span class="spinner"></span> Loading validators...</div>';

    try {
      if (HosWallet.getValidators) {
        validators = await HosWallet.getValidators();
      } else {
        validators = [];
      }
      renderValidators();
      populateValidatorSelect();
    } catch (e) {
      console.error('Failed to load validators:', e);
      listEl.innerHTML = '<div style="color:var(--danger);text-align:center;padding:var(--space-lg)">Failed to load validators.</div>';
    }
  }

  function renderValidators() {
    var listEl = document.getElementById('validator-list');
    if (!validators.length) {
      listEl.innerHTML = '<div style="text-align:center;padding:var(--space-lg);color:var(--text-muted);font-style:italic">No validators available.</div>';
      return;
    }

    listEl.innerHTML = validators.map(function (v) {
      var name = v.name || v.identity || (v.votePubkey ? v.votePubkey.slice(0, 8) + '...' : '—');
      var commission = v.commission != null ? v.commission + '%' : '—';
      var stake = v.activatedStake != null ? formatAmount(v.activatedStake / 1e9, 0) + ' SOL' : '—';
      return '<div class="validator-item" onclick="selectValidator(\'' + escapeHtml(v.votePubkey || '') + '\')">' +
        '<span class="v-name">' + escapeHtml(name) + '</span>' +
        '<span class="v-commission">' + commission + '</span>' +
        '<span class="v-stake">' + stake + '</span>' +
        '</div>';
    }).join('');
  }

  function populateValidatorSelect() {
    var select = document.getElementById('stake-validator');
    if (!validators.length) {
      select.innerHTML = '<option value="">No validators found</option>';
      return;
    }
    select.innerHTML = validators.map(function (v) {
      var name = v.name || v.identity || (v.votePubkey ? v.votePubkey.slice(0, 12) + '...' : '—');
      var commission = v.commission != null ? ' (' + v.commission + '%)' : '';
      return '<option value="' + escapeHtml(v.votePubkey || '') + '">' + escapeHtml(name) + commission + '</option>';
    }).join('');
  }

  window.selectValidator = function (votePubkey) {
    document.getElementById('stake-validator').value = votePubkey;
    // Scroll to stake form
    document.querySelector('#tab-stake .panel').scrollIntoView({ behavior: 'smooth' });
  };

  window.sortValidators = function () {
    var sort = document.getElementById('validator-sort').value;
    if (sort === 'commission') {
      validators.sort(function (a, b) { return (a.commission || 0) - (b.commission || 0); });
    } else if (sort === 'stake') {
      validators.sort(function (a, b) { return (b.activatedStake || 0) - (a.activatedStake || 0); });
    }
    renderValidators();
  };

  async function loadStakeAccounts() {
    var listEl = document.getElementById('stake-accounts-list');
    var emptyEl = document.getElementById('stake-accounts-empty');

    try {
      if (HosWallet.getStakeAccounts) {
        stakeAccounts = await HosWallet.getStakeAccounts(solAddress);
      } else {
        stakeAccounts = [];
      }

      if (!stakeAccounts.length) {
        listEl.innerHTML = '';
        emptyEl.style.display = '';
        document.getElementById('staked-amount').textContent = '0 SOL';
        document.getElementById('staked-rewards').textContent = '0 SOL';
        return;
      }

      emptyEl.style.display = 'none';

      var totalStaked = 0;
      var totalRewards = 0;

      listEl.innerHTML = stakeAccounts.map(function (sa) {
        var amount = sa.lamports ? sa.lamports / 1e9 : 0;
        totalStaked += amount;
        totalRewards += sa.rewards || 0;

        var status = sa.status || 'active';
        var statusClass = status.toLowerCase().replace(/\s/g, '');
        var validator = sa.validatorName || (sa.votePubkey ? sa.votePubkey.slice(0, 8) + '...' : '—');

        return '<div class="stake-account">' +
          '<span class="sa-status ' + statusClass + '">' + status + '</span>' +
          '<span style="flex:1">' + escapeHtml(validator) + '</span>' +
          '<span style="font-weight:600">' + formatAmount(amount, 4) + ' SOL</span>' +
          '<button class="btn-small" onclick="unstakeSOL(\'' + escapeHtml(sa.pubkey || '') + '\')" style="margin-left:auto">Unstake</button>' +
          '</div>';
      }).join('');

      document.getElementById('staked-amount').textContent = formatAmount(totalStaked, 4) + ' SOL';
      document.getElementById('staked-rewards').textContent = formatAmount(totalRewards, 4) + ' SOL';
    } catch (e) {
      console.error('Failed to load stake accounts:', e);
      listEl.innerHTML = '<div style="color:var(--danger)">Failed to load stake accounts.</div>';
    }
  }

  window.stakeSOL = async function () {
    var amount = parseFloat(document.getElementById('stake-amount').value);
    var validatorPubkey = document.getElementById('stake-validator').value;

    if (!amount || amount < 0.01) {
      return alert('Minimum stake is 0.01 SOL.');
    }
    if (!validatorPubkey) {
      return alert('Please select a validator.');
    }

    if (!confirm('Stake ' + amount + ' SOL with this validator?')) return;

    try {
      var privateKey = await getPrivateKey();
      if (!privateKey) return alert('Cannot access signing key.');

      if (!HosWallet.stakeSOL) {
        return alert('Staking not implemented in wallet.js yet.');
      }

      var txHash = await HosWallet.stakeSOL(amount, validatorPubkey, privateKey);
      alert('Staking transaction submitted! It may take a few minutes to activate.');

      cachedBalances = null;
      balanceCacheTime = 0;
      stakeAccounts = [];
      loadBalances();
      loadStakeAccounts();
    } catch (e) {
      console.error('Stake failed:', e);
      alert('Staking failed: ' + (e.message || e));
    }
  };

  window.unstakeSOL = async function (stakeAccountPubkey) {
    if (!confirm('Unstake this account? It takes ~2-3 days to deactivate.')) return;

    try {
      var privateKey = await getPrivateKey();
      if (!privateKey) return alert('Cannot access signing key.');

      if (!HosWallet.unstakeSOL) {
        return alert('Unstaking not implemented in wallet.js yet.');
      }

      await HosWallet.unstakeSOL(stakeAccountPubkey, privateKey);
      alert('Unstaking initiated. Your SOL will be available in ~2-3 days.');
      stakeAccounts = [];
      loadStakeAccounts();
    } catch (e) {
      console.error('Unstake failed:', e);
      alert('Unstake failed: ' + (e.message || e));
    }
  };

  // ── NFTs ──

  async function loadNFTs() {
    if (ownedNFTs.length > 0) return; // Already loaded

    var loadingEl = document.getElementById('nft-loading');
    var gridEl = document.getElementById('nft-grid');
    var emptyEl = document.getElementById('nft-empty');

    try {
      if (HosWallet.getNFTs) {
        ownedNFTs = await HosWallet.getNFTs(solAddress);
      } else {
        ownedNFTs = [];
      }

      loadingEl.style.display = 'none';

      if (!ownedNFTs.length) {
        gridEl.innerHTML = '';
        emptyEl.style.display = '';
        return;
      }

      emptyEl.style.display = 'none';
      renderNFTGrid(ownedNFTs);
    } catch (e) {
      console.error('Failed to load NFTs:', e);
      loadingEl.innerHTML = '<span style="color:var(--danger)">Failed to load NFTs.</span>';
    }
  }

  function renderNFTGrid(nfts) {
    var gridEl = document.getElementById('nft-grid');
    gridEl.innerHTML = nfts.map(function (nft, i) {
      var imgSrc = nft.image || nft.uri || '';
      var name = nft.name || 'Untitled #' + (i + 1);
      return '<div class="nft-card" onclick="showNFTDetail(' + i + ')">' +
        (imgSrc ? '<img src="' + escapeHtml(imgSrc) + '" alt="' + escapeHtml(name) + '" loading="lazy">' :
          '<div style="aspect-ratio:1;background:var(--bg-input);display:flex;align-items:center;justify-content:center;color:var(--text-muted)">No image</div>') +
        '<div class="nft-name">' + escapeHtml(name) + '</div>' +
        '</div>';
    }).join('');
  }

  window.showNFTDetail = function (index) {
    selectedNFT = ownedNFTs[index];
    if (!selectedNFT) return;

    var content = document.getElementById('nft-detail-content');
    var imgSrc = selectedNFT.image || selectedNFT.uri || '';
    var attrs = selectedNFT.attributes || [];

    content.innerHTML =
      (imgSrc ? '<img src="' + escapeHtml(imgSrc) + '" style="width:100%;border-radius:8px;margin-bottom:var(--space-lg)" alt="">' : '') +
      '<h3 style="margin:0 0 var(--space-md)">' + escapeHtml(selectedNFT.name || 'Untitled') + '</h3>' +
      (selectedNFT.description ? '<p style="font-size:0.85rem;color:var(--text-muted);margin-bottom:var(--space-lg)">' + escapeHtml(selectedNFT.description) + '</p>' : '') +
      (attrs.length ? '<div style="display:flex;flex-wrap:wrap;gap:var(--space-sm)">' +
        attrs.map(function (a) {
          return '<div style="background:var(--bg-input);border:1px solid var(--border);border-radius:6px;padding:var(--space-xs) var(--space-md);font-size:0.75rem">' +
            '<span style="color:var(--text-muted)">' + escapeHtml(a.trait_type || a.key || '') + ':</span> ' +
            escapeHtml(String(a.value || '')) +
            '</div>';
        }).join('') + '</div>' : '');

    document.getElementById('nft-detail-modal').classList.add('open');
  };

  window.closeNFTModal = function () {
    document.getElementById('nft-detail-modal').classList.remove('open');
    selectedNFT = null;
  };

  window.promptSendNFT = async function () {
    if (!selectedNFT) return;
    var toAddr = prompt('Enter recipient Solana address:');
    if (!toAddr || !toAddr.trim()) return;
    toAddr = toAddr.trim();

    if (!/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(toAddr)) {
      return alert('Invalid Solana address format.');
    }

    if (!confirm('Send "' + (selectedNFT.name || 'this NFT') + '" to ' + toAddr.slice(0, 8) + '...?')) return;

    try {
      var privateKey = await getPrivateKey();
      if (!privateKey) return alert('Cannot access signing key.');

      if (!HosWallet.sendNFT) {
        return alert('NFT transfer not implemented in wallet.js yet.');
      }

      var mint = selectedNFT.mint || selectedNFT.address;
      await HosWallet.sendNFT(mint, toAddr, privateKey);
      alert('NFT sent successfully!');
      closeNFTModal();
      ownedNFTs = [];
      loadNFTs();
    } catch (e) {
      console.error('NFT send failed:', e);
      alert('NFT transfer failed: ' + (e.message || e));
    }
  };

  // ── Private Key Access ──

  async function getPrivateKey() {
    // Try global myIdentity first (set by app.js/crypto.js)
    if (window.myIdentity && window.myIdentity.privateKey) {
      return window.myIdentity.privateKey;
    }

    // Try loading from IndexedDB (same pattern as crypto.js getOrCreateIdentity)
    try {
      var db = await openIdentityDB();
      var tx = db.transaction('keys', 'readonly');
      var store = tx.objectStore('keys');
      var req = store.get('identity');
      var result = await promisifyRequest(req);
      if (result && result.privateKey) {
        return result.privateKey;
      }
    } catch (e) {
      console.warn('IndexedDB key access failed:', e);
    }

    return null;
  }

  function openIdentityDB() {
    return new Promise(function (resolve, reject) {
      var req = indexedDB.open('HumanityOS', 1);
      req.onupgradeneeded = function () {
        if (!req.result.objectStoreNames.contains('keys')) {
          req.result.createObjectStore('keys');
        }
      };
      req.onsuccess = function () { resolve(req.result); };
      req.onerror = function () { reject(req.error); };
    });
  }

  function promisifyRequest(req) {
    return new Promise(function (resolve, reject) {
      req.onsuccess = function () { resolve(req.result); };
      req.onerror = function () { reject(req.error); };
    });
  }

  // ── Token Helpers ──

  function tokenToMint(token) {
    switch (token) {
      case 'SOL': return 'So11111111111111111111111111111111111111112'; // Wrapped SOL
      case 'USDC': return USDC_MINT;
      case 'USDT': return USDT_MINT;
      default: return token;
    }
  }

  function tokenDecimals(token) {
    switch (token) {
      case 'SOL': return 9;
      case 'USDC': return 6;
      case 'USDT': return 6;
      default: return 9;
    }
  }

  // ── Formatting ──

  function formatAmount(val, decimals) {
    if (val == null || isNaN(val)) return '0';
    return Number(val).toLocaleString('en-US', {
      minimumFractionDigits: 0,
      maximumFractionDigits: decimals
    });
  }

  function formatUSD(val) {
    if (val == null || isNaN(val)) return '0.00';
    return Number(val).toLocaleString('en-US', {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });
  }

  function formatTime(ts) {
    try {
      var d = new Date(typeof ts === 'number' ? ts * 1000 : ts);
      var now = new Date();
      var diff = now - d;
      if (diff < 60000) return 'just now';
      if (diff < 3600000) return Math.floor(diff / 60000) + 'm ago';
      if (diff < 86400000) return Math.floor(diff / 3600000) + 'h ago';
      if (diff < 604800000) return Math.floor(diff / 86400000) + 'd ago';
      return d.toLocaleDateString();
    } catch (e) {
      return '';
    }
  }

  function escapeHtml(str) {
    if (!str) return '';
    return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  // ── Status Display ──

  function showPending(elId, msg) {
    var el = document.getElementById(elId);
    el.className = 'tx-status pending';
    el.innerHTML = '<span class="spinner"></span> ' + msg;
    el.style.display = '';
  }

  function showSuccess(elId, msg) {
    var el = document.getElementById(elId);
    el.className = 'tx-status confirmed';
    el.innerHTML = msg;
    el.style.display = '';
  }

  function showError(elId, msg) {
    var el = document.getElementById(elId);
    el.className = 'tx-status error';
    el.textContent = msg;
    el.style.display = '';
  }

  function hideTxStatus(elId) {
    var el = document.getElementById(elId);
    if (el) el.style.display = 'none';
  }

})();
