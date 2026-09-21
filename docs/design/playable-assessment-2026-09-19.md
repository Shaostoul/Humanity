# What a person can actually play today

> Read-only assessment, 2026-09-19, written against the code at v0.1321.1, not
> against the docs. Every claim carries a file path. Commissioned by the
> operator: "We really need to figure out what it is we're missing and how to
> proceed to get to a near fully playable game."
>
> Four operator decisions arrived while this was being written and are treated
> as settled facts throughout: the mothership is PREMADE authored content and
> the player only modifies their own home area of roughly an acre; the
> construction tools serve the player and the developer with one surface, and
> making them good is the leverage point; the ship is self-sufficient without
> the player, who is just one crew member; and NPCs come after the loops,
> because an NPC cannot farm until the farming loop is finished. Combat, NPC
> lives and player roles such as captain or architect are explicitly deferred.
>
> Docs this corrects rather than replaces:
> [gameplay-loop-map.md](gameplay-loop-map.md) (its ORDER is still sound, its
> current-state columns are two months stale),
> [first-playable.md](first-playable.md) (says `NetSyncSystem` is instantiated
> nowhere; it is, `src/lib.rs:1902`),
> [home-design.md](home-design.md) (says there is no in-app way to place a
> machine; there is, `src/gui/pages/construction.rs:404-440`).

## The headline

**A person can sit down alone tonight, mine an asteroid with a drone, smelt the
ore, forge a hammer, plant a garden, cook, eat, equip clothes, cast an ability,
collect eggs from a chicken, buy and sell at a trading post, build a structure,
and finish a quest chain, all through the UI with no console and no developer
beside them.** Eleven loops genuinely close. The reason it does not feel like a
game is not that the systems are missing; it is that nothing in the world tells
the player any of this exists, most of what they build is discarded at exit, the
fastest crop takes 4.7 real hours, and **a downloaded build ships none of the
3D art.**

That last one is the single biggest finding in this report and it reframes three
of the operator's seven complaints. `.github/workflows/build-desktop.yml:90-93`
copies `data/`, `assets/icons/` and `assets/shaders/` into the release bundle
and nothing else. `assets/models/` (188 MB, including all 109 plant model
folders and all 15 furniture models) and `assets/textures/` (81 MB) **are not in
any downloaded release**, on either bundle path (the data archive at `:260-268`
has the same three lines). A build run from the repo root loads them; a build a
player downloads falls back to an untextured primitive for every machine
(`src/engine/home_meshes.rs:566-630`) and a procedural mesh for every crop
(`:953-986`). 269 MB of authored art currently reaches nobody.

The operator's belief that "we have a lot of framework built but it still all
needs to be wired together" is now **half wrong and half very right**. The
engine-side wiring largely happened: the closure ladder shipped fifteen
increments in v0.745 to v0.759, and 24 of 42 systems are registered and ticking
(`src/lib.rs:1055-1319`, audited against `tests/engine_wiring_lint.rs`). What did
not happen is the layer between the working simulation and the person.

One number frames it. Since the design bible was written on 2026-07-07 this repo
has taken **1,755 commits. 22 touched `src/systems/`. 294 touched
`src/renderer/`.** The gameplay ladder has not been the work for two and a half
months. That was a defensible choice and the renderer arc produced real wins,
but it is why the question is being asked now.

---

## 1. The loops that close

A loop closes when a player can trigger it from the UI or the world, gets a
consequence, and has a reason to repeat it. These do:

| Loop | The player action | Where it lives |
|---|---|---|
| **Drone mining** | Commission a drone at an asteroid, ore is delivered home | `src/systems/mining.rs`, asteroids `src/lib.rs:1353-1397` |
| **Smelt and craft** | Crafting page, 444 recipes, skill-gated and station-gated | `src/systems/crafting/mod.rs:865-885` |
| **Farming** | Plant a tower, bed or field, water, fertilize, harvest | `src/systems/farming/mod.rs` |
| **Eat, drink, breathe, keep warm** | Vitals drain, food restores, five ways to die | `src/systems/food.rs:500-586` |
| **Death and respawn** | Die at 0 HP, "YOU DIED" with a named cause, respawn | `src/gui/pages/hud.rs:13`, `src/lib.rs:12306-12345` |
| **Power, water, air** | Cut the power, watch the pump stop and the air foul | `src/systems/electrical.rs`, `plumbing.rs`, `atmosphere.rs` |
| **Livestock** | Walk to a chicken, press E, get an egg and farming XP | `src/lib.rs:11672-11760` |
| **Build a structure** | Crafting page, Structures, Build; materials consumed, scaffold rises | `src/gui/pages/crafting.rs:381`, `src/systems/construction/mod.rs:116-240` |
| **Buy and sell** | Walk to a trading post, Trade, buy at 1.25x and sell at 0.5x | `src/gui/pages/vendor.rs:19`, `src/lib.rs:12002-12055` |
| **Quests** | Browse, accept; all six objective types have live emitters | `src/gui/pages/quests.rs:118`, `src/systems/quests/mod.rs:267-293` |
| **Gear and abilities** | Equip moves the item, armour mitigates, keys 1-9 cast | `src/lib.rs:12161-12260`, `src/gui/pages/hud.rs:670-760` |

