# Project Universe — Game Design & Repository Structure

This document defines the canonical structure of the Project Universe codebase. It is simultaneously:

* A technical map for developers
* A learning map for players
* A knowledge graph for AI systems
* A truth-preserving educational artifact

Every folder corresponds to a real-world domain. Nothing exists without justification.

---

## ROOT — single source of truth

```
root/
├─ Cargo.toml
├─ README.md
├─ LICENSE
├─ CHANGELOG.md
├─ game_design.md
├─ philosophy/
├─ engine/
├─ world/
├─ life/
├─ society/
├─ industry/
├─ science/
├─ technology/
├─ mathematics/
├─ education/
├─ narrative/
├─ ui/
├─ multiplayer/
├─ modding/
├─ data/
├─ assets/
├─ tools/
└─ tests/
```

The root is intentionally sparse. All complexity is pushed downward into explicit domains.

---

## philosophy/ — intent before mechanics

```
philosophy/
├─ purpose.md            # Why the game exists
├─ ethics.md             # Non-negotiable moral constraints
├─ poverty_eradication.md# Systems approach to ending poverty
├─ peaceful_unification.md# Conflict avoidance as design law
├─ realism_vs_playability.md # Explicit tradeoffs
└─ design_laws.md        # Axioms governing all systems
```

This folder prevents design drift. Nothing downstream may violate these files.

---

## engine/ — universal rules, no content

```
engine/
├─ mod.rs                # Engine root
├─ time/                 # Tick rate, calendars, aging
├─ physics/              # Motion, forces, collisions
├─ energy/               # Conservation, transfer, storage
├─ simulation/           # Deterministic world updates
├─ persistence/          # Save/load, serialization
├─ input/                # Player and system inputs
└─ rendering/            # Low-level render abstraction
```

The engine knows nothing about farming, people, or stories. Only rules.

---

## world/ — spatial context

```
world/
├─ space/
│  ├─ ships/             # Procedural and handcrafted ships
│  ├─ stations/          # Orbital and deep-space habitats
│  └─ navigation/        # Orbits, routes, fuel use
├─ planets/
│  ├─ terrain/           # Heightmaps, geology
│  ├─ biomes/            # Environmental zones
│  └─ climate/           # Weather and long-term patterns
├─ scale/
│  ├─ personal.rs        # 0.1 acre homestead scale
│  ├─ settlement.rs     # Multi-household scale
│  ├─ city.rs            # Dense planning scale
│  └─ civilization.rs   # Interstellar abstraction
└─ procedural/           # Generators with known constraints
```

Spaceships precede planets to avoid computational and conceptual overload.

---

## life/ — biology as system

```
life/
├─ flora/
│  ├─ crops/             # Edible plants
│  │  └─ potato.ron      # Growth, nutrition, labor cost
│  ├─ trees/             # Long-cycle resources
│  └─ fungi/             # Decomposition and food
├─ fauna/
│  ├─ animals/           # Non-human life
│  └─ humans/
│     ├─ anatomy.ron     # Body systems
│     ├─ nutrition.ron   # Dietary requirements
│     ├─ labor.ron       # Physical work capacity
│     └─ learning.ron    # Skill acquisition
├─ ecology/              # Interactions between life
└─ health/               # Injury, disease, recovery
```

Farming emerges naturally from life constraints instead of being hard-coded.

---

## society/ — cooperation and structure

```
society/
├─ households/           # Family and cohabitation units
├─ education/            # Knowledge transfer between people
├─ governance/           # Decision-making systems
├─ economics/
│  ├─ barter.ron         # Direct exchange
│  ├─ currency.ron       # Abstract value systems
│  └─ scarcity.ron       # Resource limitation modeling
├─ law/                  # Rules and enforcement
└─ culture/              # Norms and traditions
```

Poverty is modeled as a systemic failure, not a number.

---

## industry/ — transforming matter

```
industry/
├─ gathering/
│  ├─ mining/            # Raw materials extraction
│  ├─ logging/           # Biomass harvesting
│  └─ harvesting/        # Crop collection
├─ crafting/             # Hand-scale fabrication
├─ construction/         # Buildings and ships
├─ manufacturing/        # Repetitive, automated processes
└─ maintenance/          # Repair and degradation
```

Clear separation between hand labor and industrial systems.

---

## science/ — discovery

```
science/
├─ physics/
├─ chemistry/
├─ biology/
├─ astronomy/
├─ materials/
└─ experimentation/
```

Science produces knowledge, not items.

---

## technology/ — applied knowledge

```
technology/
├─ agriculture/
├─ energy/
├─ transportation/
├─ computing/
├─ medicine/
└─ spaceflight/
```

Technology consumes science and industry inputs.

---

## mathematics/ — formal foundations

```
mathematics/
├─ arithmetic/
├─ geometry/
├─ algebra/
├─ calculus/
├─ statistics/
└─ optimization/
```

Mathematics powers simulation, AI reasoning, and explicit teaching.

---

