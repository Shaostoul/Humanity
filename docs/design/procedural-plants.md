# Procedural Plants: seed to senescence from numbers

> Status: DESIGN, not yet built. Written 2026-07-30, revised the same day after three
> adversarial reviews (engine, botany, scoping) corrected several load-bearing claims.
> Read `docs/design/infinite-of-x.md` first. This document is a worked example of that rule.
>
> **Scope warning.** This is the **crop-fidelity** arc, not "the plants arc". PRIORITIES'
> plants slot is about PLANET-SURFACE vegetation: grass instancing, tree instancing, and
> impostors replacing the rejected far-tree card sheet. That is what the operator actually
> asked for and what closes the "black squares in a grid" regression. This document addresses
> **homestead and garden crops**, which is the only thing `plant_mesh.rs` draws. It must not
> consume the planet-vegetation slot.

## The question

Plants that go from seed to seedling to sprouting all the way to senescence, for pumpkins,
strawberries, apples, redwoods, grapevines and everything else, including ailments: yellowing
leaves, insects eating them, burns. As close to real as we can get, with the whole plant
kingdom in a few GB.

The instinct that "fruit and leaf shape are probably the bare minimum art" is close but one
step short. **Leaf and fruit shape are also numbers.** The minimum art set is nearer zero
files than thirty.

## Why: coverage, not storage

The honest justification is **coverage**, and only coverage.

`data/plants.csv` has 134 species. `data/plants_visual.ron` has **19** authored recipes.
`assets/models/plants/` has models for **19** crop species. So **115 of 134 crops have no
art and no recipe at all**, and fall back to `generic_visual()` (`src/renderer/plant_mesh.rs:77`),
which exists specifically because the operator complained in v0.903 that "the potato garden is
just a plain slab of brown". No amount of GLTF sourcing scales to the infinite-of-X rule, and
hand-authoring 134 flat recipes does not scale to the species count this project implies.

**Do not argue this on storage.** An earlier draft of this document claimed the procedural
system replaces 183 MB of GLTFs. That is false by roughly 30x, and the measurement is worth
recording so nobody repeats it:

| Asset | Size | Actually replaceable by this design? |
|---|---|---|
| `fir_sapling` | 67 MB | **No.** Consumed by the near-tree PLANET path (`src/lib.rs:9202`, :9287, :9390). Different subsystem. |
| `pine_sapling_small` | 66 MB | **No.** Same. |
| `shrub_01` | 21 MB | **No.** Decor. |
| `potted_plant_01` | 20 MB | **No.** Decor. |
| 19 Quaternius crop species (4 to 6 stages each) | **5.6 MB** | Yes, in principle. |

So the real trade is a new organ atlas against 5.6 MB of removable GLTF. At RGBA8, which is
what this renderer actually uses, that atlas is a **net storage increase**. Say so plainly.

Two further measured facts that matter:

- The release bundle copies only `assets/icons/` and `assets/shaders/`
  (`.github/workflows/build-desktop.yml`), so **none of the plant art reaches a downloaded
  build**. The procedural path is already the only thing a player sees. That is an argument
  for investing in it, and also a reason the GLTF tier needs a shipping decision either way.
- The Quaternius pack is the cheapest working art pipeline in the repo: 19 species at 4 to 6
  stages for 5.6 MB, roughly 7 KB per model, one small palette PNG carrying all material
  colors. The cost problem is not "3D models are big", it is "photoscans are big".

## What already exists (do not rebuild any of this)

| Piece | Where | State |
|---|---|---|
| Procedural plant mesh generator | `src/renderer/plant_mesh.rs` (712 LOC) | WORKING. 6 form archetypes, **continuous** growth `t`, a `wilt` axis, deterministic seeding, `generic_visual()` fallback. |
| Per-species visual recipe | `data/plants_visual.ron` | WORKING, hot-reloadable. 19 of 134 crops authored. |
| Hero GLTF crop tier | `src/engine/home_meshes.rs:843-852` | WORKING. Bed and field crops with a model render at the growth quartile as material type 19. |
| Agronomic species data | `data/plants.csv` | 134 species, real USDA/FAO-derived numbers, including unused `ph_min`/`ph_max`. |
| Per-plant stress schema | `data/entities/plants/plant_001.ron` | AUTHORED BUT UNPARSED. Named water/nutrient/temperature/light/space stress, `disease_state`, `pest_pressure`, biomass/root/canopy indices, `yield_potential`. |
| Per-species physiology | `data/entities/plants/tomato.ron` | AUTHORED BUT UNPARSED. Per-stage duration and NPK, DLI/PPFD/photoperiod, Kc evapotranspiration, CO2, `rotation_group`, `common_pests`, `common_diseases`, pruning. |
| Plant genetics | `data/genetics.ron` | AUTHORED BUT UNWIRED. 12 heritable plant traits, 5 genetic diseases each carrying a `visual_sign` string. |
| Disease contagion engine | `src/systems/ecology.rs` | WRITTEN AND WORKING, unregistered, never touches crops (queries `Health` + `Transform`; crops have neither). |
| Impostor baker | `src/renderer/billboard_bake.rs` | WORKING, generic over `BakePart`. Single side-on view, **not** an octahedral impostor baker. |
| LOD registry | `data/lod/categories.ron` + `src/lod_registry.rs` | WORKING, grass/shrub/tree rows and Settings sliders already rendered. |

**Most of this system is already on disk and simply not connected.** The largest piece of work
is not writing a simulation, it is parsing files we already ship and pointing an existing
disease engine at crops.

## A live bug, found while writing this

Plants are created with material type 12 and `base_color = [1,1,1,1]`
(`src/engine/home_meshes.rs:891`). The type-12 fragment branch
**unconditionally** treats `material.base_color.xyz` as a planet centre
(`assets/shaders/pbr/90-fragment-main.wgsl:779`):

