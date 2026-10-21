# The homestead's upper floor (layout draft for ship homes increment 7)

Written 2026-10-20 as the Content (C1) lane's layout draft for
[ship-homes-increment-7-plan.md](ship-homes-increment-7-plan.md). It changes no data:
the homestead files wait for Wave 0's new fields (`HomeStructure.floors`,
`InteriorWall.base`). It places everything the plan's C1 paragraph asks for, so the
builder copies numbers instead of designing. Every coordinate is in the home's
body-local metres (x east, z south, y up, from the box's min corner), the same frame
as [data/homes/homestead.ron](../../data/homes/homestead.ron).

It rests on three things:

- **The dated finding**
  [2026-10-20-stairs-guards-and-headroom.md](../reference/findings/2026-10-20-stairs-guards-and-headroom.md),
  which reads the 2021 International Residential Code and England's Approved
  Document K and takes the stricter rule wherever they differ: a riser of at most
  196 mm (7.72 in), a tread of at least 279 mm (11 in) for steps drawn with no
  nosing, 2032 mm (6 ft 8 in) of headroom, a guard wherever a walkable edge drops
  more than 600 mm (23.6 in), guards at least 914 mm (36 in) tall (this draft uses
  1.0 m, 39.4 in), handrails 900 to 965 mm (35.4 to 38 in) above the nosing line on
  both sides of a stair 1.0 m (39.4 in) or wider, and ceilings of at least 2134 mm
  (7 ft) in habitable rooms.
- **The target plan** in [player-home.md](player-home.md), "What a second storey
  needs": the quiet wing (bedroom, dressing room, bathroom, wet room, study) goes up
  over the house's north half, reached by a stair off the hall's north side where
  the bedroom door is now, and by the elevator at (31, 26); the freed 168 m2
  (1,808 sq ft) downstairs becomes a dining room and a second bedroom.
- **The orchestrator's lid rule (2026-10-20, from the light-budget analysis):** the
  body goes to 6.0 m (19 ft 8 in), and every ground-floor room except the
  greenhouse keeps a 3 m room under a floor slab at top 3.0. A slab nobody walks on
  is that room's ceiling, and it confines the room's light and bounced-light probes:
  about 830 more probes, where 6 m rooms everywhere would add about 16,700.

## 0. In short

- **One slab layer at top 3.0 m (9 ft 10 in), 0.25 m (10 in) thick,** covers the
  whole acre except the greenhouse and three openings: the stairwell, the elevator
  shaft and a ladder hatch. Nineteen rectangles (section 1). The ceiling below is
  2.75 m (9 ft 0 in), the upper storey's 3.0 m, to the glass shell roof.
- **Upstairs, eight rooms.** The five quiet-wing rooms keep their exact x/z
  rectangles and ids and rise 3.0 m; a 2.5 m (8 ft 2 in) upper hall stands over the
  ground hall's north half; a back corridor on the workshop's roof and a lift lobby
  over the service way connect the elevator and the ladder (section 2). Every room is
  reachable from the stair top.
- **The stair is stairs-1, moved out of the vehicle bay** and re-specified to the
  finding: 16 risers of 187.5 mm (7.38 in) on 281.25 mm (11.07 in) treads, 33.7
  degrees, 1.2 m (3 ft 11 in) wide, rising west along the hall's north wall from
  x 41.0 to x 36.5, enclosed by walls on both sides. Its headroom is at least 2.229 m
  (7 ft 3.8 in), 197 mm (7.8 in) over the IRC (section 3).
- **Downstairs, the freed band becomes four rooms:** a dining room, a second
  bedroom, a shower room (the entrance-storey WC) and a boot room off the entry
  (section 4).
- **The roof over the house's south half, and everywhere else, is inaccessible**
  (section 6). Nothing leads onto it.
- **Five things do not balance and need a decision or code** (section 9). The
  biggest: the lid rule puts slabs over the solar array, the fields and the
  orchard court, the three rooms whose loops run on sunlight through the glass roof.

## 1. The slab layer

Each slab is a `FloorSlab` (plan section 2): corners `a` and `b`, `top: 3.0`,
`thickness: 0.25`. They tile the acre exactly, with no overlaps, apart from four
openings (checked by script, section 10).

| # | label | a (x, z) | b (x, z) | under it | material (recommendation, section 8) |
|---|---|---|---|---|---|
| 1 | Power terrace roof | (0, 0) | (20, 22) | power terrace | glass |
| 2 | Vehicle bay roof | (20, 0) | (55, 22) | vehicle bay | steel |
| 3 | Service way roof | (0, 22) | (30.1, 30) | service way, west part | steel |
| 4 | Service way roof, north of the shaft | (30.1, 22) | (34, 25.1) | service way | steel |
| 5 | Service way roof, east of the shaft | (31.9, 25.1) | (34, 26.9) | service way | steel |
| 6 | Lift lobby floor | (30.1, 26.9) | (32.5, 30) | service way | steel |
| 7 | Lift lobby floor, by the hatch | (32.5, 28.1) | (34, 30) | service way | steel |
| 8 | Orchard court roof | (34, 22) | (55, 30) | orchard court | glass |
| 9 | Plant room roof | (0, 30) | (12, 51) | plant room | concrete |
| 10 | Forge roof | (12, 30) | (20, 51) | forge | concrete |
| 11 | Workshop roof | (20, 30) | (34, 51) | workshop (and the back corridor's floor) | steel |
| 12 | Upper floor, quiet wing | (34, 30) | (55, 38) | dining room, second bedroom, shower room, boot room | oak |
| 13 | Upper floor, stair-top landing | (34, 38) | (36.5, 39.3) | hall, west end | oak |
| 14 | Upper floor, hall east of the stairwell | (40.5, 38) | (55, 39.3) | hall, entry | oak |
| 15 | Upper floor, hall, and the house's south roof | (34, 39.3) | (55, 51) | hall, entry, pantry, kitchen, great room, console room | oak |
| 16 | Mushroom room roof | (0, 51) | (10, 61) | mushroom room | concrete |
| 17 | Aquaponics roof | (0, 61) | (10, 73) | aquaponics | concrete |
| 18 | Fields roof | (0, 73) | (40, 89) | fields | glass |
| 19 | Barn roof | (40, 73) | (55, 89) | barn | steel |

The four openings (gaps, not slabs):

| opening | x | z | why |
|---|---|---|---|
| Stairwell | 36.5 to 40.5 | 38.0 to 39.3 | the flight below rises through it (section 3) |
| Elevator shaft | 30.1 to 31.9 | 25.1 to 26.9 | exactly elevator-1's footprint, so the car's edge meets the slab with no gap a body could fall through |
| Ladder hatch | 32.5 to 34.0 | 26.9 to 28.1 | ladder-1 climbs through it into the lift lobby (section 5) |
| Greenhouse | 10 to 55 | 51 to 73 | the one 6 m room (player-home.md: "The greenhouse gets its height back") |

Slab area 3,894.76 m2 (41,923 sq ft), openings 1,000.24 m2; together exactly the
acre's 4,895 m2.

The ground walls stay 3.0 m tall and run up into the slab's thickness; the slab's
underside is the ground rooms' ceiling at 2.75 m. Every ground door (at most 2.4 m)
and window head (at most 2.5 m) is under it.

## 2. Upstairs

### Zones (origin y 3.0, height 3.0)

| id | label | origin (x, y, z) | size (x, y, z) | type_id | room_type |
|---|---|---|---|---|---|
| room-bedroom | Bedroom | (34, 3, 30) | (8, 3, 8) | room_bedroom | bedroom |
| room-dressing | Dressing room | (42, 3, 30) | (3, 3, 8) | room_dressing | dressing |
| room-bathroom | Bathroom | (45, 3, 30) | (5, 3, 4) | room_bathroom | bathroom |
| room-wetroom | Wet room | (45, 3, 34) | (5, 3, 4) | room_wetroom | wetroom |
| room-study | Study | (50, 3, 30) | (5, 3, 8) | room_study | study |
| room-upper-hall | Upper hall | (34, 3, 38) | (21, 3, 2.5) | room_hall | hall |
| room-back-corridor | Back corridor | (32, 3, 30) | (2, 3, 10.5) | room_hall | hall |
| room-lift-lobby | Lift lobby | (30, 3, 26.9) | (4, 3, 3.1) | room_hall | hall |

The five quiet-wing rooms keep their ids, rectangles and room types: only `origin.y`
changes, so every machine's `room` field, the mirror (bedroom) and the wardrobe
(dressing room) walk-ups, and the "one appearance room, one wardrobe room" test
keep working. No two upstairs rooms overlap. Everything upstairs stands on the
house's north half (z 30 to 40.5, the house's midline) except the back corridor and
the lift lobby, which exist only because the elevator stands outside the house, 3 m
west and 4 m north of its corner.

Each upstairs room is a rectangle on purpose: a room is named by the zone holding
its bounding-box centre (`name_rooms_from_zones`), so an L-shaped hall could take
another room's name.

### Walls (base 3.0, height 3.0, to the shell roof)

`at` is the opening's left edge in metres from corner `a`, as in the ground file.
Doors are 2.1 m swing doors at 0.9 m wide, except where stated.

| id | a | b | material | openings |
|---|---|---|---|---|
| U1 north (exterior) | (34, 30) | (55, 30) | oak | windows at 1.5 and 6.0 (bedroom), 17.0 (study): 1.4 wide, sill 0.9, height 1.3; at 12.5 (bathroom): 1.4 wide, sill 1.7, height 0.8. No doors: the court's roof is not walkable |
| U2 west | (34, 26.9) | (34, 40.5) | oak | door at 12.1 (z 39.0 to 39.9): back corridor to upper hall. Also the lift lobby's east wall (z 26.9 to 30) |
| U3 south | (32, 40.5) | (55, 40.5) | oak | windows at 12.0 and 18.0 (x 44.0 and 50.0): 1.4 wide, sill 0.9, height 1.3, looking south over the roof at the greenhouse's upper glass |
| U4 | (34, 38) | (50, 38) | oak | doors at 7.0 (x 41.0 to 41.9) bedroom, 9.5 (x 43.5) dressing room, 13.0 (x 47.0) wet room |
| U5 | (50, 38) | (55, 38) | oak | door at 3.0 (x 53.0 to 53.9) study |
| U6 | (42, 30) | (42, 38) | oak | door at 5.0 (z 35.0) bedroom to dressing room |
| U7 | (45, 30) | (45, 38) | concrete | none |
| U8 | (45, 34) | (50, 34) | concrete | door at 2.0, 0.8 wide (x 47.0) wet room to bathroom |
| U9 | (50, 30) | (50, 38) | oak | none |
| U10 corridor | (32, 30) | (32, 40.5) | oak | none |
| U11 lobby north | (30, 26.9) | (34, 26.9) | steel | door at 0.1, 1.8 wide, 2.4 high, `style: "slide"` (x 30.1 to 31.9): the elevator's landing door, over the shaft |
| U12 lobby west | (30, 26.9) | (30, 30) | steel | none |
| U13 lobby south | (30, 30) | (34, 30) | steel | door at 2.2, 1.6 wide, 2.4 high, slide (x 32.2 to 33.8): lobby to back corridor |

These mirror the ground floor's quiet-wing walls exactly, with three differences:
the bedroom's door moves from x 37.0 to x 41.0 (the stairwell now runs along the
old door's place, and the new door is 4.5 m from the stair top, clear of the top
landing), the bedroom's garden door and the study's court door are gone (upstairs,
the court is a roof), and U2, U3 and U10 to U13 are new.

