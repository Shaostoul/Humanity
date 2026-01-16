# data_model.md — Project Universe Canonical Data Model

This document defines the **schemas, invariants, units, and validation rules** for Project Universe data. Data is the canonical description of reality. Code must interpret data; it must not redefine it.

Design intent:

* Deterministic simulation
* AI-readable truth
* Mod-safe extensibility
* Educational traceability (why success/failure occurs)

---

## 1) Data principles

### 1.1 Canonical identifiers

* Every definable thing has a stable ID: `Namespace:Category:Name`

  * Example: `core:flora:potato`
* IDs are case-insensitive for lookups, but stored normalized (lowercase, snake_case).

### 1.2 Separation of concerns

* **definitions/**: what a thing *is*
* **recipes/**: how things *transform*
* **practices/**: how actions are correctly performed (with failure cases)
* **equations/**: formal relationships and models
* **constraints/**: global non-negotiables
* **failure_cases/**: explicit causal error models

### 1.3 Deterministic loading

* Data load order is deterministic.
* All references are resolved at load time.
* Missing references are hard errors.

### 1.4 Units are mandatory

* Every numeric field has units.
* Units are validated.
* Conversions are centralized and explicit.

---

## 2) Directory layout

```
data/
├─ README.md
├─ definitions/
│  ├─ flora/
│  ├─ fauna/
│  ├─ materials/
│  ├─ tools/
│  ├─ machines/
│  ├─ structures/
│  ├─ vehicles/
│  ├─ ship_modules/
│  ├─ chemicals/
│  ├─ nutrients/
│  ├─ diseases/
│  └─ skills/
├─ recipes/
│  ├─ crafting/
│  ├─ cooking/
│  ├─ manufacturing/
│  └─ construction/
├─ practices/
│  ├─ agriculture/
│  ├─ animal_husbandry/
│  ├─ food_preservation/
│  ├─ water/
│  ├─ energy/
│  └─ maintenance/
├─ equations/
│  ├─ growth/
│  ├─ nutrition/
│  ├─ energy_balance/
│  ├─ heat_transfer/
│  └─ economics/
├─ constraints/
│  ├─ realism_constraints.ron
│  └─ unit_system.ron
├─ failure_cases/
│  ├─ agriculture/
│  ├─ health/
│  ├─ machines/
│  └─ storage/
├─ glossaries/
│  ├─ terms.ron
│  └─ labels.en.ron
└─ localization/
   ├─ en/
   └─ ...
```

---

## 3) File formats

### 3.1 Preferred formats

* **RON** (`.ron`) for structured data (Rust-native, readable).
* **Markdown** (`.md`) for narrative explanations and teaching notes.

### 3.2 Prohibited formats for canonical truth

* Binary-only formats as canonical rule sources.
* Spreadsheets as canonical truth.

### 3.3 Naming conventions

* Files: `snake_case.ron`
* IDs inside files: `namespace:category:name`
* One primary definition per file.

---

## 4) Core schema building blocks

### 4.1 Common header (all definitions)

Every definition includes:

* `id: DefId`
* `version: SemVer`
* `name_key: LocKey` (localization key)
* `description_key: LocKey`
* `tags: [Tag]`
* `sources: [SourceRef]` (where this knowledge comes from)
* `invariants: [Invariant]` (validation rules)

#### SourceRef

* `title`
* `publisher`
* `year`
* `url` (optional)
* `notes` (optional)

Purpose: educational audit trail and conflict resolution.

---

## 5) Unit system

### 5.1 Canonical units

* Length: `m`
* Area: `m2`
* Volume: `m3`, `L`
* Mass: `kg`, `g`
* Time: `s`, `min`, `h`, `day`
* Temperature: `C`, `K`
* Energy: `J`, `kWh`
* Power: `W`
* Pressure: `Pa`
* Flow: `L_per_min`, `m3_per_s`
* Nutrition: `kcal`, `g_protein`, `g_fat`, `g_carbs`, `mg_micronutrient`

### 5.2 Quantity type

All numeric fields use:

* `Qty { value: f64, unit: UnitId }`

Rules:

* No bare floats in definitions.
* Unit conversions occur only in the unit library.

---

## 6) Definition schemas (canonical)

### 6.1 FloraDef (plants)

Required fields:

* `taxonomy` (optional but preferred)
* `growth_model: GrowthModelRef`
* `soil_requirements: SoilReq`
* `water_requirements: WaterReq`
* `light_requirements: LightReq`
* `temperature_range: TempRange`
* `seasonality: SeasonProfile`
* `labor_profile: LaborProfile`
* `yields: [YieldDef]`
* `pest_susceptibility: [RiskRef]`
* `disease_susceptibility: [RiskRef]`
* `failure_modes: [FailureCaseRef]`

Purpose: a plant is defined by constraints and failure, not only by reward.

### 6.2 FaunaDef (animals/humans)

Required fields:

* `needs: NeedsProfile` (water, calories, shelter)
* `diet: DietProfile`
* `health_model: HealthModelRef`
* `reproduction` (optional)
* `labor_capability` (for humans and working animals)
* `products: [YieldDef]` (milk, eggs, wool)
* `risk_profile` (stress, disease)
* `failure_modes: [FailureCaseRef]`

### 6.3 MaterialDef

Required fields:

* `state` (solid/liquid/gas)
* `density`
* `strength` (when applicable)
* `thermal_properties`
* `chemical_properties` (when applicable)
* `toxicity` (when applicable)
* `failure_modes` (corrosion, spoilage, contamination)

### 6.4 ToolDef

Required fields:

* `function_tags` (cutting, digging, fastening)
* `efficiency_modifiers` (bounded)
* `durability_model`
* `maintenance_requirements`
* `failure_modes`

Rule: tools modify labor and feasibility; they do not create impossible outcomes.

### 6.5 StructureDef

Required fields:

* `footprint_area`
* `volume`
* `capacity` (storage, occupancy)
* `thermal_envelope` (if relevant)
* `load_limits` (if relevant)
* `maintenance`
* `failure_modes`

### 6.6 MachineDef

Required fields:

* `inputs` (materials/energy)
* `outputs`
* `efficiency`
* `heat_waste`
* `maintenance`
* `safety_risks`
* `failure_modes`

### 6.7 SkillDef

Required fields:

* `domain_tags`
* `learning_curve_model`
* `prerequisites`
* `assessment_methods`

Rule: skills do not grant magic bonuses; they reduce error and waste within constraints.

---

## 7) Recipes (transformations)

A recipe is a deterministic transformation:

* `id`
* `inputs: [ItemStack]`
* `tools_required: [ToolTag]`
* `stations_required: [StationTag]` (optional)
* `time_required: Qty(time)`
* `energy_required: Qty(energy)` (optional)
* `outputs: [ItemStack]`
* `byproducts: [ItemStack]` (optional)
* `skill_requirements: [SkillReq]`
* `failure_behavior: FailureBehaviorRef`

Rule: recipes must declare time and labor/energy costs.

---

## 8) Practices (how to do things correctly)

A practice defines the **correct method** and why.

Fields:

* `id`
* `scope_tags`
* `steps: [PracticeStep]`
* `constraints: [ConstraintRef]`
* `required_tools`
* `required_conditions` (soil moisture range, temperature)
* `common_mistakes: [MistakeDef]`
* `failure_modes: [FailureCaseRef]`
* `success_metrics: [MetricDef]`

Purpose: practices power education, mentor dialogue, and procedural quest generation.

---

## 9) Failure cases (causality)

A failure case is a structured explanation of what goes wrong.

Fields:

* `id`
* `trigger_conditions` (expressed as predicates)
* `symptoms` (state changes)
* `diagnosis_steps`
* `consequences` (yield reduction, illness, breakdown)
* `mitigations`
* `preventions`

Rule: Every major system must have explicit failure cases.

---

## 10) Constraints (global non-negotiables)

Constraints define hard limits (physics, biology, labor).

Fields:

* `id`
* `predicate`
* `severity` (error/warn)
* `message_key`

Examples:

* Calories in must cover calories out.
* Storage temperature above threshold increases spoilage rate.
* Labor exceeds human capacity produces fatigue and error.

---

## 11) Localization model

### 11.1 Keys

* `name_key` and `description_key` are required.
* Human-visible strings are never embedded in definitions.

### 11.2 Labels

* `data/glossaries/labels.en.ron` provides short UI labels.
* Full educational text lives in `education/` markdown.

---

## 12) Asset references (representation pointers)

Definitions may reference assets via stable paths, but assets are never authoritative.

### 12.1 Canonical asset pointers

* `model_glb: AssetPath` (optional)
* `icon_png: AssetPath` (optional)
* `textures: [TextureRef]` (optional)
* `audio_cues: [AudioRef]` (optional)

Rules:

* Missing assets are warnings (unless required by UI).
* Assets may not carry mechanics metadata.

---

## 13) Validation rules (must fail build)

The build must fail if:

* A definition is missing units on numeric fields.
* A referenced ID does not resolve.
* A practice is missing failure modes.
* A recipe is missing time and required tools.
* A mod overrides a protected constraint.
* Localization keys are missing for any shipped language.

---

## 14) Mod extension rules

Mods may add:

* new definitions
* new recipes
* new practices
* new failure cases

Mods may not:

* remove or weaken global constraints
* change unit system
* redefine existing canonical IDs without explicit namespacing

---

## 15) Minimal examples (conceptual)

### Flora example: potato

* Definition includes: soil pH, days to maturity, temperature range, water needs, yield ranges, common diseases, failure cases.

### Practice example: seed planting

* Steps include: soil preparation, depth, spacing, watering schedule, and measurable success metrics.

These examples must be implemented in actual `.ron` files under `data/`.
