# humanity-protocol

## What it is for

What the app and the server must agree on, byte for byte. A crate of its own since rung 1 of the
crate split (2026-11-01, `crates/humanity-protocol/`; it was `src/relay/core/`): the identity and
its signatures, sealing to a person's key, canonical CBOR and the BLAKE3 ids of
signed objects, the signed objects themselves (key cards, moderation, market payloads, profile
copies, recovery through trusted people), the key derivations, the check that the project's
release keys made a release (`release_signature`, 2026-11-02: the desktop updater's verify half,
moved here so the server can check a signed release before the website marks it Signed,
[front-door.md](../design/front-door.md) section 3.1), and the privacy work done on a file
before it leaves a device (stripping and describing its hidden metadata). It is small and
security-critical, so it can be reviewed on its own, and as its own crate it lets the app's network
side (net) and the server (relay) compile side by side.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-protocol/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-protocol/src/describe_metadata.rs`, `crates/humanity-protocol/src/chat_access.rs`, `crates/humanity-protocol/src/did.rs`, `crates/humanity-protocol/src/encoding.rs`, `crates/humanity-protocol/src/error.rs`, `crates/humanity-protocol/src/hash.rs`, `crates/humanity-protocol/src/identity.rs`, `crates/humanity-protocol/src/kdf.rs`, `crates/humanity-protocol/src/key_card.rs`, `crates/humanity-protocol/src/market_payloads.rs`, `crates/humanity-protocol/src/moderation.rs`, `crates/humanity-protocol/src/merkle_disclosure.rs`, `crates/humanity-protocol/src/object.rs`, `crates/humanity-protocol/src/pq_crypto.rs`, `crates/humanity-protocol/src/profile_copy.rs`, `crates/humanity-protocol/src/recovery.rs`, `crates/humanity-protocol/src/release_signature.rs`, `crates/humanity-protocol/src/signing.rs`, `crates/humanity-protocol/src/solana_rpc.rs`, `crates/humanity-protocol/src/strip_heif_metadata.rs`, `crates/humanity-protocol/src/strip_metadata.rs`, `crates/humanity-protocol/src/strip_video_metadata.rs`
- **Size:** 13,559 lines in 29 files, 1,258 of them in 5 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `test-support` 3, `native` 1.
- **May use** (the plan): core. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): relay, net, gui, engine. **Used today by:** relay 141, gui 77, net 32, engine 13.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 180 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-protocol
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_protocol::`):

| Item | References | From |
|---|---:|---|
| `chat_access::MAIN_CHANNEL` | 56 | relay 31, gui 17, engine 8 |
| `did::did_for_pubkey` | 17 | relay 13, gui 4 |
| `object::Object` | 16 | relay 11, gui 4, net 1 |
| `pq_crypto` | 15 | relay 9, net 4, gui 2 |
| `encoding::from_canonical_bytes` | 13 | relay 12, gui 1 |
| `chat_access::PostLevel` | 9 | relay 5, engine 2, gui 2 |
| `pq_crypto::DilithiumKeypair` | 9 | net 5, gui 4 |
| `chat_access` | 7 | relay 5, gui 2 |
| `encoding` | 6 | gui 2, net 2, relay 2 |
| `key_card` | 6 | gui 2, net 2, relay 2 |
| `pq_crypto::derive_dilithium_seed` | 6 | net 4, gui 2 |
| `chat_access::ReadLevel` | 5 | engine 2, gui 2, relay 1 |
| `object::ObjectBuilder` | 5 | gui 3, net 2 |
| `profile_copy` | 5 | relay 4, gui 1 |
| `did` | 4 | relay 4 |

And 48 more items, 84 references (`node scripts/crate-cards.js --interface humanity-protocol` lists them all).
<!-- end generated -->

## Rules

- **Never write or quote an algorithm name from memory.** Read
  [cryptography.md](../reference/cryptography.md), the canonical table, before writing one (in
  code, docs or public copy) and whenever you change cryptographic code here, and update the table
  in the same commit. Source: [CLAUDE.md](../../CLAUDE.md), Cryptography.
