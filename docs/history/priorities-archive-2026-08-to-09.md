# Priorities archive: 2026-08-21 to 2026-09-19

Retired from `docs/PRIORITIES.md` on 2026-09-20 at v0.1326.1.

These are the dated status blocks that had stacked up at the top of the tactical
backlog. Every arc described here is settled: it shipped, it was refuted, or it
was explicitly deferred. They are kept verbatim because they carry the REASONING,
which is the part a summary loses: what was measured, what was tried and failed,
and which hypotheses were ruled out so nobody re-derives them.

The work that was still open when this was archived moved forward into the new
`docs/PRIORITIES.md` TIER 0 and its fenced-arcs section. The companion record of
WHY each decision was made is `data/coordination/orchestrator_state.json`.

Newest first, exactly as they stood in the file.

---

> **PLAY WAS BROKEN AND IS FIXED, 2026-09-19 (v0.1323.0, BUG-078).** Operator:
> "I tried to log into the game but instead of loading the game world it threw
> me into the profile page. I can't seem to get into game." The bedroom's new
> standing mirror is a screen showing `profile`, in-world screens draw real
> pages against the same `GuiState` as the main UI, and `profile::draw` opened
> by writing `active_page`. So entering the world drew the mirror and the
> mirror navigated the app back out. A page draw may not steer the app now, and
> a gate walks the shipped home and fails if any screen navigates. Verified on
> the shipped exe: Play lands in the world, 0 panics. Nothing else is blocked
> by it; the video and platform work below is unchanged.

> **WATCHING THINGS IN THE WORLD, 2026-09-18.** Operator, in the console
> room: "can we actually play a video on the red line monitor ... or should we
> play a video that's stored on my PC?", then, on the platforms: watching
> YouTube, Twitch and Rumble on the in-world monitors "is what we want".
>
> **SHIPPED (v0.1318.0):** the video screen picks its own file. A control
> strip (Open, Play/Pause, name, time) that hides over a playing film, the
> in-app picker on the screen itself starting in Videos, the choice remembered
> per screen in `AppConfig::screen_media`, and one-time conversion with the
> machine's ffmpeg for anything the player refuses (cache keyed by path, size
> and date; percentage on screen; an honest message naming the fix). Proven on
> `wall_screen_5`: an H.264 + AAC MP4 converted, played with audio, moved
> 76,832 of 921,600 pixels between two snapshots a second apart, cache hit on
> the second open, verify-screens 12/12.
>
> **ALSO SHIPPED (unreleased, on main):** a folder is a disc. The app finds
> the disc structure on a drive or in a folder, picks the main title (largest
> title set, menu excluded, parts in order) and converts the whole title as
> one film; a protected disc is detected from its packet headers BEFORE any
> conversion and answers with one plain sentence. Proven on wall_screen_5: a
> two-part title joined into one film, a cache hit on the second open, and a
> scrambled copy refused with the Open button still reachable.
>
> **THE LEGAL POSITION IS NOW WRITTEN DOWN**, at the operator's direction:
> `docs/reference/media-stance.md` (what we do, what we will not do, a platform
> table so nobody meets an unnamed wall, and a commitment to switch a feature
> off publicly if anyone objects) and the Library page
> `docs/user/rights/laws_that_limit_this_software.md` (the rule, the harm, the
> free petition process that already exists for changing it, and the
> one-sentence reform to ask for). The standing norm is in CLAUDE.md: name the
> law, the patent or the platform rule where the person hits it, and say which
> of the three it is.
>
> **OPEN DEFECT, small:** a `watch:` screen with no connected server builds an
> address with no host and retries forever showing a URL parse error
> (`src/engine/screens/live.rs`, `gui_state.server_url` empty). It should say
> that there is no server to watch, in a sentence. Seen by the operator in the
> console room on v0.1318.1.
>
> **NEXT, in order.** **Route A, our own player on open streams:** an HLS and
> DASH client plus the operating system's licensed decoders (Media Foundation,
> VideoToolbox), which also settles H.264 and AAC without shipping a patented
> decoder, and an Owncast instance on the VPS so anyone can stream to
> united-humanity.us from OBS and be watched on the monitors with no third
> party involved. Mission-aligned and unblocked. **Route B2, the embedded
> browser:** Chromium rendered OFFSCREEN onto a screen surface through a
> SEPARATE opt-in host process (the default build never needs the Chromium
> SDK; the runtime is an opt-in, hash-verified download; our own watch page
> hosts each platform's OFFICIAL embed with their ads untouched; per-platform
> `embed.status` in `data/web/sites.json` is the legality record). The spike
> was dispatched and died in its reading stage with nothing written, so it
> starts fresh. **Refused as policy:** stream extraction the yt-dlp way (it
> breaks the platforms' terms, strips the ads that make embedding permitted,
> and invites a takedown against the repository that hosts our releases).
> B1 (a docked WebView2 panel) is dropped: it cannot project onto a 3D surface.

> **ACTIVE: THE FRAME COST ARC, 2026-09-18.** Operator: "despite the cloud
> layer being off the planet is tanking performance to ~12FPS. Do we have
> diagnostics to tell us where all the performance is being spent?" Measured
> that day at the operator's mirrored settings, clouds off, real GPU
> timestamps, 2560 x 1387 (21 boots; design of record
> `docs/design/frame-cost-arc.md`). The clouds are not the cost:
>
> - **The planet pass is per-pixel, independent of geometry.** `gpu.celestial`
>   is 44.5 ms on the moon with SIX patches drawn, 61 ms at the 400 km limb,
>   43 ms in the Sahara at noon; shadows, atmosphere scatter, SSAO, god rays
>   and water FFT move it by nothing; surface-detail octaves 9 to 11 ms, patch
>   count 17 ms only at grazing ocean views. About 12 to 15 ns per pixel where
>   the engine's own 40-tap fullscreen pass costs 0.12.
> - **The interior opaque pass is about 23 ns per pixel per layer** (one flat
>   quad filling the view costs 160 ms; the console room 76 to 83 ms of
>   `gpu.scene` plus 14 of glass); the 211-light untiled loop is only 8 ms of
>   it; early-z works (refuted: discard, overdraw); the camera wall adds 17 ms
>   only while in view.
> - **Forest vantages:** near-tree photoscans 68 ms of 156 at Fuji (516 trees
>   in range, no LOD ladder, no frustum test), cluster cards about 38, grass
>   17 (already the right technique). **Low ocean eyes:** the water shell pass
>   42 to 49 ms, heap-order alpha blend with depth write and no backface cull.
>
> **H1 CONFIRMED, P1 SHIPPED (v0.1315):** the seven PSOs are compiled from
> ONE 14,296-line module and four use `fs_main`; every fragment paid the
> module's per-invocation private storage (the cloud march's 2.9 KB
> `var<private> g_bc_lc`, reachable from `fs_main` through `cloud_layer` even
> with clouds off). Proven in one boot by a same-boot A/B through the shader
> hot reload: the moon's `gpu.celestial` 44.1 ms with the shell branches
> reachable, 7.4 ms with them unreachable, on a picture that draws none of
> them. Shipped as WGSL override constants (`05-overrides.wgsl`) guarding
> the atmosphere, cloud and ocean dispatches, with the two terrain pipelines
> compiled with all three off and a per-pipeline dead-branch registry pinned
> by tests. Gate at the operator's settings: moon 45.4 to 8.3 ms, Sahara
> 31.4 to 5.3, blue marble 5.0 to 1.1, limb 63.5 to 13.3 (10.9 to 24.1 fps),
> Fuji 85 to 69.
>
> **P2 SHIPPED (v0.1316.0, 2026-09-18, the per-class pipelines):** the ten megashader
> PSOs now compile per material class through the same override switches:
> General (all three shell branches off: the five classic PSOs and the two
> terrain PSOs), Shell (atmosphere and ocean on, cloud off: the atmosphere and
> water shells) and Cloud (only the cloud march on: the type-15 deck). A
> `shader_class(material_type)` classifier is pinned to the WGSL guard bands
> by a test that was proven red; the draw loops select the PSO by class
> (`transparent_for` / `overlay_for`) and switch only on change; the
> celestial transparent list is stable-sorted by planet layer band (the
> review caught a class-keyed sort lifting the fallback dome under the sea
> with scattering off; fixed with a pure key and four tests). At the
> operator's settings: limb `gpu.celestial_t` 25.0 to 0.37 ms (24.5 to 30
> fps, the floor of 25 met), Sahara 10.4 to 0.19 with the sky rows
> bit-identical, ocean-storm-low 48.8 to 1.39 (the water shell was paying the
> cloud march too, so W1 below is re-scoped), the home `gpu.scene` 11.4 to
> 3.0, the console room 82.9 to 16.8 ms with verify-screens 12/12, the moon
> unchanged. The operator then approved the modular shader system as the next
> rungs, not a rewrite: keep the part-file assembly and the switches, split
> the entry per class (P3), defer a data-driven material graph.
>
> **Next, in order** (ms saved per unit of risk, `docs/design/frame-cost-arc.md`):
> P3 SHIPPED (v0.1317.0): `fs_main` is gone; six class entries (surface,
> terrain, vegetation, water, shell, cloud) share `frag_prologue` and
> `frag_tail` in a new 80-fragment-shared part, thirteen PSOs each compile one
> entry, draw loops pick the opaque PSO by class, and a `[Pipelines]` line
> logs per-PSO compile times. Phase A on the shipped build (same-boot A/B
> against a union entry carrying the terrain and vegetation blocks): console
> `gpu.scene` 17.1 to 14.1 ms, home 2.7 to 2.2, so the split is a perf
> increment (register pressure), and the gate had every vantage faster or
> equal with pixels inside the rig floors. Cost: boot 6 s slower from three
> more PSO bakes; a wgpu pipeline cache is the named next step. **Rig guard
> shipped the same release:** `scripts/lib/machine-guard.js` is the one process
> query (HumanityOS, cargo, rustc, link, cl), probe-sweep waits before boot
> and samples before and after every capture, a contaminated capture is
> marked and perf-report refuses to grade it; `wind` and `anim_clock`
> showcase pins; `tests/rig_pin_lint.rs`. **Found by that work, OPEN:**
> `just lints` is RED on main at `file_size_ratchet` (lib.rs 19.9k lines vs
> an 18.1k budget, renderer/mod.rs 5.6k vs 3.9k, chat.rs, gui/mod.rs and six
> more), so `just verify` cannot pass and every downstream lint is bypassed;
> the fix is the owed extractions, not a budget raise. Rank it right after
> I1.
> V1 SHIPPED AND REFUTED (v0.1316.0): the near-tree colour draws are
> frustum-culled with the coverage arithmetic untouched (hide radius
> bit-identical, 151.9 m in both arms, no oscillation on a heading change;
> off-screen models kept as shadow-only casters because a conifer behind the
> camera still shades the ground ahead), and clean boots of both arms agree
> to 0.3 ms at Fuji with 145 of 260 photoscans removed from the colour pass:
> the whole near-tree cost is ON-SCREEN fragment work, about 0.47 ms per
> visible model, so the rung with reach is the LOD ladder and impostor
> handoff, after P3's phase A says what a foliage fragment costs once the
> terrain and vegetation blocks are folded apart (merged for the tested
> `NearTreeDrawPlan`, the `[NearTree]` 1 Hz counters and the shadow-only
> split the LOD rung needs; the refill variant was +16.5 ms and heading
> dependent, set aside); I1 the screen-emitter
> hoist plus `gpu.screen_scene` ids (the two console-room vantages
> `console-face-6` and `console-face-3` now exist as fixtures, floor 10);
> W1 re-scoped: the water shell is 1.4 ms after P2, so the depth prepass plus
> backface cull is now a fidelity and ordering call (deterministic nearest
> fragment instead of heap-order blend), not a perf item; then clustered
> lights, interior culling, a near-tree LOD ladder. Rejected on evidence: a
> masked-discard variant, an interior depth prepass, a G-buffer.
>
> **Instrumentation landed (v0.1315):** `cpu.frame_total`, `cpu.present_wait`
> and `cpu.fps_cap_sleep`; scopes on the screen surface pass, the camera sky
> pass, the particle compute and the sky-view LUT; the camera re-render, its
> overlay and lines under `gpu.screen_*`; the cloud keys, the water shell and
> the screens registered on the Performance page, held equal to what the
> engine records by a scan-based test; and a fix for a pre-existing defect the
> review found: opening the Performance page decayed every GPU row to zero
> within a third of a second (the GPU harvest ran on UI-only frames; it now
> freezes like the CPU column). Still missing: a split of bodies, patches and
> grass inside `gpu.celestial` (three passes over the same attachments).
>
> **Also from this day:** the F10 sidebar took no input because the HUD
> allocated a full-screen hover rect and egui 0.31's hit test stops at the
> first covering rect regardless of sense (BUG-076, fixed with a headless
> click test); Escape closes the expanded sidebar first and repeated Escape
> presses are dropped; `debug/ui_request.json` drives the main UI for the
> rig. **Fixed the same day:** `vsync: false` killed the app on its first
> rendered frame (BUG-077, a surface reconfigure inside the frame arm; now
> deferred to the next frame's start, proven with a negative control).
> **Closed the same day:** the Settings clamps that rewrote a typed 0 to 0.1
> at the next boot. Zero now means off for tree density, grass cover and blade
> detail (`AppConfig::apply` clamps to the shared range constants,
> `trees_in_cell` answers 0 in both vegetation streams, the grass harvest is
> gated by one pure predicate), six headless tests with positive controls on
> real ground; no web mirror exists for these sliders.

> **THE IN-WORLD SCREENS LADDER, 2026-09-16.** Operator direction:
> the native app pages (the real inventory page, not the web page) as
> touchscreens inside the 3D world; a fixed console room in the home populated
> by those displays; live feeds and movies on displays; real websites on
> in-game monitors, with affiliate links only after the legality of each
> platform is settled; and our own player rather than embedding VLC. Six
> rungs, in build order, with the state of each:
>
> 1. **ScreenSurface** (MERGED, v0.1313): a persistent second egui context
>    rendered into a wgpu texture that a new unlit material type 24 samples;
>    the look ray (mouse-look) or the cursor ray (free cursor) becomes synthetic
>    egui pointer events, so the player looks at the wall and clicks the real
>    inventory. `debug/screen_request.json` drives the same event path for
>    the rig. `docs/design/in-world-screens.md`.
> 2. **The screen as data** (MERGED): `screen: Some((source, px, face,
>    bezel_m, brightness))` on a machine def; `wall_screen` and
>    `desk_monitor` in both home designs; two placed in the common room. The
>    `source` string carries a scheme (`page:`, `watch:`, `camera:`,
>    `video:`, `web:`) and a `ScreenProvider` per kind plugs into the one
>    registry `engine::screens::provider_for`.
> 3. **Live feed and in-game camera** (MERGED, v0.1314): the MJPEG Watch
>    stream on a wall with one viewer per screen (a picture, once up, is never
>    overdrawn by the status page); a `camera_post` machine whose pose renders
>    the world onto a screen at 10 Hz, one camera per frame, through a
>    `render_view_onto` shared with the hires screenshot path, with a parked
>    depth texture and the live frame's daylight rule.
> 4. **The console room** (MERGED): room-grade zone types, `room_type` on a
>    zone joining `data/rooms.ron`, rooms covered by a zone take the zone's
>    id instead of `room_N`, a `console_room` zone placed in the home.
>    Vocabulary: command deck = the mothership bridge; console room
>    (battlestation) = the home workstation. All six screens now hang on its
>    three doorless walls (v0.1314): inventory and the garden camera west,
>    tasks and our own site east, the live stream and the demo clip north.
> 5. **Video** (MERGED, v0.1314): WebM/Matroska + AV1 + Opus through
>    pure-Rust decoders on a background thread with an audio-led clock
>    (`src/media`, `docs/design/media-player.md`); a looping clip on a wall
>    with its sound placed by distance and bearing, starting at the placed
>    mix, click-to-pause, notices at the display's size. Not done: sync
>    between players, seek, subtitles. rav1d's debug-only borrow checker
>    aborts about one debug test run in twenty; the device tests run in
>    release (documented in the media doc).
> 6. **The readable web** (MERGED, v0.1314): html5ever
>    parse, egui render, no JavaScript, no Chromium (decision brief 4),
>    behind `readable_web` which defaults to OFF; `data/web/sites.json` is
>    the one bookmark list both clients read and carries `embed.status` per
>    site (`needs_review` until a human records the terms basis, url, date
>    and name) with every affiliate field null. The view hangs on a wall
>    (off = a notice naming the switch, never a fetch) and
>    `scripts/verify-screens.js` (`just verify-screens`) proves with nobody
>    at the keyboard that a click on the inventory wall collapses a container
>    and a click on a link on our own site navigates: 12 of 12 checks green
>    on 2026-09-18. Test targets are our own site only; nothing affiliate
>    until the legality column says so. A placement gate on `embed.status`
>    is the next rung on this line.
>
> **Gates for any screen change:** both cargo checks, the screens / surface /
> dispatch / machines lib tests, the standalone lints, AND a boot that enters
> the world and drives the wall through the dev IPC (static verification
> cannot see a dark or mirrored screen). `just verify-screens` is the named
> gate. Known cost: the console room with all six walls live (a camera and a
> clip among them) ran at 9 fps on rig defaults; the screens' share of that
> frame is the next perf item on this line.
>
> **Deferred from this arc, on purpose:** VR controller rays; per-context
> `thread_local` page state (a screen and the main UI showing the same page
> share it); moving affiliate decisions out of `needs_review` (operator reads
> the terms); synchronised playback between players (needs the relay clock);
> the rooms.ron entries for entry, pantry, hall and utility (named, not yet
> functional).

> **NEW ARC FENCED, NOT YET STARTED: POPULATE THE SHIP, AND SEAT A DOZEN,
> 2026-09-15.** Operator direction, two calls. (1) The LLM-driven AI player is
> BACKBURNERED in favour of simple AI humans: 500 in one mothership sector that
> live their lives and visibly do things, billions eventually, on the reasoning
> that an LLM agent added to a working crowd framework is one more inhabitant
> rather than a special case. (2) Co-op must seat **at least a dozen** players,
> because a dozen people already want in; private-with-friends or MMO, but twelve
> is the floor. Architecture in `docs/design/crowd-simulation.md`, mode axes in
> `docs/design/game-modes.md`, measurement protocol in
> `docs/design/npc-crowd-stress.md`.
>
> **The architecture, in one line:** do not simulate 500 agents, because that is
> a dead end at 501. Three tiers instead: an aggregate population per zone that
> costs the same at 500 or 500 million, a roster whose members are DERIVED
> (`where_is(agent, t)` is a pure function, nothing ticks), and an embodied set
> promoted only for what is visible. Per-frame cost then scales with what you can
> see, not with who lives there. The keystone refactor is turning chores from
> countdown timers (`remaining -= dt` in a JSON value, which cannot be derived)
> into timetable entries, which simultaneously kills the network bill: the relay
> ships the seed and the timetable once, clients derive the crowd, only
> deviations sync.
>
> **MEASURED, not assumed.** No interior cost capture existed (all 577 on disk
> were cloud/altitude ladders), so one was taken: `home-clock-noon`, real GPU
> timestamps, operator settings mirrored, in `.probe-rig/sweeps/20260915-202858/`.
> The interior deck is **18.84 ms GPU with the six crew that ship today**
> (`gpu.scene` 12.619, `gpu.transparent` 5.072, `cpu.system.ai` 0.017). It is
> already over a 60 fps budget before one extra inhabitant exists, and **17.7 ms
> of that is unattributed and has nothing to do with NPCs**. So the first rung is
> not crowd work at all: capture the same vantage at a quarter of the pixel count
> and find out whether `gpu.scene` is fill-bound or vertex-bound. 30 fps is not a
> lowered bar, it is this project's own: 48 vantages carry a perf floor, the range
> is 5 to 30, and none is 60.
>
> **SHIPPED TODAY out of this investigation** (the co-op half, because it was a
> live bug rather than a design): the per-socket broadcast forwarder was
> `while let Ok(msg) = broadcast_rx.recv().await`, which treats a lagged receiver
> exactly like a closed channel, so the loop ended, the `tokio::select!` aborted
> the read task, and the whole WebSocket was torn down. A client that fell behind
> was evicted from the game world AND chat. At two players the 256-slot buffer is
> ~8 seconds of slack so it never fired; at a dozen streaming position at 15 Hz it
> is ~1.4 seconds, which an ordinary TLS stall exceeds, and it cascades into
> reconnect storms against `IDENTIFY_RATE_MAX`. Now `recv_skipping_lag`, matching
> what `live.rs:394` already did for video viewers. Extracted rather than fixed
> inline so it is testable, with three tests where the first asserts the setup
> genuinely lags (the second would pass against the bug otherwise).
>
> **THREE DOC CLAIMS CORRECTED, each of which would have mis-budgeted this arc.**
> The crowd-stress inventory counted `behavior.rs` and `flow_field.rs` as working
> systems; both are 26-line dead stubs with zero call sites and non-existent data
> files, under a comment claiming flow fields "support million-agent navigation".
> **Nothing in this repo can path an agent around a wall.** It listed
> `IntervalAction`, which exists nowhere; the real type is `AutonomousTask`, whose
> system is never registered and whose tick only increments a counter. And it had
> no row for animation: there is **no skeletal pipeline and no GLTF animation
> import at all**, so NPCs and remote players draw as a two-primitive body-and-head
> marker. A crowd on today's renderer is sliding markers. Separately,
> `mothership-superstructure.md:129`'s "instancing path is confirmed dead code" is
> half stale: `render_instanced` does have zero callers, but v0.1091 shipped a live
> instanced grass draw that is the template for crowd rendering.
>
> **NEXT on this arc** (each rung separately measurable): 0) reconcile relay and
> client worlds, since the relay simulates a multi-deck ship while the client
> renders the flat homestead and remote Y is clamped to a constant to stop crew
> floating in the sky; 0.5) attribute the 17.7 ms; 1) the `npcs:N` knob plus
> `[npc-diag]` counters, copying `lights:N`; 2) instanced crowd rendering, which
> also delivers per-NPC variety for free since `Appearance` already models it;
> 3) the timetable refactor; 4) promotion/demotion with interest management;
> 5) navigation; 6) animation. **Not started.** The content wave below is still
> the active work unless the operator says otherwise.
>
> **CONTENT WAVE SIX, AND THE RULE THAT CAME OUT OF IT, 2026-09-15
> (v0.1312.10 to v0.1312.16). 46 of 143 topics have a document, 20 are
> complete experiences with all four layers, against 37 and 11 at the start
> of the day. Run `just curriculum` for the live number.**
>
> **NEXT: keep writing, and budget a refutation pass PER GUIDE.** 97 topics
> still have no document. The cheapest are the 16 that already have both data
> and sources, so one document completes all four layers. `just curriculum`
> lists them.
>
> **THE RULE.** Eight guides landed this wave, written by parallel writers
> against a standing brief. Three were then handed to a critic whose only job
> was to refute, and all three came back with real defects: 8 in the ecosystem
> guide, 9 in the sky guide, 9 in the climate guide. Every one of those
> writers had self-reported thoroughly and confidently. **A writer's own
> report is not evidence.** The dominant defect is NOT a wrong number:
>
> 1. **A correctly-read source restated with its scope removed.** The Forest
>    Service said "in coastal Oregon" and the guide said "the Forest Service
>    records", then told a Kitsap gardener that soil pH 4.3 "is your starting
>    number". Our own `soil.json` contradicts it for half the mapped ground
>    here: Alderwood runs 5.1 to 6.5. Same shape four more times in the
>    climate guide alone, including a spring-only frost rule stated with no
>    season attached, which on the autumn ladder would have deferred row cover
>    by five weeks.
> 2. **A claim credited to a source that itself credits somebody else.** The
>    land-use correlation is Feist et al. 2011, and Scholz et al. 2011 says so
>    in its own discussion. Eratosthenes was cited to a NOAA page containing no
>    occurrence of Eratosthenes. The Longitude Act was cited to NIST pages
>    containing neither "Longitude Act" nor "latitude".
> 3. **A check that cannot fail.** The sky guide proved "a full moon is due
>    south at its highest" by sampling 23 minutes after the transit and
>    offering the resulting 8.8 degree miss as the confirmation. At the actual
>    transit it is a seventh of a degree off, sixty times better.
> 4. **An altered quotation.** NOAA's page says "5 states"; the guide printed
>    "2 states" inside the quotation marks, having noticed NOAA's count was
>    stale and corrected it in place.
>
> The standing brief is now on disk at `docs/contributor/library-writer-brief.md`
> instead of being re-derived from memory every wave. Hand a writer that path.
>
> **THREE NEW GATES, each from a failure that had already shipped.**
>
> - `just check-library-render` runs the REAL web reader over every shipped
>   document and inspects the output. All six existing Library gates read the
>   markdown source; none had ever looked at a page. First run found an
>   underscore inside a word opening emphasis (not the GFM rule), so
>   `what_soil_is.md`, `silverdale_wa`, `poison_hemlock` and a set of CITED
>   SOURCE URLS lost their underscores and went italic: 82 spans, 23 documents.
>   **Native had none of them**, so it was findable only by opening both
>   clients side by side, which nobody does.
> - Four tests in `src/gui/widgets/markdown.rs` so the native reader is gated
>   too. That asymmetry is the real lesson: gate both mirrors or neither.
> - `just check-library-counts` refuses a document asserting a locale record
>   count the data no longer supports. Three sea star records landed and
>   silently falsified five sentences across three guides. On its first run it
>   found a document nobody had mentioned; making it newline-tolerant found
>   another that had survived a search-and-replace by wrapping across the line
>   break between the number and the word.
>
> **21 TOPICS WERE WIRED TO DATA THAT WAS ALREADY ON DISK.** The curriculum
>   cited 19 of 210 data files. `data/chemistry/toxins.csv` carries botulinum,
>   saxitoxin, amatoxin and aflatoxin, which is exactly what the Keeping Food
>   and Wild Food ladders are about; `data/medical.ron` carries the conditions
>   and procedures the Health ladder needs. "has simulation data" went 20 to 35
>   percent and "ALL FOUR" did not move, which is the proof it was not gamed.
>   Deliberately NOT wired: `materials_adhesive`, whose only matching rows are
>   `glue_0` and `adhesive_slime_0`.
>
> **GAPS REPORTED BY WRITERS, NOT YET FIXED.** Each is a place the curriculum
> promises something the data cannot support:
>
> - `data/laws/laws.json` is the declared data layer for BOTH `greywater` and
>   `forage_ethics` and has nothing for either: no licences, seasons, limits,
>   Migratory Bird Treaty Act, Marine Mammal Protection Act, tideland
>   ownership, or treaty rights. There is a rule about licensing your dog.
> - `data/chemistry/alloys.csv` records ONE of the four strength properties its
>   topic promises and has no temper column, so 6061-O and 6061-T6 are one row.
>   Five more defects listed in the v0.1312.16 commit.
> - `plants.csv` and `creatures.csv` have no pest or disease field, no damage
>   mode, and no beneficial predators, so the pests guide's central lesson (a
>   broad spray removes the control you already had) is unmodellable.
> - `items.csv` cannot express edge state, tool condition, a workholding
>   requirement, or a mallet.
> - `data/constellations.json` is missing Cepheus and Hydrus. Cepheus is the
>   awkward one: it adjoins Polaris on the Cassiopeia side.
>
> **STILL OPERATOR-ONLY.** Releases v0.1308.0 onward are unsigned, so the
> desktop updater offers nothing. And the two demoted lethal guides
> (`/library#making-water-safe-to-drink`, `/library#keeping-what-you-grew`)
> stay at `sourced` until a human has read them, per the rule the operator
> chose; `curriculum-status.js` enforces it.

