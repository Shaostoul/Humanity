# Ship Zoning & Transit Architecture (Project Universe)

## Purpose
Define a natural, layered private/public megaship layout that:
- feels human and restorative (park/wilderness-forward),
- scales to massive population and logistics,
- teaches real-world systems thinking,
- remains resilient during failures.

This spec complements `design/feature_web.md` and should be represented in the Feature Web / Starseed Atlas.

---

## Core Principles

1. **Private-first daily life, public-first civilization**
   - Player homes are practical and farm-like.
   - Public areas are nature-rich civic parklands.

2. **Visible from space**
   - Public park biomes and civic landmarks (e.g., mall) must be orbit-readable.

3. **Transit resilience by design**
   - Fast primary routes + always-available fallback routes.

4. **Functional layering**
   - Separate life, civic, and industrial traffic to reduce conflict and cognitive load.

5. **Teachability**
   - Every space should map to real-world analogs and learning outcomes.

---

## Layered Spatial Model

## Layer 1 — Private Habitat Ring
**Role:** player home, personal growth, preparation.

### Typical private modules
- Bedroom
- Battlestation
- Network
- Garden
- Workshop
- Garage
- Armory
- Bathroom/Healthcare
- Living room

### Design traits
- Calm local movement
- High personalization
- Utility-first with natural textures/forms

---

## Layer 2 — Community Greenbelt (Public Park Layer)
**Role:** morale, social commons, nature restoration, passive education.

### Public-space concepts
- Nature trails
- Waterfront walks
- Commons clearings
- Community orchards/agroparks
- Event lawns

### Design traits
- Strong natural biomes
- Long-form walkability
- Leisure + mental health loops
- Major visual signature visible from space

---

## Layer 3 — Civic/Commercial Spine
**Role:** high-traffic shared services and economic/social interaction.

### Public module examples
- Mall
- Public market
- Training/academy hubs
- Medical centers
- Fleet service kiosks
- Community interaction nodes

### Design traits
- Landmark architecture embedded in greenbelt clearings
- High accessibility
- Social and economic density

---

## Layer 4 — Industrial/Logistics Layer
**Role:** throughput, construction, repair, transport, fleet operations.

### Functional systems
- Hangars
- Cargo bays
- Fabrication complexes
- Resource processing
- Dispatch control

### Design traits
- Mostly segregated from leisure spaces
- Freight-first paths
- Safety and maintainability prioritized

---

## Transit Architecture

## Vertical Transit
### 1) Elevators (Primary, fast)
- Main vertical connector between private and public/civic layers.
- High throughput.

### 2) Ramps/Stairs (Fallback, resilient)
- Always available backup if elevators fail.
- Lower throughput, higher traversal time.
- Also supports exercise and exploration loops.

## Horizontal Transit
### 3) Monorails (Primary inter-public)
- Rapid movement between major public hubs.
- Best for long-distance non-freight movement.

### 4) Trails/Walkways (Experience layer)
- Scenic routes through parks and waterfronts.
- Supports morale and social encounters.

### 5) Service rail/cargo lanes (industrial)
- Freight and operations movement isolated from passenger routes.

---

## Transit Rule Set (Resilience)

- Every critical destination must have:
  - **Primary route** (fast), and
  - **Fallback route** (slower but guaranteed).

- Public safety requirement:
  - No single elevator/monorail failure can isolate a district.

- Routing clarity:
  - Vertical = district/layer switching
  - Horizontal = hub-to-hub travel

---

## Public/Private Naming Convention

Use **simple home labels** as primary titles and **public analogs** as subtext.

Examples:
- Bedroom -> Public: Crew Quarters District
- Battlestation -> Public: Operations Center
- Network -> Public: Fleet Communications Grid
- Garden -> Public: Agropark / Community Agriculture Decks
- Workshop -> Public: Fabrication Complex
- Garage -> Public: Transit & Vehicle Bays
- Armory -> Public: Fleet Armory
- Bathroom/Healthcare -> Public: Medical & Bio Support Wing
- Living room -> Public: Community Commons

---

## Visibility from Space Requirements

1. Public park layer must be geometrically legible from orbit.
2. Major civic locations (e.g., mall) must be distinct visual landmarks.
3. Transit lines should be readable as network structure at macro scale.
4. Lighting language should communicate district type (residential/public/industrial).

---

## Gameplay & Education Outcomes

- Encourages noncombat-first loops while retaining strategic depth.
- Reinforces real-world urban planning, logistics, and redundancy concepts.
- Supports both VR immersion and mobile management parity.
- Preserves mental-health-oriented environmental design through nature-forward public zones.

---

## Utility Systems & Cooperative Restoration

Utilities are first-class gameplay systems and visible to players in both private and public spaces.

### Utility classes
- Power
- Water/Waste
- Network/Comms
- Industrial throughput (refinement/production)

### Visibility model
- Player homes expose local usage/readouts (personal consumption and status).
- Public spaces show district utility health and dependency state.
- Utility state impacts accessibility and performance of dependent systems.

### Event model
- Utility failures can trigger fleet-wide cooperative restoration events.
- Examples:
  - Main power cascade -> selected gameplay loops disabled/degraded.
  - Industrial zone damage -> lower refinement rates, production bottlenecks.
  - Network degradation -> reduced multiplayer/market/coordination capabilities.

### Restoration flow
1. Diagnose fault chain.
2. Route players to required modules/roles.
3. Gather or craft replacement components.
4. Restore in dependency order.
5. Verify system stabilization and reopen gated services.

### Design intent
- Teach infrastructure interdependence in an intuitive, participatory way.
- Convert outages into meaningful cooperative gameplay rather than passive downtime.

---

## Implementation Notes (Near-term)

1. Represent these layers/modules as domains in Feature Web.
2. Add transport edge types:
   - vertical_primary
   - vertical_fallback
   - horizontal_public
   - horizontal_service
3. Add resilience checks in planning views:
   - isolation risk,
   - single-point failure detection,
   - route redundancy score.
4. Build a 2D zoning prototype before 3D spatialization.

---

## Open Questions

1. Final megaship form factor (ring/halo hybrid/other).
2. Exact population tier targets per ship variant.
3. Public:private space ratio by development phase.
4. Monorail/elevator energy budgets and failure simulation design.
5. How much of industrial layer is player-visible vs abstracted.