Content behind them: 867 items, 444 recipes, 189 plants, 92 creatures, 110
abilities, 20 skills, 12 blueprints, 43 authored quests in five files.

That is a real game's worth of machinery, and more than the design bible claims,
because the bible predates the ladder shipping.

---

## 2. Where it stops

In the order a new player hits them.

**2.1 Nothing tells the player the game exists.** No in-world tutorial, no
first-time prompt, no HUD quest tracker. `gs_first_steps` is auto-accepted at
`src/lib.rs:1321` and then never surfaced: grep `quest` in
`src/gui/pages/hud.rs` returns nothing. The onboarding that exists
(`src/gui/pages/main_menu.rs:100-113`) is platform onboarding, identity and
server connection, and routes a first-boot user to the Humanity dashboard, not
into the world. Press Enter World and you stand in a 55 by 89 metre room with no
objective on screen.

**2.2 You cannot see what is killing you.** The HUD draws a health bar and a
credit balance (`src/gui/pages/hud.rs:102-119`). Satiation, hydration, oxygen and
body temperature exist, drain and kill, but appear only in the Inventory page's
Status section (`src/gui/pages/inventory.rs:1431-1497`). A player starves with no
warning outside a menu; the death screen is the first HUD element that mentions
it.

**2.3 What you build does not persist.** `src/save_load.rs` contains zero
references to `Structure` or `Construction`; `WorldSave.constructions` is
labelled dormant schema at `src/persistence.rs:55-57` and is never written. Also
unsaved: vitals and player position (`src/save_load.rs:133` says so in a
comment), machine state, battery charge, water level, livestock, and the garden
tuning sliders (a `thread_local GardenEditState`,
`src/gui/pages/inventory.rs:190`, "until garden persistence lands"). Saved:
inventory, skills, crops, credits, quests, vehicles, placed items, appearance.
A session's construction work is discarded at exit, silently.

**2.4 A built structure does nothing.** `Structure.provides` is read by a label
(`src/gui/pages/crafting.rs:365`) and by the system that constructs it, and
nothing else. The crafting station gate reads `placed_machine_types` from
`home.ron` instead (`src/lib.rs:11948-11956`), so building a furnace does not
unlock smelting. The construction sink terminates in a decorative box.

**2.5 Crop pacing makes an evening's play impossible.** A game day is 1,200 real
seconds (`src/systems/time.rs:63`) and growth is `growth_days * SECONDS_PER_DAY`
(`src/systems/farming/mod.rs:1041`). The fastest crop in `data/plants.csv` is
oyster mushroom at 14 days: **4.7 real hours**. The median is 120 days, 40 real
hours. The nutrient slider caps at 1.5x (`:1060`). There is no away-time growth.
The only escape is the 1x to 600x clock slider on the weather panel
(`src/gui/pages/weather_panel.rs:296`), a review tool that accelerates hunger and
the sun too. **Nothing a player plants tonight can be harvested tonight.** It is
worse than that: no code path plants anything at world load, so a fresh world's
garden is empty boxes until the player presses Plant.

**2.6 There is no threat.** `hostile_wildlife` defaults to false
(`src/config.rs:648`) and the wolf row in `data/entities/wild_spawns.ron` is
commented out. Consistent with combat being deferred, but it means the only
stakes are environmental.

**2.7 The ore runs out, then comes back.** Three asteroids are hardcoded at
`src/lib.rs:1353-1397` with finite ore; `src/systems/mining.rs:284-292` despawns
a depleted one and nothing respawns it. They are spawned unconditionally at
startup and are not in `WorldSave`, so they return fresh every relaunch. Within a
session the faucet dries up; across sessions it is unlimited. Neither behaviour
is designed.

**2.8 Trade completes without moving anything.** `src/engine/frame_ws_poll.rs:1619-1625`
sets the status to "Trade completed - items exchanged" and no item is ever
removed from or added to an ECS `Inventory`. Offers are hand-typed strings, not
`items.csv` ids (`src/gui/pages/trade.rs:350-383`).

---

## 3. Built but not wired

The instinct is right; the list is shorter and more specific than expected,
because most of it got wired in July.

**Orphaned data, zero code references:**
- `data/npcs.ron`, 596 lines, 32 authored NPCs with services, shop stock and
  dialogue, plus `data/dialogues.ron`, 1,232 lines. Neither is referenced by any
  line of `src/`. The only mentions in the repo are `docs/FEATURES.md` and a
  comment in `data/economy.ron:19`.
