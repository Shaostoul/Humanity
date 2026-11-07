# World audit, 2026-11-06: everything placed in today's game world

Status: an audit written 2026-11-06 for the fresh start of the game world. It changes nothing.
The plan that acts on it (the whole-ship layout, the wipe and the rebuild) is
[mothership-master-plan.md](mothership-master-plan.md).

Why it exists, in the operator's words (2026-11-06): "Right now our game world is very haphazard
with homestead and community elements weirdly placed with some just floating in space. I was
thinking we should do a wipe of the entire homestead and start fresh to make sure we get all the
weird artifacts."

## How it was done

- **Every data file that places something** was found by searching `data/` for positions
  (`pos`, `position`, `origin`, `offset`, `spawn`, `spot`, `post`), then read, and every
  reader was found by searching `src/`, `crates/`, `scripts/` and `tests/` for the file's name.
- **Where things land** was computed, not eyeballed. Three scripts in the session scratchpad
  read the data files of this tree (HEAD 5a62cf2ae) and classify every point against the ship
  file's boxes (zones, plots, corridor tubes, districts):
  `C:\Users\Shaos\AppData\Local\Temp\claude\f6ab35374b1e8799\1c730720-392c-4cef-bc76-fe12d380cf51\scratchpad\shp-classify.js`
  (districts, wild spawns, fixed quest places, code-placed points),
  `...\scratchpad\shp-commons.js` (the Commons' machines against the Commons' walls) and
  `...\scratchpad\shp-lights.js` (each light against its body's box). Wild spawns are scattered
  exactly as `spawn_wilds` scatters them (src/engine/world_load.rs:951 onward).
- **One boot** took the screenshots: the main checkout's release build of 2026-11-04 (the only
  one on the machine; this task builds nothing), booted by `scripts/probe-sweep.js` with
  `--allow-other-build` and this tree's data, six audit vantages
  (`...\scratchpad\shp-vantages.json`). Five captured; the sixth (a whole-ship overview from
  180 m up) left the station's frame and is not the view asked for. The placements come from
  the data files, which the build only draws.

"The void" below means a point inside no zone, no plot and no corridor tube: there is no floor
there, only the visual hull wrap around the cluster, and the stars underneath.

## 0. The worst findings

1. **Thirteen of the fourteen ship districts float in space.** Hangar Bay 1, Mech Bay 1,
   Reactor 1, the Medical Bay, the Armory and the Arena stand at x 200 to 250; the Factory
   District, Storage, Cargo Freight and the Agricultural Bay at x -190 to -150; the Transit Hub
   at x 130 to 154. The hull wraps only x -6 to 107. They are coloured filler blocks with no
   floor, outside the hull, and no corridor reaches any of them. The Concourse (x 80 to 120)
   sticks 13 m out of the hull's side and is not reachable either. (data/blueprints/ship_structure.ron:1568-1660;
   screenshot `audit-districts-east.png`.)
