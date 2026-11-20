# Solana Wallet Integration

Solana keys are Ed25519. The wallet's key is made from the same 32 bytes the recovery phrase writes down (the identity's seed), under its own label, so the wallet needs no separate app and no extra setup, and the wallet key is never the identity's root.

> **Changed 2026-11-19** (the PQC review's item 2, docs/reference/research/2026-11-16-pqc-review/; decided by the operator on 2026-11-16). Until then the identity key WAS the wallet: the seed itself was the Ed25519 private key, so exporting the wallet key to any wallet tool handed over the whole identity and every past private message. Every wallet address changed with the fix, with no migration and no notice in the apps, since nobody uses the app yet (operator, 2026-11-20). The canonical row: docs/reference/cryptography.md, "Solana wallet".

## Key Derivation

```
Identity seed (the recovery phrase's 32 bytes of entropy)
  -> BLAKE3 derive_key("hum/ed25519-wallet/v1", seed) = the wallet's 32-byte Ed25519 private key
     (native src/net/identity.rs wallet_secret_from_seed, web web/shared/wallet.js walletSecretFromSeed)
  -> Concatenate: [32-byte private key + 32-byte public key] = Solana Keypair (64 bytes)
     (web exportSolanaKeypair)

Ed25519 Public Key (32 bytes)
  -> Base58 encode -> Solana Address (e.g., "7xK9m...")
```

- Public key -> base58-encode -> Solana address
- Private key -> sign Solana transactions
- BIP39 24-word recovery phrase backs up both identity AND wallet
- The wallet key is re-derived from the seed when needed and never stored; the seed itself is protected by the same PBKDF2-600k vault

## Features (Phased)

### Phase 1: View Only (ship first)

Zero libraries required. Ships fast.

- **Derive Solana address** from the wallet's own derived Ed25519 public key (base58 encode, ~30 lines of vanilla JS)
- **Display address** in profile + settings with QR code and copy button
- **Show SOL balance**, query Solana mainnet RPC via fetch()
- **Show USDC balance**, query SPL token accounts via fetch()
- **Transaction history**, recent sends/receives with timestamps and amounts
- **No private key exposure**, view-only, no signing yet

### Phase 2: Send & Receive

- **Receive**, show address + QR (already done in Phase 1)
- **Send SOL**, amount input, recipient address, confirm dialog, sign transaction
- **Send USDC**, SPL token transfer
- **Transaction signing**, derive the wallet key from the seed, build transaction, sign with Ed25519
- **Confirmation modal**, show amount, recipient, estimated fee (~$0.00025), require explicit confirm
- **Transaction status**, pending -> confirmed -> finalized

### Phase 3: Token Swaps (DEX)

Trade one crypto for another (SOL <-> USDC) via decentralized exchange.

- **Implementation:** Jupiter aggregator API (aggregates all Solana DEXes for best price)
- **UI:** Simple swap interface, "Swap [amount] [SOL] -> [USDC]", show rate, confirm
- **Why useful:** Users can convert volatile SOL to stable USDC, or buy SOL with USDC
- **Jupiter API:** Free, no API key needed, returns transaction to sign client-side

### Phase 4: Staking

Lock SOL to help secure the Solana network, earn ~7% annual yield.

- **Implementation:** Native Solana staking via stake program instructions
- **UI:** "Stake [amount] SOL" -> pick validator -> confirm -> show rewards
- **Why useful:** Project treasury earns passive income on idle SOL reserves
- **Minimum:** 0.01 SOL (practically no minimum)

### Phase 5: NFT & Digital Assets

Unique on-chain items (game items, land deeds, certificates, art).

- **Implementation:** Metaplex standard on Solana
- **Use cases for HumanityOS:**
  - In-game items as NFTs (tradeable between players)
  - Skill certifications as on-chain credentials
  - Land/property deeds in the game world
  - Achievement badges
- **UI:** Gallery view of owned NFTs, send/receive, marketplace integration

## Architecture

```
+-- HumanityOS App -----------------------------------------+
|                                                            |
|  Identity (crypto.js)                                      |
|  +-- 32-byte seed (in an Ed25519 CryptoKey, storage only)  |
|  +-- BIP39 recovery phrase (24 words)                          |
|  +-- PKCS8/JWK storage (IndexedDB)                        |
|           |                                                |
|           v                                                |
|  Wallet Module (wallet.js) <-- NEW                         |
|  +-- walletKeyFromSeed(seed) -> key + base58 address       |
|  +-- getBalance(address) -> SOL amount                      |
|  +-- getTokenBalance(address, mint) -> amount               |
|  +-- getTransactions(address) -> history                    |
|  +-- sendSOL(to, amount, privateKey) -> txHash              |
|  +-- sendToken(to, amount, mint, privateKey)                |
|  +-- signTransaction(tx, privateKey) -> signed              |
|           |                                                |
|           v                                                |
|  Solana RPC (JSON-RPC over HTTPS)                          |
|  +-- mainnet-beta.solana.com (default)                      |
|  +-- Helius free tier (100k req/day)                        |
|  +-- Custom RPC (configurable in settings)                  |
+------------------------------------------------------------+
```

## RPC Endpoints (no library needed, just fetch)

```javascript
// Get SOL balance
fetch('https://api.mainnet-beta.solana.com', {
  method: 'POST',
  body: JSON.stringify({
    jsonrpc: '2.0', id: 1,
    method: 'getBalance',
    params: [solanaAddress]
  })
});

// Get USDC balance (SPL token)
fetch('https://api.mainnet-beta.solana.com', {
  method: 'POST',
  body: JSON.stringify({
    jsonrpc: '2.0', id: 1,
    method: 'getTokenAccountsByOwner',
    params: [solanaAddress, { mint: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v' }]
  })
});
```

## Base58 Encoding (no library needed)

```javascript
// Base58 alphabet (Bitcoin/Solana standard)
const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function base58Encode(bytes) {
  // Convert byte array to BigInt, then repeatedly mod 58
  // ~30 lines of code, no dependencies
}

function base58Decode(str) {
  // Reverse of above
}
```

## Security

### Key Protection

- Private key never leaves the device
- Transaction signing happens client-side only
- No private key sent over network (ever)
- The wallet key is derived from the seed under its own label, one way: exporting it exposes the wallet and nothing else (since 2026-11-19; before, it WAS the identity's seed)
- The recovery phrase restores wallet access (same 24 words)

### Transaction Safety

- Confirmation modal for all outgoing transactions
- Show USD equivalent (via CoinGecko price API)
- Show network fee estimate
- "Are you sure?" for amounts > $100
- Double-confirm for amounts > $1000
- Transaction simulation before broadcast (Solana supports this)

### Warnings (displayed prominently)

```
HumanityOS is in active development (pre-v1.0).
Your wallet keys are stored locally on your device.
We cannot recover lost keys or reverse transactions.
Back up your 24-word recovery phrase -- it protects both your identity AND your wallet.
Use at your own risk.
```

## UI Integration

### Profile Page

```
+-- Wallet ---------------------------------+
| SOL  12.45 SOL (~$1,245.00)              |
| USDC $500.00                              |
|                                           |
| Your address:                             |
| [7xK9m...abc123] [Copy]                  |
| [QR Code]                                 |
|                                           |
| [Send] [Receive] [History]                |
+-------------------------------------------+
```

### Settings Page

```
-- Wallet ----------------------------------
  Solana Address    7xK9m...abc123  [Copy]
  Network           [Mainnet]
  Custom RPC        [                    ]
  Show balance      [  ON  ]
--------------------------------------------
```

## Files to Create/Modify

| File | Change |
|------|--------|
| `web/shared/wallet.js` | NEW, Solana wallet module (base58, RPC queries, tx building) |
| `web/shared/wallet.js` | `walletKeyFromSeed()` and `exportSolanaKeypair()`: the wallet key derived from the seed (2026-11-19; crypto.js's old `extractSolanaKeypair`, which exported the seed itself, is gone) |
| `web/pages/settings-app.js` | Add wallet section (address display, network config) |
| `web/chat/chat-profile.js` | No wallet on another person's profile card: an address cannot be worked out from a public identity key (removed 2026-11-19) |
| `web/pages/donate-app.js` | Use derived address instead of hardcoded placeholder |

## Dependencies

- ZERO npm packages
- ZERO build step
- All crypto done via Web Crypto API (already available)
- Base58 encoding: ~30 lines of vanilla JS
- Solana RPC: standard fetch() calls
- Price data: CoinGecko public API (free, no key)

## Relationship to Existing Systems

- **Identity**, Wallet IS the identity. Same keys, same recovery phrase, same recovery.
- **Donations**, Donation page shows the server owner's derived Solana address automatically.
- **Marketplace**, Future: listings can accept SOL/USDC payment directly via wallet.
- **Game Economy**, Future: in-game currency backed by SPL tokens.
- **NFTs**, Future: game items, achievements, certifications as on-chain assets.

## Legal Disclaimer

HumanityOS is not a registered money services business (MSB). The wallet is a self-custody tool, users control their own keys. HumanityOS never has access to user funds. This is equivalent to a user running their own Solana CLI wallet.

Standard self-custody disclaimer (displayed on first wallet use + in settings):

"HumanityOS provides self-custody wallet tools. You are solely responsible for your private keys and recovery phrase. We cannot access, recover, or reverse any transactions. Lost keys mean permanently lost funds. This software is provided as-is with no warranty. Use at your own risk."
