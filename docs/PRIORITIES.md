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
> Current at **v0.1326.1, 2026-09-20**. Retired blocks live in
> `docs/history/priorities-archive-2026-08-to-09.md` and
> `docs/history/priorities-archive-2026-05-to-08.md`.

---

## TIER 0: the next thing to work on

Strict rank. Take the top item that is not marked CLAIMED. Everything below
TIER 0 is real work that has not been ranked against these four; do not promote
anything into this list without the operator.

### 1. FIXED in v0.1330.0. Weather is a place now, not a global (BUG-080)

Kept at the top for one more pass because the arc it opened is not finished.

The operator's "it morphs instead of remaining consistent in its positioning"
is closed. `wx_fade` is deleted, nothing in the cloud path reads camera altitude,
and the weather condition rides an environment region with a position and a
radius. Measured on `stormfade-*`: the descent column is flat within 1 percent
(200.6 / 201.9 / 202.5 / 202.5 / 202.7 / 201.0 at 200 / 120 / 90 / 60 / 30 /
10 km) against a 3.3x cliff before, and 12,000 km is unchanged at 22.1 so the
marble is not whitened. See BUG-080 and `docs/design/environment-fields.md`.

**What remains on this arc, in order:**

1. ~~**Aurora**~~ BUILT in v0.1331.0. Two ovals, one per pole, drawn as emission
   inside the air with the ray solved against the emitting layer so limb
   brightening falls out of the geometry. It was the second consumer of the
   region buffer and it needed no new channel, no new binding and no change to
   the record, which is the claim the mechanism was making. Fixtures
   `aurora-polar-1500` and `aurora-orbit-5000`. Follow-ups: an `override`
   switch so a pipeline that cannot draw it does not compile the branch at all,
   and the A/B cost measurement that switch makes possible.
2. **Disasters through the buffer.** `disasters.rs` already stores position,
   radius and intensity and nothing outside that file reads them, so a wildfire
   is invisible. Needs a consumer to be worth anything, which is why it now
   ranks below aurora rather than above it.
3. **The bake gap.** The sun-shadow cache and profile bakes in
   `45-cloud-temporal.wgsl` read the base coverage and do not apply the region
   floor, so a storm lights and self-shadows as though it were not there. A
   second-order error against a first-order fix; wants its own measurement.
4. **Layer 1**: temperature, pressure and wind as analytic fields with locked
   CPU and GPU twins. This is what makes the rest of the inventory in the design
   doc (humidity, fog, snow line, outbreaks, pollution, light pollution, ocean
   currents) reachable.

**Do not re-propose:** a weight that reads camera altitude, distance, or how
much of the planet is on screen. That is the defect class, not a tuning knob.
### 2. The orbital TV-static: a LIGHTING defect, bisected to the direct-sun term

Rewritten 2026-09-20 after measuring it. The previous entry called 2000 km
"much improved but not clean" and pointed at the far-rung sampling story. Both
halves were wrong for the symptom the operator actually reported, so read this
before spending anything on the old framing.

**What the captures show** (all at the operator's own graphics settings, run as
`node scripts/probe-sweep.js --only <id> --operator-config`):

- `approach-2000km-ultra` is not improved at all. The masses disintegrate into
  isolated grains.
- `approach-2000km-high` is the important one, because High is the DEFAULT tier
  and therefore what every user sees. Its cloud masses have CORRECT, coherent
  silhouettes in the right geographic places, and their interiors are filled
  with per-pixel black-and-white noise. That is precisely the operator's report:
  "awfully dark and white in spots to the point of looking like old TV static."

**The bisect, using the screen-path channel instrument in
`assets/shaders/pbr/45-cloud-temporal.wgsl`** (`map_diag` N; the channels render
one raw ingredient of the march as greyscale and, importantly, still go through
the same resolve and composite as content, so they converge like content):

| channel | vantage | result |
| --- | --- | --- |
| 1, coverage alpha | `orbit-2000-high-mask` | **clean**: coherent masses, crisp edges, no speckle |
| 3, ambient luminance | `orbit-2000-high-amb` | **clean**: flat, even grey interiors |
| 2, direct-sun luminance | `orbit-2000-high-sun` | **GRAINY**: the same masses, full of salt-and-pepper |

So the density field, its thresholding, and the accumulation that carries alpha
are all fine. The direct-sun term is the sole carrier.

**Four hypotheses refuted, each built and captured. Do not re-propose them:**

1. Coverage sampling (the far-rung story). Refuted by the clean channel-1 mask.
2. The sun-shadow CACHE. `orbit-2000-high-nolight` (`cloud_light 0`) is
   indistinguishable from the baseline.
3. The per-pixel march DITHER. `orbit-2000-high-nodither` (`cloud_dither 0`) is
   indistinguishable from the baseline.
4. Ambient shaping. Refuted by the clean channel-3 capture.

