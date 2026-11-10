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
time in a state. Since 2026-11-09 ("the look", design section 16) it builds each near person's OWN
mesh (`look`: the shape and face and body detail targets applied to every base-mesh vertex, the
body's triangles under a worn garment dropped, MakeHuman's clothes, shoes, hats, hair, brows and
lashes placed on the shaped mesh by their bindings and skinned with the bound vertices' weights, the
skeleton made again from the shaped mesh, the feet's soles measured with the shoes on), versioned
by everything it is built from and rebuilt every frame with the reuse switch off; and the library
resolves what each person wears (`dress_for`: their look's choices, else their wearer kind's
defaults) and how large the largest dressed person can be (`largest_own_mesh`, 2026-11-10: the engine
plans the own-mesh pool, `renderer::mesh_pool`, for it). Since 2026-11-10 (rung 2b, design section 17) it also
holds the interactions (`interact`: anchors as rows of `data/characters/interactions.ron` that objects name,
motion warping of the hips onto THIS seat from the enter clip's contact frame, the lying and climbing
target poses, two-bone arm IK with a grip library blended per finger, look-at, and the first-person
head folded away). It is a crate of its own because it is pure (no GPU, no egui, no file format reader: the engine
reads glTF into `humanity_core::skin_data` and draws the palettes), so every test runs headless, and
because animation never reaches the server: the relay build never compiles it (the operator,
2026-11-08: "a new crate, humanity-animation: yes").

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-animation/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-animation/src/clip.rs`, `crates/humanity-animation/src/feet.rs`, `crates/humanity-animation/src/graph.rs`, `crates/humanity-animation/src/humanoid.rs`, `crates/humanity-animation/src/interact.rs`, `crates/humanity-animation/src/joint_map.rs`, `crates/humanity-animation/src/library.rs`, `crates/humanity-animation/src/look.rs`, `crates/humanity-animation/src/motion.rs`, `crates/humanity-animation/src/pose.rs`, `crates/humanity-animation/src/retarget.rs`, `crates/humanity-animation/src/role_map.rs`, `crates/humanity-animation/src/skeleton.rs`, `crates/humanity-animation/src/tiers.rs`
  - test builds only: `crates/humanity-animation/src/fixtures.rs`, `crates/humanity-animation/src/tests.rs`
- **Size:** 6,232 lines in 17 files, 580 of them in 2 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): none.
- **May use** (the plan): core. **Uses today** (references in non-test code): core 21.
- **May be used by** (the plan): engine. **Used today by:** engine 137.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 21 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-animation
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_animation::`):

| Item | References | From |
|---|---:|---|
| `Phase` | 14 | engine 14 |
| `AnchorKind` | 11 | engine 11 |
| `GarmentKind` | 8 | engine 8 |
| `DressChoice` | 7 | engine 7 |
| `interact::phase_len` | 7 | engine 7 |
| `LookLibrary` | 5 | engine 5 |
| `FootRig` | 4 | engine 4 |
| `LoadedBody` | 4 | engine 4 |
| `PersonMesh` | 4 | engine 4 |
| `Skeleton` | 4 | engine 4 |
| `interact::ContactPart` | 4 | engine 4 |
| `Limbs` | 3 | engine 3 |
| `Part` | 3 | engine 3 |
| `look::build_person` | 3 | engine 3 |
| `AnchorDef` | 2 | engine 2 |

And 39 more items, 54 references (`node scripts/crate-cards.js --interface humanity-animation` lists them all).
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
- **A person's own mesh is built from the look, never kept past it** (2026-11-09): `look_key` hashes
  everything `build_person` reads (body, shape weights, detail values, garments, the two check
  switches), `rebuild_due` builds again when the key changes and every frame with the reuse switch
  off (`humanity_core::reuse`). The engine builds on worker threads (`characters.ron`'s
  `look_threads`, 2026-11-10; `build_person` is a pure function of the shared library and the inputs,
  so keep it free of anything a thread cannot hold), drawing the last mesh built for that body while
  a rebuild runs, and with the switch off builds on the frame, every frame. The shared body with its
  macro shape on the GPU draws everyone else. Source: CLAUDE.md,
  the no-caches rule (refined 2026-10-30); character-animation.md section 16.
