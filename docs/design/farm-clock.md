# The farm on its own clock: exact long steps, then events, then computed when looked at

Status: **rung 1 BUILT and measured, 2026-10-30** (the four places below exact, and five more found
on the way; what is still taken a step at a time is listed under rung 1 with its measured size:
`src/systems/farming/step_path.rs`, proven by `step_tests.rs`); **rung 2 BUILT and measured,
2026-10-31** (each grow area on its own turn of the farm's own clock, the readers bringing it up to
now first: `src/systems/farming/clock.rs`, proven by `clock_tests.rs`); rungs 3 and 4 are plan.
Companion to `docs/design/sim-frame-cost.md`
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

**Rung 2: the farm on its own clock. BUILT 2026-10-31** (`src/systems/farming/clock.rs`, the tick in
`farming/mod.rs`; proven by `clock_tests.rs`). FarmingSystem gathers the elapsed game time and
advances each grow area at an interval of its own, a setting (Settings > Gameplay > "Garden
updates": every frame, every second, every minute, every hour; default every second, which at the
normal real-time speed is every real second; the 6 hours and the day the operator also named are
measured below and not offered yet). The interval is in GAME
seconds: the farm's step lengths, and so its one remaining first-order error (the air, below), stay
the same at any clock speed; at a clock fast enough that a frame spans the interval, every area
advances every frame, as before.

