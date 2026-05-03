# Pages Registry

> **Audited 2026-05-03.** This is the canonical inventory of every UI page in HumanityOS — native (Rust + egui) and web (HTML). Update this file in the same commit as adding/removing/renaming a page so the registry never drifts. The Pages section in MEMORY.md and CLAUDE.md should defer to this file rather than re-listing.

## How to use this file

- **Adding a page**: append a row to the relevant table below, plus update `src/gui/mod.rs::GuiPage` (native) or place the file under `web/pages/` (web).
- **Removing/renaming**: update the table, update `GuiPage`, update `src/gui/pages/escape_menu.rs::sub_pages_for()` if the page was nav-listed.
- **Audit drift**: run `cargo test --test theme_editor_coverage` to confirm no orphan pages and no enum variants without files. (TODO v0.180+: add `tests/page_registry_coverage.rs` to verify this file matches the actual filesystem.)

## Native pages (32 — `src/gui/pages/`)

Source of truth: `GuiPage` enum in `src/gui/mod.rs`.

| Name | File | Purpose | Audience | Status | Web parity |
|------|------|---------|----------|--------|------------|
| MainMenu | `main_menu.rs` | Title screen: Play, Settings, Quit. First-run identity setup + server connection. | everyone | ✓ working | native-only |
| Settings | `settings.rs` | Categories: Account, Appearance, Animations, Widgets, Notifications, Wallet, Audio, Graphics, Controls, Privacy, Data, Updates. Single-page sidebar layout (restructure pending). | everyone | ✓ working | both |
| Inventory | `inventory.rs` | Inventory grid + equipment slots + weight + item details. From `data/inventory/equipment_slots.json`. | everyone | ✓ working | both |
| Tasks | `tasks.rs` | Three-column kanban (Todo / In Progress / Done) with project selector. | everyone | ✓ working | both |
| Maps | `maps.rs` | Solar system orbit view + planet details, sidebar list grouped by type. | everyone | ✓ working | both |
| Market | `market.rs` | Marketplace: browse, search, create listings. Sidebar category filter, card grid. | everyone | ✓ working | both |
| Profile | `profile.rs` | Player profile with privacy-tiered sidebar sections. | everyone | ✓ working | both |
| Civilization | `civilization.rs` | Community/colony stats: 3-col grid, trends, charts, timeline. | everyone | ✓ working | both |
| Chat | `chat.rs` | 3-panel chat: server/channel browser, messages, member list. | everyone | ✓ working | both |
| Calculator | `calculator.rs` | Full scientific calculator + history. | everyone | ✓ working | both |
| Notes | `notes.rs` | Notes app with sidebar, editor, toolbar, autosave. | everyone | ✓ working | both |
| Calendar | `calendar.rs` | Month view, event dots, add-event form (time/color/desc). | everyone | ✓ working | both |
| Crafting | `crafting.rs` | Recipes by category, craft queue with progress. From `data/crafting/categories.json`. | everyone | ✓ working | both |
| Wallet | `wallet.rs` | SOL balance, address copy, send form, transaction history. Solana RPC proxy. | everyone | ✓ working | both |
| Guilds | `guilds.rs` | Guild browser, detail view with members + chat, create form. | everyone | ✓ working | both |
| Trade | `trade.rs` | P2P trading with escrow. | everyone | ✓ working | both |
| Files | `files.rs` | Browse + edit text files in `data/`. | dev | ✓ working | both |
| BugReport | `bugs.rs` | Submit bug reports with severity + category. From `data/bugs/taxonomy.json`. | everyone | ✓ working | both |
| Resources | `resources.rs` | Curated resources directory (Real/Sim aware). From `data/resources/catalog.json`. | everyone | ✓ working | both |
| Donate | `donate.rs` | Hero + funding goal + donation method cards + FAQ. | everyone | ✓ working | both |
| Tools | `tools.rs` | Open-source tools catalog with search + filters. From `data/tools/catalog.json`. | everyone | ✓ working | both |
| Studio | `studio.rs` | OBS-like broadcasting studio (scenes, sources, properties). | everyone | partial | web-only |
| Onboarding | `onboarding.rs` | First-run orientation + permanent reference. From `data/onboarding/quests.json`. | everyone | ✓ working | both |
| ServerSettings | `server_settings.rs` | Server / group admin (USER / MOD / ADMIN tiered, color-coded). | admin | ✓ working | native-only |
| Identity | `identity.rs` | DID, Verifiable Credentials, trust score, AI status. | everyone | ✓ working | both |
| Governance | `governance.rs` | Proposals + votes + tally (local + civilization scope). | everyone | ✓ working | both |
| Recovery | `recovery.rs` | Social key recovery (Shamir M-of-N), guardian setup. | everyone | ✓ working | both |
| Agents | `agents.rs` | Multi-AI scope coordination dashboard. | dev | ✓ working | both |
| AiUsage | `ai_usage.rs` | AI subscription quota tracker + usage log. | dev | ✓ working | both |
| Testing | `testing.rs` | QA checklist; Mark Passed / Report Issue posts to chat. From `data/testing/qa_tasks.json`. | dev | ✓ working | native-only |
| Browser | `browser.rs` | Curated bookmarks (5 categories). Foundation for in-app browser. | everyone | ✓ working | web-only |

