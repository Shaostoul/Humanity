# Cryptocurrency Exchange & Payment Layer

**HumanityOS — Payment Infrastructure Design**
**Version 1.0 — March 2026**

Non-custodial payment layer enabling donations (crypto + USD) and peer-to-peer value transfer, built on top of HumanityOS's existing Ed25519 identity system.

---

## 1. Goals & Constraints

1. **Accept donations** — one-time and recurring, in crypto and USD.
2. **Zero/near-zero fees** — every cent should reach the recipient.
3. **P2P transfers** — users send value to each other directly.
4. **Non-custodial** — the server NEVER holds private keys or custodies funds.
5. **Identity reuse** — derive wallet addresses from existing Ed25519 keypairs where possible.
6. **No build step** — client libraries loaded via `<script>` tags or ESM imports, consistent with the rest of HumanityOS.
7. **CC0 compatible** — no proprietary dependencies in core payment logic.

---

## 2. Fee Analysis

### 2.1 Chain-by-Chain Comparison

| Path | Typical Fee | Notes |
|------|------------|-------|
| **Bitcoin Lightning Network** | ~1 sat (~$0.001) | Requires channel management; near-instant finality. Inbound liquidity can be a problem for new nodes. |
| **Bitcoin on-chain** | $0.50–$5.00 (varies wildly) | Slow (10–60 min), fee spikes during congestion. Not suitable for small transfers. |
| **Solana** | ~$0.00025 per tx | Fast (~400ms finality). Ed25519 native — direct key reuse. Priority fees add ~$0.001–0.01 during congestion. |
| **Ethereum L1** | $0.50–$50+ | Unusable for small transfers. |
| **Arbitrum (ETH L2)** | $0.01–$0.10 | EVM-compatible. Requires bridge from L1. 7-day withdrawal to L1. |
| **Optimism (ETH L2)** | $0.01–$0.10 | Similar to Arbitrum. OP Stack ecosystem. |
| **Base (ETH L2)** | $0.001–$0.05 | Coinbase-backed. Cheapest ETH L2 for most operations. No withdrawal delay with fast bridges. |
| **USDC on Solana** | ~$0.00025 | Same as native SOL. Stablecoin — no volatility risk. |
| **USDC on Base** | $0.001–$0.05 | Cheap stablecoin transfers with EVM compatibility. |
| **USDC on Ethereum L1** | $2–$20 | Prohibitive for donations under ~$100. |
| **Stripe (USD)** | 2.9% + $0.30 | Standard card processing. A $5 donation loses $0.45 (9%). A $100 donation loses $3.20 (3.2%). |
| **Stripe (ACH/bank)** | 0.8%, capped at $5 | Better for larger amounts. 3–5 business day settlement. |
| **GitHub Sponsors** | 0% fee | GitHub absorbs all payment processing. Only available for open-source projects/individuals. Payouts via Stripe Connect. |
| **Open Collective** | 0% platform fee (fiscal host takes 0–10%) | Transparent finances. Good for nonprofits. |
| **PayPal donations** | 2.89% + $0.49 | Worse than Stripe for small amounts. |

### 2.2 Cheapest Path by Use Case

| Use Case | Recommended Path | Effective Fee |
|----------|-----------------|---------------|
| **Small donation ($1–$20)** | GitHub Sponsors | 0% |
| **Large donation ($100+)** | USDC on Solana or GitHub Sponsors | ~$0.00025 or 0% |
| **Recurring donation (USD)** | GitHub Sponsors or Stripe ACH | 0% or 0.8% |
| **Recurring donation (crypto)** | USDC on Solana + scheduled client-side signing | ~$0.00025/tx |
| **P2P transfer (crypto)** | SOL or USDC on Solana | ~$0.00025 |
| **P2P transfer (small, instant)** | Bitcoin Lightning | ~$0.001 |
| **P2P transfer (EVM ecosystem)** | USDC on Base | ~$0.005 |

### 2.3 Honest Assessment