- **Time-sliced:** each interval is cut into 16 turns (8 until the in-game measurement below). Each grow AREA (a crop's `tower_id`: a tower,
  a bed, a field, "" for the hand-planted crops) has one of turns 1 to 15, the turn holding the
  fewest crops when the area is first seen; slot 0 is the air's. A frame advances the areas whose
  turn it crossed, each by exactly the game time it owes since its last advance (`AreaClock::owed`:
  the frames' steps summed, so a jump of the clock is not a step, as it never was), so an
  interval's work lands on sixteen frames, not one. Areas that advanced together owe the same to the
  bit, so a turn is one run of the farm's body (`FarmingSystem::run`, the old tick with a `Scope`).
  A crop carries a `FarmArea` mark (its area's index) so a turn picks out its crops by a number.
- **The air** steps on slot 0 and never waits longer than a game second, whatever the interval:
  the household breathes it, the atmosphere system judges it and the HUD shows it every frame,
  and it is cheap (no crops in it). It steps on every area's breath as that area's last advance
  worked it out (`clock::Held`), as one tick's air always stepped on that tick's average breath.
- **Everything every frame**, the farm exactly as before rung 2 (one run of the body over the
  whole farm, bit for bit): the interval at every frame or none published (a headless test, a
  store nobody set up), **the Dev page's "Turn off all reuse"** (`crate::reuse`: an area not yet at
  now is state kept stale on purpose), and a burn harming the crops (`flight::harm_per_sec`, on
  real seconds a frame at a time).
- **The irrigation and the humidifiers** draw from the tanks at each area's rate as its last
  advance worked it out, published every frame (`FarmClock::irrigation_l_day`); the grow lights'
  power follows their timer every frame.

**What reads the farm brings it up to now first** (every reader found, 2026-10-31):

| Reader | Where | How it brings its crops up to now |
|---|---|---|
| A player's command: harvest, harvest many, water, fertilize, Clear (picking), Plant tower, Plant bed, the showcase tower, weed control, a pH amendment, Hand-pollinate | `FarmingSystem::tick`, `touched` | The areas it names, or its crops' areas, advance to now (this frame's step included) before the commands run; then they run once more with no time, so what they draw and breathe from then on is what the command left (a harvested crop drinks no more) |
| A sowing from the backpack (its place is chosen inside it), a pest control (a ventilation reaches the room's air, a row cover the bees), a new bee colony (its hive serves several areas), the dev grow | the same | Every area, and for a pest control or a colony the air too |
| The save (`save_active_home`: the autosave every 120 s, quitting, a character switch, an edit's save) | `save_load.rs` | `farming::settle(.., 0)`: every area and the air exactly to the moment the save's clock records |
| A save loaded (`resume_home`, the offline catch-up's handover) | `save_load.rs`, `clock::hand_over_resume` | Every area's clock starts again from the loaded moment: its crops are as the save left them (brought to that moment before it was written), and the time away is the catch-up's, as before |
| The Garden page's data (the main screen's Inventory page, or a wall screen showing it) | `engine/garden_mirror.rs` | `settle(.., LOOK_FRESH_S)`: every area more than a second (and a turn) behind advances, and while the page keeps being drawn the farm's turns run on at most a second, so its next asks find nothing to do |
| The plant meshes (a stage, a tenth of health: the wilt) | `engine/home_meshes.rs` `rebuild_plant_meshes` | The same ask, while aboard (`aboard_station`: the home is near enough to be seen; on a planet or in space it is not), so on board the garden is never more than a second behind whatever the setting |
| The grow rooms' and the home's air (the body's warmth and humidity, the atmosphere system's breathable, the Air readout) | `survival_env.rs`, `atmosphere` | The air's own turn, at least every game second |

So the setting decides how often the farm advances what nobody looks at: on board, or with the
Garden page open, it is never more than a second behind, which the default already is. The
longer choices are for the farm while the player is off the station, and for the server's lazy
homes of rung 4. **The headless server does not run FarmingSystem** (`HumanityOS --headless` runs
the relay alone, `relay::run_relay`; no ECS system ticks there), so there is no server setting
yet; rung 4 is where a server's homes get their clock.

**What an area's advance carries** (the coupling, worked out from the tick, 2026-10-31). An area's
run takes, together, everything only that area reads or writes: its crops with their soil
(`CropSoil`), its pests (`SoilMemory::pests[area]`: the pressure, the releases, a row cover), its
weeds (`weeds[area]`, and a mulch's nitrogen tie-up drawn from its units), its soil pH units
(`ph[area]`), its banked organic nitrogen (`organic[area]`), its feeder's opened bags
(`feed_open[(area, item)]`), its crops' pollination and picking records, and its notices (short of
a nutrient, out of a pH window, waiting for a pollinator: each area keeps what it was told until
its own turn). None of those is read by another area, so slicing them is exact by rung 1.
What is shared is read where it stands, and is first order in the interval:

- **The air** (every grow room's and tent's, the home's own): one coupled whole (a room's air
  exchanges with the home's, a tent's with its room's, the household breathes in the home's), so
  it steps on its own turn, on every area's breath held from that area's last advance, and an area
  reads its room's air as the air's last step left it. Rung 1's own coupling (a crop's caps read
  the air a step late, the air steps on the step's average breath), now a turn apart.
- **The home's fertilizer stock** (`home_stock`, the feeder's bags): drawn as each area advances.
  It matters only when the stock runs out partway, when the areas that advance first get it (rung
  1: "in crop order"; now in turn order).
- **The tanks** (`WaterSupply`: water there or not, the irrigation running): read as each area
  advances; the plumbing sees the areas' held rates.
- **The grow lights' share-out** (`lighting::lamp_cover`): a light is shared out by what each
  plot's crops need (`area_needs`, over every area's living crops), so an area's cover depends on
  the other areas' crops as they stand; a crop that died in another area since that area's turn
  still counts until its turn.
- **The hives' colonies** (`SoilMemory::hives`): age on the air's turn; an area reads whether its
  hive's colony works as the colony stands.
- **A crop sown or spawned outside a command** (the dev showcase's replant, a test): it joins its
  area's next advance, which steps it from that area's last turn. A sowing by a command brings the
  area to now first, so its crop starts at its moment.

**Measured** (`clock_tests.rs`, 2026-10-31; each run against the farm advancing every area every
frame at the same frame length, `step_tests::compare`, held to `step_tests::tolerance` for a step of
the interval plus what is said here):

- **The golden digest does not move**, with no interval published and at the default (its ticks
  are 8 game hours, so every area is due every tick and the body runs once over the whole farm).
- **Every second and every minute**, two and a half game minutes across a sunrise in frames of a
  tenth of a second (so a second's eight turns land on different frames): every stage the same.
  Without the air everything to rounding, and the water (3.6e-5) and health (2.3e-3) apart are the
  reference's own: it stores each as an f32 every frame and at a tenth of a second the rounding
  runs one way, while the farm on its own clock rounds once a turn. With the air, the run ends
  75 s after the sunrise, in the rooms' fastest change of the day: at a second a room's CO2 0.16%
  apart and its water vapour 0.08%; at a minute a room's CO2 15% and a mushroom's CO2-capped health
  0.19, because the air steps on each area's breath as that area's last turn left it, so a room's
  drawdown at sunrise starts up to an interval late. A turn apart, first order in the interval.
- **Ten minutes and an hour**, step_tests' thirty game hours (188 crops die, about 400 change
  stage), frames of a game minute: every stage the same, within rung 1's bounds. Without the air
  at an hour: `growing_seconds` 0.05 s, health 7.6e-6, a unit's N 0.16%, aphids 1.2e-6 at a level
  of 3.6e-5 (the pests read each crop's water stress at its area's step start, rung 1's own
  term). With the air at an hour: a dying crop's death 192 s (rung 1 at a 3,600 s tick: 354 s), the
  home's CO2 3.8% (rung 1: 3.7%); at ten minutes 6 s and 1.1%.
- **Six hours and a day do not hold up yet** (`six_hours_and_a_day_measured`): over rung 1's
  bounds, because inside a step that long what rung 1 still reads at a step's start (the feeder's
  dose for the step, a new organic bank's age, a field's weather) grows with it. At six hours a
  unit's N 0.9%, a crop's health 0.05, a new organic bank's age 19%, and with the air a grow room's
  CO2 88%; at a day a unit's N 2.8% and the bank's age 48%. So Settings offers up to an hour
  (`clock::LONGEST_CHOICE_S`); the farm takes up to a day (`MAX_INTERVAL_S`) for rung 4 and for
  measuring. Rung 3's events are what makes the long ones exact.
- **The readers:** a harvest pressed between two turns picks the ripe lettuce (with the bring-up
  left out, in the same test, it finds it unripe and does nothing); with an hour's interval a
  reader that looks brings every area within a second at its first ask and keeps it within a
  second and a turn after; with reuse off every area and the air advance every frame and the end
  is the farm's with no interval to the bit.
- **Notices** (short of a nutrient, a pest appearing, out of a pH window) come at their area's
  turn: up to an interval late for an area nothing looks at.

**The cost** (`docs/design/sim-frame-cost.md`, "FarmingSystem on its own clock"): in the game
with the showcase's 1,981 crops at every second, FarmingSystem went from 3.5 ms of every frame to
0.24 ms on average, and its worst frame from 5 to 7.5 ms to 0.63 to 0.94 ms. The budget (0.1 ms
average, 0.5 ms worst) is not met yet: one whole advance of those crops is 3.5 to 4 ms, which at
every second is 0.12 ms a frame at 30 fps however it is sliced, and each turn has a fixed cost of
about 0.16 ms (the rooms' air map, the pollinators' reach, the passes over the marks). Both next
cuts are named there.

**Left for rung 3:** the air between events in closed form (its turn-apart coupling above is the
largest spread left); the readings a step still takes at its start, which is what keeps six hours
and a day from holding up; each crop's next change of look as an event, so the plant meshes and
the Garden page need not keep the whole home within a second to show it; a sown or spawned crop
joining an area mid-turn without the area's past.

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
an area's pests and an area's soil pH. The plan was for rung 2 to keep that coupling exact by
advancing a whole AREA together with its room's air. Building it (2026-10-31) found the air cannot
go with an area: every grow room exchanges with the home's own air, a tent with its room's, and the
household breathes in the home's, so the air of one area is the air of all of them. Rung 2
advances an area with everything only it reads (its crops, soil, pests, weeds, pH, organic N,
feeder bags, records: "What an area's advance carries" under rung 2) and steps the air as one
whole on its own turn, every second, on each area's breath as that area last advanced: the
coupling rung 1 already took a step at a time, now a turn apart. Rung 3 needs each coupled
quantity to have its own closed form and
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
- Rung 2 (built): `src/systems/farming/clock_tests.rs`: the rotation's rule; the reuse switch
  (every area every frame, to the bit); every second and every minute over a sunrise in frames of
  a tenth of a second, ten minutes and an hour over step_tests' thirty hours, each against the
  farm advancing every area every frame; the harvest between two turns (with and without the
  bring-up); a reader that looks; `farm_cost_per_frame` and `six_hours_and_a_day_measured`
  (ignored, measurements). `golden_tests.rs`: the golden digest at the default interval.
- A rung-3 equivalence rig: the same home, the same script of outside events, run ticked (rung 2)
  and event-driven (rung 3); every crop's state compared at every event and at the end.
- The away-versus-live rig (headless-sim-runner.md) becomes the check that rung 4's lazy homes and
  the live farm agree.