- `data/enchantments.csv`, 134 rows. `grep -rn "enchant" src/ --include=*.rs`
  returns nothing at all.
- `src/systems/self_sufficiency.rs` plus `data/food/crop_nutrition.ron`: built
  and tested, zero consumers. The Home page still shows hand-typed kcal strings.
- `data/biomes.ron`, 16 biomes with real precipitation and temperature, still
  orphaned while the weather tables stay hardcoded.
- `data/behaviors.ron` and `config/flow_field.toml` are read by
  `src/systems/ai/behavior.rs:3` and `flow_field.rs:3`. **Neither file exists.**
  Both modules are 26 lines of type declarations with zero call sites.

**A registered, ticking, entirely dead system.** `src/systems/interaction.rs` is
registered at `src/lib.rs:1242`. It queries `Interactable`, which is never
attached to any entity anywhere in `src/`. It writes an `interaction_prompt`
DataStore slot that nothing reads. Its trigger branch is a `log::debug!`. Every
real in-world verb is a hardcoded `else if` chain at `src/lib.rs:2492-2660`.

**Systems with an `impl System` and no registration (18 of 42,
`tests/engine_wiring_lint.rs:32-86`):** Ecology, Hydrology, Disaster, Placement,
Aging, Astronomy, Docking, CreativeArts, Fire, Genetics, Governance, Geology,
Waste, Offline, Oceanography, Medical, Transportation, Hvac. All eighteen are
genuinely unregistered; none is a stale entry, though Aging and Genetics have
stale reasons (they say "needs living entities with age"; livestock now spawns).

**Registered but inert:** `ManufacturingSystem` (`src/lib.rs:1261`) ticks against
`ProductionFacility` entities nothing spawns. `PlacementSystem` is unregistered
and also data-dead: it early-returns on a `"placement_state"` key that has no
writer anywhere.

**Relay endpoints the game client never calls:** `game_perceive`,
`game_interact`, `game_query_inventory`, `game_query_entity`
(`src/relay/relay.rs:3535-3550`), all with range checks and rate limits, built
for AI agents. The relay also broadcasts `game_time_sync` every five seconds
(`src/relay/mod.rs:780`) and the client drops it at
`src/engine/net_route.rs:197`, although `NetMessage::TimeSync` has a working
handler at `src/net/sync.rs:404`. That bridge is three lines, and without it two
players in one world have two different clocks and two different suns.

**Dead transport.** `src/net/client.rs` `NetClient`, the purpose-built game
socket, is referenced only from inside its own file.

**Pages that look finished and are not.** Full audit of all 56 page modules is
summarised here; the five that matter:
`src/gui/pages/wallet.rs` is a fake financial UI (balance is permanently 0.0,
`src/gui/mod.rs:3555`; "Send" fabricates a transaction row stamped "Confirmed"
at `:196-210`; no Solana RPC exists in the codebase).
`src/gui/pages/tasks.rs` create, edit and delete only mutate a local Vec; the
relay's `task_create`/`task_update`/`task_delete` handlers are never called from
native, so a task you make vanishes on the next list response.
`src/gui/pages/profile.rs` says the Body, Interests, Social Links and Private
Notes sections are "stored locally and never shared" (`:184`); they appear in
neither `src/config.rs` nor `src/persistence.rs` and reset every launch.
`src/gui/pages/recovery.rs` is entirely inert and is the **default landing of
the Platform tab** (`src/gui/pages/platform.rs:70`).
`src/gui/pages/notes.rs` and `calendar.rs` have no storage at all, and notes
shows a "Saved" badge driven by a two-second timer.

---

## 4. The world, complaint by complaint

Two of the seven are now answered by the operator's decisions rather than by
code, and one of them changes sign entirely.

### 4.1 "A kinda crappy player home, poorly arranged, a giant empty room used as a tech demo area"

**The size is not the problem, and this is the correction.** The live home is one
ship zone, `data/blueprints/ship_structure.ron:39`, 55 by 89 metres: 4,895 m²,
which is **1.21 acres**. That is the operator's own "1 acre or whatever it is for
that specific ship design". The shell is right. What is wrong is everything
inside it.

- **Only 6.7 percent of it is partitioned.** 19 interior walls, all inside x 39
  to 55 and z 24 to 47. The thirteen room-grade sub-zones
  (`ship_structure.ron:849-950`: entry, common, kitchen, pantry, hall, bedroom,
  bathroom, wetroom, study, utility, two workshops, console room) total about
  330 m². The other 4,565 m² is one continuous open volume.
