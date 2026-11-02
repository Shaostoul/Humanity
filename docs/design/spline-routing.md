# Spline routing: pipes, hoses and cables laid along surfaces, to code, shown in the AR layer

**Status: the complete plan, 2026-11-01. Rung 0 (rules and materials as data) is BUILT, 2026-11-02;
the rest is not.** It is written whole, every
part of the feature included, so that the data shapes, the per-frame budget and the scale are
decided once and each rung below is a slice of the real thing (CLAUDE.md, "Plan the whole feature,
build it in rungs, keep performance in mind throughout"). The numbers behind the "to code" rules
are in two dated findings of the same day:
[pipe and cable routing rules](../reference/findings/2026-11-01-pipe-and-cable-routing-rules.md)
(cited below as R1 to R53, its finding numbers; R45 to R53 added by the 2026-11-02 refutation
pass) and
[manual door releases](../reference/findings/2026-11-01-manual-door-releases.md) (D1 to D19).

Read with: [utility-wiring.md](utility-wiring.md) (ports, cable physics), [conduits-node-graph.md](conduits-node-graph.md)
(the junction-node graph and its unbuilt stages), [grid-hierarchy.md](grid-hierarchy.md) (home to
fleet), [line-overlays.md](line-overlays.md) (the world-space line primitive), [locks.md](locks.md)
(door locks and their open stages), [cosmos-architecture.md](cosmos-architecture.md) section
17a-sexies (the 2026-05-12 AR-glasses plan), [detection-sensing.md](detection-sensing.md),
[crate-split.md](crate-split.md), [infinite-of-x.md](infinite-of-x.md).

## 1. The operator's words

2026-11-01, after seeing a post of someone's spline mesh deformer whose splines "automatically
conform to any ground mesh or object surface" (the post is inspiration only; nothing of it is
copied):

> Seems like something we could incorporate into HOS for cables and whatever else. We want to make
> the process of laying pipes, cables, and whatever else as easy as possible. I like the idea of
> using splines for things with certain "to code" things. Like right now all our cables and pipes
> are just straight tubes and 90 degree corners. With splines we could have curves. I figure this
> simplified version of interacting with machines, pipes, cables, etc. would be a lot easier for
> people to digest. We could say it's augmented reality. What we design could literally be used to
> AR whenever I can afford AR glasses. This is how we can justify the constellation lines we added
> to the skybox and real star catalog. I guess we would say it's all part of the HUD. Do we have a
> crate or HUD.rs or whatever? If it's part of something already, what's the hud a part of?

The same day, his direction for this plan:

> We should go with what people would experience in the real world. I guess that means we should
> add a way of changing the material of the pipe/cable/tube/whatever. The spline/line/edge/vertex
> whatever should be easy to move. I don't want to suffer from fixed voxel placements. I want to
> see curvy cables/hoses and rigid pipes that area actually doing things instead of just cosmetic.
> It'd be cool to some how justify a player tapping into infrastructure elements to do things. Like
> boarding an alien craft, locating their data cables, and then trying to hack in. Like if there's a
> door and it has emergency panels to remove to manually open powered doors. Players should be able
> to open that panel. We also want to make adding 'features' to pipes/cables/hoses. Like we have a
> vertex along the spline we can move to add a water sprinkler for fires or watering plants or
> whatever.

And, on snapping to surfaces:

> The we could add the ability to add anchors or remove anchors to keep cables/hoses/whatever
> anchored with like those ramps so they can be driven over safely with heavy machinery/vehicals.

Earlier words this plan keeps, as recorded in the code (`src/ship/conduits.rs`, v0.535): "the
straight lines that pass through everything for the pipes is wrong. We want realistic routed
pipes/conduits ... copper pipes for all potable water ... rigid tubes ... flexible tubes are hoses
and power extension cords ... automatically procedurally draw their support structures, mounting
brackets and passthrough gaskets", and, of potable water, "no PVC or other synthetics that leach
toxins". From the 2026-05-12 AR plan (cosmos-architecture.md 17a-sexies): "FPS AR-glasses style
overlays, faintly see orbital paths while in FPS mode, with asteroid trajectories highlighted (no
Blender-style controls, just there visually)."

What his direction asks of the plan, point by point, and where each is answered:

| His point | Where |
|---|---|
| Real world first; realistic behaviour is the default | 6, 7.5 |
| The material of any run changes in place, with one picker | 5.3, 15 |
| Points move freely, no grid or voxel placement forced; snapping helps, never cages | 8.2 |
| Curvy hoses and cables, rigid pipe that behaves like rigid pipe | 6 |
| Functional, not cosmetic: length, material, size and route feed the simulation | 10 |
| Features along a run, slid along it, each a real node | 11 |
| Anchors added and removed, ramps that vehicles drive over | 12 |
| Tapping into infrastructure: finding runs, opening panels, emergency door releases, hacking | 13 |
| "Do we have a HUD.rs, and what is it part of?" | 4 |
| The AR layer that justifies the constellation lines and leads to real glasses | 16 |

## 2. In one paragraph

