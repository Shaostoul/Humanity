# Public claims audit, 2026-09-06

Read-only audit of public-facing copy against the actual code and the live server.
Five surfaces audited by parallel agents; every finding then adversarially verified by a
second agent instructed to refute it. Only findings that survived refutation are listed.

**35 confirmed: 5 high, 20 medium, 10 low.** By surface: README and root docs 13,
download page 7, funders brief and applications 6, outreach briefs 5, homepage 4.

Each entry gives the claim verbatim, where it lives, what contradicts it, and wording that
is accurate without being timid. Fix the highs first: each is a false capability claim on a
page a stranger reads before anything else.


---

## HIGH

### 1. C:/Humanity/README.md:30

**Verdict:** false | **Surface:** readme

> Your 24-word backup phrase recovers everything if you lose your device.

**Why it is wrong.** Since the sealed-sender cutover the server does not hold your message history at all. web/chat/chat-dm-store.js:1-13: "The relay's dm_mailbox is a delivery window: envelopes carry no sender and expire after the server's TTL. Long-term DM history therefore lives HERE, in IndexedDB." src/relay/mod.rs:404 sets that TTL default to 30 days and mod.rs:808-810 runs a sweep that deletes expired envelopes. chat-dm-store.js has no export, sync, or backup method (only init/insert/deleteConversation/_persistMeta). The same file, lines 28-34, says your follow graph and friendship certificates are also stored only there: "the server stores no edges; these sets ARE the user's social state." So the seed restores your identity and unlocks the last 30 days sitting in the mailbox, and nothing older. A user who loses a laptop loses every conversation and their contact list.

**Accurate wording.** Your 24-word backup phrase restores your identity itself on any device, plus whatever the server is still holding for you. Your message history is deliberately not on the server: it is stored encrypted on the device that received it, and undelivered mail expires after 30 days. That is what makes the relay unable to read or hand over your conversations, and it also means the device is the copy that matters. Device-to-device history backup is not built yet.

### 2. web/pages/index.html:506 (live: https://united-humanity.us/ , "Plan your day" card, under "Six things it does today.")

**Verdict:** misleading | **Surface:** homepage

> A simple board for what to do, what is in progress, and what is done.

**Why it is wrong.** In the desktop app this page tells you to download, the board can be read but not used. Pressing New Task only pushes a struct into memory (src/gui/pages/tasks.rs:419, `state.tasks.push(task);`); tasks.rs contains no `ws_client` send and no save call anywhere. The next server task_list wipes it (src/lib.rs:17175, `state.gui_state.tasks.clear()`), and nothing persists it to disk, so the task is gone on the next connect or restart. On the web the board is real but writing needs a chat sign-in or an admin key: web/pages/tasks-app.js:359 falls back to "Not signed in. Enter Admin API Key or sign in at /chat first.", and the REST create path `POST /api/tasks` requires API_SECRET auth (src/relay/api.rs:1037, "create a task via bot API (requires API_SECRET auth)"). The screenshot directly above, captioned "Here is what you actually get", shows a populated board whose three tasks are hardcoded snapshot fixtures (src/gui/ui_snapshots.rs:229, "Plant the spring greens"); the live board is empty.

**Accurate wording.** A shared board for what to do, what is in progress, and what is done. Reading it works anywhere. Adding a task needs the web page and a chat sign-in today, because the desktop app's New Task button is not wired to the server yet.

### 3. web/pages/download.html:169

**Verdict:** false | **Surface:** download

> It keeps itself up to date.

**Why it is wrong.** src/updater.rs:296 sets `let require_manifest = crate::release_update::embedded_pubkeys().is_provisioned();` and src/updater.rs:297-301 makes a release eligible only if `(!require_manifest || find_asset_url(&r.assets, MANIFEST_NAME).is_some())`, where MANIFEST_NAME = "release-manifest.json" (src/updater.rs:458). src/release_update.rs:52 compiles data/release/signing_pubkeys.json into every build, and src/release_update.rs:66 `is_provisioned()` returns true when both keys are non-empty. That file on disk today has ed25519 = "2ff5c5fca553e8ca4e1e32d6bf45694e1611b3c3a954a712c6475fc6cbc3556f" and a full 1952-byte dilithium key, so require_manifest is TRUE in every shipped build. Actual release state, from `gh api "repos/Shaostoul/Humanity/releases?per_page=20"` (the exact window src/updater.rs:388 fetches): v0.1297.1 manifest=false, v0.1297.0 false, v0.1296.0 false, v0.1295.0 false, v0.1294.3 false, v0.1294.2 false, v0.1294.1 TRUE, v0.1294.0 false, and false for every release down to v0.1290.1. Only v0.1294.1 is eligible. A user running v0.1297.1, the exact build this page hands them, hits src/updater.rs:326 `if !is_newer(target_clean, &self.current_version)` with target 0.1294.1 against current 0.1297.1, so the state becomes UpToDate and they are never offered anything again. A user on anything older than v0.1294.1 is offered v0.1294.1, which is six releases behind Latest.

**Accurate wording.** One download that does everything: the desktop app, the game, and the server are the same program. Update checking is built in, but right now only signed releases can install themselves, and the latest release is not yet signed, so check the releases page and grab new builds by hand for now.

### 4. web/pages/download.html:461

**Verdict:** false | **Surface:** download

> You never need to re-download manually.