- **The machines in that volume are a demo grid and the room labels are
  decorative.** In zone mode `MachineHome::placements`
  (`src/machines.rs:1609-1617`) ignores `inst.room` entirely and uses absolute
  x/z clamped into the zone. So 17 crafting stations labelled `room: "study"`
  sit in three straight rows on open floor at z 21.4, 22.6 and 24.6
  (`data/machines/home.ron:2735-2890`), and the whole power and water plant is
  labelled `room: "garage"` at x 3 to 11 (`:2437-2640`) **when no `garage` room
  exists in the live layout at all.** Same for `commons`, `bay` and `garden`.
- **55 of 71 machine catalogue types have no model** (`model: None`; 61 boxes
  and 10 cylinders), and in a downloaded build the other 16 have none either.

**Gap type: content authoring, unblocked by a decision but blocked by a tool.**
There is no open design question left here. The work is to partition the acre and
place its machines, and per the operator that is the construction editor's job,
which puts it downstream of section 6's tool work.

### 4.2 "The gardens don't look good"

**What exists.** Five garden arrays (`data/machines/home.ron:2949-3008`): 18
potato beds, 10 oilseed beds, 8 grain trays, 24 nutrition towers, 6 mushroom
racks, plus four field machines and two apothecary towers. A real procedural crop
renderer (`src/renderer/plant_mesh.rs`, six form archetypes, continuous growth,
a wilt axis).

**What is missing.** Three things, compounding.
1. **There is no garden room.** `zone_types.ron` defines `room_garden` and **no
   zone in `ship_structure.ron` uses it.** The arrays sit at x 5 to 40, z 37 to
   65, in the unwalled open shell. No glazing, no enclosure, no aisle, no floor
   treatment.
2. **Beds are literally boxes.** `potato_grow_bed` is a 2.0 by 0.4 by 1.0 brown
   box (`home.ron:1507-1512`). No soil surface, no rim, no water.
3. **The hero-model path is gated off for towers.** `src/engine/home_meshes.rs:888`
   requires `grid_spot.is_some()`, so all 26 towers always render procedurally by
   explicit decision, and the towers are the showpiece. Plus nothing is planted at
   world load, so the default garden is empty boxes.

**Gap type: content for the room and bed models, one code gate for the towers,
one small code path for a starter planting.**

### 4.3 "We only have one plant 3D model from the looks of it"

**In a downloaded build there are zero, which is why it looks that way.** In a
repo build there are 109 model folders covering 18 species at four growth stages
(`assets/models/plants/`), but selection is convention over config by exact id
match and **12 of 189 rows in `data/plants.csv` match**: tomato, carrot, wheat,
lettuce, corn, rice, pumpkin, beet, apple, orange, watermelon, bamboo. Six
purchased model sets (bushberries, cactus, flower, grass, mushroom, palmtree)
match no plant id and are unused. `data/plants_visual.ron` adds 19 authored
procedural recipes; roughly 115 species have neither a recipe nor a model and all
render through one `generic_visual()` leafy shape
(`src/renderer/plant_mesh.rs:77`), which is exactly what "only one plant model"
looks like. **No hero model exists for any crop the homestead is actually built
around**: potato, oilseed, flax, sunflower, amaranth, bean, garlic and strawberry
all have none.

**Gap type: a shipping decision, then a five-line schema change, then content.**
Add a `model` column to `plants.csv` so every tomato cultivar can point at
`tomato`, and the 18 existing families cover far more than 12 rows.

### 4.4 "The cabling and plumbing needs work aesthetically"

**What exists.** A genuinely good router. `src/ship/conduits.rs:103` runs a
Manhattan path, places brackets every 1.5 m and at bends, adds elbows for rigid
kinds, gaskets where a run crosses a wall, and dedupes overlapping fittings
(`src/engine/home_meshes.rs:1133-1141`). It is unit-tested. There is real
electrical engineering behind it (`src/utilities.rs`: AWG, ampacity, voltage
drop, NEC derating).

**What is missing is entirely on the render side.**
- Every pipe, bracket and gasket is the **same 8-sided cylinder**, one cached
  `Mesh::cylinder(device, 0.05, 1.0, 8)` at `home_meshes.rs:1102`, scaled. At a
  0.012 m copper radius an 8-sided tube is a visible octagon.
- **Elbows render as nothing**: `FittingKind::Elbow => {}` at `:1230`. Rigid runs
  turn 90 degrees with no corner piece.
- **Flexible runs do not sag.** `is_rigid()` only picks metalness and roughness
  (`:1158`); the geometry is identical straight segments.
- **Everything runs at one shared height**, `service_y = ceiling - 0.3` (`:1131`),
  so power, water and nutrient all live in the same plane at 2.7 m and cross
  there. No trays, no bundling, no wall-following.
- **Every pipe is emissive 0.5** (`:1162`), so the runs glow faintly in the dark.
- `ConduitKind::color()` is dead; the utility legend colour wins (`:1159`), so
  nothing is copper-coloured.
