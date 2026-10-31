# The farm on its own clock: exact long steps, then events, then computed when looked at

Status: **rung 1 BUILT and measured, 2026-10-30** (the four places below exact, and five more found
on the way; what is still taken a step at a time is listed under rung 1 with its measured size:
`src/systems/farming/step_path.rs`, proven by `step_tests.rs`); rungs 2 to 4 are plan. Companion to `docs/design/sim-frame-cost.md`
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
`planted_at`). (Building rung 1 found that pest pressure was not, around `grow`: see rung 1.)

**The four that depend on step size** (sim-frame-cost.md, F4):

1. **The season health record** (`health_seconds += health * dt`): a right sum in fact (the tick
   moved the health first), so a long step counts the whole step at the health it ended with.
   Exact: integrate health along its path through the step (it moves linearly toward its cap,
   then holds).
2. **The 0.2 water-stress crossing:** a crop drying out crosses into stress partway through a
   step; a long step applies one regime to the whole of it. Exact: split the step at the moment the
   water level reaches the threshold (level / rate), then run each part in its own regime.
3. **Dawn and dusk:** light gates growth (`light_growth_rate`); a step spanning sunrise counts it all
   dark or all lit. Exact: split the step at the sun's crossing (the clock knows when).
4. **The feeder's doses:** one dose decision per step; over a long step the unit can run short in
   the middle. Exact: dose to cover the step's whole draw, or split at the moment the store would
   run out.

## The rungs (each shippable alone, each proven)

**Rung 1: every step exact. BUILT 2026-10-30** (`src/systems/farming/step_path.rs`, proven by
`step_tests.rs`). Each tick's step is cut where its regime changes and every piece is solved in
closed form, because within a piece the water level, the health and the growth age all move in
straight lines:

- **Light** (`step_path::light_spans`): the step cut at sunrise, sunset and the lamps' switch-off
  (06:00 and 18:00 on the solar dial, `solar::sun_factor`'s arc, and `LightingData::lamp_hours`
  after sunset), each part grown at its own light. The end hour is taken to the clock's f64
  seconds (`solar_hour_fine`): GameTime's f32 hour would move a step's sunrise by up to 7 ms.
- **Water** (`step_path::WaterPath`): dehydration, rain and the irrigation's hold are one clamped
  line, so the 0.2 stress line is crossed at most once, at the level over the rate, and each side
  runs in its own regime. (The old order, dry, then irrigate, then rain, also left a field held at
  its target in a drizzle one step's rain above it.)
- **Health and its season record**: toward its cap and holding there (or down to zero under
  stress), so `health_seconds` gains the exact integral (it was the END health times the whole
  step, a right sum).
- **Stage**: the running maximum of the effective progress over the path, the tick's own "only
  forward" rule. A crop that dies, or ripens with no picking window (the loop then skips it), stops
  at that moment: its water, health, records and planted_at are where they were then.
- **The feeder** doses for its target PLUS the step's draw (the draw follows the growth clock, so it
  is known before the health), urine only for the part of the draw before its month
  (`FertilizerDose::share_allowed`); a fed unit's cap reads the store the feeder holds, an unfed
  one's the store halfway through its draw.

Found by reading the whole tick, and fixed the same way:
- **Pests never arrived at a short step.** A new pest under `FORGET_BELOW` (1e-6) was dropped every
  tick, and a frame's arrivals at normal speed are about 4e-10, so no pest arrived at a step under
  about a minute (it depends on the pest's arrival rate and its hosts' share), while an hour's step
  kept them. Now only a pest with no host is forgotten.
- **Predation** was "grow the whole step, then take the predators' share": now solved with the
  breeding (`pests::grow_removed`, the Riccati equation's closed form), and an area's step is cut
  where a release runs out; a weed area's where a mulch rots away.
- **A fed bed's pH never moved at a short step**: a nitrifying pool under 1e-9 was dropped, and a
  frame of urine adds about 5e-10. The line is now 1e-15 and what falls under it reacts at once.
- **A crop sown at the step's end lived the whole step** (`step_path::SownThisTick`).
- **Outdoor pests' conditions** (the damp of the night, a row cover's day warmth), the crops'
  breath and photosynthesis and the irrigation's litres follow the step's light and stress, and
  the weed cap reads the crop's growth age halfway through the step.

