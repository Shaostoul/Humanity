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

### RESUME HERE: where the work stands (updated 2026-10-09)

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
- **NEXT non-game item, started 2026-10-09: blocking and safe mode** (TIER 1 item 3, widened by the
  operator: "give users the tools to protect themselves", plus a safe mode that protects children
  from predators and scams, and everyone else too). Design in `docs/design/blocking-and-safe-mode.md`
  (written 2026-10-09): its section 3.7 lists six defects to fix regardless; the first, web data
  sync answering strangers, is fixed (BUG-170), web's half of the rest (BUG-171) and the relay's
  (BUG-172) and native's (BUG-173) shipped in v0.1465.0; native's still needs seeing in a running
  app (the click-to-load placeholder; a first call between two home networks). NEXT: the "Who can
  reach me" table with friendship passes that never expire (design section 10a), then Block on
  both clients. Step A (passes v2) shipped in v0.1466.0; step B ("who can reach me") merged 2026-10-09 (Settings > Safety on both clients, safe defaults enforced by the relay, contact requests that carry the requester's pass). Step B follow-ups: "People I choose" for Messages and Trades acts like Friends until per-friend message and trade ticks exist (the operator asked to choose individuals and groups); a contact request refused under Nobody gets the same reach_refused as a message, so clients may offer the request again; incoming rings are not also filtered by the callee's client; tests/snapshots/settings_full.png predates the Safety section. Step C, Block on both clients, ships in v0.1468.0 (one block list per identity on native; game nameplates not yet, since RemotePlayer carries no identity key). Step D, reports the admins can check, ships in v0.1469.0 (signed reports, DM evidence checked against the sender's own signature, never a message carrying a file, Reports for admins and moderators; still to build: sending group reports to the group's creator, and showing the per-country help lines from data/safety/outside_help.json in the dialog). Step E, our own STUN and the room-scoped call forwarder, ships in v0.1470.0: calls and voice rooms relay only through the server, Google gone from both clients; calls stay unavailable until the operator opens UDP 3478 (docs/admin/call-forwarder.md). Step F, warnings and the recovery-phrase guard, ships in v0.1471.0: warnings under direct and group messages from strangers (some for friends too), matched from data/safety/warnings.json identically on both clients (17 shared cases), a stranger's links held until Open, and a send holding 4 or more of your own recovery phrase's words in a row is stopped (numbered lists included); reported posts never show admins a file or a link. Step B and D follow-ups built 2026-10-10 (design 10c-ii, a Message/Call/Trade tick per friend; 10e-ii, help outside this server in the report dialog): web halves merged, desktop halves in review, shipping together. NEXT in the safety design: step G, the protected setup (spec 10h, written from the dated finding `docs/reference/findings/2026-10-10-childrens-online-safety-rules.md`: a PIN lock on this device's safety settings, a review of existing friends, groups and rooms when it is turned on, an always-visible line telling the person it is on, never called "kid safe", "child safe" or "compliant"). **Operator decisions from that finding** (each with the finding's reasoning; the setup is built with neutral words meanwhile): (1) the age position: the web chat's entry screen says "you confirm you are 18 years or older", which a setup for children contradicts; the finding's two coherent options are a general-audience service for 13 and over (the setup aimed at parents of teenagers; recommended) or 18 and over with the setup described "for someone you look after"; (2) whether the project server's rules page (`web/pages/rules.html`) should name pornography and content encouraging suicide, self-harm or eating disorders as not allowed (the UK Online Safety Act condition, section 12(5), under which mandatory age checks would not apply); (3) whether to email the FTC's COPPA hotline (CoppaHotLine@ftc.gov) describing the no-sign-up design, which would turn three unknowns into staff guidance (an outside message, the operator's to send). Also from it, not yet built: an admin tool to erase another person's data on a server; the California AB 1043 finding (being researched 2026-10-10; in force 1 January 2027). Also the camera-safe sign-in items above, awaiting the operator's pick. Found while building A: (1) the web Trade
  page (`web/pages/trade-app.js`) never answers `identify_challenge`, so its socket is never
  signed in and its trade requests never reach the relay (an old bug; `tasks-app.js` has the
  sign-in pattern); (2) for step B, the relay cannot tell a text message from a voice message
  inside a sealed DM, so `message` and `voice_message` can only be told apart by the recipient's
  client (discard a voice message from someone whose pass lacks it) unless the kind travels
  outside the seal, which would tell the server what kind of message it is. Client follow-ups from the
  relay fix: both clients
  auto-reject a ring when already in a call, which tells a caller a hidden person is online;
  neither sends `friend_cert` on trade requests yet (friends' trade notes are cut to 80
  characters until they do); the 10-active-trades limit counts received trades, so strangers'
  pending requests can block someone starting their own. Next is its increment 0 (the other five defects),
  then increment 1 (block on both clients). Eight questions for the operator sit in its summary,
  each with a recommendation. Voice: route calls through a room-scoped forwarder in the relay
  plus our own STUN on one UDP port, Google removed (section 7; the port is the operator's call).
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
- **Legal research needed before reports go further (raised 2026-10-09).** Reports (design 10e)
  must never carry a way to open a reported file: a DM carrying an encrypted file holds the key
  in its text, so such messages cannot be evidence (refused by the relay, not offered by either
  client). Still to research into a dated findings document (CLAUDE.md, "Research a legal
  question"): what a server operator must and must not do on receiving a report that may
  involve child sexual abuse material (for example the United States' 18 U.S.C. 2258A duties of
  electronic communication service providers, and the equivalents elsewhere), including whether
  a volunteer-run server counts as a provider, what must be preserved, and where it must be
  reported. Until then, the reports UI tells people to contact emergency services and their
  country's official reporting line, and admins are never shown files. The per-country list of
  official lines is being researched in `docs/reference/findings/2026-10-09-outside-help-lines.md`.
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
- **NEXT in TIER 0:** increment 5b (the household page: give a friend a permit to build on
  your plot, and walking into a permitted plot), then increment 6 (the starts as data, the
  Cabin, the mess hall; fold in a second mess hall along First Street). Day 5 (a real
  two-person session on the VPS) is unblocked whenever the operator has a partner.
- **Follow-ups found today (not started):** BUG-161 (the copresence rig's steady-speed
  judges fail a starved machine); BUG-163 part 2 (refresh a stale data folder on update:
  docs/design/data-folder-updates.md, waits on the operator below); no gasoline or diesel
  item (Refined Fuel stands in for the generator's gasoline); the refinery's drum accepts
  any liquid; campfire light (point lights in the celestial pass); heaters aboard only, and
  data/hvac.ron's heat pump and wood stove have no machine; inside survey regions without
  terrain tiles the ground draws at 54 m triangles while buildings stand on the finest
  ground (BUG-156's note); far trees should re-ground on the patch drawn under them;
  src/systems/fire.rs takes fire damage per frame in f32 (BUG-164's note); every drink
  gives 30 points whatever its volume; nothing applies Bleeding or infections yet; a
  second mess hall along First Street (fold into increment 6); the backpack-overflow toast
  shows an item id and says it "stayed in Home" for things that were never there ("Backpack
  full: 8 x wood_plank_0 stayed in Home", lib.rs and gui/organize.rs `return_to_storage`):
  name the item and say it went to home storage.
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
  system is never registered.
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
are never saved; `data/medical.ron` is never applied (its system is not
registered with the game); `data/species.json` and `data/geology.ron` are not
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
4. **Voice tells Google the player's IP address (found 2026-10-05).** WebRTC asks a STUN server
   for the public address, and every client lists Google's first: native
   src/net/webrtc.rs (~175), web web/chat/chat-voice-rooms.js (~25), and the relay's own
   ICE list in src/relay/turn.rs (~65), which also advertises a STUN at the server's
   port 3478 that nothing listens on (checked on the VPS: no coturn, nothing on
   3478). Fix wanted: a small STUN binding responder inside the relay binary (the
   stun module already builds and parses the messages), the clients told to use only
   their own server's, and Google dropped. Needs UDP 3478 opened on the VPS firewall:
   the operator's call (asked 2026-10-05). Until then the privacy page should say so.
3. **Blocking a sender (found again 2026-10-04 by the conflict guide's fact
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
   in `docs/design/web-native-parity.md`. Native chat is the parent. Steps 1 and
   2 done; NEXT is step 3 (message rows, timestamp pill, inline reactions), then
   header and composer, top-nav alignment, and a spacing sweep with dead-CSS
   removal.
2. **Studio and streaming (Track S).** `docs/design/studio-streaming.md`. Native
   capture, encode and stream shipped (v0.853 to v0.854), so build the widget on
   web first and mirror once native transport exists. Order: S0 persistent
   session, S1 web studio widget and modal, S2 viewer widgets and modal, S3
   privacy guard (independent, can land early), S4 native mirror.
3. **In-app ops console.** `docs/design/in-app-ops.md`. Slice 1 (System/Health)
   shipped on both clients. Remaining: the alert-channels editor (the first
   write panel), a backups panel, a federation panel, then fail2ban /
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
12. **Litestream or equivalent continuous backup.** Documented-optional today
    and verified NOT deployed. SQLite WAL to blob storage, RPO about a minute.
13. **Mobile clients.** Android needs a JNI bridge for the keyring plus an
    AndroidKeyStore backend; iOS mostly needs a build target.

---

## TIER 3: UX accessibility (the ELI5 mandate)

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
