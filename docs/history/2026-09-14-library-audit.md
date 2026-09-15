# Library documentation audit, 2026-09-14

An eight-batch adversarial sweep of the 80 non-Constitution documents in the
in-app Library. 223 agents, 107 raw findings, each checked by two independent
verifiers whose instruction was to REFUTE it.

## How to read the confidence column

The sweep let a finding survive on 1 of 2 verifier votes, which is lenient, so
the raw "77 confirmed" overstates the result. This document splits them:

- **Unanimous (55)**: neither verifier could refute it. Treat as real, but
  confirm the specific line before editing, because a verifier checking a claim
  is not the same as a verifier checking your fix.
- **Contested (22)**: one verifier refuted it. These are UNADJUDICATED, not
  real and not disproven. Several are genuine disagreements about which of two
  conflicting primary sources to follow.

Nothing here is a style opinion; the sweep was told to report only verifiable
defects and to cite a URL or a repo file:line for each.

## STATUS: all 55 unanimous findings closed (v0.1306.2 through v0.1306.8)

Worked in order of what could actually hurt someone, then what misleads, then
what wastes a contributor's afternoon:

| Release | What it closed |
|---|---|
| v0.1306.2 | The two criticals: the shared `API_SECRET`, and social recovery promised with no implementation |
| v0.1306.3 | Things that break a server or burn someone: 4 more SELF-HOSTING defects, 6 fire staff (two wrong flash points, both erring toward looking safe) |
| v0.1306.4 | Claims the code contradicts: three Accord promises, the CC BY-SA licensing framing, the discarded chat signature, English-only, the offline claims |
| v0.1306.5 | Peppers on the wrong pollination list, the missing drowning hazard, a solar figure that did not reconcile, two distribution layers documented as live that are not |
| v0.1306.6 | Contributor docs describing a deleted Real/Sim toggle, a removed `server/` directory, and shipped systems called "early/planned" |
| v0.1306.7 | Four modding-guide dead ends: a rebuild that is not needed, a working feature documented as broken, a file nothing renders, a hot-reload that does not happen |
| v0.1306.8 | Every cross-document link in the Library (fixed in the build, not by hand), ui-system's drifted transcriptions, three removed APIs in ai-onboarding, the ROADMAP staleness |

**Three findings were recorded rather than fixed**, because they are product
decisions rather than documentation defects, and they now live in
`docs/accord/conformance_gaps.md`: governance votes are trust-weighted rather
than one-per-person, votes are publicly attributable rather than confidential,
and friendship certificates cannot be revoked. The Accord keeps its principles;
the gaps are written down so no reader is misled meanwhile.

**One is a server fix, not a doc fix**: `git.united-humanity.us` is absent from
its TLS certificate's SAN list, so hundreds of per-file URLs per release point
at a host no validating client will connect to. The certbot command that closes
it is recorded in `torrent-infrastructure.md`. Deliberately not run from a
session, being production TLS on the live site.

**The 22 contested findings below remain unadjudicated** and were not acted on.

---

## Fixed in v0.1306.2

Both criticals, confirmed by hand before the fix:

- **SELF-HOSTING.md** wrote the production `.env` inside a quoted heredoc, so
  `API_SECRET=$(openssl rand -hex 32)` was written literally and every node that
  followed the document shared one published secret.
- **getting-started.md, humanitys_mission.md, ROADMAP.md** promised social
  recovery as a seed-phrase safety net. No Shamir implementation exists in `src/`
  or `web/`; the recovery page can look up and display shares but nothing creates
  them. A reader who believed it could lose their identity permanently.

## Unanimous findings, open

Ordered by file. Severity is the sweep's, not re-judged here.

### `01-VISION.md`

- **[high]** 01-VISION describes a "Real/Sim toggle" that was deleted from the product in v0.197.0
  - Where: line 19 ("Product shape" section)
  - Why: This is the second document a new contributor or AI agent reads, and it states the product's core interaction model. It describes a control that does not exist and was deliberately removed, so a reader will design pages around a mode switch the codebase rejects. It also directly contradicts two-realities.md line 16 in the same shipped Library, which tells the reader the toggle was removed and must not be reintroduced.
  - Fix: In docs/contributor/01-VISION.md, replace the toggle sentence with the current model from docs/design/two-realities.md: the app chrome is always real, the game is entered through Play, and the two realities are separated by navigation rather than a mode switch. Then run `node scripts/build-library.js` to resync data/library/.

### `06-SOURCE-OF-TRUTH-MAP.md`

- **[high]** 06-SOURCE-OF-TRUTH-MAP claims domain systems have no dedicated Rust modules and are "early/planned"
  - Where: line 44 ("Important gap") and lines 97-101 (section D, Systems domains)
  - Why: This file's stated job is to let a newcomer "quickly tell what is already real vs planned," and it is the file 00-START-HERE and 04-CONTRIBUTING route people to for that judgement. Reading section D as "early/planned" is the precise failure mode CLAUDE.md's "never rebuild what exists" rule exists to prevent.
  - Fix: Rewrite section D and the "Important gap" paragraph in docs/contributor/06-SOURCE-OF-TRUTH-MAP.md against the real src/systems/ inventory and tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS (which now names only EcologySystem, HydrologySystem, DisasterSystem and PlacementSystem as unregistered). Rerun `node scripts/build-library.js`.

### `ROADMAP.md`

- **[medium]** ROADMAP.md is 376 releases stale while presenting itself as the single source of truth
  - Where: lines 3-8 (the framing), lines 40-75 ("Right now"), lines 420-421
  - Why: ONBOARDING.md line 60 tells readers STATUS.md and ROADMAP.md "are living documents updated every release, unlike the snapshot below", and this file's own "Right now" section is a strict-ranked active queue. A reader on the public roadmap page is told what is being worked on right now and gets the July picture, with the project's single largest completed arc since then missing entirely.
  - Fix: Regenerate the roadmap (and data/roadmap.json via scripts/roadmap-to-json.js) against current PRIORITIES.md: refresh "Right now", add the privacy arc, and replace the hardcoded "~v0.930 with ~900 tagged releases" with the current numbers or with no number at all.

- **[medium]** ROADMAP.md marks social recovery `[done]` and member-directory opt-out `[next]`; both statuses are wrong
  - Where: lines 125-126 and lines 132-133
  - Why: The `[done]` tag on social recovery is the claim that backs getting-started.md's seed-phrase reassurance, so the two documents reinforce each other's error. The stale `[next]` understates what the platform already protects, which matters for a privacy-first project's public roadmap.
  - Fix: Move social recovery to `[building]` or `[planned]` and drop the "never locks you out forever" clause. Move member-directory opt-out to `[done]` and fold in the rest of the shipped privacy work (sealed-sender DMs, the follows-graph removal, friendship certificates, DM padding, Tor onion service).

- **[medium]** ROADMAP.md's own link to the public roadmap 404s
  - Where: line 7
  - Why: The link text already says the right URL; only the href is wrong, so a reader who clicks rather than retypes lands on a 404 from the project's own roadmap document. The same /pages/ prefix error would break any other doc that copied this pattern.
  - Fix: Change the href to https://united-humanity.us/roadmap to match the link text and the nginx clean-URL rule.
  - Source: https://united-humanity.us/roadmap

### `SELF-HOSTING.md`

- **[high]** SELF-HOSTING nginx config puts limit_req_zone inside server {}, which nginx refuses to load
  - Where: lines 411-412, inside the `server { listen 443 ssl http2; ... }` block
  - Why: `nginx -t` fails with `"limit_req_zone" directive is not allowed here`, and the guide's very next step is `sudo systemctl restart nginx` (line 469) with no config test in between. On a box that was already serving, that restart takes the site down and it does not come back. The repo already learned this lesson once and wrote it down; the shipped self-hosting guide still ships the landmine.
  - Fix: Move both lines into a snippet outside the vhost, exactly as provision-vps.sh does: `/etc/nginx/conf.d/humanity-limits.conf`. Leave only the `limit_req zone=...` consumers inside the location blocks, and add `sudo nginx -t` before the restart.
  - Source: https://nginx.org/en/docs/http/ngx_http_limit_req_module.html

- **[high]** Quick Start hands self-hosters the desktop GPU binary; the purpose-built relay artifact is never mentioned
  - Where: lines 36-50 ("Option A: download the ready-made program (recommended)")
  - Why: The recommended path for the guide's primary audience points at the wrong artifact. The desktop Linux build links winit/wgpu/egui/cpal and needs libasound2 and libudev present (the workflow installs libasound2-dev, libudev-dev to build it); a minimal Debian VPS has neither, so it fails at the dynamic linker before printing anything useful. A correct, smaller, checksummed relay binary is published on the same release and is what scripts/provision-vps.sh actually fetches, and this document never names it.
  - Fix: Change Option A to `wget https://github.com/Shaostoul/Humanity/releases/latest/download/HumanityOS-relay-linux-x64` plus the matching `.sha256`, show the `sha256sum -c` step, and note that the platform desktop binaries are the desktop app, not the server. Windows/macOS have no relay artifact today, so say so rather than implying `--headless` on those downloads is the server path.

