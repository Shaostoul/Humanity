# asset_rules.md — Assets as Representation, Not Authority

This document defines **strict rules for all visual and audio assets** used in Project Universe.

Assets exist to *represent* reality. They must never *define* reality.

Any asset that encodes mechanics, rules, or authoritative state is invalid.

---

## 1. Core asset principles

1. **Assets are non-authoritative** — mechanics live in data and simulation.
2. **Assets are interpretable** — humans and machines must understand what they depict.
3. **Assets are replaceable** — changing an asset must not change outcomes.
4. **Assets are truthful** — visuals must not misrepresent scale, capacity, or function.

---

## 2. Asset directory structure

```
assets/
├─ models/
│  ├─ glb/               # 3D models (GLB)
│  └─ collision/         # Optional simplified collision meshes
├─ textures/
│  ├─ albedo/
│  ├─ normal/
│  ├─ roughness/
│  ├─ metallic/
│  └─ masks/
├─ icons/                # UI symbols
├─ animations/
│  ├─ skeletal/
│  └─ procedural/
├─ audio/
│  ├─ music/
│  ├─ ambience/
│  └─ effects/
├─ shaders/
└─ ui/
```

---

## 3. 3D models (GLB)

### 3.1 Allowed content

GLB files may contain:

* geometry
* materials
* skeletal rigs
* animation clips
* named attachment points (e.g., `socket_handle`)

GLB files may not contain:

* gameplay logic
* physics constants
* damage values
* efficiency modifiers
* yield data

---

### 3.2 Scale and units

* **Canonical unit:** meters
* 1.0 unit = 1 meter

Rules:

* Models must be authored to real-world scale.
* Non-realistic scaling must be explicitly documented and justified.

---

### 3.3 Orientation

* Forward axis: +Z
* Up axis: +Y

Consistency is mandatory for tooling and automation.

---

## 4. Collision assets

* Collision meshes are simplified representations.
* Collision must approximate shape, not exaggerate capacity.
* Collision does not imply strength or durability.

---

## 5. Textures

### 5.1 Purpose

Textures convey appearance only:

* surface color
* roughness
* reflectivity

Textures may not encode:

* hit points
* quality tiers
* rarity

---

### 5.2 Resolution and intent

* Resolution choices must reflect intended viewing distance.
* Excessive resolution that harms low-power systems is discouraged.

---

## 6. Icons and UI assets

Icons:

* represent concepts or actions
* must be symbolic and unambiguous

Icons may not:

* imply power not present
* imply guarantees of success

---

## 7. Animations

Animations:

* represent motion or process
* do not imply speed, efficiency, or effectiveness

Example:

* A faster animation does not mean faster work.

---

## 8. Audio

### 8.1 Audio categories

* Music: mood and pacing
* Ambience: environmental context
* Effects: feedback

---

### 8.2 Audio limitations

Audio may:

* signal events
* reinforce feedback

Audio may not:

* encode hidden state
* replace visual or data-based feedback

---

## 9. Shaders

Shaders:

* interpret surface properties
* must not encode gameplay logic

Visual effects may not alter simulation outcomes.

---

## 10. Asset referencing

Assets are referenced from data definitions via:

* stable paths
* optional metadata pointers

Rules:

* Missing assets are non-fatal unless explicitly required by UI.
* Assets must not be required for simulation correctness.

---

## 11. Educational integrity

Assets used in education must:

* reflect real proportions
* avoid misleading abstraction
* be accompanied by explanatory text when simplified

---

## 12. Validation rules

The build must fail if:

* an asset contains embedded logic
* asset scale violates declared units
* an asset is required for simulation logic
* visual capacity contradicts data-defined capacity

---

## 13. Modding rules for assets

Mods may:

* add or replace assets
* reskin existing definitions

Mods may not:

* change mechanics via assets
* encode hidden bonuses or penalties

All modded assets are subject to the same validation rules.

---

## 14. Design intent restated

Assets are lenses.

They help humans perceive reality.

They do not define reality.
