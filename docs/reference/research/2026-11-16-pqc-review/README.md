# A post-quantum cryptography review from a contributor (2026-11-16)

rbrasher, who works in post-quantum cryptography, reviewed the repository after the operator's reply on
the project's Discord (2026-11-16) and sent the pointers summarised here, with a list of sources for the
work. This note records them, what was checked in the code the same day, the plan, and the decisions
that are the operator's. Nothing in the cryptography has changed yet except a corrected comment.

## Checked in the code (2026-11-16)

1. **The wallet key is the master secret.** `src/net/identity.rs` `derive_keypair_from_mnemonic` uses
   the recovery phrase's 32 bytes of entropy (`seed_from_phrase`) directly as the Ed25519 signing key,
   which is the Solana wallet (docs/reference/cryptography.md: Ed25519's only roles are the seed source
   and the Solana wallet). The ML-DSA-65 and ML-KEM-768 seeds are derived from the same 32 bytes through
   BLAKE3 `derive_key` under `hum/dilithium3/v1` and `hum/kyber768/v1`
   (`crates/humanity-protocol/src/pq_crypto.rs`). So the Solana private key, exported to any wallet tool,
   is the root of the chat identity and of every past DM. **Confirmed.**
2. **The KEM key cannot be rotated**: it is a pure function of the phrase; if it leaks, the only remedy
   is a new phrase and identity. **Confirmed.**
3. **A stale comment**: `pq_crypto.rs` described the input as BIP39's 64-byte PBKDF2 seed; every caller
   passes the 32-byte entropy, so a BIP39 passphrase is not supported. **Confirmed and corrected**
   (the comment, and docs/reference/cryptography.md).

## The reviewer's recommendations, in his order

1. **A hybrid KEM for DMs first, the contained change**: X-Wing (X25519 + ML-KEM-768) in place of bare
   ML-KEM-768, RustCrypto's `x-wing` natively (built on the `ml-kem` crate already used) and the XWing
   already inside the vendored `@noble/post-quantum` 0.6.1 on the web (`scripts/build-noble-bundle.mjs`
   only has to export it). X-Wing keys come from a 32-byte seed, so the phrase-derived identity can
   stay. Check first: native and web give matching results under a known-answer test (like the existing
   ML-KEM one); the risk of `x-wing` being 0.1.0-rc.0, following draft 06 and unaudited; the envelope's
   version. Never combine the X25519 and ML-KEM secrets by hand: X-Wing on both sides avoids that.
   (Before v1.0 the project keeps no compatibility code, CLAUDE.md, so the envelope changes outright.)
2. **Forward secrecy is a design decision before a library choice.** Today every DM is sealed to a key
   derived from the recovery phrase, which is what lets any device restore all history. Forward secrecy
   deliberately breaks that: history would move between devices by transfer or an encrypted backup.
   Once that trade is accepted, from least to most work: rotating signed prekeys plus a BLAKE3 hash
   chain (close to the group-epoch scheme, but a homemade protocol); OpenMLS (MIT) with Cryspen's
   experimental X-Wing ciphersuite (one-to-one and groups, needs a delivery service and a WebAssembly
   build); Signal's `spqr` (AGPL-3.0, compatible; the strongest reference, not a stable public library).
   First research question: which fits a relay-mailbox model where the peer is often offline.
3. **Signal and Threema as design references, not drop-ins.** Signal's Triple Ratchet (PQXDH then SPQR)
   assumes per-device sessions and server-held prekeys, and libsignal is not packaged for outside use.
   Threema's Ibex reaches forward secrecy without a prekey server (2DH, upgrading to 4DH once the peer
   replies), which suits the mailbox design, and its specifications are explicit about session states,
   rejects, races, counter skips and user warnings; but it is classical Diffie-Hellman and a KEM version
   would be a new protocol needing its own review. Threema's device-group key, history transfer and
   backup specifications are the most useful part: they show how to keep multi-device history once
   forward secrecy is in place.
4. **The identity derivation**: sound in principle (separate BLAKE3 labels, both FIPS standards support
   seed-based keys), with the three weak spots above: derive the Ed25519 key under its own label (wallet
   addresses change, so it needs a migration); derive KEM keys by index under the label, each signed into
   the key card (rotation, and the first step toward signed prekeys); fix the stale comment.
5. **An outside review** of whatever is built; the project's own notes say none has happened since the
   post-quantum switch.

His sources for the work (read each under the house rules: robots.txt first, quotes with dates): the
RustCrypto `x-wing` docs, the X-Wing paper (eprint.iacr.org/2024/039), Cryspen's post on post-quantum
OpenMLS, the IETF draft-mahy-mls-xwing, the `@noble/post-quantum` 0.6.1 README, the SPQR README and
Cryspen's SPQR verification post, Threema's cryptography whitepaper, its Ibex announcement, security
proof and 2023 security analysis, and its statement on the ETH findings. The exact links are in the
operator's Discord thread of 2026-11-16 and the session journal.

## Testing the young crates (his second message, the same day)

His point: the known-answer tests check that the project's two implementations agree with each other,
not that either is correct against the standard. They pin the BLAKE3 sub-seeds and hashes of the
ML-DSA and ML-KEM public keys for one fixed seed (`pq_crypto.rs` tests, `scripts/pq-kat.mjs`), check one
noble signature under RustCrypto's `ml-dsa`, and round-trip sign and verify on the web, all from fixtures
the project's own code made: they catch drift, not non-conformance with FIPS 203 or 204.