```
let rad_w = in.world_position - material.base_color.xyz;
let mu_geo = dot(rad_w / rl, normalize(camera.sun_direction.xyz));
sun_gate = smoothstep(TERRAIN_TERMINATOR_LO, TERRAIN_TERMINATOR_HI, mu_geo);
```

`sun_gate` then multiplies the entire direct-sun term (`:1176`). For a plant, `base_color.xyz`
is the **point (1,1,1) in world space**, not a planet centre, and the smoothstep window is only
0.016 wide, so the gate is effectively binary. Evaluating the shader math over a 40 m garden
footprint: roughly **half the plants have direct sunlight switched off**, split by a hard
diagonal through (1,1,1) perpendicular to the sun, and that line **sweeps as the sun moves**.

This ships today, in every procedurally drawn plant. It is independent of everything else in
this document and should be fixed on its own. Cheapest correct fix is to give plants their own
material type with the planet gate omitted; the quick fix is to guard the gate on a `params.w`
bit. Either way the acceptance gate is the renderer bar in "Verification" below, plus a
homestead capture via `debug/screenshot_request.json`.

## The core idea

**A plant is not a model. It is a program plus a parameter block.** Everything below follows
from refusing to store a plant as geometry.

## Layer 0: four primitives, not a model library

Xfrog shipped separate Tree, Horn and Leaf components for years and eventually collapsed them
into one, because they were architecturally the same thing. A branch, petiole, fruit stalk,
awn, tendril and root are all one swept generalized cylinder with different numbers. That gives
a four-primitive kernel:

| Primitive | What it is | What it covers |
|---|---|---|
| **Sweep** | a cross-section carried along a spine, both varying per station | every stem, branch, trunk, petiole, awn, tendril, root, plus banana, cucumber, pepper, carrot, pea pod |
| **Lathe** | a profile curve revolved, with rib and bend modulation | apple, tomato, orange, plum, melon, squash, eggplant, grape berry |
| **Phyllotaxis** | elements distributed over a surface of revolution | pineapple fruitlets, corn kernels, strawberry achenes, sunflower, pine cone, artichoke |
| **RadialArray** | n copies about an axis | petals, sepals, whorls, palm fronds |

Four generators plus per-species numbers. A much better match for infinite-of-X than one
monolithic parameter struct, and the "hard" fruits stop being special cases.

### How many numbers is a plant, actually

Counted from primary sources, not secondary summaries:

- **Weber-Penn** (SIGGRAPH '95) is **exactly 80 named parameters** at 4 levels; 65 at
  `Levels=3`. Arbaro's `quaking_aspen.xml` has exactly 80 entries, confirming the count.
- **proctree**, the deliberately minimal generator, gets a plausible tree from **19 shape
  floats plus a seed**, about 80 bytes.
- **ez-tree** presets are 60 scalars, roughly 45 geometric.

One tree species is **60 to 90 floats, 240 to 360 bytes**. Ten thousand species fits in 3 MB.
**Parameterization is not the bottleneck.** Generation throughput, LOD, and fragment cost are.

The rest of the kingdom is cheaper than the tree:

- **Leaf outline**: the simplified Gielis equation SGE-1 is **two** parameters, R-squared above
  0.985, validated on 3,310 leaves across 53 species. SGE-2 is three.
- **Fruit profile**: the explicit Preston equation is **five** floats, validated on 751
  muskmelons, and it has a **closed-form volume integral**. Yield mass, nutrition and trade
  value scale with real volume without ever building a mesh, which matters for the 99.9% of
  fruit that is simulated but never rendered.
- **Rib and lobe modulation**: three floats. Use `|cos(k*theta/2)|^q` with `q < 1`, not a sine,
  because pumpkin and squash sinuses are creases. Use **odd** lobe counts (3, 5, 7); Weber-Penn
  explicitly warns even counts read as artificially symmetric.

### Where closed forms genuinely die

Gielis cannot express cordate, palmate, pinnate or any compound leaf, and no number of extra
parameters fixes it: a single-valued `r(phi)` cannot fold back past its own attachment, cannot
recurse, and cannot express the sinus-to-lobe feedback that is the real mechanism. Cordate is
the sneaky one because it looks easy. Budget a separate path for compound leaves from the start.

Two workable approaches, and we prototyped and validated the second:

1. **Runions, Tsiantis and Prusinkiewicz (2017)** is the rigorous answer. Their Fig. 9 is a
   two-axis morphospace: **timing of lateral vein outgrowth** by **rate of webbing** walks the
   whole range from entire to serrate to lobed to recursively lobed, and simultaneously swings
   venation from pinnate to palmate. Roughly 5 to 8 parameters for the entire eudicot range.
   It is an iterative simulation at tens to hundreds of ms per leaf, so it is an **offline bake
   only**: run once per species at load, cache an outline polygon plus vein polylines, and the
   runtime representation is identical to the closed-form case.
2. **A width-profile family.** A midrib with `w(s) = s^p (1-s)^q` moves the widest point
   anywhere along the blade (ovate, elliptic, obovate, lanceolate, linear, needle); a periodic
   width modulation gives margins; a deeper one gives pinnate lobing; and a basal term that
   pulls low stations back behind the petiole gives **cordate**, which Gielis cannot. A separate
   radial family with N lobe tips and a basal sinus gives palmate. Compound leaves are an
   arrangement rule applied to either.

A validation sheet generating 17 species this way, including every species named in the original
question, came to **520 bytes of parameters for all of them combined**, with venation drawn
procedurally rather than authored.

### The assemblies

Some organs are not solids at all, and a fruit system that only lathes profiles hits a wall at
exactly the crops a farming game needs:

- **Grape cluster, oat panicle, sorghum**: a branched rachis with a berry at every terminal.
  This is **the tree generator called again at 10 cm scale**, plus a few iterations of sphere
  separation.
- **Wheat and barley spikes**: the one-dimensional case of the same thing. Arbaro already ships
  barley, wheat, rush and horsetail as Weber-Penn parameter files, which is direct evidence the
  tree parameterization stretches to cereals unmodified.
- **Corn ear**: phyllotaxis, but **maize is not golden-angle**. Kernels sit in paired rows with
  an even rank count, typically 12 to 20. Reusing sunflower code gives a spiral cob that reads
  as obviously wrong.
- **Pineapple and strawberry**: phyllotaxis on a surface of revolution.

Bark and skin are procedural triplanar noise, or CC0 tiles from Poly Haven and ambientCG, which
carry the identical license to this repo.

### License constraint (load-bearing)

`LICENSE` is **CC0 1.0 Universal**. Every mature open-source tree generator is GPL: Arbaro
GPL-2, Blender Sapling GPL-2, improved-sapling GPL-2, friggog/tree-gen GPL-3. **None can be
ported here.** Even permissive ones (ez-tree MIT, proctree BSD-3) require retaining a copyright
notice, a genuine wart in a public-domain dedication.

The clean route is the one Arbaro, Sapling and tree-gen each independently took: **implement
from the Weber-Penn paper**, whose Appendix and equations are complete enough. Read permissive
sources to check understanding; write the code from the paper.

Clean data sources: **USDA PLANTS** (~50,000 US species) is a US federal work, public domain,
the cleanest possible match for CC0. **GBIF** backbone taxonomy is CC0. The **BBCH monograph**
2018 JKI edition is CC BY 4.0. **TRY** is request-based and must not be redistributed; use it
to calibrate and publish only derived numbers.

## Layer 1: one clock, no stage models

### This is already the shipped architecture

`build_plant` already takes a continuous `t` and a deterministic seed. There are no stage
meshes. What actually blocks continuous growth is **one expression**:
`src/engine/home_meshes.rs:797` computes `t` as the **stage-index
fraction** `(i+1)/n` instead of the continuous progress the farming system already has at
`src/systems/farming/mod.rs:1065`, and the rebuild signature at
`:659-677` hashes `growth_stage` as a **string**, so nothing re-triggers between stages.

Threading continuous progress through and adding a quantized-`t` term to the rebuild signature
is roughly a 15-line change that makes plants visibly grow smoothly with **zero renderer risk**.
That is the first increment, and it validates the whole thesis before anyone touches a vertex.

### Organ birth times

Every organ carries a **birth time**. A node is born at `tau = i * plastochron`; its leaf then
grows on its own age clock. Young organs are automatically smaller; senescence automatically
takes the oldest first; a seedling and a mature plant are the same function at different `tau`.

The named stages in `plants.csv` (96 distinct vocabularies, 130 tokens across 134 species)
become **labels on ranges of the clock**. Adopt **BBCH** as the vocabulary where it applies: a
two-digit code, principal stage 0 to 9 times secondary 0 to 9, descended from the Zadoks cereal
code, with about 50 published crop keys. This makes the game's stage labels the same labels a
real grower and a real extension bulletin use.

### But a pure function of tau is not enough

`f(tau, seed) -> geometry` is history-free, so nothing that happened to the plant persists.
That rules out pruning (the whole point of `tomato.ron`'s pruning block, and of grapevine cane
and spur pruning where **fruit only forms on shoots from one-year-old wood**), herbivory, wind
breakage, training and espalier, apical-dominance release after decapitation, and etiolation.
The "32 to 64 bytes per instance" figure only holds for an untouched annual.

Tier the representation explicitly:

| Tier | Who | Per-instance state | Representation |
|---|---|---|---|
| 0 | grass, field crops, annuals, anything past ~20 m | 8 to 16 B | pure `f(tau, seed)`; damage derived deterministically from `(seed, organ_index, accumulated_stress)` so it is stable frame to frame without storage |
| 1 | shrubs, mid-field trees | 24 to 48 B | geometry from age via an allometric curve rather than stored |
| 2 | player-tended orchard, vineyard and bed plants (tens to low thousands) | persistent | a real incremental generator with a bud and segment list carrying per-segment birth year, bearing-wood age, and damage flags |

Promote and demote between tiers on distance and player interaction.

### Meristem fate is discrete, and that is not a rendering detail

Several key transitions are genuine changes in meristem fate and topology, not labels:

- **Floral initiation in cereals** converts the shoot apex to a spike, after which **no further
  leaves are ever produced**. Final leaf number is fixed (wheat 8 to 12, maize 16 to 22). A
  generator that keeps emitting phytomers forever gives a wheat plant with too many leaves.
- **Bolting** in lettuce, carrot, beet, cabbage, celery and onion is topological: the compressed
  rosette apex becomes an inflorescence apex, internodes jump from near zero to long, and leaf
  production stops. It is also a **crop failure**, which is excellent gameplay.
- **Cacao** (shipped) forms a jorquette: the orthotropic chupon terminates in a whorl of 3 to 5
  fan branches and a new chupon takes over.
- **Cacao and fig** show cauliflory: flowers and fruit form on trunk and main branches from
  cushions, which no phyllotaxis or branch-angle rule can produce.

Give the generator a per-apex state machine `{Vegetative, Transitioning, Inflorescence,
Terminated, Dormant}`, and an `organ_site` enum on the fruit and flower rule
`{axillary, terminal, cauline_cushion, geocarpic, scape}` so cacao, fig, peanut and garlic are
data rows, not special cases.

### Phyllotaxis is not the golden angle

The existing generator hardcodes `2.39996` (137.5 degrees). That is the wrong default for a
large share of the shipped list:

- **Distichous** (180 degrees, two-ranked): all Poaceae. Wheat, rice, corn, barley, oat, rye,
  millet, sorghum, sugarcane, bamboo, lemongrass, plus garlic and leek. **Thirteen shipped
  species.** A golden angle makes maize and wheat leaves spiral around the culm, which is
  instantly, visibly wrong.
- **Decussate** (opposite pairs at 90 degrees): basil, mint, oregano, sage, thyme, maple.
- **Spiral** but at the Fibonacci rational 2/5 (144 degrees, apple and rose) or 3/8, not exactly
  137.5.

Parameterize as `{divergence_deg f32, jugy u8, whorl_count u8, ranked bool}`, defaulting to
137.5/1/1 only for the spiral archetype. Add the **phyllochron** (thermal time per leaf, DSSAT's
PHINT, one f32): leaf count = accumulated thermal time / PHINT is the cheapest possible driver
of visible growth and is already the industry-standard parameter.

### Thickening is not universal either

"Woody perennials thicken with age" is flatly wrong for five shipped species. Bamboo, palm,
coconut, banana and sugarcane are monocots with **no vascular cambium**. A bamboo culm reaches
full height *and* full diameter in a single season and never thickens again. Palms use a primary
thickening meristem, so an old palm is not a fatter palm. Banana has no trunk at all; the
pseudostem is rolled leaf sheaths (the data's own `pseudostem` token), it is monocarpic, and
after fruiting the mother dies and a sucker takes over (`sucker`, `pup`).

Make thickening a per-archetype enum: `{none, cambial_stem, primary_thickening_monocot,
cambial_root, anomalous_concentric}`. Note that in the shipped list thickening is often a
**root** phenomenon: carrot, parsnip and turnip are thickened taproots, and beet has anomalous
concentric cambia, which is what makes the rings in a beet. Add a `monocarpic` flag with a
`clonal_replacement` rule so banana, pineapple, bamboo and agave-types die after fruiting and
are replaced by an offset rather than resetting to stage 0.

### The integer-parameter trap

Scales, lengths, radii and angles interpolate cleanly. **Integer parameters pop**: segments per
stem, children per parent, level count, lobe count, split counts. Weber-Penn already solved the
split case and the trick generalizes: a per-stem **error accumulator**, so `nSegSplits = 1.2`
means "one clone on 80% of segments". Fade new children in from zero scale; hold segment and
level counts at their mature values while scaling the unused tail to zero length.

### Driving the clock, and why not yet

Growth today is `elapsed_wall_clock / growth_days` times health, recomputed from raw age every
tick and never integrated. Thermal time is the right replacement, **but adopting it is a
game-balance change to 134 crops disguised as a rendering improvement**, and the clock
underneath cannot currently carry the realism claim:

- A game day is **1200 s** and the year is **120 days**. Maize to black layer is ~2700 F-days
  = ~1500 C-days base 10; at a 28 C mean that is ~83 real days, **2.7 game seasons**, so maize
  could not finish inside a game summer. Winter wheat would consume most of a game year.
- **Photoperiod is a literal no-op.** `src/systems/time.rs:86-100` hardcodes a 06:00 to 18:00
  arc with no latitude and no seasonal variation, so daylength is exactly 12.0 h everywhere,
  always. Any critical-daylength comparison is a compile-time constant. That silently destroys
  hop, hemp, onion (bulbing is a photoperiod class matched to latitude), garlic, potato, soybean
  (maturity groups are a critical-daylength ladder), lettuce and spinach bolting, and
  June-bearing versus day-neutral strawberry.
- Weather is **one global value** with a +/-5 C random draw, so there is no per-plot temperature.
- F-days and C-days differ by 1.8x and published tables mix them **without labelling**. `Tbase`
  is a **fitted** value, not a physical constant (maize 8 C in SIMPLE, 10 C in US extension
  practice), so pairing a `Tbase` from one source with thresholds from another is silently wrong.

Therefore, if thermal time is adopted:

1. Ship latitude-aware daylength first, with an explicit **civil-twilight** convention (sun at
   -6 degrees, the agronomic standard; geometric sunrise-to-sunset differs by 30 to 50 minutes
   at mid-latitudes and will shift flowering by days against any published threshold). Compute
   once per latitude band per day, zero per-plant cost. Expose the indoor lamp schedule as a
   player-controlled photoperiod.
2. Store every threshold in **real C-days with a mandatory citation field**, store `Tbase` and
   `Tcap` as a **matched set from the same publication**, tag units (`c_days` | `f_days`) and
   convert at load. Add a data-validation test that fails when a species row's `Tbase` and stage
   thresholds carry different citations.
3. Derive each species' initial `Tsum` from its existing `growth_days` at a reference
   temperature so **nothing re-times on day one**, then re-balance deliberately with a stated
   acceptance criterion and a named owner.
4. Apply one global `thermal_time_scale` derived from the 1200 s day and 120 day year.

Rendering and phenology should not land in the same arc. One is eyeball-verifiable; the other
needs playtesting.

## Layer 2: health as pigments plus masks

Today `health` is one 0-to-100 scalar falling only from dehydration and a WiFi RF penalty, and
`wilt` is literally `1.0 - health/100` (`src/engine/home_meshes.rs:806`).
Every ailment looks identical: browner.

### Color

Start with a **4-way blend** between theme-token colors (healthy green, chlorotic yellow,
anthocyanin red, necrotic brown) driven by named stress channels. The repo already proves this
reads correctly: `plant_mesh.rs:329` lerps toward dry brown on wilt.

**PROSPECT is a research task, not a bake step**, and should not sit on the critical path:

- It is a plate model producing directional-hemispherical reflectance and transmittance across
  2101 bands. Converting to RGB needs CIE convolution and an illuminant choice, and the output
  is not a PBR base_color.
- It has **no specular term**, yet close-range leaf appearance is dominated by the cuticular
  lobe. It gives **one** reflectance, while adaxial and abaxial surfaces differ strongly, and
  the abaxial is exactly what matters for wilting, flutter, and rust and downy-mildew diagnosis.
- It is a **leaf** model and is out of domain for fruit. Tomato red is lycopene, pepper red is
  capsanthin, neither in its carotenoid basis.
- `Cbrown` is a dimensionless fudge coefficient, so "necrosis is physically derived" is
  over-claimed.

If it is adopted later: use it for **diffuse albedo only**, add explicit specular and roughness
plus an adaxial/abaxial pair, bake **transmittance** from the same run for a wrap term (backlit
leaf glow is the strongest single cue that vegetation is real), give fruit its own pigment ramp,
reduce to a 3D table, commit the bake script under `scripts/`, and **fit it analytically in
WGSL rather than adding a bind-group binding** (see the binding budget below).

**Pigments are not symmetric.** Carotenoid is **unmasked**: concentration stays roughly flat
while chlorophyll falls, which is why yellow is reliable every year. Anthocyanin is
**synthesized de novo** and requires sugar, light and cool nights, with a per-species capacity
that is effectively zero in many species. Modelling them symmetrically makes every plant redden
identically. Correct form: `carotenoid` a per-species constant revealed by falling chlorophyll,
`anthocyanin` a separate additive term with per-species capacity times a cold-and-light term.

### Spatial masks

One packed RGBA field per leaf archetype: **R** = distance to nearest vein, **G** = distance to
margin, **B** = areole cell id, **A** = base-to-tip midrib coordinate. Plus two per-leaf
scalars: leaf age, and adaxial versus abaxial.

**The mask must take venation type.** In parallel-veined monocots, interveinal chlorosis reads
as **stripes**, not a reticulate net. Maize nitrogen deficiency is a **V-shaped chlorosis from
the tip down the midrib of the lowest leaves**; maize potassium is tip-and-margin necrosis of
the lowest leaves.

Move everything constant over a leaf into the **vertex or instance** attribute (pigments,
stressor vector, damage severity all interpolate to a constant and cost nothing in the fragment
shader). Bake the pattern library into a **channel-packed atlas** and do one texture fetch plus
a dot product against a per-leaf weight vector, rather than N procedural noise evaluations.
Reserve genuine per-fragment noise for the 0-to-20 m tier, gated on distance. This matters
because alpha-tested foliage is overdraw-dominated, there is **no depth prepass**, and the
opaque fragment shader already contains `discard`, which inhibits early-Z for every draw using
that PSO.

### Deficiencies, corrected

Phloem mobility is the right organizing rule, and Layer 1 gives it to us free because organ
birth times are already known. But the earlier draft got several specifics wrong:

- **Calcium symptoms are not leaf patterns.** They are fruit and meristem: blossom end rot
  (tomato, pepper), tipburn (lettuce, cabbage), bitter pit (apple), black heart (celery). All
  five hosts are shipped. Worse, BER is usually **not** a soil-calcium problem: Ca moves in the
  xylem with transpiration and fruit transpire very little, so it is a water-management and EC
  problem. Teaching "add calcium" teaches the wrong fix. Put Ca on an **organ-disorder channel**
  driven by transpiration and EC, not on the leaf mask.
- **Boron is phloem-mobile in polyol translocators**, which is the entire Rosaceae: apple, pear,
  cherry, plum, peach, plus celery. The old-versus-new rule **inverts** exactly where the game
  has the most fruit trees. Add a per-species `boron_polyol_mobile` flag. B symptoms are also
  mostly structural (corky core, cracked stem, hollow heart), not chlorotic.
- **Sulfur was omitted**, and S is the single most important differential against N: S is
  general chlorosis of **young** leaves, N of **old**.
- **Zinc and manganese are missing.** Zn gives interveinal plus shortened internodes (rosetting,
  little-leaf in fruit trees). Mn gives interveinal with softer vein contrast than Fe.
  Fe/Mn/Zn/Mg discrimination is the actually hard part of any real key.
- **Drive availability through pH, not concentration alone.** The classic field case is
  lime-induced iron chlorosis above pH 7.5 with abundant soil iron present. The repo already
  carries unused `ph_min`/`ph_max` on all 134 rows.

### Pests and disease, corrected

The earlier draft had three sign errors that would actively teach the wrong thing:

- **Aphids do not stipple.** They cause leaf **curl and distortion plus copious honeydew**,
  which then grows **sooty mold** as a secondary organism. Stippling is **thrips** (silvering
  plus black frass specks) and **spider mites** (fine stippling plus webbing, exploding in hot
  dry conditions).
- **"Powdery mildew surface noise" renders as uniform frost.** Powdery mildew is an ectoparasite
  that establishes **discrete circular colonies that coalesce**, usually adaxial first. **Downy
  mildew** is an oomycete and the opposite in every respect: angular, vein-delimited yellow
  blotches on the upper surface with grey sporulation on the **underside**. One "mildew" mask
  teaches a false equivalence.
- **"Blight with chlorotic halos" conflates four different diseases.** Late blight
  (*Phytophthora*, water-soaked greasy lesions with white sporulation at the advancing margin on
  the underside, kills in days), early blight (*Alternaria*, concentric target spot, lower leaves
  first), Septoria (small lesions containing visible black pycnidia), and **halo blight**
  (*Pseudomonas syringae*), which is the only one that actually has a chlorotic halo, because
  halos are a bacterial toxin signature.

Make the ailment library a **data registry of lesion models**, per infinite-of-X:
`{surface: adaxial|abaxial|both, geometry: circular|angular_vein_delimited|target_ring|serpentine|blotch|marginal,
foci_density, radius_vs_degree_days, halo, sporulation_color, substructure: pycnidia|uredinia|none,
latent_period_dd, wetness_hours_required, temp_window}`. Same generator, correct botany, each
new pathogen a RON row.

**Plants do not recover from systemic infections.** TMV and CMV are incurable and permanent;
the correct player action is to **rogue and destroy** and disinfect tools. Fusarium and
Verticillium wilts persist in soil for years; fire blight persists in cankered wood. The
existing EcologySystem semantics (severity decays, natural recovery, cure threshold) teach the
opposite of correct practice. Add per-pathogen flags:
`{resolution: curable|systemic_incurable|lethal, tissue: foliar|vascular|root|fruit|seedling,
inoculum_source: airborne|splash|soil_persistent|vector|seedborne, soil_persistence_years}`.

There are currently **no soilborne, root or vascular classes**, so nothing would ever justify
crop rotation, resistant rootstocks or solarization, even though `rotation_group: Solanaceae`
already sits in `tomato.ron`. Give soil-persistent pathogens a per-plot inoculum counter that
decays only with time out of the host family, which makes `rotation_group` load-bearing. Damping
off is also missing and is where most real home-grow losses occur.

Disease progress is an **epidemic**, not a per-plant scalar: van der Plank logistic progress
with a latent period (three numbers), and infection events gated on a leaf-wetness-duration by
temperature window from a published table (Mills for apple scab, Blitecast for late blight).
That turns spraying into a real decision instead of a chore.

### The educational claim, corrected

The earlier draft said the player "learns a real dichotomous key". **That is false as designed,
and it is the strongest over-claim in the original document.**

Real diagnosis is hard precisely because the symptom-to-cause map is **many-to-one**. A pale,
stunted plant is produced by nitrogen deficiency, root rot, nematodes, waterlogging, cold soil,
herbicide carryover, root-bound containers, and low pH. No leaf mask distinguishes them. A game
that generates a visually unique mask per cause teaches a **bijection that does not exist**,
which is anti-education for a project whose pitch is real survival skills.

Worse, the single most diagnostic feature a real extension diagnostician uses **is not the leaf
at all**. It is the **spatial distribution across the planting**: scattered at random suggests
abiotic, nutritional or genetic; discrete patches with a spreading front suggests biotic;
row-aligned or field-edge suggests equipment, spray drift or wind. That channel is nearly free,
because the arrangement of affected plants in a bed is already known.

So: deliberately introduce **symptom aliasing**, and build the evidence channels that resolve it:

1. **Spatial distribution** across the bed.
2. **Destructive inspection**: pull the plant to see root rot or nematode galls, cut the stem to
   see vascular browning for Fusarium and Verticillium, **turn the leaf over** to see
   sporulation or mites.
3. **Soil and tissue tests** as items.
4. **History**: what was planted here last season.

And restate the claim as **"the player learns the real diagnostic process"**, not a key, because
the process is exactly what the aliasing forces. Also keep the honestly undecidable cases,
including "hidden hunger" where yield drops with zero visual symptom.

## Layer 3: species as inheritance, not enumeration

Author roughly 100 family and growth-form archetypes and express each species as a **delta**.
`Rosaceae/tree` covers apple, pear, cherry and plum with about 10 numbers of difference.

**Do not claim this scales to 400,000 species.** An earlier draft computed "all known vascular
plants at 1 KB each = 400 MB". That is arithmetic over numbers that do not exist: there is no
free, machine-readable table of per-species phenology parameters. SIMPLE publishes 14 crops;
Kew gives 342,953 **names**; USDA PLANTS gives coarse duration and bloom period, not GDD
thresholds or critical daylengths. The real deliverable is **hand-assembled parameters for the
134 shipped crops**, one row each with a mandatory citation field.

Also note the archetype story collides with the shipped data: 68 of the 96 stage vocabularies
are used by exactly one species, and several are sci-fi (`warp_bloom`, `cryo_bloom`,
`phase_shift`, `ignition`, `charge`). Those do not inherit from a Rosaceae archetype and need a
documented degrade rule: an archetype with authored durations and no thermal-time model.

## Layer 4: how it renders

### The transport constraints, measured

- `Vertex` is exactly **32 bytes** (`pos f32x3 @0`, `normal f32x3 @12`, `uv f32x2 @24`). No
  color, no second UV, no tangent. Per-face RGB is smuggled through `uv.x` as an exact integer;
  bits 16, 17 and 18 are taken and only **bits 19 to 23** remain before f32's 2^24 ceiling.
- Faces are **fully unshared**, so every plant triangle costs **108 bytes**. The shipped
  strawberry at maturity is ~674 triangles, ~71 KiB, for one plant.
- There is **no GPU mesh instancing**. Every classic draw is `draw_indexed(0..n, 0, 0..1)`, and
  the single 16-byte instance attribute is spoken for by terrain patch translation and LOD fade.
- `RenderObject` has **no pipeline field** and `set_pipeline` is called **once** before iterating
  objects. There is no sorting by pipeline.
- Group 3 is the only texture group and carries bindings 0 to 15; 4 bind groups is wgpu's
  baseline `max_bind_groups`. Adding a binding means updating **all three** `create_bind_group`
  sites, which is exactly the defect that shipped ten releases panicking on world entry.
- **No block compression.** Zero `TextureFormat::Bc*` anywhere; every texture is Rgba8UnormSrgb
  or Rgba8Unorm. Quote atlas budgets in RGBA8: 30 masks at 512 squared is **30 MB**, not 10.
- **No mipmaps on the albedo path**: `mip_level_count: 1`, `mipmap_filter: Nearest`, and every
  sample site is `textureSampleLevel(..., 0.0)`, an explicit LOD 0.
- `alpha_to_coverage_enabled` is **false** on every PSO and there is no masked pipeline; cutout
  is a bare `discard` in the opaque shader.
- The sun shadow pipeline has **`fragment: None`**. It cannot alpha-test.
- Plant geometry is baked in **absolute homestead world coordinates** with an identity model
  matrix, and plants go into `all_objects`, not `celestial_objects`, so they never receive the
  floating-origin offset or planet rotation. **The system works only inside the ship and
  homestead scene.**
- `rebuild_plant_meshes` does `fs::read_to_string` of the RON on **every rebuild**, with
  `unwrap_or_default()`, so a single typo silently empties the registry and reverts the whole
  world to procedural blobs with no error surfaced.
- `Mesh::from_vertices` uses `create_buffer_init` with `BufferUsages::VERTEX` only, **no
  COPY_DST**, so a mesh buffer cannot be written in place; every rebuild reallocates.

### What follows from that

**Never touch the shared `Vertex`.** Widening it is repo-wide (mesh builders, planet_surface,
planet_chunks including `PATCH_MESH_BYTES`, far_trees, ship rooms, six PSO vertex states, the
billboard baker's own pipeline). Instead give plants **their own vertex format, their own
instance format, their own PSO pair, and their own draw list**, the way terrain patches already
work: after the classic loop, `set_pipeline(plant_pipeline)`, bind the vegetation arena once,
then instanced or indirect draws. `patch_arena.rs` is the in-repo template (shared mega buffers,
`RangeAlloc`, `multi_draw_indexed_indirect` over up to 16384 draws) and `batched_variant_of`
hands a new material branch to the batch path for free.

A format with headroom, roughly 36 bytes, less than the 96 bytes per triangle the unshared
format burns today and it buys smooth shading back:

```
pos            f32x3      12
normal         Snorm16x2   4   (octahedral)
uv             Unorm16x2   4
birth_tau/ref  Unorm16x2   4   <- Layer 1 needs this or continuous growth cannot reach the GPU
pigment        Uint32      4   (chlorophyll / carotenoid / anthocyanin / necrotic, u8 each)
damage         Uint32      4   (severity nibbles + pattern id + organ id)
wind wt/phase  Unorm8x4    4
```

Per-plant state (tau, health, yaw, scale, LOD fade) goes on the **instance** attribute, not the
vertex. Without a per-plant instance attribute, per-plant LOD is impossible anyway, because
today every plant in a tower config is merged into **one** mesh drawn as **one** object.

**Split the clock from the topology.** CPU-rebuild only at discrete structural checkpoints
(organ *set* changes: leaf N appears, flower opens, fruit sets) and carry the continuous part in
the vertex shader from `birth_tau` plus a per-instance `tau`. Otherwise continuous growth means
reallocating every plant buffer every frame. Also add `COPY_DST` to `Mesh` and a size-class
reuse path so structural rebuilds `write_buffer` instead of reallocating, and cache the parsed
registry behind an mtime check so live editing still works but the steady state does zero I/O.

### LOD ladder

- **0 to 20 m**: hero GLTF where a model exists (keep material type 19), otherwise full
  procedural mesh with pigment and damage masks.
- **20 to 150 m**: reduced organ count, leaf clusters merged into cards.
- **150 m and beyond**: baked billboard cards from `billboard_bake.rs`.

Two corrections to the earlier draft:

1. **Do not say "octahedral impostors" and do not cite `far_trees.rs` as a reuse target.**
   `billboard_bake.rs` renders exactly **one** side-on orthographic view, unlit albedo with
   binary coverage alpha, into a hardcoded 3x2x512 atlas whose tile decode is mirrored as
   literals in WGSL. Octahedral impostors need an 8x8 view grid, a normal and depth G-buffer,
   and 3-nearest-view blending; that is its own increment. And `far_trees.rs` was pulled to
   **default OFF** after the operator said "black squares in a grid" twice, with an explicit
   instruction not to iterate the card sheet.
2. **Cards need three things before they can ship**: mips (copy the working CPU box-filter chain
   in `ground_textures.rs`), a switch off explicit `textureSampleLevel(..., 0.0)`, and
   `alpha_to_coverage_enabled: true` on the plant PSO. Mip-less, coverage-less alpha-tested leaf
   cards are the single most notorious shimmer failure in foliage rendering and would look worse
   in motion than the merged geometry they replace.

Also required and not optional: a **masked shadow variant** with a real fragment stage that
samples atlas alpha and discards. Without it, cutout foliage casts solid rectangular shadows,
and excluding plants from the caster loop instead leaves the ground under a canopy fully lit.

Declare distances as rows in `data/lod/categories.ron`, which already renders Settings sliders
per category, and activate the `grass` row in the same increment (its own comment warns that a
slider that does nothing is a lying UI).

### Render budget, in the units that bite

The earlier draft compared **sim** bytes to **render** bytes, which is a category error. State
the render budget separately:

- A believable apple tree is 20k to 200k triangles. At 108 bytes per triangle that is 2 to 22 MB
  of GPU buffer for **one tree**. The current "tree" archetype it would replace is a 5-branch
  toy with about 25 leaves total.
- Set a hard cap, for example **40k triangles and 4 MB for the largest near-field plant**, and
  design the generator to hit it.
- Drop the unshared-face requirement in the new format: with real UVs and per-organ attributes,
  vertices can be shared along a stem, roughly halving the per-triangle cost.
- Budget triangles per frame, draw calls per frame, GPU vertex bytes resident, and CPU
  microseconds to generate one plant at each tier. Generating a 2k to 8k triangle plant in
  release Rust costs roughly 50 to 500 microseconds on one core: fine for variant baking, far
  too slow per instance per frame. Generate **8 to 32 variants per species** and vary instances
  by transform, tint and wind phase.

Procedural generation saves **disk and authoring**, not VRAM. `.kkrieger` was 96 KB on disk and
expanded to 200 to 300 MB in RAM.

## The GLTF tier: a stated boundary, not a fork

`home_meshes.rs:843-852` routes any bed or field crop with `assets/models/plants/<id>_<1..4>/`
to a real GLTF at the growth quartile. That covers apple, tomato, corn, wheat, lettuce, pumpkin,
watermelon, rice, carrot, beet, orange and more: precisely the crops a player stands in front of.
Procedural leaf cards will not beat a purpose-modelled crop at 2 m.

The rule, stated as architecture rather than left implicit:

- **Hero GLTF** for near-field bed and field crops that have a model.
- **Procedural** for tower net cups, for the ~115 species with no model, and for the mid and far
  LOD of *all* species, including baking impostors from the hero models via the same baker.
- The procedural mid-tier must blend into the hero model at the handoff distance using the
  existing `RenderObject.fade` Bayer crossfade so there is no pop.

That is a documented tier boundary with a clear rule, not two renderers competing for pixels.

## Verification (non-negotiable)

`cargo test` and naga validation **cannot** see pipeline-layout, device-limit or bind-group
mismatches. v0.782-784 shipped three unbootable releases; v0.1029-v0.1038 shipped ten releases
that panicked on world entry while menu-only boot checks stayed green. Every increment here
touches PSOs or bind groups. The bar for each:

1. `cargo build --features native --release`
2. `cargo check --features relay --no-default-features` (a native-only module left ungated kept
   the VPS deploy red for 25 consecutive releases)
3. Launch `target/release/HumanityOS.exe` with **`HUMANITY_NO_FOCUS=1`** in its env
4. **Enter the world**: `node scripts/probe-sweep.js --only blue-marble-12000km --exe target/release/HumanityOS.exe`, expect panics=0
5. Grep `%APPDATA%/HumanityOS/logs/run.log` for `PANIC`
6. A homestead-with-planted-tower capture via `debug/screenshot_request.json`
7. Before adding any group-3 binding: grep `texture_bind_group_layout` for every
   `create_bind_group` site and count entries at each

Per the GUI-first rule, any new grower-facing knob (LOD bands, IPM level, spray schedule,
rotation group, scouting) ships its in-app control in the **same** increment or the debt is
logged in `docs/design/in-app-ops.md`.

## Build order, sequenced by risk

The earlier draft was sequenced backwards: it put the generator before the transport that has to
carry it, and put LOD, impostors and wind last, so the two things most likely to sink the design
would be validated after all the authoring work was sunk. Corrected:

**Phase A, visible value with zero renderer risk.**

1. **Continuous growth.** Thread `effective_progress` into `rebuild_plant_meshes`, replace the
   stage-index `t`, add a quantized-`t` term to the rebuild signature, cache the parsed registry
   behind an mtime check, and log RON parse failures loudly. Plants visibly grow smoothly. ~15
   lines plus the cache.
2. **Fix the type-12 terminator bug** (see above). Independent of everything else.
3. **Data coverage.** Extend `data/plants_visual.ron` past its 19 recipes. Pure data, satisfies
   infinite-of-X, no code change. Add `plants_visual.ron` to `src/embedded_data.rs` so shipped
   builds stop silently degrading to generic rosettes, add `schemas/plant_visual.toml`, and add
   a `just validate-data` check.
4. **Wire the authored sim schema.** Parse `data/entities/plants/*.ron` into a real stress-channel
   `CropInstance` (its own increment, with the save-format change stated), give crops `Transform`
   and `Health`, register `EcologySystem`. Blight starts spreading bed to bed with almost no new
   simulation code.

**Phase B, the transport, proven with the crude geometry we already have.**

5. Plant PSO pair (opaque plus **masked shadow**), plant vertex and instance formats, material
   type 20 without the planet gate, and a vegetation arena on the `patch_arena.rs` pattern.
6. LOD ladder, cards, mips, alpha-to-coverage, proven at 150 m.

**Phase C, the fidelity.**

7. The recursive growth-unit generator, replacing the 6 hardcoded archetypes. Write it as a
   **direct recursive walk**, not an L-system interpreter: string rewriting allocates
   exponentially, and two grammars cannot be interpolated, so you can only step generations,
   which is exactly the stage-swapping this design exists to avoid. Every generator that ships
   in a game walks the recursion directly. Preserve the every-crop-renders guarantee and state
   the migration path for the 19 existing recipes.
8. Pigments and the damage-mask library in WGSL, where shader hot-reload makes iteration
   seconds instead of a 3-minute rebuild.

**Phase D, separate arc.** Thermal time, photoperiod and the 134-crop rebalance, gated on
latitude-aware daylength and a per-plot temperature field, with its own acceptance criterion.

## Deliberately out of scope

- Do not iterate the far-tree card sheet. It is default OFF after a twice-repeated operator
  rejection.
- Do not widen the shared `Vertex`.
- Do not add to the hardcoded wild-tree species in `src/terrain/planet_chunks.rs`. PRIORITIES
  already commits to converting that to data.
- Do not regenerate plant meshes for autumn. SpeedTree deliberately does not make seasons
  keyframable and cross-fades exported seasons instead: **seasonal change is a material and
  leaf-density problem, not a geometry-regeneration problem.**
- Do not run a self-organizing growth simulation (Palubicki et al. 2009) at runtime; it is
  hundreds of ms to seconds per tree. Use it as an **offline oracle** to generate growth
  sequences and fit parameter trajectories. Its pipe-model diameter rule
  (`d^n = d1^n + d2^n`, n between 2 and 3) is better than Weber-Penn's radius falloff and is
  worth adopting on its own.
- Do not port GPL code.
- Do not build a plant inspector as a new page without justifying it against "every page must
  earn its existence".

## The Real/Sim mirror

`docs/design/two-realities.md` obliges naming the real-life half of any game system. For plants
it is **real plant identification and diagnosis**: help a real gardener work the same diagnostic
process on a real plant, and track a real garden's plantings and harvests. That half does not
exist in the app today.

It is the clearest case in the project where the game mechanic and the real-life tool are the
*same model pointed in opposite directions*: the game generates symptoms from causes, and the
real tool infers causes from symptoms. Which is also exactly why the symptom aliasing above
matters. A real tool that pretended the map was one-to-one would give confidently wrong advice
to someone whose food depends on it.