2. **Eleven wild resource nodes stand in the void, and eight more inside the Commons.** The
   forage and resource rows of data/entities/wild_spawns.ron are absolute positions authored
   for a planet-era homestead (its own header still says "the home footprint sits roughly x
   0-40, z 0-70"). Today a berry bush stands 0.3 m outside p1's east wall, fallen logs and a
   stone outcrop in the 10 m gap between p1 and the Commons, flax past p1's south wall, salt
   flats inside the Commons' walled room block, sand pits and stone outcrops on the Commons'
   floor and three "rare ore veins" in First Street. They do not move with the home: a
   homestead on any other plot still gets them around p1 and the Commons. (Section 5;
   screenshot `audit-p1-east-gap.png`.)
3. **The Commons' garden is cut by walls.** The 3 x 3 tower grove, the fish tank, the composter
   and the mushroom rack, placed mid-hall in v0.768 (2026-07-08), had the old home's showroom
   walls translated in around them on 2026-07-28 (v0.1023.1). The mushroom rack, its tent
   humidifier and its CO2 fan stand exactly on the room block's south wall; three towers stand
   0.2 m from it; the grove straddles the block's x 16 partition. No test checks the Commons'
   machines against walls (`every_placed_machine_stands_inside_the_room_it_names` skips every
   machine whose zone is not "home", crates/humanity-ship/src/machines.rs:3181). (Section 2.1;
   screenshots `audit-commons-block-open-side.png`, `audit-commons-from-above.png`.)
4. **Six of the Commons' lights shine outside the Commons**: four inside the player's own home
   on p1 (a warm lamp at 0.8 m in the dining room, a strip at 1.0 m in the kitchen, a cool
   panel in the greenhouse, a 39-intensity spotlight in the orchard court) and two in the void.
   They belong to the Commons body, so they are charged to every player's meter like every zone
   light. Lights are placed at body origin plus offset with no clamp (src/engine/home_meshes.rs:3100-3103).
   They arrived with v0.1026.1 (2026-07-28, "homestead lighting").
5. **The Commons holds the previous home's walls.** Its walled "room block" (Commons-local x 4
   to 20, z 9 to 29) is the old 12-room dwelling block, moved in whole, open on its east side
   because in the home that side was the home's own shell; its rooms have no labels or purpose.
   South of it stands the curved eight-material showroom ring. Crew walks, the guest's arrival
   point and the rigs' meeting route have each had to be moved around the block.
6. **The solar-system hologram hangs in space** half a metre outside p1's west wall, placed by
   code, not data (src/engine/world_load.rs:208 and :1312-1314). Screenshot
   `audit-hologram-outside-p1.png`.
7. **A guest's put-away home is parked 1 km off the ship and 200 m below its deck**
   (`HOME_AWAY_ORIGIN`, crates/humanity-ship/src/ship_structure.rs:423), still simulating, its
   machines floating under it when seen from outside (ship-homes-and-logistics.md:849).
8. **Four quest places are fixed ship coordinates from the planet era**: "the outdoor fields"
   is now the greenhouse, "the berry thicket" and "the flax hollow" are in p1's barn and fields,
   and "the northeast ridge" is First Street. They never follow the player's plot
   (data/entities/destinations.ron; `pos` is a "World XZ centre",
   crates/humanity-systems/src/quests/mod.rs:158-160).
9. **The operator's own car is parked in the void** at ship (91.07, 0, 148.29), east of the
   cabin block, in his save `%APPDATA%\HumanityOS\saves\offline_home.json`
   (`deployed_vehicles`, `outside_home: false`).
