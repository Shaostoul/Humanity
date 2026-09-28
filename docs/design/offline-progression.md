# Offline progression

**Status:** designed 2026-09-21. **BUILT 2026-09-25 for crops, builds under
construction and craft batches; 2026-09-27 for soil pH, the automated
machines, the mining drone and livestock** (single player, device clock).
Battery charge, tank levels and vessel contents are saved since 2026-09-27
and deliberately do not advance. See
"What is built" below. The server clock for multiplayer is what remains.
**Operator decision, 2026-09-21.** Verbatim:

> "Offline growth should be a toggle for both single player, multiplayer, and
> MMO. That's a cool feature. I guess that'd be like just a device clock check in
> how much time has elapsed since last login? Opens up a chance for cheating but,
> since we want to include all dev tools which are kind of like cheats seems kind
> of like a moot point. We should set that feature to be able to work for
> multiple things, like crafting. Certain craft jobs could take forever. We can
> justify it as the player character is continuing to live their life while the
> human is not playing. Kind of like a dual lives of the same person being lived.
> I think this is where the 1x speed makes perfect sense for long form gameplay."

## What it is

While you are not playing, your character keeps living. On the next login, every
system that advances on elapsed time is caught up by however long you were away.
A wheat crop planted before bed has grown by morning. A smelt job that takes
eleven hours is done when you come back.

This is the feature that makes **1x crop speed** a real choice rather than a
punishment. At 1x, real agricultural time, the fastest crop in `plants.csv`
takes about 4.7 hours, which nobody will sit through. With offline progression
the wait happens while you are at work, and 1x becomes the setting for long-form
play. The two features are designed together: see the growth multiplier in
`src/systems/farming/mod.rs` (`DEFAULT_CROP_GROWTH_SPEED`).

It is a TOGGLE, on every mode: single player, multiplayer and MMO.

## Not a farming feature

The operator was explicit that this must generalize: "We should set that feature
to be able to work for multiple things, like crafting." So it is not a farming
change with a crafting patch bolted on later. It is one mechanism that any
time-advancing system can opt into.

The shape that follows: a system declares that it advances on elapsed time, and
the catch-up pass hands every such system the elapsed interval once at login.
Systems that already compute state from `elapsed_seconds` against a stored start
time (farming's `planted_at` is exactly this) need almost nothing: advance the
clock and their next tick is already correct. Systems built as countdown timers
need converting to a start-time-plus-duration form first, which is the same
refactor the NPC timetable work needs (`docs/design/crowd-simulation.md`, rung 3).

Candidates, in the order they are worth doing:

1. **Crops.** Already start-time based. The cheapest one, and the one that
   motivated the feature.
2. **Crafting, smelting, refining.** The operator's own example. "Certain craft
   jobs could take forever" is a feature, not a problem, once you are not sitting
   there watching them.
3. **Animal growth and livestock**, when they exist.
4. **Machine production runs** already expressed as jobs.

## What must NOT advance offline

Named here so the decision is deliberate rather than an accident of which
systems happened to read the clock.

- **Vitals.** Hunger, thirst and energy must not drain you to death while you are
  at work. The fiction settles this cleanly: the character is living their life,
  which includes eating. Vitals resume from a reasonable state at login, they do
  not integrate eight hours of starvation.
- **Anything that destroys player property without a decision.** A crop dying of
  thirst offline, a base running its battery flat and freezing, a fire spreading.
  These are the difference between "I came back to progress" and "I came back to
  a ruin I could not prevent." If a system can consume or destroy, it participates
  only with an explicit design pass and probably a cap. Garden pests are in this
  class (decided 2026-09-26): an infestation would cut the harvest while nobody
  could respond, so pest pressure does not advance offline; the character's
  upkeep keeps them down. The soil nutrients the crops draw are not: growth made
  offline is paid for from the unit on return, as the crop's own feeding.

The honest tension: a fully simulated homestead SHOULD run out of water if the
pump has no power for eight hours, and the project's whole posture is realistic
first. The resolution is that offline is not the same as simulated: what the
character does while you are away includes ordinary upkeep. Realism applies to
the hours you are present.

## Cheating

A device clock check is trivially defeated by setting the system clock forward.
The operator weighed this and decided it does not matter: "Opens up a chance for
cheating but, since we want to include all dev tools which are kind of like
cheats seems kind of like a moot point." That is consistent with the project
already shipping Dev and Creative play modes to every player.

**But the clock source must differ by mode**, because the reasoning only holds
where the consequence is self-inflicted:

- **Single player:** the device clock. Cheating yourself is your business.
- **Multiplayer and MMO:** the SERVER's clock, always. One player advancing their
  own clock would otherwise mint resources into a shared economy, which is not
  cheating yourself, it is cheating everyone else. The relay already holds
  authoritative game time (`game_time_sync`), so this is a matter of reading the
  clock that already exists rather than building one.

## During development: every session starts from the default home (2026-09-25)

Operator, verbatim: "let's stay in the dev mode, I don't want to diverge
again so that we can make sure I always see what you build and what our
default is. Once the default is good then I'll be more comfortable
'playing.' Until then perpetual saving for normal gameplay like experience
is a liability."

The divergence was real: his save held 1,976 crops with 1,575 dead of
thirst, and because the showcase garden only replants an EMPTY garden, it
would never have come back, while a new player got a fresh one.

So Settings > Gameplay > "Start every session from the default home" is ON
by default (`fresh_world_each_launch`). While it is on, only the character
(name, look, outfit) is applied from the save; the home, garden, inventory,
builds, craft batches and clock start from the default every launch, and
offline progression does not apply. Saving writes only the character into
the existing save, so the progress on disk is left exactly as it was and
comes back when the setting is turned off. A save written with no progress
save yet is marked `progress_saved: false` and is never applied as a home.
The build editor still saves the LAYOUT into `data/`, which is the default
itself, so shaping the starting home keeps working. Revisit at launch.

## What is built (2026-09-25)

- **The clock is saved and restored.** Before this, `GameTime` started at zero
  on every launch while restored crops kept `planted_at` values read from the
  previous session's clock, so a garden rewound on restart (a code comment
  claimed the clock was saved; nothing wrote it). `save_active_home` now stores
  it, and `save_load::resume_home` restores it through a TimeSystem request
  channel (`time_restore_elapsed_request`). The clock never resumes behind the
  newest planting, which heals saves written before the fix.
