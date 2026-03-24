# Gardening Through Gameplay

## Purpose

Define how Humanity teaches gardening through gameplay as applied competence: producing food and healthy plants reliably across conditions, with minimal waste and predictable results.

Gardening education is treated as an iterative practice: observe → plan → act → measure → adjust.

---

## Core Definitions

**Plant**  
A living system with needs, tolerances, and growth phases.

**Soil**  
A dynamic medium (structure, biology, chemistry, water, air) that governs root function.

**Microclimate**  
Local conditions that differ from regional weather (shade, wind, walls, elevation, moisture).

**Fertility**  
The soil’s capacity to supply nutrients in usable forms over time.

**Constraint**  
A limiting factor preventing healthy growth (light, water, nitrogen, temperature, pests, compaction, etc.).

**Yield**  
Harvested output measured by mass, quality, timing, and reliability.

---

## Design Constraints

1. **No magical agriculture.** Growth follows modeled biology and resource constraints.
2. **Failure is instructional.** Problems must be diagnosable with clear causal traces and retriable.
3. **Inputs have tradeoffs.** Fertilizers, pesticides, irrigation, and soil amendments carry costs, risks, and externalities.
4. **Systems must be legible.** Players can inspect soil, plant status, pests, moisture, nutrient balance, and microclimate.
5. **Transfer is required.** Skills must generalize to new crops, soils, seasons, and climates.
6. **Sustainability is measurable.** Soil health can improve or degrade based on practices.

---

## What “Learning Gardening” Means In-Game

Represented by:
- consistent germination and establishment
- reduced plant stress and disease incidence
- higher yield per area and per unit input
- improved soil structure and biology over time
- reliable scheduling (planting windows, succession, harvest timing)
- resilience under weather volatility and pest pressure

Not represented by:
- single-click planting/harvest automation as mastery
- “rare seed” items as primary skill gating
- progression that ignores ecology, seasons, or resource realities

---

## Canonical Gameplay Loop

### Site → Plan → Plant → Maintain → Diagnose → Harvest → Improve

1. **Site**
   - assess light map, wind exposure, drainage, slope, frost pockets, shade patterns
   - identify water access and soil type constraints

2. **Plan**
   - choose crops by season, climate, and goals
   - design bed layout, spacing, rotations, companions, access paths
   - plan irrigation and mulch strategy
   - decide fertility approach (compost, amendments, fertigation)

3. **Plant**
   - seed starting vs direct sow
   - planting depth, timing, spacing, hardening off, transplant shock controls

4. **Maintain**
   - watering schedule tuned to soil + weather
   - mulching, weeding, staking/trellising, pruning
   - integrated pest management (IPM) actions

5. **Diagnose**
   - interpret symptoms (chlorosis, wilting, leaf curl, spots, stunting)
   - distinguish causes (nutrient deficiency vs pH lockout vs overwatering vs disease)

6. **Harvest**
   - harvest timing affects quality and continued production
   - post-harvest handling and storage

7. **Improve**
   - update soil organic matter, structure, biology
   - refine rotations and bed design
   - perform postmortems on failures and document fixes

---

## Teaching Mechanics

### 1) Light and Microclimate Mapping
Players build a light map:
- hours of direct sun by season
- shade movement
- heat retention near structures
- wind corridors

Crop suitability is enforced mechanically:
- fruiting crops demand higher sun thresholds than leafy greens.
- bolting risk increases with heat stress and day length.

### 2) Soil as a System (Structure + Biology + Chemistry)
Soil has visible states:
- texture class and compaction
- infiltration rate and water-holding capacity
- organic matter
- microbial activity indicator
- pH and nutrient availability (with measurement error and tool resolution)

Actions affect long-term soil trajectory:
- excessive tillage reduces structure and biology
- compost improves fertility but may introduce salts or imbalance if misused
- mulch moderates moisture and temperature but can harbor pests if unmanaged

### 3) Water Management With Real Constraints
Watering is not “fill bar.”
Systems include:
- evapotranspiration driven by heat, wind, humidity
- root depth progression over time
- overwatering causing hypoxia and disease risk
- drip vs overhead irrigation tradeoffs

### 4) Nutrients: Availability, Not Just Quantity
Nutrients exist in forms with rules:
- pH lockout reduces uptake
- nitrogen drives vegetative growth but can reduce fruiting if excessive
- calcium transport depends on transpiration patterns
- micronutrients matter at low thresholds

Players learn:
- deficiency symptom patterns
- confirming via tests or controlled trials
- adjusting with measured amendments (not random dumping)

