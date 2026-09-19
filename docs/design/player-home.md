# The player's home

Written 2026-09-19, from the operator's brief: "feel free to redesign the whole
initial player home to better utilize the full space we have. You can base it on
the fibonacci sequence or you can figure out something better. When we make a
great player home then we can mirror that to all the NPC homes so they're
aesthetically pleasing should the player enter one for whatever reason."

His verdict on what was there before: "we have a kinda crappy player home that's
poorly designed arranged as we've just been using the giant empty room as a tech
demo area."

The wider decisions this sits under are in [the-mothership.md](the-mothership.md):
the ship is premade authored content, the player's acre is the only thing they
modify, and a home has to be DATA so premade designs can become a library that
players contribute to.

## The footprint, and what was wrong with it

The player's home is one zone in `data/blueprints/ship_structure.ron`: a box
**55 m by 89 m by 3 m**, 4,895 square metres, which is 1.21 acres. That is
already the operator's "1 acre or whatever it is for that specific ship design",
so the shell was never the problem.

What was wrong was the inside. Before this pass the acre held 19 interior walls,
all crowded into a 16 by 23 m corner, partitioning about 330 m2. The other **4,565
square metres, 93 percent of the home, was one continuous open hall**, and the
machines standing in it were arranged in straight demo rows: seventeen crafting
stations in three lines on open floor, the whole power and water plant in a grid
with nothing around it, five garden arrays in the middle of nowhere. Six of the
seven in-world screens were on the walls of one 3.5 by 3 m booth, which is the
"corridor of identical monitors" the brief asks to avoid.

The new plan partitions the whole acre. Twenty-three rooms, every one of them
named and functional, and no unclaimed floor left over.

## The plan

```
        x=0        12   20        34                      55
  z=0   +----------------+-------------------------------+
        |  POWER TERRACE |         VEHICLE BAY           |
        |   20 x 22      |          35 x 22              |
  z=22  +----------------+----------+--------------------+
        |     SERVICE WAY 34 x 8    |  ORCHARD COURT     |
        |                           |     21 x 8         |
  z=30  +------+---------+----------+--------------------+
        |PLANT | FORGE   | WORKSHOP |     THE HOUSE      |
        |12x21 | 8 x 21  |  14 x 21 |      21 x 21       |
  z=51  +------+---------+----------+--------------------+
        | MUSH |                                         |
        | 10x10|          THE GREENHOUSE 45 x 22         |
  z=61  +------+                                         |
        |AQUA  |                                         |
        |10x12 |                                         |
  z=73  +------+----------------------+------------------+
        |     THE FIELDS 40 x 16      |   BARN 15 x 16   |
  z=89  +-----------------------------+------------------+
```

The house, at 441 m2, is 9 percent of the acre. The rest is the work that keeps
it alive, and the ratio is the point: this is a homestead, not a house with a
garden.

Inside the house (x 34..55, z 30..51), entered from the ship's corridor on the
east wall at z 39..41:

| Room | Rect (x, z) | Size |
|---|---|---|
| Entry | 52..55, 38..43 | 3 x 5 |
| Hall | 34..52, 38..43 | 18 x 5 |
| Bedroom | 34..42, 30..38 | 8 x 8 |
| Dressing room | 42..45, 30..38 | 3 x 8 |
| Bathroom | 45..50, 30..34 | 5 x 4 |
| Wet room | 45..50, 34..38 | 5 x 4 |
| Study | 50..55, 30..38 | 5 x 8 |
| Pantry | 34..42, 43..46 | 8 x 3 |
| Kitchen | 34..42, 46..51 | 8 x 5 |
| Great room | 42..52, 43..51 | 10 x 8 |
| Console room | 52..55, 43..51 | 3 x 8 |

## Walking through it

You arrive through the corridor from the Commons and step into the **Entry**, a
3 by 5 m vestibule with a coat rack, a bench shelf and a rug. It is deliberately
the smallest room in the house: you are compressed for two steps before anything
opens up.