10. **Six data files place things nobody reads**: data/world/spawn.ron (a "bedroom" spawn in a
    Fibonacci home and a geosynchronous orbit over Silverdale), data/blueprints/habitat.ron (a
    60 x 60 m Fibonacci habitat) and data/ships/bridge.ron, layout_medium.ron, reactor.ron and
    starter_fleet.ron (the relay's old six-room frigate, the Pioneer). Each is compiled into the
    exe (crates/humanity-core/src/embedded_data.rs:115-124, :141) and no code asks for it.

What is NOT wrong: the player's home itself. Every one of its 55 lights is inside its box, every
machine of home.ron and home_solo.ron stands in the room it names (a test since 2026-09-19), every
door joins two rooms, and the stair meets the stairs finding. Its internal issues are smaller and
listed in section 3.

## 1. The ship's structure

The ship file is data/blueprints/ship_structure.ron (1,662 lines). One frame, metres, y up, the
origin at plot p1's corner. The hull is visual only and wraps zones, plots and corridor tubes,
not districts (crates/humanity-ship/src/hull.rs:331-356, `world_bounds` at
crates/humanity-ship/src/ship_structure.rs:1517-1530).

| Thing | Where (ship metres) | Placed by | What is wrong |
|---|---|---|---|
| Zone `commons` "The Commons" | x 65..99, z 20..75, 8 m, glass roof | ship_structure.ron:109-1031 | Holds the previous home's walls (section 2.1); 6 of its 25 lights stand outside it (finding 4) |
| Zone `street-1` "First Street" | x 65..75, z 85..1185, 4 m, glass roof, no lights | :1033-1053 | A 1.1 km corridor with homes on one side only (plots cannot turn); three rare-ore veins stand in it |
| Zone `cabins-1` "Cabin Block A" | x 77..80, z 88..136, 3 m | :1055-1110 | Its washroom's fixtures are not built (header :61); fine otherwise |
| Zone `commons-2` "The South Commons" | x 77..101, z 770..810, 4 m | :1111-1142 | Exists only to keep p5 to p12 within 5 minutes of a mess hall along a 1.1 km street; a symptom of the street layout |
| Corridors (3 zone-to-zone) | Commons to street at x 70; street to cabins at z 92; street to South Commons at z 790 | :1144-1172 | None |
| Plots p1..p12 (homestead) | x 0..55, z 99(N-1) .. +89, 6 m | :1173-1341 | West side only; the 10 m gaps between them are void, not ground |
| Plots c1..c16 (cabin) | x 80.5..83.65, z 88..128, 3 m | :1342-1566 | None |
| The gap between p1 and the Commons | x 55..65, z 0..89 | (nothing) | Void inside the hull: logs, a stone outcrop, a berry bush and a Commons light stand in it |
| District `res-1` "Residential District A" | x 0..100, z 0..1195 | :1569-1575 | Draws nothing (its homes are the plots); 4 m past the hull's end |
| Districts `hangar-1`, `mech-1`, `power-1` ("Reactor 1"), `medical-1`, `armory-1`, `arena-1` | x 200..250, z 0..189 | :1576-1617 | Outside the hull, no floor, no corridor (finding 1) |
| District `mall-1` "The Concourse" | x 80..120, z 205..245 | :1618-1624 | Partly outside the hull; no floor, no corridor; the three market stalls it should hold are in the Commons |
| District `transit-1` "Transit Hub" | x 130..154, z 205..229 | :1625-1631 | Outside the hull; nothing transits |
| Districts `industrial-1`, `storage-1`, `cargo-1`, `agri-1` | x -190..-160, z 0..120 | :1632-1659 | Outside the hull, no floor, no corridor |
| The hull wrap | x -6..107, z -6..1191, a profile lofted along z | data/blueprints/hull_profile.ron, crates/humanity-ship/src/hull.rs | Its station labels ("full beam over the home", "approaching the Commons") were written for an X-axis ship; since the twelve plots it lofts along z, so the "bow" is behind p1 and the two engine greebles sit past p12 |
| The rail graph | every body's `rail_nodes: []` | ship_structure.ron | Empty: the home's train platform (`train-1`) has nothing to run to |

**Scale.** The whole built ship is a flat slab 1.2 km long, about 113 m wide and about 13 m tall,
one deck at y 0. Every decided description of the mothership is something else: a drum of 250 m
radius and 2 km length for homes and gardens and a spine with thrust gravity (habitat-generation.md,
2026-09-15). The master plan takes this up.

## 2. Community and commons elements

### 2.1 The Commons (zone `commons`, Commons-local metres, ship = local + (65, 0, 20))

| Thing | Commons-local | Placed by | What is wrong |
|---|---|---|---|
| Four market-stall dividers | x 30..34 at z 8, 19, 33, 47 | ship_structure.ron:121-155 | None (the stalls should be in a market; see the Concourse) |
| The curved eight-material showroom ring (8 walls, one per material 1..8, each a different door) | x 8..20, z 30.5..42.75 | :157-325 | A teaching exhibit kept on purpose (homestead.md:146-148, the forever-development rule) but standing loose in the hall with one side open; it was the old home's materials and doors showroom |
| The walled "room block" (15 walls, unlabelled rooms) | x 4..20, z 9..29, no east wall | :327-720 | The superseded 12-room dwelling block of homestead.md section 3, moved in on 2026-07-28 (v0.1023.1: "all 23 old wall segments translate into the commons"); open on its east side, rooms with no purpose, walls built through the garden |
| Garden: `mushhum_c1`, `mushfan_c1`, `mush_c1` | (11.5, 29) | data/machines/ship.ron:494-535 | Stand exactly on the block's south wall (0.00 m) |
| Garden: `aqua_c1` (fish tank), `composter_c1` | (11.5, 24), (11.5, 26.5) | ship.ron:512-527 | Inside the walled block |
| Garden: the 3 x 3 tower array `ctower_0..8` | origin (14, 24), 2.4 m apart | ship.ron:937-948 | Inside the block, straddling its x 16 partition (0.4 m from it), the south row 0.2 m from its south wall |
| Apothecary towers `apoth_c1`, `apoth_c2` | (21.5, 24), (21.5, 32) | ship.ron | In the open hall, east of the block's open side |
| Market stalls `market_c1..3` (trading posts) | (32, 12), (32, 26), (32, 40) | ship.ron:552-575 | Fine where they are, but they are the ship's whole market, and the Concourse district is empty |
| The mess hall (volume `mess-hall`): a serving counter, 4 tables, 16 chairs, the crew's long table | x 0..30, z 0..8 | ship_structure.ron:1011-1018; ship.ron | Fine |
| The recreation room (volume `recreation`): screen, rug, two couches | x 0.5..10.5, z 43.5..51.5 | :1019-1026; ship.ron | Fine |
| 25 lights | | ship_structure.ron:721-1005 | 6 outside the zone: lines 757, 768, 779, 790, 812, 823 (finding 4) |
| The guest's arrival point (`spawn`) | (22.5, 47.5) = ship (87.5, 67.5) | :1006 | Fine since increment 4 (it used to be inside the room block) |
| The ship's food store | mess-hall spot (2, 2) | data/food/ship_stores.ron:72 | Fine |
| Crew chores (16) and posts (6) | mess hall and the Commons' east aisle | data/npc/chores.ron, data/npc/crew.ron | Fine, but every spot was chosen to dodge the room block |
| Wild nodes inside the Commons | salt flats (77.9, 42.8) and (79.5, 35.0) inside the block; sand pits (85.5, 57.6), (93.0, 60.2); stone outcrops (71.6, 51.1), (71.9, 60.3); a fallen log (65.7, 67.0), ship metres | data/entities/wild_spawns.ron | A salt flat and a sand pit on a mall floor (section 5) |

**Two furniture sets.** The relay furnishes every room a second time, invisibly, for AI agents'
perception and the survey_storage quest: its furniture (data/ships/room_equipment.ron, by room
type) is placed by CODE on a ring around each room's middle (src/relay/handlers/game_state.rs:679-695),
and in the Commons that ring crosses the room block (ship-homes-and-logistics.md:917). It agrees
with nothing the game draws.

**Power.** The Commons' tent humidifier, CO2 fan and fish tank run off the family home's battery
bank (`battery_7`, data/machines/ship.ron header :15-21), and every zone's lights are charged to
each player's own home meter (ship_structure.ron header :43-44), so First Street, the cabin hall
and the South Commons have no lights at all.

### 2.2 The cabin block, the South Commons, First Street

The cabin block (16 cabins on one hall) and the South Commons (a mess hall with its own store)
are placed correctly. What is wrong is the layout they serve: a single 1.1 km street with homes
on one side forces a second commons halfway down it, puts the far homes out of the relay's 250 m
view of the Commons, and leaves the street unlit.

## 3. The player's home

The home is a design file, data/homes/homestead.ron (2,440 lines), copied at
data/homes/shipped/homestead.ron, placed by the loader at the player's plot (p1 by default,
`default_plot` at ship_structure.ron:1567). Machines are data/machines/home.ron (family) and
home_solo.ron (solo), home-local metres. It is the operator-directed redesign of 2026-09-19
(player-home.md) plus the upper floor of 2026-10-20 (homestead-upper-floor.md): 55 x 89 x 6 m,
30 room zones, 55 lights, 7 structures, spawn at the front door (53.5, 40.5).

