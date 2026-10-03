# Shared building

**SUPERSEDED IN PART (2026-10-03, the operator):** homes get their own places on the
mothership and never overlap, and a player builds only inside their own home, so the
"everyone in one home frame, anyone builds anywhere" model below is out; the build was
stopped at its first step (its draft contract is kept in the session scratchpad, not in
the tree). What survives: relay-held piece records, the seq and snapshot protocol, the
client's SharedPiece apply and save separation, refunds. The frame becomes the ship frame
with per-player plots; see docs/design/ship-homes-and-logistics.md.

**Status (2026-10-03):** first increment was being built (week plan Day 4). Designed by an
agent from four read-only maps of the code, then implemented in waves with a critic
per lane. **Two decisions are the operator's and are NOT made here:** co-op trust vs
enforced rules, and where players meet. The design ships the reversible default
(co-op trust) and is built so either decision changes code in one place, never the
messages or the table (section 6).

Read with: `docs/design/homes-as-profiles.md` (Server homes), `docs/design/game-modes.md`,
`src/systems/construction/shared.rs` (the contract).

## 0. In one paragraph

Shell pieces built aboard while a player is in the shared world become relay-owned records. Each record gets a never-reused `piece_id`, an `owner` DID and a `frame`. The relay writes each change straight to its own SQLite table and broadcasts it with a sequence number. Each client spawns the pieces into its ECS with a `SharedPiece` marker. Because of that marker, they are drawn, collide and shelter with no new render code, but they never enter the local save and never earn the viewer rewards or refunds. Pieces built before joining, and every other blueprint, keep today's private behaviour unchanged. Code checked for this design: relay.rs, msg_handlers.rs, storage/mod.rs, construction/mod.rs, placement.rs, save_load.rs, build_place.rs, lib.rs, net_route.rs, features.rs, did.rs and Cargo.toml.

## 1. The first increment

**The one kind: the stateless shell piece.** These are the seven blueprints marked `shared: true` in data: `wood_foundation`, `stone_foundation`, `wood_wall`, `stone_wall`, `metal_wall`, `wood_wall_window` and `roof` (data/blueprints/basic.ron). Tests use `wood_foundation` and `wood_wall`.

Why these seven:
- **They stack.** Walls snap to foundations and the roof snaps to walls (`snap_to`, `mount: OnTop`). Sharing only one of them would leave pieces floating in other players' views. They are one kind, not three.
- **The pose is the whole state.** They have no `DoorOpen`, no contents filed under `built:{uid}`, no `stations` and no power. A piece is fully described by blueprint + pose. That is the subset of `ConstructionSave` the relay can be the authority for (persistence.rs:228-259).
- **Everything else already reads the ECS.** The renderer (planet_build.rs:425-468), collision (build_place.rs:351-376) and the shelter test (survival_env.rs:219-220) all read the ECS, so a spawned remote piece draws, blocks and shelters with no new code.
- **What is left out, and why:**
  - `wood_wall_door` needs a door-state message.
  - Chests need server-held inventories.
  - Stations and generators would be wired into the viewer's own power (mod.rs:556-559).

  The Wave 0 data test stops anyone flagging those as shared before the code supports them.

**Scope:**
- **Where:** aboard only, in the home frame (`site == None`). This is the frame co-presence positions already use (net_route.rs:375-414, station_off = 0 aboard, lib.rs:3537-3539).
- **When:** only while joined (`game_joined && copresence_active && !copresence_solo`).
- **What the player sees:**
  - Player A presses E on a shell piece. The scaffold appears for A and for B about one round trip later.
  - Each viewer grows the scaffold locally, then it becomes a finished piece.
  - B takes it down with F, and it disappears for both.
  - Both reconnect, or the relay restarts, and the piece comes back with the same `piece_id`.
- **Unchanged:**
  - Non-shared blueprints built while joined stay private, exactly as today. The hint says "only in your home".
  - Planet sites, the B-key editor, vehicles and crops are not shared.
  - game_state.rs, `GameWorld` and the v9 snapshot blob are not touched.

## 2. Wire messages