- The node graph shipped and is authored empty:
  `data/machines/home.ron:3389-3390`, `conduit_nodes: []`, `conduit_edges: []`.

**Gap type: code capability, well-scoped, one increment.** Elbow, bracket and
gasket meshes; per-kind vertical offsets; a tray when several runs share a leg;
more tube segments; sag for flexible kinds. One open taste call, listed in
section 7.

### 4.5 "NPCs don't really work"

**What exists.** Two unrelated things, neither of which is what the docs
describe. Relay crew NPCs: the server simulates chore agents on its own Pioneer
(`src/relay/handlers/game_state.rs:80-140, 306-360`), broadcasts positions, and
the client draws them as amber box-and-sphere capsules with a name and a live
chore nameplate. And livestock, which graze and can be harvested.

**What is missing.**
- The 32 authored NPCs and 1,232 lines of dialogue are dead data (section 3).
- **No character model exists anywhere in `assets/`.** NPCs are a
  `Mesh::box_xyz(0.42, 1.4, 0.26)` plus a `Mesh::sphere(0.17)`
  (`src/lib.rs:9005-9012`).
- **No pathfinding and no animation.** The two AI navigation modules are
  26-line stubs reading files that do not exist; there is no skinned pipeline, so
  NPCs slide.
- **They are in the wrong world.** The relay loads
  `data/ships/starter_fleet.ron`, a 40 by 16 by 8 m ship; the client walks the 55
  by 89 m acre. `src/net/sync.rs:33` and `:295-298` override Y to a fixed 1.0
  "until relay/client layout alignment lands", so crew walk Pioneer coordinates
  inside the home, through walls.
- **There are no NPCs at all offline.** `RemoteNpc` is only ever created from a
  websocket message (`src/engine/net_route.rs:149-167`), and a solo home sends no
  `game_join` (`src/lib.rs:7198`). A single-player world is empty of people.

**Gap type: code capability, and correctly deferred.** The operator's own
reasoning settles the ordering: an NPC cannot farm until farming is finished, so
NPC behaviour is downstream of every loop it would perform. The one thing worth
doing early is cheap and unblocks the rest: a client-side NPC spawner so a solo
player is not alone in a city, even with placeholder behaviour.

### 4.6 "The residential module isn't properly built, just kinda scattered about"

**What exists.** One residential district, `ship_structure.ron:951`: `res-1`, 120
by 200 by 4 metres. It is filled by `src/ship/home_structure.rs:1001`
`tile_home_clones`, which bakes the player's own home shell once and stamps it
into every slot, then stamps a corridor segment between adjacent slots. With a 55
by 89 m home and a 2 m gap that is a 2 by 2 grid: four identical copies of the
player's house. Eleven other districts (hangar, mech bay, reactor, medical,
armory, arena, mall, transit, industrial, storage, cargo, agri) get generic box
fillers.

**What is missing.**
- **The clones have no collision.** `ship_wall_segments`
  (`src/ship/wall_collision.rs:145-156`) builds colliders only from
  `zone.body.walls`; the macro zone list and all tiled clone geometry produce no
  segments. You walk through every residential building.
- **The clones are empty shells** by explicit design (`home_structure.rs:993-1000`
  copies walls and structures, never machines, lights or furniture).
- **Every slot is the same house.** `home_design_roster()` returns exactly one
  design and its own comment says so.
- **The other districts are solid silhouettes.** `zone_filler.ron` declares
  `mesh_kind: "stall" | "array" | "cradle" | "rack"` and **the code never reads
  `mesh_kind`**; every non-residential type gets `footprint_box`.
- **One Y plane only** (`home_structure.rs:1054-1058` says so outright), and the
  `Deck` tier the superstructure doc describes does not exist in the zone system
  at all.

**Gap type: this is now a content job that the tool cannot yet do.** The
operator's decision removes the design question: the mothership is premade, so
the residential district is authored content, and a city of hand-written RON is
not a viable authoring path. It is the clearest argument in this report for
section 6's ordering.

### 4.7 "We haven't been able to build any multiple storied spaceship interiors"

**More exists than the complaint implies.** `src/ship/layout.rs` has a real
`ShipDef { decks }` with `Direction::Up | Down` and BFS pathfinding across decks,
and `data/ships/starter_fleet.ron` is a genuine two-deck ship.
`src/ship/fibonacci.rs:212` has `RoomConfig.level` with storey stacking at `:528`
and a guard test at `:2081` proving rooms on different storeys are not adjacent.
The construction editor has a working storey selector
(`src/gui/pages/construction.rs:59-120`). Vertical movement all works: ladder
climb (`src/renderer/camera.rs:1104-1125`), elevator car riding
(`src/lib.rs:3500-3560`), teleporter pads, and stairs, ramps and deck slabs via
the step-up footing sampler (`:3570-3625`).

