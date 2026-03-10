# Difficulty -> Simulation Fidelity Matrix

Goal: map player-facing difficulty to simulation depth in a controlled, deterministic way.

## Difficulty ladder

1. Baby/Creative
2. Easy
3. Medium
4. Hard
5. Realistic

## Fidelity levels

- L0: disabled
- L1: abstract
- L2: medium systems
- L3: high-fidelity (organ-level, complex interactions)

## Matrix (authoritative per session)

| System Domain | Baby/Creative | Easy | Medium | Hard | Realistic |
|---|---:|---:|---:|---:|---:|
| Hydration/Energy | L1 | L2 | L2 | L3 | L3 |
| Injury | L0 | L1 | L2 | L2 | L3 |
| Organ Damage | L0 | L0 | L1 | L2 | L3 |
| Disease | L0 | L1 | L2 | L2 | L3 |
| Affect/Stress | L1 | L1 | L2 | L2 | L3 |
| Skill Degradation/Fatigue | L0 | L1 | L2 | L2 | L3 |
| Animal Welfare Dynamics | L0 | L1 | L2 | L2 | L3 |
| Crop/Soil Interdependence | L1 | L1 | L2 | L2 | L3 |

## Authority rule

Fidelity difficulty is session-authoritative, not per-player, to avoid net desyncs and progression exploits.

## Accessibility overrides

UI and teaching guidance can be personalized per player, but simulation law remains session-authoritative.

Examples:
- child mode gets simplified UI explanations while simulation remains Medium for the session
- adult mode can show full diagnostics in Realistic sessions

## Implementation note

Expose difficulty as a `FidelityPreset` enum in core session orchestration to ensure deterministic toggles and policy checks.
