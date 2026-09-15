# Habitat generation: spines and drums

> **Operator direction, 2026-09-15.** Two generation modes are wanted, so NPC
> inhabitants have somewhere to live aboard the mothership:
>
> 1. **Spine** -- "skyscraper like blocks that contain everything necessary to
>    survive, like industry, residential, research, in a rough cube shape (or
>    configurable)". A static, non-rotating section, more like a skyscraper or
>    an aircraft carrier.
> 2. **Drum** -- "the inside of a spinning drum where the gardens are visible
>    from the inside... allows for gravity while not accelerating". References
>    given: Babylon 5, The Expanse, Macross. The operator's own long-armed ship
>    design spins its arms as a sectioned-drum alternative.
>
> Companion to [crowd-simulation.md](crowd-simulation.md), which says WHO lives
> here; this says WHERE.

## Decisions (operator, 2026-09-15)

- **First drum: 250 m radius, 2 km long.** "The smallest comfortable makes
  perfect sense." 1.89 rpm for a full 1g, under the ~2 rpm Coriolis comfort
  limit, ~780 one-acre allotments.
- **Keep developing the existing construction editor.** "The easier we make it
  for me, you, and other developers the better." No second editor.
- **Normal play edits only your own zone; editing beyond it needs a rank.** See
  the powers/rank split in [game-modes.md](game-modes.md).
- **Gravity is real, not faked.** "The whole point of doing the drum/ring shape
  is to have the spin and gravity. If the spin were to stop then I'd like
  gravity to go away." Confirmed achievable at no cost; see Gravity below.

## Terminology

Two habitat kinds, and the words used throughout this doc:

- **DRUM** -- rotating, spin gravity, curved interior surface you walk on the
  inside of, gardens visible overhead. The operator's word; keep it.
- **SPINE** -- the static, non-rotating section. The "skyscraper / aircraft
  carrier" shape: a structural volume packed with stacked decks and bays
  carrying mixed functions (industry, research, docking, storage) rather than
  one function per zone.

"Block" was used in an earlier draft for the second kind and is **retired**: it
collided with the solid placeholder boxes `generate_zone_filler` already emits,
which is the opposite of the intended meaning (those have no interiors at all).

### The physics consequence of a static section

Worth stating plainly because it follows from the operator's own realism
requirement and changes what a SPINE is for. **A non-rotating section of a
coasting ship is in freefall: it has no gravity at all.** If the drum spins for
gravity and the spine does not spin, the spine is a zero-g volume.

That is realistic and matches the references: Babylon 5 has zero-g sections, and
The Expanse's Nauvoo spins its drum while the rest of the hull does not.

It is also a genuinely good split rather than a compromise. Zero-g is an
ADVANTAGE for exactly the functions a spine holds: moving mass takes no lifting,
large structures carry no self-weight, docking does not fight a rotating frame.
So the drum is where you live and farm at 1g, and the spine is industry,
docking, fabrication and storage where weightlessness is the point.

**RESOLVED 2026-09-15: the spine has thrust gravity.** The operator: "the most
common state of the fleet is going to be in the perpetually forward motion going
between stars so it makes sense the spine has gravity." A perpetually-burning
fleet has a uniform aft-pointing gravity field through the whole hull, so the
spine is habitable with conventional decks perpendicular to the thrust axis, and
zero-g is the EXCEPTION (drive off, repair, coast) rather than the spine baseline.

This has a large and non-obvious consequence for the drum, because thrust gravity
and spin gravity are perpendicular and therefore TILT the drum floor. The full
model, the tilt table, g-load effects and the zero-g locomotion toolkit now live
in [gravity-and-movement.md](gravity-and-movement.md). Read that before building
either habitat: it is what decides how much thrust the fleet can cruise under.

## The unifying idea: one generator, a curvature parameter

A spine is a drum of infinite radius. If both kinds are one system parameterised
by curvature, you get the two the operator asked for plus everything between: a
gently-curved large-radius ring section, a tight high-spin drum, a flat deck.
That is the Infinite-of-X answer, and it avoids the "two partial gardening games"
anti-pattern the project already learned once.

Concretely: a habitat volume carries an **axis** (or none), a **radius** (or
infinity), and an **arc extent**. Flat deck = no axis. Full drum = 360 degrees.
Sectioned drum = the operator's spinning arms, which are arcs of a drum that was
never completed.

## Spine: close, and not a new subsystem

**What already works.** `HomeStructure::tile_home_clones`
(`src/ship/home_structure.rs:988`) already takes a zone volume and fills it with
a grid of complete walled dwellings, with real walls, doors, detected rooms, and
auto-generated corridors bridging adjacent slots. That is genuinely a
volume-to-content generator and it is shipped.