### Guards (base 3.0, height 1.0 m, 39.4 in)

| id | a | b | material | what it guards |
|---|---|---|---|---|
| G1 | (36.5, 39.3) | (40.5, 39.3) | oak balustrade (10) | the stairwell's long edge, along the gallery: up to 2.6 m down to the treads |
| G2 | (40.5, 38.0) | (40.5, 39.3) | oak balustrade (10) | the stairwell's east end, over the flight's foot |
| G3 | (32.5, 26.9) | (32.5, 28.1) | steel balustrade (9) | the ladder hatch's west side, 3.0 m down to the service way |
| G4 | (32.5, 28.1) | (32.7, 28.1) | steel balustrade (9) | the hatch's south side, west of the ladder |
| G5 | (33.3, 28.1) | (34.0, 28.1) | steel balustrade (9) | the hatch's south side, east of the ladder |

**Which piece draws a guard: a balustrade wall** (lane G, merged 2026-10-20). A guard
is an interior wall with `base: 3.0` and `height: 1.0` whose material is a
balustrade (data/blueprints/wall_materials.ron: 9 "Steel balustrade", 10 "Oak
balustrade", `form: Some("guard")`): it draws posts, upright balusters closer than
the 100 mm sphere and a top rail at the wall's top (the `guard` row of
data/blueprints/structure_types.ron), and collides as the wall it is, so the
movement lane's banded collision (a wall stops a body where its y0 to y1 overlaps
the knee-to-eye band) stops a walker on the floor it stands on. AD K 3.2(b) accepts
"any wall, parapet, balustrade or similar obstruction as guarding". G1 and G2 are
oak, like the upper hall's walls; G3 to G5 are steel, like the lift lobby's. (When
this draft was written no piece drew a guard, and they were to be plain 1.0 m
walls.) The room flood fill does not split a room at a guard (Wave 0's
`ROOM_DIVIDER_MIN_M`), so the stairwell stays part of the upper hall.

