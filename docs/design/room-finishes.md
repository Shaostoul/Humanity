# Room finishes: trim, wall, floor and ceiling finishes, per room with a per-wall override

Status: **rungs 1 and 2 built**. Rung 1 (2026-11-14): skirting and crown swept along every room's
walls with mitred corners, from profile data, chosen per room and per wall side in the build
editor, with defaults by room purpose. Rung 2 (2026-11-15): a casing round every door and window
on each face, mitred at the head, a stool and apron under each window, the skirting stopped
against the casing's edge, chair and picture rails stopped at the casings, and the outer walls'
inside faces chosen per wall like an interior wall's side. Rungs 3 to 6 are planned below.

The operator, 2026-11-14: "What else could we do to improve the look of rooms? What about like
floor and ceiling trim? Should we make it easily configurable per wall or room or something?"

The rule this plan follows is CLAUDE.md's realistic-first: real swept trim geometry with mitred
corners and proper materials, the way a joiner fits it, not a texture painted on the wall or a box
stood against it.

## 1. What a finished room has, and what we model

A real room is finished in layers, each its own trade:

| Layer | What it is | Rung |
|---|---|---|
| Skirting (baseboard) | A board along the foot of every wall, covering the gap between the floor and the wall, mitred at corners, stopped against door casings | 1 |
| Crown (cornice, coving) | A moulding where the wall meets the ceiling, mitred at corners | 1 |
| Door and window casings, sills | The board framing each opening, mitred at the head, sitting on a plinth block or stopping on the skirting | 2 |
| Chair rail, picture rail | A moulding across the wall at about 0.9 m (chair) or 0.3 m under the ceiling (picture) | 2 |
| Wall finish | Paint (a colour), wallpaper (a pattern), tile with grout, wood panelling (tongue and groove, wainscot to the chair rail) | 3 |
| Floor finish | Planks (real board widths and a laying direction), tile with grout, carpet, sheet | 4 |
| Ceiling finish | Plain plaster, exposed beams, coffered | 5 (coffered and roses later) |
| The real-life twin | Planning a real room's finishes: lengths of each moulding, areas of each finish, a shopping list | 6 |

## 2. Data: everything is a catalog row (infinite-of-x)

One file, `data/blueprints/room_finishes.ron` (read by
`crates/humanity-ship/src/room_finishes.rs`), holds:

- **Profiles**: a cross-section as (out, up) points in metres, the visible face only, from where it
  meets the wall to where it meets the floor (skirting), the ceiling (crown) or the opening's edge
  (casing). A `kind` says where it is swept: `Skirting`, `Crown`, `Casing`, `Sill` (a window's
  stool), `ChairRail`, `PictureRail`. A board fixed to the wall along both edges (a casing, a
  rail, a stool) runs from the wall at one edge round to the wall at the other, so its edge facing
  the opening, or its underside, is drawn too. Sizes are common trade sizes (skirting 95 to 170 mm
  tall, 15 to 20 mm thick; a plaster cove about 75 mm each way; a sprung crown about 95 mm drop;
  a casing 58 to 70 mm wide; a stool 22 to 25 mm thick standing about 45 mm off the wall; a chair
  rail 45 to 58 mm, a picture rail 45 mm tall).
- **Fit**: where the trim not tied to a floor, a ceiling or an opening goes: a chair rail centred
  0.9 m up, a picture rail's top 0.3 m under the ceiling, a stool's horns 20 mm past the casing.
- **Trim materials**: an id, a `look` (Paint, Wood, Metal), a base colour, roughness and
  metallic. The look picks the shader's material type; colours are data, independent of the
  app's theme (the theme colours the app's panels, never the world).
- **Room-purpose defaults**: rows of `data/rooms.ron` keys with the skirting, crown, casing, sill,
  rails and material each gets, and a fallback row, so no room needs setting by hand. Living rooms
  and bedrooms: a torus skirting, a plaster cove and an ogee casing with a bullnosed stool, in
  white enamel; a study: an ogee skirting, a sprung crown and the ogee casing in oak; kitchen and
  bathroom: a plain square-edge board, no crown, a pencil-round casing and a square stool; working
  rooms: a rubber cove base and the openings left in their steel; gardens and growing rooms:
  nothing. The rails are off in every room until someone chooses one (a rail at 0.9 m would stand
  into the furniture placed flush against the skirting).
