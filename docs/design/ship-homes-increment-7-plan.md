# Increment 7 build plan: stacked homes and decks

Written 2026-10-20 from four read-only surveys of the code (geometry and drawing, movement,
the shared world and its rules, building aboard and history) after the operator's "Sounds like a
plan, let's proceed" to the session's recommendation: give the homestead an upper floor, so its
stairs, ladder and elevator lead somewhere, and make it the gate the design names for any
multi-storey cabin block or apartment (docs/design/ship-homes-and-logistics.md, "Increment 7").
The decisions below were made by the orchestrator (section 4 lists them so the operator can
overrule any).

## 0. In short

A home can have more than one storey. The homestead gets the first floor its own plan already
asked for (docs/design/player-home.md, "What a second storey needs"): the quiet wing (bedroom,
dressing room, bathroom, wet room, study) goes up over the house's north half, reached by a stair
off the hall's north side and by the elevator in the service way at (31, 26), "the shaft a second
storey would land in"; the ground floor it frees becomes a dining room and a second bedroom. Walls
upstairs stop a body upstairs only, doors open for the storey they are on, the floor between is a
real slab you stand on and cannot jump through, lights stay on their own storey, the sun does not
shine through a floor, and the elevator rides between storeys. Proven in the running game by a rig
that walks up the stairs into the upstairs bedroom.

## 1. What the code assumes today (from the surveys, file:line)

- **The body is one box.** `HomeStructure` is width x depth x `height` (src/ship/home_structure.rs
  232-236); the homestead's `height` is 3.0 (data/homes/homestead.ron:63), its glass roof. Mesh
  generation draws ONE floor quad at y 0 and ONE ceiling at `height` (home_structure.rs 774-776).
- **Walls have no base.** `InteriorWall` is two (x, z) ends and a height from the floor (168-194);
  openings' `sill` is from the zone floor (86); `ShellCut` has no base (67-72). Walls, sills,
  headers, mitres (832-864), corner columns (881-904, 1812-1824) are all built from y 0.
- **Room detection is 2-D.** `detect_rooms` (1357-1476) is one flood fill, every wall blocking at
  every height, each room 0..`height`; it feeds DDGI, the lamps, footing, the HUD and the editor.
  `name_rooms_from_zones` (1488-1531) contains by x/z.
- **Collision is one plane.** `WallSegment` has no y (src/ship/wall_collision.rs 1-11, 19-25,
  127-129); door collision ignores y (src/lib.rs 5287-5307); doors auto-open by horizontal distance
  (src/ship/door_panels.rs 124-137, src/systems/door_anim.rs 97-110).
- **Footing takes the first room by x/z** (src/engine/footing.rs 46-49) though stairs, ramps and
  the existing `deck` piece already step by height (`STEP_UP_M` from the floor stood on).
- **The elevator has two stops 3 m apart** (footing.rs 24-30, lib.rs 3617-3678 with `_h` unused,
  the car drawn `anim * 3.0` at lib.rs 7913); only a person at the base can call it.
- **Teleports set x and z only** (src/engine/move_check.rs 137-149; `transit_link_at`
  src/ship/transit.rs 85-89 has no y). Every placement (`put_player_at`, `stand_player_at`,
  src/engine/home_plot.rs 1413-1528) re-seats the floor from the old floor. The editor close
  stands you at the zone floor + 1.7 (lib.rs 6990-7027) and the relay insists on it
  (`editor_off_floor`, src/relay/handlers/move_check.rs 110-115, 228).
- **No head clamp:** a jump puts the head about 3.1 m up, through a slab (src/renderer/camera.rs
  1488-1519). The ladder is chosen by x/z (lib.rs 3707-3739); Shift near a ladder top descends.
- **Nothing aboard stands on a built piece** (footing reads rooms and ship structures only; the
  planet has `planet_build::built_floor_radius`).
- **Plots are 3 m tall** (data/blueprints/ship_structure.ron 1173..1327, all twelve homestead
  plots 55 x 3 x 89); `design_fits_plot` refuses a taller home (ship_structure.rs 924-933) and
  neighbours stop drawing it (src/ship/neighbours.rs 149-151). A neighbour's shell bake drops zones
  (home_structure.rs 1183-1228).
