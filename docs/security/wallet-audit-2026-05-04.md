# HumanityOS Wallet/Vault Security Audit — 2026-05-04

> **⚠️ DO NOT COMMIT THIS REPORT PUBLICLY UNTIL CRITICAL ITEMS 1–4 ARE REMEDIATED.**
>
> This document contains concrete exploit paths against unfixed vulnerabilities. Publishing it before the fixes ship gives attackers a roadmap. Keep this file local (or in a private branch) until the BIP39 fix, native vault iteration upgrade, key-rotation UI warning, and CDN-vendoring fix have all landed in main. Once those four are fixed, this report becomes a useful public artifact (good security hygiene to publish post-fix audits).

## TL;DR

The wallet code is **NOT YET SAFE** for a public Solana grant push. Cryptographic primitives — Ed25519 via Web Crypto, ML-DSA-65, Argon2id server-side KDF — are mostly correct. What's broken is integration points and supply-chain hygiene. Four wallet-specific issues will cost users money the moment HumanityOS goes public. The most urgent: the "BIP39" recovery phrase is non-standard and won't restore in Phantom / Solflare / Backpack. Users will think their funds are gone.

**Safe to commit publicly?** **NO** — not until findings 2, 3, 4, and 7 are remediated.

---

## Critical findings (could lose user funds directly)

### 1. Key generation entropy — PASS

`web/chat/crypto.js:51-55` uses `crypto.subtle.generateKey('Ed25519', ...)` and the fallback at line 105 uses `crypto.getRandomValues()`. Native side `src/relay/core/pq_crypto.rs:55-60` uses the `getrandom` crate. No `Math.random()` in any keypair / BIP39 / vault flow.

**No action required.**

### 2. BIP39 incompatibility — CRITICAL

`web/chat/crypto.js:824-877` is **not** real BIP39. It encodes the 32-byte Ed25519 seed directly with an 8-bit SHA-256 checksum. There is no PBKDF2-HMAC-SHA512 mnemonic-to-seed conversion. There is no SLIP-0010 derivation path (`m/44'/501'/0'/0'` for Solana).

A user who exports their 24 words from HumanityOS and imports them into Phantom, Solflare, or Backpack will get a different Solana address showing zero balance. They will think their funds are lost. The wallet-guide copy at `web/pages/wallet-guide-app.js:170` implies standard recoverability — it does not have it.

**Fix path (pick one):**
- Implement standard BIP39 (PBKDF2-HMAC-SHA512, 2048 iterations, salt = "mnemonic" + optional passphrase) + SLIP-0010 derivation at `m/44'/501'/0'/0'` for Solana wallets.
- OR rename the export to **"HumanityOS Recovery Phrase (not Phantom-compatible)"** with explicit warnings everywhere it's surfaced. Provide a HumanityOS-only re-import path. Stop implying standard recoverability.

The first path is the right long-term answer if Solana wallets are a first-class feature. The second is acceptable only if HumanityOS is positioned as having its own non-portable wallet system.

### 3. Vault iteration count + threat model — CRITICAL

- `src/config.rs:13` uses **100,000** PBKDF2-SHA256 iterations on the native vault.
- `web/chat/crypto.js:481` uses **600,000** iterations on the web vault.

The encrypted blob is uploaded via `PUT /api/vault/sync` (`src/relay/api.rs:2197-2226`) and stored opaquely in the `vault_blobs` SQLite table. PBKDF2-SHA256 is GPU-vulnerable. A server breach gives an attacker offline brute-force on every weak passphrase. The server already has Argon2id available (`src/relay/core/kdf.rs`) but the vault sync path does not use it.

**Fix:**
1. Bump native PBKDF2 from 100k to 600k as a one-line stopgap (`src/config.rs:13`).
2. Migrate both client paths to Argon2id with parameters `m=64MiB, t=3, p=4`. Add a `kdf_version` field to the vault blob format so old blobs continue to decrypt while new ones use the modern KDF.

### 4. Key rotation strands SOL — CRITICAL

