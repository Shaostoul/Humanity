# Character Customization Roadmap (Human + Suit Layer)

## Goal
Support a customizable human character with realistic body physics and a continuous base suit layer (~1mm conceptual thickness) that always remains present, plus attachable style layers.

## Layer model

1. **Body layer**
   - human morphology parameters
   - rig + physics constraints

2. **Base suit layer (always on)**
   - form-fitting full-body pressure/safety suit
   - supports visual style parameters without removing protective baseline

3. **Attachment layers**
   - skirts, blouses, belts, bandoliers, jewelry, gloves, socks, shoes, backpacks, etc.

## Data model direction

- body profile
- face/hair profile
- suit style profile
- attachment manifests
- material/physics tags

## Phase order

1. data schema + save format
2. runtime placeholder mannequin
3. body parameter morph controls
4. base suit rendering + color/style controls
5. attachment socket system
6. cloth/secondary physics tuning

## Requirement
Cosmetic identity must remain portable across offline, p2p, and dedicated modes.