> **THE LIBRARY AS A TEACHING SYSTEM, 2026-09-15 (v0.1307 to v0.1309). The
> operator: "let's focus on getting the library to 100% UX/UI/content wise. The
> real library content and the databases of stuff (like plants, animals, cities,
> planets, equipment, etc.) essentially functions as our framework for the
> gameplay. We want to teach real-life through a simulation." Backend first, by
> his agreement, then content.**
>
> BACKEND, now complete:
>
> - **A denominator.** `data/curriculum/syllabus.json`, 143 topics across 18
>   subjects, practical-first. "How complete is the Library" had no answer
>   before this because there was nothing to divide by. Read it with
>   `just curriculum`.
> - **Four layers per topic** (reading, skill, data, source), because a topic
>   with a document and no data is a book, and a topic with data and no document
>   is a mechanic nobody understands. At the time of writing: skills 100
>   percent, documents 11, simulation data 20, sources 52, all four 0.
> - **A source registry.** `data/sources/registry.json`, 54 authorities each
>   carrying a licence and a use rule, so the operator decision "public domain
>   in the bundle, share-alike fetched at runtime" is checkable rather than
>   remembered. Roles like `state-fish-wildlife` resolve per locale, which is
>   how a topic stays location-generic.
> - **A locale schema.** `schemas/locale.toml`. 66 of the 143 topics (46
>   percent) have no correct answer until you name a place.
> - **Library structure**: three tiers (section, category, document), a tag
>   filter on four axes, lazy full-text search, working cross-references, Back,
>   a Contents outline per document, and heading anchors shared by both clients
>   so `/library#<doc>/<heading>` resolves in the app and on the website.
>
> WHAT THE NEW GATES CAUGHT, both on our own work:
>
> - The fire staff guide was `verified` at `hazard: lethal` while citing two
>   phrases, "published alloy and fibre data" and "manufacturer SDS". Its flame
>   temperatures traced to a self-published dataset that lists naphtha at
>   4591 C. Re-sourced against OSHA 1910.106, NIOSH, CAMEO, ATSDR, the NASA
>   Materials Data Handbooks, NIST and the named DuPont guides; ten body facts
>   corrected; status honestly reduced to `sourced`.
> - The markdown renderer had no code-fence handling at all, so SELF-HOSTING.md
>   drew 50 shell comments at title size and buried its real sections.
>
> WHERE IT STANDS AT THE END OF 2026-09-15:
>
> - **33 of 143 topics have a document (23 percent), 18 are sourced, 11 are
>   verified, and 8 are complete experiences with all four layers.** Every
>   `data` reference in the syllabus resolves for the first time. Read it with
>   `just curriculum`.
> - **Silverdale is complete as data**: eight locale files plus 122 species,
>   every value from a federal source, and a readable gazetteer generated from
>   the same data the simulation runs on.
> - **The Library has three faces** (documents, dictionary, curriculum), three
>   tiers, tag filters, lazy full-text search, cross-references, Back, a
>   Contents outline per document, heading anchors shared by both clients, and
>   ladder navigation at the foot of every page. Learn is cut into six ordered
>   stages so the rail stays scannable as it grows.
>
> NEXT, in value order:
>
> - **Content. 110 of 143 topics still have no document.** That is the honest
>   headline and the whole remaining job on this axis. Prefer topics whose
>   LOCALE DATA already exists, because those complete all four layers in one
>   pass: that is how earth_tides, earth_hazards, earth_soil_types and
>   grow_calendar became complete experiences.
> - **The lethal-hazard subjects are the hard part and must not be written
>   fast**: water-bath and pressure canning, botulism, every foraging topic,
>   where amateur electrical work stops, generators, heating a space, human
>   waste, and hunting. Each needs the treatment the fire staff guide got, and
>   the pattern that worked for species data applies: write it, then have a
>   separate pass try to refute it.
> - **Terrain.** The locale POINTS AT USGS 3DEP and NOAA hydrography rather
>   than embedding them, so Silverdale is not yet the ground you walk on. This
>   is the largest single piece of the operator's "Silverdale is the world"
>   decision that remains untouched.
> - **The runtime fetch layer.** Five registry entries (GBIF, iNaturalist,
>   OpenStreetMap, Wikipedia) are marked `use: fetch` and nothing fetches yet,
>   so the share-alike half of the licensing decision is still a promise with
>   no mechanism.
> - **Fifteen lookalike species are named-but-absent**, all non-lethal
>   (knotweeds, brooms, gorse, tansy, groundsel). The deadly pairs are closed.
> - **`just verify` is red on the monolith ratchet**, not on any test. Six
>   files past budget, about 5000 lines across several sessions. It wants a
>   quiet checkout: extracting from lib.rs and renderer/mod.rs while other
>   sessions are live in them is the three-way merge hazard CLAUDE.md warns
>   about. Everything else is green, and a fresh release binary was built and
>   booted on 2026-09-15 with 0 panics and 0 errors.



> **LIBRARY DOCUMENTATION AUDIT, 2026-09-14 (v0.1306.x). The operator asked
> whether the docs we ship are accurate, starting with the US Constitution.
> Both halves of that question are now answered and neither answer was good.**
>
> The Constitution was rebuilt from the National Archives parchment
> transcription after the House Rules and Manual proved to be the wrong kind of
> source: it carried three genuine word errors into the shipped text, 14
> manufactured `* * * * * *` elision marks inside complete sentences, 5 invented
> bracketed subjects printed as constitutional text, and it dropped Article I
> Section 8's granting clause so the enumerated powers shipped with no grantee.
> Full account in the v0.1306.1 commit and `scripts/build-constitution.js`.
>
> Then an eight-batch adversarial sweep covered the other 80 Library documents:
> 223 agents, 107 findings, **55 unanimous across two verifiers**. The two
> criticals are fixed (a quoted heredoc that gave every self-hoster the same
> published `API_SECRET`; a social-recovery promise with no implementation
> behind it). **53 unanimous findings remain open**, listed per file with
> location, reason and fix in `docs/history/2026-09-14-library-audit.md`.
>
> NEXT, in rough value order:
>
> - **Contributor docs describe a product that no longer exists.** `01-VISION.md`
>   and `two-realities.md` both describe a Real/Sim toggle removed in v0.197.0,
>   `06-SOURCE-OF-TRUTH-MAP.md` calls shipped domain systems "early/planned",
>   `development_loop.md` tells contributors to `cd server && cargo check`.
> - **SELF-HOSTING has 4 more unanimous findings**, including an nginx
>   `limit_req_zone` inside `server {}` that nginx refuses to load, and a Quick
>   Start that hands self-hosters the desktop GPU binary instead of the relay.
> - **`ui-system.md` has 5**, including a migration table marking shipped items
>   unshipped and token tables that disagree with `data/gui/theme.ron`.
> - **`fire_staff_materials.md` has 6**, all mine from 2026-09-08: two wrong
>   flash points, a cost-table price for the spec the document says not to buy.
> - **22 contested findings are UNADJUDICATED**, not refuted. The water-filter
>   one is a real CDC-vs-CDC conflict worth resolving deliberately.


> **TYPOGRAPHY, v0.1300.0 to v0.1301.1 (2026-09-07). The operator asked why the
> project uses so many different fonts and whether one could be cohesive between
> web and app. The answer was that nobody had ever chosen one, twice: native
> inherited egui's four bundled faces, web took a system stack that renders a
> different typeface on every operating system. He then chose the long-term
> answer on three criteria: user safety, ease of use, and global access.**
>
> Two evaluations, 33 agents, recorded in full in `docs/design/typography.md`.
> The decisive finding was not about typefaces. A 2026 Annals of Dyslexia
> meta-analysis pooled 15 studies, 91 effect sizes and N=688 on special dyslexia
> typefaces and found g = -0.04. Marinus et al. 2016 explains why the folklore
> survives: Dyslexie beat Arial by 7 percent, then they matched Arial's SPACING
> to Dyslexie's and the advantage vanished. **The benefit is spacing, and
> spacing is a CSS property and an egui parameter, not a font file.**
>
> SHIPPED:
>
> - **The seed phrase was proportional.** `settings.rs` rendered the 24 BIP39
>   words as a bare proportional label at 11.9px inside a frame whose own
>   comment calls it "the single most dangerous string in the app", while
>   `main_menu.rs` rendered the same words in monospace 40 lines away. Eight
>   character-by-character sites now use Hack, including both base64 invite
>   ticket fields, which are the only strings in the product where 0, O, I and l
>   coexist (base58 omits all four, keys are lowercase hex, BIP39 is lowercase
>   a-z). Zero bytes.
> - **Every arrow in the proportional UI was tofu on Linux and macOS.** Measured:
>   Ubuntu-Light carries 0 of 112 arrows, Hack carries 109, and epaint puts Hack
>   in Monospace only. U+2192 appears 111 times in `src/gui`. Hack is now the
>   computed-position fallback in Proportional. Four entries left BROKEN_GLYPHS.
> - **`set_fonts` lived inside the emoji branch**, so on a machine with no system
>   emoji font it never ran at all. Hoisted out.
> - **Japanese and Chinese were tofu**: no bundled face has a single CJK
>   codepoint. The system-font probe now covers CJK for zero redistributed bytes.
> - **The snapshot rig rendered a font stack nobody runs** (bare contexts, no
>   fonts installed) and BROKEN_GLYPHS was partly derived from it. All seven
>   contexts now install the app's real chains.
> - **Noto Sans is the interface face**, native and web. 2,965 codepoints against
>   Ubuntu-Light's 1,194, complete Cyrillic and Greek Extended. IBM Plex Sans,
>   which I had recommended, measured as a script REGRESSION: 895 codepoints and
>   FEWER Cyrillic than the face it would replace. Web self-hosts 8 variable
>   woff2 subsets with unicode-range, so an English reader pulls 35 KB; the CSP
>   already said `font-src 'self'`.
> - **The web dyslexia toggle was a placebo AND a no-op** on Linux and Android
>   (fontconfig ships no Comic Sans metric alias). Now "Reading Comfort" at the
>   WCAG 1.4.12 doses, with word spacing coupled to letter spacing because
>   raising letter spacing alone measurably slows readers.
> - **The star catalogues are CC BY-SA 4.0**, verified from both upstream LICENSE
>   files. `data/stars.csv` and `stars.bin` are adapted material shipping in
>   every release, so LICENSES.md now carries a second share-alike offer.
>   `credits.ron` had ONE star row labelled `athyg` describing HYG; now three,
>   matching the three tiers the UI offers.
>
> NEXT, in this lane, in order:
>
> 1. **Emphasis is conveyed by colour alone in the native app.** egui's
>    `strong()` changes colour, not weight, and no Bold ships. That is a real
>    accessibility gap. Needs a named bold family plus a `widgets` helper. Noto
>    Sans Bold was downloaded and deliberately deleted rather than shipped
>    unusable.
> 2. **Native has no Reading Comfort control**, which is the dual-UI drift
>    CLAUDE.md forbids. `RichText::extra_letter_spacing` and `line_height` exist
>    and are already used on the seed phrase; wiring them to a theme token is
>    the work.
> 3. **`font_size_small` is 11.9 and is the size used for the highest-stakes
>    string.** Raising it is the best-supported accessibility change left.
> 4. **`data/credits.ron` is only loaded inside `load_world`**, so Settings >
>    Credits shows a "could not read" warning until the user first enters the 3D
>    world, blaming a file that ships fine.
> 5. ~~**The Library markdown renderer** still has no table support and splits
>    emphasis across line breaks.~~ **BOTH FIXED, verified v0.1312.11.** Both
>    readers now buffer and join wrapped source lines before applying inline
>    markup, and both parse GFM pipe tables. Proven by running the real web
>    reader over all 107 shipped documents: 118 tables render, 0 separator rows
>    leak, and 532 emphasis spans that cross a line break all close.
>    `just check-library-render`.
>
> ---
>
> **NEXT IN THE NON-GAME LANE: signed moderation logs, rung 3.** Rungs 1 and 2
> shipped today (v0.1298.0 schemas + KAT, v0.1299.0 relay-side enforcement with
> 15 tests, attacked twice, seven holes found and fixed). Rung 3 converts the
> remaining unsigned mutation paths: `/ban` and `/mute` typed in chat, and the
> profile-modal buttons, which still write to the database with no signed
> record. Until it lands the log records only actions taken through the new
> path, which is why `web/pages/rules.html` says "the moderation audit log is
> half built" and why that sentence must change in the same commit.
>
> Read `docs/design/signed_moderation_logs.md` first: its "Current state"
> section now carries what rung 2 actually does, and the two independent
> authority guards that must both survive (the space_id equality check and the
> owner-is-already-admin anchor). Deleting either one leaves the whole suite
> green, so they are easy to mistake for redundant.
>
> Also open in this lane, all small, all surfaced by the public claims audit
> below: the web marketplace's dead message box, desktop task-board
> persistence, and putting the trust score on a listing.

> **v0.1299.1 (2026-09-06): THE PUBLIC CLAIMS AUDIT, front-door and Library
> lane.** `docs/history/2026-09-06-public-claims-audit.md` verified 35 claims
> against the code; the 22 under `web/pages/`, `docs/outreach/` and
> `data/library/` are now fixed, the 13 root-document ones were the other
> session's lane and are already committed.
>
> The download page said "It keeps itself up to date" and "You never need to
> re-download manually". The updater only installs a release carrying
> `release-manifest.json`, and 11 of the last 12 releases are unsigned, so a
> user on the exact build that page hands them is offered nothing. Reworded to
> describe the gate, not the current signing state, because unsigned-latest is
> this project's normal condition. The public-leaders brief promised a fork
> "owes nothing to anyone: no fee, no credit"; the OpenStreetMap regions we
> commit are ODbL with attribution and share-alike. It also named marketplace
> listings and shared tasks as "signed entries that cannot be quietly altered",
> in a section titled "The guarantee", when both are plain mutable SQLite rows.
>
> **WHAT THE AUDIT EXPOSED THAT IS A PRODUCT GAP, NOT A COPY GAP.** Each of
> these was fixed in the wording; the underlying hole is still open, and each
> is small:
>
> 1. **The web marketplace's message box is dead.** `market-app.js` still
>    sends `{type:'listing_message_send'}`, a message type deleted server-side
>    in the sealed-sender cutover, so it fails to deserialize and is dropped at
>    `relay.rs`. A buyer types a question, presses Send, sees the box clear and
>    "No messages yet", and nothing was sent and no error was shown. The app
>    does this correctly (`market.rs` "Message Seller" opens an E2EE DM); the
>    web page just needs the same treatment. **Cheapest real win on this list.**
> 2. **The desktop task board never persists.** `tasks.rs` New Task pushes a
>    struct into memory; there is no `ws_client` send and no save path, and the
>    next `task_list_response` clears it. So the board is empty offline, every
>    time, which is exactly the scenario the outreach docs invite a reader to
>    test. Desktop notes are the same shape (`GuiNote`, no serde, no save).
> 3. **The trust score is computed and shown nowhere useful.**
>    `GET /api/v2/trust/{did}` answers live and every input is published, but
>    neither marketplace UI mentions it; you must paste a DID into the Identity
>    page. Putting it on a listing is the point of having it.
>
> Left for another lane: root `CREDITS.md:37` still says "~1,300 releases"
> (2,041 is the real figure, and it was the third disagreeing number of three).
> And `index.html`'s FRAUD block says "Records are cryptographically signed and
> cannot be quietly altered", the same overreach as the one fixed in the
> leaders brief, but it was not itself an audited finding so it was flagged
> rather than rewritten.

> **REAL SKILLS, SECOND RUNG (2026-09-06). Four new Library guides, written
> because a verified account with a real audience reposted the project that day
> and sent traffic to /library, and the operator replied in public "more going
> in as I write them."** Content-only session, no `src/`, no `web/`, no
> `assets/`, run alongside the front-door and far-rung work in the same
> checkout.
>
> The gap this closes: the first rung (v0.1096) taught someone to grow a tomato,
> which is a wonderful first success and nutritionally minor. Measured against
> the project's own thesis in `the_five_adversaries.md` (poverty is needs gated
> behind permission, the antidote is capability), a Library that stops at salad
> is not yet making good on "helping educate humanity on STEM and the trades."
>
> SHIPPED, all four under Real Skills, every number sourced to a university
> extension service, USDA, FDA, CDC, EPA or WHO:
>
> 1. **`saving_your_own_seeds.md`.** The purest expression of the thesis: a seed
>    you saved cannot be gated. Two rules that decide everything (open-pollinated
>    never hybrid; start with self-pollinating annuals), the traps named
>    (biennials need two seasons, the SDSU cucurbit crossing groups, corn),
>    wet and dry processing, the sub-8-percent drying target, storage, and the
>    germination test. Publishes the Maine and Colorado State longevity tables
>    side by side and says out loud that they disagree (lettuce 5 years vs 1),
>    with the advice to plan on the shorter number and settle it with a test.
> 2. **`growing_food_you_can_live_on.md`.** The honest counterweight to the
>    tomato, and it says so in its first line: a pound of tomatoes is about 82
>    calories, a pound of potatoes about 350, a pound of dry beans about 1,550
>    (USDA FoodData Central). Potatoes, dry beans, winter squash with real
>    extension yields per 100-foot row and the calories-per-row arithmetic shown
>    so a reader can redo it. Prints Iowa State's potato figure and Utah State's
>    side by side, a factor of three apart, and explains that the spread is
>    real rather than an error.
> 3. **`keeping_what_you_grew.md`.** The one with genuine safety stakes.
>    Botulism first, the pH 4.6 line, 212 F vs 240 F, and the rule stated in a
>    block quote: low-acid foods (which is ALL fresh vegetables) cannot be
>    water-bath canned. Freezing, drying and root cellaring in full; exactly ONE
>    complete tested canning procedure (NCHFP crushed tomatoes, with the
>    acidification table and the altitude table); pressure canning explicitly
>    OUT of scope with a pointer to the USDA Complete Guide rather than a half
>    lesson. Spoilage signs, the do-not-taste rule, and NCHFP's disposal and
>    bleach-cleanup procedures.
> 4. **`making_water_safe_to_drink.md`.** Companion to `storing_water_safely.md`
>    (which covers the calm case; this one covers an unknown source), cross-linked
>    both ways. Frames treatment as three separate problems (germs, dirt,
>    chemicals) that no single method solves. The new material is the filter
>    chapter: CDC's pathogen sizes against required pore sizes, microfiltration
>    vs ultrafiltration vs reverse osmosis, why carbon filters are not treatment,
>    and the EPA purifier standard. Ends on the row of the table that is entirely
>    empty: nothing here fixes chemical contamination, and boiling concentrates it.
>
> THE ADVERSARIAL RE-READ CAUGHT TWO REAL HAZARDS IN THE FIRST DRAFTS, which is
> the reason that pass is mandatory and not a formality:
>
> - **Raw dry beans are toxic and the guide did not say so.** A guide telling
>   beginners to grow dry beans had no cooking warning. Phytohaemagglutinin: the
>   FDA Bad Bug Book records that four or five improperly cooked red kidney beans
>   cause severe vomiting. Added K-State's procedure (soak 5 hours, discard the
>   soak water, boil 30 minutes in fresh water) AND the counterintuitive part,
>   do not cook dry beans in a slow cooker, since below boiling it leaves the
>   toxin intact while making the beans soft enough to eat.
> - **A blanching table quoted without its own footnote.** Colorado State's
>   times (green beans 4 min, broccoli 4, diced carrots 3) carry the note
>   "blanching times given are for 5,000 feet or higher. At altitudes below 5000
>   feet, subtract one minute." Colorado wrote for Colorado. Most readers are
>   below 5,000 feet, so the guide now prints both and uses it as the worked
>   example of checking whose altitude a table was written for.
>
> Also added: seed potatoes are frequently fungicide-treated, plant them do not
> eat them; tomato acidification is required even when pressure canning; use
> Mason-type jars; bleach must be plain sodium hypochlorite only; and the
> *Giardia* row of the water table softened to "less reliable" because CDC's own
> two pages differ in emphasis and the cautious reading wins.
>
> TWO RENDERING DEFECTS FOUND BY LOOKING AT THE DEPLOYED PAGE, not the source,
> and both were mine (fixed in the two commits after v0.1297.1):
>
> - **Markdown tables do not render in the Library.** `web/pages/library-app.js`
>   has no table support at all: the deployed page showed 0 `<table>` elements
>   and 10 visible `|---|` separator rows. All nine tables were rewritten as
>   bulleted lists with every sourced number unchanged. ~~**No Library document
>   should contain a markdown table until the renderer supports one.**~~ None of
>   the six older Real Skills guides used a table, which is why this had never
>   been hit. **SUPERSEDED: both readers gained pipe tables, and as of v0.1312.11
>   that is proven rather than assumed. Tables are fine to write. The one
>   remaining shape that does NOT render is a table indented inside a list item,
>   which `check-library-render` now refuses.**
> - **A `**bold**` span that wraps across a line break never closes**, because
>   the renderer processes markdown line by line, so the reader sees literal
>   asterisks ("per pound**. Protein 2.1 g"). 22 lines across three guides, all
>   from hard-wrapping at 72 columns without checking the emphasis markers
>   survived. Now checked mechanically: every bold and italic span opens and
>   closes on one source line.
>
> ~~STILL OPEN, web lane, cosmetic: `library-app.js` renders each source line as
> its own block, so a wrapped list item loses its hanging indent and the
> continuation sits at the left margin.~~ **FIXED.** `web/shared/markdown.js`
> buffers paragraphs, list items and block quotes and joins them before applying
> inline markup, and `src/gui/widgets/markdown.rs` does the same. Guarded by
> `just check-library-render`.
>
> **A THIRD RENDERING DEFECT, found v0.1312.11 by a check that looks at the
> rendered page instead of the source.** An underscore inside a word opened
> emphasis, which is not the GFM rule. So `what_soil_is.md`, `silverdale_wa`,
> `poison_hemlock` and, worst, a set of CITED SOURCE URLS lost their underscores
> and went italic from the second underscore to the next: a reader clicking a
> citation in `cold_and_hypothermia.md` or `insulation_and_heat_loss.md` got a
> mangled URL. 82 spans across 23 shipped documents. The NATIVE reader showed
> every one of them correctly, because it parses links character by character
> rather than by regex, so this was a web-only divergence findable only by
> opening both clients side by side. Fixed in `web/shared/markdown.js`; the
> guard is proven red by reverting it.
>
> WIRING: `scripts/build-library.js` gains the four entries (it is in the same
> "ui" lane as `docs/` and `data/library/` per `data/coordination/lanes.json`),
> Real Skills is now a ten-rung ladder in order (grow, multiply, save seed, feed
> yourself, keep the harvest, close the soil loop, collect water, store water,
> make water safe, make power). `data/glossary.json` +18 terms (botulism,
> solanine, open-pollinated, hybrid seed, biennial, blanching, root cellar,
> water-bath and pressure canning, micron, absolute pore size and the rest),
> 442 to 460. `check-doc-links.js` stays at 0 broken; all 32 external source
> URLs were checked live.
>
> NEXT, if this arc continues: the obvious remaining holes in Real Skills are
> **cooking dry staples** (the bean-toxin warning is currently buried inside a
> growing guide and deserves its own rung), **a first-aid rung**, and **growing
> in a cold climate / season extension**. None is claimed or started.
>
> ---
>
> **v0.1296 (2026-09-06): THE FRONT DOOR. A verification pass measured the live
> site and relay against what the project promises in public, and the first
> minute of a stranger's visit was the worst part of the product.** Non-game
> session, ran alongside the far-rung cloud arc without touching src/renderer,
> assets/shaders or src/terrain.
>
> SHIPPED (v0.1295.0, v0.1296.0). **The DM verification wall is gone**: sending
> a private message required the `verified` role, reachable only through the
> admin-only /verify command, so writing to another person was a permission a
> human granted one account at a time. The knock budget from the 2026-08-24
> sealed-sender work (20 a day to strangers, lifted by a client-held friendship
> certificate) sat BELOW that check and was therefore dead code for every
> ordinary member. Role check removed, every other limit kept; the web client
> enforced an even stricter rule and had to change too. **A muted person could
> still DM**: /mute has written to the muted_members table and left user_roles
> alone since v0.246, but the DM path only ever compared the legacy role, so it
> matched nobody. Public chat called is_muted the whole time, which is why it
> stayed invisible. Fixed, and red-proved by reverting the condition. **Federation
> peer management became an admin surface**: POST /api/admin/federation (add,
> add_key, trust, remove, signed over its own "admin_federation" purpose so a
> stats signature cannot be replayed) plus web admin controls; native has had a
> panel since v0.722, so STATUS.md saying "no admin UI to add/trust peers yet"
> was stale. **The web can create proposals**, not only vote. **/rules is
> published**: the rules, the real moderator powers read out of the enforcing
> code, the appeals route including one that works while banned, and an explicit
> list of the accountability this server does not have. Plus: technical jargon
> defaults to hidden for first-time visitors, four pages that still called our
> identity Ed25519 corrected, the download page's false "full access to all core
> features" replaced, the market importer refusing to publish its demo catalog to
> a real server, dev pages collapsed in the site menu, and `just brief` printing
> the release-signing VERDICT instead of the table header that was hiding it.
>
> REFUTED, so nobody re-does them: the landing page's join form is NOT off
> screen (it renders at scrollY 0 at both 1280 and 375; the original y=3483
> measurement came from a browser pane with innerHeight 0, the same
> zero-viewport trap that also produced a false overflow reading later the same
> session). Web voting is NOT missing; governance.html has signed vote_v1
> voting, KAT-locked. The Rivertown Bikes fixtures in market_payloads.rs are
> inside a `#[cfg(test)]` module, so the live fake directory came from a manual
> importer run, not from the relay.
>
> NEXT, in order, all discovered while doing the above:
>
> 1. **Signed moderation logs, rung 1.** The design was revised this session to
>    resolve what blocked it for four months (the relay cannot sign, because it
>    does not hold a moderator's key, so the moderator's CLIENT signs). Schemas
>    and cross-language builders for `mod_action_v1` and `space_policy_v1` with
>    a KAT, the way vote_v1 has one. Rung order is in
>    `docs/design/signed_moderation_logs.md`. Until rung 2, a ban is still an
>    unsigned row with no banned_by, no reason and no expiry, and /rules says so
>    out loud.
> 2. ~~**Canonical CBOR is not canonical across clients.**~~ **WITHDRAWN
>    2026-09-06, this was wrong and there is nothing to fix.** Rust DOES sort:
>    `to_canonical_bytes` (`src/relay/core/encoding.rs:18`) calls `canonicalize`,
>    which sorts map keys at `:62-66` by length then bytewise, the identical rule
>    to `cborMap` in `web/shared/canonical-cbor.js:96-108`. `cbor_map` alone is
>    unsorted and its own doc comment says so ("NOT yet canonicalized, call
>    to_canonical_bytes"); `payload_cbor` routes through the canonical encoder.
>    Multi-key payloads are byte-identical across languages, which is why a
>    browser-built proposal verified on the Rust relay. The original claim came
>    from reading `cbor_map` and stopping one function short. Rung 1 above is
>    NOT blocked by this. Do not "fix" the encoder: changing a signing byte
>    format that is already correct would break every client at once.
>    (For text keys, RFC 7049 length-first and RFC 8949 bytewise are the same
>    function, so the length-first rule is not a nonstandard variant. Byte-string
>    keys are the only case where they diverge, and nothing uses them.)
>    Separately and deliberately: `src/gui/pages/market_publish.rs:371` bypasses
>    the canonical encoder via `payload_raw` because market prices are floats and
>    the canonical encoder prohibits floats. That is documented in place and is
>    not a bug, since the envelope signs the payload as opaque bytes.
> 3. **A proposal has no readable title ON THE WEB.** Corrected 2026-09-06:
>    NATIVE is fine. `src/gui/pages/governance.rs:91` `payload_texts` already
>    decodes title and body from the signed payload. Only
>    `web/pages/governance.html` is broken: it renders `proposal_type` as the
>    card heading (`:298`) and the object_id as the body (`:304-305`). Fix on the
>    CLIENT, not the relay: one extra bulk call to
>    `GET /api/v2/objects?object_type=proposal_v1&limit=N` (the endpoint and the
>    `listObjects` helper both exist, and the market pages already do exactly
>    this), joined by object_id. Do NOT add the decode to the relay's proposals
>    handler: `src/relay/storage/governance.rs` reads through `with_conn`, the
>    single WRITER mutex, so a blob read plus CBOR decode per proposal would sit
>    on the relay's only write connection behind a public unauthenticated
>    endpoint.
> 4. **The two moderation paths are not equivalent.** The `mod_action` path
>    refuses self-targeting and refuses a non-admin acting on an admin
>    (`src/relay/handlers/msg_handlers.rs`); the slash path in
>    `src/relay/handlers/broadcast.rs` has neither, so a mod can /kick or /mute an
>    admin, and can kick themselves. Also /kick means two different things
>    depending on which path was used.
> 5. **Reports go nowhere trackable.** No status column, no resolution, and the
>    notification reaches only moderators connected at that instant. /rules admits
>    both, which is the honest stopgap, not the fix.
> 6. **OPERATOR ONLY: 11 of the last 12 releases are unsigned**, so every v0.421+
>    desktop client has no update path and no error. `just sign-release vX.Y.Z`
>    needs the passphrase and cannot be done by an agent. `just brief` now shows
>    the verdict on every session start, and the uptime workflow warns off-box.
> 7. **`app/web/` is a 286-file tracked duplicate that nothing DEPLOYS but
>    something REGENERATES.** Corrected 2026-09-06: it is not unreferenced.
>    `Justfile:43` runs `just bundle-web` inside `ship`, and `Justfile:55` runs
>    `git add -- app/web`, so every `just ship` rebuilds those 286 files and
>    stages them into whoever ran it. `scripts/sync-web-root.sh` still reads only
>    `web/`, so none of it reaches the site. Deleting the directory alone is
>    undone by the next `ship`. Remove `scripts/bundle-web.js` and the Justfile
>    lines in the SAME commit, and mind the order: `:43` is unprefixed and fatal,
>    so deleting the script first breaks `ship` after `bump` has already mutated
>    tracked files. It has now misled two sessions mid-grep.
>    RESOLVED 2026-09-06 as far as an agent should take it: NOT deleted, because a
>    July session already put keep-or-drop to the operator as an explicit decision
>    (see the "STALE app/web/ BUNDLE" block further down this file) and dropping it
>    removes a half-built offline-bundle feature rather than tidying junk. The trap
>    is defused instead: `scripts/bundle-web.js` now writes `app/web/README.md`
>    saying the tree is generated, where the real sources are, and that the
>    keep-or-drop question is open. The bundle was also resynced (it had drifted 68
>    files since 2026-07-31). What remains is the operator decision, unchanged.
>

> **v0.1294 (2026-09-06): FAR RUNG 4c. The in-deck blackout and the band's
> veil are gone, the band costs what the march costs again, and the far
> rung's remaining error is now one number.** Two usage cutoffs killed three
> agent rounds; the work the first round had already produced and its critic
> had already reviewed was on disk, so it was adopted by hand rather than
> re-run.
>
> SHIPPED. **E1 lever 1**: the profile's hand-off weight and tap level read
> the PIXEL footprint alone. The march's own step was inside that maximum,
> and it grows as one over cosine on oblique rays and is uncapped on a
> clear-air stride, so the profile fired on samples whose pixel never handed
> off. **E2**: the sample's shading is evaluated twice inside the band and
> the RADIANCE is lerped by the profile's share of the sample's alpha.
> Mixing optical depths linearly let a weight of 0.007 carry a whole
> in-cloud column into the field's own shading; the in-deck frame read 23
> against 109 with the profile off and 124 at full weight, outside both
> endpoints, which is the signature of a cross-term rather than a bad blend.
> The sun column is taken from the field's own depth and never from a
> blended one. **E1 lever 2**: the element law scales the horizontal element
> by the same factor the vertical clamp applies, so a slant ray counts
> crossings inside the layer rather than across the whole bin. **D4**: no
> tap where the share is zero, and the tap is reused along the ray while the
> sample stays in the same level and half-bin, within a sixteenth of a bin
> vertically and a quarter cell horizontally, with the in-bin slope carried.
> The vertical bound is the critic's: the fraction is read bilinearly
> between bin centres and the element law is linear in it, so a tap frozen
> across a half-bin would have printed a staircase, and worse, would have
> contaminated the very gate that judges lever 2. **D5**, and its own
> critic's blocker on it: the rolling bake is capped at 16 rows a level a
> frame, but the FAST pass gets 32, because at 8 it took 32 to 64 seconds at
> the rig's frame rate against every fixture's 12 to 14 second settle, so
> the global map would never have validated inside a capture, the Low sheet
> would have fallen back to its old path and its gate could not have failed.
> **D2 REVERTED**: the profile share's exemption from the step economy
> bought nothing. The coverage masks carry the same tiny-component census
> with the economy on and off (10170 against 10136), so it removed no
> coverage grain, while the band cost multiplied two to four times.
>
> GATES ON v0.1294.1 (sweep 20260906-054230). PASSED: the in-deck view is
> fixed, both arms now reading the same frame (max 12, mean 0.36, 0.05
> percent of pixels above 8 levels, against mean 93 and 99.999 percent
> before), and its channel-10 twin decodes to exactly 0 in every annulus, so
> the profile contributes nothing in-deck at all. The band's two standing
> views agree to 0.02 levels (rain 26 km was 11 levels brighter, mid-alt had
> 26 percent of its pixels moved). The band's cost is back: the 60 km rung
> reads 127.6 ms with the profile on against 126.3 with it off, within one
> percent, where it was 204; the 400 km rung 78 ms, orbit 21.6. The
> sub-band rungs sit at their own coin-flip floor (60 km on-vs-off mean
> 6.67 against a same-exe knob-0 floor of 6.63).
>
> STILL OPEN, and now stated as numbers. (1) THE RESIDUAL: with lever 2
> judged honestly for the first time (the tracer's fixed-law prediction
> against a capture RENDERED by the fixed law, correlation 0.990), the
> render carries 27 to 29 percent more alpha than the law predicts, and the
> excess is UNIFORM across every off-nadir band. A constant multiplicative
> factor outside the element law is a narrow thing to hunt: the candidates
> are the bake's fraction (the area above the 0.02 density gate, the rind's
> faint skin, unioned over four heights, which a uniform 60 m reference
> march measures at 8.55 percent coverage where the fraction claims 25 to
> 30), the temporal resolve, and the composite. (2) THE LOW TIER: with the
> global map validating for the first time, the Low sheet's profile path
> paints 99.8 percent coverage against High's 34.2. It has never actually
> run before tonight. (3) BELOW THE BAND the marched field's emptiness now
> dominates undisguised: the 200 km rung reads 7.9 percent coverage against
> the 620 km rung's 47.3, which is the vertical comb (a 928 m step through a
> 150 to 400 m layer based at the slab floor), not a profile defect.
> (4) The orbit picture is a pale wash with the window rectangles still
> faintly readable; its near-opaque area fell to 4.7 percent against High's
> 30.8, so it now under-covers in opacity while over-covering in area.
>
> RIG LESSON: a 26-cell sweep leaks weather pins into standing-vantage
> twins. The in-deck cell read 36.5 on both arms tonight against 109.5 when
> it ran in a three-cell battery, because it does not pin cover, type and
> weather of its own and inherited the previous cell's. Relative
> comparisons inside one sweep stay valid; absolute levels across sweeps do
> not. Every standing twin needs its own weather pins.
>
> NEXT, in order:
> 1. **The residual** (the uniform 28 percent): the one number between the
>    far rung and its default. Instrument first, in this order: the bake's
>    fraction against the uniform reference march at one camera, then the
>    resolve, then the composite.
> 2. **The Low sheet's profile path** (99.8 against 34.2), which the D5 fix
>    exposed by making the global valid.
> 3. **The vertical comb** below the band. The top bound is now diagnosed
>    (high confidence, measured): the bound DOES reach the stride and its
>    units are right, but it publishes the gap to a plane one HORIZONTAL
>    cluster radius above the cloud, because the vertical bound subtracts
>    the bounding radius `br`, which is HORIZONTAL. The stride then overrides
>    the comb and marches 1.35 km a step, ABOVE the 928 m vertical ceiling, so the ray
>    strides PAST the 150 to 400 m layer instead of stopping above it. Proof:
>    on the pixels where the bit LOSES a cloud the fix arm used FEWER samples
>    than its own average (7.6 against 8.6 at 250 km), and the whole nadir
>    chord is marched in 8.6 samples against the reference march's 192. Two
>    further defects: the from-above value is published into `g_v2_sdf_m`,
>    which is deliberately never reset when the body path is skipped, so a
>    kilometre-scale gap leaks into samples that evaluated no body; and a
>    from-above stride is not clamped to the vertical ceiling, so a miss
>    degrades to a 1.35 km step instead of to today's comb. The other half
>    of the bit, the in-cloud floor cap, MEASURABLY WORKS and should be split
>    out with its own switch: found clouds render more opaque with it (the
>    top-alpha band at 60 km 0.55 to 1.96 percent against the reference's
>    3.75). And the gate itself was VOID on its own preconditions (the
>    reference reads 8.55 percent coverage against its own 20-point bar), so
>    the fixture must be re-cut onto a cloudier camera with a prod-steps twin
>    before any of this is re-judged. Then the default flip, then deleting
>    the fade constants and the twin.
> 4. The look ("still kinda look like spheres"), the per-cell cluster table
>    (the 36.8 km class), the horizon rung (the 5 m sunset), increment 5,
>    cloud layering, the planet pass arc.

