# Afternoon loop plan (2026-07-01, operator awake and actively testing)

> **Read this file FIRST at the start of every wake-up iteration.** The
> conversation may be summarized/compacted between wake-ups; this file is the
> durable source of truth for the backlog and safety rules. Also re-read
> `docs/PRIORITIES.md` (Active Focus) and
> `data/coordination/orchestrator_state.json` (recent_decisions) each time in
> case a prior iteration already updated them.

## Context: how this loop differs from last night's

The operator is AWAKE and actively testing HumanityOS live in a running game
window right now (not asleep/unattended like the 2026-07-01 overnight loop --
see `2026-07-01-night-loop-plan.md` and its `-results.md` for that prior
session's full backlog, all of which is now closed or handed off as noted
there). That changes a few things:

- **Do not disrupt the operator's live session.** Never kill/restart their
  running `HumanityOS.exe`. Building + archiving a NEW versioned exe via
  `just build-game` is fine (it's a separate file, doesn't touch their
  running process) -- they close and relaunch on their own schedule.
- **Still never launch a GUI window from this loop.** No `just play`/`just
  launch`. If live verification is needed, either it's headless (`just
  snapshot <page>`) or it's described to the operator to check themselves.
- All the SAME safety rules from the overnight loop still apply regardless
  of the operator being present: never `Write` over an existing file (Edit
  only), never point a test relay at production, never use a worktree exe,
  never run `just clean-worktrees` mid-session, commit small, verify every
  change (both cargo checks, lib tests count must not drop, 5 lints,
  doc-links check).
- This loop can run for as long as there's real, well-scoped, safe work
  left in the backlog below -- there's no fixed "8 hours" framing this time,
  just "work what's been discussed" per the operator's own words. Use
  judgment on when the backlog is genuinely exhausted (same standard as the
  overnight loop's own stopping criteria).

## Priority order

1. **The self-sustaining homestead design -- RESULT IN, see the full section
   below.** ~90% of a complete single-occupant self-sufficient homestead
   already exists as real game data; this is now concrete assembly work
   (Phase A: author `data/machines/home_solo.ron` from an exact,
   already-worked-out bill of materials), not open design work. This is
   the single most mission-critical ask in this message
   ("people need to see the bare minimum for 100% self-sufficiency").
2. Studio streaming pipeline -- wire the ALREADY-EXISTING WebRTC/Opus
   infrastructure into the Studio page (see backlog below).
3. Humanity / Governance / Laws / Donate pass (visual + functional).
4. Register the 4 disconnected-but-valuable systems found in the afternoon
   audit (ConstructionSystem, ManufacturingSystem, AISystem, OfflineSystem)
   as the foundation for economy automation + NPC tasks.
5. Economy automation Phase 1 (drone auto-relaunch + auto-smelt + dt
   time-scale fix).
6. NPC task-AI minimal first step (extend the relay's crew-NPC tick with
   real task recognition).
7. Docs sync every cycle (BUGS/FEATURES/PRIORITIES/journal), same discipline
   as the overnight loop.

## Priority #1 IN DETAIL: the self-sustaining homestead design (RESULT IN, ready to implement)

The dispatched `self-sustaining-homestead-design` Workflow (3 research
agents + 1 synthesis) returned a complete, rigorously-grounded design.
**Headline finding: ~90% of a fully self-sufficient single-occupant
homestead already exists as real, verified game data** -- this is
assembly work, not a from-scratch build. The design's intellectual
backbone is a pre-existing, excellent doc most of this session didn't
know about: `docs/design/self-sufficiency.md` (written 2026-06-07,
"weakest coupled loop" / Liebig's-law model, the honest "light cap"
math on why indoor gardens can't grow all the calories). Read that doc
FIRST, then this section, before touching anything.

**Phase A -- assemble the solo home from EXISTING data (no new
authoring, ~1 new file). Do this first, it's the highest-value/lowest-
risk slice:**
1. Create `data/machines/home_solo.ron` (mirror the structure of the
   existing `data/machines/home.ron`, a proven, working 3-person
   design) using this exact bill of materials, sized to real 2,200
   kcal/day + 4.0 kWh/day + 80 L/day one-person demand (full sizing math
   already worked out, don't re-derive it, just implement it):
   - **Power:** 4x `solar_panel`, 2x `battery_bank`, 1x
     `wind_turbine_small`, 1x `generator_portable`.
   - **Water:** 1x `water_tank` (8000 L cistern), 1x `water_pump`, 1x
     `water_purifier`, 1x `home_water_use`.
   - **Food (this is the loop that was weakest for 3 people and now
     closes near 100% for 1 -- the pedagogical payoff):** 9x
     `aeroponic_tower_nutrition`, 1x `aeroponic_tower_apothecary`, 8x
     `potato_grow_bed`, 3x `oilseed_bed`, 2x `staple_grain_tray`, 2x
     `mushroom_rack`, 1x `aquaponic_tank`, 1x `grain_field`, 1x
     `legume_field`, 1x `grain_silo`, 1x `irrigation_system`.
   - **Air:** 1x `air_recycler`.
   - **Waste:** 1-2x `composter`.
   - Copy the `connections` topology straight from `home.ron` (already
     correct), just with fewer instances. Use the existing `arrays` grid
     RON syntax for the tower/bed blocks (infinite-of-X: one line per
     block, not one line per instance).
2. Reuse the existing room shells from
   `data/blueprints/homestead_layout.ron` -- for the solo footprint,
   omit `depot`/`hangar`/`ranch` (already supports being commented out,
   same pattern `ranch` already uses), keep `garden` at its full 34x34 m
   (correctly sized for 1 person, oversized for 3 -- don't shrink it).
3. Furniture/walls/doors all reuse existing `structures.csv` ids --
   nothing new needed here either.
4. **Verify it, don't just author it**: load it via the construction
   editor's buildability report (the same real NEC/wattage math already
   built), confirm power/water/air/waste self-sufficiency actually
   balances per the math above, and add a test if a reasonable one
   exists (e.g. a `home_solo.ron` parses + its declared machine ids all
   resolve against `data/machines/*.ron`'s real catalog, mirroring
   `parses_the_shipped_home_structure`-style tests already in the repo).

**Phase B -- author the 4 genuine content gaps the design found (small,
specific, in priority order, only after Phase A ships and is verified):**
1. **DONE (v0.657.0).** `data/plants.csv`: added `oyster_mushroom`,
   `shiitake`, `button_mushroom` -- backs the `mushroom_rack`'s kcal claim
   with real crop ids (only alien fungi existed before).
2. **DONE (v0.657.0).** `data/creatures.csv`: added `tilapia` and
   `channel_catfish` -- backs the aquaponic tank's B12/omega-3 closure
   claim with a real freshwater tank species. Note: `creatures.csv` has no
   runtime loader at all yet, so this is honest content-gap closure, not a
   mechanically-computed claim (still string-based in the machine
   catalog, same as before).
3. **DEFERRED (not started).** `data/plants.csv` calorie columns (or a new
   `data/food/crop_nutrition.ron` mapping) -- today per-machine kcal are
   hardcoded STRINGS in the machine catalog ("+120 kcal/d"), not
   computed from real crop yield data; this is what actually lets a
   future self-sufficiency score compute from data instead of trusting
   hand-typed text.
4. `data/self_sufficiency/component_outputs.ron` + a `location.ron` +
   a household-size selector -- turns the whole design from prose into
   an actual computed per-loop score with autonomy days, exactly per
   self-sufficiency.md's own "what data we'd add" section. This is
   real, substantial follow-on work (probably its own multi-cycle
   effort) -- don't try to do it in one pass.

**Phase C -- the honest teaching artifacts (the operator's actual
stated goal, do this once A+B are solid):**
1. A live "grow-light draw vs power-budget meter" that turns red the
   instant an LED grow-light is added past the free-sun headroom --
   self-sufficiency.md itself already calls this out as "the single
   most honest teaching artifact." The data already exists
   (`electrical.ron`'s `grow_light`, 100 W); this is meter/wiring logic,
   not new data.
2. A "what this cannot close the loop on" panel on the Home page,
   surfacing the design's own section 8 findings: electronics/
   semiconductor manufacturing, metal/alloy production at ore scale,
   medicine synthesis, and equipment-replacement/capital-goods renewal
   are ALL things this game's own recipe data already abstracts away
   real industrial infrastructure to make craftable at a workbench --
   mark these as externally-sourced/traded, visually distinct from the
   closed survival loops. **This panel is the actual pedagogical payoff
   the operator asked for** ("people need to see the bare minimum...
   so they understand the importance of all supporting civilizational
   infrastructure") -- don't skip it once A+B are done, it's the point,
   not a nice-to-have.

Full design reasoning + exact sizing math for every number above is saved
permanently at `docs/design/homestead-solo-design.md` -- read that file
for the complete write-up (this section is a compressed summary of it).
The raw workflow's power/water/air/waste and food/crafting research
reports (more granular file:line citations than the design doc needed to
carry) are NOT saved permanently -- they were used to produce
homestead-solo-design.md and that document already contains every
actionable fact from them; don't re-run that research.

## Backlog: Studio streaming pipeline (priority #2)

The Studio page's scene/source management is real, solid UI; the honesty
fix (v0.652.0) already labels the rehearsal-only reality so this is no
longer misleading anyone in the meantime. The real infrastructure already
exists and just needs plumbing:
- `src/net/webrtc.rs` -- full STUN/TURN/ICE, `WebrtcManager`,
  `offer_to_voice`, already proven by the live voice-chat feature.
- `src/net/voice.rs` -- Opus encode/decode, `mic_level()` (a REAL mic level
  reader, currently unused by Studio's still-static `let level = 0.4_f32`
  placeholder at `studio.rs:448` -- this alone is a quick, real win: swap
  the placeholder for `net::voice::mic_level()`).
- `src/relay/storage/streams.rs` -- `create_stream`/`end_stream`/
  `update_stream_viewer_peak`, unused by Studio (Studio's Go Live never
  calls into this at all -- wiring it in is what would make streaming real,
  not just less-fake-looking).
Concrete follow-ups, roughly in order of value/effort:
1. **DONE (v0.658.0).** Real mic meter: swapped the static `0.4_f32`
   placeholder for `crate::net::voice::mic_level()` (the same reader the
   voice-chat mic test uses). Reads 0 unless a mic test or a live voice
   session is actually capturing -- honest, consistent with the page's
   existing "rehearsal only" framing, not a fake-but-moving bar.
2. Program/Preview split -- the single biggest OBS-workflow gap (can't
   stage a scene change before it's "live"). Needs a second canvas/state
   for "what's staged" vs "what's shown," a real design decision on layout,
   not a one-line fix -- scope carefully, this alone could be a full cycle.
   **STILL OPEN.**
3. Wire Go Live to actually call `create_stream`/relay stream lifecycle +
   `net::webrtc`/`net::voice` for real transport. This is the "weeks each"
   item `docs/STATUS.md` already tracks -- don't force the WHOLE thing in
   one cycle; land it in verifiable slices (e.g. "camera capture only,
   local preview, no network yet" before "actually transmits"). **STILL
   OPEN.**
4. **DONE (v0.658.0).** Inline help via the `help_modal` pattern: 3
   `help_button` calls added (Scenes, Resolution/Bitrate/FPS, Chat
   Overlay) + matching `data/help/topics.json` entries
   (`studio-scenes-sources`, `studio-stream-settings`,
   `studio-chat-overlay`). Notable discovery: the native `help_button`/
   `draw` plumbing was fully wired at the app level (registry load +
   modal render loop) but had ZERO real call sites anywhere in the native
   UI until this -- Studio is the first page to actually use the built
   help system.
5. Move the 7 hardcoded source-type fill colors (`studio.rs:312-318`) into
   `theme.ron` as a proper palette; remove `studio.rs` from
   `tests/theme_token_lint.rs::LEGACY_OFFENDERS` once migrated.
   **STILL OPEN -- scoped out this cycle**: `studio.rs` has 13 total
   hardcoded `Color32` literals (not just the 7 source-type fills), and
   the lint's allowlist is per-file, so migrating only 7 wouldn't earn
   removal from `LEGACY_OFFENDERS` anyway -- doing this properly means
   all 13 in one pass (7 source-type tokens + AFK-timer purple + meter
   background + border alpha, etc.), each needing a `theme.ron` token +
   `theme.rs` accessor + a `theme_editor_coverage`-satisfying settings.rs
   row. Left for a dedicated pass rather than a partial one that changes
   nothing checklist-visible.

## Backlog: Humanity / Governance / Laws / Donate (priority #3)

- **Governance (highest value, functionality not just visuals)**: native
  `governance.rs` renders zero real data today (static instructional text
  only). Wire it to fetch `GET /api/v2/proposals` + tally and render a real
  feed (reuse `expandable_row`/`card` patterns from Laws). Then ship the
  write path: the server already fully supports propose/vote via the
  generic `POST /api/v2/objects` (proposal_v1/vote_v1 signed objects,
  `src/relay/storage/governance.rs`'s own test helpers `make_proposal`/
  `make_vote` show the exact shape) -- build a `ObjectBuilder`-based
  propose/vote flow signed with the user's Dilithium3 key
  (`crate::net::identity`) on native, AND fix the web page's vote-button
  stub (`web/pages/governance.html:207` currently just shows an `alert()`
  telling the user to curl the API themselves -- wire real signing via the
  existing `pq-identity.js` pattern instead). Track "did I already vote"
  client-side so the button relabels after a successful vote.
- **Laws**: surface the already-loaded-but-unused `categories` field as
  filter chips. Convert the plain-text BASE/REAL badge into a real chip
  (reuse/extend `goal_chip` from `humanity.rs`).
- **Donate — real bug, fix regardless of the payment-method question
  below**: native's donation list is populated ONLY from local per-install
  Settings, never from the server's `/api/server-info` `funding.addresses`
  (which the relay already serializes when `funding.enabled`) -- only web
  actually fetches it. Add a `funding: Option<serde_json::Value>` (or a
  typed `FundingConfig`) field to native's `ServerInfo`
  (`src/gui/mod.rs:93`), and populate `state.donate_addresses` from it on
  connect (mirroring the existing `main_menu.rs` health-check
  background-thread pattern for the actual HTTP fetch). This is
  unambiguous and safe to do regardless of which payment methods end up
  listed.
- **Donate — payment methods (DO NOT touch until the operator confirms)**:
  the operator said Patreon is their only channel that's actually gotten
  money, but this repo's `data/server-config.json` has no Patreon entry at
  all (GitHub Sponsors, Solana, Bitcoin, Ethereum, Monero only) -- flagged
  to the operator as a discrepancy, unresolved as of this loop starting.
  **Do not add/guess at new payment-method entries (CashApp, PayPal, bank
  accounts, or a Patreon entry) until the operator answers** -- this is a
  real content decision (what are the actual account handles/links?), not
  something to invent. If the operator's answer arrives mid-loop, act on it
  then; otherwise skip this specific sub-item and leave it noted.
- **Humanity (Mission Dashboard) visual pass**: give the hero title an icon
  badge (from `icons.rs`) + accent-colored top border; replace the plain
  `metric()` tiles in "Where we stand" with `civilization.rs`'s
  `draw_stat_card` (progress bar / trend arrow support, already built);
  vary card visual weight (accent border on "Start here", quieter treatment
  for the manifesto/essay cards) instead of 9 identical grey cards.

## Backlog: register the 4 disconnected-but-valuable systems (priority #4)

Found in the afternoon audit -- NOT the same as the ~15 confirmed-dead
files from the overnight loop (those are zero-value scaffolding, correctly
left alone pending an operator cleanup decision). These four are REAL,
already-written, valuable code that just needs to be turned on:
- **`ConstructionSystem`** (`src/systems/construction/mod.rs`) -- blueprint
  -> timed build -> `Structure` entity. Fully coded, not registered.
  Registering this is a prerequisite for the economy automation backlog
  below (auto-building end products from refined material).
- **`ManufacturingSystem`** (`src/systems/manufacturing.rs`) --
  `ProductionFacility` -> timed recipe -> `output_count`. Fully coded, not
  registered. Also a prerequisite for economy automation.
- **`AISystem`** (`src/systems/ai/mod.rs`) -- a real herd/predator/guard/
  passive/aggressive creature-behavior state machine, not registered, and
  its `AIBehavior` component is never spawned on any entity. Registering
  it alone does nothing without an actual creature entity to attach it to
  -- lower priority than the other three unless there's a concrete
  "spawn wildlife" ask; note it as available but don't force a use for it.
- **`OfflineSystem` / `AutonomousTask`** (`src/systems/offline.rs`,
  `ecs/components.rs:970`) -- a real scheduling primitive explicitly built
  for "AFK NPC chores (patrol, gather, build)" with catch-up logic for
  elapsed time, not registered, its component never spawned. Its own doc
  comment admits it stops at "the scheduling primitive" -- no action
  callback exists yet. This is a real candidate foundation for the NPC
  task-AI backlog below, but per that backlog's own recommendation, the
  actual crew NPCs the operator sees live in the RELAY's `game_state.rs`,
  not this ECS-side system -- decide during that cycle whether to extend
  `OfflineSystem` for ECS-side NPCs or extend `game_state.rs` directly for
  the relay-side crew (the afternoon audit recommended the latter as the
  smaller, more directly-relevant first step).

For each: register in `lib.rs`'s `system_runner.register(...)` list, run
`engine_wiring_lint` to confirm it's no longer flagged, and verify nothing
downstream breaks (a newly-ticking system touching entities that don't
exist yet should no-op safely, not panic -- write a test proving that if
one doesn't already exist).

## Backlog: economy automation Phase 1 (priority #5)

Full feasibility assessment already done (afternoon audit) -- this is
genuinely a small, additive slice because the hard plumbing already exists
and is proven by tests:
1. **Time-scale fix (do this first, it's foundational)**: none of
   `DroneSystem`, `CraftingSystem`, or `ManufacturingSystem` currently
   respect `TimeSystem.time_scale` -- they all use the raw per-frame `dt`.
   Fix at the `SystemRunner::tick` call site in `lib.rs` (multiply `dt` by
   the current time_scale ONCE before passing it to every system) so
   "accelerated for testing" works for every timer-based system at once,
   not just the game clock.
2. **Drone auto-relaunch**: extend `DroneSystem`'s `Deliver`/despawn arm
   (`src/systems/mining.rs`) so that if a `Home`/base entity has a standing
   auto-mine order, it re-commissions the same trip automatically instead
   of waiting for a manual `commission_drone` write.
3. **Auto-smelt trigger**: a small addition (new marker component
   `AutoRefine{recipe_id}` on a machine entity, or ~15 lines in
   `CraftingSystem::tick`) so ore sitting in a smelter's inventory
   auto-fires the existing `smelt_iron`-style recipe path without a
   `craft_request` write. Reuses 100% of existing `CraftingSystem` logic
   (input check, consumption, timed completion, skill XP).
4. **End product**: use an EXISTING tool recipe already in `recipes.csv`
   (e.g. `craft_hammer`: iron_ingot + wood_plank -> hammer) as the proof
   target -- zero new blueprint/recipe data needed for this phase.
Do NOT attempt Phase 2 (real named end products: cistern/truck/gun) in
this loop without operator input -- it needs new blueprint RON authored
AND a real design decision (is a truck a placed `Structure` or an
inventory `Item`?) that shouldn't be guessed at. Note it as a following-up
open question if reached.

## Backlog: NPC task-AI minimal first step (priority #6)

Full feasibility assessment already done (afternoon audit). The crew NPCs
the operator actually sees wandering the mothership run on the RELAY side
(`src/relay/handlers/game_state.rs::GameState::tick`) -- pure Brownian
motion in a bounding box today, no task concept, no world-state awareness.
The ECS-side `AISystem` is a completely separate, unregistered, task-less
creature-behavior system and is NOT what's rendering the crew NPCs the
operator is looking at -- don't confuse the two or "fix" the wrong one.

Recommended minimal step (from the audit): extend `GameState::tick` with a
per-NPC state machine: idle -> scan for the nearest already-tracked
actionable world fact (a `WasteAccumulator` category over its "needs
emptying" threshold, a `ProductionFacility` with `running == false`, a crop
entity at harvest `growth_stage`) -> replace the wander bounding-box drift
with straight-line movement toward that target's position (no pathfinding
exists in the engine at all -- this is an accepted, honest limitation for
this first step, not a gap to silently fix) -> hold "working" for a few
seconds and flip the target's state (empty the bin / set running=true /
harvest the crop) plus a simple visual cue -> return to idle. No animation
system required. Do NOT attempt general-purpose multi-NPC task-priority
scheduling, real pathfinding, or animation in this pass -- those are
separate, larger projects the audit explicitly flagged, not this step's
scope.

## Known items intentionally NOT in this loop's scope

- **~15 confirmed-dead scaffold files** (autonomy.rs, blueprint.rs, csg.rs,
  the logistics/ and navigation/ trees, physics/{fluid,collision}.rs,
  psychology.rs, input/{mod,bindings}.rs, learning.rs, maps.rs) plus
  `FarmAutomation` (found afternoon audit, also genuinely empty) -- a real,
  safe cleanup opportunity, deliberately left for an explicit operator
  go-ahead before bulk-deleting across this many files/subsystems in one
  sweep, per the overnight loop's own reasoning. Do not delete these this
  loop either, unless the operator explicitly says to.
- **Mute Server notification-infrastructure build-out** -- real design
  options already presented to the operator (tiered audio/badge/mention
  suppression + a bandwidth-saving WS-disconnect-while-muted mode), no
  decision made yet on sequencing/scope. Don't start building this without
  the operator picking a starting scope.
- **Full real ship orbital mechanics** (the manual sun-angle override,
  v0.653.0, is the stopgap already shipped) -- a genuinely large, separate
  project per `docs/design/cosmos-architecture.md`'s own Phase 4c/5 sizing.
  Not in scope for this loop.

## Per-cycle checklist (repeat this loop)

1. Read this file + `docs/PRIORITIES.md` Active Focus +
   `orchestrator_state.json` recent_decisions (in case a prior cycle this
   session already updated them, or the homestead-design workflow result
   landed).
2. Pick the next unaddressed backlog item, in priority order, skipping
   anything explicitly gated on operator input (Donate payment methods,
   Mute Server scope, dead-code deletion, economy Phase 2's design
   question).
3. Read the real surrounding code before writing anything.
4. Implement for real (Edit, never Write over an existing file).
5. Verify: both cargo checks, lib tests (count must not drop), 5 lints,
   doc-links check if docs changed, a headless snapshot if a GUI page
   changed visually.
6. Commit small (`git commit -F <tmpfile>`), bump version if Rust changed.
7. Update BUGS.md/FEATURES.md as appropriate.
8. Update `docs/PRIORITIES.md` (what's next) + `orchestrator_state.json`
   (why) in a separate journal commit.
9. Push both commits to `origin main`.
10. If this cycle produced a new versioned exe worth the operator testing
    (a real user-visible fix/feature), run `just build-game` and mention it
    in the journal so the operator knows to relaunch -- but don't do this
    for every tiny internal change, only meaningful ones, to avoid needless
    churn.
11. Continue to the next cycle (self-paced, per `/loop`'s dynamic mode) --
    call `ScheduleWakeup` with the full original loop prompt, unless the
    backlog is genuinely exhausted (all non-gated items done) or something
    needs the operator's direct input to proceed further, in which case
    stop and write a results doc (mirroring
    `2026-07-01-night-loop-results.md`'s format) summarizing what shipped
    and what's waiting on the operator.