All outbound messages are `__game__:{json}`, as RelayMessage::System (broadcast) or Private, the same as every other game message (msg_handlers.rs:3095-3099, 3963-3969). All inbound types start `game_`, so `ws_message_feature` gates them under Feature::Game automatically (features.rs:284-290).

**Piece object** (it appears inside several messages):

| field | type | meaning |
|---|---|---|
| piece_id | u64 | SQLite AUTOINCREMENT, never reused, even after a delete or a restart |
| frame | string | `"home"` in this increment (see section 6) |
| blueprint_id | string, max 64 | |
| position | [f32;3] | metres in the frame, Y up; snapped to the grid by the relay |
| rotation | [f32;4] x,y,z,w | snapped to the exact quarter turn by the relay |
| scale | [f32;3] | x and z equal the blueprint size; y may be levelled |
| owner | string | `did:hum:<22 base58>` of the builder's bound socket key (did.rs `did_for_pubkey`); never taken from the client |
| owner_name | string, max 48 | the joined entity's name at build time, for display only |
| placed_at | f64 | unix seconds, on the relay's clock |

**Client to relay:**

- **`game_build`**: {type, `req_id` u32 (client-chosen, unique per session, echoed back), `frame` string, `blueprint_id` string, `position` [3], `rotation` [4], `scale` [3]}
- **`game_unbuild`**: {type, `req_id` u32, `piece_id` u64}
- **`game_pieces_request`**: {type, `frame`}. This is the resync request.

**Relay to clients:**