### 5) Pest and Disease as Ecology (IPM)
Pests and diseases have:
- life cycles
- habitat conditions
- predator relationships
- spread dynamics

IPM tools:
- monitoring and thresholds (scouting cadence)
- physical barriers (row cover)
- cultural controls (spacing, airflow, sanitation)
- biological controls (predators)
- chemical controls as last resort with side effects

### 6) Scheduling, Succession, and Rotation
The game enforces:
- planting windows
- days-to-maturity variability by temperature
- succession planting for continuous harvest
- crop rotation reducing pest/disease buildup and balancing soil drawdown

### 7) Diagnostic Discipline (Gardening Epistemology)
When problems occur, the player must:
- record symptoms and context
- propose competing causes
- run a minimal test (change one variable, use a control bed/pot)
- measure results and update practices

This prevents “folk remedy spam” from working reliably.

---

## Competence Catalog

1. **Site Assessment**
   - microclimate identification
   - drainage and slope management
   - bed placement optimization

2. **Soil Management**
   - structure improvement
   - organic matter building
   - pH management
   - composting basics and amendment dosing

3. **Planting and Propagation**
   - seed starting
   - transplanting and hardening off
   - spacing and depth correctness
   - germination control variables

4. **Watering**
   - schedule design by soil and weather
   - irrigation system choice and maintenance
   - drought/heat wave response

5. **Nutrition**
   - deficiency recognition
   - controlled feeding
   - avoiding over-fertilization and salt buildup

6. **Pest and Disease**
   - scouting and thresholds
   - IPM sequencing
   - sanitation practices
   - prevention via airflow and spacing

7. **Harvest and Post-Harvest**
   - timing for quality
   - storage and preservation basics
   - seed saving prerequisites (where appropriate)

8. **System Planning**
   - rotations
   - companion logic (mechanistic, not magical)
   - season extension (mulch, low tunnels, cold frames)

---

## Systems That Must Integrate Gardening

### Construction / Crafting
- beds, trellises, irrigation, cold frames, compost bins are buildable systems
- material choices affect longevity and performance

### Ecology / Weather
- forecast drives frost protection and watering decisions
- extreme events require triage and mitigation strategies

### Economy / Logistics
- seed, compost inputs, tools, water have costs
- yield and quality feed into nutrition, trade, and planning

---

## UI Requirements

### Garden Planner
- bed layout, spacing validation, sunlight overlay
- rotation history overlay (crop families)
- predicted planting windows and maturity ranges

### Plant Card
- growth stage
- stress indicators (heat, water, nutrient)
- pest/disease status with confidence levels
- required interventions ranked by expected effect

### Soil Panel
- moisture profile (surface vs root zone)
- compaction/infiltration indicator
- organic matter trend
- pH and nutrient availability with tool resolution

### Logbook
- actions taken with timestamps
- weather notes
- scouting notes
- harvest records and quality ratings
- postmortems for failures

Legibility is mandatory: outcomes must be explainable.

---

## Validation and Scoring

### Metrics
- germination rate and establishment rate
- yield per area and per unit input (water, amendments, labor)
- soil health trend (organic matter, structure, biology proxy)
- pest/disease incidence and recovery time
- schedule reliability (meeting target harvest windows)
- waste rate (loss to spoilage, bolting, disease)

### Failure Handling
Failures must:
- show the causal chain (conditions → stress → symptom → outcome)
- surface top candidate causes with supporting evidence
- allow controlled retry without punitive lockout loops

---

## Data Outputs Required

Every gardening interaction must serialize cleanly for `.csv`, `.ron`, and `.rs`.

### Canonical Record Types
- `Crop`
- `Variety`
- `Bed`
- `SoilProfile`
- `Planting`
- `IrrigationEvent`
- `AmendmentEvent`
- `ScoutingObservation`
- `PestDiseaseEvent`
- `Intervention`
- `Harvest`
- `WeatherSlice`
- `Postmortem`

### Cross-Cutting Required Fields
- stable IDs
- timestamps (game-time; optionally wall-time)
- location/context snapshot
- inputs/outputs (amounts, units)
- uncertainty/measurement metadata (tool, resolution, calibration)
- procedure version and reproducibility seed where relevant
- links between records (bed → planting → observations → interventions → harvest)

---

## Non-Negotiables

- Gardening is taught by doing: planning, measurement, intervention, and iteration.
- Soil health must be persistent and affected by behavior.
- Reliable yield requires respecting constraints: light, water, temperature, fertility, and ecology.
- Diagnostics must reward controlled changes and evidence over ritual.