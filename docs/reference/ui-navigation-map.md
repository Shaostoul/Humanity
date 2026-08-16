# UI Navigation Map

> Built 2026-08-16 from a full survey of both surfaces (operator request:
> "create like a tree graph so we can easily see where everything connects...
> we're partially looking for mismatched names"). Companion to
> `docs/PAGES.md` (the page REGISTRY: what exists). This file maps how pages
> CONNECT and what each entry point is NAMED, because a button saying one
> thing and its page saying another is exactly how users get lost.
>
> Update this file when nav structure or user-facing page names change.
> `tests/page_registry_lint.rs` guards existence, not naming; naming drift is
> caught by re-running this survey (or by a future heading-vs-label lint).

## The native tree (the product; web mirrors this)

Top nav, one row, drawn in `escape_menu.rs` (order and exact labels):

```
HumanityOS (native top nav)
|
|- Play            -> in-world FPS (GuiPage::None); opens Showroom if no character yet
|- Characters      -> Showroom picker (same GuiPage::None; picker forced)
|- Humanity        -> Humanity page (mission hub)
|   |- Mission Dashboard  (inline dashboard; NOT the Civilization page - see OPEN-7)
|   |- Governance         -> governance content ("Civic Participation")
|   |- Laws               -> laws content
|   |- Directory          -> identity content ("Identity & Credentials") - see OPEN-8
|   |- Donate             -> donate content ("Support HumanityOS")
|- Chat            -> Chat page
|   |- # scratchpad
|   |- DMs (n)
|   |- Groups (n)
|   |- Commons (n)        merged federated rooms, "?" explainer
|   |- Servers (n)        "+" add server; server icon -> Relay Control ("MY SERVERS")
|   |   |- per-server cog -> Server Settings ("Server: <name>")
|   |- right rail: Studio / Friends (n) / <server> members
|- Studio          -> Studio page (streaming; no page title - see OPEN-5)
|- Watch           -> Watch page
|- Profile         -> Real page (sidebar "Profile")
|   |- PRIVATE:  Body & Measurements | Identity | Private Notes
|   |- PERSONAL: Network Profile | Interests | Skills
|   |- PUBLIC:   Social Links | Streaming
|   |- BELONGINGS: Wallet
|   |- LIFE:     Market | Trade | Guilds
|- Home            -> Homes page ("Your Home")
|- Quests          -> Quests page (sim quests + learn-by-doing chains)
|- Tasks           -> Tasks page (kanban)
|- Inventory       -> Inventory page
|- Crafting        -> Crafting page
|- Map             -> Cosmos render (heading "Cosmos" - see OPEN-1)
|   |- views: System | Galactic | Night Sky
|- Platform        -> Platform page (sidebar)
|   |- Recovery ("Social Key Recovery") | Calculator | Notes | Calendar
|   |- Files | Bugs ("Report a Bug") | Testing | Performance | Dev
|   |- Planet Tuner | Browser
|- Library         -> Library page (document tree + Dictionary)
|- Tools           -> Tools page (external catalog)
|- Settings        -> Settings page
|   |- Account | Appearance | Animations | Widgets | Notifications | Wallet
|   |- Audio | Graphics | Gameplay | Controls | Privacy | Data | Updates
|- [Aa]            display-mode cycle (icons/text/both)

Market page (reached from Profile > LIFE > Market, or onboarding links):
|- Directory (default)   "Market Directory"
|   |- Offerings | Providers | "+ Publish" (shop + offering forms)
|- Classifieds           free-form listings
```

## The web tree

Header bar + mobile drawer, injected by `web/shared/shell.js`:

```
united-humanity.us / public.guide (web)
|
|- header (mirrors the app): Play* | Humanity | Chat | Studio* | Profile
|   | Home | Quests* | Tasks | Inventory | Crafting | Map | Platform
|   | Tools | Library | Settings          (*see OPEN items)
|- mobile drawer
|   |- Main (mirrors the app): the header list + Humanity Accord
|   |- Community and trade: Watch | Wallet | Market | Trade | Guilds
|   |   | Donate | Civilization | Governance | Laws | Shared Files
|   |   | Identity | Recovery | Roadmap
|   |- Tools, system and dev: Calculator | Calendar | Notes | Bookmarks
|   |   | Files | Ops | Bug Reports | Dev
|- footer: CC0 | GitHub | Take Tour       (no internal page links)
|- not in any nav: /devlog /wallet-guide /mission /admin /agents
```

Routes: every `web/pages/X.html` serves at `/X` (nginx try_files). 18 legacy
`301` aliases exist (`/quests`->`/tasks`, `/studio`->`/chat`, ...); see the
"Redirect aliases" table in the survey section of the 2026-08-16 history
entry for the full list.

## Naming rule (adopted 2026-08-16)

**The nav button label is the page's name.** The page heading states that
same name; subtitles and section eyebrows carry any longer description. Web
tab titles are `<Name>, HumanityOS` with the SAME Name. When a page is
reachable from two surfaces, both use the native label (web mirrors native).

## FIXED in v0.1144.0 (the unambiguous wave)

| Where | Was | Now |
|---|---|---|
| native Tasks heading | "Task Board" | "Tasks" |
| native Files heading | "File Browser" | "Files" |
| native Trade heading | "P2P Trading" | "Trade" |
| native Quests page | no page title at all | "Quests" |
| native Relay Control rail | "MY RELAYS" | "MY SERVERS" (users know "servers") |
| native Market classifieds heading | "Marketplace" | "Classifieds" (matches the tab) |
| native Profile first open | no sidebar highlight (default section id "inventory" was not in the list) | defaults to "body" |
| native boot-page dropdown | offered both "Maps" and "Cosmos" (identical page) | "Maps" only |
| web /inventory tab title | "Gear, HumanityOS" | "Inventory, HumanityOS" |
| web /notes tab title | "Journal, HumanityOS" | "Notes, HumanityOS" |
| web /tasks headings | "All Quests" (+ Daily/Story/Side/Personal Quests) while a SIBLING nav entry is Quests | "All Tasks" family |
| web drawer label | "Watch live" | "Watch" (matches native) |
| web market section | "Marketplace" | "Classifieds" (matches native tab) |
| web tab-title format | 9 inverted ("HumanityOS, X") + 1 dash | all "X, HumanityOS" |
| deploy script | web/home/ flavors never reached the web root (SELF-HOSTING's documented cp had nothing to copy) | synced to /home/ |

## OPEN items (each needs a taste call or a real increment)

Ranked by user confusion:

1. **Map / Maps / Cosmos (4 names, 2 duplicate variants).** Button "Map"
   (both surfaces) opens a page headed "Cosmos"; the enum has BOTH
   `GuiPage::Maps` and `GuiPage::Cosmos` drawing the identical page, so nav
   highlight and back-stack behave differently depending on entry path.
   Recommendation: merge the variants into `Maps`, heading "Map", and let
   "Cosmos" name the System view tab it actually describes.
2. **The Quests fork.** Native "Quests" = quest chains page. Web "Quests" ->
   /onboarding (self-sufficiency intro), while the URL /quests 301s to
   /tasks. Recommendation: web Quests should point at a real quests page
   (or /onboarding renamed), and the /quests alias should follow the nav,
   not /tasks.
3. **Chat vs Network (web).** Nav says "Chat"; the page is titled
   "HumanityOS, Network" with H1 "HumanityOS Network", and wallet.html links
   it as "the Network page". Recommendation: "Chat" everywhere; "network"
   stays lowercase concept copy.
4. **Play / Studio / Download (web).** Two header tabs ("Play", "Studio")
   both open /download, the drawer merges them into one entry, and the URL
   /studio 301s to /chat: three labels, one destination, plus a stray
   redirect. Recommendation: Play -> /download stays; Studio -> /chat
   (streaming lives in the chat Studio rail) or gets its own page; kill the
   contradictory alias.
5. **Untitled native pages.** Studio has no page title; Testing and Browser
   lead with taglines ("Verify features Claude shipped"). Recommendation:
   small title line each, matching their labels.
6. **Heading diverges from label (both surfaces, same words):** Governance
   -> "Civic Participation"; Donate -> "Support HumanityOS"; Humanity ->
   "HumanityOS"; Home -> "Your Home" / slogan H1 (web). Recommendation:
   lead with the label word, keep the phrase as subtitle.
7. **Civilization is near-unreachable.** The Humanity sidebar item "Mission
   Dashboard" renders an INLINE dashboard, not `civilization::draw`
   ("Community Dashboard"), which is reachable only via an onboarding link.
   Recommendation: either the sidebar item renders the real page, or the
   variant merges into the inline dashboard and dies.
8. **Humanity sidebar "Directory"** opens content headed "Identity &
   Credentials" (native) while the web drawer calls the same thing
   "Identity". Recommendation: "Identity" both places.
9. **Web nav order + mirror claim.** Native order is Platform-Library-Tools;
   web renders Platform-Tools-Library; the drawer's "mirrors the app"
   section adds Humanity Accord and omits Characters + Watch; the shell.js
   mirror comment lists 14 tabs but renders 15. Recommendation: match
   native order, fix the comment, retitle the section honestly.
10. **Web orphans.** /admin and /agents are linked from nowhere; /devlog and
    /wallet-guide are one-link wonders; /mission is a redirect stub still
    linked from the landing page. Recommendation: Ops-adjacent drawer links
    for admin/agents/devlog, retire the mission stub links.
11. **Icon collisions (native).** Watch has NO icon (blank slot); Platform
    and Tools share the same wrench; Library/Governance and Map/Browser
    share glyphs. Recommendation: one distinct glyph each via the
    `icons::paint_*` helpers.
12. **Eleven `GuiPage` variants are never `active_page`** (Calculator,
    Trade, Files, BugReport, Donate, Identity, Governance, Laws, Recovery,
    Testing, Browser): their content renders inside Platform/Humanity/
    Profile sections only. Harmless today, but boot-page config and the
    back-stack can never reach them. Recommendation: fold them into
    section-only modules (delete the variants) in a cleanup increment.
13. **Native "Characters" and "Watch" have no web counterpart**; native has
    no "Roadmap"/"Ops"-style entries the web drawer has. Acceptable
    divergence (document it), but the drawer section title should not claim
    a mirror while diverging (see 9).
