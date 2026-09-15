# Gravity and movement: one vector, many sources

> **Operator direction, 2026-09-15.** "The most common state of the fleet is
> going to be in the perpetually forward motion going between stars so it makes
> sense the spine has gravity. We could even introduce G changes based on the
> ship's maneuvering. That way if needed the ship could accelerate at 3 g or more
> but, that'd come with all of the negative effects of crew and equipment. Maybe
> certain animals/plants die when under a sustained 5 g burn to escape an
> incoming missile. For zero-g we can add grappling hooks, jet packs, mag boots,
> ropes, ladders, etc. We want to make moving as fluid as possible with all kinds
> of tools to get around. Either static (ladders) or dynamic (grappling hook)."
>
> Companion to [habitat-generation.md](habitat-generation.md) (the drum and the
> spine) and [crowd-simulation.md](crowd-simulation.md) (who lives in them).

## The model: gravity is a vector sum, and the engine is already shaped for it

Everything the operator described collapses into one rule:

```
g_total(position) = g_thrust + g_spin(position) + g_body(position)
up      = -normalize(g_total)
g_accel =  length(g_total)
```

- **`g_thrust`** -- uniform through the whole ship, pointing aft, magnitude
  `F/m`. Present whenever the drive is lit. This is what gives the spine
  gravity, and it is why a perpetually-burning fleet has a habitable spine.
- **`g_spin`** -- `omega^2 * r` radially outward from each rotating section's
  axis. Zero at the axis, maximum at the rim, zero when the section is despun.
- **`g_body`** -- the existing planet/moon radial term, already implemented.

**The engine already stores gravity in exactly this form.** `set_surface_up`
(`src/renderer/camera.rs:259`) takes an arbitrary `Vec3`, normalizes it, and
re-derives yaw and pitch so the view does not pop. `radial_step`
(`src/surface_move.rs:198`) takes `g_accel` as a plain scalar, computed at the
call site from `gravity_at(altitude)` (`src/lib.rs:4827`). Direction and
magnitude are already separate, already general, and the magnitude is already
surfaced to the F2 readout as `surface_gravity_now`.

So "gravity is the sum of its contributors" is not a rewrite. It is a new
expression feeding two slots that already accept it. What is NOT free is the
geometry bookkeeping around it: the radial model still assumes `r` is a distance
from a centre, which is the sign-inversion work documented in
[habitat-generation.md](habitat-generation.md).

## The consequence nobody asked for: under burn, the drum floor tilts

This falls directly out of taking the operator's realism seriously, and it is the
most important thing in this document.

If a drum spins about the ship's thrust axis, spin gravity points radially
outward and thrust gravity points aft. They are perpendicular. Their sum is
tilted, by `atan(thrust / spin)`, away from the drum floor's normal.

With the drum spinning for a full 1 g:

| Thrust | Total g felt | "Down" tilts from the floor by |
|---:|---:|---:|
| 0.05 g | 1.001 | 2.9 deg |
| 0.1 g | 1.005 | 5.7 deg |
| 0.3 g | 1.044 | 16.7 deg |
| 0.5 g | 1.118 | 26.6 deg |
| 1.0 g | 1.414 | 45.0 deg |
| 3.0 g | 3.162 | 71.6 deg |
| 5.0 g | 5.099 | 78.7 deg |

**Read the two ends of that table, because they are two different games.**

At a realistic interstellar cruise thrust of 0.05 to 0.1 g, the drum floor leans
by about three to six degrees. That is a subtle, permanent, barely-conscious
slope. Water pools very slightly aft. Plants lean. Dropped things roll, slowly,
always the same way. It is a beautiful, cheap, constant reminder that you live
on a moving ship, and it costs one vector addition.

At a 5 g emergency burn, "down" is 79 degrees off the floor. What was the floor
is now very nearly a wall, and everything in the drum falls aft at five gravities.
**That is not a problem to design around. It is the mechanism that makes a
high-g burn catastrophic**, which is exactly the drama the operator asked for.
The missile-evasion burn does not need scripted consequences; the consequences
are the vector sum.

### It also settles the drum's orientation

The drum axis should be **parallel to the thrust axis**, and the reason is in the
arithmetic rather than in taste.

With the axis parallel to thrust, spin gravity (radial) is perpendicular to
thrust gravity (axial) at every point on the rim, so the total is the same
magnitude and the same tilt everywhere: at 0.1 g thrust, 1.005 g and 5.7 degrees,
uniformly.

