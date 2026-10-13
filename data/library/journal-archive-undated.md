# Orchestrator journal archive -- undated

Decisions rotated out of `data/coordination/orchestrator_state.json` (oldest first within each batch; newest overall is in the live journal). Source of truth for "why we did X" once it ages past the live tail. See also git log + the GitHub releases.

## undated

**Decision:** Cycle 7 of overnight loop (v0.646.0): wired ecs::cosmos::body_position_in_system_meters to real Kepler math instead of DVec3::ZERO stub

**Why:** Investigating src/systems/navigation/orbital.rs (backlog candidate) found it dead code (zero callers anywhere) -- superseded by src/cosmos.rs, the already-shipped canonical Sol-system Kepler model (v0.262.8, powers the Maps page + FPS world spawn). While confirming nothing else needed orbital.rs, found an adjacent LIVE stub: ecs::cosmos::body_position_in_system_meters (the Phase-2 cosmos position resolver ContainerRef::Body case, docs/design/cosmos-architecture.md) always returned zero with a comment saying the real math lands in a later phase -- but that math had already shipped separately in src/cosmos.rs, just never wired to the ECS resolver. Wired it for the sol system (the only system with body data today); unknown system/body ids still fall back to zero, documented not panicking. 4 new tests, proven via revert-and-retest (2 of 4 fail against the reverted stub with the exact expected wrong value). No live caller exists outside ecs/cosmos.rs tests yet (Phase 3 Cosmos page / Phase 4 ship containers not built), so no user-visible behavior changed -- this is progress banked for those future phases, and keeps the ECS resolver in sync with the Maps page math instead of drifting. orbital.rs itself left in place, unreferenced but harmless -- deletion is out of scope for tonight's sweep.


## undated

**Decision:** Cycle 8 of overnight loop (v0.647.0, BUG-044): spoiled food had zero gameplay consequence -- fixed EAT handler to check the spoilage side-table

**Why:** Investigated src/systems/food.rs (backlog candidate, plan doc estimated needs a data model + tick logic). Found spoilage tracking itself already fully implemented and correct (per-slot timer, freshness aging, spoiled flag, GC). The real gap was narrower: the EAT handler resolved nutrition purely by item_id from the static NutritionProfile and never consulted the spoilage side-table, so a spoiled item could be eaten with full nutrition and zero risk forever whenever its own raw_consumption_risk was 0 (true for all cooked/canned/preserved food) -- exactly what the TODO comment at the spoiled-flip site described but never implemented. Fix: EAT now finds the eaten items inventory slot, checks self.spoilage for spoiled, and applies 25% nutrition + guaranteed food_poisoning if so. 1 new test, proven via revert-and-retest (fails against the reverted code with the exact expected wrong behavior).


## undated

**Decision:** Cycle 9 of overnight loop (v0.648.0): confirmed learning.rs dead code, implemented Cosmos page Track (continuous body-follow), found + fixed a 4th dead-file doc-pointer (maps.rs), logged Mute Server as an open question

**Why:** src/systems/skills/learning.rs practice-hours Skill::add_practice confirmed dead code (zero callers) -- the real skill system is the XP-based SkillSystem in skills/mod.rs, an entirely different superseding design. A fresh full-repo TODO/stub grep (not just the plan docs original list) found 2 more items: chat.rs Mute Server button has no notification infrastructure to hook into yet (no OS toast/sound, no per-channel unread tracking) so wiring it to a bare flag would be a hollow no-op feature -- logged as a genuine open question rather than force-built. cosmos.rs Track button (a disabled stub) WAS self-contained since cycle 7 already wired the real orbital math it needed -- implemented continuous camera-follow via a new GuiState.cosmos_tracked_body field, toggled/cleared via 2 extracted pure functions (toggle_tracked_body, focus_should_clear_tracking), 4 tests proven via revert-and-retest. Added snapshot_cosmos to ui_snapshots.rs since the page had no headless screenshot coverage before. While investigating Track/Focus, found src/gui/pages/maps.rs (591 lines) is ALSO fully dead code -- GuiPage::Maps has forwarded to cosmos::draw since v0.203.2 -- the 4th instance this session of a superseded file left in place with stale doc pointers (after sky.rs, orbital.rs, learning.rs). Fixed the stale file-path references in FEATURES.md and PAGES.md.


## undated

**Decision:** Cycle 10 of overnight loop (v0.648.1, docs-only): re-audited the larger/riskier stub bucket and found ALL 8 files (11 total incl. submodules) are also dead code, not design-decision-pending

**Why:** The plan docs original filing said these 8 files needed real design decisions rather than a mechanical fill-in. Given the nights pattern hit 4-for-4 on stub-superseded-by-real-implementation (sky.rs, orbital.rs, learning.rs, maps.rs), re-checked every file for external callers instead of trusting the original filing. Result: AutonomySimulator, construction::blueprint::Blueprint (superseded by a DIFFERENT same-named real Blueprint/BlueprintRegistry in construction/mod.rs), CsgBrush/CsgOp, the entire logistics/ tree (LogisticsSystem/Shipment/CargoContainer), the entire navigation/ tree (NavigationSystem/Star/CelestialBody/SurfacePoint -- superseded by the real cosmos.rs + gui/pages/cosmos.rs), FluidSimulation, CollisionHandler, and PsychologySystem/NeedsState (superseded by the already-live Vitals system) all have ZERO external callers anywhere in the codebase. None of these needed a design decision -- unlike SkyRenderer/Mute Server which have genuine product ambiguity, these are just confirmed-dead scaffolding. Left all 11 files in place (not deleted) -- deleting across 6 subsystems in one unattended sweep is a bigger, more visible action than anything else done tonight, so this is documented as a safe future cleanup opportunity (~250 lines) rather than executed unilaterally.


## undated

**Decision:** Cycle 11 of overnight loop (v0.648.2, docs-only): live-verified the WebRTC signaling relay pass-through, closing priority #2s remaining follow-up

**Why:** Cycle 5 verified the stream lifecycle (start/join/leave/chat/stop, fixed BUG-043) but left stream_offer/answer/ice routing as read-as-correct-but-not-live-tested since it seemed to need a real WebRTC peer. Realized the RELAY-SIDE routing (as opposed to the actual WebRTC media handshake) doesnt need a real peer -- just 3 authenticated WS connections. Wrote a dedicated multi-connection Node test script (streamer/viewer/bystander bots) against a fresh local relay: streamer starts a stream, sends an offer to the viewer, viewer answers, streamer sends ICE. Confirmed live: viewer got the offer+ICE with exact payload, streamer got the answer, bystander got NONE of the 3 signaling types (correct unicast, no leakage), the delivered from field was the real connection-authenticated key not the client-supplied one (relay.rs never trusts client-asserted identity -- correct anti-spoof design, confirmed not a gap), and the streamer got no self-echo. This closes the relay-side half of the flagged follow-up; the actual WebRTC media handshake + client scene-management UI remain genuinely unverifiable without a real browser/str0m peer or the live production relay, flagged for the operator rather than attempted against production tonight.


## undated

**Decision:** Cycle 12 of overnight loop (v0.649.0, v0.650.0): self-improvement adversarial-review pass caught and fixed a real regression in this sessions own BUG-044 fix

**Why:** A web/frontend TODO sweep (since all prior cycles were Rust-heavy) turned up only 1 hit -- a Tauri-era dead TODO in shell.js guarded behind window.__TAURI__, never true post-Tauri-deprecation, not worth fixing dead code. Given both explicit priorities and the full stub backlog were closed, dispatched an independent adversarial-review agent (fresh context, no attachment to the code) over the whole nights diff (cb089287..HEAD) across 7 areas before wrapping up, specifically to catch anything that shipped wrong overnight. It found ONE real bug: BUG-044s spoiled-food EAT handler (cycle 8) found the eaten items slot via forward position() search, but Inventory::remove_item actually consumes from the LAST matching slot backward -- a real, reachable mismatch whenever the same item_id occupies two slots (fresh + spoiled), silently defeating the whole fix. Fixed with a matching reverse search + a new multi-slot regression test, proven via revert-and-retest (the original single-slot test could not have caught this -- needed an independently-constructed multi-slot scenario). The other 6 areas (chat role, group voice membership, main-menu health check, viewer_peak, ecs::cosmos AU-to-meters, Cosmos Track toggle) were confirmed correct, no changes needed -- including useful negative-space confirmations. Also fixed, separately, a stale v0.283.0 comment in lib.rs claiming native has no WebRTC stack (it does, shipped v0.485-495), found while cross-referencing STATUS.md for the cycle-11 verification.


## undated

**Decision:** Resolved the SkyRenderer open question (v0.651.0): operator confirmed removal, deleted src/renderer/sky.rs entirely

**Why:** Overnight loop cycle 6 found SkyRenderer had zero external callers and logged it as an open question rather than guessing whether it had a future role. Operator reviewed this morning and confirmed: since the code was already unreachable (never instantiated), there was nothing to disable-and-check-visually -- removing it changes nothing by construction. Deleted the file (346 lines incl. its own 6-test module) and its mod declaration in src/renderer/mod.rs. Weather/WeatherCondition (imported but defined elsewhere) untouched, still live.


## undated

**Decision:** Researched Mute Server design space (no code shipped yet -- this is groundwork for a future decision, presented to the operator as options)

**Why:** Operator brainstormed multiple possible meanings for chat Mute Server (audio, push-notification granularity by @everyone/@here/@role/@username, bandwidth savings via stopping automatic polling) and asked for more nuance. Investigated the REAL current state before designing: delivery is WS-push (broadcast_and_store, relay.rs) with NO polling loop anywhere for chat messages (the only recurring intervals are an unrelated 30s stats counter and a separate P2P group-sync poll) -- so there is no polling to disable for bandwidth savings; the only way muting could plausibly save bandwidth is closing/not-opening the WS connection to a muted server entirely (losing live messages until reconnect) or suppressing outbound typing-indicator traffic (minor). notification_prefs (dm/mentions/tasks/dnd) exists as a real schema+API but is PURELY ROUND-TRIPPED, never enforced -- the relay broadcasts to everyone regardless of these prefs, so today it is a settings-popup with zero delivery effect. Mention detection is real but narrow on web only (chat-ui.js isMentioned(), single @username regex, wired to toast+sound via the browser Notification API + a Web Audio chime) and DOES NOT EXIST on native (no @everyone/@here/@role concept anywhere, zero native mention-detection code). Native has ZERO desktop-notification and ZERO sound-on-message infrastructure at all (only an unrelated in-app update-available toast exists). Per-channel/server unread tracking is real but web-only, purely client-side, non-persistent, rebuilt on every page load; native only has per-DM unread. CONCLUSION presented to operator: before Mute Server can have real nuance (tiered audio/badge/mention-level suppression), native needs the underlying notification primitives built first (desktop toast, sound-on-message, cross-client mention detection incl. @everyone/@here/@role which do not exist yet anywhere) -- Mute Server design and that prerequisite infra build were proposed as a two-phase plan, not yet greenlit by the operator.


## undated

**Decision:** Started the afternoon loop (docs/history/2026-07-01-afternoon-loop-plan.md) at operator request, after fixing BUG-045 (mirrored-home floor/ceiling/trim missing) and shipping the construction-editor sun-angle override (v0.653.0); dispatched a self-sustaining-homestead-design Workflow (3 research agents + 1 synthesis) in parallel

