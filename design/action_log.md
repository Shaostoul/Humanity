# design/action_log.md

## Purpose

Defines the canonical action log format used for deterministic replays.

Actions are immutable inputs to simulation.
They are validated before application.

The action log exists to guarantee:
- determinism
- auditability
- explainability

---

## ActionLog

- `seed` (string or integer, required)
- `start_tick` (integer, required, >= 0)
- `actions` (array of ActionEntry, required)

ActionEntry:
- `tick` (integer, required, >= start_tick)
- `actor_id` (string, required)  
  Human or authorized agent performing the action.
- `action` (enum, required)
- `targets` (object, required)  
  References relevant ids.
- `params` (object, optional)  
  Numeric/string parameters with units where applicable.

---

## Farming Actions (initial set)

### prepare_plot
targets:
- `plot_id`
params (optional):
- `cover_state` (enum)
- `amendment_ids` (array)

### plant_seed
targets:
- `plot_id`
params:
- `species_id`
- `count` (integer >= 1)

### transplant
targets:
- `plant_id`
- `to_plot_id`

### water
targets:
- `plot_id`
params:
- `water_id`
- `liters` (number > 0)

### fertilize
targets:
- `plot_id`
params:
- `nutrient_id`
- `amount_units` (enum: `mg`,`g`,`kg`)
- `amount_value` (number > 0)

### manage_cover
targets:
- `plot_id`
params:
- `cover_state` (enum)

### prune
targets:
- `plant_id`
params (optional):
- `intensity` (number 0.0–1.0)

### inspect
targets:
- `plot_id`

### treat_pests_or_disease
targets:
- `plant_id`
params:
- `treatment_id`

### harvest
targets:
- `plant_id`
params (optional):
- `harvest_intent` (enum: `ideal`,`early`,`late`)

### remove_crop
targets:
- `plant_id`

---

## Validation Rules

- Unknown actions are invalid.
- All referenced ids must exist at the time of action application.
- Quantities must be finite and within declared bounds.
- Actions must not bypass system constraints.

---

## Notes

Time/labor and energy costs are declared by system specs and computed during application, not authored into the action log unless explicitly modeled.
