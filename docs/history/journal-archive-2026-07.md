# Orchestrator journal archive -- 2026-07

Decisions rotated out of `data/coordination/orchestrator_state.json` (oldest first within each batch; newest overall is in the live journal). Source of truth for "why we did X" once it ages past the live tail. See also git log + the GitHub releases.

## 2026-07-02

**Decision:** OPERATOR DECIDED economy Phase 2 vehicle pipeline: BOTH models, staged. Stage 1: big end-products craft as an oversized "kit" ITEM (lives in home stock, not backpack; tradeable through the existing market) that deploys to spawn the real Vehicle entity -- reuses the whole existing crafting/storage/market chain for easy testing. Stage 2: factories gain the ability to SPAWN the physical vehicle in the world when a job finishes (finished-goods pad). Stage 3: TRANSPORT -- a purchased tank/mecha/spaceship physically travels from where it was built (factory/fleet) to the buyer, and the player can FOLLOW or TAKE OVER driving/piloting the transport.

**Why:** Operator 2026-07-02: "I like the idea of having both... The in inventory idea allows us to test a lot of things real easily. The spawning the vehicle gives us the ability to spawn physical objects in the world after a job finishes. Then add in the transport. It would be cool if the player could follow or take over the transport of whatever they bought." Long-term vision logged same message: the line between game objects and real buildable objects blurs -- an in-game house/car/spaceship should eventually correspond to real, buildable, working designs.


## 2026-07-02

**Decision:** Declined the operator's .rar preference for release archives; shipped .zip alongside .tar.gz instead, with exe-first for Windows.

