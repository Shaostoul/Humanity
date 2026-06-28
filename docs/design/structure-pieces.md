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

- **Elevator ride + ladder climb** need an animated/moving structure state (a moving
  floor the player rides, or a climb mode that overrides gravity) + a destination floor.
  That is its own increment ("moving structure state") -- a fake that drops you in mid-air
  would be a bandaid. Until then an elevator/teleporter Frame is a visible, placeable arch.
- **Multi-level landings**: stairs currently climb to their top step; standing past the top
  falls back to the room floor until an upper platform/floor exists there. Real upper
  storeys (a second room-floor plane the stairs connect) are a home-design follow-up.
- **Trains + roads as networks**: a train platform / road slab is placeable now; the rail
  line + road graph (node+spline, per-class material stack) is v0.585-586 (see
  `conduits-node-graph.md` for the reusable node-graph pattern, and the material-layering
  design).
- **Solid-body collision for tall pieces**: structures aren't wall-colliders yet, so you can
  walk into the side of a tall solid box. The default pieces are short (platform 0.4 m) so
  this isn't visible; add structure colliders when a tall solid piece ships.
