# design/schemas/plots/plot.schema.md

## Purpose

Defines the canonical data contract for a cultivation Plot used by the Farming system.

A Plot is the site that holds:
- plant instances (by reference)
- a substrate instance (by reference)
- exposure/protection configuration

Plot data describes what exists and what is true.  
It does not encode behavior.

---

## Schema: PlotDefinition (optional)

Use a definition only if multiple plots share identical structure.

- `plot_type_id` (string, required)
- `common_name` (string, required)
- `plot_type` (enum, required)  
  Allowed: `ground_bed`, `raised_bed`, `container`, `hydroponic_unit`
- `default_area_m2` (number, required, > 0)
- `default_plant_slots` (integer, optional, >= 0)
- `default_density_limit` (number, optional, >= 0)

---

## Schema: PlotInstanceState

### Identity
- `plot_id` (string, required, stable identifier)
- `plot_type_id` (string, optional, must resolve if present)

### Geometry / Capacity
- `plot_type` (enum, required; same enum as PlotDefinition)
- `area_m2` (number, required, >= 0)
- `plant_slots` (integer, optional, >= 0)
- `plant_density_limit` (number, optional, >= 0)

Invariant:
- At least one of `plant_slots` or `plant_density_limit` must be present.

### Exposure and Protection
- `cover_state` (enum, required)  
  Allowed: `open`, `mulched`, `row_cover`, `greenhouse`, `shade_cloth`
- `light_access_factor` (number, required, 0.0–1.0)

### References
- `substrate_id` (string, required, must resolve to SubstrateInstanceState)
- `plant_ids` (array of string, required; may be empty; must resolve to PlantInstanceState)

### Attachments (capabilities only)
- `irrigation_attachment_id` (string, optional)
- `lighting_attachment_id` (string, optional)
- `sensor_attachment_ids` (array of string, optional)

Attachments may provide capability surfaces, never hidden mechanics.

---

## Global Invariants

- Unknown fields are forbidden unless explicitly added to the schema.
- All references must resolve.
- All enum values must match allowed sets.
- Any violation is a validation failure.