**Why:** Operator asked to enable loop mode to work through everything discussed this session (Studio streaming pipeline, Humanity/Governance/Laws/Donate pass, 4 disconnected-but-valuable systems, economy automation, NPC task-AI), AND separately asked for a dedicated subagent effort on designing a maximally self-sufficient single-occupant homestead (the mission-critical educational baseline: feed/power/water/air/waste for one person using only real, already-shipped or plausibly-authorable game content, with an honest accounting of what cannot be closed-loop at single-home scale). Given the scale of the homestead ask and ultracode being on, used a 2-phase Workflow (parallel research: existing power/water/air/waste systems, existing food/crafting systems, real-world self-sufficiency facts; then one synthesis pass) rather than a single agent, so the design is grounded in both the actual codebase and real homesteading figures, not guessed. Before starting the loop, also fixed two things the operator hit live while testing: BUG-045 (ClonableHomeDesign::bake_local_groups only extracted material_walls, dropping floors/ceilings/trim for every cloned home in a residential zone) and shipped a manual sun-angle override for the construction editor (the real astronomical sun direction is correct but the mothership has no orbital rotation simulated at all -- a fixed GEO-above-Silverdale position set once at init and never updated -- so there was no way to get better lighting; full ship orbital mechanics is a real, separate, larger project already scoped in cosmos-architecture.md, out of reach for a quick fix).


## undated

**Decision:** The self-sustaining-homestead-design Workflow returned: ~90% of a complete single-occupant self-sufficient homestead already exists as real game data. Saved permanently to docs/design/homestead-solo-design.md, phased into the afternoon loop plan (Phase A/B/C)

**Why:** The design synthesis discovered (and this session had not previously known) that docs/design/self-sufficiency.md (written 2026-06-07) already provides a rigorous loop-based self-sufficiency model (energy/water/food/waste/air, Liebig-limiting-factor scoring, the honest light-cap math on why indoor gardens cannot grow all the calories), and that data/machines/home.ron (the existing 3-person seed home) already encodes real quantified values consistent with that model. The design re-derives an exact one-person bill of materials (4 solar panels, 2 battery banks, 1 wind turbine, 1 generator for power; 1 cistern+pump+purifier for water; 9 nutrition towers+1 apothecary+8 potato beds+3 oilseed+2 grain trays+2 mushroom racks+1 aquaponic tank+1 grain field+1 legume field+1 silo for food reaching ~94-100% of one-person caloric need, up from ~50% at 3-person scale -- the pedagogical payoff the operator wanted, the SAME garden closes for 1 person; 1 air recycler; 1-2 composters), citing exact machine/crop/recipe ids already in the data files, with the sizing math shown throughout. It flags 4 genuine small content gaps (no real edible mushroom crop, no tank fish species for the aquaponic B12/omega-3 claim, no per-crop calorie data bridging plants.csv to the food loop, no editable component-output/location table for a future computed self-sufficiency score) and an honest section on what CANNOT be closed at single-home scale (electronics/semiconductor manufacturing, ore-scale metal/alloy production, medicine synthesis, capital-equipment replacement) -- framed explicitly as the pedagogical point, not a shortfall, matching the operators own framing (baseline for 1 human reveals why civilization-scale infrastructure matters). Phased into 3 implementation phases: A (assemble data/machines/home_solo.ron from 100% existing data, no new authoring, highest value/lowest risk), B (author the 4 content gaps), C (build the honest teaching artifacts -- a grow-light-vs-power-budget meter and a what-this-cannot-close Home-page panel, which IS the operators actual stated goal made visible in-app).


## undated

**Why:** target/ had ballooned to 1.1TB (never cleaned across ~859 releases) + 40 agent-worktree target/ dirs at 274GB filled the 2TB drive to 152GB free. Deleted BUILD CACHES ONLY (safe, rebuildable), preserving all source + unmerged worktree commits; added `just clean-heavy` to prevent recurrence. Native header now matches web (wrap + icon/text/both via NavDisplayMode); mobile menu is a full-screen popup with a fixed top-center RGB close button (transform driven by !important inline because some webviews would not apply the class-based transform). Glow download: verify by content (ImageReader::with_guessed_format) not the .tmp file extension.


## undated

**Why:** Operator realized he had no local SSH setup memory (it was there all along) and framed the all-in-one app as a SAFETY requirement: loading another app is a failure point. Console shells to the OS ssh via the humanity-vps alias + existing key, worker-threaded, naturally operator-gated by key possession. Turned the Relay Control Center Control section from copy-the-command cards into real RUN buttons.


## undated

**Why:** Operator asked to add the watch page to app+web and to rotate the TURN credential. Built the receive side of streaming (live_viewer decodes MJPEG to an egui texture) mirroring the publisher. Replaced the committed static TURN password with coturn REST-API ephemeral creds (HMAC-SHA1 over a server-only secret) so nothing sensitive ships to clients and rotation is a one-line VPS change; clients fall back to STUN-only if the relay has no secret, so the migration cannot break voice. nginx active config is sites-enabled/humanity (standalone file, not a symlink) - the earlier sed hit sites-available and no-op'd; backups go to /root, never into sites-enabled (nginx would load them).


## undated

**Why:** Infinite-of-X is a hard project rule and the tooltip was a clear violation. Embedded the JSON via include_str! with a disk-first/embedded-fallback load so a shipped exe without the data/ folder still shows the curated table rather than Unknown/Uncharted.


## undated

**Why:** New concurrency-heavy code with an auth gate deserves a hard second look. 4 review dimensions (concurrency, security, correctness, resource/UX) + per-finding adversarial verification surfaced 10 issues; verification killed 2 as not-real. Fixed: blocking viewer-poll stalling the frame pump (moved to its own thread), missing per-stream viewer cap (added 200-cap DoS guard), GPU-map-failure wedging the capture forever (poll() now releases the slot), ON AIR showing before any frame sent (gated on frames>0), resolution picker always yielding 720p (now parses height), fps offering an unreachable 60, unbounded title field (clamped 200). The verify-before-fix pattern paid off: 2 of 10 raw findings were false and would have been wasted work.


## undated

**Why:** Research found the transport was nearly free (str0m already ships H264/VP8/AV1 packetizers) and the ENCODER was the sole blocker: no pure-Rust real-time video encoder exists and every C-toolchain one is a dependency class the project refused twice. So v1 = MJPEG via the image crate (zero new deps) over a codec-agnostic wire format; H.264 via the windows crate + Media Foundation MFT (hardware NVENC, no C toolchain) is the next rung. Relay routes mounted UNDER /ws/ so nginx prefix-match proxy applies with ZERO config changes (verified: 101 Switching Protocols through prod nginx). Publisher auth is in-band (first WS frame) because a Dilithium key+sig query string is a ~10KB URL that nginx 414s. Stream id = publisher registered name resolved server-side (no spoofing). Full stack proven by an e2e test that decodes the received JPEG + a real-GPU readback test.


## undated

**Decision:** v0.874.0 live weather: NASA GIBS MODIS_Terra_Cloud_Fraction_Day (WMS, no key) -> official 101-entry palette LUT -> RG8 1440x720 mask (R=fraction, G=validity) -> group-3 binding 5 -> cloud_weather blends it as PLACEMENT (envelope smoothstep(0.35,0.9) x meso-carved structure), validity 0 = procedural fallback. Fetcher src/net/live_weather.rs (native-gated - relay CI gotcha), yesterday-UTC composite (todays is ~12pct swaths), nearest-palette classify with distance gate (exact match left 12pct valid), 30min refresh, APPDATA cache, Settings>Planets toggle.

**Why:** Operator asked for real-world weather in-game 2026-07-17. Daily MODIS fraction is near-binary (cloudy-at-any-point saturates) so rendering it 1:1 whited out the globe; mask+carve keeps real geography (verified vs reference: Sahara clear, ITCZ band, Europe cloud) with realistic broken decks. Weather dims live on the renderer everywhere (constants moved to renderer::WEATHER_MAP_W/H because renderer compiles under relay).


## undated

**Decision:** v0.875.0 1m terrain ladder: PatchId.path u32->u64, TILE_MAX_PATCH_DEPTH 16->20 (0.42m triangles), DETAIL_FINE octaves extended 4->11 (125m->1m wavelengths, Nyquist gates 14..20, ~x0.55 amplitude taper to rock scale). Driver picks the cap up automatically.

**Why:** Operator max-settings directive (get to ~1m triangles). Depth 19 (0.84m) engages at the default 640-leaf budget; depth 20 needs the Settings 768 ceiling. Test gotcha worth remembering: screen_error_px has a 1m distance floor and bounds include the radial band, so a fat test band (+-200m) makes every patch within 200m tie at max priority and saturate the leaf budget - descent tests must model MEASURED (thin) bands like production steady state.


## undated

**Decision:** v0.876.0 ocean split (Stage 1 of docs/design/ocean.md): terrain patches render TRUE BATHYMETRY when the connected-ocean mask is present (displaced_radius_f64_true, no water faces); a separate translucent water shell (material type 16) draws the sea - own shallow quadtree (WATER_MAX_PATCH_DEPTH 14, 144 leaves) at sea radius over mask-ocean patches, vertex-stage Gerstner-style height displacement (4 cosine trains, no warp) + reused v0.816 water shading (Fresnel sky, sun glitter, wave normals). CPU twin terrain/ocean_waves.rs with a WGSL-constants lockstep guard test. Ground clamp floats the player on sea+wave height over ocean (drawn==sampled).

**Why:** Operator wants real water (sail/swim/dive eventually). Material bind group gained VERTEX visibility for the type gate + planet center. GOTCHA (new guard in shader_loader test): inserting code between @vertex/@fragment and their fn orphans the attribute onto a const - naga validates FINE but the module has no entry point and every pipeline dies at first boot; the test now pins entry points by name. Known tuning debt: two-tone shallow-shelf banding through the 0.88 alpha, patch-boundary shading steps; underwater fog + diving = Stage 3.


## undated

**Decision:** v0.877.0 water shell moved to the TRANSPARENT celestial pass (alpha blend, no depth write, before atmo/cloud shells). The v0.876 opaque-list push stamped the sea solid (REPLACE blend ignores alpha).

**Why:** Fresnel alpha needs real blending for coastal shallow-water visibility. Boot-verified; float held at wave height. Session note: staged lit captures remain flaky because the subsolar longitude is a function of app uptime (PLANET_SPIN_RATE), not the game clock - the time verb moves the HUD clock but not the lit hemisphere deterministically. Next arc targets exactly this.


## undated

**Decision:** v0.878.0 sun-frame unification: planet spin now derives from GAME TIME + the world sun azimuth (dev_travel::planet_spin_from_time: spin = sun_az + (hour-12)*TAU/24), replacing all six uptime*PLANET_SPIN_RATE sites via one lib helper current_planet_spin(). Subsolar longitude = (12-hour)*15deg by construction, the game clock is Earths lon-0 UTC. Hour derived f64 from GameTime::elapsed_seconds, NOT the f32 hour field (v0.872 quantization lesson). Convention-locking test composes the spin with dir_to_latlon_deg.

**Why:** Three independent clocks (uptime spin, HUD clock, astro sun) made the HUD say noon over a dark surface and burned a whole session of staged captures. Verified: time 12 + camera lon 0 = fully lit disc (Sahara under real weather), surface drop lands in daylight as predicted. Spin rate is now TAU per game day (20 real min).


## undated

**Decision:** v0.879.0 regression batch after operator field report on v0.878.1: (1) ONE cached spin per frame (state.current_spin at RedrawRequested top) - the six v0.878 call sites straddled the TimeSystem tick, physics on pre-tick hour vs render on post-tick hour = ~0.7m dt-jittered ground offset = constant flicker + swimming shorelines. (2) Weather decoder inpaints MODIS swath slits <=24px by row-wise linear interpolation with wrap - the slits rendered as procedural stripes between pinned real swaths. (3) Water shell: NO skirts (they blend-stacked through the translucent surface as border seam lines) + vertex wave displacement fades to zero beyond 2-8km so far patches are exact spheres with bit-matching LOD borders. (4) FTL approach governor: per-frame world step capped at half the distance to the nearest landable surface (Earth + locked body) - at high gear one frame covered 100+km, making the 100km co-rotate band unenterable and each band exit a catapult (the operators fast/slow oscillation).

