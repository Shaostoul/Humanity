# Crowd navigation

People who walk somewhere: through doors, round furniture and round each other, at the speeds and
spacing real people keep. This is the whole feature, every rung, and the record of rung 1 as built.

Started 2026-11-13. Code: `crates/humanity-ship/src/crowd_nav/` (the mesh, the path, avoidance, the
crowd) and `src/engine/crowd_walk.rs` (the dev crowd walking on it). Status of each rung: section 10.

## 1. Why

- The dev crowd walked through furniture (the journal, 2026-11-11: "The dev crowd walks through
  furniture (crowd verb ignores obstacles)"). The `crowd` verb put people on circles round a point,
  so in the Commons' mess hall they walked through tables, counters and each other.
- The next videos need a crowd crossing the Commons, the hangar and the mall convincingly.
- The crew the relay walks go "straight line, no pathfinding yet" (docs/FEATURES.md, the chore
  rotation), kept clear of walls only by where their chores are placed.
- At MMO scale (thousands of people aboard a ship), every NPC, every visitor in the mall, every
  worker in the hangar needs the same three things: a floor plan it can plan over, a path, and a way
  not to walk into the next person.

## 2. What exists (2026-11-13)

| Piece | Where | What it does |
|---|---|---|
| Wall collision | `crates/humanity-ship/src/wall_collision.rs` | Every wall, door, stair side and guard as a 2D segment with heights and a kind; the walk sweeps a body against them. The navigation mesh reads the same segments, so a person and a walker agree on what blocks. |
| Furniture placements | `crates/humanity-ship/src/machines.rs` (`PlacedMachine`), `src/engine/home_meshes.rs` (`placements_now`) | Every machine and piece of furniture with its position, size and turn. The player does not collide with them today; the crowd does from rung 1. |
| Rooms graph | `crates/humanity-ship/src/layout.rs` | Rooms joined by doors, BFS between rooms: the coarse graph for long trips (section 4.3). |
| Ship zones and corridors | `crates/humanity-ship/src/ship_structure.rs` | The zones' boxes and decks, the corridors' tubes: where the floor is (section 3.1). |
| Crew NPCs | `src/relay/handlers/` (the chore rotation) | Walk in a straight line to a chore's site, 2 Hz `game_npc_update` to players who see them. |
| Flow field | `crates/humanity-systems/src/ai/flow_field.rs` | A 27-line stub ("supports million-agent navigation"): the type and nothing in it. Section 4.4 says what it becomes. |
| The dev crowd | `src/engine/animation.rs` (`CrowdFigure`, `CrowdMotion`), `src/engine/ipc.rs` (`crowd`) | Stand-ins for the rigs: circles, turning on the spot, a path back and forth. |

## 3. The navigation mesh

### 3.1 What it is built from: the layout as data

Everything the mesh reads is already data, the same data the walk and the renderer read:

- **Floors**: each ship zone's ground floor (its box at its deck height), each corridor's deck,
  each floor slab's walking surface (a storey upstairs), later each street's surface on a planet.
  A hole in a slab (a stairwell) is no rectangle and so no floor (`crowd_nav::floor::floor_rects`).
- **Walls and doors**: `wall_collision::ship_wall_segments`, with a door's aperture already cut.
  Which segments block is the walk's own rule (`WallSegment::stops_body`, the knee-to-eye band),
  so a wall upstairs does not block the storey under it.
- **Doors that do not open for a walker**: a locked door, a manual one, a window, as their shut
  panel's segment. An automatic, unlocked door is a gap; the door opens for walkers as they come
  (the crowd is a door actor, `home_meshes::render_door_panels`).
- **Furniture**: every placed machine and piece of furniture as a turned box, its foot to its top;
  it blocks when it stands across the knee-to-eye band (a rug does not, a table does, a lamp
  overhead does not).
- **Stairs, ramps, ladders, elevators, teleporters**: placed structures. Stairs' and ramps' sides
  block as walls today; rung 3 adds their walkable surfaces and the links between storeys.
- **Built pieces** (`build_place::built_piece_segments`) and **vehicles** parked aboard: rung 2.

### 3.2 How it is built (Recast's pipeline)

The technique is Recast's (Mikko Mononen, Recast Navigation, 2009, the navigation mesh builder in
Unity, Unreal, Godot and O3DE): rasterise the walkable floor into cells, erode it by the body's
radius so a body's centre anywhere on the mesh is clear of every obstacle, split the walkable cells
into regions, trace their contours, and make convex polygons joined by portals. The path is planned
over polygons, never cells.

