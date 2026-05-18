# Full-PQ Cutover — Execution Runbook

**Status:** ready to execute (single focused session). Cryptographic
risk already retired + shipped. This is the turnkey checklist so the
finish is mechanical + reviewed, not improvised into the auth core.

**Operator decisions (locked):** full clean fresh-schema wipe (data
loss fine; server not serving); DM = pure ML-KEM-768; Dilithium3 =
identity+signing; Kyber768 = DM; Ed25519 = Solana-wallet only; ONE
scheme, no per-user algo chooser.

---

## Already DONE + shipped + KAT-locked (the only uncertain part)

- `src/net/dm_pq.rs` (v0.262.28) — pure ML-KEM-768 → BLAKE3-KDF →
  AES-256-GCM; recipient key deterministic from the BIP39 seed.
- `web/chat/pq.js` (v0.262.29) — `pqDeriveKyber/pqDmSeal/pqDmOpen`,
  byte-identical to native, proven by `pq_crypto.rs::
  kyber_cross_language_kat` + `scripts/pq-kat.mjs`
  (noble ml_kem768 == RustCrypto). `pqDeriveIdentity/pqSignMessage`
  (Dilithium) already exist + KAT'd.
- `scripts/pq-wipe.sh` + `just pq-wipe yes` (v0.262.30) — double-gated
  fresh-schema wipe, backs up first, executes nothing until invoked.

**Implication:** "does PQ interop work web↔native?" = YES, locked,
cannot silently drift. Everything below is mechanical wiring.

## Wire contract (all 3 surfaces conform exactly)

- `registered_names.public_key` = Dilithium3 pubkey **hex** (THE
  identity; DID derives from it). New col `kyber_public TEXT`
  (base64 ML-KEM-768 encapsulation key). No `ecdh_public` /
  `dilithium_public` cols, no `key_rotations`, no
  `server_settings.require_pq_signatures`.
- `Identify` msg fields: `public_key` (= Dilithium hex),
  `kyber_public` (base64). (Inc3b later: + `challenge_sig` =
  Dilithium sig over a server nonce; tracked separately, do NOT bundle.)
- DM transport (Inc2b.2 SHIPPED — REALITY, supersedes the original
  `{recipient,ek_ct,nonce,ct}` sketch): the relay's `Dm` variant is
  UNCHANGED (`{to, content, encrypted, nonce, ...}`); the relay stays
  zero-knowledge. The full PQ envelope is packed into the opaque
  `content` STRING as JSON — NO relay schema/struct change:
    `content = JSON.stringify({ v:1,
        r:{ek_ct_b64,nonce_b64,ct_b64},   // sealed to RECIPIENT kyber pub
        s:{ek_ct_b64,nonce_b64,ct_b64} }) // sealed to SELF (own kyber pub)`
  `encrypted:true`; top-level `nonce` = `r.nonce_b64` (kept only so the
  existing `msg.encrypted && msg.nonce` guards still trip). **Dual-seal
  is mandatory**: pure ML-KEM is recipient-only (sender keeps no
  shared secret), so without the `s` copy a sender could never read
  their OWN sent messages from server history on any device. Open =
  try `r` then `s` with our own deterministic Kyber secret — covers
  sent/received × both parties × any device. **Inc4 native MUST
  produce/consume this exact `{v:1,r,s}` JSON-in-`content` shape or
  web↔native DM breaks.** Relay change required + SHIPPED: `handle_dm`
  uses a 128 KB `dm_char_limit` when `encrypted` (a dual-sealed 2 KB
  plaintext ≈ 9 KB of base64; char-limiting opaque ciphertext is
  meaningless — the user-visible plaintext cap is enforced client-side
  before sealing). Relay serves each member's `kyber_public` in
  peer_joined/user-list payloads (`kyber_public` field) so a sender
  can encapsulate.
- Chat message signing: Dilithium only (`pq_signature` over
  `content\ntimestamp`); the relay verifies against `public_key`
  (which IS the Dilithium key now). Drop Ed25519 verify + the
  soft/gated dual-sign path.

## Execution order (each step: edit → `cargo check` native+relay →
unit tests → `node scripts/pq-kat.mjs` → ship; clients before relay
deploy; wipe LAST)

