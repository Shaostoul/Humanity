# Quest Engine Architecture (Plan-to-Quest Compiler)

## Goal
Convert player plans into structured quest graphs that support both:
- infinitely recurring loops (variable cadence), and
- finite single-pass objectives.

This system should teach real-world systems thinking while remaining game-readable.

---

## Quest Classes

## 1) Recurring (Cyclic)
Repeating tasks with variable timescale.

Examples:
- crop watering/monitoring,
- reactor inspection,
- route patrol,
- weekly logistics balancing,
- recurring training drills.

Fields:
- cadence (`daily`, `weekly`, custom interval)
- tolerance window
- streak/consistency metrics

## 2) Finite (Single-pass)
Tasks with clear completion.

Examples:
- build car,
- grow tree to milestone maturity,
- clear derelict ship,
- complete emergency repair trial.

Fields:
- start state
- completion criteria
- success/failure outcomes

---

## Plan-to-Quest Compiler

Player states a goal -> compiler generates dependency graph.

### Example: Build a Car
1. Gather required raw materials
2. Refine into usable metal forms
3. Fabricate components
4. Assemble systems
5. Validate/test
6. Register/deploy

### Example: Grow a Tree
1. Select species and target environment
2. Prepare substrate and nutrients
3. Seed/plant establishment
4. Monitor water/light/air/pressure/gravity constraints
5. Milestone growth checks
6. Mature state completion

Compiler outputs:
- task graph (nodes/edges)
- required inputs/resources
- role/skill requirements
- expected duration ranges
- risk/contingency tasks

---

## Quest Types (Domain Taxonomy)

- Production
- Stewardship
- Expedition
- Certification
- Emergency Response
- Civic Infrastructure

Each quest can include one or more domain tags.

---

## Rank & Certification Model (Proof-Based)

Ranks should be earned through verified performance, not only XP.

Examples:
- Master Welder:
  - emergency hull repair under pressure/time constraints
  - weld integrity threshold
  - no critical breach recurrence window

- Security Seal Specialist:
  - sustained defensive weld reliability against hostile breach attempts

Certification requires:
- objective metrics
- mission replay/proof logs
- pass thresholds

---

## Data Model (Draft)

```json
{
  "questId": "q_build_car_001",
  "type": "finite",
  "domain": ["production", "logistics"],
  "goal": "Build functional utility car",
  "nodes": [
    { "id": "n1", "title": "Gather iron ore", "status": "open" },
    { "id": "n2", "title": "Refine steel", "status": "open" },
    { "id": "n3", "title": "Assemble chassis", "status": "open" }
  ],
  "edges": [
    { "from": "n1", "to": "n2", "type": "depends_on" },
    { "from": "n2", "to": "n3", "type": "depends_on" }
  ],
  "proof": {
    "required": true,
    "metrics": ["integrity", "completionTime"],
    "thresholds": { "integrity": 0.92 }
  }
}
```

---

## UI Integration

## Header Positioning
- H becomes **Dashboard** (overview/triage)
- Quests available via Private dropdown

## Logbook Integration
Quest engine writes to Logbook views:
- Journal
- Quest Log
- Field Notes
- Proofs

---

## Time Horizon Views

- Today (daily)
- This Week
- Milestones (weeks/months)
- Legacy (months/years)

Long quests (olive tree, animal care) live naturally in Legacy with periodic recurring subloops.

---

## Failure & Recovery

- failed steps branch to recovery tasks
- outage events can auto-insert dependency blockers
- cooperative missions can unlock blocked chains

---

## Immediate Next Steps

1. Implement minimal quest schema in app data model.
2. Add plan intake form (goal -> generated graph).
3. Add Logbook tab shell with quest tracks.
4. Add one certification quest prototype (e.g., welding trial).
