# Structural building pieces (stairs, ladders, elevators, teleporters, trains, roads)

Status: Stage 2 shipped (v0.583 placement + v0.584 walkable footing & teleporters).
Owner surface: `data/blueprints/structure_types.ron` + `src/ship/structure.rs`.

## Why

The operator's tech-demo home needs every structural element a real building has:
"stairs, ladders, elevators, teleporters, trains, roads, etc." Rather than a bespoke
mesh + placement + behaviour path per type, every non-machine, non-wall structural
element is ONE data entry. Add a buildable by adding a line to the .ron -- no code
(infinite-of-X). The construction "Structure" palette renders the same list an AI can
enumerate, and the console verbs `add_structure` / `rm_structure` act on it.

## Data model

`StructureType` (registry, `structure_types.ron`):
- `id`, `label`, `category` (palette tab; "Structure" today)
- `kind` -- the SEMANTIC tag that drives behaviour: `Wall` (drawn, not placed),
  `Stairs`, `Ladder`, `Elevator`, `Teleporter`, `Train`, `Road`.
- `shape` -- the parametric GEOMETRY builder: `Box`, `Steps`, `Ramp`, `Ladder`,
  `Frame`, `Slab`. Two pieces can share a shape but differ in kind.
- `size` (w, h, d), `color`, `steps` (step/rung count).

`PlacedStructure` (on `HomeStructure.structures`, serde-default so old homes load):
`type_id` + `pos (x,y,z)` + `rot_deg` + `pair` (teleporter link). Saves with the home.

## Geometry + rendering

`structure::structure_mesh(ty, pos, yaw)` yaw-rotates the local parametric mesh and
translates it. Pieces render through the SAME `material_walls` path as walls (grouped
by colour in `generate_meshes`). Winding is CCW-front / back-cull correct -- locked by
`every_triangle_winds_outward` (an adversarial review caught every box rendering
inside-out before this test existed; do not regress it).

## Helper gizmos (v0.587)

Every object type carries a passive HELPER widget drawn with the line primitive (shows
through walls), so the operator + an AI can see extents/topology at a glance:
- placed structures + machines: a wireframe bounds cube (selected structure glows amber);
- roads: amber node rings + cyan edge centerlines;
- conduits: a ring marker at each pipe-graph node (edges already render as solid pipes);
- lights: the diamond + RGB range sphere / spotlight cone (v0.572-582).
A master toggle ("Helper gizmos") quiets the passive bounds/range/node overlays when the
view is busy; the INTERACTIVE editing handles (corner orbs, opening slide/resize cubes,
the light diamond) are always shown so you never lose the ability to edit.

## Editor

- Footer "Structure" palette (leftmost, gated to the HomeStructure editor). Pick a piece
  to hold it; click the floor to drop; `[` / `]` rotate; right-click cancels. "Wall" is
  special -- it enters the wall-DRAW tool instead of placing.
- Each placed piece draws a wireframe BOUNDS gizmo (the line primitive); the selected one
  glows amber. Click a piece (ray-vs-AABB) to select + edit pose / teleporter pairing.
- A "Structures (N)" list mirrors the wall list; undo/redo covers placements.

## Behaviour (gameplay)

- WALKABLE footing (v0.584): `walk_surface(ty, pos, yaw, px, pz)` returns the standable
  height under the player -- the step under you on Stairs, the interpolated slope on a
  Ramp, the flat top of a Box/Slab platform. The first-person ground sampler raises the
  player's floor to the highest REACHABLE surface (a `STEP_UP` = 0.6 m cap stops a tall
  box yanking you up its side -- you use the stairs). So stairs/ramps/platforms are
  walkable. Frames + Ladders give no footing (you pass through / climb).
- TELEPORTERS (v0.584): stepping onto a teleporter whose `pair` is linked jumps you to
  the partner pad (a 1.2 s cooldown stops ping-ponging on arrival).