- **What is measured from the body is measured from the person's own mesh** (2026-11-09; rung 2a's
  lesson): a person's legs, soles, toe cap and heel counter come from their built mesh, shoes on
  (`FootRig::with_soles` before `with_extents`), and a shoe's sole lifts them (`PersonMesh::lift`).
  Source: character-animation.md section 15, "For the clothes and hair (the next rung), what this
  one learned".
- **Garments, sliders and dress are data**: `data/characters/details.ron` and `garments.ron` are
  written by `scripts/gen-makehuman-look.js` (never by hand); cosmetics.csv's `model` and `covers`
  name a garment and the cover regions it hides; `characters.ron`'s wearers carry each kind's
  default `dress` per body. A garment's row names its texture as the converter made it (`texture`, a
  BC7 DDS; `texture_px`, `normal_map`, `luma`, 2026-11-10; `art_px`, its colour map's own size, the most it is drawn at, the same day); the crate reads only the row (`look::read_garment_defs` reads the rows alone, `CharactersDef::parse` the characters file alone, for Settings before the world loads), the engine
  reads the file. Source: CLAUDE.md, Infinite-of-X; character-animation.md section 16.
- **An interaction is an anchor row that objects name, never code** (2026-11-10, rung 2b):
  `data/characters/interactions.ron` holds each anchor (its place on its object, the approach, the
  contacts, each phase's clip or target pose, root driven or not, phase limits), `grips.ron` the
  grips; a machine definition's, a structure type's or an opening style's `anchors` name them. The
  crate poses an engaged person (`interact::pose`, `apply_limbs`, `look_at`); it never decides who is
  engaged or moves anyone: the engine's gameplay grants (`engine::interact`) and the capsule stays the
  truth. The anchor ids also go over the network, so the core compiles the shipped file's ids in
  (`humanity_core::stance::AnchorIds`) and both ends drop any other. Source: character-animation.md
  3.3 and section 17; CLAUDE.md, Infinite-of-X.
- **The seat's contact is measured on the drawn mesh** (2026-11-10): the warp puts the hips joint
  over the seat by how far the person's own seat flesh (trousers on) stands under it in the anchor's
  pose, measured on the mesh they are drawn with (`engine::anchor_probe::seat_drop`), never a
  constant; rung 2a's lesson ("measure what is drawn") again.
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
- **The look (2026-11-09):** the crate's `look::tests` (the version and the reuse switch, red with
  the switch ignored); the core's `look_data` tests (a .mhclo bound vertex lands where MakeHuman's
  own fitted .obj put it on the raw base mesh, red with two weights swapped; the files read and a
  broken one is refused) and `player_look` tests (the one clamping reader: unknown slider, out of
  range, too many; the look's rate); the engine's `src/engine/animation_look_tests.rs` (a person's own
  mesh is their look; the build's cost and memory; shod feet stand on the floor, red with the soles
  not measured; no skin through the clothes in walk, crouch and sit on the drawn skinned mesh, red
  with covers off) and `look_textures`' read test (either size, either GPU); the renderer's `bc7` tests (the
  shipped textures decode to their sources; `the_gpu_decodes_bc7_as_we_do`, ignored by default, ties the
  decoder to the GPU's) and `texture_pool` tests (a slot handed on and the pool allocated once, red with
  an allocation per garment); the renderer's `mesh_pool` tests (2026-11-10: room handed from person to
  person and the own-mesh pool's buffers allocated once, red with an allocation per person; a mesh past
  the plan refused and taking nothing; the longest kept giving way, then packing until the largest
  fits) and `the_own_mesh_pool_is_planned_for_the_largest_dressed_person` here in the look tests (the
  largest dressed person built for real fits `largest_own_mesh`, red with a two-sided garment counted
  once); `scripts/tests/bc7.test.js` (our encoder); the relay's `msg_handlers::look_tests` (the cap
  and the reader at the relay). In the running game: `anim-look-face-{female,male}` (detail sliders at
  their ends), `anim-look-covers-off` (the red arm, seen), `anim-shoes` and `anim-shoes-soles-off`
  (the shod foot probe, and its red twin that must FAIL), `anim-stairs-soles-off` and
  `anim-crowd-near-look-off` (the cost twin). The converters:
  `node scripts/gen-makehuman-look.js --check` and `gen-makehuman-body.js --check`.