**Why it is wrong.** Same gate as above. The only release in the updater's 20-release fetch window carrying release-manifest.json is v0.1294.1, confirmed by `gh release view v0.1294.1 --repo Shaostoul/Humanity --json assets` (release-manifest.json 689 bytes, release-manifest.json.sig.json 4577 bytes) and by its absence from v0.1294.2, v0.1294.3, v0.1295.0, v0.1296.0, v0.1297.0 and v0.1297.1. The fetched manifest at https://github.com/Shaostoul/Humanity/releases/download/v0.1294.1/release-manifest.json declares version "0.1294.1". So v0.1294.1 is literally the only version auto-update can install today, and every release after it must be downloaded by hand. There is no second update path: the only callers of download_version are src/gui/pages/settings.rs:3966 and src/gui/pages/hud.rs, both driven by the same Updater::evaluate_update.

**Accurate wording.** HumanityOS checks GitHub for newer versions when it starts. It will only install a release the operator has signed, so an unsigned release is skipped rather than trusted. Until the newest releases are signed, updating means downloading the new build from the releases page yourself.

### 5. docs/outreach/for_public_leaders.md:70-74 (shipped at data/library/for_public_leaders.md, served at https://united-humanity.us/library#for-public-leaders)

**Verdict:** false | **Surface:** outreach-briefs

> The entire project is released under CC0, the most permissive public-domain dedication that exists. Legally, that means any government, school, or community may fork it, translate it, rename it, rebrand it, adapt it to local crops and local law, and owes nothing to anyone: no fee, no credit, no permission.

**Why it is wrong.** The repository's own ../../LICENSES.md contradicts it. ../../LICENSES.md:65-68: "HumanityOS source is licensed as stated in the repository root. These third-party entries cover DATA, which is licensed separately from the code that reads it." ../../LICENSES.md:12-35 spells out two live obligations on shipped files: data/maps/regions/*.bin are OpenStreetMap derivatives under ODbL 1.0, where "Anything that DRAWS this data must show the credit where the drawing is shown" and "If you redistribute a release bundle containing data/maps/regions/, you carry this obligation forward." data/credits.ron marks OpenStreetMap, ATHYG and ESA Gaia with attribution_required: true, and a unit test in src/credits.rs fails the build if an attribution-required source has no surface showing its notice. data/library/CREDITS.md:67-72 adds ESA Gaia DR3 under CC-BY-SA-IGO 3.0 and Solar System Scope planetary textures under CC-BY 4.0. So a government that forks and rebrands does owe credit, and for the map regions owes share-alike too. This is the most consequential wrong sentence in the set because it is a specific legal assurance given to people who act on legal assurances, in a document that opens with "It is written to be checked, not believed."

**Accurate wording.** The project's own code, documents and game data are released under CC0, the most permissive public-domain dedication that exists: any government, school or community may fork it, translate it, rename it, rebrand it and adapt it to local crops and local law, owing us nothing and asking nobody. A few third-party datasets we ship keep their original open terms and travel with the bundle. The OpenStreetMap region files are ODbL, which asks for credit on any map drawn from them and for the same terms if you pass them on, and some star and planet imagery is CC-BY. Every one is listed with what it asks in ../../LICENSES.md in the repository. Nothing in that list costs money, and nothing can be revoked.

---

## MEDIUM

### 1. C:/Humanity/README.md:38

**Verdict:** false | **Surface:** readme

> Every line of code, every design doc, every commit is in the public domain ([CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)). Copy it, fork it, sell it, teach from it. **No attribution required.**

**Why it is wrong.** ../../LICENSES.md:31-35 states the opposite about files that are committed to this repo: "The region files are a Derivative Database... publicly distributing them means offering them under ODbL 1.0 as well. They are hereby offered under ODbL 1.0. If you redistribute a release bundle containing data/maps/regions/, you carry this obligation forward." `git ls-files data/maps/regions/` returns 4 tracked files (seattle-center.bin, seattle-center.dem.bin, silverdale.bin, silverdale.dem.bin), so those obligations are inside 'every commit'. ../../LICENSES.md:18 requires the credit "Map data (c) OpenStreetMap contributors, ODbL 1.0" wherever the data is drawn. CREDITS.md:67-72 adds ESA Gaia DR3 under CC-BY-SA-IGO 3.0 and Solar System Scope textures under CC-BY 4.0. data/credits.ron has three entries with `attribution_required: true` (lines 27, 63, 74), and src/credits.rs has a unit test that fails if such a source names no surface showing its notice. README.md never links ../../LICENSES.md at all. A forker who trusts this sentence and redistributes the repo breaks ODbL and two CC-BY licences.

**Accurate wording.** The code, the docs and the commits we wrote are public domain (CC0 1.0). Copy it, fork it, sell it, teach from it, no attribution required. A few real-world data files we ship carry their own licences: the OpenStreetMap map regions are ODbL 1.0, the Gaia star catalogue is CC-BY-SA-IGO 3.0, and the Solar System Scope textures are CC-BY 4.0. If you redistribute those files you carry those terms forward. The full list is in [../../LICENSES.md](../../LICENSES.md).

### 2. C:/Humanity/CONTRIBUTING.md:21-23

**Verdict:** false | **Surface:** readme

> # 3. Open the chat\n# Visit http://localhost:3210 in your browser ,  that's it.\n# No npm install, no build step, no Docker.

**Why it is wrong.** This is the first thing a new contributor is told to do and it returns 404. src/relay/mod.rs:1096-1098 sets the fallback to `ServeDir::new("client").fallback(ServeFile::new("client/index.html"))`. There is no `client/` directory in the repo (`ls -d client` fails, `git ls-files client/` is empty) and no reference to `client/` in the Justfile, scripts/, or any .github workflow, so nothing ever creates it. The project's own admin doc says so outright at docs/admin/SELF-HOSTING.md:66-67: "no build step populates `client/` -- so on a bare relay `/` returns 404."

