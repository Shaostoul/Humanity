# Drone mining: distant asteroids are the ship's ore, drones fetch it, and anyone may fly one

Status: a DESIGN, written 2026-11-06. Nothing new in it is built. The operator's words are quoted
with their date; everything else is a **Proposal**, this document's reasoning, never his. It is
part of the mothership plan ([mothership-master-plan.md](mothership-master-plan.md), section 1.1,
question 8).

The operator, 2026-11-06, agreeing that no ore nodes belong aboard the ship:

> "Correct. Resources come from distant asteroids we send the drones to collect."

> "We can allow players to pilot drones and add a drone mining game for mining but, I don't know
> how we should do it. Maybe a cargo capacity and fuel limit."

## 0. In short

- **Asteroids are the only ore source aboard.** The wild ore outcrops and veins on the ship are
  removed (master plan rung 1); the Survey quests read their ore from the drone loop.
- **Automated trips stay the default**: the Mining panel's commission, the hold's manifest, Keep
  mining, Repair and the offline catch-up all stay. What changes: a trip takes the time its
  distance and the drone's engine really take, and a trip is planned against two limits, the
  **propellant** (the delta-v budget, the rocket equation) and the **hold** (kilograms).
- **Piloted mode is optional**: the person takes the controls at the rock and flies the approach
  and the mining themselves, in a small 3D scene around one asteroid, with the same two limits
  and with hazards that make the choices real: which rock, how much to carry, when to turn back.
- **What is realistic about remote piloting**: it depends only on distance. Out to about 60,000 km
  the signal's round trip is under 0.4 s and the person can fly directly; to about 390,000 km (the
  Moon's distance) it grows to 2.6 s, where one study measured exploration taking 150% longer; past
  that, piloting from the ship turns into giving orders the drone carries out. Realistic mode keeps
  the delay; Simplified mode drops it (section 3).
- **Between the stars, every rock is far and fast**: the nearest is about 1 AU away on average and
  moves past the fleet at its cruise speed, which no rocket can match. Drones reach those rocks the
  way small craft reach anything out there: by the jump, decided for small craft on 2026-11-03
  (section 2).
- **Upgrades come through the fleet's progression** (the crew votes and builds, game-setting-and-intro.md
  section 3.4): tank, hold, engine, cutter, anchors, sensors, autonomy, shielding, a jump module.
- **Seven rungs**, the first one data only (section 10).

## 1. What exists (verified against the code, 2026-11-06)

| Piece | Where | What it does today |
|---|---|---|
| The drone loop | crates/humanity-systems/src/mining.rs (`DroneSystem`, 1,842 lines) | One drone per player. A commission names one asteroid and a manifest (ores and units, at most `DRONE_CAPACITY` = 10 units). Phases Outbound, Mining, Returning, Done. Ore is taken from that one finite asteroid (`AsteroidBody::take`); a mined-out asteroid is deleted. Cargo lands in home storage. |
| Trip times | mining.rs:339-341 | `OUTBOUND_SECS`, `MINING_SECS`, `RETURNING_SECS` = 5 s each, "Dev-scale; tune later / move to data". The drone carries `home_pos` and `target_pos` "so travel time scales with distance", but nothing reads them for time yet. |
| Running costs | data/mining/drone.ron (`DroneSpec`) | Sized as a DJI FlyCart 100, a rotor cargo drone: 85 kg payload, a 4,264 Wh battery charge a trip from the home's grid at 3 kW, a service every 100 trips (carbon fibre sheet and rubber sheet). Its own comment: "A drone in vacuum would fly on thrusters, not rotors; when the game's drone gets its own design these numbers change with it." |
| Two modes | Settings > Gameplay > Mining drone (`MODE_KEY`) | Realistic: no charge, no launch. Simplified (the default): the dock charges anyway, half the wear. |
| Standing order | "Keep mining" (`auto_mine_order`) | Relaunches the same trip after each delivery until the asteroid is mined out. |
| Time away | `mining::advance_away` | The drone keeps flying its standing order while the player is offline, bounded by the asteroid's real stock (docs/design/offline-progression.md). |
| The Mining panel | src/gui/pages/inventory.rs (the Mining section, the per-asteroid window) | The asteroid's window allocates the hold and launches; the drone's row shows the trip, Stop (Keep mining off), Repair (the service) and Cancel (a launch waiting for the battery). |
| The engine bridge | src/engine/drone.rs | The Settings mode, Repair and Cancel into the system; the readout onto the panel. |
| Asteroids | data/asteroids/belt.ron, types.csv | Three rocks a new world starts with (M-12, S-7, C-3), 30 to 90 km away, keeping station with the ship; five classes in types.csv. |
| The map | src/gui/pages/maps/ | Draws each asteroid at its position (Maps increment 1, 2026-10-21) and the drone on its way. |
| The drone hangar | data/machines/home.ron:744 (`drone_hangar`, "1 mining drone") | A machine in the homestead. |
| Voxel asteroids | crates/humanity-terrain/src/asteroid.rs | Seeded voxel volumes at 0.5 m a voxel in a sparse octree, ore veins by class, `remove_voxel` for mining, a mesher. Tested, not connected to the game (gameplay-loop-map.md calls it "tested-but-disconnected"). |
| The skill | data/skills/skills.csv | `mining`: "Extract ore and stone from rock and asteroids". |