Ahead of you, west, the wall opens into the **Hall**. This is not a corridor. It
is 18 m long and 5 m wide, a gallery with two rugs, a bench, bookshelves and a
light strip running its length, and every room in the house opens off it, so you
never cross one room to reach another. On its north side, doors to the bedroom,
the dressing room and the wet room. On its south side, a door to the pantry and a
4 m cased opening into the great room. At its far west end, a door straight into
the workshop, which is why the work wing does not have to be reached through the
kitchen.

Turn south through that opening and the ceiling stays at 3 m but the room widens:
the **Great room**, 10 by 8 m, with the whole south wall in tempered glass
looking into the greenhouse. Couches face the north wall, where the stream screen
hangs, so the glass is behind you and never glares. The dining table sits at the
west end by the kitchen pass, under its own rug.

**West through a 2.4 m opening is the Kitchen**, a 8 by 5 m galley with the sink,
stove and oven in one run along the outer wall, a work island in the middle, and
its own door into the greenhouse. That door is the single most-used thing in the
house: three steps from a pot to the beds. Behind the kitchen, off the hall, is
the **Pantry**, deliberately the one room with no glazing.

**East of the great room, through a door, the Console room.** It is 3 m wide and
8 m deep, glazed at the far end. You sit at the desk against the east wall with
the web screen above it and the garden camera on the wall opposite, and the
greenhouse glows at the end of the room. Six screens used to be crammed in here;
now there are two and a desk monitor, and the room is a place to sit rather than
a display case.

**North of the hall is the quiet half.** The **Bedroom** is 8 by 8 m with the bed
against the west wall, a desk under the north window and a standing mirror by the
door. Its north windows and its own garden door look into the **Orchard court**,
an 8 m deep planted strip between the house and the works, which exists so that
the bedroom does not look at a forge. Through the bedroom's east door is the
**Dressing room**, a 3 by 8 m walk-through with three wardrobes, and beyond that
the **Bathroom** and the **Wet room**. That is the classic sequence: bed, clothes,
water, and you can do the whole morning without crossing the house. At the
north-east corner, off the entry, the **Study**: desk, three bookcases, the home
server and the network uplink.

**West of the house** the hall's back door puts you in the **Workshop**, 14 by
21 m, benches under a screen playing whatever clip you left running. Through a
door, the **Forge**: smelter, forge and kiln in a concrete-walled cell of its own,
because a hot shop should not share air with a loom. Through another, the **Plant
room**: batteries, generator, air recycler, the water train and the cistern, the
only room in the house you can turn off and kill everything else.

**North of all three**, doors into the **Service way**, an 8 m belt with a light
strip and storage racks, which connects to the **Vehicle bay** (drone pad, vehicle
assembler, trading post, train platform, loading dock with stairs and a ramp) and
to the **Power terrace** (the solar array and the wind turbine, placed at the
terrace's south end so the cable run to the batteries is 12 m instead of the 20 m
it used to be).

**South of the house** is the **Greenhouse**, 990 m2 of glass-roofed growing:
eighteen potato beds, ten oilseed beds, eight grain trays, a 24-tower block, a
potting bench, and a pair of chairs and a table at the east end, because a
greenhouse you cannot sit in is a factory. The planting calendar hangs on the
wall beside the great room's door. West of it, two rooms that need their own
climate: the **Mushroom room**, dark by design, and the **Aquaponics** room with
the fish tanks.

**At the far end**, past a wide opening, the **Fields** and the **Barn** with the
silo. This is the longest walk in the home, roughly 70 m of corridor and room from the front door, and
it is deliberately the thing you visit least. A teleporter pad at each end (the
vehicle bay and the fields) makes the trip instant when you are carrying a
harvest.

## Why these proportions

The operator offered Fibonacci and also said "or you can figure out something
better", and separately that his earlier Fibonacci work was "mostly just me
trying to figure out how to lay things out and scale things. Not like a fixed all
players or ships follow this rule."

