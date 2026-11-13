# Pulleys: the block and tackle as real gear

**Status:** planned whole 2026-11-13; rung 1 built the same day (the model, the data, the
Calculator page's Pulleys mode with its schematic, the geometry and collision generator, the
Library page). Sources: [the dated finding](../reference/findings/2026-11-13-pulleys-and-rigging.md)
(F1 to F12 below).

The operator, 2026-11-12, with a photo of a handheld two-block set, three sheaves a block: "we
should have a piece of gear be pulleys like this hand held picture." Then: "we want a pulley that
makes me life something 1,000 times heavier and it auto calculates the dimensions of everything",
drawn "exclusively with math, no .obj like file" with good physics shapes. And: "If possible let's
recommend the best configuration for the scale. Like a human has to use this thing. Or a truck can
use it. Or spaceship."

## 0. What already existed (searched first)

- **The Library guide** "Force, Levers and Mechanical Advantage"
  (`docs/user/skills/force_levers_and_mechanical_advantage.md`, copied into `data/library/` by
  `node scripts/build-library.js`; the in-app copy is not under `docs/accord/`, it is listed in
  `data/library/catalog.json`) already taught counting parts of rope, multiplying stacked tackles,
  the Army's 10 percent friction rule and a rope factor of safety of 4. Its "How the game models
  it" said "there are no pulleys". Rung 1 enhances that section rather than writing a new page.
