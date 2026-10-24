# HumanityOS: Priorities

> **This is the TACTICAL backlog: what is next, right now.** The TOP item of
> TIER 0 is what gets worked on next. If you are picking up work without
> context, read this file first, then
> `data/coordination/orchestrator_state.json` for WHY we got here.
>
> Its strategic, themed, public-facing companion is
> **[ROADMAP.md](ROADMAP.md)** (the same to-do list grouped by theme with
> status badges, rendered on the website from `data/roadmap.json`). Use
> ROADMAP.md for "where are we going"; use this file for "what is the very
> next thing." Keep the two consistent, and regenerate the JSON
> (`node scripts/roadmap-to-json.js`) in the same commit as any ROADMAP edit.
>
> **Update rule:** every session that meaningfully changes scope updates this
> file before ending. Record WHAT COMES NEXT here and WHY in the journal. When
> an arc finishes, retire it: move the block to `docs/history/` with its
> reasoning intact rather than leaving it here looking pending. A shipped item
> still marked open is the most expensive defect this file can carry, because
> the next session rebuilds it.
>
> **Keep it short.** A backlog nobody finishes is a backlog that does not route
> work. Detail belongs in the design doc for that arc; this file carries the
> decision and the pointer.
>
> If a tool or a doc points you at "Active focus" (`just brief` still does), it
> means **TIER 0** below: that section was renamed on 2026-09-20 when the dated
> status blocks were retired.
>
> Current at **v0.1388.0, 2026-09-27**. Retired blocks live in
> `docs/history/priorities-archive-2026-09-20-to-27.md`,
> `docs/history/priorities-archive-2026-08-to-09.md` and
> `docs/history/priorities-archive-2026-05-to-08.md`.

---

## TIER 0: the next thing to work on

Strict rank. Take the top item that is not marked CLAIMED. Everything below
TIER 0 is real work that has not been ranked against these; do not promote
anything into this list without the operator.

Retired on 2026-09-27, verbatim and under their old headings, to
`docs/history/priorities-archive-2026-09-20-to-27.md`: 1b (the aurora pass,
shipped v0.1382.0), 1c and 3 (the coast glow, BUG-081), 2 (the dark clouds,
fixed v0.1384.0), 2a, 2a-ii and 2a-i (the grain measurements, condensed into
2a below), 2b-0, 2b and 2c (the grain bisect and its refuted list), and 5 (the
page snapshots, done). A code comment or fixture that cites one of those
numbers finds it there.

### NEXT: a headless, GPU-free, faster-than-real-time sim runner (designed 2026-10-22)

`docs/design/headless-sim-runner.md` (the operator: "Could you do 1-billion-x?"): movement 20x
to 800x real time headless, slow processes 10^4x to 10^6x by coarse steps with a measured error,
~10^9x only by the time-away jump; the storeys and falls rigs from minutes with the GPU to seconds
without it. Ten increments (~27-35 agent-days); 1 to 5 move code out of lib.rs and need quiet
windows. Started 2026-10-22: increment 0 (stage timers; 18.7 ms of a 34.6 ms frame is timed by
nothing) and FarmingSystem's cost (3.51 ms of the 3.80 ms systems tick, a windowed-game win too).
**The cost review is DONE and on disk: `docs/design/sim-frame-cost.md`** (timers first, then
FarmingSystem F1+F2 for ~2.5-3 ms, then gating the Garden mirror, est. 4-9 ms every world frame,
to frames that draw the Inventory page). Next agent: a domain-writer for the timers + the farming
golden-state test, then F1/F2/S1 (lib.rs: needs a quiet window).

### QUEUED BY THE OPERATOR 2026-10-22 (updated 2026-10-23)

Done 2026-10-23: the CLAUDE.md split by audience (81 KB to 36 KB, six `docs/ai/` documents,
f894a9612), the saved procedures as skills (`.claude/skills/` release, merge-agent, game-checks,
4f2d6903a), and the Rust pin (1.99.1 for the PC, GitHub and both servers, `rust-toolchain.toml`,
plus the UPDATES row in `just brief`, 4d4bbb153). The PC restarted with a 100 GB page file.

1. **In flight (16 agents):** the 14 resumed from their handoff notes (each in its own worktree's
   `.claude/handoff/`), plus the closed-loop test design and the How-to page design. Merge each
   as it reports (the `merge-agent` skill); review the voice/privacy diff for changed tests.
2. **Crate updates in the next quiet window** (after this batch merges): `cargo update` has 313
   compatible updates and 40 dependencies sit behind a newer major (docs/SOP.md "Keeping
   components up to date"); then the batch release.
3. **A closed-loop test, 1 person to all of humanity** (the operator, verbatim in the journal):
   "as close to 100% closed loop self-sustaining as possible for 1 person up to 100% of Humanity",
   with two metrics: **Earth, today, real** (sourced, dated data) and **the game's mothership**
   housing billions across interstellar space, starting from today's technology and adding
   promising R&D (fusion) as labelled options; "think like 1,000 years from now". The design and
   sourced data are being written (homestead-engineer, with a refutation pass); then the headless
   Rust test on the real systems (the headless runner's first big customer).
4. **A How-to page for step-by-step procedures** (operator, 2026-10-23: "a page dedicated to SOP.
   It's kind of like a crafting page but, could incorporate more stuff. Like parent categories
   could be good and services"). The full design is being written: one procedure model that
   Crafting shares, a category tree, services that do it for you, runs that become Tasks and
   Calendar entries, both realities, and the platform's own SOPs with steps that are admin
   actions a person or an AI can run.
5. **An in-game event journal for play sessions** (after the MCP server lands): events written
   by kind (combat, damage, AI, errors, economy...) to small capped files, a "mark" key that
   bookmarks a moment with a screenshot, and an MCP tool that reads only one kind since a mark,
   so reading 5 minutes of an encounter costs a few hundred lines, not the whole log.
6. **A #claude channel and Claude Code inside the Dev tab (later):** a federated channel everyone
   can read but only the operator's key and the AI's key can post in (the existing
   `channels.read_only` column is the start), and Claude acts only on messages signed by the
   operator's key, so text posted elsewhere can never steer it. Then a terminal pane running
   Claude Code in the Dev tab.
7. **Leftovers the CLAUDE.md split found** (doc-truth, then toolsmith): five docs still call
   CLAUDE.md the canonical crypto table (docs/README.md, docs/admin/SELF-HOSTING.md:873,
   docs/design/platform-architecture.md:241, docs/BUS-FACTOR.md:83, which also says DMs are
   "pure Kyber768"); docs/design/media-round-5-plan.md:603 cites a CLAUDE.md rule that does not
   exist; docs/ai/model-tiering.md treats Fable as a live tier; check the desktop data folder's
   `cache/` against the no-caches rule. Lints proposed: CLAUDE.md under ~40 KB with every
   `docs/ai/*.md` named in its Router; every "CLAUDE.md, <name>" citation resolves; every
   `docs/ai/` file linked from docs/ai/README.md. MEMORY.md could lose ~5 KB more.

### DONE 2026-10-21/22: the five pieces paused at the usage cap all shipped

Maps increment 2 (v0.1539.0), the sky bodies list (v0.1540.0), recovery through trusted people
(v0.1541.0), bounced light's Ambient level and probe streaming (v0.1542.0), the Sun's real body
(v0.1543.0; `docs/history/2026-10-21-in-flight.md` has how they were paused). Left from them:
- **The planets' orbit rings cross the whole sky as faint green lines**, from the ground too (seen
  in the Sun's sunset capture, the operator's far-out screenshot): useful on the map, clutter in
  the sky; decide whether the sky draws them at all, or only near a body, or as a setting.
- The Sun: the aperture (30 mm, dev key `sun_aperture_mm`) has no Settings control; the corona is
  too faint to see until the engine has eye adaptation; within the last tenth of the radius the
  limb law runs darker than measured (under half a pixel); `pipeline.rs` still calls the Sun's
  material "the sun's radial glow". The home's glass roof now reads clear from inside by day (it
  was a grey haze lit from the far side): the operator may want to see it.
- Two soft blue glows on the Maps overview (one by Mars and Earth, one lower right), not chased.

**Also queued (operator's question, 2026-10-21: "Do rooms not automatically calculate their
volumes on the fly?").** They do (the flood fill, live), but the static power meters charge a
controller-driven machine its hand-measured `average_watts` (src/machines.rs `average_load_watts`),
re-measured by a person whenever a room changes; a player's own rooms will never have one. Make the
meters read the simulation's actual duty over the last game day (the data figure only as a
first-day estimate), and stop hand-quoting room volumes in data comments and Library guides
(generate them from the data, or describe the method).

### THIS WEEK (planned 2026-10-02): playable multiplayer, proven with two people

Planned by a five-reader survey, three competing week plans and a judge, with
every load-bearing claim checked by an adversarial verifier
(`docs/history/2026-10-02.md`). Day 1's fixes shipped in v0.1436.0
(BUG-107 to BUG-112). The rest, in order:
- **Review** of v0.1428.0 to v0.1436.0: DONE v0.1437.0 (8 confirmed, fixed as
  BUG-113 to BUG-121 over two rounds, each reviewed). Follow-ups: the crew
  figures in lib.rs have the head-inside-body fault BUG-120 fixed for players;
  settled trade ids live per save (replay into another home); the client's
  game-off refusal match copies the relay's wording (share one function);
  trade escrow needs server-held inventories.
- **Day 2:** DONE v0.1438.0: the first quest's iron step counts graphite
  smelting (a new `Make(item_id, quantity)` objective counts units of an item
  from any recipe), and the homepage game panel plays a real clip (the home
  flyover, 1.2 MB, on screen only, still for reduced motion or data saver).
  The black plant models in the tower net cups: FIXED v0.1443.0 (the crop
  palettes' colour space, not the engine).
- **Day 3:** DONE v0.1440.0. `scripts/second-player.js` signs in as a real
  identity, joins the shared world and walks a path (`just verify-second-player`
  runs it against a throwaway relay); other players move by snapshot
  interpolation on the sender's clock (BUG-125); leaving or being banned
  despawns at once (BUG-126). Proven in the real game in v0.1441.0:
  `just verify-copresence` boots ONE game against a throwaway relay, the
  scripted player crosses in front of the camera, and the game records where
  it drew the figure every frame (`debug/remote_players_request.json`); the
  judge (`scripts/lib/copresence-judge.js`, 23 tests) checks seen, steady
  speed, never backwards, no jump, on the line, in view, on time by the
  computer's clock, and no other figure, plus the figure's colour in two
  screenshots. Follow-ups it found, both fixed v0.1442.0: the hair band
  across the face (BUG-127; its cause, an inside-out sphere mesh, is open for
  8 other callers as BUG-128) and rigs missing the DXC shader-compiler pair
  (all seven rigs now share scripts/lib/dxc-dlls.js); the dev camera
  request turns on fly mode, which takes the player out of the shared world
  (so it hides every other player: use the showcase `cam` request instead);
  stopping, turning and leaving are not judged in the real game yet.
- **Day 4, RE-SCOPED 2026-10-03 by the operator's decisions below:** before
  shared building, homes need their own places on one mothership (plots in a
  ship frame), building limited to your own plot, and shared spaces to meet in.
  The design is docs/design/ship-homes-and-logistics.md (2026-10-03: the full
  list of home kinds as data, Homestead the default start, food physically real
  for every home, 14 increments). Increment 1a SHIPPED v0.1442.0 (the ship and
  the home are separate files, the home assembled at its plot, a Dev Plots and
  Districts panel). Increment 1b MERGED 2026-10-03 (v0.1448.0): the relay
  hands out plots and each player's home stands on its own. Five review
  rounds found and fixed 5, 7, 16, 11 and 6 problems (the worst: a returning
  player whose chat was already connected joined before the ship loaded and
  was refused as "a different ship"; the rigs could not see it until a
  connect-first menu entry was added). Rigs on the merged tree: `--plots`
  30/30 in both join orders and with the menu entry, default 21/21.
  Increment 2 MERGED 2026-10-04 (v0.1451.0): players walk from their own door
  into the Commons and see each other there; neighbours are drawn as shells
  with doors that open for others; the plot is remembered per identity and
  server; a guest's home is put away and a guest never stands on a plot. Two
  review rounds. DAY 5 IS UNBLOCKED in code: the operator finds a partner
  (Discord or a brother) when ready. Increment 3 MERGED 2026-10-04
  (v0.1456.0): the relay's world is the mothership (the Commons, its mess
  hall, First Street), the crew work and eat there from the same food stores
  as players on one game clock, the explore quest ends with "find your home",
  a stored Pioneer world upgrades with progress kept. 16 review findings fixed;
  on main, all rigs green (67/67 per --plots order, the crew judged where they
  are drawn). NPC homesteads ANSWERED 2026-10-04: their contribution stays 0, and during
  early development the fleet is UNLIMITED with a per-player ledger of what
  each player used and contributed, in the red or the black (built 2026-10-04 on branch fleet-ledger: the setting fleet_supply_mode, the ledger, Inventory > The fleet, Server Settings > ADMIN > Fleet supply; design doc, increment 3, "The fleet ledger, as built"; released v0.1458.0).
  Increment 4 MERGED 2026-10-04 (v0.1459.0): getting around at ship scale
  (the relay's speed check answers an oversized move with a correction, never
  a freeze, and knows every honest fast move; transit links by stable ids;
  "aboard" is inside the ship's bounds; delivery by view, in at 250 m and out
  at 300 m; each home has its own air; nobody is told who lives on which
  plot). Three reviews, 21 findings fixed; its new rig legs found BUG-151.
  DONE (v0.1464.0, 2026-10-05; next is increment 5b, the household page): increment 5, building only on your own plot, was IN PROGRESS 2026-10-05 (build plan:
  docs/design/ship-homes-increment-5-plan.md; Wave 0, the contract, building; then Waves 1 to 3)
  (design doc section 7:
  `may_build`/`may_remove`, the permit certificate, ShipStructureEditing off
  while joined unless the relay grants `can_edit_ship`, through the ALTER
  block per BUG-046). Left for later (design doc section 7): a Dev move of the plot
  does not carry animals and decoration plants; idle plots are never given
  back automatically (open question 19); a vehicle-driving rig leg needs an
  IPC verb. The rig follow-up about the previous order's game still holding the exe is done (BUG-142, v0.1452.0: all four rigs share a copy that waits out the lock). Follow-ups 1a found: put
  the Commons machines on the ship's own power instead of the household
  battery; `just verify-screens` fails 2 of 12 because the starter kit's
  17-item backpack pushes the inventory's "Home" row off the screen (the rig
  needs to scroll it into view; not 1a). Was: increment 1a (the ship and the home
  come apart: a plots list in the ship file, the home in its own design file,
  assembled at my plot) and 1b (the relay hands out plots, each player spawns at
  their own, proven by the co-presence rig in both join orders); then increment 2
  (walk from your home to the Commons and meet there), which is what Day 5 needs.
  Its section 9 holds 18 open questions for the operator, each with a
  recommendation.
  **Multiplayer homes, decided by the operator 2026-10-03** (verbatim in the
  journal): every player's home has its OWN location on the mothership; homes
  never overlap; a player builds only inside their own home; there is no "main
  player", all players are equal; players meet anywhere on the mothership (mess
  hall, recreation room, crafting areas, the hangar where private ships are
  kept), and some missions start by grouping at a transport ship in the hangar;
  traded goods are physically transported (by the player, a robot or another
  method), so distance and ship-to-ship travel cost time, which is what trains,
  elevators and other fast travel are for; several STARTS of different scale
  (a full homestead for dedicated players, an apartment in a complex with shared
  group areas such as the mess hall, and a bare-minimum start for kids and
  people short on time). It must make logistical sense; HumanityOS could serve
  city planning. This SUPERSEDES the first shared-building design
  (docs/design/shared-building.md: every home at the same coordinates, anyone
  building anywhere), whose build was stopped at its first step.
- **Day 5:** a real two-person session on the VPS, then fix what it finds and
  reshoot the clips.
