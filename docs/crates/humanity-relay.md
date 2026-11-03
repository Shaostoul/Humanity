# humanity-relay

## What it is for

The server: the relay the VPS runs as `HumanityOS --headless`. It holds the WebSocket and HTTP
handlers (`relay/handlers`, `relay/api.rs` and its siblings), the SQLite storage
(`relay/storage`), federation between servers, the admin action registry (`relay/admin`),
continuous backup (`relay/backup_stream`), the file store, and the How-to data model (`how_to`). The
deploy builds this crate and what it uses on GitHub with no GPU or GUI code in it
(`--features relay --no-default-features`), so it is also the crate whose size decides how long a
deploy takes.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - `feature = "relay"`: `relay/`, `how_to/`
- **Size:** 111,657 lines in 222 files, 18,822 of them in 56 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `relay/relay.rs` 6,309 of 6,340, `relay/api.rs` 3,762 of 3,930, `relay/handlers/profile.rs` 620 of 650, `relay/files.rs` 1,568 of 1,650, `relay/handlers/msg_handlers.rs` 4,373 of 4,660, `relay/api_market.rs` 697 of 800, `relay/handlers/stream.rs` 437 of 550.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `relay` 3, `mesh` 1.
- **May use** (the plan): core, protocol, ship, systems; through those, also terrain. **Uses today** (references in non-test code): protocol 97, ship 39, core 28, systems 10.
- **May be used by** (the plan): gui, engine. **Used today by:** gui 104, engine 11.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 992 tests, and this runs exactly them, checked against every test's name (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter):