**The two gaps.**

- **It is single-Y-plane by construction** (`home_structure.rs:1041-1045`), and
  every authored `Zone` in `data/blueprints/ship_structure.ron` sits at `y = 0`.
  A skyscraper is the same tiler looping a Y axis.
- **Only `residential` produces interiors.** Every other zone type routes to
  `generate_zone_filler` (`home_structure.rs:938`), which tiles solid silhouette
  boxes via `footprint_box` -- no walls, nothing enterable. Industry, research
  and the rest are currently scenery.

So the spine is: give the tiler a Y loop, and let non-residential types emit
`HomeStructure` bodies instead of solid boxes. `resolve_positions`
(`src/ship/fibonacci.rs:483`) is the room packer to feed it; it needs a bounding
volume added, since today it packs outward from a room list with no container.

**Scale reality check.** The "mothership" today is about 400 x 200 m of labelled
wireframe around one real 55 x 89 m walled house. Zone height is capped at 12 m
in the data and 100 m in the editor (`construction.rs:2357-2359`). A spine wants that cap raised and the Y loop above.

**Doc drift to be aware of:** `mothership-superstructure.md` presents
`Deck > Zone > Room` as the shipped model. There is no `Deck` type in the zone
system at all; `DeckDef` exists only in the legacy `layout.rs` path used by
`starter_fleet.ron`. Neither tier above `Zone` is built.

## Drum: substantial, but roughly 60 percent already exists

This is the surprise, and it is a good one.

**Walking on a curved surface with a per-frame-variable up vector, while the
frame spins beneath you, is already solved and shipped.** It is what planets do.
`src/surface_walk.rs` and `src/surface_move.rs` take `up` as an argument
throughout and contain no world-Y assumption:

- `tangent_basis(up)` (`surface_walk.rs:162`) builds an east/north frame for any
  up, with pole safety.
- `surface_forward(up, yaw, pitch)` / `surface_look_angles(up, dir)`
  (`surface_walk.rs:181`, `:193`) go both ways.
- `Camera::set_surface_up` (`renderer/camera.rs:259`) re-derives yaw and pitch in
  the new basis so the view does not pop, and there is already a smooth EASE
  between two different up-bases across an altitude band (`lib.rs:4035`).
- `surface_wish_dir` (`camera.rs:982-995`) projects WASD into the tangent plane
  with no Y anywhere in it.

**Frame parenting is also solved.** The persistent `frame_lock_anchor` plus the
`frame_lock_capture` / `frame_lock_ship_pos` round trip (`dev_travel.rs:116`,
`:129`) is exactly "an entity parented to a rotating frame": walk and gravity
move the anchor in the unrotated frame, and the spin carries it. The view
co-rotates (`co_rotate_look`, `camera.rs:298`). Boarding and leaving a moving
hull frame, with the look direction re-based across the transition, is debugged
(`lib.rs:3196-3283`).

**Gravity is already data-driven and position-dependent.** `gravity_curve` and
`gravity_at(alt)` (`terrain/planet.rs:176`, `:212`) are a natural hook for
`omega^2 * r`. And `docs/design/engine-architecture.md:716` already specifies
exactly this: "Gravity vector points outward from rotation axis. Magnitude =
omega^2 x radius." It was designed and never built.

**Rapier is not in the way.** `PhysicsWorld::step` is never called
(`ship/wall_collision.rs:4` says so outright); the player is a hand-rolled
kinematic controller writing the camera directly. So there is no engine gravity
vector to fight. The flip side is that you get nothing for free either: no
Coriolis, no thrown objects arcing correctly.

### The three real risks

1. **The sign inversion.** On a planet the floor is at MINIMUM radius and the
   model is full of `r.max(rest)` (`surface_walk.rs:229`, `:236`, `:328`;
   `surface_move.rs:251`). In a drum the floor is at MAXIMUM radius, so every one
   becomes `min` and `rest = hull_r - eye - clearance`. It sounds trivial and is
   not: it inverts the meaning of ground, altitude, rest, clamp and fall across
   two well-tested modules, and every existing test encodes the planet sign. The
   right shape is a signed gravity sense (+1 planet, -1 drum) threaded through,
   not a duplicated module.

2. **The controller split, which is the largest piece of real work.** The engine
   has two disjoint movement models: planet surface (variable up, frame-locked,
   radial) and building interior (world-Y gravity, 2-D XZ collision, ladders
   clamped on `.y`). Boarding a station explicitly DESTROYS surface mode
   (`lib.rs:3226-3227`). A drum is both at once: a curved surface INSIDE a
   structure. And `ship/wall_collision.rs` is architecturally 2-D -- it takes XZ
   line segments and states "Y is never touched" (`:11`) -- so a floor that
   curves needs it genuinely rewritten, not adapted.

