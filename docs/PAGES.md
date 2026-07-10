# Pages Registry

> **Re-audited 2026-06-30 (was 2026-05-03).** This is the canonical inventory of every
> UI page in HumanityOS, native (Rust + egui) and web (HTML). Update this file in the
> same commit as adding/removing/renaming a page so the registry never drifts. The
> Pages section in MEMORY.md and CLAUDE.md should defer to this file rather than
> re-listing.
>
> **Latest change (v0.699.0, 2026-07-04):** removed 17 dead `GuiPage` variants — the 5
> category-landing pages (`OverviewReality/Sim/Tools/Settings/Dev`) and the 12
> `Settings*` sub-page variants — leaving **36** variants (35 pages + the `None`
> in-game state). All 17 were unreachable since the v0.196 single-row-nav rewrite;
> Settings content is untouched (it lives in `settings.rs`'s internal router). The same
> release rehomed Calculator + Files into the Platform tab and Trade + Guilds into the
> Real tab so they're reachable again. Earlier: `Agents`, `AiUsage`, standalone
> `Onboarding`, and `Resources` were removed as variants (v0.197.0 / v0.415.0).

## How to use this file

- **Adding a page**: append a row to the relevant table below, plus update `src/gui/mod.rs::GuiPage` (native) or place the file under `web/pages/` (web).
- **Removing/renaming**: update the table, update `GuiPage`, update `src/gui/pages/escape_menu.rs::sub_pages_for()` if the page was nav-listed.
- **Audit drift**: `tests/page_registry_lint.rs` (built 2026-07-02, runs in `just lints`) now mechanically enforces this file against the code: every `GuiPage` variant must be mentioned here, every referenced page file must exist, every `web/pages/*.html` must be listed, and the standalone count in the web-pages heading must equal the real file count. Prose accuracy (purpose text) still needs a human audit pass now and then.

## Native pages (35 `GuiPage` variants, `src/gui/pages/`, plus the `None` in-game/no-menu state)

Source of truth: `GuiPage` enum in `src/gui/mod.rs`.

### Top-level tool pages

| Name | File | Purpose | Audience | Web parity |
|------|------|---------|----------|------------|
| MainMenu | `main_menu.rs` | Title screen: Play, Settings, Quit. First-run identity setup + server connection. | everyone | native-only |
| Settings | `settings.rs` | Router into the 11 Settings sub-pages below (single-page sidebar layout). | everyone | both |
| Inventory | `inventory.rs` | Inventory grid + equipment slots + weight + item details. From `data/inventory/equipment_slots.json`. | everyone | both |
| Tasks | `tasks.rs` | Three-column kanban (Todo / In Progress / Done) with project selector. | everyone | both |
| Maps | `cosmos.rs` (alias) | Solar system orbit view + planet details, sidebar list grouped by type. `GuiPage::Maps` has forwarded to `cosmos::draw` since v0.203.2 -- same rendered page as the Cosmos row below, reached via a different nav label (Reality overview calls it "Maps", Sim overview calls it "Cosmos"). The standalone maps.rs file was dead code (zero callers, found 2026-07-01) and was DELETED in the 2026-07-02 sweep. | everyone | both |
| Market | `market.rs` | Marketplace: browse, search, create listings. Sidebar category filter, card grid. | everyone | both |
| Profile | `profile.rs` | Player profile with privacy-tiered sidebar sections. | everyone | both |
| Civilization | `civilization.rs` | Community/colony stats: 3-col grid, trends, charts, timeline. | everyone | both |
| Chat | `chat.rs` | 3-panel chat: server/channel browser, messages, member list. | everyone | both |
| Calculator | `calculator.rs` | Full scientific calculator + history. | everyone | both |
| Notes | `notes.rs` | Notes app with sidebar, editor, toolbar, autosave. | everyone | both |
| Calendar | `calendar.rs` | Month view, event dots, add-event form (time/color/desc). | everyone | both |
| Crafting | `crafting.rs` | Recipes by category, craft queue with progress. From `data/crafting/categories.json`. | everyone | both |
| Wallet | `wallet.rs` | SOL balance, address copy, send form, transaction history. Solana RPC proxy. | everyone | both |
| Guilds | `guilds.rs` | Guild browser, detail view with members + chat, create form. | everyone | both |
| Trade | `trade.rs` | P2P trading with escrow. | everyone | both |
| Files | `files.rs` | Browse + edit text files in `data/`. | dev | both |
| BugReport | `bugs.rs` | Submit bug reports with severity + category. From `data/bugs/taxonomy.json`. | everyone | both |
| Library | `library.rs` | Two faces: DOCUMENTS (Humanity Accord + companions from `data/library/`, nested tree) and a directory of free external tools/websites, in one top-level tab (added v0.373-375; absorbed what used to be the standalone Resources page). | everyone | both |
| Donate | `donate.rs` | Hero + funding goal + donation method cards + FAQ. | everyone | both |
| Tools | `tools.rs` | Open-source tools catalog with search + filters. From `data/tools/catalog.json`. | everyone | both |
| Studio | `studio.rs` | OBS-like broadcasting studio (scenes, sources, properties) with a Program/Preview split (v0.664): scene clicks stage into Preview, "Cut to Program" pushes live. Scene/source management is real UI state; actual capture/encoding/transport is not built yet (see STATUS.md TIER 2). | everyone | native-only, no web page |
| Quests | `quests.rs` | The single quest surface: live in-game quests (auto-track + XP, from `QuestSystem`) render first, then the learn-by-doing self-sufficiency chains (`data/onboarding/quests.json`). Absorbed the old Profile page's game-quests section (v0.415.0). | everyone | both |
| ServerSettings | `server_settings.rs` | Server / group admin (USER / MOD / ADMIN tiered, color-coded). Game-world ban management now lives here too as an ADMIN subsection (`game_admin::draw_section`, folded in v0.479, the standalone `GameAdmin` variant was removed). | admin | native-only |
| Identity | `identity.rs` | DID, Verifiable Credentials, trust score, AI status. | everyone | both |
| Governance | `governance.rs` | Proposals + votes + tally (local + civilization scope). **Native is fully LIVE as of v0.660**: fetches real proposals/tallies from `/api/v2`, casts Dilithium-signed `vote_v1` votes, and submits `proposal_v1` proposals via an in-page form (built with the in-crate ObjectBuilder the relay verifies with). **WEB voting is LIVE too (2026-07-01)**: `governance.html` builds + Dilithium-signs `vote_v1` objects in the browser (`pq-object.js buildVoteV1` over the KAT-locked `canonical-cbor.js`; identity from localStorage via `pq-relay-auth.js getPqIdentity`) and POSTs them to `/api/v2/objects`; byte-equality with the Rust encoder is locked by `just vote-kat` (scripts/vote-object-kat.mjs paired with object.rs::vote_v1_cross_language_kat). Voted proposals persist per identity fingerprint in localStorage. Web still lacks the proposal-CREATION form (native-only for now). | everyone | both |
| Laws | `laws.rs` | Location-aware rules + rights, nested Humanity to locality; HumanityOS base set + real-law summaries. Data: `data/laws/laws.json`. Web mirror `web/pages/laws.html` (shared jurisdiction-chain logic in `web/shared/laws-logic.js`). | everyone | both |
| Recovery | `recovery.rs` | Social key recovery (Shamir M-of-N), guardian setup. | everyone | both |
| Cosmos | `cosmos.rs` | Three-mode astronomical map: System (Sol planets), Galactic (nearby stars in ly), Night Sky (Earth-centered celestial sphere with constellations). Added v0.203.0. | everyone | both |
| Homes | `homes.rs` | Your offline homestead (v0.379): the Fibonacci homestead blueprint as a browsable design, pick a build scale (Solo/Family/Community/Colony), see power/water demand for that scale. | everyone | both |
| Testing | `testing.rs` | QA testing tasks; operator-facing checklist, Mark Passed / Report Issue posts to chat. From `data/testing/qa_tasks.json`. | dev | native-only |
| Dev | `dev.rs` | Developer tools (v0.777, a Platform section not a GuiPage variant): spawn any of the 92 creatures.csv species in front of the player, despawn-all, live creature count; documents the walk-up creature editor (look at a creature, press G). Cheats-gated (Settings toggle). | dev | native-only |
| Browser | `browser.rs` | Curated bookmarks (5 categories); opens links in the OS default browser. Foundation for a future in-app browser (not built). | everyone | web-only |

### Merged super-tabs (fold multiple pages into one `section_nav` sidebar; page carve v0.358-360, still evolving)

These are also top-level `GuiPage` variants, but each one delegates to several of the
pages above rather than being standalone content. Added when the nav was consolidated
from many top-level buttons to a handful of tabs.

| Name | File | Folds in |
|------|------|----------|
| Real | `real.rs` | Renamed "Profile" in the UI (v0.378; the enum variant name `Real` is the internal legacy name). Profile's sections (Body/Identity/Notes/Network/Interests/Skills/Social/Streaming) plus Inventory, Wallet, Tasks, Maps, Market. |
| Platform | `platform.rs` | The software-itself tab: Recovery, Tools, Calculator, Files, Bugs, Testing, Dev, Browser. (Settings was pulled back OUT to its own top-level tab per an explicit operator call: "never buried in another menu.") |
| Humanity | `humanity.rs` | The collective/mission tab: the Mission Dashboard (the real landing content) plus Governance, Identity (as "Directory"), Onboarding, Donate, Library (as "Resources"). |

### Category-landing pages + Settings sub-page variants — REMOVED v0.699.0

The 5 category-landing pages (`OverviewReality/Sim/Tools/Settings/Dev`) and the 12
`GuiPage::Settings*` sub-page variants (`SettingsAccount` .. `SettingsUpdates`) were
**deleted in v0.699.0** as dead code. Both were stranded by the v0.196.0 single-row-nav
rewrite: nothing navigated to an Overview page, and the `Settings*` variants were only
ever reachable as cards on the (unreachable) `OverviewSettings`. Their supporting
helpers (the `category_overview` + `settings_pages` modules, and escape_menu's
`top_categories()` / `sub_pages_for()` / `category_pages()` / `category_meta()`) went
with them. Found in the 2026-07-04 page-access audit.

The **Settings content is unaffected** — it lives in `settings.rs`, which has its own
internal `SettingsCategory` router (Account / Appearance / Animations / Widgets /
Notifications / Wallet / Audio / Graphics / Controls / Privacy / Data / Updates), fully
reachable via the top-level **Settings** tab. Nothing a user could reach was lost.

Same release **rehomed** the working pages that the Overview deletion would otherwise
have orphaned: **Calculator** and **Files** joined the **Platform** tab's section-nav
(alongside Tools / Bugs / Testing / Browser); **Trade** and **Guilds** joined the
**Real** (Profile) tab alongside Market / Wallet. `Civilization` remains a `GuiPage`
variant but is currently unreached (its content overlaps the Humanity tab's Mission
Dashboard — a page-uniqueness decision is pending: wire as a distinct stats view or
retire).

### Not `GuiPage` variants (reached by a flag or a direct call, not the page-nav dispatch)

These have files in `src/gui/pages/` but are NOT rows in the `GuiPage` enum, don't
confuse them with removed pages, they're alive and load-bearing, just reached
differently:

| File | What it actually is |
|------|----------------------|
| `construction.rs` | The in-app Construction/Build Editor (homestead walls, utilities, mothership superstructure, see `docs/STATUS.md`'s Construction section). Gated by `GuiState.construction_active: bool`, an overlay drawn alongside whatever page is active, not a page of its own. |
| `showroom.rs` | The character-select/appearance-editor panel. Gated by `GuiState.showroom_active`, drawn directly from `src/lib.rs`'s render loop (`pages::showroom::draw`), not via `GuiPage` dispatch. |
| `diagnostics.rs` | F-key dev-HUD overlays (F2 performance, F3 network, F4 system), stacked in the corner, shown alongside any page. |
| `keymap.rs` | Key-rebind data/UI consumed by the Settings "Controls" section (`settings.rs`), not a standalone page. |
| `game_admin.rs` | Game-world ban management, folded into `ServerSettings` > ADMIN as a subsection (v0.479; the old standalone `GameAdmin` variant was removed). |
| `onboarding.rs` | Shared drawing helper (`onboarding::draw_quests`) consumed by `Quests`; not a standalone page since the `Onboarding` variant was removed (v.415.0). |
| `hud.rs` | In-game HUD (health, hotbar, crosshair, compass, FPS, weather), drawn during gameplay, not a menu page. |
| `vendor.rs` | Trading-post buy/sell modal (v0.747, closure ladder rung 3). Opened from the trading post machine's walk-up card, gated by `GuiState.vendor_open`; prices from `data/trade_goods.ron`, settles via `economy::vendor_buy/sell` in lib.rs's frame bridge. |
| `escape_menu.rs` | Shared nav bar (colour-coded by category) rendered across all tool pages. (Its dead `top_categories()`/`sub_pages_for()` category-browse helpers were removed v0.699.0 with the Overview pages.) |
| `placeholder.rs` | Utility stub for a not-yet-built page. |
| `mod.rs` | Module root, re-exports. |

**Removed `GuiPage` variants (do not resurrect without checking why they were cut):**
`Agents` and `AiUsage` (v0.197.0, operator: "That AI Agents page also seems useless. As
well as the AI usage." Multi-AI coordination moved to `data/coordination/*` + the relay
`agent_sessions` table); standalone `Onboarding` and `Resources` (v0.415.0, folded into
Quests and Library respectively); `GameAdmin` (v0.479, folded into ServerSettings); the
`Play` variant (v0.415.0, unused, Crafting/Studio are top-level tabs now). `web/pages/ai-usage.html` was DELETED 2026-07-05 in the fluff-trim pass
(dead, zero inbound links). `web/pages/agents.html` is KEPT: a live,
functional agent-coordination dashboard (POST override secured v0.698.0)
linked from README.

## Web pages (`web/pages/*.html`: 36 standalone; audit/ai-usage/dashboard/data/projects removed 2026-07-05)

Web is a superset of native, adds marketing/landing/dev pages that don't need a native counterpart.

| Name | File | Purpose | Audience | Web-only? |
|------|------|---------|----------|-----------|
| Index | `index.html` | Landing page. "Own your tools. Own your life." 3 hero CTAs. | everyone | yes |
| Home | `home.html` | The ideal closed-loop homestead outline (one person, six loops, honestly sized): renders `data/home_outline.json`, the SAME data the native Home page's "ideal closed loop" panel renders -- web mirrors native. Replaced the old localStorage room-decorator (v0.700, operator direction: Home outlines the perfect closed loop AND doubles as the game's Home requirements list). | everyone | both |
| Onboarding | `onboarding.html` | Web's own onboarding flow (native's standalone Onboarding page was removed v0.415.0 and folded into Quests; web was NOT re-checked for the same fold in this pass). | everyone | web-only in practice |
| Download | `download.html` | Desktop binary download + module list. | everyone | yes |
| WalletGuide | `wallet-guide.html` | "?" help page from Wallet. | everyone | yes |
| Roadmap | `roadmap.html` | Public roadmap view, rendered from `data/roadmap.json`. | everyone | yes |
| Dev | `dev.html` | Developer hub. | dev | yes |
| Ops | `ops.html` | Operations / monitoring. | admin | yes |
| Admin | `admin.html` | Admin dashboard. **Read-only** (`admin-app.js` has exactly one `fetch()` call, a GET; no service control, no alert-channel editing, no backup trigger, mutating admin actions require the native exe or SSH). | admin | yes |
| Accord | `accord.html` | The Humanity Accord rendered as a navigable web page (built 2026-07-01 during the fleet redo of the destroyed Accord-page work; registry row added 2026-07-02 when page_registry_lint caught the omission). | everyone | web (native Library page shows the same documents) |
| Shared Files | `shared-files.html` | The public file library (v0.675): browse/search files people shared (3D-printable parts, models). Backed by GET /api/uploads; files enter it by attaching a 3D/model format in chat (`?share=1` on upload). Chat photos stay unlisted. | everyone | web-only (native follow-up: needs a download-manager UX; links open in browser meanwhile) |
| Web | `web.html` | (purpose unclear, TODO audit, carried over unresolved from the last audit) | unknown | yes |