> **v0.1293 (2026-09-06): FAR RUNG 4b, three G0 defects fixed and two more
> found by G1..G7; the default stays off.** Two panels of readers and
> critics on the v0.1292.1 captures, then a two-implementer workflow with
> critics and repairs, merged serially and shipped after the full static
> bar.
>
> FIXED (4b). D1, the window-edge steps: the level walk collapsed onto one
> level for every sample outside the finest CONTAINED window (at 873 km
> everything outside level 2's 255 km window), so each window's edge band
> handed to the GLOBAL map (a different estimator, 15 to 38 percent more
> coverage on the same ground) and printed as a bright strip and a step; the
> critic verified the rectangle geometry on four rims to within 5 px. Fix:
> re-walk from La + 1 so the band hands to the next containing level (the
> contract had specified the collapse; it is amended). D2, the in-window
> grain: the reader's atlas-variance story was REFUTED by a control on disk
> (at a forced level, with identical atlas cells, the tiny-component census
> fell 2566 to 108 with the step economy off): the deep relaxation strode
> across the thin layer's bin in the profile share. Fix: the profile share
> is exempt from the relaxation and the footprint floor (scaled by
> 1 - w_pf_prev). D3, the marched field empties from above: CONFIRMED and
> deeper than diagnosed: the built layer is based at the SLAB BASE, the
> march's final sample lands exactly on that base plane where density is 0,
> and the vertical reject leaves the SDF at its sentinel so the stride is
> inert from above; only one comb sample per ray can find a humilis (hit
> probability h / 928 = 0.11 to 0.22) at EVERY altitude, and the found ones
> are skimmed by the economy floors. The reader's fix was broken four ways
> (a bound that omitted br, a top read polluted by the sun ladder, ungated
> effects, a gate whose reference arm rendered nothing under the 224 cap).
> Fix, corrected: the vertical gap to the ADMITTED region as the SDF lower
> bound and the in-cloud floor at a quarter of the found cloud's height from
> a snapshotted top, behind flags-pad bit 12 (F10 "Built-body top bound",
> ipc `cloud_top_bound`, default off) with a rebuilt gate (a uniform 60 m
> reference at eco 0, full-res masks at 250 and 60 km, a VOID condition:
> prod must sit more than 4 points below ref).
>
> FOUND BY G1..G7 (open, panel in flight): E1 OVER-COVERAGE wherever the
> profile has weight: the descent ladder's coverage climbs from 50 percent
> at 620 km to 97 at 200 km (the hand-off band), forced level 0 at 60 km
> paints 67 to 100 percent against 3 to 44 marched, the adjacent-rung IoU
> sits at 0.4 to 0.9 against 0.97, the profile over-covers High by 10 points
> at orbit (44 vs 34), and the reference-vs-analytic dumps disagree on f by
> 0.12 to 0.17 while agreeing on G (the reference samples two heights per
> bin and under-counts thin clouds, so which f is right is open). In the
> band the frame fills with a pale veil at a weight of 0.15. E2 THE IN-DECK
> BLACKOUT: operator-bm12 (inside the deck) renders mean luminance 23
> against 109 with the profile on, where w is about 0.007: the lighting
> mixes OPTICAL DEPTH linearly, so a huge analytic column times a tiny
> weight dominates the ladder. Rain-26km +11 levels, mid-alt 26 percent of
> pixels moved. Also: D4 the band costs more than either side (the march
> runs the full field AND the taps: 60 km r1 131 -> 204 ms, r4 5.5 -> 16.4;
> in-deck twins +8 to +20 percent; horizon views cheaper) and D5 the bake
> runs 9 to 18 ms per frame at 3 to 4 fps because the time-based cadence
> bakes a fraction per SECOND (per frame it scales with frame time; the
> contract's figures assumed 60 fps). Passed: G6 at orbit (22.0 vs High
> 25.6 ms r1; 1.49 vs 1.96 r4), G0(b/c/e), G0(d) statistically (pixel-exact
> outside the coin-flip regime). Rig lessons: every map_diag channel leaves
> through the sRGB swapchain (ratios must decode; raw bytes read
> L + r(6 - L)); a HARD twin of channel 10 makes the band excess a pure
> same-pixel ratio; the reference bake at 30 m steps overruns the 224 cap.
>
> GATES ON v0.1293.1: 4b gates on v0.1293.1 (sweeps 20260906-0351 to -0357). G0(d): knob 0
> statistically identical to v0.1292.1 (bm12 max 2, the closeup 0.14 percent
> above 8 against its 0.34 floor, the 60 km humilis nadir 11.3 percent with
> identical mean 171.2 and white 7.6: the coin-flip regime again). D1: the
> decoded cardinal instrument on the auto level map reads a max bin step of
> 0.507 level (bar 0.5; before the re-walk the same south line read 1.567):
> the global hand-off is gone, one bin pair sits at the bar. D2: the orbit
> mask census is UNCHANGED (10317 tiny components vs 9996; High 122), so the
> economy's relaxation was not the orbit grain's carrier; the s60/s120
> fixed-step pair proves A7 (mask IoU 0.986, coverage 68.4 vs 69.3) while
> their LIGHTING differs (white 18 vs 34 percent): the shading, not the
> transmittance, is step-dependent. D3: NULL at 250 km (prod 4.25 percent,
> ref 8.55, fix 4.36 inside the cap-valid disc), NEGATIVE at 60 km (prod
> 7.11, ref 8.35, fix 4.62; the fixture is void there by its own rule since
> prod sits within 4 points of ref) and +66 percent cost at 250 km (62 to
> 103 ms); the look-40 pair loses 13.6 percent of the mask in 3440 tiny
> blobs: coin flips re-rolled. A read-only diagnosis of the null is in
> flight. THE REFERENCE MARCH ITSELF is the finding of the night: a uniform
> 60 m march of the built field at 250 km reads 8.55 percent coverage at
> the 0.216 threshold where the profile's f claims 25 to 30 percent and
> High 31: the bake's f (area with density above 0.02, the rind's faint
> skin, unioned over four heights) is several times the OPAQUE area the
> mask counts, which is E1's core and the 4d item. E2 check: bm12 in-deck
> with the profile on reads 68.9 (before 23.3, off 109.5): 4b's D1/D2 moved
> it, 4c's radiance blend is the fix. COST: the D2 exemption's foot_floor
> scaling re-enables the fine interior floors in the band's marched share:
> 200 km 186 to 260 ms, 310 km 117 to 420 ms at knob 1 (knob 0 untouched);
> the foot_floor half of D2 reverts in 4c, the relaxation half stays.
>
> NEXT, in order:
> 1. **4c**: E1 and E2 from the second panel, D4 (reuse the tap along the
>    ray, skip it where w would be 0), D5 (cap the bake rows per frame so the
>    cost is bounded by frame, not by second); then the full G0..G7 again;
>    flip the default when G1..G6 pass; then delete `CLOUD_V2_FADE_LO/HI`.
> 2. **F5, the stride over the noise sheet**, in `cloud_march_core`.
> 3. The look ("still kinda look like spheres"), the per-cell cluster table
>    (the 36.8 km class), the horizon rung (the 5 m sunset), increment 5,
>    cloud layering, the planet pass arc.

