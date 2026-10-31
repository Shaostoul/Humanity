# humanity-protocol

## What it is for

What the app and the server must agree on, byte for byte. It is today's `src/relay/core/`: the
identity and its signatures, sealing to a person's key, canonical CBOR and the BLAKE3 ids of
signed objects, the signed objects themselves (key cards, moderation, market payloads, profile
copies, recovery through trusted people), the key derivations, and the privacy work done on a file
before it leaves a device (stripping and describing its hidden metadata). It is small and
security-critical, so it can be reviewed on its own, and as its own crate it lets the app's network
side (net) and the server (relay) compile side by side.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - `feature = "relay"`: `relay/core/`
- **Size:** 12,645 lines in 25 files, 1,193 of them in 4 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 1.
- **May use** (the plan): core. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): relay, net, gui, engine. **Used today by:** relay 97, gui 45, net 29.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 2 in test code, to net 1, relay 1: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 162 tests, and this runs exactly them, checked against every test's name:

```
just test-lib relay::core::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `relay::core::did::did_for_pubkey` | 17 | relay 13, gui 4 |
| `relay::core::object::Object` | 16 | relay 11, gui 4, net 1 |
| `relay::core::pq_crypto` | 15 | relay 9, net 4, gui 2 |
| `relay::core::encoding::from_canonical_bytes` | 13 | relay 12, gui 1 |
| `relay::core::pq_crypto::DilithiumKeypair` | 9 | net 5, gui 2, relay 2 |
| `relay::core::encoding` | 6 | gui 2, net 2, relay 2 |
| `relay::core::key_card` | 6 | gui 2, net 2, relay 2 |
| `relay::core::pq_crypto::derive_dilithium_seed` | 6 | net 4, gui 2 |
| `relay::core::object::ObjectBuilder` | 5 | gui 3, net 2 |
| `relay::core::pq_crypto::verify_dilithium` | 5 | relay 4, net 1 |
| `relay::core::profile_copy` | 5 | relay 4, gui 1 |
| `relay::core::did` | 4 | relay 4 |
| `relay::core::kdf::pbkdf2_sha256` | 4 | gui 2, net 2 |
| `relay::core::recovery` | 4 | gui 2, relay 2 |
| `relay::core::did::fingerprint_to_hex` | 3 | relay 3 |

And 35 more items, 53 references (`node scripts/crate-cards.js --interface humanity-protocol` lists them all).
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
- **It may use only core** (the ratchet in `just lints`), and it compiles only with the `relay`
  feature, which `native` turns on (`Cargo.toml`), so every change here reaches both builds.

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance.
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

- **Rung 1 cuts protocol with core,** the first two crates of the workspace. Its test code's
  wrong-way references (At a glance) move first, and its `pub(crate)` items used across the line
  become `pub`. Source: [crate-split.md](../design/crate-split.md), "Order of work".
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
