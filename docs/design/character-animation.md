# Character animation: one body, from the player to a ship of crew

**Status: rungs 0 and 1 BUILT on 2026-11-08 (section 13), with the real bodies (MakeHuman, female
and male), clips (Quaternius), the walking pace and the body sliders on 2026-11-09 (section 14);
rung 2a, feet on the world with crouch, jump and the network's stance, BUILT on 2026-11-09 (section
15); the rest of rung 2 and on are DESIGN. Section 12's questions were all decided on 2026-11-08.** Written after the operator came
across NVIDIA's MotionBricks (https://github.com/NVlabs/motionbricks, SIGGRAPH 2026) on 2026-11-08
and asked where a technique of that class fits. The licensing question has its own dated finding:
[`docs/reference/findings/2026-11-08-motionbricks.md`](../reference/findings/2026-11-08-motionbricks.md).

This is the whole feature planned once, with all its bells and whistles, as CLAUDE.md asks
("Plan the whole feature, build it in rungs"), so the data shapes, the frame budget and the scale
are decided before the first slice, and the first slice is a rung of the real thing. It builds on
what exists (section 0) and on three designs that already wait for it:
[`crowd-simulation.md`](crowd-simulation.md) (animation is its rung 6),
[`fire-props-and-combustion.md`](fire-props-and-combustion.md) (props in hands need "a character
body with skeletal animation and two hand attachment points"; the operator, 2026-09-24: "The
character body makes the most sense long term.") and [`camera-system.md`](camera-system.md) (third
person "Shows the player's character, gear, and animations").

## The answer in one paragraph

Build **one humanoid body and one animation system** that the player (first and third person), the
other players and the crew all use, and a family of skeletons for creatures, all as data. Drive it
through **one interface borrowed from MotionBricks**: the character's recent poses, plus target
poses placed relative to the thing it is using (a chair, a ladder, a workbench), plus where its root
should go. Behind that interface, pick the solver per rung: a **state machine with blend spaces**
first (rung 1, and the simplified mode for good), then **motion matching with motion warping and
inertialized transitions** as the full-realism core (the technique shipping games use today), then
**learned compression** when the database grows, and only last, if it beats motion matching in a
blind comparison within budget, **a learned inbetweener of the MotionBricks class**, preferably our
own trained on open data. Feet and hands meet the real decks, stairs, ladders and terrain through
IK. Poses never cross the network: only states and targets, at the rate positions already go. The
NVIDIA weights are legally usable but not open source by the OSI's measure; whether they may ever
be an optional pack is the operator's call (question 1).

## 0. What exists today (read from the code on 2026-11-08)

- **No skinned mesh anywhere.** The vertex type (`crates/humanity-core/src/vertex.rs`) has no joint
  indices or weights. The glTF loader reads the first primitive of the first mesh
  (`src/assets/mod.rs`, `primitives().next()`); the model viewer shows a static pose: "skins, morph
  targets and animations are not played" ([`model-viewer.md`](model-viewer.md)).
- **The player's own avatar** is boxes and a sphere on a podium (`place_avatar`,
  `src/engine/world_load.rs`), whose comment says "later increments swap this for a skinned mesh
  with cosmetic slots". **Other players and crew** are a body, a head and hair
  (`figure_parts`, `remote_figure_parts`, `crew_figure_parts` in `src/engine/net_route.rs`), turned
  at a person's turn rate (`crates/humanity-core/src/turning.rs`, BUG-165 and BUG-166).
- **Cosmetics** tint six slots, head, chest, legs, feet, hands and back
  (`crates/humanity-core/src/cosmetics.rs`, `data/cosmetics/cosmetics.csv`); "when a skinned mesh
  lands, a `model` column gets added and the slot renders the gltf instead".
- **Camera**: first person, third person and orbit (`src/renderer/camera.rs`). No body is drawn in
  first person.
- **Movement** the body must show: walk, sprint, jump, falls and landings
  (`crates/humanity-systems` `falls.rs`, `landing.rs`), swimming and flight on planets
  (`src/lib.rs`), stairs, ramps, slabs, elevator cars and ladders with hatches and gates
  (`src/engine/footing.rs`, `ladder_at`, `ladder_gates`), carrying loads (`src/engine/carry_load.rs`),
  sleeping in a bed (`sleep.rs`), wounds and illness (`wounds.rs`, `illness.rs`), body heat
  (`body_heat.rs`). There is no crouch control. Zero-g's attach, impulse and tether primitives are
  designed ([`gravity-and-movement.md`](gravity-and-movement.md)), not built.
- **Creatures**: livestock wander on graze paths with "Placeholder block bodies sized from real
  species mass" (`docs/FEATURES.md`, Livestock; `crates/humanity-systems/src/livestock.rs`);
  predators hunt the player under their own locomotion and bite
  (`crates/humanity-systems/src/ai/mod.rs`). Species are rows of `data/creatures.csv`.
- **Network**: our position goes out 15 times a second with the camera's yaw
  (`drive_position_send`, `src/engine/net_route.rs`), delivered only to players who have us in view
  (`src/relay/handlers/game_interest.rs`).
- **LOD**: `data/lod/categories.ron` has `animal_small` and `animal_large` rows, every one with
  `billboard_m: 0.0`; the tree card baker (`src/renderer/billboard_bake.rs`) bakes side views at run
  time.
- **GPU**: wgpu 24 with compute pipelines in use (`src/renderer/particles_gpu.rs`,
  `room_probes_gpu.rs`), storage buffers at default limits. Rapier joints exist but are never created
  (fire-props design, section 1).
- **No machine-learning runtime** is in the dependency tree.

## 1. The whole feature, stated once (the 2030 version)

1. One **humanoid body** with a standard skeleton, skinned on the GPU, its shape from the person's
   look (height, build), dressed by cosmetic slots that are real meshes.
2. **The same body in first and third person**: in first person you see your arms, legs and
   clothes, and your shadow is a person's.
3. **Locomotion** that reads as a real person: starts, stops, turns, strafes, crouch, stairs,
   slopes, ladders, swimming, falls and hard landings, zero-g pushes and grabs, carrying, and gaits
   coloured by the body's state (a wounded leg limps, cold shivers, tiredness slumps).
4. **Interactions as data**: sit, lie down, use a station, open a door, climb through a hatch,
   vault, harvest, milk, pick up, hand over, each an anchor on the object, aligned to the real object
   by warping.
5. **Feet and hands on the real world**: foot IK on decks, treads and terrain, foot locking with no
   sliding, hands on ladder rungs and props, head and eyes toward what the person looks at.
6. **Two hands for props**, as the fire props need: two grip sockets and grip poses.
7. **Crew and crowds** at the scale crowd-simulation.md plans, with level of detail from a full body
   near the camera to baked animation far away.
8. **Creatures** with their own skeletons and gaits from real gait data.
9. **Secondary motion and physics**: hair and clothes swing, ragdoll on death and hard falls, hit
   reactions; all client-local.
10. **Faces**: blinks, gaze and lip movement from the person's own voice, client-local.
11. **VR**: the body driven from a headset and two hands through the same IK.
12. **Dev tooling**: a skeleton overlay, motion matching's candidates, contact and IK markers, an
    Animation section on the Dev page, rig vantages for every interaction, and the frame costs
    split per stage.

## 2. The body

### 2.1 One skeleton standard, by role

- **A canonical humanoid skeleton in data**, `data/characters/skeletons/humanoid.ron`: each joint's
  name, parent, rest pose, its **role** and the lowest LOD tier it belongs to. Roles use the bone
  names of VRM 1.0's humanoid list (`hips`, `spine`, `chest`, `upperChest`, `neck`, `head`,
  `leftUpperLeg`, `leftLowerLeg`, `leftFoot`, `leftToes`, `leftShoulder`, `leftUpperArm`,
  `leftLowerArm`, `leftHand`, the fingers, `leftEye`, `jaw` and the right side), an open, documented
  naming for humanoids in glTF, so any humanoid rig that names its bones that way maps by role.
  We adopt the names as a vocabulary, not the VRM file format.
- **Twist joints** for the forearms, upper arms and thighs (skinning quality where linear blend
  skinning collapses), and **sockets** (`grip_l`, `grip_r`, `back`, `hip_l`, `hip_r`, `head_top`)
  as data, not code, so a prop attaches to a named socket.
- **About 60 joints at full detail** (30 of them fingers), about 25 at middle detail, a handful for
  the far tier's bake. Exact counts come from the body chosen in rung 1.
- **Every motion source is retargeted by role at load**: rest poses aligned, the root's travel
  scaled by the ratio of leg lengths, in memory only (never written back to disk; section 9).
  MotionBricks' SOMA output, CMU's ASF/AMC, BVH and glTF clips all go through the same retargeter.
- **Creatures are more rows** (section 6): a skeleton file per body plan, named in a new
  `skeleton` column of `data/creatures.csv`.

### 2.2 glTF skinning, and how the GPU does it

- The glTF loader learns skins: all primitives, the node tree, `skins` with `joints` and
  `inverseBindMatrices`, and the `JOINTS_0`/`WEIGHTS_0` attributes (four influences a vertex; a
  second set for faces and hands where an artist uses eight). The spec: "Each skin contains a
  REQUIRED `joints` property that lists the indices of nodes used as joints and an OPTIONAL
  `inverseBindMatrices` property". Morph targets (for body shape and correctives) the same way.
- A **skinned vertex** type beside today's: position, normal, tangent, UV, joint indices (u8x4 or
  u16x4), weights (unorm16x4).
- **The pose palette** is one storage buffer per frame: a 3x4 matrix (48 bytes) per joint, each
  character at an offset, so instancing and skinning coexist (crowd-simulation.md's note, and the
  v0.782 lesson that storage buffers are proven by booting, not by a test).
- **Near characters are skinned once a frame by a compute pass** into a transient vertex buffer the
  depth, shadow cascades and main passes all read, so a body is not skinned again for every shadow
  cascade. **Middle and far characters skin in the vertex shader**, which costs no extra memory.
  Linear blend skinning with twist joints first; dual-quaternion skinning for shoulders and wrists
  and pose-space corrective shapes (elbows, knees, shoulders) are later rungs.

### 2.3 Where the body comes from

- **MakeHuman's assets are CC0** (base mesh, the targets that change body shape, a 163-bone default
  skeleton, clothes), and it makes "no claim whatsoever" over exported files (the MotionBricks
  finding, section 11). A
  CC0 body with real shape controls fits a CC0 project, and its targets drive `Appearance`'s height
  and build directly. **Chosen 2026-11-08** (question 2), with its skeleton also mapped to SOMA's 30
  joints; the files to download are listed in
  [`2026-11-08-character-assets.md`](../reference/findings/2026-11-08-character-assets.md).
- Starter clips: **Quaternius's Universal Animation Library**, CC0, "120+ Animations" on a humanoid
  rig, retargeted by role.

### 2.4 Cosmetic slots that are meshes

- `cosmetics.csv` gains `model` (a skinned glTF bound to the humanoid skeleton) and `covers` (the
  body regions it hides, so the skin under a sleeve is not drawn and never pokes through). The tint
  stays as the garment's base colour, so today's catalog keeps working.