**Why:** Operator: seams too visible, cloud tiling, constant flicker, shifting shores, cannot reach the surface (stuck ~100mi), speed reset oscillation. Also diagnosed non-bug: planet_clouds was toggled FALSE in the shared config - restored; cloud density was never a code regression. Verified: noon disc clean of stripes with rich matching clouds, 128km ocean view seam-free, deterministic lighting staging works (subsolar-lon math). Descent governor awaits operator manual-flight confirmation.


## undated

**Decision:** v0.880.0 second field-report batch: (1) UNIFIED FLIGHT - co-rotate band (10-100km) now honors the FULL mouse-wheel gear with the same approach governor (step <= alt/2), replacing the 2000x clamp that made it a crawl while one notch past 100km resumed billion-x FTL (the exact stuck-at-100.0km oscillation in the operators screenshot); walk band keeps the bounded wheel + settle. (2) RADIAL_WISH_EPS 0.05 dead zone - surface_wish_dir strips WASD tangentially but leaves 1e-7 radial float noise, and any positive residue took the lift branch at walk speed = the walking-on-Hawaii float-up bug. (3) ONE hotbar - the display-only inventory letter strip under the numbered ability bar removed, ability bar promoted to the bottom slot row at full size. (4) Cloud TOWERING - cloud_carve raises the band top per column by coverage x band thickness (cumulus towers, stratus stays flat; light march shares it). (5) Swath-boundary [1,2,1] blur on the decoded fraction channel - adjacent MODIS passes are hours apart and their time-discontinuity steps rendered as razor seams; now ~100km soft fronts.

**Why:** Operator second field report: still stuck at low orbit (screenshot showed Alt exactly 100.0km = the band boundary), W floats up on the surface, two hotbars, flat clouds vs their real-sky reference photo, remaining hard cloud seams. Their icosphere question answered: the shell mesh is an icosphere but cloud patterns are per-pixel raymarch - the seams were satellite swath time boundaries, not mesh seams. NEXT (operator-directed): HOME DECOUPLING ARC - detach the homestead from the player frame into a stable LEO orbit (ISS-like, visible from the ground; colony ships later).


## undated

**Decision:** v0.881.0 THE HOME DECOUPLING (operator-ordered): the homestead now lives on a real 400km LEO orbit (Earth-centered inertial, period ~92.6 real min, phase = wall-clock UTC so the orbit persists across sessions). Aboard (<400m), the player frame RIDES the station (ship_world_pos += orbital delta per frame) so every home-local system (walls, floors, elevators, machines, crops, construction) works unchanged; the boarding snap adopts the station as frame origin while preserving the cameras world position. Away, ONE pass-level translation moves the entire scene pass (+ room lights + line geometry) to the stations orbital offset, and home-local physics gates off (aboard_station flag); camera_position DataStore becomes home-local so proximity systems silence naturally. Spawn snap fires at character-load completion.

**Why:** Operator: like were on the ISS; colony ships visible from the surface later. Bring-up bugs found live: (1) the spawn snap at world_loaded fired at BOOT and got overwritten by the launcher character-load ship write - moved to character-load completion; (2) the ride tested aboard-ness BEFORE applying the orbital delta, so one 200ms hitch frame let the station advance 1.5km past the 400m radius and dropped the lock - the ride now applies delta first and only the player flying away disengages. Bonus: the orbit-screenshot deck photobomb is dead (verified at Hawaii, no deck). Follow-ups queued: Return-home should target the station, station glint/marker from the ground, Map page shows the orbit, inclination taste.


## undated

**Decision:** v0.882.0 third field-report batch: (1) LOD split/merge HYSTERESIS - once a nodes children are resident it stays split until error < split_px*0.7 (stateless, residency is the memory); the hard threshold flipped parent<->child every frame as the planet spin swept the error past 12px, redrawing coastlines at different samplings = the land/water flashing. (2) Water surface LIFTED 1.2m above nominal sea (ocean_waves::SURFACE_LIFT_M, shared by mesh builder + float clamp + test) - beach-line terrain coincided with the shell within cm and z-shimmered. (3) Flight band (10-100km) now uses fly_wish_dir (fly where you look; W with nose down descends) - surface_wish_dir strips W to the tangent plane, which is right for WALKING but made flight require Shift to descend.

**Why:** Operator: terrain cannot maintain highest detail / flickers worst around landmass-above-water; weird hang at altitude needing Shift to descend. ALSO DESIGNED (operator ask, next arc): TARGET MARKERS - construction-mode respawn/teleport-point marker; Maps-page selection of station/planets/asteroids/ships/enemies -> in-world HUD ring + look-at label (the machine-Tab-label pattern generalized); all mirrored on the Maps page; Stargate-style teleporters much later. Journaled in PRIORITIES as the next UI arc.


## undated

**Decision:** v0.883.0 stationary-LOD fixed point: select_patches now applies the leaf BUDGET before issuing build requests. The old order requested missing children first, so a saturated tree kept commissioning builds it could never draw - cache grew to the 256MB eviction cap, idle children got evicted, hysteresis thresholds flipped, and the budget tail reshuffled every frame: a perpetual build->evict->rebuild wave = the operators standing-still-on-Fuji LOD churn. Now a stationary view converges: refine to budget, requests stop, evictions stop, drawn set becomes frame-identical.

**Why:** Operator third report: rapid LOD switching while parked on Mount Fuji. KNOWN REMAINING (journaled, next): camera_request altitude parking uses the BASE heightmap (no tiles/detail), so parking low over tile-heavy peaks (Fuji) can place the camera inside the drawn mountain until the surface clamp heals after tiles stream in - my verification capture was spoiled by exactly this; operator flight is the real churn test. Up-close roughness (black skirt walls on steep slopes) should shrink as stable neighbors converge to matching depths - reassess after operator flight.


## undated

**Decision:** v0.883.2 TRUE-SCALE terrain: earth.ron surface_relief 0.011 -> 0.003123 (= heightmap window 19,900m / radius 6,371,000m, exactly 1:1 vertical scale). The old value was ~3.5x exaggeration from the 11km-blur era.

**Why:** Operator on Fuji: Minecraft-blocky terraced slopes + Fuji really tall/skinny vs their reference photo. At 3.5x, adjacent 460m tile cells on the volcano differ 800m drawn (>60deg cliff faces with black sides); at true scale ~23deg natural slopes. One data change fixes proportions AND most of the blockiness AND shrinks skirt/crack exposure. Detail-noise amps are real meters x relief so they scale consistently. Hot-reloadable RON. Residual mild flicker journaled as acceptable-for-now (fixed point converges; margins churn only during spin sweep).


## undated

**Decision:** v0.884.0 smooth per-vertex terrain normals: adjacent face normals averaged per grid vertex (outward fallback), lighting interpolates across faces; per-face color/slope-shade unchanged (packed transport). Kills the Minecraft stepping - flat shading had rendered every 0.3m heightmap quantization quantum as a shaded ledge on near-flat plains.

**Why:** Operator final ask before AFK: remove the stepping, make it smoother. Loop mode engaged per operator instruction to work the backlog: marker arc (PRIORITIES 6a), altitude-parking tile-awareness, ocean Stage 2.


## undated

**Decision:** v0.885.0 (loop iteration 1): (a) TARGET MARKERS v1 - gui_state.target_markers (name, render pos, dist) filled per frame with the home station when tracked and >1km away; HUD draws an encapsulating ring + name + distance (accent when looked at) via the machine-marker pattern; Cosmos page gains a Stations section with a Track toggle (session-scoped, default on - config persistence is a follow-up). (b) TILE-AWARE ALTITUDE PARKING - camera_request lat/lon parking now ensures the tile region + samples the DRAWN elevation (detail + tiles) so low parks over tile peaks start above the cone.

**Why:** Operator design (PRIORITIES 6a) v1 slice + the Fuji spawn-inside-mountain fix. Verified: boot clean, 25km Fuji park lands correctly above terrain, true-scale + smooth normals confirmed gorgeous. Loop continues: terrain follow-ups then ocean Stage 2.


## undated

**Decision:** v0.886.0 (loop iteration 2): (a) SUN CORONA - new shader type 17 radial glow (brightness falls with the view rays impact parameter, center-bright melting into space) drawn on a 3x transparent shell over the emissive core; the halo material had existed since the beginning but was NEVER drawn - the white-blob sun was just the bare core. (b) track_station moved into SettingsState + persisted in AppConfig (serde default true). Boot-verified per the pipeline rule.

**Why:** Loop-mode pick: the white-blob sun was a long-known operator complaint, well-scoped, high visual payoff. NEXT-ITERATION CALL: ocean Stage 2 (things float) requires Earth-frame ECS entities - vehicles/crates currently live in the station frame only; that architecture (world-anchored entities, persistence model) deserves operator taste input, so the loop defers it and picks web-parity for the Stations section + smaller polish next; journaled per the no-AskUserQuestion instruction.


## undated

**Decision:** v0.886.2 (loop iteration 3, housekeeping): session history docs/history/2026-07-18.md written (the v0.874-v0.886 planet-quality marathon + lessons + open threads), PRIORITIES 6a marked v1-shipped with remaining slices, CI green (deploys + verify), relay healthy (2 peers), version synced.

**Why:** Session-end convention while the operator is AFK: everything durable is on disk. Loop winds down to a slow health heartbeat - remaining backlog items (ocean Stage 2 architecture, marker extensions) genuinely want operator taste; solo shipping them risks rework. The loop keeps a slow cadence to catch CI/relay regressions until the operator returns.


## undated

**Decision:** v0.886.3 (loop heartbeat): VPS deploy for v0.886.2 failed on the recurring transient github.com:22 timeout; just sync recovered it (VPS at v0.886.2, relay healthy) but exposed a stale Justfile line - the sync recipe still hard-required web/activities (deleted in the 2026-07-05 trim). Guarded it like the deploy recipe already was.

**Why:** Heartbeat found red CI; playbook remedy worked; the stale recipe would have failed every future manual just sync at its last step.


## undated

**Decision:** v0.887.0 max-graphics + LOD equilibrium: (1) LOD error metric fixed twice - eff_r clamps bound radius to patch edge (fat conservative bands made everything within 20km claim dist=1m and starve the underfoot) AND unbuilt children inherit the PARENTS measured band padded 60m (the planet-wide bands mid-radius sits km below summits, so unbuilt children on mountains read km-away and the descent stalled - coasts split, Rainier never). (2) MAX_OBJECTS 1024->4096; patch budget ceiling 3072 (default 2048, operator at 3072); diag confirms equilibrium maxleaf ~5px at the 4px target. (3) Settings LOAD fix - the three Planet LOD sliders saved since v0.873 but were never applied at boot (the operators graphics dont save bug). Defaults maxed (split 4px, budget 2048, builds 64). (4) Sun core -> type-17 radial glow in the transparent list (no silhouette seam inside the corona). (5) Water: alpha 0.96 (coast glow), coarse warp 1.35 + WAVE1 slope 0.035 (offshore corduroy). (6) Dense-cloud edge sharpening (silver-lining reference) + Rust mirrors synced. [ChunkDiag] 1Hz telemetry kept as permanent dev tooling.

