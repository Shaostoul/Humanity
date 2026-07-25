# Homestead: the fully-fledged player home

> Status: design, drafted 2026-07-25. This is the HOMESTEAD DESIGN ARC from
> docs/PRIORITIES.md (Active focus, overnight backlog item 10). Operator request:
> "We need to get walls properly placed, plumbing, electrical, decorations,
> furniture, etc. for all the different rooms of the house... a fully fledged
> homestead with walls, doors, windows, rooms, lighting, rugs, chairs, tables,
> desks, tools, machines, etc."
>
> Companions: `home-design.md` (one home, one surface), `homestead-solo-design.md`
> (the sizing math this reuses), `wall-corners.md` (deferred corner-seam bug),
> `construction-architecture.md`, `utility-wiring.md`, `self-sufficiency.md`,
> `two-realities.md`. Everything below is data-first per `infinite-of-x.md`.

## 1. Where the home stands today (the honest baseline)

The live home is `data/blueprints/ship_structure.ron` zone `"home"`: a fixed
55 x 89 x 3 m steel box with a glass roof (`shell_material: 1`, `roof_material: 4`),
connected to the `"commons"` zone by one corridor at lat 40 (its mouth cuts the
east shell near z 39..41). Inside the box:

- **Eight demo walls** (x 43..55, z 45..58), one per material id 1..8, each with a
  different door style (swing, slide, rotate, energy, nanowall, fixed). This is a
  materials-and-doors SHOWROOM, not a house. That is exactly what "walls properly
  placed" means: replace the showcase with a real room program.
- **Eight demo lights** and seven demo structures (elevator, ladder, ramp, stairs,
  two teleporters, train platform).
- **The machine layer works**: `data/machines/home.ron` / `home_solo.ron` place the
  whole power/water/food/waste plant as `MachineInstance`s (absolute x/z, zone
  "home"), connected by `MachineConnection`s with real cable/pipe specs, validated
  by `MachineHome::buildability_report` (src/machines.rs), simulated live by
  `ElectricalSystem` (PowerStatus), `PlumbingSystem` (WaterStatus), and
  `AtmosphereSystem` (AirStatus, registered in src/lib.rs).
- **Zero furniture.** No bed, chair, table, or rug exists in the walkable home.
  Furniture data exists in four disconnected stores, none of which renders here:
  `data/structures.csv` (read only by `src/gui/pages/homes.rs`),
  `data/blueprints/basic.ron` (BlueprintRegistry; ConstructionSystem is not
  registered and never ticks, per docs/FEATURES.md), `data/items.csv` (e.g.
  `rug_0`), and `data/ships/room_equipment.ron` (relay starter ship only).

Wall model (src/ship/home_structure.rs): `InteriorWall { a, b, height, material,
thickness, layers, openings }` between corner nodes snapped to a 5 cm grid;
`Opening { Door | Window, at, width, sill, height, style, locks, auto_open,
control_panel }`. Materials come from `data/blueprints/wall_materials.ron` (8 real
materials with real thickness defaults). The construction editor
(src/gui/pages/construction.rs, B key) edits all of it live and saves the RON.
Rooms are EMERGENT regions between walls; nothing names them yet.

## 2. Design shape: a house inside a greenhouse

The glass-roofed 55 x 89 bay IS the greenhouse: the garden arrays already grow
under its roof in open floor, which is what the self-sufficiency canopy math
requires (sun-lit canopy, no grow lights). So the homestead is a
**house-within-a-greenhouse**: a walled ~16 x 20 m dwelling block beside the
corridor mouth, with the garden, fields, heavy fabrication (sawmill, smelter),
and vehicle floor staying open-bay exactly where the machine files put them.
Nothing about the bay, the corridor, or the machine plant moves.

## 3. Room program

House block: x 39..55, z 24..44 (320 m^2), 3 m ceilings, southeast of the bay,
receiving the corridor aperture on the east shell at z 39..41. Ten rooms:

| Room | Rect (x1..x2, z1..z2) | Size m | Purpose |
|---|---|---|---|
| entry | 51..55, 36..44 | 4 x 8 | Mudroom; corridor aperture opens into it |
| common | 43..51, 36..44 | 8 x 8 | Living + dining, the social heart |
| kitchen | 39..43, 38..44 | 4 x 6 | Cooking (stove, oven, freezer, sink) |
| pantry | 39..43, 36..38 | 4 x 2 | Food storage (typed containers) |
| hall | 41..51, 34..36 | 10 x 2 | Spine to the private wing |
| bedroom | 39..44, 28..34 | 5 x 6 | Sleep, wardrobe, personal storage |
| bathroom | 44..47, 31..34 | 3 x 3 | Toilet, sink, shower |
| wetroom | 44..47, 28..31 | 3 x 3 | Laundry + mirror (appearance editor legacy) |
| study | 47..51, 28..34 | 4 x 6 | Desk work, books, planning |
| utility | 39..44, 24..28 | 5 x 4 | Batteries, purifier, water heater, breaker |
| workshop | 47..55, 24..28 (+51..55, 28..36) | ~48 | Benchwork, tools, 3D printer |

Adjacency (edges are doors): corridor > entry > common; common > kitchen >
pantry; common > hall; hall > bedroom, bathroom, wetroom, study; hall or
workshop > utility; entry > workshop; kitchen > bay (garden door); workshop >
bay; utility > bay. Exact rects are editor-tunable; the adjacency is the design.

Room identity: until real room detection lands (home_structure.rs header calls
room subdivision a later stage), place one `Zone` volume per room
(HomeStructure.zones) so each room has a wireframe label; add room-grade rows
(kitchen, bedroom, bath...) to `data/blueprints/zone_types.ron`, which is a pure
data edit. The purpose/actions vocabulary already exists in `data/rooms.ron` +
`data/rooms/room_actions.ron` and gets joined when detection arrives.

## 4. Walls, doors, windows

Every wall above is an `InteriorWall` row in ship_structure.ron zone "home";
every door and window is an `Opening` on its wall. All authorable today, in-app,
via the construction editor. Rules:

1. **Standard sizes.** Door 0.9 x 2.1 m, window 1.4 x 1.3 m with 0.9 m sill, the
   single-source dimensions block already in `data/blueprints/homestead_layout.ron`.
   Wider exceptions: entry-to-common 1.2 m; workshop bay door 2.4 m.
2. **Equal thickness at every shared corner.** The deferred corner bug
   (wall-corners.md) is a mismatched-THICKNESS seam. Sidestep it by design: set
   `thickness: Some(0.10)` on every partition regardless of material (oak
   partitions, concrete wet-walls, all 0.10). The seam fix stays deferred and
   this house never triggers it.
3. **Do not author walls flush with the corridor mouth** (ship_structure.ron
   header rule): the entry's east side is the shell; only the corridor's
   ShellCut opens it. The entry gets no authored east wall.
4. **Materials teach.** Partitions oak (3); bathroom/wetroom/utility walls
   concrete (2) at 0.10 thickness; one tempered-glass (4) half-wall between
   common and entry for sightline. Interior windows face the garden bay: west
   walls of kitchen/bedroom/utility and north walls of kitchen/common/entry get
   Window openings, because the "outdoors" here is the greenhouse.
5. **Door styles.** Swing everywhere; slide for bathroom/wetroom (3 x 3 rooms);
   keep locks off by default (locks.ron machinery exists when wanted).
6. **The showcase survives.** Move the eight demo material/door walls into the
   `"commons"` zone (it has four stub walls and floor to spare) as a permanent
   teaching exhibit, per the forever-development rule: never trim dev surfaces.

## 5. Systems per room

### 5.1 Electrical