**Backups (2026-10-02):** an inventory found the newest snapshots restore
cleanly (integrity check ok, row counts matching live) and that the backup
key exists only on the VPS (a copy is now on the operator's PC). Fixed in
v0.1439.0: BUG-122 to BUG-124. v0.1442.0: game saves keep rotating snapshots
with Restore in Settings > Data (BUG-129), and "Back up now" keeps the newest
10 (BUG-130). Still open: a Restore has not been clicked in a running game.
**Waiting on the operator (2026-10-04, after his answers):** who and when for
the Day 5 session (he will post on his Discord or ask a brother when it is
ready); how to balance what NPC homesteads give the fleet against what human
players give (his open question). ANSWERED 2026-10-04 (journal, verbatim):
backups stay as they are; no GitHub branch protection for now (immediate deploys
suit the dev cycle); the shared-world clock got an in-app admin control (its
default, 72x that morning, became real time the same night: see THE FIRST
HOUR, decision 5); multiplayer stays ahead of arc A ("not as important as getting
the game playable"); the net-cup lean stays; server worlds start every player
FRESH (only appearance crosses; the mothership gateway arrives players stripped
of upgrades); joint homes wanted later; NPCs eat from the same stores; BUG-136
overloaded = slower walking and no jumping under gravity in realistic mode, mass
still applies in low g; pipes show their real material, made identifiable with
colour, labels and stripes (research the standards first); donations: nonprofit
https://www.sponsor-a-can.org/donate/ (tax-deductible), him directly
https://www.patreon.com/Shaostoul (not); the life-safety guides stay "sourced"
until a qualified person is officially involved; the landing hero shot becomes a
night-to-sunrise shot from Silverdale toward Mount Rainier. Earlier: the start
tiers (2026-10-03: the full list, Homestead default), the expiring erase marker
(2026-10-04: being built), the old plain backups deleted (2026-10-02).

### RESUME HERE: where the work stands (updated 2026-10-10)

- **FIRST, the VPS (BUG-169, found 2026-10-09):** a crawler filled the disk through the git
  mirror's archive downloads on 5 October; both v0.1464.x deploys failed and the live relay ran
  from a deleted binary. Done 2026-10-09:
  - the operator's `just sync`;
  - nginx limits archive requests and Forgejo deletes archives after 10 minutes (and was
    restarted);
  - the v0.1464.x release files were re-fetched and verified;
  - the 73 Disk Guard alerts were removed from #announcements.

  In the repo: `scripts/clear-build-cache.sh`, the disk guard and watchdog fixes, and the git
  site's nginx config. The mirror synced again at 22:14 UTC.
- **#announcements is one post per release (operator, 2026-10-09: "The cleaner it is, means it
  is easier for people to follow development.").** 5,325 bot posts became 2,354: one per release,
  "🚀 **vX.Y.Z**: title · [Release notes]". The deploy workflow no longer posts, and the desktop
  build posts once per release, only after a successful build (it used to post "binaries ready"
  after failed builds too). The old rows are saved in `/root/announcements-cleanup/` on the VPS;
  the re-seed archive `data/announcements_archive.json` matches the clean channel; the script is
  `scripts/vps/clean-announcements.py` (safe to rerun).
- **Safety: the planned steps are built (2026-10-09 to 10-10, v0.1465.0 to v0.1475.0).** The
  operator asked for "tools to protect themselves" and a mode that protects children from
  predators and scams. Design: `docs/design/blocking-and-safe-mode.md` (sections 10a to 10j hold
  the operator's answers and each step's spec as built). Shipped on the relay and both clients:
  the six defects of 3.7 (BUG-170 to BUG-173); friendship passes that never expire (A); "who can
  reach me", safe by default, with a Message, Call and Trade tick per friend (B, 10c-ii); Block
  (C); reports the admins can check, help outside the server, reported posts never showing a file
  or a link (D, 10e-ii); calls through our own STUN and forwarder (E); warnings on messages and
  the recovery-phrase guard (F); the protected setup (G); an admin erasing another person's data
  (10i); a report about a group reaching its creator, with Remove changing the group key (10j);
  BUG-176 and BUG-177; a refused contact request that says so; the ten-trade limit; the web Trade
  page.
  **Operator decisions** (each with a recommendation, none blocking what is built): (1) the age
  position: the web chat's entry screen says "you confirm you are 18 years or older", which a
  setup for children contradicts; the dated finding's options are a general audience of 13 and
  over (recommended, the setup aimed at parents of teenagers) or 18 and over with the setup "for
  someone you look after"; (2) whether `web/pages/rules.html` should name pornography and content
  encouraging suicide, self-harm or eating disorders as not allowed (the UK Online Safety Act's
  section 12(5) condition); (3) whether to email the FTC's COPPA hotline (CoppaHotLine@ftc.gov)
  about the no-sign-up design (an outside message, the operator's to send); (4) opening UDP 3478
  so calls work (`docs/admin/call-forwarder.md`); plus the report-duties and camera-safe sign-in
  items below. **Before any app-store listing** (Flathub, Snap, WinGet, Homebrew, F-Droid),
  decide on California AB 1043 first (`docs/reference/findings/2026-10-10-california-ab-1043-age-signals.md`):
  a listing would very likely require an age request at first launch.
  **Smaller follow-ups, not started:** the native fixes of BUG-173 still want seeing in a running
  app: a first call between two home networks, once the port is open (the click-to-load picture
  placeholder was seen rendered 2026-10-10, `tests/snapshots/chat_click_to_load.png`); the relay
  cannot tell a text message from a voice message inside a sealed DM, so a pass's
  `voice_message` can only be enforced by the recipient's app. Done 2026-10-10 (v0.1476.0):
  files in DMs and P2P groups encrypted on both clients and shown inline on the desktop (10k), BUG-178
  (Pin, Edit and React no longer reach the server from a private conversation), BUG-179 (the
  desktop log no longer carries DM file keys), and the account words. Then a batch review of the week (three critics, one area each) found about 30 defects at the seams, all fixed: v0.1476.1 (web, incl. BUG-180) and v0.1477.0 (relay and desktop); BUG-181 lists them. A second review of those fixes found six more (BUG-182), fixed in v0.1478.0, the main one spec 10l: a friendship pass counts as given only once the server says it took it. And v0.1479.0 made the web scratch pad as private as the desktop's (BUG-183); v0.1480.0 has each app check its own call setting on an incoming ring too, not only the relay (BUG-184); v0.1481.0 takes Block into the game and stops sending other players anyone's health, inventory or quests (BUG-185). A second batch review of v0.1478 to v0.1481 led to spec 10m, shipped in v0.1482.0 (BUG-186): passes across one person's own devices, and the desktop no longer asks the server for a DM's history. A review of 10m found more two-device orderings; spec 10n (v0.1483.0, BUG-187) syncs each friend's choice as its own note, as block notes are, and 10o (same release, BUG-188) closes its parity review. **Done 2026-10-11:** the nginx upload change (live; paused for an hour while BUG-189 was fixed in v0.1484.0). **Next:** the **Media showcase** (`docs/design/files-showcase.md`: the operator answered categories, public or members only, labels, a storage meter, resumable uploads for multi-GB files; adult content waits for a dated legal finding). Increment 1 (relay) and its review fixes F1 to F7 are merged on main, not yet released; a file already public (chat, Shared Files) goes on Media as public only. **Released v0.1485.0 (2026-10-11):** the Media page on both apps, resumable uploads, per-role Media/private/daily/hourly/held limits, private files expiring with the DM mailbox, members-only tickets, BUG-190 (videos and HEIC/AVIF photos stored without their location); nginx resumable block live. **Merged on main 2026-10-11, not yet released:** answers 4, 5, 6 and 9 (hidden details removed on the sender's device with a remembered keep/remove switch and a "send anyway?" step for a file that could not be checked; Files in messages settings; encrypted kept copies of opened private files), on both apps; a public HEIC on the web goes to the server, which cleans it; the composer hints sit above the text box; BUG-192's code (federation back-off, Re-check key, add-by-key with today's keys); and BUG-193 to BUG-195 (the desktop took everyone's profile as its own; desktop saves wiped fields they do not edit; the website pushed its stored profile on every connect): every save now sends the whole profile, and the relay puts the person's privacy map on their own copies only. Then two review rounds (2026-10-11, four critics then three) found and fixed BUG-196 to BUG-199 (profile saves on the wrong copy or counted done too early, friends-only fields in two HTTP lookups, a removed peer's live link, kept-copy leftovers) and the follower script's roll back hazards; the reviewed follower is installed on public.guide (`/usr/local/lib/humanity/`, ran once on Linux: "already on v0.1485.1"). **Released v0.1486.0 (2026-10-12 UTC):** deployed; public.guide updated itself from v0.1485.1 to v0.1486.0 (backup, health check, federation back up both ways within seconds). **Released v0.1487.0 (2026-10-12 UTC):** answer 10, device-to-device big files, on both apps and the relay: a file too big for the server goes from the sender's device to the receiver's in a DM, through the server's call relay unless two friends allow direct connections, checked piece by piece, resumable, one connection per pair, and every end said to the other side (also when a note had to wait for the connection, or never arrived). Three review rounds (rules 1 to 9 in files-showcase.md) and a real two-browser run (20 MB, and 120 MB with the receiver's page reloaded mid-transfer, resumed to Saved/Sent). BUG-200 fixed in both apps ("Accept files from" kept per server and identity; on a server never seen the tighter word wins when a profile is there). **Released v0.1487.1:** the phone check of web chat (375 px): the header fits and every button takes a tap, the text box fits, the footer tab no longer covers it, and chat uploads show a percentage. public.guide updated itself to v0.1487.0 at 07:22 UTC. **Next (the plan the operator approved):** back to the game: the first real two-person session on the operator's server (needs him and one other person), then the household page (v0.1488.0 carries its groundwork; the game wiring, the Household card and the in-game proof are being built), IN PROGRESS 2026-10-12 (plan: docs/design/ship-homes-increment-5b-plan.md; a plot's holder gives, renews and takes back household permits, Block takes them back, the permit holder walks in and builds; Wave 0 with a worktree builder). Still to do for this feature: a desktop-to-web run between two real apps (needs a way to drive the desktop chat from a script; no dev IPC for it yet), showing each person their forwarded daily allowance, group transfers, and a desktop "direct connections" setting (the website has it). Small follow-ups: BUG-200 is fixed in both apps (the word each server held is remembered per server and person), except that a choice reaches a background server only when the app is on it; the desktop's protected setup has no PIN step for opening "Accept files from" (it is greyed instead); group file settings wait for a group-settings increment. BUG-191 (profiles do not replicate: no app signs them) IN PROGRESS 2026-10-12: plan docs/design/bug-191-profile-copies-plan.md (the public copy only, signed by the person's apps, withdrawn on unlisted and on erase), Wave 0 with a builder. Library A to Z shipped in v0.1483.0. Known limit, unchanged: the relay keeps at most 4,096 withdrawn passes per person (`src/relay/storage/friend_passes.rs`), and each untick uses one.