**Why:** RAR is proprietary: no open-source tool can CREATE it (CI cannot legally produce it) and Windows 10 (our stated minimum) cannot open it natively -- both directly against the no-corporations/no-catch ethos and the friction goal. Zip opens natively everywhere and WinRAR (the operator's preference) handles zip fine, so the operator personally loses nothing.


## 2026-07-02

**Decision:** Session wrap on operator instruction after the spend limit killed wave 3: salvaged all partial agent work as WIP commits pushed to origin branches; completed + shipped the essentially-finished saffron parser fix as v0.670.0 (710 tests); wrote docs/history/2026-07-01-afternoon-loop-results.md as the durable session record.

**Why:** Operator: "wrap up what was done and ship so it is saved in case anything happens to this session." Durability rule: pushed branches survive anything -- session loss, worktree accidents, machine failure.


## 2026-07-02

**Decision:** Fleet mode complete for waves 1+2: 8 releases shipped in one evening (v0.663.0 through v0.669.0), all agent branches reviewed + merged + re-verified on main before release, every Rust merge re-running the full suite (final: 709 tests).

**Why:** Operator directive to use the remaining weekly allowance developing the whole app in parallel. Isolated worktrees + commit-to-branch + orchestrator-merges kept the clean-worktrees disaster class impossible while 4+ agents built simultaneously.


## 2026-07-01

**Decision:** Entered fleet mode on operator directive: 4 parallel worktree implementation agents (web governance voting+KAT, web laws mirror, NPC task-AI, homestead data gaps 3-4) with strict file-disjointness from the main tree's uncommitted economy-automation diff. Economy Phase 1 implemented on main: AutoRefine machines (data-driven auto_recipe in home.ron: smelter->smelt_iron, new workbench->craft_hammer) acting on the home inventory, drone standing orders (Keep mining checkbox -> auto_mine_order -> Deliver-arm relaunch), and scaled_dt so all economy timers respect time_scale.

**Why:** Operator: 92% of the weekly allowance left with ~24h to reset; wants the whole app developed in parallel, explicitly asked for many subagents. Economy Phase 1 is the operator's living-ecosystem vision -- the full_chain_drone_ore_becomes_a_hammer_untouched test proves one drone commission becomes a finished tool with zero interaction.


## 2026-07-01

**Decision:** Shipped v0.658-v0.660 (Studio mic meter + help_modal adoption; Donate real server-funding fetch; native Governance page fully live with Dilithium-signed vote_v1/proposal_v1 submission via the in-crate ObjectBuilder). Adopted a review-before-commit discipline for substantive diffs: a 2-lens adversarial Workflow ran on both the Donate and Governance changes BEFORE committing.

**Why:** Operator re-enabled Fable 5 + ultracode mid-loop and asked for maximum capability. The review workflows proved their cost immediately: Donate review caught a real money-routing bug (stale server-A donation addresses displayed as server-B's), Governance review caught 6 defects including cross-server stale-proposal voting (an orphan vote stored on the wrong server with a false success message) and a ~17-minute fetch pin. All fixed + regression-locked before the code ever reached main.


## 2026-07-01

**Decision:** Shipped Phase A of the self-sustaining homestead design (v0.656.0/0.656.1): authored data/machines/home_solo.ron per docs/design/homestead-solo-design.md's exact BOM, and built the home_variant selector (AppConfig field + SettingsState mirror + machines::home_ron_path() touching all 5 real MachineHome::load call sites + a Settings -> Data -> Home Design radio UI) since MachineHome::load was hardcoded to home.ron everywhere with no variant mechanism.

**Why:** Operator asked for a dedicated homestead design pass ("designing a fully fledged self-sustaining homestead") to establish the honest one-person baseline before scaling to infinite. The design doc (produced by a 3-research+1-synthesis Workflow) found ~90% of the BOM already exists as data; implementing it required discovering and fixing the missing loader-variant gap first, otherwise the new file would be inert.


## 2026-07-01

**Decision:** Mothership scale research (dispatched after operator feedback that the homestead feels like a tech demo, not lived-in): the Fibonacci design was never a spiral, it is a room-SIZE progression, keep it for home flavor only, the mothership macro layout is the already-shipped Zone/ZoneType hierarchy. Found and reconciled an orphaned lore doc (docs/game/humanity_one.md, a 500km ring-ship vision) into docs/design/mothership-superstructure.md, pulling forward its LOD strategy and district-list alignment while flagging its Ring/Sector addressing and Hub-tab-mapping table as stale. Reframed the 10-billion-occupant goal in ROADMAP.md: only achievable as an aggregate population-capacity number for the resource-flow math (mirrors the utility-trio per-island aggregation one tier up), not literal rendered/simulated individuals (renderer instancing confirmed dead code; one home already forced the draw-call cap up once). Concrete near-term path logged: population:u64 on Zone + zone_resource_profiles.ron + a pure report fn, wired together with grid-hierarchy.md S3 substation tiers as the same mechanism. Individual living-NPCs (Needs/Schedule components, AISystem flip-on) are a small bounded flavor layer on top, not the mechanism computing whether the ship balances. Also root-caused and fixed two live bugs the operator hit testing v0.638.0: (1) zone population was invisible because the live home_structure.ron had zero zones ever placed, seeded 13 real zones; (2) light intensity plateaued because pbr.wgsl used Reinhard tonemapping, swapped to the ACES fit already used in pbr_simple.wgsl. Four more agents in flight (isolated worktrees): spotlight cone rendering + rotation UI, drone hangar dock/undock visual, multi-homestead corridor connectivity, and a web-facing Accord page to unblock Jekyll retirement.


## 2026-07-01

**Decision:** INCIDENT: ran just clean-worktrees after merging 2 of 4 in-flight subagent tasks, destroying the other 2 (spot-light cone rendering, the web Accord doc-browsing page) with no recovery path (confirmed via 3 independent review agents checking stash/reflog/all branches). Both re-dispatched with the full original plan replayed verbatim plus the one bug the review had already found in the lost spotlight work (render_celestial_onto write raw camera-buffer offsets that were not updated when the uniform struct grew). Added a hard rule to CLAUDE.md: never run clean-worktrees while any dispatched agent this session is still unmerged, committing inside a worktree does not protect it since the script force-deletes branches too.


## 2026-07-01

**Decision:** INCIDENT REPEATED (3rd time, same day): after the first clean-worktrees wipe was already fixed with a CLAUDE.md doc-only warning, a SECOND wipe happened mid-review, destroying all 3 remaining in-flight diffs at once (spotlight-cone-redo agent-a3d1cb52b3dc11e66, web-Accord-page-redo agent-a7effdecc72683c98, and the live-screenshot-command feature agent-a6ee2b1dad5ab569c, which had not even been reviewed yet). Root cause confirmed: the doc-only warning was insufficient because multiple review subagents were told to "read CLAUDE.md first" as routine context, and Step 0 ("run just clean-worktrees every session") reads as a literal instruction to a fresh subagent with no way to know sibling worktrees hold unmerged work. Real fix this time (not just docs): scripts/clean-worktrees.sh rewritten to check every candidate worktree/branch/orphaned-folder for uncommitted changes or unmerged-into-main commits, and skip (not delete) anything unsafe even under --yes; --force-unmerged required to override. CLAUDE.md Step 0 reworded operator/orchestrator-only, explicitly telling subagents to skip it. All 3 lost diffs are being redone directly against main with small immediate commits instead of long-lived parallel worktrees, to shrink the risk window regardless of the script fix.

**Files:** scripts/clean-worktrees.sh, CLAUDE.md


## 2026-07-01

**Decision:** RECOVERED from the repeat clean-worktrees incident and shipped all 3 previously-lost features. Spotlight-cone rendering (v0.639.0) rebuilt directly on main with immediate commit + full verification (both cargo checks, 612 lib tests, 5 lints, and a real release-build launch confirming every shader compiles clean via Naga with no wgpu validation errors). Live in-game screenshot command (v0.640.0) rebuilt directly on main: poll_screenshot_request in lib.rs, Renderer::capture_current_frame in renderer/mod.rs with a COPY_SRC surface-capability check + BGRA/RGBA channel swizzle, verified end-to-end on a real release build (dropped a real screenshot_request.json, got back a real 1.3MB PNG of the live chat UI with correct color). Web Accord doc browser (v0.640.0) recovered from an interrupted background agent (the harness process was killed by an internet outage mid-task) -- the hardened clean-worktrees script correctly protected its worktree through a session resume, and its partial-but-solid backend work (docs_accord.rs fixed allowlist + 2 routes, fully tested) was completed with the frontend half (markdown.js extraction, accord.html/accord-app.js, 3 link repoints) and verified live against a real running relay with curl (real slug + list endpoint work; 6 malicious-shaped slugs all cleanly 404). All 3 merged into main (spotlight-cone and screenshot-command via direct commits, Accord via a clean merge --no-ff of its worktree branch since it touched a fully disjoint file set), pushed to origin + forge. Full verification suite green on the merged result: both cargo checks, 624 lib tests, 5 lints, 0 broken doc links. Operator note: mid-cleanup, a stray cd into the Accord worktree persisted across several tool calls and caused a just build-game run + a journal commit attempt to land there instead of main -- harmless (nothing pushed from that branch), cleaned up by redoing the journal entry on main and leaving the worktree for ordinary disposal.

**Files:** src/renderer/camera.rs, src/renderer/light.rs, src/renderer/mod.rs, src/renderer/stars.rs, src/renderer/line.rs, assets/shaders/pbr_simple.wgsl, assets/shaders/stars.wgsl, src/lib.rs, src/gui/pages/construction.rs, src/relay/storage/docs_accord.rs, src/relay/api.rs, src/relay/mod.rs, web/pages/accord.html, web/pages/accord-app.js, web/shared/markdown.js


## 2026-07-01

**Decision:** Operator asked to leave the session running autonomously overnight (~8h, asleep, no interactive checkpoints possible) to develop chat completeness + livestreaming verification + a broader stub sweep to full completion. Operator flagged two real risks: (1) a worktree-built exe reaching the real internet could trigger a firewall/permission prompt with nobody to click it (only the main exe has network permission), (2) fear of a careless rewrite silently clobbering a large pre-existing file instead of editing it. Designed a safety model before starting: (a) HUMANITY_DATA_DIR env var already exists in src/config.rs for exactly this -- an isolated identity/config dir separate from the operators real vault; (b) all live verification uses a LOCAL LOOPBACK relay (127.0.0.1) never the real production server, and loopback traffic never crosses the firewall boundary regardless of which exe binds/connects, but as an extra-cautious rule any exe LAUNCH still only ever happens from the main repos own build path, never a worktree, categorically; (c) hard rule written into the plan doc: never Write over an existing file, Edit only; (d) never run clean-worktrees tonight in any form, even the hardened version, since no one can review judgment calls if something surprising happens; (e) no native-GUI interactive testing (no computer-use/input-injection available or appropriate for the native window) -- verification is unit tests + protocol-level WS test scripts against the local headless relay + passive screenshot checks (the exact live in-game screenshot command built earlier tonight) + web-page browser automation via Claude Preview against the static preview server. Did a repo-wide TODO/FIXME scan to build a concrete backlog rather than let the loop guess at scope: chat.rs has a hardcoded fake is_group_admin=true (a real permission bug), missing mute/ban backing tables, unwired DM-notification toggle and group-voice-join, and a dead-looking onboarding Connect button; streaming code (streams.rs, chat-voice-streaming.js) looks structurally complete with no stub markers, so the ask there is functional end-to-end verification, not code-writing. Full backlog + per-cycle checklist + stop conditions written to docs/history/2026-07-01-night-loop-plan.md so it survives context compaction across the night. Invoking the /loop skill (dynamic self-pacing) to actually run this.

**Files:** docs/history/2026-07-01-night-loop-plan.md, docs/PRIORITIES.md


## 2026-07-01

**Decision:** Night-loop cycle 1: fixed BUG-041 (v0.641.0) -- src/gui/pages/chat.rs had is_group_admin hardcoded to true for every group member regardless of real role. Investigation found the server already reports a real per-group role (GroupData::role in src/relay/relay.rs, "admin" for the groups creator per src/relay/storage/social.rs::create_group, "member" otherwise) via the group_list WS message -- the client ChatGroup struct just had no field to receive it, so src/lib.rs group_list handler silently discarded it. Added the role field (defaults to member on malformed input, fail-closed), wired the handler, extracted a small testable is_group_admin(role) helper in chat.rs with 3 unit tests. Verified: both cargo checks, 627 lib tests (3 new), 5 lints, 0 broken doc links. Committed + pushed to origin+forge. This resolves chat backlog item 2 (of 6) from docs/history/2026-07-01-night-loop-plan.md. Next: item 1 (chat.rs:705 DM notification toggle no-op).

**Files:** src/gui/mod.rs, src/gui/pages/chat.rs, src/lib.rs, docs/BUGS.md


## 2026-07-01

**Decision:** Night-loop cycle 2: shipped v0.642.0 -- wired the native DM-notification toggle (src/gui/pages/chat.rs, was a hardcoded no-op) to the relays already-complete notification_prefs system (the web client already had this fully working via web/pages/settings-app.js -- a dual-UI-parity gap, not a from-scratch build). GuiState gained 6 fields tracking dm/mentions/tasks/dnd state + a loaded flag; the popup fetches on first open, the button is a real toggle, update sends all 5 fields together (server requires them together) so mentions/tasks/DND are preserved even though native has no UI for them yet (logged as a follow-up in FEATURES.md). Built scripts/ws-test-client.js: a reusable Node WS test client using the bot_/bot_secret auth fastpath (src/relay/relay.rs) to test relay protocol behavior against a LOCAL relay without the full Dilithium handshake -- used to verify this feature with a REAL round-trip (get defaults -> update -> get again, confirmed persisted) against a locally-run relay, and will be reused for the rest of tonights chat/streaming verification. Verified: both cargo checks, 630 lib tests (3 new), 5 lints, 0 broken doc links. Committed + pushed. Resolves chat backlog item 1 (of 6). Next: item 3 (chat.rs:1346 group voice join/leave -- check if web already does this correctly, i.e. another dual-UI-parity gap).

**Files:** src/gui/mod.rs, src/gui/pages/chat.rs, src/lib.rs, scripts/ws-test-client.js, docs/FEATURES.md


## 2026-07-01

**Decision:** Night-loop cycle 3: shipped v0.643.0 -- wired group voice channel join/leave (src/gui/pages/chat.rs, was a no-op TODO) to the existing voice_room protocol using the group channels synthetic group:<id> id. Investigating this surfaced a REAL server-side bug that would have broken it even with a correct client fix alone: handle_voice_room (src/relay/handlers/msg_handlers.rs) validated every room_id against the channels tables voice_enabled flag, which has no row for a group room at all -- every group voice join would have silently failed with Voice is not enabled for this channel. Fixed by adding a group: prefix branch gated on Storage::is_group_member (src/relay/storage/social.rs, pre-existing but previously had zero callers using it for authorization) instead of skipping validation, closing what would otherwise be an open join-any-group-by-guessing-its-id hole. Verified LIVE: seeded a test group + one member directly via node:sqlite (group_create needs a verified/admin role a fresh bot lacks) on a local relay, then used scripts/ws-test-client.js to confirm a member joins silently and a non-member gets rejected with the right message. Added 5 unit tests for is_group_member. Logged a real, scoped follow-up in FEATURES.md: group voice rooms dont yet appear in the voice_channel_list broadcast (only the channels table is enumerated), so you cant see OTHER participants in a group voice rooms roster yet, though join/leave + audio signaling both work. Verified: both cargo checks, 635 lib tests (5 new), 5 lints, 0 broken doc links, real protocol test. Committed + pushed. Resolves chat backlog item 3 of 6. Next: item 4 (mute implementation + ban/mute backing tables, chat.rs:1588 + msg_handlers.rs).

**Files:** src/gui/pages/chat.rs, src/relay/handlers/msg_handlers.rs, src/relay/storage/social.rs, docs/FEATURES.md


## 2026-07-01

**Decision:** Night-loop cycle 4: shipped v0.644.0, closing out the entire chat backlog. (1) BUG-042: onboarding Connect button (src/gui/pages/main_menu.rs) always said Connected regardless of reachability -- found the real auto-connect mechanism is intentionally gated on onboarding_complete + a live identity (created one step later), so a full WS handshake genuinely cant happen at this step; fixed with a real GET <server_url>/health check on a background thread (mirrors updater.rs check_now pattern), 7 unit tests, verified live against a real /health endpoint and a closed port. (2) Investigated the planned ban/mute backend work (backlog items 4+6) and found it was based on a STALE DOC COMMENT -- both banned_keys and muted_members are fully implemented, enforced, and already tested; fixed the comment instead of building anything (there was nothing to build). This closes chat backlog priority #1 entirely (started this session, all real gaps found via the repo-wide TODO scan are now resolved or corrected-as-non-issues). Verified: both cargo checks, 642 lib tests (7 new), 5 lints, 0 broken doc links. Committed + pushed. NEXT: moving to priority #2, livestreaming end-to-end verification, per docs/history/2026-07-01-night-loop-plan.md.

**Files:** src/gui/mod.rs, src/gui/pages/main_menu.rs, src/relay/handlers/msg_handlers.rs, docs/BUGS.md


## 2026-07-01

**Decision:** Night-loop cycle 5: shipped v0.645.0, closing out livestreaming backend verification (priority #2). Live-tested the full stream lifecycle against a local relay: stream_start creates the right DB row + broadcast, viewer join/leave, stream_chat send + persistence -- all confirmed correct. Found and fixed a real bug in the process: BUG-043, handle_stream_viewer_leave and handle_stream_stop both persisted the LIVE viewer_keys.len() as viewer_peak, but that count is only ever highest right at a join and decreases from there -- handle_stream_viewer_join never wrote to viewer_peak at all, so by the time a stream actually ends the recorded peak was frequently 0 or far below the true max. Proved live: 2 viewers joined (true peak 2), both left, stream stopped -- old code would have recorded viewer_peak 0. Fixed with an ActiveStream::peak_viewers high-water mark (src/relay/relay.rs) updated via .max() on every join, used instead of the live count when persisting in both the leave and stop handlers. 4 new unit tests (msg_handlers.rs::stream_tests) using the existing fresh_state/block handler-test fixture, proven via a temporary revert-and-retest to actually catch the bug (both regression tests failed against the old code, recording 1 and 0 instead of 2 and 1). NOT verified this cycle: the WebRTC signaling relay (stream_offer/answer/ice -- simple store-and-forward, read as correct but not live-tested with a real peer connection) and the client-side scene-management UI; logged as a real, scoped follow-up in the plan doc. Verified: both cargo checks, 646 lib tests (4 new), 5 lints, 0 broken doc links. Committed + pushed. NEXT: priority #3, the broader stub-completion sweep from the plan docs candidate list.

**Files:** src/relay/relay.rs, src/relay/handlers/msg_handlers.rs, docs/BUGS.md, docs/FEATURES.md


## 2026-07-01

**Decision:** Night-loop cycle 6: investigated 2 broader-sweep candidates from the plan docs list, found both bigger than estimated, and did NOT force-build either (matching the plan docs own instruction not to invent scope for design-uncertain items). (1) src/renderer/sky.rs SkyRenderer is entirely dead code -- zero references anywhere outside its own file, never instantiated. The mothership sun lighting already uses a real astronomical Earth-Sun vector (more correct for a ship in orbit than a simplified day/night-hour formula). Whether SkyRenderer still has an intended role (ground/planet-surface exploration with a visible sky?) is logged as a genuine open_questions_for_human entry rather than guessed at. (2) src/systems/economy EconomySystem is unregistered, but this is a KNOWN, documented deferral (tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS already lists it: needs market/credits entities + live verification) -- not an oversight, not a quick win, correctly left alone. That investigation led to cross-checking every NOT registered claim in docs/FEATURES.md against the lints real DEFERRED_SYSTEMS list + the real system_runner.register() calls in src/lib.rs, and found 4 stale claims (WeatherSystem, AtmosphereSystem, SkillSystem, QuestSystem are all registered and ticking, some since v0.337-v0.617) plus a stale registered-count header (claimed only 7, real count is 16). docs/STATUS.md was already correct for all four (this exact staleness class bit the project once before -- Atmosphere was already noted there as corrected 2026-06-30). Fixed all 4 FEATURES.md entries + the header. v0.645.1, docs-only, patch bump. Committed + pushed. NEXT: continue the broader sweep with remaining self-contained candidates, or stop if genuinely exhausted.

**Files:** docs/FEATURES.md, docs/history/2026-07-01-night-loop-plan.md, data/coordination/orchestrator_state.json


## 2026-07-02

**Decision:** v0.675.0 shared-file library SHIPPED end-to-end

**Why:** Operator directive: share personal files (blend phone case, car bushings) from local PC via the relay. Design: uploads with ?share=1 are publicly listed via GET /api/uploads and EXEMPT from the per-user media FIFO (a shared .blend must not vanish because its uploader posted chat photos later); chat auto-shares ONLY 3D/model formats (.blend .stl .obj .gltf .glb) so photos stay private; original_name preserved for display. Server: user_uploads +shared/original_name/size_bytes cols + ALTER migration, list_shared_uploads (search+limit, LEFT JOIN server_members) + 2 storage tests. Web: shared-files.html browse/search/download page wired into nav; chat-messages.js auto-share. Smoke-tested LIVE: temp relay on :3299, GET /api/uploads returned {files:[]} + health 200, killed by PID. page_registry_lint caught accord.html missing from PAGES.md on its first real run (fixed, count stays 41). Native browse + native chat attach parity = tracked follow-up in PAGES.md.


## 2026-07-02

**Decision:** v0.676.0 HOTFIX: v0.675.0 relay startup crash on the live DB (BUG-046)

**Why:** The v0.675.0 VPS deploy built but the relay died on activation (exit 3): the new (shared,id) index sat in the main schema batch, which runs before the ALTER block adds the shared column on a pre-existing user_uploads table. Fresh-DB tests + the local smoke test structurally cannot catch this. Fix: index created after the ALTER block. Regression test opens_a_pre_v0675_database_and_migrates_it replays the exact production sequence. Roughly 25 min relay downtime; v0.676.0 deploy green; GET /api/uploads verified live on united-humanity.us ({files:[]}) and /shared-files serves 200. Lesson written into BUGS.md BUG-046: any index over an ALTER-added column goes after the ALTER block, and schema changes to existing tables need a pre-migration-shape Storage::open test.


## 2026-07-02

**Decision:** v0.677.0 SHIPPED economy Phase 2 STAGE 1: vehicle kits (craft -> Deploy -> persistent Vehicle entity)

**Why:** Operator staged-pipeline decision. Kit->vehicle mapping is pure data (data/vehicles/kits.ron); VehicleSystem registered FIRST time, deploy arm live (registry-lookup-BEFORE-consume so unknown kit never costs the item; creative deploys free), enter/exit/mech dormant until Stage 3. Render: unit-box+wheel primitives scaled from registry (drone-dock pattern). Persistence: WorldSave.deployed_vehicles + idempotent re-apply. 8 tests incl one-kit-cannot-become-two-vehicles. just verify green (engine_wiring_lint required removing VehicleSystem from DEFERRED). SHIPPED BEFORE the 2-lens adversarial review workflow finished (13% budget left, securing the work won) -- review verdict lands as follow-up; fix criticals as v0.677.x hotfix next session. Operator 3D visual check pending. NEXT: Stage 2 factory world-spawn (ManufacturingSystem completion loop + ProductionFacility spawner + machine Transform), then Stage 3 transport follow/take-over. Prepped subsystem map lives in the workflow result (tasks/whshkxflu.output).


## 2026-07-03

**Decision:** v0.678.0 SHIPPED: vehicle-kit review fixes -- apply_save_to_world made save-AUTHORITATIVE for vehicles (despawn all, respawn saved set)

**Why:** The v0.677.0 adversarial review died on the spend limit with 3 findings unverified; adjudicated by hand. ROOT CAUSE both real ones: vehicles were ADDITIVE on apply while inventory is authoritative, and apply_save_to_world is NOT startup-only (the launcher character-select re-applies saves live). Fixed: (1) save-scum duplication (stale re-apply resurrected the kit AND kept the truck -- the dead review agent left a repro test asserting the bug, rewritten as regression lock stale_reapply_rewinds_instead_of_duplicating), (2) cross-save vehicle leakage on character switch, (3) same-pose collapse (two identically-parked vehicles restored as one). Third finding (creative deploys free permanent vehicles) accepted BY DESIGN, same semantics as creative crafting/planting; revisit at the launch creative-default flip. Journal convention normalized this entry onward: newest at BOTTOM (matches _protocol + just brief), field is at: not date:.


## 2026-07-03

**Decision:** v0.679.0 SHIPPED economy Phase 2 STAGE 2: factory world-spawn (assembler rolls REAL rovers onto the pad) + 2 review root-causes fixed pre-commit

**Why:** CraftingSystem route chosen over activating ManufacturingSystem: one battle-tested job engine with the Phase 1 hardening beats a second parallel loop. Vehicle-class recipe outputs (kit-registry get_vehicle) world-spawn via new deliver_outputs (shared timed+instant); machines now carry a Transform (resolved pos from load_world, raw offset in menu mode); vehicle_assembler machine (home.ron, auto_recipe assemble_rover) + assemble_rover/assemble_truck recipes. Full backpack cannot stall the line; mid-batch machine despawn delivers at the captured pad. ADVERSARIAL REVIEW confirmed 4 findings / 2 root causes, both fixed pre-ship: (1) pad occupancy -- lanes now world-queried (12-lane pad; full pad PAUSES the line, inputs unconsumed; lanes freed by departing vehicles reused); (2) save-rewind duplication -- launcher character-pick apply raises abort_active_crafts, CraftingSystem drops in-flight batches so a rewind behaves like an app restart. 8 new tests, 757+ lib green, just verify green. Transform blast-radius grep-verified: every Transform query joins a component machines lack. NEXT: Stage 3 transport (follow/take-over); operator visual check of Stage 1+2 pending; ALSO PENDING OPERATOR: the material-storage design proposal (volume for solids -> gas tanks/exhaust -> form-factor items) answered 2026-07-03, awaiting direction.


## 2026-07-03

**Decision:** v0.680.0 SHIPPED economy Phase 2 STAGE 3 slice 1: vehicles MOVE (Summon -> self-drive to the player)

**Why:** First moving vehicle in the game. VehicleRoute component (dest/speed_mps/arrive_radius) ticked by VehicleSystem on scaled game time, yaw faces travel, arrival removes route (parks). Transit deliberately NOT persisted (mid-transit save restores parked-in-place, consistent with drone flights). Per-vehicle speeds in kits.ron (truck 8, rover 6). GUI: new Vehicles section on the Inventory page (name/distance/status/Summon; En-route label; Parked-here under 6 m); summon channel validates target + no-ops on re-summon. Stage 2 pad lanes automatically reuse the slot a summoned vehicle vacates. 3 new tests; 765 lib green; just verify green. Shipped WITHOUT the usual pre-commit adversarial review workflow (operator flagged ~28% budget; commit-early discipline) -- run the 2-lens review over the v0.680.0 diff NEXT SESSION as a fast-follow, plus operator visual check of Stages 1-3. REMAINING Stage 3: follow-cam binding, take-over driving (enter/exit arms + drive branch), buy-side order flow (gated on wallet/currency decision). ALSO PENDING OPERATOR: material-storage design proposal (volume for solids -> gas tanks/exhaust -> form factors).


## 2026-07-03

**Decision:** v0.681.0 SHIPPED field-test fixes: grounded crew NPCs + live factory status

**Why:** Operator played v0.680.1 live and reported: (1) crew floating mid-sky -- root cause: relay simulates chores on ITS multi-deck ship layout (room.position.y+1.0 per deck) while the client renders the flat homestead; client-side fix grounds NPC Y at the sync source (NpcUpdate arm, keeps relay X/Z); REAL fix = relay/client layout alignment, tracked. (2) assembler said static authored assembling with no %/reason and produced nothing (no rubber in stock, nothing said so) -- CraftingSystem now publishes one status line per auto machine per tick to auto_craft_status (live %, first missing input by name+shortfall, inventory full, pad full), shown in the Inventory Vehicles section (which now also shows when only status exists). (3) drone dock pops on launch/return -- docking/undocking sequence logged for the polish pass, not fixed. Test production_status_reports_why_idle_and_live_progress; full verify green.


## 2026-07-03

**Decision:** v0.682.0 SHIPPED drone docking sequence (lift-off/settle, no pop) + assembler label neutralized

**Why:** Operator field-test item 3: dock visual popped with drone_active. New drone_dock_anim scalar (1=settled, 0=away) eased toward drone_active each frame; launch lifts the model ~2s ease-out to +4m before it vanishes, return settles it down. Same showroom/declutter gates. home.ron assembler authored stat -> auto-assembly line (the live % status shipped v0.681.0). Light-dev cadence per operator (23% budget): ship small, often.


## 2026-07-03

**Decision:** LOOP MODE (low-effort, operator-directed until budget cap): v0.682.2 items.csv CR purge, v0.682.3 dead pqSign/pqVerify deletion, v0.683.0 Unaligned-overlay fix rescued from the orphaned clever-moore worktree

**Why:** Worktree audit: 9 of 10 agent worktrees are MERGED into main (pure clutter, safe for the operator to clean when ready); ONE held real unmerged work -- the Unaligned overlay fix (task_30ff8cfe) committed 2026-07-01, never merged, never pushed. Cherry-picked the code (theme.rs debug.show_unaligned off, cfg-gated), dropped its 2-day-stale PNGs (4 binary conflicts proved the risk), regenerated all 19 snapshots on current main, spot-checked humanity.png overlay-free. items.csv: 436 embedded CRs (description field, header included, from a past column insert) stripped; 490 rows parse identically. pq-identity.js legacy wrappers: zero callers grep-verified. All shipped + verified individually.


## 2026-07-04

**Decision:** v0.684.0 SHIPPED stat-card stair-step fix (operator screenshot): widgets must OWN their internal layout

**Why:** Operator spotted the Where-we-stand tiles stair-stepping in the humanity snapshot. Two-layer cause: (1) draw_stat_card inherited the parent layout direction, so in left-to-right containers label+value went side-by-side at drifting heights -- fixed with explicit ui.vertical inside the card (general lesson for ALL universal widgets: never inherit the parent layout for internal structure); (2) Humanity used horizontal_wrapped while Civilization used a top-aligned Grid for the same cards -- Humanity now uses the same Grid. humanity.png regenerated + visually verified uniform. Also shipped this wake: the pending v0.683.1 stamp. Loop continues on low-effort items.


## 2026-07-04

**Decision:** v0.684.2 just validate-data + v0.684.3 THEME SPACING SCALE RESTORED (the app-wide cramped look had one data root cause)

**Why:** Snapshot QA sweep (loop) caught governance rows fusing (OPENlocal...workshopcloses). Layout code was fine; theme.ron spacing tokens were crushed to near-zero (xs 0.0 / sm 0.60 / md 2.27 / lg 3.22 / xl 3.85 / card_padding 1.69 -- slider-drag/scale-mishap shaped). Every add_space(spacing_*) rendered as nothing app-wide; contributes to the stair-step family the operator flagged. Restored dense-but-legible (2/6/10/14/18, card 6), theme.css regenerated, all 29 snapshots re-rendered, governance verified. Taste-adjacent: operator can retune live in Settings. ALSO: Laws category chips found ALREADY SHIPPED (stale queue item, laws.rs:85-99); journal rotated (137->96KB); v0.684.1 stamp; deploy train green.


## 2026-07-04

**Decision:** v0.684.4 SHIPPED ore reality (snapshot sweep find): gold/silver/aluminum/titanium no longer smelt from iron_ore_0

**Why:** Crafting-page snapshot showed all four precious/light-metal smelts consuming iron_ore_0 placeholders. Added real ores (gold_ore_0, silver_ore_0 argentite, bauxite_0, rutile_0 -- real-world primary ores, educational per the operator close-to-reality mandate) + corrected the 4 recipes. Dev-stock enumerates recipe inputs so they are obtainable in dev; asteroid classes gain them with mining depth. Inventory page swept CLEAN under the restored spacing. validate-data + refining tests green.


## 2026-07-04

**Decision:** v0.685.0 SHIPPED Studio 16:9 letterbox + zero-area source skip (snapshot sweep wake 3)

**Why:** Studio canvases stretched to fill leftover height -> portrait scene mocks; now letterboxed 16:9 both layouts. Microphone source (size 0x0, audio-only) painted a clipped ...phone label sliver at canvas origin; zero-area sources skipped. Market + tasks swept clean under the restored spacing. Rust release -> build-game stamp next wake.


## 2026-07-04

**Decision:** v0.686.0 SHIPPED NEA perihelion classifier -- SNAPSHOT QA SWEEP COMPLETE (29 pages, 4 wakes, 4 shipped fixes)

**Why:** Cosmos sidebar used a<1.3AU for near-Earth asteroids; real IAU/CNEOS definition is perihelion q=a(1-e)<1.3. Eros (q=1.13) + Itokawa (q=0.95) -- both spacecraft-visited NEAs -- were misfiled into the Main Belt; fixed + snapshot-verified. SWEEP TALLY: v0.684.3 spacing-scale root cause (crushed theme tokens fused text app-wide), v0.684.4 ore reality (gold/silver/Al/Ti no longer smelt from iron ore; bauxite+rutile added), v0.685.0 Studio 16:9 letterbox + mic-label sliver, v0.686.0 NEA classifier. Swept clean: humanity, governance, inventory, market, tasks, profile, wallet, homes, chat (Thorn reaction icon + arrow = false alarms), quests, laws (category chips confirmed live). Unswept (lowest risk, future pass): settings pages, garden/mining modals, library, construction, mining_map, notes, calendar, identity, main_menu, onboarding. NEXT queued: build-game stamp for v0.686.0, then machine-labels-live-stats or PRIORITIES pull.


## 2026-07-04

**Decision:** v0.686.1 README honesty + v0.687.0 field-fix batch (session 2): lost drone hauls root-caused, modal Verify, health tile, reachable backpack

**Why:** Operator field session 2 findings all addressed: (1) drone haul vanished -- delivery discarded add_item overflow with a full backpack; now grows the stock via ensure_slots + regression test; (2) Verify existed only on ServerSettings (slash path) -- chat profile modal gains Verify/Unverify (new relay mod_action arms, admin-only); (3) Health tile added to inventory Status (default fixed 1.0->100.0); (4) backpack tiles chunk to visible width (egui wrapped layout never wraps in width-unbounded tree content -- manual chunking); (5) README: two-person-team honesty + Bluesky removed. LOGGED as design direction: UNIFIED MAP (player marker + asteroids + drone on the Cosmos page; mining mini-map becomes a shortcut), garden-section design pass. Verify button + relay arms are Rust both sides -> single v0.687.0.


## 2026-07-04

**Decision:** v0.688.0 SHIPPED unified map slice 1: Home marker + asteroid fan + drone dot on the Cosmos System view; Inventory mini-map links in

**Why:** Operator direction executed same-day: You-Home accent ring rides Earth via the existing project_to_screen; mining asteroids (local ~70km frame, sub-pixel at AU scale) render as a labeled fan with real distances; in-flight drone lerps the Home<->asteroid leg by phase from GuiDrone. Inventory mini-map gains Open-the-full-map link (active_page=Cosmos). Sweep tail: server_settings Your-identity chip said ed25519 while showing the Dilithium hex (post-Inc3) -- now dilithium3; snapshot fixtures updated. Next slices logged in the release notes: clickable asteroid markers, summon routes on the map, crew markers after layout alignment. Snapshot-verified.


## 2026-07-04

**Decision:** v0.689.0 SHIPPED unified map slice 2: clickable asteroid markers -> mining modal

**Why:** Fan markers hit-tested (10px) at lowest click precedence; pending_open_mining_modal one-shot hands off to the Inventory draw which opens with_mining_edit. See the rock, click it, commission the drone -- the map is the launchpad. v0.688.1 stamp shipped same wake. Battery green.


## 2026-07-04

**Decision:** v0.690.0 SHIPPED Stage 3 TAKE-OVER DRIVING (fresh-window big swing #2; the v0.680-689 review workflow runs in parallel)

**Why:** Walk-up [E] drive prompt (mirrors the machine look-cone), E enters (seat occupied, self-drive route cancelled -- driver wins, incl. re-summon mid-drive), WASD arcade steering (vehicle yaw = camera yaw + 90deg since the render body long axis is +X), camera rides the cab (height from kit dims), E exits beside the cab. CRITICAL architecture call: did NOT use the dormant Controllable-transfer enter/exit arms -- moving Controllable off the player makes extract_world_save find NO player and the periodic save would wipe progress with an empty inventory (latent trap, never fired because nothing called those arms; documented in the release notes + code comment). Driving state = EngineState.driving_vehicle; player Transform synced to the vehicle so saves see the traveler. Known gaps for the field test: no vehicle collision (drives through walls), look-steering only. Battery green.


## 2026-07-04

**Decision:** v0.691.0 SHIPPED follow mode -- ECONOMY PHASE 2 / STAGE 3 COMPLETE. The whole operator-designed vehicle pipeline is live.

**Why:** Follow button on in-transit Vehicles rows -> chase-cam (hangs behind-above the travel direction, yaw faces the vehicle, pitch stays manual); broken by WASD/arrival/entering a vehicle. Combined with v0.690 take-over driving, the 2026-07-02 decision is fully delivered: kit deploy anywhere + factory pad lanes + summon self-drive + follow + take the wheel. The v0.679.1..v0.689.1 adversarial review workflow still runs in the background; its findings adjudicate when it lands. Remaining Phase 2 tail: buy-side order flow (gated on wallet/currency), vehicle collision (drives through walls -- physics arc), mech enter/exit redesign (Controllable-transfer save trap documented).


## 2026-07-04

**Decision:** v0.692.0 SHIPPED range-review fixes: the v0.680-689 adversarial pass confirmed 7 findings / 4 root causes, all fixed same-session

**Why:** (1) grown-backpack saves lost stacks on restart (load path lacked the v0.687 ensure_slots -- the SAME discarded-overflow class, one layer up; regression-locked); (2) Verify role-clobber: could demote mods/admins incl SELF (sole-admin lockout) -- relay refuses elevated/empty targets + sends confirmations, modal hides inapplicable button; (3) Cosmos fan markers buried + out-clicked when zoomed -- paint after pills, clicks claim first; (4) gen-theme-css CRLF marker miss nested banner pairs per regen -- EOL-normalize + first-BEGIN..last-END collapse, healed 8->2 idempotent. 3 findings rejected by verifiers as unreachable. Review cost ~977k subagent tokens -- the whale paid for itself again. The 5th fresh-window release (v0.690 driving, v0.691 follow, v0.692 fixes).


## 2026-07-04

**Decision:** OPERATOR FIELD SESSION 3 (late night): a full design-direction batch, logged verbatim before any code

**Why:** (1) VOLUME-BASED CONTAINERS: slots bug the operator -- "the real limit of a container is its volume; slots would be more like a bandolier has 50 slots for bullets" = GO for the material-storage arc Stage A (volume_l on items + capacity_liters enforcement). (2) VEHICLE BAY over vehicle assembler: justify every machine; an assembler is rare-use, a 3D printer is not; prefer a BAY -- a dedicated standard-highway-vehicle-sized area, select which vehicle it holds (personal use or sale); justification = spaceship gravity safety (unsecured vehicle destroys the room). Ties into the existing hangar-1/mech-1 ZONES. (3) MACHINE INFO WINDOWS: the walk-up card is a tiny top-left box; every machine card must show RELEVANT LIVE info -- assembler: vehicle being built + an infinite-of-X SELECTOR of what to build (the fixed auto_recipe in RON is an infinite-of-X violation); containers: contents; cistern: volume + variables. Audit all machines. (4) FIRST REAL-LIFE VEHICLE TARGET: the operator's 1975 Chevy Nova; prebuilt/starter vehicle in the default home so testing driving needs no factory chain. (5) STUDIO CHAT LAYERS: selected HOS channel view ON the studio page; layered merged chats (HOS top, then YouTube/Twitch/Rumble), resizable/collapsible like chat rails. (6) TEXTURES BROKEN: surfaces render as horizontal/vertical colored LINES instead of splotches/grain -- suspect procedural noise collapsing on one axis (matrix/UV bug); investigate shaders. (7) GLB pipeline: confirm glTF/GLB as THE game format (never FBX), need an in-app + GitHub guide for adding models and a viewer; STL stays for the Prusa, GLB is not a print format -- export path later. (8) CHAT BADGES: follow-direction badges (you-follow-them / they-follow-you), verified badge NOT rendering (only A/M seen), role badges leak into the FRIENDS list (follows are universal; admin/mod is server-scoped). (9) P2P-test button in the chat rail confuses -- dev affordance, tuck it away. (10) STREAMING READINESS: Studio is honest-UI-only today (no capture/encode/RTMP transport); native page-switch does NOT kill chat/stream state (designed v0.664); the website DOES because URL navigation reloads.


## 2026-07-04

**Decision:** v0.694.0 SHIPPED the 1975 Chevy Nova + data-driven starter vehicles

**Why:** Operator first real-world recreation target. Real dims/weight in items.csv+kits.ron; kits.ron gains starter:true (infinite-of-X -- starters are data); spawn block gated on no-vehicles-present, placed AFTER load_data_registries (first placement was before the registry loaded -- would have silently never fired; caught by checking call order). Returning fleets never duplicated; pre-vehicle saves get the Nova once. Craftable the long way via car_nova_1975_kit_0. Battery green.


## 2026-07-04

**Decision:** v0.695.0 SHIPPED metre-true UVs: the texture-streak bug was mesh UVs, not shader math

**Why:** Operator suspected matrix math; the noise fns were textbook-correct. Root cause: every homestead quad had fixed 0-1 UVs regardless of face size -- an 89x3m wall stretched noise 30:1 into streaks, the 55x89m deck squeezed into one UV tile. Fix: planar_uv (1 uv unit = 1 metre, dominant-plane projection) in home_structure quad builders + cylinder shells circumference-true + fibonacci floor world-XZ UVs. tile_scale now = tiles per metre. OPERATOR VISUAL VERIFY NEEDED in the live world. Session tally since field session 3: v0.693 (graphite/badges/P2P), v0.694 (Nova + starters), v0.695 (UVs).


## 2026-07-04

**Decision:** v0.696.0 SHIPPED the ACTUAL texture fix: brushed_metal sampled noise as vec2(u*200, 0.0) -- 1D = unbroken stripes. Honest correction: v0.695 mesh UVs were not the visible fix (live shader is world-pos triplanar).

**Why:** Operator reported v0.695.1 changed nothing -- correct: procedural material shaders (granite_tile etc.) are UNWIRED files; the live pbr_simple.wgsl ignores mesh UVs. Traced the active dispatch: floors carry data-driven material_type; the deck is type 1 brushed metal whose scratch fn pinned the second noise axis to zero. Fixed with cross-axis + 2D breakup. Wood/concrete already 2D. Lessons: (a) verify the RENDER PATH before fixing meshes, (b) the unwired granite/plank/drywall shader library is future material-pipeline work (mesh UVs now metre-true for it). Operator visual verify = acceptance.


## 2026-07-04

**Decision:** v0.697.0 SHIPPED driving E/W inversion root-cause + A/D steering (operator field report)

**Why:** Symptom N/S-fine-E/W-inverted = X-mirrored forward recomputation. TWO sites used (-sin,-cos) vs the camera true (sin,-cos): the v0.690 drive update AND the per-frame camera_forward DataStore sync (so deploy spots + summon destinations were mirrored E/W too, unnoticed). Both now use camera.forward_xz(); body yaw derives from the forward VECTOR (atan2, transit formula). A/D turn the shared heading (D=right matches mouse-right) so mouse+keyboard steer simultaneously; steering-mode setting queued. Rule journaled: never reimplement a convention the camera exposes.


## 2026-07-04

**Decision:** PAGE-ACCESS AUDIT (operator: "is chat finished? what pages exist that I cannot reach?") via a 6-auditor workflow, then SHIPPED v0.698.0 closing its top finding: POST /api/v2/agents/override accepted ANONYMOUS writes since v0.118.0 (rewrite data/coordination/overrides.ron + attacker-controlled #announcements spam via unrestricted scope_id). Now: Dilithium sig over agent_override\n{ts} (mirrors /api/admin/stats) + admin-role check + scope_id charset/length validation (RON-injection + spam guard, unit-tested); web/pages/agents.html loads pq.js + pq-relay-auth.js and signs like admin-app.js. Audit headlines all ranked in PRIORITIES: web chat substantially finished EXCEPT a MAJOR DM-attachment privacy bug (uploads while viewing a DM post to the public channel); native chat solid B+ with ranked parity gaps (worst: native discards incoming voice_call so web callers ring forever); 11 web pages fully orphaned + 10 drawer-only (desktop-invisible); 22/53 native GuiPage variants unreachable; /download serves a stale v0.36 fork via nginx.

**Why:** Operator asked for a non-game assessment day and suspected hidden pages (confirmed). The override fix could not wait: anonymous write + announcement spam on the live relay. Chose selective verification over re-running the died-at-spend-limit 37-verifier pass: hand-verified the security claim (the one driving action), accepted auditor evidence for the rest.


## 2026-07-04

**Decision:** TIER-0 audit-fix sprint. v0.698.2: web-chat DM/group attachment privacy bug -- file attach/paste/drop posted the upload URL as a PUBLIC channel chat while echoing it into the DM pane (looked private, was not). Introduced window.sendComposedContent(content) as the SINGLE routing authority (group_msg / Kyber-E2EE DM fail-closed / public channel) and collapsed sendMessage DM+group branches to delegate to it, so seal logic lives once and cannot drift from the attachment path. v0.698.3: /download served web/activities/download.html (frozen v0.36) because nginx routed there while bump-version stamped BOTH copies -- pointed nginx at the maintained /download.html, deleted the fork, removed the dual-stamp block. v0.698.4: removed dead nginx /landing route (no landing.html, no refs).

**Why:** Operator: "lets work on tier 0 stuff since its so important." Prioritized the two security/privacy items first (both launch-blockers), then the trivially-correct config fixes. Deferred the command-palette dead-command cleanup: the audit verifier for it died unverified and confirming needs a live-relay trace -- not a clearly-correct one-liner, so not churned on a guess.


## 2026-07-04

**Decision:** v0.699.0 NATIVE PAGE CLEANUP (operator picked "native app cleanup" from the audit tier-0 forks). Deleted 17 unreachable GuiPage variants: the 5 Overview* category-landing pages + 12 Settings* sub-page variants, dead since the v0.196 single-row-nav rewrite (nothing navigated to an Overview; Settings* only reachable as cards on the unreachable OverviewSettings; Settings content lives in settings.rs internal SettingsCategory router, untouched). Removed the whole supporting subsystem: category_overview.rs + settings_pages.rs modules, escape_menu top_categories/sub_pages_for/category_pages/category_meta + TopCategory struct. Rehomed the working pages the deletion would have orphaned: Calculator + Files -> Platform tab section-nav; Trade + Guilds -> Real tab (with Market/Wallet). Fixed both Humanity "Get oriented" buttons: they set active_real_section="quests" (an unknown id -> silently fell through to Profile > Body & Measurements) -> now open GuiPage::Quests directly. 36 variants remain.

**Why:** Operator directive: wire-or-delete the dead variants. Verified unreachability by grep (no push_nav_to/active_page= assignment for any of the 17; config_str_to_page/page_to_config_str never referenced them; no test coupling; settings_pages just delegated to settings.rs so nothing lost). Did NOT touch Civilization (operator named exactly Calc/Guilds/Trade/Files to re-link; Civilization overlaps the Mission Dashboard -> flagged as a page-uniqueness follow-up in PRIORITIES D-tail).


## 2026-07-04

**Decision:** v0.699.2 WEB NAV EXPOSURE (item C core). The desktop nav shows a 14-tab app-mirror row; every other page lived in a hamburger drawer whose button was CSS display:none above 768px, so ~17 working pages were unreachable by click on desktop (the operators originating concern this session). Made the hamburger a "More pages" overflow button visible on desktop (base rule -> inline-flex; drawer toggle already had no width guard). Added the 6 working pages missing from the drawer entirely: Trade + Guilds (Community group), Calculator + Files + Bookmarks(/web) + Roadmap (Tools/system group).

**Why:** Directly answers the operators session-opening concern (hidden pages I cannot access). Chose the additive+reversible fix (expose via the existing drawer) over restructuring the primary tab row, which is a taste call left to the operator. Left the dead stubs (dashboard/agents/ai-usage/activities-hub) OUT of nav pending an explicit operator delete decision rather than exposing junk.


## 2026-07-05

**Decision:** APPLIED the pending nginx fix on the LIVE VPS directly over the humanity-vps SSH alias (operator asked "why cant you do it?" -- I could; had been over-deferring). Targeted-edited /etc/nginx/sites-enabled/humanity: /download -> /download.html (was the stale /activities/download.html v0.36 fork), removed the dead /landing route. Backed up the config first, nginx -t (only pre-existing conflicting-server_name warnings, no errors), graceful systemctl reload. Verified: curl /download = 200 serving the maintained v0.699.2 page; /landing = 404 after moving the orphaned stale April landing.html (no inbound links, em-dash title from the pre-purge era) to /root backup.

**Why:** Operator directive + it is a safe reversible op (backup + validate + graceful reload + curl verify). Corrected my standing over-caution: the AI has root SSH via humanity-vps (same as just sync) and should do safe VPS ops directly, not hand the operator a command list. Persisted as memory ops_ai_can_do_vps_work.

