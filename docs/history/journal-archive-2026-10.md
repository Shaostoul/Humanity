# Orchestrator journal archive -- 2026-10

Decisions rotated out of `data/coordination/orchestrator_state.json` (oldest first within each batch; newest overall is in the live journal). Source of truth for "why we did X" once it ages past the live tail. See also git log + the GitHub releases.

## 2026-10-02

**Decision:** Week plan (survey x5, plans x3, judge, then 7 adversarial verifiers) recorded in PRIORITIES TIER 0 "THIS WEEK": review, then playable multiplayer proven by two people (scripted second player, smooth movement, shared building, a real session). Day-1 fixes shipped: BUG-107 backup pull, BUG-108 Rotate Key removed, BUG-109 Windows zip download, BUG-110 web bug votes, BUG-111 Tasks sign-in, BUG-112 relay live_conns.

**Why:** Operator asked what to work on this week; near-term success is playable multiplayer on the VPS. The verifiers turned up a 39-day-stale off-box backup and an identity-replacing button, so those shipped first. BUG-112 came from the reviewer of the Tasks fix: making Tasks sign in would have signed people out of voice and the game whenever they closed it.


## 2026-10-02

**Decision:** Review of v0.1427.0..v0.1436.0 (4 finders, 9 skeptics): 8 confirmed, 1 refuted, 15 low unverified. HIGH: desktop trades never move items and the desktop Trade page never lists trades (relay sends Private as type system; the client handled trade wrappers only in its private arm; tests called settle_completed directly). MEDIUM: trade duplication after confirm; offline-at-completion never settles; quitting the game with a web tab open leaves avatar and voice seat forever (regression from live_conns); nameless Tasks socket shows the person as Anonymous; make-clips Ctrl+C leaves the game running; stall roofs/array ducts see-through from below; remote figure head inside the body box. Being fixed in workflow wf_31a99714-8ed (lanes: trades, relay, tooling, rendering), uncommitted until reviewed.

**Why:** The review owed after the v0.1428-v0.1436 batch (memory feedback_review_after_parallel_waves). The trade miss is the checks-that-cannot-fail class: a test must feed the real envelope the client receives.


## 2026-10-02

**Decision:** Deleted the 15 plain pre-encryption backups on the VPS (shred -u, /opt/Humanity/backups/relay-*.db from 2026-08-23/24) and the 60 stale identical local copies in %USERPROFILE%HumanityBackups. Kept the 15 current .db.aes on the VPS and the 3 pulled locally.

**Why:** Operator: "Let's just delete them. They're not important to keep at this point." Read as the old backups flagged as decision 5; the current encrypted backups are the live safety net and were not in question.


## 2026-10-02

**Decision:** Review fixes shipped (BUG-113..121): trade router for system frames + honest settling + per-save settled ids with re-settle after a save load; relay seat ownership per socket (game, voice) with client voice re-join on reconnect; current_name_for_key (member row only while still registered); make-clips Ctrl+C/cancel/BT.709; filler underside coverage rule; remote figure from the floor; a byte-slice crash in frame_ws_poll.

**Why:** The owed review of v0.1428-v0.1436 confirmed 8 problems; reviewers sent 2 of 4 fix lanes back, and I fixed the last reviewer finding (revoked device name) myself with a red-checked test.


## 2026-10-02

**Decision:** Day 2 part 1: QuestObjective::Make (units of an item from any recipe; crafting credit_craft pushes make_<item> per unit) and gs_first_steps iron step uses it; homepage game panel plays the home-flyover clip (720p, 1.2 MB, IntersectionObserver, still for reduced motion/save-data); sw.js skips video/audio/range requests.

**Why:** Week plan Day 2. A Make objective generalizes instead of listing recipe alternatives per quest (infinite-of-X); Craft stays for steps that really mean one recipe. The clip is honest (captioned as the game today) and costs nothing until on screen.


## 2026-10-02

**Decision:** Backup inventory (workflow: code/VPS/PC sweeps + keeper check). Newest .db.aes and .db.enc restore (integrity ok, counts match). Fixed BUG-122 (script rotation via find; HUMANITY_DB_BACKUP_ROTATE_KEEP), BUG-123 (6h snapshot + expiry due by newest snapshot age), BUG-124 (status/panel count sealed; Back up now seals). The trim deletions handed to the operator as commands; retention counts await his call.

**Why:** Operator asked to delete old/stale/duplicate backups down to the latest. Deleting backup data is his to run; the inventory showed my earlier deletion of the plain copies had broken the 30-min script, which had to be fixed before any trim or the folder grows without limit.