> **v0.1292 (2026-09-06): THE FAR RUNG MERGED (perf arc increment 4), knob
> 0 until its gates pass.** Both halves of `docs/design/cloud-far-rung.md`
> v2, built in separate worktrees on the A17 stub by the relaunched
> two-implementer workflow (the first run died on the usage limit with
> nothing cached), each reviewed by a critic and repaired, merged serially
> (Rust fast-forward, WGSL three-way, clean) and shipped after the full
> static bar (both feature checks, naga, 14 constant-sync tests, 12 profile
> unit tests, the sidebar snapshots, four lints).
>
> WHAT LANDED. Rust: `CloudProfileFrame` / `CloudProfileState` (toroidal
> windows scrolled by whole cells, time-based refresh, fills, the global's
> fast and rolling passes, the flags) with unit tests; `CloudProfileCache`
> (6144x3584 RGBA8, 7 mips, the b14 override on the albedo-group builder);
> four pipelines; calibration, bake and mip passes hoisted out of the
> near-mode block; the `light2_color` pad; a 1 Hz `[CloudProfile]` line; the
> atlas dump request; ipc `cloud_profile` (0 | 1 | hard | ref | L0..L5); the
> F10 checkbox and level combo; map_diag 10/11/12; `cloud-profile-compare.js`,
> `cloud-radial-profile.js --step`, the rig's `dump_cloud_profile` key; 102
> self-pinning gate fixtures (317 vantages). WGSL: the real bake (noise part
> at the cell's own mip; the built part as per-cloud ellipses from the
> calibration table, hoisted once per fragment; union and Poisson forms;
> sqrt-encoded columns), the two calibration stages, the bit-exact
> `cv2_place` / `cv2_density_tail` refactors, the march wiring (the element
> law, the w = 1 skip of density and the sun ladder, the lighting mix), the
> Low sheet on the global map. Orchestrator wiring: `MAX_TIMED_PASSES` 48,
> the mip burst timed under `gpu.cloud_profile`, `CLOUD_FR_CALIB_ARCH`
> shared with its sync pair, two contract corrections. New instrument:
> `scripts/cloud-mask-iou.js` (luminance masks, cubic 512, consecutive-pair
> IoU; proven on disk: High r2 vs r4 0.98, Ultra vs High 0.01).
>
> THE CRITICS' LOAD-BEARING WARNING: knob-0 bit-exactness across the two new
> function boundaries in 41-cloud-bodies.wgsl cannot be proven statically
> (compiler fusion), so G0(d) runs first on the exe; if non-zero beyond the
> rig's floor, the 41 refactor reverts to inline and the bake duplicates the
> placement block. Deviations of record: no knob forces the global, so the
> three r4 rungs at L5 have no "one coarser" sibling; G7 pairs are
> `<id>-prof-on/-off` twins of the standing vantages; every new cell sits on
> the Sahara camera (the covladder column is unlit at the default clock).
>
> GATES: G0 on v0.1292.1 (sweeps 20260906-0154 to -0159). PASSED: (b) the
> prove-red prints the HARD ring at 640 to 680 px at 250 km (predicted 667 to
> 732) and at 720 px at 60 km quarter res (predicted 730 to 1039), the auto
> arm ring-free (no annulus above 1.05x the median); (c) forced level 0 moves
> the near deck (mean 42 at bm12, 43 at the closeup); (e) step invariance at
> L2 (eco 0 vs 1 luminance 196.0 vs 196.2); (a) channels 10/11/12 render and
> move under the knob; panics 0; knob-0 COST identical to the shipped exe on
> all three cells (closeup 330.0 vs 329.5 ms, bm12 16.3 vs 16.2, the 60 km
> rung 131.0 vs 130.9), so the 41 refactor is cost-neutral. UNDECIDED: (d)
> knob-0 bit-exactness: bm12 at the floor, the closeup 0.55 percent of pixels
> above 8 levels, the 60 km rung 11.4 percent (mean 6.7); the same-exe floor
> on those two cells decides. THE ORBIT: coverage restored (the mask 28
> percent white against 0.9 on the shipped build and High's 31), coherent
> sheets, gpu.cloud_screen 21.8 ms + gpu.cloud_profile 16.7 at 873 km full
> res, and THREE DEFECTS: (D1) the nested window boundaries print as
> rectangular brightness and coverage steps (the hand-off is not
> brightness-neutral: a coarser level's f is systematically larger); (D2)
> the field inside the windows is grainy per pixel (mask census 9996
> components of 16 px or less against High's 7); (D3) at 250 km inside the
> HARD ring the MARCHED field is nearly empty against the dense profile
> outside it, so the auto band is a coverage ramp from nothing to full: the
> march's 928 m vertical ceiling cannot resolve a 300 m humilis at ANY
> horizontal footprint (the shipped orbit emptiness by the same mechanism),
> so the hand-off criterion (horizontal footprint, lodf -2..0) is the wrong
> variable and a vertical-resolution criterion is the candidate. Default
> stays knob 0; a three-reader diagnosis panel with critics runs on the
> captures and G1..G7 run for the numbers.
>
> NEXT, in order:
> 1. **The far-rung gates** G0 (d, a, c, e, b) then G1..G7 in the contract's
>    order; flip the default to knob 1 when G1..G6 pass; then delete
>    `CLOUD_V2_FADE_LO/HI` and the twin after G4.
> 2. **F5, the stride over the noise sheet** (the default arm's over-credit
>    x1.8 to x2.4 on sub-gate wisps): in `cloud_march_core`, now that the far
>    rung is in.
> 3. The look ("still kinda look like spheres"), the per-cell cluster table
>    (the 36.8 km class, 63 to 105 ms) and the horizon rung (the 5 m sunset
>    stratocumulus horizon, 117 to 195 ms), increment 5, cloud layering, the
>    planet pass arc.

> **v0.1291 (2026-09-06): the operator's report answered in code: the F10
> sidebar, the sun-cache band fix, and three measurements instead of three
> guesses.** The five-finding workflow (16 agents: reader + adversarial
> critic per finding, implementer + reviewer + repair where a fix survived)
> returned two merges and three refutations of the orchestrator's own
> stories.
>
> MERGED. (U1) The Cloud dev (F10) panel is a scrollable LEFT SIDEBAR; F10
> frees the cursor through reconcile_cursor (the one cursor authority) and
> restores it on close; a 22 px tab on the right edge collapses it and a
> painted cross closes it; the tab is neither drawn nor click-sensed while
> the cursor is grabbed (the reviewer's blocker: under a Confined grab the
> hidden cursor pins at x = 0 and a fire click would have re-opened the
> panel); slider readouts are focus-free labels so nothing on the panel can
> swallow WASD; headless snapshots tests/snapshots/cloud_dev.png and
> cloud_dev_collapsed.png. (F1) `CLOUD_LC_FAR_ANALYTIC` 1.0 -> 0.0. The
> mechanism of record: the constant never governed what runs beyond the
> coarse window (the per-pixel ladder always does, `cloud_sun_tau`'s
> !cached path; `light_cache_tau` returns w = 1 there); it only set the
> coarse outer band's blend TARGET, so with 1.0 the band ended on the
> analytic noise-envelope column against the ladder one pixel beyond it,
> and over built lobes (where the column charges the envelope crown the
> ladder leaves within a rung) the band printed BLACK: the operator's 36.8 km
> "square", reproduced on the rig as a ring along the coarse edge (diff cache
> on/off mean 18.9 at nadir, 32 percent of pixels above 8 levels). The
> critic's cheaper option (make the far field the analytic column too) was
> rejected by the captures: the column is far darker than the ladder on
> stratocumulus and congestus fields. Cost: the v0.1288 band saving reverses
> (rain 26 km about 122 -> 152 ms). GATE PASSED on the v0.1291.1 exe (sweep
> 20260906-004852, Sahara 36.8 km, stratocumulus, cloud_res 2): the ring is
> gone at nadir and at 45 degrees (Sun-source channel: fine white, coarse
> grey, a smooth hand-off, no dark band); mean brightness with the cache on
> vs off went from 10.6 levels darker (v0.1289.1: 186.0 vs 196.6) to 1.2
> (195.4 vs 196.6) at nadir and 0.7 at 45 degrees; the per-pixel diff that
> remains (nadir mean 10.4) is lobe-scale (the 760 m coarse cells' trilinear
> tau against the per-pixel ladder) and cancels in the mean; 45 degrees
> mean 0.9. Costs for the record (res 2, rig defaults): rain-26km-nadir
> 41.7 ms, operator-bm12 35.5, the square nadir 74.6 on / 69.2 off, 45
> degrees 105.6 / 92.2 (the cache costs MORE than the ladder at 36.8 km over
> stratocumulus: the 36.8 km class needs the cluster table, not the cache).
> The square fixture becomes a standing vantage after the far-rung merge
> (vantages.json is the far-rung Rust implementer's file until then).
>
> REFUTED AND MEASURED. (F2) The horizon darkening under the step economy:
> the reader blamed the footprint floor (29 to 231 m at 20 to 160 km from
> 5 m) landing the first in-cloud sample past the rind; the critic voided
> its evidence (the only on-disk horizon pair came from a sweep where the
> eco knob was inert), refuted the ambient-ramp chain, and found the direct
> term's sign plausibly opposite at sunset. First rig pair (humilis field,
> 5 m sunset horizon) had NO cloud in frame; the second pair (stratocumulus
> and congestus fields filling the horizon band) reads eco 0 vs 1 mean 2.4,
> luminance bands equal, visually identical: NOT REPRODUCED (the operator's
> pair was nine sunset minutes apart). The cost it exposed: that 5 m sunset
> horizon over stratocumulus costs 195 ms (eco 0) / 117 ms (eco 1) in the
> cloud pass at half res, grazing rays with footprints under 1 km that the
> far rung does not touch: the 'horizon rung', a perf class of its own. (F3) "Distant clouds vanish with the warp
> band-limit": the reader's 224-iteration-cap chain was refuted (bit 7 and
> bit 8 feed one OR, so bit 8 alone cannot move the refine); the critic's
> mechanism of record is `cloud_noise.rs:995` capping the mip
> variance-restoring gain at 2.0 while a 2x2x2 box drops sigma about 2.83
> per level, so the band-limited warp amplitude collapses geometrically with
> mip; the full-amplitude warp's halo makes grazing rays saturate early, so
> "more clouds" with it off is partly an artifact. First rig pair (cumulus
> field from 4.5 km) showed no difference between arms; the second pair over
> a dense stratocumulus deck (cover 0.85) neither: coverage masks identical,
> no 224-cap hits in the steps channel, the OFF arm only adds fine hash on
> the deck top. NOT REPRODUCED; the mip-gain collapse stays a property to
> fix inside the far rung's near band if a same-clock repro appears. (F4) In-cloud light: the march's alpha
> is invariant under the switch (only the resolve filter couples it,
> bounded); the row is relabelled "changes the LIGHT only". (F5) The
> sample-anchored march: the OFF arm's mechanism confirmed by a reproduced
> twin (the old march measured the SDF stride at the sample and spent it
> from the state, missing a 400 m body on 26 percent of jitter phases: the
> static); the ON arm is Beer-Lambert within 1 percent on uniform slabs, BUT
> the critic found a real defect on the DEFAULT arm: with the SDF stride
> live, sub-gate wisps between bodies (the noise sheet unioned back in at
> deck coverage, which the SDF cannot see) are over-credited x1.8 to x2.4
> or missed outright by the right-endpoint anchoring. Deferred behind the
> far rung (same function); the row is relabelled so OFF reads as the
> known-wrong twin.
>
> THE FAR RUNG: the stub's runtime twin proof passed (v0.1290.1 against
> v0.1289.1 on the same cells sits at the same-exe floor once the first boot
> of a session is excluded; that first boot differs from every later run by
> 0.3 percent of orbit pixels on ANY exe, a rig fact). The two-implementer
> workflow was lost to the usage limit with nothing cached and relaunched
> (wf_dda2f985). Baseline paragraph written into the contract's Gates.
>
> NEXT, in order:
> 1. **F1 gate** on v0.1291.1 (the square fixture on/off, the Sun-source
>    ring, rain-26km cost for the record); then the F2/F3 second pairs
>    decide whether either needs a fix now or a measured "no defect".
> 2. **Increment 4, the far rung**: merge the two branches serially onto
>    main (the Rust side's cloud_dev.rs hunks re-applied over the sidebar
>    rewrite), verify, then gates G0 to G7 in the contract's order.
> 3. **F5, the stride over the noise sheet** (the default arm's over-credit):
>    after the far rung, in cloud_march_core.
> 4. The look ("still kinda look like spheres"), the per-cell cluster table
>    (the 36.8 km class: 63 to 105 ms with the cache not helping), increment
>    5, cloud layering, the planet pass arc.

> **v0.1290 (2026-09-05): the far-rung contract v2, the operator's test
> report on v0.1289, and the integration stub.** Two things landed and one
> arc pivoted on the operator's eye.
>
> THE FAR RUNG (perf increment 4) went through a 5-agent design panel and its
> adversarial critique returned NEEDS_REVISION with 8 blockers, all real: the
> bake cost was understated about 100x (per-texel SDF point tests), the
> transmittance law was O(seg^2) (halving the step halved the extinction),
> the group-3 wiring premise was wrong, three gates read an alpha channel the
> diag never writes (map_diag 1 is opaque greyscale), the speckle target sat
> below the metric's floor, the sampler's mip filter is Nearest, the point
> estimator was a coin flip at fine levels, and the lattice had no polar
> behaviour. The orchestrator's v2 decisions (A1..A19) became
> `docs/design/cloud-far-rung.md`: an analytic built-part bake from a
> runtime CALIBRATION table (per archetype and height: equivalent-circle
> radius ratio and mean density, point-tested once on the shipped SDF),
> TOROIDAL clipmap windows on an absolute equal-angle equirect lattice (no
> anchor, no re-anchor spike, graceful at the poles), TIME-based cadence
> gated on the MAX, a step-invariant element law (`tau_elem = sigma D L_elem`,
> exponent `seg / L_elem`) with a prove-red on eco 0 vs 1, the column stored
> (nine slices per level, atlas 6144x3584, 117 MB allocated), luminance
> gates calibrated red first, G5 at or below High's own census, and an
> INTEGRATION STUB (constants + synthetic bake + real mip pass + the lattice
> tap + channels 10/11/12) committed to main before either implementer
> starts. `scripts/cloud-speckle-census.js` reproduces every baseline (High
> 252, off 263, eco0 2464 on the colour render). The contract's own cost
> derivation: a level-5 full bake 9 to 37 ms, steady 0.03 to 0.3 ms per
> level per frame, the global 140 ms per pass rolled over 60 s.
>
> THE OPERATOR'S REPORT (ten screenshots, live toggling at half res, Ultra):
> (1) a DARK ROTATED SQUARE of shadowed clouds at 36.8 km over a broken
> cumulus field, gone with the sun cache off: the cache WINDOW (48.6 / 97 km,
> planet-fixed); inside it the baked ladder gives real in-cloud sun columns,
> outside it the v0.1288 analytic column under-shadows a cumulus field
> (Jensen: a mean-density slant is far thinner than an in-cloud one), so the
> far field is too bright and the window edge is a seam; the v0.1288 "look
> identical" was measured only at rain-26km-nadir, a stratiform case.
> SHIPPED DEFAULT, top fix. (2) step economy darkens and flattens the
> horizon clouds (the deep relaxation starts at trans 0.5; never measured on
> a horizon look at half res): violates the increment's own rule. (3) warp
> band-limiting (bit 8, default since v0.1272) makes distant clouds vanish:
> coverage decays with distance under a band-limited thresholded field, the
> same class the far rung is built around, in the near band. (4) in-cloud
> light (the Eddington experiment) makes clouds transparent: a light model
> touching alpha. (5) sample-anchored march on = opaque wall, off =
> see-through static: the old march under-counted extinction (v0.1271), the
> wall is most likely right; check for double counting. (6) field walls
> null: confirmed. Plus: the F10 panel is too long to see; make it a
> scrollable LEFT SIDEBAR, F10 frees the cursor, a collapse tab on its right
> edge. Dispatched as workflow wf_de0676a5: per finding diagnose ->
> adversarial verify -> implement in a worktree if confirmed and bounded ->
> review -> repair; the sidebar in parallel. Every shader fix keeps its
> existing switch as the A/B twin.
>
> NEXT, in order:
> 1. **Merge and gate the operator-report fixes** (serially: F1 sun-cache
>    far fallback, F2 economy relaxation, F3 warp-band coverage, F4/F5 as the
>    diagnoses decide, U1 the sidebar) with `--operator-config` rig sweeps
>    once the operator's instance exits: the 36.8 km cumulus square fixture,
>    a 5 m sunset horizon eco 0/1 pair, the 2.2 km above-deck bit-8 pair.
> 2. **Increment 4, the far rung**: the two-implementer workflow from
>    `docs/design/cloud-far-rung.md` on the stub, then gates G0 to G7 in the
>    contract's order (G0 prove-red first; the F2 baseline with bits 16/17
>    off/on and High's map_diag 1 census are preconditions).
> 3. **The look** ("still kinda look like spheres"): the design-A geometry
>    increment; its experiment `hum-top-30m` first.
> 4. The per-cell cluster table, increment 5 (operator-gated), cloud
>    layering, the planet pass arc (`gpu.celestial` 80 to 107 ms at the
>    operator's settings: the frame's biggest lever now).

> **v0.1289 (2026-09-05): the operator's testing loop, made to point at the
> same switch and the same settings.** After v0.1288 the operator reported
> "clouds are looking better from all angles" and "still a bit rough and
> kinda look like spheres", and asked for a marker on the exact F10 switch
> under test and for their settings to be the real Ultra. Shipped: (1)
> `data/gui/dev_tests.json` names the F10 rows under test; the Cloud dev
> panel lists them in red at the top and paints a red TEST tag beside each
> named row, re-read live (the chat names the same label in bold); (2)
> Settings > Graphics > Presets with "Ultra (reference)" (what the rig
> measures: cloud march at half res, constructed bodies, every feature at
> physical strength) and "Extreme" (every slider at its limit) from
> `data/gui/graphics_presets.json`; the rig's `--operator-config` flag is the
> other half of the sync; (3) `just snapshot cloud_dev` renders the F10 panel
> headlessly. Their screenshot had **Field walls** on (the v0.1281 null
> instrument, about 10%) and WALK mode at 0.3 km among cumulus, the
> closeup-class case where the caches break even.
>
> MEASURED AT THE OPERATOR'S LIVE SETTINGS (rig --operator-config, v0.1288, half res, real GPU timestamps; ms): bm-12 inside: frame 191, cloud march 25, planet pass (gpu.celestial: terrain + ocean + atmosphere) 99; bm-12 4.6 km: frame 189, cloud 17, planet 107; cumulus closeup: frame 140, cloud 44, planet 25; 0.4 km among cumulus looking across (the operator's screenshot case): frame 175, cloud 55, planet 79. At their settings (ssao 0.96, godrays 0.98, planet_lod_px 4, terrain_split_px 2, terrain_patch_budget 12288, sun shadows, FFT ocean) the planet pass is 80 to 107 ms against 30 to 37 at the rig defaults, and the clouds are 13 to 30 percent of the frame. The sub-10 fps is now the planet pass, not the clouds; the planet arc is the bigger lever from here, and every frame-level claim must be measured with --operator-config (the rig defaults understate the planet pass 3x).
>
> NEXT, in order:
> 1. **Increment 4, the far rung** (design panel wf_92c5b89f in flight):
>    the orbit speckles, a per-cell prefiltered profile chosen by footprint,
>    the Low sheet redrawn from it.
> 2. **The look**: "still kinda look like spheres" is the design-A geometry
>    increment (dome cores with wide contact discs, `dome_sink` per genus,
>    caps budding on the dome, blue-noise placement, 3D erosion on the built
>    path); its discriminating experiment `hum-top-30m` first.
> 3. **The per-cell cluster table** (the closeup's break-even), then
>    increment 5 (operator-gated quarter march), then cloud layering, then
>    the planet pass arc (`gpu.celestial` 30 to 37 ms everywhere).

> **v0.1288 (2026-09-05): PERF INCREMENT 2, step economy, and THE DEFAULTS FLIP:
> the sun-shadow cache, the body cluster cache and the step economy are ON by
> default (F10 toggles kept for A/B); the sun cache's coarse BAND target was the analytic column (reverted to the ladder in v0.1291, finding F1; the ladder always ran beyond the window).** The interior step floors are now
> footprint-aware (a ray 300 km out no longer steps 22 m) and opaque rays
> relax their step; strength in `light7_color.y` (`cloud_step_eco`).
>
> MEASURED (half res, Ultra, clock frozen, both caches on, real GPU
> timestamps):
>
> ```
> gpu.cloud_screen ms, economy off -> on (both caches on, clock frozen): bm-12 inside 38.5 -> 26.0 (32% off), bm-12 4.6 km 18.7 -> 17.1, rain 26 km 22.7 -> 19.9 (12%), orbit 873 km 159 -> 30.6 (5.2x). March-steps channel mean: inside 72.5 -> 57.1 (21% fewer steps), orbit 23.1 -> 19.1.
> Look: pixel diff mean inside 0.28, 4.6 km 0.35, rain 0.69 (radial profile identical, no contour bands, grain 1.40 -> 1.41); orbit mean 4.2 with the white fraction 9.1% -> 5.0%: the orbit Ultra view is speckles (sub-pixel constructed bodies sampled as snow) with or without the economy, the far-rung problem of increment 4; the economy makes that picture 5x cheaper and slightly sparser.
> Two nulls before this: the knob write sat before the wholesale uniform upload (dead wire), then the SDF stride's quarter-rind body cap overrode the floors; the steps channel was the gate that exposed both.
> default arm: tmp-D8-above                   201.9              16.64              5.04   timestamps
> default arm: tmp-D8-cu                      235.1              48.72              3.79   timestamps
> default arm: tmp-D8-in                      272.8              26.12              4.78   timestamps
> default arm:
> ```
>
> SUN-CACHE PARITY (why interiors read +10 to +12 brighter with the cache):
> ```
> Clock frozen, in one boot: inside the stratocumulus top the direct-sun channel reads 236.6 (ladder) vs 241.3 (cache), the render 216.9 vs 219.2 (+2.3 levels; the world-shape LOD pin alone accounts for +0.7); at bm-12 4.6 km the direct-sun channel 176.2 vs 175.7 and the render 179.9 vs 179.9 (identical). The +10 to +12 of the v0.1286 report was the rig's rotation drift between captures, not the cache. Residual +2 inside a deck = the trilinear averaging of tau over 190 m cells and the absent cone jitter; accepted.
> ```
>
> SUN-CACHE FAR FALLBACK (the closeup, where the cache lost 76 to 131 ms):
> ```
> Both caches on, clock frozen, ladder beyond the coarse window vs the analytic column: closeup 108.7 vs 110.7 ms (same; its rays stay inside the windows), rain 26 km 151.5 vs 122.1 (19% cheaper), horizon look at 3 km 91.2 vs 82.7 (9% cheaper); look identical at the closeup and the horizon (diff mean 0.2 to 0.3) and within noise at rain (2.4, bands 209.6 vs 207.8). CLOUD_LC_FAR_ANALYTIC is now 1.0 (the analytic column beyond the windows). [ANNOTATED v0.1291, finding F1: this constant never governed what runs beyond the coarse window (the ladder does, cloud_sun_tau's !cached path); it only set the coarse outer band's blend target. The band-target swap WAS measured here with the band in frame (the 29 ms swing at rain-26km-nadir proves it) and moved that stratiform picture by about 1.8 levels; its effect over BUILT cumulus lobes, where g_sun_tau_col is the envelope crown and the ladder taps the built body, was unmeasured, and there the band printed as a BLACK ring at the coarse edge (the operator's 36.8 km square, reproduced: diff cache on/off mean 18.9 at nadir). Reverted to 0.0; the 122 ms rain and 82.7 ms horizon numbers above no longer apply (expect about 152 and 91 again).] The closeup remains the weak spot of both caches (76 ms with neither, 109 with both): its rays look up and across through thin cloud where the ladder exits the slab in a rung or two while the cache read is a fixed cost; a follow-up.
> ```
>
> NEXT, in order:
> 1. **Operator judges the defaults in flight** (F10 toggles off to compare).
>    Expected: about 3x inside a deck, 5x at orbit, the closeup roughly flat.
> 2. **The per-cell cluster table** (increment 3b): the body cache loses on
>    long oblique rays (closeup 76 to 98 ms with both caches) because the
>    per-invocation slot arrays spill; a per-cell table in a texture stores
>    the cluster once for everyone.
> 3. **Increment 4, the far rung**: from 873 km the Ultra clouds are SPECKLES
>    (sub-pixel constructed bodies sampled as snow, `tmp-L2-orbit-*` in sweep
>    20260905-210918), with or without the economy; a per-cell prefiltered
>    density profile chosen by footprint replaces them with cloud masses.
> 4. **Increment 5, operator-gated**: the interleaved quarter march.
> 5. Cloud layering (design); the planet pass arc (`gpu.celestial` 30 to 37
>    ms everywhere).

> **v0.1287 (2026-09-05): PERF INCREMENT 3, the per-ray body cluster cache
> (F10 "Body cluster cache", dev pad bit 17).** At Ultra every density sample
> rebuilt the lobe cluster of up to nine cells from hashes; the march now
> keeps nine per-ray slots (cell weather alpha + built lobes) and evaluates
> the union on cached lobes, keyed by the cell and by the cluster width with
> a 2% tolerance; the sun ladder reads a slot the eye built but never builds
> one (within the rig repeat floor, not bit-exact). A first cut
> that sized clusters by the CELL-CENTRE weather to make the cache exact
> changed coverage drastically on its own (bm-12 inside dark 93% to 21%) and
> was dropped: a perf increment must not move the field.
>
> MEASURED (half res, Ultra, clock and world-shape LOD pinned, one boot, real
> GPU timestamps; sc = sun-shadow cache, bc = body cluster cache):
>
> ```
> gpu.cloud_screen ms (sc = sun cache, bc = body cache): bm-12 inside sc0 106 -> 67 (bc on), sc1 60 -> 37; both caches 106 -> 37 (2.9x). bm-12 4.6 km sc0 66 -> 37, sc1 30 -> 18; both 66 -> 18 (3.7x). Cumulus closeup (2.6 km, look 62, long oblique rays) sc0 76 -> 125 (WORSE), sc1 131 -> 98; both 76 -> 98: the per-invocation slot arrays spill to local memory and a ray that crosses many cells amortises few builds, so the closeup is the case for the per-cell cluster TABLE (next body increment); the sun cache alone also loses there (76 -> 131) because rays beyond the 48/97 km windows run the full ladder plus the window blend, the far-fallback question of the v0.1286 report.
> On/off pixel diff (same sun-cache state, clock frozen): inside 0.20 mean (max 5), 4.6 km 0.016 (max 4), closeup 0.48 (max 53, 0.8% of pixels over 8): within the rig repeat floor.
> Rig clock freeze: with time_scale 0 after each park the closeup pairs stopped rotating and became exact; the earlier "bit-exact" failures (mean 13 to 37) were rotation.
> ```
>
> RIG: the game clock is now frozen between park and capture (showcase
> `time_scale` 0, sent by `probe-sweep.js` after every park). The 20-minute
> day had been turning the planet under the parked camera, which is what
> rotated captures about the nadir and (with the settle timing) shifted where
> a "vantage" actually was; the closeup pairs are exact with it. Absolute
> numbers from sweeps before this fix are comparable only within their own
> sweep. The heading-pin item is closed by this.
>
> NEXT, in order:
> 1. **Sun-shadow cache parity** (`scratchpad abK.sh`, next sweep): the
>    direct-sun channel off/on and the world-shape LOD pin off/on on the
>    ladder arm decide whether the +10 to +12 interior brightening is the
>    bake's LOD pin, the missing cone jitter, or the trilinear averaging;
>    then the cache (and this body cache) become the defaults if the look
>    holds for the operator.
> 2. **Increment 2, step economy**: interior step floors at half the sample
>    footprint, the transmittance-budget exit, the BUILT-path saturating
>    interior remap.
> 3. **Increment 4, the far rung** (the 363 ms orbit row).
> 4. **Increment 5, operator-gated**: the interleaved quarter march.
> 5. Cloud layering (design), then the planet pass arc (`gpu.celestial` 30 to
>    37 ms everywhere).

> **v0.1286 (2026-09-05): PERF INCREMENT 1, THE SUN-SHADOW CACHE, built and
> measured (F10 "Sun shadow cache", default off).** Sun optical depth is now a
> planet-fixed cached quantity: rungs 2 to 11 of the per-sample sun ladder
> are baked into two nested 3D windows around the camera ground point (fine
> 48.6 km at 190 x 240 m, coarse 97 km at 760 x 480 m) as an R16F slice atlas
> riding group 3 binding 0 (no bind-group-layout change), one trilinear tap
> per sample, rungs 0 and 1 kept per pixel, one eighth of each window baked
> per frame, a full bake on re-anchor. Built from the written contract
> `docs/design/cloud-sun-shadow-cache.md` (as-built deviations appended there)
> by two worktree agents on disjoint files, each critic-reviewed and repaired,
> merged as patches, both feature checks, naga and the cloud_light tests
> green.
>
> MEASURED (half res, Ultra, clock and world-shape LOD pinned, cache off vs
> on in one boot, real GPU timestamps):
>
> ```
> gpu.cloud_screen ms, cache off -> on (bake gpu.cloud_light in brackets): bm-12 4.6 km 58.9 -> 16.1 (2.5), 3.7x; bm-12 inside 215.9 -> 152.0 (2.6), 30% (gate asked 40%); cumulus closeup 156.6 -> 137.2 (1.6), 12%; rain 26 km 51.8 -> 46.7 (1.1); sc deck top 27.8 -> 22.5 (0.9); sc inside top 13.0 -> 12.6 (0.9).
> Look: luminance bands and grain unchanged at rain, closeup, deck top and inside bm-12 (grain 0.552 -> 0.550, 1.331 -> 1.409, 2.989 -> 2.998); interiors read BRIGHTER with the cache by 10 to 12 levels (sc inside top 221 -> 231, bm-12 4.6 km 41.6 -> 53.8), the deck top +3.5; direct-sun channel grain 1.775 -> 1.738 (not up); above-deck radial profile a uniform +10 offset with NO ring at a window edge; the Sun-source channel reads fine-window (white) nearly everywhere at both bm-12 cameras.
> Open: the cached optical depth runs systematically lower than the per-pixel ladder (the bake evaluates density at the world shape LOD with no cone jitter, and trilinear filtering averages tau over 190 m cells); a slice-dump parity test against the CPU twin is the next check before the cache can become the default. With world-shape LOD pinned (bit 3, required so both arms shade the same mips) the bm-12 4.6 km camera sits inside cloud, so that pair is an in-cloud pair, not an above-deck one.
> ```
>
> NEXT, in order:
> 1. **Operator judges the look** of the cache in flight (F10 "Sun shadow
>    cache"; the "Sun source" bisect channel shows where each sample got its
>    sun: white = fine window, grey = coarse, dark = decided locally or
>    outside). If it holds, it becomes the default in the next release; the
>    open question of the far fallback (the ladder beyond the coarse window
>    versus the analytic column, `CLOUD_LC_FAR_ANALYTIC`) is measured on one
>    horizon look first.
> 2. **Increment 2, step economy**: interior step floors at half the sample
>    footprint (continuous with distance), the transmittance budget exit, the
>    BUILT-path saturating interior remap. Gates: March-steps histogram down
>    30% in-deck, horizon band off the cap, masked mean within 3%.
> 3. **Increment 3, the body cost**: a per-ray current-cell lobe cache in
>    `cloud_v2_body`, then the per-cell cluster table storing the SDF. Gate:
>    bit-exact diff; `gpu.cloud_screen` down 25% at bm-12 and the closeup.
> 4. **Increment 4, the far rung inside the march** (the 363 ms orbit row):
>    a per-cell prefiltered density-by-height profile chosen by footprint,
>    planet-fixed, transmittance-blended; the Low sheet redrawn from it.
> 5. **Increment 5, operator-gated**: the interleaved quarter march.
> 6. **Cloud layering** (operator question, 2026-09-05): the design picks ONE
>    family per ray, so a cumulus field under a cirrus veil cannot exist yet.
>    Layering as separate height bands the march skips between is the design
>    to write after the perf arc; estimated 1.3 to 1.6x a single layer, not 2
>    to 3x, and pointless before the per-sample cost is fixed.
> 7. **The planet pass** (`gpu.celestial` 30 to 37 ms in every situation) is
>    the next arc: with clouds free the frame would still be about 27 fps at
>    2560 wide.

> **v0.1285 (2026-09-05): PERF DAY 0 DONE; the passes are measured.** The
> rig now copies `debug/frame_costs.json` beside every vantage capture when
> the sweep env carries `HUMANITY_FRAME_COSTS=1`, and
> `scripts/cloud-costs-table.js` tabulates `gpu.cloud_screen`,
> `gpu.celestial` and `cpu.patch_build` per vantage. Every gate below reads
> those, never the manifest fps. Day-0 table (2560x1387, clouds half, clock
> pinned):
>
> ```
> id                          frame_ms   gpu.cloud_screen gpu.cloud_resolve     gpu.celestial         gpu.scene   cpu.patch_build   timing
> tmp-P-bm12-high                 45.1               4.13              0.16             36.45              0.23              5.03   timestamps
> tmp-P-bm12-off                  39.2               0.00              0.00             35.50              0.23              5.26   timestamps
> tmp-P-bm12-ultra               329.6             211.30              0.30             30.63              0.24              7.74   timestamps
> tmp-P-cu-high                   56.5               2.14              0.13             33.41              0.24             11.09   timestamps
> tmp-P-cu-off                    51.3               0.00              0.00             31.91              0.23              9.05   timestamps
> tmp-P-cu-ultra                 178.2             145.91              0.29             29.19              0.24             13.94   timestamps
> tmp-P-na40-high                 54.9               1.82              0.16             38.95              0.25              9.03   timestamps
> tmp-P-na40-off                  40.7               0.00              0.00             38.51              0.23              8.97   timestamps
> tmp-P-na40-ultra               336.1             217.68              0.30             39.16              0.24              8.64   timestamps
> tmp-P-orbit-high                76.1              13.38              0.19             35.87              0.24             10.70   timestamps
> tmp-P-orbit-off                 74.5               0.00              0.00             34.41              0.24             10.67   timestamps
> tmp-P-orbit-ultra              387.7             363.43              0.30             19.62              0.25             10.74   timestamps
> tmp-P-rain-high                 47.9               9.81              0.29             35.87              0.24              9.43   timestamps
> tmp-P-rain-off                  46.1               0.00              0.00             36.15              0.24             10.73   timestamps
> tmp-P-rain-ultra                76.7              46.11              0.33             26.74              0.23              9.92   timestamps
> tmp-P-sctop-high                67.0               9.09              0.31             36.87              0.25              5.38   timestamps
> tmp-P-sctop-off                 53.4               0.00              0.00             35.40              0.23              6.05   timestamps
> tmp-P-sctop-ultra               85.4              24.10              0.29             35.82              0.24              5.19   timestamps
> ```
>
> Read it as: at Ultra the cloud march (`gpu.cloud_screen`) is 211 ms inside a
> deck, 146 at the closeup, 46 over the 26 km overcast, 24 at the deck top,
> while High is 2 to 10 ms in the same places: the constructed bodies times the
> sun ladder ARE the frame, as the panel said. `gpu.celestial` (the planet:
> terrain, ocean, atmosphere) is 30 to 37 ms in every situation, the floor the
> cloud arc cannot pass and the number the planet arc will open with.
>
> Also shipped: the bit-exact dead-tap trim (`cloud_density_hi` skips the
> fray and detail taps at `cs.v2 == 1.0`, and the puff tap on the sun-profile
> path; `cloud_density_light` deleted; before/after pixel diff in-deck mean
> 1.31, above-deck 0.41, under the rig repeat floor of about 4;
> `scripts/cloud-diff.js` is the gate); the cloud resolution persisted
> (`cloud_res_div` in config, default quarter, the F10 choice survives a
> restart, so play and rig agree); the removed carve-saturation knob's
> leftover Rust plumbing gone (bits 16-17 genuinely free). CAUTION learned:
> the closeup before/after pair came out rotated (look 62, unpinned heading),
> so bit-exact gates at oblique looks are blocked until the heading pin
> lands (NEXT item 4 of the v0.1284 block); and a patch script that only
> throws on a missing anchor can print success while a hunk silently fails
> to apply; every removal now ends with a post-condition grep.
>
> NEXT: increment 1 of the perf plan, the sun-shadow cache, gated on the
> table above (`gpu.cloud_screen` down 40% or more in-deck, forced A/B masked
> mean within 3%, no halo at opaque/clear voxel boundaries, no ring at window
> edges, prove red first with a hard window switch). Design and the rest of
> the ladder: the v0.1284 block below.

> **v0.1284 (2026-09-05): THE PERFORMANCE PLAN (panel wf_de1c02dc) and the
> rosette panel's verdict.** The frame is per-STEP cost, not per-pixel and not
> the far field: every cloud step pays one eye density plus up to 12 sun-ladder
> densities, and at Ultra each of those rebuilds the constructed cluster (3x3
> cells, up to 20 lobes), so the ladder is 80-88% of all density work in every
> situation. Same-boot tier ladder at bm-12: Ultra 180-226 ms, High 47, Low 41
> (the 41 ms floor is terrain, ocean, SSAO, shadows, god rays). Half res costs
> 4x quarter, and the cloud resolution is NOT persisted (`cloud_dev_res_div`
> resets to quarter every boot; the operator plays at half by hand; the rig
> defaults to quarter). From orbit, yes, 96% of pixels march cloud 300+ km
> away with 22-58 m interior step floors, but that is about a third of an
> orbit frame, not the in-deck 5x. The v0.1244 near/far split died of six seam
> classes intrinsic to a second camera-anchored history renderer stitched by
> pixel ownership; a correct far field is the same density at a coarser
> footprint chosen per sample like a mip, planet-fixed, no history, blended in
> transmittance. Predicted (2560x1387, Ultra, half): in-deck 180-226 ms to
> 95-110 after increment 1 and 55-70 after the plan; the operator's 3.7 km
> in-cloud 143 to 75-85 to 50-60; 26 km overcast 57-73 to 40-45 to 35-40;
> orbit 42 to 30-35 to 25-30 (then terrain and CPU bound). Every share is a
> manifest subtraction until Day 0 measures pass times: treat as 2x slop.
>
> ALSO SHIPPED: the noise-path wind shear no longer grows with the session
> clock (base and top of a column drifted apart at (wind_hi - wind_lo) * t,
> an 85 degree tilt after an hour that the 120 s rig pin never saw; the
> column now drifts at the band-mean wind, shear stays with the bounded
> lean); the Low sheet draws `cloud_weather` with 97% cores (reads as a
> satellite view from orbit); `clouds.rs` `cloud_noise` mirrors the GPU
> lattice (KAT-guarded; the stale triplanar mirror hid BUG-074 from every CPU
> twin); `cloud-radial-coherence.js` prints edge counts and "empty" (the
> post-fix bm-12 0.00 was vacuous; the non-vacuous proof is rain-26km-nadir
> +0.58/+0.43/+0.27 to -0.04/-0.05/+0.03).
>
> NEXT, in order (the performance arc, operator priority):
> 0. **Day 0.** (a) A rig sweep with `HUMANITY_FRAME_COSTS=1` so
>    `debug/frame_costs.json` gives `gpu.cloud_screen` per vantage at half res
>    on High and Ultra: operator-bm12, sc-top-3p0km, rain-26km-nadir,
>    cumulus-closeup-ultra, nadir-anchor-40, an 873 km park, each with a
>    clouds-off twin; every later gate is written against those numbers.
>    (b) The bit-exact dead-tap trim: skip the fray and detail taps on the eye
>    path when `cs.v2 >= 0.999` (they feed a term multiplied by 1 - v2), skip
>    fray/detail/puff/cell on sun-profile evaluations (`cloud_sun_tau` reads
>    only `.x`), delete the uncalled `cloud_density_light`. Gate: the diff
>    channel renders black. (c) Persist the cloud resolution as a settings
>    field so play and rig agree.
> 1. **The sun-shadow cache.** Sun optical depth becomes a planet-fixed cached
>    quantity in nested 3D windows around the camera, baked by the same rung
>    ladder on the profile density (a bake pass replacing the dead
>    `fs_cloud_octa` in 45-cloud-temporal.wgsl), read with one tap per
>    sample; each pixel keeps rungs 0 and 1 on-axis (30 m, 57 m); the analytic
>    column beyond the windows; dev pad bit 16; the family lookup stays local.
>    Gates: forced A/B masked mean within 3% at the standing vantages,
>    `gpu.cloud_screen` down 40% or more in-deck, no halo at opaque/clear
>    voxel boundaries, no ring at window edges (prove red first by forcing a
>    hard window switch).
> 2. **Step economy** in `cloud_march_core`: interior clamps floored at half
>    the sample footprint (145 m at 100 km, 1.2 km at 873 km, continuous),
>    transmittance-budget relaxation and exit under 1/512, plus the BUILT-path
>    saturating interior remap (the noise-path one was null). Gates: March
>    steps down 30% in-deck, horizon band off the 224 cap, masked mean within
>    3%, no contour bands.
> 3. **Body cost per eye step**: a per-ray current-cell lobe cache in
>    `cloud_v2_body`; if not enough, a per-cell cluster table storing the SDF
>    with both rinds derived from it. Gate: bit-exact diff, 25% off at bm-12.
> 4. **The far rung inside the march**: per-cell prefiltered density-by-height
>    profile chosen by the same footprint `lodb` as every mip, planet-fixed,
>    no history, transmittance-blended; a 2D profile map for orbit and the
>    Low sheet; `g_march_max_km` at most a level selector. Gates: forced-level
>    A/B within 2%, a 60 km to 3 km descent ladder monotone with silhouette
>    IoU over 0.97, no ring in the diff radial profile, orbit
>    `gpu.cloud_screen` down 50%.
> 5. **Operator-gated, default off**: a deterministic 4-phase interleaved
>    quarter march reconstructed to half (about 85 ms at half res); the
>    operator's own parked on/off test decides it.
>
> PARKED BEHIND THE PERF ARC (rosette panel wf_47184b56): the field plan,
> design A geometry half (dome cores with wide contact discs, `dome_sink` per
> genus, caps budding on the dome, blue-noise placement, 3D erosion; increment
> C folded into its interior profile), preceded by its discriminating
> experiment `hum-top-30m` (lat 24 lon 14, 1.35 km, type 0.33 cover 0.35, ms 1,
> map_diag 0 and 1): coherence with real edges within 0.15 of the controls
> means the extruded walls are look work; over +0.5 means a second carrier and
> the geometry increment becomes correctness work run beside the perf arc.
> Also open: a deterministic straight cut through the Low sheet from orbit
> (rig 20260905-011952, absent at High, not temporal, cause unknown); the
> first-capture-after-boot render state (rig discards it; cause open); the
> rig heading pin; the in-cloud light default flip at `sc-inside-top`; the
> visual-sweep re-judge of every unpinned down-look spec.

> **v0.1283 (2026-09-05): rosette CONFIRMED GONE by the operator; PERFORMANCE
> is the next priority.** Operator, on v0.1282.1: "I finally didn't see the
> rosette! Well done!" and then: "even at half resolution I'm crawling FPS
> wise. It seems like I'm seeing the volumetric clouds from a very far
> distance instead of the 2D cloud layer while far away." The code agrees:
> since v0.1250 (ONE RENDERER) the screen march is the only cloud renderer
> at every altitude, the octa map is retired, and far content is cheaper
> only by the footprint stride. Rig frame times at 2560x1387, clouds half,
> Ultra: inside a deck about 200 ms, a few km above 60-100 ms, orbit 40 ms.
> A read-only performance panel (four map lenses, three designs, two
> judges, synthesis with predicted frame times per situation) is in flight;
> its plan replaces this block's NEXT list when it lands. Increment D (the
> design-B saturating carve remap) was measured and removed: levels
> identical to each other and 10 levels darker than off at the deck top,
> High closeup bit-identical, rain grain worse; the noise-path interior is
> already saturated at high cover, and the deck-top nadir gradient is the
> two-stream source's angular term. FOUND (v0.1283.2): the 146 dark centre was
> the rig's FIRST CAPTURE AFTER BOOT, a deterministic render state (bit-identical
> across three boots; captures two to four read 192). The rig now takes a
> discarded first pass, `sc-top-3p0km` is rewritten to its warm baseline (192
> to 216, no hole), and the 'dark centre' motivation for increment D is void.
> The cause (the sticky showcase pins' first application against the temporal
> history is the suspect) is a toolsmith item. The dev pad bisect bits 13-17 are now a
> 3-bit index at 13-15 (`cloud_bisect_index`), so bits 16 and 17 are free.
>
> NEXT, in order:
> 1. **Performance arc** (operator priority): the panel's first increment,
>    expected to be the far-field handoff (near screen march within an
>    altitude-scaled range; a map, sheet or cheaper march beyond; blended in
>    transmittance over a depth band, never a per-ray switch, so neither the
>    v0.1233 seam nor BUG-074 can return). Gates: fps at operator-bm12,
>    sc-top-3p0km, rain-26km-nadir, cumulus-closeup-ultra, plus an orbit
>    vantage; the look judged by the same captures.
> 2. **Flip the in-cloud light on by default** at `sc-inside-top` (over 170)
>    once the perf floor holds with it on; it reads 230 there today.
> 3. **Re-judge every unpinned down-look golden spec** (`just visual-sweep`):
>    the family is local everywhere now and the specs predate the fix.
> 4. **Pin the rig heading** so masked statistics across captures are safe;
>    rotation-invariant scripts meanwhile (`cloud-radial-coherence.js`,
>    `cloud-lum-bands.js`, `cloud-radial-profile.js`).
> 5. Parked behind performance: the field designs from the rosette panel
>    (A domes / flat bases / blue-noise placement / 3D erosion; B the
>    noise-first deck; C a plume library), the Low-tier orbit seams (draw the
>    sheet from `cloud_weather`, let cores go opaque), the stair-stepped
>    band-top edge, the round grey discs on the 26 km overcast, the inert
>    `g_march_max_km` clamp (which the handoff will likely wire).