**Why:** Operator max-graphics directive + field report. KNOWN COST: 3072 patches ~15 FPS on the 4070 (draw-call bound) - the budget is a slider; batching/instancing journaled as the perf arc. Remaining from the report: vegetation placeholders, true-scale body audit, god rays, cloud catalog, settle drift (~150m/s residual).


## undated

**Decision:** v0.888.0 PROCEDURAL VEGETATION v1: trees (crossed-card trunk+canopy conifers, 7-13m) at patch depth >=15 and grass tufts at >=18, baked INTO patch meshes at build time - deterministic per-PatchId xorshift scatter, elevation-gated (3m..treeline 1700m), positions through the same elevation sampler as the grid so everything stands ON the drawn ground; LOD free (vegetation lives and dies with its patch). Verified: Washington lowland hillsides read as forest. ALSO: TRUE-SCALE AUDIT COMPLETE - all body radii (sun 695,700km IAU nominal), masses, and semi-major axes in data/star_systems/sol.json match real values; nothing to fix.

**Why:** Operator asks: vegetation placeholders + verify true scale in sizes/distances/mass. REMAINING from their report, journaled for next iterations: god rays (needs post-process infra), cloud catalog expansion (100+ types - regime table growth), settle lateral drift (~150m/s residual while parked), and the PERF ARC: 3072 patches = 15 FPS draw-call-bound -> batching/instancing.


## undated

**Decision:** v0.889.0 OVERNIGHT 1+2: (a) LOD PREFETCH - select_patches leaves past 0.55x split_px request unbuilt children early (cap 12/selection, skipped at budget saturation) so motion crosses thresholds into resident meshes; direct answer to operator cache/preload ask against move-flicker. (b) LOD_CLEARANCE_M 2.5 added to BOTH clamp_above_ground and rest_radius (shared floor - settle never fights clamp) fixing see-through-Earth while standing: drawn coarse patches can bulge above the full-detail clamp model.


## undated

**Decision:** v0.890.0 OVERNIGHT 3-5: (a) F6 saves exact camera pose to debug/bookmarks.json in the planet UNROTATED frame (spin-independent; frame_lock_capture math), restore via camera_request {bookmark:bm-N|last} - exact placement+aim teleports for screenshots. (b) Q/E flight roll: Camera.roll (view-only, rolled_up rotates the look_at up vector; movement basis untouched), 80deg/s, auto-eases level on foot; Q keeps shoulder-swap on foot, E stays interact. (c) Blend band 100-1000km now rides (1-s) smoothstep of the spin delta each frame - kills the ~465 m/s one-frame velocity yank at the 100km co-rotate boundary, both directions. Plus GuiState.pending_toasts for engine paths without an egui clock.


## undated

**Decision:** v0.891.0 PERF ARC (4x): the 15-FPS-at-3072-patches bottleneck was NOT GPU draw count but CPU submission - one queue.write_buffer PER OBJECT per pass (3000+/frame) + full material bind-group rebinds per object. Fix: upload_object_uniforms (one staging vec, ONE write_buffer per pass) + bound_material elision (patches share one material). Measured identical Everest 3km scene, 2048 draws saturated: 19.8->78.6 FPS (50.4->12.0ms). NEW MEASUREMENT HARNESS: portable perf probe - copy exe to scratch dir + portable.txt + junctions to repo data/assets, autopilot with server_url EMPTY (offline world, no relay noise, identity guard satisfied), camera_request scenic view, screenshot_done.json carries fps+frame_ms_avg. Reusable for any future perf comparison.


## undated

**Decision:** v0.893.0 CLOUD CATALOG 7 FAMILIES (+altocumulus, cumulonimbus, nimbostratus; tent hw 0.42->0.22; tests extended). FXC GOTCHA (cost one panic): naga HLSL backend cannot pass array<f32,N> across fn boundaries (X3017) - regime blend is now one scalar-accumulator loop. PARKED DRIFT re-measured on the probe: alt rock-steady 50.1km over 60s parked at Everest co-rotate band, no lateral drift possible by construction (anchor only moves on input); the old ~150m/s note does not reproduce - CLOSED. Probe world runs accelerated game time (~77x, fresh-world default) - use showcase {time:H} for staged light, local noon = 12 - east_lon/15 (SUBTRACT for east; Everest ~6.2). Yellow-quad artifact at Oahu = distant-island no-imagery elevation-ramp fallback on a coarse patch, pre-existing, self-heals on approach - NOT a bug.


## undated

**Decision:** v0.894-v0.895 OVERNIGHT (cont): v0.894.0 trees now spawn at EVERY depth >=15 with fixed-point accept keeping per-AREA density constant (they used to VANISH when patches refined near the camera - forests only existed at ridge distance); TREES_PER_PATCH 10->40; camera_request ocean parking clamps to the water surface (Iceland-at-120m had parked UNDERWATER over the seafloor). v0.895.0 GOD RAYS: one additive depth-march pass (godrays.rs/wgsl) between celestial and scene passes - ridge silhouettes carve shafts; verified via the new camera_request {aim:sun} staging rig after an hour of blind time-staging proved sun-in-frame shots are luck without it (probe world runs ~77x clock, sun races). Projection cross-checked exact (sun ndc 0.01,0.03 dead-center on aim). PROBE GOTCHAS learned: local noon = 12 - east_lon/15; staged times expire in real seconds at 77x; scenic-view aim is nadir-relative so the sun is almost never in frame naturally.


## undated

**Decision:** v0.896.0: imagery-green biome gate for vegetation (surface_color sampled at scatter points - real Earth imagery IS the biome map; kills Sahara/glacier trees), vegetation cards take the radial up as normal (horizontal card normals rendered black at noon), ocean parking radius-gated in BOTH camera_request and the walk clamp (ocean mask misses fjords + may still be loading when scripted requests fire; ground >60m below sea = float at sea level, keeps Death Valley walkable). KNOWN QUIRK journaled: scene brightness follows global game hour while sun position is longitude-aware - staged captures far from lon 0 look dark at their local noon; unify when it matters.


## undated

**Decision:** CORRECTION to the lighting-quirk note: the scene sun is CONSTANT intensity 2.5 with the real astronomical direction (only set_sun_light site, lib.rs ~15938) - there is NO hour-based dimming; the earlier diagnosis was wrong. Staged noon formula H = 12 - east_lon/15 is UTC-independent (sun_az cancels: spin = sun_az + (H-12)*TAU/24 and subsolar-at-L needs spin = sun_az - lon_rad). Sahara + Iceland staged noons verified bright; Oahu/Rainier staged shots still read dusk-dim - unpinned (suspects: high-latitude sun elevation + ACES curve, probe 77x clock racing the staged hour, or a spin-model subtlety). aim:sun rig verified exact at Everest. Do NOT re-flag the hour-dimming claim.


## undated

**Decision:** v0.897.0 MORNING ROUND 1: (a) VEGETATION CELLS - plant positions from a planet-fixed lat/lon hash grid (fixed 6 randoms per plant so patches sharing a cell stay stream-aligned); same plants at every LOD depth, so splits no longer reshuffle the forest (big share of reported flicker + grass-vanishing). (b) God rays: SCREEN blend (OneMinusDst) so bright cloud decks receive ~nothing (operator: rays blew clouds to super white) + overhead MODIS dimming to ~15% under real overcast. (c) Settings: distant-planet sliders labeled as such (operator hunted them for ground flicker - they only shape space views), ground-detail trio captioned, split floor 4->2px. LESSON: killed the operator LIVE game with a sloppy taskkill window-title filter - kill the probe by PID from its own Start-Process handle, never by image name while the operator plays.


## undated

**Decision:** v0.898-v0.899 THE FLICKER AUTOPSY + SHADOWS: (1) Flicker was TWO oscillators: the 256MB patch cache (sized for 640 leaves in another era) forced build->evict->rebuild waves at 6144-leaf budgets (probe: draws swinging 3572->1561->4577/s, req pinned, cache pinned) -> 1.5GB + never-evict-recent(120 frames); and prefetch had hollowed the residency-keyed hysteresis -> re-keyed on last-frame DRAWN set + committed splits get 5% budget grace so the refusal wall cannot wander. Probe-proof: 5s of byte-identical selections at 6144. (2) SUN SHADOW MAP shipped (4096 ortho, texel-snapped, PCF, depth-only vs_main variant so ocean waves cast; group-3 bindings 6-8 shared by every pass so interiors receive too). wgpu lessons: camera layout carries the lights storage buffer (bind it in ANY camera-layout group); a pass cannot sample the texture it writes (dummy-depth group-3 variant for the shadow pass). Veg cards cast shadows for free (in patch meshes).


## undated

**Decision:** v0.900-v0.901 CLOSE-OUT: shadow-caster distance cull (65 km anchor bound - the ortho box is 1.5 km); SSAO shipped (10-tap golden-angle, depth-only, celestial slot, multiply blend, reverse-Z linearized from the real projection m22/m32; sky/clouds untouched, probe-verified no halos). Handoff docs: PRIORITIES final-sprint block with ranked remaining wants (SSAO-successor items: tangent-space ground texture <8m, geomorph fades, cloud-shadowed god rays, Settings toggles for sun_shadows/godray_intensity/ssao_strength renderer fields), history 2026-07-19 day-sprint section, MEMORY.md arc updated.


## undated

**Decision:** v0.902.0 TEXTURE ARC: living ocean (25km+2.5km planet-pinned hue/wave-strength variation kills tiling; 3.2m/0.8m camera-relative micro ripples give close motion; nadir alpha 0.93) + sub-metre GROUND texture. KEY TECHNIQUE unlocked: the CAMERA-RELATIVE PRECISION DOMAIN - fragment offsets taken relative to the camera (small = full f32 precision) + the camera planet-frame position mod 64m poked into light0_cone_inner.yzw as the anchor; periodic lattice noise with periods dividing 64m makes anchor jumps seamless. This BREAKS THE 8m PRECISION FLOOR for any future surface detail (normal maps, tangent textures, decals) - reuse the pattern. Residual wants: faint patch-seam blockiness in the sea variation, coast foam, tangent-space normal-mapped ground material.


## undated

**Decision:** v0.903 DIVING + GARDENS + 4-AGENT AUDIT WAVE: (a) ocean enterable to the Marianas - ROOT CAUSE was ground_radius_m using the sea-CLAMPED displaced radius; now true bathymetry on water worlds, water-surface floor applied separately and pierced by descend input; neutral buoyancy underwater; egui underwater tint; probe-verified at -60m mid-Pacific. (b) Gardens: only 10/134 crops had visual recipes, rest SILENTLY skipped mesh gen (audit) - generic_visual fallback (FNV-varied) + 9 hand-authored staples (potato incl). (c) v0.904 data hygiene from the gameplay audit: creature loot ids namespaced to real items (leather->leather_hide_0 etc, 89 subs), rice/coffee recipes consume farm outputs, plant_fiber/brass/diamond dead inputs remapped, 4 fiber-bundle conversion recipes, gold/silver ores added to M-12 + bauxite/rutile to S-7 (were vendor-only chains). AUDIT REPORTS (4 subagents) captured in this journal + PRIORITIES: gameplay-loop gaps (quest files 80% dead ids - REWRITE NEEDED; no forage faucet for wood/stone/clay/salt), planet appearance integration map (albedo path is generic; pluto needs a def; gas giants = flat ochre, type-18 band shader designed; USGS PD map URLs journaled), plant asset pack links (Quaternius/Kenney/KayKit CC0 verified).


## undated