The plant (4 solar, 2 battery banks, wind, generator) already exists in
home_solo.ron and feeds live `PowerStatus {generation, consumption, balance,
battery_wh, autonomy_hours}` (src/systems/electrical.rs), shown on the Home page
and checked by the buildability report (Power source / Energy balance / Wiring /
Conduits / Power circuit). The house adds CIRCUITS as `MachineConnection`s with
pinned specs from `data/utilities/conduits.ron`:

| Circuit | Cable | Serves | Real-world lesson |
|---|---|---|---|
| Lighting | cu_awg14 (15 A) | all room fixtures | lighting is the cheap circuit |
| Kitchen | cu_awg12 (20 A) | stove, oven, freezer | appliance workhorse circuit |
| Utility | cu_awg10 (30 A) | water heater | big resistive loads need gauge |
| General | cu_awg14 | study/bedroom/common small loads | |
| Workshop | cu_awg12 | workbench, 3D printer | |

Gap to close (code, small): `PlacedLight` has no wattage, so house lighting is
invisible to PowerStatus. Fix: add `watts` to `data/lighting/light_types.ron`
entries + sum switched-on placed lights into ElectricalSystem demand (the
grow-light meter in `MachineHome::grow_light_report` is the pattern). Outlets
and switches are not modeled; if wanted, an `wall_outlet` Furniture catalog
entry gives them a body and a port anchor, but they are cosmetic until then.

### 5.2 Plumbing

Live today: cistern > pump > purifier > `home_water_use` (one aggregate 80 L/day
node), per plumbing island, publishing `WaterStatus` (src/systems/plumbing.rs).
The house splits the aggregate into per-room fixtures (new catalog entries, all
following the existing Port pattern; PlumbingSystem already treats Water and
HotWater identically, src/machines.rs):

| Fixture (new machine id) | Room | Ports | Notes |
|---|---|---|---|
| kitchen_sink | kitchen | Water In, HotWater In | replaces part of home_water_use |
| bath_sink | bathroom | Water In, HotWater In | |
| shower | bathroom | Water In, HotWater In | the big hot-water draw |
| toilet | bathroom | Water In, Waste Out | Waste is already in the Utility enum |
| washer | wetroom | Water In, Electricity In | laundry |
| water_heater | utility | Electricity In, Water In, HotWater Out | **HotWater's first producer** |
| septic_tank | outside, near utility | Waste In | from the structures.csv reference row |

Pipes: `pex_half` branches, `copper_threequarter` trunk (both already in
conduits.ron). Grey water: sinks/shower/washer drain to the composter, whose
"+155 L/d greywater" reclaim already feeds irrigation in home.ron's loop math.
Black water: toilet > septic_tank (new machine; content today ends there, which
is honest). `Utility::HotWater` exists end-to-end in src/utilities.rs but has
zero producers or consumers today; this arc activates it with data only.

### 5.3 Air and heat

- **Air is live.** AtmosphereSystem ticks the home's sealed space and publishes
  the O2/CO2/pressure readout (Home page "Live air" card); the air_recycler is
  the priority-1 shed-last load. The house needs nothing new here.
- **Heat is not simulated yet.** `HvacSystem` (src/systems/hvac.rs, with
  RoomEnvironment/HvacUnit components) exists but is never registered, and
  `data/hvac.ron` (heat_pump, wood_stove, radiant_floor, thermostat...) is
  reference data with no machine-catalog counterpart. Defer: when heat becomes a
  loop, register HvacSystem and give utility + common rooms a heat_pump and
  wood_stove machine entry. Not a blocker for any increment below.

## 6. Furnishing manifest

Furniture becomes **machine-catalog entries, category "Furniture"** in
data/machines/home.ron + home_solo.ron. This is deliberate: the machine layer
already gives placement in the editor (Machines panel), persistence, walk-up
cards (src/gui/pages/hud.rs), typed-container storage (`container_type` >
`data/containers/types.csv`), auto-crafting hooks, and, since v0.734, a
`model: Option<String>` GLB field (docs/game/model-pipeline.md) with primitive
fallback. No new system is needed to make every item below real. Footprints
come from data/structures.csv; storage pieces get container rows in
data/containers/types.csv.