**Where to look next.** The channel-2 doc names what is inside it: sun taps,
powder, cavity-on-direct. The cache is already excluded, so the per-sample sun
taps and the powder/cavity shaping are what remain. The question worth asking
first is why alpha converges over the accumulation while the direct-sun term
does not, given both ride the same resolve: a low-sample stochastic estimate
that the variance clip refuses to accumulate would produce exactly this.
`orbit-2000-high-notemporal` (history snap, one un-accumulated frame) is built
and not yet captured; it answers that question directly.

**The far-rung sampling defect is REAL but is a different defect.** Ultra
rendering 0.9 percent coverage against High's 31 is the item below, and fixing
it would not have touched the default tier or the symptom reported here.

**Method that keeps working, reuse it:** change ONE thing per build, capture
between each, and check that the instrument could have failed before trusting a
clean result. Channel 1 would have proved nothing if it rendered an analytic
coverage instead of the marched one; it renders the marched one.
### 3. The night-side coast glow. SEVEN hypotheses dead, and a method lesson

The operator has now reported this twice, most recently 2026-09-21. Fixture:
`orbit-terminator-3000km`. Measure it with the new
`node scripts/night-side-mean.js <png>`, which prints the mean colour of the
dark half plus the share of pixels reading as a lit trace. Two dark frames
cannot be ranked by eye, and that is part of why this has survived so long.

**Established by measurement (2026-09-21):**

- The `AMBIENT_FLOOR` constant in `90-fragment-main.wgsl` IS a real
  contributor. Zeroing it moved the night-side mean from 2.69/3.43/3.95 to
  1.93/2.85/3.67. It is a constant, slightly BLUE-biased floor
  (0.005, 0.005, 0.006) applied through a `max()`, so zeroing `sky_ambient`
  alone never removed it, which is exactly why the earlier "ambient forced to
  zero" arm showed land going black while bright shallow-water albedo kept
  glowing. It is NOT the whole story: the lit-pixel share barely moved.
- `sky_ambient` itself contributes about nothing at night: zeroing the entire
  indirect term landed on the same numbers as zeroing just the floor.
- The water SHELL is not involved at this range. Forcing the whole `fs_water`
  output to black changed nothing, because at 3000 km the ocean is not drawn
  by the water shell at all. That single fact retroactively explains why the
  four earlier water-focused hypotheses all failed.

**THE METHOD LESSON, which matters more than any of the above.** A later arm
zeroed `color = ambient + lo` in the shared tail and the DAY side stayed fully
lit. That is impossible if the edited code were running, so at least one arm
never reached the renderer, and every conclusion built on top of it is void.
The arms above are the ones that demonstrably moved the numbers.

So, before ANY further bisecting here: **run a positive control first.** Force
the suspected entry to emit magenta and confirm the planet turns magenta. Only
then start turning things off. Seven hypotheses have now died on this bug and
at least one of them died for the trivial reason that it was never executed.
See the memory note `feedback_checks_that_cannot_fail`.

**Still open and worth trying, in order:** identify which fragment entry
actually draws the planet at orbital range (a positive control answers this in
one boot), then bisect within THAT entry; and independently, decide whether
the blue-biased `AMBIENT_FLOOR` should be neutral and smaller, which is a
defensible improvement on its own but touches interiors and deep space, so it
needs a check that those keep their silhouette floor.
### 4. The far-rung gates, G0(d) and G1 to G7

Unchanged, and still the plan for the deeper cloud work. The increment is merged
behind knob 0; the gates are what turn it on. Design of record:
`docs/design/cloud-far-rung.md` (v2, after the v1 contract failed its adversarial
critique on eight real blockers). The measured target it exists to fix: at 873 km
Ultra renders about 0.9 percent coverage against High's 31, because one sample
per ray misses a 300 m layer vertically.

### 5. Fifty-five stale page snapshots

All 55 checked-in PNGs under `tests/snapshots/` are stale relative to main,
including pages the GUI extraction never touched. Proven by control, not
assumed: the base `src/gui` was restored into a worktree, five pages re-rendered,
and they came out byte-identical to what the extracted code produces while both
differ from what is committed.

So the snapshot check cannot catch a real change today, and a snapshot diff is
not evidence of anything until this is done. Regenerating blind would bake in
whatever drifted: the honest fix is `just snapshots`, then LOOK at the results
page by page before committing new baselines. Nobody owns it yet.

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
2. **Clear the old agent worktrees** under `.claude/worktrees/`. Audited
   2026-08-04: none could be cheaply proven redundant, and
   `just clean-worktrees` force-deletes branches and has destroyed
   review-approved work before. Operator-only by standing rule.
3. **Two gameplay questions** from
   `docs/design/playable-assessment-2026-09-19.md` section 7. **Crop growth speed
   is ANSWERED (2026-09-20)**: a growth multiplier separate from the world clock,
   1x / 10x / 100x plus a custom value, shipping at 10x, implemented and tested;
   Tier A item 3 is unblocked. Offline progression was answered on 2026-09-21 as well, and is broader than crops (a toggle on all three modes, applied to crafting too); it is designed in `docs/design/offline-progression.md` and not yet built. Still
   open: what the first ten minutes are, and whether a pipe reads as its real
   material or its utility colour. A third, lower: does multiplayer enforce
   anything, or is it co-operative trust until launch. NOTE that the report's
   question 2 ("do the 3D models ship with the release") is ANSWERED: they do,
   since v0.1322.0.
