# Educational Gameplay — Design Philosophy

**Status:** Active
**Author:** Shaostoul + Claude
**Date:** 2026-03-17
**Affects:** All game modules, skill systems, quest design, scenario scripting

---

## Principle: Learning through irresistible gameplay

Every real-world skill should be learnable in-game through gameplay so engaging that players *choose* to learn because the game demands it. Education is not a side effect — it is the core design goal. But it must never feel like a textbook. The game creates situations where mastering a real skill is the only way to succeed, and success feels incredible.

---

## How it works

### Skills in exciting contexts

Real-world skills are practiced inside high-stakes, high-excitement scenarios. The context makes the learning memorable and the repetition enjoyable.

**Examples:**

- **Welding** — Learn welding by reinforcing a bunker against alien hordes with friends. A bad weld means the wall fails and the team dies. A strong weld holds the line and earns the squad's trust.
- **Electronics / PCB repair** — Fix captured alien weapon tech during an expedition. Correct PCB diagnosis unlocks alien weapons for the squad. Misidentify a component and the device overloads.
- **Farming / Gardening** — Grow food to sustain a colony on a hostile planet. Real soil science, companion planting, and seasonal cycles determine whether the settlement starves or thrives.
- **Construction** — Build shelter before a storm hits. Structural integrity follows real engineering principles — bad framing collapses under wind load.
- **Medical / First aid** — Stabilize injured teammates during a firefight. Correct triage and wound treatment determine who makes it back alive.
- **Cooking** — Prepare meals that grant buffs to the expedition team. Real recipes, real nutrition science. Bad food = debuffs or illness.
- **Piloting / Navigation** — Navigate a ship through an asteroid field using real celestial navigation principles. Plot a course using star charts, not a magic GPS arrow.
- **Electrical wiring** — Wire up a base's power grid. Real circuit principles: overload a circuit and you lose power to critical systems during a crisis.

### Cooperative skill application

Multiple players can collaborate on the same task. This mirrors real-world teamwork and creates natural teaching moments.

- **Multiple welders on one joint** — One player tacks, another runs the bead, a third inspects. Communication and coordination matter.
- **Surgical teams** — One player operates, another monitors vitals, a third manages anesthesia. Each role requires different knowledge.
- **Construction crews** — Framing, wiring, plumbing, and finishing happen in parallel. Sequencing matters — you cannot drywall before the wiring is inspected.
- **Farm teams** — One player manages soil, another handles irrigation, a third focuses on pest control. Specialization and cooperation produce better results than any solo effort.

### Skill proficiency affects real outcomes

Player skill proficiency is not just a number on a progress bar — it directly affects gameplay outcomes in ways that matter.

| Proficiency | Outcome |
|-------------|---------|
| Novice | High failure rate, slow execution, visible mistakes. The game coaches you through it. |
| Competent | Reliable results, normal speed. You can handle routine tasks independently. |
| Expert | Fast execution, bonus quality, ability to improvise under pressure. Unlock advanced techniques. |
| Master | Teach other players, handle edge cases, create custom solutions. Recognized by the community. |

A bad weld means the team dies. A good PCB fix unlocks alien weapons. A perfect surgical intervention saves a teammate who would otherwise be out for the rest of the mission. Proficiency is not cosmetic — it is survival.

---

## Both realism and fantasy serve education

### Realistic simulation

The underlying models are grounded in real-world physics, chemistry, biology, and engineering. When a player learns to weld in-game, the technique, metallurgy, and safety principles map to real welding. The simulation is accurate enough that skills transfer to the physical world.

### Fantasy scenarios

Fantasy and sci-fi contexts provide the motivation and excitement that make repetitive practice enjoyable. Nobody wants to practice welding beads on a flat plate for 100 hours. Everyone wants to weld a barricade shut while aliens pound on the other side and their friends are counting on them.

The fantasy does not compromise the realism of the skill — it wraps it in a context that makes the player *want* to get better.

---

## Replayability through procedural and scripted experiences

### Procedural generation

- Terrain, weather, resource distribution, and enemy behavior are procedurally generated.
- No two expeditions play out the same way, even with the same team and skills.
- Procedural variation forces players to adapt their knowledge rather than memorize solutions.

### Scripted experiences

- Hand-crafted scenarios for teaching specific skills (tutorials, guided missions, certification challenges).
- Story-driven campaigns that weave multiple skills together into a coherent narrative.
- Community-created scenarios shared through the platform.

### Combined approach

The best experiences blend both. A scripted mission structure ("defend the outpost for 3 nights") with procedurally generated details (enemy approach vectors, weather conditions, available materials) ensures the player must apply real understanding, not rote memorization.

---

## Skill categories

A non-exhaustive list of real-world skill domains the game aims to cover:

| Category | Example skills |
|----------|---------------|
| **Fabrication** | Welding, machining, 3D printing, woodworking, metalworking |
| **Electronics** | Circuit design, PCB repair, soldering, embedded systems, power distribution |
| **Agriculture** | Soil science, crop rotation, composting, hydroponics, aeroponics, animal husbandry |
| **Construction** | Framing, roofing, concrete work, plumbing, electrical wiring, HVAC |
| **Medical** | First aid, triage, wound care, pharmacology, surgical procedures, diagnostics |
| **Culinary** | Cooking techniques, nutrition, food preservation, baking, fermentation |
| **Navigation** | Celestial navigation, map reading, orienteering, GPS/radio, dead reckoning |
| **Piloting** | Aircraft, spacecraft, watercraft — controls, physics, emergency procedures |
| **Science** | Chemistry, physics, biology, geology, astronomy, ecology |
| **Engineering** | Structural, mechanical, electrical, software, systems design |
| **Survival** | Shelter building, water purification, fire making, foraging, weather reading |
| **Communication** | Radio operation, signal processing, encryption, language, diplomacy |

Each category has its own progression system, and skills from different categories combine in emergent ways during gameplay.

---

## Related documents

- [Core education model](../core/education_model.md) — How learning is represented and validated
- [Core skill progression](../modules/core-skill-progression.md) — Skill progression system design
- [Core teaching graph](../modules/core-teaching-graph.md) — Teaching graph architecture
- [Feature web](../feature_web.md) — Interactive teaching-first feature graph
- [Gardening game](gardening-game.md) — First minigame, grounded in real botanical data
- [Module specs](../modules/README.md) — Individual skill module specifications
- [History](../history.md) — Project timeline and origins