- **Rung 1 (built)**: one storey of a region; cells of 0.1 m (larger past a million cells); a cell is
  walkable when it stands on a floor, its centre is a body's radius from every wall segment's face
  and every footprint's edge (exact distances, not a dilation), and it is a body's radius inside the
  floor's edge. Walkable cells are merged greedily into maximal rectangles, which are convex, and
  each pair of rectangles that touch shares a portal. A field of the nearest blocked cell (a
  two-pass vector distance transform, Danielsson, 1980) gives avoidance the edge of the walk in one
  read. Connected pieces of floor are labelled, so a destination is only chosen where a walk reaches.
- **Rung 2**: Recast's watershed regions and contour simplification in place of rectangles (far
  fewer polygons for the same floor, so A* and the funnel do less), still in our own Rust.
- **Rung 4**: a tiled mesh (Recast's tile cache): the ship cut into tiles of 16 m, each built on its
  own, so an edit rebuilds the tiles it touches and a deck of any size is streamed in by tile.

### 3.3 Storeys and the links between them

Each storey is its own layer (Recast's compact heightfield keeps every span of a column, so one
column holds the ground floor and the floor above). Stairs and ramps join two layers with walkable
polygons along their slope; ladders, elevators and teleporters join them with off-mesh links (Detour's
`dtOffMeshConnection`): a link has two ends, a cost in seconds (an elevator's wait), and the act that
crosses it (`humanity_animation` anchors already climb a ladder and ride a car), so A* plans a walk
that takes the stairs or calls the lift, and the person is drawn doing it.

### 3.4 Rebuilt when the layout changes, and only then

A mesh is kept in memory while what it was built from holds (CLAUDE.md, the reuse rule refined
2026-10-30): `NavCache` keys it by a fingerprint of its inputs (FNV-1a over every number of every
wall, door, footprint and floor that reaches the region and blocks on its storey), so a table moved
1 cm rebuilds it and a table moved in another zone does not. With the Dev page's "Turn off all
reuse" switch off (`humanity_core::reuse::enabled()` false), the mesh is built every frame. The
furniture list the engine reads from the placements is kept by the machine rebuild counter
(`HomeDraw::edits`, which every placement rebuild bumps), and read every frame with reuse off.

At tile scale (rung 4) the fingerprint is per tile, and a moved table rebuilds the one or two tiles
under it on a worker thread; the old tile serves until the new one is ready.

### 3.5 Dynamic obstacles

A crate set down, a vehicle parked, a door shut on purpose: each blocks the polygons under it until
it moves (Detour's tile cache "obstacles", which re-rasterise only their tiles). People are never
carved into the mesh; avoidance handles them (section 5).

## 4. The path

### 4.1 A* on the mesh, string-pulled

A* (Hart, Nilsson and Raphael, "A Formal Basis for the Heuristic Determination of Minimum Cost
Paths", 1968) over the polygons, each polygon entered at the middle of the portal it is entered by,
the straight-line distance to the destination as the heuristic. The corridor of polygons found is
pulled tight through its portals by the funnel algorithm (Lee and Preparata, "Euclidean shortest
paths in the presence of rectilinear barriers", Networks 14, 1984; the "simple stupid funnel"
form, Mononen, 2010): the shortest line through the corridor, its corners on the walkable floor's
corners. Portals are narrowed by half a cell at each end, so a corner keeps a body's radius off the
furniture it turns round.

### 4.2 Following a path

A walker steers for its path's next corner at its own speed and slows into the destination over
its last 0.8 m. Avoidance pushes it off the line, so it plans the path again from where it stands
every 1.5 s (rung 1). Rung 2 keeps the corridor of polygons and only re-pulls it (Detour's
`dtPathCorridor`: moving along the surface, then optimising the visible corners), which is cheaper
than a new A* and keeps the walker on its route through a crowd.

### 4.3 Long trips: the rooms graph first

A walk across the ship (a home to the mall, 400 m) plans first over the rooms-and-doors graph
(`layout.rs` already has rooms joined by doors, and BFS between them; rung 3 makes it Dijkstra by
walking time), then over the mesh only for the stretch to the next door: hierarchical path planning
(Botea, Müller and Schaeffer, "Near Optimal Hierarchical Path-Finding", 2004). The full A* over a
whole deck never runs per walker.

### 4.4 Many people to one place: flow fields

When hundreds head to the same place (the mess hall at a meal, the hangar at a launch), one field of
directions over the mesh toward that place serves all of them (Treuille, Cooper and Popovic,
"Continuum Crowds", SIGGRAPH 2006; the flow field `crates/humanity-systems/src/ai/flow_field.rs`
was stubbed for). A walker with a common destination reads the field instead of planning its own
path. Rung 5.

### 4.5 Where people prefer to walk

Polygons carry a cost per metre by what they are (Recast's area types): a walkway is cheaper than a
lawn, a lane through the market costs more at its busiest hour, a hazard area costs a lot. Rung 3.

## 5. Avoidance between people

### 5.1 ORCA

Optimal reciprocal collision avoidance (van den Berg, Guy, Lin and Manocha, "Reciprocal n-Body
Collision Avoidance", Robotics Research, ISRR 2009, Springer 2011): for each neighbour, the velocities
that would meet it within the time horizon form a cone; each person takes half the change that
leaves it, as a half-plane of allowed velocities; the velocity nearest the preferred one inside all
half-planes and under the person's top speed is found by a small linear program (incremental, in
two dimensions; when no velocity satisfies them all, the one that breaks them least, keeping the
edge of the walk a hard constraint). It is collision-free for people who all use it, smooth, and
cheap: each person reads at most 10 neighbours within 4 m, found through a grid. Our own code from
the paper's construction (`crowd_nav::avoid`).

The edge of the walk is one more half-plane for each person: the nearest edge, read from the mesh's
field of nearest blocked cells, may be closed on no faster than reaches it in 0.6 s. The move is
then kept on the walkable cells (the whole move, else its x or z alone, else none), so no body
centre ever enters furniture or a wall whatever avoidance chose.

### 5.2 Anticipation

People react to the time until they would collide, not to distance, and hardly at all past about 3
seconds (Karamouzas, Skinner and Guy, "Universal Power Law Governing Pedestrian Interactions",
Physical Review Letters 113, 238701, 2014). Rung 1 uses a 2.5 s horizon with ORCA. Rung 3 replaces
the half-planes with that paper's power-law energy (its "implicit crowds" solver, Karamouzas et al.,
SIGGRAPH 2017), which reproduces the measured spacing and the lanes that form in two-way corridors.

### 5.3 Personal space and side preference

- **Personal space**: Hall's proxemics (The Hidden Dimension, 1966) puts "personal distance" at
  about 0.45 to 1.2 m; walkers passing strangers accept less. Rung 1 keeps 0.05 m beyond each body's
  radius from another person. Rung 3 makes it a function of local density: about 0.3 m at walkway
  level of service A, shrinking to touch at E (section 7).
- **Side preference**: people in a two-way stream pass on one side (keep right in most of the
  world), which forms lanes (Moussaid, Helbing and others, "Experimental study of the behavioural
  mechanisms underlying self-organization in human crowds", Proceedings of the Royal Society B,
  2009). Rung 3 adds it as a bias in the preferred velocity, its side as data per culture.
- **Groups**: friends walk side by side, then in a V as it fills (Moussaid et al., "The walking
  behaviour of pedestrian social groups", PLoS ONE, 2010). Rung 5, with the NPC schedules that make
  groups.

### 5.4 Dense crowds

Past about 2 people a square metre ORCA turns jittery and pairwise. A crowd that dense (a
concourse at a launch, the mall at a festival) is simulated as a continuum (Narain, Golas, Curtis
and Lin, "Aggregate Dynamics for Dense Crowd Simulation", SIGGRAPH Asia 2009): the density and
velocity on a grid, each person following the field. Rung 6.

## 6. Doorway queueing

A door is a bottleneck: flow through it grows about linearly with its width (Seyfried et al., "New
Insights into Pedestrian Flow Through Bottlenecks", Transportation Science 43(3), 2009), and people
form a queue or a zipper of two lanes in front of it rather than pressing in.

- **Rung 2**: each door has a capacity in people a second from its width (the measured flow per
  metre of width; the number to take, from Seyfried's experiments and the planning handbooks, is
  to be checked against the paper before it is written into data). A walker whose path crosses a
  door asks the door for a turn; the door hands turns out at its capacity, one direction at a time
  for a door under 1.2 m (single file), both at once for a wider one; a walker waiting stands at a
  queue point back from the door, spaced by the queue's level of service (Fruin's queuing standards,
  section 7), facing the door.
- An automatic door opens for a walker coming to it (rung 1 already: the crowd are door actors);
  a manual or locked door is a wall in the mesh until someone with the right opens it.
- Rung 1 has no queue: a walker that has gone less than 0.3 m in 4 s gives its destination up and
  picks another, so a jam in a doorway dissolves instead of standing forever.

## 7. Real pedestrian numbers

| What | Number | Source |
|---|---|---|
| Free walking speed, mean | 1.34 m/s | Weidmann, Transporttechnik der Fussganger, ETH Zurich IVT Schriftenreihe 90, 1993 (checked 2026-11-13 in arXiv 1512.01426, which quotes his fundamental diagram's parameters) |
| Jam density | 5.4 people/m2 (Weidmann); other studies 3.8 to 10 | the same; the spread from the review in the same search |
| Free speed by purpose | shopping about 1.04 m/s, commuting about 1.45 m/s | quoted in the same review |
| Body | an ellipse 0.61 m by 0.46 m | Fruin, Pedestrian Planning and Design, 1971 |
| Walkway level of service | A over 35 sq ft (3.25 m2) a person, B 25 to 35, C 15 to 25, D 10 to 15, E 5 to 10, F under 5 sq ft (0.46 m2) | Fruin, 1971 (the bands as quoted on a Cornell course page, checked 2026-11-13; that page has a misprint in B, so read the bands against the book before they become data) |
| Personal distance | about 0.45 to 1.2 m | Hall, The Hidden Dimension, 1966 |
| Time horizon of avoidance | reactions follow time to collision, little past about 3 s | Karamouzas, Skinner and Guy, 2014 |

How they enter the game: each walker's speed is drawn from the measured spread round 1.34 m/s
(rung 3; rung 1 takes the `crowd` verb's speed or its mix), slower in a shop, faster on a commute;
the personal space a crowd keeps follows its level of service; a doorway's turns follow its width.
All of it is data (`data/crowd/walking.ron`), not numbers in code (`docs/design/infinite-of-x.md`).

## 8. The MMO scale budget

A ship of thousands, a mall of hundreds in view. The rule (CLAUDE.md, compute and bandwidth are a
hard budget): only gameplay state syncs, at a low rate; anything cosmetic runs on each client and is
never networked.

| Who walks | Simulated where | Rate | Network |
|---|---|---|---|
| Crew and NPCs with jobs (gameplay: they carry goods, staff the mess hall, answer you) | the relay, authoritative | path planned on the server when a job starts; avoidance at 10 Hz among the NPCs near any player, 1 Hz (path following only) far from everyone | the destination and the path's start time once (each client walks the same path with the same code), then a correction at 2 Hz only while avoidance has moved them off it: about 8 bytes a person an update (16-bit x and z, 8-bit facing, 8-bit state), to players within 40 m (the interest radius the crew updates already use) |
| Ambient people (shoppers, passers-by: atmosphere, no gameplay) | each client, on its own | full avoidance within 30 m of the camera, path following only past it, frozen and drawn as the cheapest LOD past 100 m | none: each client's mall has its own shoppers |
| Other players | their own client | their own input | their position as today |

Per-frame cost on a client, to hold (measured costs of rung 1 are in section 10):

| Load | Budget |
|---|---|
| 100 people near the camera, full avoidance | 0.3 ms |
| 1,000 people, 200 with avoidance, the rest following paths | 1.0 ms |
| 10,000 aboard, the rest frozen or on the server | 1.5 ms, the rest is the server's |

On the relay: 1,000 NPCs at 10 Hz with ORCA is about 2 ms a tick on one core at rung 1's per-person
cost; past that, the far ones step at 1 Hz. A mesh rebuild runs on a worker thread at tile scale
(rung 4), never in a tick.

## 9. What the relay's crew adopt

The crew's chore walk (`src/relay/handlers/`, "straight line, no pathfinding yet") takes the same
`crowd_nav` code, which is in `humanity-ship`, compiled into the relay already:

1. The relay builds the mesh of the ship's ground deck from `wall_collision::everyones_walls` (what it
   already builds for the crew's wall checks), the floors (`crowd_nav::floor::floor_rects`) and the
   furniture of the ship's machines, once at start and again when the ship's layout changes (the
   fingerprint).
2. A chore's walk is `find_path` from where they stand to the chore's site; the 2 Hz
   `game_npc_update` carries the path's corners once and the position as now, so a client draws them
   walking the path between updates instead of the straight line.
3. Avoidance among the crew (and round players, whose positions the relay has) at 10 Hz.
4. The mess hall's meal rush is where doorway queueing (rung 2) is first seen by players.

## 10. Rungs

| Rung | What | State |
|---|---|---|
| 1 | Mesh of one storey of a region (rectangles), A* and the funnel, ORCA, the dev crowd walking to destinations on it, doors opening for walkers, the cache by fingerprint obeying the reuse switch | built 2026-11-13 |
| 2 | Doorway queueing; Recast regions and contours for fewer polygons; the path corridor kept and re-pulled; built pieces and parked vehicles as obstacles | planned |
| 3 | Storeys joined by stairs, ramps, ladders, elevators (off-mesh links, the acts drawn); area costs; the rooms graph for long trips; personal space by density, side preference, measured speed spread; the power-law anticipation | planned |
| 4 | Tiled mesh rebuilt per tile on a worker; the relay's crew on it (section 9) | planned |
| 5 | Flow fields for shared destinations; groups; NPC schedules giving destinations (the mess hall at meals) | planned |
| 6 | Dense crowds as a continuum; ambient crowds by level of detail to 10,000 aboard | planned |

### 10.1 Rung 1 as built

- `crates/humanity-ship/src/crowd_nav/`: `mesh.rs` (the inputs, the rectangles, portals,
  components, the nearest-edge field, `NavCache`), `path.rs` (A*, the funnel), `avoid.rs` (ORCA and
  its linear programs), `crowd.rs` (walkers: destinations in their roam, paths, avoidance, the move
  kept on the walkable cells, facing), `floor.rs` (the floors of the ship's layout), `tests.rs`.
- `src/engine/crowd_walk.rs`: the dev crowd on it. Each frame with walkers it gathers the walls
  that block on the walkers' storey, the doors that do not open for them, the furniture (kept by the
  machine rebuild counter, read every frame with reuse off) and the ship's floors, ensures the mesh,
  and steps the walkers. Cost on the Performance page inside "Characters" (`cpu.crowd_nav`, a stage
  inside `cpu.animation`).
- The `crowd` verb: `{"crowd":"N,radius,speed,turn,ahead,walk"}`. Aboard, where the walls are,
  the walking stand-ins walk on the mesh by default: each starts on its ring, picks a destination
  within `radius` of the crowd's centre (or `nav:<roam_m>`), walks there, stands a moment and picks
  another. `walk` = `circle` keeps the old rings (a measurement that wants people at known places).
  Standing stand-ins stand on the mesh and step aside; turning ones turn on the spot as before.
- The crowd opens automatic doors as it comes to them (a door actor beside other players).

Tests (all seen red first; the red output is in each test's comment):
`crowd_nav::tests::a_path_from_one_room_to_another_goes_through_the_door`,
`no_agent_position_is_inside_a_furniture_footprint_over_a_simulated_minute`,
`two_agents_walking_head_on_pass_without_overlapping`,
`the_navmesh_rebuilds_when_furniture_moves_and_not_otherwise`,
`the_crowds_region_holds_while_its_walkers_walk` (the rig found the first engine glue building the
mesh 141 times in 10 s, its region following the walkers),
`a_path_from_the_commons_reaches_the_home_through_the_corridor` (the shipped ship), and
`the_walking_rules_in_data_parse`.

Vantages: `crowd-nav-mess-hall` (100 walkers among the mess hall's tables) and `crowd-nav-doors`
(60 walking in and out of the Commons' small rooms through their doorways).

Measured, 2026-11-13, the RTX 4070 PC, a release build, rig defaults (other agents' builds shared
the machine, so these are upper bounds; the rig marked the captures contaminated):

| What | Number |
|---|---|
| The mesh for the mess hall crowd: 24 x 24 m at 0.1 m (240 x 241 cells), 13 walls, 21 pieces of furniture, 9 floors | built once, 6.9 ms; 31,108 walkable cells in 71 rectangles |
| The mesh for the doors crowd: 22 x 22 m | built once, 6.2 ms; 43,392 walkable cells in 183 rectangles |
| `cpu.crowd_nav`, 100 walkers (the fingerprint, the walls near the region, every walker's behaviour, avoidance and move) | 0.41 ms a frame on average |
| `cpu.crowd_nav`, 60 walkers | 0.14 ms a frame on average (worst 0.40 ms) |

The 100-walker average is over rung 1's 0.3 ms budget (section 8) with the machine shared; a clean
measure and, if it holds, the per-walker cost (the A* replans every 1.5 s, the linear programs) are
rung 2's first work. A crowd's first frame also places every walker and plans every first path at
once, a one-frame spike (worst seen 68 ms on the shared machine, with the frame's worst at 334 ms);
rung 2 spreads the first plans over frames.

## 11. Open questions for the operator

Kept in the report of the work that raised them; decided ones move into the sections above.

## 12. References

- Mononen, M. Recast Navigation (2009 onward), and "Simple Stupid Funnel Algorithm" (2010).
- Hart, P. E., Nilsson, N. J., Raphael, B. "A Formal Basis for the Heuristic Determination of
  Minimum Cost Paths". IEEE Transactions on Systems Science and Cybernetics 4(2), 1968.
- Lee, D. T., Preparata, F. P. "Euclidean shortest paths in the presence of rectilinear barriers".
  Networks 14(3), 1984.
- Danielsson, P.-E. "Euclidean distance mapping". Computer Graphics and Image Processing 14, 1980.
- Botea, A., Muller, M., Schaeffer, J. "Near Optimal Hierarchical Path-Finding". Journal of Game
  Development 1(1), 2004.
- van den Berg, J., Guy, S. J., Lin, M., Manocha, D. "Reciprocal n-Body Collision Avoidance".
  Robotics Research (ISRR 2009), Springer Tracts in Advanced Robotics 70, 2011.
- Karamouzas, I., Skinner, B., Guy, S. J. "Universal Power Law Governing Pedestrian Interactions".
  Physical Review Letters 113, 238701, 2014 (arXiv 1412.1082).
- Karamouzas, I., Sohre, N., Narain, R., Guy, S. J. "Implicit Crowds: Optimization Integrator for
  Robust Crowd Simulation". ACM Transactions on Graphics 36(4), SIGGRAPH 2017.
- Treuille, A., Cooper, S., Popovic, Z. "Continuum Crowds". ACM Transactions on Graphics 25(3),
  SIGGRAPH 2006.
- Narain, R., Golas, A., Curtis, S., Lin, M. C. "Aggregate Dynamics for Dense Crowd Simulation".
  ACM Transactions on Graphics 28(5), SIGGRAPH Asia 2009.
- Moussaid, M., Helbing, D., Garnier, S., Johansson, A., Combe, M., Theraulaz, G. "Experimental
  study of the behavioural mechanisms underlying self-organization in human crowds". Proceedings of
  the Royal Society B 276, 2009.
- Moussaid, M., Perozo, N., Garnier, S., Helbing, D., Theraulaz, G. "The Walking Behaviour of
  Pedestrian Social Groups and Its Impact on Crowd Dynamics". PLoS ONE 5(4), 2010.
- Seyfried, A., Passon, O., Steffen, B., Boltes, M., Rupprecht, T., Klingsch, W. "New Insights into
  Pedestrian Flow Through Bottlenecks". Transportation Science 43(3), 2009 (arXiv physics/0702004).
- Fruin, J. J. Pedestrian Planning and Design. Metropolitan Association of Urban Designers and
  Environmental Planners, 1971.
- Weidmann, U. Transporttechnik der Fussganger. Schriftenreihe des IVT 90, ETH Zurich, 1993.
- Hall, E. T. The Hidden Dimension. Doubleday, 1966.

## His answers (operator, 2026-11-13)

Crowds navigate by default aboard (circle stays for measurements), and the player's own character stops at furniture the way the crowd does: "Agreed. We want the ship to feel alive."

**Built 2026-11-13.** The furniture list the crowd's mesh reads is the player's too: one conversion
(`Footprint::of_placed`), one list kept current by `crowd_walk::refresh_furniture` (by the machine
rebuild counter, every frame with reuse off), and the walk's collision (lib.rs, beside the shut doors
and the built pieces) takes each footprint near the body as five blockers between its heights
(`Footprint::walls`: the four edges, for square corners, and a core along its long axis, which
settles a body found inside it back out). The heights are the walls' rule: a rug under the knee is
walked over, a table or a bed (0.6 m) is not. While an interaction holds the body (seated, lying,
getting up) no furniture is read, since the seat stands where the body is; the walk takes it back at
the approach point. Every shipped seat's and bed's approach stands clear among the real furniture
(`crowd_walk::tests::the_player_still_reaches_every_seat_and_bed_on_the_ship`). A body placed inside
a piece wider than about 1.45 m can settle into a corner pocket instead of out (the core is one
segment); walking never takes it there.
