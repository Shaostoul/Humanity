# Offline progression

**Status:** designed 2026-09-21, not yet built.
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
  only with an explicit design pass and probably a cap.

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

## Open questions

- Is there a cap on how much elapsed time one login can claim? Unbounded is the
  simplest reading of "certain craft jobs could take forever", and a returning
  player after six months finding a finished world is arguably the point. A cap
  is worth revisiting only if some system turns out to behave badly at scale.
- Do offline hours consume inputs that were not reserved up front? A crafting job
  should reserve its inputs when it starts, which sidesteps the question.
- Does the character's offline upkeep cost anything, or is it free? Free is the
  simpler start.

## Related

- Crop growth multiplier: `src/systems/farming/mod.rs`, Settings > Gameplay.
- `docs/design/playable-assessment-2026-09-19.md` section 7, question 1.
- Timetable-shaped chores rather than countdown timers:
  `docs/design/crowd-simulation.md` rung 3, the same refactor.
