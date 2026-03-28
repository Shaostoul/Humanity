/**
 * Wallet Guide — renders step-by-step wallet education sections
 * from a data-driven array. No crypto jargon without explanation.
 */
(function () {
  'use strict';

  // ── Guide section data ──

  var sections = [
    {
      id: 'what-is-a-wallet',
      title: '1. What is a Wallet?',
      content: function () {
        return '<p>Think of a crypto wallet like a bank account that <strong>you</strong> control completely. No bank, no company, no government can freeze it, close it, or take your money.</p>' +
          '<p>A regular bank account works like this: the bank holds your money, and you ask them permission to send it. If the bank closes or gets hacked, your money could be at risk.</p>' +
          '<p>A crypto wallet works differently: <strong>you</strong> hold the keys. The money is stored on a public ledger (the blockchain) that nobody owns, and only your private key can move your funds.</p>' +
          '<p>Your wallet has two parts:</p>' +
          '<ul>' +
          '<li><strong>Public key (your address)</strong> — Like your email address. You share this with people so they can send you money. Safe to share with anyone.</li>' +
          '<li><strong>Private key</strong> — Like your email password. This proves the money is yours. <strong>Never share this with anyone, ever.</strong></li>' +
          '</ul>' +
          '<div class="callout">Think of it this way: Your address is your mailbox. Anyone can drop letters (money) in. But only you have the key to open it and take things out.</div>';
      }
    },
    {
      id: 'your-hos-wallet',
      title: '2. Your HumanityOS Wallet',
      content: function () {
        return '<p>Here is the good news: <strong>you already have a wallet.</strong></p>' +
          '<p>When you created your HumanityOS identity, the system generated an Ed25519 key pair for you. This is the same type of cryptography that Solana (a major cryptocurrency network) uses.</p>' +
          '<p>That means your HumanityOS identity key <strong>is</strong> your Solana wallet address. No extra setup, no additional accounts, no third-party apps needed.</p>' +
          '<ul>' +
          '<li>Your identity key = your Solana wallet</li>' +
          '<li>Your 24-word seed phrase backs up both your identity AND your wallet</li>' +
          '<li>You can send and receive SOL (Solana\'s currency) and any Solana-based tokens (like USDC)</li>' +
          '</ul>' +
          '<div class="callout">You do not need to "create a wallet" separately. If you have a HumanityOS identity, you have a Solana wallet. Visit the <a href="/wallet">Wallet page</a> to see it.</div>';
      }
    },
    {
      id: 'how-to-receive',
      title: '3. How to Receive Money',
      content: function () {
        return '<p>Receiving crypto is the simplest operation. Someone sends money to your address, and it shows up in your balance.</p>' +
          '<ol class="steps">' +
          '<li>Go to the <a href="/wallet">Wallet page</a></li>' +
          '<li>Your Solana address is shown at the top (a long string of letters and numbers, like <code>7xK...q3m</code>)</li>' +
          '<li>Click <strong>Copy</strong> to copy your address to clipboard</li>' +
          '<li>Share that address with the person sending you money (by text, email, or QR code)</li>' +
          '<li>They send SOL or tokens to your address</li>' +
          '<li>The funds appear in your balance within a few seconds</li>' +
          '</ol>' +
          '<div class="callout">Your address is <strong>public and safe to share</strong>. It is like giving someone your email address. They can send you money, but they cannot take money out. Only your private key can do that.</div>' +
          '<p>You can also click the <strong>Receive</strong> button on the wallet page to show a QR code. The sender can scan this QR code with their phone to send you money without typing the address manually.</p>';
      }
    },
    {
      id: 'how-to-send',
      title: '4. How to Send Money',
      content: function () {
        return '<p>Sending crypto means moving money from your wallet to someone else\'s address.</p>' +
          '<ol class="steps">' +
          '<li>Go to the <a href="/wallet">Wallet page</a> and click the <strong>Send</strong> tab</li>' +
          '<li>Paste the recipient\'s Solana address into the "Recipient Address" field</li>' +
          '<li>Enter the amount you want to send</li>' +
          '<li>Choose the token (SOL, USDC, etc.)</li>' +
          '<li>Click <strong>Review Send</strong></li>' +
          '<li>Double-check the address and amount in the confirmation popup</li>' +
          '<li>Click <strong>Confirm Send</strong></li>' +
          '</ol>' +
          '<div class="callout callout-warn">Always double-check the recipient address. Crypto transactions <strong>cannot be reversed</strong>. If you send to the wrong address, the money is gone. Copy-paste addresses instead of typing them by hand.</div>' +
          '<p><strong>About network fees:</strong> Every transaction on Solana costs a tiny fee (about $0.0003, less than a penny). This fee goes to the network validators who process your transaction. You need a small amount of SOL in your wallet to pay this fee, even if you are sending a different token like USDC.</p>';
      }
    },
    {
      id: 'how-to-buy',
      title: '5. How to Buy Crypto',
      content: function () {
        return '<p>There are several ways to get crypto into your wallet:</p>' +
          '<div class="option-card">' +
          '<h4>Option A: Someone sends you crypto directly</h4>' +
          '<p>A friend, employer, or client sends SOL or tokens to your wallet address. This is the simplest way. Share your address (see "How to Receive") and they send it from their wallet.</p>' +
          '</div>' +
          '<div class="option-card">' +
          '<h4>Option B: Buy on an exchange and transfer</h4>' +
          '<p>Exchanges are websites where you can buy crypto with a debit card or bank transfer. Popular ones include Coinbase, Kraken, and Gemini.</p>' +
          '<ol class="steps">' +
          '<li>Create an account on an exchange (e.g., <a href="https://www.coinbase.com" target="_blank" rel="noopener">Coinbase</a>, <a href="https://www.kraken.com" target="_blank" rel="noopener">Kraken</a>)</li>' +
          '<li>Complete their identity verification (required by law)</li>' +
          '<li>Add a payment method (bank account or debit card)</li>' +
          '<li>Buy SOL with your local currency (USD, EUR, etc.)</li>' +
          '<li>Go to the exchange\'s "Withdraw" or "Send" page</li>' +
          '<li>Paste your HumanityOS wallet address as the destination</li>' +
          '<li>Choose "Solana" as the network</li>' +
          '<li>Confirm the withdrawal</li>' +
          '</ol>' +
          '</div>' +
          '<div class="option-card">' +
          '<h4>Option C: Use a fiat on-ramp service</h4>' +
          '<p>On-ramp services let you buy crypto with a card and send it directly to your wallet address, without creating an exchange account. They are simpler but may have slightly higher fees.</p>' +
          '<ul>' +
          '<li><a href="https://www.moonpay.com" target="_blank" rel="noopener">MoonPay</a> — supports cards and bank transfers</li>' +
          '<li><a href="https://transak.com" target="_blank" rel="noopener">Transak</a> — supports 100+ countries</li>' +
          '<li><a href="https://ramp.network" target="_blank" rel="noopener">Ramp Network</a> — low fees, many payment methods</li>' +
          '</ul>' +
          '</div>' +
          '<div class="callout">Tip: If you are just starting out, buying a small amount ($10-20) first is a good way to learn how it works without risking much.</div>';
      }
    },
    {
      id: 'how-to-convert-to-usd',
      title: '6. How to Convert to USD',
      content: function () {
        return '<p>When you want to turn your crypto back into regular money (USD, EUR, etc.), you need to use an exchange.</p>' +
          '<ol class="steps">' +
          '<li>Create an account on a crypto exchange if you do not have one (e.g., <a href="https://www.coinbase.com" target="_blank" rel="noopener">Coinbase</a>, <a href="https://www.kraken.com" target="_blank" rel="noopener">Kraken</a>, <a href="https://www.gemini.com" target="_blank" rel="noopener">Gemini</a>)</li>' +
          '<li>Find the exchange\'s "Deposit" page and copy their Solana deposit address</li>' +
          '<li>Go to your <a href="/wallet">HumanityOS Wallet</a> and send SOL to that exchange address</li>' +
          '<li>Wait for the deposit to appear on the exchange (usually a few seconds to a minute)</li>' +
          '<li>Sell your SOL for USD (or your local currency) on the exchange</li>' +
          '<li>Withdraw the USD to your bank account</li>' +
          '</ol>' +
          '<p><strong>Typical fees:</strong></p>' +
          '<ul>' +
          '<li>Sending from HumanityOS to exchange: ~$0.0003 (Solana network fee)</li>' +
          '<li>Exchange trading fee: ~0.5-1% of the amount</li>' +
          '<li>Bank withdrawal: $0-25 depending on the exchange and method</li>' +
          '</ul>' +
          '<div class="callout">Most exchanges take 1-5 business days to send money to your bank. Faster methods (like instant transfer) may cost a small extra fee.</div>';
      }
    },
    {
      id: 'how-to-swap',
      title: '7. How to Swap Tokens',
      content: function () {
        return '<p><strong>What are tokens?</strong> On the Solana network, SOL is the main currency (like the US Dollar is the main currency in America). But there are also "tokens" that run on the same network, each with a different purpose:</p>' +
          '<ul>' +
          '<li><strong>USDC</strong> — A stablecoin worth exactly $1. It does not go up or down in price like SOL does. Good for saving money without price swings.</li>' +
          '<li><strong>USDT</strong> — Another stablecoin worth $1, similar to USDC.</li>' +
          '</ul>' +
          '<p><strong>Why swap?</strong> If you have SOL but want something that does not change in price, you can swap SOL for USDC. Or if you have USDC and want SOL (to stake it or pay fees), you swap the other way.</p>' +
          '<ol class="steps">' +
          '<li>Go to the <a href="/wallet">Wallet page</a> and click the <strong>Swap</strong> tab</li>' +
          '<li>Choose what you want to swap FROM (e.g., SOL) in the top dropdown</li>' +
          '<li>Choose what you want to swap TO (e.g., USDC) in the bottom dropdown</li>' +
          '<li>Enter the amount</li>' +
          '<li>The exchange rate and output amount are shown automatically</li>' +
          '<li>Click <strong>Swap</strong> and confirm</li>' +
          '</ol>' +
          '<div class="callout">HumanityOS uses <strong>Jupiter</strong>, the most popular Solana token exchange. It finds the best price across many exchanges automatically. Swap fees are very small (usually less than 0.3%).</div>' +
          '<p><strong>What is slippage?</strong> Prices can change slightly between when you click "Swap" and when the transaction completes. Slippage tolerance sets the maximum price change you will accept. The default 0.5% works well for most swaps.</p>';
      }
    },
    {
      id: 'backup-and-security',
      title: '8. Backup and Security',
      content: function () {
        return '<p>Your wallet is only as safe as your backup. Here is what you need to know:</p>' +
          '<h3>Your seed phrase IS your backup</h3>' +
          '<p>When you created your HumanityOS identity, you received a <strong>24-word seed phrase</strong>. This phrase can restore your entire identity AND wallet on any device. It is the master key to everything.</p>' +
          '<h3>Rules for your seed phrase</h3>' +
          '<ol class="steps">' +
          '<li><strong>Write it on paper.</strong> Physical paper cannot be hacked. Store it somewhere safe (fireproof box, safety deposit box).</li>' +
          '<li><strong>Never store it digitally.</strong> Do not save it in a notes app, email, cloud drive, screenshot, or text message. If your device is compromised, so is your wallet.</li>' +
          '<li><strong>Never share it with anyone.</strong> No legitimate service will ever ask for your seed phrase. Anyone who asks is trying to steal your money.</li>' +
          '<li><strong>Make multiple copies.</strong> Store copies in different physical locations in case of fire, flood, or other disasters.</li>' +
          '<li><strong>Test your backup.</strong> Before putting significant money in your wallet, try restoring from your seed phrase on a different device to make sure it works.</li>' +
          '</ol>' +
          '<div class="callout callout-danger"><strong>If you lose your seed phrase AND your device, your funds are gone forever.</strong> There is no "forgot password" button. No company can recover it. This is the trade-off for having a wallet that nobody can freeze or censor.</div>' +
          '<h3>Other security tips</h3>' +
          '<ul>' +
          '<li>Lock your device with a strong password or biometrics</li>' +
          '<li>Be careful of fake websites that look like HumanityOS (always check the URL)</li>' +
          '<li>Never click links in emails or messages claiming to be from your wallet</li>' +
          '<li>Start with small amounts until you are comfortable with the process</li>' +
          '</ul>';
      }
    },
    {
      id: 'glossary',
      title: '9. Glossary of Terms',
      content: function () {
        var terms = [
          ['SOL', 'The native currency of the Solana blockchain. Used to pay transaction fees and can be staked for rewards.'],
          ['USDC', 'A stablecoin (token) pegged to the US Dollar. 1 USDC = $1. Issued by Circle. Good for storing value without price volatility.'],
          ['SPL Token', 'Any token that runs on the Solana network. USDC and USDT are examples of SPL tokens. Like how apps run on your phone, tokens run on Solana.'],
          ['Transaction', 'A transfer of crypto from one address to another. Each transaction is recorded permanently on the blockchain.'],
          ['Gas Fee', 'The small cost paid to the network for processing your transaction. On Solana, this is about $0.0003 per transaction.'],
          ['Block Confirmation', 'When the network verifies your transaction and adds it to the permanent record. On Solana, this happens in about 0.4 seconds.'],
          ['Wallet Address', 'Your public identifier on the blockchain. A long string of letters and numbers. Safe to share. Like an email address for money.'],
          ['Private Key', 'The secret key that proves ownership of your wallet. Never share it. Like the password to your bank account.'],
          ['Public Key', 'The counterpart to your private key. Used to generate your wallet address. Safe to share.'],
          ['Seed Phrase', 'A set of 24 words that can restore your entire wallet and identity. Your master backup. Write it on paper and guard it with your life.'],
          ['On-Ramp', 'A service that lets you buy crypto with regular money (USD, EUR, etc.). Examples: Coinbase, MoonPay.'],
          ['Off-Ramp', 'A service that lets you sell crypto for regular money and withdraw to your bank account.'],
          ['DEX', 'Decentralized Exchange. A platform for swapping tokens directly from your wallet, without an intermediary holding your funds. Jupiter is a DEX aggregator.'],
          ['CEX', 'Centralized Exchange. A company that holds your funds while you trade. Examples: Coinbase, Kraken, Binance. You trust them with your money.'],
          ['Staking', 'Locking up SOL to help secure the network. In return, you earn rewards (about 7% per year). Like earning interest at a bank.'],
          ['Slippage', 'The difference between the expected price of a swap and the actual price when it executes. Usually very small (under 1%).'],
          ['Validator', 'A computer that helps process transactions on Solana. When you stake SOL, you choose a validator to delegate to.'],
          ['Lamports', 'The smallest unit of SOL. 1 SOL = 1 billion lamports. Like how 1 dollar = 100 cents, but much smaller.']
        ];

        var html = '<p>Crypto has a lot of specialized vocabulary. Here is what the most common terms mean in plain English:</p>';
        html += '<div class="glossary-grid">';
        for (var i = 0; i < terms.length; i++) {
          html += '<dl class="glossary-term"><dt>' + terms[i][0] + '</dt><dd>' + terms[i][1] + '</dd></dl>';
        }
        html += '</div>';
        return html;
      }
    }
  ];

  // ── Render ──

  function render() {
    var tocList = document.getElementById('toc-list');
    var container = document.getElementById('guide-sections');
    if (!tocList || !container) return;

    var tocHtml = '';
    var sectionHtml = '';

    for (var i = 0; i < sections.length; i++) {
      var s = sections[i];
      tocHtml += '<li><a href="#' + s.id + '"><span class="toc-num">' + (i + 1) + '.</span>' + s.title.replace(/^\d+\.\s*/, '') + '</a></li>';
      sectionHtml += '<div class="guide-section" id="' + s.id + '">';
      sectionHtml += '<h2>' + s.title + '</h2>';
      sectionHtml += s.content();
      sectionHtml += '</div>';
    }

    tocList.innerHTML = tocHtml;
    container.innerHTML = sectionHtml;

    // Smooth scroll for TOC links
    tocList.addEventListener('click', function (e) {
      var link = e.target.closest('a');
      if (!link) return;
      e.preventDefault();
      var target = document.querySelector(link.getAttribute('href'));
      if (target) {
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', render);
  } else {
    render();
  }
})();