- **The clock is NOT jumped forward by the time away.** Every system that
  reads the clock would then advance offline by accident, which is exactly
  what "What must NOT advance offline" forbids. Instead each system opts in
  inside `save_load::catch_up_world`:
  - **Crops:** living crops' `planted_at` moves back by the time away, so the
    growth-speed setting applies to those hours like any others. Water and
    health are per-tick and are not advanced (offline upkeep).
  - **Builds under construction:** progress advances, capped at the build
    time, so the ConstructionSystem's own next tick completes the build with
    its quest event and skill XP. Materials were consumed at the start, so
    nothing is spent offline.
  - **Craft batches:** time remaining counts down by the time away (floored at
    zero, so the batch delivers on the next tick through the normal path).
    Inputs were spent when the batch started.
  - **Soil pH (2026-09-27):** lime, sulfur and nitrifying ammonium that were
    still reacting keep reacting, since soil chemistry is not upkeep and
    destroys nothing (crop health is not integrated offline). `resume_home`
    hands the time away to the farming tick
    (`farming::soil_ph::hand_away_secs`), which steps it once at the player's
    growth speed and Soil pH setting, because those reach the DataStore only
    after the resume. The acidity of the growth made while away is not in it:
    that growth pays its nitrogen on the first tick back and its acidity
    reacts from then on. Weeds and pests do not advance (they cost crop
    health the player could not answer).
- **Craft batches are saved at all**, which they were not: a batch's inputs
  are consumed when it starts, and a restart dropped the batch, so every
  restart destroyed whatever was mid-smelt. The CraftingSystem publishes its
  list (`active_crafts_export`) for the save and takes restored batches
  (`restore_active_crafts`). A machine batch is keyed by the machine's
  instance id rather than its entity, which also fixed a machine starting a
  second batch beside its first whenever world entry respawned it.
- **Builds are saved at all**, which they were not: `WorldSave.constructions`
  existed from the start with nothing writing it, so every structure the
  player built was discarded at exit after its materials had been spent.
- **Toggle:** Settings > Gameplay > "Keep growing while away", on by default,
  persisted in `config.json` as `offline_progression`. Everything below
  opts in behind it; with it off the saved state comes back exactly as saved
  and nothing moves on.
- **Never silent:** a "While you were away (8 h 12 min), 12 plants kept
  growing." notice on load.

### The automated machines, the drone and livestock (2026-09-27)