**Decision:** v0.905.0 THE SOLAR SYSTEM GETS REAL (operator greenlight wave): maps subagent baked Moon+Mars (SSS 8K CC-BY) and Pluto (NASA New Horizons PD, south-pole no-data smoothly filled) via new generalized scripts/build-planet-albedo.js; pluto.ron created + embedded; grade_albedo dry-world passthrough (G1). Type-18 gas-giant band shader (Jupiter belts+GRS, Saturn gold, Uranus cyan, Neptune azure+storm) keyed by material params.w - probe-verified. 5 comets added (69 bodies; moons/asteroids ALREADY existed - the operator ask was largely satisfied by existing data). Detail-draw-distance slider (0.5-3x, default 1.5) scaling detail_octave_fade via the view_pos.w pad; budget ceiling 12288, MAX_OBJECTS 16384. CREDITS.md + in-app Library Credits category (build-library.js sources repo-root CREDITS.md). GREENLIT NEXT (operator): Poly Haven CC0 plant models to replace procedural plants (GLB pipeline into plant_mesh/garden rendering) + ambientCG CC0 PBR texture sets for terrain ground materials (bind via the camera-relative precision domain; needs a group-3 binding addition - follow the shadow-map bindings 6-8 pattern for both bind group creation sites).


## undated

**Decision:** v0.906.0 STORM SEAS + ASSET DELIVERIES: ocean de-squared (3 rotated noise octaves replace the single axis-aligned tap - value-noise lattice was the operator-reported rectangles), STORM STATE from the live MODIS weather (chop x2.4 + steepness-driven whitecap foam under real rain cells; probe-verified streaked wind-lanes fading to calm at the storm boundary; foam capped 0.72). TWO SUBAGENT DELIVERIES COMMITTED: assets/textures/ground/ (4 ambientCG CC0 PBR sets, 2K color+normal PNG + manifest + blend mapping: rock by slope, sand by beach band, grass by imagery green, dirt default) and assets/models/plants/ (6 Poly Haven CC0 photoscans + manifest; loader analysis: parse_gltf_mesh loads FIRST-mesh-FIRST-primitive only and discards textures - grass_medium_02 + fern_02 usable now, shrub/potted/saplings need a merge-repack, all need texture support for full beauty). NEXT WIRING: ground-texture group-3 bindings (follow shadow bindings 6-8 pattern) + gltf texture support / repack script.


## undated

**Decision:** v0.907.0 GROUND PBR TEXTURES + PARALLEL BACKLOG WAVE. Solo: ambientCG grass/dirt/rock/sand wired as an 8-layer Rgba8Unorm texture array (group-3 bindings 9/10, new src/renderer/ground_textures.rs: parallel PNG decode 276ms, CPU sRGB->linear + PER-CHANNEL MEAN NORMALIZATION to 128 so textures carry pure structure and NASA imagery keeps owning color, CPU mip chain, repeat+aniso sampler, neutral 1x1 fallback = exact pre-texture render). Shader: triplanar in the v0.902 camera-relative anchor domain (GROUND_TILE_M=2 divides 64m), explicit footprint LOD, slope->rock / imagery-green->grass / warm-bright->sand / default dirt weights, dominant-material normal perturbation (normal now var). Micro-detail window widened 4->8 m/px * ddk. ALSO: Settings sliders sun_shadows/godray_intensity/ssao_strength (full config<->gui<->renderer chain + zero-strength early-outs), underwater depth-graded tint + HUD depth readout (underwater_depth_m). Boot-verified (0 PANIC), probe-verified Sahara sand grain + Canyon dirt/rock speckle. AGENTS: quest rewrite (41 dead ids fixed, 4 files; Travel objectives replaced - no travel emitter exists), sawmill+grain_mill MachineDefs + pre-placed, forage findings (stationary-creature flag needed for flora), plant repack script scripts/repack-plant-gltf.js -> 6 *_merged.gltf single-primitive loader-ready. FINDING (pre-existing, now top of asset queue): EUROPE-NOON DARKNESS - France 47N noon Clear is night-dark in v0.905.1 AND v0.906 (A/B probe-proven); Sahara same clock bright. Suspect MODIS cloud-ground-shadow double-darkening vs Clear sky regime + dark farmland imagery. Merged stray agent branch clever-moore (egui Unaligned overlay suppression).


## undated

**Decision:** v0.908.0 EUROPE-NOON-DARKNESS FIX (root-caused by probe A/B + reading the actual albedo texels): Blue Marble vegetation is ~20x darker than desert in LINEAR light (France 47N luma 0.021 vs Sahara 0.40) - temperate noon was night-dark in every build, not a regression. 3-part fix: (1) land_gain() shadow lift at bake - below 0.15 linear-luma knee ride luma^0.5 power curve (hue-preserving, continuous at knee) + green nudge; France bakes ~5.4x brighter, deserts byte-identical (tests share the fn). (2) Cloud ground-shadow ceiling 0.5->0.35 (MODIS daily mask keeps temperate land ~permanently decked; half-light noon read gloomy). (3) Ground texture layers desaturated 60% toward luma BEFORE per-channel mean-normalization (unequal channel scales skewed per-pixel hue - bright grass texels went warm-brown over France). LESSONS: grade_albedo raw is LINEAR (bake decodes first); classifiers should NOT re-linearize. Probe: France noon pitch black -> readable green field w/ grass texture. Further brightening is now an operator taste call (knee 0.15 / exp 0.5 / shade 0.35 are the knobs, all in planet_surface.rs + the type-12 cloud-shadow block).


## undated

**Decision:** v0.909.0 OVERNIGHT ENVIRONMENT BATCH (operator morning report): (1) OCEAN WHITEOUT root-caused - detail-distance slider scaled the ANIMATED wave/foam fades past their pixel-coverage bound into speckle; new detail_octave_fade_aa (unscaled) for all water octaves + hard 5m/px foam reach + crest-only threshold. Probe-verified at the exact 44km regression view: smooth blue. (2) SEA STATES 0..1 via fill_color.w pad (glassy 0.3x chop / classic / storm 2.3x + slate darkening + crest streaks), driven by game wind smoothed 30s, max with local MODIS storm cell; showcase {sea:x|auto} pin. (3) Beach de-blued: ocean floor fades over first ~40m depth (OCEAN_FLOOR_DEPTH_BAND 0.002). (4) PLANTS IN-WORLD: material type 19 (textured mesh + alpha cutout via existing albedo_bind_group path), loader texture decode (agent), 17 split variants (agent), decorations.ron scatters 51 plants at homestead anchors (machine_objects pattern; decoration_mesh_cache reuses GPU resources across reloads). (5) CLOUD DENSITY CONTRAST: regime tint spread 0.42-1.0 (was 0.68-1.0) BOTH tables (wgsl+rust mirror), + column-opacity darkening 0.68x at body_total>0.72. (6) TRANSITIONS: bands scale by body radius (clamp 0.001-4, floors 200m/2km/20km), 6% edge hysteresis kills boundary mode-flapping, set_surface_up preserves WORLD forward on up changes (the aim-loss root cause - yaw/pitch were basis-relative through a 90deg blend). (7) SETTINGS TRUTH PASS (agent audit): 23 hints added w/ low-high tradeoffs, 40 controls confirmed live, VSync + Invert-Y were decorative -> wired (surface reconfigure / both look paths + persisted). AUDIO ENGINE IS ORPHANED (AudioManager: zero callers) - volume sliders honestly hinted, integration queued. KNOWN NEW BUGS: teleport-over-deep-ocean lands km off (bathymetry convention mismatch, blocks ocean probe shots), cloud raymarch underside banding, deck-interior FPS 10-16. v0.909.1 removed 12MB lintrun/ binaries committed by git add -A (now gitignored).


## undated

**Decision:** v0.910.0 OCEAN ALTITUDE REFERENCE: alt over has_water oceans now measured from sea surface (max(ground_r, def.radius)), fixing HUD 3km-off readings, premature walk-band engagement, and the flight governor braking against the SEABED on ocean approaches. Divers read negative alt. Probe: 2.9km->7m. Calm-glassy sea state visually confirmed (mirror sheet under Clear 2m/s). TRANSITION DESIGN DIRECTION (answering the operator direction question): keep the banded model short-term (now planet-scaled + hysteretic + aim-preserving) but the end-state is dissolving bands into CONTINUOUS curves of normalized altitude (co-rotation weight, speed governor, up blend - each already blends across one band; extend across the whole range so no threshold exists) + a sphere-of-influence frame hierarchy for translation beyond the Earth locale. Session narrative: docs/history/2026-07-20.md.


## undated

**Decision:** v0.911.0 MORNING-REPORT BATCH: (1) HOME ROUND-TRIP root-caused by audit agent - v0.881 orbital station moves 7.66 km/s; dev-travel return restored the DEPARTURE-time stash -> player returned to empty space while labels (which never ride station_off) still projected home content. Fix: dock to station CURRENT frame + labels project with station_off (gui_state.station_off) + roll reset. FULL DECOUPLING PLAN (5 steps: StructureInstance fold -> data/structures.ron frames -> player FrameRef) in the audit report, journal-adjacent. (2) STRING LIGHTS: nearest-8 cap was pre-v0.782 legacy; now influence-sorted keep-64. (3) REAL TREES groundwork: near_tree_instances mirrors the vegetation stream (same xorshift/gates) so models stand where cards are; tree_model_distance slider DEFAULT 0 (EXPERIMENTAL) because cutout alpha is unproven visually - twig textures NOW carry real alpha (Poly Haven mask PNGs merged via ffmpeg alphamerge, variant gltfs repointed; in-engine stat confirms 93-97% transparent texels) but probe captures still showed pale slabs (which may be the CARD trees, not models - aim roulette prevented the disambiguating shot; operator screenshot will settle it instantly). white_key_alpha_if_cutout added in the loader for pale-bg cutout sheets. (4) PERF: 4-lens workflow -> caster cull 65->6km, cloud sun-tau saturation break, slab-crossing-scaled view samples, godray gate hoist; the 5th win (upload skip) PROBE-BISECTED TO BREAK THE ATMOSPHERE DOME on DX12 (byte-identical writes; ordering subtlety) - reverted with do-not-reattempt comment. Findings doc: docs/dev/performance-findings-2026-07-20.md. (5) DEV DOCS: docs/dev/ = 8-guide content-pipeline suite (agent, 0 broken links); stale docs flagged: assets/shaders/README.md (wrong groups), docs/game/model-pipeline.md rule 7. (6) Ship-deck decorations REMOVED (operator: planets, not the spaceship deck). PROBE-VERIFIED before ship: atmosphere dome matches v0.910.1 baseline, 0 panics.


## undated

**Decision:** v0.912.0 OPERATOR FIELD-REPORT BATCH 2: (1) DAYTIME STAR OCCLUSION - atmosphere alpha was pure transmittance (~0.1-0.3 up) so the starfield rode through the day sky; alpha now rises with sky luminance (sky_lum*3.2 max) - day occludes, twilight fades in, night untouched. (2) GEOMETRIC NEAR CHOP: ocean W5 (18m/0.12m) + W6 (6m/0.05m) height trains displace real vertices within 800m (deep-water cps 0.30/0.52, dirs WAVE2/WAVE5); CPU twin TRAINS extended to 6 + MAX_WAVE_HEIGHT_M updated + lockstep test widened (700->1400 byte window + dir list). (3) OCEAN DE-MOIRE: water AA band 4-12px -> 9-24px per wavelength (the dotted gratings were marginally-resolved octaves beating the pixel grid); foam threshold 0.16-0.30, storm gate 0.5-0.95, cap 0.4. (4) TREE LOD SWAP: SurfaceVertexData.tree_card -> bit 17 of packed UV (pack_color_to_uv_flags; grass cards unmarked); shader discards marked cards within shadow_u.params.w (=tree_model_distance, poked su[19]); probe-verified real branches near + cards far. Cutout alpha CONFIRMED WORKING (operator screenshot showed needle sprays through the card slab; the slab WAS the card, now hidden). (5) LIGHTS 64->256; 1000+ = clustering arc queued. LESSON: the camera_request look_offset aim is unreliable at ground level (settle interaction) - probe ground shots need several attempts; an aim fix for the probe is worth an hour some session.


