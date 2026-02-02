# tests/replays/farming_week_01.md

## Purpose

Defines a minimal deterministic replay scenario for the Farming system.

This is a specification artifact used for regression testing:
- same initial data + same action log + same seed
- must produce identical milestone results

---

## Replay Metadata

- `replay_id`: `farming_week_01`
- `seed`: `12345`
- `start_tick`: `0`
- `duration_ticks`: `7`

---

## Initial Data References

- Plot: `plot_001`
- Substrate: `substrate_001` (type `loam_basic`)
- Plant definition: `potato_basic` (example)
- Water resource: `water_clean`
- Nutrient resource: `fertilizer_basic`
- Actor: `human_001`

---

## Action Schedule

Tick 0:
- prepare_plot(plot_001)
- plant_seed(plot_001, potato_basic, count=3)
- water(plot_001, water_clean, liters=2.0)

Tick 1:
- inspect(plot_001)

Tick 2:
- water(plot_001, water_clean, liters=1.0)

Tick 3:
- fertilize(plot_001, fertilizer_basic, amount_value=50, amount_units=g)

Tick 4:
- inspect(plot_001)

Tick 5:
- water(plot_001, water_clean, liters=1.0)

Tick 6:
- inspect(plot_001)

---

## Required Milestones (Assertions)

At Tick 1:
- substrate.water_l is within [0, water_capacity_l]
- at least one plant stage is not `dead`

At Tick 3:
- nitrate_mg increased relative to Tick 0 (unless capped by schema/model)
- no contamination flags introduced

At Tick 6:
- all plants have deterministic states (hash stable)
- stage_progress values remain within [0.0, 1.0]
- health remains within [0.0, 1.0]

Optional (if your plant definition permits rapid growth):
- at least one plant stage has advanced beyond `seed`

---

## Failure Conditions

Replay fails if:
- determinism breaks (state hash mismatch)
- any invariant is violated
- any quantity becomes negative
- any explanation references missing rules or missing data