Checked and fine: lights (0 of 55 outside the box), machines (a test), doors, the stair. What is
still wrong inside it:

| Thing | Where (home-local) | Placed by | What is wrong |
|---|---|---|---|
| `train-1`, a train platform, with `ramp-1` its boarding ramp | (48, 0, 10), (48, 0, 17.4), the vehicle bay | homestead.ron:2158-2164 | A platform inside a private home with no rail link (`pair: None`, every rail graph empty): a tech-demo piece from the materials showroom era |
| `teleporter-1` / `teleporter-2`, a pair 60.6 m apart inside one home | (22.5, 0, 20), (31, 0, 80) | homestead.ron:2165-2192 | Tech-demo pieces; whether teleporters exist at all is open question 10 of ship-homes-and-logistics.md |
| The hologram (the solar-system orrery) | home corner + (-0.5, 1, 2.5): outside the west wall | code: src/engine/world_load.rs:208, :1312-1314 | In space (finding 6); a start's content (`Hologram`, data/homes/starts.ron:139) with no place in the design |
| Greenhouse cool panels | 2.7 m up under the 6 m glass roof | homestead.ron lights | Left open in homestead-upper-floor.md:455-456 |
| Service runs | drawn as ceiling cylinders across living rooms | home.ron connections | The conduit node graph is empty (player-home.md:358-366) |
| home.ron's `loops` prose | "8 solar panels", "26 towers", "2 aquaponic fish tanks" (home.ron:16-19) | home.ron | Disagrees with the machine set (10 panels, home.ron:5107); the fish tank count includes the Commons' |
| Metering | the family home's meter carries the Commons' garden and every zone's lights | home.ron `ship_machines: Some("ship.ron")` | The ship's machines are on a household's meter (a later increment, never built) |
| No respawner | the shipped homes have no respawner room; respawn stands you at the front door | homestead.ron `spawn` | game-setting-and-intro.md:751-755 wants a pod beside the bedroom |
| Wild spawns, decorations, herd, showcase garden | the homestead start's `home_content` (starts.ron:139) | world_load.rs | The herd, decorations and garden are anchored to machines and move with the home; the WILD spawns are absolute and do not (section 5) |

