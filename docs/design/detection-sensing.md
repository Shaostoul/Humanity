# Detection & sensing -- multi-modal "seeing" (sight / RF / smell / sound / seismic)

> **Status:** DESIGN ONLY (not built). Captures the operator's vision so we can move on until we know how
> to solve the PERFORMANCE problem (this must be cheap enough for MMO servers). The telecom RF emission
> (`docs/design/telecom.md`, the `RfEmitter` from v0.620) is ONE signature feeding this layer; this doc is
> the broader sensing model. Nothing here is wired yet -- it's the idea, the constraints, and the open
> questions.

## The core idea

A thing is detected when a **sensor** picks up a **signature** it emits, modulated by the environment and
the sensor's capability. It is deliberately MULTI-MODAL: there is no single "stealth" stat -- you are
loud or quiet PER CHANNEL, and a given watcher only senses the channels its hardware/biology supports.
This is the awareness/stealth layer (the bridge into the otherwise combat-free game): it is about being
SEEN or not, not about weapons.

The design payoff: the variety of detection channels drives material + tech + tactics choices. If the
aliens on a mission detect RF easily, you bring zero-RF gear (wired comms, no power-armor WiFi). If they
hunt by smell, you mask your pheromones / move downwind. If they are blind but feel the ground, you move
slow. You CAN bring the loud-but-powerful loadout (RF-blasting power armor) -- just be prepared for the
higher detection chance, and bring a good team and good weapons. Tradeoffs, not a single right answer.

## The channels (each a Signature kind + a matching Sensor kind)

| Channel | Emitted by | Sensed by | Modulated by | Notes |
|---|---|---|---|---|
| **Sight (visual)** | any lit object; a moving silhouette | eyes / cameras | LIGHT level, smoke/fog, distance, line-of-sight, the seer's optics | the default channel; needs light to work |
| **Light source** | a flashlight, laser, glowing panel, muzzle flash | anything that sees | darkness makes it MORE visible (a laser down a dark corridor screams) | a light is both useful + a giveaway |
| **RF** | powered electronics: WiFi, power armor, vehicles, the `wifi_router` | RF-capable sensors only (an advanced alien can "see" RF; a human cannot) | strength (higher RF = easier), distance, shielding/walls | not everything senses RF, but it always EXISTS -- you can be RF-loud + think you're hidden |
| **Pheromone / smell** | creatures, food, the player; deliberate pheromone emitters | scent-capable noses | AIR: disperses at a rate set by atmosphere + WIND (direction matters -- downwind carries far, upwind little) | pheromones can ATTRACT / REPEL / MARK |
| **Sound** | footsteps, machine hum, gunshots, explosions | hearing | distance, walls, ambient noise floor; gunshots + explosions are LOUD spikes that draw attention | |
| **Seismic** | footsteps, vehicles, explosions, heavy machinery | ground-vibration sensors / creatures that feel the floor | distance through the substrate, ground material | "seeing" through vibration in the ground |

A signature has a **strength** + a **channel**; a sensor has a **channel** + a **sensitivity** (+ a range,
+ optionally a facing/cone for sight). Detection of one (emitter, sensor) pair = does the signature's
strength, after environmental falloff to the sensor's position, exceed the sensor's sensitivity threshold.

## The hard constraint: PERFORMANCE (this is why it is design-only)

A naive model -- every emitter raytraced to every sensor every frame, with real gas/wind/acoustic
physics -- would kill the frame budget and is impossible at MMO scale (N emitters x M sensors x channels
x frames). The whole point of this doc is to NOT do that. Candidate cheap approximations to evaluate:

- **Coarse tick, not per-frame.** Re-evaluate detection a few times a second, not every frame.
- **Spatial buckets / a grid.** A sensor only considers emitters in nearby cells (broad-phase), so it is
  O(emitters x local-sensors), not O(NxM).
- **Analytic falloff, not simulation.** Distance-squared (or per-channel curve) attenuation + a few
  environment multipliers (light, fog, wind dot-product for smell, wall-count), NOT a fluid/acoustic sim.
- **Per-channel "loudness" fields baked coarsely** (e.g. a smell field that diffuses on a low-res grid a
  few times a second) rather than per-particle.
- **Detection as a SCORE that ramps**, not a binary raycast -- cheap to accumulate, gives "partially
  aware -> alerted" without precise geometry.
- **LOD by importance.** Full detail near the player; cheap/none for distant NPC-vs-NPC.

The open research question (the reason we doc + defer): which approximation gives believable multi-modal
stealth at MMO scale without a physics bill. Likely a hybrid: analytic point-to-point for RF/sound/sight
+ a coarse diffusing grid for smell/seismic.

## How it connects to what exists

- `RfEmitter` (`src/ecs/components.rs`, v0.620) is already the RF signature for a powered wireless device;
  the FarmingSystem already reads summed RF (the plant-harm consequence). A sensing layer would add
  `Sensor`s that read the same emitters -- reuse, don't duplicate.
- `AtmosphereSystem` (`src/systems/atmosphere.rs`) owns air + (future) wind, which the smell/pheromone
  dispersion needs.
- `Weather` (wind speed/direction) feeds smell dispersion direction.
- Light level (renderer / time-of-day / the per-home lights) feeds the sight + light-source channels.

## Staged plan (when we tackle it)

1. **Components + a coarse SensingSystem.** `Signature { kind, strength }` + `Sensor { kind, sensitivity,
   range }` + `SensingSystem` on a slow tick doing broad-phase + analytic falloff -> a per-sensor
   "detected" list / alert score. Start with RF + sound (point-to-point, cheapest).
2. **Sight + light** (line-of-sight + light level; reuse the renderer's light data).
3. **Smell / pheromone** (a coarse diffusing field keyed to wind from Weather; attract/repel/mark).
4. **Seismic**, and NPC/enemy AI consuming the alert score (the actual stealth gameplay).

## Open questions for the operator

- Is the first visible slice a **stealth HUD** (you see your own per-channel loudness + nearby emitters),
  or **enemy reactions** (NPCs that alert when they detect you)? (The HUD is far cheaper + ships first.)
- How "real" should smell/seismic be -- a coarse diffusing grid, or just an analytic radius for v1?
- Do walls fully block a channel, or attenuate it (per-channel)?
- Pheromones: player-deployable items (lures/repellents), or only creature-emitted for now?
- Per-channel detection: a hard threshold (detected/not) or a ramping alert score (partial awareness)?