4. **The two demoted lethal Library guides**
   (`/library#making-water-safe-to-drink`, `/library#keeping-what-you-grew`)
   stay at `sourced` until a human has read them, per the rule the operator
   chose. `curriculum-status.js` enforces it.
5. **GitHub branch and tag protection on `main`.** Deploy auto-pushes to the
   live relay with no approval gate. GitHub settings, not code.
6. **Donations copy** needs the exact earmarked Sponsor-A-Can URL for HumanityOS
   and confirmation of whether those donations are tax-deductible and earmarked,
   before the CTA and FAQ wording can be finalized.
7. **Landing screen 2 hero shot:** click Play, frame something pretty, and tell
   the session to capture (`debug/screenshot_request.json`); it swaps the cosmos
   stand-in for the real 3D shot.

---

## Fenced arcs

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

- A placement gate on `embed.status` (nothing affiliate until the legality
  column says so; `data/web/sites.json` is the record).
- **The screens' share of the interior frame.** The console room with all six
  walls live ran at 9 fps on rig defaults.
- Player-synchronised playback (needs the relay clock), and subtitles. Seek
  shipped in v0.1325.0 and is off this list.
- **Open defect, small:** a `watch:` screen with no connected server builds an
  address with no host and retries forever showing a URL parse error
  (`src/engine/screens/live.rs`, `gui_state.server_url` empty). It should say in
  one sentence that there is no server to watch.
- Deferred on purpose: VR controller rays; per-context `thread_local` page state
  (a screen and the main UI showing the same page share it); the `rooms.ron`
  entries for entry, pantry, hall and utility (named, not yet functional).

**Gate for any screen change:** both cargo checks, the screens / surface /
dispatch / machines lib tests, the standalone lints, AND a boot that enters the
world and drives the wall through the dev IPC. `just verify-screens` is the
named gate; static verification cannot see a dark or mirrored screen.

### B. The frame cost arc, remaining rungs

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

`docs/design/playable-assessment-2026-09-19.md` is the honest read: eleven loops
close end to end through the UI with no console and 24 of 42 systems tick, so
"framework built but not wired" is half wrong. What is missing is the layer
between the simulation and the person. Its tier ladder is the build order.

- **Tier A (make the existing game legible and durable).** A0 (ship the art) is
  DONE in v0.1322.0. Remaining: persist `Structure` and `Construction` into
  `WorldSave` (built structures are DISCARDED at exit today); **offline progression**
  (`docs/design/offline-progression.md`, designed 2026-09-21, operator-requested,
  generalizes past farming to crafting and any time-advancing system, and is what
  makes the new 1x crop speed a real choice rather than a punishment); put the simulation
  on the HUD; fix crop pacing (blocked on the operator question above); a
  scripted first-run sequence in the world; make a built thing do something by
  consuming `Structure.provides`.
- **Tier B (make the construction tool good enough to build a city).** Pick one
  canonical layout schema of the three that exist; the four multi-storey
  blockers in order, starting with a base Y on `InteriorWall`; collision for
  generated geometry; then author the acre with the tool. The report argues the
  tool over hand-authoring on evidence: the four code blockers stopping a second
  storey are the same four stopping the editor.
- **Tier C (make the world look right).** Un-gate hero plant models for towers;
  the conduit render pass; models for the machines a player stands in front of
  daily; read `mesh_kind` in `zone_filler.ron`.
- **Tier D (multiplayer, in the only order that works).** Move `PlacedItem` out
  of `src/gui` so `persistence` compiles into the relay; bridge
  `game_time_sync`; nameplates and appearance sync; make a trade move items; a
  `SystemRunner` host in the relay.
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

97 of 143 topics still have no document. The cheapest 16 already have both data
and sources, so one document completes all four layers; `just curriculum` lists
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
2. **`/api/members` directory opt-out.** Design settled (reuse the existing
   `profiles.privacy` JSON with a `directory: "unlisted"` key, honored in
   `get_members`, `get_member_count` and `get_member_by_key`), deliberately
   deferred: a backend flag is useless without the user-facing toggle, so build
   both in the same privacy-UI increment. Verify json1 is compiled in first.

---

## TIER 2: big-feature gaps

Real features the system promises but does not deliver on every platform. Weeks
of work each.

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
8. **Native trade UI completion.** The Trade page exists in `src/gui/pages/` but
   trade events (`trade_response`, `trade_confirm`) are not dispatched. Wire them
   or remove the page until it is ready.
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

- `docs/history/priorities-archive-2026-08-to-09.md` (the cloud, rosette, perf,
  screens, Library and content arcs, 2026-08-21 to 2026-09-19).
- `docs/history/priorities-archive-2026-05-to-08.md` (the old Active focus stack
  back to the web chat rebuild, plus the settled TIER 0 and TIER 1 entries).

Do not reintroduce a hand-maintained "recently shipped" list here. One rotted to
v0.283.0 while the project shipped past v0.515, and a third competing "what is
done" list is worse than none.