## 4. Anything in space

| Thing | Where | Placed by | Verdict |
|---|---|---|---|
| Thirteen districts | x 200..250, x 130..154, x -190..-150 | ship_structure.ron:1576-1659 | Wrong: floating filler blocks |
| The hologram | (-0.5, 1, 2.5) | code | Wrong: outside p1 |
| A guest's put-away home | corner (-1000, -200, 0) | code (`HOME_AWAY_ORIGIN`) | Wrong: still simulated, machines float under it |
| Eleven wild nodes, two Commons lights, the operator's car | the void between boxes | data, his save | Wrong |
| The UHS Mothership's orbit | synchronous, over Silverdale, LVLH attitude | data/stations/home.ron | Consistent in itself; whether the fleet is at Earth or between the stars is open (game-setting-and-intro.md question 2, leaving-the-ship.md question 1) |
| Three asteroids | km from the ship frame: (60, 12, -30), (-45, 8, 55), and C-3 | data/asteroids/belt.ron | Fine: placed in space on purpose, read from data |
| Dev's Land on Earth | the Oahu coast, lat 21.3, lon -157.8 | code: src/lib.rs:6032-6036 | A planet-side default coordinate in code (leaving-the-ship.md question 11) |

## 5. Wild spawns and fixed quest places

Each row of data/entities/wild_spawns.ron scatters `count` copies within `radius` of an ABSOLUTE
ship point (src/engine/world_load.rs:951). Computed instances:

| Row (line) | Instances in the void | Inside p1 | Inside the Commons | In First Street |
|---|---|---|---|---|
| berry_bush x4 (:24) | (55.3, 82.0), (55.7, 86.8) | 2 | | |
| wild_flax x6 (:25) | (17.7, 90.5), (14.5, 90.0), (20.1, 92.5) | 3 | | |
| fallen_log x3 (:30) | (55.4, 68.8), (57.3, 77.5) | | (65.7, 67.0) | |
| stone_outcrop x3 (:31) | (63.8, 51.8) | | 2 | |
| clay_pit x2 (:32) | | 2 | | |
| salt_flat x2 (:33) | | | 2, both inside the room block | |
| sand_pit x2 (:34) | | | 2 | |
| ore_outcrop x3 (:48) | | 3 | | |
| rare_ore_vein x3 (:49) | | | | 3 |
| **Total (28)** | **11** | **10** | **7** (+1 log) | **3** |