Plus mirrors of native pages: `chat.html`, `inventory.html`, `tasks.html`, `maps.html`, `market.html`, `profile.html`, `civilization.html`, `calculator.html`, `notes.html`, `calendar.html`, `crafting.html`, `wallet.html`, `guilds.html`, `trade.html`, `files.html`, `bugs.html`, `resources.html`, `donate.html`, `tools.html`, `identity.html`, `governance.html`, `laws.html` (jurisdiction-chain + filter logic shared via `web/shared/laws-logic.js`), `recovery.html`, `agents.html` (functional agent dashboard, secured v0.698.0, linked from README), `settings.html`.

**Not mirrored on web at all:** the Construction/Build Editor and Cosmos (both 3D-viewport/gizmo-heavy; web has no wgpu renderer, so a literal mirror isn't the right shape) and the merged super-tabs (Real/Platform/Humanity, web keeps the flatter per-page nav instead). (The 5 category-Overview landing pages and the 12 Settings sub-page variants that used to be listed here were removed from native in v0.699.0; web's `settings.html` remains a single page covering the same Settings ground.)

## Web hubs (entry points outside `web/pages/`)

| Name | File | Purpose |
|------|------|---------|
| Chat hub | `web/chat/index.html` | Cooperative chat with cryptographic identity. |