**Accurate wording.** # 3. Open the chat\n# The relay serves the API and the WebSocket, not the website. Serve the web/\n# folder with any static file server, or copy web/ into a folder named client/\n# beside the binary and the relay will serve it at http://localhost:3210.\n# Either way: no npm install, no build step, no Docker.

### 3. C:/Humanity/README.md:17

**Verdict:** false | **Surface:** readme

> Built-in marketplace with listings, reviews, seller ratings, and escrowed trades.

**Why it is wrong.** The relay rejects escrow outright. src/relay/core/market_payloads.rs:594-597: "// wallet and escrow are RESERVED: reject until those addons ship" and the error "settlement.mode \"{mode}\" is not available: only \"directory\" is active (wallet/escrow are reserved)"; line 601 additionally refuses any listing that even mentions `escrow_policy_ref`. The marketplace page states the truth to users at web/pages/market.html:171: "The platform never handles payment: it lists and introduces, and you settle directly." The native market page has no buy button at all, per its own comment at src/gui/pages/market.rs:501-502: "escrow buying still waits for the trade-flow follow-up (no dead buttons)." Escrow does exist, but only for player-to-player item swaps inside the game (src/relay/storage/trading.rs:1-3, two-sided confirm lock), which is a different feature from the marketplace this row describes. Listings, reviews and seller ratings are real (src/gui/pages/market.rs:127, 367-426).

**Accurate wording.** Built-in marketplace with signed listings, reviews, and seller ratings. The platform introduces buyers and sellers and never touches the money, so you settle directly and no operator can freeze your sale. Escrow exists today for player-to-player item trades inside the 3D world; escrowed payment for real goods is not built.

### 4. C:/Humanity/README.md:16 (repeated as "Encrypted notes" under "What's working right now" at README.md:65)

**Verdict:** misleading | **Surface:** readme

> Kanban boards, calendars, private encrypted notes, skill tracking.

**Why it is wrong.** Notes are not encrypted by default on either client. On the web, web/pages/notes-app.js:18-20 says encryption is opt-in per note: "Each note can optionally have per-note passphrase encryption on top of the base localStorage layer. All data lives in localStorage under 'hos_notes_v1'"; newNote() creates every note with `encrypted: false` (line 142) and saveNotes() writes plain JSON (line 72). On the desktop app it is worse: src/gui/mod.rs:1293-1299 defines GuiNote with no serde derive and no encryption, and a grep across src/ finds no save or load path for notes anywhere, so desktop notes are an in-memory Vec that is lost on every restart. A user who writes a private note in the desktop app and restarts finds it gone.

**Accurate wording.** Kanban boards, calendars, notes with optional passphrase encryption, skill tracking. (Web notes stay on your device in browser storage; you turn on encryption per note. Desktop notes do not save between sessions yet.)

### 5. C:/Humanity/README.md:22

**Verdict:** misleading | **Surface:** readme

> Add the desktop app and the whole toolset - including the 3D world - runs offline with local saves.

**Why it is wrong.** The local save covers the game world only. src/persistence.rs:15-25 defines WorldSave as name, timestamp, game_time, player position/rotation/health, inventory, skills, constructions, weather, plus placed items, vehicles, crops, credits and quests. Nothing else in the toolset persists locally: native tasks arrive from the relay over the socket (src/lib.rs:17114 pushes GuiTask entries from a `task_list_response`, logged at 17121 as "Received {} tasks from server"), so the Kanban board is empty offline; native calendar events live in `state.cal_events`, a Vec initialised empty at src/gui/mod.rs:5208 with no save path anywhere; native notes are the same. So offline you get the 3D world and nothing else.

**Accurate wording.** Add the desktop app and the 3D world runs fully offline with local saves. The collaboration tools (chat, tasks, market, voting) are server-backed by design and pick back up when you reconnect.

### 6. C:/Humanity/README.md:137 (the "Run your own server" card, README.md:131-137)

**Verdict:** misleading | **Surface:** readme

> Under 10 minutes from zero to live.

**Why it is wrong.** The card shows only the compile-from-source path (`git clone`, then `cargo build --release --features relay --no-default-features`), and that build alone took 7 minutes 6 seconds on a GitHub-hosted CI runner: run 34060618146, job "Build relay (linux-x64, headless)", started 2026-09-06T21:17:40Z, completed 21:24:46Z. That leaves under three minutes for the clone, nginx, systemd and TLS on hardware slower than a CI runner. The project's own guide recommends the opposite path: docs/admin/SELF-HOSTING.md:26-27 says a Rust compiler is needed "only if you choose to build from source (Option B below) -- the normal path is a ready-made download, no compiler needed", and SELF-HOSTING.md:143 notes a 1 GB box provisions in minutes precisely because there is "no compiling".

**Accurate wording.** ### 🏠 Run your own server\n1. Download `HumanityOS-linux-x64` from [Releases](https://github.com/Shaostoul/Humanity/releases/latest)\n2. `chmod +x HumanityOS-linux-x64`\n3. `./HumanityOS-linux-x64 --headless`\n4. nginx + systemd in front\n\nNo compiler, no Docker. Under 10 minutes from download to live. Building from source instead takes about 10 minutes on its own. **[Full guide →](docs/admin/SELF-HOSTING.md)**

### 7. C:/Humanity/README.md:156

**Verdict:** misleading | **Surface:** readme

> | **Logs** | No analytics, no tracking pixels; the relay database never stores IP addresses (IPs touch memory only for abuse rate-limiting) |

