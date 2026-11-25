# Page audit, 2026-11-24: every page a person can visit, and whether they can find it

Asked for by the operator on 2026-11-24: "Let's do an assessment of all the pages a user can visit at
the moment. See if any can't be found." Beside it, from the day before: "The less pages our app has
the better as it'll be easier for users to remember." This audit feeds the navigation redesign
(`docs/design/navigation-redesign.md`: the full-screen Menu and the breadcrumb) and the planned
Community hub (Watch, Files and maybe Music folded into a server's list of spaces).

Method: read from the code on main (2026-11-24), not from memory. Native: `enum GuiPage` in
`src/gui/mod.rs` and `page_id` in `src/gui/dispatch.rs` (38 variants, so 37 pages and the `None`
in-world state), `data/gui/navigation.json` (the one file that places every desktop page and every
website page), the callers of `state.go(GuiPage::...)`, `data/keymaps.ron`. Web: `web/pages/*.html`
(45), `web/chat/index.html` and `read.html`, `web/home/technical.html`, `web/shared/shell.js` (its
top row and its "More pages" drawer), `scripts/nginx/humanity.conf` (routes and redirects), and a
script that counted, for each web page, which other files link to it. Nothing was booted.

## 1. The answer in numbers

| Count | Number | Which |
|---|---|---|
| Native pages | 37 (38 variants with the in-world `None`) | 34 are placed in the menu by `navigation.json`; 3 are placed "outside" it (title screen, Profile alias, Servers you run) |
| Website pages | 48 files | 45 in `web/pages/`, the chat (`/chat`, `/chat/read`), one alternate homepage (`web/home/technical.html`) |
| Native pages nobody can reach | 0 | a test (`navigation_tests.rs`) fails when a page is in no row; every page has a way in |
| Native pages reached only one hidden way | 1 | Servers you run (`RelayControl`): a small server icon in the chat's left list |
| Website pages in no menu, row or drawer | 7 | `/music`, `/mission`, `/hi`, `/widgets`, `/wallet-guide`, `/recovery`, `/404` (see 3.2); of these only `/music` is a real gap |
| Pages that do not work on a phone, native | 7 menu rows | Characters, Studio, Testing, Dev Performance, Planet Tuner, Journal, AI (they draw, but need a desktop) |
| Pages the phone's browser cannot reach | 1 real, 4 by nature | real: `/music` (not in the drawer); by nature: Quests, Studio, Testing, Servers you run are desktop only |
| Web-only (no native page) | 14 pages, plus the front door, the 404 and a partial Admin | Accord, Civilization, Devlog, Download, Hi, Mission, Onboarding, Ops, Privacy, Recovery, Roadmap, Rules, Wallet guide, Widgets |
| Native-only (no website page) | 6 pages, plus Server Settings (the website has its ADMIN part only) | Title screen, Play, Quests, Studio, Testing, Servers you run |
| Two pages doing one job | 8 groups | section 5 |
| Dead or broken | 3 | an unused placeholder page module, a "coming soon" line on Calculator, a hidden duplicate enum name (section 4) |

## 2. Every native page

"Row" means the desktop and phone Top menu (`data/gui/navigation.json`), which is the same menu on
both: the phone runs the same app (`src/android.rs`), with no 3D world. "Hub" means a button or card
on another page. Dev pages are permanent tooling (CLAUDE.md, "Forever development"): marked DEV, never
proposed as cuts.

| Page | Web twin | How it is reached | Findings |
|---|---|---|---|
| MainMenu (title screen) | none (front door `/` is its cousin) | launch; Back past the first page; first-steps | Never in a menu, by design. Native-only, fine |
| None (in the world) | none | Play hub's Enter the world, Characters picker's Enter | Not a page. Phone: Enter is a notice (no world) |
| Humanity | `/humanity` | row category 1; H button; first boot landing | The Mission Dashboard. Its row holds Directory, Governance, Laws, Donate; Mission, People, Rules, Roadmap, Devlog wait (rung 7) |
| Identity ("Directory") | `/identity` | Humanity row; Humanity page | Stands in for the planned People page |
| Governance | `/governance` | Humanity row; Humanity page card | |
| Laws | `/laws` | Humanity row; Humanity page card | |
| Donate | `/donate` | Humanity row; Humanity page card | |
| Play (the game's hub) | `/download` (Download) | row category 2 | Cards for Characters, Quests, Inventory, Crafting. No world on a phone |
| Quests | `/onboarding` (a different, web-made page) | Play row; Humanity page card | Native-only name. Web calls its twin Onboarding and lights its "quests" tab: one thing, two names |
| Inventory | `/inventory` | Play row; the I key in the world; Maps page | Also two-realities: the game's pack, and your real things |
| Crafting | `/crafting` | Play row; Build button | |
| Characters picker | none | Play row (a part of the world's screen, not a `GuiPage`) | Desktop only |
| Chat | `/chat`, `/chat/read` | row category 3; bell and notices; Alt+C footer | The hub of the planned Community hub |
| Watch | `/watch` | Chat row; server strip | Folds into Community hub |
| Studio | `/studio` redirects to `/chat` | Chat row | Desktop only (screen capture). The sender half of Watch |
| Files | `/files` | Chat row; server strip; chat attachments | Folds into Community hub |
| Orgs | `/orgs` | Chat row | Website: drawer only, not in the top row |
| Homes ("Life", the Home page) | `/home` | row category 4 | Name clash: the row says Life, the page Home, the website tab Home |
| Real ("Profile") | `/profile` | Life row | The page is called Real in code and Profile on screen: a name that differs (section 4) |
| Profile (alias) | n/a | nothing links to it by choice; onboarding steps used to | Not a page: forwards to Real. Retire the variant |
| Tasks | `/tasks` | Life row; notice entries | |
| Maps | `/maps` | Life row | |
| Wallet | `/wallet`, `/wallet-guide` | Life row | Web has a separate help page |
| Market | `/market` | Life row | Duplicate of Trade (section 5) |
| Trade | `/trade` | Life row | Duplicate of Market. Website: drawer only |
| Apps | `/apps` | row category 5 | |
| Notes | `/notes` | Apps row | |
| Calendar | `/calendar` | Apps row | Overlaps Tasks |
| Calculator | `/calculator` | Apps row | Website says "Live exchange rates coming soon" (key `calculator.currency_notice`) |
| Music | `/music` | Apps row; Settings > Music button | Website: only the Settings link reaches it |
| Browser | `/web` (labelled Bookmarks in the drawer) | Apps row | Same page, three names (Browser, `/web`, Bookmarks) |
| Library | `/library`, `/accord` | row category 6 | `/accord` is a permalink to one shelf |
| HowTo | `/how-to` | Library row; Tasks page | |
| Dev (DEV) | `/dev` | row category 7 | Parts: Dev, Performance, Planet Tuner, Claude, Journal, AI (sections, not pages) |
| BugReport ("Bugs") (DEV) | `/bugs` | Dev row; keymap | QA workflow with Testing |
| Testing (DEV) | none | Dev row | Desktop only |
| Settings | `/settings`, `/recovery` | row category 8 | 18 sections inside (Account ... Credits) |
| ServerSettings | `/admin` (ADMIN part only) | Settings row; the chat's cog; Servers you run | |
| RelayControl ("Servers") | `/ops` is its nearest web twin (Ops) | only the small server icon in the chat's left list (`left_panel.rs`) | Orphan-ish: in no menu, no row; the tooltip is the only sign. Fix in 3.1 |

Overlays that look like pages and are not (`docs/PAGES.md`, "Not GuiPage variants"): the Build
editor (B key), the character showroom, the vendor card, the medbay card, the F2 to F4 overlays,
the HUD. They are reached in the world by key or by looking at a thing, and are not pages. In-world
screens (monitors) draw pages too; that is a second way in for those pages, not a new page.

## 3. Orphans and weak entries

### 3.1 Native

No native page is unreachable. Two are weakly placed:

| Page | Today | Fix |
|---|---|---|
| Servers you run (`RelayControl`) | one tooltip-only icon in the chat's left list; `navigation.json` lists it "outside" | give it its own row: the planned Servers page (rung 7) as an item of the Chat category; until then add it to the Chat row as "Servers", and the Menu page's Chat chapter lists it |
| Profile alias | a variant that only forwards | delete the variant and point its few callers (`profile.rs`, `server_settings.rs`, onboarding) at the real page |

Shortcut-only entries: none. The one key that opens a page is I (Inventory); Esc leaves.

### 3.2 Website

| Page | How it is reached today | Verdict and fix |
|---|---|---|
| `/music` | a button in Settings > Music (`settings.html`); not in the top row, not in the drawer, not in the sitemap | A real gap: the desktop has Music in Apps. Add it to the drawer and to the Apps page's list |
| `/mission` | typed address only; the page is a redirect to `/library#humanitys-mission` | Retire the HTML file; make it an nginx 301 |
| `/hi` | the printed card's QR address | Intended orphan; keep, never add to a menu |
| `/widgets` | a link on `/dev` | DEV page; fine. Add to the drawer's "Developer and operator" group so it is not hidden |
| `/wallet-guide` | a "?" on Wallet | Fine now; moves into the help system with the Menu (navigation.json already says so) |
| `/recovery` | Settings; the chat's key help | Fine; its desktop twin is Settings > Account |
| `/404` | nginx only | Not a page anyone visits on purpose |
| `/chat/read` | `/chat` sends a visitor with no key | Fine; no menu entry needed |
| `web/home/technical.html` | linked from nothing, not served by the live nginx | Not a page of the site: a homepage flavour a self-hoster may copy over `index.html`. Keep; document it in `docs/PAGES.md` only |

On the website only the drawer ("More pages", shown on desktop too since v0.699.2) reaches these:
Orgs, Trade, Market, Wallet, Donate, Civilization, Governance, Rules, Laws, Identity, Roadmap,
Calculator, Calendar, Notes, Bookmarks, Devlog. The top row holds 17 tabs and omits them. That is
reachable but easy to miss; the Menu page replaces both.

## 4. Dead, broken and confusing

| Item | Where | What to do |
|---|---|---|
| Unused placeholder page | `src/gui/pages/placeholder.rs` ("Native version coming soon.") is declared in `pages/mod.rs` and used by no page | Delete it (KISS, CLAUDE.md 2026-11-24). Nothing draws it |
| "Live exchange rates coming soon" | `data/i18n/en.json` `calculator.currency_notice`, shown on web `calculator.html` | Say what works (manual rate entry) without a promise, or build the rates |
| Enum name opposite to the screen | `GuiPage::Real` is the page shown as Profile; `GuiPage::Homes` is the page shown as Home or Life | Operator rule 2026-10-19 (rename internals to match what people see): rename `Real` to `Profile` once the alias variant goes, and settle Life versus Home (question 1 of the navigation design) before renaming `Homes` |
| Names that differ between the two apps | Quests / Onboarding; Browser / Bookmarks (`/web`); Directory / Identity; Life / Home / Real | One word each, in `navigation.json`, then both apps |
| Rows that exist in the data and draw nothing | Mission, People, Rules, Roadmap, Devlog, Worlds, Servers on the desktop (`new: true`) | Intended: waiting for rung 7. The Menu page must not list them until built |
| Stale registry text | `docs/PAGES.md` still carries the "Natural groupings" nav-category table and a "Re-audited 2026-06-30" heading | Refresh after the Menu lands; this file is the current audit |

No route 404s: every address in `navigation.json` has a file (`web/pages/<name>.html`, nginx
`try_files $uri $uri.html`) or a redirect (`/studio`, `/quests`, `/guilds`, `/map`, `/browse` and
the others in `humanity.conf`). `/quests` redirects to `/onboarding`, which is why the website has no
Quests page.

## 5. Two pages, one job (the fewer-pages list)

| # | Group | Why they are one job | Recommendation |
|---|---|---|---|
| 1 | Market and Trade | Listings and escrowed peer trades are both "swap things with another person" | Merge into one page, Market, with two parts: Listings and Trades. Both apps. Saves a page and a row |
| 2 | Watch, Studio and Files (and Music) | Watching, going live and sharing media are the same server's spaces | Fold into the Community hub as spaces of one server. Studio becomes the "Go live" part of Watch (desktop only) |
| 3 | Servers you run and Server Settings | Both are "manage a server" | One Servers page: Yours (health, control, config) and a server's rules and admin. Already the rung 7 plan; do it, with the Chat row entry |
| 4 | Profile alias and Real | One page, two variants | Delete the alias variant |
| 5 | Mission, Civilization and the Humanity page | The Mission Dashboard has three addresses | Delete `/mission` (301 to the Library essay) and keep `/civilization` as an alias of `/humanity` until the dashboard mirror lands, then 301 it |
| 6 | Quests (desktop) and Onboarding (web) | The learn-by-doing chains and the game quests are "one page, two kinds" on the desktop and two names on the web | Name the website page Quests, keep one name in `navigation.json` |
| 7 | Tasks, Calendar and Notes | Three personal organisers, kept on the device | Candidate, not a recommendation to cut yet: a single Organiser page with the three as parts would remove two rows. Ask the operator (his call: "people remember fewer pages") |
| 8 | Accord, Rules, Privacy, Laws, Roadmap, Devlog | Reading pages: the Accord and Privacy are already Library shelves on the desktop | Keep one reading home: the Library gains Rules, Roadmap and Devlog shelves; the website keeps their short addresses as redirects |

Do not touch, as dev tooling: Dev, Bugs, Testing, Performance, Planet Tuner, Claude, Journal, AI,
Ops, Admin, Widgets, Devlog's release feed. Dev and debug pages are never cuts.

If 1 to 6 and 8 land, the desktop goes from 37 pages to about 32 (Trade, the Profile alias, Server Settings and Studio stop being separate
pages) and the website from 45 to about 41 (Mission, Civilization and the reading pages become
redirects), with no feature lost.

## 6. Pages that fold into the planned Menu and Community hub

| Page | Goes into | Note |
|---|---|---|
| Watch, Files, Studio, maybe Music | Community hub (spaces of a server) | Music has no server meaning today; keep it under Apps unless the operator wants listening rooms |
| Orgs | Community hub, or stays a Chat row item | An organisation is a group of people on servers; a space kind is a natural fit |
| Directory (Identity) | People (rung 7) | already planned |
| Wallet guide, Recovery, Privacy, Laws as help | Menu's Help chapter and the Library | already in `navigation.json`'s `outside` list |
| The "More" and the second row | the Menu page | the redesign removes both |

Phone: the Menu should read a `desktop_only` flag (the file already has it for the website) and hide,
or say plainly why it is unavailable (the hardware-notice rule), these rows on a phone: Characters,
Studio, Testing, Dev Performance, Planet Tuner, Journal, AI, and Play's Enter the world.

## 7. The orphans and their fixes, in one list

1. `/music` on the website: add to the drawer, the Apps page and the sitemap.
2. Servers you run: a Chat-row item "Servers".
3. `/mission` page: replace by a redirect.
4. Profile alias: delete the variant.
5. `/widgets`: show in the drawer's Developer and operator group.
6. `GuiPage::Real`, `GuiPage::Homes`: rename to what people see once the Life or Home question is answered.
7. `placeholder.rs`: delete.
