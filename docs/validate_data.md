# tools/validate_data.md

Validation is a mandatory gate that must pass before any simulation, replay, or merge.

## Inputs
- Repository data under `data/`
- Schemas under `design/schemas/`

## Required checks (must fail hard)
1. Unknown-field rejection
2. Enum validation (exact match)
3. Numeric bounds (no negatives unless declared; 0–1 indices enforced)
4. Unit presence where required (liters, mg, m3, C, factors)
5. Reference resolution:
   - every `*_id` reference points to an existing file/object
6. Definition/instance separation:
   - definitions must not contain instance-only fields
   - instances must reference definitions
7. No mechanics in data:
   - reject fields like `bonus`, `rarity`, `multiplier`, `magic`, `free_yield`

## Output
- A list of errors with file paths and field paths
- Zero errors required to proceed
