# HumanityOS: Priorities

> **This is the TACTICAL backlog (what is next, right now).** Its strategic, themed,
> public-facing companion is **[ROADMAP.md](ROADMAP.md)** (the same to-do list, grouped
> by theme with status badges, rendered on the website). Use ROADMAP.md for "where are
> we going"; use this file for "what is the very next thing." Keep them consistent.
>
> **Read this file first if you're picking up work without context.** This is the strict-ranked backlog. The TOP item of TIER 0 is what gets worked on next; everything else waits.
>
> **Update rule:** every session that meaningfully changes scope updates this file before ending. The orchestrator_state.json journal records WHY a decision was made; this file records WHAT comes next. Don't mistake one for the other.

## Active focus

> **>>> OPERATOR FIELD SESSION 3 DIRECTIVES (2026-07-04 late, journaled in
> full in orchestrator_state):** quick batch SHIPPED v0.693.0 (graphite from
> C-class asteroids answers coal-in-space; live V badges; friends-list role
> badges scoped out; P2P test tucked under Dev tools). REMAINING, in rough
> order: (1) FOLLOW-DIRECTION badges (you-follow / follows-you; needs the
> relay to expose both directions to the client). (2) STARTER 1975 CHEVY
> NOVA: the operator's first real-life recreation target; prebuilt in the
> default home (kits.ron entry + starter-vehicle spawn) so driving needs no
> factory chain. (3) TEXTURE BUG: surfaces render as colored LINES not
> splotches/grain -- suspect procedural noise collapsing on one axis;
> investigate shaders. (4) MACHINE INFO-WINDOW OVERHAUL: every walk-up card
> shows relevant LIVE info; assembler gets an infinite-of-X vehicle SELECTOR
> (fixed auto_recipe in RON is an infinite-of-X violation); containers show
> contents; cistern shows volume. (5) VEHICLE BAY redesign: justify every
> machine; bay = dedicated standard-vehicle-sized area (gravity-safety
> justification), select the held vehicle; ties into hangar/mech ZONES;
> 3D printer more justified than an assembler. (6) VOLUME-BASED CONTAINERS =
> material-storage Stage A is GO (slots only for bandolier-likes). (7) GLB
> model pipeline guide (in-app + GitHub) + viewer; GLB for game, STL stays
> for print. (8) STUDIO CHAT LAYERS: HOS channel view on the Studio page,
> then merged YouTube/Twitch/Rumble layers, resizable/collapsible. (9) REAL
> STREAMING PIPELINE: Studio is UI-only today -- capture/encode/RTMP is the
> gap for relay + multistream. (10) STEERING-MODE
> setting: mouse-look / keys-only / hybrid toggle for driving (v0.697 ships
> hybrid: mouse looks, A/D turn the same heading). <<<**


> **>>> SHARED-FILE LIBRARY SHIPPED (v0.675.0, 2026-07-02, Fable 5): the
> operator's "share my .blend phone case / car bushings from my PC" request,
> end to end. `POST /api/upload?share=1` publishes; NEW `GET /api/uploads`
> lists (search + limit); shared files are EXEMPT from the per-user media
> FIFO so a shared .blend never vanishes under later chat photos; chat
> auto-shares ONLY 3D/model formats (.blend .stl .obj .gltf .glb -- photos
> stay private); `original_name` preserved for display. NEW web page
> `shared-files.html` (browse/search/download, in the nav). Smoke-tested
> against a live local relay. page_registry_lint earned its keep on day 2:
> caught `accord.html` missing from PAGES.md. **v0.676.0 HOTFIX rode right
> behind (BUG-046):** v0.675.0's relay crashed at startup on the LIVE DB --
> the new index sat in the schema batch, before the ALTER block adds the
> `shared` column on pre-existing tables; fresh-DB tests/smoke structurally
> can't see this. Fixed + regression-locked with a pre-migration-shape
> `Storage::open` test; ~25 min relay downtime; `/api/uploads` verified
> live on united-humanity.us. **Native follow-up tracked in
> PAGES.md:** in-app shared-files browsing + native chat file-attach parity.
> **Next up (operator's staged vehicle-pipeline decision, logged 2026-07-01):**
> economy Phase 2 -- purchased vehicle arrives as a kit ITEM first (fast to
> test), then factory world-SPAWN after a job finishes, then physical
> transport the player can follow or take over. Before that, the smaller
> remaining threads below are fair game. <<<**

> **>>> ECONOMY PHASE 2 STAGE 1 SHIPPED (v0.677.0, 2026-07-02, Fable 5):
> vehicle KITS. Craft a Pickup Truck Kit / Rover Kit at the workbench
> (steel+iron+rubber, feedable by the Phase 1 drone->smelter chain), click
> Deploy on the item card, and a real Vehicle entity assembles 6 m in front
> of you: body/cabin/4-wheel primitives from data/vehicles/kits.ron
> proportions, persistent across world re-entry AND app restart
> (WorldSave.deployed_vehicles). VehicleSystem registered for the first
> time (deploy arm live; enter/exit/mech dormant until Stage 3). All
> data-driven: a new deployable vehicle = rows in kits.ron + items.csv
> (+ recipe). 8 tests incl. one-kit-cannot-become-two-vehicles + save
> round-trip. Adversarially reviewed pre-commit (2-lens + verifier).
> Operator visual check pending next play session (3D primitives).
> **STAGE 2 SHIPPED (v0.679.0, 2026-07-03):** factory world-spawn. The new
> Vehicle Assembler machine (build palette) auto-runs assemble_rover: home
> stock ingots + rubber become a REAL rover on the pad 3 m in front of the
> machine -- drone -> smelter -> assembler = mine-ore-to-vehicle untouched.
> Vehicle-class recipe outputs world-spawn via CraftingSystem::
> deliver_outputs (shared timed+instant path); full backpack can't stall
> the line; mid-batch machine despawn still delivers at the captured pad;
> machines now carry a Transform (their world pose). NOT the
> ManufacturingSystem route -- one job engine (CraftingSystem) with the
> Phase 1 hardening beats activating a second parallel one. 5 tests +
> data lint. **NEXT: Stage 3 transport** -- the produced/purchased vehicle
> physically travels factory -> buyer (DroneSystem phase-machine is the
> template, camera tp_target for follow, VehicleSystem enter/exit for
> take-over). Also queued: operator visual check of Stage 1+2 primitives;
> the buy-side (market Buy -> factory job) needs the wallet/currency
> decision. **FIELD-TEST FOLLOW-UPS (operator screenshots 2026-07-03,
> partially fixed v0.681.0):** crew grounded client-side -- the REAL fix
> is relay/client LAYOUT ALIGNMENT (relay simulates its multi-deck ship;
> client renders the flat homestead; chore sites need to come from the
> actual home layout); drone dock POPS on launch/return -- wants a real
> docking/undocking sequence; machine labels are static authored strings
> -- consider live label stats fed from auto_craft_status. <<<**

> **>>> OPERATOR DESIGN DIRECTION (2026-07-04 field session 2): UNIFIED MAP.**
> One map to rule them: the main Maps/Cosmos page should show the PLAYER'S
> location (marker next to Earth), and located asteroids should appear on
> that same map -- "everything synced to one thing instead of separate
> systems" (today the mining mini-map on the Inventory page and the Cosmos
> page are disjoint). Design sketch: Cosmos System view gains (a) a player/
> home marker at Earth, (b) the live AsteroidBody entities plotted near it,
> (c) drone-in-flight dot reusing GuiDrone. The Inventory mini-map then
> becomes a shortcut INTO the Cosmos page. ALSO from the session: the
> Garden section of the Inventory page needs a design pass (operator:
> "improve the garden section" -- unspecified, gather requirements next
> play session), and the broader inventory-page restructure remains open
> (nested-container tiles memory has the earlier direction). <<<**

> **>>> FLEET MODE COMPLETE (2026-07-01/02 night, Fable 5): 8 more releases
> in one evening, v0.663.0 -> v0.669.0, built by parallel worktree agents +
> the orchestrator, every branch reviewed/merged/re-verified on main (709
> lib tests green, up from 659 at loop start). Shipped: economy automation
> Phase 1 (ONE drone commission becomes a hammer untouched -- the
> living-ecosystem loop; 5 adversarial-review defects fixed pre-commit);
> web Laws mirror; homestead data gaps #3-#4 (85-crop nutrition bridge +
> component-output/location tables + loaders); WEB governance voting REAL
> (canonical-CBOR JS byte-locked vs Rust via `just vote-kat`); NPC crew
> chores + the first-ever native crew rendering (crew were NEVER visible
> before); the cannot-close civilization panel; the grow-light honesty
> meter (+ real bug fix: batteries counted as 48 kWh/day phantom demand
> EACH); Studio Program/Preview split. **Remaining non-gated:** snapshot
> QA sweep findings (agent still rendering), crop-nutrition Home-page
> integration (compute the food loop from the new data), chore-label
> nameplates, saffron fractional-yield parser bug, studio.rs 13-literal
> theme migration, Studio real transport (multi-cycle). **Gated on
> operator:** Donate payment methods, Mute Server scope, dead-code
> deletion, economy Phase 2 (truck = Item or Structure?).** <<<**