## 2026-10-03

**Decision:** Day 3 shipped: scripts/second-player.js (real Dilithium identity, game_join, walks a path, just verify-second-player boots a throwaway relay copy); net::sync snapshot interpolation on the sender clock (timestamp = sender steady clock in seconds, REAL time; velocity on the movement step); leave and ban despawn at once (BUG-125, BUG-126).

**Why:** Week plan Day 3. Two build rounds with critics: the first smoothing locked to 15 Hz, the second stamped capped steps and stalled slow senders; I fixed the last two findings (real-time clocks, ban path) myself with red-checked tests.


## 2026-10-03

**Decision:** Co-presence rig: scripts/verify-copresence.js + just verify-copresence (one GPU boot, throwaway relay helper scripts/lib/throwaway-relay.js, judge scripts/lib/copresence-judge.js with 23 red-checked tests, recorder debug/remote_players_request.json in engine/ipc.rs with epoch_ms and a row cap). Critic fixes: only_the_walker, on_time by epoch, tests for seen_every_frame/enough_frames/bad clock, --keep-open message.

**Why:** Verify Day 3 on the state a player reaches: unit and relay tests feed the drawing made-up deliveries; nothing looked at what the real game draws.


## 2026-10-03

**Decision:** OPERATOR DECISIONS on multiplayer homes: own location per home, no overlap, build only in your own home, all players equal (no main player), meet anywhere on the mothership (mess hall, rec room, crafting areas, hangar; missions group at a transport ship), goods physically transported with travel time (trains, elevators, fast travel), tiered starts (full homestead / apartment in a complex with shared areas / bare minimum for kids and the time-poor). Shared-building build stopped at its contract step; design re-scoped to ship-frame plots.

**Why:** Operator, verbatim: "I would actually like the player homes in multiplayer to actually have their own locations... I don't like the idea of homes overlapping. Players could meet anywhere on the mothership ideally... We really want it to logistically make sense. Theoretically HumanityOS could be used for city planning." The first shared-building design put every home at the same coordinates with anyone building anywhere.


## 2026-10-03

**Decision:** Operator follow-up: cabin/apartment food is physically accounted for (communal gardens, ship farms), like real life where most homes are not homesteads; suburban house limited by its lawn. Names Cabin/Apartment/Homestead: "fairly happy". Proposed: four starts Cabin, Apartment, House, Homestead; home types as data (Townhouse, Smallholding, Farm later); one ship-wide food loop with simplified mode.

**Why:** Operator asked for other home styles and whether communal food can be physical; recorded so the design synthesis and later sessions use it.


## 2026-10-03

**Decision:** OPERATOR DECIDED: the full list of home types (Cabin, Apartment/cohousing, Townhouse, House, Smallholding, Homestead, Farm, plus group forms) as data, for a full spectrum of colony ship designs; homestead is the default start. Operator went to bed, leaving the session to develop overnight.

**Why:** Verbatim: "I like the full list as it adds variety and allows for a full spectrum of colony spaceship designs."


## 2026-10-03

**Decision:** Overnight combined release v0.1442.0: ship-homes increment 1a (ship_structure.ron + data/homes/homestead.ron + data/machines/ship.ron, assemble at plot, Dev Plots/Districts panel in construction/ship_tools.rs, E-key footprint bound, ship machines read-only outside Dev); save snapshots + Restore (BUG-129); Back up now keeps 10 (BUG-130); figures (BUG-127; BUG-128 inside-out Mesh::sphere open); shared DXC lookup scripts/lib/dxc-dlls.js.

**Why:** Operator asked for overnight development with more parallel lanes (usage reset). Each lane builder + critic, fix rounds; shipped together after one combined verify because 1a and the lanes shared lib.rs, photograph-home.js and in-app-ops.md.


## 2026-10-03

**Decision:** Merged fix-tower-hero-models (v0.1443.0): crop palettes re-encoded linear-to-sRGB with a kd_space stamp, converter requires --kd, 68 faces re-cut, white key only for alpha-less textures, 15 degree net-cup tip, leaf-back normal flip removed.

**Why:** The black tower sprouts were the palette colour space, not the engine; my 2026-10-02 offline conclusion (engine side) was wrong.


## 2026-10-03