- MULTI-LEVEL (v0.588): a `Deck` piece (flat Slab) + a "Place at height" field drops a
  piece any height above the floor, so a deck becomes an upper-level landing at a
  staircase top. The footing sampler uses the player's ACTUAL height (`camera.y -
  eye_height`, v0.589), not the lagging rest floor -- so a deck at a stair/ladder top is
  reachable as you climb to it (the old lagging value rejected anything > 0.6 m up).
- ELEVATOR RIDE (v0.590): an elevator car is a MOVING floor. Runtime state lives in
  `EngineState.elevator_state` (keyed by structure index, not saved): `(anim 0..1, target,
  was_riding)`. Stepping onto the car toggles its destination (ride to the other end);
  waiting in the shaft at floor level recalls the car down to you; an idle car stays put.
  The footing sampler returns the car's animated height for anyone in the shaft footprint,
  so the rider is carried (ascending snaps up each frame; descending tracks via gravity --
  slightly hoppy but correct). A cached slab mesh renders the car at its live height. Place
  a deck at the top to step off. ELEVATOR_TRAVEL=3.0 m (one storey), rate ~1.5 m/s.
- LADDER CLIMB (v0.589): standing within a ladder's reach, holding Space climbs up (Shift
  down), gravity suspended, clamped to the ladder span; the climb only engages when the
  camera is already near the span (no teleport-snap), and a generous reach keeps a
  wall-flush ladder usable. Step off the top onto a deck (place the deck so it OVERLAPS the
  ladder top for a seamless dismount). `CameraController::climb_zone`, set per-frame from
  the structure pieces near the player.

## Material layering (v0.585)

Surfaces carry a stack of `SurfaceLayer { material, thickness_m }`, ordered top
(exposed) to bottom. "Rhino-lining on a truck bed" + "a road is asphalt over base over
subgrade." On a wall (`InteriorWall.layers`):
- `exposed_material()` = the top layer (else the bare wall material) and drives the
  rendered face colour, so a coated wall reads as its coating. A glass-clad wall (top
  layer alpha < 1) renders through the transparent pass by design -- the coating is what
  you see; collision still uses the structural `resolved_thickness()`.
- `total_thickness()` = the structural wall + every coat (shown in the editor's teach panel).
- Editor: the wall detail's "Surface layers" section (add on top / remove / reorder /
  per-layer thickness). Console: `add_layer <wall> <mat> <thickness>` / `rm_layer <wall> <n>`.

Road CLASSES are FIXED stacks in `data/blueprints/road_types.ron` (`structure::road_types()`):
footpath / residential / highway / runway, each a `SurfaceLayer` stack so "a runway has
different needs than a residential side road." A road piece / graph edge (v0.586) carries
one of these. The stacks reuse the `wall_materials.ron` ids so they teach the same
density / strength / cost.

## Deferred (next stages, intentionally NOT faked)

- **Elevator polish** (shipped functional in v0.590): a glassy *descent* (glue the rider to
  the car instead of gravity-hopping), a footing footprint that matches the visual slab
  exactly (today footing uses the full piece footprint, ~forgiving), and a distance CALL
  (today you board or wait in-shaft to summon it). All cosmetic/nice-to-have.
- **Auto-stacking placement**: placing a deck at an upper level uses the manual "Place at
  height" field; click-to-place-on-the-surface-under-the-cursor (raycast against piece
  tops) is a nicer follow-up. The height field is the robust control for now.
- **Roads as a graph (v0.586, shipped):** `HomeStructure.road_nodes` + `road_edges`. Each
  edge is a ribbon (reusing `wall_box`) between two nodes, coloured by its road CLASS's top
  layer. Editor: the left-panel "Roads" section (add node / drag x-z / wire an edge with a
  class + width). Console: `add_road_node` / `add_road` / `rm_road_node` / `rm_road`. A
  build-mode gizmo draws node rings + the curved edge centerlines (the line primitive).
- **Curved splines (v0.591):** each edge follows a Catmull-Rom curve (`road_edge_centerline`)
  whose off-curve control points come from each node's single other neighbour -- so a road
  BENDS smoothly through a degree-2 "through" node and stays STRAIGHT at junctions (3+ edges)
  and dead-ends (the control point mirrors the segment, degenerating Catmull-Rom to a line).
  generate_meshes ribbons consecutive curve samples (8 per edge) with `wall_box`. Add
  intermediate nodes for tighter control; very sharp through-angles can overshoot slightly.
  Still pending: the **rail line** between train platforms, and road FOOTING (walk/drive on
  the surface -- marginal on a flat floor, matters once roads sit at varied heights).
- **Solid-body collision for tall pieces**: structures aren't wall-colliders yet, so you can
  walk into the side of a tall solid box. The default pieces are short (platform 0.4 m) so
  this isn't visible; add structure colliders when a tall solid piece ships.
