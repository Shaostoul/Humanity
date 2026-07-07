# Gameplay loop map

> Written 2026-07-07 from a verified whole-codebase survey (7-agent map, every
> claim checked against lib.rs register calls + tests/engine_wiring_lint.rs).
> This is the CURRENT-STATE vs DESIGNED-STATE picture of every gameplay loop,
> with an ordered closure ladder at the end. Companion docs:
> [progression-skills-gear.md](progression-skills-gear.md) (skills, abilities,
> gear), [engagement-modes.md](engagement-modes.md) (one sim depth, five
> engagement modes), [decision-briefs.md](decision-briefs.md) (open taste
> calls), docs/FEATURES.md (feature inventory).
>
> **The thesis (from the vision):** the hierarchy of needs IS the progression
> system. Each tier below must be survivable before the next one matters, and
> each tier FEEDS the next. One engineering-grade simulation depth; players
> choose HOW they engage per domain (direct / trade / automation / cooperative
> / background), never a dumbed-down sim.

## The tier stack

```
T0 BODY        vitals: eat, drink, breathe, warmth, rest, waste        LIVE
T1 HABITAT     power -> water -> air -> food -> compost -> crops       LIVE
T2 PRODUCTION  mine -> refine -> craft -> automate -> store            LIVE to storage
T3 EXCHANGE    currency, market, trade, NPC vendors                    coded server-side, not connected
T4 COMMUNITY   guilds, governance, laws, civilization stats            surfaces live, sims disconnected
T5 EXPANSION   vehicles -> ships -> planets -> colonies                first rungs only
X1 RISK        exposure (live), combat/disease/fire/disaster (dormant)
X2 PROGRESSION skills (live), quests (partial), gear/abilities (data authored, engine absent)
```

X1 and X2 cut across every tier. The game today is a genuinely playable
T0-T2 with stakes from X1's environmental half; T3 is the single most
valuable closure (see ladder).

---

## T0: BODY (live)

**What runs:** FoodSystem (src/systems/food.rs, lib.rs:5191) decays
satiation/hydration/energy/oxygen/body-temp/waste on the Vitals component;
eating maps items to the 12 nutrition profiles in data/food_system.ron
(kcal, raw-consumption risk, spoilage rolls); Rest restores energy; Compost
turns waste into fertilizer_0. Status effects (data/status_effects.csv, 67
rows) apply and expire; the speed stat modifier is consumed live
(well_nourished +10 percent, hypothermia half speed). Exposure is real:
outside the hull is vacuum + weather cold (EnvironmentContext,
lib.rs:6797-6841); a home power loss makes indoor air unbreathable.

**Sharpest gaps (all verified):**
- Health hits 0 and NOTHING happens. No death, no respawn, no penalty.
- No health regeneration exists in any live path (rest/food do not heal).
- Effects are speed-only: damage_per_tick / healing_per_tick / every other
  stat in status_effects.csv are parsed and consumed by nothing, so
  food_poisoning has zero consequence.
- Vitals are visible only on the Inventory page; no in-world HUD warning.

**Designed closure:** the death-and-recovery loop. Death inserts Dead,
fades to the RESPAWNER room (the fibonacci design has always had one, F2),
respawn with a light penalty (drop nothing pre-launch; tune later), and a
one-line death cause ("You froze outside the hull"). A tiny EffectTick pass
consumes damage/healing per tick from the CSV, which simultaneously gives
poison teeth and creates regeneration as the first healing. MedicalSystem
(dormant, data/medical.ron: 62 injuries/treatments) is the T0 deepening
AFTER that, not before.

---

## T1: HABITAT (live; the game's proudest loop)

**What runs:** Solar (lib.rs:5027) + Electrical (5028, data/electrical.ron)
simulate supply/demand/battery; Plumbing pumps water on power; Atmosphere
(5035) runs the sealed home's O2/CO2 with powered scrubbers; Farming (5169)
grows 132 plant species (data/plants.csv) in towers/beds/trays/fields with
irrigation gated on live water; harvest routes overflow to typed containers
(grain silo); compost closes waste back to fertilizer. RF from wireless
devices harms crops (the wired-vs-WiFi tradeoff is playable). Cut power and
you watch air, water, and crops degrade in order. This chain is the
educational core and it works.

**Sharpest gaps:**
- Plant environment windows are decorative: pH/temp/humidity/seasons are
  parsed, DISPLAYED, and ignored by the growth tick. Seasons exist
  (TimeSystem) and do nothing to crops.
- Weather is global and consequence-free (no rain-to-irrigation coupling,
  no visuals) and its tables are hardcoded while data/biomes.ron (16 rich
  biomes with precipitation/temp data) is completely orphaned.
- Cooking is generic crafting: required_station parsed but never enforced;
  cooking_methods/meal_quality/preservation_methods (28 authored entries)
  consumed by nothing; no refrigerator slows spoilage.
- Harvest-item coverage: only ~19 produce items for 132 plants; herbs,
  legumes, and mushrooms harvest to a warning log. Fix is a harvest_item
  column on plants.csv (already sketched in code comments).