- **[high]** The Production nginx config never serves the website; the doc contradicts itself about it
  - Where: lines 65-70 versus the nginx block at lines 446-450
  - Why: A self-hoster follows the Production section end to end and gets a server whose homepage and every web page returns 404, exactly the failure the doc warned about two sections earlier and said this section would fix. The later "Your own homepage" section (which relies on `/var/www/humanity-site/index.html`) also cannot work, because no directive in the given config ever reads from disk.
  - Fix: Replace the hand-written nginx block with instructions to install `scripts/nginx/humanity.conf` and substitute the domain (the same two steps provision-vps.sh:348-352 performs), or at minimum add `root /var/www/humanity; index index.html;` and a `try_files $uri $uri.html $uri/ =404;` in `location /`, with the relay proxy confined to `/ws`, `/api/` and `/uploads/`.

- **[medium]** The systemd unit makes the backups directory read-only, so the in-app "Back up now" the same doc promises cannot write
  - Where: lines 356-358 (systemd unit) versus lines 162-166 ("Backups and restore")
  - Why: An operator who follows this document gets a hardened unit and a documented backup button that errors out on every click, plus an empty backup list that looks like "no backups yet" rather than "cannot write". The restore recipe at line 175 (`cp backups/<chosen>.db data/relay.db`) then has nothing to restore from.
  - Fix: Add `/opt/Humanity/backups` to ReadWritePaths in the unit (`ReadWritePaths=/opt/Humanity/data /opt/Humanity/backups`), or note that the in-app backup path needs it. The in-process encrypted snapshots write to `<db_dir>/backups` and are unaffected, which is why this fails silently for the manual path only.
  - Source: https://man7.org/linux/man-pages/man5/systemd.exec.5.html

### `ai-onboarding.md`

- **[high]** ai-onboarding.md tells agents to use key rotation when compromised; the handler was removed
  - Where: line 381 (FAQ), repeated at line 102 and line 254
  - Why: This is the security-incident answer in the document AI participants are told to follow, and it describes a mechanism that no client can invoke and no server will accept. An agent whose key is compromised follows this, finds nothing, and has no documented fallback, while its old key stays live on every federated server. The same claim appears twice more as a flat statement of capability (lines 102 and 254), so a reader has no signal that it is stale.
  - Fix: Remove all three claims. State the real position: there is no live key-rotation path today; a compromised key means generating a fresh identity from a new BIP39 seed and telling contacts out of band. If the signed-object `key_rotation_v1` path is meant to be the successor, say so and say it is not yet wired end to end.

- **[medium]** ai-onboarding.md's protocol table lists a `dm` WebSocket message that no longer exists
  - Where: line 220, in the WebSocket Protocol message-type table
  - Why: This is the reference table an AI agent codes its client against. Sending `{"type":"dm"}` is rejected as an unknown message and the agent has no pointer to the three messages it actually needs, nor to the fact that one DM is two dm_put deposits (recipient copy plus self copy).
  - Fix: Replace the row with `dm_put` / `dm_fetch` / `dm_purge` and describe the sealed-sender shape: the sender identity is signed inside the ciphertext, the relay stores a sender-less expiring mailbox, and one send is two deposits.

- **[medium]** ai-onboarding.md documents a `hum/solana/v1` KDF path the code says never shipped
  - Where: line 53, step 1 of "Creating Your Identity"
  - Why: An agent implementing wallet derivation against a KDF path that does not exist produces an address that does not match what the chat client derives, so funds sent to it are unreachable from the app. The same stale label also appears in web/pages/settings.html:939, which is outside this batch but should be fixed together.
  - Fix: State the real derivation: the Solana wallet is the Ed25519 keypair derived directly from the BIP39 seed scalar, the same one the chat client uses (web/chat/crypto.js `extractSolanaKeypair`). Drop the `hum/solana/v1` label.

### `audio-file.md`

- **[low]** Three code references have drifted line numbers or values presented as exact
  - Where: audio-file.md line 28 and line 46; vehicle.md line 67; plant.md line 44
  - Why: Each is introduced with an exactness claim ("exactly as written", "the real tomato line from the game", a specific line number), which is what makes them worth fixing: a reader who diffs against the real file and finds a mismatch stops trusting the rest of the guide. None of them breaks the instructions themselves, since the column counts and formats are still right.
  - Fix: Re-copy the two example lines from the current data files and drop the two hardcoded source line numbers in favour of the symbol names (`SoundCatalog::load` in src/lib.rs, `VEHICLE_KITS_RON` in src/embedded_data.rs), which do not drift.

### `collecting_rainwater.md`

- **[medium]** Rain barrel guide never names drowning as a hazard, the reason extension guidance gives for keeping the lid secured
  - Where: section "Build the base like it matters" lines 97-113, and "Setting it up" step 5 at lines 135-138
  - Why: The guide is explicit and thorough about every other hazard it covers: the 460 lb tipping load ("a falling barrel can seriously injure a child or pet standing next to it"), mosquitoes, ladder falls, asbestos roofing, and water into basements. It presents the screen as "your mosquito defense" only. A reader who understands the screen as an insect measure has no stated reason not to lift the lid to dip a watering can, or to leave a barrel uncovered between fixes, which is exactly the state extension guidance warns about. 50 gallons of open water at toddler height is a drowning hazard independent
  - Fix: Add drowning to the hazard framing in two places: in the base/safety list around line 112, state that a barrel of standing water is a drowning risk to a small child or pet and the lid and screens must stay secured at all times, not only during mosquito season; and at step 5 (line 136) change "This is your mosquito defense" to say the cover is both the mosquito defense and the child-and-animal barrier. Also state plainly that the water is drawn from the spigot and the barrel is never opened to dip.
  - Source: https://water.rutgers.edu/projects-and-programs/rain-barrels/

### `communication_and_association.md`

- **[high]** Contact consent cannot actually be revoked: friendship certificates are permanent and unrevocable, and unfollow does not close the DM pathway
  - Where: lines 24-27 ("Consent in contact") and line 76 (binding requirement 1), against C:/Humanity/data/library/consent_and_control.md lines 72-85 and line 40
  - Why: Once you have issued a friendship certificate, the holder has unlimited DM access to you forever. The relay verifies the certificate statelessly against a preimage that contains only a domain tag and the two public keys, so there is nothing to expire and nothing to revoke; the only way to invalidate it is to abandon your identity key, which is also your account and your Solana wallet address. Unfollowing tells the user "Unfollowed" but leaves the pathway wide open. There is no recipient-side block at the relay either, so a person being harassed by someone they once friended has no mechanism at
  - Fix: Add a revocation mechanism (certificate serial plus a recipient-published revocation object the relay consults at dm_put, or an expiry field in the preimage with renewal) and a recipient-side deny list enforced in handle_dm_put; until one exists, correct lines 26-27 and 76 to state what the software actually offers.

### `cosmos-architecture.md`

- **[low]** cosmos-architecture.md gives two different paths for the star catalog, one of which does not exist
  - Where: line 959 vs the "What's still missing: Phase 4b" section
  - Why: Two spellings of the same file in one document, one of them wrong, is the kind of ambiguity that produces a second copy of a 119k-row dataset when someone creates the missing path rather than finding the existing one.
  - Fix: Change `data/cosmos/stars.csv` to `data/stars.csv` in docs/design/cosmos-architecture.md. Rerun `node scripts/build-library.js`.

### `data/library/failure_of_legitimacy.md`