The quests lean on them: Initial Survey's ore samples and Distant Expeditions' rare ore come from
the outcrop and the vein (`quest_ore_comes_from_nodes_a_player_can_walk_to`,
crates/humanity-systems/src/quests/mod.rs).

Fixed quest places (data/entities/destinations.ron), all ship coordinates:

| id | Label | Where | Lands in |
|---|---|---|---|
| outdoor_fields | "the outdoor fields" | (29, 63), r 10 | p1's greenhouse (the fields are at z 73..89) |
| wild_thicket | "the berry thicket" | (52, 82), r 8 | p1's barn |
| flax_hollow | "the flax hollow" | (18, 88), r 8 | p1's fields, half past its south wall |
| wolf_ridge | "the northeast ridge" | (75, 100), r 12 | First Street and the cabin hall |

The three other places (`front_door`, `mess_hall`, `recreation`) are anchors that follow the
player (`OwnFrontDoor`, `NearestVolume`): the right pattern.

## 6. Orphaned data and legacy code

| File or code | Status | Evidence |
|---|---|---|
| data/world/spawn.ron | No reader | compiled in (embedded_data.rs:115, :415); no other reference in src/ or crates/ |
| data/blueprints/habitat.ron | No reader | embedded_data.rs:141; `HabitatDefinition` appears nowhere else |
| data/ships/bridge.ron, layout_medium.ron, reactor.ron | No reader | embedded_data.rs:121-123 only |
| data/ships/starter_fleet.ron (the Pioneer) | No reader | named only in comments recording red evidence (src/relay/handlers/ship_world.rs:4, :487) |
| The relay's Pioneer world upgrade | Backward-compatibility code (CLAUDE.md: none before v1.0) | `PREVIOUS_PERSIST_KEY` "game_world_snapshot_v9" (src/relay/handlers/game_state.rs:1631), the upgrade at ship_world.rs:304-380, fixture tests/fixtures/relay/pioneer_world_v9.json |
| data/blueprints/homestead_layout.ron, fibonacci_homestead.ron, crates/humanity-ship/src/fibonacci.rs | Still read: the Home page's design view and its bill of materials, and the editor | Not world placements; the only "respawner" room lives here, and src/engine/world_load.rs:698 still looks for it first |
| data/blueprints/zone_filler.ron | Read (districts, stock piles) | Draws the floating district blocks; stays as a registry once districts are real zones |

## 7. Positions decided in code (infinite-of-x violations)

Live code holds few literal positions (a search of non-test code for `Vec3::new` literals with a
component of 5 m or more found three, none a world placement). The violations are code that
decides WHERE, or reads data in the wrong frame:

1. **The hologram's place**: `Vec3::new(-0.5, 1.0, 2.5)` as the fallback and `hologram_spot`
   (src/engine/world_load.rs:208, :1312-1314).
2. **The guest's put-away home**: `HOME_AWAY_ORIGIN = (-1000.0, -200.0, 0.0)`
   (crates/humanity-ship/src/ship_structure.rs:423).
3. **The relay's furniture ring**: positions computed around each room's middle
   (src/relay/handlers/game_state.rs:679-695).
4. **Wild spawns read in the ship frame**: `Vec3::new(s.pos.0, 0.0, s.pos.1)`
   (src/engine/world_load.rs:951); the data has no frame field, so a homestead on p5 gets p1's
   nodes.
5. **Quest places read in the ship frame**: `DestinationDef::pos`, "World XZ centre"
   (crates/humanity-systems/src/quests/mod.rs:158-160).
6. **Light offsets unclamped**: a zone light is placed at body origin + pos with no check that it
   is inside its body (src/engine/home_meshes.rs:3100-3103); nothing at load refuses one outside.