- Garments follow the body's shape targets (the same targets transferred to the garment, or fitted
  through a proxy), so one jacket fits every build.
- Two **grip sockets** replace the single hands slot for props
  (`data/inventory/equipment_slots.json`); hand poses come from a small grip library (power grip,
  pinch, staff hold, two-handed axe) blended per finger.

## 3. Movement and actions

### 3.1 Layers, from the bottom

1. **Locomotion**: the root and the whole body while moving or standing.
2. **Actions** over it, masked to the upper body (wave, drink, swing a tool, carry).
3. **Interactions**: full-body sequences anchored to an object (3.3).
4. **Procedural**: IK (section 4), look-at, lean into turns, breathing, the body's state.
5. **Physics**: ragdoll, hit reactions, secondary motion (rung 8).

### 3.2 The locomotion set

Idle (with fidgets), walk, jog, run and sprint; starts, stops, turns on the spot and pivots;
strafing and backpedalling (the lower body follows travel, the upper body follows aim); **crouch**
(a new control and a lower capsule); stairs up and down and slopes; ladders (mount at the foot or
from the top through the hatch, climb, dismount); swim at the surface and under water; jump, fall
and land, with a hard landing whenever `falls.rs` says the landing hurt; carry heavy
(`carry_load.rs`); zero-g push, glide, grab and tether (the three primitives of
gravity-and-movement.md). **Styles from the body's state**: a leg wound limps (`wounds.rs`), cold
shivers (`body_heat.rs`), fatigue slumps, illness slows.

### 3.3 Interactions are anchors on things (the smart-primitive idea, as our data)

MotionBricks' "Smart Primitives" place target poses around an object, so sitting or vaulting snaps
together "like bricks". The same idea, written as our own data, with no neural network required:

- An **interaction anchor** is a row of `data/characters/interactions.ron`: the anchor's pose
  relative to its object, an approach (where to stand, which way to face), contact targets (hips on
  the seat, hands on the rungs, feet on the step), the clip or target poses of each phase (enter,
  loop, exit), whether the root is driven by the clip, and how long each phase may take.