**Component files in `src/gui/pages/` (NOT pages):**
- `mod.rs` — module root
- `escape_menu.rs` — RGB-coloured nav bar (shared across all tool pages)
- `hud.rs` — in-game HUD (health, hotbar, crosshair, compass, FPS, weather)
- `placeholder.rs` — utility for unbuilt pages

## Web pages (`web/pages/*.html` — 38 standalone)

Web is a superset of native — adds marketing/landing/dev pages that don't need a native counterpart.

| Name | File | Purpose | Audience | Web-only? |
|------|------|---------|----------|-----------|
| Index | `index.html` | Landing page. "Own your tools. Own your life." 3 hero CTAs. | everyone | yes |
| Home | `home.html` | Logged-in home / dashboard. | everyone | yes |
| Onboarding | `onboarding.html` | Mirrors native Onboarding. | everyone | both |
| Download | `download.html` | Desktop binary download + module list. | everyone | yes |
| WalletGuide | `wallet-guide.html` | "?" help page from Wallet. | everyone | yes |
| Dashboard | `dashboard.html` | Games / activities hub. | everyone | yes |
| Projects | `projects.html` | Project showcase. | everyone | yes |
| Roadmap | `roadmap.html` | Public roadmap view. | everyone | yes |
| Dev | `dev.html` | Developer hub. | dev | yes |
| Data | `data.html` | Data management UI (saves, backups, sync, USB). | dev | yes |
| Ops | `ops.html` | Operations / monitoring. | admin | yes |
| Admin | `admin.html` | Admin dashboard. | admin | yes |
| Web | `web.html` | (purpose unclear — TODO audit) | unknown | yes |

Plus mirrors of every native page: `chat.html`, `inventory.html`, `tasks.html`, `maps.html`, `market.html`, `profile.html`, `civilization.html`, `calculator.html`, `notes.html`, `calendar.html`, `crafting.html`, `wallet.html`, `guilds.html`, `trade.html`, `files.html`, `bugs.html`, `resources.html`, `donate.html`, `tools.html`, `identity.html`, `governance.html`, `recovery.html`, `agents.html`, `ai-usage.html`, `settings.html`.

## Web hubs (entry points outside `web/pages/`)

| Name | File | Purpose |
|------|------|---------|
| Chat hub | `web/chat/index.html` | Cooperative chat with cryptographic identity. |
| Activities hub | `web/activities/index.html` | Game / real-world activities directory. |
| Game | `web/activities/game.html` | "Humanity: The Game" entry. |
| Gardening | `web/activities/gardening.html` | Garden activity. |
| Download (mirror) | `web/activities/download.html` | Mirrors `web/pages/download.html`. |

## Pages mentioned in docs as "needed but not built"

(Low confidence — based on grep of CLAUDE.md / STATUS.md / FEATURES.md / roadmap. Verify before scheduling work.)

- **Welcome page** — replace the welcome system channel (deleted in v0.126); HOS-managed page with editable content.
- **Rules page** — same shape as Welcome, replaces deleted rules channel.
- **Accord page** — Humanity Accord rendered as a navigable page (currently linked as a doc).
- **Features page** — auto-generated from a data file; landing page audit recommended this to substantiate the "150+ features" claim.
- **Releases / Changelog page** — public version history.
- **Federation page** — peer server browser.
- **Backups page** — backup history + manual trigger.
- **In-app browser page** — full webview (CEF/wry/tauri-style). Currently the `Browser` page is a bookmarks-only stub.
- **Window/chrome custom title bar** — settings or independent overlay (operator-requested, deferred).
- **Multi-monitor manager** — settings sub-page (future).

## Natural groupings (used by two-tier nav, landed v0.179.0)

Source of truth: `escape_menu.rs::sub_pages_for()`. Top categories from `top_categories(theme)`.

| Top tier | Color token | Sub-pages |
|----------|-------------|-----------|
| **Reality** | `nav_reality` (red) | Profile, Chat, Wallet, Donate, Tasks, Market, Civilization, Governance, Maps, Recovery, Identity |
| **Sim** | `nav_sim` (purple) | Inventory, Crafting, Studio, Guilds, Trade |
| **Tools** | `nav_tools` (blue) | Calculator, Calendar, Notes, Resources, Tools, Browser |
| **Settings** | `nav_settings` (gray) | Settings, Onboarding, ServerSettings |
| **Dev** | `nav_dev` (amber) | Testing, Bugs, Agents, AiUsage, Files |

Dev visibility is gated by `theme.nav_dev_visible` (default `true` during the development period; toggle in Settings → Animations → Developer mode). At v1.0 the default flips to `false` and only shows when the operator opts in.

Future Dev pages to add as they ship: PerformanceProfiler, NetworkInspector, EntityInspector, LogViewer, ConfigDump.
