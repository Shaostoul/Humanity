# Pages Registry

> **Re-audited 2026-06-30 (was 2026-05-03).** This is the canonical inventory of every
> UI page in HumanityOS, native (Rust + egui) and web (HTML). Update this file in the
> same commit as adding/removing/renaming a page so the registry never drifts. The
> Pages section in MEMORY.md and CLAUDE.md should defer to this file rather than
> re-listing.
>
> **One Files page, 2026-10-19:** the Media page, the developer's Files page (the data/ browser with
> the server's Shared Files) and, on the website, the Shared Files page and the data browser became ONE
> page called **Files**, with PLACES at the top of its left pane: Shared on <server>, Shared files,
> Admin (admins), the HumanityOS folder and, on the desktop, Your folders (operator: "What's the real
> difference between these two? Why would we not want to merge them?", and "I'd rather sync the names
> now while no one is using HOS"). The names follow everywhere: `GuiPage::Files` (page id `files`),
> `src/gui/pages/files.rs` + `files/`, the relay's `src/relay/files*.rs` and `/api/files/*` routes
> (the data-file routes moved to `/api/data-files`), `web/pages/files.html` + `files-*.js` at `/files`,
> `data/files/`. The playback engine `src/media/` keeps its name: it is media in its true sense.
>
> **Latest change (v0.699.0, 2026-07-04):** removed 17 dead `GuiPage` variants — the 5
> category-landing pages (`OverviewReality/Sim/Tools/Settings/Dev`) and the 12
> `Settings*` sub-page variants — leaving **36** variants (35 pages + the `None`
> in-game state) at the time; two pages added since (`RelayControl` v0.846,
> `Watch` v0.857) bring the current total to **38** (37 pages + `None`). All 17 were unreachable since the v0.196 single-row-nav rewrite;
> Settings content is untouched (it lives in `settings.rs`'s internal router). The same
> release rehomed Calculator + Files into the Platform tab and Trade + Guilds into the
> Real tab so they're reachable again. v0.1147 (2026-08-17): `Civilization` variant merged into the Humanity tab (its Mission Dashboard embeds civilization.rs's draw_live_body; the standalone page was reachable only via one onboarding link). Earlier: `Agents`, `AiUsage`, standalone
> `Onboarding`, and `Resources` were removed as variants (v0.197.0 / v0.415.0).

> **Companion:** `docs/reference/ui-navigation-map.md` maps how these pages
> CONNECT (nav trees, both surfaces) and tracks name consistency between
> buttons, headings, and titles (survey + naming rule adopted 2026-08-16).

## How to use this file

- **Adding a page**: append a row to the relevant table below, plus update `src/gui/mod.rs::GuiPage` (native) or place the file under `web/pages/` (web).
- **Removing/renaming**: update the table, update `GuiPage`, update `src/gui/pages/escape_menu.rs::sub_pages_for()` if the page was nav-listed.
- **Audit drift**: `tests/page_registry_lint.rs` (built 2026-07-02, runs in `just lints`) now mechanically enforces this file against the code: every `GuiPage` variant must be mentioned here, every referenced page file must exist, every `web/pages/*.html` must be listed, and the standalone count in the web-pages heading must equal the real file count. Prose accuracy (purpose text) still needs a human audit pass now and then.

## Native pages (34 `GuiPage` variants, `src/gui/pages/`, plus the `None` in-game/no-menu state)

Source of truth: `GuiPage` enum in `src/gui/mod.rs`.

### Top-level tool pages

| Name | File | Purpose | Audience | Web parity |
|------|------|---------|----------|------------|
| MainMenu | `main_menu.rs` | Title screen: Play, Settings, Quit. First-run identity setup + server connection. | everyone | native-only |
| Settings | `settings.rs` | Router into the Settings sections (single-page sidebar layout; the list is the `SettingsCategory` enum in `src/gui/mod.rs`). Account holds **Recovery through trusted people** (2026-10-20, `settings/recovery.rs`, after Security): what it is, that it is not finished (choosing guardians and splitting the phrase are not built), and its two working lookups (an account's guardians, the pieces a guardian keeps); it was the Platform tab's Recovery page, `GuiPage::Recovery`, removed with that tab. Web: `recovery.html`, linked from the website's Settings > Security. Gameplay holds **Where you live** (ship homes increment 6, 2026-10-12; `where_you_live.rs`): "In this world you live in a Homestead (a family of three).", the household choice for the live save, and where to go to live somewhere else. It replaced Home Design (Family / Solo, one setting per install). | everyone | both |
| Inventory | `inventory.rs` | Inventory grid + equipment slots + weight + item details. From `data/inventory/equipment_slots.json`. | everyone | both |
| Tasks | `tasks.rs` | Three-column kanban (Todo / In Progress / Done) with project selector. | everyone | both |
| Maps | `maps/mod.rs` | THE map page (nav button "Maps"; heading "Maps" since v0.1145). Views: Planet (OpenStreetMap regions, GPS style), Solar System (3D orbits + planet details), Galaxy (real-catalog stars from data/stars-map.bin, light-year scale), Night Sky (celestial sphere + constellations). One file per view in `src/gui/pages/maps/` (`maps/system_view.rs`, `maps/planet_view.rs`, `maps/galaxy_view.rs`, `maps/night_sky.rs`, with `maps/body_list.rs`, `maps/sky_events.rs`, `maps/time_bar.rs`, and since Maps increment 1 `maps/camera.rs`, `maps/controls.rs`, `maps/you_layer.rs`, `maps/stations.rs`). Opened from the game it frames your ship (the UHS Mothership) and follows it, drawn from its real meshes by `src/engine/map_view.rs` from what `src/engine/map_feed.rs` reports; its state is one `GuiState::maps` (`gui/state_types/maps.rs`). It was `gui/pages/cosmos.rs` until Maps increment 0 (2026-10-20, docs/design/maps-zoom-to-anything.md); `crate::cosmos` is the Sol model, not the page. The duplicate `GuiPage::Cosmos` variant was MERGED into Maps in v0.1145. | everyone | both |
| Market | `market.rs` | Marketplace: browse, search, create listings. Sidebar category filter, card grid. | everyone | both |
| Profile | `profile.rs` | Player profile with privacy-tiered sidebar sections. | everyone | both |
| Chat | `chat.rs` | 3-panel chat: server/channel browser, messages, member list. | everyone | both |
| Calculator | `calculator.rs` | Full scientific calculator + history. | everyone | both |
| Notes | `notes.rs` | Notes app with sidebar, editor, toolbar, autosave. | everyone | both |
| Calendar | `calendar.rs` | Month view, event dots, add-event form (time/color/desc). | everyone | both |
| Crafting | `crafting.rs` | Recipes by category, craft queue with progress. From `data/crafting/categories.json`. | everyone | both |
| Wallet | `wallet.rs` | SOL balance, address copy, send form, transaction history. Solana RPC proxy. | everyone | both |
| Guilds | `guilds.rs` | Guild browser, detail view with members + chat, create form. | everyone | both |
| Trade | `trade.rs` | P2P trading with escrow. | everyone | both |
| BugReport | `bugs.rs` | Submit bug reports with severity + category. From `data/bugs/taxonomy.json`. | everyone | both |
| Library | `library.rs` | Everything you READ, in three faces: DOCUMENTS (section > category > document, from `data/library/`, with a tag filter, full-text search, working cross-references, a Contents outline per document and heading anchors, and a rail that folds down to the open document's path plus whatever you unfold by hand, 2026-09-25; shelves and documents listed A to Z by default, or in the catalog's Suggested reading order, a remembered choice, 2026-10-10), the DICTIONARY, and CURRICULUM (v0.1311, `data/curriculum/syllabus.json`), the Library's map of ITSELF: every subject a person needs and honestly how far each has got, with unwritten topics shown dimmed rather than hidden. The external tools/websites third face moved to the Tools page 2026-07-30 (part of the Apps page since 2026-10-19), so Library and Apps split by verb rather than by ownership. Web mirror: `library.html`, same manifest. | everyone | both (data-parity enforced) |
| Donate | `donate.rs` | Hero, the two ways to give (the nonprofit Sponsor-a-Can, tax-deductible; the maintainer on Patreon, not), more direct links + the server's funding goal, charities, FAQ. Both sides read `data/donate/*.json`. | everyone | both |
| Tools | merged into Apps | **Merged into the Apps page on 2026-10-19** (`GuiPage::Tools` and its page id "tools" are gone). The external catalog (`data/external/catalog.json`) is now the Apps page's Other programs (`software`) and its Help and Services (`service`, split by each category's `section`). The website's Tools page became its Apps page (`apps.html`) on 2026-10-20. | everyone | both (data-parity enforced; finding, launching and installing desktop-only) |
| Studio | `studio.rs` | OBS-like broadcasting studio (scenes, sources, properties) with a Program/Preview split (v0.664): scene clicks stage into Preview, "Cut to Program" pushes live. Real broadcast transport shipped v0.853-0.855 (GPU window capture, MJPEG frames over a binary WebSocket to the relay at /ws/live/pub). Chat page carries a full-width live strip (v0.1150) with Go Live + the live-now directory. | everyone | native-only, no web page |
| Watch | `watch.rs` | Stream viewer (v0.857): browse live broadcasts on the connected relay (GET /api/live) and watch one (MJPEG frames over /ws/live/sub via `net/live_viewer.rs`; not segments). Sits beside Studio in the nav; the web mirror is `watch.html`. | everyone | both |
| Files | `files.rs` (+ `files/`) | **One page for every file (2026-10-19)**: the Media page, with the developer's Files page (`files/app_folder.rs`, the HumanityOS folder, and `files/shared_files.rs`, the server's Shared files) merged in as PLACES at the top of its left pane (`files/places.rs`): on the server, Shared on <server> (what follows), Shared files and, for admins, Admin; on this computer, the HumanityOS folder (the app's own folder, the one holding data/, assets/ and mods/, read folder by folder; a text file read and edited in the middle, Save keeps `<name>.bak` beside it, Put back the previous version) and Your folders. The words say Files: the heading "Files on {host}", Add to Files, Put on the Files page, Remove from Files. Shared on <server> was first built 2026-10-11 (increment 2 of docs/design/files-showcase.md): the pictures, videos, 3D models and files people on this server chose to show. A featured strip, filters (category, kind from `data/files/kinds.json`, person, My picks, search over titles), a grid paged by More, content-label badges (a labelled tile shows its label until clicked; the protected setup hides it), pick counts and uploader names; a picture opens in the image viewer, a video in the app's own player (AV1 WebM; without sound for now), a model or file as a card with Download. Pick, Remove (says whether the file goes), Feature (admins), Report (the existing dialog). Add to Files (picker, form, the person's share shown, an upload over it stopped before it starts) and Put on the Files page from a Shared Files row on the Files page (public only). Suggest a category. Members-only files come through the session's upload token. Admins: Server Settings > ADMIN > Media (visibility, space, share, previews, categories, suggestions, the meter and prune lists). Sits beside Watch in the nav. | everyone | both Since round 5 (2026-10-12): "Files on {host}", Media and Admin tabs (the Media settings moved here from Server Settings), Sort, scroll paging, the shelf per category, each item's own page (`files/item_view.rs`) and Edit (`files/edit.rs`); the server strip (`server_strip.rs`) above it. Since 2026-10-18 a Your folders tab for everyone, desktop only (`files/your_folders.rs`: folders on this computer to look through and pick files from for Add to Files, marked where already on Media; no website version, operator 2026-10-13, so every browser behaves the same). Since 2026-10-19 the desktop Media tab is three panes (`files/panes.rs`, `files/list.rs`, `files/viewer.rs`): the list, the item to fit (a sound with its waveform and spectrum) and its details, side by side or stacked by the window's shape, resizable, the split remembered; Edit is in place in the details pane (no window); the old tiles are the Gallery view; an item from a chat card opens the same parts in one window. The website keeps its one column for now. Since 2026-10-20 (desktop): a Library guide in the HumanityOS folder (a file in data/library/ the Library catalogs) has Open in the Library (the details' button, the line over its text, a double-click in the tree), opening it in the Library's reader (`files/app_folder.rs` `open_in_library`); and the page's explanations are folded behind one small How this page works toggle by the heading (`files/how.rs`, closed until opened, config.json `files_how_open`), Shared files' public line and the HumanityOS folder's caution staying where the person acts. |
| Apps | `apps.rs` (+ `apps/games.rs`, `apps/own_tools.rs`, `apps/programs.rs`, `apps/library.rs`, `apps/covers.rs`, `apps/game_detail.rs`, `apps/entry_detail.rs`, `apps/detail.rs`; readers `src/games/`, `src/software/`) | **The Games library and the Tools page as one page, 2026-10-19** (operator: "changing the name of ... games to apps. Then merge tools with games/apps"; then "extending the RGB style to CMY"). In the nav beside Play and Characters. Six parts, each under a band in its own colour AND its name (colour never alone; theme tokens, `widgets/tint_section.rs`): RED Games (Steam, the Epic Games Store and GOG, read from each store's own records, started through the store's own app; Steam's cached pictures; Epic covers fetched from Epic's own host ONLY when the person turns the switch on, kept in memory, never written; achievements, screenshots, Workshop mods and links in the details; the two dated findings with their Whys), GREEN HumanityOS tools (the app's own pages, `data/gui/humanityos_tools.json`, each opening its page), BLUE Other programs (the catalog's free programs: which are installed from Windows' records, the Start menu's shortcuts and Steam's; Launch; Install through winget after a consent step; a Steam copy gets Steam's links), CYAN Help (urgent and protective needs) and MAGENTA Services (everyday needs), the catalog's help websites, YELLOW Added by you (any game or program added by its own program; Rename, Remove). One toolbar: search, sort, Grid or List, a chip per part, only what is on this computer; kept in config.json (`apps_view`, `apps_added`, `apps_epic_covers`). Short tiles: a name drawn once. Since 2026-10-20: a program not found that winget has shows Install on its tile and List row, which opens its details with the consent step (nothing runs before I agree); copies installed from the Microsoft Store are found (`software/store.rs`, the catalog's `store_family`, read only) and launched by their app id; the parts' explanations and their Whys are folded behind one How this page works toggle (config.json `apps_how_open`), the safety line, Help's line, the covers switch and the not-affiliated lines staying visible. | everyone | both: the website's `apps.html` mirrors it (HumanityOS tools, Other programs, Help, Services; data-parity enforced); the Games and Added by you parts, and finding, launching and installing, are desktop-only: a website cannot read or start what is installed on the computer it runs on |
| RelayControl | `relay_control.rs` (+ `host_node.rs`) | Server Control Center (v0.846; nav tab renamed "Relays" to "Servers" v0.1130 per federation-ux.md): manage servers you own - health, actions, config; the in-app VPS Console (v0.858) runs server commands over SSH so admin needs no separate terminal. "Host a node on this PC" (`host_node.rs`) starts/stops a relay on THIS machine with a port / who-can-connect (network, or this computer only = the relay's `BIND_ADDRESS`) / database / server-name form, shows the loopback and LAN addresses to hand to a friend, and is the whole page when you have no relay yet; a running local node joins the rail and swaps the VPS-targeted Control + Console sections for its own controls. | admin | native-only |
| Quests | `quests.rs` | The single quest surface: live in-game quests (auto-track + XP, from `QuestSystem`) render first, then the learn-by-doing self-sufficiency chains (`data/onboarding/quests.json`). Absorbed the old Profile page's game-quests section (v0.415.0). | everyone | both |
| ServerSettings | `server_settings.rs` | Server / group admin (USER / MOD / ADMIN tiered, color-coded). Game-world ban management now lives here too as an ADMIN subsection (`game_admin::draw_section`, folded in v0.479, the standalone `GameAdmin` variant was removed). | admin | native-only |
| Identity | `identity.rs` | DID, Verifiable Credentials, trust score, AI status. | everyone | both |
| Governance | `governance.rs` | Proposals + votes + tally (local + civilization scope). **Native is fully LIVE as of v0.660**: fetches real proposals/tallies from `/api/v2`, casts Dilithium-signed `vote_v1` votes, and submits `proposal_v1` proposals via an in-page form (built with the in-crate ObjectBuilder the relay verifies with). **WEB voting is LIVE too (2026-07-01)**: `governance.html` builds + Dilithium-signs `vote_v1` objects in the browser (`pq-object.js buildVoteV1` over the KAT-locked `canonical-cbor.js`; identity from localStorage via `pq-relay-auth.js getPqIdentity`) and POSTs them to `/api/v2/objects`; byte-equality with the Rust encoder is locked by `just vote-kat` (scripts/vote-object-kat.mjs paired with object.rs::vote_v1_cross_language_kat). Voted proposals persist per identity fingerprint in localStorage. Web still lacks the proposal-CREATION form (native-only for now). | everyone | both |
| Laws | `laws.rs` | Location-aware rules + rights, nested Humanity to locality; HumanityOS base set + real-law summaries. Data: `data/laws/laws.json`. Web mirror `web/pages/laws.html` (shared jurisdiction-chain logic in `web/shared/laws-logic.js`). | everyone | both |
| Homes | `homes.rs` (+ `household.rs`, `where_you_live.rs`) | Your offline homestead (v0.379): the Fibonacci homestead blueprint as a browsable design, pick a build scale (Solo/Family/Community/Colony), see power/water demand for that scale. Since ship homes increment 6 (2026-10-12) the page says where you live (the live save's start and household, read only, under the Household card), and the homestead's section (design, loops, what one home cannot close) is drawn only for a household file with loops, so a Cabin's page has none. Heading the page, with or without a design loaded, the **Household** card (`household.rs`, ship homes increment 5b, 2026-10-12): give a friend a household permit for your plot on this server (lengths from `data/ship/household.ron`, Give asks first), renew it with one press, take it back (asks first; "Taking it back..." until the server confirms), and see the permits friends gave you and whether the server says they work. Not on the web, which has no game: it swallows permit notes and shows nothing. | everyone | both |
| Testing | `testing.rs` | QA testing tasks; operator-facing checklist, Mark Passed / Report Issue posts to chat. From `data/testing/qa_tasks.json`. | dev | native-only |
| Dev | `dev.rs` | Developer tools (v0.777, the Dev tab's Dev section, not a GuiPage variant of its own): spawn any of the 99 creatures.csv species in front of the player, despawn-all, live creature count; documents the walk-up creature editor (look at a creature, press G). Cheats-gated (Settings toggle). | dev | native-only |
| Planet Tuner | `planet_tuner.rs` | Live per-planet physics editor (v0.1120, a section of the Dev tab; artificial-planet ladder item 3): live readout of gravity at the player's altitude, latitude, temperature here vs body reference, catalog pressure + magnetic field, breathability, day length; grouped editors over the frame-locked body's PlanetDef writing changed fields into `data/planets/<id>.ron` via the comment-preserving `assets::ron_edit` (hot reload rebuilds the planet within a frame); gravity_curve raw editor with validated rejects. Dev-mode + cheats gated. | dev | native-only |
| Browser | `browser.rs` | The websites database (`data/web/sites.json`: 10 categories, 38 sites, each with an embed-legality record) as cards. With the readable-web opt-in ON (Settings > Privacy, off by default) a card or a typed address opens INSIDE the app in the readable web view (`widgets/web_view.rs`: html5ever-parsed headings/paragraphs/links/images/lists/tables/code, clickable links, back/forward, no browser engine, no JavaScript, "Open in browser" escape hatch). Off = cards open the OS browser. Design: `docs/design/readable-web.md`. | everyone | both |

### Merged super-tabs (fold multiple pages into one `section_nav` sidebar; page carve v0.358-360, still evolving)

These are also top-level `GuiPage` variants, but each one delegates to several of the
pages above rather than being standalone content. Added when the nav was consolidated
from many top-level buttons to a handful of tabs.

| Name | File | Folds in |
|------|------|----------|
| Real | `real.rs` | Renamed "Profile" in the UI (v0.378; the enum variant name `Real` is the internal legacy name). Profile's sections (Body/Identity/Notes/Network/Interests/Skills/Social/Streaming) plus Inventory, Wallet, Tasks, Maps, Market. |
| Dev | `dev_tab.rs` | The developer tools tab (2026-10-20; it replaced the Platform tab, which the operator called redundant): Bugs, Testing, Performance, Dev and Planet Tuner in one side list, all kept (perpetual development). Platform's Calculator, Notes, Calendar and Browser are HumanityOS tools on the Apps page, each opening its own page; its Recovery is in Settings > Account. Web twin: the Dev Workbench, `dev.html`, carries the same list of five. |
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
| `showroom.rs` | The character-select/appearance-editor panel (Characters, the Play picker: WHO, WHERE and one Enter bar). Gated by `GuiState.showroom_active`, drawn directly from `src/lib.rs`'s render loop (`pages::showroom::draw`), not via `GuiPage` dispatch. Since ship homes increment 6 (2026-10-12): the WHERE column's **A new home** row shows "Choose where you live" (every start from `data/homes/starts.ron`, the Apartment greyed) while your own home has no save yet, and once it has one says a new character moves into it; the card of the **connected server** asks the server where you live there and shows "Where you live here: a Cabin, c2.", or the kinds of home with their free counts and **Choose**, and the note that a server starts you fresh and keeps your home there on this computer (`where_you_live.rs` `draw_choice`, `draw_server`). The WHO hint says each server keeps its own home and things for you. Snapshots: `tests/snapshots/picker_new_home.png`, `picker_where_you_live.png`. |
| `where_you_live.rs` | "Choose where you live" (ship homes increment 6), one widget drawn in Settings > Gameplay, on the Home page, on the picker's new-home row and on a server's card. Not a page of its own. Web mirror of the server card: the chat's "Where you live" window. |
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

## Web pages (`web/pages/*.html`: 41 standalone; platform removed with the app's Platform tab 2026-10-20; files merged media + shared-files + the data browser 2026-10-19; media added 2026-10-11; hi + rules added 2026-09-06; widget-gallery added 2026-08-25; humanity hub added + agents removed 2026-08-17; devlog added 2026-08-01; library + platform added and resources removed 2026-07-30; mission added 2026-07-16; audit/ai-usage/dashboard/data/projects removed 2026-07-05)

Web is a superset of native, adds marketing/landing/dev pages that don't need a native counterpart.

| Name | File | Purpose | Audience | Web-only? |
|------|------|---------|----------|-----------|
| Index | `index.html` | Landing page, rebuilt 2026-07-16 as a 7-screen picture-book scroll ("one breath per screen"): galaxy hero + one giant CTA, game intro, four real-screenshot cards, cost-nothing checklist, blueprint dream, proof bar, warm welcome-home close. Under 150 words of copy; the mission essay moved to mission.html. | everyone | yes |
| Mission | `mission.html` | Permanent forwarder to `/library#humanitys-mission` since v0.1093.2 (the mission essay was merged into the Library so ONE doc serves both clients; the page holds only SEO meta + an instant redirect + a fallback link; a comment forbids regrowing content there). Linked from landing screen 5 "Read the full mission". | everyone | yes |
| Rules | `rules.html` | The published rules for united-humanity.us, who moderates and what powers they actually hold (taken from the enforcing code, not from intentions), how to report, how to appeal including a route that works while banned, and an explicit list of the accountability this server does NOT yet have. Replaces the retired #rules channel whose text is archived in `data/server/legacy_channel_content.json`. Required by `docs/accord/communication_and_association.md`, which says a space must publish its rules and declare its authority before anyone participates. | everyone | yes |
| Hi | `hi.html` | First-contact page at /hi, the QR target on printed business cards. Read once, standing up, by someone who has just met the founder and knows nothing: one plain sentence saying what this is, one thing to do in the next minute (one short growing guide), and the address to come back to. Phone-first and deliberately one screen, so it answers before a thumb reaches to scroll. No jargon by design. | everyone | yes |
| Devlog | `devlog.html` | Public devlog: renders the GitHub Releases feed live client-side (`devlog-app.js`, sessionStorage-cached, load-older pagination), so it can never drift from the real release history; links the releases.atom RSS feed. Landing target for social posts; linked from the landing proof bar "updates shipped" tile. Native mirror: release notes reach native via the updater today; a native Devlog reader is a candidate follow-up. | everyone | yes |
| Home | `home.html` | The ideal closed-loop homestead outline (one person, six loops, honestly sized): renders `data/home_outline.json`, the SAME data the native Home page's "ideal closed loop" panel renders -- web mirrors native. Replaced the old localStorage room-decorator (v0.700, operator direction: Home outlines the perfect closed loop AND doubles as the game's Home requirements list). | everyone | both |
| Onboarding | `onboarding.html` | Web's own onboarding flow (native's standalone Onboarding page was removed v0.415.0 and folded into Quests; web was NOT re-checked for the same fold in this pass). | everyone | web-only in practice |
| Download | `download.html` | Desktop binary download + module list. | everyone | yes |
| WalletGuide | `wallet-guide.html` | "?" help page from Wallet. | everyone | yes |
| Roadmap | `roadmap.html` | Public roadmap view, rendered from `data/roadmap.json`. | everyone | yes |
| Dev | `dev.html` | The Dev Workbench (icons, assets, components, theme tokens), and since 2026-10-20 the website's Dev tab: the app's Dev tab list of five (Bugs linked, Testing, Performance and Planet Tuner marked desktop-only) plus the Widget Playground. | dev | yes |
| Widget Playground | `widgets.html` | Inert widget sandbox + reference (added 2026-08-25): the web home for real widgets AND experimental ones. Demos buttons, a toggle, the shared press-and-hold confirm (modal `holdConfirm` + inline fast-RGB clock ring via `/shared/hold-confirm.js`), the channeling/badge-pulse RGB effects, and an experimental Cistercian-numeral widget (0-9999 as one glyph), with plain-language captions. Nothing triggers a real action; linked from the Platform hub. Native counterpart: the Settings > Widgets "Effects test bench". | everyone (dev/edu) | yes |
| Ops | `ops.html` | Operations / monitoring. | admin | yes |
| Admin | `admin.html` | Admin dashboard. Stats, federation management (POST /api/admin/federation), and since 2026-10-11 the **Files page** section (`files-admin.js`, the web home of native Server Settings > Media): who sees the Media page, the Media budget and each person's share, and "Public visitors get previews only" (saved with `server_settings_update` over the page's own signed-in socket, `/shared/relay-session.js`; a save ends at the relay's answer, a refusal, a dropped connection or 15 seconds), the categories, suggestions waiting, and the meter (with what the server keeps on its own line) with the prune lists (Remove, and Keep on the server). No service control, alert-channel editing or backup trigger: those need the native exe or SSH. | admin | yes |
| Accord | `accord.html` | The Humanity Accord rendered as a navigable web page (built 2026-07-01 during the fleet redo of the destroyed Accord-page work; registry row added 2026-07-02 when page_registry_lint caught the omission). | everyone | web (native Library page shows the same documents) |
| Watch | `watch.html` | Web stream viewer (v0.857 streaming arc): watch live broadcasts from the relay in the browser; mirrors the native Watch page. | everyone | both |
| Files | `files.html` | **Web mirror of the native Files page** at `/files` (2026-10-19: the Media page at `/media`, the Shared Files page `shared-files.html` and the data browser that was `files.html` became this one page). PLACES at the top of its left pane (`files-places.js`, `/shared/files-places.css`): Shared on <server> (what follows), Shared files (`files-shared.js`, the Shared Files page that was: search, a row per file with its badge, the file chosen with Download, Put on the Files page on your own, Upload a file through `shared-files-upload.js`, files dropped there going to Shared Files), and for admins Admin and The server's data folder (`files-folder.js`, `/shared/files-folder.css`: the server's data/ through `/api/data-files`, signed POSTs the server answers for its admins only, BUG-206; a tree, viewers and the editor; not called the HumanityOS folder, which on the desktop is the app's own folder on the computer), in the desktop's order; each at its own address (`/files#shared-files`, `#folder`, `#admin`). No Your folders on the web (operator, 2026-10-13). Shared on <server> was first built 2026-10-11 (docs/design/files-showcase.md "Increment 2, exact", then round 2 against the relay's follow-ups; `files-app.js`, its resumable upload `files-upload.js`, its words for sizes and the file space `files-space.js`, styles `/shared/files.css`). Pictures, videos, 3D models and files people on this server chose to share: a featured strip (every featured item, `featured=1`), filters (category, kind from `data/files/kinds.json`, person, My picks from the signed `POST /api/files/picks`, a search over titles), a grid paged by More, a viewer (pictures large, videos in a `<video>`, models and files as name, size and Download under the uploader's own file name), Pick and Unpick, Remove (its words from each item's `keeps_file`), Feature for admins, Keep on the server and Stop keeping for admins and moderators (a "Kept by the server" badge), Report (context `media`, the item as evidence, the help outside this server for the danger reasons), Add to Files (only for a role that may upload; the person's share from `/api/files/mine`, an upload over it, over the role's largest file for the Files page or over the space left stopped before it starts; a resumable upload with a progress bar, Pause and Resume, carrying on after a dropped connection or a reload, the role's daily or hourly allowance and a full space at the finish said in the server's words; then `POST /api/files`), the person's uploads waiting to be published (Publish, Remove), Put on the Files page from a Shared Files row (public only), and Suggest a category. `/files?id=<n>` opens one item (the chat's Reports view links there). Every write on an item names its file as well as its number, and a page out of date loads again. Opens its own signed-in socket (`/shared/relay-session.js`) for the session upload token, the server's settings and the role list. Members-only files are fetched with the token in an `X-Upload-Token` header and shown through object URLs, so no token is in any address. Pictures load only from this server (`/uploads/`, `/api/files/file/`). Admins also get the admins' section here (`files-admin.js`, the same one as on `admin.html`). In the nav beside Watch. Tests: `scripts/tests/files-web.test.js`, `scripts/tests/files-web-round2.test.js`. | everyone | both Since round 5 (2026-10-12): "Files on {host}", the Admin tab (/admin's Files page panel is one line pointing here), Sort, scroll paging, the shelf per category, `/files/<id>` addresses served by the relay with link-preview tags; the server strip. |
| Web | `web.html` | Web mirror of the native Browser page (2026-09-16): the same `data/web/sites.json` cards (categories, descriptions, embed review state on hover) opening in a new window, plus the person's own bookmarks in localStorage. The in-app reader itself is native-only (a website cannot host it). | everyone | yes |

| Library | `library.html` | **Web mirror of the native Library** (built 2026-07-30). Document tree + reader + Dictionary + Curriculum, reading the SAME `data/library/index.json` manifest, the SAME markdown files and the SAME `data/curriculum/syllabus.json` native reads from disk. Before this, web exposed only the 17 Accord docs via the relay while the app shipped 53: the single worst parity gap in the registry. Enforced now by `tests/page_parity_lint.rs`. Since 2026-09-25 the page has ONE scrollbar: the rail has no scroller of its own, travels with the page and pins at whichever end you are scrolling toward (`pinRail` in library-app.js). Since 2026-10-10 the tree is A to Z by default with a Suggested (catalog order) choice kept in localStorage; the rule is `library-order.js`, the twin of native `rail_order`. | everyone | both |
| Apps | `apps.html` | **Web mirror of the native Apps page** (the website's Tools page made its Apps page on 2026-10-20; `apps-app.js`). The same six parts in the same order, each under a band in its colour AND its name (theme tokens: `--dm-bg`, `--group-bg`, `--server-bg` with the nav colours for edges, and the `--section-*` cyan, magenta and yellow): RED Games and YELLOW Added by you are one line each saying the desktop app does them, with a link to /download (a website cannot see or start what is installed); GREEN HumanityOS tools from the same `data/gui/humanityos_tools.json`, each tile opening the website's page for it (`TOOL_PAGES` in apps-app.js; Quests and Studio, which the website has no page for, say "In the desktop app"); BLUE Other programs, CYAN Help and MAGENTA Services from the same `data/external/catalog.json`, split by each service category's `section`. One toolbar: search across the parts, Sort (Name; Category with a catalog part shown), Grid or List, a chip per part with its count. Short tiles (the name once, on the cover). The details at the right (under the parts on a phone). The view kept in localStorage `hos_apps_view` (the app's `apps_view` fields). In the nav beside Play, as in the app. Tests: `scripts/tests/apps-web.test.js`. | everyone | both |

Plus mirrors of native pages: `humanity.html` (the Humanity tab hub: mission + links to civilization/governance/laws/identity/donate/accord; added 2026-08-17 so the web finally groups what the app sidebar groups), `chat.html`, `inventory.html`, `tasks.html`, `maps.html`, `market.html`, `profile.html`, `civilization.html`, `calculator.html`, `notes.html`, `calendar.html`, `crafting.html`, `wallet.html`, `guilds.html`, `trade.html`, `files.html`, `bugs.html`, `donate.html`, `identity.html`, `governance.html`, `laws.html` (jurisdiction-chain + filter logic shared via `web/shared/laws-logic.js`), `recovery.html`, `settings.html`.

**Retired 2026-07-30:** `resources.html` + `resources-app.js`. It rendered `data/resources.json`, a curation that shared only 17 URLs with the `data/resources/catalog.json` the native Library rendered. Both files and `data/tools/catalog.json` merged into a single `data/external/catalog.json` (24 categories, 103 entries, 18 duplicate URLs collapsed) rendered by the Tools page on both sides, and since 2026-10-19 (desktop) and 2026-10-20 (website) by the Apps page. The in-game "sim" half of `resources.json` was not external at all and was preserved to `data/onboarding/sim_guides.json`, which still needs a render surface.

**Not mirrored on web at all:** the Construction/Build Editor (3D-viewport/gizmo-heavy; web has no wgpu renderer, so a literal mirror isn't the right shape) and the merged super-tabs (Real/Humanity; the Dev tab's web twin is the Dev Workbench, `dev.html`). (The 5 category-Overview landing pages and the 12 Settings sub-page variants that used to be listed here were removed from native in v0.699.0; web's `settings.html` remains a single page covering the same Settings ground.)

## Web hubs (entry points outside `web/pages/`)

| Name | File | Purpose |
|------|------|---------|
| Chat hub | `web/chat/index.html` | Cooperative chat with cryptographic identity. Its command palette also opens windows that mirror game panels on the signed-in socket, among them **Where you live** (`web/chat/chat-home.js`, ship homes increment 6, 2026-10-12): your home on this server's ship, or the kinds of home with their free counts and Choose, every word from the server's answer. |

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
| **Reality** | `nav_reality` (red) | Profile, Chat, Wallet, Donate, Tasks, Market, Civilization, Governance, Maps, Identity |
| **Sim** | `nav_sim` (purple) | Inventory, Crafting, Studio, Guilds, Trade |
| **Tools** | `nav_tools` (blue) | Calculator, Calendar, Notes, Library, Apps, Browser |
| **Settings** | `nav_settings` (gray) | Account, Appearance, Animations, Widgets, Notifications, Wallet, Audio, Graphics, Controls, Privacy, Data, Updates, Server Admin |
| **Dev** | `nav_dev` (amber) | Bugs, Testing, Performance, Dev, Planet Tuner (the Dev tab, 2026-10-20) |

Dev visibility is gated by `theme.nav_dev_visible` (default `true` during the development period; toggle in Settings -> Animations -> Developer mode). At v1.0 the default flips to `false` and only shows when the operator opts in.