**Why it is wrong.** The database half is true (no ip column anywhere in src/relay/storage/). The parenthetical "memory only" is not: src/relay/relay.rs:2516-2518 writes `tracing::warn!("Identify rate limit hit for ip={}...", client_ip, ...)` and relay.rs:2636-2638 writes `tracing::warn!("New-identity-per-IP cap hit for ip={}...")`, both of which land in the service log on disk. Separately, docs/reference/retention_and_deletion_semantics.md:146 records "Web-server IP logs: nginx rotation cut from 14 days to 2 on the VPS", and docs/admin/tor-onion-service.md:12 confirms "nginx logs are cut to 2 days for fail2ban only". Under a row labelled "Logs", the sentence reads as "we never log your IP", and the web server does for two days.

**Accurate wording.** | **Logs** | No analytics, no tracking pixels. The relay database has no IP column at all. The web server in front keeps access logs for 2 days so it can ban abusers, and the relay writes an IP to its log only when a rate limit trips. For no IP at all, reach the server over the Tor onion service. |

### 8. C:/Humanity/README.md:17 (the "Buy, sell, and trade" row)

**Verdict:** misleading | **Surface:** readme

> A separate multi-layer trust score (identity, vouching, activity - no surveillance) shows who you're dealing with.

**Why it is wrong.** The trust score is real (src/relay/storage/trust_score.rs computes VC count, vouching graph entropy, activity diversity and account age; GET /api/v2/trust/{did} answers live) but it appears nowhere in either marketplace UI. Grepping web/pages/market-app.js, web/pages/market.html and src/gui/pages/market.rs for "trust" returns nothing. The only surface that renders it is the standalone Identity page after you paste in a DID (web/pages/identity.html:214-222). The native Identity page cannot even do that: src/gui/pages/identity.rs is 196 lines with no HTTP client, and its own header calls it "a read-only viewer" that prints the endpoint string "GET /api/v2/trust/{did}" at line 137 as documentation. Placed in the buying-and-selling row, the claim promises a signal at the point of the deal that is not there.

**Accurate wording.** A separate multi-layer trust score (identity, vouching, activity, no surveillance) is computed for every DID and every input is published, so nobody has to trust a black box. You look it up on the Identity page today; wiring it into the marketplace listings is next.

### 9. web/pages/index.html:529 (live: https://united-humanity.us/ , "Know the rules where you live" card, under "Six things it does today.")

**Verdict:** misleading | **Surface:** homepage

> Rights and rules in plain language, narrowed from all of humanity down to your own locality. Summaries with sources, not legal advice.

**Why it is wrong.** The whole dataset is one chain through the author's own hometown. data/laws/laws.json defines exactly six jurisdictions: Humanity, Earth, United States, Washington, Kitsap County (line 30), Silverdale (line 36), and the 170 rules break down as humanity 9, usa 36, wa 96, kitsap 29, with earth 0 and silverdale 0. For any visitor outside Kitsap County, Washington, "your own locality" resolves to nothing. The file's own disclaimer (data/laws/laws.json:3) says: "This is a WORKED EXAMPLE, not the law... Entries may be incomplete, out of date, or simply wrong, and most have never been checked by a person." The Laws page itself repeats that honestly (web/pages/laws.html:145), so the homepage is the only surface that drops the caveat, while also filing the feature under "Six things it does today."

**Accurate wording.** Rights and rules in plain language, traced from all of humanity down to a single county. So far that chain is worked out for one place only, Kitsap County in Washington, as a proof of the format. Every entry cites its source, most have not been checked by a person yet, and none of it is legal advice.

### 10. web/pages/index.html:501 (live: https://united-humanity.us/ , "Grow your food" card, under "Six things it does today.")

**Verdict:** false | **Surface:** homepage

> Track seeds, beds and harvests, and what to plant next where you live.

**Why it is wrong.** The second half of that sentence describes a feature that does not exist. Grepping the whole tree for "what to plant", "plant next" and "recommend" finds the claim on web/pages/index.html:501 and nothing else. data/plants.csv carries seasons plus temp_min_c and temp_max_c but no hardiness zone, no frost dates and no planting calendar (its documented columns run growth_days through adverse_plants, lines 1 to 31), and no UI anywhere consumes a location to suggest a crop. The first half is real but is game-side, not a real-life garden log: crops round-trip in the world save (src/persistence.rs, "Growing crops (v0.863): every CropInstance round-trips"), while grow-area settings are explicitly not saved yet (src/gui/pages/inventory.rs:181, "In-memory edit config for one grow area (until garden persistence lands)").

**Accurate wording.** Track seeds, beds and harvests in the homestead you grow in the game. Advice on what to plant next where you live is not built yet.

### 11. web/pages/index.html:510 (live: https://united-humanity.us/ , card title)

**Verdict:** misleading | **Surface:** homepage

> Trade with people near you

**Why it is wrong.** Nothing in the market is proximity aware. `location` is a free-text field the seller types (src/relay/storage/mod.rs:1430) that is only ever displayed, never matched: the native market page uses it once, as a label (src/gui/pages/market.rs:329, `("Location:", &listing.location)`), and the web market filters only by category, condition and sort order (web/pages/market-app.js:541 to 555). There is no distance search, no geolocation, no radius and no region grouping anywhere in the market code. What you actually see is every listing on the relay you happen to be connected to, wherever the seller is.

**Accurate wording.** Trade with people on your server

### 12. web/pages/download.html:400

**Verdict:** false | **Surface:** download

> Works on any distribution. Right-click the zip, Extract Here, then run the HumanityOS file inside.