```
just test-lib --crate humanity-engine relay:: how_to:: --skip engine::call_relay:: --skip gui::crossing_tests::how_to:: --skip gui::pages::how_to:: --skip net::call_relay::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `relay::storage::ServerSettings` | 18 | gui 18 |
| `relay::storage::RoleDef` | 11 | gui 10, engine 1 |
| `how_to::model::Hazard` | 7 | gui 7 |
| `how_to::model::Audience` | 6 | gui 6 |
| `how_to::library` | 3 | gui 3 |
| `how_to::runs::Run` | 3 | gui 3 |
| `how_to::view::View` | 3 | gui 3 |
| `relay::admin::Registry` | 3 | gui 3 |
| `how_to::library::Library` | 2 | gui 2 |
| `how_to::model` | 2 | gui 2 |
| `how_to::view` | 2 | gui 2 |
| `relay::run_relay` | 2 | engine 1, gui 1 |
| `relay::services::ServiceInfo` | 2 | engine 1, gui 1 |
| `relay::storage::BannedUser` | 2 | engine 1, gui 1 |
| `relay::storage::GameBan` | 2 | engine 1, gui 1 |

And 44 more items, 47 references (`node scripts/crate-cards.js --interface humanity-relay` lists them all).
<!-- end generated -->

## Rules

- **The removed tables stay removed:** `direct_messages`, `listing_messages`,
  `groups`/`group_members`/`group_messages` and `follows`. A feature that seems to need a
  server-side social edge uses the certificate pattern (client-held, statelessly verified). Source:
  [CLAUDE.md](../../CLAUDE.md), "The removed tables stay removed", and
  [storage.md](../ai/storage.md), "Removed tables".
- **Never reference an ALTER-added column from the main schema batch** in
  `src/relay/storage/mod.rs`: on the live database the relay dies at startup while every fresh-DB
  test passes (BUG-046). Source: [storage.md](../ai/storage.md), "ALTER-added columns", held by
  [schema_alter_order_lint.rs](../../tests/schema_alter_order_lint.rs).
- **Account export and erase name only tables that exist.** Source:
  [account_sql_lint.rs](../../tests/account_sql_lint.rs).
- **Every admin action is in the registry** (`data/admin/actions.json`, which both apps and an AI
  read), or named as a member command with its reason; nothing an admin does needs a shell. Source:
  [admin_action_coverage.rs](../../tests/admin_action_coverage.rs),
  [in-app-ops.md](../design/in-app-ops.md) and [CLAUDE.md](../../CLAUDE.md), "GUI-first
  configurability".
- **The server learns only what it must.** The DM mailbox has no sender column and keeps the day
  of arrival only; a privacy finding is work now. Source:
  [cryptography.md](../reference/cryptography.md) ("DM metadata") and [CLAUDE.md](../../CLAUDE.md),
  "Nobody is using the platform yet".
- **A dev relay binds loopback:** `BIND_ADDRESS=127.0.0.1` for a relay run by hand, 127.0.0.1 in
  tests; never change firewall settings to quiet a prompt. Source: [CLAUDE.md](../../CLAUDE.md), "No
  Windows Firewall prompts from dev work".
- **The server build is the one CI deploys:** `just verify-relay-compiles` before every push of
  Rust, and `just verify-relay` after changing anything here. A push to main deploys
  ([release.md](../ai/release.md)). Source: [CLAUDE.md](../../CLAUDE.md), Known gotchas and Quick
  orientation.
- **Moving a server carries its keys:** `data/backup.key` and `data/erased-accounts.key` go with
  `relay.db`. Source: [storage.md](../ai/storage.md), "Moving a server".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance, and in the server's own feature set
  `just verify-relay` (about 200 s, every lib test the relay build compiles).
- **Two relays federating, in one process on real loopback sockets:**
  `cargo test --features relay --no-default-features --test federation_two_relays`
  ([federation_two_relays.rs](../../tests/federation_two_relays.rs); no recipe runs it).
- **Against a real throwaway relay** (a copy of the release exe on a free loopback port with its
  own database): `just verify-second-player` (about 20 s), and the game-booting family
  `just verify-copresence`, `just verify-shared-build`, `just verify-starts`,
  `just verify-household`.
- **`just lints`:** the ALTER-order, account-SQL and admin-coverage lints above.
- **Node tests in `just rig-tests`:** [throwaway-relay.test.js](../../scripts/tests/throwaway-relay.test.js)
  (loopback only), [second-player.test.js](../../scripts/tests/second-player.test.js),
  [relay-follow.test.js](../../scripts/tests/relay-follow.test.js),
  [backup-rotate.test.js](../../scripts/tests/backup-rotate.test.js),
  [backups-admin-web.test.js](../../scripts/tests/backups-admin-web.test.js),
  [admin-erase-web.test.js](../../scripts/tests/admin-erase-web.test.js).
- The cross-language known-answer tests (`just vote-kat`, `just mod-kat`, ...) pair the website's
  encoders with the relay's: see [humanity-protocol](humanity-protocol.md).

## Open work

- **The in-app ops console's remaining slices:** the relay-to-system bridge (fail2ban, relay
  restart, secrets; designed, not built: it installs a sudoers rule and a root helper on the live
  servers), the socket admin messages in the coverage lint, the Backups panel's actions through the
  registry. Source: [PRIORITIES.md](../PRIORITIES.md), TIER 2 item 3, and TIER 0 NOW item 8.
- **Federation activation:** the web mirror of Phase 1, then a per-peer profile-gossip rate limit,
  a second operator-controlled relay federated end to end, vetted peers. Source: TIER 2 item 4 and
  [federation-activation.md](../design/federation-activation.md).
- **`/api/send` per-IP rate limit** (needs X-Real-IP plumbing; low value while the bot path is the
  trusted `API_SECRET` path). Source: TIER 1 item 1.
- **`relay-follow.test.js` is flaky under load.** Source: TIER 0 NOW item 8.
- **Rung 3 cuts relay** with net and the renderer, and measures the server build on GitHub; its
  test code's wrong-way references (At a glance) move first. Source:
  [crate-split.md](../design/crate-split.md), "Order of work".

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
