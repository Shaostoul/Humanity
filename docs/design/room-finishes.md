# Room finishes: trim, wall, floor and ceiling finishes, per room with a per-wall override

Status: **rung 1 built (2026-11-14)**: skirting and crown swept along every room's walls with
mitred corners, stopped at doors, from profile data, chosen per room and per wall side in the build
editor, with defaults by room purpose. Rungs 2 to 6 are planned below.

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
  (casing). A `kind` says where it is swept: `Skirting`, `Crown`, `Casing`; rung 2 adds `ChairRail`
  and `PictureRail`. Sizes are common trade sizes (skirting 95 to 170 mm tall, 15 to 20 mm thick;
  a plaster cove about 75 mm each way; a sprung crown about 95 mm drop).
- **Trim materials**: an id, a `look` (Paint, Wood, Metal), a base colour, roughness and
  metallic. The look picks the shader's material type; colours are data, independent of the
  app's theme (the theme colours the app's panels, never the world).
- **Room-purpose defaults**: rows of `data/rooms.ron` keys with the skirting, crown and material
  each gets, and a fallback row, so no room needs setting by hand. Living rooms and bedrooms: a
  torus skirting and a plaster cove in white enamel; a study: an ogee skirting and a sprung crown
  in oak; kitchen and bathroom: a plain square-edge board, no crown; working rooms: a rubber cove
  base; gardens and growing rooms: nothing.
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

Both are left out of the file while empty, so every home saved before finishes is written byte
for byte as before.

**Resolution, field by field**: the wall side's choice, else the room's, else the room purpose's
default, else the fallback. So a wall side can change only the material and keep the room's
profiles. An id the catalog lacks falls to the fallback's (the editor names it in a warning line).

The perimeter shell's four inner faces take their rooms' finish; a per-edge override for them is
rung 2 (`HomeStructure::shell_finish`, four optional choices), since the editor does not select
the shell as a wall today.

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

**Openings.** The skirting stops square at a door's jambs (and at a window that reaches down into
it, a glazed door); the crown stops only where an opening reaches up into it. A corridor's cut
through the shell is a door to the trim. A stopped end gets an end cap (the cross-section filled
from the wall corner). Rung 2's casings then stand in those gaps and the skirting dies into their
plinth blocks.

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
  shows *Room finish*: three pickers (Skirting, Crown, Trim material), each starting at "Usual for
  this room: <name>", then None, then the catalog's rows; a drawing of the two cross-sections at
  scale in the trim's colour; and *Use the usual choices* to take every choice back. The home
  rebuilds on each change, so the 3D view is the full-size preview.
- **A wall**: select it. The panel shows *This wall's finish*, one block per side headed "Side
  facing <room>", each picker starting at "As the room: <name>", with its own drawing.
- **An AI or a person editing data**: the same fields in the home's RON (`finish: (skirting:
  Some("ogee_170"))` on a zone, `finish: (left: (material: Some("oak")))` on a wall). The editor's
  save writes them; the shipped homes set none and take the defaults.

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

At a terrace of homes (Row Street) the neighbours' coarse trim adds a few thousand vertices each,
merged into the same few draws. Later rungs keep the same rule: every finish is baked, merged by
look and drawn at a lower detail for a neighbour; wall and floor finishes are shader patterns
over the existing wall and floor meshes (tile and grout, plank seams from data widths), not new
geometry, so they cost per-pixel time, measured per rung, and no vertices.

## 9. The rungs

1. **Skirting and crown** (built 2026-11-14): this document's sections 2 to 6.
2. **Casings, sills, rails, and the shell's override**: door and window casings swept round each
   opening with mitred heads and plinth blocks the skirting stops on; window sills (a stool and an
   apron); chair and picture rails at their heights, stopped at openings; a per-edge finish for
   the perimeter shell. Budget: about 1.5x rung 1's vertices.
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
