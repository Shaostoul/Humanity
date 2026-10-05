# Leaving the ship: down to a planet and back

**Status: a proposal, written 2026-10-05, read against the code at v0.1461.0 (commit 3a48f742c).**
Nothing in it is decided or built. The operator decides. Every recommendation below is the
author's (an agent's), and section 8 lists the questions that need his answer.

**Why now.** Fresh installs start in Normal mode since the operator's decision of 2026-10-04,
and in Normal mode nobody can leave the ship (docs/PRIORITIES.md:198-205, found by the Library
sweep of 2026-10-05). The only ways off are the Dev page's Travel and Land buttons and F9
flight, all Dev-only. So a new player never meets rain, cold, a planet or building on one,
although the game draws a true-scale Earth with Silverdale's real streets and terrain, and many
Library guides teach skills that only make sense outdoors (the Library's own game sections say
so: docs/user/skills/floating_and_boats.md:536-539, navigating_without_instruments.md:491-493).

**How claims are marked** (the convention of [ship-homes-and-logistics.md](ship-homes-and-logistics.md)):
- Repo facts cite `file:line`, read at v0.1461.0.
- World facts cite a link and the source's own date (published or last edited; "no date shown"
  where the page shows none).
- "Computed" means arithmetic on cited figures. The inputs and the formulas are in Appendix B,
  so anyone can redo them.
- "Assumption" means a number of ours that data or a measurement should replace.
- Section 1 is what exists. Sections 4 to 7 are proposals: anything there that does not cite a
  file does not exist yet.

---

## 0. In short

- **The ship is far up.** It hangs in a synchronous orbit 35,870 km above the equator at
  Silverdale's longitude (data/stations/home.ron:33-45; height computed). From Silverdale it
  stands due south, 35 degrees above the horizon (computed).
- **So the honest trip takes about 7 to 8 hours each way, most of it a coast.** From an orbit
  over the equator, a cheap drop meets the air near the equator; aiming for Silverdale's
  latitude directly costs far more and still arrives where the geometry says, not where you
  want (computed, section 2.2). Real crews come home from a low orbit whose path crosses the
  landing site. So the trip is: a transfer to a low orbit tilted to pass over Silverdale (a 5
  hour 16 minute coast, computed), a wait for the pass, then a deorbit burn, a capsule entry and a
  parachute landing (about an hour, assumed).
- **The proposal is a scheduled fleet shuttle**: a capsule, the transfer stage that carries it
  between the ship and low orbit, and a reusable booster for the way up. You book a seat, board
  in person, ride in honest time, and step out onto a field at the edge of Silverdale with the
  weather, the cold and the survival rules on. The same service takes you back.
- **The clock:** what happens above the air runs on the game clock, like the ship's own orbit;
  what happens in the air (entry, parachutes, touchdown, about 25 minutes) runs in real seconds
  because you watch it. A solo player may wait the part above the air out the way sleep passes a
  night. In a shared world nobody can, so you ride along or sign off, and your seat holds.
- **The cost:** in early development a seat is free and the trip is written into your fleet
  ledger as used, at its real weight once propellant exists as goods (the operator's
  unlimited-fleet decision of 2026-10-04).
- **The modes:** Simplified by default (always a seat, nothing fails, you land on the field);
  Realistic (seats limited by the fleet's stocks, weather holds, a landing ellipse, real g on the
  couch); Dev keeps every teleport and gains "arrive now".
- **Multiplayer:** the relay holds bookings and where each traveller is (the ship, or a named
  place on the ground), never trajectories. Every game computes the flight from the departure
  record, so a trip costs a few hundred bytes and no traffic per frame. Weather becomes a
  function of place and day, so two players on the field see the same rain without sending it.
- **The first increment, about a day:** book a seat from the Maps page, board at a marked spot in
  the Commons (until the hangar opens), wait out the trip on a trip card, arrive standing on the
  Silverdale field with the survival rules on, and come back. Your own world only; a shared world
  gets one honest sentence until increment 3.
- **Twelve open questions** are in section 8, each with a recommendation. The two biggest: is the
  game's present the time while the fleet is still at Earth, and is 7 to 8 hours each way
  acceptable on the real-time shared server.

---

## 1. What exists today, and what does not

### 1.1 What a trip can build on