- **Automated machines** (the Barn's `AutoRefine` machines: the grain mill,
  smelter, workbench, sawmill, fuel refinery, vehicle assembler).
  `resume_home` hands them the time away (`crafting::away::AwayWork`), and
  the CraftingSystem runs it once the machines exist, before it starts any
  batch of its own. The hours are run moment by moment by the session's own
  rules: a machine starts a batch when its inputs are on hand, spends them
  then, runs the recipe's craft time, files the product in home storage
  and starts again; it rests at its keep target (the mill's 20 flour); what
  one machine makes feeds the next (ore to ingot to hammer) from the moment
  it is made; a batch in flight at the save lands when it was due and the
  machine carries on from there; a batch still running when the time runs
  out is left running, where the player finds it. The outputs get the XP
  and quest credit a session batch gets, without the sound. One notice
  lists what they made ("the home's machines made 21 Flour").
- **The drone** (`mining::advance_away`). The trip in flight finishes, and
  while "Keep mining" is set the drone keeps flying the same trip until its
  asteroid is mined out, as in a session. Each haul lands in the backpack
  and is usable by the machines only from the moment it landed. A trip still
  in the air when the time runs out is left in the air. The asteroids, the
  drone with its cargo, and the standing order are saved now: all three were
  rebuilt fresh at every launch, which refilled every asteroid and lost the
  ore in a flying drone's hold.
- **Livestock** (`livestock::timers_after_away`). Each homestead animal's
  yield timer (egg, milk, wool) is saved by its herd slot ("chicken#0") and
  moves on by the time away, to ONE yield waiting: the same cap as a player
  at home who never collects, so the away time cannot give more than the
  hours at home would. Before this the herd respawned ready at every launch.
  The livestock system has no hunger, illness or growth yet, so nothing else
  is advanced, and when it gets them they belong to "What must NOT advance
  offline": the character feeds the animals. Nothing dies because the player
  was away.
- **Power offline.** An electric automated machine may draw only power the
  home could have spared: the Usage meter's day balance, what the home makes
  on its own (each panel's yield at the site, a turbine's site average, no
  backstop genset) less what it uses (every machine's day average draw), for
  the life support mode the player has chosen
  (`crafting::away::day_power_balance`). It is paid out no faster than the
  home made it: by any moment of the time away the machines have drawn at
  most the spare watts times the hours so far. A home that makes less than
  it uses has nothing to spare and runs no electric machine while away, and
  the notice says so instead of leaving an idle machine unexplained.
  Batteries are not a source: they move a day's power from noon to night,
  they do not add to it. No automated machine in the shipped homes has a
  power role today, and neither shipped home makes what it uses (the meter
  of 2026-09-27), so this rule bites the moment one does
  (`the_shipped_homes_have_no_power_to_spare_while_away` pins it).
- **What deliberately does not move on:** tanks do not refill offline (a
  machine's tap water comes out of what the tanks held), the backstop genset
  burns nothing, the weather and fires do not run, predators do not hunt the
  herd, and a machine never runs on stock that was not there. The inert
  `ManufacturingSystem` facility counter has no spawned facilities and
  produces nothing, so there is nothing to catch up there.
- **What the home's machines hold is saved (2026-09-27), and does not move on
  while away.** Each battery bank's charge, each water tank's litres and each
  machine vessel's contents (the genset and refinery fuel drums, the grain
  silo, the pantry, the freezer, the furniture drawers, with their residue and
  toxic history) are saved by machine instance id
  (`WorldSave.machine_levels`, `engine::machine_levels`). None of it was
  before: every restart put each bank and tank back at half, which undid the
  night's discharge or the day's charge, and emptied every vessel, which
  destroyed whatever was stored in it; entering the world, which respawns
  every machine, did the same within a session, and now carries the levels
  across. By the rule above none of them is advanced by the time away: a
  bank neither charges nor runs flat while the player is out (the panels are
  not simulated offline, and a flat bank is the "battery running flat"
  case), a tank does not refill, and a drum is not burned. They come back as
  saved, and the automated machines' tap water while away comes out of the
  tanks as saved. The home's own air (oxygen, carbon dioxide, humidity) and
  each grow room's air were already saved in `SoilMemory` and are not
  advanced either, since the air runs on the game clock, which does not
  jump. Not saved on purpose: the fraction of a fuel unit a running genset
  has burned toward its next whole unit, and the fraction of a litre the
  plumbing carries between ticks, both smaller than one unit.
- **Applying the same save twice is the same result.** The launcher applies
  the save again when the character is picked, so on a character select the
  home's storage (the Barn) now comes back with the save as well
  (`save_load::after_resume`); it used to keep the previous state, which let
  a rewound backpack and a kept Barn both hold the same goods.
- **Not yet:** the server clock for multiplayer and MMO saves (none exist
  yet). A dead animal is not saved and comes back alive and ready.

## Open questions

- Is there a cap on how much elapsed time one login can claim? Unbounded is the
  simplest reading of "certain craft jobs could take forever", and a returning
  player after six months finding a finished world is arguably the point. A cap
  is worth revisiting only if some system turns out to behave badly at scale.
  (2026-09-27: still unbounded in time. The machines and the drone carry only
  a safety bound on the work one return may run, 200,000 machine batches and
  100,000 drone trips, which a real home never nears because inputs and
  asteroids run out first; it stops a save with an input-free recipe from
  looping.)
- ~~Do offline hours consume inputs that were not reserved up front?~~
  **Answered 2026-09-27 by what was built: no input is reserved ahead, and
  none needs to be.** A batch spends its inputs when it STARTS, away exactly
  as at home (a manual craft, a scaffold and a machine batch all do). The
  time away runs the automated machines moment by moment, so a batch starts
  only on stock that was on hand at that moment: the backpack, the Barn,
  what an earlier batch made, drone ore from the moment it landed, tap water
  from what the tanks held. Offline hours therefore consume only inputs that
  existed, never more than a session would, and a machine short of an input
  simply waits, as it does at home.
- Does the character's offline upkeep cost anything, or is it free? Free is the
  simpler start.

## Related

- Crop growth multiplier: `src/systems/farming/mod.rs`, Settings > Gameplay.
- `docs/design/playable-assessment-2026-09-19.md` section 7, question 1.
- Timetable-shaped chores rather than countdown timers:
  `docs/design/crowd-simulation.md` rung 3, the same refactor.
