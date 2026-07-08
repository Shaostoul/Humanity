# Ship superstructure: from one home box to a mothership

> Drafted 2026-07-08 from the operator's build-editor screenshot and the
> question "how do we build the mothership superstructure - we have no way of
> generating hallways/rooms outside the player home." Companion to
> decision-briefs.md Brief 1 (vehicle bay zones - this doc absorbs it),
> cosmos-architecture.md (the Ship container model), gameplay-loop-map.md T5.

## The insight: the ship is a cluster of home boxes

The HomeStructure model that already works (a fixed outer box + freely drawn
interior walls + per-structure shell/roof materials + placed lights + spawn
point + wall collision + door/window openings) is the right primitive for
EVERY pressurized space on the ship, including the communal mall. The roof
material already defaults to GLASS. What the system lacks is not a better
primitive - it is plurality and connection:

1. There is exactly ONE structure (the home). No second box can exist.
2. Boxes have no world origin of their own (the home sits at an implicit spot).
3. Nothing connects boxes (no corridors), so separate boxes would be
   disconnected islands you EVA between.
4. Nothing wraps the cluster in a hull, so from outside it reads as floating
   crates, not a ship.

## Increment ladder (each shippable alone)

### A. ShipStructure: many zones (the foundation, direction-independent)

A ship is a list of ZONES in one data file. Each zone carries an id, a label,
a purpose tag, a world origin offset, and the ENTIRE existing HomeStructure
body unchanged (box dims, walls, openings, materials, lights, spawn):

```ron
(
    zones: [
        (id: "home",    label: "Player Home",  purpose: "residence", origin: (0.0, 0.0, 0.0),   body: (...existing home_structure fields...)),
        (id: "commons", label: "The Commons",  purpose: "commons",   origin: (70.0, 0.0, 0.0),  body: (...)),
        (id: "bay",     label: "Vehicle Bay",  purpose: "bay",       origin: (0.0, 0.0, 100.0), body: (...)),
    ],
    corridors: [ ... see B ... ],
)
```

- The current home becomes zone 0; home_structure.ron migrates outright into
  the new file (no-compat-debt, pre-launch).
- The build editor gains a zone selector (dropdown or click a zone in the
  viewport); every existing tool (wall drawing, corner pins, openings,
  materials, lights) operates on the selected zone unchanged.
- Meshes, wall collision, room bounds, and machine placement iterate zones.
  Machine instances gain an optional zone id (default "home").
- Purpose tags are data the GUI and sims read: "residence", "commons", "bay",
  "corridor", "agriculture". Brief 1's vehicle bay = a zone with purpose
  "bay" and a big door; the brief retires into this doc.

### B. Corridors: generated connections

A corridor row references two zones' door openings and generates a straight
tube between them (floor, two walls, ceiling; glass ceiling optional):

```ron
(from_zone: "home", from_opening: 2, to_zone: "commons", to_opening: 0, width: 3.0, glass_top: true)
```

- Generation: both openings' world positions are known; extrude a box between
  them, cut the openings, add collision. No hand-drawn walls per hallway.
- v1 corridors are straight and axis-aligned (zones placed to suit); L-bends
  are a follow-up (two segments + an elbow).
- Airlock flag later for EVA-rated separations; today every corridor is
  pressurized and the atmosphere volume is shared.

### C. The Commons (authoring, not new tech)

The operator's target: "a shopping mall with a garden in the center with a
giant glass ceiling." After A + B this is a DATA task:

- One big zone (e.g. 34 x 55 x 8 m) with purpose "commons" and roof glass.
- Garden center: the existing grow-area machines (fields, towers, beds)
  placed in the middle - they already work anywhere machines place.
- Shops: trading_post machines (the vendor modal already works) along the
  walls; market stalls are machine instances.
- The AtmosphereSystem's sealed-space model extends from "the home" to "each
  zone + connected corridors" (v1 can keep ONE shared air volume for the
  whole pressurized cluster - honest enough until airlocks land).

### D. The hull wrap (what makes it LOOK like a ship)

A generated shell around the zone cluster so the exterior reads as a vessel:

- v1: an extruded data-driven profile (nose / mid / engine silhouette scaled
  to the cluster's bounding volume) rendered as hull-material geometry
  around the zones, with cutouts where glass roofs need starlight. Greeble
  blocks (engines, radiators, comm masts) as data rows.
- The fibonacci spiral aesthetic (the original homestead vision) can be one
  hull profile among several; profiles are data files.
- Purely visual: no collision change outside, interiors unchanged.

### E. Later, explicitly out of scope now

Multi-deck Y stacking (zones already carry a Y origin; the editor UX is the
work), NPC pathing across the corridor graph (src/ship BFS exists), per-zone
atmosphere with airlocks, ship-in-ship (cosmos container model already
nests), hull damage.

## Sim integration notes

- Power/water: machines already join electrical/water islands by connection;
  zones change nothing (connections can cross zones through corridors).
- The relay-side Pioneer frigate stays separate until the multiplayer ship
  arc; this doc is the LOCAL mothership the player walks.
- Persistence: zones save exactly like the home does today (same serde body).

## Recommendation

Start with A (multi-zone), because every imaginable superstructure direction
(boxy zones, freeform vector hulls, voxel hulls) needs "more than one
enclosed space" first, and A is pure generalization of shipped, tested code.
B (corridors) immediately after - together they unlock C as an authoring
session. D is the visual payoff pass and the right place for operator taste
iteration (hull profiles as data = screenshot-driven tuning like the
planets).