A **run** (one pipe, hose, tube or cable, the trades' own word) is saved as its ends, a short list
of **control points** that sit on the surfaces they were placed on, the **anchors** that hold it,
the **features** along it, and its **material and size**, nothing more. Everything else is
computed: the curve (lines and arcs for rigid and bendable material, a settled flexible rod for
hose and cable), the fittings, the supports, the code checks, the mesh, and the run's place in the
power, water, data or air network, where its real length, size and material decide what reaches
the far end. Rules and materials are data rows with a source each, grouped into rule profiles
(a home, a ship, a spacecraft). The person edits a run in the **AR layer**, the one owner of every
world-anchored overlay in the game (route handles and previews, port gizmos, labels, markers,
orbit rings, constellation figures), built so that a pair of AR glasses is a different camera and
compositor rather than a rewrite.

## 3. What exists today, and what this plan does with it

Read before designing (all checked 2026-11-01 on main at `0d9526099`):

| What | Where | This plan |
|---|---|---|
| Routing: up from each end to a per-storey service height, across in Manhattan legs, down; risers through slabs; ends kept under the real ceiling | `src/engine/home_meshes.rs` `route_run`, `run_site`; `src/ship/conduits.rs` `route_conduit`, `add_slab_passthroughs` | Replaced by the auto-router (8.4), which keeps the rule that a rigid run never cuts diagonally through a room (R26 now gives it a source) |
| The drawn pipes: one cached unit cylinder per straight leg, hangers and wall gaskets deduped by position, the selected machine's flow beads | `rebuild_connection_objects` (about line 2668), `connection_objects`, `connection_flow_paths` | Replaced by swept meshes merged per cell (9); the dedup idea survives as shared anchors (12) |
| `ConduitKind`, a closed enum of four kinds with a fixed radius each and `for_resource` choosing copper for potable water | `src/ship/conduits.rs` | Removed: behaviour, size and look become material rows (5.3); the potable rule becomes a rule row (7.1) |
| Pipe materials (the look) and marker bands per ISO 14726 at each end, past each bend and every 6.1 m | `data/piping/pipe_materials.ron`, `marking_schemes.ron`, `src/ship/pipe_materials.rs`, `pipe_marking.rs`, `src/engine/pipe_markers.rs` | Kept and extended: the material file becomes the one catalog (5.3); bands are placed by arc length on the curve (9.2) |
| Ports with anchors, watts, flow, Mbps; the cable registry with AWG, ampacity, ohm per metre, bandwidth, range, RF emission; `check_cable`, `check_data_link`, the auto-pickers | `crates/humanity-core/src/utilities.rs`, `data/utilities/conduits.ron` | Kept; the cable rows merge into the material catalog as its size rows; the checks run on the real routed length (10) |
| Connections and the junction-node graph: `MachineConnection {from, to, kind, spec}`, `ConduitNode {id, pos, tier, kind, grid_tie}`, `ConduitEdge` | `src/machines.rs` | Replaced by `runs` (5.1, 5.5); nodes become features (11) |
| Per-connection cable picker, superconductor bulk upgrade | `src/gui/pages/construction.rs` `draw_machines_and_connections` | Grows into the material and size picker for every run (15) |
| Drag-to-connect, port node gizmos, clickable pipes (picked by sampling each polyline every 0.35 m) | `src/engine/editor.rs` `try_pick_port`, `try_pick_connection`; `src/lib.rs` | Kept as gestures; the gizmos and the rubber band become AR items (16); picking becomes per-run capsules by arc length (9.3) |
| Electrical and water islands that tick: `ElectricalSystem` balances and sheds per island, `PlumbingSystem` flows and fills tanks per island, power cuts stop pumps | `src/systems/electrical.rs`, `src/systems/plumbing.rs`, `MachineHome::electrical_islands` | Kept; islands come from runs and features, and line losses and pressure join them (10) |
| An orthogonal "up-over-down" router with per-content lanes, never called by anything | `src/systems/construction/routing.rs`, `data/routing_rules.ron` | Deleted in the auto-route rung; its lane order (power highest, waste lowest) becomes a rule row with a real source (R28, R29, R32 in place of an unread "NEC 300.4") |
| Road splines: uniform Catmull-Rom, 8 samples an edge, 2D, which "can overshoot slightly" | `src/ship/home_structure.rs` `road_edge_centerline`; `docs/design/structure-pieces.md` | Moves onto the shared curve core in a late rung (18) |
| Locks with a no-power `Crank` override, `HackPanel` and `CutPower` defeats, a `power_dependent` flag nothing reads yet; door control panels | `src/ship/lock_types.rs`, `data/blueprints/lock_types.ron`, `docs/design/locks.md` | The tapping rungs build on them (13) |
| A fire suppressor component, garden irrigation | `ecs::components::FireSuppressor`, `Irrigator` | Sprinkler and drip features drive them (11) |
| Shared building: relay-held records per frame, sequence numbers, snapshots in parts, no builder on the wire | `src/systems/construction/shared.rs`, `docs/design/shared-building.md` | Runs in shared frames follow the same pattern (14) |

## 4. The HUD question, answered

**Yes, there is a `hud.rs`: `src/gui/pages/hud.rs` (about 3,100 lines).** It is the flat layer
painted over the 3D view (health, hotbar, crosshair, compass, frame rate, weather, the survival
rows, the quest line, room and lift cards, the death screen, and the labels it projects onto
machines and people), drawn by egui as one full-screen, paint-only area that the main loop in
`src/lib.rs` shows when no page is open and `GuiState::show_hud` is on. **It is part of the GUI**,
the `gui` module, which the crate split puts in `humanity-gui`.

**Nothing owns the rest of what is HUD-like.** The constellation figures, orbit rings and the
editor's rings, guides and rubber band are 3D lines drawn by the renderer's line pass
(`src/renderer/line.rs`, `overlay_draw.rs`, `stars.rs`, so `humanity-renderer`) from lists the main
loop builds (`humanity-engine`); the floating label is a GUI widget (`body_pill.rs`); the port
spheres and flow beads are meshes in the scene. Section 16 makes them one thing, the AR layer, and
says what moves where.

The whole inventory, for reference:

| HUD-like thing | How it draws | Where | Crate (plan) |
|---|---|---|---|
| The HUD proper: vitals, hotbar, crosshair, compass, frame rate, weather, quest line, room and lift cards, death screen | 2D, fixed to the screen | `src/gui/pages/hud.rs` | gui |
| Projected labels: machine cards, crew nameplates, quest and death-pack markers | 2D at a projected world point | `hud.rs` (`marker_placement`, machine cards), `src/engine/net_route.rs` `nameplate_labels` | gui, engine |
| The build editor's CAD dimensions | 2D at projected points | `hud.rs` `draw_construction_overlay` | gui |
| The body pill and card (circle, pill, card at one anchor) | 2D widget at a projected point | `src/gui/widgets/body_pill.rs` | gui |
| Orbit rings, constellation figures | 3D lines, depth-tested, no depth write | `src/renderer/line.rs`, `overlay_draw.rs`, `stars.rs`; toggles in Settings, "Sky / map lines" (`sky_orbit_mode`, `sky_constellations`) | renderer |
| Door auto-open rings, editor rings, alignment guides, port arrows, the drag rubber band | 3D lines | pushed into `ring_lines` in `src/lib.rs` and `home_meshes::render_door_panels` | engine, renderer |
| Port spheres, flow beads | meshes in the scene pass | `src/lib.rs`, `home_meshes.rs` | engine |
| Dev overlays (F2, F3, F4) | 2D | `src/gui/pages/diagnostics.rs` | gui |
| The Maps page's overlay switches, as data rows | 2D and 3D on the Maps page | `src/gui/pages/maps/overlays.rs`, `data/maps/layers.ron` | gui |

## 5. The data: what is saved, what is computed

### 5.1 Saved per run

A sketch of the saved shape (field names are the plan's; the rung that builds it fixes them):

```rust
/// One pipe, hose, tube or cable. Saved in a household's machine rows (the save and the
/// home's data file), and, in a shared frame, as a relay record (section 14).
pub struct Run {
    pub id: RunId,                 // stable, unique in its frame, never reused (a u64 counter)
    pub material: String,          // a row of the material catalog ("copper_l_drawn", "pex_a", "nm_b")
    pub size: String,              // a size row of that material ("1/2", "12-2", "cat6")
    pub ends: [RunEnd; 2],         // where it starts and stops
    pub points: Vec<ControlPoint>, // the interior points, in order from ends[0]
    pub features: Vec<FeatureAt>,  // tees, valves, taps, sprinklers... along it (section 11)
    pub cut_length_m: Option<f32>, // a flexible run's length as cut (its slack); None: taut plus the material's default slack
    pub damage: Vec<DamageAt>,     // leaks, cuts, crush, at a position along it (section 10.3)
}

pub enum RunEnd {
    Port { machine: String, port: u16 }, // a machine's port, by instance id and port index
    Feature { run: RunId, feature: u16 },// the branch outlet of a tee, an outlet a cord plugs into
    Open { at: ControlPoint },           // a capped or cut end, or a hose lying free
    Node { id: String },                 // a conduit node, rungs 1 to 5 only: rung 6 turns nodes into junction features
}

pub struct ControlPoint {
    pub pos: [f32; 3],             // metres in the run's frame (a plot, a zone, a site: f32 is mm-exact at frame scale)
    pub on: SurfaceRef,            // what it sits on, so it follows that surface when the surface moves
    pub standoff_m: f32,           // the gap from the surface to the run's centreline (clip or hanger)
    pub hold: Hold,                // what holds the run here
    pub bend: Option<BendChoice>,  // rigid and bendable runs only: how this corner turns (6.2)
}

pub enum SurfaceRef {
    Wall { zone: u16, wall: u32, face: Face },
    Floor { zone: u16, slab: Option<u32> },    // None: the zone's ground floor
    Ceiling { zone: u16, slab: Option<u32> },  // a slab's underside, or the roof
    Machine { instance: String, face: Face },
    Piece { piece: u64 },                      // a built construction piece
    Run { run: RunId, s_m: f32 },              // lying along another run (bundled, or in a tray)
    Terrain,                                   // a planet site's ground (f64 sampled, 8.1)
    Air,                                       // nothing: a free span
}

pub enum Hold {
    Anchor(String), // an anchor row: a strap, clamp, hanger, J-hook, tie, ramp (section 12)
    Laid,           // held by friction only; real on a floor-like surface, else gravity takes it
    Shape,          // a shaping point with no hold of its own (the curve goes through it)
}

pub struct FeatureAt {
    pub kind: String,        // a feature row ("ball_valve", "sprinkler_pendent", "duplex_outlet")
    pub s_m: f32,            // where along the run, by arc length from ends[0]
    pub pinned_to: Option<u16>, // a control point it rides on: it moves with that point
    pub state: FeatureState, // open or shut, tripped, its own few numbers
}
```

What is NOT saved: what the run carries. That stays derived from what feeds it, as the marker bands
are today (`MachineHome::line_content`): honest by construction, and a cut run carries nothing past
the cut. Nor the colour, which is the material's and the marking scheme's.

### 5.2 Computed, never saved

Each of these is rebuilt from the saved run and the frame's geometry, and is reuse in the sense of
CLAUDE.md's no-caches rule (kept in memory, tagged with a version its source bumps, and obeying the
Dev page's "turn off all reuse" switch, `HUMANITY_NO_REUSE=1`):

- the **curve**: lines and arcs, or a settled rod (section 6), with an arc-length table;
- the **fittings**: elbows, couplings at the material's stick length, sleeves, glands, reducers;
- the **supports** the person did not place (Simplified mode), and the gaps where Realistic needs one;
- the **code report**: each rule's verdict and the stretch it applies to (section 7);
- the **mesh**, its LOD levels, its pick capsules, its marker band sites (section 9);
- the run's **edges in the network graph**, split at its features, and their length, resistance,
  roughness and rise (section 10);
- its **AR items**: handles, warnings, labels (section 16).

### 5.3 The catalogs (infinite-of-x)

Everything that can exist more than once is a data row (`docs/design/infinite-of-x.md`), each row
with a `source` naming a finding (R1 to R53, D1 to D19) or `ours` with a reason:

| File | Rows | Replaces or extends |
|---|---|---|
| `data/piping/pipe_materials.ron` | **the one catalog of what a run is made of**: look (as today), behaviour class (Rigid, Bendable, Flexible; 6.1), what it may carry, and its **sizes**: outside and inside diameter, minimum bend radius by method, stick length, mass per metre, roughness, pressure and temperature rating, and for cable the AWG, ampacity, ohm per metre, voltage rating, bandwidth, range, RF emission, cost per metre | `pipe_materials.ron` grows; `data/utilities/conduits.ron`'s rows become size rows of their materials, so one id resolves to look, behaviour and electrical or fluid specs |
| `data/piping/fittings.ron` | elbows and drain bends (90, 60, 45 and 22.5 degrees; short and long radius), tees, couplings, unions, reducers, caps, sleeves, glands, transits, bushings: the angles each allows, centreline radius, socket length, mesh dimensions, friction (equivalent length) | new |
| `data/piping/anchors.ron` | straps, clips, cushioned clamps, rod hangers (rod size by pipe size, R9), J-hooks, harness ties, cable trays, protector ramps: what they hold, what they fix to (wood, concrete, steel), stand-off, load, mesh | new; the dedupe of today's hangers lives on as one anchor holding several runs |
| `data/piping/run_features.ron` | valves, taps, sprinklers, drip emitters, outlets, junction boxes, splices, breakers, sensors, cleanouts, network taps, service entrances (section 11) | new; `ConduitNode`'s `grid_tie` becomes the service entrance row |
| `data/piping/run_rules.ron` | the rule rows and the **profiles** that group them (section 7) | new; absorbs `data/routing_rules.ron`'s lanes |
| `data/piping/benders.ron` | bending tools: which material and sizes, centreline radius, the angle marks (R1, R24) | new |
| `data/gui/ar_layers.ron` | the AR layer's categories (section 16) | new; "Sky / map lines" becomes rows here |

`Utility` stays a closed enum (`crates/humanity-core/src/utilities.rs`: each utility has its own physics); everything
under it is rows.

**As built (rung 0, 2026-11-02).** The material catalog's types and loader are in the CORE,
`humanity_core::run_materials`, not the ship: `utilities`' cable and data checks read its specs, and the
core may reference no crate above it. The rules, fittings, anchors and benders, which only the ship's
checks and curve will read, are the ship's (`ship::run_rules`, `run_fittings`, `run_anchors`,
`run_benders`). Every row's `source` is `humanity_core::row_source::Source` (`R(n)`, `D(n)`,
`Ours("why")`), and the tests read the findings documents' own numbered paragraphs, so a row naming a
finding that does not exist fails `just validate-data`; `scripts/check-run-sources.js` holds the same
rules in `just lints` and in CI, which runs no lib tests. Each rule also carries a `standing` (Law,
Standard, Specification, Guidance, Maker, Ours) saying how binding its source is where its profile
applies, so the editor (7.3) calls only `Law` "code". Two shapes the table above did not foresee: a
radio link (WiFi) is a `links` row, not a material, since nothing is laid; and a size is offered to the
cable and data checks only when its check's own number is known (ampacity, bandwidth, flow), so a
size with no number for its check is never checked as zero. (NM-B's 60 C ampacity is now sourced,
R45, but not yet added: adding it puts those sizes in the cable check and the pickers, which have no
cost for them; see 7.4.)

