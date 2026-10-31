# humanity-gui

## What it is for

Everything a person sees and touches in the desktop app's pages: every page and widget (`gui/`,
with `GuiState`, the state every page reads, in `gui/mod.rs`), the settings that save them
(`config`, which also holds the identity vault), notifications, and the pages' backends: the Games
and Apps readers (`games`, `software`), the media player (`media`), the readable web (`web_reader`)
and the updater that Settings > Updates drives. It also owns the chat-safety code that sits in
`engine/` but works on the GUI's state (protected mode, warnings, DMs, blocking, reports, reach,
key cards, group reports, file transfer, friend codes, the friend choices, the passes' answers,
household permits, low impact). The native app is the source of truth; the website mirrors it.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:
  - `feature = "native"`: `gui/`, `config.rs`, `notifications/`, `games/`, `software/`, `media/`, `web_reader/`, `updater.rs`, `engine/protected.rs`, `engine/warnings.rs`, `engine/dm.rs`, `engine/block.rs`, `engine/report.rs`, `engine/reach.rs`, `engine/key_cards.rs`, `engine/group_report.rs`, `engine/file_transfer.rs`, `engine/friend_code.rs`, `engine/choice.rs`, `engine/put_answer.rs`, `engine/household.rs`, `engine/low_impact.rs`
- **Size:** 190,121 lines in 374 files, 29,145 of them in 74 test files.
- **Line budgets** (`tests/file_size_ratchet.rs`, in `just lints`; lines now of the budget, closest first): `gui/pages/maps/sky_events.rs` 463 of 465, `gui/pages/maps/ship_view.rs` 406 of 410, `gui/mod.rs` 4,743 of 4,800, `gui/pages/maps/time_bar.rs` 245 of 250, `gui/pages/maps/system_view.rs` 774 of 790, `gui/state_types.rs` 1,812 of 1,850, `gui/loaders.rs` 1,610 of 1,650, `gui/pages/maps/mod.rs` 346 of 355, `gui/pages/maps/overlays.rs` 458 of 470, `gui/pages/maps/camera.rs` 526 of 540, `gui/pages/maps/bodies.rs` 345 of 355, `gui/pages/chat/p2p_groups.rs` 776 of 800, `gui/pages/construction.rs` 4,120 of 4,250, `gui/pages/maps/you_layer.rs` 266 of 275, `gui/pages/maps/body_list.rs` 353 of 365, `gui/pages/maps/scene.rs` 140 of 145, `gui/pages/maps/labels.rs` 154 of 160, `gui/pages/maps/galaxy_view.rs` 380 of 395, `gui/pages/maps/planet_view.rs` 418 of 435, `gui/pages/maps/controls.rs` 120 of 125, `gui/pages/chat/modals.rs` 1,486 of 1,550, `gui/pages/maps/orbits.rs` 278 of 290, `gui/pages/maps/night_sky.rs` 254 of 265, `gui/pages/chat/left_panel.rs` 2,005 of 2,100, `gui/pages/maps/diagram.rs` 114 of 120, `gui/pages/chat.rs` 4,448 of 4,700, `gui/widgets/level_picker.rs` 151 of 160, `gui/pages/maps/stations.rs` 117 of 125, `gui/pages/maps/world_layer.rs` 135 of 145, `gui/pages/maps/belts.rs` 317 of 385, `gui/pages/chat/right_panel.rs` 477 of 600.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `native` 202.
- **May use** (the plan): core, protocol, ship, terrain, systems, relay, net, renderer. **Uses today** (references in non-test code): net 328, core 300, ship 159, systems 157, relay 109, protocol 45, renderer 28, terrain 25.
- **May be used by** (the plan): engine. **Used today by:** engine 487.
- **Wrong-way references:** 0 in code (`node scripts/check-crate-layers.js` keeps it there); 22 in test code, to engine 22: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted").
- **Its lib tests:** 1,302 tests, and this runs exactly them, checked against every test's name:

```
just test-lib gui:: config:: notifications:: games:: software:: media:: web_reader:: updater:: engine::protected:: engine::warnings:: engine::dm:: engine::block:: engine::report:: engine::reach:: engine::key_cards:: engine::group_report:: engine::file_transfer:: engine::friend_code:: engine::choice:: engine::put_answer:: engine::household:: engine::low_impact::
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):

| Item | References | From |
|---|---:|---|
| `gui::GuiState` | 38 | engine 38 |
| `gui::GuiPage` | 15 | engine 15 |
| `config::AppConfig` | 14 | engine 14 |
| `gui::ToastKind` | 14 | engine 14 |
| `gui::pages::chat::norm_server_url` | 14 | engine 14 |
| `gui` | 9 | engine 9 |
| `gui::ChatMessage` | 9 | engine 9 |
| `gui::pages::chat::format_timestamp` | 8 | engine 8 |
| `gui::pages::where_you_live::starts` | 7 | engine 7 |
| `gui::ServerConnection` | 6 | engine 6 |
| `gui::theme::Theme` | 6 | engine 6 |
| `config::Capability` | 5 | engine 5 |
| `gui::ChatChannel` | 5 | engine 5 |
| `gui::ChatUser` | 5 | engine 5 |
| `gui::Place` | 5 | engine 5 |

And 241 more items, 327 references (`node scripts/crate-cards.js --interface humanity-gui` lists them all).
<!-- end generated -->

## Rules

- **Native first; the website mirrors it.** A new UI pattern is built in egui first, then mirrored
  in `web/`; a web feature that adds a pattern asks whether native needs it and either ports it or
  says why not in the design doc. Source: [CLAUDE.md](../../CLAUDE.md), "Rust-first canonical UI",
  "Dual-UI parity" and "Web MIRRORS native"; [ui-system.md](../design/ui-system.md), "Page parity".
- **Colours are theme tokens** from `data/gui/theme.ron`, never `Color32::from_rgb` under
  `src/gui/`; a new colour is a token plus an accessor in `theme.rs` (never a `LEGACY_OFFENDERS`
  entry), and every token is editable in Settings. Source: [CLAUDE.md](../../CLAUDE.md), "One theme
  source", and [ui-system.md](../design/ui-system.md), "Tokens".
- **Every word comes from `data/i18n/en.json` by key** (`t("page.key")`), added with
  `node scripts/i18n-add.js`; every control carries a plain-language hint by key; a jargon word is
  marked `[[term]]` for its glossary card. Source: [CLAUDE.md](../../CLAUDE.md), "One word source",
  and [ui-system.md](../design/ui-system.md), "Words", "Hints" and "The glossary on every page".
- **New widgets go in `src/gui/widgets/`** with a matching CSS class or shared JS component on the
  web, both reading theme tokens. Source: [CLAUDE.md](../../CLAUDE.md), "Universal widgets", and
  [ui-system.md](../design/ui-system.md), "How to add a new widget".
- **Anything a person can configure is reachable in the app,** built in the same increment as the
  feature, or the shell-only debt is logged in [in-app-ops.md](../design/in-app-ops.md). Source:
  [CLAUDE.md](../../CLAUDE.md), "GUI-first configurability".
- **One concept, one word:** "recovery phrase", never "seed phrase"; "no sign-up", never "no
  account"; "Restore from recovery phrase". Source: [CLAUDE.md](../../CLAUDE.md), "Account words",
  held by `tests/terminology_lint.rs`.
- **Say what we will not build, and why, on the screen where the person meets it,** naming whether
  it is a law, a patent or a platform holder's rule. Source: [CLAUDE.md](../../CLAUDE.md), "Saying
  what we will not build, and why".
- **Never trim dev or debug tooling** (the Testing, Bugs, Files and Dev pages, the F2/F3/F4
  overlays) as cleanup. Source: [CLAUDE.md](../../CLAUDE.md), "Forever development".
- **The vault is cryptography:** `config.rs` seals the identity (its row in
  [cryptography.md](../reference/cryptography.md)); read the table before changing it or naming an
  algorithm. The chat-safety files follow [blocking-and-safe-mode.md](../design/blocking-and-safe-mode.md).
- **A page that renders is not yet a page that works:** check a change by interacting with it, not
  only by its snapshot. Source: [ui-system.md](../design/ui-system.md), "Verifying the native UI".

## How to test it

- **Its lib tests:** the `just test-lib` line in At a glance, the headless interaction tests of
  `gui/ui_snapshots.rs` among them (synthetic clicks, no GPU).
- **Snapshots:** `just snapshot <name>` for one page, `just snapshots` for all, `just snapshot-check`
  for "did I break a page I did not touch?". They NEED A GPU: without one each page is skipped and
  the run still exits 0, so read the output and open the PNGs.
- **`just lints`:** `theme_token_lint`, `theme_editor_coverage`, `icon_glyph_lint`,
  `page_registry_lint` (every page in [PAGES.md](../PAGES.md)), `page_parity_lint`,
  `settings_persistence_lint` (a setting survives a restart), `i18n_lint`, `terminology_lint`,
  `emdash_lint`, `snapshot_recipe_lint`.
- **The website's twins, in `just rig-tests`:** [i18n-web.test.js](../../scripts/tests/i18n-web.test.js),
  [hints-web.test.js](../../scripts/tests/hints-web.test.js),
  [glossary-terms.test.js](../../scripts/tests/glossary-terms.test.js),
  [a11y-web.test.js](../../scripts/tests/a11y-web.test.js),
  [contrast.test.js](../../scripts/tests/contrast.test.js), and each page's `-web` test.
- **In the running game:** `just verify-screens` (pages on the wall screens, clicked with nobody at
  the keyboard), `just verify-household` (the Household card), `just verify-starts` (choosing a
  start).

## Open work

- **BUG-217, open:** a task made in the desktop app is never saved. `GuiState::tasks` lives only in
  memory and the server's task list replaces it when it arrives (the engine's `frame_ws_poll.rs`);
  a person's own tasks are Real-side data, to be kept on the device beside the server's shared
  board. Source: [BUGS.md](../BUGS.md) and [two-realities.md](../design/two-realities.md).
- **Accessibility:** no desktop screen reader yet (AccessKit must exist before the window is shown);
  233 desktop Tab stops have no name; a tooltip on every interactive element; a WCAG 2.1 AA audit.
  Source: [PRIORITIES.md](../PRIORITIES.md), TIER 0 NOW item 8 and TIER 3 items 1 and 4.
- **The other limited fields** (a Media item's title and description, a category's name, a note on a
  takedown) still stop typing at their limit; each can take the over-limit widget when its form
  gets a save that can say why. Source: TIER 0 NOW item 8b.
- **The Garden views** (pH, pollination, picking, air, weeds, outlook: about 4.5 ms on the Inventory
  screen's turn) and the per-tile keys, from per-area change signatures in the engine's garden
  mirror. Source: TIER 0 NOW item 12 and [sim-frame-cost.md](../design/sim-frame-cost.md).
- **Stale in PRIORITIES:** TIER 3 item 5 says native has no glossary widget; it has one
  (`widgets/glossary_text.rs`, 2026-10-23). Found 2026-10-31 writing this card.
- **Rung 4 cuts gui,** leaving the engine on top. The engine-folder files it owns move into its crate
  then, and their `crate::engine::` paths change; its test code's wrong-way references (At a glance)
  move first. Pages stay one crate because every page uses `GuiState`. Source:
  [crate-split.md](../design/crate-split.md).

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-engine (the sim runner session): what is needed. Why: the reason.`

None yet.
