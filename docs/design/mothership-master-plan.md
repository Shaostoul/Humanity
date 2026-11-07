# The mothership master plan: the whole ship, where everyone lives, and the fresh start

Status: a PLAN, written 2026-11-06. Nothing in it is built, and the wipe waits for the operator to
read it. Every operator decision is quoted with its date and source; everything else is marked
**Proposal** and is this document's reasoning, never his. The audit it rests on is
[world-audit-2026-11-06.md](world-audit-2026-11-06.md).

The operator, 2026-11-06: "I guess this is further incentivizing our need to get the whole
mothership laid out! Right now our game world is very haphazard with homestead and community
elements weirdly placed with some just floating in space. I was thinking we should do a wipe of
the entire homestead and start fresh to make sure we get all the weird artifacts."

## 0. In short

- **The ship.** The UHS Mothership is a straight, non-rotating SPINE with spinning DRUMS strung
  along it, a BOW where craft dock and a STERN where the drive burns. Sector 1, the first drum,
  is the one the operator sized himself: 250 m radius, 2 km long, 1 g at the floor at 1.89 rpm
  (2026-09-15). Everything walkable in this plan is inside Sector 1, at real scale. The rest of
  the ship is data, drawn from outside and counted, not walked (section 3).
- **Where people live.** Homes stand on the drum floor at 1 g, grouped in neighbourhoods of a
  few hundred people around a node (a commons with a mess hall, a recreation room, washrooms, a
  garden and a market row), every home within 5 minutes' walk of it. Industry, the reactors,
  storage and hangars are in the spine, behind the bearing that separates it from the drum
  (sections 3.4 to 3.6).
- **Arrival.** A transport docks in Hangar Bay 1 in the bow; the receptionist's desk is in the
  Arrivals hall beside it; a lift runs down the fore end cap to the Fore Landing on the drum
  floor, where General Services (barracks, public respawner, medbay) stands next to the lift
  foot; then the walk or the rail to your home (section 3.7).
- **The wipe.** Remove the whole ship file's layout (14 districts, First Street, the South
  Commons, the Commons and its old-home walls, 28 plots), the wild nodes and fixed quest places
  that were authored for a planet, the code that places the hologram and the guest's home, six
  orphaned data files and the relay's Pioneer upgrade. Keep the homestead and cabin designs
  (they are good and tested), the machines, the starts, and the operator's character (section 6).
- **Rung 1.** Neighbourhood 1 on Sector 1's floor, still flat: a continuous ground under
  everything (no more void), 12 homestead plots and a 16-cabin block around a rebuilt commons
  whose garden sits in its own room, every placement in data, and the new gates (nothing stands
  in the void, no machine crosses a wall, every light inside its body, every frame named), each
  seen red against today's data first (section 6.4). Rung 2 rolls that floor into the drum.

## 1. What is decided (the operator's words, dated)

| Date | Decision | Source |
|---|---|---|
| 2026-06-29 | The mothership needs a laid-out superstructure: "figure out how we lay out the superstructure of the mothership -- how to lay out rail lines, elevators, teleporters, hangar bays, mech bays, cargo tunnels, storage areas, massive industrial factory areas. We'll want to figure out how to build the public meeting zone / the shopping mall." | [mothership-superstructure.md](mothership-superstructure.md):7-10 |
| 2026-07-01 | Noise travels through the frame: "if the industrial area is too close, no one can sleep, the noise comes through the steel frame of the ship." | mothership-superstructure.md:55-56 |
| 2026-07-01 | The population is an aggregate, not rendered people: "we don't literally have to render everything and do physics for everything, most things are just simple calculations like going to x to spend x time then go to do this for x time, at any given time everything is being done by a certain percentage of people." | mothership-superstructure.md, open questions |
| 2026-09-15 | Two habitat kinds: a SPINE ("skyscraper like blocks that contain everything necessary to survive, like industry, residential, research, in a rough cube shape (or configurable)") and a DRUM ("the inside of a spinning drum where the gardens are visible from the inside... allows for gravity while not accelerating"). | [habitat-generation.md](habitat-generation.md):3-13 |
| 2026-09-15 | **First drum: 250 m radius, 2 km long.** "The smallest comfortable makes perfect sense." | habitat-generation.md:20-23 |
| 2026-09-15 | "The whole point of doing the drum/ring shape is to have the spin and gravity. If the spin were to stop then I'd like gravity to go away." | habitat-generation.md:27-29 |
| 2026-09-15 | The spine has thrust gravity: "the most common state of the fleet is going to be in the perpetually forward motion going between stars so it makes sense the spine has gravity." | habitat-generation.md:62-67; [gravity-and-movement.md](gravity-and-movement.md):3-11 |
| 2026-09-15 | "the spine of the ship has to be at the center of the hyperboloid and the rotating outer mass has to be on the outside of the hyperboloid shape." | gravity-and-movement.md:419-420 (commit 2068406a4) |
| 2026-09-15 | A crowd: "If we can efficiently simulate 500 (or whatever is a doable amount on our weak hardware) simple AI humans in one mothership sector ... Eventually we want a mothership that houses billions of simple AI crew." | [crowd-simulation.md](crowd-simulation.md):3-8 |
| 2026-09-15 | Normal play edits only its own zone ("normal play will have edit area confined to their default zone"); co-op "has to support at least a dozen." | [game-modes.md](game-modes.md):67-68, :273-278 |
| 2026-09-19 | The ship is premade: "the homeship is meant to be premade. Kind of like a giant flying lived in city." It runs without the player: "The player just happens to be one of the crew members." | [the-mothership.md](the-mothership.md):14-22 |
| 2026-09-19 | A player modifies only "their specific home area in the spaceship. Like the 1 acre or whatever it is for that specific ship design." | the-mothership.md:27-30 |
| 2026-09-19 | The Fibonacci homestead "is not a rule": "mostly just me trying to figure out how to lay things out and scale things." | the-mothership.md:39-43 |
| 2026-09-19 | One good home, then many: "When we make a great player home then we can mirror that to all the NPC homes"; homes are data; players may submit designs. | the-mothership.md:117-133 |
| 2026-09-19 | "the mothership has a population of over a billion" (said of NPC cost). | the-mothership.md:89-95 |
| 2026-09-27 | Power: "For now we can base power budget on nuclear reactors and then players can build solar ... essentially provide unlimited (at least to start)". | data/ship_power.ron:3-9 |
| 2026-10-03 | Homes: "I would actually like the player homes in multiplayer to actually have their own locations"; "I'd like for player build area to be limited to their specific home. I don't like the idea of homes overlapping."; "Players could meet anywhere on the mothership ideally. The mess hall, recreation room, crafting areas, hangar where all the private spaceships are stored, or whatever."; goods "transported either by the player or an automated robot or some other method"; "We really want it to logistically make sense. Theoretically HumanityOS could be used for city planning." | [ship-homes-and-logistics.md](ship-homes-and-logistics.md):13-21 |
| 2026-10-03 | The full list of home kinds ("I like the full list as it adds variety and allows for a full spectrum of colony spaceship designs"); the full homestead is the default start; cabins and apartments eat from communal gardens and the ship's farms. | ship-homes-and-logistics.md:25-28 |
| 2026-10-04 | The fleet is unlimited in early development, with a per-player ledger "in the red or black". | ship-homes-and-logistics.md:923 |
| 2026-10-04 | Server worlds start fresh: "only their appearance is kept." Normal mode and his MMO server run in real time ("let's have everything be real time, not the 72x"). About ten more plots so more than two people can have homes. | ship-homes-and-logistics.md:1257, :1265, :1266 |
| 2026-10-20 | The homestead's upper floor: "Sounds like a plan, let's proceed". | [ship-homes-increment-7-plan.md](ship-homes-increment-7-plan.md):5 |
| 2026-11-02 | The arrival: "arriving as a new crew member to the main colony ship ... Flying in from space on a transport spacecraft with other people and cargo ... Start the player off with a bedroom of their own. Then upon leaving the ship they check in with admin guy like a receptionist and choose their play style (simple home, advanced, or whatever). If they never check in and something happens to them they go to general services like a barracks and medbay. With a proper home players could respawn at their home or choose where they want to respawn." | [game-setting-and-intro.md](game-setting-and-intro.md):66 |
| 2026-11-03 | "Agreed" to: small craft jump, the motherships never do and cruise under thrust; weapons for things that are not people, sport by consent in the Arena. | game-setting-and-intro.md:1099-1116 |
| 2026-11-06 | The ship's hum: "I was also thinking we could incorporate a subtle brown noise that acts like the hum of the engines and the resonance of everything?" The orchestrator's journal records the decision as a hum "tuned to the music's key and louder near the engines". | [music.md](music.md):37-38; data/coordination/orchestrator_state.json, 2026-11-06 evening |
| 2026-11-06 | "do a wipe of the entire homestead and start fresh" (quoted above), after reading a plan. | this task's brief |

