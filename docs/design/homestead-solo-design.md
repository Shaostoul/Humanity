# Single-Occupant Closed-Loop Homestead — Buildable Design for HumanityOS

> Status: **design complete, ready to implement.** Produced 2026-07-01 by a
> dedicated research+synthesis pass (operator request: "designing a fully
> fledged self-sustaining homestead including all the machines, furniture,
> walls, etc... People need to see the bare minimum for 100% self-sufficiency
> so they understand the importance of all supporting civilizational
> infrastructure"). See `docs/history/2026-07-01-afternoon-loop-plan.md` for
> the phased implementation backlog this feeds into (Phase A/B/C). Read
> `docs/design/self-sufficiency.md` FIRST — it's the intellectual backbone
> this design implements.

**Grounding:** every machine id, crop id, and recipe id below is verified
against the live data files. The existing `data/machines/home.ron` is
already a *3-person* designed home; this deliverable re-derives it honestly
for **ONE person**, shows the sizing math, flags real content gaps, and
gives a build order. Source files read: `data/machines/home.ron`,
`data/towers/aeroponic_configs.ron`, `data/electrical.ron`, `data/hvac.ron`,
`data/waste_management.ron`, `data/food_system.ron`, `data/plants.csv`,
`data/recipes.csv`, `data/structures.csv`,
`data/blueprints/homestead_layout.ron`, `data/garden/grow_media.ron`,
`data/medical.ron`, `data/creatures.csv`, `docs/design/self-sufficiency.md`.

The intellectual backbone is `docs/design/self-sufficiency.md`:
self-sufficiency is the **weakest of several coupled loops** (energy,
water, food, nutrients, air, heat) averaged **over the worst stretch the
climate throws at you**. Energy is measured in **Wh/day, not nameplate
watts**. The binding constraint for food is **light (single sun-lit
canopy), not floor area**.

---

## 0. One-person daily needs (the demand side — the numbers everything is sized to)

| Need | Per-person/day figure | Source basis |
|---|---|---|
| Calories | **2,200 kcal/day** complete nutrition | FAO adult reference (2,000–2,500); self-sufficiency.md food loop |
| Protein | ~55 g | standard RDA |
| Electricity | **~4.0 kWh/day** (off-grid frugal) | see §1 math; self-sufficiency.md warns nameplate ≠ energy |
| Water (household net) | **~80 L/day** (drink/cook/wash; irrigation recycled separately) | `home.ron` uses 240 L/day for 3 → 80 L/person |
| Air (O2) | ~0.55 kg O2/day; CO2 scrub to match | HVAC life-support sizing |
| Organic/nutrient waste | ~1.5 kg/day food+crop+humanure → compost | `home.ron` composter is 1.2 kg/d nutrient |

The existing home.ron's demand line "~6,600 kcal/day (3 people)" ÷ 3 =
**2,200 kcal/person** — used as the anchor throughout.

---

## 1. POWER — sized to 4.0 kWh/day, one person

### Demand build-up (Wh/day, per self-sufficiency.md's "duty cycle, not peak")
Using the actual consumer loads already in the home.ron catalog (real
per-unit draws in the data, not nameplate):

| Load (machine id / catalog `power`) | Watts | Duty | Wh/day |
|---|---|---|---|
| `air_recycler` (Consumer 25 W, priority 1 — life support) | 25 | 24 h | 600 |
| `water_pump` (Consumer 10 W) | 10 | ~6 h | 60 |
| `water_purifier` (Consumer 13 W) | 13 | ~6 h | 78 |
| `irrigation_system` (Consumer 7 W) | 7 | ~4 h | 28 |
| `network_uplink` (6 W) + `home_server` (30 W) | 36 | 24 h | 864 |
| 9 tower pumps @ 15 W (scaled for 1 person, see §3) | 135 | ~2 h pulsed | 270 |
| `aquaponic_tank` air pump (50 W) ×1 | 50 | 24 h | 1,200 |
| Lighting: 4× `led_light` (10 W ea, `electrical.ron`) | 40 | 5 h | 200 |
| Cooking `oven`/`stove` (structures.csv `oven` = 2,400 W) | 2,400 | 0.25 h | 600 |
| Misc (fridge/`freezer` 200 W, chargers, fan) | — | — | ~700 |
| **TOTAL DEMAND** | | | **~4,600 Wh/day ≈ 4.6 kWh/day** |

Trim the always-on internet stack (server + uplink = 864 Wh, a *want* not a
survival *need*) and one person lands at a **~3.7–4.0 kWh/day survival
floor**, ~4.6 kWh/day with comforts. Design target: **4.0 kWh/day**.

### Supply: solar sizing (the honest Wh/day calc)
- `solar_panel` in both `home.ron` catalog and `electrical.ron`:
  **peak_watts = 400 W**, efficiency 0.22.
- Real available energy = panel W × **sun-hours/day** (self-sufficiency.md:
  "~4.5 sun-hours" temperate assumption), × system losses
  (inverter+wiring+charge, ~0.80).
- Per panel/day: `400 W × 4.5 h × 0.80 = 1,440 Wh ≈ 1.44 kWh/day` — this
  **exactly matches** the catalog stat `"+1.4 kWh/d"`. The data is
  self-consistent.

**Panels needed:** `4.0 kWh ÷ 1.44 kWh/panel = 2.8 → round up to 3 panels`,
add 1 for winter/cloud derate → **4× `solar_panel`** (vs 8 in the 3-person
home; halving-plus is correct since one person is ~45% of the 3-person
energy load).

Supply check: `4 × 1.44 = 5.76 kWh/day` gross vs 4.0 kWh demand → **44%
headroom** for cloudy days and winter (when sun-hours can halve). At 2.5
winter sun-hours: `4 × 400 × 2.5 × 0.80 = 3.2 kWh` — dips below demand,
which is what the battery + generator cover.

### Storage: battery autonomy
- Catalog `battery_bank`: `capacity_wh: 4000, max_charge/discharge 2000 W`
  = **4 kWh usable/bank**.
- Days of autonomy = `storage ÷ daily_demand`. For a **~2 days autonomy**
  target (self-sufficiency.md style): `2 × 4.0 = 8 kWh → 2× battery_bank`.

**→ 2× `battery_bank` (8 kWh, ~2 days).** (The 3-person home uses 8 banks /
32 kWh; scaling by load 4.0/12.0 ≈ ⅓ → ~2.7 banks, so 2–3. Use 2, keep the
generator.)

### Backup + wind
- `generator_portable` (catalog: backup 2 kW, fuel) ×1 — covers the deep
  multi-day no-sun-no-wind tail.
- `wind_turbine_small` (catalog `Generator(watts: 150)` → "+0.7 kWh/d") ×1
  optional: adds nighttime/winter kWh when solar is weakest. Keep 1 for
  redundancy per self-sufficiency.md's "backup source per critical loop."

**POWER BILL OF MATERIALS (1 person):** 4× `solar_panel`, 2× `battery_bank`,
1× `wind_turbine_small`, 1× `generator_portable`. Wired as home.ron already
models: each panel home-runs to a bank, banks tied into one bus, bus feeds
loads by priority (air_recycler = priority 1, shed last).

---

## 2. WATER — sized to 80 L/day net, one person

### Demand
- `home_water_use` catalog models **240 L/day for 3** (drink/cook/wash) →
  **80 L/day for 1**.
- Irrigation (`irrigation_system`) is **not net demand**: greywater
  recycling covers it (home.ron: composter "+155 L/d greywater"). Net
  demand = household 80 L/day only.

### Supply + storage
- `water_pump` catalog: powered well lift **2.0 L/min** = 120 L/hr →
  refills 80 L in ~40 min/day. Ample.
- `water_purifier` catalog: **+240 L/d potable** throughput (polishes,
  doesn't source) — 3× the 1-person need, so keep 1 unit; it's not the
  bottleneck.
- `water_tank` (Cistern) catalog: **8,000 L**, passive rain catchment
  0.1 L/min (~144 L/day, free, no power).
- **Autonomy = storage ÷ demand = 8,000 L ÷ 80 L/day = 100 days.** The
  3-person home cites 33 days from the same 8,000 L cistern; for one person
  the *same tank* gives 100 days — could downsize to a ~3,000 L cistern and
  still hold ~37 dry days, but there's no separate 3,000 L tank id, so keep
  **1× `water_tank`** (over-provisioned buffer is a feature, and
  self-sufficiency.md says dry-season storage is the binding constraint).

**WATER BOM (1 person):** 1× `water_tank` (8,000 L cistern), 1×
`water_pump`, 1× `water_purifier`, 1× `home_water_use` (the tap/shower sink
node). Plumbing island: cistern → pump → purifier → irrigation + household
taps (exactly the home.ron connection topology).

Real fixtures from `structures.csv`: `rain_collector` (rooftop barrel w/
filter), `sink`, `shower_unit` (in wetroom equipment), `toilet`,
`septic_tank` — all exist and drop into the wetroom/bathroom.

---

## 3. FOOD — the weakest loop (honest, per self-sufficiency.md's light cap)

### Demand: 2,200 kcal/day + complete nutrition, one person.

### The canopy cap (why this is the binding loop)
self-sufficiency.md is explicit: **free sunlight is a single canopy**; a
room's output is capped near its illuminated floor footprint no matter how
many towers you stack. The biointensive anchor: **~700–1,000 m² of
intensive growing feeds one person a complete diet for a year.** The F34
garden room is 34×34 = **1,156 m²** — so for **one person this room is
finally correctly sized** (it was over-subscribed at 3 people). This is the
pedagogical payoff: **the same garden that was "the weakest loop, ~half the
calories" for 3 people actually closes for 1.**

### Sizing the indoor garden for ONE person

Using the calorie stats already in the home.ron catalog and real crop data
from `plants.csv`:

| Machine id | Catalog kcal/unit/day | Qty for 1 person | kcal/day | Role |
|---|---|---|---|---|
| `potato_grow_bed` | +120 kcal | **8 beds** | 960 | Bulk indoor calories (aeroponic potato, NASA/CIP-real) |
| `aeroponic_tower_nutrition` ("Variety tower") | +92 kcal | **9 towers** | 828 | ALL vegetables/vitamins/plant protein (32 crops, see below) |
| `oilseed_bed` | +25 kcal (fats) | **3 beds** | 75 | Indoor fats (sunflower/flax from plants.csv) |
| `mushroom_rack` | +50 kcal | **2 racks** | 100 | Uses dark vertical space, no light — see gap ⚠️ |
| `aquaponic_tank` | +75 kcal | **1 tank** | 75 | Closes **B12 + omega-3** — see gap ⚠️ |
| `staple_grain_tray` | +20 kcal | **2 trays** | 40 | Token cereal (worst per-m² indoor crop, kept minimal) |
| **Indoor subtotal** | | | **~2,078 kcal/day** | |

**Indoor alone reaches ~2,078 kcal ≈ 94% of the 2,200 kcal one-person
need** — vs the ~50% it managed for 3 people. The small remainder +
preservation buffer is carried by ONE outdoor field, not four:

| Machine id | Catalog kcal/day | Qty | Role |
|---|---|---|---|
| `grain_field` | +3,100 kcal | **1** (or a small fraction of one) | Bulk calorie surplus → dried/stored |
| `legume_field` | +700 kcal, **+N fixing** | **1** | Protein + closes nitrogen for the grain field |
| `grain_silo` | "750 days stored" | **1** | Winter/bad-harvest buffer (self-sufficiency.md preservation term) |

With one `grain_field` the home is now in **calorie surplus** for a single
person (surplus → silo → trade, which is the point of scaling to
"infinite"). Drop `tuber_field` and `oilseed_field` entirely for the solo
build (they were 3-person bulk).

### The 9 variety towers — exact crop ids (all real in `plants.csv`, all in `aeroponic_configs.ron` tower `"nutrition"`)
One tower already carries **32 distinct crops** at 1 slot each (50-slot
capacity), covering every food group. From `data/plants.csv` (verified
ids): `lettuce, spinach, kale, cabbage, broccoli, cauliflower, beet,
turnip, radish, carrot, parsnip, celery, leek, onion, garlic, chive,
cucumber, zucchini, tomato, pepper, eggplant, okra, strawberry, bean, pea,
soybean, lentil, chickpea, parsley, cilantro, basil, dill, mint`. Nine such
towers give abundant redundancy and continuous harvest for one person. Add
**1× `aeroponic_tower_apothecary`** (the `"apothecary"` config: 24
herbs/remedies incl. `chamomile, lavender, ginger, turmeric, echinacea,
calendula, aloe_vera`) for teas, seasoning, and folk remedies.

### Grow media (all real in `data/garden/grow_media.ron`)
`aeroponic` (towers), `soil_bed` (potato/oilseed `_bed` suffix match),
`grain_tray`, `mushroom`, `aquaponic`, `field`. No new media needed.

**FOOD BOM (1 person):** 9× `aeroponic_tower_nutrition`, 1×
`aeroponic_tower_apothecary`, 8× `potato_grow_bed`, 3× `oilseed_bed`, 2×
`staple_grain_tray`, 2× `mushroom_rack`, 1× `aquaponic_tank`, 1×
`grain_field`, 1× `legume_field`, 1× `grain_silo`, 1× `irrigation_system`.

### Cooking/preservation (all real recipes in `recipes.csv`, machines in `structures.csv`)
`oven` + `stove` (from structures.csv) run: `grind_flour` (grain_mill) →
`cook_bread`, `cook_soup`, `cook_stew`, `cook_canned_food`,
`cook_protein_bar`, `cook_dried_fruit`, `cook_dried_meat`, `cook_juice`.
Preservation to survive the seasonal gap: canning/drying via those recipes
+ `pantry` + `freezer` (structures.csv). This closes the "preservation
buffer" term self-sufficiency.md requires.

---

## 4. AIR — sealed-habitat life support, one person

The home is modeled as a **sealed** habitat (space/mothership context), so
air is a hard closed loop per self-sufficiency.md §5.
- `air_recycler` catalog: Consumer 25 W (priority 1), Air OUT 20 L/min —
  sized for a 3-person household "with margin." **One 25 W unit vastly
  over-serves one person; keep 1** (redundancy + the priority-1 shed-last
  behavior is the teaching point: cut power → air suffocates).
- The garden's plant canopy is itself a bioregenerative O2 source
  (self-sufficiency.md: "plant/algae area per person") — 1,156 m² of
  canopy is far more than one person's O2 needs, so the recycler is
  backup, not primary.
- Real alternatives available in `hvac.ron` if a bigger sealed build is
  wanted: `oxygen_generator` (electrolysis, capacity_people 8),
  `co2_scrubber` (capacity_people 10), `air_recycler` (Sabatier,
  capacity_people 12). For **one person the home.ron `air_recycler` is the
  right, cheapest choice.**

**AIR BOM:** 1× `air_recycler`. On open-Earth (non-sealed) builds this
becomes free ventilation (hvac.ron `intake_fan`/`exhaust_fan`/
`hepa_filter`) and the recycler is omitted.

---

## 5. WASTE / NUTRIENTS — the loop-closer, one person

self-sufficiency.md §4: waste cycling is *what lowers every other demand
term* — closing nutrient + water loops drives external inputs toward zero.
- `composter` catalog: +1.2 kg/d nutrient, +155 L/d greywater. **One
  person produces ~1.5 kg/day** organic (food scraps + crop residue +
  humanure). **1× `composter` suffices** (the 3-person home used 2 to
  bridge the 60–90 day compost maturation lag; for one person a single
  staged bin is adequate, but a 2nd is cheap insurance for the maturation
  gap).
- Nutrient closure math (home.ron loop): compost + urine diversion give
  **N 587% / P 533% / K 197%** of crop demand — potassium is the limiting
  nutrient at ~2×, still in surplus. `legume_field` fixes nitrogen.
  `aquaponic_tank` fish waste fertilizes towers (closed fish→plant loop).
- Real facilities in `waste_management.ron` that map 1:1: `compost_bin`
  (0 W, `wood_log×10`), `compost_tumbler` (faster), `biodigester` (methane_
  fuel 0.30 + digestate_fertilizer 0.55 → feeds the backup generator's
  fuel loop!), `septic_tank` (structures.csv) for blackwater.

**WASTE BOM:** 1–2× `composter` (use `compost_bin`/`compost_tumbler` from
waste_management.ron as the real-world mapping), optional 1×
`biodigester` to turn humanure/scraps into `methane_fuel` for
`generator_portable` (closes a slice of the energy loop too).

---

## 6. THE HOUSE ITSELF — rooms, walls, furniture (all real, no new data)

Room shells from `data/blueprints/homestead_layout.ron` (Fibonacci-sized,
3 m ceilings). For a **solo** home, use the smaller subset and skip the
3-person-scale rooms:

| Room id | Dims (m) | Purpose | Keep for solo? |
|---|---|---|---|
| `respawner` | 2×2 | character/spawn anchor | yes (spawn) |
| `wetroom` | 3×3 | shower, laundry, water recycler | yes |
| `bathroom` | ~1.5×1.5 | toilet, sink, waste_processor | yes |
| `bedroom` | 5×5 | sleep | yes |
| `kitchen` | 8×8 | cook + hologram/spawn | yes |
| `livingroom` | 13×13 | living | optional (downsize) |
| `study` | 21×21 | fabrication + network | yes (workshop) |
| `garden` | 34×34 | the food engine | **yes — correctly sized for 1** |
| `garage` | 55×55 | power + water cluster, solar on hull | yes (can shrink) |
| `depot`/`hangar`/`ranch` | 89 / 144 / 233 | bulk/logistics | **omit for solo** |

**Walls/floors/doors** — all real in `structures.csv`: `concrete_foundation`,
`wood_floor`/`tile_floor` (wetroom/bathroom), `wood_wall`/`concrete_wall`/
`window_wall`, `wood_door`/`metal_door`, `green_roof` (insulation — the
envelope lever from self-sufficiency.md §6). Standard door/window sizes
come from the layout file's single-source-of-truth block
(`door_width 0.9`, etc.).

**Furniture** — all real in `structures.csv`: `bed`, `nightstand`,
`wardrobe` (bedroom); `dining_table`, `chair`, `couch`, `pantry`, `oven`,
`stove`, `freezer`, `sink` (kitchen); `desk`, `shelf`, `bookshelf`,
`tool_rack`, `electronics_bench`, `workbench` (study/workshop); `toilet`,
`sink`, `shower_unit` (wetroom/bathroom); `ceiling_light`/`floor_lamp`
(10–20 W LED, in electrical.ron). **Zero new furniture data needed.**

**Envelope/heat (self-sufficiency.md's biggest demand-side lever):**
`hvac.ron` gives `heat_pump` (COP 3.0), `geothermal_loop` (COP 4.5),
`wood_stove` (0 W, burns `wood_log` — off-grid heat), `radiant_floor`,
`evaporative_cooler` (400 W, dry-climate). For a solo off-grid home a
`wood_stove` + `green_roof` insulation is the lowest-energy pairing.

---

## 7. GENUINE CONTENT GAPS (things the design needs with NO adequate existing data)

Honest gaps to author, following the schema patterns in the files above —
**not** worked around silently.

1. ~~**No terrestrial edible mushroom crop.**~~ **CLOSED (v0.657.0,
   2026-07-01).** Added `oyster_mushroom`, `shiitake`, `button_mushroom` to
   `data/plants.csv` (type `vegetable`, no `fungus` type exists yet --
   matches the "or reuse `vegetable`" fallback this gap originally called
   for). Real-world-grounded: oyster 14-day cycle / 0.85-0.95 humidity,
   shiitake 60-day / 0.80-0.90, button 25-day / 0.85-0.95, all no-light
   (matching `mushroom_rack`'s "no light needed" stat). Regression test:
   `systems::farming::plant_registry_csv_tests::
   shipped_plants_csv_has_real_edible_mushrooms`.

2. ~~**No tank/aquaponic fish species.**~~ **CLOSED (v0.657.0,
   2026-07-01).** Added `tilapia` and `channel_catfish` to
   `data/creatures.csv` (`habitat_biomes: river`, `domesticable: true`,
   `raw_fish`/`fish_oil`/`fish_scale` loot matching the existing
   salmon/tuna rows -- fish_oil already models the omega-3 closure).
   Note: `creatures.csv` currently has **no runtime loader at all** (no
   `CreatureRegistry`/`CreatureDef` consumes it anywhere in `src/`) --
   it is pure reference data today, same status as every other row in the
   file. Adding these species is honest content-gap closure for when that
   loader exists; it does not yet make the aquaponic tank's B12/omega-3
   claim mechanically computed (that still rests on the hand-typed
   `aquaponic_tank` catalog stat in home.ron/home_solo.ron, per gap #3
   below).

3. ~~**No calorie/macro field on `plants.csv`.**~~ **CLOSED (v0.663,
   2026-07-01) — data + loader shipped, UI integration pending.** Calories
   used to live only in `food_system.ron` **nutrition_profiles** keyed by
   broad *category*, while per-machine kcal were **hardcoded strings in
   `home.ron` catalog stats** ("+120 kcal/d") — no per-crop calorie/yield-
   to-kcal bridge, so the food loop couldn't be computed from crop data.
   → **Authored `data/food/crop_nutrition.ron`:** one entry per FOOD crop
   id in `data/plants.csv` (85 entries: vegetables, fruits, grains,
   legumes, edible herbs, the three edible mushrooms; alien/decorative/
   fiber/tree/medicinal-remedy types skipped) with `plant_id`,
   `calories_per_100g`, `protein_g/fat_g/carbs_g` (USDA magnitude per 100 g
   edible), and `grams_per_yield_unit` (the bridge normalizing plants.csv's
   abstract yield units into real mass). Loaded by
   `src/systems/self_sufficiency.rs` (`CropNutrition::from_ron` + the pure
   `food_supply_kcal_per_day`). Every plant_id is cross-checked against
   plants.csv by a unit test; the potato bridge is calibrated to land
   within 2x of home.ron's "+120 kcal/d per bed" claim (924 vs 960 kcal for
   8 beds). Still to do (deferred): wire the computed supply into the
   Home-page loop summary so it replaces the hand-typed catalog strings.

4. ~~**No editable component-output table for the self-sufficiency
   score.**~~ **CLOSED (v0.663, 2026-07-01) — data + loader shipped, UI
   integration pending.** The generation/collection/recycling assumptions
   used to be baked into catalog stat *strings* in home.ron, not a
   queryable data file.
   → **Authored `data/self_sufficiency/component_outputs.ron`:** one entry
   per generation/collection/recycling machine in home.ron's catalog
   (solar_panel, wind_turbine_small, generator_portable, water_tank,
   water_pump, water_purifier, composter, air_recycler) with `{id,
   output_value, unit, assumptions}` — e.g. `solar_panel: 1.44, "kWh/day",
   "400 W peak x 4.5 sun-hours x 0.80 system losses"`. Plus
   **`data/self_sufficiency/location.ron`** (the reference location
   "Silverdale, WA": sun-hours summer/winter, annual rainfall, heating/
   cooling degree-days — real Kitsap County values). Loaded by
   `src/systems/self_sufficiency.rs` (`ComponentOutputs`/`Location`
   loaders + the pure `household_energy_balance`). Every component id is
   cross-checked against the home.ron catalog by a unit test; all figures
   were verified to AGREE with the home.ron catalog stats + this doc's
   sizing math (no disagreements found). Still to do (deferred): a
   `household_size` selector and wiring the tables into a live computed
   per-loop score with autonomy days.

5. ~~**Grow-light energy meter (flagged in the doc, not built).**~~
   **CLOSED (v0.664, 2026-07-01).** `MachineHome::grow_light_report`
   (src/machines.rs) computes the placed grow lights' draw (fixture watts
   x `GROW_LIGHT_DUTY_HOURS` = 14 h/day, the mid-range of the 12-16 h crop
   photoperiod) against the home's free headroom (generation minus all
   non-grow-light demand) and verdicts it green (within headroom) / amber
   (eating battery reserves daily) / red (the lights alone outdraw the
   whole home's generation). Rendered as a dedicated meter row in the
   construction editor's Buildability panel the moment at least one light
   is placed, with the red-state teaching note "this is why the garden
   uses the sun." A placeable `grow_light` catalog entry (100 W, matching
   `electrical.ron`, priority 5 = shed first) was added to BOTH
   `home.ron` and `home_solo.ron`; neither seed design places one (the
   reference gardens are sun-lit -- that is the lesson). Regression
   tests: `machines::tests::grow_light_meter_green_amber_red_thresholds`
   + `shipped_catalogs_offer_a_placeable_grow_light`.

---

## 8. WHAT THIS HOMESTEAD CANNOT CLOSE THE LOOP ON (the pedagogical payoff)

> **Surfaced in-app (2026-07-01, Phase C item #2):** the Home page now renders
> these five categories as the "What one home cannot close" panel, directly
> below the closed-loop summary -- data-driven from
> `data/self_sufficiency/cannot_close.ron` (edit the RON, not the code), drawn
> by `src/gui/pages/homes.rs`. Keep that file in sync when this section changes.

This is the operator's whole point: **the baseline for ONE human is what
reveals why civilization-scale infrastructure matters.** The game data
itself proves these gaps — the recipes "exist" but their input chains are
abstracted single steps that hide enormous real infrastructure:

1. **Electronics / semiconductors.** `recipes.csv` has
   `manufacture_solar_cell` (glass+wire+aluminum → solar_cell) and
   `manufacture_cpu` (gold+copper+glass → cpu, "etch logic gates into
   silicon wafer die"). In reality a solar cell or CPU needs a **fab**:
   silicon purification to 9-nines, doping, photolithography, cleanrooms
   — none of which one person or one homestead can do. The game
   *abstracts a global supply chain into one bench recipe.* When your
   inverter, battery BMS, or well-pump controller dies, you **cannot make
   the replacement chip** — you trade for it. This is the clearest "why
   civilization" lesson.

2. **Metal & alloy production from raw ore.** `recipes.csv` smelting
   (`smelt_iron`, `smelt_steel`, `smelt_aluminum`, `smelt_titanium`) needs
   `iron_ore + coal` and a `smelter_0`. A homestead `smelter`/`forge` (in
   home.ron) can do *small* jobs, but the ore itself requires **mining at
   scale** (electrical.ron `mining_drill` = 75 kW — 18× the whole home's
   daily budget), and real aluminum/titanium need electrolytic reduction
   (megawatt smelters). One person can forge a nail from scrap; they
   cannot mine and refine a tonne of steel for a new cistern.
   `waste_management.ron` recycling (`metal_shredder` at 15 kW,
   `e_waste_processor` at 3 kW) lets you *recycle* existing metal, but you
   can't bootstrap virgin metal solo.

3. **Medicine synthesis.** `recipes.csv` `craft_antibiotics` ("Culture and
   extract antimicrobial compounds," water+flour+sugar → antibiotics) and
   `craft_medkit_full` exist, and `medical.ron` treats broken bones,
   infections, poisoning. But real antibiotic, insulin, vaccine, or
   analgesic synthesis needs pharmaceutical-grade fermentation,
   purification, and QC — a hospital/industry, not a `chemistry_set_0`.
   The homestead can grow the **apothecary tower** (chamomile, echinacea,
   calendula, aloe — folk remedies, explicitly "not medical advice" in
   the data) and stitch a wound, but **cannot make the medicines that
   prevent early death.** This is why trade + medical infrastructure is
   non-negotiable.

4. **Equipment replacement & the maintenance long-tail**
   (self-sufficiency.md §7: "a system that fails on an unobtainable part
   is not self-sufficient"). Solar panels have `lifetime_hours: 219000`
   (~25 yr), batteries degrade, pumps wear out, the `air_recycler` blower
   fails. The homestead can **fabricate/repair** simple parts (forge, 3D
   printer, `workbench`), but the panel's PV cells, the battery's lithium
   cells, the pump's motor windings, and every circuit board come from
   **outside**. A closed loop on *consumables* (food, water, air,
   nutrients) does **not** mean a closed loop on *capital equipment.*

5. **Raw chemistry inputs the loops quietly assume.** Coal for every
   smelt recipe; hydroponic nutrient salts (aeroponic_configs.ron BOM
   lists "mineral salts" as `buy`); pH up/down; refrigerant/coolant
   (`freezer` needs `coolant:2`); plastics/PVC for tower columns. Even a
   "closed" nutrient loop needs **mineral inputs** (phosphorus especially
   — a globally-mined, non-renewable resource).

**Framing (the lesson, not a failure):** A single human, given a perfect
off-grid homestead, can close **energy, water, air, nutrients, and
~94–100% of food** — the *survival* loops. What they **cannot** close is
**manufacturing, metallurgy at scale, medicine, and equipment renewal** —
the loops that require *many people specializing and trading.* That gap
**is** civilization. Establishing this honest baseline for **1** is
exactly what lets us then scale to "infinite": each new person the
community adds is what unlocks a slice of the closed infrastructure (a
shared fab, a clinic, a mine) that no individual homestead can carry alone.

---

## 9. BUILD ORDER / PHASING (actionable backlog)

**Phase A — Assemble the solo home from existing data (NO new authoring; ~1 file).**
1. Create `data/machines/home_solo.ron` (or add a `variant` to home.ron)
   using the BOMs above: 4 solar / 2 battery / 1 wind / 1 generator; 1
   cistern / 1 pump / 1 purifier / 1 household node; 1 air_recycler; 1–2
   composters; 9 nutrition towers + 1 apothecary + 8 potato beds + 3
   oilseed + 2 grain trays + 2 mushroom racks + 1 aquaponic + 1
   grain_field + 1 legume_field + 1 silo + 1 irrigation. Copy the
   `connections` topology from home.ron (it's already correct), just
   fewer instances. Use the `arrays` grid syntax for the tower/bed blocks
   (infinite-of-X, one line each).
2. Use the existing `blueprints/homestead_layout.ron` room shells;
   comment out `depot`/`hangar` for the solo footprint (they're already
   commented-out-capable like `ranch`).
3. Place furniture/walls from `structures.csv` per §6 — all existing ids,
   drop into the construction editor.
   *Deliverable: a walkable 1-person home whose Home-page loop summary
   balances. This is 90% assembly of parts that already exist.*

**Phase B — Author the flagged content gaps (§7), in priority order:**
4. `plants.csv`: add `oyster_mushroom` + 1–2 more edible fungi (gap #1) —
   unblocks the mushroom_rack honestly.
5. `creatures.csv`: add `tilapia` / `channel_catfish` (gap #2) — unblocks
   the aquaponic B12/omega-3 claim.
6. `plants.csv` calorie columns **or** `data/food/crop_nutrition.ron`
   (gap #3) — lets the food loop compute from crops, not hand-typed
   catalog strings.
7. `data/self_sufficiency/component_outputs.ron` + `location.ron` +
   household-size selector (gap #4) — turns the design into a
   **computed** per-loop score with autonomy days, exactly as
   self-sufficiency.md §"Proposed model" specifies.

**Phase C — The honest teaching artifacts:**
8. Wire the **grow-light draw vs power-budget meter** (gap #5, the doc's
   stated "single most honest teaching artifact") — turns red when LEDs
   exceed free-pump headroom, proving the canopy cap.
9. Add a "**what this cannot close**" panel to the Home page surfacing
   §8: mark electronics/metal/medicine/equipment as **externally-sourced**
   (traded), visually distinct from the closed survival loops — this is
   the operator's core pedagogical goal made visible in-app.

---

### Key file paths for the developer who builds this next
- Layout target to author: `data/machines/home_solo.ron` (mirror `home.ron`)
- Room shells (reuse as-is): `data/blueprints/homestead_layout.ron`
- Tower crop configs (reuse as-is): `data/towers/aeroponic_configs.ron`
- Grow media (reuse as-is): `data/garden/grow_media.ron`
- Gap authoring targets: `data/plants.csv` (mushrooms + calorie cols),
  `data/creatures.csv` (tank fish), new `data/food/crop_nutrition.ron`,
  new `data/self_sufficiency/component_outputs.ron`
- Component specs to size against: `data/electrical.ron`, `data/hvac.ron`,
  `data/waste_management.ron`, `data/structures.csv`, `data/recipes.csv`
- Design rationale (read first): `docs/design/self-sufficiency.md`

**Bottom line:** ~90% of a fully-fledged single-occupant homestead
**already exists as data** and needs only to be assembled into a
`home_solo.ron` (Phase A). The genuine gaps are small and specific — an
edible mushroom, a tank fish, a per-crop calorie bridge, and the editable
component-output/location tables the design doc already scoped. The
"cannot close" section is not a shortfall to fix but **the deliverable's
whole point**: it is exactly the map of why civilization-scale mining,
manufacturing, medicine, and trade exist.