(The whole `web/activities/` directory was removed 2026-07-05: its hub + gardening
game went in the v0.699.3 trim, and `game.html` -- a redundant "download the app"
page -- in v0.699.4. The game IS the downloaded native app; the Download page's
"Humanity: The Game" module card now reads "Included" instead of linking to a
browser demo.)

## Pages mentioned in docs as "needed but not built"

(Low confidence, based on grep of CLAUDE.md / STATUS.md / FEATURES.md / roadmap. Verify before scheduling work.)

- **Welcome page**, replace the welcome system channel (deleted in v0.126); HOS-managed page with editable content.
- **Rules page**, same shape as Welcome, replaces deleted rules channel.
- ~~Accord page~~ BUILT: `web/pages/accord.html` exists (see the standalone table above).
- **Features page**, auto-generated from a data file; landing page audit recommended this to substantiate the "150+ features" claim.
- **Releases / Changelog page**, public version history.
- **Federation page**, peer server browser + admin (Phase 1 of `docs/design/federation-activation.md`, still unbuilt as of 2026-06-30).
- **Backups page**, backup history + manual trigger.
- **In-app browser page**, full webview (CEF/wry/tauri-style). Currently the `Browser` page is a bookmarks-only stub.
- **Window/chrome custom title bar**, settings or independent overlay (operator-requested, deferred).
- **Multi-monitor manager**, settings sub-page (future).