## education/ — learning as gameplay

```
education/
├─ concepts/             # Atomic ideas
├─ lessons/              # Structured instruction
├─ challenges/           # Applied problem-solving
├─ apprenticeships/      # Long-term skill gain
└─ assessments/          # Knowledge verification
```

No mechanic exists without a learning pathway.

---

## narrative/ — dialogue with purpose

```
narrative/
├─ characters/           # Knowledge-bearing NPCs
├─ dialogues/            # Structured conversations
├─ events/               # System-driven storytelling
└─ history/              # Recorded world state
```

Narrative explains systems instead of distracting from them.

---

## ui/ — clarity over spectacle

```
ui/
├─ hud/
├─ menus/
├─ tooltips/
└─ accessibility/
```

UI is an educational surface, not decoration.

---

## multiplayer/ — shared reality

```
multiplayer/
├─ synchronization/
├─ authority/
└─ cooperation/
```

Multiplayer preserves determinism and fairness.

---

## modding/ — constrained openness

```
modding/
├─ api/                  # Stable extension points
├─ examples/             # Reference mods
└─ validation/           # Reality compliance checks
```

Mods may extend reality but not contradict it.

---

## data/ — machine-readable truth

```
data/
├─ definitions/          # Canonical entities
├─ recipes/              # Transformations
├─ equations/            # Formal relationships
├─ dependencies/         # Knowledge graphs
└─ glossaries/           # Human and AI reference
```

This folder is the AI integration anchor.

---

## assets/ — sensory representation only

```
assets/
├─ models/
│  └─ glb/               # 3D models (GLB)
├─ textures/             # Albedo, normal, roughness maps
├─ icons/                # UI symbols
├─ audio/
│  ├─ music/
│  ├─ ambience/
│  └─ effects/
├─ animations/           # Skeletal and procedural
└─ shaders/              # Visual interpretation rules
```

Assets contain no logic. They visualize existing systems.

---

## tools/ — real-world utility

```
tools/
├─ planners/             # Layout and resource planning
├─ calculators/          # Energy, food, space
├─ blueprints/           # Build guides
└─ export_real_world/    # Printable and usable outputs
```

These tools are intended for use outside the game.

---

## tests/ — verification

```
tests/
├─ simulation_tests/     # Physical correctness
├─ balance_tests/        # System stability
└─ education_tests/      # Teaching accuracy
```

If knowledge is wrong, the build fails.

---

## External Knowledge Alignment — real-world homesteading canon

Project Universe explicitly aligns its systems with accumulated real-world self‑sufficiency knowledge (e.g., multigenerational homesteading, subsistence agriculture, off‑grid living). These domains are not treated as flavor or inspiration, but as **authoritative reality inputs** that shape mechanics, data schemas, and education pathways.

To support this, the following structural refinements apply:

---

### life/ (expanded intent)

* flora/ and fauna/ definitions must encode:

  * soil requirements
  * seasonal constraints
  * labor intensity
  * failure modes (disease, neglect, climate mismatch)
* ecology/ explicitly models crop rotation, biodiversity, pest pressure, and nutrient cycles as taught in real homesteading practice.

This ensures farming and animal care behave as they do in reality, not as abstract resource generators.

---

### industry/ (expanded granularity)

```
industry/
├─ food_processing/      # Preservation, fermentation, storage
├─ water_systems/        # Wells, rain capture, filtration
├─ energy_systems/       # Wood, solar, wind, human labor
```

These systems reflect real self‑sufficiency priorities: food longevity, water security, and energy independence.

---

### education/ (real-skill mapping)

Education content is structured to mirror real homesteading skill acquisition:

* concepts/ — atomic truths (soil pH, animal feed ratios)
* lessons/ — step-by-step practices (planting, butchering, preserving)
* challenges/ — constraint-based scenarios (limited land, poor soil, harsh climate)
* apprenticeships/ — long-duration mastery paths reflecting real experience curves

Knowledge is validated through outcomes, not completion.

---

### data/ (authoritative knowledge encoding)

```
data/
├─ practices/            # Canonical real-world methods
├─ failure_cases/        # What happens when methods are ignored
├─ constraints/          # Physical and biological limits
```

This allows AI systems and gameplay logic to reason about *why* things work or fail.

---

### narrative/ (expert voices)

Narrative characters function as domain experts:

* Farmers
* Builders
* Mechanics
* Preservation specialists

Dialogue conveys tested practices, tradeoffs, and lived consequences rather than fiction.

---

### tools/ (real-world parity)

Tools must be usable both in-game and externally:

* planting calendars
* food storage calculators
* land-use planners
* energy budgeting tools

Outputs are intentionally compatible with real homesteading decision-making.

---

These integrations ensure Project Universe reflects **universal, field-tested human knowledge**, not invented abstractions. The game remains computationally efficient, pedagogically sound, and grounded in reality.

---

This structure encodes reality, teaches it, and allows it to scale from a single human to a united civilization without conceptual collapse.
