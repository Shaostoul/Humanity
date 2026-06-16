# Construction Architecture (next-generation)

> Status: DESIGN (v0.456, 2026-06-15). Operator direction: the construction system must support
> NON-GRID / organic shapes (alien ships), GENERATE realistic structure (studs/joists/sheathing)
> from a simple authored boundary, test REAL physics loads, and be DESTRUCTIBLE (blow a hole in a
> wall, with consequences like decompression). Researched across voxel/grid, SDF/organic,
> procedural-framing, structural-physics, and hybrid/room approaches; this is the synthesis.
> Built so far: Step 1 (the room-placement editor, v0.459 - per-wall kinds + position/size +
> add/remove rooms). See the staged plan in section (e); Steps 2+ (framing, structural physics,
> destruction, non-grid) are designed but not yet built.

## Decision in one sentence

Build a **five-layer pipeline** where the player edits a thin **Boundary layer** (a bounding-box room today, a planar wall-graph / signed-distance hull later), a deterministic **generator** turns each boundary into a **node-beam framing graph** of real members (the "realistic structure"), a cheap event-driven **structural solver** runs over that graph for load-bearing + destruction, and **Rooms stay a derived semantic view** for gameplay. The current `HomesteadLayout` + `WallKind` editor is **slice 1 of this same pipeline**, not a throwaway  -  we generalize the boundary representation without touching the layers above it.

This is the convergent answer across all five surveys: survey 5's three-layer model (semantic / geometric / proxy), survey 3's boundary→split-grammar→members generator, survey 4's node-beam graph as the canonical structural data model, and survey 1/4's flood-fill island detection for destruction. We adopt the boundary representation from survey 2 (SDF) **only at the outer hull layer and only later**, never as the simulation foundation  -  that satisfies requirement 1 (organic) without the per-voxel cost trap every survey flagged.

---

## (a) The layered data model

Dependency is **one-directional and downward**: the player edits the Boundary; everything below is **derived, cached, and lazily rebuilt** on a dirty mark. Nothing below the Boundary is ever authored or saved as primary data (it regenerates).

### Layer 0  -  Semantic / Room layer (the gameplay unit)
**Holds:** `Room { id, type, access, lights, sealed: bool, EnclosedSpace handle, [E] actions, owner, bounding-or-volume reference }`. This is *exactly today's* `RoomInfo` + `RoomTypeRegistry` + `EnclosedSpace`, untouched.
**Connects to:** the Boundary layer by a back-reference (which boundary faces enclose this room). It is a **derived view**  -  survey 5's central insight. Today `RoomInfo` is produced as a side-output of `build_meshes`; we keep that, but make room *detection* a named function so the only thing that changes when geometry goes organic is that one bridge.

### Layer 1  -  Boundary / Surface layer (THE authority, what the player edits + what gets destroyed)
**Holds:** the high-level shape spec. This is the layer whose *representation* must generalize. The plan:

- **NOW (slice 1):** today's `RoomConfig` (an AABB: position + dimensions) plus `WallSet`/`WallKind`. Already shipped.
- **NEXT:** promote each wall to an explicit **oriented wall segment**  -  a `WallSpec { baseline: (Vec2 start, Vec2 end), height, thickness, kind, openings: [...] }`. A room's four AABB walls become four WallSpecs; this is a lossless re-encoding of what we already have, but a WallSpec is no longer required to be axis-aligned.
- **LATER (organic):** the set of WallSpecs is a **planar straight-line graph (a DCEL / half-edge structure)**: vertices + wall edges, where bounded faces are floors/rooms. Curved walls = polyline-approximated edges (survey 3's faceted-polyline + curved-glulam-rib model). For genuinely sculpted *outer shells* (alien hulls, earthworks) add a parallel **SDF hull sub-model** (survey 2) that is render+collision only and is NOT where framing or the structural graph live.

**Why this representation (decisive, for a mid-range PC in Rust):**
> **The boundary is a B-rep-flavored planar wall graph (DCEL), NOT a voxel/SDF field and NOT polygon soup.** Justification:
> - A **fixed voxel lattice is rejected outright** (requirement 1). Every survey-1 game proves the lattice can only *approximate* angled lumber as cube clusters, and survey 2/4 both document that per-voxel structural integrity scales factorially (Teardown's own devs)  -  fatal for a billions-of-users mandate.
> - **Polygon soup is rejected** because it has no topology  -  you cannot ask "which rooms does this wall bound" or "is this volume sealed," which are the exact queries Layer 0 needs.
> - A **planar wall graph** keeps walls as *segments at any angle* (organic-capable), gives O(1) face/adjacency queries for room detection (survey 5's DCEL minimal-cycle = rooms), serializes to a tiny RON, regenerates deterministically, and the engine **already computes wall adjacency this way** (`find_shared_edges` in `fibonacci.rs` is a primitive shared-edge finder  -  the DCEL is its principled generalization).
> - The **SDF stays at the hull layer only** (survey 2's "best-in-class organic, but no native load-bearing" finding) so we get sculptable organic shells without paying the SDF tax on the structural sim. The hull SDF feeds rendering (dual contouring for sharp where needed) and rapier collision (evaluate field, push along gradient); the *framing* is still discrete members.

### Layer 2  -  Generated-structure layer (the "realistic blueprint")
**Holds:** `Vec<FramingMember>` where `FramingMember { member_type: StudBottomPlate|TopPlate|DoubleTopPlate|Stud|EndStud|KingStud|JackStud|Header|Sill|Cripple|Blocking|Joist|Rafter|SheathingPanel|FinishPanel, node_a, node_b, section: (w,h), material_id, layer_index, parent_wall_id }`, plus the **node list** `FramingNode { pos: Vec3, mass, anchored: bool }` that the members connect (survey 4's node-beam graph). Materials come from the existing `data/blueprints/materials.ron` (steel/wood/composite with real `tensile/compressive/yield_strength_mpa`, `density`, `modulus_elasticity`)  -  that table already exists and is exactly the infinite-of-X store this layer consumes.
**Connects to:** generated deterministically from Layer 1 by a pure function (section (b)). **NOT serialized**  -  regenerated on load from the boundary + a `FramingRuleset` id, so saves stay tiny (survey 3's save-the-spec rule).

### Layer 3  -  Physics-proxy layer (loads, failure, collision)
**Holds:** the **structural support graph** (nodes + members from Layer 2, with `current_load`, `support_path_to_anchor`), and the **rapier3d collision/rigid-body proxies** (already in the stack). Anchored nodes = foundation/ground contact.
**Connects to:** built directly on the Layer-2 graph (node-beam *is* the proxy  -  no separate data model). Destruction mutates Layer 1, which dirties Layer 2, which re-solves Layer 3. Falling debris = rapier rigid bodies spawned per disconnected island.

### Layer 4  -  Render layer
**Holds:** instanced meshes. Members render as **instanced unit boxes** (survey 3/Houdini copy-to-points idiom; thousands of studs are cheap on the existing wgpu instanced pipeline). Sheathing/finish/glass/mirror panels and trim render as today's `HomesteadMeshes` families (walls/trim/windows/mirrors/ceilings/floors). The organic hull (when present) is dual-contoured from the SDF.
**Connects to:** generated from Layers 1+2; the existing `apply_homestead_meshes` upload path is reused.

**The loop (survey 5's RimWorld incremental pipeline, mapped to our code):** player edit OR blast → mutate Layer 1 → mark affected walls/region **dirty** → regenerate Layer 2 framing for dirty walls only → re-solve Layer 3 sub-graph → re-detect affected Rooms (re-seal/split/merge) → re-mesh dirty Layer 4 chunks. Today's `construction_dirty` → `rebuild_homestead()` flag IS this loop at coarse (whole-home) granularity; we make it incremental later.

---

## (b) Room boundary → generated framing (the "realistic structure" pass)

A **pure deterministic function** `generate_framing(wall: &WallSpec, rules: &FramingRuleset) -> (Vec<FramingNode>, Vec<FramingMember>)`, working in the wall's local 2D frame (U = length, V = height, W = thickness). This is survey 3's CityEngine-CGA / AGACAD pipeline, which is the proven path and the only one that yields real, buildable blueprints:

1. **Plates:** bottom plate at V=0 full span; top plate(s) at the head, doubled per `plate_config`.
2. **End studs** at U=0 and U=len.
3. **Stud field  -  the floating-repeat operator** (survey 3's core trick): place a stud every `stud_spacing` (16in/24in/600mm from the ruleset); count = `floor(span/spacing)`, last bay floats. This is what makes spacing correct on *any* wall length, including non-axis-aligned ones.
4. **Openings sub-grammar** (per `WallKind::Door`/`Window` opening, which we already have as `(center_t, width, height, sill)` tuples in `build_meshes`): delete field studs inside the rough opening; emit king studs (flanking, full height), jack/trimmer studs (inboard, to header), a **header sized by span** (ruleset lookup  -  survey 3 pitfall: a 600 mm and a 3 m opening need different headers), a sill (windows), cripples above header + below sill.
5. **Blocking/nogging** rows per ruleset.
6. **Layer stack** (survey 3 + survey 5 BIM multilayer): structural frame is layer 0; **sheathing** (greedy 4×8-ft / 1.22×2.44 m panel tiling with opening cutouts), WRB, siding, drywall are each generated by their own rule against the *same* boundary, offset in W. Each layer independently toggle-able and independently destructible.

**Determinism is mandatory** (survey 3 pitfall + this repo's KAT discipline): no `HashMap` iteration order in the generator, store **actual** lumber dims (2×4 = 38×89 mm) not nominal, so regenerate-on-load is byte-stable and saves never "shift."

**The `csg.rs` stub becomes real here only for the SDF hull** (organic phase): the existing `CsgBrush { op: Union|Subtract|Intersect }` is exactly survey 2's "build and destroy are one boolean op"  -  a blast is `max(-brush, field)`. Floor/ceiling roof toggles already exist; framing for floors (joists) and roofs (rafters) reuses the same generator with different member types.

---

## (c) Structural loads, failure/destruction, and the sealed-air tie-in

**Solver choice (decisive):** a **quasi-static support-propagation + cheap relaxation pass**, event-driven, NOT a continuous high-rate dynamic sim.
- Survey 4 is explicit: BeamNG's 2 kHz mass-spring is the wrong tool for a static building; survey 1's "castles don't move" lesson says run the expensive solve only on **static** structures, **on edit or on damage**, never per-frame.
- **Steady state:** propagate `support` from anchored (foundation/ground-contact) nodes through the member graph, accumulating tributary mass + a moment term, attenuating with distance (survey 1's Medieval-Engineers model, but on *real members*, not cubes). For each member compute `utilization = |load| / material_limit` using the real `tensile/compressive/yield_strength_mpa` already in `materials.ron`. This populates the **red/green stress overlay** (survey 1+4's single most-praised feature; also AI-discoverable structural state). This is what `structural.rs`'s `StructuralResult { Stable|Unstable|Collapsed }` enum is for  -  currently a stub; this fills it in.
- **Optional refinement:** a Position-Based-Dynamics distance-constraint pass for visible sag/deflection while dragging (survey 4  -  PBD is unconditionally stable at low iteration count, dodging the mass-spring stiffness/timestep explosion).

**Failure (two-stage, survey 4 JBeam):** `utilization > deform_threshold` → permanent yield (reduce stiffness, render bent  -  "creaks and bends"); `utilization > break_threshold` → delete member ("snaps"). On any deletion, **re-solve the affected sub-graph only** (bounded iteration count to prevent infinite same-frame cascade  -  survey 4 pitfall); load redistributes to neighbors which may then fail → emergent progressive collapse, **no scripted sequences**.

**Destruction / blow a hole (requirement 4):**
1. Delete members/nodes inside the blast radius (apply impulse falloff to surviving nearby nodes).
2. Re-solve → overstressed survivors cascade-fail.
3. **Flood-fill from all anchored nodes** over the surviving graph (survey 1 Teardown + survey 4 Chaos connection-graph  -  the single highest-value steal). Any connected component **not** reachable from an anchor is an island → spawn **one rapier rigid body per island** with recomputed mass/inertia → it falls. Cap/merge debris islands (survey 4 pitfall: a large collapse spawning thousands of bodies tanks the frame). Because this is **graph-based, it needs no grid**  -  works on the node-beam graph directly, satisfying requirement 1.

**Sealed-air survival tie-in (requirements 4 + 5):** the hole is a mutation of Layer 1, which **dirties room detection**. Room re-detection (section d) re-evaluates `sealed` on the affected `Room`. The existing `EnclosedSpace`/`AtmosphereSystem` already models this end-to-end:
- A breach flips the room's `EnclosedSpace.sealed = false` and raises `leak_rate`. In space (outside `homestead_bounds` is vacuum, per `load_world`), `AtmosphereSystem::equalize` then pulls pressure toward `Atmosphere::vacuum()`, and the existing `DECOMPRESSION_DAMAGE` path (pressure < 0.1 atm → 50 dmg/s) fires. **This requires zero new survival code**  -  only wiring "breach detected → set that room's `EnclosedSpace` unsealed."
- Two adjacent breached rooms **merge** their enclosed volumes (gas equalizes between them)  -  survey 5's room-merge-on-hole, implemented by re-detection joining the two faces.
- **Volumetric/roof-aware sealing** (survey 5 pitfall): sealing must account for floor/ceiling/roof, not just walls-in-plan, or an open-topped room reads as sealed. Today's `homestead_bounds` is a single AABB; the per-room `EnclosedSpace` + roof toggle give us the hook to make this per-room and roof-aware.

---

## (d) How rooms-as-gameplay survive (requirement 5)

**Rooms are a derived view, never the authority** (survey 5's load-bearing rule). The bridge is **one function, `detect_rooms(boundary) -> Vec<Room>`**:
- **NOW:** rooms come straight from `RoomConfig` (1 config = 1 room), as today. `build_meshes` already emits `RoomInfo`.
- **ORGANIC:** `detect_rooms` becomes **DCEL minimal-cycle extraction**  -  bounded faces of the planar wall graph are rooms; doors are special half-edges that keep faces *separate-but-connected* (survey 5 pitfall: a doored room must stay sealed-but-passable, RimWorld gives doors their own region). Open/Window/Solid `WallKind`s already encode passability; this maps directly.
- Everything that *consumes* a Room  -  `RoomTypeRegistry` (type/purpose/[E] actions/access), `room_lights`, `EnclosedSpace`, the showroom/spawn/hologram room lookups in `load_world`, `gui_state.room_bounds`, the pipe router's room AABBs  -  is **unchanged**, because they all read `RoomInfo`/`Room`, not geometry. That is the whole payoff of the derived-view discipline: going organic rewrites *only* `detect_rooms`.

Type assignment stays explicit/player-set (or auto-suggested from contents); we never lose the gameplay meaning. Survey 5's `IfcRelSpaceBoundary` idea  -  store "this room is bounded by these wall faces"  -  is how a destroyed face knows *which* rooms to re-evaluate (dirty-set seeding).

---

## (e) Staged implementation plan (each step ships something usable)

**Step 1  -  Room placement editor. SHIPPED (v0.459).**
The construction editor (B in-world) now edits, per room: the four wall kinds (v0.455), a Pin
toggle + X/Z position (explicit position overrides the Fibonacci spiral; un-pinned rooms still
auto-place around the pinned ones), width + depth, Delete room, and an Add-Room picker (types
from the registry). Edits flow through `gui_state.construction_rooms: Vec<ConstructionRoom>`
-> the ID-keyed reconcile in `lib.rs` (retain / upsert / reorder, pin seeded from the room's
resolved position so Pin freezes-in-place) -> `rebuild_homestead` -> Save writes the RON. So
the operator authors the floor plan directly instead of fighting the auto-layout.
- Deferred to later slices: mouse-DRAG placement (DragValue only for now); non-grid / angled
  walls (the `WallSpec` oriented-segment re-encoding); per-room height (the global ceiling
  slider still owns Y); and live machine/pipe/hologram/spawn refresh on an in-game move
  (`rebuild_homestead` refreshes meshes + lights + bounds + HUD `room_bounds`; machines + pipes
  + spawn still resolve at `load_world`, so they refresh on the next relaunch).

**Step 2  -  Framing generator (Layer 2), wall-by-wall.**
Implement `generate_framing(WallSpec, FramingRuleset)` + a `data/blueprints/framing_rules.ron` (stud spacing, sections, plate config, opening sub-grammar, layer stack). Render members as instanced boxes behind a "show framing" toggle. **Ships:** real studs/plates/headers  -  the "realistic blueprint"  -  visible in-app, plus a generated bill of materials. Rooms/atmosphere untouched.

**Step 3  -  Structural solver + overlay (Layer 3, static).**
Fill in `structural.rs`: support propagation from anchored nodes, per-member utilization from `materials.ron`, red/green N-key overlay. Event-driven (on edit). **Ships:** "this wall is overloaded" feedback before you build  -  load-bearing made legible (requirement 3).

**Step 4  -  Destruction + debris + decompression.**
Blast = delete members → re-solve → flood-fill islands → rapier debris; wire "breach → `EnclosedSpace.sealed=false`." **Ships:** blow a hole, the floor above drops, the room decompresses (requirement 4)  -  reuses the existing atmosphere sim.

**Step 5  -  Non-grid boundary (DCEL) + organic rooms.**
Replace the AABB room store with a planar wall graph; `detect_rooms` → DCEL faces; allow angled/polyline walls in the editor. **Ships:** non-orthogonal and faceted-organic homesteads (requirement 1) with framing + physics + sealing all still working, because they sit on layers that never assumed a grid.

**Step 6  -  SDF hull for sculpted shells (optional, organic ceiling).**
Add the SDF hull sub-model (the now-real `csg.rs`) + dual-contour render + rapier collision for genuinely blobby alien hulls, with framing as curved glulam ribs along the hull. **Ships:** the alien-ship vision, without ever putting the *simulation* on a voxel lattice.

---

## (f) Honest risks + what to prototype FIRST to de-risk

**The two unproven claims are "organic" and "real structural physics." De-risk them before committing the full pipeline:**

1. **PROTOTYPE FIRST  -  the node-beam solver + flood-fill on a hand-built framed wall+floor (a weekend spike, pure Rust, no renderer).** Author ~200 members for one room with a floor above, run support propagation, delete a king stud, confirm the header + floor cascade-fail and the disconnected island is detected. This validates survey 4's claim cheaply and tells us if the support-graph fidelity is "believable" before we wire it to geometry. **Risk it retires:** structural-integrity passes are notoriously fragile and counter-intuitive (survey 1: Empyrion green-builds collapse when you add stairs); we need to feel the tuning burden early.

2. **PROTOTYPE SECOND  -  the framing generator on one non-axis-aligned wall.** Generate studs on a wall at 30° with one window; confirm the floating-repeat spacing and rough-opening sub-grammar are correct off-axis. **Risk it retires:** survey 3's honest caveat that stud-realism presumes straight prismatic lumber  -  confirm "organic = faceted polylines + curved ribs" is the fidelity contract we accept, *before* promising blobs.

**Standing risks to budget for (all flagged by the surveys):**
- **Corner/T-junction resolution** (survey 3's "hidden hard part"): two walls at a corner double-up plates/studs. Needs an explicit corner rule (3-stud vs California). `find_shared_edges` already does primitive corner detection  -  extend, don't restart.
- **DCEL robustness** (survey 5): open doorways, walls that don't quite meet, duplicate vertices break face extraction. Budget a snapping/tolerance pass + a "leak detection" pass (a room that flood-fills to exterior is *unsealed*, not a bug).
- **Incremental rebuild from day one** (survey 5): full re-detect/re-solve/re-mesh on every edit will hitch a large base. Today's whole-home `rebuild_homestead` is fine at 9 rooms but must go dirty-region before settlements.
- **Determinism** (survey 3 + repo KAT discipline): byte-stable generator or saves drift.
- **Debris cap** (survey 4): merge small islands, cap rigid-body count, settle/despawn debris (billions-of-users mandate).
- **Relay-build gate** (repo gotcha): everything touching GUI/render/rapier gets `#[cfg(feature = "native")]`. The framing generator, ruleset parsing, and the support-graph math are **pure geometry/data (glam + serde only)**  -  keep them feature-agnostic like `routing.rs` is, so they compile under `relay` and stay unit-testable without linking the bin (dodges the Windows `LNK1318` PDB limit).

---

## (g) Editor UX + multistory (operator direction, 2026-06-15)

The number-field side panel (v0.459-462) is functional but NOT intuitive: editing one room re-shuffled the others (fixed v0.462 by pinning all rooms on open), there is no spatial way to see or move rooms, and doors are always centred. The operator's requirements:
- SEE + MOVE rooms spatially, not via number fields. "I have no easy way of seeing or interacting with or moving the rooms around."
- Doors NOT always centred (positionable along a wall).
- MULTISTORY is a hard requirement: a player home can be several stories; a shopping mall is multiple stories but "technically a giant room aboard the spaceship"; possibly many malls.

### The editor: a top-down floor-plan canvas (the intuitive tool)
- An egui 2D canvas that shows the CURRENT LEVEL from above (a blueprint). Each room is a rectangle: drag the body to move (snap to a grid), drag edge handles to resize. Click a wall to set its kind; a door shows as a mark on the wall you DRAG to position (non-centred). The 3D home rebuilds live as you edit the plan.
- A LEVEL selector (floor up / down, add / remove level). You edit one level at a time; the home stacks in Y.
- The existing per-wall + numeric side panel stays as the fine-tuning surface; the canvas becomes the PRIMARY one.
- This is The-Sims / Sweet-Home-3D / blueprint-editor idiom -- the proven intuitive model for floor plans.

### Multistory data model (two distinct kinds, both needed)
A. STACKED ROOMS -- separate rooms on different floors (a house: floor 1 + floor 2). Each `RoomConfig` already has `position.y` + height; add a `level: i32` so the editor can group / filter rooms per floor (y derives from `level * story_height`, or stays explicit). The level selector shows + edits one floor's rooms.
B. MULTI-LEVEL ROOM (the mall) -- ONE tall room volume with internal floor SLABS at heights (mezzanines), linked by stairs / escalators. Add `floor_levels: Vec<f32>` to a room (slab heights inside its own volume). This is one room spanning levels, NOT stacked rooms; it is the mall / atrium case.
Both fit the layered model: per-level wall graphs at the boundary layer, and floor slabs become joists in the generated-structure layer. Vertical circulation (stairs / ladders / elevators / escalators) is a new placeable piece type.

### Non-centred doors
A door opening gains a position-along-the-wall parameter (0..1 of the span, or metres from a corner). The current centre-on-the-shared-edge becomes the DEFAULT, overridden by dragging the door in the canvas. This lives on the boundary `WallSpec` (`openings: [{offset, width, height, sill, kind}]`).

### Build order for this UX layer
1. Top-down floor-plan canvas: move + resize rooms in 2D, live rebuild. The intuitive editor. **SHIPPED v0.463 (canvas) + v0.466 (3D room grab) + v0.467 (3-column layout).**
2. Level selector + STACKED-room multistory (place rooms on floors; the home stacks in Y). **NEXT (the big multistory unlock).**
3. Openings as PLACED OBJECTS (add a door/window/airlock to a spot on a still-solid wall; move + resize). **SHIPPED v0.469** (superseded the v0.468 wall-kind slide gizmo).
4. MULTI-LEVEL rooms (internal floor slabs + vertical circulation) -- the mall.
This sits ON TOP of the existing layout data (no data thrown away); the number panel stays for numeric fine-tuning.

### Openings as placed objects (SHIPPED v0.469 -- supersedes the v0.468 slide gizmo)
The v0.468 model (a whole wall IS a `Door`/`Window`, cut once at a slide offset) was the wrong mental model: it conflated wall presence with connection, doors were always centred, you could not add a second opening or resize one, and the slide value decoupled from reality (the panel said 20 m on a 2 m wall). v0.469 makes an opening an **additive object placed on an otherwise-solid wall**:
- **Data:** `OpeningKind { Door, Window, Airlock, Hatch }` + `Opening { wall:u8, kind, u, v, w, h, profile:Option<String> }` and `RoomConfig.openings: Vec<Opening>` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so the shipped RON is byte-identical and all prior round-trip tests stay green). `u` = centre along the wall, `v` = centre height up the wall (floor-snapped kinds force the base to the floor), `w`/`h` = size. `profile` is the reserved Stage-2 SVG-cutout hook.
- **Cutting:** `build_meshes` appends each room's `openings` (filtered by wall) to the SAME per-opening tuple list it already used, OUTSIDE the `WallKind` match -- so a plain Solid wall is cut where an object sits and stays solid everywhere else, multiple openings per wall already work (`wall_with_openings` loops the spans), and the free centre height `v` maps to the `sill = v - h/2` the existing glass/casing geometry speaks. A per-opening `glazed` flag lets a wall mix a door + a window.
- **Gizmo (3D):** `fibonacci::opening_handles` returns a move handle (and, for placed non-floor openings, four edge resize handles) per opening of the SELECTED room, all on the wall's VERTICAL plane. `lib.rs` hit-tests the wall-FACE plane (ray vs plane through `wall_start` with the horizontal normal `n = u_hat x Y`; decompose the hit into `u = (hit-start).dot(u_hat)`, `v = hit.y - start.y`), so a left-drag MOVES (door: u + floor-snap; window: u and v) and an edge-handle drag RESIZES, every result clamped to the wall. Move cube = accent, resize cubes = warning, both float proud of the wall toward the camera.
- **Editor (numeric):** the right pane lists each opening (kind + wall combos, Along/Up/W/H DragValues whose `.range()` is the LIVE wall length + ceiling, a Remove) plus Add Door/Window/Airlock buttons. Because the ranges are bound to the real wall, the value can never exceed it -- the 20m-vs-2m decouple is gone. A per-frame re-clamp keeps every opening inside its wall after a room resize.
- **Back-compat + migration:** the legacy `WallKind::Door/Window` faces (the shipped bedroom windows) still render and still slide (handles with `opening_index: None` write `WallSet.offsets`), so `opening_handles_track_wall_offset` passes byte-for-byte. `fibonacci::promote_walls_to_openings(&mut layout)` (editor-button only, never the loader) converts a legacy face into an `Opening` + demotes the face to Solid for operators who want to freely move/resize it.
- **Garden-wall regen bug:** root cause was adjacency-dependent wall EXISTENCE -- a `WallKind::Open` consulted no adjacency (a permanent gap) and the `neighbor_owns` ownership-transfer flickered in `find_shared_edges`' tolerance dead-band, so a wall could read as missing after a neighbour moved. Fixed structurally: `Open` self-heals to `Solid` when no passable neighbour faces it, and `neighbor_owns` never suppresses a wall that carries its own placed opening. (`open_wall_reseals_without_neighbor` locks it.)

Locked by nine v0.469 tests (cut-on-solid, free-height glass, two-per-wall, u+v clamp, door floor-snap, openings round-trip, empty-not-serialized, legacy promotion, open-reseal) plus the retained v0.468 + round-trip tests; 437 lib tests green.

**Stage 2 (deferred, hooks reserved):** SVG cutout silhouettes (`Opening.profile` -> a `cutout_profiles.ron` of normalized closed polygons, reusing the trim `sweep_profile` engine) for arched/porthole windows that are still a square WxH box, and the single curved-surface seam (an `ApertureSurface` `box_point(u,v)`/`box_normal(u,v)` so the SAME opening bends onto a curved hull while reading as a flat square aperture). Stage 1 threads the box through the existing tuple so Stage 2 swaps the cutter + adds the data file with no editor/gizmo changes.

---

**Relevant files (all absolute):**
- Boundary + generator + room detection live in `C:\Humanity\src\ship\fibonacci.rs` (`HomesteadLayout`, `RoomConfig`, `WallKind`/`WallSet`, `build_meshes`, `find_shared_edges`, `RoomInfo`).
- Construction system + stubs to flesh out: `C:\Humanity\src\systems\construction\mod.rs`, `...\structural.rs` (`StructuralResult`), `...\csg.rs` (`CsgBrush`), `...\routing.rs` (the pattern to copy  -  pure, feature-agnostic, data-driven, tested).
- Editor GUI surface: `C:\Humanity\src\gui\pages\construction.rs` (add room position/size + add/remove here for Step 1).
- Live rebuild loop + room→gameplay wiring: `C:\Humanity\src\lib.rs` (`rebuild_homestead`, `load_world`, `construction_dirty`, `room_lights`, `homestead_bounds`).
- Sealed-air sim to reuse for destruction consequences: `C:\Humanity\src\systems\atmosphere.rs` (`EnclosedSpace`, `AtmosphereSystem`, `DECOMPRESSION_DAMAGE`).
- Material properties (Layer 2's infinite-of-X store): `C:\Humanity\data\blueprints\materials.ron` (real `tensile/compressive/yield_strength_mpa`, `density`, `modulus_elasticity`).
- Room semantics: `C:\Humanity\src\ship\room_types.rs` (`RoomTypeRegistry`).
- New ruleset data file to add at Step 2: `C:\Humanity\data\blueprints\framing_rules.ron`.
- Doc destination: `C:\Humanity\docs\design\construction-architecture.md`.

**Load-bearing code facts that shaped the recommendation (so the doc author can trust them):**
- The editor already mutates `gui_state.construction_rooms` and sets `construction_dirty`; `lib.rs::rebuild_homestead` regenerates meshes + refreshes `room_lights` and `homestead_bounds` from `RoomInfo`  -  this is already the survey-5 derived-view loop at coarse granularity.
- `find_shared_edges` (fibonacci.rs:616) is a working shared-wall/adjacency detector returning overlap segments  -  the DCEL is its principled generalization, not a from-scratch build.
- `RoomInfo` is the single struct every gameplay consumer reads (lights, sealed bounds, pipe-router AABBs, room_bounds, showroom/spawn/hologram lookups), which is precisely why "rooms = derived view" is cheap to preserve.
- `EnclosedSpace`/`AtmosphereSystem` already implement sealed/unsealed equalization + vacuum decompression damage; destruction only needs to flip `sealed`.