With the axis perpendicular to thrust, the spin radial sweeps THROUGH the thrust
direction as you walk around the circumference, so the two terms add at one point
and cancel at the opposite one. At the same 0.1 g thrust that is 1.100 g at the
aft-most point and 0.900 g at the fore-most: a 20 percent gravity swing depending
where you stand, with the tilt direction rotating too. Unlivable, and a nightmare
to build farms in.

So: drum axis along the thrust axis. Which is also what the references show.

### What this settles

- **The drum and the spine are not two gravity systems.** They are one field with
  two contributors, and the interesting behaviour lives in the interaction.
- **Cruise thrust should be LOW.** It is more physically honest (continuous 1 g
  to another star is an absurd energy budget even for a torch drive), and it is
  what keeps the drum usable while burning. A high cruise thrust would make the
  drum permanently unusable and the whole rotating habitat pointless.
- **High-g burns are an evacuation event.** Above roughly 0.5 g of thrust the
  drum is not a place you can stand. Crew go to acceleration couches in the spine,
  where thrust gravity is the only term and the decks are already perpendicular to
  it. That is why the spine exists and why its decks face the way they do.
- **Spinning down before a hard burn is a real decision with real cost.** Despin
  takes time and energy. Do it and the drum is zero-g plus thrust, so its floor is
  the aft endcap and everything not tied down has already fallen. Do not do it and
  you get the tilted 5 g above. Either way the farm suffers.

## What already exists (verified against code, 2026-09-15)

Rather more than expected, and in one case the design was already sitting in a
data file nobody reads.