### 5.4 One word for the thing

"Run" for any pipe, hose, tube or cable, the trades' own word ("a cable run", "a pipe run"). The
code says "conduit" for the generic today (`ConduitKind`, `conduits.ron`, `conduit_nodes`), but a
conduit is a real, specific thing, the tube a cable is pulled through (EMT), which the plan needs
as itself (rung 17: a conduit run that holds cable runs). So, by the 2026-10-19 naming rule (one
concept, one word, renamed everywhere in the same piece of work): `ConduitKind` goes,
`conduits.ron` merges into the material catalog, conduit nodes become features, `conduit_runs`
becomes `runs`, and the editor's "Wire / pipe" and "utility lines" headings become "Runs" with the
glossary term `[[run]]` ("a length of pipe, hose, tube or cable from one place to another").
"Conduit" stays only for conduit.

### 5.5 What happens to today's saved connections

No compatibility code (CLAUDE.md, "No backwards-compatibility debt before launch"):

- **The data files** (`data/machines/home.ron` with 99 connections, `home_solo.ron` 58, `ship.ron` 2,
  `cabin.ron` none) are rewritten once, in the rung that lands the model, by a one-off node script that
  is run, checked and not kept: each connection becomes a run whose control points are the corners
  of the route `route_run` draws today, so the home looks the same the day the model lands, with
  each corner now a handle. Material: today's kind default (copper for potable water, the cord,
  hose and data-cable rows), its pinned `spec` as the size.
- **`connections` and `conduit_edges` leave** `MachineHome` and `HouseholdMachines` in that rung.
  `conduit_nodes` stays until rung 6, a run ending at one by `RunEnd::Node`, so the editor's
  "Place in view" node tool and "Branch" keep working; rung 6 turns each node into a junction feature
  (a tee, a junction box, or the service entrance for a `grid_tie` node) and removes the node
  list and `RunEnd::Node`. No data
  file holds a node or an edge today (`conduit_nodes: []` in `home.ron` and `ship.ron`), so nothing
  is converted then.
- **A character's save made before that rung** loads with its machines and without its runs (serde
  ignores the old field; no reader is kept for it). Nobody uses the game yet. The editor's "Auto-route
  the missing runs" (rung 9) is a feature in its own right, for any new home, and re-wires such a
  save in one click.

## 6. The curve

### 6.1 Three material behaviours

What a run is made of decides its shape, as it does in the real world (R1 to R7, R19 to R26):

| Class | Real examples | Shape |
|---|---|---|
| **Rigid** | hard (drawn) copper over 3/4 in, steel, PVC, CPVC, rigid conduit | straight lengths joined by **fittings** of standard angles; never bent in place (PVC and CPVC change direction only with fittings, R4) |
| **Bendable** | soft (annealed) copper, hard copper to 3/4 in with a bender (R1), EMT with a hand bender (R24), CSST (R7) | straight lengths joined by **bends**: circular arcs of at least the method's radius, at any angle, or by fittings |
| **Flexible** | hose, power cord, cable, PEX in free lay (R3), fibre | a smooth curve through its points, at least its minimum bend radius, hanging or lying where gravity and its stiffness put it between holds |

A run's class is its material row's; changing the material (15) changes the shape at once: copper
to PEX turns elbows into sweeps of 6 times the diameter; PEX to copper turns sweeps into elbows.

### 6.2 Rigid and bendable runs: lines and arcs

The control points of a rigid or bendable run are its **corners**, the places a plumber or
electrician would bend or fit it, which is also how a real tube bender describes a part (feed,
rotate, bend). The curve is the corner polyline with each corner **filleted by a circular arc** in
the plane of its two legs:

- **Radius.** A bend's radius is at least the material and size's minimum for the method (bender
  rows, R1, R24); a fitting's is the fitting's own centreline radius (short or long radius elbow).
  `BendChoice` on the point picks bender or fitting; Realistic picks a fitting where the material
  has no bender row (hard copper over 3/4 in, R1; PVC, R4).
- **Angle.** A bend may turn any angle. A fitting turns only the angles the fitting rows offer: for
  drains the bends R38 names (quarter, sixth, eighth and sixteenth bends: 90, 60, 45 and 22.5
  degrees), R38 also deciding which may turn which way; for pressure copper the standard 90 and
  45-degree elbows the CDA's friction table lists. Realistic snaps the corner's angle to the nearest offered one
  by moving the point along its leg (the person sees it move), or composes two fittings (two
  45-degree elbows with a short straight, a real offset).
- **Feasibility.** A fillet needs `R tan(theta / 2)` of straight on each leg. Where a leg is too
  short for both of its corners, the corner is shown in the AR layer with the length it needs, and
  the editor offers the fixes: move a point, use a fitting (a tighter radius), or join two corners
  into one.
- **Lengths.** Couplings sit at the material's stick length along straights; a bend never falls on
  a coupling.

The result is tangent-continuous (G1) everywhere, its curvature exact and piecewise constant, which
is the real shape of bent tube and makes the radius check trivially true by construction.

### 6.3 Flexible runs: a curve through the points, then the rod settles

