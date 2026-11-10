# humanity-animation

## What it is for

Character animation, as plain Rust (docs/design/character-animation.md; rungs 0 and 1 built
2026-11-08, the real bodies and clips 2026-11-09). It holds the humanoid skeleton standard (VRM 1.0's
bone names as roles, mapped from any rig by a role map in `data/characters/role_maps/`), poses and
the pose palette the GPU skins with (with its morph block: which shape targets a body draws with),
clips sampled from imported keys and retargeted onto every body by role, the joint map that reads
MakeHuman's skeleton across onto SOMA's 30 joints, blend spaces and the locomotion state machine,
the motion tracker that reads speed and turn rate from positions, the near and middle drawing
tiers, and the bodies themselves (a female and a male MakeHuman body and the test figure as the
fallback), each with the shape targets the look drives (height, and since 2026-11-09 the character
page's sliders: build, proportions and ancestry, each a row of the body file) and a skeleton that
follows them (`Skeleton::reshaped`), and the stable hash that varies the crew's shapes. Since
2026-11-09 (rung 2a) it also puts the feet on the world (`feet`: contacts labelled on every clip at
load, the ground asked of the engine through the `Ground` trait, two-bone analytic leg IK on each
person's reshaped skeleton, the pelvis offset, the foot turned to the slope about a toe pivot, foot
locking and stair footsteps), and the graph reads crouch, time off the ground, vertical speed and
time in a state. It is a crate of its own because it is pure (no GPU, no egui, no file format reader: the engine
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
- **The palette layout is shared with two shaders and the renderer**: three `vec4` rows per matrix
  (`pose::PALETTE_ROWS_PER_JOINT`), and per body its model matrix, then the morph block
  (`pose::MORPH_SLOTS` matrices: the active shape targets and their weights), then its joints; read
  by `assets/shaders/skinning.wgsl`, `assets/shaders/skinning_vs.wgsl` and written as
  `renderer::skinning::MORPH_SLOTS` (that module compiles in the relay build, which has no animation
  crate; `engine::animation`'s tests hold the two equal). Change all of them together.
- **A slider is a row of the body file, never code** (2026-11-09): `shape.sliders` names each
  slider's id, its group and the targets it drives (`Grid`, `Line`, `Blend`); the ids are the
  look's keys (`Appearance::shape`, `PlayerLook::shape`) and the words' keys
  (`characters.shape_<id>`), so a new slider is a row and its words. A look without a value is the
  body as made. The person's height is theirs: `shape_for` lets the height targets take up what the
  slider targets do to it. The loader refuses rows that could turn on more targets at once than
  `MAX_ACTIVE_MORPHS`. Source: the operator, 2026-11-09 ("Yeah, let's go for it."); CLAUDE.md,
  Infinite-of-X; character-animation.md section 14, the sliders.
- **The feet never read the world themselves.** The ground under a foot comes through
  `feet::Ground`, which the engine implements (`engine::feet_ground`: the footing aboard, the drawn
  terrain on a planet with positions and directions in f64 to the sampler); the solve works in the
  body's model space on the person's OWN reshaped skeleton (`Figure.shape`, never the body's
  unshaped one), and nothing here moves the person (the capsule is the truth). Feet are never
  aligned across rigs (rung 1's lesson): a sole point is its joint's rest height, turned with the
  joint. Source: character-animation.md section 4 and section 15.
- **Every new number of the feet is data**: `characters.ron`'s `feet` (`FeetDef`, each field with
  its shipped default), so a modder tunes them and the tests read the shipped ones. Source:
  CLAUDE.md, Infinite-of-X.
- **A clip's hips travel is an offset from the rest, and its speed is the body's.** Retargeting
  scales a gait's `speed_mps` by the body's leg length against the source's, and the engine divides
  a person's speed by their own stride scale (shape and size) before the graph reads it, so the
  feet keep still on every body. Source: 2026-11-09, the first real clips on two bodies.

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
- **The real bodies and clips:** `node scripts/gen-makehuman-body.js --check` and
  `node scripts/gen-quaternius-clips.js --check` (both in `just rig-tests`) rebuild them from the
  sources on `E:\HumanityOS\` and compare byte for byte, or, where the sources are not on the machine,
  compare the files with the hashes their `SOURCE.md` records. `--preview DIR` on the body converter
  renders front, side, back and head views to PNG without a boot.
- **The engine's MakeHuman tests** (`src/engine/animation_tests.rs`): both bodies skin to their
  bind mesh at rest and stay whole in every gait and stance; a look's height reshapes the body and
  its skeleton follows (the reshaped mesh skins onto itself at rest); the morph block and morph
  buffer are laid out as the shaders read them. Since 2026-11-09, the sliders: each row drives its
  targets as the body file says and the height holds while they move
  (`the_body_sliders_drive_their_targets_as_the_body_file_says`), the skeleton follows a build
  slider (`the_skeleton_follows_a_build_slider`), every slider has its words, and a crew is varied
  and stable (`a_crew_is_varied_and_each_keeps_their_shape`).
- **The feet (rung 2a, 2026-11-09):** the crate's `feet::tests` (the two-bone solve reaches its
  target and keeps the knee in front) and `tests::the_shipped_graph_crouches_and_jumps`; the
  engine's `src/engine/animation_feet_tests.rs` walks the real MakeHuman bodies through their real
  clips over a test floor, stair and slope and measures the DRAWN feet (foot vertices skinned with
  the GPU's palette, `engine::foot_probe::skinned_feet`), each with a feet-off control that must
  fail the same bound. In the running game: the vantages `anim-stairs`, `anim-ramp`, `anim-crouch`,
  `anim-slope-map` with their `foot_probe` check (scripts/lib/foot-probe-check.js) and their
  `-feet-off` red twins; `anim-crowd-{near,middle}-feet-off` for the cost.
- **The sliders in the running game:** the vantages `anim-shape-{female,male}-{build,ancestry}` (and
  `-skeleton`, the joints and bones drawn on top: showcase `skeletons`, the Dev page's Show
  skeletons) stand a row of people at the sliders' ends (showcase `lineup`); `anim-crowd-*-plain`
  is the crowd with no shapes, the same-boot twin that measures what varied shapes cost.

## Open work

- **The rest of rung 2** (character-animation.md section 10; 2a, the feet, crouch, jump and the
  network's stance, built 2026-11-09, section 15): interaction anchors with motion warping (sit,
  bed, ladder with hatch, station, door), two grip sockets (the `sockets` rows of `humanoid.ron` are
  ready), hands on rungs (two-bone arm IK, the same `feet::two_bone`), true first person, and the
  network's pitch and interaction fields.
- **Sit** is retargeted (`clips.ron`) but no state plays it until the interaction anchors.
- **Feet, what 2a left** (section 15, "Known limits"): crouching keeps the standing capsule (no
  passing under low things yet); other players and the crew on a planet still stand +Y up (only the
  player's own body takes the local radial); the dev crowd on a planet lives in the home frame;
  the stair planner moves a foot only along the stair; starting to walk from standing can hold a
  planted foot until its lock stretches to the break and snaps (a start step is the fix).
- **Twist joints on the MakeHuman bodies**: MakeHuman's arm and leg twist segments are chained
  (`lowerarm02` is the wrist's parent), not siblings of the driver as `apply_twists` expects, so the
  converter merges them for now; a chained twist (or the converter keeping them as siblings) is the
  skinning-quality step after rung 2.
- **Garments must follow the sliders** (2.4's second half, with MakeHuman's clothes): a garment
  needs the same targets transferred to it, or a fitted proxy, so one jacket fits every build.
- **A look changed mid-session reaches others only at the next join** (the look rides `game_join`);
  a small look update message is the step when that matters.
- **Middle bodies cast no sun shadow** (rung 1's vertex-shader path draws only in the scene pass);
  rung 4's instanced crowd skinning adds a skinned shadow PSO.
- **Middle-tier pose evaluation at 15 Hz with sharing**, and the far tier's baked animation: rung 4.
  With the body sliders each middle body skins up to seven shape targets in the vertex shader (0.2
  to 0.6 ms of scene at 34 to 50 bodies, character-animation.md 8.0): rung 4's crowd draw should give
  middle bodies only their largest two or three.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
