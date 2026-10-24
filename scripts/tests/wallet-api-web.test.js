// The website's wallet page calls only functions the wallet library really exports (2026-10-24).
// Run: node --test scripts/tests/wallet-api-web.test.js
//
// The accessibility pass found web/pages/wallet-app.js calling HosWallet.getBalance,
// getTokenBalance and getTransactions, none of which web/shared/wallet.js exports (it has
// getSOLBalance, getUSDCBalance and getTransactionHistory), so the balance never loaded and the
// history stayed empty: two of the calls were wrapped in "if it exists" checks that hid it. This
// reads every HosWallet.<name> the page uses and every name in wallet.js's `window.HosWallet = {}`,
// and fails on any the library lacks, unless it is listed below as deliberately not built yet (the
// page says so to the person when they press it).
//
// Seen red 2026-10-24 against the old page (git show HEAD~1:web/pages/wallet-app.js,
// WALLET_APP=<that copy>): "getBalance, getTokenBalance, getTransactions" (sendNFT, deliberately unbuilt, was then
// added to NOT_BUILT_YET).
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..', '..');
const PAGE = process.env.WALLET_APP || path.join(ROOT, 'web', 'pages', 'wallet-app.js');
const LIB = path.join(ROOT, 'web', 'shared', 'wallet.js');

// Called by the page, not in the library yet, and the page tells the person it is not built.
const NOT_BUILT_YET = new Set(['unstakeSOL', 'sendNFT']);

function exported() {
  const src = fs.readFileSync(LIB, 'utf8');
  const at = src.indexOf('window.HosWallet = {');
  assert.ok(at >= 0, 'wallet.js still exposes window.HosWallet');
  const body = src.slice(at, src.indexOf('\n  };', at));
  return new Set([...body.matchAll(/^\s+([A-Za-z_]\w*)\s*:/gm)].map((m) => m[1]));
}

test('every wallet function the page calls exists in the wallet library', () => {
  const lib = exported();
  const page = fs.readFileSync(PAGE, 'utf8');
  const called = new Set([...page.matchAll(/HosWallet\.([A-Za-z_]\w*)/g)].map((m) => m[1]));
  assert.ok(called.size > 5, 'the page calls the library');
  const missing = [...called].filter((n) => !lib.has(n) && !NOT_BUILT_YET.has(n)).sort();
  assert.deepStrictEqual(missing, [], `called but not in wallet.js: ${missing.join(', ')}`);
});

test('a function listed as not built yet is still missing (so the list cannot go stale)', () => {
  const lib = exported();
  for (const n of NOT_BUILT_YET) assert.ok(!lib.has(n), `${n} now exists: take it off NOT_BUILT_YET`);
});