**Checked 2026-11-16:** `src/net/dm_pq.rs` `frozen_seed_kyber_pubkey_kat` covers only the KEM public key
from a frozen seed (key generation agreeing); `scripts/pq-kat.mjs` round-trips encapsulation inside
noble only; nothing encapsulates on one side and decapsulates on the other; no `cargo audit`,
`cargo deny` or `npm audit` runs in CI, the scripts or the Justfile. **Confirmed.**

His three additions, now in the plan:

1. **Official vectors in CI**: NIST's ACVP vectors for key generation, encapsulation and decapsulation
   (FIPS 203) and key generation, signing and verification (FIPS 204), against the exact pinned versions,
   web and native.
2. **Cross-implementation tests both ways**: encapsulate in noble and decapsulate in Rust and the
   reverse, and a complete sealed-sender v2 envelope fixture each side must open.
3. **Advisory checks and deliberate upgrades**: `cargo audit` or `cargo vet` and `npm audit`; keep the
   exact-version pins and treat every upgrade as a change that reruns all of the above.

**Both questions answered (read 2026-11-16):**

- **Upstream does run the official vectors.** `ml-dsa` 0.1.1's tests README (docs.rs, the crate's
  source): "The JSON test vector files are taken from the [NIST ACVP repository]. Specifically,
  `key-gen.json`, `sig-gen.json` and `sig-ver.json` are the \"internal projection\" files", taken "from
  commit [65370b8] of that repo". `ml-kem` 0.3.3's: "`key-gen.json` and `encap-decap.json` are the
  \"internal projection\" files from the [ML-KEM key generation test directory][keyGen] and [encap/decap
  test directory][encapDecap]", the same commit. Both also carry `wycheproof.rs`. The published crates
  ship the README and `wycheproof.rs` but not the JSON vector files, so the vectors run in the
  upstream repository, not from the download: our CI still has to run them against our pinned
  versions, as recommended, and noble's side is unchecked.
- **Advisories (RustSec, rustsec.org, read 2026-11-16):** `ml-kem` has no advisory page (the address
  scheme checked against a crate that has one). `ml-dsa` has RUSTSEC-2025-0144 (CVE-2026-22705, CVSS
  6.4 medium, issued 27 January 2026): "A timing side-channel was discovered in the Decompose algorithm
  which is used during ML-DSA signing"; "Patched >=0.1.0-rc.3"; "Integer division was replaced with a
  constant-time Barrett reduction." **The pinned 0.1.1 is past the patch, so not affected.** An
  advisory scan in CI would have said so on its own; it is still the plan.

**His sources for the official vectors** (a third message the same day; read each under the house
rules, robots.txt first, before relying on it):

- NIST: the ACVP-Server release notes (v1.1.0.38, at newreleases.io), the NIST PQC archive FAQ
  (csrc.nist.gov/Projects/Post-Quantum-Cryptography/PQC-Archive), and the pqc-forum thread on ML-KEM
  negative, intermediate and edge-case vectors.
- Vector sets and their users: C2SP CCTV (github.com/C2SP/CCTV), C2SP Wycheproof, Go's
  `crypto/mlkem` Wycheproof test, the `fips203` crate's CCTV README, py-acvp-pqc, and wolfSSL's post on
  ML-KEM and ML-DSA at the CAVP.
- The crates' own test READMEs: `ml-dsa` 0.1.1 and `ml-kem` (0.2.1's README; check 0.3.3's), which
  answer whether upstream already runs ACVP vectors.
- Versions to weigh: `@noble/post-quantum` 0.7.1 is out (the project vendors 0.6.1); the X-Wing
  draft is at -11 (draft-connolly-cfrg-xwing-kem, IETF Datatracker), while RustCrypto's `x-wing`
  0.1.0-rc.0 follows draft 06: whether noble's XWing and `x-wing` follow the same draft is part of the
  known-answer check, and a draft gap would decide which to wait for.

## Decisions that are the operator's

**Decided (operator, 2026-11-16):** "Agree with your recommendations. Best point to do this now is before anyone is using the app. I don't use the wallet so we should be safe to move forward with these when we have usage." So: (1) forward secrecy, with an
encrypted-backup path for history (the recovery phrase restores the identity; old DMs come back by
transfer from another device or from the encrypted backup); (2) the wallet's Ed25519 key derived under
its own label now, before anyone uses the app (no funds to move: the operator does not use the wallet).
Both start at the weekly reset, after the conformance tests.

1. **Forward secrecy versus restoring all history from the phrase.** With forward secrecy, a new device
   restored from the recovery phrase gets your identity but not your old DMs, unless they are moved from
   another device or an encrypted backup. This changes what the recovery phrase promises, so it is his.
2. **The wallet's derivation.** Deriving the Ed25519 key under its own label changes every wallet
   address. Before v1.0 nothing else needs to be kept, but any funds at the old address must be moved
   first, and only by the person who owns them.

## The plan (after the decisions, at the weekly reset)

1. Now, done: the stale comment and the cryptography table's note.
1b. First, before any change to the cryptography: the official ACVP vectors, the cross-implementation
   tests both ways with a full envelope fixture, and the advisory checks in CI (his second message), so
   every change after them is held to the standard, not only to ourselves.
2. The wallet key derived under its own label (after decision 2), with the app saying plainly that the
   address changed and how to move funds from the old one.
3. X-Wing for DMs, after the three checks above; a dated finding on `x-wing`'s pre-release risk and
   licence, and the native/web known-answer test seen red first.
4. KEM keys by index, signed into the key card: rotation.
5. Forward secrecy (after decision 1): research which option fits the offline mailbox, then a plan.
6. An outside review, once the above is built.
