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

Where it plugs in is an open question pending the systems inventory: the
`EnvironmentContext` already assembled per frame (vacuum outside the hull,
weather cold, unbreathable air on power loss) is the natural carrier, since it is
already the channel through which environment reaches gameplay.

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