| Item (catalog id) | Rooms | Footprint m (W x D x H) | Storage | Asset |
|---|---|---|---|---|
| bed | bedroom | 2.0 x 1.0 x 0.6 | no | primitive now, GLB wanted |
| nightstand x2 | bedroom | 0.5 x 0.4 x 0.55 | container (small) | primitive |
| wardrobe | bedroom | 1.2 x 0.6 x 2.0 | container (clothing) | primitive |
| couch x2 | common | 2.0 x 0.9 x 0.8 | no | primitive |
| chair x6 | common, study, kitchen | 0.5 x 0.5 x 0.9 | no | primitive |
| dining_table | common | 2.0 x 1.0 x 0.75 | no | primitive |
| table (side) x2 | common, entry | 1.5 x 0.8 x 0.75 | no | primitive |
| bookshelf x3 | common, study | 1.0 x 0.35 x 2.0 | container (dry goods) | primitive |
| desk x3 | study, bedroom, workshop | 1.4 x 0.7 x 0.75 | container (drawer) | primitive |
| shelf x6 | pantry, wetroom, workshop | 1.0 x 0.3 x 1.8 | container | primitive |
| pantry_cabinet x2 | pantry | 2.0 x 0.6 x 2.0 | container (food_safe) | primitive |
| rug x3 | common, bedroom, entry | 3.0 x 2.0 x 0.02 | no | thin box primitive |
| mirror | wetroom, bathroom | 0.6 x 0.03 x 0.8 | no | emissive quad exists (Mirror wall kind) |
| toilet, bath_sink, shower | bathroom | per structures.csv | no | primitive |
| kitchen_sink + counter | kitchen | 0.6 x 0.5 x 0.85 | no | primitive |
| tool_rack x2 | workshop, utility | 1.5 x 0.2 x 1.0 | container (tools) | primitive |
| stove, oven | kitchen | already in home.ron catalog | no | primitive, live AutoRefine |
| freezer | kitchen | 1.0 x 0.7 x 1.8 | container (frozen food) | primitive |
| workbench | workshop | already in home.ron catalog | no | primitive, live AutoRefine |
| battery_bank x2, water_purifier, air_recycler, water_heater, home_server, network_uplink | utility | existing catalog entries relocated | varies | primitive |

Rugs note: a rug is a 2 cm box primitive today (no decal path exists); it works,
it is just chunky until a flat-quad render path or GLB replaces it.

Decorations: `data/entities/decorations.ron` is the scatter system (emptied
v0.911, reserved for ground structures); potted plants for the common room can
use the existing `assets/models/plants/potted_plant_01` GLTFs through it, or a
`potted_plant` Furniture entry with `model` set. Prefer the Furniture entry:
one pattern for everything indoors.

## 7. Lighting plan

Fixtures are `PlacedLight` rows referencing `data/lighting/light_types.ron`
(ceiling_panel, warm_lamp, cool_panel, spotlight, strip). Strips take a
multi-point `path` + `subdivision` (the v0.781/v0.792 string-light system); the
light budget is 256 sorted by influence (2048 with tiled lists), raised in
v0.911/v0.912 precisely so a string's every bulb lights its surroundings.

| Room | Fixtures |
|---|---|
| entry | 1 ceiling_panel; strip path along the corridor-side wall |
| common | 2 ceiling_panel; 2 warm_lamp (couch ends); 1 string-light strip looped above the dining table |
| kitchen | 1 ceiling_panel; 1 cool_panel over the counter; under-cabinet strip |
| pantry | 1 warm_lamp |
| hall | strip path down the ceiling centerline |
| bedroom | 1 ceiling_panel; 2 warm_lamp (nightstands) |
| bathroom | 1 cool_panel at the mirror |
| wetroom | 1 cool_panel |
| study | 1 ceiling_panel; 1 warm_lamp (desk) |
| utility | 1 cool_panel |
| workshop | 2 cool_panel; 1 spotlight aimed at the workbench |

