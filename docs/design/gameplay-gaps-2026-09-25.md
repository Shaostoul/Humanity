# Gardening, crafting, inventory and storage: what exists, what is missing

**Surveyed 2026-09-25** by four read-only passes over the code and data (after
the operator asked: "What could we do more gameplay focused? ... What else are
we missing to make gardening, crafting, and inventory more complete? Does the
stuff we have store anywhere physically?"). File references are as of that
date; re-check before trusting one. The container data basis is
[`docs/reference/findings/2026-09-25-container-materials-and-reuse.md`](../reference/findings/2026-09-25-container-materials-and-reuse.md).

## Progress

- **Defects 1-7: FIXED 2026-09-26** (v0.1344.0). Items return to their
  container when the backpack is full; a craft with no room is refused before
  spending anything and a finished one waits for room; crafting XP counts
  vessel-kept outputs; the showcase garden shows in the Garden panel and the
  sliders and field climate reach it; a new player starts with a kit from
  `data/world/player.ron`; every recipe input has a source, every plant has
  planting stock, the vendor sells real seeds, and
  `tests/recipe_sources_lint.rs` keeps it so; edibility, nutrition and
  spoilage come from `data/food/item_profiles.ron` (34 new profiles), not
  item-id prefixes.
- **Water is real: FIRST RUNG DONE 2026-09-26** (v0.1344.0). The Irrigation
  machine (`irrigates: true`, an `Irrigator` marker) waters grow areas only
  while powered, and draws each growing plant's `water_liters_per_day` from
  its plumbing island through the plumbing sim (real minutes, the same time
  base as the home's other machines). Rain waters outdoor fields. Hand
  watering takes 2 L from the fullest tank and is refused with a notice when
  the tanks are empty. The family home's water budget: production 2.40
  L/min, household 1.35, leaving about 1.05 for a showcase garden that needs
  up to 1.0 when everything is growing, so the cistern is the buffer. Still
  to do on water: filling a jug from a tank and pouring it back (part of the
  containers arc, where fluids become litres).
- **Containers remember and have materials: 3a + 3b DONE 2026-09-26**
  (v0.1345.0). See [containers.md](containers.md): memory of the last content,
  a Clean action that uses water, the toxic-history rule, a materials table
  and reactivity rules.
- **Stored goods are physical: DONE 2026-09-26** (v0.1346.0). The Barn's
  solid placeholder blocks are now open pallet racks (`ZoneFiller` mesh_kind
  "rack": posts and four decks, clamped under the 3 m ceiling), and what is
  filed in the Barn shows as crates on their decks, lowest deck first, one
  96 L crate per 96 L of stock (`src/engine/stock_piles.rs`). The default
  home's Barn starts with a homestead dry store (grain, flour, seed
  potatoes, fertiliser, lumber: 1,620 L, 17 crates) in
  `data/places/seed.json`. Every water tank carries a level gauge. Named
  bags, the garage and the car trunk are elsewhere and are not drawn in the
  Barn. Not yet: the harvest overflow goes into machine vessels, not the
  Barn, so harvesting does not yet grow the crate count; crates of one
  generic look rather than sacks, barrels and crates by content class.
- **Fluids are litres, first rung: DONE 2026-09-26** (v0.1347.0). Recipes
  that need tap water draw it from the tanks; bottles and jerrycans are
  filled at a tank and poured back; drinking a bottle returns it empty. See
  [containers.md](containers.md).
- **Containers as items (3c): BLOCKED** on the unified placement schema
  (Tier B in PRIORITIES.md). The walk-up machine card and every vessel are
  tied to `data/machines/home.ron` placements; a container that can be
  picked up and set down needs placement to be one system first.
- Found while merging, still open: the Eat/Drink buttons still show on water
  MACHINES (the food system now ignores those clicks); `animal_fat_0` has
  base material `plant_fiber` and there is no tallow material; items.csv
  files 11 medical supplies, trees, flowers and alien plants under category
  "food"; `cook_coffee` makes an energy drink; soap has no lye; one hide
  tans into two leathers; eating one of anything counts as 100 g.

## Defects found (things that are wrong, not merely missing)

1. **Items vanish when the backpack is full.** "Take to backpack" removes the
   placed item before adding it, and ignores the overflow
   (`src/gui/pages/inventory.rs` ~1796; `InventoryOp::Add` return value
   ignored at `src/systems/inventory/mod.rs` ~764). Manual craft outputs that
   do not fit are also discarded with only a log line
   (`src/systems/crafting/mod.rs` ~518-524).
2. **The showcase garden is invisible to the Garden panel and the sliders.**
   `auto_seed_showcase` tags crops with machine INSTANCE ids (`ntower_0`,
   `grain_field_1`), the Garden panel and the water/nutrient sliders key on
   type or tower-config ids, and the field climate check keys on a `_field`
   suffix the instance ids do not have (`src/engine/ipc.rs` ~222,
   `src/gui/pages/inventory.rs` ~212-247 and ~1894-1913,
   `src/systems/farming/mod.rs` ~1132). Its `grain_tray` and `potato_bed`
   keys match no machine type, so those surfaces are never seeded
   (`data/world/showcase.ron`).
3. **Twelve recipe inputs have no source at all**, blocking 12 recipes, mostly
   name mismatches: `painkiller_0`/`antibiotic_0` vs the made
   `painkillers_0`/`antibiotics_0`, `charcoal_lump_0` vs `coal_0`,
   `fiberglass` not an item, `animal_fat_0` only from animals that never spawn,
   `salt_block_0` unsourced, and cotton/hemp/jute/apple/banana/coffee with no
   seed item. Three recipe ids are duplicated (`craft_compass`,
   `craft_binoculars`, `refine_fuel`).
4. **The first seed only comes from a dev button.** Harvest returns two seeds,
   but `seed_<id>_0` exists for 61 of 189 plants, the vendor's `seed_bag_0` is
   not an item, and `starting_items` in `data/player.toml` is never read (the
   player spawns empty).
5. **Cooked foods feed nobody.** Cake, pie, omelette, stew, sandwich, juice
   and nine more have no nutrition profile, so eating them does nothing
   (`src/systems/food.rs` ~191-238), while `grain_mill_0` and `grain_silo_0`
   count as food because of their `grain_` prefix.
6. **Crafting XP is skipped** when a machine's own vessel takes every output
   (early return, `src/systems/crafting/mod.rs` ~1281).
7. Six of the Crafting page's eleven sidebar categories match no recipe
   (`data/crafting/categories.json`), so 225 recipes appear only under "All".

## Missing, by area

**Water.** Irrigation takes no water from the tanks: farming only reads
whether the cistern is above 2%. The Irrigation machine draws from plumbing
but farming never reads it, so switching it off changes nothing. Rain waters
no crop. Each plant's `water_liters_per_day` is display-only. Tank litres and
water items (jugs) do not convert in either direction.

**Gardening.** One nutrient slider instead of N-P-K (the N-P-K, pH and
humidity columns in `plants.csv` are display-only; `farming/soil.rs` is an
unused scaffold). No pests, disease, weeds, pollination or rotation. Grow
lights and day length do nothing. Yield ignores the crop's health. Legumes and
fiber never spoil; nothing consumes `grain_wheat_0`. **Plant models: 12 of 189
species** have 3D growth-stage models (tomato, carrot, wheat, lettuce, corn,
rice, pumpkin, beet, apple, orange, watermelon, bamboo); six more model sets
(mushroom, berry bush, flowers, cactus, palm, grass) match no plant id; towers
always draw the procedural stand-in.

**Crafting.** No tools-required column, tool wear, quality tiers,
byproducts, failure, recipe discovery or station power draw
(`quality_levels` in `manufacturing.ron` is parsed and unused). 256 of 308
outputs are used by nothing. In the one-person home, 12 of 17 station types
are unreachable (160 recipes); the construction editor can add any machine
for free in any play mode.

**Containers.** The design is half there: 17 container types and 12 content
classes with a `food_safe` flag, and a written rule that a food container
"must not have previously held a non-food-safe class"
(`data/containers/content_classes.ron`). None of that rule is code: a
container forgets its contents the moment it is emptied, `base_material` and
`food_safe` are never read, there is no lining, no cleaning, no residue. The
17 types are not items (the items called crate, barrel, fuel drum and water
tank are plain solids with no capacity). Liquids are counted as whole jugs in
slots, not litres in a tank.

**Physical storage.** Nothing stored is drawn in the world: no crates, piles,
tank levels or cargo that fills up. The organize layer is a flat list of
labels with a path string and no capacity; only a walk-up card shows
"X / Y L". The construction editor already has `cargo` and `storage` zone
types whose filler tiles boxes across the floor at a FIXED amount
(`data/blueprints/zone_filler.ron`, `home_structure::generate_zone_filler`):
that seam is where a Starmade-style "the room fills as the stock grows" would
go.

## Proposed order (not yet decided by the operator)

1. **Defects 1-7.** Small, and each one is a place the game lies to the
   player or loses their things.
2. **Water is real.** Irrigation draws litres from the cistern at each plant's
   `water_liters_per_day`, only while the Irrigation machine is powered; rain
   waters field crops; fill a jug from a tank and pour it back.
3. **Containers are real.** The 17 types become items with capacity; each
   container carries its material and lining and a content history. A
   petroleum, solvent, pesticide or cleaner history bars food and drinking
   water permanently; food-to-food switches need a cleaning action (for
   dairy: rinse, hot alkaline wash, acid rinse, sanitise); linings and
   plastics remember previous contents for longer than stainless steel
   (Codex: 1, 2 or 3 fills). Reactive metals refuse acidic or salty foods.
   All of it from the findings table, as data. Fluids become litres in a
   tank.
4. **You can see your stuff.** Storage and cargo zones fill with crates,
   sacks and barrels in proportion to what is stored; tanks show their level.
5. **Crafting depth**: tools required and worn, byproducts, station power,
   quality. **Gardening depth**: N-P-K, pests, light, yield from health, the
   six unmapped plant models, then more species.
