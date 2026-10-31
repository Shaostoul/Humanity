# The farm on its own clock: exact long steps, then events, then computed when looked at

Status: **plan, 2026-10-30.** Nothing here is built yet. Companion to `docs/design/sim-frame-cost.md`
(where the farm's per-frame cost was measured and cut) and `docs/design/headless-sim-runner.md`
(time acceleration, the dt ladder and the step-invariance tests this plan reuses).

## Why

The operator, 2026-10-30, on FarmingSystem running every frame: "What if we only update plant data
once every hour or 6 or 24 or whatever we configure? I imagine this would help MMO servers." And:
"Since we can realistically simulate what the values are going to be excluding special events (IE:
meteorite strikes the water tank causing it to lost volume or something). Could we to a soft sim
out and then only calculate the value of the viewed objects along its timeline? Like assuming all
variables stay consistent there'd be no variation. I think of this idea kind of like 'fake
rendered frames' that newer games are using but, for other stuff."

- **Today** FarmingSystem advances every crop every frame: 2.4 ms a frame on a quiet machine with
  the showcase's 1,981 crops (after F1 and F2), 30 to 120 times a second, though a plant at real
  time changes by nothing anyone could see in a second.
- **An MMO server cannot do that.** A thousand homes of a thousand crops each, ticked every frame,
  is a machine's whole budget for one system.
- **The rules are known**, so a crop's state at any moment can be computed from its last known state,
  its rates and the time since. That is not a guess the way a generated frame is: it is the answer,
  exactly, until something interrupts. The offline catch-up (`save_load::catch_up_world`,
  `docs/design/offline-progression.md`) already does this for hours away in one step.

## What already handles a long step, and what does not

Already exact over any step: pest pressure (`pests::grow`, solved exactly: "a tick of any length
gives the same answer as many small ones"), the soil draw (`soil::uptake_fraction` follows the
growth clock), dehydration (linear, clamped at zero), the growth clock itself (moved through
`planted_at`).

**The four that depend on step size** (sim-frame-cost.md, F4):

1. **The season health record** (`health_seconds += health * dt`): a left sum, so a long step
   counts the whole step at the health it started with. Exact: integrate health along its path
   through the step (it moves linearly toward its cap, then holds).
2. **The 0.2 water-stress crossing:** a crop drying out crosses into stress partway through a
   step; a long step applies one regime to the whole of it. Exact: split the step at the moment the
   water level reaches the threshold (level / rate), then run each part in its own regime.
3. **Dawn and dusk:** light gates growth (`light_growth_rate`); a step spanning sunrise counts it all
   dark or all lit. Exact: split the step at the sun's crossing (the clock knows when).
4. **The feeder's doses:** one dose decision per step; over a long step the unit can run short in
   the middle. Exact: dose to cover the step's whole draw, or split at the moment the store would
   run out.

## The rungs (each shippable alone, each proven)

**Rung 1: every step exact.** The four fixes above. Proof: the farming golden state
(`src/systems/farming/golden_tests.rs`) run at one tick per frame and at one tick per second (and
the headless-sim-runner dt ladder: 1, 10, 60, 600, 3,600 s) ends with the same fingerprint, or
within a stated tolerance for the parts that are sums of floats. Until then the farm's results
depend on the frame rate, which is a correctness bug in its own right (a 30 fps and a 120 fps
player grow different gardens).

**Rung 2: the farm on its own clock.** FarmingSystem gathers the elapsed time and advances the farm
at an interval of its own, a setting (Settings > Gameplay: "Garden updates", every frame, every
second, every minute; default every second at normal speed). To keep the step's cost from landing
on one frame, the areas are spread over the interval (time-sliced: each frame advances the areas
whose turn it is, each by exactly its own time since its last advance, which rung 1 makes exact).
Budget: FarmingSystem at most 0.1 ms a frame on average and 0.5 ms on its worst frame with the
showcase's crops. Anything that reads a crop (the Garden page, a harvest, the plant meshes) first
brings that crop's area up to now, so nothing ever shows a stale crop.

**Rung 3: events, and crops computed when looked at.** Each crop and each shared resource knows
when its next change of regime happens: the water-stress crossing, the next growth stage, ripening,
dawn and dusk, the tank running dry (its level over the total draw), a pest becoming noticeable.
Those times go in one queue. Between them nothing is stepped at all; a crop's water, health and
growth at "now" come from its last state and its rates, in closed form, when something reads it.
An event brings the things it touches up to its moment and gives them new rates. Outside events
(watering, a harvest, a meteorite through the tank) are events too: they bring everything they
touch up to now first. Cost: proportional to events, not to crops times frames.

**Rung 4: the server.** Each home is simulated lazily: a home nobody is looking at costs nothing
until its next event or until someone touches it, then catches up exactly (rung 1 makes a catch-up
one long, exact step). The offline catch-up and the live farm become one code path.

## The hard part: coupling

Crops are not independent. They drink from one tank, breathe into one room's air (humidity and CO2
feed back into each crop's health cap through `humidity::health_ceiling` and `co2_ceiling`), share
an area's pests and an area's soil pH. Rung 2 keeps that coupling exact by advancing a whole AREA
(and its room's air) together. Rung 3 needs each coupled quantity to have its own closed form and
its own events: the room's air is a linear system (leakage, fans, the crops' breath), so it has an
exact exponential solution between events, and "the humidity leaves a crop's window" is an event.
That is the most work in this plan and the reason rung 3 follows rung 2 instead of replacing it.

## What stays per frame

What a person sees moving: wind in the leaves, the sky, the camera. Those are the renderer's and
are cosmetic (CLAUDE.md: cosmetic simulation runs locally and is never networked). The plant meshes
change only at a growth stage, which rung 3 turns into an event instead of a per-frame check.

## Tests

- The golden state at several tick rates (rung 1), and the dt ladder of headless-sim-runner.md.
- A rung-3 equivalence rig: the same home, the same script of outside events, run ticked (rung 2)
  and event-driven (rung 3); every crop's state compared at every event and at the end.
- The away-versus-live rig (headless-sim-runner.md) becomes the check that rung 4's lazy homes and
  the live farm agree.