- **True zero-fee** is only possible through GitHub Sponsors (they eat the cost) or direct crypto transfers where the user pays the negligible network fee.
- **Solana is the clear winner** for HumanityOS: Ed25519 native (key reuse), sub-cent fees, fast finality, and stablecoin support (USDC/USDT).
- **Lightning Network** is great for Bitcoin maxis but adds operational complexity (channel management, liquidity).
- **USD donations** should route through GitHub Sponsors first (0%), with Stripe as fallback for users who want a direct donation page.

---

## 3. Architecture

### 3.1 Key Derivation from Ed25519 Identity

HumanityOS already generates Ed25519 keypairs for identity. Solana uses Ed25519 natively, so the same keypair can directly serve as a Solana wallet.

```
Existing HumanityOS identity:
  privateKey: Uint8Array(32)   ← Ed25519 seed
  publicKey:  Uint8Array(32)   ← Ed25519 public key

Solana wallet:
  Keypair = { secretKey: Uint8Array(64), publicKey: PublicKey }
  secretKey = concat(seed, publicKey)   ← Solana's format
  address = base58(publicKey)           ← Solana address
```

**The user's Solana address IS their public key**, just base58-encoded. No additional key generation needed.

#### Code Example: Derive Solana Wallet from Existing Identity

```js
import { Keypair } from '@solana/web3.js';
import bs58 from 'bs58';

function solanaKeypairFromIdentity(identity) {
  // Solana expects 64-byte secret key = [seed(32) || pubkey(32)]
  const secretKey = new Uint8Array(64);
  secretKey.set(identity.privateKey, 0);       // 32-byte seed
  secretKey.set(identity.publicKey, 32);        // 32-byte public key
  return Keypair.fromSecretKey(secretKey);
}

function solanaAddressFromPublicKey(publicKeyBytes) {
  return bs58.encode(publicKeyBytes);
}

// Usage:
// const keypair = solanaKeypairFromIdentity(myIdentity);
// const address = keypair.publicKey.toBase58();
// "7xK9m..." — this IS the user's payment address
```

### 3.2 Multi-Chain Key Derivation

For chains that do NOT use Ed25519 (Ethereum, Bitcoin), keys must be derived separately:

| Chain | Curve | Derivation Strategy |
|-------|-------|-------------------|
| **Solana** | Ed25519 | Direct reuse of identity keypair |
| **Ethereum/Base/Arbitrum** | secp256k1 | HKDF from Ed25519 seed → secp256k1 private key |
| **Bitcoin (on-chain)** | secp256k1 | Same HKDF derivation, BIP-32 path |
| **Bitcoin Lightning** | secp256k1 | Delegate to external Lightning wallet (not self-derived) |

```js
/** Derive a secp256k1 key from the Ed25519 seed for EVM chains */
async function deriveEvmKey(ed25519Seed) {
  // HKDF-SHA256: extract-then-expand
  const ikm = ed25519Seed;                            // 32 bytes
  const salt = new TextEncoder().encode('HumanityOS-EVM-v1');
  const info = new TextEncoder().encode('secp256k1-private-key');

  const keyMaterial = await crypto.subtle.importKey(
    'raw', ikm, 'HKDF', false, ['deriveBits']
  );
  const derived = await crypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt, info },
    keyMaterial, 256
  );
  return new Uint8Array(derived); // 32-byte secp256k1 private key
}
```

**Important**: Multi-chain derivation means a single seed compromise exposes ALL derived wallets. This is acceptable because the Ed25519 seed is already the root of trust in HumanityOS.

### 3.3 Non-Custodial Design