**Four hard blockers stop any of it reaching the live home or ship.**
1. **`ShipDef`/`DeckDef` is not the live world.** Its only consumer is the
   headless relay (`src/relay/handlers/game_state.rs:366-400`). The renderer never
   touches it. `data/ships/layout_medium.ron` is a *third*, incompatible schema
   with every room at y = 0.
2. **Interior walls have no base Y.** `InteriorWall { a: (f32,f32), b: (f32,f32),
   height, ... }` (`src/ship/home_structure.rs:171-193`) is 2-D endpoints plus a
   height rising from the zone floor. **You cannot author a wall on an upper
   storey.**
3. **Collision is flattened to one plane.** `ship_segments_impl`
   (`src/ship/wall_collision.rs:145-156`) uses `zone.origin.0` and `.2` and never
   `.1`. Stack two zones and the upper deck's walls block the lower deck.
4. **The footing sampler is single-storey by construction.**
   `src/lib.rs:3568-3575` takes the first `room_bounds` entry whose XZ box
   contains the player, commented "Room floors are coplanar in the home".
   `ELEVATOR_TRAVEL` is a hardcoded `3.0`.

What you can build today is an upper level of deck slabs reached by stairs, with
no walls and no ceiling on it.

**Gap type: code capability, four changes, and they are the same four that gate
the construction tool.** That is the key structural insight of this report and
section 6 is built on it.

---

## 5. Single player versus multiplayer

**Single player is the whole game.** All 24 registered systems run client-local.
Everything in section 1 works offline.

**Multiplayer is co-presence and nothing else.** A player joins by walking into
the 3D view while the chat socket is up (`src/lib.rs:6675-6697`); there is no
game server browser and no connect dialog, and the character-select launcher's
Open Net card refuses unless you already connected in Chat
(`src/gui/pages/showroom.rs:553`). Position streams at 15 Hz. Other players
appear as teal capsules with no nameplate (`src/lib.rs:8962-8993`; the comment at
`:8960` says nameplates are a follow-up) plus a HUD roster
(`src/gui/pages/hud.rs:135-156`). Doors open for remote players. That is the
complete list of ways another human affects your world.

**What does not travel:** inventory, crops, machine state, buildings, terrain
edits, health, crafting, skills, vehicles, livestock, weather, or the clock. None
has a message type in either direction. Of 17 protocol variants in
`src/net/protocol.rs`, six are ever constructed.

**Peer-to-peer gameplay does not exist.** `str0m` carries E2EE chat groups and
voice calls only (`src/net/webrtc.rs:1-6`); every `WebrtcEvent` consumer at
`src/lib.rs:14233-14300` is chat, voice or a debug echo.

**A dedicated server is a relay plus a 20 Hz avatar tracker.**
`grep "crate::systems::\|crate::ecs::" src/relay/` returns zero matches.
`SystemRunner` is instantiated only in the native app and the wasm entry. The
relay does persist what reaches it (positions, its own crew, `game_time`, quest
and XP progress) and that survives a restart.

**Single-player-only by accident rather than design.** Three things, and they are
the whole multiplayer story:

1. **`WorldSave` is native-gated because of one GUI type.**
   `src/persistence.rs:53` holds `placed_items: Vec<crate::gui::PlacedItem>`, and
   `crate::gui` is native-only, so `src/lib.rs:104` gates the whole module. **A
   dedicated server cannot hold a player's home even in principle.** Nothing else
   in `WorldSave` is native-specific. Moving one type out of `gui` is the highest
   capability-per-line change in the repo.
2. **All 47 gameplay system modules already compile into the relay binary.**
   `src/systems/mod.rs` has no feature gates. They take
   `(&mut hecs::World, dt, &DataStore)`, no GPU, no egui. They are client-only for
   two non-design reasons: the relay has no `SystemRunner` host, and no protocol
   message could carry their state.
3. **The relay and the client render different places** (section 4.5). Two
   players share X and Z while each sees their own private house.

---

## 6. The shortest credible path

The existing closure ladder is not wrong, it is **finished**. Rungs 1 through 8
shipped, rung 10 largely shipped, rung 9 is superseded by the mothership
decision. Adding more systems adds nothing a player can perceive.

The ordering principle changes accordingly. The ladder's was "stakes before
content, sinks before faucets". The new one is: **a player must be able to see
the simulation, keep what they make, and finish something in one sitting; then
the tool that authors the world must be good enough to author a city.**

### The one strategic call, and my answer

The operator asked which is the better first move for the world itself: invest in
the construction tools, or hand-author the mothership in data files. **Invest in
the tools, and the evidence is stronger than the preference.**

- A city cannot be hand-written. One acre of home is already 3,390 lines of
  `ship_structure.ron` plus 3,390 of `home.ron` for thirteen rooms and 105
  machines. A residential district alone is four dwellings today and is meant to
  house a population in the billions. RON does not scale to that by hand, by any
  author, human or AI.