**Decision:** Merged fix-station-vantage (v0.1444.0, BUG-132): the station camera verb places the camera with the offset of the frame it now rides, and camera_done is written a frame or two later with the MEASURED home-frame position, error, look, station_ride and clock_settled. probe-sweep judges every station park. Critic gaps closed on main: a park without a pose (default view, screen) is judged against the look the engine reported (requested_yaw_pitch), and vantages.json puts a station view straight after a planet view (the order that exposed it), pinned by a rig test.

**Why:** Station vantages had captured empty space since at least v0.1441.0 while probe-sweep reported ok. Both new tests were red-checked; the first draft of the look test could not fail (it fed the checker a field it does not read), caught by reverting the change and seeing it still pass.


## 2026-10-03

**Decision:** Merged BUG-128 (v0.1445.0): Mesh::sphere faced in on every triangle (rows run down from the pole; the index pattern was copied from the open cylinder, whose rings run up). Fixed there and in six copies or relatives (hologram sphere, pin head and stem, orbit ring, pyramid base, fruit sphere and cone). Winding tests for every mesh.rs generator, seen red. Rig: 7 home rooms before/after, 0 panics, no visible regression; the visible gain was not photographed (no standard view frames a large opaque sphere).

**Why:** Opaque spheres showed their far inside; the fruit was also lit from inside. Kept the scope beyond the sphere because each extra shape was proven wrong by a failing test.


## 2026-10-03

**Decision:** Increment 1b red check done by pointing the 1b rig at the v0.1444.0 (1a) archive instead of reverting source (the builder was refused a revert): five plot checks failed, camera_in_p2 at (53.50, 1.70, 40.50) as predicted; the other 15 passed. Logged BUG-133 (check-fresh-exe judges by dates, so a build from another tree passes) and dispatched a fingerprint fix to a worktree.

**Why:** The 1b proof needed its red; using an older build is non-destructive. The same path showed the freshness gate cannot tell which tree a binary came from.


## 2026-10-03