The rotate modal at `web/chat/crypto.js:1044-1075` warns about "followers and friends linked to your old key" but says **nothing** about Solana funds. Rotation generates a fresh Ed25519 keypair (line 1100), which produces a fresh Solana address (`web/shared/wallet.js:98-102`). The old wallet balance stays at the abandoned address. The server-side handler `src/relay/handlers/msg_handlers.rs:75-113` records the rotation but cannot move funds.

Senders who already have the old address will keep sending into a wallet the user no longer controls.

**Fix:** Show a pre-rotation modal that:
- Reads the current wallet balance.
- If balance > 0, requires the user to send everything to the new address first OR explicitly acknowledge the loss twice.
- If balance = 0, still shows the warning text so users understand the principle.

### 5. PQ migration not implemented client-side, plus CDN supply-chain risk — CRITICAL

`web/shared/pq-identity.js:235-263` exposes `deriveDilithiumSeed` and `pqKeygenFromSeed`, but no caller invokes them in `web/chat/crypto.js`. New accounts get Ed25519 only. When the migration ships, it must preserve the existing Ed25519 wallet seed bit-for-bit — but today's Ed25519 keys come from `crypto.subtle.generateKey` (random), not from a derivable master seed. Migration design needs to extract the existing Ed25519 seed (the path exists at `web/chat/crypto.js:208-216` via `extractSeedFromPkcs8`) and use IT as the BIP39 master, deriving Dilithium3 from the same source.

**Worse:** `web/shared/pq-identity.js:201` lazy-loads `@noble/post-quantum` from `https://esm.sh/`. A third-party CDN compromise on first PQ-key generation = attacker controls every new Dilithium3 keypair generated by every HumanityOS user. This is a one-shot supply-chain attack waiting to happen.

**Fix:**
1. Vendor `@noble/post-quantum` locally. Pin a specific version. Add Subresource Integrity hashes to the script tag.
2. Spec the Ed25519 → dual-stack migration before any client code ships. The migration must be tested against existing accounts that have funds at their Ed25519 wallet address.

### 6. Server-side private key exposure — PASS (with caveat)

The vault flow is opaque to the server:
- Client encrypts at `web/chat/crypto.js:475-487`.
- POST `/api/vault/sync` handler.
- `Storage::store_vault_blob` writes the blob as a string at `src/relay/storage/vault_sync.rs:7-15`.
- Server never imports, parses, or decrypts the vault.

**Caveat:** the Identify message handler at `src/relay/relay.rs:2106-2108` accepts a public key without proving private-key ownership at handshake time. An attacker could open a WebSocket connection claiming to be `<your_pubkey>` and receive inbound messages addressed to that key. Subsequent signed operations would still fail, but session-level metadata leaks.

**Fix:** Add nonce-based challenge-response to the Identify handshake. Server sends a random nonce; client signs nonce with private key; server verifies before accepting the session.

---

## High findings (enables phishing/loss, not direct theft)

### 7. Profile gossip "trusted peer" loophole — HIGH

`src/relay/handlers/federation.rs:99-119` `should_accept_profile_gossip` returns `true` for any inbound profile gossip with empty `signature_hex` from a tier-2-or-higher peer. A compromised admin on a tier-2 federated server can inject profiles for any public key — including planting attacker-controlled wallet hints in the freeform `socials` JSON field of a target user's profile.

Attack scenario: User A is a HumanityOS regular with a Patreon-like donation page where her Solana wallet address is published in her profile. Attacker compromises a tier-2 server. Attacker pushes an unsigned "profile update" for User A's public key, replacing her Solana address with the attacker's. Donors who view her profile now see the attacker's wallet address.

**Fix:**
1. Flip the gate to **require** valid client signatures for all profile gossip — break unsigned profile sync entirely.
2. Until that ships, render an "unverified" badge in profile views when `signed_profiles.signature` is empty so users can detect tampering.

### 8. Wallet UI address truncation in confirm modal — HIGH

`web/pages/wallet.html:428-429` and `web/pages/wallet-app.js:352-357` show truncated 8+4 character addresses in the send-confirm modal. The full address appears only in a hover tooltip. The middle 16 characters (where vanity-grinding attackers reproduce common prefixes and suffixes to fool users) are entirely hidden.