- The four blockers are shared. Every code change that lets the editor build a
  second storey (a base Y on `InteriorWall`, Y-banded collision, a level-aware
  footing sampler, one canonical layout schema) is the same change that lets
  anyone author a multi-deck ship. Fixing them once fixes complaint 7, unblocks
  complaint 6, and makes complaint 1 a content job instead of an impossible one.
- The tool is already most of the way there. The editor round-trips rooms, places
  and drags machines live in 3D, has a storey selector, and autosaves. It is not
  a greenfield build; it is four defects and a schema decision.
- The alternative fails twice. Hand-authoring produces a ship the player's own
  tool cannot open, which contradicts the operator's own stated goal and the
  repo's standing rule that an AI design must be a player design by construction
  ([home-design.md](home-design.md)).

The one honest caveat: tool work shows the operator nothing for a while. That is
why it is Tier B and not Tier A. Tier A is what makes the game playable tonight;
Tier B is what makes the world worth being in.

### Tier A: make the existing game legible and durable

This is most of the win and none of it is speculative.

0. **Ship the art.** Add `assets/models/` and `assets/textures/` to
   `.github/workflows/build-desktop.yml:90-93` and `:260-268`, or decide
   deliberately that they do not ship (section 7). Until this is settled, every
   hour spent on models, textures or plant art reaches nobody who downloads the
   game. It is two lines and 269 MB of download.
1. **Persist the world.** Write `Structure` and `Construction` into `WorldSave`
   (the field exists, `src/persistence.rs:24`), plus vitals, player position,
   machine state, livestock and the garden sliders. Today a session's building
   work is thrown away at exit with no warning, which converts every other loop
   from progress into play-acting. Contained in `src/save_load.rs`.
2. **Put the simulation on the HUD.** Satiation, hydration, oxygen, body
   temperature, and the active quest objective. One file,
   `src/gui/pages/hud.rs`, which already has a health bar to copy. Without it the
   five death causes are invisible mechanics that only frustrate.
3. **Fix crop pacing**, whichever way section 7 decides. A game whose fastest
   crop is 4.7 real hours cannot be played for an evening. Once the intent is
   named this is a data or constant change, not a system change. Plant the
   starter garden at world load in the same increment.
4. **A first-run sequence in the world.** Not a tutorial system: a scripted first
   ten minutes written as quest rows, using the engine that already has all six
   emitters. Walk here, press E on this, open this page, plant this, eat this.
   The engine work is surfacing the tracker (item 2) and a first-boot hand-off.
5. **Make a built thing do something.** Consume `Structure.provides` in the
   station gate at `src/lib.rs:11948` so building a furnace unlocks smelting. A
   dozen lines, and it turns construction from decoration into the material sink
   the ladder always intended.

Tier A is roughly one focused week and it converts a tech demo into a game.

### Tier B: make the construction tool good enough to build a city

6. **Pick one canonical layout schema.** Three exist: `ShipDef`/`DeckDef` (has
   decks, is not rendered), `Zone`/`HomeStructure` (is rendered, is single-plane),
   and `fibonacci.rs` `RoomConfig.level` (has levels, is the legacy pre-zone
   path). Nothing multi-storey can be built until one wins. `Zone`/`HomeStructure`
   is the obvious choice because it is the one the renderer, the editor and the
   collision system already speak.
7. **The four multi-storey blockers**, in order: a base Y on `InteriorWall`;
   Y-banded wall collision; a level-aware footing sampler; a deck-plate generator
   on the zone body so a floor exists above the ground plane. This is complaint 7
   answered and complaint 6 unblocked.
8. **Collision for generated geometry.** Today you walk through every residential
   clone. Feed `tile_home_clones` output into `ship_wall_segments`.
9. **Then author the acre with the tool**, partitioning the 4,565 m² of open
   shell into real rooms and moving the seventeen-station demo grid into a
   workshop. This is complaint 1, and it should be done in the editor so it
   proves the editor.

### Tier C: make the world look right

10. **Un-gate hero plant models for towers** (`src/engine/home_meshes.rs:888`)
    and add a `model` column to `plants.csv` so the 18 existing model families
    cover far more than 12 species.
11. **Conduit render pass:** elbow, bracket and gasket meshes, per-kind height
    offsets, trays for shared legs, sag for flexible runs, more tube segments.
12. **Models for the machines a player stands in front of daily:** smelter,
    workbench, cooker, composter, trading post, tower.
13. **Read `mesh_kind` in `zone_filler.ron`** so a mall is stalls and a hangar is
    cradles instead of thirteen opaque blocks.

### Tier D: multiplayer, in the only order that works

