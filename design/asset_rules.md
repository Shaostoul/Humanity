# asset_rules.md

This document defines strict rules for all visual and audio assets used in Humanity.

Assets exist to represent reality.  
They must never define reality.

Any asset that encodes mechanics, rules, or authoritative state is invalid.

---

## 1. Core asset principles

1. Assets are non-authoritative.  
   Mechanics live in design, data, and simulation.

2. Assets are interpretable.  
   Humans and tools must understand what they depict.

3. Assets are replaceable.  
   Changing an asset must not change outcomes.

4. Assets are truthful.  
   Visuals must not misrepresent scale, capacity, or function.

---

## 2. Asset organization

Assets are grouped by representation type:

- models (3D geometry)
- textures (surface appearance)
- icons (symbolic UI)
- animations (motion only)
- audio (feedback and ambience)
- shaders (visual interpretation)
- UI assets (layout and presentation)

Exact folder layout is implementation-specific but must preserve this separation.

---

## 3. 3D models (GLB)

### Allowed content

GLB files may contain:
- geometry
- materials
- skeletal rigs
- animation clips
- named attachment points

GLB files may not contain:
- behavior
- logic
- physics constants
- damage values
- efficiency modifiers
- yield data

---

### Scale and units

- Canonical unit: meters  
- 1.0 unit equals 1 meter

Models must be authored to real-world scale.

Any intentional deviation must be explicitly documented and justified.

---

### Orientation

- Forward axis: +Z  
- Up axis: +Y

Consistency is mandatory for tooling and automation.

---

## 4. Collision assets

- Collision meshes are simplified representations.
- Collision approximates shape only.
- Collision does not imply strength, durability, or capacity.

---

## 5. Textures

Textures convey appearance only:
- surface color
- roughness
- reflectivity

Textures may not encode:
- hit points
- quality tiers
- rarity
- mechanical modifiers

Resolution must match intended viewing distance.

Excessive resolution that harms low-power systems is discouraged.

---

## 6. Icons and UI assets

Icons:
- represent concepts or actions
- must be symbolic and unambiguous

Icons may not:
- imply power not present
- imply guarantees of success
- misrepresent consequences

---

## 7. Animations

Animations represent motion or process only.

They must not imply:
- speed
- efficiency
- effectiveness

A faster animation does not mean faster work.

---

## 8. Audio

Audio categories:
- music (mood and pacing)
- ambience (environmental context)
- effects (feedback)

Audio may:
- signal events
- reinforce feedback

Audio may not:
- encode hidden state
- replace visual or data-based explanation

---

## 9. Shaders

Shaders interpret surface properties only.

Shaders may not:
- encode mechanics
- encode thresholds
- alter simulation outcomes

Visual effects must remain non-authoritative.

---

## 10. Asset referencing

Assets are referenced from data via:
- stable paths
- optional metadata pointers

Rules:
- Missing assets are non-fatal unless explicitly required by UI.
- Assets must never be required for simulation correctness.

---

## 11. Educational integrity

Assets used for learning must:
- reflect real proportions
- avoid misleading abstraction
- be accompanied by explanation when simplified

---

## 12. Validation rules

The build must fail if:
- an asset contains embedded logic
- asset scale violates declared units
- an asset is required for simulation logic
- visual capacity contradicts data-defined capacity

---

## 13. Asset modification

Mods may:
- add or replace assets
- reskin existing definitions

Mods may not:
- change mechanics through assets
- encode hidden bonuses or penalties

All modded assets are subject to the same validation rules.

---

## Closing statement

Assets are lenses.

They help humans perceive reality.

They do not define reality.