- **Report duties: the finding is in, three decisions for the operator (2026-10-10).**
  `docs/reference/findings/2026-10-10-report-duties-child-abuse-material.md`: in the United
  States a "provider" with actual knowledge of apparent child sexual abuse material must report
  it to NCMEC's CyberTipline and preserve what it reported for one year, with no duty to scan;
  official sources do not say whether a volunteer's small server is a provider. In the UK an
  individual can be the provider and a user's report counts as knowing. Done: a reported post's
  uploaded files and links are never shown to admins (`without_files_or_links`). Decisions, each
  with a recommendation: (1) "Delete the post" leaves the uploaded file at its public address;
  recommended: take it out of public view at once and hold it, unreadable through the app, for
  the one-year preservation period, then delete it. (2) Reports are kept 90 days after a
  decision (the operator's answer); recommended: one year for the `child_danger` reason, 90 days
  for the rest. (3) Whether to ask NCMEC (ESPteam@ncmec.org) if a volunteer-run server counts as
  a provider and should register; recommended: yes, the operator writes it (an outside message
  is the operator's to send). The rest of this entry is the earlier note.
- **Legal research for reports: DONE 2026-10-10.** The question raised 2026-10-09 (a server's
  duties on a report that may involve child sexual abuse material) is answered in
  `docs/reference/findings/2026-10-10-report-duties-child-abuse-material.md`; its three decisions
  are the "Report duties" item above. Reports never carry a way to open a reported file, and
  admins are never shown files or links.
- **Signing in where cameras can see (raised 2026-10-09; the operator's PIN was once watched
  and money was stolen).** Found: the desktop "Quick PIN" unlock (`src/auto_unlock.rs`,
  KeychainPin) has no limit on wrong guesses, and the guessing can be done offline by anyone who
  has the computer's signed-in Windows account (the device key sits in the OS keychain beside the
  blob), so a 4 to 6 digit PIN protects little there. Recommended, ranked: (1) sign in on a new
  device by scanning a QR from one already signed in, camera-safe because the code holds only a
  one-time public lock; (2) OS-backed unlock (Windows Hello, Touch ID or Face ID, Android), where
  the device's security chip limits guesses, replacing Quick PIN; (3) security keys as an
  optional unlock (FIDO2 hmac-secret); (4) secrets never shown while sharing a screen through the
  app, and on Windows the window kept out of screen captures while one is shown; (5) anything that
  moves money asks again. Not worth it: a custom code format, RFID tags holding secrets. Awaiting
  the operator's pick; belongs in an identity-on-every-device design.
- **Released 2026-10-05:** v0.1459.0 to v0.1463.0 and v0.1464.0 (ship-homes increment 5, building
  only on your own plot; BUG-165 and BUG-166, the walking-backwards fixes; two new clips),
  all delivered to the taskbar exe.
- DONE on Windows v0.1513.0 (src/media/system.rs; the Media page, items over the chat and in-world
  screens, with sound; the Media page's video got its sound and Mute in the same release; "Play here"
  on a video card in chat, v0.1514.0). Still to do from this entry: macOS through AVFoundation,
  Linux through the person's own ffmpeg libraries, and the OpenH264 check below. The original entry:
  (operator asked 2026-10-17, "How do we give users the ability to play MP4s in our
  app/website?"): **play MP4 (H.264 video, AAC sound) in the desktop app by asking Windows.**
  The website already plays MP4 (the browser's own decoders). Windows 7 and later carry both
  decoders in Media Foundation for any desktop application, H.264 falling back to software, so a
  player that calls Media Foundation itself gets the picture AND the sound (the September
  finding's missing sound was the browser engine's limit): dated finding
  `docs/reference/findings/2026-10-17-windows-media-foundation-decoders.md`, legal reading
  unchanged from 2026-09-19. Build: a Media Foundation path in `src/media` (Source Reader opens
  the MP4, decoded frames and sound into the existing `VideoPlayer`), used on Windows for any
  file the AV1/Opus path cannot play, in chat cards ("Play here"), the Media page and in-world
  screens; a plain sentence when the decoders are missing (Windows N without the Media Feature
  Pack; H.265 without Microsoft's extension). macOS (AVFoundation) after; Linux keeps the
  person's own ffmpeg. Per platform, with the sentence to show when no decoder exists:
  `docs/reference/findings/2026-10-17-platform-video-decoders.md` (macOS/iOS: AVFoundation, all
  built in; Android: required by the CDD, AMediaCodec; Linux: nothing to count on, load the
  person's own ffmpeg libraries at run time; the Microsoft Store only for H.265 on Windows). Its
  open check: whether Cisco's OpenH264 now decodes Main and High (its README and its release notes
  disagree), which decides how soft the Linux sentence can be on Fedora.
- DONE v0.1512.0: **a Media item posted in chat is a card** (operator, 2026-10-17: "I'd rather people
  know exactly what they're downloading instead of guessing"): one line that opens out to its
  picture, description and every file, and its full window on a click. Posts made before this
  linked one file and stay as they are.
- DONE v0.1512.0: **rename a file of a Media item, and use a picture already on it as its cover**
  (operator, 2026-10-17: "I uploaded a png with the blend and STL files but, I can't seem to assign
  it as the preview photo"). The ending stays fixed on a rename.
- DONE v0.1513.0: the Media card follow-ups (labels by name on the desktop, cards in the website's
  thread panel, the framed item window signing through the chat, the desktop's picture-link rule,
  sizes held to a shared fixture). Left: in that framed window a members-only item's FILES still
  need the page's own upload token, which a passphrase-wrapped identity has not got (pass the
  chat's token across, or give the frame a session) DONE v0.1516.0 (the bridge lends the chat's upload
  token too); a report made from the framed window, adding a file through Edit there, and the upload
  window a drop opens, are not bridged.
- DONE v0.1511.0: **drag and drop onto the window, the upload window that posts in chat, and the
  picker's List / Details / Pictures views** (operator, 2026-10-17). Follow-ups: the web chat page
  gets the same upload window as the Media page (being built), video previews in the Pictures view
  (DONE v0.1515.0 on Windows, through its own decoders; elsewhere still the person's ffmpeg)
  (a frame through the person's ffmpeg), and a folder drop that offers the files inside.
- DONE v0.1510.0: **every file shows its type, always** (operator, 2026-10-17: "so people ALWAYS
  know what file type they're looking at ... protect people inexperienced with technology and
  possibly illiterate people"): a badge with a drawing, a word and a colour (green to look at,
  amber for care, red for programs, scripts, shortcuts and disguised files, grey when unknown) on
  every file in both apps, from `data/safety/file_risks.json`; and the Edit window, Add to Media
  and "Send a file" in the chat sidebar's colours, darker, the way the file picker already was.
- DONE v0.1509.0: **a video attached in chat is a card, not a bare path (BUG-203).** Both apps show
  every public file that is not a picture as a card (name, size, Download, Show in folder); the
  website says when the browser cannot play a video; the desktop says a video plays in the
  computer's own player and offers View in 3D for a model; the relay keeps the name's words
  (`<ms>_WH40K_Marriah_Carey.mp4`). Same release: **what kind of file it is** (a Discord
  suggestion): a line under a received file saying it is a program, script, archive and so on, and
  calling out a file whose name says a picture or video over a program's bytes (public cards,
  private files, device-to-device files, Media rows; `data/safety/file_risks.json`). And the file
  picker's dimmed backdrop and tinted sections. Follow-ups: "Play here" for a WebM video in chat on
  the desktop (the Media page's player), and an MP4 there through the computer's own ffmpeg
  (`transcode::begin_ingest`), as the Media page does.
  The chat bar fix below is CONFIRMED by his screenshot of 2026-10-14: the message box sits
  fully above the taskbar.

- DONE v0.1496.0: **the desktop chat's message bar no longer sits under the taskbar.** The bar
  reserved a fixed 52 px but its content is about 66 px even at the default text size; it now
  reserves its own parts as last drawn (`INPUT_BAR_REST_ID`) plus one line and the box's extra
  lines. Follow-up: a headless test at a larger text size, and the one-line box height measured
  instead of `COMPOSER_ONE_LINE_H` (22 px; at a much larger text size a line is taller).
- **NEXT in TIER 0 (updated 2026-10-12, after v0.1492.0; weekly usage at 96%, resets Thursday 8 PM):**
  - **Increment 6 is DONE** (Waves 0 to 3): `just verify-starts` passes 18/18 and was seen red on
    a real build (the save swap broken). Left for later: a second home of your own on one
    computer (the new-home row offers the choice only before your own home is saved), moving
    house, a server-held character. Plan `docs/design/ship-homes-increment-6-plan.md`.
  - **Increment 7, stacked homes and decks: IN FLIGHT (operator, 2026-10-20: "Sounds like a plan,
    let's proceed").** Plan `docs/design/ship-homes-increment-7-plan.md` (four code surveys; the
    contract, seven lanes, five decisions made for the operator). The homestead gets the first
    floor docs/design/player-home.md already planned (the quiet wing upstairs by a stair off the
    hall and the elevator at (31, 26); a dining room and a second bedroom downstairs). Wave 0 (the
    contract), the stairs-and-guards finding with the upper floor's layout draft, and the light
    budget analysis are running; Wave 1's lanes follow Wave 0's merge. (Update, same evening:
    Wave 0 merged; Wave 1's lanes G, M, R, C1, C2 and L running; the home's probe priority merged,
    home 22,092 probes at 1.00 m, the rest of the ship 9,878 at 2.50 m, the GPU cost unchanged;
    a live "Bounced light detail" setting in Settings > Graphics being built.) (Update 2026-10-21:
    Wave 1's lanes G, M, R, C1, C2 merged and proven in the running game (verify-storeys 76/76,
    verify-falls 88/88); three critics found 2 HIGH and ~20 lesser seams, fixed by six fixers
    merged the same morning (f5e0a2a32 to aa1d32c27) and released in v0.1534.0 with BUG-140.
    Lane L, slabs stopping lamps and sun in the room GI, and a seam critic's six findings (the editor's
    close on a stair or in the shaft, E and Take down through doorways, sight by height, a hung
    unit's storey) followed in v0.1535.0. Increment 7 is DONE apart from the follow-ups below.)
  - **Increment 7 follow-ups (found by the 2026-10-21 fixers).** Fixed the same day (the drawing
    fixer, merged 680525ccf; the movement fixer, c311a3521; both in v0.1537.0): the HUD's
    labels, the dimension overlay and the 3-D helpers follow the editor's cut-away; plants and tents
    follow it; the room card names the elevator for the whole ride; machine mesh slots are pooled
    across world entries; the cut-away is redrawn after a world load; port gizmos stay under their
    ceiling and inside the shell; hangers reach the real ceiling (the greenhouse roof); placed box
    pieces collide by their box and height (a raised deck is a ceiling too); a built box is solid to
    the editor's close and to the spawn a Save writes; a shut door stops E and Take down; elevator
    cars are keyed by ids; a spot on a wall's centre line is no longer moved through that wall;
    stale comments and player-home.md's plan and walkthrough. Still open: `moves::on_my_storey` and
    `stands_in_storey_of` (relay reach, quests) assume 3 m storeys (the relay holds no slabs);
    upstairs, slab occlusion costs +0.53 ms at 2560x1387 (pack the layer rows, cull runs per light
    tile: docs/design/room-gi.md, Floors between storeys); a jump from the stair's seventh step can
    slip past a slab edge (walled by G1 in the homestead); the editor's close can step onto a piece
    0.6 m up; on the ground floor a spot exactly on an interior wall still counts as a place to
    stand (only upper floors check room edges); teleporter and elevator frame posts have no
    collision (hollow frames); box pieces and stairs do not hide HUD labels; runs to the
    greenhouse's g1 and g2 end at 2.70 m (the per-storey end clamp); a machine selection survives a
    storey change (its handles hidden); the family mushroom tents settle at 68 to 82% humidity,
    under the oyster's 85% (known); `just launch-bg` and hand-typed boots skip the rigs' boot lock.
  - **Next for bounced light: probe streaming** (operator's question, 2026-10-20, "is there any
    other way?"): only rooms near the player hold probes, so the ship's size stops mattering and
    the shared Commons need not drop to 2.5 m to keep the home at 1 m. Hardware ray-traced GI on
    cards that support it is the later upgrade, with probes kept as the fallback.
  - **DONE 2026-10-20: the Platform tab is retired (operator: "I feel like the platform page we had
    before is redundant now").** One Dev tab (`GuiPage::Dev`, src/gui/pages/dev_tab.rs) holds Bugs,
    Testing, Performance, Dev and Planet Tuner in a side list; Calculator, Notes, Calendar and Browser
    are Apps' HumanityOS tools; Recovery is Settings > Account > "Recovery through trusted people",
    right after Security, in plain words. Web: platform.html retired (and removed from the live site
    by scripts/sync-web-root.sh), the top menu's Dev opens the Dev Workbench (dev.html), which carries
    the same list; recovery.html is linked from Settings > Security. lib.rs followed and the three
    temporary bridges were deleted (5aac163bb, 2026-10-21).
  - **DONE 2026-10-20: Maps increment 0** (docs/design/maps-zoom-to-anything.md section 6):
    src/gui/pages/cosmos.rs is src/gui/pages/maps/, one file per view, its state one
    `GuiState::maps`; the three Maps snapshots rendered byte-identical.
  - **DONE 2026-10-21: Maps increment 1** (v0.1536.0): the page opens on your ship at ~1.5 ship
    lengths with Earth behind it and follows it (Stop following, Back to me, the Home key, an "Open
    where I left it" toggle); the real world drawn behind the symbols from the map's own camera
    (`engine/map_view.rs`, `render_view_onto_kept`, half resolution while moving, at most once a
    second at rest, aboard and within 60 km only); the ship named UHS Mothership; station rows from
    data with computed altitudes (35,870 km: the game's synchronous orbit keeps pace with the 24 h
    solar day, the real geostationary 35,786 km with the sidereal day; the row says so); the
    overview looks down from ecliptic north; Lagrange points and reference rings fixed. BUG-210
    (the sun direction in the hull frame for the celestial pass and god rays) fixed on the way.
    Next: increment 2, a solar system that looks real (the diagram toggle and a map light were
    left for it). Found, not fixed: **Earth's north pole faces the ship** (from the equatorial
    orbit the rendered Earth shows its pole under the ship where the equator at 122 W should be;
    older than BUG-140 and BUG-210); SSAO takes its focal length from the window height, wrong for
    the map's canvas; point lights are picked by distance from the player, so from 1.5 km some
    hull lights are missing; `cpu.frame_total` and `cpu.present_wait` are recorded only on game
    frames; switching back to Live or the System view shows one frame of the old backdrop; the
    upcoming-events list may report one conjunction several times (Jupiter and the Moon three times
    within a day; unconfirmed).
  - **DONE 2026-10-21: the Moon's node and perigee turn** (v0.1536.0): JPL's rates on any body that
    carries them; the Moon within 0.34 degrees of Horizons in latitude and 2.5 in longitude over
    2000 to 2040; the Maps page finds exactly NASA's four solar eclipses of 2026 and 2027 (it now
    allows for the Moon's parallax). Lunar eclipses are not predicted on the page yet.
  - **DONE 2026-10-21: rigs take turns** (v0.1536.0): verify-runtime's gate clock starts at the
    game's boot, not while waiting for a quiet machine (that wait refuses with exit 1 after 60 min,
    never a false FAIL); a machine-wide boot lock (`scripts/lib/boot-lock.js`) so two rigs never
    boot at once; worktrees find the DXC compiler in the main checkout. Gaps: `just launch-bg` and
    hand-typed boots skip the lock; other rigs still ignore unknown flags; perf-report does not yet
    refuse an FXC sweep. Flaky under load (passed alone): `engine::file_transfer` "one dial one
    allocation", `net::webrtc::turn` wrong credentials, `relay::resumable` follow-ups "a dropped
    request keeps its finish to itself" (2026-10-21, 3 of 3 alone), `gui::pages::files::resumable`
    "pause_resume_and_cancel" (2026-10-21, 3 of 3 alone). Also the relay-follower rig tests (scripts/tests/relay-follow.test.js: a different one of its 12 failed in two loaded runs, 12 of 12 twice alone, 2026-10-22). Two upload tests in one day: make the
    resumable-upload tests independent of timing (they fail only while other builds load the machine).
  - **DONE 2026-10-21: BUG-140, the planets orbit the right way** (v0.1534.0): the ecliptic enters
    the world by a rotation, not a mirror; the planets match JPL Horizons within 0.32 degrees, the
    Moon's phases match the US Naval Observatory within 1.44 (it had no elements and took its phase
    from a hash), the Sun on the ground is unchanged. Left (BUG-140's entry in BUGS.md): from the
    ground at night the Moon is drawn black (FIXED 2026-10-21 as BUG-212: a body in the sky takes
    its own sun, the ground keeps the camera's; BUG-211 the same day, Earth's pole toward the
    ship, both in v0.1538.0); the Moon's node and perigee (FIXED v0.1536.0, JPL's rates).
  - **DONE 2026-10-21: the sky bodies BUG-211 and BUG-212 left (recorded under BUG-212, "The
    follow-up")**: every body turns by its own IAU rotation from sol.json (the Moon keeps its real
    near side to Earth, with its optical libration), the frame lock rides that turn; the daytime
    and twilight Moon shows through the air while the stars stay hidden; weather fog hides the
    Moon and the stars by its depth; another body's clouds and air take the Sun before any air;
    the frame body is lit from low orbit over the night side (it was dark whole, seen in the rig);
    the white point on the Moon was the HUD crosshair (a showcase `hud` verb hides it for sky
    captures); the compass aboard points to Earth's north. Left: the Moon's face is turned by the
    real clock while Earth's spin follows the game clock, which is right while the game day is
    real time and leaves the Moon's phase-to-hour relation off on a faster day; the game has no
    axial tilt, so the Moon's north stands near the zenith-pole of the game sky rather than tilted
    as in life; walking on the Moon takes "north" from world +Y, 1.5 degrees off the Moon's pole.
  - **DONE 2026-10-21: Recovery through trusted people, end to end on both apps**
    (docs/design/recovery-through-trusted-people.md; desktop Settings > Account, website /recovery).
    Set up: choose 2 to 10 friends and how many must help; the 32 bytes the recovery phrase writes
    down are split with Shamir over GF(256) (`blahaj`, MIT; the website writes out the same
    arithmetic, held to it by a shared fixture and blahaj's own field vectors), each piece sealed
    to a friend's Kyber768 key (ML-KEM-768, BLAKE3-KDF, AES-256-GCM) and posted as a signed
    `recovery_share_v1`; the server never learns who the trusted people are (a note to self names
    them). Ask for help: a one-time key on the new device, a 16-character code read aloud (its
    fingerprint, so no server can swap the key), the trusted person checks it is really them by a
    call or face to face and sends their piece sealed to that key; the device rebuilds and restores
    through "Restore from recovery phrase", the same account (the key-change step is gone). The
    person's own devices see "someone asked" with "That was not me". Server: `recovery_pieces`,
    `recovery_floors`, `recovery_asks`, `recovery_answers` (the first design's tables, which named
    guardians in the clear, are dropped at start), seven-day requests, limits, erase and export.
    Left: the trusted people's Kyber keys come from the server's member list as for DMs (signed key
    cards would close that for every feature); the website needs the account readable in this
    browser to set up or help (the standalone-page limit); the asking device keeps its key in
    memory only, so closing it means a new code.
  - **A "per-room ambient" lowest level for Bounced light detail** (promised to the operator,
    2026-10-20, after he asked why not just brighten textures): one ambient colour per room from
    that room's own lamps and windows, no probe lattice, about a tenth of the probes' memory; it
    follows lamps and keeps rooms apart but is flat within a room. A shader change (the room GI
    sample), so it follows lane L's merge; the live setting being built now has probe-spacing
    levels only.
  - **Media round 5: Waves 0, 1 and 2 are DONE** (released through v0.1494.0): the desktop and
    website item pages and editors, counts, the item page for social sites, the strips, the
    Admin tab, the snapshots, the Reports page's Open. Not built: the browser proof rig
    `scripts/verify-media-round5.js` (plan section 6). Known flake:
    `resumable::tests::pause_resume_and_cancel` failed once under load.
    In v0.1491.0 and v0.1492.0: Wave 0, 1A (relay edits), 1B (counts, the item page for social
    sites; the live nginx routes `/media/<number>` to the relay), 1C (desktop list and Admin tab),
    1E (desktop strip), 1F (web strip), 1G (web list). Next: Wave 2 (2A the website's item page
    and editor, after 1D; 2B the proof rig and snapshots), then phase 2 ("Also put this on...").
    public.guide's nginx needs the same `/media/<number>` block by hand. Waiting on the operator:
    nothing. The operator has no address for a DMCA agent, so none is registered until a fiscal
    sponsor or the project's own nonprofit exists (journal 2026-10-13). The entity findings are
    public since 2026-10-13, without the founder's personal notes, which are kept privately.
  - **Next for Media (operator, 2026-10-13):**
    - DONE 2026-10-13 (v0.1495.0): **"Report a copyright problem"** on both apps (works signed out),
      admins decide in the Admin tab, the public takedown log (rules page, desktop window). Open:
      on the desktop item page a button placed on its own line under the actions stopped two
      tests' clicks registering (`media/item_view.rs` comment); worth a look.
    - DONE v0.1516.0 (media/your_folders.rs; marks from a private on-this-computer record, media/from_here.rs, until phase 2 fingerprints exist): **Your folders**: point HumanityOS at folders (his E: "Media" drive), browse them like a
      file explorer, pick files to put on a server's Media page; nothing leaves the computer
      unless picked. Builds on `src/gui/widgets/file_browser.rs`; marks which local files are
      already on which server (phase 2's fingerprints). Desktop only (operator, 2026-10-13: no website
      version, so every browser behaves the same).
    - DONE v0.1517.0 (src/games/ readers, src/gui/pages/games.rs; found this computer's 27 Steam
      games and Fortnite, `node scripts/lib-tests.js games::this_computer --ignored --nocapture`).
      Play and Open in Steam confirmed by the operator on 2026-10-19. Open: GOG untried on a
      real install (none here). DONE v0.1519.0: a library to look at (Grid and List, search,
      sort by name, recently played, most played or size, store filter; a details panel with
      playtime, size, achievements, his own screenshots, Workshop mods by number and links out;
      Steam covers read in place from Steam's own cache, Epic and GOG by their program's icon; no
      store logos), all as the dated finding
      docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md allows. Checked on
      the operator's real library (27 Steam games with covers, hours and achievements; Fortnite by
      its icon). Open: in the details panel, right-aligned text (the achievement dates) and the
      banner run flush against the scroll bar with no gap (cosmetic; not clipped, checked at full
      resolution; being fixed with the Tools page's shared panel); Workshop
      mod names live on Steam's servers (not fetched); Epic and GOG store pages need an address
      name their local records do not carry; descriptions or genres only ever from Wikidata, on a
      press, per the finding.
      **A Games library** (operator idea, 2026-10-13): list and launch every installed game
      (Steam, Epic, GOG, anything installed elsewhere), the stores still doing downloads and
      updates behind it. Findings: `docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md`
      (no store's terms mention launchers; recommended first version: read the stores' install
      records without writing to them, start a game only on a person's click through the store's
      own link or the game's exe, Install/Update open the store's app, no SteamCMD, no store
      sign-in, no logos yet, an on-screen note saying why).
    - DONE v0.1520.0: **the Tools page like the Games page** (operator: "What if we make
      the tools page like the games page? Then what if through that interface users can
      install/launch/go to the website(s) for the various non-game apps?"). Grid and List on the
      shared `widgets/shelf.rs`; installed programs found from Windows' records, App Paths and
      Steam (7 of 40 on the operator's machine) and launched on a click; Install through winget
      only after a consent step in the details (per
      docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md). Open: the REAL
      winget install has never run (test builds compile it out; the operator's first Install is
      the first real one). DONE v0.1526.0: Microsoft Store programs found
      (`src/software/store.rs`, `store_family` data for 11 programs), Install on the tile, the
      intro folded behind "How this page works" (website mirror in progress). DONE v0.1523.1: the
      website's page mirrors the look as its Apps page.
    - DONE v0.1523.0 (operator, 2026-10-19): **Games and Tools merged into one Apps page** in six
      coloured parts (red Games, green HumanityOS tools from data/gui/humanityos_tools.json, blue
      Other programs, cyan Help, magenta Services, yellow Added by you), tiles a third shorter
      with the name once, Steam links for programs installed through Steam, launch through the
      Start-menu shortcut when Windows records no program file (DaVinci Resolve, checked on his
      machine), and Epic covers on an opt-in switch (Epic's CDN only, never stored; the first
      real fetch happens when he turns it on). Website mirror: DONE v0.1523.1. DONE v0.1524.0: **Media renamed Files** on both
      apps and merged with the developer Files page: places On the server (Shared on <host>,
      Shared files, Admin) and On this computer (HumanityOS folder: the app's own folder with data,
      assets and mods, text files editable with a .bak kept; Your folders); internal names too
      (GuiPage::Files, src/gui/pages/files*, src/relay/files*, /api/files/*, files.html,
      data/files/, docs/design/files-showcase.md; the old data routes became /api/data-files). Kept
      on purpose: src/media/ (the playback engine), the SQLite tables and signed purposes. The
      website's old Shared Files page and data browser became places of its Files page. Found on
      the way and fixed: BUG-205 (the data-file write route took no signature).
    - DONE v0.1521.0: **the Media page in three panes, editing in place** (operator: a
      list on the left, the item fitted to the middle (picture, 3D model, video, a sound
      visualiser), its details on the right; top, middle and bottom when the window is tall; Edit
      turns the details pane into the form, in the tinted parts so edit mode stands out, and
      Save returns it to view, instead of a window on top). `media/panes.rs`, `widgets/panes.rs`,
      `widgets/sound_view.rs` + `waveform.rs` (1024 peaks and a 24-band spectrum, computed once);
      the tiles kept as a Gallery view; the chat card opens one window with the same viewer and
      details. Open: a 3D model is fetched only on View in 3D (models can be 200 MB); nothing is
      selected until a row is pressed (a selection counts a view); Opus, M4A and AAC sounds say
      they cannot play here yet; leaving the page with unsaved edits asks on the next page (the
      app changes page in about 50 places). The website mirrors it since v0.1522.0 (web/pages/media-panes.js, media-sound.js).
    - DONE 2026-10-13 (v0.1495.1): **the project's history in the Library**, a "Project History"
      shelf with the 38 dated session histories and the 7 decision logs (catalog rows; re-run
      `node scripts/build-library.js` after a new history page).
  - DONE and released 2026-10-12: increment 5b (household permits, v0.1489.0), BUG-191 (profile
    copies, v0.1490.0), the first-hour audit's S5, S6, S8 and F11, BUG-163 part 2, rule 7 and
    the takedown section (v0.1491.0).
  Day 5 (a real two-person session on the VPS) is unblocked whenever the operator has a partner.
- **Follow-ups found today (not started):** BUG-161 (the copresence rig's steady-speed
  judges fail a starved machine: DONE 2026-10-20, a starved capture reads CONTAMINATED, exit
  3, and the rig holds builds at BelowNormal during each capture); BUG-163 part 2 (refresh a stale data folder on update:
  docs/design/data-folder-updates.md: DONE v0.1490.0); no gasoline or diesel
  item (Refined Fuel stands in for the generator's gasoline; DONE 2026-10-20, f4562e2c3:
  gasoline_0 and diesel_0, the refinery's refine_crude makes both, the generator burns
  gasoline, Refined Fuel retired); the refinery's drum accepts any liquid (DONE 2026-10-20,
  c4bb04a13: the drum is its crude-oil feed, takes only what its recipe runs on, and its
  card says why); campfire light (point lights in the celestial pass); heaters aboard only, and
  data/hvac.ron's heat pump and wood stove have no machine (DONE 2026-10-20, commit "A heat
  pump and a wood stove heat the home": heat_pump, a Mitsubishi MSZ-FS09NA, 620 W for 2,815 W
  of heat, its COP off its sheet's table, charged only its draw; wood_stove, an EPA-certified
  Drolet Escape 1800, 13,040 W on 4.07 kg of Wood Logs an hour from a firebox that takes only
  Wood Logs, its fire taking the home's oxygen and venting up an unmodelled flue; both built at
  a workbench and placed from the Life support tab; heaters are still aboard only); inside
  survey regions without
  terrain tiles the ground draws at 54 m triangles while buildings stand on the finest
  ground (BUG-156's note); far trees should re-ground on the patch drawn under them;
  src/systems/fire.rs takes fire damage per frame in f32 (BUG-164's note; DONE 2026-10-20,
  acd8a17e2: a fire's tick is its exact dose, the same at 30 and 240 frames a second); every
  drink gives 30 points whatever its volume (DONE 2026-10-20, 0800f1a5b: a drink puts back
  the water it carries, from its litres in item_profiles.ron); (Bleeding, bites and burns are wounds since S8,
  2026-10-12; falls hurt nobody yet (DONE 2026-10-20, 5e5ba96e6: a landing hurts by its
  speed, data/medical/falls.ron, a sprained ankle from about 2 m on Earth up to death
  from about 15 m in Realistic, softer in Forgiving; wired into lib.rs and released in
  v0.1527.0; proven in the running game by `just verify-falls` (2026-10-20, 40/40, red with the
  lib.rs hand-off removed): world entry hurts nobody, a 3 m walk off the stairs' top hurts
  nobody in Forgiving and sprains an ankle in Realistic, letting go at the ladder's top (3.2 m)
  breaks one, and a body placed in the air is not hurt. Since v0.1531.0 (76/76) it also falls on
  planet ground (Sahara at noon, 9.81 m/s^2: Forgiving unhurt, Realistic a sprained ankle, a
  placed body unhurt) and walks DOWN the stairs (no landing; red with BUG-208's step-down
  removed); the rig's held key now releases a camera park like a key press; a planet fall lands
  at its real speed at any frame rate (BUG-209); every rig writes requests through
  scripts/lib/write-request.js. The built deck: DONE 2026-10-20, commit "The planet walk stands
  on built pieces": the planet walk's floor was the terrain alone, so a player went straight
  through a roof; now it is the higher of the terrain and a finished piece's top under the body
  within a 0.6 m step up of the feet (`planet_build::built_floor_radius`), and verify-falls'
  planet deck leg walks off a 3.2 m Wood Roof built on the sand (88/88), red with the floor
  disabled (tests/fixtures/falls/red-deck-floor-disabled.json). Not yet covered: a vehicle
  stopping. Left from it: a built roof does not stop a climb from below; on a planet pieces are
  placed only from the ground (no second storey); aboard, nothing stands on a built piece yet
  (the aboard walk's floor is the rooms and the ship's own structures). Open from that work:
  (1) ten of the game's request readers (camera, screen, ui, autopilot, remote_players,
  door_points, screenshot, cloudmap, cloud_profile_dump, record, cancel) still delete a request
  they find empty, so a hand-dropped `debug/screenshot_request.json` written non-atomically can
  be lost; the docs say "any content", so either take an empty file after a few frames or
  document `{}` (src/engine/ipc.rs, movie.rs) (DONE 2026-10-20, commit "Every dev request
  survives being read while it is still being written": all eleven readers, the showcase one
  included, take their file through `take_request_text`, which leaves an EMPTY file for 250 ms
  (`REQUEST_EMPTY_GRACE`, wall-clock time from the first sighting) and takes it the moment it
  has text; still empty after that, it is taken anyway: "any content" stays true for
  screenshot, cloudmap, cloud_profile_dump, door_points and cancel, while camera, screen, ui,
  autopilot, remote_players and record answer it as malformed JSON and showcase does nothing
  with it); (2) GAMEPLAY, the operator's call:
  on a planet Space is a climb at walking speed, so a 0.35 s tap lifts you about 2.6 m and the
  fall back sprains an ankle in Realistic (a real jump is about half a metre); (3)
  machine-guard.test.js "the real PowerShell lowers a real rustc" fails on a loaded machine (its
  15 s helper exits first), the BUG-152 class (DONE 2026-10-20: the helper lives until the test
  kills it, with a 10-minute ceiling). Found by it: the aboard walk can skip the stairs' 0.26 m top step at low frame rates
  (54 ms frames dropped 2.79 m instead of 3.00 m), so near a wound threshold the frame rate can
  decide which wound you get (DONE 2026-10-20, commit "A long frame walks every stair tread",
  BUG-207: a frame that would carry the body more than 0.20 m, shorter than any tread, is walked
  in steps no longer than that with the floor asked between them, so a slow frame stands on
  every tread and falls off the top from 3.00 m; bit for bit unchanged at 60 frames a second,
  walking or sprinting; the running game still to be re-checked by `just verify-falls`). Also
  found and fixed the same day (BUG-208, commit "The walk steps down stairs and leaves the floor
  at an edge"): walking DOWN the homestead's stairs had landed at 7.67 m/s, a 3 m fall and a
  sprained ankle in Realistic, since v0.1527.0, and Space mid-fall launched a jump. And the
  homestead's stairs, ladder and elevator climb a storey into a 3 m ceiling with no upper floor,
  so their tops are a fall: the upper floor is ship-homes increment 7, "stacked homes and
  decks" (docs/design/ship-homes-and-logistics.md), not patched here), and nothing spawns fires); a
  second mess hall along First Street (fold into increment 6); the backpack-overflow toast
  (DONE 2026-10-12: it names the item and says it went to home storage). Found with the
  2026-10-20 four: nothing burns diesel yet (data/electrical.ron's diesel
  generator has no machine; DONE 2026-10-20, commit "A diesel generator burns the refinery's
  diesel": generator_diesel in both homes' catalogs, a Kubota GL-7000 class set, 6.5 kW on
  2.6 L an hour, built at a workbench and placed from the Power tab, its drum taking only
  Diesel; no design doc named another consumer, and a diesel tractor or boat waits on vehicles
  burning fuel at all); food still gives a flat 10 Hydration for produce and 3 for the
  rest, and its satiation is per item whatever the item weighs (src/systems/food.rs
  `consume`), the same defect the drinks had (DONE 2026-10-20, commit "Food feeds and waters
  by what it weighs": each food's grams eaten in item_profiles.ron `portions`, its items.csv
  weight less USDA SR28's refuse; each profile's USDA water_g; Satiation at a day's 2,000 kcal
  for a day's hunger). Left from it: water and kcal are per profile, so a cucumber reads its
  group's 92% water, not its own 95% (per-item nutrition is its own step).
  Found in the v0.1527.0 seam fixes: 47 older website text colours in 16 files (4 chat
  stylesheets, 12 pages) are too faint on the light theme; `web-theme-light.test.js` check 7
  holds them in a per-file count (`OLDER_TEXT`) that can only shrink, so fixing them is
  shrinking that list to zero (DONE v0.1528.0: all 47 fixed, `OLDER_TEXT` deleted; left: check 7
  does not measure a page variable defined in another file, or one whose light value is itself a
  variable). Found with the heat pump and wood stove (v0.1528.0): the generators' fuel drum still
  accumulates in f32 (`ElectricalSystem::fuel_accum`, the frame-rate class fire.rs fixed on
  2026-10-20; DONE 2026-10-20: an f64 carry, where 240 fps had burned 0.4% less than the rate);
  a wood stove does not balance in a sealed hull (about 107 kg of oxygen a day at
  full burn against about 2,900 kg in a home's air, and no makeup air in either life-support
  mode: a design question, whether a ship home's stove needs an outside air feed or should be a
  planet-only heater; DONE 2026-10-20, commit "A wood stove takes its fire's air from outside
  through its kit": the dated finding docs/reference/findings/2026-10-20-stove-combustion-air.md
  quotes 24 CFR 3280.709(g), which admits a solid-fuel stove to a manufactured home only with "a
  combustion air inlet" that "shall conduct the air directly into the fire chamber", and Drolet's
  Escape 1800 manuals, which require its fresh air kit in a mobile home (AC01211 for the model on
  legs) and prefer room air only in an ordinary house that leaks; so `burns_fuel.combustion_air`
  is data, the shipped stove is `Outside` and takes none of the home's oxygen, the ship's supply
  ledger meters it on an "oxygen" line in kg in both modes, a `Room` stove keeps the old
  behaviour, and the Garden panel says which. Left from it: the ship's air beyond the home has
  no model, so nothing aboard runs short of what the kit draws; the fire's excess air and where
  its flue's exhaust goes aboard are not modelled; NFPA 211 was not read); data/hvac.ron's
  original heat pump and stove rows are unsourced.
- **A gap the Normal-mode default exposes (found 2026-10-05 by the Library sweep):** in
  Normal mode NOBODY CAN LEAVE THE SHIP. The Dev page's travel and Land buttons and F9 flight are
  the only ways off it, all Dev-only, and no vehicle or route goes down. So by default a
  player never meets rain, cold, a planet or building on one. Wanted: a real way down for
  Normal play (the design's transport ship in the hangar, a shuttle or a lander, with what
  it costs), or an honest note in the game until it exists. A design question for the
  operator (ship-homes design section 9 already pictures missions grouping at a transport
  ship in the hangar). PROPOSAL written 2026-10-05: docs/design/leaving-the-ship.md (a
  scheduled fleet shuttle to a field at Silverdale, 7 to 8 hours each way at 1x; twelve
  questions for the operator, each with a recommendation; a first increment of about a day).
- **Twelve plots merged (v0.1462.0): the far homes are far.** From p12's door to the mess
  hall is about 13 minutes on foot, against the design's rule (section 2.9) of a mess hall
  within 5 minutes of every home. Wanted: a second mess hall (or a transit stop) along
  First Street. Plots are on the west side only until a home can turn to face a street.
- **Operator choices to confirm when convenient:** the opening's first tool is a
  fishing rod (no fishing yet) and its useful thing a storage chest; death's 60 minutes
  of play before a pack is gone. Increment 5's four questions, answered with the plan's
  recommendations (2026-10-05): materials from a piece taken down go to whoever takes it
  down; a released plot's pieces come down with it; household permits end within 90 days,
  renewable, and the household page is a later increment 5b; `can_edit_ship` builds the
  ship's shared spaces through the server, whole-ship editing stays offline Dev, Admin has
  it by default. Two more from increment 5's review (2026-10-05; logged, nothing changed):
  (a) the caps: twelve plots at 512 pieces each (6,144) exceed the 4,096 a ship holds, so
  eight full plots fill the ship and stop everyone else building, even on their own plot,
  and identities cost nothing to make; recommended: a ship cap of at least the plots times
  the frame cap (6,144; 7,168 counting the two shared spaces), or a share of the ship's cap
  kept for each plot; (b) height and support: a piece may stand anywhere from 1 m below a
  plot's floor to 40 m above it although a plot is 3 m tall, and the relay does not check
  what a piece rests on, so a modified game can float walls about 37 m up; recommended:
  bound a piece's height by the plot's storeys (one 3 m storey today, about 3.4 m with a
  foundation and a roof) and have the relay check that each piece rests on the floor or on
  another piece. Also (2026-10-05): BUG-163 part 2, when an update changes a data file a
  player edited: A keep the edit (recommended for now), B replace it and keep the edit
  beside it, or C move the edit into a local mod; should crops' growth answer a room's
  temperature (recommended: yes in Realistic, no in the simplified mode; 17 crops' ranges
  exclude the rooms' 21 C, so heaters would matter); Antibiotics' trading-post price (base
  5 since BUG-145; the home-made recipe is gone, so it could rise).
- **FTL reopened (operator, 2026-10-05, a proposal):** FTL, with ships built to survive
  indefinitely if it is ever lost (docs/design/gravity-and-movement.md, the interstellar
  section). Recommended; awaiting his answer.

### THE FIRST HOUR: a basic starting loop with stakes (audit 2026-10-04)

The operator asked how the basic starting gameplay loop is going. A read-only
trace of a new player's first hour with stakes on (Normal play mode, progress
kept) is `docs/design/first-hour-audit-2026-10-04.md`: the mechanics mostly
exist, but out of the box there are no stakes, nothing teaches the game, and
several things break. Ranked:

- **Blockers (fix first):** B2 a player who typed a name is joined to the live
  shared world without being asked (72x clock: thirsty in about 20 minutes, no
  sleep; the ship has only two plots, so the third identity ever is a guest who
  cannot build); B3 Quit from the hub or the updater skips the save; B4 the Solo
  home has no smelter, so the first quest cannot finish; B5 quests that can
  never finish (Travel steps read a position walking never moves; ore_sample_0
  and rare_ore_0 come only from creatures never spawned); B1 the default "fresh
  home" setting wipes progress every launch (a default, see the decision below).
- **Friction:** F1 nothing teaches I, E, F1 or Alt; F2 the vendor, privacy and
  machine-card windows need Alt held and nothing says so; F3 the quest text
  points at a Dev-only button, completions are silent, the HUD goes blank after
  Toolsmith; F4 machines eat the backpack (BUG-150, fixed v0.1459.0); F5 the
  bedroom's bed says "sleep here" but cannot be slept in; F7 nothing planted
  ripens in a session; F8 food poisoning can kill with no cure and no word;
  "Keep mining" traps the drone in endless empty trips with no Stop; logs are
  refused as "pack is full" with 47 L free (each log is 26 L); building with
  too few materials does nothing in the world and says why only on the
  Crafting page; the smelter's fuel choice is lost on every load; the
  character picker's hint ("Gear and skills stay in the world you earn them
  in") is false.
- **Missing stakes:** S1 quitting heals and refills (vitals never saved); S2
  death costs nothing; the B editor places any machine for free even in
  Normal, and writes it into the data files rather than the save; S4 the free showcase garden replants itself in every
  mode; S6 stored food never spoils; S7 animals have no needs; S8 the medical
  system is never registered (FIXED 2026-10-12: wounds and healing by itself, one owner in
  FoodSystem's body pass; the unregistered MedicalSystem and data/medical.ron deleted).
- **Operator decisions, ANSWERED 2026-10-04 (late evening):** "I like your
  suggestions. For normal mode, especially for my MMO server, let's have
  everything be real time, not the 72x. That way anyone joining isn't dealing
  with accelerated death. We'll wait until we have everything actually working
  before we accelerate everything for fast mode." So:
  1. **Defaults flip now:** fresh installs start in Normal mode with progress
     kept ("Start every session from the default home" off); the rigs pin Dev;
     the operator's own saved settings keep Dev. (After the session-flow and
     world-friction lanes merge: they touch the same config and Normal-mode
     code.)
  2. **Death:** Simplified mode keeps "Nothing was lost"; in Realistic mode the
     carried items stay where you fell for a while, to go back for. (After the
     survival lane merges.)
  3. **The first ten minutes:** check your vitals, eat, plant, craft a tool,
     send the drone for iron, smelt it, build one thing, and end at your front
     door looking out on the Commons. (After the quests lane merges.)
  4. **More plots:** about ten along First Street, so more than two people can
     have homes. (Started at once: data and relay, no overlap with the lanes.)
  5. **Real time:** the shared world's clock defaults to 1x, not 72x (the solo
     game already defaults to 1x); a faster "fast mode" waits until everything
     works. (Started at once; the live server's stored 72 is set to 1 after the
     deploy.)

### IN FLIGHT AT THE USAGE CAP (2026-09-27 evening): resume from these branches

Weekly usage reached 89%, so every agent was told to commit (WIP if not
done, with a message saying what is left) and stop. Each branch is
`worktree-agent-<id>` in `.claude/worktrees/agent-<id>`. Merge a finished
one the way today's were merged (preview with `git merge-tree`, cherry-pick
or merge, verify, boot, release); resume a WIP one by giving a new agent its
branch and its commit message. Remove a line here once its branch is merged.

State after the wrap-up (merged ones are removed: the Wi-Fi harm removal,
saved machine levels and the electroculture findings landed in v0.1394.0;
the one game clock in v0.1395.0; the ship reactor feed in v0.1396.0; the
Drying and Fermenting guides, verified, with the jerky recipe loop fixed
(jerky and dried meat now start from mutton), in v0.1396.1; the HDR scene
target increments 3 and 4 in v0.1398.0):

- None left. The last one, the sun cascades branch `a2932acea7b338a59`,
  merged in v0.1397.0 (increment 0); increment 1 is drafted as text in
  docs/design/sun-cascades-wip/ and never built, tracked under TIER 0 item
  3c. (Removed 2026-10-02: the week-plan survey found this line still here.)

### 0. The ship and the playable game (arc C, ranked first 2026-09-27)

Spaceship first (Blocked #2, answered 2026-09-27): the ship's life support,
the garden, building and survival come before the rendering items below.
The work list is arc C in "Fenced arcs"; the next items there, from the
operator's answers of 2026-09-27, all SHIPPED: saved battery and other
stored levels (v0.1394.0), the 24-hour configurable day (v0.1395.0, Blocked
#3), and the ship reactor as the homes' metered power (v0.1396.0, Blocked
#3b). The Wi-Fi crop harm was removed on 2026-09-27. Next in arc C is the
first unticked item in its "Fenced arcs" list.
Rendering items below still run beside it on files that do not overlap.

**Economy and crafting follow-ups (2026-10-04, from BUG-143/145):** the
trading post now sells only real items (v0.1453.0), no recipe resells for more
than its inputs cost at standard grade, and vehicles carry real bills of
materials (v0.1455.0), and a hand craft draws on the home's storage
(BUG-147, v0.1457.0). Still to do from that: the right station per vehicle
class (everything is built at `workbench_0`), and the `assemble_*` kit
recipes given real bills of materials too. BUG-146 (a better craft grade
looped at the vendor) is fixed, merging in v0.1463.0: a grade's price is
capped by what its parts cost at the post (`src/systems/economy/parts.rs`);
a price that falls with the post's stock was not enough on its own, since
the first sales still paid in full, but could sit on top later. Smaller:
`craft_optical_fiber` makes copper wire (no optical fibre item), sterile
bandages yield 8 against 5 plain, several item weights differ between
items.csv and trade_goods.ron, and herbal_tea_0 and brass_key_0 are named
by data but are not items.

**Game gaps the Library fact checks found (2026-10-04),** each verified in
the code by the guide writers under the default settings: health and vitals
are never saved; `data/medical.ron` is never applied (FIXED 2026-10-12: deleted with its
system; wounds live in data/medical/wounds.ron); `data/species.json` and `data/geology.ron` are not
read by anything; farm animals have no illness, feed or water needs and do not
breed; nothing in the game makes Scrap Metal unless the Dev page spawns its
robots; the water pump card said 12 L/min (fixed v0.1452.1). Also: the
groundwater model in `hydrology.rs` is not running, so the well pump has no
aquifer behind it; the wind turbine makes a constant 4.4 W with no wind
model, and there is no water power; only food carried in an inventory
spoils (the Freezer and pantry do not age food, the food data's temperature
zones are unused, and the spoilage clock is not saved between launches).
And: cable sizing (`cheapest_cable_for`) falls back to a superconductor, a
material that does not exist, whenever no copper cable passes, so a long
heavy power run "passes" (3,000 W over 60 m picks it), and the Construction
page offers "Upgrade N power run(s) to superconductor" today; the lightning
code in `disasters.rs` is never registered, so it never runs. The heat, fire
and fuel guides' check (same night) added BUG-153 (the Campfire ability
promises a fire with warmth and light, and only heals 3), BUG-154 (the backup
generator runs on Paint, Glue or Crude Oil) and BUG-155 (the greenhouse quest
asks for a heater that does nothing).

### 1. Environment regions: the rest of the arc BUG-080 opened

Weather has been a place since v0.1330.0 (BUG-080,
`docs/design/environment-fields.md`): the condition rides an environment region
with a position and a radius, and nothing in the cloud path reads camera
altitude. The aurora was the region buffer's second consumer (v0.1331.0) and
has had its own additive pass after the cloud composite since v0.1382.0, so the
per-pipeline `override` switch this list once asked for no longer applies:
`aurora_emission` has one call site, pinned by a test.

Remaining, in order:

1. **Disasters through the buffer.** `disasters.rs` already stores position,
   radius and intensity and nothing outside that file reads them, so a wildfire
   is invisible. Needs a consumer to be worth anything.
2. **The bake gap.** The sun-shadow cache and profile bakes in
   `45-cloud-temporal.wgsl` read the base coverage and do not apply the region
   floor, so a storm lights and self-shadows as though it were not there. A
   second-order error against a first-order fix; wants its own measurement.
3. **Layer 1: BUILT 2026-09-27** (design doc, "Layer 1 as built"). Air
   temperature, pressure and prevailing wind at any place and date, per world
   from `data/environment/climate.ron` (Earth fitted to the NCEP/NCAR 1991-2020
   reanalysis by `scripts/climate-fit.js`, with land versus sea seasons; Mars's
   column from the NASA fact sheet), in `systems/env_layer1.rs` and a WGSL twin
   in `00-bindings-vertex.wgsl` that a test RUNS against the CPU copy. The body
   heat model now feels the air where the player stands: temperature, pressure
   and wind at the player all come from it, with the weather as the deviation.
   Rain versus snow and the HUD readout are BUILT (2026-09-27, design doc
   "Rain or snow, decided by the air": the air where it falls picks the phase by
   Jennings et al. 2018's model, every precipitation reader shares it, and the
   HUD prints the temperature, wind and phase at the player).
   Next consumers, in order: **cloud advection** (the first GPU caller:
   `env_l1_wind_body` once per ray, `EnvClimate` as a uniform); the sea state
   reading the wind at the player (DONE 2026-09-28, `Weather::sea_state_target`);
   field crops and water bodies sampling the
   climate at THEIR positions; weighting the weather's deviation by its
   region's influence at the player. Open data gaps: Mars by latitude and season, the
   Moon (a sunlight function of local solar time).

Aurora follow-ups, none urgent: the red cap is a look change for the operator
(the layer top is data, `region_kinds.ron` params[3]); the Low cloud tier seen
from the ground is right by construction but was never confirmed on a capture;
about 1.3 ms with the oval filling the screen is the curtain shading itself, and
a half-resolution pass is the lever if that matters.

Still unreproduced: shores glowing at a grazing view while the sun is visible.
(The clouds glistening at the dusk line is item 2a.)

**Do not re-propose:** a weight that reads camera altitude, distance, or how
much of the planet is on screen. That is the defect class, not a tuning knob.

### 2. Clouds: what remains after the sheets and the brightness fix (was items 2-0 and 2)

Shipped, each with its full record in the archive: procedural placement as a
fraction instead of a sheet (v0.1333.0), synoptic storms and fronts
(`cloud_synoptic_warp`, v0.1334.0), the High-tier ball pit (`CLOUD_CELL_SPLIT`
0.05, v0.1335.0), the repeating shapes (a fixed rotation per tiled noise tap)
and the multiple-scattering gain calibrated against the Sahara (1.8), both
v0.1337.0, and the deck lit across each march step instead of at a sample about
0.9 km inside it (`CLOUD_STEP_LIGHT`, v0.1384.0; environment-program increment
10c).

Remaining, in order (the sheets plan first, then what the brightness fix left):

1. **Bake the weather field** into the existing `weather_map` texture at low
   cadence, so richer structure costs nothing at march time.
2. **Climatology and type mix.** The type coordinate clusters around 0.5, the
   cumulonimbus centre, so about 38% of the planet draws as cumulonimbus against
   about 1% deep convection on Earth; stratocumulus, the commonest real type, is
   about 0.3%. Equalise the coordinate and retune the centres, or derive type
   from the storm structure. The Rust `cloud_regime` mirror and its tests change
   in step.
3. **Cellular texture** (Worley 15-40 km cells for flat decks). Measure the
   deck's column optical depth first: texture only shows below roughly 30.
4. **The brightness residuals** (increment 10c): (a) Ultra's +2 ms, next lever a
   step budget weighted by eye transmittance; (b) a +5% residual against the
   economy-off march, suspects the trapezoid view depth on a skirt-to-core step
   and the ambient's weight; (c) re-derive the gain default (1.8,
   `src/gui/mod.rs`) at the in-atmosphere vantages before touching it; (d)
   `cloudlum-2000-high-noms-eco0` and `cloudlum-under-*` are sweep-order
   sensitive, so compare them only within one sweep.
5. **Measure before raising:** the powder term at a noon down-look on High (the
   review's replica overstated its other two causes by 3x or more); crevice
   darkening `CLOUD_PUFF_AO`; built bodies as the High default
   (environment-program increment 16, never done).

Tighter comma heads are a two-constant change (more twist, a smaller radius):
ask the operator before winding them tighter.

**The ocean, found on the way.** The glint stripes from 55 km are FIXED
(BUG-101, 2026-09-27: the coarse crest warp now runs wherever a wave train is
drawn). The rectangular blocks in the open-ocean colour are FIXED too (BUG-103,
2026-09-27): value-noise lattice seams, because neighbouring cells reached a
shared corner by float routes the compiler rounded apart. `value_noise` now
hashes the INTEGER lattice point (`lattice_hash`, twin
`renderer::lattice_noise`, bit-identical on the GPU); measured 3.34 to 0.83 at
55 km and 1.68 to 0.82 at 150 km on `scripts/lattice-seam-metric.mjs`, no
measurable cost. Every `value_noise` pattern re-rolled (sea colour, shore,
surf, crest warp, land detail, materials); voronoi, the cloud lattice and the
ground micro noise kept their patterns on integer corners. A seam on the
Bahama Bank shallows went with it.

**Do not retry:** base resolution for the sheets (raised twice, the complaint
returned both times); for the ball pit, the cell split fully off, gated to the
upper band, or moved into the water term alone; gating `reg.tint` off under
multiple scattering (measured 2026-09-27: it lifts a cumulonimbus base seen from
0.5 km by 29%, undoing the v0.909 storm darkness; a tops-only gate is the option
if the 2% top inversion ever matters).

Fixtures: `cloudgrey-*`, `decklum-*`, `deck-55-*` and `cloudlum-*`, each
carrying its measured result in its own `desc`.

### 2a. The grain at the dusk line: keep or revert the animated jitter (the operator's call)

The clouds' grain is worst in the twilight band, not at noon: 73x the
full-daylight figure at the dusk line on the old metric
(`orbit-terminator-3000km`), which is where the operator saw them "glistening".
Every grain ranking taken at noon was taken in the easiest regime, and the
ranking inverts between the two.

What ships: the depth jitter has been ANIMATED since v0.1331.18
(`45-cloud-temporal.wgsl`, "UN-FROZEN AGAIN"), which gives the temporal filter
something to average. The operator chose FROZEN in v0.1253.2 after his own
on/off experiment at a close camera, before either number below existed.

The trade, on the blur-proof measure (`just terminator-grain`, 2026-09-27; the
old metric could be won by a box blur). Figures on today's scene in brackets:

| | frozen | animated |
| --- | --- | --- |
| GRAIN at the dusk line, ~15 px scale | 13.9% (15.7%) | 7.0% (6.3%): 2.0x (2.47x) less, real detail kept |
| fizz between settled frames, region | 0.34% (0.33%) | 2.09% (2.40%) |
| fizz, band 5 alone | 1.73% (1.49%) | 10.1% (9.93%) |

**The question, in one line:** is that parked-frame fizz an acceptable price for
half the grain at the dusk line? Keeping it is the status quo. Reverting is one
line in `45-cloud-temporal.wgsl` (the comment there names it), and the fallback
once offered for a "no" does not hold: no setting of the spatial filter lowers
the visible grain at the dusk line (forced to full it reads "same" at band 5,
and 1.12x at best in bands 3 and 4). Not flipped either way unilaterally: it is
a look decision he made with his own eyes.

Settled, do not re-propose (full record in the archive): the still-frame crumb
is the density field's own structure, so raising `cloud_res` makes it worse;
the dither, convergence, the upsample, coverage sampling, the sun-shadow cache,
ambient shaping, the accumulator and the per-pixel cone azimuth were each
refuted by a built arm; the sun ladder on the unjittered grid costs 8% of the
brightness for little; widening the spatial gate or strength does nothing at
the dusk line. Rank any new candidate across bands 3 to 5, never at noon, and
positive-control an arm before believing a null. The measure judges a STILL:
motion, where the operator's "boiling" lives and the temporal filter cannot
converge, is untested.

### 3b. 8-bit banding everywhere: CLOSED v0.1398.0 (the HDR target and one dither); increment 5 next

Found 2026-09-24 while fixing the aurora. The scene renders straight into the
8-bit sRGB surface format (`renderer/mod.rs`, `surface_format` picked by
`is_srgb()`, and `create_scene_texture` uses it too), each pass tonemaps in
its own shader, and **nothing dithers before the 8-bit write.** A slow dark
gradient therefore quantises into flat rings one display level apart, and the
eye reads each ring as a hard edge. The aurora's faint diffuse glow spanned
three or four levels and drew as nested ellipses with crisp outlines; rendered
alone at full strength the rings multiplied into a contour map.

v0.1331.21 dithers the AURORA's own output (`srgb_dither` in
`30-atmosphere.wgsl`, triangular noise of one 8-bit step converted to linear
at the pixel's value). Every other dark gradient is still exposed: the night
sky, the atmosphere's limb and twilight falloff, dusk terrain, fog. Expect
more "harsh edges between shades" reports from those until this lands.

**The real fix** is the one every modern renderer uses: render the scene into
an `Rgba16Float` target, keep radiance linear and unclamped through every
pass, and do ONE tonemap and ONE dither in a final pass to the surface. It
touches every pass that currently writes `surface_format` (the scene texture,
bloom, godrays, SSAO, the cloud composite, the celestial passes) and every
shader that tonemaps inline, so it is an arc, not an increment. Until then,
`srgb_dither` is the stopgap to reach for on any surface that gets reported.

**Planned 2026-09-27:** `docs/design/hdr-scene-target.md` is the build order
(every writer of the scene with its blend and range, every inline tonemap and
what it becomes, the capture paths, and six bootable increments; increment 4,
one dither in a final pass, closes the banding report). Estimated cost 0.2 to
0.4 ms at 1600x900.

**Increments 1 and 2 built 2026-09-27** (design doc section 6): every scene
pass draws into a scene target (`renderer/scene_target.rs`) and one present
pass copies it to the display; camera screens and the hi-res capture go
through a view scratch and the same pass. Still 8-bit, so bit-exact: a GPU
test round-trips every code of every channel, and the 3840x2160 capture at
console-face-3 is byte-identical between the old and new builds. `gpu.present`
measures 0.05 to 0.07 ms at 2560x1387. `tests/scene_format_lint.rs` keeps every
scene PSO on `scene_format()`. **Next: increment 3** (`scene_format_for` returns
`Rgba16Float`, clamp flag on), then increment 4, the one dither.

**Increments 3 and 4 built (2026-09-27) and measured (2026-09-28), merged in
v0.1398.0: the banding report is CLOSED.** The scene target is `Rgba16Float`,
and ONE triangular dither in the present pass replaces the aurora's own and
`srgb_dither`. Same-boot A/B (design doc section 7): the float target moves
still scenes by at most 2 codes; the dither cuts the mean flat run in the dark
parts of every banding vantage (aurora-over-land-dark 7.1 to 2.3, night-horizon
4.8 to 1.8, shore-dawn 2.6 to 1.7) with means and the aurora comb held; about
0.03 to 0.06 ms at 2560x1387 against an estimate of 0.2 to 0.4. The
high-frequency gates' 35 vantages pin `present_dither: "0"`. **Next:
increment 5**, linear radiance behind a runtime `hdr_linear` flag.

### 3c. The ship's rooms: no bounce light, and the sun shines through shelves (2026-09-27)

A read-only lighting review measured it at 25b-mushroom-racks: interiors get
NO indirect light (the whole term is the 0.005 silhouette floor), so walls sit
at 2.5% of the floor where interreflection predicts about 25 to 30%; and the
home never casts into the sun shadow map, so the lowest shelf under three
others reads 130 against the open floor's 133. Two arcs, in this order:
1. **Room GI**: per-room DDGI irradiance probes. **Rung 1 BUILT 2026-09-27,
   merged in v0.1390.0**: probes traced against each
   room's own box, `docs/design/room-gi.md`. At 25b the wall goes from 4 to 46
   sRGB (wall/floor 0.17 before the tone map, target 0.15 to 0.40); at the new
   `25c-oyster-rack-close` the shelf underside goes from 2 to 76 and the oyster
   block front from 43 to 110; cost at console-face-6 about 0.6 ms (update
   0.21, sampling about 0.4), panics 0. Dev switch `showcase {"room_gi":"0"}`.
   NEXT after the cascades: rung 2, tracing each room's contents (a voxel
   volume per room, DDGI relocation and classification, the per-room
   visibility flag on), then portals for glass walls and doorways.
2. **Near sun cascades** with the home as a caster:
   `docs/design/sun-cascades.md` (camera-centred clipmap cascades C0 to C3 in
   an atlas beside the existing far map, no bind group layout change; five
   increments). Room GI has landed; increment 0 (the fixtures) merged in
   v0.1397.0, and increment 1 (the atlas and C1) is drafted in
   `docs/design/sun-cascades-wip/`, not built.

### 4. The far-rung gates, G0(d) and G1 to G7

Unchanged, and still the plan for the deeper cloud work. The increment is merged
behind knob 0; the gates are what turn it on. Design of record:
`docs/design/cloud-far-rung.md` (v2, after the v1 contract failed its adversarial
critique on eight real blockers). The measured target it exists to fix: at 873 km
Ultra renders about 0.9 percent coverage against High's 31, because one sample
per ray misses a 300 m layer vertically.

---

## Blocked on the operator

Not AI work. Listed so a session knows to route around them rather than pick
them up.

1. **Release signing happens on the operator's own schedule.** Recorded here
   only so a session understands why the desktop updater may be offering
   nothing: it trusts signed releases only, and an ineligible one is invisible
   rather than an error. **Do not raise this with the operator** - standing
   rule, CLAUDE.md "Release signing is the operator's to raise, never yours".
   He signs when he decides to; `docs/admin/release-signing.md` is there if he
   asks.
2. **ANSWERED 2026-09-27: the spaceship first, planets after.** Operator: "We
   should focus on the space ship first but, we do need to get farming working
   on the planet too. I imagine the spaceship gardens are a great way to figure
   out all the physics, like the gasses, liquids, etc. to properly account for
   things. Then going to Earth or some other planet might simplify some things
   and complicate others." The ship's closed loops it pointed at are BUILT
   (v0.1377.0 to v0.1384.0, docs/design/ship-life-support.md): greenhouse
   moisture condensed back to the tanks, and CO2 from people and mushrooms and
   O2 from plants as a mass balance through the ship's air. Crop light from the
   ship's real sun direction (the BUG-090 fix) shipped 2026-09-28. Seasons and
   ground farming (Silverdale in data/home_outline.json) come after.
   **Follow-up (asked 2026-09-27):** TIER 0 is all rendering, while the
   spaceship-first answer sent the day's work to arc C (the playable game and
   ship life support). Should arc C rank against TIER 0, or stay a fenced arc
   worked beside it? The operator asked what this meant (2026-09-27 evening);
   explained, with the AI's call to put arc C at the top of TIER 0 to match
   spaceship-first unless he says otherwise.
3. **ANSWERED 2026-09-27: a 24-hour day by default, configurable, with an
   hour that stays an hour.** Operator, verbatim: "The default day length
   should be 24 hours. Though we want it to be configurable. Like, maybe prefer
   a 20 hour day or 36 hour day. However that shouldn't change how long an hour
   is unless they change the setting that makes stuff happen faster/slower."
   So: hours per day is a setting (default 24), the length of an hour is fixed,
   and only the separate time-speed setting makes things run faster or slower.
   **Built (2026-09-27):** one game clock, Settings > Gameplay > Time (time
   speed default 1, hours in a day default 24, days in a year default 365,
   the last the agent's choice); decision-briefs.md Brief 6 says how, and
   proposes 72x as the simplified mode's default. The question as it was asked:
   **One clock or two, for the body and the garden (asked 2026-09-26).** The body
   runs on real seconds, the garden on 20-minute game days at 10x growth, so
   urine is a fraction of a percent of the garden's nitrogen in play, and room
   air (game hours) and tank water (real days) are 72x apart (BUG-092 item 7).
   Choosing one clock, or a slower garden, is a design choice for the
   full-realism and simplified modes.
   **Laid out as one question with a recommendation on 2026-09-27:**
   docs/design/decision-briefs.md Brief 6 ("How long is a day?").
3b. **Ship and garden decisions raised overnight 2026-09-27** (each with its
   evidence in the doc named; none blocks other work). The three energy
   bullets were merged on 2026-09-27 onto the v0.1384.0 meter's figures; the
   earlier wording is in docs/history/priorities-archive-2026-09-20-to-27.md.
   - **ANSWERED 2026-09-27 (energy).** Operator, verbatim: "For now we can
     base power budget on nuclear reactors and then players can build solar
     and other means of producing electricity. We could track user resource
     usage but, essentially provide unlimited (at least to start) then we
     could figure out how to track a whole fleets worth of supplies because,
     that'll be important once the MMORPG storyline begins once the game is
     ready for release in a couple years." So: the ship's reactor supplies the
     homes, effectively unlimited to start and metered; players can build
     solar and other generation; fleet-wide supply tracking comes later with
     the MMORPG storyline. BUILT 2026-09-27 (docs/design/ship-life-support.md
     section 8): in the default Station-supplied mode every home power island
     is tied to one KLT-40S-class reactor (35 MWe, trade-press figure), used
     after the home's own generation and batteries, so nothing sheds; every
     watt-hour drawn and returned is metered per home in f64 and saved
     (`systems::ship_power`, WorldSave `ship_supply`), shown on the Home card,
     the HUD and the Usage meter. Realistic runs on the home's own generation
     and batteries. A Solar Panel blueprint (400 W) offsets the draw one for
     one aboard and powers a planet site, which the reactor never reaches. The
     fleet ledger is next (section 8), not built. The energy questions below
     are superseded by this; kept for the record.
   - **Energy: neither home's budget closes, in either mode**
     (docs/design/ship-life-support.md section 7). Each home now counts what it
     makes at its site (NREL PVWatts for Silverdale: 1.09 kWh a panel a day,
     0.51 in January; the wind turbine 0.11 kWh a day at 2.8% capacity; the
     generator shown apart as a backstop). Family: makes about 11.0 kWh a day
     against 24.0 Station-supplied (13.0 short, about 12 more panels) and 61.8
     Realistic. Solo: 4.45 against 8.6 and 32.8. The questions:
     - How to close the Realistic loop: the home's air at 60%, tighter
       bulkheads between the grow rooms and the home, more panels, or leave it
       to Station-supplied.
     - How to close Station-supplied, now that the water heater, washer and
       tower pumps charge their real daily energy (EIA RECS 2020): more panels,
       a heat-pump water heater, or accept it?
     - Which SITE the loops are for, station or ground (the BUG-090 decision).
     - Whether PLAY should match the meter: each panel still makes about 3.1
       kWh a day in the live sim, 2.8x the site; batteries carry load at night
       since v0.1384.0, but the homes would still brown out.
     - The water heater at the US survey average or at the home's own hot
       water plus standby loss; the outline's conservative December solar; and
       whether to keep a wind turbine that makes 0.11 kWh a day at this site.
   - Whether the homes carry an oxygen electrolyser (about 2.6 kWh a day
     family, 1.3 solo) to close the oxygen shortfall.
   - The family home's mushrooms once its air settles near 675 ppm: a loop
     of their own that hands their CO2 to the greenhouse, fewer racks,
     bigger tent humidifiers, or accept up to about 9% loss
     (ship-life-support.md section 7).
   - Whether the stale-air harm to mushrooms follows the garden's Off /
     Gentle / Realistic setting (as built) or the Ship life support mode, and
     whether a 900 ppm tent setpoint (the humidifiers cannot hold 90% at
     Cornell's 800 in the family home) is acceptable.
   - How to close the Food loops (91% family, 81% solo, since the mushroom
     yields were sourced): more beds, or more blocks per tent.
   - **ANSWERED 2026-09-27: no Wi-Fi crop harm. REMOVED the same day.** Operator:
     "We'll assume no wi-fi crop harm at this time." The FarmingSystem's RF
     drain, the `RfEmitter` component, the machine `rf_emission` field and the
     Wi-Fi-harms-a-grow buildability warning are gone; the router stays as a
     network device, and a test holds that a powered router leaves crop health
     unchanged. He also mentioned
     farmers using copper coils to increase growth (electroculture); a dated
     findings document is being researched before anything goes into the sim.
     The question as it was asked: the v0.620 Wi-Fi crop harm: keep, scale to
     the evidence, make it a setting, or remove
     (docs/reference/findings/2026-09-27-wifi-and-plants.md; no source shows a
     household router harming a garden at the distances plants sit from one).
4. **Clear the old agent worktrees** under `.claude/worktrees/`. Audited
   2026-08-04: none could be cheaply proven redundant, and
   `just clean-worktrees` force-deletes branches and has destroyed
   review-approved work before. Operator-only by standing rule.
5. **Two gameplay questions** from
   `docs/design/playable-assessment-2026-09-19.md` section 7. **Crop growth speed
   is ANSWERED (2026-09-20)**: a growth multiplier separate from the world clock,
   1x / 10x / 100x plus a custom value, shipping at 10x, implemented and tested;
   Tier A item 3 is unblocked. Offline progression was answered on 2026-09-21 as well, and is broader than crops (a toggle on all three modes, applied to crafting too); rung 1 (crops, builds and craft batches, single player) was BUILT on 2026-09-25 and rung 2 (automated machines, the drone, livestock) on 2026-09-27 (v0.1387.0), see `docs/design/offline-progression.md`. Still
   open: what the first ten minutes are. (Pipes ANSWERED 2026-10-04: the real
   material, including paint, made identifiable with colour, labels and stripes;
   research the existing marking standards first. Research DONE
   (`docs/reference/findings/2026-10-04-pipe-marking-standards.md`) and
   increment 1 SHIPPED v0.1454.0: material bodies from
   `data/piping/pipe_materials.ron`, ISO 14726 marker bands from
   `data/piping/marking_schemes.ron`, Simplified/Full in Settings, content taken
   from the machine a line leaves. Next increments: on-pipe text labels and flow
   arrows (ISO 20560-1), GHS pictograms, paint and insulation as a player
   finish, plastics and steels in `ConductorMaterial`, markers at valves,
   branches and wall crossings, separate service heights so different runs do
   not sit inside each other, and checking our own middle bands against the
   printed ISO 14726 Table 3. The data row `pex_half` in
   `data/utilities/conduits.ron` is a potable PEX line recorded as copper, which
   contradicts the copper-only-for-drinking-water rule in `src/ship/conduits.rs`
   (2026-06-25); it goes with the plastics increment.) A third, lower: does multiplayer enforce
   anything, or is it co-operative trust until launch. NOTE that the report's
   question 2 ("do the 3D models ship with the release") is ANSWERED: they do,
   since v0.1322.0.
6. **The two demoted lethal Library guides**
   (`/library#making-water-safe-to-drink`, `/library#keeping-what-you-grew`)
   stay at `sourced` until a human has read them, per the rule the operator
   chose. `curriculum-status.js` enforces it.
   **Added 2026-09-24:** the fire-performance guides on the lethal topic
   `materials_fire_staff` (`/library#fire-staff-materials`, updated, plus the
   new `/library#staff-tubes`, `/library#fire-performance-fuels` and
   `/library#fire-performance-clothing`) are at `sourced` for the same reason.
7. **GitHub branch and tag protection on `main`: NOT for now (operator,
   2026-10-04):** immediate deploys to the live relay suit the dev cycle.
8. **ANSWERED 2026-10-04: donations copy.** The operator: a tax-deductible
   donation goes to the nonprofit at https://www.sponsor-a-can.org/donate/ ; a
   gift to him directly (he receives it, not tax-deductible) goes to
   https://www.patreon.com/Shaostoul "for the time being". Done the same day:
   both are the two "ways to give" cards at the top of the Donate page on both
   clients (`data/donate/routes.json`), each with one sentence saying where the
   money goes and whether it is tax-deductible, and in the FAQ, README, website
   docs and outreach guides. Sponsor-a-Can's 501(c)(3) status checked against
   IRS data: `docs/reference/findings/2026-10-04-sponsor-a-can-tax-status.md`.
   Patreon is the first card, because the page is "Support HumanityOS" and the
   "Fund the work" buttons open it. Still the operator's calls: (a) whether the
   PayPal, Cash App and GitHub Sponsors links he added on 2026-07-13 stay listed
   as "more ways to give directly" now that he named Patreon as the direct link;
   (b) the server's funding goal, "$100,000, Full-time development for 1 year"
   in `data/server-config.json`, which both clients show under the direct-gifts
   heading: it dates from v0.25.0 (2026-03-21) and nothing in the repo shows he
   set or confirmed it (keep, change, or `goal_usd: 0` to hide it; there is no
   in-app editor yet, see "Boot-default config editor" in
   `data/admin/ops_registry.json` planned); (c) the funders guide names his Vice
   President role at Sponsor-a-Can but lists that income in its books only as
   "side-job", so whether to say the side-job is Sponsor-a-Can is his. Known
   gap, not Donate's own: the desktop updater swaps only the exe, so an
   auto-updated install keeps its old `data/` (old FAQ, no route cards) and
   only a fresh download gets new data files.
9. **Landing screen 2 hero shot: direction ANSWERED 2026-10-04.** No ship worth
   showing yet, so: start dark on the Milky Way, then a sunrise over a scenic
   place, e.g. from Silverdale, WA toward Mount Rainier. The rig can frame it.
10. **ANSWERED 2026-09-29: keep the self-hosted git mirror.** Reinstalled
   the same day as a pull mirror of GitHub (Forgejo v16.0.5, syncs every 8
   hours by itself; `docs/admin/forgejo-setup.md`). The operator made the
   admin account through an SSH tunnel and set up the mirror.
11. **OPEN, decide before 1 January 2027: is the Tools page's Install button a "covered
   application store" under California's AB 1856?** The act defines one as a program that
   "distributes and facilitates the download of applications from third-party developers"
   and from that date gives it duties (asking the operating system for an age signal and
   passing it to developers). A Tools page that only links distributes nothing; one that runs
   winget facilitates a download, and whether it also "distributes" is not answered by the
   text. Settled by a lawyer, guidance from the California Attorney General, or asking the
   bill author's office. Until decided the button is fine to ship (the act is not in force).
   Detail: `docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md`, 2.10.
   Told to the operator once, 2026-10-19.
12. **DONE 2026-10-22: the file edited and tested by the AI (backup
   `/root/nginx-humanity.bak-20261022`), the reload run by the operator; `/files/<n>` now reaches
   the relay's item page (checked: its og:title).** Was: one
   edit on the VPS after v0.1524.0 (Media renamed Files), 2026-10-20. The AI's
   remote write was refused by the session's safety classifier, so this waits for the operator:
   in `/etc/nginx/sites-enabled/humanity`, change the line `location ~ ^/media/[0-9]+$ {` to
   `location ~ ^/files/[0-9]+$ {` (the block is otherwise the same), then `sudo nginx -t && sudo
   systemctl reload nginx`; until then an item's own address (`/files/<number>`, its link-preview
   page) does not reach the relay. A copy of the config as it was is at
   `/root/nginx-humanity.bak-20261020`. The retired pages (`tools.html`, `media.html`,
   `shared-files.html`, their scripts and `shared/media.css`) no longer need moving by hand:
   `scripts/sync-web-root.sh` removes them on the next deploy (v0.1527.0). Detail:
   `docs/admin/SELF-HOSTING.md`.

---

## Fenced arcs

**Multiplayer comes before arc A (operator, 2026-10-04):** "Let's keep working on multiplayer. The screens are essentially functional but, not fully. They're not as important as getting the game playable at the moment."

**Arc A (in-world screens) is the one the operator picked, 2026-09-20**, when
asked which should come up next after the cloud work. Take arc A work ahead of
the rest of this section. The others remain unranked against each other and
against TIER 0: that ordering is the operator's call, and asking for it is
cheaper than guessing.

### A. In-world screens, the remaining rungs. PICKED (operator, 2026-09-20)

Six rungs merged (v0.1313 to v0.1314): the ScreenSurface, the screen as machine
data, the live feed and in-game camera, the console room, the video player, and
the readable web. Design in `docs/design/in-world-screens.md`,
`docs/design/readable-web.md`, `docs/design/media-player.md`.

Remaining, in the order they were fenced:

- ~~A placement gate on `embed.status`~~ BUILT 2026-09-25: a forbidden site
  is refused before it is fetched on the Browser page and every wall, an
  unreviewed one carries a note (`docs/design/readable-web.md`). **All 34
  records DECIDED 2026-09-25 by the operator** from
  `docs/reference/findings/2026-09-25-site-embed-terms.md`: 21 allowed, each
  with the credit line its licence asks for (`embed.attribution`, drawn on
  every page; the checker requires it for an allowed third-party site), seven
  forbidden (Project Gutenberg, Khan Academy, Instructables, Coursera,
  Discord, GOG, Examine), four unknown (no terms address it), OpenFarm and
  the dead ISS tracker link removed. Asking GOG and others for permission is
  the operator's to send; the contact routes are being gathered.
- ~~The screens' share of the interior frame (the console room at 9 fps)~~
  STALE, corrected 2026-09-25: that figure predates the P2/P3 megashader split.
  Measured after P3 (`docs/design/frame-cost-arc.md`, the phase-B table):
  `console-face-6` 29.8 fps at the 30 fps vsync cap, `gpu.scene` 14.16 ms,
  `gpu.transparent` 2.45; `console-face-3` (camera wall in view) 11.81 / 2.64.
  A web screen costs 0.064 ms GPU. What remains is the room's own scene cost,
  which belongs to arc B, not to the screens.
- **Sync** (redesigned by the operator 2026-09-25): a one-shot "jump to where
  they are". The receiver's own app loads the same URL and seeks to the same
  moment; playback, pauses and ads stay each viewer's own. Screens are drawn
  per viewer and never streamed; synced displays share a SOURCE and only our
  own domains. `docs/design/in-world-screens.md`, "Screens are per viewer".
  Then subtitles. Seek shipped in v0.1325.0 and is off this list.
- ~~Open defect: a `watch:` screen with no server retries a hostless URL~~
  FIXED 2026-09-18 in a494c7fc (`live_status` names where to set a server
  and does not connect at all); this line was stale until 2026-09-25.
- Deferred on purpose: VR controller rays; per-context `thread_local` page state
  (a screen and the main UI showing the same page share it); the `rooms.ron`
  entries for entry, pantry, hall and utility (named, not yet functional).

**Gate for any screen change:** both cargo checks, the screens / surface /
dispatch / machines lib tests, the standalone lints, AND a boot that enters the
world and drives the wall through the dev IPC. `just verify-screens` is the
named gate; static verification cannot see a dark or mirrored screen.

### B. The frame cost arc, remaining rungs

**Startup (the operator, 2026-10-18: "why HumanityOS seems to be taking a long time"): DONE
v0.1518.0, 13.3 s to 8.4 s to playable** (median of 3 boots each, same machine and day, DXC
present; `scripts/boot-timing.js`). The two cloud PSOs build in the background
(`renderer/deferred_build.rs`, `Pipeline::poll_clouds`) and the planet bake runs while the menu
shows. Leads left, each under a second: the cloud-noise upload still waits on its bake inside
`shaders_and_pipelines` (0 to 0.8 s; nothing draws clouds before their pipelines exist, so it
could upload when the bake lands), and the planet prebuild starts after renderer init (earlier
would overlap the adapter request, which a past finding warns against). Without the DXC dlls the
clouds stay off (BUG-204).

**Open, measured 2026-09-24: `gpu.celestial_t` is 25.6 ms looking straight
down from 600 km over the auroral oval** (`aurora-over-water`, about 20 fps on
the rig), against 11.3 ms at an oblique 300 km view. Two suspects are already
ruled OUT by measurement, so start elsewhere: the aurora itself (the thin-sheet
rework left the pass at 25.80 -> 25.64 ms, cost-neutral), and the emission
twin computing and discarding the atmosphere integral (moving its return ahead
of the integral changed 25.64 -> 25.63 ms). What differs straight down is that
the shell fills the whole screen, so look at what else in the celestial
transparent pass scales with shell coverage.

Design of record `docs/design/frame-cost-arc.md`. P1, P2 and P3 shipped: there is
no `fs_main`, six class entries share `frag_prologue` and `frag_tail`, thirteen
PSOs each compile one entry, and the split was itself a perf win (console
`gpu.scene` 17.1 to 14.1 ms).

- A wgpu pipeline cache. P3 cost 6 seconds of boot from three more PSO bakes,
  and this is the named next step. Note that wgpu 24 advertises PIPELINE_CACHE
  only on Vulkan; the DX12 path returns a unit struct that stores nothing, so
  requesting it naively fails device creation (the v0.782 class of bug).
- Split bodies, patches and grass inside `gpu.celestial` (three passes over the
  same attachments), which is still one unattributed number.
- A near-tree LOD ladder and impostor handoff. V1 proved the whole near-tree
  cost is ON-SCREEN fragment work, about 0.47 ms per visible model, so culling
  has no more to give and the ladder is the rung with reach.
- Clustered lights, then interior culling.
- W1 re-scoped: the water shell is 1.4 ms after P2, so the depth prepass plus
  backface cull is a fidelity and ordering call (a deterministic nearest fragment
  instead of heap-order blend), not a perf item.
- Rejected on evidence, do not re-propose: a masked-discard variant, an interior
  depth prepass, a G-buffer.

### C. The playable game

**Gameplay arc (operator: "Let's work through that", 2026-09-25).** Shipped
v0.1344.0 to v0.1388.0, in outline: the gap survey's seven defects; water as
litres; container memory, cleaning and materials; visible storage; crafting
depth (tools worn, stations that need power, byproducts at real ratios, quality
grades); gardening depth (yield from season health, light and grow lights,
N-P-K and the nitrogen loop, pests, per-crop nutrient removal, soil pH,
pollination, weeds, humidity and fungal disease, plants drawn at their spacing,
mushrooms fruiting in tents); then, the operator having chosen the SPACESHIP
FIRST (Blocked #2), the ship's closed air and water loops and the mushrooms'
CO2 in two modes (v0.1377.0 to v0.1381.0), an honest energy meter and
batteries that carry the night (v0.1384.0), mushroom racks that draw their
blocks and beds (v0.1385.0), built beds that sleep the night and chests that
hold items (v0.1387.0), and the Gagge two-node body heat model in Forgiving
and Realistic modes, with Rest as a short nap (v0.1388.0,
`docs/design/body-heat.md`). Reviews of the batches found and fixed BUG-088,
BUG-092, BUG-097 and BUG-100. The full running account, verbatim, is in
`docs/history/priorities-archive-2026-09-20-to-27.md`; progress and open items
live at the top of `docs/design/gameplay-gaps-2026-09-25.md` and in
`docs/design/ship-life-support.md`.

Open: containers as items (the gap doc's 3c) waits on the unified placement
schema. Crop light from the ship's real sun (BUG-090) was FIXED 2026-09-28: the
home hangs over its longitude on every date, and its panels, grow lights,
crops and HUD clock follow the sun the deck sees; a panel on a planet and the
weather's day warmth follow the sun where they are (BUGS.md: only the latitude
question is left, and it waits for ground farming).

**Gameplay gap survey (2026-09-25):** `docs/design/gameplay-gaps-2026-09-25.md`
lists seven defects (items lost when the backpack is full, the showcase garden
invisible to the Garden panel, 12 unsourced recipe inputs, no first seed, cooked
food with no nutrition, skipped XP, empty crafting categories) and the missing
pieces for water, gardening, crafting, containers and visible storage, with a
proposed order. Container data basis:
`docs/reference/findings/2026-09-25-container-materials-and-reuse.md`.

`docs/design/playable-assessment-2026-09-19.md` is the honest read: eleven loops
close end to end through the UI with no console and 24 of 42 systems tick, so
"framework built but not wired" is half wrong. What is missing is the layer
between the simulation and the person. Its tier ladder is the build order.

- **Tier A (make the existing game legible and durable).** DONE: A0, the art
  ships (v0.1322.0); builds persist and the world clock is saved; offline
  progression rungs 1 and 2 behind a Settings toggle (crops, scaffolds and craft
  batches, then the automated machines on real inputs and spare power, the drone
  and livestock); the simulation on the HUD (food, water, energy, air, body
  temperature and waste under the health bar, and the active quest objective;
  Always / When low, the default / Off); crop pacing (the growth-speed
  setting); and built things that work (a Furnace is a smelter and kiln, a Crafting
  Table is a workbench, a bed sleeps the night, a chest holds items across
  restarts: `construction/uses.rs`, `systems/sleep.rs`,
  `engine/built_uses.rs`); and `shelter` (2026-09-27): Build puts the piece in
  hand with a ghost at the crosshair, R turns it, E builds it there
  (`engine/build_place.rs`), a roof (`mount: OnTop`) rests on the walls it
  covers, and a roof on three walls over the player keeps wind and rain off the
  body heat model (a roof alone keeps the rain off), shown on the HUD. Since
  the BUG-102 fix (2026-09-27) this works on a planet's ground, where the
  weather is: pieces stand in planet build sites (`construction/site.rs`,
  `engine/planet_build.rs`), the aim meets the ground under the crosshair, a
  hall under several touching roof tiles shelters, and the review's other
  findings are closed. Remaining: a scripted first-run sequence in the world;
  the server clock for offline progression in multiplayer; for shelter, the
  warmth of a fire (DONE 2026-09-28: the wind direction against the open
  side, `ShelterCheck::wind_share`, so three walls shelter only with their
  back to the wind; a roof keeping the clear night sky's cold off the body;
  and a roof as shade from the sun, `body_heat::open_sky_radiant_c`); and
  the gameplay sun rises at 6:00 and sets at 18:00 every day at every
  latitude, so day length never changes with season (a Brief 6 question).
- **Tier B (make the construction tool good enough to build a city).** Pick one
  canonical layout schema of the three that exist; the four multi-storey
  blockers in order, starting with a base Y on `InteriorWall`; collision for
  generated geometry; then author the acre with the tool. The report argues the
  tool over hand-authoring on evidence: the four code blockers stopping a second
  storey are the same four stopping the editor. Built blueprint pieces got
  their first rung of this on 2026-09-27 (quarter turns, a roof resting on the
  walls, walls on a foundation, one placement function for ghost and build,
  then building on a planet's ground in f64-anchored build sites, one storey
  at a time, no double builds), and share the rest of the list: nothing stands
  on a roof yet (no second storey), doors and windows set into walls are DONE
  (2026-09-28: the Wood Wall with Door and the Wood Wall with Window,
  `construction::doorway`), walls built straight on uneven ground now meet
  at the top (`placement::level_top`, 2026-09-28: each stands on its own
  ground, taller or shorter by up to 0.3 m to reach its neighbour's top, so
  the roof rests flush), built pieces are now solid aboard and on a planet's ground (DONE
  2026-09-28: `build_place::built_piece_segments`, and on the ground
  `planet_build::collide_on_site` resolves the walk's anchor step in the build
  site's frame) and can be TAKEN DOWN (F with a piece in hand, materials back),
  and they are a
  third shape beside the home editor's `InteriorWall` and the ship structure
  pieces.
- **Tier C (make the world look right).** Hero plant models in tower net cups: DONE v0.1443.0
  (merged branch fix-tower-hero-models). The black sprouts were the crop
  palettes' COLOUR SPACE: linear MTL colours written byte for byte and decoded
  as sRGB, so lettuce reached the shader at 1.5% brightness and beet at 0.3%
  (the 2026-10-02 offline check looked for black texels, found none, and
  wrongly concluded "engine side": the bytes were never black, only in the
  wrong space). Palettes re-encoded (--kd linear, stamped), 68 mis-wound
  faces re-cut, the white key only for textures with no alpha channel, plants
  tip 15 degrees out of their cups. Leftover: home-racks-noon and
  home-racks-night still land in open space, so no rig vantage shows a tower;
  the 15 degree tip is a taste call for the operator;
  the conduit render pass; models for the machines a player stands in front of
  daily; read `mesh_kind` in `zone_filler.ron` (DONE v0.1434.0:
  `structure::filler_parts` builds market stalls, ship cradles and machine
  arrays beside the racks, and the filler box now faces out; it was wound
  inside out, which nothing showed while every filler stood taller than an
  eye).
- **Tier D (multiplayer, in the only order that works).** The save format
  compiles into the relay (DONE 2026-09-28, v0.1415.0: `PlacedItem`
  moved to `systems::inventory::placed`, `MachineLevels` to
  `systems::machine_levels`, and `persistence` and `storage` lost their native
  gate); `game_time_sync` bridged (DONE 2026-09-29, v0.1424.0; operator:
  the host clock wins): while the player is in the shared world
  (`copresence_active`, not chat-only) the relay's clock sets the game
  clock, its speed (one second a second) and its calendar (24-hour days,
  the relay's 86,400-second day, added after the review), every jump is summed
  (`time::REBASE_SLOT`) so each crop's `planted_at` moves with it and no
  garden ripens or resets on joining, sleep is refused (nobody can skip the
  host's night), and 20 s without a sync hands the speed back while the
  host's date stays; nameplates (DONE 2026-09-28, v0.1421.0:
  another player's name floats over them like the crew's) and appearance
  sync (rung 1 DONE 2026-09-29, v0.1430.0: skin tone, hair colour and
  height travel in game_join, the relay clamps and keeps them on the entity
  and in game_player_joined, each client keeps them on RemotePlayer.look;
  rung 2 DONE v0.1431.0: the figure's head is
  in their skin tone with a cap of hair in their hair colour, sized by their
  height, the body still teal; not yet seen in a capture, which needs two
  clients at once); make a trade move items (DONE v0.1433.0: an offer
  line names an item you carry and carries its items.csv id, wear and grade
  in the relay's TradeItem; Confirm waits until you carry the whole offer;
  on `__trade_complete__` each client settles its own side once,
  `trade::settle_completed`, and a received id this game does not know is
  not added. Still open: the relay holds no inventories, so nothing keeps
  the items in escrow between confirming and completing, and a player who
  drops an offered item in that window still delivers the other side; the
  real fix is server-held inventories, which comes with the relay
  `SystemRunner` host); a `SystemRunner` host in the relay.
- **Tier E: NPCs**, which is arc D below.

### D. Populate the ship, and seat a dozen

Fenced 2026-09-15, NOT STARTED. The operator's two calls: the LLM-driven AI
player is backburnered in favour of simple AI humans (500 in one mothership
sector, billions eventually), and co-op must seat at least a dozen players.
Architecture in `docs/design/crowd-simulation.md`, modes in
`docs/design/game-modes.md`, measurement in `docs/design/npc-crowd-stress.md`.

The architecture in one line: do not simulate 500 agents, because that is a dead
end at 501. Three tiers instead (an aggregate population per zone, a roster whose
members are DERIVED, an embodied set promoted only for what is visible), so
per-frame cost scales with what you can see rather than with who lives there.

Rungs, each separately measurable: 0) reconcile the relay and client worlds (the
relay simulates a multi-deck ship while the client renders the flat homestead,
and remote Y is clamped to a constant to stop crew floating in the sky);
0.5) attribute the 17.7 ms of the 18.84 ms interior deck that has nothing to do
with NPCs; 1) the `npcs:N` knob plus `[npc-diag]` counters; 2) instanced crowd
rendering; 3) the timetable refactor (chores stop being countdown timers, which
also kills the network bill); 4) promotion and demotion with interest
management; 5) navigation; 6) animation.

Measured, not assumed, and it changes the budget: NOTHING in this repo can path
an agent around a wall (`behavior.rs` and `flow_field.rs` are 26-line dead stubs
with no call sites), and there is NO skeletal pipeline and no GLTF animation
import, so NPCs and remote players draw as a two-primitive marker. A crowd on
today's renderer is sliding markers.

### E. The Library curriculum

About 90 of 143 topics still have no document (2026-09-27, after Growing
Mushrooms and Testing and Correcting Soil, both sourced and then checked claim
by claim by an independent pass). The cheapest already have both data and
sources, so one document completes all four layers; `just curriculum` lists
them and gives the live count. Budget a refutation pass PER GUIDE (48 of 60
researched species records were refuted on adversarial review).

Data gaps the curriculum already promises and cannot support:

- `data/laws/laws.json` is the declared data layer for both `greywater` and
  `forage_ethics` and has nothing for either.
- `data/chemistry/alloys.csv` records one of the four strength properties its
  topic promises and has no temper column, so 6061-O and 6061-T6 are one row.
- `plants.csv` and `creatures.csv` have no pest or disease field, so the pests
  guide's central lesson is unmodellable.
- `items.csv` cannot express edge state, tool condition, a workholding
  requirement, or a mallet.
- `data/constellations.json` is missing Cepheus and Hydrus.

For the remaining lethal-adjacent topics (`forage_toxic`, `forage_edible`,
`health_poisoning`, `chem_toxins`, `hunting`, `butchery`), write the prompt the
way the finished guide would describe itself to a reader. See the CLAUDE.md
note: the safety content is unchanged, it is the framing that trips the
classifier.

### F. Watching things on the in-world monitors

Operator: watching YouTube, Twitch and Rumble on the in-world monitors "is what
we want". The legal position is written down in
`docs/reference/media-stance.md` and the Library page
`docs/user/rights/laws_that_limit_this_software.md`.

- **Route A, our own player on open streams:** an HLS and DASH client plus the
  operating system's licensed decoders (Media Foundation, VideoToolbox), which
  also settles H.264 without shipping a patented decoder, and an Owncast
  instance on the VPS so anyone can stream to united-humanity.us from OBS and be
  watched on the monitors with no third party involved. Mission-aligned and
  unblocked. Re-read `docs/reference/findings/` before quoting the codec
  position: the video half needs no licence, the AUDIO half is the stuck one.
- **Route B2, the embedded browser:** Chromium rendered OFFSCREEN onto a screen
  surface through a SEPARATE opt-in host process (the default build never needs
  the Chromium SDK, the runtime is an opt-in hash-verified download, our own
  watch page hosts each platform's OFFICIAL embed with their ads untouched).
  Measured cost is not the obstacle: 0.064 ms GPU and under 0.63 ms CPU per 720p
  screen. The spike died in its reading stage with nothing written, so it starts
  fresh. `docs/design/embedded-browser.md`.
- **Refused as policy:** stream extraction the yt-dlp way. It breaks the
  platforms' terms, strips the ads that make embedding permitted, and invites a
  takedown against the repository that hosts our releases. B1 (a docked WebView2
  panel) is dropped: it cannot project onto a 3D surface.

### G. Realistic fire props, fire, smoke and gases (designed 2026-09-24)

Operator: 1:1 in-game flow-arts fire props (fire staff, poi, fire axe, darts,
fans, hoops) with believable fire, smoke and emitted gases, for teaching and for
depth beyond a health bar, while staying intuitive. Design, scout findings and
his seven answers: `docs/design/fire-props-and-combustion.md` (section 6 is
decisions). Key facts: EmberGen-class volumetrics were NEVER built (the fire,
smoke and explosion rows in data/particles.ron are never spawned); items cannot
carry a model; there is no body, hand or rope physics; the frame is 8-bit with
bloom off.

- **Next rung, unblocked: F1, one source of truth.** Turn the verified research
  in `docs/reference/research/2026-09-24-fire-performance/` into data rows both
  the Library and the sim read (alloys.csv solidus, liquidus, temper and
  service-limit columns; a fuels table; materials rows for fabrics). First
  resolve the 54 conflicts the data-consistency scout listed between
  data/chemistry/ and that research (fire-props-scout-reports.json).
- **Then the character body arc** (skinned mesh, skeleton, two hand attachment
  points instead of the single hands slot): the operator chose body first for
  props in hands. Pivot-mounted props are test fixtures only.
- **Rules he set for every rung:** full-realism and simplified modes; gear AND a
  toggle for gas visibility; open source only, our own solver (no EmberGen
  licence); cosmetic simulation client-local and never networked, each tier with
  a measured frame budget; fire trails drawn as the eye sees them; coloured fire
  from chemistry data plus a creative any-colour toggle.
- Ordering against arcs A to F is the operator's call.

### H. Video for social media (the clip maker, v0.1435.0, operator 2026-09-30)

The operator chose video as the lever after two days of logs showed about 19
visitors from social links (update posts are changelogs for people who already
care). `just clips` records every shot in `scripts/clips.json` frame-exact
(`src/engine/movie.rs`) and cuts 16:9, 9:16 and a still. First batch: the sea
sunset, Japan from orbit, the open sea and Silverdale are strong; Earth turning
is small in frame and the Fuji drop's middle is blurry low-resolution land and
haze. Next rungs, in order:
- **Native portrait render:** the 9:16 cut is a centre crop of a landscape
  frame (about 780 px wide, upscaled). Render the live view at 1080x1920.
- **Better shots:** the drop needs the ground's texture detail at mid-altitude,
  and Earth turning wants a closer, sunlit framing; add shots of whatever ships
  next (a trade, building a wall with a door).
- **Record button in the Studio page** (logged in `docs/design/in-app-ops.md`).
- **Text overlays and music**, if posting shows captions burned in do better.

---

## TIER 1: hardening before invites scale beyond a known group

**Effectively closed.** Everything code-actionable shipped (fail2ban, watchdog
plus multi-channel alerting, SQLite corruption recovery, crash-loop detection),
and the two decision-gated items were decided by the operator in 2026-05.
Retired detail is in `docs/history/priorities-archive-2026-05-to-08.md`.

Two residuals from the 2026-06-12 security audit, both MEDIUM, neither a launch
blocker:

1. **`/api/send` per-IP rate limit.** Needs X-Real-IP plumbing. Low value while
   the bot path is the trusted API_SECRET path.
2. **`/api/members` directory opt-out: SHIPPED** (found stale 2026-10-09).
   `src/relay/storage/members.rs` honours `directory: "unlisted"` in
   `profiles.privacy`, and both clients have the switch (web chat-privacy.js,
   native src/gui).
4. **DONE v0.1470.0 (checked 2026-10-22: no Google STUN anywhere in src/ or web/; calls and
   voice rooms go through the server's own forwarder): Voice told Google the player's IP address
   (found 2026-10-05).** The original finding, for the record: WebRTC asks a STUN server
   for the public address, and every client lists Google's first: native
   src/net/webrtc.rs (~175), web web/chat/chat-voice-rooms.js (~25), and the relay's own
   ICE list in src/relay/turn.rs (~65), which also advertises a STUN at the server's
   port 3478 that nothing listens on (checked on the VPS: no coturn, nothing on
   3478). Fix wanted: a small STUN binding responder inside the relay binary (the
   stun module already builds and parses the messages), the clients told to use only
   their own server's, and Google dropped. Needs UDP 3478 opened on the VPS firewall:
   the operator's call (asked 2026-10-05). Until then the privacy page should say so.
3. **DONE (checked 2026-10-22): Block shipped in v0.1468.0, withdrawable friendship passes in
   v0.1466.0, "who can reach me" in v0.1467.0. The original finding: Blocking a sender (found again 2026-10-04 by the conflict guide's fact
   check; recorded in `docs/accord/conformance_gaps.md`).** Nobody can stop
   another person messaging them: a friendship certificate cannot be withdrawn
   (`verify_friend_cert` is stateless, no revocation), unfollowing does not
   close the message path, a stranger's certless "knocks" allow 20 a day
   (`DM_KNOCKS_PER_DAY`), native chat has no block at all, and web's block list
   only hides messages locally, by name. The Accord requires consent to be
   revocable. Wanted before the shared world opens to strangers: a
   recipient-side deny list the relay enforces at dm_put (kept as the
   recipient's own sealed data where possible, in the certificate pattern),
   certificate revocation (a serial or an expiry in the preimage, with the KAT
   tests updated on both clients), and a Block button on both clients.

---

## TIER 2: big-feature gaps

Real features the system promises but does not deliver on every platform. Weeks
of work each.

**Candidate (suggested to the operator by a member of the public, 2026-10-05; not yet
decided): search from the Browser page's address bar.** Today the address bar takes URLs only.
Suggested: Ecosia as the default engine, and privacy-first defaults like FireDragon or
Brave. Before building: the engine list as a data file the player chooses from, and a
dated findings document on each engine's terms for being queried from inside another
app (the 2026-09-25 site-embed review is the pattern). The embedded Chromium for video
platforms keeps those platforms' players and advertising intact (the 2026-09-18
agreement), so ad-blocking defaults do not apply there.

> **Cross-cutting mandate (CLAUDE.md non-negotiable rule): GUI-first
> configurability.** Every ops and config capability must be reachable in-app,
> not CLI-only. The shipped ops work (alerts, backups, fail2ban, watchdog,
> secrets) is still CLI/SSH, and that is tracked debt. See
> `docs/design/in-app-ops.md`. New features with an ops dimension build their
> in-app control in the same increment.

1. **Web-mirrors-native parity (Track W).** Divergence map and migration order
   in `docs/design/web-native-parity.md`. Native chat is the parent. Steps 1 to 6
   DONE (2026-10-22): message rows with the reactions in the time pill, the
   expanded pill and the right-click menu, the header and composer, the top menu,
   the dead-CSS sweep (and a reaction that cancelled itself on the sender's own
   echo, fixed). Left (the parity doc): DM rows need a working reply before their
   pill; native should offer Pin only where the relay allows it; the rails and
   modals as view modules.
2. **PARKED (operator, 2026-10-22: "Let's avoid working on this for now."). Studio and streaming (Track S).** `docs/design/studio-streaming.md`. Native
   capture, encode and stream shipped (v0.853 to v0.854), so build the widget on
   web first and mirror once native transport exists. Order: S0 persistent
   session, S1 web studio widget and modal, S2 viewer widgets and modal, S3
   privacy guard (independent, can land early), S4 native mirror.
3. **In-app ops console.** `docs/design/in-app-ops.md`. Slice 1 (System/Health)
   shipped on both clients. The backups panel shipped 2026-10-22 with continuous
   backup (item 12; an in-app "restore on next restart" remains). Remaining: the
   alert-channels editor (the first write panel; continuous backup's alerts go
   through its file today), a federation panel, then fail2ban /
   relay-control / secrets (these need a sudo-gated relay-to-system bridge),
   then factoring out the action registry with AI-facing list and run endpoints
   plus a coverage test.
4. **Federation activation.** `docs/design/federation-activation.md`. Native
   Phase 1 admin UI shipped (v0.722.0); the web mirror is what remains of Phase
   1. Phase 2 per-peer profile-gossip rate limit, Phase 3 a second
   operator-controlled relay federated end to end (the load-bearing test is
   whether moderation propagates), Phase 4 vetted third-party peers. The
   fail-closed default means dormant is safe.
5. **P2P groups, phases 3 to 5.** `docs/design/p2p-groups.md`. P1 and P2 are
   done on both clients (signed objects, offline-joinable invites, E2EE
   messages, group-as-channel, leave and disband). P3 P2P transport (the relay
   becomes signaling-only), P4 relay-independence (multi-relay signaling plus
   peer-assisted plus TURN: the actual payoff, a group survives a dead home
   relay), P5 serverless discovery (mDNS/DHT).
6. **Privacy follow-ups** from the 2026-08 arc: full native inline image decrypt
   and render; group-chat attachments (the same pattern as DM attachments, not
   yet done); friendship certificate expiry and revocation; a native TURN
   toggle. Beyond these the honest remaining set is mixnet-class traffic
   analysis and fundamental limits.
7. **Native voice tail.** The str0m arc shipped voice itself. Remaining:
   per-peer volume / mute / squelch UI, web transmit-mode UI, a two-str0m CI
   harness, graceful relay restart.
8. **Native trade UI completion.** DONE: the page has sent `trade_response`
   and `trade_confirm` since v0.756, and since v0.1433.0 a completed trade
   moves the items (see Tier D above).
9. **Library, the federated file and media catalog.** `docs/design/library.md`.
   The Files engine first (trust-tiered LRU cache, bounded disk by construction,
   identity by content hash), then the Files UI, pin and torrent, perceptual
   dedup, federation aggregation, and folding Tools / Browser / Resources in.
10. **Device mesh.** `docs/design/device-mesh.md`. Your devices back up each
    other and the relay; review every device's system info from any one device.
    Phase A system-info reporting and a My Devices dashboard, B backup
    designation and pull (subsumes the shipped PowerShell stopgap), C restore
    flow, D LAN direct-sync plus mobile members and remote wipe.
11. **Real-life-first boot and the real/fake multi-save model** (revised
    2026-06-30; the operator rejected the "game/simulator toggle" framing as too
    confusing). Multiple saves, each house or character flagged real or fake.
12. **Continuous backup: BUILT 2026-10-22, waiting on release and the operator's
    setup.** WAL shipping inside the relay (`src/relay/backup_stream/`,
    `docs/admin/continuous-backup.md`) to a folder, an S3-compatible bucket or another
    HumanityOS server, sealed with the backup key, RPO about a minute, point-in-time
    restore, in-app Restore drill, PC monthly drill (`scripts/backup-drill-pc.ps1`).
    Next: the operator's deployment steps in that doc (united-humanity.us and
    public.guide keeping each other's copies), then an in-app restore-on-restart.
13. **PARKED (operator, 2026-10-22, with streaming). Mobile clients.** Android needs a JNI bridge for the keyring plus an
    AndroidKeyStore backend; iOS mostly needs a build target.

---

## TIER 3: UX accessibility (the ELI5 mandate)

**Operator, 2026-10-22:** "I like the idea of everything here but, let's only focus on English for
now but, make sure we prep for all the other languages." So: tooltips, the first five minutes,
the WCAG 2.1 AA audit and the glossary on every page, in English, with every new or touched
string going through `data/i18n/` so a translation is data, not code. Queued after the website
chat work (Track W step 3) merges, because both touch the same pages.

The mission requires this layer. Not optional, just sequenced after the
load-bearing work.

1. **A tooltip on every interactive element**, in plain language. Audit pages one
   at a time.
2. **The first five minutes.** A guided tour: identity, seed backup, first
   channel, first message, status, done. The Onboarding page exists but the flow
   needs polish.
3. **Localization expansion.** Five languages today (en, es, fr, ja, zh). Add at
   least ar, hi, pt, ru, de, sw. `data/i18n/` supports it; the work is
   translation, not code.
4. **A full accessibility audit** against WCAG 2.1 AA. The modes exist in
   `src/gui/theme.rs`; the audit does not.
5. **Glossary on every page.** 442 terms in `data/glossary.json`; web has the
   overlay, native has no widget yet.

---

## TIER 4: long horizon

Do not touch these until TIERs 0 to 3 are mostly done. Listed so they are not
forgotten.

1. **LoRa mesh hardware integration.** Needs actual radio hardware on hand.
2. **STARK selective disclosure.** The scaffold exists; circuit design deferred.
3. **AI agent governance.** First-class AI participation is documented in
   `docs/ai/onboarding.md`; as more AI participants connect, Article 14 needs to
   become enforced rules with appeals rather than documented intent.
4. **Distribution beyond GitHub.** The Forgejo mirror exists, BitTorrent and IPFS
   are scaffolded; Codeberg, Software Heritage and a WinGet manifest are pending
   per `docs/admin/distribution-mirrors.md`.
5. **Real-hardware control layer.** Bind a home to real monitoring and automation
   hardware, so the game becomes the control panel for an actual homestead. The
   north star.

---

## Tier criteria: how to decide where something goes

- **TIER 0**: the next thing to work on, strict-ranked, at most a handful of
  items. If TIER 0 has grown a second "current focus" block, that is the signal
  to resolve it back into one order.
- **TIER 1**: "we can invite known people but not unknown people until this is
  done."
- **TIER 2**: "the feature is promised but does not fully work." Multi-week.
- **TIER 3**: "real users can use the app but they need help understanding it."
- **TIER 4**: "nice eventually; do not let it crowd out the load-bearing work."

When adding an item, pick the LOWEST tier it could justifiably go in. Tier-up is
rare; tier-down is normal as things turn out less critical than they felt.

---

## Where the shipped work went

This file lists only what is NOT done. For what shipped, the live sources are
`git log`, the GitHub release titles (unusually descriptive in this repo),
`data/coordination/orchestrator_state.json` `recent_decisions` (the WHY),
`docs/FEATURES.md`, `docs/STATUS.md`, and `docs/history/<date>.md`.

Retired backlog blocks, verbatim and with their reasoning intact:

- `docs/history/priorities-archive-2026-09-20-to-27.md` (the aurora pass, the
  dark clouds, the coast glow, the grain arc's measurements, the page
  snapshots, and the gameplay arc's running account, retired 2026-09-27).
- `docs/history/priorities-archive-2026-08-to-09.md` (the cloud, rosette, perf,
  screens, Library and content arcs, 2026-08-21 to 2026-09-19).
- `docs/history/priorities-archive-2026-05-to-08.md` (the old Active focus stack
  back to the web chat rebuild, plus the settled TIER 0 and TIER 1 entries).

Do not reintroduce a hand-maintained "recently shipped" list here. One rotted to
v0.283.0 while the project shipped past v0.515, and a third competing "what is
done" list is worse than none.