**The EVA kit is already authored.** `data/docking.ron` carries a magnetic
grapple (`:54`), `maneuvering_unit` ("Jetpack attachment providing 6-axis
thrust", `:157`), `tether_line` ("50-meter retractable cable", `:168`) and
`mag_boots` ("Electromagnet-soled boots for walking on ferrous hull surfaces in
zero-g", `:179`). `DockingSystem` deserializes them as
`eva_equipment: Vec<ron::Value>` (`src/systems/docking.rs:28`) and **never reads
them**. That is four of the operator's six tools already named, described and
loaded. It is the schema anchor for the `locomotion_aid` rows below.

**`vertical_step` is already written as a jetpack.**
(`src/surface_walk.rs:295-338`.) It takes a thrust as a TARGET RATE and ramps
velocity toward it, works from either side so an upward burn arrests a fall, and
terminal-clamps free fall. Its own doc comment calls it "a jetpack spool, not a
teleport". What it lacks is a fuel hook and any axis but the radial one.

**Mag boots are the cheapest tool, and the precedent is the elevator.** The
elevator moves the player by moving the FLOOR under him
(`src/lib.rs:3482-3488`), needing no movement-mode work at all. Mag boots are the
same trick: hold the player to a surface and let the existing walking controller
run with `g_accel` near zero. First tool to build, by a distance.

**The `zero_gravity` status effect row already exists**
(`data/status_effects.csv:82`, `speed:0.6:multiply`), as does a live stressor
loop to hang a `high_g` sibling on.

### The five real gaps

1. **There is no lateral velocity anywhere.** Tangential motion is a
   DISPLACEMENT, not a velocity: `anchor += tangential.normalize() * step`
   (`src/lib.rs:4607-4609`). Only the radial axis carries state
   (`state.surface_vr`). Release the stick and you stop dead. **Zero-g is the
   exact opposite: release and you keep drifting.** This is the single biggest
   gap, and it is upstream of every dynamic tool.
2. **Two parallel movement systems, and zero-g lives in the primitive one.**
   The interior controller (`renderer/camera.rs:960-1140`) is world-Y with
   ladders, elevators and teleporters. The surface controller
   (`lib.rs:4369-4900` into `surface_move.rs`) has variable-up, ballistics and
   swimming but no ladders. They are mutually exclusive
   (`surface_translation_owned`, `camera.rs:1041`). Zero-g happens aboard ship,
   which is the side WITHOUT the good movement model.
3. **True zero g is unreachable by construction.** Both paths clamp: interior
   `g.clamp(0.01, 50.0)` (`camera.rs:921`) and surface `.max(0.01)`
   (`lib.rs:4830`). The comment explains why (a literal zero made jumps one-way
   trips), which is sound for today and is exactly the blocker for real zero-g.
4. **No long-range raycast against world geometry.** Everything shipping is
   short-range or AABB-only: crosshair interaction is capped at 3 m
   (`systems/interaction.rs:15`), the terrain mesh raycast is test-only, and
   rapier's `cast_ray` sits in a world that is never populated. **A grappling
   hook needs a target point and there is nothing to ask.** This is the grapple's
   real cost, not the tether maths.
5. **No data-driven item-verb path.** Equipment is data-driven for passive stats
   only (armor, speed, carry capacity, swing damage). The only thing an equipped
   item can DO is one hardcoded attack swing (`lib.rs:13502-13560`). Even the
   ability system is data for parameters and hardcoded for verbs
   (`systems/abilities.rs:283-300`). Without an `on_use` path, each tool is
   bespoke code, which is precisely what the three-primitive design is trying to
   avoid.

**Swimming is the right shape and the wrong integrator.** It already does
no-floor, look-direction control with a medium-specific speed cap. But it is a
neutral hold (`v_r = 0.0` unconditionally, `surface_move.rs:239-249`), so
momentum is exactly the thing it does not model. Copy its control scheme, not its
physics.

## G-load effects: one number, many consumers

`g_total` magnitude is a single scalar that every affected system can read. The
design rule is that nothing gets a bespoke "burn damage" path; they all read the
same value and apply their own tolerance curve from data.

Proposed tolerances, as data rather than code, per the Infinite-of-X rule:

- **Crew.** Comfortable to ~1.5 g. Degraded above ~2 g (reduced speed, slower
  actions). Grey-out and injury risk above ~4 g sustained. Couches raise
  tolerance substantially, which is what makes them worth building.
- **Plants.** Structural failure of stems and trellises, then root and water
  problems. A sustained 5 g burn killing a crop is the operator's own example and
  should be literally true, not scripted.
- **Animals.** Lower tolerance than humans for the small and the flighted;
  livestock need restraint.
- **Structures and equipment.** A load rating per piece. Racks, towers and
  anything tall and thin fails first. This is also the honest reason a habitat
  is built low and wide.
- **Loose objects.** Everything unsecured moves. This is where the cost lives,
  not in the gravity maths (see below).

### Where it plugs in, verified

Two of the three consumers already have a live environmental-stress pathway, so
this is an extension rather than an invention. (Note `gameplay-loop-map.md` says
otherwise on both counts; it is stale by several hundred versions and those gaps
closed in v0.745 and v0.749.)

- **Crew: about eight lines.** `src/systems/food.rs:450-616` runs one query with
  five live health drains (starvation, dehydration, suffocation, freezing, heat
  exhaustion). Every stressor has the identical shape, e.g. the cold one at
  `food.rs:543-550`: apply a condition, compute an amount scaled by gear resist,
  add to `health_drain`, record it if it is the worst. A g-stressor written that
  way inherits gear-resistance scaling, the death-cause tracker, death itself and
  the HUD readout for free. Add a `high_g` row next to the existing
  `zero_gravity` one.
- **Plants: about three lines, and the template is RF.**
  `src/systems/farming/mod.rs:1006-1008` already does
  `crop.health -= RF_HEALTH_PENALTY * home_rf * dt` for an ambient scalar field,
  and `:1011-1015` kills the crop to `STAGE_DEAD`. A g-load line beside it gives
  the operator's "plants die under a sustained 5 g burn" literally, not scripted.
- **Animals: the one genuinely new mechanism.** Livestock get `Health` and `Dead`
  and dead animals stop producing, but nothing environmental has ever damaged
  them; combat is the only path. The template to copy is the dormant
  `DisasterSystem` (`systems/disasters.rs:227-290`), which already does
  radius-falloff damage to anything with `Transform + Health`.
- **Equipment: from scratch.** There is no durability model. Item `durability` is
  display-only, `ShipSystems { hull_integrity, ... }` is never constructed, and
  the structural-load analysis module is orphaned. The one live precedent is
  `Container.damage_ratio` (`systems/inventory/containers.rs:159`).

`EnvironmentContext` (`src/ecs/components.rs:146-164`, built at
`src/lib.rs:5501-5561`) is the natural carrier: it already has exactly three
fields and one real gameplay consumer, so adding `g_load: f32` with a safe
default is short plumbing. Farming does not read it, so the plant path needs its
own read.

**Important: nothing can currently PRODUCE a g-load.** The ship is on rails.
`src/station/orbit.rs` is closed-form Keplerian kinematics evaluated from the
clock, with no forces, no mass and no integration. `PropulsionDef` exists as a
type (`systems/vehicles/propulsion.rs:9`), is referenced by nothing, and cites a
`data/propulsion.csv` that does not exist. So the burn state has to be AUTHORED
(a ship flight-plan with thrust over time) before any of the consequences above
have an input. That is the first piece of work in this arc, and it is the one
with no existing foundation at all.

## The locomotion toolkit: three primitives, not six features

The operator named grappling hooks, jet packs, mag boots, ropes, ladders and
handholds, and asked for movement to be "as fluid as possible". Built as six
features that is six special cases that interact badly. Built as primitives it is
one system and the list becomes data.

**Every tool named is a combination of three primitives:**

| Primitive | What it does | Without it, in zero-g |
|---|---|---|
| **Attach** | bind to a surface or point, continuous or momentary | you cannot stop |
| **Impulse** | apply a force, one-shot or sustained | you cannot start |
| **Tether** | constrain distance to an anchor | you cannot turn |

Which composes as:

| Tool | Attach | Impulse | Tether |
|---|:--:|:--:|:--:|
| Ladder / rungs | continuous, along a path | pull along it | -- |
| Handhold | momentary, at a point | push off | -- |
| Mag boots | continuous, any ferrous surface | -- | -- |
| Rope / safety line | anchored at one end | -- | fixed length |
| Grappling hook | ranged, on impact | reel along the line | variable length |
| Jet pack | -- | sustained, any direction, fuel-limited | -- |

So the schema is one `locomotion_aid` row with a primitive set and parameters
(range, strength, fuel, what surfaces it grips, whether the tether reels), and
new tools are rows. A magnetic grapple is attach-ranged plus tether plus reel; a
piton is attach-point; a winch is tether plus reel with no attach of its own.

**Mag boots deserve a specific note** because they are the bridge: they convert a
zero-g volume back into a walkable surface, which means the existing walking
controller handles it with `g_accel` near zero and an artificial "grip" holding
the player to the surface. That is the cheapest possible zero-g locomotion and
probably the first one to build, because it needs no new movement mode at all.

**The fluidity the operator wants comes from transitions, not from tools.** Six
tools that each require an explicit mode switch will feel worse than two that
blend. The design target is: push off a wall, drift, fire a grapple mid-flight,
swing, release, land on mag boots, walk. No modal prompts. That is a control-feel
problem and it will need real iteration, not a spec.

## What is genuinely expensive here

Almost none of this is the physics. To be explicit, since the operator asked
whether cheapness implied simplification:

- **Gravity direction and magnitude: free.** One vector add, one normalize.
- **Coriolis: nearly free.** One cross product, `-2 * omega x v`.
- **G-load effects: free.** One scalar compared against tolerance curves that are
  data.
- **Player locomotion: cheap.** Attach/impulse/tether are a handful of constraint
  operations on one body.
- **Every loose OBJECT under variable gravity: this is the expensive one.** It
  means waking rapier (currently never stepped, `ship/wall_collision.rs:4`) and
  giving it a rotating, accelerating reference frame with per-body forces. A
  drum's worth of unsecured items under a 5 g burn is a genuine rigid-body
  simulation.

The honest simplification, when it is needed, is therefore **not** to fake
gravity but to limit WHAT gets simulated as a free body: tag a small set of
objects as loose and physical, treat the rest as secured until a threshold is
crossed, and then convert them. Same trick as the crowd's promotion boundary in
[crowd-simulation.md](crowd-simulation.md), applied to objects instead of people.

## Open questions

- **Does the drum despin before a hard burn, automatically or by player choice?**
  Automatic is safer and less interesting; manual is a real decision with a real
  cost, and matches this project's teaching intent.
- **What is cruise thrust?** It sets the permanent floor tilt. 0.05 to 0.1 g
  gives a 3 to 6 degree lean, which reads as characterful rather than annoying.
- **Do NPC inhabitants respond to burns?** If the crowd runs to couches when the
  burn alarm sounds, the ship feels alive in a way no ambient chatter achieves.
  This is a good early use of the crowd timetable.

## Related docs

- [habitat-generation.md](habitat-generation.md), the drum and the spine
- [crowd-simulation.md](crowd-simulation.md), the promotion-boundary pattern this
  borrows for loose objects
- [infinite-of-x.md](infinite-of-x.md), why locomotion aids are rows