- **`game_built`** (broadcast): {type, `seq` u64, `server_time` f64 unix seconds (the same clock as game_time_sync's server_time), `req_id` u32 (the builder's; everyone else ignores it), `piece` Piece}
- **`game_unbuilt`** (broadcast): {type, `seq`, `piece_id`, `by` (the remover's DID), `req_id`}
- **`game_pieces`** (private, in parts): {type, `frame`, `seq` (the seq this snapshot is current to), `server_time`, `part` (1-based), `parts`, `pieces` [Piece, at most 128 per part]}. An empty world sends one part with `[]`.
- **`game_build_refused`** (private): {type, `req_id`, `action` ("build" | "unbuild" | "pieces"), `reason` code, `message` (a plain sentence)}

Reason codes:
- `not_in_game`, `rate_limited`, `bad_shape`, `bad_frame`
- `unknown_blueprint`, `not_shared`, `off_grid`, `out_of_bounds`
- `occupied`, `world_full`, `owner_full`
- `no_such_piece`, `storage_error`
- `not_allowed`: reserved for enforced rules and never sent under co-op trust.

`seq` is an in-memory counter. It goes up by exactly one per accepted change and restarts at 0 with the relay. That is harmless, because a relay restart drops every socket and each rejoin brings a fresh snapshot.

## 3. Relay side

**In memory.** New `src/relay/handlers/shared_build.rs` defines `BuildWorld { pieces: BTreeMap<u64, Piece>, seq: u64, registry: BlueprintRegistry }`.
- It lives in `RelayState.build_world: RwLock<BuildWorld>`: a field next to relay.rs:283, initialised beside relay.rs:460 by `BuildWorld::load(&db)`.
- Loading it in `RelayState::new` means the real-relay test harness (features.rs:697-722) and the production boot get the same behaviour.
- The registry comes from `data/blueprints/basic.ron`, falling back to the embedded copy (embedded_data.rs:96,254, the same order as registries.rs:165-174). This matters for the throwaway relay, which runs from a temp directory with no data/blueprints.
- `systems` and `ecs` compile into the relay unconditionally (lib.rs:10, 105), so `BlueprintRegistry::from_ron`, `Transform`, `placement::world_aabb` (placement.rs:149) and `GRID_M` (placement.rs:38) are all usable on the relay.

**Storage.** New `src/relay/storage/world_pieces.rs` (impl Storage: insert, delete, load_all). The table goes in its own `execute_batch` beside the game tables (storage/mod.rs:673-690):

```sql
CREATE TABLE IF NOT EXISTS world_pieces (
    piece_id     INTEGER PRIMARY KEY AUTOINCREMENT,
    frame        TEXT NOT NULL,
    blueprint_id TEXT NOT NULL,
    pos_x REAL NOT NULL, pos_y REAL NOT NULL, pos_z REAL NOT NULL,
    rot_x REAL NOT NULL, rot_y REAL NOT NULL, rot_z REAL NOT NULL, rot_w REAL NOT NULL,
    scale_x REAL NOT NULL, scale_y REAL NOT NULL, scale_z REAL NOT NULL,
    owner_did    TEXT NOT NULL,
    owner_name   TEXT NOT NULL DEFAULT '',
    placed_at    INTEGER NOT NULL,          -- unix ms
    state_json   TEXT NOT NULL DEFAULT '{}' -- door/health later; present now so that needs no ALTER
);
```

- **No index.** At 2048 rows nothing needs one: the load reads everything and deletes go by primary key.
- **BUG-046.** The rule is met trivially: this is a new table, so every column is in its CREATE.
- **Rule for later:** any column added to it by ALTER (a zone, say) gets its index after the ALTER block (the pattern at storage/mod.rs:803-812), plus a pre-migration-shape `Storage::open` test like uploads.rs `opens_a_pre_v0675_database_and_migrates_it`.

Why a dedicated table and not `GameEntity` records:
- A bump of the v9 key would wipe them and restart entity ids (game_state.rs:1276-1311).
- They would wait up to 30 s for the periodic save.
- They would inflate every `game_welcome` and every `game_perceive`.

Here every change is written immediately, before it is broadcast. The owner is stored as the DID (31 characters), not the 3,904-hex-character Dilithium key. A ban sweep can still find a player's pieces by computing the DID from the banned key.

**`handle_game_build`.** Validation runs cheapest first:
1. Rate: a `key|build` bucket at 200 ms, kept in the existing `last_perception_times` map (relay.rs:215-219; the same shape as msg_handlers.rs:3534-3574). Otherwise `rate_limited`.
2. Joined: `game_world.read().find_player_entity(key)`. Otherwise `not_in_game`. The `owner_name` is read from that entity's name component, then the lock is dropped.
3. Shape: `req_id` is a u32; strings are within their caps; every number is finite; arrays have the right lengths. Otherwise `bad_shape`. `frame == "home"`, otherwise `bad_frame`.
4. Blueprint: `registry.get` (otherwise `unknown_blueprint`) and `bp.shared` (otherwise `not_shared`).
5. Pose, through `shared::validate_pose` (Wave 0, the same function the client tests run against real placements):
   - The rotation is a unit quaternion within 1e-3, a pure yaw, and a multiple of 90° within 0.5°.
   - x and z sit on the `GRID_M` grid within 1 mm.
   - Bounds: |x| and |z| at most 2000 m, y from -100 to 500 m. These are sanity bounds, not geometry.
   - Scale: x and z equal `bp.size` within 1 mm, and y lies between 0.1 and `size.y + LEVEL_TOLERANCE_M` (placement.rs:67). `level_top` can stretch a piece's height, so the y check has to allow it.

   The relay then **canonicalises** the pose (exact grid, exact quarter-turn quaternion), so every client stores identical numbers.
6. Under `build_world.write()`:
   - caps: 2048 per world (`world_full`) and 512 per owner (`owner_full`);
   - `shared::same_box` against pieces in the same frame (the `SAME_BOX_M` 2 cm rule of placement.rs:70, 269-283), otherwise `occupied`;
   - the DB insert (otherwise `storage_error`), then the map insert;
   - `seq += 1`, and the `game_built` broadcast, sent while still holding the lock so ring order equals seq order.

The owner always comes from the bound socket key. The two-phase identify (v0.274.0) already proves who that is, so nobody can build in someone else's name.

**`handle_game_unbuild`.**
- Rate (`key|unbuild`, 200 ms) and joined, as for a build.
- The piece must exist, otherwise `no_such_piece`.
- Then one function, `may_remove(remover_did, &piece) -> bool`, which returns `true` under co-op trust. Enforced rules go here and nowhere else.
- Then the DB delete, the map remove, `seq += 1`, and the `game_unbuilt` broadcast under the lock.

**Snapshot on join.** `send_pieces_snapshot(state, key)` is called right after the welcome's Private send at msg_handlers.rs:3095-3099. A fresh join and the rejoin resync share that send site, so one call covers both. It takes `build_world.read()` and sends every part while holding it, so no build can land between two parts. `game_pieces_request` (with a `key|pieces` bucket at 1 s) calls the same function.

**Dispatch.** Three arms are added in the raw `game_*` match (relay.rs:3538-3588): `game_build`, `game_unbuild` and `game_pieces_request`. Pieces are not `GameEntity` records, so `game_perceive` does not list them (see section 9).

## 4. Client side

**Wave 0 contract: `src/systems/construction/shared.rs`.** It compiles into both the relay and the native build, and holds:
- `SharedPiece { piece_id: u64, owner: String, owner_name: String, mine: bool }` (an ECS component);
- the serde `Piece` wire struct;
- `SharedBuildIntent { blueprint_id, pose: Transform, spent: Vec<(String, u32)> }`;
- `OUT_CHANNEL = "shared_build_out"` and `FRAME_HOME = "home"`;
- `validate_pose`, `same_box`, and the bounds and cap constants.

Blueprint gains `#[serde(default)] shared: bool` (construction/mod.rs:24-79). `BuildRequest` gains `shared: bool` plus a builder method.

**Sending a build.**
1. When E is pressed (build_place.rs:88-102), the request becomes `BuildRequest::new(id, pose).on(site).shared(state.shared_build.accepting(..))`, where accepting means joined and active, not solo, and `site.is_none()`. The E handler also refuses when a pending shared build has the same box, so a double press does not spend materials twice.
2. ConstructionSystem keeps its occupied and materials checks and consumes the materials as today (mod.rs:418-487). It also records `spent`.
3. If `req.shared && bp.shared && site.is_none()` and the `OUT_CHANNEL` exists, it pushes a `SharedBuildIntent`, sets the status "Placing Wood Wall in the shared world..." and `continue`s with no local spawn (mod.rs:489-504). This is the accepted-build point the maps identified, so builds that are refused locally are never sent. If the channel is missing (tests, no engine), it builds locally.
4. In the co-presence block (lib.rs:6862-6874), `state.shared_build.tick(world, data_store, ws, now)` drains the channel and sends `game_build` through `ws_client.send`. The system runner ticks before this block (lib.rs:6703-6712), so the intent goes out on the same frame.

**Applying the messages.** New `src/net/shared_build.rs` holds `SharedBuildClient { index: HashMap<u64, hecs::Entity>, last_seq: Option<u64>, staging, pending_builds, pending_unbuilds, inbox, my_did }`. It is a field on EngineState (state.rs, beside 769-771). `my_did` comes from the identity's Dilithium key via `crate::relay::core::did::did_for_pubkey`: the native feature includes relay (Cargo.toml:18), and the Identity page already calls it (gui/pages/identity.rs:41).

`route_game_message` (net_route.rs:19-201) gets four arms that hand the JSON to `shared_build.receive`. `tick` processes the inbox in order:
- **`game_pieces`**: parts are staged; when the last part arrives, the world is fully replaced. Pieces whose ids are missing from the list are despawned, missing ones are spawned, and `last_seq = seq`.
- **`game_built` / `game_unbuilt`**:
  - ignored while `last_seq` is None (a snapshot is coming);
  - ignored when `seq <= last_seq` (a duplicate);
  - on a gap (`seq > last_seq + 1`), applied anyway, and a `game_pieces_request` is sent.
- **Spawning a piece:**
  - `age = server_time - placed_at`.
  - If `age < bp.build_time`: spawn `(Transform, Construction { progress: age, build_time, builder_key: Some(owner) }, SharedPiece)`. This is the first real value `builder_key` has ever held.
  - Otherwise spawn `(Transform, Structure { health, max_health, provides, uid: 0 }, SharedPiece)`.
  - A blueprint unknown locally becomes a Structure with defaults, logged once.
- **Despawning:** go through `index`, after checking that the entity still carries a `SharedPiece` with that id (hecs reuses entity ids).
- **Confirmations, refunds and reconciliation:**
  - A `game_built` whose `req_id` matches a pending build, with `owner == my_did`, confirms that build.
  - A refusal of a build refunds `spent` through `inventory_transfer_ops` (TransferOp add, the same path as build_place.rs:309-316) and shows a notice: "Wood Wall not built: one already stands there. 6 Wood Plank back."
  - A pending build left 10 s without a reply sends one `game_pieces_request`. At the next completed snapshot, a pending build whose box equals a piece with `mine` is confirmed (the occupancy rule makes the box unique per frame). One still unresolved 10 s after that is refunded.
  - Pending builds survive a socket drop.

**How a remote piece is drawn.** No change. `push_render_objects` (planet_build.rs:425-468) puts it in the home list, which is shifted by station_off (lib.rs:15204-15234), the same as remote figures. The scaffold's growth is local and cosmetic, and is never networked.

**Keeping remote pieces apart from the player's own** (each seam the maps found):
1. **The save.** `extract_world_save` (save_load.rs:163-200) uses `hecs::Without<…, &SharedPiece>` for both the Structure and the Construction queries. The pattern already exists at save_load.rs:1808.
2. **The save-apply wipe** (save_load.rs:419-428) uses `Without<Or<&Structure, &Construction>, &SharedPiece>`, so a slot load never deletes shared pieces.
3. **uids.** `assign_uids` (uses.rs:224-237) skips SharedPiece, which stays at uid 0, so `built:{uid}` remains a private namespace.
4. **Rewards.** The completion code (mod.rs:537-547) fires the quest event, XP and the sound only when there is no SharedPiece or `SharedPiece.mine` is true. Other people's pieces finish quietly.
5. **Stations and power** (mod.rs:556-559, 283-298; built_uses.rs:38-46): unaffected. The Wave 0 data test fails if `shared: true` is set on a blueprint with `stations`, `generates`, `power_watts`, `doorway`, or `provides` of storage, rest or crafting_station.
6. **Placement.** `rest_height` and `level_top` (placement.rs:237-263) take a filter. A piece that will be shared rests only on shared pieces, so nobody sees a shared wall floating on someone's private foundation. `occupied` still checks everything.
7. **Take-down** (build_place.rs:271-291):
   - If the planned entity carries a SharedPiece, the client sends `game_unbuild`, records a pending unbuild with the blueprint's materials, and shows "Taking down the Wood Wall...". It does not despawn the piece or refund anything yet.
   - When `game_unbuilt` arrives with `by == my_did` and a matching `req_id`, the materials are refunded and the notice is shown. Splitting `apply_take_down` (build_place.rs:301-330) into a refund part and a despawn part makes this possible.
   - The despawn itself happens in the generic apply path, for everyone.

**Leaving and disconnecting.**
- **Deliberate step-out or solo** (lib.rs:6800-6820): despawn every SharedPiece and clear `index` and `last_seq`.
- **Socket drop** (lib.rs:6895-6905): **keep** the pieces, since they do not move and a dropped connection must not open the walls. Set `last_seq = None`; the rejoin's snapshot fully replaces them.

**Wording on screen.**
- The placing hint reads "E build (shared: everyone here sees it)" or, for a non-shared piece while joined, "E build (only in your home)".
- Settings currently tells joined players "the server remains the authority on shared state" (settings.rs:3386-3400), which is untrue for builds today. The text becomes: "Foundations, walls, windows and roofs you build here are kept by the server. Other builds stay in your own home." That makes it true.

**No web mirror needed.** The game world is native-only, and the web app already filters `__game__` out of chat (web/chat/app.js:1112-1134).

## 5. Bandwidth and compute budget

**Network:**
- A build or take-down event is about 230 B inbound and about 400 B per receiving socket outbound (the JSON escaped inside the System envelope).
- The rate cap is 5 per second per player. Realistic building is at most 0.5 per second.
- 12 players building: about 2.4 KB/s per socket realistically, 24 KB/s at the cap. With no building, 0. For comparison, position updates are about 45 KB/s (12 players × 15 Hz × about 250 B).
- Join or resync: at most 2048 pieces × about 300 B, about 600 KB once, in parts of 128 (about 38 KB each). Resync requests are limited to one per second.
- Nothing cosmetic is networked: scaffold growth, the tint and the sound are all local.

**Compute:**
- Relay: one occupancy pass of at most 2048 AABB compares per build (microseconds), plus one SQLite insert or delete.
- Client: one spawn or despawn per event, and at most 2048 extra ECS boxes drawn through the existing path.

## 6. What the open operator decisions would change

**Co-op trust vs enforced rules.** Co-op trust is the shipped default:
- anyone joined may place shared pieces within bounds and take down any shared piece;
- the remover gets the materials;
- the client charges its own materials;
- the relay checks shape, bounds, rate, existence, caps and occupancy.

Enforced rules change only code, never the wire or the table:
- `may_remove` becomes owner or `is_game_admin` (msg_handlers.rs:3121-3232), and the refusal reason becomes `not_allowed`.
- A `may_build` zone check needs a rank such as `can_edit_ship`, the AI's unratified Mode/Rank proposal (game-modes.md:26-76).
- A reach check against the player's last accepted position needs the anti-teleport freeze fixed first (msg_handlers.rs:3285-3295). Today that rule has no time component and stops updating a player's stored position after a jump over 100 m.
- Refunds go to the owner.
- The relay charges the materials. This is the big one: it needs server-held inventories, which arrive with the relay SystemRunner host and closed characters (PRIORITIES.md:759-768).

The stored `owner` and the reason codes exist from the first increment.

**Where players meet.** It is absorbed by the `frame` field and column.
- **Status quo** (each player's own home, same coordinates): `frame = "home"` and nothing changes.
- **Visiting the host's home:** `frame = "home:<host did>"`, plus a new "share my home" action that uploads the host's private shell pieces.
- **A server-owned station, mothership or the Pioneer:** `frame = "zone:<id>"`, with bounds per zone taken from data. Clients must then draw the server's layout, which is arc D rung 0 (PRIORITIES.md:784-786).
- **A planet:** `frame = "site:<id>"` plus a `world_sites (site_id, body, ox, oy, oz REAL)` table holding the f64 origin. This needs player positions in site frames, and co-presence on planets does not exist yet.

None of these changes a message.

**The 2026-06-07 rule.** Building in the shared world is a Server-kind home ("Server means the server owns the truth", homes-as-profiles.md:104-116). Shared pieces are therefore not in the builder's offline home. The status line says so.

## 7. Tests

**Real-relay tests** (src/relay/features.rs, using `spawn_relay`, `bind_socket`, `join_game`, `send_json` and `frames_until_quiet` at features.rs:697-908 and 1251). Run them with `cargo test --features relay --no-default-features --lib relay::features`. Each one is seen red by deleting the line it guards.

1. `two_players_see_a_shared_build_appear_and_disappear`: A builds `wood_foundation`. Both A and B receive `game_built` with the same piece_id, blueprint, canonical pose and A's DID, and A's copy carries A's `req_id`. B unbuilds; both receive `game_unbuilt` with `by` = B's DID, and the row is gone from the DB.
2. `the_same_spot_cannot_be_built_twice`: B gets `occupied`, and the DB holds one row.
3. `shared_builds_survive_a_reconnect_and_a_relay_restart`: A builds, A's socket closes, A rebinds and joins, and A's `game_pieces` holds the piece. The state is then dropped, `Storage::open` reopens the same path, a new `RelayState` and router start, C joins, and the piece has the same piece_id.
4. `a_refused_build_says_why_and_stores_nothing`, table-driven over: off grid, tilted, non-unit, NaN, wrong scale, out of bounds, `furnace` (`not_shared`), an unknown id, frame `"site:x"`, a socket that is identified but not joined (`not_in_game`), unbuild of a missing id (`no_such_piece`), and two builds within 200 ms (`rate_limited`).
5. `every_change_moves_seq_by_one_and_the_snapshot_carries_it`.
6. `a_large_world_arrives_in_parts`: 300 pieces arrive as 3 parts with consecutive part numbers, and the union equals the DB.
7. Add `game_build`, `game_unbuild` and `game_pieces_request` mapping to Feature::Game in the existing test at features.rs:552-554.
8. A storage test: open a DB created without `world_pieces`, check the table is created, and round-trip an insert, a delete and a load.

**Client lib tests** (`cargo test --features native --lib`):
- `shared.rs`:
  - every pose `placement_pose` produces (each shared blueprint × 4 turns × a few grid points, on an empty world and stacked) passes `validate_pose`. The placer and the validator can never disagree.
  - The rejects (off-grid, tilted, non-unit, wrong scale, out of bounds) fail.
  - `same_box` agrees with `occupied`.
  - The data test: shared only on stateless pieces.
- Construction:
  - a shared request for a shareable piece consumes its materials, pushes an intent and spawns nothing;
  - a non-shareable piece with `shared = true` builds locally as today;
  - a scaffold with `SharedPiece { mine: false }` completes with no quest event or XP, and one with `mine: true` gets both;
  - `assign_uids` leaves shared pieces at 0.
- save_load: a world with 1 private + 1 shared piece extracts 1 construction; applying the save keeps the shared piece (exactly once) and respawns the private one.
- `net/shared_build.rs`:
  - a snapshot fully replaces the pieces;
  - a duplicate seq is ignored, and a gap sends `game_pieces_request`;
  - a refusal refunds exactly `spent`;
  - a snapshot confirms a pending build by box + `mine`, or refunds it after the timeout;
  - a deliberate leave clears all shared pieces, while a disconnect keeps them;
  - a stale `index` entry (a reused entity) is not despawned.
- net_route: each of the four JSON types reaches the inbox.
- build_place: `take_down` on a SharedPiece sends `game_unbuild` and does not despawn or refund.

**In the real game** (new `just verify-shared-build`, reusing verify-copresence's guard, throwaway relay, sandbox boot, join and camera steps; Justfile:569-582). It boots one game instance, with the second player as a script, so the one-GPU rule holds.
1. `scripts/second-player.js` gains:
   - `--build "bp@x,y,z,turns;..."`, which reads sizes from basic.ron, one regex per line;
   - `--unbuild-after S`;
   - `--watch-builds`, which logs every `game_built` and `game_unbuilt` it sees.
2. A game IPC request, `debug/built_pieces_request.json` `{"seconds": N}`, modelled on remote_players_request (ipc.rs:2956-3060). Every frame it records each SharedPiece: id, blueprint, pose, scaffold progress or finished, whether it was pushed to the render list, the frame time, and the count of constructions `extract_world_save` would write.
3. The rig:
   1. The walker builds a foundation and a wall 6 m in front of the parked camera.
   2. The judge (a new `scripts/lib/shared-build-judge.js` plus its node test) checks that the piece is in the ECS within 1.5 s of the walker's `game_built`, at its pose within 1 cm, and in the render list.
   3. The scaffold finishes at build_time ± 0.5 s.
   4. In the screenshot, the projected box shows the wall's category tint (the pixel-count method `figurePixels` uses).
   5. The walker unbuilds; the piece is gone within 1.5 s, and a second screenshot no longer shows it.
   6. The walker builds again. `throwaway-relay.js` gets a `restart()` that kills the relay and respawns it on the same exe, port and dbPath. The game reconnects through its backoff ladder and rejoins, and the piece returns with the same piece_id.
   7. The save count stays 0 for the whole run.
   8. The reverse direction: a showcase verb `shared_build_req` (ipc.rs:326 / planet_build.rs:500-549) pushes a BuildRequest through the real `build_request` channel exactly as E does, after granting materials in the sandbox. The walker logs a `game_built` whose owner is the game's DID.

   The judge must be seen red once, with the `game_built` arm removed from net_route.

## 8. Files, grouped so builders do not share a file

**Wave 0, the contract** (one builder, small, merges first):
- NEW `src/systems/construction/shared.rs`, with its tests
- `src/systems/construction/mod.rs`: `pub mod shared;`, `Blueprint.shared`, `BuildRequest.shared`
- `data/blueprints/basic.ron`: seven `shared: true` flags

**Wave 1, in parallel:**
- **A, relay:**
  - NEW `src/relay/handlers/shared_build.rs`
  - NEW `src/relay/storage/world_pieces.rs`
  - `src/relay/handlers/mod.rs`
  - `src/relay/storage/mod.rs` (the table and `mod`)
  - `src/relay/relay.rs` (three arms at 3538-3588, the field at 283, the init at 460)
  - `src/relay/handlers/msg_handlers.rs` (one call after 3099)
  - `src/relay/features.rs` (tests)
- **B, client world and save:**
  - `src/systems/construction/mod.rs` (the divert at 489-504, `spent` at 470-487, the reward gate at 537-547)
  - `src/systems/construction/uses.rs` (224-237)
  - `src/systems/construction/placement.rs` (the rest/level filter)
  - `src/save_load.rs` (163-200, 419-428, tests)
  - `src/persistence.rs:73-75` (the stale "dormant" comment)
- **C, client net and engine:**
  - NEW `src/net/shared_build.rs`
  - `src/net/mod.rs`
  - `src/engine/net_route.rs` (four arms)
  - `src/engine/state.rs` (the field)
  - `src/lib.rs` (tick and leave in the co-presence block at 6795-6905; register `OUT_CHANNEL` beside the `build_request` registration)
  - `src/engine/build_place.rs` (E at 88-102, take-down at 271-330, hint)
  - `src/gui/pages/settings.rs` (3386-3400)
  - `src/engine/ipc.rs` (the recorder and the showcase verb)

**Wave 2, the rig** (can be written against the contract, runs once A and C have landed):
- `scripts/second-player.js` and `scripts/tests/second-player.test.js`
- `scripts/lib/throwaway-relay.js` (`restart`)
- NEW `scripts/verify-shared-build.js`
- NEW `scripts/lib/shared-build-judge.js` and NEW `scripts/tests/shared-build-judge.test.js`
- `Justfile` (the recipe, plus the judge test added to the node test line at Justfile:551)

**Docs (orchestrator, last):**
- NEW `docs/design/shared-building.md`
- `docs/FEATURES.md`
- `docs/PRIORITIES.md` (Day 4)
- `docs/STATUS.md` (the stale rows at 311-312 and 381)
- `docs/design/game-modes.md` (282-288)

## 9. Risks, and what is left for later

**Risks:**
1. **Material trust.** A modified client can build for free under co-op trust. Ownership cannot be forged, because it comes from the proven socket key.
2. **Co-op removal.** Anyone can take down anything and keep the materials. Built walls are solid, so a player can be walled in; take-down is the remedy until enforced rules exist.
3. **Delivery.** Events and snapshot parts ride the shared 256-slot ring, and a socket that falls behind silently skips messages (relay.rs:29-60). The seq gap check and resync catch it. Chat-only sockets also receive build events, because there is no delivery filter.
4. **Lost pending builds.** A build pending when the player leaves and never rejoins loses its materials if the relay refused it. This is rare.
5. **Data skew.** The relay's registry decides shareability and size, and the client's decides scaffold time and tint. A modded client shows an odd box in the right place. The relay loads the registry only at boot; there is no hot reload.
6. **Private overlap.** The relay cannot see private pieces, so a shared and a private piece can overlap on one client.
7. **Limits are constants.** The caps and bounds are compile-time constants for now. Making them server settings needs Server Settings rows (the GUI-first rule); log that in docs/design/in-app-ops.md.
8. **Clocks.** Each client's scaffold finishes at a slightly different moment because of clock skew. This is cosmetic.

**Later:**
- door state: a `game_piece_state` message; the `state_json` column is ready;
- chests: contents keyed by piece_id, which needs server-held inventories;
- stations and power on shared pieces;
- planet sites;
- B-key editor content, which first needs ids for each element: walls and structures are addressed by Vec index today;
- vehicles and crops;
- the enforced-rules set listed in section 6;
- delivery only to sockets in the game, and interest management;
- Game Admin controls to list or remove pieces by owner, including a ban sweep;
- listing pieces in `game_perceive` so AI agents can see builds.