7. **Dev's Land**: Earth's landing point is the Oahu coast in code (src/lib.rs:6032-6036).
8. **Rooms by id**: the avatar podium looks for a room named "respawner" before reading room
   actions (src/engine/world_load.rs:698), a legacy id from the Fibonacci layout.

Tests pin positions too, which is right for tests but is the list of what a new layout must
update: `P1_DOOR`, `P2_DOOR`, `COMMONS` (src/engine/home_plot.rs:1881-1884); the stair, ladder
and elevator tops (src/engine/survival_env.rs:565-568); the relay's `room_for_position` points
(src/relay/handlers/ship_world.rs:540-541); 52 Rust files read the shipped ship or home files
(145 call sites).

## 8. The operator's own save

`%APPDATA%\HumanityOS\saves\offline_home.json` (560 KB, last written 2026-11-05):

- **Kept by any plan:** character name, appearance and outfit; 56 inventory stacks; 66 placed
  items (in containers, not in the world); 10,164 credits; skills; quests.
- **World positions:** `player_position` (0, 0, 0); one car at ship (91.07, 0, 148.29), in the
  void; `home_plot_box` null; no built pieces (`constructions` empty); no herd.
- **His garden:** 1,976 crops keyed by machine id: 1,575 dead (80%), 383 mature, 10 ripe, 8
  seeds. 1,200 of them are in the home's variety towers (`ntower`), 450 in the Commons' towers
  (`ctower`), the rest in apothecary towers, oil beds, racks and fields. It is the perpetual
  showcase's planting (data/world/showcase.ron), mostly dead.
- **His installed data folder** (`%APPDATA%\HumanityOS\data`, stamped v0.1525.0 on 2026-10-20)
  holds older copies of the ship file, the home design and the machine files. The refresh
  (crates/humanity-core/src/storage/refresh.rs) replaces a file nobody edited and keeps one the
  player saved from the editor; Settings > Data lists the kept ones.

## 9. The checks that should have caught these

| Gap | What a check would say |
|---|---|
| Nothing checks that a placed thing stands on a floor | "salt_flat#0 at (77.9, 42.8) is inside the Commons' room block"; "berry_bush#0 at (55.3, 82.0) is in no zone, plot or corridor" |
| The Commons' machines are skipped by the room test | "mush_c1 stands on the wall (4,29)-(20,29)" |
| Lights are not checked against their body | "commons light 757 warm_lamp at (-27.4, 0.8, 12.75) is outside the Commons" |
| Districts are "areas, not boxes, and are not checked" (ship_structure.ron header :28) | "hangar-1 is outside the hull and no corridor reaches it" |
| Wild spawns and quest places have no frame | "wild_thicket sends a p5 homesteader into p1's barn" |
| Orphaned data files are not detected | "data/world/spawn.ron is compiled in and read by nothing" |

The master plan's rung 1 turns each row into a test, seen red against today's data first.

## Screenshots

All in the session scratchpad,
`C:\Users\Shaos\AppData\Local\Temp\claude\f6ab35374b1e8799\1c730720-392c-4cef-bc76-fe12d380cf51\scratchpad\shp-shots\`:

- `audit-commons-block-open-side.png`: from inside the Commons looking west into the room
  block's open east side; the tower grove stands between the block's walls, the HUD names the
  place "commons:room_1".
- `audit-commons-from-above.png`: the Commons from 16 m up, hull hidden: the room block, the
  curved showroom ring south of it, the market stall dividers on the east wall, the recreation
  room's couches, and p1's rooms beyond the west wall.
- `audit-p1-east-gap.png`: the gap between p1's east wall and the Commons: a log, berry bushes
  and a stone outcrop standing on nothing, stars underneath.
- `audit-hologram-outside-p1.png`: the orrery hologram in space beside p1's dark west wall.
- `audit-districts-east.png`: the district row at x 200 floating in space east of the hull:
  ship cradles, racks and arrays with no floor.
- `audit-ship-overview.png`: NOT the view asked for (the camera left the station's frame at
  180 m); kept for inspection only.