**Why it is wrong.** .github/workflows/build-desktop.yml:24-27 builds Linux on `os: ubuntu-22.04` for `target: x86_64-unknown-linux-gnu`, a dynamically linked glibc binary (glibc 2.35 on that runner). There is no musl target and no static link anywhere in the matrix. That binary will not start on musl distributions such as Alpine, and will not start on glibc older than 2.35, which rules out Debian 11 (2.31), Ubuntu 20.04 (2.31) and RHEL 9 (2.34). No Linux runtime dependencies are documented anywhere either; the only apt lines in the repo are docs/admin/SELF-HOSTING.md:385 (nginx) and docs/admin/tor-onion-service.md:28 (tor).

**Accurate wording.** Built on Ubuntu 22.04 for x86_64, so it runs on any glibc 2.35 or newer distribution (Ubuntu 22.04+, Debian 12+, Fedora 36+). Alpine and other musl systems need a build from source. Right-click the zip, Extract Here, then run the HumanityOS file inside.

### 13. web/pages/download.html:386 (and the identical Intel wording at :393)

**Verdict:** misleading | **Surface:** download

> macOS 12 or newer, Apple Silicon (M1 and later). Double-click the download to unpack it, then open HumanityOS.

**Why it is wrong.** `grep -rn "codesign|notarize|notarytool|xattr|quarantine" .github/` returns NONE FOUND. The macOS builds in .github/workflows/build-desktop.yml:29-38 are plain `cargo build --release` output, never codesigned with a Developer ID and never notarized. A browser download carries com.apple.quarantine, so Gatekeeper refuses to open it and the stated flow stops there. The page's only security-warning panel is titled "Windows SmartScreen Warning" (line 430) and explains the cause as a missing paid Windows code signing certificate. The one string matching "gatekeeper" on the page is line 453, "No corporate gatekeeper can pull the app," which is the metaphor, not the macOS system. Mac users are given no warning at all and an instruction that will not work.

**Accurate wording.** macOS 12 or newer, Apple Silicon (M1 and later). Unpack the download, then move HumanityOS somewhere you keep apps. macOS will refuse to open it the first time because we do not pay Apple's yearly notarization fee. Right-click the app and choose Open, then confirm, or run `xattr -dr com.apple.quarantine HumanityOS` once. Add a matching macOS section to the security-warning panel below.

### 14. web/pages/download.html:468 (repeated at :480 as "about 20 MB of memory and half a percent of one core")

**Verdict:** stale | **Surface:** download

> measured on our live server, the relay uses about 20&nbsp;MB of memory and a half-percent of one CPU core

**Why it is wrong.** Measured live on the production VPS just now over the humanity-vps SSH alias: `ps -eo rss=,pcpu=,etimes=,args= | grep HumanityOS` returned `50748  0.3  3380  /opt/Humanity/target/release/HumanityOS --headless`. That is 50,748 KB resident, so about 50 MB, not about 20 MB, and the process was only 3380 seconds old, so it had not yet grown into a steady state. The CPU half of the claim holds at 0.3 percent. The conclusion the paragraph draws still survives (a 1 GB box runs this fine), only the memory number is wrong by about 2.5x.

**Accurate wording.** It is featherweight: measured on our live server right now, the relay uses about 50 MB of memory and under half a percent of one CPU core. A donated laptop or a $4/mo VPS runs it comfortably (floor: 1 GB RAM, 1 core, 20 GB disk).

### 15. docs/outreach/for_public_leaders.md:87-90

**Verdict:** false | **Surface:** outreach-briefs

> Community coordination, marketplace listings, shared tasks, and governance decisions happen in public, recorded as signed entries that cannot be quietly altered.

**Why it is wrong.** True for governance, false for the marketplace and tasks. Governance really is signed: src/relay/storage/governance.rs:1-6, "A proposal is a signed_object of type proposal_v1. A vote is a signed_object of type vote_v1." Chat messages carry a signature column (src/relay/storage/mod.rs:501). But the other two named surfaces are plain mutable SQLite rows with no signature at all. src/relay/storage/mod.rs:849-860 defines project_tasks (id, title, description, status, priority, assignee, created_by, created_at, updated_at, position, labels) with no signature column, and src/relay/storage/mod.rs:1430-1444 defines marketplace_listings (id, seller_key, seller_name, title, description, category, condition, price, payment_methods, location, images, status, created_at, updated_at) with no signature column either. A server operator can silently edit a listing's price or a task's assignee and nothing detects it. In a document whose closing section is titled "The guarantee," naming an integrity property that two of the four surfaces do not have is the exact failure the document promises not to commit.

**Accurate wording.** Governance proposals and votes are recorded as signed objects that cannot be quietly altered, and every chat message carries its sender's signature. Marketplace listings and shared tasks are public but are ordinary server records today, which means the person running the server could edit them without leaving a trace. Signing those is on the list. Until it lands, verify them the way you would verify any hosted record, and know that the argument for trusting the server does not yet extend to those two.

### 16. docs/outreach/for_public_leaders.md:49-52

**Verdict:** false | **Surface:** outreach-briefs

> The desktop app keeps its data on the user's own device and is designed to work without an internet connection: the library, the plans, the records stay usable in a blackout or a remote valley.

**Why it is wrong.** Two of the three named things are local; "the plans" is precisely the one that is not. The library is local: src/lib.rs:1459 loads it from the on-disk data directory via load_library(&data_dir), and Notes are local too. But the desktop task board has no local persistence whatsoever. The only writes to state.gui_state.tasks in the entire crate are clear() followed by push() from the relay's task_list_response (src/lib.rs:17073, 17114, 17175, 17216); there is no tasks field in AppConfig (src/config.rs) and no save path in src/persistence.rs. Boot the app in a blackout or a valley with no signal and the task board is empty, every time. This is the claim a leader is most likely to test in exactly the scenario it names.