3. **The Y-axis spin monoculture.** Spin is hardcoded about world Y in 61
   `from_rotation_y` sites, and `dev_travel.rs:127-128` documents the assumption
   that "planet spins about Y, camera yaw is about world Y, so they compose
   exactly". **The dodge:** define the drum's own local frame with its axis along
   LOCAL Y and rotate the whole drum frame into world orientation. The spin math
   then stays Y-axis in drum-local space, which is where it already works, while
   the drum still LOOKS horizontal. The engine already has the precedent for this
   trick: `state.station_world_rot` is consumed by rotating the world into the
   hull frame rather than rotating hull geometry (`engine/state.rs:215-217`).

### What to build it out of

**Not the ship stack.** The hull loft is strictly rectangular by construction
(`hull.rs:619-642`): a box that tapers in width and height along one axis, with
no radial parameter and no cross-section model. `Zone` is an axis-aligned AABB
with no rotation field at all. Neither can express a cylinder.

**The planet stack.** `planet_surface::build_surface_mesh`
(`terrain/planet_surface.rs:559`) is the only curved surface in the codebase that
people walk on and that grass and trees populate. For "gardens visible from the
inside", that is the precedent. The other two pieces worth aiming at a drum:
`sweep_profile` (`fibonacci.rs:766`), which extrudes a closed 2-D profile along a
straight run and wants generalising to an arc; and `orbit_ring_mesh`
(`hologram.rs:610`), the repo's only real torus, as a reference.

Also note `Mesh::cylinder` (`renderer/mesh.rs:325`) has OUTWARD normals, which is
the wrong way round for a drum interior.

### Sky and light

The atmosphere is an oversized shell sphere drawn only when `atmosphere_color` is
`Some` (`lib.rs:12462`), so suppressing it for a drum is a data decision, not a
code change. But there is no interior-volume renderer: a drum needs the far side
lit and visible overhead, a linear light running down the axis, and haze along a
chord. The shell model cannot express any of that. New work, and separable.

## Gravity: real, and free, for a reason

The operator asked whether the cheapness implied a simplification. It does not,
and the reason is worth writing down because it makes the realistic option the
easy one.

**Gravity in this engine is a scalar parameter, not a simulation.** `radial_step`
(`src/surface_move.rs:198`) takes `g_accel: f64` and hands it to `vertical_step`,
which does closed-form ballistics with terminal velocity. Rapier is never stepped
(`ship/wall_collision.rs:4`), so there is no rigid-body solver in the player's
path to be expensive in the first place.

So for a drum, the physically correct expression IS the implementation:

```
g_accel = omega^2 * r
```

One multiply. Everything the operator asked for follows for free:

- **Spin down and gravity genuinely goes away.** As omega drops, `g_accel` drops
  continuously to zero. Not a special case, not a toggle: the same expression.
- **The gravity gradient is real and automatic.** g scales with r, so gravity
  weakens as you climb toward the axis and is exactly zero at the centre. That is
  correct physics and it emerges with no extra code. It is also free gameplay:
  low-g near the axis, full weight at the rim.
- **Coriolis is also nearly free.** It is `-2 * omega x v`, one cross product per
  moving body. **Correcting earlier advice in this doc's build order:** the
  question is whether Coriolis is FUN, not whether it is affordable. It costs
  almost nothing.

**What zero-g actually costs is movement, not physics.** With `g_accel = 0` the
ballistics already behave correctly (you keep your radial velocity and drift).
What does not exist is any way to CONTROL yourself without a floor to push
against: the tangential movement model assumes walking. Handholds, push-off,
tethers or thrusters are real gameplay work. That work, not the gravity maths, is
what a zero-g SPINE would require.

**The genuinely expensive thing** is simulating every loose OBJECT in the drum
under spin, which would mean waking rapier and giving it a rotating frame. That
is where a simplification may eventually be wanted. The player's own gravity is
not where the cost is.

## Scale: what a drum actually holds

Spin gravity is `g = omega^2 * r`. Comfort research generally puts the Coriolis
limit near 2 rpm, which sets a floor on radius for a full 1g.

| Radius | Length | 1g spin | Surface speed | Interior area | Acres |
|---:|---:|---:|---:|---:|---:|
| 250 m | 2 km | 1.89 rpm | 50 m/s | 3.14 M m2 | ~780 |
| 500 m | 4 km | 1.34 rpm | 70 m/s | 12.6 M m2 | ~3,100 |
| 500 m | 8 km | 1.34 rpm | 70 m/s | 25.1 M m2 | ~6,200 |