## Natural groupings (nav-category data, no longer used by a two-tier nav bar)

Source of truth: `escape_menu.rs::sub_pages_for()`. The two-tier nav bar itself was
removed at v0.196.0 ("single-row nav is cleaner"), but this data lives on and now
drives the 5 `Overview*` category-landing pages documented above, this table is kept
here as the canonical grouping reference rather than duplicated.

| Top tier | Color token | Sub-pages |
|----------|-------------|-----------|
| **Reality** | `nav_reality` (red) | Profile, Chat, Wallet, Donate, Tasks, Market, Civilization, Governance, Maps, Recovery, Identity |
| **Sim** | `nav_sim` (purple) | Cosmos, Inventory, Crafting, Studio, Guilds, Trade |
| **Tools** | `nav_tools` (blue) | Calculator, Calendar, Notes, Library, Tools, Browser |
| **Settings** | `nav_settings` (gray) | Account, Appearance, Animations, Widgets, Notifications, Wallet, Audio, Graphics, Controls, Privacy, Data, Updates, Server Admin |
| **Dev** | `nav_dev` (amber) | Testing, Bugs, Files |

Dev visibility is gated by `theme.nav_dev_visible` (default `true` during the development period; toggle in Settings -> Animations -> Developer mode). At v1.0 the default flips to `false` and only shows when the operator opts in.
