# Procedural Plants: seed to senescence from numbers

> Status: DESIGN, not yet built. Written 2026-07-30.
> Read `docs/design/infinite-of-x.md` first. This document is a worked example of that rule.
> Sequencing: PRIORITIES puts the water arc first and plants third in the hyper-realism
> sequence (water, clouds, plants). Nothing here preempts that. This is the design the
> plant arc executes when it comes up.

## The question

We need plants that go from seed to seedling to sprouting all the way to senescence,
for pumpkins, strawberries, apples, redwoods, grapevines, and everything else, including
ailments: yellowing leaves, insects eating them, burns. As close to real as we can get.
And we want the whole plant kingdom to fit in a few GB.

The instinct that "fruit and leaf shape are probably the bare minimum art" is close, but
it is one step short. The real answer is that **leaf and fruit shape are also numbers**,
and the minimum art set is closer to zero files than to thirty.

## What already exists (do not rebuild any of this)

| Piece | Where | State |
|---|---|---|
| Procedural plant mesh generator | `src/renderer/plant_mesh.rs` (712 LOC) | WORKING. 6 form archetypes, continuous growth `t`, a `wilt` axis, deterministic per-plant seeding, and `generic_visual()` so no crop is ever unrendered. |
| Per-species visual recipe | `data/plants_visual.ron` | WORKING, hot-reloadable. 19 of ~134 crops have an authored recipe; the rest fall back to `generic_visual()`. |
| Agronomic species data | `data/plants.csv` | 134 species, real USDA/FAO-derived numbers. |
| Per-plant stress schema | `data/entities/plants/plant_001.ron` | AUTHORED BUT UNPARSED. Water/nutrient/temperature/light/space stress, `disease_state`, `pest_pressure`, biomass/root/canopy indices, `yield_potential`, `edibility_state`. |
| Per-species physiology | `data/entities/plants/tomato.ron` | AUTHORED BUT UNPARSED. Per-stage duration and NPK, DLI/PPFD/photoperiod, Kc evapotranspiration, CO2, rotation group, `common_pests`, `common_diseases`. |
| Plant genetics | `data/genetics.ron` | AUTHORED BUT UNWIRED. 12 heritable plant traits, 5 genetic diseases each carrying a `visual_sign` string. |
| Disease contagion engine | `src/systems/ecology.rs` | WRITTEN AND WORKING, but unregistered and never touches crops (it queries `Health` + `Transform`; crops are spawned with neither). |
| Impostor baker | `src/renderer/billboard_bake.rs` | WORKING and already generic over `BakePart { vertices, indices, texture }`. |
| LOD registry | `data/lod/categories.ron` + `src/lod_registry.rs` | WORKING, with grass/shrub/tree rows and Settings sliders already rendered. |

The honest summary: **most of this system is already on disk and simply not connected.**
The largest single piece of work is not writing a simulation, it is parsing files we already
ship and pointing an existing disease engine at crops.

### The art situation, measured

`assets/models/plants/` is 183 MB. Two sapling photoscans (`fir_sapling`, `pine_sapling_small`)
account for 133 MB of that, and six Poly Haven photoscans account for 181 MB. They ship
redundant source and `_merged` bins plus 2K textures that `src/assets/mod.rs` downscales to
1024 on every load. Roughly 75% of the tree is unreachable by any code path.

More importantly: `.github/workflows/build-desktop.yml` copies only `assets/icons/` and
`assets/shaders/` into the release bundle. **None of the plant art ships to users.** A player
who downloads HumanityOS today sees the procedural plants and nothing else. The GLTF library
is a dev-checkout-only luxury.

By contrast the Quaternius CC0 crop pack is 18 species at 4 to 6 growth stages for **4.68 MB
total**, about 7 KB per model, using a single small palette PNG for all material colors. That
pack is the proof that the cost problem is not "3D models are big", it is "photoscans are big".

## The core idea

**A plant is not a model. It is a program plus a parameter block.**

Everything below follows from refusing to store a plant as geometry.

## Layer 0: four primitives, not a model library

The single most useful structural finding from surveying the field: Xfrog shipped separate
Tree, Horn, and Leaf components for years and eventually **collapsed them into one**, because
they were architecturally the same thing. A branch, a petiole, a fruit stalk, an awn, a
tendril, and a root are all one swept generalized cylinder with different numbers.