```
┌─────────────────────────────────────────────────────┐
│                    CLIENT (Browser)                  │
│                                                      │
│  ┌──────────┐   ┌──────────────┐   ┌──────────────┐ │
│  │ Ed25519  │──▶│ Wallet Keys  │──▶│ Transaction  │ │
│  │ Identity │   │ (derived)    │   │ Signing      │ │
│  └──────────┘   └──────────────┘   └──────┬───────┘ │
│                                           │         │
│  ┌──────────────────────────────┐         │         │
│  │ Encrypted Tx History (local) │         │         │
│  └──────────────────────────────┘         │         │
└───────────────────────────────────────────┼─────────┘
                                            │ Signed tx
                                            ▼
                              ┌──────────────────────┐
                              │   Blockchain RPC      │
                              │   (Helius, Alchemy)   │
                              └──────────────────────┘

┌─────────────────────────────────────────────────────┐
│               RELAY SERVER (Rust/axum)               │
│                                                      │
│  - Routes payment-related WebSocket messages          │
│  - Stores NO private keys                            │
│  - Stores NO balances                                │
│  - Optional: caches tx confirmations for UX          │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**The server's role in payments is limited to**:
1. Relaying payment request/confirmation messages between users (over existing WebSocket).
2. Optionally caching transaction confirmation status (public blockchain data).
3. Hosting the donation page UI.

### 3.4 Balance Display Without Full Nodes

Use third-party RPC providers with free tiers:

| Provider | Free Tier | Best For |
|----------|-----------|----------|
| **Helius** | 100k requests/day | Solana (recommended — best DX) |
| **QuickNode** | 50 req/sec, 10M credits/mo | Multi-chain |
| **Alchemy** | 300M compute units/mo | Ethereum L2s |
| **Solana public RPC** | Rate-limited | Fallback only |
| **Infura** | 100k requests/day | Ethereum ecosystem |

```js
/** Fetch SOL balance — client-side, no server involvement */
async function getSolBalance(publicKeyBase58) {
  const resp = await fetch('https://mainnet.helius-rpc.com/?api-key=FREE_KEY', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0', id: 1,
      method: 'getBalance',
      params: [publicKeyBase58]
    })
  });
  const { result } = await resp.json();
  return result.value / 1e9; // lamports → SOL
}
```

---

## 4. P2P Transfers

### 4.1 Flow

```
Alice wants to send 5 USDC to Bob:

1. Alice opens Bob's profile → clicks "Send"
2. Client shows Bob's Solana address (derived from his public key)
3. Alice enters amount → client builds Solana transaction
4. Client signs transaction with Alice's derived Solana keypair
5. Client submits signed tx to Solana RPC
6. Client sends relay message to Bob: { type: "payment_sent", txHash, amount, chain }
7. Bob's client receives message, verifies tx on-chain
8. Bob sees "Alice sent you 5 USDC" with link to explorer
```

### 4.2 Payment Request Messages (WebSocket)

New relay message types:

```json
{
  "type": "PaymentRequest",
  "target": "<recipient_public_key_hex>",
  "sender_key": "<sender_public_key_hex>",
  "amount": "5.00",
  "currency": "USDC",
  "chain": "solana",
  "address": "<solana_address>",
  "memo": "For lunch",
  "signature": "<ed25519_sig_of_payload>"
}
```

```json
{
  "type": "PaymentConfirmation",
  "target": "<recipient_public_key_hex>",
  "sender_key": "<sender_public_key_hex>",
  "tx_hash": "5UxK3...",
  "chain": "solana",
  "amount": "5.00",
  "currency": "USDC",
  "signature": "<ed25519_sig_of_payload>"
}
```

These are unicast messages (using existing `target` field), so only the sender and recipient see them.

### 4.3 QR Codes & Contact Cards

Existing P2P contact cards (`chat-p2p.js`) already encode public keys. Extend them to include payment info:

```js
const paymentCard = {
  name: profile.name,
  publicKey: myIdentity.publicKeyHex,
  solanaAddress: solanaKeypairFromIdentity(myIdentity).publicKey.toBase58(),
  // EVM address only if multi-chain is enabled
  evmAddress: ethers.computeAddress(await deriveEvmKey(myIdentity.privateKey)),
  lightningInvoice: null  // optional, user-provided
};

// Encode as QR using existing qrcode.js
QRCode.toCanvas(canvas, JSON.stringify(paymentCard));
```

### 4.4 Transaction History

Stored locally in `localStorage` or IndexedDB, encrypted with the user's passphrase (same AES-256-GCM + PBKDF2 pattern as vault/notes):

```js
const txRecord = {
  id: crypto.randomUUID(),
  direction: 'sent',          // 'sent' | 'received'
  counterparty: peerPublicKeyHex,
  counterpartyName: 'Bob',
  amount: '5.00',
  currency: 'USDC',
  chain: 'solana',
  txHash: '5UxK3...',
  status: 'confirmed',        // 'pending' | 'confirmed' | 'failed'
  timestamp: Date.now(),
  memo: 'For lunch'
};
```

History is never sent to the server. Users can optionally back it up to their encrypted vault blob.

### 4.5 Confirmation UX

```
  pending (tx submitted)
     │
     ├──▶ confirmed (1+ confirmations on-chain)
     │
     └──▶ failed (tx rejected / timed out after 60s)