Total ~20 placed lights: well inside budget even with the bay's existing lights.

## 8. Build order

Six increments, each shippable and verifiable alone. Verification for anything
visual: boot the release exe, drop `debug/screenshot_request.json`, read the PNG
(the v0.782 lesson: tests do not catch render reality).

1. **The shell (data only, one session).** Move the 8 showcase walls to the
   commons zone; author the Section 3+4 walls/doors/windows in ship_structure.ron
   zone "home"; set `spawn` to the entry; add per-room Zone labels (+ room-grade
   zone_types.ron rows). Verify by walking every doorway and screenshotting each
   room. Equal-thickness rule throughout.
2. **Light it (data only).** Section 7's PlacedLights, including the two strip
   paths. Screenshot each room lit with GI off.
3. **Water and power fixtures (data only).** Section 5.2's new catalog entries
   (kitchen_sink, bath_sink, shower, toilet, washer, water_heater, septic_tank)
   in home.ron + home_solo.ron; retire the aggregate home_water_use from the solo
   instance list; wire Section 5.1's circuits with pinned conduit specs; move the
   utility cluster (batteries, purifier, recycler, server, uplink) into the
   utility room. Done when the buildability panel is green and WaterStatus still
   balances at ~80 L/day.
4. **Furnish (data only).** Section 6's Furniture-category catalog entries +
   per-room instances; container rows in data/containers/types.csv for wardrobe/
   pantry/freezer/tool_rack. Done when each room screenshot matches its manifest
   and a Store/Take works on the wardrobe.
5. **Honest lighting power (small code).** watts on light_types.ron entries;
   switched-on PlacedLights join ElectricalSystem demand; the Home page energy
   ledger moves when you flip the roof lights on. Also register HvacSystem here
   if heat is wanted early.
6. **Real models (assets).** Export per-piece GLBs (assets/models/furniture.blend
   is unexported source; the pipeline is docs/game/model-pipeline.md and
   docs/dev/adding-3d-models.md), set each catalog entry's `model`. Rooms go from
   colored primitives to real furniture with zero data-shape changes.

The wall-corner seam fix (wall-corners.md) stays its own carefully-iterated
effort and blocks nothing above.

## 9. What does not exist yet (all verified against the tree)

1. Furniture rendering in the native home: nothing places or draws any of the
   four furniture data stores in the box home today.
2. GLB furniture assets: assets/models/ holds only plants + unexported .blend
   sources. Everything ships as primitives until increment 6.
3. HotWater producers/consumers: the utility exists, no machine uses it.
4. Per-fixture plumbing: one aggregate home_water_use node today.
5. Blackwater/waste chain: no septic machine; Waste utility has no consumers.
6. PlacedLight wattage: house lighting draws no power in the sim.
7. HVAC runtime: HvacSystem is written but never registered; hvac.ron has no
   loader into the machine layer.
8. Room detection/naming for emergent wall-bounded regions (Zone labels are the
   stopgap); rooms.ron's purposes/actions are not yet joined to the box home.
9. Rug/floor-covering flat render path (thin box works, looks chunky).
10. ConstructionSystem (blueprint build queue over basic.ron) is not registered;
    this design does not depend on it.
11. Outlets/switches as physical objects.
12. The corner-seam fix for mismatched wall thickness (deferred by design;
    Section 4 rule 2 avoids it entirely).

## 10. See also

- `docs/design/home-design.md`: one home, one surface, AI == player parity.
- `docs/design/homestead-solo-design.md`: every machine count and Wh/day figure.
- `docs/design/construction-architecture.md`: the five-layer long-term pipeline.
- `docs/design/utility-wiring.md` + `data/utilities/conduits.ron`: cables/pipes.
- `docs/design/wall-corners.md`: the deferred seam, and why rule 4.2 exists.
- `docs/design/two-realities.md`: the same manifest doubles as the REAL-side
  possessions template (a real home's rooms, fixtures, and storage), which is
  why fixtures carry real footprints and real circuit sizes.