Standing rules that shape the plan (CLAUDE.md): infinite-of-x (anything that exists more than
once is data); no backwards compatibility before v1.0, except the version-tagged blueprint schemas
that must keep loading old player saves; realistic-first (build the 2030 technique as a rung of
the real thing); every deep system has a full-realism and a simplified mode; compute and bandwidth
are a hard budget; never trim dev or teaching tools.

## 2. Where the pieces disagree, and how this plan settles them

| Conflict | The sources | This plan (Proposal unless quoted above) |
|---|---|---|
| How big is the ship? | A 250 m x 2 km first drum (decided); "a population of over a billion" (decided wording); docs/game/humanity_one.md's 500 km x 100 km ship for 10 billion (vision, never decided); today's 1.2 km flat slab (built) | A 2 km drum holds thousands, not billions (section 3.1 does the arithmetic). Sector 1 is the decided drum and the only walkable sector; later sectors are larger rings that the ship's data counts and the exterior shows. Question 1. |
| Cylinder or hyperboloid? | The operator's hyperboloid line (2026-09-15); the 150 m waist and its gravity table are an agent's elaboration (gravity-and-movement.md:430-448) | At those numbers the floor slopes 9.1 degrees along the axis at the flared ends, so it would have to be terraced. Sector 1 is a plain cylinder; the hyperboloid is kept for the ship's outer form. Question 2. |
| At Earth or between the stars? | The ship orbits over Silverdale today (data/stations/home.ron); the lore says between the stars (game-setting-and-intro.md question 2; leaving-the-ship.md question 1) | Inside a drum the light comes from the spine's sunline either way: piped sunlight near a star, lamps between stars. Question 3. |
| Where do you wake? | Today: your front door (the home's `spawn`); the 2026-11-02 brief: home, or a place you choose, or General Services; GSI: a pod beside the bedroom | Respawn places are data rows (section 3.7); a home's pod waits for respawn-and-revival.md's answers. |
| The old intro | docs/game/intro_sequence.md: private landing ports, quarters on a habitat ring | Superseded by the 2026-11-02 brief; game-setting-and-intro.md question 13 asks to retire it. |
| One flat deck or a drum? | Everything built stands at y 0; every decision says drum | Rung 1 lays out the drum floor unrolled (flat, in drum coordinates); rung 2 rolls it. One layout, one set of coordinates, a curvature switched on. |

## 3. The ship at real scale

### 3.1 The mothership: a bow, sectors on a spine, a stern

```
 SIDE VIEW (not to scale between parts; along the axis, the bow faces the direction of travel)

  gateway          BOW                SECTOR 1              SECTOR 2 ... SECTOR N          STERN
  (a point     +----------+   +=========================+   +======== ... ========+   +-----------+
   in space)   | Hangar   |   |   DRUM 250 m radius,    |   | larger rings,       |   | main drive|
  transports   | Bay 1,   |===|   2,000 m, 1.89 rpm     |===| counted, drawn      |===| reactors, |
   arrive -->  | Arrivals,|   |   <- SPINE (non-rot.) ->|   | from outside        |   | radiators |
               | freight  |   +=========================+   +======== ... ========+   +-----------+
               +----------+   ^ fore end cap  aft end cap ^                            the hum is
                 0.05-0.1 g   (spoke lifts)  (risers,                                  loudest here
                 (thrust)                     freight lifts)
```

| Part | Size | Gravity | Holds | Status |
|---|---|---|---|---|
| Bow | Proposal: 120 m across, 300 m long, non-rotating | thrust only (cruise 0.05 to 0.1 g, question 10) | Hangar Bay 1 and further bays, the Arrivals hall and the desk, mech bays, freight intake | Proposal |
| Sector 1 drum | **250 m radius, 2,000 m long** (decided 2026-09-15) | 1.00 g at the floor (0.988 g at 3 m up, 0.976 g at 6 m) | homes, neighbourhood commons, the Concourse, General Services, the greenbelt, the ship's farms | Proposal for its contents |
| Sector 1 spine segment | Proposal: 60 m across, 2,000 m long, decks across the axis every 5 m (10 m for industry) | thrust only | sector engineering (reactors, air, water, waste), industry, storage, the freight rail | Proposal |
| End caps | discs of 250 m radius at each end of the drum | 0 at the axis to 1 g at the rim | spoke lifts (4 a cap), bearings, utility risers | Proposal |
| Further sectors | data rows; the lore's size (question 1) | | the rest of the population, as an aggregate | Proposal |
| Stern | Proposal: data; the main drive, the main radiators, propellant | thrust | the ship's loudest machinery | Proposal |

**The arithmetic of size.** Closed-loop-test.md puts each person at 50 m2 of lit crops and 10.8 kW
of electricity (closed-loop-test.md:60-62, :79-81). Sector 1 holds about 2,400 to 9,000 people depending
on the mix of homes (ship-homes-and-logistics.md section 2.7, recomputed in 3.5 below). A billion
people would need on the order of 100,000 such drums, about 200,000 km of ship. So "over a
billion" and "a 2 km first drum" are both kept only if the ship grows in sectors of very
different size: the first small and walkable, the later ones rings of kilometres to tens of
kilometres in radius (humanity_one.md's 100 km diameter holds billions). The aggregate tier of
crowd-simulation.md counts them at no per-frame cost. Question 1 asks him which numbers to write
down.

### 3.2 Sector 1: the drum

```
 END VIEW (looking aft), to scale: 1 character = about 25 m

                    . - ~ ~ ~ - .
               .'   drum floor   '.        floor radius 250 m (1.00 g)
             /    (homes, farms)    \      spin: 0.198 rad/s = 1.89 rpm, a turn in 31.7 s
            |                        |     rim speed 49.5 m/s
            |       ( SPINE )        |     spine 60 m across, non-rotating, on bearings
            |       sunline ->       |     the sunline: the spine's light strip, 220 m above
             \                      /      every point of the floor
               '.                .'        air: 3.93 x 10^8 m3 (the whole drum)
                    ' - ~ ~ ~ - '
```

- **Numbers** (computed from the decided 250 m x 2 km): floor area 3.14 km2 (776 acres),
  circumference 1,571 m. The air of the whole drum is 393 million m3, so one home's leak or one
  fire is a small change to the ship's air (a buffer the per-home model in
  [ship-life-support.md](ship-life-support.md) does not yet have).
- **Spoke lifts** run down the end caps, rim to axis 250 m: 100 s at 2.5 m/s, with about 0.1 g of
  sideways Coriolis push at that speed (ship-homes-and-logistics.md section 2.6). Full realism
  rides it; simplified mode fades through it.
- **Under thrust the floor leans**: 2.9 degrees at 0.05 g cruise, 5.7 degrees at 0.1 g
  (gravity-and-movement.md:58-70). Question 10.
- **A flat home does not fit a 250 m floor.** A 55 m wide homestead built flat on the curve
  stands 1.51 m off the floor at its edges and its floor leans 6.3 degrees there against local
  gravity (computed). So homes are not placed flat on the drum: their designs stay authored flat in
  their own metres and are ROLLED with the drum, walls radial, floors following the curve (rung
  2). That is the same "one generator, a curvature parameter" idea habitat-generation.md:77-86
  proposes.

### 3.3 The spine, the bow and the stern

The spine does not rotate, so its gravity is the drive's thrust, pointing aft. Its decks are discs
across the axis ("down" is aft), stacked like a skyscraper lying along the axis. At 0.05 to 0.1 g a
person weighs a tenth of their weight or less: right for what habitat-generation.md:56-60 says a
spine is for (moving mass, big structures, docking), and wrong for daily life, so nobody lives
there (ship-homes-and-logistics.md section 2.8, Proposal).

**Sector 1's spine segment** (Proposal), fore to aft:

| Segment (axial m) | What | Why there |
|---|---|---|
| 0 to 150 | Fore hub: the spoke lifts' tops, the passenger rail station | the lifts meet the axis at the end cap |
| 150 to 900 | Storage and the cargo tunnels (freight rail along the axis) | goods physically moved (decided); low gravity eases handling |
| 900 to 1,500 | Industry: fabrication, the factory district, machine shops | the noise rule: as far as possible from the fore landing's homes, isolated from the drum by the bearing |
| 1,500 to 1,850 | **Sector engineering**: a reactor pair of the game's class (two KLT-40S class, 60 MW net, closed-loop-test.md:64-66), switchgear, the air plant, water recovery, waste processing | near the aft end cap's utility risers; one pair feeds about 5,560 people at 10.8 kW each (computed) |
| 1,850 to 2,000 | Aft hub: freight lifts down to the Aft Landing, utility risers | |

**The bow** (Proposal): Hangar Bay 1 (today's district is 50 x 12 x 40 m; it becomes a walkable
zone with big doors, ship cradles and a launch lane, as [mothership-superstructure.md](mothership-superstructure.md)
M5 describes), more bays as arrivals grow (game-setting-and-intro.md:811), mech bays beside them,
the Arrivals hall with the receptionist's desk, freight intake and customs. The transports dock
here because docking "does not fight a rotating frame" (habitat-generation.md:56-60).

**The stern** (Proposal): the main drive, the main radiators, the propellant. Radiators matter at
this scale: 67 m2 of fin per person (closed-loop-test.md:84), about 0.34 km2 for 5,000 people,
mounted on the non-rotating spine outside the drums.

### 3.4 The drum floor, zoned

```
 SECTOR 1 DRUM FLOOR, UNROLLED (to scale: 1 character = about 33 m across, 50 m along)
 around (s) ->  0 ......................... 785 ......................... 1571 (= 0 again)
 along (a)
   0 +-----L1--------------------------L2-------------------------L3--------------L4-+
     | FORE LANDING: spoke-lift feet L1-L4, Arrivals foot (L1), General Services,    |
 120 | the Concourse (market, food hall, fleet board, memorial), rail station        |
     +-------------------+-------------------+-------------------+-------------------+
     | N1 (rung 1)       | N2                | N3                | N4                |
     |  FORE RESIDENTIAL: dense near the landing (cabins, apartments, townhouses,    |
 420 +-------------------+-------------------+-------------------+-------------------+
     | N5                | N6                | N7                | N8                |
     |  houses); each cell 393 m around x 300 m along, one node at its middle        |
 720 +-------------------------------------------------------------------------------+
     | GREENBELT: park, the lake (water store and thermal mass), orchards, the       |
     | arena field, school and Library hall, a clinic, the mid-drum rail station     |
1000 +-------------------+-------------------+-------------------+-------------------+
     | H1                | H2                | H3                | H4                |
     |  HOMESTEAD RING: homesteads facing the greenbelt and the farms                |
1300 +-------------------------------------------------------------------------------+
     | THE SHIP'S FARMS: fields, greenhouses, aquaculture, barns, seed bank          |
     | (lamp-lit between stars: 4.8 kWh per m2 a day for staple crops)               |
1880 +-------------------------------------------------------------------------------+
     | AFT LANDING: freight lifts, depot, food processing, recycling and compost,    |
2000 | the utility risers (the drum floor's noisiest band)                           |
     +-------------------------------------------------------------------------------+
```

| Band (a, m) | Area | Use | Rules it serves |
|---|---|---|---|
| 0 to 120 | 0.19 km2 | Fore Landing: Arrivals foot, General Services, Medical Bay, the Concourse | game-setting-and-intro.md:730-732 (General Services beside the Medical Bay and near Arrivals); mothership-superstructure.md M3 (the civic heart) |
| 120 to 720 | 0.94 km2 | Fore residential, 8 neighbourhoods N1 to N8 | density follows transit (ship-homes-and-logistics.md section 2.8) |
| 720 to 1,000 | 0.44 km2 | Greenbelt and civic | at least 10% parks (Perry, section 2.5); docs/game/ship_zoning_transit.md layer 2 |
| 1,000 to 1,300 | 0.47 km2 | Homestead ring, 4 neighbourhoods H1 to H4 | homesteads next to agriculture (section 2.8) |
| 1,300 to 1,880 | 0.91 km2 | The ship's farms | feeds the cabins and apartments (decided 2026-10-03); at 65 m2 of growing floor a person, about 14,000 people's food (computed from closed-loop-test.md:80) |
| 1,880 to 2,000 | 0.19 km2 | Aft Landing | freight and noise kept from homes (section 2.8) |

All Proposal. The bands are rows of a data file (section 4), not code.

### 3.5 Neighbourhoods

A neighbourhood is a cell about 393 m around (a quarter of the circumference, one spoke lift per
quarter) by 300 m along, around one NODE (Proposal):

- **The node, the neighbourhood commons:** a mess hall with its serving counter and food store, a
  recreation room, washrooms, a garden court (the commons garden in its own glass-roofed room), a
  market row, parcel lockers on the freight side, a clinic room and a Library room as those
  systems arrive (ship-homes-and-logistics.md section 2.5's node list).
- **Walking:** the farthest door is 345 m from the node along the streets in rung 1's N1 (4.1
  minutes at 1.4 m/s), inside the 5-minute rule that is already a test
  (`every_home_reaches_a_mess_hall_within_five_minutes`).
- **Size:** a few hundred people, the 150 to 500 band of the Dunbar proposal
  (mothership-superstructure.md, 2026-07-01).
- **Streets** are paved strips of the ground; homes face them; freight lanes run behind homes and
  never share passenger paths (planning rule, section 2.9 of ship-homes-and-logistics.md).

**Capacity of Sector 1** (Proposal, recomputed for these bands; 25% of each neighbourhood for
streets, the node and green):

| Mix | Per neighbourhood | 12 neighbourhoods | People |
|---|---|---|---|
| All homesteads (55 x 89 m) | about 18 plots | about 216 | 216 to 650 (1 to 3 a home) |
| Mixed (as N1: 12 homesteads, a 16-cabin block, then apartment blocks as they arrive) | about 300 people | | about 3,600 |
| Dense (cabins and apartments, the farms feed them) | about 700 people | | about 8,400 |

The mix is a per-sector setting in data (ship-homes-and-logistics.md section 2.7). Food land sets
the dense ceiling (the farm band feeds about 14,000) and power sets it too: lamp-lit food alone
is about 9.4 kW a person (closed-loop-test.md:82), so 8,400 people need about 1.5 reactor pairs
between the stars.

### 3.6 How a player's home sits in it

- **The plot is the acre** "or whatever it is for that specific ship design" (decided
  2026-09-19). Plot kinds are the home kinds (decided 2026-10-03): Cabin (3.15 x 2.5 m inside a
  block), Apartment (37 to 70 m2 in a block around a common house), Townhouse, House,
  Smallholding, Homestead (55 x 89 m, the default start), Farm. Each kind is a row of
  data/homes/starts.ron with its shipped design; Cabin and Homestead are offered today.
- **No overlap, build only inside your own plot** (decided 2026-10-03): `validate()` and
  `may_build` already enforce both.
- **Every plot stands on the ground.** A plot is a box ON the drum floor's ground layer, never
  floating in a gap. Its door opens onto a street of the ground; the corridor tubes of today exist
  only where a building joins another building (a cabin's door into its block's hall).
- **Plots turn** (quarter turns, rung 3) so a street can have homes on both sides; until then all
  homestead doors face spinward (+s), as today they all face east.
- **Tiered starts** (decided 2026-10-03): the Homestead grows its own food on its acre; the Cabin
  eats crew meals at the node's mess hall; the Apartment (when built) shares a common house.
- **Unclaimed plots are lived in.** The ship is "a giant flying lived in city" (decided): every
  plot no player holds is an NPC household's, drawn as its kind's shipped design (built since
  increment 2), lit at night and furnished once the cosmetic tier carries lights and furniture
  (the-mothership.md: cosmetic crew are welcome first). Proposal.

### 3.7 Where a new player arrives

```
 THE ARRIVAL PATH (Proposal; the 2026-11-02 brief in order)

 gateway --transport (12+ seats, cargo)--> HANGAR BAY 1 (bow) --walk--> ARRIVALS HALL
                                             cargo unloaded,             the receptionist's desk:
                                             other craft in cradles      welcome, your bedroom, where
                                                                         you live (Cabin / Homestead
                                                                         / ...), death mode, where you
                                                                         wake, privacy, "Go home"
                                                                              |
                                                     arrival lift down the fore end cap (100 s)
                                                                              |
 never checked in? <-- GENERAL SERVICES (barracks, public respawner,  <-- FORE LANDING
                       medbay) beside the lift foot                         | the Concourse
                                                                            | the axial rail or
                                                                            v a walk
                                                                      YOUR HOME (N1: 4 minutes)
```

- **The transport** is a design file built with the construction tools (game-setting-and-intro.md:645-647),
  12 seats to start (data/ship/arrivals.ron, proposed there).
- **The desk** is a site for the relay's crew; the conversation is a panel on each player's own
  screen, so nobody queues (game-setting-and-intro.md:687-690).
- **Respawn** (the 2026-11-02 brief): at your home, or where you choose, or at General Services if
  you never checked in. Respawn places are DATA rows (a home's pod, a block's shared pod, the
  public respawner, a friend's with a permit), per game-setting-and-intro.md:747-760; how a body
  comes back is respawn-and-revival.md's.
- **Until rung 4 builds this**, a new player starts at their front door as today, and a guest at
  N1's commons.

### 3.8 Life support, power, engineering, the drive, and the hum

- **Sector engineering** sits in the spine's aft segment (3.3); its utility risers come down the
  aft end cap to the Aft Landing; the farms beside it are the biological half of life support
  (the crops make 0.62 to 0.76 kg of oxygen a person a day, closed-loop-test.md:85).
- **Homes keep their own loops** (ship-life-support.md models each home's air and water), drawing
  on the ship when short, as the Station-supplied mode does today; the ship's own supply,
  ledger and meter replace the household battery that powers the Commons' machines today (audit
  section 2.1). Fleet supply stays unlimited in early development (decided 2026-10-04).
- **The hum.** The operator, 2026-11-06: "I was also thinking we could incorporate a subtle brown
  noise that acts like the hum of the engines and the resonance of everything?"
  ([music.md](music.md):37-38). The music design makes hum sources data,
  `data/audio/hum_sources.ron` (kind, position in ship metres, strength, colour), each falling
  with distance and with every room boundary crossed, and places its first drive source at
  today's stern because "The ship has no engine space yet" (music.md:333-339). This plan gives
  those rows their places: the reactor pair, pumps and air plant of each sector's engineering
  segment (3.3), the farms' lamp power plant, the stern's drive. Proposal: the same rows feed the
  sleep-disturbance planning rule mothership-superstructure.md proposed on 2026-07-01 (one source
  list, two readers), and a zone type may carry a default source so every built engine room
  hums without a hand-placed row. On the drum floor the hum is loudest at the Aft Landing
  (structure-borne through the aft end cap) and quietest at the fore homes. Today the ambient
  bed is one non-spatial loop (data/sounds.toml, `[ambient.space_station]`, `spatial = false`).
- **The noise buffer** between homes and industry is open question 14 of
  ship-homes-and-logistics.md; this plan puts every loud source in the spine or at the Aft
  Landing, so the drum floor's rule becomes a distance from the aft end cap (question 11).

### 3.9 Hangars, vehicle bays, industry, storage, cargo

| Place | Where | Notes |
|---|---|---|
| Hangar Bay 1 and further bays | bow | transports, private craft (decided: "hangar where all the private spaceships are stored"), mission groups gather at a waiting transport (decided 2026-10-03) |
| Mech bays | bow, beside the hangars | cradles with port hookups (mothership-superstructure.md) |
| Vehicle bays | inside homes (the homestead's 35 x 22 m bay exists) and at the landings | ground vehicles drive the drum floor's streets |
| Industry, storage, cargo tunnels | spine | the freight rail runs the spine's length, freight lifts at both hubs |
| Freight depots | Fore and Aft Landings | parcel lockers at every node (ship-homes-and-logistics.md section 5) |

### 3.10 Medical, school, markets, mess halls, gardens, farms, sport

| Place | Where |
|---|---|
| Medical Bay and General Services | Fore Landing (3.7) |
| Clinic and Library rooms | each node, as those systems arrive |
| School and Library hall | greenbelt |
| The Concourse (the market, a food hall, the fleet projects board, the memorial) | Fore Landing |
| Market rows | each node |
| Mess halls | each node (the 5-minute rule) and the Concourse's food hall |
| Gardens | each node's garden court; each homestead's own; the greenbelt's orchards |
| The ship's farms | the farm band (3.4) |
| The Arena and the Armory | greenbelt (the field) and the bow (target ranges): sport by consent (decided 2026-11-03) |

### 3.11 Transit

| Mode | Route | Numbers |
|---|---|---|
| Walking | streets on the ground | 1.4 m/s design; the game walks at 5 m/s |
| Axial rail on the drum floor | Fore Landing, greenbelt, Aft Landing | 2 km in 3.4 to 4.8 minutes at 25 to 35 km/h (section 2.6 of ship-homes-and-logistics.md) |
| Spoke lifts | 4 down each end cap | 100 s rim to axis at 2.5 m/s |
| Spine passenger rail | along the spine, bow to stern | between sectors |
| Spine freight rail | along the spine | goods only (planning rule) |
| Small craft | bow hangars | between motherships (they jump; the motherships never do, decided 2026-11-03) |

The resilience rule of docs/game/ship_zoning_transit.md:77-110 stands: every critical place has a
fast route and a slow one, and no single lift or line failure isolates a district (the stairs in
each end cap are the slow route).

## 4. From one player to an MMO server (infinite-of-x)

Everything above is rows, not code (Proposal for the shapes):

| Data | What a row is | Example |
|---|---|---|
| `data/ship/sectors.ron` | a sector: its kind (drum or ring), radius, length, spin, its band rows, its spine segments, its neighbourhood instances | `(id: "s1", radius_m: 250.0, length_m: 2000.0, ...)` |
| `data/ship/bands/*.ron` | a band of a drum floor: from, to, use | `(from_m: 120.0, to_m: 720.0, use: "residential")` |
| `data/ship/neighbourhoods/*.ron` | a TEMPLATE: the node's design, street pattern, plot slots by kind, size | `n-mixed-a` (12 homesteads, 16 cabins, a commons) |
| ship file (data/blueprints/ship_structure.ron) | the authored or generated instances: neighbourhoods placed by (sector, s, a), the ground layer, zones, plots with stable ids | plot `s1-n1-h03` |
| `data/blueprints/zone_types.ron` | each zone type's colour, its default hum source and `thermal_output` | |
| `data/audio/hum_sources.ron` (music.md) | a hum source: kind, position in ship metres, strength, colour | the sector's reactor pair |
| `data/homes/starts.ron`, `data/homes/shipped/*.ron` | home kinds and their designs | existing |

- **Offline, one player**: N1 is built in full; your home stands on the default plot; every other
  plot is drawn as its kind's shipped design (built since increment 2). The rest of Sector 1 is
  drawn from the same templates at lower detail (section 5).
- **A server**: the relay hands out plots in a claim order (N1's first, then N2's, then the next
  sector's), by stable id (`game_plots`, built in increment 1b). A full kind gets a waiting list
  (open question 4 of ship-homes-and-logistics.md). Players meet anywhere; the relay delivers what
  is within 250 m and drops it past 300 m (data/ship/shared_world.ron), about one neighbourhood.
- **An MMO**: a neighbourhood is the unit of interest and of sharding; a full sector opens the
  next; the population beyond the players is crowd-simulation.md's three tiers (an aggregate per
  zone, a roster for the loaded sector, an embodied few near you), so the cost scales with what
  you can see, not with who lives aboard.
- **Generated or authored**: rung 3 adds a generator that turns a template plus a placement into
  plots with stable ids; any neighbourhood may instead be authored by hand in the editor (the
  construction tools serve both, decided 2026-09-19). The planning rules (5 minutes to a mess
  hall, 15 minutes to every daily need, no home within the noise buffer, freight apart from
  passengers, no single failure isolating a district) run as tests over whatever the data says.

## 5. Budgets: per frame, memory, network

**The frame**: 8.3 ms at 2560 x 1440 and 120 fps (game-setting-and-intro.md:884-885; his display
runs 120 fps). Today's interior deck measured 18.84 ms, most of it not NPCs (PRIORITIES section D,
rung 0.5), and increment 7's frame costs are still owed (ship-homes-increment-7-plan.md:278-281),
so these are targets to measure against, not readings.

| Tier | What is loaded | Detail | GPU target (Proposal) |
|---|---|---|---|
| L0 | the room you are in and the rooms of your home in view | full meshes, machines, lights, room GI | 3.0 ms |
| L1 | your neighbourhood: the node, neighbour plots within about 300 m | neighbours as baked shell groups of their kind's design (built), instanced since every neighbour of a kind is the same design; lights only on an open plot | 1.5 ms |
| L2 | the rest of the drum floor, the far side 500 m overhead included | one merged roof-and-canopy mesh per neighbourhood, rebuilt when a plot in it changes: in-memory, version-tagged, and obeying the Dev page's "turn off all reuse" switch (CLAUDE.md's no-caches rule) | 1.0 ms |
| L3 | the spine, the end caps, other sectors, the exterior | silhouettes from data | 0.5 ms |
| | sky, post, UI | | 2.3 ms |

- **Memory** (Proposal, to measure): L2 at about 15,000 triangles a neighbourhood, 16 in the drum,
  about 240,000 triangles; one homestead design's shell is shared by every instance of it. The
  draw-call cap (raised to 1024 in v0.528) argues for instancing neighbours, not more calls.
- **Simulation**: your own home's machines run in full; neighbours' homes are theirs and are not
  simulated in your game; the ship's machines run on the ship's supply in aggregate; crowds per
  crowd-simulation.md.
- **Network**: gameplay state only, at a low rate; cosmetic simulation is local and never
  networked (decided 2026-09-24, CLAUDE.md); the relay's view of 250 m in, 300 m out stays.

## 6. The wipe and the rebuild

### 6.1 What the wipe removes

**Data** (all in this tree; the operator's installed copy is in 6.3):

| Remove | Where | Why |
|---|---|---|
| The whole layout of the ship file: zones `commons`, `street-1`, `cabins-1`, `commons-2`; 3 corridors; 28 plots; 14 districts; `id: "mothership-1"` | data/blueprints/ship_structure.ron:100-1661 | Rebuilt for Sector 1 (rung 1); the new `id` starts every plot afresh on a relay, which is intended (no compatibility before v1.0) |
| The Commons' 15-wall room block (the superseded 12-room house) | ship_structure.ron:327-720 | Audit finding 5 |
| The Commons' six lights outside its box | ship_structure.ron:757, 768, 779, 790, 812, 823 | Audit finding 4 |
| The Commons' machine positions (garden, stalls, mess, recreation) | data/machines/ship.ron | Re-placed in N1's commons; ids kept so the garden's crops carry |
| All nine wild-spawn rows | data/entities/wild_spawns.ron:24-49 | Planet-era absolute positions (audit section 5); question 8 |
| The four fixed quest places | data/entities/destinations.ron (`outdoor_fields`, `wild_thicket`, `flax_hollow`, `wolf_ridge`) | Replaced by anchors that follow the player's plot, or retired with their steps |
| Orphaned files | data/world/spawn.ron, data/blueprints/habitat.ron, data/ships/bridge.ron, layout_medium.ron, reactor.ron, starter_fleet.ron (and their entries in crates/humanity-core/src/embedded_data.rs) | No reader (audit section 6) |
| `train-1`, `ramp-1`, `teleporter-1`, `teleporter-2` from the home | data/homes/homestead.ron and data/homes/shipped/homestead.ron | Tech-demo pieces; transit belongs to the ship (question 5) |

**Code that decides where things go:**

| Remove or change | Where |
|---|---|
| The hologram's fallback and `hologram_spot`: the hologram becomes a placed fixture of the home design | src/engine/world_load.rs:208, :1312-1314 |
| `HOME_AWAY_ORIGIN`: a guest's home is kept as data, not parked 1 km off and simulated | crates/humanity-ship/src/ship_structure.rs:423 and its users |
| Wild spawns and quest places read in the ship frame: rows name their frame (`Home`, `Ship`, `Planet { lat, lon }`) | src/engine/world_load.rs:951; crates/humanity-systems/src/quests/mod.rs:158-160 |
| The relay's furniture ring: the relay reads the same positions the game draws | src/relay/handlers/game_state.rs:679-695 |
| Unclamped light offsets: a light outside its body is refused at load | src/engine/home_meshes.rs:3100-3103 and the loader |
| The "respawner" room id lookup: room actions only | src/engine/world_load.rs:698 |
| The relay's Pioneer upgrade (compatibility code) | src/relay/handlers/ship_world.rs:304-380, game_state.rs:1631, tests/fixtures/relay/pioneer_world_v9.json |
| District drawing as floating filler | crates/humanity-ship/src/structure.rs (filler parts), until districts are real zones |
| Every zone's lights billed to each home's meter | the house lighting consumer in src/lib.rs (ship_structure.ron header :43-44) |

**Save fields** (no migration; CLAUDE.md, 2026-06-30 and 2026-10-23): `ship_place` (a plot id is
not trusted after a ship change and the home goes to the default plot of its kind: already the
code, src/engine/save_slot.rs:612-616), `home_plot_box`, `player_position`, `deployed_vehicles`
positions, `left_packs` positions. The relay's `game_plots` and `world_pieces` start empty under
the new ship id; the stored relay world (`PERSIST_KEY`) starts fresh. Server saves
(`saves/servers/<hash>-<key>.json`) are keyed by the old hash and are simply never found again.

**The version-tagged blueprint rule stays**: blueprint schemas that must keep loading old player
saves keep their version tags (CLAUDE.md); the ship file and home designs are not blueprints of
that kind, and this wipe touches no blueprint schema.

### 6.2 What the wipe keeps

- **The homestead design** (data/homes/homestead.ron, shipped copy): the 2026-09-19 redesign the
  operator asked for and the 2026-10-20 upper floor he approved; 30 rooms, every machine in its
  room, 55 lights inside its box, its screens (the console room's desk monitor showing chat, the
  study's Library, the bedroom's mirror) and its tests. It becomes the template every homestead
  plot draws.
- **The cabin design** and its machines.
- **The machine catalogue**, home.ron and home_solo.ron (positions are home-local, so they move
  with the home), ship.ron's catalogue and ids.
- **The starts** (data/homes/starts.ron) and the fleet ledger.
- **The eight-material showroom ring**, moved into a room of its own in N1's commons (question 7).
- **zone_filler.ron** as the registry real districts will use.
- **The operator's character** (6.3).

### 6.3 The operator's own save, his garden, and his data folder

His save `%APPDATA%\HumanityOS\saves\offline_home.json` (audit section 8):

- **Kept**: his name, appearance and outfit, inventory (56 stacks, 66 placed items in containers),
  10,164 credits, skills, quests. Nothing in the wipe touches these fields.
- **Reset by the ship change itself**: his plot (the home lands on rung 1's default homestead
  plot), the player's position (front door).
- **His car**, parked in the void at (91.07, 0, 148.29): reset to his home's vehicle bay (one
  rule for every save: a vehicle outside every box is moved home).
- **His garden**: 1,976 crops, 1,575 of them dead (80%). Recommendation: wipe it with the rest.
  The home's machine ids do not change, so a kept garden WOULD carry over, but it is the
  perpetual showcase's planting, mostly dead, and the showcase replants itself whenever a Dev or
  Creative world has no crops (data/world/showcase.ron). Question 6.
- **His installed data folder** (`%APPDATA%\HumanityOS\data`, stamped v0.1525.0): the refresh
  replaces every file nobody edited with the new version and KEEPS one he saved from the editor
  (crates/humanity-core/src/storage/refresh.rs). Before the wipe ships, check Settings > Data for
  a kept ship file, home design or machine file; "Use the game's copy" moves his aside.

### 6.4 Rung 1: Neighbourhood 1, flat

The smallest correct slice the game can stand on. It is a rung of the real thing: the layout,
the coordinates and the relations are Sector 1's; only the curvature waits for rung 2.

**The frame.** Sector 1's drum floor, unrolled: x is the distance around the drum (s), z the
distance along it from the fore end cap (a), y up from the floor. N1 is the cell s 0 to 393, a 120
to 420 (3.4). In rung 2 one transform rolls every point: angle s / 250 m, radius 250 m minus y.

**What it holds** (Proposal; the implementer authors the exact rows):

```
 N1, rung 1, to scale: 1 character = about 6.5 m around, 10 m along (N1-local metres)
 s (around) ->
 0      55 65    120 130   185 195   250 260                         393
 +-------+--+------+--+------+--+------+--+-------------------------------+  a = 0
 |  A1   |  |  B1  |  |  C1  |  |  D1  |  |    cabin block (16 cabins     |
 |  door>|st| door>|st| door>|st| door>|st|    on a 48 m hall)            |
 |  @ 40 |  |      |  |      |  |      |  |                               |
 +-------+  +------+  +------+  +------+  |        N1 COMMONS             |  a = 89
 |  A2   |  |  B2  |  |  C2  |  |  D2  |  |   (mess hall, recreation,     |
 |       |  |      |  |      |  |      |  |    washroom, garden court,    |
 |       |  |      |  |      |  |      |  |    market row, exhibit room)  |
 +-------+  +------+  +------+  +------+  |        guest arrival          |  a = 188
 |  A3   |  |  B3  |  |  C3  |  |  D3  |  |                               |
 |       |  |      |  |      |  |      |  |       green / paths           |
 +-------+--+------+--+------+--+------+--+-------------------------------+  a = 300
   12 homestead plots, 55 x 89 m, doors spinward onto streets "st";
   everything stands on ONE ground layer (no gaps, no void)
```

1. **The ground layer** (new in the ship file): floor slabs under the whole cell with their
   materials (paving for streets, a planted ground elsewhere), walkable, allowed under plots and
   zones by `validate()`. It replaces the void and First Street. Shared air (the ship's).
2. **Twelve homestead plots** in four columns of three (55 x 89 m, 10 m streets, 10 m gaps), doors
   spinward onto the streets; the farthest door is 345 m from the commons (4.1 minutes).
3. **The cabin block**: the existing hall and sixteen 3.15 x 2.5 m cabins, moved beside the commons.
4. **N1's commons** (rebuilt): the mess hall, the recreation room and the washroom as labelled
   volumes; the garden in its own glass-roofed court (the towers, fish tank, composter, racks and
   apothecary towers, ids kept); the three market stalls along a wall; the showroom ring in its own
   exhibit room (question 7); the guest's arrival point in the open. Its machines on the ship's
   own meter.
5. **The hologram** placed by the home design (a fixture row in the study or the console room).
6. **No districts, no wild nodes, no fixed quest coordinates aboard.** The Survey quests read their
   ore from the mining loop (question 8).
7. **The hull wrap off by default** (it would put a lid over open ground; the drum of rung 2 is
   the real structure). The code and its H toggle stay (forever-development rule).
8. **The relay**: the same ship file, so its rooms are N1's commons, its volumes and the ground;
   chores, crew posts and the stores re-authored for the new commons; furniture from the game's
   own rows.

**The gates** (each written first and seen RED against today's data; the red line goes in the
test's comment, per this wave's rules):

| Test | Red today, expected message |
|---|---|
| `every_placed_thing_stands_on_a_floor` (machines of every file, lights, wild spawns, quest places, crew spots, store spots, spawns, structures, vehicles in the shipped saves) | 11 wild nodes in the void, e.g. "berry_bush#0 at (55.3, 82.0) is in no zone, plot, corridor or ground" |
| `no_ship_machine_crosses_a_wall` | "mush_c1 (mushroom_rack) at Commons (11.5, 29) stands on the wall (4,29)-(20,29)" |
| `every_light_is_inside_its_body` (ship zones and home designs) | "commons light warm_lamp at (-27.4, 0.8, 12.75) is outside the Commons" (six) |
| `no_district_without_a_floor` (a district is a walkable zone or absent) | "hangar-1 is outside the hull and no corridor reaches it" (thirteen) |
| `every_spawn_row_and_place_names_its_frame` | every row of wild_spawns.ron and four of destinations.ron |
| `the_hologram_stands_where_the_design_puts_it` | "the hologram is at (-0.5, 1, 2.5), outside the home" |
| `no_orphaned_data_file` (every compiled-in data file has a reader, exceptions listed in the test) | data/world/spawn.ron and five more |
| `no_ship_light_on_a_household_meter` | "the Commons' 25 lights are on household p1's meter" (today every zone's lights bill every home) |

Kept and re-pointed: `every_home_reaches_a_mess_hall_within_five_minutes`,
`every_plot_spawn_is_where_the_game_stands_on_it`, `the_shipped_ron_has_no_wall_coplanar_with_a_corridor_mouth`,
the relay's crew and furniture walks (`every_crew_walk_between_its_sites_is_clear_of_walls`,
`every_crew_walk_is_clear_of_the_ships_furniture`, `no_crew_figure_is_ever_on_a_plot`,
`every_room_of_the_ship_has_a_floor_of_its_own`), the home's own layout tests (unchanged: the home
design is kept).

**Runtime checks**: `just verify-runtime` (boot and enter the world), `just verify-starts` (a
homestead and a cabin land on N1), `just verify-copresence --plots` (two players walk from their
doors to the commons and meet), `just verify-household`, `just verify-shared-build`,
`just verify-storeys` and `just verify-falls` (the home's stairs, now on a plot that is not at the
origin), `just verify-screens` (the console room's screens, the chat monitor among them), the
audit's six vantages re-shot as "after" pictures, and new vantages `n1-overview`,
`n1-commons-garden`, `n1-cabin-row` judged by the visual-sweep.

### 6.5 The rungs after

Each rung is shippable alone, has its tests (red first) and its runtime check.

| Rung | What | Tests | Runtime check |
|---|---|---|---|
| 2. The drum | Roll Sector 1's floor: the transform (f64 on the CPU, CLAUDE.md's planet-scale rule), up is radial, homes rolled with it (walls radial, floors following the curve), the spin as a frame (the planet surface walk already takes `up` as an argument, habitat-generation.md:131-158), the far side drawn (L2), the sunline lights the floor | the transform round-trips within 1 mm at 250 m; a walk around the whole circumference returns to its start; footing on a rolled floor at every storey | a new `verify-drum` rig (walk 100 m spinward and back, up vector and footing judged); vantages `drum-floor-noon`, `drum-far-side` |
| 3. Neighbourhoods as data | templates, the generator, stable plot ids, plots that turn, N1 to N8 and H1 to H4 | the planning rules over the generated layout; ids stable under regeneration; no overlap | `verify-copresence` with a dozen players (the co-op floor) |
| 4. Arrival | the bow, Hangar Bay 1 as a walkable zone, the Arrivals hall and desk, the arrival lift down the fore end cap, General Services at the Fore Landing, the transport (game-setting-and-intro.md rungs 2 and 3) | the arrival path is floor end to end; every desk step reachable; respawn rows resolve | a new `verify-arrival` rig |
| 5. Engineering and the ship's supply | the spine's engineering segment, ship machines and lights on the ship's meter and ledger, hum sources at the engineering segment and the stern (`data/audio/hum_sources.ron`), the noise-buffer rule reading the same rows | no ship machine on a household meter; no home within the buffer | `verify-runtime`; the hum level read at three vantages |
| 6. Farms and freight | the farm band as real machines filling the mess halls' stores (replacing the stand-in "36 meals a game day"), parcel lockers, freight lanes, the Aft Landing depot, the spine freight rail | a meal's food traces to a farm row; freight never shares passenger paths | `verify-starts` (a cabin eats from the farms) |
| 7. Civic | the Concourse, the Medical Bay, the greenbelt (park, lake, arena field, school and Library hall) | 15 minutes to every daily need | vantages and a walk rig |
| 8. The exterior | the bow, the sectors, the stern drive, the radiators, the drums turning, at the size question 1 settles; the hull wrap retired | the exterior is built from the sector rows | `verify-runtime` exterior vantages |
| 9. MMO scale | claim order across neighbourhoods and sectors, interest by neighbourhood, crowd tiers 1 to 3 | per-frame cost flat as population grows (crowd-simulation.md) | `npc-crowd-stress` measurements |

### 6.6 What depends on today's placements, and how each moves

| Depends | Today | Moves how, and when |
|---|---|---|
| tests/visual/vantages.json: 30 aboard-the-home vantages (11 home views, 3 ship-scale, 3 screen-faced, 13 Maps) and their `expect` text (the golden specs) | 14 of them carry a pose in ship metres that assumes p1 at the origin (the camera request's "home-frame") | Rung 1 adds a plot-relative pose (`frame: "plot"`) to the camera request; home poses move to it unchanged; screen-faced ones need nothing (they park by screen id); `ship-first-street`, `ship-cabin-row`, `mess-hall-counter` are re-authored for N1 |
| scripts/home-vantages.json (32 poses) and scripts/photograph-home.js | "on plot p1 the home sits at the ship's origin, so they are ship metres too" | the same plot-relative frame, rung 1 |
| `just verify-copresence` | spawn (27.5, 1.7, 143.5) inside p2, meet pose (76, 1.7, 64) in the Commons, the door-points fixture scripts/tests/fixtures/door-points-p1.json | re-authored for N1 in rung 1; the fixture regenerated (it is regenerated on every ship change by design) |
| `just verify-falls`, `just verify-storeys` (scripts/lib/falls-judge.js, storeys-judge.js) | walk the home's stair in home-local metres | plot-relative; the home design is unchanged, so the walks are too |
| `just verify-screens`, `verify-live-screen` | park by screen id in the console room | nothing (the home design is kept) |
| `just verify-starts` (scripts/lib/starts-judge.js) | cabin row and homestead plots on First Street; a cabin's meal at the Commons | re-pointed to N1, rung 1 |
| The farm | crops keyed by machine and tower ids | ids kept (home and commons), so crops carry; the showcase replant is unchanged |
| The in-world screens, the wall screen showing chat (desk_monitor_2, page:chat, data/machines/home.ron:3709) | in the home design | nothing; the recreation room's screen (`rec_screen_1`) moves with N1's commons, id kept |
| Reactor metering per home | every zone's lights and the Commons' machines on each home's meter | rung 1: ship lights and ship machines leave household meters (the ground could not be lit otherwise); rung 5 builds the ship's own supply |
| The relay's world (rooms from the ship file), crew chores (data/npc/chores.ron), posts (crew.ron), the stores (data/food/ship_stores.ron) | spots chosen to dodge the room block | re-authored for N1's commons, rung 1 |
| Rust tests that read the shipped ship or home files | 52 files, 145 call sites; constants such as `P1_DOOR`, `P2_DOOR`, `COMMONS` (src/engine/home_plot.rs:1881-1884), the relay's `room_for_position` points (ship_world.rs:540-541), the stair tops (survival_env.rs:565-568) | rung 1 updates each; the home-local ones stay |
| crates/humanity-ship/src/hull.rs tests | count neighbours and lids of the twelve plots | rung 1 (the hull off by default; its tests keep testing the code on a fixture) |
| The ship's hum (music.md): `data/audio/hum_sources.ron`, positions in ship metres, the first drive source at "the ship's stern" | not yet built; if it lands before the wipe, its stern is today's slab's end past p12 | rung 1 keeps one drive source at Sector 1's aft end (axial 2,000 m, the spine) so the hum already rises toward the Aft Landing; rung 5 adds the engineering segment's sources |
| The Maps page's ship symbol and its rigs (scripts/lib/map-ship-check.js and the others) | the ship as one symbol in orbit | nothing in rung 1; rung 8 (the exterior) |
| Quests: Initial Survey, Distant Expeditions, the explore quest, getting_started | ore from the wild outcrop and vein; fixed places; "find your home" | rung 1 re-points the ore to the mining loop and the places to anchors (question 8) |

## 7. Questions for the operator

Each with a recommendation. The wipe itself waits for his read of this plan.

1. **How big is the whole mothership, and how many live aboard?** Sector 1 is the drum you sized
   (250 m x 2 km); it holds thousands. *Recommendation:* the ship is a spine of sectors whose
   count and sizes are data; Sector 1 is the small walkable one near the bow; later sectors are
   rings of kilometres (humanity_one.md's 100 km diameter at the far end), so "over a billion"
   stays true and costs nothing per frame (the aggregate tier). Tell us the number to write down.
2. **Cylinder or hyperboloid for Sector 1?** *Recommendation:* a plain cylinder, exactly your
   250 m x 2 km. A hyperboloid with a 150 m waist has a floor that slopes 9 degrees along the axis
   at its ends and would need terraces; keep the hyperboloid as the ship's outer form or a later
   sector.
3. **Where does a home's light come from inside the drum?** *Recommendation:* the spine's sunline,
   on the ship's day cycle: piped sunlight while the fleet is at a star, reactor lamps between
   stars. Glass roofs keep working (they see the sunline); the homestead's solar panels read the
   sunline's light, and between the stars the ledger shows that every panel watt was a reactor
   watt first. This ties to "is the fleet at Earth or between the stars", still open in
   game-setting-and-intro.md (question 2).
4. **One deck or many for homes?** *Recommendation:* one: the drum floor at 1 g. Homes keep their
   own storeys (the homestead's two), apartment blocks stack storeys inside their own buildings,
   and nobody lives in the spine (a tenth of a g or less).
5. **What do we keep of today's home?** *Recommendation:* the homestead design whole (your
   2026-09-19 redesign and the upper floor) as every homestead's template; drop the train
   platform, its ramp and the teleporter pair from it (transit belongs to the ship); a respawn pod
   beside the bedroom when respawn-and-revival.md settles how waking works.
6. **Your garden and save.** *Recommendation:* keep your character (name, look, outfit, skills,
   credits, inventory, quests); move your car home; wipe the garden (1,575 of 1,976 crops are
   dead, and the showcase replants itself in Dev and Creative). Your home's machine ids do not
   change, so keeping the garden is possible if you want it.
7. **The old walls in the Commons.** *Recommendation:* keep the eight-material showroom ring as a
   teaching exhibit in a room of its own in N1's commons (never trim teaching tools); retire the
   15-wall room block, which is the superseded 12-room house, not an exhibit.
8. **Berry bushes, salt flats and ore veins aboard?** *Recommendation:* none aboard: they are
   planet things. Aboard, ore comes from the asteroid mining loop, fibre, berries and wood from the
   ship's farms and orchards, and the Survey quests change to match.
9. **The districts.** *Recommendation:* take all fourteen out now; each comes back as a real,
   walkable place in its rung (the hangar with arrival, engineering with the ship's supply).
10. **Cruise thrust** (it sets the spine's gravity and the drum floor's lean). *Recommendation:*
    0.05 g: a 2.9 degree lean on the drum floor, and a spine where things weigh a twentieth.
11. **How far must homes be from the loud places?** (open question 14 since 2026-10-03)
    *Recommendation:* put every loud source in the spine or at the Aft Landing, and set the
    distance from a sleep guideline once researched and written down as a dated finding.
12. **Neighbourhood 1's mix.** *Recommendation:* the first neighbourhood holds the dozen homesteads
    and the cabin block side by side next to the landing (compact for the first server); later
    neighbourhoods follow the density rule (dense by the landing, homesteads by the farms).

## 8. Sources

Design documents read for this plan: [the-mothership.md](the-mothership.md),
[mothership-superstructure.md](mothership-superstructure.md), [ship-superstructure.md](ship-superstructure.md),
[ship-homes-and-logistics.md](ship-homes-and-logistics.md) and its increment plans
[5](ship-homes-increment-5-plan.md), [5b](ship-homes-increment-5b-plan.md), [6](ship-homes-increment-6-plan.md),
[7](ship-homes-increment-7-plan.md), [ship-life-support.md](ship-life-support.md),
[leaving-the-ship.md](leaving-the-ship.md), [homestead.md](homestead.md),
[homestead-solo-design.md](homestead-solo-design.md), [homestead-upper-floor.md](homestead-upper-floor.md),
[home-design.md](home-design.md), [player-home.md](player-home.md), [homes-as-profiles.md](homes-as-profiles.md),
[gameplay-loop-map.md](gameplay-loop-map.md), [game-setting-and-intro.md](game-setting-and-intro.md),
[habitat-generation.md](habitat-generation.md), [gravity-and-movement.md](gravity-and-movement.md),
[crowd-simulation.md](crowd-simulation.md), [closed-loop-test.md](closed-loop-test.md),
[game-modes.md](game-modes.md), [two-realities.md](two-realities.md),
[../game/ship_zoning_transit.md](../game/ship_zoning_transit.md), [../game/humanity_one.md](../game/humanity_one.md),
[../PRIORITIES.md](../PRIORITIES.md) (sections Day 4 and D), [../STATUS.md](../STATUS.md).
Data: data/blueprints/ship_structure.ron, data/homes/, data/machines/, data/entities/,
data/npc/, data/food/ship_stores.ron, data/stations/home.ron, data/ship_power.ron, data/sounds.toml.