### Inc3 relay (the spine — biggest blast radius)
Files: `src/relay/storage/mod.rs` (registered_names CREATE TABLE:
add `kyber_public`; replace the ecdh+dilithium ALTER migrations with
one idempotent `kyber_public` ALTER), `src/relay/storage/dms.rs`
(replace `store/get_ecdh_public` + `store/get_dilithium_public` with
`store/get_kyber_public` + update tests),
`src/relay/relay.rs` (~25 sites: `Peer`/`Identify`/`PeerInfo`/
`UserInfo` structs — replace the `ecdh_public`+`dilithium_public`
pair with a single `kyber_public`; identify handler stores
`kyber_public`; broadcast/user-list serve it; the
`get_dilithium_public(k)` at the pq-verify site → just use `k` since
`public_key` IS the Dilithium key now),
`src/relay/api.rs` + `src/relay/handlers/broadcast.rs` (same field
rename). Net: the relay treats `public_key` as an opaque string, so
this is mostly a mechanical `ecdh+dilithium → kyber` field/fn rename
+ deleting the dual-stack ALTERs. ~25 edits; cargo will list them all.

### Inc2b web `crypto.js` + chat-*.js
- Identity: `getOrCreateIdentity()` derives the BIP39 seed, then
  `pqDeriveIdentity(seed32)` → Dilithium is `myIdentity` (publicKey =
  hex). `generateKeypair()`/Ed25519 path → keep ONLY to derive the
  Solana wallet (already separate). Sign chat via `pqSignMessage`.
- DM: `pqDeriveKyber(seed32)` on connect; send `kyber_public` in the
  identify message; `chat-dms.js`/`chat-messages.js` DM send →
  `pqDmSeal(peerKyberPub, text)` → `{recipient,ek_ct,nonce,ct}`;
  receive → `pqDmOpen(myKyberSecret, ...)`. Peer's `kyber_public`
  comes from the relay member/user payloads.
- DELETE: ECDH keygen/`deriveSharedKey`, the random per-browser ECDH
  vault key, the Settings "import ECDH key" UI/flow, Ed25519-as-chat-
  identity. Vault no longer stores an ECDH private key.

### Inc4 native
`src/lib.rs` / `src/gui/pages/chat.rs` / `src/gui/pages/settings.rs`:
DM path → `crate::net::dm_pq` (`DmPqKeypair::from_bip39_seed`,
`seal`/`open`); identity = Dilithium from seed (pq_crypto); sign
Dilithium. **Delete** `src/net/dm_crypto.rs`, the Settings
ECDH-import UI, `ecdh_private_hex`/`ecdh_public` GuiState fields,
`from_pkcs8_base64`.

### Inc5 trim + docs
Delete: dead Ed25519-identity code, `require_pq_signatures`
(server_settings col + relay enforcement + Server-Settings UI row),
the soft/gated dual-sign verify + `pq_dualsign` telemetry, the
Ed25519↔Dilithium map, `key_rotations`. Update CLAUDE.md crypto
table (reality), STATUS.md/FEATURES.md, this doc → "DONE".

### Inc6 review + cutover (ATTENDED — the only human-gated part)
1. `security-review` skill on the full crypto/relay diff. Fix HIGH/MED.
2. Ship all increments; CI deploys the new relay (current clients are
   now incompatible — fine, not serving).
3. `just pq-wipe yes` → fresh schema.
4. **Operator live-verify (only a human with 2 clients can):** web
   onboard from seed → native onboard from SAME seed → both show same
   identity → send DM web→native AND native→web → both decrypt.
   Re-onboard a 2nd identity, DM between them. If any fail: restore
   the pre-wipe backup, do not declare done.
5. On green: tag, release, build-game, mark this doc DONE.

## Rollback

Every pre-cutover step is non-destructive source (revert commit).
The wipe is reversible from the timestamped `backups/relay-PREWIPE-*`
the script makes (stop relay; cp back to `data/relay.db`; start). The
only true point of no return is declaring DONE after step 6.4 passes.

## Why this is NOT an interleaved-autonomous task

It is the platform's auth core; the final correctness gate (6.4) is a
two-client live handshake a human must watch. Done as one focused
reviewed pass it is ~2–3 hrs and safe. Dribbled across exhausted
context it risks a silent auth break — the exact production-incident
pattern at the worst layer. The uncertain risk is already retired, so
pausing here is safe; the finish is now a checklist, not a gamble.