So here is the honest case for using it here, and it is a practical one rather
than a mystical one.

**The shell is already a golden rectangle.** 55 and 89 are consecutive Fibonacci
numbers, and 89/55 is 1.618. Whoever sized this allotment chose those numbers.
Given a golden rectangle, the one division the shape offers for free is the
gnomon: take the largest square out and the remainder is another golden
rectangle. That single fact suggested the band structure, and the bands landed on
sizes that were already the right sizes for what goes in them: 22 m is a
generous yard, 21 m is a deep enough house, 22 m of greenhouse is four bed rows
plus aisles, 16 m is enough for four field plots.

**The room sizes tile without offcuts, and that is the real argument.** 3, 5, 8,
13 and 21 metres are all genuinely good room dimensions: 3 m is a shower or a
dressing aisle, 5 m is a bathroom or a study, 8 m is a bedroom or a kitchen,
13 m and 21 m are halls and shops. Because each is the sum of the two below it, a
5 m room and a 3 m room stack exactly against an 8 m room, and an 8 and a 13 make
a 21. Walls line up across the plan and no leftover slivers appear. In the house:
21 = 8 + 3 + 5 + 5 across the north wing, 21 = 8 + 5 + 8 down the depth, and
21 = 8 + 10 + 3 across the south wing, where the great room took the extra metre
off a 13 because a 10 m living room and a 3 m den read better than 13 and 0.

That last sentence is the part worth keeping. **The sequence was used where it
helped and abandoned where it did not.** A layout that reads well to a person
standing in it beats a layout with an elegant derivation, and every room here was
sized by asking what goes in it first.

**One rule decides every door.** Up to 1.2 m it is a hinged leaf that swings;
wider than that it is a pocket door that slides into the wall, 2.4 m tall
instead of 2.1, and its approach distance grows with its width so it is already
open when you reach it. That is why the great room's 4 m opening onto the hall,
the kitchen pass, the greenhouse slider and the vehicle bay's roller all behave
like the openings they are, rather than like a 4 m panel on a hinge.

**Material is doing as much work as proportion.** The wall material ids
(1 steel, 2 concrete, 3 oak, 4 tempered glass, 8 white HDPE) are chosen per wall
rather than per house: the dwelling is oak, the wet cells and the forge are
concrete, the works are steel, the grow rooms are white plastic, and the two
faces that matter for the view, the house's south wall onto the greenhouse and
the screen wall between the vehicle bay and the orchard court, are glass. The
shell roof is glass everywhere, so every room in the home has stars above it.

## The screens, and where each one earns its place

The operator's standard, verbatim: "the player bedroom could have like a standing
mirror like display that also doubles as a touchscreen for changing character
appearance."

So the rule applied here is that a screen goes where a person would already be
standing when they wanted that thing, and a room that does not need one does not
get one. How screens work at all is [in-world-screens.md](in-world-screens.md).

| Screen | Room | Shows | Why there |
|---|---|---|---|
| `wall_screen_1` | Entry | `page:inventory` | Over the bench where you drop your kit. Coming home, the first question is what you are carrying and what is in the house. |
| `wall_screen_2` | Kitchen | `page:tasks` | The household board by the work island. This is where a family's to-do list actually lives. |
| `wall_screen_3` | Console room | `camera:camera_post_1` | The watch post. The camera looks down the greenhouse tower rows. |
| `wall_screen_4` | Great room | `watch:shaostoul` | The television, on the wall the couches face. |
| `wall_screen_5` | Workshop | the demo clip | Over the two main benches. A clip playing while you build is what a workshop screen is for. |
| `wall_screen_6` | Console room | `web:united-humanity.us` | The reading screen, above the desk. `verify-screens.js` parks here. |
| `wall_screen_7` | Forge | `page:crafting` | Above the smelter and the forge, where you are standing when you want a recipe. |
| `wall_screen_9` | Hall | `page:quests` | By the front of the hall, on the way out. What is on today. |
| `wall_screen_10` | Greenhouse | `page:calendar` | On the wall beside the great room's garden door. A planting calendar at the greenhouse threshold. |
| `desk_monitor_1` | Study | `page:library` | On the study desk, among the bookcases. |
| `desk_monitor_2` | Console room | `page:chat` | On the console desk. Comms at the comms station. |
| `standing_mirror_1` | Bedroom | `page:profile` | The operator's own example. A 1 by 2 m portrait panel beside the bedroom door. |

