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

