# humanity-animation

## What it is for

Character animation, as plain Rust (docs/design/character-animation.md; rungs 0 and 1 built
2026-11-08). It holds the humanoid skeleton standard (VRM 1.0's bone names as roles, mapped from any
rig by a role map in `data/characters/role_maps/`), poses and the pose palette the GPU skins with,
clips sampled from imported keys and retargeted onto a body by role, the joint map that reads
MakeHuman's skeleton across onto SOMA's 30 joints, blend spaces and the locomotion state machine,
the motion tracker that reads speed and turn rate from positions, and the near and middle drawing
tiers. It is a crate of its own because it is pure (no GPU, no egui, no file format reader: the engine
reads glTF into `humanity_core::skin_data` and draws the palettes), so every test runs headless, and
because animation never reaches the server: the relay build never compiles it (the operator,
2026-11-08: "a new crate, humanity-animation: yes").

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-animation/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-animation/src/clip.rs`, `crates/humanity-animation/src/graph.rs`, `crates/humanity-animation/src/humanoid.rs`, `crates/humanity-animation/src/joint_map.rs`, `crates/humanity-animation/src/library.rs`, `crates/humanity-animation/src/motion.rs`, `crates/humanity-animation/src/pose.rs`, `crates/humanity-animation/src/retarget.rs`, `crates/humanity-animation/src/role_map.rs`, `crates/humanity-animation/src/skeleton.rs`, `crates/humanity-animation/src/tiers.rs`
  - test builds only: `crates/humanity-animation/src/fixtures.rs`, `crates/humanity-animation/src/tests.rs`
- **Size:** 2,238 lines in 14 files, 484 of them in 2 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): none.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 3.
- **May be used by** (the plan): engine. **Used today by:** engine 9.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 12 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-animation
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_animation::`):

| Item | References | From |
|---|---:|---|
| `Animator` | 1 | engine 1 |
| `CharacterLibrary` | 1 | engine 1 |
| `DrawTier` | 1 | engine 1 |
| `MotionTracker` | 1 | engine 1 |
| `Pose` | 1 | engine 1 |
| `Tint` | 1 | engine 1 |
| `library::Fallback` | 1 | engine 1 |
| `pose` | 1 | engine 1 |
| `tiers` | 1 | engine 1 |

All 9 items shown.
<!-- end generated -->

## Rules

- **The body is an asset choice, not a code choice.** No body, rig, clip, state or role is named in
  code: bodies, role maps, joint maps, clips, the graph and the tiers are files under
  `data/characters/`; a new rig is a role map. Source: the operator, 2026-11-08 (build it
  body-agnostic); CLAUDE.md, Infinite-of-X; character-animation.md section 9.
- **Pure.** No GPU, no egui, no file reading but the RON data in `library`; glTF arrives as
  `humanity_core::skin_data`. The relay never depends on it (`cargo check --features relay
  --no-default-features` holds that: the root names it only under `native`). Source: the operator,
  2026-11-08 (question 7); character-animation.md section 9.
- **Nothing kept across runs; in-memory results rebuilt from the files.** Clips are retargeted at
  load and never written; the engine reads the library again on a hot reload and, with the reuse
  switch off, every few seconds (`engine::animation`). A figure's place in the graph is state, not
  reuse. Source: CLAUDE.md, the no-caches rule; character-animation.md 8.7.
- **The capsule is the truth.** Nothing here moves a body; the inputs are read from where it went.
  Source: character-animation.md 3.3.
- **Poses never cross the network.** Remote bodies are animated from their interpolated positions.
  Source: character-animation.md 8.5.
- **The palette layout is shared with two shaders**: three `vec4` rows per matrix
  (`pose::PALETTE_ROWS_PER_JOINT`), read by `assets/shaders/skinning.wgsl` and
  `assets/shaders/skinning_vs.wgsl`; change all three together.

## How to test it

- **Its lib tests:** the `just test-lib --crate humanity-animation` line above (`src/tests.rs`, with
  the rigs of `src/fixtures.rs`: a T-pose body and a foreign rig with other names, proportions, an
  A-pose, rolled bone frames and a 0.01 armature). They cover the shipped standard against VRM 1.0's
  required bones, role maps and the hierarchy check, the palette, glTF's three interpolations, the
  retarget across the foreign rig, twist joints, blend spaces and stride warping, the SHIPPED graph
  on marker clips, the motion tracker, the tiers, and the shipped role maps with the
  MakeHuman-to-SOMA joint map.
- **The engine's side** (`src/engine/animation_tests.rs`, `src/assets/skin.rs`,
  `src/renderer/skinning.rs`): the shipped data read whole through the real glTF reader, the clip
  library's walk retargeted onto the test figure landing on the walk authored on the figure, the
  figure skinned on the CPU at rest and walking, tints, facing, the dev crowd, both skinning shaders
  validated against the megashader.
- **In the running game:** the rig vantages `anim-*` in `tests/visual/vantages.json` (the crowd
  verbs `crowd`, `figure_path`, `figures` and `camera_mode` of `engine/ipc.rs`), with
  `HUMANITY_FRAME_COSTS=1` for `cpu.animation`, `cpu.skinning` and `gpu.skinning`.
- **The test figure itself:** `node scripts/gen-test-figure.js --check` (in `just rig-tests`) fails
  when the committed glTF files differ from what the generator makes.

## Open work

- **Rung 2** (character-animation.md section 10): foot IK on decks, treads and terrain with foot
  locking, crouch, interaction anchors with motion warping, two grip sockets (the `sockets` rows of
  `humanoid.ron` are ready), true first person, and the network's stance, pitch and interaction
  fields.
- **The real body and clips**: MakeHuman's CC0 base mesh and skeleton and Quaternius's clips, once
  the operator says yes to each download (docs/reference/findings/2026-11-08-character-assets.md
  lists the files and the Quaternius licence question). The role maps `makehuman_default` and
  `soma_30` are written; the body file and its shape targets come with the download.
- **Middle bodies cast no sun shadow** (rung 1's vertex-shader path draws only in the scene pass);
  rung 4's instanced crowd skinning adds a skinned shadow PSO.
- **Middle-tier pose evaluation at 15 Hz with sharing**, and the far tier's baked animation: rung 4.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