- **The interactions (rung 2b, 2026-11-10):** the crate's `interact::tests` (the climb's hands
  alternate on rungs, red with both hands moving together, and let go past the top, red held on;
  look-at's limits, red unclamped; the phase weights, red with the exit run forward and again with
  the enter and exit swapped; the phase codes); the core's `stance::act_tests` (the
  pitch and act readers, red with any anchor id kept and the pitch unclamped); the engine's
  `src/engine/animation_interact_tests.rs` on the shipped bodies (the drawn hips on the chair, the
  couch and the bed's edge with warping on, and failing with it off; lying on the covers; the
  gripping hands on the rungs up and past the top, red with the hands off and with them held on the
  top rung; the hands on the workbench; the first-person
  head folded away, red drawn; every anchor an object names is a row); the relay's
  `a_players_pitch_and_act_reach_the_others_clamped` (a real socket); `scripts/tests/anchor-probe-check.test.js`
  (the rig's judge). In the running game: `anim-sit-chair`, `anim-sit-couch`, `anim-lie-bed`,
  `anim-ladder-climb`, `anim-workbench`, `anim-door-hand`, `anim-fp-legs` with their `anchor_probe`
  check and the red twins (`-warp-off`, `-hand-ik-off`, `-head-shown`) that must FAIL it;
  `anim-crowd-{near,middle}-2b-off` for the cost.
- **The sliders in the running game:** the vantages `anim-shape-{female,male}-{build,ancestry}` (and
  `-skeleton`, the joints and bones drawn on top: showcase `skeletons`, the Dev page's Show
  skeletons) stand a row of people at the sliders' ends (showcase `lineup`); `anim-crowd-*-plain`
  is the crowd with no shapes, the same-boot twin that measures what varied shapes cost.

## Open work

- **What rung 2b left** (character-animation.md section 17, "Known limits"): lying, climbing, a
  bench and a door are target poses with IK (the free clip library has no clip for them); the doors
  open themselves at 2.6 m, so a hand takes a door already swinging rather than opening it; held
  things are not drawn (no item has a model), only the hands' grips and the two-handed hold; what
  each hand holds does not cross the network yet; a crew member's activity names no anchor until the
  crowd timetable (crowd-simulation.md rung 3); the take-off anticipation from the jump key is still
  open.
- **Feet, what 2a left** (section 15, "Known limits"): crouching keeps the standing capsule (no
  passing under low things yet); other players and the crew on a planet still stand +Y up (only the
  player's own body takes the local radial); the dev crowd on a planet lives in the home frame;
  the stair planner moves a foot only along the stair; starting to walk from standing can hold a
  planted foot until its lock stretches to the break and snaps (a start step is the fix).
- **Twist joints on the MakeHuman bodies**: MakeHuman's arm and leg twist segments are chained
  (`lowerarm02` is the wrist's parent), not siblings of the driver as `apply_twists` expects, so the
  converter merges them for now; a chained twist (or the converter keeping them as siblings) is the
  skinning-quality step after rung 2.
- **The look, what 2026-11-09 left** (design section 16, "Known limits"): middle bodies draw the
  shared body with tinted regions (no garments or hair meshes); the player's own hair fringe is not
  blended on a planet (the core is); hair has no strand-depth occlusion or self-shadow map yet; the
  generated eyes stay (MakeHuman's eye meshes and the teeth and tongue are not drawn); MakeHuman's
  proxy meshes are not used; the sport top's hem rides over the skin at the small of the back in a
  crouch or a seat (up to 19 vertices, 2.8 cm); measurement sliders do not read back in centimetres.
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