There is no `wall_screen_8`; the number was skipped rather than shuffling the
others, because `1`, `2` and `6` are named by `scripts/verify-screens.js` and
renaming them would quietly shrink that gate.

**Rooms deliberately left without a screen:** the bathroom, the wet room, the
dressing room, the pantry, the plant room, the service way, the vehicle bay, the
fields and the barn. A bedroom has one only because it is a mirror. The
temptation with a page registry this large is to hang one of everything on a
wall, and that is exactly the showroom the brief is trying to get away from.

**The mirror does two things.** The panel itself is a screen showing your
profile, and walking up to it and pressing E opens the appearance editor. That
second half was dead code before this pass: `src/lib.rs` matched the room id
against the literal strings `"wetroom"` and `"bedroom"`, but room ids in the live
home come from the zone that covers them (`room-bedroom`, `room-wetroom`), so
neither ever matched and there was no way to reach the look editor from inside
the world at all. It now reads the room's **actions**, which come from
`data/rooms.ron`: a room offering `customize_appearance` is a mirror, one
offering `change_outfit` is a wardrobe. The bedroom has the first, the dressing
room has the second, and any home anyone authors gets both for free by naming a
room type.

## The captures, and an honest verdict on each

`node scripts/photograph-home.js` boots the release build, enters the world and
stands in every room. The evidence folder is
`.probe-rig/home-photos/runs/<stamp>/`, one PNG per vantage plus a manifest
naming the pose each was taken from. 29 of 29 captured, zero panics.

The screens gate runs against the same build:
`node scripts/verify-screens.js` parks in the console room and passes 12 of 12,
which proves the wall inventory is still clickable, the web wall still
navigates, and the task board still draws, from their new rooms.

The question the brief asks is whether it looks like somewhere a person lives.
Room by room, and not flattering it:

| Capture | Verdict |
|---|---|
| **Overview** | The acre reads. From above you can name every space without being told, which is the thing that was not true before. |
| **Entry** | Good. A narrow bright vestibule, then the hall opens through a framed aperture. The compression and release works. |
| **Hall** | Good as a route, thin as a room. The light strip runs its length, three doors on one side and the great room's wide opening on the other, and you can see the workshop door at the far end. |
| **Great room** | The glass wall onto the greenhouse is the best thing in the plan. The room itself is too big for what is in it: a seating group at one end, a dining set at the other, and bare floor between. |
| **Kitchen** | Good. The garden beyond the glass is right there, and the galley run reads. The appliances are featureless dark boxes. |
| **Console room** | The best room in the home. Three metres wide, the web screen over the desk on one side and the garden camera opposite, the greenhouse glowing at the far end. It is a place to sit, which is what it was not before. |
| **Bedroom** | Weak. Eight metres square is a real bedroom size, but with one bed, two nightstands and a rug in it the room reads empty, and the service runs cross its ceiling. |
| **Bedroom mirror** | The concept works: a full-height panel showing the Profile page, and walking up to it opens the look editor. The page is laid out for a wide screen, so in portrait it fills the top third and the rest is black. |
| **Dressing room** | Good. Three wardrobes down one side, shelves down the other, and the bedroom visible through the open door. It reads as exactly what it is. |
| **Bathroom, wet room** | Utilitarian and fine. The pipe drops to each fixture read correctly HERE, which is the one place they help. |
| **Study** | Good. Book-lined, a monitor on the desk showing the Library, a window onto the court. |
| **Orchard court** | The window from the court into the lit bedroom is the nicest detail in the home. The planters read as empty boxes, because nothing is planted in them. |
| **Workshop** | Was the weakest: the first sweep came back mostly black. The lighting is roughly doubled now and the benches, racks and the clip screen all read. The service runs still dominate the frame. |
| **Forge, plant room** | Legible, industrial, correct. |
| **Greenhouse** | Strong. Walking down the tower avenue with plants growing on both sides is the most convincing thing in the acre. |
| **Vehicle bay, service way, power terrace** | A big apron with a drone pad, a loading dock, a transit platform and a solar array around the edges. An apron is supposed to be mostly floor, so this is the intended emptiness, but it is at the limit of it. |
| **Mushroom room, aquaponics** | The mushroom room works: eight racks in a dark cell. The aquaponics room does not: two fish tanks in 120 square metres. |
| **Fields, barn** | Nearly empty, and not intentionally. Four plots and a silo in 1,120 square metres is a lot of floor for very little. |