> **v0.1282 (2026-09-05): THE ROSETTE FOUND AND KILLED (BUG-074).** It was
> never layer geometry. Since v0.1232.5 the per-ray cloud family was read at
> the midpoint of the UNCLIPPED top-shell chord, which for any down-look runs
> through the planet, so every near-nadir pixel read the cloud type about
> ninety degrees around the globe in its own screen azimuth; the 8-19 degree
> type cells printed as wedges from the nadir and sectors with a band base
> above the camera as see-through slivers. One line (`reg_reach`, the lookup
> capped at four slab thicknesses down the ray) took operator-bm12 from
> +0.91/+0.81/+0.96 inner-bin coherence to 0.00, slivers 2 to 0, and the
> frame to plain fog; rain-26km-nadir grain 1.99 to 0.61; the type-pinned
> closeup bit-identical; the horizon-seam vantages clean. Found by a fresh
> read-only panel (five lenses, two refuters each; the march lens reproduced
> the pattern offline at r 0.66 before any rebuild). The synthetic checker
> had bypassed this stage, which is why "projection correct" was misread as
> "unavoidable". Full record: `docs/BUGS.md` BUG-074.
>
> ALSO SHIPPED, default off: increment C (F10 "interior saturation", showcase
> `cloud_int_sat`). A null where aimed: the stratocumulus deck is owned by the
> sheet union and C touches only the constructed bodies (bit-identical at the
> 3.0 km deck top); deep inside a Cb it darkens (39 to 34), the right sign.
> THE INTERIOR TARGET IS ALREADY MET where the family is local: 180 m under a
> stratocumulus top the in-cloud light reads 230 of 255 at sat 0 (new standing
> vantage `sc-inside-top`; the ladder 3.0/2.8/2.6/2.4/2.2/2.0 km read
> 175/230/218/208/191/153).
>
> NEXT, in order:
> 1. **Deck top from its own level** (`sc-top-3p0km`, new standing): the nadir
>    ray sees deepest and prints a dark centre, 146 against 207 at the edge.
>    Fix on the NOISE path: a saturating, coverage-preserving density remap
>    (design B increment 1, the Nubis remap) so the carve reaches 1 within
>    tens of metres of the surface. Gate: nadir within 15 levels of the
>    surround; `sc-inside-top` stays over 170.
> 2. **Flip the in-cloud light on by default** once (1) lands: re-judge at
>    `sc-inside-top` (over 170), `operator-bm12`, `cumulus-closeup-ultra`.
> 3. **Re-judge every unpinned down-look golden spec** (`just visual-sweep`):
>    the family is now the local one everywhere, so every such vantage changed
>    look, for the better, and the specs were written against the bug.
> 4. **Pin the rig heading.** Two 4.6 km down-look captures in one sweep came
>    out rotated about the nadir; masked statistics across captures are unsafe
>    until `camera_request` takes an azimuth. Rotation-invariant scripts in the
>    meantime: `scripts/cloud-radial-coherence.js`, `cloud-lum-bands.js`,
>    `cloud-radial-profile.js`.
> 5. Smaller, observed this session: a stair-stepped hard edge where the band
>    top is seen edge-on from 20 m above it (`sc-top-3p0km`, top-left); the
>    Low tier from orbit shows hard-edged patches (the sheet caps alpha at 72%
>    and the ground draws its own cloud darkening from a different weather
>    field) - draw the Low sheet from `cloud_weather` and let dense cores go
>    opaque; the inert `g_march_max_km` clamp (no writer) can be retired or
>    wired; the 26 km rain overcast shows perfectly circular grey discs.
> 6. Then the field designs from the panel (A: domes, flat bases, blue-noise
>    placement, 3D erosion on the built path; B: the 3D noise-first deck), now
>    judged against a renderer that no longer lies about the family.

> **v0.1281 (2026-09-04): increment B 2.1 built, null at Ultra; the pad is
> full.** The three-octave measure-preserving domain warp on the noise path
> (24 km / 0.8 km, 3 km / 120 m, 0.6 km / 30 m; cell split following the
> walls) is behind bit 23 (F10 "Field walls"), the LAST exactly representable
> f32 bit: the next flag needs a `bitcast<u32>` pad or the one-hot bisect bits
> 13-17 folded into an index. Measured with the in-cloud light on: bm-12
> 57.8 -> 58.3, coherence and coverage unchanged, rain and closeup unchanged,
> ~10% fps. Same structural reason as the wall-wander null: at Ultra the field
> the eye sees at bm-12 is the CONSTRUCTED bodies, which the noise-path warp
> does not touch. NEXT, in order, all on the built path: (1) interior
> density / skirt sharpness of the bodies (the model deck is optically half a
> real one; a top must be opaque within tens of metres) - the change that
> should move the interior-brightness gate; (2) design 2.2: Poisson occupancy
> with a 2- and 6-cell organisation multiplier, an isotropic placement warp
> replacing the row stagger, DISP2 26 -> 45 m, ERODE2 22 -> 32 m, a 150 m
> DISP3 octave; (3) the noise-path warp judged at High quality on its own
> vantage. Then re-measure the in-cloud light at gain 1 and flip it on.

> **v0.1280 (2026-09-04): THE IN-CLOUD LIGHT, BUILT AND MEASURED; THE
> LIMITER IS THE MODEL DECK OPTICAL THICKNESS.** The design (fidelity
> reference + code audit + refuters) put numbers on the target: at 45/km the
> transport mean free path is 148 m, Koschmieder visibility 87 m, so 99% of an
> in-cloud pixel comes from the nearest 100 m and the inside of a sunlit
> cumulus at noon is fog-white, 170-210 of 255 after the tonemap. The shipped
> interior read 48 because every "multiple scattering" term was a
> transmittance with no return path (an interior that DARKENED with more
> extinction), the diffuse floor used the regime band top 4 km up as its
> column, and the sun ladder resolved neighbouring lobes for deep samples.
> Built behind bit 22 (F10 "In-cloud light", `cloud_ms`, gain `cloud_ms_gain`):
> burial from the local column, depth-split ladder, Eddington two-stream
> source, rind-only relief/hue/powder/opacity-darkening; `map_diag 8` =
> burial; `scripts/cloud-ab-metrics.js`.
>
> MEASURED at the operator bm-12 camera: burial reads 1.0 wherever the eye
> looks (the light engages), extinction x3 now BRIGHTENS (58 -> 61; was 45 ->
> 29), the interior goes achromatic (B/R 1.08 -> 0.92), but the masked mean
> reached 58 against 135+; gain 2.5 lands at 101, linear, so physics needs
> ~3x. That factor IS the optical thickness: the model column at envelope
> density ~0.5 is tau 13 over 600 m where real cumulus is 27, and at tau 27
> the same formula gives 143/255 by itself. The light model is right in form;
> its input is the model's thin interior - the same thinness that lets the
> eye see 600 m into a skirt and the ocean through slivers. DEFAULT OFF until
> the interior gate is met by physics. Retired: the direct-channel RED gate
> (that channel is weighted by eye transmittance and carries the radial
> geometry whatever the lighting does).
>
> NEXT, in order: (1) a saturating interior density remap (density -> 1
> inside the core, coverage-preserving: the Nubis remap shape) measured at
> bm-12 by the same gates, then A re-measured at gain 1 and flipped on;
> (2) increment B field work from the design: measure-preserving domain
> warps at 24 km / 3 km / 0.6 km on the noise path, Poisson occupancy and an
> isotropic placement warp on the built path, convective base sharpness per
> family, and the stratocumulus genus rule for continuous decks.

> **v0.1278-v0.1279 (2026-09-04): THE ROSETTE IS LAYER GEOMETRY. PROVEN WITH A
> SYNTHETIC FIELD.** The operator saved F6 bookmark bm-12 at the rosette
> (1.63 km, straight down, clear, mid-Pacific); the rig reproduces it
> pixel-exact (`operator-bm12`). At that camera every remaining hypothesis was
> a null, one sweep each, clock pinned: wall wander 0.5-5 km, sharp base,
> interior relief fade, coarse deep sun ladder, every density component off
> one at a time. From 5 km above the location is a solid overcast; a 3 km
> sideways move gives the same style of picture; extinction x3 makes it 97%
> opaque and DARKER; the direct-sun channel alone reads +0.99. Then the
> operator asked whether a wrong transform could be it, and the SYNTHETIC
> CHECKER (bit 21, F10) answered: density replaced by a known 0.5 km
> checkerboard filling 1.0-1.6 km renders as a perspective-correct grid from
> 3 km above and as THE ROSETTE from bm-12 - one see-through cell below,
> opaque elsewhere, lattice lines as radial slivers. **The projection is
> correct.** From just inside the top of a 600 m-thick layer of km-scale
> features you can only see through it straight down and along channels
> aligned with your sight line: geometry, not a bug, and it cannot be
> removed. Standing vantages `checker-bm12` / `checker-bm12-above` guard the
> projection.
>
> **What is wrong is the LOOK of that geometry, and that is the next work:**
> (1) inside cloud the walls must be fog-white from diffuse multiple
> scattering, not dark shaded petals - the single-scatter sun ladder resolves
> nearby lobe shadows through the eye, and raising extinction darkens instead
> of whitening; (2) gaps must not be km-long straight canyons between 5 km
> cells - real cumulus fields have 1-3 km cells with turbulent 100-500 m
> walls, so aligned channels close within a few hundred metres. Two fidelity
> increments (Nubis-class light volume / multi-octave scattering; field cell
> and wall scales matched to real cumulus), designed next.
>
> Instruments shipped, all default off: density component bisect (bits
> 13-17), sharp base (18), interior relief fade (19), coarse deep ladder
> (20), synthetic checker (21); knobs `cloud_hv_km`, `cloud_sigma_mul`,
> `cloud_shear`. Rig notes: the bookmark restore answers once per boot (use
> the lat/lon equivalent); full res inside a deck times out the camera
> request; `just build-game` purges archived exes beyond five, so capture an
> old-build baseline while it exists.

> **v0.1276-v0.1277 (2026-09-04): THE CONTOUR PINWHEEL, REPRODUCED AND
> KILLED.** The operator captured a full-frame topographic contour pinwheel
> in the real render at 26.4 km over a rain overcast. Reproduced on the rig
> at that exact state: the first standing RED vantage this arc has had for
> the fan (`rain-26km-nadir`, `rain-26km-nadir-full`). Not a regression
> (v0.1271.1 shows the same bands under noise), not the iteration cap (0% at
> 224), not the resolve. With the depth dither ON the bands dissolve into
> exactly the per-pixel static the operator disabled the dither to escape:
> the bands and the static were ONE defect in two disguises, a deck-top
> entry whose depth below the surface is the march comb phase. Root cause:
> the v0.1272 entry bisection was gated on `seg_len > 2*step_near`, and
> step_near is capped at 4.5% of the slab (522 m) while the cone step at
> 14-21 km is 500-700 m, so the bisection NEVER FIRED at altitude - a guard
> added for cost that the design never asked for. Now gated on the
> bisection stop width, five taps with a 30 m stop for the first entry on a
> ray and two for entries behind it. Red vantage, full res, dither off:
> grain 17.8 -> 3.1, bands gone; what remains at the nadir is the soft
> radial elongation of the deck holes - the prism-wall content geometry.
>
> Also: the v0.1275 `g_v2_sdf_m` reset at every carve is REMOVED (it turned
> the no-SDF sentinel on for every body-skipped sample, i.e. clear air, and
> killed the clear-air stride); the ladder leak is closed by saving and
> restoring the value around the sun ladder instead. And a frame-time scare
> (nadir-anchor-40 at 3.7-5.4 fps against a remembered 15.6) was the
> sticky-pin hazard confounding FPS: with cover, type and weather pinned,
> v0.1272.1 and the current build both read 3.9 fps. Both anchor vantages
> now pin them. The 58 km "texture" look is the volumetric deck itself (a
> scattered field at 58 km is a speckle sheet); what is real there is that
> cloud tops read flat white from altitude, a fidelity item, not a handoff.

> **v0.1275 (2026-09-03): THE ROSETTE NAMED, INCREMENT 1 BUILT, AND THE
> RIG CAN NO LONGER FIND THE DRAMATIC FAN.** The residual hunt (3 mappers,
> 6 refuters, design; wf_7b894dc6-a02) cleared the sun ladder (alpha never
> reads tau) and the temporal resolve (a parked camera accumulates the march
> plus a uniform blur) and named the carrier: every coverage-defining field
> has a horizontal correlation length larger than the regime band height
> (finest shape Worley cell 5.6 km; cumulus band 5.2 km), so masses are
> vertical-walled prisms whose walls converge at the nadir. Correct
> perspective of an unphysical field; three physics reviewers called it a
> defect. Built from its design: ladder side-channel hygiene (unconditional),
> the `cloud_shear` lean pin (F10 slider), `scripts/cloud-radial-coherence.js`
> (the spoke metric cannot see elongation; this can - validated, peak 72 px
> from the predicted nadir pixel), a thin-deck toggle (null by construction
> at Ultra: constructed-body height is width x aspect, not the band).
>
> MEASURED: the nadir coherence tracks body height (congestus +0.20,
> stratocumulus none), temporal-off is identical, and the lean changed the
> field but did NOT move the coherence peak. At the operator horizon-look
> and steep-look states on the CURRENT build the sun and ambient channels
> show solid bodies, no fan, no slivers; the archived v0.1270.1 exe at
> identical pins shows the bullseye rings and 1.4-2.5x the grain, but also no
> dramatic fan. So the rig does not reproduce the photographed fan on either
> build at any vantage tried, while the rings and pepper the operator
> photographed are gone. NEXT: the operator flies v0.1275.1. If a fan
> remains, capture its EXACT camera (HUD altitude and heading into a
> camera_request) rather than guessing vantages. Increment 2 (regime-matched
> deck thickness, bounded per-family lean, height-varying warp) waits on that.

> **v0.1273-v0.1274 (2026-09-03): the assessment design is fully executed.**
> 2A (carve normaliser floor, bit 10) built; null at a 9.2 km clear sky
> (grain 1.85 vs 1.86, identical coverage) because the stencil regime
> (weather alpha below ~0.13) is not present there; default OFF until a
> low-alpha vantage exhibits it. 1B+1C (isotropic near step + bounded far
> angle term, bit 9) built and verified clean at 3.4 / 9.2 / 60 km: coverage
> and sun channels unchanged within noise, cloud fraction within one point,
> the step-count chord sweep gone (12.3 -> 8.4 at 9.2 km), at 8-10% frame
> time in the near field. Default OFF: no visible gain, real cost, and
> performance is the top open item. Both are instruments now. Second
> confirmation that the step law is not the residual rosette carrier. The
> residual hunt (sun ladder first tap, resolve rest blend, 2D-extruded
> coverage field) is running as workflow wf_7b894dc6-a02. Rig rule learned:
> keep sweep batches under about six captures at slow altitudes (an
> 18-capture run blew the 10-minute tool ceiling and lost everything);
> metrics in one process (`scripts/cloud-iso-metrics.js`).

> **v0.1272.0 (2026-09-01): THE SAMPLE-ANCHORED MARCH. Glitter, dark pepper
> and winking clouds were ONE estimator bug; it is fixed and ON by default.**
> The v0.1271 assessment (7 agents, 1-D twins of the shipped loop) found the
> march biased to FIRST order in the step h: the march position `t_cur` and
> the sample position `tm` were two different variables and every endpoint
> rule mixed them (SDF stride measured at one, spent from the other; entry
> backtrack to the wrong point; entry trapezoid against zero; first-step
> rewind behind the eye; exit half-step dropped). Inside the deck h = seg/16
> = 188 m / cos(theta), so each bias was a nadir pinwheel in the MEAN and the
> frozen per-pixel jitter made each a static coin flip. Fix (dev pad bit 7,
> F10 "Sample-anchored march"): the sample IS the state, 2-tap bisection
> entry localisation, priming tap at the eye, skirt floor, exit credited,
> refine at rind/4. Plus bit 8: the domain warp band-limited to its own tile
> (`CLOUD_V2_WARP_LODC` = -5.2 encoded a 6.96 km tile; the warp tile is
> 0.19-1.4 km, so every silhouette was a 3-20 m inside/outside hash).
>
> GATES (written before the sweep; one run, clock pinned, 9.2 km cloudy):
> real-render grain 5.05 -> 1.76 (-65%); coverage grain -45% / -65% with the
> warp fix; direct-sun grain -61% / -73%; direct-sun luminance UP (110 -> 125,
> the sunlit rind integrated again); cloud fraction UP (6.0 -> 11.0%: missed
> clouds return). The new `map_diag 6` entry-depth channel read RED first on
> the old march (grain 15.4, a per-pixel 0-600 m hash filling every cloud,
> the twin said 311 +- 280 m) and near-flat on the new one. SHARP PREDICTION
> HELD: the depth-jitter factorial collapses from a 1.9 gap to 0.06 with the
> fix, so the depth dither is no longer load-bearing and can be left OFF.
>
> **The rosette is NOT this.** Radial energy did not fall (it rose with the
> extra cloud content; the metric is content-weighted). Per the design, a
> spoke that holds after the estimator fix means the residual radial
> structure is not the step law. Next suspects, in order: the sun ladder
> first tap (~0.9 km), the resolve rest blend, and the 2D-extruded coverage
> field seen from within (dirp is constant along a nadir ray). With the
> noise gone the fan is now MORE legible in the operator view, which is the
> right state to hunt it in.
>
> Still to do from the design: 1B+1C isotropic near step + bounded far angle
> term (bit 9), 2A normaliser floor (bit 10); delete bits 5/6 (both null).
> Tooling: `scripts/cloud-twin/` (the twins, permanent),
> `scripts/cloud-gate-metrics.js` (all gate metrics in one process),
> `scripts/cloud-fraction.js` (cloud loss beside every number).

> **v0.1270.0 (2026-09-01): ONE CHECKBOX WAS DRIVING TWO UNRELATED JOBS.**
> This is why the "TV static versus rosette" tension existed at all. The F10
> "Spatial dither" box set `dither_on`, which gated BOTH the depth jitter
> (where inside its step each sample is taken) AND `g_lod_jitter`, the
> 2026-08-24 per-pixel mip dither added to cure concentric mip rings. Its own
> comment says it: *"the mip ladder crosses integer levels at fixed radii
> around the camera... print those crossings as CONCENTRIC RINGS"*, and
> *"lodb is monotone in screen radius on a down look"*. The operator had that
> box OFF in every screenshot of this arc, to kill the static - which silently
> disabled the depth jitter too. So every screenshot they sent was the
> worst-case configuration while the rig ran the best case, which is also why
> my captures never looked as bad as theirs. Now two switches.
>
> FACTORIAL (one sweep each, 9.2 km, clock pinned, 0.7 noise floor):
> depth ON + mip ON 23.94; depth ON + mip OFF 23.74; depth OFF + mip ON 27.36;
> depth OFF + mip OFF 27.43. **The depth jitter carries all of it; the mip
> dither carries none.**
>
> The mip dither NEVER helped in any state tested and once hurt (3.4 km clear
> 23.65 with it vs 21.05 without; 9.2 km 23.94 vs 23.74; 60 km 14.71 vs 14.71),
> so **its default is now OFF**: a per-pixel random mip through the non-linear
> carve averages to a biased result whose bias itself varies with radius, so
> the ring cure carried its own radial signature. The DEPTH jitter default is
> UNCHANGED, because its effect is SCENE-DEPENDENT - it helps strongly at
> 9.2 km clear and mildly hurts at 3.4 km cloudy (17.44 vs 16.02). Do not tune
> it from a single vantage. Guards: `jitter-factorial-34km` and
> `depth-jitter-load-bearing`.
>
> **The 6-agent shape-lod audit cleared every shape field as the carrier**, and
> its adversarial half earned its keep. The mapper rated the surface
> displacement octaves "dominant"; the magnitude refuter replicated the noise
> bake in JS and MEASURED adjacent mips to be highly correlated (rho 0.96 to
> 0.998), not decorrelated as the mapper assumed - so a mip crossing moves the
> surface 1 to 3 m, not 29 m, against clouds 85 to 760 m across. That confirms
> the empirical negative from the F10 world-anchored-shape A/B independently.
> Domain warp, erosion and the march step schedule were each refuted 2-0; the
> rind and carve hinge came back clean.
>
> Two durable facts from it: the sun path never fetches the domain warp at all
> (`g_sun_profile = 1.0` on every sun tap forces `wn` to the FBM mean), so the
> warp cannot be the direct-sun carrier; and the in-flight comment attributing
> the near-field ramp to `CLOUD_V2_INT_LODC` = -9.56 names the WRONG constant -
> that is the interior turbulence tap, while the fine displacement is
> `CLOUD_V2_DISP2_LODC` = -8.76.
>
> METHOD NOTE, the fourth measurement hazard in this arc: showcase pins are
> STICKY across vantages in one sweep. A measurement vantage must set every
> pin it depends on - including `cloud_cover` and `weather` - or it silently
> inherits whatever ran before it. Comparisons WITHIN one sweep stay valid
> (all cells inherit the same thing); comparisons ACROSS sweeps do not.