**Accurate wording.** The desktop app keeps the library, your notes and your records on the user's own device, so they stay readable in a blackout or a remote valley, and more importantly the skills it teaches keep working when nothing else does. Shared task lists are today's exception: they live on the server and need a connection. Making them work offline and sync later is on the list.

### 17. docs/outreach/how_humanityos_helps.md:37-38 (shipped at data/library/how_humanityos_helps.md; this is the most-linked document of the set)

**Verdict:** false | **Surface:** outreach-briefs

> Everything, the code, the data, the documents, is released into the public domain. No company owns it. No one ever will.

**Why it is wrong.** "the data" is the wrong word. ../../LICENSES.md:65-68 says exactly the opposite: "HumanityOS source is licensed as stated in the repository root. These third-party entries cover DATA, which is licensed separately from the code that reads it." The shipped OpenStreetMap region files (data/maps/regions/silverdale.bin, seattle-center.bin and their .dem.bin companions) are ODbL 1.0 with an attribution and share-alike obligation stated at ../../LICENSES.md:12-35; data/credits.ron flags OpenStreetMap, ATHYG and ESA Gaia as attribution_required: true; data/library/CREDITS.md:67-72 lists Gaia DR3 as CC-BY-SA-IGO 3.0 and the Solar System Scope textures as CC-BY 4.0. The sentence's real point (no company owns it, no one ever will) survives intact once the data carve-out is named.

**Accurate wording.** The code and the documents are released into the public domain, and so is the game data we made ourselves. A few outside datasets we ship, such as the OpenStreetMap town maps and some star and planet imagery, keep their own free licences, which ask for credit rather than money; ../../LICENSES.md lists every one. No company owns HumanityOS. No one ever will.

### 18. docs/outreach/applications/futo-microgrant.md:51-53 (sent to grantapps@futo.org 2026-08-02 per the outcome log at line 96)

**Verdict:** false | **Surface:** funders-and-books

> Public domain (CC0), not just open source. Nobody, including me, can ever fence it off, relicense it, or sell it back to the people it was built for. Capture is structurally impossible.

**Why it is wrong.** Same evidence as the funders document: LICENSE is CC0 1.0, which imposes no conditions on derivative works. Under CC0 a third party can absolutely relicense their own modified version under proprietary terms and sell it. 'Capture is structurally impossible' is the strongest form of the error and it is in a letter already sent to a funder whose entire thesis is software ownership, so it is the claim most likely to be checked and the most damaging if it is.

**Accurate wording.** Public domain (CC0), which goes further than open source in one direction and not at all in another. Nobody, including me, can withdraw it, revoke a license, or make the public copy disappear, because there is no license to revoke. Anyone can fork it, including a company that wants to sell its own build; CC0 does not forbid that and I do not want it to. What matters is that the free version can never be taken away from the people using it.

### 19. docs/outreach/for_funders_and_sponsors.md:65-67 (identical in data/library/for_funders_and_sponsors.md, live)

**Verdict:** stale | **Surface:** funders-and-books

> Roughly 1,000 releases have shipped in the open so far, each one visible and verifiable at https://github.com/Shaostoul/Humanity.

**Why it is wrong.** The GitHub releases API reports 2,039 published releases (repos/Shaostoul/Humanity/releases?per_page=1 returns a Link header with rel="last" at page 2039), and the repository has 2,048 tags. Even on 2026-08-03, when this text was written, 1,714 tags already existed. Three different numbers are published across the surface for the same thing: 'Roughly 1,000' here, 'roughly 1,100 public releases' in docs/outreach/applications/futo-microgrant.md:35, and '~1,300 releases' in data/library/CREDITS.md:37. All three are low, and the sentence invites the reader to go count.

**Accurate wording.** More than 2,000 tagged releases have shipped in the open so far, 2,039 as of 6 September 2026, each one visible and verifiable at https://github.com/Shaostoul/Humanity. Re-derive this number from `gh release list` whenever the page is edited rather than restating it from memory, and use the same number in the Credits page and any application.

### 20. docs/outreach/for_funders_and_sponsors.md:144 (identical in data/library/for_funders_and_sponsors.md, live)

**Verdict:** misleading | **Surface:** funders-and-books

> - more languages, so the tools reach people the current five miss

**Why it is wrong.** There are five translation files (data/i18n/{en,es,fr,ja,zh}.json) but each contains only 37 leaf strings, almost entirely nav labels plus a dozen words like Save, Cancel, Search. Nothing in the product reads them: `grep -ro data-i18n web/` returns 0 hits, and `grep -rn 'i18n.t(' web/` returns exactly one hit, at web/shared/i18n.js:9, inside the module's own usage comment. There are no call sites in application code. The native egui client has no localization at all (no language control in src/gui/pages/settings.rs). The web Settings page does offer a language selector that stores a preference and fires a languagechange event 'so any i18n-aware UI re-renders' (web/pages/settings-app.js:168-173), but no UI is i18n-aware, so a funder who selects Spanish gets an English app. 'The current five' tells a reader the software already works in five languages. It works in one.

**Accurate wording.** - translation, starting from almost nothing. Five language files exist, but they cover 37 strings and no screen reads them yet, so today the app is English only. Real localisation is one of the clearest things money would buy, and one of the biggest barriers between these tools and the people who need them most.

---

## LOW

### 1. C:/Humanity/CHANGELOG.md:3