**The honest summary: about two thirds of it reads as somewhere a person lives,
and the third that does not fails for reasons that are not the plan.**

1. **Nothing is textured.** Walls are flat colour with no trim, skirting or
   material detail, so a room's character comes entirely from its shape.
2. **Fifty-five of seventy-one machine types have no model.** A kitchen whose
   stove, oven and sink are three grey boxes cannot read as a kitchen no matter
   where they stand. And the sixteen that DO have one draw at the model file's
   own size rather than the size the catalog declares: `pantry_cabinet` says
   2.0 by 2.0 by 0.6 m and renders as a roughly 0.8 m kitchen unit. Every piece
   of modelled furniture in the home is therefore smaller than the footprint its
   own data claims, which is a large part of why rooms read emptier than they
   measure.
3. **The service runs cross the living rooms.** Power and water are drawn as
   coloured cylinders at ceiling height between the machines they connect, with
   a bracket every couple of metres, and a central plant room feeding a house
   means long runs. In the bathroom that reads as plumbing; across a bedroom
   ceiling it reads as a building site. They can be switched off today (the
   construction editor's "Pipe" visibility filter), and the real fix is either
   routing them through the conduit NODE GRAPH that already exists in the data
   and is empty, or a per-room ceiling to hide them above, which is the same
   change a second storey needs.
4. **The rooms are sized for a furniture set that does not exist yet.** An 8 m
   bedroom and a 10 m great room are ordinary real sizes. They look empty
   because fifteen furniture models is not a house's worth of things.

That last one is a deliberate choice rather than an oversight. Shrinking the
rooms until today's fifteen models fill them would make the captures look better
now and produce a cramped home later, and the project's standing rule is to build
the version a 2030 release would ship and fix the fidelity underneath it.

**One room is genuinely mis-sized and it is the aquaponics room.** Two fish tanks
do not fill 120 square metres, and unlike the bedroom there is no furniture
coming to fill it. The machine SET is frozen by the `loops` block at the bottom
of `home.ron`, which states counts in prose ("2 aquaponic fish tanks", "26
towers", "8 solar panels"), so adding a third tank makes that text a lie. The
right order is to revise the loop sizing first and the room second. The fields
are the same shape of problem with a better excuse: four plot machines are a
TOKEN for a field rather than the field itself, and a field is supposed to be
big.

## Where you wake up, and the doll in the dressing room

Two things followed from partitioning the acre that were not obvious until it
was partitioned.

**World entry used to put the player in the largest room.** That was a sane
fallback while the acre was one open hall with a house in the corner. The moment
it became twenty-three rooms the largest one was the greenhouse, so a player
would have woken among the beds instead of at their own front door. The home
body already carries a `spawn: Some((x, z))` that the build-mode avatar gizmo
sets, and it was being read for build mode only. World entry now prefers it, and
this home's spawn is just inside the front door facing west down the hall, which
is the first thing the design wants you to see. The largest-room fallback is
still there for a home that declares nothing.

**The character-select avatar is drawn in the world, not only in the showroom.**
It stands on a podium at the spawn room, which would have put a blockman among
the tower rows. A figure on a podium is a dress form, so it now goes to the room
whose type offers `change_outfit`, which is the dressing room, falling back to
the appearance room and then to the spawn room. Same rule as the walk-up: the
room's data decides, not a hardcoded id.

## What the data could not express

Four things, in the order they cost the design something.

1. **A room cannot have its own ceiling.** `HomeStructure` renders one roof plane
   at the body's `height`, so every room in the acre is 3 m and the greenhouse,
   the workshop and the vehicle bay cannot be taller than the bathroom. A 3 m
   ceiling over 990 m2 of glasshouse is the one thing in this plan that is
   genuinely wrong, and the fix is the same one a second storey needs: a base Y
   and a top Y on a wall, and a ceiling plane per room rather than per body. The
   smallest change that would help on its own is an optional `ceiling_height`
   on `Zone`, drawn as a lid where it is lower than the shell.

2. **Openings are cut per wall, not per room, so a door is a number along a
   segment.** `at: 9.5` on the wall from (34,38) to (50,38) is a door into the
   dressing room, and nothing in the file says so. When a wall is re-drawn every
   opening on it has to be re-measured by hand. The smallest fix is an optional
   `label` on `Opening`, which costs one field and makes the file readable.

3. **The flood fill ignores doors.** `detect_rooms` blocks every cell a wall
   passes through regardless of its openings, which is what keeps rooms separate,
   but it also means nothing in the data model knows which rooms CONNECT. There
   is no adjacency graph, so nothing can check that every room is reachable from
   the front door. That check was done by hand here. A door-aware second pass
   over the same grid would give the engine a room graph for almost nothing, and
   NPC pathing will want it anyway.

4. **A machine's declared SIZE does not scale its model.** The `size` field
   drives the primitive fallback, the screen quad and the click box; the glTF
   loader ignores it, so a `pantry_cabinet` declared 2 m wide draws at whatever
   size its model file was authored, about 0.8 m. The fix is one multiply where
   the mesh is parsed in `src/engine/home_meshes.rs`, against the model's own
   bounds. The reason not to do it blind is that it resizes all fifteen existing
   models at once and wants a look before and after.

5. **Furniture has no collision.** Machines are drawn and clicked but not
   collided with, so a wardrobe placed half inside a wall looks wrong and plays
   fine. Every placement in this pass was checked by arithmetic in a throwaway
   script; that check is now a real test
   (`every_placed_machine_stands_inside_the_room_it_names`), but it proves the
   machine is in the right ROOM, not that it clears the furniture beside it.

None of the five changed the design. The first one constrained it: the greenhouse
and the vehicle bay would both be taller if they could be.

## What a second storey needs

Multi-storey interiors do not work yet, and per the brief the fixes belong with
the construction tool rather than here. The assessment on the
`worktree-agent-a6dc7a9251136237c` branch lists four blockers, all still true:

1. **`InteriorWall` has no base Y.** It is two 2-D endpoints plus a height rising
   from the zone floor, so an upper-storey wall cannot be authored at all.
2. **Collision is flattened to one plane.** `ship_segments_impl` in
   `src/ship/wall_collision.rs` uses the zone origin's x and z and never its y,
   so an upper deck's walls would block the lower deck.
3. **The footing sampler takes the first room whose XZ box contains the player**
   (`src/lib.rs`, commented "Room floors are coplanar in the home"), so two rooms
   stacked at the same x/z are ambiguous.
4. **There is no single canonical layout schema.** `ShipDef`/`DeckDef` has decks
   and is only read by the headless relay; `Zone`/`HomeStructure` is what the
   renderer draws and is single-plane; `data/ships/layout_medium.ron` is a third
   shape with every room at y = 0.

What this plan would do with a second storey, so the work has a target:

- **The quiet wing goes up.** Bedroom, dressing room, bathroom, wet room and
  study move to a first floor over the house's north half, reached by a stair off
  the hall's north side where the bedroom door is now. That frees 168 m2 on the
  ground floor for a proper dining room and a second bedroom.
- **The greenhouse gets its height back** and a mezzanine walkway at 3 m over the
  tower block, which is how real tall-crop glasshouses are worked.
- **The vehicle bay gets a gantry** at 4 m, which is what the existing stairs and
  ramp in it are actually for.

The elevator already placed in the service way at (31, 26) is there for this: it
is the shaft a second storey would land in, and it is in the data now so the
position is decided before the code exists.

## Reusing this as the NPC home

**It is already a one-line reference, and that line already exists.**
`HomeStructure::tile_home_clones` in `src/ship/home_structure.rs` fills the
residential district by baking the player's own shell and stamping it into every
slot, choosing a design per slot from `home_design_roster()`, which today returns
exactly one design: `self`. So improving the player's home improved every
neighbour's house in the same commit, with no copy and no second file.

That is the right answer for the SHELL, and it should stay a reference rather
than a copy, because a copy would fork the moment either was edited.

What it does not carry is everything inside. `tile_home_clones` copies walls and
structures and deliberately not zones, lights or machines, so a neighbour's
quarters is a correct floor plan with nothing in it. Two things would close that,
in order of value:

1. **Carry the lights.** They are already per-zone data with world positions, and
   a translated copy is the same arithmetic the walls get. A lit empty house
   reads far better than an unlit one.
2. **Carry a furniture subset.** Machines resolve through `MachineHome`, which is
   a separate file from the structure, so this needs a rule about which machines
   are personal (a player's own trading post, their drone) and which are
   architectural (a bed, a stove, a sink). The honest split is the `category`
   field that already exists: Furniture and Kitchen and Water copy, Power and
   Production and Displays do not.

Neither is in this pass. The brief asked what makes the home reusable as the
template, and the answer is that the mechanism is already a reference; what is
missing is that the reference carries only the bones.

## Files this touches

| File | What changed |
|---|---|
| `data/blueprints/ship_structure.ron` | The whole `home` zone: 27 interior walls, 43 placed lights, 7 structures, 23 room zones plus the 13 unchanged mothership districts. |
| `data/machines/home.ron` | Every instance repositioned and re-roomed; 5 new screens; the `standing_mirror` def; the arrays moved into the greenhouse and the mushroom room. |
| `data/machines/home_solo.ron` | The one-person variant retargeted onto the same rooms, with three screens and the mirror. |
| `data/rooms.ron` | New room types `dressing`, `hall`, `entry`; the appearance station moved from the wet room to the bedroom. |
| `data/blueprints/zone_types.ron` | New zone type `room_dressing`. |
| `src/lib.rs` | The character-station walk-up reads what a room's actions OPEN instead of matching two hardcoded room ids, which were never going to match. |
| `src/ship/room_types.rs`, `src/gui/state_types.rs`, `src/engine/home_meshes.rs` | `RoomTypeRegistry::action_pages` and `RoomBounds::action_pages`: the semantic half of a room's actions, separate from the labels, plus the test that the home has exactly one appearance room and one wardrobe room. |
| `src/machines.rs` | The test that every placed machine really stands inside the room it names, in both home files. |
| `src/engine/world_load.rs` | World entry prefers the home's authored spawn point over the largest room; the character-select avatar goes to the room that changes clothes. |
| `src/ship/home_structure.rs` | The shipped-home test now derives its bounds from the authored zones and checks every room, rather than pinning one room's coordinates. |
| `scripts/photograph-home.js`, `scripts/home-vantages.json` | The rig that took the pictures below. It boots the release build, enters the world, stands at each authored pose and captures the viewport. `node scripts/photograph-home.js`, or `--only 05-great-room` for one. |