**Measured** (`step_tests.rs`: the golden garden made to cross every regime, from 05:20 for 30 game
hours, at a 60 s, 600 s and 3,600 s tick; 188 crops die, about 400 change stage). BEFORE (the same
tests on the code as it was, at 600 s and 3,600 s against 60 s), without the air: `growing_seconds`
off by 540 s and 3,540 s and `health_seconds` by 8% and 50% (a crop dying partway through a step
was counted alive to the step's end), `planted_at` by 1,080 s and 4,680 s, a fed unit's N by 0.8%
and 5%, the field's cabbage caterpillars present at one tick length and missing at the other;
with the air, also 34 crops in a different stage (the grow-lit tower's). AFTER, without the air
the farm tracks: every stage the same, water exact, health to an f32 ulp (7.6e-6), stress crossings
to 0.04 s, `health_seconds` to 2.5e-5, a unit's N to 0.2% (where compost overshoots it) and its P
and K to 4e-5. With the air: as below.

Still taken a step at a time, each first order in the step (its error shrinks with the step) and
invisible at a frame's step or rung 2's default second; each is rung 3's, where the coupled
quantity gets its own closed form and its own events:
- **The air.** The crops' humidity and CO2 caps read the room's air as the last step left it, and
  the air steps after the crops on the step's average breath and photosynthesis, in its own
  15-minute slices (`humidity::step_rooms`). A grow room's air turns over in minutes, so a crop
  easing toward a moving cap sees it a step late: measured, a dying crop's death moves by about a
  tenth of the step (54 s at 600 s, 354 s at 3,600 s) and the home's CO2 by 0.3% and 3.7%. The
  largest spread left; it is "the hard part" below.
- **What the pests and the weeds read**: a crop's water stress, its unit's excess nitrogen and its
  room's warmth and humidity at the step's start; the weeds' emergence from the seed bank as the
  step began (the bank moves over years). Measured: aphids 0.25% apart at a millionth of a full
  infestation.
- **The feeder's dose for a step's draw lands at the step's start**, so compost's slow N and urine's
  acidity start working a step early (organic N 0.03%, a nitrifying pool 0.05%), and a new bank is
  averaged into the youngest organic cohort before it ages (its mean age 4% apart: it only decides
  when a 30-day cohort closes; the release rate changes only at a year). When the home stock runs
  out partway through a step the last of it goes in crop order, and a crop that dies partway was
  covered for its whole step's draw (the rest stays in its unit's soil).
- **The pollination and picking records** step on the stage the last step left (a record starts the
  step after a stage turns): one step's lag per stage change, never cumulative.
- **What a tick reads at its end**: the season and the weather's temperature (a field's climate
  factor), the grow lights' power draw (the electrical sim's load now), a burn's harm (the flight
  model's acceleration now, on real seconds), and the player's commands: a hand watering, a
  fertilizing or a pest control at the step's end acts over the whole step it ends (a sowing does
  not, above). Rung 2's "brings that crop's area up to now first" is the fix. The plumbing and
  electrical systems tick before the farm, so they see its irrigation demand a step late.

The golden digest moved (`golden_tests.rs`, regenerated): its 8-game-hour steps (speed 400) end at
08:00, 16:00 and midnight, so two steps in three spanned a sunrise or a sunset and the old tick
counted each all lit or all dark by its last moment, and every fix above changes some of its
state.

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

- Rung 1 (built): `src/systems/farming/step_tests.rs`, the golden garden at a 60 s, 600 s and
  3,600 s tick (the dt ladder of headless-sim-runner.md from its 60 s rung), with and without the
  air; `step_path.rs`'s own tests (one long step against 1,440 short ones, crossing every regime).
- A rung-3 equivalence rig: the same home, the same script of outside events, run ticked (rung 2)
  and event-driven (rung 3); every crop's state compared at every event and at the end.
- The away-versus-live rig (headless-sim-runner.md) becomes the check that rung 4's lazy homes and
  the live farm agree.