Everything else upstairs is enclosed by walls, so no other edge is open. The roof
(section 6) is unreachable, so its edges need no guards.

### Who reaches what

From the stair top (the upper hall's west end): the upper hall; from it the bedroom
(x 41.0), the dressing room (x 43.5), the wet room (x 47.0) and through it the
bathroom, the study (x 53.0) and the back corridor (z 39.0); from the back corridor
the lift lobby (x 32.2); from the lobby the elevator car (x 30.1) and the ladder
down. The bedroom and the dressing room also connect directly (z 35.0), so the
morning route of player-home.md (bed, clothes, water) still runs as before:
bedroom to dressing room, then 3.5 m of hall to the wet room.

## 3. The stair

**Its type.** The `stairs` entry in `data/blueprints/structure_types.ron` changes
from `size: (1.4, 3.0, 3.6), steps: 14` to `size: (1.2, 3.0, 4.5), steps: 16`. The
Steps shape draws 16 boxes, each a riser of 3.0 / 16 = 187.5 mm (7.38 in) on a tread
of 4.5 / 16 = 281.25 mm (11.07 in), the last flush with the upper floor. Against the
finding:

| rule | the new stair | limit (stricter code) | margin |
|---|---|---|---|
| riser | 187.5 mm | at most 196 mm (IRC) | 8.5 mm |
| tread | 281.25 mm | at least 254 mm (IRC), at most 300 mm (AD K) | 27 mm and 19 mm |
| nosing | none drawn | none needed from 279 mm (IRC R311.7.5.3) | 1.85 mm |
| pitch | 33.7 degrees | at most 42 (AD K) | 8.3 degrees |
| 2R + G | 656 mm (25.8 in) | 550 to 700 mm (AD K) | inside |
| width | 1.2 m (47.2 in) | at least 914 mm (IRC); AD M's 850 mm | 286 mm |
| rise of the flight | 3.0 m | at most 3835 mm (IRC) | 0.84 m |
| handrails | both sides, 0.93 m (36.6 in) above the nosing line | IRC one side, 864 to 965 mm; AD K both sides at 1.0 m and wider, 900 to 1000 mm | inside both |

The old type broke the IRC's riser limit (214 mm) and both codes' rail rules (the
finding's table). The 1.85 mm nosing margin is the thinnest number here: a builder
who shortens the run must add nosings or more risers, never just squeeze the treads.

**Where, and which way.** `stairs-1` (the id kept, section 7), `type_id: "stairs"`,
`pos: (38.75, 0.0, 38.65)`, `rot_deg: 270.0`. Its footprint is x 36.5 to 41.0, z
38.05 to 39.25, along the inside face of the hall's north wall. At yaw 270 the
Steps' rising direction (local +z) points west, so the foot is at x 41.0 in the
middle of the hall and the top at x 36.5, at the hall's west end. You come in at the
front door facing west down the hall (the authored spawn, (53.5, 40.5)), and the
stair's foot is on your right after 12.5 m. It replaces the bedroom door at x 37.0 to 37.9, which
falls inside the flight: the stair now runs where the bedroom door was.

**Its enclosure.** A new ground wall A, (36.5, 39.3) to (41.0, 39.3), base 0,
height 3.0, oak, closes the flight's south side, so the stair runs between two
walls, entered at its foot. A closed stair needs no stair guard, which matters
because no piece draws a sloped guard. Upstairs, G1 stands on the same line, so from
the hall the stair is a wall and from the gallery a glass parapet. The bottom
landing is the hall itself: 1.2 m clear in front of the first riser (x 41.0 to 42.2)
satisfies AD K's "at least as great as the smallest width of the flight" and the
IRC's 36 in. The top landing is the upper hall's west end, 2.5 by 2.5 m (slabs 13
and 15, x 34 to 36.5, z 38 to 40.5), with the back corridor's door on its far side,
2.5 m from the top step, and no door swinging over it.

**Headroom.** The headroom rule measures from the line through the treads'
nosings. For this flight that line is `y = 0.1875 + d x (0.1875 / 0.28125)` at a
distance d from the foot. Slab 14 stands over the flight's first 0.5 m (x 40.5 to
41.0) with its underside at 2.75 m, so the least headroom anywhere under it is at
its edge, d = 0.5:

> 2.75 - (0.1875 + 0.5 x 0.6667) = 2.75 - 0.521 = **2.229 m (7 ft 3.8 in)**, against
> the IRC's 2.032 m (+197 mm, 7.8 in) and AD K's 2.0 m (+229 mm).

The slab could reach no further than d = 0.796 m (IRC) or 0.844 m (AD K). Beyond
d = 0.5 the stairwell is open to the shell roof at 6.0 m, so the least headroom
there is 6.0 - 3.0 = 3.0 m at the top step. Under the hall's slab at the foot:
2.75 m. The ground hall's ceiling (2.75 m) passes the IRC's 7 ft by 616 mm (24 in).

**Handrails.** Both codes require them, and since lane G (2026-10-20) every stair
carries them: the `handrail` row of data/blueprints/structure_types.ron puts a round
rail on both sides of a stair 1.0 m or wider, its top 0.93 m over the line of the
tread edges. (When this draft was written nothing drew one.)

## 4. The ground floor's freed band (z 30 to 38)

### Zones (origin y 0, height 3.0, ceiling 2.75 under the slab)

| id | label | origin (x, z) | size (x, z) | area | type_id | room_type |
|---|---|---|---|---|---|---|
| room-dining | Dining room | (34, 30) | (8, 8) | 64 m2 (689 sq ft) | room_common | dining (new) |
| room-bedroom-2 | Second bedroom | (42, 30) | (8, 8) | 64 m2 | room_bedroom | guest_bedroom (new) |
| room-shower | Shower room | (50, 30) | (5, 4) | 20 m2 (215 sq ft) | room_bathroom | bathroom |
| room-boot | Boot room | (50, 34) | (5, 4) | 20 m2 | room_entry | entry |

21 m across = 8 + 8 + 5. The second bedroom is an 8 by 8 m double, the old main
bedroom's size, far over the space standard's 11.5 m2 for a double.

**Two new room types are needed in `data/rooms.ron`:** `dining` (a dining table,
chairs, a sideboard; no action beyond the usual) and `guest_bedroom` (actions
`sleep` and `personal_storage`, with no `customize_appearance`). The second bedroom
cannot use `bedroom`: that type carries the mirror's action, and
`the_shipped_home_has_one_appearance_room_and_one_wardrobe_room`
(`src/ship/room_types.rs`) requires exactly one room in the home to open the
appearance station.

**Why a shower room and a boot room.** With the bathroom and wet room upstairs, the
ground floor would have no WC, and the second bedroom would be the one bedroom you
reach without the stair, so it is where a resident who cannot climb would sleep.
Approved Document M asks for a WC on the entrance storey in every new dwelling
(1.17(a)) and a WC and basin there in an accessible one (2.27(a)). The shower room
sits directly under the upstairs bathroom, the way real houses stack their wet
rooms on one drain. The boot room is the cloakroom it is reached from: coats and
boots off the entry, the WC beyond. Section 9 records what this costs the water
loop.

### Ground walls: what changes

| wall | change |
|---|---|
| (34, 38) to (50, 38), the hall's north wall | openings become: a door at 0.6, 1.6 wide, 2.4 high, slide, open_dist 3.0 (x 34.6 to 36.2, the dining room, under the stair-top landing at the hall's west end) and a door at 13.0, 0.9 wide (x 47.0, the second bedroom, kept). The doors at 3.0 (old bedroom: inside the flight now) and 9.5 (old dressing room) are removed |
| (42, 30) to (42, 38) | its door at 5.0 is removed: the dining room does not open into a bedroom |
| (45, 30) to (45, 38) and (45, 34) to (50, 34) | deleted: they stood inside the second bedroom |
| (50, 30) to (50, 38) | gains a door at 2.0, 0.9 wide (z 32.0 to 32.9): second bedroom to shower room |
| new (50, 34) to (55, 34), concrete | a door at 2.0, 0.8 wide (x 52.0): shower room to boot room |
| (50, 38) to (55, 38) | unchanged: its door at 3.0 (x 53.0) now leads from the entry into the boot room |
| (34, 30) to (55, 30), the house's north wall | keeps the windows at 1.5 and 6.0 and the garden door at 4.6, now the dining room's; adds a window at 9.5 (1.4 wide, sill 0.9, height 1.3) for the second bedroom; the window at 12.5 becomes sill 0.9, height 1.3 (a bedroom window, was the bathroom's high one); the window at 17.0 becomes sill 1.7, height 0.8 (the shower room's, was the study's); the door at 19.2 (the study's court door, now in the shower room) is removed |
| new wall A, (36.5, 39.3) to (41.0, 39.3), oak, base 0, height 3.0 | the stair's enclosure (section 3) |