- No away-time growth (crops freeze when the app closes).
- Self-sufficiency scoring (src/systems/self_sufficiency.rs +
  data/food/crop_nutrition.ron) is built and tested but only tests consume
  it; the Home page still shows hand-typed kcal strings.

**Designed closure:** make the environment matter. Season + temperature
multiply growth (one canonical pipeline; delete EcologySystem's discarded
seasonal multiplier), weather feeds outdoor field irrigation, stations gate
cooking, refrigeration slows spoilage, and the self-sufficiency score goes
live on the Home page so the player has a single number to push toward 100
percent. Each of these is a small, independent increment.

---

## T2: PRODUCTION (live to storage; sinks missing)

**What runs:** drone mining against finite asteroids (manifest, standing
orders), smelting/crafting over 362 recipes with skill gates + XP,
AutoRefine machines running continuously against backpack + home storage,
typed containers with wrong-class damage, volume-gated inventory, vehicle
kits that assemble into real driveable vehicles (the starter Nova). The
faucet-to-storage half is genuinely automated end to end.

**Sharpest gaps:**
- The material SINK is missing: ConstructionSystem is registered with 12
  blueprints loaded and queue_build() has ZERO callers. Smelters feed
  storage that feeds nothing.
- Asteroids are three hardcoded rocks in lib.rs (5241-5275), an
  acknowledged infinite-of-X violation; mining pacing constants are code.
- Station gating designed but absent (smelt iron by hand in a field).
- ManufacturingSystem duplicates AutoRefine conceptually (registered,
  nothing spawns its facilities, 626-line manufacturing.ron parsed as
  opaque values). Fold its quality/waste ideas into AutoRefine, do not
  finish it separately.

**Designed closure:** construction as the sink. A Build verb in the world
(pick blueprint, ghost preview, queue_build consumes materials, timed
build, Structure appears with its `provides` capability) plus a
data/asteroids registry with discovery/respawn. That turns
mine-refine-craft into mine-refine-craft-BUILD, which is the loop the
homestead fantasy needs.

---

## T3: EXCHANGE (the highest-value closure in the game)

**What exists, verified:** the relay ships COMPLETE escrow trading
(create/respond/confirm, order book, partial fills, history) and a COMPLETE
marketplace (listings, images, reviews, seller ratings) with REST + WS
routes. data/economy.ron (currency config, 240 lines) and
data/trade_goods.ron (255 priced goods with NPC buy/sell formulas) are
fully authored. data/npcs.ron has 32 NPCs including merchants.

**And none of it is connected:** EconomySystem is unregistered with a TODO
where the wallet credit goes; nothing loads economy.ron or trade_goods.ron;
the native Market and Trade pages are local-state mocks that never issue a
network call; there is no in-game credit balance anywhere; relay trade
items are opaque strings with no items.csv identity.

**Designed closure (ordered):**
1. Credits exist: load economy.ron, register EconomySystem, wallet field on
   the player (persisted in WorldSave), starting credits by the authored
   age formula.
2. First NPC vendor: one merchant NPC in the home world selling/buying at
   trade_goods.ron prices (the faucet/sink that makes credits mean
   something). Vendor UI = the existing market card widgets.
3. Native Market page talks to /api/listings (the web app already does).
4. Native Trade page speaks the WS trade_request escrow flow, with an item
   identity schema (items.csv id + qty) replacing free-text.
5. Prices flow from ONE source: trade_goods.ron base values, recipes derive
   crafted-good value from inputs.

Engagement modes note: T3 is what makes Trade mode (mode 2) real for every
other domain. A player who hates gardening buys food with mining credits.
That is why this closure outranks everything else.

---

## T4: COMMUNITY (surfaces live, sims disconnected)

**What exists:** LIVE native governance page with Dilithium-signed
proposals/votes via the relay object store; LIVE laws browser
(data/laws/laws.json, 2680 lines); guild CRUD on the relay with a native
Guilds page that is a local mock; relay civilization stats storage with a
native dashboard showing static numbers; a dormant GovernanceSystem and an
unloaded data/governance/proposal_types.ron (quorum/threshold rules).

**Designed closure:** wire the mocks to their real endpoints (Guilds,
Civilization) the same way as T3.3; load proposal_types.ron so vote rules
are data; then the long-game piece: guild-scoped production pools, which is
engagement mode 4 (Cooperative) made real. Community is deliberately AFTER
exchange: an economy of one player must work before an economy of guilds.

---

## T5: EXPANSION (first rungs)

**What exists:** driveable vehicles (arcade physics, no collision), summon
transit, the relay-side Pioneer frigate the crew NPCs live on, Earth as a
skybox icosphere, a tested-but-disconnected voxel asteroid terrain, dormant
docking/transportation/ship-systems scaffolds pointing at data files that
do not exist.