(Babylon 5 is roughly the last row. 1 acre = 4047 m2.)

So a 500 m radius by 2 km drum is already an MMO-shard-sized world at one acre
per resident, and a 250 m by 2 km starter drum holds around 780 allotments. That
is the number to design the population against.

**Precision note.** Surface speed of 50-70 m/s is an order of magnitude gentler
than the planet case that already produced a shipped bug (the stale-anchor
altitude error, `surface_move.rs:269-290`, measured at 428 m of disagreement
between 30 and 7 fps). But the drum is 2 km across rather than 12,742 km, so the
same absolute error is a far larger FRACTION of the world. The planet-scaled
constants are meaningless here and must be re-derived: `ANCHOR_CONTINUITY_M`
(100 km, fifty times the drum's diameter), `SURFACE_ENGAGE_ALT` (10 km),
`CO_ROTATE_MAX_ALT` (100 km). Also the spin-delta staleness guard at
`lib.rs:4438` rejects deltas at or above 0.01 rad; at 1.34 rpm and 30 fps the
delta is 0.0047, which passes, but it is closer than it looks and is frame-rate
dependent.

## Occupancy: empty to claim, or full and finite

The operator's question, and their instinct: *"Alternatively the drum could be
fully inhabited forcing users to use the finite space they have available
carefully (i think this is the best MMO experience requiring users to join groups
to form larger farms than just the 1 acre or whatever 1 player is assigned at the
start)."*

**Agreed, and for a reason bigger than MMO dynamics.** An empty drum you expand
into teaches "when constrained, take more space". That is the opposite of this
project's thesis. A full drum where you hold one acre and must cooperate to farm
at scale teaches finite shared resources and cooperation over individualism,
which is the mission stated as a mechanic. It is also exactly what the roadmap
already wants from Generation-Ship co-op: "a shared life-support habitat where
selfishness literally collapses the colony."

**The refinement: full, but with turnover.** A strictly full drum makes the
newcomer experience "everything is taken". Let allotments change hands as
inhabitants move, retire or leave, so a newcomer receives an allotment because
someone gave one up. Scarcity stays real, the door stays open, and the NPC crowd
becomes load-bearing rather than decorative: they are your neighbours and your
competition for space, not scenery.

**The dependency this creates:** a fully-inhabited drum needs the tier-1 aggregate
population from [crowd-simulation.md](crowd-simulation.md) on day one, because
"full" has to mean something numerically before it can mean anything
emotionally. The two arcs are coupled: habitat generation gives the crowd
somewhere to live, and the crowd is what makes the habitat feel occupied.

## Build order

1. **Spine**, because it is an enhancement of a shipped generator and needs
   no new movement model: Y-loop the tiler, give non-residential zone types real
   interiors, raise the height cap.
2. **A flat drum-shaped SHELL** with no spin and no curved walking: prove the
   geometry, the interior lighting and the far-side-overhead look while the
   player still walks on a conventional floor. This de-risks the art direction
   before touching the movement stack.
3. **The signed gravity sense** through `surface_walk` / `surface_move`, with the
   planet tests kept green and drum-sign twins added.
4. **Drum-local Y-axis spin**, reusing the frame-lock anchor, with the
   planet-scaled constants re-derived for a 2 km world.
5. **Curved-floor collision**, the genuine rewrite of `wall_collision`.
6. **Coriolis**, last but cheap. See Gravity above: it is one cross product,
   so the question is whether it is fun, not whether it is affordable.

## Open questions

- **Drum axis orientation.** The references all show a horizontal axis you look
  down the length of. The drum-local-frame trick above should buy that look at
  Y-axis cost; it needs proving before it is relied on.
- **Do spine and drum share one editor?** `mothership-superstructure.md` has had
  "one editor or two" open since 2026-07-01. Curvature-as-a-parameter argues for
  one.
- **Does the player build inside their acre with the existing construction
  editor, or is an allotment a different kind of object?** Reusing the homestead
  editor is the obvious answer and would make an allotment just a small zone.

## Related docs

- [crowd-simulation.md](crowd-simulation.md), who lives here
- [mothership-superstructure.md](mothership-superstructure.md), the zone system
  and the M1-M5 staging (read its BUILT claims skeptically)
- [engine-architecture.md](engine-architecture.md), which already specifies spin
  gravity at line 716
- [infinite-of-x.md](infinite-of-x.md), why curvature is a parameter
