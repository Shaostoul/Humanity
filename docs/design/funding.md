# Funding & Donation System

Aggregates multiple funding sources into a single donate page with live progress tracking. Server owner configures goals and addresses; clients query balances client-side.

## Funding Sources (priority order)

| Source | Why | Fees |
|--------|-----|------|
| GitHub Sponsors | Already linked, $5k matching year 1 | 0% |
| Solana (SOL/USDC) | Ed25519 native (matches HumanityOS identity), sub-cent tx fees | ~0% |
| Bitcoin | Largest network, ideological reach | network fee only |
| Stripe (optional) | Credit card fallback for non-crypto users | ~2.9% + 30c |

## Server Config

`/api/server-info` extended with a `funding` block. Stored in `data/server-config.json`, editable by server owner.

```json
{
  "server_name": "United Humanity",
  "owner_key": "abc123...",
  "funding": {
    "goal_usd": 100000,
    "goal_label": "Full-time development for 1 year",
    "sources": [
      { "type": "github_sponsors", "url": "https://github.com/sponsors/Shaostoul" },
      { "type": "solana", "address": "" },
      { "type": "bitcoin", "address": "" }
    ],
    "display_progress": true
  }
}
```

Server reads this file at startup and serves it via the existing `/api/server-info` endpoint. No new endpoints needed.

## Donate Page

**File:** `ui/pages/donate.html` + `donate-app.js`

### Wireframe

```
+----------------------------------------------------------+
|  [shell.js nav bar]                          [Donate]     |
+----------------------------------------------------------+
|                                                           |
|   End Poverty. Unite Humanity.                            |
|   Your support funds full-time open-source development.   |
|                                                           |
|   +----------------------------------------------------+ |
|   | [$12,400 of $100,000]  ============----------  12% | |
|   | Full-time development for 1 year                   | |
|   +----------------------------------------------------+ |
|                                                           |
|   +------------------+  +------------------+              |
|   | [GH icon]        |  | [SOL icon]       |              |
|   | GitHub Sponsors  |  | Solana           |              |
|   | [Sponsor btn]    |  | addr... [copy]   |              |
|   | 0% fees, matched |  | [QR code]        |              |
|   +------------------+  +------------------+              |
|                                                           |
|   +------------------+  +------------------+              |
|   | [BTC icon]       |  | [card icon]      |              |
|   | Bitcoin          |  | Credit Card      |              |
|   | addr... [copy]   |  | [Stripe btn]     |              |
|   | [QR code]        |  | (coming soon)    |              |
|   +------------------+  +------------------+              |
|                                                           |
|   --- Breakdown ---                                       |
|   GitHub Sponsors:  $X,XXX/mo recurring                   |
|   Solana wallet:    $X,XXX (SOL + USDC)                   |
|   Bitcoin wallet:   $X,XXX                                |
|                                                           |
|   --- FAQ ---                                             |
|   > What are funds used for?                              |
|     Server costs, full-time development, infrastructure.  |
|   > Is my donation tax-deductible?                        |
|     Not yet. We'll incorporate as 501(c)(3) when          |
|     donations exceed $50k/year.                           |
|   > Do you hold my crypto?                                |
|     No. All crypto donations go directly to the wallet    |
|     addresses shown. Non-custodial, self-custody only.    |
|                                                           |
+----------------------------------------------------------+
```

## Data Aggregation (client-side)

All balance queries happen in the browser. No server-side API keys needed.

```javascript
// donate-app.js

const CACHE_TTL = 5 * 60 * 1000; // 5 minutes
let cachedTotals = null;
let cacheTimestamp = 0;

async function fetchFundingTotals(sources) {
  if (Date.now() - cacheTimestamp < CACHE_TTL && cachedTotals) return cachedTotals;

  const totals = { github: 0, solana: 0, bitcoin: 0, total: 0 };

  // Solana: query via free Helius RPC (100k req/day) or public mainnet RPC
  // Returns SOL balance + SPL token balances (USDC)
  // Convert SOL to USD via CoinGecko free API

  // Bitcoin: query via mempool.space or blockstream.info public API
  // GET https://mempool.space/api/address/{addr}
  // Convert BTC to USD via CoinGecko

  // GitHub Sponsors: no public balance API — manual entry in config
  // or scrape from sponsors page (fragile), or use GraphQL with token

  cachedTotals = totals;
  cacheTimestamp = Date.now();
  return totals;
}
```

### RPC endpoints (free, no API key required)

| Chain | Endpoint | Rate limit |
|-------|----------|------------|
| Solana | `https://api.mainnet-beta.solana.com` | generous |
| Solana | Helius free tier | 100k req/day |
| Bitcoin | `https://mempool.space/api/address/{addr}` | generous |
| Prices | `https://api.coingecko.com/api/v3/simple/price` | 30 req/min |

## Profile Integration

Server owner's profile card (`chat-profile.js`) shows a "Support this project" link when the viewed profile matches `serverInfo.owner_key`.

```
+---------------------------+
| [avatar]  ServerOwnerName |
| Pronouns | Location       |
| Bio text here...          |
|                           |
| [Support this project ->] |  ← links to /donate
| [====------] 12% funded   |
+---------------------------+
```

Logic: fetch `/api/server-info`, check if `profile.public_key === serverInfo.owner_key`, render funding section if match.

## QR Codes

Reuse existing `qrcode.js` (already loaded for chat identity). Generate QR on page load for each crypto address.

## Nav Integration

`shell.js` adds a "Donate" link in the nav bar. Only shown if `/api/server-info` returns a `funding` block.

## Files Changed

| File | Change |
|------|--------|
| `ui/pages/donate.html` | New — donation page |
| `ui/pages/donate-app.js` | New — balance queries, progress bar, QR generation |
| `server/src/api.rs` | Extend `/api/server-info` to include `funding` from config |
| `data/server-config.json` | New — server owner editable funding config |
| `ui/shared/shell.js` | Add conditional "Donate" nav link |
| `ui/chat/chat-profile.js` | Add funding section to owner's profile card |

## Privacy

- Blockchain balances are inherently public — no new privacy concern
- Individual donor names NOT displayed unless they opt in via a future "wall of supporters" feature
- GitHub Sponsors respects each donor's privacy settings

## Legal

- **Not a 501(c)(3)** — donations are NOT tax-deductible (yet)
- Incorporate when revenue exceeds $50k/year
- **No MSB classification** — non-custodial crypto (wallet addresses displayed, not exchanged or held on behalf of users)
- No securities implications — donations, not investments; no tokens issued