- **Names** are words by key: `finishes.profile.<id>` and `finishes.material.<id>` in
  `data/i18n/en.json` (a test fails when a row has no words).

Later rungs add rows to the same file: `wall_finishes` (paint colours, wallpapers, tile with grout
width and colour, panelling), `floor_finishes` (planks with board width and length, tile, carpet),
`ceiling_finishes`, each with its own room-purpose defaults (a kitchen's tile splashback to 0.6 m
over the worktop, a bathroom's tile to 1.2 m and full height in the shower).

## 3. The choice, and how it resolves

A choice is a `Finish`: each field (skirting, crown, material; later wall, floor, ceiling) is a
catalog id, the word `none`, or left open. It is stored where the thing it finishes lives:

- **A room's** on its zone: `Zone::finish`. In the box home a room is the region its walls
  enclose, and the zone that covers it is its identity (its id, its label, its `room_type`), so the
  finish moves, copies and goes away with the zone.
- **A wall side's** on its wall: `InteriorWall::finish`, a `WallFinish` with `left` and `right`
  (left: on your left walking from the wall's `a` to its `b`, seen from above with x east and z
  south).
- **An outer wall's inside face** on the home (rung 2): `HomeStructure::shell_finish`, a
  `ShellFinish` with `north` (the z = 0 edge), `east`, `south` and `west`, each over the rooms
  along it as an interior wall's side is over its room.

Both are left out of the file while empty, so every home saved before finishes is written byte
for byte as before.

**Resolution, field by field**: the wall side's choice, else the room's, else the room purpose's
default, else the fallback. So a wall side can change only the material and keep the room's
profiles. An id the catalog lacks falls to the fallback's (the editor names it in a warning line).

The perimeter shell's four inner faces resolve through `shell_finish` first (rung 2), then their
rooms'. The editor does not select the shell as a wall, so its choices sit under the room's, one
block for each outer wall the selected room runs along (section 6).

## 4. Geometry: the swept trim (rung 1, `crates/humanity-ship/src/room_trim.rs`)

**Finding the faces.** On each storey every wall standing in it becomes a footprint polygon in the
floor plan: each interior wall exactly as the wall mesh draws it (its mitred, clipped or squared
ends, from the one function both read, `HomeStructure::wall_feet`), the perimeter shell's pieces
round its open spans, the corner columns. The trim runs along the **boundary of their union that
faces into the box**: every polygon edge is split where any other polygon's edges cross it, and a
piece is kept when a point 0.2 mm off it, on its outward side, is inside no other polygon and
inside the box. A wall meeting another mid-span, a T, an X, a free end, a corner column and the
shell all come out as one closed outline per room, and a corner is where two kept pieces meet. A
glass wall and a guard stand in the union (the trim stops against them) but carry none.

**Which room a face is in.** The smallest zone of its storey holding a point 5 cm in front of it.
Upstairs, a face over no slab (a stairwell) or in no zone carries no trim.

**Mitres.** Each profile point at offset `o` from the wall sits, at a corner, where the two faces'
lines offset by `o` cross. Inside and outside corners at any angle come out of the one formula, and
two runs meeting at a corner share every vertex exactly (the test reads them). A corner sharper
than about 14 degrees stops the mitre at 8 times the offset; a run shorter than its two inside
mitres collapses to a wedge rather than turning inside out.

**Openings.** Every horizontal trim stops square where an opening's trim crosses its height band:
at the casing's outer edge where the face cases that opening (rung 2), with the stool's horns
where the band takes in a window's stool, else at the opening's own edge. So the skirting stops
against a door's casing (and a glazed door's), a chair rail against a door's casing and a window's
stool, and the crown only where an opening reaches up into it. A corridor's cut through the shell
is a door to the trim. A stopped end gets an end cap, the cross-section filled by ear clipping
(`room_finishes::triangulate`, which the editor's drawing fills with too), so a beaded skirting or
a picture rail's hook caps cleanly.

**Casings (rung 2, `crates/humanity-ship/src/room_casings.rs`).** Each opening is cased on each
face whose piece of the outline holds its middle and whose finish has a casing. In the face's plane
(metres along the wall's line, height) a door's casing is three parts: the legs from the floor up
the jambs and the head across, each the casing profile with its `up` axis pointing away from the
opening, MITRED at the two head corners by the same offset-line formula the plan mitres use (the
two parts' lines at each profile offset cross on the 45-degree diagonal, so the leg's end and the
head's start share every point). A window whose foot is 0.3 m or more over the floor stands on its
stool: the legs start at the opening's foot, the stool's top is level with it and runs the casing's
width and its horns past each jamb, the apron (the casing board) hangs under it as wide as the
casing outside to outside, and is left off where it would reach down into the skirting. A window
whose face has no stool ("none") is framed on all four sides, mitred at every corner. Near a
corner, the crown or the ceiling the casing is ripped narrower (its profile sawn at the room left,
a sawn edge from the wall to the face), two openings closer than two casings butt halfway between
them, and under 30 mm left a casing is not fitted on that face, the skirting then stopping at the
jamb as before. Casings take the face's material, so a wall side's
oak cases its doors in oak.

**Rails (rung 2).** A chair rail is swept like the skirting at its height (centred 0.9 m over the
floor) where the wall stands that high; a picture rail with its top 0.3 m under the ceiling where
the wall reaches it. Both mitre at the corners and stop at the casings (above).

**Heights.** The skirting sits on the storey's floor (the slab's drawn top upstairs). The crown
sits under the ceiling over that face: the lowest slab underside above the floor, else the roof;
only where the wall reaches the ceiling (a wall stopping short of it is a partition and has none).

**Normals and winding.** Each profile segment's normal is its (-du, do) carried into the world;
neighbouring segments under 40 degrees apart share a smoothed normal (an ogee reads as a curve),
sharper ones stay crisp (a fillet reads as an edge). Every triangle is wound to face its normal
(the opaque pass culls back faces).

**Detail.** The player's own home bakes every profile point. A neighbour's home bakes each
profile's bounding outline (its top and its face, two quads a run), since its rooms are seen from
afar, through a door or a glass roof.

## 5. Drawing it: the existing shell families, no new per-frame work

The trim is part of the home's shell bake (`HomeStructure::generate_meshes_with_shell_cuts`), run
when the home is built or edited, never per frame. `HomesteadMeshes::trim` is one group per trim
material (`TrimGroup`: mesh plus look), merged by look across the ship's zones and across every
neighbour's home, so the number of draws is the number of trim materials in use (two to four), not
the number of rooms or homes. The engine fills one slot per group in the home's shell families
(`engine/home_meshes.rs`) and draws them in the opaque pass with the walls; they cast into the sun's
shadow map and the local lights' as the walls do.

The showcase's `{"room_trim":"0"}` bakes the homes without trim, `"1"` with it: the same-boot red
twin for screenshots and for timing what the trim costs. Permanent dev tooling.

## 6. Configuring it: GUI-first, in the build editor

- **A room**: select its zone (click its box in the build view). Under its room type the panel
  shows *Room finish*: seven pickers (Skirting, Crown, Casing, Window sill, Chair rail, Picture
  rail, Trim material), each starting at "Usual for this room: <name>", then None, then the
  catalog's rows; a drawing of every cross-section at true scale in the trim's colour (the
  skirting and crown against the wall, the rails, the casing seen from above beside its opening,
  the sill with its apron); and *Use the usual choices* to take every choice back. The home
  rebuilds on each change, so the 3D view is the full-size preview.
- **An outer wall** (rung 2): under the room's block, *Outer walls of this room* (folded until one
  has a choice) holds one block per outer wall the room runs along ("North outer wall"), each
  picker starting at "As the rooms: <name>". A choice there sets that wall's whole inside
  face, every room along it.
- **A wall**: select it. The panel shows *This wall's finish*, one block per side headed "Side
  facing <room>", each picker starting at "As the room: <name>", with its own drawing.
- **An AI or a person editing data**: the same fields in the home's RON (`finish: (skirting:
  Some("ogee_170"), chair_rail: Some("torus_dado_58"))` on a zone, `finish: (left: (material:
  Some("oak")))` on a wall, `shell_finish: (north: (picture_rail: Some("none")))` on the home). The
  editor's save writes them; the shipped homes set none and take the defaults.

Rung 3 adds the wall, floor and ceiling pickers to the same panel, with swatches.

## 7. The two realities

- **The game twin** is what rung 1 builds: the rooms of the home you walk.
- **The real-life twin** (rung 6) plans a real room's finishes with the same catalog and the same
  resolution: the person draws or measures their room (or picks it from the possessions and place
  pages), chooses a skirting, a crown and the wall, floor and ceiling finishes, and gets the
  quantities a builder's merchant sells by: linear metres of each moulding with the usual 10%
  waste and the number of standard lengths, the count of inside and outside mitres, square metres
  of each finish, litres of paint at the coverage on the tin, boxes of tile. The trim outline this
  rung computes already gives the lengths and corners.

## 8. What it costs, and how it stays in budget

Measured at rung 1 (2026-11-14; the numbers are recorded in the rung's report and in
`room_trim_tests.rs` `shipped_homes_bake_their_trim_within_budget`):

| | Cost |
|---|---|
| Bake, the homestead (2 storeys, 29 rooms), full detail | 15,518 vertices; about 10 ms of CPU in a debug test build at a build or edit, nothing per frame |
| The other shipped homes | 384 (the cabin) to 9,552 (the household homestead) vertices |
| Video memory | 32 bytes a vertex plus 4 an index: about 0.6 MB for the homestead |
| Draws | one per trim material in use (2 to 4), shared by every zone and neighbour |
| A neighbour's home | its trim at the bounding outline: 2,928 vertices for a homestead, about a fifth |
| Budget the test holds | 60,000 vertices per home at full detail, 15,000 coarse |

GPU time at the home vantages, measured same-boot with the showcase's `room_trim` switch (trim
off, then on, at the same pose), at midnight station time on the final rung 1 build (2026-11-15;
the sweep marked every capture as sharing the machine with other work, so these are readings to
re-take on an idle machine, though all three poses agree):

| Vantage | GPU, all passes | Main pass (`gpu.scene`) | Lamps' shadow maps (`gpu.shadow_local`) |
|---|---|---|---|
| Great room | 23.25 -> 23.93 ms | +0.38 ms | 0.70 -> 0.93 ms |
| Upstairs bedroom | 22.26 -> 22.85 ms | +0.38 ms | 0.56 -> 0.82 ms |
| Kitchen | 26.70 -> 27.34 ms | +0.36 ms | 0.72 -> 0.91 ms |

About 0.6 ms, under 3% of the frame and under the 5% bar. Most of it is every lamp's shadow map
drawing the trim as it draws the walls; the trim stays in them (operator, 2026-11-15) while it is
under the bar. If a later rung pushes it over, the trim is split per room so a lamp draws only its
own room's (the walls' bake would split the same way).

**Rung 2** (2026-11-15, `shipped_homes_bake_their_trim_within_budget` prints the counts):

| | Rung 1 | Rung 2 |
|---|---|---|
| The homestead, full detail | 15,518 vertices | 22,424 (1.44x, the budget about 1.5x) |
| The household homestead | 9,552 | 15,078 |
| The row houses | (not recorded) | 6,626 to 10,194 |
| A neighbour's homestead, coarse | 2,928 | 5,384 |
| Bake, the homestead, debug test build | about 10 ms | 13 to 20 ms, at a build or edit only |
| Video memory, the homestead | about 0.6 MB | about 0.8 MB |
| Draws | one per trim material | unchanged: the casings, stools and rails join their face's material group |

GPU time, same boot, trim off then on at rung 1's three poses and a fourth that frames a door and
two windows (the dining room; the vantage named for the bedroom window lands there since the
layout changed), medians from the frame-cost files. Every capture shared the machine with other
agents' compiles, so these too are readings to re-take on an idle machine; noon and night agree
within 0.1 ms except one noon reading.

| Vantage | GPU, all passes (night) | Main pass | Lamps' shadow maps | Noon, all passes |
|---|---|---|---|---|
| Great room | 23.06 -> 23.93 ms (+3.8%) | +0.45 ms | 0.70 -> 1.13 ms | 23.02 -> 23.90 (+3.8%) |
| Upstairs bedroom | 22.09 -> 22.92 (+3.8%) | +0.34 | 0.56 -> 1.04 | 22.27 -> 23.16 (+4.0%) |
| Kitchen | 26.28 -> 27.19 (+3.5%) | +0.55 | 0.72 -> 1.09 | 26.19 -> 27.19 (+3.8%) |
| Dining room (door, two windows) | 19.77 -> 20.66 (+4.5%) | +0.41 | 0.69 -> 1.15 | 19.55 -> 20.83 (+6.5%) |

All the trim together costs about 0.9 ms, rung 2's share of it about 0.3 ms over rung 1's 0.6.
Under the 5% bar at every pose at night; one noon reading in the dining room went over (6.5%),
and the same pose at night read 4.5%. Half of the cost is the lamps' shadow maps (+0.4 to 0.5 ms
against rung 1's +0.2 to 0.26), so the per-room split named above is the next step if a clean
reading confirms the trim over the bar.

At a terrace of homes (Row Street) the neighbours' coarse trim adds a few thousand vertices each,
merged into the same few draws. Later rungs keep the same rule: every finish is baked, merged by
look and drawn at a lower detail for a neighbour; wall and floor finishes are shader patterns
over the existing wall and floor meshes (tile and grout, plank seams from data widths), not new
geometry, so they cost per-pixel time, measured per rung, and no vertices.

## 9. The rungs

1. **Skirting and crown** (built 2026-11-14): this document's sections 2 to 6.
2. **Casings, sills, rails, and the shell's override** (built 2026-11-15): door and window casings
   swept round each opening with mitred heads, the skirting stopping against the casing's edge;
   window sills (a stool and an apron); chair and picture rails at their heights, stopped at the
   casings; a per-edge finish for the perimeter shell. Budget: about 1.5x rung 1's vertices
   (section 8). Plinth blocks (a block at a casing's foot the skirting dies into, thicker than
   both) are left for a later rung as an optional row on a casing.
3. **Wall finishes**: paint colours (data, theme-independent), wallpaper patterns, tile with grout
   (tile size, grout width and colour as data; a kitchen splashback and a bathroom's tile by
   default), wood panelling (wainscot to the chair rail). Shader patterns keyed per wall face over
   the existing wall mesh; the per-face material split the wall bake already has (`by_mat`) grows
   a face key. Budget: no new vertices, the pattern's per-pixel cost measured at the vantages.
4. **Floor finishes**: planks with real board widths, lengths and a laying direction (staggered
   joints), tile with grout, carpet. Per-room floor patches from the room outline this rung
   computes. Budget as rung 3.
5. **Ceiling finishes**: plain, exposed beams (swept, like the trim), coffered and ceiling roses
   later.
6. **The real-life planner** (section 7).

## 10. Decided (operator, 2026-11-15, relayed by the orchestrator)

1. **The default house style stays**: a torus skirting and a plaster cove in white enamel in the
   living rooms, oak with an ogee skirting and a sprung crown in the study.
2. **The crown stays under a glass slab** (at its underside), since a person can turn it off per room.
3. **The wood grain takes the material's colour in rung 3**, measured (today the shader's grain is
   one oak-like tone whatever the colour, so an oak and a walnut would look alike).
4. **The trim stays in the lamps' shadow maps** while its cost is under the 5% bar (section 8).