**Designed direction (not near-term):** vehicles get physics + fuel + cargo
(they already share the container system's shape); the bay/zones brief
(decision-briefs.md Brief 1) is the staging ground; ships become the T5
vehicle class with interiors (the ship layout + BFS pathfinding code
already exists); planets go from skybox to landable via the existing
PlanetRegistry + heightmap pipeline that is written but unwired. Every T5
increment should reuse a T1-T2 system in a new shell (a ship is a mobile
homestead; life support is the SAME AtmosphereSystem).

---

## X1: RISK (environmental half live, agent half dormant)

**Live:** exposure, suffocation, hypothermia, starvation. **Dormant but
complete:** CombatSystem (damage types, armor mitigation, loot tables,
death), FireSystem (spread/suppression), EcologySystem (disease contagion),
DisasterSystem (21 types with chain reactions), MedicalSystem (62
conditions/treatments), AISystem (5 behavior modes, deals no damage).
**Authored with zero loaders:** creatures.csv (92 creatures with health,
hostility tiers, loot tables, AI strings), factions.ron (25).

**Designed closure ladder (each rung playable):** death loop first (T0),
then the EffectTick pass, then register CombatSystem + creatures.csv loader
+ one passive harvestable animal (Harvestable component exists unconsumed:
milk/wool/eggs - farming value before fighting value), then hostile
creatures with the AISystem attack arm queueing DamageEvents, then player
weapons (see progression doc for gear schema), then fire/disease/disasters
as homestead threats that the T1 systems defend against (suppressors,
medicine, shelters). Combat is a RISK layer serving the survival fantasy,
not a separate genre bolted on.

## X2: PROGRESSION

Covered in full in [progression-skills-gear.md](progression-skills-gear.md).
Summary of current state: 20 skills live with learn-by-doing XP (10 have no
trainer); quests tick but 3 of 5 objective types have no event emitters, no
accept UI, and no persistence; equipment slots are cosmetic page-state;
abilities do not exist in the engine while data/spells.csv (110) and
data/enchantments.csv (107) sit fully authored; the relay holds two MORE
progression models (hardcoded quest/XP JSON and skill_dna's
reality_xp/fantasy_xp split). The design unifies these into one canonical
model whose Real/Sim XP split the relay schema already anticipated.

---

## The closure ladder (strict order, each rung independently shippable)

Rationale for the order: stakes before content, sinks before faucets,
one-player economy before community, reuse before new systems.

1. **DEATH AND RECOVERY** (T0/X1). Dead at 0 HP, respawner room, cause line,
   EffectTick pass (poison + regeneration live). Small; touches food.rs +
   one new tick + hud. Test: walk into vacuum, die, respawn at F2.
2. **CONSTRUCTION ENTRY POINT** (T2). Build verb -> queue_build ->
   Structure with provides; build_ quest events start firing (unblocks the
   authored construction quest chain). Test: build the furnace blueprint,
   construction quest advances.
3. **CREDITS + FIRST VENDOR** (T3.1-3.2). economy.ron + trade_goods.ron
   loaders, EconomySystem registered, wallet on Vitals-bearer, one merchant
   NPC trading at authored prices. Test: sell 10 wheat grain, buy a tool.
4. **QUEST REPAIR** (X2). Persistence, accept/browse UI, Travel/Talk
   emitters, XP rewards field. Unlocks the 9 authored-but-unreachable
   quests as actual content.
5. **NATIVE MARKET/TRADE WIRING** (T3.3-3.4). The two mock pages speak to
   the relay for real; item identity schema for escrow.
6. **ENVIRONMENT MATTERS** (T1). Season/temp growth pipeline, station-gated
   cooking, refrigeration, harvest_item column (all 132 plants harvestable),
   self-sufficiency score live on the Home page.
7. **CREATURES, PASSIVE FIRST** (X1). creatures.csv loader + spawner,
   Harvestable animals, then hostiles + CombatSystem registration.
8. **GEAR AND ABILITIES** (X2). Per the progression doc: Equipped component,
   armor/stat pipeline through net_stat_multiplier, ability loader over
   spells.csv.
9. **ZONES + VEHICLE BAY** (T5, gated on Brief 1 approval).
10. **COMMUNITY WIRING** (T4). Guilds/civilization pages to real endpoints;
    proposal_types.ron loaded.

Rungs 1-4 are each roughly one focused session for a competent implementer
against this map. Nothing in this ladder invents a new system: every rung
activates or connects something that already exists, which is the cheapest
kind of gameplay there is.

## Standing cleanups this survey exposed (do opportunistically)

- Delete or repurpose: data/skills/default_profile.json (dead path, wrong
  ids), data/psychology.ron (its system was deleted; keep the file only if
  a morale sim is genuinely planned), economy/fleet.rs's reference to
  nonexistent resources.csv, CombatSystem's private status-effect fork
  (use the CSV-backed StatusEffects component instead).
- Two building models (blueprint Structures vs HomeStructure editor) need a
  reconciliation note in the construction design doc when rung 2 lands;
  likewise VehicleSystem's dormant enter/exit arms vs the live lib.rs
  driving path (fold or wire, do not keep both).
- HydrologySystem has a registration-blocking bug: it reads
  Weather where the store holds Mutex<Weather> (hydrology.rs:323). Fix
  before ever registering it.
- The relay quest/XP JSON track and skill_dna are progression forks; the
  progression doc's unification section owns this.