`src/gui/pages/wallet.rs:148-161` has NO send-confirm modal at all. The native wallet shows a "QR Code" placeholder text instead of a real QR code.

**Fix:**
1. Web: show full address in the confirm modal, ideally with monospace font and visual block breaks every 4 characters.
2. Native: implement a real send-confirm flow before any transaction-signing code ships.

### 9. End-to-end test coverage — PARTIAL/HIGH

Good unit-test coverage exists for crypto primitives:
- `src/relay/core/pq_crypto.rs:233-365`
- `src/relay/handlers/federation.rs:582-714`
- `src/relay/core/kdf.rs:111-164`
- `src/relay/core/did.rs:123-205`
- `src/relay/handlers/broadcast.rs:89-105`

**Zero JavaScript unit tests** for `web/chat/crypto.js`. **No integration test** covers the full wallet lifecycle: create → backup → wipe → restore → rotate → spend. `src/config.rs` has no tests at all.

**Fix:**
1. Node integration tests for the JS crypto module (BIP39 round-trip, vault encrypt/decrypt, key rotation cert).
2. Rust integration test for `/api/vault/sync` round-trip.
3. Key-rotation test that builds a real dual-cert and verifies on the server.
4. End-to-end test using throwaway wallets with tiny amounts before any release that touches wallet code.

---

## Top-priority fixes (ordered by severity × ease)

| # | Fix | Effort | Severity |
|---|-----|--------|----------|
| 1 | Vendor `@noble/post-quantum` locally with SRI hashes | 1 day | Critical |
| 2 | Add balance warning to key-rotation modal | 1 day | Critical |
| 3 | Bump native PBKDF2 from 100k → 600k (one-line stopgap) | 1 hour | Critical |
| 4 | Show full address in send-confirm modal (web) + add native confirm | half day + 1 day | High |
| 5 | Decide BIP39 strategy: real BIP39+SLIP-0010 OR rename + warn | 2-5 days for real BIP39 | Critical |
| 6 | Require signed profile gossip; ship "unverified" badge in interim | 1-2 days | High |
| 7 | Migrate both vaults to Argon2id with versioned blob format | 2-3 days | Critical |
| 8 | Add Identify-handshake challenge-response | 1 day | Medium |
| 9 | Spec Ed25519 → dual-stack migration before any client code ships | 2-3 days planning + implementation | Critical |
| 10 | Add the integration tests in finding 9 | 3-5 days | High |

**Realistic timeline before public Solana grant push:** the four critical items can land in ~5-7 focused days of work. Tests and the BIP39 decision can run in parallel. Don't go public until items 1-5 from this table have shipped to main.

---

## Out of scope for this audit

- Solana RPC endpoint hijacking (someone MITMing the user's connection to a Solana node).
- Jupiter swap transaction validation (`web/shared/wallet.js:656-688` doesn't pre-validate that Jupiter-returned transaction instructions match the user-confirmed quote — separate issue worth its own audit).
- NFT transfer flow.
- Native config.json file permissions on disk.
- Mobile fallback path when Ed25519 isn't supported in the browser's Web Crypto.

---

## Audit metadata

- **Performed:** 2026-05-04
- **Method:** Read-only static analysis of source files; no code modified, no commits made.
- **Files reviewed:**
  - `web/chat/crypto.js`
  - `web/shared/pq-identity.js`
  - `web/shared/wallet.js`
  - `web/pages/wallet.html`, `wallet-app.js`, `wallet-guide-app.js`
  - `src/config.rs`
  - `src/relay/core/pq_crypto.rs`, `kdf.rs`, `did.rs`
  - `src/relay/handlers/msg_handlers.rs`, `federation.rs`
  - `src/relay/storage/vault_sync.rs`
  - `src/relay/relay.rs` (Identify handler)
  - `src/relay/api.rs` (vault sync endpoint)
  - `src/gui/pages/wallet.rs`
  - `tests/*` (coverage assessment)
- **Confidence:** High on the critical findings (citations verified against current code). Medium on test-coverage assessment (negative results — "no test exists" — are harder to verify exhaustively than positive findings).