## undated

**Decision:** v0.913.0 FIELD-REPORT BATCH 3: (1) TREE TRUNKS - single-material merge made trunks borrow the twig texture -> cutout discarded them; split script now emits _bark companions (own bark texture, shared base offset) + engine draws both parts; script PERMANENTLY prefers *_diff_a_*.png alpha twins (a re-split had silently reverted to alpha-less JPGs - caught by the in-engine transparency stat, 0% -> the white-slab regression; regex was shell-mangled on first patch, fixed via Edit). (2) VARIETY+DENSITY: 6 shapes (fir/pine x v1-3), heights 4-18m pow-1.6 skew (BOTH stream sites in lockstep), TREES_PER_CELL 42->100, GRASS 36->80, hero cap nearest-64 (distance-sorted enumeration). (3) SKY: day-elevation-driven occlusion (day*0.985) + sun-disc window (toward_sun mixes back to transmittance alpha) - whole dome hides stars at noon, sun stays sharp. (4) FLICKER root-caused by parked probe: [a] eviction removed recency-stale INTERIOR nodes of drawn-leaf descent chains -> one-frame d6/6-Mpx ancestor flash at the camera; collect_evictions now protects the whole ancestor chain of last_drawn. [b] planet spin drifts fringe errors across the split threshold perpetually; fresh splits need 1.15x (dead zone both sides). Parked convergence trickle documented - geomorph fades remain the definitive anti-pop (queued). (5) F6 BOOKMARKS GUI: Dev > Travel lists bookmarks grouped by category, one-click restore via the same restore_location_bookmark path, category box tags future F6 saves; bookmarks.json gains category field. (6) RESEARCH doc docs/dev/rendering-research-atmosphere-water.md (Hillaire LUT sky, aerial perspective, shoreline depth fade, FFT ocean) with an 8-step ranked roadmap - items 1-3 (sun transmittance, aerial perspective, shoreline fade) are the next graphics arcs. Boot-verified 0 panics; probe: trunked varied dense forest confirmed on screen.


## undated

**Decision:** v0.914.0 BATCH 4 + LOOP MODE ENABLED (operator: "enable loop mode to work through everything we have discussed and the backlog"): (1) TREE POP - card-hide radius now tracks the drawn-model reach per frame (was the full slider radius while models capped at 64 -> trees past the cap had NO representation; approaching dense stands rotated near trees out). (2) NIGHT OCEAN GLOW - foam was an UNLIT constant mixed after water_shade; now sunlit (n_geo dot sun * intensity), dark at night; coverage tightened (0.20-0.36 steep, 0.55+ gate, cap 0.35). (3) Species max heights (fir 22m pine 16m +-12%, both stream sites lockstep), grass 160/cell (4.4x). (4) Bookmark delete (x) + recategorize (>) in Dev > Travel; lib rewrites bookmarks.json. QUEUED as bookmark-studio arc: maps-page teleport, POV preview editing, time/weather scrub. LOOP QUEUE (from operator + PRIORITIES, ranked): [1] sun-disc transmittance (research item 1, S), [2] aerial perspective on terrain (research item 2, M), [3] shoreline depth fade + foam line (research item 3, M - fixes water-land interface), [4] terrain stepping smoothing, [5] exposure calibration (washed dome + white water reflections), [6] water tess/subdivision increase near camera, [7] geomorph fades, [8] bookmark studio arc, [9] LOD ladder per size category + plants.csv species tie-in, [10] light clustering, [11] audio integration, [12] decoupling steps 3-5. Loop protocol: solo hot-file work, ship each increment, probe-verify renderer changes, journal every release.


## undated

**Decision:** v0.915.0 LOOP ITERATION 1 - SUN TRANSMITTANCE (research item 1): renderer::atmosphere::sun_transmittance (Chapman od_to_space + the WGSL beta_ext construction beta_ray + 1.11*beta_mie) tints the global sun LIGHT color per frame when frame-locked to an atmosphere planet (lib.rs sun-light block; construction override exempt). Physics test: noon passes, horizon dims + reddens 2x+, set = black. Probe: warm canopy light at 18:24, black after sunset. LOOP QUEUE next: [2] aerial perspective on terrain (M - march 4-6 samples camera-to-fragment with the Chapman legs in the type-12 branch, surface*T + S, strength slider), then [3] shoreline depth fade + foam line.


## undated

**Decision:** v0.916.0 LOOP ITERATION 2 - AERIAL PERSPECTIVE (research item 2): exponential height haze at the end of fs_main shared path (color*T + sky*(1-T)); params poked via the unused per-light cone pads (light1_cone_inner.y sigma folded with strength*altitude-density, .z slant cap 3H, light2_cone_inner.yzw sky color day+sunset-tinted from the v0.915 transmittance, light3_cone_inner.yzw camera radial up for the slant bound). Interior passes zero the pads (full uniform rewrite) so rooms never fog. Settings aerial_strength slider 0-2 default 1. GOTCHA: WGSL camera struct has SCALAR light0..7_cone_inner fields, NOT an array - camera.light_cone_inner[1] fails naga parse (caught by the embedded-shader validate test). Probe: 11km canyon vista wears a distance-deepening warm veil. LOOP QUEUE next: [3] shoreline depth fade + foam line (M - depth copy after opaque pass at group 3 binding 11, thickness absorption tint, waterline alpha ramp, animated foam band, depth-attenuated wave amplitude + ocean_waves twin).


## undated

**Decision:** v0.917.0 LOOP ITERATION 3 - SHORELINE (research item 3): AVOIDED the depth-texture infrastructure entirely - build_water_patch_mesh now takes the heightmap and bakes per-vertex seafloor depth (decimetres) into the color->packed-UV transport; the packed scalar (water_bit + depth_dm) interpolates as LINEAR DEPTH across triangles. Shader: turquoise shallows (<9m), shoal wave attenuation (<7m), waterline alpha feather (<1m - the hard sea/land polygon edge dissolves), animated breathing surf line (0.2-2.2m band, along-shore noise, sea-state scaled, sunlit like all foam, footprint-faded by 9m/px). REPRODUCED the operator white-stripes ocean at 6m/day: NOT a regression (identical pre-v0.917), root = grazing fresnel reflecting the overbright sky -> the EXPOSURE CALIBRATION iteration owns it (sky_term math: horizon ramp * sun 2.5 * WATER_SKY_GAIN 0.2 does not explain the white alone; suspect the tonemap-normalized washed dome, research item 4 multiple-scattering + ATMO_EXPOSURE walk-down). LOOP QUEUE next: [4] exposure calibration (washed dome + white water; research item 4: multiple-scattering energy + walk ATMO_EXPOSURE 4.0 toward 1.x, retire NEAR/HAZE compensators), then [5] terrain stepping smoothing, [6] geomorph fades, [7] bookmark studio, [8] LOD ladder + plants.csv, [9] light clustering, [10] audio.


## undated

**Decision:** v0.918.0 LOOP ITERATION 4 - EXPOSURE CALIBRATION (research item 4): three-tier atmosphere exposure - ground SKY rays ride new ATMO_EXPOSURE_DOME 1.7 (was the space-calibrated 4.0 that ACES-clipped the dome white), ramping back to full via w_alt(rp->1.0 shell top) so 400km limb + 12000km marble stay bit-identical; grazing surface rays (b~rp, the white veil on grazing water) blend toward the sky tier instead of FULL, horizon seam continuous; analytic isotropic multiple-scatter term ATMO_MS_ISO 0.07 on the same per-channel integral, gated by (1 - max(w_alt,w_far)) = exactly where the dome dimmed; twilight star-occlusion gain 3.2->4.5 compensates. NEAR/HAZE compensators KEPT deliberately (they fix close-range surface haze, not the dome; revisit under research item 5). BUG-047 found en route: shell meshes rode 5.min(planet_max_subdiv) - at low planet-detail settings the icosphere face planes sink INSIDE the planet (level-0 inradius 0.79R vs 0.97R ground) = whole sky underground, stars at noon, no limb. Probe rig had max_subdiv=0 from an old perf experiment, which made the bug masquerade as a code regression across an hour of exe bisection (v0.913-v0.917 all identical). Fixed: both shells pinned at level 5; rig config restored to defaults. LESSON (in BUGS.md): dump rig config vs config.rs defaults BEFORE bisecting visuals; camera_request look_offset_deg is a SCALAR (arrays parse as 0). Probe-verified: Sahara noon saturated-blue dome, desert horizon gradient, Gulf of Sidra grazing water veil-free, red sunset ember, 400km limb restored, marble unchanged. LOOP QUEUE next: [5] terrain stepping smoothing (heightmap terracing on steep slopes), then [6] geomorph fades, [7] bookmark studio arc (maps-page teleport, live POV preview, time/weather scrub), [8] LOD ladder per size category + plants.csv species tie, [9] light clustering 1000+, [10] audio engine integration, [11] decoupling steps 3-5, plus water tessellation near camera, sky-view LUT (item 5), Gerstner choppiness (item 6), FFT cascades (item 8), probe camera-aim fix.


## undated

**Decision:** v0.919.0 LOOP ITERATION 5 - SYNTHESIZED HEIGHTMAPS (queue: terrain stepping smoothing): probe diagnosis first - Earth terrain is already smooth everywhere (every elevation sampler is Catmull-Rom bicubic; Fuji ground/6km + Everest captures clean), the operator icosphere stepping lives on bodies WITHOUT a heightmap file: no planet_heightmaps entry -> chunked-LOD never activates -> Moon at 200m = bare uniform icosphere, km-wide flat facets (probe capture). Fix: new terrain/procedural_heightmap.rs synthesizes a 1024x512 grid at load for any def without a measured file - fBm Perlin highlands (seed offsets 201..) + power-law stamped craters (parabolic bowl + raised rim, u^3 size bias, great-circle pole-safe stamping); ALL parameters derive from existing RON fields (terrain_seed, noise_frequency/octaves, atmosphere alpha -> crater survival, radius -> 0.5% relief clamped 2-12km) = infinite-of-x clean, no per-body tables. ~100ms/body at load, 1MB resident. Dropping the grid into planet_heightmaps lights the WHOLE Earth machinery untouched (chunked patches, drawn==sampled clamp, DetailNoise wrinkle, per-pixel albedo bake - moon/mars/pluto ship real albedos). sea_level NOT overridden for synthesized (airless = maria color threshold, not coastline). PlanetHeightmap::from_grid w/ from_bytes validations. Probe: Moon 200m/51km = continuous cratered regolith w/ real albedo + crater bowls, Mars 30km = rusty dunes under warm limb. Terrain tests 98 green, relay clean. FOLLOW-UP noted in module header: sub-27km craters unresolvable at grid res - needs a DetailNoise-style crater octave for walking-distance craters. LOOP QUEUE next: [6] geomorph fades (definitive anti-pop; DELICATE - touches the v0.913 frame-frozen selection machinery, plan approach first: dither-fade swap w/ retained parent mesh via existing ancestor-chain protection), then [7] bookmark studio arc, [8] LOD ladder per size category, [9] light clustering, [10] audio engine, [11] decoupling 3-5.


## undated