- **The website and the Rust code must encode identically.** After changing an encoding, a signed
  object or the words a signature covers, run the cross-language known-answer test that pairs with
  it: `just pq-kat` (the identity derived from the recovery phrase), `just group-kat` (`group_v1`
  and object verification), `just vote-kat` (`vote_v1`, paired with `object.rs`'s
  `vote_v1_cross_language_kat`), `just mod-kat` (`mod_action_v1`, `space_policy_v1`, paired with
  `moderation.rs`'s `cross_language_kat`). Source: the Justfile's comments on each recipe.
- **Every key anything is sealed to comes only from that person's key card,** their Kyber768 /
  ML-KEM-768 key signed by their own Dilithium3 / ML-DSA-65 identity, checked and pinned by the
  sealing app; never from a server's list. Source: [CLAUDE.md](../../CLAUDE.md), Cryptography, and
  [key-cards.md](../design/key-cards.md).
- **Protection is work now, never polish.** The operator, 2026-10-09: "Security is definitely
  something we need to get right. We must protect the users." Safe by default; people open up what
  they want. Source: [CLAUDE.md](../../CLAUDE.md), "Nobody is using the platform yet".
- **Change a format outright, no compatibility code before v1.0,** except the documented active
  migrations (the PBKDF2 100k-to-600k vault migration). When the words a signature covers change,
  servers on the old and new builds refuse each other until both run the new one, so the release
  is followed by the federation live check ([federation-live-check.md](../admin/federation-live-check.md)).
  Source: [CLAUDE.md](../../CLAUDE.md), "No backwards-compatibility debt before launch".
- **It may use only core** (the ratchet in `just lints`, which also holds its Cargo.toml to the
  plan), and its code uses none: only its tests use the core's helpers, through a dev-dependency,
  so it compiles beside the core rather than after it. The root package depends on it under the
  `relay` feature, which `native` turns on, so every change here reaches both builds. Its own
  `native` feature carries the desktop vault's key derivation (`kdf::pbkdf2_sha256`).
- **The release check compiles the project's release public keys in**
  (`data/release/signing_pubkeys.json`, `release_signature::embedded_pubkeys`), and its tests check
  the REAL signed pairs of two releases against them (`tests/fixtures/release/`, kept byte for byte
  by `.gitattributes`): if those fail, the committed keys are not the ones the operator signs
  with, and every desktop app would refuse every update. Never regenerate the fixtures; add a newer
  real pair beside them with `gh release download vX.Y.Z --pattern 'release-manifest.json*'`.
  Source: [cryptography.md](../reference/cryptography.md), Release signing.

## How to test it

- **Its lib tests:** `just test-lib --crate humanity-protocol`, its own test program. A test's
  name is its path in this crate: `kdf::tests::...`, no longer `relay::core::kdf::tests::...`.
- **The server build's battery:** `just verify-relay` (about 200 s; the relay feature set's own lib
  tests, these among them), and `just verify-relay-compiles` before a push.
- **The four known-answer tests** above, whenever their encoder or signed words change.
- **The website's twins,** all in `just rig-tests`:
  [sealed-dm.test.js](../../scripts/tests/sealed-dm.test.js) (the rigs' sealed DM against
  `net/dm_pq.rs`), [friend-pass.test.js](../../scripts/tests/friend-pass.test.js),
  [recovery-web.test.js](../../scripts/tests/recovery-web.test.js) (against `recovery.rs`),
  [profile-copy-web.test.js](../../scripts/tests/profile-copy-web.test.js),
  [strip-metadata.test.js](../../scripts/tests/strip-metadata.test.js) and
  [describe-agree.test.js](../../scripts/tests/describe-agree.test.js) (both apps say the same words
  about a file's hidden information).

## Open work

- **Cut in rung 1 (2026-11-01),** with core: callers name it `humanity_protocol::...`. The fixtures
  other crates' tests share (`profile_copy::fixtures`, `strip_heif_metadata::build`,
  `pq_crypto::fixtures`) sit behind its `test-support` feature. Source:
  [crate-split.md](../design/crate-split.md), rung 1.
- **STARK selective disclosure:** the circuit design is deferred. The hash-based half ships here
  (`merkle_disclosure.rs`, Merkle paths over BLAKE3), verified by the relay's `api_v2_zk.rs`, which
  also holds the STARK scaffold. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 4, item 2.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