Every new room is reachable from the hall: the dining room and the second bedroom
directly, the shower room through the second bedroom or the boot room, the boot
room from the entry (checked by script).

## 5. The elevator and the ladder

**elevator-1 stays at (31.0, 0.0, 26.0)** and now has somewhere to go: its stops are
the service way floor (0) and the lift lobby floor (3.0). The shaft opening is the
car's footprint exactly, so stepping off the car at the top crosses no gap. The
lobby's door onto the shaft (U11) must open only when the car is at the top, or a
person opens it and walks into a 3 m drop; no field expresses that (section 9,
request 4). The type's 3.0 m frame ends level with the top stop, so a rider's body
passes its top bar on the way up; a frame the height of the shaft (6.0 m) would
look right, but then the shaft's top must not be taken for a third stop. Keeping the
3.0 m type is the safe choice for this increment.

**ladder-1 moves from (11, 32) to `pos: (33.0, 0.0, 28.0)`, `rot_deg: 0.0`,** in
the service way, 0.8 m east of the elevator, under the ladder hatch. You climb it
facing south, and at the top step south over it onto the lobby floor (slab 7). It is
the second way up that needs no power: if the elevator cannot run, the ladder still
reaches the upper floor. Why it leaves the plant room: under the lid rule the plant
room is 3 m with a ceiling at 2.75, so a 3 m ladder there would run into its
ceiling; given a hatch, it would put a person onto the roof, which then needs a
guard at every edge that drops (section 6). The hatch is guarded on its west side
(G3) and on its south side except the ladder's own 0.6 m (G4, G5); its north and
east sides are the lobby's walls. The ladder's 0.6 m gap is the one walkable 3 m
drop left aboard. A real ladderway closes it with a self-closing gate, and the game
has no gate piece (section 9, request 4). The builder must check which way the
climb's step-off goes (camera.rs); the placement assumes "forward, over the top".

