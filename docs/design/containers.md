# Containers that remember what they held

**Designed 2026-09-26** from the operator's brief of 2026-09-25 and the dated
research in
[`docs/reference/findings/2026-09-25-container-materials-and-reuse.md`](../reference/findings/2026-09-25-container-materials-and-reuse.md).

The operator, verbatim: "A single liquid container wouldn't contain fuel,
milk, water, etc. Also, a container that held oil shouldn't be used to hold
milk unless a very strict cleaning procedure took place otherwise the tank
would poison everyone that drank the milk it held. (I don't know the limits
of what containers can be cleaned and which can't. Also the interior coating
probably matters, like steel vs stainless vs copper vs etc."

## What exists (2026-09-25 survey)

17 container types (`data/containers/types.csv`), 12 content classes
(`data/containers/content_classes.ron`) with a `food_safe` flag, a whitelist
check, and damage when the wrong class is forced in. A container holds one
item at a time. Missing: any memory of what it held, any use of its material,
any cleaning, liners, or fluids measured in litres.

## The rules, and where each comes from

1. **History beats material.** A container that ever held a toxic material
   (the Food Code's definition: petroleum products, non-food lubricants,
   solvents, pesticides, cleaners, paints) may never hold food or drinking
   water again, whatever it is made of. It may still hold other non-food
   contents. Source: FDA Food Code 7-203.11 and its definition of poisonous
   or toxic materials; the Pasteurized Milk Ordinance for milk tanks.
   *Game:* a `toxic_ever` flag on the container, set by storing any content
   class marked toxic (flammable, toxic, corrosive, hazmat), never cleared.
2. **Switching contents needs cleaning.** An emptied container remembers its
   last content. Refilling it with the SAME content is fine (topping up);
   anything else is refused until it is cleaned. *Game:* `last_content` on
   the container and a Clean action that uses water from the home tanks (and,
   for food and dairy vessels, a cleaning agent when the game has one).
   Dairy cleaning as the sources describe it: lukewarm rinse, hot alkaline
   wash above 120 F, acid rinse at pH 3 to 4, then sanitise just before the
   next use.
3. **Absorbent surfaces remember longer.** Plastics, wood, rubber, organic
   (epoxy, phenolic) linings and unglazed or cracked ceramic absorb what they
   held; glass and metal do not. The Codex code for bulk edible-oil tanks
   keeps some previous cargoes banned for 1 fill in stainless steel, 2 in
   organically coated tanks, and 3 for leaded products. *Game:* each material
   carries a `memory_fills` number; after cleaning, an absorbent container
   carries a taint of its previous content for that many fills.
4. **Material fitness.** Food and drinking water need a food-grade surface.
   Reactive metals refuse certain foods: copper and brass nothing below
   pH 6, no edible oils, no milk; galvanised steel nothing acidic and no
   unlined drinking water; aluminium no acidic or salty storage; plain carbon
   steel and cast iron no pickling or long wet storage. 316 stainless takes
   brines and salty foods that 304 should not hold long. *Game:* a materials
   table and per-content traits (acidic, salty, fatty, dairy).
5. **Pressure vessels are their own class.** Gas cylinders and fuel cans
   hold their product only; the sources read do not settle repurposing them,
   so the game does not allow it.

## Data

- `data/containers/materials.csv` (new): id, name, food_grade, absorbent,
  memory_fills, refuses (pipe-separated traits: acidic, salty, fatty, dairy,
  water), note, source (a section of the findings doc).
- `types.csv`: `base_material` becomes a real material id from that table
  (the dairy tank becomes `stainless_304`, the fuel drum `carbon_steel`, the
  water tank `hdpe`, and so on).
- Content traits for foods ride on the food profiles
  (`data/food/item_profiles.ron` gives each edible item a profile; a small
  table maps profiles to traits).

## Built

- **3a (2026-09-26):** `last_content` and `toxic_from` on every container;
  an emptied container takes only more of the same until it is CLEANED
  (the machine card offers Clean, which uses 2% of its capacity in water from
  the home tanks, 1 to 50 L); a container that held fuel, a solvent, a
  corrosive or anything hazardous never holds food or drinking water again
  (`leaves_toxic_history` in content_classes.ron). The card says what the
  vessel remembers. New type: the 1000 L IBC tote, the real-world case.
- **3b (2026-09-26):** `data/containers/materials.csv` (17 materials),
  `data/containers/content_traits.ron` (food profile and class traits), real
  material ids in types.csv, and a `direct_contact` column so cabinets and
  freezers of packaged food are not judged by their liner. A direct-contact
  vessel refuses food if its material is not food-grade and refuses traits
  its material reacts with, without damage. New types: copper pot,
  galvanised bucket, fermenting crock. Not yet: the absorbent memory
  (`memory_fills` is recorded but not used) and a cleaning agent for food and
  dairy vessels.

- **Visible storage (2026-09-26):** the Barn's racks are real shelving and
  what is filed in the Barn shows as crates on their decks; each water tank
  shows its level. `src/engine/stock_piles.rs`; details in the gap doc.
- **3d, first rung (2026-09-26):** `data/containers/fluids.ron` and
  `src/systems/fluids.rs`. A recipe that needs a measure of tap water
  (`water_purified_0`, 1 L) draws it from the home tanks when the backpack
  has none, and the Crafting page counts the tanks too; a water tank's card
  fills the empty bottles and jerrycans you carry and pours full ones back;
  drinking a bottle hands the empty bottle back. Each vessel keeps to one
  fluid, so a vessel is an empty item and a full item rather than an item
  that remembers a fluid. The starter kit has two empty bottles; the vendor
  sells bottles and jerrycans. Not yet: fuel and milk as litres (their tanks
  still hold items), partial fills, a watering can for hand watering (it
  still draws straight from the tanks, like a hose).
- **3c is blocked** on the unified placement schema (a container that can be
  picked up and set down needs machine placement and item placement to be
  one system; PRIORITIES.md Tier B).

## Increments

- **3a. Memory and cleaning.** `toxic_ever` and `last_content` on the
  container, the refusal messages, and a Clean action on the machine card.
- **3b. Materials.** The materials table, real material ids in types.csv,
  food-grade and reactivity checks, absorbent memory.
- **3c. Containers are items.** The 17 types become items with capacity that
  can be placed in the organize layer and moved.
- **3d. Fluids are litres.** A tank holds litres of one liquid; empty jugs,
  buckets and cans exist as items; fill a jug from a tank and pour it back.
- **Then visible storage**: storage and cargo zones fill with crates, sacks
  and barrels as stock grows, and tanks show their level.
