# Character animation: one body, from the player to a ship of crew

**Status: DESIGN, 2026-11-08. Nothing in this document is built.** Written after the operator came
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
  and build directly. Rung 1 recommends it (question 2).
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

- **Simplified**: the state machine with blend spaces, foot IK on, no finger animation, a smaller
  near tier. Works everywhere and is the automatic choice where the budget pie says so.
- **Full realism**: motion matching with warping and inertialized transitions, full IK, styles from
  the body's state, secondary motion, and the learned rung where present.
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

1. **May NVIDIA's MotionBricks weights ever ship or be fetched, as an optional pack, given they are
   legally usable but not under an OSI-approved licence and come with no training code or data?**
   Your rule: "We want to keep everything on open-source." *Recommendation: no for the core game;
   decide at rung 7 with measurements in hand, and only as an opt-in pack showing its licence.*
2. **Which body to build on?** MakeHuman's CC0 assets, a body made by a contributor under CC0, or one
   made from scratch. *Recommendation: MakeHuman's, for its CC0 status and real shape controls.*
3. **Accept motion capture from the community under CC0**, the way music contributions are accepted,
   to grow the database beyond CMU, ACCAD and 100STYLE? *Recommendation: yes, with the same review
   of what each contributor used.*
4. **See your own body in first person by default?** *Recommendation: yes, with a Settings toggle
   and "Reduce motion" beside it.*
5. **Default animation mode?** *Recommendation: full realism where the measured budget allows,
   simplified automatically below it; gameplay is the same either way.*
6. **Order against the other arcs**: crowd-simulation.md puts animation last, the fire props want the
   body first. *Recommendation: rungs 0 to 2 now (they serve the player, co-presence and props),
   rung 4 after the crowd's timetable and navigation.*
7. **A new crate, `humanity-animation`**, outside the crate split's planned ten? *Recommendation:
   yes; the relay never needs animation, and a pure crate keeps it testable without a GPU.*

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