```

Client polls the RPC for transaction status. Solana confirms in ~400ms so the UX is nearly instant. For EVM chains, poll every 3 seconds until confirmed (typically 2–15 seconds on L2s).

---

## 5. Donation System

### 5.1 One-Time Donations

**Crypto**: Display a Solana address (the project's public key) on the donation page. User sends any SPL token (SOL, USDC, etc.) directly. No intermediary.

**USD**: Link to GitHub Sponsors (0% fee) as primary option. Stripe checkout as secondary option for users who prefer card payments.

```html
<!-- donate.html — donation page integration -->
<section id="donate-crypto">
  <h3>Donate with Crypto (near-zero fees)</h3>
  <p>Send SOL or USDC to:</p>
  <code id="project-solana-address">7xK9m...</code>
  <canvas id="donate-qr"></canvas>
  <p class="fee-note">Network fee: ~$0.00025 per transaction</p>
</section>

<section id="donate-usd">
  <h3>Donate with USD</h3>
  <a href="https://github.com/sponsors/Shaostoul">
    GitHub Sponsors (0% fee — recommended)
  </a>
  <button onclick="openStripeCheckout()">
    Card Payment (2.9% + $0.30 fee)
  </button>
</section>
```

### 5.2 Recurring Donations

| Method | How It Works | Fee |
|--------|-------------|-----|
| **GitHub Sponsors** | Built-in recurring tiers | 0% |
| **Stripe Subscriptions** | Server creates Stripe subscription, charges monthly | 2.9% + $0.30/charge |
| **Crypto (client-scheduled)** | Client stores a recurring reminder; user manually approves each tx | ~$0.00025/tx |
| **Crypto (smart contract)** | Solana program with pre-authorized debit (e.g., token delegation) | ~$0.001/tx + deploy cost |

**Recommended approach**: GitHub Sponsors for USD recurring, client-side reminders for crypto recurring. Smart contract subscriptions are possible but add complexity and audit requirements — defer to Phase 5.

### 5.3 Donation Receipts

When a donation is detected (via RPC polling or webhook):

```json
{
  "type": "DonationReceived",
  "donor_key": "<public_key_hex>",
  "amount": "50.00",
  "currency": "USDC",
  "chain": "solana",
  "tx_hash": "3vRq8...",
  "timestamp": 1742000000000,
  "message": "Thank you for supporting HumanityOS!"
}
```

This could be broadcast to a `#donations` channel (with donor consent) or sent as a unicast acknowledgment.

---

## 6. Security

### 6.1 Private Key Protection

- Private keys exist ONLY in the browser's memory (`CryptoKey` objects where possible, `Uint8Array` otherwise).
- Keys are derived on-demand from the Ed25519 seed and discarded after signing.
- The seed itself is already protected by HumanityOS's existing passphrase-based encryption (AES-256-GCM, PBKDF2 600k iterations).
- **No private key material is ever sent over WebSocket or HTTP.**

### 6.2 Transaction Signing in Browser

**Web Crypto API limitation**: Web Crypto supports Ed25519 signing (used for identity) but does NOT support Solana transaction serialization. We need `@solana/web3.js` for transaction building and signing.

```js
// Transaction signing happens entirely client-side
const transaction = new Transaction().add(
  SystemProgram.transfer({
    fromPubkey: keypair.publicKey,
    toPubkey: new PublicKey(recipientAddress),
    lamports: amount * LAMPORTS_PER_SOL
  })
);
transaction.sign(keypair);  // Signs with derived Solana keypair
const txHash = await connection.sendRawTransaction(transaction.serialize());
```

For EVM chains, use `ethers.js` (or the lighter `viem`) for transaction construction and signing. Both support browser environments.

