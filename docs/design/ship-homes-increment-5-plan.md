<!-- Written 2026-10-05 by a planning agent reading main at 2405e95d2; saved here by the orchestrator. The four questions in section 6 were answered with their recommendations (see "Decisions taken" below) for the operator to confirm. -->

# Increment 5 build plan: building only on your own plot

Date: 2026-10-05. Read against main at 2405e95d2 (v0.1462.x). Sources: docs/design/ship-homes-and-logistics.md (sections 2, 4, 7 incl. "as built" notes for increments 1 to 4 and the twelve plots, 8, 9), docs/design/shared-building.md, CLAUDE.md, the journal (data/coordination/orchestrator_state.json, operator answers of 2026-10-04), and the code cited below. Every line number was read today.

## 0. In short

- A building piece is a relay-held record in a FRAME: `plot:<id>` (pose relative to the plot's corner), `zone:<id>` (a shared space), `site:<id>` reserved for planets. Seven stateless shell blueprints are shareable (`shared: true`).
- In the shared world: you place/remove shareable pieces only on your own plot, or on a plot whose holder signed you a household permit (`hum/permit/v1\n{server}\n{plot}\n{grantee}\n{expiry}`, where `{server}` is the relay's own `did:hum:`, so a permit works only on the server it was given on (added 2026-10-05 after Wave 0); checked statelessly like `verify_friend_cert`). Owner removes anything on their plot; permit holder only their own pieces; visitor nothing; admins may remove anything.
- Shared spaces (`zone:*`) need the server rank `can_edit_ship` (new roles column, guarded ALTER after the main schema, no index; BUG-046 rule).
- Local Dev mode no longer edits the ship structure (B editor's ship scope, E-key "build anywhere") while in a shared world; offline Dev keeps everything.
- Relay stores pieces in a new `world_pieces` table, keeps a per-frame seq, sends each player the snapshot of every frame in view (same 250/300 m radii as increment 4's delivery), and refuses with a reason code plus one plain sentence.
- Client spends materials, sends the build, draws nothing until the relay confirms (then a scaffold grows as today), refunds on refusal, never saves shared pieces, never carries them with a home move.
- Two findings change the plan: (a) nobody can walk into another player's plot today (increment 2's neighbours are render-only shells whose doors never open for you), so permits are enforceable but not usable in-game yet, and a neighbour's plot pieces cannot be photographed; the only place a cross-player piece is visible on screen today is a shared zone built by a rank holder. (b) increment 4's privacy rule (nobody is told who lives where) means the wire never carries a piece's owner; each recipient is told only `mine`.
- Seven waves; every shared hot file (lib.rs, relay.rs, storage/mod.rs, msg_handlers.rs, game_state.rs, features.rs, net_route.rs, ipc.rs, home_plot.rs, config.rs, Justfile, verify-copresence.js) belongs to exactly one wave; all fit their line budgets.
- Proof rig: `just verify-shared-build` = `node scripts/verify-copresence.js --build`, one game + two scripted players on a throwaway relay, including the design's named case (a build refused inside the other player's plot, both from the relay's side and the game's).
- One-day cut: drop permits, per-frame interest (send every frame to every joined player, same wire), the screenshot leg and the restart leg.

## 1. What exists today that this uses (file:line)

### 1.1 Plots, frames and the ship
- Twelve homestead plots, 55 x 3 x 89 m, pN at (0, 0, 99(N-1)): data/blueprints/ship_structure.ron:1019 onward. Commons zone origin (65,0,20), 34 x 8 x 55 m (ship_structure.ron:69-76); First Street `street-1` (ship_structure.ron:985).
- `Plot` with `aabb()`: src/ship/ship_structure.rs:252-290. `PlotArrival`/`ShipPlots` (relay's view: id, kind, origin, size; `plot()`): ship_structure.rs:591-640. `HOME_ZONE_ID`: :50. `home_plot()`: :1314-1318. `home_is_away()` (guest): :1666-1668. `ship_file()`: :1930.
- Places already named exactly like frames: src/ship/door_points.rs:28-47 (`"zone:<id>"` / `"plot:<id>"`, box min/max, `own`) and `door_points()` :96-141 (zones minus `home`, then plots). The relay's room exits already use `plot:<id>` (src/relay/handlers/ship_world.rs:116-121).
- Neighbours are render-only default shells, no collision, doors that never open for you: src/ship/neighbours.rs:1-38 (finding (a) above).

### 1.2 The relay's plot table and world
- Table `game_plots (world_id, plot_id, owner_did, assigned_at)`, own batch: src/relay/storage/mod.rs:716-728. Functions: `plot_owner_id` (DID from key) src/relay/storage/plots.rs:35-40, `claim_plot` :50-87, `release_plot` :96-113, `plot_holder(world, plot)` :119-128 (the permit check needs it), `release_plot_by_id` :135-143.
- GameWorld: struct src/relay/handlers/game_state.rs:338-400 (`ship_plots` :385, `rules` :388, `interest` :399), `new()` :506-533, `load_ship()` :581-593 (has the full `ShipStructure`), `assign_home` :1077-1104, `release_home` :1111-1113, `plot_holder_in_world` :1120-1122, `despawn_player` :1130-1139 (forgets moves and interest).
- Each player's entity carries its plot (`set_home_plot`, ship_world.rs:228-247); `own_plot_of(entity)` reads the box (src/relay/handlers/move_check.rs:282-292).
- Delivery by view and privacy: src/relay/handlers/game_interest.rs `components_seen_by_others` :79-95 (strips `home_plot`), `viewer_keys` :127-134, `send_to` (GameTo) :190-197, `send_each` :199.
- Release paths: src/relay/handlers/home_plots.rs `leave_world_for_erase` :90-126, `give_up_plot_if_asked` :133-142, `handle_game_admin` :146-154, `handle_game_release_plot` :169-228. Erase handler order: msg_handlers.rs:1832-1884 (leave_world_for_erase at :1854 BEFORE `delete_account` at :1855; receipt merge pattern :1856-1862).

### 1.3 The welcome and game_join flow
- `handle_game_join`: src/relay/handlers/msg_handlers.rs:2925-3171; welcome JSON :3128-3141, sent :3145-3149 while the world write lock is held (the snapshot hook goes right after); lock released :3168.
- `handle_game_position_update`: :3288-3451; view rejudge :3353-3355, messages sent before the lock drops :3375-3377 (P4 ordering rule).
- Rate helpers: `check_perception_rate` :3592-3606, `perception_rate_allows` :3610-3625 (200 ms per `key|action`, map relay.rs:219). `is_game_admin` :3185-3188. `send_game_private` :4010-4016.
- Dispatch: src/relay/relay.rs:3548-3595 (string match; fleet ledger's one-arm pattern :3590-3594). GameWorld built in `RelayState::new` :428-432 with the db in hand. `game_*` types map to Feature::Game by prefix: src/relay/features.rs:284-290.
- There is no game message enum: relay side is the string match above; client side is `route_game_message` (src/engine/net_route.rs:11-170) with the fleet's hook-first pattern (:14-17) and the welcome arm (:32-62). src/net/protocol.rs (NetMessage) stays untouched, as in 1b.

### 1.4 Building on the client
- E/F keys, ghost, gates: src/engine/build_place.rs: `key()` :75-118 (BuildRequest pushed :109-114), `frame()` :123-198 with the plot gate :155-161 whose Dev exemption is `play_mode.allows(ShipStructureEditing)` (:159), `refused_off_plot` :204-210, `PLOT_EDGE_EPS_M = 0.15` :216, `outside_own_plot` :226-231 (guest: everything refused), `off_plot_hint` :235-241, `build_refusal` (materials, at the crosshair) :270-312, `take_down` :386-408, `apply_take_down` :416-445, `built_piece_segments` (pieces are solid) :466-491, plot test :529-571.
- Ghost pose: src/engine/planet_build.rs `ghost()` :243-286 (`placement_pose` :283). Drawing every Structure/Construction aboard: `push_render_objects` :427-500. Dev verb `dev_build` :502-566.
- ConstructionSystem: src/systems/construction/mod.rs: `Blueprint` :24-79, `BuildRequest` :161-189, `Construction.builder_key` :371-377, `Structure.uid` :380-391, tick :417-603 (occupied :438-445, materials taken from pack then storage :447-520, scaffold spawn :522-537, completion with quest event/XP/sound :550-581, `assign_uids` :585).
- Placement: src/systems/construction/placement.rs: `GRID_M` :38, `LEVEL_TOLERANCE_M` :67, `SAME_BOX_M` :70, `quarter_turn` :73-75, `world_aabb` :149-159, `rest_height` :176-206, `placement_pose` :211-226, `level_top` :237-263, `occupied` :269-283. `assign_uids`: uses.rs:224-237; `first_in_view` (take-down target) uses.rs:163.
- Save separation seams: src/save_load.rs extract constructions :227-275, apply wipe :570-581.
- Home moves carry pieces (must not carry shared ones): src/engine/home_plot.rs `forget_shared_world` :498-528, `carry_saved_pieces` :954-982, `carry_built_pieces` :1120-1163, `built_positions` :1165-1182, `home_things_json` (rig) :1379-1403, `apply_welcome_home` :744-841, `GUEST_NO_EDITOR` :191.
- Editor: `toggle_build_editor` src/engine/editor.rs:2017-2114 (guest refusal :2024-2029), `autosave_ship_structure` :317-349. Ship-scope gates: src/gui/pages/construction/ship_tools.rs :21-67 (zone selector pinned + hint), :207-220 (corridors), :439-442 (plots and districts); src/gui/pages/construction.rs `sync_ship_machine_lock` :659-685.
- Normal-mode home save: src/engine/own_home.rs header :1-38 (Dev writes data files, Normal/Creative keep the home in the save), `write_data_files` :103-143 (ship file written only with ShipStructureEditing, :110), `keep_own_home` :145-164. Increment 5 does not change what the save holds; it adds a server-held layer the save never sees.

### 1.5 Play mode and the roles table
- `PlayMode` (Normal default since 2026-10-04) src/config.rs:95-139; `Capability` :141-171; `play_mode_allows` :173-190; truth-table test :1940-1985.
- `roles` table: CREATE src/relay/storage/mod.rs:1331-1349, guarded ALTERs :1362-1418, built-in seed (INSERT OR IGNORE, after the ALTERs, the v0.262.2 lesson) :1420-1446.
- src/relay/storage/roles.rs: `RoleDef` :17-62, `Default` (deny) :72-99, `list_roles` :107-136, `role_def` :141-170, `upsert_role` :185-225 (positional SQL ?1..?15), upgrade test :357-413. `get_role` src/relay/storage/channels.rs:519-534. Capability pattern: src/relay/handlers/stream.rs:42-45.
- Admin control: Roles grid src/gui/pages/server_settings.rs headers :2371-2375, rows :2455-2517, add form :2520-2560. Registry entry data/admin/ops_registry.json:311-315.
- First admin by env: src/relay/mod.rs:622-639 (`ADMIN_KEYS`).
- Pre-migration fixture with an old `roles` table: tests/fixtures/relay/relay_v0_1456.sql (roles at :250); the pattern test src/relay/handlers/fleet_ledger_tests.rs:338-385.

### 1.6 The friendship-note pattern
- src/relay/core/pq_crypto.rs:233-291: `FRIEND_CERT_DOMAIN` :260, `friend_cert_preimage` :263-265, `build_friend_cert` :272-277, `verify_friend_cert` :281-291 ("MINT AND CHECK LIVE TOGETHER", :250-257), KAT test :535-560, `verify_dilithium` :109-131.
- Delivery: sealed control DM. src/net/dm_pq.rs:174-177 (minting moved out), `DmInner.cert` :180-200, `CTL_*` :202-205; src/engine/dm.rs control filter :147-153, `ingest_control` :198-244, `send_dm_control` :249-287, `send_friend_cert` :290-306; src/net/dm_store.rs is per identity AND server (`load(seed, identity_hex, server_url)` :105), certs :275-288. Web only swallows the three known controls: app/web/chat/crypto.js:783-809, app/web/chat/chat-social.js:459 (an unknown `[[hum:...]]` would render as text).

### 1.7 The engine-module template (copy the fleet ledger)
- Client: src/engine/fleet.rs `on_game_message` :106-139 (net_route hook), `on_welcome` :308-321, `tick` :409-435 (called once per frame in the co-presence block, src/lib.rs:6796). Relay: src/relay/handlers/fleet_ledger.rs `handle` :586-649 behind one relay.rs arm, tests in a sibling `fleet_ledger_tests.rs` (`#[path]`, :651-653).
- lib.rs: `build_request`/`build_status` channels :1259-1260; EngineState constructor at :1789 (`moves: Default::default()` :2039); co-presence block :6705-6831.

### 1.8 Rig machinery
- scripts/verify-copresence.js (2,818 lines): header :1-160; `--plots` per order `runPlotsOnce` :1577; walker spawn and "stop" on stdin :1689-1717; `doorPointsOf` :1089; `takeShots` :1101; `mainPlots` :2797. scripts/second-player.js: options :197-286, `deriveIdentity` :377-391 (exported library), `logOthers` :588-620, stdin lines :1019-1033, exports :1045-1078. scripts/lib/throwaway-relay.js: `relayEnv` drops ADMIN_KEYS :167-189, `startRelay` :422 (data/ mirrored, loopback only, expectSha256 required).
- Game probes: showcase verbs src/engine/ipc.rs:382-502 (`build`, `solo`, `respawn`, `walk_to`, `build_editor`); recorder `debug/remote_players_request.json` :3418-3659 (done JSON :3607-3652, `notices` :3633); door points :3661-3682. `dev_stock_materials` (Crafting's "Dev: stock all materials", src/gui/pages/crafting.rs:307-309).
- Rules the rig must keep: one GPU, machine guard; BUG-133 (`runFreshGate`, `requireBootCopy`, `spawnGame`, `builtinDataLines`, `other_build`; scripts/tests/rig-boot.test.js:77-165); `just rig-tests` list (Justfile ~597); verify-copresence recipe Justfile:654-655.

### 1.9 Constraints that shape the waves
- Line budgets (tests/file_size_ratchet.rs:376-417), measured today: src/lib.rs 16,439/16,700; src/relay/relay.rs 6,359/6,400 (41 free); src/relay/handlers/msg_handlers.rs 4,630/4,660 (30 free); src/engine/frame_ws_poll.rs 1,784/1,800 (not touched); src/gui/mod.rs 4,710/4,800 (not touched); src/gui/pages/construction.rs 4,018/4,250. All new logic goes in new files; hot files get one-line calls.
- BUG-046: new table in its own batch with every column in its CREATE; new column on an existing table by guarded ALTER after the main batch, ALTER before the seed, no index over it.
- BUG-133: any embedded fallback calls `embedded_data::note_builtin_copy` after a disk read (`read_data_or_embedded`, src/embedded_data.rs:390-427), or the rigs fail `no_builtin_data`.
- BUG-159: new tests use `Storage::open_temp(tag)` (storage/mod.rs:2393) / `crate::test_temp`, never `std::env::temp_dir()`.
- Account: every table erased must be exported and left-rows-checked (src/relay/storage/account.rs grabs :97-108, dels :257-264, `erase_left_rows` :297-322, pinned by `the_left_rows_check_reads_every_table_the_erase_answers_for`); tests/account_sql_lint.rs requires the table's CREATE in storage/mod.rs.

## 2. Reconciliation: what is stale in the two docs, and what replaces it

| Doc statement | Today | Replace with |
|---|---|---|
| shared-building.md: frame `"home"`, "site == None, station_off = 0 aboard" (§1, §2, §6) | Since 1a/1b aboard pieces are in SHIP metres; the home is assembled on its plot | Frames `plot:<id>` (plot-local pose) and `zone:<id>`; the client converts with the frame's origin (new src/ship/build_frames.rs) |
| shared-building.md §6 co-op trust: anyone builds anywhere, takes anything down, remover keeps materials | Operator 2026-10-03: build only in your own home | `may_build`/`may_remove` (§3.3 below); materials still on the client's word until holdings (increment 8) |
| shared-building.md Piece object carries `owner` DID and `owner_name` to everyone; `game_unbuilt.by` | Increment 4 review P7: nobody is told who lives where (game_interest.rs:79-95) | No owner on the wire; per-recipient `mine`; table stores `owner_did` only (no name) |
| shared-building.md: "a reach check needs the anti-teleport freeze fixed first (msg_handlers.rs:3285-3295)" | Increment 4 replaced the 100 m rule with move_check.rs | Reach check now possible; not needed for increment 5 (later) |
| shared-building.md §9: build events reach chat-only sockets, "no delivery filter" | Increment 4: `RelayMessage::GameTo`, delivery by view | Build events go to players with the frame in view (game_interest `send_to`/`send_each`) |
| shared-building.md: socket drop keeps pieces | `forget_shared_world` (home_plot.rs:498-528) now also covers server switches, refusals, a guest's home coming back | Every departure despawns all shared pieces; the next welcome's snapshots restore them |
| shared-building.md: "2048 per world, 512 per owner"; ship-homes §8.4 "not all 2,048 at once" | Twelve plots | Caps per frame 512, per owner 512, per ship 4,096 (constants in shared.rs; GUI debt logged in in-app-ops.md) |
| shared-building.md: snapshot after the welcome at msg_handlers.rs:3095-3099 | Welcome sent at :3145-3149 | Per-frame snapshots for frames in view, right after :3149 |
| shared-building.md: four arms in `route_game_message` (net_route.rs:19-201) | House pattern is a hook first (net_route.rs:14-17) | `engine::shared_build::on_game_message` hook + `on_welcome` in the welcome arm |
| shared-building.md: throwaway relay "runs from a temp directory with no data/blueprints" | Since 2026-10-03 it mirrors the tree's data/ | Disk first, embedded fallback with `note_builtin_copy` |
| shared-building.md line refs (relay.rs 283/460/3538-3588, lib.rs 6795-6905/6862-6874, settings.rs 3386-3400, features.rs 552-554, ipc.rs 2956-3060, mod.rs 537-547, state.rs 769-771, Justfile 551/569-582) | Moved | relay.rs 283/428-432/3548-3595; lib.rs 6705-6831; settings.rs 3452-3467; features.rs 284-290; ipc.rs 3418-3659; construction mod.rs 550-581; state.rs 778-807; Justfile rig-tests ~597, verify-copresence 654-655 |
| ship-homes §7 inc 5 files: "src/net/dm_pq.rs (permit building, beside build_friend_cert)" | `build_friend_cert` moved to src/relay/core/pq_crypto.rs:272 on 2026-09-19 (dm_pq.rs:174-177) | Mint and verify permits in pq_crypto.rs; dm_pq.rs only gets a control constant (permit delivery wave) |
| ship-homes inc 5: "src/config.rs (158-170)"; "src/relay/storage/mod.rs (roles, 1229-1245)"; game-modes.md:59 "storage/mod.rs:1229" | ShipStructureEditing config.rs:159-162, `play_mode_allows` :176-190; roles :1331-1446 | As listed |
| ship-homes §4 rule 5: "Today it does: Dev is the default (config.rs:130-131)" | Normal is the default since v0.1461.0; rigs pin Dev | Still needed: the operator's own config and every rig run Dev |
| ship-homes §4 rule 6, §7 1a: Normal player can move the hangar; E-key pieces bounded to own plot "(Dev exempt)" | Fixed/built in 1a (build_place.rs:155-161, 204-231) | Increment 5 removes the Dev exemption while joined |
| ship-homes §4 rules 2-3 (household builds, visitors never build) | Nobody can enter another plot at all (neighbours.rs) | Server rule built now; walking into a permitted plot is an operator question (§6 Q3b) |
| ship-homes §7 inc 5 "ShipStructureEditing switched off while joined unless the relay grants can_edit_ship" | Local ship-file edits change the ship hash, and the relay refuses the next join as "a different ship" | The rank grants relay-held pieces in shared zones; the B editor's ship scope stays offline-only (matches the brief; confirm as §6 Q4) |
| ship-homes §8: "That file is untracked" | Tracked (1e8ed5cb2) | Drop the sentence |
| ship-homes §8.2 "Refunds go to the owner" | Relay holds no inventories | Operator question §6 Q1 (recommend: the remover gets them) |
| PRIORITIES "server home is FRESH" (operator 2026-10-04, journal) | Not built | Increment 5's server-held pieces are the first server-held part of a home; the offline home still comes along privately (increment 6 territory) |

## 3. The design against today's code

### 3.1 Frames (new src/ship/build_frames.rs, compiled into relay and game)
- `BuildFrame { id: String /* "plot:p3" | "zone:commons" */, kind: Plot|Zone, origin: Vec3, size: Vec3 }`; `BuildFrames::of_ship(&ShipStructure)` = every zone except `home` (`HOME_ZONE_ID`) plus every plot, mirroring door_points.rs:96-138; `frame_at(ship_point)` (the frame whose floor box holds the piece's centre); `to_local(pose)`, `to_ship(pose)`; `floor_distance(frame, point)` for interest.
- A piece belongs to the frame holding its centre and its whole turned footprint must lie in [0, size] on x and z with 0.15 m slack (`PLOT_EDGE_EPS_M`, moved from build_place.rs:216 into the shared contract so client and relay use one rule). Height is not bounded by the plot (a roof stands at 3.0-3.2 m on a 3 m plot); y sanity bounds -1..40 m local.
- Plot origins are integral, so plot-local x/z sit on the 1 m grid; the relay still checks the grid in ship metres (origin + local) so a later non-integral origin cannot break it.

### 3.2 Wire messages (all `__game__:` JSON; inbound map to Feature::Game by prefix)
Client to relay:
- `game_build {type, req_id: u32, frame, blueprint_id, position[3], rotation[4], scale[3], permit?}`; pose frame-local.
- `game_unbuild {type, req_id: u32, piece_id: u64, permit?}`.
- `game_pieces_request {type, frame}` (resync after a seq gap; 1 s bucket).
- `permit` object: `{issuer: <Dilithium hex>, plot: "p3", grantee: <grantee's did:hum>, expiry: <unix s>, sig: <base64>}`; preimage exactly `hum/permit/v1\n{server}\n{plot}\n{grantee}\n{expiry}` (`{server}` = the relay's own `did:hum:`, passed by the relay as a fact; the wire `permit` also carries `server`, which the relay never reads).
Relay to client:
- `game_built {type, frame, seq, server_time, piece{piece_id, blueprint_id, position, rotation, scale, placed_at}}` to every player with the frame in view; the builder's copy adds `req_id` and `mine: true`.
- `game_unbuilt {type, frame, seq, piece_id}`; the remover's copy adds `req_id`.
- `game_pieces {type, frame, seq, server_time, part, parts, pieces[{..., mine?}]}`, private, at most 128 per part, an empty frame is one part with `[]`.
- `game_frame_out_of_view {type, frame}`.
- `game_build_refused {type, req_id, action: "build"|"unbuild"|"pieces", reason, why?, message}`.
- Welcome gains `ranks: {can_edit_ship: bool, take_down_any: bool}` (admin/owner for the latter).
Reason codes: `not_in_game`, `rate_limited`, `bad_shape`, `bad_frame`, `unknown_blueprint`, `not_shared`, `off_grid`, `bad_turn`, `bad_scale`, `out_of_bounds`, `outside_frame`, `not_allowed` (+ `why`: `not_your_plot`, `not_your_piece`, `guest`, `permit_expired`, `permit_not_from_holder`, `permit_bad`, `ship_rank`), `occupied`, `frame_full`, `owner_full`, `world_full`, `no_such_piece`, `storage_error`. `seq` is per frame, in memory, restarts at 0 with the relay (every rejoin gets fresh snapshots).

### 3.3 Relay side
Storage (new src/relay/storage/world_pieces.rs; table in storage/mod.rs right after game_plots :728, own batch, every column in its CREATE, no index needed at these sizes):
```
world_pieces (piece_id INTEGER PRIMARY KEY AUTOINCREMENT, world_id TEXT NOT NULL, frame TEXT NOT NULL,
  blueprint_id TEXT NOT NULL, pos_x/pos_y/pos_z REAL, rot_x/rot_y/rot_z/rot_w REAL, scale_x/scale_y/scale_z REAL,
  owner_did TEXT NOT NULL, placed_at INTEGER NOT NULL, state_json TEXT NOT NULL DEFAULT '{}')
```
API fixed now so the handler wave can code against it: `insert_world_piece(&NewPiece) -> u64`, `delete_world_piece(world, id) -> bool`, `load_world_pieces(world) -> Vec<StoredPiece>`, `delete_world_pieces_in_frame(world, frame) -> usize`, `delete_world_pieces_of_owner(did) -> usize`. Written before any broadcast.

Rank: `can_edit_ship INTEGER NOT NULL DEFAULT 0` in the roles CREATE (fresh DBs) and a guarded ALTER placed after the R4 block (:1418) and before the seed (:1424) that also runs `UPDATE roles SET can_edit_ship = 1 WHERE id = 'admin'`; seed tuple gives admin 1, others 0. `RoleDef.can_edit_ship` with `#[serde(default)]`, positional SQL extended (get(15), ?16). Grant = role `owner`, or `role_def(get_role(key)).can_edit_ship`.

In memory: `PieceBook` on GameWorld (`pieces`, init in `new()`, frames and blueprint registry set in `load_ship()`, rows loaded in `RelayState::new` beside relay.rs:428-432). Registry read disk first via `read_data_or_embedded(Path::new("data"), "blueprints/basic.ron")`. Pieces whose frame left the ship file are skipped with a log line (rows kept).

`game_build`, cheapest first: rate (`perception_rate_allows(key, "build")`) -> in world -> shape (req_id u32, strings capped at 64, all finite, array lengths) -> frame known -> blueprint known and `shared` -> pose (`shared::validate_pose`: unit yaw-only quaternion at a quarter turn within 0.5 degrees, grid within 1 mm, scale x/z = blueprint size, y between 0.1 and size.y + LEVEL_TOLERANCE_M; then canonicalise) -> footprint inside the frame -> `may_build` -> caps -> `same_box` in the frame -> DB insert -> seq+1 and broadcast under the same lock (ring order = seq order).

`may_build(frame, builder)`:
- `plot:X`: builder's entity `home_plot.id == X` (owner, no DB read); else a permit with `did(issuer) == plot_holder(ship, X)`, `grantee == plot_owner_id(builder)`, `plot == X`, expiry 0 (if allowed, §6 Q3a) or in the future, signature valid; else `not_allowed` with `guest` (builder holds no plot) or `not_your_plot`, or the permit's own why.
- `zone:Y`: rank required, else `not_allowed/ship_rank`.
`may_remove(remover, piece)`: plot holder removes anything on their plot; a valid permit holder removes only pieces whose `owner_did` is theirs (`not_your_piece`); admins/owners (`is_game_admin`) remove anything; zones need the rank; else `not_your_plot`.

Interest: per player a set of frames in view, judged by floor distance to the frame's box against `rules.delivery.in_view_m`/`out_of_view_m` (250/300, data/ship/shared_world.ron). On join (after :3149): clear the set, judge, send each in-view frame's snapshot. On each accepted move (after :3355): new frames get a snapshot, lost frames get `game_frame_out_of_view`, sent before the lock drops. On despawn: forget the set (game_state.rs:1130-1139). On the shipped ship a player at p1's door has p1, p2, p3, the Commons and First Street in view; p4 (256.5 m) is out.

Releases and erase: when a plot is given back (admin release, give-up, erase) every piece on it is taken down (memory, DB, `game_unbuilt` to viewers) and the admin notice says how many (subject to §6 Q2). Erase also deletes every piece the account placed anywhere: memory in `leave_world_for_erase`, rows in `delete_account` (counted as `world_pieces` in the receipt; the plot's pieces taken down earlier are added to the receipt like `ship_plots` at msg_handlers.rs:1856-1862). Export lists the account's pieces.

Logs for the rig, never a key: `Game: built piece {id} {bp} on {frame}`, `Game: build refused ({reason}/{why}) on {frame}`, `Game: took down piece {id} on {frame}`, `Game: take-down refused ({reason}/{why})`.

### 3.4 Client side
- Gate (new `engine::shared_build::gate`, pure): offline (`copresence_active` false) behaves exactly as today (Dev builds anywhere, others own plot only, all private). Joined: own plot + shareable piece -> Shared(`plot:own`); own plot + other piece -> Private (stays in your home); another plot -> Shared only for a shareable piece with an unexpired permit for that plot, else Refused; a zone -> Shared only with `ranks.can_edit_ship` and a shareable piece, else Refused; no frame (corridors) -> Refused; guest -> Refused unless permit. Dev gets no exemption while joined.
- Sending: E builds a `BuildRequest` with `.shared(frame)`; the ConstructionSystem keeps its occupied and materials checks, takes the materials, records `spent` (pack and storage parts), and instead of spawning pushes a `SharedBuildIntent` into the `shared_build_out` DataStore channel (registered beside lib.rs:1259); status "Placing Wood Wall in the shared world...". A shared request for a non-shareable piece, or with no channel, is refused before anything is spent. `shared_build::tick` (every frame, one line in the co-presence block) sends intents as `game_build` (one per 250 ms; a `rate_limited` reply is resent after 500 ms), keeps pending builds by req_id, and refunds intents when not joined.
- Applying: `game_pieces` parts staged per frame; on the last part the frame is replaced (spawn missing, despawn absent, keep same ids), seq set, pending builds whose box matches a `mine` piece confirmed. `game_built`/`game_unbuilt`: ignored while that frame has no snapshot, ignored at or below the frame's seq, applied on a gap with one `game_pieces_request`. A piece younger than its build time spawns as `Construction{progress: server_time - placed_at}` + `SharedPiece{piece_id, frame, mine}` and grows locally; older spawns as `Structure`. `game_frame_out_of_view` despawns that frame's pieces. Every welcome starts the shared pieces afresh; every departure despawns them all.
- Refusals: `game_build_refused` refunds `spent` through `inventory_transfer_ops` (overflow to storage) and toasts one sentence plus what came back. A pending build left 10 s unanswered asks for that frame's snapshot once and is resolved by it.
- Take-down (F): on a `SharedPiece`, the gate mirrors `may_remove` (own plot any; permit + mine; zone + rank; `take_down_any`); allowed sends `game_unbuild` and changes nothing until `game_unbuilt` with our req_id, which refunds the blueprint's materials to the remover; refused shows the sentence. `apply_take_down` splits into refund and despawn (build_place.rs:416-445).
- Seams: `extract_world_save` and the apply wipe skip `SharedPiece`; `assign_uids` leaves them at uid 0; completion rewards fire only for no `SharedPiece` or `mine`; `carry_built_pieces`, `carry_saved_pieces` and `built_positions` skip them; a piece that will be shared rests only on shared pieces (`placement_pose_where` filter) so nobody else sees it floating on a private foundation.
- Dev off while joined: `config::ship_editing_allowed(mode, in_shared_world) = mode.allows(ShipStructureEditing) && !in_shared_world`, used at build_place.rs:159, own_home.rs:110, ship_tools.rs:26-29/214-218/440, construction.rs:665; the pinned zone selector's hint and the Gameplay note say why.

### 3.5 How a refusal reads (one copy of each sentence in shared.rs, used by the relay's `message` and the game)
- At the crosshair while placing, before anything is sent: "Placing Wood Wall: this is someone else's plot; you build only on your own plot, or where its holder has given you a household permit [Esc] done". Zone without rank: "Placing Wood Wall: the ship's shared spaces are built by people this server has given that rank [Esc] done". Non-shell piece off your plot: "only foundations, walls, windows and roofs can be built outside your own home". Guest: today's off_plot_hint guest line.
- After the server refuses (toast): "Wood Wall not built: <sentence>. 6 Wood Plank back." Sentences: not_your_plot as above; permit_expired "your household permit for this plot has run out; ask its holder for a new one"; permit_not_from_holder "this permit is not from the plot's holder (it may have changed hands)"; ship_rank as above; occupied "one already stands there"; frame_full/owner_full/world_full "this plot / you / the server already keep as many pieces as the server allows (512/512/4,096)"; data mismatch codes "this game and the server disagree about the piece; update whichever is older"; storage_error "the server could not save it, so nothing was built; try again"; not_in_game "you are not in the shared world right now".
- Take-down refusal: "This Wood Wall was put up by someone else: only this plot's holder can take it down" (not_your_piece); "This is someone else's plot: you cannot take down what stands on it" (not_your_plot).
- Placing hint when it works: "[E] build here (kept by the server: anyone near sees it)" or "[E] build here (only in your own home)".

## 4. Waves

Shared-file ownership (each owned by exactly one wave): src/lib.rs 2B; src/relay/storage/mod.rs 1A; src/relay/relay.rs 2A; message routing: relay.rs 2A and src/engine/net_route.rs 2B (src/net/protocol.rs untouched); src/relay/handlers/msg_handlers.rs 2A; game_state.rs 2A; src/relay/features.rs 2A; src/engine/ipc.rs 2B; src/engine/home_plot.rs 1B; src/config.rs 1C; CLAUDE.md 0; Justfile and scripts/verify-copresence.js 3A; src/gui/mod.rs and frame_ws_poll.rs nobody. Two files are edited by two waves in sequence, never in parallel: src/systems/construction/mod.rs (0 fields, then 1B tick) and src/engine/build_place.rs (1C one line, then 2B routing).

Budget after all waves: relay.rs about 6,366/6,400; msg_handlers.rs about 4,642/4,660 (keep every edit a one-line call); lib.rs about 16,442/16,700.

Each test below is red on the code before its wave; for new functions the red statement names the guarded line whose removal fails it, as every test in this repo records.

### Wave 0: the contract (one builder, merges first)
Files: NEW src/systems/construction/shared.rs; src/systems/construction/mod.rs (`pub mod shared;`, `Blueprint.shared` with serde default, `BuildRequest.shared_frame` + `.shared(frame)`); data/blueprints/basic.ron (`shared: true` on wood_foundation, stone_foundation, wood_wall, stone_wall, metal_wall, wood_wall_window, roof; lines 6-17); NEW src/ship/build_frames.rs + src/ship/mod.rs; src/relay/core/pq_crypto.rs (`PLOT_PERMIT_DOMAIN`, `plot_permit_preimage`, `build_plot_permit`, `verify_plot_permit` beside the friend cert); CLAUDE.md Cryptography table (one "Plot permits" row, required in the same commit).
Delivers: SharedPiece, wire structs, SharedBuildIntent + channel name, `validate_pose`, `same_box`, `box_inside_frame`, caps, reason/why codes and their sentences, frames and conversions, permit mint/verify.
Tests:
- `every_shareable_pose_the_placer_makes_passes_validate_pose` (each shared blueprint x 4 turns x grid points, empty and stacked): red with the yaw check inverted.
- `validate_pose_refuses_and_canonicalises` (off grid, tilted, non-unit, NaN, wrong scale, y out of bounds): red with the grid check removed.
- `same_box_agrees_with_occupied`: red with SAME_BOX_M changed in one copy.
- `shared_only_on_stateless_blueprints` (no stations, generates, power_watts, doorway, storage/rest/crafting provides; the seven are flagged): red before basic.ron gains the flags ("wood_wall is not shareable").
- `frames_of_the_shipped_ship` (2 zones + 12 plots, no `home`, `frame_at` picks p3 for a point in p3, round trip local/ship exact): red with `home` not filtered.
- `a_footprint_over_the_plot_line_is_outside` (the build_place.rs:529 cases moved here): red with the 0.15 m slack doubled.
- `plot_permit_roundtrip_and_pinned_preimage` (pinned "hum/permit/v1\ndid:hum:srv\np3\ndid:hum:abc\n0" since 6e0174cd7, plus `a_permit_given_on_one_server_is_refused_on_another`; wrong plot, grantee, expiry, issuer fail): red with the fields swapped in the preimage.

### Wave 1 (three builders in parallel, plus the rig writer)
1A Relay storage and the rank. Files: src/relay/storage/mod.rs (table batch after :728; CREATE column, guarded ALTER + admin UPDATE between :1418 and :1424, seed tuple; `mod world_pieces`), NEW src/relay/storage/world_pieces.rs, src/relay/storage/roles.rs, src/relay/storage/account.rs (grab, del, EXISTS line), src/gui/pages/server_settings.rs (header "Edit ship" at :2371-2375, row checkbox :2491-2495, add-form checkbox :2535-2539), data/admin/ops_registry.json (roles entry :311-315 names the new permission).
Tests:
- world_pieces.rs `pieces_round_trip_and_ids_are_never_reused` (insert, delete, reopen, next id higher): red with AUTOINCREMENT dropped.
- `pieces_of_a_frame_and_of_an_owner_go_together`: red with the WHERE on world_id removed.
- roles.rs `can_edit_ship_round_trips` (upsert/list/role_def): red with get(15) reading another column.
- roles.rs `an_existing_roles_table_gains_can_edit_ship_and_only_admin_has_it` (rewind to the pre-column shape with rows, reopen): red with the guarded ALTER removed ("no such column: can_edit_ship").
- fixture test on tests/fixtures/relay/relay_v0_1456.sql (opens, admin true, verified false, `world_pieces` exists and takes a row): red with the ALTER removed.
- account.rs `an_erase_takes_the_accounts_pieces_and_the_export_lists_them`: red with the del line removed; the existing left-rows pin goes red until the EXISTS line is added.
- server_settings: a headless grid test (or snapshot) that the column is drawn and toggles the draft: red before the checkbox.
1B Client save and world seams. Files: src/systems/construction/mod.rs (tick: divert after materials, `spent`, refusal before spend for non-shareable or missing channel, reward gate), src/systems/construction/uses.rs (assign_uids), src/systems/construction/placement.rs (`placement_pose_where`), src/save_load.rs (:236-275, :574-581), src/engine/home_plot.rs (carry, built_positions, `forget_shared_world_keeping_home` despawns SharedPiece).
Tests:
- `a_shared_build_spends_once_pushes_an_intent_and_spawns_nothing`: red today (a local scaffold spawns).
- `a_shared_request_for_a_piece_that_is_not_shareable_spends_nothing`: red with the check after the spend.
- `someone_elses_scaffold_finishes_quietly_and_mine_earns_its_reward` (quest event `build_stone_foundation`, XP): red with the gate removed.
- `assign_uids_leaves_shared_pieces_at_zero`: red today.
- `a_shared_wall_rests_on_a_shared_foundation_not_a_private_one`: red with the filter ignored (rests at 0.2 on the private one).
- save_load `shared_pieces_never_enter_the_save_and_survive_a_save_load` (1 private + 1 shared: one construction saved; apply keeps the shared one exactly once): red today.
- home_plot `a_home_move_carries_private_pieces_and_leaves_shared_ones` (welcome Move p1->p2): red today (the shared piece travels 99 m); `leaving_the_shared_world_takes_shared_pieces_down`: red today; `built_positions_leave_out_shared_pieces`: red today.
1C Dev off while joined. Files: src/config.rs (`ship_editing_allowed`, `ship_editing_for(&GuiState)`, PlayMode doc), src/engine/build_place.rs (:159 only), src/engine/own_home.rs (:110), src/gui/pages/construction/ship_tools.rs (:26-29, :43-67 hint text, :214-218, :440), src/gui/pages/construction.rs (:665), src/gui/pages/settings.rs (Gameplay note :3452-3467; erase list :497-503 gains "what you built in the shared world"), src/gui/pages/hud.rs (stale comment :263-270).
Tests:
- config `ship_editing_in_and_out_of_a_shared_world` (Dev offline true, Dev joined false, Normal/Creative false both ways): red with today's mode-only gate.
- build_place `dev_in_a_shared_world_builds_only_on_its_own_plot`: red with the Dev exemption kept (Commons accepted).
- own_home `a_dev_save_in_a_shared_world_writes_the_home_and_not_the_ship_file`: red with :110 unchanged.
- ship_tools `the_zone_selector_is_pinned_in_a_shared_world_and_says_why` (headless, Dev + copresence_active): red today (the combo is drawn).
3A starts here, writing only (see Wave 3).

### Wave 2 (two builders in parallel, after Wave 1 merges)
2A Relay handler. Files: NEW src/relay/handlers/shared_build.rs, NEW src/relay/handlers/shared_build_tests.rs, src/relay/handlers/mod.rs, src/relay/handlers/game_state.rs (field, `new()`, `load_ship()`, `despawn_player`), src/relay/handlers/msg_handlers.rs (welcome `ranks` at :3128-3141; `on_join` after :3149; frame rejudge after :3355; erase receipt at :1854-1862), src/relay/relay.rs (one arm after :3594 for game_build/game_unbuild/game_pieces_request; one load line after :432), src/relay/handlers/home_plots.rs (take a released plot's pieces down; erase drops the account's pieces from memory), src/relay/features.rs (end-to-end tests).
Tests (features.rs on a real relay unless noted):
- `a_build_on_your_own_plot_reaches_the_neighbour_and_a_build_in_theirs_is_refused` (A on p1 builds: A gets req_id + mine, B gets it without; B builds in plot:p1: not_allowed/not_your_plot; one row): red with `may_build` returning Ok.
- `only_the_holder_takes_down_what_stands_on_their_plot` (visitor refused, holder removes any, admin removes any): red with `may_remove` returning true.
- `a_household_permit_lets_its_holder_build_and_take_down_only_their_own` (permit minted from A's seed for B's DID; B builds; B cannot take down A's piece; A can take down B's; an expired permit, a permit from C, and a p1 permit used on p2 are each refused with their why): red with the permit branch removed.
- `the_ships_spaces_need_the_rank` (plain: ship_rank; a role with can_edit_ship and an admin: accepted): red with zone frames accepted from anyone.
- `the_welcome_says_what_this_player_may_do` (ranks): red before the field.
- `a_join_gets_the_plots_in_view_and_not_the_far_ones` (pieces on p2 and p12; a joiner at p1 gets p1, p2, p3, zones, not p12; walking toward p12 brings it; walking away sends out_of_view): red with every frame sent.
- `pieces_keep_their_ids_across_a_reconnect_and_a_relay_restart` (relay_on on one shared guard): red with the DB insert skipped.
- `every_change_moves_its_frames_seq_by_one_and_the_snapshot_carries_it`; `a_large_plot_arrives_in_parts` (300 pieces, 3 parts): red with part numbering off by one.
- `a_refused_build_says_why_and_stores_nothing` table-driven (off grid, tilted, wrong scale, NaN, unknown id, `furnace` not_shared, bad frame, footprint over the plot line, not in game, two builds within 200 ms, unbuild of a missing id): red per row with its check removed.
- `nobody_is_told_who_built_what` (no did:hum, owner or name in anything B receives; `mine` only in A's own copies): red with the owner put back on the wire.
- `giving_back_a_plot_takes_down_what_stood_on_it` (admin release; erase also removes the account's pieces elsewhere; receipt counts them): red with release leaving pieces.
- shared_build_tests.rs (pure, GameWorld + `Storage::open_temp`): may_build/may_remove truth tables; interest in/out with the 250/300 gap; caps; occupancy; canonical pose stored: each red with its line removed.
2B Client net and engine. Files: NEW src/engine/shared_build.rs, NEW src/engine/shared_build_tests.rs, src/engine/mod.rs, src/engine/state.rs (one field), src/lib.rs (constructor line near :2039, channel line at :1260, tick line in the co-presence block), src/engine/net_route.rs (hook before the match, `on_welcome` in the welcome arm), src/engine/build_place.rs (gate, hint, E routing through one `press_build` function, F routing through `take_down_entity`, `outside_own_plot` onto the shared rule), src/engine/planet_build.rs (`placement_pose_where` for a piece that will be shared), src/engine/ipc.rs (recorder `shared` rows with screen rects from `camera.view_projection_matrix()`; done JSON `shared_build` {ranks, ship_editing, frames with seq, pending counts, pieces, save_constructions, pack}; showcase verbs `place`, `take_down`, `stock`).
Tests:
- `the_gate_in_and_out_of_the_shared_world` (every row of §3.4): red with the zone/rank branch missing.
- `take_down_gate_mirrors_may_remove`: red with the visitor row allowed.
- `a_snapshot_replaces_one_frame_and_leaves_the_others`: red with a global replace.
- `a_duplicate_is_ignored_and_a_gap_asks_for_that_frame_again`: red with the seq compare removed.
- `out_of_view_takes_only_that_frames_pieces_down`: red today.
- `a_refusal_refunds_exactly_what_was_spent_and_says_why` (toast text and TransferOps): red with the refund removed.
- `my_piece_is_confirmed_by_its_req_id_and_by_a_snapshot`: red with confirmation by req_id removed.
- `a_stale_index_entry_never_despawns_another_entity`: red with the SharedPiece id check removed.
- `a_welcome_starts_the_shared_pieces_afresh_and_reads_the_ranks`: red before `on_welcome`.
- net_route `each_shared_build_message_reaches_the_client`: red before the hook (falls to `_ => {}`).
- build_place `e_on_a_shareable_piece_in_the_shared_world_sends_and_spawns_nothing` and `f_on_a_shared_piece_sends_game_unbuild_and_waits`: red today.
- ipc `place_take_down_and_stock_verbs_parse` and `the_recorder_reports_shared_pieces`: red before the verbs.

### Wave 3 (after Wave 2 merges)
3A The rig (written from Wave 1 on, run now; one game at a time). Files: scripts/verify-copresence.js (`--build` mode beside `--plots`, reusing `setupRig`, the gate, `doorPointsOf`, `walk_to`, `takeShots`, walker spawn, `--dry-verdict`), scripts/second-player.js (+ scripts/tests/second-player.test.js), NEW scripts/lib/shared-build-judge.js (+ NEW scripts/tests/shared-build-judge.test.js), scripts/lib/throwaway-relay.js (explicit `env` option so a rig can set ADMIN_KEYS; the shell's ADMIN_KEYS is still dropped) (+ scripts/tests/throwaway-relay.test.js), Justfile (`verify-shared-build` recipe; judge test added to `rig-tests`). As a mode of verify-copresence.js it needs no new BOOTS_THE_GAME entry in scripts/tests/rig-boot.test.js.
Tests: second-player `build and unbuild commands parse`, `quarter turns match placement::quarter_turn`, `sizes read from basic.ron`, `permitPreimage equals the Rust KAT string` (red before each exists); throwaway-relay `an explicit admin key reaches the relay, the shell's does not` (red before the option); shared-build-judge: a good synthetic run passes and each doctored record fails exactly its own check (red before the judge).
3B Permits in the game (or increment 5b, see §6 Q3). Files: src/net/dm_pq.rs (`CTL_PLOT_PERMIT`), src/engine/dm.rs (filter :147-153, ingest/verify/store, send), src/net/dm_store.rs (permits per server), NEW src/gui/pages/household.rs + one call from an existing page (owner: pick a friend, choose a length, give; list given permits; grantee: list held permits), app/web/chat/crypto.js and app/web/chat/chat-social.js (swallow the new control so web never renders it), src/engine/shared_build.rs (attach the matching permit; gate rows for permit plots).
Tests: `a_received_permit_is_verified_and_kept_per_server` (red with the verify removed), `a_build_on_a_permitted_plot_attaches_its_permit` (red without), web: the control is not rendered (red before the list change).

### Docs (orchestrator, last)
docs/design/shared-building.md (status and §1-§9 brought to the plot model), docs/design/ship-homes-and-logistics.md (increment 5 "as built"), docs/design/game-modes.md (Mode/Rank: `can_edit_ship` built; :59 line ref), docs/reference/retention_and_deletion_semantics.md (erase list), docs/design/in-app-ops.md (caps as constants; listing and removing pieces by owner), docs/ai/onboarding.md (+ data/ai/onboarding.json: agents may build), docs/FEATURES.md, docs/STATUS.md, docs/PRIORITIES.md, journal.

Checks per merge: `just verify`, `cargo check --features relay --no-default-features`, `just verify-relay` after 1A and 2A, `just validate-data` after Wave 0, and, serialized on the one GPU, `just verify-copresence --plots` (all orders) and the default rig after 1B, 1C and 2B, because those touch what the co-presence judges read (`home_things`, the editor close).

## 5. The proof rig: `just verify-shared-build`

`node scripts/verify-copresence.js --build [--dry-verdict <manifest>]`, evidence in .probe-rig/copresence/runs/<stamp>-build/.
1. Guard and gate as today (machine guard, `runFreshGate`, Dev sandbox, saved servers cleared). Throwaway relay with the tree's data/ and `ADMIN_KEYS=<R's key>` (R's key derived in the rig through second-player.js's exported `masterSeedFrom`/`deriveIdentity`).
2. Walker A (`TestBotBuilder`, plain) joins first and stands at p1's door; the game boots and joins (probe: welcomed, home_plot p2); walker R (`TestBotShipwright`, rank holder) joins third (p3). Door points give every frame's box.
3. Theirs reaches us (plot): A builds a foundation and a wall on p1 (stdin `build wood_foundation@plot:p1:48,0,36,0`, then the wall on it). Judged from the recorder's `shared` rows: in the game's world within 1.5 s of A's `built` line, at p1.min + local within 1 cm with the exact quarter turn, a scaffold that finishes at placed_at + build_time +- 0.5 s, and `save_constructions` unchanged. (No picture: a neighbour's plot is behind its shell from everywhere a player can stand.)
4. Ours reaches them: `{"stock":"1"}` then `{"place":"wood_foundation@48,135,0"}` (p2). The game's piece is `mine`, the pack loses exactly 8 planks once, and A logs `saw built ... plot:p2` at the same pose within 1.5 s.
5. The design's named case, both ways: A sends a build inside p2: `build refused: not_allowed (not_your_plot)`, a relay.log refusal line, and the game never draws it. The game places inside p1: the plot sentence is on screen (probe `notices`), nothing is spent, no relay line from the game in that window, A sees nothing new.
6. Who takes down: A unbuilds the game's piece: refused not_allowed/not_your_plot, still drawn. The game takes it down (`take_down` at the piece): A logs `saw unbuilt` within 1.5 s, gone from the game's world, 8 planks back once.
7. Shared spaces need the rank, and everyone sees them: the game walks (door route, `walk_to`) to MEET_POSE (76, 1.7, 64) facing south. The game's place in the Commons shows the rank sentence; A's build in `zone:commons` is refused not_allowed/ship_rank; R builds a wood wall at zone-local (11, 0, 51), 6 m in front of the camera. Judged: in the game's world within 1.5 s, its screen rect inside the view, and wood-tinted pixels counted in that rect of the screenshot (the tint measured off the first run, as the crew amber was).
8. Dev keeps everything only offline: probe `ship_editing` false while joined; the B editor opens pinned to the home; `solo` out gives `ship_editing` true and zero shared pieces; `solo` in restores A's two pieces and R's wall with the same piece ids.
9. Over the run: no correction of the game or a walker, panics 0, `no_builtin_data`.
Optional last leg (cut first): relay restart (a `restart()` in throwaway-relay.js) and the same ids return; the relay test already proves it.
Seen red once on a real build before it is trusted: with `may_build` returning Ok, step 5 fails exactly `refused_their_build_in_our_plot`; with the client's `game_built` handling removed, steps 3 and 7 fail. Every judge check is also shown failing alone on a doctored copy of a green manifest (`--dry-verdict`).

## 6. Questions only the operator can answer (each with a recommendation)
1. When someone takes down a piece they did not build (the plot's holder, or an admin), who gets the materials? Recommend: whoever takes it down, as today's take-down works. Giving them to the builder needs the server to hold inventories (increment 8) to reach someone who is offline. (The design wrote "refunds go to the owner".)
2. When a plot is given back (admin release, a home that does not fit, an erased account), what happens to the pieces on it? Recommend: they come down with it, and the admin's notice says how many; otherwise the next household moves into a stranger's half-built walls. Erased accounts lose their own pieces everywhere either way.
3. Permits. (a) The server stores nothing, so a permit cannot be withdrawn before it runs out. Recommend: every permit has an end date, 90 days at most, renewable with one press; no endless household permits. (b) Nobody can walk into another player's plot today (neighbours are drawn shut, increment 2), so a permit holder cannot reach the plot. Recommend: build the permit rule on the server now (scripted players prove it), and ship the household page together with walking into a permitted plot and seeing its holder's own home, as increment 5b; it is also the first step toward the joint homes you asked for.
4. The ship-editing rank. Recommend: it lets its holders build in the ship's shared spaces through the server; the build editor's whole-ship editing stays an offline Dev activity, because a local ship edit changes the ship and this server would refuse that game's next join. Also: the built-in Admin role gets the rank by default (and owners always). Question 9 (Mode and Rank) is otherwise still open; a column off for everyone but admins changes nothing until granted, so it is reversible.

## 7. If it must land in one day
Keep: Wave 0 without the permit functions (and so no CLAUDE.md row), 1A, 1B without the shared-only rest filter, 1C, 2A with owner-only rules plus rank zones and admin removal and no permits, 2B without permit attachment, and rig steps 1-6 and 8.
Cut, in this order: permits on both sides (3B and the 2A branch; the `permit` field stays reserved), per-frame interest (send every frame's snapshot to every joined player and every event to every joined player; same messages, so it returns later as a relay-only change), the screenshot in step 7 (keep the recorder's presence and screen-rect checks), the restart leg, the release clean-up of a plot's pieces (Q2; the holder can take anything down), and the shared-only rest filter (invisible today: neighbours' plots cannot be seen into).

## Critical files for implementation
- C:\Humanity\src\systems\construction\shared.rs (new, the contract both sides compile)
- C:\Humanity\src\relay\handlers\shared_build.rs (new, may_build, may_remove, interest, snapshots)
- C:\Humanity\src\engine\shared_build.rs (new, gate, send, apply, refunds, probe)
- C:\Humanity\src\relay\storage\mod.rs (world_pieces table and the can_edit_ship ALTER, BUG-046)
- C:\Humanity\src\engine\build_place.rs (E/F routing, the Dev exemption that goes while joined)

## Decisions taken (2026-10-05, the recommendations in section 6; for the operator to confirm)

1. Materials from a piece taken down by someone who did not build it go to whoever takes it down.
2. Pieces on a plot that is given back come down with it.
3. Every household permit has an end date, at most 90 days, renewable; the household page and walking into a permitted plot are a later increment 5b.
4. `can_edit_ship` lets its holders build in the ship's shared spaces through the server; whole-ship editing stays an offline Dev activity; the built-in Admin role has the rank by default.