> **v0.1269.0 (2026-09-01): THE PROBE RIG WAS FALLING.** The operator caught
> it from a single HUD word: every rig capture reads `WALK x1 [F9 to fly]`
> while their own session reads `FLY x1M - hover, no gravity`. The movement
> PHYSICS reads `gui_state.dev_hover` (`MoveMode::from_dev_flight(dev_hover)`),
> but `camera_request` set only `dev_fly_mode` + `controller.fly_mode` at all
> three placement sites, so the model stayed in Walk: gravity pulled, the
> ground clamped, and a camera placed at altitude SANK during its settle.
> Measured drift: 5.9 km requested captured at 5.7; 3.4 requested captured at
> 3.2. Every altitude-sensitive vantage in the cloud arc was shot below its
> requested altitude, and cloud artifacts here are altitude-sensitive BY
> DEFINITION (the deck spans 1-12 km and the whole question is where the
> camera sits inside it). Fixed at all three sites; verified 9.2 km holds and
> ground vantages still sit on the surface. Guard: `altitude-hold-check`.
>
> **That is the THIRD measurement-validity defect in two days** - the empty
> vantage, the non-deterministic advection clock, and now the falling camera.
> Together they explain the shape of this entire arc: fixes that measured well
> and then failed in flight. Before trusting ANY visual verdict from the rig,
> confirm the capture is of the state you asked for.
>
> **Shape-lod hypothesis: TESTED, NOT CONFIRMED (honest negative).**
> `g_v2_disp_lod` is assigned per sample from `lodb` = log2 of a
> camera-distance footprint, which violates the invariant written directly
> above its own declaration ("Displacement is SHAPE, so every evaluation that
> reaches a given point in the world must agree on it"). Since lines of equal
> distance-to-camera project to circles centred on the nadir, camera-anchored
> shape detail would paint exactly the observed artifact, and the fine
> displacement octave (`CLOUD_V2_INT_LODC` = -9.56) ramps across roughly
> 1.7 km to 425 km - the near field - at a mip per doubling. Built the test
> behind dev pad bit 3 (F10 "World-anchored cloud shape"). Result at 9.2 km,
> clock pinned: spoke 23.90 camera-anchored vs 23.48 world-anchored, inside
> the 0.7 noise floor; step-comb frames visually near-identical. The invariant
> IS violated and is worth fixing on principle, but it is NOT the carrier of
> this artifact. Toggle kept, default off.
>
> Also hardened the dev pad bit tests: `chord_foot` was `w >= 3.5`, a
> magnitude test a fourth bit would have silently broken - the same collision
> that already caught the shape-frame flag earlier in this arc.
>
> STILL OPEN: the artifact is anchored to the local vertical (see the v0.1268
> block below for that proof). Leading unexamined suspects, in order: the step
> schedule's `dt_vert`, which divides by `r_rate = abs(dot(normalize(p), rd))`
> - the ray's verticality - and so varies sampling density by up to 20x from
> nadir to horizon; and the DECK DEPTH, since several regime bands span 4-6 km
> of vertical extent (`t_h_lo`/`t_h_hi` against a 1-12 km slab) where real
> cumulus is under 1 km thick, so looking down from inside it is looking down a
> deep well of cloud rather than at a sheet. The second is the operator's own
> multi-layer-atmosphere suggestion arriving from a different direction.

> **v0.1268.0 (2026-09-01): THE ROSETTE IS ANCHORED TO STRAIGHT DOWN,
> NOT TO THE CAMERA.** This corrects the heuristic printed further down
> this file and the target of the last five fixes. Aim the camera 40
> degrees OFF nadir and the pinch stays at the NADIR point, well below
> the crosshair; at 70 degrees it leaves the frame entirely; at 0
> degrees it sits exactly on the crosshair. Every operator screenshot
> in this arc was a down-look, where nadir and crosshair COINCIDE, so
> "view-anchored" and "vertical-anchored" were indistinguishable in the
> evidence. The test to apply from now on is therefore NOT only "does a
> photon arriving here care where the camera is" but also, and for this
> artifact chiefly, **"does it care which way is DOWN?"**
>
> STRUCTURAL CAUSE (found, not yet fixed): `cloud_weather_adv(dir, ...)`
> takes only a DIRECTION, so the coverage field is 2D extruded
> vertically through the whole 1-12 km deck. Along a ray pointing
> straight down, `dirp` is CONSTANT for every sample, so the entire
> column returns ONE coverage value; oblique rays sweep many values and
> average. Variation along a ray falls to exactly zero at the local
> vertical: a nadir-anchored singularity by construction. It explains
> every symptom at once, including the ones that defeated the
> view-dependence fixes: strongest inside the deck, faint from orbit at
> the sub-camera point, present in coverage alpha, unmoved by anything
> keyed on the camera. Realistic-first candidate is real altitude
> structure in the coverage field (wind shear veering the lookup with
> height, as Nubis/Decima do), but MEASURE FIRST: the finest weather
> octave is ~600 km against an 11 km deck, so physically-scaled shear
> may be far too small to decorrelate the column, and a bigger-than-real
> shear would visibly smear the deck.
>
> **THE RIG WAS NOT DETERMINISTIC, so distrust this whole arc.** The
> cloud advection clock is app-start-relative, so every boot dropped the
> field somewhere new: the SAME build and SAME vantage differed in 20%
> of pixels by more than 40 levels between two runs. Every cross-run
> before/after here compared two cloud FIELDS, not two builds. Fixed
> (`cloud_clock_pin`, showcase `cloud_clock`, F10 "Freeze cloud
> drift"): cross-run difference 34.2 -> 4.1 mean levels. Worse, the
> `pinch-inside` vantage added at v0.1267 rendered an EMPTY DUSK SCENE
> with no clouds in it, and the v0.1267 fix was declared verified
> against it - the `feedback_checks_that_cannot_fail` class, again. And
> at exact nadir the camera roll is degenerate, so consecutive captures
> silently rotate the scene; measurement vantages now use a tilted aim.
>
> DEAD, do not re-open: the 224-step iteration cap (rebuilt step-count
> channel reads ~22 steps of 224, flat noise, no ring). PARTIAL FIX
> SHIPPED: the chord is gone from the detail scale entirely (per-sample
> now, beside `g_v2_disp_lod`); one run, clock pinned, old 23.64 -> new
> 19.11 against a coverage reference of 18.68 and a 0.7 noise floor.
> v0.1267 had only CAPPED that term, which bounded the sweep and added a
> transition ring where the cap engaged. Live A/B kept behind the F10
> "Old chord detail scale" box. Also measured: old + temporal OFF reads
> 19.31, i.e. **the temporal accumulator is a matched filter for
> anything anchored in screen or vertical space** - it averages ~25
> frames per direction, so it sharpens a weak standing bias into a
> visible pattern. That is why the artifact always looked far stronger
> than its per-frame magnitude.
>
> NEW INSTRUMENTS (permanent): `map_diag` 4 = march step count, 5 =
> step comb, both with F10 buttons; `scripts/cloud-spoke-metric.js`
> (radial-sliver metric - needs a same-run control, noise floor ~0.7);
> vantages `nadir-anchor-40`, `nadir-anchor-70`, `steps-inside`,
> `foot-chord-ab`.

> **v0.1267.0 (2026-09-01): THE CHORD NO LONGER SETS THE DETAIL SCALE -
> the inside-the-layer pinch.** Operator: "It still exists very
> strongly while inside the cloud layer... I do not understand why it
> keeps pinching at the bottom." The answer was already written in the
> codebase, four releases before anyone connected it to the rosette:
> g_v2_foot_m (the per-ray footprint that freezes the constructed
> body's displacement / erosion / warp mip) was taken at the SEGMENT
> MIDPOINT, and its own comment admits "the surface DETAIL of every
> cloud on the ray changes by 1.34 mip levels along one screen row.
> Coarser above where the segment is long, finer below where it is
> short." That is a radial gradient - and INSIDE the deck it is the
> WHOLE term, because m0 collapses to ~0 in every direction while the
> chord runs from a few km straight down to hundreds near the horizon.
> FIXED: the chord's contribution is capped at CLOUD_FOOT_CHORD_CAP =
> 4 km (cloud-body scale). The useful part survives; the part that only
> encodes viewing ANGLE is dropped. Outside the deck this changes
> almost nothing (m0 dominates), which is exactly why the artifact was
> always worst inside. The per-RAY freeze is preserved so the eye and
> its sun taps still shade one surface (the v0.1234 rule).
> FIVE RADIAL MECHANISMS ADDRESSED: placement lattice (v0.1252.7),
> coverage/mip drift (v0.1263 + v0.1265), view-dependent sun
> transmittance (v0.1264), view-dependent ambient relief (v0.1266),
> chord-driven detail scale (this). GOVERNING TEST, which caught the
> last three: does a photon arriving here care where the camera is?
> A LESSON WORTH KEEPING: three of the five were already DOCUMENTED in
> code comments as known scale-dependencies - written honestly, and
> never connected to the artifact they were causing. When an artifact
> resists, grep the comments for admissions before adding suspects.


> **v0.1266.0 (2026-09-01): the ambient residue - and tau_vert
> ELIMINATED by measurement.** Operator on v0.1265.1: "The rosette
> lives but it is almost gone", with three faint residues - one "from
> high orbit... much more pronounced in the ambient light setting", on
> the full SHEET rather than the voxel clouds. ELIMINATION FIRST: a new
> carve_magnitude_fit measured the per-mip carve MAGNITUDE (what
> tau_vert is built from) at gains 0.997-1.010 - already mip-invariant
> to 1%, because the v0.1265 signed-threshold fit corrected magnitude
> as a side effect. tau_vert is NOT the carrier and needs no gain
> table; recorded so nobody builds one. THE CARRIER: crown_shade and
> pouch_shade are functions of body, a MIPPED sample, so they inherit
> the view footprint - and on the CONSTRUCTED path they are already
> neutralised (ring_off, v0.1252.6) while on the NOISE path they run
> full strength, exactly where the residue shows. Ambient now takes 35%
> of each; DIRECT keeps them fully, since the sun path no longer reads
> the view footprint at all after v0.1264.
> FOUR RADIAL MECHANISMS ADDRESSED ACROSS THIS ARC: the placement
> lattice projecting as orchard rows (v0.1252.7), coverage/mip drift
> (v0.1263 + v0.1265, ladder +53% -> scattered +-8%), view-dependent
> sun transmittance (v0.1264), and view-dependent ambient relief
> (this). The governing test, which caught the last two: DOES A PHOTON
> ARRIVING HERE CARE WHERE THE CAMERA IS? Anywhere the answer is no and
> the code says yes is another rosette.
> STILL OPEN, in priority order: PERFORMANCE part 2 (Nubis3 amortized
> light grid ~40% + adaptive march resolution - full res is still
> sub-10 FPS); MULTI-LAYER ATMOSPHERE (the top fidelity item - a sky of
> one deck cannot read as a real sky); field variety V2 (size span,
> base scatter) for the cotton-ball shapes.


> **v0.1265.0 (2026-09-01): THE CARVE HINGE SPLIT - mip drift is now
> essentially GONE.** The operator localised the residual exactly: "the
> rosette is weakest now against the voxel clouds but still very
> present in the SHADER clouds". The constructed (voxel) bodies take
> coverage from an SDF with NO mip dependence; the noise (shader) body
> coverage IS a thresholded mip - so the residual had to be the drift.
> THE BLOCKER, now removed: the hinge had ONE per-mip number doing two
> jobs - the half-width (a SOFTNESS, used as a DIVISOR, necessarily
> positive) which also doubled as the threshold shift. The coverage fit
> wants NEGATIVE shifts at mips 2-5, a divisor cannot go negative, and
> so v0.1263 was clamped at its floor with 36% of the correction out of
> reach. CLOUD_CARVE_T0..T8 is now a SIGNED threshold offset applied as
> (body - (thr + T)) / sw, softness uniform, T carrying every shift.
> MEASURED, mips 1-6: +18/28/27/35/34/53% (monotone CLIMB) before the
> arc -> +18/18/18/14/14/7% after v0.1263 -> NOW +1/+1/0/-4/+4/-8%.
> Scattered noise about zero instead of a climb, and scattered error
> cannot form a coherent radial gradient. Total coverage error 0.947 ->
> 0.408: the full unconstrained target.
> THE THREE RADIAL MECHANISMS FOUND IN THIS ARC, all now addressed:
> (1) coverage/mip drift (v0.1263 + this), (2) view-dependent sun
> transmittance (v0.1264 - sun tau was floored by the VIEW footprint),
> (3) the placement lattice projecting as orchard rows (v0.1252.7).
> IF A RESIDUE REMAINS: continue the view-dependence audit - tau_vert,
> the ambient shaper and the powder gate all read view-derived
> quantities, and the test is one line: does a photon arriving here
> care where the camera is? Anywhere the answer is no and the code says
> yes is another rosette.


> **v0.1264.0 (2026-09-01): THE SECOND ROSETTE - sun transmittance no
> longer depends on the camera.** The operator separated the two
> mechanisms with one capture set: at 2.7 km under a thick deck their
> COVERAGE ALPHA is uniformly white (saturated, no pattern) while
> DIRECT SUN and AMBIENT show an enormous radial flower. Coverage
> saturates and hides its drift; the lighting does not. THE BUG:
> cloud_sun_tau computed each tap mip as max(lodb, log2(seg)) - the sun
> tap FLOORED BY THE VIEW FOOTPRINT. Sunlight arriving at a point in a
> cloud does not care where the camera is, but lodb does, and on a
> down-look it is monotone in the angle from the nadir. So sun
> transmittance at a FIXED world point changed with the viewer screen
> angle: a radial lighting gradient centred on the view axis, by
> construction. FIXED with a view-independent band limit - the tap own
> segment length, floored by the 260 m radiative-smoothing scale - so
> tau is a pure function of world position and sun direction, with cost
> still capped.
> TWO RADIAL MECHANISMS NOW ADDRESSED: coverage/mip drift (v0.1263,
> ladder +53% -> +7%) and this. ALSO CONFIRMED from their discard
> bisect: RED filling everything below the horizon under the deck is
> CORRECT (downward rays are genuinely behind the planet), not a bug.
> NEXT if a radial residue survives: audit every remaining term in the
> lighting chain for lodb dependence the same way - tau_vert, the
> ambient shaper and the powder gate all read view-derived quantities,
> and the same question applies to each: does a photon arriving here
> care where the camera is?


> **v0.1263.0 (2026-09-01): THE ROSETTE MECHANISM CONFIRMED, AND
> LARGELY CORRECTED - carve widths refitted against COVERAGE.**
> Operator: "Ambient shows rosette. Direct sun shows rosette. Coverage
> alpha shows it." All three channels is what localised it to COVERAGE
> rather than lighting. Then the decisive test: PINNING THE MIP to a
> constant collapsed coverage from a full cloud field to almost
> nothing - coverage is enormously mip-sensitive, and the mip a sample
> takes is set by its footprint, which on a down-look is monotone in
> the angle from the nadir. Coverage that changes with mip paints a
> radial gradient centred on the view axis. THAT IS THE ROSETTE.
> THE FIX: a new fitter (coverage_width_fit) sweeps the carve width per
> level minimizing absolute COVERAGE error against the level-0 truth -
> the objective the standing note asked for since it was written. The
> shipped fitter minimizes the MEAN of E[relu], which is how it could
> pass its own gate while coverage drifted +53%. MEASURED: mips 1-6
> went from +18/28/27/35/34/53% (a monotone CLIMB) to
> +18/18/18/14/14/7% (nearly FLAT). A constant offset cannot paint a
> radial gradient; only a climbing one can. flower-nadir - the vantage
> that has shown agate mip rings for this entire arc - now renders
> flat and unstructured.
> ALSO: the width is a DIVISOR in the hinge, so it must stay positive;
> the unconstrained fit wants small NEGATIVE widths (0.947 -> 0.408 vs
> the legal 0.643), which is the remaining headroom and would need the
> hinge reformulated to take a signed threshold shift separately from
> its softness. AND: coverage_across_the_mip_ladder held a STALE COPY
> of the width table, so it reported the ladder of a table that was no
> longer shipping - which is why earlier width edits appeared inert. A
> harness that does not track what ships is a harness that lies.
> AND: the F10 "Discard reasons" checkbox missed in v0.1262 now ships
> (the plumbing landed but the edit script aborted before the UI
> insert - GUI-first means the button ships in the same commit).


> **v0.1262.0 (2026-09-01): the DISCARD-REASON BISECT, and the
> composite is exonerated.** Every path in the cloud composite that
> kills a pixel used to do it with a bare discard, so a region of
> missing cloud looked identical whichever of SIX reasons removed it.
> Each now paints its own colour under F10 "Discard reasons" (showcase
> {"cloud_discard":"1"}): blue = ray missed the shell, purple = segment
> behind camera, orange = empty slab segment, RED = analytic
> planet-horizon cull, GREEN = scene depth (terrain in front of the
> cloud), grey = the march found no cloud. FIRST VERDICT at the
> cumulus-closeup vantage: every killed pixel is GREY - not one red,
> green, blue or orange. The geometric culls never fire. So the
> composite does not remove clouds; where cloud is missing, the MARCH
> produced no coverage. Combined with v0.1261 (map deleted outright),
> both of the leading splotch suspects are now eliminated by
> measurement and the hunt moves to the march/field.
> THE OPERATOR NOW HOLDS THE TOOL: flip the toggle while a splotch is
> on screen and the colour names its cause instantly - the right shape
> of deliverable, since their exact state has never reproduced on the
> rig. If it paints GREY there too, the field/march is confirmed and
> the next instrument is a march-side channel (first-hit distance,
> iteration count, transmittance) rather than another suspect.


> **v0.1261.0 (2026-09-01): THE OCTA MAP IS FULLY DELETED.** Operator,
> after v0.1260 stopped sampling it: "Let's go for the full delete. I
> still see the effect." Removed end to end - the composite binding,
> its layout entry, the WGSL declaration and every map helper
> (map_basis / map_encode / map_catmull_rom / to_local / to_world); the
> composite is now GATED ON THE SCREEN PAIR (it used to be gated on the
> octa pair, which is exactly why the map could not simply be deleted -
> the composite refused to run without it); set_cloud_temporal no
> longer allocates the 4096^2 RGBA16F ping-pong pair or its bind groups
> (~256 MB VRAM freed); and the Cloud Octa Temporal Pass dispatch plus
> its idle/boost bookkeeping are gone from render_celestial_onto. Both
> feature sets compile, 53 cloud tests green, rig 3 vantages panics=0,
> FPS unchanged or better. This retires the last remnant of the
> two-renderer architecture begun in v0.1250.
> IF THE SPLOTCHES SURVIVE THIS, the map is definitively exonerated and
> the next suspects are the composite's own discard paths - the scene-
> depth occlusion test (terrain LOD patch depth in front of the cloud
> segment) and the analytic planet-occlusion discard - both of which
> can kill whole regions of cloud and neither of which has been
> instrumented yet. A discard-reason bisect channel is the tool.


> **v0.1260.0 (2026-09-01): the retired map was still being composited
> - the operator diagnosed it blind.** They asked "is there another
> shader or texture affecting cloud shaders that is not supposed to be?"
> There was exactly one: the octa direction map stopped DISPATCHING in
> v0.1250 but cloud_composite.wgsl kept BINDING its texture and
> blending it under every cloud pixel as the near-over-map backdrop. A
> render target that is never written is not a guaranteed-zero source
> across backends and driver paths, and whatever it held went into the
> final cloud colour - a plausible source of the "clouds disappear in
> weird splotches" report. The backdrop term is now literally zero.
> ALSO: the v0.1258 trapezoid was a NO-OP - dens_prev is advanced one
> line above where it was read, so it averaged a value with itself.
> Fixed (capture before advance); with it actually working the
> coverage-alpha grain went 2.04 -> 1.86, and the 1.91 previously
> credited to it was noise. LESSON for the next march edit: dens_prev /
> sdf_prev are advanced BEFORE the shading block, so anything wanting
> the previous step must capture it first.
> FOLLOW-UP available if splotches persist: the composite still binds
> cloud_map and keeps map_catmull_rom; removing the binding and the
> whole map arm (plus the octa textures and their pass) is a clean
> deletion now that ONE RENDERER is settled, and would also free the
> 4096^2 RGBA16F pair.


> **v0.1259.0 (2026-09-01): BLUEPRINT 1 TESTED AND REFUTED - a real
> negative result, plus the F10 shape A/B.** Implemented the designed
> cure for the rosette (per-mip CDF histogram matching in the noise
> bake, replacing the linear mean/sigma renormalization) and ran BOTH
> harnesses. It measured WORSE: mips 1-6 drifted +27/37/37/56/47/74%
> against the linear form 18/28/27/35/34/53%. The width harness then
> refit the table DOWN and passed its own gate, but coverage did not
> recover. REASON (now written into the width-table comment so nobody
> retries it): matching the GLOBAL distribution does not make a mip
> agree LOCALLY with the area-averaged truth - thresholding a smoothed
> field is not the same as smoothing a thresholded field. Bake and
> table reverted; gate green. THE LEVER IS THE CONSUMPTION SIDE: refit
> the soft-hinge width against COVERAGE rather than the mean - which
> is exactly what the standing note in clouds.rs coverage_vs_mip has
> said since it was written. That is the next attempt at the rosette,
> and it needs a coverage-objective fitter (the existing harness fits
> mean-error), which is a contained, testable piece of work.
> SHIPPED FOR TESTING: F10 "Cloud shape frame" checkbox (and showcase
> {"cloud_shape":"0"}) rendering the pre-v0.1256 isotropic ball
> cluster, so the squash/wind-stretch work can be A/B compared live.
> The pad light7_color.w is now a bit field (bit 0 dither, bit 1
> shape); the dither test was corrected to read bit 0 only.
> QUEUE: coverage-fitted widths (rosette); multi-layer atmosphere (top
> fidelity item); perf part 2 (Nubis3 light grid + adaptive res).


> **v0.1258.0 (2026-09-01): THE ROSETTE UNIFIED WITH THE MEASURED MIP
> DRIFT - Blueprint 1 is the cure, not a side quest.** The operator
> described the artifact geometrically for the first time: "the bottom
> 45 degree cone beneath my feet is scrunching the clouds together...
> at a distance the textures look proper fluffy, closer to my feet they
> warp inwards", plus "the static is concentrated at the CENTER of the
> clouds". THE UNIFICATION: both quantities that set the march sampling
> scale are monotone in the angle from the NADIR - r_rate (radial dot
> ray) drives dt_vert, and foot = tm * pix_ang drives lodb/mip. Step
> size AND detail mip therefore vary radially about the feet BY
> CONSTRUCTION. The v0.1252.4 workflow already MEASURED that the
> rendered result is not invariant to that scale (carve response
> residual doubling per rung from mip 3, ~15% relative near threshold,
> eight uncompensated taps). Appearance varying with a quantity that
> varies radially about the crosshair IS the rosette. So BLUEPRINT 1
> (per-mip histogram matching in cloud_noise.rs renormalize_level, then
> refit the carve width table from the harness) is the direct cure for
> the oldest complaint in the arc and is now THE next increment.
> SHIPPED this round: trapezoid step integration (the exact integral of
> a linear ramp over the step, using the endpoint density already held;
> halves estimator variance, free; speck-alpha 2.04 -> 1.91).
> ELIMINATED WITH NUMBERS: fine near-surface refine (-25% grain but
> +64% cost, rejected as a trade); the fine erosion band (2.13 vs 2.04,
> not the carrier); surface-centred sampling (2.39, worse - the
> deterministic comb returns); interior turbulence 0.42->0.15 (1.99,
> noise). Queue after Blueprint 1: multi-layer atmosphere (the top
> FIDELITY item), perf part 2 (Nubis3 light grid + adaptive march res).


> **v0.1257.0 (2026-09-01): cloud cost pass part 1, and the ATMOSPHERIC
> LAYERS answer.** Operator at sub-1 FPS on max settings. The sun
> ladder outnumbers view samples 12:1 and was doing two kinds of pure
> waste, both now removed: (1) SLAB SKIP - the geometric ladder reaches
> ~125 km while the cloud band is ~12 km, so far taps paid a FULL
> cluster evaluation (3x3 search + 20-lobe build) to learn there is no
> cloud in empty stratosphere (the view march clips to the slab; the
> sun ladder never did). Skip, not break - a low sun re-enters along a
> shallow chord. Physically exact. (2) COARSE SUN CLUSTER - sun taps
> built all 20 lobes and then had the result smoothed by the 260 m sun
> rind, which discards that detail BY DESIGN; lobes are placed
> largest-first so the sun now unions the first 6 only. The eye keeps
> all 20 - silhouettes are never coarsened. MEASURED: closeup 74.5 ->
> 59.7 ms (-20%), full res 256 -> 184.5 ms (-28%), look unchanged.
> NEXT ON PERF (part 2, still needed - this is not enough): the Nubis3
> amortized summed-density light grid, already scoped in the v0.1252.2
> workflow output at ~40% march savings plus long-range inter-cloud
> shadows; then adaptive march resolution keyed on shell screen
> coverage (full res is cheap from space, brutal inside the deck).
> THE OPERATOR ASKED whether real ATMOSPHERIC LAYERS would help the
> clouds. YES, and it is now the TOP FIDELITY ITEM above further
> single-deck polish: a sky of isolated puffs at ONE altitude cannot
> read as a real sky however good each puff is. Real skies are almost
> always multi-layer - low cumulus/stratocumulus, mid altocumulus and
> altostratus, high cirrus - and the layering (different heights,
> different characters, different lighting, one seen THROUGH another)
> is most of what the eye uses to judge a sky. The genus archetypes
> already exist; what is missing is independent DECKS at their own
> altitudes rather than one band that picks a genus.


> **v0.1256.0 (2026-09-01): THE PRIMITIVE WAS A BALL - per-cloud shape
> frame.** Operator: "How can we make these clouds incredibly less
> spherical... still just giant cotton balls of slightly varying
> shape." Root cause, finally named: every lobe is length(p-c)-r, a
> literal SPHERE. Domain warp, displacement, erosion and the smooth
> union all DECORATE a round object - which is why twenty fidelity
> increments never cured the cotton-ball reading. cloud_v2_body now
> transforms the QUERY POINT into a per-cloud shape frame before the
> cluster SDF: rotate into a wind direction shared across 8-cell
> patches (free cloud STREETS), divide by shape axes (vertical squash
> 0.42-0.88 from genus aspect, along-wind stretch 1.0-1.5, cross-wind
> the reciprocal so the three multiply to 1 - volume and areal
> coverage untouched), then scale the distance back by the smallest
> axis so it stays a conservative bound for the march leap. Placement
> AND lobes live in the frame, so a cloud stretches as one object, not
> a string of stretched beads. Cost measured near-zero (12 vs 13.3
> FPS). LOCKSTEP by design: applied in the FIELD, never inside
> cv2_cloud_sdf, so the CPU twin and its 8 tests stay exactly valid.
> NEXT, in order: (1) PERFORMANCE - at full res the march IS the frame
> budget (4-5 FPS); a cost pass (step-count ceiling, empty-space
> skipping, adaptive res) is what makes Half/Full usable, and it is
> now the top item. (2) Coverage continuity across the pre-volumetric
> to volumetric handoff. (3) Blueprint 1 (bake histogram matching) -
> still the one MEASURED suspect left for the rosette. (4) Blueprint 2
> remainder: size-span widening (V2) and the giants tier.


> **v0.1255.0 (2026-09-01): CLOUD RESOLUTION IS A SETTING - the operator
> question answered, and it was the biggest untried lever.** Yes: the
> cloud layer really is lower resolution than the surface. The march has
> always run at a QUARTER of screen resolution (one cloud sample per 4x4
> screen pixels) while terrain renders full-res beside it - that IS the
> "solid pixels" look and the "100% transparent or opaque" edges. New
> cloud_res_div (4 quarter / 2 half / 1 full) in the F10 panel and via
> showcase {"cloud_res"}. The march footprint now derives from
> ndc_step.y * tanf (its own rasterized Nyquist rate) instead of a
> hardcoded *4.0: bit-identical at quarter res, and it makes a
> resolution raise buy real FIELD DETAIL rather than a sharper upsample.
> MEASURED at the closeup vantage: 13.3 / 6.3 / 2.0 FPS for quarter /
> half / full; half renders visible cauliflower turret structure where
> quarter renders blobs. HALF IS THE SWEET SPOT pending a perf pass.
> ALSO SETTLED: the sun-cone spiral is INNOCENT of the rosette - polar
> decomposition of the direct-sun bisect, cone ON vs OFF, is identical
> within content noise (ring 10.77 vs 10.01, spoke 13.73 vs 13.50). The
> rosette is STILL UNCAUGHT after clearing every renderer mechanism;
> the measured mip-response drift (Blueprint 1, histogram matching) is
> the remaining named suspect and the next increment. THEN Blueprint 2
> (field variety) for the cotton-ball shapes. PERF NOTE for the
> resolution work: at 13 FPS quarter-res the march is already the frame
> budget - a cost pass (step-count cap, empty-space skipping) would let
> half res be the default.


> **v0.1254.4 (2026-08-31): the F10 cloud dev panel + THE MEASURED
> CALIBRATION BLUEPRINT.** F10 now opens a Cloud Dev panel (GUI-first:
> dither toggle, temporal toggle, bisect-channel selector) driving the
> SAME gui_state fields the showcase pins write - one source of truth.
> THE WORKFLOW MEASURED THE AGATE (it ran coverage_vs_mip +
> carve_consistency live): areal coverage is mip-INVARIANT at overcast
> thresholds - the arcs are carve RESPONSE drift (residual DOUBLING
> per rung from mip 3; up to ~15% relative near threshold) + the
> trilinear inter-mip sigma dip (5-13%, what the lod dither grinds
> into static). EIGHT uncompensated taps enumerated - the CELL tap
> (5 mips deep, feeds LWP luminance) is the prime agate candidate; the
> weather-map mips have NO renorm at all; the sun-march taps ride the
> whole uncompensated chain deepest. NEXT INCREMENT (Blueprint 1, the
> agate cure): replace the linear per-mip renorm in cloud_noise.rs
> renormalize_level with per-mip HISTOGRAM MATCHING (256-bin CDF LUT
> onto level 0) - drift-free at EVERY threshold by construction and
> cheaper than the current renorm; then re-fit the carve width table
> with the harness; acceptance = flower-nadir with dither OFF shows NO
> arcs, and both dithers retire. THEN Blueprint 2 (the cotton-ball
> cure): field variety - size spread, base-altitude scatter, wind
> stretch/streets (kills the perspective rush + the operator's
> "spherical cotton balls"). Full blueprints:
> %TEMP% claude tasks/w8911qxqs.output. Edge translucency ("100%
> transparent or opaque at edges") rides the same calibration.

> **v0.1254.2 (2026-08-31): THE FADE BAND WAS THE WARP - crescents +
> sheet-to-snowflakes, one root.** Operator on v0.1254.1 ("huge
> improvement!"): two remaining asks - the white sheet devolving into
> snowflake specks on approach, and the residual "rosette" warping a
> FIXED-distance cloud (eaten centers, C-shapes). Rig bisect chain:
> crescents reproduced over the Sahara at 2 km; SDF-leap-off left them
> STANDING (leap innocent, restored); fade-off dissolved them into
> solid masses at identical FPS (CONVICTED). The v2-to-noise
> representation handoff is distance-keyed and its band interferes:
> mid-fade = crescents, far end = the noise sheet collapsing into
> discrete v2 clouds. SHIPPED: CLOUD_V2_FADE_LO/HI 1.9/2.0 -> 3.9/4.0
> - the whole flying band is pure constructed bodies and the morph
> happens where a cloud subtends ~a pixel (invisible by scale
> separation). NOT infinity: at orbital footprints sub-footprint lobes
> would point-sample as speckle; the carve-hinge noise body stays the
> correct coarse representation. Verified: crescent vantage solid at
> 15 FPS; space disc granular banks at 17 FPS. WATCH: far-range
> coverage calibration across the now-distant handoff (occupancy
> growth cap) if an approach still reads a density pop; the
> low-alt/orbit handoff observation from the operator ("voxel clouds
> swap to shader cover") should now also be re-judged - it was this
> same band.

> **v0.1254.0 (2026-08-31): THE OPERATOR'S EXPERIMENT SETTLES IT -
> frozen jitters + the gray-sun gate.** The operator ran the
> cloud_temporal live toggle and delivered the decisive result:
> temporal on/off changes only SOFT static vs SHARP static, and LOW
> quality (the direct shell path - one smooth unjittered sample per
> screen pixel) is the ONLY clean tier. Verdict: the TV static was IN
> THE INPUT all along - three frame-advancing white-noise jitters
> (subpixel ray, depth, lod dither) re-rolled every frame on a
> quarter-res grid, which no accumulator can average under motion and
> which reference titles (NMS, Elite, Helldivers) simply do not do.
> SHIPPED: all three jitters FROZEN to static per-pixel hashes -
> spatial dither survives (rings stay dissolved; flower-nadir guards),
> but a parked frame is now pixel-identical to the last: no fizz, no
> film-grain crawl, no blinking clouds. DO NOT reintroduce a frame
> term without re-running the on/off experiment. ALSO: the permanent
> gray sun after surfacing = sun_cloud_alpha crossing the DRAWN shell
> (~51 km) instead of the density band top (~12 km) - a cloud 10 km
> below the sightline dimmed the disc anywhere in the 6-51 km window;
> now gated on the composite frame's own band top. Shipped without a
> rig sweep (operator game running, one-GPU rule; operator verifying
> live). WATCH: any banding/ring return at nadir (the frozen dither
> keeps spatial decorrelation, but the temporal averaging of residual
> pattern is gone); the low-orbit swap ("voxel clouds disappear and
> swap to the shader cover on approach" - the operator's handoff
> observation, needs its own look).

> **v0.1252.8 (2026-08-31): MERGED-CAP CONSTRUCTION - change 4 landed,
> the arc's numbers.** The structural half of the field-coherence
> rebuild: r_hi 0.44*width, r_lo 0.11 (bounded by r_hi - flat genera
> inverted the pareto clamp, the CPU twin caught it as a panic where
> WGSL would have silently saturated), budding 0.45-0.62, relative
> smin floor 0.5*mean_r (cap 340 m). CPU twin mirrored SAME COMMIT,
> unifying two pre-existing drifts (twin r_lo 0.05 vs shader 0.06;
> twin blend_m unclamped). NEW PERMANENT HARNESS:
> projected_fill_fraction_report in cloud_primitives.rs (24 clouds x 4
> genera) - cv2_fill_frac now carries MEASURED fills 0.807/0.810/
> 0.381/0.849; re-run + re-paste after ANY lobe-construction change.
> MEASURED: speck-sun 1.12 -> 0.36 (below the pre-arc alpha floor);
> closeup 1.01 -> 0.69. ARC TOTALS (one night): closeup grain 2.71 ->
> 0.69 (-75%), sun channel 2.57 -> 0.36 (-86%), plus the orchard-rows
> root cause and the dot-lattice diagnosis. RESIDUAL, the last named
> carrier: dark speckle on cloud faces = view-path fine texture +
> remaining shading terms; next instrument round = a VIEW-PATH TEXTURE
> BISECT (showcase pins disabling erosion bands one at a time, same
> map_diag pattern). Then: small-disc dynamic march resolution;
> terminator vantage; sunset grazing twinkle re-check.

> **v0.1252.7 (2026-08-31): THE ORCHARD ROWS - the starburst-at-the-
> feet ROOT CAUSE, found and fixed.** The operator's oldest complaint
> ("the starburst at my feet", persisting through the map retirement,
> the reprojection rewrite, and the whole lighting overhaul) is the
> cloud PLACEMENT LATTICE seen from above: straight rows of clouds on
> the 1.1 km brick grid project as SPOKES THROUGH THE NADIR (the
> orchard-from-a-drone effect) - view-locked at any altitude, stronger
> with height, content-side so no renderer change could touch it.
> Photographed on the rig at 41 km midday (rows converging at the
> crosshair), killed with CLOUD_V2_ROW_WANDER 0.28 (per-row smooth
> sine, wavelength ~9 cells, random phase per row - no straight line
> of centres in any direction; adjacent clouds shift together so the
> v0.1232 clumping-cost lesson holds; y-only, inside the 3x3 search
> budget). Checked-innocent on the way: godray behind-camera guard,
> the three ray builders' tanf/aspect/ndc consistency, the water
> sky-mirror. OPERATOR-VERIFY next flight: the down-look at altitude
> that always showed the starburst. Remaining queue unchanged:
> merged-cap construction (change 4, fenced, spec in the v0.1252.6
> block), small-disc dynamic march resolution, terminator vantage for
> the low-sun check, the sunset grazing twinkle.

> **v0.1252.6 (2026-08-31): THE DOT LATTICE named and half-killed - the
> field-coherence shading wave.** The bisect instrument photographed
> the truth: the direct-sun channel over cumulus is a LATTICE OF
> DISCRETE DOTS, one per constructed-body lobe - per-lobe cap-vs-
> crevice contrast 4.7-11x where physics allows 1.1-1.35x below the
> ~300 m radiative-smoothing scale (Marshak 1995). THAT is the
> operator's "TV static"/"sandblast"/"atoms of spheres" across 15
> releases, and near the nadir it composes into the melted flower. A
> 3-agent workflow audited the chain (sun-tau carries 85-95%; the
> v0.1252.4 profile-all change had the sun marching the BARE lobe
> cluster) and designed 5 ordered changes. SHIPPED (1,2,3,5): sun
> marches the envelope with a 260 m ramp (CLOUD_V2_SUN_SMOOTH_M) + no
> warp in profile mode; cavity-on-direct compressed to 1.15x on the
> built path (cav_dir_w); hybrid-band ring neutralizer (ring_off);
> stale jitter comment fixed. MEASURED: full closeup grain 2.59 ->
> 1.01 (61%, the largest single improvement of the arc; everything
> before combined was ~5%); the central cumulus renders as a solid
> luminous mass for the first time. FENCED NEXT (change 4, atomic):
> MERGED-CAP CONSTRUCTION - r_hi 0.34->0.44*width, sep 0.45-0.62,
> RELATIVE smin floor 0.5*mean_r (cap 340 m), fill table RE-MEASURED
> via the projection harness (never estimated), CPU-twin mirror in
> src/renderer/cloud_primitives.rs + lib tests IN THE SAME COMMIT,
> CLOUD_BODY_TOP p99 re-checked. Full acceptance protocol G1-G6 (incl.
> the connected-bright-components contiguity metric and the
> anti-cardboard joint gate) in the workflow output:
> %TEMP%/claude tasks/w84w6pbni.output. TUNING KNOBS if the operator
> reads peak whites as washed: SUN_SMOOTH_M toward 200, cav_dir_w 0.12
> toward 0.2. WATCH: low-sun whole-cloud shadowing (add a terminator
> vantage); shaded-side speckle persists until change 4.

> **v0.1252.5 (2026-08-31): the budget stride reverted - it re-created
> the melted flower at night.** Operator night captures on v0.1252.4:
> giant dark melted-agate flower inside the deck at 2-7 FPS. The
> v0.1252.4 every-step budget stride was the creator: with the slab
> exit hundreds of km away (always, inside the deck),
> max(dt, remaining/left) forced KM strides from the FIRST sample,
> overriding the MFP/SDF refinement - and coarse near-steps are the
> v0.1241 melted-flower mechanism verbatim. REPLACED with final-step
> tail integration: only iteration 224 stretches to cover the
> remaining segment as ONE coarse sample (footprint self-selects a
> deep mip); near sampling untouched; truncation bias still bounded.
> NEW permanent vantage night-flower (5.7 km, rain, post-sunset)
> guards the night/inside-deck state no daytime vantage covered - the
> fix capture there: black, soft, structureless, 21 FPS. LESSON (for
> the incident book): a budget clamp must NEVER touch near samples;
> and every march change needs a NIGHT + inside-deck verification, not
> just the daytime ladder. OPEN from the operator's report: the
> grazing-sun TWINKLE (bright spots return when raising height or
> nearing the sun - direct-channel, sun-angle-dependent, watch after
> this fix); the faint 198 km night tail rosette.

> **v0.1252.4 (2026-08-31): the rest-state fixes - four symptoms, four
> mechanisms, all closed.** Operator on v0.1252.3, the most diagnostic
> report of the arc: (a) faint rosette LOCKED TO THE CURSOR while the
> planet turns; (b) "1900s film dust" crawl + clouds BLINKING in/out
> ONLY when parked; (c) moving = clean (the inversion!); (d) white
> sparkle on shadowed faces. Mechanisms: (a) = TWO stacked causes, both
> closed - the radial history-stretch limit cycle about the approach
> epipole (fix: sustained-zoom alpha escalation to 0.95) AND the
> iteration-cap truncation boundary, a function of slant = screen
> radius from the aim point (fix: BUDGET-AWARE STRIDE - when
> iterations run low the stride grows to cover the segment instead of
> truncating; the grazing iteration-cap tail is CLOSED). (b)+(c) = the
> variance clip's fingerprint: parked, deep blend converges but
> clamping history into each frame's noisy box re-injects noise as a
> slow random walk; ghosts require motion, so gamma now widens to 3
> sigma at rest, tightening to 1 under motion (fix: MOTION-ADAPTIVE
> CLIP GAMMA). (d) = the detailed first-two sun taps letting
> single-pixel full sun through dark faces (fix: sun-profile for ALL
> taps; speck-sun grain 2.57 -> 1.58, near the alpha floor).
> flower-nadir's down-look now renders with NO rings/petals/fibers -
> first time in the arc. Remaining fenced bosses unchanged: small-disc
> dynamic march resolution; erosion coherence + Nubis3 light grid.

> **v0.1252.3 (2026-08-30): the dark-cloud clue - the alpha-edge alias
> found by derivation audit.** Operator on v0.1252.2: "static always
> present, even when the clouds are dark" (dusk silhouettes, orange
> sparkle rims). Lighting-independent grain = the carrier is ALPHA at
> edges. Three fixes: (1) CR NEIGHBOURHOOD CLAMP in the composite - the
> v0.1251 Catmull-Rom's negative lobes were SHARPENING unconverged
> march noise into full-screen dots during fast flight (the operator's
> low-orbit pepper); clamped to the 2x2 texel min/max, standard
> practice. (2) ABSOLUTE-sigma gate on the resolve spatial filter - the
> relative-only test under-engaged on bright decks. (3) THE BIG ONE:
> CLOUD_V2_INT_LODC -7.9 -> -9.56. Every v2 tap's lod constant follows
> log2(tile/256) within 0.1 mip EXCEPT the interior turbulence, which
> matched no derivation and sampled 3.2x finer than the footprint -
> view-path aliasing wherever alpha does not saturate: silhouette
> sparkle, thin skirts, down-look pinholes. TWO FENCED BOSSES REMAIN
> (the honest residuals): (a) SMALL-DISC RESOLUTION - the quarter-res
> march gives a 150 px planet disc ~37 cloud samples across (the one
> thing the retired octa map did better); needs dynamic march
> resolution keyed on shell screen coverage. (b) EROSION COHERENCE -
> gaps/gap-edges in the field are near-binary at meter scale where real
> cloud edges are translucent over tens of meters; the field-character
> rebuild (with the Nubis3 summed-density light grid as its companion).

> **v0.1252.2 (2026-08-30): the sun-profile cutover - reference-grade
> light-march smoothing, workflow-designed.** A 3-agent workflow
> (mechanism audit + reference survey + fix design) turned the bisect
> verdict into four landed changes, all shader-only: (1) SUN-PROFILE
> MODE - far sun taps (i>=2) read the constructed body with its sub-MFP
> fields at their means (interior turbulence, fine displacement, Worley
> erosion skipped; g_sun_profile flag) - the audit's #1 carrier was the
> interior turbulence field (~4 m content via the frozen g_v2_disp_lod)
> point-sampled by 200-400 m segments, delta_tau 1.5-4 rms = the direct
> coin flip; Nubis3's first-two-taps-per-pixel rule verbatim, and a
> perf win (4 fetches saved per far tap). (2) CELL-FREE tau_vert
> envelope (second ALU-only hinge; the 20.8 m cell voxels were the
> ambient residual). (3) HZD LIGHT CONE - far taps spiral laterally
> (K=0.12 of distance, golden angle, frame-advanced phase): the line
> integral becomes the area integral lateral scattering physically
> performs. (4) NUBIS-2017 RELAXED-BEER floor (0.7*ph_wide*exp(-0.25
> tau)) - deep-shadow contrast capped at 0.25x plain Beer; the one
> LOOK-affecting change (shadow faces lift 2-4x at tau 6-16; tune
> CLOUD_SUN_RELAX down if washed). MEASUREMENT CAVEAT discovered: the
> weather field phase is BOOT-DEPENDENT, so cross-sweep crop metrics
> are content-confounded; only within-boot channel ratios are valid
> (sun/alpha ratio 3.31 -> ~1.6-2.4 across states). RIG WANT (logged):
> a weather-phase pin for deterministic content across boots.
> RESIDUAL + ENDGAME: the irreducible ladder floor scales with ext_km *
> pixel_pitch; if the operator's eyes still read static, the fenced
> architectural answer is the Nubis3 amortized summed-density light
> grid (256x256x32, 8-frame amortization, first-two-taps-per-pixel;
> ~40% march cost SAVINGS + long-range inter-cloud shadows).

> **v0.1252 (2026-08-30): the stipple forensics - alpha is innocent, the
> LIGHTING carries the grain.** Operator (on v0.1251.1, confirming "way
> better" overall): the close-cloud TV-static/sandblast remains the
> target. NEW INSTRUMENT: the screen-path channel bisect (showcase
> map_diag 1/2/3 renders coverage-alpha / direct-sun / ambient as
> grayscale; vantages speck-alpha/sun/amb + motion-closeup). VERDICT:
> alpha grain 0.78 (SMOOTH - the density field is not the carrier),
> direct sun 2.57 (THE carrier), ambient 1.39. Mechanism: alpha
> SATURATES away fine density structure, lighting is LINEAR in it.
> Shipped: variance-adaptive spatial filter in the resolve; ambient
> cavity AO damped (multiple scattering fills crevices); tau_vert from
> the pre-erosion carve envelope (ambient channel 1.39 -> 0.89). IGN
> dither REVERTED - structured error survives a mean filter as a
> halftone weave; jitter spectrum must match the filter kernel.
> REMAINING (the next increment, workflow-assisted): direct-sun
> self-shadow structure at the 22 m light-mfp scale (sigma 45/km, the
> per-pixel tau of the 2-tap sun ladder + the steep octave response) -
> real contrast real clouds smooth via LATERAL multiple scattering.
> Candidates: multi-tap sun cone at coarser lod, lateral diffusion term,
> response softening, sigma-vs-octave rebalance. Full-image same-pose
> grain 2.71 -> 2.57 so far; the operator wants a step change, not 5%.

> **v0.1251 (2026-08-30): spin-aware reprojection + the static's true
> mechanism.** Operator on the v0.1249 exe: clouds "uncanny valley low
> detail... like TV static", atmosphere lower-detail than the surface.
> Both reads were RIGHT. (1) The resolve's motion floor read the planet's
> spin sweep as camera motion (the planet-local delta folds spin into
> translation) and pinned alpha at 0.6+ for every non-co-rotating camera,
> effectively switching the temporal filter OFF - the whole-disc static
> IS the raw jittered march. Now the resolve gets the motion SPLIT
> exactly (f64 CPU chain): content rotation as a rigid spin rotation
> applied per pixel to the hit point + the RAW camera translation; the
> alpha floor keys on reprojection RELIABILITY (real zoom, or slides
> past ~8 texels/frame) instead of raw slide. (2) The cloud layer
> literally renders at fraction-res vs full-res terrain - the composite
> now reconstructs the half-res buffer with Catmull-Rom (same 9-tap the
> map arm used). Identity-fallback preserves old math when the split is
> unavailable. Rig: no regressions across 5 vantages, panics=0, FPS
> flat-to-up (space 19.9). NEXT FIDELITY ITEM (operator's "uncanny"
> verbalized): cumulus interiors read as granular salt-and-pepper
> stipple, not coherent cauliflower lobes - the fine erosion bands carve
> micro-cavities at their noise floor; needs a fidelity-expert pass on
> erosion coherence (NOT a sampling bug; unchanged by this increment).

> **v0.1250 (2026-08-30): ONE RENDERER - the octa map retired.** The
> operator refuted the v0.1249 rosette kill on their machine (third failed
> kill claim on this artifact: v0.1237, v0.1245+, v0.1249 - every rig
> verification passed under rig conditions and failed under theirs), plus
> new damage: DARK gray-blue daytime sky under the deck (the v0.1248
> near-over-map change backdropped stale/aerial map content over the WHOLE
> sky), the hurricane-eye ownership circle at 2.6 km, and a flight-wobble
> report (no flight code changed in 20 releases - the deck swims at 4-5
> FPS and since v0.1243 correctly rolls with attitude; verify, don't
> dismiss). ARCHITECTURE VERDICT: every view the operator has praised is
> the per-pixel near march; every artifact class they hate is the map or
> one of its seams; at disc views the map re-marched ~1M texels/frame -
> the same budget as a half-res screen march spent mostly off-screen. SO:
> near_mix pinned 1.0 (lib.rs), octa_runs pinned false (mod.rs, texture
> stays zeroed), the 32 km screen-march ownership leash removed
> (45-cloud-temporal), the composite's distance-ramp key + near_has gate
> removed (near owns every pixel it touched; empty map backdrop = no-op
> OVER). Map machinery kept dormant in-tree. RISK held to the ladder: the
> round-3 white-veil note said the screen march integrates sub-grid
> structure to featureless white at disc ranges - but the compact-support
> carve hinge has since made clear footprints exactly clear at coarse
> mips; judged on captures, not assumed. Next if veil confirmed:
> fractional-coverage extinction at coarse footprints (the flight-sim
> technique), NOT a map revival.

> **v0.1248 (2026-08-30): NEAR-OVER-MAP + the two-field diagnosis.** The
> operator's 8-shot ladder pinned the disease: the near arm and the map
> render DIFFERENT skies, and the composite's mix() REPLACED map content
> wherever near claimed - every stitch line was an artifact (blue halos
> punching to raw sky at thin near edges, a clear hole under the camera
> ringed by map deck, inverted blobs on the ceiling). Composite is now
> premultiplied NEAR-OVER-MAP: the map is the backdrop everywhere, near
> refines on top, thin near reveals map never sky; cost = bounded double
> density where both drew the same cloud. The v0.1247 sun-drift floor is
> REMOVED (it perpetually re-noised the map = the surviving checkerboard;
> the diff-driven alpha already handles lighting change; resume-drop
> kept). NEW OPERATOR-CONFIRMED FACTS: the ROSETTE is visible from DEEP
> SPACE instantly (map CONTENT bias, not temporal - reproduce: from-space
> park + cloudmap dump, no flight needed); the gravity-well handoff SNAPS
> the view basis (ship well -> Earth well, camera transition bug - OWN
> ITEM); surfacing from underwater visibly changes the sky (the
> resume-drop after underwater octa idle - correct but visible).
> ENDGAME (dedicated increment, discuss with operator): ONE cloud
> representation with continuous LOD - the near arm as REFINEMENT of the
> map's field rather than a second field; every seam class dies at once.

> **v0.1247 hotfix (2026-08-30): dump crash + checker quilt; the rosette
> hunt's state.** The v0.1246 cloudmap dump panicked the operator's live
> session (octa textures lacked COPY_SRC; the first fix hit the wrong
> create site - mod.rs/lib.rs pristine regions are CRLF, edited regions
> LF: use the Edit tool there and grep-verify every instrument after
> writing). The v0.1246 sun-delta invalidation pulsed alpha every ~7 s on
> the 20-minute day = the diagonal checker quilt; now a continuous floor
> capped 0.25. TWO DIAGNOSTIC CYCLES WERE PHANTOMS: "the compositor never
> runs" (the probe had never survived to disk) and "the map is empty"
> (the rig park had silently collapsed from 112 km to 0.3 km before the
> dump - an OLD latent probe-hold drift over 40-60 s, exposed by settle
> 75; suspect the sweep autopilot holding a stale travel target - OWN RIG
> BUG, diagnose before any long-settle forensics). With a held park the
> map dump shows healthy full-disc content; [CloudArm]/[CloudGate]/
> [CloudPasses] 1 Hz instruments are permanent. THE ROSETTE (operator's
> persistent nadir starburst, present at [CloudReproj] 0.000): map
> machinery proven healthy end to end; next discriminator is OPERATOR-
> SIDE - drop debug/cloudmap_request.json while the rosette is on screen
> (safe as of v0.1247); fibres in the dump = content bias (march-side
> hunt: the octa jitter kernel / footprint at the anchor), clean dump =
> sampling side (composite Catmull-Rom / decode).

> **v0.1246 (2026-08-29): the three-part conviction - frozen map, sentinel
> death spiral, over-tight cut.** (1) The octa pass was CPU-skipped at
> near_mix==1.0 (below ~30 km) while the per-pixel composite still showed
> the map in the whole horizon band: the operator's night-bright band WAS
> frozen daylight (proven: every lit march term is ~0 at night). Dispatch
> now ORs regime 3; resume-after-freeze floors EMA to 1; sun-delta
> invalidation exists at all now. (2) PARKED DOES NOT EXIST here: the
> 20-minute day sweeps content 37 km/s past a world-frame hover (5.3
> km/frame at 7 FPS); the LEVEL-triggered sentinel fired every frame
> (cadence suspended -> 16.7M marches -> the 7 FPS itself, self-locking)
> and the v0.1245 6-texel cut amputated accumulation at the sweep's 24
> texels -> per-frame point-sample = the rosette repainted forever.
> Sentinel now EDGE-triggered (spike vs level); cut raised to 48; the
> cadence-skip branch (which advected with NO bound) gets the same cut.
> Frame-locked parks measure [CloudReproj] 0.000 - why the rig was always
> clean. (3) Resolve clip moments bilinear (12-px block plateaus);
> composite key 2x2 weight-blended; under_deck units fixed (was 16.8 km,
> not the slab base - masked by the dispatch skip); rank-1 octa temporal
> jitter -> R2 pair. NEW INSTRUMENTS: debug/cloudmap_request.json dumps
> the octa map itself (rgb|alpha double-wide PNG - the content-vs-
> sampling discriminator), night-horizon standing vantage. DEFERRED:
> grazing iteration-cap tail (obliquity-scaled refine floor at
> 40-clouds.wgsl:2750), extent-rim fade (composite e.z hard cut -> smooth
> + guard-ring march), composite Catmull-Rom has no mip at minification
> (sparkle at orbit).

> **v0.1245 (2026-08-29): flight-smear hard cut + occluded-map cadence gate
> + rig descent knob. READ THE HONESTY NOTE (superseded by v0.1246 above -
> the cut was over-tight and the sentinel analysis incomplete).** The operator's radial
> starburst converges at the MOTION EPIPOLE (they arrive by FTL flight; the
> map history reprojects along the per-frame delta; sustained descent
> displaces every fetch radially about the nadir, and the old floors kept
> 65 percent of it per step). Fixed: hard per-texel history cut past 6
> texels of shift (45-cloud-temporal octa path), map full-rate cadence only
> while the map is the VISIBLE renderer (near_mix rides light7_color.x,
> offset 320 - the in-layer 3 FPS was 16.7M occluded texels marching every
> frame, and low FPS is itself why the accumulator stayed static), and
> camera_request {"descend_mps":N} + the standing descent-live vantage
> (sustained flight on the rig at last; note the frame-lock compensation:
> the ANCHOR must sink with the pinned camera below the co-rotate ceiling).
> HONESTY: the rig's achievable descent (~10 km/s effective) stays under
> the cut's 6-texel threshold, so the A/B could not photograph the
> operator's 30-100-texel regime - the cut shipped on mechanism + safety
> (it only fires where reprojection is geometrically meaningless; parked
> states verified untouched). IF THE OPERATOR STILL SEES FIBRES after
> v0.1245: next suspects, in order - (a) the REGIME-2 full-sphere map
> (inside the slab, extent = pi, anchor zenith: the antipode at the FEET
> has the projection's true stretch singularity - the operator's "dome"
> instinct; consider splitting regime 2 into two hemispheric windows or
> anchoring at the horizon), (b) the near arm's resolve under 3 FPS
> motion (same epipolar logic, cloud_resolve.wgsl has the shift_tx cut
> already - verify its threshold), (c) capture their live run.log DURING
> a sighting ([CloudRegime] + a new shift_tx histogram instrument).

> **v0.1244 (2026-08-29, in verification): the per-pixel regime split - the
> "missing tech" for the sheet-to-ballpit transition.** The operator's
> persistent down-look starburst was the near march's footprint cap
> (min(screen*4, map_texel)) forcing ~5x-above-Nyquist sampling at long
> slant - undersampling moire, radial on a down look; every altitude-band
> move just relocated it (74 km -> 36 km). Both fixed at the root:
> footprint = the near grid's own Nyquist (screen*4 alone), and the global
> altitude crossfade is REPLACED by a per-pixel key: cloud_march_core gains
> g_march_max_km (screen path 34 km - rays abstain pre-step beyond it, far
> end clamped; kills the both-whales 3 FPS), cloud_composite gains binding
> 5 = march_dist and keys each pixel (near owns content < 20 km, map > 32
> km, claim requires drawn alpha so clear foreground cannot blank distant
> banks), near_mix reduced to an arming gate (px ramp x 45..60 km ceiling).
> Handoff is now per CONTENT at matched apparent scale (the MSFS-style
> continuous LOD). Verify on the blend ladder + flower + starburst +
> marble vantages before ship; watch for (a) near/map representation
> disagreement at the 20-32 km seam, (b) thin-near-over-far-sheet pixels
> (accepted edge case, documented in cloud_composite.wgsl).

> **CLOUDS, state as of v0.1239 (2026-08-29).**
>
> **The operator's starburst-at-the-feet: REPRODUCED AND ROOT-CAUSED (the
> flown camera).** Why every fix "did nothing": the rig TELEPORTS (camera ~30 m
> from the ship-frame origin), the operator FLIES, and with no floating-origin
> rebase the whole journey accumulates in the f32 camera.position (~3.6e7 m,
> ulp 4 m). The new ipc knob `far_frame_km` re-splits the same absolute pose
> onto the rig (vantage `starburst-far` = starburst-repro + far_frame_km
> 36000), and it reproduced the operator's artifact on the first try: murky
> whole-frame veil + cardinal speckle cross at the nadir, while the teleported
> twin rendered normal clouds. THE DOMINANT CAUSE was not even in the shaders:
> `dist = render_off.length()` in the celestial draw loop measured the planet
> from the SHIP-FRAME ORIGIN, not the camera - [CloudRegime] read px=280
> mix=0.00 at 4.3 km altitude, i.e. the ORBITAL far-map cloud regime and a
> starved planet LOD rendered from inside the cloud layer. Fixed: dist =
> (render_off - camera.position).length() in f64; heals px, LOD level,
> visual_scale floor, chunk activation, near_mix, and the atmosphere gate at
> once. Two real f32-lattice sites fixed in the same pass: the cloud motion
> delta (was f32 subtraction at 3.6e7 - fed light4, resolve prev_dpos, motion
> gates; now f64 end to end via DVec3 cloud_prev_cam_local) and the resolve's
> big-form reprojection (now small-form normalize(rd*t_w - prev_dpos)).
> `starburst-far` is a STANDING vantage now - any flown-state regression shows
> up on a teleporting rig. The architectural cure for the whole defect class
> is a FLOATING-ORIGIN REBASE (periodically fold camera.position back into
> ship_world_pos); that is the logged follow-up, not this increment.
>
> **The earlier map-path rosette (v0.1237) was real but separate:**
> quasi-periodic jitter hashes + never-jittered map directions; fixed with a
> PCG hash + sub-texel direction jitter. Faint residue is partly the field's
> REAL east-west stagger anisotropy - re-run the parked bisect before chasing
> it as aliasing. The motion-gate fixes (v0.1235 epipole term, v0.1236 motion
> floor) stay, but they were never this artifact.
>
> **v0.1242 (2026-08-29): the melted flower + sphere-atoms, both cured.**
> Operator on v0.1241.1 still saw a crosshair-centred marble/flower (1.9-134
> km, persisting at hover) and "atoms made of spheres". Critic-led workflow
> refuted temporal feedback (hover co-rotates at every reported altitude;
> the clip box is built from a history-free march buffer); the iteration-
> count diagnostic proved the rings are STEP-COUNT ISOLINES - the march
> jittered the sample inside each step but the step LADDER was one
> deterministic comb anchored at m0, so the integer count staircases in
> screen radius and each tread prints a ring on a flat deck. Fixed with
> LADDER-PHASE jitter (first step advances by the jittered fraction) -
> rings gone same-vantage (flower-nadir, now standing). Sphere-atoms:
> the 2026-08-25 eyeball fix had stripped ALL surface erosion from built
> clouds; restored as Nubis-class ONE-SIDED WORLEY EROSION in the DISTANCE
> domain (moves the surface, cannot ring it; 20-160 m octaves from
> cloud_detail_tex, height-phased, edge-proximity strength; stride margin
> grown by the carve). Closeup verdict: carved fractal silhouettes, cost
> neutral. Orbit FPS (operator 5-8): the honest px now runs the near march
> at planetary slant ranges (~200M samples/frame); near_mix gains an
> ALTITUDE FADE (full below 40 km, octa map owns above 80 km; derived from
> cam_r_ratio so the px=280 origin-distance bug cannot return). Also:
> px_hash + g_lod_jitter moved to PCG (last two hash21 users on this path)
> and the CLOUD_V2_FADE_HI handoff dithered.
>
> **Cloud perf follow-up (small):** on fully-built samples (cs.v2 ~ 1) the
> four density-space erosion band taps in 40-clouds.wgsl:1977-2052 are
> computed and discarded - gate them on cs.v2 < 0.999 to reclaim 4 texture
> taps per sample. Deferred from v0.1242 to keep that increment visual-only.
>
> **v0.1243 (2026-08-29): blend-band streaks, roll misregistration, sun
> bleed - fixed; two handoffs logged.** The 74.4 km radial streaks were the
> crossfade band (mix=0.14): the near arm is ~5.5x above Nyquist for the
> field its footprint cap targets at planetary slant (moire = radial
> combing), AND the cloud ray basis ignored camera ROLL and mode
> transitions (origin audit #19: forward()/right() vs the rendered
> rolled_up view matrix - the whole cloud layer twisted about the
> crosshair whenever the camera rolled; all three consumers now extract
> the view-matrix rows). Crossfade lowered to 22..42 km. LONG-TERM
> (better design, critic-endorsed): key the blend PER-PIXEL on the near
> arm's own first-hit dist_km (MRT loc 1) instead of a global altitude
> proxy - fixes down-look AND horizon cases and lets near rays abstain
> (perf) in one move; needs a composite binding for march_dist (bind-
> group discipline: count entries at EVERY create site). Sun-through-
> clouds: god rays sample the SUNWARD deck crossing now (frame_lock::
> sun_cloud_alpha - max of the pinned procedural field via cloud_
> reference::weather_pinned_field and the live grid) and the type-17
> disc/halo intensities scale by exp(-4a) per frame. STILL OPEN, handed
> to the ABYSSAL rung-2b owner (their lane files): the ocean sun-
> specular ignores clouds - multiply (1 - 0.9*ca) into sun_shadow_f at
> 90-fragment-main.wgsl:1697 (type-12 glint, hardcoded 1.0), :1670
> (old_glint), :478 and :746 (type-16), using the v0.898 ground-shadow
> pattern at :1608-1617 but at the fragment-to-sun deck crossing
> (r = sea radius * CLOUD_SHELL_SCALE). ALSO LOGGED (own increment):
> the aboard-station frame family from the origin audit - celestial/
> godray sun direction not hull-rotated (#17, lib.rs:17762 area), body/
> cloud/atmo rotation spin-only (#18, lib.rs:8948), orbit rings world-
> pinned (#27) - lighting rotates off the visible sun as the hull turns.
>
> **RE-SCOPED after v0.1243: map clipping mostly healed; one residual.**
> The razor-wall clipping was the twisted ray basis (audit #19, fixed) -
> the mismatched rays fell outside the map extent. AFTER the fix,
> marble-inertial (sweep 20260829-071407) shows natural weather-field
> edges everywhere except ONE small triangular cloud fragment with two
> straight edges mid-frame - the signature of an octa-FOLD seam leak
> (reflected/clipped content where map taps cross the octahedral fold
> without proper wrap: Catmull-Rom taps or the sub-texel direction
> jitter). Diagnose with a map-UV/seam visualization at that vantage
> before tuning.
>
> **Old text for context (superseded):** Evidence: marble-inertial capture, sweep
> 20260829-060201 - at 112 km with mix=0.00 (the new altitude fade), cloud
> patches render coherent but end in hard straight edges, all facing the
> same direction. The 12c extent controller's regime-1 window (asin(rt/c)
> + 4 deg + drift, ~92 deg at 112 km) SHOULD cover the disc, so the cut is
> not obviously the cone rim - diagnose with a map-texel visualization
> before tuning anything. This corner (map as SOLE renderer at 40-200 km)
> was never exercised before: pre-v0.1239 the px bug gave it wrong extents,
> post-v0.1239 the near march always covered it. The operator hovers in
> exactly this band.
>
> **Swiss-cheese sheets: fixed.** The v0.1234 union scaled the field density by
> sheet_w, pushing it under the visibility threshold at partial coverage - holes.
> Now mix(built, max(built, body), sheet_w): the sheet keeps full density, the
> UNION is what fades.
>
> **STILL OPEN, the last big look item: clouds read as sphere clusters up
> close.** Placement, coverage, detail mips, temporal artifacts are all fixed or
> gated - the remaining problem is the LOBE SHAPE itself. Next levers, in order:
> (1) stronger domain warp relative to lobe radius (CLOUD_V2_WARP_FRAC 0.42,
> tile 1.7r - try 0.6/1.3 with an A/B on cumulus-closeup-ultra); (2) flatten the
> lobe primitive into a base-weighted ellipsoid so buds read as risen dough
> rather than marbles; (3) only then relight (the smin normal groundwork from
> v0.1232.2 is computed and unused).

> **ACTIVE (parallel lane): THE ABYSSAL ADOPTION ARC (operator, 2026-08-28:
> "Let's do all of it").** Full technical menu, constants and porting cautions:
> `docs/reference/abyssal-ocean-weather.md` (commit 942f1921; source repo
> github.com/Token-Gremlin/natural-disasters, MIT). This arc runs in the
> `ocean-weather` lane (`data/coordination/lanes.json`, carved out of engine)
> so it never collides with the cloud session's files (40/41/45-clouds.wgsl,
> `src/renderer/cloud_*.rs`). Rungs, STRICT order, each verified before the
> next; per-event tuning numbers go in `data/weather/events.ron`
> (infinite-of-x), never hardcoded:
>
> 1. **DONE v0.1238.0 - Ocean event field core (CPU, f64)**:
>    `src/terrain/ocean_events.rs` - the four analytic disaster fields
>    (tsunami soliton with asymmetric shoaling + drawdown, rogue Gerstner
>    group, Rankine vortex + swirl-coord rotation, hurricane eyewall ring +
>    glassy eye) in event-local tangent frames, vec4 uniform packing, 18
>    tests pinning every adopted constant.
> 2. **GEOMETRY HALF DONE v0.1240.0 - WGSL twin + geometry**: event height
>    displaces the drawn sea (CameraUniforms 14-row tail block at offset
>    672, layout-test-pinned); buoyancy rides it (rogue = envelope only);
>    WGSL constant-scanner lockstep test; dev pin showcase
>    {"ocean_event":kind, ocean_event_bearing, ocean_event_distance};
>    probe-proven with close-range captures (tsunami ridge + drawdown,
>    maelstrom bowl; hurricane confirms the pow-negative-base WGSL fix).
>    SHADING DONE v0.1241.0 (rung 2b): tsunami breaking-lip foam
>    (lacework-carved, face-biased), saturating vortex shear foam, rogue
>    crest band, hurricane glassy-eye chop+foam suppression - all analytic
>    at the fragment's planet-model position, no new varyings, buoyancy
>    untouched. REMAINING (rung 2c): swirl-coord advection of the wave
>    lookup - touches the HEIGHT path, so it needs the CPU twin + lockstep
>    extension and the swirl clock (ocean_event row 12.w, reserved). Note
>    for 2c+3: dev-pin amplitudes are clamped to the +-12 m patch band
>    (MAX_SEA_HEIGHT_M); full 34 m walls + full-strength maelstrom foam
>    (the 0.62 shear cap engages near strength 34) need the lifecycle to
>    publish dynamic patch bounds.
> 3. **Lifecycle + gameplay**: event params data-driven in
>    `data/weather/events.ron`; spawn/ramp/decay through `weather_events.rs`;
>    REGISTER `DisasterSystem` (written, never registered); damage + HUD; the
>    float clamp rides the wall via the rung-2 twin.
> 4. **Lightning**: CPU midpoint-displacement bolts (forks p=0.42, return
>    strokes amp 0.62^i with flicker) + instanced ribbon draw; two strongest
>    bolts flash sea/cloud/sky through shared light slots; thunderstorm event
>    emits. (No weather audio exists engine-wide; thunder logged, not built.)
> 5. **Waterspout / tornado funnel**: raymarched analytic funnel on the Vortex
>    event core (rotating-frame detail noise, dual-HG forward phase).
> 6. **Foam v2** in `ocean_fft.rs`: add the steepness criterion (Stokes H/L
>    1/7 + leeward bias + crest gate - catches the spilling breakers the
>    Jacobian misses), bubbles channel; re-verify the Monahan histogram.
> 7. **Crest spray**: `particles_gpu` spawn-from-breaking (candidates roll
>    against the FFT foam/crest field), forward-scatter puff shading.
> 8. **Rain overhaul**: closed-form vertex-shader rain, sub-pixel streak
>    energy conservation (vThin), fbm squall curtains, rain rings on water.
> 9. **Water shading**: backlit crest SSS with the event-thinness gate,
>    mss-to-roughness LOD, sun-disc-widened glint, wind-frame Langmuir foam.
> 10. **Cloud env probe in water reflections** - CROSSOVER rung, touches cloud
>     files; schedule WITH the cloud session when its current arc lands.
> 11. **GPU compute FFT + horizontal chop** (the long-planned water-fft
>     increment 4, using the 8-fields-in-4-complex-IFFTs layout as reference).
>     Riskiest renderer change, deliberately last.
>
> Cloud cherry-picks (erosion bite curves, local-density powder alternative,
> per-pixel-depth reprojection) belong to the CLOUD session's own plan, not
> this arc - they are listed in the reference doc Tier 3 for it to read.

> **CLOUD COVERAGE: RESOLVED (v0.1234).** Asked 0.95, delivered 1.00 from nadir;
> `node scripts/cloud-coverage-metrics.mjs <sweep>` over `overcast-nadir-ultra`
> PASSES. The winning mechanism was the SHEET UNION in cloud_carve: overcast is
> a continuous stratiform layer, so past sky-wide coverage 0.6 the noise field
> is unioned back under the per-cell constructed clusters until the sky closes
> near 0.9. Gate it on the GLOBAL coverage (material.base_color.a), never the
> local weather alpha - the local value is 1.0 inside any cloud at any coverage
> and closes the whole sky. Growth is capped at 1.35x and the smooth-min blend
> radius at 300 m (uncapped, coverage-grown clouds were giant melted-wax blobs).
>
> **THE DETAIL MIP IS PER SAMPLE (v0.1234), keep it that way.** The per-ray
> freeze at the segment midpoint surfaced a cloud 500 m away at the mip of a
> point 300 km downrange - the NEAREST clouds were the smoothest. g_v2_disp_lod
> is now set from each view sample's own footprint just before the density
> call; the eight sun taps that follow reuse it, which is the eye/sun surface
> consistency the freeze existed for.
>
> **THE SHELF (base-tangent seam): visually gone, numerically open.** Repro
> found: inside the layer ~3 km, cover 0.55, level view - the re-authored
> `base-horizon-seam-{3p0,3p7}km` vantages. The gate still measures a 1.93x
> detail step (threshold 1.25) and stays RED rather than being tuned to pass;
> part of the step is genuine depth (crowded translucent cloud above the line
> reads softer), part may remain artifact. Judge against the capture, not the
> ratio alone, before spending on it again.

> **MEASUREMENT LESSONS from this arc. Four wrong answers were produced
> confidently before being caught; each is now enforced in a script.**
>
> 1. **Areal coverage must be measured from NADIR.** From a grazing camera the
>    same A/B read 46% with and without a change, because a sparse field fills
>    the frame near the horizon. `cloud-coverage-metrics.mjs` refuses to score a
>    vantage whose `look_offset_deg` is not 0.
> 2. **Classify cloud on SATURATION, not brightness.** Calibrated on real
>    pixels: cloud 0.05-0.07, sand 0.36, sky 0.39 - while luminance overlaps all
>    three (cloud 163-224, sand 191, sky 170). A brightness threshold reported
>    86% cloud in a frame that was mostly sand.
> 3. **A gate whose subject is not in frame is not a gate.** The first seam
>    vantages had no cloud at the seam row and reported before and after
>    identical. The guard added to catch that was then itself fooled by pale
>    horizon haze passing the cloud-fraction test, producing a confident 6.6x
>    "detail step" that was haze against cloud. Looking at the image caught it;
>    the number never would have. Both guards now sit in
>    `cloud-seam-metrics.mjs`, which currently REFUSES to score and exits 2.
> 4. **Do not judge across non-adjacent captures by eye.** A change that
>    "clearly" enlarged the clouds measured at 6.6% of pixels differing - the
>    comparison being made was against a remembered older frame, not the actual
>    previous one.

> **WORKFLOW: WGSL edits need NO cargo build.** The megashader assembles from
> on-disk parts when they exist (`shader_loader::assembled_pbr_source_from_dir`;
> the embedded copy is the stripped-install fallback) and probe-sweep junctions
> the repo `assets/` into the rig. Edit the shader, run the sweep. A whole
> session was spent paying four-minute rebuilds per shader iteration.

> **SHELL: use a QUOTED heredoc for commit messages.** `<<MSGEOF` interpolates,
> so backticks in prose get command-substituted and words vanish from the
> message (v0.1232.6 lost the word it was defining). Write the message with
> `<<'MSGEOF'` and put the version in with sed afterwards.


> **CLOUDS: THE THREE OPEN ITEMS (v0.1232.3).** Everything below was measured,
> not guessed. Read the findings before reattempting either fix - both are
> written and deliberately switched off, so the work is not the code.
>
> **1. Snowflakes from orbit.** Operator: "a ton of white dots appear
> everywhere... like snow flakes." Cause is understood: the v0.1230 power-law
> made most clouds a few hundred metres, so from orbit each lands on about one
> pixel, and a sub-pixel bright object cannot be filtered - only twinkled. The
> fix shape is right (fade the built body back to the noise body across a 250 m
> to 1 km footprint window, a mip fade) and IS WRITTEN in
> `CLOUD_V2_FADE_LO/HI`, currently neutralised at 1.9/2.0.
>
> **BLOCKER, and this is the real task: the two body models are not
> brightness-matched.** The noise body renders darker, so fading toward it with
> distance darkened the far half of every frame. Measured at
> `cumulus-closeup-ultra`, mean grey over a fixed crop: **191.1 fade off, 157.4
> fade on**, and still 157 with the shading term that was first blamed removed
> entirely. Match the two bodies at the handover, THEN reopen the window. Do
> not tune the window; it is not the window.
>
> **2. Clouds read as opaque fluff, not cloud.** The smooth-min normal is now
> COMPUTED and nearly free (the smin already derives the blend factor h that
> combines the distances; the same h combines the normals - one normalize per
> lobe, no extra field evaluations, against three to six full re-evaluations
> for finite differences). It yields the sky-facing cosine and the seam
> strength 4h(1-h) that peaks in the crevices between buds.
>
> **BLOCKER: wiring it into `ao` turned every cloud into a dark silhouette**,
> and three retunes failed to recover. The diagnosis: the normal is only
> meaningful within a rind of the surface - deep inside a body the gradient
> direction is arbitrary, and interior samples carry most of the accumulated
> weight, so an occlusion built from it is applied hardest exactly where it
> means least. Next attempt: weight by surface proximity (`g_v2_sdf_m` is
> already published) and apply to the AMBIENT only, never to direct - a
> sky-view term is by definition about the sky.
>
> **3. The horizon seam.** Operator screenshots at 5.7 and 6.2 km show a hard
> horizontal line across the frame with visibly different cloud rendering above
> and below it, plus one cloud "indented on the right side". NOT yet
> investigated. The standing theory from the earlier arc is a uniformly-capped
> slab top; the new evidence suggests looking instead at the cloud BASE shell
> horizon, which from 6 km sits about 280 km away and projects as a near-
> straight line at exactly that screen height, and at what changes
> discontinuously in the marched segment as a ray stops intersecting the lower
> slab boundary.
>
> **Method note for this arc.** Perf on this rig has ~1.6x run-to-run variance
> (the same unchanged frame measured 37 ms once and ~60 ms three times), so a
> single sample is not evidence. Take repeated or back-to-back measurements
> before quoting a number; the 137.7-vs-59.8 jitter comparison is trustworthy
> because it was back-to-back.


> **ACTIVE: THE CLOUD PLAN (from the v0.1228 decision).** The operator, out of
> patience: "I am really tired of seeing these spheres with zero transparency
> and TV static effect... I don't get why we can't get rid of this."
>
> **SHIPPED v0.1228.0 (increments 0 + 1).** Ultra never survived a restart (the
> loader whitelist omitted it, and the next save overwrote the choice), so every
> recent cloud change was reviewed on a renderer the operator's game reverted,
> and what they were describing was the older noise path. And the near-field
> temporal denoiser accelerated its own blend rate in proportion to
> disagreement with no motion gate - positive feedback against noise, so it
> switched itself off at exactly the pixels it existed to fix, even at rest.
> Both fixed, both zero frame cost.
>
> **NEXT, in order. Do not reorder without reading why.**
>
> **Inc 2. Make the sun see the surface the eye sees.** The comment at
> `41-cloud-bodies.wgsl:351-360` claims all eight sun-shadow taps sample the
> displaced surface. They do not: the displacement mip comes from the caller's
> `lodb`, and `cloud_sun_tau` passes a different `lod_t` per tap
> (`40-clouds.wgsl:2029`), so near taps land where the displacement is gone.
> The silhouette is bumpy while the LIGHTING still shades a smooth sphere.
> Very likely why the v0.1221 displacement work "did nothing". ~0 ms.
>
> **Inc 3. Kill the coin flip: step by distance, not a fixed hop.** Clear air is
> marched in 495 m hops (`slab_h * CLOUD_STEP_BAND_FRAC`, 11 km slab x 0.045)
> while the cloud edge it is hunting is 90 m thick, so every silhouette pixel is
> a per-frame coin flip. Note the v0.1218 refinement `max(seg/16, 30 m)` does
> NOT help from the ground: `seg` is the whole slab crossing, so seg/16 = 690 m
> and the `min` always picks 495 m. It was verified from inside the deck, where
> it does work. `cv2_cloud_sdf` already returns a real distance in metres and we
> throw it away - use it, in the hoisted per-cell form
> (`environment-program.md:769-779`), NOT the naive per-sample form that gave
> 4 fps at v0.1210. Gate at `gpu.cloud_screen <= 4 ms`, abandon if it misses.
>
> **Inc 4. Demote the sphere from surface to envelope.** This is the "spheres
> with zero transparency" complaint. Density is `clamp(-best / 90 m, 0, 1)`, a
> linear ramp off a distance field, which leaves 79-93% of every lobe at
> constant full opacity with only a 90 m soft shell - measured per archetype.
> Displacement is 9-26% of lobe radius, far too small to disguise a sphere. The
> 14-lobe cluster should decide WHERE a cloud is and its proportions (its flat
> condensation base is genuinely good, keep it) while multi-octave noise carves
> the surface, with a vector domain warp applied BEFORE the lobe reduction so
> shapes can fold and overhang instead of merely getting bumpy. Same increment
> fixes plain defects found in the audit: cloud width is drawn UNIFORMLY
> (`:134`) and should follow a power law; lobe count is hardcoded to 14 (`:46`)
> against 6-48 in `data/clouds/archetypes.ron`; `cv2_arch_index` (`:152-162`)
> can never return 1, so **cumulus congestus has never once been rendered**;
> and placement is not wind-advected while the coverage gating it is, so clouds
> pop in fully formed instead of fading.
>
> **Inc 5. Fix the interior and the light.** Shade on the analytic smooth-min
> normal (the lobe loop can accumulate it free; named "the designed cure" in
> four journal entries and never built). Restore crown as sky-view and pouch as
> a crevice mask. Delete Beer-powder (`CLOUD_POWDER_STRENGTH = 0.92`): droplets
> scatter essentially all the light they receive, so a cloud edge physically
> cannot be darker than the sky behind it, and ours measured 0.71x. Add the
> adiabatic vertical water gradient and a turbulent interior field at 50-500 m.
>
> **Explicitly NOT the plan: a voxel-atlas / full Nubis-3 rebuild.** More famous,
> but this project has no artists and no offline fluid pipeline, and
> `environment-program.md:775` already names it the fallback rather than the plan.
>
> **Clouds should NOT become particles.** Camera-facing cards are right for a
> bounded, short-lived puff (a smoke grenade) and wrong for a deck to the
> horizon: ~10,000 km2 of cloud is a quarter-million sorted blended quads, they
> lose parallax the moment you fly into them (the "oriented to me" complaint,
> already fixed once), and they cannot report absorption along a ray, which is
> what dims the sun disc and casts cloud shadow on the ground. The 2030 version
> of a smoke grenade is a small dense voxel grid marched against the SAME
> scattering model - so if we ever want one, feed a local volume into the cloud
> march rather than building a second billboard system beside it.


> **LESSON (v0.1227): counting frame-conversion sites is not the same as
> finding them.** v0.1225 converted the world into the station hull frame and
> the comments proudly labelled the consumers "site 1 of 3" through "3 of 3".
> There were four. `src/renderer/stars.rs` builds its sky rotation from the
> camera forward/up, which while riding are hull-frame vectors, so the sun and
> Earth swept past correctly and the stars stayed nailed to the deck. The
> release note even asserted the opposite.
>
> What would have caught it: the missed site had no textual link to the
> others - it does not mention the station, the hull, or the frame, so no grep
> for those words finds it. The reliable sweep is to enumerate every consumer
> of a CAMERA-derived direction or its own camera uniform, not every mention of
> the frame. When converting a frame, list the renderer subsystems that hold
> their own view matrix (stars, godrays, particles, any post pass) and rule
> each in or out explicitly.
>
> Still unconverted, and known: moon fill and godrays (deferred, see the
> v0.1225 list below).

> **ATTRIBUTIONS ARE NOW A SURFACE (v0.1227).** `data/credits.ron` +
> Settings > Credits + `LICENSES.md`. **Adding any real-world data source
> means adding a row in the same commit** - `src/credits.rs` has a test that
> fails if a source marked `attribution_required` names no surface showing its
> notice. OpenStreetMap is the one with teeth: ODbL treats the RENDERED view
> as a Produced Work needing a visible notice wherever it is drawn (Maps
> footer + in-world HUD line, both from `credits::OSM_NOTICE`), AND the region
> files as a Derivative Database that must itself be offered under ODbL. A
> credit in the repo alone does not discharge the first.


> **SHIPPED v0.1225.0: the homestead gets a day.** The station now propagates
> real Keplerian elements from `data/stations/home.ron` on the GAME clock, with
> a nadir-pointing (LVLH) attitude. Full reasoning in the release message and
> `src/station/orbit.rs`. The one number worth carrying forward: **orbital
> position cannot light anything** - the sun is 1 AU away, so a whole
> synchronous orbit moves the sun direction by 0.03 degrees. Only attitude can.
> Gate: `scripts/home-clock-metrics.mjs` over the `home-clock-*` vantage trio.
>
> **NOTICED WHILE VERIFYING, not yet chased: solar generation aboard looks
> inverted.** In the A/B captures the HUD read `gen 1948W` at local midnight
> aboard and `gen 150W` at local noon aboard. That is the wrong way round, and
> the likely cause is that `SolarSystem` scales panel output by the GLOBAL
> game hour (a lon-0 ground site) while the station now has its own sun angle
> from its attitude. The two clocks disagreed before this change too; the
> change just made it visible. The fix is presumably to drive aboard-station
> panels from the same hull sun vector the renderer uses
> (`station_world_rot.inverse() * sun_dir`) rather than from the wall-clock
> hour. Verify the inversion first - it was read off a HUD in two captures
> taken at different day counts, which is suggestive, not proof.
>
> **Deferred from this increment, in order:**
> 1. **Eclipse / umbra for hull geometry.** The homestead never enters Earth's
>    shadow. `sun_gate` is hardcoded 1.0 for everything but the planet-surface
>    branch (`90-fragment-main.wgsl`), and `lit_uniform`
>    (`renderer/mod.rs:2235-2237`) stamps a flat 2.5 intensity over the
>    celestial pass's day-gated value. Night aboard is currently "the sun is
>    behind the hull", not "the planet is between us and the sun". Both are
>    needed for a real orbital night.
> 2. **Moon fill and godrays through the hull frame.** Three sites were
>    converted (celestial `render_off`, `sun_dir`, local up); these two were
>    not, so they still reason in world axes aboard.
> 3. **Rotation-aware particle rebase** (`lib.rs` floating-origin rebase
>    handles translation only).
> 4. **The cosmos ephemeris is still on `SystemTime::now()`.** Same class of
>    bug as the one just fixed, one level up: the planets' own positions do
>    not follow the game clock either. Nobody has reported it because the
>    drift is slow, but the hour slider does not move them.
> 5. **Rate-limited attitude slew.** Changing attitude mode re-points the
>    station instantly; a real one would slew on RCS over minutes.
> 6. **Web mirror of the station card** - orbit, attitude and next sunrise read
>    from the same `data/stations/home.ron`.


> **OPEN (2026-08-26, from live play on v0.1223.1) - two operator reports,
> neither reproduced yet. Read this before re-deriving either.**
>
> **1. "Glowing ocean" at night.** Screenshot at 01:05 local: a large soft
> pale-cyan mass over an otherwise correctly dark planet, with dark holes in
> it and two small isolated cells nearby. Their run.log for that session puts
> the camera at **alt=399.8 km with `[CloudRegime] mix=1.00`**, i.e. the NEAR
> screen-march cloud regime at full strength. So despite the name, the prime
> suspect is CLOUD, not water: `CLOUD_NIGHT_FLOOR = 0.006` is added UNGATED at
> three sites in 40-clouds.wgsl (the `day` factor multiplies the sun and
> ambient terms but not the floor), which on the night side leaves every cloud
> sample at a flat `base_color * 0.006` - a shadeless pale mass following the
> coverage field, holes and all. NEXT STEP: an orbital night vantage over a
> cloudy region at ~400 km, then A/B the floor at 0.
>
> **RULED OUT: the water path.** Chased first and disproven, do not redo it.
> The real defect found there is genuine but is NOT this: the sky-view LUT
> cannot represent a deep-night sun at all - its sun-elevation axis spans
> `mu_s in [-0.15, 1.0]` and both samplers clamp below that (atmo_luts
> `u_to_mu`, and the `(mu_s + 0.15) / 1.15` clamps in sky_view_lut.wgsl), so
> past about 8.6 degrees under the horizon it keeps returning civil-twilight
> radiance. The drawn sky and the celestial pass both multiply that away with
> `celestial_sun_day`; the two consumers of `water_sky_lut` (the water mirror
> and `sky_ambient`) never did. A day-gated version was built and A/B captured
> at ocean-night-glow: **both the gated and ungated builds render the night sea
> black**, so at 150 m this changes nothing visible and cannot be the report.
> It was REVERTED rather than shipped, because gating `sky_ambient` removes the
> night ambient fill from all terrain and props and would darken night scenes
> while a black-screen report (below) is open. Worth doing later on its own
> merits, scoped to the water mirror only, with its own evidence.
>
> The A/B pair is kept as `ocean-night-glow` + `ocean-noon-control` in
> vantages.json. They are a matched pair on purpose: night alone cannot fail
> honestly, since a gate that zeroed the mirror at every hour would also pass
> it. The noon twin is what proves a gate is time-dependent rather than off.
>
> **2. "Solid black floor, no homestead, no Earth/planets."** Reported
> immediately after the ocean report, same v0.1223.1 session. NOT reproduced
> and not diagnosable from disk: their run.log ends in a clean shutdown at
> 07:12:59 with no later boot, so there is no failing process or crash trace to
> read. The session it belongs to was at 400 km orbit with terrain and water
> both drawing (`[WaterDiag] draws=1024 covered=true`). Needs a repro from the
> operator: which build, and what they did just before it went black.


> **ACTIVE (2026-08-21): the ENVIRONMENT PROGRAM** - the council plan of
> record at docs/design/environment-program.md, executed serially by rank.
> Done through v0.1184.0: increments 7 (ocean specular AA), 8 (reference
> arbiter + joint gate), 9 (sampling law), 10a/10b (integrator + field
> polarity), 11a/11b (fades deleted + weather fractions), THE MIRROR BUG
> (v0.1183), and 12c SLICE A (extent-parametrized temporal map, resample
> re-anchors, arm everywhere, atmo-order fix - A/B-proven vs a v0.1183.1
> control; adversarial review's 5 findings fixed pre-ship).
> **SHIPPED v0.1186.0 (slice B): translation reprojection** - the
> operator's motion smear ("solitaire artifact") killed via per-texel
> history reprojection with an analytic shell-sphere parallax distance,
> a PLANET-LOCAL motion baseline (the world-frame one slides 1.3-2.1 km
> per frame at a PARKED camera - measured), a >15 deg teleport guard,
> and motion-adaptive blend. Parked captures crisp, delta ~0 at rest;
> the MOTION verdict is the operator's. Rig unblocked: unconditional
> re-park + 6 s settle in probe-sweep.js (first with-time request of a
> boot lands ~8 h early; engine ordering fix owed, chip task).
>
> **SHIPPED v0.1198.0: THE VANISH ROOT CAUSE + the 12d two-regime
> architecture.** The vanish was NOT the wx-floor handoff: cloud_carve
> divided by (1 - thr) while the body tops at CLOUD_BODY_TOP = 0.79,
> capping cores at carve ~0.68 (typical 0.2-0.4); the four erosion
> bands - calibrated for carve-1 cores - ground the entire from-below
> deck to ZERO (stage forensics at pinned coverage 1.0: pre-erosion
> carve max 0.23, post-erosion 0.000 on every sky ray). Fix: divide by
> (CLOUD_BODY_TOP - thr), the contract the constant's own comment
> documents. Plus 12d: NEAR regime (>= 1000 px) = half-res fullscreen
> per-pixel march with analytic pad-basis rays + screen reprojection
> (no direction cache -> the whole solitaire/ghost family structurally
> impossible near the planet); FAR keeps the octa map. Verified across
> 7 vantages, panics=0; cov100-underdeck is the permanent regression
> gate (coverage 1.0 = no legitimate gap, any blue zenith = defect).
>
> **SHIPPED v0.1199.0 (12e): the march/resolve split** - the operator's
> first-flight verdict on 12d ("still ghosting... way faster to
> disappear but still present" + "clouds look a lot like static, best
> on the cliff-like edge") traced to 12d's single blend constant, which
> cannot both converge the jittered march and kill stale history. 12e =
> quarter-res full-rate subpixel-jittered march + a standalone resolve
> with VARIANCE-CLIPPED reprojected history at base alpha 0.12: ghosts
> snap in one frame, static converges ~8 frames deep. Measured hf-noise
> -69% under-deck / -53% mid-alt at unchanged march cost. Adversarial
> review caught + fixed pre-ship: the history-drop flag was coupled to
> the octa cadence sentinel (~8-11 m/frame) and would have dropped
> accumulation every frame of ordinary fast flight; it now fires only
> on true teleports (delta > 0.25 x slab distance).
>
> **SHIPPED v0.1201.0 (12f): cloud underside relief.** The flat
> ceiling was arithmetic (every lighting term saturated at overcast
> tau + a constant warm bounce at 57-63% of base radiance, chroma sign
> inverted). Landed: LWP mottle field (solidity-gated density
> multiplier from existing taps), transmittance-scaled near-neutral
> bounce, vertical-tau split for the diffusion floor + CIE solar term,
> pouch shading. All four executable gates pass
> (scripts/cloud-underside-metrics.mjs): mottle 1.26x -> 1.91x, chroma
> sign corrected, gradation preserved, coverage unbroken. Tried and
> REVERTED: thr -> 0 at cov 1.0 (fills the slab vertically - scud).
>
> **SHIPPED v0.1208.0: procedural placement default + continuous mip
> dither; THE WHITE-CONTINENT CASE CLOSED** - it was a continent-sized
> STRATUS cell in the planet-fixed cloud-family field, frozen over
> North America because every rig boot re-seeds the same game minute.
> Every path A/B had compared different REGIONS. Rig-methodology
> lessons journaled (v0.1208 entry): pin the cloud TYPE when hunting
> coverage differences; same-region or it proves nothing.
>
> **SHIPPED v0.1214.0: THE APPROACH VANISH, ROOT-CAUSED AND CLOSED.**
> The atmosphere DOME was painting over the composited deck. The 12c
> order rule composited clouds BEFORE the transparent pass whenever the
> camera sat outside the atmosphere; over the disc that dome alpha is
> near-opaque, so it erased them. Measured: the composite wrote 1.2% of
> the disc under the old order, 99.9% under the new one, with every
> discard sentinel reading ZERO (drawn, then overdrawn). Ladder is now
> smooth 20,000 km -> 900 km (was a 60.9% -> 8.8% cliff). Physically
> correct too: the march already applies aerial perspective, so the old
> order applied the air column twice. It masqueraded as a terrain bug
> because the cliff sat exactly at the chunked-terrain trigger (1.5
> planet radii), which flips the shell lists and hence this ordering.
> ALSO: the Ultra eyeball rings - the v2 body is a DISTANCE field, so a
> footprint-derived rind is a metric radius and the 8-tap sun ladder
> shaded eight shrunken copies of each lobe; the rind is now frozen
> once per ray.
>
> **NEXT (TOP): operator verdict on v0.1214.1** (approach continuity
> orbit-to-ground, Ultra lobes ring-free), then by rank:
> 0. THE HORIZON LINE (operator, still open): a dead-straight line
>    through the whole cloud layer seen from inside/above the deck,
>    gone once underneath. Needs a NATURAL-weather repro - pinning
>    coverage 1.0 just buries the camera in dense cloud. Side finding
>    deserving its own item: inside a pinned-100% deck at noon the view
>    renders near-BLACK, where real in-cloud is bright white fog.
> **NEXT (TOP): operator verdict on v0.1208.1** (procedural-only sky
> feel, static-square death, approach continuity), then by rank:
> 1. THE SPHERE-BALL LOOK + per-lobe shelving (operator: "obviously
>    all balls/spheres... decimated spheres" + "weird shelving") - the
>    puff/cell lobe construction reads as uniform ball packs at close
>    range and the march's step ladder terraces each lobe. This is the
>    Ultra v2 constructed-body track (increments 14/15) pulled forward:
>    real cloud bodies are not sphere unions.
> 2. Stratus mesoscale structure: a 3,000 km featureless stratus sheet
>    is family-correct but visually dead - the increment-15 statistical
>    far field gives sheets their real broken texture.
> 3. Deck underside polish continues (chroma gate flaked warm under
>    changed weather - re-measure under pinned weather before chasing).
> 4. Black horizon hairline (chord-sag suspect), water F6/F4 A/Bs,
>    stars-below-cloud-top, cloud streets (13).
>
> (Superseded round, kept for the record: THE FAR/NEAR HANDOFF POP -
> with a live contradiction that turned out to be the regime field.) The operator's orbit-approach "huge patch of clouds
> just vanishes" is the regime switch (analyst: 9.4x footprint jump =
> +3.2 mips in ONE frame, carve compensator saturated at its 0.02 cap).
> BUT the derived fix (near march footprint = min(screen*4,
> cloud_pix_ang_map())) produced a WHITE CONTINENT at 4,500-6,700 km
> while the OCTA at a near-identical mip (2.8 vs 2.3) renders ~45%
> areal: HALF A MIP CANNOT DOUBLE COVERAGE, so footprint is NOT the
> whole octa-vs-near difference (v0.1204 journal has the full data).
> Suspects for the residual difference: the composite's Catmull-Rom of
> a 4096 map at a ~250 px disc (severe minification averaging), the 12e
> resolve's variance clip, the weather wlod delta, the octa's 2x
> spatial supersample. STEP 0 (instrument first): a 1 Hz [CloudRegime]
> log line in lib.rs printing px + near + altitude - THREE sweeps were
> confounded this round by guessing which regime a park ran.
> 2. Overcast-completion veil: engine coverage 1.0 = thr floor 0.347 =
>    ~87% areal; at >= 0.95 coverage add a thin base-level stratus veil
>    that closes the sky AREALLY without filling the slab vertically
>    (the round-5 scud lesson; gate4 recalibrated to 18% meanwhile).
> 3. Black horizon hairline (atmosphere shell chord-sag suspect), water
>    F6/F4 A/Bs, stars-below-cloud-top, cloud streets (13), 14/15.
> Operator watch items: 12e ghosting residue (RESOLVE_CLIP_GAMMA toward
> 0.75), 12f underside taste (LWP mix range; pouch 0.72), the new 1 km
> base feel, edge stipple on cloud silhouettes (march jitter at
> boundaries - if reported, widen the resolve's neighbourhood or add a
> spatial post-filter).
>
> (Superseded live-path protocol, kept for the confound catalog: the
> haze-corrected clouds-on/off design in journal 2026-08-22 - the
> wx-floor fade suspect is now largely moot post-carve-fix but the
> protocol remains the right instrument for any future live-vs-pinned
> coverage dispute.)
>
> (Superseded round below, kept for the confound catalog:)
> **SAME-REGION coverage-invariance measurement.** The
> night's final re-adjudication (environment-program.md 12c, READ IT
> FIRST - it names four instrument confounds that each looked like a
> rendering defect): the 15%/80%/60% triplet spread is dominated by
> WINDOW-SIZE SAMPLING VARIANCE of the ~2000 km-cell pinned field, and
> nothing yet cleanly demonstrates a rendering non-invariance. The
> clean protocol: one boot, orbit + nadir captures within a minute,
> CROP the orbit frame to the nadir frame's exact ground region,
> compare areal coverage in the crop (same field, same moment, same
> region, two footprints); several offsets for the under-deck rung.
> Only a spread surviving THAT is worth hunting. PROVEN NOT A TERM:
> the carve width table (three tables render identically). Rig debt
> blocking pinned captures: a showcase_request carrying cloud_cover
> resets the game clock ~8 h (chip task filed with the repro; send
> the camera request AFTER any showcase pin as the workaround).
> After that: 12c slice B (RG16F first_t + parallax-corrected history),
> descent-ladder re-run on the fixed map, then increments 13+ by rank.
> The ladder gates increments: node scripts/probe-sweep.js --ladder then
> scripts/ladder-score.mjs.
>
> Logged debt: cumulonimbus width capped at 8 km because the v2 cell grid
> (3.2 km, 3x3 neighbourhood) cannot host wider clouds; the permanent fix
> is a coarse cloud-grid tier for storm-scale systems - the cap must not
> silently become the design ceiling.