- **The intent curve** is a **centripetal Catmull-Rom** spline through the control points (alpha
  0.5): it passes through every point the person places ("put a point down and the cable goes
  through it"), and, unlike the uniform form the roads use, it is proven to form no cusp or
  self-intersection within a segment (Yuksel, Schaefer and Keyser, 2011). It is what the editor
  draws while a point is being dragged.
- **The built shape** is a **rod settled between holds**: an XPBD Cosserat rod (Macklin, Müller and
  Chentanez, 2016; Kugelstadt and Schömer, 2016), stretch, bend and twist constraints from the
  material's size row, gravity, and contact with the surfaces the run lies on. Ports and anchors
  pin it; a `Laid` point pins it only where the surface under it can hold it by friction (a floor,
  a shallow slope), so a laid point on a wall or ceiling falls to the next anchors: "a loose hose or
  cable lies where physics puts it between anchors". `Shape` points steer the starting guess and pull
  gently; they do not pin. The cut length sets the slack: a cable longer than the path sags or snakes.
- **When it runs.** The settle runs on an edit or a disturbance (a drag, a vehicle running over it,
  a feature added) as a background job of a fixed step count, then sleeps; until it finishes the
  intent curve is drawn. It is cosmetic beyond the run's length and its laid path, so it runs on each
  client and is never networked (CLAUDE.md, the compute budget); it must give the same shape on the
  same inputs on one machine (fixed steps, no time-dependent input), so a reload looks the same.
- **Simplified mode** skips the settle: the intent curve is the shape, its tight stretches eased to
  the minimum radius.

### 6.4 The minimum bend radius, enforced per class

- **Rigid and bendable:** by construction (6.2): the fillet's radius is never below the rule's.
- **Flexible:** after the settle, the curvature along the rod (its discrete turning angle per
  segment length) is compared with `1 / R_min`. A tighter stretch is a **kink**: shown amber in the
  AR layer with the rule ("PEX: at least 6 times its outside diameter", R3), and real in the
  simulation (10.2: a kinked hose passes less flow; a cable bent tighter than its maker allows
  gains a fault rate). The drag itself is not blocked: the person sees the warning appear as they
  pull too tight, as a real hose would show a kink.

### 6.5 Why not the other curve families

- **Cubic Bezier** gives each point two tangent handles. That is a second idea to learn for every
  point, the opposite of "as easy as possible", and its curvature is no easier to bound.
- **Uniform Catmull-Rom** (the roads' today) passes through its points but can cusp and overshoot
  (the roads' own note); centripetal fixes that at no cost.
- **Clothoids** (curvature changing linearly) are the right transition for a vehicle at speed on a
  road or rail. Bent tube is not shaped like that: it is arcs and straights. Wrong physics for pipe.
- **A general minimum-radius-constrained spline fit** (an optimisation) would be slower, harder to
  test and still not the real shape of a bend. The fillet construction is exact, and the rod is the
  real physics of a flexible line.

## 7. "To code": the rules, from data

### 7.1 The rule kinds

Every row in `data/piping/run_rules.ron` is one of these kinds, with its numbers, what it applies
to (materials, sizes, contents, wall ratings) and its `source`:

| Kind | Checks | Example rows (findings) |
|---|---|---|
| `bend_radius` | the fillet or the settled curvature against the minimum | copper by size and temper (R1), PEX 6x OD and 18x against the coil (R3), NM-B about 4x (maker's, R19), UTP 4x, fibre 10x installed and 20x while pulling (R20), ship cable 4D to 12D (R22), aircraft bundles 10x (R23) |
| `fittings_only` | a rigid material has no bend | PVC, CPVC (R4) |
| `support_spacing` | the longest unsupported stretch, horizontal and vertical | 24 CFR 3280 plastic 3 and 4 ft (R8), UFGS steel and copper by size (R9), PEX 32 and 48 in (R3), NM cable 4 1/2 ft (R13), ship cable 24 in (R14), aircraft 24 in (R16), spacecraft ties (R17), J-hooks 4 to 5 ft (R18) |
| `support_near` | a support within a distance of a bend, a valve, a box, a runout's end | 1 ft of a bend, 5 ft at valves, 3 ft of a runout's end (R9, R10); 12 in of a box (R13) |
| `separation` | the clearance between two content groups, parallel or crossing | communications 2 in from power (R27), 12 in when parallel (R20); conduit 6 in from hot pipes and flues (R31); PEX 6 in from a water heater flue (R34); wiring above flammable fluid lines, 6 in where possible, 2 in minimum (R32) |
| `keep_out` | a volume a content may not enter | piping in the 6.0 ft (1.83 m) dedicated space over a panel (R28); piping over switchboards (R29) |
| `vertical_order` | which content runs above which where they share a route | power above water (R28, R29, R32; the drip reasoning), the old `routing_rules.ron` lanes |
| `no_shared` | holes or supports two contents may not share | pipe and wire through one hole (R30); raceways and piping on one support (R31) |
| `slope` | a drain falls toward its outlet | 1/4 in per foot, 1/8 in with a cleanout or for larger pipe (R36, R37) |
| `drain_turn` | which fitting may turn a drain which way | R38 |
| `penetration` | how a run crosses a wall, deck or stud | sleeve and gasket by wall material (today's passthrough); plates within 1 1/2 in of a stud face (R41); bulkhead integrity, no heat-sensitive metal in a fire boundary, glands or transits through watertight bulkheads (R39, R40); fire stops every 7 m vertically for some cable (R40) |
| `pull_limit` | bends between pull points in a conduit | two 90-degree bends for data conduit (R20); 360 degrees for power conduit (NEC 358.26, R45) |
| `fill` | how full a conduit or tray may be | 0.40 (R22) |
| `allowed_material` | which materials may carry a content | drinking water: copper, the operator's rule recorded in `conduits.rs` (open question 2) |

Rows are grouped into **profiles**, because the same cable is strapped every 4 1/2 ft in a
manufactured home (R13) and every 24 in aboard a small passenger vessel (R14). The starting
profiles: `dwelling_us` (24 CFR 3280 where it has a number, UFGS for metal pipe and conduit, the
makers' radii), `ship_us` (46 CFR and IEEE 45.8, which 46 CFR incorporates), `spacecraft` (NASA-STD-8739.4A,
with FAA AC 43.13-1B as the analogue where NASA gives no number). A ship file names the profile for
its services, a home file for its own wiring, so a home aboard the ship follows the dwelling
profile inside its walls and the ship's in the corridors; the editor says which applies where. A
new profile (another country, an alien ship's) is rows.

Where a rule has no public source the row is marked `ours`, its number the maker's or the closest
public rule, and the editor's words never call it "code". Each row also carries its `standing`
(Law, Standard, Specification, Guidance, Maker, Ours), and only Law is called "code". The NEC was
first thought unreadable (R42); the 2014 edition, the one 24 CFR 3280 incorporates, was read on
2026-11-02 from Public.Resource.Org's posting (R45, numbers only), so in the dwelling profile its
numbers are Law.

### 7.2 Evaluating them

A run's check is a pure function of the run, its computed curve, the frame's other runs and the
frame's geometry (`ship::run_checks`, relay-safe). Separation and keep-out checks look only at runs
and equipment in the same cells (9.3), so a check costs what is near the run, not the ship.

### 7.3 What the editor says

Each finding has a **severity**:

- **Fault**: physically impossible or wrong in kind (a rigid bend tighter than any method can make,
  water in a data cable, a run through a door opening). The editor will not build it; the AR
  layer shows the place in red and why.
- **Code**: breaks a rule row (a strap 1.9 m from the last where the profile says 1.37 m). Built,
  shown amber, listed in the run's card and the buildability report, and, in Realistic, real in the
  simulation (10.3).
- **Advice**: good practice without a rule (a cleanout at a long drain's upper end).

The words come from `data/i18n/en.json` by key, in plain language with the number and the source
behind a "Why" fold: "Needs a strap here: it has gone 1.9 m without one, and cable like this is
held every 1.37 m (24 CFR 3280.808)". Each finding carries its **fix** as a button that does it:
"Add a strap", "Move it 0.15 m from the panel", "Use a 45-degree elbow". An AI reads the same list
through the construction console and the introspection JSON.

### 7.4 What the refutation pass left for later rungs (2026-11-02)

The pass over rung 0's data (84 of 101 rules confirmed, 6 corrected, 2 refuted; 16 of 67 sizes
corrected; the findings document's notes of that date) found eight things the data alone cannot
settle. Each needs code, so each belongs to a rung:

1. **Inside or centreline radius** (rungs 3 and 8). The NEC's NM rule, IEEE 45.8 and Parker
   measure a bend to its inner edge; the NEC's EMT table and Klein to the centreline; PEX at the
   bend's mid-point; the CDA does not say. `BendMin` needs a field saying which, and the check
   converts before comparing with the curve's centreline curvature. run_rules.ron's header records
   each row's convention meanwhile.
2. **`no_parallel` only inside the preferred gap** (rung 8). The FAA's rule forbids wiring parallel
   to a fuel line only where 6 in cannot be kept; applied flatly it would flag a compliant run.
3. **`penetration_pex_hole`'s `clearance_m`** is a difference in diameter (the hole 1/4 in wider),
   not a gap on each side (rung 8).
4. **`bend_data_conduit`'s six diameters**: against the outside diameter it fails 3/4 in EMT on
   Klein's 5.5 in bender, against the bore it passes. TIA-569 was not read; the rule may need a
   "times the bore" option (rung 8 or 19).
5. **The comms-to-power exemption** (NEC 800.133 Exception 1) must count conductors in a raceway
   as exempt once THHN runs inside EMT (rung 19).
6. **NM-B ampacity** is sourced (60 C column: 15, 20, 30, 40, 95 A) but not added, because it would
   put those sizes in the cable check and the pickers with no cost (rung 6, with the material
   picker).
7. **Six-diameter pipe bends** (UFGS 22 00 00, DoD projects only) are not applied: they would fail
   every CDA copper radius. A profile row if a military profile is ever wanted.
8. **A ship switchboard's working space** (46 CFR 120.340(f): 610 mm in front, 455 mm behind) is not
   added yet; a `keep_out` row for `ship_us` (rung 8).

### 7.5 Two modes

Settings > Gameplay > **Pipes and cables: Simplified / Realistic**, built like the page's other
realism switches. **Realistic is the default** (the operator, 2026-11-01: "We should go with what
people would experience in the real world"):

- **Realistic:** the three behaviour classes, the rod settle, every rule of the place's profile,
  Code findings with real consequences, supports placed by the person (the editor proposes them).
- **Simplified:** the same materials drawn the same way, but bends eased to the radius silently,
  supports placed automatically at the profile's spacing, no Code findings (Faults stay: water does
  not flow in a data cable in any mode), and no consequences from rules (no kinks, no leaks from
  unsupported spans). Harm to the body from a run (a shock from a cut live cable) follows the
  existing harm settings, not this switch.

## 8. Placing and editing a run

### 8.1 What a point snaps to

The cursor's pick ray is tested against the frame's geometry, nearest hit first:

- **Walls**: `ship::wall_collision::WallSegment` (a 2D segment, its half thickness and its heights
  `y0` to `y1`), as two faces and their ends, every zone's walls in world space as `run_site`
  gathers them now;
- **Floors and ceilings**: the storeys' `FloorSlab`s (top and underside) and each zone's box (its
  floor and roof), as `RunSite::hanger_top` reads them now;
- **Machines**: each placed machine's pick volume (its box from the placement and the catalog's
  size; `engine::geom::ray_aabb_hit`); later its model's faces;
- **Built pieces**: the construction system's piece boxes;
- **Other runs**: their capsules (9.3), to lay a cable along a pipe or into a tray;
- **Terrain** at a planet site: the height sampler in f64 (CLAUDE.md, f32 at planet scale);
- and, for the AR-glasses target, the **headset's detected planes** (16.5): surfaces come through
  one `SurfaceSource` interface, so a real room is just another source.

Today's home is boxes and wall segments, so these analytic tests are exact; a triangle BVH over a
static mesh is added only when a surface is not a box (a curved hull).

### 8.2 Free placement; snapping helps, never cages

The operator: "I don't want to suffer from fixed voxel placements."

- **No grid.** A point slides continuously over the surface under the cursor, at the run's
  stand-off. Nothing rounds it.
- **Helps, each its own toggle** in the run panel (GUI-first) and each drawn in the AR layer as a
  faint guide when it acts, as the alignment guides do now: magnet to **ports** (within a few
  centimetres of screen-space distance), **parallel to another run** at the separation rule's
  distance (a neat rack), the structure's **alignment guides** (`snap_to_alignment`'s 0.3 m),
  and for rigid runs an **angle snap** to the fitting angles.
- **Off for a moment:** hold Alt (rebindable) or press "Free" in the panel: the point leaves the
  surface and moves in the view plane, or along the move gizmo's axes, into the air (a free span of
  hose, a rigid pipe across a gap with supports at each end).
- **Off for good:** "Snapping: off" in the panel; surfaces still report what a point sits on when it
  happens to touch one, for the checks.

### 8.3 Following surfaces

Between two points on different surfaces (a wall and the ceiling), the straight leg would cut
through the air of the room. In Realistic, for rigid and bendable runs, the editor inserts the
corner where the two surfaces meet (the wall-ceiling crease), which is where an electrician runs
it (R26: exposed conduit parallel with or at right angles to walls and ceilings). A flexible run
gets the same crease point as a `Shape` point. A free span is a choice made with "Free" (8.2), never
an accident.

### 8.4 Auto-route

"Route" proposes a code-correct path between two ports (or from a port to a run, a tee):

- **The routing graph** of a frame: nodes along the creases where walls meet floors and ceilings,
  at wall ends and corners, at candidate wall penetrations (not through doors or windows), at slab
  risers, and along existing runs' racks; edges along surfaces. Built once per structure change and
  kept with the structure's version number (reuse that cannot go stale, obeying the reuse switch).
- **A* search** with a cost of length, plus each bend (a fitting's cost for rigid, small for
  flexible), plus each penetration (by wall material and rating), plus rule penalties: keep-out
  volumes are walls of the graph, separation from other runs costs by how close and how long,
  vertical order prefers the content's lane, and a drain's edges are allowed only falling at the
  profile's slope. Sharing a rack with runs of a compatible content is cheaper, so parallel runs
  bundle along a ceiling the way the conduit-node graph's Stage 3 wanted.
- **The result** is a corner polyline on surfaces; the material's shape (6.2, 6.3) is built from it;
  every point is editable afterwards, and nothing re-routes a run the person has edited unless they
  ask.
- This replaces today's fixed up-across-down (`route_conduit`) and still never cuts diagonally
  through a room, because the graph has no edges through the air. The per-storey service height and
  the risers through slabs become graph nodes and costs, not special cases.

### 8.5 Editing by hand

All of it in the AR layer (16), all of it also from the run panel and the console (15):

- **Drag** a point along its surface; the run re-fillets or re-curves live.
- **Insert** a point by clicking the run where it should bend; **remove** one with Delete or the
  point card's button.
- **Draw** a new run point by point from a port with the pen tool, ending on a port, a run (a tee
  is added) or open (a cap, or a hose lying free).
- The **point card** (a body pill and card at the point): its hold (anchor type, laid, shape), its
  surface, its stand-off, for a corner the bend method and angle.
- **Slide** a feature or an anchor along the run (11, 12).
- **Undo and redo** with the editor's existing stack.

## 9. The mesh

### 9.1 Sweeping

- A circular cross-section is swept along the curve with **rotation-minimizing frames** by the
  double reflection method (Wang, Jüttler, Zheng and Liu, 2008): stable on straights, no flip at an
  inflection, cheap. (Frenet frames are undefined on straights and flip.)
- **Sampling by curvature, not by length:** a straight is one span however long (its two rings,
  plus where a band or fitting starts); an arc is cut into `ceil(theta / step)` spans; a settled
  rod's segments are merged while their turning stays under the step. The angular step per LOD
  (ours, to tune by measurement): 10 degrees near, 22.5 middle, 45 far.
- **Sides by apparent size:** a cross-section's sides by the run's radius on screen (for example 16
  near for a 1 in pipe, 6 for a data cable, 4 far), and past a distance the run is drawn by its
  cell's far LOD or culled with its room.
- Texture coordinates run along the arc length, so a material's print line or a cord's twist can be
  drawn without geometry.

### 9.2 Fittings, anchors and bands

- **Fittings are built from their rows' dimensions**, procedurally, no art assets: an elbow is the
  arc itself swept at the socket's diameter with collars at each end; a tee, a coupling, a union,
  a valve body from their rows; the gasket collar at a wall stays (today's passthrough), sized by
  the penetration rule.
- **Anchors** likewise (a strap over the pipe into the surface, a rod hanger to the ceiling over
  it, a J-hook, a ramp's channels); one anchor holding several runs (a shared hanger, a tray) is
  drawn once, today's dedup made real.
- **Marker bands** (ISO 14726, `ship::pipe_marking`) are placed by arc length: at each end, just
  past each bend (an arc for rigid runs; for a flexible run, a stretch that turns more than 45
  degrees within a metre, ours), and never more than 6.1 m apart along the curve, swept as short
  sleeves on the same frames, merged per colour as now.

### 9.3 Batching and picking

- The meshes are **merged per cell** (a room, or an 8 m cube of a large space) **per material**:
  one draw per material per visible cell, rebuilt only when a run in that cell changes. Today's one
  render object per straight leg goes.
- **Picking** keeps per run a set of capsules along the curve at LOD0 sampling, in a small BVH per
  cell. A ray returns the run, the arc length `s` and the point, within a screen-space margin (a few
  pixels, so a thin cable is as easy to click as a thick pipe), which also says where an inserted
  point, a feature or an anchor would go. Today's sampling every 0.35 m with a 0.28 m radius goes.
- The build editor's **cut-away** keeps working on the curve's samples (`parts_under_cut`).

### 9.4 Budgets

These are budgets (ours), to be measured in the rung that builds each part; the rung's gate fails
if the measurement is over (CLAUDE.md: a waste of 5% or more is worth a fix).

| What | Budget |
|---|---|
| Runs idle, per frame | 0 ms CPU (static merged meshes; nothing rebuilt) |
| Drawing a home's runs | within the home pass's existing GPU budget; the AR layer's own pass at most 0.3 ms GPU |
| Dragging one point, per frame | at most 1.0 ms CPU for the dragged run's curve, mesh and checks, and its cells' re-merge |
| One run's rebuild (curve, fittings, mesh, checks), a 20 m run with 10 corners | at most 0.3 ms |
| A cell's re-merge | at most 2 ms, off the frame thread when over 0.5 ms |
| A home cold (about 100 runs) | at most 20 ms at load |
| Auto-route of one run in a home | at most 5 ms, on a worker; the preview while dragging from a port at 10 Hz at most |
| Flexible settle | a worker job of fixed steps, at most 10 ms of worker time for a 20 m cable; the intent curve shows meanwhile |
| The AR layer's compositor (16) | at most 0.2 ms CPU per frame for a few thousand items |
| Network solves (10) | per island, only when its inputs changed; at most 2 ms of sim time per game second for a whole ship, in slices |

### 9.5 The scale case: thousands of runs on an MMO ship

Take 200 homes of about 100 runs each plus the ship's own services: about 25,000 runs, averaging
15 m and 8 corners.

- **Saved:** about 0.5 to 1 KB a run (points, holds, features), so 12 to 25 MB for the ship's runs,
  held by the relay only for shared frames and by each household for its own (14).
- **Drawn:** only cells in view and near: the rooms the player can see (the home pass's room
  visibility and the cut-away), LOD0 within a few metres. At about 500 triangles a run at LOD0, a
  room of 40 runs is 20,000 triangles; far cells are far LODs or culled.
- **Rebuilt:** only dirty cells; a ship loads its nearest cells first, the rest in background
  slices over about 2 s.
- **Simulated:** islands are recomputed only when topology changes (union-find over runs and
  features, as now); each island's solve runs when its inputs changed, at the sim tick, never per
  frame, and a home reports one rolled-up draw to its substation (grid-hierarchy.md's open
  question, answered the same way).
- **Checked:** by cell, so a check reads its neighbours, never the ship.

## 10. Functional: what a run does

### 10.1 Networks come from runs and features

A network is a graph whose edges are the stretches of runs between features and ends, and whose
nodes are machine ports, features (a valve, a tee, an outlet) and junctions. Islands (the union-find
`electrical_islands` and `water_islands` do now) come from this graph, so a shut valve, a tripped
breaker, a cut or an unplugged cord splits an island, and the existing systems (`ElectricalSystem`
per island, `PlumbingSystem` per island, the pump that stops when its power is shed) do the rest.
The length of each edge is its **real routed arc length** plus its vertical runs, never the straight
line between machines (which the Conduits buildability check uses today).

### 10.2 Physics per utility

Each built on what exists, each solved per island when its inputs change:

- **Electricity:** `check_cable`'s ampacity and voltage drop, today a design-time verdict, also run
  in the island's tick: a radial walk from the island's sources gives each load the voltage it
  really gets after its runs' resistance (ohm per metre from the size row, times the routed length
  out and back). A load below its machine row's minimum voltage browns out (runs slower, or stops,
  per its row); a run over its ampacity heats, and in Realistic gains a fault rate, and far over it
  becomes an ignition source for the existing fire system.
- **Water:** pressure and flow. Each island's sources give head (a pump's curve from its row, a
  tank's height); each edge loses head by Darcy-Weisbach with the material's roughness, plus its
  fittings as equivalent lengths (the CDA's Table 14.7, still to be read, on the routing finding's list of what is still unknown),
  plus its rise. The plumbing system's flows are then capped by what the pressure can push, so a
  long, thin, bendy run starves the fixture at its end, which teaches pipe sizing. A kinked hose
  (6.4) adds loss. Drains flow by gravity at their slope; a drain laid level backs up.
- **Data:** `check_data_link`'s bandwidth and range over the routed length; a data run parallel to
  a power run inside the separation rule's distance for more than a metre loses link quality (the
  size of the effect is ours, labelled so); fibre is immune (its RF emission and pickup are zero).
- **Air:** hoses and, later, ducts (a rectangular section is a material row) with the same flow and
  loss model at air's density.

### 10.3 Breaks, leaks and cuts

Realistic only, from the run's state and its rules:

- **Leaks** start at joints (fittings, couplings) at a rate from the fitting row, raised by an
  unsupported span, a kink, freezing or damage. The water goes to the **drip point**: the first
  surface straight below the leak. A leak over a live electrical box or a panel faults it, which is
  why the keep-out and vertical-order rules exist (R28, R29).
- **Cuts and crush:** a vehicle's wheel over a laid run without a ramp (12), a tool, a fire. A cut
  splits the island at that point; a cut live cable is a shock hazard (the harm follows the existing
  harm settings).
- **Repair** is a feature: a coupling or a splice placed over the damage.

## 11. Features along a run

The operator: "Like we have a vertex along the spline we can move to add a water sprinkler for
fires or watering plants or whatever."

A **feature** is a row of `data/piping/run_features.ron` placed at an arc length `s` on a run, or
pinned to one of its control points (then it moves with the point: the vertex he describes). It is
a node of the run's network, with ports in the `utilities::Port` model, and it drives an existing
system where one exists:

| Feature | Sits on | What it does |
|---|---|---|
| Tee, wye | pipe, hose | a branch: a new run starts at its outlet (`RunEnd::Feature`) |
| Junction box, splice | cable | a branch or a join for cable; a splice is also a repair |
| Valve (ball, gate, check) | pipe, hose | open, shut or throttled; shut splits the island; a check valve passes one way |
| Tap, hose bib | water pipe | an outlet: a water consumer while open; a hose run can start from it |
| Fire sprinkler head | water pipe | an `ecs::components::FireSuppressor` (range and strength from its row) fed by its island: it sprays only while its run carries water at its row's minimum pressure, and opens by its fusible link's temperature or by hand |
| Drip emitter | water pipe or tubing | an `Irrigator` with its litres an hour, watering the grow area in its radius from its island |
| Outlet (socket) | power cable | a power port a machine's cord plugs into (`RunEnd::Feature`) |
| Breaker, fuse | power cable | opens over its current; reset by hand |
| Sensor (flow, pressure, current, temperature) | any | a live reading shown on its AR label (16) |
| Cleanout | drain | where the slope rule's exception asks one (R36) |
| Cap, plug | any open end | closes it |
| Network tap | data cable | a data port onto the run's network (13) |
| Service entrance | any | where a home meets the ship's main line (`ConduitNode::grid_tie` today; grid-hierarchy.md S1) |

Placement follows the material: on a rigid run a tee or a valve sits on a straight, never on an
arc (a real fitting cannot), and Realistic slides it to the nearest straight; on a flexible run,
anywhere. Each row names the skill and tools it takes to install, its cost and its mesh dimensions.

## 12. Anchors, and ramps vehicles drive over

The operator: "add anchors or remove anchors to keep cables/hoses/whatever anchored with like those
ramps so they can be driven over safely with heavy machinery/vehicals."

- An **anchor** is a control point's `Hold::Anchor(row)`: a strap, clip, cushioned clamp (plastic
  pipe's hangers must not cut or abrade it, R8), rod hanger, J-hook, harness tie, a place in a tray.
  Its row says what it holds, what it fixes into (a wood screw, a concrete anchor, a weld stud), its
  stand-off and its load. Add one by clicking the run with the anchor tool; remove it with Delete;
  slide it along.
- **Support spacing reads anchors**, so the code check (7.1) is the real one: the person sees the gap
  that needs a strap and fixes it with a click.
- **Between anchors a flexible run lies where the rod puts it** (6.3), so removing an anchor on a
  wall lets the cable sag to the next.
- **A cable protector ramp** is an anchor on a floor-laid stretch: a row with its channels (count and
  size), its slope, its footprint and its load rating. A vehicle wheel crossing a laid run is
  checked against the ramp over it: rated at or above the wheel's load, nothing happens; under-rated,
  the ramp takes damage; no ramp, the run is crushed (10.3) and the vehicle bumps. Two data gaps
  before this works: the ramps' load ratings (a maker's sheet, on the routing finding's list of what is still unknown) and the
  vehicles' wheel loads (no mass field was found in `data/vehicles/kits.ron`).
- **Shared supports** (one hanger, one tray, many runs) are one anchor that several runs reference,
  and the rule that raceways and piping never share supports (R31) is checked on it.

## 13. Tapping into infrastructure

The operator: "It'd be cool to some how justify a player tapping into infrastructure elements to do
things. Like boarding an alien craft, locating their data cables, and then trying to hack in. Like
if there's a door and it has emergency panels to remove to manually open powered doors. Players
should be able to open that panel."

The run and feature model makes this physical: to do something to a network you must reach a run
that carries it, which runs somewhere real. Four steps, each a rung or two:

1. **Find.** The AR layer shows what a run carries (its content, its live state: flow, live or dead,
   pressure) for every run the character **knows**: every run of their own home and plot, and any
   other they have traced or sensed. Unknown runs (inside walls, behind panels, aboard someone else's
   or an alien craft) are not shown until revealed by **sensing gear**, the detection layer's
   sensors applied to runs (`detection-sensing.md`): a current or field probe finds live power, an
   RF scanner finds unshielded data carrying traffic (the media rows' `rf_emission` already exists;
   fibre gives nothing), a thermal viewer finds hot water and loaded cable, a tone tracer follows one
   run from an end the person can reach, a wall scanner finds runs in a wall. Each reveal adds the
   run to the character's known set, which their save keeps. A plain toggle, "Show every run",
   stands in for the gear (CLAUDE.md: a gear-gated feature also gets a plain toggle).
2. **Open.** Concealed runs sit in **service chases** behind **access panels**, features of a wall
   (a panel row: removable with or without a tool, the time it takes, its noise, whether it goes back).
   Doors carry their own (below).
3. **Tap.** On an exposed run: a junction or splice on a cable (power off first, or live work and its
   shock hazard), a tee or a saddle tap on a pipe (shut the valve first or it sprays), a network tap
   on a data cable. A tap is a feature (11) that gives the character's device a port on that network.
4. **Act.** Through the tap: draw power or water, or reach the data network's devices. A door
   controller on that network is where `DefeatMethod::HackPanel` (`locks.md` stage 3: a held action
   with a skill and a time) is worked from, instead of only at its keypad; `CutPower` is cutting the
   controller's or the actuator's power run.

**A powered door with an emergency release**, from the real rules (D1 to D19):

- The door's actuator becomes a **load on a power run** and its controller a **device on a data
  run**; the lock rows' `power_dependent` flag (unread today) reads the controller's real island.
- Every powered door gets an **emergency release panel on each side**: a marked cover that comes off
  **without a tool** (rail's rule, D1: a "cover or a screen capable of removal without requiring the
  use of a tool"), behind it a release that **declutches the drive**, after which the door slides by
  hand, or by its crank where the door is heavy (a ship's Class 3 sliding door is "operated by power
  and by hand gear", D12; the existing `Crank { turns }` lock interaction is that hand gear).
- The release works **without power, from both sides** (D1, D5, D8, D13, D18), and is marked so it
  can be found in the dark (luminescent marking, D1; "simple and obvious", D5): the AR layer shows
  the marking on your own ship even with the power off.
- **Pressure doors and airlocks** follow NASA (D18): two separate actions to unlatch and a manual
  equalization valve opened first; a hatch does not open against a pressure difference.
- **Lift landing doors** stay locked without the key and the skill, with the car's power switched off
  first (D9 to D11), which keeps today's rule that a landing door never opens onto an empty shaft.
- Which way a door fails when power goes (shut, open, or where it stands) is the open fork in
  `locks.md`; real codes choose by door type and always keep a manual release (open question 3).

**An alien craft** is, in this model, a ship file like ours with its own rule profile (their codes),
its own materials (their cables), markings in a script the player cannot read, and no run known to
the player until sensed. Boarding it is find, open, tap, act. No alien ship exists in the game yet;
the rung waits for one.

## 14. Networking

Runs are gameplay state; the shape of a flexible run is not.

- **What syncs.** In a shared frame (a plot others can see, the ship's shared spaces), each run is a
  **relay-held record**, the shared-building pattern (`docs/design/shared-building.md`,
  `src/systems/construction/shared.rs`): `game_run_put` (the whole saved run, on commit), `game_run_remove`,
  `game_run_state` (a feature's discrete state: a valve turned, a breaker tripped; damage started or
  repaired), a snapshot in parts on entering a frame, a sequence number per change, permission by
  plot and household permit as for pieces, and **no builder on the wire** (the
  `nobody_is_told_who_built_what` rule, with its own test for runs).
- **When.** On commit, when a point is released, never while it is dragged; other players see the
  run move when it lands. Idle: nothing.
- **Sizes and rates.** A run record is about 0.5 to 1.5 KB as the wire's JSON; a frame of 500 runs is
  sent in parts kept under the relay's 128 KB message cap (about 64 runs a part). Edits are
  rate-limited like builds. A state change is about 50 bytes.
- **Never sent:** the settled rod, the meshes, flow animation, the AR layer, the checks (each client
  computes them from the same records and rule rows), and continuous values (flow, pressure,
  voltage), which each client computes from the synced discrete state. An outcome every player must
  agree on in a shared space (whether a door there has power) is decided by the relay from discrete
  inputs once the relay simulates the ship (the ship-homes plan's "when the relay stops simulating
  the Pioneer"); until then such doors keep today's behaviour.
- **Private homes** keep their runs in the household's save, not on the server, until the owner
  opens the home to visitors (as the ship-homes plan keeps a home's layout).

## 15. In-app controls, and the web

GUI-first (CLAUDE.md): everything is a control in the app.

- **The construction editor's "Runs" panel** (it replaces the utility-lines editor's dropdowns): the
  runs by what they carry; for one run, the **material and size picker** (today's per-connection cable
  picker grown to every run and every material that may carry its content, the superconductor bulk
  upgrade kept), its length and cut length, its code report with fix buttons, its features and
  anchors, Route, Re-route, Remove.
- **Tools:** draw a run, insert a point, add an anchor (by type), add a feature (by type), cut, join.
- **Snapping toggles** (8.2) and the Free button.
- **The point card and the feature card** in the viewport (body pill and card).
- **The construction console** (`exec_construction_command`, the AI's act surface) gains `run_add`,
  `run_route`, `run_point_add`, `run_point_move`, `run_point_rm`, `run_material`, `run_feature_add`,
  `run_anchor_add`, `run_check`; the introspection JSON gains each run with its check report.
- **Settings:** Gameplay > Pipes and cables (7.5); Graphics > AR layer (16.4).
- **The dev IPC** gains run verbs for rigs (a showcase pin of a run's points, the check report as
  JSON), so every gate below can be driven without a hand on the mouse.
- **Words:** every label and hint from `data/i18n/en.json` by key, glossary terms `[[run]]`,
  `[[bend radius]]`, `[[strap]]`, `[[service chase]]`, `[[fitting]]`.

**The web does not mirror the route editor, and this is why** (CLAUDE.md, "Dual-UI parity"): the
build editor is not mirrored on the web at all (`docs/PAGES.md`, "Not mirrored on web at all": it is
a 3D-viewport and gizmo tool and the web has no renderer), and the AR layer's settings act on the 3D
view the web does not have. The rung that adds the Runs panel adds this reason to that line of
`PAGES.md`. Nothing here adds a web UI pattern that native lacks.

## 16. The AR layer

The operator: "We could say it's augmented reality. What we design could literally be used to AR
whenever I can afford AR glasses. This is how we can justify the constellation lines we added to the
skybox and real star catalog. I guess we would say it's all part of the HUD."

### 16.1 The concept

**The AR layer owns every overlay anchored in the world**: route handles and previews, port gizmos,
rule warnings, labels and pills, markers, nameplates, door rings and guides, orbit rings and
constellation figures. **The HUD proper** keeps what is fixed to the screen (vitals, hotbar,
crosshair, compass, cards about where you are). In the fiction, the AR layer is what the
character's visor draws over the world (open question 1); in the Real side, it is what a pair of AR
glasses would draw over a real room.

### 16.2 The model (`humanity-core`, module `ar`)

Pure data, no GPU and no egui, f64 in world space, so every crate can produce items and the
renderer and GUI can draw them:

```rust
pub struct ArItem {
    pub category: CategoryId,    // a row of data/gui/ar_layers.ron ("sky.constellations", "build.handles")
    pub anchor: ArAnchor,        // World(DVec3) | Entity(u64) | Sky(unit DVec3, at infinity)
    pub shape: ArShape,          // Stroke(points) | Handle(kind, size) | Label(i18n key, args) | Marker(icon) | Tube(...)
    pub style: StyleToken,       // a theme token, never a colour
    pub depth: DepthMode,        // Occluded | XRay (drawn faint behind walls) | OnTop
    pub reveal: Reveal,          // Always | Known | Sensed(channel) | DevOnly
    pub pick: Option<PickId>,    // what a click on it means, for the item's producer
}
pub struct ArView { pub eye: DVec3, pub view: DMat4, pub proj: DMat4, pub viewport: Rect }
```

`ArView` is the one camera abstraction: today the player's camera, tomorrow each eye of a headset.

### 16.3 Producers, compositor, drawing

- **Producers** in their own crates: the sky (constellation figures, orbit rings), the runs (handles,
  previews, warnings, live labels), the machines (ports, labels), the doors (rings, release
  markings), the people (nameplates), quests and the death pack (markers), the editor (guides,
  dimensions). Each produces `ArItem`s, rebuilt when its source's version changes.
- **The compositor** (`humanity-engine`, `engine/ar.rs`): gathers the items each frame, applies the
  Settings switches, the HUD mode, the reveal rules (what the character knows or senses), the
  cut-away and distance, and splits them: world strokes, tubes and handles to the renderer's AR pass,
  labels, pills and markers to the GUI's AR labels.
- **The renderer's AR pass** (`humanity-renderer`, `renderer/ar_pass.rs`, grown from `line.rs` and
  `overlay_draw.rs`): draws strokes as **wide lines** (screen-space quads with a few pixels of
  anti-aliased width, in place of today's 1 px aliased `LineList`), handles as small instanced
  meshes, in the item's depth mode (X-ray is what makes "see the run inside the wall" possible), into
  **its own premultiplied-alpha target, composited over the scene last**. That separate target is
  the hinge for glasses (16.5).
- **The GUI's AR labels** (`humanity-gui`, `gui/ar_labels.rs`): every projected label through the
  body pill widget, the collision-dodging layout it already does, and the HUD's projected machine
  cards, nameplates and markers moved here from `hud.rs`, which keeps only the screen-fixed HUD.

### 16.4 Settings

Settings > Graphics > **AR layer** (where "Sky / map lines" sits today): one row per category from `data/gui/ar_layers.ron` (name and
hint by i18n key, default on or off, which HUD modes include it, whether it needs sensing), drawn
from the rows, so a new category is a row, and a coverage test (like `theme_editor_coverage`) fails
when a row has no switch. Today's "Sky / map lines" section (orbit rings, constellation figures)
becomes the AR layer's Sky rows, renamed in the code and config in the same rung (the naming rule);
their colours stay theme tokens in Appearance.

### 16.5 The AR-glasses target

What changes for glasses, and what does not:

- **A different camera:** `ArView` from the headset's per-eye pose and projection, through OpenXR
  (Khronos's open standard; which Rust binding, and its licence, is a findings question for that
  rung).
- **A different compositor:** the AR pass's own target is submitted to the headset as a layer over
  the real world, instead of being blended over our rendered scene; the scene pass does not run.
- **Different surfaces for a real room:** the headset's detected planes and meshes become a
  `SurfaceSource` (8.1), so the same run model, rules and editor lay out a real cable run in a real
  house, the two-realities axiom's Real side (`docs/design/two-realities.md`), with the rule profile
  of the person's own jurisdiction.
- **The same:** the items, the categories, the producers, the run model, the checks, the words.

### 16.6 Where it lives in the crate split, and what moves

| Piece | Crate | From |
|---|---|---|
| `ar` model and projection math | humanity-core | new |
| Producers | their own crates (ship for runs and doors, systems for sensing reveals, core for the sky, engine for the editor's guides) | the code that pushes into `ring_lines` today |
| Compositor | humanity-engine (`engine/ar.rs`) | new; takes over `ring_lines` and its call sites in `src/lib.rs` |
| AR pass | humanity-renderer (`renderer/ar_pass.rs`) | `line.rs`, `overlay_draw.rs`'s line functions |
| AR labels | humanity-gui (`gui/ar_labels.rs`) | `hud.rs`'s projected labels, `body_pill.rs` unchanged |
| Settings section and config fields | humanity-gui | `settings.rs` "Sky / map lines", `config.rs` `sky_orbit_mode`, `sky_constellations` |
| Category rows | `data/gui/ar_layers.ron` | new |

Every arrow points down the layers (`scripts/crate-layers.json`): the core knows nothing of ships or
pages; the renderer and GUI read core items; the engine wires them.

## 17. Where the routing pieces live in the crate split

| Piece | Crate | Module (plan) |
|---|---|---|
| Ports, utilities, cable and data checks | humanity-core | `utilities` (as now) |
| The material catalog and row sources (built, rung 0) | humanity-core | `run_materials`, `row_source` (the cable checks read the catalog, so it cannot live above the core) |
| The rod solver | humanity-core | `physics::rod` (pure) |
| Run model, rule and part catalogs (`run_rules`, `run_fittings`, `run_anchors`, `run_benders`: built, rung 0), curve, mesh as plain data, checks, auto-router, surfaces | humanity-ship | `ship::runs`, `ship::run_curve`, `ship::run_mesh`, `ship::run_checks`, `ship::run_route`, `ship::surfaces` (all relay-safe, no GPU) |
| Islands from runs | humanity-ship | `machines` |
| Network physics, damage, sprinklers, drip, doors on power, sensing reveals | humanity-systems | `systems::electrical`, `plumbing`, new `pipe_flow`, `run_damage`, `fire`, `farming`, `door_anim`, `interaction` |
| Shared run records | humanity-systems (the contract) and humanity-relay (the table and handlers) | `systems::construction::shared_runs`, `relay::storage::runs` |
| Mesh upload and the AR pass | humanity-renderer | `renderer::ar_pass`, existing mesh upload |
| Runs panel, cards, settings, AR labels | humanity-gui | `gui::pages::construction`, `gui::ar_labels`, `gui::pages::settings` |
| Editing input, cell rebuilds, bands, the compositor, IPC verbs | humanity-engine | `engine::run_edit`, `engine::run_draw` (replaces `rebuild_connection_objects`), `engine::pipe_markers`, `engine::ar`, `engine::ipc` |

The mesh is built as plain vertex data in `ship::run_mesh` (as terrain builds its chunk meshes as
plain data) so its tests run without a GPU; only the upload is the engine's and the renderer's.

## 18. The rungs

In order. Each is a slice of the architecture above, never a throwaway; each has a gate that is run
and **seen failing first** on a deliberately broken input before it is trusted (memory
`feedback_checks_that_cannot_fail`). Sizes are estimates of the change: **S** under about 500
lines, **M** about 500 to 1,500, **L** about 1,500 to 3,000; to be corrected by what each rung
actually takes.

| # | Rung | What it delivers | Gate (red first) | Size |
|---|---|---|---|---|
| 0 | **Rules and materials as data** (BUILT 2026-11-02) | `run_rules.ron` with the three profiles, `fittings.ron`, `anchors.ron`, `benders.ron`, the material catalog's behaviour and size fields (conduits.ron merged in), loaders, every row with a `source` | `just validate-data` loads them; a lint fails on a row with no `source` (seen failing on one); a test finds 24 CFR 3280.808's 1.37 m and CDA's 57 mm for 1/2 in annealed copper | M (took about 2,300 lines of Rust, 600 of them tests, and 2,200 of data: L by lines, most of it the 101 rule rows and the catalog's 67 sizes of 17 materials) |
| 1 | **The run model** | `Run` and its parts replace `connections` and `conduit_edges` (nodes kept as ends until rung 6); data files converted once; islands, the Conduits check and the markers read runs; **voltage drop on the real routed length** | the home's machine files round-trip byte for byte; the existing routing, marker and island tests pass on runs; `just verify-runtime` enters the world; a vantage screenshot of the utility room matches the one before (compared on a deliberately moved run first, to see it fail) | L |
| 2 | **AR layer: model and compositor, with the sky moved in** | `core::ar`, `engine::ar`, the AR pass on today's lines; constellation figures and orbit rings as items; "Sky / map lines" becomes the AR layer's Sky rows | the sky vantage's screenshot is pixel-identical before and after; the category coverage test fails on a row with no switch | M |
| 3 | **The curve core** | `ship::run_curve`: fillets with radius and angle rules, centripetal Catmull-Rom, arc length, rotation-minimizing frames, the feasibility report | unit tests: every fillet at or over its minimum; arc length within 1 mm on known shapes; no frame flip on a helix; the uniform Catmull-Rom (the roads') shown to cusp on a known input where the centripetal does not | M |
| 4 | **The mesh, fittings and bands on curves** | swept meshes merged per cell and material, elbows as real bends, couplings at stick lengths, bands by arc length, picking by capsules; today's cylinders and 90-degree corners go | a watertightness test (no gap at an arc's joins; seen failing with a misaligned frame); the rebuild budget measured in a lib test; `just verify-runtime` and a judged vantage screenshot; frame costs while dragging (`game_frame_costs`) | L |
| 5 | **AR layer: the editor's items, and hand editing** | handles, guides, port gizmos and the rubber band as AR items; drag on surfaces (walls, slabs, ceilings, machine boxes), insert and remove points, Free and snapping toggles, the point card, console verbs, introspection | an engine test drags a point along a wall through the dev IPC and reads it back on the wall's plane; the panel's controls are proven clickable (not only drawn); console verbs round-trip | L |
| 6 | **Materials in place, and features** | the material and size picker for every run (shape class changes at once); the feature rows; features slid along `s` or pinned to points; valve, tap, sprinkler, drip emitter, outlet, junction, cap; conduit nodes become tees and the service entrance | changing copper to PEX turns an elbow into a sweep (test); a fire under a sprinkler on a shut valve burns on, and is suppressed once the valve opens (seen failing with the valve shut); a drip emitter waters its crops only while its run carries water | L |
| 7 | **Anchors** | anchor rows placed, slid and removed; straps, clamps, hangers, J-hooks, trays; shared anchors; ramps on floors (vehicle crossing waits on the data gaps, 12) | support spacing reads anchors (a run with straps 1.5 m apart fails the dwelling profile's 1.37 m, passes at 1.3 m) | M |
| 8 | **Code checks and the two modes** | every rule kind evaluated by cell; the report with severities, words, sources and fix buttons; warnings in the AR layer; Settings > Gameplay > Pipes and cables | one test per rule kind from the findings' own numbers, each written to fail first; a Simplified run shows no Code findings and keeps its Faults | L |
| 9 | **Auto-route** | the frame's routing graph, A* with rule costs, previews while dragging from a port, "Auto-route the missing runs"; `route_conduit` and `systems::construction::routing` deleted, `routing_rules.ron`'s lanes moved into rules | the seed home's runs auto-route with no Fault and no Code finding in Realistic; a route never leaves a surface (a test that fails on a route through the air); the route budget measured | L |
| 10 | **Flexible physics** | the XPBD rod settle between holds, laid and shape points, kinks from the settled curvature, the reuse switch obeyed | a hose of 1.5 m between anchors 1 m apart settles to the analytic catenary within tolerance (seen failing with gravity off); identical shape on a reload; `just verify-runtime`; frame costs | L |
| 11 | **Network physics** | runtime voltage drop per island, brown-outs and overheating; water pressure and flow with fitting losses, drains by slope; data link quality with separation | hand-worked cases: a 30 m run of 14 AWG at 15 A drops what Ohm's law says; a level drain backs up; a data run beside power for 5 m degrades and a fibre one does not | L |
| 12 | **Damage** | leaks at joints, drip points, a leak over a live box faults it, cuts and crush split islands, repair by coupling or splice | tests for each, seen failing first | M |
| 13 | **AR layer: labels, pills and markers** | projected labels move to `gui::ar_labels`; `hud.rs` keeps the screen-fixed HUD; live sensor readings on run labels | the HUD's screenshots unchanged at the existing vantages; the label layout tests (`body_pill`) still pass | M |
| 14 | **Shared runs** | relay records per frame, put, remove and state messages, snapshots in parts, permits, no builder on the wire | relay feature tests (`just verify-relay`), including a "nobody is told who built what" test for runs; message sizes measured against the cap | L |
| 15 | **AR layer: X-ray, its own target, wide lines** | depth modes; the AR pass in its own premultiplied target composited last; anti-aliased wide strokes | a run behind a wall draws faint in X-ray and not at all in Occluded (vantage screenshots); the composite is pixel-identical to rung 2's sky where nothing else changed | M |
| 16 | **Hidden runs, access panels, sensing** | runs in chases behind wall panels, panel rows, sensing gear revealing runs by channel, the known set in the save, the plain "Show every run" toggle | a concealed run is not shown until revealed, and is shown after (test and screenshot) | L |
| 17 | **Powered doors on runs, and the emergency release** | door actuators as loads, controllers as data devices, `power_dependent` read live; release panels on both sides, the declutch and the crank, marking; airlock rules | cut a door's power run and the door stops; take off the panel, work the release and the crank, and it opens; it does not open against a pressure difference | M |
| 18 | **Taps and hacking** | network taps, power and water taps, `HackPanel` worked through a tap, `CutPower` as cutting the run | a door controller reached through a tap unlocks after the held action; fibre cannot be found by an RF scan | M |
| 19 | **Conduit and trays as runs that hold runs** | a conduit run that cables are pulled through, fill and pull limits | fill over 0.40 is a Code finding; a third 90-degree bend between pull points in data conduit is one | M |
| 20 | **Roads on the shared curve core** | `road_edge_centerline` moves to centripetal Catmull-Rom from `run_curve` | the road tests pass and the overshoot case no longer overshoots | S |
| 21 | **Alien craft** | a foreign ship's runs under its own profile and materials, unknown until sensed, boarding as find, open, tap, act | waits on an alien ship existing in the game | M |
| 22 | **AR glasses** | `ArView` from OpenXR, the AR target as a headset layer, headset planes as a surface source, the Real side's run planner | waits on hardware; a findings doc on the binding's licence first | L |

The first rungs already touch every point of the operator's direction: real material behaviour
(1, 3, 4), the material picker (6), free placement (5), functional length (1) and networks (6, 11),
features (6), anchors (7). The tapping rungs (16 to 18) build on all of them.

## 19. Open questions for the operator: ANSWERED 2026-11-02

Only the ones that are taste or direction; everything else above is decided, and reversible.
All three answered on 2026-11-02, each with the recommendation below:

- **1, the AR layer:** "Let's go with that. Could also be glasses, eye contacts, or even a brain
  implact. We'll start with the default always on until we get further along in development and
  things are running smoothly." So the wearer is a CATEGORY of gear (a visor, glasses, contact
  lenses, an implant: data rows, infinite-of-x), and until the game is further along the AR layer
  is simply on, with no gear needed; the gear rows and the "always on" switch come when the wearable
  does.
- **2, drinking water aboard:** "Let's go with this." Copper only as the ship profile's rule,
  shown as this ship's rule (`allowed_drinking_water_ship`, already so since rung 0).
- **3, powered doors on power loss:** "Sounds good. Let's do this." A per-door-type data row with
  real defaults, and a manual release on both sides of every powered door (rung 17).

The questions as they were put:

1. **Is the AR layer something the character wears?** A visor the character puts on (so it can be
   lost, damaged and upgraded, and its sensing channels are gear), or simply the game's HUD.
   *Recommendation:* a visor in the fiction, which is what justifies the constellation lines, with a
   plain "always on" setting so nothing depends on owning it (CLAUDE.md's gear-gated rule).
2. **Drinking water aboard: copper only, or anything a code allows?** Your rule (recorded in
   `conduits.rs`, v0.535) is copper for all potable water, "no PVC or other synthetics that leach
   toxins"; with materials now changeable, the plumbing codes read allow PEX for water distribution
   (R3 is a design guide for it). *Recommendation:* keep copper only as the ship profile's rule, shown
   as "this ship's rule" rather than as law; other profiles (a real US house on the Real side) follow
   their own code.
3. **Which way a powered door fails when its power goes** (locks.md's first open fork, needed at rung
   17): shut, open, or where it stands. *Recommendation:* a per-door-type data row with real
   defaults (a fire door shuts, D14; an exit stays where it is and opens by hand, D1, D5), and every
   powered door has a manual release on both sides whatever it does (D1 to D19), so a locked-shut
   default never traps anyone.

## 20. Risks

- **The frame and lib.rs are the merge funnel.** Rungs 1, 4 and 5 touch `home_meshes.rs`,
  `editor.rs` and `lib.rs`; each should move code out (into `engine::run_draw`, `engine::run_edit`)
  rather than add to `lib.rs`, and stage hunks by name in the shared checkout.
- **The crate split may cut crates while this is built.** The modules above are named for their
  crates, so a cut moves files, not designs; a rung that lands across a cut re-runs
  `node scripts/check-crate-layers.js`.
- **The rod solver is the riskiest budget.** It is a worker job with a fixed step count and a
  measured ceiling; if it cannot settle a long cable inside its budget, the segment length grows with
  the run's length (a coarser rod far from any point the person touched), measured, not guessed.
- **Rules without a primary source.** A row with no public source stays `ours` (the ship drain
  slope, elbow dimensions and rubber hose radii could not be reached on 2026-11-02, R53); the editor
  must never call such a row "code".
- **Checks that pass because they test the setup.** Every gate above is a measurement of the result
  (the settled shape, the delivered voltage, the screenshot), seen failing first.

## 21. References

- C. Yuksel, S. Schaefer, J. Keyser, "Parameterization and applications of Catmull-Rom curves",
  Computer-Aided Design, 2011 (centripetal Catmull-Rom has no cusps or self-intersections within a
  segment).
- W. Wang, B. Jüttler, D. Zheng, Y. Liu, "Computation of rotation minimizing frames", ACM
  Transactions on Graphics, 2008 (the double reflection method).
- M. Macklin, M. Müller, N. Chentanez, "XPBD: position-based simulation of compliant constrained
  dynamics", Motion in Games, 2016.
- T. Kugelstadt, E. Schömer, "Position and orientation based Cosserat rods", Symposium on Computer
  Animation, 2016.
- The findings: [pipe and cable routing rules](../reference/findings/2026-11-01-pipe-and-cable-routing-rules.md)
  and [manual door releases](../reference/findings/2026-11-01-manual-door-releases.md), both
  2026-11-01; the earlier [pipe marking standards](../reference/findings/2026-10-04-pipe-marking-standards.md).
