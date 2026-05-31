# HumanityOS — Gameplay Systems & Loops (holistic map)

> **Status:** design anchor for the gameplay arc (opened 2026-05-30 from the operator's
> vision brain-dump). This is the map of *what loops exist and how they connect*; each
> loop ships as its own increment. Most of the underlying systems already exist in code
> (see `tests/engine_wiring_lint.rs::DEFERRED_SYSTEMS` — ~40 systems implemented,
> mostly unregistered) — the work is **wiring them into loops + spawning content + the
> connective glue**, not writing them from scratch.

## Development posture: fully unlocked

For development we play **as if the player has unlocked everything 100%** — all recipes
available, materials on hand, every system active — so every loop is testable *as we
build it*, and we can balance space requirements, timings, and interactions across the
whole system at once. Progression / skill-gating / tech-unlock is a **presentation layer
added last** (it just hides/locks parts of an already-working system). A dev/creative
provisioning (stock materials + unlock recipes) is the first thing we build.

## The big picture: three layers + the connective tissue

```
   NEEDS (why)            PRODUCTION (how)                WORLD (where/with-what)
   ──────────             ────────────────                ───────────────────────
   nutrition  ◄────────── cook ◄── manufacture ◄── refine ◄── acquire
   hydration              (food)   (components/      (raw→     (mine via drone,
   health                          items/tools)      material)  farm, forage, salvage)
   oxygen                    │            │              │            │
   temperature               └── construct (base/infrastructure) ◄────┘
   energy/rest                                                   power · water · logistics
   sanitation                                                   time · weather · vehicles
```

You produce **to meet needs**; meeting needs (esp. nutrition) **buffs you**, which makes
producing/acquiring easier — the core compounding loop. Scarcity (finite asteroids),
decay/maintenance, and disasters apply pressure.

## 1. Character survival needs — *why you produce*

| Need | Model (proposed) | System (exists) |
|---|---|---|
| **Nutrition** | Baseline **Satiation** (hunger) must stay > 0 or health drains. Above baseline, a few axes (calories/energy, protein, vitamins) — **variety + surplus → buffs**, deficiency → debuffs. **Cooked** food = more satiation + better/longer buffs; **raw** = edible but less, some risky. | `food`, `farming` |
| **Hydration** | **Thirst** baseline; water from sources/processing. | `hydrology` (water bodies), `food` |
| **Health** | Injury/healing; death at 0. | `combat`, `medical` |
| **Oxygen** | Breathable air in enclosed/space; suffocation otherwise. | `atmosphere` |
| **Temperature** | Cold/heat stress; mitigated by shelter/clothing/HVAC. | `weather`, `hvac` |
| **Energy/rest** | Exertion drains; rest restores; affects work speed. | `psychology`, `aging` |
| **Sanitation** | Waste handling; neglect → disease pressure. | `waste`, `ecology` |

**Buffs from good nutrition (operator's idea):** move speed, reload/work speed, night
vision, stamina, carry weight, etc. — concrete, visible rewards for eating well. Harm
from poor nutrition: the inverse debuffs + health drain.

## 2. Production chain — *how you sustain + grow*

### Acquire (raw inputs)
- **Mining via space drone (operator's core loop):** player commissions a **drone** →
  it travels to an **asteroid** → mines requested ores (iron, nickel, platinum…) over
  **time** → returns home → **drops off raw material**. Asteroids hold **finite**
  resources; in MMO many players swarm one for the valuable ores then the scraps; when
  **fully consumed or abandoned the asteroid entity is deleted**. (Asteroid voxel system
  + ore veins already exist in `terrain/asteroid.rs`; the drone is a timed autonomous
  task + cargo return.)
- **Farm / garden:** plant → grow (driven by `game_time` + water + season) → harvest. A
  **concise base plant set covers complete nutrition forever**; players expand. (See §4.)
- **Forage / gather** (surface pickups) and **salvage / recycle** (break items back to
  materials) round out acquisition.

### Refine → Manufacture → Construct
- **Refine:** raw ore/material → refined material (e.g. iron ore → iron ingot → steel).
  Recipe chains already exist (`crafting`, `smelting`/`refining` categories).
- **Manufacture:** material → **components** (motors, gears, screws, rails…) → items /
  tools / machines. (`crafting`, `manufacturing`; components data exists.)
- **Construct:** items → structures / base / infrastructure. (`construction`/`placement`.)

## 3. Connective world systems

Drones & vehicles + **autonomous tasks** (the mining drone is the first); **logistics /
storage** (typed material-aware **containers — already built**, + inventory);
**power/electrical** (machines need power); **water/hydrology**; **time/seasons** (wired);
**weather**; **economy/trading** (player↔player, relay-mediated); **governance**.

## 4. Concise complete plant set (proposed starting point)

Goal: the smallest set of plants whose harvests **together** cover complete human
nutrition (carbs, protein, fats, key vitamins/minerals) — grounded in real survival
agronomy (fits the educational mission). Players expand from here.

| Plant | Covers | Notes |
|---|---|---|
| **Potato** | Carbohydrate/calorie staple, vitamin C, potassium | High calories/area, fast, stores well |
| **Soybean** (or beans/lentil) | Protein + fat | Complete-ish protein, nitrogen-fixing |
| **Kale/Spinach** (leafy green) | Vitamins A/K/folate, iron, calcium | Fast cycle, cut-and-come-again |
| **Tomato** | Vitamin C, antioxidants, flavor | Cookable; high yield |
| **Sunflower** (seed/oil) | Fats/oil, vitamin E | Oil pressing → cooking fat |
| **Carrot** | Vitamin A, fiber, root storage | Root crop, long storage |

(6 plants ≈ complete nutrition. Final list/balance is the operator's call — this is a
defensible default.)

## What exists vs what needs building

- **Exists (mostly unregistered, see the lint):** farming, asteroid+mining, crafting,
  food, medical, atmosphere, hydrology, weather, electrical, vehicles, construction,
  economy, logistics, AI/autonomy.
- **Needs building:** the **loops** that connect them, **content entities** (spawned
  crops/asteroids/drones), the **full-unlock dev provisioning**, the **GUI→ECS command
  bridge** (so UI buttons drive real ECS actions), and the new glue: the **drone task**,
  **nutrition effects/buffs**, and the **real crafting consume/produce**.

## Proposed build order

1. **Full-unlock dev provisioning** — stock the player with materials + unlock recipes; the GUI→ECS command bridge this needs is reused everywhere. *(testing enabler)*
2. **Real crafting loop** — Craft button → ECS consume inputs / produce outputs → inventory updates. *(first end-to-end playable loop)*
3. **Cooking + nutrition** — eat/cook → satiation/hydration + buffs/harm; cooked vs raw.
4. **Gardening loop** — plant → grow (time/water) → harvest → food. (game_time already wired.)
5. **Drone↔asteroid mining** — commission drone → timed trip → returns raw materials; asteroid depletion + deletion (server-authoritative for MMO).
6. **Refining chain depth** — raw → material → component multi-step, consumed by crafting.
7. **Survival systems online** — register + feed health/atmosphere/power/water/temperature/waste as their content exists; each moves off the lint's deferred list.
8. **Progression layer (last)** — skills/XP/quests/tech-unlock gate the now-working systems.

Each step: register/wire the relevant system(s), spawn the content, verify with an
automated test where possible + the operator playing. The engine-wiring lint tracks which
systems have come online.