- **Data rows:** `data/components.csv` has a `pulley` component ("Grooved wheel for rope or
  belt"), `cable` (wire rope) and `rope_component`; `data/items.csv` has `rope_0` (hemp, 10 m);
  `data/structures.csv`'s portcullis needs a `winch:1`; `data/tech_tree.ron` names pulleys under the
  wheel. None of them modelled advantage. Rung 3 builds the gear item on these rows.
- **Carrying:** `crates/humanity-systems/src/encumbrance.rs` (the carrying limit follows gravity;
  Forgiving and Realistic modes). Rung 4 routes a lift through it.
- **Tube code:** the only swept tube is the renderer's `Mesh::polytube_data`
  (`src/renderer/mesh.rs`), one unwelded ring pair per segment, in the root package where the
  systems crate cannot reach it; the pipe and cable spline routing's own tube meshing is its rung 8,
  not built. So the pulley geometry carries a small welded, parallel-transported tube sweep of its
  own (`MeshData::add_tube`), which the spline routing can adopt when it reaches rung 8.

Nothing in `docs/FEATURES.md` or `docs/STATUS.md` named pulleys, tackles or winches.

## 1. The physics

### 1.1 Ideal advantage

A **stage** (a simple tackle, F12: "uses one rope and one or more blocks") is one rope reeved
between a fixed block and a moving block. Its ideal mechanical advantage is the number of parts of
rope at the moving block (the Navy rule the Library guide already teaches). A **compound** system
(F12: "more than one rope with two or more blocks") stacks stages block on block: the moving block
of stage 2 takes the hauling part of stage 1, and the advantages multiply.

### 1.2 Friction compounds per sheave

Raising, each sheave passes on only its bearing's efficiency e of the rope's pull. With the hauling
part leaving the fixed block (the reeving of Crosby's tables), a stage of n parts runs over n
sheaves, its parts carry P e, P e², ... P eⁿ, and

    real advantage = e + e² + ... + eⁿ = e (1 - eⁿ) / (1 - e)

which is always below n and never above e / (1 - e) however many parts are added: 24 for bronze
bushings, 49 for roller bearings. This is why big advantages need compound systems: four parts on
four parts (16 ideal) loses less than one sixteen-part tackle.

The bearing rows (`data/pulleys/pulleys.ron`, `bearings`):

| Bearing | e | Source |
|---|---|---|
| Plain bore | 0.909 | F1 compounded (the Army's 10 percent of the load per sheave, 1 / 1.10); F2 calls it "high bearing friction" |
| Bronze bushed | 0.96 | F6, Crosby Ratio A for one part |
| Anti-friction (roller) | 0.98 | F6, Crosby Ratio B for one part |

The model reproduces Crosby's printed ratios for 1 to 8 parts within 2 percent in both columns
(the test `real_advantage_is_below_ideal_by_the_friction_model`). The Army's rule taken literally is
linear (pull = load × (1 + 0.1 × sheaves) / parts) and is the more pessimistic field rule; the
Library guide teaches it as such.

### 1.3 Rope taken in

Rope pulled = ideal advantage × lift, exactly, whatever the friction. Friction costs force, never
rope: a 1,000:1 system takes in 1,000 m of rope for every metre the load rises, and the friction
means you also pull harder than one thousandth of the load. Work in = pull × rope; work out =
weight × lift; their ratio is the system's efficiency (real / ideal).

### 1.4 The gear's own weight and long falls

In the Realistic mode each stage also lifts its moving block's mass and half the weight of the rope
hanging in its falls (each part hangs between the two blocks, so half its weight bears on the lower
one; our approximation, conservative for horizontal compound stages). The block masses come from
the geometry (section 3), so the model settles them in a few rounds; the rope's mass per metre
comes from its row (F3, F6). At long falls this is what bends the numbers: the 45 t example below
carries 188 kg of rope.

### 1.5 Strength: working load limits and design factors

- **Rope:** working load limit = k × d², the Army's rules of thumb converted to SI: manila
  13.79 N/mm² (F3, "SWC ... equal to the square of the rope diameter"), 6×19 wire rope 110.3 N/mm²
  (F4, "multiply by 8"). Breaking strength = WLL × the rope's design factor: 4 for fibre (F3), 5 for
  running wire rope (F9, our pairing with F4).
- **Blocks:** sized for their load × 4 (F10, "Generally expressed for blocks as a ratio of 4:1").
  The fixed block carries the load plus the hauling part; the moving block carries the load.
- **The refusal (Realistic):** a stage whose hauling part (its highest tension while raising) is
  over its rope's working load limit is refused, with both numbers. The configurator sizes the
  rope itself unless the person fixes a size; with a fixed rope it looks for a tackle with enough
  parts to keep that rope within its limit.
- **The failure (Realistic, rung 5):** over the breaking strength the rope parts and over 4 × its
  rating a block fails.

### 1.6 Bend ratio

Sheave tread diameter = the rope's sheave ratio × d: about twice the circumference for fibre line,
2πd (F5), and "never less than 20 times the wire-rope diameter" for wire rope (F4, F5). A fibre
block is about three times the line's circumference long (F5); the generator's block comes out
near that from its parts, and never shorter.

### 1.7 Winches and geared hoists

A winch's rating holds on the first layer of rope only and can fall by as much as half on the last
(F8). Geared chain hoists: spur gear about 85 percent efficient, screw gear 50, differential 35
(F11).

### 1.8 Stretch (rung 2)

Rope stretch needs each rope's elongation at its working load from a maker's table; none was read
yet (the finding, "still unknown"). Until then the Realistic mode has friction, weight and limits
but no stretch.

### 1.9 Simple, compound, complex

Rung 1 models simple and compound systems. **Complex** systems (a stage whose moving block is not
on the next stage's hauling part, such as the Spanish burtons, where one rope's parts pull on
another's block in the same stage) are rung 6: their advantage is worked out by tension balance on
each block rather than by multiplying stages.

## 2. The configurator

`humanity_systems::pulley::recommend` (`crates/humanity-systems/src/pulley/tackle.rs`).

**Inputs:** the load's mass, the lift height, the gravity (Earth on the Calculator page; the
world's own in the game), the puller (a data row: its force, its speed at that force, its stroke,
whether it is a person), the rope (a data row), the bearing (a data row), the mode, and optionally
a rope size the person fixed.

**The puller rows** (`pullers`): one person, 444.8 N (F7, "about 100 pounds per man" on a
vertical pull); four people on a horizontal line, 4 × 266.9 N (F7, 60 pounds each); a truck's
winch, 53.4 kN; a ship's winch, 150 kN; the fleet's hangar hoist, 1.5 MN. Speeds, strokes and the
three machines' forces are **design values** (the truck winch maker's page refused automated
reading), marked so in the data and on the screen's sources line.

**The search:** every plan of up to `max_stages` stages (4) of up to `max_parts_per_stage` parts
(8, two quadruple blocks), about 2,800 plans. Each is checked first by friction alone (cheap), then
in full (the gear's weight, the rope sized or checked). The first plan that the puller can pull and
whose ropes are within their limits wins, in this order: **fewest stages** (the simplest rigging),
then **fewest sheaves**, then **least rope taken in**, then **the shortest throw for the last
stage** (in a compound system the last stage travels the lift times every earlier stage's
advantage). When nothing within the limits does it, the strongest plan is shown and the Realistic
mode refuses with how far short it falls.

**Outputs:** each stage's parts, sheaves above and below, rope and sheave diameters, block length,
travel, span and rope length; the ideal and real advantage and the efficiency; the pull; the rope
pulled; the time (rope pulled over the puller's speed); the rope needed and its mass; the blocks'
mass; the verdict (straight lift, tackle, beyond a tackle); the advice; the refusal.

**What it recommends, by puller** (Realistic, bronze bushings, from the model on 2026-11-13):

| Lift | Puller | Rope | System | Real of ideal | Pull | Rope pulled | Time |
|---|---|---|---|---|---|---|---|
| 1,000 kg, 3 m | one person | manila | 5 × 6 | 22.8 of 30 | 431 N | 90 m | 6 min |
| 1,000 kg, 3 m | four people | manila | 3 × 4 | 9.8 of 12 | 997 N | 36 m | 2.4 min |
| 1,000 kg, 3 m | truck winch | wire | straight (one block) | 0.96 | 10.2 kN | 3 m | 3.3 min |
| 100 t, 3 m | truck winch | wire | 5 × 5 | 18.9 of 25 | 51.9 kN | 75 m | 83 min |
| 100 t, 3 m | ship's winch | wire | 8 | 6.6 of 8 | 150 kN | 24 m | 1.6 min |
| 100 t, 3 m | hangar hoist | wire | straight (one block) | 0.96 | 1.02 MN | 3 m | 30 s |
| 45 t, 1 m | one person | manila | 8 × 5 × 7 × 8 | 1,013 of 2,240 | 436 N | 2,240 m | 2.5 h |

The last row is the operator's 1,000 times: one person, who can hold up about 45 kg, lifting 45 t
one metre, through four tackles stacked block on block. The real advantage is about 1,000, but the
ideal is 2,240, so 2,240 m of rope passes through the hands (55 percent of the effort goes to
friction), it takes two and a half hours, and the blocks weigh 3.1 t. That is the trade, said in
words on the screen: "for every metre 45.0 t rises, 2240 m of rope is pulled."

**When something else wins** (`Advice`), said plainly beside the result:

- a person who needs two or more stacked tackles is told a geared chain hoist does it with far less
  rope, with the hoist's efficiency (F11) beside the tackle's;
- a person whose lift takes more than ten minutes is told more people on a line along the ground
  (F7), or a winch, would be quicker;
- a winch pulling more than half its rating is told the rating holds on the first layer only (F8);
- the number of strokes (resets) when the puller's stroke is shorter than the rope pulled;
- beyond any tackle (the 100 t person case): "Use a winch, a geared hoist or hydraulics."

## 3. The geometry from math

`crates/humanity-systems/src/pulley/geometry.rs`. No mesh file is read; every dimension comes from
the configurator's numbers.

**The block's frame:** the pin along X, the sheaves turning in the Y-Z plane, Y along the pull,
the origin at the pin's centre; the fixed block's head (its shackle) points up, the moving block's
down.

**The sizing** (strength-of-materials rules, our derivation; the material's tensile strength and
density from `data/materials.csv`, steel by the data's `block.material`):

| Part | Rule |
|---|---|
| Sheave tread D | the rope's sheave ratio × d (section 1.6) |
| Flange | 1.5 d above the tread; sheave width 1.8 d; groove a round bottom of 0.55 d |
| Ultimate load F | working load × 4 (F10) |
| Pin d_p | the larger of double shear at 0.6 × tensile strength and bending across the sheave stack (M = F L / 8) |
| Side plates | each carries F / 2 across the net section beside the pin hole, and in bearing at 1.5 × tensile strength; 2 mm at least |
| Partitions | half a side plate, between neighbouring sheaves |
| Shackle | two legs with a bend factor of 2 (π/4 d² σ = F); pin 1.15 × the bar |
| Length | the plates' width plus 2.5 shackle pins for the head, or the rope's block-length rule if longer |

So a block for 100 t is a different object from a block for 1 t in every proportion (the test
`block_sizes_follow_the_load`): the example above gives a 1,000 kg person's blocks 190 mm long on
22 mm pins, and a 100 t ship's winch blocks 1.36 m long on 155 mm pins, each block about a tonne.

**The mesh:** side plates and partitions are boxes; sheaves and pins are lathed (a profile turned
about the pin: bore, flange, the grooved tread, flange); the shackle's bow and every rope are tubes
swept along their centre lines with parallel-transported rings (no twisting, welded joints, smooth
at sampled curves). The rope is reeved as a rigger reeves it: up the first fixed sheave's +Z side,
over it, down the -Z side to the first moving sheave, under it, and so on, the wraps sampled as
arcs at the pitch radius, the dead end made fast to the other block's facing end.

**Collision:** one box per block body and one per shackle, from the same parameters; one capsule
per straight run of rope (the wraps lie inside the block boxes). The test
`collision_shapes_enclose_the_geometry` checks every vertex of every block and rope.

**Built once per configuration:** `ShapeStore` keeps the last built shapes under a key hashed from
every number they were built from (a version of the parameters), builds again when any number
changes, and builds every time it is asked while the reuse switch is off
(`humanity_core::reuse::enabled()`, the Dev page; CLAUDE.md, "No caches kept across runs"). The
Calculator panel's last answer obeys the same switch.

## 4. Use in the world

- **The gear item (rung 3):** a block-and-tackle kit as `data/equipment.csv` rows by size class
  (hand, vehicle, ship, hangar), a `data/recipes.csv` recipe each (steel for blocks and pins by the
  geometry's mass, rope by the configurator's length), reusing the `pulley` component and the rope
  rows. The item stores its configuration (the stage plan, the rope and its size), never a mesh; the
  mesh is built from it where it is seen.
- **Anchors (rung 3):** a tackle needs a fixed point rated at least the fixed block's load (the
  load plus the hauling part, `anchor_n`): structure rows gain an `anchor_wll_n`, trees by trunk
  diameter, and the ship's hardpoints by data. An anchor under its load refuses in Realistic.
- **Lifting and dragging (rung 4):** through the carry rules (`encumbrance.rs`): a load too heavy
  to carry can be hooked to the moving block; the player's pull is the tackle's `pull_n`, which must
  be within the player's force row, and the rope taken in moves the load. Dragging along the ground
  adds the surface's friction coefficient to the load (rung 4 sources it).
- **The Library page:** "Force, Levers and Mechanical Advantage" now points at the Pulleys mode
  and states the friction model (rung 1).

## 5. Dual modes

- **Realistic** (the default on the Calculator page): friction per sheave by bearing, the gear's
  own weight, working load limits that refuse a lift with the reason; rope stretch (rung 2) and
  failure over the breaking strength (rung 5) join it.
- **Simplified:** the ideal machine, advantage = parts, no weight, nothing refused or failing. The
  game's setting joins `data/gameplay/levels.json` and Settings > Gameplay in rung 3.

## 6. Where it lives (native first, with the web mirror)

**The Calculator page's Pulleys mode** (`src/gui/pages/calculator/pulleys.rs`), chosen over the
alternatives in `docs/PAGES.md`: it is a calculator for a real job, so it belongs on a real-life
page (the two realities rule: the app's pages are your real life; the game's item opens the same
panel from inside the world, rung 3); it sits in the Tools menu beside the arithmetic; and the
website's Calculator already has modes, so the mirror joins an existing switch. The Crafting page
was the other candidate and was rejected: it is where gear is made, and designing a lift is not
making it (the item's recipe will link to the panel instead).

Every word is a key under `pulley.` and `calculator.mode_*` in `data/i18n/en.json`, every control
has a hint, jargon is marked for the glossary (mechanical advantage, sheave, working load limit
were added to `data/glossary.json`), and the schematic draws in theme tokens only. The schematic
draws the beam and the ground, each block with its sheaves, every part of rope, the load and the
pull; even-numbered stages hang from the beam and odd ones are anchored to the ground, because each
stage must pull the one before's hauling part away from its block.

**The web mirror is rung 2.** The model is Rust, and a second implementation in JavaScript would
drift. The website gets the same Pulleys mode in its Calculator with the model compiled to
WebAssembly (the `wasm` feature), or, if that build is not ready, a JavaScript port checked against
a fixture of cases the Rust tests write out. Until then the web's Calculator has no Pulleys mode,
which this paragraph records (CLAUDE.md, dual-UI parity).

## 7. Budget

The configurator runs when an input changes (and every frame only while the reuse switch is off):
about 2,800 plans, most rejected by one multiplication, a handful designed in full; well under a
millisecond. A shape is built once per configuration. Nothing is networked but the configuration
itself (a stage plan, a rope id and size: a few bytes); every client builds the mesh from it.

## 8. The rungs

1. **The model and the designer (built 2026-11-13).** The pure model in
   `crates/humanity-systems/src/pulley/`; the rows in `data/pulleys/pulleys.ron`; the Calculator's
   Pulleys mode with its schematic; the geometry and collision generator with its store; the
   Library page; the finding. Tests seen red first (section 9).
2. **The web mirror and better rows.** The website's Pulleys mode (section 6); synthetic rope rows
   (polyester, nylon, HMPE) and rope stretch from makers' tables; sourced pulling speeds; "the room"
   input (how far the hauling end can travel, so overhauls are counted); an option to prefer lighter
   gear (more parts, thinner rope) over fewer sheaves.
3. **The gear in the world.** The equipment and recipe rows; the generated meshes uploaded and
   drawn (`MeshData` straight into the renderer's vertex format); anchors with ratings; the mode in
   Settings > Gameplay and the gameplay levels.
4. **Lifting and dragging through it.** Hooking a load, the pull through the carry rules, the rope
   taken in moving the load; the rope as a swinging, stretching physics rope built on the same
   centre line.
5. **Failure.** A rope over its breaking strength parts and a block over four times its rating
   fails (Realistic): the load drops, the danger zone under a suspended load is drawn, and the
   Library guide's rules about standing clear are taught by the world.
6. **Complex systems and machines.** Tension-balance solving for complex reeving; chain hoists, a
   capstan and winches as gear of their own, sharing this physics.

## 9. Tests (rung 1)

In `crates/humanity-systems/src/pulley/tests.rs`, each seen red against a wrong model first (the
evidence is in each test's comment): the shipped rows parse; real advantage below ideal and equal
to Crosby's ratios within 2 percent; rope pulled = ideal advantage × lift in every mode and bearing;
the recommendation follows the puller (a person's 1,000 kg differs from a truck's and a ship's, and
at 100 t the truck's differs from the ship's); a rope over its working limit refused in Realistic
with the numbers and lifted in Simplified; block sizes following the load; collision shapes
enclosing every vertex; shapes built once per configuration unless reuse is off. The page is pinned
by `just snapshot calculator_pulleys`.

## 10. Open questions for the operator

1. **A person's pull.** The Army's 100 pounds (445 N) is a soldier hauling hand over hand on a
   vertical line. Should the designer offer a lighter "anyone" row (a child, an older person) so it
   does not overpromise? Recommendation: yes, as data rows, once a primary source for them is read.
2. **The ranking.** Fewest stages, then fewest sheaves, is how riggers keep it simple; at hangar
   scale it picks one huge single block on very thick rope. Recommendation: keep it, and add the
   rung 2 "lighter gear" choice.
3. **Gravity on the Calculator.** It assumes Earth. Recommendation: a world choice beside the load
   (Earth, Mars, the Moon, the ship's spin), read from `data/planets/`.

## His answers (operator, 2026-11-13)

Lighter pullers (an older person, a child) once a primary source is read, and a planet's gravity chosen beside the load: "Sure."