### 6.3 Phishing Protection

- **Address verification**: When sending to another HumanityOS user, the client derives their expected Solana address from their known public key and compares it to the requested address. Mismatch = warning.
- **Domain binding**: Payment request messages are signed with the sender's Ed25519 key. The relay verifies signatures before forwarding (same as all other messages).
- **Confirmation dialog**: All transactions require explicit user approval with amount, recipient name, and address displayed clearly.
- **No clipboard auto-paste for addresses** — users must explicitly select the recipient from their contact list or manually verify pasted addresses.

### 6.4 Rate Limiting

Extend existing relay rate limiting (Fibonacci backoff) to payment-related message types:

```rust
// In relay.rs — payment messages get stricter limits
match msg_type {
    "PaymentRequest" | "PaymentConfirmation" => {
        // Max 10 payment messages per minute per public key
        if payment_rate_exceeded(sender_key) {
            return Err("Payment rate limit exceeded");
        }
    }
    // ... existing rate limiting for other message types
}
```

### 6.5 Supply Chain Security

`@solana/web3.js` and `ethers.js` are large, widely-used libraries but represent supply chain risk. Mitigations:
- Pin exact versions (no `^` or `~` in version specifiers).
- Vendor the libraries (copy into `client/vendor/`) rather than loading from CDN.
- Subresource Integrity (SRI) hashes if loaded from CDN.
- Review release changelogs before upgrading.

---

## 7. Regulatory Considerations

### 7.1 Money Transmitter Classification

A platform becomes a **Money Services Business (MSB)** or **money transmitter** under FinCEN rules when it:
1. Accepts and transmits money or monetary value, OR
2. Provides money transfer services, OR
3. Acts as a custodian of customer funds.

**HumanityOS avoids classification because**:
- The server never holds, controls, or transmits funds.
- Users sign and submit transactions directly to blockchain RPCs.
- The server only relays messages (payment requests/confirmations), not value.
- This is analogous to a messaging app where users discuss payments — the app itself does not process them.

### 7.2 Key Legal Precedents

- **FinCEN 2019 Guidance (FIN-2019-G001)**: Non-custodial wallet providers that do not have independent control over user funds are generally NOT money transmitters.
- **SEC v. Coinbase (2023–2024)**: Custodial exchanges are securities intermediaries; non-custodial protocols are treated differently.
- **EU MiCA Regulation (2024)**: Crypto-asset service providers (CASPs) must register, but non-custodial software wallets are excluded.

### 7.3 What WOULD Trigger Compliance Requirements

Do NOT implement any of the following without legal review:
- **Fiat on/off ramps** (converting USD ↔ crypto) — this makes you a money transmitter.
- **Custodial wallets** (server holds keys on behalf of users).
- **Order book / matching engine** (buying/selling crypto = exchange).
- **Token swaps** (even non-custodial DEX aggregation may trigger requirements in some jurisdictions).
- **Staking-as-a-service** (SEC has taken enforcement action on this).

### 7.4 KYC/AML Thresholds

As a non-custodial platform, KYC/AML obligations are minimal. However:
- If adding Stripe for USD donations, Stripe handles KYC for their side.
- If the project itself receives donations above $10,000 in aggregate, standard nonprofit/business reporting applies.
- Individual P2P crypto transfers have no KYC requirement (same as handing cash to someone).
- If the platform ever integrates fiat conversion, the $3,000 threshold for identity verification and $10,000 for CTR filing would apply.

### 7.5 Recommended Legal Steps

1. Consult a fintech attorney before launching Phase 3 (P2P transfers) — even non-custodial platforms get regulatory scrutiny.
2. Add clear terms of service stating the platform does not custody funds.
3. Implement sanctions screening if operating internationally (OFAC SDN list).
4. Keep the donation page compliant with state charitable solicitation laws if accepting USD donations.

---

## 8. Implementation Phases

### Phase 1: Wallet Address Display (2–3 weeks)

**Goal**: Users can see their Solana address derived from their existing identity.

- Derive Solana keypair from Ed25519 identity in `crypto.js`.
- Display Solana address in profile panel (`chat-profile.js`).
- QR code for the address (reuse existing `qrcode.js`).
- Copy-to-clipboard button.
- No transactions yet — display only.