**Verdict:** stale | **Surface:** readme

> All notable changes to HumanityOS.

**Why it is wrong.** The file's newest entry is "## v0.43.0 ,  Comprehensive Game Data & Documentation (2026-03-24)" (CHANGELOG.md:8) and it closes with "*Spanning from initial commit (2026-01-16) through v0.43.0 (2026-03-24).*". `git log -1 -- CHANGELOG.md` returns commit f1cbf92b dated 2026-03-24. The current release is v0.1297.1, published 2026-09-06 (gh release list), and Cargo.toml reads 0.1297.1. The file is missing roughly 1,254 releases and five and a half months, and carries no note saying it was superseded. A visitor opening the changelog of a project whose whole pitch is verifiability sees a record that silently stops.

**Accurate wording.** Historical changelog, kept through v0.43.0 (2026-03-24). It is no longer maintained by hand: the per-release record now lives in [GitHub Releases](https://github.com/Shaostoul/Humanity/releases) and the narrative record in [docs/history/](docs/history/) and the [devlog](https://united-humanity.us/devlog).

### 2. C:/Humanity/CONTRIBUTING.md:137 (repeated at CONTRIBUTING.md:147, "If it touches voice/video → `chat-voice.js`")

**Verdict:** stale | **Surface:** readme

> | `chat-voice.js` | Voice rooms, 1-on-1 calls, video panel, unified right sidebar |

**Why it is wrong.** `ls web/chat/` shows no chat-voice.js. The voice code was split into five files: chat-voice-calls.js, chat-voice-modal.js, chat-voice-rooms.js, chat-voice-streaming.js and chat-voice-webrtc.js. getUserMedia and getDisplayMedia now live in chat-voice-webrtc.js (lines 41, 91, 179, 232). The same table also omits six modules that exist and are loaded: pq.js, chat-dm-store.js, chat-groups-p2p.js, chat-live.js, chat-privacy.js and chat-onboarding.js. A new contributor told to open chat-voice.js finds nothing there.

**Accurate wording.** | `chat-voice-*.js` | Voice rooms, 1-on-1 calls, video panel and screen share, split across `-calls`, `-rooms`, `-webrtc`, `-modal`, `-streaming` |\n| `pq.js` | Dilithium3 and Kyber768 primitives |\n| `chat-dm-store.js` | Encrypted local DM history and the client-side social graph |\n| `chat-groups-p2p.js` | End-to-end encrypted group conversations |\n\n(and at line 147: "If it touches voice/video → the `chat-voice-*.js` module for that surface")

### 3. C:/Humanity/CONTRIBUTING.md:62, 65, 84 (the ~30 modules figure repeats at CONTRIBUTING.md:192)

**Verdict:** stale | **Surface:** readme

> │   │   ├── api.rs          ← REST API (~2800 LOC).  /  │   │   └── storage/        ← SQLite domain modules (~30 files).  /  ├── data/                   ← Hot-reloadable game/config data (CSV/TOML/RON/JSON, ~108 files).

**Why it is wrong.** Measured today: `wc -l src/relay/api.rs` is 4475 lines, not ~2800. `ls src/relay/storage/*.rs | wc -l` is 49, not ~30. `find data -type f | wc -l` is 346, not ~108. README.md:206 already carries the corrected "40+ SQLite domain modules", so the two front-door files disagree with each other as well as with the tree.

**Accurate wording.** │   │   ├── api.rs          ← REST API (~4500 LOC).\n│   │   └── storage/        ← SQLite domain modules (~50 files).\n├── data/                   ← Hot-reloadable game/config data (CSV/TOML/RON/JSON, ~350 files).\n\n(and at line 192: "is split by domain (~50 modules)")

### 4. C:/Humanity/README.md:216

**Verdict:** stale | **Surface:** readme

> │   ├── pages/               ← Standalone pages (36 of them)

**Why it is wrong.** `ls web/pages/*.html | wc -l` returns 43. The count undersells the project rather than overselling it, but it is a stated number in a repo whose argument is that its numbers can be checked, and checking it fails.

**Accurate wording.** │   ├── pages/               ← Standalone pages (43 of them)

### 5. web/pages/download.html:477

**Verdict:** stale | **Surface:** download

> The full app (about 62&nbsp;MB). The desktop program you download above.

**Why it is wrong.** Asset sizes on v0.1297.1 from `gh release view v0.1297.1 --repo Shaostoul/Humanity --json assets`: HumanityOS-windows-x64.exe = 66,255,872 bytes (66 MB), HumanityOS-macos-arm64.zip = 170,908,861 bytes (171 MB), HumanityOS-macos-x64.zip = 172,112,602 bytes (172 MB), HumanityOS-linux-x64.zip = 174,841,244 bytes (175 MB). And "the desktop program you download above" is not the same file on every platform: this page's own download button prefers the zip for mac and Linux, per the `patterns` object at web/pages/download.html:596-602 (`macos: [/macos-arm64\.zip$/i, ...]`, `linux: [/linux-x64\.zip$/i, ...]`) while Windows gets the bare `.exe`. So the number is roughly right for Windows only and understates the actual mac and Linux download by about 2.8x.

**Accurate wording.** The full app. The desktop program you download above: about 66 MB for the Windows .exe, or about 175 MB for the mac and Linux bundles, which carry the game data alongside the binary. It is the server too: run it with --headless and it hosts without a window.

### 6. web/pages/download.html:258 (the "Humanity: The Game" module card, shown next to "✓ Included" at :262)

**Verdict:** misleading | **Surface:** download

> ~500 MB to 2 GB

**Why it is wrong.** Nothing on the release is anywhere near that size. The largest desktop artifact on v0.1297.1 is HumanityOS-windows-x64.zip at 177,859,983 bytes (178 MB), and the bare exe the Windows button actually serves is 66,255,872 bytes (66 MB). Sitting under a green "✓ Included" badge, this reads as the size a reader is about to download. The only way an install approaches 500 MB is by opting in afterwards to the Ultra star catalog ("~350 MB", src/renderer/stars.rs:1644) and the Milky Way glow texture ("~99 MB PNG", src/renderer/stars.rs:831), both fetched on demand from inside the app.

**Accurate wording.** Included in the download, about 66 MB to 180 MB depending on platform. Optional high-detail star and galaxy data can be fetched later from inside the app and adds up to about 450 MB more.

### 7. docs/outreach/privacy_by_architecture.md:100-101

**Verdict:** false | **Surface:** outreach-briefs

> Both are now gone. Contacting a seller opens a normal end to end encrypted direct message.

**Why it is wrong.** True in the desktop app, false on the website, which is where a Library reader is already standing. The app does it right: src/gui/pages/market.rs:455-488 draws a Contact section with a "Message Seller" button calling chat::open_dm_conversation, captioned "Messages go directly to the seller, end-to-end encrypted." The web marketplace was never converted. web/pages/market-app.js:327-336 still renders a "Type a message..." box on every listing, and line 1245-1256 sends {type: 'listing_message_send'} over the relay socket. That message type was deleted server-side: src/relay/relay.rs:1760-1766 records "listing_message_*: buyer-seller listing threads ... Marketplace contact rides sealed-sender E2EE DMs now," and no such variant remains in the RelayMessage enum, so the JSON fails to deserialize and is dropped at src/relay/relay.rs:3475-3478. Confirmed still live: `curl.exe -s "https://united-humanity.us/pages/market-app.js" | grep -c listing_message_send` returns 1. A buyer types a question to a seller, presses Send, sees the box clear and "No messages yet," and the message goes nowhere with no error shown.

**Accurate wording.** Both are now gone. In the desktop app, contacting a seller opens a normal end to end encrypted direct message. The web marketplace has not been moved over yet and its old message box does nothing at all, which is a bug we are fixing; until then, contact a seller from the app or by direct message in chat.

### 8. docs/outreach/for_funders_and_sponsors.md:80 (identical in data/library/for_funders_and_sponsors.md:80, live)

**Verdict:** false | **Surface:** funders-and-books

> Patreon: about $21 net over the last twelve months.

**Why it is wrong.** The line sits under the heading 'Income, per month:' (line 77) yet states a twelve month figure, and the document's own total two lines later is 'Total: about $620 a month' (line 84), which is $600 plus $21 per month. If Patreon were $21 across a whole year the total would be about $602, not $620. The session record confirms the intended figure is monthly: 'Income: $600 Sponsor-A-Can stipend + ~$21 Patreon net' (docs/history/2026-08-01.md:252-253). As written, the line understates Patreon income twelvefold and contradicts the arithmetic printed directly beneath it, which is exactly the kind of internal inconsistency an NLnet stage two verification catches first.

**Accurate wording.** - Patreon: about $21 a month, averaged over the last twelve months. (Twelve years of history there: $15,812 lifetime net since October 2014, with a peak around $220 a month in 2019.)

### 9. docs/outreach/for_funders_and_sponsors.md:11-13 (identical in data/library/for_funders_and_sponsors.md, live)

**Verdict:** misleading | **Surface:** funders-and-books

> HumanityOS is released into the public domain under CC0, which means nobody owns it

**Why it is wrong.** True of everything the project writes, but not of everything it ships. The official data package distributes data/planets/moon_albedo.bin and data/planets/mars_albedo.bin (both listed in the data-manifest-v0.1297.1.json release asset, whose source_dirs are data, assets/icons, assets/shaders), and the project's own Credits page attributes those bakes to 'Solar System Scope textures (Moon, Mars planetary imagery bakes) - CC-BY 4.0' (data/library/CREDITS.md:70-72). CC-BY 4.0 requires attribution and cannot be re-dedicated to CC0, so some files in the download are owned and carry a condition. Credits already discloses this correctly; the funders brief's blanket sentence does not, and a licensing review is precisely where NLnet stage two looks.

**Accurate wording.** Everything we write is released into the public domain under CC0, so nobody owns it. A few third-party data files we ship with it, such as planetary imagery and star catalogues, stay under their own free licenses and are credited by name on our Credits page. Nothing in the stack is proprietary and nothing requires a payment or a permission.

### 10. web/pages/settings.html:1036 (the 'Analytics' toggle at line 1035, live at https://united-humanity.us/settings)

**Verdict:** misleading | **Surface:** funders-and-books

> Share anonymous usage data to improve HumanityOS

**Why it is wrong.** This contradicts the public claim 'No surveillance. No ads, no telemetry, nothing harvested' in docs/outreach/applications/futo-microgrant.md:46-47. The letter is the accurate one: the toggle is inert. Its only other appearance in the entire tree is the default value 'analytics: false' at web/shared/defaults.js:49, and nothing reads that value or transmits anything. So the code claim holds, but a FUTO or NLnet reviewer who opens Settings to verify 'no telemetry' finds a switch offering to send usage data, and has no way to know it is dead. The page should be fixed, not the letter.

**Accurate wording.** Remove the toggle entirely, since nothing implements it. If a placeholder is wanted, state the truth: 'Usage analytics: none. HumanityOS collects no usage data, so there is nothing to turn on or off. If that ever changes it will be opt-in and announced first.'