- **Light crosses storeys.** Point, spot and strip lights have no occlusion (the light loop in
  assets/shaders/pbr/80-fragment-shared.wgsl 274-345), and the home is not in the sun shadow map, so
  sun through the glass roof lights under any slab. Opaque faces are back-face culled and
  `floor_quad` faces up only (renderer/pipeline.rs 2114). DDGI: `room_box` takes the glass roof as
  every room's lid (src/engine/room_gi.rs 97-152); the probe budget is nearly full (31,874 of
  `MAX_TOTAL_PROBES` 32,768, src/renderer/room_probes.rs 92-95), and an over-budget ship coarsens
  every room.
- **The ship hash** is FNV over the ship file (ship_structure.rs 1379-1382, 1961-1968): growing the
  plots changes it (older games are refused as "a different ship"; the door-points rig fixture
  must be regenerated); a new field on `HomeStructure`, `ShipZone` or `Plot` changes it for every
  ship unless it is skipped when empty.
- **Already right:** zone `origin.y`, placed structures' and lights' y, the HUD's and the relay's
  3-D room checks, the relay's vertical speed allowance, built-piece collision banding
  (build_place.rs 750-779), storey-aware build resting (`STOREY_STEP_M`), the falls code, BUG-207
  substeps and BUG-208 step-down, 3-D aboard and air boxes, remote players' y.

## 2. The shape (Wave 0's contract)

- **`HomeStructure.floors: Vec<FloorSlab>`**, serialized only when not empty. A slab is a box:
  `a: (x, z)`, `b: (x, z)` (corners), `top: f32` (its walking surface, body-local y), `thickness`
  (default 0.25 m), optional `label`. A storey's floor is one or more slabs; a stairwell or an
  elevator shaft is the gap between them, so no hole-cutting is needed. A slab is a floor to stand
  on, a ceiling for the room under it, a lid for that room's air and light, and it blocks the sun.
- **`InteriorWall.base: f32`**, default 0, serialized only when not 0: the wall rises from `base` to
  `base + height`. An opening's `sill` is measured from the wall's base. `ShellCut` gains the same.
- **Storeys are derived, not declared:** the bands between 0, each distinct slab top, and the
  body's `height`. A wall belongs to the band its base is in; a zone to the band its origin y is
  in. `HomeStructure::storeys()` returns them; `detect_rooms` floods each band with only that
  band's walls and gives each room its band's floor and height.
- **`WallSegment` gains `y0`, `y1`** (world y), with full height the default, so every consumer
  compiles unchanged until it reads them.
- **Hash stability:** a test that every shipped ship and home file serializes, and hashes,
  byte-for-byte as before when it uses none of the new fields.

## 3. The design, by lane

- **Geometry (G).** Slabs drawn as boxes (top, underside and edges, so visible from below) in the
  FLOORS buffers, never the ceilings (which the roof toggle hides). Walls, sills, headers, mitres,
  clips and corner columns per band. Door panels, the control panel and locks from the wall's
  base. The neighbour bake keeps slabs. Air: `air_top` counts slabs and upper zones; the home's air
  volume is the sum of its rooms per storey (the pinned 14,685 m3 test changes on purpose and says
  why). Conduits per storey.