**Files modified**: `crypto.js`, `chat-profile.js`
**Dependencies**: `@solana/web3.js` (for PublicKey + base58 — or just use a standalone base58 encoder, ~200 bytes)
**Risk**: Low. No funds involved.

### Phase 2: Accept Donations (1–2 weeks)

**Goal**: The project can receive crypto and USD donations.

- Create `donate.html` page with project Solana address + QR code.
- Add GitHub Sponsors link (0% fee primary option).
- Optional: Stripe checkout integration for card payments.
- Display recent donations (poll project wallet via Helius RPC).
- Add `#donations` channel for community visibility.

**Files modified**: New `donate.html`, `shell.js` (nav entry)
**Dependencies**: Helius free API key, GitHub Sponsors enrollment
**Risk**: Low. Project receives funds, no user funds at risk.

### Phase 3: P2P Transfers (4–6 weeks)

**Goal**: Users can send SOL and USDC to each other.

- Load `@solana/web3.js` in client (vendored, ~400KB gzipped).
- Transaction building and signing in browser.
- Payment request/confirmation relay messages.
- Send dialog in DM/profile views.
- Transaction history (encrypted, local-first).
- Balance display (RPC polling, cached in memory).

**Files modified**: New `client/payments.js`, `chat-dms.js`, `chat-profile.js`, `chat-p2p.js`, `relay.rs` (new message types)
**Dependencies**: `@solana/web3.js`, Helius RPC
**Risk**: Medium. Real funds involved — needs security review and testing on devnet first.

### Phase 4: Multi-Chain Support (4–8 weeks)

**Goal**: Support Ethereum L2s (Base, Arbitrum) and Bitcoin Lightning.

- HKDF key derivation for secp256k1 chains.
- `ethers.js` or `viem` for EVM transaction signing.
- Chain selector in send dialog.
- Lightning: integrate with LNbits or similar (user provides their own Lightning address).
- Unified transaction history across chains.

**Files modified**: `payments.js` (expand), `crypto.js` (add HKDF derivation)
**Dependencies**: `ethers.js` or `viem`, Lightning wallet integration
**Risk**: Medium-high. Multiple chains = larger attack surface. Each chain needs independent testing.

### Phase 5: Recurring & Subscription Payments (6–10 weeks)

**Goal**: Automated recurring crypto donations and subscription-style payments.

- Client-side recurring reminders with one-click approve.
- Explore Solana token delegation for true auto-debit (requires SPL Token approve + a crank program).
- Stripe subscription integration for USD recurring.
- Payment dashboard showing active subscriptions.

**Files modified**: `payments.js`, new `payment-subscriptions.js`, potentially a Solana program
**Dependencies**: Solana program development (Anchor framework), Stripe API
**Risk**: High. Smart contract development requires formal audit. Start with client-side reminders.

---

## 9. Recommended Stack

### 9.1 Primary Chain: Solana

**Why Solana first**:
- Ed25519 native — zero-friction key reuse from HumanityOS identity.
- ~$0.00025 per transaction — effectively free.
- ~400ms finality — instant UX.
- USDC natively available — stablecoin support without bridges.
- Large ecosystem, well-maintained JS SDK.
- Free RPC tiers available (Helius: 100k req/day).

### 9.2 Libraries