- **[high]** Both documents that authorize resistance and irreversible force cite "Use of Force Constraints" as their governing limit, and that document is not shipped in the app or on the website
  - Where: line 113 ("Relationship to Other Documents"); identical dangling reference at data/library/irreversible_actions.md:71
  - Why: failure_of_legitimacy.md:96-104 tells the reader that "Resistance to illegitimate authority is justified when peaceful correction is unavailable and harm is ongoing", and irreversible_actions.md:26-39 sets out when a person may be executed. Both then point the reader at Use of Force Constraints for the limits — proportionality, necessity, discrimination between aggressors and non-aggressors, the prohibition on force to suppress dissent. That document is unreachable from either surface: both the native Library and web/pages/library-app.js read the same data/library/index.json manifest (web/page
  - Fix: Add docs/reference/use_of_force_constraints.md to data/library/catalog.json (the "How We Decide" or "Safety and Care" category) and run node scripts/build-library.js to resync data/library/ and regenerate index.json. Until then, the two references should not present it as an available governing document.
  - Source: https://united-humanity.us/docs/reference/use_of_force_constraints.md

### `data/library/for_public_leaders.md`

- **[high]** Public-leaders brief tells governments every chat message carries its sender's signature; the relay verifies the signature then throws it away
  - Where: lines 96-102 ("The encryption question, answered plainly")
  - Why: The Dilithium3 signature is verified in transit (src/relay/relay.rs:4997-5038, with absent-or-invalid rejected for non-bot senders), so live spoofing is genuinely blocked. But the verified signature is then discarded: src/relay/storage/channels.rs:262-277 writes that None into the messages.signature column, and the broadcast copy carries None too, so no client ever receives a signature to check. Stored chat history therefore cannot be independently re-verified by anyone, and the server operator could edit a stored message without leaving a trace, which is precisely the property the very next s
  - Fix: Say that chat messages are post-quantum signed by the sender and verified on arrival, so a message cannot be forged in flight, but the signature is not yet stored alongside the message, so stored chat history cannot be independently re-verified. Put chat in the same "signing those is on the list" sentence as listings and tasks.

- **[high]** Two documents describe the shipped star catalogues as credit-only; they are CC BY-SA 4.0, and LICENSES.md warns in bold against exactly this framing
  - Where: line 79 ("No lock-in, by construction"); same defect at data/library/how_humanityos_helps.md lines 38-40 ("What HumanityOS is")
  - Why: I confirmed the licence at the upstream primary source, and the repo's own canonical records agree: data/credits.ron:77 and :87 both record `licence: "CC BY-SA 4.0"` for HYG and ATHYG. This is not an obscure optional asset: data/stars.csv and data/stars.bin are the default ~120k-star catalogue every player sees, and LICENSES.md:78-82 notes they are copied into every release archive by the wholesale `cp -r data/` in build-desktop.yml, so redistributing a release bundle carries the share-alike obligation forward. Share-alike is a materially different obligation from credit: it constrains the ter
  - Fix: In both files, state both obligations: the star catalogues are CC BY-SA 4.0 and the OpenStreetMap regions are ODbL 1.0, and both ask for credit and that you pass adapted copies on under the same terms. Drop "CC-BY" and "credit rather than money". Since LICENSES.md is the cited authority, re-check these two paragraphs whenever it changes.
  - Source: https://raw.githubusercontent.com/astronexus/HYG-Database/main/LICENSE

### `data/library/humanitys_mission.md`

- **[high]** Mission page says the app "speaks several languages"; it is English only, and the funders brief in the same Library says so
  - Where: line 209 ("Built for every situation" section, "Whatever your language or ability" bullet)
  - Why: The five files in data/i18n/ hold 37 keys each (34 user-visible strings: the nav bar plus a handful of common/chat/settings labels). Nothing consumes them. In web/, the only occurrence of i18n.t( anywhere is the usage example in the header comment of web/shared/i18n.js:9; web/shared/shell.js:187-198 calls only i18n.load(), which sets <html lang>/<dir> and fires an event. On the native side the files are embedded at src/embedded_data.rs:113-117 and read by no GUI code: there is no translation function in src/gui/ at all. So both halves of the product render English regardless of the setting. Tw
  - Fix: Replace with the funders brief's wording, which is accurate: the app is English today; five partial language files exist and no screen reads them yet. Real localisation is an unbuilt item, not a shipped feature.

- **[high]** Mission page offers recovery "from trusted friends who each hold an encrypted piece" of your key; no such recovery exists in any client
  - Where: lines 161-164 ("You can never be locked out") and lines 198-201 ("When you lose your device")
  - Why: No Shamir split or combine exists anywhere in src/ or web/. Grepping the whole tree for "shamir" returns only prose in src/gui/pages/recovery.rs:17,29,32, schema comments in src/relay/storage/mod.rs:120,1934,1952, and that module header. Both recovery pages are read-only lookups with no way to create shares: the native page (src/gui/pages/recovery.rs, 106 lines total) has two buttons, "Look up" and "Show shares"; the web page at web/pages/recovery.html:122 tells the user to "Set one up by posting recovery_share_v1 signed_objects, one per guardian", which no UI or client code can do. The 24-wor
  - Fix: Mark the guardian clause "(in progress)" exactly as the radio-links clause is marked, or drop it and keep only the seed-phrase recovery, which works. Do not restore it until a client can actually split a seed and post the shares.

- **[high]** Offline and on-device claims do not hold for tasks or notes; the public-leaders brief already contradicts the mission page on this
  - Where: lines 188-191 ("When there is no internet"); same defect at data/library/how_humanityos_helps.md line 25 and data/library/for_public_leaders.md lines 49-51
  - Why: Verified in code, for both halves of the claim. Tasks: the native Create button at src/gui/pages/tasks.rs:396-425 only does state.tasks.push(task); nothing is written to disk, and grepping src/ for task_create returns relay-side handlers only, so the native client never sends one. The list is populated from and cleared by the server (src/lib.rs:17076, 17117, 17124 "Received N tasks from server"), so an offline-created task is silently lost on the next refresh or on exit, and never syncs. On the web the board fetches /api/tasks (web/pages/tasks-app.js:77) and creates over the relay socket, so i
  - Fix: Say what is true per surface: the Library and your game save are on-device and work offline; notes are local and encrypted on the web client but are not yet persisted in the desktop app; tasks are server-side on both and need a connection. Propagate for_public_leaders.md's own honest task sentence to the mission page and the helps page, and correct that file's "your notes and your records" clause.

### `development_loop.md`

- **[medium]** development_loop.md tells contributors to run `cd server && cargo check`, but server/ was removed in the v0.90 restructure
  - Where: line 26 (Step 3: Develop)
  - Why: This is the pre-push gate the document presents as mandatory, and the relay half of it cannot run at all. CLAUDE.md records that a missing relay check kept the Deploy-to-VPS workflow red for 25 consecutive releases, so this is the exact command whose absence has already cost the project a silent live-code freeze.
  - Fix: In docs/contributor/development_loop.md replace the three-command line with the real gate: `just verify` (which runs `cargo check --features native`, `cargo check --features relay --no-default-features`, the lib tests and the lints) and note `just verify-runtime` for renderer or shader work. Rerun `node scripts/build-library.js`.

- **[medium]** development_loop.md requires updating CHANGELOG.md, which the changelog itself says is no longer maintained
  - Where: line 43 (Step 5: Update Docs)
  - Why: The instruction directs every development cycle to write into a file that has been formally retired for roughly 1,260 releases, and it omits the records that actually are kept (GitHub Releases, docs/history/, the journal at data/coordination/orchestrator_state.json). A contributor following the step produces a stray entry in a dead file and skips the live record.
  - Fix: In docs/contributor/development_loop.md, replace the CHANGELOG.md bullet with the GitHub release notes plus docs/history/<date>.md and data/coordination/orchestrator_state.json, matching CLAUDE.md's cross-session persistence table. Rerun `node scripts/build-library.js`.

### `distribution-mirrors.md`

- **[low]** distribution-mirrors.md: the /releases/latest symlink does not exist and two third-party pricing rows are stale
  - Where: line 337 (adoption order step 3), line 296 (Pinata), line 312 (web3.storage)
  - Why: The `latest` claim is the actionable one: anyone scripting a download against /releases/latest/ gets a 404 with no hint that version directories are the only path. The two pricing rows are in a planning table rather than an instruction, so they mislead a cost decision rather than break anything.
  - Fix: Either create the symlink on the VPS or drop the sentence and point readers at manifest.json for the newest tag. Refresh the Pinata row to 1 TB at $20/mo, and either retire the web3.storage row or rename it to Storacha/Fil One with its current terms.
  - Source: https://united-humanity.us/releases/

### `fire_staff_materials.md`

- **[high]** Denatured alcohol's flash point is left vague, and the vagueness points the wrong way
  - Where: line 259 (Fuel table, Denatured alcohol row); interacts with line 262
  - Why: The row is placed directly under kerosene (38 C) and lamp oil (66 C), so "Between the above" reads as roughly 38-66 C. The real figure is about 13 C. Three lines later the document defines fuels flashing below 51 C as "the dangerous class" and names only camp fuel as being inside it, so a reader following the table concludes denatured alcohol is in the safer band when by the document's own rule it is not. The row also calls it "the genuinely cool option", which contradicts the document's own thesis two sections earlier that peak flame temperature barely varies, and contradicts its own cited da
  - Fix: Give the number: flash point about 13 C (55 F), citing an SDS. Move the row above kerosene so the table stays monotonic, add it explicitly to the "dangerous class" sentence at line 262, drop "the genuinely cool option" or restate it as low radiant output rather than low temperature, and add the invisible-flame warning.
  - Source: https://www2.atmos.umd.edu/~russ/MSDS/ethanol_denatured.htm

- **[high]** The wick price in the cost table is for the one spec the document tells you not to buy
  - Where: line 336 (What it costs, Wick row, "Proper staff (7075)" column); contradicts lines 162-166
  - Why: $76 is the cheapest of 23 variants: 1/16 inch thick, half an inch wide. Lines 165-166 say "Buy 1/4 inch" and "3 to 5 inches is the normal staff range". The matching roll costs $295 to $650, so the "Proper staff" total of $321-$466 and the build-versus-buy conclusion at lines 344-352 both rest on a price for material the document has just told the reader to reject. Separately, the recommended combination (1/4 inch thick at 3+ inches wide) is not sold by the cited supplier at all, whose 1/4 inch stops at 2.0 inches.
  - Fix: Price the roll the document actually recommends, or drop the roll line and price by the foot ($2.95/ft at 1/4" x 1"), and note that 1/4 inch thickness is only stocked up to 2 inches wide so the thickness and width recommendations cannot both be met from one supplier.
  - Source: https://firemecca.com/products/k1-tape-wick

- **[high]** Camp fuel's flash point is wrong by about 14 C, in the direction that makes it look safer
  - Where: line 256 (Fuel table, Camp fuel row)
  - Why: Line 251 tells the reader "Flash point is the number to read", then the first row gets it wrong for the most volatile and most commonly used fuel in the craft. The error contradicts the manufacturer's SDS and the document's own cited source, and it errs high, i.e. it makes the fuel look 14 C less volatile than it is. The internal conversion (-4 C = 25 F) is self-consistent, so the error is in the value, not the arithmetic, and will not be caught by a reader spot-checking the units.
  - Fix: State flash point below -18 C (below 0 F) per the Coleman SDS, and cite the SDS rather than a fire-arts page.
  - Source: https://zenstoves.net/MSDS/Coleman.htm

- **[medium]** The safety kit presented as complete omits a fire extinguisher and burn first aid
  - Where: lines 266-289 ("The safety kit is part of the build")
  - Why: The section explicitly claims completeness ("A first fire staff without these is not finished") and is the only safety inventory in a document about setting things on fire. A blanket handles a wick or a person; it does not handle a spilled and ignited fuel depot, which is the scenario the document itself creates by telling the reader to keep a depot and a spin-off station. The extinguisher type also matters: water on a naphtha or kerosene fire spreads it, and the cited sources say so.
  - Fix: Add a CO2 or ABC extinguisher (with the explicit "not water" note) and basic burn first aid to the list, and note that the extinguisher belongs at the fuel depot, not only at the performance area.
  - Source: https://www.firetoys.com/blogs/fire/fire-safety-for-fire-spinners

- **[medium]** "Grass, not pavement" is given as a safety item with no dry-ground caveat
  - Where: lines 288-289 (safety kit) and line 160 (wick section)
  - Why: Presented under "The safety kit is part of the build" and called "the highest-return habit here", so it reads as a safety instruction rather than the wick-life tip it is at line 160. Dry grass, leaf litter, scrub and drought conditions are a wildfire ignition source, and the document itself says a dripping staff is the main cause of burns. Fuel dripping onto dry grass is the failure mode this bullet walks the reader into.
  - Fix: Qualify it: green or damp grass, dirt or gravel; never dry grass, leaf litter, scrub, heath or forest floor; check local fire restrictions and permission first. Keep the wick-life rationale at line 160 where it belongs.
  - Source: https://www.firetoys.com/blogs/fire/fire-safety-for-fire-spinners

- **[low]** Fibreglass firesleeve continuous rating is overstated against the cited manufacturer
  - Where: line 191 (heat shielding table)
  - Why: The 1200 C splash figure is exact, but the cited manufacturer gives a single continuous rating of 260 C with no 300 C upper bound. In a heat-shield table the continuous rating is the number a builder sizes the standoff against, and 40 C of unearned margin is the wrong direction for a safety part.
  - Fix: State 260 C continuous, per the cited datasheet, or cite a specific product that is rated to 300 C.
  - Source: https://www.davlyngroup.com/products/firesleeve/

### `first_solar_power.md`

- **[medium]** Solar guide's panel-sizing paragraph cites a 515 Wh daily load the document never computed; its own total is 520 Wh
  - Where: line 109, contradicting line 77 and line 81
  - Why: The two worked numbers the whole guide builds toward are the daily load and the panel that refills it, and the panel-sizing paragraph refers back to a load figure that does not exist. A reader checking the arithmetic (which the guide actively invites, since it shows every multiplication) finds the numbers do not reconcile, which undermines confidence in the sizing method the guide is teaching. The conclusion happens to survive (344 Wh still falls short of 520, and 688 Wh still covers it), so nothing downstream is wrong, but the stated premise is stale, evidently left from an earlier draft of t
  - Fix: Change "the 515 Wh day above" at line 109 to "the 520 Wh day above" to match the total computed at line 77. Consider also naming which of the two days is meant, since Step 2 ends by offering both a 520 Wh untrimmed day and a 376 Wh trimmed day, and a 100-watt panel's 344 Wh nearly covers the trimmed one.
  - Source: C:/Humanity/data/library/first_solar_power.md:77

- **[low]** The DOE kettle example and the nameplate-is-the-maximum rule are not in the Virginia Cooperative Extension page cited for them
  - Where: lines 27-31 and lines 52-63 and line 181, source at line 221
  - Why: Both claims are true and are genuinely DOE's, but neither is findable in the only source the guide lists for them. The VCE page is an older republication of the DOE article that predates the kettle example. A reader or reviewer checking the kettle figure against the cited page concludes the guide invented it, which is the kind of false alarm that costs a sourced document its credibility. The kettle number is load-bearing: it is reused at line 181 as the anchor for why a small station cannot run heating appliances.
  - Fix: Add https://www.energy.gov/energysaver/estimating-appliance-and-home-electronic-energy-use to the Sources list at line 221 and attribute the kettle example and the "maximum power drawn by the appliance" rule to it, keeping the VCE page for the amps x volts method and the appliance wattage table.
  - Source: https://www.energy.gov/energysaver/estimating-appliance-and-home-electronic-energy-use

### `furniture.md`

- **[medium]** furniture.md promises items.csv hot-reloads while the game runs; no gate rebuilds the item registry
  - Where: lines 51-56 ("Seeing it in the game")
  - Why: A modder saves items.csv, waits for their furniture to appear 'within moments', sees nothing, and starts debugging a CSV line that is actually correct. The sibling recipe.md gets this right at its line 44 ("leave the world, and re-enter it, and your changes are picked up"), so the two guides in the same folder disagree about the same mechanism.
  - Fix: Match recipe.md: the registry reloads at startup and on every world entry, so leave the world and re-enter, or restart. Either drop the schemas/item.toml sentence or note that its hot-reload flag is aspirational and nothing consumes it.

### `gameplay-loop-map.md`

- **[high]** gameplay-loop-map.md presents a two-month-stale survey as the "CURRENT-STATE" picture; five of its closure-ladder rungs have shipped
  - Where: lines 50, 118, 146, 215, 234 and the closure ladder (rungs 1, 2, 3, 7, 8)
  - Why: The document opens by declaring itself "the CURRENT-STATE vs DESIGNED-STATE picture of every gameplay loop" and ends with a strict-order build ladder. Rungs 1, 2, 3, 7 and 8 of that ladder are already in the binary. CLAUDE.md's standing rule is "never rebuild what exists"; an implementer or agent handed this map as the current state would rebuild death/respawn, the construction sink, credits and the vendor, creature spawning, and the ability system.
  - Fix: Re-survey against src/lib.rs's registration block (lines 1190-1320) and tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS, then either rewrite docs/design/gameplay-loop-map.md's state sections and strike the completed ladder rungs, or add a dated banner at the top stating the survey is a 2026-07-07 snapshot superseded by the shipped v0.746-v0.761 arc and pointing readers at DEFERRED_SYSTEMS for live truth.

### `getting-started.md`

- **[medium]** Cross-folder relative links are dead in the shipped Library, including the self-hosting hand-off
  - Where: line 93 (also line 40 and 92; same class in README.md lines 33-34, ONBOARDING.md lines 55-131, admin-README.md lines 19-45)
  - Why: These are the navigation spines of the three entry-point documents. A user finishing getting-started.md and clicking through to self-hosting gets a 404; an admin clicking any of the seven ops links in admin-README.md gets a 404; ONBOARDING.md sends readers to ../../CLAUDE.md, a repo file that is not distributed with the app at all. Same-folder links (getting-started.md to ONBOARDING.md, the ten links in user-creating-README.md) are the ones that could work, and only if the renderer maps .md filenames to library slugs.
  - Fix: Two options: teach build-library.js to rewrite known doc paths into the Library's own deep-link grammar (web/pages/library-app.js:33-38 already defines `/library#<slug>` as filename minus .md with underscores to hyphens), or rewrite the links in the source docs to that slug form. Links to files not in the catalog at all (CLAUDE.md, PRIORITIES.md, STATUS.md, PAGES.md) should become plain text or GitHub URLs.

- **[medium]** getting-started.md and ONBOARDING.md tell readers the platform is in daily use; the project's own context file says the opposite
  - Where: line 44 ("When can you use it?"); ONBOARDING.md line 65
  - Why: A new user is told there is an active community, joins a channel expecting people, and finds an empty room, which reads as the software being broken rather than pre-launch. getting-started.md step 4 ("Say hello. Join a channel and send your first message") and step 5 ("step into the simulation and plant your first garden") set the same expectation against a game the operator states is not playable yet.
  - Fix: Say where it actually is: the platform runs and you can use it, development is in the open and moving fast, and day-to-day conversation currently happens on the Discord already linked at ONBOARDING.md line 136. Keep the honest "early and improving fast" framing the same paragraph already has, just drop the daily-use claim.

### `infinite-of-x.md`

- **[medium]** infinite-of-x.md cites four data files and two source files that no longer exist
  - Where: lines 53, 58, 84, 122 (Migrated tables and "Already done (reference patterns)")
  - Why: These rows are the doc's worked examples and its "Already done (reference patterns)" list, explicitly offered with "Study these to understand the target architecture." Four of the paths a reader would open to learn the pattern are dead, which undercuts the section's whole purpose.
  - Fix: In docs/design/infinite-of-x.md repoint the planets row to data/star_systems/sol.json, the tools row to data/external/catalog.json, and strike or annotate the resources and ai_usage rows as retired (v0.415.0 and v0.197.0). Rerun `node scripts/build-library.js`.

### `planet.md`

- **[high]** planet.md tells modders a new solar-system body requires a cargo rebuild; sol.json has been disk-first since v0.1121
  - Where: lines 105-113 ("Seeing it in the game", the "You added a brand new body" bullet)
  - Why: This is the one step in the whole planet guide that would stop a non-programmer cold, and it is wrong. The guide's audience is explicitly people with no compiler ('If you can open a folder and edit a text file, you can do this'), so telling them a new world needs `cargo build` makes them abandon the feature that in fact works from a text editor. Line 116 then compounds it: "If you are not set up to rebuild the game, no problem: editing the existing worlds is the fun part anyway."
  - Fix: Replace the bullet with the real rule: edit `data/star_systems/sol.json` next to the exe, restart, done. Add the one genuine caveat from src/cosmos.rs:116-122, that the disk copy must keep a `catalog_version` equal to or newer than the shipped one or the embedded catalog wins (so keep the shipped version number when hand-editing), and drop the "not set up to rebuild" consolation paragraph.

### `quest.md`

- **[high]** quest.md tells authors Travel objectives never complete, citing a file whose header says the opposite
  - Where: lines 86-88, repeated at line 131 and line 144
  - Why: A quest author reading the shipped guide will avoid a working objective type, and the doc sends them to the exact file that refutes it, which reads as the guide being untrustworthy generally. The troubleshooting section at line 131 also sends anyone whose quest is stuck chasing a non-existent Travel bug instead of their real typo.
  - Fix: Document Travel as working, point at `data/entities/destinations.ron` for valid destination ids, and note the real gotcha from exploration.ron:8-9 (accept the quest before walking there; leaving and returning re-fires). Keep the warning for `Talk` only, which still has no emitter.

### `rights_and_responsibilities.md`

- **[high]** "One person, one voice" is contradicted by the shipped voting code, which weights every vote by a trust score
  - Where: lines 55-58 ("5. Equal Standing") and lines 46-51 ("4. Secure Participation")
  - Why: Influence on a proposal is not equal and is not counted one-per-person. It is the voter's credential count (0.30), how many distinct people vouched for them (0.25), how varied their recent posting has been (0.15) and how long they have been a member (0.10). Because sigmoid(0) returns 0.0, a member who joined today, holds no credentials and has no vouchers contributes a weight of roughly 0.03 (their own vote object is the single activity type) while a long-tenured, well-vouched member approaches the 0.95 cap: an amplification factor of about thirty. That is amplification and suppression by acce
  - Fix: Either change the tally to one-member-one-vote (count rows, not `weight_at_vote`) and keep the trust score for eligibility only, or amend rights_and_responsibilities.md section 5 to state plainly that governance votes are weighted by a published trust score and name the inputs, so the shipped promise matches the shipped behaviour.

- **[high]** "Confidentiality of votes" is false: every vote is a publicly fetchable, voter-attributable object
  - Where: lines 26-32 ("2. Privacy and Confidentiality")
  - Why: Anyone on the internet can call GET /api/v2/objects?object_type=vote_v1&author_fp=<fingerprint> and get back every vote a given identity has cast, with the choice recoverable by base64+CBOR-decoding payload_b64 (governance.rs `read_text(object, "choice")` reads exactly that field). Votes are also permanently linked to the voter in the `votes` table by `voter_did`. Because each vote is a Dilithium-signed object, the voter can also prove to a third party how they voted, which is the vote-selling and coercion vector the project's own docs/reference/voting_integrity_constraints.md lines 51-79 forb
  - Fix: Either remove "votes" from the confidentiality list in section 2 and state that governance votes are public signed objects by design (with the coercion trade-off named), or change the implementation to separate eligibility from ballot content before claiming ballot confidentiality.

### `saving_your_own_seeds.md`

- **[high]** Peppers and eggplant are attributed to a University of Maine "self-pollinated list" that classifies them as insect-cross-pollinated
  - Where: lines 53-57 ("Rule 2: start with plants that pollinate themselves")
  - Why: Maine says the exact opposite of what the document attributes to it: Pepper and Eggplant are the two entries in Maine's insect-cross-pollinated column, not its self-pollinated column. The document therefore tells a beginner that peppers and eggplant can be saved without isolation, on the authority of a source that says they need it. Sweet-pepper x hot-pepper crossing by bees is the classic home seed-saving failure, and the surprise is capsaicin in what the grower believes is a bell pepper. The error also makes the document contradict itself: its own "Insect-pollinated (harder)" list two lines 
  - Fix: Drop the false Maine attribution. Either (a) list peppers under Minnesota's authority only and state the conflict plainly, e.g. "Minnesota lists peppers as self-pollinating and a good beginner crop; Maine's table puts peppers and eggplant among the insect-pollinated crops, so grow only one pepper variety in a season you intend to save from", or (b) move peppers and eggplant to the "Insect-pollinated (harder)" list. Eggplant has no supporting source at all in the cited set (Minnesota names only tomatoes, peppers, beans and peas) and should be removed from the easy list either way.
  - Source: https://extension.umaine.edu/publications/2750e/

- **[low]** Threshing method ("a sack ... crushing or treading") is credited to Maine, which describes a different method
  - Where: lines 140-144 ("Dry seeds: beans, peas, lettuce", step 4)
  - Why: The technique itself is sound and widely used, so nothing goes wrong for a reader who follows it. But the document's credibility model is that every claim traces to the named extension source, and a reader who opens the Maine link to check this one will not find it. The winnowing half of the same sentence (pouring between containers in front of a fan onto a tarp) IS Maine's and is quoted accurately, which makes the invented first half harder to spot.
  - Fix: Replace the threshing clause with Maine's actual wording (break the pods open, or rub seed heads between gloved palms), or keep the sack-and-tread method but attribute it generally rather than to Maine.
  - Source: https://extension.umaine.edu/publications/2750e/

### `spaceship.md`

- **[high]** spaceship.md promises a visible result from a file nothing renders
  - Where: line 7 and line 69
  - Why: The guide's audience is a first-time modder. They will copy the file, rename a room, restart, walk around, see nothing, and spend the afternoon hunting a RON typo that does not exist. What they actually edited is the room model the relay reports over the `game_perceive` JSON protocol (the surface documented in ai-onboarding.md), not the 3D ship a desktop player walks through.
  - Fix: State up front what the file drives: the relay's world model that `game_perceive` / `game_welcome` report, which AI agents and the shared-world protocol read. Tell modders how to actually observe a change (the perception JSON), and point anyone who wants to change the 3D interior at the construction editor and `data/blueprints/ship_structure.ron` instead of quietly warning them away from it at line 57.

### `torrent-infrastructure.md`

- **[high]** The BitTorrent distribution layer is documented as live since v0.129.0; the live manifest has no magnets and no torrents
  - Where: line 3 ("Status: live since v0.129.0"), lines 6-8, line 105; mirrored in distribution-mirrors.md lines 342-346
  - Why: The doc presents three fetch methods 'in order of bandwidth-friendliness' and the first two do not exist. A reader who wants the censorship-resistant path, which is the whole stated point of the sovereignty layer, will paste a magnet that is not there and download a .torrent that 404s. The equally confident 'shipped v0.129.0' checkmark in distribution-mirrors.md's adoption order compounds it.
  - Fix: Demote the status to the truth (the seeder and manifest wiring existed at v0.129.0 but the current manifest carries no magnets and no .torrent siblings are published), or restore the seeding half of regen-releases-manifest on the VPS. Either way the two documents must agree, and distribution-mirrors.md's step 4 checkmark needs the same correction.
  - Source: https://united-humanity.us/releases/manifest.json

- **[high]** The per-file Forgejo CDN that every shipped data-manifest points at fails TLS hostname verification
  - Where: lines 127-139 ("Layered architecture", the per-file manifest section)
  - Why: The delta-sync channel this section describes cannot work for any client that validates TLS, which is all of them. 575 URLs per release ship pointing at a host that no browser or HTTP library will connect to, so the file-level update path is dead on arrival and anyone investigating gets a certificate error rather than an informative failure.
  - Fix: This is most likely a server fix rather than a doc fix: add git.united-humanity.us to the Let's Encrypt certificate's SAN list (the apex and chat subdomain are already there) and reload nginx. If the Forgejo mirror is not meant to be the per-file CDN any more, change scripts/gen-data-manifest.js's base URL and correct this section.

### `two-realities.md`

- **[high]** two-realities.md contradicts its own core rule: it forbids the Real/Sim toggle, then closes by describing one
  - Where: line 92 (closing paragraph) vs lines 16 and 87
  - Why: The document's own header says "Read this before designing ANY page, widget, or data model," and it is cited as a foundational design axiom. Its final sentence reinstates the exact mechanism the document was written to kill, so a designer who reads to the end gets the opposite instruction from a designer who stops at the rule. The leftover sentence is also the likely source of the same error in 01-VISION.md.
  - Fix: In docs/design/two-realities.md, delete the trailing clause "and the toggle inside each shared tool flips the dataset" from the closing paragraph so it reads consistently with lines 16 and 87. Rerun `node scripts/build-library.js`.

### `ui-system.md`

- **[high]** ui-system.md's "Migration status (live)" marks four shipped items as unshipped, and contradicts its own text on one of them
  - Where: lines 267, 268, 270, 272
  - Why: Four of the eleven checkboxes in the doc's live status board are wrong, and the help-modal row is contradicted by line 246 of the same file, so the board cannot be trusted at all. Line 268 in particular would send someone on a colour migration that was completed, against a hex value that is no longer anywhere in the theme.
  - Fix: In docs/design/ui-system.md tick lines 267, 268 and 270 with the versions they landed in, delete line 272 and note that the standalone native onboarding page was folded into Tasks/Quests in v0.415.0 (onboarding::draw_quests is called from src/gui/pages/tasks.rs:637), and bump the "Last updated" line. Rerun `node scripts/build-library.js`.

- **[high]** ui-system.md's page-parity inventory lists three pages that exist in neither UI and two that are no longer native pages
  - Where: lines 297-319 ("Page parity (web <-> native)", "Inventory at v0.124.0")
  - Why: The section ends with a "Maintenance rule" declaring that empty boxes in this audit are a dual-UI bug, so the table is presented as an enforcement artifact. Eight of its rows describe pages that were deleted between v0.197.0 and v0.1147, which means anyone auditing parity against it will chase pages that do not exist while the real registry (docs/PAGES.md) and its two lints go unconsulted.
  - Fix: Delete the duplicated inventory from docs/design/ui-system.md and replace it with a pointer to docs/PAGES.md plus tests/page_registry_lint.rs and tests/page_parity_lint.rs, which are the enforced sources. Rerun `node scripts/build-library.js`.

- **[medium]** ui-system.md's canonical token tables do not match data/gui/theme.ron, the file they claim to quote
  - Where: lines 25-108 (Canonical color palette, Spacing and sizing, Typography)
  - Why: Most rows in all three tables are wrong by a wide margin (icon_size is off by more than 2x, font_size_title by nearly 50 percent). The doc does carry an escape hatch saying the RON wins, but it is presented as the canonical reference a web implementer matches against, and a web page built to these numbers would visibly diverge from native, which is the exact drift the document exists to prevent.
  - Fix: Either regenerate the three tables in docs/design/ui-system.md from data/gui/theme.ron, or replace them with a short pointer to the RON plus the theme editor, since data/gui/theme.ron is live-edited from the Settings page and any transcription will drift again. Rerun `node scripts/build-library.js`.

- **[medium]** ui-system.md component registry names a modal API that does not exist
  - Where: line 157 (Component registry, "Modal" row)
  - Why: The registry is the lookup table a contributor uses to find the existing widget before writing a new one, and this row points at a module path that will not compile. Every other row in the table I checked resolves to a real function, so the one wrong entry is easy to trust.
  - Fix: Change the native cell in docs/design/ui-system.md to `widgets::dialog::dialog` / `dialog_anchored`, and rerun `node scripts/build-library.js`.

- **[low]** ui-system.md's help-topic notes name the wrong caller and an outdated topic count
  - Where: line 246 ("How to add a new help topic")
  - Why: Someone told to copy the one existing call site would open chat.rs and find a private fork of the widget rather than the shared helper, which is how a second modal implementation gets propagated.
  - Fix: In docs/design/ui-system.md name keymap.rs and studio.rs as the call sites, update the count to 13, and note that chat.rs still carries a private draw_help_modal worth folding into the shared widget. Rerun `node scripts/build-library.js`.

## Contested findings, unadjudicated

One verifier refuted each of these. Do not act on one without re-checking the primary source yourself; the refutations are often substantive.

### `00-START-HERE.md`

- **[medium]** 00-START-HERE ships in the in-app Library but routes the reader to files the app does not distribute
  - Where: lines 20, 29-36, 41-44 ("Read next", "Fast orientation", "Where to work first")
  - Why: This document is a pure router: essentially every instruction in it is "go read that other file," and the single most important target (CLAUDE.md, named as "the real source of truth") is not shipped with the binary. An app reader who follows the onboarding path reaches a dead end at step 1.
  - Fix: Either add the repo URL alongside each pointer in docs/contributor/00-START-HERE.md (github.com/Shaostoul/Humanity), or exclude the contributor-router docs from data/library/catalog.json so the Library only ships documents that stand alone. Rerun `node scripts/build-library.js` after either change.

### `02-ARCHITECTURE.md`

- **[low]** 02-ARCHITECTURE undercounts the relay storage modules
  - Where: line 35 (module table, src/relay/ row)
  - Why: A wrong module count is a small thing on its own, but this table is the orientation page's one concrete inventory, and the same stale figure is repeated in CLAUDE.md, so fixing only one of them will leave the two disagreeing.
  - Fix: Change 30 to 50 in docs/contributor/02-ARCHITECTURE.md and in CLAUDE.md's architecture tree in the same commit. Rerun `node scripts/build-library.js`.

### `03-MODULE-MAP.md`

- **[low]** 03-MODULE-MAP cites three systems as single files that are now directories
  - Where: lines 19, 43, 47
  - Why: Minor, but this file is specifically the map of where each domain lives, so a reader opening the cited path finds nothing. The data path in the same line (data/skills/skills.csv, 20 rows) is correct, as are data/materials.csv and data/creative_arts.ron elsewhere in the file.
  - Fix: In docs/contributor/03-MODULE-MAP.md change the three citations to `src/systems/skills/`, `src/systems/crafting/` and `src/systems/farming/`. Rerun `node scripts/build-library.js`.

### `06-SOURCE-OF-TRUTH-MAP.md`

- **[low]** 06-SOURCE-OF-TRUTH-MAP names a crypto source file and a web directory that do not exist
  - Where: line 69 (section B) and line 109 (section E)
  - Why: Both are lookup pointers in the map's design-to-implementation table, so each sends a reader to a path that returns nothing. The web/activities/ row is also the only implementation evidence offered for section E (Game/immersive integration), leaving that row with no valid citation.
  - Fix: In docs/contributor/06-SOURCE-OF-TRUTH-MAP.md change `hashing` to `hash` and replace the web/activities/ citation with the real surfaces (src/gui/pages/ plus src/terrain/, src/renderer/). Rerun `node scripts/build-library.js`.

### `cosmos-architecture.md`

- **[medium]** cosmos-architecture.md contradicts itself on the single locked decision about sim-time speed
  - Where: line 229 (section 6) vs line 1384 (section 17a) vs line 581 (section 15) and line 1672 (section 18)
  - Why: A reader arriving at section 6 (the Time section, where anyone would look first) gets region-based 100x-10000x time acceleration, which the locked decision explicitly forbids, and sections 15 and 18 tell them the question is still open. Three of the four places that discuss time disagree with the one that is marked authoritative.
  - Fix: In docs/design/cosmos-architecture.md, rewrite section 6's time-speed paragraph to the locked 1x rule with FTL as the fast-travel mechanism, strike open question 1 in section 15, and tick the section 18 checklist box with a pointer to section 17a. Rerun `node scripts/build-library.js`.

- **[low]** cosmos-architecture.md's position-composition pseudo-code uses the pre-rename variant and omits one variant
  - Where: line 126 (section 3, world_position pseudo-code)
  - Why: The pseudo-code is the doc's only worked example of the core algorithm, and it contradicts the enum definition given four paragraphs earlier and the enum that actually shipped.
  - Fix: In docs/design/cosmos-architecture.md rename the arm to `ContainerRef::Vessel(id)` and add a `Pocket` arm so the example covers the finalized variant set. Rerun `node scripts/build-library.js`.

### `data/library/humanitys_mission.md`

- **[high]** Mission page promises no one can erase what you said or forbid you from saying it; the project's own published rules page documents exactly those powers
  - Where: lines 141-147 ("What it protects" section, "Free speech" bullet)
  - Why: The moderation powers are real and shipped: src/relay/storage/channels.rs:578 ban_user, :666 mute_user, :839 delete_message, and src/relay/storage/pins.rs:216 delete_message_by_id(msg_id, requester_key, is_admin). The sentence is true of direct peer-to-peer conversation and of encrypted DMs, but false of public chat on any server including the official one, where an admin can delete a message for everyone (not appealable), wipe a channel's history (not appealable), and mute or ban a key. This is the project's headline promise and the first item in its own list of what it protects, so a journal
  - Fix: Scope the sentence to what actually holds: no one can reach into your private messages, your keys, or your device, and two connected peers can always speak with no server in the middle. Public channels on any server are moderated by whoever runs that server; point to /rules, which already states the powers and the appeal routes honestly.

### `fire_staff_materials.md`

- **[medium]** Titanium both does and does not melt below the flame
  - Where: line 68 and lines 73-75 versus line 118
  - Why: These are flat contradictions fifty lines apart, and the first one is the document's stated organising principle ("The one fact that explains every other choice"). A reader comparing the shaft table against the failure table cannot tell which claim to act on. The 1730-1930 C figure is correct and well sourced, so line 118 is the wrong half. The same sentence also overreaches in the other direction: at 1668 C titanium is below the stated 1730-1930 C band, not "inside" it, and epoxy at 120 C is nowhere near it.
  - Fix: Delete "melts above the flame" from line 118, and reword line 73 as "every row above sits below the peak of that range" rather than "falls inside that range".

- **[medium]** "Never for blowing" is attached to only one fuel, implying the others are acceptable for fire breathing
  - Where: line 256 (end of the Camp fuel row)
  - Why: Fire breathing is never mentioned anywhere else in the document, so this three-word prohibition lands as a per-fuel property in a comparison table. The reader's natural inference is that lamp oil and kerosene are the fuels you blow. Aspirating any liquid hydrocarbon causes chemical pneumonitis; the hazard is not specific to naphtha's volatility, and the mainstream safety guidance is to not do it at all.
  - Fix: Either remove the phrase, since the document is about staffs and does not otherwise cover fire breathing, or replace it with a standalone line saying fire breathing is out of scope and is dangerous with every fuel on the list.
  - Source: https://www.firetoys.com/blogs/fire/fire-safety-for-fire-spinners

- **[low]** The document contradicts itself on available wick width
  - Where: line 163 versus lines 166, 173 and 298
  - Why: Three separate places recommend a width the same document says two paragraphs earlier is not sold, and the recommendation is repeated in the sizing table a builder would work from when ordering.
  - Fix: Change "3 to 5 inches" to "3 to 4 inches" in all three places, or note that 5 inch is custom order only.
  - Source: https://firemecca.com/products/k1-tape-wick

### `first_solar_power.md`

- **[low]** The 10 to 15 percent AC conversion loss, which drives the station-sizing recommendation, has no source
  - Where: lines 84-87
  - Why: This number is not decorative; it is the reason the guide tells the reader to buy a station larger than their computed watt-hour budget, and it is the difference between a 500 Wh station covering the 376 Wh day or not. It is also the one figure in the document that is plausible but unattributed, in a guide that explicitly promises every number is attributed. The figure is in the right range for inverter efficiency, so the advice is sound; the problem is that a reader cannot check it and the document's own stated standard is not met.
  - Fix: Either attribute the figure to a qualifying source (a DOE or university extension statement on inverter/AC conversion efficiency), or reword it as the guide's own engineering rule of thumb rather than a sourced number, so it does not sit unmarked under the line 17 promise.
  - Source: C:/Humanity/data/library/first_solar_power.md:17

### `gameplay-loop-map.md`

- **[medium]** gameplay-loop-map.md's lib.rs line citations are off by roughly four thousand lines
  - Where: lines 39, 70, 72, 112 (citations lib.rs:5191, 5027, 5028, 5035, 5169, 5241-5275, 6797-6841)
  - Why: Every lib.rs anchor in the document lands in unrelated code, so a reader verifying any claim in the survey is sent to the wrong place and may conclude the claim is false when the underlying statement about the system is still accurate (for example the hydrology Mutex<Weather> bug at line 291 is real, just at src/systems/hydrology.rs:331 rather than :323).
  - Fix: Either refresh the line anchors in docs/design/gameplay-loop-map.md or, better, replace them with stable references (system name plus file, e.g. "FoodSystem, src/systems/food.rs, registered in src/lib.rs's SystemRunner block") so they survive lib.rs churn. Rerun `node scripts/build-library.js`.

### `growing_food_you_can_live_on.md`

- **[low]** "Four to nineteen times as much energy per pound" contradicts the document's own USDA calorie table
  - Where: line 244 ("The honest counterweight"); the table it contradicts is at lines 36-45 ("Calorie density")
  - Why: "This guide" is explicitly the three crops potatoes, dry beans AND winter squash. The lower bound of the stated range corresponds to potatoes, not to the weakest of the three: winter squash is 2.5 times the tomato, not four times. The sentence is the document's closing argument for the whole guide, and it overstates the payoff of the one crop the document already flags as "generous" because you do not eat the skin or the seed cavity. A reader who checks the arithmetic the guide invites them to check ("The multiplication is ours and we show it, so you can check it") finds it does not come out.
  - Fix: Change to "two and a half to nineteen times", or scope the claim to the crops it actually fits, e.g. "food that holds two and a half to nineteen times as much energy per pound, and four times or better for potatoes and beans."
  - Source: https://fdc.nal.usda.gov/

### `humanity_accord.md`

- **[high]** Accord Art 19(4) lets an emergency panel suspend any article, including the ones the Accord and the Absolute Prohibitions declare unsuspendable
  - Where: line 156 (Article 19, clause 4), against line 141 (Article 17), line 32 (Article 1), and C:/Humanity/data/library/absolute_prohibitions.md lines 178-189
  - Why: Clause 19(4) places no article off limits. A grep of the whole document for "notwithstanding", "non-derogable", "shall not be suspended", "may not be suspended" returns nothing, so Article 1 (dignity), Article 17 (lethal autonomy, which says in its own text that no loopholes are permitted) and Article 18 (mass surveillance) are all suspendable for 30 days by a small panel. Nothing bars a fresh 30-day suspension the moment the previous one lapses, so a rolling suspension is unbounded. The panel itself is never constituted anywhere in the document: no size, no selection method, no rotation rule,
  - Fix: Add a non-derogable clause to Article 19(4) naming the articles that may never be suspended (at minimum 1, 4, 14, 17, 18), bar consecutive or repeated suspension of the same article within a stated window, and constitute the panel (size, selection, rotation) either in Article 19 or by reference to Article 21's draw-by-lot mechanism.

- **[medium]** Article 19's ratification procedure cannot be carried out as written: no depositary, no registry, no canonical text to sign
  - Where: lines 153 and 155 (Article 19, clauses 1 and 3)
  - Why: This is the one article a State must be able to execute, and three of its terms are undefined anywhere in the document. "Public deposit" names no depositary and no location. "Public registry" is named once and never constituted: no operator, no address, no rule for who maintains it. "Three-fourths of all ratifying States" needs an authoritative roster, and nothing says the registry is that roster or how a claimed ratification is verified. And "a cryptographic signature of the exact text" specifies no algorithm and no canonical encoding, while the shipped text itself contains typographic forms 
  - Fix: Name the depositary and the registry (or state that the registry is the repository at a given URL / content-addressed location), define the roster of ratifying States as the registry's contents, and pin the canonical form: a named signature algorithm over a stated normalization (for example the SHA-256 or BLAKE3 hash of the UTF-8 text after a specified newline and quote normalization), with the hash of the current version published alongside it.

- **[low]** Attribution line begins with a stray comma, reading as a truncated signature
  - Where: line 320
  - Why: The line that carries authorship on a document offered for public ratification opens with a dangling comma, which reads as a dropped leading word or name. It is the same class of defect the separately-audited us-constitution.md had in its signature block, and it is the last line a reader sees.
  - Fix: Remove the stray comma or restore the missing lead-in (for example "Authored by Michael Boisson (Shaostoul)"), in docs/accord/humanity_accord.md, then re-run node scripts/build-library.js to resync data/library/.

### `infinite-of-x.md`

- **[low]** infinite-of-x.md's two content counts are stale, one by 22 percent
  - Where: lines 121 and 127 ("Already done (reference patterns)")
  - Why: Small, but CLAUDE.md already carries the correct 483 and 442 figures, so the Library and the operating doc disagree on the size of the same datasets.
  - Fix: Update both counts in docs/design/infinite-of-x.md (chemistry 483, glossary 460) and reconcile the glossary figure with CLAUDE.md, which says 442. Rerun `node scripts/build-library.js`.

### `keeping_what_you_grew.md`

- **[low]** "Tomatoes are no longer treated as acid foods" contradicts the NCHFP low-acid list quoted earlier in the same document, and the water-bath recipe that follows it
  - Where: line 231 ("Tomatoes and the acid problem"); contradicts line 37 and the crushed-tomato boiling-water process at lines 251-268
  - Why: The document quotes NCHFP's low-acid list at line 37, which excludes most tomatoes from the low-acid category, then states at line 231 that tomatoes are no longer treated as acid foods. Taken with the document's own headline rule ("Low-acid foods CANNOT be safely canned in a boiling water bath. They require a pressure canner"), that reading would forbid the tested crushed-tomato boiling-water process the document prints twenty lines later. The practical instruction is correct and errs safe, so nothing unsafe follows, but a careful reader is left unsure whether the recipe they were just given i
  - Fix: Restate as NCHFP does: tomatoes are borderline acid foods, and because variety, over-maturity and dead or frost-killed vines can push individual fruit above pH 4.6, USDA requires added acid to guarantee the acid-food margin before boiling-water processing. Drop "no longer treated as acid foods."
  - Source: https://nchfp.uga.edu/how/can/general-canning-information/ensuring-safe-canned-foods/

### `making_water_safe_to_drink.md`

- **[high]** Microfilter is credited with removing bacteria, and the one CDC bullet dropped from the quoted filter-buying guidance is the bacteria bullet
  - Where: lines 109-110 (filter ladder), lines 126-129 (quoted CDC emergency guidance), line 198 and lines 209-210 (summary table and its definition); contradicts lines 99-100
  - Why: 1 micron absolute is the only filter-buying number the document ever gives the reader, and the summary table then tells that same reader bacteria are handled. Someone who buys exactly what the guide specifies gets a filter CDC says does not remove bacteria, while believing it does. The guide even describes such a filter as "Good for a clean wilderness stream" with no chemical step, which is the scenario where the error is unmitigated. It also contradicts the document's own CDC size table at lines 99-100, which correctly says bacteria need "a pore size of 0.2 to 0.4 or smaller." The CDC bullet 
  - Fix: In the quoted CDC emergency guidance at lines 126-129, restore the dropped bullet: a portable filter "must have an absolute pore (hole) size of 0.3 micron or smaller to remove bacteria." Then correct the ladder at line 109-110 and the table at line 198 so bacteria removal is tied to roughly 0.3 micron absolute or smaller, not to "under 1 micron" generally, and change the parenthetical definition at lines 209-210 accordingly. CDC's plain sentence is worth adding verbatim: "Most portable water filters will remove parasites, but not viruses or bacteria."
  - Source: https://www.cdc.gov/water-emergency/about/index.html

### `planet.md`

- **[low]** planet.md says sol.json lists 69 bodies and furniture.md says decorations.ron is empty; both counts are stale
  - Where: planet.md line 25; furniture.md line 45
  - Why: Cosmetic drift in otherwise accurate guides. Worth correcting because the numbers are exactly the kind of detail a reader spot-checks against the file to decide whether the guide is current.
  - Fix: Say "70 bodies" or drop the count in favour of "every body in the solar system", which does not go stale. In furniture.md, change "currently empty" to note it holds a couple of crop-scatter rows and is still not the furniture path.

### `progression-skills-gear.md`

- **[high]** progression-skills-gear.md says abilities are absent from the engine and proposes files that already exist
  - Where: lines 44-46 ("Current state"), line 87 and line 90 (Part 2), line 127 (Part 3)
  - Why: Part 2 and Part 3 are written as implementation contracts ("Schemas are concrete so an implementing session writes loaders against them"). An implementer following them would write a second ability loader over a file path that no longer exists, and create data/equipment.csv on top of the one already shipped. The stated row counts are also all wrong.
  - Fix: Update docs/design/progression-skills-gear.md's "Current state" to record AbilitySystem as shipped in v0.753, change every `data/spells.csv` reference to `data/abilities.csv`, mark the rename and data/equipment.csv as done rather than proposed, and refresh the row counts (abilities 152, enchantments 133, equipment 32). Rerun `node scripts/build-library.js`.

### `validate_data.md`

- **[medium]** validate_data.md points at a schemas directory that does not exist
  - Where: line 7 (Inputs)
  - Why: The document declares itself "a mandatory gate that must pass before any simulation, replay, or merge," and its only two inputs are the data tree and the schema tree. One of the two paths is wrong, so anyone implementing or re-running the gate looks in an empty location. The document also never mentions the gate that actually exists, `just validate-data` (C:/Humanity/Justfile:369-370).
  - Fix: In docs/contributor/validate_data.md change the schema path to `schemas/` and add a line pointing at `just validate-data` as the current runnable check. Rerun `node scripts/build-library.js`.

## What the sweep itself missed

From a completeness critic run after the sweep, asked what no batch covered.

- **[high]** Every confirmed finding names a generated file; fixing where they point will be undone by the next build
  - Before triage, rewrite every finding's path to its source. The mapping is mechanical: for each catalog.json row, docs-source is d.src and the shipped name is path.basename(d.src), except where a basename collides, in which case it is d.src with the leading docs/ stripped and slashes turned into hyphens (that rule produces admin-README.md, user-creating-README.md and ai-onboarding.md). Concretely: data/library/fire_staff_materials.md is docs/mission/... per its catalog row, data/library/SELF-HOST

- **[high]** The shipped Library at HEAD was not what docs/ says, and the drifted document is the one carrying the critical
  - Commit the pending rebuild now so main matches. Then add the check to CI, because it is four lines: for each catalog.json row, compare the source bytes to the shipped copy and fail non-zero on any mismatch. Prove the gate works before trusting it by editing one character in a docs/ source and confirming the check goes red. The same script run in --write mode is just build-library.js, so the gate and the builder share one code path.

- **[high]** No batch checked how the Library renders: native shows raw link syntax for all 108 links, web shows anchors
  - Decide the contract, then make both ends meet it. Minimum honest fix: teach strip_md to reduce a markdown inline link to text, so native readers stop seeing punctuation, which is one replace and costs nothing. Proper fix: render intra-Library links as in-app navigation (the flat shipped name maps one-to-one to an index.json entry, so the jump target is already addressable) and external http links via ui.hyperlink_to. To verify, run just snapshot library and read the PNG: the link text must appear withou

- **[high]** 36 relative links are dead in the shipped Library and every one of them works in the repo; the sweep found one and never generalized
  - Fix it in the builder, not 36 times by hand. In build-library.js, after copying, rewrite each markdown link target: if the basename resolves to a shipped flat name, replace the target with that name; if it does not, the link cannot survive and should become plain text or an absolute https://united-humanity.us/... URL. Then fail the build on any remaining relative link with a slash in it. To confirm the gate bites, add a deliberate ../foo.md link to a docs/ source and check the build refuses. The

- **[high]** The funders brief tells funders the project is CC0 and uncapturable; the project's own licence file says that is not true of the tree it ships
  - Edit docs/ source for for_funders_and_sponsors.md to say what is actually true and still fully answers the funder's question: everything the project's contributors wrote is CC0, and the third-party real-world data it redistributes carries its own share-alike terms which travel with it. The uncapturability argument survives intact, because share-alike is if anything stronger against capture than CC0. Check the fix by grepping the four sibling briefs (for_families, for_individuals, for_affiliate_p

- **[medium]** ONBOARDING.md tells new users to press the Real/Sim switch, deleted in v0.197.0
  - Delete the sentence in docs/user/ONBOARDING.md and rebuild. Then close the class rather than the instance: grep -ril 'real/sim' across data/library/ returns 01-VISION.md, ONBOARDING.md, gameplay-loop-map.md, progression-skills-gear.md, two-realities.md, index.json and tags.json. Two of those seven were fixed by the sweep; check all seven. Confirm the feature is really gone with git show 3bb5e1f8 --stat before rewriting anything, since two-realities.md is a design doc whose subject is the concept

- **[medium]** One false claim, three different severities, and four more surfaces the sweep's scope could not see
  - Treat the claim, not the document, as the unit. Grep all six surfaces for 'social recovery', 'trusted friend', 'guardian' and 'Shamir' and settle on one truthful sentence, then apply it everywhere in one commit, assigning the whole set the highest severity any instance earned. Regenerate data/roadmap.json with node scripts/roadmap-to-json.js after editing ROADMAP.md, or the website keeps the old text. For the two recovery pages, the honest version is that the server-side share index exists and t

- **[medium]** 35 of 80 documents appear in no finding at all, and three of the four I probed carry defects of confirmed classes
  - Pull the eight batch manifests and diff them against the 80-document list; any document not on a manifest was never audited and needs a pass. For documents that were on a manifest and returned nothing, spot-check three at random against the classes already confirmed, because a batch that found nothing in ten documents is more likely to have skimmed than to have found ten clean documents in a corpus where 45 others yielded 77 defects. Prioritise the four safety-critical silent ones (storing_water

- **[low]** The Library's own taxonomy files cite the deleted Real/Sim toggle as their organising principle, and no batch covered any JSON
  - Rewrite the note in data/library/tags.json to describe the axis on its own terms (real-life material versus in-simulation material) without invoking a control that no longer exists, then re-run node scripts/build-library.js so index.json picks it up. While in there, decide whether the note field should render at all: it is written as user-facing help text, both clients silently drop it, and either wiring it up or deleting it is better than a third state. Add the three JSON files to the audit sco

- **[low]** Confirmed finding on validate_data.md is substantively right but its wording will mislead the fixer
  - Correct the path to schemas/ in the docs/ source. Then check each of the seven 'must fail hard' items against what the cargo test line actually asserts, and either implement the missing checks or restate the section as the specification it is rather than the gate it claims to be. Cheapest verification: introduce an unknown field into one data/ CSV and run just validate-data; if it passes, check 1 does not exist.