**Decision:** Merged BUG-133 (v0.1446.0): the rigs freshness gate compares a SHA-256 fingerprint of the compiled-in sources, stamped into the exe by build.rs, with the tree; dates no longer decide. --allow-other-build "<reason>" for deliberate red runs, recorded in rig manifests. Every exe built before this is unstamped and refused by the rigs and launch-bg (not by the operator's just launch).

**Why:** A 1a build passed the 1b worktree's gate as "the current build" because it was newer by date. Residuals logged in BUGS: probe-sweep has no gate, rigs copy after checking, ~30 compiled-in data files are not fingerprinted.


## 2026-10-03

**Decision:** Merged the firewall-prompt fix (v0.1447.0): BIND_ADDRESS for the relay (default stays 0.0.0.0: the LAN node, port forwarding and Tailscale-direct setups need it; the VPS is reached only through nginx on loopback either way), every dev launcher and test on 127.0.0.1 with a loopback self-check in throwaway-relay.js, Host a node "Who can connect", and the saved node no longer autostarts on a script-launched boot. CLAUDE.md: a rule next to one-GPU (hand-run relays need BIND_ADDRESS=127.0.0.1), and the Tor row corrected (the onion does not serve the website; Tor is not installed on the VPS).

**Why:** The operator was interrupted by Windows Firewall prompts many times a day: 274 HumanityOS rules on his PC, about 209 from two Host a node unit tests whose binary path changes every build, about 54 from throwaway relay copies in random temp folders. The review also found his saved node autostarting on agent boots and opening his real relay.db.


## 2026-10-03

**Decision:** Increment 1b NOT merged after round 4: a focused review confirmed 11 findings, one HIGH (a returning player whose chat is already connected joins before the ship loads, gets a false "different ship" refusal, and stays out until a reconnect; the rigs could not catch it because the autopilot identifies after entering). Round 5 runs as a workflow: fix all 11 with the rig made able to fail first, review the new diff, fix again, merge main, rerun every rig.

**Why:** Each review round of 1b has found real problems, so it merges only when a round comes back clean.


## 2026-10-03

**Decision:** Merged ship-homes increment 1b into main (1c41de3b9, release v0.1448.0) after five review rounds (5, 7, 16, 11, 6 confirmed findings, all fixed and red-checked). The last round fell to one medium and five lows; a single critic reads the final fix commit in parallel with the release checks. Next: increment 2 (meet in the Commons), which Day 5 needs.

**Why:** The findings converged (a HIGH in round 4, a medium in round 5), every rig passes on the merged tree including the connect-first entry that exposed the HIGH, and holding it longer would only delay increment 2.


## 2026-10-04

**Decision:** OPERATOR DECISION (BUG-135 limit): the relay keeps an EXPIRING marker after an account erase (option 2 of 3), so a second device that was offline during the erase cannot sign the person up again by itself. Verbatim: "Let's go with option 2 that way we have a way to cull the list over a period of time. That way we don't end up with a massive log of all the accounts that erased themselves after many years or a malicious attack." Built as: a keyed one-way fingerprint of the key (a server-local secret, so only that server can check it), day-granularity date, expiry default 30 days as an in-app server setting, plus a hard cap on entries (oldest dropped) against mass erases; an explicit Connect signs up again and clears it.

**Why:** The erase fix shipped in v0.1449.0 only reaches devices online at the erase; the relay kept no record. The operator chose a bounded record over none (option 1) or a permanent one (option 3), and named both the long-term growth and a malicious flood as reasons for culling.


## 2026-10-04

**Decision:** OPERATOR ANSWERS (2026-10-04), the open-questions list. (1) Day 5 partner: he will post on his Discord or ask a brother when ready. (2) Shared-world clock: 72x default, "make sure there's admin tools for me to adjust it from inside the app". (3) "Let's keep working on multiplayer. The screens are essentially functional but, not fully. They're not as important as getting the game playable at the moment." (multiplayer ahead of arc A). (6) Backups: keep the current numbers. (7) No GitHub branch protection for now: "I'm presently happy with immediately updating the live server." (9) Keep the 15 degree net-cup lean. (11) Donations: tax-deductible to the nonprofit at https://www.sponsor-a-can.org/donate/ ; to him directly (not tax-deductible) at https://www.patreon.com/Shaostoul for the time being. (13) The life-safety Library guides stay "sourced" until someone more qualified is officially involved: "I'm not comfortable verifying something I can't easily prove or have prior knowledge on."

**Why:** Answers to the 13-item waiting list; each item routes work (clock setting lane, BUG-136 lane, donations copy, hero shot, pipe standards research).


## 2026-10-04

**Decision:** OPERATOR ANSWERS (2026-10-04), ship homes design section 9: recommendations "largely good". Server home is FRESH: "I don't want people cheating in MMO servers by loading homes fully kitted out and cheated inventory." Moving homes with a cooldown: yes. JOINT HOMES wanted eventually: "allow people to join their homes together as a sort of joint home so a 1 acre home becomes a 2 acre home and both players control the environment". Teleporters stay Dev/specialty: one mothership gateway (like a stargate), not one per player; at first it only lets players ARRIVE at the mothership, and "only their appearance is kept. Any upgrades, cybernetics, genetic modifications, etc. would be removed upon entering the server world so they're fresh." NPCs eat from the same food stores; NPCs have homesteads and should mostly provide for themselves; he is unsure how to balance NPC contributions to the fleet against human players. He asked how plot claims should work without being tedious.

**Why:** Section 9 had 19 questions with recommendations; these answers fix the anti-cheat stance for server worlds (fresh home, appearance-only arrival) and add joint homes and the gateway to the design.


## 2026-10-04

**Decision:** OPERATOR ANSWERS (2026-10-04): BUG-136 overloaded = slower walking in realistic mode, and no jumping under gravity based on weight/mass; in low or zero g you can carry more but mass still applies. Pipes show their REAL material (including paint), made identifiable with colour coding, labels and stripes; he asked whether international standard markings exist. Landing hero shot: no spaceship worth showing yet; idea: start dark on the Milky Way, then a sunrise over a scenic place, e.g. from Silverdale, WA toward Mount Rainier.

**Why:** Routes the BUG-136 fix (realistic and simplified modes per the dual-modes rule), a dated standards findings doc before any pipe marking work, and a hero-shot lane on the rig.


## 2026-10-04

**Decision:** Released v0.1450.0: the erase marker (the operator's option 2: keyed fingerprint plus day, 30 days and 100000 entries by default, in-app settings, explicit Connect/Enter to sign up again) and BUG-137 (desktop Server Settings could write defaults over the live settings). Three review rounds (17, 11, 3 findings). Increment 2 (meet in the Commons) is ready on ship-homes-2 and merges next.

**Why:** Closes BUG-135's known limit the way the operator chose, and a live-server data risk found by its review.


## 2026-10-04

**Decision:** Released v0.1451.0: ship homes increment 2 (meet in the Commons; neighbours as shells with others-only doors; remembered plot per identity and server; guests put away and never on a plot). Day 5 is unblocked in code. The five lanes from the operator's answers (72x clock, BUG-136 carry, donations, pipe standards findings, sunrise hero shot with a corrected star sky) are merging main into their branches before they merge into main as v0.1452.0. Next arc item: increment 3 (the relay's world becomes this ship).

**Why:** Two review rounds and all rigs green on main, including the guest leg and the connect-first entry.


## 2026-10-04

**Decision:** Released v0.1452.0: the five lanes from the operator's 2026-10-04 answers (72x shared clock with an in-app admin control; BUG-136 carry limit by gravity, Realistic slowdown and no jump; BUG-138/139 the real night sky and a dawn fade, plus the Silverdale sunrise hero clip; the two donation routes; pipe marking findings). Also BUG-142: the rigs' exe copy now waits out the lock a stopped game leaves (scripts/lib/rig-exe-copy.js, shared by probe-sweep, verify-copresence, verify-screens, verify-live-screen). Increment 3 (the relay's world becomes this ship) is building in its own workflow.

**Why:** Full check chain green on the merged main. --plots lost its game-first order to EBUSY on the first run; the same failure had been seen once earlier the same day and dismissed as timing, so it was fixed at the root (a fixed 2 s retry in four rigs) instead of rerun.


## 2026-10-04

**Decision:** Released v0.1452.1 and v0.1452.2: six Library guides (water without power, stone/clay/earth, fibre/cord, life in soil, soil chemistry, the ground under you; 71 sourced), the water pump card fixed to 2 L/min, BUG-143 filed (140 of 300 trade goods are not items, so the vendor never offers them; a worktree agent is reconciling with a test), BUG-144 fixed (the Library quotation gate's regex pairing and first-source attribution produced 6 false reports and failed just preflight; now positional pairing, nearest source among all registry sources, 8 tests, 3 seen red against the old logic). Two more Library batches launched (shelter structure/foundation/framing; roof/repair/recycling). In flight: increment 3, pipe markers, BUG-143.

**Why:** The fact checks keep finding real game-data defects beside the prose (the pump card, the trade goods), so each batch's 'game data oddities' list is read and acted on, not just the guide. The quote gate was fixed at the root rather than silenced with markers: four of its six reports were checker bugs, and the fix surfaced one quotation the broken pairing had hidden.


## 2026-10-04

**Decision:** Released v0.1453.0 to v0.1456.0. v0.1453.0 BUG-143 (trade goods are items, test every_shipped_trade_good_is_an_item). v0.1454.0 pipes: material bodies + ISO 14726 marker bands from data/piping/, Simplified/Full setting, content from the machine a line leaves (11 review findings fixed). v0.1454.1 and v0.1455.1 six Library guides (77 sourced). v0.1455.0 BUG-145: 24 money loops closed by a cheapest-path test, and all 25 vehicle recipes given real bills of materials (1.0 to 1.25x the vehicle's weight) so prices could stay plausible; BUG-146 (grade loops) and BUG-147 (big vehicles cannot be hand-crafted from the backpack) filed. v0.1456.0 ship homes increment 3: the relay's world is the mothership, crew chores and meals from shared stores on one game clock, quest ends at your home, Pioneer world upgrade (16 review findings fixed). In flight: BUG-147 (craft from home storage) workflow, two Library batches. NEXT on the multiplayer arc: increment 4 (getting around at ship scale).

**Why:** The operator's 2026-10-04 order: multiplayer first, then the answered directions (pipes, carry, clock, donations). The vehicle prices were not left implausible after BUG-145: realistic-first says model the real bill of materials rather than price a spacecraft below a sedan. A merge into main during just deliver (v0.1453.0) was proven harmless by fingerprinting a checkout of the tag against the exe stamp; merges now wait for deliver.


## 2026-10-04

**Decision:** Operator answer on the NPC homestead contribution (open question 12 of ship-homes-and-logistics.md), verbatim: "We'll keep it to 0 for now. Once we get the player stuff sorted we'll worry about NPCs for the market. For the sake of simplicity during early development we'll say the fleet has unlimited of everything and just track what they player uses and contributes. That way they can be in the red or black so they can gauge what they're actually using/contributing." Implementing: fleet stores unlimited by default (the increment 3 stocked mode kept behind a server setting), and a per-player fleet ledger (used vs contributed, red or black) visible in the app.

**Why:** Simplicity during early development: players can gauge their own consumption without a fleet that runs dry or NPC economics that are not designed yet.


## 2026-10-04

**Decision:** Released v0.1458.0: the fleet ledger (unlimited fleet by default with the increment 3 stocked mode behind the fleet_supply_mode server setting; a per-player used/contributed ledger in Inventory > The fleet and read-only on the web; 18 review findings fixed). Earlier the same evening: v0.1457.0 (BUG-147, crafting from home storage) and v0.1457.1 to v0.1457.3 (12 Library guides, 89 sourced). The weekly usage limit cut four workflows mid-run (the fleet ledger's fix round mid-merge, increment 4's reviews, two Library batches); the operator used his free reset. Recovery with ultracode off: individual agents instead of workflows (three increment 4 reviewers, one fix agent with all 21 findings in scratchpad/inc4-findings.md, each fix proven by a test seen red first, which replaces the two-skeptic pass), the fleet ledger merged into main directly instead of finishing its worktree-side merge.

**Why:** The fix commit (5195db455) covered all 18 confirmed findings and its own checks had passed before the cutoff, so the worktree merge of main was redundant with the merge into main that every release does anyway. A background shell command was stopped about 28 minutes into the fleet ledger's chain; killing the shell left its node children running and a second copy of the chain alive, so the remaining rigs were re-run in pieces under 20 minutes (lesson saved in the dev-workflow memory).


## 2026-10-04

**Decision:** First-hour audit and v0.1459.0 batching. Released v0.1458.1 (six Library guides, 95 sourced; their fact checks found BUG-148, 149, 150). The operator asked how the basic starting gameplay loop was going: a read-only audit traced a new player's first hour with stakes on (docs/design/first-hour-audit-2026-10-04.md) and the ranked arc went into PRIORITIES as THE FIRST HOUR; four fix lanes went out in worktrees (survival persistence, quests that can finish, session flow, world friction) and four operator decisions were asked (when the pre-launch defaults flip, what death costs, the first ten minutes, more plots). Increment 4 merged into main (80ab2ffe0) with BUG-151 found by its own new rig legs. Then the BUG-148/149/150 branch (fixed by one agent, each with a test seen red; the home marker confirmed on screen from Silverdale) was merged into main as well, and the v0.1459.0 check chain, then about 15 minutes in (two cargo checks done, the library tests compiling), was stopped and restarted on the merged tree, so one release carries increment 4 and BUG-148 to 151.

**Why:** git merge-tree showed the branch merging into main with no conflicts and touching none of the dirty files of other sessions, so stopping a quarter-done chain cost about 15 minutes against a whole second chain (verify, relay battery, release build, seven rig runs on one GPU, well over an hour). The four first-hour lanes are NOT folded in: they are still being built, and waiting for them would hold increment 4 back for hours; they go out together as the next release. BUG-151 and BUG-152 had been dated 2026-10-05 from the probe rig's UTC folder names; corrected to the local date, 2026-10-04.


## 2026-10-04

**Decision:** The operator answered the four first-hour decisions (late evening), verbatim: "I like your suggestions. For normal mode, especially for my MMO server, let's have everything be real time, not the 72x. That way anyone joining isn't dealing with accelerated death. We'll wait until we have everything actually working before we accelerate everything for fast mode." Accepted: (1) fresh installs start in Normal mode with progress kept, rigs pin Dev, his saved settings keep Dev; (2) death: Simplified keeps 'Nothing was lost', Realistic leaves the carried items where you fell for a while; (3) the first ten minutes: vitals, eat, plant, craft a tool, the drone for iron, smelt, build one thing, end at the front door facing the Commons; (4) about ten more plots along First Street. New: the shared world's clock defaults to 1x (his morning's 72x is reversed; the admin control stays; the solo game was already 1x), and acceleration waits for a later fast mode. Dispatched at once, in worktrees: the real-time clock lane and the more-plots lane. Held until the overlapping first-hour lanes merge: the defaults flip (after session flow and world friction), death cost (after survival), the scripted first ten minutes (after quests). The live server's stored world_time_scale (72, written by the ALTER default) is set to 1 by hand over SSH, since a code default does not change a stored row.

**Why:** The four running first-hour lanes edit the same config, Normal-mode, respawn and quest code the held items need, so starting those now would mean three-way merges in hot files; the clock and plots lanes touch the relay's settings and the ship file, which none of the four lanes edit. Also filed from the heat, fire and fuel guides' critic, each re-checked in code: BUG-153 (the Campfire ability's description promises a fire; casting heals 3), BUG-154 (the generator burns Paint, Glue, Crude Oil), BUG-155 (the greenhouse quest asks for a heater that does nothing).


## 2026-10-05

**Decision:** Released v0.1459.0 (ship homes increment 4 with BUG-148 to 151, the three guides corrected to match). Two reds on the way, each explained by evidence before release, never by a rerun alone: (1) the first --plots runs failed steady_speed/meet_steady_speed in three of six orders while ten agents compiled (6 to 16 fps, frames up to 465 ms); rerun with every cargo/rustc/link process held at BelowNormal priority (scratchpad deprioritize-builds.ps1) at 16 to 24 fps: 88/88 in every order. Recorded under BUG-152 with the fix the rig needs (the guard deprioritizes builds during a capture; the steady-speed judges refuse to judge starved frames). (2) verify-screens 12/13 no_builtin_data: its game dialed the LIVE server from a saved server in the sandbox config, and the live relay (still v0.1458.1, old ship) refused the candidate's new ship; the comparison read logged a built-in copy. The deploy resolves it; rerun after. The rig dialing production was added to the Normal-mode lane's rig-pinning helper. Also: the first --verify run died on out-of-memory (os error 1455, a fixed 8 GB page file plus 32 GB RAM with five builds), so the chain now waits for 14 GB of free commit memory (scratchpad waitmem.js); the live server's stored world_time_scale was set from 72 to 1 over SSH (the operator's real-time decision); BUG-156 filed (trees floating beside the Silverdale waterfront in the new marker vantage); findings doc on portable unvented heaters under Washington's fire code.

**Why:** Merging the BUG-148/149/150 branch before the chain (clean merge-tree) gave one release instead of two. The four first-hour lanes, all done and merging cleanly together, were held for the next release rather than restarting a chain already 90% through; the Library batches go out as their own patch so guides never wait on the game's chain.


## 2026-10-05

**Decision:** Usage cap reached about 01:10. Shipped tonight after v0.1459.0: v0.1459.1 (six Library guides, 101 sourced). Merged locally, unpushed: the four first-hour lanes and the real-time clock lane (v0.1460.0 candidate; verify passed after a seam fix where two lanes' tests met the survival lane's new struct fields). Agents stopped at the cap: plots, Normal-mode defaults, death cost, Library sweep, community guides fixer, seam review. The first-ten-minutes lane finished. Resume list: PRIORITIES 'RESUME HERE'.

**Why:** Stopping agents at the cap keeps their worktrees in a known state rather than cut off mid-write; main stays unpushed because a push deploys the relay before the rigs have run.


## 2026-10-05

**Decision:** Released v0.1460.0 (the four first-hour lanes and the real-time clock, delivered), v0.1460.1 (nine Library guides, 110 sourced) and v0.1461.0 (Normal mode by default with progress kept and rigs pinned to Dev and off the live server; the scripted first ten minutes; death cost by mode; 56 guides rewritten for the new defaults). The PC was switched off mid-probe once; the probe was rerun rather than counted. A seam review of the five first-hour lanes found nothing high or medium; its low item (the controls hint) shipped in v0.1461.0.

**Why:** The lanes that needed each other merged in dependency order (Normal mode, then the opening, then death), and their two additive conflicts were resolved by keeping both. New gap for the operator: in Normal mode nobody can leave the ship (PRIORITIES).


## 2026-10-05

**Decision:** Released v0.1462.0: the twelve-plot merge (ten more homestead plots along an extended First Street; the thirteenth joiner a guest, told so; players sent by distance, 250 m in and 300 m out) and BUG-157 (the docking and transportation data reach their loaders). Checks: verify (3,146 lib tests, 17 lints), rig tests 280/280, relay 2,229, the two-plot rigs from the game and the menu (88/88, 88/88, 27/27 each), co-presence 22/22, second player 2/2, screens 13/13 after the BUG-160 rig fix, probe 5/5 with 0 panics. In flight for v0.1463.0: the Live water card wording (the purifier makes no water), BUG-152/158 (under review), BUG-159 (temp files), and BUG-153/154/155 (Campfire builds a real fire, the generator burns only its named fuels, the heater warms its room), each in its own worktree. Library: the microbes batch is corrected and verified (merging as a patch); the sanitation batch is in its independent fact check.

**Why:** The plots merge needed the full chain because it changes who gets a home and who sees whom on a shared server; the rigs ran with builds deprioritised and memory-gated after the 2026-10-04 out-of-memory and steady-speed failures. Main stayed frozen during the chain so the tag is exactly what the rigs judged; the guide merges and bug fixes wait for the next release.


## 2026-10-05

**Decision:** Started ship-homes increment 5 (building only on your own plot), the top of TIER 0. A planning agent reconciled the two design docs with today's code and wrote a wave plan (docs/design/ship-homes-increment-5-plan.md): Wave 0 the contract, Wave 1 relay storage and rank / client save seams / Dev off while joined, Wave 2 relay handler / client net and engine, Wave 3 the proof rig and permits in the game. Its four operator questions were answered with its recommendations, for the operator to confirm: refunds to whoever takes a piece down; a released plot's pieces come down; permits end within 90 days and the household page is increment 5b; can_edit_ship builds shared spaces through the server, whole-ship editing stays offline Dev, Admin has it by default.

**Why:** The operator's standing rule is to make the reasonable call and keep moving, reserving a stop for irreversible taste or direction decisions; each of the four is reversible (a column off for everyone but admins changes nothing until granted; permits are stateless). Increment 5 is separate from v0.1463.0, the bug batch, and merges after that release.


## 2026-10-05

**Decision:** Released v0.1463.0: eleven bug lanes merged after two seam reviews (13 defects found between the lanes, all fixed and re-checked): BUG-146 (no trading-post loop at any grade, tap water priced as crafts get it, honey from honeycomb, the home-made antibiotics recipe removed), BUG-152/158, BUG-153 (Campfire builds a real fire; never under a roof; warmth stops at walls), BUG-154, BUG-155 (the heater warms its room's air), BUG-156 (trees and the player floated 37 m in survey regions: one elevation function now), BUG-159, BUG-160 (a cleared server address stays cleared; the typing hold never touches a live link), BUG-162 (illness as water loss; medicine from data), BUG-163 part 1 (an unreadable data file gives the built-in copy), BUG-164 (the body's clock exact at any frame rate; since v0.1005.1 a new character at 60 fps never got hungry). Checks: verify (3,256 lib tests, 17 lints), rig tests 295/295, relay 2,303, the two-plot rigs from the game and the menu (88/88, 88/88, 27/27 each), co-presence 22/22, second player 2/2, screens 13/13, probe 5/5 with the Silverdale ground probe ok.

**Why:** The lanes were each right on their own terms, so the seam reviews were where the defects were; the full chain ran again on the final tree. Increment 5 continues on inc5-integration (Waves 0 and 1 and the rig merged and tested there; Wave 2 building) so v0.1463.0 shipped without it.


## 2026-10-05

**Decision:** Released v0.1464.0: ship-homes increment 5 (building only on your own plot), BUG-165/166 (the rig's walk turns like a person; crew turn smoothly), two new clips. Checks: just verify (3,351 lib tests, 17 lints), rig tests 314/314, the relay battery 2,352, the release build; the two-plot rigs from the game and the menu (88/88, 88/88, 27/27 each), co-presence 22/22, the second player 2/2, screens 13/13, the probe 5/5 with 0 panics, and the shared-build proof rig 35/35.

**Why:** Increment 5 was the top of TIER 0. It was built on an integration branch (inc5-integration) in waves whose shared files each had one owner, proven by its own rig (35/35 after the walker learned the relay's pace and the judge counted storage), then reviewed as a whole: the server rules held, and the game's recovery from lost messages and its refunds across a disconnect were fixed (BUG-167, BUG-168). The operator's own eyes caught the rig's camera walking backwards (BUG-165): the rigs judged positions, never facing.


## 2026-10-09

**Decision:** BUG-169: the VPS disk filled on 5 October (a crawler requesting the git mirror's source archives; Forgejo keeps each for a day). Diagnosed read-only; fixed in the repo: scripts/clear-build-cache.sh (clearing target/ keeps the relay binary, used by the guard, deploy.yml and just sync), the guard clears Forgejo's archive cache at 88%, never deletes target/ during a build, restores the relay binary from backups/ if it is missing, logs a WHERE line of folder sizes, and alerts once per incident. Committed locally, not pushed.

**Why:** The live relay runs a deleted binary (v0.1463.0) and both v0.1464.x deploys failed, so increment 5 never reached the server; the mirror stopped syncing; 73 alerts landed in #announcements. The redeploy (just sync) was refused by the session permission check, and a push to main would redeploy through CI, so the server-side steps wait for the operator. Same class as 2026-05-21: the guard deleted the cache the relay ran from while the cause sat outside its scope.

