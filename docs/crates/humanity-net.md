# humanity-net

## What it is for

The app's side of the network: the WebSocket client to a server (`ws_client`, `client`), the
shared world's state as it arrives (`sync`, `protocol`), sealed direct messages and their history on
this device (`dm_pq`, `dm_store`), P2P groups (`group_e2ee`), voice and calls (`voice`, `webrtc`,
`call_relay`), device-to-device files (`file_link`, `file_offer`, `file_transfer`), the live stream
and the live weather, and the recovery-phrase identity (`identity`). It also holds the chat-safety
rules with no socket and no GUI in them (`reach`, `block_list`, `protected`, `warnings`, `report`),
so they are tested headless. Its use of the server is almost all the protocol, so it compiles beside
the relay rather than on top of it.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - `feature = "native"`: `net/`
- **Size:** 31,064 lines in 54 files, 3,840 of them in 12 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 44.
- **May use** (the plan): core, protocol. **Uses today** (references in non-test code): core 45, protocol 29.
- **May be used by** (the plan): gui, engine. **Used today by:** gui 328, engine 95.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 8 in test code, to relay 7, engine 1: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 265 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib net:: --skip gui::pages::settings::recovery_net:: --skip terrain::planet::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `net::file_risk::file_type` | 18 | gui 18 |
| `net::warnings::GUARD_LINE` | 13 | gui 12, engine 1 |
| `net::protected::ProtectedAction` | 12 | gui 12 |
| `net::dm_store::DmStore` | 11 | gui 8, engine 3 |
| `net::identity::pq_sign_chat` | 11 | gui 11 |
| `net::dm_pq::DmInner` | 10 | gui 6, engine 4 |
| `net::ws_client::WsClient` | 10 | engine 5, gui 5 |
| `net::webrtc::WebrtcEvent` | 8 | engine 8 |
| `net::protocol::NetMessage` | 7 | engine 7 |
| `net::reach::Refusal` | 7 | gui 7 |
| `net::sync::RemotePlayer` | 7 | engine 6, gui 1 |
| `net::dm_pq::parse_file_marker` | 5 | gui 5 |
| `net::dm_pq::seal_v2` | 5 | gui 5 |
| `net::household::PermitKey` | 5 | engine 5 |
| `net::identity::mnemonic_from_seed` | 5 | gui 5 |

And 179 more items, 289 references (`node scripts/crate-cards.js --interface humanity-net` lists them all).
<!-- end generated -->

## Rules

- **Never write or quote an algorithm name from memory;** read
  [cryptography.md](../reference/cryptography.md) first and update it in the same commit as any
  change to the sealing, signing or vault code here. Source: [CLAUDE.md](../../CLAUDE.md),
  Cryptography.
- **Seal only to a key card,** the person's Kyber768 / ML-KEM-768 key signed by their own Dilithium3
  / ML-DSA-65 identity, checked and pinned here (`key_pins.rs`), never a key from a server's list.
  Source: [key-cards.md](../design/key-cards.md), "Every place that seals to a person".
- **Nothing secret reaches a log,** not a key and not a message's words: the desktop once wrote each
  DM file's decryption key into its log. Source: [BUGS.md](../BUGS.md), BUG-179.
- **This person's network address goes only where they chose.** Calls default to Private (through
  the server's forwarder); a direct connection is the person's own choice, and no third-party
  server is asked by default. Source: [BUGS.md](../BUGS.md), BUG-171 and BUG-173, and
  [PRIORITIES.md](../PRIORITIES.md), TIER 2 item 6 ("How my calls connect").
- **A connection never waits on a receive with a timeout each turn:** nonblocking sockets and a
  short idle sleep. Source: [BUGS.md](../BUGS.md), BUG-201 (and TIER 0 NOW item 8, the voice call
  manager fixed the same way).
- **The website is this code's twin.** `web/chat/` and `web/shared/` must say and check the same
  things, so a rule changed here changes there in the same piece of work, and its `-web` test is
  the proof. Source: [CLAUDE.md](../../CLAUDE.md), "Web MIRRORS native" and "Dual-UI parity".
- **It may use core and protocol only** (the ratchet in `just lints`); its tests bind 127.0.0.1.
  Source: [crate-split.md](../design/crate-split.md) and [CLAUDE.md](../../CLAUDE.md), "No Windows
  Firewall prompts from dev work".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance. The voice harness among them runs two
  apps on 127.0.0.1 (`relay_only_rig_tests`, `call_mode_tests`); CI does not build the native
  feature set, so only `just verify` runs it.
- **The website's twins, in `just rig-tests`:** [sealed-dm.test.js](../../scripts/tests/sealed-dm.test.js),
  [friend-pass-web.test.js](../../scripts/tests/friend-pass-web.test.js),
  [reach-web.test.js](../../scripts/tests/reach-web.test.js),
  [block-web.test.js](../../scripts/tests/block-web.test.js),
  [report-web.test.js](../../scripts/tests/report-web.test.js),
  [warnings-web.test.js](../../scripts/tests/warnings-web.test.js),
  [protected-web.test.js](../../scripts/tests/protected-web.test.js),
  [calls-through-server.test.js](../../scripts/tests/calls-through-server.test.js),
  [voice-rejoin.test.js](../../scripts/tests/voice-rejoin.test.js),
  [p2p-sync-own-devices.test.js](../../scripts/tests/p2p-sync-own-devices.test.js),
  [p2p-direct-offers.test.js](../../scripts/tests/p2p-direct-offers.test.js),
  [file-transfer-web.test.js](../../scripts/tests/file-transfer-web.test.js).
- **In the running game:** `just verify-copresence` (two players see each other move, against a
  throwaway relay) and `just verify-live-screen` (a wall screen watching a stream this rig publishes
  to its own local relay). One game at a time: [game-boots.md](../ai/game-boots.md).

## Open work

- **P2P groups, phases 3 to 5:** P2P transport (the relay only signals), relay independence (a
  group survives a dead home relay), serverless discovery. Source: [PRIORITIES.md](../PRIORITIES.md),
  TIER 2 item 5, and [p2p-groups.md](../design/p2p-groups.md).
- **Privacy, what honestly remains:** signing the call setup with the identity key, so a server that
  altered it would be caught; voice over the WebSocket for networks that block UDP. Source: TIER 2
  item 6.
- **The voice tail:** per-peer volume, mute and squelch, graceful relay restart, the two-str0m
  harness in CI. Source: TIER 2 item 7.
- **Device mesh:** your devices back up each other and the relay. Source: TIER 2 item 10 and
  [device-mesh.md](../design/device-mesh.md).
- **Rung 3 cuts net** with relay and the renderer; its test code's wrong-way references (At a
  glance) move first. Source: [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