- **Movement (M).** Footing: the highest floor (room floor, slab top, structure surface, or a
  finished built piece's top) at or below the feet plus the step-up, found by height. Collision:
  a wall stops a body only where the wall's y0..y1 overlaps the body's knee-to-eye band (the
  built-piece rule). Doors collide and auto-open only for a body on their storey. A jump stops
  under the slab above (head clamp at the eye plus a head allowance). The ladder is chosen by
  height; Shift at a ladder top on a floor walks (sprints), and only Shift while on the ladder
  descends. The elevator's stops are the storey floors its shaft spans (derived from the slabs and
  the shaft's height), callable from any landing; riding, Space goes up a stop and Shift down. A
  teleport sets y to the far pad's floor and re-seats the floor there; pads are found by height.
  Every placement re-seats the floor from where it put the feet. The editor close stands you on
  the storey you left from.
- **Light (L).** Direct lights reach only fragments on their own storey band (plus rooms that span
  bands, the greenhouse); slabs cast into the sun shadow map; DDGI room boxes per storey with a slab
  as an opaque lid and a slab as the upper room's floor; the probe budget raised or the acre's
  spacing set so a second storey does not coarsen every room (perf-expert measures first). One
  builder: this is the hot shader.
- **Rules (R).** The twelve homestead plots grow to 6.0 m (two 3.0 m storeys; the cabin plots stay);
  the door-points fixture is regenerated; the relay accepts an editor close at any floor inside
  the plot box (feet between the plot floor and its top less a storey); a serving counter or store
  is in reach only on the same storey (a vertical limit of a storey, not the 5 m sphere); the
  quests' volumes and front door keep y; relay transit ends keep y.
- **Content (C1).** A dated finding first: stair rise and going, pitch, headroom, width, handrails,
  guards (height and where required), and ceiling heights, from the primary sources (the US IRC,
  England's Approved Document K, and OSHA where it applies), quoted and dated. Then the homestead's
  upper floor as data (both copies, homestead.ron and shipped/homestead.ron), per player-home.md:
  slabs at 3.0 m over the north half with the stairwell and elevator shaft open, the quiet wing's
  zones moved up (machines and furniture with them), walls with `base: 3.0`, a stair off the hall's
  north side, guards wherever a floor edge drops more than the finding says, the dining room and
  second bedroom downstairs, the body `height` 6.0 so the greenhouse gets its height back. The
  stairs type follows the finding (if the 0.214 m riser is over a limit, more risers).
- **Editor (C2).** GUI-first: the B editor shows one storey at a time (the ones above it hidden),
  places walls, openings, lights and pieces on the chosen storey, and adds, resizes and removes
  floor slabs; the floor pick takes the chosen storey's floor.
- **Rig (V).** `just verify-storeys`: from the front door to the hall stair, up, into the upstairs
  bedroom (the HUD names it), a wall upstairs stops the body there, the ground floor's walls below
  do not, the elevator rides up and down and is called from the upper landing, a jump under the
  slab does not go through, a guarded edge cannot be walked off, and screenshots up and down.

## 4. Decisions made for the operator (overrule any)

1. Storeys are derived from floor slabs, not a declared list: less to keep in step, and a
   stairwell is a gap, not a hole.
2. A storey is 3.0 m floor to floor, the slab 0.25 m (ceiling below 2.75 m, above England's 2.3 m
   and the IRC's 7 ft; the finding confirms or corrects).
3. The homestead's body becomes 6.0 m and its plots 6.0 m: the greenhouse gets its 6 m back, and
   the ship hash changes (older games are refused until updated; nobody is using it yet).
4. The upper floor's layout is player-home.md's target plan; the greenhouse mezzanine and the
   vehicle-bay gantry come after this increment.
5. In the elevator, Space and Shift choose the floor while riding.
6. Every ground-floor room except the greenhouse keeps a 3 m room under a slab at 3.0 (a slab
   nobody walks on is that room's ceiling); only the greenhouse rises to 6 m. Lidding them adds
   about 830 bounced-light probes; 6 m rooms everywhere would add about 16,700 (the light-budget
   analysis, below). **Revised the same day:** the lids over rooms NOT under the upstairs are
   GLASS (`FloorSlab.material: Glass`), so those rooms keep exactly today's look and daylight; the
   rooms under the upstairs get its solid floor. Put to the operator as three options (rise to 6 m;
   3 m under glass; 3 m under a solid ceiling) with glass recommended; the layout builder made the
   same recommendation on its own (the solar array, fields and orchard court need the sun).
7. A household's water is used per person and SHARED by its fixtures: a second shower room does not
   add a second day's draw (the layout's downstairs shower room would otherwise add about 162 L a
   day to the meter for the same people). Real use follows the people, not the number of taps.
8. A guard is a real piece, not a wall: posts, balusters at the finding's spacing and a top rail at
   its height; and stairs get handrails. Both are new structure-drawing work (lane G), and until
   then the layout's guards are 1.0 m walls with base 3.0.
9. The elevator's landing door is interlocked (it opens only with the car at that landing) and the
   ladder hatch has a self-closing gate: otherwise each is a walk-in 3 m drop (lane M).
10. The falls rig's aboard walk-off-the-top leg moves to a deck the rig places for itself: the
    stairs now lead to a floor, and no unguarded 3 m edge remains in the home by design.
    **As built (2026-10-21):** the rig places a `deck` piece with its top at the upper floor's
    height in the middle of the largest room open to the roof (the greenhouse), through the build
    editor's console line `add_structure deck ...` sent by a new showcase verb, `console`, which
    keeps the change to that session (it never arms the Dev mode's autosave into the data files),
    and both walk legs step off it. The ladder leg now climbs through the hatch and steps onto the
    upper floor across the gate (no fall, no wound); the stairs-down leg walks the 16 risers.
    scripts/verify-falls.js's header holds the red and green runs. The rig's door points report
    the walls by storey too (src/ship/door_points.rs `StoreyWalls`), so the upper hall's south wall
    no longer crosses the ground hall in verify-copresence's map.

## 7. The layout and the finding (C1, 2026-10-20)

`docs/reference/findings/2026-10-20-stairs-guards-and-headroom.md` (the IRC 2021 through
Washington's adoption and the Stairway Manufacturers' Association's licensed copy; Approved
Document K 2013; ICC's own reader refused the script with a captcha, and the 2024 IRC's R318 text
was not readable) and `docs/design/homestead-upper-floor.md` (every coordinate checked by a
script). The present stairs break the IRC's 196 mm riser (214.3 mm), have no nosing, handrails or
guards and no top landing; a 3.0 m storey takes 16 risers of 187.5 mm on 281.25 mm treads
(33.7 degrees). The layout: 19 slabs at 3.0 leaving the stairwell, the elevator shaft, a ladder
hatch and the greenhouse open; the quiet wing's five rooms keep their ids and rectangles and rise
3.0 m; an upper hall, a back corridor and a lift lobby; stairs-1 moves to the hall's north wall
(type (1.2, 3.0, 4.5), 16 steps, headroom 2.229 m); downstairs a dining room, a second bedroom
(room types `dining`, `guest_bedroom`), a shower room and a boot room; ladder-1 beside the elevator
into the lift lobby; ramp-1 becomes train-1's 1:17 boarding ramp (the old 1:3.75 ramp's "wheelchair
friendly" note was false). Moving stairs-1 and ladder-1 breaks verify-falls' walk-off-the-top leg
and `src/engine/survival_env.rs`'s pinned top positions (both listed in the layout document).

## 6. The light-budget analysis (perf-expert, 2026-10-20, read-only)

- **Lamps through a floor:** each slab is an exact occluder of the line from a fragment to a lamp.
  Slab boxes go in the room GI table every pipeline already binds (group 0 binding 7) as rows after
  the rooms; each light lists up to 3 slabs its range sphere touches (computed on the CPU, packed in
  the unused `cone_inner.yzw`); frag_tail tests the mid-plane crossing and the rectangle after the
  range reject. About +2 to +4 ps per in-range light with a slab listed, and a saving for every
  light it blocks; views with no slab are bit-identical. One switch, `HAS_SLAB_OCCLUSION`, in the
  Surface and Vegetation entries (the `HAS_ROOM_GI` set). No bind-group change. A per-light height
  band was rejected: wrong at the stairwell, exactly where the rig walks.
- **The sun through a floor:** the same slab rows and crossing test toward the sun, applied even
  with sun shadows off (a floor is not a quality option), at the sun term and the four in-entry
  `sun_shadow` sites (so leaf transmission is blocked too, the BUG-060 rule). About 0.02 to
  0.05 ms at 1600x900 on the RTX 4070. The far shadow map cannot do it (1.9 to 8 m bias, 0.73 m
  texels); the near cascades are not built yet and stay the plan for walls.
- **The probe budget premise was stale:** the ship already holds 41 rooms and 26,804 probes at
  1.33 m (coarsened since 2026-10-05; the 31,874 figure is from 2026-10-03). Fix, first: priority
  in `lattice_for`, the player's own home zone keeps 1 m and other zones coarsen first (free).
  Later: the depth moments in their own Rg16Float texture (-38% memory, a layout change and a
  boot verify), and the nearest rooms updated first.
- **Measurements still owed** (one rig at a time): a frame-cost baseline
  (`HUMANITY_FRAME_COSTS=1 node scripts/probe-sweep.js --only
  console-face-6,console-face-6-room-gi-off,home-overview-noon,home-racks-noon`) before and after
  lane L, and a same-boot A/B of the slab term once a `{"slab_occlusion":"0"}` showcase pin exists.

## 5. Waves

- **Wave 0 (one builder, merges first): the contract.** Section 2, with nothing visible changed.
- **Wave 1, in parallel on disjoint files:** G (home_structure.rs mesh code, door_panels.rs,
  neighbours.rs, ship_space.rs, home_meshes.rs conduits), M (footing.rs, wall_collision.rs,
  camera.rs, move_check.rs, transit.rs, home_plot.rs placements, and src/lib.rs, which M alone
  edits; other lanes hand lib.rs changes to the orchestrator), L (renderer and shaders,
  room_gi.rs, room_probes.rs), R (relay files, ship_structure.ron, quests, the fixture), C1 (the
  finding, then the two homestead files and structure_types.ron), C2 (editor.rs,
  construction.rs).
- **Wave 2:** the rig (V), then a critic review of the merged wave before release.

Each lane's new tests are seen red first, with the red output recorded in the test's comment.
