# design/schemas/items/harvest_output.schema.md

## Purpose

Defines the canonical data contract for harvest outputs produced by farming.

Outputs must carry explicit safety and quality fields for downstream systems.

---

## Schema: HarvestOutput

### Identity
- `output_id` (string, required, stable identifier)
- `source_plant_id` (string, required, must resolve)
- `species_id` (string, required, must resolve to PlantDefinition)
- `common_name` (string, required)

### Quantity
- `quantity_value` (number, required, >= 0)
- `quantity_units` (string, required)  
  Example: `kg`, `count`

### Quality (bounded, explicit)
- `quality_grade` (enum, required)  
  Allowed: `poor`, `fair`, `good`, `excellent`
- `moisture_index` (number, required, 0.0–1.0)
- `defect_rate` (number, required, 0.0–1.0)

Optional:
- `sugar_starch_index` (number, optional, 0.0–1.0)

### Safety
- `contamination_flags` (array of string, optional)
- `edibility_state` (enum, required)  
  Allowed: `safe`, `restricted_use`, `unsafe`

### Provenance
- `harvest_tick` (integer, required, >= 0)
- `notes` (string, optional)

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- Quantities must be finite and non-negative.
- All indices must be within 0.0–1.0.
- References must resolve.