- **Objects name their anchors**: furniture and beds, machines and stations (a new field on the
  machine definitions in `crates/humanity-ship/src/machines.rs`' data), structure kinds in
  `data/blueprints/structure_types.ron` (Ladder, Stairs, Elevator), doors (`door_anim`), plants for
  harvest, animals for milking and shearing (`livestock.rs`). A new chair is a new row, not code.
- **Motion warping** aligns the clip's contact frame with the real anchor: the root's travel and turn
  are stretched over the approach so the hips land on this chair, whatever its height. **Stride
  warping** fits stride length to speed; **orientation warping** turns the lower body to travel.
- **Gameplay never waits for animation.** The capsule and the existing systems stay the truth:
  reach (`moves.rs`), move checks (`move_check.rs`), footing. The animation follows them, and root
  motion moves the capsule only inside an interaction the gameplay system granted (sitting down,
  mounting a ladder).

## 4. Feet and hands on the world (IK)

- **Ground under each foot**: aboard, `footing.rs` already answers "where is the floor under a body"
  (treads, ramps, slabs, an elevator car); asked per foot. On a planet, the terrain sampler with the
  direction kept in f64 to the sampler (CLAUDE.md: "keep planet positions AND unit directions in f64
  (`DVec3`) all the way to a terrain or data sampler"), and its normal.
- **Two-bone analytic IK** per leg, the pelvis lowered by the larger drop, the foot turned to the
  surface with a toe pivot, and **foot locking** while a foot is in contact (contacts labelled
  automatically on each clip at load, or given by a generator's contact output), so feet never
  slide.
- **Footsteps planned on stairs**: the riser and tread come from the stair piece's own data, so each
  foot lands on a tread centre instead of hovering between treads.
- **Hands**: two-bone arm IK to grip sockets and ladder rungs (rung spacing from the ladder piece);
  both hands on a two-handed prop.
- **Look-at**: head, then eyes, toward the camera's aim (players) or a point of interest (crew),
  inside the turn limits of `turning.rs`.
- **Full-body IK** (an iterative solver over the whole chain) for reaching and props is a later
  rung; analytic two-bone solves cover rung 2.
- **Built for the feet on 2026-11-09 (rung 2a)**: section 15 says what was built and what it chose
  where this section left room (four sole points, the lock on the contact point, footsteps planned
  in the swing, the pelvis bounded by the legs' reach).

## 5. First and third person: one body

- **True first person**: the same body is drawn in first person with the head and neck hidden from
  the eye camera; you see your arms, legs, clothes and props, and your shadow is the whole body.
  The camera rides the head bone with stabilization (a low-pass on its translation; yaw and pitch
  stay the player's), replacing camera-system.md's sinusoidal head bob with real head motion.
- **Third person**: the same body from `camera.rs`'s follow camera.
- **The simplified and comfort options**: "Reduce motion" keeps the camera off the head bone; "Show
  my body in first person" can be turned off. Both in Settings, words by key.

## 6. Crew, crowds and creatures

- **Crew** use the humanoid. The timetable of crowd-simulation.md rung 3 says what each is doing;
  each activity names an interaction anchor (a workstation, a bunk, a mess table), so a crew member
  at work is the same data as a player using the same station.
- **Creatures** get a skeleton per body plan (quadruped small and large, bird, fish, and so on) as
  rows, and **gaits from real gait data**: footfall order, duty factor and stride against speed per
  species family (`data/characters/gaits.csv`, sourced like every other number). A procedural gait
  (a phase per leg, the swing foot placed by IK on the real ground) gives walk, trot, canter and
  gallop from those rows, with a small clip set for actions (graze, bite, lie down). Today's bite
  and flee behaviours (`ai/mod.rs`, `livestock.rs`) choose the action; the animation only shows it.
- **Open quadruped capture is scarce**: the dog data in AI4Animation is CC BY-NC (the MotionBricks
  finding, section 11). The
  2030 route that needs no licensed data is **gaits learned in simulation** (a physics character
  taught to walk by reinforcement, whose motion we own), baked into the same database the humanoid
  uses. That is rung 7.

## 7. The techniques, compared honestly

| | State machine + blend spaces | Motion matching | Learned motion matching | Learned inbetweening with target poses (MotionBricks class) |
|---|---|---|---|---|
| How | Authored states, each blending a few clips by speed and direction; authored transitions | Every few frames, search the whole capture database for the frame that best matches the current pose and the planned future path; jump there with a short blend (Clavet, GDC 2016) | Three small networks replace the database and the search (Holden et al., 2020) | A transformer and a decoder generate 0.6 to 1.9 s of motion from 4 recent poses and up to 4 target poses (NVIDIA, 2026) |
| Quality | Good with good clips; foot sliding and visible blends unless IK cleans up | "almost indistinguishable from a raw mocap sequence" (Clavet) | As motion matching | State of the art on its own benchmarks; artifacts out of distribution (its model card) |
| New behaviour costs | Clips plus states plus transitions, by hand | Capture plus markup; no transition authoring | Capture plus retraining | Target poses only, "with no retraining", within what the training data covered |
| Data it needs | Tens of clips | Minutes to hours of unstructured capture | The same as motion matching | Hundreds of hours: MotionBricks used about 700 |
| Memory | Small: the clips | Grows with the data (poses plus features) | Flat: the networks | Large: about 200 million parameters, 380 MB at FP16 |
| Cost per character | Microseconds on the CPU | A search a few times a second on the CPU | Microseconds on the CPU | One GPU generation per 0.6 to 1.9 s: 1.28 ms on an RTX 4090 at FP16 (NVIDIA's figure) |
| Runs on every GPU | Yes | Yes (CPU) | Yes (CPU) | Only through our own runtime; NVIDIA's needs TensorRT |
| Determinism | Yes | Yes | Yes | Depends on runtime and precision (its model card) |
| Licence and data for us | CC0 clips exist | CMU, ACCAD and 100STYLE usable | The same data | NVIDIA's weights: legal, not open source by the OSI's measure; our own: no open dataset near its size |
| Shipping today | Every engine | Industry standard for locomotion | In shipped games | Research; its own abstract says real-time control "remains dominated by traditional techniques" |

**What a 2030 release uses**, by this reading: motion matching (or its learned form) as the
locomotion backbone, with warping and IK, and generative models arriving for the long tail of
interactions. So the architecture is the **interface** (recent poses, target poses relative to
anchors, root constraints) with swappable solvers, and the build order is the solvers in the order
they pay off. Every solver serves the same anchors, so nothing is thrown away when a later solver
arrives.

## 8. Budgets

Measured numbers are marked as such; everything else is an **estimate to be measured** in the rung
that builds it, with `HUMANITY_FRAME_COSTS=1` and a rig vantage per case. Hardware: the operator's
RTX 4070 (about half an RTX 4090's memory bandwidth, 504 against 1,008 GB/s by vendor specification,
not re-read for this plan) and a modest non-NVIDIA GPU, the project's stated floor of an RX 580
(256 GB/s; [`engine-architecture.md`](engine-architecture.md): "Target: GTX 1060 / RX 580 minimum").
The interior already costs 18.84 ms of GPU at the homestead (crowd-simulation.md, measured
2026-09-15), so animation must fit beside it.

### 8.0 Measured in rung 1 (2026-11-09)

`HUMANITY_FRAME_COSTS=1`, the release build, the vantages `anim-*` (section 13), on the operator's
RTX 4070 at the rig's default graphics, 2560x1387, a background game held to 30 frames a second
(the stairs and ladder pairs shared the machine with other builds, flagged by the sweep; the rows
below are from clean captures unless marked). The body is the test figure (1,517 vertices, 24
joints): MakeHuman's base mesh is about nine times the vertices, which scales the skinning rows, not
the pose rows. Every `-boxes` twin ran in the same boot.

| Case | cpu.animation (all of it) | of which cpu.skinning (upload) | gpu.skinning (compute) | gpu.scene, boxes to bodies |
|---|---|---|---|---|
| One body (the chair; stairs and ladder alike, shared machine) | 0.08 ms | 0.05 ms | 0.004 ms | within run-to-run spread |
| 4 bodies in third person (the player and three stand-ins) | 0.10 ms | 0.07 ms | 0.004 ms | |
| 50 bodies 6 to 14 m away (16 near, 34 middle) | 0.32 ms | 0.18 ms | 0.006 ms | 6.85 to 6.99 ms (+0.14) |
| 50 bodies 18 to 26 m away (all middle) | 0.26 ms | 0.04 ms | none | 6.88 to 7.41 ms (+0.54) |

Earlier the same day, in low impact beside the operator's game (minimum graphics, 960x540, 15 frames
a second, his game sharing the card): 0.66 ms of cpu.animation at 50 near-and-middle bodies, gpu.scene
+0.13 ms, and the rest in proportion.

Against 8.1's estimates: the pose work is about 3 to 5 microseconds a body (estimate 2 to 5); compute
skinning of 16 near bodies 0.006 ms (estimate 0.2 to 0.4 ms, for a body nine times heavier: about
0.05 ms scaled); fifty middle bodies skinned in the vertex shader about 0.5 ms of scene time (estimate
0.5 to 1.5 ms for about 500, so per body it costs more than the estimate: each region is its own draw
through the whole surface fragment entry, six draws a body; rung 4's instanced crowd draw is the
lever). The upload at 50 bodies is seventeen small queue writes a frame (the palette and one
parameter block per near body); one parameter buffer with offsets is the cheap fix if it grows. A
first build gave the compute pass a submit of its own: 0.46 ms of CPU a frame for ONE body (low
impact), now ridden in the scene pass's encoder (0.05 ms).

**With the real bodies (2026-11-09)**, the same way: release build, rig defaults, 2560x1387, a
background game held to 30 frames a second, one boot (the sweep's machine guard found no build or
other game running; contaminated 0). Each case ran three times in that boot: the old boxes, the test
figure for everyone (showcase `figures` `test_figure`, the `-figure` vantages) and the MakeHuman
bodies, women and men by the crowd's hash. The test figure is 1,517 vertices and 24 joints; each
MakeHuman body 15,479 vertices, 56 joints and 15 shape targets.

| Case | cpu.animation, figure to MakeHuman | cpu.skinning | gpu.skinning | gpu.scene: boxes, figure, MakeHuman | all GPU stages, figure to MakeHuman |
|---|---|---|---|---|---|
| One body (the chair) | 0.064 to 0.056 ms | 0.041 to 0.035 | 0.004 to 0.006 | 8.45, 8.44, 8.77 ms | 14.42 to 14.44 |
| 50 bodies 6 to 14 m (16 near, 34 middle) | 0.372 to 0.523 ms | 0.221 to 0.251 | 0.006 to 0.024 | 7.52, 7.33, 7.76 ms | 13.71 to 14.00 |
| 50 bodies 18 to 26 m (all middle) | 0.155 to 0.241 ms | 0.027 to 0.034 | none | 6.95, 7.05, 7.15 ms | 13.20 to 13.30 |
| The player alone in third person (idle, walk, run; female and male) | 0.056 to 0.073 ms (MakeHuman) | 0.035 to 0.046 | 0.006 to 0.007 | | |

So: the pose work is about 10 microseconds a MakeHuman body against 7 for the test figure (56
joints against 24, ten regions against six, and the morph block); compute skinning of 16 near
MakeHuman bodies is 0.024 ms (8.1's estimate was 0.2 to 0.4 ms); the scene costs about 0.3 to 0.4 ms
more for 16 near and 34 middle bodies ten times heavier, and 0.1 ms more for 50 middle ones. One body
is within the run-to-run spread (the scene read 0.33 ms more, the GPU as a whole 0.02). Memory: each
MakeHuman body holds 8.11 MB on the GPU (0.74 MB of vertices, 7.4 MB of shape targets, the indices),
the test figure 0.10 MB; a near place's skinned output is 0.50 MB (16 near: 7.9 MB per body that fills
them); a body's palette is 2.8 KB a frame (its model matrix, the six-row morph block and 56 joints).
On disk, in git: 4.3 MB a body (8.6 MB for both), most of it the shape targets; the clips 0.7 MB.

**With the body sliders (2026-11-09)**, the same way, each pair in one boot: the crowd with varied
shapes (every stand-in its own build, proportions, ancestry and height, up to seven shape targets on,
section 14's sliders) against the same crowd drawn as the body as made (showcase `varied_shapes` 0:
no target on, which is what main drew), vantages `anim-crowd-near` and `anim-crowd-middle` and their
`-plain` twins. Two boots were clean of other builds for the near pair, one for the middle pair:

| Case | cpu.animation, plain to varied | cpu.skinning | gpu.skinning | gpu.scene | all GPU stages |
|---|---|---|---|---|---|
| 50 bodies 6 to 14 m (16 near, 34 middle), boot 1 | 0.410 to 0.536 ms | 0.185 to 0.213 | 0.023 to 0.083 | 7.30 to 7.92 ms | 13.32 to 14.17 |
| the same, boot 2 | 0.735 to 0.513 ms | 0.388 to 0.209 | 0.023 to 0.083 | 7.57 to 7.86 ms | 13.79 to 14.37 |
| 50 bodies 18 to 26 m (all middle), boot 2 | 0.266 to 0.327 ms | 0.036 to 0.036 | none | 7.18 to 7.36 ms | 13.35 to 13.59 |

So: compute skinning of 16 near bodies costs 0.06 ms more with their targets on (about 4 microseconds
a body, both boots alike); the scene, which skins the middle bodies in the vertex shader, 0.2 to 0.6
ms more for 34 middle bodies and 0.2 ms more for 50 (the spread between boots is as large); the CPU
side is inside its own run-to-run spread (the pose work is the same; working out a shape is a few
lookups and a hash a person, and a reshaped skeleton is kept while the shape stays). At section 8.4's
500 middle bodies the scene's share would be a few milliseconds, so rung 4's instanced crowd draw
should take only each middle body's largest two or three targets (or one shape a group shares, a
reuse that obeys the switch); near bodies can keep all seven.

**With feet on the world (rung 2a, 2026-11-09)**, the same way: release build, rig defaults,
2560x1387, a background game held to 30 frames a second, each pair in one boot (the feet against the
Dev page's Feet on the world off, showcase `feet` 0, which is the pose main drew), vantages
`anim-crowd-near` and `anim-crowd-middle` and their `-feet-off` twins (section 15). The feet are
CPU work, so only cpu.animation moves:

| Case | cpu.animation, feet off to on | cpu.skinning | gpu.skinning | gpu.scene | all GPU stages |
|---|---|---|---|---|---|
| 50 bodies 6 to 14 m (16 near with the full solve, 34 middle with the pelvis offset), sweep 3, clean | 0.504 to 0.714 ms | 0.182 to 0.188 | 0.083 to 0.082 | 7.47 to 7.47 ms | 13.32 to 13.26 |
| the same, sweep 1 (before the swing's own ground questions), clean | 0.486 to 0.685 ms | 0.183 to 0.186 | 0.081 to 0.081 | 7.49 to 7.48 | 13.68 to 13.52 |
| 50 bodies 18 to 26 m (all middle, the pelvis offset), sweep 3, clean | 0.341 to 0.424 ms | 0.032 to 0.033 | none | 7.17 to 7.10 | 12.94 to 12.80 |

So: the full solve costs about 10 to 12 microseconds a near body (0.2 ms for 16: sweep 3's 0.21 ms
less 34 pelvis offsets is 9.5 microseconds a body), against 8.1's estimate of 1 to 3: each body asks
the ground 10 to 17 times a frame (two sole samples and two slope samples a foot, the landing
projection while a foot comes down, and a swinging foot's back of the heel, toe cap and the ground
ahead of it), each a walk over every zone's slabs and
pieces in `Footing::floor_rising`, and each leg's solve rebuilds the 56 model matrices three times
(`feet::set_model_rotation`). Both are levers if the near tier grows: a per-frame list of the pieces
near each body, and refreshing only a leg's own joints. The pelvis offset costs about 1.7
microseconds a middle body (0.083 ms for 50). The automatic budget (`full_budget_ms`, 1 ms, section
15) therefore buys all 16 near bodies here with room to spare. GPU and skinning are unchanged within the run-to-run spread. The single
bodies of `anim-stairs`, `anim-ramp`, `anim-crouch` and `anim-slope-map` are not cost readings: their
foot probe (`foot_probe`), CPU skinning of the feet and a ground question per foot vertex, runs inside
the same cpu.animation stage while the costs are gathered.

**Why 56 joints and not MakeHuman's 163**: the 56 are exactly the bones that move in rung 1 (the role
map's); the other 107 would only ever sit at rest under their parents until face and twist rungs.
From the rows above, pose work grows about 0.1 microsecond a joint, so the full rig would cost about
20 microseconds a body (1 ms of CPU at 50 bodies, against 0.52) and 7.8 KB of palette a body a frame
(3.9 MB a frame at section 8.4's 500 middle bodies, against 1.4 MB), for nothing that moves. A face
rung or twist joints are a role-map line and a converter run away.

### 8.1 Per frame, the operator's RTX 4070 (estimates)

| What | Count | CPU | GPU |
|---|---|---|---|
| Pose evaluation, state machine (sample 2 to 4 clips, blend) | 1 player, 12 players, 50 crew near | 2 to 5 us each, 0.1 to 0.3 ms | none |
| Motion matching search (accelerated, 5 to 10 a second each) | the same 63 | 0.3 to 1 ms in all, spread over worker threads | none |
| Foot and hand IK (4 two-bone solves, 2 to 4 ground queries) | 63 | 1 to 3 us each, 0.1 to 0.2 ms | none |
| Pose palette upload (60 joints x 48 B) | 63 | negligible | 0.18 MB a frame |
| Compute skinning, near bodies | 16 near at full detail | none | 0.2 to 0.4 ms |
| Vertex-shader skinning, middle bodies | up to about 500 | none | 0.5 to 1.5 ms, with the raster |
| Far tier, baked animation, instanced | thousands | none | 0.2 to 0.5 ms |
| **Learned inbetweener, if ever (rung 7)** | 32 characters regenerating about once a second | input preparation | about 2 to 3 ms for a batch of 8 (NVIDIA measured 1.54 ms on a 4090; a 4070 has about half the bandwidth), one batch every few frames: about 0.5 ms a frame on average with spikes, unless it runs alongside on a separate queue |

### 8.2 Per frame, a modest non-NVIDIA GPU (RX 580 class, estimates)

- CPU work is the same as 8.1 (it scales with the processor, not the GPU).
- Skinning and raster cost roughly 3 to 5 times the 4070's, so the near tier drops to 4 to 8 full
  bodies and the middle tier's count follows the resource budget pie
  ([`resource-budgets.md`](resource-budgets.md)).
- **A MotionBricks-sized model is not viable per frame here.** Reading its 380 MB of FP16 weights
  once takes at least 1.5 ms at 256 GB/s and a generation reads them several times, without tensor
  cores: about 10 to 30 ms a generation in our own compute kernels. It could animate a handful of
  characters at most, off the frame. On a CPU through ONNX Runtime a generation would take a large
  fraction of a second (estimate). This is the strongest technical reason the learned rung is last
  and optional.

### 8.3 Memory

- Pose palette: 2.9 KB a character; 500 characters double-buffered about 2.9 MB.
- Transient skinned vertices for near bodies: about 1 MB each at full detail, so the near tier is
  capped (16 on the 4070 by default, fewer on the floor).
- Motion data: an hour at 30 Hz with 60 joints is about 40 MB with quantized rotations, a few MB
  after curve compression; motion matching's features add about 12 MB an hour. Two to four hours:
  roughly 30 to 100 MB of RAM. Learned motion matching: under 10 MB of networks.
- A MotionBricks-class model: 380 MB of VRAM at FP16 (758 MB at FP32) plus activations, and the same
  again as a download (the music's core set, for scale, is 6.3 MiB in the app).

### 8.4 Infinite-of-x: hundreds near, many more far

| Tier | Where | How many | Skeleton | Solver | Skinning |
|---|---|---|---|---|---|
| Near | to about 15 m | 16 (budgeted) | full, with fingers | motion matching, warping, full IK | compute, once a frame |
| Middle | 15 to 40 m | hundreds | about 25 joints | state machine or matching at a lower rate, poses evaluated at 15 Hz and interpolated, pelvis-only ground offset | vertex shader, instanced with a palette offset |
| Far | 40 to about 150 m | thousands | none drawn | a few gait cycles baked per body type into an animation texture at load, a phase per instance | none (texture lookup), one draw per body type |
| Beyond | | billions | | crowd-simulation.md tiers 1 and 2, nothing drawn | |

The 40 m line is where `CREW_NAME_DIST` already retires the nameplate (crowd-simulation.md), so the
two culls agree. Crew in the same gait at middle range may share a pose with a phase offset (a reuse
that obeys the switch, 8.7).

### 8.5 What crosses the network

- **Never poses.** Each client animates every body it draws from states and targets. A streamed
  pose (30 joints at 6 bytes, 30 times a second) is about 5.4 KB/s per character per viewer, so 12
  players in view would cost each viewer more than the whole position stream did before interest
  management (about 45 KB/s a socket at 12 players, `game_interest.rs`).
- **Added to the existing 15 Hz position update**: a stance byte (stand, crouch, sit, climb, swim,
  fly, zero-g), the look pitch in a byte (yaw already goes), and, **only when they change**, the
  interaction (anchor id, object instance id, start time), what each hand holds, and an emote id. A
  few bytes on most updates.
- **Speed and direction are not sent**: they come from the interpolated positions, which feed the
  same locomotion solver the local player uses.
- **Crew send nothing per frame**: their activity is derived from the timetable (crowd-simulation.md
  rung 3); only deviations sync.
- **Animation is cosmetic and needs no determinism**: two clients may place a foot a centimetre
  apart. Anything gameplay depends on (reach, hits, collisions) reads the capsule, never the pose.

### 8.6 The dual-mode rule

- **Simplified**: the state machine with blend spaces, no finger animation, a smaller near tier,
  and for the feet the **pelvis-only ground offset** (decided at rung 2a, 2026-11-09: every body is
  raised or lowered as a whole onto the ground under its planted feet, one ground query a foot, no
  leg IK and no locking), rather than no IK at all, because the offset alone removes the worst of
  rung 1's sinking and floating (a whole body on a stair line or between treads) for almost no
  cost. Works everywhere and is the automatic choice where the budget pie says so (for the feet
  since rung 2a: the full solve's own per-frame budget, `full_budget_ms`, section 15). Settings >
  Gameplay > Animation chooses it; Realistic is the default (question 5).
- **Full realism**: motion matching with warping and inertialized transitions, full IK, styles from
  the body's state, secondary motion, and the learned rung where present. For the feet since rung
  2a: the near tier's full solve (two-bone leg IK, the foot turned to the slope about a toe pivot,
  foot locking, stair footsteps), the middle tier's pelvis offset (8.4).
- **Gameplay is identical in both** (the capsule is the truth). A "Reduce motion" comfort option
  stands apart from both (section 5).

### 8.7 The no-caches rule

- **TensorRT's plan cache** (`.trt-cache/`, MotionBricks' README: "Plans are cached in
  `.trt-cache/`, so later launches start quickly") is a cache kept across runs and forbidden in
  development. We do not use TensorRT at all: it is also proprietary and NVIDIA-only. Any learned
  model would run in our own wgpu compute pipelines, built at startup like every other pipeline, with
  no pipeline cache on disk; the cold path is made fast instead.
- **Retargeted clips, motion matching's acceleration structure and the far tier's baked animation
  textures** are built in memory at load from the source files and never written to disk. Each is
  tagged with the version of the files it came from and rebuilt when they change (hot reload).
- **Every in-memory reuse obeys the one switch** (`humanity_core::reuse::enabled()`,
  `HUMANITY_NO_REUSE=1`, the Dev page's "Turn off all reuse"). With it off: motion matching searches
  the whole database every query (no acceleration structure), the middle tier evaluates every pose
  every frame and shares none, and the far tier draws middle-tier skeletons instead of baked
  textures. Slower, and it rules stale data in or out with one click.

## 9. Data files and where the code lives

- **Data (infinite-of-x)**: `data/characters/skeletons/*.ron`, `data/characters/bodies/*.ron` (mesh,
  skeleton, which shape targets the look drives), `data/characters/clips.ron` (each clip's file,
  tags, loop flag, and its credit id in `data/credits.ron`), `data/characters/interactions.ron`,
  `data/characters/gaits.csv`, a humanoid row in `data/lod/categories.ron`, `model` and `covers` in
  `cosmetics.csv`, `skeleton` in `creatures.csv`; schemas beside them in `schemas/`. No list of
  clips, states or anchors in code. Every motion source's licence goes in `data/credits.ron` and
  `LICENSES.md` in the same commit (CC BY sources need their credit shown).
- **Code**: the pure part (skeletons, poses, clip sampling, blending, retargeting, motion matching,
  IK) in a crate of its own, `humanity-animation`, using only `core`, like `humanity-music`: the
  relay never compiles it, because animation never reaches the server. Ground queries reach it
  through a small trait the engine implements over `footing` and the terrain sampler. Skinning
  pipelines in the renderer; wiring in `src/engine/`. A crate is an addition to the crate split's
  plan ([`crate-split.md`](crate-split.md)); if it is not wanted, the same code sits in `core`.
- **GUI-first**: Settings gains the animation mode, the first-person body and "Reduce motion"; the
  Dev page gains an Animation section (skeleton overlay through the existing line primitive, the
  matcher's candidates and costs, contacts, IK targets, LOD tier colours, clip and anchor
  inspectors). Words by key, each control with its hint.
- **Web parity**: the website draws no 3D world, so the game's animation has no web twin. The one
  shared surface is the model viewer ([`model-viewer.md`](model-viewer.md)): once the desktop's
  loader reads skins, both viewers can play a file's own animations from that one spec, the
  desktop first and the web mirroring it.

## 10. The rungs, in order

Each rung ships on its own, is measured, and is verified by booting the release exe and entering the
world (`just verify-runtime`), plus the rig vantage it adds.

0. **Measure first**: a rig vantage per case (a figure walking stairs, a ladder, a chair, a crowd of
   N) on today's figures, so every later rung has a baseline.
1. **The skinned body, for everyone.** glTF skins in the loader, the skinned vertex, the pose
   palette, compute and vertex-shader skinning; the MakeHuman CC0 body with the humanoid skeleton;
   Quaternius CC0 clips retargeted by role; a state machine with idle, walk, run and turn blend
   spaces; drawn for the player in third person, for other players (speed from their positions, no
   network change) and for crew. Cosmetics as tinted body regions, then as meshes. Unblocks the fire
   props' body arc and makes co-presence read as people.
2. **Feet, hands and things.** Foot IK on decks, treads and terrain with foot locking; crouch;
   interaction anchors as data with motion warping for sit, bed, ladder (with hatch), station and
   door; two grip sockets; true first person; the network's stance, pitch and interaction fields.
   **2a BUILT 2026-11-09 (section 15)**: the feet on decks, treads, ramps and terrain with foot
   locking, crouch, jump and the network's stance. The interaction anchors, the grips, hands on
   rungs, true first person, pitch and the interaction field are 2b.
3. **Motion matching**, the full-realism core: the database built at load from CMU, ACCAD and
   100STYLE (credited), retargeted; trajectory prediction from the controller; inertialized
   transitions (Bollo, GDC 2018); contact labels; styles from the body's state. The state machine
   stays as the simplified mode.
4. **Crowds**: the middle and far tiers, instanced skinning, baked animation textures at load, crew
   activities from the timetable (after crowd-simulation.md rungs 3 to 5).
5. **Creatures**: skeletons per body plan, gaits from real data, procedural legs with IK, action
   clips.
6. **Learned motion matching** once the database passes an hour or two, to hold memory flat.
7. **The learned rung**: a MotionBricks-class inbetweener for interactions the database does not
   cover, and gaits learned in simulation for creatures. Only if it beats rung 3 in a blind side by
   side on the canonical vantages, within budget, on both reference GPUs. Our own model trained on the
   same open data, run in our own wgpu kernels; NVIDIA's weights only if the operator allows them
   (question 1), as an optional pack.
8. **Physics and secondary motion**: ragdoll on death and hard falls (Rapier's joints), hit
   reactions, spring bones for hair and loose clothes, cloth later; all client-local, never networked.
9. **Faces and VR**: blinks, gaze, lip movement from voice; full body from a headset and two hands
   through the same IK ([`vr-tracking.md`](vr-tracking.md)).

## 11. Recommendation per rung

- **Rung 0 and 1: now.** They are the operator's "body first" (fire props, 2026-09-24), they make
  co-presence and crew read as people, and they need no licensed data (CC0 body and clips).
- **Rung 2: right after.** Interactions as data are the part of MotionBricks worth having, and they
  need no network model.
- **Rung 3: the core.** Motion matching is the technique shipping games use, works on every GPU from
  the CPU, and the open data is enough for it.
- **Rungs 4 and 5: when the crowd and creature arcs reach them.**
- **Rung 6: only when memory says so.**
- **Rung 7: last, gated by a measured comparison.** Do not port MotionBricks' Python runtime: its
  value is the model, and its runtime is TensorRT. Adopt its interface from rung 2 on, so a learned
  solver can slot in later without changing anything around it.

## 12. Questions only the operator can answer

**All seven DECIDED by the operator on 2026-11-08** ("Let's do this so players get a good player
model and animations."; rungs 0 to 2 now; "Agreed" to the rest). The questions as asked, each with
its decision:

1. **May NVIDIA's MotionBricks weights ever ship or be fetched, as an optional pack, given they are
   legally usable but not under an OSI-approved licence and come with no training code or data?**
   Your rule: "We want to keep everything on open-source." *Recommendation: no for the core game;
   decide at rung 7 with measurements in hand, and only as an opt-in pack showing its licence.*
   **Decided 2026-11-08:** never in the core game; later, possibly, an optional upgrade pack, like
   the star catalogue download.
2. **Which body to build on?** MakeHuman's CC0 assets, a body made by a contributor under CC0, or one
   made from scratch. *Recommendation: MakeHuman's, for its CC0 status and real shape controls.*
   **Decided 2026-11-08:** MakeHuman (CC0), with its skeleton ALSO mapped to SOMA's 30-joint layout,
   so a learned model could drive it later ("That sounds reasonable. Save us a lot of effort instead
   of doing it ourselves."). Everything stays body-agnostic all the same (the body is an asset
   choice, not a code choice): `data/characters/role_maps/makehuman_default.ron`,
   `role_maps/soma_30.ron` and `joint_maps/makehuman_to_soma30.ron` (section 2.1). The bottom quality
   tier is lower-detail versions of the SAME MakeHuman body (decimated LODs, then section 8.4's baked
   far tier), not Quaternius's low-poly characters, so there is no visible change of style between
   tiers (the operator asked; the coordinator's answer, 2026-11-08).
3. **Accept motion capture from the community under CC0**, the way music contributions are accepted,
   to grow the database beyond CMU, ACCAD and 100STYLE? *Recommendation: yes, with the same review
   of what each contributor used.* **Decided 2026-11-08:** yes, reviewed like music contributions.
   The motion data plan was approved with it: Quaternius for rung 1; 100STYLE as the core of motion
   matching, CMU for breadth and ACCAD for selected actions, all credited; community CC0 captures
   later.
4. **See your own body in first person by default?** *Recommendation: yes, with a Settings toggle
   and "Reduce motion" beside it.* **Decided 2026-11-08:** yes, with a Settings switch and a
   "Reduce motion" option (rung 2).
5. **Default animation mode?** *Recommendation: full realism where the measured budget allows,
   simplified automatically below it; gameplay is the same either way.* **Decided 2026-11-08:** as
   recommended.
6. **Order against the other arcs**: crowd-simulation.md puts animation last, the fire props want the
   body first. *Recommendation: rungs 0 to 2 now (they serve the player, co-presence and props),
   rung 4 after the crowd's timetable and navigation.* **Decided 2026-11-08:** rungs 0 to 2 now, the
   crowd tiers later.
7. **A new crate, `humanity-animation`**, outside the crate split's planned ten? *Recommendation:
   yes; the relay never needs animation, and a pure crate keeps it testable without a GPU.*
   **Decided 2026-11-08:** yes. Built (docs/crates/humanity-animation.md).

## 13. Built: rungs 0 and 1 (2026-11-08)

What rung 1 built, against sections 2 to 10, and what it chose where the plan left room.

- **The body is data, and body-agnostic** (question 2, decided): `data/characters/characters.ron`
  names the body, the graph, the clip bake rate, the tiers and each wearer kind's default clothes;
  a body is a file in `bodies/`, its rig a role map in `role_maps/`. Until the downloads, the body is
  the procedural test figure (`scripts/gen-test-figure.js`: an A-pose jointed figure with
  Blender-style bone names, forearm twist joints and six materials) and the clips are a procedural
  clip library on a deliberately different rig (T-pose, Mixamo-style names, rolled bone frames, a
  0.01 armature), so retargeting is proven on the shipped files: the library's walk retargeted onto
  the figure lands on the same walk authored on the figure within a degree and a half.
- **The skeleton standard (2.1)**: `skeletons/humanoid.ron` holds every VRM 1.0 humanoid role (its
  required set exactly VRM's), eleven of ours (six twist joints; the second neck joint and four
  fingertips of SOMA's 30), each role's parent, tier, aim and alignment, and the six sockets rung 2's
  grips need. Role maps are written for the test rigs, MakeHuman's default skeleton
  (`makehuman_default`) and SOMA's 30 joints (`soma_30`); `joint_maps/makehuman_to_soma30.ron`
  writes MakeHuman to SOMA out joint for joint, held to the two role maps by a test.
- **Retargeting (2.1)** is in model space with the rest poses aligned per limb (`align`: the shortest
  turn from the body's rest bone onto the source's), the root's travel scaled by the ratio of hips
  heights; joints the source does not map keep their rest. Twist joints take a share of their
  driver's twist at run time (swing-twist split), so they work for any pose source.
- **glTF skins (2.2)**: `src/assets/skin.rs` reads the first skin, every primitive drawn with it,
  JOINTS_0 and WEIGHTS_0 (renormalised to sum exactly 65535), the node tree (a non-joint armature's
  matrix kept as the root's `outer`), inverse binds and every animation (Linear, Step, CubicSpline).
  Morph targets (body shape) come with the MakeHuman body.
- **The GPU (2.2)**: a `SkinnedVertex` (48 bytes) read two ways from one buffer; one pose palette
  per frame (three `vec4` rows a matrix, the body's model matrix first); near bodies compute-skinned
  once a frame into ordinary meshes (so the shadow and every pass draw them unchanged), middle bodies
  skinned in the vertex shader by a PSO of their own (the megashader plus `vs_skinned`, the surface
  class's entry and permutation), built on a background thread so the boot is not longer; until it
  is ready, middle bodies are compute-skinned too. Middle bodies cast no sun shadow yet (rung 4).
  Bodies use material type 25: plain PBR, no panel grid.
- **The tiers (8.4)** live in `characters.ron` (near 15 m and 16 bodies, middle 40 m), not in
  `data/lod/categories.ron` as section 9 suggested: that file's rows drive Settings sliders by stage,
  and the character tiers are not its stages. Past the middle distance a body is drawn as a middle
  one until rung 4's far tier.
- **The state machine (rung 1)**: `locomotion.ron`, states idle, turn (turn right, idle, turn left
  over the turn rate) and move (walk and run over the speed, one stride phase, played at the body's
  speed within 0.55 to 1.6 times, backwards for a backwards walk), crossfades by data. Inputs are read
  from positions by one motion tracker for every body (teleports reset it).
- **Who is drawn**: the player in third person, other players (from their interpolated positions,
  nothing new sent), the crew, the showroom figure (the look and outfit live) and the dev crowd.
  **Third person now walks the body**: it runs first person's walk, floor, walls, ladders and lifts
  and stands the camera behind (it had flown a camera with no figure and no collision since the M1
  review, so there was no body to draw). Interactions (E and F) stay first person's until rung 2.
- **Cosmetics as tinted regions (2.4's first half)**: each body material takes the person's skin,
  hair or a cosmetic slot's colour, else the wearer kind's default (players keep the teal top that
  said "a player", crew the amber). Garments as meshes come with the real body.
- **Rung 0's baseline** is the same scenes with the old box figures, a same-boot A/B (showcase
  `figures`), in the vantages `anim-stairs`, `anim-ladder`, `anim-chair`, `anim-crowd-near`,
  `anim-crowd-middle` and their `-boxes` twins; `anim-tp-move` and `anim-tp-gaits` show the player's
  body in third person and the gaits.
- **Walk speed, decided** (operator, 2026-11-09: "These all sound reasonable. LEt's proceed."): the realistic mode walks at 1.4 m/s
  and runs at about 4 m/s; the simplified mode keeps today's 5 m/s walk and 9.5 m/s sprint. Built
  the same day as Settings' walking pace (section 14), which answers the walk-speed limit below.
- **Known limits the rigs showed**: the game walks at 5 m/s (`WALK_SPEED_MPS`, a jog) and sprints at
  9.5, so the player's own body plays the run clip (at 1.4 times its speed) when walking, and the
  sprint runs past the clip's 1.6 times limit, so the feet slide; real walking is about 1.4 m/s (a
  gameplay speed question for the operator). The third-person camera has no collision yet: in the
  homestead's narrow hall, three metres behind the body, a door lintel can cut across the view
  (camera-system.md's "pulls forward when blocked" is the fix). Feet sink up to 4.5 cm into the floor
  over a cycle and float over stair treads: rung 2's foot IK (fixed in rung 2a, section 15).
- **What rung 2 starts from**: the per-figure `Animator` and `MotionTracker` (add stance and the
  interaction inputs), `humanoid.ron`'s sockets and aims, the role-indexed skeleton (two-bone IK
  needs the upper, lower and end joints by role, which `Skeleton::joint_by_role` gives), the
  third-person walk (first person's body draw is the same `Person` with the head regions hidden), and
  the network's 15 Hz position update, untouched so far.

## 14. Built: the real bodies, the clips and the walking pace (2026-11-09)

The rest of rung 1, after the operator's yes to both downloads (2026-11-09: "Yes. We need at least a
male and a female." and "Yes, let's see what it has.") and his decision on walking speed.

- **The bodies (2.3)**: `scripts/gen-makehuman-body.js`, our own converter (node, like
  `gen-test-figure.js`: no compile, runs in `just rig-tests`, byte-identical on a rebuild), reads
  MakeHuman's CC0 data from `E:\HumanityOS\makehuman-src\a8bc2d54\` (every input checked against its
  git blob hash) and writes `makehuman_female` (1.59 m) and `makehuman_male` (1.73 m) into
  `assets/models/characters/makehuman/` with `SOURCE.md`. None of MakeHuman's AGPL code is used: a
  body is the base mesh plus a third of each ancestry target for its gender (MakeHuman's default
  mix; its average-build universal targets are empty), a joint is the mean of its vertex group, and
  the weights are merged by our own rule. Without the sources (CI) `--check` compares the files with
  `SOURCE.md`'s hashes and says so; `--preview DIR` renders front, side, back and head views to PNG,
  which is how the regions were tuned without a boot.
- **56 joints, not 163** (the plan's "about 60 at full detail"): exactly the bones
  `role_maps/makehuman_default.ron` names, so the role map is the keep list. Every dropped bone goes to
  its nearest kept ancestor: the in-chain and twist segments (`spine04`, `upperarm02` and the like,
  which rung 1 left at rest under their parent anyway, so nothing that moves changed), the
  metacarpals, the breasts and the face (rung 9); the four smaller toes go to the kept middle toe so
  all five bend together. Every rest rotation is the identity (the retargeter works in model space).
  What it saves is measured in 8.0.
- **Dressed as regions (2.4's first half)**: the dominant bone of each face picks skin, a long-sleeved
  shirt, trousers, shoes or hands, with straight cuts for the waist, collar and hairline; the shirt is
  smoothed (Taubin) so it reads as cloth over the body; the scalp is hair-coloured with a 6 mm shell
  that thins to its edge, for the volume of short hair; the brows are generated tapered strips laid on
  the skin; the eyes are generated spheres (sclera, iris in the look's eye colour, pupil) where
  MakeHuman's eye helpers sit, on the eye bones. MakeHuman's helper geometry (tights, skirt, hair and
  genital helpers, teeth, joint cubes) is not drawn. Garments as meshes, and real hair, need
  MakeHuman's clothes and hair assets (CC0 too): another download, so the operator's call.
- **Shape targets on the GPU (2.2)**: 15 per body as glTF morph targets (height, proportions, the
  muscle-and-weight grid's edges and corners, the three ancestries as differences from the default
  mix), each with its joints' movement as an accessor `joint_deltas:<target>`. Each person's active
  targets ride in the pose palette's morph block (`pose::MORPH_SLOTS` matrices between the model
  matrix and the joints); both skinning shaders add them before skinning, so one body on the GPU
  draws every shape. The look's height drives the height targets (data: the body file's `shape`),
  the rest as a uniform scale, so a tall person is long in the leg, not scaled up head and all; the
  skeleton follows the shaped mesh (`Skeleton::reshaped`: joints moved by the targets, inverse binds
  made again), kept per person while their shape stays the same (a value keyed by its own inputs,
  made again every frame with the reuse switch off and on every library reload). MakeHuman's ground
  helper never moves under these targets (the converter checks), so feet stay on the floor.
- **The bodies are chosen by the look**: `characters.ron` lists `bodies` (female, male) and the
  `fallback` (the test figure, drawn for a body that does not load); `Appearance::body_type` picks
  one (the character page's new chooser, words by key), and travels to other players in the look
  (`PlayerLook::body`, a plain id or nothing, clamped like the rest); a person whose look names none,
  and every crew member, takes one of the wearer kind's bodies by a stable hash, so a crew is women
  and men.
- **The clips (2.3)**: `scripts/gen-quaternius-clips.js` trims the twelve clips used (idle, walk,
  jog, sprint, crouch idle and walk, jump start, air and land, sit down, idle and up) out of the
  library's in-place file, keys unchanged, with Quaternius's `License.txt` byte for byte, into
  `assets/models/characters/quaternius_ual/`; `role_maps/quaternius_ual.ron` maps its 65-joint rig.
  Each gait's speed is measured from its feet (the median backward speed of a planted foot): walk
  0.979, jog 5.898, sprint 8.905, crouch walk 0.717 m/s; the root-motion file agrees on the walk and
  is 9% slower on the runs. Retargeting scales a gait's speed by the body's legs, and the engine
  divides a person's speed by their own stride scale before the graph reads it, so the feet keep
  still at every size. Turning on the spot stays our procedural clip (the free tier has none).
  Crouch, jump and sit are retargeted and wait for rung 2's inputs.
- **Two retargeting fixes the real rigs found**: a finger now rides its hand's rest alignment (a
  T-pose clip's finger turns on an A-pose hand bent every finger about 45 degrees against it), and
  the feet are no longer aligned (every rig stands its feet flat at rest, whichever way its foot
  bone points; aligning tilted MakeHuman's foot onto Quaternius's ankle-to-ball bone and sank the
  idle 2.6 cm, 0.5 without).
- **The walking pace (the operator, 2026-11-09)**: Settings > Gameplay > Walking pace, Simplified by
  default (the house rule): Realistic walks at 1.4 m/s and runs at 4, Simplified keeps 5 and 9.5.
  No mode setting for movement existed, so this is the smallest honest one: `settings.walk_realistic`
  in AppConfig (an old settings file reads as Simplified), words and hints by key, the speeds four
  rows of `data/game.csv` (hot reloaded), applied to the controller's walk speed and a new sprint
  factor (`engine::walk_pace`). The locomotion graph places the walk at 1.6 m/s, the jog at 4 and the
  sprint at 8.9, so the Realistic walk is the walk alone (played 1.4 times, the library walks
  slowly), the Realistic run the jog alone slowed, the Simplified walk mostly the jog and the sprint
  the sprint.

### Answers after rung 1 (operator, 2026-11-09)

Rung 1's agent left three questions; his answers, verbatim:

1. **Keep both bodies with all 15 shape targets (8.6 MB in git), or height only?** "We don't need
   to be so careful with space usage. This stuff sounds reasonable. I'm sure we'll end up having
   larger ones as we get closer to v1.0". Kept whole (and a standing rule in CLAUDE.md since).
2. **Real clothes and hair from MakeHuman's CC0 assets, after foot placement (rung 2)?** "Yeah,
   might as well include these." The source is MakeHuman's system asset pack,
   `makehuman_system_assets_cc0.zip` (280,737,770 bytes, Last-Modified 2024-04-14), listed on
   makehumancommunity.org's asset pack page as CC0 (hair, eyebrows, eyelashes, eyes, teeth, skins,
   proxies, and clothes: casual, elegant, sport and work suits, shoes, two fedoras). Its first mirror
   (`files2.makehumancommunity.org`) disallows every robot in its robots.txt, so it is fetched from the
   second (`files.makehumancommunity.org`, no robots.txt), once the download itself is approved.
3. **Build and ancestry sliders on the character page?** "Yeah, let's go for it." The 15 targets
   already ship; the sliders drive them through the look, like height.

### Built after rung 1: the body sliders (2026-11-09)

Answer 3, built: the character page shapes the body with sliders for build, proportions and
ancestry, every other player sees the shape you chose, and a crew is no longer one body copied.

- **Every slider is a row of the body file** (`data/characters/bodies/makehuman_*.ron`,
  `shape.sliders`; read by `library.rs`): a `Grid` (two sliders spanning a square of targets: muscle
  and weight, MakeHuman's grid, a target at each edge and corner, weighted bilinearly), a `Line` (one
  slider, a target at each end: proportions) and a `Blend` (shares of one whole, each part's target
  its difference from the even mix: the three ancestries). Each row names its group (the page's
  heading) and its slider ids, which are both the words' keys (`characters.shape_<id>`) and the
  look's keys. A new slider is a new row and its words; nothing in code names one. The loader
  refuses a target the mesh lacks, an id a look could not carry, an id named twice, and a body whose
  rows could turn on more targets at once than the palette holds.
- **Values live in the look**: `Appearance::shape`, slider id to a value from 0 to 1, saved with the
  character (`#[serde(default)]`: a save from before the sliders loads with the body as made). A
  slider the look does not name sits in its middle, a blend's parts at an even share, which is the
  body as made, so an empty map costs nothing and Reset simply empties it (and puts the height back
  to 1.0).
- **The person's height stays theirs.** MakeHuman's ancestry targets move the top of the head
  (measured on the female body at full weight: African +7.9 cm, Asian -9.4 cm, Caucasian +1.5 cm;
  proportions' ideal end +1.0 cm; the build targets 0 to 0.5 cm), so `shape_for` adds up how far the
  slider targets raise the head (each target's rise measured from the mesh at load) and the height
  targets take up the difference. A person at look height 1.0 stands at the body's own height
  whatever their build or ancestry; the test holds it to a centimetre at look heights 0.9, 1.0 and
  1.15.
- **The skeleton follows**, as it does for height: the converter wrote each target's movement of
  every joint beside it, so `Skeleton::reshaped` moves each joint by every active target's movement
  and remakes the inverse binds. The largest movement a build slider makes is the hips' (the mean
  of their flesh): 6.3 cm back and up at least muscle and most weight on the female body. Two tests
  hold it (below), and the rig shows it (Proof).
- **The palette needs no more slots** (brief item 3): the most a person turns on at once is seven
  targets, one height target, three around a point of the muscle-weight grid (the centre has none),
  one end of proportions and two of the three ancestries (each weighted by its share less the
  smallest share, which shapes the body exactly as shares less a third would because the three
  targets add up to nothing, and leaves one off). `pose::MAX_ACTIVE_MORPHS` is ten, so
  `MORPH_SLOTS` stays at two. Each active target costs every skinned vertex two more reads in both
  skinning shaders; measured at the crowd vantages (section 8.0, "With the body sliders"), 0.06 ms
  more compute skinning for 16 near bodies and 0.2 to 0.6 ms more scene for 34 to 50 middle ones,
  CPU inside its spread. Rung 4's crowd draw should give middle bodies only their largest targets.
- **Over the network**: `PlayerLook::shape` (humanity-core, so the relay reads the same shape and
  passes it through unchanged in the joined player's `appearance`), each id cleaned like a body id
  (`clean_shape_id`, at most 24 characters), each value clamped to 0 to 1 and rounded to a thousandth,
  at most 16 of them; sent as short decimals and left out entirely when empty, so a look without
  sliders is the same bytes as before. A full set is about 100 bytes, once per join and once per
  view entry, never per frame (8.5). A look without `shape` (an older app's) reads as the body as
  made. The look still goes only at join, as before: a change made mid-session reaches others at
  the next join.
- **A crew is varied**: `characters.ron`'s `crew` wearer names a range per slider (muscle 0.3 to
  0.75, weight 0.2 to 0.8, proportions 0.3 to 0.7, each ancestry 0 to 1 then made into shares) and a
  height range (0.94 to 1.06); each member's values are picked in them by a stable hash of who they
  are (`library::stable_unit`, FNV-1a over the slider's id from the person's seed, then SplitMix64's
  finisher), so each keeps theirs every frame and twenty people get twenty shapes. The dev crowd is
  the same kind, so the crowd vantages now measure varied shapes.
- **The words, and why these words.** MakeHuman's own data (its CC0 `data/modifiers/` folder at
  commit a8bc2d54, read 2026-11-09) labels the sliders "Muscle", "Weight", "Proportions",
  "African", "Asian" and "Caucasian", describes each ancestry as "African ethnicity of the human (the
  three ethnicity values are normalized so they sum to 100%)." and proportions as "Proportions of the
  human features, often subjectively referred to as qualities of beauty<br>(min is unusual, center
  position is average and max is idealistic proportions)." Every macro value runs 0 to 1 with the
  average in the middle (its targets are named `averagemuscle`, `averageweight`, and the average
  build's target is empty); the ancestries default to a third each. What the page says:
  - **Build**: "Muscle" from "less" to "more" and "Weight" from "lighter" to "heavier", MakeHuman's
    own words for the sliders with the ends in plain words.
  - **Proportions**, from "shorter legs" to "longer legs": MakeHuman's ends are "unusual" and
    "idealistic", a judgement of beauty the page does not repeat. Measured, its uncommon end lowers
    the hips about 1 cm, widens the waist and narrows the shoulders; its ideal end raises the hips 2.2
    cm and narrows the waist. The words say what each end does, and the hint says real people are
    found all along it.
  - **Ancestry**, not MakeHuman's "ethnicity" (the sliders shape a face and body, which follow where
    a person's ancestors came from; ethnicity also takes in culture, language and belonging, which a
    body shape cannot show), as shares of one whole
    showing their percentages, moving together as MakeHuman's do. The parts are "African", "Asian"
    and "European": MakeHuman's first two words, and a place for the third where MakeHuman says
    "Caucasian", so all three name where ancestors came from in the same plain way (an attempt to
    quote a style guide on the word, APA Style's racial and ethnic identity page, could not be read
    on 2026-11-09: the page came back empty to an automated reader). The converter names that target
    `european` too (MakeHuman's file names stay as they are in its input list). The note under the
    heading says skin colour is chosen on its own.
  - A glossary card for MakeHuman (`data/glossary.json` `makehuman`), marked in the page's note.
- **Web**: the website has no character page to mirror (`docs/PAGES.md`, `showroom.rs`: "No website
  page (no game in a browser)"); the shape reaches the web nowhere else, so there is nothing to
  mirror and no divergence (the dual-UI parity rule's documented "no").
- **Dev tooling (section 9's Animation tools, begun)**: the Dev page's Characters card, **Show
  skeletons**, draws every drawn body's joints and bones on top of everything (through the overlay
  list the editor's gizmos use), so a skeleton can be seen inside its body; showcase verbs `shape`
  (the look's slider values), `lineup` (a row of people at given slider values, facing the camera),
  `skeletons` and `varied_shapes` (the A/B above), each released between vantages
  (`scripts/lib/showcase-pins.js`).
- **Proof**: the vantages `anim-shape-{female,male}-build` (the body as made, thin, heavier, the
  heaviest corner, slight, muscular, strong and heavy) and `-ancestry` (as made, each ancestry alone,
  each end of proportions), each with a `-skeleton` twin. The first sweep stood the row 45 cm under
  the floor (the camera at 1.25 m, the row on the feet under an eye-height camera): the skeleton
  overlay showed it at once, the legs running on below the floor line; the camera now stands at eye
  height. In every capture all seven or six stand at one height, and every joint sits inside its body
  (hips in the pelvis, knees in the knees, a hand joint in each hand). Tests, each seen red: the
  sliders drive their targets as the body file says and the height holds
  (`the_body_sliders_drive_their_targets_as_the_body_file_says`), the skeleton follows a build
  slider, every slider has its words, a crew is varied and stable, the shares add up to a whole, a
  look's shape is clamped and an old look reads as the body as made, and a save from before the
  sliders loads. The character page: `tests/snapshots/character_shape.png`.

### Answers after the body sliders (operator, 2026-11-09)

- **Ancestry words.** He asked why "European" instead of MakeHuman's "Caucasian". The coordinator's
  answer: "Caucasian" is Blumenbach's 1795 race-classification term named after the Caucasus, not a
  place a person's ancestors come from the way "African" and "Asian" are, while its old scope (North
  Africa, the Middle East, South Asia) is wider than "European", so the word may be narrower than
  MakeHuman's shape. Kept as built for now; a one-word change if he decides otherwise.
- **Which "races", and the people MakeHuman leaves out.** He asked: "What are the 'races' to
  consider? What about Native Americans and other races not on the list? Is that a tuning of
  MakeHuman?" MakeHuman sculpted exactly three ancestry shapes; we use them unchanged. Human
  variation is continuous, so any fixed list leaves people out. Instead of more categories, the
  coordinator recommended MakeHuman's face and body detail targets, so anyone can shape their own
  face; he answered "Yes." (fetched the same evening: 1,095 files, 17.7 MB, the character assets
  finding's evening addendum). The three ancestry blends stay as quick starting points. Built after
  foot placement, beside clothes and hair; with hundreds of targets, each person's face is built once
  when their look changes (versioned by the look, obeying the reuse switch) rather than blended on
  the GPU every frame.
- **A changed look reaching others mid-session.** He asked what rate limit it would take: sent when
  Done is pressed, at most once every 10 seconds per player, collapsed to the latest shape, only to
  players who can see the person, and capped by the relay at a few a minute (protection against a
  modified client flooding nearby players, and against flicker harassment). Later, not urgent.

## 15. Built: rung 2a, feet on the world, crouch and jump (2026-11-09)

The operator approved rungs 0 to 2 on 2026-11-08 ("Let's do this so players get a good player
model and animations.") and on 2026-11-09 asked for MakeHuman's clothes and hair after foot
placement, so the feet came next. Section 4's design, built for the feet; the rest of rung 2
(interaction anchors, grips, hands on rungs, true first person) is 2b.

- **The ground under each foot (section 4's first bullet)** comes through one trait,
  `humanity_animation::Ground` (the crate never reads the world: it stays pure), which the engine
  answers in `src/engine/feet_ground.rs`. Aboard, `ShipGround` asks `engine::footing` per sole
  point, with the person's own feet as the height a step up is measured from (so a foot finds the
  tread a body there could step onto, never the deck over its head): the room's floor, a floor
  slab, a stair tread, a ramp, a deck, a platform, an elevator car, a finished built piece. On a
  planet, `PlanetGround` reads the drawn ground the planet walk stands the player on (the patch drawn
  there this frame, through one single-shot `DrawnPatchSurface` per frame, with the map packs' road
  beds and building pads in it; the elevation field where no patch is drawn yet), every position
  AND direction in f64 all the way to the sampler (a render point near the camera becomes
  `anchor + rot^-1 (p - camera)` in the body's unrotated frame, the ground point comes back the same
  way), and its slope from two more samples a quarter metre away in the tangent plane. The showroom
  stands on its podium (a level floor at its height).
- **Contacts are labelled on every clip at load** (`feet::label_contacts`, on each body's own
  bake): a foot is planted when its lowest sole point is within 4 cm of the lowest it goes in that
  clip and no higher than 8 cm over the floor, rising or falling slowly, and sliding backwards under
  the body at the clip's own speed (a clip played in place moves a planted foot exactly that way),
  the limits growing with the clip's speed; one-frame plants are dropped; a clip tagged `air` in
  `clips.ron` (the jump's air loop, whose hanging feet look low only against themselves) plants
  nothing. Measured on the female body (the male alike): idle both feet 100%, walk 63% and 61% (one
  foot alone 76% of the time), crouch walk 59% and 64%, jog 10% and 10%, sprint 14% and 10%, the air
  loop 0%. The runs' short stance is the clips' own (a few frames at 30 a second); the swing rule
  below keeps every unplanted foot out of the ground anyway. The graph blends the labels as it
  blends the poses (`Animator::contacts`).
- **The solve (section 4's second bullet)**, on each person's OWN reshaped skeleton (`Figure.shape`,
  never `body.skeleton`) in the body's model space, after the graph's pose: each foot has FOUR sole
  points (the back of the heel and the point under the ankle, turned with the foot; the point under
  the ball and the toe tips, turned with the toes; each at its joint's rest height, so feet are
  never aligned across rigs, rung 1's lesson; the heel and toe reach are measured from the mesh at
  load). A planted foot first turns to the slope about its ankle (the ground's own normal on a
  planet and on the test grounds; aboard two samples across 8 cm, a riser read as a step, not a
  slope; at most 35 degrees); then its lowest sole point against the ground (each point against the
  plane of the ground sampled under the heel or the ball, so a toe tip on a slope reads the slope)
  is the pivot and is put ON the ground, which removes rung 1's 1 to 5 cm of sinking: on a step's
  edge the ball rests and the heel hangs (the toe pivot), at a heel strike the back of the heel
  touches. (A first build turned the foot about the pivot after lifting it, and a foot already
  heel-down at a heel strike went 11 degrees further into a 20% slope, 3.7 cm deep.) A swinging foot
  keeps the clip's lift over the real ground under it, eased as the ground changes and never
  through it: it asks the ground under all four of its sole points (the planes under the heel and
  the ball miss a riser between them, and swinging over a nosing the toe tips went 15 to 16 cm into
  the step above in the second rig sweep), the toe tips read where the toe cap's front is at the
  toes' pitch that frame (`FootRig::toe_ahead`, measured from the mesh at load for every 5 degrees
  of pitch: the cap's front stands a few centimetres above the sole, so a foot pitched toes-down
  carries it ahead of the toe tips, and with only the sole points read the homestead stair test
  still found a front vertex 1.4 cm ahead of them, 9.4 cm into the riser above), the back of the
  heel read where the heel counter is at the foot's pitch (`FootRig::heel_behind`: walking DOWN
  the stair, a foot pitched toes-up carried it behind the heel's back point and cut 6.3 cm into the
  riser of the step it was leaving in the tests' whole round, 12.4 cm in the fourth rig sweep), and, until it
  starts down onto its landing, the ground 12 cm further ahead (`swing_ahead_m`), so the lift rises
  before the next step the way a foot clears a stair, instead of popping up at the riser.
- **The pelvis** is lowered by the larger drop (so the lower foot can reach its ground with the knee
  still bent) or raised, less far, when both feet stand higher than the root (a remote player's
  root interpolated between treads, a dev stand-in gliding up a stair line), smoothed over 0.07 s,
  and never so high that a planted foot is out of its leg's reach (the follow lags a root climbing a
  stair while the trailing foot stays on a lower tread). Then a **two-bone analytic IK** per leg
  (`feet::two_bone`, the method of D. Holden's "Simple Two Joint IK": the knee angle from the law of
  cosines with the hip-to-ankle line held, then the chain swung about the hip onto the target, the
  bend kept in the plane the clip's knee already bends in once it bends 30 degrees or more; a nearly
  straight leg is first twisted about the hip-to-ankle line toward the knee over the toes, because
  the MakeHuman idle's knees, a few degrees bent and splayed with the legs' rest, bowed outward as
  the solve bent the uphill leg across the planet vantage's slope) puts each ankle exactly on its target;
  the foot takes the clip's orientation turned to the slope; the twist joints are redone.
- **Foot locking** keeps a planted foot's CONTACT POINT where it touched down: the heel's sole point
  at a heel strike, then the ball's as the foot rolls onto it (the lock moves from one to the other
  where they are, so nothing jumps), never the ankle, which rises and travels forward as a foot
  rolls over its ball (the first build locked the ankle and the rolling foot slid 5 cm on a flat
  floor). It holds fully from the frame the foot plants, lets go over 0.15 s when the clip lifts the
  foot, and breaks at once when the body is 35 cm from it (a teleport, a turn the clip does not
  follow). The graph's stride scale is unchanged (speed divided by the person's own stride scale);
  the lock takes up what stride warping leaves.
- **Footsteps on stairs (section 4's third bullet)**: `Footing::tread_at` gives the stair piece's
  axis, the tread's middle and its depth from the piece's own size and riser count. While a foot
  swings down toward a tread, the move that puts the WHOLE flat foot (back of the heel to the toe
  tips, from its rest length: a foot touches down pitched and then flattens about its locked heel,
  growing forward) on that tread's middle is worked out and eased in as the foot comes down (none
  12 cm up, all of it at touchdown), so the foot lands there and the lock keeps it there: no jump at
  touchdown and no slide after (a first build moved the lock at touchdown and slid the planted foot
  10 cm). The end of the foot pointing up the stair is kept at least 3.5 cm short of the next riser
  (`tread_margin_m`), the other end may hang over the nosing behind, as heels do on a narrow tread
  (a man's 26 cm foot on the homestead's 28 cm tread leaves 1 cm either side, and the projection of
  a touchdown lands within about 2 cm, so exact centring and a 1.5 cm margin both left toe tips a
  fraction of a millimetre into the step above). Where it lands is projected when the foot is low and coming down
  (its speed over the ground halved, as a landing foot slows to rest; refined each frame), the move
  is worked out exactly from where the foot is each frame (easing the move lagged the sweeping foot;
  easing the tread's middle lagged the last nosing), a one- or two-frame gap in a clip's contact is
  closed, and a foot planted again while its lock is still letting go keeps the same spot.
  On the shipped homestead's own stair through the real footing, both bodies walking the
  anim-stairs stand-in's path (the root gliding up the line of the nosings, up to a riser under each
  tread), as a clip at its own pace and through the game's graph at 60 and at 30 frames a second,
  every planted foot rests on its tread, its lowest drawn vertex between 0.2 cm under and 1.1 cm
  over the ground under it, sliding at most 1.0 cm (one start from standing aside, under Known
  limits), and no swinging foot's vertex goes more than 0.1 cm into a step (before the swing read
  its own sole points and the toe cap, 16.6 cm, then 9.4 cm); over the path's whole round at 30
  frames a second, up, the turn on the landing, down and the turn at the foot, planted feet 0.1 cm
  under to 1.3 cm over and swinging feet at most 0.5 cm into a step (6.3 cm before the heel
  counter was read, 3.7 cm with it read only at the foot's own pitch: a vertex near the ankle is
  skinned partly to the shin and swings further than the rigid foot, so the toe cap and the heel
  counter are read for the most either stands out within 15 degrees of the pitch); on a test stair with the planner off,
  toes stand 18 cm inside the step above; with the feet off, 40 cm in. The rig's own run is under
  Proof.
- **Modes and tiers (8.4 and 8.6)**: `FootMode::Full` for near bodies in the Realistic animation
  mode (Settings > Gameplay > Animation, Realistic by default, question 5), and question 5's
  "simplified automatically below it" as a budget: each frame the nearest bodies get the full solve,
  as many as `full_budget_ms` (characters.ron, 1 ms) buys at what a solve has been costing on this
  machine (every full solve is timed, the cost eased over frames so one slow frame changes nothing),
  and the rest of the near tier the pelvis offset (`Report::feet_over_budget` counts them; on the
  operator's machine a solve costs about 12 microseconds, so the budget buys all 16 near bodies with
  room to spare, and it bites only on a much slower CPU or a larger near tier); `FootMode::Pelvis`, the
  pelvis offset alone with one ground query a foot and no leg IK or locking, for middle bodies and
  for everyone in Simplified (where it raises the whole body until no planted foot is in the ground
  and holds through a run's flight); `FootMode::Off` is rung 1's pose, the Dev page's Feet on the
  world switch for a same-boot A/B (showcase `feet`).
- **Crouch** is a key (Settings > Controls > Crouch, C by default, held; `GameAction::Crouch`,
  data-driven binding, its label in the action table like every other bind): the walk slows to the
  crouch pace (data/game.csv `crouch_speed_realistic_mps` 0.8 and `crouch_speed_simplified_mps` 1.6,
  never faster than the walk; Shift does not sprint while crouching), the first-person view lowers
  0.55 m, eased (`Camera::view_drop`, a VIEW offset: the camera's position, which the walk, the
  footing, the walls and reach all read, stays the standing eye, so crouching changes what you see
  and not where the game holds you; a lower capsule that passes under low things is a later step,
  because the walk's floor and wall rules read one fixed eye height), and the graph plays the crouch
  idle and crouch walk (locomotion.ron's `crouch_idle` and `crouch_move`).
- **Jump** (Space, the existing input) plays the take-off, the air loop and the landing: the graph
  reads the time off the ground (the walk's own grounded flag, `CameraController::airborne`: not on
  a ladder, not in dev flight; a step down a stair is no time off the ground) and the vertical speed
  read from where the body went (`MotionTracker`); a body that leaves the ground going up plays the
  take-off, one in the air 0.3 s without it (a fall) goes straight to the air loop, and touching
  down plays the landing before standing or walking on. The feet are off while off the ground.
- **The network carries the stance (8.5)**: `humanity_core::stance` (compiled into the app and the
  relay): one number, `"stance"`, the stance's code (plan 8.5's list: stand, crouch, sit, climb,
  swim, fly, zero-g; only stand and crouch are sent today) plus 128 when off the ground, LEFT OUT
  when standing on the ground so a standing player's message is the same bytes as before, and read
  by both ends through one clamping reader (anything that is not a whole number from 0 to 255 or not
  a known code reads as standing). The relay passes it on to the players who have the sender in view
  only when it is not plain standing. Others' bodies crouch and jump from it (`RemotePlayer::posture`).
  A few bytes on an update, only while crouching or in the air.
- **The character page shows the height in metres** ("1.62 m", words by key,
  `characters.height_metres`) beside the height slider, in place of the bare multiplier: the chosen
  body measured from its mesh with the look's height, build and ancestry through `shape_for`
  (`LoadedBody::standing_height_m`; the sliders' lesson that ancestry moves the top of the head is
  in it), published by `engine::animation` when the look changes (`GuiState::my_height_m`).
- **Dev tooling**: the Dev page's Feet on the world switch, and with Show skeletons on, a marker
  where each foot found the ground (joint-coloured while it is locked); showcase verbs `feet`,
  `animation_mode`, `crowd_crouch`, `foot_probe` and `hold` with `crouch`; the foot probe
  (`src/engine/foot_probe.rs`): for a set time, each near body's foot vertices skinned on the CPU
  with the very palette the GPU draws, measured against the ground under them, written to
  `debug/foot_probe.json` and judged by `scripts/lib/foot-probe-check.js` (the vantages'
  `foot_probe`: a planted foot more than 1.5 cm in or 2.5 cm over the ground fails; flat ground under
  a stair vantage, or nobody planted, is MISCONFIGURED).
- **Web**: the website draws no 3D world and has no game Settings or character page to mirror, so
  nothing here has a web twin (the dual-UI parity rule's documented "no").
- **Proof, in the tests** (each seen red; `src/engine/animation_feet_tests.rs` walks the shipped
  MakeHuman bodies through their shipped clips and measures the DRAWN feet, the foot vertices skinned
  on the CPU with the palette the GPU would get, against the ground, each walk run again with the
  feet off as a control that must fail the same bound):
  - Flat floor, both bodies, idle, walk, jog, sprint, crouch idle and crouch walk: planted feet
    between 0.2 cm under and 1.1 cm over the floor, sliding at most 1.5 cm; feet off: the sprint
    5.1 cm (female) and 4.8 cm (male) in, a walk sliding 9 to 10 cm, a crouch walk 17 to 18 cm.
  - The stair: above. A 20% slope: planted feet 0.1 to 1.0 cm over it (feet off: 6.0 cm in, 4.2 cm
    over). Walking a quarter faster than the clip's stride: a locked foot slides 0.8 cm, unlocked
    20 cm.
  - The contact labels (above), the two-bone solve (and a nearly straight knee bending over the
    toes), the graph's crouch and jump, the stance's wire
    form in the core, through the app's message and through a real relay (a real-socket test), the
    animation mode's save, and the walking pace's words (BUG-289).
- **Proof, in the running game** (the release build, rig defaults, 2560x1387, a background game held
  to 30 frames a second; each vantage's foot probe over 1.5 s, every third foot vertex skinned with
  the frame's own palette and measured against the ground under it; the feet from sweep
  `20261110-063602` on the tree merged with main, the costs from the clean sweep `20261110-053403`,
  both in this rung's worktree's `.probe-rig/sweeps/`; every green vantage passes its foot probe and
  every `-feet-off` red twin, the same boot with the feet off as rung 1 drew them, FAILS it on its
  depth):
  - `anim-stairs` (the stand-in walking the homestead's own stair through the game's graph):
    planted feet 0.2 to 1.0 cm over their treads, 26 and 30 planted frames, 58 frames on a planned
    tread, every swinging vertex over its ground (0.4 cm the lowest); `anim-stairs-feet-off`: 36.0
    and 37.8 cm into the steps. On the way there the second sweep read the left foot 17.2 cm into the
    step above and the swinging toes 15 to 16 cm into the risers, and the fourth and fifth, with
    every planted foot passing, swinging feet still 8 to 12 cm in (the toe cap and heel counter
    measured on the body never reached each person's own legs). Captures `anim-stairs.png`,
    `anim-stairs-feet-off.png`.
  - `anim-ramp` (the stand-in walking down the ship's ramp): 0.1 to 0.8 cm over it;
    `anim-ramp-feet-off`: 2.1 and 2.2 cm in.
  - `anim-crouch` (the player crouched, the C key held, and the stand-in walking crouched, as
    another player's crouch arrives over the network): 0.1 to 0.5 cm over the floor, both people.
  - `anim-slope-map` (the player standing across the 21% cross slope of the Silverdale bank, third
    person): 0.3 and 0.5 cm over the drawn ground with 15 cm between the ground under the two feet,
    the uphill knee bent over its toed-out foot; `anim-slope-map-feet-off`: the uphill foot 3.9 cm
    in, the downhill one 10.9 cm in the air. The first two sweeps showed no body there at all (drawn
    400 km up with the station, Known limits).
  - `anim-crowd-near` and `anim-crowd-middle` and their `-feet-off` twins: the costs in 8.0.
- **For the clothes and hair (the next rung), what this one learned**:
  - A sole point is its joint's rest height over the body's floor (y = 0), so a SHOE with a sole of
    its own changes it: once garments are meshes, measure each foot's lowest vertex at rest with its
    shoe on (`FootRig::with_extents` already measures the heel and toe reach from the foot's
    vertices; the heights want the same treatment), or a shod foot sinks by its sole.
  - `LoadedBody::foot_vertices` are the vertices whose largest weight is on a foot or toes joint;
    a shoe bound the same way joins them, so the foot probe measures the shoe, which is right.
  - Measure what is drawn: the first builds here passed every check of the IK's own targets while
    the drawn toes stood 18 cm inside the next tread; only the CPU-skinned mesh against the ground
    showed it. The same goes for a sleeve through a forearm.
  - Test the path the game takes, not a neighbour of it: every walk in the tests used the body's
    own rig while the game builds each person's legs on their reshaped skeleton, and the toe cap
    and heel counter measured from the mesh never reached that copy (`person_rig`,
    `FootRig::with_measured` now carry them, and a test compares the two). A garment measured at
    load has the same trap: whatever is measured from the body must reach each person's copy.
  - A rig's probe window falls wherever the stand-in happens to be: a path walked both ways in the
    rig must be walked both ways in the tests (the way down found the heel counter).
  - The engine's test program takes about ten minutes to rebuild in a worktree; keep the logic in
    the pure crate where its tests take seconds, and put a debug print behind an environment
    variable (the feet tests' `FT_DEBUG=1`) rather than rebuilding to look.
- **Known limits, for the rungs after**:
  - Crouching keeps the standing capsule: the view lowers and the body crouches, but nothing yet
    lets a crouched body pass under something low (the walk's floor and wall rules read one fixed
    eye height, `EYE_HEIGHT_M`, in many places; a crouch capsule is its own careful step).
  - On a planet only the player's own body stands on the local radial; other players and the crew
    there are still drawn +Y up (their positions' frame on a planet is the planet co-presence work's),
    and the dev crowd (`crowd`, `figure_path`) lives in the home frame, which lib.rs moves to the
    station's orbital place before the scene pass (`station_off`), so the planet slope vantage
    measures the player's own body. That shift also drew the player's own body 400 km up until this
    rung: the scene list is the home frame, and the body on a planet stands in render space, so it
    now joins the list after the shift (`draw_people`'s `render_space`, as the planet piece in hand
    does). Third person's small lift over the shoulder is along the local up there too.
  - The stair planner moves a foot only along the stair (a foot that lands across the stair's side
    edge is the full-body or footstep-search rung's), and asks the toe tips and the back of the heel
    for a tread only low in the swing.
  - Quaternius's runs plant a foot for 10 to 14% of their cycle; a run's IK holds that long and keeps
    every other frame out of the ground, which is right for the clips we have; motion matching's
    captured runs (rung 3) bring their own contacts.
  - The take-off clip plays once the body has left the ground (its crouch before the leap is late);
    an anticipation from the jump key is the step if it shows.
  - Middle bodies get the pelvis offset alone, so a middle body on a stair stands with one foot on
    its tread and the other a little in the air: 8.4's tier, by design.
  - Starting to walk from standing, the idle's planted contacts last into the blend to the walk, so
    a planted foot can be held while the body moves off until its lock stretches to the 35 cm break
    and the foot snaps to the clip's place (seen once in the tests: the male body, the stand-in's
    path through the graph at 60 frames a second, a 35 cm slide in one frame). A start step (rung 3's
    motion matching has captured starts; until then a step taken when a lock stretches past half the
    break) is the fix; a softer let-go would only trade the snap for a slide.

## References

- MotionBricks: README, model cards and ONNX guide, https://github.com/NVlabs/motionbricks; the
  paper, https://arxiv.org/abs/2604.24833. Licences: the finding linked at the top.
- Motion matching: Simon Clavet, "Motion Matching and The Road to Next-Gen Animation", GDC 2016.
- Learned motion matching: Holden, Kanoun, Perepichka and Popa, ACM Transactions on Graphics, 2020;
  example code (MIT), https://github.com/orangeduck/Motion-Matching.
- Inertialization: David Bollo, "Inertialization: High-Performance Animation Transitions in 'Gears of
  War'", GDC 2018.
- glTF 2.0 skins: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html.
- VRM 1.0 humanoid bone names: https://github.com/vrm-c/vrm-specification.
