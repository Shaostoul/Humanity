# Turret defense minigame (design capture)

Status: idea captured 2026-06-13 (operator). Not built yet. A marker turret
(`defense_turret` in `data/machines/home.ron`) sits on the garage hull beside the
mining-drone hangar as the future home of this loop.

## The pitch

The player home rides on a rotating ring of a mothership. Its outer hull (the garage
surface in-game) carries the solar array, the mining-drone hangar, and a player-operated
**defense turret installed above the garage, near the drone hangar spot**. The turret is
something you **build, maintain, and operate** to defend the mothership.

This is the engineering "to code" theme applied to defense: a turret is a real machine
(power, cooling, ammunition or charge, optics, a mount, a fire-control loop), so the
minigame teaches the same buildable-thing literacy as the plumbing and electrical do.

## The three verbs

- **Build** — assemble the turret from parts (mount, barrel/emitter, power feed, cooling,
  optics/sensor, fire-control). Real bill-of-materials like the aeroponic tower's `parts`
  list, sourced via the existing mine -> refine -> craft economy. Slots into build mode
  (content arc 4) once that exists.
- **Maintain** — it draws power (already modeled: a 20 W standby `Consumer`, priority 4 so
  it sheds first under deficit), heats up when fired (ties into the future thermal/HVAC
  loop), wears its barrel/optics, and consumes ammo or capacitor charge. Maintenance is a
  recurring sink for the production economy and a reason to keep the power loop healthy.
- **Operate** — the actual minigame: man the turret (walk up, press E, enter a turret view),
  track and engage incoming threats (debris, raiders, hostile drones) before they damage
  the ring. Skill + upgrades improve traverse speed, range, cooling, auto-track.

## Hooks into existing systems

- **Power loop (live, v0.437):** the turret is already a `PowerConsumer`; firing could spike
  its draw and visibly dent the live power balance / drain the batteries. Defense vs. power
  is a real tradeoff the player feels.
- **Mining loop (`src/systems/mining.rs`):** the drone hangar is right beside the turret;
  threats could target the drone or the hull. Ammo/charge is crafted from mined materials.
- **Combat system (`src/systems/combat.rs`, written, unregistered):** the damage/health and
  projectile bones may be reusable for threats and turret fire.
- **Interaction ([E], registered):** walk-up-and-operate matches the operable-machines arc.
- **Skills/quests:** a Gunnery / Defense skill line; quests to survive waves or build the
  turret.

## Why it fits the mission

Cooperative defense of a shared mothership is a multiplayer hook (content arc:
multiplayer/social): multiple players man multiple turrets on the same ring, or one builds
while another operates. It turns the homestead from purely productive into something a
community defends together, without abandoning the real-machine literacy north star.

## Suggested sequencing

Best built AFTER: build mode (to assemble it), operable machines (the [E] operate verb), and
ideally the live thermal loop (so firing heat matters). Until then it stays a labeled hull
installation. A minimal first slice once those land: a single fixed turret you press E to
operate against a scripted debris wave, scored, with power draw while firing.