> **>>> AFTERNOON LOOP continued (2026-07-01 evening, Fable 5): four more
> releases shipped, each adversarially reviewed pre-commit where substantive.
> v0.657.0 homestead gaps #1-2 (edible mushrooms in plants.csv, tilapia/
> catfish in creatures.csv). v0.658.0 Studio real mic meter
> (net::voice::mic_level) + FIRST-EVER help_modal adoption (3 topics; the
> native help plumbing had zero call sites until now). v0.659.0 Donate page
> fetches the connected server's REAL funding info (was native-blind,
> web-only); review caught a money-routing bug pre-commit (stale server-A
> addresses shown as server-B's) -- fixed + regression-locked; the fake
> "$350/$1000" progress bar is gone. v0.660.0 native GOVERNANCE GOES LIVE:
> real proposal feed with weighted tally bars + Dilithium-signed vote_v1/
> proposal_v1 submission built with the in-crate ObjectBuilder the relay
> verifies with (7 regression tests incl. relay-storage round-trip);
> review found + fixed 6 defects incl. cross-server stale-proposal voting.
> **Next up:** (a) Laws quick wins (surface the loaded-but-unused
> `categories` as filter chips; BASE/REAL as a real chip), (b) Humanity
> page visual pass, (c) economy automation Phase 1 (time-scale fix first,
> then drone auto-relaunch -> auto-smelt -> craft_hammer proof -- the
> operator's living-ecosystem vision), (d) homestead Phase B gaps #3/#4
> (crop-calorie bridge, component_outputs.ron) + Phase C (grow-light meter,
> "what this cannot close" Home panel), (e) NPC task-AI minimal step.
> Web governance voting = its own tracked item (needs canonical-CBOR JS +
> KAT). <<<**

> **>>> AFTERNOON LOOP, Phase A of the homestead design SHIPPED (v0.656.1,
> 2026-07-01): `data/machines/home_solo.ron` -- the one-person self-sufficient
> homestead from `docs/design/homestead-solo-design.md` (4 solar/2 battery/1
> wind/1 generator, 1 cistern/pump/purifier/tap, 1 air recycler, 2 composters,
> 9 nutrition towers + 1 apothecary + 8 potato beds + 3 oilseed + 2 grain
> trays + 2 mushroom racks + 1 aquaponic tank + 1 grain field + 1 legume
> field + 1 silo + 1 irrigation -- ~2,078 kcal/day indoors alone, ~94% of one
> person's need). Discovered `MachineHome::load` was hardcoded to always read
> `home.ron`, so built the missing selector plumbing too:
> `AppConfig.home_variant` + `machines::home_ron_path()` + a Settings -> Data
> -> "Home Design" (Family/Solo) radio-button UI. 2 new regression tests;
> full verify pass (both cargo checks, 659 lib tests, all 5 lints, doc-links);
> versioned exe built. **Next up (Phase B per the design doc + the loop plan):
> author the 4 flagged content gaps in priority order -- (1)
> `oyster_mushroom`/`shiitake` in `plants.csv` (unblocks `mushroom_rack`'s
> honesty), (2) `tilapia`/`channel_catfish` in `creatures.csv` (unblocks the
> aquaponic B12/omega-3 claim), (3) calorie/macro columns on `plants.csv` or a
> new `data/food/crop_nutrition.ron` (lets the food loop compute from crops
> instead of hand-typed catalog strings), (4)
> `data/self_sufficiency/component_outputs.ron` + `location.ron` + a
> household-size selector data table (turns the design into a computed
> per-loop score). Then Phase C (grow-light meter + a "what this cannot
> close" Home-page panel), then the loop's remaining priorities: Studio
> streaming pipeline, Humanity/Governance/Laws/Donate pass, registering the
> disconnected systems, economy automation Phase 1, NPC task-AI.** <<<**

> **>>> AFTERNOON LOOP RUNNING (2026-07-01, operator AWAKE and actively
> testing HumanityOS live -- different from the earlier overnight loop) --
> see [`docs/history/2026-07-01-afternoon-loop-plan.md`](history/2026-07-01-afternoon-loop-plan.md)
> for the full backlog + safety rules. Read that file FIRST every wake-up.
> Triggered by the operator directly: "enable loop mode to work what's been
> discussed" + "dedicate a subagent to designing a fully fledged
> self-sustaining homestead." A `self-sustaining-homestead-design` Workflow
> (3 research agents + 1 synthesis) is in flight -- its result becomes
> priority #1 once delivered (see the plan doc). Also fixed this turn before
> the loop started: BUG-045 (cloned/mirrored homes in a residential zone had
> no floor/ceiling/trim, only walls -- operator screenshot report) and the
> manual sun-angle override for the construction editor (v0.653.0, operator
> was stuck with unfixably bad lighting since the mothership has no
> orbital rotation simulated at all yet -- a real, separate, larger project
> per the cosmos-architecture design doc). Priority order for the loop:
> homestead design implementation, Studio streaming pipeline, Humanity/
> Governance/Laws/Donate pass, registering 4 disconnected-but-valuable
> systems (ConstructionSystem/ManufacturingSystem/AISystem/OfflineSystem),
> economy automation Phase 1, NPC task-AI minimal step. Explicitly NOT in
> scope without operator input: Donate payment-method list (Patreon
> discrepancy unresolved), Mute Server scope, the ~15+1 dead-code files
> (cleanup opportunity, not yet greenlit for deletion), full ship orbital
> mechanics.** <<<**

> **>>> DAYTIME SESSION (2026-07-01, operator awake, following up on the overnight
> loop's open questions): (1) SkyRenderer REMOVED (v0.651.0) -- operator confirmed
> deletion once told the code had zero external callers already; no visual change
> possible since it was never invoked. (2) Storage architecture / SurrealDB question
> RESOLVED (v0.651.1, docs-only) -- verdict: not a hybrid DBMS, SQLite does 100% of
> real database work, RON/CSV/TOML is a content layer not a second engine; SurrealDB
> evaluated on current facts (BSL 1.1 license, not OSI open source; RocksDB backend
> is a risky C++ dependency given this repo's known Windows linker issues; young 3.x
> line with an open perf-regression issue) and NOT adopted for now -- full reasoning
> in docs/design/storage-architecture.md's new "Is this a hybrid DBMS?" section, so
> this doesn't need re-litigating. The ~133 .surql files the operator recalled were
> confirmed via git history to be pre-rename "project_universe" speculative
> world-knowledge schemas, never wired to any code -- not a prior backend plan. (3)
> Mute Server DESIGN RESEARCHED, not yet built -- see
> `open_questions_for_human` in orchestrator_state.json + the operator conversation
> for the two-phase proposal (build native notification primitives first, since none
> exist; then build tiered mute on top). Awaiting operator's steer on scope before
> writing any code for this one.** <<<**

> **>>> OVERNIGHT AUTONOMOUS LOOP RUNNING (started 2026-07-01, ~8h unattended,
> operator asleep) -- see [`docs/history/2026-07-01-night-loop-plan.md`](history/2026-07-01-night-loop-plan.md)
> for the mission, safety rules, and full backlog. Read that file FIRST at the
> start of every wake-up iteration tonight; it's the durable source of truth
> across context resets. Priority order: (1) chat feature completeness, DONE
> as of cycle 4, (2) livestreaming end-to-end verification, DONE (backend)
> as of cycle 5, (3) a broader stub-completion sweep (now active). Docs
> sync every cycle. On stop: write `docs/history/2026-07-01-night-loop-results.md`.
> **Progress: chat backlog fully shipped (v0.641.0-v0.644.0, see git log /
> journal for detail). Livestreaming backend verified live end-to-end
> (cycle 5, v0.645.0) -- start/stop/viewer-join-leave/chat all confirmed
> correct against a real local relay, EXCEPT a real bug found + fixed:
> BUG-043, `viewer_peak` was fed the live viewer count at leave/stop time
> (only ever highest right at a join, decreasing from there) instead of a
> tracked historical high-water mark -- proved live (2 viewers peak, both
> leave, stream stops, old code would've recorded 0) and with 4 tests
> proven via revert-and-retest. NOT verified: the WebRTC signaling relay
> (simple pass-through, read as correct but not live-tested) and the
> client-side scene-management UI -- logged as a real follow-up in the
> plan doc if time remains later. **Priority #3 (broader stub sweep,
> cycle 6, v0.645.1): two candidates turned out bigger than estimated and
> were NOT force-built** -- `SkyRenderer` (`src/renderer/sky.rs`) is fully
> dead code (never instantiated anywhere; the real sun lighting already
> uses astronomically-real Earth-Sun vectors) and its intended future role
> is a genuine product question, logged in
> `orchestrator_state.json::open_questions_for_human`. `EconomySystem`'s
> deferral is already correctly documented in the lint itself ("needs
> market/credits entities") -- not a quick win, left alone. That
> investigation surfaced a real, high-confidence doc-accuracy fix instead:
> 4 stale "NOT registered, never ticks" claims in FEATURES.md (Weather,
> Atmosphere, Skills, Quests are all actually registered and ticking;
> STATUS.md already had it right), fixed. **Cycle 7 (v0.646.0):**
> `src/systems/navigation/orbital.rs`'s Kepler stub is dead code (zero
> callers anywhere) -- left alone, not deleted. But checking for the
> real math's home found `src/ecs/cosmos.rs`'s
> `body_position_in_system_meters` (the Phase-2 cosmos position
> resolver's `ContainerRef::Body` case) was ALSO a `DVec3::ZERO` stub,
> and unlike orbital.rs this one is real, documented, currently-inert
> infrastructure (no live caller yet -- Phase 3's Cosmos page / Phase
> 4's ship containers aren't built) waiting on exactly the Kepler math
> that already shipped separately in `src/cosmos.rs` (Maps page /
> Sol-system model, v0.262.8). Wired it: now calls
> `crate::cosmos::find_body` + `body_world_position_3d_au` for the
> `"sol"` system and converts AU to meters; unknown system/body still
> falls back to zero (documented). 4 new tests, proven via
> revert-and-retest. No user-visible behavior changed tonight (nothing
> calls this path in the live game loop yet) but it's real progress
> banked for Phase 3+. **Cycle 8 (v0.647.0, BUG-044):** food spoilage's
> data model + tick logic already worked correctly -- the real gap was
> narrower: the EAT handler never checked the `spoiled` flag, so a
> spoiled item could be eaten with full nutrition and zero risk forever
> (cooked/canned/preserved food all has raw_consumption_risk 0). Fixed:
> spoiled food now grants 25% nutrition + guaranteed food_poisoning. 1
> new test, proven via revert-and-retest. **Cycle 9 (v0.648.0):**
> `learning.rs`'s practice-hours `Skill` confirmed DEAD (superseded by
> the real XP-based `SkillSystem` in `skills/mod.rs`) -- left alone. A
> fresh full-repo TODO grep (not just the original list) found 2 more:
> chat's "Mute Server" button needs notification infrastructure that
> doesn't exist yet, logged as a real open question rather than wiring
> a hollow flag; Cosmos page's "Track" button (disabled stub) WAS
> self-contained (the orbital math already existed from cycle 7) --
> implemented continuous camera-follow, 4 new tests via
> revert-and-retest, plus a new `snapshot_cosmos` headless screenshot
> test (the page had none before). Bonus: found `src/gui/pages/maps.rs`
> (591 lines) is ALSO fully dead code -- `GuiPage::Maps` has forwarded
> to `cosmos::draw` since v0.203.2 -- 4th instance this session of
> "superseded file left in place, docs still point at it." Fixed the
> stale FEATURES.md/PAGES.md file pointers. **Cycle 10 (v0.648.1,
> docs-only):** re-checked the plan doc's own "larger/riskier, needs a
> design decision" bucket (8 files) for external callers instead of
> taking the original filing at face value -- ALL of them are ALSO
> zero-caller dead scaffolding (autonomy.rs, blueprint.rs, csg.rs, the
> whole logistics/ and navigation/ trees, physics/fluid.rs,
> physics/collision.rs, psychology.rs, input/{mod,bindings}.rs -- 11
> files, ~250 lines total). None of these needed a design decision at
> all, unlike SkyRenderer/Mute Server; they're just confirmed-dead, a
> safe cleanup opportunity for later, left in place tonight (same
> conservative call made for the other 4 dead-file finds). This closes
> out the ENTIRE original backlog list, both buckets. Only 2 genuinely
> open product questions remain (SkyRenderer, Mute Server), both
> already logged. **Cycle 11:** live-verified the WebRTC signaling
> relay pass-through (`stream_offer`/`stream_answer`/`stream_ice`) --
> 3 bot connections (streamer/viewer/bystander) against a fresh local
> relay confirmed correct unicast routing (bystander got nothing),
> server-authenticated `from` (not client-spoofable), and no
> self-echo. This closes the relay-side half of livestreaming's
> remaining follow-up. What's left (the actual WebRTC media handshake
> + the client-side scene-management UI) needs a real browser/str0m
> peer or the live production relay -- out of scope for the loopback
> harness, flagged for the operator rather than attempted against
> production tonight. This effectively completes priorities #1 and #2
> in full, plus the entire #3 backlog (both original buckets). Next:
> if runway remains, look for genuinely new ground (e.g. a web/
> frontend TODO sweep, since tonight's work was almost entirely
> Rust-side) rather than re-covering closed backlog.**
> **Cycle 12 (v0.649.0, v0.650.0): self-improvement pass.** The web/
> frontend TODO sweep turned up nothing actionable (1 hit total, a
> Tauri-era dead-code TODO in `shell.js` guarded behind a
> `window.__TAURI__` check that's never true post-Tauri-deprecation --
> not worth fixing code that never runs). Instead dispatched an
> independent adversarial-review agent over the whole night's diff
> (`cb089287..HEAD`) before wrapping up -- and it found a REAL bug in
> this session's OWN BUG-044 fix (cycle 8): the spoiled-food slot
> lookup used forward search (`position`) while `Inventory::remove_item`
> actually consumes from the LAST matching slot backward, so a
> fresh+spoiled pair of the same item in different slots could silently
> defeat the whole fix. Fixed (v0.649.0) with a matching reverse search
> + a new multi-slot regression test, proven via revert-and-retest. The
> other 6 reviewed areas were confirmed correct, no changes needed.
> Also fixed a stale v0.283.0 comment in `lib.rs` claiming native has no
> WebRTC stack (it does, shipped in the v0.485-495 arc) -- found while
> cross-referencing STATUS.md (v0.650.0, comment-only). **This is a
> genuinely good stopping point**: both explicit priorities done, the
> full stub backlog closed or correctly reclassified, and a
> self-review pass caught + fixed the one real regression from
> tonight's own work. Next: write
> `docs/history/2026-07-01-night-loop-results.md` summarizing the
> whole night, then stop the loop (~8h target reached; see the
> timestamps in git log from v0.640.1 onward).** <<<**

> **SONNET 5 SESSION CONTINUED (2026-07-01) -- recovered from a repeat clean-worktrees
> incident, shipped all 3 previously-lost features.** `just clean-worktrees` destroyed
> ALL THREE in-flight diffs a second time mid-review (spotlight-cone rendering, the web
> Accord doc browser, and the live screenshot command), this time simultaneously, because
> the first fix was doc-only (a CLAUDE.md warning) and a subagent told to "read CLAUDE.md
> first" read Step 0 literally and ran the destructive cleanup itself. Real fix this time:
> `scripts/clean-worktrees.sh` now structurally refuses to remove a worktree/branch with
> uncommitted changes or commits not merged into main, even under `--yes`; only an explicit
> `--force-unmerged` can destroy real work. All 3 features were rebuilt and SHIPPED: **v0.639.0**
> spot-light cone rendering (real cones, not the point-light placeholder -- `RoomLight`
> carries an optional cone, `CameraUniforms` grew to 672 bytes, every hardcoded buffer
> offset recomputed, verified via a real release-build launch confirming every shader
> compiles clean). **v0.640.0** live in-game screenshot command (drop
> `debug/screenshot_request.json`, get `debug/screenshot_N.png` back within a frame --
> verified end-to-end with a real capture of the live chat UI). **v0.640.0** Humanity Accord
> in-app doc browser (17 governance docs, fixed-allowlist backend verified against 6
> malicious-shaped slug attacks with a real running relay, two-pane web browser at
> `/accord`, the 3 dead GitHub-blob links repointed) -- this one survived a mid-session
> internet outage that killed the harness process; the hardened script protected its
> worktree through the resume, and its solid partial backend work was completed rather
> than redone from scratch. Full verification suite green on the merged result: both
> cargo checks, 624 lib tests, 5 lints, 0 broken doc links. See
> `data/coordination/orchestrator_state.json` recent_decisions for the full incident
> writeup and the CLAUDE.md "known gotchas" entry for the script's new safety model.

> **FIRST SONNET 5 SESSION (2026-06-30) -- docs cleanup + M2c zone population shipped.**
> The three construction forks below are STILL open and unresolved, nothing about them
> changed today. What did happen: (1) a 13-agent reacquaintance assessment; (2) a large
> doc-hygiene + cleanup pass (ROADMAP/STATUS/PAGES re-synced to reality, the OpenClaw
> personal-assistant template deleted a third time from repo root + docs/ai/ +
> docs/reference/ + docs/design/ + docs/network/, 133 dead SurrealDB `.surql` files
> removed, ~180 dead/duplicate files total removed or archived, a live-site OpenClaw
> config leak found and fixed on the public Jekyll site); (3) operator resolved the
> multi-crate question, single crate is final, docs corrected to match; (4) a
> no-backwards-compatibility-debt directive (CLAUDE.md Working norm); (5) the
> game/simulator toggle idea was REJECTED and replaced with a real/fake multi-save
> model + real-life-first boot (TIER 2 item 9 below); (6) **v0.638.0 SHIPPED**:
> mothership zone interior population, residential zones clone the player's home into
> every slot, every other zone type gets a generic tiled filler, two new zone types
> (armory, arena). Not yet visually confirmed in the live 3D viewport, operator should
> eyeball a populated zone next launch. Full narrative in `docs/history/2026-06-30.md`.

> **>>> AUTONOMOUS BULK RUN PAUSED (v0.637, 2026-06-29 night) -- AWAITING OPERATOR BULK-TEST + STEER. <<<**
> Loop mode shipped **9 verified construction/superstructure releases** (all compile relay+native, lib
> tests, 5 lints, snapshots; exes archived): **v0.629** in-view conduit-node placement + drag-port-to-node;
> **v0.630** per-utility usage meters + home self-sufficiency (non-punitive); **v0.631** mothership ZONES
> (M1) -- zone_types registry + wireframe district boxes; **v0.632** conduit node TIERS + service-entrance
> grid-tie; **v0.633** machine ROTATION (yaw); **v0.634** zone interactivity (click/drag/duplicate);
> **v0.635** mothership RAIL node graph (M2); **v0.636** viewport HIDE-per-type (declutter); **v0.637** RAIL
> CARS (animated). The loop then **stopped adding features by design** -- the contained editor backlog had
> thinned to padding (more transit graphs = the same pattern repeated; toggles = trivial), and the genuinely
> valuable next work needs YOUR steer. **Three open forks for the operator:**
> 1. **M1 zone-editor architecture** -- one editor with a zoom/scale switch (mothership <-> zone <-> room)
>    vs separate editors? (`docs/design/mothership-superstructure.md`). Blocks growing the zone editor.
> 2. **M3 civic MALL / meeting zone** -- the social heart: shop stalls (owner + market listing), plaza,
>    transit-hub access. Needs a design pass (ties the market + guild systems).
> 3. **grid S3 multi-home tiers** -- substations aggregating homes -> the fleet grid + zone-level metering
>    (`docs/design/grid-hierarchy.md`). Needs the home->fleet aggregation model decided.
> **Bulk-test the 9 releases when you launch; your feedback (visual tweaks + which fork to take) sets the
> next direction.** The loop is on a long heartbeat (30 min) until you steer; interrupt anytime to redirect.

> **UTILITY TRIO + TELECOM + CONDUIT DEBUG-VIZ ALL SHIPPED (v0.604-623, 2026-06-29).** Power, water,
> air are real at design-time AND runtime with consequence chains (power->water->food->vitals,
> power->air->vitals); the telecom/data utility teaches real media tradeoffs (Cat6 / fibre / WiFi, with
> WiFi RF harming nearby grows); and the build editor now has colour-coded conduit flow visualization
> (v0.622) refined in v0.623/v0.624 (selected-machine-only rainbow flow, static per-utility pipe colours,
> smaller readable beads). **v0.624 fixed the two bugs the operator caught on visual-verify at root:**
> the missing CISTERN TOPS (Mesh::cylinder_capped wound both caps inward -> back-face-culled) and the
> CAN'T-CLICK-MACHINES regression (build-mode entry never rebuilt `machine_pick`; now it does). All
> verified (relay+native compile, 33 machines tests, 5 lints, snapshot).
>
> > **CONSTRUCTION VIEWPORT-FIRST PUSH (operator: "every object needs a proper gizmo; do it in the view;
> > fix the conduit overlap"). Phase 1 shipped (v0.626):** pipes/wires are now CLICKABLE (select a routed
> > connection -> it highlights + the panel gives Remove); conduit support brackets are DEDUPED by
> > position (fixes the overlapping-bracket polygon waste); port handles are bolder. **v0.627: port NODE
> > gizmo redesign** -- the in/out rings became a solid sphere + 4 cardinal arrows (in=input, out=output),
> > and the GRID HIERARCHY vision is captured in `docs/design/grid-hierarchy.md` (home->substation->
> > generator->fleet, non-punitive metering to teach supply/demand). **v0.628: pipes TERMINATE at the
> > matching-utility port nodes.** **v0.629 (build Phase 2):** the pipe GRAPH is built in-view -- "Place in
> > view" drops a conduit node on a floor click, and a dragged machine port can land ON a node (branches
> > onto the main line). **LOOP MODE ENGAGED (operator, 2026-06-29 eve):** keep shipping the backlog
> > autonomously. **Backlog order:** (1) ~~grid S2 metering~~ DONE (v0.630: `utility_meters` per-utility
> > generation/demand/self-sufficiency in the Buildability panel, non-punitive). (2) **MOTHERSHIP SUPERSTRUCTURE**
> > (`docs/design/mothership-superstructure.md`): ~~M1 Zone primitive~~ DONE (v0.631: zone_types registry +
> > Zone on HomeStructure + editor + wireframe render) -> **M2 transit node graphs (NEXT)** (rail multi-
> > stop / elevator shafts / teleporter / cargo tunnels) -> M3 civic MALL/meeting zone -> M4
> > industrial+cargo -> M5 hangar/mech bays. **OPEN FORK (operator):** M1 used a panel+wireframe; the
> > "one editor with a zoom/scale switch vs separate mothership/zone/room editors" question
> > (mothership-superstructure.md) is deferred for your steer before the zone editor grows. (3) **Phase 3
> > trunk hierarchy** -- `ConduitNode.tier` ROUTING (`conduits-node-graph.md` Stage 2; tier EDITING +
> > grid-tie node shipped v0.632, routing still TODO). (4) BULK nice-to-haves: ~~conduit tier editing~~ +
> > ~~service-entrance node~~ (v0.632) + ~~machine rotation~~ (v0.633) + ~~zone
> > select/drag/duplicate gizmo~~ (v0.634) DONE; ~~viewport hide-per-type~~ DONE (v0.636).
> > **Superstructure M2: rail NODE GRAPH shipped (v0.635)** -- topology + editor + gizmo (cars/routing =
> > M2b). **M2b rail CARS shipped (v0.637)** -- animated cars along rail
> > edges. **NEXT loop:** more M2 transit (elevator-shaft node / teleporter edge / cargo tunnel), a zone
> > Hide toggle in the Zones panel, OR M3 civic-mall prototype. Watch the ~8 HomeStructure positional
> > literals on any new serde-default vec field (done 3x: zones, rail). Journal ~134 KB -- rotate near 150.
> >
> > **VIEWPORT DRAG-TO-CONNECT shipped (v0.625):** wiring is now a 3D gesture -- select a machine, drag
> > one of its coloured port handles onto another machine to wire them (the confusing from/to dropdowns
> > are now just a fallback). Array-member machines (e.g. a grain tray) are now movable too (first drag
> > explodes the array into instances).
> >
> > **Build-editor NEXT = conduit TRUNK HIERARCHY (Stage 2 of `conduits-node-graph.md`).** The operator's
> > "moveable main lines + machines branch to them, some paths look wrong" is the Stage-1 node-graph
> > (shipped) limited by per-edge Manhattan routing; Stage 2 (tier 0/1/2 main/sub/subsub + routing that
> > follows the parent line before dropping to the child) is the realism fix. No new data model -- `tier`
> > already exists on `ConduitNode`. Drag-to-connect could also extend to dropping a port on a conduit
> > NODE (not just another machine) once the trunk hierarchy lands.
>
> > **NEXT (open forks -- operator steer, or take the reasonable one):**
> > 1. **detection-sensing implementation** (`docs/design/detection-sensing.md`) -- the big combat-adjacent
> >    multi-modal stealth system (sight/light/RF/smell+wind/sound/seismic). BLOCKED on two operator calls:
> >    the MMO **performance approach** (coarse tick + spatial buckets + analytic falloff vs per-frame
> >    physics) and **HUD-first vs enemy-reactions** scope. The v0.620 `RfEmitter` is one ready channel.
> > 2. **superconductor upgrade MISSION** -- the cable type + bulk-upgrade button exist (v0.616); gate the
> >    room-temp superconductor behind a research/quest so it's earned, not free.
> > 3. **sim-realism-roadmap primitives** (`docs/design/sim-realism-roadmap.md`) -- the remaining gaps from
> >    the 12-agent realism audit.
> > 4. The deferred build-editor polish (rotation gizmo for primitives, viewport hide-per-type).
>
> **BUILD-EDITOR BACKLOG CLEARED (v0.612-614, operator-picked after the wiring arc, 2026-06-29).** The
> object-management trio: **multi-select** + group delete/nudge (Ctrl+click rows, v0.612), **alignment
> snap guides** while dragging (v0.613), and **lock-per-type** (fat-finger protection, v0.614). Deferred
> as low-value/high-effort (logged): a rotation gizmo (machines are primitive shapes, no rot field;
> structures already rotate via `[`/`]`) + viewport hide-per-type (fiddly multi-site render filtering).
>
> > **NEXT (open forks -- operator steer or pick the reasonable one):** the **water->FOOD** chain shipped
> > (v0.611), so power->water->food->vitals runs end to end. Remaining big threads: AIR/atmosphere
> > life-support utility (the 3rd of the energy/water/air trio; integrate the existing AtmosphereSystem),
> > INTERNET/data utility, the **superconductor upgrade mission** + a wire-A-to-B gizmo + per-cable type
> > picker (the `spec` field exists, no UI yet), or the deferred build-editor polish above.
>
> **UTILITY-WIRING + LIVE WATER SIM ARC COMPLETE (v0.604-611, operator "do the wiring; no magic
> transmission; spin up subagents", 2026-06-29).** Power + water are now REAL at design-time AND runtime:
> - **Power (v0.604-607):** `src/utilities.rs` cable physics + `conduits.ron` registry (real NEC copper,
>   superconductor as the upgrade target); machine `ports` + `storage`; buildability **Conduits** check
>   (auto-sizes cheapest copper per run) + **Power-circuit** connectivity check (union-find, every load
>   must reach a generator); **runtime per-island power-flow gating** (`PowerCircuit`, ElectricalSystem
>   sheds per island, no magic transmission).
> - **Water (v0.608-610):** a live **PlumbingSystem** (`WaterTank`/`WaterProducer`/`WaterConsumer`/
>   `PlumbingCircuit`) coupled to power -- the FIRST power->water consequence chain (cut the grid, the
>   well pump stops, the cistern drains). A "Live water" Home-page card. An adversarial review caught the
>   seed topology was inert; v0.610 fixed it (verified fill-when-powered / drain-when-cut).
> - **Docs:** FEATURES.md (was stale since v0.496) + ROADMAP.md + utility-wiring.md brought current.
>
> > **NEXT (the consequence-chain thread, sim-realism-roadmap gap #2):** water->FOOD -- the FarmingSystem
> > already models crop water + dehydration + a `garden_irrigation` top-up that is currently a FREE GUI
> > slider; gate it on actual cistern availability (dry cistern -> crops stop being watered -> wilt) to
> > complete power->water->food. Then: a data/internet utility, the superconductor upgrade mission, and
> > the build-editor backlog (multi-select, rotation gizmo). (`register PlumbingSystem` is DONE -- it ticks
> > against WaterTank/WaterProducer/WaterConsumer, not the old WaterFixture scaffold, which was deleted.)
>
> **STRUCTURAL BACKLOG WAVE FULLY COMPLETE (v0.583-592, operator "proceed until caught up" + "enable
> loop mode", 2026-06-27).** The v0.582 "keep working" feedback wave's structural list AND its
> deferrals, all cleared as one data-driven system (see `docs/design/structure-pieces.md`). The
> autonomous loop (operator away) added the deferrals on top of the directed v0.583-587:
> - **v0.588** -- multi-level foundation: a `Deck` piece + "Place at height" so a deck lands as an
>   upper landing atop stairs; footing sampler uses the player's live height (gated) so it's reachable.
> - **v0.589** -- LADDER CLIMB (hold Space at a ladder, gravity suspended, clamped to span).
> - **v0.590** -- ELEVATOR RIDE (a moving car carries the rider; step on to ride, wait in-shaft to recall).
> - **v0.591** -- CURVED ROADS (Catmull-Rom splines bending through degree-2 nodes, straight at junctions).
> - **v0.592** -- RAIL LINE between paired train platforms (steel rails + ties, deduped).
> - Movement-touching releases (ladder/elevator) each got an adversarial review that caught + fixed real
>   bugs (a blocking deck-rejection, a clamp teleport-snap, a wall-flush drop-out, a jump-cheese regression).
> The home tech demo is now buildable end to end + multi-level THREE ways (stairs / ladder / elevator).
>
> > **NEXT CANDIDATES = the structural REFINEMENTS (operator's pick; not started -- new work, awaiting
> > direction; docs/design/structure-pieces.md):** a ridable moving TRAIN CAR (horizontal
> > elevator-equivalent) + platform-beside-track placement; a glassy elevator DESCENT + a distance CALL;
> > road FOOTING (walk/drive on the surface; marginal on a flat floor); auto-stacking PLACEMENT
> > (click-to-place-on-the-surface-under-the-cursor vs the manual height field); solid-body collision for
> > tall structure pieces. All cosmetic/nice-to-have, none blocking. The directed backlog is DONE.
>
> The directed-then-deferred structural list:
> - **v0.583** -- data-driven `StructurePiece` registry (`structure_types.ron`: wall/stairs/ramp/
>   ladder/elevator/teleporter/train/road) + a "Structure" footer palette (leftmost; "Add wall" moved
>   there) + viewport placement/ghost/bounds-gizmo/select + console `add_structure`/`rm_structure`.
> - **v0.584** -- WALKABLE stairs/ramps/platforms (the first-person ground sampler raises the player's
>   floor to the structure surface under them, step-up capped) + working TELEPORTERS (pair jump + cooldown).
> - **v0.585** -- material LAYERING: `SurfaceLayer` stack on walls (exposed top layer drives colour +
>   `total_thickness`) + `road_types.ron` fixed stacks (footpath/residential/highway/runway) + editor + `add_layer`.
> - **v0.586** -- ROADS as a node+edge GRAPH (`RoadNode`/`RoadEdge`; ribbon mesh per edge coloured by
>   the class top layer; editor + node-ring/edge-line gizmo + `add_road*` console).
> - **v0.587** -- helper widgets on EVERYTHING: machine bounds cubes + conduit-node markers + a master
>   "Helper gizmos" toggle gating the passive overlays (interactive handles always shown).
> - Each release: native+relay compile, lib tests, 3 lints, archived exe, + an adversarial subagent
>   review that caught/fixed 3 real bugs (boxes rendered inside-out; a road-list panic; an untracked-file CI break).
>
> > **NEXT CANDIDATES = the honest deferrals (operator's pick; docs/design/structure-pieces.md):**
> > elevator RIDE + ladder CLIMB (a moving/animated structure-state increment + a destination floor);
> > multi-level landings (upper storeys the stairs connect to); curved road SPLINES + road FOOTING
> > (walk/drive on the surface); the RAIL LINE between train platforms; solid-body collision for tall
> > structure pieces. Each is a focused next increment, none blocking.

> **HOME-CONSTRUCTION REDESIGN -- MAJOR PIECES COMPLETE (v0.532-0.537, operator-directed "build it
> all" push, 2026-06-25).** Node/wall construction: a FIXED outer box (55 x 89 x 3 m steel allotment)
> + freely-designed INTERIOR WALLS placed as segments between corner nodes; same tools for any
> structure; edited equally by an AI (the RON file) and a human (the editor). What shipped:
> - **Stage 1 (data model) v0.532** -- `src/ship/home_structure.rs` (`HomeStructure` + RON load/save +
>   `generate_meshes` -> `HomesteadMeshes`). `data/blueprints/home_structure.ron` = the steel box.
> - **Stage 2a (openings) v0.533** -- doors/windows on walls with data-driven animation STYLE + mesh
>   cutting (piers/header/sill).
> - **Stage 2b (render + editor) v0.534** -- HomeStructure wired into the LIVE render (load_world +
>   rebuild_homestead) + the node/wall editor in `construction.rs` (`draw_wall_editor`): draw walls by
>   clicking corner nodes (chained, snapped, translucent preview), edit corners/height/openings, Save.
> - **Stage 2c (rooms-from-walls) v0.535** -- `HomeStructure::detect_rooms()` flood-fills the floor
>   plan into rooms, live. Plus tested foundations `systems/door_anim.rs` + `ship/conduits.rs`.
> - **Stage 3 (plumbing loop) v0.536** -- `rebuild_connection_objects` routes every connection via
>   `conduits.rs` (up/across/down, copper-potable vs flexible, ceiling hangers + material-aware
>   passthrough gaskets); the wall editor gained Machines + Connections panels.
> - **Stage 4 (animated doors) v0.537** -- `ship/door_panels.rs` + `render_door_panels`: each opening's
>   panel animates by its style (swing/slide/iris/energy/nanowall...), doors ease open on approach,
>   windows are fixed glass.
> - **Stage 5 (position-based machines) v0.538** -- machines in a HomeStructure home position by ABSOLUTE
>   world coords (clamped into the box), never skipped on a stale room id, so they survive wall-edit
>   room-id churn AND the old home.ron machines render (visible at box edges, draggable). `load_world` +
>   `placements()` box-mode branches kept in sync; HUD occlusion -> geometric; garden count -> by stat.
>   Found + scoped by a 5-agent discovery workflow; legacy ship layout untouched.
> - **Stage 6 (clear glass roof) v0.539** -- HomeStructure `roof_material` (default 4 = glass); the
>   ceiling renders translucent in the see-through pass, always visible (a sealed clear roof you see
>   the stars through). Data-driven; opaque roof = roof_material 1.
> - Adversarial reviews v0.534 / v0.535-536 / v0.537 / v0.538 ALL CLEAN (v0.538 verified the two
>   placement copies byte-identical); v0.539 = a low-risk material choice.
>
> > **REDESIGN COMPLETE through v0.539 -- every operator-named item shipped.** Remaining is all
> > operator-gated (a launch-check or a data call), none blocking:
> > - **Operator data decision (OPEN, the one real fork):** the old home.ron machines render but the
> >   v0.538 review confirmed many shipped offsets are negative, so they STACK at the box corner (a
> >   pile, overlapping, near-zero-length conduits) -- visible but poor. Pick: keep-and-drag / CLEAR
> >   for a fresh box (archive first) / I re-author home.ron into a clean positive-coord layout.
> > - **Door-animation FEEL tuning** (open distance / easing / hinge side) after the launch-check.
> > - **Deferred v0.531 review follow-ups (minor, dormant):** object-cap reorder+warn (hologram
> >   truncates before machines when >1024), sphere ghost floor-lift, ghost-over-panel gate.

> **EDITOR-POLISH + MATERIALS + MACHINE-SELECT BATCH SHIPPED (v0.540-553, operator launch-test
> feedback waves, 2026-06-26).** A full wave of build-mode polish:
> - v0.543-548: double-sided walls (kill see-through), CAD dimension overlay (wall lengths + corner
>   angles + feature gaps), door interaction rings + a dev-overlay toggle, native-chat reconnect-loop
>   fix, and THE editor-clickability fix (the full-screen dimension Area was swallowing panel clicks ->
>   rewrote as `ctx.layer_painter`; see memory `feedback_ui_interactability`).
> - v0.549: TAP-VS-DRAG on the corner orbs (click = select + show on the right panel, click-and-HOLD =
>   move); orbs shrunk + dropped to the wall base, clickable through the floor.
> - v0.550: round CORNER COLUMNS fill the cube wall joins (a slim double-sided cylinder of the wall's
>   half-thickness at each >=2-wall join, in the most-opaque meeting material).
> - v0.551: per-pie-slice corner ANGLES on a ground circle (each slice labelled at its midpoint on the
>   floor, raised 10cm; a 2+-wall join shows all its angles).
> - v0.552: WALL MATERIAL picker (pick + render + learn). `data/blueprints/wall_materials.ron` = 8 real
>   materials (steel/concrete/oak/tempered-glass/aluminum/pine/granite/HDPE, real density/tensile/cost/
>   renewable); the wall re-colors per material (per-material meshes; glass -> transparent pass); the
>   panel shows the real properties. Adversarially reviewed (clean).
> - v0.553: MACHINE-IN-VIEWPORT selection -- click a machine in the 3D view (or the list) to select +
>   inspect it on the right panel (type/room/position/power/stats/connections); a ground ring
>   highlights it. Adversarially reviewed; 3 findings fixed (stale pick on the move-fast-path, wall-draw
>   selection exclusivity, array-machine Remove no-op).
> - All native+relay green, lints green, versioned exes archived.
> > **DOOR VISUALS SHIPPED v0.554** -- Opening gained a `locked` state (serde-default; a locked door
> > stays shut, with a "Locked" editor checkbox per door); ENERGY doors are a glowing transparent field
> > (green unlocked / red locked) instead of an opaque slab; NANOWALLS are metallic semi-transparent
> > with a time-driven shimmer (see-through as they dissolve open); each opening's style + lock state
> > floats as build-mode TEXT. Doors now route energy/nanowall/windows through the transparent pass (the
> > panel_motion alpha is finally used). Verified green; the look is operator-confirm (native 3D).
> > **home.ron machine pile RESOLVED v0.554.1 (re-authored, the v0.538-open fork closed):** the box
> > migration read the old room-relative offsets as absolute, piling every machine at the (0,0) corner.
> > A constant per-room shift lifted the 3 clusters into distinct in-box areas (garage x[3,28] z[5,30];
> > garden x[4.5,34] z[37,65]; study x[39.6,43.2] z[24.6]) preserving each cluster's internal layout;
> > buildability + load/round-trip tests pass. Refinable live now that machines are editor-selectable
> > (v0.553), so re-author was the clear best of keep / clear / re-author.
> > **REMAINING (all optional, none blocking, want operator eyes on v0.549-554 first):**
> > - Deeper door polish if wanted: a per-door alpha gradient as a door opens (needs per-door materials,
> >   currently shared), in-PLAY door text (currently build-mode only), an in-world lock toggle.

> **WALL-PHYSICS + EDITOR wave (v0.555-557, operator launch-test feedback 2026-06-26).** Grounded by a
> 6-agent design workflow (corners/physics/thickness/destructibility) + an adversarial critique; the
> implementation-ready plan + critique live in this session's workflow transcript.
> - **v0.555 hull angle** -- a wall ending on the box perimeter now shows its angle vs the hull.
> - **v0.556 COLLISION + per-wall THICKNESS (the run-through fix).** The player IS the camera (rapier is
>   DORMANT -- never stepped), so collision is a geometric SLIDE, not a rapier rewrite: `src/ship/
>   wall_collision.rs` builds thin 2D segments (perimeter + each wall's solid pier spans, DOOR apertures
>   cut so doorways are gaps, WINDOWS stay solid) + `resolve()` pushes the camera XZ out, SUBSTEPPED so a
>   sprint/frame-hitch can't tunnel a 1mm wall (the review HIGH, fixed + tested). Doors collide live
>   (closed/locked block; open+unlocked pass). Per-wall `thickness: Option<f32>` + `shell_thickness` +
>   `default_thickness_m` per material in wall_materials.ron (resolved override->material->0.15), threaded
>   into mesh+collider+room-detect; a Thickness control (down to 1mm) + "auto" in the wall editor.
>   FirstPerson only; third-person + furniture/machine colliders are tracked follow-ups.
> - **v0.557 build-mode AVATAR** -- a draggable teal figure + pyramid gizmo; leaving build mode spawns
>   you at it (seeded at your current spot, clamped to the box).
> > **WALL-MODEL STAGED PLAN (from the workflow, the corner answer to "what better way to fill corners"):**
> > - **Stage 2 SHIPPED v0.558 -- MITER corners.** 2-wall joins cut each end to the bisector (wall_end_miter
> >   intersects the offset edges; a/b-end side flip; degenerate -> square; 3+ joins keep the cylinder;
> >   free/hull ends square). wall_piece builds the prism from the 4 footprint corners; wall_with_openings
> >   lerps the side edges so a mitred end carries through piers/sill/header. 3 geometry tests + an
> >   adversarial review (flushness 0.00000 m all angles, no bugs). Follow-ups (minor): perimeter/hull
> >   corners still square (interior-only); opening jamb skews slightly if placed hard against a mitred
> >   corner; the per-face wall_piece normals point inward (benign via double-siding, commented).
> > - **Stage 3 DEFERRED -- destructibility HP.** Do NOT build until the formula is re-derived against the
> >   REAL 8 materials + proven by an ordering test: the critique caught the draft formula's own numbers
> >   off ~2.3x, "paper" isn't in the DB, and tensile-as-HP scores granite(15MPa) weaker than oak(90) --
> >   needs a toughness_factor/hardness blend (add a column to wall_materials.ron) so brittle-thick stone
> >   resists blunt impact. HP is DERIVED (K*tensile*thickness*area*clamped-density), never serialized.
> >   Gate damage behind an explicit source (weapon/tool), NOT movement collision (a sprint bump must not
> >   delete a wall). Mid-span T-junctions (a wall ending on another wall's FACE) are an unhandled join
> >   class for the miter pass -- resolve before committing the endpoint-snap join model.
> > **WAVE v0.559-567 SHIPPED (operator launch-test feedback, 2026-06-26).** v0.559 miter no longer
> > deforms door/window frames (mitre ONLY true wall ends; opening cuts square); v0.563 fixed "flipped"
> > gizmo normals (overlay pass clears depth + depth-sorts) + smaller orbs; v0.564 door AUTO/MANUAL-open
> > states + window-glass z-fight inset; v0.565 constant-width auto-open LINE ring (drawn like orbit
> > paths); v0.566 mid-span T-junction CLIP (the deferred join class -- a thick wall T-ing into another no
> > longer spears through); **v0.567 door CONTROL PANELS** (a MANUAL door, inert before, gets a
> > wall-mounted panel; walk up in first person + press E to open/close; green/red glowing box; HUD
> > "[E] open/close door"; manual door's open target reads a per-door flag, collision follows it). All
> > native+relay green, ship/door tests + lints green, exes archived; v0.567 adversarially reviewed (4
> > fixes: flag-reset-on-rebuild, panel wall-end fallback, manual-only + no-menu gating).
> > **WAVE v0.568-571 SHIPPED (the big multi-topic feedback batch, 2026-06-27).** v0.568 all gizmo
> > BOUNDS are constant-width LINE circles via a reusable `line::push_circle`/`push_polyline` primitive
> > (docs/design/line-overlays.md saves the idea for grenade-arc / laser reuse) + orbs to 0.05 m; v0.569
> > collapsible nested left panel (walls / machines / utility-lines-by-kind) with Save/Close PINNED +
> > gizmo HOVER states (idle->hover->active) on orbs/cubes/pyramid; v0.570 data-driven door LOCKS Stage 1
> > (lock_types.ron key/keypad/knob/crank/biometric; Opening.locks generalizes locked:bool; control-panel
> > unlock; docs/design/locks.md staged plan); v0.571 data-driven local LIGHTS Stage 1 (light_types.ron,
> > HomeStructure.lights, GI-off toggle) + a fix to a pre-existing renderer CLOBBER (point lights never lit
> > the interior; now the Renderer stores live light state + injects via lit_uniform). All adversarially
> > reviewed; native+relay+lints+tests green; exes archived.
> > **THE remaining big wave item -- NODE-BASED CONDUITS (grounded, deferred, ranked LAST):** the operator
> > wants pipes/conduits as a node GRAPH (main/sub/subsub lines; edit nodes, software auto-routes the mesh)
> > replacing the delete-only connection list. Plan in the ground-construction-systems workflow output;
> > synthesis ranked it last (biggest system, most borrow churn, no urgent pull now the collapsible panel
> > shipped). Build when the operator calls it.
> > **STILL OPEN (operator-requested, none blocking):**
> > - **Locks Stages 2-5** (knob/crank + key-item gating; lockpick/hack; wall-mounted locks; shoot/blow ->
> >   needs destructibility). docs/design/locks.md.
> > - **Lighting Stages 2+** -- real spot CONES + bar AREA shading (shader maths), emissive-surface-as-light
> >   (harvest the energy-door / lock-indicator glow), click-to-place, >8-light culling. NOTE: the v0.571
> >   renderer fix makes point lights + the real sun direction reach the interior for the FIRST time --
> >   launch-test the home still looks right with GI ON.
> > - **Door-content system** -- data-driven multi-PART doors for a premade catalog, custom/stained-glass,
> >   REAL iris doors (sliding petals -- operator: the current iris is "totally wrong"), revolving doors.
> > - **Control-panel actions beyond open/close** -- emergency power, hack (lock/unlock now ships in v0.570).
> > - **nanowall = animated water CAUSTICS** (not the current uniform pulse; shader, ref image given).
> > - **door/wall LABELS** with on-door / on-wall / both placement + a draggable position gizmo.
> > - **Destructibility Stage 3** (HP/physics) -- re-derive the formula first; Locks Stage 5 hooks it.

> **ACTIVE 2026-06-23: HOME-DESIGN AI/PLAYER PARITY arc (operator-directed).** Make the AI's
> home designs use the SAME machinery players build with, so they're inherently player-workable
> + real-world-valid (steel-primary + wood; the homestead enclosed in a steel ship where Earth
> and ship share identical plumbing/electrical). North star + staged plan: `docs/design/home-design.md`.
> - **Stage 1 (machine placement) SHIPPED + HARDENED:** v0.519 place a machine in the selected
>   room; v0.521 x/z/y offset editing; v0.522 a 4-dimension adversarial review fixed 5 real bugs
>   (room-delete orphan cleanup, machine-remove connection pruning, deterministic BTreeMap save,
>   room-aware offset ranges, array-id collision guard) — all in testable `MachineHome` methods
>   reusable by the AI too.
> - **Stage 2 (connections) SHIPPED v0.523:** the per-room panel gains a Connections section —
>   wire a machine in this room to any machine by kind (power/water/nutrient/fuel/air/waste), list
>   + Remove, validated `add_connection`/`remove_connection`. Verified by a `construction` UI
>   snapshot (the panel actually renders) + a unit test.
> - **Stage 3a (buildability validator: power + wiring) SHIPPED v0.524:** the editor's whole-home
>   "Buildability" section computes real kWh/day from the placed machines — is there a power source
>   for the load, does energy balance over a representative day with the battery carrying the night,
>   is the wiring intact — each as a green/amber/red verdict. `MachineHome::buildability_report` is
>   pure + AI-callable. 5 unit tests + the snapshot show it. Seed home reads all-pass.
> - **Live editor 3D preview SHIPPED v0.525:** machine edits (offset drag / add / remove /
>   connect / room move) now refresh the machine meshes LIVE in the editor instead of only on world
>   entry. Fixed the operator's "I can't move the objects in the weird list" -> the offset fields
>   always worked, but nothing rebuilt the mesh, so dragging looked broken. Machines got their own
>   `machine_objects` render list + `MachineHome::placements` (pure, tested position resolver) +
>   `rebuild_machine_objects` on a `construction_machines_dirty` flag. (Live power ECS + connection
>   pipes still resolve on next world entry -- a follow-up.)
> - **home.ron RESTORED + save() hardened (v0.525-0.526):** an in-game "Save machines" had degraded
>   the shipped home.ron (lost 56/59 design comments + ~12 machines + 11 connections) and it got
>   committed incidentally. Restored the authored design from 652bff60; save() now preserves the
>   leading comment header so future saves do not re-strip it. (serde can't keep interspersed
>   comments -- design rationale belongs in the leading header or docs/design/.)
>
> **ACTIVE NEXT -- BUILD MODE (operator-directed 2026-06-24, from launch testing v0.525-ish).** The
> operator wants the construction editor to feel like a game build mode, not a numeric list:
>   (1) **Footer placement palette SHIPPED v0.527** -- a bottom bar with category tabs (Power / Water
>       / Food / Production / Defense / Logistics, with counts) + a 10-wide grid of the types in the
>       selected category; collapsed by default, Expand for more. Data-driven: `category` field on
>       MachineDef + `MachineHome::palette_categories()` (the 26 seed types are tagged). Click an item
>       -> placed in the SELECTED room (lands at center, appears live since v0.525). Snapshot-verified.
>   (2) **Ghost placement SHIPPED v0.529** -- click a palette item to HOLD it (it gets an accent
>       fill + border); the editor renders it as a semi-transparent ghost on the room floor under the
>       cursor; left-click DROPS it exactly where you click (offset from that room center, not the
>       center). Stays held for multi-place; right-click / re-click cancels. Reuses the room
>       floor-raycast (`cursor_floor_hit`). Ghost mesh cached (no per-frame leak). Also removed the
>       legacy garden markers (the 2 non-responsive sphere-towers). Launch-verify (3D).
>   (3) **Live connection lines SHIPPED v0.530** -- connections render as live colored cylinders that
>       follow rooms (replaced the static routed pipes). Trade-off surfaced to operator: simple lines
>       vs realistic routed pipes; awaiting their read on the look.
>   (4) **GPU-leak + stale-held-item fixes SHIPPED v0.531** (from an adversarial review): a HIGH
>       per-frame room-drag leak (renderer was append-only; now has a replace_mesh/update_material
>       reuse API used by the shell/machine/ghost rebuilders), per-edit machine+ghost leaks, and the
>       stale-held-item-across-editor-close wrong-context placement.
>   (5) **NEXT -- EASY PLUMBING step 2 (operator ask):** click machine A then machine B to draw a
>       connection (machine-pick raycast + connect mode + a Wiring palette category), on top of the
>       v0.530 live lines. Pending the operator's read on the line look.
>   (6) **Click a machine to select + drag to move it** in the viewport (machine-pick raycast).
> - **Build-mode review follow-ups (deferred, from the v0.531 adversarial review):**
>   - 1024 object cap can still be exceeded by a very dense garden (a big MachineArray); worse, the
>     truncation drops the hologram/remote-players (pushed AFTER machines) not the excess machines.
>     Fix: reorder the all_objects pushes so the unbounded machines are last + a one-shot overflow warn,
>     or a growable object buffer (infinite-of-X).
>   - The placement ghost isn't floor-lifted for a sphere shape (dormant -- no sphere in the catalog).
>   - The ghost previews even when the cursor is over a side panel (gate on egui pointer-over-area).
>   (7) Future categories beyond machines: Structure (place a room/wall), Furniture -- the palette
>       framework already supports any category; these need their own placement actions wired in.
> - **Deferred (operator's pick when build-mode lands):** Stage 3b validator (water/structure/
>   materials -- a data-model step); Stage 4 unify the model (home-design.md open questions);
>   the v0.522 save-success-toast polish.

> **FOLLOW-UP (from the 2026-06-23 "Too many connection attempts" incident): GRACEFUL
> RELAY RESTART.** Every deploy restarts the relay, which drops ALL client WebSockets at
> once -> a reconnect storm. v0.520.0 raised the per-IP identify limit 10 -> 30/min to make
> that survivable, but the real fix is a relay that hands off / drains connections on
> restart so a deploy never blips active users (and ideally preserves in-memory voice_rooms
> -- see the older voice note below). Also a follow-up: the NATIVE ws_client should back off
> past the 60s window on a "Too many connection attempts" message (the web client now does;
> the native client's x2-from-5s backoff is ~3/min so it's under 30/min, but explicit
> respect-the-throttle is more robust). Until then: avoid pushing many releases in a short
> window (each one restarts the relay).

> **ACTIVE 2026-06-22: INVENTORY REDESIGN (operator-directed) — nested-container TILES.**
> The operator specified a spatial inventory: every container is a card holding its items
> as evenly-sized TILES with its sub-containers nested inside (person -> shirt -> pocket ->
> {pen, keychain, wallet}; house -> rooms -> containers; car -> trunk), with MULTIPLE
> inventories visible so items can be transferred between them and inspected. Full spec in
> memory `design_nested_container_inventory`.
> - **SHIPPED v0.512.0:** `draw_container` recursive renderer (items = tiles, sub-containers
>   = nested cards), tile selection + inspect card.
> - **SHIPPED v0.513.0:** header contents counts + whole-row click to toggle a container.
> - **SHIPPED v0.514.0 — item TRANSFER (organize layer, the operator's chosen model):**
>   `PlacedItem { key, name, qty, container-path }` pool on GuiState, seeded from the places
>   spine (`flatten_placed_items`); non-backpack tiles source from the pool so moves are
>   live; selecting an item shows a "Move to" combo (`collect_containers`) that re-tags its
>   container. Serialize-ready for a save.
> - **SHIPPED v0.516.0 — backpack <-> container transfer** (the ECS boundary): the
>   `inventory_transfer_ops` channel + InventorySystem application; "Stash to" (backpack
>   -> container) and "Take to backpack" (container -> backpack).
> - **SHIPPED v0.517.0 — persistence:** `placed_items` ride in `WorldSave`, so transfers
>   survive a restart (serde-default for old saves).
> - **ARC COMPLETE (organize layer).** Per-container **capacity/weight** is the operator's
>   explicit "later" (they chose organize-layer-first) — do NOT build it unasked. The
>   container-header click is covered by the headless interaction harness; the combo/button
>   interactions are best confirmed once in `just launch`.

> **Dev-experience tooling LANDED (v0.515.0 + follow-ups, 2026-06-22..23):** headless egui
> interaction harness, `verify.yml` CI (green), `just brief`, Windows-safe git hooks,
> `engine_wiring_lint` fixed, agent-status staleness, journal rotation (`just rotate-journal`,
> the journal went 513 KB -> 70 KB). See memory `dev_workflow_tooling`. Deferred audit items
> the operator should weigh in on: a one-command `just release` wrapper (fork-test the
> create-vs-auto-publish race first); slimming CLAUDE.md's finished PQ changelog into
> `docs/history`.
>
> **SHIPPED 2026-06-22 (v0.511.0): MINING MAP reads as a journey** — route line in accent +
> drone labelled mid-trip ("drone · outbound"), fixed the "Hom drone" label collision. The
> "see your little drone going off to mine, with real distance" view.

> **SHIPPED 2026-06-22 (v0.508-0.510): GARDEN EDIT SLIDERS ARE NOW FUNCTIONAL (autonomous loop, native).**
> The garden edit modal was cosmetic -- the per-medium form existed but nothing consumed
> its values. Now: v0.508.0 moved the grow-media into `data/garden/grow_media.ron` (a
> plot-type is a data edit, infinite-of-X); v0.509.0 wired the **water** slider to crop
> survival (a configured tower keeps crops topped up -> healthy/grows, low -> wilts);
> v0.510.0 wired the **nutrient** slider to growth speed (0.5x..1.5x). Pattern: the GUI
> publishes a neutral `HashMap<tower_id,f32>` per frame (`garden_irrigation` /
> `garden_nutrient`), lib.rs bridges it to the DataStore, FarmingSystem reads it -- no GUI
> type leaks into the sim layer. Proven by `per_area_irrigation_keeps_configured_crops_watered`
> + `per_area_nutrient_speeds_growth`.
>
> **NEXT (top of the loop queue): LIVE HOME SIM.** `homes.rs` still shows AUTHORED
> self-sufficiency strings from `home.ron` loops, not the running sim. Make it read live
> `PowerStatus` (ElectricalSystem/SolarSystem/Battery already publish it). The blocker:
> home machines + those systems only spawn/tick after `load_world` (Enter World), not in
> MENU mode -- so the Home page is frozen. Fix = spawn home machines + tick the power
> systems at startup (a startup-ordering change at ~lib.rs:2000/2050). Needs careful
> verify (snapshots + the operator's eye -- 'shows != works' for egui), so it is a focused
> pass, not a fire-and-continue step. Then: grow-light-vs-power meter (depends on this);
> extend per-area sim to soil-bed/field crops (no tower_id link yet).

> **SHIPPED 2026-06-18..21 (v0.485-0.495): NATIVE VOICE CHAT, end to end.** Mic
> capture + Opus + RNNoise + transmit modes (Phase A + input stack), str0m WebRTC
> Opus media (Phase B), per-channel voice rooms interoperable with web over
> `voice_room_signal` (Phase C), and live two-way audio (Phase D). Voice is now
> **per text channel** (the voice room IS the channel, keyed by its id + the
> `voice_enabled` flag), which fixed "clicking the mic does nothing". Defaults:
> Noise suppression + Push-to-talk on CapsLock. Full reference:
> [native_voice.md](network/native_voice.md). ALSO shipped: **headless UI
> snapshots** (`just snapshots` renders egui pages to PNGs so the native UI can be
> reviewed without launching the app) + `just verify` / `lints` / `preflight`.
>
> **NEXT (voice + dev-infra, agreed with operator):**
> 1. ~~In-process WebRTC test harness~~ **DONE (v0.496):**
>    `inproc_webrtc_tests::two_str0m_opus_roundtrip` in src/net/webrtc.rs drives two
>    str0m instances through ICE + DTLS in-process and asserts an Opus frame
>    round-trips. Voice/net media changes are now CI-verifiable.
> 2. **Native per-peer voice controls** UI (volume / mute / squelch), mirroring
>    the web's `web/chat/chat-voice-modal.js`, plus a visible in-call indicator.
> 3. **Web transmit-mode parity** (the web has no open-mic / PTT / VAD UI yet).
> 4. Graceful relay restart so a deploy does not drop active voice (a deploy
>    currently clears the in-memory `voice_rooms` + drops all client WS).
>
> Older queued items (launcher visual verify, two-player co-presence test,
> server-side character persistence) remain below, unchanged by the voice arc.

> **The strategic, public planning doc is now [ROADMAP.md](ROADMAP.md)** (it renders to the app +
> website via `data/roadmap.json`). PRIORITIES stays the tactical "very next thing"; the roadmap is
> "where we are going" + the full themed backlog. Keep them consistent.
>
> **Signing P0 RESOLVED (2026-06-16):** desktop auto-update was dead since v0.421 (no signed
> release); the operator signed v0.470.0 and `just check-signing` confirms it green. Every release
> from v0.421 onward MUST be signed; `scripts/check-release-signing.js` (in `just status`) flags it.
>
> **SHIPPED 2026-06-16:** website parity (v0.469.x), the security patch + signing pipeline verified
> (v0.470.0), multistory editor + structural de-risk spike + web accessibility (v0.471.0),
> **multiplayer co-presence client wiring (v0.472.0)** (NetSyncSystem reuses the authenticated chat
> WS; remote players render as teal avatars), **battery state-of-charge sim (v0.473.0)**, and
> **the character launcher + Game Admin page (v0.474.0)**: Play opens a character/home picker
> (Homes wired; Open-Net/Closed-Net placeholders) with a persisted default that skips the picker and
> a Customize-Look button; Game Admin issues game-world bans that are STRUCTURALLY SEPARATE from chat
> bans (free speech is a right, MMO play is a privilege).
> **NEXT (top of the queue):**
> 1. Operator visually verifies the v0.474.0 launcher + Game Admin layout (run `just launch`; egui
>    can't be auto-verified). 2. The scheduled two-player co-presence test on the VPS (operator's
>    tester, later today). 3. Multiplayer polish for that test: remote-player NAMEPLATES +
>    WORLD-SNAPSHOT PREFILL (a joiner should see already-present players immediately, not only on
>    their next move). 4. Then server-side character persistence (the Open-Net/Closed-Net launcher
>    sections + `character_v1` signed object + `character_policy` per server), and GitHub branch
>    protection. See [characters-and-servers.md](design/characters-and-servers.md) +
>    [first-playable.md](design/first-playable.md).

**ACTIVE 2026-06-15: CONSTRUCTION EDITOR arc (operator-directed detour, paused the LIVE HOME
SIM arc below).** The operator is building the in-app homestead editor by rapid screenshot
feedback. SHIPPED: v0.463 top-down canvas -> v0.466 3D room grab -> v0.467 3-column layout ->
v0.468 door/window slide gizmo -> **v0.469 OPENINGS AS PLACED OBJECTS** (the v0.468 wall-kind
model was unintuitive; redesigned). Now: a door/window/airlock is an additive `Opening` object
(`RoomConfig.openings: Vec<Opening>`) placed on a still-solid wall -- add multiple per wall,
move via a 3D wall-plane handle (door snaps to floor; window free up/down/left/right), RESIZE via
edge handles or numeric W/H, every value clamped to the real wall (the 20m-vs-2m bug is gone).
Also fixed the garden-wall regen bug (Open self-heals to Solid; `neighbor_owns` never hides a
wall that owns a placed opening). Legacy WallKind windows still work + a `promote_walls_to_openings`
path. **NEXT construction slice = LEVEL selector + STACKED-room multistory** (build-order step 2
in `docs/design/construction-architecture.md` (g)) - the big multistory unlock the operator
flagged as a hard requirement (homes several stories; the mall is multiple stories aboard the
ship). Then: Stage 2 SVG/curved cutouts (`Opening.profile` hook reserved), click-on-wall-to-place,
multi-level rooms (the mall). Resume the LIVE HOME SIM arc after the editor reaches multistory.

**ACTIVE 2026-06-13: LIVE HOME SIMULATION arc (operator pick from the content-strategy
survey).** The survey's finding: the home is a beautiful diorama, not a sim. ~270 LOC of
real simulation logic (ElectricalSystem, PlumbingSystem, AtmosphereSystem, VehicleSystem,
ConstructionSystem) is written but ticks for nobody because the entities are never spawned.
The arc: make the home a LIVE engineering sim. **Increment 1 SHIPPED (v0.437.0):** machines
declare an optional `power` role in home.ron (Solar/Generator/Consumer); load_world spawns
them as live ECS entities (PowerGenerator/PowerConsumer/SolarPanel, tagged HomeMachine for
idempotent re-spawn); a new SolarSystem scales solar by sun_factor(hour) and the now-
registered ElectricalSystem sums supply/demand + publishes a live PowerStatus to the
DataStore; the HUD shows a live "Power: gen/use/net W" line that climbs from ~1750 W at 8am
to ~3350 W at noon and falls to the wind baseload at night. **NEXT increments (this arc):**
(1b) battery state-of-charge integrating surplus/deficit over time (the "2.8 days autonomy"
becomes live + the generator kicks in on deep deficit); (1c) the live Home-page loop summary
reads PowerStatus instead of authored strings; (1d) per-machine-card live stats (solar card
shows live watts); (1e) register PlumbingSystem against WaterTank/WaterFixture the same way.
Then arcs 2-6 from the survey: survival stakes, operable machines ([E] runs them), build
mode, enclosed-space climate, vehicles/drone fleet. Full ranked roadmap in the journal
(orchestrator_state.json, 2026-06-13 content-strategy decision).

**(Prior groundwork, v0.427-0.436, all shipped):** 3D home populated with data-driven
machines + LOD labels + [E] interaction; honest closed-loop homestead design + Home-page
closure summary; realistic orthogonal pipe routing with elbows/collars/brackets/valves/wall-
penetration-sleeves (data/routing_rules.ron); mouse-sensitivity boot fix (BUG-038). Operator: populate the home with primitives for ALL machines +
connections, then make it a demonstrable self-sufficient homestead anyone can load and
learn from. **Shipped v0.427 -> v0.433:** data-driven machine layout (`home.ron` +
`src/machines.rs`, infinite-of-X catalog/instances/connections) with `box_xyz`/`pyramid`/
`segment` primitives (v0.427); floating LOD machine labels with status icons + occlusion +
Tab-reveal (v0.428-0.430); walk-up [E] interaction cards (v0.431); closed-loop stats +
Home-page closure summary (v0.432); **v0.433 the honest indoor garden:** a 4-lens design
workflow proved LIGHT (not floor area) caps indoor food, so 1156 m2 of sun-lit garden ~=
a ONE-person diet, and grow-lights to feed 3 would draw 2.5-12x the energy budget. Built
sun-lit: closes B12+omega-3 via aquaponics, honestly offloads ~half the calories + most
fat to outdoor fields. New `arrays` data feature fills any room with one RON line. Lesson
baked into the loop notes + `docs/design/self-sufficiency.md`. **NEXT (operator-flagged by
the design pass, high teaching value): a live GROW-LIGHT-DRAW vs power-budget meter** that
turns red the instant an LED is added past the free pump headroom, the single most honest
teaching artifact in the build. Then: live-sim wiring (loop numbers move with day/night
play), an in-world closure HUD, or the expedition/stealth layer (design-doc filed).

**Sign step pending:** the v0.426.0 release is built + waiting for `just sign-release
v0.426.0` (operator passphrase). Security sprint code-complete; only the GitHub
branch-protection click remains (docs/admin/security-hardening-tasks.md item 2).

---

**DONE 2026-06-13: SECURITY SPRINT TAIL (v0.423.0->.426.0).** Continued + largely
closed the 2026-06-12 audit. A scoping workflow (9 agents) re-verified ALL prior shipped
fixes still hold (SVG-XSS, vault-replay, object/announce quotas, headers, twemoji,
gossip-amplification, future-timestamp, all SOLID by file:line) and surfaced + planned
the open items. Shipped: **(1) object storage hardening v0.423.0** (per-author quota
capped count but not bytes -> MAX_SIGNED_OBJECT_PAYLOAD 256KiB at the put_signed_object
chokepoint covering REST + gossip, 413 on REST; + a size cap on the federation_rate
map); **(2) hard-delete remnants v0.424.0** (PRAGMA secure_delete=ON + wal_checkpoint
TRUNCATE after bulk wipes; honest backup-residual documented; byte-scan test proves
scrub); **(3) member-directory opt-out, FULLY DONE v0.425.0 backend+web + v0.426.0
native** (profiles.privacy directory:"unlisted" honored in get_members/count/get_member
via a shared LEFT JOIN + json_extract; web checkbox + native checkbox & the first-ever
native profile_update send). **REMAINING = operator-only tasks** (no code): /api/send
nginx edge rate-limit (paired VPS zone-def + conf block; LOW value, API_SECRET-gated)
and GitHub branch/tag protection + deploy approval gate, both with exact steps in
**docs/admin/security-hardening-tasks.md**; plus the ongoing `just sign-release vX` duty.
The audit's CODE surface is now closed. **NEXT (operator pick from ROADMAP.md "Right
now"): the next gameplay arc (garden plot-types registry OR First Playable).**

---

**DONE 2026-06-12: DOCS REFACTOR (v0.422.1->.3, docs-only patches on main).** The whole
docs folder was refactored to the operator's brief: (1) all em dashes removed from active
docs (they read as AI-written); (2) stale crypto fixed (Ed25519->Dilithium3 identity,
ECDH->Kyber768 DM, per the CLAUDE.md table); (3) the roadmap is now the to-do list,
`docs/ROADMAP.md` is the single canonical, themed, status-badged roadmap that the website
renders from `data/roadmap.json` (via `scripts/roadmap-to-json.js`); (4) audience-first
structure, `docs/README.md` is the router, with `user/ admin/ ai/ contributor/` folders
each having a who-this-is-for README + clear onboarding (`user/getting-started.md` covers
who/what/when/where/why/how for non-technical readers). docs root went 50->17 files; stale
indexes fixed; `scripts/check-doc-links.js` holds the tree at 0 broken links. **NEXT
(operator pick from ROADMAP.md "Right now"):** the next gameplay arc (garden plot-types
registry OR First Playable arc), or the security-sprint tail (sign each release
[operator-only], member-directory opt-out UI, GitHub branch protection). Going forward,
maintain `docs/ROADMAP.md` as the strategic to-do and regenerate the JSON after edits.

---

**ACTIVE: INVENTORY styling batch (operator on v0.404 screenshot, 2026-06-08), happy with direction, wants polish.** Items: (1) slim ALL buttons, even padding, font-driven height [**DONE v0.405**, universal Button is font-driven + tokenized button_pad_y; steppers slimmed to compact_button_height]; (2) slim the mining +/- [**DONE v0.405**]; (3) "Start collapsed" → a proper bordered button (full area clickable) [**v0.406 NEXT**]; (4) every section collapsible + more defined, with the standard Collapse/Expand/Start-collapsed on ALL nested lists [**v0.406**]; (5) row striping #000000 / #020202 (subtle) [**DONE v0.406**, row_stripe color token → egui faint_bg_color]; (6) move location text inline after the title in parens [**DONE v0.406**, place_to_tree puts it in the label]; (7) INLINE ROW EXPANSION, click an item row to expand in place (picture/3d + full details over rows), click to collapse, instead of a popup/top detail [**v0.408, the big one**]. **v0.407, "Start collapsed" → bordered Button DONE** (widgets::Button.active(); full-area clickable, shows on-state). **v0.408, stripe → #040404 + animated 1px RGB section dividers DONE** (new `widgets::rgb_section_divider` reusing `row::rgb_from_time`, placed between the 4 section boundaries). **NEW BIG ASK (operator on v0.407 screenshot):** a **UNIVERSAL NESTED-EXPANDABLE WIDGET** with configurable columns, to be reused across HumanityOS. **Garden redesign on it:** tower title row = "Aeroponic Tower make/model/version" + row widgets → expand to **SLOT rows** (slot 1..N, plant + simple stats) → click a slot → expand THAT row to a multi-row **card** with image/3D-model placeholder + a **description** + details. Plus: **fix the "Plant this tower" STACKING bug** (replant kept adding 33 more → 66 → 99; the slot model makes it idempotent), and the **garden section gets collapse/expand** (falls out of the widget). **Sequenced:** **v0.409 DONE**, `widgets::expandable_row` + `row_cell` (the universal nesting primitive + configurable-column cell) + TowerConfig make/model/version (in the RON) + the garden tower groups refactored onto expandable_row with a make/model/version title row + Plant button in the header. **v0.410 DONE**, SLOT model: CropInstance.tower_slot; the tower handler fills fixed slots idempotently (despawn-dead-then-refill), killing the 33→66→99 stacking bug (proven by tower_replant_fills_slots_idempotently); threaded through GuiCrop + sync. **v0.411 DONE** (the visible payoff): SLOT ROWS + click-to-expand plant CARD, each tower renders slot 1..N (plantings flattened so slot index == crop.tower_slot) as nested `widgets::expandable_row`s with aligned `widgets::row_cell` columns (Slot# | name | status | growth bar); click a slot → multi-row CARD = new `widgets::placeholder_tile` (colour from new `widgets::swatch_color`) + name + role + description (planting note) + details grid (Stage/Growth/N·P·K/Water·day/Temp/Reservoir/Health) + Harvest/Water/Fertilize; empty/planned slots show tile+name+description + "Not yet planted". New universal widgets `swatch_color` + `placeholder_tile`. Verified: cargo check clean, `cargo test --lib` 398/0, all 4 gui lints pass. (Also added `test = false` to the bin so `cargo test --lib` skips the native bin link, dodges the Windows LNK1318 PDB-limit; full gotcha + the standalone-`rustc --test` lint workaround in CLAUDE.md.) **v0.412 DONE**: all five inventory MAIN sections (Status/Equipment/You&places/Garden/Mining) are now collapsible via new closure-free `widgets::section_disclosure` (painted triangle, memory-persisted, honors the global force); the Collapse-all/Expand-all/Start-collapsed control bar was hoisted to a GLOBAL top bar that drives sections + nested trees + tower groups in one click. Verified: check clean, lib 398/0, all 4 gui lints. **v0.413 DONE** ("the big one"): INLINE item-detail expansion. The tree widget (`container_node`/`tree_list_ex`) now takes an `inline: &mut dyn FnMut(&mut Ui, &str)` that renders an expand-in-place body under the SELECTED leaf; the backpack passes a closure that draws the new `draw_item_card` (placeholder image tile + category badge + details grid + description + Eat/Drink/Plant/Equip/Drop) from an inventory snapshot. Deleted the old top-of-panel detail + its 5 handlers; item actions apply after the panel. Verified: check clean, lib 398/0, all 4 gui lints. **v0.414 DONE**, Mining + You&places on the universal row: asteroids (expand → per-ore composition bars) / drone manifest (expand → per-asteroid availability; steppers stay in the title row) / active drone (expand → fetching+cargo) all on `expandable_row`+`row_cell`; the places tree's BRANCHES now render THROUGH expandable_row (one nesting primitive app-wide) and item leaves get a fixed name column; 3 new theme tokens `cell_narrow_width`/`cell_short_width`/`cell_name_width` (Settings sliders + css regen; garden literals migrated). **v0.415.0 = a STRAY TAG, skipped** (a quoting-mangled commit tagged the wrong commit; per the never-delete/re-tag SOP we incremented past it, no release exists for it, don't create one). **v0.416.0 DONE, retired-page cleanup + THE RELAY-BUILD FIX:** `save_load` was ungated while importing native-gated `persistence`, so EVERY relay build, CI Deploy to VPS AND `just sync`, had been red for 25 straight releases (v0.381→v0.414) and the live relay binary never rebuilt; one-line native gate fixes it (lesson in CLAUDE.md gotchas: check `--features relay --no-default-features` before pushing Rust). Cleanup: play.rs + resources.rs DELETED (with GuiPage::Play/Resources + ResourceCategory plumbing), the standalone onboarding page renderer deleted (quest-chain machinery kept; first boot now lands on the **Humanity Mission Dashboard**; boot-page picker = Humanity + Library; legacy config strings migrate), and Profile's Quests section retired, live sim quests now render ON the top-level Quests page above the learn-by-doing chains (the operator's "one page, two kinds" model realized). Verified per increment: native+relay check clean, lib 398/0, all 4 gui lints + engine_wiring_lint. **v0.417 NEXT (operator-picked 2026-06-11):** the long-deferred garden PLOT-TYPES arc (soil/sand/pots/trays/direct-sow, infinite-of-X registry) and/or the **First Playable arc** (persistence depth → 3D HUD vitals → walk-up stations → death/respawn → guided first day; proposed + plan-filed this session), operator chose "v0.414 + hygiene first", so the next arc pick is theirs; the web Mission-Dashboard mirror stays queued behind "app first".

**INVENTORY one-panel redesign (operator feedback on v0.397 screenshot, 2026-06-08), DONE v0.400-v0.404.** The 3-panel split (left nav / center Mining / right detail, built v0.390 + v0.396) reads as messy/stair-stepped; the operator wants **back to ONE panel** with a clean rows/columns "Excel spreadsheet" layout (aligned widgets), **status bars capped (~200px)**, and **every width a theme token** (universal, editable in Settings, nothing hardcoded). **v0.400, step 1/2 DONE:** 3 theme tokens (status_bar_width 200, stat_name_width 86, stat_value_width 82) wired through theme.rs + theme.ron + Settings sliders + theme.css regen; `stat_row` caps the bar at status_bar_width (was uncapped = the main stair-step); old STAT_NAME_W/STAT_VALUE_W consts migrated to tokens. theme_editor_coverage passes (100% token coverage). **v0.401, ONE PANEL DONE:** reverted the 3 SidePanels into a single CentralPanel via boundary-checked wrapper surgery (1166→1136 lines, no content moved); the default (nothing-selected) view is a clean single column with the v0.400 capped bars. **v0.402, SPREADSHEET GARDEN DONE:** the garden is now a grouped TABLE (one collapsible group per tower with a Plant button; columns Plant|Stage|Growth|N·P·K|Water/day|Temp|State|Actions, growth bar capped at status_bar_width, Harvest/Water/Fertilize inline; respects Collapse/Expand-all). Detail block simplified to item-only; dead tree helpers removed. **The operator's full inventory feedback (one panel + spreadsheet + capped bars + tokenized widths) is addressed across v0.400–v0.402.** **v0.403, GARDEN ROW POLISH DONE** (operator happy with v0.402, asked to tighten): new `widgets::compact_button` (short, tight inline button, fixes the overlapping Harvest/Water/Fertilize; buttons were taller than the tight rows), `status_bar_height` token (5px thin bars for vitals + garden growth), `compact_button_height` token (18px), tighter grid spacing; both tokens editable in Settings. **v0.404, MINING ALIGNED DONE:** asteroids → Grid [Asteroid|Type|Ores]; drone manifest → Grid [Ore|Available|In-manifest with the [-] qty [+] steppers]; active drones → Grid [Drone|Status|Fetching|Cargo] + a thin capped progress bar. **The operator's full v0.402-screenshot polish message is complete (garden rows + compact buttons + thin bars + mining alignment).** **Queued minor:** the selected-item detail still renders at the TOP on item-click (relocate inline if wanted). **Then the big one:** the deferred garden PLOT-TYPES arc (infinite-of-X gardens: aeroponic/soil/sand/pot/direct-sow/trays, data-driven) the operator opened.

<!-- Set this to the single most important thing right now. -->
**ACTIVE: UI / merge-pages arc (2026-06-02).** Operator design-collab from live play, shipping universal widgets + a page consolidation toward a slim **Real | Play | Platform** top nav (operator confirming the bucket mapping before pages move). LANDED: compact `stat_row` table (v0.345), drone capacity manifest + styled square `stepper_button` (v0.346–0.348), **inventory as a uniform container-node `tree_list`** (v0.349, operator greenlit; the SAME widget scales from a toothbrush to a planet), **Studio quick-access in the chat right rail** (v0.350, native mirror of the website's studio widget; survives the nav condense so livestreaming is never stranded), **`section_nav` universal sidebar/TOC widget** (v0.351, generalized Profile's grouped switcher sidebar into `widgets::section_nav` + `SectionNavItem` and refactored Profile onto it to prove universality + DRY out the duplicate helpers; click-action-agnostic so the one widget serves both switcher pages and infinite-scroll TOCs), **chat composer fix** (v0.352, the "… is typing" row no longer shoves the text field under the taskbar in snapped-fullscreen), and the **Earth-rooted container hierarchy** (v0.353, data-driven `Place` spine in `data/places/seed.json`; the inventory now shows your backpack INSIDE Earth → WA → Silverdale → home → You → Backpack via the same `tree_list` rooted at the planet, with Silverdale's real lat/long shown as the terrain bridge; the seed is per-user personal data, deliberately not embedded, distributed builds fall back to flat), and **inventory = entity-first + colour-coded + populated** (v0.354, top level is now ENTITIES [You, your home, your 1975 Chevy Nova] with `location` as an attribute rather than the deep Earth chain; `tree_list` nodes get colour swatches by kind so "what is where" reads at a glance; the home is seeded with a realistic 58-item Mt. Rainier 3-day summit kit in nested containers). and the **`lockable_gate` widget** (v0.355, operator-specified `[title] [show/hide] [unlock] [passphrase]` private-section lock: collapsed + locked by default, real vault-passphrase verification via `decrypt_private_key`, in-memory-only so a restart re-locks; demoed gating the entire Wallet page). and applied to the **seed-phrase reveal** in Settings (v0.356, re-enter the passphrase to show the 24 words; was previously a one-click reveal with no passphrase). Verified that Identity (public DID lookup) + Recovery (social-recovery setup) are NOT private data and correctly left them unlocked. NEXT: **(a)** the **Profile PRIVATE group** (Body & Measurements, Private Notes) is the remaining lock candidate, pending the operator's call on what they consider private; **(b) container arc increment 2** = in-app place editing (add/rename/move rooms + containers + worn-item slots under You, GUI-first) so each user defines their own; **(c)** the **Real | Play | Platform** page consolidation (recommendation delivered to the operator); **reconcile the two tree widgets**, NOT a straight migration (scoped v0.351): the closure-based `widgets/tree.rs` (sole consumer = the Files page) supports **lazy children** (a file browser can't build the whole filesystem eagerly), **per-node icons**, and **colored leaves**, all of which the eager data-driven `tree_list` lacks. So "one universal tree" = unify the ROW STYLING into one shared renderer + co-locate both as two documented MODES of one tree module (eager `tree_list` for bounded trees like inventory; lazy closure `tree_node` for unbounded/dynamic trees like the filesystem), retiring the divergence, NOT forcing Files onto a crippled eager API. **CONFIRMED + IN PROGRESS**, the page carve is **Humanity · Chat · Real · Play · Platform** (operator 2026-06-03; the H button now opens Humanity, not chat). Step 1 (nav regroup into the five + H→Humanity + all bucket decisions) shipped v0.357. Step 2 = fold each tab into ONE `section_nav` page, tab by tab. **REAL FOLDED v0.358** (6 buttons → 1 "Real" tab, Profile's sidebar flattened in + Possessions/Wallet/Tasks/Map/Market). KEY unlock: the **delegate pattern** (the tab renders a SidePanel `section_nav` + delegates the content to the sub-page's existing `draw`, whose CentralPanel renders beside it) means **no per-page rewrite**, so Play/Platform/Humanity fold the same way. **ALL FOLDED v0.360**, the top nav is now the clean **6: H · Humanity · Chat · Real · Play · Platform · Settings** (was ~20 buttons; H opens Humanity; **Settings pulled to its own top-level tab v0.361**, the most-accessed page, never buried). **Design rule** (operator, load-bearing for the rest of the carve): most-used → top-level (1 click); rich-feature pages may keep their own 2nd-level sidebar (the double-sidebar is fine for Map/Crafting, NOT for Settings); big single pages → internal scroll + section TOC; keep top-level tabs ≤~10. NEXT: **Humanity Mission Dashboard built v0.362** (mission + the three scopes + live scoreboard + CTAs + the AI-as-Humanity line, from `docs/design/humanity-page.md`; landing-page quality). **Polished v0.363:** zero em-dashes (now BANNED in all user-facing copy, they read as machine-written), leads with the personal "why" (one family's survival is everyone's, hence CC0), and a single **H/Humanity** tab (the brand H became the tab's icon; the duplicate brand button is gone). **v0.364, the em-dash ban is now enforced APP-WIDE:** the whole native UI, all of web, and the user-facing data files were swept (~1500 occurrences across ~140 files; code comments, dev docs, vendored/generated files, historical message archives, and the backend relay strings were intentionally left), and `tests/emdash_lint.rs` (the `theme_token_lint` analog) now FAILS the build on any new em-dash inside an `src/gui/` string. The web is em-dash-clean too, so the web-landing mirror inherits a clean base. NEXT for it: **(i)** wire the platform-wide **scoreboard metrics** (relay fetch → GuiState: humans/AI onboarded, donations, federation totals, only online-now is live so far); **(ii)** **mirror it to the WEB landing**, the operator's actual goal is for this page to *replace the website's landing page*, so port the same structure/copy to `web/`). **UPDATE (operator 2026-06-05): app FIRST, web later.** Do NOT touch the live landing yet. Get the mission right IN THE APP, then mirror so the experience is consistent (first visit or millionth, web or app). FINDING: the live `index.html` is already rich + mission-led (hero "End poverty. Unite humanity." + a full vision section + the concrete "free water/energy/food" hook), so naively replacing it would REDUCE content; when the app is right, lead the landing with the mission while keeping its concrete hooks. DURABLE PRINCIPLES: the individual ("what I'm doing") AND the civilization ("what we're doing together") must both be instantly identifiable to anyone of any age or state of mind; consistent + predictable + easy to follow; every added PAGE is friction, minimize sprawl ("avoid another dysfunctional US government"). v0.365 sharpened the native Humanity hero to name BOTH scopes (For you / For all of us) as the page spine. **v0.366** (operator feedback on the live build) added the MECHANISM he asked for: a **"Why it's built this way"** card mapping each system (quests+sim, encrypted chat, tasks/notes, maps, marketplace+trust, owned-identity+federation, CC0) to its poverty-ending design, the way the Onboarding page is concrete about it. And **fixed the Resources page** (operator: "barely populated... maybe because we don't have an all category") with an **All** default view + a dense wrapping link grid + a populated catalog (6→10 categories, 14→37 links, led by Water/Energy/Food/Housing). **v0.367** made the onboarding QUESTS universal (operator: the "Lake Tahoe" line is an unverifiable place/event claim with no link, and too US-specific; each quest must read as something that applies *everywhere*, Sahara to tundra) by rewriting `data/onboarding/quests.json` (native + web both load it) to strip the place/event claim, nation stats, and currency, reframing around universal principles + climate-spanning options. And added a **"What it protects"** Mission Dashboard section (the liberty half beside the poverty mechanism): free speech first (operator: "loss of free speech is a death sentence for a free and independent people"), then privacy, ownership, and self-governance, each showing how the *design* defends it. **v0.368** mined the docs (the Humanity Accord + two Explore agents) to deepen the pitch so it shows the platform helps in EVERY scenario: enriched "Why it's built this way" (data-driven/moddable: the tool bends to any place) and "What it protects" (post-quantum + zero-knowledge "even we can't read your DMs", never-locked-out via social recovery, and the keystone **"Bound by a constitution, not a promise"** = the Accord as CC0 law), and added a new **"Built for every situation"** section (no internet / server-down / device-loss / no-money-or-papers / any-language-or-ability / disaster). All claims SHIPPED except the radio mesh, flagged "(in progress)" to stay honest. The Mission Dashboard is now a comprehensive, scenario-spanning manifesto. **v0.369** added an in-app **Humanity Accord viewer** (embedded via `include_str!`, a reusable lightweight markdown reader + "Read the Humanity Accord" button) and an **"Early days, built in the open"** early-access section (states plainly the app is early, flags "in progress", reframes bugs as building-in-public participation). **NEXT TWO INCREMENTS (operator-chosen 2026-06-06 via AskUserQuestion):** **(1) LIBRARY = a new top-level tab** holding all the docs in a nested tree like the inventory (reuses the v0.369 doc viewer; data-driven manifest so docs are added by file, not code). **(2) QUESTS = one page, two kinds**, a single Quests page with BOTH gameplay quests (auto-track + XP) AND learn-by-doing chains (manual check, deeper per-method, e.g. a chain for each way to collect/purify/store water), and **retire the standalone onboarding page** (the Mission Dashboard now covers that ground). **Library SHIPPED v0.370** (new top-level tab; a data-driven docs tree + in-app markdown reader holding the Humanity Accord + 16 companion docs, curated by `scripts/build-library.js` into `data/library/`). **v0.372 expanded it** to a 3-level tree: the Accord docs nest under a **HumanityOS** section, and a **Tools and Websites** section lists the external links (sourced from the shared `resources/catalog.json`, so links live once). Doc entries render inline; link entries open the browser. **Open:** the Resources page now overlaps the Library's links (consolidate, or keep both? operator's call). **QUESTS UNIFIED (v0.373):** one learn-by-doing **Quests** section under **Real** (LIFE group), onboarding chain (First Steps) pinned top, the sim woven into the chain steps as practice (no game/real duplication). The Profile game-quests panel + the standalone onboarding page are retired (the page's hero/concepts overlap the Mission Dashboard; "Get oriented" now jumps to Real › Quests). **Cleanup follow-up:** delete the now-dead `profile::draw_quests` + `onboarding::draw` (and decide whether the in-game quest-registry/XP still surfaces anywhere). **v0.374 batch:** Resources **retired into the Library** (Humanity sidebar item removed; quest text repointed); Library **sections collapsible** + a **search filter** (so the websites scale to 1000+); **Quests** + **Studio** promoted to **top-level tabs** (Quests = the learn-by-doing path; Studio = livestreaming, right of Chat). Nav is now Humanity · Chat · Studio · Real · Quests · Play · Platform · Library · Settings (9 tabs). **Cleanup list grows:** `resources::draw` joins `profile::draw_quests` + `onboarding::draw` as now-dead page renderers to delete in one pass. **DIRECTION SHIFT (2026-06-06):** remove the **Real/Play distinction**; make the survival/production systems (garden, inventory, crafting, market) first-class top-level features that just WORK; add **Crafting** top-level. **NORTH STAR (keep in mind throughout):** every in-game system maps to a real-world system a person can actually build → a **parts list** (3D-print / buy / trade) → and LAST, the app **automates + monitors** those real systems (control layer for your real homestead, e.g. real aeroponics). Design the game systems NOW as real buildable things. (This supersedes the old "Real vs Sim = separate pages" firewall for now; real/sim separation returns last.) **REORG PENDING:** operator unhappy with the cramped Real page; I proposed Profile | Home(homestead) | Crafting and asked him to pick the granularity before building. **v0.375** did the **Library refinement** he asked for: websites are now a single **External Resources** card list (full-width cards, search + tag-filter chips, scales to thousands) and clicking one opens an **in-app detail page** with a "Load website" button instead of launching the browser immediately (an in-app browser is the noted future enhancement). **Decided next:** **Inventory** must be a **top-level page** and **Crafting** its own page "for now". **Still designing with the operator:** the inventory page layout (he wants nested-tree-left + details-right; unsure 2 vs 3 panels), what combines elegantly into inventory (crafting GUI is nearly identical), and the full Real/Play removal + Home/Profile grouping. **v0.376 shipped NAV REORG step 1:** Inventory + Crafting are now top-level tabs (the one-item Play fold is retired; nav = Humanity · Chat · Studio · Real · Quests · Inventory · Crafting · Platform · Library · Settings, 10 tabs). Kept nav-only/low-risk: dead Play left for the cleanup pass, Real keeps its Possessions shortcut for now, and the inventory internals (the 2-vs-3-panel rebuild) untouched pending the operator's panel call. **NEXT for the reorg:** the inventory-internals rebuild (a shared tree→contents→detail widget Crafting reuses; my recommendation = 3-pane file-manager style, garden as a container node) once the panel direction is set, then the full Real/Play removal + Home/Profile grouping. **v0.377 shipped the PLAY button** (operator: "add a dedicated button for the FPS game part ... Click Play to start FPS game mode"), FPS mode IS `GuiPage::None` (nav + pages hide, cursor grabs), so Play is a nav button → `None`, leading the game cluster (Play · Quests · Inventory · Crafting), with a play-triangle icon; Esc still toggles back. **HOMES-AS-PROFILES model ADOPTED (operator, 2026-06-07; new `docs/design/homes-as-profiles.md`):** each **home is a save profile** rendered by ONE shared homestead UI, differing only by a **kind** toggle, **Offline** (local sim; the first offline home = the 100%-self-sustaining **Fibonacci** design = the gamified blueprint others copy), **Server** (multiplayer relay), **Real** (bound to real monitoring/automation hardware; the north-star control layer, built LAST). Homes are built from **designs** (first = Fibonacci). This **dissolves the Real/Play firewall**, real-vs-game becomes a property of the home you're in, not a page split; Play enters the world for the *selected* home. **Sequenced:** Play DONE → home model + home-select → offline Fibonacci design → server homes → parts-list-from-design → real homes LAST. The save-model rework is NOT built yet (open forks in the doc: can a home change kind? design-vs-home split? where home-select lives? what a Real home shows, operator's architecture call). **v0.378 shipped a big operator batch (from the live build):** Play moved leftmost; **Real renamed to Profile**; **Tasks** + **Map** promoted to top-level tabs (Tasks right of Quests; Map = Cosmos); nav is now **Play · Humanity · Chat · Studio · Profile · Quests · Tasks · Inventory · Crafting · Map · Platform · Library · Settings** (13). Profile's sidebar trimmed to Profile-sections + Wallet + Market; **Possessions/Tasks/Map removed** (now top-level), **Streaming moved into Studio** (public channel URL + LIVE flag). Added a **profile selector** (one "Base" character for now). **Library External Resources cards are now self-contained:** a **"Load website" button at the TOP** of each card + all the data (title/tag/desc/url) below; the click-to-detail gate is gone (only the explicit button launches). **Chat/Studio persist-load:** documented as already-true (state lives in GuiState; nothing is torn down on nav; the forward rule, real stream pump runs in the engine, not the page, is in studio.rs's header). **CHARACTER MODEL adopted** (homes-as-profiles.md new Characters section): one **base character** (real self, typed once) + per-server **augmented** versions (shared base look) + **shared-vs-locked inventory** (Diablo II offline/open-bnet vs closed/ladder; servers can force starter gear). Character (who you are) enters a Home (where/what world). The character data-model (multi-character saves, shared/locked stash) is captured with open forks, NOT built. **v0.379, HOMES increment 1 (operator: build it + "I like your suggestions" + "keep developing offline, multiplayer after singleplayer works"):** an Explore survey found the Fibonacci homestead ALREADY exists as rich data (`data/blueprints/fibonacci_homestead.ron`, 13 rooms F1→F233 with materials + power + water), so I **surfaced** it rather than authoring it. New `GuiPage::Homes` + `pages/homes.rs` (a "Home" top-level tab, paint_house icon, by Profile): a read-only **Design browser** with a **scale selector** (Solo/Family/Community/Colony) showing, per scale, the total power + water **demand**, the aggregated **bill of materials** (#3), a **self-sufficiency summary** (solar/wind/water/greenhouse/composter counts = #4 partial), and rooms by tier. Structs + `load_homestead_design` (RON) in `gui/mod.rs`. **Improvements ADOPTED** (operator-approved) as the direction: #1 base character = the existing crypto identity (no second "you"); #2 home kind = who owns the truth (you/server/sensors); #3 BOM-in-blueprint (done); #4 closure score (started; exact output-vs-demand needs generation-capacity data = next layer); #5 forkable/signed CC0 designs; #6 Real home = digital twin; #7 progressive disclosure. **NEXT (offline):** increment 2 = the Home **save-wrapper** (`WorldSave` gains `kind=Offline` + `design`; minimal home list; Play enters the selected home) + ground the base character in identity (#1); then the closure-score data layer (#4). Server/Real/character-augmentation stay deferred until offline singleplayer is solid. **v0.380, self-sufficiency model doc + save-wrapper model (operator: "what variables are there to consider?" + "start the save wrapper"):** (1) new `docs/design/self-sufficiency.md` breaks down homestead self-sufficiency as **coupled loops** (energy/water/food/waste/air/thermal/materials), each supply/demand/storage/loss over time, gated by **location+climate, scale, autonomy margin, time horizon, loop-coupling**; key truths: energy is **Wh+storage not nameplate**, self-sufficiency = the **limiting loop** (Liebig's minimum); proposes a buildable **score** (per-loop supply/demand ratio + autonomy-days + overall = the weakest loop, gated by place+household, same metric sim/real). (2) **Save-wrapper MODEL**, finding: the game has **no working save/load** (persistence is test-only; entering regenerates the homestead fresh). So built the model now, lifecycle next: `WorldSave` gains `kind` (default "offline") + `design` (default "fibonacci") via serde defaults (zero migration), a `new_offline` constructor, 3 passing tests (incl. legacy-JSON-defaults). Home page frames the design as **your offline home** (one home; progressive disclosure). **NEXT (offline):** increment 3 = the **save/load lifecycle**, extract live state (inventory/skills/time/crops/constructions) ↔ WorldSave, load the active home on world-enter, auto-save on exit (the game persists *nothing* between sessions today, so this is real value); then the self-sufficiency **score** data layer once the operator reacts to the variables doc + sets the editable component-output numbers. **v0.381, SAVE/LOAD LIFECYCLE (homes increment 3):** offline progress now **persists between sessions** (it persisted *nothing* before, `persistence` was test-only). An Explore survey found the tick is **ungated** (systems run in menu loops + 3D), so the ECS player is authoritative from startup. New `src/save_load.rs`: extract/apply player **inventory + skills** ↔ a single `offline_home.json`; **applied at startup** (which also makes exit-save safe, the player carries the loaded state, so a no-play session round-trips instead of overwriting empty); **saved** on window-close + a **periodic** self-throttling save (robust to in-app-quit/crash). Scope = inventory + skills only (lowest coupling, no glam/schema change); **deferred** health/position/game_time (TimeSystem owns its clock)/vitals/crops/quests → reload = "wake rested at home with your stuff + skills intact". 2 tests pass (round-trip + offline-kind guard). **NEXT (offline):** extend persistence to health/position/game_time, then vitals + crops (needs a no-double-spawn guard) + quests; then the self-sufficiency score data layer. **v0.382, AEROPONIC TOWERS increment 1 (operator new task):** two curated **50-plant aeroponic tower** configs as data (`data/towers/aeroponic_configs.ron`), the homestead's **food loop** made concrete. An Explore found every target species already in `data/plants.csv` (128 plants) → zero new plant data, pure curation; and nutrition isn't a runtime sim (satiation/hydration only) → the towers document their design. **Tower 1 "Daily Greens and Beans"** (nutrition): 17 species (greens/brassicas/legumes incl. soybean protein/fruiting/roots/allium) with a `covers` list + an HONEST `gaps` list (bulk calories, fats, B12, D, omega-3 = field crops/ranch/sun/animals, not a tower). **Tower 2 "Remedy and Flavor"** (apothecary): 21 herbs with culinary + **traditional/folk** medicinal notes + a disclaimer (not medical advice; per-plant cautions: comfrey topical-only, valerian sedative, st_johns_wort interactions, feverfew not-in-pregnancy). New `TowerConfig`/`TowerPlanting` + loader + a collapsible **Aeroponic towers** section on the Home page (per-tower covers/gaps/disclaimer + the 50-slot planting list). Test asserts RON parses + both sum to 50. **NEXT:** increment 2 = the **3D placeholder** (cylinder Mesh helper + a placed tower entity + simple plant markers), then increment 3 = **farming integration** (plant a tower → crops grow → harvest → nutrition). **v0.383, increment 2 DONE: MAX-VARIETY reframe + 3D placeholder.** Operator: "max variety, one of each (1 lettuce + other types), make sure they grow together, excited for the placeholder." Reframed both towers to **maximum variety** (33 distinct food + 24 distinct herbs, one of each; capacity stays 50, room for the community). Captured the **compatibility insight**: aeroponics shares a reservoir + air, NOT soil, so soil companion/adverse rules relax; the real constraint is a shared **reservoir pH + temp/humidity window** (which widens the variety), a check computing it from `plants.csv` is the next feature. Built the **3D placeholder**: `Mesh::cylinder` + `Mesh::sphere` helpers + a grey cylinder per tower + a green sphere per variety in a helix, placed on the **garden** floor (markers capped at 12/tower; render is crash-safe via the 256 soft-cap). Proposed improvements: tie the tower to the self-sufficiency loops (sum `water_liters_per_day` + harvest timeline from `growth_days`), community tower "recipes", staggered planting. **v0.384, increment 2b: fixed inverted cylinder normals + DATA-DRIVEN geometry.** Operator (screenshot): "normals inverted, seeing the inside; design towers + amount dynamically; wide diameter + adjust helix density like coarse/fine bolt threads." Fixed `Mesh::cylinder` to use **row-major rings + the same winding as the working sphere** (now outward-facing; dropped the inverted caps, it's a solid-walled open tube). `TowerConfig` gains **`diameter_m` / `height_m` / `helix_turns`** (serde defaults); the RON sets them per tower to show it off (nutrition wide+fine 0.6/2.4/6, apothecary narrow+coarse 0.32/1.9/2.5). `load_world` reads each tower's geometry → per-tower cylinder + one marker per **curated variety** (dynamic count) along the helix. **NEXT (increment 3):** the **compatibility check** (plant pH/temp/humidity shared window + outlier flags) + the **farming hookup** (plant a tower → CropInstances grow → harvest). Deeper capacity-from-geometry math waits for the planting loop. **v0.385, towers in the inventory (operator confirmed the winding fix worked + flagged "not seeing the towers in the inventory").** The towers now appear in the inventory's **"You & your places"** tree under **Home** (matching the container model: a tower is a structure holding plants), `inventory.rs::towers_tree_node` builds an "Aeroponic Towers" node → each tower (name + plant count + height) → its planted varieties. Display-only for now. **v0.386, increment 3: FARMING HOOKUP (plant a tower → crops grow → harvest).** Operator: "keep developing the rest of the features." The towers are now **functional**, reusing the existing gardening loop. An Explore found the GUI→ECS bridge (GuiState flag → DataStore Mutex channel → FarmingSystem drains) + CropInstance growth/water/harvest + the Garden render are all reusable. Built: `GuiState.pending_plant_tower` → a new `plant_tower_request` channel → a FarmingSystem handler that spawns one CropInstance per tower planting (dev-friendly: **no seed cost** yet, to get it working) → the crops auto-mirror to `GuiState.crops` and render in the inventory **Garden** with the existing Water/Harvest/Dev-grow controls. A "**Plant a tower:**" row of buttons in the Garden section drives it. So: plant tower → ~33 crops appear → grow → harvest into inventory + Farming XP, with zero new growth/harvest code. 4 farming tests pass (incl. the full loop). **NEXT:** the **compatibility check** (plant pH/temp/humidity shared window); a `tower_id` on CropInstance so the inventory tower nodes show "N/M ready"; then the seed economy + the real-world parts list. **v0.387, FIX: plant-a-tower did nothing.** The `plant_tower_request` channel got registered only in the **test** `make_store` (the `data.insert` anchor matched the `#[cfg(test)]` helper first), not the **runtime** (lib.rs ~984). At runtime the channel was absent → nothing spawned. Fixed by registering it in the runtime next to `plant_request`. The rest of the loop is ungated (the menu garden works without entering 3D). **Lesson logged:** a new DataStore channel must be added to BOTH the runtime (`lib.rs` resumed) and the test `make_store`; `replace_all:false` hits the test first. **v0.388, GARDEN crop list = compact table (first slice of the operator's inventory redesign).** Operator confirmed the farming loop works + asked to redesign the crop list: nested/collapsible-by-tower, single row not three, fixed columns, ~200px progress bar (full width is rough), upgrade the inventory page to left+right panels, maybe add NPK/nutrient/water/temp columns. Shipped the highest-value low-risk slice: the Garden crops are now a compact **egui::Grid table** (one row each, fixed aligned columns Plant/Stage/Growth/Water+Health/Actions, 200px bar). **v0.389, GROUP BY TOWER + COLLAPSIBLE (the operator's lead request done).** Added a crop→tower linkage (`tower_id: Option<String>`) to CropInstance + GuiCrop, threaded end-to-end (`pending_plant_tower`→`(tower_id, plant_ids)` tuple + the `plant_tower_request` channel [registered in BOTH lib.rs runtime AND farming make_store, the v0.387 lesson] + the FarmingSystem tower handler tags each spawned crop + lib.rs sync mirrors it). The Garden render now groups crops by tower into an `egui::CollapsingHeader` per tower (title = tower-config name + "N/M ready", default_open, id_salt) each holding the v0.388 compact Grid; seed-planted crops fall under "Other crops". cargo check clean, release exit 0, emdash 2/2, farming 4/4. **v0.390, INVENTORY = LEFT nav + RIGHT detail + CENTRAL workspace (the operator's "at least left + right panel" done).** The vitals/equipment/tree/garden/mining were all crammed into one shared CentralPanel+ScrollArea (one tall column); rewrote the panel boundaries into a resizable left `SidePanel` (status + equipment + the "You & your places" tree) | the existing right item-detail panel | a central workspace (Garden + Mining). File-manager shape: structure left, act in the center, selected-item detail right. All FOUR gui lints green, farming 4/4, release exit 0; inventory.rs diff a tight 30 lines. **LESSON (load-bearing):** do NOT run `cargo fmt` in this repo, it is not maintained rustfmt-clean, so a whole-crate fmt churned 242 files AND moved a trailing `// theme-exempt:` comment off its line (line >100 cols), silently breaking theme_token_lint; caught via `git diff --stat` before pushing, reverted all fmt-only files, re-applied by hand. Match surrounding style manually. And when touching `src/gui/` or `src/renderer/`, run ALL FOUR style lints (emdash, theme_token, theme_editor_coverage, icon_glyph), not just emdash. **v0.391, NPK + Water/day + Temp COLUMNS on the crop table (the redesign's last "maybe" done).** The data was in plants.csv but dropped at parse; extended the farming data model (PlantRow + PlantDef + from_csv) to parse nutrient_n/p/k, ph_min/max, temp_min_c/max_c, humidity_min/max (all serde-default), surfaced N/P/K + water_per_day + temp into GuiCrop (via the lib.rs sync's existing PlantDef handle), and rendered three compact columns ("N·P·K", "Water/day", "Temp") in the grouped Garden Grid. ph/humidity parsed too (not shown yet) for the compatibility check. farming tests 5/5 (added a full-row parse lock), all 4 gui lints green. Width watch: 8 columns + 200px bar is wide; fits on a large monitor + resizable left panel, move needs→tooltip/detail if it clips. **The operator's entire inventory-redesign message is now fully shipped (v0.388 table → v0.389 group-by-tower → v0.390 left+right panels → v0.391 needs columns).** **v0.392, TOWER COMPATIBILITY CHECK done (the "make sure they grow together" feature).** Aeroponics shares one reservoir + air (not soil), so the constraint is a common pH/temp/humidity window. New `TowerCompat` + `compute_tower_compat` (gui/mod.rs) intersect each axis across a tower's species (Some window = shareable; None = conflict, naming the binding plants), cached in `GuiState.tower_compat` (computed once in the lib.rs crop sync), rendered per tower on the Home page (green ✓ shared-window line, or ⚠ per-conflict lines + a split hint). Tests 2/2, farming 5/5, all 4 gui lints green. **The aeroponic-tower feature is now end-to-end: curate → Home browser + compatibility → 3D placeholder → inventory tree → plant/grow/harvest loop → grouped crop table with NPK/water/temp.** **v0.393, TOWER REAL-WORLD PARTS LIST done (the north-star game→real bridge).** Each tower now carries a data-driven `parts` bill of materials (TowerPart {name, qty, source, note} on TowerConfig; ~9 standard tower-garden parts per tower in the RON, scaled by geometry), shown on the Home page under its plantings, framed as a refinable starting list (the operator/community tune the values via the data file). Tests assert ≥5 parts each with a source; all 4 gui lints green. **v0.394, TOWER SELF-SUFFICIENCY NUMBERS:** each tower shows its total daily water draw + harvest window on the Home page (folded into compute_tower_compat; uses PlantDef water_per_day + growth_days). **The aeroponic-tower feature is now COMPREHENSIVELY end-to-end: curate → Home (browse + compatibility + water/harvest + parts list) → 3D placeholder → inventory tree → plant/grow/harvest → grouped crop table with NPK/water/temp.** **OPERATOR FEEDBACK BATCH (2026-06-08, with screenshot of v0.394 inventory):** three directed asks, **(1) Creative/Survival mode**, default Creative during early dev, so the seed economy can be built without needing seeds yet [**DONE v0.395**, GuiState.creative_mode default true, bridged to a DataStore flag farming+crafting read to skip resource requirement+consumption; toggle in the inventory left panel]; **(2) the GARDEN belongs in the LEFT nav tree, not the central panel**, I put it wrong; clicking any entry (inventory item OR garden plot/crop) should show its details in the RIGHT panel [**DONE v0.396**, garden is now a "Garden" tree section in the left panel (towers=plots → crops); the right panel renders crop/tower/item detail by selection; the central panel is Mining-only; the tree widget now allows selectable container headers so a planted tower can be picked; the 8-column table is retired]; **(3) collapse/expand-all buttons + a "default collapsed" checkbox** for the nested lists (inventory + garden) [**DONE v0.397**, `widgets::tree_list_ex(default_open, force)` + a Collapse-all / Expand-all / "Start collapsed" control bar driving both trees; trees_start_collapsed defaults true so they start collapsed every launch (no GUI-pref persistence exists yet, so collapsed-by-default is how "start collapsed on first load" is honored)]. **The operator's three-ask batch is COMPLETE.**

**SEED ECONOMY arc (operator-directed via AskUserQuestion, 2026-06-08).** **v0.398, step 1 DONE:** 47 new seed items in items.csv (one per distinct tower variety; generated from plants.csv names); SURVIVAL-mode planting consumes seeds (tower handler consumes one seed/variety, skips unseeded; individual plant already did); CREATIVE (default) is free; a "Dev: stock seeds" button grants the **one-seed-of-each starter set** so survival is testable. Seeds are **plot-agnostic**. **★ BIG OPERATOR VISION (durable, drives the next arc): GARDENS ARE INFINITE-OF-X.** Aeroponic towers are ONE plot type. He wants soil beds, **sand** (he has sand at home, wants to recreate it), pots "made out of anything", direct-sow (no tray), **optional** seedling/sprout trays, and "all possible options, including those we haven't thought of yet" = a **data-driven plot / growing-method registry**. Seed acquisition = ALL of harvest-yields-seeds + optional tray + buy/trade. Starter set = one seed of each. **v0.399, (a) harvest yields seeds DONE:** a survival harvest returns produce + 2 seeds of that plant (creative stays clean), so the loop is self-sustaining (plant 1 → harvest → 2 back → replant + surplus). **The basic seed economy LOOP is now complete: seed items + survival-consume + starter-grant (v0.398) + harvest-regenerates (v0.399).** **NEXT (seed economy, sequenced):** (b) **data-driven garden PLOT TYPES**, generalize "tower" into a plot/method registry (aeroponic / soil / sand / pot / raised-bed / direct-sow / …), each data-defined and moddable (the larger architectural arc he just opened, infinite-of-X applied to gardens); (c) optional **seedling-tray** sub-system; (d) on-new-game starter grant. Keep all of it plot-agnostic. Future from this batch: seedling/sprout harvesting trays; the player starts with a base plant set when the game is ready; the **seed economy** is now UNBLOCKED (build it gated behind survival mode). **Still-standing backlog:** the 8-column-table width watch, tower follow-ups (deepen compat / parts→BOM), the web Mission-Dashboard mirror. (The dead-renderer cleanup SHIPPED v0.416.0, resources/play/onboarding-page deleted, game quests folded into the Quests page.) **(Older, now-stale) NEXT: the QUESTS rework** (one page, two kinds: gameplay quests [auto-track + XP] + learn-by-doing chains [manual, deeper per-method, e.g. each way to collect/purify/store water], and retire the standalone onboarding page). Dedupe DONE (v0.371): `render_markdown` now lives once in `widgets::markdown`, used by both `library.rs` + `humanity.rs`. **Open follow-ups on the mission page:** keep iterating the Mission Dashboard copy/flow to the operator's taste; the real scoreboard metrics stay deferred (placeholders are honestly framed for this early stage); the web-landing mirror still waits until the app reads right. **Condensation pass** (runs alongside the folds): each section should FILL its row cleanly like the Settings page rather than leave a small section + 98% blank, Skills done (v0.359, `egui::Grid` one-row), Settings de-stepped (v0.359, 170px label column); next, spread the loved `tree_list` nested-list to **Notes / Tasks / Quests** (operator's ask, "a great way to condense a ton of information", but "don't overdo it"). The switch-one-section model (the Settings model the operator likes) stays, infinite-scroll is NOT required; condensing sections is the fix. Separate pages stay whole (delegate model) for **web parity**; the landing tab on launch becomes a setting (VR "boot into Play" still works). Decisions: **2 maps** (the real map carries toggleable Humanity LAYERS, donation/pothole/member pins, opt-in coarse location like "Silverdale, WA"; the sim map lives in Play); Recovery→Platform; Resources+Identity→Humanity; **Civilization → the Humanity Community/Mission Dashboard** (repurpose its empty sim metrics to real collective ones). **DESIGN THREADS the operator set this arc** (full detail in `orchestrator_state.json` v0.350 entry): **container model extends to the PLANET**, "mark Earth as my container"; the real-self root chain is Earth → region (Washington/Kitsap) → Silverdale WA → ~1-acre property → 2-story home → rooms → containers → items (uniform tree, just a higher root; the player is a node). **Real-terrain world-gen north star**, in-game terrain heightmaps should semi-match real life (source real DEM/elevation data, USGS/SRTM/Copernicus, keyed to the container's lat/long; a teaching/familiarity feature). **REAL vs SIM = SEPARATE PAGES, never a toggle** (a forgettable toggle is a trust risk for the mission, the page itself is the firewall). Tracked deferrals: deep container nesting needs ECS containers-within-containers (inventory is flat slots today); variable-icon detail-level + compact toggle for `tree_list`; draggable segmented-divider drone manifest; the empty 3D home (no placed crafting stations, world-content arc).

**COMPLETE: Gameplay-loop arc (2026-05-30).** Operator vision brain-dump + "develop as if the user unlocked everything 100%", wire the actual play loops on top of the now-wired engine. Holistic map in `docs/design/gameplay-loops.md` (survival needs → production chain → connective systems → threats → progression-LAST). ~40 systems already exist in code (the engine_wiring_lint deferred list), so this arc is WIRING loops + spawning content + GUI→ECS glue, NOT writing new systems. **Build order: (1) full-unlock dev provisioning ✅ + (2) real crafting loop ✅ (SHIPPED v0.329.0); (3) cooking + nutrition ✅ (SHIPPED v0.330.0, Vitals + StatusEffects + FoodSystem registered; eat/decay/conditions/poisoning/well_fed); (4) gardening ✅ (SHIPPED v0.331.0, plant/water/harvest + Garden panel + dev-grow; closes garden→cook→eat); (5) drone↔asteroid mining ✅ (SHIPPED v0.332.0, AsteroidBody + Drone + DroneSystem; commission→trip→mine finite asteroid→deliver→delete-when-empty; Mining panel); (6) refining-chain depth ✅ (SHIPPED v0.333.0, nickel/platinum/stainless ingots + smelt recipes; 2-tier ore→ingot→alloy; closes mine→refine→craft); (7) survival systems online [IN PROGRESS], #7a energy/rest ✅ (v0.335.0); #7b environment-coupled oxygen/temperature ✅ (v0.336.0, homestead-AABB context → oxygen drain/hypoxia/suffocation + body-temp/hypothermia when exposed; hunger now tangible too); WeatherSystem registered ✅ (v0.337.0, drives the exposed-environment temperature; first deferred sim system live with a real consumer); #7c sanitation ✅ (v0.338.0, waste→compost→fertilizer→Fertilize crops; **all 5 listed survival needs now live**); remaining #7 = register the heavier sim systems (atmosphere/hydrology/ecology/disasters) **when they gain real consumers** (the weather precedent, not cosmetic un-deferral); (8) progression layer [✅ DONE], #8a skills + XP foundation ✅ (v0.340.0, the built-but-unwired `skills/` scaffold now wired end-to-end: SkillRegistry loads skills.csv, PlayerSkills on the player, SkillSystem registered LAST drains a shared `xp_grants` channel the action systems push to; XP from **craft→recipe skill / harvest→farming / mine→mining**; live levels+XP in the profile Skills panel; **data-integrity fix**, recipes.csv `skill_required` reconciled from a non-canonical vocabulary to real skill ids [235 rows, category-aware] so XP can't silently no-op, locked by a new drift lint); **#8b tech-unlock** ✅ (v0.341.0, skills GATE crafting: CraftingSystem authoritatively rejects under-level crafts, the crafting UI shows "Requires {skill} Lv N (you: Lv M)" + locks the button, a **Dev: max skills** button preserves the 100%-unlocked testing posture) + **#8c quests** ✅ (v0.342.0, `quests/` scaffold wired: `QuestRegistry::from_ron_dir`, QuestSystem registered, player auto-accepts the **Getting Started** chain, Craft/Harvest objectives advance via a `quest_events` channel + Gather via live inventory, prerequisite chaining, a profile **Quests** panel; also fixed a #8b fresh-player **deadlock**, level-1 recipes are the free starter tier, gating begins at level 2). **★ GAMEPLAY-LOOP ARC COMPLETE, build order #1–#8 all shipped (v0.329→v0.342, 21 commits): the full production + survival + progression sandbox is delivered.** Next arc is the operator's call. **TEST & HARDEN underway (operator's first live play):** v0.343.0 fixed round-1 findings, the linchpin was a stale `target/release/data` shadowing live repo data (`find_data_dir` now prefers the repo's own `data/`), which had made dev-stock no-op + the quest show its raw id + skills stay empty when running the build exe directly; plus a 3-stage drone visual (+ progress bar), clearer mining labels, and a real skill sheet replacing placeholders. Known gap (future arc): the 3D player home is empty, loops are menu-driven, no walk-up crafting stations yet. #3b: SPEED modifiers ✅ (v0.334.0), Drink action ✅ (v0.339.0, hydration symmetric with Eat); still tracked: stamina/vision modifiers, spoilage→nutrition.** #1+#2 shipped the reusable GUI→ECS command bridge (GuiState flag → main-loop writes a DataStore Mutex channel → the owning System drains + acts in its tick) + a "Dev: stock all materials" button (one stack of every recipe input, inventory auto-grown via `Inventory::ensure_slots`) + a real Craft button doing consume/produce on the player's ECS inventory (proven by `crafting_bridge_tests`). Proposed concise complete-nutrition plant set: potato/soybean/leafy-green/tomato/sunflower/carrot, **operator to finalize**.

**FOUNDATION COMPLETE, engine-wiring arc (2026-05-29):** native P2P NAT-traversal (TURN, v0.320.0); typed containers + content-class compatibility (v0.321.0); server game-world + player-progress persistence (v0.322.0); ENGINE WIRING, item/recipe/plant registries load into the runtime DataStore so crafting crafts + inventory resolves real names (v0.323.0); game_time export + the engine-wiring ENFORCEMENT LINT (v0.324.0, the `theme_token_lint` analog: every `impl System` must be registered OR deferred-with-reason, 7 registered, the deferred list shrinks as the gameplay-loop arc wires systems). This is the platform the gameplay-loop arc builds on. **DATA EXPANSION is now unblocked** (crafting/inventory exercise the item/recipe schema with working code) and proceeds organically as each loop lands, refining chains, components, tools, plants, recipe byproducts, chemistry→crafting links.

**PAUSED -- superseded by the game-dev arc (resume after, or per operator):**
**ACTIVE: Clean web chat VIEW rebuild (Track W, pivoted 2026-05-26).** Operator's call: stop incrementally patching the tangled web view, rebuild it from scratch to mirror native 1:1, **keep the proven JS engine** (WS/crypto/WebRTC), and make sync *mechanical*. Live chat is non-precious (no users) so we rebuild in place. **Spec + sync backbone: `docs/design/chat-layout.md`**, one web `view/*` module per native `draw_*` (same names), engine↔view boundary via the `hos` event bus, DOM stays (accessibility improves, never canvas; WASM considered + rejected for canvas/a11y reasons). Build order: scaffold + constants + event-bus boundary → engine extraction from app.js → centerPanel/messageRow/timestampPill → leftRail → rightRail → composer/header → modals → sweep old view files + dead CSS. Incremental-patch history (now superseded): left+right rails done (v0.287.4-.9), nav labels (v0.287.7), message-row flatten + grouped pill rows (v0.287.10/.11), these stay live as the clean rebuild replaces them section by section. Native WebRTC transport remains the committed parallel next-major-effort (unblocks native voice + streaming).

## TIER 0: pre-public launch blockers
Items here are mandatory before inviting public users. Operator-attended where noted. **Order matters within the tier.**

0. **★ SECURITY AUDIT 2026-06-12, the CRITICAL is a launch blocker.** A 10-dimension multi-agent audit (51 agents, every finding adversarially verified; Fable 5 re-verified the top items by hand) produced 30 verified findings. The **no-fork quick-wins shipped v0.417.0** (stored-XSS via uploaded SVG → blocked + `Content-Disposition: attachment` on `/uploads`; missing security headers on the live `/` and `/chat` entry pages → re-listed + X-Frame-Options added, applied live; twemoji un-SRI'd CDN script → vendored same-origin; profile-gossip future-timestamp lockout → 24h bound). **STILL OPEN, ranked:**
   - **(a) CRITICAL, auto-update RCE: ACTIVATED + crypto-proven 2026-06-12 (v0.421.0).** The operator generated the hybrid keypair (`just gen-release-key`), the PUBLIC keys are committed in `data/release/signing_pubkeys.json` + compiled into v0.421.0, the private `release-signing-key.enc` is gitignored + externally backed up (passphrase recorded non-digitally), and a local self-verify test confirmed the private key ↔ embedded public keys are a matching pair ("Signed + self-verified OK"). So v0.421.0+ builds ENFORCE signatures end-to-end. **Remaining: sign each published release** (`just sign-release vX` after CI uploads binaries), v0.421.0 is the first to sign once its Build-Desktop run finishes; from here on an unsigned release is invisible to auto-update for v0.421+ users (CLAUDE.md SOP step 5 + docs/admin/release-signing.md). NOTE: legacy (v0.420-and-earlier, empty embedded keys) users still auto-update normally to v0.421.0, then enforce. [original code-shipped detail below]
   - **(a-orig) CODE SHIPPED v0.418.0 (the updater half) + v0.419.0 (find_newer_exe).** The updater now verifies a **hybrid Ed25519 + Dilithium3 signed manifest** (both must verify) + the artifact SHA-256 before installing, and only OFFERS releases that carry a signed manifest, so a GitHub/release compromise or a stray/malicious tag can no longer push code. New `src/release_update.rs` (sign/verify/keygen + 8 tests, smoke-tested incl. wrong-passphrase reject), CLI `--gen-release-key`/`--sign-release`, `just gen-release-key`/`just sign-release`, `docs/admin/release-signing.md`. Operator decided: hybrid scheme, dedicated key, passphrase-encrypted file (Argon2id+AES-GCM), signed LOCALLY (never CI). **TO ACTIVATE (operator, one-time): `export HUMANITY_SIGNING_PASSPHRASE=... && just gen-release-key`, commit `data/release/signing_pubkeys.json`, ship a release, then `just sign-release vX` per release.** Until then the embedded pubkeys are empty → updater warns + legacy behaviour (so nothing breaks pre-activation). **find_newer_exe DONE v0.419.0:** `src/main.rs::find_newer_exe` now verifies each candidate `vX_HumanityOS.exe` against the embedded keys (detached `.sig.json` sidecar, hybrid sig over the file's SHA-256) before launching, an unsigned/tampered local build is skipped, not exec'd. New `--sign-file` CLI + `verify_file_against_sidecar`; `scripts/archive-build.js` opt-in-signs each dev build when the key+passphrase are present (unprovisioned → legacy, so the dev flow isn't blocked). 10 release_update tests. **So the only thing left for this CRITICAL item is the operator ACTIVATION** (`just gen-release-key` → commit pubkeys → ship → `just sign-release`), after which the whole updater + launcher chain is fail-closed.
   - **(b) DONE v0.422.0, vault-sync replay.** Added a relay-side anti-replay cache: after a signed request verifies, the relay records the (key, purpose, timestamp) tuple and rejects a second sighting within the window (409). No client or protocol change. `RelayState.auth_nonce_fresh` + `seen_auth_nonces`; applied to vault_sync PUT/GET/DELETE (the dangerous DELETE/PUT replays + the GET). Reusable for the other signed endpoints if needed.
   - **(c) DONE v0.420.0, `POST /api/v2/objects` per-author quota.** Added a sliding-window per-author submission cap (30/60s, keyed by a short hash of the author key, map-size-bounded) → 429 over the cap. The signature already authenticates the author; the quota closes the storage-exhaustion vector (flooding distinct objects under one valid key). `api_v2_objects.rs::post_object` + `RelayState.object_submit_rate`. (A sustained-flood daily quota is a possible future tightening; the per-minute cap kills the 1000s/sec exploit by 4 orders of magnitude.)
   - **(d) LARGELY MITIGATED by the active release signing; residual documented.** A compromised CI can no longer push a malicious DESKTOP release that users will install, because CI cannot sign (the key is operator-local, never in CI) and v0.421+ updaters reject unsigned releases. The residual is the VPS RELAY deploy (a compromised CI could deploy a malicious relay binary via deploy.yml) + GitHub branch/tag protection (operator GitHub-settings, not code). Recommended (operator): enable branch protection + required signed commits on `main` in GitHub settings; the relay-binary-verification on the VPS is a future hardening. Lower priority now that the desktop-RCE path is closed.
   - **(e) DEFERRED to the UI/onboarding work, public `/api/members` directory** exposes name/key/last_seen with no opt-out. Design settled: reuse the existing `profiles.privacy` JSON with a `directory: "unlisted"` key (no schema change), honored in `get_members` (+ `get_member_count` for pagination) and `get_member_by_key` (404 for unlisted). Deferred because (1) a backend opt-out flag is useless without a user-facing toggle to set it, which belongs in the privacy/settings UI the operator is restructuring, and (2) the `get_members` filter wants a `json_extract` subquery on the hot member-list query and json1 isn't currently used in storage (verify it's compiled in first). Build the toggle + filter together in the UI increment. It is a documented design choice (you appear in a server's directory when you JOIN it), MEDIUM, not a launch blocker.
   - **(f) PARTLY DONE v0.420.0.** Federation gossip amplification (HIGH) FIXED: a per-SOURCE inbound rate limit (50/s, reusing `federation_rate` with an `:inbound` key) drops + doesn't re-emit when a source floods, so 1×rate inbound can no longer become N×rate outbound. Announce-flood FIXED: a global cap on `POST /api/v2/announce` (20/60s) bounds the blast radius if `API_SECRET` leaks. STILL OPEN: `/api/send` per-IP rate limit (needs X-Real-IP plumbing; the bot path is the trusted API_SECRET path, low value), and message hard-delete leaves WAL/backup remnants (no retention/secure-erase policy, needs a retention design).
   - Full detail + the correctly-REFUTED findings (home coords = demo data, avatar/banner/link `data:` XSS = server-blocked, `unsafe-inline` CSP = `esc()`-neutralized, etc.) are in the session transcript + the operator's private memory note `security_audit_2026_06_12.md`. **Deliberately NOT committed to the public repo** (disclosure hygiene). The operator chose "quick-wins now, sprint later", items (a)-(f) are the candidate security sprint.

1. **DONE: nginx `/health` routing.** Verified live 2026-06-11: `https://united-humanity.us/health` returns 200 JSON. The fix has been in `scripts/nginx/humanity.conf` (`location = /health { proxy_pass ... }`, v0.285.x), this entry was doc lag. Off-site monitoring can use the public endpoint.

2. **DONE: GitHub webhook deleted + endpoint fail-closed (v0.285.0).** The stale webhook (pointed at a dead ngrok URL, 404 for months) was deleted from the GitHub repo. The relay's `/api/github-webhook` endpoint now FAILS CLOSED, rejects when `WEBHOOK_SECRET` is unset (was fail-open, a forged-announcement spoof vector). Note: this webhook was NEVER the update-autoposter, that's the CI Deploy Bot via `/api/send` + `API_SECRET`, a separate path that's unaffected and healthy.

3. **DONE: off-site backup (stopgap).** 2026-05-20: `scripts/backup-relay-from-vps.ps1` + a Windows scheduled task ("HumanityOS Relay Backup Pull", every 6h) now pull the live relay DB from the VPS to the operator's PC, genuine 3-2-1 backup (live DB / VPS-local 30-min snapshots / off-site PC). This is the "immediate" half of the device-mesh vision (`docs/design/device-mesh.md`); the full in-app version is TIER 2. NOTE: the PC backup is off-site but a SINGLE off-site copy. A second target (phone, NAS, or a cheap second VPS) would make it 3-2-1-with-redundancy. Phase B of the device mesh generalizes this.

4. **DONE: 2026-05-21 release-mirror cleanup + retention automation.** Cleaned 277 old release dirs from `/var/www/humanity/releases/` (freed 91 GB; 91% → 13%). v0.283.4 extends `scripts/humanity-disk-guard.sh` to enforce 10-version retention automatically on every 20-min cycle + regenerate the manifest. Cascade is structurally prevented from recurring.

5. **DONE: backup script repaired + in-repo.** The pre-v0.90.0 path bug was silently backing up an empty fossil DB for over a month. v0.283.4 ships `scripts/humanity-backup-db.sh` as the source of truth, the `deploy.yml` workflow now copies it to `/usr/local/bin/humanity-backup-db` on every deploy. Fossil backups moved to `backups/fossil-pre-v0.90/` for historical interest only.

6. **DONE: Orphan Ed25519 admin rows cleanup.** 2026-05-21: ADMIN_KEYS env updated to Shaostoul's Dilithium hex (3904 chars), 4 orphan rows DELETEd, relay restarted, verified `user_roles` is Dilithium-only.

7. **DONE: Inc6 attended wipe.** Verified 2026-05-20 by direct SQL.

8. **DONE: TLS auto-renew sanity check.** certbot.timer runs on a 12h cycle; last run 2026-05-20 16:42, next 2026-05-21 06:15. All 3 certs valid 50-68 days out. No action needed.

9. **DONE: API_SECRET length audit.** 64 chars (above 32-char threshold). No action needed.

## TIER 1: hardening before invites scale beyond known group
Items here protect against the realistic adversary (script kiddie, opportunistic abuser, eager fan with sticky fingers). Order within tier is flexible; pick what's cheapest first.

**TIER 1 is effectively closed.** All code-actionable items shipped; the two decision-gated items were decided by the operator 2026-05-20 (fail2ban over Cloudflare; skip off-box monitor; plan federation). Remaining federation *implementation* is tracked in TIER 2.

1. **DONE: DDoS protection, fail2ban (v0.286.x).** Operator chose self-hosted fail2ban over Cloudflare. nginx jails added (`scripts/fail2ban/nginx.local`): `nginx-limit-req` (bans IPs repeatedly tripping nginx rate limits) + `nginx-botsearch` (bans exploit-path scanners), conservative thresholds + `ignoreip` for loopback/private. sshd jail was already active. Installed live + version-controlled (deploy.yml installs + reloads). Composes with the in-app gates (v0.279/v0.280).

2. **DONE (VPS-side): Monitoring + alerting (v0.286.2).** Watchdog (2-min liveness + self-heal) + `scripts/humanity-alert.js` configurable multi-channel external alerting (ntfy/Discord/Telegram/webhook), wired into watchdog + disk-guard. Admin opt-in via `data/alert-channels.secrets.json`. **Off-box monitor (whole-VPS-down) explicitly SKIPPED per operator 2026-05-20** ("not too concerned"). If revisited: a free uptime service or PC scheduled task can reuse the same alert channels.

3. **DONE: SQLite corruption recovery (v0.286.0).** `Storage::open_resilient`, boot integrity check + restore-newest-healthy-backup or refuse-to-start. 4 tests.

4. **Federation: design DONE, implementation in TIER 2.** Operator chose "plan activation." Design + vetting + abuse model + 4-phase plan in `docs/design/federation-activation.md`. Key finding: federation is already fail-closed (trust_tier 0 default; unknown peers can't connect), so dormant = safe; the implementation phases (admin UI, profile-gossip rate limit, second-VPS end-to-end test, then third-party peers) are the work. Moved to TIER 2 #1.

5. **DONE (via watchdog, v0.285.2): crash-loop detection.** Watchdog self-heals + alerts (chose this over systemd StartLimit, which would give up + leave the relay dead, bad for unattended).

## TIER 2: big-feature gaps
Items here are real features the system promises but doesn't deliver on every platform. Weeks of work each.

> **Cross-cutting mandate (CLAUDE.md non-negotiable rule, 2026-05-20): GUI-first configurability.** Every ops/config capability must be reachable in-app, not CLI-only. The recent TIER 0/1 ops work (alerts, backups, fail2ban, watchdog, secrets) is all CLI/SSH today, that's tracked debt. See `docs/design/in-app-ops.md` for the audit + the north-star admin action registry (GUI renders it AND an AI can enumerate it) + the build order. NEW features with an ops dimension build their in-app control in the same increment.

1. **Web-mirrors-native parity (Track W, ACTIVE).** Full divergence map + migration order in `docs/design/web-native-parity.md`. Native chat is the parent; web is the old UI being rebuilt to mirror it, incrementally (web stays usable throughout; theme tokens already shared). Migration order: (1) left-rail tabs→stacked-collapsible-sections ✅ + 1b studio→right ✅ + 1c scratchpad top-row ✅ + 1d identity→account-menu ✅, (2) right-rail Friends/Members ✅, (3) message rows + timestamp pill + inline reactions **[NEXT]**, (4) header + composer, (5) top-nav alignment (labels ✅ v0.287.7; native tiering pending), (6) spacing sweep + dead-CSS removal (`style.css`, `chat-voice.js` are dead). Each step = its own increment + version bump.

2. **Studio + streaming (Track S, phased, dependency-ordered).** Full vision in `docs/design/studio-streaming.md`. Right-rail studio widget (top, for streamers) + full Studio modal + docked inverted chat + per-friend viewer widgets + multi-stream viewer modal + **persistent stream across all pages** + **privacy guard** (auto-hide on sensitive pages/buttons). KEY CONSTRAINT: streaming transport exists on WEB (real WebRTC) but NOT native (stubs only); native Studio is a UI page with no transport. So build the widget on web first (functional), mirror to native once native transport exists. Order: S0 persistent session (gate for "always stream" + viewers) → S1 web studio widget+modal → S2 viewer widgets+modal → S3 privacy guard (can land early, independent) → S4 native mirror. Native transport = the same weeks-long WebRTC lift as native voice (#4). **v0.350.0: a native Studio quick-access section now sits at the top of the chat right rail (above Friends), mirroring the website's studio-widget placement, a Go Live/End Stream toggle + Open Studio launcher. This preserves Studio access through the Real/Play nav consolidation (the top-nav Studio button is being folded away); it is ACCESS only, the transport-bearing native mirror is still S4, gated on native WebRTC.**

3. **In-app ops console (phased, pays down the CLI debt).** Per `docs/design/in-app-ops.md`. Slice 1 (System/Health dashboard) SHIPPED v0.287.0 (web). Remaining: native parity for it, then (2) Alert-channels editor (first write panel), (3) Backups panel, (4) Federation panel (= #5 Phase 1), (5) fail2ban/relay-control/secrets (need a sudo-gated relay→system bridge), (6) factor out the action registry + AI-facing list/run endpoints + a coverage test.

4. **Native voice.** Channel-list voice icon click is a TODO (chat.rs:1060). No WebRTC stack at all. Needs: `webrtc-rs` integration, audio capture → kira pipeline, playback routing for N peers, mute/deafen UI, connection state machine. Web users have voice; native users are observer-only today. (Shares the WebRTC-transport lift with Track S native streaming, do them together.)

5. **Federation activation (phased).** Design done, `docs/design/federation-activation.md`. Phase 1: Server Settings → Federation admin UI (list/add/trust/defederate peers + per-channel federation toggle), native + web. Phase 2: per-peer profile-gossip rate limit. Phase 3: second operator-controlled relay, federate the two, verify end-to-end, esp. whether moderation propagates to federated content (load-bearing test). Phase 4: open to vetted third-party peers. Fail-closed default = safe to build incrementally.

6. **Native streaming viewer.** Subsumed into Track S (S4 native mirror).

7. **Native trade UI completion.** Trade page exists in `src/gui/pages/`. Trade events (`trade_response`, `trade_confirm`, etc.) aren't dispatched. Either wire them up or remove the page until ready.

4. **Litestream / continuous backup.** Beyond the nightly rsync floor in TIER 0, set up real continuous replication. SQLite WAL → S3-compatible blob storage. RPO ~1 minute, RTO ~10 minutes from cold.

5. **Mobile clients.** Android (JNI bridge for keyring + AndroidKeyStore; new keychain backend), iOS (Keychain Services already works via `keyring` crate, needs only an iOS build target). Big effort either way.

6. **Device mesh** (design doc: `docs/design/device-mesh.md`). The operator's vision: your devices back up each other + the relay; review all devices' system-info (hardware, storage, health) from any one device; device roles (battle-station / accessory / relay / archive). Phased: A) system-info reporting + "My Devices" dashboard, B) backup designation + pull + encryption-at-rest (subsumes the shipped PowerShell stopgap), C) restore flow, D) LAN direct-sync + mobile mesh members + remote wipe. The VPS-as-rendezvous architecture (devices report up, read all-devices down) fits the existing federation model. On-mission sovereignty tooling, give it to every user, not just the operator.

7. **Library, federated file/media catalog (NEW, designed 2026-05-26).** Full design in `docs/design/library.md`. One "free public access" page, tabbed by consume-mode: **Files** (federation-hosted media/art/3D models, download in, upload, pin) + **Software** (folds in the Tools page) + **Web** (folds in Browser + Resources). Files engine = trust-tiered LRU cache (unverified shared pool + per-user sub-cap; verified+ per-user quota → **bounded disk by construction**) + curated permanent tier (roled pin → permanent + quota refund → routed to the existing torrent seeder + Internet Archive). Identity by **content hash (SHA-256)**: exact dupes auto-link; near-dupes (image perceptual hash) trigger a side-by-side **preview-confirmation dialog** (3D/binaries: exact-hash only). Rule: **ephemeral = server-local; pinned = federated**; catalog aggregates lightweight metadata across `/api/federation/servers`, grouped by source server. Extends `assets.rs`/`uploads.rs`/`roles.rs`/`pins.rs`/`server_settings.rs` + `docs/admin/torrent-infrastructure.md`. Phased: Files engine → Library/Files UI (web→native) → pin/torrent → perceptual dedup → federation aggregation → fold Tools/Browser/Resources in. Seed content: the 187 archived Project Universe media files. GUI-first quota/cap config per server admin.

8. **P2P Groups, relay-independent groups (NEW, designed 2026-05-27; operator chose "true P2P" over relay-mediated/federated-fallback).** Full design + phased plan in `docs/design/p2p-groups.md`. Today groups are 100% relay-mediated (`handle_group_create/join/msg` → relay SQLite), so a relay outage breaks create/join/messaging and the invite URL 404s, contradicts "no single point of failure." Target: a group is a **signed object + append-only signed membership/message logs** replicated peer-to-peer over the existing WebRTC DataChannels (`web/chat/chat-p2p.js`); relays are **optional accelerators** only. Invite = **signed connection ticket** (not a URL). E2EE via a per-epoch group key (generalize the Kyber768 dual-seal in `src/net/dm_pq.rs`), re-keyed on membership change. **Core gap = relay-independent signaling** (today `webrtc_signal` rides one relay) → solved by multi-relay failover + peer-assisted signaling (+ TURN/peer-relay for NAT). Phased: **P1** sovereign data + working signed-ticket invite (fixes the 404; relay still signals) → **P2** signed + E2EE messages → **P3** P2P transport (relay = signaling-only) → **P4** relay-independence (the payoff: kill the home relay, a group with ≥1 reachable peer still works) → **P5** serverless discovery (mDNS/DHT). **Decisions settled 2026-05-27** (operator): TURN = operator-run **+** peer-as-TURN; encryption = per-epoch group key; relay = optional accelerator (see doc). Builds on the signed-object/gossip model (`storage-architecture.md`) + signed-log governance (`signed_moderation_logs.md`).
   - **P1 sub-steps:** (a) **DONE v0.292.0**, relay sovereign data model: `group_v1`/`group_member_v1` signed-object types projected into `p2p_groups` + `p2p_group_roster` (the membership-log fold) via `src/relay/storage/groups_p2p.rs`, wired into `put_signed_object`, additive (old relay-mediated path untouched), 3 tests incl. "unauthorized admit rejected". Object-format spec captured as the module doc-comment. (b) **DONE v0.292.1**, cross-language signed-object construction. Built the missing web primitive: `web/shared/canonical-cbor.js` (byte-exact canonical CBOR matching `src/relay/core/encoding.rs`, length-first key sort, shortest-int, definite-length) + `web/shared/pq-object.js` (`buildSignedObject`/`buildGroupV1`/`buildGroupMemberV1` → the `POST /api/v2/objects` submission). **KAT-locked**: `scripts/group-object-kat.mjs` (`just group-kat`) ↔ `groups_p2p.rs::group_v1_canonical_kat` assert identical payload hex + object_id, web builds objects byte-identical to what the relay verifies. Native already builds objects via `ObjectBuilder`. This unblocks ALL web signed objects (votes/vouches/recovery), not just groups. (c) **DONE v0.293.0**, invite + admission model. Capability design (offline-joinable): creator posts a `group_invite_v1` committing to `BLAKE3(secret)` + expiry; the ticket carries the secret out-of-band; a joiner posts `group_join_v1` revealing it; the roster fold admits the join author iff the secret matches + not expired, **no creator online needed**, randos rejected. Relay: `index_group_invite`/`index_group_join` + `p2p_group_invites` table (`groups_p2p.rs`), 7 tests (incl. wrong-secret/expired/non-creator-invite rejection). Web: `buildGroupInviteV1`/`buildGroupJoinV1` + `encodeInviteTicket`/`decodeInviteTicket` + `randomInviteSecret` (`pq-object.js`). (d) **DONE v0.294.0**, chat UI create+join flow (**the 404 is fixed**). New relay read endpoint `GET /api/v2/groups?pubkey=<hex>` (`api_v2_objects::my_p2p_groups` off the projection). New `web/chat/chat-groups-p2p.js` (lazy-imports the ESM object layer + vendored blake3; uses the chat's own Dilithium signer): create group → `buildGroupV1` → POST /api/v2/objects; per-group "create invite" → copyable ticket (`buildGroupInviteV1` + `encodeInviteTicket`, 7-day); join → paste ticket → `buildGroupJoinV1`. `chat-social.js` `renderGroupList` now renders P2P groups (click → roster + invite dialog) above legacy ones; `promptCreateGroup`/`promptJoinGroup` repointed to the P2P flow. **No browser e2e run yet** (preview unavailable), relay compiles, all KATs/tests pass, submission fields + signable bytes match Rust by construction; operator to visually verify. (e) legacy-path retirement, operator **deleted** the live test group (no migration needed); retire the old relay-mediated group code AFTER the operator browser-verifies the (d) flow (kept as fallback until then).
   - **Phase 2 (E2EE group messages), COMPLETE both clients + the conversation is a CHANNEL, not a modal (v0.295.0 / v0.297.1 / v0.299.0 / v0.300.0):**
   - **v0.301.0, delete groups (Leave + Disband).** Operator: the group list only grew, no way to remove. Built sovereign-signed-object style (matches join/invite): **Leave** (anyone) relaxes `index_group_member` to authorize self-removal (`group_member_v1 {action:"remove", subject:self}`), reuses the existing object type. **Disband** (creator only) is a new `group_disband_v1` that sets a `p2p_groups.disbanded` flag (guarded idempotent ALTER for the live DB); `p2p_groups_for_member` filters disbanded, so it drops off everyone's list (the signed object is the durable tombstone; `index_group` INSERT OR IGNORE never un-disbands). `GET /api/v2/groups` now returns `is_creator` per group (server-computed) so the UI shows **Disband** only to the creator. Web: right-click → Leave / Disband (confirm). Native: header **Leave** + **Disband** buttons. 4 new relay tests (13/13 groups_p2p). Both clients return to #general + refresh on action. **Epoch-key bootstrap note** (operator hit "No epoch key yet" sending into a freshly-joined group): rekey-on-join is creator-driven, only the creator's client seals an epoch to a new joiner, on next open; it self-heals. Future polish: rekey from the group LIST, not just on open.
   - **v0.300.0, group = channel, no modal (operator UX fixes).** Operator tested v0.299.x and required: the group conversation must be the SAME interface as server channels (switching to a group should feel like #general → #announcements), not a bolt-on modal, and the modal kept landing behind its own darkening backdrop. WEB (v0.299.2): killed the dialog; `openP2pGroup` switches the main chat center panel like `switchChannel` (`window.activeP2pGroup`, standard `addChatMessage` renderer, composer monkey-patch, 4s poll); invite via header link / right-click popover (no full-bleed backdrop). WEB first-load bug (groups only appeared after interacting) fixed: `loadP2pGroups` owns the fetched flag, `connect()` proactively loads after identity. NATIVE (v0.300.0): removed `draw_p2p_group_modal`; a P2P group is a new `chat_active_channel` prefix `p2pgroup:<id>` joining the `dm:/group:/scratchpad` dispatch; `enter_p2p_group`/`poll_p2p_group` project decrypted `GroupMessage`s into `chat_messages` so the standard renderer handles them; header has Back + name + "Copy invite"; composer routes `group_msg_v1` over HTTP. Removing every full-bleed overlay eliminates the modal-behind-backdrop bug CLASS. KNOWN LIMITATION: native `enter_p2p_group` does ~4-6 blocking ureq calls on the click frame → brief hitch on a high-latency relay; thread it / dedupe redundant epoch+members fetches if noticeable. P2-relay shipped v0.295.0 (`group_epoch_key_v1` + `group_msg_v1` projections; relay stores/serves opaque ciphertext only; read endpoints `/groups/{id}/{members,messages,epoch}`; 9 tests). P2-web shipped v0.297.1 (web crypto helpers in `pq-object.js`: `buildGroupEpochKeyV1` / `buildGroupMsgV1` / open helpers + `decodeCanonicalCbor`; chat UI rewritten as a real E2EE chat view with 4s polling; initial epoch key issued on group create). **P2-cross-identity + native shipped v0.299.0:** (a) **Web rekey-on-join** (`chat-groups-p2p.js` `rekeyIfCreatorNeeds`), when the creator opens a group dialog and the roster has new members not covered by the current `epoch_key`, auto-mint a fresh epoch sealed to the full roster (forward secrecy on rotation). (b) **Native chat-in-groups**, new `src/net/group_e2ee.rs` (epoch-key sealing + AES-GCM under it, byte-identical to web; 2 tests pass); `src/net/api_v2.rs` adds submit/fetch helpers + `rekey_if_creator_needs` + initial epoch on create; `draw_p2p_group_modal` gains a full chat view (message list, compose + Enter-to-send, refresh button). The two clients now interop end-to-end on the same group object. **Phases 3-5 remain:** P3 P2P transport (relay = signaling-only) → P4 relay-independence (multi-relay signaling + peer-assisted + TURN; the actual payoff, the group survives a dead home relay) → P5 serverless discovery (mDNS/DHT). **Polish queued for after operator browser-verifies cross-identity chat:** right-click context menu on P2P groups; signed `group_leave_v1` + `group_disband_v1` objects.

9. **Real-life-first boot + real/fake multi-save model (REVISED 2026-06-30, was "game/simulator opt-out toggle," operator rejected the toggle framing: "too confusing from the start").** The actual direction: HumanityOS has multiple game saves, and each house/character in a save is flagged real or fake. Real means it maps to the operator's (eventually any user's) actual life: their real house, and with it their real resources (clothing, car, furniture, etc.) entered as data. Fake means the sim/game sandbox as it exists today. The app's default state is real-life, with sim/game content loaded secondarily, not the reverse. Today the app boots straight into the game unconditionally (an early-dev shortcut: Esc from the chat page drops you straight into the loaded world); the target is that the sim/game world does NOT eager-load at all unless the active save/character is flagged fake OR the user explicitly opens it from Settings. This needs, in order: (1) the real/fake flag itself, likely a field on the save/character model (`src/persistence.rs`'s `WorldSave` or a new per-character record, nothing like this exists in code yet, confirmed by grep 2026-06-30) (2) a way to author "real" resource data (starting with the operator's own house/car/furniture/clothing as the first real content) (3) the boot-sequence change so `load_world` in `src/lib.rs` only fires for a fake-flagged save or an explicit Settings opt-in, replacing today's unconditional Esc-from-chat path. Scope this properly (it touches persistence + character system + boot sequence) before starting; don't rush a shim.

## TIER 3: UX accessibility (the ELI5 mandate)
The platform's mission requires this layer. Not optional, just sequenced after the load-bearing security/feature work.

1. **Tooltip pass on every interactive element.** Every button, every input, every icon: short tooltip explaining what it does in plain language. Audit pages one at a time.

2. **"First 5 minutes" onboarding flow.** New user opens the app, what do they see? Today: a chat with no context. Build a guided tour: identity → seed backup → join your first channel → send your first message → set your status → done. The Onboarding page exists but needs flow polish.

3. **Localization expansion.** 5 languages today (en, es, fr, ja, zh). Add: ar, hi, pt, ru, de, sw at minimum. Existing infrastructure (`data/i18n/`) supports it; the work is translation, not code.

4. **Full accessibility audit.** High-contrast, screen-reader, colorblind, reduced-motion modes already in code (`src/gui/theme.rs` has the tokens). Audit every page against WCAG 2.1 AA. Fix violations. Document the audit in `docs/accessibility-audit.md`.

5. **Glossary integration on every page.** 150+ terms in `data/glossary.json`. Right-click any unfamiliar term → glossary popup. Native widget doesn't exist yet; web has it.

## TIER 4: long horizon
Don't touch these until TIERs 0-3 are mostly done. Listing them so they're not forgotten.

1. **LoRa mesh hardware integration.** Roadmap item. Requires actual radio hardware on hand.
2. **STARK selective disclosure.** Scaffold exists; circuit design deferred.
3. **Game-world depth.** The simulation/educational gameplay loop. Big. Cosmos Phase 4d shipped; ship-at-origin world exists; voxel asteroids exist. Lots of content + system work left.
4. **AI agent governance.** First-class AI participation is in `docs/ai/onboarding.md`. As more AI participants connect, governance protocols (Article 14 of the Humanity Accord) need to evolve from "documented intent" to "enforced rules with appeals."
5. **Distribution layer beyond GitHub.** Forgejo mirror exists. BitTorrent + IPFS scaffolded. Codeberg + Software Heritage + WinGet manifest still pending per `docs/admin/distribution-mirrors.md`.

## Recent shipped work

This file lists only NOT-yet-done items. For what shipped, the live sources are:
`git log`, `data/coordination/orchestrator_state.json` `recent_decisions` (the why),
the GitHub releases, and `docs/history/<date>.md`. (A hand-maintained "last 30 days"
list lived here but rotted to v0.283.0 while the project shipped past v0.515 -- a third
competing "what's done" list is worse than none. Don't reintroduce it: SHIPPED recaps go
in the journal, this file stays forward-looking.)

## Tier criteria: how to decide where something goes

- **TIER 0**: "We can't credibly invite strangers until this is done." Operator-attended OK.
- **TIER 1**: "We can invite known people but not unknown people until this is done." Self-service operator can fix.
- **TIER 2**: "Feature is promised but doesn't fully work." Multi-week effort.
- **TIER 3**: "Real users can use the app but they need help understanding it." Mission-critical for accessibility.
- **TIER 4**: "Nice eventually; don't let it crowd out the load-bearing work."

When adding an item, pick the LOWEST tier it could justifiably go in (i.e., the most urgent). Tier-up is rare; tier-down is normal as we discover things are less critical than they felt.