So the loop, the panel, the modes, the costs and the offline catch-up exist; the distances, the
propellant, the drone's space design, the piloted scene and the asteroid field do not.

## 2. Where the rocks are decides everything (realism first)

The operator's standard for the ship, 2026-11-06: "We want to get close to what a real-life
spaceship of this design might be like." For mining that means distances and speeds:

| Where the fleet is | Where the rocks are | How a drone gets there |
|---|---|---|
| Rocks the fleet carries along | parked beside the ship and moving with it, like today's three (30 to 90 km) | by rocket, minutes to hours; no signal delay |
| In a star system (today's data: the ship orbits over Earth, data/stations/home.ron) | in belts and swarms, each rock on its own orbit, usually far from the next | by rocket, hours to weeks, delta-v of hundreds of m/s to km/s; delay grows with distance |
| Between the stars (the setting's lore; open as game-setting-and-intro.md question 2) | interstellar objects at about 0.1 to 0.2 per cubic AU (game-setting-and-intro.md 3.2, citing Do, Tucker and Tonry 2018) | **only by the jump** (below) |

**Between the stars, computed.** At 0.1 to 0.2 objects per cubic AU, the nearest one is on average
1.2 to 0.95 AU away (the mean nearest-neighbour distance of randomly placed points is about
0.554 times the density to the power of minus one third). An AU of light takes 499 s, so the round
trip is 16.6 minutes. And the rock is not waiting: the fleet passes it at its cruise speed, about
15,000 km/s at 5% of light speed. A rocket would need that much delta-v to stop beside it. So a
drone that mines between the stars is a **small craft that jumps**: small craft jump (decided
2026-11-03, game-setting-and-intro.md 10.1), and "a jump ends moving with whatever it jumped to"
(game-setting-and-intro.md 3.1), which is exactly the speed match a rocket cannot give. The jump
makes a far rock reachable; the rocket budget below covers the last kilometres and the work there.

**Recommendation:** model all three. The asteroid rows name their frame (carried with the fleet;
on an orbit in a system; in an interstellar field) the way the master plan asks every spawn row
to name its frame, and the trip planner picks rocket or jump by the frame.

## 3. Remote piloting and the speed of light

A signal to the drone and the picture back take twice the distance over the speed of light
(299,792 km/s):

| Distance | Round trip | What a pilot feels |
|---|---|---|
| 60 km (today's rocks) | 0.0004 s | nothing |
| 10,000 km | 0.067 s | nothing |
| 60,000 km | 0.40 s | the edge of "low latency" |
| 384,400 km (the Moon) | 2.56 s | a clear lag; move, then wait |
| 1 AU | 998 s (16.6 min) | no direct control |

**What the research says.** Mellinkoff, Spydell, Bailey and Burns, "Quantifying Operational
Constraints of Low-Latency Telerobotics for Planetary Surface Operations" (arXiv:1710.01254, v2
revised 16 January 2018), drove a rover in a lunar analogue site and compared the low-latency case
(0.4 seconds or less) with "a 2.6 second delay" (the far side of the Moon driven from Earth): "The
results from this experiment show a 150% increase in exploration time when the latency is
increased to 2.6 seconds. This indicates that such a delay significantly complicates real-time
exploration strategies." (Abstract, read 2026-11-06.)

**So, realistically** (Proposal, from those two points):

- **Direct piloting** out to a round trip of 0.4 s (60,000 km from the person).
- **Delayed piloting** from 0.4 to 2.6 s (to about 390,000 km): the controls and the picture arrive
  late by the true light time, and the scene shows where the drone will be when the command
  arrives (a predicted ghost, the standard aid for delayed teleoperation).
- **Orders only** past 2.6 s: the person marks targets and cuts on the rock's model and the drone
  carries them out; the picture arrives as it really would.
- **To fly a far rock directly**, be near it: a person who rides along in their own small craft
  (jumping with the drone between the stars) pilots from there with no delay. The delay is measured
  from the person, not from the ship.
- **Simplified mode drops the delay** at every distance.

## 4. The two limits: propellant and hold

**The rocket equation decides every trip.** A drone of dry mass m_d with propellant m_p and an
engine of exhaust speed v_e (the specific impulse times 9.81 m/s2) can change its speed by
v_e x ln(mass before / mass after). It flies out empty and comes back loaded, so the cargo it can
bring home falls as the trip's delta-v grows, and past some distance it cannot come back at all.

**A worked example** (the numbers are ASSUMED for illustration, Proposal: a 300 kg drone with
200 kg of propellant and a 300 s engine, so v_e is 2,942 m/s and the empty drone's whole budget is
1,503 m/s; 10% of the propellant held in reserve). Each leg's delta-v is what it takes to speed up
and slow down again (twice the cruise speed for a rock carried with the fleet; more for a rock on
its own orbit):

| Delta-v each way | Propellant used going out | Most cargo it can bring home |
|---|---|---|
| 50 m/s | 8 kg | 9,710 kg (the hold is the limit) |
| 100 m/s | 17 kg | 4,423 kg (the hold) |
| 300 m/s | 49 kg | 925 kg |
| 500 m/s | 78 kg | 250 kg |
| 700 m/s | 106 kg | none: it cannot come back even empty |

(Computed.) A near rock is limited by the hold; a far one by the tank; one rock too far is a
one-way trip. That is the operator's "cargo capacity and fuel limit", and it gives the three
choices the game is built on:

- **Which rock**: near and poor, or far and rich; spinning or calm; a solid body or a loose rubble
  pile; surveyed or a guess.
- **How much to carry**: every kilogram aboard on the way home costs propellant; the panel shows the
  largest load that still gets home with the reserve.
- **When to turn back**: mining takes time and the cutter heats and wears; the panel shows the point
  of no return as a mark on the propellant gauge.

**Trip time** follows from the drone's thrust: burn to a cruise speed, coast, burn to stop. With an
assumed 0.5 m/s2: 60 km at 50 m/s is 21.7 minutes one way, at 150 m/s 11.7 minutes (and three
times the delta-v); 1,000,000 km at 150 m/s is 116 minutes (computed). Realistic mode uses the real
times; Simplified mode shortens them by the game's simplified time scale.

**Propellant is made aboard** (Proposal): water, split into hydrogen and oxygen or used straight in
a water thruster, from icy and carbonaceous rocks (C-3 is the start). The drone's first loop closes
on itself: some of every icy haul refuels the next trip. Simplified mode may treat propellant as
free, the way its dock already charges for free.

## 5. Automated trips (the default)

The loop that exists, made real:

1. **Commission** at the Mining panel or from the Maps page: pick a rock, set the manifest; the
   panel now shows the trip's delta-v, the propellant it takes, the largest safe load, the time out
   and back, and the round-trip light delay.
2. **Refusal with the reason** (Realistic): a trip that cannot come back with its reserve is not
   launched, and the panel says which limit stops it ("too far for a full tank: 640 of 610 m/s").
   Simplified trims the manifest to what fits and says so.
3. **The flight** runs on the phase machine that exists, its phase times now computed from distance
   and thrust (the 5 s constants go to data as the Simplified floor).
4. **Delivery** at the ship's ore intake (section 8), then home or to the fleet's stores.
5. **Keep mining**, Repair and the time away stay as they are, the time away now spending
   propellant and refuelling at the intake like a session does.

## 6. Piloted mode (optional)

**What the person does.** From the drone console (in their home, or in the bow's drone bay), they
take control when the drone arrives at its rock:

- **Approach and anchor**: match the rock's spin (a slow tumbler is easy, a fast one hard), pick a
  landing spot, fire the anchors. The thrusters are six-axis: no wings, no air.
- **Cut and load**: the cutter removes voxels from the rock (the voxel asteroid of
  crates/humanity-terrain/src/asteroid.rs, 0.5 m voxels, ore veins by class); each voxel's ore goes
  into the hold by its real mass; a heavier hold flies heavier.
- **Read the gauges**: propellant left, propellant needed to get home with this load, the reserve,
  the hold's kilograms, the cutter's heat, the link's delay and strength.
- **Turn back**: undock and fly home, or hand back to the autopilot for the transfer.

**Why fly instead of automate** (Proposal): a piloted trip can take rocks the autopilot refuses
(fast tumblers, rubble piles, a vein the survey only half saw) and can follow a vein by eye; the
Mining skill rises faster when flown. Automation is never worse at what it will do: piloting adds
reach, never a tax on people who prefer not to fly.

**Server play**: several players' drones can work one rock; the rock's stock is the server's; what
crosses the network is the trip's result (ore taken, the rock's change), never the flight
(section 9).

## 7. Hazards

| Hazard | What it does | Choice it makes | Realistic | Simplified |
|---|---|---|---|---|
| Spin and tumble | docking needs matched rotation; anchors fail on a fast tumbler | pick a calm rock, or a skilled approach | real spin rates from the rock's data | spin capped slow |
| Rubble pile | loose material: anchors slip, cutting throws debris | solid body or rubble | from the rock's class and size | anchors always hold |
| Dust plume | cutting raises dust that blinds sensors for a while | cut slower, or wait | yes | shorter |
| Outgassing | icy bodies warmed by cutting vent gas jets that push the drone | richer water, harder work | yes, on icy and carbonaceous classes | gentle |
| Behind the rock | the rock blocks the signal; control passes to the autopilot until line of sight returns | where to anchor | yes | off |
| Radiation | electronics upsets far from the ship's shielding | shielding upgrade | rare faults | off |
| Wear | the existing service clock, plus the cutter | when to service | as today | half, as today |
| Running dry | the drone drifts: a rescue trip (another drone, or a person's craft) tows it home | the reserve | yes | it limps home on its reserve |

## 8. Where the drone lives on the ship

A drone cannot launch from a home on the drum floor: the drum is a sealed, spinning cylinder. So
(Proposal):

- **The drone's cradle is in the bow's drone bay** (a space program row,
  `data/ship/space_program.toml` `drone_bay`, one bay for every 500 people), with the propellant
  plant and the **ore intake**.
- **The home's drone hangar becomes the drone console**: the person commands and flies their drone
  from home (data/machines/home.ron's `drone_hangar` is re-labelled, its id kept).
- **Ore travels home as freight** (the logistics rule, decided 2026-10-03: goods are "transported
  either by the player or an automated robot or some other method"): intake, spine freight rail,
  the home's parcel locker. Simplified mode puts it straight into home storage, as today.

## 9. Data shapes

| Data | What a row is | Change |
|---|---|---|
| `data/mining/drones.ron` (replacing data/mining/drone.ron) | a drone class: dry mass, propellant capacity, specific impulse, thrust, hold kg and volume, cutter rate, battery and charger, service parts, jump-capable | new shape; the FlyCart row retired, no compatibility code (none before v1.0) |
| `data/asteroids/*.ron` | a rock: id, name, class, mass and size, spin period, frame (`Fleet`, `Orbit { elements }`, `Field`), position and velocity in that frame, ores (finite), hazards, survey level | adds fields; belt.ron's three become `Fleet` rows |
| `data/asteroids/fields.ron` | an interstellar or belt field: density per cubic AU, class mix, size distribution, seed | new: rocks generated near the fleet as it moves |
| `data/mining/hazards.ron` | a hazard kind: what triggers it, its effect, its severity in each mode | new |
| `data/fleet/upgrades.ron` (proposed in game-setting-and-intro.md 3.4) | upgrade rows on drone lines: `drone_tank`, `drone_hold`, `drone_engine`, `drone_cutter`, `drone_anchor`, `drone_sensor`, `drone_autonomy`, `drone_shield`, `drone_jump` | rows only |
| The save (`WorldSave::mining_drone`) | the drone's state: charge, wear, propellant, the trip in flight | adds propellant and the trip's plan; old saves start a fresh drone (as today when the field is missing) |

Every drone, rock, field and hazard is a row: a server can run a thousand rocks and a hundred
drone classes with no code (CLAUDE.md, infinite-of-x).

## 10. Rungs

| Rung | What | Tests (each seen red first) | Runtime check |
|---|---|---|---|
| 0. Data | `drones.ron` with a space drone class (numbers sourced in a dated finding: thruster classes, a real small spacecraft's masses); asteroid rows with frames, distances and velocities; the 5 s phase constants into data | the trip planner's delta-v and cargo match the rocket equation within 0.1%; every asteroid row names a frame | none (data and pure functions) |
| 1. Propellant and distance in the automated loop | trip time from distance and thrust; propellant spent and refilled at the intake; the panel's new lines (delta-v, largest safe load, times, delay); refusal with the reason (Realistic), trimming (Simplified); the time away spends propellant | a trip too far is refused with its reason; Simplified trims instead; offline hauls never exceed the propellant | `just verify-runtime`; the Mining panel snapshot |
| 2. The field | asteroids generated from `fields.ron` around the fleet as it moves; survey levels (sensors reveal composition); the Maps page draws them with their reach (inside the tank, inside a jump) | generated rocks are stable for a seed; reach shading matches the planner | Maps vantage |
| 3. Piloted mode | the one-rock scene: the voxel asteroid connected, six-axis flight, anchors, the cutter removing voxels into the hold by mass, the gauges; hand-over to and from the autopilot | a cut voxel's ore lands in the hold by its mass; flying home with too heavy a hold runs the tank dry (Realistic) | a `verify-drone` rig (launch, take control, cut ten voxels, fly home) |
| 4. Hazards and the light delay | spin, rubble, dust, outgassing, the rock's shadow, the delayed controls with the predicted ghost (Realistic) | a command arrives after the computed light delay; a fast tumbler refuses anchoring | the rig at a far rock |
| 5. Fleet upgrades and the server | drone lines in `data/fleet/upgrades.ron`; several players' drones on one rock; the rock's stock server-held | two drones cannot take the same voxel; an upgrade changes the planner's numbers | `verify-copresence` with two drones |
| 6. The jump | jump-capable drones and riding along in a person's own craft (with the jump's design, game-setting-and-intro.md) | a jumped drone ends matched to its rock | rig |

## 11. Budgets

- **Automated trips**: a phase machine per drone, a few arithmetic operations a tick; under
  0.05 ms a frame for a hundred drones (to be measured, rung 1).
- **The piloted scene** (only while someone pilots): one voxel rock at 0.5 m (the mesher exists;
  remeshing only the changed octree node), one six-axis body, local particles for dust and gas.
  Targets (Proposal): GPU under 2.0 ms, CPU under 0.5 ms at 2560 x 1440, inside the master plan's
  8.3 ms frame. The ship's world beyond the console is not drawn while flying.
- **Memory**: one rock's octree at a time in the piloted scene; rows for the rest.
- **Network**: gameplay state only, at a low rate (CLAUDE.md): a trip's launch and its result, the
  rock's stock changes, each drone's position at most once a second for others' maps. The flight
  itself, the dust and the debris are local and never networked.

## 12. Questions for the operator

Each with a recommendation; until he answers, the recommendation stands.

1. **Do drones jump between the stars?** Recommend yes: small craft jump (decided 2026-11-03), and
   between the stars no rocket reaches a rock moving past at the fleet's speed. A jump-capable drone
   is a later upgrade (rung 6); until then drones mine the rocks the fleet carries and, in a
   system, the rocks in reach of their tank.
2. **Should the light delay be part of Realistic mode?** Recommend yes, by the true distance from
   the person (section 3), with Simplified dropping it; and riding along in your own craft as the
   way to fly a far rock directly.
3. **Where is the drone kept?** Recommend the bow's drone bay, with the home's drone hangar
   becoming the console you fly it from, and ore reaching home as freight (section 8).
4. **What is the propellant?** Recommend water from icy and carbonaceous rocks, so mining feeds the
   next trip; Simplified may make it free.
5. **What does flying earn?** Recommend reach (rocks the autopilot refuses) and faster Mining skill,
   never more ore per trip from the same rock, so automation stays a full way to play.
6. **One drone per player, or more?** Recommend one personal drone to start, more through the
   fleet's upgrades, and fleet-owned drones on a server feeding the ship's stores with every
   player's contribution written in the fleet ledger.
7. **A skill for flying?** The skills list has Mining and Navigation but no piloting. Recommend
   Navigation for the flying and Mining for the cutting, until vehicles and craft need a piloting
   skill of their own.