| Piece | Where | What it gives a trip |
|---|---|---|
| The ship's orbit | data/stations/home.ron:33-45 (Synchronous, over longitude -122.3, nadir-pointing); src/station/orbit.rs:175-209 (closed-form Kepler), driven by the game clock through `sim_seconds` (:67) | where every trip starts; a propagator that already handles ellipses (eccentricity clamped to 0.95 at :183; a transfer ellipse from this orbit is about 0.73, computed) |
| A descent with no loading screen | src/engine/frame_lock.rs:10 (surface mode below 10 km), :32 (co-rotation below 100 km), :40 (eased to the inertial frame up to 1,000 km); the chunked-LOD Earth; leaving and boarding the ship by its bounds (src/lib.rs:3379-3445) | a camera that already goes from the ship to the grass, flown today by Dev travel and the rigs |
| Silverdale | data/locales/silverdale_wa/locale.json:16-26 (centre and bounds); data/maps/regions/silverdale.bin and silverdale.dem.bin (real roads, buildings and terrain, built when the camera comes within 40 km, src/engine/region_meshes.rs:28) | the place to land |
| Life on the ground | src/engine/survival_env.rs:217-256 (outside: the weather's air at the player, wind, rain or snow, shelters), :145-153 (breathable only where the body's air allows it) | rain, cold, wind and shelter, all live once fly mode is off |
| Building on a planet | src/engine/planet_build.rs; src/systems/construction/site.rs (a build site pinned to the ground, pieces within 1 km join it, :60); kept in the save (the site round-trip test in src/save_load.rs) | building down there already works; on a planet you build from what you carry (src/systems/crafting/home_store.rs:13-16) |
| The home keeps running | src/lib.rs:6629-6638 (every system ticks each frame, wherever the player stands); offline catch-up on the next launch (src/config.rs:649-652) | nothing at home pauses while you are away |
| Death on a planet | [death-and-your-pack.md](death-and-your-pack.md), lines 38-53 | a Realistic death leaves the pack on the planet's ground; you wake in the respawner aboard (line 22) |
| The way home on screen | BUG-148, fixed in v0.1459.0 (docs/BUGS.md:3466): the Home Station marker shows from the ground | you can always see where the ship is |
| Dev travel | src/gui/pages/dev.rs:233-393; src/lib.rs:5819-6091 | the placement maths for "stand on the ground at this latitude and longitude" (lib.rs:5988-6091) |
| Vehicles | src/systems/vehicles/mod.rs (deploy :243, summon :327, a drive on real seconds :368); data/vehicles/kits.ron (pickup, rover, 1975 Nova, 6 to 12 m/s) | seats (`VehicleSeat`), entering and leaving, a vehicle that follows a route |
| Spacecraft as goods | data/items.csv:365 (Spacecraft Pod, 2,000 kg), :404 (Shuttle, 8,000 kg); data/recipes.csv:557 and :562 (real bills of materials since v0.1455.0) | names and bills a transport can use |
| G on the body | data/ship/flight.ron:72-79 (crew: safe to 1.5 g, harmed from 4 g at 2 health a second); the felt g reaches the survival context (src/engine/survival_env.rs:165-169) | a place for an entry's g to land on the body |
| The clock | [decision-briefs.md](decision-briefs.md), lines 200-211 (motion you watch stays on real seconds) and 222-231 (a shared world runs at 1x); sleep holds the clock at 7,200x (src/systems/sleep.rs:36) and is refused in a shared world (:164-170) | the rule for who may wait a trip out |
| Departures, as a design | [ship-homes-and-logistics.md](ship-homes-and-logistics.md), section 3 (a departure is a record; boarding in person) and section 5.10 (the next departure computed from a start time plus a headway, never stored; arrivals on the server's wall clock) | the shape the shuttle's timetable takes |
| The fleet ledger | ship-homes-and-logistics.md, increment 3, "The fleet ledger, as built" (`data/ship/fleet_ledger.ron`) | where a free seat's real cost can show |
| The descent ladder | tests/visual/vantages.json, `ladder`: one boot, one Silverdale column, 16 rungs from 12,000 km down to 0.3 km, with checks for pops at known boundaries; 43 more vantages at Silverdale's latitude, from the ground to 12,000 km | the rig proof for a descent mostly exists already |

### 1.2 What does not exist

- **No way down outside Dev.** The Dev page is gated to the Dev play mode (src/gui/pages/dev.rs:74-91; src/config.rs:186-188).
- **No hangar you can reach.** `hangar-1` is a district (data/blueprints/ship_structure.ron:1043-1048),
  drawn with empty ship cradles (data/blueprints/zone_filler.ron:50-57), with no floor and no
  corridor. The relay's rooms are zones, so it is no room either (ship-homes increment 3, "as
  built": the districts are not rooms).
- **No vehicle that flies.** kits.ron has no spacecraft row; data/items.csv has no engine,
  thruster, parachute, heat shield or propellant item; `PropulsionDef` and `ShipSystems` are used
  by nothing (src/systems/vehicles/propulsion.rs:9, ships.rs:9); of the vehicle, docking and
  transportation systems only `VehicleSystem` is registered (src/lib.rs:1321).
- **No route, timetable, seat or trip.**
- **No planet on the relay.** Its world is the ship file in ship metres (src/relay/handlers/game_state.rs:751-785,
  ship_world.rs). The game joins the shared world only while aboard (src/engine/home_plot.rs:236-247),
  and Dev travel steps out of it first (src/lib.rs:5882-5885, 5994-5997).
- **Weather differs per player.** Each game rolls its weather from its own computer's random
  source (src/systems/weather.rs:311, rolls at :322 and :396-418), on top of a climate that is a
  pure function of place and season (src/systems/env_layer1.rs). Two players in one place would
  see different rain.
- **Dev's Land does not land.** Its Earth target is a coordinate written in code, the Oahu coast
  (src/lib.rs:6012-6016), 1.5 km up (:6017), with fly mode on (:6066-6068), and fly mode
  suspends the survival rules (src/engine/survival_env.rs:178-181).
- **Quest destinations are ship points only** (data/entities/destinations.ron).

---

## 2. The real-world reference

### 2.1 Where the ship is (computed)

- The home's orbit is synchronous with the solar day (src/station/orbit.rs:59; src/lib.rs:3362),
  so it sits 42,241 km from Earth's centre, 35,870 km up, moving at 3.07 km/s. A real
  geostationary orbit keeps the sidereal day and sits at 42,164 km; the game's 77 km difference is
  deliberate (the test comment at src/station/orbit.rs:312-319).
- From the ship, Earth is 17.4 degrees across.
- From Silverdale (47.64 N) the ship stands due south, 35.3 degrees up, 38,240 km away. Even the
  planned 2 km drum ([habitat-generation.md](habitat-generation.md), lines 18-29) is 11 arc-seconds
  there: a point of light, never a shape.

### 2.2 The way down from this orbit (computed)

- **The cheapest drop** is a 1.49 km/s burn against the direction of travel, then a coast of 5
  hours 15 minutes, meeting the air (120 km up) at 10.3 km/s. That speed is close to a return
  from the Moon (Orion, section 2.3). But an orbit over the equator, dropped this way, meets the
  air near the equator.
- **Aiming north directly is dear and does not help.** With a survivable entry angle (6.5
  degrees below the horizon), reaching the air at 25 N costs 2.8 km/s and at 47.6 N 4.3 km/s, and
  where along that latitude the capsule arrives is fixed by the geometry, not chosen. A search of
  every coast time from 20 minutes to 15 hours (single arcs, both ways round) found none that
  meets the air over Silverdale, or up to 30 degrees south of it on its meridian, at an angle
  between 5 and 8 degrees below the horizon. The cheapest arc that does reach the air above
  Silverdale (2.5 km/s, 4.9 hours) comes in 47 degrees steep: about 200 g at the peak by Allen
  and Eggers' simple formula, where 6.5 degrees gives about 31 g even without lift.
- **So real returns come from a low orbit.** A crewed capsule leaves a low orbit with a small burn
  timed so its path crosses the landing site, enters at about 7.8 km/s, and is on the ground within
  hours of leaving its station (Soyuz: no more than three and a half, section 2.3).
- **For this ship that means:** a departure burn that also tilts the path toward Silverdale's
  latitude (2.3 km/s to a low orbit tilted 48 degrees), the 5 hour 16 minute coast, about 2.5 km/s
  to settle into the low orbit (less with aerobraking, a Realistic option for later), a wait for
  the pass over Silverdale, a deorbit burn of about 0.1 km/s, and about an hour to touchdown.
  About 7 to 8 hours in all (the coast computed, the other phases assumed: section 4.3).
- **Buying speed is dear.** Cutting the coast from 5.3 to 3.7 hours costs about half again as
  much (3.9 to 5.9 km/s for the transfer, plane change left out), and no transfer of sane cost
  goes under about 3.5 hours.

### 2.3 How people and cargo come down today

| Kind | Example | How it lands | What the body feels | Source |
|---|---|---|---|---|
| Capsule from low orbit | Soyuz | entry at 122 km; a drogue chute slows it from 230 to 80 m/s, the main to 7.2 m/s; landing rockets fire 1 m above the ground and touch down at 1.5 m/s, on land | 4 to 5 g; reentry and landing take no more than three and a half hours | [ESA, "Way back to Earth"](https://www.esa.int/Science_Exploration/Human_and_Robotic_Exploration/PromISSe/Way_back_to_Earth) (2012 mission page, no date shown) |
| Capsule at lunar-return speed | Orion, Artemis I | nearly 25,000 mph at entry, about 20 minutes from the first air to splashdown at about 20 mph | an uncrewed test flight | [NASA, 2022-12-11](https://www.nasa.gov/centers-and-facilities/hq/splashdown-nasas-orion-returns-to-earth-after-historic-moon-mission/) |
| Its heat shield | Orion's Avcoat | during its skip entry, charred material broke away where gases inside could not vent; Artemis II keeps the shield, with changes to how the entry is flown | | [NASA, 2024-12-05](https://www.nasa.gov/missions/artemis/nasa-identifies-cause-of-artemis-i-orion-heat-shield-char-loss/) |
| The g of a direct return | Apollo | lunar returns averaged 6.63 g at peak, Earth-orbit returns (Apollo 7 and 9) 3.34 g | | Stroud and Klaus, [NASA NTRS 20080026216](https://ntrs.nasa.gov/citations/20080026216) (2006) |
| Cargo capsule | Dragon, CRS-32 | brought down 4,122 lb; undocked at 12:05 p.m. on 23 May, splashed down at 1:44 a.m. EDT on 25 May 2025 (about 38 hours from undocking to splashdown) | | [NASA, 2025-05-25](https://www.nasa.gov/blogs/spacestation/2025/05/25/spacex-dragon-splashes-down-off-the-coast-of-california/) |
| Small cargo capsule | Varda W-1 | landed at the Utah Test and Training Range on 2024-02-21, the first reentry under the FAA's Part 450 licence | | [Varda, W-1](https://www.varda.com/mission/w-1) (no date shown) |
| Spaceplane | Space Shuttle | a glider to a runway | under 1.5 g, but for up to 17 minutes | Stroud and Klaus (2006), above |
| Spaceplane, next | Dream Chaser | first flight slipped to late 2026, as a free-flyer instead of a station visit | | [Spaceflight Now, 2025-09-26](https://spaceflightnow.com/2025/09/26/sierra-spaces-dream-chaser-debut-mission-delayed-again-no-longer-docking-to-station/) |
| Propulsive | Starship, flight 11 | flew deliberate gaps in its heat-shield tiles, flipped and fired its engines for a controlled splashdown (launched 2025-10-13) | | [Wikipedia](https://en.wikipedia.org/wiki/Starship_flight_test_11) (edited 2026-06-03) |

What the same sources say about bodies and landings:
- A fit person stands about 12 g for a moment; a crew weakened by long weightlessness only 3.5 to
  5 g, and NASA's crew return vehicle design capped sustained entry loads at 4 g (Stroud and Klaus,
  2006). People who live aboard at 1 g, as the drum is designed to give, are not weakened that way.
- Water forgives an imprecise landing but needs a recovery at sea; a land landing needs impact
  attenuation (parafoils, retrorockets, shock absorbers) and, with good accuracy, can keep away from
  towns (Stroud and Klaus, 2006). Soyuz fires its small rockets just before touching down on land.
- Capsules are blunt on purpose: a blunt body pushes the shock wave away from itself, and most of
  the heat goes with it (Allen and Eggers, [NACA Report 1381](https://ntrs.nasa.gov/citations/19930091020), 1958). The heating at the
  nose follows Sutton and Graves ([NASA TR R-376](https://ntrs.nasa.gov/citations/19720003329), 1971-11-01).

### 2.4 The way up (computed, with cited inputs)

- **To low orbit:** 7.8 km/s plus typically 1.5 to 2 km/s lost to drag and gravity
  ([Wikipedia, Delta-v budget](https://en.wikipedia.org/wiki/Delta-v_budget), edited 2026-07-02).
- **From there to the ship:** 2.46 km/s to start the climb and 2.32 km/s at the top, where the
  path also turns into the ship's plane: 4.78 km/s from a low orbit tilted 47.6 degrees,
  Silverdale's latitude, against 3.94 km/s from one over the equator (computed). About 0.8 km/s is
  the price of launching so far north.
- **About 14 to 15 km/s in all.** Nobody flies that in one vehicle today: a reusable booster lifts
  to low orbit and a transfer stage carries on. Such a stage is being built for satellites:
  Impulse Space's Helios, low orbit to geostationary in under a day ([TechCrunch, 2025-09-16](https://techcrunch.com/2025/09/16/same-day-delivery-comes-to-space-as-impulse-promises-satellite-transport-in-hours-not-months)).
  Refilling a big ship's tanks in orbit had not been attempted as of July 2026
  ([SpaceDaily, 2026-08-27](https://spacedaily.com/t-starship-13-test-flights-orbital-refueling-mars-hurdle)).
- **What a transfer weighs:** with a methane and oxygen stage at an assumed 370 s specific
  impulse, each 4.78 km/s transfer burns about 2.7 tonnes of propellant for every tonne it
  delivers (computed). That is the weight a seat should carry in the fleet ledger.

### 2.5 What a game made for 2030 models

- **Space to ground with no cut.** Starfield's director judged the transition not worth the
  engineering and kept space and surface apart ([TheGamer, 2023-01-18](https://www.thegamer.com/starfield-wont-feature-seamless-space-planet-landing/)).
  HumanityOS already has the hard part: the frame lock and the chunked Earth carry a camera from
  orbit to the grass (section 1.1).
- **Heat and heat shields as real limits:** Kerbal Space Program 1.0 made reentry heating real and
  heat shields necessary ([GamingOnLinux, 2015-04-27](https://www.gamingonlinux.com/2015/04/kerbal-space-program-reaches-version-10-has-a-bunch-of-new-stuff/page=1/)).
- **An entry glow shaped by the vehicle:** Star Citizen 3.10 (released 2020-08-05,
  [starcitizen.tools](https://starcitizen.tools/Update:Star_Citizen_Alpha_3.10.0)) drew entry effects
  from a signed distance field that follows each ship's shape, size and orientation
  ([RSI patch notes](https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-3-10-0-live-5789362-patch-notes)).
- **Flying the real procedure:** Reentry, a capsule simulator of the Mercury, Gemini and Apollo
  craft ([Nextgov, 2025-11-14](https://www.nextgov.com/emerging-tech/2025/11/learning-fly-how-reentry-simulates-nasas-most-daring-missions/409534)).

So the 2030 version here is: a route from real orbital mechanics, a vehicle with a real shape (a
capsule is a solid of revolution, so a lathe of its outer mould line is its true geometry, not a
stand-in), an entry glow from the vehicle's own distance field and the computed heating, the g on
the body, honest time, and waiting allowed only where the rules allow it. Left out on purpose: a
loading screen, a teleport, an invented drive.

---

## 3. What is not proposed

- **A teleport down in Normal play.** It is what the Dev tools are for.
- **A cut to a loading screen.** The engine already does the seamless version.
- **A space elevator.** The lore dates the first to the 2060s (docs/game/humanity_one.md:19), and
  the ship hangs exactly where one would end, over the equator at -122.3. But no material can
  build one today. If it is ever wanted, it would be a labelled fiction, the way the FTL proposal
  of 2026-10-05 treats FTL ([gravity-and-movement.md](gravity-and-movement.md), lines 388-397).
- **An invented faster drive for the shuttle.** Section 2.2 shows the honest price of speed.
- **Moving the ship lower.** See question 2.

---

## 4. The proposal: the trip as the player lives it

### 4.1 The Silverdale shuttle

- **Run by the fleet**, like the mess hall. The operator: "The player just happens to be one of
  the crew members" (docs/design/the-mothership.md:15-16), and the ship carries on without them.
- **Three vehicles, each a data row** in a new spacecraft file (infinite-of-X: a new vehicle is a
  row): the capsule (seats as acceleration couches, a heat shield, parachutes, landing rockets),
  the transfer stage between the ship and low orbit (no heat shield, never enters the air), and the
  booster for the way up (it flies back to the field's pad).
- **The player sees one thing:** "the Silverdale shuttle", with the parts named as they act ("the
  transfer stage lets go").
- **Every phase explains itself** in one plain sentence on the trip card, on the thing rather than
  in a manual (the operator's inline-first preference): why the coast is five hours, why capsules
  are blunt, why it lands where its orbit passes.

### 4.2 Booking and boarding

- **The timetable is data:** a first departure and how often after it, the next one computed and
  never stored (ship-homes section 5.10). Because the ship is geostationary, the geometry over
  Silverdale repeats every day, so each departure time has a fixed trip length, worked out once by
  a planning script and checked by a test.
- **Book from anywhere:** the Maps page at first, the departure board in the hangar from
  increment 4 (ship-homes section 3 already lets you sign up from anywhere).
- **Board in person:** stand in the boarding circle at departure time (ship-homes section 3). Until
  the hangar opens, the circle is a marked spot at the Commons' arrival point (its `spawn`,
  data/blueprints/ship_structure.ron:950), as a data row, so moving it later is a data edit.
- **Take what you carry:** the backpack, 50 kg and 65 L (src/systems/inventory/mod.rs:219;
  src/systems/encumbrance.rs). Bulk goods ride as cargo later (ship-homes section 5).

### 4.3 The trip down, phase by phase

| Phase | What you see and do | Length | Clock | Basis |
|---|---|---|---|---|
| Board | take a couch; the hatch closes | before departure | | |
| Undock | the transfer stage backs out of the bay; the hull slides away | 15 min | game | assumption |
| Departure burn | 2.3 km/s, a steady push of about half a g | 8 min | game | speed change computed; thrust assumed |
| The coast | Earth grows from 17 degrees across to filling the window; the ship shrinks to a star | 5 h 16 min | game | computed |
| Into low orbit | about 2.5 km/s | 8 min | game | speed change computed; thrust assumed |
| Waiting for the pass | a dawn or a dusk every 45 minutes | 0 to 90 min, set by the timetable | game | assumption |
| Deorbit and fall | a 0.1 km/s nudge, then about half an hour of falling | 35 min | game | assumption, the order of a Soyuz return |
| Entry | the glow, the radio blackout, 4 to 5 g on the couch | 10 min | real | ESA; NASA |
| Parachutes and touchdown | drogue from 230 to 80 m/s, main to 7.2 m/s, landing rockets at 1 m, touchdown at 1.5 m/s | 15 min | real | ESA |
| **Total** | | **about 7 to 8 hours**, about 25 minutes of it in the air | | |

At the Simplified clock speed a solo player can pick (72x), the coast is 4.4 minutes.

### 4.4 The trip up

| Phase | Length | Clock | Basis |
|---|---|---|---|
| Board at the pad beside the field | before departure | | |
| Launch to low orbit, staging, the booster flying back to the pad (up to about 3 g) | 10 min | real | assumption |
| Meeting the transfer stage | 1 to 3 h, set by the timetable | game | assumption |
| Transfer burn | 8 min | game | 2.46 km/s computed |
| The coast up | 5 h 16 min | game | computed |
| Arrival burn, turning into the ship's plane | 8 min | game | 2.32 km/s computed |
| Approach and docking in the hangar | 30 min | game | assumption |
| **Total** | **about 7 to 9 hours** | | |

### 4.5 The clock rule

- **Above the air, the game clock**, exactly like the ship's own orbit (src/station/orbit.rs:67).
  Undocking, the burns, the coasts and the waits all count on it.
- **In the air, real seconds**, because you watch it, the rule for a summoned vehicle's drive
  ([decision-briefs.md](decision-briefs.md), lines 200-211).
- **Who may wait it out:** in your own world, "Wait until arrival" holds the clock fast through
  the part above the air, the mechanism sleep already uses (src/systems/sleep.rs:36). Those 6 to 7
  hours then pass in about 3 real seconds, and they cost what those hours cost: crops grow, the
  body gets hungry and thirsty, the day moves on. In a shared world nobody can hold the host's clock, the
  same reason sleep is refused there (src/systems/sleep.rs:164-170): you ride along, or sign off and
  arrive anyway (section 6).
- **A trip always costs its true time on the game clock**; how much of it you sit through is your
  choice where the rules allow, and not where they do not.

### 4.6 What it costs

- **Simplified:** the seat is free, and the trip is written into your fleet ledger as used (a new
  kind in `data/ship/fleet_ledger.ron`), so the balance in the red or the black shows what a trip
  weighs. Its value is the propellant and heat-shield share at their prices once those exist as
  goods (section 2.4: about 2.7 tonnes of propellant per tonne moved, each transfer); until then
  the line is written with a named placeholder of 0, the pattern of
  `npc_homestead_fleet_meals_per_day` (ship-homes question 12).
- **Realistic:** seats come out of the fleet's stocks when the server runs the stocked supply
  mode, and propellant and ablator are drawn from its stores.
- **Your own vehicle:** later (increment 7).

### 4.7 Where you land

- **Silverdale first**, the canonical world (data/locales/silverdale_wa/locale.json:2: a place is
  selected by id, never named in code). So the field is a data row, never a coordinate in src/.
- **A field, not a pad in a street:** open public ground inside the Silverdale region (bounds
  47.58 to 47.70 N, 122.62 to 122.75 W; locale.json:21-26), chosen by the operator. A test proves it
  touches no building or road in data/maps/regions/silverdale.bin and stands above the water.
- **The game names the liberty it takes:** a real landing zone is kilometres across and far from
  homes. This one sits at the edge of town so you can walk in. Saying so in the game follows the
  honest label the FTL proposal gives a deliberate fiction ([gravity-and-movement.md](gravity-and-movement.md),
  lines 388-397).
- **Where exactly:** Simplified always on the field; Realistic anywhere in the route's landing
  ellipse (data), so you may have a walk.
- **The way up** leaves from a pad beside the field (the booster lands back on it).
- **More places later,** as rows: the Seattle Center region already ships
  (data/maps/regions/seattle-center.bin).

### 4.8 On the ground, and coming back

- **You step out with fly mode off:** the real weather reaches the body (rain, cold, wind, at the
  Body heat mode you have set), shelters count, you build from what you carry, and the Home Station
  marker shows the ship to the south.
- **The field's shelter** (from increment 2): a hut of the existing wall and roof pieces at a
  data-defined build site, so the first rain has somewhere to go.
- **Coming back:** stand in the pad's boarding circle at an up departure.

### 4.9 Your home and your body while you are away

- **Your home keeps running.** Every system ticks each frame wherever you stand (src/lib.rs:6629-6638):
  crops grow, machines run, the reactor meters. On the ground your home's storage is out of reach
  (src/systems/crafting/home_store.rs:13-16). In a shared world your home is still your own game's;
  the relay keeps your plot as it does whenever you are away (ship-homes question 19). Sign off
  mid-trip and the offline catch-up runs the home forward at your next launch (src/config.rs:649-652).
- **Your body keeps its needs.** Hunger, thirst and tiredness run on the clock through the trip; you
  eat and drink what you carry; the entry's g lands on you through the couch.
- **Dying down there** today wakes you in the respawner aboard ([death-and-your-pack.md](death-and-your-pack.md), line 22),
  7 hours from a Realistic pack that lasts 60 minutes of play (line 62). Question 8 proposes a
  respawner in the field's shelter.

---

## 5. Normal, Simplified, Realistic and Dev

Two different switches meet here:
- **The play mode** (src/config.rs:124-139): Normal, Creative or Dev.
- **The trip's own two modes**, the dual-mode house rule for deep systems (CLAUDE.md, 2026-09-24): a
  new row "Trips" in Settings > Gameplay, Simplified by default, beside Body heat, Carrying weight
  and Death (src/config.rs:789-815).

| | Normal or Creative, Simplified (the default) | Normal or Creative, Realistic | Dev |
|---|---|---|---|
| Getting a seat | always a seat | seats per departure from the fleet's stocks; a departure can fill | any seat |
| Cost | free; a ledger line | propellant and ablator from the fleet's stores | none; a Dev trip is recorded as worth 0 (the pattern of `item_creative`) |
| Time | the true time; in your own world you may wait out the part above the air | the same | also "Arrive now"; in a shared world it steps out first, as Travel does today (src/lib.rs:5882-5885) |
| Weather at the field | never stops a landing | holds, or diverts to an alternate field, past the wind and visibility limits (data) | ignored |
| Where you come down | on the field | anywhere in the landing ellipse | anywhere |
| G on the body | felt, never harmful | the couch's real limits (a new `couch` row in data/ship/flight.ron) | felt, never harmful |
| What can go wrong | nothing | a hold, a diversion, a hard landing, each told plainly | nothing |
| Dying on the ground | wake in the field's shelter, nothing lost (question 8) | wake in the field's shelter; the pack lies where you fell | as Normal |
| Teleports | none | none | all kept |

Creative gives free building and free materials. A trip is not a material, so Creative rides the
shuttle like Normal.

**What Dev keeps, all of it:** Travel to any body, Land on surface, F9 fly and hover, the speed
multiplier up to a billion times, the F6 location bookmarks and Return home (src/gui/pages/dev.rs:233-393).
New for Dev: "Arrive now" for a trip, the trip verbs for the rigs, and Land's targets read from the
same landing-site rows (question 11).

---

## 6. Multiplayer

### 6.1 What the relay knows today

- Its world is the ship: the ship file's zones are its rooms, positions are ship metres, a player's
  entity carries health, stamina, an empty inventory, experience, quests and their plot
  (src/relay/handlers/game_state.rs:751-785; ship_world.rs).
- It checks every move against a speed allowance and declared jumps (src/ship/moves.rs;
  src/relay/handlers/move_check.rs), and tells only the players who have a mover in view
  (src/relay/handlers/game_interest.rs).
- It knows nothing of planets. The game stays joined only while aboard (src/engine/home_plot.rs:236-247),
  and Dev travel steps out of the shared world before it moves (src/lib.rs:5882-5885).

### 6.2 What it must learn

| Thing | Held as | For how long | Why |
|---|---|---|---|
| Routes and timetables | data, read by relay and game alike | | the next departure is computed, never stored (ship-homes section 5.10) |
| A booking | a row in a new `trips` table: player key, route, departure time, direction, seat, state. A new table in its own batch, so the BUG-046 rule is met trivially | until the trip settles, then deleted | a seat must hold when you sign off |
| Where a player is | a `place` on their entity: `ship`, or `site:<id>` for a place on the ground | while in the world | moves are checked, and delivered, in the right frame |
| Boarding | a declared move, `moved: {by: "trip", route, departure}`, accepted only for the relay's own booking and only from the route's boarding circle, the way a teleporter is checked by its link (ship-homes increment 4) | | nobody claims to be on Earth by saying so |
| Arrival | the relay stands a traveller at the route's arrival point once the departure time plus the trip's length has passed on its own wall clock (ship-homes section 5.10) | | a traveller who signed off still arrives |
| Seats taken | a count per departure | | the departure board; never names |

New messages, sketched: `game_trip_book`, `game_trip_cancel`, `game_trip_booked` (with the
departure and arrival times), `game_departures` (counts only).

### 6.3 What syncs and what stays local

| What | Synced | Network | Per frame (GPU, budget to be measured) |
|---|---|---|---|
| Booking, cancelling | to the relay | a few hundred bytes a trip | |
| Boarding, leaving, arriving | to players with the bay or the field in view | one message each | |
| The shuttle's flight | no: every game computes it from the departure record and the clock (Kepler above the air, a stored entry profile in it) | none | |
| Fellow passengers | who sits where is in the departure record, shared only with that departure's passengers | none | |
| The capsule drawn | local | none | 0.1 ms (assumption) |
| Entry glow | local | none | 0.3 ms (assumption) |
| Parachutes | local | none | 0.1 ms (assumption) |
| Landing-rocket smoke | local, about 2 seconds | none | 0.2 ms while firing (assumption) |
| Terrain, clouds and the town on the way down | local | none | what the descent ladder already measures at each rung |
| Positions on the ground | at the rate used aboard, only to players in the same place and in view | as aboard | |
| Weather at a place | no: seeded from the place and the game day | none | |
| Pieces built on the ground | not until shared building's `site:<id>` frame (ship-homes section 8) | | |

The budgets are measured on the operator's machine with the rigs' frame costs at the descent
ladder's rungs before increment 2 ships (CLAUDE.md, "Compute and bandwidth are a hard budget").

**Precision:** a place on the ground is a build-site frame: an f64 origin in the body's frame,
positions as small f32 offsets from it (the `PlanetSite` rule, src/systems/construction/site.rs).
Planet-scale coordinates never cross the wire as f32.

### 6.4 Privacy

- The relay keeps where a player is going only until they arrive; the row is then deleted.
- The departure board shows counts, never names. Your own booking is yours.
- Others see a person board only if they stand where they could see it, as they would see them
  walk.
- The trip's ledger line follows the ledger's rules: your own lines only; admin totals held back
  below three players (ship-homes increment 3, "The fleet ledger, as built").

---

## 7. Increments, in build order

Each one is proven on the state the player reaches, and its failing case is seen before its
passing case is trusted (the house rule of ship-homes section 7). Sizes are estimates.

### Increment 0, an hour: say so (optional)

- One sentence where a Normal player looks for a way down, the Maps page's planet view
  (src/gui/pages/cosmos.rs): there is no way down in Normal play yet, it is being built, and the
  Dev play mode's Travel page lands you meanwhile. Removed when increment 1 ships.
- Proof: the page's snapshot shows the sentence in Normal and not in Dev.

### Increment 1, about a day: a seat to Silverdale and back, in your own world

What changes (all new):
- `data/transit/surface_routes.ron`: one route, `silverdale`. Its boarding circle (zone `commons`,
  its spawn point, 3 m) until the hangar opens; its field (locale `silverdale_wa`, latitude,
  longitude, a 30 m circle), provisional until the operator picks (question 6); departures (a first
  time and every 6 game hours, an assumption); the down and up phase lists with each phase's
  length, clock and one-line reason, generated by a planning script from the arithmetic of
  Appendix B.
- `scripts/plan-surface-route.js`: regenerates those numbers (the scratch versions used for this
  document are the start of it).
- `src/systems/trips.rs`, pure and in every feature set (as `death_pack.rs` is): the route loaded
  from disk first with a built-in copy, the next departure, a trip's state (booked, riding a phase,
  arrived), phases advancing on their clock, the boarding check, and the waiting rule.
- `src/engine/trip.rs`: "Book a seat" on the Maps page's Silverdale card; the trip card while riding
  (the phase, the time left, the phase's one line, "Wait until arrival" in your own world); the
  arrival through the placement Dev's Land already uses (src/lib.rs:5988-6091), moved into a shared
  function, with fly mode off and the eye at standing height on the drawn ground; the way up from the
  pad's circle back to the boarding circle aboard, through the path Dev's Return home already uses
  (src/lib.rs:5831-5878, which docks to the ship's current place in its orbit and releases the frame
  lock).
- In a shared world the button says, in one sentence, that the shuttle runs in your own world until
  the server can hold your seat (increment 3).
- `debug/trip_request.json`, a rig verb in the pattern of `camera_request.json`
  (src/engine/ipc.rs): book, wait, status.
- While the card is up it covers the view, and in this increment the whole trip can be waited out,
  the air part included, because there is nothing to watch yet. The card is not a stand-in for the
  ride: it stays, as the "Wait until arrival" screen, in every later increment, and increment 2 adds
  the ride beside it.
- Not in it: a vehicle drawn, the ride's view, the hangar, the ledger line, a second place.

Proof (each test seen red first):
- `the_field_lies_in_its_locale`: the circle is inside locale.json's bounds.
- `the_field_is_open_ground`: the circle touches no building outline or road in silverdale.bin and
  stands above the water in silverdale.dem.bin. Red: the field moved onto a building.
- `the_coast_takes_what_kepler_says`: the coast phase's length equals half the period of the
  transfer ellipse from the ship's radius (from MU_EARTH and the 86,400 s day, src/station/orbit.rs:55
  and :59) to the low orbit, within 1 percent, computed in the test by its own formula rather than
  the module's.
- `a_trip_moves_only_with_its_clock`: game-clock phases advance with the game clock, air phases with
  real seconds.
- `waiting_costs_the_trips_hours`: after waiting, the game clock has moved on by the trip's length,
  so the crops and the body's needs have too.
- `waiting_is_refused_in_a_shared_world`, as sleep's is.
- `you_arrive_standing_outside_on_earth`: on arrival the frame lock holds Earth, fly mode is off,
  the survival context is Outside and breathable (survival_env.rs `whereabouts`, `outside_context`).
- `normal_mode_still_cannot_teleport`: the play-mode truth table is unchanged (src/config.rs:176-190).
- The rig, on a Normal-mode sandbox (the rigs pin Dev today, so this leg sets Normal): book, wait,
  then at the field the frame lock's anchor is inside the circle, the HUD's movement line reads WALK,
  not FLY, and a screenshot shows the ground with the Home Station marker to the south; then the way
  up stands the player at the boarding circle aboard. Red: the same leg on v0.1461.0 stops at "book".

### Increment 2, two to three days: the ride you can watch

What changes:
- **The camera rides the real trajectory.** Above the air from closed-form conics
  (src/station/orbit.rs `propagate`), placed in the Earth-centred f64 frame the way Dev travel moves
  the ship frame, with the frame lock taking over through its existing bands. In the air from an entry
  profile stored with the route (height, downrange, speed, flight-path angle, g and heating against
  time), integrated at development time from a point mass with drag and lift (Allen and Eggers'
  model, 1958) and the Sutton and Graves heating (1971).
- **The capsule:** lathed from an outer mould line in data, with couches and a window. Parachutes
  as data, opening at the ESA figures; landing rockets firing at 1 m.
- **The entry glow:** a shell drawn at an analytic distance from the hull (a capsule's distance field
  is cheap and exact), as bright as the computed heating, only between the heights the profile says
  it glows; radio silent through the blackout.
- **The body:** the profile's g reaches the survival context while seated; a `couch` tolerance row
  in data/ship/flight.ron (in Simplified it never harms).
- **The field's shelter** (section 4.8).
- **Waiting** now skips only the part above the air. The air part plays in real seconds, about 25
  minutes, and can be skipped after the first time you have seen it (the clock still moves by its
  length).

Proof:
- Tests: the stored profile's peak g and peak heating match a fresh run of the integrator; the
  parachutes open at the ESA heights and speeds; the couch row exists and Simplified never harms.
- The rig rides a real trip and captures at the descent ladder's rung heights along the trip's own
  path (tests/visual/vantages.json `ladder`, 12,000 km down to 0.3 km): every height reached in
  order at the profile's time, the ladder's own boundary checks still passing (they are about height,
  not place), the capsule's silhouette in the window, the glow present only in its band, and the felt
  g the game publishes (`felt_gravity`, src/engine/survival_env.rs:169) within 5 percent of the
  profile. Frame costs at each height against the existing approach vantages' baselines plus the
  budgets in section 6.3. Red: the glow forced on above its band fails its check.

### Increment 3, two to three days: the shared world holds your seat

What changes:
- The relay half of section 6.2: the `trips` table, the messages, the declared boarding move, the
  `place` on the entity, the arrival on the relay's wall clock (also for a traveller who has signed
  off), and the trip's ledger line.
- The game half: a trip in a shared world runs on the server's clock; "Wait until arrival" is
  refused there and "Sign off: your seat holds" is offered instead; a traveller leaves the ship's view
  delivery at departure and joins the field's place on arrival (the ship's, on the way back).
- On the ground each player is still alone in this increment, and the field says so.
- GUI first (CLAUDE.md): Server Settings > ADMIN > Trips (a route on or off, seats per departure, how
  often, the server's trip mode), listed in data/admin/ops_registry.json.
- The web shows your own booking read-only, as it shows your fleet ledger.

Proof:
- Relay tests (each seen red): a seat holds across a sign-off and the traveller arrives at the field
  while offline; a second booking on one departure is refused; boarding from outside the circle, or
  without a booking, is corrected like any oversized jump; the departure board carries no names; a
  database written by v0.1461.0 opens with the new table (a fixture, unedited).
- `just verify-second-player` gains a leg: the scripted player books and boards; the game sees it
  leave and not return before its arrival time.

### Increment 4, one to two days: the hangar you walk to

What changes:
- `hangar-1` becomes a zone with a floor, walls and the big doors, joined to the Commons by a
  straight corridor from the Commons' east wall (x 99) to the hangar's west wall (x 200) at z 30, the
  stretch where the two overlap (z 20 to 40). It is clear of every plot in the shipped file and in the
  more-plots lane's working copy as of 2026-10-05 (plots p1 to p12 all stand at x 0 to 55).
- The boarding circle moves into the hangar (a data edit); the capsule stands in one of the cradles
  the hangar already draws; the departure board becomes an in-world screen (the screens of
  v0.1313.0).
- **Wait for the more-plots lane to merge first:** it rewrites the same ship file.
- **The ship hash changes,** as it did in ship-homes increments 3 and 4: a game built before this
  increment is refused as "a different ship" by a relay running it, until both are updated.

Proof: relay tests (the hangar is a room; the corridor is clear of every plot); the co-presence rig
walks the game to the hangar and boards, and the walker sees it board and the capsule leave.

### Increment 5, three to five days: meeting at Silverdale

What changes:
- Within a place on the ground (`site:<id>`, from the route's field, named on the entity since
  increment 3): the speed check and view delivery, as aboard.
- Weather seeded from the place and the game day instead of the computer's random source, so every
  game at the field rolls the same weather (no traffic). In your own world nothing changes that you
  could notice.
- Building on the ground stays private to your save until shared building lands.

Proof: the co-presence rig run on the field with two players, judged as the Commons meeting is (seen,
steady, on the line, in view); both games' weather records equal for the whole run. Red: the old
random source.

### Increment 6, three to five days: Realistic trips

What changes: the Settings row (section 5); seats from the stocked fleet; propellant and ablator as
goods drawn per trip; weather limits with holds and an alternate field (data); the landing ellipse; the
couch's real limits; your carried mass checked against the seat's allowance.

Proof: one test per rule, each seen red; a rig leg with the weather forced past the limits sees a hold
and then a diversion to the alternate field.

### Increment 7, later: your own capsule

The Spacecraft Pod (data/items.csv:365) as a kit; engines, propellant, parachutes and a heat shield as
items with bills; flying the entry yourself by rolling the capsule to steer its lift, the way Apollo's
crews steered; the pod brought back up as cargo on the shuttle, since a capsule cannot launch itself.

### Increment 8, later: more places

More fields as rows (any region with open ground); the Moon (no air: landing on engines alone);
splashdown routes with a boat to shore, once boats drive.

**Why this order.** Increment 1 closes the Normal-mode gap fastest. Increment 2 makes it the 2030
version. Increment 3 brings it to the shared server, where most players will be. Increment 4 makes
boarding the in-person moment ship-homes section 3 describes, once the ship file is free. Increment 5
lets people meet down there. The deep mode and your own vehicle come after the trip itself is good.

---

## 8. Open questions for the operator

Each has a recommendation.

1. **When is now?** The lore sets the game after the fleet left Earth (docs/game/humanity_one.md:22-23);
   the code parks the ship over Silverdale. Recommend: the present is the time the fleet is still at
   Earth, written into the lore doc. Routes are per body in data, so the same shuttle serves whatever
   the fleet orbits later, and your FTL answer decides how it leaves.
2. **Keep the ship where it is?** Recommend keeping the synchronous orbit: its day matches
   Silverdale's, the ship stands in Silverdale's sky, and the lighting aboard is built on it. The price
   is the 7 to 8 hour trip. A low orbit would make the trip about an hour, but give 16 sunrises a day
   aboard (data/stations/home.ron's own comment).
3. **Seven to eight hours each way on the real-time server.** Recommend accepting it: ride along with
   the other passengers, or sign off and arrive; the seat holds. A faster schedule waits for the "fast
   mode" of your 2026-10-04 answer.
4. **Which clock runs a trip?** Recommend the game clock above the air and real seconds in it
   (section 4.5).
5. **Which vehicle?** Recommend the Soyuz pattern on the way down (parachutes and landing rockets on a
   field: the most-flown crewed landing on land, needing no prepared pad, and you watch the chutes
   open), and a reusable booster from a pad beside the field on the way up. A splashdown with a boat,
   and a spaceplane to a runway, can be other routes later.
6. **Where exactly at Silverdale?** Recommend you pick a public open space at the edge of town; it
   becomes a data row; a test proves it is open ground; and the game names the liberty it takes. Never
   near a private home.
7. **How often?** Recommend every 6 game hours in Simplified (an assumption), once a day in Realistic
   (the shortest wait, since the geometry repeats daily).
8. **Dying on the ground.** Recommend a respawner in the field's shelter, used while you are on that
   body. In Realistic your pack still lies where you fell, which is now a walk, not a 7-hour trip.
9. **Building on Earth beside real homes.** Recommend building only on open ground (no mapped building
   or road under the piece), within a set distance of the field, private to your save until shared
   building, and never at a private address. Your call, since the town is real.
10. **What a seat is worth in the ledger.** Recommend the propellant and heat-shield share at their
    prices once they exist as goods; until then a named placeholder of 0, with the line still written.
11. **Dev's Land.** Recommend its Earth targets come from the same landing-site rows (the Silverdale
    field first, the Oahu coast kept as a second row), not from a coordinate in code.
12. **Say so today?** Recommend increment 0 now: an hour, removed when increment 1 ships.

---

## Appendix A: stale or conflicting things found while writing this

- **docking.ron's ports and procedures would load empty:** the file calls them `docking_ports` and
  `docking_procedures` (data/docking.ron:11, :213), the loader reads `ports` and `procedures`
  (src/systems/docking.rs:26, :29). The system is not registered anyway.
- **transportation.ron's space list would load empty:** the file calls it `space`
  (data/transportation.ron:142), the loader reads `space_infrastructure` (src/systems/transportation.rs:22).
  The system is never constructed (ship-homes section 5.1).
- **Three module headers name data files that do not exist:** data/vehicles.csv
  (src/systems/vehicles/mod.rs:4), data/ship_classes.csv (ships.rs:3), data/propulsion.csv
  (propulsion.rs:3; also noted in [gravity-and-movement.md](gravity-and-movement.md), lines 247-254).
- **Dev's Land writes Earth's target in code** (src/lib.rs:6012-6016), against the locale rule
  (locale.json:2).
- **The Shuttle item is "Personnel transport between stations"** (data/items.csv:404), and the
  Spacecraft Pod's bill (data/recipes.csv:562) has no engine, propellant, parachute or heat-shield part,
  because none exists as an item; its ceramic plates are the nearest thing to a heat shield.
- **Weather is rolled per game** from the computer's random source (src/systems/weather.rs:311); a
  shared place needs it seeded (increment 5).
- **Lore against code:** the lore's present is after the fleet's departure, the code's ship is over
  Silverdale (question 1). A question, not a bug.

## Appendix B: the arithmetic

**Inputs.**
- Earth's gravitational parameter 3.986004418e14 m3/s2 (src/station/orbit.rs:55); Earth's radius
  6,371 km (the game's).
- The ship's period: 86,400 s, synchronous with the solar day (src/station/orbit.rs:59; src/lib.rs:3362).
- The air begins at 120 km (Soyuz's entry interface is 122 km, ESA).
- A survivable entry angle: 5 to 8 degrees below the horizon, 6.5 nominal (assumption, Apollo-class).
- Silverdale: 47.6445 N, 122.6949 W (locale.json:16-19); the ship over 122.3 W (data/stations/home.ron:42).
- Low orbit 200 km for the climb, 300 km for the fast-transfer comparison.
- The air's scale height: 7.1 km (assumption), for the peak-deceleration estimate only.

**Methods.** Kepler's third law for the ship's radius; the vis-viva equation for speeds; half the
transfer ellipse's period for each coast; Lambert's problem (universal variables, as in Bate, Mueller
and White's textbook) for the off-equator and faster transfers, searched over coast times from 20
minutes to 15 hours, single arcs both ways round; the rocket equation for propellant; Allen and
Eggers' peak deceleration for a ballistic entry, the entry speed squared times the sine of the entry
angle, over twice e times the scale height (NACA Report 1381).

**Results.**

| Quantity | Value |
|---|---|
| The ship's radius, height, speed | 42,241 km, 35,870 km, 3.07 km/s |
| Earth's width seen from the ship | 17.4 degrees |
| The ship seen from Silverdale | due south, 35.3 degrees up, 38,240 km away |
| Cheapest drop: burn, coast, entry speed | 1.49 km/s, 5 h 15 min, 10.3 km/s |
| Falling straight down from rest instead | 4 h 8 min, after a 3.07 km/s burn |
| Entering at 15 / 25 / 35 / 47.6 N directly | 2.2 / 2.8 / 3.4 / 4.3 km/s |
| A single arc into the air over Silverdale at 5 to 8 degrees | none found |
| The cheapest arc that does reach the air over Silverdale | 2.5 km/s, 4.9 h, entering 47 degrees steep |
| Ballistic peak deceleration at 10.3 km/s, 47 / 6.5 degrees steep | about 203 g / about 31 g |
| Ship to a 48-degree low orbit | 2.3 km/s, 5 h 16 min, then about 2.5 km/s |
| Coast 5.3 h against 3.7 h (coplanar) | 3.9 against 5.9 km/s |
| A 200 km low orbit to the ship, from 0 / 28.5 / 47.6 degrees | 3.94 / 4.29 / 4.78 km/s |
| Propellant for 4.78 km/s at 370 s | about 2.7 t per tonne delivered (mass ratio 3.73) |
| The 6 to 7 hours above the air, waited out at 7,200x | about 3 to 3.5 real seconds |

The scratch scripts that produced these are in the session's scratchpad, not the repo; increment 1's
`scripts/plan-surface-route.js` is where they would go to stay.

## Sources

Each with the date the page itself carries.

- ESA, "Way back to Earth" (PromISSe, 2012 mission page, no date shown): https://www.esa.int/Science_Exploration/Human_and_Robotic_Exploration/PromISSe/Way_back_to_Earth
- NASA, "Splashdown! NASA's Orion Returns to Earth After Historic Moon Mission", 2022-12-11: https://www.nasa.gov/centers-and-facilities/hq/splashdown-nasas-orion-returns-to-earth-after-historic-moon-mission/
- NASA, "NASA Identifies Cause of Artemis I Orion Heat Shield Char Loss", 2024-12-05: https://www.nasa.gov/missions/artemis/nasa-identifies-cause-of-artemis-i-orion-heat-shield-char-loss/
- K. J. Stroud and D. M. Klaus, "Spacecraft Design Considerations for Piloted Reentry and Landing", NASA JSC and University of Colorado, NTRS 20080026216, 2006: https://ntrs.nasa.gov/citations/20080026216
- NASA, "SpaceX Dragon Splashes Down off the Coast of California", 2025-05-25: https://www.nasa.gov/blogs/spacestation/2025/05/25/spacex-dragon-splashes-down-off-the-coast-of-california/
- Varda Space Industries, W-1 mission page (no date shown): https://www.varda.com/mission/w-1
- Spaceflight Now, Dream Chaser's debut delayed and no longer docking, 2025-09-26: https://spaceflightnow.com/2025/09/26/sierra-spaces-dream-chaser-debut-mission-delayed-again-no-longer-docking-to-station/
- Wikipedia, "Starship flight test 11", edited 2026-06-03: https://en.wikipedia.org/wiki/Starship_flight_test_11
- SpaceDaily, on orbital refuelling after Starship's 13th flight, 2026-08-27: https://spacedaily.com/t-starship-13-test-flights-orbital-refueling-mars-hurdle
- TechCrunch, Impulse Space's same-day transport to GEO, 2025-09-16: https://techcrunch.com/2025/09/16/same-day-delivery-comes-to-space-as-impulse-promises-satellite-transport-in-hours-not-months
- Wikipedia, "Delta-v budget", edited 2026-07-02: https://en.wikipedia.org/wiki/Delta-v_budget
- H. J. Allen and A. J. Eggers, NACA Report 1381, 1958: https://ntrs.nasa.gov/citations/19930091020
- K. Sutton and R. A. Graves, NASA TR R-376, 1971-11-01: https://ntrs.nasa.gov/citations/19720003329
- TheGamer, on Starfield's landings, 2023-01-18: https://www.thegamer.com/starfield-wont-feature-seamless-space-planet-landing/
- GamingOnLinux, Kerbal Space Program 1.0, 2015-04-27: https://www.gamingonlinux.com/2015/04/kerbal-space-program-reaches-version-10-has-a-bunch-of-new-stuff/page=1/
- Star Citizen Alpha 3.10.0 release date (2020-08-05), starcitizen.tools: https://starcitizen.tools/Update:Star_Citizen_Alpha_3.10.0 ; patch notes: https://robertsspaceindustries.com/spectrum/community/SC/forum/190048/thread/star-citizen-alpha-3-10-0-live-5789362-patch-notes
- Nextgov, on the Reentry simulator, 2025-11-14: https://www.nextgov.com/emerging-tech/2025/11/learning-fly-how-reentry-simulates-nasas-most-daring-missions/409534

## Related docs

- [ship-homes-and-logistics.md](ship-homes-and-logistics.md): departures, boarding in person, goods
  transport, the fleet ledger, the shared-world increments this builds on.
- [gravity-and-movement.md](gravity-and-movement.md): travel times, the no-FTL decision and its
  2026-10-05 reopening, the g tolerances.
- [death-and-your-pack.md](death-and-your-pack.md): what a death costs, and where the pack lies on a
  planet.
- [the-mothership.md](the-mothership.md): the ship runs without you; you are one of the crew.
- [decision-briefs.md](decision-briefs.md): Brief 6, the one clock.
- [first-hour-audit-2026-10-04.md](first-hour-audit-2026-10-04.md): the first hour this gap sits in.