## 6. The roof over the house's south half: inaccessible

**Decision: the slab over z 40.5 to 51 (pantry, kitchen, great room, console room
and the hall's south half) is an inaccessible roof, like every other lid.** The upper
hall's south wall (U3) closes the upper storey there, and nothing leads onto any
roof.

Why not a roof terrace:

- **It adds nothing the house lacks.** A terrace there would look south into the
  greenhouse, which the great room's glass wall and the greenhouse's own seating
  already do, and it would still be indoors, under the same glass shell 3 m above.
- **Its guards would be most of its edge.** A drop of 3 m into the greenhouse along
  z 51 (21 m) asks a guard of at least 1.0 m there, and its edge against the forge,
  workshop and great-room roofs (no drop) would still need a barrier to keep the
  rest of the roof unreachable. That is a lot of new guard for no new use.
- **A reachable roof is reachable everywhere.** The lids join into one level
  surface across the acre. Any way up onto one (a terrace door, a plant-room hatch)
  makes all of them walkable, and every opening in them (the shaft, the hatch) then
  needs guarding too.

A terrace stays possible later: a door in U3 and guards along its edges, nothing
else.

## 7. The demo pieces: where each now leads

| piece | before | after |
|---|---|---|
| stairs-1 (vehicle bay, (34, 0, 4)) | 3 m stair to nothing; broke the riser, rail and headroom rules | **moves into the hall as the house stair** (section 3). It cannot stay in the bay: the bay's ceiling is 2.75 m, and its headroom would fail from its 4th tread |
| ramp-1 (vehicle bay, (38, 0, 4)) | 1.2 m over 4.5 m, 1:3.75, to nothing | **becomes train-1's boarding ramp:** the `ramp` type becomes `size: (1.8, 0.4, 6.8)`, 1:17 (AD K allows 1:16.8 at a 6.8 m going), at `pos: (48.0, 0.0, 17.4)`, `rot_deg: 180.0`, rising north from z 20.8 to the platform's south edge at z 14.0 and its 0.4 m top. At 0.4 m it needs no handrail (AD K 2.12, 600 mm or less) and no guard (its sides drop less than 600 mm). Its note's "wheelchair friendly" becomes true. A 1.2 m ramp could not stay: its top would leave 1.55 m of headroom under a 2.75 m ceiling |
| train-1 ((48, 0, 10)) | unpaired platform | unchanged; now reached by ramp-1. 2.35 m of headroom over its 0.4 m top |
| teleporter-1 ((22.5, 0, 20)) and teleporter-2 ((31, 0, 80)) | the vehicle bay to the fields | unchanged: both on the ground floor, 2.6 m frames under the 2.75 m ceiling |
| elevator-1 ((31, 0, 26)) | rose 3 m to nothing | the service way to the lift lobby and the upper floor (section 5) |
| ladder-1 ((11, 0, 32)) | climbed 3 m to nothing in the plant room | moves to (33.0, 0.0, 28.0): the service way to the lift lobby (section 5) |

**What the vehicle bay gets under the rule:** a 2.75 m (9 ft 0 in) ceiling under a
steel slab, its stair gone to the house, its ramp re-made as the train platform's
boarding ramp, and its teleporter and platform as they were. The 4 m gantry
player-home.md wanted for the bay cannot exist under a 3 m lid; that is a cost of
the light budget, stated so it is chosen rather than forgotten.

## 8. Machines, furniture and lights

### Moving up with their rooms (`offset.y` + 3.0, x, z and rotation unchanged)

In `data/machines/home.ron`, 35 instances:

- **Bedroom:** bed_1, nightstand_1, nightstand_2, rug_2, standing_mirror_1, desk_2,
  chair_6, chair_9, side_table_4, and shelf_6, which also moves from z 37.0 to
  z 36.6 so the new bedroom door (x 41.0 to 41.9) clears it.
- **Dressing room:** wardrobe_1, wardrobe_2, wardrobe_3, shelf_2, shelf_7,
  mirror_2 (y 1.1 to 4.1), rug_6.
- **Bathroom:** toilet_1, bath_sink_1, mirror_1 (y 1.1 to 4.1), shelf_3.
- **Wet room:** shower_1, washer_1, shelf_8.
- **Study:** desk_1, chair_5, desk_monitor_1 (y 0.75 to 3.75), bookshelf_2,
  bookshelf_3, bookshelf_5, server_1, uplink_1 (y 0.62 to 3.62), router_1 (y 0.84
  to 3.84), rug_5, chair_8.

In `data/machines/home_solo.ron`, 17 instances: toilet_1, bath_sink_1, mirror_1,
shower_1, washer_1, shelf_2 (the wet room's), bed_1, nightstand_1, nightstand_2,
rug_2, desk_2, standing_mirror_1, wardrobe_1, desk_1, chair_5, bookshelf_2,
bookshelf_3.

### Moving on the ground floor

| id | from | to | why |
|---|---|---|---|
| side_table_1 | (40.5, 0, 38.6), rot 180 | (39.0, 0, 42.55), rot 0 | it stood inside the flight; now against the hall's south wall |
| chair_7 | (42.2, 0, 39.4) | (43.2, 0, 39.6) | off the stair's foot landing |
| air_handler_c1 (court) | y 2.7 | y 2.5 | its top (2.942) would pierce the slab's underside (2.75) |
| air_handler_h1 to h4 (plant room) | y 2.7 | y 2.5 | the same |

The only machines taller than 2.75 m in any lidded room are those five air
handlers (all others checked). The greenhouse's air_handler_g1 and g2 may stay at
2.7 or rise toward its new 6 m roof; that is the light and air lanes' call.

### New furniture (no stats, so free to add; `data/machines/home.ron`)

| id | machine | room | offset (x, y, z) | rotation |
|---|---|---|---|---|
| dining_table_3 | dining_table | room-dining | (38.0, 0, 34.0) | 0 |
| chair_13, chair_14 | chair | room-dining | (37.5, 0, 33.1), (38.5, 0, 33.1) | 0 |
| chair_15, chair_16 | chair | room-dining | (37.5, 0, 34.9), (38.5, 0, 34.9) | 180 |
| chair_17, chair_18 | chair | room-dining | (36.7, 0, 34.0), (39.3, 0, 34.0) | 90 and 270, facing the table (check in the editor) |
| rug_10 | rug | room-dining | (38.0, 0, 34.0) | 0 |
| shelf_20 (sideboard) | shelf | room-dining | (41.6, 0, 34.0) | 90 |
| bed_2 | bed | room-bedroom-2 | (43.6, 0, 34.0) | 0 |
| nightstand_3, nightstand_4 | nightstand | room-bedroom-2 | (42.9, 0, 32.9), (42.9, 0, 35.1) | 0 |
| rug_11 | rug | room-bedroom-2 | (46.5, 0, 34.0) | 0 |
| wardrobe_4 | wardrobe | room-bedroom-2 | (49.6, 0, 36.5) | 90 |
| desk_4, chair_19 | desk, chair | room-bedroom-2 | (47.2, 0, 30.4), (47.2, 0, 31.1) | 180, 0 |
| shelf_21 (bench) | shelf | room-boot | (50.35, 0, 36.0) | 270 |
| wardrobe_5 (coats) | wardrobe | room-boot | (54.6, 0, 35.5) | 90 |
| rug_12 | rug | room-boot | (52.5, 0, 36.0) | 90 |

The great room keeps its own dining set by the kitchen pass: the dining room is the
formal table, the great room's the everyday one. The shower room's fixtures
(toilet_2 at (54.5, 0, 31.0), bath_sink_2 at (54.6, 0, 32.4), mirror_3 at (54.8,
1.1, 32.4), all rotation 90, and shower_2 at (50.6, 0, 30.6), rotation 0) wait on
the water decision in section 9.

Each new piece was checked to stand clear of its room's walls, the stair, the other
pieces and the swing of every new door.

### Lights (`data/homes/homestead.ron`)

- **Up 3.0 with their rooms:** the bedroom's warm_lamp (38, 2.6, 34) to y 5.6 and
  its bedside warm_lamp (35.2, 0.9, 33) to y 3.9; the dressing room's warm_lamp
  (43.5, 2.6, 34) to 5.6; the bathroom's and wet room's cool_panels (47.5, 2.7, 32)
  and (47.5, 2.7, 36) to 5.7; the study's warm_lamp (52.5, 2.6, 34) to 5.6 and
  ceiling_panel (52.5, 2.7, 31) to 5.7.
- **New upstairs:** a strip along the upper hall from (34.5, 5.6, 39.9) to (54.5,
  5.6, 39.9) (intensity 7, range 6, like the ground hall's), a strip along the back
  corridor from (33.0, 5.6, 30.5) to (33.0, 5.6, 40.0) (intensity 6, range 5), and a
  ceiling_panel in the lift lobby at (32.0, 5.7, 28.5).
- **New downstairs:** warm_lamps at (38.0, 2.6, 34.0) (dining room) and (46.0, 2.6,
  34.0) (second bedroom), a cool_panel at (52.5, 2.7, 32.0) (shower room, intensity
  6, range 5), a ceiling_panel at (52.5, 2.7, 36.0) (boot room), and a warm_lamp at
  the stair's foot, (41.6, 2.6, 38.65), because the enclosed flight's lower half is
  in the ground band, where the upper hall's strip does not reach once lights keep
  to their storey.
- **The service way's strip** (3, 2.6, 26) to (31, 2.6, 26) runs into the elevator
  shaft (x 30.1 to 31.9): end it at (29.9, 2.6, 26).
- **The greenhouse's four cool_panels** hang at 2.7 m under what is now a 6 m roof:
  raising them toward 5.7 m is the light lane's call.

## 9. What does not balance, and what needs a decision or code

1. **The lid rule shades three rooms that live on sunlight.** The power terrace
   holds the home's 10 solar panels, the fields its four field crops (the bulk of
   the calories), and the orchard court four potato beds and two sun-lit apothecary
   towers. Today's simulation does not notice: solar output and crop daylight follow
   the clock alone (`systems::solar::sun_factor`), so the Energy and Food loops'
   numbers would not move. But the world would contradict them once slabs cast into
   the sun shadow map (plan, lane L): panels producing nameplate power under a steel
   ceiling. **Recommendation (needs code, data field):** give `FloorSlab` an
   optional `material` (written only when set) and let a glass slab confine probes
   like any lid while passing sunlight, then make slabs 1, 8 and 18 glass. The probe
   budget is unchanged (the rooms stay 3 m) and the sun still reaches them through
   the shell. Without it, the honest alternatives are moving the panels onto a
   reachable roof or the crops under lamps, and both change the loops.
2. **Handrails, and any sloped stair guard, need a piece (code).** Done by lane G
   (2026-10-20): every stair carries handrails (the `handrail` row), and a guard is a
   balustrade wall (wall materials 9 and 10). A sloped stair guard is still not
   needed: the flight runs between two walls.
3. **Two room-detection rules for the contract (Wave 0).** (a) A wall shorter than
   its band (a 1.0 m guard) should not split a room in the flood fill, or the
   stairwell becomes its own room. (b) The space above the lids outside the upper
   rooms and the greenhouse, about 3,650 m2 by 3 m, must not become a room: as one,
   it would carry probes and about 11,000 m3 of air nobody breathes, against the
   orchestrator's 830-probe estimate. One way: in a band above the ground, a room
   exists only where a zone declares it. And `name_rooms_from_zones` must be
   band-aware: the dining room and the bedroom above it share an x/z rectangle (8 by
   8 each, so the "smallest zone wins" rule ties).
4. **Two doors need an interlock the data cannot express (code).** The lobby's door
   onto the shaft (U11) must open only while the car is at the top; the ladder's
   0.6 m access gap wants a self-closing gate. Until then, both are 3 m drops a
   player can walk into.
5. **The downstairs shower room double-counts water.** Every fixture carries its own
   daily draw (`toilet`: "-42 L/d flush", `shower`: "-105 L/d", `bath_sink`: "-15
   L/d"), so a second set adds about 162 L a day to the meter for the same people.
   **Decision for the orchestrator:** either the household's water is split between
   the two sets (a revision of the Water loop's figures and its `loops` prose), or
   the shower room ships without fixtures for now and the ground floor has no WC,
   which AD M 1.17(a) asks of every new dwelling.

Smaller items for the builder:

- **The greenhouse's walls should rise to 6.0:** upper walls (base 3.0, height 3.0)
  over (10, 51) to (34, 51) steel, (34, 51) to (55, 51) glass, (10, 51) to (10, 73)
  HDPE and (10, 73) to (55, 73) steel. Without them the greenhouse's upper air joins
  the space above every lid.
- **The spawn (53.5, 40.5) now has the upper hall above it** (on its south wall's
  line). It must resolve to the ground floor: a storey on `spawn`, or "the lowest
  floor at that x/z".
- **A wall spanning two bands:** this draft never uses one (every upper wall is its
  own record with `base: 3.0`, and wall A stops at 3.0), so it works with the
  contract as written, where a wall belongs to the band its base is in.

## 10. Checks run

A throwaway script (not shipped) encoded this layout and checked, all passing:
no two rooms overlap on either storey and the ground rooms tile the acre exactly
(4,895 m2); every upstairs room is reachable through doors from the stair top, and
every new ground room from the hall; every door lies on its wall, opens between the
two rooms it names, and no two openings on a wall overlap; the 19 slabs do not
overlap each other or any opening and with the openings tile the acre exactly;
every upstairs room stands on slab or a declared opening (the upper hall holds the
5.2 m2 stairwell, the lobby the 1.8 m2 hatch); a slab edge meets the stair's top at
x 36.5 with no gap; the stair's riser, tread, pitch, run and headroom as in section
3; ramp-1 lies in the bay, meets the platform's edge and clears the trading post;
ladder-1 stands under its hatch with its climber clear of the shaft; every moved and
new machine stands in the room it names, clear of walls, the stair, each other and
every new door's swing, and the stair's foot landing is clear.

## 11. What else follows the move

- `scripts/verify-falls.js`, `scripts/lib/falls-judge.js` and
  `scripts/tests/falls-judge.test.js` walk up stairs-1 in the vehicle bay and off its
  unguarded 3 m top. That drop no longer exists (the top now arrives on a floor), so
  the walk-off legs need a new 3 m edge (the ladder's access gap is the one left) or
  new verdicts; the stairs-down leg works on the new stair, and the ladder leg works
  at ladder-1's new place. The judge reads the stairs' geometry from
  structure_types.ron, so the new riser and tread change its numbers.
- `src/engine/survival_env.rs`
  (`the_top_of_the_homesteads_own_elevator_breathes_the_homes_air`) pins the stairs'
  top at (34, 4.7, 5.7) and the ladder's at (11, 4.9, 32): both move (to about
  (36.6, 4.7, 38.65) and (33.0, 4.9, 27.5)).
- `src/engine/footing.rs`'s comment "the homestead's `stairs-1` leads to no upper
  floor either" becomes untrue.
- [player-home.md](player-home.md)'s plan table, its walkthrough (the quiet half, the
  loading dock with stairs and a ramp) and "What a second storey needs" describe the
  single-storey home and should point here once the data lands.
- Both copies, `data/homes/homestead.ron` and `data/homes/shipped/homestead.ron`, get
  every change (they differ only in how a teleporter names its pair).

## 12. As built (2026-10-20, Wave 1 lane C1)

The data follows this draft, with these differences, each decided or found while building it:

- **The lids are glass wherever no one lives above them** (the orchestrator's revision of
  decision 6). Solid slabs lie exactly under the upstairs rooms: the quiet wing, the upper hall
  (three slabs round the stairwell), the back corridor and the lift lobby (two slabs round the
  hatch). Every other ground room keeps today's daylight under a glass slab. That re-tiles
  section 1 into 23 slabs: the workshop's roof is split so the back corridor's 2 m strip
  (x 32 to 34, z 30 to 40.5) is solid, the house's south roof starts at z 40.5 (the upper
  hall's south wall), and the service way's roof is cut round the lift lobby and the shaft.
  The four openings are unchanged, and slabs and openings still add up to the acre.
- **The elevator shaft is enclosed upstairs** on its north, east and west sides (three
  3.0 m steel walls, base 3.0). Without them a rider at the top could step off the car onto
  the lids, and section 6's rule is that a reachable lid makes every lid a roof.
- **The greenhouse's zone stays 3.0 m tall** though its room rises to the 6.0 m roof (its walls
  rise with it, base 3.0). The zone is also the greenhouse's air box in the life-support model,
  and a 6.0 m box doubled the humid air it leaks into the house at the model's 0.5 air changes
  an hour: the house's four air handlers then held 53% instead of their 50%
  (`the_shipped_homes_hold_their_air_and_return_their_water`, 2026-10-20). Raising the zone is
  the Air loop's call: more handling for the house's air, or a tighter greenhouse.
- **The guards are balustrades** (lane G's guard piece, merged the same day): G1 and G2 oak
  (wall material 10), G3 to G5 steel (9), base 3.0, 1.0 m tall, at the positions in the table
  of section 2. Each draws posts, balusters and a top rail and collides as the wall it is.
- **New furniture ids**: chair_13 to chair_15, rug_10, shelf_20, shelf_21 and desk_4 were
  already in use (the workshop's desk, the greenhouse's seats), so the dining chairs are
  chair_16 to chair_21, the second bedroom's desk chair chair_22, the rugs rug_11 to rug_13,
  the sideboard shelf_23, the boot room's bench shelf_24 and the desk desk_5. The shower
  room's fixtures are placed in both machine files, the solo home's too, because the water is
  now shared (below) and the entrance floor needs its WC either way.
- **The water is shared** (decision 7): a fixture's daily figure is the household's use of that
  kind of fixture, and every fixture of the kind shares it (`household_use` in
  data/machines/home.ron and home_solo.ron; `MachineHome::household_share` and the plumbing
  tick's `HouseholdUse`). Section 9 item 5 is closed: the shower room adds no water.
- **ramp-1 became the train platform's boarding ramp in a follow-up commit** (a3e55a85b,
  2026-10-20; lane C1 itself touched only the stairs row of structure_types.ron, beside lane
  G's guard rows). Section 7's change is built as planned: the `ramp` row is
  `size: (1.8, 0.4, 6.8)`, 1:17, and ramp-1 stands at `(48.0, 0.0, 17.4)` at 180 degrees, so
  the 1.2 m ramp with 1.55 m to the bay's lid is gone.
- **The checks of section 10 are tests now**: src/ship/homestead_layout_tests.rs, run with
  `node scripts/lib-tests.js homestead_layout_tests`.