That gives a four-primitive kernel which expresses the entire plant kingdom:

| Primitive | What it is | What it covers |
|---|---|---|
| **Sweep** | a cross-section carried along a spine, both varying per station | every stem, branch, trunk, petiole, awn, tendril, root, and the curved fruits (banana, cucumber, pepper, carrot, pea pod) |
| **Lathe** | a profile curve revolved, with rib and bend modulation | apple, tomato, orange, plum, melon, squash, eggplant, grape berry, and every fruit that is a solid |
| **Phyllotaxis** | elements distributed over a surface of revolution | pineapple fruitlets, corn kernels, strawberry achenes, sunflower head, pine cone, artichoke, composite flowers |
| **RadialArray** | n copies about an axis | petals, sepals, whorls, palm fronds |

Four generators plus per-species numbers. This is a much better match for the infinite-of-X
rule than one monolithic parameter struct, and it means the "hard" fruits stop being special
cases: they are assemblies built from primitives we needed anyway.

### How many numbers is a plant, actually

Counted from primary sources rather than secondary summaries:

- **Weber-Penn** (SIGGRAPH '95, the canonical parametric tree) is **exactly 80 named
  parameters** at 4 levels: 12 global, 13 trunk, 15 per branch level times 3 levels, 4 leaf,
  AttractionUp, and 5 pruning. At `Levels=3` it is 65. Arbaro's `quaking_aspen.xml` contains
  exactly 80 entries, which independently confirms the count.
- **proctree**, the deliberately minimal generator, gets a plausible tree from **19 shape
  floats plus a seed**, about 80 bytes.
- **ez-tree** presets are 60 scalars, roughly 45 of them geometric.

So one tree species is **60 to 90 floats, 240 to 360 bytes**. Ten thousand species fits in
3 MB. Parameterization is emphatically **not** the bottleneck. Generation throughput and LOD
are.

And the rest of the plant kingdom is *cheaper* than the tree, not more expensive:

- **Leaf outline**: the simplified Gielis equation SGE-1 is **two** parameters (size, elongation)
  with R-squared above 0.985, validated on 3,310 leaves across 53 species. SGE-2 is three.
- **Fruit profile**: the explicit Preston equation is **five** floats, validated as a solid of
  revolution on 751 muskmelons. It also has a **closed-form volume integral**, which is a free
  gift: yield mass, nutrition, and trade value scale with real volume without ever building the
  mesh. That matters for the 99.9% of fruit that is simulated but never rendered.
- **Rib and lobe modulation**: three floats. Use `|cos(k*theta/2)|^q` with `q < 1`, not a sine,
  because pumpkin and acorn squash sinuses are creases, not sinusoids. Use odd lobe counts
  (3, 5, 7): Weber-Penn explicitly warns that even counts read as artificially symmetric.
- **Phyllotaxis**: two floats (divergence angle, spacing) plus a count.

### Where closed forms genuinely die

Gielis cannot express cordate, palmate, pinnate, or any compound leaf, and no number of extra
parameters fixes it: a single-valued `r(phi)` cannot fold back past its own attachment point,
cannot recurse, and cannot express the sinus-to-lobe feedback that is the actual biological
mechanism. Cordate is the sneaky one, because it looks like it should be easy. Budget a
separate generative path for compound leaves from the start rather than discovering this after
building the whole leaf system on Gielis.

Two workable approaches, and we validated the second:

1. **Runions, Tsiantis and Prusinkiewicz (2017)** is the rigorous answer. Leaf form emerges
   from marginal convergence points, veins growing toward them, and blade webbing. Their Fig. 9
   is a literal two-axis morphospace: **timing of lateral vein outgrowth** by **rate of webbing**
   walks the whole range from entire to serrate to lobed to recursively lobed, and simultaneously
   swings venation from pinnate to palmate. Roughly 5 to 8 parameters for the entire eudicot
   range. It is an iterative simulation at tens to hundreds of ms per leaf, so it is an
   **offline bake only**: run it once per species at load, cache an outline polygon plus vein
   polylines, and the runtime representation is then identical to the closed-form case.
2. **A width-profile family**, which is what we prototyped and validated. A midrib with
   `w(s) = s^p (1-s)^q` moves the widest point anywhere along the blade (ovate, elliptic,
   obovate, lanceolate, linear, needle); a periodic width modulation gives margins; a deeper one
   gives pinnate lobing; and a basal term that pulls low stations back behind the petiole gives
   **cordate**, which Gielis cannot. A separate radial `r(phi)` family with N lobe tips and a
   basal sinus gives palmate (maple, grapevine, pumpkin, cotton). Compound leaves are an
   arrangement rule applied to either.

A validation sheet generating 17 species this way, including every species named in the original
question, came to **520 bytes of parameters for all of them combined**, with venation drawn
procedurally rather than authored.

Three outline families cover essentially every vascular plant leaf:

1. **Simple**: a midrib with a width profile `w(s) = s^p (1-s)^q`, normalized. Two exponents
   move the widest point anywhere along the blade, which gives ovate, elliptic, obovate,
   lanceolate, linear, and needle. A periodic modulation of the width gives margins
   (serrate, dentate, crenate). A deeper modulation gives pinnate lobing (oak, dandelion).
   A basal term that pulls low stations back behind the petiole gives cordate and sagittate.
2. **Palmate**: a radial `r(phi)` about the petiole with N lobe tips and a basal sinus.
   This is maple, grapevine, pumpkin, squash, cucumber, cotton, castor.
3. **Compound**: an arrangement rule (trifoliate, pinnate, palmate) applied to a blade from
   family 1 or 2. This is strawberry, clover, tomato, ash, walnut, horse chestnut.

That is 5 to 9 floats per leaf. A validation sheet generating 17 species this way, including
every species named in the original question, came to **520 bytes of parameters for all of
them combined**. Venation is drawn procedurally from the same parameters, not authored.

Fruit and other organs follow the same principle: a **profile curve revolved around an axis**,
plus angular rib modulation, plus an optional bend. One generator produces apple, tomato,
orange, plum, grape, pumpkin (ribs), banana (bend), and strawberry (cone plus achene dots).
The genuine exceptions that need extra machinery are few and known: corn ears and wheat spikes
need grain packing on an axis, grape clusters need a branching rachis, and pineapple needs a
tiled hexagonal surface. Three extra rules, not three extra model files.

Bark and skin are procedural triplanar noise, not scanned textures.

**Result: the bare minimum hand-made art for the entire plant kingdom is close to zero files.**
Not thirty leaf textures. Zero, plus a handful of optional detail masks if we later decide a
particular hero species deserves one.

## Layer 1: one clock, no stage models

The single most important decision: **do not model discrete growth stages.** Model one
continuous developmental clock and derive every organ from it.

The mechanism is that **every organ carries a birth time**:

- A node is born at `tau = i * plastochron`.
- Its leaf then grows on its own age clock, `size = f(tau - birth)`.
- Therefore young organs are automatically smaller than old ones, with no extra code.
- Therefore senescence, which takes the oldest organs first, is free.
- Therefore a seedling and a mature plant are literally the same function at different `tau`.

The named stages already in `plants.csv` (`seed:sprout:vegetative:flower:fruit:ripe`, and 95
other vocabularies across 134 species) become **labels on ranges of the clock**, used for UI,
quests, and gameplay. They stop being separate meshes. Nothing in the data has to change.

Woody perennials use the same generator one level up, in **annual increments**: a shoot born
in year Y extends exactly one segment that year; the following year its tip spawns the next
generation of shoots and the original segment only thickens. That gives secondary thickening
(a trunk becomes a trunk because its radius keeps accruing every year it lives), deciduous
leaf drop on current-season wood, dormancy, and bearing age (apples form on spurs on wood at
least two years old, which is why a young tree is all leaf and no crop).

Redwoods and grapevines are the same generator with different branching parameters and a
different vigor decay. A redwood is a strong leader with weak laterals; a grapevine is a weak
leader with strong laterals and tendril attachment.

### Driving the clock

Today growth is `elapsed_wall_clock / growth_days`, multiplied by health. That is a timer,
not a plant. The upgrade path, in order of value per unit of work:

1. **Growing degree days** (thermal time). `GDD = sum of max(0, mean_temp - t_base)`. This is
   how real agronomy predicts phenology, it needs one new number per species (`t_base`) plus
   a per-stage GDD threshold, and it replaces exactly one line in `farming/mod.rs`. It also
   makes a cold season genuinely slow a crop instead of just penalizing health.
2. **Photoperiod**. Short-day, long-day, day-neutral, plus vernalization and chilling hours
   for fruit trees. This is roughly 5 numbers per species. Note the current blocker:
   `src/systems/time.rs` hardcodes a 06:00 to 18:00 sun arc with no latitude and no seasonal
   day-length variation, so real photoperiod needs solar geometry first.
3. **Per-location climate**. Weather is currently one global value with no spatial dimension
   and no diurnal swing. Season plus global temperature is the honest ceiling until a spatial
   climate field exists. Do not pretend otherwise in the UI.

## Layer 2: health as pigments plus masks

The current model is a single `health` scalar 0 to 100, dropping only from dehydration and a
WiFi router RF penalty, and `wilt` is literally `1.0 - health/100`. Every ailment therefore
looks identical: browner.

The replacement has two halves.

### Half one: color is pigment concentration, not authored RGB

Leaf reflectance is a physical function of pigment load. The standard model is **PROSPECT-D**,
which takes 7 inputs (leaf structure N, chlorophyll Cab, carotenoid Car, anthocyanin Anth,
brown pigment Cbrown, water Cw, dry matter Cm). Only 5 of those do anything in the visible
band, since water and dry matter absorb in NIR and SWIR. So the game needs at most a 5-D
lookup, and realistically a 3-D one over (Cab, Anth, Cbrown) with Car and N as cheap analytic
corrections. Baked to a small 3D texture that is single-digit MB, or fitted to about 20 floats
per species. MIT-licensed reference implementations exist (`jbferet/prospect`), and measured
validation sets with matched reflectance and pigment assays are public (ANGERS 2003, LOPEX93).

Then every symptom in the game is **one of exactly four operations**:

1. **Perturb the pigment field.** Chlorosis is Cab down. Purpling is Anth up. Necrosis is
   Cbrown up with everything else to zero. Senescence is Cab falling faster than Car, and
   that ratio *is* the yellowing.
2. **Overlay a superficial layer.** Powdery mildew, sooty mold, rust spore dust, honeydew
   gloss. These occlude albedo and must NOT be written into the pigment field.
3. **Change transmission.** Water-soaking (bacterial lesions, late blight, frost) is a
   translucency change before it is a color change. Leaf-miner tunnels are the same class.
4. **Change geometry.** Chewing holes (alpha cutout), skeletonization (alpha keyed to vein
   distance), galls and rust pustules (displacement), wilt and leaf roll (vertex deform),
   reduced leaf size (zinc "little leaf").

### Half two: one packed leaf atlas drives every spatial pattern

A single RGBA texture per leaf archetype carries:

- **R** = distance to nearest vein
- **G** = distance to margin
- **B** = areole cell id (Voronoi cells bounded by minor veins)
- **A** = base-to-tip midrib coordinate

That one texture generates interveinal chlorosis (R), marginal scorch and tip burn (G),
bacterial angular lesions that stop dead at a vein (B), the V-from-the-tip nitrogen pattern
in grasses (A), and skeletonization (R again). Add two per-leaf scalars, **leaf age** and
**adaxial vs abaxial**, and every symptom can be placed correctly.

### The educational payload is the placement, not the leaf

This is the part that makes the game teach reality rather than decorate it.

Phloem mobility is a hard physiological rule. **Mobile** nutrients (N, P, K, Mg, Mo, Cl) are
stripped out of old leaves to feed new growth, so their deficiency symptoms appear on the
**oldest** leaves. **Immobile** ones (Ca, S, Fe, Mn, B, Cu, Ni; Zn intermediate) cannot be
recovered, so symptoms hit the **newest** leaves.

We get this for free, because Layer 1 already knows every organ's birth time. A player who
internalizes "yellow bottom leaves versus yellow top leaves" has learned the single
highest-yield real-world diagnostic split there is. Layered on top: interveinal versus uniform,
margin versus blade, a sharp green vein net (iron) versus a blurry one (manganese), and whether
leaf size changed at all (zinc).

Roughly 24 conditions are genuinely nameable from pattern alone. Another 8 or so are honest
look-alikes that require context: which leaves, is the soil wet, are other species affected,
what changed last week. A few are undecidable without a lab test, including "hidden hunger",
where yield drops with **zero visual symptom**. Simulating that honestly is more educational
than a game where looking is always sufficient, and we should ship the ambiguity rather than
sand it off.

### What this costs

A dozen mask functions in WGSL, four pigment scalars, and roughly 4 bytes of per-leaf runtime
state (condition id, severity, seed, age). **Zero art files.**

A validation sheet rendering 22 distinct conditions this way, from nitrogen deficiency through
chewing holes, leaf miners, powdery mildew, rust pustules, early blight target-spot rings,
vein-bounded bacterial lesions, mosaic virus mottling, and senescence, confirmed all 22 read
as visually distinct and diagnostically correct.

## Layer 3: species as inheritance, not enumeration

Do not author 400,000 species. Author roughly 100 family and growth-form archetypes, and
express each species as a **delta** from its archetype. `Rosaceae/tree` covers apple, pear,
cherry, and plum with about 10 numbers of difference between them.

This is the infinite-of-X rule applied honestly, and it is also how the existing
`generic_visual()` fallback already behaves. Extending species coverage from 19 authored
recipes to all 134 becomes data entry, and beyond that becomes generation.

## Layer 4: how it renders, and the three real blockers

The current path: `src/engine/home_meshes.rs::rebuild_plant_meshes` CPU-generates flat-shaded
triangles for every `CropInstance`, merges every plant in a tower or bed into ONE mesh, and
draws it as a single identity-transform object with material type 12. That is genuinely clever
and it works. It also has hard limits.

**Blocker 1: the 32-byte vertex.** `Vertex` is exactly `position f32x3 + normal f32x3 + uv f32x2`.
There is no color channel, no second UV, no tangent. Per-face RGB is smuggled through `uv.x`
as an exact integer, with `uv.y` carrying blue. Bits 16, 17, and 18 are taken (water, tree card,
grass card) and only bits 19 to 23 remain before f32's 2^24 exact-integer ceiling. Textured leaf
cards and damage masks both need real UVs, so both are blocked simultaneously. Widening `Vertex`
touches every mesh producer in the repo plus six pipeline vertex states, so the plant path should
get **its own vertex format and its own material type (20 is free)** rather than widening the
shared one or extending type 12.

**Blocker 2: type 12 is overloaded with planet semantics.** Plants currently inherit an
unconditional terminator gate that reads `material.base_color.xyz` as a planet centre, and they
cannot use base_color as a tint because the fallback multiplies the packed color by it. Claim
type 20.

**Blocker 3: plants are homestead-only.** Plant geometry is baked in absolute homestead world
coordinates and drawn with an identity model matrix, so it cannot ride the floating origin or
planet rotation. Putting procedural plants on a planet surface requires anchoring the way
`far_trees.rs` already does.

Additional constraints that shape the design rather than block it:

- Group 3 is full at bindings 0 to 15, and 4 bind groups is wgpu's baseline `max_bind_groups`.
  Any new plant texture or atlas must join group 3, and adding a binding means updating EVERY
  `create_bind_group` site for that layout. That exact mistake broke ten consecutive releases.
- There is no GPU instancing for meshes. The one instance-rate attribute is spoken for by
  terrain patches. `src/renderer/patch_arena.rs` is the in-repo template for doing it properly
  (shared mega buffers, `RangeAlloc`, `multi_draw_indexed_indirect`).
- There is no wind or vertex animation for foliage anywhere. When it is added, it must be
  evaluated in the camera-anchored 64 m-modulus domain with wavelengths dividing 64, per the
  f32-at-planet-scale rule, not from a planet-radius-magnitude dot.
- Every plant triangle costs 108 bytes because faces are fully unshared, which is a hard
  requirement of the packed-color flat-shading contract and is locked by a test.
- `billboard_bake.rs` is already generic over `BakePart`. Pointing it at `PlantMeshBuilder`
  output gives plant impostors with **zero new baking code**.

### LOD ladder

- **0 to 20 m**: full procedural mesh, per-organ, with pigment and damage masks.
- **20 to 150 m**: reduced organ count, leaf clusters merged into cards.
- **150 m and beyond**: octahedral impostors from the existing baker.
- **Far**: contribution to terrain albedo only.

Declare these distances as rows in `data/lod/categories.ron`, which already renders Settings
sliders per category. Do not invent new distance constants.

## The storage math, honestly

The "millions of plants in a few GB" framing needs one correction up front: **procedural
generation saves disk, not VRAM.** `.kkrieger` was 96 KB on disk and expanded to 200 to 300 MB
in RAM with long load times. The win is real but it is a disk and authoring win.

With that said:

| Thing | Cost |
|---|---|
| One species parameter block | 60 to 120 floats plus a few strings, well under 1 KB |
| All ~134 current crops | a few hundred KB |
| All ~400,000 known vascular plant species at 1 KB each | ~400 MB |
| One plant instance (position, species id, seed, age, health) | 8 to 32 bytes |
| One million instances | 8 to 32 MB |
| Pigment LUT plus leaf field atlases | single-digit MB |

So yes: the entire plant kingdom fits in single-digit GB, and a rich playable subset fits in
well under 1 GB. That is less than the 183 MB of GLTFs currently sitting in the repo for six
photoscans that never reach a user.

**The real constraint is not storage, it is pixel shading of alpha-tested leaves.** Keep total
on-screen alpha-tested coverage under roughly 2 to 4 times screen area. And do NOT generate a
unique mesh per instance: generate 8 to 32 mesh variants per species and vary instances by
transform, tint, and wind phase. Generating a 2k to 8k triangle plant in release Rust costs
roughly 50 to 500 microseconds on one core, which is fine for variant baking and far too slow
to do per instance per frame.

## Build order

Each increment is independently shippable and independently visible.

1. **Wire what is already authored.** Parse `data/entities/plants/*.ron` into a real per-plant
   stress-channel state, replacing the single opaque `health` scalar. Give crops `Transform`
   and `Health`, register `EcologySystem`, and blight starts spreading bed to bed with almost
   no new simulation code. Add `plants_visual.ron` to `src/embedded_data.rs` so a shipped build
   stops silently degrading every crop to a generic rosette.
2. **Growing degree days.** Replace the elapsed-time growth line with a thermal-time
   accumulator. Requires adding accumulator fields to `CropInstance`, which is a save-format
   change, so do it outright with no compat shim.
3. **The botanical parameter schema.** Replace the flat 20-field `PlantVisualDef` with leaf
   family parameters, phyllotaxis, branching, and organ birth times. Add `schemas/plant_visual.toml`
   (there is no plant schema today).
4. **One recursive growth-unit generator** replacing the 6 hardcoded form archetypes, so
   redwood, grapevine, and pumpkin vine all come out of one code path.
5. **Material type 20 plus a plant vertex format** with real UVs, a pigment word, and a damage
   word. This is the gate for everything visual in Layer 2.
6. **The pigment LUT and the procedural mask library** in WGSL.
7. **Impostors, instancing, and wind.** Point `billboard_bake.rs` at `PlantMeshBuilder`, then
   follow the `patch_arena.rs` pattern for a vegetation arena.

Increments 1 and 2 are pure simulation and need no renderer work at all. They are the cheapest
realism per hour in the whole plan.

## Deliberately out of scope

- Do not iterate the far-tree card sheet. It is default OFF with a twice-repeated operator
  rejection, and long-range trees arrive only via the instancing and impostor arc.
- Do not widen the shared `Vertex`. Give plants their own format.
- Do not add to the hardcoded wild-tree species in `src/terrain/planet_chunks.rs`. PRIORITIES
  already commits to converting that to data.
- Do not build a plant inspector as a new page without justifying it against "every page must
  earn its existence". Folding it into the existing Garden and Inventory surface is the default.

## The Real/Sim mirror

`docs/design/two-realities.md` obliges naming the real-life half of any game system. For plants
it is **real plant identification and diagnosis**: point the same pigment-and-pattern model at
a photo or a checklist and help a real gardener name what is wrong with a real plant, and track
a real garden's planting dates and harvests. That half does not exist anywhere in the app today.
It is deferred, not forgotten, and it is the single clearest case in the project where the game
mechanic and the real-life tool are the *same model* pointed in opposite directions: the game
generates symptoms from causes, and the real tool infers causes from symptoms.