| Library | Size (gzip) | Purpose | Load Strategy |
|---------|-------------|---------|---------------|
| `@solana/web3.js` | ~400KB | Transaction building, signing, RPC | Vendor in `client/vendor/` |
| `bs58` | ~2KB | Base58 encoding (Solana addresses) | Vendor (or inline — it's tiny) |
| `ethers.js` (Phase 4) | ~120KB (ethers v6 ESM tree-shaken) | EVM transaction signing | Vendor, load only when needed |
| `qrcode.js` | Already loaded | QR codes for payment addresses | Existing dependency |

### 9.3 RPC Providers

| Provider | Free Tier | Chain | Recommended For |
|----------|-----------|-------|-----------------|
| **Helius** | 100k req/day | Solana | Primary Solana RPC |
| **Alchemy** | 300M CU/mo | ETH, Base, Arbitrum | EVM chains (Phase 4) |
| **QuickNode** | 50 req/sec | Multi-chain | Backup / fallback |
| **Solana Foundation** | Public, rate-limited | Solana | Last-resort fallback |

### 9.4 Donation Infrastructure

| Service | Fee | Use |
|---------|-----|-----|
| **GitHub Sponsors** | 0% | Primary USD recurring donations |
| **Solana wallet** | ~$0.00025/tx | Primary crypto donations |
| **Stripe** | 2.9% + $0.30 | Fallback USD card payments |

---

## 10. Open Questions & Tradeoffs

### Should we reuse the identity key as the wallet key?

**Pro**: Simplest UX — no extra key management. Users already back up their identity seed.
**Con**: Tighter blast radius — compromised seed = compromised wallet AND identity.
**Decision**: Yes, reuse. The identity seed is already the single root of trust. Adding a separate wallet seed doubles the backup burden with no real security gain (if the identity seed is compromised, the attacker can impersonate the user anyway).

### Should the relay store any payment data?

**Pro**: Enables cross-device transaction history sync (via encrypted vault blob).
**Con**: Any payment data on the server increases regulatory risk.
**Decision**: Store nothing payment-related on the server. Transaction history lives in the client's encrypted vault blob (existing infrastructure). The relay only forwards ephemeral payment messages.

### Should we support fiat on-ramps?

**Pro**: Users who don't have crypto can still participate.
**Con**: Fiat on-ramps = money transmitter classification. Regulatory nightmare.
**Decision**: No. Link to external services (Coinbase, MoonPay) for users who need to buy crypto. Keep HumanityOS non-custodial and outside MSB scope.

### CDN vs vendored libraries?

**Pro (CDN)**: Smaller repo, browser caching across sites.
**Con (CDN)**: Supply chain risk, availability dependency, CSP complications.
**Decision**: Vendor libraries in `client/vendor/`. Consistent with HumanityOS's no-build-step philosophy and existing CSP configuration.

---

## 11. Privacy Considerations

- **Payment messages are encrypted in transit** (WSS) and are unicast (only sender and recipient see them).
- **Blockchain transactions are public**. Anyone who knows a user's Solana address can see their balance and transaction history. This is an inherent property of transparent blockchains.
- **Mitigation**: Users can generate additional wallet addresses (HD-style derivation from the same seed) for privacy separation. Not implemented in Phase 1–3.
- **USDC on Solana** has no built-in privacy. For privacy-preserving transfers, consider supporting privacy-focused chains in a future phase, but this adds significant regulatory complexity.

---

## 12. Failure Modes & Mitigations

| Failure | Impact | Mitigation |
|---------|--------|------------|
| RPC provider down | Cannot display balance or send tx | Fallback to secondary provider; cache last-known balance |
| User loses seed phrase | Wallet funds lost forever | Existing backup/recovery system (encrypted vault, BIP39 mnemonic) |
| Solana network congestion | Tx delayed or dropped | Retry with priority fee; show clear "pending" status |
| Library supply chain attack | Malicious code steals keys | Vendor + pin versions; SRI hashes; manual review on update |
| Phishing payment request | User sends funds to attacker | Address derivation verification; confirmation dialog with name + address |
| Regulatory action | Platform forced to add KYC/AML | Non-custodial design minimizes exposure; consult attorney proactively |

---

## References

- [FinCEN 2019 Guidance on Virtual Currency](https://www.fincen.gov/sites/default/files/2019-05/FinCEN%20Guidance%20CVC%20FINAL%20508.pdf)
- [Solana Web3.js Documentation](https://solana-labs.github.io/solana-web3.js/)
- [Helius RPC Free Tier](https://www.helius.dev/)
- [GitHub Sponsors FAQ](https://docs.github.com/en/sponsors)
- [EU MiCA Regulation — Wallet Exclusions](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A32023R1114)
- [HumanityOS Identity Architecture](/design/architecture_decisions/client_side_identity_keys.md)
- [HumanityOS Key Management](/design/identity/keys_and_sessions.md)