**Decision:** v0.920.0 LOOP ITERATION 6 - GEOMORPH FADES (queue item 6, definitive anti-pop): terrain LOD splits/merges now dissolve over 0.30s with a COMPLEMENTARY 4x4 Bayer screen-door crossfade (old+new generations partition the screen per-pixel: no holes, no double-write, opaque depth intact, zero alpha blending). The v0.913 selection machinery is UNTOUCHED - this is presentation on a drawn-set diff. Plumbing invented for it that generalizes: RenderObject.fade rides model[0].w (the VS now rebuilds homogeneous w after transform - a no-op for TRS matrices - freeing the whole matrix w-row as per-object metadata; normal matrix computed from the clean transform), and fs_main dithers at the TOP so ANY faded object works - the tree/animal LOD ladder (queue 8) can ride the same channel. classify_lod_swaps pairs vanished-parent/appeared-children (split), appeared-parent/vanished-children (merge), orphan rises (stream-ins), orphan pops (culled = instant, drawing off-screen fades is waste). ingest purges re-appearing ids from falling lists (hysteresis flip mid-fade cannot double-mask). collect_evictions protects mid-fade patches (merge children are NOT ancestors of drawn leaves - the v0.913 chain guard alone would evict them mid-dissolve = hole flash). Settings > Graphics Smooth detail transitions toggle default ON, full persistence chain. Gated off during activation + not-fully-covered. 42 tests green (classifier/ingest/eviction/config/shader-validate); probe boots 0 PANICs, night+day Fuji clean, no stuck dither at steady state. LOOP QUEUE next: [7] bookmark studio arc (maps-page teleport integration + live POV preview while editing + time/weather scrub), then [8] LOD ladder per size category (can now use the fade channel), [9] light clustering, [10] audio engine, [11] decoupling 3-5.


## undated

**Decision:** v0.921.0 OPERATOR-DIRECTED FIX - god rays respect planetary occlusion (report: rays leaking around/through Earth from the station night side, immersion-breaking). ROOT: the screen-space godray pass marches depth toward the sun SCREEN position - it cannot know about occluders outside the frame; with the sun off-screen behind Earth, far-depth sky pixels read lit and shafts leak. FIX: geometric truth on the CPU - renderer::godrays::segment_sphere_visibility (segment vs sphere, smoothstep 3% past the limb, 5 unit tests incl. the exact station scenario) folded via lib.rs sun_occlusion_factor across ALL solar bodies into godray_scale at both call sites. Ground-level works free (sun below horizon = 0), Moon transits dim rays free. DECOMPOSITION FINDING for the operator screenshots: after the fix a faint warm sky band remains that is SUN-INDEPENDENT (identical across 8-game-hour sun sweep; survives milkyway-glow toggle off) = the GALACTIC BAND authored in the starfield texture - physically real for night-side orbit. The operator bright wash = leak stacked on that band. If the band itself reads too strong, it is a starfield-texture/sky-settings taste call, NOT a bug - flag to operator. NOTED FOLLOW-UP: eclipse LIGHTING (sun still lights the station on the night side? not verified) is a separate feature gap, not addressed here. Loop queue unchanged: [7] bookmark studio arc next.


## undated

**Decision:** v0.922.0 + v0.923.0 OPERATOR FIELD-REPORT BATCH (4 items, 3 read-only scout agents fanned out for diagnosis, fixes shipped serially through the hot files): [1] OCEAN NEAR-FIELD REWORK v0.922 - operator: analytic shading just is not cutting it (zebra stripes 800m, moire rings 6m). Root: trig has no mip chain + fixed frequencies interfere coherently. Shipped: procedural 2048^2 tiling random-phase wave texture as ground_tex LAYER 8 (zero new bindings, threaded boot gen ~0.3s, RG=slopes B=crest), ocean_tex_gradient samples 2 scrolled octaves (16m+64m, camera-anchored metre domain, explicit mip LOD) replacing the FINE analytic trains + micro ripples for SHADING (W1-3 swell keeps analytic; ALL SIX still displace in VS - CPU twin + lockstep test untouched); slope soft-clamp kills normal flips (zebra mechanism) at any sea state; foam rides the texture crest channel. TUNING LESSON: texture slopes are channel-normalized - physical steepness (4-14deg calm-storm) applied in shader; first attempt at ~0.75 slope swung Fresnel white/dark = white chaos. ALSO: rig shader hot-reload does NOT work (no watcher events; every shader iteration needs a full rebuild - do not trust the docstring). Sea-pin fix: {sea:x} override now encodes +2.0 = pinned, bypassing max() with the MODIS storm cell (could never calm a storm before). [2] DESCENT HANG v0.922 - scout EXONERATED the v0.917 depth bake (4-tap bilinear, budgeted 8/frame, sub-ms); real culprit = synchronous terrain build loop saturating at LOD activation (64x ~1ms builds/frame for seconds). Fix: 3ms wall-clock cap on the build loop; backlog spreads, v0.920 crossfades smooth it. [3] MOMENTUM v0.923 - scout mapped the decouple: (a) flight-band Space thrust along world +Y = TANGENTIAL at equator (slides not lifts) -> new fly_wish_dir_up thrusts along LOCAL RADIAL; (b) the 100-1000km blend band rode only (1-s) of the spin delta (v0.890) shedding ~465 m/s from 100km up -> now FULL ride through 80% of the band, fade last 20%, inertial ISS regime beyond 1000km unchanged (real physics there). [4] VEGETATION v0.923 - roots: tree_model_distance DEFAULT 0.0 (any regenerated config turned trees off - now 120, test-locked) + the bare-forest interaction (models fail to draw -> cards hidden anyway -> bare ground; drawn==0 now hides nothing). LOD ladder stage 1: renamed model-stage slider + NEW silhouettes-out-to slider (shadow-uniform params2.x, shader far-discard). QUEUED next rungs: billboard mid-stage, grass far-cutoff (needs a grass bit in packed UV), shrub category, animal ladder, per-stage crossfades on the v0.920 fade channel. Probes: calm 800m deep-blue rippled ocean, 6m storm dark chop w/ organic foam streaks, Fuji forest 273 near trees w/ trunks+shadows on a FRESH default config. LOOP QUEUE continues: bookmark studio arc, then the remaining ladder rungs.


## undated

**Decision:** LOOP MODE RE-ENABLED (operator, going AFK frequently) with enriched mandate: (1) always consider a better approach before extending the current one; (2) MAX QUALITY first, tune every hang to max efficiency WITHOUT visual compromise; (3) dev-aids are first-class work (easier AI+human development = the infinite-of-x / GUI-first principle). Saved to memory feedback_max_quality_and_dev_aids.md. REORDERED LOOP QUEUE: [1] FIX SHADER HOT-RELOAD (dev-aid multiplier - the rig watcher never fires, discovered 2026-07-21; every shader iteration costs a 3.5min rebuild; diagnose watch() wiring + junction/notify on Windows), [2] THREADED terrain patch builds (worker pool; removes the descent cost entirely instead of the 3ms cap), [3] selection skip when parked + PatchBounds cache (several ms/frame), [4] sky-view LUT / sky-as-radiance (research item 5; one consistent sky the water reflects), [5] LOD ladder rungs 2+ (billboard mid-stage, grass bit in packed UV, shrub/animal categories, per-stage crossfades), [6] bookmark studio arc, [7] light clustering 1000+, [8] audio engine integration, [9] decoupling steps 3-5. Protocol unchanged: one item per iteration, ship end-to-end, probe proof, journal, 60s wakeups.


## undated

**Decision:** v0.924.0 LOOP (dev-aid mandate, queue item 1) - MEGASHADER HOT-RELOAD shipped + probe-proven: saving assets/shaders/pbr_simple.wgsl in a RUNNING game revalidates (naga parse+validate+entry-point pin, shared shader_loader::validate_wgsl - bad saves REJECTED with a log, never crash) and rebuilds the 4 PSOs in place (Pipeline::recreate_pipelines REUSES bind group layouts so live bind groups stay valid; world untouched). Proof: ocean painted RED live over the Mediterranean, reverted to blue, 2 recompile lines. KEY LESSONS: (a) the ShaderLoader watch/poll scaffolding existed since birth but was never CALLED anywhere; (b) the notify filesystem watcher delivers ZERO events through the rig NTFS junction - on the junction path AND the canonicalized real path (2 build cycles burned proving it) - detection is now a 1 Hz MTIME poll (free, alias-proof, editor-proof); (c) the poll call must live with the UNCONDITIONAL per-frame debug polls (first placement inside the celestial block never ran); (d) PSO rebuild = ~30s FXC / ~5s DXC - keep dxcompiler.dll+dxil.dll in the rig (staged now); async swap is a follow-up. Iteration loop for shader work drops from 3.5min rebuild+reboot+re-entry to seconds with state intact. docs/dev/adding-shaders.md documents the workflow. LOOP QUEUE next: [2] threaded terrain patch builds (worker pool - removes the descent cost entirely), then [3] parked selection skip + PatchBounds cache, [4] sky-view LUT, [5] LOD ladder rungs 2+, [6] bookmark studio, [7] light clustering, [8] audio, [9] decoupling 3-5.


## undated

**Decision:** v0.925.0 OPERATOR REPORT FIX - hard black shell around Earth from orbit: the v0.913 daylight star-occlusion day term (geometric sun-elevation at the camera) is a GROUND rule but applied from ORBIT it forced ~98.5% alpha on the whole shell incl. the near-zero-radiance outer limb = opaque black ring swallowing the starfield. Gated by (1 - max(w_alt, w_far)) - ground bit-identical (gate=1 there), from space the limb occludes stars only via its own sky_lum brightness -> smooth fade to stars. FIRST FIX SHIPPED VIA THE v0.924 HOT-RELOAD: edit + mtime touch swapped PSOs in the RUNNING probe in 3.3s (DXC dlls staged in rig - keep them there), iterated at the operator exact 665km vantage, zero rebuild cycles. Probe: limb=thin blue band fading into stars; Sahara ground noon rechecked zero stars. NOTE the mtime-capture nuance: the boot snapshot takes the CURRENT file mtime, so an edit made BEFORE launch needs one touch after boot to trigger. Loop resumes at queue [2] threaded terrain builds.


## undated

**Decision:** v0.926.0 LOOP (queue item 2) - THREADED TERRAIN BUILDS: build_patch_mesh fans across a scoped worker burst (available_parallelism-2, atomic work index, 4ms deadline w/ per-worker progress guarantee) - the whole 64-patch frame budget completes in one ~4ms slice vs ~60ms serial; GPU upload + cache insert stay frame-thread. BETTER-WAY CALL: rejected the persistent async pool (would Arc-ify heightmaps/albedos/tiles across dozens of sites for ~10% more win); scoped burst = zero ownership churn, borrow-safe by construction (workers read-only, frame thread waits in scope so no alias with tile poll). ENABLER: TerrainTiles mpsc ends Mutex-wrapped (std channels !Sync; locks touched only by the frame thread) making &TerrainTiles Sync; poll() drains to a batch before mutating. PROBE: cold-region arrival (never-visited coast = worst-case storm) holds ~15.6fps DURING the build storm (was a multi-second hang); settled 10fps at that vantage = GPU draw cost of 12.9k patches - owned by next items. 101 terrain tests green. LOOP QUEUE next: [3] parked-selection skip + PatchBounds cache (several ms/frame at rest), then [4] sky-view LUT, [5] LOD ladder rungs 2+, [6] bookmark studio, [7] light clustering, [8] audio, [9] decoupling 3-5.


## undated