14. **Move `PlacedItem` out of `src/gui`** so `persistence` compiles into the
    relay. One type relocation; it unblocks everything below.
15. **Bridge `game_time_sync`.** Three lines in `src/engine/net_route.rs`.
16. **Player nameplates and appearance sync.** The name is already on the wire
    and thrown away.
17. **Make a trade move items**, with an `items.csv` id schema replacing the
    free-text fields.
18. **A `SystemRunner` host in the relay** for the systems that must be
    arbitrated, farming and construction first.

### Tier E: NPCs

Deliberately last, on the operator's own reasoning. One exception worth pulling
forward into Tier B: **a client-side NPC spawner**, so a solo player is not alone
in a premade city, even with placeholder behaviour. Everything else (loading
`npcs.ron` and `dialogues.ron`, pathfinding, animation, schedules, a character
model) waits for the loops those NPCs would perform.

### What is polish pretending to be foundation

The renderer arc, honestly assessed. The frame-cost work was real engineering and
the wins were large, but 294 of 1,755 commits went there while the loop a player
experiences got 22. The in-world screens ladder, the video player, the disc
reader and the embedded browser spike are finished work no player can reach,
because a player cannot yet keep a house they built. The recommendation is not to
undo any of it; it is to stop adding to it until Tier A holds.

Also not foundation: registering more of the 18 dormant systems. Each should wait
for the increment that gives it a visible consequence, which is what
`DEFERRED_SYSTEMS` already says. And `src/systems/interaction.rs` should be
deleted or given its consumer; a registered system that ticks and does nothing is
worse than an absent one.

---

## 7. What only the operator can decide

Two questions remain open (3 and 4). Question 1 was answered on 2026-09-20 and
question 2 on 2026-09-19; two others this report would have raised were already
answered: the
home's footprint is the acre and it is correct, and NPCs come after the loops.

1. **How fast should a crop grow in real time? ANSWERED 2026-09-20.** A growth
   MULTIPLIER, separate from the world clock, with 1x / 10x / 100x offered and a
   custom value allowed. Shipped default 10x. The operator, verbatim: "I would
   like to have normal real growth speed but, with a custom option for
   accelerating plant growth... it'd be nice for people to be like I want either
   1x speed or 10x or even 100x. For development purpose we could default to 10x
   growth speed (not clock speed) just so we can actually test plant life cycles
   without waiting days/weeks/months."

   Note what this deliberately does NOT do: it does not raise the clock, which
   would have dragged the day/night cycle, the seasons and the weather along with
   it, and it does not rewrite `growth_days`. `plants.csv` keeps its real
   agricultural numbers, so 1x stays a truthful mode and the figures stay
   teachable at every rung. Implemented in
   `src/systems/farming/mod.rs` (`DEFAULT_CROP_GROWTH_SPEED`,
   `clamp_growth_speed`), applied as one more factor beside health, nutrient and
   climate, published from `lib.rs` as `crop_growth_speed`, and exposed in
   Settings > Gameplay. Tier A item 3 is unblocked.

   **Offline growth is a separate, still-open want**, not part of this answer.
   The operator liked it and was unsure where it belongs: "I like the idea of
   offline growth but, that may be most applicable to MMO. Could be single player
   too." It needs its own decision and its own increment.
2. **Do the 3D models ship with the release?** Adding `assets/models/` and
   `assets/textures/` puts 269 MB into every download. Alternatives are shipping a
   curated subset, or a first-run asset fetch. Until this is answered no art work
   is worth doing. This blocks Tier A item 0.
3. **What is the first ten minutes?** Name the five things a brand new player
   should do in order. The quest engine can express any answer; nobody has
   written the answer down. This blocks Tier A item 4.
4. **Does a pipe read as its real material or as its utility colour?** The code
   contains both answers and the colour legend currently wins, so nothing is
   copper-coloured. A one-sentence taste call that shapes Tier C item 11.

A fifth, lower and not urgent: **does multiplayer enforce anything, or is it
co-operative trust until launch?** Every system runs client-local and
unarbitrated today, and Dev/Creative mode stays enabled in a shared world. If
trust is acceptable for now, Tier D stays cheap.

---

## Where this report is uncertain

- I did not boot the game. Everything here is read from code and data. Claims
  about how something looks follow from the code path; only a screenshot settles
  how it actually reads.
- I could not tell whether the operator has been judging the game from a repo
  build (models load) or a downloaded release (none do). That distinction changes
  the severity of complaints 1, 2 and 3 and is worth confirming before any art
  work starts.
- The multiplayer interpolation mismatch (the client sends at 15 Hz,
  `src/net/sync.rs:417` interpolates for 20 Hz, and dead reckoning uses a velocity
  hardcoded to zero at `src/engine/net_route.rs:213`) should produce visible
  stutter on remote avatars. The numbers are certain; the visible effect is not
  verified.
</content>