**Decision:** v0.927.0 OPERATOR REPORT FIX - frame-lock proximity auto-switch: flying Moon->Earth manually never engaged Earth re-entry because frame_lock_body changed ONLY on teleports (the departure body frame persisted). Now a per-frame sweep rates every solar body by envelope ratio (alt / per-body inertial-blend ceiling - Earth 1000km, Moon ~273km, same v0.909 band_k scaling): enter below 0.9x (fresh anchor captured at current pos - no jump), release beyond 1.2x, hysteresis kills flap, station_ride keeps precedence. Makes the operator standing rule automatic (Earth frame at Earth, Moon at Moon, ship aboard ship). Probe logs proved all three arms: engage-from-None at world entry, release at 150000km over the Moon, teleports unaffected, 0 flap, 0 PANICs. ~15us/frame. NOTE the engage log wording says switched for engage-from-None too. LOOP QUEUE unchanged: [3] parked-selection skip + PatchBounds cache next, then sky-view LUT, LOD ladder rungs, bookmark studio, light clustering, audio, decoupling.


## undated

**Decision:** v0.928.0 LOOP (queue item 3) - PARKED-SELECTION SKIP: both ~30k-node LOD selections (terrain + water) skip while the camera is parked in co-rotating surface mode (local pose static <0.25m + view dot >0.99999 + params unchanged) with NO invalidation - builds, ALL THREE eviction sites, tile arrivals, settings, outstanding requests, partial coverage each force a fresh walk. Inertial orbit never skips (planet turns beneath). Selection derives Clone; ChunkState carries last_selection + pose + sel_dirty; reuse means identical draws so the v0.913 hysteresis/fades/eviction-guards see an unchanged steady state by construction. Probe: Fuji parked settled 25.9fps (class ran ~18-22 before), scene identical, 0 PANICs, 28 tests green. LOOP QUEUE next: [4] sky-view LUT / sky-as-radiance (research item 5 - one consistent sky the water reflects, stars occluded by physics not the alpha patch), then [5] LOD ladder rungs 2+, [6] bookmark studio, [7] light clustering, [8] audio, [9] decoupling 3-5.


## undated

**Decision:** v0.929.0 OPERATOR REGRESSION HOTFIX - v0.927 auto-switch broke spawn/return-home (floating in space above Earth): the home station orbits at 400km INSIDE Earth 1000km envelope, so any frame without station_ride set (spawn placement, docking) let the switch lock Earth with a garbage anchor (the spawn-time envelope -6.37 log line WAS the smoking gun - I noted it as harmless; LESSON: a negative/degenerate metric in a probe log is never harmless, chase it). Fix: (1) auto-switch yields while aboard_station OR station_ride; (2) candidates with ratio <= 0 (camera at/inside the body radius = degenerate placement) never engage. Genuine flights untouched (0.9 enter / 1.2 release). Probe: spawn lands in room_1 interior, no FrameLock line at entry, 0 PANICs. Loop resumes at [4] sky-view LUT.


## undated

**Decision:** v0.930.0 OPERATOR REPORT FIX - the 10s hang at the whole-disc altitude (both directions: fly out past 1.5R, or teleport far from the surface): NOT the sky-sphere as first suspected (chunked_on caps it at level 7 inside 1.5R) - the DEACTIVATION SHRINK was O(N^2): full-cache rescan per eviction x ~15k evictions on ONE frame (probe repro at operator scale: 20,009 patches / 790MB = the exact freeze). FIXES: (1) collect_evictions linear (one pass + one sort, 2048/call cap); (2) departure shrink amortizes every frame while over the warm floor, frame stamp ticks while deactivated so the 120-frame recency guard ages out (drain trickles ~19/frame as entries age - full drain ~1min, zero fps impact; could advance the stamp faster if we ever care); (3) BONUS: sky-sphere level 7+ builds moved to a background thread w/ progressive lower-level fallback, enabled by Arc-wrapping planet_heightmaps/planet_albedos (the refactor threading asked for twice - done now, ~19 sites). Probe: 30.0 fps immediately after the crossing that froze 10s, batches drain in log, 0 PANICs. LESSON: an O(N) scan inside an eviction loop is O(N^2) waiting for a big cache - grep for min_by_key-inside-while when hunting one-frame hangs. Loop resumes at [4] sky-view LUT.


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated


## undated

**Decision:** Trees-near-render root cause: the vegetation green-dominance gate (green > red*1.04) rejected brown-green Blue Marble texels. Raw linear vegetation is often brown-green (Tasmania forest r/g 1.09, Kansas prairie 1.01, operator hills 1.08) while barren is r/g >= 1.44 (Gobi..Outback). Fix: shared veg_biome_ok() helper (red < green*1.25 AND green > blue*1.04) used verbatim by BOTH the card bake and the near-model harvest so the streams cannot drift. Found via new per-gate rejection autopsy in the [NearTree] log (permanent diagnostics) + biome_gate_separates_vegetated_from_barren regression test over the shipped albedo. Ships as v0.955.0.


## undated

**Decision:** Operator decisions: (1) billboard textures = BAKE OUR OWN from 3D models (automated Total Annihilation-style sprite bake; benefits modders, zero manual art per new model) - alpha-card LOD rung unblocked; (2) CC0 audio APPROVED - Kenney.nl download greenlit, audio arc unblocked (operator sourcing music from others separately). Both queue AFTER the three field reports (trees DONE-pending-verify, atmosphere limb, ocean waves).


## undated

**Decision:** Atmosphere-limb fix (field report #2, southern Australia hidden from space): the v0.912/0.913 daylight star-occlusion boost (alpha_occ = max(alpha, sky_lum*4.5, day)) applied to EVERY type-14 shell fragment, including rays that hit the planet. Near the disc edge the bright limb in-scatter saturated alpha to 1.0 and painted flat blue over the continent. Stars only sit behind rays that MISS the planet, so the boost is now gated by !hits_surface; surface-hitting rays keep pure transmittance alpha (~0.5 at those angles = readable land through physically blue haze, like real limb photos). NEW permanent vantage australia-limb-12000km (showcase time 4.2 lights 130E) guards this. Blue-marble + limb-400km compared before/after: approved look preserved. Ships v0.956.0.


## undated

**Decision:** Ocean wave height (field report #3, still just a flat 2D shape): root cause was mesh resolution, not shading - WATER_MAX_PATCH_DEPTH 14 gave ~38 m vertices, so the 6-50 m chop trains could not exist as geometry and ALL visible wave detail was fragment fiction. Fix: depth cap 14->17 (~4.8 m verts near the eye, pixel-driven so only near-camera refines), WATER_MAX_LEAVES 144->256 (MAX_OBJECTS is 16384, the small-slice comment was stale), chop amplitudes to Beaufort-4-ish (W4 0.22->0.45, W5 0.12->0.35, W6 0.05->0.1), and NEW shoal damping (smoothstep 0.4..7 m of baked depth) in the vertex stage + CPU twin (wave_height_shoaled_m; float clamp passes drawn-seafloor depth) so taller chop never stabs through beaches. Lockstep test enforced both sides. VERIFIED: storm capture shows real wave bodies + foam; calm shows fine relief; fps 84-88 at the grazing vantage. KNOWN + PRE-EXISTING (A/B proven, NOT from this change): a white/black sheet band at the horizon at grazing angle - present in 07-23 and 07-25-morning captures with OLD code, unchanged when amplitudes reverted live via hot-reload. Filed as next ocean queue item. Ships v0.957.0.


## undated

**Decision:** Grazing horizon sheet band ROOT-CAUSED + FIXED: the white/black slabs riding the horizon in every ocean vantage were the CLOUD DECK seen edge-on - proven by the new clouds:0/1 showcase IPC knob (clouds off = clean horizon, on = slabs). A deck fragment at the visual horizon sits behind ~160 km of air (sqrt(2Rh)) and should dissolve into haze, but all three cloud variants only had a limb fade flooring at 0.55. Fix: shared cloud_low_cam_haze() - slant-distance fade 30..80 km, active ONLY when the camera is inside the deck shell, so the from-orbit blue marble + limb-400km cloud faces are bit-untouched (verified by sweep). Hot-reload A/B verified at the artifact conditions (time 10.7): clean horizon with clouds ON. The clouds knob is a permanent dev-aid. Ships v0.958.0.


## undated

**Decision:** Billboard bake increment 1 SHIPPED (operator call: bake our own, automated TA-style): src/renderer/billboard_bake.rs renders any model parts side-on (unlit albedo + 0.5 cutout alpha, transparent background, ortho framed on joint AABB, swapchain-format target so read_texture_to_png applies unchanged) and returns the world footprint for card sizing. IPC bake:trees bakes all 6 conifers (crown+_bark) to debug/bakes/*.png in <1s total. Verified: pine sprites are perfect billboards; fir reads sparse but is FAITHFUL to the wispy model (same cutout threshold as type-19). Deliberate: lighting stays with the consumer (baking it would freeze one sun angle into every card); one side view since the card stream yaws randomly. Increment 2 = atlas upload + textured alpha-cards replacing the colored-quad silhouettes, where resolution/threshold tuning belongs. Ships v0.959.0.


## undated

**Decision:** AUDIO ARC increment 1 SHIPPED (operator approved the CC0 download): 26 Kenney CC0 sounds (Interface/UI/Impact/RPG packs, ~3 MB total) imported to assets/audio/{ui,sfx} under the catalog names data/sounds.toml already defined (the catalog machinery existed with ZERO files + zero call sites). Wired: AudioManager::try_new (graceful None on no-audio-device machines - the old new() panicked), constructed in EngineState; Settings master/music/sfx sliders now HONEST (pushed to kira on change, not placebos); first real call site = UI click sound on egui-consumed presses via the catalog; [sfx.mining_hit] catalog entry added (impactMining). Verified: 1150 lib tests including NEW shipped_sounds_decode_through_kira (catalog loads 103 entries, every shipped file decodes through kira ogg - a silent first-click failure surfaces in CI not headphones), boot 0 panics + clean device init. NEXT audio increments: footsteps into the movement system (SurfaceType plumbing exists), menu open/close + notification call sites, ambient loops (need nature sources - Kenney has none), music when operator-sourced tracks arrive. Ships v0.960.0.


## undated

**Decision:** OVERNIGHT BACKLOG received (operator heading to bed, ~01:00): 10-item ranked queue persisted to PRIORITIES.md Active focus (liftoff bug top; dense forests; water depth-20 LOD; per-type LOD settings registry; zero-prior-knowledge content-creation docs; Library=all docs; archived-task review; leaf+space-dust particles; megashader file split R&D; homestead arc then NPC-AI stress tests). Standing frame reaffirmed: max graphics then optimize, efficiency = player/creature count ceiling (thousands concurrent as the bar). Workflows re-sanctioned by operator for parallelizable plain-worded work. Current in-flight: billboard increment 2 (sprite-textured tree cards, v0.961.0) building.


## undated

**Decision:** Billboard increment 2 SHIPPED: tree cards are textured sprites from the auto-baked atlas. Transport solved twice: normals are useless (VS rotates+normalizes) and the packed-uv fraction trick loses precision at the 2^17 flag base, so sprite cards use a NEGATIVE uv.x sentinel: |uv.x| = (1+tile) + u01*0.5 (tiny base = sub-texel f32 interpolation), uv.y = v01; real radial normals kept for lighting. Atlas = fixed 1536x1024 created zeroed at init, group-3 binding 14 (3 bind-group sites + layout + WGSL decl), bake copies in-place per the LUT no-rebuild convention; params.w bit 2 gates with a flat-green pre-bake fallback; one-shot attempted flag stops failed-bake re-parse loops. Emission: imagery planets emit ONE crossed sprite-quad pair per tree (tile = species*3 + (r5>>11)%3, matching near_tree_instances exactly); noise planets keep legacy colored cards. Probe: atlas ready in 0.8s, fuji ridgelines show real tree silhouettes at hundreds of meters where green slabs were. Forests read THIN now - density raise is backlog item 2. Ships v0.961.0.

